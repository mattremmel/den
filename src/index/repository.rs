//! IndexRepository trait and result types.

use crate::domain::{Note, NoteId, Tag, Topic};
use crate::infra::ContentHash;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use thiserror::Error;

// ===========================================
// Cycle 1: IndexError Type
// ===========================================

/// Errors that can occur during index operations.
#[derive(Debug, Error)]
pub enum IndexError {
    /// The requested note was not found in the index.
    #[error("note not found: {id}")]
    NoteNotFound { id: String },

    /// A database error occurred.
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// The query is invalid.
    #[error("invalid query: {0}")]
    InvalidQuery(String),

    /// An I/O error occurred.
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Result type for index operations.
pub type IndexResult<T> = Result<T, IndexError>;

// ===========================================
// Cycle 2: IndexedNote Basic Structure
// ===========================================

/// A note as stored in the index.
///
/// This represents the indexed view of a note, including metadata
/// for efficient querying without reading the source file.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexedNote {
    id: NoteId,
    title: String,
    description: Option<String>,
    created: DateTime<Utc>,
    modified: DateTime<Utc>,
    path: PathBuf,
    content_hash: ContentHash,
    topics: Vec<Topic>,
    aliases: Vec<String>,
    tags: Vec<Tag>,
}

impl IndexedNote {
    /// Creates a new IndexedNote with all fields.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: NoteId,
        title: impl Into<String>,
        description: Option<impl Into<String>>,
        created: DateTime<Utc>,
        modified: DateTime<Utc>,
        path: PathBuf,
        content_hash: ContentHash,
        topics: Vec<Topic>,
        aliases: Vec<String>,
        tags: Vec<Tag>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            description: description.map(|d| d.into()),
            created,
            modified,
            path,
            content_hash,
            topics,
            aliases,
            tags,
        }
    }

    /// Returns the note's unique identifier.
    pub fn id(&self) -> &NoteId {
        &self.id
    }

    /// Returns the note's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the note's description, if any.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Returns when the note was created.
    pub fn created(&self) -> DateTime<Utc> {
        self.created
    }

    /// Returns when the note was last modified.
    pub fn modified(&self) -> DateTime<Utc> {
        self.modified
    }

    /// Returns the path to the source file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the content hash of the source file.
    pub fn content_hash(&self) -> &ContentHash {
        &self.content_hash
    }

    /// Returns the note's topics.
    pub fn topics(&self) -> &[Topic] {
        &self.topics
    }

    /// Returns the note's aliases.
    pub fn aliases(&self) -> &[String] {
        &self.aliases
    }

    /// Returns the note's tags.
    pub fn tags(&self) -> &[Tag] {
        &self.tags
    }

    // ===========================================
    // Cycle 3: IndexedNote Builder
    // ===========================================

    /// Creates a builder for constructing an IndexedNote.
    pub fn builder(
        id: NoteId,
        title: impl Into<String>,
        created: DateTime<Utc>,
        modified: DateTime<Utc>,
        path: PathBuf,
        content_hash: ContentHash,
    ) -> IndexedNoteBuilder {
        IndexedNoteBuilder {
            id,
            title: title.into(),
            description: None,
            created,
            modified,
            path,
            content_hash,
            topics: Vec::new(),
            aliases: Vec::new(),
            tags: Vec::new(),
        }
    }
}

/// Builder for constructing an IndexedNote.
pub struct IndexedNoteBuilder {
    id: NoteId,
    title: String,
    description: Option<String>,
    created: DateTime<Utc>,
    modified: DateTime<Utc>,
    path: PathBuf,
    content_hash: ContentHash,
    topics: Vec<Topic>,
    aliases: Vec<String>,
    tags: Vec<Tag>,
}

impl IndexedNoteBuilder {
    /// Sets the description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Sets the topics.
    pub fn topics(mut self, topics: Vec<Topic>) -> Self {
        self.topics = topics;
        self
    }

    /// Sets the aliases.
    pub fn aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases = aliases;
        self
    }

    /// Sets the tags.
    pub fn tags(mut self, tags: Vec<Tag>) -> Self {
        self.tags = tags;
        self
    }

    /// Builds the IndexedNote.
    pub fn build(self) -> IndexedNote {
        IndexedNote {
            id: self.id,
            title: self.title,
            description: self.description,
            created: self.created,
            modified: self.modified,
            path: self.path,
            content_hash: self.content_hash,
            topics: self.topics,
            aliases: self.aliases,
            tags: self.tags,
        }
    }
}

// ===========================================
// Cycle 5: SearchResult Type
// ===========================================

