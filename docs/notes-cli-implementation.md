# Notes CLI - Rust Implementation Plan

A phased implementation plan for the notes CLI tool, starting with a library crate and CLI interface.

## Project Structure

```
notes-cli/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Library root, re-exports
│   ├── main.rs             # CLI entry point
│   │
│   ├── cli/
│   │   ├── mod.rs          # Clap app definition, command routing
│   │   ├── index.rs        # index command
│   │   ├── list.rs         # ls command
│   │   ├── search.rs       # search command
│   │   ├── note.rs         # new/show/edit commands
│   │   ├── topics.rs       # topics command
│   │   ├── tags.rs         # tags/tag/untag commands
│   │   ├── links.rs        # link/unlink/backlinks/rels commands
│   │   └── check.rs        # check command
│   │
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── note.rs         # Note struct, frontmatter types
│   │   ├── topic.rs        # Topic parsing, hierarchy logic
│   │   ├── tag.rs          # Tag type
│   │   └── id.rs           # ID generation
│   │
│   ├── index/
│   │   ├── mod.rs
│   │   ├── repository.rs   # Index trait definition
│   │   ├── sqlite.rs       # SQLite implementation
│   │   └── queries.rs      # Query builders
│   │
│   └── infra/
│       ├── mod.rs
│       ├── fs.rs           # File operations
│       ├── parser.rs       # Frontmatter parsing
│       └── config.rs       # Config file handling
│
└── tests/
    ├── integration/
    └── fixtures/
```

## Dependencies

```toml
[dependencies]
# CLI
clap = { version = "4", features = ["derive"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"

# Database
rusqlite = { version = "0.31", features = ["bundled", "functions"] }

# File operations
walkdir = "2"

# ID generation
ulid = "1"

# Time
chrono = { version = "0.4", features = ["serde"] }

# Error handling
thiserror = "1"
anyhow = "1"

# Hashing
sha2 = "0.10"

[dev-dependencies]
tempfile = "3"
assert_cmd = "2"
predicates = "3"
```

## Implementation Phases

### Phase 1: Core Domain Types

**Goal**: Define the fundamental data structures.

**Files**: `src/domain/*.rs`

**Deliverables**:
- `Note` struct with all frontmatter fields (including optional `description`)
- `Topic` type with parsing and hierarchy methods
- `Tag` type (newtype over String)
- `NoteId` type wrapping ULID with helper methods:
  - `NoteId::new()` - generate new ULID
  - `NoteId::prefix()` - return first 10 characters for filename
  - `NoteId::timestamp()` - extract creation time from ULID
- Serialization/deserialization for frontmatter

**Key decisions**:
- Use `chrono::DateTime<Utc>` for timestamps
- Topic stores both the full path and parsed segments
- Strong typing prevents mixing up IDs, topics, tags
- ULID provides both uniqueness and chronological sorting
- Description is optional, used for search ranking and future vector embeddings

**Test coverage**:
- Topic parsing edge cases (leading/trailing slashes, empty segments)
- Frontmatter round-trip serialization (with and without optional fields)
- ULID prefix extraction
- ULID timestamp extraction matches `created` field

---

### Phase 2: File Operations

**Goal**: Read and write note files.

**Files**: `src/infra/fs.rs`, `src/infra/parser.rs`

**Deliverables**:
- `parse_frontmatter(content: &str) -> Result<(Frontmatter, &str)>`
- `serialize_frontmatter(frontmatter: &Frontmatter) -> String`
- `read_note(path: &Path) -> Result<Note>`
- `write_note(path: &Path, note: &Note) -> Result<()>`
- `scan_notes_directory(dir: &Path) -> impl Iterator<Item = PathBuf>`
- Content hash computation (SHA256 of file contents)

**Key decisions**:
- Frontmatter delimited by `---` on its own line
- Preserve body content exactly (no normalization)
- Handle BOM, different line endings

