//! SQLite repository and query builders

mod repository;

pub use repository::{
    IndexError, IndexRepository, IndexResult, IndexedNote, IndexedNoteBuilder, SearchResult,
    TagWithCount, TopicWithCount,
};
