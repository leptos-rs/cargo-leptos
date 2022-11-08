mod cargo;
mod config;
mod error;
mod sass;
mod wasm_pack;

use clap::{Parser, Subcommand};
pub use config::Config;
pub use error::{Error, Reportable};
use log::LevelFilter;
use simplelog::{ColorChoice, ConfigBuilder, TermLogger, TerminalMode};
use std::{env, fs, path::Path};

#[derive(Debug, Parser)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Build artifacts in release mode, with optimizations
    #[arg(short, long)]
    release: bool,

    /// Verbosity (none: errors & warnings, -v: verbose, --vv: very verbose, --vvv: output everything)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Path to configuration file (defaults to './leptos.toml')
    #[arg(short, long)]
    config: Option<String>,
}

impl Cli {
    pub fn read_config(&self) -> Result<Config, Reportable> {
        Config::read(&self.config)
    }
}

#[derive(Debug, Subcommand, PartialEq)]
enum Commands {
    /// Adds a default leptos.toml file to current directory
    Init,
    /// Compile the client and server
    Build,
    /// Remove the target directories (in app, client and server)
    Clean,
    /// Run the cargo tests for app, client and server
    Test,
    /// Run the cargo update for app, client and server
    Update,
}

fn main() {
    let mut args: Vec<String> = env::args().collect();
    // when running as cargo leptos, the second argument is "leptos" which
    // clap doesn't expect
    if args.get(1).map(|a| a == "leptos").unwrap_or(false) {
        args.remove(1);
    }

    let args = Cli::parse_from(&args);

    setup_logging(args.verbose);

    if let Err(e) = try_main(args) {
        log::error!("{e}")
    }
}

fn try_main(args: Cli) -> Result<(), Reportable> {
    if args.command == Commands::Init {
        return Config::save_default_file();
    }
    let config = args.read_config()?;
    let projects = config.projects();
    let style = config.style();
    let release = args.release;
    match args.command {
        Commands::Init => panic!(),
        Commands::Build => {
            wasm_pack::run("build", &projects.app, release)?;
            wasm_pack::run("build", &projects.client, release)?;
            cargo::run("build", &projects.server, release)?;
            sass::run(style, release)
        }
        Commands::Test => {
            cargo::run("test", &projects.app, release)?;
            cargo::run("test", &projects.client, release)?;
            cargo::run("test", &projects.server, release)
        }
        Commands::Clean => {
            cargo::run("clean", &projects.app, release)?;
            cargo::run("clean", &projects.client, release)?;
            cargo::run("clean", &projects.server, release)?;
            rm_dir("target")
        }
        Commands::Update => {
            cargo::run("update", &projects.app, release)?;
            cargo::run("update", &projects.client, release)?;
            cargo::run("update", &projects.server, release)
        }
    }
}

fn setup_logging(verbose: u8) {
    let log_level = match verbose {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    let config = ConfigBuilder::default()
        .set_time_level(LevelFilter::Off)
        .build();
    TermLogger::init(log_level, config, TerminalMode::Stderr, ColorChoice::Auto)
        .expect("Failed to start logger");
    log::info!("Log level set to: {log_level}");
}

fn rm_dir(dir: &str) -> Result<(), Reportable> {
    let path = Path::new(&dir);

    if !path.exists() {
        log::debug!("Not cleaning {dir} because it does not exist");
        return Ok(());
    }
    if !path.is_dir() {
        log::warn!("Not cleaning {dir} because it is not a directory");
        return Ok(());
    }

    log::info!("Cleaning dir '{dir}'");
    fs::remove_dir_all(path).map_err(|e| Into::<Error>::into(e).file_context("remove dir", dir))?;
    Ok(())
}
