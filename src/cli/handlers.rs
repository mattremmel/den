//! Command handlers (stubs).

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::{
    BacklinksArgs, CheckArgs, EditArgs, IndexArgs, LinkArgs, ListArgs, NewArgs, RelsArgs,
    SearchArgs, ShowArgs, TagArgs, TagsArgs, TopicsArgs, UnlinkArgs, UntagArgs,
};
use crate::index::{IndexBuilder, FileResult, ProgressReporter, SqliteIndex};

/// Progress reporter that prints to stdout.
struct ConsoleReporter {
    verbose: bool,
}

impl ConsoleReporter {
    fn new(verbose: bool) -> Self {
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
fn index_db_path(notes_dir: &Path) -> PathBuf {
    notes_dir.join(".index").join("notes.db")
}

pub fn handle_index(args: &IndexArgs, notes_dir: &Path, verbose: bool) -> Result<()> {
    let db_path = index_db_path(notes_dir);
    let mut index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    let builder = IndexBuilder::new(notes_dir.to_path_buf());
    let mut reporter = ConsoleReporter::new(verbose);

    if args.full {
        println!("Rebuilding index...");
        let result = builder
            .full_rebuild_with_progress(&mut index, &mut reporter)
            .with_context(|| "failed to rebuild index")?;

        for error in &result.errors {
            eprintln!("  {}", error);
        }
    } else {
        println!("Updating index...");
        let result = builder
            .incremental_update_with_progress(&mut index, &mut reporter)
            .with_context(|| "failed to update index")?;

        if verbose && (result.added > 0 || result.modified > 0 || result.removed > 0) {
            println!(
                "  {} added, {} modified, {} removed",
                result.added, result.modified, result.removed
            );
        }

        for error in &result.errors {
            eprintln!("  {}", error);
        }
    }

    Ok(())
}

pub fn handle_list(_args: &ListArgs) -> Result<()> {
    println!("ls: not yet implemented");
    Ok(())
}

pub fn handle_search(_args: &SearchArgs) -> Result<()> {
    println!("search: not yet implemented");
    Ok(())
}

pub fn handle_new(_args: &NewArgs) -> Result<()> {
    println!("new: not yet implemented");
    Ok(())
}

pub fn handle_show(_args: &ShowArgs) -> Result<()> {
    println!("show: not yet implemented");
    Ok(())
}

pub fn handle_edit(_args: &EditArgs) -> Result<()> {
    println!("edit: not yet implemented");
    Ok(())
}

pub fn handle_topics(_args: &TopicsArgs) -> Result<()> {
    println!("topics: not yet implemented");
    Ok(())
}

pub fn handle_tags(_args: &TagsArgs) -> Result<()> {
    println!("tags: not yet implemented");
    Ok(())
}

pub fn handle_tag(_args: &TagArgs) -> Result<()> {
    println!("tag: not yet implemented");
    Ok(())
}

pub fn handle_untag(_args: &UntagArgs) -> Result<()> {
    println!("untag: not yet implemented");
    Ok(())
}

pub fn handle_check(_args: &CheckArgs) -> Result<()> {
    println!("check: not yet implemented");
    Ok(())
}

pub fn handle_backlinks(_args: &BacklinksArgs) -> Result<()> {
    println!("backlinks: not yet implemented");
    Ok(())
}

pub fn handle_link(_args: &LinkArgs) -> Result<()> {
    println!("link: not yet implemented");
    Ok(())
}

pub fn handle_unlink(_args: &UnlinkArgs) -> Result<()> {
    println!("unlink: not yet implemented");
    Ok(())
}

pub fn handle_rels(_args: &RelsArgs) -> Result<()> {
    println!("rels: not yet implemented");
    Ok(())
}
