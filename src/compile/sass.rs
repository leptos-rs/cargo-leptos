use crate::internal_prelude::*;
use crate::internal_prelude::*;
use crate::{
    ext::{
        sync::{wait_piped_interruptible, CommandResult, OutputExt},
        Paint,
    },
    logger::GRAY,
    signal::{Interrupt, Outcome},
};
use tokio::process::Command;

use crate::{ext::Exe, service::site::SourcedSiteFile};

pub async fn compile_sass(style_file: &SourcedSiteFile, optimise: bool) -> Result<Outcome<String>> {
    let mut args = vec![style_file.source.as_str()];
    optimise.then(|| args.push("--no-source-map"));

    let exe = Exe::Sass.get().await.dot()?;

    let mut cmd = Command::new(exe);
    cmd.args(&args);

    trace!(
        "Style running {}",
        GRAY.paint(format!("sass {}", args.join(" ")))
    );

    match wait_piped_interruptible("Dart Sass", cmd, Interrupt::subscribe_any()).await? {
        CommandResult::Success(output) => Ok(Outcome::Success(output.stdout())),
        CommandResult::Interrupted => Ok(Outcome::Stopped),
        CommandResult::Failure(output) => {
            warn!("Dart Sass failed with:");
            println!("{}", output.stderr());
            Ok(Outcome::Failed)
        }
    }
}
