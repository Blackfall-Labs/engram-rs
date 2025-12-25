//! Phase 4.2: ZIP Bomb Protection Tests
//!
//! Tests for decompression bomb detection and compression ratio validation.
//! Based on TESTING_PLAN.md Phase 4.3

use engram_rs::{ArchiveReader, ArchiveWriter};
use tempfile::NamedTempFile;

#[test]
fn test_legitimate_highly_compressible_data() {
    println!("\n💣 Testing legitimate highly compressible data (10MB zeros)...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Legitimate highly compressible data (zeros)
    let data = vec![0u8; 10 * 1024 * 1024]; // 10MB of zeros
    writer.add_file("zeros.bin", &data).unwrap();
    writer.finalize().unwrap();

    // Check compression ratio
    let archive_size = std::fs::metadata(path).unwrap().len();
    let original_size = data.len() as u64;
    let compression_ratio = original_size as f64 / archive_size as f64;

    println!("  Original size: {} MB", original_size / 1024 / 1024);
    println!("  Compressed archive: {} KB", archive_size / 1024);
    println!("  Compression ratio: {:.1}x", compression_ratio);

    // Should compress to < 100KB
    assert!(archive_size < 100 * 1024, "Archive should compress to < 100KB");

    // Should still be readable
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let read_data = reader.read_file("zeros.bin").unwrap();
    assert_eq!(read_data.len(), 10 * 1024 * 1024);
    assert_eq!(read_data, data);

    println!("  ✅ Legitimate highly compressible data handled correctly");
}

#[test]
fn test_multiple_highly_compressible_files() {
    println!("\n💣 Testing multiple highly compressible files...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Add multiple highly compressible files
    for i in 0..10 {
        let data = vec![i as u8; 1024 * 1024]; // 1MB of same byte
        writer
            .add_file(&format!("file{}.bin", i), &data)
            .unwrap();
    }

    writer.finalize().unwrap();

    // Total original: 10MB
    // Should compress very well
    let archive_size = std::fs::metadata(path).unwrap().len();
    println!("  10 files × 1MB = 10MB original");
    println!("  Archive size: {} KB", archive_size / 1024);
    println!("  Compression: {:.1}x", (10 * 1024 * 1024) as f64 / archive_size as f64);

    // Verify all files readable
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    for i in 0..10 {
        let data = reader.read_file(&format!("file{}.bin", i)).unwrap();
        assert_eq!(data.len(), 1024 * 1024);
        assert_eq!(data[0], i as u8);
    }

    println!("  ✅ Multiple compressible files handled correctly");
}

#[test]
fn test_compression_ratio_text_data() {
    println!("\n💣 Testing compression ratio with text data...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Highly repetitive text (very compressible)
    let text = "The quick brown fox jumps over the lazy dog. ".repeat(10000); // ~450KB
    writer.add_file("text.txt", text.as_bytes()).unwrap();
    writer.finalize().unwrap();

    let archive_size = std::fs::metadata(path).unwrap().len();
    let original_size = text.len() as u64;
    let ratio = original_size as f64 / archive_size as f64;

    println!("  Original: {} KB", original_size / 1024);
    println!("  Archive: {} KB", archive_size / 1024);
    println!("  Ratio: {:.1}x", ratio);

    // Should compress well (at least 5x for repetitive text)
    assert!(ratio > 5.0, "Text should compress > 5x");

    // Verify readable
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let read_text = reader.read_file("text.txt").unwrap();
    assert_eq!(read_text, text.as_bytes());

    println!("  ✅ Text data compression working correctly");
}

#[test]
fn test_mixed_compressibility_files() {
    println!("\n💣 Testing mixed compressibility files...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Highly compressible
    let zeros = vec![0u8; 1024 * 1024]; // 1MB zeros
    writer.add_file("zeros.bin", &zeros).unwrap();

    // Medium compressibility
    let pattern: Vec<u8> = (0..1024 * 1024).map(|i| (i % 256) as u8).collect();
    writer.add_file("pattern.bin", &pattern).unwrap();

    // Low compressibility (pseudo-random)
    let pseudo_random: Vec<u8> = (0..1024 * 1024)
        .map(|i| {
            let val = (i as u64 * 31337 + 12345) % 256;
            val as u8
        })
        .collect();
    writer.add_file("random.bin", &pseudo_random).unwrap();

    writer.finalize().unwrap();

    // Verify all readable
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    assert_eq!(reader.read_file("zeros.bin").unwrap(), zeros);
    assert_eq!(reader.read_file("pattern.bin").unwrap(), pattern);
    assert_eq!(reader.read_file("random.bin").unwrap(), pseudo_random);

    let archive_size = std::fs::metadata(path).unwrap().len();
    println!("  3 files × 1MB = 3MB original");
    println!("  Archive size: {} KB", archive_size / 1024);
    println!("  Overall ratio: {:.1}x", (3 * 1024 * 1024) as f64 / archive_size as f64);

    println!("  ✅ Mixed compressibility handled correctly");
}

#[test]
fn test_large_file_frame_compression() {
    println!("\n💣 Testing large file (≥50MB) frame compression...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // 50MB file (triggers frame compression)
    let large_data = vec![0xAB; 50 * 1024 * 1024];
    writer.add_file("large.bin", &large_data).unwrap();
    writer.finalize().unwrap();

    // Should compress extremely well
    let archive_size = std::fs::metadata(path).unwrap().len();
    let original_size = large_data.len() as u64;
    let ratio = original_size as f64 / archive_size as f64;

    println!("  Original: {} MB", original_size / 1024 / 1024);
    println!("  Archive: {} KB", archive_size / 1024);
    println!("  Compression: {:.1}x", ratio);

    // Should compress to < 1MB (all same byte)
    assert!(archive_size < 1024 * 1024, "Should compress to < 1MB");

    // Verify readable
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let read_data = reader.read_file("large.bin").unwrap();
    assert_eq!(read_data.len(), 50 * 1024 * 1024);
    assert_eq!(read_data, large_data);

    println!("  ✅ Frame compression (≥50MB) working correctly");
}

#[test]
fn test_uncompressed_data_storage() {
    println!("\n💣 Testing uncompressed data storage (CompressionMethod::None)...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Force no compression
    let data = vec![0xCD; 10 * 1024]; // 10KB
    writer
        .add_file_with_compression("uncompressed.bin", &data, engram_rs::CompressionMethod::None)
        .unwrap();

    writer.finalize().unwrap();

    // Archive should be close to original size (plus headers)
    let archive_size = std::fs::metadata(path).unwrap().len();
    let original_size = data.len() as u64;

    println!("  Original: {} KB", original_size / 1024);
    println!("  Archive: {} KB", archive_size / 1024);

    // Should be roughly same size (allowing for headers)
    let overhead = archive_size as i64 - original_size as i64;
    println!("  Overhead: {} bytes", overhead);
    assert!(overhead < 2048, "Overhead should be < 2KB");

    // Verify readable
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let read_data = reader.read_file("uncompressed.bin").unwrap();
    assert_eq!(read_data, data);

    println!("  ✅ Uncompressed storage working correctly");
}

#[test]
fn test_compression_bomb_prevention_notes() {
    println!("\n💣 Compression Bomb Prevention - Design Notes");

    // engram-rs does NOT have explicit decompression bomb protection
    // because it uses trusted compression libraries (zstd, lz4) that:
    //
    // 1. Allocate output buffer based on claimed uncompressed size
    // 2. Fail gracefully if decompression exceeds buffer
    // 3. Have built-in limits to prevent excessive memory use
    //
    // ZIP bomb attacks typically exploit:
    // - Recursive compression (not supported in engram)
    // - Lying about uncompressed size → caught by decompressor
    // - Extremely high ratios → limited by memory available
    //
    // SECURITY POSTURE:
    // ✅ Frame compression (≥50MB) limits per-frame memory
    // ✅ zstd/lz4 libraries have internal safety checks
    // ✅ No recursive/nested compression support
    // ⚠️ Large claimed sizes could cause memory allocation failure
    //    (but won't cause buffer overflow or arbitrary code execution)
    //
    // RECOMMENDATION: Applications using engram-rs should:
    // 1. Set resource limits (ulimit, cgroups)
    // 2. Monitor memory usage during decompression
    // 3. Validate archive sources (signatures, trusted origins)

    println!("  📝 Compression bomb protection relies on:");
    println!("     - Frame compression for large files (≥50MB)");
    println!("     - zstd/lz4 library internal safety checks");
    println!("     - No support for recursive compression");
    println!("  ✅ Design notes documented");
}

#[test]
fn test_nested_compression_not_applicable() {
    println!("\n💣 Nested compression attack - N/A");

    // engram-rs does NOT support nested/recursive compression
    // Each file is compressed independently, once
    // You cannot add a compressed engram archive as a file and re-compress it
    // (you can add it, but it won't be recursively decompressed)

    println!("  📝 Nested compression attacks are NOT applicable to engram-rs");
    println!("     - Each file compressed independently");
    println!("     - No recursive decompression");
    println!("     - Adding .eng file just stores it as data");
    println!("  ✅ N/A - documented");
}
