# TDD Plan: Performance Optimization (den-1bc.3)

**Target:** <1s rebuild for 1000 notes, <100ms queries

## Executive Summary

Three optimization categories, implemented in priority order:

1. **Benchmark infrastructure** - Establish baselines before optimizing
2. **Index rebuild optimizations** - Batch transactions, eliminate double-reads
3. **Query optimizations** - Eliminate N+1 patterns, push filters to SQL

---

## Phase 1: Benchmark Infrastructure

### 1.1 Add criterion benchmarks

**Test (RED):** Create benchmark that measures current performance

```rust
// benches/index_benchmarks.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_full_rebuild(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_rebuild");
    for size in [100, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::new("full_rebuild", size),
            &size,
            |b, &size| {
                let (dir, _files) = create_test_notes(size);
                let builder = IndexBuilder::new(dir.path().to_path_buf());
                b.iter(|| {
                    let mut index = SqliteIndex::open_in_memory().unwrap();
                    builder.full_rebuild(&mut index).unwrap()
                });
            },
        );
    }
    group.finish();
}
```

**Implementation (GREEN):**
- Add `criterion` to dev-dependencies
- Create `benches/` directory with benchmark harness
- Helper function `create_test_notes(count)` generating realistic notes

**Files:**
- `Cargo.toml` - add criterion dependency
- `benches/index_benchmarks.rs` - rebuild benchmarks
- `benches/query_benchmarks.rs` - query benchmarks
- `benches/helpers.rs` - shared test data generation

### 1.2 Benchmark queries

```rust
fn bench_search(c: &mut Criterion) {
    let (index, _dir) = setup_index_with_notes(1000);

    c.bench_function("search_simple", |b| {
        b.iter(|| index.search("architecture").unwrap())
    });

    c.bench_function("search_phrase", |b| {
        b.iter(|| index.search("\"software architecture\"").unwrap())
    });
}

fn bench_list_by_topic(c: &mut Criterion) {
    let (index, _dir) = setup_index_with_notes(1000);
    let topic = Topic::new("software/architecture").unwrap();

    c.bench_function("list_by_topic", |b| {
        b.iter(|| index.list_by_topic(&topic, true).unwrap())
    });
}

fn bench_list_with_tags(c: &mut Criterion) {
    let (index, _dir) = setup_index_with_notes(1000);

    c.bench_function("list_multi_tag_filter", |b| {
        // Simulate filtering by 3 tags
        b.iter(|| {
            let tag1 = Tag::new("rust").unwrap();
            let tag2 = Tag::new("cli").unwrap();
            // Current impl: 2 separate queries + intersection
            let notes1 = index.list_by_tag(&tag1).unwrap();
            let notes2 = index.list_by_tag(&tag2).unwrap();
            // intersection logic...
        })
    });
}
```

---

## Phase 2: Index Rebuild Optimizations

### 2.1 Batch transaction for full rebuild

**Current bottleneck:** Each `upsert_note()` call wraps itself in a transaction. For 1000 notes, that's 1000 separate transactions.

**Test (RED):**
```rust
#[test]
fn full_rebuild_uses_single_transaction() {
    // Use a connection that tracks transaction count
    // or verify performance improvement is >50%
    let dir = create_test_notes(100);
    let builder = IndexBuilder::new(dir.path().to_path_buf());
    let mut index = SqliteIndex::open_in_memory().unwrap();

    let start = Instant::now();
    builder.full_rebuild(&mut index).unwrap();
    let duration = start.elapsed();

    // Should complete in <500ms for 100 notes
    // (baseline without batching is ~2-3 seconds)
    assert!(duration < Duration::from_millis(500));
}
```

**Implementation (GREEN):**

Add `upsert_notes_batch` method to `IndexRepository`:

```rust
// src/index/traits.rs
pub trait IndexRepository {
    // ... existing methods ...

    /// Batch insert multiple notes in a single transaction.
    /// More efficient than calling upsert_note repeatedly.
    fn upsert_notes_batch(
        &mut self,
        notes: &[(&Note, &ContentHash, &Path)],
    ) -> IndexResult<()>;
}
```

