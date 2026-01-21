//! Hierarchical topic path type for organizing notes.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// A hierarchical topic path for organizing notes.
///
/// Topics are forward-slash separated paths like `software/architecture/patterns`.
/// They support hierarchy queries (parent, ancestors, descendants).
///
/// # Validation Rules
/// - Non-empty after normalization
/// - Segments can contain alphanumeric characters, hyphens, and underscores
/// - Case-sensitive: `Software` ≠ `software`
///
/// # Normalization
/// - Leading/trailing slashes are stripped
/// - Consecutive slashes are collapsed
/// - Surrounding whitespace is trimmed
///
/// # Examples
///
/// ```
/// use den::domain::Topic;
///
/// let topic = Topic::new("software/architecture/patterns").unwrap();
/// assert_eq!(topic.segments(), &["software", "architecture", "patterns"]);
/// assert_eq!(topic.depth(), 3);
///
/// let parent = topic.parent().unwrap();
/// assert_eq!(parent.to_string(), "software/architecture");
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Topic {
    path: String,
    segments: Vec<String>,
}

/// Error returned when parsing an invalid topic path.
#[derive(Debug, Clone)]
pub struct ParseTopicError(String);

impl fmt::Display for ParseTopicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ParseTopicError {}

impl Topic {
    /// Creates a new Topic from a path string.
    ///
    /// The path is normalized (trimmed, slashes collapsed) and validated.
    ///
    /// # Errors
    ///
    /// Returns `ParseTopicError` if:
    /// - The path is empty or whitespace-only
    /// - The path normalizes to empty (e.g., "///")
    /// - Any segment contains invalid characters
    pub fn new(path: &str) -> Result<Self, ParseTopicError> {
        // Trim surrounding whitespace
        let trimmed = path.trim();

        // Split by slash
        let raw_segments: Vec<&str> = trimmed.split('/').collect();

        let mut segments: Vec<String> = Vec::new();

        for raw_seg in raw_segments {
            // Empty segment from consecutive slashes, leading slash, or trailing slash
            if raw_seg.is_empty() {
                continue;
            }

            // Trim the segment
            let seg = raw_seg.trim();

            // Whitespace-only segment (non-empty raw but empty after trim)
            if seg.is_empty() {
                return Err(ParseTopicError(
                    "invalid segment: segments cannot be whitespace-only".to_string(),
                ));
            }

            // Validate the segment characters
            if !Self::is_valid_segment(seg) {
                return Err(ParseTopicError(format!(
                    "invalid segment '{}': segments must contain only alphanumeric characters, hyphens, and underscores",
                    seg
                )));
            }

            segments.push(seg.to_string());
        }

        // Check for empty result
        if segments.is_empty() {
            return Err(ParseTopicError("topic path cannot be empty".to_string()));
        }

        let path = segments.join("/");
        Ok(Self { path, segments })
    }

    /// Returns whether a segment contains only valid characters.
    fn is_valid_segment(segment: &str) -> bool {
        !segment.is_empty()
            && segment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }

    /// Returns the path components as a slice.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Returns the number of segments in the path.
    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// Returns the parent topic, or `None` if this is a root-level topic.
    pub fn parent(&self) -> Option<Topic> {
        if self.segments.len() <= 1 {
            return None;
        }

        let parent_segments = &self.segments[..self.segments.len() - 1];
        let path = parent_segments.join("/");
        Some(Topic {
            path,
            segments: parent_segments.to_vec(),
        })
    }

    /// Returns all ancestor topics, from root to immediate parent.
    ///
    /// Does not include self.
    pub fn ancestors(&self) -> Vec<Topic> {
        let mut result = Vec::new();
        for i in 1..self.segments.len() {
            let ancestor_segments = &self.segments[..i];
            let path = ancestor_segments.join("/");
            result.push(Topic {
                path,
                segments: ancestor_segments.to_vec(),
            });
        }
        result
    }

    /// Returns whether this topic is an ancestor of another topic.
    ///
    /// A topic is an ancestor if it is a proper prefix at segment boundaries.
    /// `software` is an ancestor of `software/api` but NOT of `software-dev`.
    pub fn is_ancestor_of(&self, other: &Topic) -> bool {
        // Can't be ancestor of self or shorter/equal paths
        if self.segments.len() >= other.segments.len() {
            return false;
        }

        // Check segment-by-segment prefix match
        self.segments
            .iter()
            .zip(other.segments.iter())
            .all(|(a, b)| a == b)
    }
}

