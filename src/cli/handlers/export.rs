//! Handler for the `export` command.

use std::path::Path;

use anyhow::{bail, Result};
use serde::Serialize;

use crate::cli::{ExportArgs, ExportFormat, output::OutputFormat};
use crate::domain::{Tag, Topic};
use crate::export::{
    LinkResolver, LinkResolverOptions, SiteConfig, generate_site, render_note_html,
    template::RenderOptions,
};
use crate::index::{IndexRepository, IndexedNote, SqliteIndex};
use crate::infra::read_note;

use super::index_db_path;
use super::resolve::{ResolveResult, print_ambiguous_notes, resolve_note};

/// Result of an export operation.
#[derive(Debug, Serialize)]
pub struct ExportResult {
    /// Number of notes exported
    pub notes_exported: usize,
    /// Output path (if writing to file)
    pub path: Option<String>,
    /// Note ID (for single note export)
    pub id: Option<String>,
    /// Note title (for single note export)
    pub title: Option<String>,
}

/// Handle the `export` command.
pub fn handle_export(args: &ExportArgs, notes_dir: &Path) -> Result<()> {
    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)?;

    match (&args.note, args.all) {
        (Some(query), false) => handle_single_export(args, &index, notes_dir, query),
        (None, true) => handle_bulk_export(args, &index, notes_dir),
        _ => unreachable!(),
    }
}

/// Export a single note.
fn handle_single_export(
    args: &ExportArgs,
    index: &SqliteIndex,
    notes_dir: &Path,
    query: &str,
) -> Result<()> {
    let indexed_note = match resolve_note(index, query)? {
        ResolveResult::Unique(note) => note,
        ResolveResult::Ambiguous(notes) => {
            print_ambiguous_notes(query, &notes);
            bail!("Ambiguous note reference");
        }
        ResolveResult::NotFound => {
            bail!("Note not found: {}", query);
        }
    };

    let file_path = notes_dir.join(indexed_note.path());
    let parsed = read_note(&file_path)?;

    match args.export_format {
        ExportFormat::Html => {
            // Create link resolver if requested
            let link_options = LinkResolverOptions::default();
            let resolver = if args.resolve_links {
                Some(LinkResolver::from_index(index, &link_options))
            } else {
                None
            };

            let options = RenderOptions {
                template_path: args.template.as_deref(),
                theme: args.theme.as_deref(),
                link_resolver: resolver.as_ref(),
            };

            let html = render_note_html(&parsed.note, &parsed.body, &options)?;

            match &args.output {
                Some(output_path) => {
                    // Determine if output is a directory or file
                    // Treat as directory if: already a dir, ends with /, or has no extension
                    let is_dir = output_path.is_dir()
                        || output_path.to_string_lossy().ends_with('/')
                        || output_path.extension().is_none();

                    let output_file = if is_dir {
                        std::fs::create_dir_all(output_path)?;
                        let slug = crate::infra::slugify(parsed.note.title());
                        output_path.join(format!("{}.html", slug))
                    } else {
                        // It's a file path - ensure parent directory exists
                        if let Some(parent) = output_path.parent().filter(|p| !p.as_os_str().is_empty()) {
                            std::fs::create_dir_all(parent)?;
                        }
                        output_path.clone()
                    };

                    std::fs::write(&output_file, &html)?;

                    print_result(
                        &args.cli_format,
                        ExportResult {
                            notes_exported: 1,
                            path: Some(output_file.display().to_string()),
                            id: Some(parsed.note.id().to_string()),
                            title: Some(parsed.note.title().to_string()),
                        },
                        &format!(
                            "Exported '{}' to {}",
                            parsed.note.title(),
                            output_file.display()
                        ),
                    );
                }
                None => {
                    // Output to stdout
                    print!("{}", html);
                }
            }
        }
        ExportFormat::Pdf => {
            bail!("PDF export is not yet implemented. Use --format html and convert manually.");
        }
        ExportFormat::Site => {
            bail!("Site export requires --all flag to export all notes.");
        }
    }

    Ok(())
}

