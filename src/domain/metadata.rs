//! Metadata types for different note kinds.
//!
//! Each note kind can have associated metadata with typed fields appropriate
//! for that content type. All metadata types support arbitrary additional fields
//! via a flattened HashMap.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use super::NoteKind;

/// Chapter information for transcripts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    /// Chapter title.
    pub title: String,
    /// Start time in seconds from the beginning.
    pub start_seconds: u32,
}

/// Metadata for book notes.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BookMetadata {
    /// Authors of the book.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    /// ISBN (10 or 13 digit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isbn: Option<String>,
    /// Publisher name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    /// Publication year.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    /// Arbitrary additional fields.
    #[serde(flatten, default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, Value>,
}

impl BookMetadata {
    /// Creates a new BookMetadata with authors.
    pub fn new(authors: Vec<String>) -> Self {
        Self {
            authors,
            ..Default::default()
        }
    }

    /// Returns true if all fields are empty/default.
    pub fn is_empty(&self) -> bool {
        self.authors.is_empty()
            && self.isbn.is_none()
            && self.publisher.is_none()
            && self.year.is_none()
            && self.extra.is_empty()
    }
}

/// Metadata for academic paper notes.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PaperMetadata {
    /// Authors of the paper.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    /// Digital Object Identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    /// Journal or conference name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub journal: Option<String>,
    /// Publication year.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    /// ArXiv identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arxiv: Option<String>,
    /// Arbitrary additional fields.
    #[serde(flatten, default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, Value>,
}

impl PaperMetadata {
    /// Creates a new PaperMetadata with authors.
    pub fn new(authors: Vec<String>) -> Self {
        Self {
            authors,
            ..Default::default()
        }
    }

    /// Returns true if all fields are empty/default.
    pub fn is_empty(&self) -> bool {
        self.authors.is_empty()
            && self.doi.is_none()
            && self.journal.is_none()
            && self.year.is_none()
            && self.arxiv.is_none()
            && self.extra.is_empty()
    }
}

/// Metadata for transcript notes (podcast, interview, lecture, etc.).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TranscriptMetadata {
    /// Source URL (YouTube, podcast feed, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Speakers in the transcript.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub speakers: Vec<String>,
    /// Duration in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u32>,
    /// Chapter markers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chapters: Vec<Chapter>,
    /// Arbitrary additional fields.
    #[serde(flatten, default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, Value>,
}

impl TranscriptMetadata {
    /// Creates a new TranscriptMetadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new TranscriptMetadata with speakers.
    pub fn with_speakers(speakers: Vec<String>) -> Self {
        Self {
            speakers,
            ..Default::default()
        }
    }

    /// Returns true if all fields are empty/default.
    pub fn is_empty(&self) -> bool {
        self.source.is_none()
            && self.speakers.is_empty()
            && self.duration_seconds.is_none()
            && self.chapters.is_empty()
            && self.extra.is_empty()
    }
}

/// Metadata for article notes (web articles, blog posts).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ArticleMetadata {
    /// Authors of the article.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    /// Source URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Publication/site name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication: Option<String>,
    /// Publication date (ISO 8601 date string).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// Arbitrary additional fields.
    #[serde(flatten, default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, Value>,
}

impl ArticleMetadata {
    /// Creates a new ArticleMetadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if all fields are empty/default.
    pub fn is_empty(&self) -> bool {
        self.authors.is_empty()
            && self.url.is_none()
            && self.publication.is_none()
            && self.date.is_none()
            && self.extra.is_empty()
    }
}

/// Generic metadata for notes that don't fit other categories.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GenericMetadata {
    /// Arbitrary additional fields.
    #[serde(flatten, default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, Value>,
}

impl GenericMetadata {
    /// Creates a new empty GenericMetadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if all fields are empty.
    pub fn is_empty(&self) -> bool {
        self.extra.is_empty()
    }
}

/// Metadata for a note, dispatched by kind.
///
/// The metadata variant should match the note's kind. When deserializing,
/// we use the kind to determine which typed metadata to expect.
#[derive(Debug, Clone, PartialEq)]
pub enum NoteMetadata {
    /// Generic metadata (just extra fields).
    Generic(GenericMetadata),
    /// Book metadata.
    Book(BookMetadata),
    /// Paper metadata.
    Paper(PaperMetadata),
    /// Transcript metadata.
    Transcript(TranscriptMetadata),
    /// Article metadata.
    Article(ArticleMetadata),
}

