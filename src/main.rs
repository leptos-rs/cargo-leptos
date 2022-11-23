mod config;
mod fs;
mod logger;
mod run;
mod sync;
mod util;

use anyhow_ext::{Context, Result};
use binary_install::Cache;
use clap::{Parser, Subcommand, ValueEnum};
use config::Config;
use fs::PathBufAdditions;
use run::{
    assets, cargo, end2end,
    new::NewCommand,
    reload, sass, serve, wasm,
    watch::{self},
    Html,
};
use std::{env, path::PathBuf};
use sync::{send_reload, src_or_style_change, wait_for, Msg, MSG_BUS, SHUTDOWN};
use tokio::signal;

lazy_static::lazy_static! {
    pub static ref INSTALL_CACHE: Cache = Cache::new("cargo-leptos").unwrap();
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

    /// Build for client side rendering instead of the default hydrate mode. Useful during development due to faster compile times.
    #[arg(long)]
    csr: bool,

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
    /// Output toml that needs to be added to the Cargo.toml file.
    Config,
    /// Compile the project. Defaults to hydrate mode.
    Build(Opts),
    /// Run the cargo tests for app, client and server.
    Test(Opts),
    /// Start the server and end-2-end tests.
    EndToEnd(Opts),
    /// Serve. Defaults to hydrate mode.
    Serve(Opts),
    /// Serve and automatically reload when files change. Defaults to hydrate mode.
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
        let path = PathBuf::from(path).without_last();
        std::env::set_current_dir(path).dot()?;
    }

    let opts = match &args.command {
        Commands::New(_) => panic!(""),
        Commands::Config => return Ok(println!(include_str!("leptos.toml"))),
        Commands::Build(opts)
        | Commands::Serve(opts)
        | Commands::Test(opts)
        | Commands::EndToEnd(opts)
        | Commands::Watch(opts) => opts,
    };
    logger::setup(opts.verbose, &args.log);

    let config = config::read(&args, opts.clone()).dot()?;

    tokio::spawn(async {
        signal::ctrl_c().await.expect("failed to listen for event");
        log::info!("Leptos ctrl-c received");
        *SHUTDOWN.write().await = true;
        MSG_BUS.send(Msg::ShutDown).unwrap();
    });

    match args.command {
        Commands::Config | Commands::New(_) => panic!(),
        Commands::Build(_) => build(&config, true).await,
        Commands::Serve(_) => serve(&config).await,
        Commands::Test(_) => cargo::test(&config).await,
        Commands::EndToEnd(_) => e2e_test(&config).await,
        Commands::Watch(_) => watch(&config).await,
    }
}

async fn e2e_test(config: &Config) -> Result<()> {
    build(config, true).await.dot()?;
    let handle = if config.cli.csr {
        serve::spawn(&config).await.dot()?
    } else {
        cargo::spawn_run(&config).await
    };
    end2end::run(config).await.dot()?;
    MSG_BUS.send(Msg::ShutDown).dot()?;
    handle.await.dot()?;
    Ok(())
}

async fn build(config: &Config, copy_assets: bool) -> Result<()> {
    log::debug!(r#"Leptos cleaning contents of "target/site/pkg""#);
    fs::rm_dir_content("target/site/pkg").dot()?;
    if copy_assets {
        assets::update(config).dot()?;
    }
    build_client(&config).await.dot()?;

    if !config.cli.csr {
        cargo::build(&config, false).await.dot()?;
    }
    Ok(())
}
async fn build_client(config: &Config) -> Result<()> {
    sass::run(&config).await.dot()?;

    let html = Html::read(&config.leptos.index_file).dot()?;

    if config.cli.csr {
        wasm::build(&config).await.dot()?;
        html.generate_html(&config).dot()?;
    } else {
        wasm::build(&config).await.dot()?;
        html.generate_rust(&config).dot()?;
    }
    Ok(())
}

async fn serve(config: &Config) -> Result<()> {
    build(&config, true).await.dot()?;
    if config.cli.csr {
        serve::spawn(&config).await?.await.dot()
    } else {
        cargo::run(&config).await
    }
}

async fn watch(config: &Config) -> Result<()> {
    let _ = watch::spawn(config).await.dot()?;

    if let Some(assets_dir) = &config.leptos.assets_dir {
        let _ = assets::spawn(assets_dir).await.dot()?;
    }
    if config.cli.csr {
        serve::spawn(&config).await?;
    }

    reload::spawn(&config).await?;

    loop {
        match build(config, false).await {
            Ok(_) => {
                send_reload().await;
                if config.cli.csr {
                    wait_for(src_or_style_change).await;
                } else {
                    cargo::run(&config).await.dot()?;
                }
            }
            Err(e) => {
                log::warn!("Leptos rebuild stopped due to error: {e}");
                wait_for(src_or_style_change).await;
            }
        }
        if *SHUTDOWN.read().await {
            break;
        } else {
            log::info!("Leptos ===================== rebuilding =====================");
        }
    }
    Ok(())
}
