use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Reportable {
    #[error("Could not {action} on {file} because of {source}")]
    FileError {
        action: &'static str,
        file: String,
        source: Error,
    },
    #[error("Could not {action} because of {source}")]
    StepError { action: &'static str, source: Error },
}

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    TomlError(#[from] toml::de::Error),
    #[error(transparent)]
    ShellError(#[from] xshell::Error),
}

impl Error {
    pub fn file_context<P: AsRef<Path>>(self, action: &'static str, file: P) -> Reportable {
        let file = file.as_ref().to_string_lossy().to_string();
        Reportable::FileError {
            action,
            file,
            source: self,
        }
    }

    pub fn step_context(self, action: &'static str) -> Reportable {
        Reportable::StepError {
            action,
            source: self,
        }
    }
}
