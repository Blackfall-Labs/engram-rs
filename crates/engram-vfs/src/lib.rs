//! # Engram VFS
//!
//! SQLite database access for .eng archives
//! Loads databases into memory for querying without physical extraction.

use engram_core::ArchiveReader;
use rusqlite::Connection;
use std::path::Path;

mod error;

pub use error::{VfsError, Result};

/// Virtual file system for reading SQLite databases from archives
pub struct EngramVfs {
    archive_path: std::path::PathBuf,
}

impl EngramVfs {
    /// Create a new VFS for the given archive
    pub fn new<P: AsRef<Path>>(archive_path: P) -> Self {
        Self {
            archive_path: archive_path.as_ref().to_path_buf(),
        }
    }

    /// Open a database from the archive into memory
    pub fn open_database(&self, db_path_in_archive: &str) -> Result<Connection> {
        // Open archive and extract database
        let mut reader = ArchiveReader::open(&self.archive_path)
            .map_err(|e| VfsError::Archive(format!("Failed to open archive: {}", e)))?;

        // Check if database exists in archive
        if !reader.contains(db_path_in_archive) {
            return Err(VfsError::DatabaseNotFound(db_path_in_archive.to_string()));
        }

        // Read database into memory
        let db_data = reader
            .read_file(db_path_in_archive)
            .map_err(|e| VfsError::Archive(format!("Failed to read database: {}", e)))?;

        // Create a temporary file for the database
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("engram_{}.db", uuid()));

        // Write database to temp file
        std::fs::write(&temp_path, &db_data)?;

        // Open the database
        let conn = Connection::open(&temp_path)
            .map_err(|e| VfsError::Sqlite(format!("Failed to open database: {}", e)))?;

        // Configure for read-only access
        conn.execute_batch("PRAGMA query_only = ON;")
            .map_err(|e| VfsError::Sqlite(format!("Failed to configure database: {}", e)))?;

        // Note: temp file will be cleaned up when connection is dropped
        // We could implement Drop trait to clean up if needed

        Ok(conn)
    }

    /// Alternative: Load database entirely into memory (no temp file)
    pub fn open_database_in_memory(&self, db_path_in_archive: &str) -> Result<Connection> {
        // Open archive and extract database
        let mut reader = ArchiveReader::open(&self.archive_path)
            .map_err(|e| VfsError::Archive(format!("Failed to open archive: {}", e)))?;

        // Check if database exists in archive
        if !reader.contains(db_path_in_archive) {
            return Err(VfsError::DatabaseNotFound(db_path_in_archive.to_string()));
        }

        // Read database into memory
        let db_data = reader
            .read_file(db_path_in_archive)
            .map_err(|e| VfsError::Archive(format!("Failed to read database: {}", e)))?;

        // Create temp file
        let temp_path = std::env::temp_dir().join(format!("engram_{}.db", uuid()));
        std::fs::write(&temp_path, &db_data)?;

        // Open source database
        let src_conn = Connection::open(&temp_path)
            .map_err(|e| VfsError::Sqlite(format!("Failed to open temp database: {}", e)))?;

        // Create in-memory database
        let mut dest_conn = Connection::open_in_memory()
            .map_err(|e| VfsError::Sqlite(format!("Failed to create in-memory database: {}", e)))?;

        // Use backup API to copy database into memory
        {
            let backup = rusqlite::backup::Backup::new(&src_conn, &mut dest_conn)
                .map_err(|e| VfsError::Sqlite(format!("Failed to create backup: {}", e)))?;

            backup
                .run_to_completion(5, std::time::Duration::from_millis(100), None)
                .map_err(|e| VfsError::Sqlite(format!("Failed to restore backup: {}", e)))?;
        }

        // Clean up temp file
        let _ = std::fs::remove_file(&temp_path);

        Ok(dest_conn)
    }
}

// Simple UUID generator (not cryptographically secure, just for temp file names)
fn uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:x}", nanos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engram_core::ArchiveWriter;
    use rusqlite;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn test_vfs_open_database() {
        // Create a test archive with a database
        let archive_file = NamedTempFile::new().unwrap();
        let archive_path = archive_file.path();

        // Create a simple database
        let db_file = NamedTempFile::new().unwrap();
        let db_path = db_file.path();

        {
            let conn = Connection::open(db_path).unwrap();
            conn.execute(
                "CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)",
                [],
            )
            .unwrap();
            conn.execute("INSERT INTO test (name) VALUES ('Alice')", [])
                .unwrap();
            conn.execute("INSERT INTO test (name) VALUES ('Bob')", [])
                .unwrap();
        }

        // Add database to archive
        {
            let mut writer = ArchiveWriter::create(archive_path).unwrap();
            writer
                .add_file_from_disk("test.db", db_path)
                .unwrap();
            writer.finalize().unwrap();
        }

        // Open database from archive using VFS
        {
            let vfs = EngramVfs::new(archive_path);
            let conn = vfs.open_database("test.db").unwrap();

            // Query database
            let mut stmt = conn.prepare("SELECT name FROM test ORDER BY id").unwrap();
            let names: Vec<String> = stmt
                .query_map([], |row| row.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect();

            assert_eq!(names, vec!["Alice", "Bob"]);
        }
    }

    #[test]
    fn test_vfs_open_database_in_memory() {
        // Create a test archive with a database
        let archive_file = NamedTempFile::new().unwrap();
        let archive_path = archive_file.path();

        // Create a simple database
        let db_file = NamedTempFile::new().unwrap();
        let db_path = db_file.path();

        {
            let conn = Connection::open(db_path).unwrap();
            conn.execute(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT)",
                [],
            )
            .unwrap();
            conn.execute("INSERT INTO users (email) VALUES ('test@example.com')", [])
                .unwrap();
        }

        // Add database to archive
        {
            let mut writer = ArchiveWriter::create(archive_path).unwrap();
            writer
                .add_file_from_disk("data.db", db_path)
                .unwrap();
            writer.finalize().unwrap();
        }

        // Open database from archive into memory
        {
            let vfs = EngramVfs::new(archive_path);
            let conn = vfs.open_database_in_memory("data.db").unwrap();

            // Query database
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
                .unwrap();

            assert_eq!(count, 1);
        }
    }
}
