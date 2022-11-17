use crate::{
    config::Config,
    util::{os_arch, PathBufAdditions},
    INSTALL_CACHE,
};
use anyhow::{anyhow, bail, ensure, Context, Result};
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

    let css_file = compile_scss(scss_file, config.cli.release)
        .await
        .context(format!("compile scss: {scss_file}"))?;

    let browsers = browser_lists(&style.browserquery).context("leptos.style.browserquery")?;

    process_css(&css_file, browsers, config.cli.release)
        .context(format!("process css {scss_file}"))?;

    Ok(())
}

async fn compile_scss(scss_file: &str, release: bool) -> Result<PathBuf> {
    let dest = "target/site/pkg/app.css";
    let mut args = vec![scss_file, dest];
    release.then(|| args.push("--no-source-map"));

    let exe = sass_exe().context("Try manually installing sass: https://sass-lang.com/install")?;
    log::debug!("Using sass executable at: {exe:?}");

    let mut cmd = Command::new(exe).args(&args).spawn()?;

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

fn sass_exe() -> Result<PathBuf> {
    // manually installed sass
    if let Ok(p) = which::which("sass") {
        return Ok(p);
    }

    // cargo-leptos installed sass
    let (target_os, target_arch) = os_arch()?;

    let dir = INSTALL_CACHE.join(Path::new("sass"));
    let exe_name = match target_os {
        "windows" => "sass.bat",
        _ => "sass",
    };

    let file = dir.with(exe_name);

    if file.exists() {
        return Ok(file);
    }

    // install cargo-leptos sass

    let version = "1.56.1";
    let url = match (target_os, target_arch) {
        ("windows", "x86_64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-windows-x64.zip"),
        ("macos" | "linux", "x86_64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-{target_os}-x64.tar.gz"),
        ("macos" | "linux", "aarch64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-{target_os}-arm64.tar.gz"),
        _ => bail!("Unable to download Sass for {target_os} {target_arch}")
      };

    let binaries = match target_os {
        "windows" => vec![exe_name, "src/dart.exe", "src/sass.snapshot"],
        "macos" => vec![exe_name, "src/dart", "src/sass.snapshot"],
        _ => vec![exe_name],
    };
    match INSTALL_CACHE.download(true, "sass", &binaries, &url) {
        Ok(None) => bail!("Unable to download Sass for {target_os} {target_arch}"),
        Err(e) => bail!("Unable to download Sass for {target_os} {target_arch} due to: {e}"),
        Ok(Some(d)) => Ok(d
            .binary(exe_name)
            .map_err(|e| anyhow!("Could not find {exe_name} in downloaded sass: {e}"))?),
    }
}
