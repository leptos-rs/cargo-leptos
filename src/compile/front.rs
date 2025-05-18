use super::ChangeSet;
use crate::{
    config::Project,
    ext::{
        eyre::AnyhowCompatWrapErr,
        fs,
        sync::{wait_interruptible, CommandResult},
        PathBufExt,
    },
    internal_prelude::*,
    logger::GRAY,
    signal::{Interrupt, Outcome, Product},
};
use camino::Utf8Path;
use std::sync::Arc;
use swc::{
    config::{IsModule, JsMinifyOptions},
    try_with_handler, BoolOrDataConfig, JsMinifyExtras,
};
use swc_common::{FileName, SourceMap, GLOBALS};
use tokio::{
    process::{Child, Command},
    task::JoinHandle,
};
use wasm_bindgen_cli_support::Bindgen;
use wasm_opt::OptimizationOptions;

pub async fn front(
    proj: &Arc<Project>,
    changes: &ChangeSet,
) -> JoinHandle<Result<Outcome<Product>>> {
    let proj = proj.clone();
    let changes = changes.clone();
    tokio::spawn(async move {
        if !changes.need_front_build() {
            trace!("Front no changes to rebuild");
            return Ok(Outcome::Success(Product::None));
        }

        fs::create_dir_all(&proj.site.root_relative_pkg_dir()).await?;

        let (envs, line, process) = front_cargo_process("build", true, &proj)?;

        debug!("Running {}", GRAY.paint(&line));
        match wait_interruptible("Cargo", process, Interrupt::subscribe_any()).await? {
            CommandResult::Interrupted => return Ok(Outcome::Stopped),
            CommandResult::Failure(_) => return Ok(Outcome::Failed),
            _ => {}
        }
        debug!("Cargo envs: {}", GRAY.paint(envs));
        info!("Cargo finished {}", GRAY.paint(line));

        bindgen(&proj).await.dot()
    })
}

pub fn front_cargo_process(
    cmd: &str,
    wasm: bool,
    proj: &Project,
) -> Result<(String, String, Child)> {
    let mut command = Command::new("cargo");
    let (envs, line) = build_cargo_front_cmd(cmd, wasm, proj, &mut command);
    Ok((envs, line, command.spawn()?))
}

pub fn build_cargo_front_cmd(
    cmd: &str,
    wasm: bool,
    proj: &Project,
    command: &mut Command,
) -> (String, String) {
    let mut args = vec![
        cmd.to_string(),
        format!("--package={}", proj.lib.name.as_str()),
        "--lib".to_string(),
        format!("--target-dir={}", &proj.lib.front_target_path),
    ];

    if wasm {
        args.push("--target=wasm32-unknown-unknown".to_string());
    }

    if !proj.lib.default_features {
        args.push("--no-default-features".to_string());
    }

    if !proj.lib.features.is_empty() {
        args.push(format!("--features={}", proj.lib.features.join(",")));
    }

    // Add cargo flags to cargo command
    if let Some(cargo_args) = &proj.lib.cargo_args {
        args.extend_from_slice(cargo_args);
    }

    proj.lib.profile.add_to_args(&mut args);

    let envs = proj.to_envs(wasm);

    let envs_str = envs
        .iter()
        .map(|(name, val)| format!("{name}={val}"))
        .collect::<Vec<_>>()
        .join(" ");

    command.args(&args).envs(envs);

    let line = super::build_cargo_command_string(command);
    trace!(?envs_str, ?line, "Constructed cargo build front cmd");
    (envs_str, line)
}

async fn bindgen(proj: &Project) -> Result<Outcome<Product>> {
    let wasm_file = &proj.lib.wasm_file;

    info!("Front generating JS/WASM with wasm-bindgen");

    let start_time = tokio::time::Instant::now();
    // see:
    // https://github.com/rustwasm/wasm-bindgen/blob/main/crates/cli-support/src/lib.rs#L95
    // https://github.com/rustwasm/wasm-bindgen/blob/main/crates/cli/src/bin/wasm-bindgen.rs#L13
    let mut bindgen = Bindgen::new()
        .debug(proj.wasm_debug)
        .keep_debug(proj.wasm_debug)
        .input_path(&wasm_file.source)
        .out_name(&proj.lib.output_name)
        .web(true)
        .dot_anyhow()?
        .generate_output()
        .dot_anyhow()?;

    let bindgen_generate_end_time = tokio::time::Instant::now();

    debug!(
        "Finished generating wasm-bindgen output in {:?}",
        bindgen_generate_end_time - start_time
    );

    bindgen
        .emit(wasm_file.dest.clone().without_last())
        .dot_anyhow()?;

    let bindgen_emit_end_time = tokio::time::Instant::now();
    debug!(
        "Finished emitting wasm-bindgen in {:?}",
        bindgen_emit_end_time - bindgen_generate_end_time
    );

    // rename emitted wasm output file name from {output_name}_bg.wasm to {output_name}.wasm for
    // backward compatibility with leptos' `HydrationScripts`
    fs::rename(
        wasm_file
            .dest
            .clone()
            .without_last()
            .join(format!("{}_bg.wasm", &proj.lib.output_name)),
        &wasm_file.dest,
    )
    .await
    .dot()?;

    if proj.release {
        optimize(&wasm_file.dest)?;
    }

    let wasm_optimize_end_time = tokio::time::Instant::now();
    debug!(
        "Finished optimizing WASM in {:?}",
        wasm_optimize_end_time - bindgen_emit_end_time
    );

    if proj.js_minify {
        proj.site
            .updated_with(&proj.lib.js_file, minify(bindgen.js())?.as_bytes())
            .await
            .dot()?
    } else {
        proj.site
            .updated_with(&proj.lib.js_file, bindgen.js().as_bytes())
            .await
            .dot()?
    };

    let js_minify_end_time = tokio::time::Instant::now();
    debug!(
        "Finished minifying JS in {:?}",
        js_minify_end_time - wasm_optimize_end_time
    );

    let front_end_time = tokio::time::Instant::now();
    info!(
        "Finished generating JS/WASM for front in {:?}",
        front_end_time - start_time
    );

    Ok(Outcome::Success(Product::Front))
}

fn optimize(file: &Utf8Path) -> Result<()> {
    OptimizationOptions::new_optimize_for_size_aggressively()
        .run(file, file)
        .dot()
}

fn minify<JS: AsRef<str>>(js: JS) -> Result<String> {
    let cm = Arc::<SourceMap>::default();

    let c = swc::Compiler::new(cm.clone());
    let output = GLOBALS
        .set(&Default::default(), || {
            try_with_handler(cm.clone(), Default::default(), |handler| {
                let fm = cm.new_source_file(Arc::new(FileName::Anon), js.as_ref().to_string());

                use anyhow::Context;

                c.minify(
                    fm,
                    handler,
                    &JsMinifyOptions {
                        compress: BoolOrDataConfig::from_bool(true),
                        mangle: BoolOrDataConfig::from_bool(true),
                        // keep_classnames: true,
                        // keep_fnames: true,
                        module: IsModule::Bool(true),
                        ..Default::default()
                    },
                    JsMinifyExtras::default(),
                )
                .context("failed to minify")
            })
        })
        .map_err(|e| e.to_pretty_error())
        .wrap_anyhow_err("Failed to minify")?;

    Ok(output.code)
}
