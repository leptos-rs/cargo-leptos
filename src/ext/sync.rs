use crate::ext::anyhow::{bail, Context, Result};
use std::{net::SocketAddr, time::Duration};
use tokio::{net::TcpStream, process::Child, sync::broadcast, time::sleep};

/// return false if interrupted or if exit code wasn't success.
pub async fn wait_interruptible(
    name: &str,
    mut process: Child,
    mut interrupt_rx: broadcast::Receiver<()>,
) -> Result<bool> {
    tokio::select! {
        res = process.wait() => match res {
            Ok(exit) => {
                if exit.success() {
                    log::trace!("{name} process finished with success");
                    Ok(true)
                } else {
                    log::trace!("{name} process finished with code {:?}", exit.code());
                    Ok(false)
                }
            }
            Err(e) => bail!("Command failed due to: {e}"),
        },
        _ = interrupt_rx.recv() => {
            process.kill().await.context("Could not kill process")?;
            log::trace!("{name} process interrupted");
            Ok(false)
        }
    }
}

pub async fn wait_for_socket(name: &str, addr: SocketAddr) -> bool {
    let duration = Duration::from_millis(500);

    for _ in 0..20 {
        if let Ok(_) = TcpStream::connect(&addr).await {
            log::debug!("{name} server port {addr} open");
            return true;
        }
        sleep(duration).await;
    }
    log::warn!("{name} timed out waiting for port {addr}");
    false
}
