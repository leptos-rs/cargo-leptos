use crate::config::Config;
use anyhow::{Context, Result};
use notify::{event::ModifyKind, Event, EventKind, RecursiveMode, Watcher};
use std::{path::PathBuf, sync::mpsc::Sender};

pub async fn run(config: Config, tx: Sender<bool>) -> Result<()> {
    // Automatically select the best implementation for your platform.
    let mut watcher = notify::recommended_watcher(move |res| event_handler(res, &tx))?;

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

fn event_handler(res: notify::Result<Event>, tx: &Sender<bool>) {
    match res {
        Ok(event) if is_watched(&event) => tx.send(true).unwrap(),
        Err(e) => println!("watch error: {:?}", e),
        _ => {}
    }
}

fn is_watched(event: &Event) -> bool {
    match &event.kind {
        EventKind::Modify(ModifyKind::Data(_)) => {}
        _ => return false,
    };

    for path in &event.paths {
        match path.extension().map(|ext| ext.to_str()).flatten() {
            Some("rs") | Some("css") | Some("scss") => return true,
            _ => {}
        }
    }
    false
}
