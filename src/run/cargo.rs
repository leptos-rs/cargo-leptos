use crate::config::Config;
use anyhow::{Context, Result};
use xshell::{cmd, Shell};

pub fn run(command: &str, path: &str, config: &Config) -> Result<()> {
    try_build(command, &path, config.release).context(format!("wasm-pack {path}"))
}

pub fn try_build(command: &str, path: &str, release: bool) -> Result<()> {
    let sh = Shell::new()?;

    let release = release.then(|| "--release");

    cmd!(
        sh,
        "cargo {command} {release...} --no-default-features --features=ssr --manifest-path {path}/Cargo.toml"
    )
    .run()?;
    Ok(())
}
