//! Error types for engram-vfs

use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VfsError {
    #[error("Archive error: {0}")]
    Archive(String),

    #[error("Database not found in archive: {0}")]
    DatabaseNotFound(String),

    #[error("SQLite error: {0}")]
    Sqlite(String),

    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    #[error("VFS registration error: {0}")]
    Registration(String),
}

impl From<VfsError> for io::Error {
    fn from(err: VfsError) -> io::Error {
        io::Error::new(io::ErrorKind::Other, err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, VfsError>;
