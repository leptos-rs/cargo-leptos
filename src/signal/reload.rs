use leptos_hot_reload::diff::Patches;
use tokio::sync::broadcast;

lazy_static::lazy_static! {
  static ref RELOAD_CHANNEL: broadcast::Sender::<ReloadType> = broadcast::channel::<ReloadType>(1).0;
}

#[derive(Debug, Clone)]
pub enum ReloadType {
    Full,
    Style,
    ViewPatches(String),
}

pub struct ReloadSignal {}

impl ReloadSignal {
    pub fn send_full() {
        if let Err(e) = RELOAD_CHANNEL.send(ReloadType::Full) {
            log::error!(r#"Error could not send reload "Full" due to: {e}"#);
        }
    }
    pub fn send_style() {
        if let Err(e) = RELOAD_CHANNEL.send(ReloadType::Style) {
            log::error!(r#"Error could not send reload "Style" due to: {e}"#);
        }
    }

    pub fn send_view_patches(view_patches: &Patches) {
        match serde_json::to_string(view_patches) {
            Ok(data) => {
                if let Err(e) = RELOAD_CHANNEL.send(ReloadType::ViewPatches(data)) {
                    log::error!(r#"Error could not send reload "View Patches" due to: {e}"#);
                }
            }
            Err(e) => log::error!(r#"Error could not send reload "View Patches" due to: {e}"#),
        }
    }

    pub fn subscribe() -> broadcast::Receiver<ReloadType> {
        RELOAD_CHANNEL.subscribe()
    }
}
