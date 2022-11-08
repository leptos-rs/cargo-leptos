use std::path::PathBuf;

use crate::Config;

#[derive(Debug, Default)]
pub struct Projects {
    pub app: Option<String>,
    pub client: Option<String>,
    pub server: Option<String>,
}

impl From<Config> for Projects {
    fn from(conf: Config) -> Self {
        let app = param_or_folder(&conf.leptos.app_path, "./app");
        let client = param_or_folder(&conf.leptos.client_path, "./client");
        let server = param_or_folder(&conf.leptos.client_path, "./server");
        Self {
            app,
            client,
            server,
        }
    }
}

fn param_or_folder(param: &Option<String>, folder_name: &str) -> Option<String> {
    if let Some(path) = param {
        Some(path.to_string())
    } else {
        let path = PathBuf::from(folder_name);
        if path.exists() && path.is_dir() {
            Some(folder_name.to_string())
        } else {
            None
        }
    }
}
