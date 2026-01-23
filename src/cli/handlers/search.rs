//! Search command handler.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;

use super::ARCHIVED_TAG;
use super::index_db_path;
use super::list::{note_matches_topic, parse_topic_filter};
use crate::cli::SearchArgs;
use crate::cli::output::{Output, OutputFormat, SearchListing};
use crate::domain::{Tag, Topic};
use crate::index::{IndexRepository, SearchResult, SqliteIndex};

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

    // 4. Exclude archived unless --include-archived
    if !args.include_archived {
        let archived_tag = Tag::new(ARCHIVED_TAG).expect("archived is a valid tag");
        results.retain(|r| !r.note().tags().contains(&archived_tag));
    }

    // 5. Format and output (results already ranked)
    format_search_output(&results, args.format, notes_dir)?;

    Ok(())
}

/// Strip HTML tags from snippet for terminal display.
pub(crate) fn strip_html_tags(s: &str) -> String {
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
                        note.id().prefix(),
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
