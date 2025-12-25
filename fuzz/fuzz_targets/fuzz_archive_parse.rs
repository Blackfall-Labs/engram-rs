#![no_main]

use libfuzzer_sys::fuzz_target;
use engram_rs::ArchiveReader;
use std::io::Write;
use tempfile::NamedTempFile;

fuzz_target!(|data: &[u8]| {
    // Skip too-small inputs (header is 64 bytes minimum)
    if data.len() < 64 {
        return;
    }

    // Write fuzz data to temporary file
    let mut temp_file = match NamedTempFile::new() {
        Ok(f) => f,
        Err(_) => return,
    };

    if temp_file.write_all(data).is_err() {
        return;
    }

    if temp_file.flush().is_err() {
        return;
    }

    let path = temp_file.path();

    // Try to open archive - should never panic
    let mut reader = match ArchiveReader::open(path) {
        Ok(r) => r,
        Err(_) => return, // Expected for invalid data
    };

    // Try to initialize - should never panic
    if reader.initialize().is_err() {
        return; // Expected for corrupted data
    }

    // Try to list files - should never panic
    let files: Vec<String> = reader.list_files().to_vec();

    // Try to read each file - should never panic
    for file in &files {
        let _ = reader.read_file(file);
    }

    // Try entry_count - should never panic
    let _ = reader.entry_count();

    // Try contains with various paths - should never panic
    let _ = reader.contains("test.txt");
    let _ = reader.contains("");
    let _ = reader.contains("/");
    let _ = reader.contains("../../../etc/passwd");
});
