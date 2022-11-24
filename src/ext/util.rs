use super::fs;
use crate::Msg;
use anyhow_ext::{bail, Context, Result};
use cargo_metadata::{Artifact, Message};
use serde::Deserialize;
use std::{borrow::Cow, path::PathBuf, process::Stdio};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::broadcast::Sender,
    task::JoinHandle,
};

pub fn os_arch() -> Result<(&'static str, &'static str)> {
    let target_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        bail!("unsupported OS")
    };

    let target_arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        bail!("unsupported target architecture")
    };
    Ok((target_os, target_arch))
}

pub trait StrAdditions {
    fn with(&self, append: &str) -> String;
    fn pad_left_to<'a>(&'a self, len: usize) -> Cow<'a, str>;
    /// returns the string as a canonical path (creates the dir if necessary)
    fn to_canoncial_dir(&self) -> Result<PathBuf>;
}

impl StrAdditions for str {
    fn with(&self, append: &str) -> String {
        let mut s = self.to_string();
        s.push_str(append);
        s
    }

    fn pad_left_to<'a>(&'a self, len: usize) -> Cow<'a, str> {
        let chars = self.chars().count();
        if chars < len {
            Cow::Owned(format!("{}{self}", " ".repeat(len - chars)))
        } else {
            Cow::Borrowed(self)
        }
    }

    fn to_canoncial_dir(&self) -> Result<PathBuf> {
        let path = PathBuf::from(self);
        if !path.exists() {
            fs::create_dir_all(&path).context(format!("Could not create dir {self:?}"))?;
        }
        let path = path
            .canonicalize()
            .context(format!("Could not canonicalize {path:?}"))?;
        Ok(path)
    }
}

impl StrAdditions for String {
    fn with(&self, append: &str) -> String {
        let mut s = self.clone();
        s.push_str(append);
        s
    }

    fn pad_left_to<'a>(&'a self, len: usize) -> Cow<'a, str> {
        self.as_str().pad_left_to(len)
    }

    fn to_canoncial_dir(&self) -> Result<PathBuf> {
        self.as_str().to_canoncial_dir()
    }
}

pub trait SenderAdditions {
    fn send_logged(&self, me: &str, msg: Msg);
}

impl SenderAdditions for Sender<Msg> {
    fn send_logged(&self, me: &str, msg: Msg) {
        if let Err(e) = self.send(msg) {
            log::error!("{me} {e}");
        }
    }
}

pub trait CommandAdditions {
    /// Sets up the command so that stdout is redirected and parsed by cargo_metadata.
    /// It returns a handle and a child process. Waiting on the handle returns
    /// a vector of cargo_metadata Artifacts.
    fn spawn_cargo_parsed(&mut self) -> Result<(JoinHandle<Vec<Artifact>>, Child)>;
}

impl CommandAdditions for Command {
    fn spawn_cargo_parsed(&mut self) -> Result<(JoinHandle<Vec<Artifact>>, Child)> {
        let mut process = self
            .stdout(Stdio::piped())
            .arg("--message-format=json-render-diagnostics")
            .spawn()?;

        let mut stdout = BufReader::new(process.stdout.take().unwrap());

        let handle = tokio::spawn(async move {
            let mut line = String::new();
            let mut artifacts: Vec<Artifact> = Vec::new();
            loop {
                match stdout.read_line(&mut line).await {
                    Ok(_) => {
                        let mut deserializer = serde_json::Deserializer::from_str(&line);
                        deserializer.disable_recursion_limit();
                        match Message::deserialize(&mut deserializer) {
                            Ok(Message::BuildFinished(v)) => {
                                if !v.success {
                                    log::warn!("Cargo build failed")
                                }
                                break;
                            }
                            Ok(Message::BuildScriptExecuted(_script)) => {}
                            Ok(Message::CompilerArtifact(art)) => artifacts.push(art),
                            Ok(Message::CompilerMessage(msg)) => log::info!("MESSAGE {msg:?}"),
                            Ok(Message::TextLine(txt)) => log::info!("TEXT {txt:?}"),
                            Err(e) => {
                                log::error!("Cargo stdout: {e}");
                                break;
                            }
                            Ok(_) => log::info!("UNPARSEABLE: {line}"),
                        };
                        line.clear();
                    }
                    Err(e) => {
                        log::error!("Cargo stdout: {e}");
                        break;
                    }
                }
            }
            artifacts
        });
        Ok((handle, process))
    }
}
