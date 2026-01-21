# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is **den** (working name: notes-cli), a Rust CLI tool for managing markdown notes with virtual folder organization through hierarchical topics. Notes are stored as flat markdown files with YAML frontmatter; organization happens through metadata, not filesystem hierarchy.

The project is currently in the design/specification phase with no implementation yet.

## Architecture

### Core Concept
- Notes are flat `.md` files with YAML frontmatter as the source of truth
- Virtual folder structure via `topics` (hierarchical paths like `software/architecture/patterns`)
- SQLite index (`.index/notes.db`) for fast querying, regenerable from source files
- ULID-based IDs for unique identification and chronological sorting

### Planned Module Structure
```
src/
├── cli/          # Clap command definitions and handlers
├── domain/       # Core types: Note, Topic, Tag, NoteId (ULID)
├── index/        # SQLite repository and query builders
└── infra/        # File I/O, frontmatter parsing, config
```

### Key Design Decisions
- **IDs**: Full ULID in frontmatter, 8-char prefix in filenames (e.g., `01HQ3K5M-api-design.md`)
- **Topics vs Tags**: Topics are hierarchical (for browsing), tags are flat (for filtering)
- **Links**: Stored in frontmatter with `rel` array for typed relationships and optional `note` context
- **FTS**: SQLite FTS5 with weighted search across title > description > aliases > body

## Documentation

- `docs/notes-cli-design.md` - Comprehensive design spec (frontmatter schema, index design, query semantics)
- `docs/notes-cli-implementation.md` - Phased Rust implementation plan with module structure and dependencies

## Planned Commands

```
notes index [--full]              # Rebuild/update index
notes ls [TOPIC] [--tag TAG]      # List notes (trailing / includes descendants)
notes search <QUERY>              # Full-text search
notes new <TITLE> [--topic] [--tag] [--desc]
notes show/edit <ID|TITLE>
notes topics/tags [--counts]
notes tag/untag <NOTE> <TAG>
notes link/unlink/backlinks
notes check [--fix]               # Validation
```

## Build Commands (once implemented)

```bash
cargo build                       # Build debug
cargo build --release             # Build release
cargo test                        # Run all tests
cargo test <test_name>            # Run single test
cargo clippy                      # Lint
cargo fmt                         # Format
```

## Key Dependencies (planned)

- `clap` (derive) for CLI
- `rusqlite` (bundled, FTS5) for index
- `serde`/`serde_yaml` for frontmatter
- `ulid` for ID generation
- `chrono` for timestamps
- `walkdir` for directory scanning
