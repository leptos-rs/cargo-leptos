use camino::Utf8PathBuf;
use cargo_leptos::run;
use cargo_leptos::{
    config::{get_target, Cli},
    get_current_dir,
};
use cargo_manifest::Manifest;
// use cargo_metadata::MetadataCommand;
use clap::Parser;
use color_eyre::Result;
use figment::{
    providers::{Env, Serialized},
    Figment,
};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    // Parse CLI arguments. Override CLI config values with those in
    // `Config.toml` and `LEPTOS_`-prefixed environment variables.
    let initial_figment = Figment::new()
        .merge(Serialized::defaults(Cli::parse()))
        .merge(Env::prefixed("LEPTOS_"));

    let manifest_path: Utf8PathBuf = initial_figment
        .extract_inner("manifest-path")
        .expect("manifest_path must be set. This should have defaulted to Cargo.toml");
    let mut cli: Cli = initial_figment
        .merge(Cli::figment_file(&manifest_path).select("leptos"))
        .extract()?;

    // Determine whether we're in a workspace
    let manifest = Manifest::from_path(&manifest_path)
        .expect("Failed to find or parse Cargo.toml at manifest path");

    // cargo-manifest can tell us whether the Cargo.toml manifest we're analyzing is a workspace or not
    let is_workspace = match &manifest.package {
        Some(package) => match package.workspace.is_some() {
            true => true,
            false => false,
        },
        None => false,
    };

    cli.opts.is_workspace = is_workspace;

    // If it's a workspace, and we're not only building the lib target, and the bin name is not set
    if cli.opts.is_workspace && !cli.opts.lib_only && cli.bin_crate_name.is_none() {
        panic!("For a workspace, you must set bin-crate-name in the [leptos] section of your Cargo.toml or pass it on the command line.")
    }
    // If it's a workspace, and we're not only building the bin target, and the lib name is not set
    if cli.opts.is_workspace && !cli.opts.bin_only && cli.lib_crate_name.is_none() {
        panic!("For a workspace, you must set lib-crate-name in the [leptos] section of your Cargo.toml or pass it on the command line.")
    }
    // If not a workspace, and value is not set, set to detected name of package in manifest path
    // We assume the bin crate name is the same as the package name
    if !cli.opts.is_workspace && cli.bin_crate_name.is_none() {
        let name = match &manifest.package{
        Some(package) => package.name.clone(),
        None => panic!("No package name found in manifest and no bin-crate-name provided. Please define one in the [leptos] section of your Cargo.toml or provide it via the command line")
    };
        cli.bin_crate_name = Some(name);
    }
    // We assume the bin crate name is the same as the package name
    if !cli.opts.is_workspace && cli.lib_crate_name.is_none() {
        let name = match &manifest.package{
            Some(package) => package.name.clone(),
            None => panic!("No package name found in manifest and no lib-crate-name provided. Please define one in the [leptos] section of your Cargo.toml or provide it via the command line")
        };
        cli.lib_crate_name = Some(name)
    }

    let cwd = get_current_dir(Some(&cli.manifest_path));
    // Set the bin-root-path and the lib-root-path to different crates if this is a workspace
    if cli.opts.is_workspace {
        if let Some(bin_crate_name) = &cli.bin_crate_name {
            let path = format! {"{cwd}/{bin_crate_name}"};
            cli.opts.bin_opts.bin_root_path = Utf8PathBuf::from(path);
        }

        if let Some(lib_crate_name) = &cli.lib_crate_name {
            let path = format! {"{cwd}/{lib_crate_name}"};
            cli.opts.lib_opts.lib_root_path = Utf8PathBuf::from(path);
        }
    }

    let default_bin_target = env!("TARGET");
    cli.opts.bin_opts.bin_target_triple = Some(default_bin_target.to_string());

    println!("CLI: {cli:#?}");
    run(cli).await
}
