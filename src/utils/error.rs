//! patchkc's error type.
//!
//! Every fallible operation in the library surfaces a [`PatchError`] so the
//! CLI front-end (see `main.rs`) has a single place to render failures and
//! pick an exit code, instead of `.expect()`/`panic!`-ing deep inside the
//! tool the way an early prototype might.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PatchError {
    #[error("this operation requires root privileges (try sudo)")]
    NotRoot,

    #[error("io error on `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("no backup found for `{0}`")]
    BackupNotFound(PathBuf),

    #[error("module error: {0}")]
    Module(String),

    #[error("command `{cmd}` failed: {detail}")]
    Command { cmd: String, detail: String },

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, PatchError>;

impl PatchError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        PatchError::Io {
            path: path.into(),
            source,
        }
    }
}
