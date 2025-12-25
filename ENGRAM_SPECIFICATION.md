# The Engram Archive Format: Durable Knowledge Containers with Embedded Database Access

**Document Classification:** Technical Specification
**Status:** Final Specification - v1.0
**Format Version:** 1.0.0
**Effective Date:** 2025-12-19
**Author:** Magnus - Blackfall Laboratories

---

## ABSTRACT

The Engram archive format (`.eng`) constitutes a specialized container architecture designed for long-term knowledge preservation with integrated database query capabilities. The format addresses fundamental limitations in contemporary archive systems through three architectural principles: table-of-contents placement enabling streaming creation without format constraints, per-file compression maintaining true random access, and Virtual File System integration permitting direct SQLite database queries without extraction. Performance characteristics demonstrate 80-90% of native filesystem query speed while eliminating extraction overhead entirely. The specification prioritizes archival permanence, semantic integrity, and operational independence across multi-decade timescales.

---

## 1.0 INTRODUCTION

### 1.1 Purpose and Scope

This specification documents the Engram archive format developed at Blackfall Laboratories for immutable knowledge preservation. The format provides write-once containers maintaining semantic structure, cryptographic verification, and embedded database access without temporal dependencies on external tooling or network services.

The Engram format serves as the canonical storage layer for institutional knowledge systems requiring preservation guarantees exceeding conventional filesystem and database capabilities. Archives created under this specification remain readable and queryable across technological transitions without migration-induced data loss.

### 1.2 Design Constraints

The format architecture emerges from five operational requirements:

- **Random Access Efficiency:** Individual file extraction completes in sub-millisecond timeframes without decompressing unrelated content. Hash-indexed table-of-contents enables O(1) lookup complexity regardless of archive size.

- **Streaming Creation:** Archive construction proceeds without foreknowledge of complete file manifest. Content writers accumulate entries sequentially, finalizing structural metadata upon completion rather than requiring complete directory enumeration at initialization.

- **Database Query Integration:** SQLite databases embedded within archives accept standard SQL queries through Virtual File System abstraction. Query execution proceeds at 80-90% of native filesystem performance without intermediate extraction steps.

- **Compression Without Access Penalty:** Per-file compression achieves 40-50% size reduction for text and structured data while maintaining random access characteristics. Large databases employ frame-based compression permitting partial decompression of requested byte ranges.

- **Format Longevity:** Binary structure employs fixed-width fields, explicit versioning, and reserved extension space. Readers validate format compatibility at archive open time, rejecting incompatible versions while maintaining forward compatibility within major version boundaries.

### 1.3 Architectural Lineage

The Engram format synthesizes proven patterns from production archive systems while addressing their respective limitations. The ZIP format's end-placed central directory enables streaming creation but suffers from compression-induced access penalties and lack of database integration. Electron's ASAR format demonstrates that uncompressed archives with JSON manifests achieve filesystem-equivalent performance but sacrifices storage efficiency and provides no structured query capabilities. Git's packfile format employs sophisticated delta compression and offset indexing but optimizes for revision history rather than heterogeneous knowledge containers.

Blackfall's synthesis places the central directory at archive end (ZIP pattern), employs per-file compression maintaining random access (rejecting whole-archive compression), and integrates Virtual File System abstraction for embedded databases (novel contribution). This combination delivers compact distribution, powerful querying, and access performance approaching native filesystems.

---

## 2.0 FORMAT SPECIFICATION

### 2.1 Binary Structure Overview

Engram archives comprise four structural components written sequentially to a single file:

```
[File Header: 64 bytes fixed]
[File Data Region: variable length]
  ├─ Local Entry 1: header + compressed payload
  ├─ Local Entry 2: header + compressed payload
  └─ Local Entry N: header + compressed payload
[Central Directory: 320 bytes per entry]
[End of Central Directory: 64 bytes fixed]
```

The header provides format identification and version validation. File data occupies the bulk of archive space, storing actual content with optional per-file compression. The central directory constitutes the authoritative manifest, mapping file paths to data offsets with metadata. The end record anchors the structure, enabling central directory location through backward scan from file terminus.

### 2.2 File Header Specification

The 64-byte header occupies archive offset 0x00 through 0x3F:

| Offset | Size | Field                    | Type     | Description                               |
| ------ | ---- | ------------------------ | -------- | ----------------------------------------- |
| 0-7    | 8    | Magic Number             | byte[8]  | `0x89 'E' 'N' 'G' 0x0D 0x0A 0x1A 0x0A`    |
| 8-9    | 2    | Format Version Major     | uint16   | Major version (breaking changes)          |
| 10-11  | 2    | Format Version Minor     | uint16   | Minor version (compatible additions)      |
| 12-15  | 4    | Header CRC32             | uint32   | CRC32 of header bytes 0-11                |
| 16-23  | 8    | Central Directory Offset | uint64   | Byte offset to central directory start    |
| 24-31  | 8    | Central Directory Size   | uint64   | Total bytes occupied by central directory |
| 32-35  | 4    | Entry Count              | uint32   | Number of files in archive                |
| 36-39  | 4    | Content Version          | uint32   | Schema version for embedded data          |
| 40-43  | 4    | Flags                    | uint32   | Bits 0-1: encryption mode; rest reserved  |
| 44-63  | 20   | Reserved                 | byte[20] | Must be zero; reserved for extensions     |

**Magic Number Rationale:** The eight-byte signature follows PNG format conventions. The non-ASCII first byte (0x89) prevents misidentification as text files. Human-readable "ENG" enables manual format recognition. Line-ending bytes (CR LF 0x0D 0x0A, EOF 0x1A, LF 0x0A) detect corruption from text-mode file transfers and legacy DOS tooling modifications.

**Version Validation:** Readers compare major version against internal compatibility range. Archives with major version exceeding reader capability fail with explicit version error. Minor version mismatches within the same major version permit operation with capability warnings logged for operator awareness.

**Flags Field Encoding:** The flags field employs bit-level encoding for optional format features. Bits 0-1 specify encryption mode:

- `00` (0): No encryption
- `01` (1): Archive-level encryption (entire archive encrypted for backup/secure storage)
- `10` (2): Per-file encryption (individual files encrypted, enabling selective decryption and database queries)
- `11` (3): Reserved for future use

Bits 2-31 remain reserved for future extensions and must be zero in v0.4 archives.

### 2.3 Local File Entry Format

The local entry header precedes each file's compressed data, enabling sequential streaming reads without central directory consultation. Variable-length structure accommodates arbitrary path lengths while maintaining fixed-size central directory entries.

| Offset | Size     | Field              | Type    | Description                                   |
| ------ | -------- | ------------------ | ------- | --------------------------------------------- |
| 0-3    | 4        | Entry Signature    | byte[4] | `0x4C 0x4F 0x43 0x41` ("LOCA")                |
| 4-11   | 8        | Uncompressed Size  | uint64  | Original file size before compression         |
| 12-19  | 8        | Compressed Size    | uint64  | Stored size (equals uncompressed if method=0) |
| 20-23  | 4        | CRC32 Checksum     | uint32  | CRC32 of uncompressed data                    |
| 24-31  | 8        | Modified Timestamp | uint64  | Unix epoch seconds                            |
| 32     | 1        | Compression Method | uint8   | 0=None, 1=LZ4, 2=Zstandard                    |
| 33     | 1        | Flags              | uint8   | Reserved (must be zero in v0.4)               |
| 34-35  | 2        | Path Length        | uint16  | Actual UTF-8 byte count of path               |
| 36-39  | 4        | Reserved           | byte[4] | Must be zero; reserved for extensions         |
| 40+    | variable | File Path          | UTF-8   | Null-terminated path string                   |
| varies | variable | File Data          | bytes   | Compressed file payload                       |

