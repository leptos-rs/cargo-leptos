use lightningcss::error::Error as CssError;
use lightningcss::error::MinifyErrorKind as CssMinifyError;
use lightningcss::error::ParserError as CssParserError;
use lightningcss::error::PrinterErrorKind as CssPrinterError;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Reportable {
    #[error("Could not {action} on {file} because of: {source}")]
    FileError {
        action: &'static str,
        file: String,
        source: Error,
    },
    #[error("Could not {action} because of: {source}")]
    StepError { action: String, source: Error },
    #[error("Invalid configuration entry {field} because of: {source}")]
    ConfigError { field: &'static str, source: Error },
    #[error("{expectation} not '{file}'")]
    NotAFileError { expectation: String, file: String },
}

impl Reportable {
    pub fn not_a_file<S: ToString, P: AsRef<Path>>(expectation: S, file: P) -> Self {
        Self::NotAFileError {
            expectation: expectation.to_string(),
            file: file.as_ref().to_string_lossy().to_string(),
        }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    TomlError(#[from] toml::de::Error),
    #[error(transparent)]
    ShellError(#[from] xshell::Error),
    #[error("{0}")]
    CSSParseError(String),
    #[error(transparent)]
    CSSMinifyError(#[from] CssError<CssMinifyError>),
    #[error(transparent)]
    CSSPrintError(#[from] CssError<CssPrinterError>),
    #[error("{0}")]
    BrowserListError(String),
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

    pub fn step_context<S: ToString>(self, action: S) -> Reportable {
        Reportable::StepError {
            action: action.to_string(),
            source: self,
        }
    }

    pub fn config_context(self, field: &'static str) -> Reportable {
        Reportable::ConfigError {
            field,
            source: self,
        }
    }
}

impl<'a> From<CssError<CssParserError<'a>>> for Error {
    fn from(e: CssError<CssParserError<'a>>) -> Self {
        Error::CSSParseError(e.to_string())
    }
}
