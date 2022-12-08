use crate::ext::anyhow::{bail, ensure, Context, Result};
use std::path::{Path, PathBuf};

pub trait PathExt {
    /// appends to path
    fn with<P: AsRef<Path>>(&self, append: P) -> PathBuf;

    /// converts this absolute path to relative if the start matches
    fn relative_to(&self, to: impl AsRef<Path>) -> Option<PathBuf>;

    /// removes the src_root from the path and adds the dest_root
    fn rebase(&self, src_root: &PathBuf, dest_root: &PathBuf) -> Result<PathBuf>;

    /// As .canonicalize() but returning a contextualized anyhow Result
    fn to_canonicalized(&self) -> Result<PathBuf>;
}
impl PathExt for Path {
    fn rebase(&self, src_root: &PathBuf, dest_root: &PathBuf) -> Result<PathBuf> {
        self.to_path_buf().rebase(src_root, dest_root)
    }

    fn relative_to(&self, to: impl AsRef<Path>) -> Option<PathBuf> {
        self.to_path_buf().relative_to(to)
    }

    fn to_canonicalized(&self) -> Result<PathBuf> {
        self.to_path_buf().to_canonicalized()
    }

    fn with<P: AsRef<Path>>(&self, append: P) -> PathBuf {
        self.to_path_buf().with(append)
    }
}

pub trait PathBufExt: PathExt {
    /// drops the last path component
    fn without_last(self) -> PathBuf;
}

impl PathBufExt for PathBuf {
    fn without_last(mut self) -> PathBuf {
        self.pop();
        self
    }
}
impl PathExt for PathBuf {
    fn with<P: AsRef<Path>>(&self, append: P) -> PathBuf {
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
    fn rebase(&self, src_root: &PathBuf, dest_root: &PathBuf) -> Result<PathBuf>
    where
        Self: Sized,
    {
        ensure!(src_root.is_absolute(), "Not canonicalized: {src_root:?}");
        ensure!(dest_root.is_absolute(), "Not canonicalized: {dest_root:?}");
        if let Some(rel) = self.relative_to(src_root) {
            Ok(dest_root.with(rel))
        } else {
            bail!("Could not rebase {self:?} from {src_root:?} to {dest_root:?}")
        }
    }

    fn to_canonicalized(&self) -> Result<PathBuf>
    where
        Self: Sized,
    {
        self.canonicalize()
            .context(format!("Could not canonicalize {:?}", self))
    }
}
