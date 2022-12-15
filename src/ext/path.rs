use crate::ext::anyhow::{bail, ensure, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};

pub trait PathExt {
    /// converts this absolute path to relative if the start matches
    fn relative_to(&self, to: impl AsRef<Utf8Path>) -> Option<Utf8PathBuf>;

    /// removes the src_root from the path and adds the dest_root
    fn rebase(&self, src_root: &Utf8Path, dest_root: &Utf8Path) -> Result<Utf8PathBuf>;

    /// removes base from path (making sure they match)
    fn unbase(&self, base: &Utf8Path) -> Result<Utf8PathBuf>;

    /// As .canonicalize() but returning a contextualized anyhow Result
    fn to_canonicalized(&self) -> Result<Utf8PathBuf>;
}

pub trait PathBufExt: PathExt {
    /// drops the last path component
    fn without_last(self) -> Self;
}

impl PathExt for Utf8Path {
    fn rebase(&self, src_root: &Utf8Path, dest_root: &Utf8Path) -> Result<Utf8PathBuf> {
        self.to_path_buf().rebase(src_root, dest_root)
    }

    fn relative_to(&self, to: impl AsRef<Utf8Path>) -> Option<Utf8PathBuf> {
        self.to_path_buf().relative_to(to)
    }

    fn unbase(&self, base: &Utf8Path) -> Result<Utf8PathBuf> {
        let mut self_comp_iter = self.components();

        for base_comp in base.components() {
            match self_comp_iter.next() {
                Some(self_comp) if base_comp != self_comp => {
                    bail!("Cannot remove base {base:?} from {self:?} because base doesn't match")
                }
                None => bail!("Cannot remove base {base:?} from {self:?} because base is longer"),
                _ => {}
            };
        }
        Ok(Utf8PathBuf::from_iter(self_comp_iter))
    }

    fn to_canonicalized(&self) -> Result<Utf8PathBuf> {
        self.to_path_buf().to_canonicalized()
    }
}

impl PathBufExt for Utf8PathBuf {
    fn without_last(mut self) -> Utf8PathBuf {
        self.pop();
        self
    }
}

impl PathExt for Utf8PathBuf {
    fn relative_to(&self, to: impl AsRef<Utf8Path>) -> Option<Utf8PathBuf> {
        let root = to.as_ref();
        if self.is_absolute() && self.starts_with(root) {
            let len = root.components().count();
            Some(self.components().skip(len).collect())
        } else {
            None
        }
    }
    fn rebase(&self, src_root: &Utf8Path, dest_root: &Utf8Path) -> Result<Utf8PathBuf>
    where
        Self: Sized,
    {
        ensure!(src_root.is_absolute(), "Not canonicalized: {src_root:?}");
        ensure!(dest_root.is_absolute(), "Not canonicalized: {dest_root:?}");
        if let Some(rel) = self.relative_to(src_root) {
            Ok(dest_root.join(rel))
        } else {
            bail!("Could not rebase {self:?} from {src_root:?} to {dest_root:?}")
        }
    }

    fn unbase(&self, base: &Utf8Path) -> Result<Utf8PathBuf> {
        self.as_path().unbase(base)
    }

    fn to_canonicalized(&self) -> Result<Utf8PathBuf>
    where
        Self: Sized,
    {
        Ok(Utf8PathBuf::from_path_buf(
            self.as_path()
                .canonicalize()
                .context(format!("Could not canonicalize {:?}", self))?,
        )
        .unwrap())
    }
}

pub fn remove_nested(paths: Vec<Utf8PathBuf>) -> Vec<Utf8PathBuf> {
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
