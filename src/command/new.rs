use cargo_generate::{GenerateArgs, TemplatePath};
use clap::Args;
use color_eyre::{eyre::eyre, Result};
use serde::{Deserialize, Serialize};

// A subset of the cargo-generate commands available.
// See: https://github.com/cargo-generate/cargo-generate/blob/main/src/args.rs

#[derive(Clone, Debug, Args, PartialEq, Eq, Serialize, Deserialize)]
#[clap(arg_required_else_help(true))]
#[clap(about)]
pub struct NewCommand {
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

    /// Local path to copy the template from. Can not be specified together with --template.
    #[clap(short, long, group("SpecificPath"))]
    pub path: Option<String>,

    /// Template path
    #[clap(short, long)]
    pub template: Option<String>,

    /// Branch to use when installing from template
    #[clap(long, conflicts_with = "tag")]
    pub branch: Option<String>,

    /// Tag to use when installing from template
    #[clap(long, conflicts_with = "branch")]
    pub tag: Option<String>,

    /// Specifies the sub-template within the template repository to be used as the actual template.
    #[arg(long)]
    pub subtemplate: Option<String>,
}

impl NewCommand {
    pub fn run(self) -> Result<()> {
        let args = GenerateArgs {
            name: self.name,
            force: self.force,
            verbose: self.verbose,
            init: self.init,
            template_path: TemplatePath {
                auto_path: self.template,
                branch: self.branch,
                tag: self.tag,
                path: self.path,
                subfolder: self.subtemplate,
                ..Default::default()
            },
            ..Default::default()
        };
        let _ = cargo_generate::generate(args).map_err(|err| eyre!(Box::new(err)))?;
        Ok(())
    }
}
