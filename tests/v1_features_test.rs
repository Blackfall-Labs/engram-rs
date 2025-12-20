use engram_rs::{ArchiveReader, ArchiveWriter};
use tempfile::NamedTempFile;

// v1.0 format constants
const END_RECORD_SIZE: usize = 64;
const MIN_FRAME_COMPRESSION_SIZE: usize = 52_428_800; // 50MB

#[test]
fn test_local_entry_headers() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    let test_data = b"Test data for LOCA validation";

    // Write archive with v1.0 format
    {
        let mut writer = ArchiveWriter::create(archive_path).unwrap();
        writer.add_file("test.txt", test_data).unwrap();
        writer.finalize().unwrap();
    }

    // Read and verify LOCA headers are present and validated
    {
        let mut reader = ArchiveReader::open(archive_path).unwrap();
        reader.initialize().unwrap();

        // Should successfully read file (validates LOCA internally)
        let data = reader.read_file("test.txt").unwrap();
        assert_eq!(data, test_data);
    }
}

#[test]
fn test_end_record_validation() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    // Create archive
    {
        let mut writer = ArchiveWriter::create(archive_path).unwrap();
        writer.add_file("file1.txt", b"Content 1").unwrap();
        writer.add_file("file2.txt", b"Content 2").unwrap();
        writer.finalize().unwrap();
    }

    // Verify ENDR is present (last 64 bytes)
    {
        let file_data = std::fs::read(archive_path).unwrap();
        let endr_signature = &file_data[file_data.len() - END_RECORD_SIZE..file_data.len() - END_RECORD_SIZE + 4];

        // ENDR signature: 0x454E4452 ("ENDR")
        assert_eq!(endr_signature, &[0x45, 0x4E, 0x44, 0x52]);
    }

    // Read should validate ENDR automatically
    {
        let mut reader = ArchiveReader::open(archive_path).unwrap();
        reader.initialize().unwrap(); // Will fail if ENDR invalid
        assert_eq!(reader.entry_count(), 2);
    }
}

#[test]
fn test_corrupted_end_record() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    // Create valid archive
    {
        let mut writer = ArchiveWriter::create(archive_path).unwrap();
        writer.add_file("test.txt", b"Test").unwrap();
        writer.finalize().unwrap();
    }

    // Corrupt the ENDR signature
    {
        let mut file_data = std::fs::read(archive_path).unwrap();
        let endr_start = file_data.len() - END_RECORD_SIZE;
        file_data[endr_start] = 0xFF; // Corrupt first byte of ENDR signature
        std::fs::write(archive_path, file_data).unwrap();
    }

    // Reading should fail due to invalid ENDR
    {
        let mut reader = ArchiveReader::open(archive_path).unwrap();
        let result = reader.initialize();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Invalid end record signature") || err_msg.contains("ENDR"));
    }
}

#[test]
fn test_frame_compression_large_file() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    // Create 60MB file (above 50MB threshold for frame compression)
    let size = 60 * 1024 * 1024;
    let large_data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

    // Write with automatic frame compression
    {
        let mut writer = ArchiveWriter::create(archive_path).unwrap();
        writer.add_file("large.bin", &large_data).unwrap();
        writer.finalize().unwrap();
    }

    // Read and verify
    {
        let mut reader = ArchiveReader::open(archive_path).unwrap();
        reader.initialize().unwrap();

        let data = reader.read_file("large.bin").unwrap();
        assert_eq!(data.len(), size);
        assert_eq!(data, large_data);
    }

    // Verify file is actually compressed (should be much smaller than 60MB)
    let archive_size = std::fs::metadata(archive_path).unwrap().len();
    assert!(archive_size < (size as u64 / 2), "Archive should be compressed (size: {}, original: {})", archive_size, size);
}

#[test]
fn test_frame_compression_threshold() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    // Create file just below threshold (40MB - should use regular compression)
    let small_size = 40 * 1024 * 1024;
    let small_data: Vec<u8> = vec![42u8; small_size];

    // Create file above threshold (60MB - should use frame compression)
    let large_size = 60 * 1024 * 1024;
    let large_data: Vec<u8> = vec![42u8; large_size];

    {
        let mut writer = ArchiveWriter::create(archive_path).unwrap();
        writer.add_file("small.bin", &small_data).unwrap();
        writer.add_file("large.bin", &large_data).unwrap();
        writer.finalize().unwrap();
    }

    // Both should decompress correctly
    {
        let mut reader = ArchiveReader::open(archive_path).unwrap();
        reader.initialize().unwrap();

        let small = reader.read_file("small.bin").unwrap();
        assert_eq!(small.len(), small_size);

        let large = reader.read_file("large.bin").unwrap();
        assert_eq!(large.len(), large_size);
    }
}

#[test]
fn test_v1_format_version() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    {
        let mut writer = ArchiveWriter::create(archive_path).unwrap();
        writer.add_file("test.txt", b"v1.0 test").unwrap();
        writer.finalize().unwrap();
    }

    {
        let reader = ArchiveReader::open(archive_path).unwrap();
        let header = reader.header();

        // Verify version is 1.0
        assert_eq!(header.version_major, 1);
        assert_eq!(header.version_minor, 0);
    }
}

#[test]
fn test_all_v1_features_combined() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    // Create archive with multiple files of varying sizes
    {
        let mut writer = ArchiveWriter::create(archive_path).unwrap();

        // Small file (no compression)
        writer.add_file("small.txt", b"Hi").unwrap();

        // Medium file (regular compression)
        let medium_data = vec![42u8; 10 * 1024 * 1024]; // 10MB
        writer.add_file("medium.bin", &medium_data).unwrap();

        // Large file (frame compression)
        let large_data: Vec<u8> = (0..(MIN_FRAME_COMPRESSION_SIZE + 1000000))
            .map(|i| (i % 256) as u8)
            .collect();
        writer.add_file("large.bin", &large_data).unwrap();

        writer.finalize().unwrap();
    }

    // Verify archive structure
    {
        let file_data = std::fs::read(archive_path).unwrap();

        // Check magic number
        assert_eq!(&file_data[0..4], &[0x89, b'E', b'N', b'G']);

        // Check ENDR signature at end
        let endr_sig = &file_data[file_data.len() - END_RECORD_SIZE..file_data.len() - END_RECORD_SIZE + 4];
        assert_eq!(endr_sig, &[0x45, 0x4E, 0x44, 0x52]);
    }

    // Read and verify all files
    {
        let mut reader = ArchiveReader::open(archive_path).unwrap();
        reader.initialize().unwrap();

        assert_eq!(reader.entry_count(), 3);
        assert_eq!(reader.header().version_major, 1);
        assert_eq!(reader.header().version_minor, 0);

        let small = reader.read_file("small.txt").unwrap();
        assert_eq!(small, b"Hi");

        let medium = reader.read_file("medium.bin").unwrap();
        assert_eq!(medium.len(), 10 * 1024 * 1024);

        let large = reader.read_file("large.bin").unwrap();
        assert_eq!(large.len(), MIN_FRAME_COMPRESSION_SIZE + 1000000);
    }
}