Implementation in `repo_impl.rs`:
```rust
fn upsert_notes_batch(
    &mut self,
    notes: &[(&Note, &ContentHash, &Path)],
) -> IndexResult<()> {
    let tx = self.transaction()?;

    // Prepare statements once, reuse for all notes
    let mut note_stmt = tx.conn().prepare_cached(
        "INSERT INTO notes (...) VALUES (...) ON CONFLICT(id) DO UPDATE SET ..."
    )?;
    let mut delete_topics_stmt = tx.conn().prepare_cached(
        "DELETE FROM note_topics WHERE note_id = ?"
    )?;
    // ... other prepared statements ...

    for (note, hash, path) in notes {
        // Use prepared statements
        note_stmt.execute(...)?;
        delete_topics_stmt.execute(...)?;
        // ... rest of upsert logic ...
    }

    tx.commit()
}
```

Update `IndexBuilder::full_rebuild_with_progress`:
```rust
pub fn full_rebuild_with_progress<P: ProgressReporter>(
    &self,
    index: &mut SqliteIndex,
    progress: &mut P,
) -> IndexResult<BuildResult> {
    index.clear()?;

    let files: Vec<PathBuf> = scan_notes_directory(&self.notes_dir)?...;

    // Parse all files first, collecting results
    let mut parsed_notes = Vec::with_capacity(files.len());
    let mut errors = Vec::new();

    for relative_path in files {
        let full_path = self.notes_dir.join(&relative_path);
        match read_note(&full_path) {
            Ok(parsed) => {
                parsed_notes.push((parsed, relative_path));
                progress.on_file(&relative_path, FileResult::Indexed);
            }
            Err(e) => {
                let build_error = fs_error_to_build_error(e, &relative_path);
                progress.on_file(&relative_path, FileResult::Error(...));
                errors.push(build_error);
            }
        }
    }

    // Batch insert all parsed notes in single transaction
    let batch: Vec<_> = parsed_notes.iter()
        .map(|(parsed, path)| (&parsed.note, &parsed.content_hash, path.as_path()))
        .collect();

    index.upsert_notes_batch(&batch)?;

    progress.on_complete(parsed_notes.len(), errors.len());
    Ok(BuildResult { indexed: parsed_notes.len(), errors })
}
```

**Refactor:** Extract prepared statement caching into helper struct.

### 2.2 Eliminate double file read in incremental update

**Current bottleneck:** Lines 236-243 in `builder.rs` read file twice:
1. `std::fs::read()` for hash computation
2. `read_note()` which reads again + parses

**Test (RED):**
```rust
#[test]
fn incremental_update_reads_each_file_once() {
    // Create a custom filesystem wrapper that counts reads
    // Or use timing to verify improvement
    let dir = create_test_notes(100);
    let builder = IndexBuilder::new(dir.path().to_path_buf());
    let mut index = SqliteIndex::open_in_memory().unwrap();
    builder.full_rebuild(&mut index).unwrap();

    // Modify half the files
    modify_notes(&dir, 50);

    let start = Instant::now();
    let result = builder.incremental_update(&mut index).unwrap();
    let duration = start.elapsed();

    assert_eq!(result.modified, 50);
    // Should be ~2x faster than current impl
    assert!(duration < Duration::from_millis(300));
}
```

**Implementation (GREEN):**

Modify `read_note` to return hash computed during read:
```rust
// src/infra/fs.rs
pub struct ParsedNoteWithHash {
    pub note: Note,
    pub content_hash: ContentHash,
    pub raw_bytes: Vec<u8>,  // If needed for hash verification
}

/// Reads and parses a note file, computing content hash in single pass.
pub fn read_note(path: &Path) -> Result<ParsedNoteWithHash, FsError> {
    let bytes = std::fs::read(path)?;
    let content_hash = ContentHash::compute(&bytes);

    // Validate encoding and parse
    let content = validate_and_decode(&bytes)?;
    let parsed = parse_with_hash(&content, content_hash.clone())?;

    Ok(ParsedNoteWithHash {
        note: parsed.note,
        content_hash,
    })
}
```

Update `incremental_update_with_progress`:
```rust
for relative_path in &current_files {
    let full_path = self.notes_dir.join(relative_path);

    // Single read that gives us both hash and parsed note
    match read_note(&full_path) {
        Ok(parsed) => {
            let current_hash = &parsed.content_hash;

            match indexed_paths.get(relative_path) {
                None => {
                    // New file - already parsed, just insert
                    index.upsert_note(&parsed.note, current_hash, relative_path)?;
                    added += 1;
                }
                Some(indexed_hash) if indexed_hash != current_hash => {
                    // Modified - already parsed, just update
                    index.upsert_note(&parsed.note, current_hash, relative_path)?;
                    modified += 1;
                }
                Some(_) => {
                    // Unchanged - skip
                    progress.on_file(relative_path, FileResult::Skipped);
                }
            }
        }
        Err(e) => { /* error handling */ }
    }
}
```

