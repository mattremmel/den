//! Link resolution for note exports.
//!
//! Resolves internal note references (ULID prefixes) to proper HTML file paths
//! in exported content.

use std::collections::HashMap;

use regex::{Captures, Regex};

use crate::index::{IndexRepository, IndexedNote};
use crate::infra::slugify;

/// Result of resolving links in content.
#[derive(Debug)]
pub struct LinkResolution {
    /// The content with resolved links.
    pub content: String,
    /// Number of links successfully resolved.
    pub resolved: usize,
    /// Number of broken links (target not found).
    pub broken: usize,
}

/// Options for link resolution.
#[derive(Debug, Clone)]
pub struct LinkResolverOptions {
    /// How to handle broken links.
    pub broken_link_handling: BrokenLinkHandling,
    /// Base path prefix for resolved links (e.g., "" for same directory, "../" for parent).
    pub base_path: String,
}

impl Default for LinkResolverOptions {
    fn default() -> Self {
        Self {
            broken_link_handling: BrokenLinkHandling::MarkBroken,
            base_path: String::new(),
        }
    }
}

/// How to handle broken links (links to notes that don't exist).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokenLinkHandling {
    /// Leave the link unchanged.
    Preserve,
    /// Add a CSS class to mark it as broken.
    MarkBroken,
    /// Remove the link, keeping only the text.
    RemoveLink,
}

/// A link resolver that converts note ID references to HTML file paths.
pub struct LinkResolver<'a> {
    /// Map from note ID prefix to (slug, title) for quick lookup.
    note_map: HashMap<String, (String, String)>,
    /// Options for resolution.
    options: &'a LinkResolverOptions,
}

impl<'a> LinkResolver<'a> {
    /// Creates a new link resolver from an index.
    pub fn from_index<R: IndexRepository>(index: &R, options: &'a LinkResolverOptions) -> Self {
        let notes = index.list_all().unwrap_or_default();
        Self::from_notes(&notes, options)
    }

    /// Creates a new link resolver from a list of notes.
    pub fn from_notes(notes: &[IndexedNote], options: &'a LinkResolverOptions) -> Self {
        let mut note_map = HashMap::new();

        for note in notes {
            let prefix = note.id().prefix();
            let slug = slugify(note.title());
            let title = note.title().to_string();

            // Store with prefix for lookup
            note_map.insert(prefix, (slug, title));
        }

        Self { note_map, options }
    }

    /// Resolves links in markdown content.
    ///
    /// Looks for markdown links like `[text](ID)` where ID looks like a ULID prefix
    /// (4+ alphanumeric characters, not starting with http/https/mailto/etc).
    pub fn resolve(&self, content: &str) -> LinkResolution {
        use std::cell::Cell;

        // Regex to match markdown links: [text](target)
        // We want to find links where target looks like a note ID (not a URL)
        let link_re = Regex::new(r"\[([^\]]*)\]\(([^)]+)\)").unwrap();

        let resolved = Cell::new(0usize);
        let broken = Cell::new(0usize);

        let result = link_re.replace_all(content, |caps: &Captures| {
            let text = &caps[1];
            let target = &caps[2];

            // Skip if it looks like a URL or path
            if is_external_link(target) {
                return caps[0].to_string();
            }

            // Try to resolve as note ID prefix
            if let Some((slug, _title)) = self.note_map.get(target) {
                resolved.set(resolved.get() + 1);
                format!("[{}]({}{}{})", text, self.options.base_path, slug, ".html")
            } else if looks_like_note_id(target) {
                // Looks like a note ID but not found - broken link
                broken.set(broken.get() + 1);
                match self.options.broken_link_handling {
                    BrokenLinkHandling::Preserve => caps[0].to_string(),
                    BrokenLinkHandling::MarkBroken => {
                        format!("[{}](# \"Broken link: {}\")", text, target)
                    }
                    BrokenLinkHandling::RemoveLink => text.to_string(),
                }
            } else {
                // Not a note ID, leave unchanged
                caps[0].to_string()
            }
        });

        LinkResolution {
            content: result.into_owned(),
            resolved: resolved.get(),
            broken: broken.get(),
        }
    }

