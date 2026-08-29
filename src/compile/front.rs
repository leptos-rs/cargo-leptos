use super::{Change, ChangeSet};
use crate::{
    config::Project,
    ext::{
        eyre::AnyhowCompatWrapErr,
        fs,
        sync::{wait_interruptible, wait_piped_interruptible, CommandResult, OutputExt},
        Exe, PathBufExt,
    },
    internal_prelude::*,
    logger::GRAY,
    signal::{Interrupt, Outcome, Product},
    wasm_split_tools,
};
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use swc::{
    config::{IsModule, JsMinifyOptions},
    try_with_handler, BoolOrDataConfig, JsMinifyExtras,
};
use swc_common::{FileName, SourceMap, GLOBALS};
use tokio::{
    process::{Child, Command},
    task::JoinHandle,
};

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

        let pkg_dir = proj.site.root_relative_pkg_dir();

        let mut files = vec![proj.lib.wasm_file.dest.clone()];

        fs::create_dir_all(&pkg_dir).await?;

        let (envs, line, process) = front_cargo_process("build", true, &proj)?;

        debug!("Running {}", GRAY.paint(&line));
        match wait_interruptible("Cargo", process, Interrupt::subscribe_any()).await? {
            CommandResult::Interrupted => return Ok(Outcome::Stopped),
            CommandResult::Failure(_) => return Ok(Outcome::Failed),
            _ => {}
        }
        debug!("Cargo envs: {}", GRAY.paint(envs));
        info!("Cargo finished {}", GRAY.paint(line));

        let previous_wasm_hash = take_front_wasm_hash(&proj);

        let input_wasm = tokio::fs::read(&proj.lib.wasm_file.source).await?;
        let wasm_hash = seahash::hash(&input_wasm);
        if front_wasm_unchanged(&proj, previous_wasm_hash, wasm_hash) {
            // Re-insert the entry taken above: the output it describes is
            // untouched.
            record_front_wasm(&proj, wasm_hash);
            info!("Finished generating JS/WASM for front (wasm unchanged; reusing previous output)");
            // A watched additional file can influence the running app without
            // changing the wasm (that is why it is watched): keep the browser
            // reload the full pipeline used to cause for those changes.
            let product = if changes.contains(&Change::Additional) {
                Product::Assets
            } else {
                Product::None
            };
            return Ok(Outcome::Success(product));
        }

        if proj.split {
            info!("Front splitting out lazy-loaded WASM files");
            let start_time = tokio::time::Instant::now();

            let split_files = wasm_split_tools::wasm_split(&input_wasm, false, &proj).await?;
            files.extend(split_files);

            let end_time = tokio::time::Instant::now();

            info!("Finished WASM splitting in {:?}", end_time - start_time);
        }

        // The module can be gigabytes; release it before wasm-bindgen loads
        // its own copy.
        drop(input_wasm);

        let outcome = bindgen(&proj, &files).await.dot();
        if let Ok(Outcome::Success(_)) = &outcome {
            record_front_wasm(&proj, wasm_hash);
        }
        outcome
    })
}

fn front_wasm_registry() -> &'static Mutex<HashMap<String, u64>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}

/// Removes and returns the hash of the wasm consumed by the last successful
/// split/bindgen run in this process. Taken rather than read so the entry
/// only exists while the output it describes is intact: the caller re-inserts
/// it once this run ends with the output known good, and a failed or
/// interrupted run leaves it absent.
fn take_front_wasm_hash(proj: &Project) -> Option<u64> {
    front_wasm_registry()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(proj.lib.wasm_file.source.as_str())
}

/// Reports whether the front wasm artifact (hashed into `hash`) is
/// byte-identical to the input of the last successful split/bindgen run in
/// this process (`previous_hash`, taken from the registry by the caller).
///
/// A rebuild caused by a change that only affects the server binary (or a
/// watched non-Rust file) does not alter the wasm: splitting and wasm-bindgen
/// would reproduce identical output, so the caller can skip them and keep the
/// previous output. A content hash rather than mtime, because a relink from
/// unchanged inputs rewrites the file without changing its bytes. The first
/// build in a process always reports "changed" (the site directory is
/// repopulated at startup), and so does a missing bindgen output.
fn front_wasm_unchanged(proj: &Project, previous_hash: Option<u64>, hash: u64) -> bool {
    previous_hash == Some(hash) && proj.lib.js_file.dest.exists()
}

/// Records the hash of the wasm the current site output was generated from,
/// so the next build can skip the pipeline when the wasm is unchanged.
/// Reached only with that output known good -- after a successful pipeline
/// run, or on a skip that left it untouched; after a failed or interrupted
/// run the entry stays absent and the pipeline runs again.
fn record_front_wasm(proj: &Project, hash: u64) {
    front_wasm_registry()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(proj.lib.wasm_file.source.to_string(), hash);
}

pub fn front_cargo_process(
    cmd: &str,
    wasm: bool,
    proj: &Project,
) -> Result<(String, String, Child)> {
    front_cargo_process_with_args(cmd, wasm, proj, None)
}

pub fn front_cargo_process_with_args(
    cmd: &str,
    wasm: bool,
    proj: &Project,
    additional_args: Option<&[String]>,
) -> Result<(String, String, Child)> {
    let mut command = Command::new("cargo");
    let (envs, line) = build_cargo_front_cmd(cmd, wasm, proj, &mut command, additional_args);
    Ok((envs, line, command.spawn()?))
}

