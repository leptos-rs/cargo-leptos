use std::{
    collections::HashMap,
    fmt::{self, Display},
    net::SocketAddr,
};

use camino::{Utf8Path, Utf8PathBuf};
use tokio::sync::RwLock;

use crate::internal_prelude::*;
use crate::{
    config::ProjectConfig,
    ext::{fs, PathBufExt},
};

#[derive(Clone)]
pub struct SourcedSiteFile {
    /// source file's relative path from the root (workspace or project) directory
    pub source: Utf8PathBuf,
    /// dest file's relative path from the root (workspace or project) directory
    pub dest: Utf8PathBuf,
    /// dest file's relative path from the site directory
    pub site: Utf8PathBuf,
}

impl SourcedSiteFile {
    pub fn as_site_file(&self) -> SiteFile {
        SiteFile {
            dest: self.dest.clone(),
            site: self.site.clone(),
        }
    }
}

impl std::fmt::Debug for SourcedSiteFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourcedSiteFile")
            .field("source", &self.source.test_string())
            .field("dest", &self.dest.test_string())
            .field("site", &self.site.test_string())
            .finish()
    }
}

impl Display for SourcedSiteFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} -> @{}", self.source, self.site)
    }
}

#[derive(Clone)]
pub struct SiteFile {
    /// dest file's relative path from the root (workspace or project) directory
    pub dest: Utf8PathBuf,
    /// dest file's relative path from the site directory
    pub site: Utf8PathBuf,
}

impl Display for SiteFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}", self.site)
    }
}

impl std::fmt::Debug for SiteFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SiteFile")
            .field("dest", &self.dest.test_string())
            .field("site", &self.site.test_string())
            .finish()
    }
}

pub struct Site {
    pub addr: SocketAddr,
    pub reload: SocketAddr,
    pub root_dir: Utf8PathBuf,
    pub pkg_dir: Utf8PathBuf,
    file_reg: RwLock<HashMap<String, u64>>,
    ext_file_reg: RwLock<HashMap<String, u64>>,
}

impl fmt::Debug for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Site")
            .field("addr", &self.addr)
            .field("reload", &self.reload)
            .field("root_dir", &self.root_dir)
            .field("pkg_dir", &self.pkg_dir)
            .field("file_reg", &self.file_reg.blocking_read())
            .field("ext_file_reg", &self.ext_file_reg.blocking_read())
            .finish()
    }
}

impl Site {
    pub fn new(config: &ProjectConfig) -> Self {
        let mut reload = config.site_addr;
        reload.set_port(config.reload_port);
        Self {
            addr: config.site_addr,
            reload,
            root_dir: config.site_root.clone(),
            pkg_dir: config.site_pkg_dir.clone(),
            file_reg: Default::default(),
            ext_file_reg: Default::default(),
        }
    }

    pub fn root_relative_pkg_dir(&self) -> Utf8PathBuf {
        self.root_dir.join(&self.pkg_dir)
    }
    /// check if the file changed
    pub async fn did_external_file_change(&self, to: &Utf8Path) -> Result<bool> {
        let new_hash = file_hash(to).await.dot()?;
        let cur_hash = { self.ext_file_reg.read().await.get(to.as_str()).copied() };
        if Some(new_hash) == cur_hash {
            return Ok(false);
        }
        let mut f = self.ext_file_reg.write().await;
        f.insert(to.to_string(), new_hash);
        trace!("Site update hash for {to} to {new_hash}");
        Ok(true)
    }

    pub async fn updated(&self, file: &SourcedSiteFile) -> Result<bool> {
        fs::create_dir_all(file.dest.clone().without_last()).await?;

        let new_hash = file_hash(&file.source).await?;
        let cur_hash = self.current_hash(&file.site, &file.dest).await?;

        if Some(new_hash) == cur_hash {
            return Ok(false);
        }
        fs::copy(&file.source, &file.dest).await?;

        let mut reg = self.file_reg.write().await;
        reg.insert(file.site.to_string(), new_hash);
        Ok(true)
    }

    /// check after writing the file if it changed
    pub async fn did_file_change(&self, file: &SiteFile) -> Result<bool> {
        let new_hash = file_hash(&file.dest).await.dot()?;
        let cur_hash = { self.file_reg.read().await.get(file.site.as_str()).copied() };
        if Some(new_hash) == cur_hash {
            return Ok(false);
        }
        let mut f = self.file_reg.write().await;
        f.insert(file.site.to_string(), new_hash);
        Ok(true)
    }

    pub async fn updated_with(&self, file: &SiteFile, data: &[u8]) -> Result<bool> {
        fs::create_dir_all(file.dest.clone().without_last()).await?;

        let new_hash = seahash::hash(data);
        let cur_hash = self.current_hash(&file.site, &file.dest).await?;

        debug!(
            "Site updated_with: {} bytes to {}, new_hash={}, cur_hash={:?}",
            data.len(),
            file.dest,
            new_hash,
            cur_hash
        );

        if Some(new_hash) == cur_hash {
            debug!("Site updated_with: hash unchanged, skipping write");
            return Ok(false);
        }

        info!(
            "Site updated_with: WRITING {} bytes to {}",
            data.len(),
            file.dest
        );

        // Check file state BEFORE write
        let before_size = match tokio::fs::metadata(&file.dest).await {
            Ok(meta) => Some(meta.len()),
            Err(_) => None,
        };
        debug!(
            "Site updated_with: file size BEFORE write: {:?}",
            before_size
        );

        fs::write(&file.dest, &data).await?;

        // Verify the write IMMEDIATELY
        let after_size = match tokio::fs::metadata(&file.dest).await {
            Ok(meta) => meta.len(),
            Err(e) => {
                error!("Site updated_with: failed to get metadata after write: {e}");
                0
            }
        };

        if after_size != data.len() as u64 {
            error!(
                "Site updated_with: SIZE MISMATCH! Wrote {} bytes but file is {} bytes: {}",
                data.len(),
                after_size,
                file.dest
            );
        } else {
            info!(
                "Site updated_with: VERIFIED {} bytes written to {}",
                after_size, file.dest
            );
        }

        // Double-check by reading the file back
        match tokio::fs::read(&file.dest).await {
            Ok(contents) => {
                if contents.len() != data.len() {
                    error!(
                        "Site updated_with: READ-BACK MISMATCH! Expected {} bytes, got {} bytes",
                        data.len(),
                        contents.len()
                    );
                }
            }
            Err(e) => {
                error!("Site updated_with: failed to read back file: {e}");
            }
        }

        let mut reg = self.file_reg.write().await;
        reg.insert(file.site.to_string(), new_hash);
        Ok(true)
    }

    async fn current_hash(&self, site: &Utf8Path, dest: &Utf8Path) -> Result<Option<u64>> {
        if let Some(hash) = self.file_reg.read().await.get(site.as_str()).copied() {
            Ok(Some(hash))
        } else if dest.exists() {
            Ok(Some(file_hash(dest).await?))
        } else {
            Ok(None)
        }
    }
}

async fn file_hash(file: &Utf8Path) -> Result<u64> {
    let data = fs::read(&file).await?;
    Ok(seahash::hash(&data))
}
