use std::sync::Arc;

use camino::Utf8PathBuf;
use tokio::process::Command;

use crate::config::{Config, Project};
use crate::ext::anyhow::{anyhow, Context, Result};
use crate::service::serve;
use crate::signal::{Interrupt, ProductChange, ProductSet};

pub async fn end2end_all(conf: &Config) -> Result<()> {
    for proj in &conf.projects {
        end2end_proj(proj).await?;
    }
    Ok(())
}

pub async fn end2end_proj(proj: &Arc<Project>) -> Result<()> {
    if let Some(e2e_cmd) = &proj.config.end2end_cmd {
        super::build::build_proj(proj).await.dot()?;
        let server = serve::spawn(proj).await;
        // the server waits for the first product change before starting
        ProductChange::send(ProductSet::empty());
        try_run(e2e_cmd)
            .await
            .context(format!("running: {e2e_cmd}"))?;
        Interrupt::request_shutdown().await;
        server.await.dot()??;
    } else {
        log::info!("end2end the Crate.toml package.metadata.leptos.end2end_cmd parameter not set")
    }
    Ok(())
}

async fn try_run(cmd: &str) -> Result<()> {
    let mut parts = cmd.split(' ');
    let exe = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid command {cmd:?}"))?;

    let args = parts.collect::<Vec<_>>();

    let dir = Utf8PathBuf::from("end2end").canonicalize_utf8()?;

    log::trace!("End2End running {cmd:?}");
    let mut process = Command::new(exe)
        .args(args)
        .current_dir(dir)
        .spawn()
        .context(format!("Could not spawn command {cmd:?}"))?;

    let mut int = Interrupt::subscribe_any();
    tokio::select! {
      _ = int.recv() => {},
      _ = process.wait() => {}
    }
    Ok(())
}
