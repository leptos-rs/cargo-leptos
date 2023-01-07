use crate::ext::anyhow::{anyhow, Context, Result};
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

    /// returns a platform independent string suitable for testing
    fn test_string(&self) -> String;

    fn starts_with_any(&self, of: &[Utf8PathBuf]) -> bool;

    fn is_ext_any(&self, of: &[&str]) -> bool;

    fn resolve_home_dir(self) -> Result<Utf8PathBuf>;

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
        self.strip_prefix(base)
            .map(|p| p.to_path_buf())
            .map_err(|_| anyhow!("Could not remove base {base:?} from {self:?}"))
    }
}

impl PathBufExt for Utf8PathBuf {
    fn resolve_home_dir(self) -> Result<Utf8PathBuf> {
        if self.starts_with("~") {
            let home = std::env::var("HOME").context("Could not resolve $HOME")?;
            let home = Utf8PathBuf::from(home);
            Ok(home.join(self.strip_prefix("~").unwrap()))
        } else {
            Ok(self)
        }
    }

    fn without_last(mut self) -> Utf8PathBuf {
        self.pop();
        self
    }

    fn test_string(&self) -> String {
        let s = self.to_string().replace("\\", "/");
        if s.ends_with(".exe") {
            s[..s.len() - 4].to_string()
        } else {
            s
        }
    }

    fn is_ext_any(&self, of: &[&str]) -> bool {
        let Some(ext) = self.extension() else {
            return false
        };
        of.contains(&ext)
    }

    fn starts_with_any(&self, of: &[Utf8PathBuf]) -> bool {
        of.iter().any(|p| self.starts_with(p))
    }

    #[cfg(test)]
    fn ls_ascii(&self, indent: usize) -> Result<String> {
        let mut entries = self.read_dir_utf8()?;
        let mut out = Vec::new();

        out.push(format!(
            "{}{}:",
            "  ".repeat(indent),
            self.file_name().unwrap_or_default()
        ));

        let indent = indent + 1;
        let mut files = Vec::new();
        let mut dirs = Vec::new();

        while let Some(Ok(entry)) = entries.next() {
            let path = entry.path().to_path_buf();

            if entry.file_type()?.is_dir() {
                dirs.push(path);
            } else {
                files.push(path);
            }
        }

        dirs.sort();
        files.sort();

        for file in files {
            out.push(format!(
                "{}{}",
                "  ".repeat(indent),
                file.file_name().unwrap_or_default()
            ));
        }

        for path in dirs {
            out.push(path.ls_ascii(indent)?);
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

pub fn remove_nested(paths: impl Iterator<Item = Utf8PathBuf>) -> Vec<Utf8PathBuf> {
    paths.fold(vec![], |mut vec, path| {
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
