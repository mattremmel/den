//! Index builder for creating and updating the notes index from markdown files.

use crate::index::{IndexRepository, IndexResult, SqliteIndex};
use crate::infra::{ContentHash, FsError, read_note, scan_notes_directory};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ===========================================
// BuildError Type
// ===========================================

/// Errors that can occur when indexing individual files.
#[derive(Debug)]
pub enum BuildError {
    /// Failed to parse a note file.
    Parse { path: PathBuf, message: String },
    /// I/O error reading file.
    Io { path: PathBuf, message: String },
    /// Encoding error (UTF-16, lone CR, etc.).
    Encoding { path: PathBuf, message: String },
}

impl BuildError {
    /// Returns the path of the file that caused the error.
    pub fn path(&self) -> &Path {
        match self {
            BuildError::Parse { path, .. } => path,
            BuildError::Io { path, .. } => path,
            BuildError::Encoding { path, .. } => path,
        }
    }

    /// Returns the error message.
    pub fn message(&self) -> &str {
        match self {
            BuildError::Parse { message, .. } => message,
            BuildError::Io { message, .. } => message,
            BuildError::Encoding { message, .. } => message,
        }
    }
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path().display(), self.message())
    }
}

impl std::error::Error for BuildError {}

// ===========================================
// Result Types
// ===========================================

/// Result of a full index rebuild.
#[derive(Debug)]
pub struct BuildResult {
    /// Number of notes successfully indexed.
    pub indexed: usize,
    /// Errors that occurred during indexing.
    pub errors: Vec<BuildError>,
}

/// Result of an incremental index update.
#[derive(Debug)]
pub struct UpdateResult {
    /// Number of new notes added.
    pub added: usize,
    /// Number of existing notes updated.
    pub modified: usize,
    /// Number of notes removed (file deleted).
    pub removed: usize,
    /// Errors that occurred during indexing.
    pub errors: Vec<BuildError>,
}

// ===========================================
// Progress Reporting
// ===========================================

/// Result of processing a single file.
#[derive(Debug, Clone)]
pub enum FileResult {
    /// File was indexed successfully.
    Indexed,
    /// File was skipped (unchanged).
    Skipped,
    /// Error occurred while processing file.
    Error(String),
}

/// Trait for receiving progress updates during index operations.
pub trait ProgressReporter {
    /// Called when a file is processed.
    fn on_file(&mut self, path: &Path, result: FileResult);
    /// Called when the build/update is complete.
    fn on_complete(&mut self, indexed: usize, errors: usize);
}

/// A no-op progress reporter.
#[derive(Default)]
pub struct NoopReporter;

impl ProgressReporter for NoopReporter {
    fn on_file(&mut self, _path: &Path, _result: FileResult) {}
    fn on_complete(&mut self, _indexed: usize, _errors: usize) {}
}

// ===========================================
// IndexBuilder
// ===========================================

/// Builder for creating and updating the notes index.
///
/// The `IndexBuilder` scans a directory for markdown files and indexes them
/// into a `SqliteIndex`. It supports both full rebuilds (clearing and
/// re-indexing everything) and incremental updates (only processing changed
/// files).
pub struct IndexBuilder {
    notes_dir: PathBuf,
}

impl IndexBuilder {
    /// Creates a new IndexBuilder for the given notes directory.
    pub fn new(notes_dir: PathBuf) -> Self {
        Self { notes_dir }
    }

    /// Returns the notes directory.
    pub fn notes_dir(&self) -> &Path {
        &self.notes_dir
    }

    /// Performs a full rebuild of the index.
    ///
    /// This clears all existing data and re-indexes all markdown files in the
    /// notes directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the notes directory cannot be scanned or if a
    /// database operation fails. Individual file errors are collected in the
    /// returned `BuildResult`.
    pub fn full_rebuild(&self, index: &mut SqliteIndex) -> IndexResult<BuildResult> {
        self.full_rebuild_with_progress(index, &mut NoopReporter)
    }

