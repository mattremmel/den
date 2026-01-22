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
            body TEXT
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
    // Cycle 13: Schema Version Table
    // ===========================================
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )?;

    // Insert initial version if not exists
    conn.execute(
        "INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (1, datetime('now'))",
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
    fn schema_version_initialized_to_1() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        let version = get_schema_version(&conn).unwrap();
        assert_eq!(version, 1, "initial schema version should be 1");
    }

    #[test]
    fn schema_version_not_incremented_on_idempotent_call() {
        let conn = test_connection();
        create_schema(&conn).unwrap();
        create_schema(&conn).unwrap();
        create_schema(&conn).unwrap();

        let version = get_schema_version(&conn).unwrap();
        assert_eq!(version, 1, "schema version should remain 1");
    }

    #[test]
    fn get_schema_version_returns_max_version() {
        let conn = test_connection();
        create_schema(&conn).unwrap();

        // Manually insert a higher version (simulating migration)
        conn.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (2, datetime('now'))",
            [],
        )
        .unwrap();

        let version = get_schema_version(&conn).unwrap();
        assert_eq!(version, 2, "should return highest version");
    }
}
