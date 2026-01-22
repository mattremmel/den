use super::*;
use crate::index::IndexError;
use std::fs;
use std::path::PathBuf;
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

use crate::domain::{Link, Note, NoteId, Rel, Tag, Topic};
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

fn sample_note(title: &str) -> Note {
    Note::new(test_note_id(), title, test_datetime(), test_datetime()).unwrap()
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
// all_rels Tests
// ===========================================

#[test]
fn all_rels_returns_empty_when_no_links() {
    let index = SqliteIndex::open_in_memory().unwrap();
    let result = index.all_rels().unwrap();
    assert!(result.is_empty());
}

#[test]
fn all_rels_returns_single_rel_with_count() {
    let mut index = SqliteIndex::open_in_memory().unwrap();

    let target_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9B".parse().unwrap();
    let link = Link::new(target_id, vec!["parent"]).unwrap();
    let note = Note::builder(test_note_id(), "Test", test_datetime(), test_datetime())
        .links(vec![link])
        .build()
        .unwrap();
    index
        .upsert_note(&note, &test_content_hash(), &test_path())
        .unwrap();

    let result = index.all_rels().unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].rel().as_str(), "parent");
    assert_eq!(result[0].count(), 1);
}

#[test]
fn all_rels_counts_each_link_once_per_rel() {
    let mut index = SqliteIndex::open_in_memory().unwrap();

    // A link with multiple rels counts once for each rel type
    let target_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9B".parse().unwrap();
    let link = Link::new(target_id, vec!["parent", "see-also"]).unwrap();
    let note = Note::builder(test_note_id(), "Test", test_datetime(), test_datetime())
        .links(vec![link])
        .build()
        .unwrap();
    index
        .upsert_note(&note, &test_content_hash(), &test_path())
        .unwrap();

    let result = index.all_rels().unwrap();
    assert_eq!(result.len(), 2);
    // Each rel counts once
    assert_eq!(result[0].count(), 1);
    assert_eq!(result[1].count(), 1);
}

#[test]
fn all_rels_counts_multiple_links_per_rel() {
    let mut index = SqliteIndex::open_in_memory().unwrap();

    // Two notes, each with a link using "parent" rel
    let target_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9B".parse().unwrap();
    let link1 = Link::new(target_id.clone(), vec!["parent"]).unwrap();
    let note1 = Note::builder(test_note_id(), "Note 1", test_datetime(), test_datetime())
        .links(vec![link1])
        .build()
        .unwrap();
    index
        .upsert_note(&note1, &test_content_hash(), &test_path())
        .unwrap();

    let other_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9C".parse().unwrap();
    let link2 = Link::new(target_id, vec!["parent"]).unwrap();
    let note2 = Note::builder(other_id, "Note 2", test_datetime(), test_datetime())
        .links(vec![link2])
        .build()
        .unwrap();
    index
        .upsert_note(
            &note2,
            &test_content_hash(),
            &PathBuf::from("notes/other.md"),
        )
        .unwrap();

    let result = index.all_rels().unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].rel().as_str(), "parent");
    assert_eq!(result[0].count(), 2);
}

#[test]
fn all_rels_returns_rels_sorted_alphabetically() {
    let mut index = SqliteIndex::open_in_memory().unwrap();

    let target_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9B".parse().unwrap();
    let link = Link::new(target_id, vec!["see-also", "parent", "child"]).unwrap();
    let note = Note::builder(test_note_id(), "Test", test_datetime(), test_datetime())
        .links(vec![link])
        .build()
        .unwrap();
    index
        .upsert_note(&note, &test_content_hash(), &test_path())
        .unwrap();

    let result = index.all_rels().unwrap();
    assert_eq!(result.len(), 3);
    // Alphabetically sorted: child, parent, see-also
    assert_eq!(result[0].rel().as_str(), "child");
    assert_eq!(result[1].rel().as_str(), "parent");
    assert_eq!(result[2].rel().as_str(), "see-also");
}

