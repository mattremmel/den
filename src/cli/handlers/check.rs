//! Check command handler.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::cli::CheckArgs;
use crate::domain::{Note, NoteId, Severity, ValidationIssue, ValidationKind, validate_notes};
use crate::infra::{FsError, read_note, scan_notes_directory, write_note};

pub fn handle_check(args: &CheckArgs, notes_dir: &Path) -> Result<()> {
    // 1. Scan directory for notes
    let paths: Vec<_> = scan_notes_directory(notes_dir)?.collect();
    if paths.is_empty() {
        println!("No notes found.");
        return Ok(());
    }

    // 2. Load notes, collecting parse errors
    let mut notes = Vec::new();
    let mut parse_issues = Vec::new();
    for path in &paths {
        let full_path = notes_dir.join(path);
        match read_note(&full_path) {
            Ok(parsed) => notes.push((path.clone(), parsed.note)),
            Err(FsError::Parse { source, .. }) => {
                parse_issues.push(ValidationIssue::parse_error(path, source));
            }
            Err(e) => {
                parse_issues.push(ValidationIssue::new(
                    path,
                    ValidationKind::ParseError(e.to_string()),
                ));
            }
        }
    }

    // 3. Validate the successfully loaded notes
    let note_refs: Vec<_> = notes.iter().map(|(p, n)| (p.clone(), n)).collect();
    let mut summary = validate_notes(&note_refs);

    // Add parse errors to the summary
    for issue in parse_issues {
        summary.add(issue);
    }

    // 4. If --fix is set, attempt to fix broken links
    let mut fixed_count = 0;
    if args.fix {
        fixed_count = fix_broken_links(&summary, notes_dir)?;
        if fixed_count > 0 {
            // Remove fixed broken link issues from summary
            summary.issues.retain(|issue| !issue.is_broken_link());
        }
    }

    // 5. Display results
    if summary.is_ok() {
        if fixed_count > 0 {
            println!("Fixed {} broken link(s). All notes OK.", fixed_count);
        } else {
            println!("All notes OK.");
        }
        return Ok(());
    }

    for issue in summary.issues_by_severity() {
        let prefix = match issue.severity() {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        println!("{}: {}", prefix, issue);
    }

    if fixed_count > 0 {
        println!("\nFixed {} broken link(s).", fixed_count);
    }

    println!(
        "\nFound {} issue(s): {} error(s), {} warning(s)",
        summary.total(),
        summary.error_count(),
        summary.warning_count()
    );

    // 6. Exit code: fail only if there are errors
    if summary.has_errors() {
        bail!("check failed");
    }
    Ok(())
}

/// Fixes broken links by removing them from affected notes.
///
/// Returns the number of broken links that were fixed.
fn fix_broken_links(summary: &crate::domain::ValidationSummary, notes_dir: &Path) -> Result<usize> {
    // Group broken links by file path
    let mut broken_by_file: HashMap<&PathBuf, HashSet<&NoteId>> = HashMap::new();
    for issue in summary.broken_links() {
        if let ValidationKind::BrokenLink { target_id } = &issue.kind {
            broken_by_file
                .entry(&issue.path)
                .or_default()
                .insert(target_id);
        }
    }

    if broken_by_file.is_empty() {
        return Ok(0);
    }

    let mut total_fixed = 0;

    // Fix each affected file
    for (rel_path, broken_targets) in broken_by_file {
        let full_path = notes_dir.join(rel_path);

        // Re-read the file to get the body
        let parsed = read_note(&full_path)?;
        let note = &parsed.note;
        let body = &parsed.body;

        // Filter out broken links
        let fixed_links: Vec<_> = note
            .links()
            .iter()
            .filter(|link| !broken_targets.contains(link.target()))
            .cloned()
            .collect();

        let links_removed = note.links().len() - fixed_links.len();
        if links_removed == 0 {
            continue;
        }

        // Build a new note with the fixed links
        let fixed_note = rebuild_note_with_links(note, fixed_links)?;

        // Write the fixed note back
        write_note(&full_path, &fixed_note, body)?;

        total_fixed += links_removed;
    }

    Ok(total_fixed)
}

/// Creates a new Note with the same fields as the original but with different links.
fn rebuild_note_with_links(note: &Note, links: Vec<crate::domain::Link>) -> Result<Note> {
    let fixed_note = Note::builder(
        note.id().clone(),
        note.title(),
        note.created(),
        note.modified(),
    )
    .description(note.description().map(|s| s.to_string()))
    .topics(note.topics().to_vec())
    .aliases(note.aliases().to_vec())
    .tags(note.tags().to_vec())
    .links(links)
    .build()
    .map_err(|e| anyhow::anyhow!("Failed to rebuild note: {}", e))?;

    Ok(fixed_note)
}
