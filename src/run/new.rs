use anyhow_ext::{anyhow, bail, Context, Result};
use clap::Args;
use std::path::PathBuf;
use tokio::process::Command;

use crate::{util::os_arch, INSTALL_CACHE};

// A subset of the cargo-generate commands available.
// See: https://github.com/cargo-generate/cargo-generate/blob/main/src/args.rs

#[derive(Clone, Debug, Args, PartialEq, Eq)]
#[clap(arg_required_else_help(true))]
#[clap(about)]
pub struct NewCommand {
    /// Git repository to clone template from. Can be a URL (like
    /// `https://github.com/rust-cli/cli-template`), a path (relative or absolute), or an
    /// `owner/repo` abbreviated GitHub URL (like `rust-cli/cli-template`).
    #[clap(short, long, group("SpecificPath"))]
    pub git: Option<String>,

    /// Branch to use when installing from git
    #[clap(short, long, conflicts_with = "tag")]
    pub branch: Option<String>,

    /// Tag to use when installing from git
    #[clap(short, long, conflicts_with = "branch")]
    pub tag: Option<String>,

    /// Local path to copy the template from. Can not be specified together with --git.
    #[clap(short, long, group("SpecificPath"))]
    pub path: Option<String>,

    /// Directory to create / project name; if the name isn't in kebab-case, it will be converted
    /// to kebab-case unless `--force` is given.
    #[clap(long, short, value_parser)]
    pub name: Option<String>,

    /// Don't convert the project name to kebab-case before creating the directory.
    /// Note that cargo generate won't overwrite an existing directory, even if `--force` is given.
    #[clap(long, short, action)]
    pub force: bool,

    /// Enables more verbose output.
    #[clap(long, short, action)]
    pub verbose: bool,

    /// Generate the template directly into the current dir. No subfolder will be created and no vcs is initialized.
    #[clap(long, action)]
    pub init: bool,
}

impl NewCommand {
    pub async fn run(&self) -> Result<()> {
        let args = self.to_args();
        let exe = cargo_generate_exe()?;
        let mut process = Command::new(exe)
            .arg("generate")
            .args(&args)
            .spawn()
            .context("Could not spawn command")?;
        process.wait().await.dot()?;
        Ok(())
    }

    pub fn to_args(&self) -> Vec<String> {
        let mut args = vec![];
        opt_push(&mut args, "git", &self.git);
        opt_push(&mut args, "branch", &self.branch);
        opt_push(&mut args, "tag", &self.tag);
        opt_push(&mut args, "path", &self.path);
        opt_push(&mut args, "name", &self.name);
        bool_push(&mut args, "force", self.force);
        bool_push(&mut args, "verbose", self.verbose);
        bool_push(&mut args, "init", self.init);
        args
    }
}

fn bool_push(args: &mut Vec<String>, name: &str, set: bool) {
    if set {
        args.push(format!("--{name}"))
    }
}

fn opt_push(args: &mut Vec<String>, name: &str, arg: &Option<String>) {
    if let Some(arg) = arg {
        args.push(format!("--{name}"));
        args.push(arg.clone());
    }
}

fn cargo_generate_exe() -> Result<PathBuf> {
    // manually installed
    if let Ok(p) = which::which("cargo-generate") {
        return Ok(p);
    }

    // cargo-leptos installed
    let (target_os, target_arch) = os_arch()?;

    let binary = match target_os {
        "windows" => "cargo-generate.exe",
        _ => "cargo-generate",
    };

    let version = "0.17.3";
    let target = match (target_os, target_arch) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        _ => bail!("No cargo-generate tar binary found for {target_os} {target_arch}"),
    };
    let url = format!("https://github.com/cargo-generate/cargo-generate/releases/download/v{version}/cargo-generate-v{version}-{target}.tar.gz");

    let name = format!("cargo-generate-v{version}");

    match INSTALL_CACHE.download(true, &name, &[], &url) {
        Ok(None) => bail!("Unable to download cargo-generate for {target_os} {target_arch}"),
        Err(e) => {
            bail!("Unable to download cargo-generate for {target_os} {target_arch} due to: {e}")
        }
        Ok(Some(d)) => d
            .binary(binary)
            .map_err(|e| anyhow!("Could not find {binary} in downloaded cargo-generate: {e}")),
    }
}
