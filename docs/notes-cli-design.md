# Notes CLI - Design Specification

A CLI tool for managing markdown notes with virtual folder organization through hierarchical topics.

## Core Philosophy

Notes are stored as flat markdown files. Organization happens through metadata, not filesystem hierarchy. A single note can appear in multiple virtual "folders" without duplication. The source of truth is always the markdown file itself.

## Frontmatter Schema

Every note contains YAML frontmatter with the following structure:

```yaml
---
id: <unique-identifier>
title: <note-title>
description: <optional-short-description>
created: <ISO-8601-timestamp>
modified: <ISO-8601-timestamp>
topics:
  - <path/to/topic>
  - <another/topic>
aliases:
  - <alternate-title>
tags:
  - <flat-tag>
links:
  - id: <target-note-id>
    rel:
      - <relationship-type>
    note: <optional-context>
---
```

### Field Definitions

#### `id` (required)
A stable unique identifier for the note. Used for linking between notes and as a persistent reference that survives title changes.

**Format**: Full ULID (26 characters)

Example: `01HQ3K5M7NXJK4QZPW8V2R6T9Y`

**Why ULID**:
- Lexicographically sortable by creation time
- First 10 characters encode millisecond-precision timestamp
- Remaining 16 characters provide randomness for uniqueness
- No coordination required (can generate offline)
- The first 10 characters are used as the filename prefix

#### `title` (required)
The primary display name for the note. Used in search results, listings, and as the default link text.

#### `description` (optional)
A short summary of the note's content or purpose. One to two sentences recommended.

**Use cases**:
- Provides context in listings without opening the file
- Improves full-text search relevance
- Combined with title for vector embeddings (semantic/fuzzy search)

**Example**:
```yaml
title: API Design Principles
description: Core principles for designing RESTful APIs, including resource naming, versioning strategies, and error handling patterns.
```

#### `created` (required)
ISO 8601 timestamp indicating when the note was created. Set once at creation, never modified.

Format: `2024-01-15T10:30:00Z`

#### `modified` (required)
ISO 8601 timestamp of the last modification. Updated whenever the file content changes.

#### `topics` (optional)
Hierarchical paths that define where this note appears in the virtual folder structure. These are the primary organizational mechanism.

**Format**: Forward-slash separated paths (e.g., `software/architecture/patterns`)

**Behavior**:
- A note with topic `software/architecture` appears when browsing both `software/` and `software/architecture/`
- Topics are case-sensitive
- Leading/trailing slashes are normalized away
- Empty topics array means the note is "unfiled" but still searchable

**Examples**:
```yaml
topics:
  - software/architecture
  - software/api
  - reference/books
```

#### `aliases` (optional)
Alternative titles for the note. Used for search matching and link resolution.

**Use cases**:
- Acronyms: A note titled "Application Programming Interface" might have alias "API"
- Alternative phrasings: "REST Design" and "RESTful Architecture"
- Previous titles after a rename

**Behavior**:
- Search queries match against title AND all aliases
- Links can reference notes by alias
- Aliases are not displayed in listings (title is always shown)

#### `tags` (optional)
Flat, non-hierarchical labels for content attributes or workflow state.

**Intended uses**:
- Content type: `evergreen`, `fleeting`, `reference`, `how-to`
- Workflow state: `draft`, `needs-review`, `archived`
- Content attributes: `has-code`, `has-diagrams`

**Behavior**:
- Tags are flat strings (no hierarchy)
- Used for filtering, not browsing
- Case-insensitive matching recommended

#### `links` (optional)
Explicit references to other notes with relationship context.

**Structure**:
```yaml
links:
  - id: 01HQ3K5M7N...
    rel: [parent]
  - id: 01HQ4A2R9P...
    rel: [manager-of, mentor-to]
    note: "Hired me at Acme Corp, 2019"
```

**Fields**:
- `id` (required): The target note's ULID
- `rel` (required): Array of relationship types describing the nature of the link
- `note` (optional): Freeform context about the relationship

