//! SQLite-backed notes index implementation.

mod builder_methods;
mod connection;
mod repo_impl;
mod transaction;

#[cfg(test)]
mod tests;

use rusqlite::Connection;

// Re-export the Transaction type
pub use transaction::Transaction;

// ===========================================
// SqliteIndex Struct
// ===========================================

/// SQLite-backed notes index.
///
/// Manages the database connection and provides access to the notes index.
pub struct SqliteIndex {
    pub(crate) conn: Connection,
}
