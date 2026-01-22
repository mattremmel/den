//! New note command handler.

use anyhow::{Context, Result, bail};
use chrono::Utc;
use std::path::Path;
use std::process::Command;

use super::index_db_path;
use crate::cli::NewArgs;
use crate::cli::config::Config;
use crate::domain::{Note, NoteId, Tag, Topic};
use crate::index::{IndexBuilder, SqliteIndex};
use crate::infra::{generate_filename, read_note, write_note};

/// Result of creating a new note (for testability).
#[derive(Debug)]
pub struct NewNoteResult {
    pub note: Note,
    pub filename: String,
}

/// Creates a new note from the given arguments (pure function, no I/O).
///
/// Validates the title, topics, and tags, then constructs a Note.
/// Returns the Note and the generated filename.
///
/// # Errors
///
/// Returns an error if:
/// - The title is empty or whitespace-only
/// - Any topic is invalid
/// - Any tag is invalid
pub fn create_new_note(
    title: &str,
    description: Option<&str>,
    topic_strs: &[String],
    tag_strs: &[String],
) -> Result<NewNoteResult> {
    // Validate title
    let trimmed_title = title.trim();
    if trimmed_title.is_empty() {
        bail!("title cannot be empty");
    }

    // Parse and validate topics
    let mut topics = Vec::new();
    for topic_str in topic_strs {
        let topic = Topic::new(topic_str)
            .with_context(|| format!("invalid topic '{}': topics must contain only alphanumeric characters, hyphens, underscores, and forward slashes", topic_str))?;
        topics.push(topic);
    }

    // Parse and validate tags
    let mut tags = Vec::new();
    for tag_str in tag_strs {
        let tag = Tag::new(tag_str)
            .with_context(|| format!("invalid tag '{}': tags must contain only alphanumeric characters, hyphens, and underscores (no spaces)", tag_str))?;
        tags.push(tag);
    }

    // Generate ID and timestamps
    let id = NoteId::new();
    let now = Utc::now();

    // Build the note
    let note = Note::builder(id.clone(), trimmed_title, now, now)
        .description(description.map(|s| s.to_string()))
        .topics(topics)
        .tags(tags)
        .build()
        .with_context(|| "failed to create note")?;

    // Generate filename
    let filename = generate_filename(&id, trimmed_title);

    Ok(NewNoteResult { note, filename })
}

/// Opens a file in the user's configured editor.
pub(crate) fn open_in_editor(path: &Path, config: &Config) -> Result<()> {
    let editor = config.editor();

    // Parse editor command (may include args like "code --wait")
    let parts: Vec<&str> = editor.split_whitespace().collect();
    if parts.is_empty() {
        bail!("editor command is empty");
    }

    let (cmd, args) = parts.split_first().unwrap();

    let status = Command::new(cmd)
        .args(args)
        .arg(path)
        .status()
        .with_context(|| format!("failed to launch editor '{}'", editor))?;

    if !status.success() {
        bail!("editor '{}' exited with non-zero status", editor);
    }

    Ok(())
}

/// Updates the modified timestamp of a note after editing.
pub(crate) fn update_modified_timestamp(path: &Path) -> Result<()> {
    let parsed = read_note(path).with_context(|| "failed to read note after editing")?;

    let now = Utc::now();
    let updated_note = Note::builder(
        parsed.note.id().clone(),
        parsed.note.title(),
        parsed.note.created(),
        now,
    )
    .description(parsed.note.description().map(|s| s.to_string()))
    .topics(parsed.note.topics().to_vec())
    .aliases(parsed.note.aliases().to_vec())
    .tags(parsed.note.tags().to_vec())
    .links(parsed.note.links().to_vec())
    .build()
    .with_context(|| "failed to rebuild note")?;

    write_note(path, &updated_note, &parsed.body)
        .with_context(|| "failed to write updated note")?;

    Ok(())
}

pub fn handle_new(args: &NewArgs, notes_dir: &Path, config: &Config) -> Result<()> {
    // Validate that the notes directory exists
    if !notes_dir.exists() {
        bail!("notes directory does not exist: {}", notes_dir.display());
    }

    // Create the note (validates inputs)
    let result = create_new_note(&args.title, args.desc.as_deref(), &args.topics, &args.tags)?;

    // Construct file path
    let file_path = notes_dir.join(&result.filename);

    // Write the note file
    write_note(&file_path, &result.note, "")
        .with_context(|| format!("failed to write note to {}", file_path.display()))?;

    // Update index (create if needed)
    let db_path = index_db_path(notes_dir);
    if let Ok(mut index) = SqliteIndex::open(&db_path) {
        let builder = IndexBuilder::new(notes_dir.to_path_buf());
        // Ignore index errors - note was created successfully
        let _ = builder.incremental_update(&mut index);
    }

    // Print success message
    println!(
        "Created: {} [{}]",
        result.note.title(),
        result.note.id().prefix()
    );
    println!("  {}", file_path.display());

    // Open in editor if requested
    if args.edit {
        open_in_editor(&file_path, config)?;
        // Update modified timestamp after editing
        update_modified_timestamp(&file_path)?;

        // Update index again after editing to capture content changes
        if let Ok(mut index) = SqliteIndex::open(&db_path) {
            let builder = IndexBuilder::new(notes_dir.to_path_buf());
            let _ = builder.incremental_update(&mut index);
        }
    }

    Ok(())
}
