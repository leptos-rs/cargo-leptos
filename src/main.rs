mod config;
mod error;
mod run;
pub mod util;

use clap::{Parser, Subcommand};
pub use config::Config;
pub use error::{Error, Reportable};
use run::{cargo, sass, wasm_pack, Html};
use std::env;
use util::StrAdditions;

#[derive(Debug, Parser)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Build artifacts in release mode, with optimizations
    #[arg(short, long)]
    release: bool,

    /// Build for client side rendering. Useful during development due to faster compile times.
    #[arg(short, long)]
    csr: bool,

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

    util::setup_logging(args.verbose);

    if let Err(e) = try_main(args) {
        log::error!("{e}")
    }
}

fn try_main(args: Cli) -> Result<(), Reportable> {
    if args.command == Commands::Init {
        return Config::save_default_file();
    }
    let config = args.read_config()?.leptos;
    let style = &config.style;
    let release = args.release;

    match args.command {
        Commands::Init => panic!(),
        Commands::Build => {
            wasm_pack::run("build", &config.app_path, release)?;
            wasm_pack::run("build", &config.client_path, release)?;
            cargo::run("build", &config.server_path, release)?;
            sass::run(style, release)?;

            let html = Html::read(&config.index_path)?;
            if args.csr {
                let profile = args.release.then_some("release").unwrap_or("debug");
                let file = util::mkdirs(format!("target/site/{profile}/"))?.with("index.html");
                html.generate_html(&file)?;
            } else {
                let file = util::mkdirs(format!("{}/src/", config.app_path))?.with("generated.rs");
                html.generate_rust(&file)?;
            }
            Ok(())
        }
        Commands::Test => {
            cargo::run("test", &config.app_path, release)?;
            cargo::run("test", &config.client_path, release)?;
            cargo::run("test", &config.server_path, release)
        }
        Commands::Clean => {
            cargo::run("clean", &config.app_path, release)?;
            cargo::run("clean", &config.client_path, release)?;
            cargo::run("clean", &config.server_path, release)?;
            util::rm_dir("target")
        }
        Commands::Update => {
            cargo::run("update", &config.app_path, release)?;
            cargo::run("update", &config.client_path, release)?;
            cargo::run("update", &config.server_path, release)
        }
    }
}
