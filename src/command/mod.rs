pub mod build;
pub mod end2end;
pub mod serve;
pub mod test;
pub mod watch;

use tokio::{
    signal,
    sync::{broadcast, RwLock},
    task::JoinHandle,
};

use crate::task::{compile::ProductSet, Change, ChangeSet};

lazy_static::lazy_static! {
  static ref INTERRUPT: broadcast::Sender<()> = broadcast::channel(1).0;
  static ref SOURCE_CHANGES: RwLock<ChangeSet> = RwLock::new(ChangeSet::new());
  static ref SHUTDOWN_REQUESTED: RwLock<bool> = RwLock::new(false);
  static ref PRODUCT_CHANGE_CHANNEL: broadcast::Sender::<ProductSet> = broadcast::channel::<ProductSet>(1).0;
  static ref RELOAD_CHANNEL: broadcast::Sender::<ReloadType> = broadcast::channel::<ReloadType>(1).0;
}

#[derive(Debug, Clone)]
pub enum ReloadType {
    Full,
    Style,
}
pub fn send_reload(ty: ReloadType) {
    if let Err(e) = RELOAD_CHANNEL.send(ty.clone()) {
        log::error!(r#"Error could not send reload "{ty:?}" due to: {e}"#);
    }
}

pub fn subscribe_reload() -> broadcast::Receiver<ReloadType> {
    RELOAD_CHANNEL.subscribe()
}

pub fn subscribe_product_changes() -> broadcast::Receiver<ProductSet> {
    PRODUCT_CHANGE_CHANNEL.subscribe()
}

pub fn send_product_change(set: ProductSet) {
    if let Err(e) = PRODUCT_CHANGE_CHANNEL.send(set) {
        log::error!("Error could not send product changes due to {e}")
    }
}

pub async fn is_shutdown_requested() -> bool {
    *SHUTDOWN_REQUESTED.read().await
}

pub async fn request_shutdown() {
    {
        *SHUTDOWN_REQUESTED.write().await = true;
    }
    _ = INTERRUPT.send(());
}

pub fn subscribe_interrupt() -> broadcast::Receiver<()> {
    INTERRUPT.subscribe()
}

pub fn send_interrupt() {
    log::trace!("Watch send interrupt");
    if let Err(e) = INTERRUPT.send(()) {
        log::error!("Watch could not send interrupt: {e}");
    }
}

pub async fn get_source_changes() -> ChangeSet {
    SOURCE_CHANGES.read().await.clone()
}

pub async fn clear_source_changes() {
    let mut ch = SOURCE_CHANGES.write().await;
    ch.clear();
    log::trace!("Watch source changed cleared");
}

pub fn send_source_change(change: Change) {
    let mut ch = SOURCE_CHANGES.blocking_write();
    let changed = ch.add(change);
    drop(ch);
    if changed {
        send_interrupt();
    }
}

pub fn ctrl_c_monitor() -> JoinHandle<()> {
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("failed to listen for event");
        log::info!("Leptos ctrl-c received");
        request_shutdown().await;
    })
}
