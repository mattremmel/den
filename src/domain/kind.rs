//! Note kind classification for different content types.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// The kind/type of a note, indicating what kind of content it represents.
///
/// Notes can represent different types of content like books, papers, transcripts,
/// or general notes. The kind affects which metadata fields are relevant.
///
/// # Default
///
/// The default kind is `Generic`, which represents a standard note without
/// specialized metadata requirements.
///
/// # Forward Compatibility
///
/// The `Other` variant allows for custom kinds not defined in the enum,
/// enabling forward compatibility with future kinds.
///
/// # Examples
///
/// ```
/// use den::domain::NoteKind;
///
/// let kind = NoteKind::Book;
/// assert_eq!(kind.as_str(), "book");
///
/// let parsed: NoteKind = "transcript".parse().unwrap();
/// assert_eq!(parsed, NoteKind::Transcript);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum NoteKind {
    /// A general-purpose note (default).
    #[default]
    Generic,
    /// A book or book chapter.
    Book,
    /// An academic paper or research article.
    Paper,
    /// A transcript of audio/video content (podcast, interview, lecture).
    Transcript,
    /// A web article or blog post.
    Article,
    /// A custom kind not defined in the enum.
    Other(String),
}

impl NoteKind {
    /// Returns the string representation of the kind.
    ///
    /// For `Other` variants, returns the custom kind string.
    pub fn as_str(&self) -> &str {
        match self {
            NoteKind::Generic => "generic",
            NoteKind::Book => "book",
            NoteKind::Paper => "paper",
            NoteKind::Transcript => "transcript",
            NoteKind::Article => "article",
            NoteKind::Other(s) => s,
        }
    }

    /// Returns true if this is the default kind (Generic).
    pub fn is_default(&self) -> bool {
        matches!(self, NoteKind::Generic)
    }

    /// Returns all known kinds (excluding Other).
    pub fn all_known() -> &'static [NoteKind] {
        &[
            NoteKind::Generic,
            NoteKind::Book,
            NoteKind::Paper,
            NoteKind::Transcript,
            NoteKind::Article,
        ]
    }

    /// Returns the names of all known kinds as strings.
    pub fn known_names() -> &'static [&'static str] {
        &["generic", "book", "paper", "transcript", "article"]
    }

    /// Parses a string into a known NoteKind, rejecting unknown kinds.
    ///
    /// Unlike `FromStr`, this method returns an error for unknown kinds
    /// instead of wrapping them in `Other`. Use this for validating user input.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the string doesn't match a known kind (case-insensitive).
    pub fn parse_strict(s: &str) -> Result<Self, ParseNoteKindError> {
        match s.to_lowercase().as_str() {
            "generic" => Ok(NoteKind::Generic),
            "book" => Ok(NoteKind::Book),
            "paper" => Ok(NoteKind::Paper),
            "transcript" => Ok(NoteKind::Transcript),
            "article" => Ok(NoteKind::Article),
            _ => Err(ParseNoteKindError {
                input: s.to_string(),
            }),
        }
    }
}

/// Error returned when parsing an unknown note kind with strict validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseNoteKindError {
    input: String,
}

impl ParseNoteKindError {
    /// Returns the invalid input that caused the error.
    pub fn input(&self) -> &str {
        &self.input
    }
}

impl fmt::Display for ParseNoteKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown kind '{}': valid kinds are {}",
            self.input,
            NoteKind::known_names().join(", ")
        )
    }
}

impl std::error::Error for ParseNoteKindError {}

impl fmt::Display for NoteKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for NoteKind {
    type Err = std::convert::Infallible;

    /// Parses a string into a NoteKind.
    ///
    /// Known kinds are matched case-insensitively. Unknown kinds become `Other`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "generic" => NoteKind::Generic,
            "book" => NoteKind::Book,
            "paper" => NoteKind::Paper,
            "transcript" => NoteKind::Transcript,
            "article" => NoteKind::Article,
            _ => NoteKind::Other(s.to_string()),
        })
    }
}

