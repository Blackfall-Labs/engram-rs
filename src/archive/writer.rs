use crate::archive::end_record::EndRecord;
use crate::archive::format::{
    CompressionMethod, EncryptionMode, EntryInfo, FileHeader, FORMAT_VERSION_MAJOR,
    FORMAT_VERSION_MINOR,
};
use crate::archive::frame_compression::{compress_frames, should_use_frames};
use crate::archive::local_entry::LocalEntryHeader;
use crate::error::{EngramError, Result};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Threshold below which files are not compressed (4KB)
const MIN_COMPRESSION_SIZE: usize = 4096;

/// Normalize path to forward slashes (cross-platform compatibility)
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// Archive writer for creating .eng files
pub struct ArchiveWriter {
    writer: BufWriter<File>,
    entries: Vec<EntryInfo>,
    current_offset: u64,
    encryption_mode: EncryptionMode,
    encryption_key: Option<[u8; 32]>,
}

impl ArchiveWriter {
    /// Create a new archive file
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self> {
        // Open with read+write for encryption support (need to read back for archive encryption)
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        let mut writer = BufWriter::new(file);

        // Write placeholder header (will be updated at finalization)
        let header = FileHeader::new();
        header.write_to(&mut writer)?;

        Ok(Self {
            writer,
            entries: Vec::new(),
            current_offset: 64, // After header
            encryption_mode: EncryptionMode::None,
            encryption_key: None,
        })
    }

    /// Enable archive-level encryption (entire archive encrypted after finalization)
    pub fn with_archive_encryption(mut self, key: &[u8; 32]) -> Self {
        self.encryption_mode = EncryptionMode::Archive;
        self.encryption_key = Some(*key);
        self
    }

