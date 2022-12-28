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
    pub fn load(cli: Opts, manifest_path: &Utf8Path, watch: bool) -> Result<Self> {
        let mut projects = Project::resolve(&cli, manifest_path, watch).dot()?;

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

    fn cwd_project(&self) -> Result<Option<Arc<Project>>> {
        let cwd = std::env::current_dir()?;
        Ok(self
            .projects
            .iter()
            .find(|p| p.bin.dir == cwd || p.lib.dir == cwd)
            .map(|p| p.clone()))
    }
    pub fn current_project(&self) -> Result<Arc<Project>> {
        if self.projects.len() == 1 {
            Ok(self.projects[0].clone())
        } else if let Some(proj) = self.cwd_project()? {
            Ok(proj)
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
