use crate::internal_prelude::*;
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

    /// cleaning the unc (illegible \\?\) start of windows paths. See dunce crate.
    fn clean_windows_path(&mut self);

    #[cfg(test)]
    fn ls_ascii(&self, indent: usize) -> Result<String>;
}

impl PathExt for Utf8Path {
    fn relative_to(&self, to: impl AsRef<Utf8Path>) -> Option<Utf8PathBuf> {
        self.to_path_buf().relative_to(to)
    }

    fn rebase(&self, src_root: &Utf8Path, dest_root: &Utf8Path) -> Result<Utf8PathBuf> {
        self.to_path_buf().rebase(src_root, dest_root)
    }

    fn unbase(&self, base: &Utf8Path) -> Result<Utf8PathBuf> {
        let path = self
            .strip_prefix(base)
            .map(|p| p.to_path_buf())
            .map_err(|_| eyre!("Could not remove base {base:?} from {self:?}"))?;
        if path == "" {
            Ok(Utf8PathBuf::from("."))
        } else {
            Ok(path)
        }
    }
}

impl PathBufExt for Utf8PathBuf {
    fn without_last(mut self) -> Utf8PathBuf {
        self.pop();
        self
    }

    fn test_string(&self) -> String {
        let s = self.to_string().replace('\\', "/");
        if s.ends_with(".exe") {
            s[..s.len() - 4].to_string()
        } else {
            s
        }
    }

    fn starts_with_any(&self, of: &[Utf8PathBuf]) -> bool {
        of.iter().any(|p| self.starts_with(p))
    }

    fn is_ext_any(&self, of: &[&str]) -> bool {
        let Some(ext) = self.extension() else {
            return false;
        };
        of.contains(&ext)
    }

    fn resolve_home_dir(self) -> Result<Utf8PathBuf> {
        if self.starts_with("~") {
            let home = std::env::var("HOME").wrap_err("Could not resolve $HOME")?;
            let home = Utf8PathBuf::from(home);
            Ok(home.join(self.strip_prefix("~").unwrap()))
        } else {
            Ok(self)
        }
    }

    fn clean_windows_path(&mut self) {
        if cfg!(windows) {
            let cleaned = dunce::simplified(self.as_ref());
            *self = Utf8PathBuf::from_path_buf(cleaned.to_path_buf()).unwrap();
        }
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
            .wrap_err(format!("Rebase {self} from {src_root} to {dest_root}"))?;
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

/// Extension Safe &str Append
///
/// # Arguments
///
/// * `path` - Current path to file
/// * `suffix` - &str to be appened before extension
///
/// # Example
///
/// ```
/// use camino::Utf8PathBuf;
/// use cargo_leptos::ext::append_str_to_filename;
///
/// let path: Utf8PathBuf = "foo.bar".into();
/// assert_eq!(append_str_to_filename(&path, "_bazz").unwrap().as_str(), "foo_bazz.bar");
/// let path: Utf8PathBuf = "a".into();
/// assert_eq!(append_str_to_filename(&path, "b").unwrap().as_str(), "ab");
/// ```
pub fn append_str_to_filename(path: &Utf8PathBuf, suffix: &str) -> Result<Utf8PathBuf> {
    match path.file_stem() {
        Some(stem) => {
            let new_filename: Utf8PathBuf = match path.extension() {
                Some(extension) => format!("{stem}{suffix}.{extension}").into(),
                None => format!("{stem}{suffix}").into(),
            };
            let mut full_path: Utf8PathBuf = path.parent().unwrap_or("".into()).into();
            full_path.push(new_filename);
            Ok(full_path)
        }
        None => Err(eyre!("no file present in provided path {path:?}")),
    }
}

/// Returns path to pdb and verifies it exists, returns None when file does not exist
pub fn determine_pdb_filename(path: &Utf8PathBuf) -> Option<Utf8PathBuf> {
    match path.file_stem() {
        Some(stem) => {
            let new_filename: Utf8PathBuf = format!("{stem}.pdb").into();
            let mut full_path: Utf8PathBuf = path.parent().unwrap_or("".into()).into();
            full_path.push(new_filename);
            if full_path.exists() {
                Some(full_path)
            } else {
                None
            }
        }
        None => None,
    }
}
