use crate::error::{EngramError, Result};
use crate::archive::format::{CompressionMethod, EntryInfo, FileHeader};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Archive reader with O(1) file lookup
pub struct ArchiveReader {
    file: File,
    header: FileHeader,
    entries: HashMap<String, EntryInfo>,
    entry_list: Vec<String>,
}

/// This is essentially our "API"; the public facing portion of our code.
impl ArchiveReader {
    /// Open an archive file for reading
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(path)?;

        // Read header
        let header = FileHeader::read_from(&mut file)?;
        header.validate_version()?;

        // Seek to central directory
        file.seek(SeekFrom::Start(header.central_directory_offset))?;

        // Read all entries
        let mut entries = HashMap::with_capacity(header.entry_count as usize);
        let mut entry_list = Vec::with_capacity(header.entry_count as usize);

        for _ in 0..header.entry_count {
            let entry = EntryInfo::read_from(&mut file)?;
            entry_list.push(entry.path.clone());
            entries.insert(entry.path.clone(), entry);
        }

        Ok(Self {
            file,
            header,
            entries,
            entry_list,
        })
    }

    /// Get archive header information
    pub fn header(&self) -> &FileHeader {
        &self.header
    }

    /// Get number of entries in archive
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// List all file paths in the archive
    pub fn list_files(&self) -> &[String] {
        &self.entry_list
    }

    /// Check if a file exists in the archive
    pub fn contains(&self, path: &str) -> bool {
        self.entries.contains_key(path)
    }

    /// Get entry information without reading data
    pub fn get_entry(&self, path: &str) -> Option<&EntryInfo> {
        self.entries.get(path)
    }

    /// Read a file from the archive
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>> {
        let entry = self
            .entries
            .get(path)
            .ok_or_else(|| EngramError::FileNotFound(path.to_string()))?;

        // Seek to file data
        self.file.seek(SeekFrom::Start(entry.data_offset))?;

        // Read compressed data
        let mut compressed_data = vec![0u8; entry.compressed_size as usize];
        self.file.read_exact(&mut compressed_data)?;

        // Decompress if needed
        let decompressed = match entry.compression {
            CompressionMethod::None => compressed_data,
            CompressionMethod::Lz4 => Self::decompress_lz4(&compressed_data, entry)?,
            CompressionMethod::Zstd => Self::decompress_zstd(&compressed_data)?,
        };

        // Verify CRC
        let computed_crc = crc32fast::hash(&decompressed);
        if computed_crc != entry.crc32 {
            return Err(EngramError::CrcMismatch {
                expected: entry.crc32,
                actual: computed_crc,
            });
        }

        Ok(decompressed)
    }

    /// Decompress LZ4 data
    fn decompress_lz4(data: &[u8], _entry: &EntryInfo) -> Result<Vec<u8>> {
        // lz4_flex::compress_prepend_size prepends the size, so we use decompress_size_prepended
        lz4_flex::decompress_size_prepended(data).map_err(|e| {
            EngramError::DecompressionFailed(format!("LZ4 decompression failed: {}", e))
        })
    }

    /// Decompress Zstd data
    fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>> {
        zstd::decode_all(data)
            .map_err(|e| EngramError::DecompressionFailed(format!("Zstd decompression failed: {}", e)))
    }

    /// Read and parse manifest.json if it exists
    pub fn read_manifest(&mut self) -> Result<Option<serde_json::Value>> {
        if !self.contains("manifest.json") {
            return Ok(None);
        }

        let data = self.read_file("manifest.json")?;
        let manifest: serde_json::Value = serde_json::from_slice(&data)
            .map_err(|e| EngramError::InvalidManifest(format!("Invalid manifest.json: {}", e)))?;

        Ok(Some(manifest))
    }

    /// Extract all entries with a given prefix
    pub fn list_prefix(&self, prefix: &str) -> Vec<&String> {
        self.entry_list
            .iter()
            .filter(|path| path.starts_with(prefix))
            .collect()
    }
}
