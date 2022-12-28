#[cfg(all(test, feature = "full_tests"))]
mod tests;

pub mod anyhow;
mod cargo;
pub mod exe;
pub mod fs;
mod path;
pub mod sync;
mod util;

pub use cargo::{MetadataExt, PackageExt};
pub use exe::{Exe, ExeMeta};
pub use path::{remove_nested, PathBufExt, PathExt};
pub use util::{os_arch, StrAdditions};