    /// Performs a full rebuild with progress reporting.
    pub fn full_rebuild_with_progress<P: ProgressReporter>(
        &self,
        index: &mut SqliteIndex,
        progress: &mut P,
    ) -> IndexResult<BuildResult> {
        // Clear the index
        index.clear()?;

        // Scan directory for markdown files
        let files: Vec<PathBuf> = scan_notes_directory(&self.notes_dir)
            .map_err(|e| crate::index::IndexError::Io {
                path: self.notes_dir.clone(),
                source: std::io::Error::other(e.to_string()),
            })?
            .collect();

        let mut indexed = 0;
        let mut errors = Vec::new();

        for relative_path in files {
            let full_path = self.notes_dir.join(&relative_path);
            match read_note(&full_path) {
                Ok(parsed) => {
                    index.upsert_note(&parsed.note, &parsed.content_hash, &relative_path)?;
                    indexed += 1;
                    progress.on_file(&relative_path, FileResult::Indexed);
                }
                Err(e) => {
                    let build_error = fs_error_to_build_error(e, &relative_path);
                    progress.on_file(
                        &relative_path,
                        FileResult::Error(build_error.message().to_string()),
                    );
                    errors.push(build_error);
                }
            }
        }

        progress.on_complete(indexed, errors.len());
        Ok(BuildResult { indexed, errors })
    }

    /// Performs an incremental update of the index.
    ///
    /// This compares the current files on disk with the indexed files and only
    /// processes files that have changed (based on content hash).
    ///
    /// # Errors
    ///
    /// Returns an error if the notes directory cannot be scanned or if a
    /// database operation fails. Individual file errors are collected in the
    /// returned `UpdateResult`.
    pub fn incremental_update(&self, index: &mut SqliteIndex) -> IndexResult<UpdateResult> {
        self.incremental_update_with_progress(index, &mut NoopReporter)
    }

    /// Performs an incremental update with progress reporting.
    pub fn incremental_update_with_progress<P: ProgressReporter>(
        &self,
        index: &mut SqliteIndex,
        progress: &mut P,
    ) -> IndexResult<UpdateResult> {
        // Get all currently indexed paths with their hashes
        let indexed_paths: HashMap<PathBuf, ContentHash> =
            index.all_indexed_paths()?.into_iter().collect();

        // Scan current directory for markdown files
        let current_files: Vec<PathBuf> = scan_notes_directory(&self.notes_dir)
            .map_err(|e| crate::index::IndexError::Io {
                path: self.notes_dir.clone(),
                source: std::io::Error::other(e.to_string()),
            })?
            .collect();

        let current_files_set: std::collections::HashSet<PathBuf> =
            current_files.iter().cloned().collect();

        let mut added = 0;
        let mut modified = 0;
        let mut removed = 0;
        let mut errors = Vec::new();

        // Process current files
        for relative_path in &current_files {
            let full_path = self.notes_dir.join(relative_path);

            // Read the file to get the content hash
            match std::fs::read(&full_path) {
                Ok(bytes) => {
                    let current_hash = ContentHash::compute(&bytes);

                    match indexed_paths.get(relative_path) {
                        None => {
                            // New file - need to add
                            match read_note(&full_path) {
                                Ok(parsed) => {
                                    index.upsert_note(
                                        &parsed.note,
                                        &parsed.content_hash,
                                        relative_path,
                                    )?;
                                    added += 1;
                                    progress.on_file(relative_path, FileResult::Indexed);
                                }
                                Err(e) => {
                                    let build_error = fs_error_to_build_error(e, relative_path);
                                    progress.on_file(
                                        relative_path,
                                        FileResult::Error(build_error.message().to_string()),
                                    );
                                    errors.push(build_error);
                                }
                            }
                        }
                        Some(indexed_hash) if indexed_hash != &current_hash => {
                            // File changed - need to update
                            match read_note(&full_path) {
                                Ok(parsed) => {
                                    index.upsert_note(
                                        &parsed.note,
                                        &parsed.content_hash,
                                        relative_path,
                                    )?;
                                    modified += 1;
                                    progress.on_file(relative_path, FileResult::Indexed);
                                }
                                Err(e) => {
                                    let build_error = fs_error_to_build_error(e, relative_path);
                                    progress.on_file(
                                        relative_path,
                                        FileResult::Error(build_error.message().to_string()),
                                    );
                                    errors.push(build_error);
                                }
                            }
                        }
                        Some(_) => {
                            // File unchanged - skip
                            progress.on_file(relative_path, FileResult::Skipped);
                        }
                    }
                }
                Err(e) => {
                    let build_error = BuildError::Io {
                        path: relative_path.clone(),
                        message: e.to_string(),
                    };
                    progress.on_file(
                        relative_path,
                        FileResult::Error(build_error.message().to_string()),
                    );
                    errors.push(build_error);
                }
            }
        }

        // Remove files that no longer exist
        for indexed_path in indexed_paths.keys() {
            if !current_files_set.contains(indexed_path) && index.remove_by_path(indexed_path)? {
                removed += 1;
            }
        }

        progress.on_complete(added + modified, errors.len());
        Ok(UpdateResult {
            added,
            modified,
            removed,
            errors,
        })
    }
}

