//! Completion handler for dynamic note name completion.

use std::path::Path;

use anyhow::Result;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

use crate::cli::CompleteNotesArgs;
use crate::index::{IndexRepository, IndexedNote, SqliteIndex};

use super::index_db_path;

/// A completion candidate with its score for sorting.
struct Candidate {
    id_prefix: String,
    title: String,
    score: i64,
}

/// Handle the complete-notes command.
///
/// Outputs completion candidates in zsh format: `ID_PREFIX:Title`
/// The colon separates the completion value from the description.
///
/// When prefix is empty, returns the most recently modified notes for discoverability.
pub fn handle_complete_notes(args: &CompleteNotesArgs, notes_dir: &Path) -> Result<()> {
    let db_path = index_db_path(notes_dir);
    if !db_path.exists() {
        // No index, no completions
        return Ok(());
    }

    let index = SqliteIndex::open(&db_path)?;
    let prefix = &args.prefix;
    let limit = args.limit;

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Handle empty prefix: show most recently modified notes
    if prefix.is_empty() {
        if let Ok(mut notes) = index.list_all() {
            // Sort by modified date descending (most recent first)
            notes.sort_by(|a, b| b.modified().cmp(&a.modified()));

            for note in notes.into_iter().take(limit) {
                let id_prefix = note.id().prefix();
                if seen_ids.insert(id_prefix.clone()) {
                    candidates.push(Candidate {
                        id_prefix,
                        title: note.title().to_string(),
                        score: 0, // All recent notes have equal priority
                    });
                }
            }
        }
    } else {
        // 1. ID prefix matches (highest priority)
        if prefix.chars().all(|c| c.is_ascii_alphanumeric()) {
            if let Ok(notes) = index.find_by_id_prefix(prefix) {
                for note in notes {
                    let id_prefix = note.id().prefix();
                    if seen_ids.insert(id_prefix.clone()) {
                        candidates.push(Candidate {
                            id_prefix,
                            title: note.title().to_string(),
                            score: i64::MAX, // Highest priority
                        });
                    }
                }
            }
        }

        // 2. Title prefix matches (second priority)
        if let Ok(notes) = index.find_by_title_prefix(prefix) {
            for note in notes {
                let id_prefix = note.id().prefix();
                if seen_ids.insert(id_prefix.clone()) {
                    candidates.push(Candidate {
                        id_prefix,
                        title: note.title().to_string(),
                        score: i64::MAX - 1, // High but below ID matches
                    });
                }
            }
        }

        // 3. Fuzzy matches (only if no exact matches and prefix >= 2 chars)
        if candidates.is_empty() && prefix.len() >= 2 {
            let matcher = SkimMatcherV2::default();

            if let Ok(all_notes) = index.list_all() {
                let mut fuzzy_candidates: Vec<(IndexedNote, i64)> = all_notes
                    .into_iter()
                    .filter_map(|note| {
                        matcher
                            .fuzzy_match(note.title(), prefix)
                            .map(|score| (note, score))
                    })
                    .collect();

                // Sort by score descending
                fuzzy_candidates.sort_by(|a, b| b.1.cmp(&a.1));

                for (note, score) in fuzzy_candidates {
                    let id_prefix = note.id().prefix();
                    if seen_ids.insert(id_prefix.clone()) {
                        candidates.push(Candidate {
                            id_prefix,
                            title: note.title().to_string(),
                            score,
                        });
                    }
                }
            }
        }

        // Sort: exact matches first (by score desc), then alphabetically by title
        candidates.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
    }

    // Limit and output
    for candidate in candidates.into_iter().take(limit) {
        // Format: ID_PREFIX:Title
        // The colon is the zsh separator between completion and description
        println!("{}:{}", candidate.id_prefix, candidate.title);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Note, NoteId};
    use crate::infra::ContentHash;
    use chrono::Utc;
    use tempfile::TempDir;

    fn setup_test_index() -> (TempDir, SqliteIndex) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join(".index").join("notes.db");
        std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let mut index = SqliteIndex::open(&db_path).unwrap();

        // Add some test notes
        let notes = vec![
            ("API Design Patterns", "01HQ3K5M7NXJK4QZPW8V2R6T01"),
            ("Architecture Overview", "01HQ3K5M7NXJK4QZPW8V2R6T02"),
            ("Building REST APIs", "01HQ3K5M7NXJK4QZPW8V2R6T03"),
            ("CLI Design", "01HQ3K5M7NXJK4QZPW8V2R6T04"),
            ("Data Structures", "01HQ3K5M7NXJK4QZPW8V2R6T05"),
        ];

        let now = Utc::now();
        for (title, id) in notes {
            let note_id: NoteId = id.parse().unwrap();
            let note = Note::builder(note_id.clone(), title, now, now)
                .build()
                .unwrap();
            let hash = ContentHash::compute(title.as_bytes());
            let path = dir.path().join(format!("{}.md", id));
            index.upsert_note(&note, &hash, &path).unwrap();
        }

        (dir, index)
    }

    #[test]
    fn test_title_prefix_match() {
        let (_dir, index) = setup_test_index();

        let notes = index.find_by_title_prefix("API").unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title(), "API Design Patterns");
    }

    #[test]
    fn test_title_prefix_match_case_insensitive() {
        let (_dir, index) = setup_test_index();

        let notes = index.find_by_title_prefix("api").unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title(), "API Design Patterns");
    }

    #[test]
    fn test_title_prefix_multiple_matches() {
        let (_dir, index) = setup_test_index();

        let notes = index.find_by_title_prefix("A").unwrap();
        assert_eq!(notes.len(), 2); // "API Design Patterns" and "Architecture Overview"
    }

    #[test]
    fn test_title_prefix_empty() {
        let (_dir, index) = setup_test_index();

        let notes = index.find_by_title_prefix("").unwrap();
        assert!(notes.is_empty());
    }

    #[test]
    fn test_title_prefix_no_match() {
        let (_dir, index) = setup_test_index();

        let notes = index.find_by_title_prefix("XYZ").unwrap();
        assert!(notes.is_empty());
    }
}
