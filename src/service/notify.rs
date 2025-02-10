use crate::compile::Change;
use crate::config::Project;
use crate::internal_prelude::*;
use crate::signal::Interrupt;
use crate::{
    compile::{Change, ChangeSet},
    config::Project,
    ext::{Paint, PathBufExt, PathExt},
    logger::GRAY,
    signal::{Interrupt, ReloadSignal},
};
use crate::{
    ext::{remove_nested, PathBufExt, PathExt},
    logger::GRAY,
};
use camino::Utf8PathBuf;
use ignore::gitignore::Gitignore;
use itertools::Itertools;
use leptos_hot_reload::ViewMacros;
use notify_debouncer_full::{
    new_debouncer,
    notify::{event::ModifyKind, EventKind, RecursiveMode},
    DebouncedEvent,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::task::JoinHandle;

const POLLING_TIMEOUT: Duration = Duration::from_millis(100);

pub async fn spawn(proj: &Arc<Project>, view_macros: Option<ViewMacros>) -> Result<JoinHandle<()>> {
    let proj = proj.clone();

    Ok(tokio::spawn(async move { run(proj, view_macros).await }))
}

async fn run(proj: Arc<Project>, view_macros: Option<ViewMacros>) {
    let (sync_tx, sync_rx) = std::sync::mpsc::channel();

    tokio::task::spawn_blocking({
        let proj = proj.clone();
        move || {
            let mut gitignore = create_gitignore_instance(&proj);
            while let Ok(event) = sync_rx.recv() {
                match event {
                    Ok(event) => handle(event, proj.clone(), &view_macros, &mut gitignore),
                    Err(err) => {
                        log::trace!("Notify error: {err:?}");
                        return;
                    }
                }
            }
            debug!("Notify stopped");
        }
    });

    let mut watcher =
        new_debouncer(POLLING_TIMEOUT, None, sync_tx).expect("failed to build file system watcher");

    let mut paths = proj.watch_additional_files.clone();
    paths.push(proj.working_dir.clone());

    for path in paths {
        if let Err(e) = watcher.watch(path.as_std_path(), RecursiveMode::Recursive) {
            log::error!("Notify could not watch {:?} due to {e:?}", proj.working_dir);
        }
    }

    if let Err(e) = Interrupt::subscribe_shutdown().recv().await {
        trace!("Notify stopped due to: {e:?}");
    }
}

fn handle(
    events: Vec<DebouncedEvent>,
    proj: Arc<Project>,
    view_macros: &Option<ViewMacros>,
    gitignore: &mut Gitignore,
) {
    if events.is_empty() {
        return;
    }

    let paths: Vec<_> = events
        .into_iter()
        .filter_map(|event| {
            if event.paths.is_empty() {
                return None;
            }

            if let EventKind::Any
            | EventKind::Other
            | EventKind::Access(_)
            | EventKind::Modify(ModifyKind::Other | ModifyKind::Metadata(_)) = event.kind
            {
                return None;
            };

            let paths = ignore_paths(&proj, &event.paths, gitignore);

            if paths.is_empty() {
                None
            } else {
                Some(paths)
            }
        })
        .flatten()
        .dedup()
        .collect();

    if paths.is_empty() {
        return;
    }

    let mut changes = ChangeSet::new();

    log::trace!("Notify handle {}", GRAY.paint(format!("{paths:?}")));

    for path in paths {
        if path.starts_with(".gitignore") {
            *gitignore = create_gitignore_instance(&proj);
            log::debug!("Notify .gitignore change {}", GRAY.paint(path.to_string()));
            continue;
        }

        if let Some(assets) = &proj.assets {
            if path.starts_with(&assets.dir) {
                log::debug!("Notify asset change {}", GRAY.paint(path.as_str()));
                changes.add(Change::Asset);
            }
        }

        let lib_rs = path.starts_with_any(&proj.lib.src_paths) && path.is_ext_any(&["rs"]);
        let lib_js = path.starts_with(&proj.js_dir) && path.is_ext_any(&["js"]);

        if lib_rs || lib_js {
            log::debug!("Notify lib source change {}", GRAY.paint(path.as_str()));
            changes.add(Change::LibSource);
        }

        if path.starts_with_any(&proj.bin.src_paths) && path.is_ext_any(&["rs"]) {
            if let Some(view_macros) = view_macros {
                // Check if it's possible to patch
                let patches = view_macros.patch(&path);
                if let Ok(Some(patch)) = patches {
                    log::debug!("Patching view.");
                    ReloadSignal::send_view_patches(&patch);
                }
            }
            log::debug!("Notify bin source change {}", GRAY.paint(path.to_string()));
            changes.add(Change::BinSource);
        }

        if let Some(file) = &proj.style.file {
            let src = file.source.clone().without_last();
            if path.starts_with(src) && path.is_ext_any(&["scss", "sass", "css"]) {
                log::debug!("Notify style change {}", GRAY.paint(path.as_str()));
                changes.add(Change::Style);
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
                changes.add(Change::Style);
            }
        }

        if path.starts_with_any(&proj.watch_additional_files) {
            debug!(
                "Notify additional file change {}",
                GRAY.paint(path.as_str())
            );
            changes.add(Change::Additional);
        }
    }

    if !changes.is_empty() {
        Interrupt::send(&changes);
    }
}

fn create_gitignore_instance(proj: &Project) -> Gitignore {
    log::info!("Creating ignore list from '.gitignore' file");

    let (gi, err) = Gitignore::new(proj.working_dir.join(".gitignore"));

    if let Some(err) = err {
        log::error!("Failed reading '.gitignore' file in the working directory: {err}\nThis causes the watcher to work expensively on file changes like changes in the 'target' path.\nCreate a '.gitignore' file and exclude common build and cache paths like 'target'");
    }

    gi
}

fn ignore_paths(
    proj: &Project,
    event_paths: &[PathBuf],
    gitignore: &Gitignore,
) -> Vec<Utf8PathBuf> {
    event_paths
        .iter()
        .filter_map(|p| {
            let p = match convert(p, proj) {
                Ok(p) => p,
                Err(e) => {
                    log::info!("{e}");
                    return None;
                }
            };

            // Check if the file excluded
            let matched = gitignore.matched(p.as_std_path(), p.is_dir());
            if matches!(matched, ignore::Match::Ignore(_)) {
                return None;
            }

            // Check if the parent directories excluded
            let mut parent = p.as_std_path();
            while let Some(par) = parent.parent() {
                if matches!(gitignore.matched(par, true), ignore::Match::Ignore(_)) {
                    return None;
                }
                parent = par;
            }

            if !p.exists() {
                return None;
            }

            Some(p)
        })
        .collect()
}

fn convert(p: &Path, proj: &Project) -> Result<Utf8PathBuf> {
    let p = Utf8PathBuf::from_path_buf(p.to_path_buf())
        .map_err(|e| eyre!("Could not convert to a Utf8PathBuf: {e:?}"))?;
    Ok(p.unbase(&proj.working_dir).unwrap_or(p))
}