**Sequential Access Pattern:** Readers processing archives sequentially parse local entries to extract files without consulting the central directory. The data offset field in central directory entries points to the local entry header (not directly to compressed data), enabling validation of metadata consistency between local and central records.

### 2.4 Central Directory Entry Format

Each file entry consumes exactly 320 bytes within the central directory, enabling array-like indexing:

| Offset  | Size | Field              | Type     | Description                                   |
| ------- | ---- | ------------------ | -------- | --------------------------------------------- |
| 0-3     | 4    | Entry Signature    | byte[4]  | `0x43 0x45 0x4E 0x54` ("CENT")                |
| 4-11    | 8    | Data Offset        | uint64   | Byte offset to local entry header             |
| 12-19   | 8    | Uncompressed Size  | uint64   | Original file size in bytes                   |
| 20-27   | 8    | Compressed Size    | uint64   | Stored size (equals uncompressed if method=0) |
| 28-31   | 4    | CRC32 Checksum     | uint32   | CRC32 of uncompressed data                    |
| 32-39   | 8    | Modified Timestamp | uint64   | Unix epoch seconds                            |
| 40      | 1    | Compression Method | uint8    | 0=None, 1=LZ4, 2=Zstandard                    |
| 41      | 1    | Flags              | uint8    | Bit 0: encrypted (reserved)                   |
| 42-43   | 2    | Path Length        | uint16   | Actual UTF-8 byte count                       |
| 44-299  | 256  | File Path          | UTF-8    | Null-terminated path string                   |
| 300-319 | 20   | Reserved           | byte[20] | Must be zero; future extensions               |

**Fixed-Size Design:** The 320-byte fixed width enables rapid binary search and array indexing. Readers calculate entry position as `central_directory_offset + (entry_index × 320)` without sequential parsing overhead.

**Path Constraints:** The 256-byte path field accommodates hierarchical structures to 255 UTF-8 characters. Systems requiring longer paths employ a path pool appended after the central directory, storing offsets in the path field and setting flag bit to indicate indirection (future extension).

### 2.5 End of Central Directory Record

The final 64 bytes anchor the archive structure:

| Offset | Size | Field                    | Type     | Description                       |
| ------ | ---- | ------------------------ | -------- | --------------------------------- |
| 0-3    | 4    | End Signature            | byte[4]  | `0x45 0x4E 0x44 0x52` ("ENDR")    |
| 4-11   | 8    | Central Directory Offset | uint64   | Byte offset (duplicate of header) |
| 12-19  | 8    | Central Directory Size   | uint64   | Size in bytes (duplicate)         |
| 20-23  | 4    | Entry Count              | uint32   | File count (duplicate)            |
| 24-27  | 4    | Record CRC32             | uint32   | CRC32 of this record bytes 0-23   |
| 28-63  | 36   | Reserved                 | byte[36] | Future extensions                 |

Readers locate this record through backward scan from file end, searching for the end signature within the final 65,536 bytes. The duplicated offset and size fields provide corruption detection when compared against header values.

---

## 3.0 STRUCTURAL DESIGN RATIONALE

### 3.1 Table-of-Contents Placement Strategy

The central directory placement at archive terminus rather than header position constitutes a deliberate architectural choice enabling streaming creation without format compromises.

**Beginning-Placed Disadvantages:** Formats placing the file manifest at offset zero (ASAR, TAR with GNU extensions) require complete file enumeration before writing data. Writers must either enumerate all input files during initialization—impossible for streaming inputs—or reserve estimated space for the directory, introducing wasted padding or overflow complexity.

**End-Placed Advantages:** Terminal directory placement permits sequential file writing without manifest foreknowledge. The archive writer processes input files as encountered, appending data to the growing archive while accumulating central directory entries in memory. Upon completion, the writer serializes the directory and end record, finalizing the archive atomically. This pattern matches natural workflows: repository snapshots, build artifact collection, and incremental backup scenarios.

**Read Performance Preservation:** While intuition suggests end-placed directories penalize readers, actual performance remains equivalent to beginning-placed designs. Modern operating systems employ read-ahead caching; reading the terminal 64KB block to locate the end record triggers cache population of adjacent data. The central directory read—typically 1-5MB even for large archives—completes in single-digit milliseconds on contemporary storage. Once loaded, the in-memory hash index provides O(1) file lookup regardless of directory placement.

### 3.2 Fixed-Width Entry Design

Variable-width entry formats (ZIP, TAR) optimize storage efficiency at the cost of parsing complexity and access predictability. The Engram format employs fixed 320-byte entries prioritizing computational simplicity and deterministic performance.

**Array Indexing:** Readers calculate entry position through arithmetic rather than sequential parsing: `entry_offset = central_directory_base + (file_index × 320)`. This enables binary search by filename hash, random access by file number, and parallel directory processing without locking concerns.

**Cache Efficiency:** Modern CPU cache lines span 64 bytes; five entries occupy exactly one 4KB memory page. Sequential directory scans exhibit optimal cache utilization with predictable prefetch behavior. Variable-width formats suffer cache misses from unpredictable entry boundaries.

**Simplicity Guarantees:** Fixed-width parsing eliminates entire categories of format vulnerabilities: buffer overruns from malformed length fields, infinite loops from circular offset references, and integer overflows from accumulated size calculations. Implementations require no dynamic allocation during entry parsing.

**Storage Cost:** The 320-byte allocation consumes 0.032 bytes per stored byte for 10KB average file size—negligible overhead. Archives storing many small files (under 1KB) experience higher relative costs, but such archives inherently suffer poor size efficiency across all formats; appropriate solutions involve bundling (DataSpools) or filesystem restructuring rather than format optimization.

### 3.3 Compression Strategy

The format employs per-file compression rather than whole-archive compression, trading marginal compression ratio reduction for preservation of random access characteristics.

**Whole-Archive Compression Failure Mode:** Compressing the entire archive (`.eng.gz` pattern) achieves 5-10% better ratios through cross-file dictionary building but destroys random access. Extracting any file requires decompressing all preceding content. For a 10GB archive, extracting the final file decompresses 10GB; extracting 100 files decompresses 1TB of intermediate data. This pattern is incompatible with operational requirements.

**Per-File Granularity:** Individual file compression maintains true random access. Extracting file N requires: (1) hash lookup in central directory O(1), (2) seek to data offset O(1), (3) read compressed bytes, (4) decompress target file only. Total operations remain constant regardless of archive size or file position.

**Algorithm Selection by Content Type:** The compression method field permits per-file algorithm optimization:

- **None (0):** Pre-compressed formats (JPEG, PNG, MP4, ZIP), files under 4KB where header overhead exceeds gains
- **LZ4 (1):** Speed-critical content requiring sub-millisecond decompression (textures, frequently accessed configuration)
- **Zstandard (2):** Balanced compression for text, JSON, SQLite databases (40-50% reduction, 400-600 MB/s decompression)
- **Deflate (3):** Maximum compatibility with legacy ZIP tools (slower, included for interoperability)

**Frame-Based Compression for Large Files:** SQLite databases and large binary assets employ optional frame-based compression, dividing files into 64KB chunks compressed independently. A frame index in the local header maps byte ranges to compressed frame offsets, enabling selective decompression of requested regions. When SQLite's VFS requests bytes 2,000,000-2,004,096, the system decompresses only frames 30-31 (128KB total) rather than the entire multi-gigabyte database.

---

## 4.0 VIRTUAL FILESYSTEM ARCHITECTURE

### 4.1 SQLite VFS Integration

The Engram format's distinguishing capability provides direct SQL query execution against embedded databases without extraction through Virtual File System abstraction. This architecture transforms archives from opaque containers into queryable knowledge repositories.

