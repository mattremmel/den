//! SQLite schema creation for the notes index.

use rusqlite::Connection;

// ===========================================
// Cycle 1: Schema Module Structure
// ===========================================

/// Creates the database schema for the notes index.
///
/// This function creates all required tables, indexes, and constraints.
/// It is idempotent - calling it multiple times is safe.
///
/// # Tables Created
/// - `notes` - Core note metadata
/// - `topics` - Hierarchical topic paths
/// - `note_topics` - Many-to-many junction for notes and topics
/// - `aliases` - Alternative names for notes
/// - `tags` - Flat tag names
/// - `note_tags` - Many-to-many junction for notes and tags
/// - `links` - Links between notes
/// - `link_rels` - Relationship types for links
/// - `schema_version` - Schema version tracking
pub fn create_schema(conn: &Connection) -> rusqlite::Result<()> {
    // ===========================================
    // Cycle 11: Foreign Key Enforcement
    // ===========================================
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    // ===========================================
    // Cycle 2: Notes Table
    // ===========================================
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS notes (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL,
            description TEXT,
            created TEXT NOT NULL,
            modified TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            body TEXT,
            aliases_text TEXT
        );",
    )?;

    // ===========================================
    // Cycle 3: Topics Table
    // ===========================================
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS topics (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE
        );",
    )?;

    // ===========================================
    // Cycle 4: Note-Topics Junction
    // ===========================================
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS note_topics (
            note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
            topic_id INTEGER NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
            PRIMARY KEY (note_id, topic_id)
        );",
    )?;

    // ===========================================
    // Cycle 5: Aliases Table
    // ===========================================
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS aliases (
            note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
            alias TEXT NOT NULL,
            PRIMARY KEY (note_id, alias)
        );",
    )?;

    // ===========================================
    // Cycle 6: Tags Table
    // ===========================================
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        );",
    )?;

    // ===========================================
    // Cycle 7: Note-Tags Junction
    // ===========================================
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS note_tags (
            note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
            tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY (note_id, tag_id)
        );",
    )?;

    // ===========================================
    // Cycle 8: Links Table
    // ===========================================
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS links (
            id INTEGER PRIMARY KEY,
            source_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
            target_id TEXT NOT NULL,
            note TEXT,
            UNIQUE(source_id, target_id)
        );",
    )?;

    // ===========================================
    // Cycle 9: Link-Rels Table
    // ===========================================
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS link_rels (
            link_id INTEGER NOT NULL REFERENCES links(id) ON DELETE CASCADE,
            rel TEXT NOT NULL,
            PRIMARY KEY (link_id, rel)
        );",
    )?;

    // ===========================================
    // Cycle 10: Indexes
    // ===========================================
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_topics_path ON topics(path);
         CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name);
         CREATE INDEX IF NOT EXISTS idx_notes_created ON notes(created);
         CREATE INDEX IF NOT EXISTS idx_notes_modified ON notes(modified);",
    )?;

    // ===========================================
    // FTS5 Cycle 1: FTS5 Virtual Table
    // ===========================================
    // Column names must match notes table for rebuild to work
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
            title,
            description,
            aliases_text,
            body,
            content='notes',
            content_rowid='rowid'
        );",
    )?;

    // ===========================================
    // FTS5 Cycle 8: INSERT Trigger
    // ===========================================
    conn.execute_batch(
        "CREATE TRIGGER IF NOT EXISTS notes_fts_insert
        AFTER INSERT ON notes BEGIN
            INSERT INTO notes_fts(rowid, title, description, aliases_text, body)
            VALUES (NEW.rowid, NEW.title, COALESCE(NEW.description, ''), COALESCE(NEW.aliases_text, ''), COALESCE(NEW.body, ''));
        END;",
    )?;

    // ===========================================
    // FTS5 Cycle 9: DELETE Trigger
    // ===========================================
    conn.execute_batch(
        "CREATE TRIGGER IF NOT EXISTS notes_fts_delete
        AFTER DELETE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, title, description, aliases_text, body)
            VALUES ('delete', OLD.rowid, OLD.title, COALESCE(OLD.description, ''), COALESCE(OLD.aliases_text, ''), COALESCE(OLD.body, ''));
        END;",
    )?;

    // ===========================================
    // FTS5 Cycle 10: UPDATE Trigger
    // ===========================================
    conn.execute_batch(
        "CREATE TRIGGER IF NOT EXISTS notes_fts_update
        AFTER UPDATE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, title, description, aliases_text, body)
            VALUES ('delete', OLD.rowid, OLD.title, COALESCE(OLD.description, ''), COALESCE(OLD.aliases_text, ''), COALESCE(OLD.body, ''));
            INSERT INTO notes_fts(rowid, title, description, aliases_text, body)
            VALUES (NEW.rowid, NEW.title, COALESCE(NEW.description, ''), COALESCE(NEW.aliases_text, ''), COALESCE(NEW.body, ''));
        END;",
    )?;

    // ===========================================
    // Cycle 13: Schema Version Table
    // ===========================================
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )?;

    // Insert initial version if not exists (version 2 includes FTS5)
    conn.execute(
        "INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (2, datetime('now'))",
        [],
    )?;

    Ok(())
}

