//! List command handler.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;

use super::{index_db_path, truncate_str};
use crate::cli::ListArgs;
use crate::cli::date_filter::DateFilter;
use crate::cli::output::{NoteListing, Output, OutputFormat};
use crate::domain::{Tag, Topic};
use crate::index::{IndexRepository, IndexedNote, SqliteIndex};

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
                println!("{:<10}  {:<50}  {:>10}", "ID", "Title", "Modified");
                println!(
                    "{:<10}  {:<50}  {:>10}",
                    "----------",
                    "--------------------------------------------------",
                    "----------"
                );

                for note in &notes {
                    let id_short = note.id().prefix();
                    let title = truncate_str(note.title(), 50);
                    let modified = note.modified().format("%Y-%m-%d").to_string();
                    println!("{:<10}  {:<50}  {:>10}", id_short, title, modified);
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

/// Parse topic filter string, extracting path and whether to include descendants.
pub(crate) fn parse_topic_filter(s: &str) -> (String, bool) {
    if s.ends_with('/') {
        (s.trim_end_matches('/').to_string(), true)
    } else {
        (s.to_string(), false)
    }
}

/// Check if a note matches the topic filter.
pub(crate) fn note_matches_topic(
    note: &IndexedNote,
    topic: &Topic,
    include_descendants: bool,
) -> bool {
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