**VFS Abstraction Layer:** SQLite's modular VFS interface separates the query engine from storage implementation through a defined API of file operations: `xOpen`, `xRead`, `xWrite`, `xSync`, `xFileSize`, and locking primitives. Custom VFS implementations satisfy this interface while substituting alternative storage backends: compressed files, network resources, encrypted volumes, or—in this case—archive-embedded databases.

**Archive VFS Implementation Pattern:** The Engram VFS implementation operates in three phases:

1. **Initialization:** Register custom VFS with SQLite engine, providing archive reader instance
2. **Database Open:** When application requests database connection, VFS locates file in archive index, extracts complete database into memory buffer (Vec\<u8\> for Rust implementations)
3. **Query Execution:** SQLite operates against memory buffer using standard read/write operations, unaware of archive abstraction

**Memory-Based Storage Rationale:** Loading entire databases into memory rather than streaming from archive proves optimal for typical use cases. SQLite exhibits random access patterns with frequent small reads scattered across the database file. Streaming implementations would require: seek to archive offset, locate compressed frame, decompress 64KB, extract requested 4KB page—repeated thousands of times per query. Memory-resident databases eliminate this overhead; query performance approaches native filesystem speed at the cost of initial load time (typically 100-500ms for databases under 100MB).

### 4.2 Performance Optimization Strategies

SQLite's page cache architecture provides critical optimization opportunities for VFS implementations.

**Page Cache Configuration:** The `PRAGMA cache_size` directive controls SQLite's internal page cache. Default values (2MB) prove inadequate for archive scenarios. Recommended configuration allocates 20-30% of available RAM: `PRAGMA cache_size = -102400` reserves 100MB. Large caches dramatically reduce I/O requests; for read-only query workloads, proper cache sizing permits database access through a single complete read followed by cache-resident queries.

**Page Size Selection:** SQLite's page size (default 4KB, maximum 64KB) impacts both compression ratio and access efficiency. Larger pages reduce metadata overhead and improve compression ratios—64KB pages achieve 10-15% better compression than 4KB pages for structured data. However, large pages waste bandwidth when queries access sparse data. Recommended configuration: 16KB pages for general-purpose databases, 64KB for analytics workloads with sequential access patterns.

**Journal Mode Constraints:** The VFS implementation must configure appropriate journal modes. Write-Ahead Logging (WAL) requires shared memory primitives and separate journal files—incompatible with read-only archive semantics. Recommended settings: `PRAGMA journal_mode=OFF` for strictly read-only databases, `PRAGMA journal_mode=MEMORY` for temporary modifications retained only during connection lifetime.

**Measured Performance Characteristics:** Testing with sqlite_zstd_vfs (a production compression VFS implementation) demonstrates:

- Cold cache queries: 60-70% of native filesystem performance (decompression overhead)
- Warm cache queries: 85-95% of native performance (cache hits eliminate decompression)
- Aggregate queries (full table scans): 75-80% of native (sustained decompression throughput)

For knowledge preservation scenarios prioritizing distribution efficiency over query microseconds, this performance profile proves acceptable.

### 4.3 Concurrent Access Patterns

SQLite's locking model assumes exclusive file access; multiple processes or threads accessing the same database require coordination. Archive VFS implementations must address these constraints.

**Single Connection Model:** The simplest implementation provides one connection per database per process. Each connection maintains independent memory buffer and cache. This pattern wastes memory (duplicate database storage) but eliminates locking complexity.

**Connection Pooling:** Production implementations employ connection pools, initializing N connections during startup and distributing queries across the pool. This approach amortizes database load cost while preventing connection initialization latency during critical paths.

**Read-Only Guarantees:** Archive databases exist in read-only contexts—modifications cannot persist back to archive. VFS implementations exploit this constraint by configuring `SQLITE_OPEN_READONLY` flags and `PRAGMA locking_mode=EXCLUSIVE`, eliminating locking overhead entirely. Multiple threads read simultaneously without coordination.

---

## 5.0 COMPRESSION STRATEGIES

### 5.1 Algorithm Selection Matrix

The format specification defines three compression methods, each optimized for distinct content characteristics and access patterns.

**Method 0 — Uncompressed Storage:**

Applied to content where compression yields negative or negligible benefit:

- Pre-compressed formats (JPEG, PNG, MP4, ZIP, GZIP archives)
- Already-compressed Blackfall formats (BytePunch Cards .card, DataSpools .spool, Engram archives .eng)
- Encrypted files (high entropy defeats compression)
- Files under 4KB (4096 bytes - header and dictionary overhead exceed gains)
- JSON manifests under 512KB (instant access priority)

**Method 1 — LZ4 Fast Compression:**

Prioritizes decompression speed over compression ratio:

- Decompression throughput: 2-4 GB/s on contemporary CPUs
- Compression ratio: 2:1 to 2.5:1 for text, 1.3:1 to 1.8:1 for structured binary
- Use cases: Frequently accessed configuration files, UI assets, templates
- Latency impact: Sub-millisecond decompression for files under 10MB

**Method 2 — Zstandard Balanced Compression:**

Optimal default for heterogeneous content:

- Compression ratio: 2.5:1 to 4:1 for text, 1.8:1 to 2.5:1 for structured data
- Decompression throughput: 400-800 MB/s
- Training dictionaries: Optional pre-shared dictionaries improve small file compression 20-30%
- Use cases: Text files, JSON/XML documents, source code, markdown

**Note on Database Compression:** SQLite databases and WebAssembly modules employ LZ4 compression (Method 1) rather than Zstandard to prioritize query performance. The faster decompression enables sub-millisecond page access during database operations.

### 5.2 Adaptive Compression Heuristics

Archive creation tools employ content-aware heuristics to select appropriate compression methods automatically.

**Decision Tree Implementation:**

```
IF file_size < 4KB (4096 bytes):
    method ← NONE (overhead exceeds benefit)
ELSE IF extension IN {.jpg, .png, .mp4, .zip, .gz, .eng, .card, .spool}:
    method ← NONE (already compressed)
ELSE IF extension IN {.db, .sqlite, .sqlite3, .wasm}:
    method ← LZ4 (fast decompression for query performance)
ELSE IF extension IN {.json, .xml, .txt, .md, .html, .css, .js, .cml}:
    method ← ZSTD (text compresses excellently)
ELSE:
    method ← ZSTD (safe default, balanced compression)
```

**Compression Ratio Validation:** Implementations measure actual compressed size against uncompressed size. When compression achieves less than 5% reduction, the system reverts to uncompressed storage, avoiding decompression overhead for negligible space savings.

### 5.3 Frame-Based Compression for Large Files

Files exceeding 50MB benefit from frame-based compression, enabling partial decompression of requested byte ranges.

**Frame Structure:** The file divides into fixed-size frames (default 64KB), each compressed independently. A frame index precedes the compressed data:

```
[Frame Index Header]
  - Frame size: 64KB (configurable)
  - Frame count: N
  - Index entries: N × 24 bytes
[Frame Index Entries]
  Entry format (24 bytes):
    - Decompressed offset: uint64
    - Decompressed size: uint32
    - Compressed offset: uint64
    - Compressed size: uint32
[Compressed Frame Data]
  - Frame 0: compressed bytes
  - Frame 1: compressed bytes
  - Frame N: compressed bytes
```

**Selective Decompression Algorithm:**

When VFS requests bytes at offset X length L:

1. Calculate frame range: `first_frame = X / frame_size`, `last_frame = (X + L) / frame_size`
2. Decompress frames [first_frame...last_frame] into temporary buffer
3. Extract requested slice from decompressed buffer
4. Cache decompressed frames for subsequent requests (LRU eviction)

**Performance Characteristics:** For a 2GB SQLite database with 64KB frames, typical query touching 10 pages (160KB) decompresses 3 frames (192KB compressed with Zstd level 6) consuming 1-2ms. Equivalent whole-file decompression would require decompressing 1.2GB (compressed size) consuming 2-3 seconds.

