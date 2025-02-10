use crate::ext::anyhow::{bail, Context, Result};
use crate::internal_prelude::*;
use std::{
    net::SocketAddr,
    process::{Output, Stdio},
    time::Duration,
};
use tokio::{
    net::TcpStream,
    process::{Child, Command},
    sync::broadcast,
    time::sleep,
};

pub trait OutputExt {
    fn stderr(&self) -> String;
    fn has_stderr(&self) -> bool;
    fn stdout(&self) -> String;
    fn has_stdout(&self) -> bool;
}

impl OutputExt for Output {
    fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.stderr).to_string()
    }

    fn has_stderr(&self) -> bool {
        println!("stderr: {}\n'{}'", self.stderr.len(), self.stderr());
        self.stderr.len() > 1
    }

    fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.stdout).to_string()
    }

    fn has_stdout(&self) -> bool {
        self.stdout.len() > 1
    }
}
pub enum CommandResult<T> {
    Success(T),
    Failure(T),
    Interrupted,
}

pub async fn wait_interruptible(
    name: &str,
    mut process: Child,
    mut interrupt_rx: broadcast::Receiver<()>,
) -> Result<CommandResult<()>> {
    tokio::select! {
        res = process.wait() => match res {
            Ok(exit) => {
                if exit.success() {
                    trace!("{name} process finished with success");
                    Ok(CommandResult::Success(()))
                } else {
                    trace!("{name} process finished with code {:?}", exit.code());
                    Ok(CommandResult::Failure(()))
                }
            }
            Err(e) => bail!("Command failed due to: {e}"),
        },
        _ = interrupt_rx.recv() => {
            process.kill().await.context("Could not kill process")?;
            trace!("{name} process interrupted");
            Ok(CommandResult::Interrupted)
        }
    }
}

pub async fn wait_piped_interruptible(
    name: &str,
    mut cmd: Command,
    mut interrupt_rx: broadcast::Receiver<()>,
) -> Result<CommandResult<Output>> {
    // see: https://docs.rs/tokio/latest/tokio/process/index.html

    cmd.kill_on_drop(true);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let process = cmd.spawn()?;
    tokio::select! {
        res = process.wait_with_output() => match res {
            Ok(output) => {
                if output.status.success() {
                    trace!("{name} process finished with success");
                    Ok(CommandResult::Success(output))
                } else {
                    trace!("{name} process finished with code {:?}", output.status.code());
                    Ok(CommandResult::Failure(output))
                }
            }
            Err(e) => bail!("Command failed due to: {e}"),
        },
        _ = interrupt_rx.recv() => {
            trace!("{name} process interrupted");
            Ok(CommandResult::Interrupted)
        }
    }
}
pub async fn wait_for_socket(name: &str, addr: SocketAddr) -> bool {
    let duration = Duration::from_millis(500);

    for _ in 0..20 {
        if TcpStream::connect(&addr).await.is_ok() {
            debug!("{name} server port {addr} open");
            return true;
        }
        sleep(duration).await;
    }
    warn!("{name} timed out waiting for port {addr}");
    false
}
