use crate::{
    ext::anyhow::{bail, Result},
    logger::GRAY,
};
use anyhow::Context;
use axum::body::Bytes;
use decompress::decompressors;
use regex::Regex;
use std::{fs, io::Cursor, path::PathBuf};

use super::util::os_arch;

lazy_static::lazy_static! {
    static ref CACHE_DIR: PathBuf = get_cache_dir("cargo-leptos").expect("Can not get cache directory");
}

pub enum Exe {
    CargoGenerate,
    Sass,
    WasmOpt,
}

struct ExeMeta {
    name: String,
    version: String,
    get_exe_archive_url: fn(version: &str, target_os: &str, target_arch: &str) -> Result<String>,
    get_exe_name: fn(target_os: &str) -> String,
}

impl ExeMeta {
    fn exe_in_global_path(&self) -> Option<PathBuf> {
        which::which(&self.name).ok()
    }

    fn get_name(&self) -> String {
        format!("{}-{}", &self.name, &self.version)
    }

    /// Returns an absolute path to be used for the binary.
    fn get_exe_dir_path(&self) -> PathBuf {
        CACHE_DIR.join(self.get_name())
    }

    fn exe_in_cache(&self) -> Option<PathBuf> {
        let (target_os, _) = os_arch().unwrap();

        if !self.get_exe_dir_path().exists() {
            return None;
        }

        let exe_name = &self.get_exe_name;
        let exe_name = exe_name(target_os);

        let exe_path = self.get_exe_dir_path().join(PathBuf::from(exe_name));

        if !exe_path.exists() {
            return None;
        }

        Some(exe_path)
    }

    fn get_download_url(&self) -> String {
        let (target_os, target_arch) = os_arch().unwrap();
        let url = &self.get_exe_archive_url;
        url(&self.version, target_os, target_arch).unwrap()
    }

    async fn fetch_archive(&self) -> Result<Bytes> {
        let url = self.get_download_url();
        log::debug!("Install downloading {} {}", self.name, GRAY.paint(&url));
        let data = reqwest::get(url).await?.bytes().await?;
        Ok(data)
    }

    fn extract_archive(&self, data: &Bytes) -> Result<()> {
        let name = self.get_name();

        let url = self.get_download_url();

        let ext = match url {
            url if url.ends_with(".tar.gz") => "tar.gz",
            url if url.ends_with(".zip") => "zip",
            _ => bail!("The download URL does not contain either '.tar.gz' or '.zip' extension"),
        };

        let temp_file_path = CACHE_DIR.join(format!("tmp-{}.{}", name, ext));
        let dest_dir = &self.get_exe_dir_path();

        let mut temp_file = fs::File::create(&temp_file_path)
            .context(format!("Could not create file {temp_file_path:?}"))?;

        let mut content = Cursor::new(data);
        std::io::copy(&mut content, &mut temp_file)?;

        let extractor = decompress::Decompress::build(vec![
            decompressors::zip::Zip::build(Some(Regex::new(r".*\.zip").unwrap())),
            decompressors::targz::Targz::build(Some(Regex::new(r".*\.tar\.gz").unwrap())),
        ]);

        log::debug!(
            "Install decompressing {} {}",
            self.name,
            GRAY.paint(temp_file_path.to_string_lossy())
        );

        extractor
            .decompress(
                &temp_file_path,
                dest_dir,
                &decompress::ExtractOpts { strip: 1 },
            )
            .context(format!(
                "Could not decompress {temp_file_path:?} to {dest_dir:?}"
            ))?;

        // Rename the root executable directory
        fs::remove_file(&temp_file_path)
            .context(format!("Could not remove file {temp_file_path:?}"))?;

        Ok(())
    }

