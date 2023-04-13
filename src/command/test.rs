use crate::compile::{front_cargo_process, server_cargo_process};
use crate::config::{Config, Project};
use crate::ext::anyhow::{Context, Result, anyhow};
use crate::logger::GRAY;

pub async fn test_all(conf: &Config) -> Result<()> {
    let mut first_failed_project = None;

    for proj in &conf.projects {
        if !test_proj(proj).await? && first_failed_project.is_none() {
            first_failed_project = Some(proj);
        }
    }

    if let Some(proj) = first_failed_project {
        Err(anyhow!("Tests failed for {}", proj.name))
    } else {
        Ok(())
    }
}

pub async fn test_proj(proj: &Project) -> Result<bool> {
    let (envs, line, mut proc) = server_cargo_process("test", proj).dot()?;

    let server_exit_status = proc.wait().await.dot()?;
    log::debug!("Cargo envs: {}", GRAY.paint(envs));
    log::info!("Cargo server tests finished {}", GRAY.paint(line));

    let (envs, line, mut proc) = front_cargo_process("test", false, proj).dot()?;

    let front_exit_status = proc.wait().await.dot()?;
    log::debug!("Cargo envs: {}", GRAY.paint(envs));
    log::info!("Cargo front tests finished {}", GRAY.paint(line));

    Ok(server_exit_status.success() && front_exit_status.success())
}
