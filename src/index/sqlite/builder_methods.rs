//! Additional methods for IndexBuilder support.

use super::SqliteIndex;
use crate::index::IndexResult;
use crate::infra::ContentHash;
use std::path::{Path, PathBuf};

impl SqliteIndex {
    /// Returns all indexed paths with their content hashes.
    ///
    /// Used by IndexBuilder for incremental updates to detect changes.
    pub fn all_indexed_paths(&self) -> IndexResult<Vec<(PathBuf, ContentHash)>> {
        let mut stmt = self.conn.prepare("SELECT path, content_hash FROM notes")?;
        let results = stmt
            .query_map([], |row| {
                let path: String = row.get(0)?;
                let hash: String = row.get(1)?;
                Ok((path, hash))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(path, hash)| {
                ContentHash::from_hex(&hash)
                    .ok()
                    .map(|h| (PathBuf::from(path), h))
            })
            .collect();
        Ok(results)
    }

    /// Removes a note from the index by its file path.
    ///
    /// Returns `true` if a note was removed, `false` if no note was found at that path.
    pub fn remove_by_path(&mut self, path: &Path) -> IndexResult<bool> {
        let rows = self.conn.execute(
            "DELETE FROM notes WHERE path = ?",
            [path.to_string_lossy().as_ref()],
        )?;
        Ok(rows > 0)
    }

    /// Clears all notes from the index.
    ///
    /// Used by IndexBuilder for full rebuilds.
    pub fn clear(&mut self) -> IndexResult<()> {
        self.conn.execute("DELETE FROM notes", [])?;
        Ok(())
    }
}
