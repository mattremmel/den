//! Command handlers (stubs).

use anyhow::{Context, Result, bail};
use chrono::Utc;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{
    BacklinksArgs, CheckArgs, EditArgs, IndexArgs, LinkArgs, ListArgs, NewArgs, RelsArgs,
    SearchArgs, ShowArgs, TagArgs, TagsArgs, TopicsArgs, UnlinkArgs, UntagArgs,
    config::Config,
    date_filter::DateFilter,
    output::{NoteListing, Output, OutputFormat, SearchListing},
};
use crate::domain::{Note, NoteId, Tag, Topic};
use crate::index::{
    FileResult, IndexBuilder, IndexRepository, IndexedNote, ProgressReporter, SearchResult,
    SqliteIndex,
};
use crate::infra::{generate_filename, read_note, write_note};

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

/// Result of creating a new note (for testability).
#[derive(Debug)]
pub struct NewNoteResult {
    pub note: Note,
    pub filename: String,
}

/// Creates a new note from the given arguments (pure function, no I/O).
///
/// Validates the title, topics, and tags, then constructs a Note.
/// Returns the Note and the generated filename.
///
/// # Errors
///
/// Returns an error if:
/// - The title is empty or whitespace-only
/// - Any topic is invalid
/// - Any tag is invalid
pub fn create_new_note(
    title: &str,
    description: Option<&str>,
    topic_strs: &[String],
    tag_strs: &[String],
) -> Result<NewNoteResult> {
    // Validate title
    let trimmed_title = title.trim();
    if trimmed_title.is_empty() {
        bail!("title cannot be empty");
    }

    // Parse and validate topics
    let mut topics = Vec::new();
    for topic_str in topic_strs {
        let topic = Topic::new(topic_str)
            .with_context(|| format!("invalid topic '{}': topics must contain only alphanumeric characters, hyphens, underscores, and forward slashes", topic_str))?;
        topics.push(topic);
    }

    // Parse and validate tags
    let mut tags = Vec::new();
    for tag_str in tag_strs {
        let tag = Tag::new(tag_str)
            .with_context(|| format!("invalid tag '{}': tags must contain only alphanumeric characters, hyphens, and underscores (no spaces)", tag_str))?;
        tags.push(tag);
    }

    // Generate ID and timestamps
    let id = NoteId::new();
    let now = Utc::now();

    // Build the note
    let note = Note::builder(id.clone(), trimmed_title, now, now)
        .description(description.map(|s| s.to_string()))
        .topics(topics)
        .tags(tags)
        .build()
        .with_context(|| "failed to create note")?;

    // Generate filename
    let filename = generate_filename(&id, trimmed_title);

    Ok(NewNoteResult { note, filename })
}

/// Opens a file in the user's configured editor.
fn open_in_editor(path: &Path, config: &Config) -> Result<()> {
    let editor = config.editor();

    // Parse editor command (may include args like "code --wait")
    let parts: Vec<&str> = editor.split_whitespace().collect();
    if parts.is_empty() {
        bail!("editor command is empty");
    }

    let (cmd, args) = parts.split_first().unwrap();

    let status = Command::new(cmd)
        .args(args)
        .arg(path)
        .status()
        .with_context(|| format!("failed to launch editor '{}'", editor))?;

    if !status.success() {
        bail!("editor '{}' exited with non-zero status", editor);
    }

    Ok(())
}

/// Updates the modified timestamp of a note after editing.
fn update_modified_timestamp(path: &Path) -> Result<()> {
    let parsed = read_note(path).with_context(|| "failed to read note after editing")?;

    let now = Utc::now();
    let updated_note = Note::builder(
        parsed.note.id().clone(),
        parsed.note.title(),
        parsed.note.created(),
        now,
    )
    .description(parsed.note.description().map(|s| s.to_string()))
    .topics(parsed.note.topics().to_vec())
    .aliases(parsed.note.aliases().to_vec())
    .tags(parsed.note.tags().to_vec())
    .links(parsed.note.links().to_vec())
    .build()
    .with_context(|| "failed to rebuild note")?;

    write_note(path, &updated_note, &parsed.body).with_context(|| "failed to write updated note")?;

    Ok(())
}

