/// Basic example demonstrating archive creation and reading
///
/// Run with: cargo run --example basic
use engram_rs::{ArchiveReader, ArchiveWriter, CompressionMethod};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Engram-rs Basic Example ===\n");

    // Create an archive
    println!("1. Creating archive...");
    create_archive()?;

    // Read from the archive
    println!("\n2. Reading from archive...");
    read_archive()?;

    println!("\n✓ Example complete!");
    Ok(())
}

fn create_archive() -> Result<(), Box<dyn Error>> {
    let mut writer = ArchiveWriter::create("example_basic.eng")?;

    // Add some files with different content
    writer.add_file(
        "readme.txt",
        b"This is a readme file for the basic example.",
    )?;
    writer.add_file(
        "data.json",
        br#"{"name": "Basic Example", "version": "1.0.0"}"#,
    )?;
    writer.add_file("notes.md", b"# Notes\n\nThis is a markdown file.")?;

    // You can also specify compression explicitly
    writer.add_file_with_compression(
        "binary.dat",
        &[0u8; 1000], // 1KB of zeros
        CompressionMethod::Lz4,
    )?;

    writer.finalize()?;
    println!("   ✓ Archive created: example_basic.eng");

    Ok(())
}

fn read_archive() -> Result<(), Box<dyn Error>> {
    let mut reader = ArchiveReader::open("example_basic.eng")?;
    reader.initialize()?;

    // List all files
    println!("   Files in archive:");
    for filename in reader.list_files() {
        println!("     - {}", filename);
    }

    // Read specific file
    println!("\n   Reading readme.txt:");
    let readme = reader.read_file("readme.txt")?;
    println!("     {}", String::from_utf8_lossy(&readme));

    // Read JSON data
    println!("\n   Reading data.json:");
    let json_data = reader.read_file("data.json")?;
    let json: serde_json::Value = serde_json::from_slice(&json_data)?;
    println!("     Name: {}", json["name"]);
    println!("     Version: {}", json["version"]);

    Ok(())
}
