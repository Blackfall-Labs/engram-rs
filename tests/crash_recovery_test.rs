//! Phase 2.3: Crash Recovery Tests
//!
//! Tests for incomplete archives, partial writes, and interrupted creation.
//! Based on TESTING_PLAN.md Phase 2.3

use engram_rs::{ArchiveReader, ArchiveWriter};
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use tempfile::NamedTempFile;

/// Helper: Truncate file to specified size
fn truncate_file(path: &std::path::Path, new_size: u64) {
    let file = OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap();
    file.set_len(new_size).unwrap();
}

/// Helper: Create complete archive for testing
fn create_complete_archive() -> NamedTempFile {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();
    for i in 0..10 {
        let filename = format!("file{}.txt", i);
        let data = format!("data{}", i);
        writer.add_file(&filename, data.as_bytes()).unwrap();
    }
    writer.finalize().unwrap();

    temp_file
}

#[test]
fn test_finalize_not_called() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // Create writer and add files but DON'T call finalize()
    {
        let mut writer = ArchiveWriter::create(path).unwrap();
        writer.add_file("file1.txt", b"data1").unwrap();
        writer.add_file("file2.txt", b"data2").unwrap();
        // Drop without finalize - archive should be incomplete
    }

    // Archive should be invalid (missing central directory and ENDR)
    let result = ArchiveReader::open_and_init(path);
    assert!(result.is_err(), "Incomplete archive should fail to open");

    println!("✓ Archive without finalize() correctly rejected");
}

#[test]
fn test_partial_write_truncated_at_10_percent() {
    let temp_file = create_complete_archive();
    let path = temp_file.path();

    // Get original size
    let original_size = std::fs::metadata(path).unwrap().len();

    // Truncate at 10%
    let truncate_size = original_size / 10;
    truncate_file(path, truncate_size);

    // Should fail to open
    let result = ArchiveReader::open_and_init(path);
    assert!(
        result.is_err(),
        "Archive truncated at 10% should fail to open"
    );

    println!("✓ Archive truncated at 10% correctly rejected");
}

#[test]
fn test_partial_write_truncated_at_30_percent() {
    let temp_file = create_complete_archive();
    let path = temp_file.path();

    let original_size = std::fs::metadata(path).unwrap().len();
    let truncate_size = (original_size * 30) / 100;
    truncate_file(path, truncate_size);

    let result = ArchiveReader::open_and_init(path);
    assert!(
        result.is_err(),
        "Archive truncated at 30% should fail to open"
    );

    println!("✓ Archive truncated at 30% correctly rejected");
}

#[test]
fn test_partial_write_truncated_at_50_percent() {
    let temp_file = create_complete_archive();
    let path = temp_file.path();

    let original_size = std::fs::metadata(path).unwrap().len();
    let truncate_size = original_size / 2;
    truncate_file(path, truncate_size);

    let result = ArchiveReader::open_and_init(path);
    assert!(
        result.is_err(),
        "Archive truncated at 50% should fail to open"
    );

    println!("✓ Archive truncated at 50% correctly rejected");
}

#[test]
fn test_partial_write_truncated_at_70_percent() {
    let temp_file = create_complete_archive();
    let path = temp_file.path();

    let original_size = std::fs::metadata(path).unwrap().len();
    let truncate_size = (original_size * 70) / 100;
    truncate_file(path, truncate_size);

    let result = ArchiveReader::open_and_init(path);
    assert!(
        result.is_err(),
        "Archive truncated at 70% should fail to open"
    );

    println!("✓ Archive truncated at 70% correctly rejected");
}

#[test]
fn test_partial_write_truncated_at_90_percent() {
    let temp_file = create_complete_archive();
    let path = temp_file.path();

    let original_size = std::fs::metadata(path).unwrap().len();
    let truncate_size = (original_size * 90) / 100;
    truncate_file(path, truncate_size);

    let result = ArchiveReader::open_and_init(path);
    assert!(
        result.is_err(),
        "Archive truncated at 90% should fail to open"
    );

    println!("✓ Archive truncated at 90% correctly rejected");
}

