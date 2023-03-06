use std::collections::HashMap;
use std::sync::Arc;

use super::ChangeSet;
use crate::config::Project;
use crate::ext::fs;
use crate::ext::sync::{wait_interruptible, CommandResult};
use crate::signal::{Interrupt, Outcome, Product};
use crate::{
    ext::{
        anyhow::{Context, Result},
        exe::Exe,
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
            return Ok(Outcome::Success(Product::None));
        }

        fs::create_dir_all(&proj.site.root_relative_pkg_dir()).await?;

        let (envs, line, process) = front_cargo_process("build", true, &proj)?;

        match wait_interruptible("Cargo", process, Interrupt::subscribe_any()).await? {
            CommandResult::Interrupted => return Ok(Outcome::Stopped),
            CommandResult::Failure => return Ok(Outcome::Failed),
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
        "--target-dir=target/front".to_string(),
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

    proj.lib.profile.add_to_args(&mut args);

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
    let wasm_file = &proj.lib.wasm_file;
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
    if proj.release {
        match optimize(&wasm_file.dest, interrupt).await.dot()? {
            CommandResult::Interrupted => return Ok(Outcome::Stopped),
            CommandResult::Failure => return Ok(Outcome::Failed),
            _ => {}
        }
    }

    let module_js = bindgen.local_modules().values().join("\n");

    let snippets = values_sorted_by_key(&bindgen.snippets())
        .iter()
        .map(|v| v.join("\n"))
        .collect::<Vec<_>>()
        .join("\n");

    let js = snippets + &module_js + bindgen.js();

    let wasm_changed = proj
        .site
        .did_file_change(&proj.lib.wasm_file.as_site_file())
        .await
        .dot()?;
    let js_changed = proj
        .site
        .updated_with(&proj.lib.js_file, js.as_bytes())
        .await
        .dot()?;
    log::debug!("Front js changed: {js_changed}");
    log::debug!("Front wasm changed: {wasm_changed}");

    log::info!("WASM changed: {wasm_changed}");
    log::info!("JS changed: {js_changed}");
    log::info!(
        "JS_snippet paths {:?}",
        bindgen.snippets().keys().collect::<Vec<_>>()
    );
    log::info!(
        "JS_module paths {:?}",
        bindgen.local_modules().keys().collect::<Vec<_>>()
    );

    if js_changed || wasm_changed {
        Ok(Outcome::Success(Product::Front))
    } else {
        Ok(Outcome::Success(Product::None))
    }
}

async fn optimize(file: &Utf8Path, interrupt: broadcast::Receiver<()>) -> Result<CommandResult> {
    let wasm_opt = Exe::WasmOpt.get().await.dot()?;

    let args = [file.as_str(), "-Os", "-o", file.as_str()];
    let process = Command::new(wasm_opt)
        .args(args)
        .spawn()
        .context("Could not spawn command")?;
    wait_interruptible("wasm-opt", process, interrupt).await
}

fn values_sorted_by_key<T>(map: &HashMap<String, T>) -> Vec<&T> {
    let mut keys = map.keys().collect::<Vec<_>>();
    keys.sort();
    keys.iter().map(|key| map.get(*key).unwrap()).collect()
}
