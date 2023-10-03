use crate::compile::Change;
use crate::config::Project;
use crate::ext::anyhow::{anyhow, Result};
use crate::signal::Interrupt;
use crate::{
    ext::{remove_nested, PathBufExt, PathExt},
    logger::GRAY,
};
use camino::Utf8PathBuf;
use itertools::Itertools;
use notify::{DebouncedEvent, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::{fmt::Display, time::Duration};
use tokio::task::JoinHandle;

pub async fn spawn(proj: &Arc<Project>) -> Result<JoinHandle<()>> {
    let mut set: HashSet<Utf8PathBuf> = HashSet::from_iter(vec![]);

    set.extend(proj.lib.src_paths.clone());
    set.extend(proj.bin.src_paths.clone());
    set.extend(proj.watch_additional_files.clone());
    set.insert(proj.js_dir.clone());

    if let Some(file) = &proj.style.file {
        set.insert(file.source.clone().without_last());
    }

    if let Some(tailwind) = &proj.style.tailwind {
        set.insert(tailwind.config_file.clone());
        set.insert(tailwind.input_file.clone());
    }

    if let Some(assets) = &proj.assets {
        set.insert(assets.dir.clone());
    }

    let paths = remove_nested(set.into_iter().filter(|path| Path::new(path).exists()));

    log::info!(
        "Notify watching paths {}",
        GRAY.paint(paths.iter().join(", "))
    );
    let proj = proj.clone();

    Ok(tokio::spawn(async move { run(&paths, proj).await }))
}

async fn run(paths: &[Utf8PathBuf], proj: Arc<Project>) {
    let (sync_tx, sync_rx) = std::sync::mpsc::channel::<DebouncedEvent>();

    let proj = proj.clone();
    std::thread::spawn(move || {
        while let Ok(event) = sync_rx.recv() {
            match Watched::try_new(&event, &proj) {
                Ok(Some(watched)) => handle(watched, proj.clone()),
                Err(e) => log::error!("Notify error {e}"),
                _ => log::trace!("Notify not handled {}", GRAY.paint(format!("{:?}", event))),
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

fn handle(watched: Watched, proj: Arc<Project>) {
    log::trace!(
        "Notify handle {}",
        GRAY.paint(format!("{:?}", watched.path()))
    );

    let Some(path) = watched.path() else {
        Interrupt::send_all_changed();
        return;
    };

    let mut changes = Vec::new();

    if let Some(assets) = &proj.assets {
        if path.starts_with(&assets.dir) {
            log::debug!("Notify asset change {}", GRAY.paint(watched.to_string()));
            changes.push(Change::Asset(watched.clone()));
        }
    }

    let lib_rs = path.starts_with_any(&proj.lib.src_paths) && path.is_ext_any(&["rs"]);
    let lib_js = path.starts_with(&proj.js_dir) && path.is_ext_any(&["js"]);

    if lib_rs || lib_js {
        log::debug!(
            "Notify lib source change {}",
            GRAY.paint(watched.to_string())
        );
        changes.push(Change::LibSource);
    }

    if path.starts_with_any(&proj.bin.src_paths) && path.is_ext_any(&["rs"]) {
        log::debug!(
            "Notify bin source change {}",
            GRAY.paint(watched.to_string())
        );
        changes.push(Change::BinSource);
    }

    if let Some(file) = &proj.style.file {
        let src = file.source.clone().without_last();
        if path.starts_with(src) && path.is_ext_any(&["scss", "sass", "css"]) {
            log::debug!("Notify style change {}", GRAY.paint(watched.to_string()));
            changes.push(Change::Style)
        }
    }

    if let Some(tailwind) = &proj.style.tailwind {
        if path.as_path() == tailwind.config_file.as_path()
            || path.as_path() == tailwind.input_file.as_path()
        {
            log::debug!("Notify style change {}", GRAY.paint(watched.to_string()));
            changes.push(Change::Style)
        }
    }

    if path.starts_with_any(&proj.watch_additional_files) {
        log::debug!(
            "Notify additional file change {}",
            GRAY.paint(watched.to_string())
        );
        changes.push(Change::Additional);
    }

    if !changes.is_empty() {
        Interrupt::send(&changes);
    } else {
        log::trace!(
            "Notify changed but not watched: {}",
            GRAY.paint(watched.to_string())
        );
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

fn convert(p: &Path, proj: &Project) -> Result<Utf8PathBuf> {
    let p = Utf8PathBuf::from_path_buf(p.to_path_buf())
        .map_err(|e| anyhow!("Could not convert to a Utf8PathBuf: {e:?}"))?;
    Ok(p.unbase(&proj.working_dir).unwrap_or(p))
}

impl Watched {
    pub(crate) fn try_new(event: &DebouncedEvent, proj: &Project) -> Result<Option<Self>> {
        use DebouncedEvent::{
            Chmod, Create, Error, NoticeRemove, NoticeWrite, Remove, Rename, Rescan, Write,
        };

        Ok(match event {
            Chmod(_) | NoticeRemove(_) | NoticeWrite(_) => None,
            Create(f) => Some(Self::Create(convert(f, proj)?)),
            Remove(f) => Some(Self::Remove(convert(f, proj)?)),
            Rename(f, t) => Some(Self::Rename(convert(f, proj)?, convert(t, proj)?)),
            Write(f) => Some(Self::Write(convert(f, proj)?)),
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

    pub fn path_starts_with_any(&self, paths: &[&Utf8PathBuf]) -> bool {
        paths.iter().any(|path| self.path_starts_with(path))
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
