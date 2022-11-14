use crate::{util, Cli};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

#[derive(Deserialize, Debug)]
struct ConfigFile {
    pub leptos: Config,
}

/// read from path or default to 'leptos.toml'
pub fn read(cli: &Cli) -> Result<Config> {
    let mut conf = read_config("leptos.toml")
        .context("read config: leptos.toml")?
        .leptos;
    conf.release = cli.release;
    conf.csr = cli.csr;
    conf.index_path = format!("{}/{}", conf.root, conf.index_path);
    conf.gen_path = format!("{}/{}", conf.root, conf.gen_path);
    conf.style.file = format!("{}/{}", conf.root, conf.style.file);
    Ok(conf)
}

fn read_config(file: &str) -> Result<ConfigFile> {
    let text = fs::read_to_string(file)?;
    log::trace!("Config file content:\n{text}");
    Ok(toml::from_str(&text)?)
}

pub fn save_default_file() -> Result<()> {
    log::debug!("Adding default leptos.toml file");
    util::write("leptos.toml", include_str!("leptos.toml"))
}

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    pub root: String,
    pub index_path: String,
    /// where to generate rust code
    pub gen_path: String,
    /// on which port to serve the client side rendered site
    pub csr_port: Option<u16>,
    pub style: Style,

    // parameters from cmd-line args
    #[serde(skip_deserializing)]
    pub release: bool,
    #[serde(skip_deserializing)]
    pub csr: bool,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Style {
    pub file: String,
    pub browserquery: String,
}
