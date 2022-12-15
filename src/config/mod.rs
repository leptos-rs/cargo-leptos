mod leptos;
mod paths;

use crate::ext::anyhow::{anyhow, ensure, Context, Result};
use crate::{ext::fs, logger::GRAY, Cli, Commands, Opts};
use cargo_metadata::{Metadata, MetadataCommand, Package as CargoPackage};
use leptos::LeptosConfig;

#[derive(Debug, Clone)]
pub struct Config {
    /// options provided on the cli
    pub cli: Opts,
    /// The full workspace metadata read from Cargo.toml
    pub workspace: Metadata,
    /// The root cargo package
    pub cargo: CargoPackage,
    /// options from the Cargo.toml metadata.leptos section.
    pub leptos: LeptosConfig,
    pub watch: bool,
}

/// read from path or default to 'leptos.toml'
pub async fn read(cli: &Cli, opts: Opts) -> Result<Config> {
    if let Ok(path) = dotenvy::dotenv() {
        log::debug!(
            "Conf using .env file {}",
            GRAY.paint(path.to_string_lossy())
        );
    } else {
        log::debug!("Conf no .env file found");
    }

    let watch = matches!(cli.command, Commands::Watch(_));
    let workspace = MetadataCommand::new().manifest_path("Cargo.toml").exec()?;

    let cargo = workspace
        .root_package()
        .ok_or_else(|| anyhow!("Could not find root package in Cargo.toml"))?
        .clone();

    let package_name = match cargo
        .targets
        .iter()
        .find(|t| t.kind.iter().any(|k| k == "cdylib"))
    {
        Some(lib) => lib.name.replace('-', "_"),
        None => cargo.name.replace('-', "_"),
    };

    let mut leptos = LeptosConfig::load(&package_name)
        .await
        .context("read config: Cargo.toml")?;

    if let Some(style) = &leptos.style_file {
        ensure!(style.exists(), "no css/sass/scss file found at: {style:?}",);
        ensure!(style.is_file(), "expected a file, not a dir: {style:?}",);
    }

    if !leptos.site_root.exists() {
        fs::create_dir_all(&leptos.site_root).await?;
    }
    if !leptos.site_root.is_absolute() {
        leptos.site_root = leptos.site_root.canonicalize_utf8()?;
    }

    ensure!(
        leptos.site_pkg_dir.is_relative(),
        "The site pkg directory has to be relative to the site dir ({}) not: {}",
        leptos.site_pkg_dir,
        leptos.site_root
    );

    Ok(Config {
        cli: opts,
        workspace,
        cargo,
        leptos,
        watch,
    })
}

impl Config {
    /// env vars to use when running external command
    pub fn to_envs(&self) -> Vec<(&'static str, String)> {
        let mut vec = vec![
            ("PACKAGE_NAME", self.leptos.package_name.to_string()),
            ("LEPTOS_SITE_ROOT", self.leptos.site_root.to_string()),
            ("LEPTOS_SITE_PKG_DIR", self.leptos.site_pkg_dir.to_string()),
            ("LEPTOS_SITE_ADDR", self.leptos.site_addr.to_string()),
            ("LEPTOS_RELOAD_PORT", self.leptos.reload_port.to_string()),
        ];
        if self.watch {
            vec.push(("LEPTOS_WATCH", "ON".to_string()))
        }
        vec
    }
}
