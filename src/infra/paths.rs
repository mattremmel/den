//! Path utilities for shell expansion.

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Expand tilde in a path to the user's home directory.
///
/// - `~` expands to the current user's home directory
/// - `~user` expands to that user's home directory
/// - Paths without tilde are returned unchanged
///
/// # Examples
///
/// ```
/// use den::infra::expand_tilde;
/// use std::path::Path;
///
/// let expanded = expand_tilde(Path::new("~/notes")).unwrap();
/// assert!(expanded.is_absolute());
/// ```
pub fn expand_tilde(path: &Path) -> Result<PathBuf> {
    let path_str = path.to_string_lossy();

    // Only expand if path starts with ~
    if !path_str.starts_with('~') {
        return Ok(path.to_path_buf());
    }

    let expanded = shellexpand::tilde(&path_str);
    Ok(PathBuf::from(expanded.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_path_unchanged() {
        let path = Path::new("/usr/local/bin");
        let result = expand_tilde(path).unwrap();
        assert_eq!(result, PathBuf::from("/usr/local/bin"));
    }

    #[test]
    fn relative_path_unchanged() {
        let path = Path::new("relative/path");
        let result = expand_tilde(path).unwrap();
        assert_eq!(result, PathBuf::from("relative/path"));
    }

    #[test]
    fn tilde_expands_to_home() {
        let path = Path::new("~/documents");
        let result = expand_tilde(path).unwrap();

        // Should be absolute (expanded)
        assert!(result.is_absolute());
        // Should end with "documents"
        assert!(result.ends_with("documents"));
        // Should not contain tilde
        assert!(!result.to_string_lossy().contains('~'));
    }

    #[test]
    fn tilde_only_expands_to_home() {
        let path = Path::new("~");
        let result = expand_tilde(path).unwrap();

        // Should be absolute (expanded)
        assert!(result.is_absolute());
        // Should not contain tilde
        assert!(!result.to_string_lossy().contains('~'));
    }

    #[test]
    fn tilde_in_middle_unchanged() {
        // Tilde in the middle of a path should not be expanded
        let path = Path::new("/path/with~tilde/in/middle");
        let result = expand_tilde(path).unwrap();
        assert_eq!(result, PathBuf::from("/path/with~tilde/in/middle"));
    }

    #[test]
    fn dot_path_unchanged() {
        let path = Path::new(".");
        let result = expand_tilde(path).unwrap();
        assert_eq!(result, PathBuf::from("."));
    }

    #[test]
    fn empty_path_unchanged() {
        let path = Path::new("");
        let result = expand_tilde(path).unwrap();
        assert_eq!(result, PathBuf::from(""));
    }
}