    /// Looks up a note by ID prefix.
    pub fn lookup(&self, id_prefix: &str) -> Option<(&str, &str)> {
        self.note_map
            .get(id_prefix)
            .map(|(slug, title)| (slug.as_str(), title.as_str()))
    }
}

/// Checks if a link target looks like an external URL.
fn is_external_link(target: &str) -> bool {
    let lower = target.to_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("tel:")
        || lower.starts_with("ftp://")
        || lower.starts_with('#') // Anchor link
        || target.contains('/') // Path-like
        || target.contains('.') // File extension or domain
}

/// Checks if a string looks like a ULID note ID prefix.
fn looks_like_note_id(s: &str) -> bool {
    // ULID prefixes are 4-26 characters, alphanumeric (Crockford Base32)
    // They start with 0-7 (timestamp encoding)
    s.len() >= 4
        && s.len() <= 26
        && s.chars().all(|c| c.is_ascii_alphanumeric())
        && s.chars().next().is_some_and(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::NoteId;
    use crate::infra::ContentHash;
    use chrono::Utc;

    fn create_mock_note(title: &str, id: &str) -> IndexedNote {
        let note_id = id.parse::<NoteId>().unwrap_or_else(|_| NoteId::new());
        let now = Utc::now();
        let content_hash = ContentHash::compute(b"test");

        IndexedNote::builder(
            note_id,
            title,
            now,
            now,
            format!("{}.md", slugify(title)).into(),
            content_hash,
        )
        .build()
    }

    #[test]
    fn test_resolve_internal_link() {
        let notes = vec![create_mock_note(
            "Target Note",
            "01HQ4A2R9PXJK4QZPW8V2R6T9Y",
        )];
        let options = LinkResolverOptions::default();
        let resolver = LinkResolver::from_notes(&notes, &options);

        let content = "See [Target](01HQ4A2R9P) for details.";
        let result = resolver.resolve(content);

        assert_eq!(result.resolved, 1);
        assert_eq!(result.broken, 0);
        assert!(result.content.contains("[Target](target-note.html)"));
    }

    #[test]
    fn test_preserve_external_links() {
        let notes = vec![];
        let options = LinkResolverOptions::default();
        let resolver = LinkResolver::from_notes(&notes, &options);

        let content = "Visit [example](https://example.com) for more.";
        let result = resolver.resolve(content);

        assert_eq!(result.resolved, 0);
        assert_eq!(result.broken, 0);
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_preserve_anchor_links() {
        let notes = vec![];
        let options = LinkResolverOptions::default();
        let resolver = LinkResolver::from_notes(&notes, &options);

        let content = "See [section](#heading) below.";
        let result = resolver.resolve(content);

        assert_eq!(result.resolved, 0);
        assert_eq!(result.broken, 0);
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_preserve_file_links() {
        let notes = vec![];
        let options = LinkResolverOptions::default();
        let resolver = LinkResolver::from_notes(&notes, &options);

        let content = "See [doc](./docs/readme.md) for details.";
        let result = resolver.resolve(content);

        assert_eq!(result.resolved, 0);
        assert_eq!(result.broken, 0);
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_broken_link_mark() {
        let notes = vec![];
        let options = LinkResolverOptions {
            broken_link_handling: BrokenLinkHandling::MarkBroken,
            base_path: String::new(),
        };
        let resolver = LinkResolver::from_notes(&notes, &options);

        let content = "See [Missing](01HZZZZZZZZZ) for details.";
        let result = resolver.resolve(content);

        assert_eq!(result.resolved, 0);
        assert_eq!(result.broken, 1);
        assert!(result.content.contains("Broken link"));
        assert!(result.content.contains("01HZZZZZZZZZ"));
    }

    #[test]
    fn test_broken_link_preserve() {
        let notes = vec![];
        let options = LinkResolverOptions {
            broken_link_handling: BrokenLinkHandling::Preserve,
            base_path: String::new(),
        };
        let resolver = LinkResolver::from_notes(&notes, &options);

        let content = "See [Missing](01HZZZZZZZZZ) for details.";
        let result = resolver.resolve(content);

        assert_eq!(result.broken, 1);
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_broken_link_remove() {
        let notes = vec![];
        let options = LinkResolverOptions {
            broken_link_handling: BrokenLinkHandling::RemoveLink,
            base_path: String::new(),
        };
        let resolver = LinkResolver::from_notes(&notes, &options);

        let content = "See [Missing](01HZZZZZZZZZ) for details.";
        let result = resolver.resolve(content);

        assert_eq!(result.broken, 1);
        assert_eq!(result.content, "See Missing for details.");
    }

    #[test]
    fn test_multiple_links() {
        // Use valid ULID strings (Crockford Base32: no I, L, O, U)
        let notes = vec![
            create_mock_note("First Note", "01HQ4A2R9PXJK4QZPW8V2R6T9Y"),
            create_mock_note("Second Note", "01HQ5B3S0QYJK5RAQX9W3S7V0Z"),
        ];
        let options = LinkResolverOptions::default();
        let resolver = LinkResolver::from_notes(&notes, &options);

        let content = "See [First](01HQ4A2R9P) and [Second](01HQ5B3S0Q) and [external](https://example.com).";
        let result = resolver.resolve(content);

        assert_eq!(result.resolved, 2);
        assert_eq!(result.broken, 0);
        assert!(result.content.contains("first-note.html"));
        assert!(result.content.contains("second-note.html"));
        assert!(result.content.contains("https://example.com"));
    }

    #[test]
    fn test_base_path() {
        let notes = vec![create_mock_note(
            "Target Note",
            "01HQ4A2R9PXJK4QZPW8V2R6T9Y",
        )];
        let options = LinkResolverOptions {
            broken_link_handling: BrokenLinkHandling::MarkBroken,
            base_path: "../".to_string(),
        };
        let resolver = LinkResolver::from_notes(&notes, &options);

        let content = "See [Target](01HQ4A2R9P) for details.";
        let result = resolver.resolve(content);

        assert!(result.content.contains("[Target](../target-note.html)"));
    }

    #[test]
    fn test_is_external_link() {
        assert!(is_external_link("https://example.com"));
        assert!(is_external_link("http://example.com"));
        assert!(is_external_link("mailto:test@example.com"));
        assert!(is_external_link("#heading"));
        assert!(is_external_link("./path/to/file.md"));
        assert!(is_external_link("file.txt"));

        assert!(!is_external_link("01HQ4A2R9P"));
        assert!(!is_external_link("01HQ4A2R9PXJK4QZPW8V2R6T9Y"));
    }

    #[test]
    fn test_looks_like_note_id() {
        assert!(looks_like_note_id("01HQ4A2R9P"));
        assert!(looks_like_note_id("01HQ4A2R9PXJK4QZPW8V2R6T9Y"));
        assert!(looks_like_note_id("0ABC"));

        assert!(!looks_like_note_id("abc")); // Too short
        assert!(!looks_like_note_id("AB")); // Too short
        assert!(!looks_like_note_id("ABC")); // Too short
        assert!(!looks_like_note_id("hello-world")); // Has hyphen
        assert!(!looks_like_note_id("A1234567890123456789012345678")); // Too long (>26)
    }

    #[test]
    fn test_link_with_special_text() {
        let notes = vec![create_mock_note(
            "Target Note",
            "01HQ4A2R9PXJK4QZPW8V2R6T9Y",
        )];
        let options = LinkResolverOptions::default();
        let resolver = LinkResolver::from_notes(&notes, &options);

        let content = "See [API Design Notes](01HQ4A2R9P) for details.";
        let result = resolver.resolve(content);

        assert!(result
            .content
            .contains("[API Design Notes](target-note.html)"));
    }

    #[test]
    fn test_empty_link_text() {
        let notes = vec![create_mock_note(
            "Target Note",
            "01HQ4A2R9PXJK4QZPW8V2R6T9Y",
        )];
        let options = LinkResolverOptions::default();
        let resolver = LinkResolver::from_notes(&notes, &options);

        let content = "See [](01HQ4A2R9P) for details.";
        let result = resolver.resolve(content);

        assert!(result.content.contains("[](target-note.html)"));
    }
}
