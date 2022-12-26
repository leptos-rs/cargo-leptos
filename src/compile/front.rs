use std::sync::Arc;

use super::ChangeSet;
use crate::config::Project;
use crate::ext::fs;
use crate::ext::sync::wait_interruptible;
use crate::signal::{Interrupt, Outcome, Product};
use crate::{
    ext::{
        anyhow::{Context, Result},
        exe::{get_exe, Exe},
    },
    logger::GRAY,
};
use camino::Utf8Path;
use itertools::Itertools;
use tokio::process::Child;
use tokio::{process::Command, sync::broadcast, task::JoinHandle};
use wasm_bindgen_cli_support::Bindgen;

pub async fn front(proj: &Arc<Project>, changes: &ChangeSet) -> JoinHandle<Result<Outcome>> {
    let proj = proj.clone();
    let changes = changes.clone();
    tokio::spawn(async move {
        if !changes.need_front_build() {
            log::trace!("Front no changes to rebuild");
            return Ok(Outcome::Success(Product::NoChange));
        }

        fs::create_dir_all(&proj.paths.site_pkg_dir).await?;

        let (envs, line, process) = front_cargo_process("build", true, &proj)?;

        if !wait_interruptible("Cargo", process, Interrupt::subscribe_any()).await? {
            return Ok(Outcome::Stopped);
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
        format!("--package={}", proj.front_package.name.as_str()),
        "--lib".to_string(),
        "--target-dir=target/front".to_string(),
    ];
    if wasm {
        args.push("--target=wasm32-unknown-unknown".to_string());
    }

    if !proj.config.lib_default_features {
        args.push("--no-default-features".to_string());
    }

    if !proj.config.lib_features.is_empty() {
        args.push(format!("--features={}", proj.config.lib_features.join(",")));
    }
    match proj.front_profile.as_str() {
        "release" => args.push("--release".to_string()),
        "dev" => {}
        prof => args.push(format!("--profile={prof}")),
    }

    let envs = proj.to_envs();

    let envs_str = envs
        .iter()
        .map(|(name, val)| format!("{name}={val}"))
        .collect::<Vec<_>>()
        .join(" ");

    command.args(&args).envs(envs);
    let line = format!("cargo {}", args.join(" "));
    (envs_str, line)
}

async fn bindgen(proj: &Project) -> Result<Outcome> {
    let wasm_file = &proj.paths.wasm_file;
    let interrupt = Interrupt::subscribe_any();

    // see:
    // https://github.com/rustwasm/wasm-bindgen/blob/main/crates/cli-support/src/lib.rs#L95
    // https://github.com/rustwasm/wasm-bindgen/blob/main/crates/cli/src/bin/wasm-bindgen.rs#L13
    let mut bindgen = Bindgen::new()
        .input_path(&wasm_file.source)
        .web(true)
        .dot()?
        .omit_imports(true)
        .generate_output()
        .dot()?;

    bindgen.wasm_mut().emit_wasm_file(&wasm_file.dest).dot()?;
    log::trace!("Front wrote wasm to {:?}", wasm_file.dest.as_str());
    if proj.optimise_front() && !optimize(&wasm_file.dest, interrupt).await.dot()? {
        return Ok(Outcome::Stopped);
    }

    let module_js = bindgen.local_modules().values().join("\n");

    let snippets = bindgen
        .snippets()
        .values()
        .map(|v| v.join("\n"))
        .collect::<Vec<_>>()
        .join("\n");

    let js = snippets + &module_js + bindgen.js();

    let wasm_changed = proj
        .site
        .did_file_change(&proj.paths.wasm_file.as_site_file())
        .await
        .dot()?;
    let js_changed = proj
        .site
        .updated_with(&proj.paths.js_file, js.as_bytes())
        .await
        .dot()?;
    log::debug!(
        "Front js {}",
        if js_changed { "changed" } else { "unchanged" }
    );
    log::debug!(
        "Front wasm {}",
        if wasm_changed { "changed" } else { "unchanged" }
    );
    if js_changed || wasm_changed {
        Ok(Outcome::Success(Product::ClientWasm))
    } else {
        Ok(Outcome::Success(Product::NoChange))
    }
}

async fn optimize(file: &Utf8Path, interrupt: broadcast::Receiver<()>) -> Result<bool> {
    let wasm_opt = get_exe(Exe::WasmOpt)
        .await
        .context("Try manually installing binaryen: https://github.com/WebAssembly/binaryen")?;

    let args = [file.as_str(), "-Os", "-o", file.as_str()];
    let process = Command::new(wasm_opt)
        .args(args)
        .spawn()
        .context("Could not spawn command")?;
    wait_interruptible("wasm-opt", process, interrupt).await
}
