use camino::Utf8PathBuf;

use super::Config;

#[test]
fn test_project() {
    let cli = crate::Opts {
        release: false,
        project: None,
        verbose: 0,
    };

    let conf = Config::load(cli, &Utf8PathBuf::from("examples/project/Cargo.toml"), true).unwrap();

    insta::assert_debug_snapshot!(conf);
}

#[test]
fn test_workspace() {
    let cli = crate::Opts {
        release: false,
        project: None,
        verbose: 0,
    };

    let conf = Config::load(
        cli,
        &Utf8PathBuf::from("examples/workspace/Cargo.toml"),
        true,
    )
    .unwrap();

    insta::assert_debug_snapshot!(conf);
}

#[test]
fn test_workspace_project1() {
    let cli = crate::Opts {
        release: false,
        project: Some("project1".to_string()),
        verbose: 0,
    };

    let conf = Config::load(
        cli,
        &Utf8PathBuf::from("examples/workspace/Cargo.toml"),
        true,
    )
    .unwrap();

    insta::assert_debug_snapshot!(conf);
}

#[test]
fn test_workspace_project2() {
    let cli = crate::Opts {
        release: false,
        project: Some("project2".to_string()),
        verbose: 0,
    };

    let conf = Config::load(
        cli,
        &Utf8PathBuf::from("examples/workspace/Cargo.toml"),
        true,
    )
    .unwrap();

    insta::assert_debug_snapshot!(conf);
}