pub fn handle_new(args: &NewArgs, notes_dir: &Path, config: &Config) -> Result<()> {
    // Validate that the notes directory exists
    if !notes_dir.exists() {
        bail!(
            "notes directory does not exist: {}",
            notes_dir.display()
        );
    }

    // Create the note (validates inputs)
    let result = create_new_note(
        &args.title,
        args.desc.as_deref(),
        &args.topics,
        &args.tags,
    )?;

    // Construct file path
    let file_path = notes_dir.join(&result.filename);

    // Write the note file
    write_note(&file_path, &result.note, "")
        .with_context(|| format!("failed to write note to {}", file_path.display()))?;

    // Update index if it exists
    let db_path = index_db_path(notes_dir);
    if db_path.exists() && let Ok(mut index) = SqliteIndex::open(&db_path) {
        let builder = IndexBuilder::new(notes_dir.to_path_buf());
        // Ignore index errors - note was created successfully
        let _ = builder.incremental_update(&mut index);
    }

    // Print success message
    println!("Created: {} [{}]", result.note.title(), result.note.id().prefix());
    println!("  {}", file_path.display());

    // Open in editor if requested
    if args.edit {
        open_in_editor(&file_path, config)?;
        // Update modified timestamp after editing
        update_modified_timestamp(&file_path)?;
    }

    Ok(())
}

/// Result of resolving a note identifier.
#[derive(Debug)]
pub enum ResolveResult {
    /// Exactly one note matched.
    Unique(IndexedNote),
    /// Multiple notes matched (ambiguous).
    Ambiguous(Vec<IndexedNote>),
    /// No notes matched.
    NotFound,
}

/// Resolves a note identifier to a unique note.
///
/// Resolution order:
/// 1. ID prefix match (if input looks like a ULID prefix)
/// 2. Exact title match
/// 3. Alias match
///
/// Returns `Unique` if exactly one note matches across all methods,
/// `Ambiguous` if multiple notes match, or `NotFound` if no match.
pub fn resolve_note<R: IndexRepository>(index: &R, identifier: &str) -> Result<ResolveResult> {
    let identifier = identifier.trim();

    // Check if it looks like a ULID prefix (alphanumeric, typically 8+ chars)
    let looks_like_id = identifier.len() >= 4
        && identifier.chars().all(|c| c.is_ascii_alphanumeric());

    let mut candidates: Vec<IndexedNote> = Vec::new();

    // 1. Try ID prefix match if it looks like one
    if looks_like_id {
        let id_matches = index
            .find_by_id_prefix(identifier)
            .with_context(|| "failed to search by ID prefix")?;

        // If we get exactly one ID match, return it immediately
        // ID matches are the most precise
        if id_matches.len() == 1 {
            return Ok(ResolveResult::Unique(id_matches.into_iter().next().unwrap()));
        }

        candidates.extend(id_matches);
    }

    // 2. Try exact title match
    let title_matches = index
        .find_by_title(identifier)
        .with_context(|| "failed to search by title")?;
    candidates.extend(title_matches);

    // 3. Try alias match
    let alias_matches = index
        .find_by_alias(identifier)
        .with_context(|| "failed to search by alias")?;
    candidates.extend(alias_matches);

    // Deduplicate by ID
    candidates.sort_by(|a, b| a.id().to_string().cmp(&b.id().to_string()));
    candidates.dedup_by(|a, b| a.id() == b.id());

    match candidates.len() {
        0 => Ok(ResolveResult::NotFound),
        1 => Ok(ResolveResult::Unique(candidates.into_iter().next().unwrap())),
        _ => Ok(ResolveResult::Ambiguous(candidates)),
    }
}

