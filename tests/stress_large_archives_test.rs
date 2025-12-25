//! Phase 3.1: Large Archive Stress Tests
//!
//! Tests for very large archives, many files, and extreme scenarios.
//! Based on TESTING_PLAN.md Phase 3.1
//!
//! Run with: cargo test --test stress_large_archives_test -- --ignored --nocapture

use engram_rs::{ArchiveReader, ArchiveWriter};
use std::time::Instant;
use tempfile::NamedTempFile;

#[test]
#[ignore] // Run manually: cargo test test_1gb_archive -- --ignored
fn test_1gb_archive() {
    println!("\n🚀 Creating 1GB archive (100 × 10MB files)...");
    let start = Instant::now();

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // Create archive with 100 files of 10MB each
    let mut writer = ArchiveWriter::create(path).unwrap();

    for i in 0..100 {
        let filename = format!("file{:03}.bin", i);
        let data = vec![i as u8; 10 * 1024 * 1024]; // 10MB
        writer.add_file(&filename, &data).unwrap();

        if (i + 1) % 10 == 0 {
            println!("  Added {} files ({} MB)...", i + 1, (i + 1) * 10);
        }
    }

    writer.finalize().unwrap();
    let create_time = start.elapsed();

    // Verify archive size
    let size = std::fs::metadata(path).unwrap().len();
    let size_mb = size / 1024 / 1024;
    println!("  ✓ Archive created: {} MB", size_mb);
    println!("  ⏱ Creation time: {:?}", create_time);

    // Open and verify random files
    println!("\n📖 Reading archive...");
    let read_start = Instant::now();
    let mut reader = ArchiveReader::open_and_init(path).unwrap();

    // Verify file count
    let files = reader.list_files();
    assert_eq!(files.len(), 100);

    // Read random files
    let test_indices = [0, 25, 50, 75, 99];
    for &idx in &test_indices {
        let filename = format!("file{:03}.bin", idx);
        let data = reader.read_file(&filename).unwrap();
        assert_eq!(data.len(), 10 * 1024 * 1024);
        assert_eq!(data[0], idx as u8);
        println!("  ✓ Verified {}", filename);
    }

    let read_time = read_start.elapsed();
    println!("  ⏱ Read time: {:?}", read_time);

    println!("\n✅ 1GB archive test complete!");
    println!("  Archive size: {} MB", size_mb);
    println!("  Files: 100");
    println!("  Create: {:?}", create_time);
    println!("  Read: {:?}", read_time);
}

#[test]
#[ignore] // Run manually: cargo test test_10k_small_files -- --ignored
fn test_10k_small_files() {
    println!("\n🚀 Creating archive with 10,000 small files...");
    let start = Instant::now();

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // 10,000 files × ~1KB each = ~10MB total
    for i in 0..10_000 {
        let filename = format!("file{:05}.txt", i);
        let data = format!("file {} data ", i).repeat(100); // ~1.2KB
        writer.add_file(&filename, data.as_bytes()).unwrap();

        if (i + 1) % 1000 == 0 {
            println!("  Added {} files...", i + 1);
        }
    }

    writer.finalize().unwrap();
    let create_time = start.elapsed();

    // Verify archive
    let size = std::fs::metadata(path).unwrap().len();
    let size_mb = size / 1024 / 1024;
    println!("  ✓ Archive created: {} MB", size_mb);
    println!("  ⏱ Creation time: {:?}", create_time);

    // Open and verify
    println!("\n📖 Opening archive with 10K files...");
    let open_start = Instant::now();
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let open_time = open_start.elapsed();

    let all_files = reader.list_files();
    assert_eq!(all_files.len(), 10_000);
    println!("  ✓ Listed 10,000 files");
    println!("  ⏱ Open + list time: {:?}", open_time);

    // Random access test
    println!("\n🎯 Random access test (1000 reads)...");
    let read_start = Instant::now();
    for i in (0..10_000).step_by(10) {
        let filename = format!("file{:05}.txt", i);
        let data = reader.read_file(&filename).unwrap();
        assert!(data.len() > 0);
    }
    let read_time = read_start.elapsed();
    println!("  ✓ 1000 random reads");
    println!("  ⏱ Read time: {:?}", read_time);
    println!("  ⏱ Avg per file: {:?}", read_time / 1000);

    println!("\n✅ 10K files test complete!");
    println!("  Total files: 10,000");
    println!("  Archive size: {} MB", size_mb);
    println!("  Create: {:?}", create_time);
    println!("  Open: {:?}", open_time);
    println!("  1000 reads: {:?}", read_time);
}

