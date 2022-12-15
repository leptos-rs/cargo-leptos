use tokio::{
    signal,
    sync::{broadcast, RwLock},
    task::JoinHandle,
};

use crate::compile::{Change, ChangeSet};

lazy_static::lazy_static! {
  static ref ANY_INTERRUPT: broadcast::Sender<()> = broadcast::channel(1).0;
  static ref SHUTDOWN: broadcast::Sender<()> = broadcast::channel(1).0;

  static ref SHUTDOWN_REQUESTED: RwLock<bool> = RwLock::new(false);
  static ref SOURCE_CHANGES: RwLock<ChangeSet> = RwLock::new(ChangeSet::default());
}

pub struct Interrupt {}

impl Interrupt {
    pub async fn is_shutdown_requested() -> bool {
        *SHUTDOWN_REQUESTED.read().await
    }

    pub fn subscribe_any() -> broadcast::Receiver<()> {
        ANY_INTERRUPT.subscribe()
    }

    pub fn subscribe_shutdown() -> broadcast::Receiver<()> {
        SHUTDOWN.subscribe()
    }

    pub async fn get_source_changes() -> ChangeSet {
        SOURCE_CHANGES.read().await.clone()
    }

    pub async fn clear_source_changes() {
        let mut ch = SOURCE_CHANGES.write().await;
        ch.clear();
        log::trace!("Interrupt source changed cleared");
    }

    pub fn send(change: Change) {
        let mut ch = SOURCE_CHANGES.blocking_write();
        let did_change = ch.add(change);
        drop(ch);

        if did_change {
            Self::send_any();
        } else {
            log::trace!("Interrupt no change");
        }
    }

    fn send_any() {
        if let Err(e) = ANY_INTERRUPT.send(()) {
            log::error!("Interrupt error could not send due to: {e}");
        } else {
            log::trace!("Interrupt send done");
        }
    }

    pub async fn request_shutdown() {
        {
            *SHUTDOWN_REQUESTED.write().await = true;
        }
        _ = SHUTDOWN.send(());
        _ = ANY_INTERRUPT.send(());
    }

    pub fn run_ctrl_c_monitor() -> JoinHandle<()> {
        tokio::spawn(async move {
            signal::ctrl_c().await.expect("failed to listen for event");
            log::info!("Leptos ctrl-c received");
            Interrupt::request_shutdown().await;
        })
    }
}
