use crate::ext::anyhow::{Context, Result};
use crate::internal_prelude::*;
use camino::{Utf8Path, Utf8PathBuf};
use std::{collections::VecDeque, path::Path};
use tokio::fs::{self, ReadDir};

use super::path::PathExt;

pub async fn rm_dir_content<P: AsRef<Path>>(dir: P) -> Result<()> {
    try_rm_dir_content(&dir)
        .await
        .context(format!("Could not remove contents of {:?}", dir.as_ref()))
}

async fn try_rm_dir_content<P: AsRef<Path>>(dir: P) -> Result<()> {
    let dir = dir.as_ref();

    if !dir.exists() {
        debug!("Leptos not cleaning {dir:?} because it does not exist");
        return Ok(());
    }

    let mut entries = self::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if entry.file_type().await?.is_dir() {
            self::remove_dir_all(path).await?;
        } else {
            self::remove_file(path).await?;
        }
    }
    Ok(())
}

pub async fn write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
    fs::write(&path, contents)
        .await
        .context(format!("Could not write to {:?}", path.as_ref()))
}

pub async fn read(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    fs::read(&path)
        .await
        .context(format!("Could not read {:?}", path.as_ref()))
}

pub async fn create_dir(path: impl AsRef<Path>) -> Result<()> {
    trace!("FS create_dir {:?}", path.as_ref());
    fs::create_dir(&path)
        .await
        .context(format!("Could not create dir {:?}", path.as_ref()))
}

pub async fn create_dir_all<P: AsRef<Path>>(path: P) -> Result<()> {
    trace!("FS create_dir_all {:?}", path.as_ref());
    fs::create_dir_all(&path)
        .await
        .context(format!("Could not create {:?}", path.as_ref()))
}
pub async fn read_to_string<P: AsRef<Path>>(path: P) -> Result<String> {
    fs::read_to_string(&path)
        .await
        .context(format!("Could not read to string {:?}", path.as_ref()))
}

pub async fn copy<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> Result<u64> {
    fs::copy(&from, &to)
        .await
        .context(format!("copy {:?} to {:?}", from.as_ref(), to.as_ref()))
}

pub async fn read_dir<P: AsRef<Path>>(path: P) -> Result<ReadDir> {
    fs::read_dir(&path)
        .await
        .context(format!("Could not read dir {:?}", path.as_ref()))
}

pub async fn rename<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> Result<()> {
    fs::rename(&from, &to).await.context(format!(
        "Could not rename from {:?} to {:?}",
        from.as_ref(),
        to.as_ref()
    ))
}

pub async fn remove_file<P: AsRef<Path>>(path: P) -> Result<()> {
    fs::remove_file(&path)
        .await
        .context(format!("Could not remove file {:?}", path.as_ref()))
}

#[allow(dead_code)]
pub async fn remove_dir<P: AsRef<Path>>(path: P) -> Result<()> {
    fs::remove_dir(&path)
        .await
        .context(format!("Could not remove dir {:?}", path.as_ref()))
}

pub async fn remove_dir_all<P: AsRef<Path>>(path: P) -> Result<()> {
    fs::remove_dir_all(&path)
        .await
        .context(format!("Could not remove dir {:?}", path.as_ref()))
}

pub async fn copy_dir_all(src: impl AsRef<Utf8Path>, dst: impl AsRef<Path>) -> Result<()> {
    cp_dir_all(&src, &dst).await.context(format!(
        "Copy dir recursively from {:?} to {:?}",
        src.as_ref(),
        dst.as_ref()
    ))
}

async fn cp_dir_all(src: impl AsRef<Utf8Path>, dst: impl AsRef<Path>) -> Result<()> {
    let src = src.as_ref();
    let dst = Utf8PathBuf::from_path_buf(dst.as_ref().to_path_buf()).unwrap();

    self::create_dir_all(&dst).await?;

    let mut dirs = VecDeque::new();
    dirs.push_back(src.to_owned());

    while let Some(dir) = dirs.pop_front() {
        let mut entries = dir.read_dir_utf8()?;

        while let Some(Ok(entry)) = entries.next() {
            let from = entry.path().to_owned();
            let to = from.rebase(src, &dst)?;

            if entry.file_type()?.is_dir() {
                self::create_dir(&to).await?;
                dirs.push_back(from);
            } else {
                self::copy(from, to).await?;
            }
        }
    }
    Ok(())
}
