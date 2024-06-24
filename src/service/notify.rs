use crate::compile::Change;
use crate::config::Project;
use crate::ext::anyhow::{anyhow, Result};
use crate::signal::Interrupt;
use crate::{
    ext::{remove_nested, PathBufExt, PathExt},
    logger::GRAY,
};
use camino::Utf8PathBuf;
use core::time::Duration;
use itertools::Itertools;
use notify::event::RenameMode;
use notify::{RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, DebouncedEvent};
use std::collections::HashSet;
use std::fmt::Display;
use std::path::Path;
use std::sync::Arc;
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

    let mut debouncer = new_debouncer(
        Duration::from_millis(200),
        None,
        move |res: DebounceEventResult| match res {
            Ok(events) => {
                events.iter().for_each(|event| {
                    sync_tx
                        .send(event.clone())
                        .expect("failed to build file system watcher")
                });
            }
            Err(e) => println!("watch error: {:?}", e),
        },
    )
    .expect("failed to build file system debouncer/watcher");

    for path in paths {
        if let Err(e) = debouncer
            .watcher()
            .watch(path.as_std_path(), RecursiveMode::Recursive)
        {
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
        use notify::event::ModifyKind;
        use notify::EventKind::Access;
        use notify::EventKind::Any;
        use notify::EventKind::Create;
        use notify::EventKind::Modify;
        use notify::EventKind::Other;
        use notify::EventKind::Remove;
        println!("try_new() event.kind {:#?}", event.kind.clone());
        Ok(match event.kind {
            Modify(modify_kind) => {
                match modify_kind {
                    ModifyKind::Name(RenameMode::Both)
                    | ModifyKind::Name(RenameMode::Any)
                    | ModifyKind::Name(RenameMode::Other) => {
                        // RenameModes are "Any", "Both", "To", "From" and "Other".
                        //
                        // Only "Both" is guaranteed to contain two filenames.
                        // "Any" are "Other" are for cross-platform compatibility.
                        //
                        // Here we are considering the less chatty DebouncedEvent.
                        //
                        // See <https://docs.rs/notify-debouncer-full/latest/notify_debouncer_full/>
                        //
                        // "Only emits a single Rename event if the rename From and To events can be matched"
                        debug_assert!(event.paths.len() == 2usize, "Rename needs two filenames");
                        Some(Self::Rename(
                            convert(&event.paths[0], proj)?,
                            convert(&event.paths[1], proj)?,
                        ))
                    }

                    ModifyKind::Data(_) | ModifyKind::Other | ModifyKind::Any => {
                        Some(Self::Write(convert(&event.paths[0], proj)?))
                    }

                    ModifyKind::Metadata(_) => None,

                    ModifyKind::Name(RenameMode::From) | ModifyKind::Name(RenameMode::To) => {
                        panic!("These events are not possible with debounced notification.");
                    }
                }
            }

            Create(create_kind) => {
                match create_kind {
                    notify::event::CreateKind::File => {
                        Some(Self::Create(convert(&event.paths[0], proj)?))
                    }
                    // Any/Folder/Other
                    _ => None,
                }
            }
            Remove(remove_kind) => {
                match remove_kind {
                    notify::event::RemoveKind::File => {
                        Some(Self::Remove(convert(&event.paths[0], proj)?))
                    }
                    // Any/Folder/Other
                    _ => None,
                }
            }
            Other | Any | Access(_) => None,
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

#[cfg(test)]
mod test {

    use core::time::Duration;
    use std::fs::remove_file;
    use std::fs::File;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::path::PathBuf;

    use notify::RecursiveMode;
    use notify::Watcher;
    use notify_debouncer_full::new_debouncer;
    use notify_debouncer_full::DebounceEventResult;
    use tokio::sync::oneshot;
    use tokio::time::timeout;

    use crate::config::Config;
    use crate::service::notify::Watched;
    use crate::GRAY;

    fn opts(project: Option<&str>) -> crate::config::Opts {
        crate::config::Opts {
            release: false,
            precompress: false,
            hot_reload: false,
            project: project.map(|s| s.to_string()),
            verbose: 0,
            features: Vec::new(),
            bin_features: Vec::new(),
            lib_features: Vec::new(),
            bin_cargo_args: None,
            lib_cargo_args: None,
            js_minify: false,
            wasm_debug: false,
        }
    }

    // Overivew :-
    //
    // SETUP: create a file in a valid project.
    //
    // 1) Construct watching mechanism.
    //
    // 2) Modfify the file.
    //
    // 3) Assert the mechanism observed a valid event.
    //
    // TEARDOWN: delete the file.
    #[tokio::test]
    async fn change_file_contents() {
        let cli = opts(Some("notify"));
        let config =
            Config::test_load(cli, "examples", "examples/notify/Cargo.toml", true, None);

        let mut filename = PathBuf::from(&config.working_dir);
        filename.push("mood.txt");

        let mut file = File::create(filename.clone()).expect("Could not create test file");
        file.write_all(b"happy\r\n")
            .expect("did not initialize file");
        file.flush().expect("initial flushing failed");
        // File::close()
        drop(file);

        let (sync_tx, sync_rx) = std::sync::mpsc::channel();
        let (success_tx, success_rx) = oneshot::channel::<bool>();

        std::thread::spawn(move || {
            while let Ok(event) = sync_rx.recv() {
                match Watched::try_new(&event, &config.projects[0]) {
                    Ok(Some(_)) => {
                        break;
                    }
                    Err(e) => log::error!("Notify error {e}"),
                    _ => log::trace!("Notify not handled {}", GRAY.paint(format!("{:?}", event))),
                }
            }
            success_tx
                .send(true)
                .expect("failed to send passing notification");
        });

        let mut debouncer = new_debouncer(
            Duration::from_millis(400),
            None,
            move |res: DebounceEventResult| {
                match res {
                    Ok(events) => {
                        // send can fail must handle
                        let _ = events.iter().for_each(|event| {
                            sync_tx
                                .send(event.clone())
                                .expect("change_file_contents: failed to build file system watcher")
                        });
                    }
                    Err(e) => println!("watch errors: {:?}", e),
                }
            },
        )
        .expect("failed to build file system watcher");

        debouncer
            .watcher()
            .watch(&filename, RecursiveMode::NonRecursive)
            .expect("could not watch {path:?}");

        // Modify file.
        let mut modify_handle = OpenOptions::new()
            .write(true)
            .open(filename.clone())
            .expect("Could not reopen file");
        modify_handle
            .write_all(b"grumpy\r\n")
            .expect("could not actively modify the file");
        modify_handle.flush().expect("second flushing failed");

        // Wait for success or a watchdog timeout.
        let received_notification = match timeout(Duration::from_millis(4000), success_rx).await {
            Ok(_) => true,
            Err(_) => {
                println!("did not receive value within 800 ms");
                false
            }
        };

        // TEARDOWN: before assert, as a failing test aborts the test.
        remove_file(filename).expect("Could not tear down file.");

        assert!(received_notification);
    }
}
