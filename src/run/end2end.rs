use std::path::PathBuf;

use crate::{
    config::Config,
    sync::{run_interruptible, shutdown_msg},
};
use ansi_term::Style;
use anyhow_ext::{anyhow, bail, Context, Result};
use tokio::process::Command;

pub async fn run(config: &Config) -> Result<()> {
    if let Some(e2e) = &config.leptos.end2end_test_cmd {
        try_run(e2e)
            .await
            .context(format!("Could not run command {e2e:?}"))
    } else {
        let bold = Style::new().bold();
        bail!(
            "Missing setting {} in {} section {}",
            bold.paint("end2end_test_cmd"),
            bold.paint("Cargo.toml"),
            bold.paint("[package.metadata.leptos]"),
        )
    }
}

async fn try_run(cmd: &str) -> Result<()> {
    let mut parts = cmd.split(' ');
    let exe = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid command {cmd:?}"))?;

    let args = parts.collect::<Vec<_>>();

    let dir = PathBuf::from("end2end")
        .canonicalize()
        .context(r#"Iusse with sub dir "end2end""#)?;

    log::trace!("End2End Running {cmd:?}");
    let process = Command::new(exe)
        .args(args)
        .current_dir(dir)
        .spawn()
        .context(format!("Could not spawn command {cmd:?}"))?;

    run_interruptible(shutdown_msg, "Test", process).await
}