**Trade-off:** This parses unchanged files too. For very large repos with few changes, could add `read_hash_only()` function. But for typical use (incremental on save), parsing cost is negligible vs I/O.

### 2.3 Use prepared statements in upsert

**Test (RED):**
```rust
#[test]
fn upsert_uses_prepared_statements() {
    // Verify via timing or statement cache hits
    let mut index = SqliteIndex::open_in_memory().unwrap();
    let notes = generate_notes(100);

    let start = Instant::now();
    for (note, hash, path) in &notes {
        index.upsert_note(note, hash, path).unwrap();
    }
    let serial_duration = start.elapsed();

    // Batch version should be significantly faster
    let mut index2 = SqliteIndex::open_in_memory().unwrap();
    let start = Instant::now();
    index2.upsert_notes_batch(&notes).unwrap();
    let batch_duration = start.elapsed();

    assert!(batch_duration < serial_duration / 2);
}
```

**Implementation (GREEN):**

Use `prepare_cached` instead of `prepare`:
```rust
fn upsert_notes_batch(&mut self, notes: &[...]) -> IndexResult<()> {
    let tx = self.transaction()?;

    {
        // Prepare all statements once
        let mut insert_note = tx.conn().prepare_cached(
            "INSERT INTO notes ... ON CONFLICT ..."
        )?;
        let mut delete_topics = tx.conn().prepare_cached(
            "DELETE FROM note_topics WHERE note_id = ?"
        )?;
        let mut delete_tags = tx.conn().prepare_cached(
            "DELETE FROM note_tags WHERE note_id = ?"
        )?;
        let mut delete_aliases = tx.conn().prepare_cached(
            "DELETE FROM aliases WHERE note_id = ?"
        )?;
        let mut insert_topic = tx.conn().prepare_cached(
            "INSERT OR IGNORE INTO topics (path) VALUES (?)"
        )?;
        // ... etc

        for (note, hash, path) in notes {
            insert_note.execute(...)?;
            delete_topics.execute([note.id().to_string()])?;
            // ... use prepared statements
        }
    }

    tx.commit()
}
```

---

## Phase 3: Query Optimizations

### 3.1 Eliminate N+1 in get_note

**Current bottleneck:** `get_note()` executes 4 separate queries:
1. Main note data
2. Aliases
3. Topics (via JOIN)
4. Tags (via JOIN)

**Test (RED):**
```rust
#[test]
fn get_note_single_query() {
    let mut index = setup_index_with_complex_note();

    // Should complete in <5ms even with topics/tags/aliases
    let start = Instant::now();
    for _ in 0..100 {
        let _ = index.get_note(&note_id).unwrap();
    }
    let duration = start.elapsed();

    // 100 calls should complete in <100ms total (vs current ~500ms)
    assert!(duration < Duration::from_millis(100));
}
```

**Implementation (GREEN):**

Single query with JOINs and GROUP_CONCAT:
```rust
fn get_note(&self, id: &NoteId) -> IndexResult<Option<IndexedNote>> {
    let mut stmt = self.conn.prepare_cached(
        "SELECT
            n.id, n.title, n.description, n.created, n.modified,
            n.path, n.content_hash,
            GROUP_CONCAT(DISTINCT a.alias) as aliases,
            GROUP_CONCAT(DISTINCT t.path) as topics,
            GROUP_CONCAT(DISTINCT tg.name) as tags
         FROM notes n
         LEFT JOIN aliases a ON n.id = a.note_id
         LEFT JOIN note_topics nt ON n.id = nt.note_id
         LEFT JOIN topics t ON nt.topic_id = t.id
         LEFT JOIN note_tags ntg ON n.id = ntg.note_id
         LEFT JOIN tags tg ON ntg.tag_id = tg.id
         WHERE n.id = ?
         GROUP BY n.id"
    )?;

    let row = stmt.query_row([id.to_string()], |row| {
        // Parse GROUP_CONCAT results
        let aliases_str: Option<String> = row.get(7)?;
        let topics_str: Option<String> = row.get(8)?;
        let tags_str: Option<String> = row.get(9)?;

        // Split comma-separated values
        let aliases: Vec<String> = aliases_str
            .map(|s| s.split(',').map(String::from).collect())
            .unwrap_or_default();
        // ... similar for topics, tags

        Ok(IndexedNote { ... })
    });

    match row {
        Ok(note) => Ok(Some(note)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(IndexError::Database(e)),
    }
}
```

