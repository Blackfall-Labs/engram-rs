# Engram Archive Format Specification v0.1

> **Note**: This specification describes both implemented and planned features. See implementation status sections for details.

## Abstract

Engram (.eng) is a custom archive format designed for cold storage and distribution of civic infrastructure data. It combines ZIP-like random access, embedded SQLite database querying via custom VFS, and efficient compression in a single container. The format enables filesystem-speed access without extraction, making it ideal for distributing structured government data, legal codes, and historical records via GitHub.

**Implementation Status**:
- ✅ **Implemented**: Core archive format (read/write), SQLite VFS integration, NAPI-RS bindings, standard compression (LZ4/Zstd)
- 🚧 **In Progress**: Electron integration, optimization phases
- 📋 **Planned**: CML-specific tokenization compression (see separate compression spec)

## Design Goals

- **No-extraction access**: Query SQLite databases and read files directly from archive
- **Random access performance**: <1ms file lookups via in-memory TOC indexing
- **Streaming creation**: TOC-at-end design enables sequential file writing
- **Git repository storage**: Preserve complete Git history and directory structure
- **Cross-platform distribution**: Native Rust implementation with Node.js/Electron bindings
- **Corruption detection**: Multi-layer checksums (header, per-file, optional end-record)
- **Efficient compression**: Per-file compression with LZ4, Zstd, or deflate

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│ Electron/Node.js Application                        │
├─────────────────────────────────────────────────────┤
│ NAPI-RS Bindings (Zero-copy buffer sharing)        │
├─────────────────────────────────────────────────────┤
│ Rust Core                                           │
│  ├─ Archive Reader (TOC index, decompression)      │
│  ├─ SQLite VFS (sqlite-vfs trait implementation)   │
│  └─ Git Reconstruction                              │
├─────────────────────────────────────────────────────┤
│ .eng Archive File                                   │
│  ├─ File Header (64 bytes)                         │
│  ├─ Local File Entries (header + compressed data)  │
│  ├─ Central Directory (320 bytes × count)          │
│  └─ End of Central Directory Record (64 bytes)     │
└─────────────────────────────────────────────────────┘
```

---

## Part I: Format Specification

### 1. File Structure

```
[File Header]                    64 bytes fixed
[Local File Entry 1]             variable (header + compressed data)
[Local File Entry 2]             variable
...
[Local File Entry N]             variable
[Central Directory]              320 bytes × entry count
[End of Central Directory]       64 bytes fixed
```

### 2. File Header (64 bytes)

| Offset | Size | Type    | Field                         | Description                                    |
|--------|------|---------|-------------------------------|------------------------------------------------|
| 0-7    | 8    | bytes   | Magic                         | `0x89 'E' 'N' 'G' 0x0D 0x0A 0x1A 0x0A`         |
| 8-9    | 2    | uint16  | Format Version Major          | Current: 0                                     |
| 10-11  | 2    | uint16  | Format Version Minor          | Current: 1                                     |
| 12-15  | 4    | uint32  | Header CRC32                  | Checksum of bytes 8-63                         |
| 16-23  | 8    | uint64  | Central Directory Offset      | Byte offset from file start                    |
| 24-31  | 8    | uint64  | Central Directory Size        | Total bytes in Central Directory               |
| 32-35  | 4    | uint32  | Entry Count                   | Number of files in archive                     |
| 36-39  | 4    | uint32  | Content Version               | Archive content version (user-defined)         |
| 40-63  | 24   | bytes   | Reserved                      | Must be zeros                                  |

**Magic Number Rationale**: Follows PNG pattern - non-ASCII lead byte (0x89), human-readable "ENG", line-ending detection bytes (CR, LF, Ctrl-Z, LF).

### 3. Local File Entry (Variable Length)

| Offset | Size     | Type   | Field                  | Description                              |
|--------|----------|--------|------------------------|------------------------------------------|
| 0-3    | 4        | bytes  | Signature              | `0x4C4F4341` ("LOCA")                    |
| 4-11   | 8        | uint64 | Uncompressed Size      | Original file size                       |
| 12-19  | 8        | uint64 | Compressed Size        | Actual data size following this header   |
| 20-23  | 4        | uint32 | CRC32                  | Of uncompressed data                     |
| 24-31  | 8        | uint64 | Modified Timestamp     | Unix epoch seconds                       |
| 32     | 1        | uint8  | Compression Method     | 0=none, 1=LZ4, 2=Zstd, 3=deflate         |
| 33     | 1        | uint8  | Flags                  | Reserved (must be 0)                     |
| 34-35  | 2        | uint16 | Path Length            | Length of following path string          |
| 36-39  | 4        | uint32 | Reserved               | Must be zeros                            |
| 40+    | variable | bytes  | File Path              | UTF-8 encoded, null-terminated           |
| ...    | variable | bytes  | Compressed Data        | Compressed file content                  |

### 4. Central Directory Entry (320 bytes)

| Offset  | Size | Type   | Field                  | Description                              |
|---------|------|--------|------------------------|------------------------------------------|
| 0-3     | 4    | bytes  | Signature              | `0x43454E54` ("CENT")                    |
| 4-11    | 8    | uint64 | Data Offset            | Byte offset to Local File Entry          |
| 12-19   | 8    | uint64 | Uncompressed Size      | Original file size                       |
| 20-27   | 8    | uint64 | Compressed Size        | Actual data size                         |
| 28-31   | 4    | uint32 | CRC32                  | Of uncompressed data                     |
| 32-39   | 8    | uint64 | Modified Timestamp     | Unix epoch seconds                       |
| 40      | 1    | uint8  | Compression Method     | 0=none, 1=LZ4, 2=Zstd, 3=deflate         |
| 41      | 1    | uint8  | Flags                  | Bit 0: Directory, Bit 1-7: Reserved      |
| 42-43   | 2    | uint16 | Path Length            | Actual path length (max 255)             |
| 44-299  | 256  | bytes  | File Path              | UTF-8, null-terminated, null-padded      |
| 300-319 | 20   | bytes  | Reserved               | Must be zeros                            |

**Design Notes**: 
- Fixed 320-byte entries enable O(1) array indexing and efficient binary search
- 256-byte path limit accommodates most hierarchical structures
- For longer paths: use path pool at archive end with offsets in this field

### 5. End of Central Directory Record (64 bytes)

| Offset | Size | Type   | Field                         | Description                           |
|--------|------|--------|-------------------------------|---------------------------------------|
| 0-3    | 4    | bytes  | Signature                     | `0x454E4452` ("ENDR")                 |
| 4-11   | 8    | uint64 | Central Directory Offset      | Duplicate of header field             |
| 12-19  | 8    | uint64 | Central Directory Size        | Duplicate of header field             |
| 20-23  | 4    | uint32 | Entry Count                   | Duplicate of header field             |
| 24-27  | 4    | uint32 | Archive CRC32                 | Optional: checksum of entire archive  |
| 28-31  | 4    | uint32 | Flags                         | Reserved                              |
| 32-63  | 32   | bytes  | Reserved                      | Must be zeros                         |

**Lookup Algorithm**: Scan backward from file end for `ENDR` signature, parse record, seek to Central Directory.

### 6. Compression Methods

| Value | Method  | Library        | Use Case                                  |
|-------|---------|----------------|-------------------------------------------|
| 0     | None    | -              | Pre-compressed data, small files          |
| 1     | LZ4     | lz4_flex       | Fast decompression, moderate compression  |
| 2     | Zstd    | zstd           | Balanced speed/compression ratio          |
| 3     | Deflate | flate2         | Maximum compatibility                     |

**Recommendation**: Use Zstd (level 3-10) for SQLite databases and JSON. Use LZ4 for frequently accessed files requiring minimal decompression latency.

### 7. Reserved Flags and Future Extensions

**Central Directory Entry Flags** (byte 41):
- Bit 0: Directory entry (size=0, no data)
- Bit 1: Encrypted (reserved, not implemented)
- Bit 2: Extended attributes present (reserved)
- Bits 3-7: Reserved (must be 0)

**Future Extension Points**:
- Reserved fields in all structures for backward-compatible additions
- Version fields allow format evolution
- Feature flags bitmap (proposed): 4 bytes required features, 4 bytes optional features

---

## Part II: SQLite VFS Integration

### 1. VFS Architecture

Engram implements SQLite's Virtual File System (VFS) interface to enable direct database querying without extraction.

```rust
use sqlite_vfs::{Vfs, DatabaseHandle};

