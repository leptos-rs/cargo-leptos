use camino::Utf8PathBuf;

use super::{ProjectConfig, VersionConfig};
use crate::{ext::exe::sanitize_version_prefix, internal_prelude::*};

#[derive(Clone, Debug)]
pub struct TailwindConfig {
    pub input_file: Utf8PathBuf,
    pub config_file: Option<Utf8PathBuf>,
    pub tmp_file: Utf8PathBuf,
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

        let version = VersionConfig::Tailwind.version();
        let sanitized_version =
            sanitize_version_prefix(version.as_ref()).unwrap_or(version.as_ref());
        let is_v4 = sanitized_version.starts_with("4.") || sanitized_version == "4";

        let config_file = if is_v4 {
            if conf.tailwind_config_file.is_some()
                || conf.config_dir.join("tailwind.config.js").exists()
            {
                info!("JavaScript config files are no longer required in Tailwind CSS v4. If you still need to use a JS config file, refer to the docs here: https://tailwindcss.com/docs/upgrade-guide#using-a-javascript-config-file.");
            }

            conf.tailwind_config_file.clone()
        } else {
            Some(
                conf.config_dir.join(
                    conf.tailwind_config_file
                        .clone()
                        .unwrap_or_else(|| Utf8PathBuf::from("tailwind.config.js")),
                ),
            )
        };

        let tmp_file = conf.tmp_dir.join("tailwind.css");

        Ok(Some(Self {
            input_file,
            config_file,
            tmp_file,
        }))
    }
}
