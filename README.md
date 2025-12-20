# engram-rs

[![Crates.io](https://img.shields.io/crates/v/engram-rs.svg)](https://crates.io/crates/engram-rs)
[![Documentation](https://docs.rs/engram-rs/badge.svg)](https://docs.rs/engram-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A unified Rust library for creating, reading, and managing **Engram archives** - compressed, cryptographically signed archive files with embedded metadata and SQLite database support.

## Features

- **📦 Compressed Archives**: LZ4 (fast) and Zstd (high compression ratio) with automatic format selection
- **🔐 Cryptographic Signatures**: Ed25519 signatures for authenticity and integrity verification
- **📋 Manifest System**: JSON-based metadata with file registry, author info, and capabilities
- **💾 Virtual File System (VFS)**: Direct SQL queries on embedded SQLite databases without extraction
- **⚡ Fast Lookups**: O(1) file access via central directory with 320-byte entries
- **✅ Integrity Verification**: CRC32 checksums for all files
- **🔒 Encryption Support**: AES-256-GCM encryption (per-file or full-archive)
- **🎯 Frame-based Compression**: Efficient handling of large files with incremental decompression

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
    // Open existing archive
    let mut reader = ArchiveReader::open("my_archive.eng")?;

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
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Generate a keypair for signing
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    // Create manifest
    let manifest = Manifest {
        version: "0.4.0".to_string(),
        id: "my-archive".to_string(),
        name: Some("My Archive".to_string()),
        description: Some("Example archive".to_string()),
        author: Some(Author {
            name: "John Doe".to_string(),
            email: Some("john@example.com".to_string()),
            url: None,
        }),
        ..Default::default()
    };

    // Create archive with manifest
    let mut writer = ArchiveWriter::create("signed_archive.eng")?;
    writer.add_file("data.txt", b"Important data")?;
    writer.write_manifest(&manifest)?;

    // Sign the manifest
    writer.sign_manifest(&signing_key, "John Doe")?;
    writer.finalize()?;

    // Later: verify the signature
    let mut reader = ArchiveReader::open("signed_archive.eng")?;
    if let Some(manifest_value) = reader.read_manifest()? {
        let manifest: Manifest = serde_json::from_value(manifest_value)?;
        let valid = manifest.verify_signatures(&verifying_key)?;
        println!("Signature valid: {}", valid);
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
│  - Entry 1 (320 bytes)                  │
│  - Entry 2 (320 bytes)                  │
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
- **Local Headers**: Frame-based compression for efficient large file handling
- **Central Directory**: Fast file lookup with O(1) access time
- **Manifest**: JSON metadata with Ed25519 signature support

## Compression

The library automatically selects compression based on file type and size:

| File Type | Size | Compression |
|-----------|------|-------------|
| Text files (.txt, .json, .md, etc.) | ≥ 4KB | **Zstd** (best ratio) |
| Binary files (.db, .wasm, etc.) | ≥ 4KB | **LZ4** (fastest) |
| Already compressed (.png, .jpg, .zip, etc.) | Any | **None** |
| Small files | < 4KB | **None** |

You can also manually specify compression:

```rust
writer.add_file_with_compression("data.bin", data, CompressionMethod::Zstd)?;
```

## Cryptography

### Signatures (Ed25519)

```rust
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;

// Generate keypair
let signing_key = SigningKey::generate(&mut OsRng);
let verifying_key = signing_key.verifying_key();

// Sign archive
writer.sign_manifest(&signing_key, "Author Name")?;

// Verify signature
let valid = manifest.verify_signatures(&verifying_key)?;
```

### Encryption (AES-256-GCM)

```rust
// Encrypt individual files
writer.add_encrypted_file("secret.txt", password, data)?;

// Decrypt when reading
let data = reader.read_encrypted_file("secret.txt", password)?;
```

## API Overview

### Core Types

- **`ArchiveWriter`** - Create and write to archives
- **`ArchiveReader`** - Read from existing archives
- **`VfsReader`** - Query SQLite databases in archives
- **`Manifest`** - Archive metadata and signatures
- **`CompressionMethod`** - Compression algorithm selection
- **`EngramError`** - Error types

### Main Operations

| Operation | Method |
|-----------|--------|
| Create archive | `ArchiveWriter::create(path)` |
| Open archive | `ArchiveReader::open(path)` |
| Add file | `writer.add_file(name, data)` |
| Read file | `reader.read_file(name)` |
| List files | `reader.list_files()` |
| Add manifest | `writer.write_manifest(manifest)` |
| Sign manifest | `writer.sign_manifest(key, signer)` |
| Query database | `vfs.open_database(name)` |

## Examples

See the [`examples/`](examples/) directory for complete examples:

- **`basic.rs`** - Creating and reading archives
- **`manifest.rs`** - Working with manifests and signatures
- **`compression.rs`** - Compression options
- **`vfs.rs`** - Querying embedded databases
- **`encryption.rs`** - Encrypting files

Run examples with:

```bash
cargo run --example basic
cargo run --example manifest
```

## Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_archive_roundtrip
```

**Test Coverage:**
- ✅ 22 unit tests (format, manifest, VFS)
- ✅ 9 integration tests (roundtrip, compression, large files)
- ✅ All tests passing

## Performance

Benchmarks on a test file (10MB):

| Compression | Write Speed | Read Speed | Ratio |
|-------------|-------------|------------|-------|
| None | 450 MB/s | 500 MB/s | 1.0x |
| LZ4 | 380 MB/s | 420 MB/s | 2.1x |
| Zstd | 95 MB/s | 180 MB/s | 3.8x |

*Benchmarked on Intel i7-12700K, NVMe SSD*

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

All functionality is now unified in a single crate with improved APIs.

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Licensed under the [MIT License](LICENSE).

## Related Projects

- **[engram-cli](https://github.com/blackfall-labs/engram-cli)** - Command-line tool for managing Engram archives
- **[engram-specification](https://github.com/blackfall-labs/engram-specification)** - Format specification

## Links

- **Crates.io**: https://crates.io/crates/engram-rs
- **Documentation**: https://docs.rs/engram-rs
- **Repository**: https://github.com/blackfall-labs/engram-rs
- **Issues**: https://github.com/blackfall-labs/engram-rs/issues
