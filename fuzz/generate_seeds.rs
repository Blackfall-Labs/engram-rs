//! Generate seed corpus for fuzzing

use engram_rs::ArchiveWriter;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus_dir = "fuzz/corpus/fuzz_archive_parse";
    fs::create_dir_all(corpus_dir)?;

    println!("Generating seed corpus...");

    // Seed 1: Empty archive (no files)
    {
        let path = format!("{}/seed_empty.eng", corpus_dir);
        let writer = ArchiveWriter::create(&path)?;
        writer.finalize()?;
        println!("✓ Generated: {}", path);
    }

    // Seed 2: Single small file
    {
        let path = format!("{}/seed_single_small.eng", corpus_dir);
        let mut writer = ArchiveWriter::create(&path)?;
        writer.add_file("test.txt", b"Hello, World!")?;
        writer.finalize()?;
        println!("✓ Generated: {}", path);
    }

    // Seed 3: Multiple files
    {
        let path = format!("{}/seed_multi.eng", corpus_dir);
        let mut writer = ArchiveWriter::create(&path)?;
        writer.add_file("file1.txt", b"First file")?;
        writer.add_file("file2.txt", b"Second file")?;
        writer.add_file("dir/file3.txt", b"Third file in directory")?;
        writer.finalize()?;
        println!("✓ Generated: {}", path);
    }

    // Seed 4: Large file with compression
    {
        let path = format!("{}/seed_large.eng", corpus_dir);
        let mut writer = ArchiveWriter::create(&path)?;
        let large_data = b"This is test data for compression. ".repeat(1000);
        writer.add_file("large.txt", &large_data)?;
        writer.finalize()?;
        println!("✓ Generated: {}", path);
    }

    // Seed 5: Binary data
    {
        let path = format!("{}/seed_binary.eng", corpus_dir);
        let mut writer = ArchiveWriter::create(&path)?;
        let binary_data: Vec<u8> = (0..255).collect();
        writer.add_file("binary.bin", &binary_data)?;
        writer.finalize()?;
        println!("✓ Generated: {}", path);
    }

    // Seed 6: Empty file (zero bytes)
    {
        let path = format!("{}/seed_zero_length.eng", corpus_dir);
        let mut writer = ArchiveWriter::create(&path)?;
        writer.add_file("empty.txt", b"")?;
        writer.finalize()?;
        println!("✓ Generated: {}", path);
    }

    println!("\nGenerated 6 seed files in {}", corpus_dir);
    Ok(())
}
