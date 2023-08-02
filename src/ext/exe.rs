use crate::{
    ext::anyhow::{bail, Context, Result},
    logger::GRAY,
};
use bytes::Bytes;
use std::{
    fs::{self, File},
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::Once,
};

use std::env;

use zip::ZipArchive;

use super::util::{is_linux_musl_env, os_arch};

#[cfg(target_family = "unix")]
use std::os::unix::prelude::PermissionsExt;
use std::time::{Duration, SystemTime};
use reqwest::ClientBuilder;

use semver::{Version};

#[derive(Debug)]
pub struct ExeMeta {
    name: &'static str,
    version: String,
    url: String,
    exe: String,
    manual: String,
}

lazy_static::lazy_static!{
    static ref ON_STARTUP_DEBUG_ONCE: Once = Once::new();
}

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
        let meta = self.meta().await?;

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

    pub async fn meta(&self) -> Result<ExeMeta> {
        let (target_os, target_arch) = os_arch().unwrap();

        let exe = match self {
            // There's a problem with upgrading cargo-generate because the tar file cannot be extracted
            // due to missing support for https://github.com/alexcrichton/tar-rs/pull/298
            // The tar extracts ok, but contains a folder `GNUSparseFile.0` which contains a file `cargo-generate`
            // that has not been fully extracted.
            // let command = &CommandCargoGenerate as &dyn Command;
            Exe::CargoGenerate => CommandCargoGenerate.exe_meta(target_os, target_arch).await.dot()?,
            Exe::Sass => CommandSass.exe_meta(target_os, target_arch).await.dot()?,
            Exe::WasmOpt => CommandWasmOpt.exe_meta(target_os, target_arch).await.dot()?,
            Exe::Tailwind => CommandTailwind.exe_meta(target_os, target_arch).await.dot()?,
        };

        Ok(exe)
    }
}

/// Tailwind uses the 'vMaj.Min.Pat' format.
/// WASM opt uses 'version_NNN' format.
/// Cargo-generate has the 'vX.Y.Z' format
/// We generally want to keep the suffix intact,
/// as it carries classifiers, etc, but strip non-ascii
/// digits from the prefix.
#[inline]
fn sanitize_version_prefix(ver_string: &str) -> String {
    ver_string.chars().skip_while(|c| !c.is_ascii_digit() || *c == '_').collect::<String>()
}

/// Attempts to convert a non-semver version string to a semver one.
/// E.g. WASM Opt uses `version_112`, which is not semver even if
/// we strip the prefix, treat it as `112.0.0`
fn normalize_version( ver_string: &str) -> Option<Version> {
    let ver_string = sanitize_version_prefix(ver_string);
    match Version::parse(&ver_string) {
        Ok(v) => Some(v),
        Err(_) => {
            match &ver_string.parse::<u64>() {
                Ok(num) => Some(Version::new(*num, 0, 0)),
                Err(_) => {
                    match Version::parse(format!("{ver_string}.0").as_str()) {
                        Ok(v) => Some(v),
                        Err(e) => {
                            log::error!("Command failed to normalize version {ver_string}: {e}");
                            None
                        }
                    }
                }
            }
        }
    }
}


// fallback to this crate until rust stable includes async traits
// https://github.com/dtolnay/async-trait
use async_trait::async_trait;

struct CommandTailwind;
struct CommandWasmOpt;
struct CommandSass;
struct CommandCargoGenerate;

#[async_trait]
impl Command for CommandTailwind {
    fn name(&self) -> &'static str { "tailwindcss" }
    fn default_version(&self) -> &'static str { "v3.3.3" }
    fn env_var_version_name(&self) -> &'static str { ENV_VAR_LEPTOS_TAILWIND_VERSION }
    fn github_owner(&self) -> &'static str { "tailwindlabs" }
    fn github_repo(&self) -> &'static str { "tailwindcss" }

