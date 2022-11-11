use std::path::Path;

use crate::{config::Config, util, Error, Reportable};
use simplelog as log;
use xshell::{cmd, Shell};

pub fn run(command: &str, path: &str, config: &Config) -> Result<(), Reportable> {
    try_build(command, &path, config.release)
        .map_err(|e| e.step_context(format!("wasm-pack {command} {path}")))?;

    util::rm_file(format!("target/site/pkg/.gitignore"))?;
    util::rm_file(format!("target/site/pkg/package.json"))?;
    Ok(())
}

pub fn try_build(command: &str, path: &str, release: bool) -> Result<(), Error> {
    let path_depth = Path::new(path).components().count();
    let to_root = (0..path_depth).map(|_| "..").collect::<Vec<_>>().join("/");

    let dest = format!("{to_root}/target/site/pkg");

    let sh = Shell::new()?;

    log::debug!("Running sh in path: <bold>{path}</>");
    sh.change_dir(path);

    let release = release.then(|| "--release").unwrap_or("--dev");

    cmd!(
        sh,
        "wasm-pack {command} --target=web --out-dir {dest} --out-name app --no-typescript {release}"
    )
    .run()?;
    Ok(())
}
