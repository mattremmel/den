//! Command handlers (stubs).

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{
    BacklinksArgs, CheckArgs, EditArgs, IndexArgs, LinkArgs, ListArgs, NewArgs, RelsArgs,
    SearchArgs, ShowArgs, TagArgs, TagsArgs, TopicsArgs, UnlinkArgs, UntagArgs,
    date_filter::DateFilter,
    output::{NoteListing, Output, OutputFormat},
};
use crate::domain::{Tag, Topic};
use crate::index::{FileResult, IndexBuilder, IndexRepository, IndexedNote, ProgressReporter, SqliteIndex};

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

        let topic = Topic::new(topic_str)
            .with_context(|| format!("invalid topic: {}", topic_str))?;

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
        let tag = Tag::new(tag_str)
            .with_context(|| format!("invalid tag: {}", tag_str))?;

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
                println!(
                    "{:<8}  {:<50}  {:>10}",
                    "ID", "Title", "Modified"
                );
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

pub fn handle_search(_args: &SearchArgs) -> Result<()> {
    println!("search: not yet implemented");
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