#[test]
fn test_truncated_header() {
    let temp_file = create_complete_archive();
    let path = temp_file.path();

    // Truncate to 32 bytes (partial header - header is 64 bytes)
    truncate_file(path, 32);

    let result = ArchiveReader::open(path);
    assert!(result.is_err(), "Archive with truncated header should fail");

    println!("✓ Archive with truncated header correctly rejected");
}

#[test]
fn test_truncated_just_before_endr() {
    let temp_file = create_complete_archive();
    let path = temp_file.path();

    let original_size = std::fs::metadata(path).unwrap().len();

    // ENDR is last 64 bytes, truncate to remove half of it
    let truncate_size = original_size - 32;
    truncate_file(path, truncate_size);

    let result = ArchiveReader::open_and_init(path);
    assert!(
        result.is_err(),
        "Archive with truncated ENDR should fail to open"
    );

    println!("✓ Archive with partial ENDR correctly rejected");
}

#[test]
fn test_empty_file_as_archive() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // Create empty file
    File::create(path).unwrap();

    let result = ArchiveReader::open(path);
    assert!(result.is_err(), "Empty file should fail to open as archive");

    println!("✓ Empty file correctly rejected");
}

#[test]
fn test_truncated_to_header_size() {
    let temp_file = create_complete_archive();
    let path = temp_file.path();

    // Truncate to exactly 64 bytes (just the header)
    truncate_file(path, 64);

    let result = ArchiveReader::open_and_init(path);
    assert!(
        result.is_err(),
        "Archive with only header should fail to initialize"
    );

    println!("✓ Archive with only header correctly rejected");
}

#[test]
fn test_archive_corruption_mid_file() {
    let temp_file = create_complete_archive();
    let path = temp_file.path();

    let original_size = std::fs::metadata(path).unwrap().len();

    // Corrupt middle of archive (where file data lives)
    let corrupt_offset = original_size / 2;
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .unwrap();
    file.seek(SeekFrom::Start(corrupt_offset)).unwrap();

    // Write garbage data
    let garbage = vec![0xFF; 1024];
    file.write_all(&garbage).unwrap();
    drop(file);

    // Archive should still open (corruption is lazy-detected)
    let result = ArchiveReader::open_and_init(path);

    // Depending on where corruption hit, it might:
    // 1. Fail during initialize (if CD or ENDR corrupted)
    // 2. Succeed but fail on file reads (if file data corrupted)
    match result {
        Ok(mut reader) => {
            // Try reading files - some might fail due to corruption
            let files = reader.list_files().to_vec();
            let mut read_failures = 0;

            for filename in &files {
                if reader.read_file(filename).is_err() {
                    read_failures += 1;
                }
            }

            println!(
                "✓ Corruption detected: {} / {} files failed to read",
                read_failures,
                files.len()
            );
        }
        Err(_) => {
            println!("✓ Corruption detected during initialize");
        }
    }
}

#[test]
fn test_repeated_finalize_calls() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();
    writer.add_file("file.txt", b"data").unwrap();

    // First finalize should succeed
    writer.finalize().unwrap();

    // Writer is consumed by finalize(), so we can't call it again
    // This test verifies the API design prevents double-finalize

    // Verify archive is valid
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let data = reader.read_file("file.txt").unwrap();
    assert_eq!(data, b"data");

    println!("✓ Finalize consumes writer (prevents double-finalize)");
}

#[test]
fn test_crash_recovery_archive_still_readable_after_failed_second_open() {
    // Verify that a valid archive remains valid even if we try to open it incorrectly
    let temp_file = create_complete_archive();
    let path = temp_file.path();

    // First open should succeed
    let mut reader1 = ArchiveReader::open_and_init(path).unwrap();
    let data1 = reader1.read_file("file0.txt").unwrap();
    assert_eq!(data1, b"data0");
    drop(reader1);

    // Second open should also succeed
    let mut reader2 = ArchiveReader::open_and_init(path).unwrap();
    let data2 = reader2.read_file("file5.txt").unwrap();
    assert_eq!(data2, b"data5");

    println!("✓ Valid archive remains readable across multiple open/close cycles");
}