/// A search result with relevance ranking.
#[derive(Debug, Clone)]
pub struct SearchResult {
    note: IndexedNote,
    rank: f64,
    snippet: Option<String>,
}

impl SearchResult {
    /// Creates a new SearchResult without a snippet.
    pub fn new(note: IndexedNote, rank: f64) -> Self {
        Self {
            note,
            rank,
            snippet: None,
        }
    }

    /// Creates a new SearchResult with a snippet.
    pub fn with_snippet(note: IndexedNote, rank: f64, snippet: impl Into<String>) -> Self {
        Self {
            note,
            rank,
            snippet: Some(snippet.into()),
        }
    }

    /// Returns the indexed note.
    pub fn note(&self) -> &IndexedNote {
        &self.note
    }

    /// Returns the relevance rank (higher is more relevant).
    pub fn rank(&self) -> f64 {
        self.rank
    }

    /// Returns the search snippet, if any.
    pub fn snippet(&self) -> Option<&str> {
        self.snippet.as_deref()
    }
}

// ===========================================
// Cycle 6: TopicWithCount Type
// ===========================================

/// A topic with associated note counts.
#[derive(Debug, Clone, PartialEq)]
pub struct TopicWithCount {
    topic: Topic,
    exact_count: u32,
    total_count: u32,
}

impl TopicWithCount {
    /// Creates a new TopicWithCount.
    ///
    /// - `exact_count`: notes with exactly this topic
    /// - `total_count`: notes with this topic or any descendant
    pub fn new(topic: Topic, exact_count: u32, total_count: u32) -> Self {
        Self {
            topic,
            exact_count,
            total_count,
        }
    }

    /// Returns the topic.
    pub fn topic(&self) -> &Topic {
        &self.topic
    }

    /// Returns the count of notes with exactly this topic.
    pub fn exact_count(&self) -> u32 {
        self.exact_count
    }

    /// Returns the count of notes with this topic or any descendant.
    pub fn total_count(&self) -> u32 {
        self.total_count
    }
}

// ===========================================
// Cycle 7: TagWithCount Type
// ===========================================

/// A tag with associated note count.
#[derive(Debug, Clone, PartialEq)]
pub struct TagWithCount {
    tag: Tag,
    count: u32,
}

impl TagWithCount {
    /// Creates a new TagWithCount.
    pub fn new(tag: Tag, count: u32) -> Self {
        Self { tag, count }
    }

    /// Returns the tag.
    pub fn tag(&self) -> &Tag {
        &self.tag
    }

    /// Returns the count of notes with this tag.
    pub fn count(&self) -> u32 {
        self.count
    }
}

// ===========================================
// Cycle 8: IndexRepository Trait
// ===========================================

/// Repository trait for the notes index.
///
/// This trait defines the interface for storing and querying indexed notes.
/// Implementations may use different storage backends (e.g., SQLite, in-memory).
pub trait IndexRepository {
    /// Inserts or updates a note in the index.
    fn upsert_note(
        &mut self,
        note: &Note,
        content_hash: &ContentHash,
        path: &Path,
    ) -> IndexResult<()>;

    /// Removes a note from the index by ID (idempotent).
    fn remove_note(&mut self, id: &NoteId) -> IndexResult<()>;

    /// Retrieves a single note by ID.
    fn get_note(&self, id: &NoteId) -> IndexResult<Option<IndexedNote>>;

    /// Lists notes by topic.
    ///
    /// If `include_descendants` is true, includes notes with child topics.
    fn list_by_topic(
        &self,
        topic: &Topic,
        include_descendants: bool,
    ) -> IndexResult<Vec<IndexedNote>>;

    /// Lists notes with a specific tag.
    fn list_by_tag(&self, tag: &Tag) -> IndexResult<Vec<IndexedNote>>;

    /// Full-text search, returns results ranked by relevance (highest first).
    fn search(&self, query: &str) -> IndexResult<Vec<SearchResult>>;

    /// Returns all topics with note counts.
    fn all_topics(&self) -> IndexResult<Vec<TopicWithCount>>;

    /// Returns all tags with note counts.
    fn all_tags(&self) -> IndexResult<Vec<TagWithCount>>;

    /// Gets content hash for incremental indexing.
    fn get_content_hash(&self, path: &Path) -> IndexResult<Option<ContentHash>>;

    /// Lists all notes in the index.
    fn list_all(&self) -> IndexResult<Vec<IndexedNote>>;

    /// Finds notes whose ID starts with the given prefix.
    ///
    /// Returns all notes with matching ID prefix (case-insensitive).
    /// An empty prefix returns an empty result.
    fn find_by_id_prefix(&self, prefix: &str) -> IndexResult<Vec<IndexedNote>>;

