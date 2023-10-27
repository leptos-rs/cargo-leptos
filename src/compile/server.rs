use std::sync::Arc;

use super::ChangeSet;
use crate::{
    config::Project,
    ext::anyhow::{Context, Result},
    ext::sync::{wait_interruptible, CommandResult},
    logger::GRAY,
    signal::{Interrupt, Outcome, Product},
};
use tokio::{
    process::{Child, Command},
    task::JoinHandle,
};

pub async fn server(
    proj: &Arc<Project>,
    changes: &ChangeSet,
) -> JoinHandle<Result<Outcome<Product>>> {
    let proj = proj.clone();
    let changes = changes.clone();

    tokio::spawn(async move {
        if !changes.need_server_build() {
            return Ok(Outcome::Success(Product::None));
        }

        let (envs, line, process) = server_cargo_process("build", &proj)?;

        match wait_interruptible("Cargo", process, Interrupt::subscribe_any()).await? {
            CommandResult::Success(_) => {
                log::debug!("Cargo envs: {}", GRAY.paint(envs));
                log::info!("Cargo finished {}", GRAY.paint(line));

                let changed = proj
                    .site
                    .did_external_file_change(&proj.bin.exe_file)
                    .await
                    .dot()?;
                if changed {
                    log::debug!("Cargo server bin changed");
                    Ok(Outcome::Success(Product::Server))
                } else {
                    log::debug!("Cargo server bin unchanged");
                    Ok(Outcome::Success(Product::None))
                }
            }
            CommandResult::Interrupted => Ok(Outcome::Stopped),
            CommandResult::Failure(_) => Ok(Outcome::Failed),
        }
    })
}

pub fn server_cargo_process(cmd: &str, proj: &Project) -> Result<(String, String, Child)> {
    let mut command = Command::new(proj.bin.cargo_command.as_deref().unwrap_or("cargo"));
    let (envs, line) = build_cargo_server_cmd(cmd, proj, &mut command);
    Ok((envs, line, command.spawn()?))
}

pub fn build_cargo_server_cmd(
    cmd: &str,
    proj: &Project,
    command: &mut Command,
) -> (String, String) {
    let mut args = vec![
        cmd.to_string(),
        format!("--package={}", proj.bin.name.as_str()),
    ];
    if cmd != "test" {
        args.push(format!("--bin={}", proj.bin.target))
    }
    if let Some(target_dir) = &proj.bin.target_dir {
        args.push(format!("--target-dir={target_dir}"));
    }
    if let Some(triple) = &proj.bin.target_triple {
        args.push(format!("--target={triple}"));
    }

    if !proj.bin.default_features {
        args.push("--no-default-features".to_string());
    }

    if !proj.bin.features.is_empty() {
        args.push(format!("--features={}", proj.bin.features.join(",")));
    }

    log::debug!("BIN CARGO ARGS: {:?}", &proj.bin.cargo_args);
    // Add cargo flags to cargo command
    if let Some(cargo_args) = &proj.bin.cargo_args {
        if !cargo_args.is_empty() {
            args.push(format!("{}", cargo_args.join(" ")))
        }
    }
    proj.bin.profile.add_to_args(&mut args);

    let envs = proj.to_envs();

    let envs_str = envs
        .iter()
        .map(|(name, val)| format!("{name}={val}"))
        .collect::<Vec<_>>()
        .join(" ");

    command.args(&args).envs(envs);
    let line = format!("cargo {}", args.join(" "));
    (envs_str, line)
}
