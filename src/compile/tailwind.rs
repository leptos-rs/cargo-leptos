use anyhow::Result;
use std::sync::Arc;
use tokio::process::{Child, Command};

use crate::{
    config::{Project, TailwindConfig},
    ext::{
        anyhow::Context,
        fs,
        sync::{wait_interruptible, CommandResult},
        Exe,
    },
    logger::GRAY,
    signal::{Interrupt, Outcome, Product},
};

use super::ChangeSet;

pub async fn tailwind(proj: &Arc<Project>, changes: &ChangeSet) -> Result<Outcome> {
    let tw_conf = match (changes.need_front_build(), &proj.lib.tailwind) {
        (true, Some(tw_conf)) => tw_conf,
        (_, _) => return Ok(Outcome::Success(Product::None)),
    };

    if !tw_conf.config_file.exists() {
        create_default_tailwind_config(tw_conf).await?;
    }

    let (line, process) = tailwind_process("build", tw_conf).await?;

    match wait_interruptible("Tailwind", process, Interrupt::subscribe_any()).await? {
        CommandResult::Success => {
            log::info!("Tailwind finished {}", GRAY.paint(line));

            // TODO: should check for the tailwind output file
            let changed = proj
                .site
                .did_external_file_change(&proj.bin.exe_file)
                .await
                .dot()?;
            if changed {
                log::debug!("Tailwind style changed");
                Ok(Outcome::Success(Product::Style))
            } else {
                log::debug!("Tailwind style unchanged");
                Ok(Outcome::Success(Product::None))
            }
        }
        CommandResult::Interrupted => Ok(Outcome::Stopped),
        CommandResult::Failure => Ok(Outcome::Failed),
    }
}

async fn create_default_tailwind_config(tw_conf: &TailwindConfig) -> Result<()> {
    let contents = r##"content: { 
        files: ["./src/**/*.rs"],
      }"##;
    fs::write(&tw_conf.config_file, contents).await
}

pub async fn tailwind_process(cmd: &str, tw_conf: &TailwindConfig) -> Result<(String, Child)> {
    let tailwind = Exe::Tailwind.get().await.dot()?;

    let args: Vec<&str> = vec![
        "-input",
        tw_conf.input_file.as_str(),
        "-output",
        tw_conf.output_file.as_str(),
        "-config",
        tw_conf.config_file.as_str(),
    ];
    let line = format!("{} {}", cmd, args.join(" "));
    let child = Command::new(tailwind).args(args).spawn()?;

    Ok((line, child))
}
