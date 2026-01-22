//! Command handlers (stubs).

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{
    BacklinksArgs, CheckArgs, EditArgs, IndexArgs, LinkArgs, ListArgs, NewArgs, RelsArgs,
    SearchArgs, ShowArgs, TagArgs, TagsArgs, TopicsArgs, UnlinkArgs, UntagArgs,
    date_filter::DateFilter,
    output::{NoteListing, Output, OutputFormat, SearchListing},
};
use crate::domain::{Tag, Topic};
use crate::index::{
    FileResult, IndexBuilder, IndexRepository, IndexedNote, ProgressReporter, SearchResult,
    SqliteIndex,
};

/// Progress reporter that prints to stdout.
struct ConsoleReporter {
    verbose: bool,
}

impl ConsoleReporter {
    fn new(verbose: bool) -> Self {
        Self { verbose }
    }
}

impl ProgressReporter for ConsoleReporter {
    fn on_file(&mut self, path: &Path, result: FileResult) {
        if self.verbose {
            match result {
                FileResult::Indexed => println!("  indexed: {}", path.display()),
                FileResult::Skipped => println!("  skipped: {}", path.display()),
                FileResult::Error(msg) => eprintln!("  error: {}: {}", path.display(), msg),
            }
        }
    }

    fn on_complete(&mut self, indexed: usize, errors: usize) {
        if errors > 0 {
            eprintln!("Indexed {} notes with {} errors", indexed, errors);
        } else {
            println!("Indexed {} notes", indexed);
        }
    }
}

/// Returns the default index database path for a notes directory.
fn index_db_path(notes_dir: &Path) -> PathBuf {
    notes_dir.join(".index").join("notes.db")
}

pub fn handle_index(args: &IndexArgs, notes_dir: &Path, verbose: bool) -> Result<()> {
    let db_path = index_db_path(notes_dir);
    let mut index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    let builder = IndexBuilder::new(notes_dir.to_path_buf());
    let mut reporter = ConsoleReporter::new(verbose);

    if args.full {
        println!("Rebuilding index...");
        let result = builder
            .full_rebuild_with_progress(&mut index, &mut reporter)
            .with_context(|| "failed to rebuild index")?;

        for error in &result.errors {
            eprintln!("  {}", error);
        }
    } else {
        println!("Updating index...");
        let result = builder
            .incremental_update_with_progress(&mut index, &mut reporter)
            .with_context(|| "failed to update index")?;

        if verbose && (result.added > 0 || result.modified > 0 || result.removed > 0) {
            println!(
                "  {} added, {} modified, {} removed",
                result.added, result.modified, result.removed
            );
        }

        for error in &result.errors {
            eprintln!("  {}", error);
        }
    }

    Ok(())
}

