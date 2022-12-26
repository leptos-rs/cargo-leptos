use crate::compile::{front_cargo_process, server_cargo_process};
use crate::config::{Config, Project};
use crate::ext::anyhow::{Context, Result};
use crate::logger::GRAY;

pub async fn test_all(conf: &Config) -> Result<()> {
    for proj in &conf.projects {
        test_proj(proj).await?;
    }
    Ok(())
}

pub async fn test_proj(proj: &Project) -> Result<()> {
    let (envs, line, mut proc) = server_cargo_process("test", proj).dot()?;

    proc.wait().await.dot()?;
    log::debug!("Cargo envs: {}", GRAY.paint(envs));
    log::info!("Cargo server tests finished {}", GRAY.paint(line));

    let (envs, line, mut proc) = front_cargo_process("test", false, proj).dot()?;

    proc.wait().await.dot()?;
    log::debug!("Cargo envs: {}", GRAY.paint(envs));
    log::info!("Cargo front tests finished {}", GRAY.paint(line));
    Ok(())
}
