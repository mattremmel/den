//! Prime command handler - outputs AI-friendly primer for the notes CLI.

use anyhow::Result;
use serde::Serialize;

use crate::cli::{PrimeArgs, PrimeFormat};

/// Available sections for the primer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Section {
    Concept,
    Schema,
    Files,
    Commands,
    Topics,
    Links,
    Validation,
    Workflows,
}

impl Section {
    /// All available sections in display order.
    const ALL: &'static [Section] = &[
        Section::Concept,
        Section::Schema,
        Section::Files,
        Section::Commands,
        Section::Topics,
        Section::Links,
        Section::Validation,
        Section::Workflows,
    ];

    /// Parse a section name (case-insensitive).
    fn from_str(s: &str) -> Option<Section> {
        match s.trim().to_lowercase().as_str() {
            "concept" => Some(Section::Concept),
            "schema" => Some(Section::Schema),
            "files" => Some(Section::Files),
            "commands" => Some(Section::Commands),
            "topics" => Some(Section::Topics),
            "links" => Some(Section::Links),
            "validation" => Some(Section::Validation),
            "workflows" => Some(Section::Workflows),
            _ => None,
        }
    }

    /// Get the section name for display.
    fn name(self) -> &'static str {
        match self {
            Section::Concept => "Concept",
            Section::Schema => "Schema",
            Section::Files => "Files",
            Section::Commands => "Commands",
            Section::Topics => "Topics",
            Section::Links => "Links",
            Section::Validation => "Validation",
            Section::Workflows => "Workflows",
        }
    }

    /// Get the full content for this section.
    fn full_content(self) -> &'static str {
        match self {
            Section::Concept => CONCEPT_FULL,
            Section::Schema => SCHEMA_FULL,
            Section::Files => FILES_FULL,
            Section::Commands => COMMANDS_FULL,
            Section::Topics => TOPICS_FULL,
            Section::Links => LINKS_FULL,
            Section::Validation => VALIDATION_FULL,
            Section::Workflows => WORKFLOWS_FULL,
        }
    }

    /// Get the compact content for this section.
    fn compact_content(self) -> &'static str {
        match self {
            Section::Concept => CONCEPT_COMPACT,
            Section::Schema => SCHEMA_COMPACT,
            Section::Files => FILES_COMPACT,
            Section::Commands => COMMANDS_COMPACT,
            Section::Topics => TOPICS_COMPACT,
            Section::Links => LINKS_COMPACT,
            Section::Validation => VALIDATION_COMPACT,
            Section::Workflows => WORKFLOWS_COMPACT,
        }
    }
}

/// A single section of the primer content.
#[derive(Debug, Serialize)]
pub struct PrimerSection {
    pub name: Section,
    pub content: String,
}

/// The complete primer content structure.
#[derive(Debug, Serialize)]
pub struct PrimerContent {
    pub sections: Vec<PrimerSection>,
}

/// Handle the prime command.
pub fn handle_prime(args: &PrimeArgs) -> Result<()> {
    let sections = get_sections(args);
    let content = build_content(&sections, args.compact);

    match args.format {
        PrimeFormat::Human => {
            print!("{}", format_human(&content));
        }
        PrimeFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&content)?);
        }
        PrimeFormat::Paths => {
            println!("<notes-dir>/              # Notes directory");
            println!("<notes-dir>/.index/notes.db  # SQLite index");
        }
    }

    Ok(())
}

/// Determine which sections to include.
fn get_sections(args: &PrimeArgs) -> Vec<Section> {
    if args.sections.is_empty() {
        Section::ALL.to_vec()
    } else {
        args.sections
            .iter()
            .filter_map(|s| Section::from_str(s))
            .collect()
    }
}

/// Build the primer content for the requested sections.
fn build_content(sections: &[Section], compact: bool) -> PrimerContent {
    let sections = sections
        .iter()
        .map(|&section| {
            let content = if compact {
                section.compact_content()
            } else {
                section.full_content()
            };
            PrimerSection {
                name: section,
                content: content.to_string(),
            }
        })
        .collect();

    PrimerContent { sections }
}

/// Format content for human-readable output.
fn format_human(content: &PrimerContent) -> String {
    let mut out = String::from("# Notes CLI Primer\n\n");

    for section in &content.sections {
        out.push_str("## ");
        out.push_str(section.name.name());
        out.push_str("\n\n");
        out.push_str(&section.content);
        out.push_str("\n\n");
    }

    out
}

