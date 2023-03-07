use super::ChangeSet;
use crate::{
    compile::{sass::compile_sass, tailwind::compile_tailwind},
    config::{Project, StyleConfig},
    ext::{
        anyhow::{anyhow, bail, Context, Result},
        PathBufExt,
    },
    fs,
    logger::GRAY,
    signal::{Outcome, Product},
};
use lightningcss::{
    stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet},
    targets::Browsers,
};
use std::sync::Arc;
use tokio::task::JoinHandle;

pub async fn style(
    proj: &Arc<Project>,
    changes: &ChangeSet,
) -> JoinHandle<Result<Outcome<Product>>> {
    let changes = changes.clone();
    let proj = proj.clone();

    tokio::spawn(async move {
        if !changes.need_style_build(true, false) {
            log::debug!("Style no build needed {changes:?}");
            return Ok(Outcome::Success(Product::None));
        }
        Ok(build(&proj).await?)
    })
}

async fn build(proj: &Arc<Project>) -> Result<Outcome<Product>> {
    let Some(style) = &proj.style else {
        log::trace!("Style not configured");
        return Ok(Outcome::Success(Product::None));
    };
    fs::create_dir_all(style.file.dest.clone().without_last())
        .await
        .dot()?;

    log::debug!("Style found: {}", &style.file);
    let css_handle = {
        let proj = proj.clone();
        let style = style.clone();
        tokio::spawn(async move {
            match style.file.source.extension() {
                Some("sass") | Some("scss") => compile_sass(&style.file, proj.release)
                    .await
                    .context(format!("compile sass/scss: {}", &style.file)),
                Some("css") => Ok(Outcome::Success(
                    fs::read_to_string(&style.file.source).await.dot()?,
                )),
                _ => bail!("Not a css/sass/scss style file: {}", &style.file),
            }
        })
    };

    let tw_handle = {
        let proj = proj.clone();
        tokio::spawn(async move {
            if let Some(tw_conf) = proj.lib.tailwind.as_ref() {
                compile_tailwind(&proj, tw_conf).await
            } else {
                Ok(Outcome::Success("".to_string()))
            }
        })
    };
    let css = css_handle.await??;
    let tw = tw_handle.await??;

    use Outcome::*;
    let css = match (css, tw) {
        (Stopped, _) | (_, Stopped) => return Ok(Stopped),
        (Failed, _) | (_, Failed) => return Ok(Failed),
        (Success(css), Success(tw)) => format!("{css}\n{tw}"),
    };
    Ok(Outcome::Success(
        process_css(&proj, &style, css)
            .await
            .context(format!("process css {}", &style.file))?,
    ))
}

fn browser_lists(query: &str) -> Result<Option<Browsers>> {
    Browsers::from_browserslist([query]).context(format!("Error in browserlist query: {query}"))
}

async fn process_css(proj: &Project, style: &StyleConfig, css: String) -> Result<Product> {
    let browsers = browser_lists(&style.browserquery).context("leptos.style.browserquery")?;

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
            Product::Style("".to_string()) //TODO
        }
        false => {
            log::trace!("Style finished without changes");
            Product::None
        }
    };
    Ok(prod)
}
