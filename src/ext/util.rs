use crate::internal_prelude::*;
use camino::Utf8PathBuf;
use clap::builder::styling::{Color, Style};
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

/// Whether the *host* uses linux and musl libc. Best-effort detection at runtime, but
/// falls back to compile time if it fails.
pub fn is_linux_musl_env() -> bool {
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
    #[cfg(target_os = "linux")]
    {
        use std::sync::OnceLock;

        static IS_MUSL: OnceLock<bool> = OnceLock::new();
        *IS_MUSL.get_or_init(|| detect_host_libc_is_musl().unwrap_or(cfg!(target_env = "musl")))
    }
}

#[cfg(target_os = "linux")]
fn detect_host_libc_is_musl() -> Option<bool> {
    use regex::Regex;
    use std::process::Command;
    use std::sync::LazyLock;

    static LIBC_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)(musl)|glibc|gnu libc|gnu c library").unwrap());

    let detect = |stream: &[u8]| {
        let text = String::from_utf8_lossy(stream);
        LIBC_RE.captures(&text).map(|caps| caps.get(1).is_some()) // 'musl' capture group
    };

    let output = Command::new("ldd").arg("--version").output().ok()?;
    detect(&output.stdout).or_else(|| detect(&output.stderr))
}

pub trait StrAdditions {
    fn with(&self, append: &str) -> String;
    fn pad_left_to(&self, len: usize) -> Cow<'_, str>;
    /// returns the string as a canonical path (creates the dir if necessary)
    fn to_created_dir(&self) -> Result<Utf8PathBuf>;
}

impl StrAdditions for str {
    fn with(&self, append: &str) -> String {
        let mut s = self.to_string();
        s.push_str(append);
        s
    }

    fn pad_left_to(&self, len: usize) -> Cow<'_, str> {
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
            std::fs::create_dir_all(&path).wrap_err(format!("Could not create dir {self:?}"))?;
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

    fn pad_left_to(&self, len: usize) -> Cow<'_, str> {
        self.as_str().pad_left_to(len)
    }

    fn to_created_dir(&self) -> Result<Utf8PathBuf> {
        self.as_str().to_created_dir()
    }
}

pub trait Paint {
    fn paint<'a>(self, text: impl Into<Cow<'a, str>>) -> String;
}

impl Paint for Color {
    fn paint<'a>(self, text: impl Into<Cow<'a, str>>) -> String {
        let text = text.into();
        let style = Style::new().fg_color(Some(self));

        format!("{style}{text}{style:#}")
    }
}
