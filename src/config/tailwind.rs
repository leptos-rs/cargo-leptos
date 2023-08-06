use camino::Utf8PathBuf;

use super::ProjectConfig;
use anyhow::{bail, Result};

#[derive(Clone, Debug)]
pub struct TailwindConfig {
    pub input_file: Utf8PathBuf,
    pub config_file: Utf8PathBuf,
}

impl TailwindConfig {
    pub fn new(conf: &ProjectConfig) -> Result<Option<Self>> {
        let input_file = if let Some(input_file) = conf.tailwind_input_file.clone() {
            conf.config_dir.join(input_file)
        } else {
            if conf.tailwind_config_file.is_some() {
                bail!("The Cargo.toml `tailwind-input-file` is required when using `tailwind-config-file`]");
            }
            return Ok(None);
        };

        let config_file = conf.config_dir.join(
            conf.tailwind_config_file
                .clone()
                .unwrap_or_else(|| Utf8PathBuf::from("tailwind.config.js")),
        );

        Ok(Some(Self {
            input_file,
            config_file,
        }))
    }
}