impl Serialize for NoteKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for NoteKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(s.parse().unwrap()) // FromStr is infallible
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // ===========================================
    // Basic Construction & Accessors
    // ===========================================

    #[test]
    fn default_is_generic() {
        assert_eq!(NoteKind::default(), NoteKind::Generic);
    }

    #[test]
    fn as_str_returns_expected_values() {
        assert_eq!(NoteKind::Generic.as_str(), "generic");
        assert_eq!(NoteKind::Book.as_str(), "book");
        assert_eq!(NoteKind::Paper.as_str(), "paper");
        assert_eq!(NoteKind::Transcript.as_str(), "transcript");
        assert_eq!(NoteKind::Article.as_str(), "article");
        assert_eq!(NoteKind::Other("custom".to_string()).as_str(), "custom");
    }

    #[test]
    fn is_default_returns_true_only_for_generic() {
        assert!(NoteKind::Generic.is_default());
        assert!(!NoteKind::Book.is_default());
        assert!(!NoteKind::Paper.is_default());
        assert!(!NoteKind::Transcript.is_default());
        assert!(!NoteKind::Article.is_default());
        assert!(!NoteKind::Other("custom".to_string()).is_default());
    }

    #[test]
    fn all_known_returns_known_variants() {
        let all = NoteKind::all_known();
        assert_eq!(all.len(), 5);
        assert!(all.contains(&NoteKind::Generic));
        assert!(all.contains(&NoteKind::Book));
        assert!(all.contains(&NoteKind::Paper));
        assert!(all.contains(&NoteKind::Transcript));
        assert!(all.contains(&NoteKind::Article));
    }

    // ===========================================
    // FromStr Parsing
    // ===========================================

    #[test]
    fn parse_known_kinds_case_insensitive() {
        assert_eq!("generic".parse::<NoteKind>().unwrap(), NoteKind::Generic);
        assert_eq!("GENERIC".parse::<NoteKind>().unwrap(), NoteKind::Generic);
        assert_eq!("Generic".parse::<NoteKind>().unwrap(), NoteKind::Generic);
        assert_eq!("book".parse::<NoteKind>().unwrap(), NoteKind::Book);
        assert_eq!("BOOK".parse::<NoteKind>().unwrap(), NoteKind::Book);
        assert_eq!("paper".parse::<NoteKind>().unwrap(), NoteKind::Paper);
        assert_eq!(
            "transcript".parse::<NoteKind>().unwrap(),
            NoteKind::Transcript
        );
        assert_eq!("article".parse::<NoteKind>().unwrap(), NoteKind::Article);
    }

    #[test]
    fn parse_unknown_kind_becomes_other() {
        let kind: NoteKind = "custom-kind".parse().unwrap();
        assert_eq!(kind, NoteKind::Other("custom-kind".to_string()));
    }

    #[test]
    fn parse_preserves_case_for_other() {
        let kind: NoteKind = "MyCustomKind".parse().unwrap();
        assert_eq!(kind.as_str(), "MyCustomKind");
    }

    // ===========================================
    // Strict Parsing (for CLI input validation)
    // ===========================================

    #[test]
    fn parse_strict_accepts_known_kinds() {
        assert_eq!(
            NoteKind::parse_strict("generic").unwrap(),
            NoteKind::Generic
        );
        assert_eq!(NoteKind::parse_strict("book").unwrap(), NoteKind::Book);
        assert_eq!(NoteKind::parse_strict("paper").unwrap(), NoteKind::Paper);
        assert_eq!(
            NoteKind::parse_strict("transcript").unwrap(),
            NoteKind::Transcript
        );
        assert_eq!(
            NoteKind::parse_strict("article").unwrap(),
            NoteKind::Article
        );
    }

    #[test]
    fn parse_strict_is_case_insensitive() {
        assert_eq!(NoteKind::parse_strict("BOOK").unwrap(), NoteKind::Book);
        assert_eq!(NoteKind::parse_strict("Book").unwrap(), NoteKind::Book);
        assert_eq!(NoteKind::parse_strict("PAPER").unwrap(), NoteKind::Paper);
    }

    #[test]
    fn parse_strict_rejects_unknown_kinds() {
        let err = NoteKind::parse_strict("custom-kind").unwrap_err();
        assert_eq!(err.input(), "custom-kind");
        assert!(err.to_string().contains("unknown kind"));
        assert!(err.to_string().contains("custom-kind"));
    }

    #[test]
    fn parse_strict_error_lists_valid_kinds() {
        let err = NoteKind::parse_strict("invalid").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("generic"));
        assert!(msg.contains("book"));
        assert!(msg.contains("paper"));
        assert!(msg.contains("transcript"));
        assert!(msg.contains("article"));
    }

    #[test]
    fn known_names_returns_all_kind_strings() {
        let names = NoteKind::known_names();
        assert_eq!(names.len(), 5);
        assert!(names.contains(&"generic"));
        assert!(names.contains(&"book"));
        assert!(names.contains(&"paper"));
        assert!(names.contains(&"transcript"));
        assert!(names.contains(&"article"));
    }

    // ===========================================
    // Display
    // ===========================================

    #[test]
    fn display_matches_as_str() {
        assert_eq!(format!("{}", NoteKind::Generic), "generic");
        assert_eq!(format!("{}", NoteKind::Book), "book");
        assert_eq!(format!("{}", NoteKind::Paper), "paper");
        assert_eq!(format!("{}", NoteKind::Transcript), "transcript");
        assert_eq!(format!("{}", NoteKind::Article), "article");
        assert_eq!(
            format!("{}", NoteKind::Other("custom".to_string())),
            "custom"
        );
    }

    // ===========================================
    // Serde Serialization
    // ===========================================

    #[test]
    fn serde_serialize_known_kinds() {
        assert_eq!(
            serde_yaml::to_string(&NoteKind::Generic).unwrap().trim(),
            "generic"
        );
        assert_eq!(
            serde_yaml::to_string(&NoteKind::Book).unwrap().trim(),
            "book"
        );
        assert_eq!(
            serde_yaml::to_string(&NoteKind::Paper).unwrap().trim(),
            "paper"
        );
        assert_eq!(
            serde_yaml::to_string(&NoteKind::Transcript).unwrap().trim(),
            "transcript"
        );
        assert_eq!(
            serde_yaml::to_string(&NoteKind::Article).unwrap().trim(),
            "article"
        );
    }

    #[test]
    fn serde_serialize_other() {
        let kind = NoteKind::Other("custom-kind".to_string());
        assert_eq!(serde_yaml::to_string(&kind).unwrap().trim(), "custom-kind");
    }

    // ===========================================
    // Serde Deserialization
    // ===========================================

    #[test]
    fn serde_deserialize_known_kinds() {
        let generic: NoteKind = serde_yaml::from_str("generic").unwrap();
        assert_eq!(generic, NoteKind::Generic);

        let book: NoteKind = serde_yaml::from_str("book").unwrap();
        assert_eq!(book, NoteKind::Book);

        let paper: NoteKind = serde_yaml::from_str("paper").unwrap();
        assert_eq!(paper, NoteKind::Paper);

        let transcript: NoteKind = serde_yaml::from_str("transcript").unwrap();
        assert_eq!(transcript, NoteKind::Transcript);

        let article: NoteKind = serde_yaml::from_str("article").unwrap();
        assert_eq!(article, NoteKind::Article);
    }

    #[test]
    fn serde_deserialize_unknown_becomes_other() {
        let kind: NoteKind = serde_yaml::from_str("my-custom-kind").unwrap();
        assert_eq!(kind, NoteKind::Other("my-custom-kind".to_string()));
    }

    // ===========================================
    // Serde Roundtrip
    // ===========================================

    #[test]
    fn serde_roundtrip_all_known_kinds() {
        for kind in NoteKind::all_known() {
            let yaml = serde_yaml::to_string(kind).unwrap();
            let parsed: NoteKind = serde_yaml::from_str(&yaml).unwrap();
            assert_eq!(&parsed, kind);
        }
    }

    #[test]
    fn serde_roundtrip_other() {
        let kind = NoteKind::Other("custom-type".to_string());
        let yaml = serde_yaml::to_string(&kind).unwrap();
        let parsed: NoteKind = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed, kind);
    }

    // ===========================================
    // Standard Traits
    // ===========================================

    #[test]
    fn clone_produces_equal_kind() {
        let kind = NoteKind::Book;
        assert_eq!(kind.clone(), kind);

        let other = NoteKind::Other("custom".to_string());
        assert_eq!(other.clone(), other);
    }

    #[test]
    fn equality_works_correctly() {
        assert_eq!(NoteKind::Book, NoteKind::Book);
        assert_ne!(NoteKind::Book, NoteKind::Paper);
        assert_ne!(
            NoteKind::Other("a".to_string()),
            NoteKind::Other("b".to_string())
        );
        assert_eq!(
            NoteKind::Other("same".to_string()),
            NoteKind::Other("same".to_string())
        );
    }

    #[test]
    fn hash_is_consistent() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(NoteKind::Book);
        set.insert(NoteKind::Book); // duplicate
        set.insert(NoteKind::Paper);

        assert_eq!(set.len(), 2);
        assert!(set.contains(&NoteKind::Book));
        assert!(set.contains(&NoteKind::Paper));
    }

    #[test]
    fn debug_includes_variant() {
        let debug = format!("{:?}", NoteKind::Book);
        assert!(debug.contains("Book"));

        let other_debug = format!("{:?}", NoteKind::Other("custom".to_string()));
        assert!(other_debug.contains("Other"));
        assert!(other_debug.contains("custom"));
    }
}