    async fn download_exe(&self) -> Result<PathBuf> {
        log::info!("Command installing {} ...", self.get_name());

        let data = self
            .fetch_archive()
            .await
            .context(format!("Could not download {}", self.get_name()))?;
        self.extract_archive(&data)
            .context(format!("Could not extract {}", self.get_name()))?;

        if let Some(binary_path) = self.exe_in_cache() {
            log::info!("Command {} installed.", self.get_name());
            Ok(binary_path)
        } else {
            bail!(
                "Binary downloaded and extracted but could still not be found at {:?}",
                self.get_exe_dir_path()
            )
        }
    }
}

pub async fn get_exe(app: Exe) -> Result<PathBuf> {
    let exe = get_executable(app).unwrap();

    let path = if let Some(path) = exe.exe_in_global_path() {
        path
    } else if let Some(path) = exe.exe_in_cache() {
        path
    } else {
        exe.download_exe().await?
    };

    log::debug!(
        "Command using {} {}. {}",
        exe.name,
        exe.version,
        GRAY.paint(path.to_string_lossy())
    );

    Ok(path)
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
fn get_cache_dir(name: &str) -> Result<PathBuf> {
    let os_cache_dir = match dirs::cache_dir() {
        Some(d) => d,
        None => bail!("Cache directory does not exist"),
    };

    let cache_dir = os_cache_dir.join(name);

    if !cache_dir.exists() {
        fs::create_dir(&cache_dir).context(format!("Could not create dir {cache_dir:?}"))?;
    }

    Ok(cache_dir)
}

fn get_executable(app: Exe) -> Result<ExeMeta> {
    let exe = match app {
        Exe::CargoGenerate => ExeMeta {
            name: "cargo-generate".to_string(),
            version: "0.17.4".to_string(),
            get_exe_archive_url: |version, target_os, target_arch| {
                let target = match (target_os, target_arch) {
                    ("macos", "aarch64") => "aarch64-apple-darwin",
                    ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
                    ("macos", "x86_64") => "x86_64-apple-darwin",
                    ("windows", "x86_64") => "x86_64-pc-windows-msvc",
                    ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
                    _ => bail!("No cargo-generate tar binary found for {target_os} {target_arch}"),
                };

                let url = format!("https://github.com/cargo-generate/cargo-generate/releases/download/v{version}/cargo-generate-v{version}-{target}.tar.gz");
                Ok(url)
            },
            get_exe_name: |target_os| match target_os {
                "windows" => "cargo-generate.exe".to_string(),
                _ => "cargo-generate".to_string(),
            },
        },
        Exe::Sass => ExeMeta {
            name: "sass".to_string(),
            version: "1.56.2".to_string(),
            get_exe_archive_url: |version, target_os, target_arch| {
                let url = match (target_os, target_arch) {
                    ("windows", "x86_64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-windows-x64.zip"),
                    ("macos" | "linux", "x86_64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-{target_os}-x64.tar.gz"),
                    ("macos" | "linux", "aarch64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-{target_os}-arm64.tar.gz"),
                    _ => bail!("No sass tar binary found for {target_os} {target_arch}")
                };
                Ok(url)
            },
            get_exe_name: |target_os| match target_os {
                "windows" => "sass.bat".to_string(),
                _ => "sass".to_string(),
            },
        },
        Exe::WasmOpt => ExeMeta {
            name: "wasm-opt".to_string(),
            version: "version_111".to_string(),
            get_exe_archive_url: |version, target_os, target_arch| {
                let target = match (target_os, target_arch) {
                    ("linux", _) => "x86_64-linux",
                    ("windows", _) => "x86_64-windows",
                    ("macos", "aarch64") => "arm64-macos",
                    ("macos", "x86_64") => "x86_64-macos",
                    _ => bail!("No wasm-opt tar binary found for {target_os} {target_arch}"),
                };
                let url = format!("https://github.com/WebAssembly/binaryen/releases/download/{version}/binaryen-{version}-{target}.tar.gz");
                Ok(url)
            },
            get_exe_name: |target_os| match target_os {
                "windows" => "bin/wasm-opt.exe".to_string(),
                _ => "bin/wasm-opt".to_string(),
            },
        },
    };

    Ok(exe)
}
