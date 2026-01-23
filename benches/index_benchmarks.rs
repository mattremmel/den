//! Benchmarks for index operations.
//!
//! Run with: cargo bench --bench index_benchmarks

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use den::domain::{NoteId, Tag, Topic};
use den::index::{IndexBuilder, IndexRepository, SqliteIndex};
use std::fs;
use tempfile::TempDir;

// =============================================================================
// Test Data Generation
// =============================================================================

/// Topics to randomly assign to notes
const TOPICS: &[&str] = &[
    "software/architecture",
    "software/patterns",
    "software/testing",
    "projects/alpha",
    "projects/beta",
    "reference",
    "personal/journal",
    "personal/ideas",
];

/// Tags to randomly assign to notes
const TAGS: &[&str] = &[
    "draft",
    "review",
    "published",
    "important",
    "rust",
    "cli",
    "async",
    "database",
];

/// Sample words for generating realistic note content
const WORDS: &[&str] = &[
    "architecture",
    "design",
    "pattern",
    "system",
    "component",
    "interface",
    "module",
    "function",
    "method",
    "class",
    "struct",
    "implementation",
    "abstraction",
    "dependency",
    "injection",
    "testing",
    "integration",
    "unit",
    "performance",
    "optimization",
];

/// Generate a deterministic note ID from an index
fn note_id_from_index(i: usize) -> NoteId {
    // Use a fixed base and add the index to get deterministic but unique IDs
    let base_ms: u64 = 1704067200000; // 2024-01-01T00:00:00Z in milliseconds
    let timestamp_ms = base_ms + (i as u64 * 1000);
    NoteId::from_timestamp_ms(timestamp_ms)
}

/// Generate frontmatter content for a note
fn generate_note_content(index: usize) -> String {
    let id = note_id_from_index(index);
    let title = format!("Note {} - {}", index, WORDS[index % WORDS.len()]);

    // Deterministically select topics and tags based on index
    let topic1 = TOPICS[index % TOPICS.len()];
    let topic2 = TOPICS[(index + 3) % TOPICS.len()];
    let tag1 = TAGS[index % TAGS.len()];
    let tag2 = TAGS[(index + 2) % TAGS.len()];

    // Generate body content
    let body_words: Vec<&str> = (0..50)
        .map(|j| WORDS[(index + j) % WORDS.len()])
        .collect();
    let body = body_words.join(" ");

    format!(
        r#"---
id: {}
title: {}
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
description: A note about {} and related concepts
topics:
  - {}
  - {}
aliases:
  - Alias for note {}
tags:
  - {}
  - {}
---

# {}

{}

## Section 1

More content about {} and its applications in software development.

## Section 2

Discussion of {} patterns and best practices.
"#,
        id,
        title,
        WORDS[index % WORDS.len()],
        topic1,
        topic2,
        index,
        tag1,
        tag2,
        title,
        body,
        WORDS[(index + 1) % WORDS.len()],
        WORDS[(index + 2) % WORDS.len()],
    )
}

/// Create a temporary directory with N note files
fn create_test_notes(count: usize) -> TempDir {
    let dir = TempDir::new().expect("Failed to create temp dir");

    for i in 0..count {
        let id = note_id_from_index(i);
        let filename = format!("{}-note-{}.md", id.prefix(), i);
        let content = generate_note_content(i);
        fs::write(dir.path().join(&filename), content).expect("Failed to write note");
    }

    dir
}

/// Set up an index with N notes already indexed
fn setup_index_with_notes(count: usize) -> (SqliteIndex, TempDir) {
    let dir = create_test_notes(count);
    let mut index = SqliteIndex::open_in_memory().expect("Failed to open index");
    let builder = IndexBuilder::new(dir.path().to_path_buf());
    builder.full_rebuild(&mut index).expect("Failed to rebuild");
    (index, dir)
}

// =============================================================================
// Index Rebuild Benchmarks
// =============================================================================

fn bench_full_rebuild(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_rebuild");

    for size in [100, 500, 1000] {
        // Create test data once, outside the benchmark
        let dir = create_test_notes(size);

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("notes", size), &size, |b, _| {
            let builder = IndexBuilder::new(dir.path().to_path_buf());
            b.iter(|| {
                let mut index = SqliteIndex::open_in_memory().unwrap();
                builder.full_rebuild(&mut index).unwrap()
            });
        });
    }

    group.finish();
}

fn bench_incremental_update_no_changes(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_no_changes");

    for size in [100, 500, 1000] {
        let dir = create_test_notes(size);
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let builder = IndexBuilder::new(dir.path().to_path_buf());
        builder.full_rebuild(&mut index).unwrap();

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("notes", size), &size, |b, _| {
            b.iter(|| builder.incremental_update(&mut index).unwrap());
        });
    }

    group.finish();
}

