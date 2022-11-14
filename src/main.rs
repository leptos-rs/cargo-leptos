mod config;
mod run;
pub mod util;

use anyhow::{Error, Result};
use clap::{Parser, Subcommand};
use config::Config;
use run::{cargo, sass, serve, wasm_pack, watch, Html};
use std::env;
use std::sync::mpsc::channel;
use tokio::task::JoinHandle;

#[derive(Debug, Parser)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Build artifacts in release mode, with optimizations
    #[arg(short, long)]
    release: bool,

    /// Build for client side rendering. Useful during development due to faster compile times.
    #[arg(long)]
    csr: bool,

    /// Verbosity (none: errors & warnings, -v: verbose, --vv: very verbose, --vvv: output everything)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Debug, Subcommand, PartialEq)]
enum Commands {
    /// Adds a default leptos.toml file to current directory
    Init,
    /// Compile the client (csr and hydrate) and server
    Build,
    /// Run the cargo tests for app, client and server
    Test,
    /// Serve. In `csr` mode an internal server is used
    Serve,
    /// Serve and automatically reload when files change
    Watch,
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

    util::setup_logging(args.verbose);

    if args.command == Commands::Init {
        return config::save_default_file();
    }
    let config = config::read(&args)?;

    match args.command {
        Commands::Init => panic!(),
        Commands::Build => build_all(&config),
        Commands::Serve => serve(config).await,
        Commands::Test => cargo::run("test", &config.root, &config),
        Commands::Watch => watch(&config).await,
    }
}

async fn serve(config: Config) -> Result<()> {
    util::rm_dir("target/site")?;
    build_client(&config)?;

    if config.csr {
        serve::run(&config).await;
    } else {
        // build server
        cargo::run("build", &config.root, &config)?;
        cargo::run("run", &config.root, &config)?;
    }
    Ok(())
}
fn build_client(config: &Config) -> Result<()> {
    sass::run(&config)?;

    let html = Html::read(&config.index_path)?;

    if config.csr {
        wasm_pack::run("build", &config.root, &config)?;
        html.generate_html()?;
    } else {
        wasm_pack::run("build", &config.root, &config)?;
        html.generate_rust(&config)?;
    }
    Ok(())
}

fn build_all(config: &Config) -> Result<()> {
    util::rm_dir("target/site")?;

    cargo::run("build", &config.root, &config)?;
    sass::run(&config)?;

    let html = Html::read(&config.index_path)?;

    html.generate_html()?;
    html.generate_rust(&config)?;

    let mut config = config.clone();

    config.csr = true;
    wasm_pack::run("build", &config.root, &config)?;
    config.csr = false;
    wasm_pack::run("build", &config.root, &config)?;
    Ok(())
}

async fn watch(config: &Config) -> Result<()> {
    let (tx, rx) = channel::<bool>();
    // load it with one so that it will start the loop
    tx.send(true).unwrap();
    let cfg = config.clone();
    let _ = tokio::spawn(async move { watch::run(cfg, tx).await });

    let mut serve_handle: Option<JoinHandle<Result<(), Error>>> = None;

    while rx.recv().is_ok() {
        serve_handle.map(|h| h.abort());
        let cfg = config.clone();
        serve_handle = Some(tokio::spawn(async move { serve(cfg).await }));
    }
    Ok(())
}
