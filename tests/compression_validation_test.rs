//! Phase 3.2: Compression Method Validation
//!
//! Tests for automatic compression method selection and effectiveness.
//! Based on TESTING_PLAN.md Phase 3.2

use engram_rs::{ArchiveReader, ArchiveWriter, CompressionMethod};
use tempfile::NamedTempFile;

/// Helper: Get the compression method used for a file in an archive
/// This is a simplified test - in real impl, we'd inspect the EntryInfo
fn get_compression_ratio(original_size: usize, compressed_archive_size: u64) -> f64 {
    compressed_archive_size as f64 / original_size as f64
}

#[test]
fn test_compression_choice_small_files() {
    println!("\n🔍 Testing compression for small files (<4KB)...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Files < 4KB should typically not be compressed (or compress minimally)
    let small_data = vec![0xAB; 2048]; // 2KB
    writer.add_file("small.bin", &small_data).unwrap();
    writer.finalize().unwrap();

    // Verify readable
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let data = reader.read_file("small.bin").unwrap();
    assert_eq!(data, small_data);

    println!("  ✓ Small files compress and decompress correctly");
}

#[test]
fn test_compression_choice_text_files() {
    println!("\n🔍 Testing compression for text files (.txt, .json, .md)...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Highly compressible text data
    let text_data = "Lorem ipsum dolor sit amet ".repeat(1000); // Highly compressible

    writer.add_file("file.txt", text_data.as_bytes()).unwrap();
    writer.add_file("data.json", text_data.as_bytes()).unwrap();
    writer.add_file("readme.md", text_data.as_bytes()).unwrap();
    writer.finalize().unwrap();

    // Check compression ratio
    let original_size = text_data.len() * 3; // 3 files
    let archive_size = std::fs::metadata(path).unwrap().len();
    let ratio = get_compression_ratio(original_size, archive_size);

    println!("  Original: {} bytes", original_size);
    println!("  Archive: {} bytes", archive_size);
    println!("  Ratio: {:.2}x", 1.0 / ratio);

    // Should compress well (at least 2x)
    assert!(ratio < 0.5, "Text should compress to <50% (got {:.2}%)", ratio * 100.0);

    // Verify data integrity
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    assert_eq!(reader.read_file("file.txt").unwrap(), text_data.as_bytes());
    assert_eq!(reader.read_file("data.json").unwrap(), text_data.as_bytes());
    assert_eq!(reader.read_file("readme.md").unwrap(), text_data.as_bytes());

    println!("  ✓ Text files compress well (>{:.0}% reduction)", (1.0 - ratio) * 100.0);
}

#[test]
fn test_compression_choice_binary_files() {
    println!("\n🔍 Testing compression for binary files (.db, .wasm)...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Binary data (less compressible than text)
    let binary_data = vec![0x12; 100 * 1024]; // 100KB

    writer.add_file("data.db", &binary_data).unwrap();
    writer.add_file("app.wasm", &binary_data).unwrap();
    writer.finalize().unwrap();

    // Verify readable
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    assert_eq!(reader.read_file("data.db").unwrap(), binary_data);
    assert_eq!(reader.read_file("app.wasm").unwrap(), binary_data);

    println!("  ✓ Binary files compress and decompress correctly");
}

#[test]
fn test_compression_choice_already_compressed() {
    println!("\n🔍 Testing compression for already-compressed files (.png, .jpg, .zip)...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Simulate compressed file formats (with magic bytes)
    let fake_png = {
        let mut data = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        data.extend(vec![0xAB; 1000]);
        data
    };

    let fake_jpg = {
        let mut data = vec![0xFF, 0xD8, 0xFF, 0xE0];
        data.extend(vec![0xCD; 1000]);
        data
    };

    let fake_zip = {
        let mut data = vec![0x50, 0x4B, 0x03, 0x04];
        data.extend(vec![0xEF; 1000]);
        data
    };

    writer.add_file("image.png", &fake_png).unwrap();
    writer.add_file("photo.jpg", &fake_jpg).unwrap();
    writer.add_file("archive.zip", &fake_zip).unwrap();
    writer.finalize().unwrap();

    // Verify data integrity (compression detection doesn't matter, just that data survives)
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    assert_eq!(reader.read_file("image.png").unwrap(), fake_png);
    assert_eq!(reader.read_file("photo.jpg").unwrap(), fake_jpg);
    assert_eq!(reader.read_file("archive.zip").unwrap(), fake_zip);

    println!("  ✓ Pre-compressed files handled correctly");
}

#[test]
fn test_compression_effectiveness_highly_compressible() {
    println!("\n🔍 Testing compression effectiveness on highly compressible data...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // All zeros - extremely compressible
    let zeros = vec![0u8; 1024 * 1024]; // 1MB of zeros
    writer.add_file("zeros.bin", &zeros).unwrap();
    writer.finalize().unwrap();

    let original_size = zeros.len();
    let archive_size = std::fs::metadata(path).unwrap().len();
    let ratio = get_compression_ratio(original_size, archive_size);

    println!("  Original: {} bytes (1 MB)", original_size);
    println!("  Archive: {} bytes", archive_size);
    println!("  Compression: {:.2}x", 1.0 / ratio);

    // Should compress extremely well (>100x for zeros)
    assert!(ratio < 0.01, "Zeros should compress to <1% (got {:.4}%)", ratio * 100.0);

    // Verify data
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let data = reader.read_file("zeros.bin").unwrap();
    assert_eq!(data.len(), 1024 * 1024);
    assert_eq!(data, zeros);

    println!("  ✓ Highly compressible data: >{:.0}x compression", 1.0 / ratio);
}

#[test]
fn test_compression_effectiveness_mixed_data() {
    println!("\n🔍 Testing compression on mixed compressibility data...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Mix of highly compressible and less compressible data
    let mut mixed_data = Vec::with_capacity(100_000);

    // Part 1: Repeated pattern (compresses well)
    mixed_data.extend(vec![0xAA; 30_000]);

    // Part 2: Sequential bytes (medium compression)
    for i in 0..30_000 {
        mixed_data.push((i % 256) as u8);
    }

    // Part 3: More repeated patterns
    mixed_data.extend(vec![0x55; 40_000]);

    writer.add_file("mixed.bin", &mixed_data).unwrap();
    writer.finalize().unwrap();

    let original_size = mixed_data.len();
    let archive_size = std::fs::metadata(path).unwrap().len();
    let ratio = get_compression_ratio(original_size, archive_size);

    println!("  Original: {} bytes", original_size);
    println!("  Archive: {} bytes", archive_size);
    println!("  Compression ratio: {:.2}x", 1.0 / ratio);

    // Mixed data should compress reasonably (but not as much as pure zeros)
    assert!(ratio < 0.5, "Mixed data should compress to <50% (got {:.2}%)", ratio * 100.0);

    // Verify data integrity
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let data = reader.read_file("mixed.bin").unwrap();
    assert_eq!(data, mixed_data);

    println!("  ✓ Mixed data compressed correctly ({:.2}x)", 1.0 / ratio);
}

#[test]
fn test_explicit_compression_methods() {
    println!("\n🔍 Testing explicit compression method specification...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    let test_data = b"test data for compression";

    // Add files with explicit compression methods
    writer
        .add_file_with_compression("none.txt", test_data, CompressionMethod::None)
        .unwrap();
    writer
        .add_file_with_compression("lz4.txt", test_data, CompressionMethod::Lz4)
        .unwrap();
    writer
        .add_file_with_compression("zstd.txt", test_data, CompressionMethod::Zstd)
        .unwrap();
    writer.finalize().unwrap();

    // Verify all readable
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    assert_eq!(reader.read_file("none.txt").unwrap(), test_data);
    assert_eq!(reader.read_file("lz4.txt").unwrap(), test_data);
    assert_eq!(reader.read_file("zstd.txt").unwrap(), test_data);

    println!("  ✓ Explicit compression methods work correctly");
}

#[test]
fn test_mixed_compression_in_archive() {
    println!("\n🔍 Testing mixed compression methods in single archive...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Mix of compression methods
    let zeros = vec![0u8; 10_000]; // Compresses well
    let pattern: Vec<u8> = (0..10_000).map(|i| (i % 256) as u8).collect(); // Medium compression
    let text = "Hello World ".repeat(1000); // Compresses well

    writer.add_file_with_compression("zeros.bin", &zeros, CompressionMethod::Zstd).unwrap();
    writer.add_file_with_compression("pattern.bin", &pattern, CompressionMethod::Lz4).unwrap();
    writer.add_file_with_compression("text.txt", text.as_bytes(), CompressionMethod::Zstd).unwrap();
    writer.add_file_with_compression("raw.txt", b"small", CompressionMethod::None).unwrap();
    writer.finalize().unwrap();

    // Read and verify all files
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    assert_eq!(reader.read_file("zeros.bin").unwrap(), zeros);
    assert_eq!(reader.read_file("pattern.bin").unwrap(), pattern);
    assert_eq!(reader.read_file("text.txt").unwrap(), text.as_bytes());
    assert_eq!(reader.read_file("raw.txt").unwrap(), b"small");

    println!("  ✓ Mixed compression methods in single archive work correctly");
}
