//! Note struct representing a markdown note with frontmatter metadata.

use crate::domain::{Link, NoteId, Tag, Topic};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// The kind of error that occurred when constructing a note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseNoteErrorKind {
    EmptyTitle,
}

/// Error returned when constructing an invalid note.
#[derive(Debug, Clone)]
pub struct ParseNoteError {
    kind: ParseNoteErrorKind,
}

impl fmt::Display for ParseNoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ParseNoteErrorKind::EmptyTitle => write!(f, "invalid note: title cannot be empty"),
        }
    }
}

impl std::error::Error for ParseNoteError {}

/// A note with frontmatter metadata.
///
/// Notes are flat markdown files with YAML frontmatter. This struct represents
/// the frontmatter portion, containing the note's identity and organizational metadata.
///
/// # Required Fields
/// - `id`: Unique ULID identifier
/// - `title`: Human-readable title (non-empty)
/// - `created`: When the note was created
/// - `modified`: When the note was last modified
///
/// # Optional Fields
/// - `description`: Brief summary of the note
/// - `topics`: Hierarchical paths for virtual folder organization
/// - `aliases`: Alternative titles for search
/// - `tags`: Flat labels for filtering
/// - `links`: References to other notes with relationship context
///
/// # Examples
///
/// ```
/// use den::domain::{Note, NoteId, Tag, Topic};
/// use chrono::Utc;
///
/// let id = NoteId::new();
/// let now = Utc::now();
/// let note = Note::new(id, "API Design", now, now).unwrap();
/// assert_eq!(note.title(), "API Design");
/// ```
#[derive(Clone, PartialEq)]
pub struct Note {
    id: NoteId,
    title: String,
    created: DateTime<Utc>,
    modified: DateTime<Utc>,
    description: Option<String>,
    topics: Vec<Topic>,
    aliases: Vec<String>,
    tags: Vec<Tag>,
    links: Vec<Link>,
}

impl Note {
    /// Creates a new Note with required fields only.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier
    /// * `title` - Human-readable title (must be non-empty)
    /// * `created` - Creation timestamp
    /// * `modified` - Last modification timestamp
    ///
    /// # Errors
    ///
    /// Returns `ParseNoteError` if:
    /// - The title is empty or whitespace-only
    pub fn new(
        id: NoteId,
        title: impl Into<String>,
        created: DateTime<Utc>,
        modified: DateTime<Utc>,
    ) -> Result<Self, ParseNoteError> {
        let title = title.into();
        let trimmed = title.trim();

        if trimmed.is_empty() {
            return Err(ParseNoteError {
                kind: ParseNoteErrorKind::EmptyTitle,
            });
        }

        Ok(Self {
            id,
            title: trimmed.to_string(),
            created,
            modified,
            description: None,
            topics: Vec::new(),
            aliases: Vec::new(),
            tags: Vec::new(),
            links: Vec::new(),
        })
    }

    /// Creates a builder for constructing a Note with optional fields.
    pub fn builder(
        id: NoteId,
        title: impl Into<String>,
        created: DateTime<Utc>,
        modified: DateTime<Utc>,
    ) -> NoteBuilder {
        NoteBuilder::new(id, title, created, modified)
    }

    /// Returns the note's unique identifier.
    pub fn id(&self) -> &NoteId {
        &self.id
    }

    /// Returns the note's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns when the note was created.
    pub fn created(&self) -> DateTime<Utc> {
        self.created
    }

    /// Returns when the note was last modified.
    pub fn modified(&self) -> DateTime<Utc> {
        self.modified
    }

    /// Returns the note's description, if any.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
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

    /// Returns the note's links.
    pub fn links(&self) -> &[Link] {
        &self.links
    }
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.title, self.id.prefix())
    }
}

impl fmt::Debug for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Note")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("created", &self.created)
            .field("modified", &self.modified)
            .field("description", &self.description)
            .field("topics", &self.topics)
            .field("aliases", &self.aliases)
            .field("tags", &self.tags)
            .field("links", &self.links)
            .finish()
    }
}

/// Builder for constructing a Note with optional fields.
pub struct NoteBuilder {
    id: NoteId,
    title: String,
    created: DateTime<Utc>,
    modified: DateTime<Utc>,
    description: Option<String>,
    topics: Vec<Topic>,
    aliases: Vec<String>,
    tags: Vec<Tag>,
    links: Vec<Link>,
}