/// Returns the current schema version.
pub fn get_schema_version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT MAX(version) FROM schema_version", [], |row| {
        row.get(0)
    })
}

/// Rebuilds the FTS5 index from the notes table.
///
/// This is useful for recovering from index corruption or after
/// bulk imports that bypass the triggers.
pub fn rebuild_fts(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("INSERT INTO notes_fts(notes_fts) VALUES('rebuild')", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===========================================
    // Test Helpers
    // ===========================================

    fn test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        conn
    }

    fn table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?",
            [name],
            |_| Ok(()),
        )
        .is_ok()
    }

    fn index_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT 1 FROM sqlite_master WHERE type='index' AND name=?",
            [name],
            |_| Ok(()),
        )
        .is_ok()
    }

    fn get_columns(conn: &Connection, table: &str) -> Vec<(String, String, bool)> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({})", table))
            .unwrap();
        stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,   // name
                row.get::<_, String>(2)?,   // type
                row.get::<_, i32>(3)? != 0, // notnull
            ))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    // ===========================================
    // Cycle 1: Schema Module Structure
    // ===========================================

    #[test]
    fn create_schema_returns_ok() {
        let conn = test_connection();
        let result = create_schema(&conn);
        assert!(result.is_ok(), "create_schema should return Ok");
    }

    // ===========================================
    // Cycle 2: Notes Table
    // ===========================================

    #[test]
    fn notes_table_created() {
        let conn = test_connection();
        create_schema(&conn).unwrap();
        assert!(table_exists(&conn, "notes"), "notes table should exist");
    }

    #[test]
    fn notes_table_has_required_columns() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let columns = get_columns(&conn, "notes");
        let column_names: Vec<&str> = columns.iter().map(|(n, _, _)| n.as_str()).collect();

        assert!(column_names.contains(&"id"), "should have id column");
        assert!(column_names.contains(&"path"), "should have path column");
        assert!(column_names.contains(&"title"), "should have title column");
        assert!(
            column_names.contains(&"description"),
            "should have description column"
        );
        assert!(
            column_names.contains(&"created"),
            "should have created column"
        );
        assert!(
            column_names.contains(&"modified"),
            "should have modified column"
        );
        assert!(
            column_names.contains(&"content_hash"),
            "should have content_hash column"
        );
        assert!(column_names.contains(&"body"), "should have body column");
    }

    #[test]
    fn notes_table_accepts_valid_row() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let result = conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        );
        assert!(result.is_ok(), "should accept valid note row");
    }

    #[test]
    fn notes_table_enforces_unique_path() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Z",
                "test.md", // duplicate path
                "Title 2",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "def456",
            ],
        );
        assert!(result.is_err(), "should reject duplicate path");
    }

    #[test]
    fn notes_table_allows_null_description_and_body() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let result = conn.execute(
            "INSERT INTO notes (id, path, title, description, created, modified, content_hash, body)
             VALUES (?, ?, ?, NULL, ?, ?, ?, NULL)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        );
        assert!(result.is_ok(), "should accept NULL description and body");
    }

    // ===========================================
    // Cycle 3: Topics Table
    // ===========================================

    #[test]
    fn topics_table_created() {
        let conn = test_connection();
        create_schema(&conn).unwrap();
        assert!(table_exists(&conn, "topics"), "topics table should exist");
    }

    #[test]
    fn topics_table_has_correct_columns() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let columns = get_columns(&conn, "topics");
        let column_names: Vec<&str> = columns.iter().map(|(n, _, _)| n.as_str()).collect();

        assert!(column_names.contains(&"id"), "should have id column");
        assert!(column_names.contains(&"path"), "should have path column");
    }

    #[test]
    fn topics_table_accepts_valid_row() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let result = conn.execute("INSERT INTO topics (path) VALUES (?)", ["software/rust"]);
        assert!(result.is_ok(), "should accept valid topic");
    }

    #[test]
    fn topics_table_enforces_unique_path() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute("INSERT INTO topics (path) VALUES (?)", ["software/rust"])
            .unwrap();

        let result = conn.execute("INSERT INTO topics (path) VALUES (?)", ["software/rust"]);
        assert!(result.is_err(), "should reject duplicate topic path");
    }

    #[test]
    fn topics_table_autoincrement_id() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute("INSERT INTO topics (path) VALUES (?)", ["topic1"])
            .unwrap();
        conn.execute("INSERT INTO topics (path) VALUES (?)", ["topic2"])
            .unwrap();

        let ids: Vec<i64> = conn
            .prepare("SELECT id FROM topics ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(ids.len(), 2);
        assert!(ids[0] < ids[1], "IDs should be auto-incrementing");
    }

    // ===========================================
    // Cycle 4: Note-Topics Junction
    // ===========================================

    #[test]
    fn note_topics_table_created() {
        let conn = test_connection();
        create_schema(&conn).unwrap();
        assert!(
            table_exists(&conn, "note_topics"),
            "note_topics table should exist"
        );
    }

    #[test]
    fn note_topics_table_has_correct_columns() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let columns = get_columns(&conn, "note_topics");
        let column_names: Vec<&str> = columns.iter().map(|(n, _, _)| n.as_str()).collect();

        assert!(
            column_names.contains(&"note_id"),
            "should have note_id column"
        );
        assert!(
            column_names.contains(&"topic_id"),
            "should have topic_id column"
        );
    }

    #[test]
    fn note_topics_accepts_valid_junction() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Insert prerequisite data
        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO topics (id, path) VALUES (1, ?)",
            ["software/rust"],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO note_topics (note_id, topic_id) VALUES (?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "1"],
        );
        assert!(result.is_ok(), "should accept valid junction");
    }

    #[test]
    fn note_topics_enforces_composite_primary_key() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Insert prerequisite data
        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO topics (id, path) VALUES (1, ?)",
            ["software/rust"],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO note_topics (note_id, topic_id) VALUES (?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "1"],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO note_topics (note_id, topic_id) VALUES (?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "1"], // duplicate
        );
        assert!(result.is_err(), "should reject duplicate junction");
    }

    // ===========================================
    // Cycle 5: Aliases Table
    // ===========================================

    #[test]
    fn aliases_table_created() {
        let conn = test_connection();
        create_schema(&conn).unwrap();
        assert!(table_exists(&conn, "aliases"), "aliases table should exist");
    }

    #[test]
    fn aliases_table_has_correct_columns() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let columns = get_columns(&conn, "aliases");
        let column_names: Vec<&str> = columns.iter().map(|(n, _, _)| n.as_str()).collect();

        assert!(
            column_names.contains(&"note_id"),
            "should have note_id column"
        );
        assert!(column_names.contains(&"alias"), "should have alias column");
    }

    #[test]
    fn aliases_accepts_valid_row() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO aliases (note_id, alias) VALUES (?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "my-alias"],
        );
        assert!(result.is_ok(), "should accept valid alias");
    }

    #[test]
    fn aliases_enforces_composite_primary_key() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO aliases (note_id, alias) VALUES (?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "my-alias"],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO aliases (note_id, alias) VALUES (?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "my-alias"], // duplicate
        );
        assert!(result.is_err(), "should reject duplicate alias");
    }

    #[test]
    fn aliases_allows_same_alias_for_different_notes() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test1.md",
                "Title 1",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Z",
                "test2.md",
                "Title 2",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "def456",
            ],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO aliases (note_id, alias) VALUES (?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "shared-alias"],
        )
        .unwrap();

        // Same alias, different note
        let result = conn.execute(
            "INSERT INTO aliases (note_id, alias) VALUES (?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Z", "shared-alias"],
        );
        assert!(
            result.is_ok(),
            "should allow same alias for different notes"
        );
    }

    // ===========================================
    // Cycle 6: Tags Table
    // ===========================================

    #[test]
    fn tags_table_created() {
        let conn = test_connection();
        create_schema(&conn).unwrap();
        assert!(table_exists(&conn, "tags"), "tags table should exist");
    }

    #[test]
    fn tags_table_has_correct_columns() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let columns = get_columns(&conn, "tags");
        let column_names: Vec<&str> = columns.iter().map(|(n, _, _)| n.as_str()).collect();

        assert!(column_names.contains(&"id"), "should have id column");
        assert!(column_names.contains(&"name"), "should have name column");
    }

    #[test]
    fn tags_accepts_valid_row() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let result = conn.execute("INSERT INTO tags (name) VALUES (?)", ["draft"]);
        assert!(result.is_ok(), "should accept valid tag");
    }

    #[test]
    fn tags_enforces_unique_name() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute("INSERT INTO tags (name) VALUES (?)", ["draft"])
            .unwrap();

        let result = conn.execute("INSERT INTO tags (name) VALUES (?)", ["draft"]);
        assert!(result.is_err(), "should reject duplicate tag name");
    }

    // ===========================================
    // Cycle 7: Note-Tags Junction
    // ===========================================

    #[test]
    fn note_tags_table_created() {
        let conn = test_connection();
        create_schema(&conn).unwrap();
        assert!(
            table_exists(&conn, "note_tags"),
            "note_tags table should exist"
        );
    }

    #[test]
    fn note_tags_table_has_correct_columns() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let columns = get_columns(&conn, "note_tags");
        let column_names: Vec<&str> = columns.iter().map(|(n, _, _)| n.as_str()).collect();

        assert!(
            column_names.contains(&"note_id"),
            "should have note_id column"
        );
        assert!(
            column_names.contains(&"tag_id"),
            "should have tag_id column"
        );
    }

    #[test]
    fn note_tags_accepts_valid_junction() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();
        conn.execute("INSERT INTO tags (id, name) VALUES (1, ?)", ["draft"])
            .unwrap();

        let result = conn.execute(
            "INSERT INTO note_tags (note_id, tag_id) VALUES (?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "1"],
        );
        assert!(result.is_ok(), "should accept valid junction");
    }

    #[test]
    fn note_tags_enforces_composite_primary_key() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();
        conn.execute("INSERT INTO tags (id, name) VALUES (1, ?)", ["draft"])
            .unwrap();

        conn.execute(
            "INSERT INTO note_tags (note_id, tag_id) VALUES (?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "1"],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO note_tags (note_id, tag_id) VALUES (?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "1"], // duplicate
        );
        assert!(result.is_err(), "should reject duplicate junction");
    }

    // ===========================================
    // Cycle 8: Links Table
    // ===========================================

    #[test]
    fn links_table_created() {
        let conn = test_connection();
        create_schema(&conn).unwrap();
        assert!(table_exists(&conn, "links"), "links table should exist");
    }

    #[test]
    fn links_table_has_correct_columns() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let columns = get_columns(&conn, "links");
        let column_names: Vec<&str> = columns.iter().map(|(n, _, _)| n.as_str()).collect();

        assert!(column_names.contains(&"id"), "should have id column");
        assert!(
            column_names.contains(&"source_id"),
            "should have source_id column"
        );
        assert!(
            column_names.contains(&"target_id"),
            "should have target_id column"
        );
        assert!(column_names.contains(&"note"), "should have note column");
    }

    #[test]
    fn links_accepts_valid_row() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO links (source_id, target_id, note) VALUES (?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "01HQ3K5M7NXJK4QZPW8V2R6T9Z",
                "Related note",
            ],
        );
        assert!(result.is_ok(), "should accept valid link");
    }

    #[test]
    fn links_enforces_unique_source_target() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO links (source_id, target_id) VALUES (?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "01HQ3K5M7NXJK4QZPW8V2R6T9Z"],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO links (source_id, target_id) VALUES (?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "01HQ3K5M7NXJK4QZPW8V2R6T9Z"],
        );
        assert!(
            result.is_err(),
            "should reject duplicate source-target pair"
        );
    }

    #[test]
    fn links_allows_null_note() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO links (source_id, target_id, note) VALUES (?, ?, NULL)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "01HQ3K5M7NXJK4QZPW8V2R6T9Z"],
        );
        assert!(result.is_ok(), "should accept NULL note");
    }

    // ===========================================
    // Cycle 9: Link-Rels Table
    // ===========================================

    #[test]
    fn link_rels_table_created() {
        let conn = test_connection();
        create_schema(&conn).unwrap();
        assert!(
            table_exists(&conn, "link_rels"),
            "link_rels table should exist"
        );
    }

    #[test]
    fn link_rels_table_has_correct_columns() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let columns = get_columns(&conn, "link_rels");
        let column_names: Vec<&str> = columns.iter().map(|(n, _, _)| n.as_str()).collect();

        assert!(
            column_names.contains(&"link_id"),
            "should have link_id column"
        );
        assert!(column_names.contains(&"rel"), "should have rel column");
    }

    #[test]
    fn link_rels_accepts_valid_row() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO links (id, source_id, target_id) VALUES (1, ?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "01HQ3K5M7NXJK4QZPW8V2R6T9Z"],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO link_rels (link_id, rel) VALUES (?, ?)",
            ["1", "implements"],
        );
        assert!(result.is_ok(), "should accept valid link_rel");
    }

    #[test]
    fn link_rels_enforces_composite_primary_key() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO links (id, source_id, target_id) VALUES (1, ?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "01HQ3K5M7NXJK4QZPW8V2R6T9Z"],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO link_rels (link_id, rel) VALUES (?, ?)",
            ["1", "implements"],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO link_rels (link_id, rel) VALUES (?, ?)",
            ["1", "implements"], // duplicate
        );
        assert!(result.is_err(), "should reject duplicate link_rel");
    }

    #[test]
    fn link_rels_allows_multiple_rels_per_link() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO links (id, source_id, target_id) VALUES (1, ?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "01HQ3K5M7NXJK4QZPW8V2R6T9Z"],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO link_rels (link_id, rel) VALUES (?, ?)",
            ["1", "implements"],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO link_rels (link_id, rel) VALUES (?, ?)",
            ["1", "extends"], // different rel, same link
        );
        assert!(result.is_ok(), "should allow multiple rels per link");
    }

    // ===========================================
    // Cycle 10: Indexes
    // ===========================================

    #[test]
    fn idx_topics_path_created() {
        let conn = test_connection();
        create_schema(&conn).unwrap();
        assert!(
            index_exists(&conn, "idx_topics_path"),
            "idx_topics_path should exist"
        );
    }

    #[test]
    fn idx_tags_name_created() {
        let conn = test_connection();
        create_schema(&conn).unwrap();
        assert!(
            index_exists(&conn, "idx_tags_name"),
            "idx_tags_name should exist"
        );
    }

    #[test]
    fn idx_notes_created_created() {
        let conn = test_connection();
        create_schema(&conn).unwrap();
        assert!(
            index_exists(&conn, "idx_notes_created"),
            "idx_notes_created should exist"
        );
    }

    #[test]
    fn idx_notes_modified_created() {
        let conn = test_connection();
        create_schema(&conn).unwrap();
        assert!(
            index_exists(&conn, "idx_notes_modified"),
            "idx_notes_modified should exist"
        );
    }

    // ===========================================
    // Cycle 11: Foreign Key Enforcement
    // ===========================================

    #[test]
    fn foreign_keys_enabled() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let fk_enabled: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fk_enabled, 1, "foreign keys should be enabled");
    }

    #[test]
    fn note_topics_fk_enforced() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Try to insert note_topics with non-existent note_id
        let result = conn.execute(
            "INSERT INTO note_topics (note_id, topic_id) VALUES (?, ?)",
            ["nonexistent", "1"],
        );
        assert!(result.is_err(), "should reject invalid note_id FK");
    }

    #[test]
    fn note_tags_fk_enforced() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Insert a valid note
        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        // Try to insert note_tags with non-existent tag_id
        let result = conn.execute(
            "INSERT INTO note_tags (note_id, tag_id) VALUES (?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "999"],
        );
        assert!(result.is_err(), "should reject invalid tag_id FK");
    }

    #[test]
    fn aliases_fk_enforced() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Try to insert alias with non-existent note_id
        let result = conn.execute(
            "INSERT INTO aliases (note_id, alias) VALUES (?, ?)",
            ["nonexistent", "my-alias"],
        );
        assert!(
            result.is_err(),
            "should reject invalid note_id FK in aliases"
        );
    }

    #[test]
    fn links_fk_enforced() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Try to insert link with non-existent source_id
        let result = conn.execute(
            "INSERT INTO links (source_id, target_id) VALUES (?, ?)",
            ["nonexistent", "01HQ3K5M7NXJK4QZPW8V2R6T9Z"],
        );
        assert!(result.is_err(), "should reject invalid source_id FK");
    }

    #[test]
    fn link_rels_fk_enforced() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Try to insert link_rel with non-existent link_id
        let result = conn.execute(
            "INSERT INTO link_rels (link_id, rel) VALUES (?, ?)",
            ["999", "implements"],
        );
        assert!(result.is_err(), "should reject invalid link_id FK");
    }

    #[test]
    fn cascade_delete_note_removes_topics() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Insert note and topic junction
        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();
        conn.execute("INSERT INTO topics (id, path) VALUES (1, ?)", ["software"])
            .unwrap();
        conn.execute(
            "INSERT INTO note_topics (note_id, topic_id) VALUES (?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "1"],
        )
        .unwrap();

        // Delete the note
        conn.execute(
            "DELETE FROM notes WHERE id = ?",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y"],
        )
        .unwrap();

        // Verify junction was cascaded
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM note_topics", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "note_topics should be empty after cascade delete");
    }

    #[test]
    fn cascade_delete_note_removes_aliases() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO aliases (note_id, alias) VALUES (?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "my-alias"],
        )
        .unwrap();

        conn.execute(
            "DELETE FROM notes WHERE id = ?",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y"],
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM aliases", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "aliases should be empty after cascade delete");
    }

    #[test]
    fn cascade_delete_link_removes_rels() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO links (id, source_id, target_id) VALUES (1, ?, ?)",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y", "01HQ3K5M7NXJK4QZPW8V2R6T9Z"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO link_rels (link_id, rel) VALUES (?, ?)",
            ["1", "implements"],
        )
        .unwrap();

        conn.execute("DELETE FROM links WHERE id = 1", []).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM link_rels", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "link_rels should be empty after cascade delete");
    }

    // ===========================================
    // Cycle 12: Idempotent Schema Creation
    // ===========================================

    #[test]
    fn create_schema_is_idempotent() {
        let conn = test_connection();

        // Call create_schema multiple times
        create_schema(&conn).unwrap();
        create_schema(&conn).unwrap();
        create_schema(&conn).unwrap();

        // Verify tables still exist
        assert!(table_exists(&conn, "notes"));
        assert!(table_exists(&conn, "topics"));
        assert!(table_exists(&conn, "note_topics"));
        assert!(table_exists(&conn, "aliases"));
        assert!(table_exists(&conn, "tags"));
        assert!(table_exists(&conn, "note_tags"));
        assert!(table_exists(&conn, "links"));
        assert!(table_exists(&conn, "link_rels"));
        assert!(table_exists(&conn, "schema_version"));
    }

    #[test]
    fn create_schema_preserves_existing_data() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Insert some data
        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        // Call create_schema again
        create_schema(&conn).unwrap();

        // Verify data still exists
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "existing data should be preserved");
    }

    // ===========================================
    // Cycle 13: Schema Version Table
    // ===========================================

    #[test]
    fn schema_version_table_created() {
        let conn = test_connection();
        create_schema(&conn).unwrap();
        assert!(
            table_exists(&conn, "schema_version"),
            "schema_version table should exist"
        );
    }

    #[test]
    fn schema_version_initialized_to_2() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let version = get_schema_version(&conn).unwrap();
        assert_eq!(version, 2, "initial schema version should be 2 (with FTS)");
    }

    #[test]
    fn schema_version_not_incremented_on_idempotent_call() {
        let conn = test_connection();
        create_schema(&conn).unwrap();
        create_schema(&conn).unwrap();
        create_schema(&conn).unwrap();

        let version = get_schema_version(&conn).unwrap();
        assert_eq!(version, 2, "schema version should remain 2");
    }

    #[test]
    fn get_schema_version_returns_max_version() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Manually insert a higher version (simulating migration)
        conn.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (3, datetime('now'))",
            [],
        )
        .unwrap();

        let version = get_schema_version(&conn).unwrap();
        assert_eq!(version, 3, "should return highest version");
    }

    // ===========================================
    // FTS5 Cycle 1: FTS5 Table Creation
    // ===========================================

    fn fts_table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=? AND sql LIKE '%fts5%'",
            [name],
            |_| Ok(()),
        )
        .is_ok()
    }

    #[test]
    fn fts_table_created() {
        let conn = test_connection();
        create_schema(&conn).unwrap();
        assert!(
            fts_table_exists(&conn, "notes_fts"),
            "notes_fts virtual table should exist"
        );
    }

    // ===========================================
    // FTS5 Cycle 2: FTS5 Table Structure
    // ===========================================

    #[test]
    fn fts_table_has_expected_columns() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // FTS5 tables can be queried for their columns
        let result: Result<i32, _> = conn.query_row(
            "SELECT 1 FROM notes_fts WHERE title IS NULL AND description IS NULL AND aliases_text IS NULL AND body IS NULL LIMIT 0",
            [],
            |row| row.get(0),
        );
        // The query should succeed (even if empty) if columns exist
        assert!(
            result.is_ok() || result.unwrap_err().to_string().contains("no rows"),
            "FTS table should have expected columns"
        );
    }

    // ===========================================
    // FTS5 Cycle 3: Schema Idempotency with FTS
    // ===========================================

    #[test]
    fn create_schema_with_fts_is_idempotent() {
        let conn = test_connection();

        create_schema(&conn).unwrap();
        create_schema(&conn).unwrap();
        create_schema(&conn).unwrap();

        assert!(
            fts_table_exists(&conn, "notes_fts"),
            "notes_fts should exist after multiple create_schema calls"
        );

        // Verify only one FTS table exists
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name='notes_fts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "should have exactly one notes_fts table");
    }

    // ===========================================
    // FTS5 Cycle 4: Manual FTS Insert
    // ===========================================

    #[test]
    fn fts_accepts_direct_insert() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // First insert a note to get a rowid
        conn.execute(
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

        // The trigger should have inserted into FTS automatically
        // Verify by searching
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'Test'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "FTS should contain the inserted note");
    }

    // ===========================================
    // FTS5 Cycle 5: FTS Search Returns Results
    // ===========================================

    #[test]
    fn fts_search_finds_matching_title() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Rust Programming Guide",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        let title: String = conn
            .query_row(
                "SELECT title FROM notes_fts WHERE notes_fts MATCH 'Rust'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(title, "Rust Programming Guide");
    }

    // ===========================================
    // FTS5 Cycle 6: FTS Search with bm25 Ranking
    // ===========================================

    #[test]
    fn fts_search_returns_bm25_rank() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, body, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Rust Guide",
                "rust rust rust rust rust",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        let score: f64 = conn
            .query_row(
                "SELECT bm25(notes_fts) FROM notes_fts WHERE notes_fts MATCH 'rust'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        // BM25 scores are negative in FTS5 (lower/more negative = better match)
        assert!(score < 0.0, "bm25 score should be negative, got {}", score);
    }

    // ===========================================
    // FTS5 Cycle 7: Weighted bm25 Search
    // ===========================================

    #[test]
    fn fts_search_weighted_title_ranks_higher() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Note A: keyword in title only
        conn.execute(
            "INSERT INTO notes (id, path, title, body, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9A",
                "a.md",
                "rust",
                "something else entirely",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        // Note B: keyword in body only
        conn.execute(
            "INSERT INTO notes (id, path, title, body, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9B",
                "b.md",
                "something else",
                "rust",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "def456",
            ],
        )
        .unwrap();

        // Query with weighted bm25: title=10, description=5, aliases=5, body=1
        let results: Vec<(String, f64)> = conn
            .prepare(
                "SELECT n.id, bm25(notes_fts, 10.0, 5.0, 5.0, 1.0) as score
                 FROM notes_fts
                 JOIN notes n ON notes_fts.rowid = n.rowid
                 WHERE notes_fts MATCH 'rust'
                 ORDER BY score",
            )
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(results.len(), 2);
        // First result (better score) should be note A (title match)
        assert_eq!(
            results[0].0, "01HQ3K5M7NXJK4QZPW8V2R6T9A",
            "Title match should rank higher"
        );
        // Verify the score is actually better (more negative)
        assert!(
            results[0].1 < results[1].1,
            "Title match score {} should be better (more negative) than body match score {}",
            results[0].1,
            results[1].1
        );
    }

    // ===========================================
    // FTS5 Cycle 8: INSERT Trigger
    // ===========================================

    #[test]
    fn fts_insert_trigger_syncs_on_note_insert() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Insert into notes table
        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Trigger Test Note",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        // FTS should be populated via trigger
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'Trigger'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "FTS should be populated by INSERT trigger");
    }

    // ===========================================
    // FTS5 Cycle 9: DELETE Trigger
    // ===========================================

    #[test]
    fn fts_delete_trigger_syncs_on_note_delete() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Delete Test Note",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        // Verify it's in FTS
        let count_before: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'Delete'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count_before, 1);

        // Delete the note
        conn.execute(
            "DELETE FROM notes WHERE id = ?",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y"],
        )
        .unwrap();

        // FTS should be cleaned up via trigger
        let count_after: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'Delete'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count_after, 0, "FTS should be cleaned up by DELETE trigger");
    }

    // ===========================================
    // FTS5 Cycle 10: UPDATE Trigger
    // ===========================================

    #[test]
    fn fts_update_trigger_syncs_on_note_update() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Original Title",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        // Update the title
        conn.execute(
            "UPDATE notes SET title = 'Modified Title' WHERE id = ?",
            ["01HQ3K5M7NXJK4QZPW8V2R6T9Y"],
        )
        .unwrap();

        // Search for new title should find it
        let count_new: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'Modified'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count_new, 1, "FTS should find updated title");

        // Search for old title should NOT find it
        let count_old: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'Original'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count_old, 0, "FTS should not find old title after update");
    }

    // ===========================================
    // FTS5 Cycle 11: Aliases Integration
    // ===========================================

    #[test]
    fn fts_search_finds_by_alias() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Insert note with aliases_text
        conn.execute(
            "INSERT INTO notes (id, path, title, aliases_text, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Main Title",
                "myalias otheralias",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        // Search by alias
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'myalias'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "FTS should find note by alias");
    }

    // ===========================================
    // FTS5 Cycle 12: Aliases Column Migration
    // ===========================================

    #[test]
    fn notes_table_has_aliases_text_column() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let columns = get_columns(&conn, "notes");
        let column_names: Vec<&str> = columns.iter().map(|(n, _, _)| n.as_str()).collect();

        assert!(
            column_names.contains(&"aliases_text"),
            "notes table should have aliases_text column"
        );
    }

    // ===========================================
    // FTS5 Cycle 13: Full Integration Test
    // ===========================================

    #[test]
    fn fts_weighted_search_full_integration() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Note A: "rust" in title only
        conn.execute(
            "INSERT INTO notes (id, path, title, description, body, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9A",
                "a.md",
                "rust programming",
                "a guide",
                "learn to code",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        // Note B: "rust" in description only
        conn.execute(
            "INSERT INTO notes (id, path, title, description, body, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9B",
                "b.md",
                "programming guide",
                "rust language overview",
                "learn to code",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "def456",
            ],
        )
        .unwrap();

        // Note C: "rust" in body only
        conn.execute(
            "INSERT INTO notes (id, path, title, description, body, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9C",
                "c.md",
                "coding guide",
                "learn programming",
                "rust is great",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "ghi789",
            ],
        )
        .unwrap();

        // Query with weighted bm25: title=10, description=5, aliases=5, body=1
        let results: Vec<String> = conn
            .prepare(
                "SELECT n.id
                 FROM notes_fts
                 JOIN notes n ON notes_fts.rowid = n.rowid
                 WHERE notes_fts MATCH 'rust'
                 ORDER BY bm25(notes_fts, 10.0, 5.0, 5.0, 1.0)",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(results.len(), 3);
        assert_eq!(
            results[0], "01HQ3K5M7NXJK4QZPW8V2R6T9A",
            "Title match should rank first"
        );
        assert_eq!(
            results[1], "01HQ3K5M7NXJK4QZPW8V2R6T9B",
            "Description match should rank second"
        );
        assert_eq!(
            results[2], "01HQ3K5M7NXJK4QZPW8V2R6T9C",
            "Body match should rank third"
        );
    }

    // ===========================================
    // FTS5 Cycle 14: Schema Version Bump
    // ===========================================

    #[test]
    fn schema_version_is_2_with_fts() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let version = get_schema_version(&conn).unwrap();
        assert_eq!(version, 2, "schema version should be 2 with FTS");
    }

    // ===========================================
    // FTS5 Cycle 15: FTS Rebuild Command
    // ===========================================

    #[test]
    fn fts_rebuild_repopulates_index() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Insert a note
        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Rebuild Test Note",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        // Manually corrupt FTS by deleting directly (bypassing triggers)
        conn.execute(
            "INSERT INTO notes_fts(notes_fts, rowid, title, description, aliases_text, body)
             VALUES ('delete', (SELECT rowid FROM notes WHERE id = '01HQ3K5M7NXJK4QZPW8V2R6T9Y'), 'Rebuild Test Note', '', '', '')",
            [],
        )
        .unwrap();

        // Verify it's gone
        let count_before: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'Rebuild'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count_before, 0, "FTS should be empty after manual delete");

        // Rebuild
        super::rebuild_fts(&conn).unwrap();

        // Verify it's back
        let count_after: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'Rebuild'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count_after, 1, "FTS should be repopulated after rebuild");
    }

    // ===========================================
    // FTS5 Edge Cases
    // ===========================================

    #[test]
    fn fts_handles_null_description_and_body() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Insert note with NULL description and body
        conn.execute(
            "INSERT INTO notes (id, path, title, description, body, created, modified, content_hash)
             VALUES (?, ?, ?, NULL, NULL, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Null Test",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        // Should still be searchable by title
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'Null'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "FTS should handle NULL fields");
    }

    #[test]
    fn fts_handles_unicode() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // FTS5's default tokenizer handles Unicode latin characters and basic symbols
        // CJK characters require special tokenizers, so we test with accented characters
        conn.execute(
            "INSERT INTO notes (id, path, title, body, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Café résumé naïve",
                "Ñoño señor jalapeño",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        // Search for accented text - FTS5 unicode61 tokenizer handles this
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'café'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "FTS should handle Unicode accented characters");
    }

    #[test]
    fn fts_empty_search_returns_no_results() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO notes (id, path, title, created, modified, content_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
                "test.md",
                "Test Note",
                "2024-01-15T10:30:00Z",
                "2024-01-15T10:30:00Z",
                "abc123",
            ],
        )
        .unwrap();

        // Search for something that doesn't exist
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'nonexistent'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "Search for nonexistent term should return 0");
    }
}