**Test coverage**:
- Parse valid frontmatter
- Handle missing/malformed frontmatter gracefully
- Round-trip file read/write preserves content

---

### Phase 3: SQLite Index

**Goal**: Persistent index with query support.

**Files**: `src/index/*.rs`

**Deliverables**:
- Schema creation/migration
- `IndexRepository` trait:
  ```rust
  trait IndexRepository {
      fn upsert_note(&mut self, note: &Note, content_hash: &str) -> Result<()>;
      fn remove_note(&mut self, id: &NoteId) -> Result<()>;
      fn get_note(&self, id: &NoteId) -> Result<Option<IndexedNote>>;
      fn list_by_topic(&self, topic: &Topic, include_descendants: bool) -> Result<Vec<IndexedNote>>;
      fn list_by_tag(&self, tag: &Tag) -> Result<Vec<IndexedNote>>;
      fn search(&self, query: &str) -> Result<Vec<SearchResult>>;
      fn all_topics(&self) -> Result<Vec<TopicWithCount>>;
      fn all_tags(&self) -> Result<Vec<TagWithCount>>;
      fn get_content_hash(&self, path: &Path) -> Result<Option<String>>;
  }
  ```
- SQLite implementation of the trait
- FTS5 setup for full-text search

**Key decisions**:
- Use `rusqlite` with bundled SQLite
- Store topic hierarchy for efficient ancestor queries
- FTS5 with porter tokenizer for stemming

**Test coverage**:
- CRUD operations
- Topic hierarchy queries
- Full-text search ranking
- Concurrent access (basic)

---

### Phase 4: Index Builder

**Goal**: Build index from filesystem.

**Files**: Extension to `src/index/`, possibly `src/index/builder.rs`

