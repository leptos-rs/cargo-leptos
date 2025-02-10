use camino::Utf8PathBuf;

use super::bin_package::BinPackage;
use crate::internal_prelude::*;

pub struct HashFile {
    pub abs: Utf8PathBuf,
    pub rel: Utf8PathBuf,
}

impl HashFile {
    pub fn new(
        workspace_root: Option<&Utf8PathBuf>,
        bin: &BinPackage,
        rel: Option<&Utf8PathBuf>,
    ) -> Self {
        let rel = rel
            .cloned()
            .unwrap_or(Utf8PathBuf::from("hash.txt".to_string()));

        let exe_file_dir = bin.exe_file.parent().unwrap();
        let abs;
        if let Some(workspace_root) = workspace_root {
            debug!("BIN PARENT: {}", bin.exe_file.parent().unwrap());
            abs = workspace_root.join(exe_file_dir).join(&rel);
        } else {
            abs = bin.abs_dir.join(exe_file_dir).join(&rel);
        }
        Self { abs, rel }
    }
}
