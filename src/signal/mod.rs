mod interrupt;
mod product;
mod reload;

pub use interrupt::Interrupt;
pub use product::{Outcome, Product, ProductSet, ServerRestart};
pub use reload::{ReloadSignal, ReloadType};

#[macro_export]
macro_rules! location {
    () => {
        $crate::command::Location {
            file: file!().to_string(),
            line: line!(),
            column: column!(),
        }
    };
}

pub struct Location {
    pub file: &'static str,
    pub line: u32,
    pub column: u32,
    pub modules: &'static str,
}