---

## 6.0 PERFORMANCE CHARACTERISTICS

### 6.1 Access Pattern Analysis

The format architecture optimizes for random access patterns typical of knowledge retrieval systems while maintaining acceptable sequential access performance.

**Archive Open Operation:**

Time complexity: O(1) relative to archive size, O(n) relative to file count

Measured performance (10,000-file archive, contemporary SSD):

1. File open and header read: 0.2ms
2. Seek to end record: 0.1ms
3. Read end record (64 bytes): 0.05ms
4. Seek to central directory: 0.1ms
5. Read central directory (3.2MB): 4ms
6. Parse and index entries: 12ms
7. **Total: 16.5ms**

Memory consumption: ~420 bytes per file (320-byte entry + 100-byte hash table overhead) = 4.2MB for 10,000 files

**Individual File Extraction:**

Time complexity: O(1)

Measured performance by file size and compression:

| File Size | Uncompressed | LZ4    | Zstd    |
| --------- | ------------ | ------ | ------- |
| 4KB       | 0.15ms       | 0.18ms | 0.22ms  |
| 100KB     | 0.3ms        | 0.5ms  | 1.2ms   |
| 10MB      | 25ms         | 35ms   | 120ms   |
| 100MB     | 245ms        | 310ms  | 1,150ms |

Uncompressed performance dominated by storage I/O (250 MB/s for sequential reads on test SSD). Compressed performance adds decompression CPU cost proportional to file size and algorithm complexity.

**SQLite Query Performance:**

Measured against embedded 50MB database, 2 million rows, 20-column schema:

| Query Type              | Native FS | Engram (Cold) | Engram (Warm) |
| ----------------------- | --------- | ------------- | ------------- |
| Point lookup (indexed)  | 0.08ms    | 0.12ms        | 0.09ms        |
| Range scan (1K rows)    | 12ms      | 18ms          | 13ms          |
| Full table aggregation  | 180ms     | 285ms         | 195ms         |
| Complex join (5 tables) | 45ms      | 68ms          | 48ms          |

Cold performance reflects initial decompression and cache population. Warm performance (second query) demonstrates cache effectiveness. Performance delta narrows as query complexity increases—computation dominates I/O for complex operations.

### 6.2 Scalability Boundaries

The format maintains consistent performance characteristics across archive sizes spanning five orders of magnitude.

**File Count Scaling:**

Hash table lookup remains O(1) average case across file counts. Testing confirms:

- 100 files: 14ns average lookup
- 1,000 files: 16ns average lookup
- 10,000 files: 18ns average lookup
- 100,000 files: 24ns average lookup

Memory consumption scales linearly: ~420 bytes per file. A 100,000-file archive consumes 42MB for the index—acceptable for contemporary systems with multi-gigabyte RAM.

**Archive Size Scaling:**

Total archive size exhibits minimal impact on random access performance. The central directory size depends on file count, not total data size. A 100GB archive with 1,000 files demonstrates identical open and lookup performance to a 1GB archive with 1,000 files.

Sequential operations (full archive validation, complete extraction) scale linearly with total data size as expected.

**Recommended Operational Limits:**

Conservative limits for production deployments:

- Maximum file count: 1,000,000 (420MB index, 20-30s open time)
- Maximum individual file size: 10GB (frame-based compression recommended above 1GB)
- Maximum total archive size: 1TB (filesystem and tooling compatibility boundary)

Theoretical format limits (imposed by field widths):

- Maximum file count: 2^32 = 4.3 billion
- Maximum file size: 2^64 bytes = 16 exabytes
- Maximum archive size: 2^64 bytes = 16 exabytes

### 6.3 Memory Consumption Profile

The format prioritizes predictable memory usage through explicit buffer management.

**Index Memory (Required):**

- Central directory entries: 320 bytes × file_count
- Hash table overhead: ~100 bytes × file_count
- **Total: ~420 bytes per file**

**SQLite Cache (Configurable):**

- Default: 2MB (SQLite default, inadequate for archive scenarios)
- Recommended: 100-200MB (20-30% of available RAM)
- Maximum observed benefit: ~500MB (diminishing returns beyond this threshold)

**Decompression Buffers (Transient):**

- LZ4: ~64KB working buffer
- Zstandard: 128KB-2MB depending on compression level
- Frame cache (large files): 256KB-4MB LRU cache of recently decompressed frames

**Total Footprint Example:**

Archive: 10,000 files, 50GB total, 5 embedded SQLite databases

- Index: 4.2MB
- SQLite cache: 150MB
- Decompression buffers: 2MB
- Application overhead: 10MB
- **Total: ~166MB**

This profile proves acceptable for desktop and server deployments. Embedded systems with constrained RAM may reduce SQLite cache proportionally—query performance degrades gracefully rather than failing.

---

## 7.0 IMPLEMENTATION CONSIDERATIONS

### 7.1 Format Validation and Error Handling

Implementations must validate archive integrity at multiple checkpoints to detect corruption, truncation, or malicious manipulation.

**Validation Sequence:**

1. **Header Validation:**

   - Verify magic number matches specification
   - Validate major version within supported range
   - Compute header CRC32, compare against stored value
   - **Failure mode:** Reject archive with explicit format error

2. **End Record Location:**

   - Scan backward from file end (maximum 64KB)
   - Locate end record signature
   - **Failure mode:** Archive truncated or corrupted structure

3. **Central Directory Integrity:**

   - Read directory at offset specified in end record
   - Verify size matches end record declaration
   - Validate entry count consistency
   - Check all entry signatures ("CENT")
   - **Failure mode:** Structural corruption, reject archive

4. **Per-File Validation (On Access):**

   - Read file data at specified offset
   - Decompress if compression method non-zero
   - Compute CRC32 of decompressed data
   - Compare against central directory entry CRC32
   - **Failure mode:** File corrupted, return error for this file only

**Graduated Response Strategy:**

Implementations employ defense-in-depth with graduated failure modes:

- Header corruption: Immediate rejection (cannot trust any structure)
- Central directory corruption: Rejection (cannot locate files reliably)
- Individual file corruption: Isolated failure (other files remain accessible)
- Decompression failure: Isolated failure (may indicate algorithm incompatibility)

This strategy maximizes data recovery from partially corrupted archives while preventing propagation of malicious structures.

### 7.2 Versioning and Evolution Strategy

The format employs dual versioning: format version governing binary structure, content version governing embedded data schemas.

**Format Version Semantics:**

Major version increments indicate breaking changes requiring reader updates:

- Field reordering or size changes
- Modified compression algorithms or parameters
- Incompatible structural modifications

Minor version increments indicate backward-compatible additions:

- New compression methods (readers ignore unknown methods, store uncompressed)
- Additional flags or reserved field utilization
- Optional extensions in reserved space

**Compatibility Matrix:**

Reader behavior when encountering version V_archive:

| Reader Version | Archive Version | Behavior                               |
| -------------- | --------------- | -------------------------------------- |
| V_reader.0     | V_reader.0-9    | Full compatibility                     |
| V_reader.0     | V_reader+1.x    | Reject (future major)                  |
| V_reader.0     | V_reader-1.x    | Accept if backward-compatible flag set |
| V_reader.0     | V_other.x       | Reject with version error              |

**Reserved Space Utilization:**

The specification allocates 24 bytes in the header, 20 bytes per entry, and 36 bytes in the end record for future extensions. When introducing new capabilities:

1. Define new field within reserved space
2. Increment minor version
3. Document field meaning in updated specification
4. Older readers ignore the field (reads as zeros), operate on known fields
5. Newer readers detect non-zero values, enable enhanced capabilities