pub fn handle_show(args: &ShowArgs, notes_dir: &Path) -> Result<()> {
    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    match resolve_note(&index, &args.note)? {
        ResolveResult::Unique(note) => {
            // Read and display the note
            let file_path = notes_dir.join(note.path());
            let parsed = read_note(&file_path)
                .with_context(|| format!("failed to read note: {}", file_path.display()))?;

            // Display frontmatter metadata
            println!("# {}", parsed.note.title());
            println!();

            if let Some(desc) = parsed.note.description() {
                println!("{}", desc);
                println!();
            }

            // Show metadata
            println!(
                "ID: {}  Created: {}  Modified: {}",
                parsed.note.id().prefix(),
                parsed.note.created().format("%Y-%m-%d"),
                parsed.note.modified().format("%Y-%m-%d")
            );

            if !parsed.note.topics().is_empty() {
                let topics: Vec<_> = parsed.note.topics().iter().map(|t| t.to_string()).collect();
                println!("Topics: {}", topics.join(", "));
            }

            if !parsed.note.tags().is_empty() {
                let tags: Vec<_> = parsed.note.tags().iter().map(|t| t.as_str()).collect();
                println!("Tags: {}", tags.join(", "));
            }

            println!();

            // Display body
            if !parsed.body.is_empty() {
                println!("{}", parsed.body);
            }

            Ok(())
        }
        ResolveResult::Ambiguous(notes) => {
            eprintln!(
                "Ambiguous: '{}' matches {} notes:",
                args.note,
                notes.len()
            );
            for note in &notes {
                eprintln!(
                    "  {} - {}",
                    &note.id().to_string()[..8],
                    note.title()
                );
            }
            eprintln!();
            eprintln!("Use the ID prefix to specify which note you mean.");
            bail!("ambiguous note identifier");
        }
        ResolveResult::NotFound => {
            bail!("note not found: '{}'", args.note);
        }
    }
}

/// Trait for launching an editor (allows mocking in tests).
trait EditorLauncher {
    fn open(&self, path: &Path) -> Result<()>;
}

/// Internal implementation that accepts a generic editor launcher.
fn handle_edit_impl<E: EditorLauncher>(
    args: &EditArgs,
    notes_dir: &Path,
    editor: &E,
) -> Result<()> {
    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    match resolve_note(&index, &args.note)? {
        ResolveResult::Unique(note) => {
            let file_path = notes_dir.join(note.path());

            editor.open(&file_path)?;
            update_modified_timestamp(&file_path)?;

            // Update index
            if let Ok(mut idx) = SqliteIndex::open(&db_path) {
                let builder = IndexBuilder::new(notes_dir.to_path_buf());
                let _ = builder.incremental_update(&mut idx);
            }

            println!("Edited: {} [{}]", note.title(), note.id().prefix());
            Ok(())
        }
        ResolveResult::Ambiguous(notes) => {
            eprintln!(
                "Ambiguous: '{}' matches {} notes:",
                args.note,
                notes.len()
            );
            for n in &notes {
                eprintln!("  {} - {}", &n.id().to_string()[..8], n.title());
            }
            eprintln!();
            eprintln!("Use the ID prefix to specify which note you mean.");
            bail!("ambiguous note identifier");
        }
        ResolveResult::NotFound => {
            bail!("note not found: '{}'", args.note);
        }
    }
}

