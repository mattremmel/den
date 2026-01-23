//! Test fixture utilities for integration tests.

pub mod harness;

use std::path::{Path, PathBuf};

/// Returns the path to the fixtures directory.
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Returns the path to a valid fixture file by name.
pub fn valid_fixture(name: &str) -> PathBuf {
    fixtures_dir().join("valid").join(name)
}

/// Returns the path to an invalid fixture file by name.
pub fn invalid_fixture(name: &str) -> PathBuf {
    fixtures_dir().join("invalid").join(name)
}

/// Reads a fixture file and returns its contents as a string.
///
/// # Panics
///
/// Panics if the file cannot be read.
pub fn read_fixture(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path.display(), e))
}
