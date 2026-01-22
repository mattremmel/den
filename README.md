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

## Usage

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
# List all topics (not yet implemented)
den topics
den topics --counts      # With note counts

# List all tags (not yet implemented)
den tags
den tags --counts        # With note counts
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
```

## Example Workflow

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

## License

MIT
