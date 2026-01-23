# den

A command-line tool for managing markdown notes with virtual folder organization through hierarchical topics.

Notes are stored as flat markdown files with YAML frontmatter. Organization happens through metadata (topics and tags), not filesystem hierarchy. A SQLite index provides fast querying and full-text search.

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
# Binary will be at target/release/den
```

### Shell Completions

Generate and install shell completions for tab-completion of commands and options:

```bash
# Bash (add to ~/.bashrc)
den completions bash > ~/.local/share/bash-completion/completions/den

# Zsh (add to fpath directory)
den completions zsh > ~/.zfunc/_den

# Fish
den completions fish > ~/.config/fish/completions/den.fish
```

## Configuration

den uses a TOML configuration file located at:

- **macOS**: `~/.config/den/config.toml`
- **Linux**: `~/.config/den/config.toml`
- **Windows**: `%APPDATA%\den\config.toml`

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
echo 'dir = "/Users/me/notes"' > ~/.config/den/config.toml

# Create your first note
den new "Getting Started with den" --topic meta --tag reference --edit

# Build the index
den index

# List all notes
den ls

# Search for content
den search "getting started"

# Edit an existing note
den edit "Getting Started"
```

## Commands

### Index Management

Before using search and list commands, build the index:

```bash
# Incremental update (fast, only processes changed files)
den index

# Full rebuild (slower, rescans everything)
den index --full
```

The index is stored at `.index/notes.db` in your notes directory.

### Creating Notes

```bash
# Create a basic note
den new "My Note Title"

# Create with topic and tags
den new "API Design" --topic software/architecture --tag draft

# Create with description
den new "Meeting Notes" --desc "Weekly team sync"

# Create and open in editor
den new "Quick Idea" --edit

# Multiple topics and tags
den new "Rust Async" --topic software/rust --topic reference --tag important --tag draft
```

### Listing Notes

```bash
# List all notes (sorted by modified date, most recent first)
den ls

# List notes in a specific topic
den ls software/rust

# List notes in topic and all descendants (trailing /)
den ls software/

# Filter by tag
den ls --tag draft

# Multiple tags (AND logic)
den ls --tag draft --tag important

# Combine topic and tags
den ls software/rust --tag reference

# Filter by date
den ls --created 2024-01-15      # Exact date
den ls --modified 7d             # Last 7 days
den ls --created 30d --tag draft # Combined filters

# Output formats
den ls --format json             # JSON output
den ls --format paths            # Just file paths (useful for scripting)
```

### Searching Notes

Full-text search across titles, descriptions, aliases, and body content:

```bash
# Basic search
den search "API design"

# Search within a topic
den search "async" --topic software/rust

# Search with tag filter
den search "error handling" --tag reference

# Output formats
den search "query" --format json
den search "query" --format paths
```

### Viewing and Editing Notes

```bash
# Show a note (by ID prefix, title, or alias)
den show 01HQ3K5M7N
den show "API Design"
den show REST            # If "REST" is an alias

# Edit a note
den edit 01HQ3K5M7N
den edit "API Design"
```

Notes can be referenced by:
- **ID prefix**: First 4+ characters of the ULID (e.g., `01HQ3K5M7N`)
- **Title**: Exact match, case-insensitive
- **Alias**: Any alias defined in the note's frontmatter

### Topics and Tags

```bash
# List all topics
den topics
den topics --counts      # With note counts
den topics --format json

# List all tags
den tags
den tags --counts        # With note counts
den tags --format json

# Add a tag to a note
den tag "API Design" important
den tag 01HQ3K5M7N review

# Remove a tag from a note
den untag "API Design" draft
den untag 01HQ3K5M7N obsolete
```

### Link Management

Create typed relationships between notes:

```bash
# Create a link with relationship type
den link "API Design" "REST Principles" --rel parent

# Create a link with multiple relationship types
den link "My Project" "Reference Doc" --rel source --rel inspiration

# Create a link with context note
den link "Meeting Notes" "Action Items" --rel followed-by --note "Discussed in Q4 planning"

# Remove a link
den unlink "API Design" "REST Principles"

# Show notes that link to a given note (backlinks)
den backlinks "REST Principles"
den backlinks "REST Principles" --rel parent    # Filter by relationship type
den backlinks "REST Principles" --format json

# List all relationship types in use
den rels
den rels --counts        # With usage counts
```

### Validation

Check your notes collection for issues:

```bash
# Check for problems (broken links, orphans, etc.)
den check

# Attempt to fix issues automatically
den check --fix
```

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
| `id` | Yes | ULID identifier (auto-generated) |
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
den --dir /path/to/notes ls

# Increase verbosity
den -v index          # Verbose
den -vv index         # More verbose
den -vvv index        # Debug level

# Version
den --version
```

## Example Workflows

### Building a Knowledge Base

```bash
# Create topic structure through notes
den new "Rust Overview" --topic programming/rust --tag evergreen
den new "Ownership in Rust" --topic programming/rust/concepts --tag reference
den new "Async Rust" --topic programming/rust/async --tag draft

# Link related notes
den link "Ownership in Rust" "Rust Overview" --rel parent
den link "Async Rust" "Ownership in Rust" --rel prerequisite

# Browse by topic
den ls programming/rust/     # All Rust notes including subtopics
den topics --counts          # See topic hierarchy

# Find what links to a concept
den backlinks "Ownership in Rust"
```

### Project Notes

```bash
# Create project notes
den new "Project Alpha" --topic projects/alpha --tag active
den new "Alpha Meeting 2024-01-15" --topic projects/alpha/meetings
den new "Alpha Architecture" --topic projects/alpha --tag decision

# Link meeting to project
den link "Alpha Meeting 2024-01-15" "Project Alpha" --rel part-of

# Find all project notes
den ls projects/alpha/

# Search within project
den search "deadline" --topic projects/alpha
```

### Research and References

```bash
# Create reference notes
den new "Clean Architecture" --topic reference/books --tag evergreen
den new "Domain-Driven Design" --topic reference/books --tag evergreen

# Add relationship between concepts
den link "Clean Architecture" "Domain-Driven Design" --rel see-also --note "Complementary approaches"

# Tag notes as you review them
den tag "Clean Architecture" reviewed
den untag "Clean Architecture" to-read

# Find all references
den ls reference/ --tag evergreen
```

## License

MIT
