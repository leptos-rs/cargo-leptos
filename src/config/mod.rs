#[cfg(test)]
mod tests;

mod assets;
mod bin_package;
mod cli;
mod dotenvs;
mod end2end;
mod lib_package;
mod profile;
mod project;
mod style;
mod tailwind;

use std::{fmt::Debug, sync::Arc};

pub use self::cli::{Cli, Commands, Log, Opts};
use crate::ext::{
    anyhow::{Context, Result},
    MetadataExt,
};
use anyhow::bail;
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::Metadata;
pub use profile::Profile;
pub use project::{Project, ProjectConfig};
pub use style::StyleConfig;
pub use tailwind::TailwindConfig;

pub struct Config {
    /// absolute path to the working dir
    pub working_dir: Utf8PathBuf,
    pub projects: Vec<Arc<Project>>,
    pub cli: Opts,
    pub watch: bool,
}

impl Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("projects", &self.projects)
            .field("cli", &self.cli)
            .field("watch", &self.watch)
            .finish_non_exhaustive()
    }
}

impl Config {
    pub fn load(
        cli: Opts,
        cwd: &Utf8Path,
        manifest_path: &Utf8Path,
        watch: bool,
        bin_args: Option<&[String]>,
    ) -> Result<Self> {
        let metadata = Metadata::load_cleaned(manifest_path)?;

        let mut projects = Project::resolve(&cli, cwd, &metadata, watch, bin_args).dot()?;

        if projects.is_empty() {
            bail!("Please define leptos projects in the workspace Cargo.toml sections [[workspace.metadata.leptos]]")
        }

        if let Some(proj_name) = &cli.project {
            if let Some(proj) = projects.iter().find(|p| p.name == *proj_name) {
                projects = vec![proj.clone()];
            } else {
                bail!(
                    r#"The specified project "{proj_name}" not found. Available projects: {}"#,
                    names(&projects)
                )
            }
        }

        Ok(Self {
            working_dir: metadata.workspace_root,
            projects,
            cli,
            watch,
        })
    }

    #[cfg(test)]
    pub fn test_load(
        cli: Opts,
        cwd: &str,
        manifest_path: &str,
        watch: bool,
        bin_args: Option<&[String]>,
    ) -> Self {
        use crate::ext::PathBufExt;

        let manifest_path = Utf8PathBuf::from(manifest_path)
            .canonicalize_utf8()
            .unwrap();
        let mut cwd = Utf8PathBuf::from(cwd).canonicalize_utf8().unwrap();
        cwd.clean_windows_path();
        Self::load(cli, &cwd, &manifest_path, watch, bin_args).unwrap()
    }

    pub fn current_project(&self) -> Result<Arc<Project>> {
        if self.projects.len() == 1 {
            Ok(self.projects[0].clone())
        } else {
            bail!("There are several projects available ({}). Please select one of them with the command line parameter --project", names(&self.projects));
        }
    }
}

fn names(projects: &[Arc<Project>]) -> String {
    projects
        .iter()
        .map(|p| p.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}
