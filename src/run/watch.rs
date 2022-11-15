use crate::config::Config;
use anyhow::{Context, Result};
use notify::{event::ModifyKind, Event, EventKind, RecursiveMode, Watcher};
use std::{path::PathBuf, sync::mpsc::Sender};

pub async fn run(config: Config, tx: Sender<bool>) -> Result<()> {
    let cfg = config.clone();
    let mut watcher = notify::recommended_watcher(move |res| match res {
        Ok(event) if is_watched(&event, &cfg) => tx.send(true).unwrap(),
        Err(e) => log::error!("watch error: {:?}", e),
        _ => {}
    })?;

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    let path = PathBuf::from(format!("{}/src", config.root));
    watcher.watch(&path, RecursiveMode::Recursive)?;

    let path = PathBuf::from(format!("{}/style", config.root));
    if path.exists() {
        watcher.watch(&path, RecursiveMode::Recursive)?;
    }

    tokio::signal::ctrl_c()
        .await
        .context("error awaiting shutdown signal")?;

    Ok(())
}

fn is_watched(event: &Event, cfg: &Config) -> bool {
    match &event.kind {
        EventKind::Modify(ModifyKind::Data(_)) => {}
        _ => return false,
    };

    for path in &event.paths {
        match path.extension().map(|ext| ext.to_str()).flatten() {
            Some("rs") if !path.ends_with(&cfg.gen_path) => return true,
            Some("css") | Some("scss") => return true,
            _ => {}
        }
    }
    false
}
