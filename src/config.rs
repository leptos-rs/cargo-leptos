use crate::{util, Cli, Error, Reportable};
use serde::Deserialize;
use std::fs;

#[derive(Deserialize, Debug)]
struct ConfigFile {
    pub leptos: Config,
}

/// read from path or default to 'leptos.toml'
pub fn read(cli: &Cli) -> Result<Config, Reportable> {
    let mut conf = read_config("leptos.toml")
        .map_err(|e| e.file_context("read config", "leptos.toml"))?
        .leptos;
    conf.release = cli.release;
    conf.csr = cli.csr;
    conf.index_path = format!("{}/{}", conf.root, conf.index_path);
    conf.gen_path = format!("{}/{}", conf.root, conf.gen_path);
    conf.style.file = format!("{}/{}", conf.root, conf.style.file);
    Ok(conf)
}

fn read_config(file: &str) -> Result<ConfigFile, Error> {
    let text = fs::read_to_string(file)?;
    log::trace!("Config file content:\n{text}");
    Ok(toml::from_str(&text)?)
}

pub fn save_default_file() -> Result<(), Reportable> {
    log::debug!("Adding default leptos.toml file");
    util::write("leptos.toml", include_str!("leptos.toml"))
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub root: String,
    pub index_path: String,
    /// where to generate rust code
    pub gen_path: String,
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
