use anyhow::Result;
use tokio::process::Command;

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
    if !tw_conf.config_file.exists() {
        create_default_tailwind_config(tw_conf).await?;
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
                log::info!("Tailwind finished {}", GRAY.paint(line));
                match fs::read_to_string(&tw_conf.tmp_file).await {
                    Ok(content) => Ok(Outcome::Success(content)),
                    Err(e) => {
                        log::error!("Failed to read tailwind result: {e}");
                        Ok(Outcome::Failed)
                    }
                }
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
    fs::write(&tw_conf.config_file, contents).await
}

pub async fn tailwind_process(
    proj: &Project,
    cmd: &str,
    tw_conf: &TailwindConfig,
) -> Result<(String, Command)> {
    let tailwind = Exe::Tailwind.get().await.dot()?;

    let mut args: Vec<&str> = vec![
        "--input",
        tw_conf.input_file.as_str(),
        "--config",
        tw_conf.config_file.as_str(),
        "--output",
        tw_conf.tmp_file.as_str(),
    ];

    if proj.release {
        // minify & optimize
        args.push("--minify");
    }

    let line = format!("{} {}", cmd, args.join(" "));
    let mut command = Command::new(tailwind);
    command.args(args);

    Ok((line, command))
}
