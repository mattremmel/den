//! Frontmatter parser for extracting YAML metadata from markdown files.

use crate::domain::Note;
use crate::infra::ContentHash;
use thiserror::Error;

/// Result of parsing a markdown file with frontmatter.
#[derive(Debug, Clone)]
pub struct ParsedNote {
    pub note: Note,
    pub body: String,
    pub content_hash: ContentHash,
}

/// Errors during frontmatter parsing.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("missing opening frontmatter delimiter '---'")]
    MissingOpeningDelimiter,

    #[error("missing closing frontmatter delimiter '---'")]
    MissingClosingDelimiter,

    #[error("invalid YAML in frontmatter: {0}")]
    InvalidYaml(#[from] serde_yaml::Error),

    #[error("invalid frontmatter: {0}")]
    InvalidFrontmatter(String),
}

/// Parses markdown content with YAML frontmatter.
///
/// # Format
/// ```text
/// ---
/// id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
/// title: Note Title
/// created: 2024-01-15T10:30:00Z
/// modified: 2024-01-15T10:30:00Z
/// ---
/// Body content here...
/// ```
///
/// Note: The content_hash is computed from the string bytes. When reading
/// files from disk, use `read_note()` which computes the hash from raw
/// file bytes (before any BOM stripping or encoding conversion).
///
/// # Errors
///
/// Returns `ParseError` if:
/// - The content doesn't start with `---`
/// - There's no closing `---` delimiter
/// - The YAML between delimiters is invalid
/// - Required fields are missing or invalid
pub fn parse(content: &str) -> Result<ParsedNote, ParseError> {
    let content_hash = ContentHash::compute(content.as_bytes());
    parse_with_hash(content, content_hash)
}

/// Internal: parses content with a pre-computed content hash.
///
/// Used by `read_note()` to provide a hash computed from raw file bytes.
pub(crate) fn parse_with_hash(
    content: &str,
    content_hash: ContentHash,
) -> Result<ParsedNote, ParseError> {
    // Check for opening delimiter - must be at the very start
    if !content.starts_with("---") {
        return Err(ParseError::MissingOpeningDelimiter);
    }

    // Find the end of the opening delimiter line
    let after_opening = if content.starts_with("---\r\n") {
        5 // "---\r\n" is 5 bytes
    } else if content.starts_with("---\n") {
        4 // "---\n" is 4 bytes
    } else if content == "---" {
        return Err(ParseError::MissingClosingDelimiter);
    } else {
        // "---" followed by something other than newline
        return Err(ParseError::MissingOpeningDelimiter);
    };

    // Find the closing delimiter
    let yaml_and_rest = &content[after_opening..];
    let closing_pos = find_closing_delimiter(yaml_and_rest)?;

    let yaml_content = &yaml_and_rest[..closing_pos];

    // Find where the body starts (after closing delimiter line)
    let after_closing = &yaml_and_rest[closing_pos..];
    let body_start = if after_closing.starts_with("---\r\n") {
        closing_pos + 5
    } else if after_closing.starts_with("---\n") {
        closing_pos + 4
    } else if after_closing == "---" {
        closing_pos + 3
    } else {
        // Should not happen if find_closing_delimiter works correctly
        closing_pos + 3
    };

    let body = if after_opening + body_start <= content.len() {
        content[after_opening + body_start..].to_string()
    } else {
        String::new()
    };

    // Parse the YAML
    let note: Note = serde_yaml::from_str(yaml_content)?;

    Ok(ParsedNote {
        note,
        body,
        content_hash,
    })
}

/// Serializes a Note and body to markdown with YAML frontmatter.
///
/// # Format
/// ```text
/// ---
/// id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
/// title: Note Title
/// created: 2024-01-15T10:30:00Z
/// modified: 2024-01-15T10:30:00Z
/// ---
/// Body content here...
/// ```
///
/// The Note's custom Serialize implementation ensures:
/// - Fields are output in the correct order (id, title, created, modified, then optional fields)
/// - Empty optional fields are omitted
/// - Link's `context` field is serialized as `note`
pub fn serialize(note: &Note, body: &str) -> String {
    let yaml = serde_yaml::to_string(note).expect("Note serialization is infallible");
    format!("---\n{}---\n{}", yaml, body)
}

