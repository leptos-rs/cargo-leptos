use crate::config::Config;
use anyhow::{Context, Result};
use tokio::process::Command;

pub async fn build(config: &Config) -> Result<()> {
    cmd("build", config).await
}

pub async fn run(config: &Config) -> Result<()> {
    cmd("run", config).await
}

pub async fn test(config: &Config) -> Result<()> {
    cmd("test", config).await
}

async fn cmd(command: &str, config: &Config) -> Result<()> {
    let manifest_path = format!("{}/Cargo.toml", config.root);
    let mut args = vec![
        command,
        "--no-default-features",
        "--features=ssr",
        "--manifest-path",
        &manifest_path,
    ];
    if config.release {
        args.push("--release");
    }

    try_cmd(&args)
        .await
        .context(format!("cargo {}", args.join(" ")))
}

pub async fn try_cmd(args: &[&str]) -> Result<()> {
    let mut cmd = Command::new("cargo").args(args).spawn()?;
    cmd.wait().await?;
    Ok(())
}
