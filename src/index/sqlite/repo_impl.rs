//! IndexRepository trait implementation for SqliteIndex.

use super::SqliteIndex;
use crate::domain::{Note, NoteId, Rel, Tag, Topic};
use crate::index::{
    IndexError, IndexRepository, IndexResult, IndexedNote, RelWithCount, SearchResult,
    TagWithCount, TopicWithCount,
};
use crate::infra::ContentHash;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

impl IndexRepository for SqliteIndex {
    fn remove_note(&mut self, id: &NoteId) -> IndexResult<()> {
        self.conn
            .execute("DELETE FROM notes WHERE id = ?", [id.to_string()])?;
        Ok(())
    }

    fn get_note(&self, id: &NoteId) -> IndexResult<Option<IndexedNote>> {
        // Query notes table
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, created, modified, path, content_hash
             FROM notes WHERE id = ?",
        )?;

        let note_row = stmt.query_row([id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        });

        let (id_str, title, description, created_str, modified_str, path_str, hash_str) =
            match note_row {
                Ok(row) => row,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(e) => return Err(IndexError::Database(e)),
            };

        // Parse values
        let note_id: NoteId = id_str
            .parse()
            .map_err(|e| IndexError::InvalidQuery(format!("invalid note ID in database: {}", e)))?;

        let created = DateTime::parse_from_rfc3339(&created_str)
            .map_err(|e| IndexError::InvalidQuery(format!("invalid created timestamp: {}", e)))?
            .with_timezone(&Utc);

        let modified = DateTime::parse_from_rfc3339(&modified_str)
            .map_err(|e| IndexError::InvalidQuery(format!("invalid modified timestamp: {}", e)))?
            .with_timezone(&Utc);

        let content_hash = ContentHash::from_hex(&hash_str)
            .map_err(|e| IndexError::InvalidQuery(format!("invalid content hash: {}", e)))?;

        let path = PathBuf::from(path_str);

