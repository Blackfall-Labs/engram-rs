//! Virtual File System support for Engram archives
//!
//! Provides access to SQLite databases embedded within engram archives,
//! allowing SQL queries against archived data.

use crate::archive::ArchiveReader;
use crate::error::{EngramError, Result};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// VFS wrapper for accessing SQLite databases in archives
pub struct VfsReader {
    reader: ArchiveReader,
    temp_dir: Option<TempDir>,
    extracted_dbs: Vec<(String, PathBuf)>,
}

impl VfsReader {
    /// Open an archive for VFS access
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut reader = ArchiveReader::open(path)?;
        reader.initialize()?;
        Ok(Self {
            reader,
            temp_dir: None,
            extracted_dbs: Vec::new(),
        })
    }

    /// List all SQLite database files in the archive
    pub fn list_databases(&self) -> Vec<String> {
        self.reader
            .list_files()
            .iter()
            .filter(|path| {
                path.ends_with(".db") || path.ends_with(".sqlite") || path.ends_with(".sqlite3")
            })
            .cloned()
            .collect()
    }

    /// Open a SQLite connection to a database in the archive
    ///
    /// The database is extracted to a temporary location for access.
    /// The temporary file is cleaned up when the VfsReader is dropped.
    pub fn open_database(&mut self, db_path: &str) -> Result<Connection> {
        // Check if database exists in archive
        if !self.reader.contains(db_path) {
            return Err(EngramError::DatabaseNotFound(db_path.to_string()));
        }

        // Ensure temp directory exists
        if self.temp_dir.is_none() {
            self.temp_dir = Some(tempfile::tempdir()?);
        }

        let temp_dir = self.temp_dir.as_ref().unwrap();

        // Create a safe filename for extraction
        let safe_name = db_path.replace(['/', '\\'], "_");
        let extract_path = temp_dir.path().join(safe_name);

        // Extract database to temp location
        let db_data = self.reader.read_file(db_path)?;
        std::fs::write(&extract_path, db_data)
            .map_err(|e| EngramError::ExtractionFailed(e.to_string()))?;

        // Track extracted database
        self.extracted_dbs
            .push((db_path.to_string(), extract_path.clone()));

        // Open SQLite connection in read-only mode
        let conn = Connection::open_with_flags(&extract_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;

        Ok(conn)
    }

    /// Get the underlying archive reader
    pub fn archive(&self) -> &ArchiveReader {
        &self.reader
    }

    /// Get mutable access to the underlying archive reader
    pub fn archive_mut(&mut self) -> &mut ArchiveReader {
        &mut self.reader
    }

    /// Check if a database is already extracted
    pub fn is_extracted(&self, db_path: &str) -> bool {
        self.extracted_dbs.iter().any(|(path, _)| path == db_path)
    }

    /// Get the path to an extracted database
    pub fn get_extracted_path(&self, db_path: &str) -> Option<&PathBuf> {
        self.extracted_dbs
            .iter()
            .find(|(path, _)| path == db_path)
            .map(|(_, extracted_path)| extracted_path)
    }
}

impl Drop for VfsReader {
    fn drop(&mut self) {
        // TempDir will automatically clean up when dropped
        // No explicit cleanup needed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::ArchiveWriter;
    use rusqlite::params;

    #[test]
    fn test_vfs_database_access() -> Result<()> {
        // Create a test database
        let temp_db = tempfile::NamedTempFile::new()?;
        let db_path = temp_db.path();

        {
            let conn = Connection::open(db_path)?;
            conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)", [])?;
            conn.execute("INSERT INTO test (name) VALUES (?1)", params!["Alice"])?;
            conn.execute("INSERT INTO test (name) VALUES (?1)", params!["Bob"])?;
        }

        // Read database into memory
        let db_data = std::fs::read(db_path)?;

        // Create an archive with the database
        let archive_path = tempfile::NamedTempFile::new()?.into_temp_path();
        {
            let mut writer = ArchiveWriter::create(&archive_path)?;
            writer.add_file("data.db", &db_data)?;
            writer.finalize()?;
        }

        // Open archive with VFS
        let mut vfs = VfsReader::open(&archive_path)?;

        // List databases
        let dbs = vfs.list_databases();
        assert_eq!(dbs.len(), 1);
        assert_eq!(dbs[0], "data.db");

        // Open database connection
        let conn = vfs.open_database("data.db")?;

        // Query the database
        let mut stmt = conn.prepare("SELECT name FROM test ORDER BY id")?;
        let names: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        assert_eq!(names, vec!["Alice", "Bob"]);

        Ok(())
    }

    #[test]
    fn test_database_not_found() {
        let archive_path = tempfile::NamedTempFile::new().unwrap().into_temp_path();
        {
            let writer = ArchiveWriter::create(&archive_path).unwrap();
            writer.finalize().unwrap();
        }

        let mut vfs = VfsReader::open(&archive_path).unwrap();
        let result = vfs.open_database("nonexistent.db");

        assert!(matches!(result, Err(EngramError::DatabaseNotFound(_))));
    }

    #[test]
    fn test_list_databases() -> Result<()> {
        let archive_path = tempfile::NamedTempFile::new()?.into_temp_path();
        {
            let mut writer = ArchiveWriter::create(&archive_path)?;
            writer.add_file("data.db", b"fake db")?;
            writer.add_file("other.sqlite", b"fake db")?;
            writer.add_file("notadb.txt", b"text")?;
            writer.finalize()?;
        }

        let vfs = VfsReader::open(&archive_path)?;
        let dbs = vfs.list_databases();

        assert_eq!(dbs.len(), 2);
        assert!(dbs.contains(&"data.db".to_string()));
        assert!(dbs.contains(&"other.sqlite".to_string()));

        Ok(())
    }
}
