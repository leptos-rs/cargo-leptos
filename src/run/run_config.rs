use std::{net::SocketAddr, path::PathBuf, time::Duration};

use regex::Regex;
use tokio::{sync::RwLock, task::JoinHandle, time::sleep};

use crate::ext::{
    anyhow::{anyhow, bail, Context, Result},
    fs,
    sync::{Msg, MSG_BUS},
    util::SenderAdditions,
};

lazy_static::lazy_static! {
  pub static ref RUN_CONFIG: RwLock<RuntimeConfig> = RwLock::new(RuntimeConfig::default());

  static ref RE_RELOAD_PORT: Regex = Regex::new(r#"reload_port\s+(\d+)\n"#).unwrap();
  static ref RE_SERVER_ADDR: Regex = Regex::new(r#"socket_address\s+"(.*)"\n"#).unwrap();

}

const RUN_CONF: &str = ".leptos.kdl";

/// loaded from the .leptos.kdl file
#[derive(knuffel::Decode, Debug)]
enum Param {
    PkgPath(#[knuffel(argument)] String),
    Environment(#[knuffel(argument)] String),
    SocketAddress(#[knuffel(argument, str)] std::net::SocketAddr),
    ReloadPort(#[knuffel(argument)] u16),
}
#[derive(knuffel::Decode, Debug)]
pub struct RenderOptions {
    #[knuffel(children)]
    options: Vec<Param>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub reload_port: u16,
    pub server_addr: SocketAddr,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            reload_port: 0,
            server_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
        }
    }
}
impl RuntimeConfig {
    pub fn load() -> Result<Self> {
        let text =
            std::fs::read_to_string(RUN_CONF).context(format!("Could not read {RUN_CONF}"))?;
        let render_options_vec = knuffel::parse::<Vec<RenderOptions>>(RUN_CONF, &text)
            .context(format!("Could not parse {RUN_CONF}"))?;

        if render_options_vec.len() != 1 {
            bail!("There should be only one RenderOptions section in {RUN_CONF}");
        }
        let render_options = &render_options_vec[0];
        let reload_port = *render_options
            .options
            .iter()
            .find_map(|o| match o {
                Param::ReloadPort(p) => Some(p),
                _ => None,
            })
            .ok_or_else(|| anyhow!("Missing reload_port parameter"))?;
        let server_addr = *render_options
            .options
            .iter()
            .find_map(|o| match o {
                Param::SocketAddress(p) => Some(p),
                _ => None,
            })
            .ok_or_else(|| anyhow!("Missing socket_address parameter"))?;

        Ok(RuntimeConfig {
            reload_port,
            server_addr,
        })
    }
}

pub async fn remove() {
    if PathBuf::from(RUN_CONF).exists() {
        if let Err(e) = fs::remove_file(RUN_CONF).await {
            log::error!("Config could not remove {RUN_CONF} due to: {e}")
        }
    }
}
pub fn send_msg_when_created() -> JoinHandle<()> {
    tokio::spawn(async {
        let path = PathBuf::from(RUN_CONF);
        let duration = Duration::from_millis(50);
        for _ in 0..400 {
            if path.exists() {
                match RuntimeConfig::load() {
                    Ok(conf) => {
                        let mut current = RUN_CONFIG.write().await;
                        if current.reload_port != conf.reload_port {
                            *current = conf;
                            log::debug!("Config run config changed");
                            MSG_BUS.send_logged("Config", Msg::RunConfigChanged);
                        }
                    }
                    Err(e) => {
                        log::error!("Config could not be loaded from {RUN_CONF} due to: {e}");
                    }
                }
                return;
            }
            sleep(duration).await;
        }
        log::error!("Config timed out waiting for {RUN_CONF}");
    })
}
