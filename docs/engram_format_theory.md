# Building a Custom Archive Format with SQLite VFS Integration

Your .eng archive format combining virtual filesystem access with embedded SQLite querying is highly achievable using modern Rust tooling. **The recommended approach uses NAPI-RS for Node.js bindings, sqlite-vfs crate for VFS implementation, and a ZIP-inspired format with TOC-at-end for efficient random access**. This architecture will deliver 2-3x better performance than WASM alternatives while maintaining the no-extraction requirement critical for Electron applications.

## Core architecture recommendation

The optimal implementation combines three proven patterns. First, use **sqlite-vfs** as your Rust VFS foundation—this trait-based library passes most of SQLite's official test harness and provides clean abstractions for implementing custom storage backends. Second, adopt an **ASAR-inspired archive structure** with a crucial modification: place the table of contents at file end (like ZIP) rather than beginning, enabling streaming creation and easy updates without full rewrites. Third, expose everything to Node.js/Electron via **NAPI-RS native bindings** rather than WASM, gaining 1.75-2.5x performance improvements for data-intensive operations plus zero-copy buffer sharing—critical when repeatedly accessing SQLite databases and JSON manifests.

The complete data flow works like this: JavaScript requests a file or database query → NAPI-RS boundary (near-zero overhead) → Rust reads TOC index from .eng file → hash table lookup (O(1)) → seek to file offset → read and optionally decompress → for SQLite files, VFS layer presents data as virtual file → SQLite engine queries normally → results return through NAPI-RS to JavaScript. Total overhead for random access: typically under 1ms including all boundary crossings.

## A. SQLite VFS implementation in Rust

### Recommended library stack

**Primary: sqlite-vfs crate** (github.com/rkusa/sqlite-vfs) provides the cleanest path forward. This crate reduces SQLite's complex VFS API to a simple Rust trait with just six required methods: `open()`, `delete()`, `exists()`, `temporary_name()`, `random()`, and `sleep()`. The associated `DatabaseHandle` trait implements file operations: `read_exact_at()`, `write_all_at()`, `size()`, `sync()`, `set_len()`, and locking primitives. Despite being labeled prototype-quality, it passes the majority of SQLite's comprehensive test harness and powers production experiments like Cloudflare Durable Objects storage.

Use **rusqlite** (the standard Rust SQLite wrapper) for database operations. It integrates seamlessly with custom VFS implementations via `Connection::open_with_flags_and_vfs()`. The combination provides type-safe, idiomatic Rust APIs while maintaining full SQLite compatibility including transactions, prepared statements, and query optimization.

### Implementation pattern for archive integration

```rust
use sqlite_vfs::{register, Vfs, DatabaseHandle};
use std::sync::{Arc, Mutex};

pub struct ArchiveVfs {
    archive_path: PathBuf,
    file_index: Arc<Mutex<HashMap<String, FileEntry>>>,
}

impl Vfs for ArchiveVfs {
    type Handle = ArchiveFile;

    fn open(&self, db: &str, opts: OpenOptions) -> Result<Self::Handle> {
        // Extract database from archive into memory buffer
        let data = self.read_from_archive(db)?;
        Ok(ArchiveFile {
            data: Arc::new(RwLock::new(data)),
            position: 0,
            lock: LockKind::None,
        })
    }
}

impl DatabaseHandle for ArchiveFile {
    fn read_exact_at(&mut self, buf: &mut [u8], offset: u64) -> Result<()> {
        let data = self.data.read().unwrap();
        let start = offset as usize;
        buf.copy_from_slice(&data[start..start + buf.len()]);
        Ok(())
    }

    fn write_all_at(&mut self, buf: &[u8], offset: u64) -> Result<()> {
        let mut data = self.data.write().unwrap();
        let start = offset as usize;
        if start + buf.len() > data.len() {
            data.resize(start + buf.len(), 0);
        }
        data[start..start + buf.len()].copy_from_slice(buf);
        Ok(())
    }
}
```

