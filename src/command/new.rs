use crate::ext::anyhow::{Context, Result};
use clap::Args;
use tokio::process::Command;
use crate::ext::exe::Exe;
use serde::{Serialize, Deserialize};

// A subset of the cargo-generate commands available.
// See: https://github.com/cargo-generate/cargo-generate/blob/main/src/args.rs

#[derive(Clone, Debug, Args, PartialEq, Eq, Serialize, Deserialize)]
#[clap(arg_required_else_help(true))]
#[clap(about)]
pub struct NewCommand {
    /// Git repository to clone template from. Can be a full URL (like
    /// `https://github.com/leptos-rs/start`), or a shortcut for one of our
    /// built-in templates: `leptos-rs/start`, `leptos-rs/start-axum`,
    /// `leptos-rs/start-axum-workspace`, or `leptos-rs/start-aws`.
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
        let exe = Exe::CargoGenerate.get().await.dot()?;

        let mut process = Command::new(exe)
            .arg("generate")
            .args(&args)
            .spawn()
            .context("Could not spawn cargo-generate command (verify that it is installed)")?;
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
