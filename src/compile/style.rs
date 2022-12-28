use std::sync::Arc;

use super::ChangeSet;
use crate::{
    config::{Project, StyleConfig},
    ext::exe::Exe,
    ext::{
        anyhow::{anyhow, bail, Context, Result},
        PathBufExt,
    },
    fs,
    logger::GRAY,
    service::site::SourcedSiteFile,
    signal::{Outcome, Product},
};
use lightningcss::{
    stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet},
    targets::Browsers,
};
use tokio::process::Command;
use tokio::task::JoinHandle;

pub async fn style(proj: &Arc<Project>, changes: &ChangeSet) -> JoinHandle<Result<Outcome>> {
    let changes = changes.clone();
    let proj = proj.clone();

    tokio::spawn(async move {
        if !changes.need_style_build(true, false) {
            log::debug!("Style no build needed {changes:?}");
            return Ok(Outcome::Success(Product::NoChange));
        }
        Ok(Outcome::Success(build(&proj).await?))
    })
}

async fn build(proj: &Project) -> Result<Product> {
    let Some(style) = &proj.style else {
        log::trace!("Style not configured");
        return Ok(Product::NoChange);
    };
    fs::create_dir_all(style.file.dest.clone().without_last())
        .await
        .dot()?;

    log::debug!("Style found: {}", &style.file);

    match style.file.source.extension() {
        Some("sass") | Some("scss") => compile_sass(&style.file, proj.release)
            .await
            .context(format!("compile sass/scss: {}", &style.file))?,
        Some("css") => {
            fs::copy(&style.file.source, &style.file.dest).await.dot()?;
        }
        _ => bail!("Not a css/sass/scss style file: {}", &style.file),
    };

    process_css(proj, &style)
        .await
        .context(format!("process css {}", &style.file))
}

async fn compile_sass(style_file: &SourcedSiteFile, optimise: bool) -> Result<()> {
    let mut args = vec![style_file.source.as_str(), style_file.dest.as_str()];
    optimise.then(|| args.push("--no-source-map"));

    let exe = Exe::Sass.get().await.dot()?;

    let mut cmd = Command::new(exe).args(&args).spawn()?;

    log::trace!(
        "Style running {}",
        GRAY.paint(format!("sass {}", args.join(" ")))
    );

    cmd.wait()
        .await
        .context(format!("sass {}", args.join(" ")))?;

    log::trace!(
        "Style compiled sass {}",
        GRAY.paint(style_file.dest.to_string())
    );
    Ok(())
}

fn browser_lists(query: &str) -> Result<Option<Browsers>> {
    Browsers::from_browserslist([query]).context(format!("Error in browserlist query: {query}"))
}

async fn process_css(proj: &Project, style: &StyleConfig) -> Result<Product> {
    let browsers = browser_lists(&style.browserquery).context("leptos.style.browserquery")?;

    let css = fs::read_to_string(&style.file.dest).await?;

    let mut stylesheet =
        StyleSheet::parse(&css, ParserOptions::default()).map_err(|e| anyhow!("{e}"))?;

    if proj.release {
        stylesheet.minify(MinifyOptions::default())?;
    }

    let options = PrinterOptions::<'_> {
        targets: browsers,
        minify: proj.release,
        ..Default::default()
    };

    let style_output = stylesheet.to_css(options)?;

    let bytes = style_output.code.as_bytes();

    let prod = match proj
        .site
        .updated_with(&style.file.as_site_file(), bytes)
        .await?
    {
        true => {
            log::trace!(
                "Style finished with changes {}",
                GRAY.paint(&style.file.to_string())
            );
            Product::Style
        }
        false => {
            log::trace!("Style finished without changes");
            Product::NoChange
        }
    };
    Ok(prod)
}
