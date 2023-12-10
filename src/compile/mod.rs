#[cfg(test)]
mod tests;

mod assets;
mod change;
mod front;
mod sass;
mod server;
mod style;
mod tailwind;

pub use assets::assets;
pub use change::{Change, ChangeSet};
pub use front::{front, front_cargo_process};
pub use server::{server, server_cargo_process};
pub use style::style;

use itertools::Itertools;

fn build_cargo_command_string(args: Vec<String>) -> String {
    std::iter::once("cargo".to_owned())
        .chain(args.into_iter().map(|arg| {
            if arg.contains(' ') {
                format!("'{arg}'")
            } else {
                arg
            }
        }))
        .join(" ")
}
