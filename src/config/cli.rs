use std::ffi::OsStr;

use crate::command::NewCommand;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use figment::{providers::{Format, Toml}, Figment};
use serde::{Deserialize, Serialize};
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Serialize, Deserialize)]
pub enum Log {
    /// WASM build (wasm, wasm-opt, walrus)
    Wasm,
    /// Internal reload and csr server (hyper, axum)
    Server,
}

#[derive(Debug, Clone, Parser, Serialize, Deserialize, PartialEq, Default)]
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

    /// Include debug information in Wasm output. Includes source maps and DWARF debug info.
    #[arg(long)]
    pub wasm_debug: bool,

    /// Verbosity (none: info, errors & warnings, -v: verbose, -vv: very verbose).
    #[arg(short, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Minify javascript assets with swc. Applies to release builds only.
    #[arg(long, default_value = "true", value_parser=clap::builder::BoolishValueParser::new(), action = clap::ArgAction::Set)]
    pub js_minify: bool,

    #[command(flatten)]
    #[serde(flatten)]
    pub bin_opts: BinOpts,

    #[command(flatten)]
    #[serde(flatten)]
    pub lib_opts: LibOpts,
}

#[derive(Debug, Clone, Parser, PartialEq, Default, Deserialize, Serialize)]
pub struct BinOpts {
    /// The features to use when compiling the bin target
    #[arg(long)]
    pub bin_features: Vec<String>,

    /// The cargo flags to pass to cargo when compiling the bin target
    #[arg(long)]
    pub bin_cargo_args: Option<Vec<String>>,
}
#[derive(Debug, Clone, Parser, PartialEq, Default, Deserialize, Serialize)]

pub struct LibOpts {
    /// The features to use when compiling the lib target
    #[arg(long)]
    pub lib_features: Vec<String>,

    /// The cargo flags to pass to cargo when compiling the lib target
    #[arg(long)]
    pub lib_cargo_args: Option<Vec<String>>,
}

#[derive(Debug, Parser, Clone, Serialize, Deserialize)]
#[clap(version)]
pub struct Cli {
    /// Path to Cargo.toml.
    #[arg(long, default_value= OsStr::new("./Cargo.toml"))]
    pub manifest_path: Utf8PathBuf,

    /// Output logs from dependencies (multiple --log accepted).
    #[arg(long)]
    pub log: Vec<Log>,

    /// An internal storage variable that determines whether we're in a workspace or not

    #[command(flatten)]
    #[serde(flatten)]
    pub opts: Opts,

    #[command(subcommand)]
    pub command: Commands,
}

impl Cli{
    pub fn figment_file(manifest_path: &Utf8PathBuf) -> Figment{
      Figment::new().merge(Toml::file(manifest_path).nested())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Subcommand, PartialEq)]
pub enum Commands {
    /// Build the server (feature ssr) and the client (wasm with feature hydrate).
    Build,
    /// Run the cargo tests for app, client and server.
    Test,
    /// Start the server and end-2-end tests.
    EndToEnd,
    /// Serve. Defaults to hydrate mode.
    Serve,
    /// Serve and automatically reload when files change.
    Watch,
    /// Start a wizard for creating a new project (using cargo-generate).
    New(NewCommand),
}
