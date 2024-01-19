use super::ChangeSet;
use crate::{
    compile::{sass::compile_sass, tailwind::compile_tailwind},
    config::Project,
    ext::{
        anyhow::{anyhow, bail, Context, Result},
        PathBufExt,
    },
    fs,
    logger::GRAY,
    signal::{Outcome, Product},
};
use camino::Utf8PathBuf;
use lightningcss::{
    bundler::{Bundler, FileProvider},
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
            log::debug!("Style no build needed {changes:?}");
            return Ok(Outcome::Success(Product::None));
        }
        build(&proj).await
    })
}

async fn build_styles(proj: Arc<Project>) -> Result<Outcome<String>> {
    let Some(style_file) = &proj.style.file else {
        log::trace!("Style not configured");
        return Ok(Outcome::Success("".to_string()));
    };

    log::trace!("Style found: {}", &style_file);
    fs::create_dir_all(style_file.dest.clone().without_last())
        .await
        .dot()?;
    match style_file.source.extension() {
        Some("sass") | Some("scss") => compile_sass(style_file, proj.release)
            .await
            .context(format!("Compile sass/scss: {}", &style_file)),
        Some("css") => {
            let abs_style_path = if style_file.source.is_absolute() {
                style_file.source.clone()
            } else {
                let mut abs_path = proj.lib.abs_dir.clone();
                abs_path.push(style_file.source.clone());
                abs_path
            };
            bundle_css(&abs_style_path)
        }
        _ => bail!("Not a css/sass/scss style file: {}", &style_file),
    }
}

fn bundle_css(abs_style_path: &Utf8PathBuf) -> Result<Outcome<String>> {
    let bundle_options = ParserOptions::default();
    let file_provider = FileProvider::new();

    let mut bundler = Bundler::new(&file_provider, None, bundle_options);
    let style_sheet = bundler
        .bundle(abs_style_path.as_std_path())
        .map_err(|e| anyhow!("Error bundling css: {:?}\n{:?}", &abs_style_path, e))?;
    // ^^^ Have to use map_err/anyhow because possible returned error captures bundler
    // internal state and then early exit via '?' requires FileProvider to be &'static.
    let css = style_sheet
        .to_css(PrinterOptions::default())
        .context(format!("Error rendering bundled css {:?}", &abs_style_path))?
        .code;

    Ok(Outcome::Success(css))
}

async fn build_tailwind(proj: Arc<Project>) -> Result<Outcome<String>> {
    let Some(tw_conf) = proj.style.tailwind.as_ref() else {
        log::trace!("Tailwind not configured");
        return Ok(Outcome::Success("".to_string()));
    };
    log::trace!("Tailwind config: {:?}", &tw_conf);
    compile_tailwind(&proj, tw_conf).await
}

async fn build(project: &Arc<Project>) -> Result<Outcome<Product>> {
    let proj = project.clone();
    let css_handle = tokio::spawn(async move { build_styles(proj) });
    let proj = project.clone();
    let tw_handle = tokio::spawn(async move { build_tailwind(proj) });
    let css = css_handle.await?.await?;
    let tw = tw_handle.await?.await?;

    use Outcome::*;
    let css = match (css, tw) {
        (Stopped, _) | (_, Stopped) => return Ok(Stopped),
        (Failed, _) | (_, Failed) => return Ok(Failed),
        (Success(css), Success(tw)) => format!("{css}\n{tw}"),
    };
    Ok(Success(process_css(project, css).await?))
}

fn browser_lists(query: &str) -> Result<Option<Browsers>> {
    Browsers::from_browserslist([query]).context(format!("Error in browserlist query: {query}"))
}

async fn process_css(proj: &Project, css: String) -> Result<Product> {
    let browsers = browser_lists(&proj.style.browserquery).context("leptos.style.browserquery")?;
    let targets = Targets::from(browsers);

    let mut stylesheet =
        StyleSheet::parse(&css, ParserOptions::default()).map_err(|e| anyhow!("{e}"))?;

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

    let style_output = stylesheet.to_css(options)?;

    let bytes = style_output.code.as_bytes();

    let prod = match proj.site.updated_with(&proj.style.site_file, bytes).await? {
        true => {
            log::trace!(
                "Style finished with changes {}",
                GRAY.paint(&proj.style.site_file.to_string())
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
