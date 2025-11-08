# Claude Code Guidance for Engram Core

**Last Updated:** 2025-11-07
**Version:** v0.1.0
**Branch:** main

---

## Project Overview

**Engram Core** is the foundational Rust implementation of the Engram archive format. This repository provides the core engine and SQLite virtual filesystem that powers all language bindings, including `engram-nodejs`, Python bindings, and other ecosystem tools.

**Key Features:**
- Pure Rust implementation of the Engram format specification
- Archive reader/writer with compression pipeline
- Multi-compression support (LZ4, Zstd, Deflate)
- SQLite virtual filesystem (VFS) for zero-copy database access
- Manifest handling and validation
- CRC32 integrity verification
- Deterministic builds for reproducibility
- Platform-independent (Windows, macOS, Linux)

**Purpose:**
This crate is designed to be consumed as a library dependency by:
- Language bindings (FFI, Napi-rs, PyO3)
- CLI tools and utilities
- Desktop applications
- Server-side services
- Embedded systems

---

## Architecture

### Directory Structure

```
engram-core/
├── crates/
│   ├── engram-core/             # Core archive format
│   │   ├── src/
│   │   │   ├── lib.rs           # Public API
│   │   │   ├── format.rs        # Format specification
│   │   │   ├── reader.rs        # Archive reading
│   │   │   ├── writer.rs        # Archive writing
│   │   │   ├── compression.rs   # Compression pipeline
│   │   │   ├── error.rs         # Error types
│   │   │   └── manifest.rs      # Manifest handling
│   │   ├── tests/               # Unit + integration tests
│   │   └── Cargo.toml
│   │
│   └── engram-vfs/              # SQLite virtual filesystem
│       ├── src/
│       │   ├── lib.rs           # VFS implementation
│       │   ├── vfs.rs           # SQLite VFS adapter
│       │   ├── cache.rs         # Block caching
│       │   └── error.rs         # VFS errors
│       ├── tests/
│       └── Cargo.toml
│
├── docs/
│   ├── engram_architecture_clarification.md
│   ├── engram_compression_spec_v0.2.md
│   ├── API.md
│   └── GETTING_STARTED.md
│
├── Cargo.toml                   # Workspace configuration
├── CONTRIBUTING.md
└── README.md
```

### Core Components

**engram-core:**
- **ArchiveReader** - Opens and reads .eng files
- **ArchiveWriter** - Creates new archives
- **EntryInfo** - File metadata (size, CRC, compression)
- **CompressionMethod** - LZ4, Zstd, Deflate, None
- **Manifest** - Archive metadata and validation

**engram-vfs:**
- **EngramVfs** - SQLite VFS adapter
- **BlockCache** - LRU cache for decompressed blocks
- **VfsFile** - Virtual file handle for SQLite

### Dependency Graph

```
┌─────────────────────────────────────────────────┐
│ Language Bindings (engram-nodejs, etc.)         │
├─────────────────────────────────────────────────┤
│ engram-vfs                                      │
│  ├─ depends on: engram-core, rusqlite          │
│  └─ provides: SQLite VFS for archives           │
├─────────────────────────────────────────────────┤
│ engram-core                                     │
│  ├─ depends on: lz4_flex, zstd, crc32fast      │
│  └─ provides: Archive read/write, compression   │
└─────────────────────────────────────────────────┘
```

---

## Development Workflow

### Initial Setup

```bash
# Clone repository
git clone https://github.com/Manifest-Humanity/engram-core.git
cd engram-core

# Build workspace
cargo build

# Run tests
cargo test

# Generate documentation
cargo doc --open
```

### Development Commands

```bash
# Build entire workspace
cargo build                      # Debug mode
cargo build --release            # Release mode

# Build specific crate
cargo build -p engram-core
cargo build -p engram-vfs

# Run tests
cargo test                       # All tests
cargo test -p engram-core        # Core tests only
cargo test -p engram-vfs         # VFS tests only
cargo test --release             # Test optimized builds

# Run with logging
RUST_LOG=debug cargo test

# Check without building
cargo check

# Format code
cargo fmt --all

# Lint with Clippy
cargo clippy --all -- -D warnings

# Generate and view documentation
cargo doc --open
cargo doc -p engram-core --open

# Benchmarks (if available)
cargo bench
```

