use super::Config;

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
    }
}

// this test causes issues in CI because the tailwind tmp_file field is an absolute path,
// so differs by platform
/* #[test]
fn test_project() {
    let cli = opts(None);

    let conf = Config::test_load(cli, "examples", "examples/project/Cargo.toml", true);

    insta::assert_debug_snapshot!(conf);
} */

#[test]
fn test_workspace() {
    let cli = opts(None);

    let conf = Config::test_load(cli, "examples", "examples/workspace/Cargo.toml", true, None);

    insta::assert_debug_snapshot!(conf);
}

#[test]
fn test_workspace_project1() {
    let cli = opts(Some("project1"));

    let conf = Config::test_load(cli, "examples", "examples/workspace/Cargo.toml", true, None);

    insta::assert_debug_snapshot!(conf);
}

#[test]
fn test_workspace_project2() {
    let cli = opts(Some("project2"));

    let conf = Config::test_load(cli, "examples", "examples/workspace/Cargo.toml", true, None);

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
        None,
    );

    insta::assert_debug_snapshot!(conf);
}

#[test]
fn test_workspace_bin_args_project2() {
    let cli = opts(Some("project2"));

    let conf = Config::test_load(
        cli,
        "examples",
        "examples/workspace/Cargo.toml",
        true,
        Some(&["--".to_string(), "--foo".to_string()]),
    );

    insta::assert_debug_snapshot!(conf);
}
