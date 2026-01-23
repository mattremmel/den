# Den

A command-line tool (`notes`) for managing markdown notes with virtual folder organization through hierarchical topics.

Notes are stored as flat markdown files with YAML frontmatter. Organization happens through metadata (topics and tags), not filesystem hierarchy. A SQLite index provides fast querying and full-text search.

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
# Binary will be at target/release/notes
```

### Shell Completions

Generate and install shell completions for tab-completion of commands and options:

```bash
# Bash (add to ~/.bashrc)
notes completions bash > ~/.local/share/bash-completion/completions/notes

# Zsh (add to fpath directory)
notes completions zsh > ~/.zfunc/_notes

# Fish
notes completions fish > ~/.config/fish/completions/notes.fish
```

## Configuration

notes uses a TOML configuration file located at:

- **macOS**: `~/.config/notes/config.toml`
- **Linux**: `~/.config/notes/config.toml`
- **Windows**: `%APPDATA%\notes\config.toml`

### Configuration Options

```toml
# Default notes directory (used when --dir is not specified)
dir = "/path/to/your/notes"

# Editor command for editing notes (used by `new --edit` and `edit` commands)
editor = "nvim"
```

### Notes Directory Resolution

The notes directory is determined in this order:

1. **CLI flag**: `--dir /path/to/notes` (highest priority)
2. **Config file**: `dir` setting in `config.toml`
3. **Current directory**: Falls back to `.` if nothing else is configured

### Editor Resolution

The editor command is determined in this order:

1. **Config file**: `editor` setting in `config.toml`
2. **$EDITOR**: Environment variable
3. **$VISUAL**: Environment variable
4. **vi**: Default fallback

The editor setting supports arguments, e.g., `editor = "code --wait"` for VS Code.

## Quick Start

```bash
# Set up your notes directory
mkdir ~/notes
echo 'dir = "/Users/me/notes"' > ~/.config/notes/config.toml

# Create your first note
notes new "Getting Started" --topic meta --tag reference --edit

# Build the index
notes index

# List all notes
notes ls

# Search for content
notes search "getting started"

# Edit an existing note
notes edit "Getting Started"
```

## Commands

### Index Management

Before using search and list commands, build the index:

```bash
# Incremental update (fast, only processes changed files)
notes index

# Full rebuild (slower, rescans everything)
notes index --full
```

The index is stored at `.index/notes.db` in your notes directory.

### Creating Notes

```bash
# Create a basic note
notes new "My Note Title"

# Create with topic and tags
notes new "API Design" --topic software/architecture --tag draft

# Create with description
notes new "Meeting Notes" --desc "Weekly team sync"

# Create and open in editor
notes new "Quick Idea" --edit

# Multiple topics and tags
notes new "Rust Async" --topic software/rust --topic reference --tag important --tag draft
```

### Listing Notes

```bash
# List all notes (sorted by modified date, most recent first)
notes ls

# List notes in a specific topic
notes ls software/rust

# List notes in topic and all descendants (trailing /)
notes ls software/

# Filter by tag
notes ls --tag draft

# Multiple tags (AND logic)
notes ls --tag draft --tag important

# Combine topic and tags
notes ls software/rust --tag reference

# Filter by date
notes ls --created 2024-01-15      # Exact date
notes ls --modified 7d             # Last 7 days
notes ls --created 30d --tag draft # Combined filters

# Output formats
notes ls --format json             # JSON output
notes ls --format paths            # Just file paths (useful for scripting)

# Include archived notes
notes ls -a                        # Show all including archived
```

### Searching Notes

Full-text search across titles, descriptions, aliases, and body content:

```bash
# Basic search
notes search "API design"

# Search within a topic
notes search "async" --topic software/rust

# Search with tag filter
notes search "error handling" --tag reference

# Output formats
notes search "query" --format json
notes search "query" --format paths

# Include archived notes
notes search "query" -a
```

### Viewing and Editing Notes

```bash
# Show a note (by ID prefix, title, or alias)
notes show 01HQ3K5M7N
notes show "API Design"
notes show REST            # If "REST" is an alias

# Edit a note
notes edit 01HQ3K5M7N
notes edit "API Design"
```

Notes can be referenced by:
- **ID prefix**: First 4+ characters of the ULID (e.g., `01HQ3K5M7N`)
- **Title**: Exact match, case-insensitive
- **Alias**: Any alias defined in the note's frontmatter

### Moving and Renaming Notes

```bash
# Rename a note (updates title and filename)
notes mv "Old Title" --title "New Title"

# Change topics
notes mv "My Note" --topic new/topic
notes mv "My Note" --topic topic1 --topic topic2   # Multiple topics

# Clear all topics
notes mv "My Note" --clear-topics

# Rename and reorganize in one command
notes mv "My Note" --title "Better Name" --topic projects/active
```

### Archiving Notes

Archive notes to hide them from default listings while preserving them:

```bash
# Archive a note
notes archive "Old Project Notes"

# Unarchive a note
notes unarchive "Old Project Notes"

# Archived notes are hidnotes by default
notes ls                       # Won't show archived notes
notes ls -a                    # Include archived notes
notes search "query" -a        # Include archived in search
```

### Topics and Tags

```bash
# List all topics
notes topics
notes topics --counts      # With note counts
notes topics --format json