pub struct EngVfs {
    archive_path: PathBuf,
    file_index: Arc<Mutex<HashMap<String, FileEntry>>>,
}

impl Vfs for EngVfs {
    type Handle = EngDatabaseHandle;
    
    fn open(&self, db: &str, opts: OpenOptions) -> Result<Self::Handle> {
        // Extract database from archive into memory buffer
        let data = self.read_from_archive(db)?;
        Ok(EngDatabaseHandle {
            data: Arc::new(RwLock::new(data)),
            position: 0,
            lock: LockKind::None,
        })
    }
    
    // Implement: delete(), exists(), temporary_name(), random(), sleep()
}

impl DatabaseHandle for EngDatabaseHandle {
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
    
    // Implement: size(), sync(), set_len(), lock(), unlock(), check_reserved_lock()
}
```

### 2. Database Loading Strategy

**In-Memory Extraction** (Recommended):
- Load entire `.db` file from archive into `Vec<u8>`
- Avoids excessive seeking and repeated decompression
- Acceptable for databases <100MB (typical for embedded civic data)
- Use `PRAGMA journal_mode=MEMORY` to prevent separate journal files

**Streaming with Block Cache** (Future optimization):
- For databases >100MB, implement block-level caching
- Decompress in SQLite page-sized chunks (4KB-64KB)
- LRU cache for frequently accessed pages

### 3. SQLite Configuration for Archive Access

```sql
-- Increase page cache (100MB = 102400KB)
PRAGMA cache_size = -102400;

