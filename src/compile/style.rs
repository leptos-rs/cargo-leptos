use std::sync::Arc;

use super::ChangeSet;
use crate::{
    config::Project,
    ext::exe::{get_exe, Exe},
    ext::{
        anyhow::{anyhow, bail, Context, Result},
        path::PathBufExt,
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
    let style_file = match &proj.paths.style_file {
        Some(f) => f,
        None => {
            log::trace!("Style no style_file configured");
            return Ok(Product::NoChange);
        }
    };
    fs::create_dir_all(style_file.dest.clone().without_last())
        .await
        .dot()?;

    log::debug!("Style found: {style_file}");

    match style_file.source.extension() {
        Some("sass") | Some("scss") => compile_sass(&style_file, proj.optimise_front())
            .await
            .context(format!("compile sass/scss: {style_file}"))?,
        Some("css") => {
            fs::copy(&style_file.source, &style_file.dest).await.dot()?;
        }
        _ => bail!("Not a css/sass/scss style file: {style_file}"),
    };

    process_css(proj, &style_file)
        .await
        .context(format!("process css {style_file}"))
}

async fn compile_sass(style_file: &SourcedSiteFile, optimise: bool) -> Result<()> {
    let mut args = vec![style_file.source.as_str(), style_file.dest.as_str()];
    optimise.then(|| args.push("--no-source-map"));

    let exe = get_exe(Exe::Sass)
        .await
        .context("Try manually installing sass: https://sass-lang.com/install")?;

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

async fn process_css(proj: &Project, style_file: &SourcedSiteFile) -> Result<Product> {
    let browsers =
        browser_lists(&proj.front_config.browserquery).context("leptos.style.browserquery")?;

    let css = fs::read_to_string(&style_file.dest).await?;

    let mut style =
        StyleSheet::parse(&css, ParserOptions::default()).map_err(|e| anyhow!("{e}"))?;

    if proj.optimise_front() {
        style.minify(MinifyOptions::default())?;
    }

    let options = PrinterOptions::<'_> {
        targets: browsers,
        minify: proj.optimise_front(),
        ..Default::default()
    };

    let style_output = style.to_css(options)?;

    let bytes = style_output.code.as_bytes();

    let prod = match proj
        .site
        .updated_with(&style_file.as_site_file(), bytes)
        .await?
    {
        true => {
            log::trace!(
                "Style finished with changes {}",
                GRAY.paint(&style_file.to_string())
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
