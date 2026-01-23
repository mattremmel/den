//! Validation functions for notes collections.
//!
//! This module provides pure functions that validate collections of notes,
//! detecting issues like duplicate IDs, broken links, and orphaned notes.
//! All functions are designed to be testable in isolation without I/O.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::domain::{Note, NoteId, ValidationIssue, ValidationSummary};

/// Validates a collection of notes for duplicate IDs.
///
/// Returns issues for any note whose ID was already seen in the collection.
/// The first occurrence of each ID is considered the "original", and subsequent
/// occurrences are reported as duplicates.
///
/// # Arguments
///
/// * `notes` - A slice of (path, note) pairs to validate
///
/// # Returns
///
/// A vector of `ValidationIssue` for each duplicate found, referencing the
/// path of the duplicate file and the path of the first occurrence.
pub fn find_duplicate_ids(notes: &[(PathBuf, &Note)]) -> Vec<ValidationIssue> {
    let mut seen: HashMap<&NoteId, &PathBuf> = HashMap::new();
    let mut issues = Vec::new();

    for (path, note) in notes {
        let id = note.id();
        if let Some(first_path) = seen.get(id) {
            issues.push(ValidationIssue::duplicate_id(
                path.clone(),
                id.clone(),
                (*first_path).clone(),
            ));
        } else {
            seen.insert(id, path);
        }
    }

    issues
}

/// Validates links in notes against a set of known IDs.
///
/// Returns issues for any link whose target is not in the known_ids set.
///
/// # Arguments
///
/// * `notes` - A slice of (path, note) pairs to validate
/// * `known_ids` - Set of valid note IDs that links can reference
///
/// # Returns
///
/// A vector of `ValidationIssue` for each broken link found.
pub fn find_broken_links(
    notes: &[(PathBuf, &Note)],
    known_ids: &HashSet<NoteId>,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    for (path, note) in notes {
        for link in note.links() {
            if !known_ids.contains(link.target()) {
                issues.push(ValidationIssue::broken_link(
                    path.clone(),
                    link.target().clone(),
                ));
            }
        }
    }

    issues
}

/// Finds notes with no topics (orphaned in the virtual folder hierarchy).
///
/// # Arguments
///
/// * `notes` - A slice of (path, note) pairs to validate
///
/// # Returns
///
/// A vector of `ValidationIssue` (with Warning severity) for each orphaned note.
pub fn find_orphaned_notes(notes: &[(PathBuf, &Note)]) -> Vec<ValidationIssue> {
    notes
        .iter()
        .filter(|(_, note)| note.topics().is_empty())
        .map(|(path, _)| ValidationIssue::orphaned(path.clone()))
        .collect()
}