-- Use memory-based journal (no external files)
PRAGMA journal_mode = MEMORY;

-- Disable WAL mode (requires shared memory)
PRAGMA locking_mode = EXCLUSIVE;

-- Optimize page size before archiving
PRAGMA page_size = 65536;
```

**Performance Tuning**:
- Larger page sizes reduce page count and improve compression
- Generous cache sizes minimize re-decompression
- Expect 80-90% of native filesystem speed with proper caching

---

## Part III: Implementation Details

### 1. Core Libraries (Rust)

| Crate           | Version | Purpose                                  |
|-----------------|---------|------------------------------------------|
| napi-rs         | 2.x     | Node.js bindings with zero-copy buffers  |
| rusqlite        | 0.31+   | SQLite operations                        |
| sqlite-vfs      | 0.x     | VFS trait implementation                 |
| lz4_flex        | 0.11+   | LZ4 decompression                        |
| zstd            | 0.13+   | Zstd compression/decompression           |
| crc32fast       | 1.4+    | CRC32 checksums                          |
| tokio           | 1.x     | Async I/O                                |
| serde/serde_json| Latest  | Manifest parsing                         |

**Optional**:
- `gitoxide` (gix-pack, gix-object): Git repository handling

### 2. Archive Reading Implementation

```rust
pub struct ArchiveIndex {
    entries: HashMap<String, EntryInfo>,
    entry_array: Vec<EntryInfo>,
}

impl ArchiveIndex {
    pub fn from_central_directory(cd_data: &[u8], count: u32) -> Result<Self> {
        let mut entries = HashMap::with_capacity(count as usize);
        let mut entry_array = Vec::with_capacity(count as usize);
        
        for i in 0..count {
            let offset = (i as usize) * 320;
            let entry_data = &cd_data[offset..offset + 320];
            
            // Parse fixed-size entry
            let entry = EntryInfo::parse(entry_data)?;
            entries.insert(entry.path.clone(), entry.clone());
            entry_array.push(entry);
        }
        
        Ok(Self { entries, entry_array })
    }
    
    pub fn lookup(&self, path: &str) -> Option<&EntryInfo> {
        self.entries.get(path)
    }
}

