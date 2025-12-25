//! Phase 2.4: Frame Compression Edge Cases
//!
//! Tests for frame-based compression boundaries and edge cases.
//! Based on TESTING_PLAN.md Phase 2.4
//!
//! Frame compression is used for files ≥ 50MB (52,428,800 bytes).
//! Frame size is 64KB (65,536 bytes).

use engram_rs::{ArchiveReader, ArchiveWriter};
use tempfile::NamedTempFile;

const LARGE_FILE_THRESHOLD: usize = 50 * 1024 * 1024; // 50 MB

/// Helper: Create archive with a file of specified size
fn create_archive_with_sized_file(filename: &str, size_bytes: usize) -> NamedTempFile {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Create data with repeating pattern (highly compressible)
    let data = vec![0xCD_u8; size_bytes];
    writer.add_file(filename, &data).unwrap();
    writer.finalize().unwrap();

    temp_file
}

#[test]
fn test_frame_exactly_at_threshold() {
    // Exactly 50MB (threshold boundary)
    let size = LARGE_FILE_THRESHOLD;
    let temp_file = create_archive_with_sized_file("threshold.bin", size);
    let path = temp_file.path();

    // Should be readable
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let data = reader.read_file("threshold.bin").unwrap();

    assert_eq!(data.len(), size);
    assert_eq!(data[0], 0xCD);
    assert_eq!(data[size - 1], 0xCD);

    println!(
        "✓ File exactly at 50MB threshold: {} bytes",
        data.len()
    );
}

#[test]
fn test_frame_just_below_threshold() {
    // 49.9MB - should NOT use frame compression
    let size = LARGE_FILE_THRESHOLD - 100_000; // 50MB - 100KB
    let temp_file = create_archive_with_sized_file("below.bin", size);
    let path = temp_file.path();

    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let data = reader.read_file("below.bin").unwrap();

    assert_eq!(data.len(), size);
    assert_eq!(data[0], 0xCD);
    assert_eq!(data[size - 1], 0xCD);

    println!(
        "✓ File just below threshold (no frames): {} bytes",
        data.len()
    );
}

#[test]
fn test_frame_just_above_threshold() {
    // 50.1MB - should use frame compression
    let size = LARGE_FILE_THRESHOLD + 100_000; // 50MB + 100KB
    let temp_file = create_archive_with_sized_file("above.bin", size);
    let path = temp_file.path();

    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let data = reader.read_file("above.bin").unwrap();

    assert_eq!(data.len(), size);
    assert_eq!(data[0], 0xCD);
    assert_eq!(data[size - 1], 0xCD);

    println!(
        "✓ File just above threshold (frames used): {} bytes",
        data.len()
    );
}

#[test]
fn test_frame_odd_sizes() {
    // Test various non-aligned sizes
    let sizes = [
        51 * 1024 * 1024,  // 51MB
        63 * 1024 * 1024,  // 63MB
        77 * 1024 * 1024,  // 77MB
        99 * 1024 * 1024,  // 99MB
        128 * 1024 * 1024, // 128MB
    ];

    for (idx, &size) in sizes.iter().enumerate() {
        let filename = format!("file{}.bin", idx);
        let temp_file = create_archive_with_sized_file(&filename, size);
        let path = temp_file.path();

        let mut reader = ArchiveReader::open_and_init(path).unwrap();
        let data = reader.read_file(&filename).unwrap();

        assert_eq!(data.len(), size);
        assert_eq!(data[0], 0xCD);
        assert_eq!(data[size - 1], 0xCD);

        println!("✓ Odd size {} MB: {} bytes", size / 1024 / 1024, data.len());
    }
}

#[test]
fn test_frame_single_frame_plus_bit() {
    // 50MB + 100 bytes - just over one frame boundary
    let size = LARGE_FILE_THRESHOLD + 100;
    let temp_file = create_archive_with_sized_file("single_frame.bin", size);
    let path = temp_file.path();

    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let data = reader.read_file("single_frame.bin").unwrap();

    assert_eq!(data.len(), size);
    assert_eq!(data[0], 0xCD);
    assert_eq!(data[size - 1], 0xCD);

    println!("✓ Single frame + small amount: {} bytes", data.len());
}

