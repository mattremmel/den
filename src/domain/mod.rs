//! Core types: Note, Topic, Tag, NoteId (ULID), Link, Rel

mod link;
mod note;
mod note_id;
mod tag;
mod topic;
mod validate;
mod validation;

pub use link::{Link, ParseLinkError, ParseRelError, Rel};
pub use note::{Note, NoteBuilder, ParseNoteError};
pub use note_id::{NoteId, ParseNoteIdError};
pub use tag::{ParseTagError, Tag};
pub use topic::{ParseTopicError, Topic};
pub use validate::{find_broken_links, find_duplicate_ids, find_orphaned_notes, validate_notes};
pub use validation::{Severity, ValidationIssue, ValidationKind, ValidationSummary};