pub fn read_file(&self, path: &str) -> Result<Vec<u8>, ArchiveError> {
    let entry = self.index.lookup(path)
        .ok_or(ArchiveError::FileNotFound)?;
    
    let compressed = self.read_at_offset(entry.offset, entry.compressed_size)?;
    
    let decompressed = match entry.compression {
        CompressionMethod::None => compressed,
        CompressionMethod::LZ4 => lz4_flex::decompress_size_prepended(&compressed)?,
        CompressionMethod::Zstd => zstd::decode_all(&compressed[..])?,
        CompressionMethod::Deflate => {
            use flate2::read::DeflateDecoder;
            let mut decoder = DeflateDecoder::new(&compressed[..]);
            let mut result = Vec::new();
            std::io::Read::read_to_end(&mut decoder, &mut result)?;
            result
        }
    };
    
    // Verify CRC32
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

### 3. Archive Writing Implementation

```rust
pub struct ArchiveWriter {
    file: BufWriter<File>,
    entries: Vec<CentralDirEntry>,
    current_offset: u64,
}

impl ArchiveWriter {
    pub fn new(path: &Path) -> Result<Self> {
        let mut file = BufWriter::new(File::create(path)?);
        
        // Write file header (placeholder for offsets)
        let header = FileHeader::new_placeholder();
        file.write_all(&header.to_bytes())?;
        
        Ok(Self {
            file,
            entries: Vec::new(),
            current_offset: 64, // After header
        })
    }
    
    pub fn add_file(&mut self, path: &str, data: &[u8], compression: CompressionMethod) -> Result<()> {
        let compressed = match compression {
            CompressionMethod::None => data.to_vec(),
            CompressionMethod::LZ4 => lz4_flex::compress_prepend_size(data),
            CompressionMethod::Zstd => zstd::encode_all(data, 3)?,
            CompressionMethod::Deflate => {
                use flate2::write::DeflateEncoder;
                use flate2::Compression;
                let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
                std::io::Write::write_all(&mut encoder, data)?;
                encoder.finish()?
            }
        };
        
        let crc32 = crc32fast::hash(data);
        let modified = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        
        // Write local file entry
        let local_entry = LocalFileEntry {
            signature: LOCAL_SIGNATURE,
            uncompressed_size: data.len() as u64,
            compressed_size: compressed.len() as u64,
            crc32,
            modified,
            compression,
            flags: 0,
            path_length: path.len() as u16,
            path: path.to_string(),
        };
        
        self.file.write_all(&local_entry.to_bytes())?;
        self.file.write_all(&compressed)?;
        
        // Store central directory entry
        let cd_entry = CentralDirEntry {
            data_offset: self.current_offset,
            uncompressed_size: data.len() as u64,
            compressed_size: compressed.len() as u64,
            crc32,
            modified,
            compression,
            flags: 0,
            path: path.to_string(),
        };
        
        self.entries.push(cd_entry);
        self.current_offset += local_entry.total_size() + compressed.len() as u64;
        
        Ok(())
    }
    
    pub fn finalize(mut self) -> Result<()> {
        let cd_offset = self.current_offset;
        
        // Write central directory
        for entry in &self.entries {
            self.file.write_all(&entry.to_bytes())?;
        }
        
        let cd_size = (self.entries.len() * 320) as u64;
        
        // Write end record
        let end_record = EndRecord {
            signature: END_SIGNATURE,
            cd_offset,
            cd_size,
            entry_count: self.entries.len() as u32,
            archive_crc: 0, // Optional
            flags: 0,
        };
        
        self.file.write_all(&end_record.to_bytes())?;
        
        // Update file header with correct offsets
        self.file.seek(SeekFrom::Start(0))?;
        let header = FileHeader {
            magic: MAGIC_NUMBER,
            version_major: 0,
            version_minor: 1,
            header_crc: 0, // Calculate after
            cd_offset,
            cd_size,
            entry_count: self.entries.len() as u32,
            content_version: 1,
            reserved: [0; 24],
        };
        
        self.file.write_all(&header.to_bytes())?;
        self.file.flush()?;
        
        Ok(())
    }
}
```

### 4. Git Repository Reconstruction

```rust
#[napi]
pub fn extract_git_repo(&self, target_path: String) -> Result<()> {
    // Create .git directory structure
    std::fs::create_dir_all(format!("{}/.git/objects", target_path))?;
    std::fs::create_dir_all(format!("{}/.git/refs/heads", target_path))?;
    std::fs::create_dir_all(format!("{}/.git/refs/tags", target_path))?;
    
    // Extract all .git/* entries
    for path in self.index.iter_prefix(".git/") {
        let data = self.read_file(path)?;
        let full_path = format!("{}/{}", target_path, path);
        
        // Create parent directories
        if let Some(parent) = Path::new(&full_path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        std::fs::write(full_path, data)?;
    }
    
    // Verify HEAD exists and is valid
    let head_path = format!("{}/.git/HEAD", target_path);
    let head = std::fs::read_to_string(&head_path)?;
    
    if !head.starts_with("ref: ") && !head.chars().all(|c| c.is_ascii_hexdigit() || c == '\n') {
        return Err(Error::from_reason("Invalid HEAD file"));
    }
    
    Ok(())
}
```

**Git Storage Strategies**:
1. **Packfiles**: Store `.git/objects/pack/*.pack` and `*.idx` directly (recommended)
2. **Loose Objects**: Store each object as separate file (simpler, more files)
3. **Custom Packing**: Use gitoxide to generate optimized packfiles

---

## Part IV: NAPI-RS Bindings

### 1. JavaScript API Design

```rust
#[napi]
pub struct EngArchive {
    inner: Arc<ArchiveReader>,
}

#[napi]
impl EngArchive {
    #[napi(constructor)]
    pub fn new(path: String) -> Result<Self> {
        let reader = ArchiveReader::open(&path)?;
        Ok(Self {
            inner: Arc::new(reader),
        })
    }
    
    #[napi]
    pub fn list_files(&self, prefix: Option<String>) -> Vec<String> {
        self.inner.list_files(prefix.as_deref())
    }
    
    #[napi]
    pub fn read_file(&self, path: String) -> Result<Buffer> {
        let data = self.inner.read_file(&path)?;
        Ok(Buffer::from(data))
    }
    
    #[napi]
    pub async fn read_file_async(&self, path: String) -> Result<Buffer> {
        let reader = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let data = reader.read_file(&path)?;
            Ok(Buffer::from(data))
        }).await?
    }
    
    #[napi]
    pub fn query_sqlite(&self, db_path: String, sql: String) -> Result<Vec<JsObject>> {
        let conn = self.inner.open_sqlite(&db_path)?;
        let mut stmt = conn.prepare(&sql)?;
        
        let columns: Vec<String> = stmt.column_names().iter()
            .map(|s| s.to_string())
            .collect();
        
        let mut results = Vec::new();
        let rows = stmt.query_map([], |row| {
            let mut obj = HashMap::new();
            for (i, col) in columns.iter().enumerate() {
                // Convert SQLite value to JavaScript value
                let value: serde_json::Value = row.get(i)?;
                obj.insert(col.clone(), value);
            }
            Ok(obj)
        })?;
        
        for row in rows {
            results.push(row?);
        }
        
        Ok(results)
    }
    
    #[napi]
    pub fn extract_git(&self, target_path: String) -> Result<()> {
        self.inner.extract_git_repo(target_path)
    }
}
```

### 2. TypeScript Usage

```typescript
import { EngArchive } from '@manifest-humanity/engram';

async function main() {
    // Open archive
    const archive = new EngArchive('./us-code-2024.eng');
    
    // List files
    const files = archive.listFiles('laws/');
    console.log('Files:', files);
    
    // Read file
    const content = await archive.readFileAsync('manifest.json');
    const manifest = JSON.parse(content.toString('utf-8'));
    
    // Query SQLite
    const results = archive.querySqlite(
        'index.db',
        'SELECT * FROM bills WHERE year = 2024'
    );
    console.log('Bills:', results);
    
    // Extract Git repository
    archive.extractGit('./extracted-repo');
}
```

### 3. Electron Integration

```typescript
// main.ts (Main Process)
import { app, BrowserWindow, ipcMain } from 'electron';
import { EngArchive } from '@manifest-humanity/engram';

let archives = new Map<string, EngArchive>();

ipcMain.handle('archive:open', async (event, path: string) => {
    const archive = new EngArchive(path);
    const id = crypto.randomUUID();
    archives.set(id, archive);
    return id;
});

ipcMain.handle('archive:query', async (event, id: string, dbPath: string, sql: string) => {
    const archive = archives.get(id);
    if (!archive) throw new Error('Archive not found');
    return archive.querySqlite(dbPath, sql);
});

// renderer.ts (Renderer Process)
const archiveId = await window.electronAPI.archiveOpen('./data.eng');
const results = await window.electronAPI.archiveQuery(
    archiveId,
    'index.db',
    'SELECT * FROM documents WHERE category = "civic"'
);
```

---

## Part V: Performance Characteristics

### 1. Expected Performance

| Operation                    | Performance               | Notes                                    |
|------------------------------|---------------------------|------------------------------------------|
| Archive opening              | 10-50ms                   | For 10K-file archives (TOC + index)      |
| Random file access (small)   | <1ms                      | Files <1MB, uncompressed                 |
| Random file access (large)   | 2-5ms                     | Files <10MB, compressed                  |
| SQLite queries               | 80-90% native speed       | With 100MB page cache                    |
| Batch file reads             | Near-linear scaling       | Minimal per-file overhead                |
| Memory usage                 | 150-300MB typical         | 100 bytes/file + SQLite cache + buffers  |

### 2. Optimization Strategies

**Archive Level**:
- Pre-sort Central Directory by access frequency (put frequently accessed files first)
- Use compression level 3-5 for balance (higher levels have diminishing returns)
- Align local file entries to 64-byte boundaries for better I/O

**SQLite Level**:
- `PRAGMA cache_size = -102400` (100MB cache)
- `PRAGMA page_size = 65536` (64KB pages before archiving)
- `PRAGMA journal_mode = MEMORY` (no external files)
- Index optimization: ensure proper indexes exist before archiving

**Application Level**:
- Connection pooling: reuse SQLite connections
- LRU caching: cache frequently accessed files in memory
- Prefetching: preload files likely to be accessed together
- Memory mapping: for very large archives (>1GB), consider mmap

### 3. Benchmarking Results (Projected)

Based on similar implementations (sqlite_zstd_vfs, ASAR):

```
Archive: 500MB compressed, 1.2GB uncompressed, 5000 files, 3 SQLite databases
Hardware: M1 Mac, NVMe SSD

Opening archive:         23ms
Random file access:      0.4ms (median), 2.1ms (95th percentile)
SQLite query (simple):   1.2ms
SQLite query (complex):  45ms (vs 38ms native = 84% speed)
Memory usage:            187MB
```

---

## Part VI: Implementation Status

### Core Archive Format
- [x] Binary format structures defined
- [x] Archive writer implementation
- [x] Archive reader with TOC indexing
- [x] Compression support (LZ4, Zstd, deflate)
- [ ] Comprehensive test suite
  - [ ] Corruption scenarios
  - [ ] Large files (>100MB)
  - [ ] Unicode paths
  - [ ] Edge cases (empty files, directories)
- [ ] Benchmark suite (vs ZIP and filesystem)

### SQLite VFS Integration
- [x] sqlite-vfs trait implementation
- [x] In-memory database extraction
- [x] rusqlite integration
  - [x] Basic queries
  - [x] Transactions
  - [x] Prepared statements
  - [ ] Concurrent reads testing
- [ ] Query performance benchmarks
- [ ] Cache configuration optimization

### NAPI-RS Bindings
- [x] NAPI-RS project structure
- [x] JavaScript API implementation
  - [x] Archive opening/closing
  - [x] File reading (sync/async)
  - [x] SQLite querying
  - [x] Git extraction
- [x] Zero-copy buffer optimization
- [x] Error handling and JS exceptions
- [x] TypeScript type definitions
- [x] Node.js environment testing

### Electron Integration
- [ ] electron-forge project setup
- [ ] Native module configuration
- [ ] IPC patterns
  - [ ] Main process: archive management
  - [ ] Renderer process: query interface
- [ ] Cross-platform builds (macOS, Windows, Linux)
- [ ] ASAR compatibility testing
- [ ] Example application

### Git Repository Support
- [x] Git directory structure storage
- [x] Packfile handling
- [x] Repository extraction
- [x] Verification (HEAD, refs, objects)
- [ ] Testing with large repositories

### Optimization & Enhancement
- [ ] Hot path profiling
- [ ] Hash algorithm optimization (xxHash vs CRC32)
- [ ] Compression parameter tuning
- [ ] SQLite connection pooling
- [ ] LRU file cache
- [ ] Memory mapping for large archives
- [ ] Allocation overhead reduction

---

## Part VII: Error Handling

### 1. Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("Archive not found: {0}")]
    FileNotFound(String),
    
    #[error("Invalid archive format: {0}")]
    InvalidFormat(String),
    
    #[error("Corrupted data in {path}: expected CRC {expected:08x}, got {actual:08x}")]
    CorruptedData {
        path: String,
        expected: u32,
        actual: u32,
    },
    
    #[error("Unsupported compression method: {0}")]
    UnsupportedCompression(u8),
    
    #[error("Unsupported format version: {major}.{minor}")]
    UnsupportedVersion { major: u16, minor: u16 },
    
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    
    #[error("Decompression error: {0}")]
    Decompression(String),
}
```

### 2. Checksum Layers

1. **Header CRC32** (fast fail): Verifies header integrity before parsing
2. **Per-file CRC32**: Verifies each file after decompression
3. **End record checksum** (optional): Detects file truncation

### 3. Corruption Recovery

```rust
pub fn validate_archive(&self) -> Result<ValidationReport> {
    let mut report = ValidationReport::new();
    
    // Verify header
    if !self.verify_header_crc() {
        report.errors.push("Header CRC mismatch".to_string());
        return Ok(report);
    }
    
    // Verify each file
    for entry in &self.index.entry_array {
        match self.read_file(&entry.path) {
            Ok(_) => report.valid_files += 1,
            Err(e) => {
                report.corrupted_files.push(entry.path.clone());
                report.errors.push(format!("{}: {}", entry.path, e));
            }
        }
    }
    
    Ok(report)
}
```

---

## Part VIII: Distribution and Usage

### 1. GitHub Distribution

**Repository Structure**:
```
us-code-archive/
├── README.md
├── manifest.json          # Archive metadata
├── us-code-2024.eng       # Primary archive
├── us-code-2024.eng.sha256
└── versions/
    ├── us-code-2023.eng
    └── us-code-2022.eng
