#[cfg(test)]
mod tests;

mod assets;
mod change;
mod front;
mod hash;
mod lightningcss;
mod sass;
mod server;
mod style;
mod tailwind;

pub use assets::assets;
pub use change::{Change, ChangeSet};
pub use front::{front, front_cargo_process, front_cargo_process_with_args};
pub use hash::add_hashes_to_site;
pub use server::{server, server_cargo_process, server_cargo_process_with_args};
pub use style::style;

use itertools::Itertools;
use tokio::process::Command;

fn build_cargo_command_string(command: &Command) -> String {
    let std_command = command.as_std();
    let program = std_command.get_program();
    let args = std_command.get_args();

    [program]
        .into_iter()
        .chain(args)
        .map(|arg| match arg.to_string_lossy() {
            arg if arg.contains(' ') => format!("'{arg}'"),
            arg => arg.into_owned(),
        })
        .join(" ")
}
