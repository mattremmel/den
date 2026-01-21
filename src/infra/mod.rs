//! File I/O, frontmatter parsing, config

mod frontmatter;

pub use frontmatter::{parse, serialize, ParseError, ParsedNote};