// =============================================================================
// Full content strings
// =============================================================================

const CONCEPT_FULL: &str = "\
Notes are flat markdown files with YAML frontmatter. Organization is virtual \
through metadata, not filesystem hierarchy. The frontmatter is the source of truth. \
A SQLite index (`.index/notes.db`) enables fast queries but can be regenerated \
from source files.";

const SCHEMA_FULL: &str = "\
**Required fields:**
- `id`: Full ULID (26 chars), e.g., `01HQ3K5M7NXJK4QZPW8V2R6T9Y`
- `title`: Display name for the note
- `created`: ISO 8601 timestamp, set once at creation
- `modified`: ISO 8601 timestamp, updated on changes

**Optional fields:**
- `description`: Short summary (1-2 sentences)
- `topics`: Hierarchical paths for virtual folders, e.g., `[\"software/architecture\", \"reference/books\"]`
- `aliases`: Alternative titles for search/linking
- `tags`: Flat labels for filtering, e.g., `[\"draft\", \"needs-review\"]`
- `links`: References to other notes with relationship context
- `kind`: Note type (generic, book, paper, transcript, article)
- `metadata`: Kind-specific fields (author, url, source, etc.)

**Example frontmatter:**
```yaml
---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9Y
title: API Design Principles
description: Core principles for RESTful API design
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
topics:
  - software/architecture
  - reference
tags:
  - evergreen
links:
  - id: 01HQ4A2R9PXYZ
    rel: [related]
---
```";

const FILES_FULL: &str = "\
**Filename convention:** `<ulid-prefix>-<slug>.md`
- ULID prefix: First 10 chars of the ULID (timestamp-based, ensures uniqueness and sorting)
- Slug: Lowercase, hyphen-separated title
- Example: `01HQ3K5M7N-api-design-principles.md`

**Directory structure:**
```
notes/
├── .index/
│   └── notes.db          # SQLite index (regenerable)
├── 01HQ3K5M7N-api-design.md
├── 01HQ3K8P2X-meeting-notes.md
└── ...
```

The filename is NOT the source of truth - frontmatter is. Renaming files doesn't break links (ID-based).";

const COMMANDS_FULL: &str = "\
**Create & Edit:**
- `notes new <TITLE> [-T topic] [-t tag] [-D desc] [-k kind]` - Create note
- `notes edit <ID|TITLE>` - Open in editor
- `notes mv <NOTE> [--title NEW] [-T topic]` - Move/rename

**List & Search:**
- `notes ls [TOPIC] [-t tag] [-k kind]` - List notes (trailing `/` includes descendants)
- `notes search <QUERY> [-T topic] [-t tag]` - Full-text search
- `notes show <ID|TITLE>` - Display note contents

**Organization:**
- `notes topics [--counts]` - List topic hierarchy
- `notes tags [--counts]` - List all tags
- `notes kinds` - List note kinds with counts
- `notes tag <NOTE> <TAG>` - Add tag
- `notes untag <NOTE> <TAG>` - Remove tag

**Links:**
- `notes link <SOURCE> <TARGET> [--rel TYPE]` - Create link
- `notes unlink <SOURCE> <TARGET>` - Remove link
- `notes backlinks <NOTE> [--rel TYPE]` - Show incoming links
- `notes rels [--counts]` - List relationship types

**Maintenance:**
- `notes index [--full]` - Rebuild/update index
- `notes check [--fix]` - Validate notes (broken links, orphans)
- `notes archive <NOTE>` / `notes unarchive <NOTE>` - Archive management

**Output formats:** Most commands support `-f human|json|paths`";

const TOPICS_FULL: &str = "\
Topics are hierarchical paths providing virtual folder organization.

**Query semantics:**
| Query | Matches |
|-------|---------|
| `software/architecture` | Notes with exactly this topic |
| `software/architecture/` | Notes with this topic OR any descendant |
| `software/` | All notes anywhere under software |

The trailing `/` indicates \"include descendants.\"

**Implicit ancestry:** A note with topic `software/architecture/patterns` appears in \
queries for `software/`, `software/architecture/`, and `software/architecture/patterns`.";

const LINKS_FULL: &str = "\
Links connect notes with typed relationships stored in frontmatter.

