use crate::error::{EngramError, Result};
use crate::archive::end_record::{EndRecord, END_RECORD_SIZE};
use crate::archive::format::{CompressionMethod, EncryptionMode, EntryInfo, FileHeader};
use crate::archive::frame_compression::{decompress_frames, should_use_frames};
use crate::archive::local_entry::LocalEntryHeader;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;

/// Normalize path to forward slashes (cross-platform compatibility)
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// Archive reader with O(1) file lookup
pub struct ArchiveReader {
    file: File,
    header: FileHeader,
    entries: HashMap<String, EntryInfo>,
    entry_list: Vec<String>,
    encryption_mode: EncryptionMode,
    decryption_key: Option<[u8; 32]>,
    decrypted_payload: Option<Vec<u8>>,
}

/// This is essentially our "API"; the public facing portion of our code.
impl ArchiveReader {
    /// Open an archive file for reading
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(path)?;

        // Read header
        let header = FileHeader::read_from(&mut file)?;
        header.validate_version()?;

        // Detect encryption mode
        let encryption_mode = header.encryption_mode();

        Ok(Self {
            file,
            header,
            entries: HashMap::new(),
            entry_list: Vec::new(),
            encryption_mode,
            decryption_key: None,
            decrypted_payload: None,
        })
    }

    /// Provide decryption key for encrypted archives
    pub fn with_decryption_key(mut self, key: &[u8; 32]) -> Self {
        self.decryption_key = Some(*key);
        self
    }

    /// Initialize the reader (must be called after open, decrypts if needed)
    pub fn initialize(&mut self) -> Result<()> {
        match self.encryption_mode {
            EncryptionMode::None => {
                // Validate ENDR for unencrypted archives
                self.validate_end_record()?;
                // Read central directory normally from file
                self.read_central_directory_from_file()?;
            }
            EncryptionMode::Archive => {
                // For encrypted archives, skip ENDR validation for now
                // TODO: Validate ENDR after decryption
                // Decrypt entire payload, then read central directory from memory
                self.decrypt_archive_payload()?;
                self.read_central_directory_from_memory()?;
            }
            EncryptionMode::PerFile => {
                // Validate ENDR for per-file encryption
                self.validate_end_record()?;
                // Central directory not encrypted, read normally
                self.read_central_directory_from_file()?;
            }
        }
        Ok(())
    }

    /// Read central directory from file
    fn read_central_directory_from_file(&mut self) -> Result<()> {
        // Seek to central directory
        self.file.seek(SeekFrom::Start(self.header.central_directory_offset))?;

        // Read all entries
        let mut entries = HashMap::with_capacity(self.header.entry_count as usize);
        let mut entry_list = Vec::with_capacity(self.header.entry_count as usize);

        for _ in 0..self.header.entry_count {
            let entry = EntryInfo::read_from(&mut self.file)?;
            entry_list.push(entry.path.clone());
            entries.insert(entry.path.clone(), entry);
        }

        self.entries = entries;
        self.entry_list = entry_list;
        Ok(())
    }

    /// Read central directory from decrypted payload buffer
    fn read_central_directory_from_memory(&mut self) -> Result<()> {
        let payload = self.decrypted_payload.as_ref()
            .ok_or(EngramError::DecryptionFailed)?;

        // Create cursor at central directory offset (payload-relative, so subtract header size)
        // The decrypted payload starts at what would be byte 64 in the file
        let cd_offset = (self.header.central_directory_offset - 64) as usize;
        let mut cursor = Cursor::new(&payload[cd_offset..]);

        // Read all entries from memory
        let mut entries = HashMap::with_capacity(self.header.entry_count as usize);
        let mut entry_list = Vec::with_capacity(self.header.entry_count as usize);

        for _ in 0..self.header.entry_count {
            let entry = EntryInfo::read_from(&mut cursor)?;
            entry_list.push(entry.path.clone());
            entries.insert(entry.path.clone(), entry);
        }

        self.entries = entries;
        self.entry_list = entry_list;
        Ok(())
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
        let normalized = normalize_path(path);
        self.entries.contains_key(&normalized) || self.entries.contains_key(path)
    }

    /// Get entry information without reading data
    pub fn get_entry(&self, path: &str) -> Option<&EntryInfo> {
        self.entries.get(path)
    }

    /// Read a file from the archive
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>> {
        // Normalize path and try both normalized and original
        let normalized = normalize_path(path);
        let entry = self
            .entries
            .get(&normalized)
            .or_else(|| self.entries.get(path))
            .ok_or_else(|| EngramError::FileNotFound(path.to_string()))?
            .clone();

        // Read data (from file or from decrypted payload)
        // For v1.0: entry.data_offset points to LOCA header, not file data
        let raw_data = match self.encryption_mode {
            EncryptionMode::Archive => {
                // Read from decrypted payload buffer
                let payload = self.decrypted_payload.as_ref()
                    .ok_or(EngramError::DecryptionFailed)?;

                // entry.data_offset is absolute (file offset), subtract header size for payload index
                let loca_start = (entry.data_offset - 64) as usize;

                // Read and validate LOCA header from memory
                let mut cursor = Cursor::new(&payload[loca_start..]);
                let local_header = LocalEntryHeader::read_from(&mut cursor)?;

                // Validate LOCA header matches central directory
                self.validate_local_header(&local_header, &entry)?;

                // Calculate data start position (after LOCA header)
                let data_start = loca_start + local_header.header_size();
                let data_end = data_start + entry.compressed_size as usize;
                payload[data_start..data_end].to_vec()
            }
            _ => {
                // Read from file (normal or per-file encrypted)
                // Seek to LOCA header
                self.file.seek(SeekFrom::Start(entry.data_offset))?;

                // Read and validate LOCA header
                let local_header = LocalEntryHeader::read_from(&mut self.file)?;

                // Validate LOCA header matches central directory
                self.validate_local_header(&local_header, &entry)?;

                // Read file data (file cursor is now positioned after LOCA header)
                let mut data = vec![0u8; entry.compressed_size as usize];
                self.file.read_exact(&mut data)?;
                data
            }
        };

        // Decrypt if per-file encryption
        let compressed_data = if self.encryption_mode == EncryptionMode::PerFile {
            self.decrypt_file_data(&raw_data)?
        } else {
            raw_data
        };

        // Decompress if needed
        // Check if file used frame-based compression (>= 50MB uncompressed)
        let decompressed = if should_use_frames(entry.uncompressed_size as usize)
            && entry.compression != CompressionMethod::None
        {
            // Use frame decompression for large files
            decompress_frames(&compressed_data, entry.compression, entry.uncompressed_size)?
        } else {
            // Regular decompression for files < 50MB
            match entry.compression {
                CompressionMethod::None => compressed_data,
                CompressionMethod::Lz4 => Self::decompress_lz4(&compressed_data, &entry)?,
                CompressionMethod::Zstd => Self::decompress_zstd(&compressed_data)?,
            }
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

    /// Read the Engram format manifest
    ///
    /// Returns the archive-level metadata from `manifest.json`.
    pub fn read_manifest(&mut self) -> Result<Option<serde_json::Value>> {
        if !self.contains("manifest.json") {
            return Ok(None);
        }

        let data = self.read_file("manifest.json")?;
        let manifest: serde_json::Value = serde_json::from_slice(&data)
            .map_err(|e| EngramError::InvalidManifest(format!("Invalid manifest.json: {}", e)))?;

        Ok(Some(manifest))
    }

    /// Read an application-specific manifest
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use engram_rs::ArchiveReader;
    /// # use engram_rs::error::Result;
    /// # fn main() -> Result<()> {
    /// let mut archive = ArchiveReader::open("backup.eng")?;
    /// archive.initialize()?;
    ///
    /// // Reads from "crisis-frame.json"
    /// let app_data: serde_json::Value = archive.read_app_manifest("crisis-frame")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_app_manifest(&mut self, app_name: &str) -> Result<serde_json::Value> {
        let path = format!("{}.json", app_name);
        let data = self.read_file(&path)?;
        serde_json::from_slice(&data)
            .map_err(|e| EngramError::InvalidManifest(format!("Invalid {}: {}", path, e)))
    }

    /// Check if an application manifest exists
    pub fn has_app_manifest(&self, app_name: &str) -> bool {
        self.contains(&format!("{}.json", app_name))
    }

    /// Extract all entries with a given prefix
    pub fn list_prefix(&self, prefix: &str) -> Vec<&String> {
        self.entry_list
            .iter()
            .filter(|path| path.starts_with(prefix))
            .collect()
    }

    /// Read and validate End Record (ENDR) from archive end
    fn validate_end_record(&mut self) -> Result<()> {
        // Seek to last 64 bytes (ENDR location)
        let file_size = self.file.metadata()?.len();
        if file_size < (END_RECORD_SIZE as u64) {
            return Err(EngramError::InvalidFormat(
                "Archive too small to contain ENDR record".to_string(),
            ));
        }

        let endr_offset = file_size - (END_RECORD_SIZE as u64);
        self.file.seek(SeekFrom::Start(endr_offset))?;

        // Read End Record
        let end_record = EndRecord::read_from(&mut self.file)?;

        // Validate against header
        end_record.validate_against_header(
            self.header.version_major,
            self.header.version_minor,
            self.header.central_directory_offset,
            self.header.central_directory_size,
            self.header.entry_count,
        )?;

        Ok(())
    }

    /// Validate Local Entry Header against Central Directory entry
    fn validate_local_header(&self, local: &LocalEntryHeader, central: &EntryInfo) -> Result<()> {
        // Verify path matches
        if local.path != central.path {
            return Err(EngramError::InvalidFormat(format!(
                "LOCA header path mismatch: expected '{}', found '{}'",
                central.path, local.path
            )));
        }

        // Verify sizes match
        if local.uncompressed_size != central.uncompressed_size {
            return Err(EngramError::InvalidFormat(format!(
                "LOCA header uncompressed_size mismatch for '{}': expected {}, found {}",
                central.path, central.uncompressed_size, local.uncompressed_size
            )));
        }

        if local.compressed_size != central.compressed_size {
            return Err(EngramError::InvalidFormat(format!(
                "LOCA header compressed_size mismatch for '{}': expected {}, found {}",
                central.path, central.compressed_size, local.compressed_size
            )));
        }

        // Verify CRC32 matches
        if local.crc32 != central.crc32 {
            return Err(EngramError::InvalidFormat(format!(
                "LOCA header CRC32 mismatch for '{}': expected 0x{:08X}, found 0x{:08X}",
                central.path, central.crc32, local.crc32
            )));
        }

        // Verify compression method matches
        if local.compression != central.compression {
            return Err(EngramError::InvalidFormat(format!(
                "LOCA header compression method mismatch for '{}': expected {:?}, found {:?}",
                central.path, central.compression, local.compression
            )));
        }

        Ok(())
    }

    /// Decrypt entire archive payload (archive-level encryption)
    fn decrypt_archive_payload(&mut self) -> Result<()> {
        let key = self.decryption_key.as_ref()
            .ok_or(EngramError::MissingDecryptionKey)?;

        // Calculate encrypted payload size (file - header - ENDR)
        let file_size = self.file.metadata()?.len();
        let encrypted_size = file_size - 64 - (END_RECORD_SIZE as u64);

        // Read encrypted payload: [nonce 12 bytes][ciphertext||tag]
        self.file.seek(SeekFrom::Start(64))?;  // After header

        // Read nonce
        let mut nonce_bytes = [0u8; 12];
        self.file.read_exact(&mut nonce_bytes)?;
        #[allow(deprecated)]
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Read ciphertext + tag (excluding ENDR at end)
        let ciphertext_size = encrypted_size - 12; // Subtract nonce size
        let mut ciphertext_with_tag = vec![0u8; ciphertext_size as usize];
        self.file.read_exact(&mut ciphertext_with_tag)?;

        // Decrypt
        let cipher = Aes256Gcm::new(key.into());
        let plaintext = cipher
            .decrypt(nonce, ciphertext_with_tag.as_ref())
            .map_err(|_| EngramError::DecryptionFailed)?;

        self.decrypted_payload = Some(plaintext);
        Ok(())
    }

    /// Decrypt file data for per-file encryption mode
    /// Input: [nonce 12 bytes][ciphertext||tag]
    /// Output: plaintext (compressed data)
    fn decrypt_file_data(&self, payload: &[u8]) -> Result<Vec<u8>> {
        if payload.len() < 28 {  // 12 nonce + 16 tag minimum
            return Err(EngramError::DecryptionFailed);
        }

        let key = self.decryption_key.as_ref()
            .ok_or(EngramError::MissingDecryptionKey)?;

        // Extract nonce (first 12 bytes)
        #[allow(deprecated)]
        let nonce = Nonce::from_slice(&payload[0..12]);

        // Rest is ciphertext + tag
        let ciphertext_with_tag = &payload[12..];

        // Decrypt
        let cipher = Aes256Gcm::new(key.into());
        cipher
            .decrypt(nonce, ciphertext_with_tag)
            .map_err(|_| EngramError::DecryptionFailed)
    }
}
