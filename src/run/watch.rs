use crate::{
    fs::remove_nested,
    logger::GRAY,
    path::{PathBufExt, PathExt},
    sync::{oneshot_when, shutdown_msg},
    util::{SenderAdditions, StrAdditions},
    Config, Msg, MSG_BUS,
};
use anyhow_ext::{Context, Result};
use notify::{DebouncedEvent, RecursiveMode, Watcher};
use std::{fmt::Display, path::PathBuf, time::Duration};
use tokio::task::JoinHandle;

pub async fn spawn(config: &Config) -> Result<JoinHandle<()>> {
    let mut paths = vec!["src".to_canoncial_dir()?];
    paths.push(
        PathBuf::from(&config.leptos.style.file)
            .without_last()
            .to_canonicalized()
            .dot()?,
    );

    let assets_dir = if let Some(dir) = &config.leptos.assets_dir {
        let assets_root = dir.to_canoncial_dir().dot()?;
        paths.push(assets_root.clone());
        Some(assets_root)
    } else {
        None
    };

    let paths = remove_nested(paths);

    log::info!(
        "Watching folders {}",
        GRAY.paint(
            paths
                .iter()
                .map(|p| p.to_string_lossy())
                .collect::<Vec<_>>()
                .join(", ")
        )
    );

    let exclude = vec![PathBuf::from(&config.leptos.gen_file).to_canonicalized()?];

    Ok(tokio::spawn(async move {
        run(&paths, exclude, assets_dir).await
    }))
}

async fn run(paths: &[PathBuf], exclude: Vec<PathBuf>, assets_dir: Option<PathBuf>) {
    let (sync_tx, sync_rx) = std::sync::mpsc::channel::<DebouncedEvent>();

    std::thread::spawn(move || {
        while let Ok(event) = sync_rx.recv() {
            if let Some(watched) = Watched::try_new(event) {
                handle(watched, &exclude, &assets_dir)
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

    if let Err(e) = oneshot_when(shutdown_msg, "Watch").await {
        log::trace!("Watcher stopped due to: {e}");
    }
}

fn handle(watched: Watched, exclude: &[PathBuf], assets_dir: &Option<PathBuf>) {
    if let Some(path) = watched.path() {
        if exclude.contains(path) {
            return;
        }
    }

    if let Some(assets_dir) = assets_dir {
        if watched.path_starts_with(assets_dir) {
            log::debug!("Watcher asset change {}", GRAY.paint(watched.to_string()));
            MSG_BUS.send_logged("Watcher", Msg::AssetsChanged(watched));
            return;
        }
    }

    match watched.path_ext() {
        Some("rs") => {
            log::debug!("Watcher source change {}", GRAY.paint(watched.to_string()));
            MSG_BUS.send_logged("Watcher", Msg::SrcChanged)
        }
        Some(ext) if ["scss", "sass", "css"].contains(&ext.to_lowercase().as_str()) => {
            log::debug!("Watcher style change {}", GRAY.paint(watched.to_string()));
            MSG_BUS.send_logged("Watcher", Msg::StyleChanged)
        }
        _ => {}
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Watched {
    Remove(PathBuf),
    Rename(PathBuf, PathBuf),
    Write(PathBuf),
    Create(PathBuf),
    Rescan,
}

impl Watched {
    fn try_new(event: DebouncedEvent) -> Option<Self> {
        use DebouncedEvent::{
            Chmod, Create, Error, NoticeRemove, NoticeWrite, Remove, Rename, Rescan, Write,
        };

        match event {
            Chmod(_) | NoticeRemove(_) | NoticeWrite(_) => None,
            Create(f) => Some(Self::Create(f)),
            Remove(f) => Some(Self::Remove(f)),
            Rename(f, t) => Some(Self::Rename(f, t)),
            Write(f) => Some(Self::Write(f)),
            Rescan => Some(Self::Rescan),
            Error(e, Some(p)) => {
                log::error!("Watcher error watching {p:?}: {e}");
                None
            }
            Error(e, None) => {
                log::error!("Watcher error: {e}");
                None
            }
        }
    }

    pub fn path_ext(&self) -> Option<&str> {
        self.path()
            .map(|p| p.extension().map(|e| e.to_str()))
            .flatten()
            .flatten()
    }

    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            Self::Remove(p) | Self::Rename(p, _) | Self::Write(p) | Self::Create(p) => Some(&p),
            Self::Rescan => None,
        }
    }

    pub fn path_starts_with(&self, path: &PathBuf) -> bool {
        match self {
            Self::Write(p) | Self::Create(p) | Self::Remove(p) => p.starts_with(path),
            Self::Rename(fr, to) => fr.starts_with(path) || to.starts_with(path),
            Self::Rescan => false,
        }
    }
}

impl Display for Watched {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Create(p) => write!(f, "create {p:?}"),
            Self::Remove(p) => write!(f, "remove {p:?}"),
            Self::Write(p) => write!(f, "write {p:?}"),
            Self::Rename(fr, to) => write!(f, "rename {fr:?} -> {to:?}"),
            Self::Rescan => write!(f, "rescan"),
        }
    }
}
