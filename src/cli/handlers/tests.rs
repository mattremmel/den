use super::*;
use crate::cli::config::Config;
use crate::cli::output::OutputFormat;
use crate::cli::{
    BacklinksArgs, EditArgs, NewArgs, RelsArgs, ShowArgs, TagArgs, TagsArgs, TopicsArgs, UntagArgs,
};
use crate::domain::{NoteId, Tag, Topic};
use crate::index::{IndexRepository, IndexedNote, SearchResult};
use crate::infra::ContentHash;
use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use std::collections::HashSet;
use std::path::PathBuf;

// Test helpers
fn test_note_id(suffix: &str) -> NoteId {
    format!("01HQ3K5M7NXJK4QZPW8V2R6T{}", suffix)
        .parse()
        .unwrap()
}

fn test_datetime() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

fn test_content_hash() -> ContentHash {
    ContentHash::compute(b"test content")
}

fn sample_indexed_note_with_topics(
    id_suffix: &str,
    title: &str,
    topics: Vec<Topic>,
) -> IndexedNote {
    IndexedNote::builder(
        test_note_id(id_suffix),
        title,
        test_datetime(),
        test_datetime(),
        PathBuf::from(format!("{}.md", id_suffix)),
        test_content_hash(),
    )
    .topics(topics)
    .build()
}

fn sample_indexed_note_with_tags(id_suffix: &str, title: &str, tags: Vec<Tag>) -> IndexedNote {
    IndexedNote::builder(
        test_note_id(id_suffix),
        title,
        test_datetime(),
        test_datetime(),
        PathBuf::from(format!("{}.md", id_suffix)),
        test_content_hash(),
    )
    .tags(tags)
    .build()
}

// ===========================================
// parse_topic_filter tests
// ===========================================

#[test]
fn parse_topic_filter_without_trailing_slash() {
    let (path, include_descendants) = parse_topic_filter("software/rust");
    assert_eq!(path, "software/rust");
    assert!(!include_descendants);
}

#[test]
fn parse_topic_filter_with_trailing_slash() {
    let (path, include_descendants) = parse_topic_filter("software/rust/");
    assert_eq!(path, "software/rust");
    assert!(include_descendants);
}

#[test]
fn parse_topic_filter_root_with_slash() {
    let (path, include_descendants) = parse_topic_filter("software/");
    assert_eq!(path, "software");
    assert!(include_descendants);
}

// ===========================================
// note_matches_topic tests
// ===========================================

#[test]
fn note_matches_topic_exact_match() {
    let note = sample_indexed_note_with_topics(
        "9A",
        "Rust Guide",
        vec![Topic::new("software/rust").unwrap()],
    );
    let topic = Topic::new("software/rust").unwrap();
    assert!(note_matches_topic(&note, &topic, false));
}

#[test]
fn note_matches_topic_no_match() {
    let note = sample_indexed_note_with_topics(
        "9A",
        "Rust Guide",
        vec![Topic::new("software/rust").unwrap()],
    );
    let topic = Topic::new("software/python").unwrap();
    assert!(!note_matches_topic(&note, &topic, false));
}

#[test]
fn note_matches_topic_descendant_match_with_flag() {
    let note = sample_indexed_note_with_topics(
        "9A",
        "Async Rust",
        vec![Topic::new("software/rust/async").unwrap()],
    );
    let topic = Topic::new("software/rust").unwrap();
    // With include_descendants=true, should match
    assert!(note_matches_topic(&note, &topic, true));
}

#[test]
fn note_matches_topic_descendant_no_match_without_flag() {
    let note = sample_indexed_note_with_topics(
        "9A",
        "Async Rust",
        vec![Topic::new("software/rust/async").unwrap()],
    );
    let topic = Topic::new("software/rust").unwrap();
    // With include_descendants=false, should NOT match
    assert!(!note_matches_topic(&note, &topic, false));
}

#[test]
fn note_matches_topic_parent_no_match() {
    let note =
        sample_indexed_note_with_topics("9A", "Rust Guide", vec![Topic::new("software").unwrap()]);
    let topic = Topic::new("software/rust").unwrap();
    // Parent topic does not match child filter
    assert!(!note_matches_topic(&note, &topic, true));
}

#[test]
fn note_matches_topic_multiple_topics() {
    let note = sample_indexed_note_with_topics(
        "9A",
        "Rust Guide",
        vec![
            Topic::new("software/rust").unwrap(),
            Topic::new("programming").unwrap(),
        ],
    );
    let topic = Topic::new("programming").unwrap();
    assert!(note_matches_topic(&note, &topic, false));
}

// ===========================================
// strip_html_tags tests
// ===========================================

#[test]
fn strip_html_tags_removes_bold() {
    let input = "Hello <b>world</b>!";
    assert_eq!(strip_html_tags(input), "Hello world!");
}

#[test]
fn strip_html_tags_no_tags() {
    let input = "Hello world!";
    assert_eq!(strip_html_tags(input), "Hello world!");
}

#[test]
fn strip_html_tags_multiple_bold() {
    let input = "<b>foo</b> and <b>bar</b>";
    assert_eq!(strip_html_tags(input), "foo and bar");
}

// ===========================================
// Search filtering integration tests
// ===========================================

#[test]
fn search_filters_by_topic_exact() {
    // Create search results with different topics
    let rust_note = sample_indexed_note_with_topics(
        "9A",
        "Rust Guide",
        vec![Topic::new("software/rust").unwrap()],
    );
    let python_note = sample_indexed_note_with_topics(
        "9B",
        "Python Guide",
        vec![Topic::new("software/python").unwrap()],
    );

    let results = vec![
        SearchResult::new(rust_note, 0.9),
        SearchResult::new(python_note, 0.8),
    ];

    let topic = Topic::new("software/rust").unwrap();
    let filtered: Vec<_> = results
        .into_iter()
        .filter(|r| note_matches_topic(r.note(), &topic, false))
        .collect();

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].note().title(), "Rust Guide");
}

#[test]
fn search_filters_by_topic_with_descendants() {
    let rust_note = sample_indexed_note_with_topics(
        "9A",
        "Rust Guide",
        vec![Topic::new("software/rust").unwrap()],
    );
    let async_note = sample_indexed_note_with_topics(
        "9B",
        "Async Rust",
        vec![Topic::new("software/rust/async").unwrap()],
    );
    let python_note = sample_indexed_note_with_topics(
        "9C",
        "Python Guide",
        vec![Topic::new("software/python").unwrap()],
    );

    let results = vec![
        SearchResult::new(rust_note, 0.9),
        SearchResult::new(async_note, 0.8),
        SearchResult::new(python_note, 0.7),
    ];

    let topic = Topic::new("software/rust").unwrap();
    let filtered: Vec<_> = results
        .into_iter()
        .filter(|r| note_matches_topic(r.note(), &topic, true))
        .collect();

    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().any(|r| r.note().title() == "Rust Guide"));
    assert!(filtered.iter().any(|r| r.note().title() == "Async Rust"));
}

#[test]
fn search_filters_by_tags_and_logic() {
    let note1 = sample_indexed_note_with_tags(
        "9A",
        "Note with both tags",
        vec![Tag::new("draft").unwrap(), Tag::new("important").unwrap()],
    );
    let note2 = sample_indexed_note_with_tags(
        "9B",
        "Note with draft only",
        vec![Tag::new("draft").unwrap()],
    );
    let note3 = sample_indexed_note_with_tags(
        "9C",
        "Note with important only",
        vec![Tag::new("important").unwrap()],
    );

    let results = vec![
        SearchResult::new(note1, 0.9),
        SearchResult::new(note2, 0.8),
        SearchResult::new(note3, 0.7),
    ];

    let required_tags: HashSet<Tag> =
        vec![Tag::new("draft").unwrap(), Tag::new("important").unwrap()]
            .into_iter()
            .collect();

    let filtered: Vec<_> = results
        .into_iter()
        .filter(|r| {
            let note_tags: HashSet<_> = r.note().tags().iter().cloned().collect();
            required_tags.is_subset(&note_tags)
        })
        .collect();

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].note().title(), "Note with both tags");
}

#[test]
fn search_preserves_rank_order_after_filtering() {
    let note1 =
        sample_indexed_note_with_topics("9A", "High Rank", vec![Topic::new("software").unwrap()]);
    let note2 =
        sample_indexed_note_with_topics("9B", "Medium Rank", vec![Topic::new("software").unwrap()]);
    let note3 =
        sample_indexed_note_with_topics("9C", "Low Rank", vec![Topic::new("software").unwrap()]);
    let note4 =
        sample_indexed_note_with_topics("9D", "Filtered Out", vec![Topic::new("other").unwrap()]);

    // Results in rank order
    let results = vec![
        SearchResult::new(note1, 0.9),
        SearchResult::new(note4, 0.85),
        SearchResult::new(note2, 0.7),
        SearchResult::new(note3, 0.5),
    ];

    let topic = Topic::new("software").unwrap();
    let filtered: Vec<_> = results
        .into_iter()
        .filter(|r| note_matches_topic(r.note(), &topic, false))
        .collect();

    assert_eq!(filtered.len(), 3);
    // Order should be preserved (highest rank first)
    assert_eq!(filtered[0].note().title(), "High Rank");
    assert_eq!(filtered[1].note().title(), "Medium Rank");
    assert_eq!(filtered[2].note().title(), "Low Rank");
}

// ===========================================
// create_new_note() tests
// ===========================================

#[test]
fn create_new_note_generates_valid_note() {
    let result = create_new_note("Test Note", None, &[], &[]).unwrap();
    assert_eq!(result.note.title(), "Test Note");
    assert!(result.note.description().is_none());
    assert!(result.note.topics().is_empty());
    assert!(result.note.tags().is_empty());
}

#[test]
fn create_new_note_sets_timestamps_to_now() {
    let before = Utc::now();
    let result = create_new_note("Test Note", None, &[], &[]).unwrap();
    let after = Utc::now();

    assert!(result.note.created() >= before);
    assert!(result.note.created() <= after);
    assert_eq!(result.note.created(), result.note.modified());
}

#[test]
fn create_new_note_with_description() {
    let result = create_new_note("Test Note", Some("A test description"), &[], &[]).unwrap();
    assert_eq!(result.note.description(), Some("A test description"));
}

#[test]
fn create_new_note_with_valid_topics() {
    let topics = vec!["software/rust".to_string(), "reference".to_string()];
    let result = create_new_note("Test Note", None, &topics, &[]).unwrap();
    assert_eq!(result.note.topics().len(), 2);
    assert_eq!(result.note.topics()[0].to_string(), "software/rust");
    assert_eq!(result.note.topics()[1].to_string(), "reference");
}

#[test]
fn create_new_note_rejects_invalid_topic() {
    let topics = vec!["software@invalid".to_string()];
    let result = create_new_note("Test Note", None, &topics, &[]);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("invalid topic"));
}

#[test]
fn create_new_note_normalizes_topics() {
    let topics = vec!["/software/rust/".to_string()];
    let result = create_new_note("Test Note", None, &topics, &[]).unwrap();
    assert_eq!(result.note.topics()[0].to_string(), "software/rust");
}

