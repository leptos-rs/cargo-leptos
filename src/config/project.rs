use crate::{
    config::lib_package::LibPackage,
    ext::{
        anyhow::{bail, ensure, Result},
        PackageExt, PathBufExt, PathExt,
    },
    logger::GRAY,
    service::site::Site,
};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::{Metadata, Package};
use serde::Deserialize;
use std::{fmt::Debug, net::SocketAddr, sync::Arc};

use super::{
    assets::AssetsConfig,
    bin_package::BinPackage,
    cli::Opts,
    dotenvs::{load_dotenvs, overlay_env},
    end2end::End2EndConfig,
    style::StyleConfig,
};

/// If the site root path starts with this marker, the marker should be replaced with the Cargo target directory
const CARGO_TARGET_DIR_MARKER: &str = "CARGO_TARGET_DIR";
/// If the site root path starts with this marker, the marker should be replaced with the Cargo target directory
const CARGO_BUILD_TARGET_DIR_MARKER: &str = "CARGO_BUILD_TARGET_DIR";

pub struct Project {
    /// absolute path to the working dir
    pub working_dir: Utf8PathBuf,
    pub name: String,
    pub lib: LibPackage,
    pub bin: BinPackage,
    pub style: StyleConfig,
    pub watch: bool,
    pub release: bool,
    pub precompress: bool,
    pub hot_reload: bool,
    pub site: Arc<Site>,
    pub end2end: Option<End2EndConfig>,
    pub assets: Option<AssetsConfig>,
    pub js_dir: Utf8PathBuf,
    pub watch_additional_files: Vec<Utf8PathBuf>,
    pub hash_file: Utf8PathBuf,
}

impl Debug for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Project")
            .field("name", &self.name)
            .field("lib", &self.lib)
            .field("bin", &self.bin)
            .field("style", &self.style)
            .field("watch", &self.watch)
            .field("release", &self.release)
            .field("precompress", &self.precompress)
            .field("hot_reload", &self.hot_reload)
            .field("site", &self.site)
            .field("end2end", &self.end2end)
            .field("assets", &self.assets)
            .finish_non_exhaustive()
    }
}

impl Project {
    pub fn resolve(
        cli: &Opts,
        cwd: &Utf8Path,
        metadata: &Metadata,
        watch: bool,
        bin_args: Option<&[String]>,
    ) -> Result<Vec<Arc<Project>>> {
        let projects = ProjectDefinition::parse(metadata)?;

        let mut resolved = Vec::new();
        for (project, mut config) in projects {
            if config.output_name.is_empty() {
                config.output_name = project.name.to_string();
            }

            let lib = LibPackage::resolve(cli, metadata, &project, &config)?;

            let js_dir = config
                .js_dir
                .clone()
                .unwrap_or_else(|| Utf8PathBuf::from("src"));

            let watch_additional_files = config.watch_additional_files.clone().unwrap_or_default();

            let bin = BinPackage::resolve(cli, metadata, &project, &config, bin_args)?;

            let hash_file = metadata
                .target_directory
                .join(bin.profile.to_string())
                .join(
                    config
                        .hash_file
                        .as_ref()
                        .unwrap_or(&Utf8PathBuf::from("hash.txt".to_string())),
                );

            let proj = Project {
                working_dir: metadata.workspace_root.clone(),
                name: project.name.clone(),
                lib,
                bin,
                style: StyleConfig::new(&config)?,
                watch,
                release: cli.release,
                precompress: cli.precompress,
                hot_reload: cli.hot_reload,
                site: Arc::new(Site::new(&config)),
                end2end: End2EndConfig::resolve(&config),
                assets: AssetsConfig::resolve(&config),
                js_dir,
                watch_additional_files,
                hash_file,
            };
            resolved.push(Arc::new(proj));
        }

        let projects_in_cwd = resolved
            .iter()
            .filter(|p| p.bin.abs_dir.starts_with(cwd) || p.lib.abs_dir.starts_with(cwd))
            .collect::<Vec<_>>();

        if projects_in_cwd.len() == 1 {
            Ok(vec![projects_in_cwd[0].clone()])
        } else {
            Ok(resolved)
        }
    }