### 3.2 Single-query tag filtering in list

**Current bottleneck:** `list` with multiple tags executes N queries (one per tag) plus in-memory intersection.

**Test (RED):**
```rust
#[test]
fn list_multi_tag_single_query() {
    let index = setup_index_with_tagged_notes(1000);
    let tags = vec![
        Tag::new("rust").unwrap(),
        Tag::new("cli").unwrap(),
        Tag::new("async").unwrap(),
    ];

    let start = Instant::now();
    let notes = index.list_by_tags(&tags).unwrap();
    let duration = start.elapsed();

    // Should complete in <50ms (vs current ~200ms with N+1)
    assert!(duration < Duration::from_millis(50));
}
```

**Implementation (GREEN):**

Add new method `list_by_tags`:
```rust
// src/index/traits.rs
fn list_by_tags(&self, tags: &[Tag]) -> IndexResult<Vec<IndexedNote>>;

// src/index/sqlite/repo_impl.rs
fn list_by_tags(&self, tags: &[Tag]) -> IndexResult<Vec<IndexedNote>> {
    if tags.is_empty() {
        return self.list_all();
    }

    // Build query with HAVING COUNT = tag count (intersection semantics)
    let placeholders: Vec<_> = (1..=tags.len()).map(|i| format!("?{}", i)).collect();
    let query = format!(
        "SELECT n.id FROM notes n
         JOIN note_tags nt ON n.id = nt.note_id
         JOIN tags t ON nt.tag_id = t.id
         WHERE t.name IN ({})
         GROUP BY n.id
         HAVING COUNT(DISTINCT t.name) = ?{}",
        placeholders.join(", "),
        tags.len() + 1
    );

    let mut stmt = self.conn.prepare(&query)?;

    // Bind tag names + count
    let mut params: Vec<&dyn rusqlite::ToSql> = tags
        .iter()
        .map(|t| t.as_str() as &dyn rusqlite::ToSql)
        .collect();
    let tag_count = tags.len() as i64;
    params.push(&tag_count);

    let note_ids: Vec<NoteId> = stmt
        .query_map(rusqlite::params_from_iter(params), |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .filter_map(|id| id.parse().ok())
        .collect();

    // Batch fetch notes
    self.get_notes_batch(&note_ids)
}
```

### 3.3 Batch note fetching

**Test (RED):**
```rust
#[test]
fn get_notes_batch_efficient() {
    let index = setup_index_with_notes(100);
    let ids: Vec<NoteId> = (0..50).map(|_| random_note_id()).collect();

    let start = Instant::now();
    let notes = index.get_notes_batch(&ids).unwrap();
    let duration = start.elapsed();

    // Batch fetch should be much faster than 50 individual calls
    assert!(duration < Duration::from_millis(20));
}
```

**Implementation (GREEN):**
```rust
fn get_notes_batch(&self, ids: &[NoteId]) -> IndexResult<Vec<IndexedNote>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders: String = (1..=ids.len())
        .map(|i| format!("?{}", i))
        .collect::<Vec<_>>()
        .join(", ");

    let query = format!(
        "SELECT
            n.id, n.title, n.description, n.created, n.modified,
            n.path, n.content_hash,
            GROUP_CONCAT(DISTINCT a.alias) as aliases,
            GROUP_CONCAT(DISTINCT t.path) as topics,
            GROUP_CONCAT(DISTINCT tg.name) as tags
         FROM notes n
         LEFT JOIN aliases a ON n.id = a.note_id
         LEFT JOIN note_topics nt ON n.id = nt.note_id
         LEFT JOIN topics t ON nt.topic_id = t.id
         LEFT JOIN note_tags ntg ON n.id = ntg.note_id
         LEFT JOIN tags tg ON ntg.tag_id = tg.id
         WHERE n.id IN ({})
         GROUP BY n.id",
        placeholders
    );

    let mut stmt = self.conn.prepare(&query)?;
    let params: Vec<String> = ids.iter().map(|id| id.to_string()).collect();

    // ... execute and collect results
}
```

### 3.4 Push date filtering to SQL

**Current bottleneck:** `list` handler applies date filters in Rust after fetching all notes.

