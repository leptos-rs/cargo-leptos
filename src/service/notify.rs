use crate::compile::Change;
use crate::config::Project;
use crate::ext::anyhow::{anyhow, Result};
use crate::ext::Paint;
use crate::signal::Interrupt;
use crate::{
    ext::{remove_nested, PathBufExt, PathExt},
    logger::GRAY,
};
use camino::Utf8PathBuf;
use itertools::Itertools;
use notify::event::ModifyKind;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

pub(crate) const FALLBACK_POLLING_TIMEOUT: Duration = Duration::from_millis(200);

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
        if let Some(config_file) = tailwind.config_file.as_ref() {
            set.insert(config_file.clone());
        }
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
    let (sync_tx, sync_rx) = std::sync::mpsc::channel();

    let proj = proj.clone();
    tokio::task::spawn_blocking(move || {
        while let Ok(event) = sync_rx.recv() {
            match event {
                Ok(event) => handle(event, proj.clone()),
                Err(err) => {
                    log::trace!("Notify error: {err:?}");
                    return;
                }
            }
        }
        log::debug!("Notify stopped");
    });

    let mut watcher = notify::RecommendedWatcher::new(
        sync_tx,
        notify::Config::default().with_poll_interval(FALLBACK_POLLING_TIMEOUT),
    )
    .expect("failed to build file system watcher");

    for path in paths {
        if let Err(e) = watcher.watch(Path::new(path), RecursiveMode::Recursive) {
            log::error!("Notify could not watch {path:?} due to {e:?}");
        }
    }

    if let Err(e) = Interrupt::subscribe_shutdown().recv().await {
        log::trace!("Notify stopped due to: {e:?}");
    }
}

fn handle(event: Event, proj: Arc<Project>) {
    if event.paths.is_empty() {
        return;
    }

    if let EventKind::Any
    | EventKind::Other
    | EventKind::Access(_)
    | EventKind::Modify(ModifyKind::Other | ModifyKind::Metadata(_)) = event.kind
    {
        return;
    };

    log::trace!("Notify handle {}", GRAY.paint(format!("{:?}", event.paths)));

    let paths: Vec<_> = event
        .paths
        .into_iter()
        .filter_map(|p| match convert(&p, &proj) {
            Ok(p) => Some(p),
            Err(e) => {
                log::info!("{e}");
                None
            }
        })
        .collect();

    let mut changes = Vec::new();

    for path in paths {
        if let Some(assets) = &proj.assets {
            if path.starts_with(&assets.dir) {
                log::debug!("Notify asset change {}", GRAY.paint(path.as_str()));
                changes.push(Change::Asset);
            }
        }

        let lib_rs = path.starts_with_any(&proj.lib.src_paths) && path.is_ext_any(&["rs"]);
        let lib_js = path.starts_with(&proj.js_dir) && path.is_ext_any(&["js"]);

        if lib_rs || lib_js {
            log::debug!("Notify lib source change {}", GRAY.paint(path.as_str()));
            changes.push(Change::LibSource);
        }

        if path.starts_with_any(&proj.bin.src_paths) && path.is_ext_any(&["rs"]) {
            log::debug!("Notify bin source change {}", GRAY.paint(path.as_str()));
            changes.push(Change::BinSource);
        }

        if let Some(file) = &proj.style.file {
            let src = file.source.clone().without_last();
            if path.starts_with(src) && path.is_ext_any(&["scss", "sass", "css"]) {
                log::debug!("Notify style change {}", GRAY.paint(path.as_str()));
                changes.push(Change::Style)
            }
        }

        if let Some(tailwind) = &proj.style.tailwind {
            if tailwind
                .config_file
                .as_ref()
                .is_some_and(|config_file| path.as_path() == config_file.as_path())
                || path.as_path() == tailwind.input_file.as_path()
            {
                log::debug!("Notify style change {}", GRAY.paint(path.as_str()));
                changes.push(Change::Style)
            }
        }

        if path.starts_with_any(&proj.watch_additional_files) {
            log::debug!(
                "Notify additional file change {}",
                GRAY.paint(path.as_str())
            );
            changes.push(Change::Additional);
        }

        if !changes.is_empty() {
            Interrupt::send(&changes);
        } else {
            log::trace!(
                "Notify changed but not watched: {}",
                GRAY.paint(path.as_str())
            );
        }
    }
}

pub(crate) fn convert(p: &Path, proj: &Project) -> Result<Utf8PathBuf> {
    let p = Utf8PathBuf::from_path_buf(p.to_path_buf())
        .map_err(|e| anyhow!("Could not convert to a Utf8PathBuf: {e:?}"))?;
    Ok(p.unbase(&proj.working_dir).unwrap_or(p))
}
