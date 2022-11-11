use crate::{Error, Reportable};
use xshell::{cmd, Shell};

pub fn run(command: &str, path: &str, release: bool) -> Result<(), Reportable> {
    Ok(try_build(command, &path, release)
        .map_err(|e| e.step_context(format!("wasm-pack {path}")))?)
}

pub fn try_build(command: &str, path: &str, release: bool) -> Result<(), Error> {
    let sh = Shell::new()?;

    let release = release.then(|| "--release");

    cmd!(
        sh,
        "cargo {command} {release...} --manifest-path {path}/Cargo.toml"
    )
    .run()?;
    Ok(())
}
