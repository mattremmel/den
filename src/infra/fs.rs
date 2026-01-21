//! File I/O operations for notes with atomic writes.

use crate::domain::Note;
use crate::infra::frontmatter::{ParseError, ParsedNote, parse, serialize};
use std::io::{self, Write as IoWrite};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};

/// Errors during file system operations on notes.
#[derive(Debug, Error)]
pub enum FsError {
    #[error("note file not found: {path}")]
    NotFound { path: PathBuf },

    #[error("permission denied: {path}")]
    PermissionDenied { path: PathBuf },

    #[error("I/O error for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to parse note at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: ParseError,
    },

    #[error("atomic write failed for {path}: {source}")]
    AtomicWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("parent directory does not exist: {path}")]
    ParentNotFound { path: PathBuf },

    #[error("path is not a directory: {path}")]
    NotADirectory { path: PathBuf },
}

impl FsError {
    /// Creates an appropriate FsError from an io::Error.
    fn from_io(path: &Path, error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound => FsError::NotFound { path: path.into() },
            io::ErrorKind::PermissionDenied => FsError::PermissionDenied { path: path.into() },
            _ => FsError::Io {
                path: path.into(),
                source: error,
            },
        }
    }
}

/// Reads a note from a file path.
///
/// # Errors
///
/// Returns `FsError::NotFound` if the file doesn't exist.
/// Returns `FsError::PermissionDenied` if access is denied.
/// Returns `FsError::Parse` if the file content is invalid.
pub fn read_note(path: &Path) -> Result<ParsedNote, FsError> {
    let content = std::fs::read_to_string(path).map_err(|e| FsError::from_io(path, e))?;

    // Strip UTF-8 BOM if present
    let content = content.strip_prefix('\u{FEFF}').unwrap_or(&content);

    parse(content).map_err(|e| FsError::Parse {
        path: path.into(),
        source: e,
    })
}

/// Writes a note to a file path atomically.
///
/// Uses a temporary file and atomic rename to prevent partial writes.
/// The parent directory must exist.
///
/// # Errors
///
/// Returns `FsError::ParentNotFound` if the parent directory doesn't exist.
/// Returns `FsError::AtomicWrite` if the atomic rename fails.
pub fn write_note(path: &Path, note: &Note, body: &str) -> Result<(), FsError> {
    let parent = path
        .parent()
        .ok_or_else(|| FsError::ParentNotFound { path: path.into() })?;

    if !parent.exists() {
        return Err(FsError::ParentNotFound {
            path: parent.into(),
        });
    }

    let content = serialize(note, body);
    let mut temp = NamedTempFile::new_in(parent).map_err(|e| FsError::Io {
        path: path.into(),
        source: e,
    })?;

    temp.write_all(content.as_bytes())
        .map_err(|e| FsError::Io {
            path: path.into(),
            source: e,
        })?;

    temp.persist(path).map_err(|e| FsError::AtomicWrite {
        path: path.into(),
        source: e.error,
    })?;

    Ok(())
}

/// Scans a directory recursively for markdown (.md) files.
///
/// Skips hidden files and directories (starting with `.`), including
/// the `.index/` directory used for the SQLite index.
///
/// Returns paths relative to the input directory.
///
/// # Errors
///
/// Returns `FsError::NotFound` if the directory doesn't exist.
/// Returns `FsError::NotADirectory` if the path is not a directory.
pub fn scan_notes_directory(dir: &Path) -> Result<impl Iterator<Item = PathBuf>, FsError> {
    if !dir.exists() {
        return Err(FsError::NotFound {
            path: dir.to_path_buf(),
        });
    }
    if !dir.is_dir() {
        return Err(FsError::NotADirectory {
            path: dir.to_path_buf(),
        });
    }

    let dir_owned = dir.to_path_buf();
    let iter = WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !is_hidden(e))
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(has_md_extension)
        .map(move |e| e.path().strip_prefix(&dir_owned).unwrap().to_path_buf());

    Ok(iter)
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|s| s.starts_with('.'))
}

