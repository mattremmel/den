//! SQLite-backed notes index implementation.

use crate::domain::{Note, NoteId, Tag, Topic};
use crate::index::{
    IndexError, IndexRepository, IndexResult, IndexedNote, SearchResult, TagWithCount,
    TopicWithCount, create_schema,
};
use crate::infra::ContentHash;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, Params};
use std::fs;
use std::path::{Path, PathBuf};

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

// ===========================================
// IndexRepository Implementation
// ===========================================

impl IndexRepository for SqliteIndex {
    fn remove_note(&mut self, id: &NoteId) -> IndexResult<()> {
        self.conn
            .execute("DELETE FROM notes WHERE id = ?", [id.to_string()])?;
        Ok(())
    }

    fn get_note(&self, id: &NoteId) -> IndexResult<Option<IndexedNote>> {
        // Query notes table
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, created, modified, path, content_hash
             FROM notes WHERE id = ?",
        )?;

        let note_row = stmt.query_row([id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        });

        let (id_str, title, description, created_str, modified_str, path_str, hash_str) =
            match note_row {
                Ok(row) => row,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(e) => return Err(IndexError::Database(e)),
            };

        // Parse values
        let note_id: NoteId = id_str
            .parse()
            .map_err(|e| IndexError::InvalidQuery(format!("invalid note ID in database: {}", e)))?;

        let created = DateTime::parse_from_rfc3339(&created_str)
            .map_err(|e| IndexError::InvalidQuery(format!("invalid created timestamp: {}", e)))?
            .with_timezone(&Utc);

        let modified = DateTime::parse_from_rfc3339(&modified_str)
            .map_err(|e| IndexError::InvalidQuery(format!("invalid modified timestamp: {}", e)))?
            .with_timezone(&Utc);

        let content_hash = ContentHash::from_hex(&hash_str)
            .map_err(|e| IndexError::InvalidQuery(format!("invalid content hash: {}", e)))?;

        let path = PathBuf::from(path_str);