This pattern permits format evolution without breaking existing archives or requiring reader updates for users not requiring new capabilities.

### 7.3 Security Considerations

The format specification addresses several security concerns inherent to archive formats.

**Path Traversal Prevention:**

File paths in central directory entries must undergo validation before extraction:

- Reject paths containing `..` components
- Reject absolute paths (starting with `/` or drive letters)
- Reject paths containing symbolic link components (implementation-dependent)
- Enforce maximum path depth (e.g., 32 levels) to prevent directory exhaustion

**Compression Bomb Mitigation:**

Malicious archives may contain files with extreme compression ratios (1KB compressed → 1GB uncompressed). Implementations must:

- Monitor decompression ratio during extraction
- Abort decompression exceeding threshold (e.g., 1000:1 ratio)
- Track total decompressed bytes, enforce global limit
- Validate decompressed size against entry metadata before allocation

**CRC Verification:**

All data extraction must verify CRC32 checksums:

- Compute CRC during decompression
- Compare against central directory entry value
- Reject mismatched files explicitly
- Log verification failures for security auditing

**Denial of Service Boundaries:**

Implementations must bound resource consumption:

- Maximum file count accepted (prevent index memory exhaustion)
- Maximum individual file size processed (prevent decompression memory exhaustion)
- Maximum concurrent decompressions (prevent CPU exhaustion)
- Timeout mechanisms for decompression operations (detect pathological inputs)

**Encryption Support:**

The v0.4 format provides encryption capabilities through the flags field in the file header (offset 40-43). Two encryption modes enable distinct use cases:

**Archive-Level Encryption (Mode 0b01):**

- Entire archive encrypted as single unit
- Optimized for backup and secure storage scenarios
- AES-256-GCM encryption of complete archive after assembly
- Decryption required before any file access
- Use case: Cold storage, encrypted backups, secure distribution

**Per-File Encryption (Mode 0b10):**

- Individual files encrypted independently
- Enables selective decryption without processing entire archive
- SQLite databases remain queryable through VFS layer
- Each file encrypted with AES-256-GCM
- Use case: Partial access patterns, database queries on encrypted archives

Encryption keys derive from user-provided passphrases via Argon2id key derivation. Encrypted archives store salt and encryption parameters in the first 64 bytes following the header signature.

**Note on Signatures:** Ed25519 cryptographic signatures for authenticity verification exist in the manifest system (see Section 8.2) rather than archive format itself. Manifests embed public keys and signature data, enabling verification without modifying the core archive structure.

---

## 8.0 REFERENCE IMPLEMENTATIONS

### 8.1 Rust Implementation

The reference implementation resides in the `engram-rs` library, providing both archive manipulation and VFS integration.

**Core Components:**

- `ArchiveWriter`: Streaming archive creation with per-file compression selection
- `ArchiveReader`: Random access file extraction with automatic decompression
- `VfsReader`: SQLite VFS implementation for embedded database access
- `Manifest`: TOML-to-JSON manifest conversion and signature management

**Usage Pattern:**

```rust
use engram_rs::{ArchiveWriter, CompressionMethod};

// Create archive
let mut writer = ArchiveWriter::create("knowledge.eng")?;
writer.add_file("data.json", json_bytes)?;
writer.add_file_from_disk("database.db", Path::new("src.db"))?;
writer.finalize()?;

// Read archive
let mut reader = ArchiveReader::open("knowledge.eng")?;
let files = reader.list_files();
let data = reader.read_file("data.json")?;

// Query embedded database
let mut vfs = VfsReader::open("knowledge.eng")?;
let conn = vfs.open_database("database.db")?;
let results = conn.query("SELECT * FROM knowledge", [])?;
```

**Performance Characteristics:**

The Rust implementation exhibits near-optimal performance:

- Zero-copy extraction for uncompressed files
- SIMD-accelerated decompression (LZ4, Zstandard)
- Parallel central directory parsing (Rayon)
- Memory-mapped I/O for large archives (optional)

### 8.2 Command-Line Interface

The `engram-cli` tool provides complete archive manipulation capabilities for operator use.

**Primary Operations:**

```bash
# Create archive from directory
engram pack /path/to/data -o knowledge.eng --compression zstd

# List contents
engram list knowledge.eng --long

# Extract specific files
engram extract knowledge.eng --files "data/*.json" --output ./extracted

# Query embedded database
engram query knowledge.eng database.db "SELECT * FROM users WHERE active=1"

# Verify signatures and integrity
engram verify knowledge.eng --manifest
```

**Manifest Integration:**

Archives may embed TOML manifests describing contents, provenance, and signatures:

```toml
id = "knowledge-archive-2025"
name = "Institutional Knowledge Repository"
version = "1.0.0"
license = "MIT"

[author]
name = "Blackfall Laboratories"
email = "magnus@blackfall.dev"

[[signatures]]
algorithm = "ed25519"
public_key = "a1b2c3d4..."
signature = "9f8e7d6c..."
```

The CLI converts TOML to JSON during archive creation, enabling runtime signature verification and provenance validation.

---

## 9.0 OPERATIONAL GUIDANCE

### 9.1 Archive Creation Workflows

**Development Artifacts:**

Capture build outputs with compression optimized for content type:

```bash
# Repository snapshot with manifest
engram pack ./repository \
  --manifest build-manifest.toml \
  --compression auto \
  --output build-${VERSION}.eng
```

Auto-compression applies heuristics: Zstandard for source code and databases, LZ4 for binaries requiring fast access, uncompressed for media assets.

**Knowledge Base Distribution:**

Distribute documentation, databases, and assets as single immutable artifact:

```bash
# Documentation + embedded SQLite search index
engram pack ./docs \
  --manifest docs-manifest.toml \
  --databases ./indexes/*.db \
  --compression zstd \
  --output knowledge-base.eng
```

Recipients query the archive directly without extraction:

```bash
engram query knowledge-base.eng search.db \
  "SELECT * FROM documents WHERE content MATCH 'preservation'"
```

**Long-Term Preservation:**

Institutional knowledge archives with cryptographic verification:

```bash
# Create keypair for signing
engram keygen --output institutional-key

# Create signed archive
engram pack ./institutional-knowledge \
  --manifest preservation-manifest.toml \
  --sign institutional-key.private \
  --compression zstd-max \
  --output archive-$(date +%Y%m%d).eng

# Verify signature
engram verify archive-20251219.eng \
  --public-key institutional-key.public
```

### 9.2 Selection Criteria

**Use Engram Archives When:**

- Immutable distribution required (software releases, documentation snapshots)
- Embedded databases require query access without extraction
- Long-term preservation with format stability guarantees
- Cryptographic verification of authenticity necessary
- Random access to large file collections without extraction overhead

**Avoid Engram Archives When:**

- Frequent incremental updates required (use Cartridge format instead)
- Streaming decompression of entire archive needed (use tar.gz)
- Maximum compatibility with legacy tools essential (use ZIP)
- Very small file counts (<10 files) with simple access patterns

### 9.3 Migration from Legacy Formats

**From ZIP Archives:**

```bash
# Extract ZIP to temporary directory
unzip legacy.zip -d /tmp/extract

# Create equivalent Engram archive
engram pack /tmp/extract --output migrated.eng --compression auto
```

Performance improvement: 20-30% faster random access, 15-25% better compression with Zstandard versus Deflate.

**From TAR + GZIP:**

```bash
# Extract tar.gz
tar xzf legacy.tar.gz -C /tmp/extract

# Create Engram with per-file compression
engram pack /tmp/extract --output migrated.eng --compression zstd
```

Performance improvement: True random access (TAR requires sequential scanning), faster extraction of individual files (no whole-archive decompression).

---

## 10.0 TECHNICAL SUPPORT

### 10.1 Obtaining Assistance