**Deliverables**:
- Full rebuild: scan all files, clear index, repopulate
- Incremental update: compare hashes, update changed files
- Progress reporting callback
- Error collection (don't fail on single bad file)

**Logic**:
```
full_rebuild():
    clear all tables
    for each .md file:
        try parse note
        if ok: insert into index
        else: collect error
    rebuild FTS
    return errors

incremental_update():
    indexed_files = get all paths from index
    current_files = scan directory
    
    for file in current_files:
        if file not in indexed_files:
            parse and insert (new file)
        else if hash differs:
            parse and update (modified)
    
    for file in indexed_files:
        if file not in current_files:
            remove from index (deleted)
```

**Test coverage**:
- Full rebuild produces correct index
- Incremental detects additions, modifications, deletions
- Malformed files don't break the build

---

### Phase 5: CLI Framework

**Goal**: Basic CLI structure with help and routing.

**Files**: `src/main.rs`, `src/cli/mod.rs`

**Deliverables**:
- Clap derive-based command structure
- Subcommand routing
- Global options (notes directory, verbosity)
- Config file support (optional override of defaults)

**Commands** (stubs only in this phase):
```
notes [OPTIONS] <COMMAND>

Options:
  -d, --dir <PATH>    Notes directory [default: ~/notes]
  -v, --verbose       Increase verbosity
  -h, --help          Print help

Commands:
  index      Rebuild or update the index
  ls         List notes by topic
  search     Full-text search
  new        Create a new note
  show       Display a note
  edit       Open note in editor
  topics     List all topics
  tags       List all tags
  tag        Add tag to note
  untag      Remove tag from note
  link       Add link between notes
  unlink     Remove link between notes
  backlinks  Show notes linking to a note
  rels       List relationship types
  check      Validate notes and index
```

**Test coverage**:
- Help output
- Argument parsing
- Unknown command handling

---

### Phase 6: Core Commands

**Goal**: Implement the most-used commands.

**Files**: `src/cli/index.rs`, `src/cli/list.rs`, `src/cli/search.rs`, `src/cli/note.rs`

**`notes index`**:
```
notes index [--full]

Rebuild the index. By default, performs incremental update.
--full forces a complete rebuild.
```

**`notes ls`**:
```
notes ls [TOPIC] [--tag TAG] [--created RANGE] [--modified RANGE] [--format FORMAT]

List notes. If TOPIC ends with /, includes descendants.

Examples:
  notes ls                    # All notes
  notes ls software/          # All notes under software
  notes ls software/api       # Exact topic match
  notes ls --tag draft        # Filter by tag
```

**`notes search`**:
```
notes search <QUERY> [--topic TOPIC] [--tag TAG] [--limit N]

Full-text search across notes.
```

**`notes new`**:
```
notes new <TITLE> [--desc "description"] [--topic TOPIC]... [--tag TAG]... [--edit]

Create a new note with generated ULID and timestamps.
Filename uses first 10 characters of ULID as prefix.
--desc adds a short description to the frontmatter.
--edit opens in $EDITOR after creation.
```

**`notes show`**:
```
notes show <ID|TITLE>

Display note content to stdout.
```

**`notes edit`**:
```
notes edit <ID|TITLE>

Open note in $EDITOR. Update modified timestamp on save.
```

**Output format considerations**:
- Default: human-readable table/list
- `--format json` for scripting
- `--format paths` for piping to other tools

---

### Phase 7: Metadata Commands

**Goal**: Topic and tag management.

**Files**: `src/cli/topics.rs`, `src/cli/tags.rs`

**`notes topics`**:
```
notes topics [--tree] [--counts]

List all topics. --tree shows hierarchy, --counts shows note counts.
```

**`notes tags`**:
```
notes tags [--counts]

List all tags with optional counts.
```

**`notes tag`**:
```
notes tag <NOTE> <TAG>...

Add tags to a note. Updates the file's frontmatter.
```

**`notes untag`**:
```
notes untag <NOTE> <TAG>...

Remove tags from a note.
```

---

### Phase 8: Validation & Link Management

**Goal**: Health checks and link management.

**Files**: `src/cli/check.rs`, `src/cli/links.rs`

**`notes check`**:
```
notes check [--fix]

Validate notes and index:
- Malformed frontmatter
- Missing required fields
- Duplicate IDs
- Broken links
- Orphaned notes (no topics)
- Index out of sync with files

--fix attempts automatic repairs where possible.
```

**`notes backlinks`**:
```
notes backlinks <NOTE> [--rel REL]

Show all notes that link to the specified note.
--rel filters by relationship type (e.g., --rel parent).
```

**`notes link`**:
```
notes link <SOURCE> <TARGET> --rel REL... [--note "context"]

Add a link from source note to target note with relationship type(s).
Updates the source file's frontmatter.
```

**`notes unlink`**:
```
notes unlink <SOURCE> <TARGET>

Remove a link between two notes.
```

**`notes rels`**:
```
notes rels [--counts]

List all relationship types in use across all links.
```

---

### Phase 9: Polish & Edge Cases

**Goal**: Production readiness.

**Deliverables**:
- Comprehensive error messages
- Graceful handling of permissions issues
- Support for notes directory not existing (init command?)
- Shell completions generation
- Man page generation

**Performance**:
- Profile index rebuild on large directories
- Optimize hot paths
- Consider connection pooling for frequent operations

---

## Testing Strategy

### Unit Tests
- Domain types: parsing, serialization, hierarchy logic
- Each module has co-located tests

### Integration Tests
- End-to-end command tests using `assert_cmd`
- Fixture directories with sample notes
- Test incremental index updates

### Property Tests (optional)
- Frontmatter round-trip
- Topic hierarchy invariants

---

## Future Considerations (Not in Initial Scope)

- **Vector search**: Embed title+description via `sqlite-vec` for semantic/fuzzy lookup
- **TUI**: Interactive browsing with `ratatui`
- **Watch mode**: Live index updates via `notify`
- **Templates**: Note templates for common formats
- **Export**: Generate static site, PDF, etc.
- **Sync**: Conflict resolution for multi-device use
- **Encryption**: Support for encrypted notes
