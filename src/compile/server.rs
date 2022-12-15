use super::ChangeSet;
use crate::{
    config::Config,
    ext::anyhow::{Context, Result},
    ext::sync::wait_interruptible,
    logger::GRAY,
    service::site,
    signal::{Interrupt, Outcome, Product},
};
use tokio::{
    process::{Child, Command},
    task::JoinHandle,
};

pub async fn server(conf: &Config, changes: &ChangeSet) -> JoinHandle<Result<Outcome>> {
    let conf = conf.clone();
    let changes = changes.clone();

    tokio::spawn(async move {
        if !changes.need_server_build() {
            return Ok(Outcome::Success(Product::NoChange));
        }

        let (line, process) = server_cargo_process("build", &conf)?;

        match wait_interruptible("Cargo", process, Interrupt::subscribe_any()).await? {
            true => {
                log::info!("Cargo finished {}", GRAY.paint(line));

                if site::ext::did_file_change(&conf.cargo_bin_file())
                    .await
                    .dot()?
                {
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

pub fn server_cargo_process(cmd: &str, conf: &Config) -> Result<(String, Child)> {
    let mut args = vec![
        cmd,
        "--no-default-features",
        "--features=ssr",
        "--target-dir=target/server",
    ];

    if conf.cli.release {
        args.push("--release");
    }

    let envs = conf.to_envs();

    let child = Command::new("cargo").args(&args).envs(envs).spawn()?;
    let line = format!("cargo {}", args.join(" "));
    Ok((line, child))
}
