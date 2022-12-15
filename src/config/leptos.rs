use crate::ext::anyhow::{bail, Result};
use crate::service::site::SiteFile;
use crate::{ext::fs, logger::GRAY};
use camino::Utf8PathBuf;
use regex::Regex;
use serde::Deserialize;
use std::fmt::Display;
use std::net::SocketAddr;

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
    pub async fn load(package_name: &str) -> Result<Self> {
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