impl NoteMetadata {
    /// Returns true if the metadata is empty (no fields set).
    pub fn is_empty(&self) -> bool {
        match self {
            NoteMetadata::Generic(m) => m.is_empty(),
            NoteMetadata::Book(m) => m.is_empty(),
            NoteMetadata::Paper(m) => m.is_empty(),
            NoteMetadata::Transcript(m) => m.is_empty(),
            NoteMetadata::Article(m) => m.is_empty(),
        }
    }

    /// Returns the authors from metadata, if applicable.
    pub fn authors(&self) -> &[String] {
        match self {
            NoteMetadata::Book(m) => &m.authors,
            NoteMetadata::Paper(m) => &m.authors,
            NoteMetadata::Article(m) => &m.authors,
            NoteMetadata::Generic(_) | NoteMetadata::Transcript(_) => &[],
        }
    }

    /// Returns the speakers from metadata, if applicable.
    pub fn speakers(&self) -> &[String] {
        match self {
            NoteMetadata::Transcript(m) => &m.speakers,
            _ => &[],
        }
    }

    /// Creates an empty metadata value for the given kind.
    pub fn empty_for_kind(kind: &NoteKind) -> Self {
        match kind {
            NoteKind::Generic | NoteKind::Other(_) => NoteMetadata::Generic(GenericMetadata::new()),
            NoteKind::Book => NoteMetadata::Book(BookMetadata::default()),
            NoteKind::Paper => NoteMetadata::Paper(PaperMetadata::default()),
            NoteKind::Transcript => NoteMetadata::Transcript(TranscriptMetadata::new()),
            NoteKind::Article => NoteMetadata::Article(ArticleMetadata::new()),
        }
    }

    /// Deserialize metadata from a serde_json::Value based on the kind.
    pub fn from_value(kind: &NoteKind, value: Value) -> Result<Self, serde_json::Error> {
        match kind {
            NoteKind::Generic | NoteKind::Other(_) => {
                serde_json::from_value(value).map(NoteMetadata::Generic)
            }
            NoteKind::Book => serde_json::from_value(value).map(NoteMetadata::Book),
            NoteKind::Paper => serde_json::from_value(value).map(NoteMetadata::Paper),
            NoteKind::Transcript => serde_json::from_value(value).map(NoteMetadata::Transcript),
            NoteKind::Article => serde_json::from_value(value).map(NoteMetadata::Article),
        }
    }

    /// Serialize metadata to a serde_json::Value.
    pub fn to_value(&self) -> Result<Value, serde_json::Error> {
        match self {
            NoteMetadata::Generic(m) => serde_json::to_value(m),
            NoteMetadata::Book(m) => serde_json::to_value(m),
            NoteMetadata::Paper(m) => serde_json::to_value(m),
            NoteMetadata::Transcript(m) => serde_json::to_value(m),
            NoteMetadata::Article(m) => serde_json::to_value(m),
        }
    }
}

