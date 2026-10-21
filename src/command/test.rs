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
    let server_status = proj.frontend_only || test_server(proj, additional_args).await?;
    let frontend_status = proj.server_only || test_frontend(proj, additional_args).await?;
    Ok(server_status && frontend_status)
}

pub async fn test_server(proj: &Project, additional_args: Option<&[String]>) -> Result<bool> {
    let (envs, line, mut proc) =
        server_cargo_process_with_args("test", proj, additional_args).dot()?;

    let server_exit_status = proc.wait().await.dot()?;
    debug!("Cargo envs: {}", GRAY.paint(envs));
    info!("Cargo server tests finished {}", GRAY.paint(line));
    Ok(server_exit_status.success())
}

pub async fn test_frontend(proj: &Project, additional_args: Option<&[String]>) -> Result<bool> {
    let (envs, line, mut proc) =
        front_cargo_process_with_args("test", false, proj, additional_args).dot()?;

    let frontend_exit_status = proc.wait().await.dot()?;
    debug!("Cargo envs: {}", GRAY.paint(envs));
    info!("Cargo frontend tests finished {}", GRAY.paint(line));
    Ok(frontend_exit_status.success())
}
