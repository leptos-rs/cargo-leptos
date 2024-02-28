use crate::config::Profile;
use camino::Utf8PathBuf;

pub struct HashFile {
    pub abs: Utf8PathBuf,
    pub rel: Utf8PathBuf,
}

impl HashFile {
    pub fn new(
        target_directory: &Utf8PathBuf,
        profile: &Profile,
        rel: Option<&Utf8PathBuf>,
    ) -> Self {
        let rel = rel
            .cloned()
            .unwrap_or(Utf8PathBuf::from("hash.txt".to_string()));

        let abs = target_directory.join(profile.to_string()).join(&rel);

        Self { abs, rel }
    }
}