    /// Tool binary download url for the given OS and platform arch
    fn download_url(&self, target_os: &str, target_arch: &str, version: &str) -> Result<String> {
        match (target_os, target_arch) {
            ("windows", "x86_64") => Ok(format!("https://github.com/{}/{}/releases/download/{}/{}-windows-x64.exe",
                                            self.github_owner(), self.github_repo(), version, self.name())),
            ("macos", "x86_64") => Ok(format!("https://github.com/{}/{}/releases/download/{}/{}-macos-x64",
                                           self.github_owner(), self.github_repo(), version, self.name())),
            ("macos", "aarch64") => Ok(format!("https://github.com/{}/{}/releases/download/{}/{}-macos-arm64",
                                            self.github_owner(), self.github_repo(), version, self.name())),
            ("linux", "x86_64") => Ok(format!("https://github.com/{}/{}/releases/download/{}/{}-linux-x64",
                                           self.github_owner(), self.github_repo(), version, self.name())),
            ("linux", "aarch64") => Ok(format!("https://github.com/{}/{}/releases/download/{}/{}-linux-arm64",
                                            self.github_owner(), self.github_repo(), version, self.name())),
            _ => bail!("Command [{}] failed to find a match for {}-{} ", self.name(), target_os, target_arch),
        }
    }

    fn executable_name(&self, target_os: &str, target_arch: &str, _version: Option<&str>) -> Result<String> {
        Ok(match (target_os, target_arch) {
            ("windows", _) => format!("{}-windows-x64.exe", self.name()),
            ("macos", "x86_64") => format!("{}-macos-x64", self.name()),
            ("macos", "aarch64") => format!("{}-macos-arm64", self.name()),
            ("linux", "x86_64") => format!("{}-linux-x64", self.name()),
            (_, _) => format!("{}-linux-arm64", self.name()),
        })
    }

    fn manual_install_instructions(&self) -> String {
        "Try manually installing tailwindcss: https://tailwindcss.com/docs/installation".to_string()
    }
}

#[async_trait]
impl Command for CommandWasmOpt {
    fn name(&self) -> &'static str { "wasm-opt" }
    fn default_version(&self) -> &'static str { "version_112" }
    fn env_var_version_name(&self) -> &'static str { ENV_VAR_LEPTOS_WASM_OPT_VERSION }
    fn github_owner(&self) -> &'static str { "WebAssembly" }
    fn github_repo(&self) -> &'static str { "binaryen" }

    fn download_url(&self, target_os: &str, target_arch: &str, version: &str) -> Result<String> {
        let target = match (target_os, target_arch) {
            ("linux", _) => "x86_64-linux",
            ("windows", _) => "x86_64-windows",
            ("macos", "aarch64") => "arm64-macos",
            ("macos", "x86_64") => "x86_64-macos",
            _ => {
                bail!("No wasm-opt tar binary found for {target_os} {target_arch}")
            }
        };

        Ok(format!(
            "https://github.com/{}/{}/releases/download/{}/binaryen-{}-{}.tar.gz",
            self.github_owner(),
            self.github_repo(),
            version,
            version,
            target)
        )
    }

    fn executable_name(&self, target_os: &str, _target_arch: &str, version: Option<&str>) -> Result<String> {
        if version.is_none() { bail!("Version is required for WASM Opt, none provided")};

        Ok(match target_os {
            "windows" => format!("binaryen-{}/bin/{}.exe", version.unwrap_or_default(), self.name()),
            _ => format!("binaryen-{}/bin/{}", version.unwrap_or_default(), self.name()),
        })
    }

    fn manual_install_instructions(&self) -> String {
        "Try manually installing binaryen: https://github.com/WebAssembly/binaryen".to_string()
    }
}

#[async_trait]
impl Command for CommandSass {
    fn name(&self) -> &'static str { "sass" }
    fn default_version(&self) -> &'static str { "1.58.3" }
    fn env_var_version_name(&self) -> &'static str { ENV_VAR_LEPTOS_SASS_VERSION }
    fn github_owner(&self) -> &'static str { "dart-musl" }
    fn github_repo(&self) -> &'static str { "dart-sass" }

