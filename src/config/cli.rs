use crate::command::NewCommand;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};

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
    pub release: bool,

    /// Which project to use, from a list of projects defined in a workspace
    #[arg(short, long)]
    pub project: Option<String>,

    /// The features to use when compiling all targets
    #[arg(long)]
    pub features: Vec<String>,

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
    pub manifest_path: Option<Utf8PathBuf>,

    /// Output logs from dependencies (multiple --log accepted).
    #[arg(long)]
    pub log: Vec<Log>,

    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    pub fn opts(&self) -> Option<Opts> {
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
pub enum Commands {
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
