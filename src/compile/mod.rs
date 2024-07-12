// #[cfg(test)]
// mod tests;

// mod assets;
// mod change;
// mod front;
// mod hash;
// mod sass;
// mod server;
// mod style;
// mod tailwind;

// pub use assets::assets;
// pub use change::{Change, ChangeSet};
// pub use front::{front, front_cargo_process};
// pub use hash::add_hashes_to_site;
// pub use server::{server, server_cargo_process};
// pub use style::style;

// use itertools::Itertools;

// fn build_cargo_command_string(args: impl IntoIterator<Item = String>) -> String {
//     std::iter::once("cargo".to_owned())
//         .chain(args.into_iter().map(|arg| {
//             if arg.contains(' ') {
//                 format!("'{arg}'")
//             } else {
//                 arg
//             }
//         }))
//         .join(" ")
// }