    fn download_url(&self, target_os: &str, target_arch: &str, version: &str) -> Result<String> {
        let is_musl_env = is_linux_musl_env();
        Ok(if is_musl_env {
            match target_arch {
                "x86_64" => format!(
                    "https://github.com/{}/{}/releases/download/{}/dart-sass-{}-linux-x64.tar.gz",
                    self.github_owner(), self.github_repo(), version, version
                ),
                "aarch64" => format!(
                    "https://github.com/{}/{}/releases/download/{}/dart-sass-{}-linux-arm64.tar.gz"
                    , self.github_owner(), self.github_repo(), version, version
                ),
                _ => bail!("No sass tar binary found for linux-musl {target_arch}")
            }
        } else {
            match (target_os, target_arch) {
                // note the different github_owner
                ("windows", "x86_64") => format!(
                    "https://github.com/sass/{}/releases/download/{}/dart-sass-{}-windows-x64.zip",
                    self.github_repo(), version, version
                ),
                ("macos" | "linux", "x86_64") => format!(
                    "https://github.com/sass/{}/releases/download/{}/dart-sass-{}-{}-x64.tar.gz",
                    self.github_repo(), version, version, target_os
                ),
                ("macos" | "linux", "aarch64") => format!(
                    "https://github.com/sass/{}/releases/download/{}/dart-sass-{}-{}-arm64.tar.gz",
                    self.github_repo(), version, version, target_os
                ),
                _ => bail!("No sass tar binary found for {target_os} {target_arch}")
            }
        })
    }

    fn executable_name(&self, target_os: &str, _target_arch: &str, _version: Option<&str>) -> Result<String> {
        Ok(match target_os {
            "windows" => "dart-sass/sass.bat".to_string(),
            _ => "dart-sass/sass".to_string(),
        })
    }

    fn manual_install_instructions(&self) -> String {
        "Try manually installing sass: https://sass-lang.com/install".to_string()
    }
}

#[async_trait]
impl Command for CommandCargoGenerate {
    fn name(&self) -> &'static str {"cargo-generate"}
    fn default_version(&self) -> &'static str { "v0.17.3" }
    fn env_var_version_name(&self) -> &'static str { ENV_VAR_LEPTOS_CARGO_GENERATE_VERSION }
    fn github_owner(&self) -> &'static str { "cargo-generate" }
    fn github_repo(&self) -> &'static str { "cargo-generate" }

    fn download_url(&self, target_os: &str, target_arch: &str, version: &str) -> Result<String> {
        let is_musl_env = is_linux_musl_env();
        
        let target = if is_musl_env {
            match (target_os, target_arch) {
                ("linux", "aarch64") => "aarch64-unknown-linux-musl",
                ("linux", "x86_64") => "x86_64-unknown-linux-musl",
                _ => bail!("No cargo-generate tar binary found for linux-musl {target_arch}"),
            }
        } else {
            match (target_os, target_arch) {
                ("macos", "aarch64") => "aarch64-apple-darwin",
                ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
                ("macos", "x86_64") => "x86_64-apple-darwin",
                ("windows", "x86_64") => "x86_64-pc-windows-msvc",
                ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
                _ => bail!("No cargo-generate tar binary found for {target_os} {target_arch}"),
            }
        };

        Ok(format!(
            "https://github.com/{}/{}/releases/download/{}/cargo-generate-{}-{}.tar.gz",
            self.github_owner(),
            self.github_repo(),
            version, version,
            target
        ))
    }

    fn executable_name(&self, target_os: &str, _target_arch: &str, _version: Option<&str>) -> Result<String> {
        Ok(match target_os {
            "windows" => "cargo-generate.exe".to_string(),
            _ => "cargo-generate".to_string(),
        })
    }

    fn manual_install_instructions(&self) -> String {
        "Try manually installing cargo-generate: https://github.com/cargo-generate/cargo-generate#installation".to_string()
    }
}

