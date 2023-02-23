use crate::{
    ext::anyhow::{bail, Context, Result},
    logger::GRAY,
};
use axum::body::Bytes;
use std::{
    io::Cursor,
    path::{Path, PathBuf},
};
use zip::ZipArchive;

use super::util::os_arch;

#[derive(Debug)]
pub struct ExeMeta {
    cache_dir: Option<PathBuf>,
    name: &'static str,
    version: &'static str,
    url: String,
    exe: String,
    manual: &'static str,
}

impl ExeMeta {
    fn from_global_path(&self) -> Option<PathBuf> {
        which::which(&self.name).ok()
    }

    fn get_name(&self) -> String {
        format!("{}-{}", &self.name, &self.version)
    }

    /// Returns an absolute path to be used for the binary.
    fn get_exe_dir_path(&self) -> Option<PathBuf> {
        if let Some(cache_dir) = &self.cache_dir {
            Some(cache_dir.join(self.get_name()))
        } else {
            None
        }
    }

    fn exe_in_cache(&self) -> Result<Option<PathBuf>> {
        let exe_path = match self.get_exe_dir_path() {
            Some(dir_path) => dir_path.join(PathBuf::from(&self.exe)),
            None => return Ok(None),
        };
        if !exe_path.exists() {
            bail!("The path {exe_path:?} doesn't exist");
        }

        Ok(Some(exe_path))
    }

    async fn fetch_archive(&self) -> Result<Bytes> {
        log::debug!(
            "Install downloading {} {}",
            self.name,
            GRAY.paint(&self.url)
        );
        let data = reqwest::get(&self.url).await?.bytes().await?;
        Ok(data)
    }

    fn extract_archive(&self, data: &Bytes) -> Result<()> {
        let dest_dir = &self.get_exe_dir_path();
        if let Some(dest_dir) = dest_dir {
            if self.url.ends_with(".zip") {
                extract_zip(data, &dest_dir)?;
            } else if self.url.ends_with(".tar.gz") {
                extract_tar(data, &dest_dir)?;
            } else {
                bail!("The download URL does not contain either '.tar.gz' or '.zip' extension");
            }

            log::debug!(
                "Install decompressing {} {}",
                self.name,
                GRAY.paint(dest_dir.to_string_lossy())
            );
        }

        Ok(())
    }

    async fn download(&self) -> Result<PathBuf> {
        log::info!("Command installing {} ...", self.get_name());
        let data = self
            .fetch_archive()
            .await
            .context(format!("Could not download {}", self.get_name()))?;
        self.extract_archive(&data)
            .context(format!("Could not extract {}", self.get_name()))?;

        let binary_path = self.exe_in_cache().context(format!(
            "Binary downloaded and extracted but could still not be found at {:?}",
            self.get_exe_dir_path()
        ))?;
        log::info!("Command {} installed.", self.get_name());

        let Some(binary_path) = binary_path else{
        bail!("No cache folder to store the download in. Something has gone wrong.")
        };

        Ok(binary_path)
    }

    pub async fn from_cache(&self) -> Result<PathBuf> {
        if let Ok(Some(path)) = self.exe_in_cache() {
            Ok(path)
        } else {
            if cfg!(feature = "no_downloads") {
                bail!("{} is required but was not found. Please install it using your OS's tool of choice", &self.name);
            }
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

/// Returns the absolute path to app cache directory, if it exists.
///
/// May return an error when system cache directory does not exist,
/// or when it can not create app specific directory.
///
/// | OS       | Example                            |
/// | -------- | ---------------------------------- |
/// | Linux    | /home/alice/.cache/NAME           |
/// | macOS    | /Users/Alice/Library/Caches/NAME  |
/// | Windows  | C:\Users\Alice\AppData\Local\NAME |
fn get_cache_dir(name: &str) -> Result<Option<PathBuf>> {
    // If the no_downloads feature is enabled, we don't need a cache dir
    if cfg!(feature = "no_downloads") {
        return Ok(None);
    }
    // This cache dir is looking for the system cache dir set by the dir crate
    let dir = dirs::cache_dir()
        .ok_or_else(|| anyhow::anyhow!("Cache directory does not exist"))?
        .join(name);
    if !dir.exists() {
        std::fs::create_dir_all(&dir).context(format!("Could not create dir {dir:?}"))?;
    }

    Ok(Some(dir))
}

pub enum Exe {
    CargoGenerate,
    Sass,
    WasmOpt,
}

impl Exe {
    pub async fn get(&self) -> Result<PathBuf> {
        let exe = self.meta()?;
        let path = match exe.from_global_path() {
            Some(p) => p,
            None => exe.from_cache().await.context(exe.manual)?,
        };

        log::debug!(
            "Command using {} {}. {}",
            exe.name,
            exe.version,
            GRAY.paint(path.to_string_lossy())
        );

        Ok(path)
    }

    pub fn meta_with_dir(&self, cache_dir: Option<PathBuf>) -> Result<ExeMeta> {
        let (target_os, target_arch) = os_arch().unwrap();

        let exe = match self {
            Exe::CargoGenerate => {
                let version = "0.17.3";

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
                    cache_dir: cache_dir.clone(),
                    name: "cargo-generate",
                    version,
                    url,
                    exe,
                    manual: "Try manually installing cargo-generate: https://github.com/cargo-generate/cargo-generate#installation"
                }
            }
            Exe::Sass => {
                let version = "1.58.0";
                let url = match (target_os, target_arch) {
                    ("windows", "x86_64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-windows-x64.zip"),
                    ("macos" | "linux", "x86_64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-{target_os}-x64.tar.gz"),
                    ("macos" | "linux", "aarch64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-{target_os}-arm64.tar.gz"),
                    _ => bail!("No sass tar binary found for {target_os} {target_arch}")
                };
                let exe = match target_os {
                    "windows" => "dart-sass/sass.bat".to_string(),
                    _ => "dart-sass/sass".to_string(),
                };

                ExeMeta {
                    cache_dir: cache_dir.clone(),
                    name: "sass",
                    version,
                    url,
                    exe,
                    manual: "Try manually installing sass: https://sass-lang.com/install",
                }
            }
            Exe::WasmOpt => {
                let version = "version_112";
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
                    cache_dir: cache_dir.clone(),
                    name: "wasm-opt",
                    version,
                    url,
                    exe,
                    manual:
                        "Try manually installing binaryen: https://github.com/WebAssembly/binaryen",
                }
            }
        };

        Ok(exe)
    }

    pub fn meta(&self) -> Result<ExeMeta> {
        let cache_dir = get_cache_dir("cargo-leptos").expect("Can not get cache directory");
        self.meta_with_dir(cache_dir)
    }
}
