# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**engram-rs** is a unified Rust library implementing the Engram v1.0 archive format. It consolidates the previous `engram-core` + `engram-vfs` split into a single crate, providing compressed archives with cryptographic signatures, manifests, and embedded SQLite database access.

**Version:** 1.0.0 (production release)
**License:** MIT
**Repository:** https://github.com/blackfall-labs/engram-rs

## Development Commands

### Building & Testing

```bash
# Build library
cargo build

# Build release (optimized)
cargo build --release

# Run all tests (43 tests)
cargo test

# Run specific test
cargo test test_archive_roundtrip

# Run tests with output
cargo test -- --nocapture

# Run doc tests
cargo test --doc

# Build examples
cargo build --examples

# Run specific example
cargo run --example basic
cargo run --example compression
cargo run --example manifest
cargo run --example vfs
```

### Code Quality

```bash
# Format code (must pass before commit)
cargo fmt

# Check formatting
cargo fmt --check

# Lint with Clippy (must pass with no warnings)
cargo clippy --all-targets --all-features

# Fix clippy warnings automatically
cargo clippy --fix

# Check without building
cargo check
```

### Publishing

```bash
# Dry-run publish (verify package)
cargo publish --dry-run

# Actually publish to crates.io (requires CARGO_TOKEN)
cargo publish --token $CARGO_TOKEN

# Build docs locally
cargo doc --open
```

## Architecture Overview

### Module Structure

```
src/
├── lib.rs                    # Public API re-exports
├── error.rs                  # Unified EngramError enum (30+ variants)
├── archive/                  # Core archive I/O
│   ├── format.rs             # Binary format constants & compression selection
│   ├── reader.rs             # ArchiveReader - read .eng files
│   ├── writer.rs             # ArchiveWriter - create .eng files
│   ├── local_entry.rs        # LOCA (Local Entry) header handling
│   ├── end_record.rs         # ENDR (End Record) validation
│   └── frame_compression.rs  # Large file (≥50MB) frame-based compression
├── manifest.rs               # Manifest system with Ed25519 signatures
├── vfs.rs                    # VfsReader - SQLite database access
└── compat.rs                 # EngramVfs - backward compatibility wrapper
```

### Binary Format (Engram v1.0)

Four-part structure:

1. **File Header (64 bytes)** - PNG-style magic `0x89 'E' 'N' 'G' ...`, version, central directory offset
2. **File Data** - LOCA headers + compressed file data (allows streaming reads)
3. **Central Directory (320 bytes/entry)** - Fixed-size entries enable O(1) HashMap lookup
4. **End Record (64 bytes)** - ENDR signature, validation checksums

**Key Design Decisions:**
- Central directory at **end** of file → enables streaming creation without knowing all files upfront
- **Fixed 320-byte entries** → O(1) HashMap lookup, sub-millisecond random access
- **LOCA headers** → each file prefixed with local header for sequential streaming
- **ENDR record** → validates completeness, cross-checks header values

### Core Components

#### ArchiveWriter (Creating Archives)

**Workflow:**
```rust
let mut writer = ArchiveWriter::create("archive.eng")?;
writer.add_file("file.txt", data)?;              // Auto compression
writer.add_file_with_compression("data", data, CompressionMethod::Zstd)?;
writer.write_manifest(&manifest)?;                // Optional
writer.sign_manifest(&signing_key, "Author")?;    // Optional
writer.finalize()?;                               // REQUIRED - writes central directory & ENDR
```

**Compression Selection (Automatic):**
- Files < 4KB → `None`
- Already compressed (.png, .jpg, .zip) → `None`
- Text files (.txt, .json, .md) → `Zstd` (best ratio)
- Binary files (.db, .sqlite, .wasm) → `LZ4` (fastest)
- Default → `Zstd`

**CRITICAL:** Always call `finalize()` or archive will be incomplete!

#### ArchiveReader (Reading Archives)

**Workflow:**
```rust
let mut reader = ArchiveReader::open("archive.eng")?;
reader.initialize()?;                    // Parse central directory (auto in open)
let files = reader.list_files();         // Vec<String>
let data = reader.read_file("file.txt")?; // Decompresses automatically
```

**Data Structure:**
```rust
ArchiveReader {
    file: File,
    header: FileHeader,
    entries: HashMap<String, EntryInfo>,  // O(1) lookup
    entry_list: Vec<String>,               // Preserve order
    // ... encryption fields
}
```

**Path Normalization:** Windows `\` → `/` for cross-platform compatibility

#### Manifest System

**Purpose:** Archive metadata and cryptographic verification

**Philosophy:** Manifest reserved for **Engram format metadata only**. Application-specific metadata goes in separate files (e.g., `crisis-frame.json`). This allows multiple applications to coexist in one archive.

**Core Fields:**
```rust
Manifest {
    version: String,                 // Format version (0.4.0)
    id: String,                      // Unique archive ID
    name: Option<String>,            // Human-readable name
    author: Option<Author>,          // Creator info
    metadata: Option<Metadata>,      // Version, timestamps, license, tags
    capabilities: Vec<String>,       // ["read", "query", "execute"]
    files: Vec<FileEntry>,           // File inventory with SHA-256 hashes
    signatures: Vec<SignatureEntry>, // Ed25519 signatures
}
```

**Signing Workflow:**
```rust
// Create
let signing_key = SigningKey::generate(&mut OsRng);
writer.sign_manifest(&signing_key, "Author Name")?;

