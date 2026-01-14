use crate::{
    config::{LightningCssConfig, Project},
    ext::Paint,
    internal_prelude::*,
    logger::GRAY,
    signal::Outcome,
};
use lightningcss::{
    bundler::{Bundler, FileProvider, SourceProvider},
    stylesheet::{MinifyOptions, ParserOptions, PrinterOptions},
    targets::{Browsers, Targets},
};
use std::path::Path;
use std::sync::Arc;

pub async fn compile_lightningcss(
    proj: &Arc<Project>,
    lightningcss_conf: &LightningCssConfig,
) -> Result<Outcome<String>> {
    let input_file = &lightningcss_conf.input_file;

    if !input_file.exists() {
        error!("LightningCSS input file does not exist: {}", input_file);
        bail!("LightningCSS input file does not exist: {}", input_file);
    }

    info!(
        "Style generating CSS with LightningCSS {}",
        GRAY.paint(input_file.to_string())
    );

    let start_time = tokio::time::Instant::now();

    let browsers = browser_lists(&proj.style.browserquery).wrap_err("leptos.style.browserquery")?;
    let targets = Targets::from(browsers);

    let css = match bundle_and_process(
        input_file.as_std_path(),
        input_file.as_ref(),
        targets,
        proj.release,
    ) {
        Ok(css) => {
            if css.is_empty() {
                error!(
                    "LightningCSS produced empty CSS from {}. \
                    Check that the file contains valid CSS and @imports resolve correctly.",
                    input_file
                );
                bail!(
                    "LightningCSS produced empty CSS output from {}",
                    input_file
                );
            }
            css
        }
        Err(e) => {
            error!("LightningCSS bundle_and_process failed: {e:?}");
            return Err(e);
        }
    };

    let end_time = tokio::time::Instant::now();

    info!(
        "Finished generating CSS with LightningCSS in {:?} ({} bytes)",
        end_time - start_time,
        css.len()
    );

    Ok(Outcome::Success(css))
}

/// A wrapper around FileProvider that logs file reads for tracing @import resolution
struct TracingFileProvider {
    inner: FileProvider,
}

impl TracingFileProvider {
    fn new() -> Self {
        Self {
            inner: FileProvider::new(),
        }
    }
}

impl SourceProvider for TracingFileProvider {
    type Error = std::io::Error;

    fn read<'a>(&'a self, path: &Path) -> Result<&'a str, Self::Error> {
        trace!(
            "LightningCSS reading: {}",
            GRAY.paint(path.display().to_string())
        );
        match self.inner.read(path) {
            Ok(content) => {
                trace!(
                    "LightningCSS read {} bytes from {}",
                    content.len(),
                    path.display()
                );
                Ok(content)
            }
            Err(e) => {
                error!(
                    "LightningCSS failed to read '{}': {} (does the file exist?)",
                    path.display(),
                    e
                );
                Err(e)
            }
        }
    }

    fn resolve(
        &self,
        specifier: &str,
        originating_file: &Path,
    ) -> Result<std::path::PathBuf, Self::Error> {
        match self.inner.resolve(specifier, originating_file) {
            Ok(resolved) => {
                trace!(
                    "LightningCSS @import \"{}\" -> {}",
                    specifier,
                    GRAY.paint(resolved.display().to_string())
                );
                Ok(resolved)
            }
            Err(e) => {
                error!(
                    "LightningCSS failed to resolve @import \"{}\" from '{}': {}",
                    specifier,
                    originating_file.display(),
                    e
                );
                error!(
                    "Hint: Did you rename or delete '{}'? Update the @import statement in '{}'",
                    specifier,
                    originating_file.display()
                );
                Err(e)
            }
        }
    }
}

pub fn bundle_and_process(
    input_path: &Path,
    filename: &str,
    targets: Targets,
    minify: bool,
) -> Result<String> {
    debug!(
        "LightningCSS bundle_and_process: input={}, minify={}",
        input_path.display(),
        minify
    );

    let fs = TracingFileProvider::new();
    let mut bundler = Bundler::new(
        &fs,
        None,
        ParserOptions {
            filename: filename.to_string(),
            ..Default::default()
        },
    );

    let mut stylesheet = match bundler.bundle(input_path) {
        Ok(ss) => ss,
        Err(e) => {
            error!("LightningCSS bundler.bundle() failed: {e}");
            bail!("Failed to bundle CSS: {}", e);
        }
    };

    let source_count = stylesheet.sources.len();
    debug!(
        "LightningCSS bundled {} source file(s): {:?}",
        source_count, stylesheet.sources
    );

    // Only run minify optimization pass when producing minified output
    // The minify() call transforms the AST (merging rules, etc.) which can
    // sometimes produce unexpected results - skip it for debug builds
    if minify {
        if let Err(e) = stylesheet.minify(MinifyOptions {
            targets,
            ..Default::default()
        }) {
            error!("LightningCSS stylesheet.minify() failed: {e}");
            bail!("Failed to minify CSS: {}", e);
        }
    }

    let result = match stylesheet.to_css(PrinterOptions {
        targets,
        minify,
        ..Default::default()
    }) {
        Ok(r) => r,
        Err(e) => {
            error!("LightningCSS stylesheet.to_css() failed: {e}");
            bail!("Failed to generate CSS: {}", e);
        }
    };

    debug!(
        "LightningCSS output: {} bytes{}",
        result.code.len(),
        if minify { " (minified)" } else { "" }
    );

    if result.code.is_empty() {
        warn!("LightningCSS to_css() returned empty string");
    }

    Ok(result.code)
}

fn browser_lists(query: &str) -> Result<Option<Browsers>> {
    Browsers::from_browserslist([query]).wrap_err(format!("Error in browserlist query: {query}"))
}