### Branch Naming Convention

```
main                             # Stable releases
feat/feature-name                # New features
fix/bug-name                     # Bug fixes
perf/optimization-name           # Performance improvements
docs/documentation-update        # Documentation
refactor/code-improvement        # Code refactoring
test/test-additions              # Test improvements
spec/format-change               # Format specification changes
```

### Commit Message Format

```
<type>(<scope>): <subject>

<body>

<footer>

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
```

**Types:** feat, fix, perf, docs, test, refactor, chore, spec
**Scopes:** core, vfs, format, compression, docs

**Example:**
```
feat(vfs): Add LRU block cache for database reads

- Implement BlockCache with configurable size limit
- Cache decompressed blocks by (file_id, block_offset)
- Automatic eviction using LRU policy
- Tests: 100% coverage, 12 new tests

Performance: 10x faster for repeated queries on same blocks

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
```

---

## Common Tasks

### Creating an Archive

```rust
use engram_core::{ArchiveWriter, CompressionMethod};
use std::path::Path;

fn create_archive() -> Result<(), Box<dyn std::error::Error>> {
    // Create new archive
    let mut writer = ArchiveWriter::create("output.eng")?;

    // Add files with automatic compression selection
    let readme = b"# My Archive\n\nHello world!";
    writer.add_file("README.md", readme)?;

    let config = b"[settings]\nkey = value";
    writer.add_file("config.toml", config)?;

    // Add file from disk
    writer.add_file_from_disk("docs/guide.md", Path::new("guide.md"))?;

    // Add file with specific compression
    let data = std::fs::read("large-dataset.json")?;
    writer.add_file_with_compression(
        "data/dataset.json",
        &data,
        CompressionMethod::Zstd
    )?;

    // Add manifest (stored uncompressed for fast access)
    let manifest = serde_json::json!({
        "name": "my-archive",
        "version": "1.0.0",
        "created": chrono::Utc::now().to_rfc3339()
    });
    writer.add_manifest(&manifest)?;

    // Finalize archive (writes central directory)
    writer.finalize()?;

    println!("Archive created: output.eng");
    Ok(())
}
```

### Reading from an Archive

```rust
use engram_core::ArchiveReader;

fn read_archive() -> Result<(), Box<dyn std::error::Error>> {
    // Open archive
    let mut reader = ArchiveReader::open("output.eng")?;

    // List all files
    println!("Files in archive:");
    for file in reader.list_files() {
        println!("  {}", file);
    }

    // Check if file exists
    if reader.contains("README.md") {
        // Get entry metadata
        let entry = reader.get_entry("README.md").unwrap();
        println!("\nREADME.md:");
        println!("  Size: {} bytes", entry.uncompressed_size);
        println!("  Compressed: {} bytes", entry.compressed_size);
        println!("  Compression: {:?}", entry.compression);
        println!("  CRC32: {:08x}", entry.crc32);

        // Read file data (automatically decompressed)
        let data = reader.read_file("README.md")?;
        let content = String::from_utf8(data)?;
        println!("  Content:\n{}", content);
    }

    // Read manifest
    if let Some(manifest) = reader.read_manifest()? {
        println!("\nManifest: {}", serde_json::to_string_pretty(&manifest)?);
    }

    Ok(())
}
```

### Using the SQLite VFS

```rust
use engram_vfs::EngramVfs;
use rusqlite::Connection;

fn query_database() -> Result<(), Box<dyn std::error::Error>> {
    // Create VFS for archive
    let vfs = EngramVfs::new("knowledge.eng")?;

    // Open database from archive
    let conn = vfs.open_database("data/knowledge.sqlite")?;

    // Query database
    let mut stmt = conn.prepare(
        "SELECT id, title, category FROM articles \
         WHERE category = ?1 \
         ORDER BY published_at DESC \
         LIMIT 10"
    )?;

    let rows = stmt.query_map(["technology"], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?
        ))
    })?;

    println!("Recent technology articles:");
    for row in rows {
        let (id, title, category) = row?;
        println!("  {}: {} ({})", id, title, category);
    }

    Ok(())
}
```

