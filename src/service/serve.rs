use std::sync::Arc;

use crate::{
    config::Project,
    ext::anyhow::Result,
    logger::GRAY,
    signal::{Interrupt, Product, ProductChange, ReloadSignal},
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
    let mut change = ProductChange::subscribe();
    tokio::spawn(async move {
        // wait for first build to finish even if no products updated
        select! {
            _ = change.recv() => {}
            _ = int.recv() => return Ok(())
        }

        let mut server = ServerProcess::start_new(&proj).await?;
        loop {
            select! {
              res = change.recv() => {
                if let Ok(set) = res {
                  if set.contains(&Product::ServerBin) {
                      server.restart().await?;
                      ReloadSignal::send_full();
                  }
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

struct ServerProcess(Option<Child>, Vec<(&'static str, String)>, Utf8PathBuf);

impl ServerProcess {
    fn new(proj: &Project) -> Self {
        Self(None, proj.to_envs(), proj.paths.cargo_bin_file.clone())
    }

    async fn start_new(proj: &Project) -> Result<Self> {
        let mut me = Self::new(proj);
        me.start().await?;
        Ok(me)
    }

    async fn kill(&mut self) {
        if let Some(proc) = self.0.as_mut() {
            if let Err(e) = proc.kill().await {
                log::error!("Serve error killing server process: {e}");
            } else {
                log::trace!("Serve stopped");
            }
            self.0 = None;
        }
    }

    async fn restart(&mut self) -> Result<()> {
        self.kill().await;
        self.start().await?;
        log::trace!("Serve restarted");
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        let bin = &self.2;
        let child = if bin.exists() {
            log::debug!("Serve running {}", GRAY.paint(bin.as_str()));
            Some(Command::new(bin).envs(self.1.clone()).spawn()?)
        } else {
            log::debug!("Serve no exe found {}", GRAY.paint(bin.as_str()));
            None
        };
        self.0 = child;
        Ok(())
    }
}
