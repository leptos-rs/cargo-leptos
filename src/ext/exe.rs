use crate::{
    ext::anyhow::{bail, Context, Result},
    logger::GRAY,
};
use bytes::Bytes;
use std::{
    fs::{self, File},
    io::{Cursor, Write},
    path::{Path, PathBuf},
    collections::HashMap,
    sync::Once
};

use std::env;

use zip::ZipArchive;

use super::util::{is_linux_musl_env, os_arch};

#[cfg(target_family = "unix")]
use std::os::unix::prelude::PermissionsExt;

use semver::Version;

#[derive(Debug)]
pub struct ExeMeta {
    name: &'static str,
    version: String,
    url: String,
    exe: String,
    manual: &'static str,
}

lazy_static::lazy_static!{
    static ref ON_STARTUP_ONCE: HashMap<Exe, Once> = {
        let mut onces = HashMap::new();
        onces.insert(Exe::CargoGenerate, Once::new());
        onces.insert(Exe::Sass, Once::new());
        onces.insert(Exe::WasmOpt, Once::new());
        onces.insert(Exe::Tailwind, Once::new());
        onces
    };

    static ref ON_STARTUP_DEBUG_ONCE: Once = Once::new();
}

const DEFAULT_CARGO_GENERATE_VERSION: &str = "0.17.3";
const DEFAULT_SASS_VERSION: &str = "1.58.3";
const DEFAULT_WASM_OPT_VERSION: &str = "version_112";
const DEFAULT_TAILWIND_VERSION: &str = "v3.3.3";

pub const ENV_VAR_LEPTOS_CARGO_GENERATE_VERSION: &str = "LEPTOS_CARGO_GENERATE_VERSION";
pub const ENV_VAR_LEPTOS_TAILWIND_VERSION: &str = "LEPTOS_TAILWIND_VERSION";
pub const ENV_VAR_LEPTOS_SASS_VERSION: &str = "LEPTOS_SASS_VERSION";
pub const ENV_VAR_LEPTOS_WASM_OPT_VERSION: &str = "LEPTOS_WASM_OPT_VERSION";


impl ExeMeta {

    #[allow(clippy::wrong_self_convention)]
    fn from_global_path(&self) -> Option<PathBuf> {
        which::which(self.name).ok()
    }

    fn get_name(&self) -> String {
        format!("{}-{}", &self.name, &self.version)
    }

    async fn cached(&self) -> Result<PathBuf> {
        let cache_dir = get_cache_dir()?.join(self.get_name());
        self._with_cache_dir(&cache_dir).await
    }

    async fn _with_cache_dir(&self, cache_dir: &Path) -> Result<PathBuf> {
        let exe_dir = cache_dir.join(self.get_name());
        let c = ExeCache {
            meta: self,
            exe_dir,
        };
        c.get().await
    }

    #[cfg(test)]
    pub async fn with_cache_dir(&self, cache_dir: &Path) -> Result<PathBuf> {
        self._with_cache_dir(cache_dir).await
    }
}

pub struct ExeCache<'a> {
    exe_dir: PathBuf,
    meta: &'a ExeMeta,
}

impl<'a> ExeCache<'a> {
    fn exe_in_cache(&self) -> Result<PathBuf> {
        let exe_path = self.exe_dir.join(PathBuf::from(&self.meta.exe));

        if !exe_path.exists() {
            bail!("The path {exe_path:?} doesn't exist");
        }

        Ok(exe_path)
    }

    async fn fetch_archive(&self) -> Result<Bytes> {
        log::debug!(
            "Install downloading {} {}",
            self.meta.name,
            GRAY.paint(&self.meta.url)
        );

        let response = reqwest::get(&self.meta.url).await?;

        match response.status().is_success() {
            true => Ok(response.bytes().await?),
            false => bail!("Could not download from {}", self.meta.url),
        }
    }

    fn extract_downloaded(&self, data: &Bytes) -> Result<()> {
        if self.meta.url.ends_with(".zip") {
            extract_zip(data, &self.exe_dir)?;
        } else if self.meta.url.ends_with(".tar.gz") {
            extract_tar(data, &self.exe_dir)?;
        } else {
            self.write_binary(data)
                .context(format!("Could not write binary {}", self.meta.get_name()))?;
        }

        log::debug!(
            "Install decompressing {} {}",
            self.meta.name,
            GRAY.paint(self.exe_dir.to_string_lossy())
        );

        Ok(())
    }

