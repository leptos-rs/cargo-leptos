use camino::Utf8PathBuf;
use cargo_leptos::{config::Cli, ext::anyhow::Result};
use clap::Parser;
use figment::{Figment, providers::{Serialized, Env}};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments. Override CLI config values with those in
    // `Config.toml` and `APP_`-prefixed environment variables.
    let initial_figment = Figment::new()
    .merge(Serialized::defaults(Cli::parse()))    
    .merge(Env::prefixed("LEPTOS_"));

    //println!("INITIAL {initial_figment:#?}");

    let manifest_path: Utf8PathBuf = initial_figment.extract_inner("manifest_path").expect("manifest_path must be set. This should have defaulted to Cargo.toml");
    let test = Cli::figment_file(&manifest_path).select("leptos");
    println!(" TEST{test:#?}");
    let cli: Cli = initial_figment
    .merge(Cli::figment_file(&manifest_path).select("leptos"))
    .extract()?;

    println!("CLI: {cli:#?}");

    // Determine manifest path by determining if we're in a workspace or not
    //
    Ok(())

    //run(args).await
}
