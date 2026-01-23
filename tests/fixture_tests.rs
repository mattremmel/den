//! Integration tests using fixture files.
//!
//! These tests verify that the frontmatter parser correctly handles
//! various valid and invalid markdown note formats.

mod common;

use common::{fixtures_dir, invalid_fixture, read_fixture, valid_fixture};
use den::infra::{ParseError, parse};
use pretty_assertions::assert_eq;

// ===========================================
// Phase 1: Infrastructure Tests
// ===========================================

#[test]
fn fixtures_directory_exists() {
    let dir = fixtures_dir();
    assert!(dir.exists(), "fixtures directory should exist: {:?}", dir);
}

#[test]
fn valid_fixtures_directory_exists() {
    let dir = fixtures_dir().join("valid");
    assert!(
        dir.exists(),
        "valid fixtures directory should exist: {:?}",
        dir
    );
}

#[test]
fn invalid_fixtures_directory_exists() {
    let dir = fixtures_dir().join("invalid");
    assert!(
        dir.exists(),
        "invalid fixtures directory should exist: {:?}",
        dir
    );
}

// ===========================================
// Phase 2: Valid Fixture Tests
// ===========================================

#[test]
fn parse_minimal_fixture() {
    let path = valid_fixture("minimal.md");
    let content = read_fixture(&path);
    let result = parse(&content).expect("minimal fixture should parse");

    assert_eq!(result.note.title(), "Minimal Note");
    assert_eq!(result.note.id().to_string(), "01HQ3K5M7NXJK4QZPW8V2R6T9Y");
    assert_eq!(result.note.description(), None);
    assert!(result.note.topics().is_empty());
    assert!(result.note.aliases().is_empty());
    assert!(result.note.tags().is_empty());
    assert!(result.note.links().is_empty());
}