```

**manifest.json**:
```json
{
  "name": "US Code Archive",
  "version": "2024.1",
  "format_version": "0.1",
  "created": "2024-10-24T00:00:00Z",
  "size": 523419648,
  "files": 12847,
  "databases": ["index.db", "search.db", "metadata.db"],
  "content_version": 1,
  "checksums": {
    "sha256": "abc123...",
    "md5": "def456..."
  }
}
```

### 2. Download and Verification

```bash
# Download
wget https://github.com/manifest-humanity/us-code/releases/latest/download/us-code-2024.eng

# Verify
sha256sum -c us-code-2024.eng.sha256

# Use in application
npm install @manifest-humanity/engram
node -e "const {EngArchive} = require('@manifest-humanity/engram'); \
         const a = new EngArchive('./us-code-2024.eng'); \
         console.log(a.listFiles().slice(0, 10));"
```

### 3. Update Strategy

**Version Scheme**: `YYYY.MINOR` (e.g., 2024.1, 2024.2)
- Major version = year of data
- Minor version = incremental updates within year

**Delta Updates** (Future):
- Generate binary diffs between versions
- Use bsdiff or similar for efficient updates
- Store deltas in separate `.engdelta` files

---

## Part IX: Security Considerations

### 1. Archive Integrity

- Multi-layer checksums prevent undetected corruption
- Magic number prevents accidental processing of non-engram files
- Version checking prevents format mismatch

### 2. SQLite Injection Prevention

```typescript
// BAD: String concatenation
archive.querySqlite('index.db', `SELECT * FROM bills WHERE id = ${userInput}`);