    fn write_binary(&self, data: &Bytes) -> Result<()> {
        fs::create_dir_all(&self.exe_dir).unwrap();
        let path = self.exe_dir.join(Path::new(&self.meta.exe));
        let mut file = File::create(&path).unwrap();
        file.write_all(data)
            .context(format!("Error writing binary file: {:?}", path))?;

        #[cfg(target_family = "unix")]
        {
            let mut perm = fs::metadata(&path)?.permissions();
            // https://chmod-calculator.com
            // read and execute for owner and group
            perm.set_mode(0o550);
            fs::set_permissions(&path, perm)?;
        }
        Ok(())
    }

    async fn download(&self) -> Result<PathBuf> {
        log::info!("Command installing {} ...", self.meta.get_name());

        let data = self
            .fetch_archive()
            .await
            .context(format!("Could not download {}", self.meta.get_name()))?;

        self.extract_downloaded(&data)
            .context(format!("Could not extract {}", self.meta.get_name()))?;

        let binary_path = self.exe_in_cache().context(format!(
            "Binary downloaded and extracted but could still not be found at {:?}",
            self.exe_dir
        ))?;
        log::info!("Command {} installed.", self.meta.get_name());
        Ok(binary_path)
    }

    async fn get(&self) -> Result<PathBuf> {
        if let Ok(path) = self.exe_in_cache() {
            Ok(path)
        } else {
            self.download().await
        }
    }
}

// there's a issue in the tar crate: https://github.com/alexcrichton/tar-rs/issues/295
// It doesn't handle TAR sparse extensions, with data ending up in a GNUSparseFile.0 sub-folder
fn extract_tar(src: &Bytes, dest: &Path) -> Result<()> {
    let content = Cursor::new(src);
    let dec = flate2::read::GzDecoder::new(content);
    let mut arch = tar::Archive::new(dec);
    arch.unpack(dest).dot()?;
    Ok(())
}

fn extract_zip(src: &Bytes, dest: &Path) -> Result<()> {
    let content = Cursor::new(src);
    let mut arch = ZipArchive::new(content).dot()?;
    arch.extract(dest).dot().dot()?;
    Ok(())
}

/// Returns the absolute path to app cache directory.
///
/// May return an error when system cache directory does not exist,
/// or when it can not create app specific directory.
///
/// | OS       | Example                            |
/// | -------- | ---------------------------------- |
/// | Linux    | /home/alice/.cache/NAME           |
/// | macOS    | /Users/Alice/Library/Caches/NAME  |
/// | Windows  | C:\Users\Alice\AppData\Local\NAME |
fn get_cache_dir() -> Result<PathBuf> {
    let dir = dirs::cache_dir()
        .ok_or_else(|| anyhow::anyhow!("Cache directory does not exist"))?
        .join("cargo-leptos");

    if !dir.exists() {
        fs::create_dir_all(&dir).context(format!("Could not create dir {dir:?}"))?;
    }

    ON_STARTUP_DEBUG_ONCE.call_once(|| {
        log::debug!("Command cache dir: {}", dir.to_string_lossy());
    });

    Ok(dir)
}

#[derive(Debug, Hash, Eq, PartialEq)]
pub enum Exe {
    CargoGenerate,
    Sass,
    WasmOpt,
    Tailwind,
}

impl Exe {
    pub async fn get(&self) -> Result<PathBuf> {
        let meta = self.meta()?;

        let path = if let Some(path) = meta.from_global_path() {
            path
        } else if cfg!(feature = "no_downloads") {
            bail!("{} is required but was not found. Please install it using your OS's tool of choice", &meta.name);
        } else {
            meta.cached().await.context(meta.manual)?
        };

        log::debug!(
            "Command using {} {} {}",
            &meta.name,
            &meta.version,
            GRAY.paint(path.to_string_lossy())
        );

        Ok(path)
    }

