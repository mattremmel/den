//! Case-insensitive tag type for categorizing notes.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// A case-insensitive tag for categorizing notes.
///
/// Tags are flat (non-hierarchical) labels for filtering notes.
/// They are normalized to lowercase internally, making `Draft`, `draft`, and `DRAFT` equivalent.
///
/// # Validation Rules
/// - Non-empty after normalization
/// - Must contain only alphanumeric characters, hyphens, and underscores
///
/// # Normalization
/// - Surrounding whitespace is trimmed
/// - Converted to lowercase
///
/// # Examples
///
/// ```
/// use den::domain::Tag;
///
/// let tag = Tag::new("Draft").unwrap();
/// assert_eq!(tag.as_str(), "draft");
///
/// // Case-insensitive equality
/// let tag2 = Tag::new("DRAFT").unwrap();
/// assert_eq!(tag, tag2);
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Tag(String); // Always stored lowercase

/// Error returned when parsing an invalid tag.
#[derive(Debug, Clone)]
pub struct ParseTagError(String);

impl fmt::Display for ParseTagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ParseTagError {}

impl Tag {
    /// Creates a new Tag from a string.
    ///
    /// The input is normalized (trimmed, converted to lowercase) and validated.
    ///
    /// # Errors
    ///
    /// Returns `ParseTagError` if:
    /// - The tag is empty or whitespace-only
    /// - The tag contains invalid characters (only alphanumeric, hyphens, underscores allowed)
    pub fn new(s: &str) -> Result<Self, ParseTagError> {
        // Trim whitespace and convert to lowercase
        let normalized = s.trim().to_lowercase();

        // Check for empty
        if normalized.is_empty() {
            return Err(ParseTagError("tag cannot be empty".to_string()));
        }

        // Validate characters
        if !normalized
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ParseTagError(format!(
                "invalid tag '{}': tags must contain only alphanumeric characters, hyphens, and underscores",
                normalized
            )));
        }

        Ok(Self(normalized))
    }

    /// Returns the normalized tag value as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tag(\"{}\")", self.0)
    }
}

impl FromStr for Tag {
    type Err = ParseTagError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl Serialize for Tag {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Tag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;

    // ===========================================
    // Phase 1: Basic Structure & Validation
    // ===========================================

    #[test]
    fn new_with_valid_tag() {
        let tag = Tag::new("draft").unwrap();
        assert_eq!(tag.to_string(), "draft");
    }

    #[test]
    fn new_rejects_empty_string() {
        assert!(Tag::new("").is_err());
    }

    #[test]
    fn new_rejects_whitespace_only() {
        assert!(Tag::new("   ").is_err());
    }

    // ===========================================
    // Phase 2: Normalization (Case-Insensitivity)
    // ===========================================

    #[test]
    fn normalizes_to_lowercase() {
        let tag = Tag::new("Draft").unwrap();
        assert_eq!(tag.to_string(), "draft");
    }

    #[test]
    fn normalizes_mixed_case() {
        let tag = Tag::new("NeedsReview").unwrap();
        assert_eq!(tag.to_string(), "needsreview");
    }

    #[test]
    fn trims_whitespace() {
        let tag = Tag::new("  draft  ").unwrap();
        assert_eq!(tag.to_string(), "draft");
    }

    // ===========================================
    // Phase 3: Character Validation
    // ===========================================

    #[test]
    fn allows_alphanumeric() {
        assert!(Tag::new("tag123").is_ok());
    }

    #[test]
    fn allows_hyphens() {
        assert!(Tag::new("needs-review").is_ok());
    }

    #[test]
    fn allows_underscores() {
        assert!(Tag::new("work_in_progress").is_ok());
    }

    #[test]
    fn rejects_spaces() {
        assert!(Tag::new("needs review").is_err());
    }

    #[test]
    fn rejects_special_chars() {
        assert!(Tag::new("tag@home").is_err());
        assert!(Tag::new("tag#1").is_err());
    }

    #[test]
    fn rejects_slashes() {
        assert!(Tag::new("path/tag").is_err()); // Tags aren't hierarchical
    }

    // ===========================================
    // Phase 4: Equality & Hashing (Case-Insensitive)
    // ===========================================

    #[test]
    fn equality_case_insensitive() {
        let t1 = Tag::new("Draft").unwrap();
        let t2 = Tag::new("draft").unwrap();
        let t3 = Tag::new("DRAFT").unwrap();
        assert_eq!(t1, t2);
        assert_eq!(t2, t3);
    }

    #[test]
    fn hash_consistent_with_equality() {
        let t1 = Tag::new("Draft").unwrap();
        let t2 = Tag::new("draft").unwrap();
        let mut set = HashSet::new();
        set.insert(t1);
        assert!(set.contains(&t2));
    }

    #[test]
    fn hashset_deduplicates_case_variants() {
        let mut set = HashSet::new();
        set.insert(Tag::new("draft").unwrap());
        set.insert(Tag::new("Draft").unwrap());
        set.insert(Tag::new("DRAFT").unwrap());
        assert_eq!(set.len(), 1);
    }

    // ===========================================
    // Phase 5: Display & Debug
    // ===========================================

    #[test]
    fn display_shows_normalized_value() {
        let tag = Tag::new("NeedsReview").unwrap();
        assert_eq!(format!("{}", tag), "needsreview");
    }

    #[test]
    fn debug_format() {
        let tag = Tag::new("draft").unwrap();
        assert_eq!(format!("{:?}", tag), "Tag(\"draft\")");
    }

    // ===========================================
    // Phase 6: FromStr
    // ===========================================

    #[test]
    fn parse_via_fromstr() {
        let tag: Tag = "draft".parse().unwrap();
        assert_eq!(tag.to_string(), "draft");
    }

    #[test]
    fn parse_normalizes() {
        let tag: Tag = "DRAFT".parse().unwrap();
        assert_eq!(tag.to_string(), "draft");
    }

    #[test]
    fn parse_error_display() {
        let err = "".parse::<Tag>().unwrap_err();
        assert!(err.to_string().contains("empty") || err.to_string().contains("invalid"));
    }

    // ===========================================
    // Phase 7: Serde Support
    // ===========================================

    #[test]
    fn serde_roundtrip() {
        let tag = Tag::new("draft").unwrap();
        let yaml = serde_yaml::to_string(&tag).unwrap();
        let parsed: Tag = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(tag, parsed);
    }

    #[test]
    fn serde_normalizes_on_deserialize() {
        let yaml = "'DRAFT'\n";
        let tag: Tag = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(tag.to_string(), "draft");
    }

    #[test]
    fn serde_in_vec_context() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Note {
            tags: Vec<Tag>,
        }
        let note = Note {
            tags: vec![
                Tag::new("draft").unwrap(),
                Tag::new("needs-review").unwrap(),
            ],
        };
        let yaml = serde_yaml::to_string(&note).unwrap();
        let parsed: Note = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(note, parsed);
    }

    #[test]
    fn serde_rejects_invalid_on_deserialize() {
        let yaml = "''\n";
        let result: Result<Tag, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    // ===========================================
    // Phase 8: Accessor Method
    // ===========================================

    #[test]
    fn as_str_returns_normalized_value() {
        let tag = Tag::new("DRAFT").unwrap();
        assert_eq!(tag.as_str(), "draft");
    }
}
