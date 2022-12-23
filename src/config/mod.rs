mod dotenvs;
mod paths;
mod project;

use std::sync::Arc;

use crate::{ext::anyhow::Result, Cli, Commands, Opts};
use anyhow::bail;
pub use project::{FrontConfig, Project};

pub struct Config {
    pub projects: Vec<Arc<Project>>,
    pub cli: Opts,
    pub watch: bool,
}

impl Config {
    pub fn load(cli: &Cli, opts: Opts) -> Result<Self> {
        let watch = matches!(cli.command, Commands::Watch(_));
        let mut projects = Project::resolve(&opts, watch)?;

        if projects.is_empty() {
            bail!("Please define leptos projects in the workspace Cargo.toml sections [[workspace.metadata.leptos]]")
        }

        if let Some(proj_name) = &opts.project {
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
            cli: opts,
            watch,
        })
    }

    fn cwd_project(&self) -> Result<Option<Arc<Project>>> {
        let cwd = std::env::current_dir()?;
        Ok(self
            .projects
            .iter()
            .find(|p| p.paths.server_dir == cwd || p.paths.front_dir == cwd)
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
