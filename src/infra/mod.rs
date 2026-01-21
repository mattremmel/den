//! File I/O, frontmatter parsing, config

mod frontmatter;
mod fs;

pub use frontmatter::{ParseError, ParsedNote, parse, serialize};
pub use fs::{FsError, read_note, write_note};
