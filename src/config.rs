use crate::{Cli, Commands, Opts};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

#[derive(Deserialize, Debug)]
struct ConfigFile {
    pub package: Package,
}

#[derive(Deserialize, Debug)]
struct Package {
    metadata: Metadata,
}

#[derive(Deserialize, Debug)]
struct Metadata {
    leptos: Config,
}

/// read from path or default to 'leptos.toml'
pub fn read(cli: &Cli, opts: Opts) -> Result<Config> {
    let file = cli.manifest_path.as_deref().unwrap_or("Cargo.toml");
    let mut conf = read_config(file)
        .context(format!("read config: {file}"))?
        .package
        .metadata
        .leptos;
    conf.cli = opts;
    conf.watch = match cli.command {
        Commands::Watch(_) => true,
        _ => false,
    };
    Ok(conf)
}

fn read_config(file: &str) -> Result<ConfigFile> {
    let text = fs::read_to_string(file)?;
    let start = text.find("[leptos").unwrap_or(0);
    log::trace!("Config file content:\n{text}");

    // so that serde error messages have right line number
    let newlines = text[..start].matches('\n').count();
    let toml = "\n".repeat(newlines) + &text[start..];
    Ok(toml::from_str(&toml)?)
}

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    pub index_path: String,
    /// where to generate rust code
    pub gen_path: String,
    /// on which port to serve the client side rendered site
    pub csr_port: u16,
    /// the port to use for automatic reload monitoring
    pub reload_port: u16,
    pub style: Style,

    // parameters from cmd-line args
    #[serde(skip_deserializing)]
    pub cli: Opts,
    #[serde(skip_deserializing)]
    pub watch: bool,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Style {
    pub file: String,
    pub browserquery: String,
}