#[test]
fn all_rels_excludes_rels_from_deleted_notes() {
    let mut index = SqliteIndex::open_in_memory().unwrap();

    let target_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9B".parse().unwrap();
    let link = Link::new(target_id, vec!["parent"]).unwrap();
    let note = Note::builder(test_note_id(), "Test", test_datetime(), test_datetime())
        .links(vec![link])
        .build()
        .unwrap();
    index
        .upsert_note(&note, &test_content_hash(), &test_path())
        .unwrap();
    index.remove_note(note.id()).unwrap();

    let result = index.all_rels().unwrap();
    // Link deleted with note - rel should not appear
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

// ===========================================
// find_by_id_prefix tests
// ===========================================

#[test]
fn find_by_id_prefix_empty_returns_empty() {
    let index = SqliteIndex::open_in_memory().unwrap();
    insert_note_with_body(&index, "01HQ3K5M7NXJK4QZPW8V2R6T9Y", "Test", "body");

    let results = index.find_by_id_prefix("").unwrap();
    assert!(results.is_empty(), "Empty prefix should return no results");
}

#[test]
fn find_by_id_prefix_full_id_matches() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let note = sample_note("Test Note");
    let hash = test_content_hash();
    let path = PathBuf::from("test.md");
    index.upsert_note(&note, &hash, &path).unwrap();

    let results = index
        .find_by_id_prefix("01HQ3K5M7NXJK4QZPW8V2R6T9Y")
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title(), "Test Note");
}

#[test]
fn find_by_id_prefix_8_char_prefix_matches() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let note = sample_note("Test Note");
    let hash = test_content_hash();
    let path = PathBuf::from("test.md");
    index.upsert_note(&note, &hash, &path).unwrap();

    let results = index.find_by_id_prefix("01HQ3K5M").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title(), "Test Note");
}

#[test]
fn find_by_id_prefix_case_insensitive() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let note = sample_note("Test Note");
    let hash = test_content_hash();
    let path = PathBuf::from("test.md");
    index.upsert_note(&note, &hash, &path).unwrap();

    // Lowercase prefix should still match
    let results = index.find_by_id_prefix("01hq3k5m").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title(), "Test Note");
}

#[test]
fn find_by_id_prefix_no_match() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let note = sample_note("Test Note");
    let hash = test_content_hash();
    let path = PathBuf::from("test.md");
    index.upsert_note(&note, &hash, &path).unwrap();

    let results = index.find_by_id_prefix("ZZZZZ").unwrap();
    assert!(results.is_empty());
}

#[test]
fn find_by_id_prefix_multiple_matches() {
    let index = SqliteIndex::open_in_memory().unwrap();
    // Insert two notes with same prefix
    insert_note_with_body(&index, "01HQ3K5M7NXJK4QZPW8V2R6T9A", "Note A", "body a");
    insert_note_with_body(&index, "01HQ3K5M7NXJK4QZPW8V2R6T9B", "Note B", "body b");

    let results = index.find_by_id_prefix("01HQ3K5M").unwrap();
    assert_eq!(results.len(), 2);
}

// ===========================================
// find_by_title tests
// ===========================================

#[test]
fn find_by_title_exact_match() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let note = sample_note("My Exact Title");
    let hash = test_content_hash();
    let path = PathBuf::from("test.md");
    index.upsert_note(&note, &hash, &path).unwrap();

    let results = index.find_by_title("My Exact Title").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title(), "My Exact Title");
}

#[test]
fn find_by_title_case_insensitive() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let note = sample_note("My Exact Title");
    let hash = test_content_hash();
    let path = PathBuf::from("test.md");
    index.upsert_note(&note, &hash, &path).unwrap();

    let results = index.find_by_title("my exact title").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title(), "My Exact Title");
}

#[test]
fn find_by_title_no_match() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let note = sample_note("Some Title");
    let hash = test_content_hash();
    let path = PathBuf::from("test.md");
    index.upsert_note(&note, &hash, &path).unwrap();

    let results = index.find_by_title("Different Title").unwrap();
    assert!(results.is_empty());
}

