//! Move/rename note command handler.

use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde::Serialize;
use std::path::Path;

use super::index_db_path;
use super::resolve::{ResolveResult, print_ambiguous_notes, resolve_note};
use crate::cli::MvArgs;
use crate::cli::output::{Output, OutputFormat};
use crate::domain::{Note, Topic};
use crate::index::{IndexBuilder, SqliteIndex};
use crate::infra::{generate_filename, read_note, write_note};

/// Result of a move operation for JSON output.
#[derive(Debug, Serialize)]
pub struct MvResult {
    pub id: String,
    pub title: String,
    pub old_path: String,
    pub new_path: String,
    pub topics: Vec<String>,
}

/// Validates the mv command arguments.
///
/// Returns an error if:
/// - No change is specified (no --title, --topic, or --clear-topics)
/// - Both --clear-topics and --topic are specified
/// - --title is empty
pub fn validate_mv_args(args: &MvArgs) -> Result<()> {
    // At least one change must be specified
    if args.title.is_none() && args.topics.is_empty() && !args.clear_topics {
        bail!("at least one of --title, --topic, or --clear-topics must be specified");
    }

    // --clear-topics and --topic are mutually exclusive
    if args.clear_topics && !args.topics.is_empty() {
        bail!("--clear-topics and --topic are mutually exclusive");
    }

    // --title must not be empty if specified
    if let Some(ref title) = args.title
        && title.trim().is_empty()
    {
        bail!("--title cannot be empty");
    }

    Ok(())
}

/// Parses and validates topic strings.
fn parse_topics(topic_strs: &[String]) -> Result<Vec<Topic>> {
    let mut topics = Vec::new();
    for topic_str in topic_strs {
        let topic = Topic::new(topic_str).with_context(|| {
            format!(
                "invalid topic '{}': topics must contain only alphanumeric characters, hyphens, underscores, and forward slashes",
                topic_str
            )
        })?;
        topics.push(topic);
    }
    Ok(topics)
}

