//! Show and Edit command handlers.

use anyhow::{bail, Context, Result};
use std::path::Path;

use super::index_db_path;
use super::new::{open_in_editor, update_modified_timestamp};
use super::resolve::{print_ambiguous_notes, resolve_note, ResolveResult};
use crate::cli::config::Config;
use crate::cli::{EditArgs, ShowArgs};
use crate::index::{IndexBuilder, SqliteIndex};
use crate::infra::read_note;

pub fn handle_show(args: &ShowArgs, notes_dir: &Path) -> Result<()> {
    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    match resolve_note(&index, &args.note)? {
        ResolveResult::Unique(note) => {
            // Read and display the note
            let file_path = notes_dir.join(note.path());
            let parsed = read_note(&file_path)
                .with_context(|| format!("failed to read note: {}", file_path.display()))?;

            // Display frontmatter metadata
            println!("# {}", parsed.note.title());
            println!();

            if let Some(desc) = parsed.note.description() {
                println!("{}", desc);
                println!();
            }

            // Show metadata
            println!(
                "ID: {}  Created: {}  Modified: {}",
                parsed.note.id().prefix(),
                parsed.note.created().format("%Y-%m-%d"),
                parsed.note.modified().format("%Y-%m-%d")
            );

            if !parsed.note.topics().is_empty() {
                let topics: Vec<_> = parsed.note.topics().iter().map(|t| t.to_string()).collect();
                println!("Topics: {}", topics.join(", "));
            }

            if !parsed.note.tags().is_empty() {
                let tags: Vec<_> = parsed.note.tags().iter().map(|t| t.as_str()).collect();
                println!("Tags: {}", tags.join(", "));
            }

            println!();

            // Display body
            if !parsed.body.is_empty() {
                println!("{}", parsed.body);
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

/// Trait for launching an editor (allows mocking in tests).
pub(crate) trait EditorLauncher {
    fn open(&self, path: &Path) -> Result<()>;
}

/// Internal implementation that accepts a generic editor launcher.
pub(crate) fn handle_edit_impl<E: EditorLauncher>(
    args: &EditArgs,
    notes_dir: &Path,
    editor: &E,
) -> Result<()> {
    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    match resolve_note(&index, &args.note)? {
        ResolveResult::Unique(note) => {
            let file_path = notes_dir.join(note.path());

            editor.open(&file_path)?;
            update_modified_timestamp(&file_path)?;

            // Update index
            if let Ok(mut idx) = SqliteIndex::open(&db_path) {
                let builder = IndexBuilder::new(notes_dir.to_path_buf());
                let _ = builder.incremental_update(&mut idx);
            }

            println!("Edited: {} [{}]", note.title(), note.id().prefix());
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

pub fn handle_edit(args: &EditArgs, notes_dir: &Path, config: &Config) -> Result<()> {
    struct RealEditor<'a>(&'a Config);
    impl EditorLauncher for RealEditor<'_> {
        fn open(&self, path: &Path) -> Result<()> {
            open_in_editor(path, self.0)
        }
    }
    handle_edit_impl(args, notes_dir, &RealEditor(config))
}
