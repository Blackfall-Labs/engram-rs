/// Example demonstrating VFS (Virtual File System) for SQLite databases
///
/// Run with: cargo run --example vfs
use engram_rs::{ArchiveWriter, VfsReader};
use rusqlite::Connection;
use std::error::Error;
use tempfile::NamedTempFile;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Engram-rs VFS Example ===\n");

    // Create a temporary SQLite database
    println!("1. Creating sample SQLite database...");
    let db_file = create_sample_database()?;
    let db_path = db_file.path();

    // Create archive with the database
    println!("\n2. Creating archive with database...");
    create_archive_with_db(db_path)?;

    // Query the embedded database
    println!("\n3. Querying embedded database...");
    query_embedded_database()?;

    println!("\n✓ Example complete!");
    Ok(())
}

fn create_sample_database() -> Result<NamedTempFile, Box<dyn Error>> {
    let db_file = NamedTempFile::new()?;
    let conn = Connection::open(db_file.path())?;

    // Create table
    conn.execute(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            active BOOLEAN NOT NULL
        )",
        [],
    )?;

    // Insert sample data
    conn.execute(
        "INSERT INTO users (name, email, active) VALUES (?1, ?2, ?3)",
        ["Alice", "alice@example.com", "1"],
    )?;
    conn.execute(
        "INSERT INTO users (name, email, active) VALUES (?1, ?2, ?3)",
        ["Bob", "bob@example.com", "1"],
    )?;
    conn.execute(
        "INSERT INTO users (name, email, active) VALUES (?1, ?2, ?3)",
        ["Charlie", "charlie@example.com", "0"],
    )?;

    println!("   ✓ Database created with 3 users");

    Ok(db_file)
}

fn create_archive_with_db(db_path: &std::path::Path) -> Result<(), Box<dyn Error>> {
    let mut writer = ArchiveWriter::create("example_vfs.eng")?;

    // Add the database file
    writer.add_file_from_disk("users.db", db_path)?;

    // Add some metadata
    writer.add_file("readme.txt", b"This archive contains a SQLite database.")?;

    writer.finalize()?;
    println!("   ✓ Archive created: example_vfs.eng");

    Ok(())
}

fn query_embedded_database() -> Result<(), Box<dyn Error>> {
    // Open archive with VFS
    let mut vfs = VfsReader::open("example_vfs.eng")?;

    // Open the embedded database
    let conn = vfs.open_database("users.db")?;

    println!("   Querying users table:");

    // Query active users
    let mut stmt = conn.prepare("SELECT id, name, email FROM users WHERE active = 1")?;
    let users = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i32>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    println!("\n   Active users:");
    for user in users {
        let (id, name, email) = user?;
        println!("     {} - {} <{}>", id, name, email);
    }

    // Count total users
    let count: i32 = conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;

    println!("\n   Total users in database: {}", count);

    Ok(())
}
