use crate::{
    compile::front::build_cargo_front_cmd,
    config::{Config, Opts},
};
use insta::assert_snapshot;
use tokio::process::Command;

use super::server::build_cargo_server_cmd;

fn release_opts() -> Opts {
    Opts {
        release: true,
        js_minify: true,
        precompress: false, // if set to true, testing could take quite a while longer
        hot_reload: false,
        project: None,
        verbose: 0,
        features: Vec::new(),
        bin_features: Vec::new(),
        lib_features: Vec::new(),
        bin_cargo_args: None,
        lib_cargo_args: None,
        wasm_debug: false,
        bin_only: false,
        lib_only: false
    }
}
fn dev_opts() -> Opts {
    Opts {
        release: false,
        js_minify: false,
        precompress: false,
        hot_reload: false,
        project: None,
        verbose: 0,
        features: Vec::new(),
        bin_features: Vec::new(),
        lib_features: Vec::new(),
        bin_cargo_args: None,
        lib_cargo_args: None,
        wasm_debug: false,
        bin_only: false,
        lib_only: false
    }
}

#[test]
fn test_project_dev() {
    let cli = dev_opts();
    let conf = Config::test_load(cli, "examples", "examples/project/Cargo.toml", true, None);

    let mut command = Command::new("cargo");
    let (envs, cargo) = build_cargo_server_cmd("build", &conf.projects[0], &mut command);

    const ENV_REF: &str = "\
    LEPTOS_OUTPUT_NAME=example \
    LEPTOS_SITE_ROOT=target/site \
    LEPTOS_SITE_PKG_DIR=pkg \
    LEPTOS_SITE_ADDR=127.0.0.1:3000 \
    LEPTOS_RELOAD_PORT=3001 \
    LEPTOS_LIB_DIR=. \
    LEPTOS_BIN_DIR=. \
    LEPTOS_JS_MINIFY=false \
    LEPTOS_HASH_FILES=true \
    LEPTOS_HASH_FILE_NAME=hash.txt \
    LEPTOS_WATCH=true \
    SERVER_FN_PREFIX=/custom/prefix \
    DISABLE_SERVER_FN_HASH=true \
    SERVER_FN_MOD_PATH=true";
    assert_eq!(ENV_REF, envs);

    assert_snapshot!(cargo, @"cargo build --package=example --bin=example --no-default-features --features=ssr");

    let mut command = Command::new("cargo");
    let (_, cargo) = build_cargo_front_cmd("build", true, &conf.projects[0], &mut command);

    assert!(cargo.starts_with("cargo build --package=example --lib --target-dir="));
    // what's in the middle will vary by platform and cwd
    assert!(
        cargo.ends_with("--target=wasm32-unknown-unknown --no-default-features --features=hydrate")
    );
}

#[test]
fn test_project_release() {
    let cli = release_opts();
    let conf = Config::test_load(cli, "examples", "examples/project/Cargo.toml", true, None);

    let mut command = Command::new("cargo");
    let (_, cargo) = build_cargo_server_cmd("build", &conf.projects[0], &mut command);

    assert_snapshot!(cargo, @"cargo build --package=example --bin=example --no-default-features --features=ssr --release");

    let mut command = Command::new("cargo");
    let (_, cargo) = build_cargo_front_cmd("build", true, &conf.projects[0], &mut command);

    assert!(cargo.starts_with("cargo build --package=example --lib --target-dir="));
    // what's in the middle will vary by platform and cwd
    assert!(cargo.ends_with(
        "--target=wasm32-unknown-unknown --no-default-features --features=hydrate --release"
    ));
}

