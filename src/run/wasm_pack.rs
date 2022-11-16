use crate::{config::Config, util};
use anyhow::{Context, Result};
use glob::glob;
use std::fs;
use tokio::process::Command;

pub async fn build(config: &Config) -> Result<()> {
    let args = vec![
        "build",
        "--target",
        "web",
        "--out-dir",
        "target/site/pkg",
        "--out-name",
        "app",
        "--no-typescript",
        config.cli.release.then(|| "--release").unwrap_or("--dev"),
        "--",
        "--no-default-features",
        "--features",
        config.cli.csr.then(|| "csr").unwrap_or("hydrate"),
    ];

    Command::new("wasm-pack")
        .args(&args)
        .spawn()
        .context("Could not spawn child command")?
        .wait()
        .await
        .context(format!("wasm-pack build {}", args.join(" ")))?;

    util::rm_file(format!("target/site/pkg/.gitignore"))?;
    util::rm_file(format!("target/site/pkg/package.json"))?;
    prepend_snippets()?;
    util::rm_dir("target/site/pkg/snippets")?;
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