impl fmt::Display for Topic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path)
    }
}

impl fmt::Debug for Topic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Topic(\"{}\")", self.path)
    }
}

impl FromStr for Topic {
    type Err = ParseTopicError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl Serialize for Topic {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.path)
    }
}

impl<'de> Deserialize<'de> for Topic {
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
    fn new_with_valid_simple_path() {
        let topic = Topic::new("software").unwrap();
        assert_eq!(topic.to_string(), "software");
    }

    #[test]
    fn new_with_valid_nested_path() {
        let topic = Topic::new("software/architecture/patterns").unwrap();
        assert_eq!(topic.to_string(), "software/architecture/patterns");
    }

    #[test]
    fn new_rejects_empty_string() {
        assert!(Topic::new("").is_err());
    }

    #[test]
    fn new_rejects_whitespace_only() {
        assert!(Topic::new("   ").is_err());
    }

    // ===========================================
    // Phase 2: Normalization
    // ===========================================

    #[test]
    fn normalizes_leading_slash() {
        let topic = Topic::new("/software/api").unwrap();
        assert_eq!(topic.to_string(), "software/api");
    }

    #[test]
    fn normalizes_trailing_slash() {
        let topic = Topic::new("software/api/").unwrap();
        assert_eq!(topic.to_string(), "software/api");
    }

    #[test]
    fn normalizes_multiple_slashes() {
        let topic = Topic::new("software//api///patterns").unwrap();
        assert_eq!(topic.to_string(), "software/api/patterns");
    }

    #[test]
    fn normalizes_surrounding_whitespace() {
        let topic = Topic::new("  software/api  ").unwrap();
        assert_eq!(topic.to_string(), "software/api");
    }

    #[test]
    fn rejects_path_that_normalizes_to_empty() {
        assert!(Topic::new("///").is_err());
    }

    // ===========================================
    // Phase 3: Segment Validation
    // ===========================================

    #[test]
    fn rejects_empty_segment_in_middle() {
        // After normalization would have empty segment
        assert!(Topic::new("software/ /api").is_err());
    }

    #[test]
    fn allows_alphanumeric_segments() {
        let topic = Topic::new("software123/api456").unwrap();
        assert_eq!(topic.to_string(), "software123/api456");
    }

    #[test]
    fn allows_hyphens_and_underscores() {
        let topic = Topic::new("my-topic/sub_topic").unwrap();
        assert_eq!(topic.to_string(), "my-topic/sub_topic");
    }

    #[test]
    fn rejects_special_characters() {
        assert!(Topic::new("software@api").is_err());
        assert!(Topic::new("soft ware").is_err());
    }

    // ===========================================
    // Phase 4: Segments & Hierarchy Methods
    // ===========================================

    #[test]
    fn segments_returns_path_components() {
        let topic = Topic::new("software/architecture/patterns").unwrap();
        assert_eq!(topic.segments(), &["software", "architecture", "patterns"]);
    }

    #[test]
    fn segments_single_component() {
        let topic = Topic::new("reference").unwrap();
        assert_eq!(topic.segments(), &["reference"]);
    }

    #[test]
    fn depth_returns_segment_count() {
        assert_eq!(Topic::new("a").unwrap().depth(), 1);
        assert_eq!(Topic::new("a/b/c").unwrap().depth(), 3);
    }

    #[test]
    fn parent_returns_none_for_root() {
        let topic = Topic::new("software").unwrap();
        assert!(topic.parent().is_none());
    }

    #[test]
    fn parent_returns_parent_path() {
        let topic = Topic::new("software/architecture/patterns").unwrap();
        let parent = topic.parent().unwrap();
        assert_eq!(parent.to_string(), "software/architecture");
    }

    #[test]
    fn ancestors_returns_all_ancestors() {
        let topic = Topic::new("software/architecture/patterns").unwrap();
        let ancestors: Vec<String> = topic.ancestors().iter().map(|t| t.to_string()).collect();
        assert_eq!(ancestors, vec!["software", "software/architecture"]);
    }

