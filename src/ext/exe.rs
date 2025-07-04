use crate::{config::VersionConfig, ext::Paint, internal_prelude::*, logger::GRAY};
use bytes::Bytes;
use std::{
    borrow::Cow,
    fs::{self, File},
    io::{Cursor, Write},
    path::{Path, PathBuf},
    str,
    sync::Once,
};

use std::env;

use zip::ZipArchive;

use super::util::{is_linux_musl_env, os_arch};

use reqwest::ClientBuilder;
#[cfg(target_family = "unix")]
use std::os::unix::prelude::PermissionsExt;
use std::time::{Duration, SystemTime};

use semver::Version;

#[derive(Debug)]
pub struct ExeMeta {
    name: &'static str,
    version: String,
    url: String,
    exe: String,
    manual: String,
}

static ON_STARTUP_DEBUG_ONCE: Once = Once::new();

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

impl ExeCache<'_> {
    fn exe_in_cache(&self) -> Result<PathBuf> {
        let exe_path = self.exe_dir.join(PathBuf::from(&self.meta.exe));

        if !exe_path.exists() {
            bail!("The path {exe_path:?} doesn't exist");
        }

        Ok(exe_path)
    }

    async fn fetch_archive(&self) -> Result<Bytes> {
        debug!(
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
                .wrap_err(format!("Could not write binary {}", self.meta.get_name()))?;
        }

        debug!(
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
            .wrap_err(format!("Error writing binary file: {:?}", path))?;

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
        info!("Command installing {} ...", self.meta.get_name());

        let data = self
            .fetch_archive()
            .await
            .wrap_err(format!("Could not download {}", self.meta.get_name()))?;

        self.extract_downloaded(&data)
            .wrap_err(format!("Could not extract {}", self.meta.get_name()))?;

        let binary_path = self.exe_in_cache().wrap_err(format!(
            "Binary downloaded and extracted but could still not be found at {:?}",
            self.exe_dir
        ))?;
        info!("Command {} installed.", self.meta.get_name());
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
    if !dest.exists() {
        fs::create_dir_all(dest).dot()?;
    }
    let content = Cursor::new(src);
    let dec = flate2::read::GzDecoder::new(content);
    let mut arch = tar::Archive::new(dec);
    arch.unpack(dest).dot()?;
    Ok(())
}

fn extract_zip(src: &Bytes, dest: &Path) -> Result<()> {
    if !dest.exists() {
        fs::create_dir_all(dest).dot()?;
    }
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
        .ok_or_else(|| eyre!("Cache directory does not exist"))?
        .join("cargo-leptos");

    if !dir.exists() {
        fs::create_dir_all(&dir).wrap_err(format!("Could not create dir {dir:?}"))?;
    }

    ON_STARTUP_DEBUG_ONCE.call_once(|| {
        debug!("Command cache dir: {}", dir.to_string_lossy());
    });

    Ok(dir)
}

#[derive(Debug, Hash, Eq, PartialEq)]
pub enum Exe {
    Sass,
    Tailwind,
    WasmOpt,
}

impl Exe {
    pub async fn get(&self) -> Result<PathBuf> {
        let meta = self.meta().await?;

        let path = if let Some(path) = meta.from_global_path() {
            path
        } else if cfg!(feature = "no_downloads") {
            bail!("{} is required but was not found. Please install it using your OS's tool of choice", &meta.name);
        } else {
            meta.cached().await.wrap_err(meta.manual)?
        };

        debug!(
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
            Exe::Sass => CommandSass.exe_meta(target_os, target_arch).await.dot()?,
            Exe::Tailwind => CommandTailwind
                .exe_meta(target_os, target_arch)
                .await
                .dot()?,
            Exe::WasmOpt => CommandWasmOpt
                .exe_meta(target_os, target_arch)
                .await
                .dot()?,
        };

        Ok(exe)
    }
}

/// Tailwind uses the 'vMaj.Min.Pat' format.
/// We generally want to keep the suffix intact,
/// as it carries classifiers, etc, but strip non-ascii
/// digits from the prefix.
///
/// Handles both semver-style prefixes (e.g., "v1.2.3") and
/// wasm-opt/Binaryen style prefixes (e.g., "version_123").
#[inline]
fn sanitize_version_prefix(ver_string: &str) -> Result<&str> {
    if let Some(rest) = ver_string.strip_prefix("version_") {
        // Handle "version_123" format (wasm-opt/Binaryen) - check this first
        Ok(rest)
    } else if let [b'v', rest @ ..] = ver_string.as_bytes() {
        // Handle "v1.2.3" format
        str::from_utf8(rest).dot()
    } else {
        Ok(ver_string)
    }
}

/// Attempts to convert a non-semver version string to a semver one.
/// we strip the prefix, treat it as `112.0.0`
fn normalize_version(ver_string: &str) -> Option<Version> {
    sanitize_version_prefix(ver_string)
        .ok()
        .and_then(|ver_string| {
            let version = Version::parse(ver_string)
                .ok()
                .or_else(|| {
                    ver_string
                        .parse::<u64>()
                        .map(|num| Version::new(num, 0, 0))
                        .ok()
                })
                .or_else(|| Version::parse(format!("{ver_string}.0").as_str()).ok());

            if version.is_none() {
                log::error!("Command failed to normalize version: {ver_string}");
            }

            version
        })
}

struct CommandTailwind;
struct CommandSass;
struct CommandWasmOpt;

impl Command for CommandTailwind {
    fn name(&self) -> &'static str {
        "tailwindcss"
    }
    fn version(&self) -> Cow<'_, str> {
        VersionConfig::Tailwind.version()
    }
    fn default_version(&self) -> &'static str {
        VersionConfig::Tailwind.default_version()
    }
    fn env_var_version_name(&self) -> &'static str {
        VersionConfig::Tailwind.env_var_version_name()
    }
    fn github_owner(&self) -> &'static str {
        "tailwindlabs"
    }
    fn github_repo(&self) -> &'static str {
        "tailwindcss"
    }

    /// Tool binary download url for the given OS and platform arch
    fn download_url(&self, target_os: &str, target_arch: &str, version: &str) -> Result<String> {
        let use_musl = is_linux_musl_env() && version.starts_with("v4");

        match (target_os, target_arch) {
            ("windows", "x86_64") => Ok(format!(
                "https://github.com/{}/{}/releases/download/{}/{}-windows-x64.exe",
                self.github_owner(),
                self.github_repo(),
                version,
                self.name()
            )),
            ("macos", "x86_64") => Ok(format!(
                "https://github.com/{}/{}/releases/download/{}/{}-macos-x64",
                self.github_owner(),
                self.github_repo(),
                version,
                self.name()
            )),
            ("macos", "aarch64") => Ok(format!(
                "https://github.com/{}/{}/releases/download/{}/{}-macos-arm64",
                self.github_owner(),
                self.github_repo(),
                version,
                self.name()
            )),
            ("linux", "x86_64") if use_musl => Ok(format!(
                "https://github.com/{}/{}/releases/download/{}/{}-linux-x64-musl",
                self.github_owner(),
                self.github_repo(),
                version,
                self.name()
            )),
            ("linux", "x86_64") => Ok(format!(
                "https://github.com/{}/{}/releases/download/{}/{}-linux-x64",
                self.github_owner(),
                self.github_repo(),
                version,
                self.name()
            )),
            ("linux", "aarch64") if use_musl => Ok(format!(
                "https://github.com/{}/{}/releases/download/{}/{}-linux-arm64-musl",
                self.github_owner(),
                self.github_repo(),
                version,
                self.name()
            )),
            ("linux", "aarch64") => Ok(format!(
                "https://github.com/{}/{}/releases/download/{}/{}-linux-arm64",
                self.github_owner(),
                self.github_repo(),
                version,
                self.name()
            )),
            _ => bail!(
                "Command [{}] failed to find a match for {}-{} ",
                self.name(),
                target_os,
                target_arch
            ),
        }
    }

    fn executable_name(&self, target_os: &str, target_arch: &str, version: &str) -> Result<String> {
        let use_musl = is_linux_musl_env() && version.starts_with("v4");

        Ok(match (target_os, target_arch) {
            ("windows", _) => format!("{}-windows-x64.exe", self.name()),
            ("macos", "x86_64") => format!("{}-macos-x64", self.name()),
            ("macos", "aarch64") => format!("{}-macos-arm64", self.name()),
            ("linux", "x86_64") if use_musl => format!("{}-linux-x64-musl", self.name()),
            ("linux", "x86_64") => format!("{}-linux-x64", self.name()),
            (_, _) if use_musl => format!("{}-linux-arm64-musl", self.name()),
            (_, _) => format!("{}-linux-arm64", self.name()),
        })
    }

    fn manual_install_instructions(&self) -> String {
        "Try manually installing tailwindcss: https://tailwindcss.com/docs/installation".to_string()
    }
}

