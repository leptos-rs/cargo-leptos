// use camino::{Utf8Path, Utf8PathBuf};
use super::Config;
use current_platform::CURRENT_PLATFORM;
fn opts(project: Option<&str>) -> crate::config::Opts {
    crate::config::Opts {
        release: false,
        js_minify: false,
        precompress: false,
        hot_reload: false,
        project: project.map(|s| s.to_string()),
        verbose: 0,
        features: Vec::new(),
        bin_features: Vec::new(),
        lib_features: Vec::new(),
        bin_cargo_args: None,
        lib_cargo_args: None,
        wasm_debug: false,
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
    insta::with_settings!({filters => vec![
        (format!(r"target/{}/debug", CURRENT_PLATFORM).as_str(), r"target/debug"),
        ]}, {
            let conf = format!("{:#?}", &conf);
            insta::assert_snapshot!(conf);
        }
    );
}

#[test]
fn test_workspace_project1() {
    let cli = opts(Some("project1"));

    let conf = Config::test_load(cli, "examples", "examples/workspace/Cargo.toml", true, None);

    insta::with_settings!({filters => vec![
        (format!(r"target/{}/debug", CURRENT_PLATFORM).as_str(), r"target/debug"),
        ]}, {
            let conf = format!("{:#?}", &conf);
            insta::assert_snapshot!(conf);
        }
    );
}

#[test]
fn test_workspace_project2() {
    let cli = opts(Some("project2"));

    let conf = Config::test_load(cli, "examples", "examples/workspace/Cargo.toml", true, None);

    insta::with_settings!({filters => vec![
        (format!(r"target/{}/debug", CURRENT_PLATFORM).as_str(), r"target/debug"),
        ]}, {
            let conf = format!("{:#?}", &conf);
            insta::assert_snapshot!(conf);
        }
    );
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

    insta::with_settings!({filters => vec![
        (format!(r"target/{}/debug", CURRENT_PLATFORM).as_str(), r"target/debug"),
        ]}, {
            let conf = format!("{:#?}", &conf);
            insta::assert_snapshot!(conf);
        }
    );
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

    insta::with_settings!({filters => vec![
        (format!(r"target/{}/debug", CURRENT_PLATFORM).as_str(), r"target/debug"),
        ]}, {
            let conf = format!("{:#?}", &conf);
            insta::assert_snapshot!(conf);
        }
    );
}