// GOOD: Parameterized queries (future enhancement)
archive.querySqliteParam('index.db', 'SELECT * FROM bills WHERE id = ?', [userInput]);
```

**Current mitigation**: Application-level query sanitization required until parameterized query support is added.

### 3. Path Traversal Prevention

```rust
fn validate_path(path: &str) -> Result<()> {
    // Reject absolute paths
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(ArchiveError::InvalidPath("Absolute paths not allowed"));
    }
    
    // Reject parent directory references
    if path.contains("..") {
        return Err(ArchiveError::InvalidPath("Parent directory references not allowed"));
    }
    
    // Reject drive letters (Windows)
    if path.len() >= 2 && path.as_bytes()[1] == b':' {
        return Err(ArchiveError::InvalidPath("Drive letters not allowed"));
    }
    
    Ok(())
}
```

### 4. Resource Limits

- Maximum file size: 2^64 bytes (practically limited by available memory)
- Maximum path length: 255 bytes (extensible via path pool)
- Maximum archive size: 2^64 bytes
- Maximum entries: 2^32 entries

**Recommended limits**:
- Single file: 100MB (for in-memory decompression)
- Archive size: 2GB (for reasonable download times)
- Entry count: 100K files (for index build performance)

---

## Part X: Future Enhancements

### 1. Planned Features

- [ ] **Parameterized SQLite queries**: Prevent SQL injection
- [ ] **Streaming decompression**: For files >100MB
- [ ] **Encryption support**: AES-256-GCM per-file encryption
- [ ] **Extended attributes**: Store POSIX permissions, xattrs
- [ ] **Delta updates**: Binary diff between archive versions
- [ ] **Signing**: Digital signatures for archive authenticity
- [ ] **Compression auto-detection**: Choose optimal method per file
- [ ] **Path pool**: Support paths >255 bytes
- [ ] **Block-level deduplication**: Reduce redundant data

### 2. Format Evolution

**Version 1.1** (Next Release):
- Feature flags bitmap (required/optional)
- Extended attributes support
- Path pool for long paths

**Version 2.0** (Future):
- Breaking changes to format structure
- Encryption support
- Signing and verification

### 3. Performance Improvements

- Memory-mapped I/O for large archives
- Multi-threaded decompression
- SIMD optimizations for checksums
- Custom allocator for reduced fragmentation

---

## Appendix A: Complete Example

### Creating an Archive

```rust
use engram::{ArchiveWriter, CompressionMethod};

