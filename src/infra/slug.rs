//! Slug generation for note filenames.

use crate::domain::NoteId;

/// Converts a title to a URL-friendly slug.
///
/// - Converts to lowercase
/// - Replaces spaces with hyphens
/// - Keeps only alphanumeric characters, hyphens, and underscores
/// - Collapses consecutive hyphens
/// - Trims leading/trailing hyphens
/// - Truncates to 50 characters (at word boundary if possible)
/// - Returns "untitled" for empty results
///
/// # Examples
///
/// ```
/// use den::infra::slugify;
///
/// assert_eq!(slugify("API Design"), "api-design");
/// assert_eq!(slugify("Hello World!"), "hello-world");
/// assert_eq!(slugify(""), "untitled");
/// ```
pub fn slugify(title: &str) -> String {
    const MAX_LENGTH: usize = 50;

    // Convert to lowercase
    let lower = title.to_lowercase();

    // Replace spaces with hyphens and filter invalid characters
    let mut result = String::new();
    for c in lower.chars() {
        if c.is_ascii_alphanumeric() {
            result.push(c);
        } else if c == ' ' || c == '-' || c == '_' {
            // Replace spaces with hyphens, keep hyphens and underscores
            result.push(if c == ' ' { '-' } else { c });
        }
        // Skip all other characters
    }

    // Collapse consecutive hyphens
    let mut collapsed = String::new();
    let mut prev_was_hyphen = false;
    for c in result.chars() {
        if c == '-' {
            if !prev_was_hyphen {
                collapsed.push(c);
            }
            prev_was_hyphen = true;
        } else {
            collapsed.push(c);
            prev_was_hyphen = false;
        }
    }

    // Trim leading and trailing hyphens
    let trimmed = collapsed.trim_matches('-');

    // Return "untitled" for empty result
    if trimmed.is_empty() {
        return "untitled".to_string();
    }

    // Truncate to MAX_LENGTH
    if trimmed.len() <= MAX_LENGTH {
        return trimmed.to_string();
    }

    // Try to truncate at a hyphen boundary
    let truncated = &trimmed[..MAX_LENGTH];
    if let Some(last_hyphen) = truncated.rfind('-')
        && last_hyphen > MAX_LENGTH / 2
    {
        // Only use hyphen boundary if it's not too early
        return truncated[..last_hyphen].to_string();
    }

    // Otherwise just truncate and trim trailing hyphens
    truncated.trim_end_matches('-').to_string()
}

