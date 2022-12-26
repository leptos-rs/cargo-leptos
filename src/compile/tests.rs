use crate::{compile::front::build_cargo_front_cmd, config::Config};
use camino::Utf8PathBuf;
use insta::assert_display_snapshot;
use tokio::process::Command;

use super::server::build_cargo_server_cmd;

fn release_opts() -> crate::Opts {
    crate::Opts {
        release: true,
        project: None,
        verbose: 0,
        bin_features: Vec::new(),
        lib_features: Vec::new(),
    }
}
fn dev_opts() -> crate::Opts {
    crate::Opts {
        release: false,
        project: None,
        verbose: 0,
        bin_features: Vec::new(),
        lib_features: Vec::new(),
    }
}

#[test]
fn test_project_dev() {
    let cli = dev_opts();
    let conf = Config::load(cli, &Utf8PathBuf::from("examples/project/Cargo.toml"), true).unwrap();

    let mut command = Command::new("cargo");
    let (envs, cargo) = build_cargo_server_cmd("build", &conf.projects[0], &mut command);

    assert_eq!(
        "\
    OUTPUT_NAME=example \
    LEPTOS_SITE_ROOT=target/site \
    LEPTOS_SITE_PKG_DIR=pkg \
    LEPTOS_SITE_ADDR=127.0.0.1:3000 \
    LEPTOS_RELOAD_PORT=3001 \
    LEPTOS_WATCH=ON",
        envs
    );

    assert_display_snapshot!(cargo, @"cargo build --package=example --bin=example --target-dir=target/server --no-default-features --features=ssr");

    let mut command = Command::new("cargo");
    let (_, cargo) = build_cargo_front_cmd("build", true, &conf.projects[0], &mut command);

    assert_display_snapshot!(cargo, @"cargo build --package=example --lib --target-dir=target/front --target=wasm32-unknown-unknown --no-default-features --features=hydrate");
}

#[test]
fn test_project_release() {
    let cli = release_opts();
    let conf = Config::load(cli, &Utf8PathBuf::from("examples/project/Cargo.toml"), true).unwrap();

    let mut command = Command::new("cargo");
    let (_, cargo) = build_cargo_server_cmd("build", &conf.projects[0], &mut command);

    assert_display_snapshot!(cargo, @"cargo build --package=example --bin=example --target-dir=target/server --no-default-features --features=ssr --release");

    let mut command = Command::new("cargo");
    let (_, cargo) = build_cargo_front_cmd("build", true, &conf.projects[0], &mut command);

    assert_display_snapshot!(cargo, @"cargo build --package=example --lib --target-dir=target/front --target=wasm32-unknown-unknown --no-default-features --features=hydrate --release");
}

#[test]
fn test_workspace_project1() {
    let cli = dev_opts();
    let conf = Config::load(
        cli,
        &Utf8PathBuf::from("examples/workspace/Cargo.toml"),
        true,
    )
    .unwrap();

    let mut command = Command::new("cargo");
    let (_, cargo) = build_cargo_server_cmd("build", &conf.projects[0], &mut command);

    assert_display_snapshot!(cargo, @"cargo build --package=server-package --bin=server-package --target-dir=target/server --no-default-features");

    let mut command = Command::new("cargo");
    let (_, cargo) = build_cargo_front_cmd("build", true, &conf.projects[0], &mut command);

    assert_display_snapshot!(cargo, @"cargo build --package=front-package --lib --target-dir=target/front --target=wasm32-unknown-unknown --no-default-features");
}

#[test]
fn test_workspace_project2() {
    let cli = dev_opts();
    let conf = Config::load(
        cli,
        &Utf8PathBuf::from("examples/workspace/Cargo.toml"),
        true,
    )
    .unwrap();

    let mut command = Command::new("cargo");
    let (_, cargo) = build_cargo_server_cmd("build", &conf.projects[1], &mut command);

    assert_display_snapshot!(cargo, @"cargo build --package=project2 --bin=project2 --target-dir=target/server --no-default-features --features=ssr");

    let mut command = Command::new("cargo");
    let (_, cargo) = build_cargo_front_cmd("build", true, &conf.projects[1], &mut command);

    assert_display_snapshot!(cargo, @"cargo build --package=project2 --lib --target-dir=target/front --target=wasm32-unknown-unknown --no-default-features --features=hydrate");
}
