use std::path::PathBuf;

use crate::{
    ext::path::PathExt,
    logger::BOLD,
    sync::{run_interruptible, shutdown_msg},
    Config,
};
use anyhow_ext::{anyhow, bail, Context, Result};
use tokio::process::Command;

pub async fn run(config: &Config) -> Result<()> {
    if let Some(e2e) = &config.leptos.end2end_test_cmd {
        try_run(e2e)
            .await
            .context(format!("Could not run command {e2e:?}"))
    } else {
        bail!(
            "Missing setting {} in {} section {}",
            BOLD.paint("end2end_test_cmd"),
            BOLD.paint("Cargo.toml"),
            BOLD.paint("[package.metadata.leptos]"),
        )
    }
}

async fn try_run(cmd: &str) -> Result<()> {
    let mut parts = cmd.split(' ');
    let exe = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid command {cmd:?}"))?;

    let args = parts.collect::<Vec<_>>();

    let dir = PathBuf::from("end2end").to_canonicalized()?;

    log::trace!("End2End Running {cmd:?}");
    let process = Command::new(exe)
        .args(args)
        .current_dir(dir)
        .spawn()
        .context(format!("Could not spawn command {cmd:?}"))?;

    run_interruptible(shutdown_msg, "Test", process).await
}
