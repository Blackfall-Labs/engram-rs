//! Binary format definitions for .eng archive files

use crate::error::{EngramError, Result};
use std::io::{Read, Write};

/// Magic number: 0x89 'E' 'N' 'G' 0x0D 0x0A 0x1A 0x0A
/// Follows PNG pattern for corruption detection
pub const MAGIC_NUMBER: [u8; 8] = [0x89, b'E', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Current format version
pub const FORMAT_VERSION_MAJOR: u16 = 1;
pub const FORMAT_VERSION_MINOR: u16 = 0;

/// Header size in bytes
pub const HEADER_SIZE: usize = 64;

/// Central Directory entry size in bytes
pub const CD_ENTRY_SIZE: usize = 320;

/// Maximum path length in bytes (UTF-8)
pub const MAX_PATH_LENGTH: usize = 255;

/// Compression methods supported
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CompressionMethod {
    None = 0,
    Lz4 = 1,
    Zstd = 2,
    Deflate = 3,
}

impl CompressionMethod {
    pub fn from_u8(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Lz4),
            2 => Ok(Self::Zstd),
            3 => Ok(Self::Deflate),
            _ => Err(EngramError::InvalidStructure(format!(
                "Unknown compression method: {}",
                value
            ))),
        }
    }
}

/// File header at the beginning of the archive
#[derive(Debug, Clone)]
pub struct FileHeader {
    pub version_major: u16,
    pub version_minor: u16,
    pub header_crc: u32,
    pub central_directory_offset: u64,
    pub central_directory_size: u64,
    pub entry_count: u32,
    pub content_version: u32,
}

impl FileHeader {
    pub fn new() -> Self {
        Self {
            version_major: FORMAT_VERSION_MAJOR,
            version_minor: FORMAT_VERSION_MINOR,
            header_crc: 0,
            central_directory_offset: 0,
            central_directory_size: 0,
            entry_count: 0,
            content_version: 0,
        }
    }

    /// Write header to a writer
    pub fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        writer.write_all(&MAGIC_NUMBER)?;
        writer.write_all(&self.version_major.to_le_bytes())?;
        writer.write_all(&self.version_minor.to_le_bytes())?;
        writer.write_all(&self.header_crc.to_le_bytes())?;
        writer.write_all(&self.central_directory_offset.to_le_bytes())?;
        writer.write_all(&self.central_directory_size.to_le_bytes())?;
        writer.write_all(&self.entry_count.to_le_bytes())?;
        writer.write_all(&self.content_version.to_le_bytes())?;

        // Write reserved bytes (24 bytes of zeros)
        writer.write_all(&[0u8; 24])?;

        Ok(())
    }

    /// Read header from a reader
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;

        if magic != MAGIC_NUMBER {
            return Err(EngramError::InvalidMagic {
                expected: u64::from_be_bytes(MAGIC_NUMBER),
                found: u64::from_be_bytes(magic),
            });
        }

        let version_major = read_u16(&mut reader)?;
        let version_minor = read_u16(&mut reader)?;
        let header_crc = read_u32(&mut reader)?;
        let central_directory_offset = read_u64(&mut reader)?;
        let central_directory_size = read_u64(&mut reader)?;
        let entry_count = read_u32(&mut reader)?;
        let content_version = read_u32(&mut reader)?;

        // Skip reserved bytes
        let mut reserved = [0u8; 24];
        reader.read_exact(&mut reserved)?;

        Ok(Self {
            version_major,
            version_minor,
            header_crc,
            central_directory_offset,
            central_directory_size,
            entry_count,
            content_version,
        })
    }

    /// Validate version compatibility
    pub fn validate_version(&self) -> Result<()> {
        if self.version_major > FORMAT_VERSION_MAJOR {
            return Err(EngramError::UnsupportedVersion {
                major: self.version_major,
                minor: self.version_minor,
            });
        }
        Ok(())
    }
}

impl Default for FileHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// Central Directory entry metadata
#[derive(Debug, Clone)]
pub struct EntryInfo {
    pub path: String,
    pub data_offset: u64,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub modified_time: u64,
    pub compression: CompressionMethod,
    pub flags: u8,
}

impl EntryInfo {
    /// Write entry to central directory
    pub fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        // Signature "CENT" (0x43454E54)
        writer.write_all(&[0x43, 0x45, 0x4E, 0x54])?;

        writer.write_all(&self.data_offset.to_le_bytes())?;
        writer.write_all(&self.uncompressed_size.to_le_bytes())?;
        writer.write_all(&self.compressed_size.to_le_bytes())?;
        writer.write_all(&self.crc32.to_le_bytes())?;
        writer.write_all(&self.modified_time.to_le_bytes())?;
        writer.write_all(&[self.compression as u8])?;
        writer.write_all(&[self.flags])?;

        // Path length and path
        let path_bytes = self.path.as_bytes();
        if path_bytes.len() > MAX_PATH_LENGTH {
            return Err(EngramError::PathTooLong(path_bytes.len()));
        }

        let path_len = path_bytes.len() as u16;
        writer.write_all(&path_len.to_le_bytes())?;

        // Path buffer (256 bytes, null-terminated)
        let mut path_buf = [0u8; 256];
        path_buf[..path_bytes.len()].copy_from_slice(path_bytes);
        writer.write_all(&path_buf)?;

        // Reserved (20 bytes)
        writer.write_all(&[0u8; 20])?;

        Ok(())
    }

    /// Read entry from central directory
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        // Read and verify signature
        let mut sig = [0u8; 4];
        reader.read_exact(&mut sig)?;
        if sig != [0x43, 0x45, 0x4E, 0x54] {
            return Err(EngramError::InvalidStructure(
                "Invalid central directory entry signature".to_string(),
            ));
        }

        let data_offset = read_u64(&mut reader)?;
        let uncompressed_size = read_u64(&mut reader)?;
        let compressed_size = read_u64(&mut reader)?;
        let crc32 = read_u32(&mut reader)?;
        let modified_time = read_u64(&mut reader)?;

        let mut compression_byte = [0u8; 1];
        reader.read_exact(&mut compression_byte)?;
        let compression = CompressionMethod::from_u8(compression_byte[0])?;

        let mut flags = [0u8; 1];
        reader.read_exact(&mut flags)?;

        let path_len = read_u16(&mut reader)?;

        let mut path_buf = [0u8; 256];
        reader.read_exact(&mut path_buf)?;

        let path = String::from_utf8(path_buf[..path_len as usize].to_vec())?;

        // Skip reserved bytes
        let mut reserved = [0u8; 20];
        reader.read_exact(&mut reserved)?;

        Ok(Self {
            path,
            data_offset,
            uncompressed_size,
            compressed_size,
            crc32,
            modified_time,
            compression,
            flags: flags[0],
        })
    }
}

// Helper functions for reading primitive types
fn read_u16<R: Read>(mut reader: R) -> Result<u16> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_u32<R: Read>(mut reader: R) -> Result<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64<R: Read>(mut reader: R) -> Result<u64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}
