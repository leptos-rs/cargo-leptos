#[cfg(all(test, feature = "full_tests"))]
mod tests;

mod command;
pub mod config;
//mod logger;
// pub mod service;
use crate::config::Commands;
// use crate::logger::GRAY;
use crate::config::get_target;
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
