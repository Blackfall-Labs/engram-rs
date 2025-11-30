//! Compatibility layer for old engram-vfs API
//!
//! Provides `EngramVfs` wrapper around new `VfsReader` for backward compatibility.

use crate::error::{EngramError, Result};
use crate::vfs::VfsReader;
use rusqlite::Connection;
use std::path::Path;

/// Compatibility wrapper for the old engram-vfs API
///
/// This provides the same interface as the old `engram-vfs::EngramVfs`
/// while using the new unified `VfsReader` internally.
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

    /// Open a database from the archive
    ///
    /// Extracts the database to a temporary file and returns a read-only connection.
    pub fn open_database(&self, db_path_in_archive: &str) -> Result<Connection> {
        let mut vfs = VfsReader::open(&self.archive_path)?;
        vfs.open_database(db_path_in_archive)
    }

    /// Open a database from the archive into memory
    ///
    /// Loads the entire database into an in-memory SQLite database.
    pub fn open_database_in_memory(&self, db_path_in_archive: &str) -> Result<Connection> {
        // First extract to temp file
        let mut vfs = VfsReader::open(&self.archive_path)?;
        let temp_conn = vfs.open_database(db_path_in_archive)?;

        // Create in-memory database
        let mut mem_conn = Connection::open_in_memory()?;

        // Use backup API to copy database into memory
        {
            let backup = rusqlite::backup::Backup::new(&temp_conn, &mut mem_conn)
                .map_err(|e| EngramError::Internal(format!("Failed to create backup: {}", e)))?;

            backup
                .run_to_completion(5, std::time::Duration::from_millis(100), None)
                .map_err(|e| EngramError::Internal(format!("Failed to restore backup: {}", e)))?;
        }

        // Configure for read-only access
        mem_conn
            .execute_batch("PRAGMA query_only = ON;")
            .map_err(|e| EngramError::Internal(format!("Failed to configure database: {}", e)))?;

        Ok(mem_conn)
    }

    /// List all databases in the archive
    pub fn list_databases(&self) -> Result<Vec<String>> {
        let vfs = VfsReader::open(&self.archive_path)?;
        Ok(vfs.list_databases())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::ArchiveWriter;
    use rusqlite::params;
    use tempfile::NamedTempFile;

    #[test]
    fn test_compat_vfs() -> Result<()> {
        // Create a test database
        let temp_db = NamedTempFile::new()?;
        let db_path = temp_db.path();

        {
            let conn = Connection::open(db_path)?;
            conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)", [])?;
            conn.execute("INSERT INTO test (name) VALUES (?1)", params!["Alice"])?;
        }

        let db_data = std::fs::read(db_path)?;

        // Create archive
        let archive_path = NamedTempFile::new()?.into_temp_path();
        {
            let mut writer = ArchiveWriter::create(&archive_path)?;
            writer.add_file("data.db", &db_data)?;
            writer.finalize()?;
        }

        // Test EngramVfs
        let vfs = EngramVfs::new(&archive_path);

        // Test open_database
        {
            let conn = vfs.open_database("data.db")?;
            let mut stmt = conn.prepare("SELECT name FROM test")?;
            let names: Vec<String> = stmt
                .query_map([], |row| row.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            assert_eq!(names, vec!["Alice"]);
        }

        // Test open_database_in_memory
        {
            let conn = vfs.open_database_in_memory("data.db")?;
            let mut stmt = conn.prepare("SELECT name FROM test")?;
            let names: Vec<String> = stmt
                .query_map([], |row| row.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            assert_eq!(names, vec!["Alice"]);
        }

        // Test list_databases
        {
            let dbs = vfs.list_databases()?;
            assert_eq!(dbs.len(), 1);
            assert_eq!(dbs[0], "data.db");
        }

        Ok(())
    }
}
