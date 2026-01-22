//! Check command handler.

use std::path::Path;

use anyhow::{Result, bail};

use crate::cli::CheckArgs;
use crate::domain::{Severity, ValidationIssue, ValidationKind, validate_notes};
use crate::infra::{FsError, read_note, scan_notes_directory};

pub fn handle_check(args: &CheckArgs, notes_dir: &Path) -> Result<()> {
    if args.fix {
        eprintln!("warning: --fix is not yet implemented");
    }

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

    // 4. Display results
    if summary.is_ok() {
        println!("All notes OK.");
        return Ok(());
    }

    for issue in summary.issues_by_severity() {
        let prefix = match issue.severity() {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        println!("{}: {}", prefix, issue);
    }
    println!(
        "\nFound {} issue(s): {} error(s), {} warning(s)",
        summary.total(),
        summary.error_count(),
        summary.warning_count()
    );

    // 5. Exit code: fail only if there are errors
    if summary.has_errors() {
        bail!("check failed");
    }
    Ok(())
}