pub fn handle_list(args: &ListArgs, notes_dir: &Path) -> Result<()> {
    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    // 1. Fetch initial set based on topic argument
    let mut notes: Vec<IndexedNote> = if let Some(topic_arg) = &args.topic {
        let (topic_str, include_descendants) = if topic_arg.ends_with('/') {
            (topic_arg.trim_end_matches('/'), true)
        } else {
            (topic_arg.as_str(), false)
        };

        let topic =
            Topic::new(topic_str).with_context(|| format!("invalid topic: {}", topic_str))?;

        index
            .list_by_topic(&topic, include_descendants)
            .with_context(|| "failed to list notes by topic")?
    } else {
        index
            .list_all()
            .with_context(|| "failed to list all notes")?
    };

    // 2. Filter by tags (AND logic)
    for tag_str in &args.tags {
        let tag = Tag::new(tag_str).with_context(|| format!("invalid tag: {}", tag_str))?;

        let notes_with_tag = index
            .list_by_tag(&tag)
            .with_context(|| format!("failed to list notes with tag: {}", tag_str))?;

        let tag_ids: HashSet<_> = notes_with_tag.iter().map(|n| n.id().clone()).collect();
        notes.retain(|n| tag_ids.contains(n.id()));
    }

    // 3. Filter by dates
    if let Some(created_str) = &args.created {
        let filter = DateFilter::parse(created_str)
            .map_err(|e| anyhow::anyhow!("invalid --created filter: {}", e))?;
        notes.retain(|n| filter.matches(n.created()));
    }

    if let Some(modified_str) = &args.modified {
        let filter = DateFilter::parse(modified_str)
            .map_err(|e| anyhow::anyhow!("invalid --modified filter: {}", e))?;
        notes.retain(|n| filter.matches(n.modified()));
    }

    // 4. Sort by modified date, most recent first
    notes.sort_by_key(|n| std::cmp::Reverse(n.modified()));

    // 5. Output based on format
    match args.format {
        OutputFormat::Human => {
            if notes.is_empty() {
                println!("No notes found.");
            } else {
                println!("{:<8}  {:<50}  {:>10}", "ID", "Title", "Modified");
                println!(
                    "{:<8}  {:<50}  {:>10}",
                    "--------", "--------------------------------------------------", "----------"
                );

                for note in &notes {
                    let id_short = &note.id().to_string()[..8];
                    let title = truncate_str(note.title(), 50);
                    let modified = note.modified().format("%Y-%m-%d").to_string();
                    println!("{:<8}  {:<50}  {:>10}", id_short, title, modified);
                }

                println!();
                println!("{} note(s)", notes.len());
            }
        }
        OutputFormat::Json => {
            let listings: Vec<NoteListing> = notes
                .iter()
                .map(|n| NoteListing {
                    id: n.id().to_string(),
                    title: n.title().to_string(),
                    path: n.path().to_string_lossy().to_string(),
                })
                .collect();
            let output = Output::new(listings);
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Paths => {
            for note in &notes {
                println!("{}", notes_dir.join(note.path()).display());
            }
        }
    }

    Ok(())
}

/// Truncates a string to fit within the given width, adding "..." if truncated.
fn truncate_str(s: &str, max_width: usize) -> String {
    if s.chars().count() <= max_width {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_width - 3).collect();
        format!("{}...", truncated)
    }
}

pub fn handle_search(args: &SearchArgs, notes_dir: &Path) -> Result<()> {
    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    // 1. Execute FTS search
    let mut results = index
        .search(&args.query)
        .with_context(|| format!("search failed for query: {}", args.query))?;

    // 2. Filter by topic (if provided)
    if let Some(topic_arg) = &args.topic {
        let (topic_str, include_descendants) = parse_topic_filter(topic_arg);
        let topic =
            Topic::new(&topic_str).with_context(|| format!("invalid topic: {}", topic_str))?;
        results.retain(|r| note_matches_topic(r.note(), &topic, include_descendants));
    }

    // 3. Filter by tags (AND logic)
    if !args.tags.is_empty() {
        let required_tags: HashSet<Tag> = args
            .tags
            .iter()
            .map(|t| Tag::new(t))
            .collect::<Result<_, _>>()
            .with_context(|| "invalid tag")?;
        results.retain(|r| {
            let note_tags: HashSet<_> = r.note().tags().iter().cloned().collect();
            required_tags.is_subset(&note_tags)
        });
    }

    // 4. Format and output (results already ranked)
    format_search_output(&results, args.format, notes_dir)?;

    Ok(())
}

/// Parse topic filter string, extracting path and whether to include descendants.
fn parse_topic_filter(s: &str) -> (String, bool) {
    if s.ends_with('/') {
        (s.trim_end_matches('/').to_string(), true)
    } else {
        (s.to_string(), false)
    }
}

/// Check if a note matches the topic filter.
fn note_matches_topic(note: &IndexedNote, topic: &Topic, include_descendants: bool) -> bool {
    let topic_path = topic.to_string();
    note.topics().iter().any(|t| {
        let t_path = t.to_string();
        if include_descendants {
            t_path == topic_path || t_path.starts_with(&format!("{}/", topic_path))
        } else {
            t_path == topic_path
        }
    })
}

/// Strip HTML tags from snippet for terminal display.
fn strip_html_tags(s: &str) -> String {
    s.replace("<b>", "").replace("</b>", "")
}

