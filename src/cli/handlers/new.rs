//! New note command handler.

use anyhow::{Context, Result, bail};
use chrono::Utc;
use std::io::Read;
use std::path::Path;
use std::process::Command;

use super::index_db_path;
use crate::cli::NewArgs;
use crate::cli::config::Config;
use crate::cli::output::{NewNoteListing, Output, OutputFormat};
use crate::domain::{Note, NoteId, NoteKind, NoteMetadata, Tag, Topic};
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
/// Validates the title, topics, tags, and kind, then constructs a Note.
/// Returns the Note and the generated filename.
///
/// # Errors
///
/// Returns an error if:
/// - The title is empty or whitespace-only
/// - Any topic is invalid
/// - Any tag is invalid
/// - The kind string is invalid
pub fn create_new_note(
    title: &str,
    description: Option<&str>,
    topic_strs: &[String],
    tag_strs: &[String],
    alias_strs: &[String],
    kind_str: Option<&str>,
    metadata_json: Option<&str>,
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

    // Parse and validate kind (strict: rejects unknown kinds)
    let kind = if let Some(k) = kind_str {
        NoteKind::parse_strict(k).with_context(|| format!("invalid kind '{}'", k))?
    } else {
        NoteKind::default()
    };

    // Parse metadata JSON if provided
    let metadata = if let Some(json_str) = metadata_json {
        let value: serde_json::Value = serde_json::from_str(json_str)
            .with_context(|| format!("invalid metadata JSON: {}", json_str))?;
        Some(
            NoteMetadata::from_value(&kind, value)
                .with_context(|| "failed to parse metadata for kind")?,
        )
    } else {
        None
    };

    // Generate ID and timestamps
    let id = NoteId::new();
    let now = Utc::now();

    // Build the note
    let note = Note::builder(id.clone(), trimmed_title, now, now)
        .description(description.map(|s| s.to_string()))
        .topics(topics)
        .aliases(alias_strs.to_vec())
        .tags(tags)
        .kind(kind)
        .metadata(metadata)
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
    .kind(parsed.note.kind().clone())
    .metadata(parsed.note.metadata().cloned())
    .build()
    .with_context(|| "failed to rebuild note")?;

    write_note(path, &updated_note, &parsed.body)
        .with_context(|| "failed to write updated note")?;

    Ok(())
}

/// Strips YAML frontmatter from content if present.
///
/// Detects the `---` delimited frontmatter block at the start of the content
/// and removes it, returning only the body.
fn strip_frontmatter(content: &str) -> &str {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return content;
    }

    // Find the closing delimiter after the opening `---`
    let after_opening = &trimmed[3..];
    // Skip to next line after opening ---
    let rest = match after_opening.find('\n') {
        Some(pos) => &after_opening[pos + 1..],
        None => return content, // Only `---` with no newline, not valid frontmatter
    };

    // Find the closing `---`
    for (i, line) in rest.lines().enumerate() {
        if line.trim() == "---" {
            // Calculate byte offset past the closing delimiter line
            let mut offset = 0;
            for (j, l) in rest.lines().enumerate() {
                if j == i {
                    offset += l.len();
                    // Skip past the newline after closing ---
                    let remaining = &rest[offset..];
                    if let Some(stripped) = remaining.strip_prefix('\n') {
                        return stripped;
                    } else if let Some(stripped) = remaining.strip_prefix("\r\n") {
                        return stripped;
                    }
                    return remaining;
                }
                offset += l.len() + 1; // +1 for newline
            }
        }
    }

    // No closing delimiter found, return original
    content
}

pub fn handle_new(args: &NewArgs, notes_dir: &Path, config: &Config) -> Result<()> {
    // Validate that the notes directory exists
    if !notes_dir.exists() {
        bail!("notes directory does not exist: {}", notes_dir.display());
    }

    // Validate --into path
    if let Some(ref subdir) = args.into {
        // Reject absolute paths
        if subdir.is_absolute() {
            bail!(
                "--into must be a relative path, not absolute: {}",
                subdir.display()
            );
        }

        // Reject path traversal
        for component in subdir.components() {
            if matches!(component, std::path::Component::ParentDir) {
                bail!("--into cannot contain '..': {}", subdir.display());
            }
        }
    }

    // Resolve target directory (vault root or subdirectory)
    let target_dir = if let Some(ref subdir) = args.into {
        let full_path = notes_dir.join(subdir);

        if !full_path.exists() {
            if args.mkdir {
                std::fs::create_dir_all(&full_path).with_context(|| {
                    format!("failed to create directory: {}", full_path.display())
                })?;
            } else {
                bail!(
                    "directory does not exist: {}\n  hint: use --mkdir to create it",
                    full_path.display()
                );
            }
        }

        if !full_path.is_dir() {
            bail!("not a directory: {}", full_path.display());
        }

        full_path
    } else {
        notes_dir.to_path_buf()
    };

    // Create the note (validates inputs)
    let result = create_new_note(
        &args.title,
        args.desc.as_deref(),
        &args.topics,
        &args.tags,
        &args.aliases,
        args.kind.as_deref(),
        args.metadata.as_deref(),
    )?;

    // Construct file path
    let file_path = target_dir.join(&result.filename);

    // Read body from file or STDIN if specified
    let body = if let Some(ref import_path) = args.file {
        let raw = std::fs::read_to_string(import_path)
            .with_context(|| format!("failed to read file: {}", import_path.display()))?;
        strip_frontmatter(&raw).to_string()
    } else if args.stdin {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .with_context(|| "failed to read from stdin")?;
        buf
    } else {
        String::new()
    };

    // Write the note file
    write_note(&file_path, &result.note, &body)
        .with_context(|| format!("failed to write note to {}", file_path.display()))?;

    // Update index (create if needed)
    let db_path = index_db_path(notes_dir);
    if let Ok(mut index) = SqliteIndex::open(&db_path) {
        let builder = IndexBuilder::new(notes_dir.to_path_buf());
        // Ignore index errors - note was created successfully
        let _ = builder.incremental_update(&mut index);
    }

    // Print success message
    match args.format {
        OutputFormat::Json => {
            let listing = NewNoteListing {
                id: result.note.id().to_string(),
                title: result.note.title().to_string(),
                path: file_path.display().to_string(),
            };
            println!(
                "{}",
                serde_json::to_string(&Output::new(listing))
                    .with_context(|| "failed to serialize output")?
            );
        }
        OutputFormat::Paths => {
            println!("{}", file_path.display());
        }
        OutputFormat::Human => {
            println!(
                "Created: {} [{}]",
                result.note.title(),
                result.note.id().prefix()
            );
            println!("  {}", file_path.display());
        }
    }

    // Open in editor by default (unless --no-edit, --stdin, or --file)
    if !args.no_edit && !args.stdin && args.file.is_none() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_frontmatter_removes_yaml() {
        let content = "---\ntitle: Test\ntags: [a]\n---\nBody content here";
        let result = strip_frontmatter(content);
        assert_eq!(result, "Body content here");
    }

    #[test]
    fn strip_frontmatter_preserves_content_without_frontmatter() {
        let content = "Just some plain text\nwith multiple lines";
        let result = strip_frontmatter(content);
        assert_eq!(result, content);
    }

    #[test]
    fn strip_frontmatter_handles_no_closing_delimiter() {
        let content = "---\ntitle: Test\nno closing delimiter";
        let result = strip_frontmatter(content);
        assert_eq!(result, content);
    }

    #[test]
    fn strip_frontmatter_handles_empty_body() {
        let content = "---\ntitle: Test\n---\n";
        let result = strip_frontmatter(content);
        assert_eq!(result, "");
    }
}