pub fn build_cargo_front_cmd(
    cmd: &str,
    wasm: bool,
    proj: &Project,
    command: &mut Command,
    additional_args: Option<&[String]>,
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
    args.extend_from_slice(&proj.lib.cargo_args);

    proj.lib.profile.add_to_args(&mut args);

    if let Some(add_args) = additional_args {
        args.extend_from_slice(add_args);
    }

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

async fn bindgen(proj: &Project, all_wasm_files: &[Utf8PathBuf]) -> Result<Outcome<Product>> {
    let wasm_file = &proj.lib.wasm_file;

    info!("Front generating JS/WASM with wasm-bindgen");

    let wasm_file_input = if proj.split {
        let mut source = proj.lib.wasm_file.source.clone();
        source.set_file_name(format!("{}_split.wasm", source.file_stem().unwrap()));
        source
    } else {
        proj.lib.wasm_file.source.clone()
    };

    let start_time = tokio::time::Instant::now();
    /* // see:
    // https://github.com/rustwasm/wasm-bindgen/blob/main/crates/cli-support/src/lib.rs#L95
    // https://github.com/rustwasm/wasm-bindgen/blob/main/crates/cli/src/bin/wasm-bindgen.rs#L13
    let mut bindgen = Bindgen::new()
        .keep_lld_exports(proj.split)
        .demangle(!proj.split)
        .debug(proj.wasm_debug)
        .keep_debug(proj.wasm_debug)
        .input_path(&wasm_file_input)
        .out_name(&proj.lib.output_name)
        .web(true)
        .dot_anyhow()?
        .generate_output()
        .dot_anyhow()?; */

    let wasm_bindgen = Exe::WasmBindgen {
        project_root: &proj.working_dir,
    }
    .get()
    .await
    .dot()?;

    let args = [
        Some("--target=web".to_string()),
        proj.split.then(|| "--keep-lld-exports".into()),
        proj.split.then(|| "--no-demangle".into()),
        proj.wasm_debug.then(|| "--debug".into()),
        proj.wasm_debug.then(|| "--keep-debug".into()),
        Some(format!("--out-name={}", proj.lib.output_name)),
        Some(format!(
            "--out-dir={}",
            wasm_file.dest.clone().without_last()
        )),
        Some(wasm_file_input.into()),
    ]
    .into_iter()
    .flatten();

    let mut cmd = Command::new(wasm_bindgen);
    cmd.args(args.clone());

    match wait_piped_interruptible(
        "wasm-bindgen",
        cmd,
        crate::signal::Interrupt::subscribe_any(),
    )
    .await?
    {
        CommandResult::Interrupted => Ok(Outcome::Stopped),
        CommandResult::Failure(output) => {
            error!("wasm-bindgen failed with:");
            println!("{}", output.stderr());
            bail!("wasm-bindgen failed")
        }
        CommandResult::Success(_) => {
            let bindgen_emit_end_time = tokio::time::Instant::now();
            debug!(
                "Finished emitting wasm-bindgen in {:?}",
                bindgen_emit_end_time - start_time
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
                for file in all_wasm_files {
                    optimize(proj, file).await?;
                }
            }

            let wasm_optimize_end_time = tokio::time::Instant::now();
            debug!(
                "Finished optimizing WASM in {:?}",
                wasm_optimize_end_time - bindgen_emit_end_time
            );

            if proj.js_minify {
                let js_file_name = wasm_file
                    .dest
                    .clone()
                    .without_last()
                    .join(format!("{}.js", &proj.lib.output_name));
                let js = fs::read_to_string(&js_file_name).await?;
                proj.site
                    .updated_with(&proj.lib.js_file, minify(&js)?.as_bytes())
                    .await
                    .dot()?;

                let js_minify_end_time = tokio::time::Instant::now();
                debug!(
                    "Finished minifying JS in {:?}",
                    js_minify_end_time - wasm_optimize_end_time
                );
            };

            let front_end_time = tokio::time::Instant::now();
            info!(
                "Finished generating JS/WASM for front in {:?}",
                front_end_time - start_time
            );

            Ok(Outcome::Success(Product::Front))
        }
    }
}

async fn optimize(proj: &Project, file: &Utf8Path) -> Result<()> {
    let wasm_opt = Exe::WasmOpt.get().await.dot()?;

    let mut args: Vec<&str> = if let Some(features) = &proj.wasm_opt_features {
        features.iter().map(|f| f.as_str()).collect()
    } else {
        vec![
            "-Oz",
            "--enable-bulk-memory",
            "--enable-nontrapping-float-to-int",
        ]
    };
    args.extend_from_slice(&[file.as_str(), "-o", file.as_str()]);

    let mut cmd = Command::new(wasm_opt);
    cmd.args(args.clone());

    trace!("WASM running wasm-opt {}", args.join(" "));

    match wait_piped_interruptible("wasm-opt", cmd, crate::signal::Interrupt::subscribe_any())
        .await?
    {
        CommandResult::Success(_) => Ok(()),
        CommandResult::Interrupted => bail!("wasm-opt was interrupted"),
        CommandResult::Failure(output) => {
            error!("wasm-opt failed with:");
            println!("{}", output.stderr());
            bail!("wasm-opt optimization failed")
        }
    }
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
