use crate::ext::anyhow::{bail, Context, Result};
use camino::Utf8PathBuf;
use std::borrow::Cow;

pub fn os_arch() -> Result<(&'static str, &'static str)> {
    let target_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        bail!("unsupported OS")
    };

    let target_arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        bail!("unsupported target architecture")
    };
    Ok((target_os, target_arch))
}

pub fn is_linux_musl_env() -> bool {
    cfg!(target_os = "linux") && cfg!(target_env = "musl")
}

pub trait StrAdditions {
    fn with(&self, append: &str) -> String;
    fn pad_left_to(&self, len: usize) -> Cow<str>;
    /// returns the string as a canonical path (creates the dir if necessary)
    fn to_created_dir(&self) -> Result<Utf8PathBuf>;
}

impl StrAdditions for str {
    fn with(&self, append: &str) -> String {
        let mut s = self.to_string();
        s.push_str(append);
        s
    }

    fn pad_left_to(&self, len: usize) -> Cow<str> {
        let chars = self.chars().count();
        if chars < len {
            Cow::Owned(format!("{}{self}", " ".repeat(len - chars)))
        } else {
            Cow::Borrowed(self)
        }
    }

    fn to_created_dir(&self) -> Result<Utf8PathBuf> {
        let path = Utf8PathBuf::from(self);
        if !path.exists() {
            std::fs::create_dir_all(&path).context(format!("Could not create dir {self:?}"))?;
        }
        Ok(path)
    }
}

impl StrAdditions for String {
    fn with(&self, append: &str) -> String {
        let mut s = self.clone();
        s.push_str(append);
        s
    }

    fn pad_left_to(&self, len: usize) -> Cow<str> {
        self.as_str().pad_left_to(len)
    }

    fn to_created_dir(&self) -> Result<Utf8PathBuf> {
        self.as_str().to_created_dir()
    }
}
