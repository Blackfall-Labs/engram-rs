//! Integration tests for engram-rs library

use engram_rs::{ArchiveReader, ArchiveWriter, Author, CompressionMethod, Manifest, VfsReader};
use rusqlite::{params, Connection};
use tempfile::NamedTempFile;

#[test]
fn test_basic_archive_roundtrip() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    // Create archive
    {
        let mut writer = ArchiveWriter::create(archive_path).unwrap();
        writer.add_file("test.txt", b"Hello, World!").unwrap();
        writer
            .add_file("data/nested.txt", b"Nested content")
            .unwrap();
        writer.finalize().unwrap();
    }

    // Read archive
    {
        let mut reader = ArchiveReader::open(archive_path).unwrap();
        reader.initialize().unwrap();

        // Check file count
        assert_eq!(reader.entry_count(), 2);

        // Check files exist
        assert!(reader.contains("test.txt"));
        assert!(reader.contains("data/nested.txt"));

        // Read files
        let content1 = reader.read_file("test.txt").unwrap();
        assert_eq!(content1, b"Hello, World!");

        let content2 = reader.read_file("data/nested.txt").unwrap();
        assert_eq!(content2, b"Nested content");
    }
}

#[test]
fn test_compression_methods() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    let test_data = b"This is test data that should compress well. ".repeat(100);

    // Create archive with different compression methods
    {
        let mut writer = ArchiveWriter::create(archive_path).unwrap();
        writer
            .add_file_with_compression("uncompressed.txt", &test_data, CompressionMethod::None)
            .unwrap();
        writer
            .add_file_with_compression("lz4.txt", &test_data, CompressionMethod::Lz4)
            .unwrap();
        writer
            .add_file_with_compression("zstd.txt", &test_data, CompressionMethod::Zstd)
            .unwrap();
        writer.finalize().unwrap();
    }

    // Verify all files decompress correctly
    {
        let mut reader = ArchiveReader::open(archive_path).unwrap();
        reader.initialize().unwrap();

        let uncompressed = reader.read_file("uncompressed.txt").unwrap();
        assert_eq!(uncompressed, test_data);

        let lz4 = reader.read_file("lz4.txt").unwrap();
        assert_eq!(lz4, test_data);

        let zstd = reader.read_file("zstd.txt").unwrap();
        assert_eq!(zstd, test_data);
    }
}

#[test]
fn test_manifest_integration() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    // Create manifest
    let mut manifest = Manifest::new(
        "test-engram".to_string(),
        "Test Engram".to_string(),
        Author {
            name: "Test Author".to_string(),
            email: Some("test@example.com".to_string()),
            url: None,
        },
        "1.0.0".to_string(),
    );

    let test_data = b"Test file content";
    manifest.add_file(
        "test.txt".to_string(),
        test_data,
        Some("text/plain".to_string()),
    );

    // Create archive with manifest
    {
        let mut writer = ArchiveWriter::create(archive_path).unwrap();
        writer
            .add_manifest(&serde_json::to_value(&manifest).unwrap())
            .unwrap();
        writer.add_file("test.txt", test_data).unwrap();
        writer.finalize().unwrap();
    }

    // Read and verify manifest
    {
        let mut reader = ArchiveReader::open(archive_path).unwrap();
        reader.initialize().unwrap();
        let manifest_value = reader.read_manifest().unwrap();
        assert!(manifest_value.is_some());

        let parsed =
            Manifest::from_json(&serde_json::to_vec(&manifest_value.unwrap()).unwrap()).unwrap();
        assert_eq!(parsed.id, "test-engram");
        assert_eq!(parsed.files.len(), 1);
        assert_eq!(parsed.files[0].path, "test.txt");
    }
}

#[test]
fn test_vfs_database_access() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    // Create a test database
    let temp_db = NamedTempFile::new().unwrap();
    let db_path = temp_db.path();

    {
        let conn = Connection::open(db_path).unwrap();
        conn.execute(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO users (name, email) VALUES (?1, ?2)",
            params!["Alice", "alice@example.com"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO users (name, email) VALUES (?1, ?2)",
            params!["Bob", "bob@example.com"],
        )
        .unwrap();
    }

    let db_data = std::fs::read(db_path).unwrap();

    // Create archive with database
    {
        let mut writer = ArchiveWriter::create(archive_path).unwrap();
        writer.add_file("users.db", &db_data).unwrap();
        writer.finalize().unwrap();
    }

    // Access database via VFS
    {
        let mut vfs = VfsReader::open(archive_path).unwrap();

        // List databases
        let dbs = vfs.list_databases();
        assert_eq!(dbs.len(), 1);
        assert_eq!(dbs[0], "users.db");

        // Open database
        let conn = vfs.open_database("users.db").unwrap();

        // Query database
        let mut stmt = conn
            .prepare("SELECT name, email FROM users ORDER BY id")
            .unwrap();
        let users: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(users.len(), 2);
        assert_eq!(users[0].0, "Alice");
        assert_eq!(users[1].0, "Bob");
    }
}

