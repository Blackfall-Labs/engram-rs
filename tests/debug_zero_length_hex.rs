//! Debug test to inspect actual bytes

use engram_rs::ArchiveWriter;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use tempfile::NamedTempFile;

#[test]
fn inspect_archive_bytes() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    println!("\n=== Creating Archive ===");

    // Create archive
    {
        let mut writer = ArchiveWriter::create(path).unwrap();
        println!("Adding empty.txt (0 bytes)");
        writer.add_file("empty.txt", b"").unwrap();
        println!("Adding regular.txt (13 bytes)");
        writer.add_file("regular.txt", b"Hello, World!").unwrap();
        println!("Finalizing");
        writer.finalize().unwrap();
    }

    println!("\n=== Inspecting File ===");

    let mut file = File::open(path).unwrap();
    let file_size = file.metadata().unwrap().len();
    println!("File size: {} bytes", file_size);

    // Read header (first 64 bytes)
    let mut header_bytes = vec![0u8; 64];
    file.read_exact(&mut header_bytes).unwrap();

    println!("\n=== Header Analysis ===");

    // Magic number (bytes 0-7)
    println!("Magic: {:02X?}", &header_bytes[0..8]);

    // Version (bytes 8-9: version_major u16, bytes 10-11: version_minor u16)
    let version_major = u16::from_le_bytes([header_bytes[8], header_bytes[9]]);
    let version_minor = u16::from_le_bytes([header_bytes[10], header_bytes[11]]);
    println!("Version: {}.{}", version_major, version_minor);

    // Header CRC32 (bytes 12-15, u32 little-endian)
    let header_crc_bytes: [u8; 4] = header_bytes[12..16].try_into().unwrap();
    let header_crc = u32::from_le_bytes(header_crc_bytes);
    println!("Header CRC32: 0x{:08X}", header_crc);

    // Central directory offset (bytes 16-23, u64 little-endian)
    let cd_offset_bytes: [u8; 8] = header_bytes[16..24].try_into().unwrap();
    let cd_offset = u64::from_le_bytes(cd_offset_bytes);
    println!("CD Offset: {} (0x{:X})", cd_offset, cd_offset);

    // Central directory size (bytes 24-31, u64 little-endian)
    let cd_size_bytes: [u8; 8] = header_bytes[24..32].try_into().unwrap();
    let cd_size = u64::from_le_bytes(cd_size_bytes);
    println!("CD Size: {}", cd_size);

    // Entry count (bytes 32-35, u32 little-endian)
    let entry_count_bytes: [u8; 4] = header_bytes[32..36].try_into().unwrap();
    let entry_count = u32::from_le_bytes(entry_count_bytes);
    println!("Entry Count: {}", entry_count);

    // Seek to central directory and inspect
    if cd_offset > 0 && cd_offset < file_size {
        println!("\n=== Central Directory ===");
        file.seek(SeekFrom::Start(cd_offset)).unwrap();

        // Read first 100 bytes of CD
        let mut cd_bytes = vec![0u8; 100.min((file_size - cd_offset) as usize)];
        file.read(&mut cd_bytes).unwrap();

        println!("First {} bytes of CD:", cd_bytes.len());
        for (i, chunk) in cd_bytes.chunks(16).enumerate() {
            print!("{:04X}: ", i * 16);
            for byte in chunk {
                print!("{:02X} ", byte);
            }
            print!("  ");
            for byte in chunk {
                if *byte >= 32 && *byte < 127 {
                    print!("{}", *byte as char);
                } else {
                    print!(".");
                }
            }
            println!();
        }
    }
}