### In-Memory Database Loading

```rust
use engram_vfs::EngramVfs;

fn load_database_to_memory() -> Result<(), Box<dyn std::error::Error>> {
    let vfs = EngramVfs::new("knowledge.eng")?;

    // Load database entirely into memory (no temp file)
    let conn = vfs.open_database_in_memory("data/knowledge.sqlite")?;

    // Subsequent queries are faster (no disk I/O)
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM articles",
        [],
        |row| row.get(0)
    )?;

    println!("Total articles: {}", count);

    Ok(())
}
```

### Verifying Archive Integrity

```rust
use engram_core::ArchiveReader;

fn verify_archive() -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = ArchiveReader::open("output.eng")?;

    println!("Verifying archive integrity...");

    // Verify all files
    for file_path in reader.list_files() {
        let entry = reader.get_entry(file_path).unwrap();

        // read_file() automatically verifies CRC32
        match reader.read_file(file_path) {
            Ok(_data) => {
                println!("  ✓ {} (CRC: {:08x})", file_path, entry.crc32);
            }
            Err(e) => {
                println!("  ✗ {} - ERROR: {}", file_path, e);
            }
        }
    }

    println!("Verification complete!");
    Ok(())
}
```

### Custom Compression Selection

```rust
use engram_core::{ArchiveWriter, CompressionMethod};

fn smart_compression() -> Result<(), Box<dyn std::error::Error>> {
    let mut writer = ArchiveWriter::create("optimized.eng")?;

    // No compression for small files (overhead not worth it)
    let small_file = b"short";
    writer.add_file_with_compression(
        "small.txt",
        small_file,
        CompressionMethod::None
    )?;

    // LZ4 for fast decompression (frequently accessed)
    let hot_data = std::fs::read("hot-path.json")?;
    writer.add_file_with_compression(
        "cache/hot-path.json",
        &hot_data,
        CompressionMethod::Lz4
    )?;

    // Zstd for best ratio (large, infrequently accessed)
    let archive_data = std::fs::read("historical-logs.txt")?;
    writer.add_file_with_compression(
        "archive/logs.txt",
        &archive_data,
        CompressionMethod::Zstd
    )?;

    // Pre-compressed files (images, videos)
    let image = std::fs::read("photo.jpg")?;
    writer.add_file_with_compression(
        "media/photo.jpg",
        &image,
        CompressionMethod::None  // Already compressed
    )?;

    writer.finalize()?;
    Ok(())
}
```

---

## Integration with Language Bindings

### As a Library Dependency

**Cargo.toml:**
```toml
[dependencies]
engram-core = { git = "https://github.com/Manifest-Humanity/engram-core", branch = "main" }
engram-vfs = { git = "https://github.com/Manifest-Humanity/engram-core", branch = "main" }

# Or when published to crates.io
engram-core = "0.1"
engram-vfs = "0.1"
```

### FFI (Foreign Function Interface)

```rust
use engram_core::{ArchiveReader, ArchiveWriter};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn engram_open_archive(path: *const c_char) -> *mut ArchiveReader {
    let c_str = unsafe { CStr::from_ptr(path) };
    let path_str = c_str.to_str().unwrap();

    match ArchiveReader::open(path_str) {
        Ok(reader) => Box::into_raw(Box::new(reader)),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn engram_read_file(
    reader: *mut ArchiveReader,
    path: *const c_char,
    out_len: *mut usize
) -> *mut u8 {
    let reader = unsafe { &mut *reader };
    let c_str = unsafe { CStr::from_ptr(path) };
    let path_str = c_str.to_str().unwrap();

    match reader.read_file(path_str) {
        Ok(data) => {
            unsafe { *out_len = data.len(); }
            let ptr = data.as_ptr() as *mut u8;
            std::mem::forget(data);
            ptr
        }
        Err(_) => {
            unsafe { *out_len = 0; }
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn engram_close_archive(reader: *mut ArchiveReader) {
    if !reader.is_null() {
        unsafe { Box::from_raw(reader); }
    }
}
```

### Napi-rs (Node.js)

See `engram-nodejs` repository for complete integration example.