#[test]
fn test_large_files() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    // Create a 1MB file
    let large_data = vec![0xAB; 1024 * 1024];

    {
        let mut writer = ArchiveWriter::create(archive_path).unwrap();
        writer.add_file("large.bin", &large_data).unwrap();
        writer.finalize().unwrap();
    }

    {
        let mut reader = ArchiveReader::open(archive_path).unwrap();
        reader.initialize().unwrap();
        let read_data = reader.read_file("large.bin").unwrap();
        assert_eq!(read_data.len(), large_data.len());
        assert_eq!(read_data, large_data);
    }
}

#[test]
fn test_many_files() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    let file_count = 100;

    {
        let mut writer = ArchiveWriter::create(archive_path).unwrap();
        for i in 0..file_count {
            let filename = format!("file_{:03}.txt", i);
            let content = format!("Content of file {}", i);
            writer.add_file(&filename, content.as_bytes()).unwrap();
        }
        writer.finalize().unwrap();
    }

    {
        let mut reader = ArchiveReader::open(archive_path).unwrap();
        reader.initialize().unwrap();
        assert_eq!(reader.entry_count(), file_count);

        for i in 0..file_count {
            let filename = format!("file_{:03}.txt", i);
            let expected = format!("Content of file {}", i);
            let content = reader.read_file(&filename).unwrap();
            assert_eq!(content, expected.as_bytes());
        }
    }
}

#[test]
fn test_prefix_listing() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    {
        let mut writer = ArchiveWriter::create(archive_path).unwrap();
        writer.add_file("docs/readme.md", b"README").unwrap();
        writer.add_file("docs/guide.md", b"GUIDE").unwrap();
        writer.add_file("src/main.rs", b"CODE").unwrap();
        writer.add_file("tests/test.rs", b"TEST").unwrap();
        writer.finalize().unwrap();
    }

    {
        let mut reader = ArchiveReader::open(archive_path).unwrap();
        reader.initialize().unwrap();

        let docs = reader.list_prefix("docs/");
        assert_eq!(docs.len(), 2);

        let src = reader.list_prefix("src/");
        assert_eq!(src.len(), 1);

        let tests = reader.list_prefix("tests/");
        assert_eq!(tests.len(), 1);
    }
}

#[test]
fn test_file_not_found() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    {
        let writer = ArchiveWriter::create(archive_path).unwrap();
        writer.finalize().unwrap();
    }

    {
        let mut reader = ArchiveReader::open(archive_path).unwrap();
        reader.initialize().unwrap();
        let result = reader.read_file("nonexistent.txt");
        assert!(result.is_err());
    }
}

#[test]
fn test_empty_archive() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    {
        let writer = ArchiveWriter::create(archive_path).unwrap();
        writer.finalize().unwrap();
    }

    {
        let mut reader = ArchiveReader::open(archive_path).unwrap();
        reader.initialize().unwrap();
        assert_eq!(reader.entry_count(), 0);
        assert_eq!(reader.list_files().len(), 0);
    }
}

#[test]
fn test_archive_encryption() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path();

    // Test data
    let test_content = b"Sensitive data that should be encrypted";
    let encryption_key: [u8; 32] = [42; 32]; // Simple test key

    // Create encrypted archive
    {
        let mut writer = ArchiveWriter::create(archive_path).unwrap();
        writer = writer.with_archive_encryption(&encryption_key);
        writer.add_file("secret.txt", test_content).unwrap();
        writer.finalize().unwrap();
    }

    // Verify file is encrypted (plaintext not visible in raw bytes)
    {
        let raw_bytes = std::fs::read(archive_path).unwrap();
        let contains_plaintext = raw_bytes
            .windows(test_content.len())
            .any(|window| window == test_content);
        assert!(
            !contains_plaintext,
            "Archive should be encrypted (plaintext should not be visible)"
        );
    }

    // Read encrypted archive with correct key
    {
        let mut reader = ArchiveReader::open(archive_path).unwrap();
        reader = reader.with_decryption_key(&encryption_key);
        reader.initialize().unwrap();

        assert_eq!(reader.entry_count(), 1);
        assert!(reader.contains("secret.txt"));

        let decrypted = reader.read_file("secret.txt").unwrap();
        assert_eq!(decrypted, test_content);
    }

    // Try to read without decryption key (should fail)
    {
        let mut reader = ArchiveReader::open(archive_path).unwrap();
        let result = reader.initialize();
        assert!(
            result.is_err(),
            "Should fail to initialize without decryption key"
        );
    }
}
