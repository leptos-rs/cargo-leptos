#[cfg(all(test, feature = "full_tests"))]
mod tests;

mod command;
pub mod compile;
pub mod config;
pub mod ext;
mod logger;
pub mod service;
pub mod signal;

use crate::config::Commands;
use crate::ext::anyhow::{Context, Result};
use crate::ext::PathBufExt;
use crate::logger::GRAY;
use camino::Utf8PathBuf;
use config::{Cli, Config};
use ext::fs;
use signal::Interrupt;
use std::env;

// pub async fn run(args: Cli) -> Result<()> {
//     let verbose = args.opts.verbose;
//     logger::setup(verbose, &args.log);

//     // If we're generating a 
//     if matches!(&args.command,Commands::New(_)){
//         return new.run().await;
//     }

//     let manifest_path = args
//         .manifest_path
//         .resolve_home_dir()
//         .context(format!("manifest_path: {:?}", &args.manifest_path))?;
//     let mut cwd = Utf8PathBuf::from_path_buf(env::current_dir().unwrap()).unwrap();
//     cwd.clean_windows_path();


//     let opts = args.opts;
//     let bin_args = opts.bin_opts;

//     let watch = matches!(args.command, Commands::Watch);
//     let config = Config::load(opts, &cwd, &manifest_path, watch, bin_args).dot()?;
//     env::set_current_dir(&config.working_dir).dot()?;
//     log::debug!(
//         "Path working dir {}",
//         GRAY.paint(config.working_dir.as_str())
//     );

//     let _monitor = Interrupt::run_ctrl_c_monitor();
//     use Commands::{Build, EndToEnd, New, Serve, Test, Watch};
//     match args.command {
//         New(_) => panic!(),
//         Build => command::build_all(&config).await,
//         Serve => command::serve(&config.current_project()?).await,
//         Test=> command::test_all(&config).await,
//         EndToEnd => command::end2end_all(&config).await,
//         Watch=> command::watch(&config.current_project()?).await,
//     }
// }



pub fn check_current_dir(path: Option<Utf8PathBuf>) -> Utf8PathBuf{
    // If path to manifest provided, get directory
    if let Some(manifest_path) = path {
        if manifest_path.is_file(){
            manifest_path.parent().expect("This path doesn't have a parent and it should").into()
        } 
        else{
            panic!("A path was provided, but it was not a path to a Cargo.toml file")
        } 
    }
    // else provide current directory
    else{
        Utf8PathBuf::from("./")
}
}