impl NoteBuilder {
    fn new(
        id: NoteId,
        title: impl Into<String>,
        created: DateTime<Utc>,
        modified: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            created,
            modified,
            description: None,
            topics: Vec::new(),
            aliases: Vec::new(),
            tags: Vec::new(),
            links: Vec::new(),
        }
    }

    /// Sets the note's description.
    ///
    /// Empty or whitespace-only strings are normalized to None.
    pub fn description(mut self, description: Option<impl Into<String>>) -> Self {
        self.description = description
            .map(|s| s.into())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        self
    }

    /// Sets the note's topics.
    ///
    /// Duplicates are removed (first occurrence kept).
    pub fn topics(mut self, topics: Vec<Topic>) -> Self {
        self.topics = deduplicate_topics(topics);
        self
    }

    /// Sets the note's aliases.
    ///
    /// Duplicates are removed case-insensitively (first occurrence kept).
    /// Empty or whitespace-only aliases are filtered out.
    pub fn aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases = deduplicate_aliases(aliases);
        self
    }

    /// Sets the note's tags.
    ///
    /// Duplicates are removed (first occurrence kept).
    pub fn tags(mut self, tags: Vec<Tag>) -> Self {
        self.tags = deduplicate_tags(tags);
        self
    }

    /// Sets the note's links.
    pub fn links(mut self, links: Vec<Link>) -> Self {
        self.links = links;
        self
    }

    /// Builds the Note.
    ///
    /// # Errors
    ///
    /// Returns `ParseNoteError` if:
    /// - The title is empty or whitespace-only
    pub fn build(self) -> Result<Note, ParseNoteError> {
        let trimmed = self.title.trim();

        if trimmed.is_empty() {
            return Err(ParseNoteError {
                kind: ParseNoteErrorKind::EmptyTitle,
            });
        }

        Ok(Note {
            id: self.id,
            title: trimmed.to_string(),
            created: self.created,
            modified: self.modified,
            description: self.description,
            topics: self.topics,
            aliases: self.aliases,
            tags: self.tags,
            links: self.links,
        })
    }
}

/// Removes duplicate topics (by equality).
fn deduplicate_topics(topics: Vec<Topic>) -> Vec<Topic> {
    let mut seen = Vec::new();
    for topic in topics {
        if !seen.contains(&topic) {
            seen.push(topic);
        }
    }
    seen
}

/// Removes duplicate aliases (case-insensitive).
/// Also filters out empty/whitespace-only aliases.
fn deduplicate_aliases(aliases: Vec<String>) -> Vec<String> {
    let mut seen_lower = Vec::new();
    let mut result = Vec::new();
    for alias in aliases {
        let trimmed = alias.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_lowercase();
        if !seen_lower.contains(&lower) {
            seen_lower.push(lower);
            result.push(trimmed);
        }
    }
    result
}

/// Removes duplicate tags (by equality, which is case-insensitive for Tag).
fn deduplicate_tags(tags: Vec<Tag>) -> Vec<Tag> {
    let mut seen = Vec::new();
    for tag in tags {
        if !seen.contains(&tag) {
            seen.push(tag);
        }
    }
    seen
}

impl Serialize for Note {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", &self.id)?;
        map.serialize_entry("title", &self.title)?;
        map.serialize_entry("created", &self.created)?;
        map.serialize_entry("modified", &self.modified)?;

        if let Some(ref desc) = self.description {
            map.serialize_entry("description", desc)?;
        }
        if !self.topics.is_empty() {
            map.serialize_entry("topics", &self.topics)?;
        }
        if !self.aliases.is_empty() {
            map.serialize_entry("aliases", &self.aliases)?;
        }
        if !self.tags.is_empty() {
            map.serialize_entry("tags", &self.tags)?;
        }
        if !self.links.is_empty() {
            map.serialize_entry("links", &self.links)?;
        }

        map.end()
    }
}

impl<'de> Deserialize<'de> for Note {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct NoteHelper {
            id: NoteId,
            title: String,
            created: DateTime<Utc>,
            modified: DateTime<Utc>,
            #[serde(default)]
            description: Option<String>,
            #[serde(default)]
            topics: Vec<Topic>,
            #[serde(default)]
            aliases: Vec<String>,
            #[serde(default)]
            tags: Vec<Tag>,
            #[serde(default)]
            links: Vec<Link>,
        }

        let helper = NoteHelper::deserialize(deserializer)?;

