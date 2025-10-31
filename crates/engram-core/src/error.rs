//! Error types for engram-core

use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EngramError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid magic number: expected {expected:X}, found {found:X}")]
    InvalidMagic { expected: u64, found: u64 },

    #[error("Unsupported format version: {major}.{minor}")]
    UnsupportedVersion { major: u16, minor: u16 },

    #[error("CRC mismatch for {path}: expected {expected:08X}, found {actual:08X}")]
    CrcMismatch {
        path: String,
        expected: u32,
        actual: u32,
    },

    #[error("File not found in archive: {0}")]
    FileNotFound(String),

    #[error("Compression error: {0}")]
    CompressionError(String),

    #[error("Decompression error: {0}")]
    DecompressionError(String),

    #[error("Invalid archive structure: {0}")]
    InvalidStructure(String),

    #[error("UTF-8 error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("Path too long: {0} bytes (max 255)")]
    PathTooLong(usize),
}

pub type Result<T> = std::result::Result<T, EngramError>;
