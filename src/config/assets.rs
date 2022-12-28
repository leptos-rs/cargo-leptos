use camino::Utf8PathBuf;

use crate::ext::PathBufExt;

use super::ProjectConfig;

pub struct AssetsConfig {
    pub dir: Utf8PathBuf,
}

impl AssetsConfig {
    pub fn resolve(config: &ProjectConfig) -> Option<Self> {
        let Some(assets_dir) = &config
            .assets_dir else {
                return None;
            };

        Some(Self {
            // relative to the configuration file
            dir: config.config_dir.join(assets_dir),
        })
    }
}

impl std::fmt::Debug for AssetsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssetsConfig")
            .field("dir", &self.dir.test_string())
            .finish()
    }
}
