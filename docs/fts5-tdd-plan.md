# FTS5 Implementation TDD Plan

## Overview

Implement SQLite FTS5 full-text search with weighted ranking for the notes index. The FTS5 virtual table will index `title`, `description`, `aliases`, and `body` columns from the `notes` table, with weighted search favoring title matches.

## Design Decisions

### External Content Table
Use FTS5's **external content** feature (`content='notes'`) rather than storing content twice. This:
- Reduces storage by ~50%
- Requires triggers to keep FTS in sync
- Is the idiomatic approach for FTS5 with existing data

### Column Weighting
Per the design spec, search relevance should be weighted:
1. **title** (highest) - weight 10.0
2. **description** (high) - weight 5.0
3. **aliases** (high) - weight 5.0
4. **body** (normal) - weight 1.0

FTS5's `bm25()` function accepts per-column weights.

### Aliases Storage
The `aliases` table stores one row per alias. For FTS, we'll concatenate all aliases into a single space-separated string. This is handled during INSERT/UPDATE via a trigger or during indexing.

## TDD Cycles

### Cycle 1: FTS5 Table Creation
**Test:** `fts_table_created`
- Assert `notes_fts` virtual table exists after `create_schema()`
- Verify it's an FTS5 table via `sqlite_master`

**Implementation:**
```sql
CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
    title,
    description,
    aliases,
    body,
    content='notes',
    content_rowid='rowid'
);
```

### Cycle 2: FTS5 Table Structure
**Test:** `fts_table_has_expected_columns`
- Query `notes_fts` structure to verify columns exist
- Note: FTS5 tables have different structure than regular tables

**Implementation:** No new code - validates Cycle 1

### Cycle 3: Schema Idempotency with FTS
**Test:** `create_schema_with_fts_is_idempotent`
- Call `create_schema()` multiple times
- Verify no errors, single FTS table exists

**Implementation:** Already using `IF NOT EXISTS`

### Cycle 4: Manual FTS Insert
**Test:** `fts_accepts_direct_insert`
- Insert a row into `notes_fts` with rowid matching a notes row
- Verify insertion succeeds

**Implementation:** The external content FTS table accepts manual inserts

### Cycle 5: FTS Search Returns Results
**Test:** `fts_search_finds_matching_title`
- Insert note into `notes` table
- Insert corresponding row into `notes_fts`
- Search via `notes_fts MATCH 'keyword'`
- Verify result returned

**Implementation:** Basic FTS MATCH query works

### Cycle 6: FTS Search with bm25 Ranking
**Test:** `fts_search_returns_bm25_rank`
- Insert multiple notes with varying keyword density
- Search and retrieve `bm25(notes_fts)` scores
- Verify scores are negative floats (FTS5 convention)

**Implementation:** `SELECT rowid, bm25(notes_fts) FROM notes_fts WHERE notes_fts MATCH ?`

### Cycle 7: Weighted bm25 Search
**Test:** `fts_search_weighted_title_ranks_higher`
- Insert note A with keyword in title only
- Insert note B with keyword in body only
- Search with weighted bm25: `bm25(notes_fts, 10.0, 5.0, 5.0, 1.0)`
- Assert note A ranks higher (less negative score)

**Implementation:** Apply weights to bm25() function call

### Cycle 8: INSERT Trigger
**Test:** `fts_insert_trigger_syncs_on_note_insert`
- Insert row into `notes` table
- Verify `notes_fts` contains corresponding row (via search)

**Implementation:**
```sql
CREATE TRIGGER IF NOT EXISTS notes_fts_insert
AFTER INSERT ON notes BEGIN
    INSERT INTO notes_fts(rowid, title, description, aliases, body)
    VALUES (NEW.rowid, NEW.title, NEW.description, '', NEW.body);
END;
```

Note: `aliases` requires a subquery or separate handling (see Cycle 11).

### Cycle 9: DELETE Trigger
**Test:** `fts_delete_trigger_syncs_on_note_delete`
- Insert note (trigger populates FTS)
- Delete note from `notes`
- Verify note is not searchable in FTS

**Implementation:**
```sql
CREATE TRIGGER IF NOT EXISTS notes_fts_delete
AFTER DELETE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, description, aliases, body)
    VALUES ('delete', OLD.rowid, OLD.title, OLD.description, '', OLD.body);
END;
```