#[async_trait]
/// Template trait, implementors should only fill in
/// the command-specific logic. Handles caching, latest
/// version checking against the GitHub API and env var
/// version override for a given command.
trait Command {
    fn name(&self) -> &'static str;
    fn default_version(&self) -> &str;
    fn env_var_version_name(&self) -> &str;
    fn github_owner(&self) -> &str;
    fn github_repo(&self) -> &str;
    fn download_url(&self, target_os: &str, target_arch: &str, version: &str) -> Result<String>;
    fn executable_name(&self, target_os: &str, target_arch: &str, version: Option<&str>) -> Result<String>;
    #[allow(unused)]
    fn manual_install_instructions(&self) -> String {
        // default placeholder text, individual commands can override and customize
        "Try manually installing the command".to_string()
    }

    /// Resolves and creates command metadata.
    /// Checks if a newer version of the binary is available (once a day).
    /// A marker file is created in the cache directory. Add `-v` flag to
    /// the `cargo leptos` command to see the OS-specific location.
    ///
    /// # Arguments
    ///
    /// * `target_os` - The target operating system.
    /// * `target_arch` - The target architecture.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the `ExeMeta` struct on success, or an error on failure.
    ///
    async fn exe_meta(&self, target_os: &str, target_arch: &str) -> Result<ExeMeta> {
        let version = self.resolve_version().await;
        let url = self.download_url(target_os, target_arch, version.as_str())?;
        let exe = self.executable_name(target_os, target_arch, Some(version.as_str()))?;
        Ok(ExeMeta {
            name: self.name(),
            version,
            url: url.to_owned(),
            exe: exe.to_string(),
            manual: self.manual_install_instructions(),
        })
    }

