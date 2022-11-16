use super::oneshot_when;
use crate::{config::Config, Msg};
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
    let features = match config.watch {
        true => "--features=ssr,leptos_autoreload",
        false => "--features=ssr",
    };
    let mut args = vec![command, "--no-default-features", features];
    if config.cli.release {
        args.push("--release");
    }

    let stop_rx = oneshot_when(
        &[Msg::SrcChanged, Msg::ShutDown],
        format!("cargo {command}"),
    );

    let mut cmd = Command::new("cargo").args(&args).spawn()?;

    (tokio::select! {
        res = cmd.wait() => res.map(|s|s.success()),
        _ = stop_rx => {
            log::debug!("Stopping cargo {command}...");
            let v = cmd.kill().await.map(|_| true);
            log::debug!("cargo {command} stopped");
            v
        }
    })
    .context(format!("cargo {}", &args.join(" ")))?;
    Ok(())
}