    /// Finds notes with an exact title match (case-insensitive).
    fn find_by_title(&self, title: &str) -> IndexResult<Vec<IndexedNote>>;

    /// Finds notes with a matching alias (case-insensitive).
    fn find_by_alias(&self, alias: &str) -> IndexResult<Vec<IndexedNote>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===========================================
    // Test Helpers
    // ===========================================

    fn test_note_id() -> NoteId {
        "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap()
    }

    fn test_datetime() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn test_content_hash() -> ContentHash {
        ContentHash::compute(b"test content")
    }

    fn sample_indexed_note() -> IndexedNote {
        IndexedNote::builder(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_datetime(),
            PathBuf::from("test.md"),
            test_content_hash(),
        )
        .build()
    }

    // ===========================================
    // Cycle 1: IndexError Type
    // ===========================================

    #[test]
    fn index_error_note_not_found_displays_id() {
        let error = IndexError::NoteNotFound {
            id: "01HQ3K5M7NXJK4QZPW8V2R6T9Y".to_string(),
        };
        let msg = error.to_string();
        assert!(msg.contains("not found"), "should mention 'not found'");
        assert!(
            msg.contains("01HQ3K5M7NXJK4QZPW8V2R6T9Y"),
            "should include the ID"
        );
    }

    #[test]
    fn index_error_invalid_query_displays_reason() {
        let error = IndexError::InvalidQuery("empty search term".to_string());
        let msg = error.to_string();
        assert!(
            msg.contains("invalid query"),
            "should mention 'invalid query'"
        );
        assert!(
            msg.contains("empty search term"),
            "should include the reason"
        );
    }