/// Runs all structural validations on a collection of notes.
///
/// Combines duplicate ID, broken link, and orphan checks into a single summary.
/// The known_ids set for broken link detection is built from the input collection.
///
/// # Arguments
///
/// * `notes` - A slice of (path, note) pairs to validate
///
/// # Returns
///
/// A `ValidationSummary` containing all issues found.
pub fn validate_notes(notes: &[(PathBuf, &Note)]) -> ValidationSummary {
    let mut summary = ValidationSummary::new();

    // Collect all known IDs from the input
    let known_ids: HashSet<NoteId> = notes.iter().map(|(_, note)| note.id().clone()).collect();

    // Run all validation checks
    for issue in find_duplicate_ids(notes) {
        summary.add(issue);
    }
    for issue in find_broken_links(notes, &known_ids) {
        summary.add(issue);
    }
    for issue in find_orphaned_notes(notes) {
        summary.add(issue);
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Link, NoteId, Severity, Topic, ValidationKind};
    use chrono::{DateTime, Utc};

    // ===========================================
    // Test Helpers
    // ===========================================

    fn test_note_id() -> NoteId {
        "01HQ3K5M7NXJK4QZPW8V2R6T9Y".parse().unwrap()
    }

    fn test_datetime() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn test_note(id_str: &str, title: &str) -> Note {
        let id: NoteId = id_str.parse().unwrap();
        Note::new(id, title, test_datetime(), test_datetime()).unwrap()
    }

    fn test_note_with_id(id: NoteId, title: &str) -> Note {
        Note::new(id, title, test_datetime(), test_datetime()).unwrap()
    }

    fn test_note_without_links() -> Note {
        Note::new(test_note_id(), "No Links", test_datetime(), test_datetime()).unwrap()
    }

    fn test_note_with_topics(id_str: &str, topics: Vec<&str>) -> Note {
        let id: NoteId = id_str.parse().unwrap();
        let ts: Vec<Topic> = topics.iter().map(|t| Topic::new(t).unwrap()).collect();
        Note::builder(id, "Test", test_datetime(), test_datetime())
            .topics(ts)
            .build()
            .unwrap()
    }

    fn test_note_without_topics() -> Note {
        Note::new(test_note_id(), "Orphan", test_datetime(), test_datetime()).unwrap()
    }

    fn test_note_without_topics_with_id(id_str: &str) -> Note {
        let id: NoteId = id_str.parse().unwrap();
        Note::new(id, "Orphan", test_datetime(), test_datetime()).unwrap()
    }

    fn test_note_with_link(id_str: &str, target: NoteId) -> Note {
        let id: NoteId = id_str.parse().unwrap();
        let link = Link::new(target, vec!["see-also"]).unwrap();
        Note::builder(id, "Linked", test_datetime(), test_datetime())
            .links(vec![link])
            .build()
            .unwrap()
    }

    fn test_note_with_links(id_str: &str, targets: Vec<NoteId>) -> Note {
        let id: NoteId = id_str.parse().unwrap();
        let links: Vec<Link> = targets
            .into_iter()
            .map(|t| Link::new(t, vec!["see-also"]).unwrap())
            .collect();
        Note::builder(id, "Multi-linked", test_datetime(), test_datetime())
            .links(links)
            .build()
            .unwrap()
    }

    fn test_note_with_self_link(id_str: &str) -> Note {
        let id: NoteId = id_str.parse().unwrap();
        let link = Link::new(id.clone(), vec!["self-ref"]).unwrap();
        Note::builder(id, "Self-linked", test_datetime(), test_datetime())
            .links(vec![link])
            .build()
            .unwrap()
    }

    fn test_note_with_topics_and_links(
        id_str: &str,
        topics: Vec<&str>,
        link_targets: Vec<NoteId>,
    ) -> Note {
        let id: NoteId = id_str.parse().unwrap();
        let ts: Vec<Topic> = topics.iter().map(|t| Topic::new(t).unwrap()).collect();
        let links: Vec<Link> = link_targets
            .into_iter()
            .map(|t| Link::new(t, vec!["see-also"]).unwrap())
            .collect();
        Note::builder(id, "Full Note", test_datetime(), test_datetime())
            .topics(ts)
            .links(links)
            .build()
            .unwrap()
    }

    // ===========================================
    // Phase 1: Duplicate ID Detection
    // ===========================================

    #[test]
    fn duplicate_ids_empty_collection() {
        let notes: Vec<(PathBuf, &Note)> = vec![];
        let issues = find_duplicate_ids(&notes);
        assert!(issues.is_empty());
    }

    #[test]
    fn duplicate_ids_single_note() {
        let note = test_note("01HQ3K5M7NXJK4QZPW8V2R6T9Y", "Note A");
        let notes = vec![(PathBuf::from("a.md"), &note)];
        let issues = find_duplicate_ids(&notes);
        assert!(issues.is_empty());
    }

    #[test]
    fn duplicate_ids_unique_ids() {
        let note_a = test_note("01HQ3K5M7NXJK4QZPW8V2R6T9A", "Note A");
        let note_b = test_note("01HQ3K5M7NXJK4QZPW8V2R6T9B", "Note B");
        let notes = vec![
            (PathBuf::from("a.md"), &note_a),
            (PathBuf::from("b.md"), &note_b),
        ];
        let issues = find_duplicate_ids(&notes);
        assert!(issues.is_empty());
    }

    #[test]
    fn duplicate_ids_detects_duplicate() {
        let id = test_note_id();
        let note_a = test_note_with_id(id.clone(), "Note A");
        let note_b = test_note_with_id(id, "Note B");
        let notes = vec![
            (PathBuf::from("a.md"), &note_a),
            (PathBuf::from("b.md"), &note_b),
        ];
        let issues = find_duplicate_ids(&notes);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].is_duplicate_id());
        assert_eq!(issues[0].path, PathBuf::from("b.md"));
    }

    #[test]
    fn duplicate_ids_issue_references_first_path() {
        let id = test_note_id();
        let note_a = test_note_with_id(id.clone(), "Note A");
        let note_b = test_note_with_id(id, "Note B");
        let notes = vec![
            (PathBuf::from("first.md"), &note_a),
            (PathBuf::from("second.md"), &note_b),
        ];
        let issues = find_duplicate_ids(&notes);

        if let ValidationKind::DuplicateId { first_path, .. } = &issues[0].kind {
            assert_eq!(first_path, &PathBuf::from("first.md"));
        } else {
            panic!("Expected DuplicateId");
        }
    }

    #[test]
    fn duplicate_ids_multiple_duplicates() {
        let id = test_note_id();
        let notes_owned: Vec<Note> = (0..3)
            .map(|i| test_note_with_id(id.clone(), &format!("Note {}", i)))
            .collect();
        let note_refs: Vec<_> = notes_owned
            .iter()
            .enumerate()
            .map(|(i, n)| (PathBuf::from(format!("{}.md", i)), n))
            .collect();

        let issues = find_duplicate_ids(&note_refs);
        assert_eq!(issues.len(), 2); // 2nd and 3rd are duplicates
    }

    #[test]
    fn duplicate_ids_different_ids() {
        let id_x: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9X".parse().unwrap();
        let id_y: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9Z".parse().unwrap();
        let notes_owned = vec![
            test_note_with_id(id_x.clone(), "X1"),
            test_note_with_id(id_x, "X2"),
            test_note_with_id(id_y.clone(), "Y1"),
            test_note_with_id(id_y, "Y2"),
        ];
        let note_refs: Vec<_> = notes_owned
            .iter()
            .enumerate()
            .map(|(i, n)| (PathBuf::from(format!("{}.md", i)), n))
            .collect();

        let issues = find_duplicate_ids(&note_refs);
        assert_eq!(issues.len(), 2); // One for each duplicate pair
    }

    // ===========================================
    // Phase 2: Broken Link Detection
    // ===========================================

    #[test]
    fn broken_links_empty_collection() {
        let notes: Vec<(PathBuf, &Note)> = vec![];
        let known_ids = HashSet::new();
        let issues = find_broken_links(&notes, &known_ids);
        assert!(issues.is_empty());
    }

    #[test]
    fn broken_links_no_links() {
        let note = test_note_without_links();
        let notes = vec![(PathBuf::from("a.md"), &note)];
        let known_ids = HashSet::from([note.id().clone()]);
        let issues = find_broken_links(&notes, &known_ids);
        assert!(issues.is_empty());
    }

    #[test]
    fn broken_links_valid_link() {
        let target_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9B".parse().unwrap();
        let note = test_note_with_link("01HQ3K5M7NXJK4QZPW8V2R6T9A", target_id.clone());
        let notes = vec![(PathBuf::from("a.md"), &note)];
        let known_ids = HashSet::from([note.id().clone(), target_id]);
        let issues = find_broken_links(&notes, &known_ids);
        assert!(issues.is_empty());
    }

    #[test]
    fn broken_links_detects_missing_target() {
        let target_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9B".parse().unwrap();
        let note = test_note_with_link("01HQ3K5M7NXJK4QZPW8V2R6T9A", target_id);
        let notes = vec![(PathBuf::from("a.md"), &note)];
        let known_ids = HashSet::from([note.id().clone()]); // target NOT included

        let issues = find_broken_links(&notes, &known_ids);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].is_broken_link());
        assert_eq!(issues[0].path, PathBuf::from("a.md"));
    }

    #[test]
    fn broken_links_issue_contains_target() {
        let target_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9B".parse().unwrap();
        let note = test_note_with_link("01HQ3K5M7NXJK4QZPW8V2R6T9A", target_id.clone());
        let notes = vec![(PathBuf::from("a.md"), &note)];
        let known_ids = HashSet::from([note.id().clone()]);

        let issues = find_broken_links(&notes, &known_ids);

        if let ValidationKind::BrokenLink { target_id: tid } = &issues[0].kind {
            assert_eq!(tid, &target_id);
        } else {
            panic!("Expected BrokenLink");
        }
    }

    #[test]
    fn broken_links_multiple_in_one_note() {
        let target_a: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9A".parse().unwrap();
        let target_b: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9B".parse().unwrap();
        let note = test_note_with_links("01HQ3K5M7NXJK4QZPW8V2R6T9C", vec![target_a, target_b]);
        let notes = vec![(PathBuf::from("a.md"), &note)];
        let known_ids = HashSet::from([note.id().clone()]); // neither target exists

        let issues = find_broken_links(&notes, &known_ids);
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn broken_links_mix_valid_and_broken() {
        let valid_target: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9A".parse().unwrap();
        let broken_target: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9B".parse().unwrap();
        let note = test_note_with_links(
            "01HQ3K5M7NXJK4QZPW8V2R6T9C",
            vec![valid_target.clone(), broken_target],
        );
        let notes = vec![(PathBuf::from("a.md"), &note)];
        let known_ids = HashSet::from([note.id().clone(), valid_target]);

        let issues = find_broken_links(&notes, &known_ids);
        assert_eq!(issues.len(), 1); // Only the broken one
    }

    #[test]
    fn broken_links_across_notes() {
        let missing: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9X".parse().unwrap();
        let note_a = test_note_with_link("01HQ3K5M7NXJK4QZPW8V2R6T9A", missing.clone());
        let note_b = test_note_with_link("01HQ3K5M7NXJK4QZPW8V2R6T9B", missing);
        let notes = vec![
            (PathBuf::from("a.md"), &note_a),
            (PathBuf::from("b.md"), &note_b),
        ];
        let known_ids = HashSet::from([note_a.id().clone(), note_b.id().clone()]);

        let issues = find_broken_links(&notes, &known_ids);
        assert_eq!(issues.len(), 2); // One per note
    }

    // ===========================================
    // Phase 3: Orphaned Note Detection
    // ===========================================

    #[test]
    fn orphaned_empty_collection() {
        let notes: Vec<(PathBuf, &Note)> = vec![];
        let issues = find_orphaned_notes(&notes);
        assert!(issues.is_empty());
    }

    #[test]
    fn orphaned_note_with_topics() {
        let note = test_note_with_topics("01HQ3K5M7NXJK4QZPW8V2R6T9A", vec!["software/rust"]);
        let notes = vec![(PathBuf::from("a.md"), &note)];
        let issues = find_orphaned_notes(&notes);
        assert!(issues.is_empty());
    }

    #[test]
    fn orphaned_note_without_topics() {
        let note = test_note_without_topics();
        let notes = vec![(PathBuf::from("orphan.md"), &note)];
        let issues = find_orphaned_notes(&notes);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].is_orphaned());
        assert_eq!(issues[0].path, PathBuf::from("orphan.md"));
        assert_eq!(issues[0].severity(), Severity::Warning);
    }

    #[test]
    fn orphaned_mixed() {
        let with_topics = test_note_with_topics("01HQ3K5M7NXJK4QZPW8V2R6T9A", vec!["software"]);
        let without_topics = test_note_without_topics_with_id("01HQ3K5M7NXJK4QZPW8V2R6T9B");
        let notes = vec![
            (PathBuf::from("ok.md"), &with_topics),
            (PathBuf::from("orphan.md"), &without_topics),
        ];

        let issues = find_orphaned_notes(&notes);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].path, PathBuf::from("orphan.md"));
    }

    #[test]
    fn orphaned_multiple() {
        let orphan_a = test_note_without_topics_with_id("01HQ3K5M7NXJK4QZPW8V2R6T9A");
        let orphan_b = test_note_without_topics_with_id("01HQ3K5M7NXJK4QZPW8V2R6T9B");
        let notes = vec![
            (PathBuf::from("a.md"), &orphan_a),
            (PathBuf::from("b.md"), &orphan_b),
        ];

        let issues = find_orphaned_notes(&notes);
        assert_eq!(issues.len(), 2);
    }

    // ===========================================
    // Phase 4: Combined Validation
    // ===========================================

    #[test]
    fn validate_notes_empty() {
        let notes: Vec<(PathBuf, &Note)> = vec![];
        let summary = validate_notes(&notes);
        assert!(summary.is_ok());
        assert_eq!(summary.total(), 0);
    }

    #[test]
    fn validate_notes_all_valid() {
        let note_a =
            test_note_with_topics_and_links("01HQ3K5M7NXJK4QZPW8V2R6T9A", vec!["software"], vec![]);
        let note_b = test_note_with_topics_and_links(
            "01HQ3K5M7NXJK4QZPW8V2R6T9B",
            vec!["reference"],
            vec![],
        );
        let notes = vec![
            (PathBuf::from("a.md"), &note_a),
            (PathBuf::from("b.md"), &note_b),
        ];

        let summary = validate_notes(&notes);
        assert!(summary.is_ok());
    }

    #[test]
    fn validate_notes_combines_issues() {
        // Create scenario with duplicate ID, broken link, and orphans
        let missing_target: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9X".parse().unwrap();

        // Two notes with same ID but have topics
        let note_dup1 =
            test_note_with_topics_and_links("01HQ3K5M7NXJK4QZPW8V2R6T9D", vec!["software"], vec![]);
        // Second duplicate - have topics to not count as orphan
        let dup2_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9D".parse().unwrap();
        let note_dup2 = Note::builder(dup2_id, "Dup2", test_datetime(), test_datetime())
            .topics(vec![Topic::new("reference").unwrap()])
            .build()
            .unwrap();

        // Note with broken link (no topics = orphan)
        let note_broken = test_note_with_link("01HQ3K5M7NXJK4QZPW8V2R6T9E", missing_target);

        // Pure orphan (no topics, no links)
        let note_orphan = test_note_without_topics_with_id("01HQ3K5M7NXJK4QZPW8V2R6T9F");

        let notes = vec![
            (PathBuf::from("dup1.md"), &note_dup1),
            (PathBuf::from("dup2.md"), &note_dup2),
            (PathBuf::from("broken.md"), &note_broken),
            (PathBuf::from("orphan.md"), &note_orphan),
        ];

        let summary = validate_notes(&notes);

        assert_eq!(summary.duplicate_ids().count(), 1);
        assert_eq!(summary.broken_links().count(), 1);
        // note_broken + note_orphan have no topics = 2 orphans
        assert_eq!(summary.orphaned_notes().count(), 2);
    }

    #[test]
    fn validate_notes_correct_counts() {
        let missing: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9X".parse().unwrap();

        let note_dup1 =
            test_note_with_topics_and_links("01HQ3K5M7NXJK4QZPW8V2R6T9D", vec!["software"], vec![]);
        let dup2_id: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9D".parse().unwrap();
        let note_dup2 = Note::builder(dup2_id, "Dup2", test_datetime(), test_datetime())
            .topics(vec![Topic::new("ref").unwrap()])
            .build()
            .unwrap();
        let note_broken = test_note_with_link("01HQ3K5M7NXJK4QZPW8V2R6T9E", missing);
        let note_orphan = test_note_without_topics_with_id("01HQ3K5M7NXJK4QZPW8V2R6T9F");

        let notes = vec![
            (PathBuf::from("dup1.md"), &note_dup1),
            (PathBuf::from("dup2.md"), &note_dup2),
            (PathBuf::from("broken.md"), &note_broken),
            (PathBuf::from("orphan.md"), &note_orphan),
        ];

        let summary = validate_notes(&notes);

        // Errors: 1 duplicate + 1 broken link = 2
        assert_eq!(summary.error_count(), 2);
        // Warnings: 2 orphans (broken.md has no topics, orphan.md has no topics)
        assert_eq!(summary.warning_count(), 2);
        assert_eq!(summary.total(), 4);
        assert!(summary.has_errors());
    }

    // ===========================================
    // Phase 5: Edge Cases
    // ===========================================

    #[test]
    fn self_referencing_link_valid() {
        let note = test_note_with_self_link("01HQ3K5M7NXJK4QZPW8V2R6T9A");
        let notes = vec![(PathBuf::from("a.md"), &note)];
        let known_ids = HashSet::from([note.id().clone()]);

        let issues = find_broken_links(&notes, &known_ids);
        assert!(issues.is_empty());
    }

    #[test]
    fn empty_topics_array_is_orphaned() {
        // Note::builder with empty topics vec
        let note = Note::builder(
            test_note_id(),
            "Empty Topics",
            test_datetime(),
            test_datetime(),
        )
        .topics(vec![]) // Explicitly empty
        .build()
        .unwrap();
        let notes = vec![(PathBuf::from("a.md"), &note)];

        let issues = find_orphaned_notes(&notes);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn duplicate_first_wins() {
        let id = test_note_id();
        let note_a = test_note_with_id(id.clone(), "First");
        let note_b = test_note_with_id(id, "Second");

        // Order: a then b
        let notes_ab = vec![
            (PathBuf::from("a.md"), &note_a),
            (PathBuf::from("b.md"), &note_b),
        ];
        let issues = find_duplicate_ids(&notes_ab);
        if let ValidationKind::DuplicateId { first_path, .. } = &issues[0].kind {
            assert_eq!(first_path, &PathBuf::from("a.md"));
        } else {
            panic!("Expected DuplicateId");
        }
    }

    #[test]
    fn validate_notes_with_cross_references() {
        // Two notes that reference each other (both valid)
        let id_a: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9A".parse().unwrap();
        let id_b: NoteId = "01HQ3K5M7NXJK4QZPW8V2R6T9B".parse().unwrap();

        let note_a = test_note_with_topics_and_links(
            "01HQ3K5M7NXJK4QZPW8V2R6T9A",
            vec!["software"],
            vec![id_b.clone()],
        );
        let note_b = test_note_with_topics_and_links(
            "01HQ3K5M7NXJK4QZPW8V2R6T9B",
            vec!["reference"],
            vec![id_a],
        );

        let notes = vec![
            (PathBuf::from("a.md"), &note_a),
            (PathBuf::from("b.md"), &note_b),
        ];

        let summary = validate_notes(&notes);
        assert!(summary.is_ok());
    }
}
