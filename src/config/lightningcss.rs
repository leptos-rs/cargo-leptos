use camino::Utf8PathBuf;

use super::ProjectConfig;
use crate::internal_prelude::*;

#[derive(Clone, Debug)]
pub struct LightningCssConfig {
    pub input_file: Utf8PathBuf,
    pub watch_dir: Utf8PathBuf,
}

impl LightningCssConfig {
    pub fn new(conf: &ProjectConfig) -> Result<Option<Self>> {
        let Some(input_file) = conf.lightningcss_input_file.clone() else {
            return Ok(None);
        };

        let input_file = conf.config_dir.join(input_file);

        let watch_dir = input_file
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| conf.config_dir.clone());

        Ok(Some(Self {
            input_file,
            watch_dir,
        }))
    }
}