    pub fn meta(&self) -> Result<ExeMeta> {
        let (target_os, target_arch) = os_arch().unwrap();

        let exe = match self {
            Exe::CargoGenerate => {
                // There's a problem with upgrading cargo-generate because the tar file cannot be extracted
                // due to missing support for https://github.com/alexcrichton/tar-rs/pull/298
                // The tar extracts ok, but contains a folder `GNUSparseFile.0` which contains a file `cargo-generate`
                // that has not been fully extracted.
                let version = self.resolve_version();

                let target = match (target_os, target_arch) {
                    ("macos", "aarch64") => "aarch64-apple-darwin",
                    ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
                    ("macos", "x86_64") => "x86_64-apple-darwin",
                    ("windows", "x86_64") => "x86_64-pc-windows-msvc",
                    ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
                    _ => bail!("No cargo-generate tar binary found for {target_os} {target_arch}"),
                };

                let exe = match target_os {
                    "windows" => "cargo-generate.exe".to_string(),
                    _ => "cargo-generate".to_string(),
                };
                let url = format!("https://github.com/cargo-generate/cargo-generate/releases/download/v{version}/cargo-generate-v{version}-{target}.tar.gz");
                ExeMeta {
                    name: "cargo-generate",
                    version,
                    url,
                    exe,
                    manual: "Try manually installing cargo-generate: https://github.com/cargo-generate/cargo-generate#installation"
                }
            }
            Exe::Sass => {
                let version = self.resolve_version();

                let is_musl_env = is_linux_musl_env();
                let url = if is_musl_env {
                    match target_arch {
                        "x86_64" => format!("https://github.com/dart-musl/dart-sass/releases/download/{version}/dart-sass-{version}-linux-x64.tar.gz"),
                        "aarch64" => format!("https://github.com/dart-musl/dart-sass/releases/download/{version}/dart-sass-{version}-linux-arm64.tar.gz"),
                        _ => bail!("No sass tar binary found for linux-musl {target_arch}")
                    }
                } else {
                    match (target_os, target_arch) {
                        ("windows", "x86_64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-windows-x64.zip"),
                        ("macos" | "linux", "x86_64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-{target_os}-x64.tar.gz"),
                        ("macos" | "linux", "aarch64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-{target_os}-arm64.tar.gz"),
                        _ => bail!("No sass tar binary found for {target_os} {target_arch}")
                    }
                };
                let exe = match target_os {
                    "windows" => "dart-sass/sass.bat".to_string(),
                    _ => "dart-sass/sass".to_string(),
                };
                ExeMeta {
                    name: "sass",
                    version,
                    url,
                    exe,
                    manual: "Try manually installing sass: https://sass-lang.com/install",
                }
            }
            Exe::WasmOpt => {
                let version = self.resolve_version();

                let target = match (target_os, target_arch) {
                    ("linux", _) => "x86_64-linux",
                    ("windows", _) => "x86_64-windows",
                    ("macos", "aarch64") => "arm64-macos",
                    ("macos", "x86_64") => "x86_64-macos",
                    _ => {
                        bail!("No wasm-opt tar binary found for {target_os} {target_arch}")
                    }
                };
                let url = format!("https://github.com/WebAssembly/binaryen/releases/download/{version}/binaryen-{version}-{target}.tar.gz");

                let exe = match target_os {
                    "windows" => format!("binaryen-{version}/bin/wasm-opt.exe"),
                    _ => format!("binaryen-{version}/bin/wasm-opt"),
                };
                ExeMeta {
                    name: "wasm-opt",
                    version,
                    url,
                    exe,
                    manual:
                        "Try manually installing binaryen: https://github.com/WebAssembly/binaryen",
                }
            }
            Exe::Tailwind => {
                let version = self.resolve_version();

                let url = match (target_os, target_arch) {
                    ("windows", "x86_64") => format!("https://github.com/tailwindlabs/tailwindcss/releases/download/{version}/tailwindcss-windows-x64.exe"),
                    ("macos", "x86_64") => format!("https://github.com/tailwindlabs/tailwindcss/releases/download/{version}/tailwindcss-macos-x64"),
                    ("macos", "aarch64") => format!("https://github.com/tailwindlabs/tailwindcss/releases/download/{version}/tailwindcss-macos-arm64"),
                    ("linux", "x86_64") => format!("https://github.com/tailwindlabs/tailwindcss/releases/download/{version}/tailwindcss-linux-x64"),
                    ("linux", "aarch64") => format!("https://github.com/tailwindlabs/tailwindcss/releases/download/{version}/tailwindcss-linux-arm64"),
                    _ => bail!("No tailwind binary found for {target_os} {target_arch}")
                };
                let exe = match (target_os, target_arch) {
                    ("windows", _) => "tailwindcss-windows-x64.exe".to_string(),
                    ("macos", "x86_64") => "tailwindcss-macos-x64".to_string(),
                    ("macos", "aarch64") => "tailwindcss-macos-arm64".to_string(),
                    ("linux", "x86_64") => "tailwindcss-linux-x64".to_string(),
                    (_, _) => "tailwindcss-linux-arm64".to_string(),
                };
                ExeMeta {
                    name: "tailwindcss",
                    version,
                    url,
                    exe,
                    manual: "Try manually installing tailwindcss",
                }
            }
        };

        Ok(exe)
    }

