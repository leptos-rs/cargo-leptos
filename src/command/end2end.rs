use std::sync::Arc;

use camino::Utf8Path;
use tokio::process::Command;

use crate::config::{Config, Project};
use crate::internal_prelude::*;
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
        if !super::build::build_proj(proj).await.dot()? {
            return Ok(());
        }

        let server = serve::spawn(proj).await;
        try_run(&e2e.cmd, &e2e.dir)
            .await
            .wrap_err(format!("running: {}", &e2e.cmd))?;
        Interrupt::request_shutdown().await;
        server.await.dot()??;
    } else {
        info!("end2end the Crate.toml package.metadata.leptos.end2end_cmd parameter not set")
    }
    Ok(())
}

async fn try_run(cmd: &str, dir: &Utf8Path) -> Result<()> {
    let mut parts = cmd.split(' ');
    let exe = parts
        .next()
        .ok_or_else(|| eyre!("Invalid command {cmd:?}"))?;

    let args = parts.collect::<Vec<_>>();

    trace!("End2End running {cmd:?}");
    let mut process = Command::new(exe)
        .args(args)
        .current_dir(dir)
        .spawn()
        .wrap_err(format!("Could not spawn command {cmd:?}"))?;

    let mut int = Interrupt::subscribe_any();

    tokio::select! {
          _ = int.recv() => {
            match process.try_wait() {
                Ok(None) | Err(_) => {
                    trace!("End2End child process still alive; attempting to kill...");
                    match process.kill().await {
                        Err(e) => warn!("Could not kill End2End child process: {}", e),
                        Ok(_) => trace!("Successfully killed End2End child process"),
                    }
                }
                Ok(Some(status)) => trace!("End2End child process already exited with {status}"),
            }
            
            Ok(())
        },
          result = process.wait() => {
            let status = result?;
            if !status.success() {
                bail!("Command terminated with exit code {}", status)
            }
            Ok(())
        }
    }
}
