use crate::error::{EngramError, Result};
use std::io::{Read, Write};

/// ENDR signature for End of Central Directory Record
pub const END_RECORD_SIGNATURE: [u8; 4] = [0x45, 0x4E, 0x44, 0x52]; // "ENDR"

/// End Record size in bytes (fixed)
pub const END_RECORD_SIZE: usize = 64;

/// End of Central Directory Record (ENDR)
///
/// Located at the very end of the archive (last 64 bytes).
/// Allows readers to validate archive completeness and locate the central directory
/// by reading from the end of the file without scanning from the beginning.
///
/// Structure (64 bytes fixed):
/// - Signature: "ENDR" (4 bytes)
/// - Version Major: uint16 (2 bytes)
/// - Version Minor: uint16 (2 bytes)
/// - Central Directory Offset: uint64 (8 bytes)
/// - Central Directory Size: uint64 (8 bytes)
/// - Entry Count: uint32 (4 bytes)
/// - Archive CRC32: uint32 (4 bytes)
/// - Reserved: 32 bytes
#[derive(Debug, Clone)]
pub struct EndRecord {
    pub version_major: u16,
    pub version_minor: u16,
    pub central_directory_offset: u64,
    pub central_directory_size: u64,
    pub entry_count: u32,
    pub archive_crc32: u32,
}

impl EndRecord {
    /// Create a new end record
    pub fn new(
        version_major: u16,
        version_minor: u16,
        central_directory_offset: u64,
        central_directory_size: u64,
        entry_count: u32,
        archive_crc32: u32,
    ) -> Self {
        Self {
            version_major,
            version_minor,
            central_directory_offset,
            central_directory_size,
            entry_count,
            archive_crc32,
        }
    }

    /// Write end record to a writer
    pub fn write_to<W: Write>(&self, mut writer: W) -> Result<usize> {
        let mut bytes_written = 0;

        // Signature "ENDR"
        writer.write_all(&END_RECORD_SIGNATURE)?;
        bytes_written += 4;

        // Version
        writer.write_all(&self.version_major.to_le_bytes())?;
        bytes_written += 2;
        writer.write_all(&self.version_minor.to_le_bytes())?;
        bytes_written += 2;

        // Central directory offset
        writer.write_all(&self.central_directory_offset.to_le_bytes())?;
        bytes_written += 8;

        // Central directory size
        writer.write_all(&self.central_directory_size.to_le_bytes())?;
        bytes_written += 8;

        // Entry count
        writer.write_all(&self.entry_count.to_le_bytes())?;
        bytes_written += 4;

        // Archive CRC32
        writer.write_all(&self.archive_crc32.to_le_bytes())?;
        bytes_written += 4;

        // Reserved (32 bytes)
        writer.write_all(&[0u8; 32])?;
        bytes_written += 32;

        Ok(bytes_written)
    }

    /// Read end record from a reader
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        // Read and verify signature
        let mut sig = [0u8; 4];
        reader.read_exact(&mut sig)?;
        if sig != END_RECORD_SIGNATURE {
            return Err(EngramError::InvalidFormat(
                "Invalid end record signature (expected ENDR)".to_string(),
            ));
        }

        // Read version
        let version_major = read_u16(&mut reader)?;
        let version_minor = read_u16(&mut reader)?;

        // Read central directory info
        let central_directory_offset = read_u64(&mut reader)?;
        let central_directory_size = read_u64(&mut reader)?;

        // Read entry count
        let entry_count = read_u32(&mut reader)?;

        // Read archive CRC32
        let archive_crc32 = read_u32(&mut reader)?;

        // Skip reserved bytes
        let mut reserved = [0u8; 32];
        reader.read_exact(&mut reserved)?;

        Ok(Self {
            version_major,
            version_minor,
            central_directory_offset,
            central_directory_size,
            entry_count,
            archive_crc32,
        })
    }

    /// Validate end record matches header
    pub fn validate_against_header(
        &self,
        header_version_major: u16,
        header_version_minor: u16,
        header_cd_offset: u64,
        header_cd_size: u64,
        header_entry_count: u32,
    ) -> Result<()> {
        // Verify version matches
        if self.version_major != header_version_major || self.version_minor != header_version_minor
        {
            return Err(EngramError::InvalidFormat(format!(
                "ENDR version mismatch: header v{}.{}, ENDR v{}.{}",
                header_version_major, header_version_minor, self.version_major, self.version_minor
            )));
        }

        // Verify central directory offset matches
        if self.central_directory_offset != header_cd_offset {
            return Err(EngramError::InvalidFormat(format!(
                "ENDR central directory offset mismatch: header {}, ENDR {}",
                header_cd_offset, self.central_directory_offset
            )));
        }

        // Verify central directory size matches
        if self.central_directory_size != header_cd_size {
            return Err(EngramError::InvalidFormat(format!(
                "ENDR central directory size mismatch: header {}, ENDR {}",
                header_cd_size, self.central_directory_size
            )));
        }

        // Verify entry count matches
        if self.entry_count != header_entry_count {
            return Err(EngramError::InvalidFormat(format!(
                "ENDR entry count mismatch: header {}, ENDR {}",
                header_entry_count, self.entry_count
            )));
        }

        Ok(())
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
    fn test_end_record_roundtrip() {
        let record = EndRecord::new(
            1,          // version_major
            0,          // version_minor
            1024,       // central_directory_offset
            3200,       // central_directory_size (10 entries * 320 bytes)
            10,         // entry_count
            0xDEADBEEF, // archive_crc32
        );

        let mut buf = Vec::new();
        let written = record.write_to(&mut buf).unwrap();

        assert_eq!(written, END_RECORD_SIZE);
        assert_eq!(buf.len(), END_RECORD_SIZE);

        let parsed = EndRecord::read_from(&buf[..]).unwrap();
        assert_eq!(parsed.version_major, record.version_major);
        assert_eq!(parsed.version_minor, record.version_minor);
        assert_eq!(
            parsed.central_directory_offset,
            record.central_directory_offset
        );
        assert_eq!(parsed.central_directory_size, record.central_directory_size);
        assert_eq!(parsed.entry_count, record.entry_count);
        assert_eq!(parsed.archive_crc32, record.archive_crc32);
    }

    #[test]
    fn test_signature_validation() {
        let mut buf = vec![0xFF, 0xFF, 0xFF, 0xFF]; // Invalid signature
        buf.extend_from_slice(&[0u8; 60]); // Rest of record

        let result = EndRecord::read_from(&buf[..]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid end record signature"));
    }

    #[test]
    fn test_validate_against_header() {
        let record = EndRecord::new(1, 0, 1024, 3200, 10, 0);

        // Valid case
        assert!(record.validate_against_header(1, 0, 1024, 3200, 10).is_ok());

        // Version mismatch
        assert!(record
            .validate_against_header(0, 4, 1024, 3200, 10)
            .is_err());

        // Offset mismatch
        assert!(record
            .validate_against_header(1, 0, 2048, 3200, 10)
            .is_err());

        // Size mismatch
        assert!(record
            .validate_against_header(1, 0, 1024, 6400, 10)
            .is_err());

        // Entry count mismatch
        assert!(record
            .validate_against_header(1, 0, 1024, 3200, 20)
            .is_err());
    }
}
