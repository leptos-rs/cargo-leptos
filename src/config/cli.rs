use std::ffi::OsStr;

use crate::command::NewCommand;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use figment::{
    providers::{Format, Toml},
    Figment,
};
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

    /// Build only the binary/server target
    #[arg(short, long, default_value = "false")]
    pub bin_only: bool,

    /// Build only the library/front target
    #[arg(short, long, default_value = "false")]
    pub lib_only: bool,

    /// An internal use variable denoting whether this is a workspace project by looking for [workspace] in the manifest
    #[clap(skip)]
    pub is_workspace: bool,

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
    /// The features to use when compiling the bin target, in a comma seperated list
    #[arg(long,value_parser, num_args=1.., value_delimiter=',')]
    pub bin_features: Vec<String>,

    /// The cargo flags to pass to cargo when compiling the bin target, in a comma seperated list
    #[arg(long, value_parser, num_args=1.., value_delimiter=',')]
    pub bin_cargo_args: Option<Vec<String>>,

    /// The command to use to run the build step. Defaults to `cargo` but could be something like
    /// `cargo cross` or `cargo px` for example
    #[arg(long, default_value = "cargo")]
    pub bin_cargo_command: Option<String>,
}
#[derive(Debug, Clone, Parser, PartialEq, Default, Deserialize, Serialize)]

pub struct LibOpts {
    /// The features to use when compiling the lib target, in a comma seperated list
    #[arg(long,value_parser, num_args=1.., value_delimiter=',')]
    pub lib_features: Vec<String>,

    /// The cargo flags to pass to cargo when compiling the lib target, in a comma seperated list
    #[arg(long,value_parser, num_args=1.., value_delimiter=',')]
    pub lib_cargo_args: Option<Vec<String>>,

    /// The command to use to run the build step. Defaults to `cargo` but could be something like
    /// `cargo cross` or `cargo px` for example
    #[arg(long, default_value = "cargo")]
    pub lib_cargo_command: Option<String>,
}

#[derive(Debug, Parser, Clone, Serialize, Deserialize)]
#[clap(version)]
#[serde(rename_all = "kebab-case")]
pub struct Cli {
    /// Path to Cargo.toml.
    #[arg(long, default_value= OsStr::new("./Cargo.toml"))]
    pub manifest_path: Utf8PathBuf,

    /// Name of Lib/frontend crate
    #[arg(long, default_value=None)]
    pub lib_crate_name: Option<String>,
    /// Name of Bin/server crate
    #[arg(long, default_value=None)]
    pub bin_crate_name: Option<String>,

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

impl Cli {
    pub fn figment_file(manifest_path: &Utf8PathBuf) -> Figment {
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