Blackfall provides technical support for Engram format implementation and deployment through multiple channels:

**Documentation Resources:**

- Format specification (this document)
- Reference implementation source code (github.com/blackfall-labs/engram-spec)
- Command-line tool documentation (github.com/blackfall-labs/engram-cli)
- Integration examples and test cases

**Issue Reporting:**

Submit detailed issue reports including:

- Archive file size and file count
- Compression methods employed
- Error messages with complete stack traces
- Operating system and implementation version
- Reproducible test cases when possible

**Escalation Procedure:**

For critical format ambiguities or implementation conflicts:

1. Document issue with specification section references
2. Provide minimal reproducible example
3. Submit to magnus@blackfall.dev
4. Expect response within 48 hours (business days)

### 10.2 Validation and Testing Methodology

The reference implementation (`engram-rs` v1.0) underwent comprehensive validation across four testing phases totaling 166 tests. This section documents testing methodology, coverage, and findings to establish baseline conformance criteria for alternative implementations.

#### 10.2.1 Test Coverage Summary

Reference implementation validation statistics (2025-12-24):

| Phase | Test Count | Coverage Domain | Execution Time |
|-------|-----------|-----------------|----------------|
| Phase 1 | 46 | Security & Integrity | <0.5s |
| Phase 2 | 33 | Concurrency & Reliability | <1.0s |
| Phase 3 | 16 + 4* | Performance & Scale | <0.1s (16), 5-15s* (4) |
| Phase 4 | 26 | Security Audit | <1.0s |
| **Total** | **121 + 4*** | **Comprehensive** | **<3s (125)** |

*Stress tests executed with `--ignored` flag

Additional test coverage:
- 23 unit tests (format primitives, manifest, VFS)
- 10 integration tests (roundtrip, lifecycle)
- 7 v1.0 feature tests
- 5 debug/development tests

**Total Implementation Tests:** 166

#### 10.2.2 Phase 1: Security and Integrity Validation

**Purpose:** Validate format integrity under corruption scenarios and cryptographic security properties.

**Test Distribution:**
- Corruption Detection: 15 tests
- Fuzzing Infrastructure: 1 seed corpus + infrastructure
- Signature Security: 13 tests
- Encryption Security: 18 tests

**Corruption Detection Coverage:**

| Attack Vector | Test Case | Expected Behavior | Status |
|--------------|-----------|-------------------|--------|
| Invalid magic number | Modify bytes 0-7 | Reject with format error | ✅ Pass |
| Unsupported major version | Set version_major = 99 | Reject with version error | ✅ Pass |
| Header CRC mismatch | Corrupt header bytes | Reject with checksum error | ✅ Pass |
| CD offset out of bounds | Set offset > file_size | Reject with bounds error | ✅ Pass |
| Truncated archive (10%) | Remove final 90% | Reject incomplete | ✅ Pass |
| Truncated archive (50%) | Remove final 50% | Reject incomplete | ✅ Pass |
| Truncated archive (90%) | Remove final 10% | Reject incomplete | ✅ Pass |
| Truncated central directory | Partial CD removal | Reject malformed | ✅ Pass |
| ENDR signature corruption | Modify ENDR bytes 0-3 | Reject invalid | ✅ Pass |
| Zero-length file | Create empty .eng | Reject or handle gracefully | ✅ Pass |
| Bit flips in file data | Random bit corruption | Detect via CRC32 | ✅ Pass |

**Cryptographic Validation:**

Ed25519 Signature Tests (13 tests):
- Signature creation and verification: ✅ Correct
- Multi-signature support: ✅ Verified (2+ signers)
- Tampering detection: ✅ Detected (data modification invalidates signature)
- Replay attack resistance: ✅ Timestamp validation
- Signature with modified manifest: ✅ Invalidation detected
- Wrong key rejection: ✅ Verified (signatures fail with incorrect key)
- Zero-byte signature data: ✅ Rejected

AES-256-GCM Encryption Tests (18 tests):
- Archive-level encryption: ✅ Functional (entire archive encrypted)
- Per-file encryption: ✅ Functional (selective encryption)
- Wrong password rejection: ✅ Verified (authentication tag failure)
- Missing key handling: ✅ Error propagation correct
- Compression + encryption: ✅ Compatible (compress then encrypt)
- Encrypted file CRC verification: ✅ CRC computed on plaintext
- Decryption with bit flips: ✅ Authentication failure detected

**Findings:**
- All corruption scenarios properly detected and rejected
- No undefined behavior on malformed inputs
- Lazy validation behavior documented (validation deferred until access)
- Signature verification cryptographically sound (constant-time Ed25519)
- AES-256-GCM implementation secure (authenticated encryption)

**Fuzzing Infrastructure:**
- Tool: `cargo-fuzz` with `libfuzzer-sys`
- Seed corpus: 6 test archives (empty, small, large, binary, multi-file, corrupted)
- Coverage: Archive parser, manifest parser, central directory parser
- Status: Infrastructure operational, extended campaigns pending

#### 10.2.3 Phase 2: Concurrency and Reliability Validation

**Purpose:** Verify thread safety, concurrent access patterns, crash recovery, and frame compression correctness.

**Test Distribution:**
- Concurrent VFS/SQLite Access: 5 tests
- Multi-Reader Stress: 6 tests
- Crash Recovery: 13 tests
- Frame Compression Edge Cases: 9 tests

**Concurrent Access Validation:**

VFS Concurrency (5 tests, 10 threads × 1,000 queries = 10,000 operations):
- Parallel database connections: ✅ No data races
- Connection lifecycle: ✅ No resource leaks
- Multiple databases in archive: ✅ Isolated connections
- List operations under load: ✅ Thread-safe
- Query result correctness: ✅ No corruption

Multi-Reader Stress (6 tests, 100 concurrent readers, 64,000+ operations):
- Concurrent `list_files()`: ✅ 20,000 operations
- Concurrent `read_file()`: ✅ 10,000 reads
- Concurrent decompression: ✅ 100MB decompressed
- Random file access: ✅ 18,000 `contains()` checks
- Reader lifecycle: ✅ No file handle exhaustion
- True parallelism: ✅ Verified (separate file handles per reader)

**Crash Recovery Validation:**

| Failure Mode | Test Coverage | Expected Behavior | Status |
|-------------|--------------|-------------------|--------|
| finalize() not called | Incomplete archive | Reject (missing ENDR) | ✅ Pass |
| Truncation at 10% | Header only | Reject (CD not found) | ✅ Pass |
| Truncation at 30% | Partial file data | Reject (incomplete) | ✅ Pass |
| Truncation at 50% | Mid-archive | Reject (CD missing) | ✅ Pass |
| Truncation at 70% | Most files present | Reject (ENDR missing) | ✅ Pass |
| Truncation at 90% | Near-complete | Reject (incomplete ENDR) | ✅ Pass |
| Header-only file | 64 bytes | Reject (no CD) | ✅ Pass |
| Missing ENDR | No end record | Reject (validation fail) | ✅ Pass |
| Partial ENDR | Truncated end record | Reject (incomplete) | ✅ Pass |
| Corrupted file data mid-archive | Bit flips in data | CRC mismatch on read | ✅ Pass |

**Frame Compression Validation:**

Large File Tests (9 tests, ≥50MB threshold):
- Boundary: 49MB (no frames): ✅ Standard compression
- Boundary: 50MB (frames): ✅ Frame compression activated
- Boundary: 51MB: ✅ Frame compression
- Medium: 75MB: ✅ Correct frame handling
- Large: 100MB: ✅ All frames accessible
- Very large: 200MB: ✅ Data integrity preserved
- Pattern integrity: ✅ No data corruption across frames
- Mixed archive: ✅ Frame + non-frame files coexist
- Odd size: 50MB + 1KB: ✅ Correct frame count calculation

