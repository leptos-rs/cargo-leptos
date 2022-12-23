mod command;
pub mod compile;
pub mod config;
mod ext;
mod logger;
pub mod service;
pub mod signal;

use crate::ext::anyhow::{Context, Result};
use crate::logger::GRAY;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use command::NewCommand;
use config::Config;
use ext::path::PathBufExt;
use ext::{fs, path, util};
use once_cell::sync::OnceCell;
use signal::Interrupt;
use std::env;

lazy_static::lazy_static! {
    pub static ref WORKING_DIR: OnceCell<Utf8PathBuf> = OnceCell::new();
}

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
    project: Option<String>,

    /// Verbosity (none: info, errors & warnings, -v: verbose, --vv: very verbose).
    #[arg(short, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Debug, Parser)]
#[clap(version)]
pub struct Cli {
    /// Path to Cargo.toml.
    #[arg(long)]
    manifest_path: Option<String>,

    /// Output logs from dependencies (multiple --log accepted).
    #[arg(long)]
    log: Vec<Log>,

    #[command(subcommand)]
    command: Commands,
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

    if let Commands::New(new) = &args.command {
        return new.run().await;
    }

    if let Some(path) = &args.manifest_path {
        let path = Utf8PathBuf::from(path)
            .without_last()
            .canonicalize_utf8()
            .dot()?;
        std::env::set_current_dir(&path).dot()?;
        WORKING_DIR.set(path).unwrap();
    } else {
        let path = Utf8PathBuf::from_path_buf(std::env::current_dir().unwrap()).unwrap();
        WORKING_DIR.set(path).unwrap();
    }

    use Commands::{Build, EndToEnd, New, Serve, Test, Watch};
    let opts = match &args.command {
        New(_) => panic!(""),
        Build(opts) | Serve(opts) | Test(opts) | EndToEnd(opts) | Watch(opts) => opts,
    };

    logger::setup(opts.verbose, &args.log);
    log::trace!(
        "Path working dir {}",
        GRAY.paint(WORKING_DIR.get().unwrap().as_str())
    );

    let config = Config::load(&args, opts.clone()).dot()?;

    let _monitor = Interrupt::run_ctrl_c_monitor();
    match args.command {
        New(_) => panic!(),
        Build(_) => command::build_all(&config).await,
        Serve(_) => command::serve(&config.current_project()?).await,
        Test(_) => command::test_all(&config).await,
        EndToEnd(_) => command::end2end_all(&config).await,
        Watch(_) => command::watch(&config.current_project()?).await,
    }
}
