use crate::config::Project;
use crate::ext::PathBufExt;
use crate::internal_prelude::*;
use crate::signal::{Interrupt, ReloadSignal};
use crate::{ext::remove_nested, logger::GRAY};
use camino::Utf8PathBuf;
use itertools::Itertools;
use leptos_hot_reload::ViewMacros;
use notify::event::ModifyKind;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use tokio::task::JoinHandle;

pub async fn spawn(proj: &Arc<Project>, view_macros: &ViewMacros) -> Result<JoinHandle<()>> {
    let view_macros = view_macros.to_owned();
    let mut set: HashSet<Utf8PathBuf> = HashSet::from_iter(vec![]);

    set.extend(proj.lib.src_paths.clone());

    let paths = remove_nested(set.into_iter());

    info!(
        "Patch watching folders {}",
        GRAY.paint(paths.iter().join(", "))
    );
    let proj = proj.clone();

    Ok(tokio::spawn(
        async move { run(&paths, proj, view_macros).await },
    ))
}

async fn run(paths: &[Utf8PathBuf], proj: Arc<Project>, view_macros: ViewMacros) {
    let (sync_tx, sync_rx) = std::sync::mpsc::channel();

    let proj = proj.clone();
    tokio::task::spawn_blocking(move || {
        while let Ok(event) = sync_rx.recv() {
            match event {
                Ok(event) => handle(event, proj.clone(), view_macros.clone()),
                Err(err) => {
                    trace!("Notify error: {err:?}");
                    return;
                }
            }
        }
        debug!("Notify stopped");
    });

    let mut watcher = notify::RecommendedWatcher::new(
        sync_tx,
        notify::Config::default().with_poll_interval(super::notify::FALLBACK_POLLING_TIMEOUT),
    )
    .expect("failed to build file system watcher");

    for path in paths {
        if let Err(e) = watcher.watch(Path::new(path), RecursiveMode::Recursive) {
            error!("Notify could not watch {path:?} due to {e:?}");
        }
    }

    if let Err(e) = Interrupt::subscribe_shutdown().recv().await {
        trace!("Notify stopped due to: {e:?}");
    }
}

fn handle(event: Event, proj: Arc<Project>, view_macros: ViewMacros) {
    if event.paths.is_empty() {
        return;
    }

    if let EventKind::Any
    | EventKind::Other
    | EventKind::Access(_)
    | EventKind::Modify(ModifyKind::Any | ModifyKind::Other | ModifyKind::Metadata(_)) =
        event.kind
    {
        return;
    };

    trace!("Notify handle {}", GRAY.paint(format!("{:?}", event.paths)));

    let paths: Vec<_> = event
        .paths
        .into_iter()
        .filter_map(|p| match super::notify::convert(&p, &proj) {
            Ok(p) => Some(p),
            Err(e) => {
                info!("{e}");
                None
            }
        })
        .collect();

    for path in paths {
        if path.starts_with_any(&proj.lib.src_paths) && path.is_ext_any(&["rs"]) {
            // Check if it's possible to patch
            let patches = view_macros.patch(&path);
            if let Ok(Some(patch)) = patches {
                debug!("Patching view.");
                ReloadSignal::send_view_patches(&patch);
            }
        }
    }
}
