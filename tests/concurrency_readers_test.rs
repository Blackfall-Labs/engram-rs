//! Phase 2.2: Multi-Reader Stress Tests
//!
//! Tests for concurrent archive readers, simultaneous file access, and decompression.
//! Based on TESTING_PLAN.md Phase 2.2

use engram_rs::{ArchiveReader, ArchiveWriter};
use std::thread;
use tempfile::NamedTempFile;

/// Helper: Create archive with N files
fn create_archive_with_files(file_count: usize) -> NamedTempFile {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();
    for i in 0..file_count {
        let filename = format!("file{}.txt", i);
        let data = format!("data{}", i);
        writer.add_file(&filename, data.as_bytes()).unwrap();
    }
    writer.finalize().unwrap();

    temp_file
}

/// Helper: Create archive with directory structure
fn create_archive_with_directories() -> NamedTempFile {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Create files in multiple directories
    for dir in 1..=5 {
        for file in 1..=10 {
            let filename = format!("dir{}/file{}.txt", dir, file);
            let data = format!("dir{}-file{}", dir, file);
            writer.add_file(&filename, data.as_bytes()).unwrap();
        }
    }

    writer.finalize().unwrap();
    temp_file
}

/// Helper: Create archive with large compressible files
fn create_archive_with_large_files(file_count: usize, file_size_mb: usize) -> NamedTempFile {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    for i in 0..file_count {
        let filename = format!("large{}.bin", i);
        let data = vec![0xAB_u8; file_size_mb * 1024 * 1024];
        writer.add_file(&filename, &data).unwrap();
    }

    writer.finalize().unwrap();
    temp_file
}

#[test]
fn test_100_concurrent_readers() {
    // Create archive with 100 files
    let temp_file = create_archive_with_files(100);
    let path = temp_file.path();

    // Spawn 100 threads, each with its own ArchiveReader
    let handles: Vec<_> = (0..100)
        .map(|thread_id| {
            let path_clone = path.to_path_buf();
            thread::spawn(move || {
                // Each thread opens its own reader (separate file handle)
                let mut reader = ArchiveReader::open_and_init(&path_clone).unwrap();

                for i in 0..100 {
                    let filename = format!("file{}.txt", i);
                    let data = reader.read_file(&filename).unwrap();
                    let expected = format!("data{}", i);
                    assert_eq!(data, expected.as_bytes());
                }

                if thread_id % 20 == 0 {
                    println!("Thread {} completed 100 reads", thread_id);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    println!("✓ 100 threads × 100 files = 10,000 successful reads");
}

#[test]
fn test_concurrent_list_operations() {
    // Create archive with directory structure (50 files total)
    let temp_file = create_archive_with_directories();
    let path = temp_file.path();

    // 20 threads repeatedly listing files
    let handles: Vec<_> = (0..20)
        .map(|thread_id| {
            let path_clone = path.to_path_buf();
            thread::spawn(move || {
                // Each thread has its own reader
                let reader = ArchiveReader::open_and_init(&path_clone).unwrap();

                for iteration in 0..1000 {
                    // List all files
                    let all_files = reader.list_files();
                    assert_eq!(all_files.len(), 50); // 5 dirs × 10 files

                    // List files in each directory
                    for dir_num in 1..=5 {
                        let prefix = format!("dir{}/", dir_num);
                        let dir_files: Vec<_> = all_files
                            .iter()
                            .filter(|f| f.starts_with(&prefix))
                            .collect();
                        assert_eq!(dir_files.len(), 10);
                    }

                    if iteration % 200 == 0 && iteration > 0 {
                        println!(
                            "Thread {} completed {} list iterations",
                            thread_id, iteration
                        );
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    println!("✓ 20 threads × 1000 iterations = 20,000 list operations");
}

#[test]
fn test_concurrent_decompression() {
    // Create archive with 10 large files (10MB each)
    let temp_file = create_archive_with_large_files(10, 10);
    let path = temp_file.path();

    // 10 threads, each decompressing a different large file simultaneously
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let path_clone = path.to_path_buf();
            thread::spawn(move || {
                // Each thread has its own reader
                let mut reader = ArchiveReader::open_and_init(&path_clone).unwrap();

                let filename = format!("large{}.bin", i);
                let data = reader.read_file(&filename).unwrap();

                assert_eq!(data.len(), 10 * 1024 * 1024); // 10MB
                assert_eq!(data[0], 0xAB);
                assert_eq!(data[data.len() - 1], 0xAB);

                println!("Thread {} decompressed {} (10MB)", i, filename);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    println!("✓ 10 threads decompressed 10MB files simultaneously (100MB total)");
}

#[test]
fn test_concurrent_random_access() {
    // Create archive with 1000 small files
    let temp_file = create_archive_with_files(1000);
    let path = temp_file.path();

    // 50 threads doing random reads
    let handles: Vec<_> = (0..50)
        .map(|thread_id| {
            let path_clone = path.to_path_buf();
            thread::spawn(move || {
                // Each thread has its own reader
                let mut reader = ArchiveReader::open_and_init(&path_clone).unwrap();

                // Use thread_id as seed for deterministic "random" access
                for i in 0..100 {
                    let file_idx = (thread_id * 17 + i * 23) % 1000; // Simple pseudo-random
                    let filename = format!("file{}.txt", file_idx);
                    let data = reader.read_file(&filename).unwrap();
                    let expected = format!("data{}", file_idx);
                    assert_eq!(data, expected.as_bytes());
                }

                if thread_id % 10 == 0 {
                    println!("Thread {} completed 100 random reads", thread_id);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    println!("✓ 50 threads × 100 random reads = 5,000 random access operations");
}

#[test]
fn test_concurrent_contains_checks() {
    // Create archive with known file set
    let temp_file = create_archive_with_files(500);
    let path = temp_file.path();

    // 30 threads checking if files exist
    let handles: Vec<_> = (0..30)
        .map(|thread_id| {
            let path_clone = path.to_path_buf();
            thread::spawn(move || {
                // Each thread has its own reader
                let reader = ArchiveReader::open_and_init(&path_clone).unwrap();

                for i in 0..500 {
                    let filename = format!("file{}.txt", i);
                    assert!(reader.contains(&filename));
                }

                // Check non-existent files
                for i in 500..600 {
                    let filename = format!("file{}.txt", i);
                    assert!(!reader.contains(&filename));
                }

                println!("Thread {} completed 600 contains() checks", thread_id);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    println!("✓ 30 threads × 600 checks = 18,000 contains() operations");
}

#[test]
fn test_reader_drop_and_recreate() {
    // Test that readers can be safely dropped and recreated
    let temp_file = create_archive_with_files(100);
    let path = temp_file.path();

    // 20 threads, each creating and dropping readers repeatedly
    let handles: Vec<_> = (0..20)
        .map(|thread_id| {
            let path_clone = path.to_path_buf();
            thread::spawn(move || {
                for _iteration in 0..50 {
                    // Create new reader
                    let mut reader = ArchiveReader::open_and_init(&path_clone).unwrap();

                    // Read a few files
                    for i in 0..5 {
                        let filename = format!("file{}.txt", i);
                        let data = reader.read_file(&filename).unwrap();
                        assert_eq!(data, format!("data{}", i).as_bytes());
                    }

                    // Drop reader
                    drop(reader);
                }

                println!(
                    "Thread {} created/dropped reader 50 times",
                    thread_id
                );
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    println!("✓ 20 threads × 50 create/drop cycles = 1,000 reader lifecycles");
}
