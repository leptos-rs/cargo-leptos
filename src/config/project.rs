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
    dotenvs::{find_env_file, overlay_env},
    end2end::End2EndConfig,
    style::StyleConfig,
};

pub struct Project {
    /// absolute path to the working dir
    pub working_dir: Utf8PathBuf,
    pub name: String,
    pub lib: LibPackage,
    pub bin: BinPackage,
    pub style: Option<StyleConfig>,
    pub watch: bool,
    pub release: bool,
    pub site: Arc<Site>,
    pub end2end: Option<End2EndConfig>,
    pub assets: Option<AssetsConfig>,
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
    ) -> Result<Vec<Arc<Project>>> {
        let projects = ProjectDefinition::parse(&metadata)?;

        let mut resolved = Vec::new();
        for (project, mut config) in projects {
            if config.output_name.is_empty() {
                config.output_name = project.name.to_string();
            }

            let proj = Project {
                working_dir: metadata.workspace_root.clone(),
                name: project.name.clone(),
                lib: LibPackage::resolve(cli, &metadata, &project, &config)?,
                bin: BinPackage::resolve(cli, &metadata, &project, &config)?,
                style: StyleConfig::new(&config),
                watch,
                release: cli.release,
                site: Arc::new(Site::new(&config)),
                end2end: End2EndConfig::resolve(&config),
                assets: AssetsConfig::resolve(&config),
            };
            resolved.push(Arc::new(proj));
        }

        let projects_in_cwd = resolved
            .iter()
            .filter(|p| p.bin.abs_dir.starts_with(&cwd) || p.lib.abs_dir.starts_with(&cwd))
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
    /// assets dir. content will be copied to the target/site dir
    pub assets_dir: Option<Utf8PathBuf>,
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
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub lib_features: Vec<String>,
    #[serde(default)]
    pub lib_default_features: bool,
    #[serde(default)]
    pub bin_features: Vec<String>,
    #[serde(default)]
    pub bin_default_features: bool,
    #[serde(skip)]
    pub config_dir: Utf8PathBuf,

    // Profiles
    pub lib_profile_dev: Option<String>,
    pub lib_profile_release: Option<String>,
    pub bin_profile_dev: Option<String>,
    pub bin_profile_release: Option<String>,
}

impl ProjectConfig {
    fn parse(dir: &Utf8Path, metadata: &serde_json::Value) -> Result<Self> {
        let mut conf: ProjectConfig = serde_json::from_value(metadata.clone())?;
        conf.config_dir = dir.to_path_buf();
        if let Some(file) = find_env_file(dir) {
            overlay_env(&mut conf, &file)?;
        }
        if conf.site_root == "/" || conf.site_root == "." {
            bail!(
                "site-root cannot be '{}'. All the content is erased when building the site.",
                conf.site_root
            );
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
    ) -> Result<Vec<(Self, ProjectConfig)>> {
        let mut found = Vec::new();
        if let Some(arr) = metadata.as_array() {
            for section in arr {
                let conf = ProjectConfig::parse(dir, section)?;
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
    ) -> Result<(Self, ProjectConfig)> {
        let conf = ProjectConfig::parse(dir, metadata)?;

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
                Self::from_workspace(md, &Utf8PathBuf::default())?
            } else {
                Default::default()
            };

        for package in metadata.workspace_packages() {
            let dir = package.manifest_path.unbase(workspace_dir)?.without_last();

            if let Some(metadata) = leptos_metadata(&package.metadata) {
                found.push(Self::from_project(package, metadata, &dir)?);
            }
        }
        Ok(found)
    }
}

fn leptos_metadata(metadata: &serde_json::Value) -> Option<&serde_json::Value> {
    metadata.as_object().map(|o| o.get("leptos")).flatten()
}

fn default_site_addr() -> SocketAddr {
    SocketAddr::new([127, 0, 0, 1].into(), 3000)
}

fn default_pkg_dir() -> Utf8PathBuf {
    Utf8PathBuf::from("pkg")
}

fn default_site_root() -> Utf8PathBuf {
    Utf8PathBuf::from("target").join("site")
}

fn default_reload_port() -> u16 {
    3001
}

fn default_browserquery() -> String {
    "defaults".to_string()
}