#[test]
fn parse_full_fixture() {
    let path = valid_fixture("full.md");
    let content = read_fixture(&path);
    let result = parse(&content).expect("full fixture should parse");

    // Required fields
    assert_eq!(result.note.title(), "Complete Note");
    assert_eq!(result.note.id().to_string(), "01HQ3K5M7NXJK4QZPW8V2R6T9Y");

    // Optional fields
    assert_eq!(
        result.note.description(),
        Some("A fully featured note for testing all optional fields")
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
    assert!(result.body.contains("# Complete Note"));
    assert!(result.body.contains("all optional fields populated"));
}

#[test]
fn parse_unicode_title_fixture() {
    let path = valid_fixture("unicode_title.md");
    let content = read_fixture(&path);
    let result = parse(&content).expect("unicode fixture should parse");

    assert_eq!(result.note.title(), "日本語タイトル");
    assert_eq!(
        result.note.description(),
        Some("Descripción en español: café")
    );
    assert!(result.body.contains("🎉"));
    assert!(result.body.contains("αβγ"));
    assert!(result.body.contains("中文内容"));
}

#[test]
fn parse_empty_body_fixture() {
    let path = valid_fixture("empty_body.md");
    let content = read_fixture(&path);
    let result = parse(&content).expect("empty body fixture should parse");

    assert_eq!(result.note.title(), "Empty Body Note");
    assert_eq!(result.body, "");
}

#[test]
fn parse_crlf_line_endings_fixture() {
    let path = valid_fixture("crlf_line_endings.md");
    let content = read_fixture(&path);
    let result = parse(&content).expect("CRLF fixture should parse");

    assert_eq!(result.note.title(), "CRLF Note");
    assert!(result.body.contains("Body with CRLF"));
}

#[test]
fn parse_with_links_fixture() {
    let path = valid_fixture("with_links.md");
    let content = read_fixture(&path);
    let result = parse(&content).expect("with_links fixture should parse");

    assert_eq!(result.note.title(), "Note With Links");
    assert_eq!(result.note.links().len(), 3);

    // First link: single rel, no context
    assert_eq!(result.note.links()[0].rel().len(), 1);
    assert_eq!(result.note.links()[0].rel()[0].as_str(), "parent");
    assert_eq!(result.note.links()[0].context(), None);

    // Second link: multiple rels, with context
    assert_eq!(result.note.links()[1].rel().len(), 2);
    assert_eq!(result.note.links()[1].rel()[0].as_str(), "see-also");
    assert_eq!(result.note.links()[1].rel()[1].as_str(), "related");
    assert_eq!(
        result.note.links()[1].context(),
        Some("Alternative approach discussed in meeting")
    );

    // Third link: single rel, with context
    assert_eq!(result.note.links()[2].rel()[0].as_str(), "manager-of");
    assert_eq!(
        result.note.links()[2].context(),
        Some("Hired me at Acme Corp, 2019")
    );
}

#[test]
fn parse_body_with_dashes_fixture() {
    let path = valid_fixture("body_with_dashes.md");
    let content = read_fixture(&path);
    let result = parse(&content).expect("body_with_dashes fixture should parse");

    assert_eq!(result.note.title(), "Body With Dashes");

    // Triple dashes followed by text should not be treated as delimiter
    assert!(
        result
            .body
            .contains("--- This line starts with three dashes")
    );

    // Horizontal rule (standalone ---) in body should be preserved
    assert!(
        result
            .body
            .contains("More content after the horizontal rule")
    );

    // Four dashes should be fine
    assert!(result.body.contains("---- Four dashes"));

    // Dashes followed immediately by text
    assert!(result.body.contains("---end"));
}

// ===========================================
// Phase 3: Invalid Fixture Tests - Delimiter Errors
// ===========================================

#[test]
fn parse_missing_opening_delimiter_fixture() {
    let path = invalid_fixture("missing_opening_delimiter.md");
    let content = read_fixture(&path);
    let result = parse(&content);

    assert!(
        matches!(result, Err(ParseError::MissingOpeningDelimiter)),
        "expected MissingOpeningDelimiter, got {:?}",
        result
    );
}

#[test]
fn parse_missing_closing_delimiter_fixture() {
    let path = invalid_fixture("missing_closing_delimiter.md");
    let content = read_fixture(&path);
    let result = parse(&content);

    assert!(
        matches!(result, Err(ParseError::MissingClosingDelimiter)),
        "expected MissingClosingDelimiter, got {:?}",
        result
    );
}

#[test]
fn parse_whitespace_before_delimiter_fixture() {
    let path = invalid_fixture("whitespace_before_delimiter.md");
    let content = read_fixture(&path);
    let result = parse(&content);

    assert!(
        matches!(result, Err(ParseError::MissingOpeningDelimiter)),
        "expected MissingOpeningDelimiter, got {:?}",
        result
    );
}

// ===========================================
// Phase 3: Invalid Fixture Tests - Missing Required Fields
// ===========================================

#[test]
fn parse_missing_id_fixture() {
    let path = invalid_fixture("missing_id.md");
    let content = read_fixture(&path);
    let result = parse(&content);

    assert!(
        matches!(result, Err(ParseError::InvalidYaml(_))),
        "expected InvalidYaml, got {:?}",
        result
    );
}

#[test]
fn parse_missing_title_fixture() {
    let path = invalid_fixture("missing_title.md");
    let content = read_fixture(&path);
    let result = parse(&content);

    assert!(
        matches!(result, Err(ParseError::InvalidYaml(_))),
        "expected InvalidYaml, got {:?}",
        result
    );
}

#[test]
fn parse_missing_created_fixture() {
    let path = invalid_fixture("missing_created.md");
    let content = read_fixture(&path);
    let result = parse(&content);

    assert!(
        matches!(result, Err(ParseError::InvalidYaml(_))),
        "expected InvalidYaml, got {:?}",
        result
    );
}

#[test]
fn parse_missing_modified_fixture() {
    let path = invalid_fixture("missing_modified.md");
    let content = read_fixture(&path);
    let result = parse(&content);

    assert!(
        matches!(result, Err(ParseError::InvalidYaml(_))),
        "expected InvalidYaml, got {:?}",
        result
    );
}

// ===========================================
// Phase 3: Invalid Fixture Tests - Invalid Values
// ===========================================

#[test]
fn parse_empty_title_fixture() {
    let path = invalid_fixture("empty_title.md");
    let content = read_fixture(&path);
    let result = parse(&content);

    // Empty title is validated by the Note deserializer, which produces InvalidYaml
    assert!(
        matches!(result, Err(ParseError::InvalidYaml(_))),
        "expected InvalidYaml for empty title, got {:?}",
        result
    );
}

#[test]
fn parse_invalid_yaml_syntax_fixture() {
    let path = invalid_fixture("invalid_yaml_syntax.md");
    let content = read_fixture(&path);
    let result = parse(&content);

    assert!(
        matches!(result, Err(ParseError::InvalidYaml(_))),
        "expected InvalidYaml, got {:?}",
        result
    );
}

#[test]
fn parse_invalid_ulid_fixture() {
    let path = invalid_fixture("invalid_ulid.md");
    let content = read_fixture(&path);
    let result = parse(&content);

    assert!(
        matches!(result, Err(ParseError::InvalidYaml(_))),
        "expected InvalidYaml for invalid ULID, got {:?}",
        result
    );
}

// ===========================================
// Phase 3: Invalid Fixture Tests - Invalid Nested Types
// ===========================================

#[test]
fn parse_empty_tag_fixture() {
    let path = invalid_fixture("empty_tag.md");
    let content = read_fixture(&path);
    let result = parse(&content);

    assert!(
        matches!(result, Err(ParseError::InvalidYaml(_))),
        "expected InvalidYaml for empty tag, got {:?}",
        result
    );
}

#[test]
fn parse_invalid_tag_chars_fixture() {
    let path = invalid_fixture("invalid_tag_chars.md");
    let content = read_fixture(&path);
    let result = parse(&content);

    assert!(
        matches!(result, Err(ParseError::InvalidYaml(_))),
        "expected InvalidYaml for invalid tag characters, got {:?}",
        result
    );
}

#[test]
fn parse_empty_topic_fixture() {
    let path = invalid_fixture("empty_topic.md");
    let content = read_fixture(&path);
    let result = parse(&content);

    assert!(
        matches!(result, Err(ParseError::InvalidYaml(_))),
        "expected InvalidYaml for empty topic, got {:?}",
        result
    );
}

#[test]
fn parse_invalid_topic_chars_fixture() {
    let path = invalid_fixture("invalid_topic_chars.md");
    let content = read_fixture(&path);
    let result = parse(&content);

    assert!(
        matches!(result, Err(ParseError::InvalidYaml(_))),
        "expected InvalidYaml for invalid topic characters, got {:?}",
        result
    );
}

#[test]
fn parse_empty_rel_fixture() {
    let path = invalid_fixture("empty_rel.md");
    let content = read_fixture(&path);
    let result = parse(&content);

    assert!(
        matches!(result, Err(ParseError::InvalidYaml(_))),
        "expected InvalidYaml for empty rel, got {:?}",
        result
    );
}

#[test]
fn parse_invalid_rel_underscore_fixture() {
    let path = invalid_fixture("invalid_rel_underscore.md");
    let content = read_fixture(&path);
    let result = parse(&content);

    assert!(
        matches!(result, Err(ParseError::InvalidYaml(_))),
        "expected InvalidYaml for rel with underscore, got {:?}",
        result
    );
}
