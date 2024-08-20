#[cfg(all(test, feature = "full_tests"))]
mod tests;

mod command;
pub mod config;
//mod logger;
// pub mod service;
use crate::config::Commands;
// use crate::logger::GRAY;
use crate::config::Cli;
use camino::Utf8PathBuf;
use color_eyre::eyre::Result;

pub async fn run(cli: Cli) -> Result<()> {
    //let verbose = cli.opts.verbose;
    //logger::setup(verbose, &cli.log);

    if let New(new) = &cli.command {
        return new.run().await;
    }

    //let mut cwd = get_current_dir(Some(&cli.manifest_path));
    //cwd.clean_windows_path();

    //let opts = cli.opts.clone();
    //let bin_args = opts.bin_opts.clone();

    //let watch = matches!(cli.command, Commands::Watch);

    //let _monitor = Interrupt::run_ctrl_c_monitor();
    use Commands::{Build, New};
    match cli.command {
        New(_) => panic!(),
        Build => command::build_all(&cli).await,
        _ => todo!(),
        // Serve => command::serve(&config.current_project()?).await,
        // Test => command::test_all(&config).await,
        // EndToEnd => command::end2end_all(&config).await,
        // Watch => command::watch(&config.current_project()?).await,
    }
}

// Check if path to Cargo.toml is valid, and find it's parent
pub fn get_current_dir(path: Option<&Utf8PathBuf>) -> Utf8PathBuf {
    // If path to manifest provided, get directory
    if let Some(manifest_path) = path {
        if manifest_path.is_file() {
            manifest_path
                .parent()
                .expect("This path doesn't have a parent and it should")
                .into()
        } else {
            panic!("A path was provided, but it was not a path to a Cargo.toml file")
        }
    }
    // else provide current directory
    else {
        Utf8PathBuf::from("./")
    }
}

pub fn check_wasm_bindgen_version(manifest_path: &str) {
    let our_version = "0.2.93"; // Not sure how to get wasm-bindgen-cli-support to emit it's own version number.
    let manifest = std::fs::read_to_string(manifest_path).expect("Manifest path to be a readable file.");
    if let Some(your_version) = manifest
    .lines()
    .filter_map(|l| {
        let version = l
            .chars()
            .filter(|c| c.is_digit(10) || *c == '.')
            .collect::<String>();

        l.split('=')
            .collect::<Vec<&str>>()
            .first()
            .map(|crate_name| {
                if crate_name.contains("wasm-bindgen") {
                    let remaining = crate_name
                        .split("wasm-bindgen")
                        .collect::<Vec<&str>>()
                        .join("");
                    if remaining.split_whitespace().collect::<String>().is_empty() {
                        Some(version)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }).flatten()
    }).next() {
        if our_version != your_version {
            panic!("{}",format!("The wasm-bindgen in your Cargo.toml has a version number {your_version} but the cargo-leptos version is {our_version} You need to set the wasm-bindgen dependency your project uses to {our_version} instead of {your_version}"));
        }
    }
}