/// Export multiple notes (bulk export).
fn handle_bulk_export(args: &ExportArgs, index: &SqliteIndex, notes_dir: &Path) -> Result<()> {
    let output_dir = match &args.output {
        Some(p) => p.clone(),
        None => bail!("Bulk export requires --output directory"),
    };

    // Get notes based on filters
    let mut notes = get_filtered_notes(index, args)?;

    // Exclude archived by default
    if !args.include_archived {
        let archived_tag = Tag::new("archived").unwrap();
        notes.retain(|n| !n.tags().contains(&archived_tag));
    }

    if notes.is_empty() {
        bail!("No notes match the specified filters");
    }

    match args.export_format {
        ExportFormat::Html => {
            // Bulk HTML export - each note as a separate file
            std::fs::create_dir_all(&output_dir)?;

            // Create link resolver if requested
            let link_options = LinkResolverOptions::default();
            let resolver = if args.resolve_links {
                Some(LinkResolver::from_notes(&notes, &link_options))
            } else {
                None
            };

            let render_options = RenderOptions {
                template_path: args.template.as_deref(),
                theme: args.theme.as_deref(),
                link_resolver: resolver.as_ref(),
            };

            let mut exported = 0;
            for indexed_note in &notes {
                let file_path = notes_dir.join(indexed_note.path());
                let parsed = read_note(&file_path)?;

                let html = render_note_html(&parsed.note, &parsed.body, &render_options)?;
                let slug = crate::infra::slugify(parsed.note.title());
                std::fs::write(output_dir.join(format!("{}.html", slug)), html)?;
                exported += 1;
            }

            print_result(
                &args.cli_format,
                ExportResult {
                    notes_exported: exported,
                    path: Some(output_dir.display().to_string()),
                    id: None,
                    title: None,
                },
                &format!("Exported {} notes to {}", exported, output_dir.display()),
            );
        }
        ExportFormat::Site => {
            // Static site generation
            let site_config = SiteConfig {
                site_title: "Notes",
                theme: args.theme.as_deref(),
                note_template: args.template.as_deref(),
            };

            let result = generate_site(&notes, &output_dir, notes_dir, &site_config)?;

            print_result(
                &args.cli_format,
                ExportResult {
                    notes_exported: result.notes_exported,
                    path: Some(output_dir.display().to_string()),
                    id: None,
                    title: None,
                },
                &format!(
                    "Generated site with {} notes and {} topic pages at {}",
                    result.notes_exported,
                    result.topic_pages,
                    output_dir.display()
                ),
            );
        }
        ExportFormat::Pdf => {
            bail!("PDF export is not yet implemented.");
        }
    }

    Ok(())
}

/// Get notes filtered by topic and tags.
fn get_filtered_notes(index: &SqliteIndex, args: &ExportArgs) -> Result<Vec<IndexedNote>> {
    let notes = match &args.topic {
        Some(topic_str) => {
            let include_descendants = topic_str.ends_with('/');
            let topic_path = topic_str.trim_end_matches('/');
            let topic = Topic::new(topic_path)?;
            index.list_by_topic(&topic, include_descendants)?
        }
        None => index.list_all()?,
    };

    // Filter by tags if specified
    if args.tags.is_empty() {
        return Ok(notes);
    }

    let required_tags: Vec<Tag> = args
        .tags
        .iter()
        .map(|t| Tag::new(t))
        .collect::<Result<_, _>>()?;

    let filtered: Vec<IndexedNote> = notes
        .into_iter()
        .filter(|n| required_tags.iter().all(|t| n.tags().contains(t)))
        .collect();

    Ok(filtered)
}

/// Print the result in the requested format.
fn print_result(format: &OutputFormat, result: ExportResult, human_message: &str) {
    match format {
        OutputFormat::Human => {
            println!("{}", human_message);
        }
        OutputFormat::Json => {
            let output = serde_json::json!({
                "data": result
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        OutputFormat::Paths => {
            if let Some(path) = &result.path {
                println!("{}", path);
            }
        }
    }
}
