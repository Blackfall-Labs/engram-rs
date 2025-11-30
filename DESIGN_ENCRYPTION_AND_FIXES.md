# Engram Core: Encryption & Critical Fixes Design

**Branch:** `feature/encryption-and-fixes`
**Date:** 2025-11-30
**Target:** Engram v0.4 (Blackfall Implementation)

---

## Overview

This design implements three critical improvements to engram-core:

1. **AES-256-GCM Encryption** with header flags (None/Archive/PerFile modes)
2. **Path Separator Normalization** (cross-platform compatibility)
3. **Manifest Scope Documentation** (format vs application separation)

---

## 1. Encryption Implementation

### 1.1 Header Format Changes

**Current Header (64 bytes):**
```
Offset  Size  Field
0-7     8     Magic Number
8-9     2     Version Major
10-11   2     Version Minor
12-15   4     Header CRC32
16-23   8     Central Directory Offset
24-31   8     Central Directory Size
32-35   4     Entry Count
36-39   4     Content Version
40-63   24    Reserved (all zeros)
```

**New Header (64 bytes):**
```
Offset  Size  Field
0-7     8     Magic Number
8-9     2     Version Major (0)
10-11   2     Version Minor (4)  ← Bump to 0.4
12-15   4     Header CRC32
16-23   8     Central Directory Offset
24-31   8     Central Directory Size
32-35   4     Entry Count
36-39   4     Content Version
40-43   4     Flags               ← NEW
44-63   20    Reserved (zeros)    ← Was 24, now 20 due to flags
```

**Flags Field (4 bytes = 32 bits):**
```rust
// Bits 0-1: Encryption Mode
ENCRYPTION_NONE     = 0b00  // No encryption (default)
ENCRYPTION_ARCHIVE  = 0b01  // Entire archive encrypted after finalization
ENCRYPTION_PER_FILE = 0b10  // Each file encrypted individually
// 0b11 reserved

// Bits 2-31: Reserved for future features
// - Bit 2: Compression required flag
// - Bit 3: Signature required flag
// - Bits 4-31: Reserved (must be 0)

// Extract encryption mode
encryption_mode = flags & 0b00000011;
```

### 1.2 Encryption Modes

#### Mode 0: ENCRYPTION_NONE (default)
```
┌──────────────┐
│ Header       │
├──────────────┤
│ Files        │ ← Unencrypted
├──────────────┤
│ Central Dir  │ ← Unencrypted
└──────────────┘
```
- No encryption applied
- Backward compatible with v0.3
- For public archives, open data

#### Mode 1: ENCRYPTION_ARCHIVE (backups, secure storage)
```
┌──────────────┐
│ Header       │ ← Plaintext (readable)
├──────────────┤
│ Encrypted    │ ← AES-256-GCM wraps everything
│ ┌──────────┐ │   after header
│ │ Files    │ │
│ ├──────────┤ │
│ │ Cent Dir │ │
│ └──────────┘ │
└──────────────┘
```

**Structure:**
```
[Header 64 bytes - plaintext]
[Nonce 12 bytes]
[Ciphertext || Auth Tag - encrypted payload + 16-byte GCM tag]
```

**CRITICAL: Offset Semantics**
- `central_directory_offset` in header is **relative to the decrypted payload buffer**, NOT the on-disk file
- Reader logic:
  1. Read header
  2. Read `[nonce][ciphertext||tag]`
  3. Decrypt into `payload` buffer
  4. Interpret `central_directory_offset` inside `payload` (as if unencrypted archive)
- This keeps offset semantics consistent across encrypted/unencrypted modes

**Benefits:**
- Single encryption operation (fast)
- Smaller overhead (1 nonce + 1 tag)
- Perfect for backups (all-or-nothing access)
- Central directory encrypted (hides file list)

**Use Cases:**
- Crisis Frame backups
- Secure archives
- Encrypted state snapshots

#### Mode 2: ENCRYPTION_PER_FILE (queryable archives)
```
┌──────────────┐
│ Header       │ ← Plaintext
├──────────────┤
│ File 1       │ ← Encrypted with nonce₁
│ [Nonce]      │
│ [Cipher||Tag]│
├──────────────┤
│ File 2       │ ← Encrypted with nonce₂
│ [Nonce]      │
│ [Cipher||Tag]│
├──────────────┤
│ Central Dir  │ ← Plaintext (file list visible)
└──────────────┘
```

