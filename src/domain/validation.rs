//! Validation error types for the check command.
//!
//! These types represent issues found during validation of the notes collection,
//! such as parse errors, duplicate IDs, broken links, and orphaned notes.

use std::path::PathBuf;

use crate::domain::NoteId;
use crate::infra::ParseError;

/// A validation issue found during checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    /// The file where the issue was found.
    pub path: PathBuf,
    /// The kind of validation issue.
    pub kind: ValidationKind,
}

impl ValidationIssue {
    /// Creates a new validation issue.
    pub fn new(path: impl Into<PathBuf>, kind: ValidationKind) -> Self {
        Self {
            path: path.into(),
            kind,
        }
    }

    /// Creates a parse error issue.
    pub fn parse_error(path: impl Into<PathBuf>, error: ParseError) -> Self {
        Self::new(path, ValidationKind::ParseError(error.to_string()))
    }

    /// Creates a duplicate ID issue.
    pub fn duplicate_id(path: impl Into<PathBuf>, id: NoteId, first_path: impl Into<PathBuf>) -> Self {
        Self::new(
            path,
            ValidationKind::DuplicateId {
                id,
                first_path: first_path.into(),
            },
        )
    }

    /// Creates a broken link issue.
    pub fn broken_link(path: impl Into<PathBuf>, target_id: NoteId) -> Self {
        Self::new(path, ValidationKind::BrokenLink { target_id })
    }

    /// Creates an orphaned note issue (note has no topics).
    pub fn orphaned(path: impl Into<PathBuf>) -> Self {
        Self::new(path, ValidationKind::Orphaned)
    }

    /// Returns true if this is a parse error.
    pub fn is_parse_error(&self) -> bool {
        matches!(self.kind, ValidationKind::ParseError(_))
    }

    /// Returns true if this is a duplicate ID error.
    pub fn is_duplicate_id(&self) -> bool {
        matches!(self.kind, ValidationKind::DuplicateId { .. })
    }

    /// Returns true if this is a broken link error.
    pub fn is_broken_link(&self) -> bool {
        matches!(self.kind, ValidationKind::BrokenLink { .. })
    }

    /// Returns true if this is an orphaned note warning.
    pub fn is_orphaned(&self) -> bool {
        matches!(self.kind, ValidationKind::Orphaned)
    }

    /// Returns the severity of this issue.
    pub fn severity(&self) -> Severity {
        self.kind.severity()
    }
}

impl std::fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.kind)
    }
}

/// The kind of validation issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationKind {
    /// File could not be parsed (malformed frontmatter, invalid YAML, missing required fields).
    ParseError(String),

    /// Another file already has this ID.
    DuplicateId {
        /// The duplicate ID.
        id: NoteId,
        /// Path to the first file with this ID.
        first_path: PathBuf,
    },

    /// A link references a note ID that doesn't exist.
    BrokenLink {
        /// The ID that was referenced but not found.
        target_id: NoteId,
    },

    /// Note has no topics (orphaned in the virtual folder hierarchy).
    Orphaned,
}

impl ValidationKind {
    /// Returns the severity of this kind of issue.
    pub fn severity(&self) -> Severity {
        match self {
            ValidationKind::ParseError(_) => Severity::Error,
            ValidationKind::DuplicateId { .. } => Severity::Error,
            ValidationKind::BrokenLink { .. } => Severity::Error,
            ValidationKind::Orphaned => Severity::Warning,
        }
    }
}

impl std::fmt::Display for ValidationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationKind::ParseError(msg) => write!(f, "parse error: {}", msg),
            ValidationKind::DuplicateId { id, first_path } => {
                write!(
                    f,
                    "duplicate ID '{}' (first seen in {})",
                    id.prefix(),
                    first_path.display()
                )
            }
            ValidationKind::BrokenLink { target_id } => {
                write!(f, "broken link to '{}'", target_id.prefix())
            }
            ValidationKind::Orphaned => write!(f, "orphaned note (no topics)"),
        }
    }
}