    /// env vars to use when running external command
    pub fn to_envs(&self) -> Vec<(&'static str, String)> {
        let mut vec = vec![
            ("LEPTOS_OUTPUT_NAME", self.lib.output_name.to_string()),
            ("LEPTOS_SITE_ROOT", self.site.root_dir.to_string()),
            ("LEPTOS_SITE_PKG_DIR", self.site.pkg_dir.to_string()),
            ("LEPTOS_SITE_ADDR", self.site.addr.to_string()),
            ("LEPTOS_RELOAD_PORT", self.site.reload.port().to_string()),
            ("LEPTOS_LIB_DIR", self.lib.rel_dir.to_string()),
            ("LEPTOS_BIN_DIR", self.bin.rel_dir.to_string()),
        ];
        if self.watch {
            vec.push(("LEPTOS_WATCH", "ON".to_string()))
        }
        vec
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct ProjectConfig {
    #[serde(default)]
    pub output_name: String,
    #[serde(default = "default_site_addr")]
    pub site_addr: SocketAddr,
    #[serde(default = "default_site_root")]
    pub site_root: Utf8PathBuf,
    #[serde(default = "default_pkg_dir")]
    pub site_pkg_dir: Utf8PathBuf,
    pub style_file: Option<Utf8PathBuf>,
    /// text file where the hashes of the frontend files are stored
    pub hash_file: Option<Utf8PathBuf>,
    pub tailwind_input_file: Option<Utf8PathBuf>,
    pub tailwind_config_file: Option<Utf8PathBuf>,
    /// assets dir. content will be copied to the target/site dir
    pub assets_dir: Option<Utf8PathBuf>,
    /// js dir. changes triggers rebuilds.
    pub js_dir: Option<Utf8PathBuf>,
    /// additional files to watch. changes triggers rebuilds.
    pub watch_additional_files: Option<Vec<Utf8PathBuf>>,
    #[serde(default = "default_reload_port")]
    pub reload_port: u16,
    /// command for launching end-2-end integration tests
    pub end2end_cmd: Option<String>,
    /// the dir used when launching end-2-end integration tests
    pub end2end_dir: Option<Utf8PathBuf>,
    #[serde(default = "default_browserquery")]
    pub browserquery: String,
    /// the bin target to use for building the server
    #[serde(default)]
    pub bin_target: String,
    /// the bin output target triple to use for building the server
    pub bin_target_triple: Option<String>,
    /// the directory to put the generated server artifacts
    pub bin_target_dir: Option<String>,
    /// the command to run instead of "cargo" when building the server
    pub bin_cargo_command: Option<String>,
    /// cargo flags to pass to cargo when running the server. Overriden by bin_cargo_command
    pub bin_cargo_args: Option<String>,
    /// An optional override, if you've changed the name of your bin file in your project you'll need to set it here as well.
    pub bin_exe_name: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub lib_features: Vec<String>,
    #[serde(default)]
    pub lib_default_features: bool,
    /// cargo flags to pass to cargo when building the WASM frontend
    pub lib_cargo_args: Option<String>,
    #[serde(default)]
    pub bin_features: Vec<String>,
    #[serde(default)]
    pub bin_default_features: bool,

    #[serde(skip)]
    pub config_dir: Utf8PathBuf,
    #[serde(skip)]
    pub tmp_dir: Utf8PathBuf,

    /// Deprecated. Keeping this here to warn users to remove it in case they have it in their config.
    #[deprecated = "This option is deprecated since cargo-leptos 0.2.3 (when it became unconditionally enabled). You may remove it from your config."]
    pub separate_front_target_dir: Option<bool>,

    // Profiles
    pub lib_profile_dev: Option<String>,
    pub lib_profile_release: Option<String>,
    pub bin_profile_dev: Option<String>,
    pub bin_profile_release: Option<String>,
}

impl ProjectConfig {
    fn parse(
        dir: &Utf8Path,
        metadata: &serde_json::Value,
        cargo_metadata: &Metadata,
    ) -> Result<Self> {
        let mut conf: ProjectConfig = serde_json::from_value(metadata.clone())?;
        conf.config_dir = dir.to_path_buf();
        conf.tmp_dir = cargo_metadata.target_directory.join("tmp");
        let dotenvs = load_dotenvs(dir)?;
        overlay_env(&mut conf, dotenvs)?;
        if conf.site_root == "/"
            || conf.site_root == "."
            || conf.site_root == CARGO_TARGET_DIR_MARKER
            || conf.site_root == CARGO_BUILD_TARGET_DIR_MARKER
        {
            bail!(
                "site-root cannot be '{}'. All the content is erased when building the site.",
                conf.site_root
            );
        }
        if conf.site_root.starts_with(CARGO_TARGET_DIR_MARKER) {
            conf.site_root = {
                let mut path = cargo_metadata.target_directory.clone();
                // unwrap() should be safe because we just checked
                let sub = conf
                    .site_root
                    .unbase(CARGO_TARGET_DIR_MARKER.into())
                    .unwrap();
                path.push(sub);
                path
            };
        }
        if conf.site_root.starts_with(CARGO_BUILD_TARGET_DIR_MARKER) {
            conf.site_root = {
                let mut path = cargo_metadata.target_directory.clone();
                // unwrap() should be safe because we just checked
                let sub = conf
                    .site_root
                    .unbase(CARGO_BUILD_TARGET_DIR_MARKER.into())
                    .unwrap();
                path.push(sub);
                path
            };
        }
        if conf.site_addr.port() == conf.reload_port {
            bail!(
                "The site-addr port and reload-port cannot be the same: {}",
                conf.reload_port
            );
        }

        #[allow(deprecated)]
        if conf.separate_front_target_dir.is_some() {
            log::warn!("Deprecated: the `separate-front-target-dir` option is deprecated since cargo-leptos 0.2.3");
            log::warn!("It is now unconditionally enabled; you can remove it from your Cargo.toml")
        }

        Ok(conf)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProjectDefinition {
    name: String,
    pub bin_package: String,
    pub lib_package: String,
}
impl ProjectDefinition {
    fn from_workspace(
        metadata: &serde_json::Value,
        dir: &Utf8Path,
        cargo_metadata: &Metadata,
    ) -> Result<Vec<(Self, ProjectConfig)>> {
        let mut found = Vec::new();
        if let Some(arr) = metadata.as_array() {
            for section in arr {
                let conf = ProjectConfig::parse(dir, section, cargo_metadata)?;
                let def: Self = serde_json::from_value(section.clone())?;
                found.push((def, conf))
            }
        }
        Ok(found)
    }

    fn from_project(
        package: &Package,
        metadata: &serde_json::Value,
        dir: &Utf8Path,
        cargo_metadata: &Metadata,
    ) -> Result<(Self, ProjectConfig)> {
        let conf = ProjectConfig::parse(dir, metadata, cargo_metadata)?;

        ensure!(
            package.cdylib_target().is_some(),
            "Cargo.toml has leptos metadata but is missing a cdylib library target. {}",
            GRAY.paint(package.manifest_path.as_str())
        );
        ensure!(
            package.has_bin_target(),
            "Cargo.toml has leptos metadata but is missing a bin target. {}",
            GRAY.paint(package.manifest_path.as_str())
        );

        Ok((
            ProjectDefinition {
                name: package.name.to_string(),
                bin_package: package.name.to_string(),
                lib_package: package.name.to_string(),
            },
            conf,
        ))
    }

    fn parse(metadata: &Metadata) -> Result<Vec<(Self, ProjectConfig)>> {
        let workspace_dir = &metadata.workspace_root;
        let mut found: Vec<(Self, ProjectConfig)> =
            if let Some(md) = leptos_metadata(&metadata.workspace_metadata) {
                Self::from_workspace(md, &Utf8PathBuf::default(), metadata)?
            } else {
                Default::default()
            };

        for package in metadata.workspace_packages() {
            let dir = package.manifest_path.unbase(workspace_dir)?.without_last();

            if let Some(leptos_metadata) = leptos_metadata(&package.metadata) {
                found.push(Self::from_project(
                    package,
                    leptos_metadata,
                    &dir,
                    metadata,
                )?);
            }
        }
        Ok(found)
    }
}

fn leptos_metadata(metadata: &serde_json::Value) -> Option<&serde_json::Value> {
    metadata.as_object().and_then(|o| o.get("leptos"))
}

fn default_site_addr() -> SocketAddr {
    SocketAddr::new([127, 0, 0, 1].into(), 3000)
}

fn default_pkg_dir() -> Utf8PathBuf {
    Utf8PathBuf::from("pkg")
}

fn default_site_root() -> Utf8PathBuf {
    Utf8PathBuf::from(CARGO_TARGET_DIR_MARKER).join("site")
}

fn default_reload_port() -> u16 {
    3001
}

fn default_browserquery() -> String {
    "defaults".to_string()
}
