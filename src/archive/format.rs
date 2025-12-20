use crate::error::{EngramError, Result};
use std::io::{Read, Write};

/// Magic number: 0x89 'E' 'N' 'G' 0x0D 0x0A 0x1A 0x0A
/// Follows PNG pattern for corruption detection
pub const MAGIC_NUMBER: [u8; 8] = [0x89, b'E', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Current format version - v1.0 with LOCA, ENDR, and frame compression
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
}

/// Encryption modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EncryptionMode {
    /// No encryption
    None = 0b00,
    /// Entire archive encrypted (for backups/secure storage)
    Archive = 0b01,
    /// Each file encrypted individually (for queryable archives)
    PerFile = 0b10,
}

impl CompressionMethod {
    pub fn from_u8(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Lz4),
            2 => Ok(Self::Zstd),
            _ => Err(EngramError::InvalidCompression(value)),
        }
    }

    /// Choose best compression based on file type and size
    pub fn choose_for_file(path: &str, size: u64) -> Self {
        // Don't compress small files
        if size < 4096 {
            return Self::None;
        }

        // Check file extension
        let path_lower = path.to_lowercase();

        // Already compressed formats
        if path_lower.ends_with(".png")
            || path_lower.ends_with(".jpg")
            || path_lower.ends_with(".jpeg")
            || path_lower.ends_with(".gif")
            || path_lower.ends_with(".mp3")
            || path_lower.ends_with(".mp4")
            || path_lower.ends_with(".zip")
            || path_lower.ends_with(".gz")
            || path_lower.ends_with(".7z")
        {
            return Self::None;
        }

        // Use Zstd for text/structured data (better compression)
        if path_lower.ends_with(".txt")
            || path_lower.ends_with(".md")
            || path_lower.ends_with(".json")
            || path_lower.ends_with(".toml")
            || path_lower.ends_with(".xml")
            || path_lower.ends_with(".cml")
            || path_lower.ends_with(".html")
            || path_lower.ends_with(".css")
            || path_lower.ends_with(".js")
        {
            return Self::Zstd;
        }

        // Use LZ4 for binary data (faster)
        if path_lower.ends_with(".db")
            || path_lower.ends_with(".sqlite")
            || path_lower.ends_with(".wasm")
        {
            return Self::Lz4;
        }

        // Default: Zstd for good compression
        Self::Zstd
    }
}

impl EncryptionMode {
    /// Extract encryption mode from flags field
    pub fn from_flags(flags: u32) -> Self {
        match flags & 0b11 {
            0b00 => Self::None,
            0b01 => Self::Archive,
            0b10 => Self::PerFile,
            _ => Self::None, // Reserved, treat as None
        }
    }