```rust
// crates/engram-napi/src/lib.rs
use napi::bindgen_prelude::*;
use engram_core::{ArchiveReader, ArchiveWriter};

#[napi]
pub struct JsEngramArchive {
    reader: ArchiveReader,
}

#[napi]
impl JsEngramArchive {
    #[napi(constructor)]
    pub fn new(path: String) -> Result<Self> {
        let reader = ArchiveReader::open(&path)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(Self { reader })
    }

    #[napi]
    pub fn list_files(&self) -> Vec<String> {
        self.reader.list_files()
    }

    #[napi]
    pub async fn read_file(&mut self, path: String) -> Result<Buffer> {
        let data = self.reader.read_file(&path)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(data.into())
    }
}
```

### PyO3 (Python)

```rust
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use engram_core::ArchiveReader;

#[pyclass]
struct EngramArchive {
    reader: ArchiveReader,
}

#[pymethods]
impl EngramArchive {
    #[new]
    fn new(path: String) -> PyResult<Self> {
        let reader = ArchiveReader::open(&path)
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
        Ok(Self { reader })
    }

    fn list_files(&self) -> Vec<String> {
        self.reader.list_files()
    }

    fn read_file<'py>(&mut self, py: Python<'py>, path: String) -> PyResult<&'py PyBytes> {
        let data = self.reader.read_file(&path)
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
        Ok(PyBytes::new(py, &data))
    }
}

#[pymodule]
fn engram_py(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<EngramArchive>()?;
    Ok(())
}
```

---

## Testing

### Test Structure

```
crates/engram-core/tests/
├── integration_test.rs          # End-to-end tests
├── compression_test.rs          # Compression tests
├── format_test.rs               # Format compliance tests
└── fixtures/                    # Test data

crates/engram-vfs/tests/
├── vfs_test.rs                  # VFS tests
├── cache_test.rs                # Cache tests
└── fixtures/
    └── test.sqlite
```

### Running Tests

