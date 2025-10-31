//! Archive writer implementation with compression support

use crate::error::{EngramError, Result};
use crate::format::{CompressionMethod, EntryInfo, FileHeader};
use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Threshold below which files are not compressed (4KB)
const MIN_COMPRESSION_SIZE: usize = 4096;

/// Archive writer for creating .eng files
pub struct ArchiveWriter {
    writer: BufWriter<File>,
    entries: Vec<EntryInfo>,
    current_offset: u64,
}

impl ArchiveWriter {
    /// Create a new archive file
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        // Write placeholder header (will be updated at finalization)
        let header = FileHeader::new();
        header.write_to(&mut writer)?;

        Ok(Self {
            writer,
            entries: Vec::new(),
            current_offset: 64, // After header
        })
    }

    /// Add a file to the archive with automatic compression selection
    pub fn add_file(&mut self, path: &str, data: &[u8]) -> Result<()> {
        // Determine compression method based on file size and type
        let compression = Self::select_compression(path, data.len());
        self.add_file_with_compression(path, data, compression)
    }

    /// Add a file with specific compression method
    pub fn add_file_with_compression(
        &mut self,
        path: &str,
        data: &[u8],
        compression: CompressionMethod,
    ) -> Result<()> {
        // Compress data if needed
        let (compressed_data, actual_compression) = self.compress_data(data, compression)?;

        // Calculate CRC32 of uncompressed data
        let crc32 = crc32fast::hash(data);

        // Get current timestamp
        let modified_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Create entry
        let entry = EntryInfo {
            path: path.to_string(),
            data_offset: self.current_offset,
            uncompressed_size: data.len() as u64,
            compressed_size: compressed_data.len() as u64,
            crc32,
            modified_time,
            compression: actual_compression,
            flags: 0,
        };

        // Write compressed data
        self.writer.write_all(&compressed_data)?;
        self.current_offset += compressed_data.len() as u64;

        // Store entry for central directory
        self.entries.push(entry);

        Ok(())
    }

    /// Add a file from disk
    pub fn add_file_from_disk(&mut self, archive_path: &str, disk_path: &Path) -> Result<()> {
        let data = std::fs::read(disk_path)?;
        self.add_file(archive_path, &data)
    }

    /// Add manifest.json from a serde_json::Value
    pub fn add_manifest(&mut self, manifest: &serde_json::Value) -> Result<()> {
        let json = serde_json::to_vec_pretty(manifest)
            .map_err(|e| EngramError::InvalidStructure(format!("Failed to serialize manifest: {}", e)))?;

        // Manifests are typically small, store uncompressed for instant access
        self.add_file_with_compression("manifest.json", &json, CompressionMethod::None)
    }

    /// Finalize the archive by writing central directory and updating header
    pub fn finalize(mut self) -> Result<()> {
        // Record central directory start
        let cd_offset = self.current_offset;

        // Write central directory entries
        for entry in &self.entries {
            entry.write_to(&mut self.writer)?;
        }

        let cd_size = self.current_offset - cd_offset + (self.entries.len() as u64 * 320);

        // Flush writer
        self.writer.flush()?;

        // Get inner file for seeking
        let mut file = self.writer.into_inner().map_err(|e| e.into_error())?;

        // Seek back to header
        file.seek(SeekFrom::Start(0))?;

        // Write final header
        let mut header = FileHeader::new();
        header.central_directory_offset = cd_offset;
        header.central_directory_size = cd_size;
        header.entry_count = self.entries.len() as u32;
        header.write_to(&mut file)?;

        // Final flush
        file.flush()?;

        Ok(())
    }

    /// Select appropriate compression method based on file characteristics
    fn select_compression(path: &str, size: usize) -> CompressionMethod {
        // Don't compress small files
        if size < MIN_COMPRESSION_SIZE {
            return CompressionMethod::None;
        }

        // Get file extension
        let extension = path.rsplit('.').next().unwrap_or("").to_lowercase();

        match extension.as_str() {
            // Already compressed formats
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "mp3" | "mp4" | "zip" | "gz" | "bz2" => {
                CompressionMethod::None
            }
            // Text formats - use Zstd for best compression
            "json" | "txt" | "xml" | "html" | "css" | "js" | "ts" | "md" | "csv" => {
                CompressionMethod::Zstd
            }
            // Database files - use Zstd level 6
            "db" | "sqlite" | "sqlite3" => CompressionMethod::Zstd,
            // Default: LZ4 for speed
            _ => CompressionMethod::Lz4,
        }
    }

    /// Compress data with fallback to uncompressed if not beneficial
    fn compress_data(
        &self,
        data: &[u8],
        compression: CompressionMethod,
    ) -> Result<(Vec<u8>, CompressionMethod)> {
        let compressed = match compression {
            CompressionMethod::None => return Ok((data.to_vec(), CompressionMethod::None)),
            CompressionMethod::Lz4 => Self::compress_lz4(data)?,
            CompressionMethod::Zstd => Self::compress_zstd(data)?,
            CompressionMethod::Deflate => {
                return Err(EngramError::CompressionError(
                    "Deflate not yet implemented".to_string(),
                ))
            }
        };

        // Use compressed only if it's actually smaller
        if compressed.len() < data.len() {
            Ok((compressed, compression))
        } else {
            Ok((data.to_vec(), CompressionMethod::None))
        }
    }

    /// Compress with LZ4
    fn compress_lz4(data: &[u8]) -> Result<Vec<u8>> {
        Ok(lz4_flex::compress_prepend_size(data))
    }

    /// Compress with Zstd (level 6 for balanced compression)
    fn compress_zstd(data: &[u8]) -> Result<Vec<u8>> {
        zstd::encode_all(data, 6)
            .map_err(|e| EngramError::CompressionError(format!("Zstd compression failed: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_selection() {
        assert_eq!(
            ArchiveWriter::select_compression("test.jpg", 10000),
            CompressionMethod::None
        );
        assert_eq!(
            ArchiveWriter::select_compression("test.json", 10000),
            CompressionMethod::Zstd
        );
        assert_eq!(
            ArchiveWriter::select_compression("test.db", 10000),
            CompressionMethod::Zstd
        );
        assert_eq!(
            ArchiveWriter::select_compression("test.bin", 10000),
            CompressionMethod::Lz4
        );
    }

    #[test]
    fn test_small_file_no_compression() {
        assert_eq!(
            ArchiveWriter::select_compression("test.txt", 1000),
            CompressionMethod::None
        );
    }
}
