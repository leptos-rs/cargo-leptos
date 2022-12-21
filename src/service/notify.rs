use crate::compile::Change;
use crate::ext::anyhow::{anyhow, Result};
use crate::signal::Interrupt;
use crate::{
    config::Config,
    logger::GRAY,
    path::{remove_nested, PathBufExt},
    util::StrAdditions,
};
use camino::Utf8PathBuf;
use itertools::Itertools;
use notify::{DebouncedEvent, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::{fmt::Display, time::Duration};
use tokio::task::JoinHandle;

pub async fn spawn(config: &Config) -> Result<JoinHandle<()>> {
    let mut paths = vec!["src".to_created_dir()?];
    if let Some(style) = &config.leptos.style_file {
        paths.push(style.clone().without_last());
    }

    let assets_dir = if let Some(dir) = &config.leptos.assets_dir {
        let assets_root = dir.to_owned();
        paths.push(assets_root.clone());
        Some(assets_root)
    } else {
        None
    };

    let paths = remove_nested(paths);

    log::info!(
        "Notify watching folders {}",
        GRAY.paint(paths.iter().join(", "))
    );

    Ok(tokio::spawn(async move {
        run(&paths, vec![], assets_dir).await
    }))
}

async fn run(paths: &[Utf8PathBuf], exclude: Vec<Utf8PathBuf>, assets_dir: Option<Utf8PathBuf>) {
    let (sync_tx, sync_rx) = std::sync::mpsc::channel::<DebouncedEvent>();

    std::thread::spawn(move || {
        while let Ok(event) = sync_rx.recv() {
            log::trace!("Notify received {event:?}");
            if let Ok(Some(watched)) = Watched::try_new(event) {
                handle(watched, &exclude, &assets_dir);
            }
        }
        log::debug!("Notify stopped");
    });

    let mut watcher = notify::watcher(sync_tx, Duration::from_millis(200))
        .expect("failed to build file system watcher");

    for path in paths {
        if let Err(e) = watcher.watch(path, RecursiveMode::Recursive) {
            log::error!("Notify could not watch {path:?} due to {e:?}");
        }
    }

    if let Err(e) = Interrupt::subscribe_shutdown().recv().await {
        log::trace!("Notify stopped due to: {e:?}");
    }
}

fn handle(watched: Watched, exclude: &[Utf8PathBuf], assets_dir: &Option<Utf8PathBuf>) {
    if let Some(path) = watched.path() {
        if exclude.contains(path) {
            log::trace!("Notify excluded: {path}");
            return;
        }
    }

    if let Some(assets_dir) = assets_dir {
        match watched.path() {
            Some(path) if path.starts_with(assets_dir) => {
                log::debug!("Notify asset change {}", GRAY.paint(watched.to_string()));
                Interrupt::send(Change::Asset(watched));
                return;
            }
            _ => {}
        }
    }

    match watched.path_ext() {
        Some("rs") => {
            log::debug!("Notify source change {}", GRAY.paint(watched.to_string()));
            Interrupt::send(Change::Source);
        }
        Some(ext) if ["scss", "sass", "css"].contains(&ext.to_lowercase().as_str()) => {
            log::debug!("Notify style change {}", GRAY.paint(watched.to_string()));
            Interrupt::send(Change::Style);
        }
        _ => log::trace!(
            "Notify path ext '{:?}' not matching in: {:?}",
            watched.path_ext(),
            watched.path()
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Watched {
    Remove(Utf8PathBuf),
    Rename(Utf8PathBuf, Utf8PathBuf),
    Write(Utf8PathBuf),
    Create(Utf8PathBuf),
    Rescan,
}

fn convert(p: PathBuf) -> Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(p).map_err(|e| anyhow!("Could not convert to a Utf8PathBuf: {e:?}"))
}
impl Watched {
    fn try_new(event: DebouncedEvent) -> Result<Option<Self>> {
        use DebouncedEvent::{
            Chmod, Create, Error, NoticeRemove, NoticeWrite, Remove, Rename, Rescan, Write,
        };

        Ok(match event {
            Chmod(_) | NoticeRemove(_) | NoticeWrite(_) => None,
            Create(f) => Some(Self::Create(convert(f)?)),
            Remove(f) => Some(Self::Remove(convert(f)?)),
            Rename(f, t) => Some(Self::Rename(convert(f)?, convert(t)?)),
            Write(f) => Some(Self::Write(convert(f)?)),
            Rescan => Some(Self::Rescan),
            Error(e, Some(p)) => {
                log::error!("Notify error watching {p:?}: {e:?}");
                None
            }
            Error(e, None) => {
                log::error!("Notify error: {e:?}");
                None
            }
        })
    }

    pub fn path_ext(&self) -> Option<&str> {
        self.path().and_then(|p| p.extension())
    }

    pub fn path(&self) -> Option<&Utf8PathBuf> {
        match self {
            Self::Remove(p) | Self::Rename(p, _) | Self::Write(p) | Self::Create(p) => Some(p),
            Self::Rescan => None,
        }
    }

    pub fn path_starts_with(&self, path: &Utf8PathBuf) -> bool {
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
