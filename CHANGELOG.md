# Changelog

All notable changes to engram-rs will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2025-12-19

### Added
- **Local Entry Headers (LOCA)**: Variable-length headers preceding each file's compressed data
  - Enables sequential streaming reads without consulting central directory
  - Includes signature validation (0x4C4F4341 - "LOCA")
  - Stores file metadata: sizes, CRC32, timestamp, compression method, path
- **End of Central Directory Record (ENDR)**: 64-byte record at archive end
  - Enables validation of archive completeness
  - Allows readers to locate central directory by reading from end
  - Signature: 0x454E4452 ("ENDR")
  - Stores version, CD offset/size, entry count, archive CRC32
- **Frame-Based Compression**: Independent 64KB frames for large files (≥50MB)
  - Enables partial decompression without reading entire file
  - Automatic activation for files ≥50MB
  - Supports both LZ4 and Zstandard compression
  - Frame structure: [frame_count][frame1_size][frame1_data][frame2_size][frame2_data]...
- **Archive-Level Encryption Support**: Full encryption mode flags in header
  - AES-256-GCM encryption for entire archive payload
  - Per-file encryption mode support
  - Encryption mode properly persisted and validated in v1.0 format
- **Comprehensive Test Coverage**: 43 total tests
  - 23 unit tests for core functionality
  - 10 integration tests for real-world scenarios
  - 7 v1.0 feature-specific tests
  - 3 documentation tests

### Changed
- **BREAKING**: Format version bumped from 0.4 to 1.0
  - Major version increment due to format changes
  - Not backward compatible with v0.3/v0.4 archives (write-only v1.0)
- Updated compression threshold from 1KB to 4KB for better heuristics
- Modified header reading logic to support v1.0+ version detection
- Archive structure now: [Header][LOCA+Data]...[Central Directory][ENDR]

### Fixed
- Fixed encryption mode flags not being read correctly in v1.0 format
- Corrected version compatibility check to handle major version >= 1
- Fixed archive-level encryption to exclude ENDR from encrypted payload

### Technical Details
- **Version Format**: 1.0 (major.minor)
- **LOCA Header Size**: Variable (40 + path_length + 1 bytes minimum)
- **ENDR Size**: Fixed 64 bytes
- **Frame Size**: 64KB (65,536 bytes)
- **Frame Threshold**: 50MB (52,428,800 bytes)
- **Compression Methods**: None (0), LZ4 (1), Zstandard (2)
- **Encryption Modes**: None (0b00), Archive (0b01), Per-File (0b10)

## [0.4.1] - 2024

### Added
- Initial manifest support with Ed25519 signatures
- VFS (Virtual File System) for embedded SQLite databases
- Archive-level and per-file encryption (AES-256-GCM)
- TOML manifest builder

### Changed
- Unified crate structure (merged engram-core + engram-vfs)
- Improved compression selection heuristics

## [0.3.0] - 2024

### Added
- Initial Engram archive format
- Central directory for O(1) file lookup
- LZ4 and Zstandard compression
- CRC32 integrity verification

[1.0.0]: https://github.com/Manifest-Humanity/engram-rs/compare/v0.4.1...v1.0.0
[0.4.1]: https://github.com/Manifest-Humanity/engram-rs/compare/v0.3.0...v0.4.1
[0.3.0]: https://github.com/Manifest-Humanity/engram-rs/releases/tag/v0.3.0
