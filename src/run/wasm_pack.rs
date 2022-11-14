use crate::{config::Config, util};
use anyhow::{Context, Result};
use glob::glob;
use simplelog as log;
use std::fs;
use std::path::Path;
use xshell::{cmd, Shell};

pub fn run(command: &str, path: &str, config: &Config) -> Result<()> {
    try_build(command, &path, config).context(format!("wasm-pack {command} {path}"))?;

    util::rm_file(format!("target/site/pkg/.gitignore"))?;
    util::rm_file(format!("target/site/pkg/package.json"))?;
    prepend_snippets()?;
    util::rm_dir("target/site/pkg/snippets")?;
    Ok(())
}

pub fn try_build(command: &str, path: &str, config: &Config) -> Result<()> {
    let path_depth = Path::new(path).components().count();
    let to_root = (0..path_depth).map(|_| "..").collect::<Vec<_>>().join("/");

    let dest = format!("{to_root}/target/site/pkg");

    let sh = Shell::new()?;

    log::debug!("Running sh in path: <bold>{path}</>");
    sh.change_dir(path);

    let release = config.release.then(|| "--release").unwrap_or("--dev");
    let features = config.csr.then(|| "csr").unwrap_or("hydrate");

    cmd!(
        sh,
        "wasm-pack {command} --target web --out-dir {dest} --out-name app --no-typescript {release} -- --no-default-features --features={features}"
    )
    .run()?;
    Ok(())
}

/// wasm-pack generate snippets for each wasm_bindgen found including in dependencies.
/// these should be prepended to the target/site/app.js file
pub fn prepend_snippets() -> Result<()> {
    let mut found: Vec<String> = Vec::new();

    let pattern = "target/site/pkg/snippets/**/*.js";
    for entry in glob(pattern).context("Failed to read glob pattern")? {
        let path = entry?;
        found.push(fs::read_to_string(path)?);
    }
    if found.len() == 0 {
        return Ok(());
    }

    let app_js = "target/site/pkg/app.js";
    found.push(fs::read_to_string(app_js)?);
    fs::write(app_js, found.join("\n"))?;
    Ok(())
}
