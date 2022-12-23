use std::{net::SocketAddr, sync::Arc};

use crate::{
    ext::{
        anyhow::{anyhow, Error, Result},
        path::PathBufExt,
    },
    service::site::Site,
    Opts,
};
use anyhow::bail;
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::{MetadataCommand, Package, Target};
use serde::Deserialize;

use super::{
    dotenvs::{find_env_file, overlay_env},
    paths::ProjectPaths,
};

#[derive(Debug)]
pub struct Project {
    pub name: String,
    pub front_package: Package,
    pub front_config: FrontConfig,
    pub front_profile: String,
    pub server_package: Package,
    pub server_target: Target,
    pub server_profile: String,
    pub watch: bool,
    pub site: Arc<Site>,
    pub paths: ProjectPaths,
}

impl Project {
    pub fn resolve(cli: &Opts, watch: bool) -> Result<Vec<Arc<Project>>> {
        let metadata = MetadataCommand::new().manifest_path("Cargo.toml").exec()?;

        let projects = WorkspaceProject::parse(&metadata.workspace_metadata)?;
        let packages = metadata.workspace_packages();

        let mut resolved = Vec::new();
        for project in projects {
            let (bin_pkg, lib_pkg) = parse_packages(&project.packages)?;

            let server = packages
                .iter()
                .find(|p| p.name == bin_pkg)
                .ok_or_else(|| package_not_found(bin_pkg, &project.packages))?;

            let front = packages
                .iter()
                .find(|p| p.name == lib_pkg)
                .ok_or_else(|| package_not_found(lib_pkg, &project.packages))?;

            let bin_targets = server
                .targets
                .iter()
                .enumerate()
                .filter(|(_, t)| t.is_bin())
                .collect::<Vec<(usize, &Target)>>();

            let server_target_idx = if let Some(bin_target) = &project.bin_target {
                bin_targets
                    .iter()
                    .find(|(_, t)| t.name == *bin_target)
                    .ok_or_else(|| target_not_found(bin_target))?
                    .0
            } else if bin_targets.len() == 1 {
                bin_targets[0].0
            } else if bin_targets.is_empty() {
                bail!("No bin targets found for member {bin_pkg}");
            } else {
                return Err(many_targets_found(bin_pkg));
            };
            let front_dir = front.manifest_path.clone().without_last();

            let mut leptos_config = FrontConfig::parse(&front_dir, &front.metadata)?;

            if leptos_config.package_name.is_empty() {
                leptos_config.package_name = front.name.replace('-', "_");
            }
            let profile = if cli.release { "release" } else { "debug" };

            let paths = ProjectPaths::new(&metadata, front, server, &leptos_config, cli);

            let proj = Project {
                name: project.name.clone(),
                front_package: (*front).clone(),
                front_profile: profile.to_string(),
                front_config: leptos_config,
                server_package: (*server).clone(),
                server_target: server.targets[server_target_idx].clone(),
                server_profile: profile.to_string(),
                watch,
                site: Arc::new(Site::new()),
                paths,
            };
            resolved.push(Arc::new(proj));
        }
        Ok(resolved)
    }

    pub fn optimise_front(&self) -> bool {
        self.front_profile.contains("release")
    }

    pub fn optimise_server(&self) -> bool {
        self.server_profile.contains("release")
    }

    /// env vars to use when running external command
    pub fn to_envs(&self) -> Vec<(&'static str, String)> {
        let mut vec = vec![
            ("PACKAGE_NAME", self.front_config.package_name.to_string()),
            ("LEPTOS_SITE_ROOT", self.front_config.site_root.to_string()),
            (
                "LEPTOS_SITE_PKG_DIR",
                self.front_config.site_pkg_dir.to_string(),
            ),
            ("LEPTOS_SITE_ADDR", self.front_config.site_addr.to_string()),
            (
                "LEPTOS_RELOAD_PORT",
                self.front_config.reload_port.to_string(),
            ),
        ];
        if self.watch {
            vec.push(("LEPTOS_WATCH", "ON".to_string()))
        }
        vec
    }
}

#[derive(Deserialize, Debug)]
pub struct FrontConfig {
    #[serde(default)]
    pub package_name: String,
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
    #[serde(default = "default_browserquery")]
    pub browserquery: String,
}

impl FrontConfig {
    fn parse(dir: &Utf8Path, value: &serde_json::Value) -> Result<Self> {
        let value = value.as_object().map(|o| o.get("leptos")).flatten();
        if let Some(value) = value {
            let mut conf: FrontConfig = serde_json::from_value(value.clone())?;
            if let Some(file) = find_env_file(dir) {
                overlay_env(&mut conf, &file)?;
            }
            Ok(conf)
        } else {
            bail!("Missing [package.metadata.leptos] section")
        }
    }
}
fn default_site_addr() -> SocketAddr {
    SocketAddr::new([127, 0, 0, 1].into(), 3000)
}

fn default_pkg_dir() -> Utf8PathBuf {
    Utf8PathBuf::from("pkg")
}

fn default_site_root() -> Utf8PathBuf {
    Utf8PathBuf::from("target/site")
}

fn default_reload_port() -> u16 {
    3001
}

fn default_browserquery() -> String {
    "defaults".to_string()
}

fn many_targets_found(pkg: &str) -> Error {
    anyhow!(
        r#"Several bin targets found for member "{pkg}", please specify which one to use with: [[workspace.metadata.leptos]] bin-target = "name""#
    )
}
fn target_not_found(target: &str) -> Error {
    anyhow!(
        r#"Could not find the target specified: [[workspace.metadata.leptos]] bin-target = "{target}""#,
    )
}

fn package_not_found(pkg: &str, packages: &str) -> Error {
    anyhow!(
        r#"Could not find the workspace package "{pkg}", specified: [[workspace.metadata.leptos]] packages = "{packages}""#,
    )
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceProject {
    name: String,
    packages: String,
    bin_target: Option<String>,
}
impl WorkspaceProject {
    fn parse(value: &serde_json::Value) -> Result<Vec<Self>> {
        let value = value.as_object().map(|o| o.get("leptos")).flatten();
        if let Some(value) = value {
            Ok(serde_json::from_value(value.clone())?)
        } else {
            Ok(Default::default())
        }
    }
}

fn parse_packages(packages: &str) -> Result<(&str, &str)> {
    let mut parts = packages.split(" ");
    match (parts.next(), parts.next(), parts.next()) {
        (Some(p1), Some(p2), None) => {
            if p1.starts_with("bin:") && p2.starts_with("lib:") {
                return Ok((&p1[4..], &p2[4..]));
            } else if p1.starts_with("lib:") && p2.starts_with("bin:") {
                return Ok((&p2[4..], &p1[4..]));
            }
        }
        (Some(p1), None, None) => return Ok((p1, p1)),
        (_, _, Some(_)) | (None, _, _) => {}
    }
    bail!("Invalid [[workspace.metadata.leptos]] members specification: {packages}")
}