        // Query topics via JOIN
        let topics: Vec<Topic> = self
            .conn
            .prepare("SELECT t.path FROM topics t JOIN note_topics nt ON t.id = nt.topic_id WHERE nt.note_id = ?")?
            .query_map([id.to_string()], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .filter_map(|path| Topic::new(&path).ok())
            .collect();

        // Query tags via JOIN
        let tags: Vec<Tag> = self
            .conn
            .prepare("SELECT t.name FROM tags t JOIN note_tags nt ON t.id = nt.tag_id WHERE nt.note_id = ?")?
            .query_map([id.to_string()], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .filter_map(|name| Tag::new(&name).ok())
            .collect();

        // Build IndexedNote
        let mut builder =
            IndexedNote::builder(note_id, title, created, modified, path, content_hash);

        if let Some(desc) = description {
            builder = builder.description(desc);
        }

        builder = builder.topics(topics).tags(tags);

        Ok(Some(builder.build()))
    }

    fn upsert_note(
        &mut self,
        note: &Note,
        content_hash: &ContentHash,
        path: &Path,
    ) -> IndexResult<()> {
        let tx = self.transaction()?;

        // 1. INSERT/UPDATE notes row
        let id_str = note.id().to_string();
        let path_str = path.to_string_lossy();
        let created_str = note.created().to_rfc3339();
        let modified_str = note.modified().to_rfc3339();
        let hash_str = content_hash.as_str();
        let aliases_text = note.aliases().join(" ");
        let aliases_text_opt = if aliases_text.is_empty() {
            None
        } else {
            Some(aliases_text.as_str())
        };

        tx.conn.execute(
            "INSERT INTO notes (id, path, title, description, created, modified, content_hash, aliases_text)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                 path = excluded.path,
                 title = excluded.title,
                 description = excluded.description,
                 modified = excluded.modified,
                 content_hash = excluded.content_hash,
                 aliases_text = excluded.aliases_text",
            rusqlite::params![
                id_str,
                path_str,
                note.title(),
                note.description(),
                created_str,
                modified_str,
                hash_str,
                aliases_text_opt,
            ],
        )?;

        // 2. Delete existing junctions
        tx.conn
            .execute("DELETE FROM note_topics WHERE note_id = ?", [&id_str])?;
        tx.conn
            .execute("DELETE FROM note_tags WHERE note_id = ?", [&id_str])?;

        // 3. Insert topics (OR IGNORE) and junctions
        for topic in note.topics() {
            let topic_path = topic.to_string();
            tx.conn.execute(
                "INSERT OR IGNORE INTO topics (path) VALUES (?)",
                [&topic_path],
            )?;
            tx.conn.execute(
                "INSERT INTO note_topics (note_id, topic_id)
                 SELECT ?, id FROM topics WHERE path = ?",
                [&id_str, &topic_path],
            )?;
        }

        // 4. Insert tags (OR IGNORE) and junctions
        for tag in note.tags() {
            tx.conn.execute(
                "INSERT OR IGNORE INTO tags (name) VALUES (?)",
                [tag.as_str()],
            )?;
            tx.conn.execute(
                "INSERT INTO note_tags (note_id, tag_id)
                 SELECT ?, id FROM tags WHERE name = ?",
                [&id_str, tag.as_str()],
            )?;
        }

        tx.commit()
    }

    fn list_by_topic(
        &self,
        topic: &Topic,
        include_descendants: bool,
    ) -> IndexResult<Vec<IndexedNote>> {
        let topic_path = topic.to_string();

        let query = if include_descendants {
            "SELECT DISTINCT n.id FROM notes n
             JOIN note_topics nt ON n.id = nt.note_id
             JOIN topics t ON nt.topic_id = t.id
             WHERE t.path = ?1 OR t.path LIKE ?2"
        } else {
            "SELECT DISTINCT n.id FROM notes n
             JOIN note_topics nt ON n.id = nt.note_id
             JOIN topics t ON nt.topic_id = t.id
             WHERE t.path = ?1"
        };

        let mut stmt = self.conn.prepare(query)?;

        let note_ids: Vec<NoteId> = if include_descendants {
            let pattern = format!("{}/%", topic_path);
            stmt.query_map(rusqlite::params![&topic_path, &pattern], |row| {
                row.get::<_, String>(0)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|id_str| id_str.parse().ok())
            .collect()
        } else {
            stmt.query_map([&topic_path], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .filter_map(|id_str| id_str.parse().ok())
                .collect()
        };

        let mut notes = Vec::with_capacity(note_ids.len());
        for id in note_ids {
            if let Some(note) = self.get_note(&id)? {
                notes.push(note);
            }
        }

        Ok(notes)
    }

    fn list_by_tag(&self, tag: &Tag) -> IndexResult<Vec<IndexedNote>> {
        let query = "SELECT DISTINCT n.id FROM notes n
                     JOIN note_tags nt ON n.id = nt.note_id
                     JOIN tags t ON nt.tag_id = t.id
                     WHERE t.name = ?";

        let mut stmt = self.conn.prepare(query)?;
        let note_ids: Vec<NoteId> = stmt
            .query_map([tag.as_str()], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .filter_map(|id_str| id_str.parse().ok())
            .collect();

        let mut notes = Vec::with_capacity(note_ids.len());
        for id in note_ids {
            if let Some(note) = self.get_note(&id)? {
                notes.push(note);
            }
        }

        Ok(notes)
    }

    fn search(&self, query: &str) -> IndexResult<Vec<SearchResult>> {
        // Phase 1: Handle empty/whitespace queries
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Execute FTS query with weighted BM25 ranking
        // Weights: title=10, description=5, aliases=5, body=1
        let mut stmt = self.conn.prepare(
            "SELECT
                n.id,
                -bm25(notes_fts, 10.0, 5.0, 5.0, 1.0) as rank,
                snippet(notes_fts, -1, '<b>', '</b>', '...', 20) as snippet
             FROM notes_fts
             JOIN notes n ON notes_fts.rowid = n.rowid
             WHERE notes_fts MATCH ?1
             ORDER BY rank DESC",
        )?;

        let row_iter = stmt.query_map([query], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, String>(2)?,
            ))
        });

        // Collect results, properly handling FTS errors
        let mut results = Vec::new();
        for row_result in row_iter.map_err(|e| {
            let msg = e.to_string();
            if msg.contains("fts5") || msg.contains("syntax") {
                IndexError::InvalidQuery(format!("invalid FTS query: {}", e))
            } else {
                IndexError::Database(e)
            }
        })? {
            // Map rusqlite::Error to IndexError for each row
            let row = row_result.map_err(|e| {
                let msg = e.to_string();
                if msg.contains("fts5") || msg.contains("syntax") {
                    IndexError::InvalidQuery(format!("invalid FTS query: {}", e))
                } else {
                    IndexError::Database(e)
                }
            })?;
            results.push(row);
        }

        // Fetch full notes and build SearchResult
        let mut search_results = Vec::with_capacity(results.len());
        for (id_str, rank, snippet) in results {
            let note_id: NoteId = id_str
                .parse()
                .map_err(|e| IndexError::InvalidQuery(format!("invalid note ID: {}", e)))?;

            if let Some(note) = self.get_note(&note_id)? {
                let result = if snippet.is_empty() {
                    SearchResult::new(note, rank)
                } else {
                    SearchResult::with_snippet(note, rank, snippet)
                };
                search_results.push(result);
            }
        }

        Ok(search_results)
    }

    fn all_topics(&self) -> IndexResult<Vec<TopicWithCount>> {
        let query = "SELECT t.path,
                            COUNT(DISTINCT nt.note_id) as exact_count,
                            (SELECT COUNT(DISTINCT nt2.note_id)
                             FROM topics t2
                             JOIN note_topics nt2 ON t2.id = nt2.topic_id
                             WHERE t2.path = t.path OR t2.path LIKE t.path || '/%'
                            ) as total_count
                     FROM topics t
                     INNER JOIN note_topics nt ON t.id = nt.topic_id
                     GROUP BY t.id
                     ORDER BY t.path";

        let mut stmt = self.conn.prepare(query)?;
        let topics = stmt
            .query_map([], |row| {
                let path: String = row.get(0)?;
                let exact_count: u32 = row.get(1)?;
                let total_count: u32 = row.get(2)?;
                Ok((path, exact_count, total_count))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(path, exact_count, total_count)| {
                Topic::new(&path)
                    .ok()
                    .map(|topic| TopicWithCount::new(topic, exact_count, total_count))
            })
            .collect();

        Ok(topics)
    }

    fn all_tags(&self) -> IndexResult<Vec<TagWithCount>> {
        let query = "SELECT t.name, COUNT(nt.note_id) as count
                     FROM tags t
                     INNER JOIN note_tags nt ON t.id = nt.tag_id
                     GROUP BY t.id
                     ORDER BY t.name";

        let mut stmt = self.conn.prepare(query)?;
        let tags = stmt
            .query_map([], |row| {
                let name: String = row.get(0)?;
                let count: u32 = row.get(1)?;
                Ok((name, count))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(name, count)| {
                Tag::new(&name)
                    .ok()
                    .map(|tag| TagWithCount::new(tag, count))
            })
            .collect();

        Ok(tags)
    }

    fn get_content_hash(&self, path: &Path) -> IndexResult<Option<ContentHash>> {
        let path_str = path.to_string_lossy();
        let mut stmt = self
            .conn
            .prepare("SELECT content_hash FROM notes WHERE path = ?")?;
        let hash = stmt.query_row([&*path_str], |row| {
            let hash_str: String = row.get(0)?;
            Ok(hash_str)
        });
        match hash {
            Ok(hash_str) => {
                Ok(Some(ContentHash::from_hex(&hash_str).map_err(|e| {
                    IndexError::InvalidQuery(format!("invalid hash: {}", e))
                })?))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(IndexError::Database(e)),
        }
    }

    fn list_all(&self) -> IndexResult<Vec<IndexedNote>> {
        let mut stmt = self.conn.prepare("SELECT id FROM notes")?;
        let note_ids: Vec<NoteId> = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .filter_map(|id_str| id_str.parse().ok())
            .collect();

        let mut notes = Vec::with_capacity(note_ids.len());
        for id in note_ids {
            if let Some(note) = self.get_note(&id)? {
                notes.push(note);
            }
        }

        Ok(notes)
    }
}

// ===========================================
// Additional Methods for IndexBuilder
// ===========================================

impl SqliteIndex {
    /// Returns all indexed paths with their content hashes.
    ///
    /// Used by IndexBuilder for incremental updates to detect changes.
    pub fn all_indexed_paths(&self) -> IndexResult<Vec<(PathBuf, ContentHash)>> {
        let mut stmt = self.conn.prepare("SELECT path, content_hash FROM notes")?;
        let results = stmt
            .query_map([], |row| {
                let path: String = row.get(0)?;
                let hash: String = row.get(1)?;
                Ok((path, hash))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(path, hash)| {
                ContentHash::from_hex(&hash)
                    .ok()
                    .map(|h| (PathBuf::from(path), h))
            })
            .collect();
        Ok(results)
    }

    /// Removes a note from the index by its file path.
    ///
    /// Returns `true` if a note was removed, `false` if no note was found at that path.
    pub fn remove_by_path(&mut self, path: &Path) -> IndexResult<bool> {
        let rows = self.conn.execute(
            "DELETE FROM notes WHERE path = ?",
            [path.to_string_lossy().as_ref()],
        )?;
        Ok(rows > 0)
    }

    /// Clears all notes from the index.
    ///
    /// Used by IndexBuilder for full rebuilds.
    pub fn clear(&mut self) -> IndexResult<()> {
        self.conn.execute("DELETE FROM notes", [])?;
        Ok(())
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

    // ===========================================
    // IndexRepository Tests - Test Helpers
    // ===========================================

    use crate::domain::{Note, NoteId, Tag, Topic};
    use crate::index::IndexRepository;
    use crate::infra::ContentHash;
    use chrono::{DateTime, Utc};

    fn test_note_id() -> NoteId {
        "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap()
    }

    fn other_note_id() -> NoteId {
        "01HQ4A2R9PXJK4QZPW8V2R6T9Z".parse().unwrap()
    }

    fn test_datetime() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn later_datetime() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-16T14:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn test_content_hash() -> ContentHash {
        ContentHash::compute(b"test content")
    }

    fn test_path() -> std::path::PathBuf {
        std::path::PathBuf::from("notes/test.md")
    }

    // ===========================================
    // Phase 1: remove_note Tests
    // ===========================================

    #[test]
    fn test_remove_note_nonexistent_is_idempotent() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let id = test_note_id();

        // Removing a non-existent note should succeed (idempotent)
        let result = index.remove_note(&id);
        assert!(result.is_ok(), "remove_note should be idempotent");
    }

    #[test]
    fn test_remove_note_existing_deletes_row() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::new(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        // Insert a note
        index.upsert_note(&note, &hash, &path).unwrap();

        // Verify it exists
        let count_before: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count_before, 1);

        // Remove it
        index.remove_note(note.id()).unwrap();

        // Verify it's gone
        let count_after: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count_after, 0, "note should be deleted");
    }

    #[test]
    fn test_remove_note_cascades_to_note_topics() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::builder(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![Topic::new("software").unwrap()])
        .build()
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        // Verify junction exists
        let junction_count_before: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM note_topics", [], |row| row.get(0))
            .unwrap();
        assert_eq!(junction_count_before, 1);

        // Remove note
        index.remove_note(note.id()).unwrap();

        // Verify junction is cascaded
        let junction_count_after: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM note_topics", [], |row| row.get(0))
            .unwrap();
        assert_eq!(junction_count_after, 0, "note_topics should cascade delete");
    }

    #[test]
    fn test_remove_note_cascades_to_note_tags() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::builder(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .tags(vec![Tag::new("draft").unwrap()])
        .build()
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        // Verify junction exists
        let junction_count_before: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM note_tags", [], |row| row.get(0))
            .unwrap();
        assert_eq!(junction_count_before, 1);

        // Remove note
        index.remove_note(note.id()).unwrap();

        // Verify junction is cascaded
        let junction_count_after: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM note_tags", [], |row| row.get(0))
            .unwrap();
        assert_eq!(junction_count_after, 0, "note_tags should cascade delete");
    }

    // ===========================================
    // Phase 2: get_note Tests
    // ===========================================

    #[test]
    fn test_get_note_nonexistent_returns_none() {
        let index = SqliteIndex::open_in_memory().unwrap();
        let result = index.get_note(&test_note_id()).unwrap();
        assert!(
            result.is_none(),
            "get_note should return None for non-existent note"
        );
    }

    #[test]
    fn test_get_note_existing_returns_basic_fields() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::new(
            test_note_id(),
            "Test Note",
            test_datetime(),
            later_datetime(),
        )
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        let retrieved = index.get_note(note.id()).unwrap().unwrap();
        assert_eq!(retrieved.id(), note.id());
        assert_eq!(retrieved.title(), note.title());
        assert_eq!(retrieved.created(), note.created());
        assert_eq!(retrieved.modified(), note.modified());
        assert_eq!(retrieved.path(), path.as_path());
        assert_eq!(retrieved.content_hash(), &hash);
    }

    #[test]
    fn test_get_note_null_description_returns_none() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::new(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        let retrieved = index.get_note(note.id()).unwrap().unwrap();
        assert_eq!(retrieved.description(), None, "description should be None");
    }

    #[test]
    fn test_get_note_with_description() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::builder(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .description(Some("A test description"))
        .build()
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        let retrieved = index.get_note(note.id()).unwrap().unwrap();
        assert_eq!(
            retrieved.description(),
            Some("A test description"),
            "description should be retrieved"
        );
    }

    #[test]
    fn test_get_note_loads_topics() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let topics = vec![
            Topic::new("software").unwrap(),
            Topic::new("software/rust").unwrap(),
        ];
        let note = Note::builder(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .topics(topics.clone())
        .build()
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        let retrieved = index.get_note(note.id()).unwrap().unwrap();
        assert_eq!(retrieved.topics().len(), 2);
        assert!(retrieved.topics().contains(&topics[0]));
        assert!(retrieved.topics().contains(&topics[1]));
    }

    #[test]
    fn test_get_note_loads_tags() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let tags = vec![Tag::new("draft").unwrap(), Tag::new("important").unwrap()];
        let note = Note::builder(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .tags(tags.clone())
        .build()
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        let retrieved = index.get_note(note.id()).unwrap().unwrap();
        assert_eq!(retrieved.tags().len(), 2);
        assert!(retrieved.tags().contains(&tags[0]));
        assert!(retrieved.tags().contains(&tags[1]));
    }

    #[test]
    fn test_get_note_empty_topics_and_tags() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::new(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        let retrieved = index.get_note(note.id()).unwrap().unwrap();
        assert!(retrieved.topics().is_empty(), "topics should be empty");
        assert!(retrieved.tags().is_empty(), "tags should be empty");
    }

    #[test]
    fn test_get_note_parses_datetime() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let created = test_datetime();
        let modified = later_datetime();
        let note = Note::new(test_note_id(), "Test Note", created, modified).unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        let retrieved = index.get_note(note.id()).unwrap().unwrap();
        assert_eq!(retrieved.created(), created, "created should match");
        assert_eq!(retrieved.modified(), modified, "modified should match");
    }