    /// Resolve the version of the command.
    /// Always guaranteed to fall back to the default version in case of any errors.
    fn resolve_version(&self) -> String {
        match &self {
            Exe::CargoGenerate => {
                let latch = ON_STARTUP_ONCE.get(self);
                let version = env::var(ENV_VAR_LEPTOS_CARGO_GENERATE_VERSION)
                    .unwrap_or_else(|_| DEFAULT_CARGO_GENERATE_VERSION.into());

                if let Some(latch) = latch {
                    latch.call_once(|| {
                        log::debug!("Command version for Cargo Generate resolved to {}", version);
                    })
                };

                version
            },
            Exe::Sass => {
                let latch = ON_STARTUP_ONCE.get(self);
                let version = env::var(ENV_VAR_LEPTOS_SASS_VERSION)
                    .unwrap_or_else(|_| DEFAULT_SASS_VERSION.into());

                if let Some(latch) = latch {
                    latch.call_once(|| {
                        log::debug!("Command version for Sass resolved to {}", version);
                    })
                }

                version
            },
            Exe::WasmOpt => {
                let latch = ON_STARTUP_ONCE.get(self);
                let version = env::var(ENV_VAR_LEPTOS_WASM_OPT_VERSION)
                    .unwrap_or_else(|_| DEFAULT_WASM_OPT_VERSION.into());

                if let Some(latch) = latch {
                    latch.call_once(|| {
                        log::debug!("Command version for WASM Optimizer resolved to {}", version);
                    })
                }

                version
            },
            Exe::Tailwind => {
                let latch = ON_STARTUP_ONCE.get(self);
                let version = env::var(ENV_VAR_LEPTOS_TAILWIND_VERSION)
                    .unwrap_or_else(|_| DEFAULT_TAILWIND_VERSION.into());

                if let Some(latch) = latch {
                    latch.call_once(|| {
                        log::debug!("Command version for Tailwind resolved to {}", version);
                    })
                }
                version
            },
            // _ => bail!("Unknown command"),
        }
    }

    /// Tailwind uses the 'vMaj.Min.Pat' format.
    /// WASM opt uses 'version_NNN' format.
    /// We generally want to keep the suffix intact,
    /// as it carries classifiers, etc, but strip non-ascii
    /// digits from the prefix.
    #[inline]
    fn sanitize_version_prefix(ver_string: &str) -> String {
        ver_string.chars().skip_while(|c| !c.is_ascii_digit() || *c == '_').collect::<String>()
    }

    /// Attempts to convert a non-semver version string to a semver one.
    /// E.g. WASM Opt uses `version_112`, which is not semver even if
    /// we strip the prefix.
    ///
    /// # Example
    ///
    /// ```
    /// let version = normalize_version("version_112");
    /// assert_eq!(version, Some("112.0.0".to_string()));
    /// ```
    fn normalize_version(ver_string: &str) -> Option<Version> {
        let ver_string = Self::sanitize_version_prefix(ver_string);
        match Version::parse(&ver_string) {
            Ok(v) => Some(v),
            Err(_) => {
                match &ver_string.parse::<u64>() {
                    Ok(num) => Some(Version::new(*num, 0, 0)),
                    Err(_) => {
                        match Version::parse(format!("{ver_string}.0").as_str()) {
                            Ok(v) => Some(v),
                            Err(e) => {
                                log::error!("Failed to normalize version {ver_string}: {e}");
                                None
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use cargo_metadata::semver::Version;
    use super::*;

    #[test]
    fn test_sanitize_version_prefix() {

        let version = Exe::sanitize_version_prefix("v1.2.3");
        assert_eq!(version, "1.2.3");
        assert!(Version::parse(&version).is_ok());
        let version = Exe::sanitize_version_prefix("version_1.2.3");
        assert_eq!(version, "1.2.3");
        assert!(Version::parse(&version).is_ok());
    }

    #[test]
    fn test_normalize_version() {
        let version = Exe::normalize_version("version_112");
        assert!(version.is_some_and(|v| {
            v.major == 112 && v.minor == 0 && v.patch == 0
        }));

        let version = Exe::normalize_version("v3.3.3");
        assert!(version.is_some_and(|v| {
            v.major == 3 && v.minor == 3 && v.patch == 3
        }));

        let version = Exe::normalize_version("10.0.0");
        assert!(version.is_some_and(|v| {
            v.major == 10 && v.minor == 0 && v.patch == 0
        }));
    }

    #[test]
    fn test_incomplete_version_strings() {
        let version = Exe::normalize_version("5");
        assert!(version.is_some_and(|v| {
            v.major == 5 && v.minor == 0 && v.patch == 0
        }));

        let version = Exe::normalize_version("0.2");
        assert!(version.is_some_and(|v| {
            v.major == 0 && v.minor == 2 && v.patch == 0
        }));
    }

    #[test]
    fn test_invalid_versions() {
        let version = Exe::normalize_version("1a-test");
        assert_eq!(version, None);
    }
}