    /// Enable per-file encryption (each file encrypted individually)
    pub fn with_per_file_encryption(mut self, key: &[u8; 32]) -> Self {
        self.encryption_mode = EncryptionMode::PerFile;
        self.encryption_key = Some(*key);
        self
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
        // Normalize path (cross-platform: always use forward slashes)
        let normalized_path = normalize_path(path);

        // CRITICAL: Compress FIRST, then encrypt (if per-file mode)
        let (compressed_data, actual_compression) = self.compress_data(data, compression)?;

        // Prepare final payload (encrypted if per-file mode)
        let final_payload = if self.encryption_mode == EncryptionMode::PerFile {
            self.encrypt_file_data(&compressed_data)?
        } else {
            compressed_data
        };

        // Calculate CRC32 of uncompressed data
        let crc32 = crc32fast::hash(data);

        // Get current timestamp
        let modified_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Record offset to LOCAL ENTRY HEADER (v1.0 format)
        let entry_start_offset = self.current_offset;

        // Create and write Local Entry Header (LOCA)
        let local_header = LocalEntryHeader::new(
            data.len() as u64,          // uncompressed_size
            final_payload.len() as u64, // compressed_size
            crc32,
            modified_time,
            actual_compression,
            normalized_path.clone(),
        );

        let header_bytes_written = local_header.write_to(&mut self.writer)?;
        self.current_offset += header_bytes_written as u64;

        // Write file data after LOCA header
        self.writer.write_all(&final_payload)?;
        self.current_offset += final_payload.len() as u64;

        // Create central directory entry (data_offset points to LOCA header)
        let entry = EntryInfo {
            path: normalized_path,
            data_offset: entry_start_offset, // Points to LOCA header
            uncompressed_size: data.len() as u64,
            compressed_size: final_payload.len() as u64,
            crc32,
            modified_time,
            compression: actual_compression,
            flags: 0,
        };

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
        let json = serde_json::to_vec_pretty(manifest).map_err(|e| {
            EngramError::InvalidManifest(format!("Failed to serialize manifest: {}", e))
        })?;

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

        // Flush writer before getting inner file
        self.writer.flush()?;

        // Capture needed values before moving writer
        let encryption_mode = self.encryption_mode;
        let encryption_key = self.encryption_key;
        let entry_count = self.entries.len() as u32;

        // Get inner file for encryption and header writing
        let mut file = self.writer.into_inner().map_err(|e| e.into_error())?;

        // Handle archive-level encryption
        if encryption_mode == EncryptionMode::Archive {
            Self::encrypt_archive_payload_static(
                &mut file,
                &encryption_key.ok_or(EngramError::InvalidEncryptionMode)?,
            )?;
        }

        // Write final header with encryption flags
        file.seek(SeekFrom::Start(0))?;
        let mut header = FileHeader::new();
        header.central_directory_offset = cd_offset;
        header.central_directory_size = cd_size;
        header.entry_count = entry_count;
        header.set_encryption_mode(encryption_mode);
        header.write_to(&mut file)?;

        // Write End Record (ENDR) at end of archive (v1.0)
        file.seek(SeekFrom::End(0))?;
        let end_record = EndRecord::new(
            FORMAT_VERSION_MAJOR,
            FORMAT_VERSION_MINOR,
            cd_offset,
            cd_size,
            entry_count,
            0, // archive_crc32 - TODO: calculate full archive checksum
        );
        end_record.write_to(&mut file)?;

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
        // Check if file should use frame-based compression (>= 50MB)
        if should_use_frames(data.len()) {
            match compression {
                CompressionMethod::None => return Ok((data.to_vec(), CompressionMethod::None)),
                CompressionMethod::Lz4 | CompressionMethod::Zstd => {
                    // Use frame-based compression for large files
                    let compressed = compress_frames(data, compression)?;
                    // Frame compression is always beneficial for large files
                    return Ok((compressed, compression));
                }
            }
        }

        // Regular compression for files < 50MB
        let compressed = match compression {
            CompressionMethod::None => return Ok((data.to_vec(), CompressionMethod::None)),
            CompressionMethod::Lz4 => Self::compress_lz4(data)?,
            CompressionMethod::Zstd => Self::compress_zstd(data)?,
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
            .map_err(|e| EngramError::CompressionFailed(format!("Zstd compression failed: {}", e)))
    }

    /// Encrypt file data for per-file encryption mode
    /// Returns: [nonce 12 bytes][ciphertext||tag]
    fn encrypt_file_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        let key = self
            .encryption_key
            .as_ref()
            .ok_or(EngramError::InvalidEncryptionMode)?;

        // Generate unique nonce for this file
        let nonce_bytes: [u8; 12] = rand::random();
        #[allow(deprecated)]
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt compressed data
        let cipher = Aes256Gcm::new(key.into());
        let ciphertext_with_tag = cipher
            .encrypt(nonce, data)
            .map_err(|_| EngramError::EncryptionFailed)?;

        // Build payload: [nonce][ciphertext||tag]
        let mut payload = Vec::with_capacity(12 + ciphertext_with_tag.len());
        payload.extend_from_slice(&nonce_bytes);
        payload.extend_from_slice(&ciphertext_with_tag);

        Ok(payload)
    }

    /// Encrypt entire archive payload (archive-level encryption)
    /// Reads everything after header, encrypts it, writes back
    ///
    /// This is a static method to avoid borrowing issues with BufWriter
    fn encrypt_archive_payload_static(file: &mut File, key: &[u8; 32]) -> Result<()> {
        // Read everything after header (from byte 64 to EOF)
        file.seek(SeekFrom::Start(64))?;
        let mut payload = Vec::new();
        std::io::Read::read_to_end(file, &mut payload)?;

        // Generate nonce for archive encryption
        let nonce_bytes: [u8; 12] = rand::random();
        #[allow(deprecated)]
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt the entire payload
        let cipher = Aes256Gcm::new(key.into());
        let ciphertext_with_tag = cipher
            .encrypt(nonce, payload.as_ref())
            .map_err(|_| EngramError::EncryptionFailed)?;

        // Seek back to position 64 (after header)
        file.seek(SeekFrom::Start(64))?;

        // Write: [nonce][ciphertext||tag]
        file.write_all(&nonce_bytes)?;
        file.write_all(&ciphertext_with_tag)?;

        // Truncate file to remove any leftover data
        let new_size = 64 + 12 + ciphertext_with_tag.len() as u64;
        file.set_len(new_size)?;
        file.flush()?;

        // Note: Central directory offsets in header are payload-relative (will be interpreted after decryption)
        Ok(())
    }
}