**Test (RED):**
```rust
#[test]
fn list_with_date_filter_uses_sql() {
    let index = setup_index_with_dated_notes(1000); // Notes from 2023-2024
    let since = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z").unwrap();

    let start = Instant::now();
    let notes = index.list_since(&since).unwrap();
    let duration = start.elapsed();

    // Should only fetch matching notes, not all 1000
    assert!(duration < Duration::from_millis(30));
    assert!(notes.len() < 500); // Roughly half
}
```

**Implementation (GREEN):**

Add filtered list methods:
```rust
fn list_filtered(&self, filter: &ListFilter) -> IndexResult<Vec<IndexedNote>> {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(since) = &filter.since {
        conditions.push("n.created >= ?");
        params.push(Box::new(since.to_rfc3339()));
    }
    if let Some(until) = &filter.until {
        conditions.push("n.created <= ?");
        params.push(Box::new(until.to_rfc3339()));
    }
    if let Some(topic) = &filter.topic {
        conditions.push("EXISTS (
            SELECT 1 FROM note_topics nt
            JOIN topics t ON nt.topic_id = t.id
            WHERE nt.note_id = n.id AND (t.path = ? OR t.path LIKE ?)
        )");
        params.push(Box::new(topic.to_string()));
        params.push(Box::new(format!("{}/%", topic)));
    }
    // ... handle tags similarly

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let query = format!(
        "SELECT n.id, ... FROM notes n {} ORDER BY n.modified DESC",
        where_clause
    );

    // Execute with params
}
```

---

## Phase 4: Final Verification

### 4.1 Integration benchmark

```rust
#[test]
fn meets_performance_targets() {
    // Target: <1s rebuild for 1000 notes
    let dir = create_test_notes(1000);
    let builder = IndexBuilder::new(dir.path().to_path_buf());
    let mut index = SqliteIndex::open_in_memory().unwrap();

    let start = Instant::now();
    let result = builder.full_rebuild(&mut index).unwrap();
    let rebuild_duration = start.elapsed();

    assert_eq!(result.indexed, 1000);
    assert!(
        rebuild_duration < Duration::from_secs(1),
        "Rebuild took {:?}, expected <1s",
        rebuild_duration
    );

    // Target: <100ms queries
    let start = Instant::now();
    let _ = index.search("architecture").unwrap();
    let search_duration = start.elapsed();

    assert!(
        search_duration < Duration::from_millis(100),
        "Search took {:?}, expected <100ms",
        search_duration
    );

    let topic = Topic::new("software").unwrap();
    let start = Instant::now();
    let _ = index.list_by_topic(&topic, true).unwrap();
    let list_duration = start.elapsed();

    assert!(
        list_duration < Duration::from_millis(100),
        "List took {:?}, expected <100ms",
        list_duration
    );
}
```

---

## Implementation Order

| Phase | Task | Est. Impact | Files |
|-------|------|-------------|-------|
| 1.1 | Add criterion benchmarks | Baseline | `Cargo.toml`, `benches/*.rs` |
| 2.1 | Batch transaction for full rebuild | 50-200% faster rebuild | `builder.rs`, `repo_impl.rs`, `traits.rs` |
| 2.2 | Eliminate double file read | 30-50% faster incremental | `builder.rs`, `fs.rs` |
| 2.3 | Prepared statements | 10-20% faster inserts | `repo_impl.rs` |
| 3.1 | Single-query get_note | 3-4x faster note fetch | `repo_impl.rs` |
| 3.2 | Multi-tag single query | 70% faster multi-tag list | `repo_impl.rs`, `traits.rs` |
| 3.3 | Batch note fetching | 5-10x faster batch ops | `repo_impl.rs` |
| 3.4 | SQL date filtering | 20-50% faster filtered lists | `repo_impl.rs`, `list.rs` |
| 4.1 | Final verification | Validate targets met | `benches/*.rs` |

---

## Cargo.toml Changes

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "index_benchmarks"
harness = false

[[bench]]
name = "query_benchmarks"
harness = false
```

---

## Risk Considerations

1. **GROUP_CONCAT delimiter collision** - If aliases/topics contain commas, parsing breaks. Mitigation: Use uncommon delimiter like `|` or `\x1F` (unit separator).

2. **Large batch memory** - Collecting 1000+ parsed notes before batch insert uses more memory. For very large repos, could chunk batches (e.g., 500 notes per transaction).

3. **SQLite query planner** - Complex JOINs might not use indexes efficiently. Run `EXPLAIN QUERY PLAN` and add indexes if needed.

4. **Prepared statement lifetime** - `prepare_cached` statements live in connection cache. This is fine for our single-connection model.