# List all tags
notes tags
notes tags --counts        # With note counts
notes tags --format json

# Add a tag to a note
notes tag "API Design" important
notes tag 01HQ3K5M7N review

# Remove a tag from a note
notes untag "API Design" draft
notes untag 01HQ3K5M7N obsolete
```

### Link Management

Create typed relationships between notes:

```bash
# Create a link with relationship type
notes link "API Design" "REST Principles" --rel parent

# Create a link with multiple relationship types
notes link "My Project" "Reference Doc" --rel source --rel inspiration

# Create a link with context note
notes link "Meeting Notes" "Action Items" --rel followed-by --note "Discussed in Q4 planning"

# Remove a link
notes unlink "API Design" "REST Principles"

# Show notes that link to a given note (backlinks)
notes backlinks "REST Principles"
notes backlinks "REST Principles" --rel parent    # Filter by relationship type
notes backlinks "REST Principles" --format json

# List all relationship types in use
notes rels
notes rels --counts        # With usage counts
```

### Validation

Check your notes collection for issues:

```bash
# Check for problems (broken links, orphans, etc.)
notes check

# Attempt to fix issues automatically
notes check --fix
```

### Exporting Notes

Export notes to HTML or generate a static site:

```bash
# Export a single note to HTML (outputs to stdout)
notes export "API Design"

# Export to a file
notes export "API Design" -o api-design.html

# Export with dark theme
notes export "API Design" --theme dark -o api-design.html

# Export all notes as a static site
notes export --all --format site -o ./my-site

# Export with resolved internal links (note references become clickable)
notes export "API Design" --resolve-links -o api-design.html

# Filter what to export
notes export --all -F site -o ./docs --topic software/   # Only software notes
notes export --all -F site -o ./docs --tag reference     # Only reference notes
notes export --all -F site -o ./docs -a                  # Include archived notes
```

Export formats:
- **html**: Single HTML document with syntax highlighting
- **site**: Static site with navigation sidebar and inter-note links
- **pdf**: PDF document (requires `wkhtmltopdf` or `weasyprint`)

## Note Format

Notes are markdown files with YAML frontmatter:

```markdown
---
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
links:
  - id: 01HQ4A2R9PXJK4QZPW8V2R6T9Y
    rel:
      - parent
  - id: 01HQ5B3S0QYJK5RAQX9W3S7T0Z
    rel:
      - see-also
    note: Related discussion
---

# API Design Notes

Your markdown content goes here...
```

### Fields

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | ULID inotestifier (auto-generated) |
| `title` | Yes | Human-readable title |
| `created` | Yes | Creation timestamp (ISO 8601) |
| `modified` | Yes | Last modified timestamp (ISO 8601) |
| `description` | No | Brief summary |
| `topics` | No | Hierarchical paths for organization (e.g., `software/rust/async`) |
| `aliases` | No | Alternative titles for search |
| `tags` | No | Flat labels for filtering |
| `links` | No | References to other notes with relationship types |

### Topics vs Tags

- **Topics** are hierarchical paths for browsing (like folders): `software/rust/async`
- **Tags** are flat labels for filtering: `draft`, `important`, `reference`

### File Naming

Files are named with the ULID prefix followed by a slug:

```
01HQ3K5M7N-api-design-notes.md
```

This ensures uniqueness while remaining human-readable.

## Global Options

```bash
# Specify notes directory
notes --dir /path/to/notes ls

# Increase verbosity
notes -v index          # Verbose
notes -vv index         # More verbose
notes -vvv index        # Debug level

# Version
notes --version
```

## Example Workflows

### Building a Knowledge Base

```bash
# Create topic structure through notes
notes new "Rust Overview" --topic programming/rust --tag evergreen
notes new "Ownership in Rust" --topic programming/rust/concepts --tag reference
notes new "Async Rust" --topic programming/rust/async --tag draft

# Link related notes
notes link "Ownership in Rust" "Rust Overview" --rel parent
notes link "Async Rust" "Ownership in Rust" --rel prerequisite

# Browse by topic
notes ls programming/rust/     # All Rust notes including subtopics
notes topics --counts          # See topic hierarchy

# Find what links to a concept
notes backlinks "Ownership in Rust"
```

### Project Notes

```bash
# Create project notes
notes new "Project Alpha" --topic projects/alpha --tag active
notes new "Alpha Meeting 2024-01-15" --topic projects/alpha/meetings
notes new "Alpha Architecture" --topic projects/alpha --tag decision

# Link meeting to project
notes link "Alpha Meeting 2024-01-15" "Project Alpha" --rel part-of

# Find all project notes
notes ls projects/alpha/

# Search within project
notes search "deadline" --topic projects/alpha
```

### Research and References

```bash
# Create reference notes
notes new "Clean Architecture" --topic reference/books --tag evergreen
notes new "Domain-Driven Design" --topic reference/books --tag evergreen

# Add relationship between concepts
notes link "Clean Architecture" "Domain-Driven Design" --rel see-also --note "Complementary approaches"

# Tag notes as you review them
notes tag "Clean Architecture" reviewed
notes untag "Clean Architecture" to-read

# Find all references
notes ls reference/ --tag evergreen
```

## License

MIT
