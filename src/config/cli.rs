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

    /// Precompress static assets with gzip and brotli. Applies to release builds only.
    #[arg(short = 'P', long)]
    pub precompress: bool,

    /// Turn on partial hot-reloading. Requires rust nightly [beta]
    #[arg(long)]
    pub hot_reload: bool,

    /// Which project to use, from a list of projects defined in a workspace
    #[arg(short, long)]
    pub project: Option<String>,

    /// The features to use when compiling all targets
    #[arg(long)]
    pub features: Vec<String>,

    /// The features to use when compiling the lib target
    #[arg(long)]
    pub lib_features: Vec<String>,

    /// The cargo flags to pass to cargo when compiling the lib target
    #[arg(long)]
    pub lib_cargo_args: Option<Vec<String>>,

    /// The features to use when compiling the bin target
    #[arg(long)]
    pub bin_features: Vec<String>,

    /// The cargo flags to pass to cargo when compiling the bin target
    #[arg(long)]
    pub bin_cargo_args: Option<Vec<String>>,

    /// Verbosity (none: info, errors & warnings, -v: verbose, --vv: very verbose).
    #[arg(short, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Debug, Clone, Parser, PartialEq, Default)]
pub struct BuildOpts {
    /// Build artifacts in release mode, with optimizations.
    #[arg(short, long)]
    pub release: bool,

    /// Precompress static assets with gzip and brotli. Applies to release builds only.
    #[arg(short = 'P', long)]
    pub precompress: bool,

    /// Turn on partial hot-reloading. Requires rust nightly [beta]
    #[arg(long)]
    pub hot_reload: bool,

    /// Which project to use, from a list of projects defined in a workspace
    #[arg(short, long)]
    pub project: Option<String>,

    /// The features to use when compiling all targets
    #[arg(long)]
    pub features: Vec<String>,

    /// The features to use when compiling the lib target
    #[arg(long)]
    pub lib_features: Vec<String>,

    /// The cargo flags to pass to cargo when compiling the lib target
    #[arg(long)]
    pub lib_cargo_args: Option<Vec<String>>,

    /// The features to use when compiling the bin target
    #[arg(long)]
    pub bin_features: Vec<String>,

    /// The cargo flags to pass to cargo when compiling the bin target
    #[arg(long)]
    pub bin_cargo_args: Option<Vec<String>>,

    /// Skip building the bin target
    #[arg(long)]
    pub bin_skip: bool,

    /// Verbosity (none: info, errors & warnings, -v: verbose, --vv: very verbose).
    #[arg(short, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

impl From<BuildOpts> for Opts {
    fn from(value: BuildOpts) -> Self {
        let BuildOpts {
            release,
            precompress,
            hot_reload,
            project,
            features,
            lib_features,
            lib_cargo_args,
            bin_features,
            bin_cargo_args,
            verbose,
            ..
        } = value;

        Self {
            release,
            precompress,
            hot_reload,
            project,
            features,
            lib_features,
            lib_cargo_args,
            bin_features,
            bin_cargo_args,
            verbose,
        }
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
            Build(build_opts) => Some(build_opts.clone().into()),
            Serve(opts) | Test(opts) | EndToEnd(opts) | Watch(opts) => Some(opts.clone()),
        }
    }
}

#[derive(Debug, Subcommand, PartialEq)]
pub enum Commands {
    /// Build the server (feature ssr) and the client (wasm with feature hydrate).
    Build(BuildOpts),
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
