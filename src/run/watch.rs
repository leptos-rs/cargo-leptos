use crate::{config::Config, Msg, MSG_BUS};
use anyhow::Result;
use notify::{event::ModifyKind, Event, EventKind, RecursiveMode, Watcher};
use std::path::Path;

use super::oneshot_when;

pub async fn run(config: Config) -> Result<()> {
    let cfg = config.clone();
    let mut watcher = notify::recommended_watcher(move |res| match res {
        Ok(event) if is_watched(&event, &cfg) => {
            if let Err(e) = MSG_BUS.send(Msg::SrcChanged) {
                log::error!("watch error: {e}");
            }
        }
        Err(e) => log::error!("watch error: {:?}", e),
        _ => {}
    })?;

    watcher.watch(&Path::new("src"), RecursiveMode::Recursive)?;

    let path = Path::new("style");
    if path.exists() {
        watcher.watch(&path, RecursiveMode::Recursive)?;
    }

    oneshot_when(&[Msg::ShutDown], "watch").await?;
    log::debug!("watch closed");
    Ok(())
}

fn is_watched(event: &Event, cfg: &Config) -> bool {
    match &event.kind {
        EventKind::Modify(ModifyKind::Data(_)) => {}
        _ => return false,
    };

    for path in &event.paths {
        match path.extension().map(|ext| ext.to_str()).flatten() {
            Some("rs") if !path.ends_with(&cfg.leptos.gen_path) => return true,
            Some("css") | Some("scss") => return true,
            _ => {}
        }
    }
    false
}