/// Generates a filename from a NoteId and title.
///
/// Format: `{10-char-prefix}-{slug}.md`
///
/// # Examples
///
/// ```
/// use den::domain::NoteId;
/// use den::infra::generate_filename;
///
/// let id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
/// assert_eq!(generate_filename(&id, "API Design"), "01HQ3K5M7N-api-design.md");
/// ```
pub fn generate_filename(id: &NoteId, title: &str) -> String {
    format!("{}-{}.md", id.prefix(), slugify(title))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===========================================
    // Phase 1: slugify() Basic Transformations
    // ===========================================

    #[test]
    fn slugify_converts_to_lowercase() {
        assert_eq!(slugify("API Design"), "api-design");
        assert_eq!(slugify("HELLO WORLD"), "hello-world");
        assert_eq!(slugify("CamelCase"), "camelcase");
    }

    #[test]
    fn slugify_replaces_spaces_with_hyphens() {
        assert_eq!(slugify("hello world"), "hello-world");
        assert_eq!(slugify("foo bar baz"), "foo-bar-baz");
    }

    #[test]
    fn slugify_collapses_multiple_spaces() {
        assert_eq!(slugify("hello   world"), "hello-world");
        assert_eq!(slugify("foo  bar   baz"), "foo-bar-baz");
    }

    #[test]
    fn slugify_removes_special_characters() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("foo@bar#baz"), "foobarbaz");
        assert_eq!(slugify("test (draft)"), "test-draft");
        assert_eq!(slugify("API: Design Notes"), "api-design-notes");
    }

    #[test]
    fn slugify_preserves_hyphens_and_underscores() {
        assert_eq!(slugify("my-title"), "my-title");
        assert_eq!(slugify("my_title"), "my_title");
        assert_eq!(slugify("foo-bar_baz"), "foo-bar_baz");
    }

    #[test]
    fn slugify_removes_leading_trailing_hyphens() {
        assert_eq!(slugify("-hello-"), "hello");
        assert_eq!(slugify("--foo--"), "foo");
        assert_eq!(slugify(" hello "), "hello");
        assert_eq!(slugify("!hello!"), "hello");
    }

    #[test]
    fn slugify_collapses_multiple_hyphens() {
        assert_eq!(slugify("foo--bar"), "foo-bar");
        assert_eq!(slugify("foo---bar----baz"), "foo-bar-baz");
        assert_eq!(slugify("hello - world"), "hello-world");
    }

    #[test]
    fn slugify_empty_string_returns_untitled() {
        assert_eq!(slugify(""), "untitled");
    }

    #[test]
    fn slugify_only_special_chars_returns_untitled() {
        assert_eq!(slugify("!@#$%"), "untitled");
        assert_eq!(slugify("---"), "untitled");
        assert_eq!(slugify("   "), "untitled");
        assert_eq!(slugify("..."), "untitled");
    }

    #[test]
    fn slugify_truncates_long_titles() {
        // Create a title longer than 50 characters
        let long_title = "this-is-a-very-long-title-that-exceeds-fifty-characters-limit";
        let result = slugify(long_title);
        assert!(result.len() <= 50, "Result should be <= 50 chars");
        assert!(!result.ends_with('-'), "Result should not end with hyphen");
    }

    #[test]
    fn slugify_truncates_at_word_boundary() {
        // This title is longer than 50 chars and has hyphens
        let long_title = "this-is-a-title-with-many-words-that-exceeds-the-fifty-character-limit";
        let result = slugify(long_title);
        assert!(result.len() <= 50);
        // Should truncate at a hyphen boundary if reasonable
        assert!(!result.ends_with('-'));
    }

    #[test]
    fn slugify_handles_unicode() {
        // Unicode characters should be filtered out, not crash
        assert_eq!(slugify("日本語タイトル"), "untitled");
        assert_eq!(slugify("Café Design"), "caf-design");
        assert_eq!(slugify("émoji 🎉 test"), "moji-test");
    }

    #[test]
    fn slugify_preserves_numbers() {
        assert_eq!(slugify("Version 2.0"), "version-20");
        assert_eq!(slugify("Chapter 10"), "chapter-10");
        assert_eq!(slugify("2024 Goals"), "2024-goals");
    }

    // ===========================================
    // Phase 2: generate_filename()
    // ===========================================

    #[test]
    fn generate_filename_combines_prefix_and_slug() {
        let id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
        let result = generate_filename(&id, "API Design");
        assert_eq!(result, "01HQ3K5M7N-api-design.md");
    }

    #[test]
    fn generate_filename_uses_10_char_prefix() {
        let id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
        let result = generate_filename(&id, "Test");
        // Should start with 10-char prefix
        assert!(result.starts_with("01HQ3K5M7N-"));
    }

    #[test]
    fn generate_filename_has_md_extension() {
        let id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
        let result = generate_filename(&id, "Any Title");
        assert!(result.ends_with(".md"));
    }

    #[test]
    fn generate_filename_handles_empty_title() {
        let id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
        let result = generate_filename(&id, "");
        assert_eq!(result, "01HQ3K5M7N-untitled.md");
    }

    #[test]
    fn generate_filename_handles_special_chars() {
        let id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap();
        let result = generate_filename(&id, "Hello, World!");
        assert_eq!(result, "01HQ3K5M7N-hello-world.md");
    }
}