fn main() -> Result<()> {
    let mut writer = ArchiveWriter::new("output.eng")?;
    
    // Add manifest
    let manifest = r#"{"name": "Example Archive", "version": "1.0"}"#;
    writer.add_file("manifest.json", manifest.as_bytes(), CompressionMethod::None)?;
    
    // Add SQLite database
    let db_data = std::fs::read("index.db")?;
    writer.add_file("index.db", &db_data, CompressionMethod::Zstd)?;
    
    // Add text files
    for entry in std::fs::read_dir("./documents")? {
        let entry = entry?;
        let path = entry.path();
        let data = std::fs::read(&path)?;
        let relative_path = format!("documents/{}", path.file_name().unwrap().to_str().unwrap());
        writer.add_file(&relative_path, &data, CompressionMethod::Lz4)?;
    }
    
    // Add Git repository
    for entry in walkdir::WalkDir::new(".git") {
        let entry = entry?;
        if entry.file_type().is_file() {
            let path = entry.path();
            let data = std::fs::read(path)?;
            let relative_path = path.strip_prefix(".")?.to_str().unwrap();
            writer.add_file(relative_path, &data, CompressionMethod::Zstd)?;
        }
    }
    
    writer.finalize()?;
    
    println!("Archive created: output.eng");
    Ok(())
}
```

### Reading from Archive

```typescript
import { EngArchive } from '@manifest-humanity/engram';

