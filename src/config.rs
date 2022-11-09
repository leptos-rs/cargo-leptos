use crate::{Error, Reportable};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub leptos: Leptos,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Leptos {
    app_path: Option<String>,
    client_path: Option<String>,
    server_path: Option<String>,
    style: Style,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Style {
    pub files: Option<Vec<String>>,
    pub browserquery: Option<String>,
}

impl Style {
    pub fn scss_files(&self) -> Vec<String> {
        const STYLE_DEFAULT: &str = "app/style/main.scss";
        if let Some(styles) = &self.files {
            log::debug!("Styles in config: {:?}", &styles);
            styles.clone()
        } else if let Some(file) = existing_file(STYLE_DEFAULT) {
            log::info!("Using default style: {STYLE_DEFAULT}");
            vec![file]
        } else {
            log::warn!("No styles configured and none found in default dir: '{STYLE_DEFAULT}'");
            Vec::new()
        }
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

    pub fn projects(&self) -> Projects {
        Projects {
            app: param_or_dir(&self.leptos.app_path, "app"),
            client: param_or_dir(&self.leptos.client_path, "client"),
            server: param_or_dir(&self.leptos.client_path, "server"),
        }
    }

    pub fn style(&self) -> Style {
        self.leptos.style.clone()
    }

    pub fn save_default_file() -> Result<(), Reportable> {
        Self::try_save_default().map_err(|e| e.file_context("", "leptos.toml"))
    }

    fn try_save_default() -> Result<(), Error> {
        log::debug!("Adding default leptos.toml file");
        let toml = include_str!("leptos.toml");
        log::trace!("Content of leptos.toml:\n{toml}");
        Ok(std::fs::write("leptos.toml", toml.as_bytes())?)
    }
}

#[derive(Debug, Default)]
pub struct Projects {
    pub app: Option<String>,
    pub client: Option<String>,
    pub server: Option<String>,
}

fn param_or_dir(param: &Option<String>, folder: &str) -> Option<String> {
    if let Some(path) = param {
        Some(path.to_string())
    } else {
        existing_dir(folder)
    }
}

fn existing_dir(dir: &str) -> Option<String> {
    let path = PathBuf::from(dir);
    if path.exists() && path.is_dir() {
        Some(dir.to_string())
    } else {
        None
    }
}

fn existing_file(file: &str) -> Option<String> {
    let path = PathBuf::from(file);
    if path.exists() && path.is_file() {
        Some(file.to_string())
    } else {
        None
    }
}