FTS5 uses special `'delete'` command for external content tables.

### Cycle 10: UPDATE Trigger
**Test:** `fts_update_trigger_syncs_on_note_update`
- Insert note with title "Original"
- Update title to "Modified"
- Search for "Modified" - should find
- Search for "Original" - should not find

**Implementation:**
```sql
CREATE TRIGGER IF NOT EXISTS notes_fts_update
AFTER UPDATE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, description, aliases, body)
    VALUES ('delete', OLD.rowid, OLD.title, OLD.description, '', OLD.body);
    INSERT INTO notes_fts(rowid, title, description, aliases, body)
    VALUES (NEW.rowid, NEW.title, NEW.description, '', NEW.body);
END;
```

### Cycle 11: Aliases Integration
**Test:** `fts_search_finds_by_alias`
- Insert note with aliases
- Insert corresponding aliases into `aliases` table
- Verify search by alias finds the note

**Challenge:** External content FTS5 with triggers can't easily join tables. Options:
1. **Store concatenated aliases in notes table** (denormalization)
2. **Rebuild FTS manually** (no triggers, explicit sync)
3. **Use trigger with subquery** (if SQLite version supports)

**Recommended approach:** Add `aliases_text` column to `notes` table for FTS purposes. The indexing code maintains this as a space-separated concatenation of aliases. Triggers use this column.

**Implementation:**
1. Add `aliases_text TEXT` column to `notes` schema
2. Update triggers to use `aliases_text` instead of placeholder
3. Indexing code sets `aliases_text` when upserting notes

### Cycle 12: Aliases Column Migration
**Test:** `notes_table_has_aliases_text_column`
- Verify `aliases_text` column exists in `notes` table

**Implementation:** Add to schema:
```sql
ALTER TABLE notes ADD COLUMN aliases_text TEXT;
```
Or include in initial CREATE TABLE (preferred for new schemas).

### Cycle 13: Full Integration Test
**Test:** `fts_weighted_search_full_integration`
- Insert three notes via normal `notes` INSERT:
  - Note A: "rust" in title, no body
  - Note B: "rust" in description only
  - Note C: "rust" in body only
- Search for "rust" with weighted bm25
- Verify ranking: A > B > C

**Implementation:** Validates all components work together

### Cycle 14: Schema Version Bump
**Test:** `schema_version_is_2_with_fts`
- After creating schema with FTS, version should be 2

**Implementation:**
- Update `schema_version` insert to version 2
- Consider migration path for existing v1 databases

### Cycle 15: FTS Rebuild Command
**Test:** `fts_rebuild_repopulates_index`
- Insert notes
- Corrupt FTS (delete directly from notes_fts)
- Call rebuild
- Verify search works again

**Implementation:**
```sql
INSERT INTO notes_fts(notes_fts) VALUES('rebuild');
```

This is FTS5's built-in rebuild command for external content tables.

## File Changes

### `src/index/schema.rs`
- Add `aliases_text` column to `notes` table
- Add `notes_fts` virtual table creation
- Add INSERT/UPDATE/DELETE triggers
- Bump schema version to 2

### `src/index/mod.rs` (no changes expected)

### Tests in `src/index/schema.rs`
- Add all test cases from cycles above
- Add helper functions for FTS testing

## Implementation Order

1. Cycles 1-3: Basic FTS table creation
2. Cycles 4-7: Search functionality with weighting
3. Cycle 12: Add `aliases_text` column
4. Cycles 8-10: Triggers for sync
5. Cycle 11, 13: Aliases integration and full test
6. Cycles 14-15: Version bump and rebuild support

## Rust Best Practices

- Use `rusqlite`'s bundled SQLite with FTS5 enabled (already in Cargo.toml)
- Parameterize all queries to prevent SQL injection
- Use transactions for multi-statement operations
- Return `IndexResult<T>` for all fallible operations
- Keep SQL in string constants for readability
- Add doc comments to new public functions
- Follow existing test patterns (helper functions, cycle comments)

## Edge Cases to Test

1. Empty search query → `InvalidQuery` error
2. Search with no results → empty Vec
3. Special characters in search → proper escaping
4. NULL description/body → handled gracefully
5. Unicode in title/body → indexed correctly
6. Very long body text → no truncation issues
