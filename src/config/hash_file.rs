use camino::Utf8PathBuf;

pub struct HashFile {
    pub abs: Utf8PathBuf,
    pub rel: Utf8PathBuf,
}

impl HashFile {
    pub fn new(workspace_root: &Utf8PathBuf, rel: Option<&Utf8PathBuf>) -> Self {
        let rel = rel
            .cloned()
            .unwrap_or(Utf8PathBuf::from("hash.txt".to_string()));

        let abs = workspace_root.join(&rel);

        Self { abs, rel }
    }
}
