use crate::{
    fs,
    sync::{run_interruptible, src_or_style_change, wait_for},
    util::os_arch,
    Config, INSTALL_CACHE,
};
use anyhow_ext::{anyhow, bail, Context, Result};
use std::path::{Path, PathBuf};
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
    let rel_dbg = config.cli.release.then(|| "release").unwrap_or("debug");

    cargo::build(config, true).await?;
    let wasm_path = Path::new(&config.target_directory)
        .join("wasm32-unknown-unknown")
        .join(rel_dbg)
        .join(&config.lib_crate_name())
        .with_extension("wasm");

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

    let wasm_path = "target/site/pkg/app.wasm";
    if config.cli.release {
        let path = "target/site/pkg/app.no-optimisation.wasm";
        bindgen.wasm_mut().emit_wasm_file(path).dot()?;
        optimize(path, wasm_path).await.dot()?;
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

async fn optimize(src: &str, dest: &str) -> Result<()> {
    let wasm_opt = wasm_opt_exe().dot()?;
    let args = [src, "-Os", "-o", dest];
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

fn wasm_opt_exe() -> Result<PathBuf> {
    // manually installed sass
    if let Ok(p) = which::which("wasm-opt") {
        return Ok(p);
    }

    // cargo-leptos installed sass
    let (target_os, target_arch) = os_arch()?;

    let binary = match target_os {
        "windows" => "bin/wasm-opt.exe",
        _ => "bin/wasm-opt",
    };

    let version = "version_111";
    let target = match (target_os, target_arch) {
        ("linux", _) => "x86_64-linux",
        ("windows", _) => "x86_64-windows",
        ("macos", "aarch64") => "arm64-macos",
        ("macos", "x86_64") => "x86_64-macos",
        _ => bail!("No wasm-opt tar binary found for {target_os} {target_arch}"),
    };
    let url = format!("https://github.com/WebAssembly/binaryen/releases/download/{version}/binaryen-{version}-{target}.tar.gz");

    let name = format!("wasm-opt-{version}");
    let binaries = match target_os {
        "windows" => vec!["bin/wasm-opt.exe", "lib/binaryen.lib"],
        "macos" => vec!["bin/wasm-opt", "lib/libbinaryen.dylib"],
        "linux" => vec!["bin/wasm-opt", "lib/libbinaryen.a"],
        _ => bail!("No wasm-opt binary found for {target_os}"),
    };
    match INSTALL_CACHE.download(true, &name, &binaries, &url) {
        Ok(None) => bail!("Unable to download wasm-opt for {target_os} {target_arch}"),
        Err(e) => bail!("Unable to download wasm-opt for {target_os} {target_arch} due to: {e}"),
        Ok(Some(d)) => d
            .binary(binary)
            .map_err(|e| anyhow!("Could not find {binary} in downloaded wasm-opt: {e}")),
    }
}
