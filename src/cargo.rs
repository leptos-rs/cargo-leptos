use crate::{Cli, Error, Reportable};
use simplelog as log;
use xshell::{cmd, Shell};

pub fn run(command: &str, args: Cli) -> Result<(), Reportable> {
    let config = args.read_config()?;
    let projs = config.projects();
    if let Some(path) = projs.client {
        try_build(command, &path, args.release).map_err(|e| e.step_context("build the client"))?;
    }
    if let Some(path) = projs.server {
        try_build(command, &path, args.release).map_err(|e| e.step_context("build the server"))?;
    }
    if let Some(path) = projs.app {
        try_build(command, &path, args.release).map_err(|e| e.step_context("build the app"))?;
    }
    Ok(())
}

pub fn try_build(command: &str, path: &str, release: bool) -> Result<(), Error> {
    let sh = Shell::new()?;

    log::debug!("Changing path to: <bold>{path}</>");
    sh.change_dir(path);

    let release = release.then(|| "--release");

    cmd!(sh, "cargo {command} {release...}").run()?;
    Ok(())
}
