use crate::archive::format::CompressionMethod;
use crate::error::{EngramError, Result};
use std::io::{Read, Write};

/// LOCA signature for local file entry headers
pub const LOCAL_ENTRY_SIGNATURE: [u8; 4] = [0x4C, 0x4F, 0x43, 0x41]; // "LOCA"

/// Local File Entry Header
///
/// Precedes each file's compressed data in the archive, enabling sequential
/// streaming reads without consulting the central directory.
///
/// Structure (variable length):
/// - Signature: "LOCA" (4 bytes)
/// - Uncompressed Size: uint64 (8 bytes)
/// - Compressed Size: uint64 (8 bytes)
/// - CRC32: uint32 (4 bytes)
/// - Modified Timestamp: uint64 (8 bytes)
/// - Compression Method: uint8 (1 byte)
/// - Flags: uint8 (1 byte)
/// - Path Length: uint16 (2 bytes)
/// - Reserved: 4 bytes
/// - File Path: variable (null-terminated UTF-8)
#[derive(Debug, Clone)]
pub struct LocalEntryHeader {
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub modified_time: u64,
    pub compression: CompressionMethod,
    pub flags: u8,
    pub path: String,
}

impl LocalEntryHeader {
    /// Create a new local entry header
    pub fn new(
        uncompressed_size: u64,
        compressed_size: u64,
        crc32: u32,
        modified_time: u64,
        compression: CompressionMethod,
        path: String,
    ) -> Self {
        Self {
            uncompressed_size,
            compressed_size,
            crc32,
            modified_time,
            compression,
            flags: 0,
            path,
        }
    }

    /// Write local entry header to a writer
    pub fn write_to<W: Write>(&self, mut writer: W) -> Result<usize> {
        let mut bytes_written = 0;

        // Signature "LOCA"
        writer.write_all(&LOCAL_ENTRY_SIGNATURE)?;
        bytes_written += 4;

        // Uncompressed size
        writer.write_all(&self.uncompressed_size.to_le_bytes())?;
        bytes_written += 8;

        // Compressed size
        writer.write_all(&self.compressed_size.to_le_bytes())?;
        bytes_written += 8;

        // CRC32
        writer.write_all(&self.crc32.to_le_bytes())?;
        bytes_written += 4;

        // Modified timestamp
        writer.write_all(&self.modified_time.to_le_bytes())?;
        bytes_written += 8;

        // Compression method
        writer.write_all(&[self.compression as u8])?;
        bytes_written += 1;

        // Flags
        writer.write_all(&[self.flags])?;
        bytes_written += 1;

        // Path length
        let path_bytes = self.path.as_bytes();
        if path_bytes.len() > u16::MAX as usize {
            return Err(EngramError::PathError(format!(
                "Path too long: {} bytes (max {})",
                path_bytes.len(),
                u16::MAX
            )));
        }
        let path_len = path_bytes.len() as u16;
        writer.write_all(&path_len.to_le_bytes())?;
        bytes_written += 2;

        // Reserved (4 bytes)
        writer.write_all(&[0u8; 4])?;
        bytes_written += 4;

        // Path (null-terminated)
        writer.write_all(path_bytes)?;
        writer.write_all(&[0u8])?; // Null terminator
        bytes_written += path_bytes.len() + 1;

        Ok(bytes_written)
    }

    /// Read local entry header from a reader
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        // Read and verify signature
        let mut sig = [0u8; 4];
        reader.read_exact(&mut sig)?;
        if sig != LOCAL_ENTRY_SIGNATURE {
            return Err(EngramError::InvalidFormat(
                "Invalid local entry signature (expected LOCA)".to_string(),
            ));
        }

        // Read fields
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

        // Skip reserved bytes
        let mut reserved = [0u8; 4];
        reader.read_exact(&mut reserved)?;

        // Read path
        let mut path_buf = vec![0u8; path_len as usize];
        reader.read_exact(&mut path_buf)?;

        let path = String::from_utf8(path_buf)
            .map_err(|e| EngramError::PathError(format!("Invalid UTF-8 in path: {}", e)))?;

        // Read null terminator
        let mut null_term = [0u8; 1];
        reader.read_exact(&mut null_term)?;
        if null_term[0] != 0 {
            return Err(EngramError::InvalidFormat(
                "Missing null terminator in local entry path".to_string(),
            ));
        }

        Ok(Self {
            uncompressed_size,
            compressed_size,
            crc32,
            modified_time,
            compression,
            flags: flags[0],
            path,
        })
    }

    /// Calculate the total size of this header when written
    pub fn header_size(&self) -> usize {
        4 + // Signature
        8 + // Uncompressed size
        8 + // Compressed size
        4 + // CRC32
        8 + // Modified timestamp
        1 + // Compression method
        1 + // Flags
        2 + // Path length
        4 + // Reserved
        self.path.len() + 1 // Path + null terminator
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_entry_roundtrip() {
        let entry = LocalEntryHeader::new(
            10000,
            5000,
            0x12345678,
            1703001600,
            CompressionMethod::Zstd,
            "test/file.txt".to_string(),
        );

        let mut buf = Vec::new();
        let written = entry.write_to(&mut buf).unwrap();

        assert_eq!(written, entry.header_size());

        let parsed = LocalEntryHeader::read_from(&buf[..]).unwrap();
        assert_eq!(parsed.path, entry.path);
        assert_eq!(parsed.uncompressed_size, entry.uncompressed_size);
        assert_eq!(parsed.compressed_size, entry.compressed_size);
        assert_eq!(parsed.crc32, entry.crc32);
        assert_eq!(parsed.compression, entry.compression);
    }

    #[test]
    fn test_signature_validation() {
        let mut buf = vec![0xFF, 0xFF, 0xFF, 0xFF]; // Invalid signature
        buf.extend_from_slice(&[0u8; 40]); // Rest of header

        let result = LocalEntryHeader::read_from(&buf[..]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid local entry signature"));
    }
}
