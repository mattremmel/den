//! Connection management for SqliteIndex.

use super::SqliteIndex;
use super::transaction::Transaction;
use crate::index::{IndexError, IndexResult, create_schema};
use rusqlite::Connection;
use std::fs;
use std::path::Path;

impl SqliteIndex {
    // ===========================================
    // In-Memory Connection
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
    // File-Based Connection
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
    // Connection Accessors
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
    // Transaction Support
    // ===========================================

    /// Begins a new transaction.
    ///
    /// The transaction will automatically rollback on drop unless `commit()` is called.
    pub fn transaction(&mut self) -> IndexResult<Transaction<'_>> {
        self.conn.execute_batch("BEGIN")?;
        Ok(Transaction::new(&self.conn))
    }
}