impl Default for NoteMetadata {
    fn default() -> Self {
        NoteMetadata::Generic(GenericMetadata::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // ===========================================
    // BookMetadata Tests
    // ===========================================

    #[test]
    fn book_metadata_default_is_empty() {
        let meta = BookMetadata::default();
        assert!(meta.is_empty());
        assert!(meta.authors.is_empty());
        assert!(meta.isbn.is_none());
        assert!(meta.publisher.is_none());
        assert!(meta.year.is_none());
    }

    #[test]
    fn book_metadata_new_with_authors() {
        let meta = BookMetadata::new(vec!["Author One".to_string()]);
        assert!(!meta.is_empty());
        assert_eq!(meta.authors, vec!["Author One"]);
    }

    #[test]
    fn book_metadata_serde_roundtrip() {
        let meta = BookMetadata {
            authors: vec!["Martin Kleppmann".to_string()],
            isbn: Some("978-1449373320".to_string()),
            publisher: Some("O'Reilly Media".to_string()),
            year: Some(2017),
            extra: HashMap::new(),
        };

        let yaml = serde_yaml::to_string(&meta).unwrap();
        let parsed: BookMetadata = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed, meta);
    }

    #[test]
    fn book_metadata_with_extra_fields() {
        let yaml = r#"
authors:
  - Author Name
isbn: "123456"
edition: 2
format: hardcover
"#;
        let meta: BookMetadata = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(meta.authors, vec!["Author Name"]);
        assert_eq!(meta.isbn, Some("123456".to_string()));
        assert_eq!(meta.extra.get("edition"), Some(&Value::from(2)));
        assert_eq!(meta.extra.get("format"), Some(&Value::from("hardcover")));
    }

    #[test]
    fn book_metadata_empty_fields_not_serialized() {
        let meta = BookMetadata::new(vec!["Author".to_string()]);
        let yaml = serde_yaml::to_string(&meta).unwrap();
        assert!(!yaml.contains("isbn"));
        assert!(!yaml.contains("publisher"));
        assert!(!yaml.contains("year"));
    }

    // ===========================================
    // PaperMetadata Tests
    // ===========================================

    #[test]
    fn paper_metadata_default_is_empty() {
        let meta = PaperMetadata::default();
        assert!(meta.is_empty());
    }

    #[test]
    fn paper_metadata_serde_roundtrip() {
        let meta = PaperMetadata {
            authors: vec!["Author One".to_string(), "Author Two".to_string()],
            doi: Some("10.1234/example".to_string()),
            journal: Some("NeurIPS 2023".to_string()),
            year: Some(2023),
            arxiv: Some("2301.12345".to_string()),
            extra: HashMap::new(),
        };

        let yaml = serde_yaml::to_string(&meta).unwrap();
        let parsed: PaperMetadata = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed, meta);
    }

    #[test]
    fn paper_metadata_with_extra_fields() {
        let yaml = r#"
authors: []
citations: 150
venue: ICML
"#;
        let meta: PaperMetadata = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(meta.extra.get("citations"), Some(&Value::from(150)));
        assert_eq!(meta.extra.get("venue"), Some(&Value::from("ICML")));
    }

    // ===========================================
    // TranscriptMetadata Tests
    // ===========================================

    #[test]
    fn transcript_metadata_default_is_empty() {
        let meta = TranscriptMetadata::default();
        assert!(meta.is_empty());
    }

    #[test]
    fn transcript_metadata_with_speakers() {
        let meta = TranscriptMetadata::with_speakers(vec![
            "Host Name".to_string(),
            "Guest Name".to_string(),
        ]);
        assert!(!meta.is_empty());
        assert_eq!(meta.speakers.len(), 2);
    }