async function main() {
    const archive = new EngArchive('./output.eng');
    
    // Read manifest
    const manifestData = await archive.readFileAsync('manifest.json');
    const manifest = JSON.parse(manifestData.toString('utf-8'));
    console.log('Manifest:', manifest);
    
    // Query database
    const bills = archive.querySqlite(
        'index.db',
        `SELECT title, year FROM bills 
         WHERE category = 'civic' 
         ORDER BY year DESC 
         LIMIT 10`
    );
    console.log('Recent bills:', bills);
    
    // List all documents
    const docs = archive.listFiles('documents/');
    console.log(`Found ${docs.length} documents`);
    
    // Extract Git repository
    archive.extractGit('./extracted-repo');
    console.log('Git repository extracted');
    
    // Validate archive integrity
    const report = archive.validate();
    console.log(`Validation: ${report.validFiles} valid, ${report.corruptedFiles.length} corrupted`);
}

main().catch(console.error);
```

---

## Appendix B: Format Comparison

| Feature                  | Engram   | ZIP      | ASAR     | SQLite-VFS |
|--------------------------|----------|----------|----------|------------|
| Random access            | ✓        | ✓        | ✓        | ✗          |
| Compression per-file     | ✓        | ✓        | ✗        | ✗          |
| SQLite querying          | ✓        | ✗        | ✗        | ✓          |
| No extraction required   | ✓        | ✗        | ✓        | ✓          |
| Streaming creation       | ✓        | ✓        | ✗        | N/A        |
| Git repository storage   | ✓        | ✓        | ✗        | ✗          |
| Multi-layer checksums    | ✓        | Limited  | Optional | ✗          |
| TOC location             | End      | End      | Start    | N/A        |
| Fixed-size entries       | ✓        | ✗        | ✗        | N/A        |
| Performance (read)       | Excellent| Good     | Excellent| Good       |
| Cross-platform           | ✓        | ✓        | ✓        | ✓          |

**Design Philosophy**: Engram combines the best features of each format - ZIP's streaming creation and compression, ASAR's speed and simplicity, and SQLite-VFS's queryability - while optimizing for civic data distribution use cases.

---

## Appendix C: References

### Standards and Specifications
- SQLite VFS: https://www.sqlite.org/vfs.html
- ZIP File Format: APPNOTE.TXT (PKWARE)
- PNG Signature: http://www.libpng.org/pub/png/spec/1.2/PNG-Rationale.html#R.PNG-file-signature

### Libraries
- sqlite-vfs: https://github.com/rkusa/sqlite-vfs
- rusqlite: https://github.com/rusqlite/rusqlite
- NAPI-RS: https://napi.rs
- ASAR: https://github.com/electron/asar

### Similar Projects
- sqlite_zstd_vfs: Compression VFS for SQLite
- Electron ASAR: Chromium archive format
- libarchive: Multi-format archive library

---

## License

This specification is released into the public domain or, where not applicable, under CC0 1.0 Universal.

## Contact

For questions, issues, or contributions:
- GitHub: https://github.com/manifest-humanity/engram
- Email: engineering@manifest-humanity.org
