
use camino::Utf8PathBuf;

use super::bin_package::BinPackage;
use super::lib_package::LibPackage;
use crate::internal_prelude::*;

pub struct HashFile {
    pub abs: Utf8PathBuf,
    pub rel: Utf8PathBuf,
}

pub enum HashablePackage<'a> {
    Bin(&'a BinPackage),
    Lib(&'a LibPackage)
}

impl <'a> HashablePackage<'a> {
    pub fn new(bin: Option<&'a BinPackage>, lib: Option<&'a LibPackage>) -> Self {
        match (bin, lib) {
            (Some(b), _) => Self::Bin(b),
            (None, Some(l)) => Self::Lib(l),
            _ => panic!("No bin or lib, nothing to see...")
        }
    }
    fn file(&self) -> &Utf8PathBuf {
        match self {
            Self::Bin(b) => &b.exe_file,
            Self::Lib(l) => &l.wasm_file.site
        }
    }
    fn dir(&self) -> &Utf8PathBuf {
        match self {
            Self::Bin(b) => &b.abs_dir,
            Self::Lib(l) => &l.abs_dir
        }
    }
}

impl HashFile {
    pub fn new(
        workspace_root: Option<&Utf8PathBuf>,
        pkg: HashablePackage,
        rel: Option<&Utf8PathBuf>,
    ) -> Self {
        let rel = rel
            .cloned()
            .unwrap_or(Utf8PathBuf::from("hash.txt".to_string()));

        let exe_file_dir = pkg.file().parent().unwrap();
        let abs;
        if let Some(workspace_root) = workspace_root {
            debug!("PKG PARENT: {}", pkg.file().parent().unwrap());
            abs = workspace_root.join(exe_file_dir).join(&rel);
        } else {
            abs = pkg.dir().join(exe_file_dir).join(&rel);
        }
        Self { abs, rel }
    }
}
