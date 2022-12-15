use crate::ext::anyhow::{anyhow, Context, Result};
use crate::{
    ext::exe::{get_exe, Exe},
    fs,
    sync::{run_interruptible, src_or_style_change, wait_for},
    Config,
};
use camino::Utf8Path;
use tokio::process::Command;
use wasm_bindgen_cli_support::Bindgen;

use super::cargo;
pub async fn build(config: &Config) -> Result<()> {
    let config = config.clone();
    let handle = tokio::spawn(async move { run_build(&config).await });

    tokio::select! {
        val = handle => match val {
            Err(e) => Err(anyhow!(e)).dot(),
            Ok(Err(e)) => Err(e).dot(),
            Ok(_) => Ok(())
        },
        _ = wait_for(src_or_style_change) => Ok(())
    }
}

async fn run_build(config: &Config) -> Result<()> {
    cargo::build(config, true).await?;
    let wasm_path = config.cargo_wasm_file();

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

    let wasm_path = config.site_wasm_file().to_absolute_blocking();
    if config.cli.release {
        let path = config.site_wasm_file().with_extension("tmp.wasm");
        bindgen.wasm_mut().emit_wasm_file(&path).dot()?;
        optimize(&path, &wasm_path).await.dot()?;
    } else {
        bindgen.wasm_mut().emit_wasm_file(wasm_path).dot()?;
    }

    let module_js = bindgen
        .local_modules()
        .values()
        .map(|v| v.to_owned())
        .collect::<Vec<_>>()
        .join("\n");

    let snippets = bindgen
        .snippets()
        .values()
        .map(|v| v.join("\n"))
        .collect::<Vec<_>>()
        .join("\n");

    let js = snippets + &module_js + bindgen.js();
    fs::write("target/site/pkg/app.js", js).await.dot()?;
    Ok(())
}

async fn optimize(src: &Utf8Path, dest: &Utf8Path) -> Result<()> {
    let wasm_opt = get_exe(Exe::WasmOpt)
        .await
        .context("Try manually installing binaryen: https://github.com/WebAssembly/binaryen")?;

    let args = [src.as_str(), "-Os", "-o", dest.as_str()];
    let process = Command::new(wasm_opt)
        .args(&args)
        .spawn()
        .context("Could not spawn command")?;
    run_interruptible(src_or_style_change, "wasm-opt", process)
        .await
        .context(format!("wasm-opt {}", &args.join(" ")))?;
    std::fs::remove_file(&src).dot()?;
    Ok(())
}