impl Command for CommandSass {
    fn name(&self) -> &'static str {
        "sass"
    }
    fn version(&self) -> Cow<'_, str> {
        VersionConfig::Sass.version()
    }
    fn default_version(&self) -> &'static str {
        VersionConfig::Sass.default_version()
    }
    fn env_var_version_name(&self) -> &'static str {
        VersionConfig::Sass.env_var_version_name()
    }
    fn github_owner(&self) -> &'static str {
        "sass"
    }
    fn github_repo(&self) -> &'static str {
        "dart-sass"
    }

    fn download_url(&self, target_os: &str, target_arch: &str, version: &str) -> Result<String> {
        let is_musl_env = is_linux_musl_env();
        Ok(if is_musl_env {
            match target_arch {
                "x86_64" => {
                    format!(
                        "https://github.com/{}/{}/releases/download/{}/dart-sass-{}-linux-x64.tar.gz",
                        self.github_owner(), self.github_repo(), version, version
                    )
                }
                "aarch64" => {
                    format!(
                        "https://github.com/{}/{}/releases/download/{}/dart-sass-{}-linux-arm64.tar.gz"
                        , self.github_owner(), self.github_repo(), version, version
                    )
                }
                _ => bail!("No sass tar binary found for linux-musl {target_arch}"),
            }
        } else {
            match (target_os, target_arch) {
                // note the different github_owner
                ("windows", "x86_64") => {
                    format!(
                        "https://github.com/sass/{}/releases/download/{}/dart-sass-{}-windows-x64.zip",
                        self.github_repo(), version, version
                    )
                }
                ("macos" | "linux", "x86_64") => {
                    format!(
                        "https://github.com/sass/{}/releases/download/{}/dart-sass-{}-{}-x64.tar.gz",
                        self.github_repo(), version, version, target_os
                    )
                }
                ("macos" | "linux", "aarch64") => {
                    format!(
                        "https://github.com/sass/{}/releases/download/{}/dart-sass-{}-{}-arm64.tar.gz",
                        self.github_repo(), version, version, target_os
                    )
                }
                _ => bail!("No sass tar binary found for {target_os} {target_arch}"),
            }
        })
    }

    fn executable_name(
        &self,
        target_os: &str,
        _target_arch: &str,
        _version: &str,
    ) -> Result<String> {
        Ok(match target_os {
            "windows" => "dart-sass/sass.bat".to_string(),
            _ => "dart-sass/sass".to_string(),
        })
    }

    fn manual_install_instructions(&self) -> String {
        "Try manually installing sass: https://sass-lang.com/install".to_string()
    }
}