        Note::builder(helper.id, helper.title, helper.created, helper.modified)
            .description(helper.description)
            .topics(helper.topics)
            .aliases(helper.aliases)
            .tags(helper.tags)
            .links(helper.links)
            .build()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn test_note_id() -> NoteId {
        "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap()
    }

    fn test_datetime() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn test_modified_datetime() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-16T14:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    // ===========================================
    // Phase 1: Basic Structure & Required Fields
    // ===========================================

    #[test]
    fn new_with_required_fields() {
        let id = test_note_id();
        let created = test_datetime();
        let modified = test_modified_datetime();

        let note = Note::new(id.clone(), "API Design", created, modified).unwrap();

        assert_eq!(note.id(), &id);
        assert_eq!(note.title(), "API Design");
        assert_eq!(note.created(), created);
        assert_eq!(note.modified(), modified);
    }

    #[test]
    fn accessors_return_correct_values() {
        let note = Note::new(
            test_note_id(),
            "Test Note",
            test_datetime(),
            test_modified_datetime(),
        )
        .unwrap();

        // Required fields
        assert_eq!(note.title(), "Test Note");
        assert_eq!(note.created(), test_datetime());
        assert_eq!(note.modified(), test_modified_datetime());

        // Optional fields default to empty/None
        assert_eq!(note.description(), None);
        assert!(note.topics().is_empty());
        assert!(note.aliases().is_empty());
        assert!(note.tags().is_empty());
        assert!(note.links().is_empty());
    }

