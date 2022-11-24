use crate::{fs, util::os_arch, Config, INSTALL_CACHE};
use anyhow_ext::{anyhow, bail, Context, Result};
use lightningcss::{
    stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet},
    targets::Browsers,
};
use std::{path::Path, path::PathBuf};
use tokio::process::Command;

const DEST: &str = "target/site/pkg/app.css";

pub async fn run(config: &Config) -> Result<()> {
    fs::create_dir_all("target/site/pkg").await.dot()?;

    let style = &config.leptos.style;
    let style_file = &style.file;

    log::debug!("Style found: {style_file}");
    let file = PathBuf::from(style_file);

    let css_file = match file.extension().map(|ext| ext.to_str()).flatten() {
        Some("sass") | Some("scss") => compile_sass(style_file, config.cli.release)
            .await
            .context(format!("compile sass/scss: {style_file}"))?,
        Some("css") => {
            fs::copy(style_file, DEST).await.dot().dot()?;
            PathBuf::from(DEST)
        }
        _ => bail!("Not a css/sass/scss style file: {style_file}"),
    };

    let browsers = browser_lists(&style.browserquery).context("leptos.style.browserquery")?;

    process_css(&css_file, browsers, config.cli.release)
        .await
        .context(format!("process css {style_file}"))?;

    Ok(())
}

async fn compile_sass(scss_file: &str, release: bool) -> Result<PathBuf> {
    let mut args = vec![scss_file, DEST];
    release.then(|| args.push("--no-source-map"));

    let exe = sass_exe().context("Try manually installing sass: https://sass-lang.com/install")?;
    log::debug!("Sass using executable at: {exe:?}");

    let mut cmd = Command::new(exe).args(&args).spawn()?;

    cmd.wait()
        .await
        .context(format!("sass {}", args.join(" ")))?;
    Ok(PathBuf::from(DEST))
}

fn browser_lists(query: &str) -> Result<Option<Browsers>> {
    Browsers::from_browserslist([query]).context(format!("Error in browserlist query: {query}"))
}

async fn process_css(file: &Path, browsers: Option<Browsers>, release: bool) -> Result<()> {
    let css = fs::read_to_string(&file).await?;

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

    fs::write(&file, style_output.code.as_bytes()).await?;

    Ok(())
}

fn sass_exe() -> Result<PathBuf> {
    // manually installed sass
    if let Ok(p) = which::which("sass") {
        return Ok(p);
    }

    // cargo-leptos installed sass
    let (target_os, target_arch) = os_arch()?;

    let binary = match target_os {
        "windows" => "sass.bat",
        _ => "sass",
    };

    let version = "1.56.1";
    let url = match (target_os, target_arch) {
        ("windows", "x86_64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-windows-x64.zip"),
        ("macos" | "linux", "x86_64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-{target_os}-x64.tar.gz"),
        ("macos" | "linux", "aarch64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-{target_os}-arm64.tar.gz"),
        _ => bail!("No sass binary found for {target_os} {target_arch}")
      };

    let name = format!("sass-{version}");
    let binaries = match target_os {
        "windows" => vec![binary, "src/dart.exe", "src/sass.snapshot"],
        "macos" => vec![binary, "src/dart", "src/sass.snapshot"],
        _ => vec![binary],
    };
    match INSTALL_CACHE.download(true, &name, &binaries, &url) {
        Ok(None) => bail!("Unable to download sass for {target_os} {target_arch}"),
        Err(e) => bail!("Unable to download sass for {target_os} {target_arch} due to: {e}"),
        Ok(Some(d)) => Ok(d
            .binary(binary)
            .map_err(|e| anyhow!("Could not find {binary} in downloaded sass: {e}"))?),
    }
}