// Verify
let manifest: Manifest = serde_json::from_value(manifest_value)?;
let valid = manifest.verify_signatures(&verifying_key)?;
```

#### VfsReader (SQLite Database Access)

**Workflow:**
```rust
let mut vfs = VfsReader::open("archive.eng")?;
let conn = vfs.open_database("data.db")?;  // Extracts to temp file
let mut stmt = conn.prepare("SELECT * FROM users")?;
// ... use rusqlite normally
// Auto-cleanup when VfsReader dropped
```

**Performance:** 80-90% of native filesystem speed

### Encryption Support

**Two Modes:**

1. **Archive-Level (`EncryptionMode::Archive = 0b01`):**
   - Entire payload encrypted with AES-256-GCM
   - Central directory encrypted
   - Use case: Secure backups

2. **Per-File (`EncryptionMode::PerFile = 0b10`):**
   - Each file encrypted individually
   - Central directory in plaintext (file list visible)
   - Use case: Selective decryption, VFS queries on unencrypted DBs

### Frame-Based Compression

For files ≥ 50MB, data is split into 64KB frames:

```
[frame_count: u32]
[frame1_size: u32][frame1_data]
[frame2_size: u32][frame2_data]
...
```

**Benefits:**
- Independent frame decompression
- Partial reads without decompressing entire file
- Memory efficient for large files

Implemented in `src/archive/frame_compression.rs`

## Common Patterns

### Error Handling

All functions return `Result<T, EngramError>`:

```rust
pub type Result<T> = std::result::Result<T, EngramError>;

// Usage
reader.read_file(path).context("Failed to read file")?;
```

30+ error variants in `src/error.rs` with clear messages.

### Testing Patterns

**Unit Tests (in each file):**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_choice() {
        assert_eq!(
            CompressionMethod::choose_for_file("test.txt", 5000),
            CompressionMethod::Zstd
        );
    }
}
```

**Integration Tests (`tests/integration_test.rs`):**
```rust
#[test]
fn test_archive_roundtrip() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let archive_path = temp_dir.path().join("test.eng");

    // Create
    let mut writer = ArchiveWriter::create(&archive_path)?;
    writer.add_file("test.txt", b"content")?;
    writer.finalize()?;

    // Read
    let mut reader = ArchiveReader::open(&archive_path)?;
    let data = reader.read_file("test.txt")?;
    assert_eq!(data, b"content");

    Ok(())
}
```

### Version Compatibility

```rust
// Reject incompatible versions
if header.version_major > FORMAT_VERSION_MAJOR {
    return Err(EngramError::UnsupportedVersion(...));
}

// Warn on minor version mismatch
if header.version_minor > FORMAT_VERSION_MINOR {
    tracing::warn!("Archive uses newer minor version");
}
```

**Current:** v1.0
- Read support: v0.3, v0.4, v1.0
- Write support: v1.0 only

## Important Implementation Details

### Central Directory Entry Structure (320 bytes)

```rust
struct CentralDirectoryEntry {
    signature: [u8; 4],        // "CENT"
    version_made_by: u16,
    version_needed: u16,
    flags: u16,
    compression_method: u8,
    encryption_mode: u8,
    modified_time: u64,
    crc32: u32,
    compressed_size: u64,
    uncompressed_size: u64,
    offset: u64,
    path_length: u16,
    path: [u8; 256],           // UTF-8, null-terminated
    reserved: [u8; 16],
}
```

Fixed size enables O(1) HashMap lookup and sub-millisecond random access.

### Path Handling

**CRITICAL:** Always normalize paths to forward slashes:

```rust
// From utils (if available), or in writer/reader
fn normalize_path(path: &str) -> String {
    path.replace('\\', '/')
}

// When adding files
let normalized = normalize_path(relative_path);
writer.add_file(&normalized, data)?;
```

### LOCA (Local Entry) Headers

Each file in the data region is prefixed with a LOCA header (~40 bytes + path):

```
[signature: "LOCA" (4 bytes)]
[version: u16]
[flags: u16]
[compression: u8]
[encryption: u8]
[crc32: u32]
[compressed_size: u64]
[uncompressed_size: u64]
[path_length: u16]
[path: variable UTF-8]
```

Enables sequential streaming reads without seeking to central directory.

### ENDR (End Record) Validation

End of archive contains ENDR record to validate completeness:

```rust
// Validates:
- CD offset matches file position
- CD size matches actual size
- Entry count matches header
- Archive CRC32 checksum
```

