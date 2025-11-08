use std::io;
use thiserror::Error;

/// Result type for engram operations
pub type Result<T> = std::result::Result<T, EngramError>;

/// Unified error type for all engram operations
#[derive(Debug, Error)]
pub enum EngramError {
    // Archive errors
    #[error("Invalid archive format: {0}")]
    InvalidFormat(String),

    #[error("Invalid magic number in archive header")]
    InvalidMagic,

    #[error("Unsupported archive version: {0}")]
    UnsupportedVersion(u16),

    #[error("File not found in archive: {0}")]
    FileNotFound(String),

    #[error("Invalid compression method: {0}")]
    InvalidCompression(u8),

    #[error("Compression failed: {0}")]
    CompressionFailed(String),

    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),

    #[error("CRC mismatch: expected {expected:08x}, got {actual:08x}")]
    CrcMismatch { expected: u32, actual: u32 },

    // VFS errors
    #[error("Database not found in archive: {0}")]
    DatabaseNotFound(String),

    #[error("Failed to extract database: {0}")]
    ExtractionFailed(String),

    #[error("SQLite error: {0}")]
    SqliteError(#[from] rusqlite::Error),

    // Manifest errors
    #[error("Invalid manifest: {0}")]
    InvalidManifest(String),

    #[error("Manifest not found in archive")]
    ManifestNotFound,

    #[error("Failed to parse manifest: {0}")]
    ManifestParseFailed(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid capability: {0}")]
    InvalidCapability(String),

    // Signature errors
    #[error("Signature verification failed: {0}")]
    SignatureVerificationFailed(String),

    #[error("Invalid signature format")]
    InvalidSignature,

    #[error("Signature not found")]
    SignatureNotFound,

    #[error("Invalid public key")]
    InvalidPublicKey,

    #[error("Invalid secret key")]
    InvalidSecretKey,

    // I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Path error: {0}")]
    PathError(String),

    // Serialization errors
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("TOML error: {0}")]
    TomlError(String),

    // General errors
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("{0}")]
    Other(String),
}

impl From<toml::de::Error> for EngramError {
    fn from(err: toml::de::Error) -> Self {
        EngramError::TomlError(err.to_string())
    }
}

impl From<toml::ser::Error> for EngramError {
    fn from(err: toml::ser::Error) -> Self {
        EngramError::TomlError(err.to_string())
    }
}

impl From<ed25519_dalek::SignatureError> for EngramError {
    fn from(err: ed25519_dalek::SignatureError) -> Self {
        EngramError::SignatureVerificationFailed(err.to_string())
    }
}