fn bench_incremental_update_with_changes(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_with_changes");

    for size in [100, 500, 1000] {
        let dir = create_test_notes(size);
        let mut index = SqliteIndex::open_in_memory().unwrap();
        let builder = IndexBuilder::new(dir.path().to_path_buf());
        builder.full_rebuild(&mut index).unwrap();

        // Modify 10% of files before each iteration
        let files_to_modify = size / 10;

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("notes", size), &size, |b, _| {
            b.iter_batched(
                || {
                    // Setup: modify some files
                    for i in 0..files_to_modify {
                        let id = note_id_from_index(i);
                        let filename = format!("{}-note-{}.md", id.prefix(), i);
                        let mut content = generate_note_content(i);
                        content.push_str("\n\nModified content for benchmark.");
                        fs::write(dir.path().join(&filename), content).unwrap();
                    }
                },
                |_| {
                    // Benchmark: incremental update
                    builder.incremental_update(&mut index).unwrap()
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

// =============================================================================
// Query Benchmarks
// =============================================================================

fn bench_search(c: &mut Criterion) {
    let (index, _dir) = setup_index_with_notes(1000);

    let mut group = c.benchmark_group("search");

    group.bench_function("simple_term", |b| {
        b.iter(|| index.search("architecture").unwrap())
    });

    group.bench_function("common_term", |b| {
        b.iter(|| index.search("software").unwrap())
    });

    group.bench_function("phrase", |b| {
        b.iter(|| index.search("\"software architecture\"").unwrap())
    });

    group.bench_function("prefix", |b| {
        b.iter(|| index.search("optim*").unwrap())
    });

    group.finish();
}

fn bench_list_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_all");

    for size in [100, 500, 1000] {
        let (index, _dir) = setup_index_with_notes(size);

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("notes", size), &size, |b, _| {
            b.iter(|| index.list_all().unwrap());
        });
    }

    group.finish();
}

fn bench_list_by_topic(c: &mut Criterion) {
    let (index, _dir) = setup_index_with_notes(1000);

    let mut group = c.benchmark_group("list_by_topic");

    let topic = Topic::new("software").unwrap();
    group.bench_function("with_descendants", |b| {
        b.iter(|| index.list_by_topic(&topic, true).unwrap())
    });

    group.bench_function("exact_match", |b| {
        b.iter(|| index.list_by_topic(&topic, false).unwrap())
    });

    let deep_topic = Topic::new("software/architecture").unwrap();
    group.bench_function("deep_topic", |b| {
        b.iter(|| index.list_by_topic(&deep_topic, true).unwrap())
    });

    group.finish();
}

fn bench_list_by_tag(c: &mut Criterion) {
    let (index, _dir) = setup_index_with_notes(1000);

    let mut group = c.benchmark_group("list_by_tag");

    let tag = Tag::new("rust").unwrap();
    group.bench_function("single_tag", |b| {
        b.iter(|| index.list_by_tag(&tag).unwrap())
    });

    // Simulate multi-tag filtering (current N+1 pattern)
    let tag1 = Tag::new("rust").unwrap();
    let tag2 = Tag::new("cli").unwrap();
    group.bench_function("multi_tag_intersection", |b| {
        b.iter(|| {
            let notes1 = index.list_by_tag(&tag1).unwrap();
            let notes2 = index.list_by_tag(&tag2).unwrap();
            // Intersection
            let ids2: std::collections::HashSet<_> =
                notes2.iter().map(|n| n.id().clone()).collect();
            let _result: Vec<_> = notes1
                .into_iter()
                .filter(|n| ids2.contains(n.id()))
                .collect();
        })
    });

    group.finish();
}

fn bench_get_note(c: &mut Criterion) {
    let (index, _dir) = setup_index_with_notes(1000);

    // Get some note IDs to look up
    let note_ids: Vec<NoteId> = (0..100).map(note_id_from_index).collect();

    let mut group = c.benchmark_group("get_note");

    group.bench_function("single_lookup", |b| {
        let id = &note_ids[0];
        b.iter(|| index.get_note(id).unwrap())
    });

    group.bench_function("100_lookups", |b| {
        b.iter(|| {
            for id in &note_ids {
                let _ = index.get_note(id).unwrap();
            }
        })
    });

    group.finish();
}

fn bench_all_topics(c: &mut Criterion) {
    let (index, _dir) = setup_index_with_notes(1000);

    c.bench_function("all_topics", |b| {
        b.iter(|| index.all_topics().unwrap())
    });
}

fn bench_all_tags(c: &mut Criterion) {
    let (index, _dir) = setup_index_with_notes(1000);

    c.bench_function("all_tags", |b| {
        b.iter(|| index.all_tags().unwrap())
    });
}

// =============================================================================
// Criterion Groups
// =============================================================================

criterion_group!(
    index_benches,
    bench_full_rebuild,
    bench_incremental_update_no_changes,
    bench_incremental_update_with_changes,
);

criterion_group!(
    query_benches,
    bench_search,
    bench_list_all,
    bench_list_by_topic,
    bench_list_by_tag,
    bench_get_note,
    bench_all_topics,
    bench_all_tags,
);

criterion_main!(index_benches, query_benches);
