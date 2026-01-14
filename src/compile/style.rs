use super::ChangeSet;
use crate::internal_prelude::*;
use crate::{
    compile::{lightningcss::compile_lightningcss, sass::compile_sass, tailwind::compile_tailwind},
    config::Project,
    ext::{fs, Paint, PathBufExt},
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
        let has_lightningcss = proj.style.lightningcss.is_some();
        if !changes.need_style_build(true, css_in_source || has_lightningcss) {
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

fn build_lightningcss(proj: &Arc<Project>) -> JoinHandle<Result<Outcome<String>>> {
    let proj = proj.clone();
    tokio::spawn(async move {
        let Some(lcss_conf) = proj.style.lightningcss.as_ref() else {
            trace!("LightningCSS not configured");
            return Ok(Outcome::Success("".to_string()));
        };
        trace!("LightningCSS config: {:?}", &lcss_conf);
        compile_lightningcss(&proj, lcss_conf).await
    })
}

async fn build(proj: &Arc<Project>) -> Result<Outcome<Product>> {
    let css_handle = build_sass(proj);
    let tw_handle = build_tailwind(proj);
    let lcss_handle = build_lightningcss(proj);

    let css = match css_handle.await {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(e)) => {
            error!("Sass build failed: {e:?}");
            return Ok(Outcome::Failed);
        }
        Err(e) => {
            error!("Sass task panicked: {e:?}");
            return Ok(Outcome::Failed);
        }
    };

    let tw = match tw_handle.await {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(e)) => {
            error!("Tailwind build failed: {e:?}");
            return Ok(Outcome::Failed);
        }
        Err(e) => {
            error!("Tailwind task panicked: {e:?}");
            return Ok(Outcome::Failed);
        }
    };

    let lcss = match lcss_handle.await {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(e)) => {
            error!("LightningCSS build failed: {e:?}");
            return Ok(Outcome::Failed);
        }
        Err(e) => {
            error!("LightningCSS task panicked: {e:?}");
            return Ok(Outcome::Failed);
        }
    };

    use Outcome::*;

    // Log the outcome of each style processor with actual content info
    debug!(
        "Style build outcomes - sass: {}, tailwind: {}, lightningcss: {}",
        outcome_summary(&css),
        outcome_summary(&tw),
        outcome_summary(&lcss)
    );

    let css = match (css, tw, lcss) {
        (Stopped, _, _) | (_, Stopped, _) | (_, _, Stopped) => {
            debug!("Style build stopped");
            return Ok(Stopped);
        }
        (Failed, _, _) | (_, Failed, _) | (_, _, Failed) => {
            warn!("Style build failed - NOT writing CSS to preserve previous good version");
            return Ok(Failed);
        }
        (Success(css), Success(tw), Success(lcss)) => {
            // Log each component's content with repr for debugging
            debug!(
                "Style CSS lengths - sass: {} bytes, tailwind: {} bytes, lightningcss: {} bytes",
                css.len(),
                tw.len(),
                lcss.len()
            );

            // Log if any component looks suspicious
            for (name, content) in [("sass", &css), ("tailwind", &tw), ("lightningcss", &lcss)] {
                if !content.is_empty() && content.trim().is_empty() {
                    warn!(
                        "Style {} returned whitespace-only content: {:?} ({} bytes)",
                        name,
                        content,
                        content.len()
                    );
                }
            }

            let combined = [css, tw, lcss]
                .into_iter()
                .filter(|s| !s.is_empty() && !s.trim().is_empty()) // Also filter whitespace-only
                .collect::<Vec<_>>()
                .join("\n");
            debug!("Style combined CSS: {} bytes", combined.len());
            combined
        }
    };

    if css.is_empty() {
        warn!("Style build produced empty CSS output");
    }

    // Skip post-processing if only lightningcss is configured (it already handles everything)
    let only_lightningcss = proj.style.file.is_none()
        && proj.style.tailwind.is_none()
        && proj.style.lightningcss.is_some();

    if only_lightningcss {
        debug!("Style using direct write (lightningcss only)");
        Ok(Success(write_css(proj, css).await?))
    } else {
        debug!("Style using process_css for post-processing");
        Ok(Success(process_css(proj, css).await?))
    }
}

fn outcome_summary<T>(outcome: &Outcome<T>) -> &'static str {
    match outcome {
        Outcome::Success(_) => "success",
        Outcome::Stopped => "stopped",
        Outcome::Failed => "failed",
    }
}

fn browser_lists(query: &str) -> Result<Option<Browsers>> {
    Browsers::from_browserslist([query]).wrap_err(format!("Error in browserlist query: {query}"))
}

async fn write_css(proj: &Project, css: String) -> Result<Product> {
    debug!(
        "Style write_css: {} bytes to {}",
        css.len(),
        proj.style.site_file
    );

    // Guard against writing empty or whitespace-only CSS
    let trimmed_len = css.trim().len();
    if trimmed_len == 0 {
        warn!(
            "Style write_css: refusing to write empty/whitespace-only CSS ({} bytes, {} after trim)",
            css.len(),
            trimmed_len
        );
        return Ok(Product::None);
    }

    if css.len() < 100 {
        warn!(
            "Style write_css: suspiciously small CSS ({} bytes): {:?}",
            css.len(),
            css
        );
    }

    let bytes: &[u8] = css.as_bytes();

    let prod = match proj.site.updated_with(&proj.style.site_file, bytes).await? {
        true => {
            info!(
                "Style finished with changes {}",
                GRAY.paint(proj.style.site_file.to_string())
            );
            Product::Style("".to_string())
        }
        false => {
            debug!("Style finished without changes");
            Product::None
        }
    };
    Ok(prod)
}

async fn process_css(proj: &Project, css: String) -> Result<Product> {
    debug!("Style process_css input: {} bytes", css.len());

    let browsers = browser_lists(&proj.style.browserquery).wrap_err("leptos.style.browserquery")?;
    let targets = Targets::from(browsers);

    let filename: String = if let Some(tw) = proj.style.tailwind.clone() {
        tw.tmp_file.to_string()
    } else if proj.style.lightningcss.is_some() {
        "lightningcss-bundle.css".to_string()
    } else {
        proj.style
            .file
            .as_ref()
            .map(|f| f.source.to_string())
            .unwrap_or_default()
    };

    let parse_options = ParserOptions {
        filename: filename.clone(),
        ..Default::default()
    };

    let css: String = match StyleSheet::parse(&css, parse_options) {
        Ok(mut stylesheet) => {
            if let Err(e) = stylesheet.minify(MinifyOptions {
                targets,
                ..Default::default()
            }) {
                error!("Style minify failed: {e}");
                return Err(eyre!("Style minify failed: {e}"));
            }

            let options = PrinterOptions::<'_> {
                targets,
                minify: proj.release,
                ..Default::default()
            };
            match stylesheet.to_css(options) {
                Ok(result) => {
                    debug!("Style process_css output: {} bytes", result.code.len());
                    result.code
                }
                Err(e) => {
                    error!("Style to_css failed: {e}");
                    return Err(eyre!("Style to_css failed: {e}"));
                }
            }
        }
        Err(e) => {
            warn!(
                "StyleSheet::parse error for '{}', falling back to input css: {e}",
                filename
            );
            css.clone()
        }
    };

    if css.is_empty() {
        warn!("Style process_css produced empty CSS");
    }

    write_css(proj, css).await
}
