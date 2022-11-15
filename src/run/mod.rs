pub mod cargo;
#[allow(dead_code)]
mod generated;
mod html;
pub mod reload;
pub mod sass;
pub mod serve;
pub mod wasm_pack;
pub mod watch;

pub use html::Html;
use tokio::sync::oneshot;

use crate::{Msg, MSG_BUS};

pub fn oneshot_when<S: ToString>(msgs: &'static [Msg], to: S) -> oneshot::Receiver<()> {
    let (tx, rx) = oneshot::channel::<()>();

    let mut interrupt = MSG_BUS.subscribe();

    let to = to.to_string();
    tokio::spawn(async move {
        loop {
            match interrupt.recv().await {
                Ok(Msg::ShutDown) => break,
                Ok(msg) if msgs.contains(&msg) => {
                    if let Err(_) = tx.send(()) {
                        log::debug!("Could not send {msg:?} to {to}");
                    }
                    return;
                }
                Err(e) => {
                    log::debug!("Error recieving from MSG_BUS: {e}");
                    return;
                }
                Ok(_) => {}
            }
        }
    });

    rx
}