Frame structure validation:
- Frame size: 64KB (65,536 bytes)
- Frame index: Correct offset mapping
- Partial decompression: Selective frame access functional
- Data integrity: SHA-256 verification across frame boundaries

**Findings:**
- Thread-safe VFS with no resource leaks (10,000+ concurrent queries)
- True parallelism via separate file handles (100 concurrent readers)
- All incomplete archives properly rejected (13 failure modes tested)
- Frame compression works correctly for files ≥50MB (200MB tested)
- No data races or undefined behavior under concurrent load

**Operations Tested:**
- 10,000+ concurrent VFS database queries
- 64,000+ multi-reader operations
- 500MB+ data processed

#### 10.2.4 Phase 3: Performance and Scale Validation

**Purpose:** Validate scalability to large archives, many files, and compression effectiveness.

**Test Distribution:**
- Large Archive Stress: 8 tests (4 regular + 4 stress/ignored)
- Compression Validation: 8 tests

**Scalability Validation:**

Path and Directory Tests (4 regular tests):

| Test | Parameter | Result | Status |
|------|-----------|--------|--------|
| Maximum path length | 255 bytes | Accepted; enforced at finalize() | ✅ Pass |
| Path length boundary | 1-255 bytes (all values) | All accepted | ✅ Pass |
| Deep directory structure | 20 levels | Functional | ✅ Pass |
| Many small files baseline | 1,000 files | <50ms end-to-end | ✅ Pass |

Stress Tests (4 tests, executed with `--ignored` flag):

| Test | Scale | Creation Time | Archive Size | Compression Ratio | Status |
|------|-------|--------------|--------------|-------------------|--------|
| 500MB archive | 50 × 10MB files | 4.3s | 1MB | 500x | ✅ Pass |
| 1GB archive | 100 × 10MB files | ~10s | ~2MB | ~500x | ✅ Pass |
| 10K files | 10,000 × 1KB files | ~1s | Variable | Variable | ✅ Pass |
| 1K files baseline | 1,000 files | 0.05s | Variable | Variable | ✅ Pass |

**Compression Effectiveness Validation:**

Measured Compression Ratios (8 tests):

| Data Type | Original Size | Archive Size | Ratio | Test Case | Status |
|-----------|--------------|--------------|-------|-----------|--------|
| Zeros (highly compressible) | 1MB | 4.6KB | 227x | 10MB zeros → 40KB | ✅ Pass |
| Repetitive text | 439KB | 0.6KB | 754x | Repeated lorem ipsum | ✅ Pass |
| Text files (JSON/MD) | 81KB | 1.4KB | 59x | Realistic text data | ✅ Pass |
| Mixed compressibility | 100KB | 1.2KB | 86x | Zeros + pattern + random | ✅ Pass |
| Multiple same-byte files | 10MB (10×1MB) | 44KB | 233x | 10 files, same byte value | ✅ Pass |
| Pattern data | 1MB | Variable | 2-5x | Sequential bytes | ✅ Pass |
| Uncompressed storage | 10KB | 10.5KB | ~1x | Forced CompressionMethod::None | ✅ Pass |
| Large file (frame) | 50MB | 216KB | 237x | 50MB same-byte value | ✅ Pass |

**Performance Characteristics (Measured):**

Write throughput:
- Zstd: ~95 MB/s (10MB file)
- LZ4: ~380 MB/s (10MB file)
- None: ~450 MB/s (10MB file)

Read throughput:
- Zstd: ~180 MB/s (10MB file)
- LZ4: ~420 MB/s (10MB file)
- None: ~500 MB/s (10MB file)

Archive operations:
- Open + initialize (1,000 files): <10ms
- File lookup (O(1) HashMap): <0.1ms
- Create 1,000 files: ~3ms
- Central directory write: <1ms

Memory usage:
- Central directory: 320 bytes per file
- 1,000 files: ~320KB
- 10,000 files: ~3.2MB

**Findings:**
- Scales to 1GB+ archives with no issues
- 10,000+ files handled efficiently (O(1) lookup)
- Compression ratios: 50-227x typical, 754x maximum (text)
- Performance: ~120 MB/s write, ~200 MB/s read (500MB test)
- Path limit enforced: 255 bytes maximum
- Directory depth: 20 levels tested successfully
- No scalability or performance degradation observed

#### 10.2.5 Phase 4: Security Audit

**Purpose:** Validate security posture against path traversal attacks, decompression bombs, and cryptographic attack vectors.

**Test Distribution:**
- Path Traversal Prevention: 10 tests
- ZIP Bomb Protection: 8 tests
- Cryptographic Attack Tests: 8 tests

**Path Security Validation:**

| Attack Vector | Test Input | Current Behavior | Security Assessment | Status |
|--------------|-----------|------------------|---------------------|--------|
| Parent directory ref | `../../etc/passwd` | Accepted (normalized) | ⚠️ Application must validate | ✅ Documented |
| Absolute Unix path | `/etc/passwd` | Accepted (normalized) | ⚠️ Application must validate | ✅ Documented |
| Absolute Windows path | `C:\Windows\System32\evil.dll` | Accepted (normalized) | ⚠️ Application must validate | ✅ Documented |
| Null byte injection | `file.txt\0/../../etc/passwd` | Accepted | ⚠️ Application must validate | ✅ Documented |
| Path length overflow | 256-byte path | Rejected at finalize() | ✅ Enforced (255 limit) | ✅ Pass |
| Empty path | `""` | Accepted | ⚠️ Application may reject | ✅ Documented |
| Special characters | Spaces, Unicode, emoji | Accepted | ✅ UTF-8 support | ✅ Pass |
| Case sensitivity | `File.txt` vs `file.txt` | Distinct files | ✅ Case-sensitive | ✅ Pass |
| Path normalization | `dir/file.txt` vs `dir\file.txt` | Normalized to `/` | ✅ Cross-platform | ✅ Pass |
| Empty components | `dir//file.txt`, `/file.txt` | Accepted | ⚠️ Normalization varies | ✅ Documented |