    /// Convert encryption mode to flags bits
    pub fn to_flags(self) -> u32 {
        self as u32
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
    pub flags: u32,
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
            flags: 0,
        }
    }

    /// Set encryption mode in flags
    pub fn set_encryption_mode(&mut self, mode: EncryptionMode) {
        self.flags = (self.flags & !0b11) | mode.to_flags();
    }

    /// Get encryption mode from flags
    pub fn encryption_mode(&self) -> EncryptionMode {
        EncryptionMode::from_flags(self.flags)
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
        writer.write_all(&self.flags.to_le_bytes())?;

        // Write reserved bytes (20 bytes of zeros - was 24, now 20 due to flags)
        writer.write_all(&[0u8; 20])?;

        Ok(())
    }

    /// Read header from a reader
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;

        if magic != MAGIC_NUMBER {
            return Err(EngramError::InvalidMagic);
        }

        let version_major = read_u16(&mut reader)?;
        let version_minor = read_u16(&mut reader)?;
        let header_crc = read_u32(&mut reader)?;
        let central_directory_offset = read_u64(&mut reader)?;
        let central_directory_size = read_u64(&mut reader)?;
        let entry_count = read_u32(&mut reader)?;
        let content_version = read_u32(&mut reader)?;

        // Read flags (v1.0+ or v0.4+)
        let flags = if version_major >= 1 || version_minor >= 4 {
            read_u32(&mut reader)?
        } else {
            // v0.3 compatibility: skip 4 bytes, flags = 0 (no encryption)
            let mut skip = [0u8; 4];
            reader.read_exact(&mut skip)?;
            0
        };

        // Skip remaining reserved bytes (20 bytes for v0.4+, 20 bytes for v0.3)
        let mut reserved = [0u8; 20];
        reader.read_exact(&mut reserved)?;

        Ok(Self {
            version_major,
            version_minor,
            header_crc,
            central_directory_offset,
            central_directory_size,
            entry_count,
            content_version,
            flags,
        })
    }

    /// Validate version compatibility
    pub fn validate_version(&self) -> Result<()> {
        if self.version_major > FORMAT_VERSION_MAJOR {
            return Err(EngramError::UnsupportedVersion(
                (self.version_major as u16) << 8 | self.version_minor as u16,
            ));
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
            return Err(EngramError::PathError(format!(
                "Path too long: {} bytes (max {})",
                path_bytes.len(),
                MAX_PATH_LENGTH
            )));
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
            return Err(EngramError::InvalidFormat(
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

        let path = String::from_utf8(path_buf[..path_len as usize].to_vec())
            .map_err(|e| EngramError::PathError(format!("Invalid UTF-8 in path: {}", e)))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_method_from_u8() {
        assert_eq!(CompressionMethod::from_u8(0).unwrap(), CompressionMethod::None);
        assert_eq!(CompressionMethod::from_u8(1).unwrap(), CompressionMethod::Lz4);
        assert_eq!(CompressionMethod::from_u8(2).unwrap(), CompressionMethod::Zstd);
        assert!(CompressionMethod::from_u8(99).is_err());
    }

    #[test]
    fn test_compression_choice() {
        // Files >= 4KB get compressed
        assert_eq!(CompressionMethod::choose_for_file("test.txt", 5000), CompressionMethod::Zstd);
        assert_eq!(CompressionMethod::choose_for_file("test.json", 10000), CompressionMethod::Zstd);
        assert_eq!(CompressionMethod::choose_for_file("test.db", 10000), CompressionMethod::Lz4);
        // Already compressed formats - never compressed
        assert_eq!(CompressionMethod::choose_for_file("test.png", 10000), CompressionMethod::None);
        // Files < 4KB not compressed
        assert_eq!(CompressionMethod::choose_for_file("test.txt", 2000), CompressionMethod::None);
        assert_eq!(CompressionMethod::choose_for_file("test.txt", 500), CompressionMethod::None);
    }

    #[test]
    fn test_file_header_roundtrip() {
        let header = FileHeader {
            version_major: 0,
            version_minor: 4,
            header_crc: 0x12345678,
            central_directory_offset: 1024,
            central_directory_size: 512,
            entry_count: 10,
            content_version: 1,
            flags: 0,
        };

        let mut buf = Vec::new();
        header.write_to(&mut buf).unwrap();

        assert_eq!(buf.len(), HEADER_SIZE);

        let parsed = FileHeader::read_from(&buf[..]).unwrap();
        assert_eq!(parsed.version_major, header.version_major);
        assert_eq!(parsed.version_minor, header.version_minor);
        assert_eq!(parsed.header_crc, header.header_crc);
        assert_eq!(parsed.central_directory_offset, header.central_directory_offset);
        assert_eq!(parsed.entry_count, header.entry_count);
    }

    #[test]
    fn test_entry_info_roundtrip() {
        let entry = EntryInfo {
            path: "test/file.txt".to_string(),
            data_offset: 1024,
            uncompressed_size: 5000,
            compressed_size: 2000,
            crc32: 0xDEADBEEF,
            modified_time: 1699999999,
            compression: CompressionMethod::Zstd,
            flags: 0,
        };

        let mut buf = Vec::new();
        entry.write_to(&mut buf).unwrap();

        assert_eq!(buf.len(), CD_ENTRY_SIZE);

        let parsed = EntryInfo::read_from(&buf[..]).unwrap();
        assert_eq!(parsed.path, entry.path);
        assert_eq!(parsed.data_offset, entry.data_offset);
        assert_eq!(parsed.uncompressed_size, entry.uncompressed_size);
        assert_eq!(parsed.compressed_size, entry.compressed_size);
        assert_eq!(parsed.crc32, entry.crc32);
        assert_eq!(parsed.compression, entry.compression);
    }
}
