use super::ChangeSet;
use crate::internal_prelude::*;
use crate::{
    compile::{sass::compile_sass, tailwind::compile_tailwind},
    config::Project,
    ext::{Paint, PathBufExt},
    fs,
    logger::GRAY,
    signal::{Outcome, Product},
};
use lightningcss::{
    stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet},
    targets::Browsers,
    targets::Targets,
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
        let css_in_source = proj.style.tailwind.is_some();
        if !changes.need_style_build(true, css_in_source) {
            debug!("Style no build needed {changes:?}");
            return Ok(Outcome::Success(Product::None));
        }
        build(&proj).await
    })
}

fn build_sass(proj: &Arc<Project>) -> JoinHandle<Result<Outcome<String>>> {
    let proj = proj.clone();
    tokio::spawn(async move {
        let Some(style_file) = &proj.style.file else {
            trace!("Style not configured");
            return Ok(Outcome::Success("".to_string()));
        };

        debug!("Style found: {}", &style_file);
        fs::create_dir_all(style_file.dest.clone().without_last())
            .await
            .dot()?;
        match style_file.source.extension() {
            Some("sass") | Some("scss") => compile_sass(style_file, proj.release)
                .await
                .wrap_err(format!("compile sass/scss: {}", &style_file)),
            Some("css") => Ok(Outcome::Success(
                fs::read_to_string(&style_file.source).await.dot()?,
            )),
            _ => bail!("Not a css/sass/scss style file: {}", &style_file),
        }
    })
}

fn build_tailwind(proj: &Arc<Project>) -> JoinHandle<Result<Outcome<String>>> {
    let proj = proj.clone();
    tokio::spawn(async move {
        let Some(tw_conf) = proj.style.tailwind.as_ref() else {
            trace!("Tailwind not configured");
            return Ok(Outcome::Success("".to_string()));
        };
        trace!("Tailwind config: {:?}", &tw_conf);
        compile_tailwind(&proj, tw_conf).await
    })
}

async fn build(proj: &Arc<Project>) -> Result<Outcome<Product>> {
    let css_handle = build_sass(proj);
    let tw_handle = build_tailwind(proj);
    let css = css_handle.await??;
    let tw = tw_handle.await??;

    use Outcome::*;
    let css = match (css, tw) {
        (Stopped, _) | (_, Stopped) => return Ok(Stopped),
        (Failed, _) | (_, Failed) => return Ok(Failed),
        (Success(css), Success(tw)) => format!("{css}\n{tw}"),
    };
    Ok(Success(process_css(proj, css).await?))
}

fn browser_lists(query: &str) -> Result<Option<Browsers>> {
    Browsers::from_browserslist([query]).wrap_err(format!("Error in browserlist query: {query}"))
}

async fn process_css(proj: &Project, css: String) -> Result<Product> {
    let browsers = browser_lists(&proj.style.browserquery).wrap_err("leptos.style.browserquery")?;
    let targets = Targets::from(browsers);

    let filename: String = if let Some(tw) = proj.style.tailwind.clone() {
        tw.tmp_file.to_string()
    } else {
        proj.style.file.as_ref()
            .map(|f| f.source.to_string())
            .unwrap_or_default()
    };

    let parse_options = ParserOptions {
        filename,
        ..Default::default()
    };

    let css: String = match StyleSheet::parse(&css, parse_options) {
        Ok(mut stylesheet) => {
            if proj.release {
                let minify_options = MinifyOptions {
                    targets,
                    ..Default::default()
                };
                stylesheet.minify(minify_options)?;
            }
            let options = PrinterOptions::<'_> {
                targets,
                minify: proj.release,
                ..Default::default()
            };
            stylesheet.to_css(options)?.code
        }
        Err(e) => {
            trace!("StyleSheet::parse error, unable to minify, falling back to input css: {e}");
            css.clone()
        }
    };

    let bytes: &[u8] = css.as_bytes();

    let prod = match proj.site.updated_with(&proj.style.site_file, bytes).await? {
        true => {
            trace!(
                "Style finished with changes {}",
                GRAY.paint(proj.style.site_file.to_string())
            );
            Product::Style("".to_string()) //TODO
        }
        false => {
            trace!("Style finished without changes");
            Product::None
        }
    };
    Ok(prod)
}


