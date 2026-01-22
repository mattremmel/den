//! SQLite-backed notes index implementation.

use crate::index::{IndexError, IndexResult, create_schema};
use rusqlite::{Connection, Params};
use std::fs;
use std::path::Path;

// ===========================================
// SqliteIndex Struct
// ===========================================

/// SQLite-backed notes index.
///
/// Manages the database connection and provides access to the notes index.
pub struct SqliteIndex {
    conn: Connection,
}

impl SqliteIndex {
    // ===========================================
    // Cycle 1: In-Memory Connection
    // ===========================================

    /// Opens an in-memory SQLite database with the notes schema.
    ///
    /// This is useful for testing and temporary indexes that don't need persistence.
    pub fn open_in_memory() -> IndexResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        create_schema(&conn)?;
        Ok(Self { conn })
    }

    // ===========================================
    // Cycle 2: File-Based Connection
    // ===========================================

    /// Opens or creates a SQLite database at the given path.
    ///
    /// Creates parent directories if they don't exist. Initializes the schema
    /// if this is a new database.
    pub fn open(path: &Path) -> IndexResult<Self> {
        // Create parent directories if needed
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            fs::create_dir_all(parent).map_err(|e| IndexError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        create_schema(&conn)?;
        Ok(Self { conn })
    }

    // ===========================================
    // Cycle 4: Connection Accessors
    // ===========================================

    /// Returns a reference to the underlying SQLite connection.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Returns a mutable reference to the underlying SQLite connection.
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    // ===========================================
    // Cycle 5: Transaction Support
    // ===========================================

    /// Begins a new transaction.
    ///
    /// The transaction will automatically rollback on drop unless `commit()` is called.
    pub fn transaction(&mut self) -> IndexResult<Transaction<'_>> {
        self.conn.execute_batch("BEGIN")?;
        Ok(Transaction {
            conn: &self.conn,
            finished: false,
        })
    }
}

// ===========================================
// Transaction Struct
// ===========================================

/// A database transaction with RAII-based automatic rollback.
///
/// The transaction will automatically rollback when dropped unless
/// `commit()` is called explicitly.
pub struct Transaction<'a> {
    conn: &'a Connection,
    finished: bool,
}

impl<'a> Transaction<'a> {
    /// Executes a SQL statement within the transaction.
    pub fn execute(&self, sql: &str, params: impl Params) -> IndexResult<usize> {
        Ok(self.conn.execute(sql, params)?)
    }

    /// Commits the transaction.
    ///
    /// Consumes the transaction, preventing automatic rollback on drop.
    pub fn commit(mut self) -> IndexResult<()> {
        self.conn.execute_batch("COMMIT")?;
        self.finished = true;
        Ok(())
    }

    /// Rolls back the transaction explicitly.
    ///
    /// Consumes the transaction. This is equivalent to dropping without commit,
    /// but makes the intent explicit.
    pub fn rollback(mut self) -> IndexResult<()> {
        self.conn.execute_batch("ROLLBACK")?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if !self.finished {
            // Attempt rollback, but ignore errors since we're in drop
            let _ = self.conn.execute_batch("ROLLBACK");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // ===========================================
    // Cycle 1: Basic In-Memory Connection
    // ===========================================

    #[test]
    fn open_in_memory_succeeds() {
        let result = SqliteIndex::open_in_memory();
        assert!(result.is_ok(), "open_in_memory should succeed");
    }

    #[test]
    fn open_in_memory_initializes_schema() {
        let index = SqliteIndex::open_in_memory().unwrap();

        // Check that the notes table exists
        let table_exists: bool = index
            .conn()
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='notes'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);

        assert!(
            table_exists,
            "notes table should exist after open_in_memory"
        );
    }

    #[test]
    fn open_in_memory_enables_foreign_keys() {
        let index = SqliteIndex::open_in_memory().unwrap();

        let fk_enabled: i32 = index
            .conn()
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();

        assert_eq!(fk_enabled, 1, "foreign keys should be enabled");
    }

    // ===========================================
    // Cycle 2: File-Based Connection
    // ===========================================

    #[test]
    fn open_creates_file() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let _index = SqliteIndex::open(&db_path).unwrap();

        assert!(db_path.exists(), "database file should be created");
    }

    #[test]
    fn open_creates_parent_directory() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("subdir").join("nested").join("test.db");

        let _index = SqliteIndex::open(&db_path).unwrap();

