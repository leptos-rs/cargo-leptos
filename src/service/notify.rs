use crate::{
    compile::{Change, ChangeSet},
    config::Project,
    ext::{Paint, PathBufExt, PathExt},
    internal_prelude::*,
    logger::GRAY,
    signal::{Interrupt, ReloadSignal},
};
use camino::Utf8PathBuf;
use ignore::gitignore::Gitignore;
use itertools::Itertools;
use leptos_hot_reload::ViewMacros;
use notify_debouncer_full::{
    new_debouncer,
    notify::{event::ModifyKind, EventKind, RecommendedWatcher, RecursiveMode},
    DebounceEventResult, DebouncedEvent, Debouncer, RecommendedCache,
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{mpsc::Sender, Arc, Mutex},
    time::Duration,
};
use tokio::task::JoinHandle;
use tracing::*;
use walkdir::WalkDir;

const POLLING_TIMEOUT: Duration = Duration::from_millis(100);

pub async fn spawn(proj: &Arc<Project>, view_macros: Option<ViewMacros>) -> Result<JoinHandle<()>> {
    let proj = proj.clone();

    Ok(tokio::spawn(async move { run(proj, view_macros).await }))
}

async fn run(proj: Arc<Project>, view_macros: Option<ViewMacros>) {
    let (sync_tx, sync_rx) = std::sync::mpsc::channel();

    let watcher = Arc::new(Mutex::new(GitAwareWatcher::new(&proj, sync_tx.clone())));

    std::thread::spawn({
        let proj = proj.clone();
        let watcher = watcher.clone();
        move || {
            while let Ok(event) = sync_rx.recv() {
                match event {
                    Ok(event) => handle(event, proj.clone(), &view_macros, watcher.clone()),
                    Err(err) => {
                        trace!("Notify error: {err:?}");
                        return;
                    }
                }
            }
            debug!("Notify stopped");
        }
    });

    if let Err(e) = Interrupt::subscribe_shutdown().recv().await {
        trace!("Notify stopped due to: {e:?}");
    }
}

