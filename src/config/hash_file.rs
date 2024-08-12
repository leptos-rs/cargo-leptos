use camino::Utf8PathBuf;

use super::bin_package::BinPackage;

pub struct HashFile {
    pub abs: Utf8PathBuf,
    pub rel: Utf8PathBuf,
}

impl HashFile {
    pub fn new(workspace_root: &Utf8PathBuf, bin: &BinPackage, rel: Option<&Utf8PathBuf>) -> Self {
        let rel = rel
            .cloned()
            .unwrap_or(Utf8PathBuf::from("hash.txt".to_string()));

        let exe_file_dir = bin.exe_file.parent().unwrap();
        let abs = workspace_root.join(exe_file_dir).join(&rel);

        Self { abs, rel }
    }
}
