//! Command handlers for the CLI.

mod check;
mod index;
mod links;
mod list;
mod metadata;
mod new;
mod resolve;
mod search;
mod show_edit;

#[cfg(test)]
pub(crate) mod tests;

use std::path::{Path, PathBuf};

use crate::index::{FileResult, ProgressReporter};

// Re-export public items
pub use check::handle_check;
pub use index::handle_index;
pub use links::{handle_backlinks, handle_link, handle_rels, handle_unlink};
pub use list::handle_list;
pub use metadata::{handle_tag, handle_tags, handle_topics, handle_untag};
pub use new::{NewNoteResult, create_new_note, handle_new};
pub use resolve::{ResolveResult, resolve_note};
pub use search::handle_search;
pub use show_edit::{handle_edit, handle_show};

// Re-export for tests
#[cfg(test)]
pub(crate) use list::{note_matches_topic, parse_topic_filter};
#[cfg(test)]
pub(crate) use search::strip_html_tags;
#[cfg(test)]
pub(crate) use show_edit::{EditorLauncher, handle_edit_impl};

// ===========================================
// Shared Utilities
// ===========================================

/// Progress reporter that prints to stdout.
pub(crate) struct ConsoleReporter {
    verbose: bool,
}

impl ConsoleReporter {
    pub(crate) fn new(verbose: bool) -> Self {
        Self { verbose }
    }
}

impl ProgressReporter for ConsoleReporter {
    fn on_file(&mut self, path: &Path, result: FileResult) {
        if self.verbose {
            match result {
                FileResult::Indexed => println!("  indexed: {}", path.display()),
                FileResult::Skipped => println!("  skipped: {}", path.display()),
                FileResult::Error(msg) => eprintln!("  error: {}: {}", path.display(), msg),
            }
        }
    }

    fn on_complete(&mut self, indexed: usize, errors: usize) {
        if errors > 0 {
            eprintln!("Indexed {} notes with {} errors", indexed, errors);
        } else {
            println!("Indexed {} notes", indexed);
        }
    }
}

/// Returns the default index database path for a notes directory.
pub(crate) fn index_db_path(notes_dir: &Path) -> PathBuf {
    notes_dir.join(".index").join("notes.db")
}

/// Truncates a string to a maximum display width, adding ellipsis if needed.
pub(crate) fn truncate_str(s: &str, max_width: usize) -> String {
    if s.chars().count() <= max_width {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_width.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}
