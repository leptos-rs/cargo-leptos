use crate::command::NewCommand;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

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

    /// Include debug information in Wasm output. Includes source maps and DWARF debug info.
    #[arg(long)]
    pub wasm_debug: bool,

    /// Verbosity (none: info, errors & warnings, -v: verbose, -vv: very verbose).
    #[arg(short, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Clear the terminal before rebuilding
    #[arg(long, short)]
    pub clear: bool,

    /// Minify javascript assets with swc. Applies to release builds only.
    #[arg(long, default_value = "true", value_parser=clap::builder::BoolishValueParser::new(), action = clap::ArgAction::Set)]
    pub js_minify: bool,

    /// Split WASM binary based on #[lazy] macros.
    #[arg(long)]
    pub split: bool,

    /// Only build the frontend.
    #[arg(long)]
    pub frontend_only: bool,

    /// Only build the server.
    #[arg(long, conflicts_with = "frontend_only")]
    pub server_only: bool,
}

#[derive(Debug, Clone, Parser, PartialEq, Default)]
pub struct BinOpts {
    #[command(flatten)]
    opts: Opts,

    #[arg(trailing_var_arg = true)]
    bin_args: Vec<String>,
}

#[derive(Debug, Clone, Parser, PartialEq, Default)]
pub struct TestSpecificOpts {
    /// Do not run the tests, only build them.
    #[arg(long)]
    pub no_run: bool,
}

impl TestSpecificOpts {
    pub fn to_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if self.no_run {
            args.push("--no-run".to_string());
        }
        args
    }
}

#[derive(Debug, Clone, Parser, PartialEq, Default)]
pub struct TestOpts {
    #[command(flatten)]
    opts: Opts,

    #[command(flatten, next_help_heading = "Test-specific Options")]
    pub opts_specific: TestSpecificOpts,
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
        match &self.command {
            Commands::New(_) => None,
            Commands::Serve(bin_opts) | Commands::Watch(bin_opts) => Some(bin_opts.opts.clone()),
            Commands::Test(test_opts) => Some(test_opts.opts.clone()),
            Commands::Build(opts) | Commands::EndToEnd(opts) => Some(opts.clone()),
            _ => None,
        }
    }

    pub fn opts_mut(&mut self) -> Option<&mut Opts> {
        match &mut self.command {
            Commands::New(_) => None,
            Commands::Serve(bin_opts) | Commands::Watch(bin_opts) => Some(&mut bin_opts.opts),
            Commands::Test(test_opts) => Some(&mut test_opts.opts),
            Commands::Build(opts) | Commands::EndToEnd(opts) => Some(opts),
            _ => None,
        }
    }

    pub fn bin_args(&self) -> Option<&[String]> {
        match &self.command {
            Commands::Serve(bin_opts) | Commands::Watch(bin_opts) => {
                Some(bin_opts.bin_args.as_ref())
            }
            _ => None,
        }
    }
}

#[derive(Debug, Subcommand, PartialEq)]
pub enum Commands {
    /// Build the server (feature ssr) and the client (wasm with feature hydrate).
    Build(Opts),
    /// Run the cargo tests for app, client and server.
    Test(TestOpts),
    /// Start the server and end-2-end tests.
    EndToEnd(Opts),
    /// Serve. Defaults to hydrate mode.
    Serve(BinOpts),
    /// Serve and automatically reload when files change.
    Watch(BinOpts),
    /// Start a wizard for creating a new project (using cargo-generate).
    New(NewCommand),

    /// Generate shell for `cargo-leptos`
    Completions { shell: Shell },
}