#[test]
fn test_workspace_project1() {
    const ENV_REF: &str = if cfg!(windows) {
        "\
    LEPTOS_OUTPUT_NAME=project1 \
    LEPTOS_SITE_ROOT=target/site/project1 \
    LEPTOS_SITE_PKG_DIR=pkg \
    LEPTOS_SITE_ADDR=127.0.0.1:3000 \
    LEPTOS_RELOAD_PORT=3001 \
    LEPTOS_LIB_DIR=project1\\front \
    LEPTOS_BIN_DIR=project1\\server \
    LEPTOS_JS_MINIFY=false \
    LEPTOS_HASH_FILES=false \
    LEPTOS_WATCH=true \
    SERVER_FN_PREFIX=/custom/prefix \
    DISABLE_SERVER_FN_HASH=true \
    SERVER_FN_MOD_PATH=true"
    } else {
        "\
    LEPTOS_OUTPUT_NAME=project1 \
    LEPTOS_SITE_ROOT=target/site/project1 \
    LEPTOS_SITE_PKG_DIR=pkg \
    LEPTOS_SITE_ADDR=127.0.0.1:3000 \
    LEPTOS_RELOAD_PORT=3001 \
    LEPTOS_LIB_DIR=project1/front \
    LEPTOS_BIN_DIR=project1/server \
    LEPTOS_JS_MINIFY=false \
    LEPTOS_HASH_FILES=false \
    LEPTOS_WATCH=true \
    SERVER_FN_PREFIX=/custom/prefix \
    DISABLE_SERVER_FN_HASH=true \
    SERVER_FN_MOD_PATH=true"
    };

    let cli = dev_opts();
    let conf = Config::test_load(cli, "examples", "examples/workspace/Cargo.toml", true, None);

    let mut command = Command::new("cargo");
    let (envs, cargo) = build_cargo_server_cmd("build", &conf.projects[0], &mut command);

    assert_eq!(ENV_REF, envs);

    assert_snapshot!(cargo, @"cargo build --package=server-package --bin=server-package --no-default-features");

    let mut command = Command::new("cargo");
    let (envs, cargo) = build_cargo_front_cmd("build", true, &conf.projects[0], &mut command);

    assert_eq!(ENV_REF, envs);

    assert!(cargo.starts_with("cargo build --package=front-package --lib --target-dir="));
    // what's in the middle will vary by platform and cwd
    assert!(cargo.ends_with("--target=wasm32-unknown-unknown --no-default-features"));
}

#[test]
fn test_workspace_project2() {
    let cli = dev_opts();
    let conf = Config::test_load(cli, "examples", "examples/workspace/Cargo.toml", true, None);

    let mut command = Command::new("cargo");
    let (_, cargo) = build_cargo_server_cmd("build", &conf.projects[1], &mut command);

    assert_snapshot!(cargo, @"cargo build --package=project2 --bin=project2 --no-default-features --features=ssr");

    let mut command = Command::new("cargo");
    let (_, cargo) = build_cargo_front_cmd("build", true, &conf.projects[1], &mut command);

    assert!(cargo.starts_with("cargo build --package=project2 --lib --target-dir="));
    // what's in the middle will vary by platform and cwd
    assert!(
        cargo.ends_with("--target=wasm32-unknown-unknown --no-default-features --features=hydrate")
    );
}

#[test]
fn test_extra_cargo_args() {
    let cli = Opts {
        lib_cargo_args: Some(vec!["-j".into(), "8".into()]),
        bin_cargo_args: Some(vec!["-j".into(), "16".into()]),
        ..dev_opts()
    };
    let conf = Config::test_load(cli, "examples", "examples/project/Cargo.toml", true, None);

    let mut command = Command::new("cargo");
    let (_, cargo) = build_cargo_server_cmd("build", &conf.projects[0], &mut command);

    assert_snapshot!(cargo, @"cargo build --package=example --bin=example --no-default-features --features=ssr -j 16");

    let mut command = Command::new("cargo");
    let (_, cargo) = build_cargo_front_cmd("build", true, &conf.projects[0], &mut command);

    assert!(cargo.starts_with("cargo build --package=example --lib --target-dir="));
    // what's in the middle will vary by platform and cwd
    assert!(cargo.ends_with(
        "--target=wasm32-unknown-unknown --no-default-features --features=hydrate -j 8"
    ));
}
