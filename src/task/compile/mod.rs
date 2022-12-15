mod assets;
mod front;
mod results;
mod server;
mod style;

pub use results::{Outcome, Product, ProductSet};

pub use assets::assets;
pub use front::{front, front_cargo_process};
pub use server::{server, server_cargo_process};
pub use style::style;
