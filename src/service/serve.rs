use std::sync::Arc;

use crate::{
    config::Project,
    ext::{anyhow::Result, append_str_to_filename, determine_pdb_filename, fs, Paint},
    logger::GRAY,
    signal::{Interrupt, ReloadSignal, ServerRestart},
};
use crate::internal_prelude::*;
use camino::Utf8PathBuf;
use tokio::{
    process::{Child, Command},
    select,
    task::JoinHandle,
};

pub async fn spawn(proj: &Arc<Project>) -> JoinHandle<Result<()>> {
    let mut int = Interrupt::subscribe_shutdown();
    let proj = proj.clone();
    let mut change = ServerRestart::subscribe();
    tokio::spawn(async move {
        let mut server = ServerProcess::start_new(&proj).await?;
        loop {
            select! {
              res = change.recv() => {
                if let Ok(()) = res {
                      server.restart().await?;
                      ReloadSignal::send_full();
                }
              },
              _ = int.recv() => {
                    server.kill().await;
                    return Ok(())
              },
            }
        }
    })
}

pub async fn spawn_oneshot(proj: &Arc<Project>) -> JoinHandle<Result<()>> {
    let mut int = Interrupt::subscribe_shutdown();
    let proj = proj.clone();
    tokio::spawn(async move {
        let mut server = ServerProcess::start_new(&proj).await?;
        select! {
          _ = server.wait() => {},
          _ = int.recv() => {
                server.kill().await;
          },
        };
        Ok(())
    })
}

struct ServerProcess {
    process: Option<Child>,
    envs: Vec<(&'static str, String)>,
    binary: Utf8PathBuf,
    bin_args: Option<Vec<String>>,
}

impl ServerProcess {
    fn new(proj: &Project) -> Self {
        Self {
            process: None,
            envs: proj.to_envs(),
            binary: proj.bin.exe_file.clone(),
            bin_args: proj.bin.bin_args.clone(),
        }
    }

    async fn start_new(proj: &Project) -> Result<Self> {
        let mut me = Self::new(proj);
        me.start().await?;
        Ok(me)
    }

    async fn kill(&mut self) {
        if let Some(proc) = self.process.as_mut() {
            if let Err(e) = proc.kill().await {
                error!("Serve error killing server process: {e}");
            } else {
                trace!("Serve stopped");
            }
            self.process = None;
        }
    }

    async fn restart(&mut self) -> Result<()> {
        self.kill().await;
        self.start().await?;
        trace!("Serve restarted");
        Ok(())
    }

    async fn wait(&mut self) -> Result<()> {
        if let Some(proc) = self.process.as_mut() {
            if let Err(e) = proc.wait().await {
                error!("Serve error while waiting for server process to exit: {e}");
            } else {
                trace!("Serve process exited");
            }
        }
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        let bin = &self.binary;
        let child = if bin.exists() {
            // windows doesn't like to overwrite a running binary, so we copy it to a new name
            let bin_path = if cfg!(target_os = "windows") {
                // solution to allow cargo to overwrite a running binary on some platforms:
                //   copy cargo's output bin to [filename]_leptos and then run it
                let new_bin_path = append_str_to_filename(bin, "_leptos")?;
                debug!(
                    "Copying server binary {} to {}",
                    GRAY.paint(bin.as_str()),
                    GRAY.paint(new_bin_path.as_str())
                );
                fs::copy(bin, &new_bin_path).await?;
                // also copy the .pdb file if it exists to allow debugging to attach
                if let Some(pdb) = determine_pdb_filename(bin) {
                    let new_pdb_path = append_str_to_filename(&pdb, "_leptos")?;
                    debug!(
                        "Copying server binary debug info {} to {}",
                        GRAY.paint(pdb.as_str()),
                        GRAY.paint(new_pdb_path.as_str())
                    );
                    fs::copy(&pdb, &new_pdb_path).await?;
                }
                new_bin_path
            } else {
                bin.clone()
            };

            let bin_args = match &self.bin_args {
                Some(bin_args) => bin_args.as_slice(),
                None => &[],
            };

            debug!("Serve running {}", GRAY.paint(bin_path.as_str()));
            let cmd = Some(
                Command::new(bin_path)
                    .envs(self.envs.clone())
                    .args(bin_args)
                    .spawn()?,
            );
            let port = self
                .envs
                .iter()
                .find_map(|(k, v)| {
                    if k == &"LEPTOS_SITE_ADDR" {
                        Some(v.to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            info!("Serving at http://{port}");
            cmd
        } else {
            debug!("Serve no exe found {}", GRAY.paint(bin.as_str()));
            None
        };
        self.process = child;
        Ok(())
    }
}
