use std::path::Path;

use crate::{config::Config, Error, Reportable};
use simplelog as log;
use xshell::{cmd, Shell};

pub fn run(command: &str, path: &str, config: &Config) -> Result<(), Reportable> {
    Ok(try_build(command, &path, config.release)
        .map_err(|e| e.step_context(format!("wasm-pack {command} {path}")))?)
}

pub fn try_build(command: &str, path: &str, release: bool) -> Result<(), Error> {
    let sh = Shell::new()?;

    log::debug!("Changing path to: <bold>{path}</>");
    sh.change_dir(path);

    let path_depth = Path::new(path).components().count();
    let to_root = (0..path_depth).map(|_| "..").collect::<Vec<_>>().join("/");
    let dest = format!("{to_root}/target/{path}");
    let release = release.then(|| "--release");

    cmd!(
        sh,
        "wasm-pack {command} --target=web --out-dir {dest} --out-name app --no-typescript {release...}"
    )
    .run()?;
    Ok(())
}
