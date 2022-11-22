use crate::{logger::GRAY, Cli, Commands, Opts};
use anyhow::{anyhow, bail, ensure, Context, Result};
use cargo_metadata::{MetadataCommand, Package as CargoPackage};
use regex::Regex;
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Debug, Clone)]
pub struct Config {
    pub cli: Opts,
    pub cargo: CargoPackage,
    pub leptos: LeptosManifest,
    pub watch: bool,
    pub target_directory: String,
}

impl Config {
    /// Get the crate name for the crate at the given path.
    pub fn lib_crate_name(&self) -> String {
        match self
            .cargo
            .targets
            .iter()
            .find(|t| t.kind.iter().any(|k| k == "cdylib"))
        {
            Some(lib) => lib.name.replace("-", "_"),
            None => self.cargo.name.replace("-", "_"),
        }
    }
}
/// read from path or default to 'leptos.toml'
pub fn read(cli: &Cli, opts: Opts) -> Result<Config> {
    let leptos = read_config("Cargo.toml")
        .context(format!("read config: Cargo.toml"))?
        .package
        .metadata
        .leptos;

    let style = Path::new(&leptos.style.file);
    ensure!(style.exists(), "no css/sass/scss file found at: {style:?}",);
    ensure!(style.is_file(), "expected a file, not a dir: {style:?}",);

    let watch = match cli.command {
        Commands::Watch(_) => true,
        _ => false,
    };
    let workspace = MetadataCommand::new().manifest_path("Cargo.toml").exec()?;

    let target_directory = workspace.target_directory.to_string();
    let cargo = workspace
        .root_package()
        .ok_or_else(|| anyhow!("Could not find root package in Cargo.toml"))?
        .clone();

    Ok(Config {
        cli: opts,
        cargo,
        leptos,
        watch,
        target_directory,
    })
}

fn read_config(file: &str) -> Result<ConfigFile> {
    let text = fs::read_to_string(file)?;
    let re: Regex = Regex::new(r#"(?m)^\[package.metadata.leptos\]"#).unwrap();
    let start = match re.find(&text) {
        Some(found) => found.start(),
        None => {
            bail!(
                "Missing Cargo.toml configuration section {}.\n\
            Append the output of {} to your Cargo.toml",
                GRAY.paint("[package.metadata.leptos]"),
                GRAY.paint("cargo leptos init")
            )
        }
    };
    log::trace!("Config file content:\n{text}");

    // so that serde error messages have right line number
    let newlines = text[..start].matches('\n').count();
    let toml = "\n".repeat(newlines) + &text[start..];
    Ok(toml::from_str(&toml)?)
}

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
    leptos: LeptosManifest,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LeptosManifest {
    /// source index file
    pub index_file: String,
    /// where to generate rust code
    pub gen_file: String,
    /// assets dir. content will be copied to the target/site dir
    pub assets_dir: Option<String>,
    /// on which port to serve the client side rendered site
    pub csr_port: u16,
    /// the port to use for automatic reload monitoring
    pub reload_port: u16,
    pub style: Style,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Style {
    pub file: String,
    pub browserquery: String,
}