#[test]
fn find_by_title_partial_does_not_match() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let note = sample_note("My Exact Title");
    let hash = test_content_hash();
    let path = PathBuf::from("test.md");
    index.upsert_note(&note, &hash, &path).unwrap();

    // Partial title should NOT match
    let results = index.find_by_title("Exact").unwrap();
    assert!(results.is_empty());
}

#[test]
fn find_by_title_multiple_with_same_title() {
    let index = SqliteIndex::open_in_memory().unwrap();
    insert_note_with_body(
        &index,
        "01HQ3K5M7NXJK4QZPW8V2R6T9A",
        "Duplicate Title",
        "body a",
    );
    insert_note_with_body(
        &index,
        "01HQ3K5M7NXJK4QZPW8V2R6T9B",
        "Duplicate Title",
        "body b",
    );

    let results = index.find_by_title("Duplicate Title").unwrap();
    assert_eq!(results.len(), 2);
}

// ===========================================
// find_by_alias tests
// ===========================================

#[test]
fn find_by_alias_exact_match() {
    let index = SqliteIndex::open_in_memory().unwrap();
    insert_note_with_aliases(
        &index,
        "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
        "Main Title",
        "myalias otheralias",
    );

    let results = index.find_by_alias("myalias").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title(), "Main Title");
}

#[test]
fn find_by_alias_case_insensitive() {
    let index = SqliteIndex::open_in_memory().unwrap();
    insert_note_with_aliases(
        &index,
        "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
        "Main Title",
        "MyAlias",
    );

    let results = index.find_by_alias("myalias").unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn find_by_alias_no_match() {
    let index = SqliteIndex::open_in_memory().unwrap();
    insert_note_with_aliases(
        &index,
        "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
        "Main Title",
        "myalias",
    );

    let results = index.find_by_alias("nonexistent").unwrap();
    assert!(results.is_empty());
}

#[test]
fn find_by_alias_partial_does_not_match() {
    let index = SqliteIndex::open_in_memory().unwrap();
    insert_note_with_aliases(
        &index,
        "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
        "Main Title",
        "myalias",
    );

    // Partial alias should NOT match
    let results = index.find_by_alias("alias").unwrap();
    assert!(results.is_empty());
}

#[test]
fn find_by_alias_multiple_aliases() {
    let index = SqliteIndex::open_in_memory().unwrap();
    insert_note_with_aliases(
        &index,
        "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
        "API Design",
        "api REST restful",
    );

    // Should match any of the aliases
    let results1 = index.find_by_alias("api").unwrap();
    let results2 = index.find_by_alias("REST").unwrap();
    let results3 = index.find_by_alias("restful").unwrap();

    assert_eq!(results1.len(), 1);
    assert_eq!(results2.len(), 1);
    assert_eq!(results3.len(), 1);
}

#[test]
fn find_by_alias_empty_aliases() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let note = sample_note("No Aliases");
    let hash = test_content_hash();
    let path = PathBuf::from("test.md");
    index.upsert_note(&note, &hash, &path).unwrap();

    let results = index.find_by_alias("anything").unwrap();
    assert!(results.is_empty());
}

// ===========================================
// Backlinks Tests
// ===========================================

fn insert_link(index: &SqliteIndex, source_id: &NoteId, target_id: &NoteId, rels: &[&str]) {
    let link_id: i64 = index
        .conn()
        .query_row(
            "INSERT INTO links (source_id, target_id) VALUES (?, ?) RETURNING id",
            [source_id.to_string(), target_id.to_string()],
            |row| row.get(0),
        )
        .unwrap();

    for rel in rels {
        index
            .conn()
            .execute(
                "INSERT INTO link_rels (link_id, rel) VALUES (?, ?)",
                rusqlite::params![link_id, rel],
            )
            .unwrap();
    }
}

#[test]
fn backlinks_nonexistent_target_returns_empty() {
    let index = SqliteIndex::open_in_memory().unwrap();
    let target_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();

    let results = index.backlinks(&target_id, None).unwrap();
    assert!(results.is_empty());
}