impl Command for CommandWasmOpt {
    fn name(&self) -> &'static str {
        "wasm-opt"
    }
    fn version(&self) -> Cow<'_, str> {
        VersionConfig::WasmOpt.version()
    }
    fn default_version(&self) -> &'static str {
        VersionConfig::WasmOpt.default_version()
    }
    fn env_var_version_name(&self) -> &'static str {
        VersionConfig::WasmOpt.env_var_version_name()
    }
    fn github_owner(&self) -> &'static str {
        "WebAssembly"
    }
    fn github_repo(&self) -> &'static str {
        "binaryen"
    }

    fn download_url(&self, target_os: &str, target_arch: &str, version: &str) -> Result<String> {
        // https://github.com/WebAssembly/binaryen/releases/download/version_123/binaryen-version_123-x86_64-windows.tar.gz - ✅
        // https://github.com/WebAssembly/binaryen/releases/download/version_123/binaryen-version_123-x86_64-macos.tar.gz - ✅
        // https://github.com/WebAssembly/binaryen/releases/download/version_123/binaryen-version_123-arm64-macos.tar.gz - ✅
        // https://github.com/WebAssembly/binaryen/releases/download/version_123/binaryen-version_123-aarch64-linux.tar.gz - ✅
        // https://github.com/WebAssembly/binaryen/releases/download/version_123/binaryen-version_123-x86_64-linux.tar.gz - ✅
        // https://github.com/WebAssembly/binaryen/releases/download/version_123/binaryen-version_123-node.tar.gz

        let base_url = format!(
            "https://github.com/{}/{}/releases/download/{}/binaryen-{}",
            self.github_owner(),
            self.github_repo(),
            version,
            version
        );
        match (target_os, target_arch) {
            ("windows", "x86_64") => Ok(format!("{base_url}-x86_64-windows.tar.gz")),
            ("macos", "x86_64") => Ok(format!("{base_url}-x86_64-macos.tar.gz")),
            ("macos", "aarch64") => Ok(format!("{base_url}-arm64-macos.tar.gz")),
            ("macos", "arm64") => Ok(format!("{base_url}-arm64-macos.tar.gz")),
            ("linux", "aarch64") => Ok(format!("{base_url}-aarch64-linux.tar.gz")),
            ("linux", "arm64") => Ok(format!("{base_url}-aarch64-linux.tar.gz")),
            ("linux", "x86_64") => Ok(format!("{base_url}-x86_64-linux.tar.gz")),
            _ => bail!(
                "Command [{}] failed to find a match for {}-{} ",
                self.name(),
                target_os,
                target_arch
            ),
        }
    }

    fn executable_name(
        &self,
        target_os: &str,
        _target_arch: &str,
        version: &str,
    ) -> Result<String> {
        let exe_name = if target_os == "windows" {
            "wasm-opt.exe"
        } else {
            "wasm-opt"
        };
        Ok(format!("binaryen-{}/bin/{}", version, exe_name))
    }

    fn manual_install_instructions(&self) -> String {
        "Try manually installing wasm-opt from Binaryen: https://github.com/WebAssembly/binaryen"
            .to_string()
    }
}

