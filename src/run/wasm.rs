use crate::{
    config::Config,
    util::{os_arch, run_interruptible},
    INSTALL_CACHE,
};
use anyhow::{anyhow, bail, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tokio::process::Command;
use wasm_bindgen_cli_support::Bindgen;

use super::cargo;

pub async fn build(config: &Config) -> Result<()> {
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
        .web(true)?
        .omit_imports(true)
        .generate_output()?;

    let wasm_path = "target/site/pkg/app.wasm";
    if config.cli.release {
        let path = "target/site/pkg/app.no-optimisation.wasm";
        bindgen.wasm_mut().emit_wasm_file(path)?;
        optimize(path, wasm_path).await?;
    } else {
        bindgen.wasm_mut().emit_wasm_file(wasm_path)?;
    }

    let snippets = bindgen
        .snippets()
        .values()
        .map(|v| v.join("\n"))
        .collect::<Vec<_>>()
        .join("\n");

    let js = snippets + bindgen.js();
    fs::write("target/site/pkg/app.js", js)?;
    Ok(())
}

async fn optimize(src: &str, dest: &str) -> Result<()> {
    let wasm_opt = wasm_opt_exe()?;
    let args = [src, "-Os", "-o", dest];
    let process = Command::new(wasm_opt)
        .args(&args)
        .spawn()
        .context("Could not spawn command")?;
    run_interruptible("wasm-opt", process)
        .await
        .context(format!("wasm-opt {}", &args.join(" ")))?;
    std::fs::remove_file(&src)?;
    Ok(())
}

fn wasm_opt_exe() -> Result<PathBuf> {
    // manually installed sass
    if let Ok(p) = which::which("wasm-opt") {
        return Ok(p);
    }

    // cargo-leptos installed sass
    let (target_os, target_arch) = os_arch()?;

    let exe_name = match target_os {
        "windows" => "bin/wasm-opt.exe",
        _ => "bin/wasm-opt",
    };

    // install cargo-leptos sass

    let version = "version_110";
    let target = match (target_os, target_arch) {
        ("linux", _) => "x86_64-linux",
        ("windows", _) => "x86_64-windows",
        ("macos", "aarch64") => "arm64-macos",
        ("macos", "x86_64") => "x86_64-macos",
        _ => bail!("No wasm-opt tar binary found for {target_os} {target_arch}"),
    };
    let url = format!("https://github.com/WebAssembly/binaryen/releases/download/{version}/binaryen-{version}-{target}.tar.gz");

    let binaries = match target_os {
        "windows" => vec!["bin/wasm-opt.exe", "lib/binaryen.lib"],
        "macos" => vec!["bin/wasm-opt", "lib/libbinaryen.dylib"],
        "linux" => vec!["bin/wasm-opt", "lib/libbinaryen.a"],
        _ => bail!("No wasm-opt binary found for {target_os}"),
    };
    match INSTALL_CACHE.download(true, "wasm-opt", &binaries, &url) {
        Ok(None) => bail!("Unable to download Sass for {target_os} {target_arch}"),
        Err(e) => bail!("Unable to download Sass for {target_os} {target_arch} due to: {e}"),
        Ok(Some(d)) => d
            .binary(exe_name)
            .map_err(|e| anyhow!("Could not find {exe_name} in downloaded wasm-opt: {e}")),
    }
}
