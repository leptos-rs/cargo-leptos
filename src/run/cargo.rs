use std::collections::HashMap;

use crate::ext::anyhow::{anyhow, Context, Result};
use crate::run::run_config;
use crate::{
    logger::GRAY,
    sync::{run_interruptible, src_or_style_change},
    util::CommandAdditions,
    Config,
};
use tokio::{process::Command, task::JoinHandle};

// for capturing the cargo output see util::CommandAdditions

pub async fn build(config: &Config, lib: bool) -> Result<()> {
    let args = args("build", config, lib);

    let (handle, process) = Command::new("cargo")
        .args(&args)
        .spawn_cargo_parsed()
        .context("Could not spawn command")?;
    run_interruptible(src_or_style_change, "Cargo", process)
        .await
        .context(format!("cargo {}", &args.join(" ")))?;
    handle
        .await
        .map_err(|e| anyhow!("cargo: could not join handle: {e}"))?;
    log::info!(
        "Cargo finished {}",
        GRAY.paint(format!("cargo {}", args.join(" ")))
    );
    Ok(())
}

pub async fn spawn_run(config: &Config, watch: bool) -> JoinHandle<()> {
    let config = config.clone();
    tokio::spawn(async move {
        if let Err(e) = self::run(&config, watch).await {
            log::error!("Cargo error: {e}")
        }
    })
}

pub async fn run(config: &Config, watch: bool) -> Result<()> {
    if watch {
        run_config::remove().await;
        let h = run_config::send_msg_when_created();
        let r = cmd("run", config, false, watch).await.dot();
        let _ = h.await;
        r
    } else {
        cmd("run", config, false, watch).await.dot()
    }
}

pub async fn test(config: &Config) -> Result<()> {
    cmd("test", config, false, false).await.dot()
}

async fn cmd(command: &str, config: &Config, lib: bool, watch: bool) -> Result<()> {
    let args = args(command, config, lib);

    let mut envs: HashMap<String, String> = HashMap::new();

    let rust_env = watch.then(|| "dev").unwrap_or("prod").to_string();
    envs.insert("RUST_ENV".to_string(), rust_env);

    let process = Command::new("cargo")
        .args(&args)
        .envs(envs)
        .spawn()
        .context("Could not spawn command")?;
    run_interruptible(src_or_style_change, "Cargo", process)
        .await
        .context(format!("cargo {}", &args.join(" ")))?;
    log::info!(
        "Cargo finished {}",
        GRAY.paint(format!("cargo {}", args.join(" ")))
    );
    Ok(())
}

fn args<'a>(command: &'a str, config: &Config, lib: bool) -> Vec<&'a str> {
    let mut args = vec![command, "--no-default-features"];

    if lib {
        args.push("--features=hydrate");
        args.push("--lib");
        args.push("--target=wasm32-unknown-unknown");
    } else {
        args.push("--features=ssr");
    }

    config.cli.release.then(|| args.push("--release"));
    args
}
