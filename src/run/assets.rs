use super::watch::Watched;
use crate::{fs, fs::PathBufAdditions, logger::GRAY, util::StrAdditions, Config, Msg, MSG_BUS};
use anyhow_ext::{Context, Result};
use std::path::PathBuf;
use tokio::task::JoinHandle;

const DEST: &str = "target/site";

pub async fn spawn(assets_dir: &str) -> Result<JoinHandle<()>> {
    let mut rx = MSG_BUS.subscribe();

    let dest = DEST.to_canoncial_dir()?;
    let src = assets_dir.to_canoncial_dir()?;
    resync(&src, &dest).context(format!("Could not synchronize {src:?} with {dest:?}"))?;

    let reserved = reserved(&src);

    log::trace!("Assets updater started");
    Ok(tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(Msg::AssetsChanged(watched)) => {
                    if let Err(e) = update_asset(watched, &src, &dest, &reserved) {
                        log::debug!(
                            "Assets resyncing all due to error: {}",
                            GRAY.paint(e.to_string())
                        );
                        resync(&src, &dest).unwrap();
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

fn update_asset(
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
                fs::copy_dir_all(f, to)?;
            } else {
                fs::copy(&f, &to)?;
            }
        }
        Watched::Remove(f) => {
            let path = f.rebase(src_root, dest_root)?;
            if path.is_dir() {
                fs::remove_dir_all(&path).context(format!("remove dir recursively {path:?}"))?;
            } else {
                fs::remove_file(&path).context(format!("remove file {path:?}"))?;
            }
        }
        Watched::Rename(from, to) => {
            let from = from.rebase(src_root, dest_root)?;
            let to = to.rebase(src_root, dest_root)?;
            fs::rename(&from, &to).context(format!("rename {from:?} to {to:?}"))?;
        }
        Watched::Write(f) => {
            let to = f.rebase(src_root, dest_root)?;
            fs::copy(&f, &to)?;
        }
        Watched::Rescan => resync(src_root, dest_root)?,
    }
    MSG_BUS.send(Msg::Reload("reload".to_string()))?;
    Ok(())
}

pub fn reserved(src: &PathBuf) -> Vec<PathBuf> {
    vec![src.with("index.html"), src.with("pkg")]
}

pub fn update(config: &Config) -> Result<()> {
    if let Some(src) = &config.leptos.assets_dir {
        let dest = DEST.to_canoncial_dir().dot()?;
        let src = src.to_canoncial_dir().dot()?;

        resync(&src, &dest).context(format!("Could not synchronize {src:?} with {dest:?}"))?;
    }
    Ok(())
}

fn resync(src: &PathBuf, dest: &PathBuf) -> Result<()> {
    clean_dest(dest).context(format!("Cleaning {dest:?}"))?;
    let reserved = reserved(src);
    mirror(src, dest, &reserved).context(format!("Mirroring {src:?} -> {dest:?}"))
}

fn clean_dest(dest: &PathBuf) -> Result<()> {
    for entry in fs::read_dir(dest)? {
        let entry = entry?;
        let path = entry.path();

        if entry.file_type()?.is_dir() {
            if entry.file_name() != "pkg" {
                log::debug!(
                    "Assets removing folder {}",
                    GRAY.paint(path.to_string_lossy())
                );
                fs::remove_dir_all(path)?;
            }
        } else if entry.file_name() != "index.html" {
            log::debug!(
                "Assets removing file {}",
                GRAY.paint(path.to_string_lossy())
            );
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn mirror(src_root: &PathBuf, dest_root: &PathBuf, reserved: &[PathBuf]) -> Result<()> {
    for entry in fs::read_dir(src_root)? {
        let entry = entry?;
        let from = entry.path();
        let to = from.rebase(src_root, dest_root)?;
        if reserved.contains(&from) {
            log::warn!("");
            continue;
        }

        if entry.file_type()?.is_dir() {
            log::debug!(
                "Assets copy folder {} -> {}",
                GRAY.paint(from.to_string_lossy()),
                GRAY.paint(to.to_string_lossy())
            );
            fs::copy(from, to)?;
        } else {
            log::debug!(
                "Assets copy file {} -> {}",
                GRAY.paint(from.to_string_lossy()),
                GRAY.paint(to.to_string_lossy())
            );
            fs::copy(from, to)?;
        }
    }
    Ok(())
}
