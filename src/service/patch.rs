use crate::config::Project;
use crate::ext::anyhow::Result;
use crate::signal::{Interrupt, ReloadSignal};
use crate::{
    ext::{remove_nested, PathBufExt},
    logger::GRAY,
};
use camino::Utf8PathBuf;
use itertools::Itertools;
use leptos_hot_reload::ViewMacros;
use notify::{DebouncedEvent, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

use super::notify::Watched;

pub async fn spawn(proj: &Arc<Project>, view_macros: &ViewMacros) -> Result<JoinHandle<()>> {
    let view_macros = view_macros.to_owned();
    let mut set: HashSet<Utf8PathBuf> = HashSet::from_iter(vec![]);

    set.extend(proj.lib.src_paths.clone());

    let paths = remove_nested(set.into_iter());

    log::info!(
        "Patch watching folders {}",
        GRAY.paint(paths.iter().join(", "))
    );
    let proj = proj.clone();

    Ok(tokio::spawn(
        async move { run(&paths, proj, view_macros).await },
    ))
}

async fn run(paths: &[Utf8PathBuf], proj: Arc<Project>, view_macros: ViewMacros) {
    let (sync_tx, sync_rx) = std::sync::mpsc::channel::<DebouncedEvent>();

    let proj = proj.clone();
    std::thread::spawn(move || {
        while let Ok(event) = sync_rx.recv() {
            match Watched::try_new(&event, &proj) {
                Ok(Some(watched)) => handle(watched, proj.clone(), view_macros.clone()),
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

fn handle(watched: Watched, proj: Arc<Project>, view_macros: ViewMacros) {
    log::trace!(
        "Notify handle {}",
        GRAY.paint(format!("{:?}", watched.path()))
    );

    let Some(path) = watched.path() else {
        Interrupt::send_all_changed();
        return
    };

    if path.starts_with_any(&proj.lib.src_paths) && path.is_ext_any(&["rs"]) {
        // Check if it's possible to patch
        let patches = view_macros.patch(path);
        if let Ok(Some(patch)) = patches {
            log::debug!("Patching view.");
            ReloadSignal::send_view_patches(&patch);
        }
    }
}
