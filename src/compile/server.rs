use std::sync::Arc;

use super::ChangeSet;
use crate::{
    config::Project,
    ext::anyhow::{Context, Result},
    ext::sync::wait_interruptible,
    logger::GRAY,
    signal::{Interrupt, Outcome, Product},
};
use tokio::{
    process::{Child, Command},
    task::JoinHandle,
};

pub async fn server(proj: &Arc<Project>, changes: &ChangeSet) -> JoinHandle<Result<Outcome>> {
    let proj = proj.clone();
    let changes = changes.clone();

    tokio::spawn(async move {
        if !changes.need_server_build() {
            return Ok(Outcome::Success(Product::NoChange));
        }

        let (line, process) = server_cargo_process("build", &proj)?;

        match wait_interruptible("Cargo", process, Interrupt::subscribe_any()).await? {
            true => {
                log::info!("Cargo finished {}", GRAY.paint(line));

                let changed = proj
                    .site
                    .did_external_file_change(&proj.paths.cargo_bin_file)
                    .await
                    .dot()?;
                if changed {
                    log::debug!("Cargo server bin changed");
                    Ok(Outcome::Success(Product::ServerBin))
                } else {
                    log::debug!("Cargo server bin unchanged");
                    Ok(Outcome::Success(Product::NoChange))
                }
            }
            false => Ok(Outcome::Stopped),
        }
    })
}

pub fn server_cargo_process(cmd: &str, proj: &Project) -> Result<(String, Child)> {
    let profile = format!("--profile={}", proj.server_profile);
    let args = vec![
        cmd,
        "--no-default-features",
        "--features=ssr",
        "--target-dir=target/server",
        profile.as_str(),
    ];

    let envs = proj.to_envs();

    let child = Command::new("cargo").args(&args).envs(envs).spawn()?;
    let line = format!("cargo {}", args.join(" "));
    Ok((line, child))
}