#[test]
fn create_new_note_with_valid_tags() {
    let tags = vec!["draft".to_string(), "important".to_string()];
    let result = create_new_note("Test Note", None, &[], &tags).unwrap();
    assert_eq!(result.note.tags().len(), 2);
    assert_eq!(result.note.tags()[0].as_str(), "draft");
    assert_eq!(result.note.tags()[1].as_str(), "important");
}

#[test]
fn create_new_note_rejects_invalid_tag() {
    let tags = vec!["has spaces".to_string()];
    let result = create_new_note("Test Note", None, &[], &tags);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("invalid tag"));
}

#[test]
fn create_new_note_normalizes_tags_to_lowercase() {
    let tags = vec!["DRAFT".to_string()];
    let result = create_new_note("Test Note", None, &[], &tags).unwrap();
    assert_eq!(result.note.tags()[0].as_str(), "draft");
}

#[test]
fn create_new_note_returns_correct_filename() {
    let result = create_new_note("API Design", None, &[], &[]).unwrap();
    // Should be 10-char prefix + slug + .md
    assert!(result.filename.ends_with("-api-design.md"));
    assert_eq!(result.filename.len(), 10 + 1 + "api-design".len() + 3);
}

#[test]
fn create_new_note_rejects_empty_title() {
    let result = create_new_note("", None, &[], &[]);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("empty"));
}

#[test]
fn create_new_note_rejects_whitespace_only_title() {
    let result = create_new_note("   ", None, &[], &[]);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("empty"));
}

// ===========================================
// handle_new() integration tests
// ===========================================

mod handle_new_tests {
    use super::*;
    use crate::infra::read_note;
    use tempfile::TempDir;

    fn test_config() -> Config {
        Config::default()
    }

    fn test_args(title: &str) -> NewArgs {
        NewArgs {
            title: title.to_string(),
            topics: vec![],
            tags: vec![],
            desc: None,
            edit: false,
        }
    }

    #[test]
    fn handle_new_creates_file() {
        let dir = TempDir::new().unwrap();
        let args = test_args("Test Note");
        let config = test_config();

        handle_new(&args, dir.path(), &config).unwrap();

        // Find the created file
        let files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();

        assert_eq!(files.len(), 1, "Should create exactly one .md file");
    }

    #[test]
    fn handle_new_file_has_correct_filename_format() {
        let dir = TempDir::new().unwrap();
        let args = test_args("API Design");
        let config = test_config();

        handle_new(&args, dir.path(), &config).unwrap();

        let files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();

        assert_eq!(files.len(), 1, "Should create exactly one .md file");
        let filename = files[0].file_name();
        let name = filename.to_string_lossy();
        assert!(name.ends_with("-api-design.md"));
        // First 10 chars should be ULID prefix
        assert_eq!(name.len(), 10 + 1 + "api-design".len() + 3);
    }

    #[test]
    fn handle_new_file_contains_valid_frontmatter() {
        let dir = TempDir::new().unwrap();
        let args = NewArgs {
            title: "Test Note".to_string(),
            topics: vec!["software/rust".to_string()],
            tags: vec!["draft".to_string()],
            desc: Some("A test description".to_string()),
            edit: false,
        };
        let config = test_config();

        handle_new(&args, dir.path(), &config).unwrap();

        // Find and read the created file
        let file = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .find(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .unwrap();

        let parsed = read_note(&file.path()).unwrap();
        assert_eq!(parsed.note.title(), "Test Note");
        assert_eq!(parsed.note.description(), Some("A test description"));
        assert_eq!(parsed.note.topics().len(), 1);
        assert_eq!(parsed.note.tags().len(), 1);
    }

    #[test]
    fn handle_new_creates_multiple_files() {
        let dir = TempDir::new().unwrap();
        let config = test_config();

        // Create two notes with different titles
        handle_new(&test_args("First Note"), dir.path(), &config).unwrap();
        handle_new(&test_args("Second Note"), dir.path(), &config).unwrap();

        // Find the created files
        let files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();

        assert_eq!(files.len(), 2, "Should create two unique files");
    }

    #[test]
    fn handle_new_fails_with_invalid_topic() {
        let dir = TempDir::new().unwrap();
        let args = NewArgs {
            title: "Test Note".to_string(),
            topics: vec!["invalid@topic".to_string()],
            tags: vec![],
            desc: None,
            edit: false,
        };
        let config = test_config();

        let result = handle_new(&args, dir.path(), &config);
        assert!(result.is_err());
    }

    #[test]
    fn handle_new_fails_with_invalid_tag() {
        let dir = TempDir::new().unwrap();
        let args = NewArgs {
            title: "Test Note".to_string(),
            topics: vec![],
            tags: vec!["has spaces".to_string()],
            desc: None,
            edit: false,
        };
        let config = test_config();

        let result = handle_new(&args, dir.path(), &config);
        assert!(result.is_err());
    }

    #[test]
    fn handle_new_fails_with_empty_title() {
        let dir = TempDir::new().unwrap();
        let args = test_args("");
        let config = test_config();

        let result = handle_new(&args, dir.path(), &config);
        assert!(result.is_err());
    }

    #[test]
    fn handle_new_fails_if_directory_doesnt_exist() {
        let args = test_args("Test Note");
        let config = test_config();

        let result = handle_new(&args, Path::new("/nonexistent/directory"), &config);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("does not exist"));
    }
}

// ===========================================
// resolve_note tests
// ===========================================

mod resolve_note_tests {
    use super::*;
    use crate::domain::Note;
    use crate::index::SqliteIndex;

    fn setup_index_with_notes() -> SqliteIndex {
        let mut index = SqliteIndex::open_in_memory().unwrap();

        // Note 1: "API Design"
        let note1 = Note::builder(
            "01HQ3K5M7NXJK4QZPW8V2R6T9A".parse().unwrap(),
            "API Design",
            test_datetime(),
            test_datetime(),
        )
        .aliases(vec!["REST".to_string(), "api".to_string()])
        .build()
        .unwrap();
        let hash1 = test_content_hash();
        index
            .upsert_note(&note1, &hash1, Path::new("01HQ3K5M7N-api-design.md"))
            .unwrap();

        // Note 2: "Rust Programming"
        let note2 = Note::builder(
            "01HQ3K5M7NXJK4QZPW8V2R6T9B".parse().unwrap(),
            "Rust Programming",
            test_datetime(),
            test_datetime(),
        )
        .build()
        .unwrap();
        let hash2 = test_content_hash();
        index
            .upsert_note(&note2, &hash2, Path::new("01HQ3K5M7N-rust-programming.md"))
            .unwrap();

        // Note 3: "API Testing" (different prefix)
        let note3 = Note::builder(
            "01HQ4A2R9PXJK4QZPW8V2R6T9C".parse().unwrap(),
            "API Testing",
            test_datetime(),
            test_datetime(),
        )
        .build()
        .unwrap();
        let hash3 = test_content_hash();
        index
            .upsert_note(&note3, &hash3, Path::new("01HQ4A2R9P-api-testing.md"))
            .unwrap();

        index
    }

    #[test]
    fn resolve_by_full_id() {
        let index = setup_index_with_notes();
        let result = resolve_note(&index, "01HQ3K5M7NXJK4QZPW8V2R6T9A").unwrap();

        match result {
            ResolveResult::Unique(note) => {
                assert_eq!(note.title(), "API Design");
            }
            _ => panic!("Expected Unique result"),
        }
    }

    #[test]
    fn resolve_by_id_prefix_unique() {
        let index = setup_index_with_notes();
        // "01HQ4A2R" only matches one note
        let result = resolve_note(&index, "01HQ4A2R").unwrap();

        match result {
            ResolveResult::Unique(note) => {
                assert_eq!(note.title(), "API Testing");
            }
            _ => panic!("Expected Unique result"),
        }
    }

    #[test]
    fn resolve_by_id_prefix_ambiguous() {
        let index = setup_index_with_notes();
        // "01HQ3K5M7N" matches both "API Design" and "Rust Programming"
        let result = resolve_note(&index, "01HQ3K5M7N").unwrap();

        match result {
            ResolveResult::Ambiguous(notes) => {
                assert_eq!(notes.len(), 2);
            }
            _ => panic!("Expected Ambiguous result, got {:?}", result),
        }
    }

    #[test]
    fn resolve_by_title_exact() {
        let index = setup_index_with_notes();
        let result = resolve_note(&index, "Rust Programming").unwrap();

        match result {
            ResolveResult::Unique(note) => {
                assert_eq!(note.title(), "Rust Programming");
            }
            _ => panic!("Expected Unique result"),
        }
    }

    #[test]
    fn resolve_by_title_case_insensitive() {
        let index = setup_index_with_notes();
        let result = resolve_note(&index, "rust programming").unwrap();

        match result {
            ResolveResult::Unique(note) => {
                assert_eq!(note.title(), "Rust Programming");
            }
            _ => panic!("Expected Unique result"),
        }
    }

    #[test]
    fn resolve_by_alias() {
        let index = setup_index_with_notes();
        let result = resolve_note(&index, "REST").unwrap();

        match result {
            ResolveResult::Unique(note) => {
                assert_eq!(note.title(), "API Design");
            }
            _ => panic!("Expected Unique result"),
        }
    }

    #[test]
    fn resolve_by_alias_case_insensitive() {
        let index = setup_index_with_notes();
        let result = resolve_note(&index, "rest").unwrap();

        match result {
            ResolveResult::Unique(note) => {
                assert_eq!(note.title(), "API Design");
            }
            _ => panic!("Expected Unique result"),
        }
    }

    #[test]
    fn resolve_not_found() {
        let index = setup_index_with_notes();
        let result = resolve_note(&index, "nonexistent").unwrap();

        match result {
            ResolveResult::NotFound => {}
            _ => panic!("Expected NotFound result"),
        }
    }

    #[test]
    fn resolve_whitespace_trimmed() {
        let index = setup_index_with_notes();
        let result = resolve_note(&index, "  Rust Programming  ").unwrap();

        match result {
            ResolveResult::Unique(note) => {
                assert_eq!(note.title(), "Rust Programming");
            }
            _ => panic!("Expected Unique result"),
        }
    }

    #[test]
    fn resolve_id_prefix_takes_precedence() {
        // If an ID prefix uniquely matches, it should return immediately
        // even if there are also title/alias matches
        let index = setup_index_with_notes();
        let result = resolve_note(&index, "01HQ4A2R9P").unwrap();

        match result {
            ResolveResult::Unique(note) => {
                assert_eq!(note.title(), "API Testing");
            }
            _ => panic!("Expected Unique result"),
        }
    }
}

// ===========================================
// handle_show integration tests
// ===========================================

mod handle_show_tests {
    use super::*;
    use crate::index::{IndexBuilder, SqliteIndex};
    use tempfile::TempDir;

    fn setup_notes_dir() -> TempDir {
        let dir = TempDir::new().unwrap();

        // Create index directory
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();

        // Create a test note
        let note_content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9A
title: API Design
description: Notes on API design principles
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
topics:
  - software/architecture
tags:
  - draft
aliases:
  - REST
---

# API Design Principles

This is the body of the note.
"#;
        std::fs::write(dir.path().join("01HQ3K5M7N-api-design.md"), note_content).unwrap();

        // Build index
        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();
        let builder = IndexBuilder::new(dir.path().to_path_buf());
        builder.full_rebuild(&mut index).unwrap();

        dir
    }

