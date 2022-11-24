use crate::{
    logger::GRAY,
    sync::{run_interruptible, src_or_style_change},
    util::CommandAdditions,
    Config,
};
use anyhow_ext::{Context, Result};
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
    handle.await?;
    log::info!(
        "Cargo finished {}",
        GRAY.paint(format!("cargo {}", args.join(" ")))
    );
    Ok(())
}

pub async fn spawn_run(config: &Config) -> JoinHandle<()> {
    let config = config.clone();
    tokio::spawn(async move {
        if let Err(e) = self::run(&config).await {
            log::error!("Cargo error: {e}")
        }
    })
}

pub async fn run(config: &Config) -> Result<()> {
    cmd("run", config, false).await.dot()
}

pub async fn test(config: &Config) -> Result<()> {
    cmd("test", config, false).await.dot()
}

async fn cmd(command: &str, config: &Config, lib: bool) -> Result<()> {
    let args = args(command, config, lib);

    let process = Command::new("cargo")
        .args(&args)
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
    let features = match (lib, config.cli.csr, config.watch) {
        (false, _, true) => "--features=ssr,leptos_autoreload",
        (false, _, false) => "--features=ssr",
        (true, false, true) => "--features=hydrate,leptos_autoreload",
        (true, false, false) => "--features=hydrate",
        (true, true, true) => "--features=csr,leptos_autoreload",
        (true, true, false) => "--features=csr",
    };
    let mut args = vec![command, "--no-default-features", features];

    if lib {
        args.push("--lib");
        args.push("--target=wasm32-unknown-unknown");
    }

    config.cli.release.then(|| args.push("--release"));
    args
}