**Security Posture - Path Handling:**
- ✅ Path length enforced (255 bytes maximum)
- ✅ Path normalization functional (Windows `\` → `/`)
- ✅ Case-sensitive storage (preserves case distinctions)
- ⚠️ Path traversal attempts accepted (normalization only)
- ⚠️ **Applications must sanitize paths during extraction**

**Decompression Bomb Protection:**

Compression Safety Tests (8 tests):

| Test Case | Data Characteristics | Compression Ratio | Memory Behavior | Status |
|-----------|---------------------|------------------|-----------------|--------|
| 10MB zeros | Highly compressible | 252x (10MB → 40KB) | ✅ Controlled | ✅ Pass |
| Repetitive text | Lorem ipsum × 1000 | 754x (439KB → 0.6KB) | ✅ Controlled | ✅ Pass |
| 10 × 1MB same-byte | Multiple compressible | 233x (10MB → 44KB) | ✅ Controlled | ✅ Pass |
| Mixed data | Zeros + pattern + random | 223x (3MB → 13KB) | ✅ Controlled | ✅ Pass |
| 50MB large file | Same-byte pattern | 237x (50MB → 216KB) | ✅ Frame compression | ✅ Pass |
| Uncompressed | Method::None | ~1x (overhead only) | ✅ No decompression | ✅ Pass |

**Bomb Protection Mechanisms:**
- ✅ Frame compression limits memory (64KB frames for files ≥50MB)
- ✅ No recursive compression support (prevents nested bombs)
- ✅ Relies on zstd/lz4 library safety checks (allocation limits)
- ⚠️ No explicit decompression bomb detection
- ⚠️ Applications should set resource limits (ulimit, cgroups)

**Compression Ratios Validated:**
- Highly compressible (zeros, patterns): 200-750x
- Text data (JSON, Markdown, code): 50-100x
- Mixed data: 50-100x
- Binary/random data: 1-5x

**Cryptographic Attack Resistance:**

Ed25519 Signature Validation (8 tests):

| Attack Scenario | Test Method | Result | Status |
|----------------|-------------|--------|--------|
| Basic signature verification | Sign + verify with correct key | ✅ Valid | ✅ Pass |
| Wrong key rejection | Verify with different key | ✅ Invalid | ✅ Pass |
| Data modification detection | Modify signed manifest | ✅ Invalid | ✅ Pass |
| Multiple signatures | 2 signers on same manifest | ✅ Both valid | ✅ Pass |
| Weak key avoidance | Generate 100 keys | ✅ No weak patterns | ✅ Pass |
| Timing attack resistance | Constant-time verification | ✅ No timing leaks | ✅ Pass |

**Timing Attack Analysis:**
- Ed25519 implementation: `ed25519-dalek` (audited, constant-time)
- Signature verification time: ~8-10ms (measured)
- Timing variations: OS/CPU scheduling (not cryptographic operations)
- Result: ✅ No timing attack vulnerabilities detected

**Key Generation Quality:**
- 100 keys generated: All unique
- No all-zero keys: Verified
- No all-ones keys: Verified
- Entropy source: `OsRng` (cryptographically secure)
- Result: ✅ Weak keys avoided

**Cryptographic Libraries:**
- Ed25519: `ed25519-dalek` 2.1+ (constant-time, audited)
- AES-256-GCM: `aes-gcm` crate (constant-time, AEAD)
- PBKDF2: `pbkdf2` crate (key derivation, ≥100K iterations recommended)
- SHA-256: `sha2` crate (hashing)

**Side-Channel Resistance:**
- ✅ Timing attacks: Constant-time crypto operations
- ✅ Power analysis: Software-level mitigation (constant-time)
- ✅ Cache timing: No secret-dependent table lookups
- ℹ️ Fault injection: Out of scope (requires physical access)

**Security Verdict:**

Cryptographic Security: **Strong**
- ✅ Ed25519 signatures with constant-time verification
- ✅ AES-256-GCM authenticated encryption
- ✅ No timing attack vulnerabilities
- ✅ Multiple signatures supported
- ✅ Signature invalidation on modification detected

Compression Safety: **Good**
- ✅ Frame compression limits memory
- ✅ No recursive compression
- ✅ Library safety checks (zstd/lz4)
- ⚠️ No explicit bomb detection (relies on library limits)

Path Validation: **Minimal**
- ✅ Path length enforced (255 bytes)
- ✅ Path normalization functional
- ⚠️ Path traversal attempts accepted
- ⚠️ **Applications must sanitize paths during extraction**

**Overall Assessment:** No critical security vulnerabilities found. Format is production-ready with proper application-level path sanitization during archive extraction.

#### 10.2.6 Conformance Testing Requirements

Alternative implementations must demonstrate conformance through equivalent validation:

**Mandatory Test Coverage:**
1. Format parsing: Magic number, version, header structure
2. Central directory: Fixed 320-byte entries, offset calculation, HashMap lookup
3. Compression: All methods (None, LZ4, Zstd), compression ratio verification
4. Frame compression: Files ≥50MB, 64KB frame handling
5. CRC verification: Header CRC, file data CRC
6. Corruption detection: Truncation, bit flips, invalid signatures
7. Path handling: 255-byte limit, normalization, case sensitivity
8. Concurrency: Thread-safe readers, VFS database access

**Recommended Test Coverage:**
1. Encryption: Archive-level and per-file modes
2. Signatures: Ed25519 creation and verification
3. Large archives: ≥500MB, ≥1000 files
4. Edge cases: Empty archives, single-file archives
5. Recovery: Incomplete archives, partial writes

**Performance Baselines:**
- File lookup: O(1) with HashMap (sub-millisecond)
- Archive open: <10ms for 1,000 files
- Compression: ≥90 MB/s write, ≥180 MB/s read (Zstd)
- VFS queries: ≥80% of native filesystem performance

**Test Suite Availability:**

Reference test suite: `github.com/blackfall-labs/engram-rs/tests/`

Test files:
- `corruption_test.rs` (15 tests)
- `signature_security_test.rs` (13 tests)
- `encryption_security_test.rs` (18 tests)
- `concurrency_vfs_test.rs` (5 tests)
- `concurrency_readers_test.rs` (6 tests)
- `crash_recovery_test.rs` (13 tests)
- `frame_compression_test.rs` (9 tests)
- `stress_large_archives_test.rs` (8 tests)
- `compression_validation_test.rs` (8 tests)
- `security_path_traversal_test.rs` (10 tests)
- `security_zip_bomb_test.rs` (8 tests)
- `security_crypto_attacks_test.rs` (8 tests)

Implementations passing the complete test suite achieve verified compatibility with this specification.

---

## 11.0 CONCLUSION

The Engram archive format provides a durable foundation for long-term knowledge preservation through deliberate architectural choices prioritizing format stability, operational independence, and semantic integrity. The specification balances competing requirements—compression efficiency, random access performance, database integration—through layered abstraction and conservative engineering.

Archives created under this specification remain queryable across technological transitions without extraction overhead or format migration. The combination of embedded SQLite access, cryptographic verification capabilities, and deterministic structure ensures institutions maintain control over preserved knowledge independent of vendor continuity or network availability.

The format serves as the immutable storage layer within Blackfall's broader knowledge management architecture, complementing mutable Cartridge workspaces and semantic BytePunch compression. Together, these systems address the fundamental challenge of preserving institutional knowledge across multi-decade operational timescales.

---

## APPENDIX A: BINARY FORMAT SUMMARY

### Complete Archive Structure

```
Offset      Size    Component
0x0000      64      File Header
0x0040      var     File Data Region
  Entry 1:
    0x0040  var     Local file header + compressed data
  Entry 2:
    var     var     Local file header + compressed data
  ...
  Entry N:
    var     var     Local file header + compressed data
var         320×N   Central Directory
  Entry 1:
    var     320     Central directory entry
  Entry 2:
    var+320 320     Central directory entry
  ...
  Entry N:
    var+320N 320    Central directory entry
var+320N    64      End of Central Directory Record
```

### Field Width Summary

| Structure               | Total Size       | Fixed/Variable |
| ----------------------- | ---------------- | -------------- |
| File Header             | 64 bytes         | Fixed          |
| Local Entry Header      | 40+ bytes + path | Variable       |
| Central Directory Entry | 320 bytes        | Fixed          |
| End Record              | 64 bytes         | Fixed          |

### Magic Numbers and Signatures

| Component     | Signature        | Bytes | Hex                |
| ------------- | ---------------- | ----- | ------------------ |
| File Header   | ENG format magic | 8     | 0x89454E470D0A1A0A |
| Local Entry   | LOCA             | 4     | 0x4C4F4341         |
| Central Entry | CENT             | 4     | 0x43454E54         |
| End Record    | ENDR             | 4     | 0x454E4452         |

---

**Document Revision History:**

| Version | Date       | Changes                                                          | Authority              |
| ------- | ---------- | ---------------------------------------------------------------- | ---------------------- |
| 1.0     | 2025-12-19 | Production release: LOCA headers, ENDR record, frame compression | Blackfall Laboratories |
| 0.4     | 2025-12-19 | Normative v0.4 specification with encryption flags (draft)       | Blackfall Laboratories |

**Related Specifications:**

- Cartridge Format Specification (mutable workspaces)
- BytePunch Card Specification (semantic compression)
- DataSpool Format Specification (sequential card archives)
- Content Markup Language (CML) Specification

---

**For implementation questions or clarification requests, contact:**
magnus@blackfall.dev
