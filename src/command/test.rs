use crate::compile::{front_cargo_process_with_args, server_cargo_process_with_args};
use crate::config::{Config, Project, TestSpecificOpts};
use crate::ext::Paint;
use crate::internal_prelude::*;
use crate::logger::GRAY;

pub async fn test_all(conf: &Config, test_opts: &TestSpecificOpts) -> Result<()> {
    let mut first_failed_project = None;
    let add_args = test_opts.to_args();

    for proj in &conf.projects {
        if !test_proj(proj, Some(&add_args)).await? && first_failed_project.is_none() {
            first_failed_project = Some(proj);
        }
    }

    if let Some(proj) = first_failed_project {
        Err(eyre!("Tests failed for {}", proj.name))
    } else {
        Ok(())
    }
}

pub async fn test_proj(proj: &Project, additional_args: Option<&[String]>) -> Result<bool> {
    let (envs, line, mut proc) =
        server_cargo_process_with_args("test", proj, additional_args).dot()?;

    let server_exit_status = proc.wait().await.dot()?;
    debug!("Cargo envs: {}", GRAY.paint(envs));
    info!("Cargo server tests finished {}", GRAY.paint(line));

    let (envs, line, mut proc) =
        front_cargo_process_with_args("test", false, proj, additional_args).dot()?;

    let front_exit_status = proc.wait().await.dot()?;
    debug!("Cargo envs: {}", GRAY.paint(envs));
    info!("Cargo front tests finished {}", GRAY.paint(line));

    Ok(server_exit_status.success() && front_exit_status.success())
}
