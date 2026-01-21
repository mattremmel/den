//! Link type representing references between notes with relationship context.

use crate::domain::NoteId;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// A relationship type for links between notes.
///
/// Relationship types are normalized to lowercase and validated.
/// They use lowercase-hyphenated format (e.g., `parent`, `see-also`, `manager-of`).
///
/// # Validation Rules
/// - Non-empty after normalization
/// - Must contain only alphanumeric characters and hyphens
/// - No underscores (distinguishes from Tags)
///
/// # Normalization
/// - Surrounding whitespace is trimmed
/// - Converted to lowercase
///
/// # Examples
///
/// ```
/// use den::domain::Rel;
///
/// let rel = Rel::new("Parent").unwrap();
/// assert_eq!(rel.as_str(), "parent");
///
/// // Case-insensitive equality
/// let rel2 = Rel::new("PARENT").unwrap();
/// assert_eq!(rel, rel2);
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Rel(String); // Always stored lowercase

/// Error returned when parsing an invalid relationship type.
#[derive(Debug, Clone)]
pub struct ParseRelError(String);

impl fmt::Display for ParseRelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ParseRelError {}

impl Rel {
    /// Creates a new Rel from a string.
    ///
    /// The input is normalized (trimmed, converted to lowercase) and validated.
    ///
    /// # Errors
    ///
    /// Returns `ParseRelError` if:
    /// - The rel is empty or whitespace-only
    /// - The rel contains invalid characters (only alphanumeric and hyphens allowed)
    pub fn new(s: &str) -> Result<Self, ParseRelError> {
        let normalized = s.trim().to_lowercase();

        if normalized.is_empty() {
            return Err(ParseRelError(
                "relationship type cannot be empty".to_string(),
            ));
        }

        if !normalized
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(ParseRelError(format!(
                "invalid relationship type '{}': must contain only alphanumeric characters and hyphens",
                normalized
            )));
        }

        Ok(Self(normalized))
    }

    /// Returns the normalized relationship type as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Rel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for Rel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rel(\"{}\")", self.0)
    }
}

impl FromStr for Rel {
    type Err = ParseRelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl Serialize for Rel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Rel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// A link from one note to another with relationship context.
///
/// Links represent explicit references between notes, stored in frontmatter.
/// Each link has a target note ID, one or more relationship types, and
/// optional context text.
///
/// # Examples
///
/// ```
/// use den::domain::{Link, NoteId, Rel};
///
/// let target: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
/// let link = Link::new(target, vec!["parent"]).unwrap();
/// assert_eq!(link.rel().len(), 1);
/// ```
#[derive(Clone)]
pub struct Link {
    target: NoteId,
    rel: Vec<Rel>,
    context: Option<String>,
}

/// Error returned when constructing an invalid link.
#[derive(Debug, Clone)]
pub struct ParseLinkError(String);

impl fmt::Display for ParseLinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ParseLinkError {}

impl Link {
    /// Creates a new Link with the given target and relationship types.
    ///
    /// # Arguments
    ///
    /// * `target` - The target note's ID
    /// * `rel` - Relationship types (must be non-empty)
    ///
    /// # Errors
    ///
    /// Returns `ParseLinkError` if:
    /// - The rel array is empty
    /// - Any rel string is invalid
    pub fn new<S: AsRef<str>>(target: NoteId, rel: Vec<S>) -> Result<Self, ParseLinkError> {
        if rel.is_empty() {
            return Err(ParseLinkError(
                "link must have at least one relationship type".to_string(),
            ));
        }

        let rels: Result<Vec<Rel>, _> = rel.iter().map(|s| Rel::new(s.as_ref())).collect();
        let rels = rels.map_err(|e| ParseLinkError(e.to_string()))?;

        Ok(Self {
            target,
            rel: rels,
            context: None,
        })
    }

    /// Creates a new Link with context.
    pub fn with_context<S: AsRef<str>>(
        target: NoteId,
        rel: Vec<S>,
        context: impl Into<String>,
    ) -> Result<Self, ParseLinkError> {
        let mut link = Self::new(target, rel)?;
        link.context = Some(context.into());
        Ok(link)
    }

