use crate::{
    config::Config,
    logger::GRAY,
    util::{oneshot_when, PathBufAdditions, SenderAdditions},
    Msg, MSG_BUS,
};
use anyhow::Result;
use notify::{DebouncedEvent, RecursiveMode, Watcher};
use std::{path::PathBuf, time::Duration};
use tokio::task::JoinHandle;

pub async fn spawn(config: &Config) -> Result<JoinHandle<()>> {
    let src_dir = PathBuf::from("src").canonicalize()?;
    let style_dir = PathBuf::from(&config.leptos.style.file)
        .without_last()
        .canonicalize()?;
    let paths = if !style_dir.starts_with(&src_dir) {
        log::info!("Watching folders {src_dir:?} and {style_dir:?} recursively");
        vec![src_dir, style_dir]
    } else {
        log::info!("Watching folder {src_dir:?} recursively");
        vec![src_dir]
    };

    let exclude = vec![PathBuf::from(&config.leptos.gen_file).canonicalize()?];

    Ok(tokio::spawn(async move { run(&paths, exclude).await }))
}

async fn run(paths: &[PathBuf], exclude: Vec<PathBuf>) {
    let (sync_tx, sync_rx) = std::sync::mpsc::channel::<DebouncedEvent>();

    use DebouncedEvent::{
        Chmod, Create, Error, NoticeRemove, NoticeWrite, Remove, Rename, Rescan, Write,
    };
    std::thread::spawn(move || {
        while let Ok(event) = sync_rx.recv() {
            match event {
                NoticeWrite(_) | NoticeRemove(_) | Chmod(_) => {}
                Remove(f) | Rename(f, _) | Write(f) | Create(f) => {
                    if is_watched(&f, &exclude) {
                        log::debug!("Watcher file changed {}", GRAY.paint(f.to_string_lossy()));
                        MSG_BUS.send_logged("Watcher", Msg::SrcChanged)
                    }
                }
                Error(e, p) => log::error!("Watcher {e} {p:?}"),
                Rescan => {
                    log::debug!("Watcher rescan");
                    MSG_BUS.send_logged("Watcher", Msg::SrcChanged)
                }
            }
        }
        log::debug!("Watching stopped");
    });

    let mut watcher = notify::watcher(sync_tx, Duration::from_millis(200))
        .expect("failed to build file system watcher");

    for path in paths {
        if let Err(e) = watcher.watch(&path, RecursiveMode::Recursive) {
            log::error!("Watcher could not watch {path:?} due to {e}");
        }
    }

    if let Err(e) = oneshot_when(&[Msg::ShutDown], "Watch").await {
        log::trace!("Watcher stopped due to: {e}");
    }
}

fn is_watched(path: &PathBuf, exclude: &[PathBuf]) -> bool {
    match path.extension().map(|ext| ext.to_str()).flatten() {
        Some("rs") if !exclude.contains(path) => true,
        Some("css") | Some("scss") | Some("sass") => true,
        _ => false,
    }
}
