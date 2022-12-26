use camino::Utf8PathBuf;

use super::Config;

fn opts(project: Option<&str>) -> crate::Opts {
    crate::Opts {
        release: false,
        project: project.map(|s| s.to_string()),
        verbose: 0,
        bin_features: Vec::new(),
        lib_features: Vec::new(),
    }
}

#[test]
fn test_project() {
    let cli = opts(None);

    let conf = Config::load(cli, &Utf8PathBuf::from("examples/project/Cargo.toml"), true).unwrap();

    insta::assert_debug_snapshot!(conf);
}

#[test]
fn test_workspace() {
    let cli = opts(None);

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
    let cli = opts(Some("project1"));

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
    let cli = opts(Some("project2"));

    let conf = Config::load(
        cli,
        &Utf8PathBuf::from("examples/workspace/Cargo.toml"),
        true,
    )
    .unwrap();

    insta::assert_debug_snapshot!(conf);
}