    /// Returns the target note's ID.
    pub fn target(&self) -> &NoteId {
        &self.target
    }

    /// Returns the relationship types.
    pub fn rel(&self) -> &[Rel] {
        &self.rel
    }

    /// Returns the optional context text.
    pub fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }
}

impl PartialEq for Link {
    fn eq(&self, other: &Self) -> bool {
        // Context is metadata, excluded from equality
        self.target == other.target && self.rel == other.rel
    }
}

impl Eq for Link {}

impl std::hash::Hash for Link {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Context is metadata, excluded from hash
        self.target.hash(state);
        self.rel.hash(state);
    }
}

impl fmt::Display for Link {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rels: Vec<&str> = self.rel.iter().map(|r| r.as_str()).collect();
        write!(f, "{} [{}]", self.target.prefix(), rels.join(", "))
    }
}

impl fmt::Debug for Link {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Link")
            .field("target", &self.target)
            .field("rel", &self.rel)
            .field("context", &self.context)
            .finish()
    }
}

impl Serialize for Link {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", &self.target)?;
        map.serialize_entry("rel", &self.rel)?;
        if let Some(ref ctx) = self.context {
            map.serialize_entry("note", ctx)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Link {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct LinkHelper {
            id: NoteId,
            rel: Vec<Rel>,
            note: Option<String>,
        }

        let helper = LinkHelper::deserialize(deserializer)?;

        if helper.rel.is_empty() {
            return Err(serde::de::Error::custom(
                "link must have at least one relationship type",
            ));
        }

        Ok(Link {
            target: helper.id,
            rel: helper.rel,
            context: helper.note,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;

    // ===========================================
    // Phase 1: Rel Basic Validation
    // ===========================================

    #[test]
    fn rel_new_with_valid_value() {
        let rel = Rel::new("parent").unwrap();
        assert_eq!(rel.to_string(), "parent");
    }

    #[test]
    fn rel_rejects_empty_string() {
        assert!(Rel::new("").is_err());
    }

    #[test]
    fn rel_rejects_whitespace_only() {
        assert!(Rel::new("   ").is_err());
    }

    // ===========================================
    // Phase 2: Rel Normalization
    // ===========================================

    #[test]
    fn rel_normalizes_to_lowercase() {
        let rel = Rel::new("Parent").unwrap();
        assert_eq!(rel.to_string(), "parent");
    }

    #[test]
    fn rel_normalizes_mixed_case() {
        let rel = Rel::new("ManagerOf").unwrap();
        assert_eq!(rel.to_string(), "managerof");
    }

    #[test]
    fn rel_trims_whitespace() {
        let rel = Rel::new("  parent  ").unwrap();
        assert_eq!(rel.to_string(), "parent");
    }

    // ===========================================
    // Phase 3: Rel Character Validation
    // ===========================================

    #[test]
    fn rel_allows_alphanumeric() {
        assert!(Rel::new("rel123").is_ok());
    }

    #[test]
    fn rel_allows_hyphens() {
        let rel = Rel::new("see-also").unwrap();
        assert_eq!(rel.to_string(), "see-also");
    }

    #[test]
    fn rel_rejects_underscores() {
        assert!(Rel::new("see_also").is_err());
    }

    #[test]
    fn rel_rejects_spaces() {
        assert!(Rel::new("see also").is_err());
    }

    #[test]
    fn rel_rejects_special_chars() {
        assert!(Rel::new("rel@1").is_err());
        assert!(Rel::new("rel#1").is_err());
    }

    #[test]
    fn rel_rejects_slashes() {
        assert!(Rel::new("parent/child").is_err());
    }

    // ===========================================
    // Phase 4: Rel Standard Traits
    // ===========================================

    #[test]
    fn rel_equality_case_insensitive() {
        let r1 = Rel::new("Parent").unwrap();
        let r2 = Rel::new("parent").unwrap();
        let r3 = Rel::new("PARENT").unwrap();
        assert_eq!(r1, r2);
        assert_eq!(r2, r3);
    }

    #[test]
    fn rel_hash_consistent_with_equality() {
        let r1 = Rel::new("Parent").unwrap();
        let r2 = Rel::new("parent").unwrap();
        let mut set = HashSet::new();
        set.insert(r1);
        assert!(set.contains(&r2));
    }

    #[test]
    fn rel_display_shows_normalized() {
        let rel = Rel::new("SeeAlso").unwrap();
        assert_eq!(format!("{}", rel), "seealso");
    }

    #[test]
    fn rel_debug_format() {
        let rel = Rel::new("parent").unwrap();
        assert_eq!(format!("{:?}", rel), "Rel(\"parent\")");
    }

    #[test]
    fn rel_as_str_accessor() {
        let rel = Rel::new("PARENT").unwrap();
        assert_eq!(rel.as_str(), "parent");
    }

    // ===========================================
    // Phase 5: Rel FromStr & Serde
    // ===========================================

    #[test]
    fn rel_parse_via_fromstr() {
        let rel: Rel = "parent".parse().unwrap();
        assert_eq!(rel.to_string(), "parent");
    }

    #[test]
    fn rel_parse_error_display() {
        let err = "".parse::<Rel>().unwrap_err();
        assert!(err.to_string().contains("empty") || err.to_string().contains("invalid"));
    }

    #[test]
    fn rel_serde_roundtrip() {
        let rel = Rel::new("parent").unwrap();
        let yaml = serde_yaml::to_string(&rel).unwrap();
        let parsed: Rel = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(rel, parsed);
    }

    #[test]
    fn rel_serde_normalizes_on_deserialize() {
        let yaml = "'PARENT'\n";
        let rel: Rel = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(rel.to_string(), "parent");
    }

    #[test]
    fn rel_serde_in_vec_context() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct TestStruct {
            rels: Vec<Rel>,
        }
        let s = TestStruct {
            rels: vec![Rel::new("parent").unwrap(), Rel::new("see-also").unwrap()],
        };
        let yaml = serde_yaml::to_string(&s).unwrap();
        let parsed: TestStruct = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(s, parsed);
    }

    #[test]
    fn rel_serde_rejects_invalid() {
        let yaml = "''\n";
        let result: Result<Rel, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    // ===========================================
    // Phase 6: Link Basic Construction
    // ===========================================

    fn test_note_id() -> NoteId {
        "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap()
    }

    #[test]
    fn link_new_with_single_rel() {
        let link = Link::new(test_note_id(), vec!["parent"]).unwrap();
        assert_eq!(link.rel().len(), 1);
        assert_eq!(link.rel()[0].as_str(), "parent");
    }

    #[test]
    fn link_new_with_multiple_rels() {
        let link = Link::new(test_note_id(), vec!["manager-of", "mentor-to"]).unwrap();
        assert_eq!(link.rel().len(), 2);
        assert_eq!(link.rel()[0].as_str(), "manager-of");
        assert_eq!(link.rel()[1].as_str(), "mentor-to");
    }

    #[test]
    fn link_new_with_context() {
        let link =
            Link::with_context(test_note_id(), vec!["parent"], "Hired me at Acme Corp").unwrap();
        assert_eq!(link.context(), Some("Hired me at Acme Corp"));
    }

    #[test]
    fn link_rejects_empty_rel_vec() {
        let result = Link::new(test_note_id(), Vec::<&str>::new());
        assert!(result.is_err());
    }

    #[test]
    fn link_target_accessor() {
        let target = test_note_id();
        let link = Link::new(target.clone(), vec!["parent"]).unwrap();
        assert_eq!(link.target(), &target);
    }

    #[test]
    fn link_rel_accessor() {
        let link = Link::new(test_note_id(), vec!["parent", "mentor"]).unwrap();
        let rels: Vec<&str> = link.rel().iter().map(|r| r.as_str()).collect();
        assert_eq!(rels, vec!["parent", "mentor"]);
    }

    #[test]
    fn link_context_accessor_none() {
        let link = Link::new(test_note_id(), vec!["parent"]).unwrap();
        assert_eq!(link.context(), None);
    }

    // ===========================================
    // Phase 7: Link Equality & Hashing
    // ===========================================

    #[test]
    fn link_equality_same_target_and_rels() {
        let link1 = Link::new(test_note_id(), vec!["parent"]).unwrap();
        let link2 = Link::new(test_note_id(), vec!["parent"]).unwrap();
        assert_eq!(link1, link2);
    }

    #[test]
    fn link_inequality_different_target() {
        let id1: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
        let id2: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Y".parse().unwrap();
        let link1 = Link::new(id1, vec!["parent"]).unwrap();
        let link2 = Link::new(id2, vec!["parent"]).unwrap();
        assert_ne!(link1, link2);
    }

    #[test]
    fn link_inequality_different_rels() {
        let link1 = Link::new(test_note_id(), vec!["parent"]).unwrap();
        let link2 = Link::new(test_note_id(), vec!["child"]).unwrap();
        assert_ne!(link1, link2);
    }

    #[test]
    fn link_equality_ignores_context() {
        let link1 = Link::with_context(test_note_id(), vec!["parent"], "context 1").unwrap();
        let link2 = Link::with_context(test_note_id(), vec!["parent"], "context 2").unwrap();
        assert_eq!(link1, link2);
    }

    #[test]
    fn link_hash_consistent_with_equality() {
        let link1 = Link::with_context(test_note_id(), vec!["parent"], "context 1").unwrap();
        let link2 = Link::with_context(test_note_id(), vec!["parent"], "context 2").unwrap();
        let mut set = HashSet::new();
        set.insert(link1);
        assert!(set.contains(&link2));
    }

    // ===========================================
    // Phase 8: Link Display & Debug
    // ===========================================

    #[test]
    fn link_display_format() {
        let link = Link::new(test_note_id(), vec!["parent", "mentor"]).unwrap();
        let display = format!("{}", link);
        assert!(display.contains("01HQ3K5M"));
        assert!(display.contains("parent"));
        assert!(display.contains("mentor"));
    }

    #[test]
    fn link_debug_format() {
        let link = Link::new(test_note_id(), vec!["parent"]).unwrap();
        let debug = format!("{:?}", link);
        assert!(debug.contains("Link"));
        assert!(debug.contains("target"));
        assert!(debug.contains("rel"));
    }

    // ===========================================
    // Phase 9: Link Serde with Field Mapping
    // ===========================================

    #[test]
    fn link_serde_roundtrip() {
        let link = Link::new(test_note_id(), vec!["parent"]).unwrap();
        let yaml = serde_yaml::to_string(&link).unwrap();
        let parsed: Link = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(link, parsed);
    }

    #[test]
    fn link_serde_with_context() {
        let link = Link::with_context(test_note_id(), vec!["parent"], "some context").unwrap();
        let yaml = serde_yaml::to_string(&link).unwrap();
        let parsed: Link = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.context(), Some("some context"));
    }

    #[test]
    fn link_serde_without_context() {
        let link = Link::new(test_note_id(), vec!["parent"]).unwrap();
        let yaml = serde_yaml::to_string(&link).unwrap();
        assert!(!yaml.contains("note:"));
    }

    #[test]
    fn link_serde_maps_note_field_to_context() {
        let yaml = r#"
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
rel:
  - parent
note: "Hired me at Acme Corp, 2019"
"#;
        let link: Link = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(link.context(), Some("Hired me at Acme Corp, 2019"));
    }

    #[test]
    fn link_serde_in_vec_context() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Note {
            links: Vec<Link>,
        }

        let note = Note {
            links: vec![
                Link::new(test_note_id(), vec!["parent"]).unwrap(),
                Link::with_context(
                    "01HQ4A2R9PXJK4QZPW8V2R6T9Y".parse().unwrap(),
                    vec!["see-also"],
                    "Related topic",
                )
                .unwrap(),
            ],
        };
        let yaml = serde_yaml::to_string(&note).unwrap();
        let parsed: Note = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(note, parsed);
    }

    #[test]
    fn link_serde_rejects_empty_rel() {
        let yaml = r#"
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
rel: []
"#;
        let result: Result<Link, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn link_serde_rejects_missing_id() {
        let yaml = r#"
rel:
  - parent
"#;
        let result: Result<Link, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }
}