#[test]
fn test_frame_very_large_file() {
    // 200MB file - multiple frames
    let size = 200 * 1024 * 1024;
    let temp_file = create_archive_with_sized_file("very_large.bin", size);
    let path = temp_file.path();

    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let data = reader.read_file("very_large.bin").unwrap();

    assert_eq!(data.len(), size);
    assert_eq!(data[0], 0xCD);
    assert_eq!(data[size - 1], 0xCD);

    println!("✓ Very large file (200MB): {} bytes", data.len());
}

#[test]
fn test_frame_mixed_sizes_in_archive() {
    // Archive with multiple files of varying sizes
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Small file (no frames)
    writer
        .add_file("small.bin", &vec![0xAA; 1024 * 1024])
        .unwrap(); // 1MB

    // Just below threshold (no frames)
    writer
        .add_file(
            "below.bin",
            &vec![0xBB; LARGE_FILE_THRESHOLD - 1_000_000],
        )
        .unwrap(); // 49MB

    // Just above threshold (frames)
    writer
        .add_file(
            "above.bin",
            &vec![0xCC; LARGE_FILE_THRESHOLD + 1_000_000],
        )
        .unwrap(); // 51MB

    // Much larger (many frames)
    writer
        .add_file("large.bin", &vec![0xDD; 100 * 1024 * 1024])
        .unwrap(); // 100MB

    writer.finalize().unwrap();

    // Read back all files
    let mut reader = ArchiveReader::open_and_init(path).unwrap();

    let small = reader.read_file("small.bin").unwrap();
    assert_eq!(small.len(), 1024 * 1024);
    assert_eq!(small[0], 0xAA);

    let below = reader.read_file("below.bin").unwrap();
    assert_eq!(below.len(), LARGE_FILE_THRESHOLD - 1_000_000);
    assert_eq!(below[0], 0xBB);

    let above = reader.read_file("above.bin").unwrap();
    assert_eq!(above.len(), LARGE_FILE_THRESHOLD + 1_000_000);
    assert_eq!(above[0], 0xCC);

    let large = reader.read_file("large.bin").unwrap();
    assert_eq!(large.len(), 100 * 1024 * 1024);
    assert_eq!(large[0], 0xDD);

    println!("✓ Mixed file sizes in single archive: all readable");
}

#[test]
fn test_frame_boundary_exact_multiple_of_frame_size() {
    // File size is exact multiple of frame size (64KB)
    let frame_size = 64 * 1024; // 64KB
    let num_frames = 1000;
    let size = LARGE_FILE_THRESHOLD + (frame_size * num_frames);

    let temp_file = create_archive_with_sized_file("exact_multiple.bin", size);
    let path = temp_file.path();

    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let data = reader.read_file("exact_multiple.bin").unwrap();

    assert_eq!(data.len(), size);
    assert_eq!(data[0], 0xCD);
    assert_eq!(data[size - 1], 0xCD);

    println!(
        "✓ File size exact multiple of frame size: {} bytes ({} frames)",
        data.len(),
        num_frames
    );
}

#[test]
fn test_frame_compression_data_integrity() {
    // Verify data integrity for large frame-compressed file with non-uniform data
    let size = 100 * 1024 * 1024; // 100MB
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Create data with pattern (each byte is its position modulo 256)
    let mut data = Vec::with_capacity(size);
    for i in 0..size {
        data.push((i % 256) as u8);
    }
    writer.add_file("pattern.bin", &data).unwrap();
    writer.finalize().unwrap();

    // Read back and verify pattern
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let read_data = reader.read_file("pattern.bin").unwrap();

    assert_eq!(read_data.len(), size);

    // Verify pattern at various points
    for i in (0..size).step_by(1_000_000) {
        assert_eq!(
            read_data[i],
            (i % 256) as u8,
            "Data mismatch at byte {}",
            i
        );
    }

    println!("✓ Frame compression preserves data integrity (100MB pattern file)");
}
