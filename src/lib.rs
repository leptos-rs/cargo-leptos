#[cfg(all(test, feature = "full_tests"))]
mod tests;

mod command;
pub mod compile;
pub mod config;
pub mod ext;
mod logger;
pub mod service;
pub mod signal;

use crate::config::Commands;
use crate::ext::anyhow::{Context, Result};
use crate::ext::PathBufExt;
use crate::logger::GRAY;
use camino::Utf8PathBuf;
use config::{Cli, Config};
use ext::fs;
use signal::Interrupt;
use std::env;
use std::path::PathBuf;

pub async fn run(args: Cli) -> Result<()> {
    let verbose = args.opts().map(|o| o.verbose).unwrap_or(0);
    logger::setup(verbose, &args.log);

    if let New(new) = &args.command {
        return new.run().await;
    }

    let manifest_path = args
        .manifest_path
        .to_owned()
        .unwrap_or_else(|| Utf8PathBuf::from("Cargo.toml"))
        .resolve_home_dir()
        .context(format!("manifest_path: {:?}", &args.manifest_path))?;
    let mut cwd = Utf8PathBuf::from_path_buf(env::current_dir().unwrap()).unwrap();
    cwd.clean_windows_path();

    let opts = args.opts().unwrap();
    let bin_args = args.bin_args();

    let watch = matches!(args.command, Commands::Watch(_));
    let config = Config::load(opts, &cwd, &manifest_path, watch, bin_args).dot()?;
    env::set_current_dir(&config.working_dir).dot()?;
    log::debug!(
        "Path working dir {}",
        GRAY.paint(config.working_dir.as_str())
    );

    if config.working_dir.join("package.json").exists() {
        log::debug!("Path found 'package.json' adding 'node_modules/.bin' to PATH");
        let node_modules = &config.working_dir.join("node_modules");
        if node_modules.exists() {
            match env::var("PATH") {
                Ok(path) => {
                    let mut path_dirs: Vec<PathBuf> = env::split_paths(&path).collect();
                    path_dirs.insert(0, node_modules.join(".bin").into_std_path_buf());
                    // unwrap is safe, because we got the paths from the actual PATH variable
                    env::set_var("PATH", env::join_paths(path_dirs).unwrap());
                }
                Err(_) => log::warn!("Path PATH environment variable not found, ignoring"),
            }
        } else {
            log::warn!(
                "Path 'node_modules' folder not found, please install the required packages first"
            );
            log::warn!("Path continuing without using 'node_modules'");
        }
    }

    let _monitor = Interrupt::run_ctrl_c_monitor();
    use Commands::{Build, EndToEnd, New, Serve, Test, Watch};
    match args.command {
        New(_) => panic!(),
        Build(_) => command::build_all(&config).await,
        Serve(_) => command::serve(&config.current_project()?).await,
        Test(_) => command::test_all(&config).await,
        EndToEnd(_) => command::end2end_all(&config).await,
        Watch(_) => command::watch(&config.current_project()?).await,
    }
}


pub fn check_wasm_bindgen_version(manifest_path: &str) {
    let our_version = "0.2.93"; // Not sure how to get wasm-bindgen-cli-support to emit it's own version number.
    let manifest = std::fs::read_to_string(manifest_path).expect("Manifest path to be a readable file.");
    if let Some(your_version) = manifest
    .lines()
    .filter_map(|l| {
        let version = l
            .chars()
            .filter(|c| c.is_digit(10) || *c == '.')
            .collect::<String>();

        l.split('=')
            .collect::<Vec<&str>>()
            .first()
            .map(|crate_name| {
                if crate_name.contains("wasm-bindgen") {
                    let remaining = crate_name
                        .split("wasm-bindgen")
                        .collect::<Vec<&str>>()
                        .join("");
                    if remaining.split_whitespace().collect::<String>().is_empty() {
                        Some(version)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }).flatten()
    }).next() {
        if our_version != your_version {
            panic!("{}",format!("The wasm-bindgen in your Cargo.toml has a version number {your_version} but the cargo-leptos version is {our_version} You need to set the wasm-bindgen dependency your project uses to {our_version} instead of {your_version}"));
        }
    }
}