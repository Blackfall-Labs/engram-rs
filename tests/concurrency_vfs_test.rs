//! Phase 2.1: Concurrent VFS/SQLite Access Tests
//!
//! Tests for multi-threaded VFS access, connection cleanup, and concurrent database queries.
//! Based on TESTING_PLAN.md Phase 2.1

use engram_rs::{ArchiveWriter, VfsReader};
use rusqlite::{params, Connection};
use std::thread;
use tempfile::NamedTempFile;

/// Helper: Create test database with N rows
fn create_test_database(row_count: usize) -> Vec<u8> {
    let temp_db = NamedTempFile::new().unwrap();
    let db_path = temp_db.path();

    let conn = Connection::open(db_path).unwrap();
    conn.execute(
        "CREATE TABLE test (
            id INTEGER PRIMARY KEY,
            value INTEGER NOT NULL
        )",
        [],
    )
    .unwrap();

    for i in 0..row_count {
        conn.execute("INSERT INTO test (value) VALUES (?1)", params![i]).unwrap();
    }

    std::fs::read(db_path).unwrap()
}

/// Helper: Create archive with database
fn create_archive_with_database(db_name: &str, db_data: &[u8]) -> NamedTempFile {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();
    writer.add_file(db_name, db_data).unwrap();
    writer.finalize().unwrap();

    temp_file
}

/// Helper: Create archive with multiple databases
fn create_archive_with_databases(databases: &[(&str, Vec<u8>)]) -> NamedTempFile {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();
    for (db_name, db_data) in databases {
        writer.add_file(db_name, db_data).unwrap();
    }
    writer.finalize().unwrap();

    temp_file
}

#[test]
fn test_concurrent_vfs_readers() {
    // Create database with 1000 rows
    let db_data = create_test_database(1000);
    let temp_file = create_archive_with_database("main.db", &db_data);
    let path = temp_file.path();

    // 10 threads, each opening their own VfsReader and querying the database
    let handles: Vec<_> = (0..10)
        .map(|thread_id| {
            let path_clone = path.to_path_buf();
            thread::spawn(move || {
                // Each thread opens its own VfsReader
                let mut vfs = VfsReader::open(&path_clone).unwrap();
                let conn = vfs.open_database("main.db").unwrap();

                // Execute 1000 queries
                for i in 0..1000 {
                    let result: i64 = conn
                        .query_row(
                            "SELECT value FROM test WHERE id = ?",
                            params![i % 1000 + 1], // SQLite IDs start at 1
                            |row| row.get(0),
                        )
                        .unwrap();
                    assert_eq!(result, (i % 1000) as i64);
                }

                println!("Thread {} completed 1000 queries", thread_id);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    println!("✓ All 10 threads completed successfully (10,000 total queries)");
}

#[test]
fn test_vfs_connection_cleanup() {
    // Create small test database
    let db_data = create_test_database(100);
    let temp_file = create_archive_with_database("test.db", &db_data);
    let path = temp_file.path();

    // Create and drop 100 VFS connections
    for i in 0..100 {
        let mut vfs = VfsReader::open(path).unwrap();
        let conn = vfs.open_database("test.db").unwrap();

        // Execute query
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM test", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 100);

        // VFS should auto-cleanup temp file on drop
        drop(conn);
        drop(vfs);

        if (i + 1) % 20 == 0 {
            println!("Created and dropped {} VFS connections", i + 1);
        }
    }

    println!("✓ All 100 VFS connections created and cleaned up successfully");
}

#[test]
fn test_concurrent_vfs_different_databases() {
    // Create three databases with different row counts
    let db1_data = create_test_database(100);
    let db2_data = create_test_database(200);
    let db3_data = create_test_database(300);

    let temp_file = create_archive_with_databases(&[
        ("db1.db", db1_data),
        ("db2.db", db2_data),
        ("db3.db", db3_data),
    ]);
    let path = temp_file.path();

    // Three threads, each accessing a different database
    let databases = [
        ("db1.db", 100),
        ("db2.db", 200),
        ("db3.db", 300),
    ];

    let handles: Vec<_> = databases
        .iter()
        .enumerate()
        .map(|(idx, (db_name, expected_count))| {
            let path_clone = path.to_path_buf();
            let db_name_clone = db_name.to_string();
            let expected_count = *expected_count;

            thread::spawn(move || {
                let mut vfs = VfsReader::open(&path_clone).unwrap();
                let conn = vfs.open_database(&db_name_clone).unwrap();

                // Query row count
                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM test", [], |row| row.get(0))
                    .unwrap();
                assert_eq!(count, expected_count);

                // Query individual rows
                for i in 0..expected_count {
                    let value: i64 = conn
                        .query_row(
                            "SELECT value FROM test WHERE id = ?",
                            params![i + 1],
                            |row| row.get(0),
                        )
                        .unwrap();
                    assert_eq!(value, i as i64);
                }

                println!(
                    "Thread {} completed: {} has {} rows",
                    idx, db_name_clone, expected_count
                );
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    println!("✓ All 3 threads accessed different databases successfully");
}

#[test]
fn test_concurrent_vfs_same_archive_different_readers() {
    // Test that multiple VfsReader instances can safely share the same archive file
    let db_data = create_test_database(500);
    let temp_file = create_archive_with_database("shared.db", &db_data);
    let path = temp_file.path();

    // 5 threads, each with their own VfsReader instance
    let handles: Vec<_> = (0..5)
        .map(|thread_id| {
            let path_clone = path.to_path_buf();
            thread::spawn(move || {
                // Each thread creates its own VfsReader
                let mut vfs = VfsReader::open(&path_clone).unwrap();
                let conn = vfs.open_database("shared.db").unwrap();

                // Query a subset of rows
                for i in (thread_id * 100)..((thread_id + 1) * 100) {
                    let value: i64 = conn
                        .query_row(
                            "SELECT value FROM test WHERE id = ?",
                            params![i + 1],
                            |row| row.get(0),
                        )
                        .unwrap();
                    assert_eq!(value, i as i64);
                }

                println!("Thread {} completed its row subset", thread_id);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    println!("✓ 5 VfsReader instances shared archive successfully");
}

#[test]
fn test_vfs_database_list_concurrent() {
    // Test concurrent calls to list_databases()
    let db1_data = create_test_database(10);
    let db2_data = create_test_database(20);
    let db3_data = create_test_database(30);

    let temp_file = create_archive_with_databases(&[
        ("alpha.db", db1_data),
        ("beta.sqlite", db2_data),
        ("gamma.sqlite3", db3_data),
    ]);
    let path = temp_file.path();

    // 10 threads calling list_databases simultaneously
    let handles: Vec<_> = (0..10)
        .map(|thread_id| {
            let path_clone = path.to_path_buf();
            thread::spawn(move || {
                let vfs = VfsReader::open(&path_clone).unwrap();

                for _ in 0..100 {
                    let dbs = vfs.list_databases();
                    assert_eq!(dbs.len(), 3);
                    assert!(dbs.contains(&"alpha.db".to_string()));
                    assert!(dbs.contains(&"beta.sqlite".to_string()));
                    assert!(dbs.contains(&"gamma.sqlite3".to_string()));
                }

                println!("Thread {} listed databases 100 times", thread_id);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    println!("✓ 10 threads called list_databases() concurrently (1000 total calls)");
}
