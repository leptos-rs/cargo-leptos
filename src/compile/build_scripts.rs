use crate::config::Project;
use crate::ext::sync::{wait_interruptible, CommandResult};
use crate::internal_prelude::*;
use crate::logger::GRAY;
use crate::signal::{Interrupt, Outcome, Product};
use std::sync::Arc;
use tokio::process::Command;
use tokio::task::JoinHandle;

pub async fn run_build_scripts(proj: &Arc<Project>) -> JoinHandle<Result<Outcome<Product>>> {
    let proj = proj.clone();

    tokio::spawn(async move { handle_commands_sequentially(&proj).await })
}

async fn handle_commands_sequentially(proj: &Arc<Project>) -> Result<Outcome<Product>> {
    let len = proj.build_scripts.len();
    for (i, (mut command, command_str)) in strings_to_commands(proj.build_scripts.clone())
        .into_iter()
        .enumerate()
    {
        info!(
            "Running build script {i} / {len}: {}",
            GRAY.paint(&command_str)
        );
        let child = command
            .spawn()
            .wrap_err(format!("Failed spawning command {command_str}"))?;

        match wait_interruptible("build script", child, Interrupt::subscribe_any()).await? {
            CommandResult::Interrupted => return Ok(Outcome::Stopped),
            CommandResult::Failure(_) => return Ok(Outcome::Failed),
            CommandResult::Success(_) => {
                debug!("Finished build script {i} / {len}");
            }
        };
    }
    Ok(Outcome::Success(Product::BuildScripts))
}

fn strings_to_commands(build_scripts: Vec<String>) -> Vec<(Command, String)> {
    build_scripts
        .clone()
        .into_iter()
        .map(|command_str| {
            //TODO: what happens if the whole command_str is ""?
            //TODO: What to do if we're running this on BlackBerry, or god forbid Samsung smartfridge?!?
            let command = if cfg!(target_family = "windows") {
                let mut c = Command::new("cmd");
                c.args(["/C", &command_str, "&& exit"]);
                c
            } else {
                // only other target_family option is UNIX
                let mut c = Command::new("sh");
                c.args(["-c", &command_str, "&& exit 1"]);
                c
            };

            (command, command_str)
        })
        .collect()
}
