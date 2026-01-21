//! Frontmatter parser for extracting YAML metadata from markdown files.

use crate::domain::Note;
use thiserror::Error;

/// Result of parsing a markdown file with frontmatter.
#[derive(Debug, Clone)]
pub struct ParsedNote {
    pub note: Note,
    pub body: String,
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
/// # Errors
///
/// Returns `ParseError` if:
/// - The content doesn't start with `---`
/// - There's no closing `---` delimiter
/// - The YAML between delimiters is invalid
/// - Required fields are missing or invalid
pub fn parse(content: &str) -> Result<ParsedNote, ParseError> {
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

    Ok(ParsedNote { note, body })
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
                } else if bytes[after] == b'\n' || (bytes[after] == b'\r' && after + 1 < bytes.len() && bytes[after + 1] == b'\n') {
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
    use pretty_assertions::assert_eq;

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
        assert_eq!(
            result.note.id().to_string(),
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y"
        );
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
        assert_eq!(
            result.note.id().to_string(),
            "01HQ3K5M7NXJK4QZPW8V2R6T9Y"
        );

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
}