**Structure per file:**
```
[Nonce 12 bytes]
[Ciphertext || Auth Tag - encrypted compressed data + 16-byte GCM tag]
```

**CRITICAL: Compression Order**
- Per-file encryption encrypts **already-compressed** data
- Flow: plaintext → **compress** → **encrypt** → write [nonce][ciphertext||tag]
- Reader: read → **decrypt** → **decompress** → plaintext
- This ensures compression actually works (compressing ciphertext is useless)

**Benefits:**
- Decrypt individual files without full archive
- File list visible (queryable)
- Granular access control (different keys per file possible in future)

**Use Cases:**
- Encrypted document archives
- Queryable knowledge bases
- Partial decryption scenarios

### 1.3 Signatures and Encryption Order

**CRITICAL: Signature Computation Order**

In encrypted archives, signatures are computed over the **encrypted bytes** (what's actually on disk), not the plaintext:

- **Archive-encrypted mode:**
  - Sign: `[Header][Nonce][Ciphertext||Tag]`
  - This allows integrity and authenticity checks **before** decryption
  - Tampered ciphertext is detected without needing the encryption key

- **Per-file mode:**
  - Sign the manifest, which contains CRC32 hashes of the encrypted file payloads
  - Each file's signature covers `[Nonce][Ciphertext||Tag]`, not plaintext

**Benefits:**
- Detect tampering even without the decryption key
- Defense-in-depth: signature verification → decryption → CRC check
- Matches standard practice (sign what's on disk)

**Note:** Full signature implementation is tracked in `ISSUES.md` #3 (edge cases). This spec establishes the order invariant for when signatures are fully integrated.

### 1.4 Encryption API

#### Writer API
```rust
use aes_gcm::{Aes256Gcm, Key, Nonce};

impl ArchiveWriter {
    /// Enable archive-level encryption
    pub fn with_archive_encryption(mut self, key: &[u8; 32]) -> Self {
        self.encryption_mode = EncryptionMode::Archive;
        self.encryption_key = Some(key.clone());
        self
    }

    /// Enable per-file encryption
    pub fn with_per_file_encryption(mut self, key: &[u8; 32]) -> Self {
        self.encryption_mode = EncryptionMode::PerFile;
        self.encryption_key = Some(key.clone());
        self
    }

    /// Add encrypted file (per-file mode)
    pub fn add_file_encrypted(&mut self, path: &str, data: &[u8], compression: CompressionMethod) -> Result<()> {
        if self.encryption_mode != EncryptionMode::PerFile {
            return Err(EngramError::EncryptionMode);
        }

        // CRITICAL: Compress FIRST, then encrypt
        // 1. Compress the plaintext
        let compressed = self.compress(data, compression)?;

        // 2. Generate unique nonce for this file
        let nonce = Nonce::from_slice(&rand::random::<[u8; 12]>());

        // 3. Encrypt the compressed data
        let cipher = Aes256Gcm::new(Key::from_slice(&self.encryption_key.unwrap()));
        let ciphertext_with_tag = cipher.encrypt(nonce, compressed.as_ref())
            .map_err(|_| EngramError::EncryptionFailed)?;

        // 4. Store: [nonce][ciphertext||tag]
        let mut payload = Vec::new();
        payload.extend_from_slice(nonce.as_slice());
        payload.extend_from_slice(&ciphertext_with_tag);

        // 5. Add to archive (no further compression)
        self.add_file_raw(path, &payload)
    }

    /// Finalize with optional archive encryption
    pub fn finalize(mut self) -> Result<()> {
        // Write central directory as normal
        let cd_offset = self.current_offset;
        for entry in &self.entries {
            entry.write_to(&mut self.writer)?;
        }

        // If archive encryption mode, encrypt everything after header
        if self.encryption_mode == EncryptionMode::Archive {
            self.encrypt_archive_payload()?;
        }

        // Update header with flags
        self.write_final_header()
    }

    fn encrypt_archive_payload(&mut self) -> Result<()> {
        // Seek to position after header (byte 64)
        // Read all data from 64 to EOF
        // Encrypt with single nonce
        // Write back: [nonce][encrypted_payload][tag]
        // Update header offsets
    }
}
```

#### Reader API
```rust
impl ArchiveReader {
    /// Open archive (auto-detects encryption)
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(path)?;
        let header = FileHeader::read_from(&mut file)?;

        let encryption_mode = EncryptionMode::from_flags(header.flags);

        Ok(Self {
            file,
            header,
            entries: Vec::new(),
            encryption_mode,
            decryption_key: None,
        })
    }

    /// Provide decryption key
    pub fn with_decryption_key(mut self, key: &[u8; 32]) -> Self {
        self.decryption_key = Some(key.clone());
        self
    }

    /// Initialize (decrypt if needed)
    pub fn initialize(&mut self) -> Result<()> {
        match self.encryption_mode {
            EncryptionMode::None => {
                // Read central directory normally
                self.read_central_directory()
            }
            EncryptionMode::Archive => {
                // Decrypt entire payload first
                self.decrypt_archive_payload()?;
                self.read_central_directory()
            }
            EncryptionMode::PerFile => {
                // Just read central directory (not encrypted)
                self.read_central_directory()
            }
        }
    }

    /// Read file (auto-decrypts if needed)
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>> {
        let normalized_path = path.replace('\\', "/");

        let entry = self.find_entry(&normalized_path)?;

        // Seek to data
        self.file.seek(SeekFrom::Start(entry.data_offset))?;

        // Read compressed data
        let mut compressed = vec![0u8; entry.compressed_size as usize];
        self.file.read_exact(&mut compressed)?;

        // Decrypt if per-file mode
        let data = if self.encryption_mode == EncryptionMode::PerFile {
            self.decrypt_file_data(&compressed)?
        } else {
            compressed
        };

        // Decompress
        self.decompress(&data, entry.compression, entry.uncompressed_size)
    }

    fn decrypt_file_data(&self, payload: &[u8]) -> Result<Vec<u8>> {
        if payload.len() < 28 {  // 12 nonce + 16 tag minimum
            return Err(EngramError::EncryptionFailed);
        }

        let key = self.decryption_key.as_ref()
            .ok_or(EngramError::MissingDecryptionKey)?;

        // Extract nonce (first 12 bytes)
        let nonce = &payload[0..12];
        // Rest is encrypted data + tag
        let encrypted = &payload[12..];

        let cipher = Aes256Gcm::new(Key::from_slice(key));
        cipher.decrypt(Nonce::from_slice(nonce), encrypted)
            .map_err(|_| EngramError::DecryptionFailed)
    }
}
```

### 1.5 Key Derivation

For Crisis Frame backups:

```rust
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;

/// Derive encryption key from passphrase
pub fn derive_key(passphrase: &str, salt: &[u8; 32]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(
        passphrase.as_bytes(),
        salt,
        100_000,  // iterations
        &mut key
    );
    key
}

// Crisis Frame usage:
let salt = config.encryption.salt;  // Stored in config
let key = derive_key(&env::var("BACKUP_PASSPHRASE")?, &salt);
writer.with_archive_encryption(&key);
```

**Note on KDF Choice:**
- PBKDF2-HMAC-SHA256 with 100k iterations is chosen for:
  - Standard, widely available
  - Simple implementation
  - Acceptable for current threat model (offline backup storage)
- **Future hardening:** Upgrade to Argon2id or scrypt when needed
  - This is a pluggable choice, not a format lock-in
  - KDF parameters can be stored in manifest for forward compatibility
  - Not a blocker for current structural work

### 1.6 Dependencies

Add to `Cargo.toml`:
```toml
# Encryption
aes-gcm = "0.10"
pbkdf2 = { version = "0.12", features = ["simple"] }
rand = "0.8"  # Already present
```

---

## 2. Path Separator Normalization

### 2.1 Implementation

**Rule:** All paths stored in archives MUST use forward slashes (`/`), matching ZIP/TAR/JAR standards.

#### Writer Changes
```rust
/// Normalize path to forward slashes
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

impl ArchiveWriter {
    pub fn add_file(&mut self, path: &str, data: &[u8]) -> Result<()> {
        let normalized = normalize_path(path);
        // ... rest uses normalized
    }

    pub fn add_file_with_compression(&mut self, path: &str, ...) -> Result<()> {
        let normalized = normalize_path(path);
        // ... rest uses normalized
    }

    pub fn add_file_from_disk(&mut self, archive_path: &str, ...) -> Result<()> {
        let normalized = normalize_path(archive_path);
        // Path normalization happens here
    }
}
```

#### Reader Changes
```rust
impl ArchiveReader {
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>> {
        // Accept both forward and backward slashes for compatibility
        let normalized = normalize_path(path);

        // Try normalized path first
        if let Ok(entry) = self.find_entry(&normalized) {
            return self.read_entry(entry);
        }

        // Fallback: try original path for legacy archives
        if let Ok(entry) = self.find_entry(path) {
            return self.read_entry(entry);
        }

        Err(EngramError::FileNotFound(path.to_string()))
    }

    pub fn contains(&self, path: &str) -> bool {
        let normalized = normalize_path(path);
        self.entries.iter().any(|e| e.path == normalized || e.path == path)
    }
}
```

### 2.2 Testing

```rust
#[test]
fn test_cross_platform_paths() {
    let mut writer = ArchiveWriter::create("test.eng").unwrap();

    // Test Windows-style paths
    writer.add_file("users\\users.json", b"{}").unwrap();
    writer.add_file("data\\nested\\file.txt", b"test").unwrap();

    // Test Unix-style paths
    writer.add_file("config/app.toml", b"").unwrap();

    writer.finalize().unwrap();

    let mut reader = ArchiveReader::open("test.eng").unwrap();
    reader.initialize().unwrap();

    // Should read with forward slashes
    assert!(reader.contains("users/users.json"));
    assert!(reader.contains("data/nested/file.txt"));
    assert!(reader.contains("config/app.toml"));

    // Should also read with backward slashes (compatibility)
    assert!(reader.contains("users\\users.json"));
    assert!(reader.contains("data\\nested\\file.txt"));

    // Verify stored paths use forward slashes
    assert_eq!(reader.list_files()[0], "users/users.json");
    assert_eq!(reader.list_files()[1], "data/nested/file.txt");
}
```

---

## 3. Manifest Scope Documentation

### 3.1 Update Manifest Module Docs

```rust
//! Manifest support for Engram archives
//!
//! # Manifest Scope
//!
//! The Engram manifest (`manifest.json`) is **reserved for format-level metadata only**:
//! - Archive identification (name, version, description)
//! - File inventory with integrity hashes
//! - Digital signatures for verification
//! - Format capabilities and compression metadata
//!
//! **Applications must use separate files for application-specific metadata:**
//! - Recommended pattern: `<app-name>.json` (e.g., `crisis-frame.json`, `myapp.json`)
//! - Applications may store multiple metadata files as needed
//! - This allows multiple applications to coexist in one archive
//!
//! # Example Archive Structure
//!
//! ```text
//! archive.eng
//! ├── manifest.json           (Engram format metadata)
//! ├── crisis-frame.json       (Crisis Frame backup metadata)
//! ├── database/crisis.db      (Application data)
//! └── logs/frame.log
//! ```
//!
//! # Usage
//!
//! ```no_run
//! use engram_rs::{ArchiveWriter, Manifest};
//!
//! let mut writer = ArchiveWriter::create("backup.eng")?;
//!
//! // 1. Add Engram format manifest (reserved fields)
//! let manifest = Manifest::new(
//!     "backup-2025-11-30".to_string(),
//!     "Crisis Frame Backup".to_string(),
//!     Author::new("Crisis Frame System"),
//!     "1.0.0".to_string()
//! );
//! writer.add_manifest(&serde_json::to_value(&manifest)?)?;
//!
//! // 2. Add application-specific manifest (separate file)
//! let app_manifest = serde_json::json!({
//!     "services": ["database", "logs", "config"],
//!     "backup_type": "nightly",
//!     "timestamp": "2025-11-30T08:00:00Z"
//! });
//! writer.add_file("crisis-frame.json",
//!     serde_json::to_string_pretty(&app_manifest)?.as_bytes())?;
//!
//! // 3. Add application data
//! writer.add_file_from_disk("database/crisis.db", "path/to/crisis.db")?;
//! # Ok::<(), engram_rs::error::EngramError>(())
//! ```
```

### 3.2 Add Reader Convenience Methods

```rust
impl ArchiveReader {
    /// Read the Engram format manifest
    ///
    /// Returns the archive-level metadata from `manifest.json`.
    pub fn read_manifest(&mut self) -> Result<Manifest> {
        let json = self.read_file("manifest.json")?;
        Manifest::from_json(&json)
    }

    /// Read an application-specific manifest
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use engram_rs::ArchiveReader;
    /// let mut archive = ArchiveReader::open("backup.eng")?;
    /// archive.initialize()?;
    ///
    /// // Reads from "crisis-frame.json"
    /// let app_data: serde_json::Value = archive.read_app_manifest("crisis-frame")?;
    /// # Ok::<(), engram_rs::error::EngramError>(())
    /// ```
    pub fn read_app_manifest(&mut self, app_name: &str) -> Result<serde_json::Value> {
        let path = format!("{}.json", app_name);
        let json = self.read_file(&path)?;
        serde_json::from_slice(&json)
            .map_err(|e| EngramError::InvalidManifest(e.to_string()))
    }

    /// Check if an application manifest exists
    pub fn has_app_manifest(&self, app_name: &str) -> bool {
        self.contains(&format!("{}.json", app_name))
    }
}
```

---

## 4. Implementation Plan

### Phase 1: Core Changes (engram-core)

**Files to modify:**
1. `src/archive/format.rs`
   - Add `flags` field to `FileHeader`
   - Bump version to 0.4
   - Add `EncryptionMode` enum

2. `src/archive/writer.rs`
   - Add path normalization
   - Add encryption fields and methods
   - Implement `with_archive_encryption()` / `with_per_file_encryption()`
   - Update `finalize()` for archive encryption

3. `src/archive/reader.rs`
   - Add path normalization in lookups
   - Add decryption support
   - Implement `with_decryption_key()`
   - Auto-decrypt based on header flags

4. `src/manifest.rs`
   - Update module documentation
   - Add convenience methods to reader

5. `src/error.rs`
   - Add encryption-related errors

6. `Cargo.toml`
   - Add `aes-gcm`, `pbkdf2` dependencies

### Phase 2: Crisis Frame Integration

**Files to modify:**
1. `crisis-frame-backup/src/config.rs`
   - Add encryption config

2. `crisis-frame-backup/src/snapshot_scheduler.rs`
   - Use `with_archive_encryption()` when creating snapshots
   - Add manifest signing

3. `crisis-frame-backup/src/restore.rs`
   - Provide decryption key when opening archives

4. `crisis-frame-backup/src/signing.rs`
   - Integrate with manifest signing

### Phase 3: Testing

1. **Unit tests** (engram-core)
   - Path normalization (cross-platform)
   - Encryption modes (all 3)
   - Key derivation
   - Roundtrip encrypt/decrypt

2. **Integration tests** (crisis-frame-backup)
   - Full backup with encryption
   - Full restore with decryption
   - Signature verification
   - Cross-compatibility with Tower backups

---

## 5. Configuration

### Crisis Frame Config

```toml
# config/default.toml
[backup.encryption]
enabled = true
mode = "archive"  # or "per-file" or "none"
salt = "hex-encoded-32-bytes"  # Generated once, never changes

# Secret in .env
CRISIS_FRAME_BACKUP_PASSPHRASE=your-strong-passphrase-here
```

### Environment Variables

```bash
# .env
CRISIS_FRAME_BACKUP_PASSPHRASE=your-strong-passphrase-here
```

---

## 6. Migration Path

### Backward Compatibility

- **v0.3 archives** (no encryption): Continue to work, flags=0
- **v0.4 archives** (with encryption): Require decryption key
- **Reader**: Auto-detects version and encryption from header

### Version Bumps

- Engram-rs: `0.3.0` → `0.4.0`
- Crisis Frame Backup: Uses engram-rs `0.4.0`

---

## 7. Security Considerations

1. **Key Management**
   - Passphrase via environment variable (not in config)
   - Salt stored in config (public, unique per installation)
   - PBKDF2 with 100,000 iterations

2. **Encryption Strength**
   - AES-256-GCM (AEAD: authenticated encryption)
   - Unique nonce per file (per-file mode) or per archive
   - 128-bit authentication tag

3. **Threat Model**
   - Protects against: Unauthorized access to backup files
   - Does not protect against: Compromised backup system (key in memory)

---

## 8. Timeline

1. ✅ **Design Review** (this document)
2. **Phase 1:** Engram-core implementation (~2 hours)
3. **Phase 2:** Crisis Frame integration (~1 hour)
4. **Phase 3:** Testing (~1 hour)
5. **Total:** ~4 hours work

---

## 9. Success Criteria

- [ ] All 3 encryption modes work (None, Archive, PerFile)
- [ ] Path normalization works cross-platform
- [ ] Manifest scope documented and clear
- [ ] Crisis Frame creates encrypted backups with signing
- [ ] Crisis Frame restores encrypted backups successfully
- [ ] All tests pass
- [ ] No breaking changes for v0.3 archives

---

**Ready to implement?** 🚀