    #[test]
    fn handle_show_by_id_prefix() {
        let dir = setup_notes_dir();
        let args = ShowArgs {
            note: "01HQ3K5M".to_string(),
        };

        let result = handle_show(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn handle_show_by_title() {
        let dir = setup_notes_dir();
        let args = ShowArgs {
            note: "API Design".to_string(),
        };

        let result = handle_show(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn handle_show_by_alias() {
        let dir = setup_notes_dir();
        let args = ShowArgs {
            note: "REST".to_string(),
        };

        let result = handle_show(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn handle_show_not_found() {
        let dir = setup_notes_dir();
        let args = ShowArgs {
            note: "nonexistent".to_string(),
        };

        let result = handle_show(&args, dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }
}

// ===========================================
// handle_edit tests
// ===========================================

mod handle_edit_tests {
    use super::*;
    use crate::index::{IndexBuilder, SqliteIndex};
    use std::cell::RefCell;
    use tempfile::TempDir;

    /// Mock editor for testing.
    struct MockEditor {
        opened: RefCell<Option<PathBuf>>,
        should_fail: bool,
    }

    impl MockEditor {
        fn new() -> Self {
            Self {
                opened: RefCell::new(None),
                should_fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                opened: RefCell::new(None),
                should_fail: true,
            }
        }

        fn opened_path(&self) -> Option<PathBuf> {
            self.opened.borrow().clone()
        }
    }

    impl EditorLauncher for MockEditor {
        fn open(&self, path: &Path) -> Result<()> {
            *self.opened.borrow_mut() = Some(path.to_path_buf());
            if self.should_fail {
                bail!("editor failed to open");
            }
            Ok(())
        }
    }

    fn setup_notes_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();

        let note = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9A
title: API Design
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
aliases:
  - REST
---
Body content.
"#;
        std::fs::write(dir.path().join("01HQ3K5M7N-api-design.md"), note).unwrap();

        // Build index
        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();
        IndexBuilder::new(dir.path().to_path_buf())
            .full_rebuild(&mut index)
            .unwrap();

        dir
    }

    fn setup_notes_dir_with_ambiguous() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();

        // Note 1
        let note1 = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9A
title: API Design
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
Note 1
"#;
        std::fs::write(dir.path().join("01HQ3K5M7N-api-design.md"), note1).unwrap();

        // Note 2 with same title
        let note2 = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9B
title: API Design
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
Note 2
"#;
        std::fs::write(dir.path().join("01HQ3K5M7N-api-design-2.md"), note2).unwrap();

        // Build index
        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();
        IndexBuilder::new(dir.path().to_path_buf())
            .full_rebuild(&mut index)
            .unwrap();

        dir
    }

    // Phase 1: Error cases

    #[test]
    fn handle_edit_not_found_returns_error() {
        let dir = setup_notes_dir();
        let args = EditArgs {
            note: "nonexistent".to_string(),
        };
        let editor = MockEditor::new();

        let result = handle_edit_impl(&args, dir.path(), &editor);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn handle_edit_ambiguous_returns_error() {
        let dir = setup_notes_dir_with_ambiguous();
        let args = EditArgs {
            note: "API Design".to_string(),
        };
        let editor = MockEditor::new();

        let result = handle_edit_impl(&args, dir.path(), &editor);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("ambiguous"));
    }

    // Phase 2: Resolution methods

    #[test]
    fn handle_edit_by_id_prefix() {
        let dir = setup_notes_dir();
        let args = EditArgs {
            note: "01HQ3K5M".to_string(),
        };
        let editor = MockEditor::new();

        let result = handle_edit_impl(&args, dir.path(), &editor);
        assert!(result.is_ok());

        let opened = editor.opened_path().unwrap();
        assert!(opened.ends_with("01HQ3K5M7N-api-design.md"));
    }

    #[test]
    fn handle_edit_by_title() {
        let dir = setup_notes_dir();
        let args = EditArgs {
            note: "API Design".to_string(),
        };
        let editor = MockEditor::new();

        let result = handle_edit_impl(&args, dir.path(), &editor);
        assert!(result.is_ok());

        let opened = editor.opened_path().unwrap();
        assert!(opened.ends_with("01HQ3K5M7N-api-design.md"));
    }

    #[test]
    fn handle_edit_by_alias() {
        let dir = setup_notes_dir();
        let args = EditArgs {
            note: "REST".to_string(),
        };
        let editor = MockEditor::new();

        let result = handle_edit_impl(&args, dir.path(), &editor);
        assert!(result.is_ok());

        let opened = editor.opened_path().unwrap();
        assert!(opened.ends_with("01HQ3K5M7N-api-design.md"));
    }

    // Phase 3: Timestamp update

    #[test]
    fn handle_edit_updates_modified_timestamp() {
        let dir = setup_notes_dir();
        let args = EditArgs {
            note: "API Design".to_string(),
        };
        let editor = MockEditor::new();

        // Read original modified time
        let file_path = dir.path().join("01HQ3K5M7N-api-design.md");
        let before = crate::infra::read_note(&file_path).unwrap();
        let original_modified = before.note.modified();

        // Small delay to ensure timestamp differs
        std::thread::sleep(std::time::Duration::from_millis(10));

        let result = handle_edit_impl(&args, dir.path(), &editor);
        assert!(result.is_ok());

        // Read updated modified time
        let after = crate::infra::read_note(&file_path).unwrap();
        assert!(
            after.note.modified() > original_modified,
            "modified timestamp should be updated"
        );
    }

    // Phase 4: Editor failure

    #[test]
    fn handle_edit_editor_failure_returns_error() {
        let dir = setup_notes_dir();
        let args = EditArgs {
            note: "API Design".to_string(),
        };
        let editor = MockEditor::failing();

        let result = handle_edit_impl(&args, dir.path(), &editor);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("editor failed"));
    }

    #[test]
    fn handle_edit_no_timestamp_update_on_editor_failure() {
        let dir = setup_notes_dir();
        let args = EditArgs {
            note: "API Design".to_string(),
        };
        let editor = MockEditor::failing();

        // Read original modified time
        let file_path = dir.path().join("01HQ3K5M7N-api-design.md");
        let before = crate::infra::read_note(&file_path).unwrap();
        let original_modified = before.note.modified();

        let _ = handle_edit_impl(&args, dir.path(), &editor);

        // Read modified time after (should be unchanged)
        let after = crate::infra::read_note(&file_path).unwrap();
        assert_eq!(
            after.note.modified(),
            original_modified,
            "modified timestamp should NOT be updated on editor failure"
        );
    }

    // Phase 5: Index update

    #[test]
    fn handle_edit_updates_index() {
        let dir = setup_notes_dir();
        let args = EditArgs {
            note: "API Design".to_string(),
        };
        let editor = MockEditor::new();

        let result = handle_edit_impl(&args, dir.path(), &editor);
        assert!(result.is_ok());

        // Verify index was updated by checking modified time in index
        let db_path = dir.path().join(".index/notes.db");
        let index = SqliteIndex::open(&db_path).unwrap();
        let notes = index.find_by_title("API Design").unwrap();
        assert_eq!(notes.len(), 1);

        // The index should reflect the updated modified timestamp
        let file_path = dir.path().join("01HQ3K5M7N-api-design.md");
        let file_note = crate::infra::read_note(&file_path).unwrap();
        assert_eq!(notes[0].modified(), file_note.note.modified());
    }
}

// ===========================================
// handle_tags tests
// ===========================================

mod handle_tags_tests {
    use super::*;
    use crate::index::{IndexBuilder, SqliteIndex};
    use tempfile::TempDir;

    fn setup_empty_index() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();
        let db_path = dir.path().join(".index/notes.db");
        let _ = SqliteIndex::open(&db_path).unwrap();
        dir
    }

    fn setup_index_with_tags(tags: &[&str]) -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();

        // Create a note with the specified tags
        let tags_yaml = tags
            .iter()
            .map(|t| format!("  - {}", t))
            .collect::<Vec<_>>()
            .join("\n");

        let note = format!(
            r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9A
title: Test Note
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
tags:
{}
---
Body
"#,
            tags_yaml
        );
        std::fs::write(dir.path().join("01HQ3K5M7N-test-note.md"), note).unwrap();

        // Build index
        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();
        IndexBuilder::new(dir.path().to_path_buf())
            .full_rebuild(&mut index)
            .unwrap();

        dir
    }

    #[test]
    fn handle_tags_empty_index() {
        let dir = setup_empty_index();
        let args = TagsArgs {
            counts: false,
            format: OutputFormat::Human,
        };
        let result = handle_tags(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn handle_tags_lists_tags_sorted() {
        let dir = setup_index_with_tags(&["rust", "draft", "important"]);
        let args = TagsArgs {
            counts: false,
            format: OutputFormat::Human,
        };
        let result = handle_tags(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn handle_tags_json_output() {
        let dir = setup_index_with_tags(&["rust", "draft"]);
        let args = TagsArgs {
            counts: true,
            format: OutputFormat::Json,
        };
        let result = handle_tags(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn handle_tags_fails_with_nonexistent_dir() {
        let args = TagsArgs {
            counts: false,
            format: OutputFormat::Human,
        };
        let result = handle_tags(&args, Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }
}

// ===========================================
// handle_rels tests
// ===========================================

mod handle_rels_tests {
    use super::*;
    use crate::index::{IndexBuilder, SqliteIndex};
    use tempfile::TempDir;

    fn setup_empty_index() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();
        let db_path = dir.path().join(".index/notes.db");
        let _ = SqliteIndex::open(&db_path).unwrap();
        dir
    }

    fn setup_index_with_links(rels: &[&str]) -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();

        // Create a note with a link using the specified rels
        let rels_yaml = rels
            .iter()
            .map(|r| format!("      - {}", r))
            .collect::<Vec<_>>()
            .join("\n");

        let note = format!(
            r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9A
title: Test Note
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
links:
  - target: 01HQ3K5M7NXJK4QZPW8V2R6T9B
    rel:
{}
---
Body
"#,
            rels_yaml
        );
        std::fs::write(dir.path().join("01HQ3K5M7N-test-note.md"), note).unwrap();

        // Build index
        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();
        IndexBuilder::new(dir.path().to_path_buf())
            .full_rebuild(&mut index)
            .unwrap();

        dir
    }

    #[test]
    fn handle_rels_empty_index() {
        let dir = setup_empty_index();
        let args = RelsArgs {
            counts: false,
            format: OutputFormat::Human,
        };
        let result = handle_rels(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn handle_rels_lists_rels_sorted() {
        let dir = setup_index_with_links(&["see-also", "parent", "child"]);
        let args = RelsArgs {
            counts: false,
            format: OutputFormat::Human,
        };
        let result = handle_rels(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn handle_rels_with_counts() {
        let dir = setup_index_with_links(&["parent", "see-also"]);
        let args = RelsArgs {
            counts: true,
            format: OutputFormat::Human,
        };
        let result = handle_rels(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn handle_rels_json_output() {
        let dir = setup_index_with_links(&["parent", "child"]);
        let args = RelsArgs {
            counts: true,
            format: OutputFormat::Json,
        };
        let result = handle_rels(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn handle_rels_paths_output() {
        let dir = setup_index_with_links(&["parent"]);
        let args = RelsArgs {
            counts: false,
            format: OutputFormat::Paths,
        };
        let result = handle_rels(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn handle_rels_fails_with_nonexistent_dir() {
        let args = RelsArgs {
            counts: false,
            format: OutputFormat::Human,
        };
        let result = handle_rels(&args, Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }
}

// ===========================================
// handle_topics tests
// ===========================================

mod handle_topics_tests {
    use super::*;
    use crate::index::{IndexBuilder, SqliteIndex};
    use tempfile::TempDir;

    fn setup_empty_index() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();
        let db_path = dir.path().join(".index/notes.db");
        let _ = SqliteIndex::open(&db_path).unwrap();
        dir
    }

    fn setup_index_with_topics(topics: &[&str]) -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();

        let topics_yaml = topics
            .iter()
            .map(|t| format!("  - {}", t))
            .collect::<Vec<_>>()
            .join("\n");

        let note = format!(
            r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9A
title: Test Note
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
topics:
{}
---
Body
"#,
            topics_yaml
        );
        std::fs::write(dir.path().join("01HQ3K5M7N-test-note.md"), note).unwrap();

        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();
        IndexBuilder::new(dir.path().to_path_buf())
            .full_rebuild(&mut index)
            .unwrap();

        dir
    }

    #[test]
    fn handle_topics_empty_index() {
        let dir = setup_empty_index();
        let args = TopicsArgs {
            counts: false,
            format: OutputFormat::Human,
        };
        let result = handle_topics(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn handle_topics_lists_topics_sorted() {
        let dir = setup_index_with_topics(&["software/rust", "reference", "software"]);
        let args = TopicsArgs {
            counts: false,
            format: OutputFormat::Human,
        };
        let result = handle_topics(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn handle_topics_with_counts() {
        let dir = setup_index_with_topics(&["software/rust", "software"]);
        let args = TopicsArgs {
            counts: true,
            format: OutputFormat::Human,
        };
        let result = handle_topics(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn handle_topics_json_output() {
        let dir = setup_index_with_topics(&["software/rust"]);
        let args = TopicsArgs {
            counts: true,
            format: OutputFormat::Json,
        };
        let result = handle_topics(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn handle_topics_fails_with_nonexistent_dir() {
        let args = TopicsArgs {
            counts: false,
            format: OutputFormat::Human,
        };
        let result = handle_topics(&args, Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }
}

// ===========================================
// handle_tag tests
// ===========================================

mod handle_tag_tests {
    use super::*;
    use crate::index::{IndexBuilder, SqliteIndex};
    use crate::infra::read_note;
    use tempfile::TempDir;

    fn setup_note_without_tags() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();

        let note = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9A
title: Test Note
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
Body content.
"#;
        std::fs::write(dir.path().join("01HQ3K5M7N-test-note.md"), note).unwrap();

        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();
        IndexBuilder::new(dir.path().to_path_buf())
            .full_rebuild(&mut index)
            .unwrap();

        dir
    }

    fn setup_note_with_tags(tags: &[&str]) -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();

        let tags_yaml = tags
            .iter()
            .map(|t| format!("  - {}", t))
            .collect::<Vec<_>>()
            .join("\n");

        let note = format!(
            r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9A
title: Test Note
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
tags:
{}
---
Body content.
"#,
            tags_yaml
        );
        std::fs::write(dir.path().join("01HQ3K5M7N-test-note.md"), note).unwrap();

        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();
        IndexBuilder::new(dir.path().to_path_buf())
            .full_rebuild(&mut index)
            .unwrap();

        dir
    }

    // Phase 1: Error cases

    #[test]
    fn handle_tag_note_not_found() {
        let dir = setup_note_without_tags();
        let args = TagArgs {
            note: "nonexistent".to_string(),
            tag: "draft".to_string(),
        };
        let result = handle_tag(&args, dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn handle_tag_invalid_tag() {
        let dir = setup_note_without_tags();
        let args = TagArgs {
            note: "Test Note".to_string(),
            tag: "has spaces".to_string(),
        };
        let result = handle_tag(&args, dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid tag"));
    }

    // Phase 2: Resolution

    #[test]
    fn handle_tag_by_id_prefix() {
        let dir = setup_note_without_tags();
        let args = TagArgs {
            note: "01HQ3K5M".to_string(),
            tag: "draft".to_string(),
        };
        let result = handle_tag(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert!(parsed.note.tags().iter().any(|t| t.as_str() == "draft"));
    }

    #[test]
    fn handle_tag_by_title() {
        let dir = setup_note_without_tags();
        let args = TagArgs {
            note: "Test Note".to_string(),
            tag: "draft".to_string(),
        };
        let result = handle_tag(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert!(parsed.note.tags().iter().any(|t| t.as_str() == "draft"));
    }

    // Phase 3: Addition

    #[test]
    fn handle_tag_adds_to_empty() {
        let dir = setup_note_without_tags();
        let args = TagArgs {
            note: "Test Note".to_string(),
            tag: "draft".to_string(),
        };
        let result = handle_tag(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert_eq!(parsed.note.tags().len(), 1);
        assert_eq!(parsed.note.tags()[0].as_str(), "draft");
    }

    #[test]
    fn handle_tag_appends_to_existing() {
        let dir = setup_note_with_tags(&["existing"]);
        let args = TagArgs {
            note: "Test Note".to_string(),
            tag: "new-tag".to_string(),
        };
        let result = handle_tag(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert_eq!(parsed.note.tags().len(), 2);
        assert!(parsed.note.tags().iter().any(|t| t.as_str() == "existing"));
        assert!(parsed.note.tags().iter().any(|t| t.as_str() == "new-tag"));
    }

    #[test]
    fn handle_tag_normalizes_case() {
        let dir = setup_note_without_tags();
        let args = TagArgs {
            note: "Test Note".to_string(),
            tag: "DRAFT".to_string(),
        };
        let result = handle_tag(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert_eq!(parsed.note.tags()[0].as_str(), "draft");
    }

    // Phase 4: Idempotency

    #[test]
    fn handle_tag_idempotent_exact_case() {
        let dir = setup_note_with_tags(&["draft"]);
        let args = TagArgs {
            note: "Test Note".to_string(),
            tag: "draft".to_string(),
        };
        let result = handle_tag(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let parsed = read_note(&file_path).unwrap();
        // Should still have only one "draft" tag
        assert_eq!(parsed.note.tags().len(), 1);
    }

    #[test]
    fn handle_tag_idempotent_different_case() {
        let dir = setup_note_with_tags(&["draft"]);
        let args = TagArgs {
            note: "Test Note".to_string(),
            tag: "DRAFT".to_string(),
        };
        let result = handle_tag(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let parsed = read_note(&file_path).unwrap();
        // Should still have only one tag (case-insensitive comparison)
        assert_eq!(parsed.note.tags().len(), 1);
    }

    // Phase 5: Timestamp

    #[test]
    fn handle_tag_updates_modified_when_changed() {
        let dir = setup_note_without_tags();
        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let before = read_note(&file_path).unwrap();
        let original_modified = before.note.modified();

        std::thread::sleep(std::time::Duration::from_millis(10));

        let args = TagArgs {
            note: "Test Note".to_string(),
            tag: "draft".to_string(),
        };
        handle_tag(&args, dir.path()).unwrap();

        let after = read_note(&file_path).unwrap();
        assert!(after.note.modified() > original_modified);
    }

    #[test]
    fn handle_tag_no_timestamp_change_when_idempotent() {
        let dir = setup_note_with_tags(&["draft"]);
        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let before = read_note(&file_path).unwrap();
        let original_modified = before.note.modified();

        let args = TagArgs {
            note: "Test Note".to_string(),
            tag: "draft".to_string(),
        };
        handle_tag(&args, dir.path()).unwrap();

        let after = read_note(&file_path).unwrap();
        // Timestamp should not change since tag was already present
        assert_eq!(after.note.modified(), original_modified);
    }
}

// ===========================================
// handle_untag tests
// ===========================================

mod handle_untag_tests {
    use super::*;
    use crate::index::{IndexBuilder, SqliteIndex};
    use crate::infra::read_note;
    use tempfile::TempDir;

    fn setup_note_with_tags(tags: &[&str]) -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();

        let tags_yaml = tags
            .iter()
            .map(|t| format!("  - {}", t))
            .collect::<Vec<_>>()
            .join("\n");

        let note = format!(
            r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9A
title: Test Note
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
tags:
{}
---
Body content.
"#,
            tags_yaml
        );
        std::fs::write(dir.path().join("01HQ3K5M7N-test-note.md"), note).unwrap();

        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();
        IndexBuilder::new(dir.path().to_path_buf())
            .full_rebuild(&mut index)
            .unwrap();

        dir
    }

    fn setup_note_without_tags() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();

        let note = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9A
title: Test Note
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
Body content.
"#;
        std::fs::write(dir.path().join("01HQ3K5M7N-test-note.md"), note).unwrap();

        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();
        IndexBuilder::new(dir.path().to_path_buf())
            .full_rebuild(&mut index)
            .unwrap();

        dir
    }

    // Phase 1: Error cases

    #[test]
    fn handle_untag_note_not_found() {
        let dir = setup_note_with_tags(&["draft"]);
        let args = UntagArgs {
            note: "nonexistent".to_string(),
            tag: "draft".to_string(),
        };
        let result = handle_untag(&args, dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn handle_untag_invalid_tag() {
        let dir = setup_note_with_tags(&["draft"]);
        let args = UntagArgs {
            note: "Test Note".to_string(),
            tag: "has spaces".to_string(),
        };
        let result = handle_untag(&args, dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid tag"));
    }

    // Phase 2: Resolution

    #[test]
    fn handle_untag_by_id_prefix() {
        let dir = setup_note_with_tags(&["draft"]);
        let args = UntagArgs {
            note: "01HQ3K5M".to_string(),
            tag: "draft".to_string(),
        };
        let result = handle_untag(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert!(parsed.note.tags().is_empty());
    }

    #[test]
    fn handle_untag_by_title() {
        let dir = setup_note_with_tags(&["draft"]);
        let args = UntagArgs {
            note: "Test Note".to_string(),
            tag: "draft".to_string(),
        };
        let result = handle_untag(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert!(parsed.note.tags().is_empty());
    }

    // Phase 3: Removal

    #[test]
    fn handle_untag_removes_tag() {
        let dir = setup_note_with_tags(&["draft", "important"]);
        let args = UntagArgs {
            note: "Test Note".to_string(),
            tag: "draft".to_string(),
        };
        let result = handle_untag(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert_eq!(parsed.note.tags().len(), 1);
        assert_eq!(parsed.note.tags()[0].as_str(), "important");
    }

    #[test]
    fn handle_untag_removes_last_tag() {
        let dir = setup_note_with_tags(&["draft"]);
        let args = UntagArgs {
            note: "Test Note".to_string(),
            tag: "draft".to_string(),
        };
        let result = handle_untag(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert!(parsed.note.tags().is_empty());
    }

    #[test]
    fn handle_untag_case_insensitive() {
        let dir = setup_note_with_tags(&["draft"]);
        let args = UntagArgs {
            note: "Test Note".to_string(),
            tag: "DRAFT".to_string(),
        };
        let result = handle_untag(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert!(parsed.note.tags().is_empty());
    }

    // Phase 4: Idempotency

    #[test]
    fn handle_untag_idempotent_tag_not_present() {
        let dir = setup_note_with_tags(&["important"]);
        let args = UntagArgs {
            note: "Test Note".to_string(),
            tag: "draft".to_string(),
        };
        let result = handle_untag(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert_eq!(parsed.note.tags().len(), 1);
        assert_eq!(parsed.note.tags()[0].as_str(), "important");
    }

    #[test]
    fn handle_untag_idempotent_no_tags() {
        let dir = setup_note_without_tags();
        let args = UntagArgs {
            note: "Test Note".to_string(),
            tag: "draft".to_string(),
        };
        let result = handle_untag(&args, dir.path());
        assert!(result.is_ok());
    }

    // Phase 5: Timestamp

    #[test]
    fn handle_untag_updates_modified_when_changed() {
        let dir = setup_note_with_tags(&["draft"]);
        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let before = read_note(&file_path).unwrap();
        let original_modified = before.note.modified();

        std::thread::sleep(std::time::Duration::from_millis(10));

        let args = UntagArgs {
            note: "Test Note".to_string(),
            tag: "draft".to_string(),
        };
        handle_untag(&args, dir.path()).unwrap();

        let after = read_note(&file_path).unwrap();
        assert!(after.note.modified() > original_modified);
    }

    #[test]
    fn handle_untag_no_timestamp_change_when_idempotent() {
        let dir = setup_note_with_tags(&["important"]);
        let file_path = dir.path().join("01HQ3K5M7N-test-note.md");
        let before = read_note(&file_path).unwrap();
        let original_modified = before.note.modified();

        let args = UntagArgs {
            note: "Test Note".to_string(),
            tag: "draft".to_string(), // Not present
        };
        handle_untag(&args, dir.path()).unwrap();

        let after = read_note(&file_path).unwrap();
        // Timestamp should not change since tag wasn't present
        assert_eq!(after.note.modified(), original_modified);
    }
}

// ===========================================
// handle_backlinks tests
// ===========================================

mod handle_backlinks_tests {
    use super::*;
    use crate::domain::Note;
    use crate::index::SqliteIndex;
    use tempfile::TempDir;

    fn setup_empty_index() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();
        let db_path = dir.path().join(".index/notes.db");
        let _ = SqliteIndex::open(&db_path).unwrap();
        dir
    }

    fn setup_index_with_target_note() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();
        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();

        // Create target note
        let target_note = Note::new(
            "01HQ5B3S0QYJK5RZQX9W3S7T0A".parse().unwrap(),
            "Target Note",
            test_datetime(),
            test_datetime(),
        )
        .unwrap();
        index
            .upsert_note(
                &target_note,
                &test_content_hash(),
                std::path::Path::new("01HQ5B3S0Q-target-note.md"),
            )
            .unwrap();

        dir
    }

    fn setup_index_with_backlinks() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();
        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();

        // Create target note
        let target_id: NoteId = "01HQ5B3S0QYJK5RZQX9W3S7T0A".parse().unwrap();
        let target_note = Note::new(
            target_id.clone(),
            "Target Note",
            test_datetime(),
            test_datetime(),
        )
        .unwrap();
        index
            .upsert_note(
                &target_note,
                &test_content_hash(),
                std::path::Path::new("01HQ5B3S0Q-target-note.md"),
            )
            .unwrap();

        // Create source note that links to target
        let source_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9A".parse().unwrap();
        let source_note = Note::new(
            source_id.clone(),
            "Source Note",
            test_datetime(),
            test_datetime(),
        )
        .unwrap();
        index
            .upsert_note(
                &source_note,
                &test_content_hash(),
                std::path::Path::new("01HQ3K5M7N-source-note.md"),
            )
            .unwrap();

        // Insert link from source to target
        insert_link(&index, &source_id, &target_id, &["parent"]);

        dir
    }

    fn setup_index_with_multiple_backlinks() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();
        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();

        // Create target note
        let target_id: NoteId = "01HQ5B3S0QYJK5RZQX9W3S7T0A".parse().unwrap();
        let target_note = Note::new(
            target_id.clone(),
            "Target Note",
            test_datetime(),
            test_datetime(),
        )
        .unwrap();
        index
            .upsert_note(
                &target_note,
                &test_content_hash(),
                std::path::Path::new("01HQ5B3S0Q-target-note.md"),
            )
            .unwrap();

        // Create source notes with different modified times
        let source1_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9A".parse().unwrap();
        let source1_note = Note::new(
            source1_id.clone(),
            "Source Note A",
            test_datetime(),
            DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
                .unwrap()
                .with_timezone(&Utc),
        )
        .unwrap();
        index
            .upsert_note(
                &source1_note,
                &test_content_hash(),
                std::path::Path::new("01HQ3K5M7N-source-a.md"),
            )
            .unwrap();

        let source2_id: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9B".parse().unwrap();
        let source2_note = Note::new(
            source2_id.clone(),
            "Source Note B",
            test_datetime(),
            DateTime::parse_from_rfc3339("2024-01-16T10:30:00Z")
                .unwrap()
                .with_timezone(&Utc),
        )
        .unwrap();
        index
            .upsert_note(
                &source2_note,
                &test_content_hash(),
                std::path::Path::new("01HQ4A2R9P-source-b.md"),
            )
            .unwrap();

        let source3_id: NoteId = "01HQ6C4T1RXJK6SZRY0X4T81CC".parse().unwrap();
        let source3_note = Note::new(
            source3_id.clone(),
            "Source Note C",
            test_datetime(),
            DateTime::parse_from_rfc3339("2024-01-14T10:30:00Z")
                .unwrap()
                .with_timezone(&Utc),
        )
        .unwrap();
        index
            .upsert_note(
                &source3_note,
                &test_content_hash(),
                std::path::Path::new("01HQ6C4T1R-source-c.md"),
            )
            .unwrap();

        // Insert links from all sources to target with different rels
        insert_link(&index, &source1_id, &target_id, &["parent"]);
        insert_link(&index, &source2_id, &target_id, &["see-also"]);
        insert_link(&index, &source3_id, &target_id, &["parent"]);

        dir
    }

    fn setup_index_with_alias() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();
        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();

        // Create target note with alias
        let target_id: NoteId = "01HQ5B3S0QYJK5RZQX9W3S7T0A".parse().unwrap();
        let target_note = Note::builder(
            target_id.clone(),
            "Target Note",
            test_datetime(),
            test_datetime(),
        )
        .aliases(vec!["REST".to_string()])
        .build()
        .unwrap();
        index
            .upsert_note(
                &target_note,
                &test_content_hash(),
                std::path::Path::new("01HQ5B3S0Q-target-note.md"),
            )
            .unwrap();

        // Create source note that links to target
        let source_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9A".parse().unwrap();
        let source_note = Note::new(
            source_id.clone(),
            "Source Note",
            test_datetime(),
            test_datetime(),
        )
        .unwrap();
        index
            .upsert_note(
                &source_note,
                &test_content_hash(),
                std::path::Path::new("01HQ3K5M7N-source-note.md"),
            )
            .unwrap();

        // Insert link
        insert_link(&index, &source_id, &target_id, &["parent"]);

        dir
    }

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

    // Phase 1: Note Resolution Tests

    #[test]
    fn backlinks_note_not_found_returns_error() {
        let dir = setup_empty_index();
        let args = BacklinksArgs {
            note: "nonexistent".to_string(),
            rel: None,
            format: OutputFormat::Human,
        };
        let result = handle_backlinks(&args, dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("note not found"));
        assert!(err.contains("nonexistent"));
    }

    #[test]
    fn backlinks_ambiguous_note_returns_error() {
        // Setup index with ambiguous ID prefix
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();
        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();

        // Create two notes with same ID prefix
        let note1 = Note::new(
            "01HQ3K5M7NXJK4QZPW8V2R6T9A".parse().unwrap(),
            "Note A",
            test_datetime(),
            test_datetime(),
        )
        .unwrap();
        let note2 = Note::new(
            "01HQ3K5M7NXJK4QZPW8V2R6T9B".parse().unwrap(),
            "Note B",
            test_datetime(),
            test_datetime(),
        )
        .unwrap();
        index
            .upsert_note(
                &note1,
                &test_content_hash(),
                std::path::Path::new("01HQ3K5M7N-note-a.md"),
            )
            .unwrap();
        index
            .upsert_note(
                &note2,
                &test_content_hash(),
                std::path::Path::new("01HQ3K5M7N-note-b.md"),
            )
            .unwrap();

        let args = BacklinksArgs {
            note: "01HQ3K5M7N".to_string(),
            rel: None,
            format: OutputFormat::Human,
        };
        let result = handle_backlinks(&args, dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("ambiguous"));
    }

    // Phase 2: Core Backlinks Query Tests

    #[test]
    fn backlinks_returns_empty_for_note_with_no_backlinks() {
        let dir = setup_index_with_target_note();
        let args = BacklinksArgs {
            note: "Target Note".to_string(),
            rel: None,
            format: OutputFormat::Human,
        };
        let result = handle_backlinks(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn backlinks_returns_linking_notes() {
        let dir = setup_index_with_backlinks();
        let args = BacklinksArgs {
            note: "Target Note".to_string(),
            rel: None,
            format: OutputFormat::Human,
        };
        let result = handle_backlinks(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn backlinks_returns_multiple_linking_notes() {
        let dir = setup_index_with_multiple_backlinks();
        let args = BacklinksArgs {
            note: "Target Note".to_string(),
            rel: None,
            format: OutputFormat::Human,
        };
        let result = handle_backlinks(&args, dir.path());
        assert!(result.is_ok());
    }

    // Phase 3: Rel Filter Tests

    #[test]
    fn backlinks_with_rel_filter_returns_matching_only() {
        let dir = setup_index_with_multiple_backlinks();
        let args = BacklinksArgs {
            note: "Target Note".to_string(),
            rel: Some("parent".to_string()),
            format: OutputFormat::Human,
        };
        let result = handle_backlinks(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn backlinks_with_invalid_rel_returns_error() {
        let dir = setup_index_with_target_note();
        let args = BacklinksArgs {
            note: "Target Note".to_string(),
            rel: Some("invalid_rel".to_string()), // underscore is invalid
            format: OutputFormat::Human,
        };
        let result = handle_backlinks(&args, dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid relationship type"));
    }

    #[test]
    fn backlinks_with_rel_filter_no_matches_returns_empty() {
        let dir = setup_index_with_backlinks();
        let args = BacklinksArgs {
            note: "Target Note".to_string(),
            rel: Some("see-also".to_string()), // link is "parent" only
            format: OutputFormat::Human,
        };
        let result = handle_backlinks(&args, dir.path());
        assert!(result.is_ok());
    }

    // Phase 4: Output Formatting Tests

    #[test]
    fn backlinks_human_format_returns_ok() {
        let dir = setup_index_with_backlinks();
        let args = BacklinksArgs {
            note: "Target Note".to_string(),
            rel: None,
            format: OutputFormat::Human,
        };
        let result = handle_backlinks(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn backlinks_json_format_returns_ok() {
        let dir = setup_index_with_backlinks();
        let args = BacklinksArgs {
            note: "Target Note".to_string(),
            rel: None,
            format: OutputFormat::Json,
        };
        let result = handle_backlinks(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn backlinks_paths_format_returns_ok() {
        let dir = setup_index_with_backlinks();
        let args = BacklinksArgs {
            note: "Target Note".to_string(),
            rel: None,
            format: OutputFormat::Paths,
        };
        let result = handle_backlinks(&args, dir.path());
        assert!(result.is_ok());
    }

    // Phase 5: Edge Cases

    #[test]
    fn backlinks_resolves_by_id_prefix() {
        let dir = setup_index_with_backlinks();
        let args = BacklinksArgs {
            note: "01HQ5B3S".to_string(), // 8-char prefix
            rel: None,
            format: OutputFormat::Human,
        };
        let result = handle_backlinks(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn backlinks_resolves_by_alias() {
        let dir = setup_index_with_alias();
        let args = BacklinksArgs {
            note: "REST".to_string(),
            rel: None,
            format: OutputFormat::Human,
        };
        let result = handle_backlinks(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn backlinks_resolves_by_alias_case_insensitive() {
        let dir = setup_index_with_alias();
        let args = BacklinksArgs {
            note: "rest".to_string(),
            rel: None,
            format: OutputFormat::Human,
        };
        let result = handle_backlinks(&args, dir.path());
        assert!(result.is_ok());
    }
}

// ===========================================
// handle_link tests
// ===========================================

mod handle_link_tests {
    use super::*;
    use crate::cli::{LinkArgs, UnlinkArgs};
    use crate::index::{IndexBuilder, SqliteIndex};
    use crate::infra::read_note;
    use tempfile::TempDir;

    // Test helpers

    fn create_test_note(dir: &Path, id: &str, title: &str) {
        let content = format!(
            r#"---
id: {id}
title: {title}
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
Body content.
"#
        );
        let prefix = &id[..10];
        let slug = title.to_lowercase().replace(' ', "-");
        let filename = format!("{prefix}-{slug}.md");
        std::fs::write(dir.join(&filename), content).unwrap();
    }

    fn create_test_note_with_links(dir: &Path, id: &str, title: &str, links_yaml: &str) {
        let content = format!(
            r#"---
id: {id}
title: {title}
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
links:
{links_yaml}
---
Body content.
"#
        );
        let prefix = &id[..10];
        let slug = title.to_lowercase().replace(' ', "-");
        let filename = format!("{prefix}-{slug}.md");
        std::fs::write(dir.join(&filename), content).unwrap();
    }

    fn setup_two_notes() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();
        create_test_note(dir.path(), "01HQ3K5M7NXJK4QZPW8V2R6T9A", "Source Note");
        create_test_note(dir.path(), "01HQ4A2R9PXJK4QZPW8V2R6T9B", "Target Note");
        // Build index
        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();
        IndexBuilder::new(dir.path().to_path_buf())
            .full_rebuild(&mut index)
            .unwrap();
        dir
    }

    fn setup_ambiguous_notes() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();
        // Two notes with same ID prefix (10 chars)
        create_test_note(dir.path(), "01HQ3K5M7NXJK4QZPW8V2R6T9A", "Note A");
        create_test_note(dir.path(), "01HQ3K5M7NXJK4QZPW8V2R6T9B", "Note B");
        create_test_note(dir.path(), "01HQ4A2R9PXJK4QZPW8V2R6T9C", "Target Note");
        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();
        IndexBuilder::new(dir.path().to_path_buf())
            .full_rebuild(&mut index)
            .unwrap();
        dir
    }

    fn test_link_args(source: &str, target: &str, rels: Vec<&str>) -> LinkArgs {
        LinkArgs {
            source: source.to_string(),
            target: target.to_string(),
            rels: rels.iter().map(|s| s.to_string()).collect(),
            note: None,
        }
    }

    fn test_link_args_with_context(
        source: &str,
        target: &str,
        rels: Vec<&str>,
        context: &str,
    ) -> LinkArgs {
        LinkArgs {
            source: source.to_string(),
            target: target.to_string(),
            rels: rels.iter().map(|s| s.to_string()).collect(),
            note: Some(context.to_string()),
        }
    }

    // ===========================================
    // Phase 1: Input Validation Tests
    // ===========================================

    #[test]
    fn handle_link_no_rels_returns_error() {
        let dir = setup_two_notes();
        let args = LinkArgs {
            source: "Source Note".to_string(),
            target: "Target Note".to_string(),
            rels: vec![],
            note: None,
        };
        let result = handle_link(&args, dir.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("at least one --rel")
        );
    }

    #[test]
    fn handle_link_invalid_rel_returns_error() {
        let dir = setup_two_notes();
        let args = test_link_args("Source Note", "Target Note", vec!["has_underscore"]);
        let result = handle_link(&args, dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid rel"));
    }

    #[test]
    fn handle_link_one_invalid_rel_among_many_returns_error() {
        let dir = setup_two_notes();
        let args = test_link_args("Source Note", "Target Note", vec!["valid", "in@valid"]);
        let result = handle_link(&args, dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid rel"));
        assert!(err.contains("in@valid"));
    }

    // ===========================================
    // Phase 2: Source Resolution Tests
    // ===========================================

    #[test]
    fn handle_link_source_not_found_returns_error() {
        let dir = setup_two_notes();
        let args = test_link_args("nonexistent", "Target Note", vec!["parent"]);
        let result = handle_link(&args, dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("source note not found"));
        assert!(err.contains("nonexistent"));
    }

    #[test]
    fn handle_link_source_ambiguous_returns_error() {
        let dir = setup_ambiguous_notes();
        let args = test_link_args("01HQ3K5M7N", "Target Note", vec!["parent"]);
        let result = handle_link(&args, dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("ambiguous source"));
    }

    #[test]
    fn handle_link_source_by_id_prefix_resolves() {
        let dir = setup_two_notes();
        // Use unique prefix that matches only Source Note
        let args = test_link_args("01HQ3K5M", "Target Note", vec!["parent"]);
        let result = handle_link(&args, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn handle_link_source_by_title_resolves() {
        let dir = setup_two_notes();
        let args = test_link_args("Source Note", "Target Note", vec!["parent"]);
        let result = handle_link(&args, dir.path());
        assert!(result.is_ok());
    }

    // ===========================================
    // Phase 3: Target Resolution Tests
    // ===========================================

    #[test]
    fn handle_link_target_resolves_to_full_id() {
        let dir = setup_two_notes();
        // Use a partial ID prefix for target
        let args = test_link_args("Source Note", "01HQ4A2R", vec!["parent"]);
        let result = handle_link(&args, dir.path());
        assert!(result.is_ok());

        // Verify link was created with full target ID
        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert_eq!(parsed.note.links().len(), 1);
        assert_eq!(
            parsed.note.links()[0].target().to_string(),
            "01HQ4A2R9PXJK4QZPW8V2R6T9B"
        );
    }

    #[test]
    fn handle_link_target_by_title_uses_resolved_id() {
        let dir = setup_two_notes();
        let args = test_link_args("Source Note", "Target Note", vec!["parent"]);
        let result = handle_link(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert_eq!(parsed.note.links().len(), 1);
        // Target ID should be the full ID of Target Note
        assert_eq!(
            parsed.note.links()[0].target().to_string(),
            "01HQ4A2R9PXJK4QZPW8V2R6T9B"
        );
    }

    #[test]
    fn handle_link_target_not_found_valid_ulid_creates_broken_link() {
        let dir = setup_two_notes();
        // Use a valid ULID that doesn't exist in the index
        let args = test_link_args("Source Note", "01HZ9Z9Z9ZXJK4QZPW8V2R6T9Z", vec!["parent"]);
        let result = handle_link(&args, dir.path());
        assert!(result.is_ok());

        // Verify broken link was created
        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert_eq!(parsed.note.links().len(), 1);
        assert_eq!(
            parsed.note.links()[0].target().to_string(),
            "01HZ9Z9Z9ZXJK4QZPW8V2R6T9Z"
        );
    }

    #[test]
    fn handle_link_target_ambiguous_returns_error() {
        let dir = setup_ambiguous_notes();
        let args = test_link_args("Target Note", "01HQ3K5M7N", vec!["parent"]);
        let result = handle_link(&args, dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("ambiguous target"));
    }

    #[test]
    fn handle_link_target_invalid_returns_error() {
        let dir = setup_two_notes();
        let args = test_link_args("Source Note", "not-a-ulid", vec!["parent"]);
        let result = handle_link(&args, dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("target not found and not a valid note ID"));
    }

    // ===========================================
    // Phase 4: Link Creation Tests (Happy Path)
    // ===========================================

    #[test]
    fn handle_link_creates_link_with_single_rel() {
        let dir = setup_two_notes();
        let args = test_link_args("Source Note", "Target Note", vec!["parent"]);
        let result = handle_link(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert_eq!(parsed.note.links().len(), 1);
        assert_eq!(parsed.note.links()[0].rel().len(), 1);
        assert_eq!(parsed.note.links()[0].rel()[0].as_str(), "parent");
    }

    #[test]
    fn handle_link_creates_link_with_multiple_rels() {
        let dir = setup_two_notes();
        let args = test_link_args("Source Note", "Target Note", vec!["parent", "see-also"]);
        let result = handle_link(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert_eq!(parsed.note.links().len(), 1);
        assert_eq!(parsed.note.links()[0].rel().len(), 2);
    }

    #[test]
    fn handle_link_creates_link_with_context() {
        let dir = setup_two_notes();
        let args = test_link_args_with_context(
            "Source Note",
            "Target Note",
            vec!["parent"],
            "Some context",
        );
        let result = handle_link(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert_eq!(parsed.note.links().len(), 1);
        assert_eq!(parsed.note.links()[0].context(), Some("Some context"));
    }

    #[test]
    fn handle_link_normalizes_rels() {
        let dir = setup_two_notes();
        let args = test_link_args("Source Note", "Target Note", vec!["PARENT", "See-Also"]);
        let result = handle_link(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let parsed = read_note(&file_path).unwrap();
        let rels: Vec<&str> = parsed.note.links()[0]
            .rel()
            .iter()
            .map(|r| r.as_str())
            .collect();
        assert_eq!(rels, vec!["parent", "see-also"]);
    }

    #[test]
    fn handle_link_preserves_existing_links() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();

        // Create source note with an existing link
        let existing_link_yaml = r#"  - id: 01HZ9Z9Z9ZXJK4QZPW8V2R6T9X
    rel:
      - existing"#;
        create_test_note_with_links(
            dir.path(),
            "01HQ3K5M7NXJK4QZPW8V2R6T9A",
            "Source Note",
            existing_link_yaml,
        );
        create_test_note(dir.path(), "01HQ4A2R9PXJK4QZPW8V2R6T9B", "Target Note");

        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();
        IndexBuilder::new(dir.path().to_path_buf())
            .full_rebuild(&mut index)
            .unwrap();

        let args = test_link_args("Source Note", "Target Note", vec!["parent"]);
        let result = handle_link(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert_eq!(parsed.note.links().len(), 2);
    }

    // ===========================================
    // Phase 5: Idempotency Tests
    // ===========================================

    #[test]
    fn handle_link_same_target_same_rels_is_noop() {
        let dir = setup_two_notes();
        let args = test_link_args("Source Note", "Target Note", vec!["parent"]);

        // First call creates the link
        handle_link(&args, dir.path()).unwrap();

        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let before = read_note(&file_path).unwrap();
        let original_modified = before.note.modified();

        // Small delay to ensure timestamp would differ
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Second call should be no-op
        let result = handle_link(&args, dir.path());
        assert!(result.is_ok());

        let after = read_note(&file_path).unwrap();
        // Timestamp should not change since no actual change
        assert_eq!(after.note.modified(), original_modified);
        assert_eq!(after.note.links().len(), 1);
    }

    #[test]
    fn handle_link_same_target_merges_rels() {
        let dir = setup_two_notes();

        // First call: add parent rel
        let args1 = test_link_args("Source Note", "Target Note", vec!["parent"]);
        handle_link(&args1, dir.path()).unwrap();

        // Second call: add see-also rel to same target
        let args2 = test_link_args("Source Note", "Target Note", vec!["see-also"]);
        handle_link(&args2, dir.path()).unwrap();

        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let parsed = read_note(&file_path).unwrap();

        // Should have one link with merged rels
        assert_eq!(parsed.note.links().len(), 1);
        assert_eq!(parsed.note.links()[0].rel().len(), 2);
        let rels: Vec<&str> = parsed.note.links()[0]
            .rel()
            .iter()
            .map(|r| r.as_str())
            .collect();
        assert!(rels.contains(&"parent"));
        assert!(rels.contains(&"see-also"));
    }

    #[test]
    fn handle_link_same_target_updates_context() {
        let dir = setup_two_notes();

        // First call: no context
        let args1 = test_link_args("Source Note", "Target Note", vec!["parent"]);
        handle_link(&args1, dir.path()).unwrap();

        // Second call: add context
        let args2 = test_link_args_with_context(
            "Source Note",
            "Target Note",
            vec!["parent"],
            "New context",
        );
        handle_link(&args2, dir.path()).unwrap();

        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let parsed = read_note(&file_path).unwrap();

        assert_eq!(parsed.note.links().len(), 1);
        assert_eq!(parsed.note.links()[0].context(), Some("New context"));
    }

    // ===========================================
    // Phase 6: Timestamp & Index Tests
    // ===========================================

    #[test]
    fn handle_link_updates_modified_timestamp() {
        let dir = setup_two_notes();
        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let before = read_note(&file_path).unwrap();
        let original_modified = before.note.modified();

        std::thread::sleep(std::time::Duration::from_millis(10));

        let args = test_link_args("Source Note", "Target Note", vec!["parent"]);
        handle_link(&args, dir.path()).unwrap();

        let after = read_note(&file_path).unwrap();
        assert!(after.note.modified() > original_modified);
    }

    #[test]
    fn handle_link_noop_preserves_timestamp() {
        let dir = setup_two_notes();
        let args = test_link_args("Source Note", "Target Note", vec!["parent"]);

        // First call
        handle_link(&args, dir.path()).unwrap();

        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let before = read_note(&file_path).unwrap();
        let original_modified = before.note.modified();

        std::thread::sleep(std::time::Duration::from_millis(10));

        // Second call (no-op)
        handle_link(&args, dir.path()).unwrap();

        let after = read_note(&file_path).unwrap();
        // Timestamp should NOT change
        assert_eq!(after.note.modified(), original_modified);
    }

    #[test]
    fn handle_link_updates_index() {
        let dir = setup_two_notes();
        let args = test_link_args("Source Note", "Target Note", vec!["parent"]);
        handle_link(&args, dir.path()).unwrap();

        // Verify index was updated by checking backlinks query works
        let db_path = dir.path().join(".index/notes.db");
        let index = SqliteIndex::open(&db_path).unwrap();

        let target_id: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9B".parse().unwrap();
        let backlinks = index.backlinks(&target_id, None).unwrap();
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].title(), "Source Note");
    }

    // ===========================================
    // Phase 7: Edge Cases
    // ===========================================

    #[test]
    fn handle_link_self_link_allowed() {
        let dir = setup_two_notes();
        let args = test_link_args("Source Note", "Source Note", vec!["self-reference"]);
        let result = handle_link(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert_eq!(parsed.note.links().len(), 1);
        // Target should be source's own ID
        assert_eq!(
            parsed.note.links()[0].target().to_string(),
            "01HQ3K5M7NXJK4QZPW8V2R6T9A"
        );
    }

    #[test]
    fn handle_link_preserves_body_content() {
        let dir = setup_two_notes();
        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let before = read_note(&file_path).unwrap();
        let original_body = before.body.clone();

        let args = test_link_args("Source Note", "Target Note", vec!["parent"]);
        handle_link(&args, dir.path()).unwrap();

        let after = read_note(&file_path).unwrap();
        assert_eq!(after.body, original_body);
    }

    // ===========================================
    // handle_unlink Tests
    // ===========================================

    fn test_unlink_args(source: &str, target: &str) -> UnlinkArgs {
        UnlinkArgs {
            source: source.to_string(),
            target: target.to_string(),
        }
    }

    fn setup_linked_notes() -> TempDir {
        let dir = setup_two_notes();
        // Add a link from source to target
        let args = test_link_args("Source Note", "Target Note", vec!["parent"]);
        handle_link(&args, dir.path()).unwrap();
        dir
    }

    #[test]
    fn handle_unlink_removes_link_from_note_file() {
        let dir = setup_linked_notes();
        let args = test_unlink_args("Source Note", "Target Note");
        let result = handle_unlink(&args, dir.path());
        assert!(result.is_ok());

        // Verify link was removed
        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert!(parsed.note.links().is_empty());
    }

    #[test]
    fn handle_unlink_link_not_found_returns_ok() {
        let dir = setup_two_notes();
        // Source has no links to target
        let args = test_unlink_args("Source Note", "Target Note");
        let result = handle_unlink(&args, dir.path());
        assert!(result.is_ok()); // Not an error, just no-op
    }

    #[test]
    fn handle_unlink_source_not_found_returns_error() {
        let dir = setup_two_notes();
        let args = test_unlink_args("Nonexistent Note", "Target Note");
        let result = handle_unlink(&args, dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn handle_unlink_preserves_other_links() {
        let dir = setup_linked_notes();
        // Add another link
        let link_args = test_link_args("Source Note", "Source Note", vec!["self-ref"]);
        handle_link(&link_args, dir.path()).unwrap();

        // Unlink only the parent link to target
        let args = test_unlink_args("Source Note", "Target Note");
        handle_unlink(&args, dir.path()).unwrap();

        // Self-link should remain
        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert_eq!(parsed.note.links().len(), 1);
        assert_eq!(
            parsed.note.links()[0].target().to_string(),
            "01HQ3K5M7NXJK4QZPW8V2R6T9A" // Source's own ID
        );
    }

    #[test]
    fn handle_unlink_updates_modified_timestamp() {
        let dir = setup_linked_notes();
        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let before = read_note(&file_path).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        let args = test_unlink_args("Source Note", "Target Note");
        handle_unlink(&args, dir.path()).unwrap();

        let after = read_note(&file_path).unwrap();
        assert!(after.note.modified() > before.note.modified());
    }

    #[test]
    fn handle_unlink_target_by_id_prefix() {
        let dir = setup_linked_notes();
        // Use ID prefix instead of title
        let args = test_unlink_args("Source Note", "01HQ4A2R9P");
        let result = handle_unlink(&args, dir.path());
        assert!(result.is_ok());

        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        let parsed = read_note(&file_path).unwrap();
        assert!(parsed.note.links().is_empty());
    }

    #[test]
    fn handle_unlink_allows_removal_of_broken_link() {
        // Create a note with a link to a non-existent note ID
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".index")).unwrap();

        // Create note with pre-existing broken link in frontmatter
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9A
title: Source Note
created: 2024-01-01T00:00:00Z
modified: 2024-01-01T00:00:00Z
links:
  - id: 01ZZZZZZZZXJK4QZPW8V2R6T9X
    rel: [broken-ref]
---

Content"#;
        let file_path = dir.path().join("01HQ3K5M7N-source-note.md");
        std::fs::write(&file_path, content).unwrap();

        // Build index
        let db_path = dir.path().join(".index/notes.db");
        let mut index = SqliteIndex::open(&db_path).unwrap();
        IndexBuilder::new(dir.path().to_path_buf())
            .full_rebuild(&mut index)
            .unwrap();

        // Unlink using the broken target ID
        let args = test_unlink_args("Source Note", "01ZZZZZZZZXJK4QZPW8V2R6T9X");
        let result = handle_unlink(&args, dir.path());
        assert!(result.is_ok());

        let parsed = read_note(&file_path).unwrap();
        assert!(parsed.note.links().is_empty());
    }
}

// ===========================================
// handle_check tests
// ===========================================

mod handle_check_tests {
    use crate::cli::CheckArgs;
    use crate::cli::handlers::handle_check;
    use tempfile::TempDir;

    fn check_args() -> CheckArgs {
        CheckArgs { fix: false }
    }

    fn valid_note_content(id_suffix: &str, title: &str) -> String {
        format!(
            r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T{id_suffix}
title: {title}
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
topics:
  - test/topic
---

Body content."#
        )
    }

    fn orphan_note_content(id_suffix: &str, title: &str) -> String {
        format!(
            r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T{id_suffix}
title: {title}
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---

Body content."#
        )
    }

    fn note_with_link(id_suffix: &str, title: &str, target_id: &str) -> String {
        format!(
            r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T{id_suffix}
title: {title}
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
topics:
  - test/topic
links:
  - id: {target_id}
    rel:
      - see-also
---

Body content."#
        )
    }

    // ===========================================
    // Cycle 1: Empty Directory
    // ===========================================

    #[test]
    fn handle_check_empty_directory_succeeds() {
        let dir = TempDir::new().unwrap();
        let args = check_args();

        let result = handle_check(&args, dir.path());

        assert!(result.is_ok());
    }

    // ===========================================
    // Cycle 2: All Valid Notes
    // ===========================================

    #[test]
    fn handle_check_all_valid_notes_returns_ok() {
        let dir = TempDir::new().unwrap();

        // Create two valid notes with unique IDs
        std::fs::write(
            dir.path().join("01HQ3K5M7N-note-a.md"),
            valid_note_content("9A", "Note A"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("01HQ3K5M7N-note-b.md"),
            valid_note_content("9B", "Note B"),
        )
        .unwrap();

        let args = check_args();
        let result = handle_check(&args, dir.path());

        assert!(result.is_ok());
    }

    // ===========================================
    // Cycle 3: Parse Errors
    // ===========================================

    #[test]
    fn handle_check_detects_parse_error() {
        let dir = TempDir::new().unwrap();

        // Create a file with invalid frontmatter
        std::fs::write(
            dir.path().join("01HQ3K5M7N-bad.md"),
            "---\ninvalid yaml: [missing bracket\n---\nBody",
        )
        .unwrap();

        let args = check_args();
        let result = handle_check(&args, dir.path());

        assert!(result.is_err());
    }

    #[test]
    fn handle_check_continues_after_parse_error() {
        let dir = TempDir::new().unwrap();

        // Create one bad file and one good file
        std::fs::write(
            dir.path().join("01HQ3K5M7N-bad.md"),
            "---\ninvalid yaml: [missing bracket\n---\nBody",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("01HQ3K5M7N-good.md"),
            valid_note_content("9A", "Good Note"),
        )
        .unwrap();

        let args = check_args();
        let result = handle_check(&args, dir.path());

        // Should fail due to parse error
        assert!(result.is_err());
    }

    // ===========================================
    // Cycle 4: Duplicate IDs
    // ===========================================

    #[test]
    fn handle_check_detects_duplicate_ids() {
        let dir = TempDir::new().unwrap();

        // Create two notes with the same ID
        std::fs::write(
            dir.path().join("01HQ3K5M7N-first.md"),
            valid_note_content("9A", "First Note"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("01HQ3K5M7N-second.md"),
            valid_note_content("9A", "Second Note"),
        )
        .unwrap();

        let args = check_args();
        let result = handle_check(&args, dir.path());

        assert!(result.is_err());
    }

    // ===========================================
    // Cycle 5: Broken Links
    // ===========================================

    #[test]
    fn handle_check_detects_broken_links() {
        let dir = TempDir::new().unwrap();

        // Create a note with a link to a non-existent ID
        std::fs::write(
            dir.path().join("01HQ3K5M7N-note.md"),
            note_with_link("9A", "Note A", "01ZZZZZZZZXJK4QZPW8V2R6T9X"),
        )
        .unwrap();

        let args = check_args();
        let result = handle_check(&args, dir.path());

        assert!(result.is_err());
    }

    #[test]
    fn handle_check_valid_links_pass() {
        let dir = TempDir::new().unwrap();

        // Create two notes where one links to the other
        std::fs::write(
            dir.path().join("01HQ3K5M7N-source.md"),
            note_with_link("9A", "Source", "01HQ3K5M7NXJK4QZPW8V2R6T9B"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("01HQ3K5M7N-target.md"),
            valid_note_content("9B", "Target"),
        )
        .unwrap();

        let args = check_args();
        let result = handle_check(&args, dir.path());

        assert!(result.is_ok());
    }

    // ===========================================
    // Cycle 6: Orphaned Notes (Warnings)
    // ===========================================

    #[test]
    fn handle_check_warnings_dont_cause_failure() {
        let dir = TempDir::new().unwrap();

        // Create an orphan note (no topics)
        std::fs::write(
            dir.path().join("01HQ3K5M7N-orphan.md"),
            orphan_note_content("9A", "Orphan Note"),
        )
        .unwrap();

        let args = check_args();
        let result = handle_check(&args, dir.path());

        // Warnings don't cause failure
        assert!(result.is_ok());
    }

    #[test]
    fn handle_check_reports_orphaned_notes() {
        let dir = TempDir::new().unwrap();

        // Create one orphan and one valid note
        std::fs::write(
            dir.path().join("01HQ3K5M7N-orphan.md"),
            orphan_note_content("9A", "Orphan"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("01HQ3K5M7N-valid.md"),
            valid_note_content("9B", "Valid"),
        )
        .unwrap();

        let args = check_args();
        let result = handle_check(&args, dir.path());

        // Should still succeed (warnings don't fail)
        assert!(result.is_ok());
    }

    // ===========================================
    // Cycle 7: Output Formatting / Mixed Errors+Warnings
    // ===========================================

    #[test]
    fn handle_check_errors_cause_failure_with_warnings_present() {
        let dir = TempDir::new().unwrap();

        // Create an orphan (warning) and duplicate IDs (error)
        std::fs::write(
            dir.path().join("01HQ3K5M7N-orphan.md"),
            orphan_note_content("9A", "Orphan"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("01HQ3K5M7N-dup1.md"),
            valid_note_content("9B", "Dup 1"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("01HQ3K5M7N-dup2.md"),
            valid_note_content("9B", "Dup 2"),
        )
        .unwrap();

        let args = check_args();
        let result = handle_check(&args, dir.path());

        // Should fail due to duplicate ID error
        assert!(result.is_err());
    }

    // ===========================================
    // Cycle 8: Edge Cases
    // ===========================================

    #[test]
    fn handle_check_nonexistent_directory_returns_error() {
        let args = check_args();
        let result = handle_check(&args, std::path::Path::new("/nonexistent/path"));

        assert!(result.is_err());
    }

    #[test]
    fn handle_check_ignores_hidden_files() {
        let dir = TempDir::new().unwrap();

        // Create a hidden file that would fail parsing
        std::fs::write(
            dir.path().join(".hidden.md"),
            "---\ninvalid yaml: [missing bracket\n---\nBody",
        )
        .unwrap();

        // Create a valid visible note
        std::fs::write(
            dir.path().join("01HQ3K5M7N-visible.md"),
            valid_note_content("9A", "Visible"),
        )
        .unwrap();

        let args = check_args();
        let result = handle_check(&args, dir.path());

        // Should succeed because hidden files are ignored
        assert!(result.is_ok());
    }

    // ===========================================
    // Cycle 9: --fix flag for broken links
    // ===========================================

    fn note_with_multiple_links(id_suffix: &str, title: &str, target_ids: &[&str]) -> String {
        let links: Vec<String> = target_ids
            .iter()
            .map(|tid| format!("  - id: {}\n    rel:\n      - see-also", tid))
            .collect();
        format!(
            r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T{id_suffix}
title: {title}
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
topics:
  - test/topic
links:
{}
---

Body content."#,
            links.join("\n")
        )
    }

    #[test]
    fn handle_check_fix_removes_single_broken_link() {
        let dir = TempDir::new().unwrap();

        // Create a note with a broken link
        let note_path = dir.path().join("01HQ3K5M7N-note.md");
        std::fs::write(
            &note_path,
            note_with_link("9A", "Note A", "01ZZZZZZZZXJK4QZPW8V2R6T9X"),
        )
        .unwrap();

        let args = CheckArgs { fix: true };
        let result = handle_check(&args, dir.path());

        // Should succeed after fixing
        assert!(result.is_ok());

        // Verify the link was removed
        let content = std::fs::read_to_string(&note_path).unwrap();
        assert!(!content.contains("01ZZZZZZZZXJK4QZPW8V2R6T9X"));
        assert!(!content.contains("links:"));
    }

    #[test]
    fn handle_check_fix_removes_only_broken_links_keeps_valid() {
        let dir = TempDir::new().unwrap();

        // Create target note
        std::fs::write(
            dir.path().join("01HQ3K5M7N-target.md"),
            valid_note_content("9B", "Target"),
        )
        .unwrap();

        // Create a note with one valid and one broken link
        let note_path = dir.path().join("01HQ3K5M7N-source.md");
        std::fs::write(
            &note_path,
            note_with_multiple_links(
                "9A",
                "Source",
                &[
                    "01HQ3K5M7NXJK4QZPW8V2R6T9B", // valid
                    "01ZZZZZZZZXJK4QZPW8V2R6T9X", // broken
                ],
            ),
        )
        .unwrap();

        let args = CheckArgs { fix: true };
        let result = handle_check(&args, dir.path());

        assert!(result.is_ok());

        // Verify broken link was removed but valid link remains
        let content = std::fs::read_to_string(&note_path).unwrap();
        assert!(!content.contains("01ZZZZZZZZXJK4QZPW8V2R6T9X"));
        assert!(content.contains("01HQ3K5M7NXJK4QZPW8V2R6T9B"));
    }

    #[test]
    fn handle_check_fix_works_across_multiple_files() {
        let dir = TempDir::new().unwrap();

        // Create two notes with broken links
        let note1_path = dir.path().join("01HQ3K5M7N-note1.md");
        std::fs::write(
            &note1_path,
            note_with_link("9A", "Note 1", "01ZZZZZZZZXJK4QZPW8V2R6T9X"),
        )
        .unwrap();

        let note2_path = dir.path().join("01HQ3K5M7N-note2.md");
        std::fs::write(
            &note2_path,
            note_with_link("9B", "Note 2", "01ZZZZZZZZXJK4QZPW8V2R6T9Y"),
        )
        .unwrap();

        let args = CheckArgs { fix: true };
        let result = handle_check(&args, dir.path());

        assert!(result.is_ok());

        // Verify both files were fixed
        let content1 = std::fs::read_to_string(&note1_path).unwrap();
        let content2 = std::fs::read_to_string(&note2_path).unwrap();
        assert!(!content1.contains("01ZZZZZZZZXJK4QZPW8V2R6T9X"));
        assert!(!content2.contains("01ZZZZZZZZXJK4QZPW8V2R6T9Y"));
    }

    #[test]
    fn handle_check_fix_does_not_modify_files_without_broken_links() {
        let dir = TempDir::new().unwrap();

        // Create target note
        std::fs::write(
            dir.path().join("01HQ3K5M7N-target.md"),
            valid_note_content("9B", "Target"),
        )
        .unwrap();

        // Create source with valid link only
        let note_path = dir.path().join("01HQ3K5M7N-source.md");
        let original_content = note_with_link("9A", "Source", "01HQ3K5M7NXJK4QZPW8V2R6T9B");
        std::fs::write(&note_path, &original_content).unwrap();

        let args = CheckArgs { fix: true };
        let result = handle_check(&args, dir.path());

        assert!(result.is_ok());

        // Verify source file still contains the valid link
        let content = std::fs::read_to_string(&note_path).unwrap();
        assert!(content.contains("01HQ3K5M7NXJK4QZPW8V2R6T9B"));
    }

    #[test]
    fn handle_check_fix_does_not_fix_duplicate_ids() {
        let dir = TempDir::new().unwrap();

        // Create two notes with the same ID
        std::fs::write(
            dir.path().join("01HQ3K5M7N-first.md"),
            valid_note_content("9A", "First Note"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("01HQ3K5M7N-second.md"),
            valid_note_content("9A", "Second Note"),
        )
        .unwrap();

        let args = CheckArgs { fix: true };
        let result = handle_check(&args, dir.path());

        // Should still fail - duplicates are not auto-fixable
        assert!(result.is_err());
    }

    #[test]
    fn handle_check_fix_does_not_fix_orphans() {
        let dir = TempDir::new().unwrap();

        // Create orphan note
        let note_path = dir.path().join("01HQ3K5M7N-orphan.md");
        std::fs::write(&note_path, orphan_note_content("9A", "Orphan")).unwrap();

        let args = CheckArgs { fix: true };
        let result = handle_check(&args, dir.path());

        // Orphans are warnings, not errors - still succeeds
        assert!(result.is_ok());

        // Verify file wasn't modified (still no topics)
        let content = std::fs::read_to_string(&note_path).unwrap();
        assert!(!content.contains("topics:"));
    }

    #[test]
    fn handle_check_fix_preserves_body_content() {
        let dir = TempDir::new().unwrap();

        let note_path = dir.path().join("01HQ3K5M7N-note.md");
        let content_with_body = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9A
title: Note with Body
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
topics:
  - test/topic
links:
  - id: 01ZZZZZZZZXJK4QZPW8V2R6T9X
    rel:
      - see-also
---

# Important Heading

This is important body content that must be preserved.

- Bullet point 1
- Bullet point 2
"#;
        std::fs::write(&note_path, content_with_body).unwrap();

        let args = CheckArgs { fix: true };
        let result = handle_check(&args, dir.path());

        assert!(result.is_ok());

        // Verify body was preserved
        let content = std::fs::read_to_string(&note_path).unwrap();
        assert!(content.contains("# Important Heading"));
        assert!(content.contains("important body content"));
        assert!(content.contains("Bullet point 1"));
    }
}
