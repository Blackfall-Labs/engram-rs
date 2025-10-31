//! Integration tests for engram-core

#[cfg(test)]
mod tests {
    use crate::*;
    use std::collections::HashMap;
    use tempfile::NamedTempFile;

    #[test]
    fn test_create_and_read_archive() {
        // Create temporary file
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Create archive
        {
            let mut writer = ArchiveWriter::create(path).unwrap();

            // Add some test files
            writer.add_file("test.txt", b"Hello, World!").unwrap();
            writer
                .add_file("data.json", br#"{"key": "value"}"#)
                .unwrap();
            writer
                .add_file("large.bin", &vec![0u8; 10000])
                .unwrap();

            writer.finalize().unwrap();
        }

        // Read archive
        {
            let mut reader = ArchiveReader::open(path).unwrap();

            assert_eq!(reader.entry_count(), 3);
            assert!(reader.contains("test.txt"));
            assert!(reader.contains("data.json"));
            assert!(reader.contains("large.bin"));

            // Read files
            let txt_data = reader.read_file("test.txt").unwrap();
            assert_eq!(txt_data, b"Hello, World!");

            let json_data = reader.read_file("data.json").unwrap();
            assert_eq!(json_data, br#"{"key": "value"}"#);

            let bin_data = reader.read_file("large.bin").unwrap();
            assert_eq!(bin_data.len(), 10000);
        }
    }

    #[test]
    fn test_compression_methods() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let test_data = b"This is test data that should be compressed well when repeated. \
                         This is test data that should be compressed well when repeated. \
                         This is test data that should be compressed well when repeated.";

        {
            let mut writer = ArchiveWriter::create(path).unwrap();
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
        }

        {
            let mut reader = ArchiveReader::open(path).unwrap();

            // Verify all methods produce correct output
            assert_eq!(reader.read_file("none.txt").unwrap(), test_data);
            assert_eq!(reader.read_file("lz4.txt").unwrap(), test_data);
            assert_eq!(reader.read_file("zstd.txt").unwrap(), test_data);

            // Verify compression actually reduced size
            let none_entry = reader.get_entry("none.txt").unwrap();
            let lz4_entry = reader.get_entry("lz4.txt").unwrap();
            let zstd_entry = reader.get_entry("zstd.txt").unwrap();

            assert_eq!(none_entry.compressed_size, none_entry.uncompressed_size);
            assert!(lz4_entry.compressed_size < lz4_entry.uncompressed_size);
            assert!(zstd_entry.compressed_size < zstd_entry.uncompressed_size);
        }
    }

    #[test]
    fn test_manifest() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let manifest = serde_json::json!({
            "name": "test-archive",
            "version": "1.0.0",
            "description": "Test archive"
        });

        {
            let mut writer = ArchiveWriter::create(path).unwrap();
            writer.add_manifest(&manifest).unwrap();
            writer.add_file("readme.txt", b"Test file").unwrap();
            writer.finalize().unwrap();
        }

        {
            let mut reader = ArchiveReader::open(path).unwrap();
            let read_manifest = reader.read_manifest().unwrap().unwrap();

            assert_eq!(read_manifest["name"], "test-archive");
            assert_eq!(read_manifest["version"], "1.0.0");
        }
    }

    #[test]
    fn test_list_prefix() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        {
            let mut writer = ArchiveWriter::create(path).unwrap();
            writer.add_file("docs/readme.md", b"readme").unwrap();
            writer.add_file("docs/guide.md", b"guide").unwrap();
            writer.add_file("src/main.rs", b"code").unwrap();
            writer.add_file("test.txt", b"test").unwrap();
            writer.finalize().unwrap();
        }

        {
            let reader = ArchiveReader::open(path).unwrap();

            let docs = reader.list_prefix("docs/");
            assert_eq!(docs.len(), 2);

            let src = reader.list_prefix("src/");
            assert_eq!(src.len(), 1);
        }
    }

    #[test]
    fn test_file_not_found() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        {
            let mut writer = ArchiveWriter::create(path).unwrap();
            writer.add_file("exists.txt", b"data").unwrap();
            writer.finalize().unwrap();
        }

        {
            let mut reader = ArchiveReader::open(path).unwrap();
            let result = reader.read_file("nonexistent.txt");
            assert!(matches!(result, Err(EngramError::FileNotFound(_))));
        }
    }
}
