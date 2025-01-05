use std::sync::Arc;

use super::ChangeSet;
use crate::config::Project;
use crate::ext::anyhow::{Context, Result};
use crate::signal::{Outcome, Product};
use crate::{ext::PathExt, fs, logger::GRAY};
use camino::{Utf8Path, Utf8PathBuf};
use tokio::task::JoinHandle;

pub async fn assets(
    proj: &Arc<Project>,
    changes: &ChangeSet,
) -> JoinHandle<Result<Outcome<Product>>> {
    let changes = changes.clone();

    let proj = proj.clone();
    tokio::spawn(async move {
        if !changes.need_assets_change() {
            return Ok(Outcome::Success(Product::None));
        }
        let Some(assets) = &proj.assets else {
            return Ok(Outcome::Success(Product::None));
        };
        let dest_root = &proj.site.root_dir;
        let pkg_dir = &proj.site.pkg_dir;

        // if reserved.contains(assets.dir) {
        //     log::warn!("Assets reserved filename for Leptos. Please remove {watched:?}");
        //     return Ok(false);
        // }
        log::trace!("Assets starting resync");
        resync(&assets.dir, dest_root, pkg_dir).await?;
        log::debug!("Assets finished");
        Ok(Outcome::Success(Product::Assets))
    })
}

pub fn reserved(src: &Utf8Path, pkg_dir: &Utf8Path) -> Vec<Utf8PathBuf> {
    vec![src.join("index.html"), pkg_dir.to_path_buf()]
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

async fn resync(src: &Utf8Path, dest: &Utf8Path, pkg_dir: &Utf8Path) -> Result<()> {
    clean_dest(dest, pkg_dir)
        .await
        .context(format!("Cleaning {dest:?}"))?;
    let reserved = reserved(src, pkg_dir);
    mirror(src, dest, &reserved)
        .await
        .context(format!("Mirroring {src:?} -> {dest:?}"))
}

async fn clean_dest(dest: &Utf8Path, pkg_dir: &Utf8Path) -> Result<()> {
    let pkg_dir_name = match pkg_dir.file_name() {
        Some(name) => name,
        None => {
            log::warn!(
                "Assets No site-pkg-dir given, defaulting to 'pkg' for checks what to delete."
            );
            log::warn!("Assets This will probably delete already generated files.");
            "pkg"
        }
    };

    let mut entries = fs::read_dir(dest).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if entry.file_type().await?.is_dir() {
            if entry.file_name() != pkg_dir_name {
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
