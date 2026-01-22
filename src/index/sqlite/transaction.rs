//! RAII-based transaction support for SQLite.

use crate::index::IndexResult;
use rusqlite::{Connection, Params};

/// A database transaction with RAII-based automatic rollback.
///
/// The transaction will automatically rollback when dropped unless
/// `commit()` is called explicitly.
pub struct Transaction<'a> {
    conn: &'a Connection,
    finished: bool,
}

impl<'a> Transaction<'a> {
    /// Creates a new transaction from a connection reference.
    pub(crate) fn new(conn: &'a Connection) -> Self {
        Self {
            conn,
            finished: false,
        }
    }

    /// Returns a reference to the underlying connection.
    pub(crate) fn conn(&self) -> &Connection {
        self.conn
    }

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
