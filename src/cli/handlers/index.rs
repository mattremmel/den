//! Index command handler.

use anyhow::{Context, Result};
use std::path::Path;

use super::{ConsoleReporter, index_db_path};
use crate::cli::IndexArgs;
use crate::index::{IndexBuilder, SqliteIndex};

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