// ===========================================
// Helper Functions
// ===========================================

fn fs_error_to_build_error(error: FsError, path: &Path) -> BuildError {
    match error {
        FsError::Parse { source, .. } => BuildError::Parse {
            path: path.to_path_buf(),
            message: source.to_string(),
        },
        FsError::InvalidEncoding { encoding, .. } => BuildError::Encoding {
            path: path.to_path_buf(),
            message: encoding,
        },
        e => BuildError::Io {
            path: path.to_path_buf(),
            message: e.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::NoteId;
    use crate::index::IndexRepository;
    use std::fs;
    use tempfile::TempDir;

    // ===========================================
    // Test Helpers
    // ===========================================

    fn minimal_note_content(id: &str, title: &str) -> String {
        format!(
            r#"---
id: {}
title: {}
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
Body content."#,
            id, title
        )
    }

    fn create_note_file(dir: &Path, filename: &str, id: &str, title: &str) {
        let content = minimal_note_content(id, title);
        fs::write(dir.join(filename), content).unwrap();
    }

    // ===========================================
    // IndexBuilder Tests
    // ===========================================

    #[test]
    fn new_creates_builder_with_notes_dir() {
        let dir = PathBuf::from("/some/path");
        let builder = IndexBuilder::new(dir.clone());
        assert_eq!(builder.notes_dir(), &dir);
    }

    // ===========================================
    // full_rebuild Tests
    // ===========================================

    #[test]
    fn full_rebuild_empty_directory_produces_empty_index() {
        let dir = TempDir::new().unwrap();
        let builder = IndexBuilder::new(dir.path().to_path_buf());
        let mut index = SqliteIndex::open_in_memory().unwrap();

        let result = builder.full_rebuild(&mut index).unwrap();

        assert_eq!(result.indexed, 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn full_rebuild_single_note_is_indexed() {
        let dir = TempDir::new().unwrap();
        create_note_file(
            dir.path(),
            "note.md",
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Test Note",
        );

        let builder = IndexBuilder::new(dir.path().to_path_buf());
        let mut index = SqliteIndex::open_in_memory().unwrap();

        let result = builder.full_rebuild(&mut index).unwrap();

        assert_eq!(result.indexed, 1);
        assert!(result.errors.is_empty());

        let note_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
        let note = index.get_note(&note_id).unwrap();
        assert!(note.is_some());
        assert_eq!(note.unwrap().title(), "Test Note");
    }

    #[test]
    fn full_rebuild_multiple_notes_all_indexed() {
        let dir = TempDir::new().unwrap();
        create_note_file(
            dir.path(),
            "note1.md",
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Note One",
        );
        create_note_file(
            dir.path(),
            "note2.md",
            "01HQ4A2R9PXJK4QZPW8V2R6T9Z",
            "Note Two",
        );
        create_note_file(
            dir.path(),
            "note3.md",
            "01HQ5B3S1QXJK4QZPW8V2R6T0A",
            "Note Three",
        );

        let builder = IndexBuilder::new(dir.path().to_path_buf());
        let mut index = SqliteIndex::open_in_memory().unwrap();

        let result = builder.full_rebuild(&mut index).unwrap();

        assert_eq!(result.indexed, 3);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn full_rebuild_invalid_notes_collected_as_errors() {
        let dir = TempDir::new().unwrap();
        create_note_file(
            dir.path(),
            "good.md",
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Good Note",
        );
        // Invalid note without proper frontmatter
        fs::write(dir.path().join("bad.md"), "No frontmatter here").unwrap();

        let builder = IndexBuilder::new(dir.path().to_path_buf());
        let mut index = SqliteIndex::open_in_memory().unwrap();

        let result = builder.full_rebuild(&mut index).unwrap();

        assert_eq!(result.indexed, 1);
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(result.errors[0], BuildError::Parse { .. }));
    }

    #[test]
    fn full_rebuild_clears_existing_index() {
        let dir = TempDir::new().unwrap();
        create_note_file(
            dir.path(),
            "note.md",
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Original Note",
        );

        let builder = IndexBuilder::new(dir.path().to_path_buf());
        let mut index = SqliteIndex::open_in_memory().unwrap();

        // First build
        builder.full_rebuild(&mut index).unwrap();

        // Remove the note file and add a different one
        fs::remove_file(dir.path().join("note.md")).unwrap();
        create_note_file(
            dir.path(),
            "new.md",
            "01HQ4A2R9PXJK4QZPW8V2R6T9Z",
            "New Note",
        );

        // Rebuild
        let result = builder.full_rebuild(&mut index).unwrap();

        assert_eq!(result.indexed, 1);

        // Old note should be gone
        let old_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
        assert!(index.get_note(&old_id).unwrap().is_none());

        // New note should exist
        let new_id: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Z".parse().unwrap();
        assert!(index.get_note(&new_id).unwrap().is_some());
    }

    #[test]
    fn full_rebuild_indexes_notes_in_subdirectories() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        create_note_file(
            dir.path(),
            "root.md",
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Root Note",
        );
        create_note_file(
            &dir.path().join("subdir"),
            "nested.md",
            "01HQ4A2R9PXJK4QZPW8V2R6T9Z",
            "Nested Note",
        );

        let builder = IndexBuilder::new(dir.path().to_path_buf());
        let mut index = SqliteIndex::open_in_memory().unwrap();

        let result = builder.full_rebuild(&mut index).unwrap();

        assert_eq!(result.indexed, 2);
    }

    // ===========================================
    // incremental_update Tests
    // ===========================================

    #[test]
    fn incremental_update_new_file_detected_and_added() {
        let dir = TempDir::new().unwrap();
        let builder = IndexBuilder::new(dir.path().to_path_buf());
        let mut index = SqliteIndex::open_in_memory().unwrap();

        // Initial empty index
        builder.full_rebuild(&mut index).unwrap();

        // Add a new file
        create_note_file(
            dir.path(),
            "new.md",
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "New Note",
        );

        let result = builder.incremental_update(&mut index).unwrap();

        assert_eq!(result.added, 1);
        assert_eq!(result.modified, 0);
        assert_eq!(result.removed, 0);
    }

    #[test]
    fn incremental_update_modified_file_detected_and_updated() {
        let dir = TempDir::new().unwrap();
        create_note_file(
            dir.path(),
            "note.md",
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Original Title",
        );

        let builder = IndexBuilder::new(dir.path().to_path_buf());
        let mut index = SqliteIndex::open_in_memory().unwrap();

        builder.full_rebuild(&mut index).unwrap();

        // Modify the file
        let new_content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Modified Title
created: 2024-01-15T10:30:00Z
modified: 2024-01-16T10:30:00Z
---
Updated body."#;
        fs::write(dir.path().join("note.md"), new_content).unwrap();

        let result = builder.incremental_update(&mut index).unwrap();

        assert_eq!(result.added, 0);
        assert_eq!(result.modified, 1);
        assert_eq!(result.removed, 0);

        // Verify the title was updated
        let note_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
        let note = index.get_note(&note_id).unwrap().unwrap();
        assert_eq!(note.title(), "Modified Title");
    }

    #[test]
    fn incremental_update_unchanged_file_skipped() {
        let dir = TempDir::new().unwrap();
        create_note_file(
            dir.path(),
            "note.md",
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Test Note",
        );

        let builder = IndexBuilder::new(dir.path().to_path_buf());
        let mut index = SqliteIndex::open_in_memory().unwrap();

        builder.full_rebuild(&mut index).unwrap();

        // Update without changing the file
        let result = builder.incremental_update(&mut index).unwrap();

        assert_eq!(result.added, 0);
        assert_eq!(result.modified, 0);
        assert_eq!(result.removed, 0);
    }

    #[test]
    fn incremental_update_deleted_file_removed_from_index() {
        let dir = TempDir::new().unwrap();
        create_note_file(
            dir.path(),
            "note.md",
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Test Note",
        );

        let builder = IndexBuilder::new(dir.path().to_path_buf());
        let mut index = SqliteIndex::open_in_memory().unwrap();

        builder.full_rebuild(&mut index).unwrap();

        // Delete the file
        fs::remove_file(dir.path().join("note.md")).unwrap();

        let result = builder.incremental_update(&mut index).unwrap();

        assert_eq!(result.added, 0);
        assert_eq!(result.modified, 0);
        assert_eq!(result.removed, 1);

        // Verify note is removed
        let note_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
        assert!(index.get_note(&note_id).unwrap().is_none());
    }

    #[test]
    fn incremental_update_mixed_operations() {
        let dir = TempDir::new().unwrap();
        create_note_file(
            dir.path(),
            "keep.md",
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Keep Note",
        );
        create_note_file(
            dir.path(),
            "modify.md",
            "01HQ4A2R9PXJK4QZPW8V2R6T9Z",
            "Modify Note",
        );
        create_note_file(
            dir.path(),
            "delete.md",
            "01HQ5B3S1QXJK4QZPW8V2R6T0A",
            "Delete Note",
        );

        let builder = IndexBuilder::new(dir.path().to_path_buf());
        let mut index = SqliteIndex::open_in_memory().unwrap();

        builder.full_rebuild(&mut index).unwrap();

        // Delete one file
        fs::remove_file(dir.path().join("delete.md")).unwrap();

        // Modify one file
        let modified_content = r#"---
id: 01HQ4A2R9PXJK4QZPW8V2R6T9Z
title: Modified Note
created: 2024-01-15T10:30:00Z
modified: 2024-01-16T10:30:00Z
---
Changed body."#;
        fs::write(dir.path().join("modify.md"), modified_content).unwrap();

        // Add a new file
        create_note_file(
            dir.path(),
            "new.md",
            "01HQ6C4T2RXJK4QZPW8V2R6T1B",
            "New Note",
        );

        let result = builder.incremental_update(&mut index).unwrap();

        assert_eq!(result.added, 1);
        assert_eq!(result.modified, 1);
        assert_eq!(result.removed, 1);
    }

    // ===========================================
    // BuildError Tests
    // ===========================================

    #[test]
    fn build_error_path_returns_correct_path() {
        let path = PathBuf::from("test/note.md");
        let error = BuildError::Parse {
            path: path.clone(),
            message: "some error".to_string(),
        };
        assert_eq!(error.path(), &path);
    }

    #[test]
    fn build_error_message_returns_correct_message() {
        let error = BuildError::Io {
            path: PathBuf::from("test.md"),
            message: "file not found".to_string(),
        };
        assert_eq!(error.message(), "file not found");
    }

    #[test]
    fn build_error_display_includes_path_and_message() {
        let error = BuildError::Encoding {
            path: PathBuf::from("test.md"),
            message: "UTF-16 detected".to_string(),
        };
        let display = error.to_string();
        assert!(display.contains("test.md"));
        assert!(display.contains("UTF-16 detected"));
    }

    // ===========================================
    // Progress Reporting Tests
    // ===========================================

    struct TestReporter {
        files: Vec<(PathBuf, FileResult)>,
        complete_called: bool,
        final_indexed: usize,
        final_errors: usize,
    }

    impl TestReporter {
        fn new() -> Self {
            Self {
                files: Vec::new(),
                complete_called: false,
                final_indexed: 0,
                final_errors: 0,
            }
        }
    }

    impl ProgressReporter for TestReporter {
        fn on_file(&mut self, path: &Path, result: FileResult) {
            self.files.push((path.to_path_buf(), result));
        }

        fn on_complete(&mut self, indexed: usize, errors: usize) {
            self.complete_called = true;
            self.final_indexed = indexed;
            self.final_errors = errors;
        }
    }

    #[test]
    fn progress_callback_invoked_for_each_file() {
        let dir = TempDir::new().unwrap();
        create_note_file(
            dir.path(),
            "note1.md",
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Note One",
        );
        create_note_file(
            dir.path(),
            "note2.md",
            "01HQ4A2R9PXJK4QZPW8V2R6T9Z",
            "Note Two",
        );

        let builder = IndexBuilder::new(dir.path().to_path_buf());
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let mut reporter = TestReporter::new();

        builder
            .full_rebuild_with_progress(&mut index, &mut reporter)
            .unwrap();

        assert_eq!(reporter.files.len(), 2);
    }

    #[test]
    fn progress_callback_receives_correct_result_types() {
        let dir = TempDir::new().unwrap();
        create_note_file(
            dir.path(),
            "good.md",
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Good Note",
        );
        fs::write(dir.path().join("bad.md"), "invalid content").unwrap();

        let builder = IndexBuilder::new(dir.path().to_path_buf());
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let mut reporter = TestReporter::new();

        builder
            .full_rebuild_with_progress(&mut index, &mut reporter)
            .unwrap();

        let indexed_count = reporter
            .files
            .iter()
            .filter(|(_, r)| matches!(r, FileResult::Indexed))
            .count();
        let error_count = reporter
            .files
            .iter()
            .filter(|(_, r)| matches!(r, FileResult::Error(_)))
            .count();

        assert_eq!(indexed_count, 1);
        assert_eq!(error_count, 1);
    }

    #[test]
    fn progress_on_complete_called_with_final_counts() {
        let dir = TempDir::new().unwrap();
        create_note_file(
            dir.path(),
            "good.md",
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Good Note",
        );
        fs::write(dir.path().join("bad.md"), "invalid content").unwrap();

        let builder = IndexBuilder::new(dir.path().to_path_buf());
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let mut reporter = TestReporter::new();

        builder
            .full_rebuild_with_progress(&mut index, &mut reporter)
            .unwrap();

        assert!(reporter.complete_called);
        assert_eq!(reporter.final_indexed, 1);
        assert_eq!(reporter.final_errors, 1);
    }

    #[test]
    fn incremental_progress_reports_skipped_files() {
        let dir = TempDir::new().unwrap();
        create_note_file(
            dir.path(),
            "note.md",
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y",
            "Test Note",
        );

        let builder = IndexBuilder::new(dir.path().to_path_buf());
        let mut index = SqliteIndex::open_in_memory().unwrap();

        builder.full_rebuild(&mut index).unwrap();

        let mut reporter = TestReporter::new();
        builder
            .incremental_update_with_progress(&mut index, &mut reporter)
            .unwrap();

        assert_eq!(reporter.files.len(), 1);
        assert!(matches!(reporter.files[0].1, FileResult::Skipped));
    }

    // ===========================================
    // Integration Tests
    // ===========================================

    #[test]
    fn full_integration_test() {
        let dir = TempDir::new().unwrap();

        // Create test notes
        create_note_file(dir.path(), "a.md", "01HQ3K5M7NXJK4QZPW8V2R6T9Y", "Note A");
        create_note_file(dir.path(), "b.md", "01HQ4A2R9PXJK4QZPW8V2R6T9Z", "Note B");

        // Full rebuild
        let builder = IndexBuilder::new(dir.path().to_path_buf());
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let result = builder.full_rebuild(&mut index).unwrap();
        assert_eq!(result.indexed, 2);

        // Verify indexed
        let note_a_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
        assert!(index.get_note(&note_a_id).unwrap().is_some());

        // Modify a file
        let modified_content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Modified Note A
created: 2024-01-15T10:30:00Z
modified: 2024-01-16T10:30:00Z
---
New content."#;
        fs::write(dir.path().join("a.md"), modified_content).unwrap();

        // Incremental update
        let result = builder.incremental_update(&mut index).unwrap();
        assert_eq!(result.modified, 1);
        assert_eq!(result.added, 0);
        assert_eq!(result.removed, 0);

        // Verify modification
        let note = index.get_note(&note_a_id).unwrap().unwrap();
        assert_eq!(note.title(), "Modified Note A");
    }
}
