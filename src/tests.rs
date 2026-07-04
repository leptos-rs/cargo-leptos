use std::sync::LazyLock;
use std::{assert_matches, path::PathBuf};

use camino::Utf8PathBuf;
use tokio::sync::Mutex;

use crate::{
    config::{Cli, Commands, Opts},
    ext::PathBufExt,
    run,
};

static RUN_LOCK: LazyLock<Mutex<()>> = LazyLock::new(Default::default);

#[tokio::test]
async fn workspace_build() {
    let _run_lock = RUN_LOCK.lock().await;

    let command = Commands::Build(Opts::default());

    let cli = Cli {
        manifest_path: Some(Utf8PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/workspace/Cargo.toml",
        ))),
        log: Vec::new(),
        command,
    };

    run(cli).await.unwrap();

    // when running the current working directory is changed to the manifest path.
    let site_dir = Utf8PathBuf::from("target/site");

    insta::assert_snapshot!(site_dir.ls_ascii(0).unwrap_or_default());
}

#[tokio::test]
async fn project_build() {
    let _run_lock = RUN_LOCK.lock().await;

    let command = Commands::Build(Opts::default());

    let cli = Cli {
        manifest_path: Some(Utf8PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/project/Cargo.toml",
        ))),
        log: Vec::new(),
        command,
    };

    run(cli).await.unwrap();

    // when running the current working directory is changed to the manifest path.
    let site_dir = Utf8PathBuf::from("target/site");

    insta::assert_snapshot!(site_dir.ls_ascii(0).unwrap_or_default());
}

#[tokio::test]
async fn project_with_outdated_lock() {
    let _run_lock = RUN_LOCK.lock().await;

    let command = Commands::Build(Opts {
        cargo_locked: true,
        ..Default::default()
    });

    let cli = Cli {
        manifest_path: Some(Utf8PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/tests/project-with-outdated-lock/Cargo.toml",
        ))),
        log: Vec::new(),
        command,
    };

    let result = run(cli).await;
    assert!(result.is_err());
    let Err(error) = result else { unreachable!() };

    let error = error.downcast_ref::<cargo_metadata::Error>();
    assert_matches!(
        error,
        Some(cargo_metadata::Error::CargoMetadata { stderr: _ })
    );
    let Some(cargo_metadata::Error::CargoMetadata { stderr }) = error else {
        unreachable!()
    };

    let lock_file_path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/tests/project-with-outdated-lock/Cargo.lock"
    ));
    assert!(!lock_file_path.exists());

    let expected_line = format!(
        "error: cannot create the lock file {} because --locked was passed to prevent this",
        lock_file_path.to_string_lossy()
    );
    assert!(stderr.lines().any(|line| line == expected_line));
}
