# Phase 1.2: Fuzzing Infrastructure - Setup Complete

**Date:** 2025-12-24
**Status:** ✅ Infrastructure Ready
**Tool:** cargo-fuzz + libFuzzer

## Summary

Fuzzing infrastructure has been set up for engram-rs using cargo-fuzz. The system can generate arbitrary corrupted archives to find edge cases and panics.

## Setup Details

### Installation

```bash
cargo install cargo-fuzz
cd engram-rs
cargo fuzz init
```

### Fuzz Targets

**Location:** `fuzz/fuzz_targets/fuzz_archive_parse.rs`

**Target:** Archive parsing and reading operations

**Tests for:**
- Archive header parsing (`ArchiveReader::open`)
- Central directory parsing (`initialize`)
- File listing (`list_files`)
- File reading (`read_file`)
- Path handling (including path traversal attempts)

**Key Properties Being Tested:**
1. **No Panics** - All operations should return `Result::Err` for invalid data, never panic
2. **Memory Safety** - No buffer overflows, use-after-free, etc.
3. **Path Traversal** - Attempts like `../../../etc/passwd` should be handled safely
4. **Edge Cases** - Empty files, truncated data, corrupted metadata

### Seed Corpus

**Location:** `fuzz/corpus/fuzz_archive_parse/`

**6 Seeds Generated:**
1. `seed_empty.eng` - Empty archive (no files)
2. `seed_single_small.eng` - Single 13-byte file
3. `seed_multi.eng` - Multiple files with directory structure
4. `seed_large.eng` - Large file with compression (~35KB)
5. `seed_binary.eng` - Binary data (all bytes 0-255)
6. `seed_zero_length.eng` - Archive with zero-length file

**Regenerate Seeds:**
```bash
cargo run --example generate_fuzz_seeds
```

## Usage

### Build Fuzz Target

```bash
cargo fuzz build fuzz_archive_parse
```

### Run Fuzzing

```bash
# Run for 60 seconds
cargo fuzz run fuzz_archive_parse -- -max_total_time=60

# Run with specific number of iterations
cargo fuzz run fuzz_archive_parse -- -runs=10000

# Run with more aggressive mutations
cargo fuzz run fuzz_archive_parse -- -max_len=100000 -len_control=0

# Use all CPU cores
cargo fuzz run fuzz_archive_parse -- -jobs=$(nproc)
```

### Check for Crashes

```bash
# Crashes are saved to fuzz/artifacts/fuzz_archive_parse/
ls fuzz/artifacts/fuzz_archive_parse/

# Reproduce a crash
cargo fuzz run fuzz_archive_parse fuzz/artifacts/fuzz_archive_parse/crash-HASH
```

### Coverage Report

```bash
cargo fuzz coverage fuzz_archive_parse
```

## Fuzzing Strategy

### Input Mutations

libFuzzer will automatically:
1. **Bit flips** - Flip random bits in valid archives
2. **Byte mutations** - Change random bytes
3. **Block deletions** - Remove chunks of data
4. **Block insertions** - Add random data
5. **Cross-pollination** - Combine parts of different seeds

### Expected Findings

**Normal (Not Bugs):**
- `EngramError::InvalidMagic` for corrupted magic numbers
- `EngramError::UnsupportedVersion` for invalid versions
- `EngramError::Io` for truncated data
- `EngramError::InvalidFormat` for corrupted structures

**Potential Bugs to Watch For:**
- **Panics** - Any panic is a bug (should return `Result::Err`)
- **Integer overflows** - Corrupted size fields causing arithmetic panics
- **Buffer overflows** - Reading past allocated memory
- **Infinite loops** - Corrupted pointers causing endless loops
- **Stack overflows** - Deep recursion from corrupted data

### Continuous Fuzzing Recommendations

For production-ready status, run fuzzing:

1. **Initial:** 24 hours on fast machine
2. **Weekly:** 1 hour regression fuzzing
3. **Pre-release:** 8+ hours before major releases
4. **CI Integration:** 5 minutes per commit (fast smoke test)

## Integration with Testing Suite

### Relationship to Phase 1.1

Phase 1.1 (Corruption Detection) tests **specific, known corruption scenarios**.
Phase 1.2 (Fuzzing) tests **millions of random mutations** to find unknown edge cases.

**Complementary Strengths:**
- Phase 1.1: Predictable, repeatable, documents expected behavior
- Phase 1.2: Unpredictable, discovers unknown vulnerabilities

### Test Pyramid

```
              /\
             /  \    Integration Tests (Phase 1.1)
            /____\   - Specific corruption scenarios
           /      \  - Known edge cases
          /        \ - Regression tests
         /__________\
        Fuzzing      Property-Based
        (Phase 1.2)  (Future)
```

## Fuzz Target Implementation

**File:** `fuzz/fuzz_targets/fuzz_archive_parse.rs`

### Design Decisions

1. **Minimum Size Check (64 bytes)**
   - Headers are 64 bytes minimum
   - Skip smaller inputs to reduce noise

2. **Temporary Files**
   - Write fuzz input to temp file (archives must be on disk)
   - Automatic cleanup via `NamedTempFile`

3. **Graceful Error Handling**
   - All `Result::Err` are expected and ignored
   - Only panics are reported as bugs

4. **Comprehensive API Coverage**
   - Tests: open, initialize, list_files, read_file, contains
   - Includes path traversal attempts

### Code Quality

- ✅ No `unsafe` code
- ✅ No `unwrap()` calls (all errors handled)
- ✅ Memory efficient (temp files cleaned up)
- ✅ Fast (skips invalid inputs early)

## Next Steps (Future Work)

### Additional Fuzz Targets

1. **fuzz_archive_write** - Test archive creation with corrupted inputs
2. **fuzz_manifest** - Test JSON manifest parsing
3. **fuzz_signatures** - Test Ed25519 signature verification
4. **fuzz_encryption** - Test encrypted archive handling
5. **fuzz_vfs** - Test SQLite VFS operations

### Advanced Fuzzing

1. **Structure-Aware Fuzzing**
   - Custom mutator that understands .eng format
   - Mutate fields intelligently (e.g., valid magic but corrupted offsets)

2. **Dictionary-Based Fuzzing**
   - Provide dictionary of valid field values
   - Increase coverage of format-specific code paths

3. **Differential Fuzzing**
   - Compare engram-rs vs. reference implementation
   - Find semantic differences

## Performance

**Expected Performance:**
- Initial corpus: 6 files
- Mutations/sec: 100-10,000 (depending on CPU)
- Coverage: Should reach 60-80% of reader code

**Actual measurements:** (To be added after initial fuzzing run)

## Conclusion

Phase 1.2 successfully establishes fuzzing infrastructure for engram-rs. The system is ready for continuous fuzzing to discover edge cases and potential panics.

**Key Achievements:**
- ✅ cargo-fuzz installed and configured
- ✅ Fuzz target implemented (no unsafe code)
- ✅ 6-file seed corpus generated
- ✅ Build verified (compiles successfully)
- ✅ Documentation complete

**Ready for:**
- Long-running fuzzing campaigns
- CI integration
- Crash reproduction and debugging

---

**Generated:** 2025-12-24
**Fuzz Target:** `fuzz/fuzz_targets/fuzz_archive_parse.rs`
**Seed Corpus:** 6 files in `fuzz/corpus/fuzz_archive_parse/`
**Status:** Infrastructure ready, fuzzing not yet run