    #[test]
    fn transcript_metadata_serde_roundtrip() {
        let meta = TranscriptMetadata {
            source: Some("https://youtube.com/watch?v=abc123".to_string()),
            speakers: vec!["Host".to_string(), "Guest".to_string()],
            duration_seconds: Some(3600),
            chapters: vec![
                Chapter {
                    title: "Introduction".to_string(),
                    start_seconds: 0,
                },
                Chapter {
                    title: "Main Topic".to_string(),
                    start_seconds: 300,
                },
            ],
            extra: HashMap::new(),
        };

        let yaml = serde_yaml::to_string(&meta).unwrap();
        let parsed: TranscriptMetadata = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed, meta);
    }

    #[test]
    fn transcript_metadata_with_chapters() {
        let yaml = r#"
source: https://example.com/video
speakers:
  - Speaker A
  - Speaker B
duration_seconds: 1800
chapters:
  - title: Intro
    start_seconds: 0
  - title: Discussion
    start_seconds: 120
"#;
        let meta: TranscriptMetadata = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(meta.chapters.len(), 2);
        assert_eq!(meta.chapters[0].title, "Intro");
        assert_eq!(meta.chapters[1].start_seconds, 120);
    }

    // ===========================================
    // ArticleMetadata Tests
    // ===========================================

    #[test]
    fn article_metadata_default_is_empty() {
        let meta = ArticleMetadata::default();
        assert!(meta.is_empty());
    }

    #[test]
    fn article_metadata_serde_roundtrip() {
        let meta = ArticleMetadata {
            authors: vec!["Blog Author".to_string()],
            url: Some("https://example.com/article".to_string()),
            publication: Some("Tech Blog".to_string()),
            date: Some("2024-01-15".to_string()),
            extra: HashMap::new(),
        };

        let yaml = serde_yaml::to_string(&meta).unwrap();
        let parsed: ArticleMetadata = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed, meta);
    }

    // ===========================================
    // GenericMetadata Tests
    // ===========================================

    #[test]
    fn generic_metadata_default_is_empty() {
        let meta = GenericMetadata::default();
        assert!(meta.is_empty());
    }

    #[test]
    fn generic_metadata_accepts_arbitrary_fields() {
        let yaml = r#"
custom_field: value
nested:
  inner: true
"#;
        let meta: GenericMetadata = serde_yaml::from_str(yaml).unwrap();
        assert!(!meta.is_empty());
        assert_eq!(meta.extra.get("custom_field"), Some(&Value::from("value")));
    }

    // ===========================================
    // NoteMetadata Tests
    // ===========================================

    #[test]
    fn note_metadata_default_is_generic_empty() {
        let meta = NoteMetadata::default();
        assert!(meta.is_empty());
        assert!(matches!(meta, NoteMetadata::Generic(_)));
    }

    #[test]
    fn note_metadata_empty_for_kind() {
        assert!(matches!(
            NoteMetadata::empty_for_kind(&NoteKind::Generic),
            NoteMetadata::Generic(_)
        ));
        assert!(matches!(
            NoteMetadata::empty_for_kind(&NoteKind::Book),
            NoteMetadata::Book(_)
        ));
        assert!(matches!(
            NoteMetadata::empty_for_kind(&NoteKind::Paper),
            NoteMetadata::Paper(_)
        ));
        assert!(matches!(
            NoteMetadata::empty_for_kind(&NoteKind::Transcript),
            NoteMetadata::Transcript(_)
        ));
        assert!(matches!(
            NoteMetadata::empty_for_kind(&NoteKind::Article),
            NoteMetadata::Article(_)
        ));
        assert!(matches!(
            NoteMetadata::empty_for_kind(&NoteKind::Other("custom".to_string())),
            NoteMetadata::Generic(_)
        ));
    }

    #[test]
    fn note_metadata_authors_returns_correct_values() {
        let book = NoteMetadata::Book(BookMetadata::new(vec!["Author A".to_string()]));
        assert_eq!(book.authors(), &["Author A"]);

        let paper = NoteMetadata::Paper(PaperMetadata::new(vec!["Author B".to_string()]));
        assert_eq!(paper.authors(), &["Author B"]);

        let transcript = NoteMetadata::Transcript(TranscriptMetadata::new());
        assert!(transcript.authors().is_empty());

        let generic = NoteMetadata::Generic(GenericMetadata::new());
        assert!(generic.authors().is_empty());
    }

    #[test]
    fn note_metadata_speakers_returns_correct_values() {
        let transcript = NoteMetadata::Transcript(TranscriptMetadata::with_speakers(vec![
            "Speaker A".to_string(),
        ]));
        assert_eq!(transcript.speakers(), &["Speaker A"]);

        let book = NoteMetadata::Book(BookMetadata::default());
        assert!(book.speakers().is_empty());
    }

    #[test]
    fn note_metadata_from_value_book() {
        let json = serde_json::json!({
            "authors": ["Author Name"],
            "isbn": "123456",
            "year": 2023
        });

        let meta = NoteMetadata::from_value(&NoteKind::Book, json).unwrap();
        if let NoteMetadata::Book(m) = meta {
            assert_eq!(m.authors, vec!["Author Name"]);
            assert_eq!(m.isbn, Some("123456".to_string()));
            assert_eq!(m.year, Some(2023));
        } else {
            panic!("Expected Book metadata");
        }
    }

    #[test]
    fn note_metadata_to_value_book() {
        let meta = NoteMetadata::Book(BookMetadata {
            authors: vec!["Author".to_string()],
            isbn: Some("123".to_string()),
            ..Default::default()
        });

        let value = meta.to_value().unwrap();
        assert_eq!(value["authors"], serde_json::json!(["Author"]));
        assert_eq!(value["isbn"], serde_json::json!("123"));
    }

    #[test]
    fn note_metadata_roundtrip_through_value() {
        let original = NoteMetadata::Paper(PaperMetadata {
            authors: vec!["Alice".to_string(), "Bob".to_string()],
            doi: Some("10.1234/test".to_string()),
            year: Some(2024),
            ..Default::default()
        });

        let value = original.to_value().unwrap();
        let restored = NoteMetadata::from_value(&NoteKind::Paper, value).unwrap();

        assert_eq!(original, restored);
    }
}