/// Format and print search results.
fn format_search_output(
    results: &[SearchResult],
    format: OutputFormat,
    notes_dir: &Path,
) -> Result<()> {
    match format {
        OutputFormat::Human => {
            if results.is_empty() {
                println!("No matching notes found.");
            } else {
                for result in results {
                    let note = result.note();
                    println!(
                        "{} {} (rank: {:.2})",
                        &note.id().to_string()[..8],
                        note.title(),
                        result.rank()
                    );
                    if let Some(snippet) = result.snippet() {
                        let clean = strip_html_tags(snippet);
                        println!("  {}", clean);
                    }
                }
                println!();
                println!("{} result(s)", results.len());
            }
        }
        OutputFormat::Json => {
            let listings: Vec<SearchListing> = results
                .iter()
                .map(|r| SearchListing {
                    id: r.note().id().to_string(),
                    title: r.note().title().to_string(),
                    path: r.note().path().to_string_lossy().to_string(),
                    rank: r.rank(),
                    snippet: r.snippet().map(|s| s.to_string()),
                })
                .collect();
            let output = Output::new(listings);
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Paths => {
            for result in results {
                println!("{}", notes_dir.join(result.note().path()).display());
            }
        }
    }
    Ok(())
}

pub fn handle_new(_args: &NewArgs) -> Result<()> {
    println!("new: not yet implemented");
    Ok(())
}

pub fn handle_show(_args: &ShowArgs) -> Result<()> {
    println!("show: not yet implemented");
    Ok(())
}

pub fn handle_edit(_args: &EditArgs) -> Result<()> {
    println!("edit: not yet implemented");
    Ok(())
}

pub fn handle_topics(_args: &TopicsArgs) -> Result<()> {
    println!("topics: not yet implemented");
    Ok(())
}

pub fn handle_tags(_args: &TagsArgs) -> Result<()> {
    println!("tags: not yet implemented");
    Ok(())
}

pub fn handle_tag(_args: &TagArgs) -> Result<()> {
    println!("tag: not yet implemented");
    Ok(())
}

pub fn handle_untag(_args: &UntagArgs) -> Result<()> {
    println!("untag: not yet implemented");
    Ok(())
}

pub fn handle_check(_args: &CheckArgs) -> Result<()> {
    println!("check: not yet implemented");
    Ok(())
}

pub fn handle_backlinks(_args: &BacklinksArgs) -> Result<()> {
    println!("backlinks: not yet implemented");
    Ok(())
}

pub fn handle_link(_args: &LinkArgs) -> Result<()> {
    println!("link: not yet implemented");
    Ok(())
}

pub fn handle_unlink(_args: &UnlinkArgs) -> Result<()> {
    println!("unlink: not yet implemented");
    Ok(())
}

pub fn handle_rels(_args: &RelsArgs) -> Result<()> {
    println!("rels: not yet implemented");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{NoteId, Tag, Topic};
    use crate::index::{IndexedNote, SearchResult};
    use crate::infra::ContentHash;
    use chrono::{DateTime, Utc};
    use std::path::PathBuf;

    // Test helpers
    fn test_note_id(suffix: &str) -> NoteId {
        format!("01HQ3K5M7NXJK4QZPW8V2R6T{}", suffix)
            .parse()
            .unwrap()
    }

    fn test_datetime() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn test_content_hash() -> ContentHash {
        ContentHash::compute(b"test content")
    }

    fn sample_indexed_note_with_topics(
        id_suffix: &str,
        title: &str,
        topics: Vec<Topic>,
    ) -> IndexedNote {
        IndexedNote::builder(
            test_note_id(id_suffix),
            title,
            test_datetime(),
            test_datetime(),
            PathBuf::from(format!("{}.md", id_suffix)),
            test_content_hash(),
        )
        .topics(topics)
        .build()
    }

    fn sample_indexed_note_with_tags(id_suffix: &str, title: &str, tags: Vec<Tag>) -> IndexedNote {
        IndexedNote::builder(
            test_note_id(id_suffix),
            title,
            test_datetime(),
            test_datetime(),
            PathBuf::from(format!("{}.md", id_suffix)),
            test_content_hash(),
        )
        .tags(tags)
        .build()
    }

    // ===========================================
    // parse_topic_filter tests
    // ===========================================

    #[test]
    fn parse_topic_filter_without_trailing_slash() {
        let (path, include_descendants) = parse_topic_filter("software/rust");
        assert_eq!(path, "software/rust");
        assert!(!include_descendants);
    }

    #[test]
    fn parse_topic_filter_with_trailing_slash() {
        let (path, include_descendants) = parse_topic_filter("software/rust/");
        assert_eq!(path, "software/rust");
        assert!(include_descendants);
    }

    #[test]
    fn parse_topic_filter_root_with_slash() {
        let (path, include_descendants) = parse_topic_filter("software/");
        assert_eq!(path, "software");
        assert!(include_descendants);
    }

    // ===========================================
    // note_matches_topic tests
    // ===========================================

    #[test]
    fn note_matches_topic_exact_match() {
        let note = sample_indexed_note_with_topics(
            "9A",
            "Rust Guide",
            vec![Topic::new("software/rust").unwrap()],
        );
        let topic = Topic::new("software/rust").unwrap();
        assert!(note_matches_topic(&note, &topic, false));
    }

    #[test]
    fn note_matches_topic_no_match() {
        let note = sample_indexed_note_with_topics(
            "9A",
            "Rust Guide",
            vec![Topic::new("software/rust").unwrap()],
        );
        let topic = Topic::new("software/python").unwrap();
        assert!(!note_matches_topic(&note, &topic, false));
    }

    #[test]
    fn note_matches_topic_descendant_match_with_flag() {
        let note = sample_indexed_note_with_topics(
            "9A",
            "Async Rust",
            vec![Topic::new("software/rust/async").unwrap()],
        );
        let topic = Topic::new("software/rust").unwrap();
        // With include_descendants=true, should match
        assert!(note_matches_topic(&note, &topic, true));
    }

    #[test]
    fn note_matches_topic_descendant_no_match_without_flag() {
        let note = sample_indexed_note_with_topics(
            "9A",
            "Async Rust",
            vec![Topic::new("software/rust/async").unwrap()],
        );
        let topic = Topic::new("software/rust").unwrap();
        // With include_descendants=false, should NOT match
        assert!(!note_matches_topic(&note, &topic, false));
    }

    #[test]
    fn note_matches_topic_parent_no_match() {
        let note = sample_indexed_note_with_topics(
            "9A",
            "Rust Guide",
            vec![Topic::new("software").unwrap()],
        );
        let topic = Topic::new("software/rust").unwrap();
        // Parent topic does not match child filter
        assert!(!note_matches_topic(&note, &topic, true));
    }

    #[test]
    fn note_matches_topic_multiple_topics() {
        let note = sample_indexed_note_with_topics(
            "9A",
            "Rust Guide",
            vec![
                Topic::new("software/rust").unwrap(),
                Topic::new("programming").unwrap(),
            ],
        );
        let topic = Topic::new("programming").unwrap();
        assert!(note_matches_topic(&note, &topic, false));
    }

    // ===========================================
    // strip_html_tags tests
    // ===========================================

    #[test]
    fn strip_html_tags_removes_bold() {
        let input = "Hello <b>world</b>!";
        assert_eq!(strip_html_tags(input), "Hello world!");
    }

    #[test]
    fn strip_html_tags_no_tags() {
        let input = "Hello world!";
        assert_eq!(strip_html_tags(input), "Hello world!");
    }

    #[test]
    fn strip_html_tags_multiple_bold() {
        let input = "<b>foo</b> and <b>bar</b>";
        assert_eq!(strip_html_tags(input), "foo and bar");
    }

    // ===========================================
    // Search filtering integration tests
    // ===========================================

    #[test]
    fn search_filters_by_topic_exact() {
        // Create search results with different topics
        let rust_note = sample_indexed_note_with_topics(
            "9A",
            "Rust Guide",
            vec![Topic::new("software/rust").unwrap()],
        );
        let python_note = sample_indexed_note_with_topics(
            "9B",
            "Python Guide",
            vec![Topic::new("software/python").unwrap()],
        );

        let results = vec![
            SearchResult::new(rust_note, 0.9),
            SearchResult::new(python_note, 0.8),
        ];

        let topic = Topic::new("software/rust").unwrap();
        let filtered: Vec<_> = results
            .into_iter()
            .filter(|r| note_matches_topic(r.note(), &topic, false))
            .collect();

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].note().title(), "Rust Guide");
    }

    #[test]
    fn search_filters_by_topic_with_descendants() {
        let rust_note = sample_indexed_note_with_topics(
            "9A",
            "Rust Guide",
            vec![Topic::new("software/rust").unwrap()],
        );
        let async_note = sample_indexed_note_with_topics(
            "9B",
            "Async Rust",
            vec![Topic::new("software/rust/async").unwrap()],
        );
        let python_note = sample_indexed_note_with_topics(
            "9C",
            "Python Guide",
            vec![Topic::new("software/python").unwrap()],
        );

        let results = vec![
            SearchResult::new(rust_note, 0.9),
            SearchResult::new(async_note, 0.8),
            SearchResult::new(python_note, 0.7),
        ];

        let topic = Topic::new("software/rust").unwrap();
        let filtered: Vec<_> = results
            .into_iter()
            .filter(|r| note_matches_topic(r.note(), &topic, true))
            .collect();

        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|r| r.note().title() == "Rust Guide"));
        assert!(filtered.iter().any(|r| r.note().title() == "Async Rust"));
    }

    #[test]
    fn search_filters_by_tags_and_logic() {
        let note1 = sample_indexed_note_with_tags(
            "9A",
            "Note with both tags",
            vec![Tag::new("draft").unwrap(), Tag::new("important").unwrap()],
        );
        let note2 = sample_indexed_note_with_tags(
            "9B",
            "Note with draft only",
            vec![Tag::new("draft").unwrap()],
        );
        let note3 = sample_indexed_note_with_tags(
            "9C",
            "Note with important only",
            vec![Tag::new("important").unwrap()],
        );

        let results = vec![
            SearchResult::new(note1, 0.9),
            SearchResult::new(note2, 0.8),
            SearchResult::new(note3, 0.7),
        ];

        let required_tags: HashSet<Tag> =
            vec![Tag::new("draft").unwrap(), Tag::new("important").unwrap()]
                .into_iter()
                .collect();

        let filtered: Vec<_> = results
            .into_iter()
            .filter(|r| {
                let note_tags: HashSet<_> = r.note().tags().iter().cloned().collect();
                required_tags.is_subset(&note_tags)
            })
            .collect();

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].note().title(), "Note with both tags");
    }

    #[test]
    fn search_preserves_rank_order_after_filtering() {
        let note1 = sample_indexed_note_with_topics(
            "9A",
            "High Rank",
            vec![Topic::new("software").unwrap()],
        );
        let note2 = sample_indexed_note_with_topics(
            "9B",
            "Medium Rank",
            vec![Topic::new("software").unwrap()],
        );
        let note3 = sample_indexed_note_with_topics(
            "9C",
            "Low Rank",
            vec![Topic::new("software").unwrap()],
        );
        let note4 = sample_indexed_note_with_topics(
            "9D",
            "Filtered Out",
            vec![Topic::new("other").unwrap()],
        );

        // Results in rank order
        let results = vec![
            SearchResult::new(note1, 0.9),
            SearchResult::new(note4, 0.85),
            SearchResult::new(note2, 0.7),
            SearchResult::new(note3, 0.5),
        ];

        let topic = Topic::new("software").unwrap();
        let filtered: Vec<_> = results
            .into_iter()
            .filter(|r| note_matches_topic(r.note(), &topic, false))
            .collect();

        assert_eq!(filtered.len(), 3);
        // Order should be preserved (highest rank first)
        assert_eq!(filtered[0].note().title(), "High Rank");
        assert_eq!(filtered[1].note().title(), "Medium Rank");
        assert_eq!(filtered[2].note().title(), "Low Rank");
    }
}
