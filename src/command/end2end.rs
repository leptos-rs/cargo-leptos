use std::sync::Arc;

use camino::Utf8Path;
use tokio::process::Command;

use crate::config::{Config, Project};
use crate::ext::anyhow::{anyhow, Context, Result};
use crate::service::serve;
use crate::signal::Interrupt;

pub async fn end2end_all(conf: &Config) -> Result<()> {
    for proj in &conf.projects {
        end2end_proj(proj).await?;
    }
    Ok(())
}

pub async fn end2end_proj(proj: &Arc<Project>) -> Result<()> {
    if let Some(e2e) = &proj.end2end {
        super::build::build_proj(proj).await.dot()?;
        if Interrupt::is_shutdown_requested().await {
            return Ok(());
        }
        let server = serve::spawn(proj).await;
        try_run(&e2e.cmd, &e2e.dir)
            .await
            .context(format!("running: {}", &e2e.cmd))?;
        Interrupt::request_shutdown().await;
        server.await.dot()??;
    } else {
        log::info!("end2end the Crate.toml package.metadata.leptos.end2end_cmd parameter not set")
    }
    Ok(())
}

async fn try_run(cmd: &str, dir: &Utf8Path) -> Result<()> {
    let mut parts = cmd.split(' ');
    let exe = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid command {cmd:?}"))?;

    let args = parts.collect::<Vec<_>>();

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
