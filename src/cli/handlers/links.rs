//! Link-related command handlers (backlinks, link, unlink, rels).

use anyhow::{Context, Result, bail};
use chrono::Utc;
use std::path::Path;

use super::resolve::{ResolveResult, print_ambiguous_notes, resolve_note};
use super::{index_db_path, truncate_str};
use crate::cli::output::{NoteListing, Output, OutputFormat, RelListing};
use crate::cli::{BacklinksArgs, LinkArgs, RelsArgs, UnlinkArgs};
use crate::domain::{Link, Note, NoteId, Rel};
use crate::index::{IndexBuilder, IndexRepository, SqliteIndex};
use crate::infra::{read_note, write_note};

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

pub fn handle_link(args: &LinkArgs, notes_dir: &Path) -> Result<()> {
    // 1. Validate rels
    if args.rels.is_empty() {
        bail!("link requires at least one --rel");
    }
    let rels: Vec<Rel> = args
        .rels
        .iter()
        .map(|r| Rel::new(r).map_err(|e| anyhow::anyhow!("invalid rel '{}': {}", r, e)))
        .collect::<Result<Vec<_>>>()?;

    // 2. Open index
    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    // 3. Resolve source (must exist)
    let source_note = match resolve_note(&index, &args.source)? {
        ResolveResult::Unique(note) => note,
        ResolveResult::Ambiguous(notes) => {
            print_ambiguous_notes(&args.source, &notes);
            bail!("ambiguous source note identifier");
        }
        ResolveResult::NotFound => {
            bail!("source note not found: '{}'", args.source);
        }
    };

    // 4. Resolve target (may not exist - broken links allowed)
    let target_id: NoteId = match resolve_note(&index, &args.target)? {
        ResolveResult::Unique(note) => note.id().clone(),
        ResolveResult::Ambiguous(notes) => {
            print_ambiguous_notes(&args.target, &notes);
            bail!("ambiguous target note identifier");
        }
        ResolveResult::NotFound => args.target.parse::<NoteId>().map_err(|_| {
            anyhow::anyhow!(
                "target not found and not a valid note ID: '{}'",
                args.target
            )
        })?,
    };

    // 5. Read source note from disk
    let file_path = notes_dir.join(source_note.path());
    let parsed = read_note(&file_path)
        .with_context(|| format!("failed to read note: {}", file_path.display()))?;

    // 6. Build new link
    let new_link = match &args.note {
        Some(ctx) => Link::with_context(
            target_id.clone(),
            rels.iter().map(|r| r.as_str()).collect(),
            ctx,
        )?,
        None => Link::new(target_id.clone(), rels.iter().map(|r| r.as_str()).collect())?,
    };

    // 7. Check for existing link, merge if needed
    let (updated_links, changed) = merge_or_add_link(parsed.note.links(), &new_link);

    if !changed {
        println!(
            "Link already exists: '{}' -> {}",
            parsed.note.title(),
            target_id.prefix()
        );
        return Ok(());
    }

    // 8. Rebuild note with updated links
    let now = Utc::now();
    let updated_note = Note::builder(
        parsed.note.id().clone(),
        parsed.note.title(),
        parsed.note.created(),
        now,
    )
    .description(parsed.note.description().map(String::from))
    .topics(parsed.note.topics().to_vec())
    .aliases(parsed.note.aliases().to_vec())
    .tags(parsed.note.tags().to_vec())
    .links(updated_links)
    .build()
    .with_context(|| "failed to rebuild note")?;

    // 9. Write atomically
    write_note(&file_path, &updated_note, &parsed.body)
        .with_context(|| "failed to write updated note")?;

    // 10. Update index
    if let Ok(mut idx) = SqliteIndex::open(&db_path) {
        let builder = IndexBuilder::new(notes_dir.to_path_buf());
        let _ = builder.incremental_update(&mut idx);
    }

    println!(
        "Added link: '{}' [{}] -> [{}] ({})",
        updated_note.title(),
        updated_note.id().prefix(),
        target_id.prefix(),
        new_link
            .rel()
            .iter()
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(())
}

/// Merge new link into existing links, or add if not present.
/// Returns (updated_links, changed).
fn merge_or_add_link(existing: &[Link], new: &Link) -> (Vec<Link>, bool) {
    let mut links = existing.to_vec();

    if let Some(pos) = links.iter().position(|l| l.target() == new.target()) {
        let existing_link = &links[pos];

        // Check if rels need merging
        let mut merged_rels: Vec<Rel> = existing_link.rel().to_vec();
        let mut added_rels = false;
        for rel in new.rel() {
            if !merged_rels.contains(rel) {
                merged_rels.push(rel.clone());
                added_rels = true;
            }
        }

        // Check if context changed
        let context_changed = new.context().is_some() && new.context() != existing_link.context();

        if !added_rels && !context_changed {
            return (links, false); // No change
        }

        // Build merged link
        let context = new.context().or(existing_link.context());
        let merged = match context {
            Some(ctx) => Link::with_context(
                new.target().clone(),
                merged_rels.iter().map(|r| r.as_str()).collect(),
                ctx,
            )
            .unwrap(),
            None => Link::new(
                new.target().clone(),
                merged_rels.iter().map(|r| r.as_str()).collect(),
            )
            .unwrap(),
        };
        links[pos] = merged;
        (links, true)
    } else {
        links.push(new.clone());
        (links, true)
    }
}

/// Remove a link to a specific target from existing links.
/// Returns (updated_links, changed) where changed is true if a link was removed.
fn remove_link(existing: &[Link], target_id: &NoteId) -> (Vec<Link>, bool) {
    let mut links = existing.to_vec();

    if let Some(pos) = links.iter().position(|l| l.target() == target_id) {
        links.remove(pos);
        (links, true)
    } else {
        (links, false)
    }
}

pub fn handle_unlink(args: &UnlinkArgs, notes_dir: &Path) -> Result<()> {
    // 1. Open index
    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    // 2. Resolve source note (must exist)
    let source_note = match resolve_note(&index, &args.source)? {
        ResolveResult::Unique(note) => note,
        ResolveResult::Ambiguous(notes) => {
            print_ambiguous_notes(&args.source, &notes);
            bail!("ambiguous source note identifier");
        }
        ResolveResult::NotFound => {
            bail!("source note not found: '{}'", args.source);
        }
    };

    // 3. Resolve target (allow ID-only for broken links)
    let target_id: NoteId = match resolve_note(&index, &args.target)? {
        ResolveResult::Unique(note) => note.id().clone(),
        ResolveResult::Ambiguous(notes) => {
            print_ambiguous_notes(&args.target, &notes);
            bail!("ambiguous target note identifier");
        }
        ResolveResult::NotFound => args.target.parse::<NoteId>().map_err(|_| {
            anyhow::anyhow!(
                "target note not found and not a valid note ID: '{}'",
                args.target
            )
        })?,
    };

    // 4. Read source file
    let file_path = notes_dir.join(source_note.path());
    let parsed = read_note(&file_path)
        .with_context(|| format!("failed to read note: {}", file_path.display()))?;

    // 5. Remove link
    let (updated_links, changed) = remove_link(parsed.note.links(), &target_id);

    if !changed {
        println!(
            "No link found: '{}' [{}] -> [{}]",
            parsed.note.title(),
            parsed.note.id().prefix(),
            target_id.prefix()
        );
        return Ok(());
    }

    // 6. Rebuild note with updated links
    let now = Utc::now();
    let updated_note = Note::builder(
        parsed.note.id().clone(),
        parsed.note.title(),
        parsed.note.created(),
        now,
    )
    .description(parsed.note.description().map(String::from))
    .topics(parsed.note.topics().to_vec())
    .aliases(parsed.note.aliases().to_vec())
    .tags(parsed.note.tags().to_vec())
    .links(updated_links)
    .build()
    .with_context(|| "failed to rebuild note")?;

    // 7. Write atomically
    write_note(&file_path, &updated_note, &parsed.body)
        .with_context(|| "failed to write updated note")?;

    // 8. Update index
    if let Ok(mut idx) = SqliteIndex::open(&db_path) {
        let builder = IndexBuilder::new(notes_dir.to_path_buf());
        let _ = builder.incremental_update(&mut idx);
    }

    // 9. Print success
    println!(
        "Removed link: '{}' [{}] -> [{}]",
        updated_note.title(),
        updated_note.id().prefix(),
        target_id.prefix()
    );

    Ok(())
}

pub fn handle_rels(args: &RelsArgs, notes_dir: &Path) -> Result<()> {
    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    let rels = index.all_rels().with_context(|| "failed to list rels")?;

    match args.format {
        OutputFormat::Human => {
            if rels.is_empty() {
                println!("No relationship types found.");
            } else {
                for r in &rels {
                    if args.counts {
                        println!("{} ({})", r.rel(), r.count());
                    } else {
                        println!("{}", r.rel());
                    }
                }
            }
        }
        OutputFormat::Json => {
            let listings: Vec<RelListing> = rels
                .iter()
                .map(|r| RelListing {
                    name: r.rel().to_string(),
                    count: if args.counts {
                        Some(r.count() as usize)
                    } else {
                        None
                    },
                })
                .collect();
            let out = Output::new(listings);
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        OutputFormat::Paths => {
            for r in &rels {
                println!("{}", r.rel());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod remove_link_tests {
    use super::*;

    fn test_note_id(suffix: &str) -> NoteId {
        format!("01HQ3K5M7NXJK4QZPW{}", suffix).parse().unwrap()
    }

    #[test]
    fn remove_link_finds_and_removes_matching_target() {
        let target_id = test_note_id("8V2R6TAA");
        let links = vec![Link::new(target_id.clone(), vec!["see-also"]).unwrap()];

        let (result, changed) = remove_link(&links, &target_id);

        assert!(changed);
        assert!(result.is_empty());
    }

    #[test]
    fn remove_link_returns_false_when_target_not_found() {
        let existing_id = test_note_id("8V2R6TBB");
        let missing_id = test_note_id("8V2R6TCC");
        let links = vec![Link::new(existing_id, vec!["parent"]).unwrap()];

        let (result, changed) = remove_link(&links, &missing_id);

        assert!(!changed);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn remove_link_preserves_other_links() {
        let target_id = test_note_id("8V2R6TDD");
        let other_id = test_note_id("8V2R6TEE");
        let links = vec![
            Link::new(target_id.clone(), vec!["see-also"]).unwrap(),
            Link::new(other_id.clone(), vec!["parent"]).unwrap(),
        ];

        let (result, changed) = remove_link(&links, &target_id);

        assert!(changed);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].target(), &other_id);
    }

    #[test]
    fn remove_link_handles_empty_links() {
        let target_id = test_note_id("8V2R6TFF");
        let links: Vec<Link> = vec![];

        let (result, changed) = remove_link(&links, &target_id);

        assert!(!changed);
        assert!(result.is_empty());
    }
}