pub fn handle_edit(args: &EditArgs, notes_dir: &Path, config: &Config) -> Result<()> {
    struct RealEditor<'a>(&'a Config);
    impl EditorLauncher for RealEditor<'_> {
        fn open(&self, path: &Path) -> Result<()> {
            open_in_editor(path, self.0)
        }
    }
    handle_edit_impl(args, notes_dir, &RealEditor(config))
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

    // ===========================================
    // create_new_note() tests
    // ===========================================

    #[test]
    fn create_new_note_generates_valid_note() {
        let result = create_new_note("Test Note", None, &[], &[]).unwrap();
        assert_eq!(result.note.title(), "Test Note");
        assert!(result.note.description().is_none());
        assert!(result.note.topics().is_empty());
        assert!(result.note.tags().is_empty());
    }

    #[test]
    fn create_new_note_sets_timestamps_to_now() {
        let before = Utc::now();
        let result = create_new_note("Test Note", None, &[], &[]).unwrap();
        let after = Utc::now();

        assert!(result.note.created() >= before);
        assert!(result.note.created() <= after);
        assert_eq!(result.note.created(), result.note.modified());
    }

    #[test]
    fn create_new_note_with_description() {
        let result =
            create_new_note("Test Note", Some("A test description"), &[], &[]).unwrap();
        assert_eq!(result.note.description(), Some("A test description"));
    }

    #[test]
    fn create_new_note_with_valid_topics() {
        let topics = vec!["software/rust".to_string(), "reference".to_string()];
        let result = create_new_note("Test Note", None, &topics, &[]).unwrap();
        assert_eq!(result.note.topics().len(), 2);
        assert_eq!(result.note.topics()[0].to_string(), "software/rust");
        assert_eq!(result.note.topics()[1].to_string(), "reference");
    }

    #[test]
    fn create_new_note_rejects_invalid_topic() {
        let topics = vec!["software@invalid".to_string()];
        let result = create_new_note("Test Note", None, &topics, &[]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid topic"));
    }

    #[test]
    fn create_new_note_normalizes_topics() {
        let topics = vec!["/software/rust/".to_string()];
        let result = create_new_note("Test Note", None, &topics, &[]).unwrap();
        assert_eq!(result.note.topics()[0].to_string(), "software/rust");
    }

    #[test]
    fn create_new_note_with_valid_tags() {
        let tags = vec!["draft".to_string(), "important".to_string()];
        let result = create_new_note("Test Note", None, &[], &tags).unwrap();
        assert_eq!(result.note.tags().len(), 2);
        assert_eq!(result.note.tags()[0].as_str(), "draft");
        assert_eq!(result.note.tags()[1].as_str(), "important");
    }

    #[test]
    fn create_new_note_rejects_invalid_tag() {
        let tags = vec!["has spaces".to_string()];
        let result = create_new_note("Test Note", None, &[], &tags);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid tag"));
    }

    #[test]
    fn create_new_note_normalizes_tags_to_lowercase() {
        let tags = vec!["DRAFT".to_string()];
        let result = create_new_note("Test Note", None, &[], &tags).unwrap();
        assert_eq!(result.note.tags()[0].as_str(), "draft");
    }

    #[test]
    fn create_new_note_returns_correct_filename() {
        let result = create_new_note("API Design", None, &[], &[]).unwrap();
        // Should be 10-char prefix + slug + .md
        assert!(result.filename.ends_with("-api-design.md"));
        assert_eq!(result.filename.len(), 10 + 1 + "api-design".len() + 3);
    }

    #[test]
    fn create_new_note_rejects_empty_title() {
        let result = create_new_note("", None, &[], &[]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("empty"));
    }

    #[test]
    fn create_new_note_rejects_whitespace_only_title() {
        let result = create_new_note("   ", None, &[], &[]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("empty"));
    }

    // ===========================================
    // handle_new() integration tests
    // ===========================================

    mod handle_new_tests {
        use super::*;
        use crate::infra::read_note;
        use tempfile::TempDir;

        fn test_config() -> Config {
            Config::default()
        }

        fn test_args(title: &str) -> NewArgs {
            NewArgs {
                title: title.to_string(),
                topics: vec![],
                tags: vec![],
                desc: None,
                edit: false,
            }
        }

        #[test]
        fn handle_new_creates_file() {
            let dir = TempDir::new().unwrap();
            let args = test_args("Test Note");
            let config = test_config();

            handle_new(&args, dir.path(), &config).unwrap();

            // Find the created file
            let files: Vec<_> = std::fs::read_dir(dir.path())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
                .collect();

            assert_eq!(files.len(), 1, "Should create exactly one .md file");
        }

        #[test]
        fn handle_new_file_has_correct_filename_format() {
            let dir = TempDir::new().unwrap();
            let args = test_args("API Design");
            let config = test_config();

            handle_new(&args, dir.path(), &config).unwrap();

            let files: Vec<_> = std::fs::read_dir(dir.path())
                .unwrap()
                .filter_map(Result::ok)
                .collect();

            let filename = files[0].file_name();
            let name = filename.to_string_lossy();
            assert!(name.ends_with("-api-design.md"));
            // First 10 chars should be ULID prefix
            assert_eq!(name.len(), 10 + 1 + "api-design".len() + 3);
        }

        #[test]
        fn handle_new_file_contains_valid_frontmatter() {
            let dir = TempDir::new().unwrap();
            let args = NewArgs {
                title: "Test Note".to_string(),
                topics: vec!["software/rust".to_string()],
                tags: vec!["draft".to_string()],
                desc: Some("A test description".to_string()),
                edit: false,
            };
            let config = test_config();

            handle_new(&args, dir.path(), &config).unwrap();

            // Find and read the created file
            let file = std::fs::read_dir(dir.path())
                .unwrap()
                .filter_map(Result::ok)
                .find(|e| e.path().extension().is_some_and(|ext| ext == "md"))
                .unwrap();

            let parsed = read_note(&file.path()).unwrap();
            assert_eq!(parsed.note.title(), "Test Note");
            assert_eq!(parsed.note.description(), Some("A test description"));
            assert_eq!(parsed.note.topics().len(), 1);
            assert_eq!(parsed.note.tags().len(), 1);
        }

        #[test]
        fn handle_new_creates_multiple_files() {
            let dir = TempDir::new().unwrap();
            let config = test_config();

            // Create two notes with different titles
            handle_new(&test_args("First Note"), dir.path(), &config).unwrap();
            handle_new(&test_args("Second Note"), dir.path(), &config).unwrap();

            // Find the created files
            let files: Vec<_> = std::fs::read_dir(dir.path())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
                .collect();

            assert_eq!(files.len(), 2, "Should create two unique files");
        }

        #[test]
        fn handle_new_fails_with_invalid_topic() {
            let dir = TempDir::new().unwrap();
            let args = NewArgs {
                title: "Test Note".to_string(),
                topics: vec!["invalid@topic".to_string()],
                tags: vec![],
                desc: None,
                edit: false,
            };
            let config = test_config();

            let result = handle_new(&args, dir.path(), &config);
            assert!(result.is_err());
        }

        #[test]
        fn handle_new_fails_with_invalid_tag() {
            let dir = TempDir::new().unwrap();
            let args = NewArgs {
                title: "Test Note".to_string(),
                topics: vec![],
                tags: vec!["has spaces".to_string()],
                desc: None,
                edit: false,
            };
            let config = test_config();

            let result = handle_new(&args, dir.path(), &config);
            assert!(result.is_err());
        }

        #[test]
        fn handle_new_fails_with_empty_title() {
            let dir = TempDir::new().unwrap();
            let args = test_args("");
            let config = test_config();

            let result = handle_new(&args, dir.path(), &config);
            assert!(result.is_err());
        }

        #[test]
        fn handle_new_fails_if_directory_doesnt_exist() {
            let args = test_args("Test Note");
            let config = test_config();

            let result = handle_new(&args, Path::new("/nonexistent/directory"), &config);
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(err.contains("does not exist"));
        }
    }

    // ===========================================
    // resolve_note tests
    // ===========================================

    mod resolve_note_tests {
        use super::*;
        use crate::domain::Note;
        use crate::index::SqliteIndex;

        fn setup_index_with_notes() -> SqliteIndex {
            let mut index = SqliteIndex::open_in_memory().unwrap();

            // Note 1: "API Design"
            let note1 = Note::builder(
                "01HQ3K5M7NXJK4QZPW8V2R6T9A".parse().unwrap(),
                "API Design",
                test_datetime(),
                test_datetime(),
            )
            .aliases(vec!["REST".to_string(), "api".to_string()])
            .build()
            .unwrap();
            let hash1 = test_content_hash();
            index
                .upsert_note(&note1, &hash1, Path::new("01HQ3K5M7N-api-design.md"))
                .unwrap();

            // Note 2: "Rust Programming"
            let note2 = Note::builder(
                "01HQ3K5M7NXJK4QZPW8V2R6T9B".parse().unwrap(),
                "Rust Programming",
                test_datetime(),
                test_datetime(),
            )
            .build()
            .unwrap();
            let hash2 = test_content_hash();
            index
                .upsert_note(&note2, &hash2, Path::new("01HQ3K5M7N-rust-programming.md"))
                .unwrap();

            // Note 3: "API Testing" (different prefix)
            let note3 = Note::builder(
                "01HQ4A2R9PXJK4QZPW8V2R6T9C".parse().unwrap(),
                "API Testing",
                test_datetime(),
                test_datetime(),
            )
            .build()
            .unwrap();
            let hash3 = test_content_hash();
            index
                .upsert_note(&note3, &hash3, Path::new("01HQ4A2R9P-api-testing.md"))
                .unwrap();

            index
        }

        #[test]
        fn resolve_by_full_id() {
            let index = setup_index_with_notes();
            let result = resolve_note(&index, "01HQ3K5M7NXJK4QZPW8V2R6T9A").unwrap();

            match result {
                ResolveResult::Unique(note) => {
                    assert_eq!(note.title(), "API Design");
                }
                _ => panic!("Expected Unique result"),
            }
        }

        #[test]
        fn resolve_by_id_prefix_unique() {
            let index = setup_index_with_notes();
            // "01HQ4A2R" only matches one note
            let result = resolve_note(&index, "01HQ4A2R").unwrap();

            match result {
                ResolveResult::Unique(note) => {
                    assert_eq!(note.title(), "API Testing");
                }
                _ => panic!("Expected Unique result"),
            }
        }

        #[test]
        fn resolve_by_id_prefix_ambiguous() {
            let index = setup_index_with_notes();
            // "01HQ3K5M7N" matches both "API Design" and "Rust Programming"
            let result = resolve_note(&index, "01HQ3K5M7N").unwrap();

            match result {
                ResolveResult::Ambiguous(notes) => {
                    assert_eq!(notes.len(), 2);
                }
                _ => panic!("Expected Ambiguous result, got {:?}", result),
            }
        }

        #[test]
        fn resolve_by_title_exact() {
            let index = setup_index_with_notes();
            let result = resolve_note(&index, "Rust Programming").unwrap();

            match result {
                ResolveResult::Unique(note) => {
                    assert_eq!(note.title(), "Rust Programming");
                }
                _ => panic!("Expected Unique result"),
            }
        }

        #[test]
        fn resolve_by_title_case_insensitive() {
            let index = setup_index_with_notes();
            let result = resolve_note(&index, "rust programming").unwrap();

            match result {
                ResolveResult::Unique(note) => {
                    assert_eq!(note.title(), "Rust Programming");
                }
                _ => panic!("Expected Unique result"),
            }
        }

        #[test]
        fn resolve_by_alias() {
            let index = setup_index_with_notes();
            let result = resolve_note(&index, "REST").unwrap();

            match result {
                ResolveResult::Unique(note) => {
                    assert_eq!(note.title(), "API Design");
                }
                _ => panic!("Expected Unique result"),
            }
        }

        #[test]
        fn resolve_by_alias_case_insensitive() {
            let index = setup_index_with_notes();
            let result = resolve_note(&index, "rest").unwrap();

            match result {
                ResolveResult::Unique(note) => {
                    assert_eq!(note.title(), "API Design");
                }
                _ => panic!("Expected Unique result"),
            }
        }

        #[test]
        fn resolve_not_found() {
            let index = setup_index_with_notes();
            let result = resolve_note(&index, "nonexistent").unwrap();

            match result {
                ResolveResult::NotFound => {}
                _ => panic!("Expected NotFound result"),
            }
        }

        #[test]
        fn resolve_whitespace_trimmed() {
            let index = setup_index_with_notes();
            let result = resolve_note(&index, "  Rust Programming  ").unwrap();

            match result {
                ResolveResult::Unique(note) => {
                    assert_eq!(note.title(), "Rust Programming");
                }
                _ => panic!("Expected Unique result"),
            }
        }

        #[test]
        fn resolve_id_prefix_takes_precedence() {
            // If an ID prefix uniquely matches, it should return immediately
            // even if there are also title/alias matches
            let index = setup_index_with_notes();
            let result = resolve_note(&index, "01HQ4A2R9P").unwrap();

            match result {
                ResolveResult::Unique(note) => {
                    assert_eq!(note.title(), "API Testing");
                }
                _ => panic!("Expected Unique result"),
            }
        }
    }

    // ===========================================
    // handle_show integration tests
    // ===========================================

    mod handle_show_tests {
        use super::*;
        use crate::index::{IndexBuilder, SqliteIndex};
        use tempfile::TempDir;

        fn setup_notes_dir() -> TempDir {
            let dir = TempDir::new().unwrap();

            // Create index directory
            std::fs::create_dir_all(dir.path().join(".index")).unwrap();

            // Create a test note
            let note_content = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9A
title: API Design
description: Notes on API design principles
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
topics:
  - software/architecture
tags:
  - draft
aliases:
  - REST
---

# API Design Principles

This is the body of the note.
"#;
            std::fs::write(
                dir.path().join("01HQ3K5M7N-api-design.md"),
                note_content,
            )
            .unwrap();

            // Build index
            let db_path = dir.path().join(".index/notes.db");
            let mut index = SqliteIndex::open(&db_path).unwrap();
            let builder = IndexBuilder::new(dir.path().to_path_buf());
            builder.full_rebuild(&mut index).unwrap();

            dir
        }

        #[test]
        fn handle_show_by_id_prefix() {
            let dir = setup_notes_dir();
            let args = ShowArgs {
                note: "01HQ3K5M".to_string(),
            };

            let result = handle_show(&args, dir.path());
            assert!(result.is_ok());
        }

        #[test]
        fn handle_show_by_title() {
            let dir = setup_notes_dir();
            let args = ShowArgs {
                note: "API Design".to_string(),
            };

            let result = handle_show(&args, dir.path());
            assert!(result.is_ok());
        }

        #[test]
        fn handle_show_by_alias() {
            let dir = setup_notes_dir();
            let args = ShowArgs {
                note: "REST".to_string(),
            };

            let result = handle_show(&args, dir.path());
            assert!(result.is_ok());
        }

        #[test]
        fn handle_show_not_found() {
            let dir = setup_notes_dir();
            let args = ShowArgs {
                note: "nonexistent".to_string(),
            };

            let result = handle_show(&args, dir.path());
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(err.contains("not found"));
        }
    }

    // ===========================================
    // handle_edit tests
    // ===========================================

    mod handle_edit_tests {
        use super::*;
        use crate::index::{IndexBuilder, SqliteIndex};
        use std::cell::RefCell;
        use tempfile::TempDir;

        /// Mock editor for testing.
        struct MockEditor {
            opened: RefCell<Option<PathBuf>>,
            should_fail: bool,
        }

        impl MockEditor {
            fn new() -> Self {
                Self {
                    opened: RefCell::new(None),
                    should_fail: false,
                }
            }

            fn failing() -> Self {
                Self {
                    opened: RefCell::new(None),
                    should_fail: true,
                }
            }

            fn opened_path(&self) -> Option<PathBuf> {
                self.opened.borrow().clone()
            }
        }

        impl EditorLauncher for MockEditor {
            fn open(&self, path: &Path) -> Result<()> {
                *self.opened.borrow_mut() = Some(path.to_path_buf());
                if self.should_fail {
                    bail!("editor failed to open");
                }
                Ok(())
            }
        }

        fn setup_notes_dir() -> TempDir {
            let dir = TempDir::new().unwrap();
            std::fs::create_dir_all(dir.path().join(".index")).unwrap();

            let note = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9A
title: API Design
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
aliases:
  - REST
---
Body content.
"#;
            std::fs::write(dir.path().join("01HQ3K5M7N-api-design.md"), note).unwrap();

            // Build index
            let db_path = dir.path().join(".index/notes.db");
            let mut index = SqliteIndex::open(&db_path).unwrap();
            IndexBuilder::new(dir.path().to_path_buf())
                .full_rebuild(&mut index)
                .unwrap();

            dir
        }

        fn setup_notes_dir_with_ambiguous() -> TempDir {
            let dir = TempDir::new().unwrap();
            std::fs::create_dir_all(dir.path().join(".index")).unwrap();

            // Note 1
            let note1 = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9A
title: API Design
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
Note 1
"#;
            std::fs::write(dir.path().join("01HQ3K5M7N-api-design.md"), note1).unwrap();

            // Note 2 with same title
            let note2 = r#"---
id: 01HQ3K5M7NXJK4QZPW8V2R6T9B
title: API Design
created: 2024-01-15T10:30:00Z
modified: 2024-01-15T10:30:00Z
---
Note 2
"#;
            std::fs::write(dir.path().join("01HQ3K5M7N-api-design-2.md"), note2).unwrap();

            // Build index
            let db_path = dir.path().join(".index/notes.db");
            let mut index = SqliteIndex::open(&db_path).unwrap();
            IndexBuilder::new(dir.path().to_path_buf())
                .full_rebuild(&mut index)
                .unwrap();

            dir
        }

        // Phase 1: Error cases

        #[test]
        fn handle_edit_not_found_returns_error() {
            let dir = setup_notes_dir();
            let args = EditArgs {
                note: "nonexistent".to_string(),
            };
            let editor = MockEditor::new();

            let result = handle_edit_impl(&args, dir.path(), &editor);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("not found"));
        }

        #[test]
        fn handle_edit_ambiguous_returns_error() {
            let dir = setup_notes_dir_with_ambiguous();
            let args = EditArgs {
                note: "API Design".to_string(),
            };
            let editor = MockEditor::new();

            let result = handle_edit_impl(&args, dir.path(), &editor);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("ambiguous"));
        }

        // Phase 2: Resolution methods

        #[test]
        fn handle_edit_by_id_prefix() {
            let dir = setup_notes_dir();
            let args = EditArgs {
                note: "01HQ3K5M".to_string(),
            };
            let editor = MockEditor::new();

            let result = handle_edit_impl(&args, dir.path(), &editor);
            assert!(result.is_ok());

            let opened = editor.opened_path().unwrap();
            assert!(opened.ends_with("01HQ3K5M7N-api-design.md"));
        }

        #[test]
        fn handle_edit_by_title() {
            let dir = setup_notes_dir();
            let args = EditArgs {
                note: "API Design".to_string(),
            };
            let editor = MockEditor::new();

            let result = handle_edit_impl(&args, dir.path(), &editor);
            assert!(result.is_ok());

            let opened = editor.opened_path().unwrap();
            assert!(opened.ends_with("01HQ3K5M7N-api-design.md"));
        }

        #[test]
        fn handle_edit_by_alias() {
            let dir = setup_notes_dir();
            let args = EditArgs {
                note: "REST".to_string(),
            };
            let editor = MockEditor::new();

            let result = handle_edit_impl(&args, dir.path(), &editor);
            assert!(result.is_ok());

            let opened = editor.opened_path().unwrap();
            assert!(opened.ends_with("01HQ3K5M7N-api-design.md"));
        }

        // Phase 3: Timestamp update

        #[test]
        fn handle_edit_updates_modified_timestamp() {
            let dir = setup_notes_dir();
            let args = EditArgs {
                note: "API Design".to_string(),
            };
            let editor = MockEditor::new();

            // Read original modified time
            let file_path = dir.path().join("01HQ3K5M7N-api-design.md");
            let before = crate::infra::read_note(&file_path).unwrap();
            let original_modified = before.note.modified();

            // Small delay to ensure timestamp differs
            std::thread::sleep(std::time::Duration::from_millis(10));

            let result = handle_edit_impl(&args, dir.path(), &editor);
            assert!(result.is_ok());

            // Read updated modified time
            let after = crate::infra::read_note(&file_path).unwrap();
            assert!(
                after.note.modified() > original_modified,
                "modified timestamp should be updated"
            );
        }

        // Phase 4: Editor failure

        #[test]
        fn handle_edit_editor_failure_returns_error() {
            let dir = setup_notes_dir();
            let args = EditArgs {
                note: "API Design".to_string(),
            };
            let editor = MockEditor::failing();

            let result = handle_edit_impl(&args, dir.path(), &editor);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("editor failed"));
        }

        #[test]
        fn handle_edit_no_timestamp_update_on_editor_failure() {
            let dir = setup_notes_dir();
            let args = EditArgs {
                note: "API Design".to_string(),
            };
            let editor = MockEditor::failing();

            // Read original modified time
            let file_path = dir.path().join("01HQ3K5M7N-api-design.md");
            let before = crate::infra::read_note(&file_path).unwrap();
            let original_modified = before.note.modified();

            let _ = handle_edit_impl(&args, dir.path(), &editor);

            // Read modified time after (should be unchanged)
            let after = crate::infra::read_note(&file_path).unwrap();
            assert_eq!(
                after.note.modified(),
                original_modified,
                "modified timestamp should NOT be updated on editor failure"
            );
        }

        // Phase 5: Index update

        #[test]
        fn handle_edit_updates_index() {
            let dir = setup_notes_dir();
            let args = EditArgs {
                note: "API Design".to_string(),
            };
            let editor = MockEditor::new();

            let result = handle_edit_impl(&args, dir.path(), &editor);
            assert!(result.is_ok());

            // Verify index was updated by checking modified time in index
            let db_path = dir.path().join(".index/notes.db");
            let index = SqliteIndex::open(&db_path).unwrap();
            let notes = index
                .find_by_title("API Design")
                .unwrap();
            assert_eq!(notes.len(), 1);

            // The index should reflect the updated modified timestamp
            let file_path = dir.path().join("01HQ3K5M7N-api-design.md");
            let file_note = crate::infra::read_note(&file_path).unwrap();
            assert_eq!(notes[0].modified(), file_note.note.modified());
        }
    }
}
