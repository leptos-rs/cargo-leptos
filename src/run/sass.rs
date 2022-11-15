use crate::config::Config;
use anyhow::{anyhow, ensure, Context, Result};
use lightningcss::{
    stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet},
    targets::Browsers,
};
use std::{fs, path::Path, path::PathBuf};
use tokio::process::Command;

pub async fn run(config: &Config) -> Result<()> {
    let style = &config.style;
    let scss_file = &style.file;

    log::debug!("Style found: {scss_file}");
    let scss = Path::new(scss_file);
    ensure!(scss.exists(), "no scss file found at: {scss_file}",);
    ensure!(scss.is_file(), "expected a file, not a dir: {scss_file}",);

    let css_file = compile_scss(scss_file, config.release)
        .await
        .context(format!("compile scss: {scss_file}"))?;

    let browsers = browser_lists(&style.browserquery).context("leptos.style.browserquery")?;

    process_css(&css_file, browsers, config.release).context(format!("process css {scss_file}"))?;

    Ok(())
}

async fn compile_scss(scss_file: &str, release: bool) -> Result<PathBuf> {
    let dest = "target/site/pkg/app.css";
    let mut args = vec![scss_file, dest];
    release.then(|| args.push("--no-source-map"));

    let mut cmd = Command::new("sass").args(&args).spawn()?;

    cmd.wait()
        .await
        .context(format!("sass {}", args.join(" ")))?;
    Ok(PathBuf::from(dest))
}

fn browser_lists(query: &str) -> Result<Option<Browsers>> {
    Browsers::from_browserslist([query]).context(format!("Error in browserlist query: {query}"))
}

fn process_css(file: &Path, browsers: Option<Browsers>, release: bool) -> Result<()> {
    let css = fs::read_to_string(&file)?;

    let mut style =
        StyleSheet::parse(&css, ParserOptions::default()).map_err(|e| anyhow!("{e}"))?;

    if release {
        style.minify(MinifyOptions::default())?;
    }

    let mut options = PrinterOptions::default();
    options.targets = browsers;

    if release {
        options.minify = true;
    }
    let style_output = style.to_css(options)?;

    fs::write(&file, style_output.code.as_bytes())?;

    Ok(())
}
