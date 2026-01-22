//! File I/O, frontmatter parsing, config

mod content_hash;
mod frontmatter;
mod fs;
mod slug;

pub use content_hash::{ContentHash, ContentHashError};
pub use frontmatter::{ParseError, ParsedNote, parse, serialize};
pub use fs::{FsError, read_note, scan_notes_directory, write_note};
pub use slug::{generate_filename, slugify};
