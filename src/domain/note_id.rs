//! ULID-based note identifier with prefix extraction and serde support.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;
use std::time::SystemTime;
use ulid::Ulid;

/// A unique identifier for notes based on ULID.
///
/// ULIDs are 26-character Crockford Base32 encoded strings that are:
/// - Lexicographically sortable (chronological order)
/// - Globally unique
/// - URL-safe
///
/// # Examples
///
/// ```
/// use den::domain::NoteId;
///
/// let id = NoteId::new();
/// println!("Full ID: {}", id);        // e.g., "01HQ3K5M7NXJK4QZPW8V2R6T9Y"
/// println!("Prefix: {}", id.prefix()); // e.g., "01HQ3K5M"
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct NoteId(Ulid);

impl NoteId {
    /// Creates a new NoteId with the current timestamp.
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    /// Creates a NoteId from a specific datetime (useful for testing).
    pub fn from_datetime(datetime: DateTime<Utc>) -> Self {
        let system_time: SystemTime = datetime.into();
        Self(Ulid::from_datetime(system_time))
    }

    /// Returns the 10-character prefix of the ULID.
    ///
    /// This prefix is used in filenames (e.g., `01HQ3K5M7N-api-design.md`).
    /// The first 10 characters encode the full 48-bit millisecond timestamp,
    /// ensuring unique prefixes for notes created at different times.
    pub fn prefix(&self) -> String {
        self.0.to_string()[..10].to_string()
    }

    /// Returns the timestamp when this ID was created.
    pub fn timestamp(&self) -> DateTime<Utc> {
        let millis = self.0.timestamp_ms();
        DateTime::from_timestamp_millis(millis as i64).expect("ULID timestamp should be valid")
    }
}

impl Default for NoteId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for NoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NoteId(\"{}\")", self.0)
    }
}

/// Error returned when parsing an invalid ULID string.
#[derive(Debug, Clone)]
pub struct ParseNoteIdError {
    value: String,
    reason: String,
}

impl ParseNoteIdError {
    /// Returns the invalid value that caused this error.
    pub fn invalid_value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for ParseNoteIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid ULID '{}': {}", self.value, self.reason)
    }
}

impl std::error::Error for ParseNoteIdError {}

impl FromStr for NoteId {
    type Err = ParseNoteIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ulid::from_string(s)
            .map(NoteId)
            .map_err(|e| ParseNoteIdError {
                value: s.to_string(),
                reason: e.to_string(),
            })
    }
}

impl Serialize for NoteId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for NoteId {
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

    #[test]
    fn new_creates_valid_ulid() {
        let id = NoteId::new();
        let s = id.to_string();
        assert_eq!(s.len(), 26, "ULID should be 26 characters");
        assert!(
            s.chars().all(|c| c.is_ascii_alphanumeric()),
            "ULID should only contain alphanumeric characters"
        );
    }

    #[test]
    fn prefix_returns_first_10_chars() {
        let id = NoteId::new();
        let prefix = id.prefix();
        let full = id.to_string();
        assert_eq!(prefix.len(), 10);
        assert_eq!(prefix, &full[..10]);
    }

    #[test]
    fn prefix_for_known_ulid() {
        let id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
        assert_eq!(id.prefix(), "01HQ3K5M7N");
    }

    #[test]
    fn timestamp_returns_creation_time() {
        // Use millisecond-truncated times since ULID only has ms precision
        let before = Utc::now().timestamp_millis();
        let id = NoteId::new();
        let after = Utc::now().timestamp_millis();

        let ts = id.timestamp().timestamp_millis();
        assert!(ts >= before, "timestamp should be >= before creation");
        assert!(ts <= after, "timestamp should be <= after creation");
    }

    #[test]
    fn from_datetime_creates_id_with_correct_timestamp() {
        let dt = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let id = NoteId::from_datetime(dt);

        // Timestamp should match within millisecond precision
        let ts = id.timestamp();
        assert_eq!(
            ts.timestamp_millis(),
            dt.timestamp_millis(),
            "timestamps should match"
        );
    }

    #[test]
    fn parse_valid_ulid_string() {
        let s = "01HQ3K5M7NXJK4QZPW8V2R6T9Y";
        let id: NoteId = s.parse().expect("should parse valid ULID");
        assert_eq!(id.to_string(), s);
    }

