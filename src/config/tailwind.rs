use camino::Utf8PathBuf;

use super::ProjectConfig;
use anyhow::{bail, Result};

#[derive(Debug)]
pub struct TailwindConfig {
    pub input_file: Utf8PathBuf,
    pub output_file: Utf8PathBuf,
    pub config_file: Utf8PathBuf,
}

impl TailwindConfig {
    pub fn new(conf: &ProjectConfig) -> Result<Option<Self>> {
        let Some(input_file) = conf.tailwind_input_file.clone() else {
            if conf.tailwind_output_file.is_some() || conf.tailwind_config_file.is_some() {
                bail!("The Cargo.toml `tailwind-input-file` is required when using `tailwind-output-file` or `tailwind-config-file`]");
            }
            return Ok(None);
        };
        let output_file = conf
            .tailwind_output_file
            .clone()
            .unwrap_or_else(|| Utf8PathBuf::from("style/tailwind.css"));

        if !input_file.exists() {
            bail!("The Cargo.toml `tailwind-input-file` does not exist: {input_file}");
        }
        let config_file = conf
            .tailwind_config_file
            .clone()
            .unwrap_or_else(|| Utf8PathBuf::from("./target/tailwind.config.js"));
        Ok(Some(Self {
            input_file,
            output_file,
            config_file,
        }))
    }
}
