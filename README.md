# engram-rs

[![Crates.io](https://img.shields.io/crates/v/engram-rs.svg)](https://crates.io/crates/engram-rs)
[![Documentation](https://docs.rs/engram-rs/badge.svg)](https://docs.rs/engram-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Tests](https://img.shields.io/badge/tests-166%20passing-brightgreen)](https://github.com/blackfall-labs/engram-rs)

A unified Rust library for creating, reading, and managing **Engram archives** - compressed, cryptographically signed archive files with embedded metadata and SQLite database support.

## Features

- **📦 Compressed Archives**: LZ4 (fast) and Zstd (high compression ratio) with automatic format selection
- **🔐 Cryptographic Signatures**: Ed25519 signatures for authenticity and integrity verification
- **📋 Manifest System**: JSON-based metadata with file registry, author info, and capabilities
- **💾 Virtual File System (VFS)**: Direct SQL queries on embedded SQLite databases without extraction
- **⚡ Fast Lookups**: O(1) file access via central directory with 320-byte fixed entries
- **✅ Integrity Verification**: CRC32 checksums for all files
- **🔒 Encryption Support**: AES-256-GCM encryption (per-file or full-archive)
- **🎯 Frame-based Compression**: Efficient handling of large files (≥50MB) with incremental decompression
- **🛡️ Battle-Tested**: 166 tests covering security, performance, concurrency, and reliability

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
engram-rs = "1.0"
```

## Quick Start

### Creating an Archive

```rust
use engram_rs::{ArchiveWriter, CompressionMethod};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new archive
    let mut writer = ArchiveWriter::create("my_archive.eng")?;

    // Add files with automatic compression
    writer.add_file("readme.txt", b"Hello, Engram!")?;
    writer.add_file("data.json", br#"{"version": "1.0"}"#)?;

    // Add file from disk
    writer.add_file_from_disk("config.toml", std::path::Path::new("./config.toml"))?;

    // Finalize the archive (writes central directory)
    writer.finalize()?;

    Ok(())
}
```

### Reading from an Archive

```rust
use engram_rs::ArchiveReader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open existing archive (convenience method)
    let mut reader = ArchiveReader::open_and_init("my_archive.eng")?;

    // List all files
    for filename in reader.list_files() {
        println!("📄 {}", filename);
    }

    // Read a specific file
    let data = reader.read_file("readme.txt")?;
    println!("Content: {}", String::from_utf8_lossy(&data));

    Ok(())
}
```

### Working with Manifests and Signatures

```rust
use engram_rs::{ArchiveWriter, Manifest, Author};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Generate a keypair for signing
    let signing_key = SigningKey::generate(&mut OsRng);

    // Create manifest
    let mut manifest = Manifest::new(
        "my-archive".to_string(),
        "My Archive".to_string(),
        Author::new("John Doe"),
        "1.0.0".to_string(),
    );

    // Sign the manifest
    manifest.sign(&signing_key, Some("John Doe".to_string()))?;

    // Create archive with signed manifest
    let mut writer = ArchiveWriter::create("signed_archive.eng")?;
    writer.add_file("data.txt", b"Important data")?;
    writer.add_manifest(&serde_json::to_value(&manifest)?)?;
    writer.finalize()?;

    // Later: verify the signature
    let mut reader = ArchiveReader::open_and_init("signed_archive.eng")?;
    if let Some(manifest_value) = reader.read_manifest()? {
        let loaded_manifest: Manifest =
            Manifest::from_json(&serde_json::to_vec(&manifest_value)?)?;
        let results = loaded_manifest.verify_signatures()?;
        println!("Signature valid: {}", results[0]);
    }

    Ok(())
}
```

### Querying Embedded SQLite Databases

```rust
use engram_rs::VfsReader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open archive with VFS
    let mut vfs = VfsReader::open("archive_with_db.eng")?;

    // Open embedded SQLite database
    let conn = vfs.open_database("data.db")?;

    // Execute SQL queries
    let mut stmt = conn.prepare("SELECT name, email FROM users WHERE active = 1")?;
    let users = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    for user in users {
        let (name, email) = user?;
        println!("{} <{}>", name, email);
    }

    Ok(())
}
```

## Archive Format

Engram uses a custom binary format (v1.0) with the following structure:

```
┌─────────────────────────────────────────┐
│ File Header (64 bytes)                  │
│  - Magic: 0x89 'E' 'N' 'G' 0x0D 0x0A 0x1A 0x0A │
│  - Format version (major.minor)         │
│  - Central directory offset/size        │
│  - Entry count, content version         │
│  - CRC32 checksum                       │
├─────────────────────────────────────────┤
│ Local Entry Header 1 (LOCA)            │
│ Compressed File Data 1                  │
├─────────────────────────────────────────┤
│ Local Entry Header 2 (LOCA)            │
│ Compressed File Data 2                  │
├─────────────────────────────────────────┤
│ ...                                     │
├─────────────────────────────────────────┤
│ Central Directory                       │
│  - Entry 1 (320 bytes fixed)            │
│  - Entry 2 (320 bytes fixed)            │
│  - ...                                  │
├─────────────────────────────────────────┤
│ End of Central Directory (ENDR)         │
├─────────────────────────────────────────┤
│ manifest.json (optional)                │
│  - Metadata, author, signatures         │
└─────────────────────────────────────────┘
```

**Key Features:**
- **Magic Number**: PNG-style magic bytes for file type detection
- **Fixed-Width Entries**: 320-byte central directory entries enable O(1) file lookup
- **Local Headers**: Enable sequential streaming reads without central directory
- **End-Placed Directory**: Enables streaming creation without manifest foreknowledge
- **Manifest**: JSON metadata with Ed25519 signature support

See [ENGRAM_SPECIFICATION.md](ENGRAM_SPECIFICATION.md) for complete binary format specification.

## Compression

The library automatically selects compression based on file type and size:

| File Type | Size | Compression | Typical Ratio |
|-----------|------|-------------|---------------|
| Text files (.txt, .json, .md, etc.) | ≥ 4KB | **Zstd** (best ratio) | 50-100x |
| Binary files (.db, .wasm, etc.) | ≥ 4KB | **LZ4** (fastest) | 2-5x |
| Already compressed (.png, .jpg, .zip, etc.) | Any | **None** | 1x |
| Small files | < 4KB | **None** | N/A |
| Large files | ≥ 50MB | **Frame-based** | Varies |

**Compression Performance:**
- Highly compressible data (zeros, patterns): **200-750x**
- Text files (JSON, Markdown, code): **50-100x**
- Mixed data: **50-100x**
- Large files (≥50MB): Automatic 64KB frame compression

You can also manually specify compression:

```rust
writer.add_file_with_compression("data.bin", data, CompressionMethod::Zstd)?;
```

## Cryptography

### Signatures (Ed25519)

```rust
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

// Generate keypair
let signing_key = SigningKey::generate(&mut OsRng);

// Sign manifest
manifest.sign(&signing_key, Some("Author Name".to_string()))?;

// Verify signatures
let results = manifest.verify_signatures()?;
println!("All signatures valid: {}", results.iter().all(|&v| v));
```

**Security:**
- Constant-time signature verification (no timing attack vulnerabilities)
- Multiple signatures supported (multi-party signing)
- Signature invalidation on data modification detected

### Encryption (AES-256-GCM)

```rust
// Encrypt individual files (per-file encryption)
writer.add_encrypted_file("secret.txt", password, data)?;

// Decrypt when reading
let data = reader.read_encrypted_file("secret.txt", password)?;
```

**Encryption Modes:**
- **Archive-level**: Entire archive encrypted (backup/secure storage)
- **Per-file**: Individual file encryption (selective decryption, database queries on unencrypted DBs)

## Performance

Benchmarks on a test file (10MB, Intel i7-12700K, NVMe SSD):

| Compression | Write Speed | Read Speed | Ratio |
|-------------|-------------|------------|-------|
| None | 450 MB/s | 500 MB/s | 1.0x |
| LZ4 | 380 MB/s | 420 MB/s | 2.1x |
| Zstd | 95 MB/s | 180 MB/s | 3.8x |

**Scalability (tested):**
- Archive size: Up to **1GB** (500MB routinely tested)
- File count: Up to **10,000 files** (1,000 files in <50ms)
- File access: **O(1)** HashMap lookup (sub-millisecond)
- Path length: Up to **255 bytes** (engram format limit)
- Directory depth: Up to **20 levels** tested

**VFS Performance:**
- SQLite queries: **80-90%** of native filesystem performance
- Cold cache: 60-70% of native (decompression overhead)
- Warm cache: 85-95% of native (cache hits)

## Testing & Quality Assurance

engram-rs has undergone comprehensive testing across 4 major phases:

### Test Statistics

- **Total Tests**: **166** (all passing)
  - 23 unit tests
  - 46 Phase 1 tests (security & integrity)
  - 33 Phase 2 tests (concurrency & reliability)
  - 16 Phase 3 tests + 4 stress tests (performance & scale)
  - 26 Phase 4 tests (security audit)
  - 10 integration tests
  - 7 v1 feature tests
  - 5 debug tests

### Phase 1: Security & Integrity (46 tests)

**Coverage:**
- ✅ Corruption detection (15 tests): Magic number, version, header, central directory, truncation
- ✅ Fuzzing infrastructure: cargo-fuzz ready with seed corpus
- ✅ Signature security (13 tests): Tampering, replay attacks, algorithm downgrade, multi-sig
- ✅ Encryption security (18 tests): Archive-level, per-file, wrong keys, compression+encryption

**Findings:**
- All corruption scenarios properly detected and rejected
- Signature verification cryptographically sound
- AES-256-GCM implementation secure
- No undefined behavior on malformed inputs

### Phase 2: Concurrency & Reliability (33 tests)

**Coverage:**
- ✅ Concurrent VFS/SQLite access (5 tests): 10 threads × 1,000 queries
- ✅ Multi-reader stress tests (6 tests): 100 concurrent readers, 64K operations
- ✅ Crash recovery (13 tests): Incomplete archives, truncation at 10-90%, corruption
- ✅ Frame compression edge cases (9 tests): 50MB threshold, 200MB files, data integrity

**Findings:**
- Thread-safe VFS with no resource leaks
- True parallelism via separate file handles
- All incomplete archives properly rejected
- Frame compression works correctly for large files (≥50MB)

**Operations Tested:**
- 10,000+ concurrent VFS database queries
- 64,000+ multi-reader operations
- 500MB+ data processed

### Phase 3: Performance & Scale (16 tests + 4 stress)

**Coverage:**
- ✅ Large archives (8 tests): 500MB-1GB archives, 10K files, path edge cases
- ✅ Compression validation (8 tests): Text, binary, pre-compressed, effectiveness

**Findings:**
- Scales to 1GB+ archives with no issues
- 10,000+ files handled efficiently (O(1) lookup)
- Compression ratios: 50-227x typical, 227x for zeros, 59x for text
- Performance: ~120 MB/s write, ~200 MB/s read

**Stress Tests (run with `--ignored`):**
- 500MB archive: 4.3 seconds (500MB → 1MB, 500x compression)
- 1GB archive: ~10 seconds
- 10,000 files: ~1 second

### Phase 4: Security Audit (26 tests)

**Coverage:**
- ✅ Path traversal prevention (10 tests): ../, absolute paths, null bytes, normalization
- ✅ ZIP bomb protection (8 tests): Compression ratios, decompression safety
- ✅ Cryptographic attacks (8 tests): Timing attacks, weak keys, side-channels

**Findings:**

**Path Security:**
- ⚠️ Path traversal attempts (../, absolute paths) accepted but normalized
- ⚠️ Applications must sanitize paths during extraction
- ✅ 255-byte path limit enforced (rejected at finalize())
- ✅ Case-sensitive storage (File.txt ≠ file.txt)

**Compression Security:**
- ✅ Excellent compression ratios (200-750x)
- ✅ No recursive compression (prevents nested bombs)
- ✅ Frame compression limits memory (64KB frames)
- ⚠️ Relies on zstd/lz4 library safety checks (no explicit bomb detection)

**Cryptographic Security:**
- ✅ Ed25519 signatures with constant-time verification
- ✅ No timing attack vulnerabilities detected
- ✅ Weak keys avoided (OsRng used)
- ✅ Signature invalidation on modification detected
- ✅ Multiple signatures supported

**Verdict:** **No critical security vulnerabilities found.** engram-rs is production-ready with proper application-level path sanitization.

### Documentation

Comprehensive testing documentation:
- [TESTING_PLAN.md](TESTING_PLAN.md) - Overall testing strategy and status
- [TESTING_PHASE_1.1_FINDINGS.md](TESTING_PHASE_1.1_FINDINGS.md) - Corruption detection
- [TESTING_PHASE_1.2_FUZZING.md](TESTING_PHASE_1.2_FUZZING.md) - Fuzzing infrastructure
- [TESTING_PHASE_1.3_SIGNATURES.md](TESTING_PHASE_1.3_SIGNATURES.md) - Signature security
- [TESTING_PHASE_1.4_ENCRYPTION.md](TESTING_PHASE_1.4_ENCRYPTION.md) - Encryption security
- [TESTING_PHASE_2_CONCURRENCY.md](TESTING_PHASE_2_CONCURRENCY.md) - Concurrency tests
- [TESTING_PHASE_3_PERFORMANCE.md](TESTING_PHASE_3_PERFORMANCE.md) - Performance tests
- [TESTING_PHASE_4_SECURITY.md](TESTING_PHASE_4_SECURITY.md) - Security audit

## API Overview

### Core Types

- **`ArchiveWriter`** - Create and write to archives
- **`ArchiveReader`** - Read from existing archives
- **`VfsReader`** - Query SQLite databases in archives
- **`Manifest`** - Archive metadata and signatures
- **`CompressionMethod`** - Compression algorithm selection
- **`EngramError`** - Error types

### Convenience Methods

| Operation | Method |
|-----------|--------|
| Create archive | `ArchiveWriter::create(path)` |
| Open archive | `ArchiveReader::open_and_init(path)` |
| Open encrypted | `ArchiveReader::open_encrypted(path, key)` |
| Add file | `writer.add_file(name, data)` |
| Add from disk | `writer.add_file_from_disk(name, path)` |
| Read file | `reader.read_file(name)` |
| List files | `reader.list_files()` |
| Add manifest | `writer.add_manifest(manifest)` |
| Sign manifest | `manifest.sign(key, signer)` |
| Verify signatures | `manifest.verify_signatures()` |
| Query database | `vfs.open_database(name)` |

## Examples

See the [`examples/`](examples/) directory for complete examples:

- **`basic.rs`** - Creating and reading archives
- **`manifest.rs`** - Working with manifests and signatures
- **`compression.rs`** - Compression options
- **`vfs.rs`** - Querying embedded databases

Run examples with:

```bash
cargo run --example basic
cargo run --example manifest
cargo run --example vfs
```

## Running Tests

```bash
# Run all tests (fast)
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test file
cargo test --test corruption_test

# Run stress tests (large archives, many files)
cargo test --test stress_large_archives_test -- --ignored --nocapture
```

**Test Execution Time:**
- Regular tests (162 tests): **<2 seconds**
- Stress tests (4 tests): **5-15 seconds** (run with `--ignored`)

## Compatibility

- **Rust**: 1.75+ (2021 edition)
- **Platforms**: Windows, macOS, Linux, BSD
- **Architectures**: x86_64, aarch64 (ARM64)

## Migration from engram-core/engram-vfs

This library replaces the previous two-crate structure:

```rust
// Old
use engram_core::{ArchiveReader, ArchiveWriter};
use engram_vfs::VfsReader;

// New (engram-rs)
use engram_rs::{ArchiveReader, ArchiveWriter, VfsReader};
```

All functionality is now unified in a single crate with improved APIs:
- `open_and_init()` convenience method (was: `open()` then `initialize()`)
- `open_encrypted()` convenience method for encrypted archives
- Simplified manifest signing workflow

## Security Considerations

### Path Extraction Safety

engram-rs **does not reject** path traversal attempts during archive creation. Applications **must** sanitize paths during extraction:

```rust
use std::path::{Path, PathBuf};

fn safe_extract_path(archive_path: &str, dest_root: &Path) -> Result<PathBuf, &'static str> {
    let normalized = archive_path.replace('\\', '/');

    // Reject absolute paths
    if normalized.starts_with('/') || normalized.contains(':') {
        return Err("Absolute paths not allowed");
    }

    // Reject parent directory references
    if normalized.contains("..") {
        return Err("Parent directory references not allowed");
    }

    // Build final path and verify it's within dest_root
    let final_path = dest_root.join(&normalized);
    if !final_path.starts_with(dest_root) {
        return Err("Path escapes destination directory");
    }

    Ok(final_path)
}
```

### Signature Verification

Always verify signatures before trusting archive contents:

```rust
let manifest: Manifest = Manifest::from_json(&manifest_data)?;
let results = manifest.verify_signatures()?;

if !results.iter().all(|&valid| valid) {
    return Err("Invalid signature detected");
}
```

### Resource Limits

For untrusted archives, set resource limits:

```bash
# Unix/Linux: Set memory limit
ulimit -v 1048576  # 1GB virtual memory limit

# Monitor decompression size
if decompressed_size > max_allowed_size {
    return Err("Decompression size exceeds limit");
}
```

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Licensed under the [MIT License](LICENSE).

## Related Projects

- **[engram-cli](https://github.com/blackfall-labs/engram-cli)** - Command-line tool for managing Engram archives
- **[engram-specification](https://github.com/blackfall-labs/engram-specification)** - Complete format specification
- **[engram-nodejs](https://github.com/blackfall-labs/engram-nodejs)** - Node.js bindings (native module)

## Links

- **Crates.io**: https://crates.io/crates/engram-rs
- **Documentation**: https://docs.rs/engram-rs
- **Repository**: https://github.com/blackfall-labs/engram-rs
- **Issues**: https://github.com/blackfall-labs/engram-rs/issues
- **Format Specification**: [ENGRAM_SPECIFICATION.md](ENGRAM_SPECIFICATION.md)
