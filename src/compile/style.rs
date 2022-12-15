use super::ChangeSet;
use crate::{
    config::Config,
    ext::anyhow::{anyhow, bail, Context, Result},
    ext::exe::{get_exe, Exe},
    fs,
    service::site,
    signal::{Outcome, Product},
};
use camino::{Utf8Path, Utf8PathBuf};
use lightningcss::{
    stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet},
    targets::Browsers,
};
use tokio::process::Command;
use tokio::task::JoinHandle;

pub async fn style(conf: &Config, changes: &ChangeSet) -> JoinHandle<Result<Outcome>> {
    let changes = changes.clone();
    let conf = conf.clone();

    tokio::spawn(async move {
        if !changes.need_style_build(true, false) {
            log::debug!("Style no build needed {changes:?}");
            return Ok(Outcome::Success(Product::NoChange));
        }
        Ok(Outcome::Success(build(&conf).await?))
    })
}

async fn build(conf: &Config) -> Result<Product> {
    let style_file = match &conf.leptos.style_file {
        Some(f) => f,
        None => {
            log::trace!("Style no style_file configured");
            return Ok(Product::NoChange);
        }
    };
    fs::create_dir_all(conf.pkg_dir().to_absolute().await)
        .await
        .dot()?;

    log::debug!("Style found: {style_file}");
    let file = Utf8PathBuf::from(style_file);

    match file.extension() {
        Some("sass") | Some("scss") => compile_sass(conf, style_file, conf.cli.release)
            .await
            .context(format!("compile sass/scss: {style_file}"))?,
        Some("css") => {
            fs::copy(style_file, &conf.site_css_file()).await.dot()?;
        }
        _ => bail!("Not a css/sass/scss style file: {style_file}"),
    };

    process_css(&conf)
        .await
        .context(format!("process css {style_file}"))
}

async fn compile_sass(conf: &Config, style_file: &Utf8Path, release: bool) -> Result<()> {
    let dest = conf.site_css_file().to_absolute().await;
    let mut args = vec![style_file.as_str(), dest.as_str()];
    release.then(|| args.push("--no-source-map"));

    let exe = get_exe(Exe::Sass)
        .await
        .context("Try manually installing sass: https://sass-lang.com/install")?;

    let mut cmd = Command::new(exe).args(&args).spawn()?;

    cmd.wait()
        .await
        .context(format!("sass {}", args.join(" ")))?;

    log::trace!("Style compiled sass to {dest:?}");
    Ok(())
}

fn browser_lists(query: &str) -> Result<Option<Browsers>> {
    Browsers::from_browserslist([query]).context(format!("Error in browserlist query: {query}"))
}

async fn process_css(conf: &Config) -> Result<Product> {
    let browsers = browser_lists(&conf.leptos.browserquery).context("leptos.style.browserquery")?;

    let file = conf.site_css_file().to_absolute().await;
    let css = fs::read_to_string(&file).await?;

    let mut style =
        StyleSheet::parse(&css, ParserOptions::default()).map_err(|e| anyhow!("{e}"))?;

    if conf.cli.release {
        style.minify(MinifyOptions::default())?;
    }

    let mut options = PrinterOptions::default();
    options.targets = browsers;

    if conf.cli.release {
        options.minify = true;
    }
    let style_output = style.to_css(options)?;

    let bytes = style_output.code.as_bytes();

    let prod = match site::write_if_changed(&conf.site_css_file(), bytes).await? {
        true => {
            log::trace!("Style finished with changes");
            Product::Style
        }
        false => {
            log::trace!("Style finished without changes");
            Product::NoChange
        }
    };
    Ok(prod)
}
