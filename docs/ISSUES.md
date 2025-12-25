# Known Issues and Improvements for Engram Core

## Issue #1: Path Separator Normalization

**Priority:** HIGH
**Component:** `writer.rs`, `reader.rs`

### Problem
The current implementation preserves OS-specific path separators (backslashes on Windows, forward slashes on Unix) when adding files to archives. This creates cross-platform compatibility issues:

- Windows archive: `users\users.json`
- Unix archive: `users/users.json`
- Reading fails when path separators don't match

### Current Workaround
Applications must manually normalize paths before adding files:
```typescript
const normalizedPath = file.path.replace(/\\/g, '/')
writer.addFileWithCompression(normalizedPath, fileData, compression)
```

### Proper Solution
**The Engram format should enforce forward slashes internally** (like ZIP, TAR, JAR formats):

1. **In `ArchiveWriter::add_file()`**: Normalize all incoming paths to forward slashes
   ```rust
   pub fn add_file(&mut self, path: &str, data: &[u8]) -> Result<()> {
       // Normalize path separators to forward slashes (cross-platform standard)
       let normalized_path = path.replace('\\', "/");

       // Rest of implementation uses normalized_path
       // ...
   }
   ```

2. **In `ArchiveWriter::add_file_from_disk()`**: Normalize before storing
   ```rust
   pub fn add_file_from_disk(&mut self, archive_path: &str, disk_path: &Path) -> Result<()> {
       let normalized_path = archive_path.replace('\\', "/");
       // Use normalized_path when creating entry
   }
   ```

3. **In `ArchiveReader::read_file()`**: Accept both formats for backward compatibility
   ```rust
   pub fn read_file(&self, path: &str) -> Result<Vec<u8>> {
       let normalized_path = path.replace('\\', "/");
       // Try normalized path first, fall back to original for legacy archives
   }
   ```

### Benefits
- Archives are portable across platforms
- Applications don't need to handle normalization
- Matches behavior of standard archive formats (ZIP, TAR)
- Reading/writing "just works" regardless of OS

### Testing Requirements
- Create archive on Windows, read on Linux
- Create archive on Linux, read on Windows
- Verify all paths use forward slashes in archive metadata
- Backward compatibility: ensure old archives with backslashes still work

---

## Issue #2: Manifest Scope and Separation of Concerns

**Priority:** HIGH (Architectural Decision)
**Component:** `writer.rs`, Specification

### Architectural Principle

**The Engram manifest should only describe the archive itself, not application-specific metadata.**

This follows established patterns in mature formats:
- **JAR**: `META-INF/MANIFEST.MF` (archive format) + application manifests
- **Docker**: Image manifest (layers, metadata) + application files
- **NPM/Cargo**: Package manifest (format) + application code

### Engram Manifest Scope (Format-Level Only)

`manifest.json` should contain **only** archive-level metadata:

```json
{
  "name": "archive-name",
  "version": "1.0.0",
  "description": "Human-readable description",
  "author": { "name": "...", "email": "..." },
  "created": "2025-01-25T10:00:00Z",
  "files": [
    {
      "path": "app-data.json",
      "sha256": "...",
      "size": 1024,
      "mime_type": "application/json"
    }
  ],
  "signatures": [
    {
      "algorithm": "ed25519",
      "public_key": "...",
      "signature": "...",
      "timestamp": 1706184000,
      "signer": "Archive Creator"
    }
  ],
  "capabilities": ["compression", "signing"],
  "metadata": {
    "engram_version": "0.2.0",
    "compression_default": "zstd"
  }
}
```

### Application Manifest Scope (Custom Files)

Applications should store their own metadata in **separate files**:

```typescript
// Crisis Tower example
writer.addManifest(engramManifest)  // Format-level: manifest.json
writer.addJson('crisis-tower.json', {  // Application-level
  services: ['users', 'bookmarks', 'procedures'],
  dependencies: { bookmarks: ['users'], procedures: ['users'] },
  version: '3.0.0',
  created: '2025-01-25T10:00:00Z'
})
```

**Benefits:**
- Clear separation of concerns
- Engram manifest stays lean and format-focused
- Applications have full control over their metadata structure
- Multiple applications can coexist in one archive
- Follows industry-standard patterns

### Problem with Current Implementation

`ArchiveWriter::add_manifest()` was ambiguous about manifest scope:
1. Is it for **format metadata** (Engram-specific)?
2. Is it for **application metadata** (Crisis Tower-specific)?

Current workaround in crisis-tower:
```typescript
writer.addManifest(engramManifest)  // Writes to manifest.json
writer.addJson('crisis-tower-manifest.json', customManifest)  // Separate file
```

This works but wasn't intentional - it exposed the architectural ambiguity.

### Recommended Solution

**Document the separation and enforce it in the specification:**

#### 1. Specification Update

Add to Engram specification:

> **Manifest Scope**
>
> The Engram manifest (`manifest.json`) is **reserved for format-level metadata only**:
> - Archive identification (name, version, description)
> - File inventory with integrity hashes
> - Digital signatures for verification
> - Format capabilities and compression metadata
>
> **Applications must use separate files for application-specific metadata:**
> - Recommended pattern: `<app-name>.json` (e.g., `crisis-tower.json`, `myapp.json`)
> - Applications may store multiple metadata files as needed
> - This allows multiple applications to coexist in one archive
>
> **Example:**
> ```
> archive.eng
> ├── manifest.json           (Engram format metadata)
> ├── crisis-tower.json       (Crisis Tower metadata)
> ├── users/users.json        (Application data)
> └── bookmarks/bookmarks.json
> ```

#### 2. Implementation Clarification

Keep current `add_manifest()` implementation but clarify its purpose:

```rust
/// Add the Engram format manifest.
///
/// This writes the archive-level metadata to `manifest.json`. The manifest should contain
/// only format-level information (name, version, signatures, file list).
///
/// **Important:** Applications should store their own metadata in separate files using
/// `add_json()` or `add_file()`. Example: `writer.add_json("myapp.json", app_metadata)`
///
/// **Reserved Fields:**
/// - `name`: Archive name
/// - `version`: Archive version
/// - `description`: Human-readable description
/// - `files`: File inventory with hashes
/// - `signatures`: Digital signatures
/// - `capabilities`: Format capabilities
///
/// **Custom Fields:**
/// Applications may add custom fields to `metadata`, but format-level fields take precedence.
pub fn add_manifest(&mut self, manifest: &serde_json::Value) -> Result<()> {
    if self.entries.iter().any(|e| e.path == "manifest.json") {
        return Err(EngramError::DuplicateEntry(
            "manifest.json already exists - call add_manifest() only once".into()
        ));
    }

    let json = serde_json::to_vec_pretty(manifest)?;
    self.add_file_with_compression("manifest.json", &json, CompressionMethod::None)
}
```

#### 3. Reader Convenience Methods

Add helpers for common patterns:

```rust
impl ArchiveReader {
    /// Read the Engram format manifest
    pub fn read_manifest(&self) -> Result<serde_json::Value> {
        self.read_json("manifest.json")
    }

    /// Read an application-specific manifest
    ///
    /// Example: `archive.read_app_manifest("crisis-tower")`
    /// Reads from: `crisis-tower.json`
    pub fn read_app_manifest(&self, app_name: &str) -> Result<serde_json::Value> {
        self.read_json(&format!("{}.json", app_name))
    }

    /// Check if an application manifest exists
    pub fn has_app_manifest(&self, app_name: &str) -> bool {
        self.contains(&format!("{}.json", app_name))
    }
}
```

### Benefits of This Approach

1. **Clear architectural boundary**: Format vs. application concerns
2. **Follows industry standards**: JAR, Docker, NPM patterns
3. **Simple implementation**: No complex multi-manifest logic needed
4. **Backward compatible**: Existing archives unchanged
5. **Self-documenting**: Pattern is clear from file structure
6. **Flexible**: Applications can use any structure they need

### Documentation for Application Developers

Add to Engram documentation:

> **Best Practices for Application Manifests**
>
> When creating Engram archives for your application:
>
> 1. **Use `add_manifest()` for format metadata only**
>    - Archive name, version, signatures
>    - Let Engram manage this automatically
>
> 2. **Store application metadata in separate files**
>    ```typescript
>    writer.addManifest(engramMetadata)  // Format level
>    writer.addJson('myapp.json', appMetadata)  // Application level
>    ```
>
> 3. **Recommended naming**: `<app-name>.json`
>    - Clear ownership
>    - Avoids conflicts
>    - Easy to discover
>
> 4. **Multiple applications can coexist**
>    ```
>    archive.eng
>    ├── manifest.json        (Engram)
>    ├── crisis-tower.json    (Crisis Tower)
>    ├── backup-tool.json     (Backup Tool)
>    └── data/...
>    ```

---

## Issue #3: Signature Verification Edge Cases

**Priority:** LOW
**Component:** Format specification

### Problem
Current signature implementation assumes:
- Signatures are in manifest
- Manifest is JSON
- Canonical form is deterministic

### Edge Cases to Consider
1. What if manifest contains floating-point numbers? (JSON serialization is non-deterministic)
2. What if manifest is very large? (Should signatures be separate file?)
3. What if archive is modified after signing? (Need to detect tampering)

### Potential Improvements
- Consider signing the entire archive (manifest + all files)
- Add integrity checks beyond just manifest signatures
- Document canonical JSON serialization requirements
- Consider separate signature file (`.eng.sig`) for very large archives

---

## Testing Checklist
- [ ] Cross-platform path normalization tests
- [ ] Multiple manifest support tests
- [ ] Signature verification edge case tests
- [ ] Backward compatibility with existing archives
- [ ] Performance benchmarks for normalization overhead

---

**Note:** These issues were discovered during crisis-tower v3 backup/restore implementation (2025-01-25).
The current TypeScript layer includes workarounds, but proper fixes should be in the Rust core.
