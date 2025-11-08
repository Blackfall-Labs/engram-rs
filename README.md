# engram-rs

**Unified engram archive library with manifest, signatures, and VFS support**

A Rust library for creating, reading, and managing engram archives - compressed, signed archive files with embedded metadata and SQLite databases.

## Features

- **Compressed Archives**: LZ4 and Zstd compression with automatic selection
- **Cryptographic Signatures**: Ed25519 signatures for authenticity verification
- **Manifest System**: JSON-based metadata with file registry and capabilities
- **Virtual File System (VFS)**: Access embedded SQLite databases within archives
- **Fast Lookups**: O(1) file access via central directory
- **Integrity Verification**: CRC32 checksums for all files

## Archive Format

Engram v0.3 format:
- Magic number: `0x89 'E' 'N' 'G' 0x0D 0x0A 0x1A 0x0A` (PNG-style)
- 64-byte header with version, central directory offset, entry count
- Compressed file data (LZ4/Zstd/None)
- Central directory with 320-byte entries
- Optional manifest.json with Ed25519 signatures

## Usage

See [CLAUDE.md](CLAUDE.md) for comprehensive development guidance.

## Testing

```bash
cargo test
```

**Test Coverage:**
- 12 unit tests (format, manifest, VFS)
- 9 integration tests (roundtrip, compression, large files)
- All 22 tests passing ✅

## License

MIT OR Apache-2.0

## Repository

https://github.com/Manifest-Humanity/engram-rs

## Version

v0.3.0 - Unified library combining core archive functionality, manifest support, and VFS access.

## Migration from engram-core/engram-vfs

This library replaces the previous two-crate structure:
- `engram-core` → `engram-rs` (core archive functionality)
- `engram-vfs` → `engram-rs::vfs` module

All functionality is now unified in a single crate with improved APIs and additional features (manifest, signatures).
