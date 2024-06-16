use std::collections::HashMap;
use std::sync::Arc;

use super::ChangeSet;
use crate::config::Project;
use crate::ext::fs;
use crate::ext::sync::{wait_interruptible, CommandResult};
use crate::service::site::SiteFile;
use crate::signal::{Interrupt, Outcome, Product};
use crate::{
    ext::{
        anyhow::{Context, Result},
        exe::Exe,
    },
    logger::GRAY,
};
use camino::{Utf8Path, Utf8PathBuf};
use swc::{config::JsMinifyOptions, try_with_handler, BoolOrDataConfig};
use swc_common::{FileName, SourceMap, GLOBALS};
use tokio::process::Child;
use tokio::{process::Command, sync::broadcast, task::JoinHandle};
use wasm_bindgen_cli_support::Bindgen;

pub async fn front(
    proj: &Arc<Project>,
    changes: &ChangeSet,
) -> JoinHandle<Result<Outcome<Product>>> {
    let proj = proj.clone();
    let changes = changes.clone();
    tokio::spawn(async move {
        if !changes.need_front_build() {
            log::trace!("Front no changes to rebuild");
            return Ok(Outcome::Success(Product::None));
        }

        fs::create_dir_all(&proj.site.root_relative_pkg_dir()).await?;

        let (envs, line, process) = front_cargo_process("build", true, &proj)?;

        log::debug!("Running {}", GRAY.paint(&line));
        match wait_interruptible("Cargo", process, Interrupt::subscribe_any()).await? {
            CommandResult::Interrupted => return Ok(Outcome::Stopped),
            CommandResult::Failure(_) => return Ok(Outcome::Failed),
            _ => {}
        }
        log::debug!("Cargo envs: {}", GRAY.paint(envs));
        log::info!("Cargo finished {}", GRAY.paint(line));

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

    let envs = proj.to_envs();

    let envs_str = envs
        .iter()
        .map(|(name, val)| format!("{name}={val}"))
        .collect::<Vec<_>>()
        .join(" ");

    command.args(&args).envs(envs);
    let line = super::build_cargo_command_string(args);
    (envs_str, line)
}

async fn bindgen(proj: &Project) -> Result<Outcome<Product>> {
    let wasm_file = &proj.lib.wasm_file;
    let interrupt = Interrupt::subscribe_any();

    log::info!("Front compiling WASM");

    // see:
    // https://github.com/rustwasm/wasm-bindgen/blob/main/crates/cli-support/src/lib.rs#L95
    // https://github.com/rustwasm/wasm-bindgen/blob/main/crates/cli/src/bin/wasm-bindgen.rs#L13
    let mut bindgen = Bindgen::new()
        .debug(proj.wasm_debug)
        .keep_debug(proj.wasm_debug)
        .input_path(&wasm_file.source)
        .web(true)
        .dot()?
        .generate_output()
        .dot()?;

    bindgen.wasm_mut().emit_wasm_file(&wasm_file.dest).dot()?;
    log::trace!("Front wrote wasm to {:?}", wasm_file.dest.as_str());
    if proj.release {
        match optimize(&wasm_file.dest, interrupt).await.dot()? {
            CommandResult::Interrupted => return Ok(Outcome::Stopped),
            CommandResult::Failure(_) => return Ok(Outcome::Failed),
            _ => {}
        }
    }

    let mut js_changed = false;

    js_changed |= write_snippets(proj, bindgen.snippets()).await?;

    js_changed |= write_modules(proj, bindgen.local_modules()).await?;

    let wasm_changed = proj
        .site
        .did_file_change(&proj.lib.wasm_file.as_site_file())
        .await
        .dot()?;

    js_changed |= if proj.release && proj.js_minify {
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

    log::debug!("Front js changed: {js_changed}");
    log::debug!("Front wasm changed: {wasm_changed}");

    if js_changed || wasm_changed {
        Ok(Outcome::Success(Product::Front))
    } else {
        Ok(Outcome::Success(Product::None))
    }
}

async fn optimize(
    file: &Utf8Path,
    interrupt: broadcast::Receiver<()>,
) -> Result<CommandResult<()>> {
    let wasm_opt = Exe::WasmOpt.get().await.dot()?;

    let args = [file.as_str(), "-Os", "-o", file.as_str()];
    let process = Command::new(wasm_opt)
        .args(args)
        .spawn()
        .context("Could not spawn command")?;
    wait_interruptible("wasm-opt", process, interrupt).await
}

fn minify<JS: AsRef<str>>(js: JS) -> Result<String> {
    let cm = Arc::<SourceMap>::default();

    let c = swc::Compiler::new(cm.clone());
    let output = GLOBALS.set(&Default::default(), || {
        try_with_handler(cm.clone(), Default::default(), |handler| {
            let fm = cm.new_source_file(FileName::Anon, js.as_ref().to_string());

            c.minify(
                fm,
                handler,
                &JsMinifyOptions {
                    compress: BoolOrDataConfig::from_bool(true),
                    mangle: BoolOrDataConfig::from_bool(true),
                    // keep_classnames: true,
                    // keep_fnames: true,
                    module: true,
                    ..Default::default()
                },
            )
            .context("failed to minify")
        })
    })?;

    Ok(output.code)
}

async fn write_snippets(proj: &Project, snippets: &HashMap<String, Vec<String>>) -> Result<bool> {
    let mut js_changed = false;

    // Provide inline JS files
    for (identifier, list) in snippets.iter() {
        for (i, js) in list.iter().enumerate() {
            let name = format!("inline{}.js", i);
            let site_path = Utf8PathBuf::from("snippets").join(identifier).join(name);
            let file_path = proj.site.root_relative_pkg_dir().join(&site_path);

            fs::create_dir_all(file_path.parent().unwrap()).await?;

            let site_file = SiteFile {
                dest: file_path,
                site: site_path,
            };

            js_changed |= if proj.release && proj.js_minify {
                proj.site
                    .updated_with(&site_file, minify(js)?.as_bytes())
                    .await?
            } else {
                proj.site.updated_with(&site_file, js.as_bytes()).await?
            }
        }
    }
    Ok(js_changed)
}

async fn write_modules(proj: &Project, modules: &HashMap<String, String>) -> Result<bool> {
    let mut js_changed = false;
    // Provide snippet files from JS snippets
    for (path, js) in modules.iter() {
        let site_path = Utf8PathBuf::from("snippets").join(path);
        let file_path = proj.site.root_relative_pkg_dir().join(&site_path);

        fs::create_dir_all(file_path.parent().unwrap()).await?;

        let site_file = SiteFile {
            dest: file_path,
            site: site_path,
        };

        js_changed |= if proj.release && proj.js_minify {
            proj.site
                .updated_with(&site_file, minify(js)?.as_bytes())
                .await?
        } else {
            proj.site.updated_with(&site_file, js.as_bytes()).await?
        };
    }
    Ok(js_changed)
}