    #[test]
    fn ancestors_empty_for_root() {
        let topic = Topic::new("software").unwrap();
        assert!(topic.ancestors().is_empty());
    }

    // ===========================================
    // Phase 5: Ancestor/Descendant Relationships
    // ===========================================

    #[test]
    fn is_ancestor_of_direct_child() {
        let parent = Topic::new("software").unwrap();
        let child = Topic::new("software/api").unwrap();
        assert!(parent.is_ancestor_of(&child));
        assert!(!child.is_ancestor_of(&parent));
    }

    #[test]
    fn is_ancestor_of_deep_descendant() {
        let ancestor = Topic::new("software").unwrap();
        let descendant = Topic::new("software/architecture/patterns").unwrap();
        assert!(ancestor.is_ancestor_of(&descendant));
    }

    #[test]
    fn is_ancestor_of_self_is_false() {
        let topic = Topic::new("software/api").unwrap();
        assert!(!topic.is_ancestor_of(&topic));
    }

    #[test]
    fn is_ancestor_of_unrelated_is_false() {
        let topic1 = Topic::new("software/api").unwrap();
        let topic2 = Topic::new("reference/books").unwrap();
        assert!(!topic1.is_ancestor_of(&topic2));
    }

    #[test]
    fn is_ancestor_requires_segment_boundary() {
        let soft = Topic::new("soft").unwrap();
        let software = Topic::new("software").unwrap();
        assert!(!soft.is_ancestor_of(&software)); // segment boundary, not string prefix
    }

    // ===========================================
    // Phase 6: Equality & Hashing
    // ===========================================

    #[test]
    fn equality_after_normalization() {
        let t1 = Topic::new("software/api").unwrap();
        let t2 = Topic::new("/software/api/").unwrap();
        assert_eq!(t1, t2);
    }

    #[test]
    fn case_sensitive_inequality() {
        let t1 = Topic::new("Software").unwrap();
        let t2 = Topic::new("software").unwrap();
        assert_ne!(t1, t2);
    }

    #[test]
    fn hash_consistent_with_equality() {
        let t1 = Topic::new("software/api").unwrap();
        let t2 = Topic::new("/software/api/").unwrap();
        let mut set = HashSet::new();
        set.insert(t1);
        assert!(set.contains(&t2));
    }

    // ===========================================
    // Phase 7: FromStr & Error Handling
    // ===========================================

    #[test]
    fn parse_via_fromstr() {
        let topic: Topic = "software/api".parse().unwrap();
        assert_eq!(topic.to_string(), "software/api");
    }

    #[test]
    fn parse_error_display() {
        let err: ParseTopicError = "".parse::<Topic>().unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn parse_error_for_invalid_chars() {
        let err = "soft@ware".parse::<Topic>().unwrap_err();
        assert!(err.to_string().contains("invalid"));
    }

    // ===========================================
    // Phase 8: Serde Support
    // ===========================================

    #[test]
    fn serde_roundtrip() {
        let topic = Topic::new("software/architecture").unwrap();
        let yaml = serde_yaml::to_string(&topic).unwrap();
        let parsed: Topic = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(topic, parsed);
    }

    #[test]
    fn serde_normalizes_on_deserialize() {
        let yaml = "'/software/api/'\n";
        let topic: Topic = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(topic.to_string(), "software/api");
    }

    #[test]
    fn serde_in_vec_context() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Note {
            topics: Vec<Topic>,
        }

        let note = Note {
            topics: vec![
                Topic::new("software/api").unwrap(),
                Topic::new("reference").unwrap(),
            ],
        };
        let yaml = serde_yaml::to_string(&note).unwrap();
        let parsed: Note = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(note, parsed);
    }

    #[test]
    fn serde_rejects_invalid_on_deserialize() {
        let yaml = "''\n"; // empty string
        let result: Result<Topic, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    // ===========================================
    // Phase 9: Debug & Display
    // ===========================================

    #[test]
    fn display_format() {
        let topic = Topic::new("software/api").unwrap();
        assert_eq!(format!("{}", topic), "software/api");
    }

    #[test]
    fn debug_format() {
        let topic = Topic::new("software/api").unwrap();
        assert_eq!(format!("{:?}", topic), "Topic(\"software/api\")");
    }
}