**Relationship types**:
- No enforced vocabulary — users define their own
- Use lowercase-hyphenated format for consistency (e.g., `parent`, `see-also`, `manager-of`)
- Common examples: `parent`, `child`, `related`, `source`, `supersedes`, `part-of`, `followed-by`

**Behavior**:
- Links are stored with their relationship metadata
- Backlinks can be filtered by relationship type
- Broken links (referencing non-existent IDs) should be flagged
- The `note` field is for human context, not queried

## File Organization

### Directory Structure

```
notes/
├── .index/
│   └── notes.db          # SQLite index database
├── 01HQ3K5M-api-design-principles.md
├── 01HQ3K8P-meeting-notes-project-x.md
├── 01HQ4A2R-rust-error-handling.md
└── ...
```

### Filename Convention

Recommended format: `<ulid-prefix>-<slug>.md`

- **ULID prefix**: First 10 characters of a ULID, providing chronological sorting and uniqueness
- **Slug**: Lowercase, hyphen-separated version of the title
- **Extension**: Always `.md`

**Example**: `01HQ3K5M7N-api-design-principles.md`

The ULID prefix solves two problems:
1. **Uniqueness**: Two notes with the same title get different prefixes
2. **Sorting**: Notes sort chronologically by creation time

**Why 10 characters**: The first 10 characters of a ULID encode the full timestamp (millisecond precision), ensuring global uniqueness across time while keeping filenames readable.

**Notes**:
- The filename is NOT the source of truth for title or date (frontmatter is)
- Filenames should be stable once created
- Renaming a file doesn't break links (ID-based linking)
- The full ULID is stored as `id` in frontmatter; the filename only uses the prefix

### Index Location

The index lives in `.index/` within the notes directory:
- Keeps index co-located with notes
- Easy to exclude from version control if desired
- Can be regenerated from source files at any time

## Index Design

### Storage: SQLite

SQLite provides the right balance of simplicity and capability:
- Single file, no server
- Full-text search via FTS5
- Complex queries for filtering/sorting
- Scales to tens of thousands of notes
- Transactional updates

### Schema

```sql
CREATE TABLE notes (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    description TEXT,          -- Optional short summary
    created TEXT NOT NULL,
    modified TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    body TEXT                  -- Full text content for FTS
);

CREATE TABLE topics (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE  -- e.g., "software/architecture"
);

CREATE TABLE note_topics (
    note_id TEXT NOT NULL REFERENCES notes(id),
    topic_id INTEGER NOT NULL REFERENCES topics(id),
    PRIMARY KEY (note_id, topic_id)
);

CREATE TABLE aliases (
    note_id TEXT NOT NULL REFERENCES notes(id),
    alias TEXT NOT NULL,
    PRIMARY KEY (note_id, alias)
);

CREATE TABLE tags (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE note_tags (
    note_id TEXT NOT NULL REFERENCES notes(id),
    tag_id INTEGER NOT NULL REFERENCES tags(id),
    PRIMARY KEY (note_id, tag_id)
);

CREATE TABLE links (
    id INTEGER PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES notes(id),
    target_id TEXT NOT NULL,  -- May reference non-existent note
    note TEXT,                -- Optional freeform context
    UNIQUE(source_id, target_id)
);

CREATE TABLE link_rels (
    link_id INTEGER NOT NULL REFERENCES links(id),
    rel TEXT NOT NULL,        -- Relationship type (e.g., "parent", "manager-of")
    PRIMARY KEY (link_id, rel)
);

-- Full-text search
CREATE VIRTUAL TABLE notes_fts USING fts5(
    title, description, aliases, body,
    content='notes',
    content_rowid='rowid'
);

-- Indexes for common queries
CREATE INDEX idx_topics_path ON topics(path);
CREATE INDEX idx_tags_name ON tags(name);
CREATE INDEX idx_notes_created ON notes(created);
CREATE INDEX idx_notes_modified ON notes(modified);
```

### Indexing Behavior

#### Full Rebuild
- Scan all `.md` files in the notes directory
- Parse frontmatter and extract metadata
- Compute content hash (for change detection)
- Clear and repopulate all tables
- Rebuild FTS index

