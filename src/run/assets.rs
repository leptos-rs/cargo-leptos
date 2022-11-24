use super::watch::Watched;
use crate::{fs, logger::GRAY, path::PathExt, util::StrAdditions, Config, Msg, MSG_BUS};
use anyhow_ext::{Context, Result};
use std::path::PathBuf;
use tokio::task::JoinHandle;

const DEST: &str = "target/site";

pub async fn spawn(assets_dir: &str) -> Result<JoinHandle<()>> {
    let mut rx = MSG_BUS.subscribe();

    let dest = DEST.to_canoncial_dir()?;
    let src = assets_dir.to_canoncial_dir()?;
    resync(&src, &dest)
        .await
        .context(format!("Could not synchronize {src:?} with {dest:?}"))?;

    let reserved = reserved(&src);

    log::trace!("Assets updater started");
    Ok(tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(Msg::AssetsChanged(watched)) => {
                    if let Err(e) = update_asset(watched, &src, &dest, &reserved).await {
                        log::debug!(
                            "Assets resyncing all due to error: {}",
                            GRAY.paint(e.to_string())
                        );
                        resync(&src, &dest).await.unwrap();
                    }
                }
                Err(e) => {
                    log::debug!("Assets recive error {e}");
                    break;
                }
                Ok(Msg::ShutDown) => break,
                _ => {}
            }
        }
        log::debug!("Assets updater closed")
    }))
}

async fn update_asset(
    watched: Watched,
    src_root: &PathBuf,
    dest_root: &PathBuf,
    reserved: &[PathBuf],
) -> Result<()> {
    if let Some(path) = watched.path() {
        if reserved.contains(path) {
            log::warn!("Assets reserved filename for Leptos. Please remove {path:?}");
            return Ok(());
        }
    }
    match watched {
        Watched::Create(f) => {
            let to = f.rebase(src_root, dest_root)?;
            if f.is_dir() {
                fs::copy_dir_all(f, to).await?;
            } else {
                fs::copy(&f, &to).await?;
            }
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
        }
        Watched::Rename(from, to) => {
            let from = from.rebase(src_root, dest_root)?;
            let to = to.rebase(src_root, dest_root)?;
            fs::rename(&from, &to)
                .await
                .context(format!("rename {from:?} to {to:?}"))?;
        }
        Watched::Write(f) => {
            let to = f.rebase(src_root, dest_root)?;
            fs::copy(&f, &to).await?;
        }
        Watched::Rescan => resync(src_root, dest_root).await?,
    }
    MSG_BUS.send(Msg::Reload("reload".to_string()))?;
    Ok(())
}

pub fn reserved(src: &PathBuf) -> Vec<PathBuf> {
    vec![src.with("index.html"), src.with("pkg")]
}

pub async fn update(config: &Config) -> Result<()> {
    if let Some(src) = &config.leptos.assets_dir {
        let dest = DEST.to_canoncial_dir().dot()?;
        let src = src.to_canoncial_dir().dot()?;

        resync(&src, &dest)
            .await
            .context(format!("Could not synchronize {src:?} with {dest:?}"))?;
    }
    Ok(())
}

async fn resync(src: &PathBuf, dest: &PathBuf) -> Result<()> {
    clean_dest(dest)
        .await
        .context(format!("Cleaning {dest:?}"))?;
    let reserved = reserved(src);
    mirror(src, dest, &reserved)
        .await
        .context(format!("Mirroring {src:?} -> {dest:?}"))
}

async fn clean_dest(dest: &PathBuf) -> Result<()> {
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

async fn mirror(src_root: &PathBuf, dest_root: &PathBuf, reserved: &[PathBuf]) -> Result<()> {
    let mut entries = fs::read_dir(src_root).await?;
    while let Some(entry) = entries.next_entry().await? {
        let from = entry.path();
        let to = from.rebase(src_root, dest_root)?;
        if reserved.contains(&from) {
            log::warn!("");
            continue;
        }

        if entry.file_type().await?.is_dir() {
            log::debug!(
                "Assets copy folder {} -> {}",
                GRAY.paint(from.to_string_lossy()),
                GRAY.paint(to.to_string_lossy())
            );
            fs::copy(from, to).await?;
        } else {
            log::debug!(
                "Assets copy file {} -> {}",
                GRAY.paint(from.to_string_lossy()),
                GRAY.paint(to.to_string_lossy())
            );
            fs::copy(from, to).await?;
        }
    }
    Ok(())
}
