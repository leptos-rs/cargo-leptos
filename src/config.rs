use crate::{Error, Reportable};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub leptos: Leptos,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Leptos {
    pub app_path: String,
    pub client_path: String,
    pub server_path: String,
    pub index_path: String,
    /// path to generated rust code
    pub gen_path: String,
    pub style: Style,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Style {
    pub files: Vec<String>,
    pub browserquery: String,
}

impl Style {
    pub fn scss_files(&self) -> &[String] {
        &self.files
    }
}

impl Config {
    /// read from path or default to 'leptos.toml'
    pub fn read(path: &Option<String>) -> Result<Self, Reportable> {
        let path = path.as_deref().unwrap_or("leptos.toml");
        Config::try_read(path).map_err(|e| e.file_context("read config", path))
    }

    fn try_read(path: &str) -> Result<Self, Error> {
        log::debug!("Reading config file {path}");
        let toml = fs::read_to_string(path)?;
        log::trace!("Config file content:\n{toml}");
        Ok(toml::from_str(&toml)?)
    }

    pub fn save_default_file() -> Result<(), Reportable> {
        Self::try_save_default().map_err(|e| e.file_context("save default", "leptos.toml"))
    }

    fn try_save_default() -> Result<(), Error> {
        log::debug!("Adding default leptos.toml file");
        let toml = include_str!("leptos.toml");
        log::trace!("Content of leptos.toml:\n{toml}");
        Ok(std::fs::write("leptos.toml", toml.as_bytes())?)
    }
}