#[test]
#[ignore] // Run manually: cargo test test_1000_files_regular -- --ignored
fn test_1000_files_regular() {
    println!("\n🚀 Creating archive with 1,000 files (non-ignored baseline)...");
    let start = Instant::now();

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    for i in 0..1000 {
        let filename = format!("file{:04}.txt", i);
        let data = format!("data{}", i);
        writer.add_file(&filename, data.as_bytes()).unwrap();
    }

    writer.finalize().unwrap();
    let create_time = start.elapsed();

    // Verify
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let files = reader.list_files();
    assert_eq!(files.len(), 1000);

    // Read all files
    for i in 0..1000 {
        let filename = format!("file{:04}.txt", i);
        let data = reader.read_file(&filename).unwrap();
        assert_eq!(data, format!("data{}", i).as_bytes());
    }

    let total_time = start.elapsed();

    println!("  ✓ 1000 files created and verified");
    println!("  ⏱ Create: {:?}", create_time);
    println!("  ⏱ Total: {:?}", total_time);
}

#[test]
fn test_maximum_path_length() {
    println!("\n🚀 Testing maximum path length (255 bytes)...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Test 255-byte path (maximum allowed in engram format)
    let long_path = "a".repeat(200) + "/" + &"b".repeat(54); // 255 chars total
    assert_eq!(long_path.len(), 255);

    writer.add_file(&long_path, b"data in long path").unwrap();
    writer.finalize().unwrap();

    // Read back
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let data = reader.read_file(&long_path).unwrap();
    assert_eq!(data, b"data in long path");

    println!("  ✓ 255-byte path works correctly");
}

#[test]
fn test_path_length_boundary() {
    println!("\n🚀 Testing path length boundaries...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Test various path lengths
    for len in [1, 50, 100, 150, 200, 254, 255] {
        let test_path = "x".repeat(len);
        writer
            .add_file(&test_path, format!("length {}", len).as_bytes())
            .unwrap();
    }

    writer.finalize().unwrap();

    // Read back all paths
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let files = reader.list_files();
    assert_eq!(files.len(), 7);

    for len in [1, 50, 100, 150, 200, 254, 255] {
        let test_path = "x".repeat(len);
        let data = reader.read_file(&test_path).unwrap();
        assert_eq!(data, format!("length {}", len).as_bytes());
    }

    println!("  ✓ Path lengths 1-255 all work correctly");
}

#[test]
fn test_deep_directory_structure() {
    println!("\n🚀 Testing deep directory structure...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Create files in deep directory structure (within 255 char limit)
    // dir1/dir2/dir3/.../dir20/file.txt
    let mut deep_path = String::new();
    for i in 1..=20 {
        if i > 1 {
            deep_path.push('/');
        }
        deep_path.push_str(&format!("d{}", i));
    }
    deep_path.push_str("/file.txt");

    assert!(deep_path.len() <= 255, "Path too long: {}", deep_path.len());

    writer.add_file(&deep_path, b"deep file data").unwrap();
    writer.finalize().unwrap();

    // Read back
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let data = reader.read_file(&deep_path).unwrap();
    assert_eq!(data, b"deep file data");

    println!("  ✓ Deep directory (20 levels) works correctly");
}

#[test]
#[ignore] // Run manually: cargo test test_500mb_archive -- --ignored
fn test_500mb_archive() {
    println!("\n🚀 Creating 500MB archive (50 × 10MB files)...");
    let start = Instant::now();

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    for i in 0..50 {
        let filename = format!("file{:02}.bin", i);
        let data = vec![i as u8; 10 * 1024 * 1024]; // 10MB
        writer.add_file(&filename, &data).unwrap();

        if (i + 1) % 10 == 0 {
            println!("  Added {} files...", i + 1);
        }
    }

    writer.finalize().unwrap();
    let create_time = start.elapsed();

    // Verify
    let size = std::fs::metadata(path).unwrap().len();
    let size_mb = size / 1024 / 1024;
    println!("  ✓ Archive created: {} MB", size_mb);
    println!("  ⏱ Creation time: {:?}", create_time);

    // Read verification
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let files = reader.list_files();
    assert_eq!(files.len(), 50);

    // Spot check a few files
    for &idx in &[0, 24, 49] {
        let filename = format!("file{:02}.bin", idx);
        let data = reader.read_file(&filename).unwrap();
        assert_eq!(data.len(), 10 * 1024 * 1024);
        assert_eq!(data[0], idx as u8);
    }

    println!("  ✓ Verified archive integrity");
    println!("\n✅ 500MB archive test complete!");
}

#[test]
fn test_many_small_files_baseline() {
    // Non-ignored baseline test with 1000 files
    println!("\n🚀 Baseline: 1000 small files...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    for i in 0..1000 {
        let filename = format!("f{}.txt", i);
        writer.add_file(&filename, b"x").unwrap();
    }

    writer.finalize().unwrap();

    let reader = ArchiveReader::open_and_init(path).unwrap();
    let files = reader.list_files();
    assert_eq!(files.len(), 1000);

    println!("  ✓ 1000 tiny files work correctly");
}