fn handle(
    events: Vec<DebouncedEvent>,
    proj: Arc<Project>,
    view_macros: &Option<ViewMacros>,
    watcher: Arc<Mutex<GitAwareWatcher>>,
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

            if let EventKind::Create(_) = event.kind {
                let added_dirs: HashSet<_> =
                    event.paths.iter().filter(|p| p.is_dir()).cloned().collect();
                watcher.lock().unwrap().watch(added_dirs.iter());
            }

            if let EventKind::Remove(_) = event.kind {
                let deleted_dirs: HashSet<_> =
                    event.paths.iter().filter(|p| p.is_dir()).cloned().collect();
                watcher.lock().unwrap().unwatch(deleted_dirs.iter());
            }

            let paths: Vec<Utf8PathBuf> = event
                .paths
                .iter()
                .filter_map(|p| {
                    let p = match convert(p, &proj) {
                        Ok(p) => p,
                        Err(e) => {
                            info!("{e}");
                            return None;
                        }
                    };

                    if !p.exists() {
                        return None;
                    }

                    Some(p)
                })
                .collect();

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

    trace!("Notify handle {}", GRAY.paint(format!("{paths:?}")));

    for path in paths {
        if path.starts_with(".gitignore") {
            debug!("Notify .gitignore change {}", GRAY.paint(path.to_string()));
            watcher.lock().unwrap().update_gitignore();
            continue;
        }

        if let Some(assets) = &proj.assets {
            if path.starts_with(&assets.dir) {
                debug!("Notify asset change {}", GRAY.paint(path.as_str()));
                changes.add(Change::Asset);
            }
        }

        let lib_rs = path.starts_with_any(&proj.lib.src_paths) && path.is_ext_any(&["rs"]);
        let lib_js = path.starts_with(&proj.js_dir) && path.is_ext_any(&["js"]);

        if lib_rs || lib_js {
            debug!("Notify lib source change {}", GRAY.paint(path.as_str()));
            changes.add(Change::LibSource);
        }

        if path.starts_with_any(&proj.bin.src_paths) && path.is_ext_any(&["rs"]) {
            if let Some(view_macros) = view_macros {
                // Check if it's possible to patch
                let patches = view_macros.patch(&path);
                if let Ok(Some(patch)) = patches {
                    debug!("Patching view.");
                    ReloadSignal::send_view_patches(&patch);
                }
            }
            debug!("Notify bin source change {}", GRAY.paint(path.to_string()));
            changes.add(Change::BinSource);
        }

        if let Some(file) = &proj.style.file {
            let src = file.source.clone().without_last();
            if path.starts_with(src) && path.is_ext_any(&["scss", "sass", "css"]) {
                debug!("Notify style change {}", GRAY.paint(path.as_str()));
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
                debug!("Notify style change {}", GRAY.paint(path.as_str()));
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

struct GitAwareWatcher {
    watcher: Debouncer<RecommendedWatcher, RecommendedCache>,
    gitignore: Gitignore,
    gitignore_path: Utf8PathBuf,
    paths: HashSet<PathBuf>,
    sync_tx: Sender<notify_debouncer_full::DebounceEventResult>,
    forced_watch_paths: HashSet<PathBuf>,
}

impl GitAwareWatcher {
    fn new(proj: &Project, sync_tx: std::sync::mpsc::Sender<DebounceEventResult>) -> Self {
        let watcher = new_debouncer(POLLING_TIMEOUT, None, sync_tx.clone())
            .expect("failed to build file system watcher");

        let mut paths: Vec<_> = proj
            .watch_additional_files
            .iter()
            .filter_map(|p| p.canonicalize().ok())
            .collect();

        let forced_watch_top_level_paths: HashSet<PathBuf> = proj
            .watch_additional_files
            .iter()
            .filter_map(|p| p.canonicalize().ok())
            .collect();

        let mut forced_watch_paths: HashSet<PathBuf> = HashSet::new();

        paths.push(proj.working_dir.clone().into());

        let paths: HashSet<PathBuf> = paths
            .into_iter()
            .flat_map(|p| {
                let is_forced_path = forced_watch_top_level_paths.contains(&p);
                WalkDir::new(p)
                    .follow_links(true)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|d| {
                        d.file_type().is_dir()
                            && !d.path().components().any(|c| c.as_os_str() == ".git")
                    })
                    .map(|d| {
                        if is_forced_path {
                            forced_watch_paths.insert(d.path().into());
                        }
                        d.path().to_owned()
                    })
                    .collect::<HashSet<_>>()
            })
            .collect();

        let gitignore_path = proj.working_dir.join(".gitignore");

        let gitignore = Self::new_gitignore(gitignore_path.as_std_path());

        let mut watcher = Self {
            watcher,
            gitignore,
            gitignore_path,
            sync_tx,
            paths: paths.clone(),
            forced_watch_paths,
        };

        watcher.watch(paths.iter());

        watcher
    }

    fn new_gitignore(gitignore_path: &Path) -> Gitignore {
        info!("Creating ignore list from '.gitignore' file");

        let (gi, err) = Gitignore::new(gitignore_path);

        if let Some(err) = err {
            error!("Failed reading '.gitignore' file in the working directory: {err}\nThis causes the watcher to work expensively on file changes like changes in the 'target' path.\nCreate a '.gitignore' file and exclude common build and cache paths like 'target'");
        }

        gi
    }

    fn update_gitignore(&mut self) {
        // Current watcher will be stopped on drop
        self.watcher = new_debouncer(POLLING_TIMEOUT, None, self.sync_tx.clone())
            .expect("failed to build file system watcher");

        self.gitignore = Self::new_gitignore(self.gitignore_path.as_std_path());

        self.watch(self.paths.clone().iter());
    }

    fn ignore_paths<'a, I>(&self, paths: I) -> HashSet<PathBuf>
    where
        I: Iterator<Item = &'a PathBuf>,
    {
        paths
            .filter_map(|p| {
                // Check if the path should always be included no matter what
                if self.forced_watch_paths.contains(p) {
                    return Some(p.clone());
                }
                // Check if the file excluded
                let matched = self.gitignore.matched(p, p.is_dir());
                if matches!(matched, ignore::Match::Ignore(_)) {
                    return None;
                }

                // Check if the parent directories excluded
                let mut parent = p.clone();
                while let Some(par) = parent.parent() {
                    if matches!(self.gitignore.matched(par, true), ignore::Match::Ignore(_)) {
                        return None;
                    }
                    parent = par.to_path_buf();
                }

                Some(p.clone())
            })
            .collect()
    }

    fn watch<'a, I>(&mut self, paths: I)
    where
        I: Iterator<Item = &'a PathBuf>,
    {
        let paths = self.ignore_paths(paths);

        for path in paths {
            if let Err(e) = self.watcher.watch(&path, RecursiveMode::NonRecursive) {
                error!("Notify could not watch {:?} due to {e:?}", path);
                continue;
            } else {
                trace!("Watch path: {path:?}");
            }
        }
    }

    fn unwatch<'a, I>(&mut self, paths: I)
    where
        I: Iterator<Item = &'a PathBuf>,
    {
        let paths = self.ignore_paths(paths);

        for path in paths {
            if let Err(e) = self.watcher.unwatch(&path) {
                error!("Notify could not watch {:?} due to {e:?}", path);
            } else {
                trace!("Unwatch path: {path:?}");
            }
        }
    }
}

fn convert(p: &Path, proj: &Project) -> Result<Utf8PathBuf> {
    let p = Utf8PathBuf::from_path_buf(p.to_path_buf())
        .map_err(|e| eyre!("Could not convert to a Utf8PathBuf: {e:?}"))?;
    Ok(p.unbase(&proj.working_dir).unwrap_or(p))
}
