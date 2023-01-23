use std::sync::Arc;

use crate::{
    config::Project,
    ext::{anyhow::Result, append_str_to_filename, determine_pdb_filename, fs},
    logger::GRAY,
    signal::{Interrupt, ReloadSignal, ServerRestart},
};
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

struct ServerProcess {
    process: Option<Child>,
    envs: Vec<(&'static str, String)>,
    binary: Utf8PathBuf,
}

impl ServerProcess {
    fn new(proj: &Project) -> Self {
        Self {
            process: None,
            envs: proj.to_envs(),
            binary: proj.bin.exe_file.clone(),
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
                log::error!("Serve error killing server process: {e}");
            } else {
                log::trace!("Serve stopped");
            }
            self.process = None;
        }
    }

    async fn restart(&mut self) -> Result<()> {
        self.kill().await;
        self.start().await?;
        log::trace!("Serve restarted");
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        let bin = &self.binary;
        let child = if bin.exists() {
            // solution to allow cargo to overwrite a running binary on some platforms:
            //   copy cargo's output bin to [filename]_leptos and then run it
            let new_bin_path = append_str_to_filename(bin, "_leptos")?;
            log::debug!(
                "Copying server binary {} to {}",
                GRAY.paint(bin.as_str()),
                GRAY.paint(new_bin_path.as_str())
            );
            fs::copy(bin, &new_bin_path).await?;
            // also copy the .pdb file if it exists to allow debugging to attach
            match determine_pdb_filename(bin) {
                Some(pdb) => {
                    let new_pdb_path = append_str_to_filename(&pdb, "_leptos")?;
                    log::debug!(
                        "Copying server binary debug info {} to {}",
                        GRAY.paint(pdb.as_str()),
                        GRAY.paint(new_pdb_path.as_str())
                    );
                    fs::copy(&pdb, &new_pdb_path).await?;
                }
                None => {}
            }
            log::debug!("Serve running {}", GRAY.paint(new_bin_path.as_str()));
            Some(Command::new(new_bin_path).envs(self.envs.clone()).spawn()?)
        } else {
            log::debug!("Serve no exe found {}", GRAY.paint(bin.as_str()));
            None
        };
        self.process = child;
        Ok(())
    }
}