```bash
# All tests
cargo test

# Specific crate
cargo test -p engram-core
cargo test -p engram-vfs

# Specific test
cargo test test_archive_roundtrip

# With output
cargo test -- --nocapture

# With logging
RUST_LOG=debug cargo test

# Release mode (optimized)
cargo test --release
```

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_archive_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.eng");

        // Write archive
        {
            let mut writer = ArchiveWriter::create(&path).unwrap();
            writer.add_file("test.txt", b"Hello, world!").unwrap();
            writer.add_manifest(&serde_json::json!({
                "name": "test",
                "version": "1.0.0"
            })).unwrap();
            writer.finalize().unwrap();
        }

        // Read archive
        {
            let mut reader = ArchiveReader::open(&path).unwrap();

            assert!(reader.contains("test.txt"));
            assert!(reader.contains("manifest.json"));

            let data = reader.read_file("test.txt").unwrap();
            assert_eq!(data, b"Hello, world!");

            let manifest = reader.read_manifest().unwrap().unwrap();
            assert_eq!(manifest["name"], "test");
        }
    }

    #[test]
    fn test_compression_methods() {
        let dir = tempdir().unwrap();
        let data = b"The quick brown fox jumps over the lazy dog".repeat(100);

        for method in [
            CompressionMethod::None,
            CompressionMethod::Lz4,
            CompressionMethod::Zstd,
        ] {
            let path = dir.path().join(format!("test-{:?}.eng", method));

            // Write
            let mut writer = ArchiveWriter::create(&path).unwrap();
            writer.add_file_with_compression("data.txt", &data, method).unwrap();
            writer.finalize().unwrap();

            // Read and verify
            let mut reader = ArchiveReader::open(&path).unwrap();
            let read_data = reader.read_file("data.txt").unwrap();

            assert_eq!(read_data, data);

            let entry = reader.get_entry("data.txt").unwrap();
            assert_eq!(entry.compression, method);
        }
    }

    #[test]
    fn test_vfs_sqlite_query() {
        use engram_vfs::EngramVfs;
        use rusqlite::Connection;

        // Create test archive with SQLite database
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("test.eng");
        let db_path = dir.path().join("test.sqlite");

        // Create test database
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)",
                []
            ).unwrap();
            conn.execute(
                "INSERT INTO users (name) VALUES (?1), (?2)",
                ["Alice", "Bob"]
            ).unwrap();
        }

        // Add to archive
        {
            let mut writer = ArchiveWriter::create(&archive_path).unwrap();
            writer.add_file_from_disk("data.sqlite", &db_path).unwrap();
            writer.finalize().unwrap();
        }

        // Query via VFS
        {
            let vfs = EngramVfs::new(&archive_path).unwrap();
            let conn = vfs.open_database("data.sqlite").unwrap();

            let mut stmt = conn.prepare("SELECT name FROM users ORDER BY id").unwrap();
            let names: Vec<String> = stmt.query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();

            assert_eq!(names, vec!["Alice", "Bob"]);
        }
    }
}
```

---

## Format Specification Compliance

### Format Version

Current: **v0.1**

Defined in `crates/engram-core/src/format.rs`:

```rust
pub const FORMAT_VERSION_MAJOR: u16 = 0;
pub const FORMAT_VERSION_MINOR: u16 = 1;
pub const MAGIC_NUMBER: [u8; 8] = [0x89, b'E', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
```

### Header Structure (64 bytes)

```rust
pub struct FileHeader {
    pub magic: [u8; 8],              // Magic number
    pub version_major: u16,          // Format version
    pub version_minor: u16,
    pub header_crc: u32,             // Header CRC32
    pub central_directory_offset: u64,
    pub central_directory_size: u64,
    pub entry_count: u32,
    pub content_version: u32,
    // 24 bytes reserved (must be zero)
}
```

### Central Directory Entry (320 bytes)

```rust
pub struct EntryInfo {
    pub path: String,               // Max 255 bytes UTF-8
    pub data_offset: u64,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub modified_time: u64,
    pub compression: CompressionMethod,
    pub flags: u8,
    // 20 bytes reserved
}
```

### Compression Methods

```rust
#[repr(u8)]
pub enum CompressionMethod {
    None = 0,      // No compression
    Lz4 = 1,       // LZ4 (fast)
    Zstd = 2,      // Zstandard (balanced)
    Deflate = 3,   // Deflate (future)
}
```

---

## Performance Optimization

### Compression Selection

```rust
// Automatic selection based on file size and type
impl ArchiveWriter {
    fn select_compression(data: &[u8], path: &str) -> CompressionMethod {
        // Small files: no compression (overhead not worth it)
        if data.len() < 4096 {
            return CompressionMethod::None;
        }

        // Pre-compressed formats
        let ext = Path::new(path).extension().and_then(|s| s.to_str());
        if matches!(ext, Some("jpg" | "png" | "mp4" | "zip" | "gz")) {
            return CompressionMethod::None;
        }

        // Large text/data: Zstd for better ratio
        if data.len() > 1_000_000 {
            return CompressionMethod::Zstd;
        }

        // Default: LZ4 for speed
        CompressionMethod::Lz4
    }
}
```

### VFS Caching Strategy

```rust
// LRU cache for decompressed blocks
pub struct BlockCache {
    cache: LruCache<(u64, u64), Vec<u8>>,  // (file_id, offset) -> data
    max_size: usize,
}

impl BlockCache {
    pub fn new(max_size_mb: usize) -> Self {
        let max_size = max_size_mb * 1024 * 1024;
        Self {
            cache: LruCache::new(max_size / 4096),  // Assume 4KB blocks
            max_size,
        }
    }

    pub fn get(&mut self, file_id: u64, offset: u64) -> Option<&[u8]> {
        self.cache.get(&(file_id, offset)).map(|v| v.as_slice())
    }

    pub fn insert(&mut self, file_id: u64, offset: u64, data: Vec<u8>) {
        while self.cache.len() * 4096 > self.max_size && self.cache.len() > 0 {
            self.cache.pop_lru();
        }
        self.cache.put((file_id, offset), data);
    }
}
```

---

## Code Style

### Rust Formatting

```bash
# Format all code
cargo fmt --all

# Check formatting
cargo fmt --all -- --check

# Clippy lints
cargo clippy --all -- -D warnings

# Clippy with pedantic
cargo clippy --all -- -W clippy::pedantic
```

### Documentation Standards

```rust
/// Brief description of the function.
///
/// More detailed explanation if needed.
///
/// # Arguments
///
/// * `path` - Archive file path
/// * `data` - File data to store
///
/// # Returns
///
/// Returns `Ok(())` on success
///
/// # Errors
///
/// Returns `EngramError::IoError` if file I/O fails
/// Returns `EngramError::PathTooLong` if path exceeds 255 bytes
///
/// # Examples
///
/// ```
/// use engram_core::ArchiveWriter;
///
/// let mut writer = ArchiveWriter::create("archive.eng")?;
/// writer.add_file("README.md", b"# Hello")?;
/// writer.finalize()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn add_file(&mut self, path: &str, data: &[u8]) -> Result<()> {
    // Implementation
}
```

---

## Publishing to crates.io

### Pre-publish Checklist

1. **Version bump** in `Cargo.toml` (both workspace and crate)
2. **Update CHANGELOG.md** with release notes
3. **Run full test suite** (`cargo test --release`)
4. **Check documentation** (`cargo doc --no-deps`)
5. **Verify examples** work with new version
6. **Update README.md** if API changed

### Publishing Process

```bash
# 1. Ensure everything is committed
git status

