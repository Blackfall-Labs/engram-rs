//! # Engram Core
//!
//! Core archive format implementation for .eng files.
//! Provides reading and writing capabilities with compression support.

mod error;
mod format;
mod reader;
mod writer;

pub use error::{EngramError, Result};
pub use format::{CompressionMethod, EntryInfo, FileHeader, MAGIC_NUMBER};
pub use reader::ArchiveReader;
pub use writer::ArchiveWriter;

#[cfg(test)]
mod tests;
