use camino::Utf8PathBuf;
use tokio::{
    process::{Child, Command},
    select,
    task::JoinHandle,
};

use crate::{
    command::{
        is_shutdown_requested, send_reload, subscribe_interrupt, subscribe_product_changes,
        ReloadType,
    },
    config::Config,
    ext::anyhow::Result,
    logger::GRAY,
    task::compile::Product,
};

pub async fn run(conf: &Config) -> JoinHandle<Result<()>> {
    let mut int = subscribe_interrupt();
    let conf = conf.clone();
    let mut change = subscribe_product_changes();
    tokio::spawn(async move {
        // wait for first build to finish even if no products updated
        select! {
            _ = change.recv() => {}
            _ = int.recv() => return Ok(())
        }

        let mut server = ServerProcess::start_new(&conf).await?;
        loop {
            select! {
              res = change.recv() => {
                if let Ok(set) = res {
                  if set.contains(&Product::ServerBin) {
                      server.restart().await?;
                      send_reload(ReloadType::Full);
                  }
                }
              },
              _ = int.recv() => {
                if is_shutdown_requested().await {
                    server.kill().await;
                    return Ok(())
                }
              },
            }
        }
    })
}

struct ServerProcess(Option<Child>, Vec<(&'static str, String)>, Utf8PathBuf);

impl ServerProcess {
    fn new(conf: &Config) -> Self {
        Self(None, conf.to_envs(), conf.cargo_bin_file())
    }

    async fn start_new(conf: &Config) -> Result<Self> {
        let mut me = Self::new(conf);
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
            Some(Command::new(&bin).envs(self.1.clone()).spawn()?)
        } else {
            log::debug!("Serve no exe found {}", GRAY.paint(bin.as_str()));
            None
        };
        self.0 = child;
        Ok(())
    }
}