/// Severity level of a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Informational message.
    Warning,
    /// Problem that should be fixed.
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
        }
    }
}

/// Summary of validation results.
#[derive(Debug, Clone, Default)]
pub struct ValidationSummary {
    /// All issues found during validation.
    pub issues: Vec<ValidationIssue>,
}

impl ValidationSummary {
    /// Creates a new empty summary.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an issue to the summary.
    pub fn add(&mut self, issue: ValidationIssue) {
        self.issues.push(issue);
    }

    /// Returns the total number of issues.
    pub fn total(&self) -> usize {
        self.issues.len()
    }

    /// Returns the number of errors.
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity() == Severity::Error)
            .count()
    }

    /// Returns the number of warnings.
    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity() == Severity::Warning)
            .count()
    }

    /// Returns true if there are no issues.
    pub fn is_ok(&self) -> bool {
        self.issues.is_empty()
    }

    /// Returns true if there are any errors.
    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }

    /// Returns issues grouped by severity, errors first.
    pub fn issues_by_severity(&self) -> impl Iterator<Item = &ValidationIssue> {
        let mut sorted: Vec<_> = self.issues.iter().collect();
        sorted.sort_by_key(|i| std::cmp::Reverse(i.severity()));
        sorted.into_iter()
    }

    /// Returns all parse errors.
    pub fn parse_errors(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues.iter().filter(|i| i.is_parse_error())
    }

    /// Returns all duplicate ID issues.
    pub fn duplicate_ids(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues.iter().filter(|i| i.is_duplicate_id())
    }

    /// Returns all broken link issues.
    pub fn broken_links(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues.iter().filter(|i| i.is_broken_link())
    }

    /// Returns all orphaned note warnings.
    pub fn orphaned_notes(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues.iter().filter(|i| i.is_orphaned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_note_id() -> NoteId {
        "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap()
    }

    fn other_note_id() -> NoteId {
        "01HQ4A2R9PXJK4QZPW8V2R6T9Y".parse().unwrap()
    }

    // ===========================================
    // ValidationIssue construction
    // ===========================================

    #[test]
    fn creates_parse_error_issue() {
        let issue = ValidationIssue::new(
            "notes/test.md",
            ValidationKind::ParseError("missing opening delimiter".to_string()),
        );

        assert_eq!(issue.path, PathBuf::from("notes/test.md"));
        assert!(issue.is_parse_error());
        assert_eq!(issue.severity(), Severity::Error);
    }

    #[test]
    fn creates_duplicate_id_issue() {
        let issue = ValidationIssue::duplicate_id(
            "notes/second.md",
            test_note_id(),
            "notes/first.md",
        );

        assert_eq!(issue.path, PathBuf::from("notes/second.md"));
        assert!(issue.is_duplicate_id());
        assert_eq!(issue.severity(), Severity::Error);

        if let ValidationKind::DuplicateId { id, first_path } = &issue.kind {
            assert_eq!(id, &test_note_id());
            assert_eq!(first_path, &PathBuf::from("notes/first.md"));
        } else {
            panic!("Expected DuplicateId variant");
        }
    }

    #[test]
    fn creates_broken_link_issue() {
        let issue = ValidationIssue::broken_link("notes/test.md", other_note_id());

        assert_eq!(issue.path, PathBuf::from("notes/test.md"));
        assert!(issue.is_broken_link());
        assert_eq!(issue.severity(), Severity::Error);

        if let ValidationKind::BrokenLink { target_id } = &issue.kind {
            assert_eq!(target_id, &other_note_id());
        } else {
            panic!("Expected BrokenLink variant");
        }
    }

    #[test]
    fn creates_orphaned_issue() {
        let issue = ValidationIssue::orphaned("notes/lonely.md");

        assert_eq!(issue.path, PathBuf::from("notes/lonely.md"));
        assert!(issue.is_orphaned());
        assert_eq!(issue.severity(), Severity::Warning);
    }

    // ===========================================
    // Display formatting
    // ===========================================

    #[test]
    fn formats_parse_error() {
        let issue = ValidationIssue::new(
            "notes/bad.md",
            ValidationKind::ParseError("invalid YAML".to_string()),
        );

        let display = issue.to_string();
        assert!(display.contains("notes/bad.md"));
        assert!(display.contains("parse error"));
        assert!(display.contains("invalid YAML"));
    }

    #[test]
    fn formats_duplicate_id() {
        let issue = ValidationIssue::duplicate_id(
            "notes/second.md",
            test_note_id(),
            "notes/first.md",
        );

        let display = issue.to_string();
        assert!(display.contains("notes/second.md"));
        assert!(display.contains("duplicate ID"));
        assert!(display.contains("01HQ3K5M7N")); // prefix (10-char)
        assert!(display.contains("notes/first.md"));
    }

    #[test]
    fn formats_broken_link() {
        let issue = ValidationIssue::broken_link("notes/test.md", other_note_id());

        let display = issue.to_string();
        assert!(display.contains("notes/test.md"));
        assert!(display.contains("broken link"));
        assert!(display.contains("01HQ4A2R9P")); // prefix (10-char)
    }

    #[test]
    fn formats_orphaned() {
        let issue = ValidationIssue::orphaned("notes/lonely.md");

        let display = issue.to_string();
        assert!(display.contains("notes/lonely.md"));
        assert!(display.contains("orphaned"));
        assert!(display.contains("no topics"));
    }

    // ===========================================
    // Severity
    // ===========================================

    #[test]
    fn severity_ordering() {
        assert!(Severity::Warning < Severity::Error);
    }

    #[test]
    fn severity_display() {
        assert_eq!(Severity::Warning.to_string(), "warning");
        assert_eq!(Severity::Error.to_string(), "error");
    }

    // ===========================================
    // ValidationSummary
    // ===========================================

    #[test]
    fn empty_summary_is_ok() {
        let summary = ValidationSummary::new();

        assert!(summary.is_ok());
        assert!(!summary.has_errors());
        assert_eq!(summary.total(), 0);
        assert_eq!(summary.error_count(), 0);
        assert_eq!(summary.warning_count(), 0);
    }

    #[test]
    fn summary_counts_errors_and_warnings() {
        let mut summary = ValidationSummary::new();
        summary.add(ValidationIssue::orphaned("notes/a.md"));
        summary.add(ValidationIssue::orphaned("notes/b.md"));
        summary.add(ValidationIssue::broken_link("notes/c.md", test_note_id()));

        assert_eq!(summary.total(), 3);
        assert_eq!(summary.error_count(), 1);
        assert_eq!(summary.warning_count(), 2);
        assert!(!summary.is_ok());
        assert!(summary.has_errors());
    }

    #[test]
    fn summary_filters_by_type() {
        let mut summary = ValidationSummary::new();
        summary.add(ValidationIssue::new(
            "a.md",
            ValidationKind::ParseError("bad".to_string()),
        ));
        summary.add(ValidationIssue::duplicate_id("b.md", test_note_id(), "c.md"));
        summary.add(ValidationIssue::broken_link("d.md", other_note_id()));
        summary.add(ValidationIssue::orphaned("e.md"));

        assert_eq!(summary.parse_errors().count(), 1);
        assert_eq!(summary.duplicate_ids().count(), 1);
        assert_eq!(summary.broken_links().count(), 1);
        assert_eq!(summary.orphaned_notes().count(), 1);
    }

    #[test]
    fn summary_sorts_by_severity() {
        let mut summary = ValidationSummary::new();
        summary.add(ValidationIssue::orphaned("warning.md"));
        summary.add(ValidationIssue::broken_link("error.md", test_note_id()));

        let sorted: Vec<_> = summary.issues_by_severity().collect();
        assert_eq!(sorted[0].severity(), Severity::Error);
        assert_eq!(sorted[1].severity(), Severity::Warning);
    }
}