    #[test]
    fn index_error_implements_std_error() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<IndexError>();
    }

    #[test]
    fn index_error_implements_debug() {
        let error = IndexError::InvalidQuery("test".to_string());
        let debug = format!("{:?}", error);
        assert!(debug.contains("InvalidQuery"));
    }

    // ===========================================
    // Cycle 2: IndexedNote Basic Structure
    // ===========================================

    #[test]
    fn indexed_note_stores_all_required_fields() {
        let id = test_note_id();
        let created = test_datetime();
        let modified = test_datetime();
        let hash = test_content_hash();
        let path = PathBuf::from("notes/test.md");

        let note = IndexedNote::new(
            id.clone(),
            "Test Title",
            Some("A description"),
            created,
            modified,
            path.clone(),
            hash.clone(),
            vec![],
            vec![],
            vec![],
        );

        assert_eq!(note.id(), &id);
        assert_eq!(note.title(), "Test Title");
        assert_eq!(note.description(), Some("A description"));
        assert_eq!(note.created(), created);
        assert_eq!(note.modified(), modified);
        assert_eq!(note.path(), path.as_path());
        assert_eq!(note.content_hash(), &hash);
        assert!(note.topics().is_empty());
        assert!(note.aliases().is_empty());
        assert!(note.tags().is_empty());
    }

    #[test]
    fn indexed_note_description_can_be_none() {
        let note = IndexedNote::new(
            test_note_id(),
            "Title",
            None::<String>,
            test_datetime(),
            test_datetime(),
            PathBuf::from("test.md"),
            test_content_hash(),
            vec![],
            vec![],
            vec![],
        );
        assert_eq!(note.description(), None);
    }

    #[test]
    fn indexed_note_stores_topics() {
        let topics = vec![
            Topic::new("software").unwrap(),
            Topic::new("software/rust").unwrap(),
        ];
        let note = IndexedNote::new(
            test_note_id(),
            "Title",
            None::<String>,
            test_datetime(),
            test_datetime(),
            PathBuf::from("test.md"),
            test_content_hash(),
            topics.clone(),
            vec![],
            vec![],
        );
        assert_eq!(note.topics(), &topics);
    }

    #[test]
    fn indexed_note_stores_tags() {
        let tags = vec![Tag::new("draft").unwrap(), Tag::new("important").unwrap()];
        let note = IndexedNote::new(
            test_note_id(),
            "Title",
            None::<String>,
            test_datetime(),
            test_datetime(),
            PathBuf::from("test.md"),
            test_content_hash(),
            vec![],
            vec![],
            tags.clone(),
        );
        assert_eq!(note.tags(), &tags);
    }

    // ===========================================
    // Cycle 3: IndexedNote Builder
    // ===========================================

    #[test]
    fn indexed_note_builder_with_required_fields_only() {
        let note = IndexedNote::builder(
            test_note_id(),
            "Title",
            test_datetime(),
            test_datetime(),
            PathBuf::from("test.md"),
            test_content_hash(),
        )
        .build();

        assert_eq!(note.title(), "Title");
        assert_eq!(note.description(), None);
        assert!(note.topics().is_empty());
        assert!(note.tags().is_empty());
    }

    #[test]
    fn indexed_note_builder_with_description() {
        let note = IndexedNote::builder(
            test_note_id(),
            "Title",
            test_datetime(),
            test_datetime(),
            PathBuf::from("test.md"),
            test_content_hash(),
        )
        .description("A description")
        .build();

        assert_eq!(note.description(), Some("A description"));
    }

    #[test]
    fn indexed_note_builder_with_topics() {
        let topics = vec![Topic::new("software").unwrap()];
        let note = IndexedNote::builder(
            test_note_id(),
            "Title",
            test_datetime(),
            test_datetime(),
            PathBuf::from("test.md"),
            test_content_hash(),
        )
        .topics(topics.clone())
        .build();

        assert_eq!(note.topics(), &topics);
    }

    #[test]
    fn indexed_note_builder_with_tags() {
        let tags = vec![Tag::new("draft").unwrap()];
        let note = IndexedNote::builder(
            test_note_id(),
            "Title",
            test_datetime(),
            test_datetime(),
            PathBuf::from("test.md"),
            test_content_hash(),
        )
        .tags(tags.clone())
        .build();

        assert_eq!(note.tags(), &tags);
    }

    #[test]
    fn indexed_note_builder_chains_all_optional_fields() {
        let topics = vec![Topic::new("software").unwrap()];
        let tags = vec![Tag::new("draft").unwrap()];

        let note = IndexedNote::builder(
            test_note_id(),
            "Title",
            test_datetime(),
            test_datetime(),
            PathBuf::from("test.md"),
            test_content_hash(),
        )
        .description("Description")
        .topics(topics.clone())
        .tags(tags.clone())
        .build();

        assert_eq!(note.description(), Some("Description"));
        assert_eq!(note.topics(), &topics);
        assert_eq!(note.tags(), &tags);
    }

    // ===========================================
    // Cycle 4: IndexedNote Standard Traits
    // ===========================================

    #[test]
    fn indexed_note_clone_produces_equal_copy() {
        let note = IndexedNote::builder(
            test_note_id(),
            "Title",
            test_datetime(),
            test_datetime(),
            PathBuf::from("test.md"),
            test_content_hash(),
        )
        .description("Desc")
        .build();

        let cloned = note.clone();
        assert_eq!(note, cloned);
    }

    #[test]
    fn indexed_note_equality_compares_all_fields() {
        let note1 = IndexedNote::builder(
            test_note_id(),
            "Title",
            test_datetime(),
            test_datetime(),
            PathBuf::from("test.md"),
            test_content_hash(),
        )
        .build();

        let note2 = IndexedNote::builder(
            test_note_id(),
            "Title",
            test_datetime(),
            test_datetime(),
            PathBuf::from("test.md"),
            test_content_hash(),
        )
        .build();

        assert_eq!(note1, note2);
    }

    #[test]
    fn indexed_note_different_titles_not_equal() {
        let note1 = IndexedNote::builder(
            test_note_id(),
            "Title A",
            test_datetime(),
            test_datetime(),
            PathBuf::from("test.md"),
            test_content_hash(),
        )
        .build();

        let note2 = IndexedNote::builder(
            test_note_id(),
            "Title B",
            test_datetime(),
            test_datetime(),
            PathBuf::from("test.md"),
            test_content_hash(),
        )
        .build();

        assert_ne!(note1, note2);
    }

    #[test]
    fn indexed_note_different_paths_not_equal() {
        let note1 = IndexedNote::builder(
            test_note_id(),
            "Title",
            test_datetime(),
            test_datetime(),
            PathBuf::from("path/a.md"),
            test_content_hash(),
        )
        .build();

        let note2 = IndexedNote::builder(
            test_note_id(),
            "Title",
            test_datetime(),
            test_datetime(),
            PathBuf::from("path/b.md"),
            test_content_hash(),
        )
        .build();

        assert_ne!(note1, note2);
    }

    #[test]
    fn indexed_note_debug_includes_type_and_fields() {
        let note = IndexedNote::builder(
            test_note_id(),
            "My Title",
            test_datetime(),
            test_datetime(),
            PathBuf::from("test.md"),
            test_content_hash(),
        )
        .build();

        let debug = format!("{:?}", note);
        assert!(debug.contains("IndexedNote"));
        assert!(debug.contains("My Title"));
    }

    // ===========================================
    // Cycle 5: SearchResult Type
    // ===========================================

    #[test]
    fn search_result_stores_note_and_rank() {
        let note = sample_indexed_note();
        let result = SearchResult::new(note.clone(), 0.75);

        assert_eq!(result.note().title(), "Test Note");
        assert!((result.rank() - 0.75).abs() < f64::EPSILON);
        assert_eq!(result.snippet(), None);
    }

    #[test]
    fn search_result_with_snippet() {
        let result = SearchResult::with_snippet(
            sample_indexed_note(),
            0.9,
            "...matching <b>text</b> here...",
        );

        assert_eq!(result.snippet(), Some("...matching <b>text</b> here..."));
        assert!((result.rank() - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn search_result_zero_rank() {
        let result = SearchResult::new(sample_indexed_note(), 0.0);
        assert!((result.rank() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn search_result_clone() {
        let result = SearchResult::new(sample_indexed_note(), 0.5);
        let cloned = result.clone();
        assert!((result.rank() - cloned.rank()).abs() < f64::EPSILON);
        assert_eq!(result.note().title(), cloned.note().title());
    }

    #[test]
    fn search_result_debug_includes_rank() {
        let result = SearchResult::new(sample_indexed_note(), 0.5);
        let debug = format!("{:?}", result);
        assert!(debug.contains("SearchResult"));
        assert!(debug.contains("0.5"));
    }

    // ===========================================
    // Cycle 6: TopicWithCount Type
    // ===========================================

    #[test]
    fn topic_with_count_stores_topic_and_counts() {
        let topic = Topic::new("software/architecture").unwrap();
        let twc = TopicWithCount::new(topic.clone(), 5, 12);

        assert_eq!(twc.topic(), &topic);
        assert_eq!(twc.exact_count(), 5);
        assert_eq!(twc.total_count(), 12);
    }

    #[test]
    fn topic_with_count_leaf_has_equal_counts() {
        let topic = Topic::new("software/rust/async").unwrap();
        let twc = TopicWithCount::new(topic, 3, 3);

        assert_eq!(twc.exact_count(), twc.total_count());
    }

    #[test]
    fn topic_with_count_zero_counts() {
        let topic = Topic::new("empty").unwrap();
        let twc = TopicWithCount::new(topic, 0, 0);

        assert_eq!(twc.exact_count(), 0);
        assert_eq!(twc.total_count(), 0);
    }

    #[test]
    fn topic_with_count_clone() {
        let topic = Topic::new("software").unwrap();
        let twc = TopicWithCount::new(topic, 10, 50);
        let cloned = twc.clone();
        assert_eq!(twc, cloned);
    }

    #[test]
    fn topic_with_count_equality() {
        let topic = Topic::new("software").unwrap();
        let twc1 = TopicWithCount::new(topic.clone(), 10, 50);
        let twc2 = TopicWithCount::new(topic, 10, 50);
        assert_eq!(twc1, twc2);
    }

    #[test]
    fn topic_with_count_debug() {
        let topic = Topic::new("software").unwrap();
        let twc = TopicWithCount::new(topic, 10, 50);
        let debug = format!("{:?}", twc);
        assert!(debug.contains("TopicWithCount"));
        assert!(debug.contains("software"));
    }

    // ===========================================
    // Cycle 7: TagWithCount Type
    // ===========================================

    #[test]
    fn tag_with_count_stores_tag_and_count() {
        let tag = Tag::new("draft").unwrap();
        let twc = TagWithCount::new(tag.clone(), 15);

        assert_eq!(twc.tag(), &tag);
        assert_eq!(twc.count(), 15);
    }

    #[test]
    fn tag_with_count_zero_count() {
        let tag = Tag::new("unused").unwrap();
        let twc = TagWithCount::new(tag, 0);
        assert_eq!(twc.count(), 0);
    }

    #[test]
    fn tag_with_count_clone() {
        let tag = Tag::new("important").unwrap();
        let twc = TagWithCount::new(tag, 42);
        let cloned = twc.clone();
        assert_eq!(twc, cloned);
    }

    #[test]
    fn tag_with_count_equality() {
        let tag = Tag::new("draft").unwrap();
        let twc1 = TagWithCount::new(tag.clone(), 5);
        let twc2 = TagWithCount::new(tag, 5);
        assert_eq!(twc1, twc2);
    }

    #[test]
    fn tag_with_count_debug() {
        let tag = Tag::new("draft").unwrap();
        let twc = TagWithCount::new(tag, 5);
        let debug = format!("{:?}", twc);
        assert!(debug.contains("TagWithCount"));
        assert!(debug.contains("draft"));
    }
}