    #[test]
    fn parse_invalid_ulid_too_short() {
        let result: Result<NoteId, _> = "01HQ3K5M".parse();
        assert!(result.is_err(), "short string should fail to parse");
    }

    #[test]
    fn parse_invalid_ulid_bad_chars() {
        // 'I', 'L', 'O', 'U' are not valid in Crockford Base32
        let result: Result<NoteId, _> = "IIIIIIIIIIIIIIIIIIIIIIIIII".parse();
        assert!(result.is_err(), "invalid characters should fail to parse");
    }

    #[test]
    fn equality_works() {
        let s = "01HQ3K5M7NXJK4QZPW8V2R6T9Y";
        let id1: NoteId = s.parse().unwrap();
        let id2: NoteId = s.parse().unwrap();
        let id3 = NoteId::new();

        assert_eq!(id1, id2, "same ULID strings should be equal");
        assert_ne!(id1, id3, "different ULIDs should not be equal");
    }

    #[test]
    fn hash_consistent() {
        let s = "01HQ3K5M7NXJK4QZPW8V2R6T9Y";
        let id1: NoteId = s.parse().unwrap();
        let id2: NoteId = s.parse().unwrap();

        let mut set = HashSet::new();
        set.insert(id1.clone());
        assert!(set.contains(&id2), "equal IDs should have same hash");

        let id3 = NoteId::new();
        set.insert(id3.clone());
        assert_eq!(set.len(), 2, "HashSet should contain 2 unique IDs");
    }

    #[test]
    fn serde_roundtrip() {
        let id = NoteId::new();
        let yaml = serde_yaml::to_string(&id).expect("should serialize");
        let parsed: NoteId = serde_yaml::from_str(&yaml).expect("should deserialize");
        assert_eq!(id, parsed);
    }

    #[test]
    fn serde_in_struct_context() {
        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct NoteFrontmatter {
            id: NoteId,
            title: String,
        }

        let fm = NoteFrontmatter {
            id: "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap(),
            title: "Test Note".to_string(),
        };

        let yaml = serde_yaml::to_string(&fm).expect("should serialize");
        assert!(yaml.contains("01HQ3K5M7NXJK4QZPW8V2R6T9Y"));

        let parsed: NoteFrontmatter = serde_yaml::from_str(&yaml).expect("should deserialize");
        assert_eq!(fm, parsed);
    }

    #[test]
    fn multiple_new_ids_are_unique() {
        let ids: Vec<NoteId> = (0..100).map(|_| NoteId::new()).collect();
        let unique: HashSet<_> = ids.iter().collect();
        assert_eq!(
            ids.len(),
            unique.len(),
            "all generated IDs should be unique"
        );
    }

    #[test]
    fn ids_sort_chronologically() {
        let dt1 = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let dt2 = DateTime::parse_from_rfc3339("2024-06-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let dt3 = DateTime::parse_from_rfc3339("2024-12-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let id1 = NoteId::from_datetime(dt1);
        let id2 = NoteId::from_datetime(dt2);
        let id3 = NoteId::from_datetime(dt3);

        // Lexicographic comparison should match chronological order
        assert!(
            id1.to_string() < id2.to_string(),
            "earlier ID should sort before later"
        );
        assert!(
            id2.to_string() < id3.to_string(),
            "earlier ID should sort before later"
        );

        // Test via Vec sort
        let mut ids = vec![id3.to_string(), id1.to_string(), id2.to_string()];
        ids.sort();
        assert_eq!(ids, vec![id1.to_string(), id2.to_string(), id3.to_string()]);
    }

    #[test]
    fn debug_format() {
        let id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
        let debug = format!("{:?}", id);
        assert_eq!(debug, "NoteId(\"01HQ3K5M7NXJK4QZPW8V2R6T9Y\")");
    }

    #[test]
    fn parse_error_display() {
        let result: Result<NoteId, ParseNoteIdError> = "invalid".parse();
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("invalid ULID"),
            "error should mention invalid ULID"
        );
    }

    // ===========================================
    // Phase 10: Structured Error Context
    // ===========================================

    #[test]
    fn parse_error_contains_invalid_value() {
        let err: ParseNoteIdError = "invalid".parse::<NoteId>().unwrap_err();
        assert_eq!(err.invalid_value(), "invalid");
    }

    #[test]
    fn parse_error_display_includes_value() {
        let err: ParseNoteIdError = "bad".parse::<NoteId>().unwrap_err();
        assert!(err.to_string().contains("'bad'"));
    }
}
