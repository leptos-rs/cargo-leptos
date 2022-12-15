use crate::ext::anyhow::{anyhow, bail, ensure, Context, Result};
use crate::service::site::SiteFile;
use crate::{ext::fs, logger::GRAY, Cli, Commands, Opts};
use camino::Utf8PathBuf;
use cargo_metadata::{Metadata, MetadataCommand, Package as CargoPackage};
use regex::Regex;
use serde::Deserialize;
use std::fmt::Display;
use std::net::SocketAddr;

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

    let watch = match cli.command {
        Commands::Watch(_) => true,
        _ => false,
    };
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
        Some(lib) => lib.name.replace("-", "_"),
        None => cargo.name.replace("-", "_"),
    };

    let mut leptos = LeptosConfig::load(&package_name)
        .await
        .context(format!("read config: Cargo.toml"))?;

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
#[derive(Clone, Deserialize, Debug)]
pub struct LeptosConfig {
    pub package_name: String,
    pub site_addr: SocketAddr,
    pub site_root: Utf8PathBuf,
    pub site_pkg_dir: SiteFile,
    pub style_file: Option<Utf8PathBuf>,
    /// assets dir. content will be copied to the target/site dir
    pub assets_dir: Option<Utf8PathBuf>,
    pub reload_port: u16,
    /// command for launching end-2-end integration tests
    pub end2end_cmd: Option<String>,
    pub browserquery: String,
}

impl LeptosConfig {
    async fn load(package_name: &str) -> Result<Self> {
        let text = fs::read_to_string("Cargo.toml").await?;
        let re: Regex = Regex::new(r#"(?m)^\[package.metadata.leptos\]"#).unwrap();
        let start = match re.find(&text) {
            Some(found) => found.start(),
            None => {
                bail!(
                    "Missing Cargo.toml configuration section {}.\n\
                Append the output of {} to your Cargo.toml",
                    GRAY.paint("[package.metadata.leptos]"),
                    GRAY.paint("cargo leptos config")
                )
            }
        };
        log::trace!("Config file content:\n{text}");

        // so that serde error messages have right line number
        let newlines = text[..start].matches('\n').count();
        let toml = "\n".repeat(newlines) + &text[start..];

        let mut conf = toml::from_str::<ConfigFile>(&toml)?.package.metadata.leptos;
        let mut env: EnvVars = envy::from_env()?;
        let package_name = match env.package_name {
            Some(p) => {
                log::debug!(
                    "Conf package_name = {p} {}",
                    GRAY.paint("from env PACKAGE_NAME")
                );
                p.to_string()
            }
            None => {
                log::debug!(
                    "Conf package_name = {package_name} {}",
                    GRAY.paint("from Cargo.toml package.name")
                );
                package_name.to_string()
            }
        };
        let site_addr = env_conf_def(
            "site_addr",
            &mut env.leptos_site_addr,
            &mut conf.site_addr,
            SocketAddr::new([127, 0, 0, 1].into(), 3000),
        );
        let site_root = env_conf_def(
            "site_root",
            &mut env.leptos_site_root,
            &mut conf.site_root,
            Utf8PathBuf::from("target/site"),
        );
        let site_pkg_dir = env_conf_def(
            "site_pkg_dir",
            &mut env.leptos_site_pkg_dir,
            &mut conf.site_pkg_dir,
            SiteFile::from(Utf8PathBuf::from("pkg")),
        );
        let style_file = env_conf(
            "style_file",
            &mut env.leptos_style_file,
            &mut conf.style_file,
        );
        let assets_dir = env_conf(
            "assets_dir",
            &mut env.leptos_assets_dir,
            &mut conf.assets_dir,
        );
        let reload_port = env_conf_def(
            "reload_port",
            &mut env.leptos_reload_port,
            &mut conf.reload_port,
            3000,
        );
        let end2end_cmd = env_conf(
            "end2end_cmd",
            &mut env.leptos_end2end_cmd,
            &mut conf.end2end_cmd,
        );
        let browserquery = env_conf_def(
            "browserquery",
            &mut env.leptos_browserquery,
            &mut conf.browserquery,
            "defaults".to_string(),
        );
        Ok(Self {
            package_name,
            site_addr,
            site_root,
            site_pkg_dir,
            style_file,
            assets_dir,
            reload_port,
            end2end_cmd,
            browserquery,
        })
    }
}

fn env_conf_def<T>(name: &str, env: &mut Option<T>, conf: &mut Option<T>, def: T) -> T
where
    T: Display,
{
    if let Some(env) = env.take() {
        log::debug!(
            "Conf {name} = {env} {}",
            GRAY.paint(format!("from env LEPTOS_{}", name.to_uppercase()))
        );
        env
    } else if let Some(conf) = conf.take() {
        log::debug!("Conf {name} = {conf} {}", GRAY.paint("from Cargo.toml"));
        conf
    } else {
        log::debug!("Conf {name} = {def} {}", GRAY.paint("default"));
        def
    }
}

fn env_conf<T>(name: &str, env: &mut Option<T>, conf: &mut Option<T>) -> Option<T>
where
    T: Display,
{
    if let Some(env) = env.take() {
        log::debug!(
            "Conf {name} = {env} {}",
            GRAY.paint(format!("from env LEPTOS_{}", name.to_uppercase()))
        );
        Some(env)
    } else if let Some(conf) = conf.take() {
        log::debug!("Conf {name} = {conf} {}", GRAY.paint("from Cargo.toml"));
        Some(conf)
    } else {
        log::debug!("Conf {name} = {}", GRAY.paint("not set"));
        None
    }
}

#[derive(Deserialize, Debug)]
struct ConfigFile {
    pub package: Package,
}

#[derive(Deserialize, Debug)]
struct Package {
    metadata: MetadataSection,
}

#[derive(Deserialize, Debug)]
struct MetadataSection {
    leptos: LeptosManifest,
}

#[derive(Deserialize, Debug, Clone)]
struct LeptosManifest {
    pub site_addr: Option<SocketAddr>,
    pub site_root: Option<Utf8PathBuf>,
    pub site_pkg_dir: Option<SiteFile>,
    pub style_file: Option<Utf8PathBuf>,
    pub assets_dir: Option<Utf8PathBuf>,
    pub reload_port: Option<u16>,
    pub end2end_cmd: Option<String>,
    pub browserquery: Option<String>,
}

#[derive(Deserialize, Debug)]
struct EnvVars {
    pub package_name: Option<String>,
    pub leptos_site_addr: Option<SocketAddr>,
    pub leptos_site_root: Option<Utf8PathBuf>,
    pub leptos_site_pkg_dir: Option<SiteFile>,
    pub leptos_style_file: Option<Utf8PathBuf>,
    pub leptos_assets_dir: Option<Utf8PathBuf>,
    pub leptos_reload_port: Option<u16>,
    pub leptos_end2end_cmd: Option<String>,
    pub leptos_browserquery: Option<String>,
}