#### Incremental Update
- Compare file modification times against index
- For changed files: re-parse and update
- For deleted files: remove from index
- For new files: add to index
- Use content hash to detect actual changes vs. just touched files

#### Watch Mode (optional)
- Use filesystem events (inotify/fsevents) to detect changes
- Apply incremental updates in real-time
- Debounce rapid changes

## Topic Hierarchy Behavior

### Implicit Ancestry

A note tagged with `software/architecture/patterns` implicitly belongs to:
- `software/architecture/patterns` (exact)
- `software/architecture` (parent)
- `software` (grandparent)

This means `notes ls software/` returns all notes under the software tree.

### Query Semantics

| Query | Matches |
|-------|---------|
| `software/architecture` | Notes with exactly this topic |
| `software/architecture/` | Notes with this topic OR any descendant |
| `software/` | All notes anywhere under software |

The trailing slash indicates "include descendants."

### Topic Listing

`notes topics` shows the full topic tree with counts:

```
software/ (47)
├── architecture/ (12)
│   └── patterns (5)
├── api (8)
└── rust (15)
reference/ (23)
├── books (10)
└── articles (13)
```

Counts include notes at that exact topic plus all descendants.

## Search Behavior

### Full-Text Search

Searches across:
- Title (highest weight)
- Description (high weight)
- Aliases (high weight)
- Body content (normal weight)

Returns results ranked by relevance.

### Vector/Semantic Search (Future)

For fuzzy lookup and semantic similarity:
- Embed `title + " " + description` as a single text chunk
- Store vectors in SQLite via `sqlite-vec` extension or separate vector store
- Enables "find similar notes" and natural language queries

### Filtering

Filters can be combined:
- `--topic software/` - restrict to topic subtree
- `--tag draft` - require specific tag
- `--created 2024-01` - date range
- `--modified 7d` - relative time (last 7 days)

### Query Examples

```bash
# Full-text search
notes search "error handling"

# Filtered search
notes search "error handling" --topic software/rust/

# Tag filter only (no text search)
notes ls --tag needs-review

# Combined filters
notes ls software/ --tag draft --modified 30d
```

## Link Resolution

### By ID
Direct and unambiguous:
```yaml
links:
  - V1StGXR8_Z5jdHi6B-myT
```

### By Title/Alias
More human-friendly but requires resolution:
- Exact title match first
- Then alias match
- Ambiguous matches should warn

### Backlinks

The index computes backlinks automatically:
```bash
notes backlinks <note-id>
notes backlinks <note-id> --rel parent    # Filter by relationship type
```

Shows all notes that link TO the specified note, optionally filtered by relationship type.

### Broken Links

Links referencing non-existent IDs are tracked:
```bash
notes check
```

Reports broken links, orphaned notes (no topics), etc.

## Error Handling

### Malformed Frontmatter

If a file cannot be parsed:
- Log a warning with filename and error
- Skip the file (don't include in index)
- Continue processing other files
- `notes check` reports all malformed files

### Missing Required Fields

If frontmatter is valid YAML but missing required fields:
- Same handling as malformed
- Clear error message indicating which field is missing

### Duplicate IDs

If two files have the same ID:
- Warn loudly
- Index the first one encountered
- Skip subsequent duplicates
- `notes check` reports duplicates

## Considerations

### Git Friendliness

- Plain markdown files diff well
- SQLite index can be gitignored (regenerable)
- Consider adding `.index/` to `.gitignore`
- Timestamps in frontmatter may cause merge conflicts

### Performance

- Index rebuild should complete in <1s for 1000 notes
- Individual queries should be <100ms
- FTS5 handles full-text search efficiently
- Content hash prevents unnecessary re-parsing

### Extensibility

The schema supports future additions:
- Custom frontmatter fields can be ignored
- New metadata fields can be added to schema
- Query language can be extended

### Portability

- Markdown files are the source of truth
- Index can be regenerated from files
- No vendor lock-in
- Easy to export/migrate
