use std::assert_matches;
use std::fmt::from_fn;
use std::sync::LazyLock;

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
    // let site_dir = Utf8PathBuf::from("target/site");

    // insta::assert_snapshot!(site_dir.ls_ascii(0).unwrap_or_default());
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

    let mut lock_file_path = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    lock_file_path.extend(["src", "tests", "project-with-outdated-lock", "Cargo.lock"]);
    assert!(!lock_file_path.exists());

    let expected_line = format!(
        "error: cannot create the lock file {lock_file_path} because --locked was passed to prevent this",
    );

    // `StyledStr::to_string` strips the color:
    // https://docs.rs/clap_builder/latest/clap_builder/builder/struct.StyledStr.html#impl-Display-for-StyledStr
    let stderr_colorless = clap::builder::StyledStr::from(stderr).to_string();

    let stderr_colorless_indent = from_fn(|f| {
        if stderr_colorless.is_empty() {
            return write!(f, ">\n");
        }

        for line in stderr_colorless.lines() {
            write!(f, "> {line}\n")?;
        }
        Ok(())
    });
    assert!(
        stderr_colorless.lines().any(|line| line == expected_line),
        "Could not find line {expected_line:?} in the stderr output:\n{stderr_colorless_indent}"
    );
}
