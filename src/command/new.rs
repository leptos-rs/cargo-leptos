use cargo_generate::{generate, GenerateArgs, TemplatePath};
use clap::{ArgGroup, Args};

use crate::internal_prelude::*;

// A subset of the cargo-generate commands available.
// See: https://github.com/cargo-generate/cargo-generate/blob/main/src/args.rs

#[derive(Clone, Debug, Args, PartialEq, Eq)]
#[clap(arg_required_else_help(true))]
#[clap(group(ArgGroup::new("template").args(&["git", "path"]).required(true).multiple(false)))]
#[clap(about)]
pub struct NewCommand {
    /// Git repository to clone template from. Can be a full URL (like
    /// `https://github.com/leptos-rs/start-actix`), or a shortcut for one of our
    /// built-in templates: `leptos-rs/start-trunk`, `leptos-rs/start-actix`, `leptos-rs/start-axum`,
    /// `leptos-rs/start-axum-workspace`, `leptos-rs/start-aws` or `leptos-rs/start-spin`.
    #[clap(short, long, group = "git-arg")]
    pub git: Option<String>,

    /// Branch to use when installing from git
    #[clap(short, long, conflicts_with = "tag", requires = "git-arg")]
    pub branch: Option<String>,

    /// Tag to use when installing from git
    #[clap(short, long, conflicts_with = "branch", requires = "git-arg")]
    pub tag: Option<String>,

    /// Local path to copy the template from. Can not be specified together with --git.
    #[clap(short, long)]
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
    pub fn run(self) -> Result<()> {
        let Self {
            git,
            branch,
            tag,
            path,
            name,
            force,
            verbose,
            init,
        } = self;
        let args = GenerateArgs {
            template_path: TemplatePath {
                git: absolute_git_url(git),
                branch,
                tag,
                path,
                ..Default::default()
            },
            name,
            force,
            verbose,
            init,
            ..Default::default()
        };

        generate(args).dot_anyhow()?;

        Ok(())
    }
}

/// Workaround to support short `new --git leptos-rs/start` command when behind Git proxy.
/// See https://github.com/cargo-generate/cargo-generate/issues/752.
fn absolute_git_url(url: Option<String>) -> Option<String> {
    url.map(|url| match url.as_str() {
        "start-trunk" | "leptos-rs/start-trunk" => format_leptos_starter_url("start-trunk"),
        "start-actix" | "leptos-rs/start" | "leptos-rs/start-actix" => {
            format_leptos_starter_url("start-actix")
        }
        "start-axum" | "leptos-rs/start-axum" => format_leptos_starter_url("start-axum"),
        "start-axum-workspace" | "leptos-rs/start-axum-workspace" => {
            format_leptos_starter_url("start-axum-workspace")
        }
        "start-aws" | "leptos-rs/start-aws" => format_leptos_starter_url("start-aws"),
        "start-spin" | "leptos-rs/start-spin" => format_leptos_starter_url("start-spin"),
        _ => url,
    })
}

fn format_leptos_starter_url(repo: &str) -> String {
    format!("https://github.com/leptos-rs/{repo}")
}