    /// Returns true if the command should check for a new version
    /// Returns false in case of any errors (no check)
    async fn should_check_for_new_version(&self) -> bool {
        match get_cache_dir() {
            Ok(dir) => {
                let marker = dir.join(format!(".{}_last_checked", self.name()));
                return match (marker.exists(), marker.is_dir()) {
                    (_, true) => { // conflicting dir instead of a marker file, bail
                        log::warn!("Command [{}] encountered a conflicting dir in the cache, please delete {}",
                            self.name(), marker.display());

                            false
                    },
                    (true, _) => { // existing marker file, read and check if last checked > 1 DAY
                        let contents = tokio::fs::read_to_string(&marker).await;
                        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH);
                        if let Some(timestamp) =
                            contents.ok()
                                .map(|s| s.parse::<u64>().ok().unwrap_or_default()) {
                            let last_checked = Duration::from_millis(timestamp);
                            let one_day = Duration::from_secs(24 * 60 * 60);
                            if let Ok(now) = now {
                                match (now - last_checked) > one_day {
                                    true => tokio::fs::write(&marker, now.as_millis().to_string()).await.is_ok(),
                                    false => false,
                                }
                            } else { false }
                        } else { false }
                    },
                    (false, _) => { // no marker file yet, record and hint to check
                        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH);
                        return if let Ok(unix_timestamp) = now {
                            tokio::fs::write(marker, unix_timestamp.as_millis().to_string()).await.is_ok()
                        } else {
                            false
                        }
                    }
                }
            },
            Err(e) => {
                log::warn!("Command {} failed to get cache dir: {}", self.name(), e);
                false
            }
        }
    }


    async fn check_for_latest_version(&self) -> Option<String> {
        log::debug!("Command [{}] checking for the latest available version", self.name());

        let client = ClientBuilder::default()
            // this github api allows anonymous, but requires a user-agent header be set
            .user_agent("cargo-leptos")
            .build()
            .unwrap_or_default();

        if let Ok(response) = client.get(
            format!("https://api.github.com/repos/{}/{}/releases/latest", self.github_owner(), self.github_repo()))
            .send().await {

            if !response.status().is_success() {
                log::error!("Command [{}] GitHub API request failed: {}", self.name(), response.status());
                return None
            }

            #[derive(serde::Deserialize)]
            struct Github {
                tag_name: String, // this is the version number, not the git tag
            }

            let github: Github = match response.json().await {
                Ok(json) => json,
                Err(e) => {
                    log::debug!("Command [{}] failed to parse the response JSON from the GitHub API: {}", self.name(), e);
                    return None
                }
            };

            Some(github.tag_name)
        } else {
            log::debug!("Command [{}] failed to check for the latest version", self.name());
            None
        }
    }

    /// get the latest version from github api
    /// cache the last check timestamp
    /// compare with the currently requested version
    /// inform a user if a more recent compatible version is available
    async fn resolve_version(&self) -> String {
        // TODO revisit this logic when implementing the SemVer compatible ranges matching
        // if env var is set, use the requested version and bypass caching logic
        let is_force_pin_version = env::var(self.env_var_version_name()).is_ok();
        log::trace!("Command [{}] is_force_pin_version: {} - {:?}",
            self.name(), is_force_pin_version, env::var(self.env_var_version_name()));

        if !is_force_pin_version && !self.should_check_for_new_version().await {
            log::trace!("Command [{}] NOT checking for the latest available version", &self.name());
            return self.default_version().into();
        }

        let version =
            env::var(self.env_var_version_name())
                .unwrap_or_else(|_| self.default_version().into()).to_owned();

        let latest = self.check_for_latest_version().await;

        match latest {
            Some(latest) => {
                let norm_latest = normalize_version(latest.as_str());
                let norm_version = normalize_version(&version);
                if norm_latest.is_some() && norm_version.is_some() {
                    // TODO use the VersionReq for semantic matching
                    match norm_version.cmp(&norm_latest) {
                        core::cmp::Ordering::Greater | core::cmp::Ordering::Equal => {
                            log::debug!(
                                            "Command [{}] requested version {} is already same or newer than available version {}",
                                            self.name(), version, &latest)
                        },
                        core::cmp::Ordering::Less => {
                            log::info!(
                                            "Command [{}] requested version {}, but a newer version {} is available, you can try it out by \
                                            setting the {}={} env var and re-running the command",
                                            self.name(), version, &latest, self.env_var_version_name(), &latest)
                        }
                    }
                }
            }
            None => log::warn!("Command [{}] failed to check for the latest version", self.name())
        }

        version
    }
}

#[cfg(test)]
mod tests {
    use cargo_metadata::semver::Version;
    use super::*;

    #[test]
    fn test_sanitize_version_prefix() {
        let version = sanitize_version_prefix("v1.2.3");
        assert_eq!(version, "1.2.3");
        assert!(Version::parse(&version).is_ok());
        let version = sanitize_version_prefix("version_1.2.3");
        assert_eq!(version, "1.2.3");
        assert!(Version::parse(&version).is_ok());
    }

    #[test]
    fn test_normalize_version() {
        let version = normalize_version("version_112");
        assert!(version.is_some_and(|v| {
            v.major == 112 && v.minor == 0 && v.patch == 0
        }));

        let version = normalize_version("v3.3.3");
        assert!(version.is_some_and(|v| {
            v.major == 3 && v.minor == 3 && v.patch == 3
        }));

        let version = normalize_version("10.0.0");
        assert!(version.is_some_and(|v| {
            v.major == 10 && v.minor == 0 && v.patch == 0
        }));
    }

    #[test]
    fn test_incomplete_version_strings() {
        let version = normalize_version("5");
        assert!(version.is_some_and(|v| {
            v.major == 5 && v.minor == 0 && v.patch == 0
        }));

        let version = normalize_version("0.2");
        assert!(version.is_some_and(|v| {
            v.major == 0 && v.minor == 2 && v.patch == 0
        }));
    }

    #[test]
    fn test_invalid_versions() {
        let version = normalize_version("1a-test");
        assert_eq!(version, None);
    }
}
