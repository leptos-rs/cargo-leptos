use anyhow::Result;
use camino::Utf8Path;
use tokio::process::Command;

use crate::internal_prelude::*;
use crate::{
    config::{Project, TailwindConfig},
    ext::{
        anyhow::Context,
        fs,
        sync::{wait_piped_interruptible, CommandResult, OutputExt},
        Exe, Paint,
    },
    logger::GRAY,
    signal::{Interrupt, Outcome},
};

pub async fn compile_tailwind(proj: &Project, tw_conf: &TailwindConfig) -> Result<Outcome<String>> {
    if let Some(config_file) = tw_conf.config_file.as_ref() {
        if !config_file.exists() {
            create_default_tailwind_config(config_file).await?
        }
    }

    let (line, process) = tailwind_process(proj, "tailwindcss", tw_conf).await?;

    match wait_piped_interruptible("Tailwind", process, Interrupt::subscribe_any()).await? {
        CommandResult::Success(output) => {
            let done = output
                .stderr()
                .lines()
                .last()
                .map(|l| l.contains("Done"))
                .unwrap_or(false);

            if done {
                info!("Tailwind finished {}", GRAY.paint(line));
                match fs::read_to_string(&tw_conf.tmp_file).await {
                    Ok(content) => Ok(Outcome::Success(content)),
                    Err(e) => {
                        error!("Failed to read tailwind result: {e}");
                        Ok(Outcome::Failed)
                    }
                }
            } else {
                warn!("Tailwind failed {}", GRAY.paint(line));
                println!("{}\n{}", output.stdout(), output.stderr());
                Ok(Outcome::Failed)
            }
        }
        CommandResult::Interrupted => Ok(Outcome::Stopped),
        CommandResult::Failure(output) => {
            warn!("Tailwind failed");
            if output.has_stdout() {
                println!("{}", output.stdout());
            }
            println!("{}", output.stderr());
            Ok(Outcome::Failed)
        }
    }
}

async fn create_default_tailwind_config(config_file: &Utf8Path) -> Result<()> {
    let contents = r#"/** @type {import('tailwindcss').Config} */
    module.exports = {
      content: {
        relative: true,
        files: ["*.html", "./src/**/*.rs"],
      },
      theme: {
        extend: {},
      },
      plugins: [],
    }
    "#;
    fs::write(config_file, contents).await
}

pub async fn tailwind_process(
    proj: &Project,
    cmd: &str,
    tw_conf: &TailwindConfig,
) -> Result<(String, Command)> {
    let tailwind = Exe::Tailwind.get().await.dot()?;

    let mut args = vec!["--input", tw_conf.input_file.as_str()];

    if let Some(config_file) = tw_conf.config_file.as_ref() {
        args.push("--config");
        args.push(config_file.as_str());
    }

    args.push("--output");
    args.push(tw_conf.tmp_file.as_str());

    if proj.release {
        // minify & optimize
        args.push("--minify");
    }

    let line = format!("{} {}", cmd, args.join(" "));
    let mut command = Command::new(tailwind);
    command.args(args);

    Ok((line, command))
}