        assert!(db_path.exists(), "database file should be created");
        assert!(
            db_path.parent().unwrap().exists(),
            "parent directories should be created"
        );
    }

    #[test]
    fn open_existing_preserves_data() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Create and populate database
        {
            let index = SqliteIndex::open(&db_path).unwrap();
            index
                .conn()
                .execute(
                    "INSERT INTO notes (id, path, title, created, modified, content_hash)
                     VALUES (?, ?, ?, ?, ?, ?)",
                    [
                        "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                        "test.md",
                        "Test Title",
                        "2024-01-15T10:30:00Z",
                        "2024-01-15T10:30:00Z",
                        "abc123",
                    ],
                )
                .unwrap();
        }

        // Reopen and verify data
        let index = SqliteIndex::open(&db_path).unwrap();
        let count: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
            .unwrap();

        assert_eq!(count, 1, "data should be preserved after reopen");
    }

    #[test]
    fn open_existing_does_not_duplicate_schema() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Open multiple times
        SqliteIndex::open(&db_path).unwrap();
        SqliteIndex::open(&db_path).unwrap();
        let index = SqliteIndex::open(&db_path).unwrap();

        // Count tables (should not have duplicates)
        let table_count: i64 = index
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='notes'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(table_count, 1, "should not have duplicate tables");
    }

    // ===========================================
    // Cycle 3: Error Handling
    // ===========================================

    #[test]
    fn open_readonly_dir_returns_io_error() {
        // Skip this test on non-Unix platforms or if running as root
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let dir = tempdir().unwrap();
            let readonly_dir = dir.path().join("readonly");
            fs::create_dir(&readonly_dir).unwrap();

            // Make directory read-only
            let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
            perms.set_mode(0o444);
            fs::set_permissions(&readonly_dir, perms).unwrap();

            let db_path = readonly_dir.join("subdir").join("test.db");
            let result = SqliteIndex::open(&db_path);

            // Restore permissions for cleanup
            let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&readonly_dir, perms).unwrap();

            assert!(result.is_err(), "should fail when parent is read-only");
            if let Err(IndexError::Io { path, .. }) = result {
                assert!(
                    path.to_string_lossy().contains("readonly"),
                    "error should include path context"
                );
            } else {
                panic!("expected Io error variant");
            }
        }
    }

    // ===========================================
    // Cycle 4: Connection Accessors
    // ===========================================

    #[test]
    fn conn_returns_reference() {
        let index = SqliteIndex::open_in_memory().unwrap();

        // Should be able to use the connection reference for queries
        let result: i64 = index
            .conn()
            .query_row("SELECT 1", [], |row| row.get(0))
            .unwrap();

        assert_eq!(result, 1);
    }

    #[test]
    fn conn_mut_allows_modifications() {
        let mut index = SqliteIndex::open_in_memory().unwrap();

        // Should be able to execute statements via mutable reference
        let result = index.conn_mut().execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Test Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        );

        assert!(result.is_ok(), "should be able to modify via conn_mut");
    }

    // ===========================================
    // Cycle 5: Transaction Support
    // ===========================================

    #[test]
    fn transaction_commits_on_success() {
        let mut index = SqliteIndex::open_in_memory().unwrap();

        {
            let tx = index.transaction().unwrap();
            tx.execute(
                "INSERT INTO notes (id, path, title, created, modified, content_hash)
                 VALUES (?, ?, ?, ?, ?, ?)",
                [
                    "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                    "test.md",
                    "Test Title",
                    "2024-01-15T10:30:00Z",
                    "2024-01-15T10:30:00Z",
                    "abc123",
                ],
            )
            .unwrap();
            tx.commit().unwrap();
        }

        // Data should persist after transaction
        let count: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
            .unwrap();

        assert_eq!(count, 1, "committed data should persist");
    }

    #[test]
    fn transaction_rollback_on_drop() {
        let mut index = SqliteIndex::open_in_memory().unwrap();

        {
            let tx = index.transaction().unwrap();
            tx.execute(
                "INSERT INTO notes (id, path, title, created, modified, content_hash)
                 VALUES (?, ?, ?, ?, ?, ?)",
                [
                    "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                    "test.md",
                    "Test Title",
                    "2024-01-15T10:30:00Z",
                    "2024-01-15T10:30:00Z",
                    "abc123",
                ],
            )
            .unwrap();
            // Transaction dropped without commit
        }

        // Data should be rolled back
        let count: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
            .unwrap();

        assert_eq!(count, 0, "uncommitted data should be rolled back");
    }

    #[test]
    fn transaction_explicit_rollback() {
        let mut index = SqliteIndex::open_in_memory().unwrap();

        {
            let tx = index.transaction().unwrap();
            tx.execute(
                "INSERT INTO notes (id, path, title, created, modified, content_hash)
                 VALUES (?, ?, ?, ?, ?, ?)",
                [
                    "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                    "test.md",
                    "Test Title",
                    "2024-01-15T10:30:00Z",
                    "2024-01-15T10:30:00Z",
                    "abc123",
                ],
            )
            .unwrap();
            tx.rollback().unwrap();
        }

        // Data should be rolled back
        let count: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
            .unwrap();

        assert_eq!(count, 0, "explicitly rolled back data should not persist");
    }
}