# 2. Dry run
cargo publish --dry-run -p engram-core
cargo publish --dry-run -p engram-vfs

# 3. Publish core first (vfs depends on it)
cargo publish -p engram-core

# 4. Wait for crates.io indexing (~1 minute)

# 5. Publish VFS
cargo publish -p engram-vfs

# 6. Tag release
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

---

## Troubleshooting

### Build Errors

```bash
# Clean build
cargo clean && cargo build

# Update dependencies
cargo update

# Check for outdated dependencies
cargo outdated
```

### Test Failures

```bash
# Run specific test with logging
RUST_LOG=debug cargo test test_name -- --nocapture

# Run tests serially (avoid parallelism issues)
cargo test -- --test-threads=1

# Run ignored tests
cargo test -- --ignored
```

### Performance Issues

```bash
# Profile with perf (Linux)
cargo build --release
perf record ./target/release/example
perf report

# Profile with Instruments (macOS)
cargo instruments --release --example example

# Benchmark
cargo bench
```

---

## Documentation

### Essential Reading

1. **README.md** - Project overview and quick start
2. **docs/engram_architecture_clarification.md** - Architecture details
3. **docs/engram_compression_spec_v0.2.md** - Compression specification
4. **docs/API.md** - API reference
5. **docs/GETTING_STARTED.md** - Tutorial for new users
6. **CONTRIBUTING.md** - Contribution guidelines

### API Documentation

```bash
# Generate documentation
cargo doc --no-deps

# Open in browser
cargo doc --no-deps --open

# Document private items (for development)
cargo doc --no-deps --document-private-items
```

---

## Resources

- **Main Repository:** https://github.com/Manifest-Humanity/engram-core
- **Specification:** https://github.com/Manifest-Humanity/engram-specification
- **Node.js Bindings:** https://github.com/Manifest-Humanity/engram-nodejs
- **Issues:** https://github.com/Manifest-Humanity/engram-core/issues

---

## Quick Reference

### Common Commands

```bash
cargo build                      # Build workspace
cargo test                       # Run all tests
cargo doc --open                 # View documentation
cargo fmt && cargo clippy        # Format and lint
```

### File Locations

```
engram-core/
├── crates/engram-core/          # Core archive implementation
├── crates/engram-vfs/           # SQLite VFS
├── docs/                        # Documentation
└── CONTRIBUTING.md              # How to contribute
```

### Basic API Usage

```rust
use engram_core::{ArchiveWriter, ArchiveReader};

// Write
let mut writer = ArchiveWriter::create("file.eng")?;
writer.add_file("README.md", b"# Hello")?;
writer.finalize()?;

// Read
let mut reader = ArchiveReader::open("file.eng")?;
let data = reader.read_file("README.md")?;
```

---

**For AI Assistants:** This is the foundational Rust crate for the Engram ecosystem. All format changes must maintain backward compatibility and update the specification. The crate is designed as a library, not a binary - users import it as a dependency. When making changes, always run the full test suite and update both inline docs and files in `docs/`. Format specification is defined in `format.rs` - changes there require careful review and spec updates.
