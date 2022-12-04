use super::path::PathExt;
use crate::ext::anyhow::{Context, Result};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};
use tokio::fs::{self, ReadDir};

pub async fn rm_dir_content<P: AsRef<Path>>(dir: P) -> Result<()> {
    try_rm_dir_content(&dir)
        .await
        .context(format!("Could not remove contents of {:?}", dir.as_ref()))
}

async fn try_rm_dir_content<P: AsRef<Path>>(dir: P) -> Result<()> {
    let dir = dir.as_ref();

    if !dir.exists() {
        log::debug!("Leptos not cleaning {dir:?} because it does not exist");
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

pub async fn write_if_changed<P: AsRef<Path>, C: AsRef<[u8]>>(
    path: P,
    contents: C,
) -> Result<bool> {
    if path.as_ref().exists() {
        let current = self::read_to_string(&path).await?;
        let current_hash = seahash::hash(current.as_bytes());
        let new_hash = seahash::hash(contents.as_ref());
        if current_hash != new_hash {
            self::write(&path, contents).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    } else {
        self::write(&path, contents).await?;
        Ok(true)
    }
}

pub async fn create_dir(path: impl AsRef<Path>) -> Result<()> {
    fs::create_dir(&path)
        .await
        .context(format!("Could not create dir {:?}", path.as_ref()))
}

pub async fn create_dir_all<P: AsRef<Path>>(path: P) -> Result<()> {
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

pub async fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    cp_dir_all(&src, &dst).await.context(format!(
        "Copy dir recursively from {:?} to {:?}",
        src.as_ref(),
        dst.as_ref()
    ))
}

async fn cp_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    let src = src.as_ref().to_canonicalized()?;
    let dst = dst.as_ref().to_path_buf();

    self::create_dir_all(&dst).await?;

    let mut dirs = VecDeque::new();
    dirs.push_back(src.clone());

    while let Some(dir) = dirs.pop_front() {
        let mut entries = self::read_dir(&dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let from = entry.path();
            let to = from.rebase(&src, &dst)?;

            if entry.file_type().await?.is_dir() {
                self::create_dir(&to).await?;
                dirs.push_back(from);
            } else {
                self::copy(from, to).await?;
            }
        }
    }
    Ok(())
}

pub fn remove_nested(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.into_iter().fold(vec![], |mut vec, path| {
        for added in vec.iter_mut() {
            // path is a parent folder of added
            if added.starts_with(&path) {
                *added = path;
                return vec;
            }
            // path is a sub folder of added
            if path.starts_with(added) {
                return vec;
            }
        }
        vec.push(path);
        vec
    })
}
