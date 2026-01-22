//! SQLite repository and query builders

mod repository;
mod schema;
mod sqlite;

pub use repository::{
    IndexError, IndexRepository, IndexResult, IndexedNote, IndexedNoteBuilder, SearchResult,
    TagWithCount, TopicWithCount,
};
pub use schema::{create_schema, get_schema_version, rebuild_fts};
pub use sqlite::{SqliteIndex, Transaction};
