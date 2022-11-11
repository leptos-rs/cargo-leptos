use crate::{Error, Reportable};
use log::LevelFilter;
use simplelog::{ColorChoice, ConfigBuilder, TermLogger, TerminalMode};
use std::{fs, path::Path};

pub fn setup_logging(verbose: u8) {
    let log_level = match verbose {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    let config = ConfigBuilder::default()
        .set_time_level(LevelFilter::Off)
        .build();
    TermLogger::init(log_level, config, TerminalMode::Stderr, ColorChoice::Auto)
        .expect("Failed to start logger");
    log::info!("Log level set to: {log_level}");
}

pub fn rm_dir(dir: &str) -> Result<(), Reportable> {
    let path = Path::new(&dir);

    if !path.exists() {
        log::debug!("Not cleaning {dir} because it does not exist");
        return Ok(());
    }
    if !path.is_dir() {
        log::warn!("Not cleaning {dir} because it is not a directory");
        return Ok(());
    }

    log::info!("Cleaning dir '{dir}'");
    fs::remove_dir_all(path).map_err(|e| Into::<Error>::into(e).file_context("remove dir", dir))?;
    Ok(())
}

pub fn mkdirs(dir: String) -> Result<String, Reportable> {
    fs::create_dir_all(&dir)
        .map_err(|e| Into::<Error>::into(e).file_context("create dir", &dir))?;
    Ok(dir)
}

pub fn write(file: &str, text: &str) -> Result<(), Reportable> {
    log::trace!("Content of {file}:\n{text}");
    fs::write(&file, text).map_err(|e| Into::<Error>::into(e).file_context("write", file))
}

pub trait StrAdditions {
    fn with(&self, append: &str) -> String;
}

impl StrAdditions for str {
    fn with(&self, append: &str) -> String {
        let mut s = self.to_string();
        s.push_str(append);
        s
    }
}

impl StrAdditions for String {
    fn with(&self, append: &str) -> String {
        let mut s = self.clone();
        s.push_str(append);
        s
    }
}
