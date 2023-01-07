#[cfg(test)]
mod tests;

mod assets;
mod bin_package;
mod dotenvs;
mod end2end;
mod lib_package;
mod project;
mod style;

use std::sync::Arc;

use crate::{
    ext::anyhow::{Context, Result},
    Opts,
};
use anyhow::bail;
use camino::Utf8Path;
pub use project::{Project, ProjectConfig};
pub use style::StyleConfig;

#[derive(Debug)]
pub struct Config {
    pub projects: Vec<Arc<Project>>,
    pub cli: Opts,
    pub watch: bool,
}

impl Config {
    pub fn load(cli: Opts, cwd: &Utf8Path, manifest_path: &Utf8Path, watch: bool) -> Result<Self> {
        let mut projects = Project::resolve(&cli, cwd, manifest_path, watch).dot()?;

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
            projects,
            cli,
            watch,
        })
    }

    #[cfg(test)]
    pub fn test_load(cli: Opts, cwd: &str, manifest_path: &str, watch: bool) -> Self {
        use camino::Utf8PathBuf;

        let manifest_path = Utf8PathBuf::from(manifest_path)
            .canonicalize_utf8()
            .unwrap();
        let cwd = Utf8PathBuf::from(cwd).canonicalize_utf8().unwrap();
        Self::load(cli, &cwd, &manifest_path, watch).unwrap()
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