**Critical consideration:** SQLite databases extracted from archives should load entirely into memory (using `Vec<u8>` as backing store) rather than streaming, because SQLite's random access patterns would cause excessive seeking and decompression overhead. For typical embedded databases under 100MB, this memory cost is acceptable. Use `PRAGMA journal_mode=MEMORY` to prevent SQLite from attempting to create separate journal files.

### Performance optimization strategies

SQLite's page cache architecture is your friend here. Configure generous cache sizes: `PRAGMA cache_size = -102400` allocates 100MB, dramatically reducing I/O. Since decompression is expensive, **every cache hit avoids re-decompressing archive data**. Testing with sqlite_zstd_vfs (a similar compression VFS) shows queries run at 80-90% of uncompressed speed with adequate caching.

Page alignment matters significantly. SQLite reads in fixed-size pages (default 4KB, configurable up to 64KB). Your VFS should report accurate sector sizes via `sector_size()` and set appropriate device characteristics like `DeviceCharacteristics::ATOMIC_4K`. For archive storage, consider storing each database with 64KB pages (`PRAGMA page_size=65536`) before archiving—this reduces page count, improves compression ratios, and decreases metadata overhead.

Avoid WAL mode complications initially. While Write-Ahead Logging offers better concurrent write performance, it requires implementing the shared memory interface (`xShmMap`, `xShmLock`, `xShmBarrier`, `xShmUnmap`)—substantial additional complexity. For read-only or single-writer cold storage scenarios, use `PRAGMA journal_mode=OFF` or `PRAGMA locking_mode=EXCLUSIVE` to simplify your VFS implementation.

## B. Archive format design for random access

### ASAR learnings applied to .eng format

