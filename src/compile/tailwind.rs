use anyhow::Result;
use tokio::process::Command;

use crate::{
    config::{Project, TailwindConfig},
    ext::{
        anyhow::Context,
        fs,
        sync::{wait_piped_interruptible, CommandResult, OutputExt},
        Exe,
    },
    logger::GRAY,
    signal::{Interrupt, Outcome},
};

pub async fn compile_tailwind(
    _proj: &Project,
    tw_conf: &TailwindConfig,
) -> Result<Outcome<String>> {
    if !tw_conf.config_file.exists() {
        create_default_tailwind_config(tw_conf).await?;
    }

    let (line, process) = tailwind_process("tailwind", tw_conf).await?;

    match wait_piped_interruptible("Tailwind", process, Interrupt::subscribe_any()).await? {
        CommandResult::Success(output) => {
            let done = output
                .stderr()
                .lines()
                .last()
                .map(|l| l.contains("Done"))
                .unwrap_or(false);

            if done {
                log::info!("Tailwind finished {}", GRAY.paint(line));
                Ok(Outcome::Success(output.stdout()))
            } else {
                log::warn!("Tailwind failed {}", GRAY.paint(line));
                println!("{}\n{}", output.stdout(), output.stderr());
                Ok(Outcome::Failed)
            }
        }
        CommandResult::Interrupted => Ok(Outcome::Stopped),
        CommandResult::Failure(output) => {
            log::warn!("Tailwind failed");
            if output.has_stdout() {
                println!("{}", output.stdout());
            }
            println!("{}", output.stderr());
            Ok(Outcome::Failed)
        }
    }
}

async fn create_default_tailwind_config(tw_conf: &TailwindConfig) -> Result<()> {
    let contents = r##"/** @type {import('tailwindcss').Config} */
    module.exports = {
      content: { 
        files: ["*.html", "./src/**/*.rs"],
      },
      theme: {
        extend: {},
      },
      plugins: [],
    }
    "##;
    fs::write(&tw_conf.config_file, contents).await
}

pub async fn tailwind_process(cmd: &str, tw_conf: &TailwindConfig) -> Result<(String, Command)> {
    let tailwind = Exe::Tailwind.get().await.dot()?;

    let args: Vec<&str> = vec![
        "--input",
        tw_conf.input_file.as_str(),
        "--config",
        tw_conf.config_file.as_str(),
    ];
    let line = format!("{} {}", cmd, args.join(" "));
    let mut command = Command::new(tailwind);
    command.args(args);

    Ok((line, command))
}