Prevents corrupted/incomplete archives from being read.

## Migration from engram-core/engram-vfs

Old code using split crates:

```rust
// OLD
use engram_core::{ArchiveReader, ArchiveWriter};
use engram_vfs::VfsReader;
```

New unified API:

```rust
// NEW
use engram_rs::{ArchiveReader, ArchiveWriter, VfsReader};
```

Backward compatibility wrapper available:

```rust
// For legacy code
use engram_rs::EngramVfs; // Same API as old engram_vfs::EngramVfs
```

## Performance Characteristics

### Benchmarks (10MB file, Intel i7-12700K, NVMe SSD)

**Write:**
- Zstd: 105ms (95 MB/s), ratio: 3.8x
- LZ4: 26ms (380 MB/s), ratio: 2.1x
- None: 22ms (450 MB/s), ratio: 1.0x

**Read:**
- Zstd: 55ms (180 MB/s)
- LZ4: 24ms (420 MB/s)
- None: 20ms (500 MB/s)

**Memory (Central Directory):**
- 100MB archive: ~35KB
- 1GB archive: ~350KB
- 10GB archive: ~3.5MB

## Dependencies (11 total)

**Compression:** `lz4_flex` 0.11, `zstd` 0.13, `crc32fast` 1.4
**Database:** `rusqlite` 0.32 (bundled, backup features)
**Cryptography:** `ed25519-dalek` 2.1, `sha2` 0.10, `aes-gcm` 0.10, `pbkdf2` 0.12, `hex` 0.4, `rand` 0.8
**Serialization:** `serde` 1.0, `serde_json` 1.0, `toml` 0.8
**Error/Utils:** `thiserror` 1.0, `anyhow` 1.0, `tracing` 0.1, `tempfile` 3.12

All dependencies are public, stable crates from crates.io.

## Integration with engram-cli

The `engram-cli` binary uses this library for all operations:

| CLI Command | Component Used |
|-------------|----------------|
| `pack` | `ArchiveWriter` |
| `list` | `ArchiveReader::list_files()` |
| `info` | `ArchiveReader` + `Manifest` |
| `extract` | `ArchiveReader::read_file()` |
| `verify` | `Manifest::verify_signatures()` |
| `sign` | `writer.sign_manifest()` |
| `keygen` | `SigningKey::generate()` |
| `query` | `VfsReader::open_database()` |
| `search` | `ArchiveReader` + pattern matching |

See https://github.com/blackfall-labs/engram-cli for CLI implementation.

## Crates.io Publication

**Metadata (in Cargo.toml):**
```toml
name = "engram-rs"
version = "1.0.0"
edition = "2021"
license = "MIT"
description = "Unified engram archive library with manifest, signatures, and VFS support"
repository = "https://github.com/blackfall-labs/engram-rs"
homepage = "https://github.com/blackfall-labs/engram-rs"
documentation = "https://docs.rs/engram-rs"
readme = "README.md"
keywords = ["archive", "compression", "cryptography", "signature", "vfs"]
categories = ["compression", "cryptography", "database", "filesystem"]
```

**GitHub Actions:**
- `.github/workflows/ci.yml` - Tests on push/PR
- `.github/workflows/publish.yml` - Publishes to crates.io on tag push

**To publish a new version:**
1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Commit changes
4. Create git tag: `git tag v1.0.0`
5. Push tag: `git push origin v1.0.0`
6. GitHub Actions will automatically publish

## Documentation

**In-Repo:**
- `README.md` (10.9 KB) - Quick start, API overview, benchmarks
- `SPECIFICATION-FULL.md` (49.5 KB) - Complete binary format specification
- `CHANGELOG.md` - Version history
- `CONTRIBUTING.md` - Development guidelines

**Code Documentation:**
- Module-level `//!` docs in each file
- `///` doc comments on all public types
- Examples in doc comments (tested with `cargo test --doc`)

**Examples (`examples/` directory):**
- `basic.rs` - Create and read archives
- `compression.rs` - Compression options comparison
- `manifest.rs` - Metadata and Ed25519 signatures
- `vfs.rs` - SQLite database queries

## Related Projects

- **engram-cli:** Command-line tool (https://github.com/blackfall-labs/engram-cli)
- **engram-specification:** Format specification (https://github.com/blackfall-labs/engram-specification)

## Quick Reference

| Constant | Value |
|----------|-------|
| `FORMAT_VERSION_MAJOR` | 1 |
| `FORMAT_VERSION_MINOR` | 0 |
| `HEADER_SIZE` | 64 bytes |
| `CD_ENTRY_SIZE` | 320 bytes |
| `MAX_PATH_LENGTH` | 255 bytes |
| `FRAME_SIZE` | 65536 bytes (64 KB) |
| `LARGE_FILE_THRESHOLD` | 52428800 bytes (50 MB) |
| Magic Number | `0x89 'E' 'N' 'G' 0x0D 0x0A 0x1A 0x0A` |

## License

MIT License - See LICENSE file
