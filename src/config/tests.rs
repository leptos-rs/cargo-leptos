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

    let conf = Config::test_load(cli, "examples", "examples/project/Cargo.toml", true);

    insta::assert_debug_snapshot!(conf);
}

#[test]
fn test_workspace() {
    let cli = opts(None);

    let conf = Config::test_load(cli, "examples", "examples/workspace/Cargo.toml", true);

    insta::assert_debug_snapshot!(conf);
}

#[test]
fn test_workspace_project1() {
    let cli = opts(Some("project1"));

    let conf = Config::test_load(cli, "examples", "examples/workspace/Cargo.toml", true);

    insta::assert_debug_snapshot!(conf);
}

#[test]
fn test_workspace_project2() {
    let cli = opts(Some("project2"));

    let conf = Config::test_load(cli, "examples", "examples/workspace/Cargo.toml", true);

    insta::assert_debug_snapshot!(conf);
}

#[test]
fn test_workspace_in_subdir_project2() {
    let cli = opts(None);

    let conf = Config::test_load(
        cli,
        "examples/workspace/project2",
        "examples/workspace/Cargo.toml",
        true,
    );

    insta::assert_debug_snapshot!(conf);
}
