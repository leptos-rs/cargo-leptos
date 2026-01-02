#[cfg(all(test, feature = "full_tests"))]
mod tests;

mod command;
pub mod compile;
pub mod config;
pub mod ext;
pub mod logger;
pub mod service;
pub mod signal;
pub mod wasm_split_tools;
mod internal_prelude {
    pub use crate::ext::{eyre::reexports::*, Paint as _};
    pub use tracing::*;
}

use crate::{config::Commands, ext::PathBufExt, logger::GRAY};
use camino::Utf8PathBuf;
use clap::CommandFactory;
use config::{Cli, Config};
use ext::{fs, Paint};
use signal::Interrupt;
use std::{env, path::PathBuf};

use crate::internal_prelude::*;

pub async fn run(args: Cli) -> Result<()> {
    if let New(new) = args.command {
        return new.run();
    }

    if let Completions { shell } = args.command {
        clap_complete::generate(
            shell,
            &mut Cli::command(),
            "cargo-leptos",
            &mut std::io::stdout(),
        );
        return Ok(());
    }

    let manifest_path = args
        .manifest_path
        .to_owned()
        .unwrap_or_else(|| Utf8PathBuf::from("Cargo.toml"))
        .resolve_home_dir()
        .wrap_err(format!("manifest_path: {:?}", &args.manifest_path))?;
    let mut cwd = Utf8PathBuf::from_path_buf(env::current_dir().unwrap()).unwrap();
    cwd.clean_windows_path();

    let opts = args.opts().unwrap();
    let bin_args = args.bin_args();

    let watch = matches!(args.command, Commands::Watch(_));
    let config = Config::load(opts, &cwd, &manifest_path, watch, bin_args).dot()?;
    env::set_current_dir(&config.working_dir).dot()?;
    debug!(
        "Path working dir {}",
        GRAY.paint(config.working_dir.as_str())
    );

    if config.working_dir.join("package.json").exists() {
        debug!("Path found 'package.json' adding 'node_modules/.bin' to PATH");
        let node_modules = &config.working_dir.join("node_modules");
        if node_modules.exists() {
            match env::var("PATH") {
                Ok(path) => {
                    let mut path_dirs: Vec<PathBuf> = env::split_paths(&path).collect();
                    path_dirs.insert(0, node_modules.join(".bin").into_std_path_buf());
                    // unwrap is safe, because we got the paths from the actual PATH variable
                    env::set_var("PATH", env::join_paths(path_dirs).unwrap());
                }
                Err(_) => warn!("Path PATH environment variable not found, ignoring"),
            }
        } else {
            warn!(
                "Path 'node_modules' folder not found, please install the required packages first"
            );
            warn!("Path continuing without using 'node_modules'");
        }
    }

    let _monitor = Interrupt::run_ctrl_c_monitor();
    use Commands::{Build, Completions, EndToEnd, New, Serve, Test, Watch};
    match args.command {
        Build(_) => command::build_all(&config).await,
        Serve(_) => command::serve(&config.current_project()?).await,
        Test(opts) => command::test_all(&config, &opts.opts_specific).await,
        EndToEnd(_) => command::end2end_all(&config).await,
        Watch(_) => command::watch(&config.current_project()?).await,
        New(_) => unreachable!(r#""new" command should have already been run"#),
        Completions { .. } => unreachable!(r#""completions" command should have already been run"#),
    }
}