pub fn handle_mv(args: &MvArgs, notes_dir: &Path) -> Result<()> {
    validate_mv_args(args)?;

    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    match resolve_note(&index, &args.note)? {
        ResolveResult::Unique(indexed_note) => {
            let old_path = notes_dir.join(indexed_note.path());
            let parsed = read_note(&old_path)
                .with_context(|| format!("failed to read note: {}", old_path.display()))?;

            // Determine new title
            let new_title = args
                .title
                .as_ref()
                .map(|t| t.trim())
                .unwrap_or(parsed.note.title());

            // Determine new topics
            let new_topics = if args.clear_topics {
                vec![]
            } else if !args.topics.is_empty() {
                parse_topics(&args.topics)?
            } else {
                parsed.note.topics().to_vec()
            };

            // Check for idempotency (no actual changes)
            let title_unchanged = new_title == parsed.note.title();
            let topics_unchanged = new_topics == parsed.note.topics();

            if title_unchanged && topics_unchanged {
                // No actual change needed
                let path_str = indexed_note.path().to_string_lossy();
                match args.format {
                    OutputFormat::Human => {
                        println!(
                            "No changes needed for '{}' [{}]",
                            new_title,
                            indexed_note.id()
                        );
                    }
                    OutputFormat::Json => {
                        let result = MvResult {
                            id: indexed_note.id().to_string(),
                            title: new_title.to_string(),
                            old_path: path_str.to_string(),
                            new_path: path_str.to_string(),
                            topics: new_topics.iter().map(|t| t.to_string()).collect(),
                        };
                        let out = Output::new(result);
                        println!("{}", serde_json::to_string_pretty(&out)?);
                    }
                    OutputFormat::Paths => {
                        println!("{}", path_str);
                    }
                }
                return Ok(());
            }

            // Build updated note
            let now = Utc::now();
            let updated_note = Note::builder(
                parsed.note.id().clone(),
                new_title,
                parsed.note.created(),
                now,
            )
            .description(parsed.note.description().map(|s| s.to_string()))
            .topics(new_topics.clone())
            .aliases(parsed.note.aliases().to_vec())
            .tags(parsed.note.tags().to_vec())
            .links(parsed.note.links().to_vec())
            .build()
            .with_context(|| "failed to rebuild note")?;

            // Determine new filename
            let new_filename = generate_filename(updated_note.id(), updated_note.title());
            let new_path = notes_dir.join(&new_filename);

            // Write to new path
            write_note(&new_path, &updated_note, &parsed.body)
                .with_context(|| format!("failed to write note to {}", new_path.display()))?;

            // Delete old file if renamed (different path)
            if old_path != new_path {
                std::fs::remove_file(&old_path).with_context(|| {
                    format!("failed to remove old file: {}", old_path.display())
                })?;
            }

            // Update index
            if let Ok(mut idx) = SqliteIndex::open(&db_path) {
                let builder = IndexBuilder::new(notes_dir.to_path_buf());
                let _ = builder.incremental_update(&mut idx);
            }

            // Output result
            match args.format {
                OutputFormat::Human => {
                    if !title_unchanged {
                        println!(
                            "Renamed '{}' to '{}' [{}]",
                            parsed.note.title(),
                            new_title,
                            updated_note.id().prefix()
                        );
                    }
                    if !topics_unchanged {
                        if new_topics.is_empty() {
                            println!(
                                "Cleared topics from '{}' [{}]",
                                new_title,
                                updated_note.id().prefix()
                            );
                        } else {
                            let topic_strs: Vec<_> =
                                new_topics.iter().map(|t| t.to_string()).collect();
                            println!(
                                "Moved '{}' to {} [{}]",
                                new_title,
                                topic_strs.join(", "),
                                updated_note.id().prefix()
                            );
                        }
                    }
                    if old_path != new_path {
                        println!("  {} -> {}", indexed_note.path().display(), new_filename);
                    }
                }
                OutputFormat::Json => {
                    let result = MvResult {
                        id: updated_note.id().to_string(),
                        title: new_title.to_string(),
                        old_path: indexed_note.path().to_string_lossy().to_string(),
                        new_path: new_filename,
                        topics: new_topics.iter().map(|t| t.to_string()).collect(),
                    };
                    let out = Output::new(result);
                    println!("{}", serde_json::to_string_pretty(&out)?);
                }
                OutputFormat::Paths => {
                    println!("{}", old_path.display());
                    if old_path != new_path {
                        println!("{}", new_path.display());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::output::OutputFormat;

    fn make_args(note: &str, title: Option<&str>, topics: Vec<&str>, clear_topics: bool) -> MvArgs {
        MvArgs {
            note: note.to_string(),
            title: title.map(|s| s.to_string()),
            topics: topics.into_iter().map(|s| s.to_string()).collect(),
            clear_topics,
            format: OutputFormat::Human,
        }
    }

    // ===========================================
    // Phase 1: Validation Unit Tests
    // ===========================================

    #[test]
    fn validate_requires_at_least_one_change() {
        let args = make_args("some-note", None, vec![], false);
        let result = validate_mv_args(&args);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("at least one of --title, --topic, or --clear-topics")
        );
    }

    #[test]
    fn validate_rejects_clear_topics_with_topics() {
        let args = make_args("some-note", None, vec!["software"], true);
        let result = validate_mv_args(&args);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("mutually exclusive")
        );
    }

    #[test]
    fn validate_rejects_empty_title() {
        let args = make_args("some-note", Some(""), vec![], false);
        let result = validate_mv_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn validate_rejects_whitespace_only_title() {
        let args = make_args("some-note", Some("   "), vec![], false);
        let result = validate_mv_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn validate_accepts_title_only() {
        let args = make_args("some-note", Some("New Title"), vec![], false);
        let result = validate_mv_args(&args);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_accepts_topics_only() {
        let args = make_args("some-note", None, vec!["software/rust"], false);
        let result = validate_mv_args(&args);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_accepts_clear_topics_only() {
        let args = make_args("some-note", None, vec![], true);
        let result = validate_mv_args(&args);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_accepts_title_and_topics() {
        let args = make_args("some-note", Some("New Title"), vec!["software"], false);
        let result = validate_mv_args(&args);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_accepts_title_and_clear_topics() {
        let args = make_args("some-note", Some("New Title"), vec![], true);
        let result = validate_mv_args(&args);
        assert!(result.is_ok());
    }

    // ===========================================
    // Topic Parsing Tests
    // ===========================================

    #[test]
    fn parse_topics_valid() {
        let topics = parse_topics(&["software/rust".to_string(), "tutorials".to_string()]);
        assert!(topics.is_ok());
        let topics = topics.unwrap();
        assert_eq!(topics.len(), 2);
        assert_eq!(topics[0].to_string(), "software/rust");
        assert_eq!(topics[1].to_string(), "tutorials");
    }

    #[test]
    fn parse_topics_invalid() {
        let topics = parse_topics(&["invalid topic with spaces".to_string()]);
        assert!(topics.is_err());
    }
}
