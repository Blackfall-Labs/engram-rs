# Engram Design Rationale: Why Every Decision Matters

_A comprehensive guide to understanding the reasoning, safety mechanisms, and trade-offs in the Engram archive format_

---

## Document Purpose

This document explains the **WHY** behind every design decision in the Engram format. It connects the initial theory (engram_format_theory.md), the implemented architecture (engram_architecture_clarification.md), the formal specification (SPECIFICATION.md), and the actual Rust implementation.

**Audience:** Future maintainers, implementers in other languages, and anyone asking "why did we make it this way?"

---

## Table of Contents

1. [Core Philosophy](#1-core-philosophy)
2. [Safety Mechanisms](#2-safety-mechanisms)
3. [Format Structure Decisions](#3-format-structure-decisions)
4. [Performance Optimizations](#4-performance-optimizations)
5. [Future-Proofing Strategies](#5-future-proofing-strategies)
6. [Implementation Patterns](#6-implementation-patterns)
7. [Trade-Offs and Alternatives](#7-trade-offs-and-alternatives)

---

## 1. Core Philosophy

### 1.1 Immutability as a Feature

**Decision:** Engrams are write-once, read-forever archives. No in-place modification.

**Why:**
- **Perfect optimization:** Knowing the exact content at compile time enables custom compression dictionaries with zero overhead
- **Guaranteed integrity:** No modification = checksums never go stale
- **Atomic updates:** New versions are entirely new files, eliminating corruption from partial updates
- **Archival trust:** Immutable artifacts can be mirrored, torrented, and verified indefinitely

**Trade-off:** Cannot do incremental updates. Solution: Delta distribution between versions.

**Alternative considered:** Append-only archives (like Git packfiles). Rejected because:
- Index complexity increases over time
- No guarantee of deduplication
- Harder to verify integrity across multiple appends

**Real-world validation:** This mirrors how successful archival systems work (Git objects, blockchain, WORM drives).

---

### 1.2 Separation of Distribution and Runtime

**Decision:** Compress for bandwidth, decompress for speed. Never read compressed data repeatedly.

**Why:**
- **Download:** 500MB compressed → users save bandwidth
- **Runtime:** 2GB decompressed in cache → instant access (<1ms)
- **Best of both worlds:** Storage-efficient distribution + filesystem-speed access

**How it works:**
1. Mount archive (one-time decompression, 2-5 seconds)
2. Cache decompressed content (memory or disk)
3. All reads hit cache (no repeated decompression)

**Alternative considered:** Streaming decompression on every read (like reading from ZIP).
**Rejected because:**
- 5-15ms per file access (unacceptable for interactive use)
- Repeated CPU overhead (decompress same file 100x)
- Battery drain on mobile devices

**Key insight:** For frequently-accessed static data, decompression cost should be paid once, not on every access.

---

### 1.3 Content vs Presentation Separation

**Decision:** Store pure semantic content (CML), let rendering engines handle presentation.

**Why:**
- **100x smaller files:** 50KB CML vs 5MB PDF with embedded fonts/layout
- **Accessibility:** Semantic structure enables screen readers, reflow, zoom
- **User control:** Dark mode, font sizes, themes without touching content
- **Future-proof:** Plain text + semantic markup survives platform death

**Alternative considered:** Self-contained PDFs with everything embedded.
**Rejected because:**
- Bloated (fonts + layout = 90% of file size)
- Fixed presentation (can't adapt to device or preference)
- Inaccessible (fixed layout, poor screen reader support)

**Example:** US Constitution as CML = 50KB. As PDF = 5MB. Same content, 100x difference.

---

## 2. Safety Mechanisms

### 2.1 The 255/256 Path Length Choice

**Implementation:**
```rust
pub const MAX_PATH_LENGTH: usize = 255;  // Maximum usable
let mut path_buf = [0u8; 256];           // Buffer size
```

**Why 255, not 256?**

This follows the **C string convention** for interoperability and safety:

1. **C Compatibility:** 256-byte buffer with guaranteed null terminator at position 255
2. **String Safety:** Even at maximum path length (255 bytes), the 256th byte is always `\0`
3. **Fixed-Size Entries:** Central Directory entries are exactly 320 bytes, enabling O(1) array indexing
4. **No Off-by-One Errors:** Path length field (u16) stores 0-255, buffer is 256, always room for null

**How it protects:**
- C implementations can treat paths as null-terminated strings safely
- Buffer overruns are impossible (length <= 255, buffer = 256)
- Rust implementations get free bounds checking
- Cross-platform compatibility (Unix PATH_MAX patterns)

**Alternative considered:** Variable-length paths with length prefix.
**Rejected because:**
- Variable-length entries break O(1) indexing in Central Directory
- Would require pointer indirection (path pool at end)
- Complicates memory mapping and binary search
- 255 bytes handles 99.9% of real-world paths

---

### 2.2 Three-Layer Checksum Strategy

**Implementation:**
1. **Header CRC32** (fast fail at open)
2. **Per-file CRC32** (verify decompressed content)
3. **Archive CRC32** (optional, full-file integrity)

**Why three layers?**

**Layer 1: Header CRC32**
- **Purpose:** Detect corruption immediately on open
- **Scope:** Bytes 8-63 of header (version, offsets, counts)
- **Fail mode:** Reject entire archive, don't waste time scanning
- **Protects against:** Truncated downloads, corrupt filesystems, bitflips in metadata

**Layer 2: Per-file CRC32**
- **Purpose:** Verify each file's content after decompression
- **Scope:** Uncompressed data
- **Fail mode:** Reject single file, continue reading others
- **Protects against:** Compression bugs, partial writes, targeted corruption

**Layer 3: Archive CRC32 (optional)**
- **Purpose:** Whole-archive verification for distribution
- **Scope:** Entire .eng file
- **Fail mode:** Warn but allow reading (content CRCs still checked)
- **Protects against:** Download errors, mirror corruption, tampering detection

**Why CRC32 instead of SHA-256?**
- **Speed:** CRC32 is 10-20x faster (critical for per-file checks)
- **Good enough:** CRC32 detects 99.9999% of accidental corruption
- **Not for security:** Engrams are content-addressed, not security-signed (future v1.0)

**Alternative considered:** Single SHA-256 for entire archive.
**Rejected because:**
- Doesn't catch individual file corruption
- Requires reading entire archive to verify anything
- Overkill for accidental corruption detection (which is the primary threat)

---

### 2.3 Path Validation and Traversal Prevention

**Threats:**
```
../../etc/passwd          (Unix traversal)
C:\Windows\System32\...   (Absolute Windows path)
\\server\share\...        (UNC path)
./symlink/...             (Symlink escape)
```

**Protection layers:**

**1. Path normalization:**
```rust
fn validate_path(path: &str) -> Result<()> {
    // No absolute paths
    if path.starts_with('/') || path.contains(":\\") {
        return Err(PathError::Absolute);
    }

    // No traversal
    for component in path.split('/') {
        if component == ".." {
            return Err(PathError::Traversal);
        }
    }

    // No null bytes (security)
    if path.contains('\0') {
        return Err(PathError::NullByte);
    }

    Ok(())
}
```

**2. UTF-8 validation:**
- Paths must be valid UTF-8 (Rust guarantees this via `String`)
- Prevents encoding attacks (UTF-7, overlong UTF-8)

**3. Length enforcement:**
- Maximum 255 bytes prevents buffer overflows in C implementations
- Empty paths rejected

**Why this matters:**
- **Archive extraction:** Prevents writing to arbitrary system locations
- **VFS mounting:** Prevents escape from virtual filesystem
- **Cross-platform:** Works on Windows, Unix, and future systems

---

### 2.4 Magic Number Corruption Detection

**Implementation:**
```rust
pub const MAGIC_NUMBER: [u8; 8] = [0x89, b'E', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
```

**Why this specific sequence?**

Follows PNG's proven pattern for detecting corruption:

**Byte 0: `0x89` (non-ASCII)**
- **Detects:** Text-mode vs binary-mode transfer errors
- **How:** Non-ASCII byte prevents file being misidentified as text
- **Example:** FTP ASCII mode would corrupt this byte

**Bytes 1-3: `ENG` (human-readable)**
- **Detects:** Manual identification ("file" command on Unix)
- **How:** Hex editors show human-readable format ID
- **Example:** Users can identify format without special tools

**Byte 4: `0x0D` (CR - Carriage Return)**
- **Detects:** Line-ending translation on Windows
- **How:** Text-mode tools might convert CR → CRLF or strip CR
- **Example:** Copying through Windows text editors

**Byte 5: `0x0A` (LF - Line Feed)**
- **Detects:** Unix line-ending handling
- **How:** Unix text tools might strip or modify LF
- **Example:** Sed/awk processing would corrupt this

**Byte 6: `0x1A` (Ctrl-Z - DOS EOF)**
- **Detects:** DOS-era tools that treat Ctrl-Z as end-of-file
- **How:** Ancient DOS tools stop reading here
- **Example:** TYPE command in DOS

**Byte 7: `0x0A` (LF - final validation)**
- **Detects:** Confirms previous bytes weren't accidental match
- **How:** Unlikely random sequence
- **Example:** Double-check for corruption

**Why PNG's pattern works:**
- Tested on billions of files across 30 years
- Catches corruption from text-mode transfers, DOS tools, encoding issues
- Simple to implement (8-byte comparison)
- Human-readable component aids debugging

**Alternative considered:** Simple 4-byte magic like "ENG\0".
**Rejected because:** Doesn't detect text-mode corruption, less robust.

---

### 2.5 Fixed-Size Entries for Memory Safety

**Decision:** Central Directory entries are exactly 320 bytes, always.

**Why fixed-size?**

**Memory safety:**
```rust
struct CentralDirectory {
    entries: Vec<[u8; 320]>,  // Guaranteed size
}

// Safe indexing
let entry_data = &cd_data[entry_index * 320..(entry_index + 1) * 320];
```

**Benefits:**
1. **Bounds checking:** Index calculation can't overflow
2. **Memory mapping:** Can mmap() directory and trust layout
3. **Binary search:** Can bisect directory without parsing
4. **O(1) access:** Array indexing by file number

**Protection against:**
- Buffer overflows (size known at compile time)
- Integer overflow (320 * u32::MAX fits in u64)
- Malformed entries (parser rejects if signature wrong)

**Alternative considered:** Variable-length entries with length prefix.
**Rejected because:**
- Requires scanning to find entry N (O(N) instead of O(1))
- Harder to memory map safely
- More complex bounds checking

---

## 3. Format Structure Decisions

### 3.1 TOC-at-End (ZIP-style) vs TOC-at-Beginning (ASAR-style)

**Decision:** Place Central Directory at end of file (like ZIP), not beginning (like ASAR).

**Why?**

**Streaming creation:**
```rust
// TOC-at-end enables this:
let mut writer = EngramWriter::create("archive.eng")?;

// Add files as you discover them (no need to know full list upfront)
for entry in walkdir::WalkDir::new(source_dir) {
    writer.add_file(entry.path())?;  // Write immediately
}

// Build and write TOC at the end
writer.finalize()?;  // Now we know all entries
```

**With TOC-at-beginning (ASAR), you'd need:**
```rust
// ASAR-style requires:
let files = collect_all_files(source_dir);  // Must scan everything first
let toc = build_toc(&files);                // Build TOC
write_toc(toc);                             // Reserve space (or estimate)
write_files(files);                         // Write data

// Problem: What if TOC size estimate is wrong?
// Must leave padding (wasted space) or rewrite entire file
```

**Real-world benefits:**
- **Git integration:** Can stream repository as you traverse
- **Build pipelines:** Add files as build system produces them
- **Updates:** Append new files, write new TOC (old TOC becomes invalid but data preserved)

**Reading is equally efficient:**
1. Seek to last 64 bytes (End-of-Central Directory)
2. Read Central Directory offset
3. Load entire Central Directory (1-5MB, one read)
4. Build hash index in memory
5. O(1) random access thereafter

**Alternative (ASAR-style TOC-at-beginning):**
- **Advantage:** Metadata available immediately (don't seek to end)
- **Disadvantage:** Can't stream creation, harder to update
- **Verdict:** Not worth the trade-off for write-once archives

---

### 3.2 Dual Versioning: Format vs Content

**Implementation:**
```rust
// Format version (binary structure)
pub const FORMAT_VERSION_MAJOR: u16 = 1;  // Breaking changes
pub const FORMAT_VERSION_MINOR: u16 = 0;  // Backward-compatible additions

// Content version (application-defined)
header.content_version: u32  // User data, semantic versioning
```

**Why two version numbers?**

**Format version** controls binary structure:
- **Major:** Breaking changes (e.g., change entry size from 320 to 400 bytes)
- **Minor:** Backward-compatible (e.g., use reserved bytes for new field)

**Example:**
- v1.0 reader can read v1.2 archives (ignores new fields)
- v1.0 reader rejects v2.0 archives (structure changed)

**Content version** tracks application data:
- **Use case:** Database schema version, CML profile version, API compatibility
- **Independent:** Format can be v1.0 while content is v7

**Why separate?**

**Scenario 1:** Format stable, content evolves
```
Archive A: Format v1.0, Content v5  (July 2025)
Archive B: Format v1.0, Content v12 (March 2026)
```
Same format reader works for both, application handles content differences.

**Scenario 2:** Format evolves, content stable
```
Archive C: Format v1.0, Content v5  (uses 320-byte entries)
Archive D: Format v2.0, Content v5  (uses 400-byte entries, new compression)
```
Applications check format version first, content version second.

**Implementation pattern:**
```rust
fn open_archive(path: &Path) -> Result<Archive> {
    let header = read_header(path)?;

    // Check format compatibility
    if header.version_major > CURRENT_MAJOR {
        return Err("Archive format too new, update reader");
    }

    // Warn but proceed on newer minor
    if header.version_minor > CURRENT_MINOR {
        warn!("Archive has newer features, may ignore some metadata");
    }

    // Application checks content version separately
    let manifest = read_manifest(&archive)?;
    if manifest.content_version > APP_CONTENT_VERSION {
        warn!("Content schema too new, may not parse correctly");
    }

    Ok(archive)
}
```

---

### 3.3 Reserved Bytes: Future-Proofing Strategy

**Implementation:**
```
File Header:    24 bytes reserved (bytes 40-63)
CD Entry:       20 bytes reserved (bytes 300-319)
End Record:     32 bytes reserved (bytes 32-63)
```

**Why reserve so much space?**

**Enables forward-compatible additions:**

**Example: Adding encryption in v1.1**
```
Current v1.0 header (40-63 = zeros):
[00 00 00 00 00 00 00 00 ...]

Future v1.1 header (uses first 8 reserved bytes):
[01 XX XX XX XX XX XX XX 00 00 00 ...]
 ^^ Flags byte (bit 0 = encrypted)
    ^^^^^^^^^^^^^^^^ Encryption metadata
                     ^^^^^^^ Still reserved

Rules:
1. v1.0 readers ignore reserved bytes (all zeros)
2. v1.1 readers check flags byte
3. If encryption flag set and reader doesn't support it → reject
4. If flags = 0 → proceed as v1.0
```

**Protected evolution:**
- New features use reserved space
- Old readers ignore unknown fields (if flags = 0)
- Required features cause graceful rejection
- Optional features degrade gracefully

**Alternative considered:** No reserved space, pack fields tightly.
**Rejected because:**
- Adding features requires format version bump
- Can't maintain backward compatibility
- Wastes more space long-term (whole new format per feature)

**Memory cost:** ~30 bytes per file * 10,000 files = 300KB. Negligible.

---

### 3.4 Signatures for Quick Validation

**Implementation:**
```rust
// Each section has a 4-byte signature
LOCA (0x4C4F4341)  // Local File Entry
CENT (0x43454E54)  // Central Directory Entry
ENDR (0x454E4452)  // End Record
```

**Why?**

**Fast corruption detection:**
```rust
fn parse_cd_entry(data: &[u8]) -> Result<Entry> {
    let sig = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    if sig != 0x43454E54 {
        return Err("Corrupted directory entry");
    }
    // Continue parsing...
}
```

**Protects against:**
- **Truncation:** If file ends mid-directory, signature check fails immediately
- **Offset errors:** If index calculation wrong, signature mismatch catches it
- **Random corruption:** 1 in 4 billion chance random data matches signature

**Scanning benefit:**
```rust
// Find End Record by scanning backward
for offset in (0..file_size).rev().step_by(64) {
    let sig = read_u32_at(offset)?;
    if sig == 0x454E4452 {  // Found "ENDR"
        return Ok(offset);
    }
}
```

**Alternative considered:** No signatures, trust offsets.
**Rejected because:**
- Offset errors cause silent corruption (wrong data returned)
- Harder to debug (no obvious detection point)
- Can't scan for structures (must trust index)

---

## 4. Performance Optimizations

### 4.1 Per-File Compression vs Whole-Archive Compression

**Decision:** Compress each file independently, not the entire archive.

**Why?**

**Random access requirement:**
```
Whole-archive compression:
[Compressed: File1 + File2 + File3 + ... + File10000]
To read File5000 → decompress everything before it (unacceptable)

Per-file compression:
[File1 compressed] [File2 compressed] ... [File10000 compressed]
To read File5000 → decompress only File5000 (< 5ms)
```

**Performance comparison:**
| Operation | Whole-Archive | Per-File |
|-----------|---------------|----------|
| Read single file | Decompress entire archive (seconds) | Decompress one file (< 5ms) |
| Memory usage | Archive size × 2 (for decompression) | File size × 2 |
| Random access | Sequential only | True random access |

**Compression ratio trade-off:**
- Whole-archive: 75% reduction (best ratio)
- Per-file: 65-70% reduction (still excellent)
- **Verdict:** 5-10% worse compression for 1000x better access speed

**When whole-archive wins:**
- Sequential access only (e.g., backup archives)
- Extremely size-constrained (embedded systems)
- **Not our use case:** We need random access

---

### 4.2 Compression Algorithm Selection

**Implementation:**
```rust
pub enum CompressionMethod {
    None = 0,
    Lz4 = 1,
    Zstd = 2,
    Deflate = 3,
}
```

**Why offer multiple options?**

**Use case matching:**

**LZ4 (Method 1):**
- **Speed:** 2-3 GB/s decompression
- **Ratio:** ~50% reduction
- **Use for:** Frequently accessed files, speed-critical assets
- **Example:** UI resources, hot-path code

**Zstandard (Method 2):**
- **Speed:** 500 MB/s decompression
- **Ratio:** 65-75% reduction
- **Use for:** Balanced choice, most files
- **Example:** Databases, JSON, text documents

**Deflate (Method 3):**
- **Speed:** 200 MB/s decompression
- **Ratio:** 60% reduction
- **Use for:** Legacy compatibility, ZIP interop
- **Example:** When tools expect deflate

**None (Method 0):**
- **Use for:** Already-compressed (JPEG, PNG, MP4, ZIP)
- **Example:** Storing images

**Per-file strategy:**
```rust
fn select_compression(file: &File) -> CompressionMethod {
    match file.extension() {
        "jpg" | "png" | "mp4" | "zip" => CompressionMethod::None,
        "db" | "sqlite" => CompressionMethod::Zstd,  // Great DB compression
        "json" | "txt" | "xml" => CompressionMethod::Zstd,  // Text compresses well
        _ if file.size() < 4096 => CompressionMethod::None,  // Too small
        _ => CompressionMethod::Lz4,  // Safe default, always fast
    }
}
```

**Why not just use one?**
- Different files have different access patterns
- User can optimize per-archive (size vs speed)
- Future compression methods can be added (Method 4, 5, etc.)

---

### 4.3 O(1) Indexing via Fixed-Size Entries

**Implementation:**
```rust
// Central Directory with fixed 320-byte entries
const CD_ENTRY_SIZE: usize = 320;

fn find_entry(cd_data: &[u8], entry_index: u32) -> &[u8] {
    let offset = (entry_index as usize) * CD_ENTRY_SIZE;
    &cd_data[offset..offset + CD_ENTRY_SIZE]
}

// No parsing needed, just math
```

**Why?**

**Performance benefits:**

**Access by index:** O(1)
```rust
// Get 5000th file instantly
let entry = find_entry(cd_data, 5000);  // Pure math, no parsing
```

**Binary search:** O(log N)
```rust
// Sorted by path, can binary search
fn find_path(cd: &CentralDirectory, path: &str) -> Option<Entry> {
    cd.entries.binary_search_by_key(&path, |e| &e.path)
}
```

**Memory mapping:**
```rust
// Can mmap() directory and trust layout
let mapping = unsafe {
    memmap2::MmapOptions::new()
        .offset(cd_offset)
        .len(cd_size)
        .map(&file)?
};

// Access entries without reading into memory
let entry_bytes = &mapping[entry_index * 320..(entry_index + 1) * 320];
```

**Alternative (variable-length entries):**
```rust
// Would require:
struct VariableEntry {
    header: EntryHeader,
    path: String,  // Variable length
}

// Must scan to find entry N
for i in 0..N {
    let len = read_u16(&data[offset])?;
    offset += len + HEADER_SIZE;  // Must read each entry
}
// O(N) instead of O(1)
```

**Memory cost:**
- Worst case: 255-byte path in 256-byte buffer = 1 byte wasted
- Average case: 50-byte path = 206 bytes wasted per entry
- 10,000 files = 2MB wasted
- **Verdict:** 2MB for O(1) access is worth it

---

### 4.4 Mount-Once Architecture

**Implementation:**
```rust
struct EngramMount {
    archive: EngramArchive,
    decompressed_cache: HashMap<String, Vec<u8>>,
    sqlite_vfs: SqliteVFS,
}

impl EngramMount {
    fn mount(path: &Path) -> Result<Self> {
        let archive = EngramArchive::open(path)?;

        // Decompress ALL files ONCE
        let mut cache = HashMap::new();
        for entry in archive.entries() {
            let data = archive.decompress_file(&entry.path)?;
            cache.insert(entry.path.clone(), data);
        }

        // Register SQLite VFS
        let vfs = SqliteVFS::new(archive.clone());
        register_vfs("engram", vfs)?;

        Ok(EngramMount { archive, cache, vfs })
    }

    fn read_file(&self, path: &str) -> Result<&[u8]> {
        self.cache.get(path)  // <1ms, memory lookup
            .ok_or(Error::FileNotFound)
    }
}
```

**Why decompress everything on mount?**

**Performance math:**

**Scenario:** 10,000 files, average 100KB compressed, 200KB decompressed

**Option A: Decompress on every read**
```
First access:  5ms (decompress)
Second access: 5ms (decompress again)
Third access:  5ms (decompress again)
100 accesses:  500ms total (BAD)
```

**Option B: Decompress once on mount**
```
Mount:          10 seconds (decompress all 10,000 files)
First access:   <1ms (memory lookup)
Second access:  <1ms (memory lookup)
100 accesses:   100ms total (GOOD)
```

**Break-even point:** After ~10 accesses per file, mount-once wins.

**Real-world access patterns:**
- Web servers: Serve same files repeatedly (1000+ accesses)
- Desktop apps: User returns to same documents (100+ accesses)
- Build tools: Iterate through files multiple times (10+ accesses)

**Memory cost:** 10,000 files × 200KB = 2GB RAM.
- Desktop: Acceptable (most systems have 16GB+)
- Server: Acceptable (dedicated memory)
- Mobile: Use disk cache instead (slower but persistent)

**Alternative:** LRU cache (decompress on demand, keep hot files).
**Rejected as default because:**
- Adds complexity (cache eviction, miss handling)
- Unpredictable performance (cache misses cause stalls)
- Mount-once is simpler and faster for typical use

---

## 5. Future-Proofing Strategies

### 5.1 Extension Without Breaking

**Strategy:** Use reserved bytes + feature flags for additions.

**Pattern:**
```rust
// v1.0 format
struct Header {
    // ... existing fields ...
    reserved: [u8; 24],  // All zeros
}

// v1.1 format (backward-compatible)
struct Header {
    // ... existing fields ...
    feature_flags: u32,  // Uses first 4 reserved bytes
    encryption_id: u32,  // Uses next 4 reserved bytes
    reserved: [u8; 16],  // Remaining reserved
}

// Reader logic
fn open_header(data: &[u8]) -> Result<Header> {
    let header = parse_header_v1_0(data);

    if header.version_minor >= 1 {
        // v1.1+ features
        let flags = read_u32(&data[40..44]);
        if flags & ENCRYPTION_FLAG != 0 {
            if !supports_encryption() {
                return Err("Archive requires encryption support");
            }
        }
    }

    Ok(header)
}
```

**Benefits:**
- Old readers work on new files (if flags = 0)
- New readers work on old files (flags defaults to 0)
- Graceful degradation (warn if optional feature unsupported)
- Hard rejection (error if required feature unsupported)

---

### 5.2 Planned Evolution Path

**Documented roadmap:**

**v0.1 (Current):**
- Basic format
- LZ4/Zstd/Deflate compression
- CRC32 validation
- Fixed 320-byte entries

**v0.2 (Planned):**
- **Feature flags bitmap** (uses 4 reserved bytes)
- **Extended attributes** (uses 8 reserved bytes)
- **Long path pool** (for paths > 255 bytes, optional)
- **Maintained backward compatibility:** v0.1 readers can read v0.2 files (if feature flags = 0)

**v1.0 (Goal):**
- **Encryption** (AES-256-GCM, uses feature flags)
- **Signing** (Ed25519 signatures, uses feature flags)
- **Compression level metadata** (for tuning)
- **Maintained backward compatibility:** Readers reject encrypted archives they can't decrypt

**Key principle:** Every version can read older versions losslessly.

---

### 5.3 Compression Method Extensibility

**Current:**
```rust
pub enum CompressionMethod {
    None = 0,
    Lz4 = 1,
    Zstd = 2,
    Deflate = 3,
}
```

**Future:**
```rust
pub enum CompressionMethod {
    // v0.1
    None = 0,
    Lz4 = 1,
    Zstd = 2,
    Deflate = 3,

    // v0.2 (future)
    Brotli = 4,
    Lzma = 5,
    Custom = 255,  // User-defined
}
```

**Handling unknown methods:**
```rust
fn decompress(data: &[u8], method: u8) -> Result<Vec<u8>> {
    match CompressionMethod::from_u8(method) {
        Ok(method) => {
            match method {
                CompressionMethod::None => Ok(data.to_vec()),
                CompressionMethod::Lz4 => lz4::decompress(data),
                CompressionMethod::Zstd => zstd::decompress(data),
                CompressionMethod::Deflate => deflate::decompress(data),
            }
        }
        Err(UnknownMethod(m)) => {
            Err(format!("Compression method {} not supported, update reader", m))
        }
    }
}
```

**Why this works:**
- New methods get new IDs
- Old readers reject archives with unknown methods
- User gets clear error message
- Can implement forward-compatibility (warn but skip unsupported files)

---

## 6. Implementation Patterns

### 6.1 Defense in Depth

**Principle:** Multiple independent checks catch errors at different layers.

**Example: Reading a file**
```rust
pub fn read_file(&self, path: &str) -> Result<Vec<u8>> {
    // Layer 1: Path validation
    validate_path(path)?;  // Check for traversal, absolute paths

    // Layer 2: Index lookup
    let entry = self.index.get(path)
        .ok_or(Error::FileNotFound)?;

    // Layer 3: Offset validation
    if entry.offset + entry.compressed_size > self.file_size {
        return Err(Error::CorruptedIndex);
    }

    // Layer 4: Signature check
    let sig = self.read_signature_at(entry.offset)?;
    if sig != LOCAL_SIGNATURE {
        return Err(Error::CorruptedEntry);
    }

    // Layer 5: Read compressed data
    let compressed = self.read_at(entry.offset, entry.compressed_size)?;

    // Layer 6: Decompress
    let decompressed = self.decompress(compressed, entry.method)?;

    // Layer 7: CRC verification
    let computed_crc = crc32(&decompressed);
    if computed_crc != entry.crc32 {
        return Err(Error::CorruptedData {
            expected: entry.crc32,
            actual: computed_crc,
        });
    }

    Ok(decompressed)
}
```

**Each layer catches different failure modes:**
1. Malicious paths
2. Missing files
3. Index corruption
4. File corruption
5. I/O errors
6. Decompression errors
7. Data corruption

**Why so many checks?**
- No single point of failure
- Clear error messages (know exactly what failed)
- Debugging aid (logs show which check failed)

---

### 6.2 Fail Fast vs Fail Gracefully

**Fail fast (reject entire archive):**
```rust
// Header corrupted → reject immediately
fn open_archive(path: &Path) -> Result<Archive> {
    let header = read_header(path)?;

    if !header.validate_crc() {
        return Err("Archive header corrupted, cannot continue");
    }

    // Don't waste time scanning a corrupted archive
}
```

**Fail gracefully (skip corrupted files):**
```rust
// Individual file corrupted → skip it, continue
fn load_all_files(&self) -> Vec<(String, Result<Vec<u8>>)> {
    self.entries
        .iter()
        .map(|entry| {
            let result = self.read_file(&entry.path);
            (entry.path.clone(), result)
        })
        .collect()
}
```

**Decision tree:**
- **Structural corruption** (header, index) → Fail fast
- **Data corruption** (single file) → Fail gracefully
- **Unknown version** → Fail fast (might misinterpret)
- **Unknown compression** → Fail gracefully (skip file)

---

### 6.3 Type Safety in Rust

**Why Rust for implementation?**

**Memory safety:**
```rust
// Impossible buffer overflow
let path_buf = [0u8; 256];  // Fixed size
let path_len = read_u16()?;

if path_len > 255 {
    return Err("Invalid path length");  // Caught by Rust's bounds checking
}

let path = String::from_utf8(path_buf[..path_len as usize].to_vec())?;
//                                    ^^^^^^^^^^^^^^ Rust checks bounds
```

**Type-driven correctness:**
```rust
// Can't accidentally pass wrong offset
fn read_at(&self, offset: u64, size: u64) -> Result<Vec<u8>> {
    // Type system enforces u64 (can't pass negative offset)
    // Can't overflow (u64::MAX is larger than any file)
}
```

**Enum exhaustiveness:**
```rust
match compression_method {
    CompressionMethod::None => { /* ... */ }
    CompressionMethod::Lz4 => { /* ... */ }
    CompressionMethod::Zstd => { /* ... */ }
    CompressionMethod::Deflate => { /* ... */ }
    // Compiler error if we forget a case
}
```

**Why this matters for safety:**
- Entire class of bugs impossible (buffer overflows, use-after-free)
- Compiler enforces invariants (can't create invalid state)
- Refactoring is safe (type errors caught at compile time)

---

## 7. Trade-Offs and Alternatives

### 7.1 Fixed vs Variable Entry Size

**Chosen:** Fixed 320-byte entries

**Trade-off analysis:**

| Aspect | Fixed | Variable |
|--------|-------|----------|
| Access speed | O(1) | O(N) |
| Memory usage | Higher (padding) | Lower (exact fit) |
| Implementation complexity | Lower | Higher |
| Binary search | Possible | Difficult |
| Memory mapping | Safe | Dangerous |

**Memory cost calculation:**
```
Average path: 50 bytes
Buffer size: 256 bytes
Waste: 206 bytes per entry

10,000 entries = 2MB wasted
100,000 entries = 20MB wasted
```

**Verdict:** 20MB is acceptable for 100,000-file archives. O(1) access is worth it.

**When variable-length wins:**
- Millions of files (memory constrained)
- Sequential-only access (no random access needed)
- Tiny embedded systems (every byte matters)

**Not our use case.**

---

### 7.2 Compression Level Configurability

**Current:** Hardcoded Zstd level 6 (balanced).

**Alternative:** Store compression level in entry metadata.

**Why not implemented yet?**

**Pros:**
- Users can optimize per-file (level 1 for speed, level 9 for size)
- Archives self-document compression settings
- Reproducible builds (know exact settings used)

**Cons:**
- Needs reserved byte (planned for v0.2)
- Decompression doesn't need level (only compression does)
- Adds complexity for marginal benefit

**Planned for v0.2:** Use 1 reserved byte for compression level.

---

### 7.3 Encryption: Why Not Yet?

**Planned for v1.0, not v0.1.**

**Why wait?**

**Complexity explosion:**
```
Without encryption:
- Read file
- Decompress
- Return

With encryption:
- Read file
- Decrypt (which algorithm? key from where?)
- Decompress (before or after decrypt?)
- Return

Plus:
- Key management (where stored? how derived?)
- Metadata (which files encrypted? same key?)
- Format version (encrypted archives can't be read by old readers)
- Performance (decrypt + decompress = 2x slower)
```

**Better approach:**
1. **v0.1:** Get core format right, widely used
2. **v0.2:** Add feature flags, extended attributes
3. **v1.0:** Add encryption using feature flags

**Interim solution:** Encrypt entire .eng file at filesystem level (dm-crypt, VeraCrypt).

---

### 7.4 Delta Updates Implementation

**Current:** New versions are entirely new archives.

**Planned:** Delta distribution between versions.

**Why not yet?**

**Requires:**
1. **Stable file IDs** (can't use paths, they might move)
2. **Content addressing** (identify unchanged files)
3. **Delta format** (which files added/removed/changed)
4. **Patch application** (safely apply delta without corruption)

**Binary diff approach:**
```
v1.eng (500MB)
v2.eng (550MB, +50MB new content)

Delta format:
- Remove: [list of file IDs]
- Add: [new file data]
- Modify: [binary diffs for changed files]

Delta size: ~55MB (instead of downloading 550MB)
```

**Planned for v0.2:**
- Add file ID field (uses reserved bytes)
- Add content hash (SHA-256 per file)
- Define delta format specification
- Implement delta generator and applier

**Current workaround:** Distribute via Git (Git's packfile format handles deltas).

---

## 8. Summary: Design Coherence

Every decision supports the core philosophy:

**Preservation → Immutability, checksums, versioning**
**Direct Access → Fixed-size entries, per-file compression, mount-once**
**Platform Independence → UTF-8 paths, standard compression, clear errors**
**Future Resilience → Reserved bytes, feature flags, semantic versioning**
**Human Legibility → Path strings, signatures, clear documentation**

The format is **simple enough to implement in a weekend** but **robust enough to last decades**.

---

## Appendix A: Common Questions

**Q: Why not use ZIP?**
A: ZIP doesn't support direct SQLite queries, has variable-length central directory (slower), and lacks our checksum layers.

**Q: Why not use SQLite directly?**
A: SQLite is a database, not an archive format. Can't store arbitrary binary files efficiently, no compression per-file, no manifest.

**Q: Why not use Git packfiles?**
A: Packfiles use delta compression (incompatible with random access), complex format, no SQLite integration.

**Q: Why 320 bytes for entries? Why not 256 or 512?**
A: 256 bytes for path + 64 bytes for metadata = 320. Next power of 2 (512) wastes 192 bytes per entry.

**Q: Can I modify an Engram?**
A: No, they're immutable. Create a new version instead. This enables perfect optimization and guaranteed integrity.

**Q: What if I need encryption?**
A: Wait for v1.0, or encrypt the entire .eng file at the filesystem level.

**Q: How do I verify an archive?**
A: Check header CRC32, then verify each file's CRC32. Optional: compute full-archive checksum.

---

## Appendix B: Implementation Checklist

**Minimum viable reader:**
- [ ] Parse header (64 bytes)
- [ ] Locate End Record (scan backward)
- [ ] Parse Central Directory (320-byte entries)
- [ ] Build path → entry hash map
- [ ] Read file data (seek to offset)
- [ ] Decompress (LZ4/Zstd/Deflate)
- [ ] Verify CRC32

**Production reader:**
- [ ] All of above
- [ ] Path validation (traversal prevention)
- [ ] Version checking (reject incompatible)
- [ ] Error recovery (handle partial corruption)
- [ ] SQLite VFS integration
- [ ] Mount-once caching
- [ ] Memory mapping (for large archives)

**Writer:**
- [ ] Collect files
- [ ] Compute CRC32s
- [ ] Choose compression per file
- [ ] Write local entries
- [ ] Build Central Directory
- [ ] Write End Record
- [ ] Verify round-trip (can read what was written)

---

_Document version: 1.0_
_Created: 2025-01-05_
_Author: Engram Team_
_Based on: engram_format_theory.md, engram_architecture_clarification.md, SPECIFICATION.md, and format.rs_
