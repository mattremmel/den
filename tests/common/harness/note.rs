//! Builder for test notes with sensible defaults.

use chrono::{DateTime, Utc};
use den::domain::{Note, NoteId, Tag, Topic};

/// Builder for creating test notes with sensible defaults.
///
/// Automatically generates an ID and timestamps, with a fluent API
/// for setting optional fields.
#[derive(Debug)]
pub struct TestNote {
    id: NoteId,
    title: String,
    created: DateTime<Utc>,
    modified: DateTime<Utc>,
    description: Option<String>,
    topics: Vec<Topic>,
    tags: Vec<Tag>,
    body: String,
}

impl TestNote {
    /// Creates a new test note with the given title.
    ///
    /// Automatically generates a unique ID and sets timestamps to now.
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: NoteId::new(),
            title: title.into(),
            created: now,
            modified: now,
            description: None,
            topics: Vec::new(),
            tags: Vec::new(),
            body: String::new(),
        }
    }

    /// Sets an explicit ID for the note.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into().parse().expect("Invalid NoteId");
        self
    }

    /// Adds a topic to the note.
    pub fn topic(mut self, topic: impl AsRef<str>) -> Self {
        self.topics
            .push(Topic::new(topic.as_ref()).expect("Invalid topic"));
        self
    }

    /// Adds a tag to the note.
    pub fn tag(mut self, tag: impl AsRef<str>) -> Self {
        self.tags.push(Tag::new(tag.as_ref()).expect("Invalid tag"));
        self
    }

    /// Sets the description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Sets the body content (builder method).
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    /// Returns the 10-character ID prefix.
    pub fn id_prefix(&self) -> String {
        self.id.prefix()
    }

    /// Returns the body content.
    pub fn get_body(&self) -> &str {
        &self.body
    }

    /// Returns the title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the ID.
    pub fn note_id(&self) -> &NoteId {
        &self.id
    }

    /// Converts this TestNote to a domain Note.
    pub fn to_note(&self) -> Note {
        Note::builder(self.id.clone(), &self.title, self.created, self.modified)
            .description(self.description.clone())
            .topics(self.topics.clone())
            .tags(self.tags.clone())
            .build()
            .expect("TestNote should always produce valid Note")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===========================================
    // Phase 2: TestNote Builder
    // ===========================================

    #[test]
    fn test_note_new_with_title() {
        let note = TestNote::new("My Test Note");
        assert_eq!(note.title(), "My Test Note");
    }

    #[test]
    fn test_note_generates_id() {
        let note = TestNote::new("Test");
        let id_str = note.note_id().to_string();
        assert_eq!(id_str.len(), 26, "Should generate a valid ULID");
    }

    #[test]
    fn test_note_builder_fluent() {
        let note = TestNote::new("Architecture Decisions")
            .topic("software/architecture")
            .tag("adr")
            .description("Important decisions")
            .body("# ADR-001\n\nWe chose Rust.");

        assert_eq!(note.title(), "Architecture Decisions");
        assert_eq!(note.get_body(), "# ADR-001\n\nWe chose Rust.");

        let domain_note = note.to_note();
        assert_eq!(domain_note.description(), Some("Important decisions"));
        assert_eq!(domain_note.topics().len(), 1);
        assert_eq!(domain_note.topics()[0].to_string(), "software/architecture");
        assert_eq!(domain_note.tags().len(), 1);
        assert_eq!(domain_note.tags()[0].as_str(), "adr");
    }

    #[test]
    fn test_note_custom_id() {
        let note = TestNote::new("Test").id("01HQ3K5M7NXJK4QZPW8V2R6T9Y");
        assert_eq!(note.note_id().to_string(), "01HQ3K5M7NXJK4QZPW8V2R6T9Y");
    }

    #[test]
    fn test_note_id_prefix() {
        let note = TestNote::new("Test").id("01HQ3K5M7NXJK4QZPW8V2R6T9Y");
        assert_eq!(note.id_prefix(), "01HQ3K5M7N");
    }
}
