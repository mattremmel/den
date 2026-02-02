//! Kinds command handler.

use anyhow::{Context, Result};
use std::path::Path;

use super::index_db_path;
use crate::cli::KindsArgs;
use crate::cli::output::{Output, OutputFormat};
use crate::index::{IndexRepository, SqliteIndex};
use serde::Serialize;

/// A kind with its note count, for JSON output.
#[derive(Debug, Serialize)]
struct KindListing {
    kind: String,
    count: u32,
}

pub fn handle_kinds(args: &KindsArgs, notes_dir: &Path) -> Result<()> {
    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)
        .with_context(|| format!("failed to open index at {}", db_path.display()))?;

    // Use efficient SQL query to count by kind
    let kinds = index.all_kinds().with_context(|| "failed to list kinds")?;

    // Calculate total note count
    let total_notes: u32 = kinds.iter().map(|k| k.count()).sum();

    match args.format {
        OutputFormat::Human => {
            if kinds.is_empty() {
                println!("No notes found.");
            } else {
                println!("{:<20}  {:>6}", "Kind", "Count");
                println!("{:<20}  {:>6}", "--------------------", "------");

                for kind_with_count in &kinds {
                    println!(
                        "{:<20}  {:>6}",
                        kind_with_count.kind().as_str(),
                        kind_with_count.count()
                    );
                }

                println!();
                println!("{} kind(s), {} note(s)", kinds.len(), total_notes);
            }
        }
        OutputFormat::Json => {
            let listings: Vec<KindListing> = kinds
                .iter()
                .map(|k| KindListing {
                    kind: k.kind().as_str().to_owned(),
                    count: k.count(),
                })
                .collect();
            let output = Output::new(listings);
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Paths => {
            // For paths format, just list kinds (one per line)
            for kind_with_count in &kinds {
                println!("{}", kind_with_count.kind().as_str());
            }
        }
    }

    Ok(())
}
