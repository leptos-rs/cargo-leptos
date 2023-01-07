#[cfg(all(test, feature = "full_tests"))]
mod tests;

mod command;
pub mod compile;
pub mod config;
mod ext;
mod logger;
pub mod service;
pub mod signal;

use crate::ext::anyhow::{Context, Result};
use crate::ext::PathBufExt;
use crate::logger::GRAY;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use command::NewCommand;
use config::Config;
use ext::fs;
use signal::Interrupt;
use std::env;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Log {
    /// WASM build (wasm, wasm-opt, walrus)
    Wasm,
    /// Internal reload and csr server (hyper, axum)
    Server,
}

#[derive(Debug, Clone, Parser, PartialEq, Default)]
pub struct Opts {
    /// Build artifacts in release mode, with optimizations.
    #[arg(short, long)]
    release: bool,

    /// Which project to use, from a list of projects defined in a workspace
    #[arg(short, long)]
    pub project: Option<String>,

    /// The features to use when compiling the lib target
    #[arg(long)]
    pub lib_features: Vec<String>,

    /// The features to use when compiling the bin target
    #[arg(long)]
    pub bin_features: Vec<String>,

    /// Verbosity (none: info, errors & warnings, -v: verbose, --vv: very verbose).
    #[arg(short, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

impl Opts {
    pub fn profile(&self) -> String {
        if self.release { "release" } else { "debug" }.to_string()
    }
}

#[derive(Debug, Parser)]
#[clap(version)]
pub struct Cli {
    /// Path to Cargo.toml.
    #[arg(long)]
    manifest_path: Option<Utf8PathBuf>,

    /// Output logs from dependencies (multiple --log accepted).
    #[arg(long)]
    log: Vec<Log>,

    #[command(subcommand)]
    command: Commands,
}

impl Cli {
    fn opts(&self) -> Option<Opts> {
        use Commands::{Build, EndToEnd, New, Serve, Test, Watch};
        match &self.command {
            New(_) => None,
            Build(opts) | Serve(opts) | Test(opts) | EndToEnd(opts) | Watch(opts) => {
                Some(opts.clone())
            }
        }
    }
}

#[derive(Debug, Subcommand, PartialEq)]
enum Commands {
    /// Build the server (feature ssr) and the client (wasm with feature hydrate).
    Build(Opts),
    /// Run the cargo tests for app, client and server.
    Test(Opts),
    /// Start the server and end-2-end tests.
    EndToEnd(Opts),
    /// Serve. Defaults to hydrate mode.
    Serve(Opts),
    /// Serve and automatically reload when files change.
    Watch(Opts),
    /// WIP: Start wizard for creating a new project (using cargo-generate). Ask at Leptos discord before using.
    New(NewCommand),
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args: Vec<String> = env::args().collect();
    // when running as cargo leptos, the second argument is "leptos" which
    // clap doesn't expect
    if args.get(1).map(|a| a == "leptos").unwrap_or(false) {
        args.remove(1);
    }

    let args = Cli::parse_from(&args);
    run(args).await
}

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
