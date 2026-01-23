//! Isolated test environment with temp directory.

use super::{DenCommand, TestNote};
use anyhow::Result;
use den::index::{IndexBuilder, SqliteIndex};
use den::infra::{generate_filename, write_note};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Isolated test environment with a temporary notes directory.
///
/// Creates a temp directory that is automatically cleaned up on drop.
/// Provides methods for adding test notes and building the index.
pub struct TestEnv {
    /// The temporary directory (kept for lifetime management)
    _temp_dir: TempDir,
    /// Path to the notes directory
    notes_dir: PathBuf,
}

impl TestEnv {
    /// Creates a new isolated test environment.
    ///
    /// The environment includes an empty notes directory that will
    /// be automatically cleaned up when the TestEnv is dropped.
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let notes_dir = temp_dir.path().to_path_buf();
        Self {
            _temp_dir: temp_dir,
            notes_dir,
        }
    }

    /// Returns the path to the notes directory.
    pub fn notes_dir(&self) -> &Path {
        &self.notes_dir
    }

    /// Returns the path where the SQLite index would be stored.
    pub fn index_path(&self) -> PathBuf {
        self.notes_dir.join(".index").join("notes.db")
    }

    /// Adds a test note to the environment.
    ///
    /// Creates the note file in the notes directory and returns the path.
    pub fn add_note(&self, test_note: &TestNote) -> PathBuf {
        let note = test_note.to_note();
        let filename = generate_filename(note.id(), note.title());
        let path = self.notes_dir.join(&filename);
        write_note(&path, &note, test_note.get_body()).expect("Failed to write test note");
        path
    }

    /// Builds the SQLite index from all notes in the directory.
    pub fn build_index(&self) -> Result<SqliteIndex> {
        let mut index = SqliteIndex::open(&self.index_path())?;
        let builder = IndexBuilder::new(self.notes_dir.clone());
        builder.full_rebuild(&mut index)?;
        Ok(index)
    }

    /// Creates a DenCommand configured for this test environment.
    pub fn cmd(&self) -> DenCommand {
        DenCommand::new().dir(&self.notes_dir)
    }

    /// Writes a file to the test environment and returns its path.
    ///
    /// Useful for creating custom templates, CSS files, etc.
    pub fn write_file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.notes_dir.join(name);
        std::fs::write(&path, content).expect("Failed to write file");
        path
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use den::index::IndexRepository;
    use den::infra::read_note;

    // ===========================================
    // Phase 1: TestEnv Foundation
    // ===========================================

    #[test]
    fn test_env_creates_temp_directory() {
        let env = TestEnv::new();
        assert!(env.notes_dir().exists(), "notes directory should exist");
        assert!(
            env.notes_dir().is_dir(),
            "notes directory should be a directory"
        );
    }

    #[test]
    fn test_env_cleanup_on_drop() {
        let path = {
            let env = TestEnv::new();
            env.notes_dir().to_path_buf()
        };
        // After env is dropped, the temp directory should be cleaned up
        assert!(
            !path.exists(),
            "temp directory should be cleaned up on drop"
        );
    }

    #[test]
    fn test_env_index_path() {
        let env = TestEnv::new();
        let index_path = env.index_path();
        assert!(index_path.ends_with(".index/notes.db"));
        assert!(index_path.starts_with(env.notes_dir()));
    }

    #[test]
    fn test_env_provides_command() {
        let env = TestEnv::new();
        let cmd = env.cmd();
        // The command should have --dir set to the notes directory
        let args = cmd.get_args();
        assert_eq!(args[0], "--dir");
        assert_eq!(args[1], env.notes_dir().to_string_lossy());
    }

    // ===========================================
    // Phase 3: TestEnv Note Addition
    // ===========================================

    #[test]
    fn test_env_add_note_creates_file() {
        let env = TestEnv::new();
        let note = TestNote::new("Test Note");
        let path = env.add_note(&note);

        assert!(path.exists(), "note file should be created");
        assert!(path.is_file(), "note should be a file");
        assert!(path.extension().is_some_and(|ext| ext == "md"));
    }

    #[test]
    fn test_env_add_note_parseable() {
        let env = TestEnv::new();
        let note = TestNote::new("Parseable Note")
            .topic("software/testing")
            .tag("integration")
            .body("# Test Content\n\nThis is a test.");

        let path = env.add_note(&note);
        let parsed = read_note(&path).expect("Should parse the note");

        assert_eq!(parsed.note.title(), "Parseable Note");
        assert_eq!(parsed.note.topics()[0].to_string(), "software/testing");
        assert_eq!(parsed.note.tags()[0].as_str(), "integration");
        assert!(parsed.body.contains("# Test Content"));
    }

    #[test]
    fn test_env_add_multiple_notes() {
        let env = TestEnv::new();

        let note1 = TestNote::new("First Note");
        let note2 = TestNote::new("Second Note");
        let note3 = TestNote::new("Third Note");

        let path1 = env.add_note(&note1);
        let path2 = env.add_note(&note2);
        let path3 = env.add_note(&note3);

        assert!(path1.exists());
        assert!(path2.exists());
        assert!(path3.exists());
        assert_ne!(path1, path2);
        assert_ne!(path2, path3);
    }

    // ===========================================
    // Phase 4: Index Building
    // ===========================================

    #[test]
    fn test_env_build_index_creates_db() {
        let env = TestEnv::new();
        let note = TestNote::new("Indexed Note");
        env.add_note(&note);

        let _index = env.build_index().expect("Should build index");

        assert!(env.index_path().exists(), "index file should be created");
    }

    #[test]
    fn test_env_build_index_includes_notes() {
        let env = TestEnv::new();

        let note1 = TestNote::new("First Indexed").id("01HQ3K5M7NXJK4QZPW8V2R6T9Y");
        let note2 = TestNote::new("Second Indexed").id("01HQ3K5M7NXJK4QZPW8V2R6T9Z");

        env.add_note(&note1);
        env.add_note(&note2);

        let index = env.build_index().expect("Should build index");

        // Query the index to verify notes are included
        let all_notes = index.list_all().expect("Should list notes");
        assert_eq!(all_notes.len(), 2, "Index should contain both notes");

        let titles: Vec<_> = all_notes.iter().map(|n| n.title()).collect();
        assert!(titles.contains(&"First Indexed"));
        assert!(titles.contains(&"Second Indexed"));
    }
}
