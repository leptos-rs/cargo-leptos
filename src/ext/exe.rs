use crate::{
    ext::anyhow::{bail, Context, Result},
    logger::GRAY,
};
use bytes::Bytes;
use std::{
    fs::{self, File},
    io::{Cursor, Write},
    path::{Path, PathBuf},
};
use zip::ZipArchive;

use super::util::os_arch;

#[cfg(target_family = "unix")]
use std::os::unix::prelude::PermissionsExt;

#[derive(Debug)]
pub struct ExeMeta {
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
        let data = reqwest::get(&self.meta.url).await?.bytes().await?;
        Ok(data)
    }

    fn extract_downloaded(&self, data: &Bytes) -> Result<()> {
        if self.meta.url.ends_with(".zip") {
            extract_zip(data, &self.exe_dir)?;
        } else if self.meta.url.ends_with(".tar.gz") {
            extract_tar(data, &self.exe_dir)?;
        } else {
            self.write_binary(&data)
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
        file.write_all(&data)
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
        std::fs::create_dir_all(&dir).context(format!("Could not create dir {dir:?}"))?;
    }

    Ok(dir)
}

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
        } else {
            if cfg!(feature = "no_downloads") {
                bail!("{} is required but was not found. Please install it using your OS's tool of choice", &meta.name);
            } else {
                meta.cached().await.context(meta.manual)?
            }
        };

        log::debug!(
            "Command using {} {}. {}",
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
                    name: "cargo-generate",
                    version,
                    url,
                    exe,
                    manual: "Try manually installing cargo-generate: https://github.com/cargo-generate/cargo-generate#installation"
                }
            }
            Exe::Sass => {
                let version = "1.58.3";
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
                    name: "wasm-opt",
                    version,
                    url,
                    exe,
                    manual:
                        "Try manually installing binaryen: https://github.com/WebAssembly/binaryen",
                }
            }
            Exe::Tailwind => {
                let version = "v3.2.4";
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
}