        // Query aliases from aliases table
        let aliases: Vec<String> = self
            .conn
            .prepare("SELECT alias FROM aliases WHERE note_id = ?")?
            .query_map([id.to_string()], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();

        // Query topics via JOIN
        let topics: Vec<Topic> = self
            .conn
            .prepare("SELECT t.path FROM topics t JOIN note_topics nt ON t.id = nt.topic_id WHERE nt.note_id = ?")?
            .query_map([id.to_string()], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .filter_map(|path| Topic::new(&path).ok())
            .collect();

        // Query tags via JOIN
        let tags: Vec<Tag> = self
            .conn
            .prepare("SELECT t.name FROM tags t JOIN note_tags nt ON t.id = nt.tag_id WHERE nt.note_id = ?")?
            .query_map([id.to_string()], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .filter_map(|name| Tag::new(&name).ok())
            .collect();

        // Build IndexedNote
        let mut builder =
            IndexedNote::builder(note_id, title, created, modified, path, content_hash);

        if let Some(desc) = description {
            builder = builder.description(desc);
        }

        builder = builder.topics(topics).aliases(aliases).tags(tags);

        Ok(Some(builder.build()))
    }

    fn upsert_note(
        &mut self,
        note: &Note,
        content_hash: &ContentHash,
        path: &Path,
    ) -> IndexResult<()> {
        let tx = self.transaction()?;

        // 1. INSERT/UPDATE notes row
        let id_str = note.id().to_string();
        let path_str = path.to_string_lossy();
        let created_str = note.created().to_rfc3339();
        let modified_str = note.modified().to_rfc3339();
        let hash_str = content_hash.as_str();
        let aliases_text = note.aliases().join(" ");
        let aliases_text_opt = if aliases_text.is_empty() {
            None
        } else {
            Some(aliases_text.as_str())
        };

        tx.conn().execute(
            "INSERT INTO notes (id, path, title, description, created, modified, content_hash, aliases_text)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                 path = excluded.path,
                 title = excluded.title,
                 description = excluded.description,
                 modified = excluded.modified,
                 content_hash = excluded.content_hash,
                 aliases_text = excluded.aliases_text",
            rusqlite::params![
                id_str,
                path_str,
                note.title(),
                note.description(),
                created_str,
                modified_str,
                hash_str,
                aliases_text_opt,
            ],
        )?;

        // 2. Delete existing junctions
        tx.conn()
            .execute("DELETE FROM note_topics WHERE note_id = ?", [&id_str])?;
        tx.conn()
            .execute("DELETE FROM note_tags WHERE note_id = ?", [&id_str])?;
        tx.conn()
            .execute("DELETE FROM aliases WHERE note_id = ?", [&id_str])?;

        // 3. Insert topics (OR IGNORE) and junctions
        for topic in note.topics() {
            let topic_path = topic.to_string();
            tx.conn().execute(
                "INSERT OR IGNORE INTO topics (path) VALUES (?)",
                [&topic_path],
            )?;
            tx.conn().execute(
                "INSERT INTO note_topics (note_id, topic_id)
                 SELECT ?, id FROM topics WHERE path = ?",
                [&id_str, &topic_path],
            )?;
        }

        // 4. Insert tags (OR IGNORE) and junctions
        for tag in note.tags() {
            tx.conn().execute(
                "INSERT OR IGNORE INTO tags (name) VALUES (?)",
                [tag.as_str()],
            )?;
            tx.conn().execute(
                "INSERT INTO note_tags (note_id, tag_id)
                 SELECT ?, id FROM tags WHERE name = ?",
                [&id_str, tag.as_str()],
            )?;
        }

        // 5. Insert aliases
        for alias in note.aliases() {
            tx.conn().execute(
                "INSERT INTO aliases (note_id, alias) VALUES (?, ?)",
                [&id_str, alias],
            )?;
        }

        // 6. Delete existing links (cascade will remove link_rels)
        tx.conn()
            .execute("DELETE FROM links WHERE source_id = ?", [&id_str])?;

        // 7. Insert links and their rels
        for link in note.links() {
            let target_str = link.target().to_string();
            let context = link.context();

            tx.conn().execute(
                "INSERT INTO links (source_id, target_id, note) VALUES (?, ?, ?)",
                rusqlite::params![id_str, target_str, context],
            )?;

            // Get the link id we just inserted
            let link_id: i64 = tx.conn().query_row(
                "SELECT id FROM links WHERE source_id = ? AND target_id = ?",
                [&id_str, &target_str],
                |row| row.get(0),
            )?;

            // Insert rels for this link
            for rel in link.rel() {
                tx.conn().execute(
                    "INSERT INTO link_rels (link_id, rel) VALUES (?, ?)",
                    rusqlite::params![link_id, rel.as_str()],
                )?;
            }
        }

        tx.commit()
    }

    fn list_by_topic(
        &self,
        topic: &Topic,
        include_descendants: bool,
    ) -> IndexResult<Vec<IndexedNote>> {
        let topic_path = topic.to_string();

        let query = if include_descendants {
            "SELECT DISTINCT n.id FROM notes n
             JOIN note_topics nt ON n.id = nt.note_id
             JOIN topics t ON nt.topic_id = t.id
             WHERE t.path = ?1 OR t.path LIKE ?2"
        } else {
            "SELECT DISTINCT n.id FROM notes n
             JOIN note_topics nt ON n.id = nt.note_id
             JOIN topics t ON nt.topic_id = t.id
             WHERE t.path = ?1"
        };

        let mut stmt = self.conn.prepare(query)?;

        let note_ids: Vec<NoteId> = if include_descendants {
            let pattern = format!("{}/%", topic_path);
            stmt.query_map(rusqlite::params![&topic_path, &pattern], |row| {
                row.get::<_, String>(0)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|id_str| id_str.parse().ok())
            .collect()
        } else {
            stmt.query_map([&topic_path], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .filter_map(|id_str| id_str.parse().ok())
                .collect()
        };

        let mut notes = Vec::with_capacity(note_ids.len());
        for id in note_ids {
            if let Some(note) = self.get_note(&id)? {
                notes.push(note);
            }
        }

        Ok(notes)
    }

    fn list_by_tag(&self, tag: &Tag) -> IndexResult<Vec<IndexedNote>> {
        let query = "SELECT DISTINCT n.id FROM notes n
                     JOIN note_tags nt ON n.id = nt.note_id
                     JOIN tags t ON nt.tag_id = t.id
                     WHERE t.name = ?";

        let mut stmt = self.conn.prepare(query)?;
        let note_ids: Vec<NoteId> = stmt
            .query_map([tag.as_str()], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .filter_map(|id_str| id_str.parse().ok())
            .collect();

        let mut notes = Vec::with_capacity(note_ids.len());
        for id in note_ids {
            if let Some(note) = self.get_note(&id)? {
                notes.push(note);
            }
        }

        Ok(notes)
    }

    fn search(&self, query: &str) -> IndexResult<Vec<SearchResult>> {
        // Phase 1: Handle empty/whitespace queries
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Execute FTS query with weighted BM25 ranking
        // Weights: title=10, description=5, aliases=5, body=1
        let mut stmt = self.conn.prepare(
            "SELECT
                n.id,
                -bm25(notes_fts, 10.0, 5.0, 5.0, 1.0) as rank,
                snippet(notes_fts, -1, '<b>', '</b>', '...', 20) as snippet
             FROM notes_fts
             JOIN notes n ON notes_fts.rowid = n.rowid
             WHERE notes_fts MATCH ?1
             ORDER BY rank DESC",
        )?;

        let row_iter = stmt.query_map([query], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, String>(2)?,
            ))
        });

        // Collect results, properly handling FTS errors
        let mut results = Vec::new();
        for row_result in row_iter.map_err(|e| {
            let msg = e.to_string();
            if msg.contains("fts5") || msg.contains("syntax") {
                IndexError::InvalidQuery(format!("invalid FTS query: {}", e))
            } else {
                IndexError::Database(e)
            }
        })? {
            // Map rusqlite::Error to IndexError for each row
            let row = row_result.map_err(|e| {
                let msg = e.to_string();
                if msg.contains("fts5") || msg.contains("syntax") {
                    IndexError::InvalidQuery(format!("invalid FTS query: {}", e))
                } else {
                    IndexError::Database(e)
                }
            })?;
            results.push(row);
        }

        // Fetch full notes and build SearchResult
        let mut search_results = Vec::with_capacity(results.len());
        for (id_str, rank, snippet) in results {
            let note_id: NoteId = id_str
                .parse()
                .map_err(|e| IndexError::InvalidQuery(format!("invalid note ID: {}", e)))?;

            if let Some(note) = self.get_note(&note_id)? {
                let result = if snippet.is_empty() {
                    SearchResult::new(note, rank)
                } else {
                    SearchResult::with_snippet(note, rank, snippet)
                };
                search_results.push(result);
            }
        }

        Ok(search_results)
    }

    fn all_topics(&self) -> IndexResult<Vec<TopicWithCount>> {
        let query = "SELECT t.path,
                            COUNT(DISTINCT nt.note_id) as exact_count,
                            (SELECT COUNT(DISTINCT nt2.note_id)
                             FROM topics t2
                             JOIN note_topics nt2 ON t2.id = nt2.topic_id
                             WHERE t2.path = t.path OR t2.path LIKE t.path || '/%'
                            ) as total_count
                     FROM topics t
                     INNER JOIN note_topics nt ON t.id = nt.topic_id
                     GROUP BY t.id
                     ORDER BY t.path";

        let mut stmt = self.conn.prepare(query)?;
        let topics = stmt
            .query_map([], |row| {
                let path: String = row.get(0)?;
                let exact_count: u32 = row.get(1)?;
                let total_count: u32 = row.get(2)?;
                Ok((path, exact_count, total_count))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(path, exact_count, total_count)| {
                Topic::new(&path)
                    .ok()
                    .map(|topic| TopicWithCount::new(topic, exact_count, total_count))
            })
            .collect();

        Ok(topics)
    }

    fn all_tags(&self) -> IndexResult<Vec<TagWithCount>> {
        let query = "SELECT t.name, COUNT(nt.note_id) as count
                     FROM tags t
                     INNER JOIN note_tags nt ON t.id = nt.tag_id
                     GROUP BY t.id
                     ORDER BY t.name";

        let mut stmt = self.conn.prepare(query)?;
        let tags = stmt
            .query_map([], |row| {
                let name: String = row.get(0)?;
                let count: u32 = row.get(1)?;
                Ok((name, count))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(name, count)| {
                Tag::new(&name)
                    .ok()
                    .map(|tag| TagWithCount::new(tag, count))
            })
            .collect();

        Ok(tags)
    }

    fn all_rels(&self) -> IndexResult<Vec<RelWithCount>> {
        let query = "SELECT rel, COUNT(*) as count
                     FROM link_rels
                     GROUP BY rel
                     ORDER BY rel";

        let mut stmt = self.conn.prepare(query)?;
        let rels = stmt
            .query_map([], |row| {
                let rel_str: String = row.get(0)?;
                let count: u32 = row.get(1)?;
                Ok((rel_str, count))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(rel_str, count)| {
                Rel::new(&rel_str)
                    .ok()
                    .map(|rel| RelWithCount::new(rel, count))
            })
            .collect();

        Ok(rels)
    }

    fn get_content_hash(&self, path: &Path) -> IndexResult<Option<ContentHash>> {
        let path_str = path.to_string_lossy();
        let mut stmt = self
            .conn
            .prepare("SELECT content_hash FROM notes WHERE path = ?")?;
        let hash = stmt.query_row([&*path_str], |row| {
            let hash_str: String = row.get(0)?;
            Ok(hash_str)
        });
        match hash {
            Ok(hash_str) => {
                Ok(Some(ContentHash::from_hex(&hash_str).map_err(|e| {
                    IndexError::InvalidQuery(format!("invalid hash: {}", e))
                })?))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(IndexError::Database(e)),
        }
    }

    fn list_all(&self) -> IndexResult<Vec<IndexedNote>> {
        let mut stmt = self.conn.prepare("SELECT id FROM notes")?;
        let note_ids: Vec<NoteId> = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .filter_map(|id_str| id_str.parse().ok())
            .collect();

        let mut notes = Vec::with_capacity(note_ids.len());
        for id in note_ids {
            if let Some(note) = self.get_note(&id)? {
                notes.push(note);
            }
        }

        Ok(notes)
    }

    fn find_by_id_prefix(&self, prefix: &str) -> IndexResult<Vec<IndexedNote>> {
        if prefix.is_empty() {
            return Ok(Vec::new());
        }

        // ULID IDs are uppercase, so normalize the prefix
        let prefix_upper = prefix.to_uppercase();

        let mut stmt = self
            .conn
            .prepare("SELECT id FROM notes WHERE id LIKE ? || '%' COLLATE NOCASE")?;

        let note_ids: Vec<NoteId> = stmt
            .query_map([&prefix_upper], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .filter_map(|id_str| id_str.parse().ok())
            .collect();

        let mut notes = Vec::with_capacity(note_ids.len());
        for id in note_ids {
            if let Some(note) = self.get_note(&id)? {
                notes.push(note);
            }
        }

        Ok(notes)
    }

    fn find_by_title(&self, title: &str) -> IndexResult<Vec<IndexedNote>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM notes WHERE title = ? COLLATE NOCASE")?;

        let note_ids: Vec<NoteId> = stmt
            .query_map([title], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .filter_map(|id_str| id_str.parse().ok())
            .collect();

        let mut notes = Vec::with_capacity(note_ids.len());
        for id in note_ids {
            if let Some(note) = self.get_note(&id)? {
                notes.push(note);
            }
        }

        Ok(notes)
    }

    fn find_by_alias(&self, alias: &str) -> IndexResult<Vec<IndexedNote>> {
        // Aliases are stored space-separated in aliases_text column
        // We need to match the alias as a whole word
        let pattern = format!("%{}%", alias);

        let mut stmt = self
            .conn
            .prepare("SELECT id FROM notes WHERE aliases_text LIKE ? COLLATE NOCASE")?;

        let note_ids: Vec<NoteId> = stmt
            .query_map([&pattern], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .filter_map(|id_str| id_str.parse().ok())
            .collect();

        // Filter to ensure exact alias match (not partial)
        let alias_lower = alias.to_lowercase();
        let mut notes = Vec::new();
        for id in note_ids {
            if let Some(note) = self.get_note(&id)? {
                // We need to check the actual aliases - fetch from DB
                let aliases_text: Option<String> = self
                    .conn
                    .query_row(
                        "SELECT aliases_text FROM notes WHERE id = ?",
                        [id.to_string()],
                        |row| row.get(0),
                    )
                    .ok()
                    .flatten();

                if let Some(text) = aliases_text {
                    let has_exact_match = text
                        .split_whitespace()
                        .any(|a| a.to_lowercase() == alias_lower);
                    if has_exact_match {
                        notes.push(note);
                    }
                }
            }
        }

        Ok(notes)
    }

    fn backlinks(&self, target_id: &NoteId, rel: Option<&Rel>) -> IndexResult<Vec<IndexedNote>> {
        let note_ids: Vec<NoteId> = match rel {
            None => {
                let mut stmt = self
                    .conn
                    .prepare("SELECT DISTINCT source_id FROM links WHERE target_id = ?")?;
                stmt.query_map([target_id.to_string()], |row| row.get::<_, String>(0))?
                    .filter_map(|r| r.ok())
                    .filter_map(|id_str| id_str.parse().ok())
                    .collect()
            }
            Some(r) => {
                let mut stmt = self.conn.prepare(
                    "SELECT DISTINCT l.source_id FROM links l
                     JOIN link_rels lr ON l.id = lr.link_id
                     WHERE l.target_id = ? AND lr.rel = ?",
                )?;
                stmt.query_map(
                    rusqlite::params![target_id.to_string(), r.as_str()],
                    |row| row.get::<_, String>(0),
                )?
                .filter_map(|r| r.ok())
                .filter_map(|id_str| id_str.parse().ok())
                .collect()
            }
        };

        let mut notes = Vec::with_capacity(note_ids.len());
        for id in note_ids {
            if let Some(note) = self.get_note(&id)? {
                notes.push(note);
            }
        }
        Ok(notes)
    }
}
