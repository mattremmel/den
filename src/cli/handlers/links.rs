//! Link-related command handlers (backlinks, link, unlink, rels).

use anyhow::{bail, Context, Result};
use std::path::Path;

use super::{index_db_path, truncate_str};
use super::resolve::{print_ambiguous_notes, resolve_note, ResolveResult};
use crate::cli::output::{NoteListing, Output, OutputFormat};
use crate::cli::{BacklinksArgs, LinkArgs, RelsArgs, UnlinkArgs};
use crate::index::{IndexRepository, SqliteIndex};

pub fn handle_backlinks(args: &BacklinksArgs, notes_dir: &Path) -> Result<()> {
    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    match resolve_note(&index, &args.note)? {
        ResolveResult::Unique(note) => {
            // Parse optional rel filter
            let rel = match &args.rel {
                Some(rel_str) => Some(crate::domain::Rel::new(rel_str).map_err(|e| {
                    anyhow::anyhow!("invalid relationship type '{}': {}", rel_str, e)
                })?),
                None => None,
            };

            let mut backlinks = index
                .backlinks(note.id(), rel.as_ref())
                .with_context(|| "failed to query backlinks")?;

            // Sort by modified date, most recent first
            backlinks.sort_by_key(|n| std::cmp::Reverse(n.modified()));

            match args.format {
                OutputFormat::Human => {
                    if backlinks.is_empty() {
                        println!("No backlinks found.");
                    } else {
                        println!("{:<10}  {:<50}  {:>10}", "ID", "Title", "Modified");
                        println!(
                            "{:<10}  {:<50}  {:>10}",
                            "----------",
                            "--------------------------------------------------",
                            "----------"
                        );

                        for backlink in &backlinks {
                            let id_short = backlink.id().prefix();
                            let title = truncate_str(backlink.title(), 50);
                            let modified = backlink.modified().format("%Y-%m-%d").to_string();
                            println!("{:<10}  {:<50}  {:>10}", id_short, title, modified);
                        }

                        println!();
                        println!("{} backlink(s)", backlinks.len());
                    }
                }
                OutputFormat::Json => {
                    let listings: Vec<NoteListing> = backlinks
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
                    for backlink in &backlinks {
                        println!("{}", notes_dir.join(backlink.path()).display());
                    }
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
