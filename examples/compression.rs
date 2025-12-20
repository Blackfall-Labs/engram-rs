/// Example demonstrating different compression methods
///
/// Run with: cargo run --example compression

use engram_rs::{ArchiveWriter, ArchiveReader, CompressionMethod};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Engram-rs Compression Example ===\n");

    // Create sample data of different sizes
    let small_data = b"Small file content";
    let medium_text = "Lorem ipsum ".repeat(500); // ~6KB
    let large_data = vec![0u8; 100_000]; // 100KB

    println!("1. Creating archives with different compression methods...\n");

    // Test No Compression
    create_archive_no_compression(small_data, &medium_text, &large_data)?;

    // Test LZ4 (fast compression)
    create_archive_lz4(&medium_text, &large_data)?;

    // Test Zstd (best compression ratio)
    create_archive_zstd(&medium_text, &large_data)?;

    // Compare results
    println!("\n2. Comparing compression results:");
    compare_archives()?;

    println!("\n✓ Example complete!");
    Ok(())
}

fn create_archive_no_compression(
    small_data: &[u8],
    medium_text: &str,
    large_data: &[u8],
) -> Result<(), Box<dyn Error>> {
    let mut writer = ArchiveWriter::create("example_none.eng")?;

    writer.add_file_with_compression("small.txt", small_data, CompressionMethod::None)?;
    writer.add_file_with_compression("medium.txt", medium_text.as_bytes(), CompressionMethod::None)?;
    writer.add_file_with_compression("large.bin", large_data, CompressionMethod::None)?;

    writer.finalize()?;
    println!("   ✓ Created: example_none.eng (No compression)");

    Ok(())
}

fn create_archive_lz4(medium_text: &str, large_data: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut writer = ArchiveWriter::create("example_lz4.eng")?;

    writer.add_file_with_compression("medium.txt", medium_text.as_bytes(), CompressionMethod::Lz4)?;
    writer.add_file_with_compression("large.bin", large_data, CompressionMethod::Lz4)?;

    writer.finalize()?;
    println!("   ✓ Created: example_lz4.eng (LZ4 - fast)");

    Ok(())
}

fn create_archive_zstd(medium_text: &str, large_data: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut writer = ArchiveWriter::create("example_zstd.eng")?;

    writer.add_file_with_compression("medium.txt", medium_text.as_bytes(), CompressionMethod::Zstd)?;
    writer.add_file_with_compression("large.bin", large_data, CompressionMethod::Zstd)?;

    writer.finalize()?;
    println!("   ✓ Created: example_zstd.eng (Zstd - best ratio)");

    Ok(())
}

fn compare_archives() -> Result<(), Box<dyn Error>> {
    use std::fs;

    let none_size = fs::metadata("example_none.eng")?.len();
    let lz4_size = fs::metadata("example_lz4.eng")?.len();
    let zstd_size = fs::metadata("example_zstd.eng")?.len();

    println!("\n   Archive sizes:");
    println!("     None:  {:>8} bytes", none_size);
    println!("     LZ4:   {:>8} bytes ({:.1}% of uncompressed)",
        lz4_size, (lz4_size as f64 / none_size as f64) * 100.0);
    println!("     Zstd:  {:>8} bytes ({:.1}% of uncompressed)",
        zstd_size, (zstd_size as f64 / none_size as f64) * 100.0);

    // Verify we can read from compressed archives
    println!("\n   Verifying decompression:");

    let mut reader_lz4 = ArchiveReader::open("example_lz4.eng")?;
    reader_lz4.initialize()?;
    let data_lz4 = reader_lz4.read_file("medium.txt")?;
    println!("     ✓ LZ4 decompressed: {} bytes", data_lz4.len());

    let mut reader_zstd = ArchiveReader::open("example_zstd.eng")?;
    reader_zstd.initialize()?;
    let data_zstd = reader_zstd.read_file("medium.txt")?;
    println!("     ✓ Zstd decompressed: {} bytes", data_zstd.len());

    // Verify content matches
    if data_lz4 == data_zstd {
        println!("     ✓ Decompressed data matches across methods");
    }

    Ok(())
}
