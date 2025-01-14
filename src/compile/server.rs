use std::sync::Arc;
use super::ChangeSet;
use crate::{
    config::Project,
    ext::anyhow::{Context, Result},
    ext::sync::{wait_interruptible, CommandResult},
    logger::GRAY,
    signal::{Interrupt, Outcome, Product},
};
use shlex::Shlex;
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
        log::debug!("CARGO SERVER COMMAND: {:?}", process);
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
    let raw_command = proj.bin.cargo_command.as_deref().unwrap_or("cargo");
    let mut command_iter = Shlex::new(raw_command);

    if command_iter.had_error {
        panic!("bin-cargo-command cannot contain escaped quotes. Not sure why you'd want to")
    }

    let cargo_command = command_iter
        .next()
        .expect("Failed to get bin command. This should default to cargo");
    let mut command: Command = Command::new(cargo_command);

    let args: Vec<String> = command_iter.collect();
    command.args(args);

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

    // If we're building the bin target for wasm, we want it to be a lib so it
    // can be run by wasmtime or spin or wasmer or whatever
    let server_is_wasm = match &proj.bin.target_triple {
        Some(t) => t.contains("wasm"),
        None => false,
    };
    if cmd != "test" && !server_is_wasm {
        args.push(format!("--bin={}", proj.bin.target))
    } else if cmd != "test" && server_is_wasm {
        args.push("--lib".to_string())
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
    // Check if the binary executable file exists
    let exe_file = &proj.bin.exe_file;
    if exe_file.exists() {
        // If it exists, use its parent directory as the target directory
        if let Some(target_dir) = exe_file.parent() {
            command.env("CARGO_TARGET_DIR", target_dir);
        }
    }

    log::debug!("BIN CARGO ARGS: {:?}", &proj.bin.cargo_args);
    // Add cargo flags to cargo command
    if let Some(cargo_args) = &proj.bin.cargo_args {
        args.extend_from_slice(cargo_args);
    }
    proj.bin.profile.add_to_args(&mut args);

    let envs = proj.to_envs();

    let envs_str = envs
        .iter()
        .map(|(name, val)| format!("{name}={val}"))
        .collect::<Vec<_>>()
        .join(" ");

    command.args(&args).envs(envs);
    let line = super::build_cargo_command_string(args);
    (envs_str, line)
}
