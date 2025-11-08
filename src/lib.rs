//! Engram-rs: Unified archive library with manifest, signatures, and VFS support
//!
//! This library provides a complete implementation of the Engram v0.3 archive format,
//! combining:
//! - Compressed archive storage (LZ4/Zstd)
//! - Manifest with signature verification
//! - VFS (Virtual File System) via embedded SQLite databases
//! - Fast O(1) file lookup
//!
//! # Example
//!
//! ```no_run
//! use engram_rs::{ArchiveWriter, ArchiveReader};
//!
//! // Create an archive
//! let mut writer = ArchiveWriter::create("example.eng")?;
//! writer.add_file("data.txt", b"Hello, World!")?;
//! writer.finalize()?;
//!
//! // Read from archive
//! let mut reader = ArchiveReader::open("example.eng")?;
//! let data = reader.read_file("data.txt")?;
//! # Ok::<(), engram_rs::error::EngramError>(())
//! ```

// Core modules
pub mod error;
pub mod archive;
pub mod manifest;
pub mod vfs;

// Re-export commonly used types
pub use archive::{
    ArchiveReader, ArchiveWriter, CompressionMethod, EntryInfo, FileHeader,
    MAGIC_NUMBER, FORMAT_VERSION_MAJOR, FORMAT_VERSION_MINOR,
    HEADER_SIZE, CD_ENTRY_SIZE, MAX_PATH_LENGTH,
};
pub use error::{EngramError, Result};
pub use manifest::{Manifest, Author, Metadata, FileEntry, SignatureEntry};
pub use vfs::VfsReader;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_basics() {
        // Ensure core types are accessible
        let _method = CompressionMethod::Zstd;
        let _header = FileHeader::new();
    }
}
