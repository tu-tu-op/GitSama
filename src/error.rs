use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),

    #[error("could not read {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not write {path}: {source}")]
    WriteFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not run Git: {0}")]
    Git(String),

    #[error("invalid configuration: {0}")]
    Config(String),

    #[error("invalid pack: {0}")]
    Pack(String),

    #[error("audio playback failed: {0}")]
    Audio(String),

    #[error("GitSama requires Git 2.54 or newer (detected {0})")]
    UnsupportedGit(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn message(value: impl Into<String>) -> Self {
        Self::Message(value.into())
    }
}