/// Template trait, implementors should only fill in
/// the command-specific logic. Handles caching, latest
/// version checking against the GitHub API and env var
/// version override for a given command.
trait Command {
    fn name(&self) -> &'static str;
    fn version(&self) -> Cow<'_, str>;
    fn default_version(&self) -> &str;
    fn env_var_version_name(&self) -> &str;
    fn github_owner(&self) -> &str;
    fn github_repo(&self) -> &str;
    fn download_url(&self, target_os: &str, target_arch: &str, version: &str) -> Result<String>;
    fn executable_name(&self, target_os: &str, target_arch: &str, version: &str) -> Result<String>;
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
        let exe = self.executable_name(target_os, target_arch, version.as_str())?;
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
                match (marker.exists(), marker.is_dir()) {
                    (_, true) => {
                        // conflicting dir instead of a marker file, bail
                        warn!("Command [{}] encountered a conflicting dir in the cache, please delete {}",
                            self.name(), marker.display());

                        false
                    }
                    (true, _) => {
                        // existing marker file, read and check if last checked > 1 DAY
                        let contents = tokio::fs::read_to_string(&marker).await;
                        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH);
                        if let Some(timestamp) = contents
                            .ok()
                            .map(|s| s.parse::<u64>().ok().unwrap_or_default())
                        {
                            let last_checked = Duration::from_millis(timestamp);
                            let one_day = Duration::from_secs(24 * 60 * 60);
                            if let Ok(now) = now {
                                match (now - last_checked) > one_day {
                                    true => tokio::fs::write(&marker, now.as_millis().to_string())
                                        .await
                                        .is_ok(),
                                    false => false,
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    (false, _) => {
                        // no marker file yet, record and hint to check
                        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH);
                        if let Ok(unix_timestamp) = now {
                            tokio::fs::write(marker, unix_timestamp.as_millis().to_string())
                                .await
                                .is_ok()
                        } else {
                            false
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Command {} failed to get cache dir: {}", self.name(), e);
                false
            }
        }
    }

    async fn check_for_latest_version(&self) -> Option<String> {
        debug!(
            "Command [{}] checking for the latest available version",
            self.name()
        );

        let client = ClientBuilder::default()
            // this github api allows anonymous, but requires a user-agent header be set
            .user_agent("cargo-leptos")
            .build()
            .unwrap_or_default();

        if let Ok(response) = client
            .get(format!(
                "https://api.github.com/repos/{}/{}/releases/latest",
                self.github_owner(),
                self.github_repo()
            ))
            .send()
            .await
        {
            if !response.status().is_success() {
                error!(
                    "Command [{}] GitHub API request failed: {}",
                    self.name(),
                    response.status()
                );
                return None;
            }

            #[derive(serde::Deserialize)]
            struct Github {
                tag_name: String, // this is the version number, not the git tag
            }

            let github: Github = match response.json().await {
                Ok(json) => json,
                Err(e) => {
                    debug!(
                        "Command [{}] failed to parse the response JSON from the GitHub API: {}",
                        self.name(),
                        e
                    );
                    return None;
                }
            };

            Some(github.tag_name)
        } else {
            debug!(
                "Command [{}] failed to check for the latest version",
                self.name()
            );
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
        trace!(
            "Command [{}] is_force_pin_version: {} - {:?}",
            self.name(),
            is_force_pin_version,
            env::var(self.env_var_version_name())
        );

        if !is_force_pin_version && !self.should_check_for_new_version().await {
            trace!(
                "Command [{}] NOT checking for the latest available version",
                &self.name()
            );
            return self.default_version().into();
        }

        let version = self.version();

        let latest = self.check_for_latest_version().await;

        match latest {
            Some(latest) => {
                let norm_latest = normalize_version(latest.as_str());
                let norm_version = normalize_version(&version);
                if norm_latest.is_some() && norm_version.is_some() {
                    // TODO use the VersionReq for semantic matching
                    match norm_version.cmp(&norm_latest) {
                        core::cmp::Ordering::Greater | core::cmp::Ordering::Equal => {
                            debug!(
                                            "Command [{}] requested version {} is already same or newer than available version {}",
                                            self.name(), version, &latest)
                        }
                        core::cmp::Ordering::Less => {
                            info!(
                                            "Command [{}] requested version {}, but a newer version {} is available, you can try it out by \
                                            setting the {}={} env var and re-running the command",
                                            self.name(), version, &latest, self.env_var_version_name(), &latest)
                        }
                    }
                }
            }
            None => warn!(
                "Command [{}] failed to check for the latest version",
                self.name()
            ),
        }

        version.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargo_metadata::semver::Version;

    #[test]
    fn test_sanitize_version_prefix() {
        // Test standard semver with 'v' prefix
        let version = sanitize_version_prefix("v1.2.3").expect("Could not sanitize \"v1.2.3\".");
        assert_eq!(version, "1.2.3");
        assert!(Version::parse(version).is_ok());

        // Test wasm-opt/Binaryen 'version_' prefix format
        let version =
            sanitize_version_prefix("version_123").expect("Could not sanitize \"version_123\".");
        assert_eq!(version, "123");

        // Test wasm-opt with suffix (like "version_120_b")
        let version = sanitize_version_prefix("version_120_b")
            .expect("Could not sanitize \"version_120_b\".");
        assert_eq!(version, "120_b");

        // Test no prefix
        let version = sanitize_version_prefix("1.2.3").expect("Could not sanitize \"1.2.3\".");
        assert_eq!(version, "1.2.3");
        assert!(Version::parse(version).is_ok());

        // Test plain number (like "123")
        let version = sanitize_version_prefix("123").expect("Could not sanitize \"123\".");
        assert_eq!(version, "123");
    }

    #[test]
    fn test_normalize_version() {
        let version = normalize_version("v3.3.3");
        assert!(version.is_some());
        let v = version.unwrap();
        assert_eq!(v.major, 3);
        assert_eq!(v.minor, 3);
        assert_eq!(v.patch, 3);

        let version = normalize_version("10.0.0");
        assert!(version.is_some());
        let v = version.unwrap();
        assert_eq!(v.major, 10);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);

        // Test wasm-opt version format (pure numeric)
        let version = normalize_version("version_123");
        assert!(version.is_some());
        let v = version.unwrap();
        assert_eq!(v.major, 123);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);

        // Test wasm-opt version with non-numeric suffix (should fail gracefully)
        let _version = normalize_version("version_120_b");
        // This might return None because "120_b" is not a valid semver or pure number
        // But the sanitization should still work correctly
        assert_eq!(sanitize_version_prefix("version_120_b").unwrap(), "120_b");
    }

    #[test]
    fn test_incomplete_version_strings() {
        let version = normalize_version("5");
        assert!(version.is_some());
        let v = version.unwrap();
        assert_eq!(v.major, 5);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);

        let version = normalize_version("0.2");
        assert!(version.is_some());
        let v = version.unwrap();
        assert_eq!(v.major, 0);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_invalid_versions() {
        let version = normalize_version("1a-test");
        assert_eq!(version, None);
    }
}
