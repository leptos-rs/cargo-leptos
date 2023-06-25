use camino::Utf8PathBuf;

use crate::ext::PathBufExt;

use super::ProjectConfig;

pub struct End2EndConfig {
    pub cmd: String,
    pub dir: Utf8PathBuf,
}

impl End2EndConfig {
    pub fn resolve(config: &ProjectConfig) -> Option<Self> {
        let Some(cmd) = &config.end2end_cmd else {
          return None
        };

        let dir = config.end2end_dir.to_owned().unwrap_or_default();

        Some(Self {
            cmd: cmd.clone(),
            dir,
        })
    }
}

impl std::fmt::Debug for End2EndConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("")
            .field("cmd", &self.cmd)
            .field("dir", &self.dir.test_string())
            .finish()
    }
}