/// Finds the position of the closing `---` delimiter.
///
/// The closing delimiter must:
/// - Appear at the start of a line
/// - Be exactly `---` followed by newline or EOF
fn find_closing_delimiter(content: &str) -> Result<usize, ParseError> {
    let mut pos = 0;
    let bytes = content.as_bytes();

    while pos < bytes.len() {
        // Find next newline or check if we're at EOF
        if pos + 3 <= bytes.len() {
            let potential = &content[pos..pos + 3];
            if potential == "---" {
                // Check what follows the ---
                let after = pos + 3;
                if after >= bytes.len() {
                    // EOF after ---
                    return Ok(pos);
                } else if bytes[after] == b'\n'
                    || (bytes[after] == b'\r'
                        && after + 1 < bytes.len()
                        && bytes[after + 1] == b'\n')
                {
                    // newline after ---
                    return Ok(pos);
                }
            }
        }

        // Move to next line
        match content[pos..].find('\n') {
            Some(newline_offset) => pos += newline_offset + 1,
            None => break, // No more newlines, we're done searching
        }
    }

    Err(ParseError::MissingClosingDelimiter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Link, NoteId, Tag, Topic};
    use chrono::{DateTime, Utc};
    use pretty_assertions::assert_eq;

    // ===========================================
    // Test Helpers
    // ===========================================

    fn test_note_id() -> NoteId {
        "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap()
    }

    fn test_timestamps() -> (DateTime<Utc>, DateTime<Utc>) {
        let created = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let modified = DateTime::parse_from_rfc3339("2024-01-16T14:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        (created, modified)
    }

    fn minimal_note() -> Note {
        let (created, modified) = test_timestamps();
        Note::new(test_note_id(), "Test Note", created, modified).unwrap()
    }

    // ===========================================
    // Phase 1: Basic Happy Path
    // ===========================================

    #[test]
    fn parse_minimal_frontmatter() {
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: API Design
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
"#;

        let result = parse(content).unwrap();
        assert_eq!(result.note.title(), "API Design");
        assert_eq!(result.note.id().to_string(), "01HQ3K5M7NXJK4QZPW8V2R6T9Y");
    }

    #[test]
    fn parse_extracts_body() {
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Test Note
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
This is the body content.

It has multiple paragraphs.
"#;

        let result = parse(content).unwrap();
        assert_eq!(result.note.title(), "Test Note");
        assert!(result.body.contains("This is the body content."));
        assert!(result.body.contains("multiple paragraphs"));
    }

    #[test]
    fn parse_empty_body() {
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: No Body
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---"#;

        let result = parse(content).unwrap();
        assert_eq!(result.note.title(), "No Body");
        assert_eq!(result.body, "");
    }

    // ===========================================
    // Phase 2: Delimiter Validation
    // ===========================================

    #[test]
    fn rejects_missing_opening_delimiter() {
        let content = r#"id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: No Opening
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
Body here
"#;

        let result = parse(content);
        assert!(matches!(result, Err(ParseError::MissingOpeningDelimiter)));
    }

    #[test]
    fn rejects_missing_closing_delimiter() {
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: No Closing
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
Body here without closing
"#;

        let result = parse(content);
        assert!(matches!(result, Err(ParseError::MissingClosingDelimiter)));
    }

    #[test]
    fn rejects_whitespace_before_delimiter() {
        let content = r#" ---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Space Before
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
"#;

        let result = parse(content);
        assert!(matches!(result, Err(ParseError::MissingOpeningDelimiter)));
    }

    #[test]
    fn handles_crlf_line_endings() {
        let content = "---\r\nid: 01HQ3K5M7NXJK4QZPW8V2R6T9Y\r\ntitle: CRLF Note\r\ncreated: 2024-01-15T10:30:00Z\r\nmodified: 2024-01-15T10:30:00Z\r\n---\r\nBody with CRLF\r\n";

        let result = parse(content).unwrap();
        assert_eq!(result.note.title(), "CRLF Note");
        assert!(result.body.contains("Body with CRLF"));
    }

    // ===========================================
    // Phase 3: YAML Validation
    // ===========================================

    #[test]
    fn rejects_invalid_yaml_syntax() {
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Bad YAML
  invalid indentation:
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
"#;

        let result = parse(content);
        assert!(matches!(result, Err(ParseError::InvalidYaml(_))));
    }

    #[test]
    fn rejects_missing_required_fields() {
        // Missing id
        let content = r#"---
title: No ID
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
"#;
        assert!(parse(content).is_err());

        // Missing title
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
"#;
        assert!(parse(content).is_err());

        // Missing created
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: No Created
modified: 2024-01-15T10:30:00Z
---
"#;
        assert!(parse(content).is_err());

        // Missing modified
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: No Modified
created: 2024-01-15T10:30:00Z
---
"#;
        assert!(parse(content).is_err());
    }

    #[test]
    fn rejects_invalid_nested_types() {
        // Invalid tag (empty)
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Invalid Tags
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
tags:
  - ""
---
"#;
        assert!(parse(content).is_err());

        // Invalid topic (empty)
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Invalid Topics
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
topics:
  - ""
---
"#;
        assert!(parse(content).is_err());
    }

    // ===========================================
    // Phase 4: Edge Cases
    // ===========================================

    #[test]
    fn triple_dash_in_body_not_delimiter() {
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Dash In Body
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
This is the body.

--- This line starts with dashes but has text after.

More body content.
"#;

        let result = parse(content).unwrap();
        assert!(result.body.contains("--- This line starts with dashes"));
        assert!(result.body.contains("More body content"));
    }

    #[test]
    fn preserves_body_leading_whitespace() {
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Whitespace Test
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
    Indented line here.
  Two space indent.
"#;

        let result = parse(content).unwrap();
        assert!(result.body.contains("    Indented line here."));
        assert!(result.body.contains("  Two space indent."));
    }

    #[test]
    fn handles_unicode() {
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: 日本語タイトル
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
description: "Descripción en español: café"
---
Body with emoji: 🎉 and unicode: αβγ δεζ
"#;

        let result = parse(content).unwrap();
        assert_eq!(result.note.title(), "日本語タイトル");
        assert_eq!(
            result.note.description(),
            Some("Descripción en español: café")
        );
        assert!(result.body.contains("🎉"));
        assert!(result.body.contains("αβγ"));
    }

    #[test]
    fn body_with_only_newlines() {
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Newlines Only
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---


"#;

        let result = parse(content).unwrap();
        assert_eq!(result.body, "\n\n");
    }

    // ===========================================
    // Phase 5: Full Integration
    // ===========================================

    #[test]
    fn parse_full_frontmatter() {
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: Complete Note
created: 2024-01-15T10:30:00Z
modified: 2024-01-16T14:00:00Z
description: A fully featured note for testing
topics:
  - software/architecture
  - reference
aliases:
  - Full Test
  - Complete Test
tags:
  - draft
  - important
links:
  - id: 01HQ4A2R9PXJK4QZPW8V2R6T9Y
    rel:
      - parent
  - id: 01HQ5B3S0QYJK5RAQX9W3S7T0Z
    rel:
      - see-also
    note: Related discussion
---
# Heading

This is the body of the complete note.

## Section

More content here.
"#;

        let result = parse(content).unwrap();

        // Check required fields
        assert_eq!(result.note.title(), "Complete Note");
        assert_eq!(result.note.id().to_string(), "01HQ3K5M7NXJK4QZPW8V2R6T9Y");

        // Check optional fields
        assert_eq!(
            result.note.description(),
            Some("A fully featured note for testing")
        );

        // Topics
        assert_eq!(result.note.topics().len(), 2);
        assert_eq!(result.note.topics()[0].to_string(), "software/architecture");
        assert_eq!(result.note.topics()[1].to_string(), "reference");

        // Aliases
        assert_eq!(result.note.aliases().len(), 2);
        assert_eq!(result.note.aliases()[0], "Full Test");
        assert_eq!(result.note.aliases()[1], "Complete Test");

        // Tags
        assert_eq!(result.note.tags().len(), 2);
        assert_eq!(result.note.tags()[0].as_str(), "draft");
        assert_eq!(result.note.tags()[1].as_str(), "important");

        // Links
        assert_eq!(result.note.links().len(), 2);
        assert_eq!(result.note.links()[0].rel()[0].as_str(), "parent");
        assert_eq!(result.note.links()[1].rel()[0].as_str(), "see-also");
        assert_eq!(result.note.links()[1].context(), Some("Related discussion"));

        // Body
        assert!(result.body.contains("# Heading"));
        assert!(result.body.contains("body of the complete note"));
        assert!(result.body.contains("## Section"));
    }

    #[test]
    fn parse_design_spec_example() {
        // Example from docs/notes-cli-design.md
        let content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: API Design Notes
description: Notes on REST API design principles
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
topics:
  - software/architecture
  - reference
aliases:
  - REST API Guide
  - API Reference
tags:
  - draft
  - architecture
---
# API Design Notes

This document describes REST API design principles.

## Principles

1. Use nouns for resources
2. Use HTTP methods appropriately
3. Version your API
"#;

        let result = parse(content).unwrap();

        assert_eq!(result.note.title(), "API Design Notes");
        assert_eq!(
            result.note.description(),
            Some("Notes on REST API design principles")
        );
        assert_eq!(result.note.topics().len(), 2);
        assert_eq!(result.note.aliases().len(), 2);
        assert_eq!(result.note.tags().len(), 2);
        assert!(result.body.contains("# API Design Notes"));
        assert!(result.body.contains("REST API design principles"));
        assert!(result.body.contains("## Principles"));
    }

    // ===========================================
    // Phase 1: Serialize - Minimal Happy Path
    // ===========================================

    #[test]
    fn serialize_minimal_note_empty_body() {
        let note = minimal_note();
        let output = serialize(&note, "");

        assert!(output.contains("id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y"));
        assert!(output.contains("title: Test Note"));
        assert!(output.contains("created:"));
        assert!(output.contains("modified:"));
    }

    #[test]
    fn serialize_produces_valid_frontmatter_format() {
        let note = minimal_note();
        let output = serialize(&note, "");

        // Must start with ---
        assert!(output.starts_with("---\n"));
        // Must have closing ---
        let parts: Vec<&str> = output.splitn(3, "---").collect();
        assert_eq!(parts.len(), 3, "Should have opening ---, yaml, closing ---");
        // First part should be empty (before opening ---)
        assert_eq!(parts[0], "");
        // Second part is the YAML content
        assert!(parts[1].contains("id:"));
        // Third part is the body (with leading newline stripped by format)
        assert!(parts[2].starts_with("\n") || parts[2].is_empty());
    }

    // ===========================================
    // Phase 2: Serialize - Body Handling
    // ===========================================

    #[test]
    fn serialize_with_simple_body() {
        let note = minimal_note();
        let output = serialize(&note, "This is the body.");

        assert!(output.ends_with("---\nThis is the body."));
    }

    #[test]
    fn serialize_with_multiline_body() {
        let note = minimal_note();
        let body = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let output = serialize(&note, body);

        assert!(output.contains("---\nFirst paragraph."));
        assert!(output.contains("Second paragraph."));
        assert!(output.contains("Third paragraph."));
    }

    #[test]
    fn serialize_body_preserved_exactly() {
        let note = minimal_note();
        let body = "  Leading spaces\n\n\nMultiple blank lines\n  Trailing spaces  \n";
        let output = serialize(&note, body);

        // Body should be exactly preserved after the closing delimiter
        assert!(output.ends_with(&format!("---\n{}", body)));
    }

    // ===========================================
    // Phase 3: Roundtrip - Minimal
    // ===========================================

    #[test]
    fn roundtrip_minimal_note() {
        let note = minimal_note();
        let serialized = serialize(&note, "");
        let parsed = parse(&serialized).unwrap();

        assert_eq!(parsed.note, note);
    }

    #[test]
    fn roundtrip_minimal_note_with_body() {
        let note = minimal_note();
        let body = "This is the body content.";
        let serialized = serialize(&note, body);
        let parsed = parse(&serialized).unwrap();

        assert_eq!(parsed.note, note);
        assert_eq!(parsed.body, body);
    }

    // ===========================================
    // Phase 4: Roundtrip - Optional Fields
    // ===========================================

    #[test]
    fn roundtrip_with_description() {
        let (created, modified) = test_timestamps();
        let note = Note::builder(test_note_id(), "Test", created, modified)
            .description(Some("A test description"))
            .build()
            .unwrap();

        let serialized = serialize(&note, "");
        let parsed = parse(&serialized).unwrap();

        assert_eq!(parsed.note.description(), Some("A test description"));
        assert_eq!(parsed.note, note);
    }

    #[test]
    fn roundtrip_with_topics() {
        let (created, modified) = test_timestamps();
        let note = Note::builder(test_note_id(), "Test", created, modified)
            .topics(vec![
                Topic::new("software/architecture").unwrap(),
                Topic::new("reference").unwrap(),
            ])
            .build()
            .unwrap();

        let serialized = serialize(&note, "");
        let parsed = parse(&serialized).unwrap();

        assert_eq!(parsed.note.topics().len(), 2);
        assert_eq!(parsed.note, note);
    }

    #[test]
    fn roundtrip_with_aliases() {
        let (created, modified) = test_timestamps();
        let note = Note::builder(test_note_id(), "Test", created, modified)
            .aliases(vec!["Alias One".to_string(), "Alias Two".to_string()])
            .build()
            .unwrap();

        let serialized = serialize(&note, "");
        let parsed = parse(&serialized).unwrap();

        assert_eq!(parsed.note.aliases().len(), 2);
        assert_eq!(parsed.note, note);
    }

    #[test]
    fn roundtrip_with_tags() {
        let (created, modified) = test_timestamps();
        let note = Note::builder(test_note_id(), "Test", created, modified)
            .tags(vec![
                Tag::new("draft").unwrap(),
                Tag::new("important").unwrap(),
            ])
            .build()
            .unwrap();

        let serialized = serialize(&note, "");
        let parsed = parse(&serialized).unwrap();

        assert_eq!(parsed.note.tags().len(), 2);
        assert_eq!(parsed.note, note);
    }

    #[test]
    fn roundtrip_with_links() {
        let (created, modified) = test_timestamps();
        let target: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Y".parse().unwrap();
        let link = Link::with_context(target, vec!["see-also"], "Related discussion").unwrap();

        let note = Note::builder(test_note_id(), "Test", created, modified)
            .links(vec![link])
            .build()
            .unwrap();

        let serialized = serialize(&note, "");
        // Verify the context field is serialized as "note"
        assert!(serialized.contains("note: Related discussion"));

        let parsed = parse(&serialized).unwrap();
        assert_eq!(parsed.note.links().len(), 1);
        assert_eq!(parsed.note.links()[0].context(), Some("Related discussion"));
    }

    // ===========================================
    // Phase 5: Roundtrip - Full Note
    // ===========================================

    #[test]
    fn roundtrip_full_note() {
        let (created, modified) = test_timestamps();
        let target1: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Y".parse().unwrap();
        let target2: NoteId = "01HQ5B3S0QYJK5RAQX9W3S7T0Z".parse().unwrap();

        let note = Note::builder(test_note_id(), "Complete Note", created, modified)
            .description(Some("A fully featured note"))
            .topics(vec![
                Topic::new("software/architecture").unwrap(),
                Topic::new("reference").unwrap(),
            ])
            .aliases(vec!["Full Test".to_string(), "Complete Test".to_string()])
            .tags(vec![
                Tag::new("draft").unwrap(),
                Tag::new("important").unwrap(),
            ])
            .links(vec![
                Link::new(target1, vec!["parent"]).unwrap(),
                Link::with_context(target2, vec!["see-also"], "Related discussion").unwrap(),
            ])
            .build()
            .unwrap();

        let body = "# Complete Note\n\nThis is the body.\n\n## Section\n\nMore content.";
        let serialized = serialize(&note, body);
        let parsed = parse(&serialized).unwrap();

        assert_eq!(parsed.note, note);
        assert_eq!(parsed.body, body);
    }

    #[test]
    fn roundtrip_design_spec_example() {
        let created = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let note = Note::builder(test_note_id(), "API Design Notes", created, created)
            .description(Some("Notes on REST API design principles"))
            .topics(vec![
                Topic::new("software/architecture").unwrap(),
                Topic::new("reference").unwrap(),
            ])
            .aliases(vec![
                "REST API Guide".to_string(),
                "API Reference".to_string(),
            ])
            .tags(vec![
                Tag::new("draft").unwrap(),
                Tag::new("architecture").unwrap(),
            ])
            .build()
            .unwrap();

        let body = r#"# API Design Notes

This document describes REST API design principles.

## Principles

1. Use nouns for resources
2. Use HTTP methods appropriately
3. Version your API
"#;

        let serialized = serialize(&note, body);
        let parsed = parse(&serialized).unwrap();

        assert_eq!(parsed.note.title(), "API Design Notes");
        assert_eq!(
            parsed.note.description(),
            Some("Notes on REST API design principles")
        );
        assert_eq!(parsed.note.topics().len(), 2);
        assert_eq!(parsed.note.aliases().len(), 2);
        assert_eq!(parsed.note.tags().len(), 2);
        assert!(parsed.body.contains("# API Design Notes"));
    }

    // ===========================================
    // Phase 6: Edge Cases - Special Characters
    // ===========================================

    #[test]
    fn roundtrip_title_with_colon() {
        let (created, modified) = test_timestamps();
        let note = Note::new(test_note_id(), "Title: With Colon", created, modified).unwrap();

        let serialized = serialize(&note, "");
        let parsed = parse(&serialized).unwrap();

        assert_eq!(parsed.note.title(), "Title: With Colon");
    }

    #[test]
    fn roundtrip_description_with_quotes() {
        let (created, modified) = test_timestamps();
        let note = Note::builder(test_note_id(), "Test", created, modified)
            .description(Some("Description with \"quotes\" and 'apostrophes'"))
            .build()
            .unwrap();

        let serialized = serialize(&note, "");
        let parsed = parse(&serialized).unwrap();

        assert_eq!(
            parsed.note.description(),
            Some("Description with \"quotes\" and 'apostrophes'")
        );
    }

    #[test]
    fn roundtrip_unicode() {
        let (created, modified) = test_timestamps();
        let note = Note::builder(test_note_id(), "日本語タイトル", created, modified)
            .description(Some("Descripción en español: café"))
            .build()
            .unwrap();

        let body = "Body with emoji: 🎉 and unicode: αβγ δεζ";
        let serialized = serialize(&note, body);
        let parsed = parse(&serialized).unwrap();

        assert_eq!(parsed.note.title(), "日本語タイトル");
        assert_eq!(
            parsed.note.description(),
            Some("Descripción en español: café")
        );
        assert!(parsed.body.contains("🎉"));
        assert!(parsed.body.contains("αβγ"));
    }

    // ===========================================
    // Phase 7: Edge Cases - Body Content
    // ===========================================

    #[test]
    fn roundtrip_body_with_triple_dashes() {
        let note = minimal_note();
        let body = "Before\n\n--- This is not a delimiter\n\nAfter";
        let serialized = serialize(&note, body);
        let parsed = parse(&serialized).unwrap();

        assert!(parsed.body.contains("--- This is not a delimiter"));
        assert!(parsed.body.contains("Before"));
        assert!(parsed.body.contains("After"));
    }

    #[test]
    fn roundtrip_body_only_newlines() {
        let note = minimal_note();
        let body = "\n\n";
        let serialized = serialize(&note, body);
        let parsed = parse(&serialized).unwrap();

        assert_eq!(parsed.body, "\n\n");
    }

    #[test]
    fn roundtrip_body_with_code_blocks() {
        let note = minimal_note();
        let body = r#"# Code Example

```rust
fn main() {
    println!("Hello, world!");
}
```

More text after.
"#;
        let serialized = serialize(&note, body);
        let parsed = parse(&serialized).unwrap();

        assert!(parsed.body.contains("```rust"));
        assert!(parsed.body.contains("println!"));
        assert!(parsed.body.contains("```"));
    }

    // ===========================================
    // Phase 8: Field Order Verification
    // ===========================================

    #[test]
    fn serialize_field_order_matches_spec() {
        let (created, modified) = test_timestamps();
        let target: NoteId = "01HQ4A2R9PXJK4QZPW8V2R6T9Y".parse().unwrap();

        let note = Note::builder(test_note_id(), "Test", created, modified)
            .description(Some("Description"))
            .topics(vec![Topic::new("software").unwrap()])
            .aliases(vec!["Alias".to_string()])
            .tags(vec![Tag::new("tag").unwrap()])
            .links(vec![Link::new(target, vec!["parent"]).unwrap()])
            .build()
            .unwrap();

        let serialized = serialize(&note, "");

        // Verify field order by finding positions
        let id_pos = serialized.find("id:").unwrap();
        let title_pos = serialized.find("title:").unwrap();
        let created_pos = serialized.find("created:").unwrap();
        let modified_pos = serialized.find("modified:").unwrap();
        let desc_pos = serialized.find("description:").unwrap();
        let topics_pos = serialized.find("topics:").unwrap();
        let aliases_pos = serialized.find("aliases:").unwrap();
        let tags_pos = serialized.find("tags:").unwrap();
        let links_pos = serialized.find("links:").unwrap();

        assert!(id_pos < title_pos, "id should come before title");
        assert!(title_pos < created_pos, "title should come before created");
        assert!(
            created_pos < modified_pos,
            "created should come before modified"
        );
        assert!(
            modified_pos < desc_pos,
            "modified should come before description"
        );
        assert!(
            desc_pos < topics_pos,
            "description should come before topics"
        );
        assert!(
            topics_pos < aliases_pos,
            "topics should come before aliases"
        );
        assert!(aliases_pos < tags_pos, "aliases should come before tags");
        assert!(tags_pos < links_pos, "tags should come before links");
    }

    // ===========================================
    // Phase 9: Idempotency
    // ===========================================

    #[test]
    fn double_roundtrip_stable() {
        let (created, modified) = test_timestamps();
        let note = Note::builder(test_note_id(), "Test Note", created, modified)
            .description(Some("A description"))
            .topics(vec![Topic::new("software").unwrap()])
            .build()
            .unwrap();

        let body = "Body content here.";

        // First roundtrip
        let serialized1 = serialize(&note, body);
        let parsed1 = parse(&serialized1).unwrap();

        // Second roundtrip
        let serialized2 = serialize(&parsed1.note, &parsed1.body);

        // Output should be stable
        assert_eq!(
            serialized1, serialized2,
            "Double roundtrip should produce identical output"
        );
    }
}