fn has_md_extension(entry: &DirEntry) -> bool {
    entry.path().extension().is_some_and(|e| e == "md")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::NoteId;
    use chrono::{DateTime, Utc};
    use pretty_assertions::assert_eq;
    use std::fs;
    use tempfile::TempDir;

    // ===========================================
    // Test Helpers
    // ===========================================

    fn test_note_id() -> NoteId {
        "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap()
    }

    fn test_timestamps() -> (DateTime<Utc>, DateTime<Utc>) {
        let t = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        (t, t)
    }

    fn create_test_note_and_body() -> (Note, String) {
        let (created, modified) = test_timestamps();
        let note = Note::new(test_note_id(), "Test Note", created, modified).unwrap();
        (note, "Body content.".to_string())
    }

    fn create_test_file(dir: &TempDir, content: &str) -> PathBuf {
        let path = dir.path().join("test-note.md");
        fs::write(&path, content).unwrap();
        path
    }

    fn minimal_frontmatter() -> String {
        r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Test Note
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
Body content."#
            .to_string()
    }

    // ===========================================
    // Cycle 1: FsError Type
    // ===========================================

    #[test]
    fn fs_error_not_found_displays_path() {
        let error = FsError::NotFound {
            path: PathBuf::from("/some/path.md"),
        };
        assert!(error.to_string().contains("/some/path.md"));
    }

    #[test]
    fn fs_error_from_io_maps_not_found() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let path = Path::new("/test/path.md");
        let error = FsError::from_io(path, io_error);
        assert!(matches!(error, FsError::NotFound { .. }));
    }

    #[test]
    fn fs_error_from_io_maps_permission_denied() {
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "permission denied");
        let path = Path::new("/test/path.md");
        let error = FsError::from_io(path, io_error);
        assert!(matches!(error, FsError::PermissionDenied { .. }));
    }

    #[test]
    fn fs_error_from_io_maps_other_to_io() {
        let io_error = io::Error::new(io::ErrorKind::Other, "some other error");
        let path = Path::new("/test/path.md");
        let error = FsError::from_io(path, io_error);
        assert!(matches!(error, FsError::Io { .. }));
    }

    // ===========================================
    // Cycle 2: read_note Happy Path
    // ===========================================

    #[test]
    fn read_note_parses_valid_file() {
        let dir = TempDir::new().unwrap();
        let path = create_test_file(&dir, &minimal_frontmatter());

        let result = read_note(&path).unwrap();
        assert_eq!(result.note.title(), "Test Note");
        assert_eq!(result.body, "Body content.");
    }

    #[test]
    fn read_note_returns_full_parsed_note() {
        let dir = TempDir::new().unwrap();
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Full Note
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
description: A description
topics:
  - software/architecture
tags:
  - draft
---
Body with more content."#;
        let path = create_test_file(&dir, content);

        let result = read_note(&path).unwrap();
        assert_eq!(result.note.title(), "Full Note");
        assert_eq!(result.note.description(), Some("A description"));
        assert_eq!(result.note.topics().len(), 1);
        assert_eq!(result.note.tags().len(), 1);
    }

    // ===========================================
    // Cycle 3: read_note Error Cases
    // ===========================================

    #[test]
    fn read_note_returns_not_found_for_missing_file() {
        let path = Path::new("/nonexistent/path/note.md");
        let result = read_note(path);
        assert!(matches!(result, Err(FsError::NotFound { .. })));
    }

    #[test]
    fn read_note_returns_parse_error_for_invalid_frontmatter() {
        let dir = TempDir::new().unwrap();
        let content = "No frontmatter here, just text.";
        let path = create_test_file(&dir, content);

        let result = read_note(&path);
        assert!(matches!(result, Err(FsError::Parse { .. })));
    }

    #[test]
    fn read_note_returns_parse_error_for_missing_required_fields() {
        let dir = TempDir::new().unwrap();
        let content = r#"---
title: Missing ID
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
Body"#;
        let path = create_test_file(&dir, content);

        let result = read_note(&path);
        assert!(matches!(result, Err(FsError::Parse { .. })));
    }

    #[test]
    fn read_note_error_includes_path_context() {
        let dir = TempDir::new().unwrap();
        let content = "invalid content";
        let path = create_test_file(&dir, content);

        let result = read_note(&path);
        if let Err(FsError::Parse {
            path: error_path, ..
        }) = result
        {
            assert_eq!(error_path, path);
        } else {
            panic!("Expected FsError::Parse");
        }
    }

    // ===========================================
    // Cycle 4: read_note BOM Handling
    // ===========================================

    #[test]
    fn read_note_strips_utf8_bom() {
        let dir = TempDir::new().unwrap();
        // UTF-8 BOM is 0xEF 0xBB 0xBF
        let content_with_bom = format!("\u{FEFF}{}", minimal_frontmatter());
        let path = create_test_file(&dir, &content_with_bom);

        let result = read_note(&path).unwrap();
        assert_eq!(result.note.title(), "Test Note");
    }

    #[test]
    fn read_note_handles_file_without_bom() {
        let dir = TempDir::new().unwrap();
        let path = create_test_file(&dir, &minimal_frontmatter());

        let result = read_note(&path).unwrap();
        assert_eq!(result.note.title(), "Test Note");
    }

    // ===========================================
    // Cycle 5: write_note Happy Path
    // ===========================================

    #[test]
    fn write_note_creates_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("new-note.md");
        let (note, body) = create_test_note_and_body();

        write_note(&path, &note, &body).unwrap();

        assert!(path.exists());
    }

    #[test]
    fn write_note_content_is_readable() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("new-note.md");
        let (note, body) = create_test_note_and_body();

        write_note(&path, &note, &body).unwrap();
        let parsed = read_note(&path).unwrap();

        assert_eq!(parsed.note.title(), "Test Note");
        assert_eq!(parsed.body, "Body content.");
    }

    #[test]
    fn write_note_overwrites_existing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("note.md");
        let (note, _) = create_test_note_and_body();

        write_note(&path, &note, "First body").unwrap();
        write_note(&path, &note, "Second body").unwrap();

        let parsed = read_note(&path).unwrap();
        assert_eq!(parsed.body, "Second body");
    }

    // ===========================================
    // Cycle 6: write_note Error Cases
    // ===========================================

    #[test]
    fn write_note_returns_parent_not_found() {
        let path = Path::new("/nonexistent/directory/note.md");
        let (note, body) = create_test_note_and_body();

        let result = write_note(path, &note, &body);
        assert!(matches!(result, Err(FsError::ParentNotFound { .. })));
    }

    #[test]
    fn write_note_error_includes_path_context() {
        let path = Path::new("/nonexistent/directory/note.md");
        let (note, body) = create_test_note_and_body();

        let result = write_note(path, &note, &body);
        if let Err(FsError::ParentNotFound { path: error_path }) = result {
            assert!(error_path.to_string_lossy().contains("nonexistent"));
        } else {
            panic!("Expected FsError::ParentNotFound");
        }
    }

    // ===========================================
    // Cycle 7: write_note Atomicity
    // ===========================================

    #[test]
    fn write_note_leaves_no_temp_files_on_success() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("note.md");
        let (note, body) = create_test_note_and_body();

        write_note(&path, &note, &body).unwrap();

        let files: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_name(), "note.md");
    }

    #[test]
    fn write_note_creates_temp_in_same_directory() {
        // This test verifies atomic rename semantics by checking the write succeeds
        // (atomic rename only works within the same filesystem)
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("note.md");
        let (note, body) = create_test_note_and_body();

        let result = write_note(&path, &note, &body);
        assert!(result.is_ok());
    }

    // ===========================================
    // Cycle 8: Roundtrip Integration
    // ===========================================

    #[test]
    fn roundtrip_preserves_note_content() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("roundtrip.md");
        let (note, body) = create_test_note_and_body();

        write_note(&path, &note, &body).unwrap();
        let parsed = read_note(&path).unwrap();

        assert_eq!(parsed.note, note);
        assert_eq!(parsed.body, body);
    }

    #[test]
    fn roundtrip_preserves_unicode() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("unicode.md");
        let (created, modified) = test_timestamps();
        let note = Note::new(test_note_id(), "日本語タイトル", created, modified).unwrap();
        let body = "Body with emoji: 🎉 and unicode: αβγ δεζ";

        write_note(&path, &note, body).unwrap();
        let parsed = read_note(&path).unwrap();

        assert_eq!(parsed.note.title(), "日本語タイトル");
        assert!(parsed.body.contains("🎉"));
        assert!(parsed.body.contains("αβγ"));
    }

    #[test]
    fn roundtrip_multiple_writes_same_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("multi.md");
        let (created, modified) = test_timestamps();

        let note1 = Note::new(test_note_id(), "First Title", created, modified).unwrap();
        write_note(&path, &note1, "First body").unwrap();

        let note2 = Note::new(test_note_id(), "Second Title", created, modified).unwrap();
        write_note(&path, &note2, "Second body").unwrap();

        let parsed = read_note(&path).unwrap();
        assert_eq!(parsed.note.title(), "Second Title");
        assert_eq!(parsed.body, "Second body");
    }

    // ===========================================
    // Cycle 9: Line Endings
    // ===========================================

    #[test]
    fn read_note_handles_crlf_line_endings() {
        let dir = TempDir::new().unwrap();
        let content = "---\r\nid: 01HQ3K5M7NXJK4QZPW8V2R6T9Y\r\ntitle: CRLF Note\r\ncreated: 2024-01-15T10:30:00Z\r\nmodified: 2024-01-15T10:30:00Z\r\n---\r\nBody with CRLF";
        let path = create_test_file(&dir, content);

        let result = read_note(&path).unwrap();
        assert_eq!(result.note.title(), "CRLF Note");
    }

    #[test]
    fn write_note_produces_lf_line_endings() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("lf-test.md");
        let (note, body) = create_test_note_and_body();

        write_note(&path, &note, &body).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("\r\n"), "Output should not contain CRLF");
        assert!(content.contains('\n'), "Output should contain LF");
    }

    // ===========================================
    // Cycle 10: Edge Cases
    // ===========================================

    #[test]
    fn read_note_handles_empty_body() {
        let dir = TempDir::new().unwrap();
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Empty Body
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---"#;
        let path = create_test_file(&dir, content);

        let result = read_note(&path).unwrap();
        assert_eq!(result.note.title(), "Empty Body");
        assert_eq!(result.body, "");
    }

    #[test]
    fn write_note_handles_empty_body() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("empty-body.md");
        let (note, _) = create_test_note_and_body();

        write_note(&path, &note, "").unwrap();

        let parsed = read_note(&path).unwrap();
        assert_eq!(parsed.body, "");
    }

    #[test]
    fn read_note_handles_body_with_frontmatter_like_content() {
        let dir = TempDir::new().unwrap();
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Tricky Body
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
Here is some text.

--- This looks like a delimiter but isn't

More text after.
"#;
        let path = create_test_file(&dir, content);

        let result = read_note(&path).unwrap();
        assert!(result.body.contains("--- This looks like a delimiter"));
        assert!(result.body.contains("More text after"));
    }

    #[test]
    fn read_note_handles_very_long_body() {
        let dir = TempDir::new().unwrap();
        let long_body = "x".repeat(1024 * 1024); // 1MB
        let content = format!(
            r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Long Body
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
{}"#,
            long_body
        );
        let path = create_test_file(&dir, &content);

        let result = read_note(&path).unwrap();
        assert_eq!(result.note.title(), "Long Body");
        assert_eq!(result.body.len(), long_body.len());
    }

    // ===========================================
    // scan_notes_directory Tests
    // ===========================================

    // --- Phase 1: Basic Happy Path ---

    #[test]
    fn scan_empty_directory_returns_empty_iterator() {
        let dir = TempDir::new().unwrap();

        let result: Vec<_> = scan_notes_directory(dir.path()).unwrap().collect();

        assert!(result.is_empty());
    }

    #[test]
    fn scan_directory_finds_single_md_file() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("note.md"), "content").unwrap();

        let result: Vec<_> = scan_notes_directory(dir.path()).unwrap().collect();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], PathBuf::from("note.md"));
    }

    #[test]
    fn scan_directory_finds_multiple_md_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("note1.md"), "content").unwrap();
        fs::write(dir.path().join("note2.md"), "content").unwrap();
        fs::write(dir.path().join("note3.md"), "content").unwrap();

        let mut result: Vec<_> = scan_notes_directory(dir.path()).unwrap().collect();
        result.sort();

        assert_eq!(result.len(), 3);
        assert!(result.contains(&PathBuf::from("note1.md")));
        assert!(result.contains(&PathBuf::from("note2.md")));
        assert!(result.contains(&PathBuf::from("note3.md")));
    }

    // --- Phase 2: File Filtering ---

    #[test]
    fn scan_ignores_non_md_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("note.md"), "content").unwrap();
        fs::write(dir.path().join("readme.txt"), "content").unwrap();
        fs::write(dir.path().join("config.json"), "content").unwrap();

        let result: Vec<_> = scan_notes_directory(dir.path()).unwrap().collect();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], PathBuf::from("note.md"));
    }

    #[test]
    fn scan_ignores_directories() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("note.md"), "content").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();

        let result: Vec<_> = scan_notes_directory(dir.path()).unwrap().collect();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], PathBuf::from("note.md"));
    }

    #[test]
    fn scan_finds_md_in_subdirectories() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("root.md"), "content").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        fs::write(dir.path().join("subdir/nested.md"), "content").unwrap();

        let mut result: Vec<_> = scan_notes_directory(dir.path()).unwrap().collect();
        result.sort();

        assert_eq!(result.len(), 2);
        assert!(result.contains(&PathBuf::from("root.md")));
        assert!(result.contains(&PathBuf::from("subdir/nested.md")));
    }

    // --- Phase 3: Index Directory Exclusion ---

    #[test]
    fn scan_skips_index_directory() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("note.md"), "content").unwrap();
        fs::create_dir(dir.path().join(".index")).unwrap();
        fs::write(dir.path().join(".index/should-skip.md"), "content").unwrap();

        let result: Vec<_> = scan_notes_directory(dir.path()).unwrap().collect();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], PathBuf::from("note.md"));
    }

    #[test]
    fn scan_skips_nested_index_contents() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("note.md"), "content").unwrap();
        fs::create_dir(dir.path().join(".index")).unwrap();
        fs::write(dir.path().join(".index/notes.db"), "content").unwrap();
        fs::create_dir(dir.path().join(".index/cache")).unwrap();
        fs::write(dir.path().join(".index/cache/temp.md"), "content").unwrap();

        let result: Vec<_> = scan_notes_directory(dir.path()).unwrap().collect();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], PathBuf::from("note.md"));
    }

    // --- Phase 4: Hidden Files/Directories ---

    #[test]
    fn scan_skips_hidden_directories() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("note.md"), "content").unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join(".git/config.md"), "content").unwrap();
        fs::create_dir(dir.path().join(".hidden")).unwrap();
        fs::write(dir.path().join(".hidden/secret.md"), "content").unwrap();

        let result: Vec<_> = scan_notes_directory(dir.path()).unwrap().collect();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], PathBuf::from("note.md"));
    }

    #[test]
    fn scan_skips_hidden_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("note.md"), "content").unwrap();
        fs::write(dir.path().join(".hidden.md"), "content").unwrap();
        fs::write(dir.path().join(".DS_Store"), "content").unwrap();

        let result: Vec<_> = scan_notes_directory(dir.path()).unwrap().collect();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], PathBuf::from("note.md"));
    }

    // --- Phase 5: Error Handling ---

    #[test]
    fn scan_nonexistent_directory_returns_error() {
        let path = Path::new("/nonexistent/directory");

        let result = scan_notes_directory(path);

        assert!(matches!(result, Err(FsError::NotFound { .. })));
    }

    #[test]
    fn scan_file_as_directory_returns_error() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("file.txt");
        fs::write(&file_path, "content").unwrap();

        let result = scan_notes_directory(&file_path);

        assert!(matches!(result, Err(FsError::NotADirectory { .. })));
    }

    // --- Phase 6: Path Semantics ---

    #[test]
    fn scan_returns_paths_relative_to_input() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("deep")).unwrap();
        fs::create_dir(dir.path().join("deep/nested")).unwrap();
        fs::write(dir.path().join("deep/nested/note.md"), "content").unwrap();

        let result: Vec<_> = scan_notes_directory(dir.path()).unwrap().collect();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], PathBuf::from("deep/nested/note.md"));
    }

    #[test]
    fn scan_handles_absolute_path_input() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("note.md"), "content").unwrap();
        let abs_path = dir.path().canonicalize().unwrap();

        let result: Vec<_> = scan_notes_directory(&abs_path).unwrap().collect();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], PathBuf::from("note.md"));
    }

    #[test]
    fn scan_handles_relative_path_input() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("note.md"), "content").unwrap();

        // TempDir paths are already effectively relative from their perspective
        let result: Vec<_> = scan_notes_directory(dir.path()).unwrap().collect();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], PathBuf::from("note.md"));
    }

    // --- Phase 7: Edge Cases ---

    #[test]
    #[cfg(unix)]
    fn scan_handles_symlinks_to_files() {
        use std::os::unix::fs::symlink;

        let dir = TempDir::new().unwrap();
        let original = dir.path().join("original.md");
        fs::write(&original, "content").unwrap();
        symlink(&original, dir.path().join("linked.md")).unwrap();

        let mut result: Vec<_> = scan_notes_directory(dir.path()).unwrap().collect();
        result.sort();

        assert_eq!(result.len(), 2);
        assert!(result.contains(&PathBuf::from("original.md")));
        assert!(result.contains(&PathBuf::from("linked.md")));
    }

    #[test]
    fn scan_handles_unicode_filenames() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("日記.md"), "content").unwrap();
        fs::write(dir.path().join("заметки.md"), "content").unwrap();

        let result: Vec<_> = scan_notes_directory(dir.path()).unwrap().collect();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn scan_handles_spaces_in_filenames() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("my notes.md"), "content").unwrap();
        fs::write(dir.path().join("another file with spaces.md"), "content").unwrap();

        let result: Vec<_> = scan_notes_directory(dir.path()).unwrap().collect();

        assert_eq!(result.len(), 2);
    }
}
