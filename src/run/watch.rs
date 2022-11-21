use crate::{
    config::Config,
    logger::GRAY,
    util::{oneshot_when, PathBufAdditions},
    Msg, MSG_BUS,
};
use anyhow::Result;
use notify::{event::ModifyKind, Event, EventKind, RecursiveMode, Watcher};
use std::path::PathBuf;

pub async fn run(config: Config) -> Result<()> {
    let cfg = config.clone();
    let mut watcher = notify::recommended_watcher(move |res| match res {
        Ok(event) if is_watched(&event, &cfg) => match MSG_BUS.send(Msg::SrcChanged) {
            Err(e) => log::error!("Watch {e}"),
            Ok(_) => {
                log::debug!(
                    "Watch change detected {}",
                    GRAY.paint(
                        event
                            .paths
                            .iter()
                            .map(|f| format!("\"{}\"", f.to_string_lossy()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                )
            }
        },
        Err(e) => log::error!("Watch {:?}", e),
        _ => {}
    })?;

    let src_dir = PathBuf::from("src");
    watcher.watch(&src_dir, RecursiveMode::Recursive)?;

    let style_dir = PathBuf::from(&config.leptos.style.file).without_last();
    // add if not nested
    if !style_dir.starts_with(&src_dir) {
        watcher.watch(&style_dir, RecursiveMode::Recursive)?;
        log::info!("Watching folders {src_dir:?} and {style_dir:?} recursively");
    } else {
        log::info!("Watching folder {src_dir:?} recursively");
    }

    oneshot_when(&[Msg::ShutDown], "Watch").await?;
    log::debug!("Watch closed");
    Ok(())
}

fn is_watched(event: &Event, cfg: &Config) -> bool {
    match &event.kind {
        EventKind::Modify(ModifyKind::Data(_)) => {}
        EventKind::Modify(ModifyKind::Any) => {} // windows throws duplicate Any events
        _ => return false,
    };

    for path in &event.paths {
        match path.extension().map(|ext| ext.to_str()).flatten() {
            Some("rs") if !path.ends_with(&cfg.leptos.gen_path) => return true,
            Some("css") | Some("scss") | Some("sass") => return true,
            _ => {}
        }
    }
    false
}
