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

pub async fn run(args: Cli) -> Result<()> {
    let verbose = args.opts().map(|o| o.verbose).unwrap_or(0);
    logger::setup(verbose, &args.log);

    if let Commands::New(new) = &args.command {
        return new.run().await;
    }

    let manifest_path = args
        .manifest_path
        .to_owned()
        .unwrap_or_else(|| Utf8PathBuf::from("Cargo.toml"))
        .resolve_home_dir()
        .context(format!("manifest_path: {:?}", &args.manifest_path))?;
    let mut cwd = Utf8PathBuf::from_path_buf(std::env::current_dir().unwrap()).unwrap();
    cwd.clean_windows_path();

    let opts = args.opts().unwrap();

    let watch = matches!(args.command, Commands::Watch(_));
    let config = Config::load(opts, &cwd, &manifest_path, watch).dot()?;
    env::set_current_dir(&config.working_dir).dot()?;
    log::debug!(
        "Path working dir {}",
        GRAY.paint(config.working_dir.as_str())
    );

    let _monitor = Interrupt::run_ctrl_c_monitor();
    match args.command {
        Commands::New(_) => panic!(),
        Commands::Build(_) => command::build_all(&config).await,
        Commands::Serve(_) => command::serve(&config.current_project()?).await,
        Commands::Test(_) => command::test_all(&config).await,
        Commands::EndToEnd(_) => command::end2end::end2end_all(&config).await,
        Commands::EndToEndWithWatch(_) => command::end2end::end2end_all_with_watch(&config).await,
        Commands::Watch(_) => command::watch(&config.current_project()?).await,
    }
}