Electron's ASAR format demonstrates that **uncompressed archives with JSON TOCs enable filesystem-speed random access**. ASAR's structure is elegantly simple: 8-byte Pickle-serialized header size, JSON directory tree, followed by concatenated file data. The JSON approach provides human readability and flexibility during development. Each file entry stores offset (as string to handle JavaScript's 53-bit integer limitation), size, and optional integrity data with per-block SHA-256 hashes.

However, ASAR has limitations for your use case. Placing the TOC at the beginning prevents streaming creation—you must know all content upfront or estimate TOC size and leave padding. It offers no compression, which is acceptable for Electron's speed-critical app.asar but wastes space for cold storage. ASAR also lacks versioning beyond optional "integrity" objects added in v3.1.0.

**For .eng, adopt a hybrid approach:** ZIP's TOC-at-end placement combined with ASAR's simplicity and speed focus. This enables streaming creation (write files sequentially, build TOC at end) while maintaining fast random access through in-memory indexing.

### Recommended .eng format specification

**File structure:**

```
[File Header: 64 bytes fixed]
[Local File Entry 1: header + compressed data]
[Local File Entry 2: header + compressed data]
...
[Central Directory: entry × count]
[End of Central Directory Record: 64 bytes]
```

**File header (64 bytes):**

```
Offset  Size  Field
0-7     8     Magic: 0x89 'E' 'N' 'G' 0x0D 0x0A 0x1A 0x0A
8-9     2     Format Version Major (uint16)
10-11   2     Format Version Minor (uint16)
12-15   4     Header CRC32
16-23   8     Central Directory Offset (uint64)
24-31   8     Central Directory Size (uint64)
32-35   4     Entry Count (uint32)
36-39   4     Content Version (uint32)
40-63   24    Reserved (zeros for future extensions)
```

The magic number follows PNG's pattern: non-ASCII first byte (0x89) prevents text misidentification, human-readable "ENG", and line-ending bytes (CR LF, Ctrl-Z, LF) that detect file corruption from text-mode transfers or DOS tools.

**Central Directory entry (320 bytes fixed):**

```
0-3     4     Signature 0x43454E54 ("CENT")
4-11    8     Data Offset (uint64, points to local header)
12-19   8     Uncompressed Size (uint64)
20-27   8     Compressed Size (uint64)
28-31   4     CRC32 of uncompressed data
32-39   8     Modified timestamp (Unix epoch)
40      1     Compression method (0=none, 1=LZ4, 2=Zstd, 3=deflate)
41      1     Flags
42-43   2     Path length (uint16)
44-299  256   File path (UTF-8 null-terminated)
300-319 20    Reserved
```

Fixed-size entries enable O(1) array indexing by file number and efficient binary searching. The 256-byte path limit accommodates most hierarchical structures; if you need longer paths, use a path pool at archive end with offsets here.

### TOC indexing and lookup implementation

On archive open, read the End of Central Directory record (scan backward from file end for signature), extract the Central Directory offset, and read all entries into memory. Build a hash table mapping file paths to entry metadata:

```rust
struct ArchiveIndex {
    entries: HashMap<String, EntryInfo>,
    entry_array: Vec<EntryInfo>, // For iteration
}

impl ArchiveIndex {
    fn from_central_directory(cd_data: &[u8], count: u32) -> Self {
        let mut entries = HashMap::with_capacity(count as usize);
        let mut entry_array = Vec::with_capacity(count as usize);

        for i in 0..count {
            let offset = i as usize * 320;
            let entry = parse_cd_entry(&cd_data[offset..offset+320]);
            entries.insert(entry.path.clone(), entry.clone());
            entry_array.push(entry);
        }

        Self { entries, entry_array }
    }

    fn lookup(&self, path: &str) -> Option<&EntryInfo> {
        self.entries.get(path)  // O(1) average case
    }
}
```

Memory overhead is minimal: approximately 100 bytes per file for the Rust structures plus the 320-byte entry. A 10,000-file archive consumes roughly 4MB for the complete index—negligible for desktop applications.

### Compression strategy maintaining random access

Use **per-file compression** rather than compressing the entire archive. This is essential: compressing everything would require full decompression to access any file. Per-file compression maintains true random access while achieving good compression ratios for individual assets.

**Algorithm selection by use case:**

- **LZ4** for speed-critical assets (textures, models, frequently accessed data): Decompression exceeds 2 GB/s on modern CPUs, minimal latency impact
- **Zstandard** for balanced compression: 40-50% size reduction with ~500 MB/s decompression, excellent ratio/speed tradeoff
- **Deflate/zlib** only for maximum compatibility if interfacing with ZIP tools

**Size threshold:** Don't compress files under 4KB—header and dictionary overhead may exceed savings. Store JSON manifests uncompressed if under 512KB for instant access without decompression CPU cost.

**Advanced: Chunked compression for large files (optional).** For multi-megabyte SQLite databases or large assets, implement frame-based compression like Google Fuchsia's format. Split files into 64KB frames, compress independently, store a seek table mapping byte ranges to frames. This enables decompressing only the portions SQLite actually reads rather than the entire database.

## C. Rust-to-Node.js integration via NAPI-RS

### Why NAPI-RS dominates WASM for this use case

Real-world benchmarks decisively favor NAPI-RS for I/O-heavy and data-intensive operations. The @napi-rs/snappy compression library shows 51-74% better throughput than WASM alternatives. Generic data processing tasks run 1.75-2.5x faster with native bindings. For your virtual filesystem scenario, **NAPI-RS provides critical advantages:** zero-copy buffer sharing (read archive data directly into JavaScript buffers without intermediate copies), direct file handle access, synchronous I/O when needed, and no memory sandboxing overhead.

WASM's architectural constraints make it poorly suited here. Every buffer crossing the WASM boundary requires copying memory—potentially multiple times when shuttling data from disk → Rust → WASM memory → JavaScript. Electron's V8 Memory Cage further restricts buffer sharing. WASM cannot access filesystem or native SQLite libraries directly, requiring all I/O to route through JavaScript shims, adding latency. For SQLite operations involving thousands of small reads, this overhead compounds severely.

**Use WASM only if:** distribution simplicity outweighs performance (single .wasm file vs. platform-specific binaries), or you need browser compatibility. For Electron desktop apps accessing archives frequently, NAPI-RS's performance advantage is definitive.

### Exposing virtual filesystem APIs

Design your JavaScript API to minimize boundary crossings—the primary performance concern. Batch operations where possible:

```rust
use napi_derive::napi;

#[napi]
pub struct EngArchive {
    inner: Arc<Mutex<ArchiveImpl>>,
}

#[napi]
impl EngArchive {
    #[napi(constructor)]
    pub fn new(path: String) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(Mutex::new(ArchiveImpl::open(path)?))
        })
    }

    // Zero-copy read for synchronous small files
    #[napi]
    pub fn read_file_sync(&self, path: String) -> Result<Buffer> {
        let archive = self.inner.lock().unwrap();
        let data = archive.read_file(&path)?;
        Ok(data.into())  // Vec<u8> → Buffer (no copy in most cases)
    }

    // Async read for I/O-bound operations
    #[napi]
    pub async fn read_file(&self, path: String) -> Result<Buffer> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            inner.lock().unwrap().read_file(&path)
        }).await?.map(|v| v.into())
    }

    // Batch reading multiple files (critical optimization)
    #[napi]
    pub async fn read_files(&self, paths: Vec<String>) -> Result<Vec<Buffer>> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let archive = inner.lock().unwrap();
            paths.iter()
                .map(|p| archive.read_file(p).map(|v| v.into()))
                .collect::<Result<Vec<_>>>()
        }).await?
    }
}
```

For SQLite integration, expose the database handle rather than individual queries:

```rust
#[napi]
pub struct EngDatabase {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

#[napi]
impl EngDatabase {
    #[napi(factory)]
    pub fn from_archive(archive: &EngArchive, db_path: String) -> Result<Self> {
        // Register custom VFS
        register("engvfs", ArchiveVfs::new(archive.clone()))?;

        let conn = rusqlite::Connection::open_with_flags_and_vfs(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            "engvfs"
        )?;

        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    #[napi]
    pub fn query(&self, sql: String, params: Vec<JsUnknown>) -> Result<Vec<Object>> {
        // Execute query, convert rows to JS objects
        // Implementation handles type conversion
    }
}
```

### Memory management and preventing leaks

NAPI-RS provides automatic reference counting for owned types. **Key rules:** use `Buffer` (owned) for data crossing async boundaries, use `&[u8]` (borrowed) for synchronous operations, wrap shared state in `Arc<Mutex<T>>`, and implement `Drop` for cleanup of file handles:

```rust
struct ArchiveImpl {
    file: std::fs::File,
    index: ArchiveIndex,
}

impl Drop for ArchiveImpl {
    fn drop(&mut self) {
        // File automatically closed, index memory freed
    }
}
```

Avoid circular references between Rust and JavaScript. If Rust holds a `ThreadsafeFunction` (JavaScript callback), ensure explicit cleanup methods or weak reference patterns.

### Electron-specific integration

Load your native module exclusively in the main process, never in renderer processes (which are often sandboxed). Use IPC for communication:

```javascript
// main.js
const { EngArchive } = require('./native/index.node');
const archive = new EngArchive('./data.eng');

ipcMain.handle('archive:read', async (event, path) => {
    return await archive.readFile(path);
});

// preload.js (with contextBridge)
const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('archive', {
    readFile: (path) => ipcRenderer.invoke('archive:read', path)
});
```

**electron-rebuild compatibility:** NAPI-RS generates platform-specific binaries. Configure your build to handle multiple platforms:

```json
{
  "napi": {
    "name": "eng-archive",
    "triples": {
      "defaults": true,
      "additional": [
        "x86_64-pc-windows-msvc",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-unknown-linux-gnu"
      ]
    }
  }
}
```

Configure electron-builder to exclude native modules from ASAR packaging:

```json
{
  "build": {
    "asarUnpack": [
      "**/*.node"
    ]
  }
}
```

## D. Learning from existing projects

### Critical projects to study

**sqlite-vfs (Rust)** is your implementation blueprint. Read the source closely—it demonstrates the minimal VFS surface area needed, shows how to handle locking states properly (a subtle requirement even for single-user VFS), and includes patterns for thread-safe access. The project successfully runs SQLite's test harness, proving the approach works.

**ratarmount (Python)** validates the SQLite-index-for-archives pattern at scale. This FUSE filesystem tested with 150GB archives containing 2M files demonstrates that SQLite indexes (consuming ~150MB) enable 2ms file access versus 250ms for naive scanning. The key insight: build the index once on first mount, persist it, and reuse it—orders of magnitude faster than scanning.

**gitoxide/gix (Rust)** provides production-quality packfile handling if you want Git repository reconstruction. The modular crate design (gix-pack, gix-odb, gix-index) shows how to structure complex binary format readers. Git's packfile format uses delta compression and sophisticated indexing—more complex than you need, but demonstrates handling of offset tables, fan-out tables for binary search, and multi-pack indexes.

**rust-vfs** offers clean VFS abstraction patterns. Its `OverlayFS` implementation (writable layer over read-only layers) is particularly relevant—you might layer a writable MemoryFS over your read-only .eng archive for temporary modifications. The trait design balances simplicity with flexibility.

**sqlite_zstd_vfs** demonstrates compression VFS performance characteristics. Study its PRAGMA configuration recommendations and benchmarking methodology. Its documentation shows real-world query performance at ~80-90% of uncompressed with proper cache sizing.

### Architecture pattern synthesis

Combine these learnings into a layered architecture:

```
┌─────────────────────────────────────┐
│  JavaScript Application (Electron)  │
└──────────────┬──────────────────────┘
               │ NAPI-RS (near-zero overhead)
┌──────────────▼──────────────────────┐
│  Rust Native Module                 │
│  ┌────────────────────────────────┐ │
│  │  Public API (EngArchive class) │ │
│  └──────────┬─────────────────────┘ │
│             │                        │
│  ┌──────────▼─────────────────────┐ │
│  │  Archive Reader                │ │
│  │  - TOC parser & indexer        │ │
│  │  - File extraction             │ │
│  │  - Decompression (LZ4/Zstd)   │ │
│  └──────────┬─────────────────────┘ │
│             │                        │
│  ┌──────────▼─────────────────────┐ │
│  │  SQLite VFS (sqlite-vfs trait) │ │
│  │  - Virtual file handles        │ │
│  │  - Memory-backed databases     │ │
│  └──────────┬─────────────────────┘ │
│             │                        │
│  ┌──────────▼─────────────────────┐ │
│  │  OS File I/O (Rust std::fs)    │ │
│  └────────────────────────────────┘ │
└─────────────────────────────────────┘
```

This architecture provides clean separation: the archive reader handles format details, the VFS layer abstracts SQLite's requirements, and the NAPI layer exposes everything to JavaScript with minimal overhead.

## E. Implementation patterns and best practices

### TOC placement and access patterns

**End-of-file TOC placement is recommended** for your cold storage scenario. When creating archives from Git repos or build artifacts, you can stream files sequentially without knowing the complete file list upfront. Write file data as you process it, accumulate Central Directory entries in memory, then write the complete TOC at the end. For updates, append new file data and write a new Central Directory—the old TOC becomes invalid but data remains intact.

Reading is equally efficient: seek to near-end (last 64KB), scan backward for End of Central Directory signature, read its offset pointer, load entire Central Directory into memory (typically 1-5MB even for large archives), build hash index. This one-time cost at open time enables O(1) random access thereafter.

**Alternative: hybrid format.** If you need instant metadata access without reading the end, place a **minimal header** with version and offset pointer at the beginning, full TOC at end. This adds 64 bytes but enables validating format version before seeking to TOC.

### Compression strategy details

Store a compression method flag per file in the Central Directory. Use this decision tree during archive creation:

```
if file_size < 4KB: store uncompressed
else if file_extension in ['.db', '.sqlite', '.sqlite3']:
    use Zstandard level 6 (good compression, acceptable speed)
else if file_extension in ['.json', '.txt', '.xml']:
    use Zstandard level 9 (text compresses extremely well)
else if file_extension in ['.jpg', '.png', '.mp4', '.zip']:
    store uncompressed (already compressed)
else:
    use LZ4 (safe default, always fast)
```

For SQLite databases larger than 50MB, consider frame-based compression with 64KB chunks. Store a seek table in the entry header:

```rust
struct CompressedFileHeader {
    frame_size: u32,
    frame_count: u32,
    frames: Vec<FrameInfo>,
}

struct FrameInfo {
    decompressed_offset: u64,
    decompressed_size: u32,
    compressed_offset: u64,
    compressed_size: u32,
}
```

When SQLite's VFS requests bytes at offset X, calculate which frames contain that range, decompress only those frames, return the requested slice. This dramatically improves query performance by avoiding decompression of entire multi-gigabyte databases.

### Version management and evolution

Implement **dual versioning:** format version in the header (governs binary structure) and content version (governs data schemas). Format version uses semantic versioning—major increments for breaking changes, minor for backward-compatible additions. When opening an archive, check:

```rust
fn validate_version(major: u16, minor: u16) -> Result<()> {
    const CURRENT_MAJOR: u16 = 1;
    const CURRENT_MINOR: u16 = 0;

    if major > CURRENT_MAJOR {
        return Err("Archive format too new");
    }
    if major < CURRENT_MAJOR {
        return Err("Archive format too old (incompatible)");
    }
    // major == CURRENT_MAJOR: check minor
    if minor > CURRENT_MINOR {
        // Warn but proceed (forward-compatible)
        warn!("Archive uses newer features (v{}.{})", major, minor);
    }
    Ok(())
}
```

**Reserve fields everywhere.** The 64-byte header has 24 reserved bytes, each Central Directory entry has 20 reserved bytes. When adding features in v1.1, you can use reserved space for new fields without breaking v1.0 readers (they ignore the fields). Document reserved bytes must be zero in current version, so old readers will predictably interpret new fields as "not present."

**Feature flags** (optional advanced pattern): Use 4 bytes for required features bitmap, 4 bytes for optional features. Bits indicate capabilities: compression method support, encryption, extended attributes, etc. Readers check required features and reject archives with unsupported requirements, but silently ignore optional features they don't understand.

### Error handling and corruption detection

Implement defense in depth with **multiple checksum layers:**

1. **Header CRC32** (fast fail): If corrupted, entire archive is unreadable—appropriate
2. **Per-file CRC32** in Central Directory: Verify after decompression before returning data
3. **End record checksum** (optional): Detect truncation

On read errors, fail gracefully:

```rust
pub fn read_file(&self, path: &str) -> Result<Vec<u8>, ArchiveError> {
    let entry = self.index.lookup(path)
        .ok_or(ArchiveError::FileNotFound)?;

    let compressed = self.read_at_offset(entry.offset, entry.compressed_size)
        .map_err(|e| ArchiveError::ReadError(e))?;

    let decompressed = match entry.compression {
        CompressionMethod::None => compressed,
        CompressionMethod::LZ4 => lz4::decompress(&compressed)?,
        CompressionMethod::Zstd => zstd::decompress(&compressed)?,
    };

    let computed_crc = crc32fast::hash(&decompressed);
    if computed_crc != entry.crc32 {
        return Err(ArchiveError::CorruptedData {
            path: path.to_string(),
            expected: entry.crc32,
            actual: computed_crc,
        });
    }

    Ok(decompressed)
}
```

Log errors with context, consider retry logic for transient I/O failures, provide API for validation (scan entire archive and verify all CRCs) for diagnosing corruption.

### Git repository reconstruction

Store the Git directory structure in your archive with special handling:

```
.git/
├── HEAD (file)
├── config (file)
├── refs/
│   ├── heads/
│   │   └── main (file with commit hash)
│   └── tags/
├── objects/
│   ├── pack/
│   │   ├── pack-{hash}.pack (large packfile)
│   │   └── pack-{hash}.idx (index)
│   └── {ab}/{cdef...} (loose objects)
└── index (staging area)
```

**Option 1: Store packfiles directly.** If your repository is already packed (typical for clones), include the packfile and index in your archive. On extraction, write them to .git/objects/pack/. Git will use them directly.

**Option 2: Custom packing.** Use gitoxide's gix-pack crate to generate packfiles optimized for your distribution. This allows custom delta compression and object ordering.

**Option 3: Loose objects.** For smaller repos or maximum simplicity, store each Git object as a separate file in the archive. More files but simpler reconstruction logic.

Provide a reconstruction API:

```rust
#[napi]
pub fn extract_git_repo(&self, target_path: String) -> Result<()> {
    // Extract all .git/* entries
    for path in self.index.iter_prefix(".git/") {
        let data = self.read_file(path)?;
        std::fs::write(format!("{}/{}", target_path, path), data)?;
    }

    // Verify HEAD exists and is valid
    let head = std::fs::read_to_string(format!("{}/.git/HEAD", target_path))?;
    if !head.starts_with("ref: ") {
        return Err("Invalid HEAD");
    }

    Ok(())
}
```

After extraction, Git commands work normally: `git log`, `git checkout`, etc.

## Concrete implementation roadmap

**Phase 1: Archive format (2-3 weeks)**

Create archive writer that generates .eng files with the specified format. Implement reader that parses TOC, builds hash index, extracts files with decompression. Write comprehensive tests including corruption scenarios, large files, various compression methods. Benchmark random access performance against ZIP and filesystem.

**Phase 2: SQLite VFS (1-2 weeks)**

Implement sqlite-vfs trait using your archive reader as backend. Start with in-memory database extraction (load entire .db file into Vec\<u8\>). Test with rusqlite, verify transactions, prepared statements, concurrent reads work correctly. Benchmark query performance with different cache sizes.

**Phase 3: NAPI-RS bindings (1 week)**

Create NAPI-RS project wrapping archive reader and SQLite access. Design JavaScript API focused on batching. Implement async operations using tokio. Test in Node.js first, then Electron. Profile boundary crossing overhead.

**Phase 4: Electron integration (1 week)**

Set up electron-forge with native module support. Implement IPC patterns for main-to-renderer communication. Configure build for multiple platforms. Test ASAR unpacking of native modules. Create minimal example application demonstrating archive access.

**Phase 5: Optimization (ongoing)**

Profile hot paths, optimize hash algorithms, tune compression parameters, implement connection pooling for SQLite if needed, add LRU caching for frequently accessed files, consider memory-mapping for very large archives.

## Library dependencies

Core Rust crates:

- `napi-rs` 2.x for Node.js bindings
- `rusqlite` 0.31+ for SQLite operations  
- `sqlite-vfs` 0.x for VFS trait (may need forking for production hardening)
- `lz4_flex` 0.11+ for fast decompression
- `zstd` 0.13+ for balanced compression
- `crc32fast` 1.4+ for checksums
- `tokio` 1.x for async I/O
- `serde` + `serde_json` for manifest parsing
- Optional: `gitoxide` crates if implementing Git features

Build tooling:

- `@napi-rs/cli` for cross-compilation and publishing
- `electron-rebuild` or `@electron-forge/plugin-native-modules`
- `cargo-make` or similar for complex build workflows

## Performance expectations

With this architecture, expect:

- **Archive opening:** 10-50ms for 10K-file archives (TOC read + index build)
- **Random file access:** \<1ms for small files, ~2-5ms for compressed files up to 10MB
- **SQLite queries:** 80-90% of native filesystem speed with 100MB page cache
- **Batch operations:** Near-linear scaling with file count (minimal per-file overhead)
- **Memory usage:** ~100 bytes per file for index + SQLite cache + working buffers (typically 150-300MB total)

This is production-ready performance for desktop applications. Cold storage distribution via GitHub works excellently—users download once, experience filesystem-like access thereafter with no extraction step.

Your .eng format will provide the best of both worlds: compact distribution (compressed archives), powerful querying (SQLite), and speed (no extraction, optimized random access). The Rust+NAPI-RS foundation ensures maximum performance while maintaining clean APIs for your Electron application.