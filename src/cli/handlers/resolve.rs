//! Note resolution utilities.

use anyhow::{Context, Result};

use crate::index::{IndexRepository, IndexedNote};

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

/// Prints detailed information about ambiguous notes to help distinguish them.
pub(crate) fn print_ambiguous_notes(identifier: &str, notes: &[IndexedNote]) {
    eprintln!("Ambiguous: '{}' matches {} notes:", identifier, notes.len());
    for note in notes {
        eprintln!("  {} - {}", note.id().prefix(), note.title());

        // Print description if present
        if let Some(desc) = note.description() {
            eprintln!("      {}", desc);
        }

        // Print aliases if present
        if !note.aliases().is_empty() {
            eprintln!("      aliases: {}", note.aliases().join(", "));
        }

        // Print tags if present
        if !note.tags().is_empty() {
            let tags: Vec<_> = note.tags().iter().map(|t| t.as_str()).collect();
            eprintln!("      tags: {}", tags.join(", "));
        }
    }
    eprintln!();
    eprintln!("Use the ID prefix to specify which note you mean.");
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
    let looks_like_id =
        identifier.len() >= 4 && identifier.chars().all(|c| c.is_ascii_alphanumeric());

    let mut candidates: Vec<IndexedNote> = Vec::new();

    // 1. Try ID prefix match if it looks like one
    if looks_like_id {
        let id_matches = index
            .find_by_id_prefix(identifier)
            .with_context(|| "failed to search by ID prefix")?;

        // If we get exactly one ID match, return it immediately
        // ID matches are the most precise
        if id_matches.len() == 1 {
            return Ok(ResolveResult::Unique(
                id_matches.into_iter().next().unwrap(),
            ));
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
    candidates.sort_by_key(|a| a.id().to_string());
    candidates.dedup_by(|a, b| a.id() == b.id());

    match candidates.len() {
        0 => Ok(ResolveResult::NotFound),
        1 => Ok(ResolveResult::Unique(
            candidates.into_iter().next().unwrap(),
        )),
        _ => Ok(ResolveResult::Ambiguous(candidates)),
    }
}
