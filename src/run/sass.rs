use crate::config::Config;
use anyhow::{anyhow, ensure, Context, Result};
use lightningcss::{
    stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet},
    targets::Browsers,
};
use std::{fs, path::Path, path::PathBuf};
use xshell::{cmd, Shell};

pub fn run(config: &Config) -> Result<()> {
    let style = &config.style;
    let scss_file = &style.file;

    log::debug!("Style found: {scss_file:?}");
    let scss_file = Path::new(scss_file);
    ensure!(scss_file.exists(), "no scss file found at: {scss_file:?}",);
    ensure!(
        scss_file.is_file(),
        "expected an scss file, not a dir: {scss_file:?}",
    );
    let css_file =
        compile_scss(scss_file, config.release).context(format!("compile scss: {scss_file:?}"))?;

    let browsers = browser_lists(&style.browserquery).context("leptos.style.browserquery")?;

    process_css(&css_file, browsers, config.release)
        .context(format!("process css {}", scss_file.to_string_lossy()))?;

    Ok(())
}

fn compile_scss(file: &Path, release: bool) -> Result<PathBuf> {
    let dest = format!("target/site/pkg/app.css");
    let sourcemap = release.then(|| "--no-source-map");

    let sh = Shell::new()?;
    cmd!(sh, "sass {file} {dest} {sourcemap...}").run()?;
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
