//! Debug test for zero-length file bug

use engram_rs::{ArchiveReader, ArchiveWriter};
use tempfile::NamedTempFile;

#[test]
fn debug_zero_length_file() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    println!("\n=== Writing Archive ===");

    // Create archive with zero-length file
    {
        let mut writer = ArchiveWriter::create(path).unwrap();

        println!("Adding empty.txt (0 bytes)");
        writer.add_file("empty.txt", b"").unwrap();

        println!("Adding regular.txt (13 bytes)");
        writer.add_file("regular.txt", b"Hello, World!").unwrap();

        println!("Finalizing archive");
        writer.finalize().unwrap();
    }

    println!("\n=== Reading Archive ===");

    // Open archive
    let mut reader = ArchiveReader::open(path).unwrap();

    println!("Entry count: {}", reader.entry_count());

    let files = reader.list_files();
    println!("Files in archive ({} total):", files.len());
    for file in files {
        println!("  - {}", file);
    }

    println!("\n=== Trying to Read Files ===");

    // Try to read regular file
    match reader.read_file("regular.txt") {
        Ok(content) => println!("✅ regular.txt: {} bytes", content.len()),
        Err(e) => println!("❌ regular.txt: {:?}", e),
    }

    // Try to read empty file
    match reader.read_file("empty.txt") {
        Ok(content) => println!("✅ empty.txt: {} bytes", content.len()),
        Err(e) => println!("❌ empty.txt: {:?}", e),
    }

    // Check if file exists
    println!("\n=== File Existence Checks ===");
    println!("contains('regular.txt'): {}", reader.contains("regular.txt"));
    println!("contains('empty.txt'): {}", reader.contains("empty.txt"));
}