    #[test]
    fn title_cannot_be_empty() {
        let result = Note::new(
            test_note_id(),
            "",
            test_datetime(),
            test_modified_datetime(),
        );
        assert!(result.is_err());

        let result = Note::new(
            test_note_id(),
            "   ",
            test_datetime(),
            test_modified_datetime(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn created_before_modified_is_valid() {
        let created = test_datetime();
        let modified = test_modified_datetime();
        assert!(created <= modified);

        let note = Note::new(test_note_id(), "Test", created, modified).unwrap();
        assert_eq!(note.created(), created);
        assert_eq!(note.modified(), modified);
    }

    // ===========================================
    // Phase 2: Builder Pattern for Optional Fields
    // ===========================================

    #[test]
    fn builder_sets_description() {
        let note = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .description(Some("A test description"))
        .build()
        .unwrap();

        assert_eq!(note.description(), Some("A test description"));
    }

    #[test]
    fn builder_sets_topics() {
        let topics = vec![
            Topic::new("software/api").unwrap(),
            Topic::new("reference").unwrap(),
        ];

        let note = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .topics(topics.clone())
        .build()
        .unwrap();

        assert_eq!(note.topics().len(), 2);
        assert_eq!(note.topics()[0].to_string(), "software/api");
    }

    #[test]
    fn builder_sets_aliases() {
        let aliases = vec!["REST API".to_string(), "API Guide".to_string()];

        let note = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .aliases(aliases)
        .build()
        .unwrap();

        assert_eq!(note.aliases().len(), 2);
        assert_eq!(note.aliases()[0], "REST API");
    }

    #[test]
    fn builder_sets_tags() {
        let tags = vec![Tag::new("draft").unwrap(), Tag::new("review").unwrap()];

        let note = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .tags(tags)
        .build()
        .unwrap();

        assert_eq!(note.tags().len(), 2);
        assert_eq!(note.tags()[0].as_str(), "draft");
    }

    #[test]
    fn builder_sets_links() {
        let target: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Y".parse().unwrap();
        let links = vec![Link::new(target, vec!["parent"]).unwrap()];

        let note = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .links(links)
        .build()
        .unwrap();

        assert_eq!(note.links().len(), 1);
        assert_eq!(note.links()[0].rel()[0].as_str(), "parent");
    }

    #[test]
    fn builder_chains_all_fields() {
        let target: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Y".parse().unwrap();

        let note = Note::builder(
            test_note_id(),
            "Full Note",
            test_datetime(),
            test_modified_datetime(),
        )
        .description(Some("Complete description"))
        .topics(vec![Topic::new("software").unwrap()])
        .aliases(vec!["Alias One".to_string()])
        .tags(vec![Tag::new("important").unwrap()])
        .links(vec![Link::new(target, vec!["see-also"]).unwrap()])
        .build()
        .unwrap();

        assert_eq!(note.title(), "Full Note");
        assert_eq!(note.description(), Some("Complete description"));
        assert_eq!(note.topics().len(), 1);
        assert_eq!(note.aliases().len(), 1);
        assert_eq!(note.tags().len(), 1);
        assert_eq!(note.links().len(), 1);
    }

    // ===========================================
    // Phase 3: Validation Rules
    // ===========================================

    #[test]
    fn aliases_are_deduplicated() {
        let aliases = vec![
            "REST API".to_string(),
            "rest api".to_string(), // case-insensitive duplicate
            "API Guide".to_string(),
        ];

        let note = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .aliases(aliases)
        .build()
        .unwrap();

        assert_eq!(note.aliases().len(), 2);
        assert_eq!(note.aliases()[0], "REST API"); // first occurrence kept
        assert_eq!(note.aliases()[1], "API Guide");
    }

    #[test]
    fn topics_are_deduplicated() {
        let topics = vec![
            Topic::new("software/api").unwrap(),
            Topic::new("software/api").unwrap(), // duplicate
            Topic::new("reference").unwrap(),
        ];

        let note = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .topics(topics)
        .build()
        .unwrap();

        assert_eq!(note.topics().len(), 2);
    }

    #[test]
    fn tags_are_deduplicated() {
        let tags = vec![
            Tag::new("Draft").unwrap(),
            Tag::new("draft").unwrap(), // case-insensitive duplicate
            Tag::new("review").unwrap(),
        ];

        let note = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .tags(tags)
        .build()
        .unwrap();

        assert_eq!(note.tags().len(), 2);
    }

    #[test]
    fn empty_description_normalized_to_none() {
        let note = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .description(Some("   "))
        .build()
        .unwrap();

        assert_eq!(note.description(), None);
    }

    #[test]
    fn id_timestamp_matches_created() {
        // Create a note where the ID was generated at the same time as created
        let created = test_datetime();
        let id = NoteId::from_datetime(created);
        let note = Note::new(id.clone(), "Test", created, test_modified_datetime()).unwrap();

        // The ULID timestamp should approximately match created
        let id_timestamp = note.id().timestamp();
        let diff = (id_timestamp - created).num_milliseconds().abs();
        assert!(diff < 1000, "ID timestamp should be close to created time");
    }

    // ===========================================
    // Phase 4: Equality & Hashing
    // ===========================================

    #[test]
    fn equality_compares_all_fields() {
        let note1 = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .description(Some("Description"))
        .build()
        .unwrap();

        let note2 = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .description(Some("Description"))
        .build()
        .unwrap();

        assert_eq!(note1, note2);
    }

    #[test]
    fn equality_fails_on_different_id() {
        let note1 = Note::new(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .unwrap();

        let note2 = Note::new(
            NoteId::new(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .unwrap();

        assert_ne!(note1, note2);
    }

    #[test]
    fn equality_fails_on_different_optional_fields() {
        let note1 = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .description(Some("Description A"))
        .build()
        .unwrap();

        let note2 = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .description(Some("Description B"))
        .build()
        .unwrap();

        assert_ne!(note1, note2);
    }

    #[test]
    fn clone_produces_equal_note() {
        let note = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .description(Some("A description"))
        .topics(vec![Topic::new("software").unwrap()])
        .build()
        .unwrap();

        let cloned = note.clone();
        assert_eq!(note, cloned);
    }

    // ===========================================
    // Phase 5: Display & Debug
    // ===========================================

    #[test]
    fn display_shows_title_and_id_prefix() {
        let note = Note::new(
            test_note_id(),
            "API Design",
            test_datetime(),
            test_modified_datetime(),
        )
        .unwrap();

        let display = format!("{}", note);
        assert_eq!(display, "API Design [01HQ3K5M7N]");
    }

    #[test]
    fn debug_shows_full_structure() {
        let note = Note::new(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .unwrap();

        let debug = format!("{:?}", note);
        assert!(debug.contains("Note"));
        assert!(debug.contains("id"));
        assert!(debug.contains("title"));
        assert!(debug.contains("created"));
        assert!(debug.contains("modified"));
        assert!(debug.contains("description"));
        assert!(debug.contains("topics"));
        assert!(debug.contains("aliases"));
        assert!(debug.contains("tags"));
        assert!(debug.contains("links"));
    }

    // ===========================================
    // Phase 6: Serde Roundtrip
    // ===========================================

    #[test]
    fn serde_roundtrip_minimal() {
        let note = Note::new(
            test_note_id(),
            "Minimal Note",
            test_datetime(),
            test_modified_datetime(),
        )
        .unwrap();

        let yaml = serde_yaml::to_string(&note).unwrap();
        let parsed: Note = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(note, parsed);
    }

    #[test]
    fn serde_roundtrip_full() {
        let target: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Y".parse().unwrap();

        let note = Note::builder(
            test_note_id(),
            "Full Note",
            test_datetime(),
            test_modified_datetime(),
        )
        .description(Some("A complete note"))
        .topics(vec![Topic::new("software/api").unwrap()])
        .aliases(vec!["REST Guide".to_string()])
        .tags(vec![Tag::new("reference").unwrap()])
        .links(vec![Link::new(target, vec!["parent"]).unwrap()])
        .build()
        .unwrap();

        let yaml = serde_yaml::to_string(&note).unwrap();
        let parsed: Note = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(note, parsed);
    }

    #[test]
    fn serde_deserialize_missing_optional_fields() {
        let yaml = r#"
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Sparse Note
created: 2024-01-15T10:30:00Z
modified: 2024-01-16T14:00:00Z
"#;
        let note: Note = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(note.title(), "Sparse Note");
        assert_eq!(note.description(), None);
        assert!(note.topics().is_empty());
        assert!(note.aliases().is_empty());
        assert!(note.tags().is_empty());
        assert!(note.links().is_empty());
    }

    #[test]
    fn serde_rejects_missing_id() {
        let yaml = r#"
title: No ID
created: 2024-01-15T10:30:00Z
modified: 2024-01-16T14:00:00Z
"#;
        let result: Result<Note, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn serde_rejects_missing_title() {
        let yaml = r#"
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
created: 2024-01-15T10:30:00Z
modified: 2024-01-16T14:00:00Z
"#;
        let result: Result<Note, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn serde_rejects_missing_created() {
        let yaml = r#"
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: No Created
modified: 2024-01-16T14:00:00Z
"#;
        let result: Result<Note, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn serde_rejects_missing_modified() {
        let yaml = r#"
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: No Modified
created: 2024-01-15T10:30:00Z
"#;
        let result: Result<Note, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn serde_validates_nested_types() {
        // Invalid tag (empty)
        let yaml = r#"
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Invalid Tags
created: 2024-01-15T10:30:00Z
modified: 2024-01-16T14:00:00Z
tags:
  - ""
"#;
        let result: Result<Note, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    // ===========================================
    // Phase 7: Frontmatter Integration
    // ===========================================

    #[test]
    fn serialized_yaml_matches_expected_format() {
        let note = Note::builder(
            test_note_id(),
            "API Design",
            test_datetime(),
            test_modified_datetime(),
        )
        .description(Some("REST API design notes"))
        .build()
        .unwrap();

        let yaml = serde_yaml::to_string(&note).unwrap();

        // Check field presence
        assert!(yaml.contains("id:"));
        assert!(yaml.contains("title:"));
        assert!(yaml.contains("created:"));
        assert!(yaml.contains("modified:"));
        assert!(yaml.contains("description:"));
    }

    #[test]
    fn deserialize_from_design_spec_example() {
        // Example from docs/notes-cli-design.md
        let yaml = r#"
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: API Design Notes
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
description: Notes on REST API design principles
topics:
  - software/architecture
  - reference
aliases:
  - REST API Guide
  - API Reference
tags:
  - draft
  - architecture
"#;
        let note: Note = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(note.title(), "API Design Notes");
        assert_eq!(
            note.description(),
            Some("Notes on REST API design principles")
        );
        assert_eq!(note.topics().len(), 2);
        assert_eq!(note.aliases().len(), 2);
        assert_eq!(note.tags().len(), 2);
    }

    #[test]
    fn timestamps_serialize_as_iso8601() {
        let note = Note::new(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .unwrap();

        let yaml = serde_yaml::to_string(&note).unwrap();
        assert!(yaml.contains("2024-01-15T10:30:00Z"));
        assert!(yaml.contains("2024-01-16T14:00:00Z"));
    }

    #[test]
    fn optional_fields_omitted_when_empty() {
        let note = Note::new(
            test_note_id(),
            "Minimal",
            test_datetime(),
            test_modified_datetime(),
        )
        .unwrap();

        let yaml = serde_yaml::to_string(&note).unwrap();

        // These should NOT appear when empty
        assert!(!yaml.contains("description:"));
        assert!(!yaml.contains("topics:"));
        assert!(!yaml.contains("aliases:"));
        assert!(!yaml.contains("tags:"));
        assert!(!yaml.contains("links:"));
    }

    // ===========================================
    // Phase 8: Edge Cases & Spec Compliance
    // ===========================================

    #[test]
    fn serde_roundtrip_link_with_context() {
        let target: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Y".parse().unwrap();
        let link =
            Link::with_context(target, vec!["see-also"], "Related discussion from 2024").unwrap();

        let note = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .links(vec![link])
        .build()
        .unwrap();

        let yaml = serde_yaml::to_string(&note).unwrap();
        assert!(
            yaml.contains("note:"),
            "context should serialize as 'note' field"
        );

        let parsed: Note = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(note, parsed);
        assert_eq!(
            parsed.links()[0].context(),
            Some("Related discussion from 2024")
        );
    }

    #[test]
    fn serde_roundtrip_link_with_multiple_rels() {
        let target: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Y".parse().unwrap();
        let link = Link::new(target, vec!["manager-of", "mentor-to"]).unwrap();

        let note = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .links(vec![link])
        .build()
        .unwrap();

        let yaml = serde_yaml::to_string(&note).unwrap();
        let parsed: Note = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed.links()[0].rel().len(), 2);
        assert_eq!(parsed.links()[0].rel()[0].as_str(), "manager-of");
        assert_eq!(parsed.links()[0].rel()[1].as_str(), "mentor-to");
    }

    #[test]
    fn deserialize_from_design_spec_example_with_links() {
        let yaml = r#"
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: API Design Notes
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
description: Notes on REST API design principles
topics:
  - software/architecture
  - reference
aliases:
  - REST API Guide
tags:
  - draft
links:
  - id: 01HQ4A2R9PXJK4QZPW8V2R6T9Y
    rel:
      - parent
  - id: 01HQ5B3S0QYJK5RAQX9W3S7T0Z
    rel:
      - see-also
      - related
    note: Alternative approach
"#;
        let note: Note = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(note.links().len(), 2);
        assert_eq!(note.links()[0].rel().len(), 1);
        assert_eq!(note.links()[1].rel().len(), 2);
        assert_eq!(note.links()[1].context(), Some("Alternative approach"));
    }

    #[test]
    fn builder_allows_duplicate_links_to_same_target() {
        let target: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Y".parse().unwrap();
        let link1 = Link::new(target.clone(), vec!["parent"]).unwrap();
        let link2 = Link::new(target, vec!["source"]).unwrap();

        let note = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .links(vec![link1, link2])
        .build()
        .unwrap();

        // Both links preserved (not deduplicated like topics/tags)
        assert_eq!(note.links().len(), 2);
    }

    #[test]
    fn aliases_filter_out_empty_strings() {
        let aliases = vec![
            "Valid Alias".to_string(),
            "".to_string(),
            "   ".to_string(),
            "Another Valid".to_string(),
        ];

        let note = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .aliases(aliases)
        .build()
        .unwrap();

        assert_eq!(note.aliases().len(), 2);
        assert_eq!(note.aliases()[0], "Valid Alias");
        assert_eq!(note.aliases()[1], "Another Valid");
    }

    #[test]
    fn created_equals_modified_is_valid() {
        let timestamp = test_datetime();
        let note = Note::new(test_note_id(), "New Note", timestamp, timestamp).unwrap();

        assert_eq!(note.created(), note.modified());
    }

    #[test]
    fn title_whitespace_is_trimmed() {
        let note = Note::new(
            test_note_id(),
            "  API Design  ",
            test_datetime(),
            test_modified_datetime(),
        )
        .unwrap();

        assert_eq!(note.title(), "API Design");
    }

    #[test]
    fn description_whitespace_is_trimmed() {
        let note = Note::builder(
            test_note_id(),
            "Test",
            test_datetime(),
            test_modified_datetime(),
        )
        .description(Some("  A description with spaces  "))
        .build()
        .unwrap();

        assert_eq!(note.description(), Some("A description with spaces"));
    }

    // ===========================================
    // Phase 9: Structured Error Context
    // ===========================================

    #[test]
    fn note_parse_error_shows_empty_title() {
        let err = Note::new(test_note_id(), "", test_datetime(), test_datetime()).unwrap_err();
        assert!(err.to_string().contains("title"));
        assert!(err.to_string().contains("cannot be empty"));
    }
}