#[test]
fn backlinks_finds_single_source_note() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let source_id = test_note_id();
    let target_id: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Z".parse().unwrap();

    // Insert source note
    let note = sample_note("Source Note");
    index
        .upsert_note(&note, &test_content_hash(), &test_path())
        .unwrap();

    // Insert link from source to target
    insert_link(&index, &source_id, &target_id, &["parent"]);

    let results = index.backlinks(&target_id, None).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id(), &source_id);
}

#[test]
fn backlinks_finds_multiple_source_notes() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let source1_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
    let source2_id: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Z".parse().unwrap();
    let target_id: NoteId = "01HQ5B3S0QYJK5RZQX9W3S7T0A".parse().unwrap();

    // Insert source notes
    let note1 = Note::new(
        source1_id.clone(),
        "Source 1",
        test_datetime(),
        test_datetime(),
    )
    .unwrap();
    let note2 = Note::new(
        source2_id.clone(),
        "Source 2",
        test_datetime(),
        test_datetime(),
    )
    .unwrap();
    index
        .upsert_note(&note1, &test_content_hash(), &PathBuf::from("s1.md"))
        .unwrap();
    index
        .upsert_note(&note2, &test_content_hash(), &PathBuf::from("s2.md"))
        .unwrap();

    // Insert links
    insert_link(&index, &source1_id, &target_id, &["parent"]);
    insert_link(&index, &source2_id, &target_id, &["see-also"]);

    let results = index.backlinks(&target_id, None).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn backlinks_filters_by_rel() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let source1_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
    let source2_id: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Z".parse().unwrap();
    let target_id: NoteId = "01HQ5B3S0QYJK5RZQX9W3S7T0A".parse().unwrap();

    // Insert source notes
    let note1 = Note::new(
        source1_id.clone(),
        "Source 1",
        test_datetime(),
        test_datetime(),
    )
    .unwrap();
    let note2 = Note::new(
        source2_id.clone(),
        "Source 2",
        test_datetime(),
        test_datetime(),
    )
    .unwrap();
    index
        .upsert_note(&note1, &test_content_hash(), &PathBuf::from("s1.md"))
        .unwrap();
    index
        .upsert_note(&note2, &test_content_hash(), &PathBuf::from("s2.md"))
        .unwrap();

    // Insert links with different rels
    insert_link(&index, &source1_id, &target_id, &["parent"]);
    insert_link(&index, &source2_id, &target_id, &["see-also"]);

    let rel = Rel::new("parent").unwrap();
    let results = index.backlinks(&target_id, Some(&rel)).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id(), &source1_id);
}

#[test]
fn backlinks_rel_filter_no_matches_returns_empty() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let source_id = test_note_id();
    let target_id: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Z".parse().unwrap();

    // Insert source note
    let note = sample_note("Source Note");
    index
        .upsert_note(&note, &test_content_hash(), &test_path())
        .unwrap();

    // Insert link with "parent" rel
    insert_link(&index, &source_id, &target_id, &["parent"]);

    // Search for different rel
    let rel = Rel::new("see-also").unwrap();
    let results = index.backlinks(&target_id, Some(&rel)).unwrap();
    assert!(results.is_empty());
}

#[test]
fn backlinks_link_without_rels_found_with_no_filter() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let source_id = test_note_id();
    let target_id: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Z".parse().unwrap();

    // Insert source note
    let note = sample_note("Source Note");
    index
        .upsert_note(&note, &test_content_hash(), &test_path())
        .unwrap();

    // Insert link WITHOUT any rels
    insert_link(&index, &source_id, &target_id, &[]);

    let results = index.backlinks(&target_id, None).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn backlinks_link_without_rels_not_found_with_filter() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let source_id = test_note_id();
    let target_id: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Z".parse().unwrap();

    // Insert source note
    let note = sample_note("Source Note");
    index
        .upsert_note(&note, &test_content_hash(), &test_path())
        .unwrap();

    // Insert link WITHOUT any rels
    insert_link(&index, &source_id, &target_id, &[]);

    // Search with rel filter - should NOT find it
    let rel = Rel::new("parent").unwrap();
    let results = index.backlinks(&target_id, Some(&rel)).unwrap();
    assert!(results.is_empty());
}

