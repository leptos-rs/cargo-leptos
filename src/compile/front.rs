use super::ChangeSet;
use crate::ext::fs;
use crate::service::site;
use crate::signal::{Interrupt, Outcome, Product};
use crate::{config::Config, ext::sync::wait_interruptible};
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

pub async fn front(conf: &Config, changes: &ChangeSet) -> JoinHandle<Result<Outcome>> {
    let conf = conf.clone();
    let changes = changes.clone();
    tokio::spawn(async move {
        if !changes.need_front_build() {
            log::trace!("Front no changes to rebuild");
            return Ok(Outcome::Success(Product::NoChange));
        }

        fs::create_dir_all(conf.pkg_dir().to_absolute().await).await?;

        let (line, process) = front_cargo_process("build", true, &conf)?;

        if !wait_interruptible("Cargo", process, Interrupt::subscribe_any()).await? {
            return Ok(Outcome::Stopped);
        }
        log::info!("Cargo finished {}", GRAY.paint(line));

        bindgen(&conf).await.dot()
    })
}

pub fn front_cargo_process(cmd: &str, wasm: bool, conf: &Config) -> Result<(String, Child)> {
    let mut args = vec![
        cmd,
        "--no-default-features",
        "--features=hydrate",
        "--lib",
        "--target-dir=target/front",
    ];

    if wasm {
        args.push("--target=wasm32-unknown-unknown");
    }
    if conf.cli.release {
        args.push("--release");
    }

    let process = Command::new("cargo")
        .args(&args)
        .envs(conf.to_envs())
        .spawn()
        .context("Could not spawn command")?;
    let line = format!("cargo {}", args.join(" "));
    Ok((line, process))
}

async fn bindgen(conf: &Config) -> Result<Outcome> {
    let wasm_path = conf.cargo_wasm_file();
    let interrupt = Interrupt::subscribe_any();

    // see:
    // https://github.com/rustwasm/wasm-bindgen/blob/main/crates/cli-support/src/lib.rs#L95
    // https://github.com/rustwasm/wasm-bindgen/blob/main/crates/cli/src/bin/wasm-bindgen.rs#L13
    let mut bindgen = Bindgen::new()
        .input_path(wasm_path)
        .web(true)
        .dot()?
        .omit_imports(true)
        .generate_output()
        .dot()?;

    let abs_wasm_path = conf.site_wasm_file().to_absolute().await;
    bindgen.wasm_mut().emit_wasm_file(&abs_wasm_path).dot()?;
    log::trace!("Front wrote wasm to {:?}", abs_wasm_path.as_str());
    if conf.cli.release && !optimize(&abs_wasm_path, interrupt).await.dot()? {
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

    let wasm_changed = site::did_file_change(&conf.site_wasm_file()).await.dot()?;
    let js_changed = site::write_if_changed(&conf.site_js_file(), js.as_bytes())
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
