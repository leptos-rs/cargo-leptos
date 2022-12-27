use std::sync::Arc;

use super::ChangeSet;
use crate::config::Project;
use crate::ext::anyhow::{Context, Result};
use crate::service::notify::Watched;
use crate::service::site::SourcedSiteFile;
use crate::signal::{Outcome, Product};
use crate::{ext::PathExt, fs, logger::GRAY};
use camino::{Utf8Path, Utf8PathBuf};
use tokio::task::JoinHandle;

pub async fn assets(
    proj: &Arc<Project>,
    changes: &ChangeSet,
    first_sync: bool,
) -> JoinHandle<Result<Outcome>> {
    let changes = changes.clone();

    let proj = proj.clone();
    tokio::spawn(async move {
        let src_root = match &proj.paths.assets_dir {
            Some(dir) => dir,
            None => return Ok(Outcome::Success(Product::NoChange)),
        };
        let dest_root = &proj.paths.site_root;

        let change = if first_sync {
            log::trace!("Assets starting full resync");
            resync(&src_root, dest_root).await?;
            true
        } else {
            let mut changed = false;
            for watched in changes.asset_iter() {
                log::trace!("Assets processing {watched:?}");
                let change =
                    update_asset(&proj, watched.clone(), &src_root, dest_root, &[]).await?;
                changed |= change;
            }
            changed
        };
        if change {
            log::debug!("Assets finished (with changes)");
            Ok(Outcome::Success(Product::Assets))
        } else {
            log::debug!("Assets finished (no changes)");
            Ok(Outcome::Success(Product::NoChange))
        }
    })
}

async fn update_asset(
    proj: &Project,
    watched: Watched,
    src_root: &Utf8Path,
    dest_root: &Utf8Path,
    reserved: &[Utf8PathBuf],
) -> Result<bool> {
    if let Some(path) = watched.path() {
        if reserved.contains(path) {
            log::warn!("Assets reserved filename for Leptos. Please remove {path:?}");
            return Ok(false);
        }
    }
    Ok(match watched {
        Watched::Create(f) => {
            let to = f.rebase(src_root, dest_root)?;
            if f.is_dir() {
                fs::copy_dir_all(f, to).await?;
            } else {
                fs::copy(&f, &to).await?;
            }
            true
        }
        Watched::Remove(f) => {
            let path = f.rebase(src_root, dest_root)?;
            if path.is_dir() {
                fs::remove_dir_all(&path)
                    .await
                    .context(format!("remove dir recursively {path:?}"))?;
            } else {
                fs::remove_file(&path)
                    .await
                    .context(format!("remove file {path:?}"))?;
            }
            false
        }
        Watched::Rename(from, to) => {
            let from = from.rebase(src_root, dest_root)?;
            let to = to.rebase(src_root, dest_root)?;
            fs::rename(&from, &to)
                .await
                .context(format!("rename {from:?} to {to:?}"))?;
            true
        }
        Watched::Write(f) => {
            let file = SourcedSiteFile {
                source: f.clone(),
                dest: f.rebase(src_root, dest_root)?,
                site: f.unbase(src_root)?,
            };
            proj.site.updated(&file).await?
        }
        Watched::Rescan => {
            resync(src_root, dest_root).await?;
            true
        }
    })
}

pub fn reserved(src: &Utf8Path) -> Vec<Utf8PathBuf> {
    vec![src.join("index.html"), src.join("pkg")]
}

// pub async fn update(config: &Config) -> Result<()> {
//     if let Some(src) = &config.leptos.assets_dir {
//         let dest = DEST.to_canoncial_dir().dot()?;
//         let src = src.to_canoncial_dir().dot()?;

//         resync(&src, &dest)
//             .await
//             .context(format!("Could not synchronize {src:?} with {dest:?}"))?;
//     }
//     Ok(())
// }

async fn resync(src: &Utf8Path, dest: &Utf8Path) -> Result<()> {
    clean_dest(dest)
        .await
        .context(format!("Cleaning {dest:?}"))?;
    let reserved = reserved(src);
    mirror(src, dest, &reserved)
        .await
        .context(format!("Mirroring {src:?} -> {dest:?}"))
}

async fn clean_dest(dest: &Utf8Path) -> Result<()> {
    let mut entries = fs::read_dir(dest).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if entry.file_type().await?.is_dir() {
            if entry.file_name() != "pkg" {
                log::debug!(
                    "Assets removing folder {}",
                    GRAY.paint(path.to_string_lossy())
                );
                fs::remove_dir_all(path).await?;
            }
        } else if entry.file_name() != "index.html" {
            log::debug!(
                "Assets removing file {}",
                GRAY.paint(path.to_string_lossy())
            );
            fs::remove_file(path).await?;
        }
    }
    Ok(())
}

async fn mirror(src_root: &Utf8Path, dest_root: &Utf8Path, reserved: &[Utf8PathBuf]) -> Result<()> {
    let mut entries = src_root.read_dir_utf8()?;
    while let Some(Ok(entry)) = entries.next() {
        let from = entry.path().to_path_buf();
        let to = from.rebase(src_root, dest_root)?;
        if reserved.contains(&from) {
            log::warn!("");
            continue;
        }

        if entry.file_type()?.is_dir() {
            log::debug!(
                "Assets copy folder {} -> {}",
                GRAY.paint(from.as_str()),
                GRAY.paint(to.as_str())
            );
            fs::copy_dir_all(from, to).await?;
        } else {
            log::debug!(
                "Assets copy file {} -> {}",
                GRAY.paint(from.as_str()),
                GRAY.paint(to.as_str())
            );
            fs::copy(from, to).await?;
        }
    }
    Ok(())
}