    // ===========================================
    // Phase 3: upsert_note - Insert Path Tests
    // ===========================================

    #[test]
    fn test_upsert_note_insert_basic_fields() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::new(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        let result = index.upsert_note(&note, &hash, &path);
        assert!(result.is_ok(), "upsert should succeed");

        let count: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "one note should be inserted");
    }

    #[test]
    fn test_upsert_note_stores_timestamps_rfc3339() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::new(
            test_note_id(),
            "Test Note",
            test_datetime(),
            later_datetime(),
        )
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        let (created, modified): (String, String) = index
            .conn()
            .query_row(
                "SELECT created, modified FROM notes WHERE id = ?",
                [test_note_id().to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        // Verify it's parseable as RFC3339
        assert!(DateTime::parse_from_rfc3339(&created).is_ok());
        assert!(DateTime::parse_from_rfc3339(&modified).is_ok());
    }

    #[test]
    fn test_upsert_note_stores_content_hash() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::new(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        let stored_hash: String = index
            .conn()
            .query_row(
                "SELECT content_hash FROM notes WHERE id = ?",
                [test_note_id().to_string()],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(stored_hash, hash.as_str(), "content_hash should match");
    }

    #[test]
    fn test_upsert_note_stores_aliases_text() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::builder(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .aliases(vec!["alias1".to_string(), "alias2".to_string()])
        .build()
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        let aliases_text: Option<String> = index
            .conn()
            .query_row(
                "SELECT aliases_text FROM notes WHERE id = ?",
                [test_note_id().to_string()],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(
            aliases_text,
            Some("alias1 alias2".to_string()),
            "aliases_text should be space-separated"
        );
    }

    #[test]
    fn test_upsert_note_creates_new_topics() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::builder(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![Topic::new("software/rust").unwrap()])
        .build()
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        let topic_count: i64 = index
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM topics WHERE path = ?",
                ["software/rust"],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(topic_count, 1, "topic should be created");
    }

    #[test]
    fn test_upsert_note_creates_topic_junctions() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::builder(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![Topic::new("software").unwrap()])
        .build()
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        let junction_count: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM note_topics", [], |row| row.get(0))
            .unwrap();

        assert_eq!(junction_count, 1, "topic junction should be created");
    }

    #[test]
    fn test_upsert_note_creates_new_tags() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::builder(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .tags(vec![Tag::new("draft").unwrap()])
        .build()
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        let tag_count: i64 = index
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM tags WHERE name = ?",
                ["draft"],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(tag_count, 1, "tag should be created");
    }

    #[test]
    fn test_upsert_note_creates_tag_junctions() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::builder(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .tags(vec![Tag::new("draft").unwrap()])
        .build()
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        let junction_count: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM note_tags", [], |row| row.get(0))
            .unwrap();

        assert_eq!(junction_count, 1, "tag junction should be created");
    }

    #[test]
    fn test_upsert_note_reuses_existing_topics() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let topic = Topic::new("software").unwrap();

        // Insert first note with topic
        let note1 = Note::builder(test_note_id(), "Note 1", test_datetime(), test_datetime())
            .topics(vec![topic.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(&note1, &test_content_hash(), &test_path())
            .unwrap();

        // Insert second note with same topic
        let note2 = Note::builder(other_note_id(), "Note 2", test_datetime(), test_datetime())
            .topics(vec![topic])
            .build()
            .unwrap();
        index
            .upsert_note(
                &note2,
                &test_content_hash(),
                &std::path::PathBuf::from("other.md"),
            )
            .unwrap();

        // Should only have one topic row
        let topic_count: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM topics", [], |row| row.get(0))
            .unwrap();
        assert_eq!(topic_count, 1, "should reuse existing topic");

        // But two junctions
        let junction_count: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM note_topics", [], |row| row.get(0))
            .unwrap();
        assert_eq!(junction_count, 2, "should have two junctions");
    }

    #[test]
    fn test_upsert_note_is_atomic() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::builder(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![Topic::new("software").unwrap()])
        .tags(vec![Tag::new("draft").unwrap()])
        .build()
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        // The upsert should be atomic - all or nothing
        index.upsert_note(&note, &hash, &path).unwrap();

        // Verify all parts were inserted
        let note_count: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
            .unwrap();
        let topic_count: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM topics", [], |row| row.get(0))
            .unwrap();
        let tag_count: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))
            .unwrap();

        assert_eq!(note_count, 1);
        assert_eq!(topic_count, 1);
        assert_eq!(tag_count, 1);
    }

    // ===========================================
    // Phase 4: upsert_note - Update Path Tests
    // ===========================================

    #[test]
    fn test_upsert_note_update_basic_fields() {
        let mut index = SqliteIndex::open_in_memory().unwrap();

        // Insert initial note
        let note1 = Note::new(
            test_note_id(),
            "Original Title",
            test_datetime(),
            test_datetime(),
        )
        .unwrap();
        index
            .upsert_note(&note1, &test_content_hash(), &test_path())
            .unwrap();

        // Update the note
        let note2 = Note::new(
            test_note_id(),
            "Updated Title",
            test_datetime(),
            later_datetime(),
        )
        .unwrap();
        let new_hash = ContentHash::compute(b"new content");
        index.upsert_note(&note2, &new_hash, &test_path()).unwrap();

        // Verify update
        let retrieved = index.get_note(&test_note_id()).unwrap().unwrap();
        assert_eq!(retrieved.title(), "Updated Title");
        assert_eq!(retrieved.modified(), later_datetime());
        assert_eq!(retrieved.content_hash(), &new_hash);
    }

    #[test]
    fn test_upsert_note_removes_stale_topic_junctions() {
        let mut index = SqliteIndex::open_in_memory().unwrap();

        // Insert note with topic A
        let note1 = Note::builder(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![Topic::new("topic-a").unwrap()])
        .build()
        .unwrap();
        index
            .upsert_note(&note1, &test_content_hash(), &test_path())
            .unwrap();

        // Update note with topic B (removing topic A)
        let note2 = Note::builder(
            test_note_id(),
            "Test Note",
            test_datetime(),
            later_datetime(),
        )
        .topics(vec![Topic::new("topic-b").unwrap()])
        .build()
        .unwrap();
        index
            .upsert_note(&note2, &test_content_hash(), &test_path())
            .unwrap();

        // Should only have junction for topic B
        let retrieved = index.get_note(&test_note_id()).unwrap().unwrap();
        assert_eq!(retrieved.topics().len(), 1);
        assert_eq!(retrieved.topics()[0].to_string(), "topic-b");
    }

    #[test]
    fn test_upsert_note_removes_stale_tag_junctions() {
        let mut index = SqliteIndex::open_in_memory().unwrap();

        // Insert note with tag A
        let note1 = Note::builder(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .tags(vec![Tag::new("tag-a").unwrap()])
        .build()
        .unwrap();
        index
            .upsert_note(&note1, &test_content_hash(), &test_path())
            .unwrap();

        // Update note with tag B (removing tag A)
        let note2 = Note::builder(
            test_note_id(),
            "Test Note",
            test_datetime(),
            later_datetime(),
        )
        .tags(vec![Tag::new("tag-b").unwrap()])
        .build()
        .unwrap();
        index
            .upsert_note(&note2, &test_content_hash(), &test_path())
            .unwrap();

        // Should only have junction for tag B
        let retrieved = index.get_note(&test_note_id()).unwrap().unwrap();
        assert_eq!(retrieved.tags().len(), 1);
        assert_eq!(retrieved.tags()[0].as_str(), "tag-b");
    }

    #[test]
    fn test_upsert_note_update_preserves_created() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let original_created = test_datetime();

        // Insert initial note
        let note1 = Note::new(
            test_note_id(),
            "Test Note",
            original_created,
            original_created,
        )
        .unwrap();
        index
            .upsert_note(&note1, &test_content_hash(), &test_path())
            .unwrap();

        // Update the note with a different created timestamp (shouldn't change stored created)
        let new_created = later_datetime();
        let note2 = Note::new(
            test_note_id(),
            "Updated Note",
            new_created,
            later_datetime(),
        )
        .unwrap();
        index
            .upsert_note(&note2, &test_content_hash(), &test_path())
            .unwrap();

        // Verify created timestamp is preserved from first insert
        let retrieved = index.get_note(&test_note_id()).unwrap().unwrap();
        assert_eq!(
            retrieved.created(),
            original_created,
            "created should be preserved from initial insert"
        );
    }

    // ===========================================
    // Phase 5: Integration Tests
    // ===========================================

    #[test]
    fn test_upsert_then_get_roundtrip() {
        let mut index = SqliteIndex::open_in_memory().unwrap();

        let note = Note::builder(
            test_note_id(),
            "Roundtrip Test",
            test_datetime(),
            later_datetime(),
        )
        .description(Some("A description"))
        .topics(vec![
            Topic::new("software").unwrap(),
            Topic::new("software/rust").unwrap(),
        ])
        .tags(vec![
            Tag::new("draft").unwrap(),
            Tag::new("review").unwrap(),
        ])
        .aliases(vec!["alias1".to_string(), "alias2".to_string()])
        .build()
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        let retrieved = index.get_note(note.id()).unwrap().unwrap();

        assert_eq!(retrieved.id(), note.id());
        assert_eq!(retrieved.title(), note.title());
        assert_eq!(retrieved.description(), note.description());
        assert_eq!(retrieved.created(), note.created());
        assert_eq!(retrieved.modified(), note.modified());
        assert_eq!(retrieved.path(), path.as_path());
        assert_eq!(retrieved.content_hash(), &hash);
        assert_eq!(retrieved.topics().len(), 2);
        assert_eq!(retrieved.tags().len(), 2);
    }

    #[test]
    fn test_upsert_remove_get_returns_none() {
        let mut index = SqliteIndex::open_in_memory().unwrap();

        let note = Note::new(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
        )
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        // Insert
        index.upsert_note(&note, &hash, &path).unwrap();

        // Verify exists
        assert!(index.get_note(note.id()).unwrap().is_some());

        // Remove
        index.remove_note(note.id()).unwrap();

        // Verify gone
        assert!(
            index.get_note(note.id()).unwrap().is_none(),
            "should return None after removal"
        );
    }

    #[test]
    fn test_multiple_notes_share_topic() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let shared_topic = Topic::new("shared").unwrap();

        // Insert first note
        let note1 = Note::builder(test_note_id(), "Note 1", test_datetime(), test_datetime())
            .topics(vec![shared_topic.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(&note1, &test_content_hash(), &test_path())
            .unwrap();

        // Insert second note
        let note2 = Note::builder(other_note_id(), "Note 2", test_datetime(), test_datetime())
            .topics(vec![shared_topic])
            .build()
            .unwrap();
        index
            .upsert_note(
                &note2,
                &test_content_hash(),
                &std::path::PathBuf::from("other.md"),
            )
            .unwrap();

        // Only one topic row should exist
        let topic_count: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM topics", [], |row| row.get(0))
            .unwrap();
        assert_eq!(topic_count, 1, "only one topic row should exist");

        // Both notes should reference it
        let junction_count: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM note_topics", [], |row| row.get(0))
            .unwrap();
        assert_eq!(junction_count, 2, "both notes should reference the topic");
    }

    #[test]
    fn test_upsert_note_triggers_fts_update() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let note = Note::builder(
            test_note_id(),
            "FTS Test Note",
            test_datetime(),
            test_datetime(),
        )
        .description(Some("searchable description"))
        .build()
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();

        index.upsert_note(&note, &hash, &path).unwrap();

        // Search FTS for title
        let title_count: i64 = index
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'FTS'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(title_count, 1, "FTS should index title");

        // Search FTS for description
        let desc_count: i64 = index
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'searchable'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(desc_count, 1, "FTS should index description");
    }

    // ===========================================
    // list_by_topic Tests
    // ===========================================

    #[test]
    fn list_by_topic_returns_empty_when_no_matches() {
        let index = SqliteIndex::open_in_memory().unwrap();
        let topic = Topic::new("nonexistent").unwrap();

        let result = index.list_by_topic(&topic, false).unwrap();

        assert!(result.is_empty());
    }

    #[test]
    fn list_by_topic_exact_match_returns_matching_note() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let topic = Topic::new("software/architecture").unwrap();

        let note = Note::builder(
            test_note_id(),
            "Architecture Note",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![topic.clone()])
        .build()
        .unwrap();
        index
            .upsert_note(&note, &test_content_hash(), &test_path())
            .unwrap();

        let result = index.list_by_topic(&topic, false).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id(), note.id());
    }

    #[test]
    fn list_by_topic_exact_match_excludes_other_topics() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let target = Topic::new("software").unwrap();
        let other = Topic::new("personal").unwrap();

        let note = Note::builder(
            test_note_id(),
            "Personal Note",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![other])
        .build()
        .unwrap();
        index
            .upsert_note(&note, &test_content_hash(), &test_path())
            .unwrap();

        let result = index.list_by_topic(&target, false).unwrap();

        assert!(result.is_empty());
    }

    #[test]
    fn list_by_topic_exact_match_excludes_descendants() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let parent = Topic::new("software").unwrap();
        let child = Topic::new("software/architecture").unwrap();

        let note = Note::builder(
            test_note_id(),
            "Architecture Note",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![child])
        .build()
        .unwrap();
        index
            .upsert_note(&note, &test_content_hash(), &test_path())
            .unwrap();

        let result = index.list_by_topic(&parent, false).unwrap();

        assert!(
            result.is_empty(),
            "exact match should not include descendants"
        );
    }

    #[test]
    fn list_by_topic_with_descendants_includes_children() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let parent = Topic::new("software").unwrap();
        let child = Topic::new("software/architecture").unwrap();

        let note = Note::builder(
            test_note_id(),
            "Architecture Note",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![child])
        .build()
        .unwrap();
        index
            .upsert_note(&note, &test_content_hash(), &test_path())
            .unwrap();

        let result = index.list_by_topic(&parent, true).unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn list_by_topic_with_descendants_includes_deeply_nested() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let root = Topic::new("software").unwrap();
        let deep = Topic::new("software/architecture/patterns/creational").unwrap();

        let note = Note::builder(
            test_note_id(),
            "Deep Note",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![deep])
        .build()
        .unwrap();
        index
            .upsert_note(&note, &test_content_hash(), &test_path())
            .unwrap();

        let result = index.list_by_topic(&root, true).unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn list_by_topic_with_descendants_includes_exact_match() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let topic = Topic::new("software").unwrap();

        let note = Note::builder(
            test_note_id(),
            "Software Note",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![topic.clone()])
        .build()
        .unwrap();
        index
            .upsert_note(&note, &test_content_hash(), &test_path())
            .unwrap();

        let result = index.list_by_topic(&topic, true).unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn list_by_topic_returns_multiple_notes() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let topic = Topic::new("software").unwrap();

        let note1 = Note::builder(test_note_id(), "Note 1", test_datetime(), test_datetime())
            .topics(vec![topic.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(&note1, &test_content_hash(), &test_path())
            .unwrap();

        let note2 = Note::builder(other_note_id(), "Note 2", test_datetime(), test_datetime())
            .topics(vec![topic.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(
                &note2,
                &test_content_hash(),
                &PathBuf::from("notes/other.md"),
            )
            .unwrap();

        let result = index.list_by_topic(&topic, false).unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn list_by_topic_returns_note_once_even_with_multiple_matching_topics() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let parent = Topic::new("software").unwrap();
        let child1 = Topic::new("software/architecture").unwrap();
        let child2 = Topic::new("software/design").unwrap();

        let note = Note::builder(
            test_note_id(),
            "Multi-topic Note",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![child1, child2])
        .build()
        .unwrap();
        index
            .upsert_note(&note, &test_content_hash(), &test_path())
            .unwrap();

        let result = index.list_by_topic(&parent, true).unwrap();

        assert_eq!(result.len(), 1, "note should appear exactly once");
    }

    #[test]
    fn list_by_topic_returns_complete_indexed_note() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let topic = Topic::new("software").unwrap();
        let tag = Tag::new("important").unwrap();

        let note = Note::builder(
            test_note_id(),
            "Complete Note",
            test_datetime(),
            test_datetime(),
        )
        .description(Some("A description"))
        .topics(vec![topic.clone()])
        .tags(vec![tag.clone()])
        .build()
        .unwrap();
        let hash = test_content_hash();
        let path = test_path();
        index.upsert_note(&note, &hash, &path).unwrap();

        let result = index.list_by_topic(&topic, false).unwrap();

        assert_eq!(result.len(), 1);
        let indexed = &result[0];
        assert_eq!(indexed.title(), "Complete Note");
        assert_eq!(indexed.description(), Some("A description"));
        assert_eq!(indexed.topics(), &[topic]);
        assert_eq!(indexed.tags(), &[tag]);
        assert_eq!(indexed.content_hash(), &hash);
    }

    // ===========================================
    // list_by_tag Tests
    // ===========================================

    #[test]
    fn list_by_tag_returns_empty_when_no_matches() {
        let index = SqliteIndex::open_in_memory().unwrap();
        let tag = Tag::new("nonexistent").unwrap();
        let result = index.list_by_tag(&tag).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn list_by_tag_returns_matching_note() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let tag = Tag::new("rust").unwrap();
        let note = Note::builder(test_note_id(), "Test", test_datetime(), test_datetime())
            .tags(vec![tag.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(&note, &test_content_hash(), &test_path())
            .unwrap();

        let result = index.list_by_tag(&tag).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id(), note.id());
    }

    #[test]
    fn list_by_tag_excludes_notes_without_tag() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let rust_tag = Tag::new("rust").unwrap();
        let python_tag = Tag::new("python").unwrap();

        let note1 = Note::builder(
            test_note_id(),
            "Rust Note",
            test_datetime(),
            test_datetime(),
        )
        .tags(vec![rust_tag.clone()])
        .build()
        .unwrap();
        index
            .upsert_note(&note1, &test_content_hash(), &test_path())
            .unwrap();

        let note2 = Note::builder(
            other_note_id(),
            "Python Note",
            test_datetime(),
            test_datetime(),
        )
        .tags(vec![python_tag])
        .build()
        .unwrap();
        index
            .upsert_note(
                &note2,
                &test_content_hash(),
                &PathBuf::from("notes/other.md"),
            )
            .unwrap();

        let result = index.list_by_tag(&rust_tag).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id(), note1.id());
    }

    #[test]
    fn list_by_tag_returns_multiple_notes() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let tag = Tag::new("rust").unwrap();

        let note1 = Note::builder(test_note_id(), "Note 1", test_datetime(), test_datetime())
            .tags(vec![tag.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(&note1, &test_content_hash(), &test_path())
            .unwrap();

        let note2 = Note::builder(other_note_id(), "Note 2", test_datetime(), test_datetime())
            .tags(vec![tag.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(
                &note2,
                &test_content_hash(),
                &PathBuf::from("notes/other.md"),
            )
            .unwrap();

        let result = index.list_by_tag(&tag).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn list_by_tag_returns_complete_indexed_note() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let tag = Tag::new("rust").unwrap();
        let topic = Topic::new("software").unwrap();
        let path = PathBuf::from("notes/test.md");

        let note = Note::builder(
            test_note_id(),
            "Complete Note",
            test_datetime(),
            test_datetime(),
        )
        .tags(vec![tag.clone()])
        .topics(vec![topic.clone()])
        .description(Some("A description"))
        .build()
        .unwrap();
        index
            .upsert_note(&note, &test_content_hash(), &path)
            .unwrap();

        let result = index.list_by_tag(&tag).unwrap();
        assert_eq!(result.len(), 1);
        let indexed = &result[0];
        assert_eq!(indexed.title(), "Complete Note");
        assert_eq!(indexed.tags(), &[tag]);
        assert_eq!(indexed.topics(), &[topic]);
        assert_eq!(indexed.path(), &path);
    }

    // ===========================================
    // all_tags Tests
    // ===========================================

    #[test]
    fn all_tags_returns_empty_when_no_tags() {
        let index = SqliteIndex::open_in_memory().unwrap();
        let result = index.all_tags().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn all_tags_returns_single_tag_with_count() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let tag = Tag::new("rust").unwrap();
        let note = Note::builder(test_note_id(), "Test", test_datetime(), test_datetime())
            .tags(vec![tag.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(&note, &test_content_hash(), &test_path())
            .unwrap();

        let result = index.all_tags().unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag(), &tag);
        assert_eq!(result[0].count(), 1);
    }

    #[test]
    fn all_tags_counts_multiple_notes_per_tag() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let tag = Tag::new("rust").unwrap();

        let note1 = Note::builder(test_note_id(), "Note 1", test_datetime(), test_datetime())
            .tags(vec![tag.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(&note1, &test_content_hash(), &test_path())
            .unwrap();

        let note2 = Note::builder(other_note_id(), "Note 2", test_datetime(), test_datetime())
            .tags(vec![tag.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(
                &note2,
                &test_content_hash(),
                &PathBuf::from("notes/other.md"),
            )
            .unwrap();

        let result = index.all_tags().unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].count(), 2);
    }

    #[test]
    fn all_tags_returns_multiple_tags_sorted() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let rust_tag = Tag::new("rust").unwrap();
        let python_tag = Tag::new("python").unwrap();

        let note = Note::builder(test_note_id(), "Test", test_datetime(), test_datetime())
            .tags(vec![rust_tag.clone(), python_tag.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(&note, &test_content_hash(), &test_path())
            .unwrap();

        let result = index.all_tags().unwrap();
        assert_eq!(result.len(), 2);
        // Alphabetically sorted: python, rust
        assert_eq!(result[0].tag(), &python_tag);
        assert_eq!(result[1].tag(), &rust_tag);
    }

    #[test]
    fn all_tags_excludes_orphaned_tags() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let tag = Tag::new("rust").unwrap();

        let note = Note::builder(test_note_id(), "Test", test_datetime(), test_datetime())
            .tags(vec![tag.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(&note, &test_content_hash(), &test_path())
            .unwrap();
        index.remove_note(note.id()).unwrap();

        let result = index.all_tags().unwrap();
        // Tag exists in DB but has no notes - should be excluded
        assert!(result.is_empty());
    }

    // ===========================================
    // all_topics Tests
    // ===========================================

    #[test]
    fn all_topics_returns_empty_when_no_topics() {
        let index = SqliteIndex::open_in_memory().unwrap();
        let result = index.all_topics().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn all_topics_returns_single_topic_with_counts() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let topic = Topic::new("rust").unwrap();
        let note = Note::builder(test_note_id(), "Test", test_datetime(), test_datetime())
            .topics(vec![topic.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(&note, &test_content_hash(), &test_path())
            .unwrap();

        let result = index.all_topics().unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].topic(), &topic);
        assert_eq!(result[0].exact_count(), 1);
        assert_eq!(result[0].total_count(), 1);
    }

    #[test]
    fn all_topics_counts_multiple_notes_per_topic() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let topic = Topic::new("rust").unwrap();

        let note1 = Note::builder(test_note_id(), "Note 1", test_datetime(), test_datetime())
            .topics(vec![topic.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(&note1, &test_content_hash(), &test_path())
            .unwrap();

        let note2 = Note::builder(other_note_id(), "Note 2", test_datetime(), test_datetime())
            .topics(vec![topic.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(
                &note2,
                &test_content_hash(),
                &PathBuf::from("notes/other.md"),
            )
            .unwrap();

        let result = index.all_topics().unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].exact_count(), 2);
        assert_eq!(result[0].total_count(), 2);
    }

    #[test]
    fn all_topics_returns_multiple_topics_sorted() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let rust_topic = Topic::new("rust").unwrap();
        let python_topic = Topic::new("python").unwrap();

        let note = Note::builder(test_note_id(), "Test", test_datetime(), test_datetime())
            .topics(vec![rust_topic.clone(), python_topic.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(&note, &test_content_hash(), &test_path())
            .unwrap();

        let result = index.all_topics().unwrap();
        assert_eq!(result.len(), 2);
        // Alphabetically sorted: python, rust
        assert_eq!(result[0].topic(), &python_topic);
        assert_eq!(result[1].topic(), &rust_topic);
    }

    #[test]
    fn all_topics_excludes_orphaned_topics() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let topic = Topic::new("rust").unwrap();

        let note = Note::builder(test_note_id(), "Test", test_datetime(), test_datetime())
            .topics(vec![topic.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(&note, &test_content_hash(), &test_path())
            .unwrap();
        index.remove_note(note.id()).unwrap();

        let result = index.all_topics().unwrap();
        // Topic exists in DB but has no notes - should be excluded
        assert!(result.is_empty());
    }

    #[test]
    fn all_topics_calculates_descendant_counts() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let software_topic = Topic::new("software").unwrap();
        let software_rust_topic = Topic::new("software/rust").unwrap();

        // Two notes with 'software' topic
        let note1 = Note::builder(test_note_id(), "Note 1", test_datetime(), test_datetime())
            .topics(vec![software_topic.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(&note1, &test_content_hash(), &test_path())
            .unwrap();

        let note2 = Note::builder(other_note_id(), "Note 2", test_datetime(), test_datetime())
            .topics(vec![software_topic.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(
                &note2,
                &test_content_hash(),
                &PathBuf::from("notes/note2.md"),
            )
            .unwrap();

        // Three notes with 'software/rust' topic
        let note3 = Note::builder(
            "01HQ3K5M7NXJK4QZPW8V2R6T11".parse().unwrap(),
            "Note 3",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![software_rust_topic.clone()])
        .build()
        .unwrap();
        index
            .upsert_note(
                &note3,
                &test_content_hash(),
                &PathBuf::from("notes/note3.md"),
            )
            .unwrap();

        let note4 = Note::builder(
            "01HQ3K5M7NXJK4QZPW8V2R6T12".parse().unwrap(),
            "Note 4",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![software_rust_topic.clone()])
        .build()
        .unwrap();
        index
            .upsert_note(
                &note4,
                &test_content_hash(),
                &PathBuf::from("notes/note4.md"),
            )
            .unwrap();

        let note5 = Note::builder(
            "01HQ3K5M7NXJK4QZPW8V2R6T13".parse().unwrap(),
            "Note 5",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![software_rust_topic.clone()])
        .build()
        .unwrap();
        index
            .upsert_note(
                &note5,
                &test_content_hash(),
                &PathBuf::from("notes/note5.md"),
            )
            .unwrap();

        let result = index.all_topics().unwrap();
        assert_eq!(result.len(), 2);

        // Topics are sorted alphabetically
        assert_eq!(result[0].topic(), &software_topic);
        assert_eq!(result[0].exact_count(), 2); // Only notes with exactly 'software'
        assert_eq!(result[0].total_count(), 5); // 'software' + 'software/rust'

        assert_eq!(result[1].topic(), &software_rust_topic);
        assert_eq!(result[1].exact_count(), 3); // Only notes with 'software/rust'
        assert_eq!(result[1].total_count(), 3); // No descendants
    }

    #[test]
    fn all_topics_handles_deep_hierarchy() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let a_topic = Topic::new("a").unwrap();
        let a_b_topic = Topic::new("a/b").unwrap();
        let a_b_c_topic = Topic::new("a/b/c").unwrap();

        // One note at each level
        let note1 = Note::builder(test_note_id(), "Note 1", test_datetime(), test_datetime())
            .topics(vec![a_topic.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(&note1, &test_content_hash(), &test_path())
            .unwrap();

        let note2 = Note::builder(other_note_id(), "Note 2", test_datetime(), test_datetime())
            .topics(vec![a_b_topic.clone()])
            .build()
            .unwrap();
        index
            .upsert_note(
                &note2,
                &test_content_hash(),
                &PathBuf::from("notes/note2.md"),
            )
            .unwrap();

        let note3 = Note::builder(
            "01HQ3K5M7NXJK4QZPW8V2R6T11".parse().unwrap(),
            "Note 3",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![a_b_c_topic.clone()])
        .build()
        .unwrap();
        index
            .upsert_note(
                &note3,
                &test_content_hash(),
                &PathBuf::from("notes/note3.md"),
            )
            .unwrap();

        let result = index.all_topics().unwrap();
        assert_eq!(result.len(), 3);

        // Topics are sorted alphabetically: a, a/b, a/b/c
        assert_eq!(result[0].topic(), &a_topic);
        assert_eq!(result[0].exact_count(), 1);
        assert_eq!(result[0].total_count(), 3); // a + a/b + a/b/c

        assert_eq!(result[1].topic(), &a_b_topic);
        assert_eq!(result[1].exact_count(), 1);
        assert_eq!(result[1].total_count(), 2); // a/b + a/b/c

        assert_eq!(result[2].topic(), &a_b_c_topic);
        assert_eq!(result[2].exact_count(), 1);
        assert_eq!(result[2].total_count(), 1); // just a/b/c
    }

    // ===========================================
    // search() Tests - Helper function
    // ===========================================

    // Valid 64-char hex hash for tests
    const TEST_HASH: &str = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";

    fn insert_note_with_body(index: &SqliteIndex, id: &str, title: &str, body: &str) {
        index
            .conn()
            .execute(
                "INSERT INTO notes (id, path, title, body, created, modified, content_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    id,
                    format!("{}.md", id),
                    title,
                    body,
                    "2024-01-15T10:30:00Z",
                    "2024-01-15T10:30:00Z",
                    TEST_HASH
                ],
            )
            .unwrap();
    }

    fn insert_note_with_description(index: &SqliteIndex, id: &str, title: &str, description: &str) {
        index
            .conn()
            .execute(
                "INSERT INTO notes (id, path, title, description, created, modified, content_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    id,
                    format!("{}.md", id),
                    title,
                    description,
                    "2024-01-15T10:30:00Z",
                    "2024-01-15T10:30:00Z",
                    TEST_HASH
                ],
            )
            .unwrap();
    }

    fn insert_note_with_aliases(index: &SqliteIndex, id: &str, title: &str, aliases: &str) {
        index
            .conn()
            .execute(
                "INSERT INTO notes (id, path, title, aliases_text, created, modified, content_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    id,
                    format!("{}.md", id),
                    title,
                    aliases,
                    "2024-01-15T10:30:00Z",
                    "2024-01-15T10:30:00Z",
                    TEST_HASH
                ],
            )
            .unwrap();
    }

    // ===========================================
    // Phase 1: Empty Results Tests
    // ===========================================

    #[test]
    fn search_returns_empty_when_no_matches() {
        let index = SqliteIndex::open_in_memory().unwrap();
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Rust Guide",
            "Learn rust",
        );

        let results = index.search("nonexistent").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_empty_query_returns_empty() {
        let index = SqliteIndex::open_in_memory().unwrap();
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Rust Guide",
            "Learn rust",
        );

        let results = index.search("").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_whitespace_query_returns_empty() {
        let index = SqliteIndex::open_in_memory().unwrap();
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Rust Guide",
            "Learn rust",
        );

        let results = index.search("   \t\n  ").unwrap();
        assert!(results.is_empty());
    }

    // ===========================================
    // Phase 2: Basic Results Tests
    // ===========================================

    #[test]
    fn search_returns_single_match() {
        let index = SqliteIndex::open_in_memory().unwrap();
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Rust Guide",
            "Learn rust",
        );

        let results = index.search("rust").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_returns_multiple_matches() {
        let index = SqliteIndex::open_in_memory().unwrap();
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9A",
            "Rust Basics",
            "Learn rust",
        );
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9B",
            "Rust Advanced",
            "Advanced rust",
        );

        let results = index.search("rust").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_result_contains_complete_indexed_note() {
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let topic = Topic::new("programming").unwrap();
        let tag = Tag::new("tutorial").unwrap();

        let note = Note::builder(
            test_note_id(),
            "Rust Guide",
            test_datetime(),
            test_datetime(),
        )
        .description(Some("A comprehensive rust guide"))
        .topics(vec![topic.clone()])
        .tags(vec![tag.clone()])
        .build()
        .unwrap();
        index
            .upsert_note(&note, &test_content_hash(), &test_path())
            .unwrap();

        let results = index.search("rust").unwrap();
        assert_eq!(results.len(), 1);

        let indexed_note = results[0].note();
        assert_eq!(indexed_note.id(), note.id());
        assert_eq!(indexed_note.title(), "Rust Guide");
        assert_eq!(
            indexed_note.description(),
            Some("A comprehensive rust guide")
        );
        assert_eq!(indexed_note.topics(), &[topic]);
        assert_eq!(indexed_note.tags(), &[tag]);
    }

    // ===========================================
    // Phase 3: Ranking Tests
    // ===========================================

    #[test]
    fn search_results_ordered_by_relevance() {
        let index = SqliteIndex::open_in_memory().unwrap();
        // Note A: keyword in title
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9A",
            "rust",
            "something else",
        );
        // Note B: keyword in body only
        insert_note_with_body(&index, "01HQ3K5M7NXJK4QZPW8V2R6T9B", "other title", "rust");

        let results = index.search("rust").unwrap();
        assert_eq!(results.len(), 2);

        // Title match should rank higher (first)
        assert!(
            results[0].note().title() == "rust",
            "Title match should rank higher"
        );
    }

    #[test]
    fn search_rank_is_positive() {
        let index = SqliteIndex::open_in_memory().unwrap();
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Rust Guide",
            "Learn rust",
        );

        let results = index.search("rust").unwrap();
        assert_eq!(results.len(), 1);
        assert!(
            results[0].rank() > 0.0,
            "Rank should be positive (negated BM25), got {}",
            results[0].rank()
        );
    }

    #[test]
    fn search_title_ranks_higher_than_body() {
        let index = SqliteIndex::open_in_memory().unwrap();
        // Note A: keyword in title only
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9A",
            "rust",
            "other content",
        );
        // Note B: keyword in body only
        insert_note_with_body(&index, "01HQ3K5M7NXJK4QZPW8V2R6T9B", "other title", "rust");

        let results = index.search("rust").unwrap();
        assert_eq!(results.len(), 2);

        // Find title match and body match
        let title_result = results.iter().find(|r| r.note().title() == "rust").unwrap();
        let body_result = results
            .iter()
            .find(|r| r.note().title() == "other title")
            .unwrap();

        assert!(
            title_result.rank() > body_result.rank(),
            "Title match rank ({}) should be higher than body match rank ({})",
            title_result.rank(),
            body_result.rank()
        );
    }

    #[test]
    fn search_title_ranks_higher_than_description() {
        let index = SqliteIndex::open_in_memory().unwrap();
        // Note A: keyword in title only
        insert_note_with_description(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9A",
            "rust",
            "other content",
        );
        // Note B: keyword in description only
        insert_note_with_description(&index, "01HQ3K5M7NXJK4QZPW8V2R6T9B", "other title", "rust");

        let results = index.search("rust").unwrap();
        assert_eq!(results.len(), 2);

        // First result should be title match
        assert_eq!(
            results[0].note().title(),
            "rust",
            "Title match should rank first"
        );
        assert!(
            results[0].rank() > results[1].rank(),
            "Title match rank ({}) should be higher than description match rank ({})",
            results[0].rank(),
            results[1].rank()
        );
    }

    // ===========================================
    // Phase 4: Error Handling Tests
    // ===========================================

    #[test]
    fn search_invalid_fts_query_returns_error() {
        let index = SqliteIndex::open_in_memory().unwrap();
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Rust Guide",
            "Learn rust",
        );

        // FTS5 rejects queries that start with AND/OR operators
        let result = index.search("AND hello");

        // The query should fail due to FTS5 syntax error
        assert!(result.is_err(), "Invalid FTS query should return error");

        match result {
            Err(IndexError::InvalidQuery(msg)) => {
                assert!(!msg.is_empty(), "Error message should not be empty");
            }
            Err(IndexError::Database(e)) => {
                // FTS errors may come through as Database errors
                assert!(
                    !e.to_string().is_empty(),
                    "Database error should have message"
                );
            }
            Ok(_) => panic!("Expected error for invalid FTS query"),
            Err(e) => panic!("Unexpected error type: {:?}", e),
        }
    }

    #[test]
    fn search_special_characters_handled() {
        let index = SqliteIndex::open_in_memory().unwrap();
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Rust Guide",
            "Learn rust programming",
        );

        // Prefix search with * should work
        let results = index.search("prog*").unwrap();
        assert_eq!(results.len(), 1, "Prefix search with * should work");
    }

    // ===========================================
    // Phase 5: Snippets Tests
    // ===========================================

    #[test]
    fn search_result_includes_snippet() {
        let index = SqliteIndex::open_in_memory().unwrap();
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Guide",
            "Learn rust programming quickly",
        );

        let results = index.search("rust").unwrap();
        assert_eq!(results.len(), 1);
        assert!(
            results[0].snippet().is_some(),
            "Should have a snippet when body matches"
        );
    }

    #[test]
    fn search_snippet_contains_match_context() {
        let index = SqliteIndex::open_in_memory().unwrap();
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Guide",
            "Learn rust programming quickly and efficiently",
        );

        let results = index.search("rust").unwrap();
        assert_eq!(results.len(), 1);

        let snippet = results[0].snippet().unwrap();
        // FTS5 snippet should contain the match with markers
        assert!(
            snippet.contains("<b>") || snippet.contains("rust"),
            "Snippet should contain match context: {}",
            snippet
        );
    }

    // ===========================================
    // Phase 6: Edge Cases Tests
    // ===========================================

    #[test]
    fn search_unicode_terms() {
        let index = SqliteIndex::open_in_memory().unwrap();
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Café Guide",
            "Résumé preparation",
        );

        let results = index.search("café").unwrap();
        assert_eq!(results.len(), 1, "Should find unicode term");
    }

    #[test]
    fn search_is_case_insensitive() {
        let index = SqliteIndex::open_in_memory().unwrap();
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Rust Guide",
            "Learn Rust",
        );

        // Search with lowercase should find uppercase
        let results = index.search("rust").unwrap();
        assert_eq!(results.len(), 1, "Case-insensitive search should work");

        // Search with uppercase should find lowercase
        let results = index.search("RUST").unwrap();
        assert_eq!(results.len(), 1, "Case-insensitive search should work");
    }

    #[test]
    fn search_multiple_words() {
        let index = SqliteIndex::open_in_memory().unwrap();
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Rust Programming Guide",
            "Learn rust programming",
        );

        // Multiple words should match documents containing all terms
        let results = index.search("rust guide").unwrap();
        assert_eq!(results.len(), 1, "Multiple words search should work");
    }

    #[test]
    fn search_phrase_with_quotes() {
        let index = SqliteIndex::open_in_memory().unwrap();
        // Note A: has exact phrase
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9A",
            "Rust Programming",
            "about rust programming here",
        );
        // Note B: has words but not as phrase
        insert_note_with_body(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9B",
            "Programming in Rust",
            "rust is good. programming is fun.",
        );

        // Exact phrase search
        let results = index.search("\"rust programming\"").unwrap();
        assert!(
            results.len() >= 1,
            "Phrase search should find at least one result"
        );
        // The exact phrase match should be first or only
        assert!(
            results[0].note().title() == "Rust Programming"
                || results[0]
                    .snippet()
                    .map_or(false, |s| s.contains("rust programming")),
            "Phrase search should find exact phrase match"
        );
    }

    #[test]
    fn search_finds_by_alias() {
        let index = SqliteIndex::open_in_memory().unwrap();
        insert_note_with_aliases(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Main Title",
            "myalias otheralias",
        );

        let results = index.search("myalias").unwrap();
        assert_eq!(results.len(), 1, "Should find note by alias");
    }

    #[test]
    fn search_finds_by_description() {
        let index = SqliteIndex::open_in_memory().unwrap();
        insert_note_with_description(
            &index,
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Main Title",
            "searchable description here",
        );

        let results = index.search("searchable").unwrap();
        assert_eq!(results.len(), 1, "Should find note by description");
    }
}