**Structure:**
```yaml
links:
  - id: 01HQ4A2R9P...      # Target note ULID (required)
    rel: [parent, source]  # Relationship types (required)
    note: \"Context here\"   # Optional freeform context
```

**Common relationship types:** `parent`, `child`, `related`, `source`, `supersedes`, \
`part-of`, `followed-by`, `see-also`

**Backlinks:** Query with `notes backlinks <NOTE>` to find all notes linking TO a \
given note. Filter by `--rel TYPE` for specific relationships.";

const VALIDATION_FULL: &str = "\
**Key constraints:**
- `id` must be a valid ULID (26 characters)
- `id` must be unique across all notes
- `created` and `modified` must be valid ISO 8601 timestamps
- `topics` paths use forward slashes, no leading/trailing slashes
- `links[].id` should reference existing notes (broken links are flagged)
- Filenames should match pattern `<ulid-prefix>-<slug>.md`

**Validation command:** `notes check` reports:
- Malformed frontmatter
- Missing required fields
- Duplicate IDs
- Broken links
- Orphaned notes (no topics)";

const WORKFLOWS_FULL: &str = "\
**Create and organize a note:**
```bash
notes new \"Meeting Notes\" -T work/meetings -t draft
notes edit 01HQ3K5M    # Edit by ID prefix
notes tag 01HQ3K5M reviewed
notes mv 01HQ3K5M -T work/meetings/2024
```

**Find and explore:**
```bash
notes ls work/          # All notes under work/
notes search \"API design\" -T software/
notes backlinks 01HQ3K5M --rel source
```

**Link notes:**
```bash
notes link 01HQ3K5M 01HQ4A2R --rel parent --rel source
notes backlinks 01HQ4A2R    # See what links to it
```

**Maintenance:**
```bash
notes index             # Update index
notes check --fix       # Find and fix issues
```";

// =============================================================================
// Compact content strings
// =============================================================================

const CONCEPT_COMPACT: &str = "\
Flat .md files + YAML frontmatter. Virtual folders via topics. \
SQLite index at `.index/notes.db` (regenerable).";

const SCHEMA_COMPACT: &str = "\
**Required:** id (ULID), title, created, modified (ISO 8601)
**Optional:** description, topics[], aliases[], tags[], links[], kind, metadata{}";

const FILES_COMPACT: &str = "\
Filename: `<10-char-ulid>-<slug>.md` (e.g., `01HQ3K5M7N-api-design.md`)
Index: `<notes-dir>/.index/notes.db`";

const COMMANDS_COMPACT: &str = "\
new/edit/mv | ls/search/show | topics/tags/kinds | link/unlink/backlinks | index/check
Trailing `/` in topic = include descendants. Use `-f json` for structured output.";

const TOPICS_COMPACT: &str = "\
`software/arch` = exact match. `software/arch/` = with descendants. `software/` = all under software.";

const LINKS_COMPACT: &str = "\
links: [{id: ULID, rel: [types], note: \"context\"}]. Common rels: parent, related, source, see-also.";

const VALIDATION_COMPACT: &str = "\
Valid ULID, unique ID, ISO timestamps, no leading/trailing slashes in topics. `notes check` validates.";

const WORKFLOWS_COMPACT: &str = "\
`notes new \"Title\" -T topic -t tag` | `notes ls topic/` | `notes link A B --rel type` | `notes check`";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_from_str_is_case_insensitive() {
        assert_eq!(Section::from_str("concept"), Some(Section::Concept));
        assert_eq!(Section::from_str("CONCEPT"), Some(Section::Concept));
        assert_eq!(Section::from_str("Concept"), Some(Section::Concept));
        assert_eq!(Section::from_str("  concept  "), Some(Section::Concept));
    }

    #[test]
    fn section_from_str_returns_none_for_invalid() {
        assert_eq!(Section::from_str("invalid"), None);
        assert_eq!(Section::from_str(""), None);
    }

    #[test]
    fn all_sections_have_content() {
        for section in Section::ALL {
            assert!(!section.full_content().is_empty());
            assert!(!section.compact_content().is_empty());
        }
    }

    #[test]
    fn compact_content_is_shorter_than_full() {
        for section in Section::ALL {
            assert!(
                section.compact_content().len() < section.full_content().len(),
                "compact should be shorter for {:?}",
                section
            );
        }
    }
}
