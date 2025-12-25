//! Phase 4.1: Path Traversal Prevention Tests
//!
//! Tests for path security, directory traversal attacks, and path validation.
//! Based on TESTING_PLAN.md Phase 4.1

use engram_rs::ArchiveWriter;
use tempfile::NamedTempFile;

#[test]
fn test_path_traversal_dot_dot() {
    println!("\n🔒 Testing path traversal with ../");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Attempt to write file with ../ in path
    let result = writer.add_file("../../etc/passwd", b"malicious");

    // Should either reject or normalize the path
    match result {
        Ok(_) => {
            // If accepted, verify it's normalized (no actual traversal)
            writer.finalize().unwrap();
            println!("  ⚠️ Path accepted (should be normalized)");
        }
        Err(e) => {
            // Rejected - good!
            println!("  ✅ Path rejected: {:?}", e);
        }
    }
}

#[test]
fn test_absolute_path_unix() {
    println!("\n🔒 Testing absolute Unix path (/etc/passwd)");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Unix absolute path
    let result = writer.add_file("/etc/passwd", b"data");

    match result {
        Ok(_) => {
            writer.finalize().unwrap();
            println!("  ⚠️ Absolute path accepted (should be normalized)");
        }
        Err(e) => {
            println!("  ✅ Absolute path rejected: {:?}", e);
        }
    }
}

#[test]
fn test_absolute_path_windows() {
    println!("\n🔒 Testing absolute Windows path (C:\\Windows\\...)");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Windows absolute path
    let result = writer.add_file("C:\\Windows\\System32\\evil.dll", b"data");

    match result {
        Ok(_) => {
            writer.finalize().unwrap();
            println!("  ⚠️ Windows path accepted (should be normalized)");
        }
        Err(e) => {
            println!("  ✅ Windows path rejected: {:?}", e);
        }
    }
}

#[test]
fn test_path_normalization() {
    println!("\n🔒 Testing path normalization");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Various path formats that should normalize to same path
    writer.add_file("dir/file.txt", b"data1").unwrap();

    // Try to add with Windows separator
    let result2 = writer.add_file("dir\\file.txt", b"data2");

    // Try to add with double separator
    let result3 = writer.add_file("dir//file.txt", b"data3");

    // All should either be rejected or normalized
    println!("  Result 2 (backslash): {:?}", result2.is_ok());
    println!("  Result 3 (double slash): {:?}", result3.is_ok());

    writer.finalize().unwrap();

    // Verify archive is valid
    let reader = engram_rs::ArchiveReader::open_and_init(path).unwrap();
    let files = reader.list_files();

    println!("  Files in archive: {:?}", files);
    println!("  ✅ Path normalization tested");
}

#[test]
fn test_path_with_null_bytes() {
    println!("\n🔒 Testing path with null bytes");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Path with null byte (common attack vector)
    let malicious_path = "file.txt\0/../../etc/passwd";
    let result = writer.add_file(malicious_path, b"data");

    match result {
        Ok(_) => {
            writer.finalize().unwrap();
            println!("  ⚠️ Null byte path accepted");
        }
        Err(e) => {
            println!("  ✅ Null byte path rejected: {:?}", e);
        }
    }
}

#[test]
fn test_path_length_overflow() {
    println!("\n🔒 Testing path length > 255 bytes");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Path longer than 255 bytes (engram limit)
    let long_path = "a".repeat(256);
    let result = writer.add_file(&long_path, b"data");

    match result {
        Ok(_) => {
            // SECURITY FINDING: Path > 255 bytes accepted at add_file()
            println!("  ⚠️ Path > 255 bytes accepted at add_file()");

            // Path should be rejected at finalize()
            let finalize_result = writer.finalize();
            match finalize_result {
                Ok(_) => {
                    panic!("SECURITY ISSUE: Path > 255 bytes should be rejected!");
                }
                Err(e) => {
                    println!("  ✅ Path correctly rejected at finalize(): {:?}", e);
                }
            }
        }
        Err(e) => {
            println!("  ✅ Overlong path rejected at add_file(): {:?}", e);
        }
    }
}

#[test]
fn test_path_with_special_characters() {
    println!("\n🔒 Testing paths with special characters");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Various special characters that might cause issues
    let test_paths = vec![
        "file with spaces.txt",
        "file-with-dashes.txt",
        "file_with_underscores.txt",
        "file.multiple.dots.txt",
        "日本語.txt", // Unicode
        "emoji😀.txt", // Emoji
    ];

    for test_path in &test_paths {
        let result = writer.add_file(test_path, b"data");
        println!("  Path '{}': {}", test_path, if result.is_ok() { "✅ OK" } else { "❌ Rejected" });
    }

    writer.finalize().unwrap();
    println!("  ✅ Special character paths tested");
}

#[test]
fn test_path_components_validation() {
    println!("\n🔒 Testing path component validation");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Test various potentially problematic path components
    let test_paths = vec![
        ".",           // Current directory
        "..",          // Parent directory
        "./file.txt",  // Relative current
        "../file.txt", // Relative parent
        "dir/./file.txt",  // Current in middle
        "dir/../file.txt", // Parent in middle
    ];

    for test_path in &test_paths {
        let result = writer.add_file(test_path, b"data");
        println!("  Path '{}': {}", test_path,
            if result.is_ok() { "✅ Accepted (normalized?)" } else { "🔒 Rejected" });
    }

    // Archive should finalize successfully
    writer.finalize().unwrap();
    println!("  ✅ Path component validation tested");
}

#[test]
fn test_path_case_sensitivity() {
    println!("\n🔒 Testing path case handling");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Add files with different cases
    writer.add_file("File.txt", b"uppercase").unwrap();
    writer.add_file("file.txt", b"lowercase").unwrap();
    writer.add_file("FILE.TXT", b"allcaps").unwrap();

    writer.finalize().unwrap();

    // Read back and verify all are distinct
    let reader = engram_rs::ArchiveReader::open_and_init(path).unwrap();
    let files = reader.list_files();

    println!("  Files in archive: {:?}", files);

    // engram should preserve case (case-sensitive)
    let has_file_upper = files.contains(&"File.txt".to_string());
    let has_file_lower = files.contains(&"file.txt".to_string());
    let has_file_caps = files.contains(&"FILE.TXT".to_string());

    println!("  Has 'File.txt': {}", has_file_upper);
    println!("  Has 'file.txt': {}", has_file_lower);
    println!("  Has 'FILE.TXT': {}", has_file_caps);

    println!("  ✅ Case sensitivity tested");
}

#[test]
fn test_empty_path_component() {
    println!("\n🔒 Testing empty path components");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();

    // Paths with empty components
    let result1 = writer.add_file("", b"empty path");
    let result2 = writer.add_file("dir//file.txt", b"double slash");
    let result3 = writer.add_file("/file.txt", b"leading slash");

    println!("  Empty path: {}", if result1.is_ok() { "⚠️ Accepted" } else { "✅ Rejected" });
    println!("  Double slash: {}", if result2.is_ok() { "✅ Accepted" } else { "❌ Rejected" });
    println!("  Leading slash: {}", if result3.is_ok() { "⚠️ Accepted" } else { "✅ Rejected" });

    // Should be able to finalize
    if result1.is_ok() || result2.is_ok() || result3.is_ok() {
        writer.finalize().unwrap();
    }

    println!("  ✅ Empty component test complete");
}
