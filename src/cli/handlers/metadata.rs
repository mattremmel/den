//! Metadata command handlers (topics, tags, tag, untag).

use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::path::Path;

use super::index_db_path;
use super::resolve::{print_ambiguous_notes, resolve_note, ResolveResult};
use crate::cli::output::{Output, OutputFormat, TagListing, TopicListing};
use crate::cli::{TagArgs, TagsArgs, TopicsArgs, UntagArgs};
use crate::domain::{Note, Tag};
use crate::index::{IndexBuilder, IndexRepository, SqliteIndex};
use crate::infra::{read_note, write_note};

pub fn handle_topics(args: &TopicsArgs, notes_dir: &Path) -> Result<()> {
    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    let topics = index
        .all_topics()
        .with_context(|| "failed to list topics")?;

    match args.format {
        OutputFormat::Human => {
            if topics.is_empty() {
                println!("No topics found.");
            } else {
                for t in &topics {
                    if args.counts {
                        println!("{} ({}/{})", t.topic(), t.exact_count(), t.total_count());
                    } else {
                        println!("{}", t.topic());
                    }
                }
            }
        }
        OutputFormat::Json => {
            let listings: Vec<TopicListing> = topics
                .iter()
                .map(|t| TopicListing {
                    path: t.topic().to_string(),
                    count: if args.counts {
                        Some(t.total_count() as usize)
                    } else {
                        None
                    },
                })
                .collect();
            let out = Output::new(listings);
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        OutputFormat::Paths => {
            for t in &topics {
                println!("{}", t.topic());
            }
        }
    }
    Ok(())
}

pub fn handle_tags(args: &TagsArgs, notes_dir: &Path) -> Result<()> {
    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    let tags = index.all_tags().with_context(|| "failed to list tags")?;

    match args.format {
        OutputFormat::Human => {
            if tags.is_empty() {
                println!("No tags found.");
            } else {
                for t in &tags {
                    if args.counts {
                        println!("{} ({})", t.tag(), t.count());
                    } else {
                        println!("{}", t.tag());
                    }
                }
            }
        }
        OutputFormat::Json => {
            let listings: Vec<TagListing> = tags
                .iter()
                .map(|t| TagListing {
                    name: t.tag().to_string(),
                    count: if args.counts {
                        Some(t.count() as usize)
                    } else {
                        None
                    },
                })
                .collect();
            let out = Output::new(listings);
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        OutputFormat::Paths => {
            for t in &tags {
                println!("{}", t.tag());
            }
        }
    }
    Ok(())
}

pub fn handle_tag(args: &TagArgs, notes_dir: &Path) -> Result<()> {
    // Validate tag first (before any I/O)
    let tag =
        Tag::new(&args.tag).map_err(|e| anyhow::anyhow!("invalid tag '{}': {}", args.tag, e))?;

    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    match resolve_note(&index, &args.note)? {
        ResolveResult::Unique(indexed_note) => {
            let file_path = notes_dir.join(indexed_note.path());
            let parsed = read_note(&file_path)
                .with_context(|| format!("failed to read note: {}", file_path.display()))?;

            // Idempotency check: if tag already exists, no-op
            if parsed.note.tags().contains(&tag) {
                println!("Tag '{}' already present on '{}'", tag, parsed.note.title());
                return Ok(());
            }

            // Build updated note with new tag
            let mut tags = parsed.note.tags().to_vec();
            tags.push(tag.clone());

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
            .tags(tags)
            .links(parsed.note.links().to_vec())
            .build()
            .with_context(|| "failed to rebuild note")?;

            // Write updated note
            write_note(&file_path, &updated_note, &parsed.body)
                .with_context(|| "failed to write updated note")?;

            // Update index
            if let Ok(mut idx) = SqliteIndex::open(&db_path) {
                let builder = IndexBuilder::new(notes_dir.to_path_buf());
                let _ = builder.incremental_update(&mut idx);
            }

            println!(
                "Added tag '{}' to '{}' [{}]",
                tag,
                updated_note.title(),
                updated_note.id().prefix()
            );
            Ok(())
        }
        ResolveResult::Ambiguous(notes) => {
            print_ambiguous_notes(&args.note, &notes);
            bail!("ambiguous note identifier");
        }
        ResolveResult::NotFound => {
            bail!("note not found: '{}'", args.note);
        }
    }
}

pub fn handle_untag(args: &UntagArgs, notes_dir: &Path) -> Result<()> {
    // Validate tag first (before any I/O)
    let tag =
        Tag::new(&args.tag).map_err(|e| anyhow::anyhow!("invalid tag '{}': {}", args.tag, e))?;

    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    match resolve_note(&index, &args.note)? {
        ResolveResult::Unique(indexed_note) => {
            let file_path = notes_dir.join(indexed_note.path());
            let parsed = read_note(&file_path)
                .with_context(|| format!("failed to read note: {}", file_path.display()))?;

            // Idempotency check: if tag doesn't exist, no-op
            if !parsed.note.tags().contains(&tag) {
                println!("Tag '{}' not present on '{}'", tag, parsed.note.title());
                return Ok(());
            }

            // Build updated note without the tag
            let tags: Vec<Tag> = parsed
                .note
                .tags()
                .iter()
                .filter(|t| *t != &tag)
                .cloned()
                .collect();

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
            .tags(tags)
            .links(parsed.note.links().to_vec())
            .build()
            .with_context(|| "failed to rebuild note")?;

            // Write updated note
            write_note(&file_path, &updated_note, &parsed.body)
                .with_context(|| "failed to write updated note")?;

            // Update index
            if let Ok(mut idx) = SqliteIndex::open(&db_path) {
                let builder = IndexBuilder::new(notes_dir.to_path_buf());
                let _ = builder.incremental_update(&mut idx);
            }

            println!(
                "Removed tag '{}' from '{}' [{}]",
                tag,
                updated_note.title(),
                updated_note.id().prefix()
            );
            Ok(())
        }
        ResolveResult::Ambiguous(notes) => {
            print_ambiguous_notes(&args.note, &notes);
            bail!("ambiguous note identifier");
        }
        ResolveResult::NotFound => {
            bail!("note not found: '{}'", args.note);
        }
    }
}
