use std::{collections::HashMap, fmt::Display, path::Path};

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{
    config::SITE_ROOT,
    ext::{
        anyhow::{Context, Result},
        fs,
    },
};

lazy_static::lazy_static! {
    static ref FILE_REG: RwLock<HashMap<String, u64>> = RwLock::new(HashMap::new());
    static ref EXT_FILE_REG: RwLock<HashMap<String, u64>> = RwLock::new(HashMap::new());
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SiteFile(Utf8PathBuf);

impl SiteFile {
    pub fn to_relative(&self) -> Utf8PathBuf {
        SITE_ROOT.get().unwrap().join(&self.0)
    }
}

impl AsRef<Path> for SiteFile {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Utf8Path> for SiteFile {
    fn as_ref(&self) -> &Utf8Path {
        self.0.as_ref()
    }
}

impl std::ops::Deref for SiteFile {
    type Target = Utf8Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&str> for SiteFile {
    fn from(value: &str) -> Self {
        Self(Utf8PathBuf::from(value))
    }
}

impl From<Utf8PathBuf> for SiteFile {
    fn from(value: Utf8PathBuf) -> Self {
        Self(value)
    }
}

impl From<&Utf8Path> for SiteFile {
    fn from(value: &Utf8Path) -> Self {
        Self(value.to_path_buf())
    }
}

impl Display for SiteFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub mod ext {
    use super::EXT_FILE_REG;
    use crate::ext::anyhow::{Context, Result};
    use camino::Utf8Path;

    /// check after writing the file if it changed
    pub async fn did_file_change(to: &Utf8Path) -> Result<bool> {
        let new_hash = super::file_hash(to).await.dot()?;
        let cur_hash = { EXT_FILE_REG.read().await.get(to.as_str()).copied() };
        if Some(new_hash) == cur_hash {
            return Ok(false);
        }
        let mut f = EXT_FILE_REG.write().await;
        f.insert(to.to_string(), new_hash);
        log::trace!("Site update hash for {to} to {new_hash}");
        Ok(true)
    }
}

pub async fn copy_file_if_changed(from: &Utf8Path, to: &SiteFile) -> Result<bool> {
    let dest = get_dest(to).await?;

    let new_hash = file_hash(&from).await?;
    let cur_hash = current_hash(to, &dest).await?;

    if Some(new_hash) == cur_hash {
        return Ok(false);
    }

    fs::copy(from, dest).await?;

    let mut reg = FILE_REG.write().await;
    reg.insert(to.to_string(), new_hash);
    Ok(true)
}

/// check after writing the file if it changed
pub async fn did_file_change(to: &SiteFile) -> Result<bool> {
    let new_hash = file_hash(&to.to_relative()).await.dot()?;
    let cur_hash = { FILE_REG.read().await.get(to.as_str()).copied() };
    if Some(new_hash) == cur_hash {
        return Ok(false);
    }
    let mut f = FILE_REG.write().await;
    f.insert(to.to_string(), new_hash);
    Ok(true)
}

pub async fn write_if_changed(to: &SiteFile, data: &[u8]) -> Result<bool> {
    let dest = get_dest(to).await?;

    let new_hash = seahash::hash(data);
    let cur_hash = current_hash(to, &dest).await?;

    if Some(new_hash) == cur_hash {
        return Ok(false);
    }

    fs::write(dest, &data).await?;

    let mut reg = FILE_REG.write().await;
    reg.insert(to.to_string(), new_hash);
    Ok(true)
}

async fn get_dest(to: &SiteFile) -> Result<Utf8PathBuf> {
    let root = SITE_ROOT.get().unwrap();

    if to.components().count() > 1 {
        let mut to = to.to_path_buf();
        to.pop();
        let dir = root.join(&to);
        if !dir.exists() {
            fs::create_dir_all(dir).await.dot()?;
        }
    }

    Ok(root.join(to))
}

async fn file_hash(file: &Utf8Path) -> Result<u64> {
    let data = fs::read(&file).await?;
    Ok(seahash::hash(&data))
}

async fn current_hash(to: &Utf8Path, dest: &Utf8Path) -> Result<Option<u64>> {
    if let Some(hash) = FILE_REG.read().await.get(to.as_str()).copied() {
        Ok(Some(hash))
    } else if dest.exists() {
        Ok(Some(file_hash(dest).await?))
    } else {
        Ok(None)
    }
}
