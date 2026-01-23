//! Archive command handlers (archive, unarchive).

use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde::Serialize;
use std::path::Path;

use super::index_db_path;
use super::resolve::{ResolveResult, print_ambiguous_notes, resolve_note};
use crate::cli::output::{Output, OutputFormat};
use crate::cli::{ArchiveArgs, UnarchiveArgs};
use crate::domain::{Note, Tag};
use crate::index::{IndexBuilder, SqliteIndex};
use crate::infra::{read_note, write_note};

/// The canonical tag used to mark archived notes.
pub const ARCHIVED_TAG: &str = "archived";

/// Result type for archive/unarchive operations.
#[derive(Debug, Serialize)]
pub struct ArchiveResult {
    pub id: String,
    pub title: String,
    pub archived: bool,
    pub path: String,
}

/// Archive a note by adding the 'archived' tag.
pub fn handle_archive(args: &ArchiveArgs, notes_dir: &Path) -> Result<()> {
    let archived_tag = Tag::new(ARCHIVED_TAG).expect("archived is a valid tag name");

    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    match resolve_note(&index, &args.note)? {
        ResolveResult::Unique(indexed_note) => {
            let file_path = notes_dir.join(indexed_note.path());
            let parsed = read_note(&file_path)
                .with_context(|| format!("failed to read note: {}", file_path.display()))?;

            // Idempotency: already archived
            if parsed.note.tags().contains(&archived_tag) {
                match args.format {
                    OutputFormat::Human => {
                        println!("'{}' is already archived", parsed.note.title());
                    }
                    OutputFormat::Json => {
                        let result = ArchiveResult {
                            id: parsed.note.id().to_string(),
                            title: parsed.note.title().to_string(),
                            archived: true,
                            path: file_path.to_string_lossy().to_string(),
                        };
                        let out = Output::new(result);
                        println!("{}", serde_json::to_string_pretty(&out)?);
                    }
                    OutputFormat::Paths => {
                        println!("{}", file_path.display());
                    }
                }
                return Ok(());
            }

            // Add archived tag
            let mut tags = parsed.note.tags().to_vec();
            tags.push(archived_tag);

            let updated_note = Note::builder(
                parsed.note.id().clone(),
                parsed.note.title(),
                parsed.note.created(),
                Utc::now(),
            )
            .description(parsed.note.description().map(|s| s.to_string()))
            .topics(parsed.note.topics().to_vec())
            .aliases(parsed.note.aliases().to_vec())
            .tags(tags)
            .links(parsed.note.links().to_vec())
            .build()
            .with_context(|| "failed to rebuild note")?;

            write_note(&file_path, &updated_note, &parsed.body)
                .with_context(|| "failed to write updated note")?;

            // Update index (ignore failures)
            if let Ok(mut idx) = SqliteIndex::open(&db_path) {
                let builder = IndexBuilder::new(notes_dir.to_path_buf());
                let _ = builder.incremental_update(&mut idx);
            }

            match args.format {
                OutputFormat::Human => {
                    println!(
                        "Archived '{}' [{}]",
                        updated_note.title(),
                        updated_note.id().prefix()
                    );
                }
                OutputFormat::Json => {
                    let result = ArchiveResult {
                        id: updated_note.id().to_string(),
                        title: updated_note.title().to_string(),
                        archived: true,
                        path: file_path.to_string_lossy().to_string(),
                    };
                    let out = Output::new(result);
                    println!("{}", serde_json::to_string_pretty(&out)?);
                }
                OutputFormat::Paths => {
                    println!("{}", file_path.display());
                }
            }
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

/// Unarchive a note by removing the 'archived' tag.
pub fn handle_unarchive(args: &UnarchiveArgs, notes_dir: &Path) -> Result<()> {
    let archived_tag = Tag::new(ARCHIVED_TAG).expect("archived is a valid tag name");

    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    match resolve_note(&index, &args.note)? {
        ResolveResult::Unique(indexed_note) => {
            let file_path = notes_dir.join(indexed_note.path());
            let parsed = read_note(&file_path)
                .with_context(|| format!("failed to read note: {}", file_path.display()))?;

            // Idempotency: not archived
            if !parsed.note.tags().contains(&archived_tag) {
                match args.format {
                    OutputFormat::Human => {
                        println!("'{}' is not archived", parsed.note.title());
                    }
                    OutputFormat::Json => {
                        let result = ArchiveResult {
                            id: parsed.note.id().to_string(),
                            title: parsed.note.title().to_string(),
                            archived: false,
                            path: file_path.to_string_lossy().to_string(),
                        };
                        let out = Output::new(result);
                        println!("{}", serde_json::to_string_pretty(&out)?);
                    }
                    OutputFormat::Paths => {
                        println!("{}", file_path.display());
                    }
                }
                return Ok(());
            }

            // Remove archived tag
            let tags: Vec<Tag> = parsed
                .note
                .tags()
                .iter()
                .filter(|t| *t != &archived_tag)
                .cloned()
                .collect();

            let updated_note = Note::builder(
                parsed.note.id().clone(),
                parsed.note.title(),
                parsed.note.created(),
                Utc::now(),
            )
            .description(parsed.note.description().map(|s| s.to_string()))
            .topics(parsed.note.topics().to_vec())
            .aliases(parsed.note.aliases().to_vec())
            .tags(tags)
            .links(parsed.note.links().to_vec())
            .build()
            .with_context(|| "failed to rebuild note")?;

            write_note(&file_path, &updated_note, &parsed.body)
                .with_context(|| "failed to write updated note")?;

            // Update index
            if let Ok(mut idx) = SqliteIndex::open(&db_path) {
                let builder = IndexBuilder::new(notes_dir.to_path_buf());
                let _ = builder.incremental_update(&mut idx);
            }

            match args.format {
                OutputFormat::Human => {
                    println!(
                        "Unarchived '{}' [{}]",
                        updated_note.title(),
                        updated_note.id().prefix()
                    );
                }
                OutputFormat::Json => {
                    let result = ArchiveResult {
                        id: updated_note.id().to_string(),
                        title: updated_note.title().to_string(),
                        archived: false,
                        path: file_path.to_string_lossy().to_string(),
                    };
                    let out = Output::new(result);
                    println!("{}", serde_json::to_string_pretty(&out)?);
                }
                OutputFormat::Paths => {
                    println!("{}", file_path.display());
                }
            }
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
