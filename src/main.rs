mod config;
mod run;
pub mod util;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::Config;
use run::{cargo, sass, serve, wasm_pack, watch, Html};
use std::env;
use tokio::{signal, sync::broadcast};

#[derive(Debug, Clone, Copy)]
pub enum InterruptType {
    CtrlC,
    FileChange,
}

lazy_static::lazy_static! {
    /// Interrupts current serve or cargo operation. Used for watch
    pub static ref INTERRUPT: broadcast::Sender<InterruptType> = {
        broadcast::channel(1).0
    };
}

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

    tokio::spawn(async {
        signal::ctrl_c().await.expect("failed to listen for event");
        INTERRUPT.send(InterruptType::CtrlC).unwrap();
    });

    match args.command {
        Commands::Init => panic!(),
        Commands::Build => build_all(&config).await,
        Commands::Serve => serve(config).await,
        Commands::Test => cargo::test(&config).await,
        Commands::Watch => watch(&config).await,
    }
}

async fn serve(config: Config) -> Result<()> {
    util::rm_dir("target/site")?;
    build_client(&config).await?;

    if config.csr {
        serve::run(&config).await;
    } else {
        // build server
        cargo::build(&config).await?;
        cargo::run(&config).await?;
    }
    Ok(())
}
async fn build_client(config: &Config) -> Result<()> {
    sass::run(&config).await?;

    let html = Html::read(&config.index_path)?;

    if config.csr {
        wasm_pack::build(&config).await?;
        html.generate_html()?;
    } else {
        wasm_pack::build(&config).await?;
        html.generate_rust(&config)?;
    }
    Ok(())
}

async fn build_all(config: &Config) -> Result<()> {
    util::rm_dir("target/site")?;

    cargo::build(&config).await?;
    sass::run(&config).await?;

    let html = Html::read(&config.index_path)?;

    html.generate_html()?;
    html.generate_rust(&config)?;

    let mut config = config.clone();

    config.csr = true;
    wasm_pack::build(&config).await?;
    config.csr = false;
    wasm_pack::build(&config).await?;
    Ok(())
}

async fn watch(config: &Config) -> Result<()> {
    let cfg = config.clone();
    let _ = tokio::spawn(async move { watch::run(cfg).await });

    let mut interrupt = INTERRUPT.subscribe();
    loop {
        let cfg = config.clone();
        let serve_handle = tokio::spawn(async move { serve(cfg).await });
        let stop = match interrupt.recv().await {
            Ok(InterruptType::CtrlC) => true,
            Ok(InterruptType::FileChange) => false,
            Err(e) => {
                log::error!("{e}");
                true
            }
        };
        serve_handle.await??;
        if stop {
            return Ok(());
        }
    }
}
