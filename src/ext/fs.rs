use anyhow_ext::{bail, Context, Result};
use std::fs::{self, ReadDir};
use std::path::{Path, PathBuf};

pub(crate) fn rm_dir_content<P: AsRef<Path>>(dir: P) -> Result<()> {
    let dir = dir.as_ref();

    if !dir.exists() {
        log::debug!("Leptos not cleaning {dir:?} because it does not exist");
        return Ok(());
    }

    for entry in self::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if entry.file_type()?.is_dir() {
            rm_dir_content(&path)?;
            self::remove_dir(path)?;
        } else {
            self::remove_file(path)?;
        }
    }
    Ok(())
}

pub fn write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
    fs::write(&path, contents).context(format!("Could not write to {:?}", path.as_ref()))
}

pub fn write_if_changed<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<bool> {
    if path.as_ref().exists() {
        let current = self::read_to_string(&path)?;
        let current_hash = seahash::hash(current.as_bytes());
        let new_hash = seahash::hash(contents.as_ref());
        if current_hash != new_hash {
            self::write(&path, contents)?;
            Ok(true)
        } else {
            Ok(false)
        }
    } else {
        self::write(&path, contents)?;
        Ok(true)
    }
}

pub fn create_dir_all<P: AsRef<Path>>(path: P) -> Result<()> {
    fs::create_dir_all(&path).context(format!("Could not create {:?}", path.as_ref()))
}
pub fn read_to_string<P: AsRef<Path>>(path: P) -> Result<String> {
    fs::read_to_string(&path).context(format!("Could not read to string {:?}", path.as_ref()))
}

pub fn copy<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> Result<u64> {
    fs::copy(&from, &to).context(format!("copy {:?} to {:?}", from.as_ref(), to.as_ref()))
}

pub fn read_dir<P: AsRef<Path>>(path: P) -> Result<ReadDir> {
    fs::read_dir(&path).context(format!("Could not read dir {:?}", path.as_ref()))
}

pub fn rename<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> Result<()> {
    fs::rename(&from, &to).context(format!(
        "Could not rename from {:?} to {:?}",
        from.as_ref(),
        to.as_ref()
    ))
}

pub fn remove_file<P: AsRef<Path>>(path: P) -> Result<()> {
    fs::remove_file(&path).context(format!("Could not remove file {:?}", path.as_ref()))
}

pub fn remove_dir<P: AsRef<Path>>(path: P) -> Result<()> {
    fs::remove_dir(&path).context(format!("Could not remove dir {:?}", path.as_ref()))
}

pub fn remove_dir_all<P: AsRef<Path>>(path: P) -> Result<()> {
    fs::remove_dir_all(&path).context(format!("Could not remove dir {:?}", path.as_ref()))
}

pub fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    cp_dir_all(&src, &dst).context(format!(
        "Copy dir recursively from {:?} to {:?}",
        src.as_ref(),
        dst.as_ref()
    ))
}

fn cp_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    self::create_dir_all(&dst)?;
    for entry in self::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            self::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
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

pub trait PathBufAdditions {
    /// drops the last path component
    fn without_last(self) -> Self;

    /// appends to path
    fn with<P: AsRef<Path>>(&self, append: P) -> Self;

    /// converts this absolute path to relative if the start matches
    fn relative_to(&self, to: impl AsRef<Path>) -> Option<PathBuf>;

    /// removes the src_root from the path and adds the dest_root
    fn rebase(&self, src_root: &PathBuf, dest_root: &PathBuf) -> Result<Self>
    where
        Self: Sized;
}

impl PathBufAdditions for PathBuf {
    fn without_last(mut self) -> Self {
        self.pop();
        self
    }
    fn with<P: AsRef<Path>>(&self, append: P) -> Self {
        let mut new = self.clone();
        new.push(append);
        new
    }
    fn relative_to(&self, to: impl AsRef<Path>) -> Option<PathBuf> {
        let root = to.as_ref();
        if self.is_absolute() && self.starts_with(root) {
            let len = root.components().count();
            Some(self.components().skip(len).collect())
        } else {
            None
        }
    }
    fn rebase(&self, src_root: &PathBuf, dest_root: &PathBuf) -> Result<Self>
    where
        Self: Sized,
    {
        if let Some(rel) = self.relative_to(src_root) {
            Ok(dest_root.with(rel))
        } else {
            bail!("Could not rebase {self:?} from {src_root:?} to {dest_root:?}")
        }
    }
}