#[test]
fn backlinks_link_with_multiple_rels_matches_any() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let source_id = test_note_id();
    let target_id: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Z".parse().unwrap();

    // Insert source note
    let note = sample_note("Source Note");
    index
        .upsert_note(&note, &test_content_hash(), &test_path())
        .unwrap();

    // Insert link with MULTIPLE rels
    insert_link(&index, &source_id, &target_id, &["parent", "mentor"]);

    // Should match on first rel
    let rel1 = Rel::new("parent").unwrap();
    let results1 = index.backlinks(&target_id, Some(&rel1)).unwrap();
    assert_eq!(results1.len(), 1);

    // Should match on second rel
    let rel2 = Rel::new("mentor").unwrap();
    let results2 = index.backlinks(&target_id, Some(&rel2)).unwrap();
    assert_eq!(results2.len(), 1);
}

#[test]
fn backlinks_works_when_source_note_exists() {
    // Target doesn't exist as a note, but source does - backlinks should work
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let source_id = test_note_id();
    let target_id: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Z".parse().unwrap();

    // Insert source note (target is NOT a note)
    let note = sample_note("Source Note");
    index
        .upsert_note(&note, &test_content_hash(), &test_path())
        .unwrap();

    // Insert link to non-existent target
    insert_link(&index, &source_id, &target_id, &["parent"]);

    let results = index.backlinks(&target_id, None).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id(), &source_id);
}

#[test]
fn backlinks_excludes_deleted_source_notes() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let source_id = test_note_id();
    let target_id: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Z".parse().unwrap();

    // Insert source note
    let note = sample_note("Source Note");
    index
        .upsert_note(&note, &test_content_hash(), &test_path())
        .unwrap();

    // Insert link
    insert_link(&index, &source_id, &target_id, &["parent"]);

    // Verify link exists
    let results = index.backlinks(&target_id, None).unwrap();
    assert_eq!(results.len(), 1);

    // Delete source note (link should be cascaded)
    index.remove_note(&source_id).unwrap();

    // Backlinks should be empty now
    let results = index.backlinks(&target_id, None).unwrap();
    assert!(results.is_empty());
}

#[test]
fn backlinks_rel_filter_case_insensitive() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let source_id = test_note_id();
    let target_id: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Z".parse().unwrap();

    // Insert source note
    let note = sample_note("Source Note");
    index
        .upsert_note(&note, &test_content_hash(), &test_path())
        .unwrap();

    // Insert link with lowercase rel
    insert_link(&index, &source_id, &target_id, &["parent"]);

    // Search with uppercase rel - Rel::new normalizes to lowercase
    let rel = Rel::new("PARENT").unwrap();
    let results = index.backlinks(&target_id, Some(&rel)).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn backlinks_returns_distinct_source_notes() {
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let source_id = test_note_id();
    let target_id: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Z".parse().unwrap();

    // Insert source note
    let note = sample_note("Source Note");
    index
        .upsert_note(&note, &test_content_hash(), &test_path())
        .unwrap();

    // Test DISTINCT by having a link with multiple rels
    // Both rel filters should return the same single note, not duplicates
    insert_link(&index, &source_id, &target_id, &["parent", "mentor"]);

    // Both rel filters should return the same single note
    let rel1 = Rel::new("parent").unwrap();
    let results1 = index.backlinks(&target_id, Some(&rel1)).unwrap();

    let rel2 = Rel::new("mentor").unwrap();
    let results2 = index.backlinks(&target_id, Some(&rel2)).unwrap();

    assert_eq!(results1.len(), 1);
    assert_eq!(results2.len(), 1);

    // Without filter
    let results_all = index.backlinks(&target_id, None).unwrap();
    assert_eq!(results_all.len(), 1);
}
