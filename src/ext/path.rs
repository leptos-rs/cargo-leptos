use crate::ext::anyhow::{bail, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};

pub trait PathExt {
    /// converts this absolute path to relative if the start matches
    fn relative_to(&self, to: impl AsRef<Utf8Path>) -> Option<Utf8PathBuf>;

    /// removes the src_root from the path and adds the dest_root
    fn rebase(&self, src_root: &Utf8Path, dest_root: &Utf8Path) -> Result<Utf8PathBuf>;

    /// removes base from path (making sure they match)
    fn unbase(&self, base: &Utf8Path) -> Result<Utf8PathBuf>;
}

pub trait PathBufExt: PathExt {
    /// drops the last path component
    fn without_last(self) -> Self;

    #[cfg(test)]
    /// returns a platform independent string suitable for testing
    fn test_string(&self) -> String;

    #[cfg(test)]
    fn ls_ascii(&self, indent: usize) -> Result<String>;
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
        let path = Utf8PathBuf::from_iter(self_comp_iter);
        Ok(if path == "" {
            Utf8PathBuf::from(".")
        } else {
            path
        })
    }
}

impl PathBufExt for Utf8PathBuf {
    fn without_last(mut self) -> Utf8PathBuf {
        self.pop();
        self
    }

    #[cfg(test)]
    fn test_string(&self) -> String {
        let s = self.to_string().replace("\\", "/");
        if s.ends_with(".exe") {
            s[..s.len() - 4].to_string()
        } else {
            s
        }
    }

    #[cfg(test)]
    fn ls_ascii(&self, indent: usize) -> Result<String> {
        let mut out = Vec::new();

        let mut entries = self.read_dir_utf8()?;
        out.push(format!(
            "{}{}:",
            "  ".repeat(indent),
            self.file_name().unwrap_or_default()
        ));

        let indent = indent + 1;
        while let Some(Ok(entry)) = entries.next() {
            let path = entry.path().to_path_buf();

            if entry.file_type()?.is_dir() {
                out.push(path.ls_ascii(indent)?);
            } else {
                out.push(format!(
                    "{}{}",
                    "  ".repeat(indent),
                    path.file_name().unwrap_or_default()
                ));
            }
        }
        Ok(out.join("\n"))
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
        let unbased = self
            .unbase(src_root)
            .dot()
            .context(format!("Rebase {self} from {src_root} to {dest_root}"))?;
        Ok(dest_root.join(unbased))
    }

    fn unbase(&self, base: &Utf8Path) -> Result<Utf8PathBuf> {
        self.as_path().unbase(base)
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
