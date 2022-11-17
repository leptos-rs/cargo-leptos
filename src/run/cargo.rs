use super::oneshot_when;
use crate::{config::Config, Msg};
use anyhow::{Context, Result};
use tokio::process::{Child, Command};

// for capturing the cargo output see util::CommandAdditions

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
    let args = args(command, config);

    let process = Command::new("cargo")
        .args(&args)
        .spawn()
        .context("Could not spawn command")?;
    run_interruptible(command, process)
        .await
        .context(format!("cargo {}", &args.join(" ")))
}

fn args<'a>(command: &'a str, config: &Config) -> Vec<&'a str> {
    let features = match config.watch {
        true => "--features=ssr,leptos_autoreload",
        false => "--features=ssr",
    };
    let mut args = vec![command, "--no-default-features", features];
    if config.cli.release {
        args.push("--release");
    }
    args
}

async fn run_interruptible(command: &str, mut process: Child) -> Result<()> {
    let stop_rx = oneshot_when(
        &[Msg::SrcChanged, Msg::ShutDown],
        format!("cargo {command}"),
    );
    (tokio::select! {
        res = process.wait() => res.map(|s|s.success()),
        _ = stop_rx => {
            log::debug!("Stopping cargo {command}...");
            let v = process.kill().await.map(|_| true);
            log::debug!("cargo {command} stopped");
            v
        }
    })?;
    Ok(())
}
