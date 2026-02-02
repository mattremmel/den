//! IndexRepository trait implementation for SqliteIndex.

use super::SqliteIndex;
use crate::domain::{Note, NoteId, NoteKind, NoteMetadata, Rel, Tag, Topic};
use crate::index::{
    IndexError, IndexRepository, IndexResult, IndexedNote, KindWithCount, RelWithCount,
    SearchResult, TagWithCount, TopicWithCount,
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
        // Single query with scalar subqueries for related data
        // Using subqueries avoids cartesian product issues with multiple JOINs
        let mut stmt = self.conn.prepare_cached(
            "SELECT
                n.id, n.title, n.description, n.created, n.modified, n.path, n.content_hash,
                (SELECT GROUP_CONCAT(alias, '\x1F') FROM aliases WHERE note_id = n.id) as aliases,
                (SELECT GROUP_CONCAT(t.path, '\x1F') FROM note_topics nt JOIN topics t ON nt.topic_id = t.id WHERE nt.note_id = n.id) as topics,
                (SELECT GROUP_CONCAT(t.name, '\x1F') FROM note_tags ntg JOIN tags t ON ntg.tag_id = t.id WHERE ntg.note_id = n.id) as tags,
                n.kind, n.metadata
             FROM notes n
             WHERE n.id = ?",
        )?;

        let result = stmt.query_row([id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, Option<String>>(11)?,
            ))
        });

        let (
            id_str,
            title,
            description,
            created_str,
            modified_str,
            path_str,
            hash_str,
            aliases_str,
            topics_str,
            tags_str,
            kind_str,
            metadata_json,
        ) = match result {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(IndexError::Database(e)),
        };

        // Use unit separator as delimiter (unlikely to appear in data)
        const SEP: &str = "\x1F";

        // Parse core values
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

        // Parse GROUP_CONCAT results
        let aliases: Vec<String> = aliases_str
            .map(|s| s.split(SEP).map(String::from).collect())
            .unwrap_or_default();

        let topics: Vec<Topic> = topics_str
            .map(|s| s.split(SEP).filter_map(|p| Topic::new(p).ok()).collect())
            .unwrap_or_default();

        let tags: Vec<Tag> = tags_str
            .map(|s| s.split(SEP).filter_map(|t| Tag::new(t).ok()).collect())
            .unwrap_or_default();

        // Parse kind - FromStr is infallible (unknown kinds become Other)
        let kind: NoteKind = kind_str.parse().unwrap();

        // Parse metadata from JSON based on kind.
        // Errors are intentionally swallowed: if the JSON is malformed or doesn't
        // match the expected schema, we return None rather than failing the query.
        // This ensures notes remain queryable even if their metadata is corrupted.
        let metadata: Option<NoteMetadata> = metadata_json.and_then(|json| {
            serde_json::from_str::<serde_json::Value>(&json)
                .ok()
                .and_then(|v| NoteMetadata::from_value(&kind, v).ok())
        });

        // Build IndexedNote
        let mut builder =
            IndexedNote::builder(note_id, title, created, modified, path, content_hash);

        if let Some(desc) = description {
            builder = builder.description(desc);
        }

        builder = builder
            .topics(topics)
            .aliases(aliases)
            .tags(tags)
            .kind(kind)
            .metadata(metadata);

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

        // Serialize metadata to JSON if present.
        // Errors are intentionally swallowed: if serialization fails (unlikely),
        // we store NULL rather than failing the upsert. The note itself is more
        // important than its indexed metadata.
        let kind_str = note.kind().as_str();
        let metadata_json = note
            .metadata()
            .and_then(|m| m.to_value().ok())
            .and_then(|v| serde_json::to_string(&v).ok());

        tx.conn().execute(
            "INSERT INTO notes (id, path, title, description, created, modified, content_hash, aliases_text, kind, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
                 path = excluded.path,
                 title = excluded.title,
                 description = excluded.description,
                 modified = excluded.modified,
                 content_hash = excluded.content_hash,
                 aliases_text = excluded.aliases_text,
                 kind = excluded.kind,
                 metadata = excluded.metadata",
            rusqlite::params![
                id_str,
                path_str,
                note.title(),
                note.description(),
                created_str,
                modified_str,
                hash_str,
                aliases_text_opt,
                kind_str,
                metadata_json,
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

        // 6. Delete existing author/speaker junctions
        tx.conn()
            .execute("DELETE FROM note_authors WHERE note_id = ?", [&id_str])?;
        tx.conn()
            .execute("DELETE FROM note_speakers WHERE note_id = ?", [&id_str])?;

        // 7. Insert authors from metadata (if applicable)
        if let Some(metadata) = note.metadata() {
            // Insert authors
            for author in metadata.authors() {
                tx.conn().execute(
                    "INSERT OR IGNORE INTO authors (name) VALUES (?)",
                    [author],
                )?;
                tx.conn().execute(
                    "INSERT INTO note_authors (note_id, author_id)
                     SELECT ?, id FROM authors WHERE name = ?",
                    [&id_str, author],
                )?;
            }

            // Insert speakers (for transcripts)
            for speaker in metadata.speakers() {
                tx.conn().execute(
                    "INSERT OR IGNORE INTO speakers (name) VALUES (?)",
                    [speaker],
                )?;
                tx.conn().execute(
                    "INSERT INTO note_speakers (note_id, speaker_id)
                     SELECT ?, id FROM speakers WHERE name = ?",
                    [&id_str, speaker],
                )?;
            }
        }

        // 8. Delete existing links (cascade will remove link_rels)
        tx.conn()
            .execute("DELETE FROM links WHERE source_id = ?", [&id_str])?;

        // 9. Insert links and their rels
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

    fn list_by_kind(&self, kind: &NoteKind) -> IndexResult<Vec<IndexedNote>> {
        let query = "SELECT id FROM notes WHERE kind = ?";

        let mut stmt = self.conn.prepare(query)?;
        let note_ids: Vec<NoteId> = stmt
            .query_map([kind.as_str()], |row| row.get::<_, String>(0))?
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

    fn list_by_author(&self, author: &str) -> IndexResult<Vec<IndexedNote>> {
        let query = "SELECT DISTINCT n.id FROM notes n
                     JOIN note_authors na ON n.id = na.note_id
                     JOIN authors a ON na.author_id = a.id
                     WHERE a.name = ? COLLATE NOCASE";

        let mut stmt = self.conn.prepare(query)?;
        let note_ids: Vec<NoteId> = stmt
            .query_map([author], |row| row.get::<_, String>(0))?
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

    fn all_kinds(&self) -> IndexResult<Vec<KindWithCount>> {
        let query = "SELECT kind, COUNT(*) as count
                     FROM notes
                     GROUP BY kind
                     ORDER BY kind";

        let mut stmt = self.conn.prepare(query)?;
        let kinds = stmt
            .query_map([], |row| {
                let kind_str: String = row.get(0)?;
                let count: u32 = row.get(1)?;
                Ok((kind_str, count))
            })?
            .filter_map(|r| r.ok())
            .map(|(kind_str, count)| {
                // FromStr is infallible - unknown kinds become Other
                let kind: NoteKind = kind_str.parse().unwrap();
                KindWithCount::new(kind, count)
            })
            .collect();

        Ok(kinds)
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

    fn upsert_notes_batch(&mut self, notes: &[(&Note, &ContentHash, &Path)]) -> IndexResult<()> {
        if notes.is_empty() {
            return Ok(());
        }

        let tx = self.transaction()?;

        {
            // Prepare all statements once for reuse
            let mut insert_note = tx.conn().prepare_cached(
                "INSERT INTO notes (id, path, title, description, created, modified, content_hash, aliases_text, kind, metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(id) DO UPDATE SET
                     path = excluded.path,
                     title = excluded.title,
                     description = excluded.description,
                     modified = excluded.modified,
                     content_hash = excluded.content_hash,
                     aliases_text = excluded.aliases_text,
                     kind = excluded.kind,
                     metadata = excluded.metadata",
            )?;
            let mut delete_topics = tx
                .conn()
                .prepare_cached("DELETE FROM note_topics WHERE note_id = ?")?;
            let mut delete_tags = tx
                .conn()
                .prepare_cached("DELETE FROM note_tags WHERE note_id = ?")?;
            let mut delete_aliases = tx
                .conn()
                .prepare_cached("DELETE FROM aliases WHERE note_id = ?")?;
            let mut delete_links = tx
                .conn()
                .prepare_cached("DELETE FROM links WHERE source_id = ?")?;
            let mut delete_authors = tx
                .conn()
                .prepare_cached("DELETE FROM note_authors WHERE note_id = ?")?;
            let mut delete_speakers = tx
                .conn()
                .prepare_cached("DELETE FROM note_speakers WHERE note_id = ?")?;
            let mut insert_topic = tx
                .conn()
                .prepare_cached("INSERT OR IGNORE INTO topics (path) VALUES (?)")?;
            let mut insert_topic_junction = tx.conn().prepare_cached(
                "INSERT INTO note_topics (note_id, topic_id)
                 SELECT ?, id FROM topics WHERE path = ?",
            )?;
            let mut insert_tag = tx
                .conn()
                .prepare_cached("INSERT OR IGNORE INTO tags (name) VALUES (?)")?;
            let mut insert_tag_junction = tx.conn().prepare_cached(
                "INSERT INTO note_tags (note_id, tag_id)
                 SELECT ?, id FROM tags WHERE name = ?",
            )?;
            let mut insert_alias = tx
                .conn()
                .prepare_cached("INSERT INTO aliases (note_id, alias) VALUES (?, ?)")?;
            let mut insert_link = tx.conn().prepare_cached(
                "INSERT INTO links (source_id, target_id, note) VALUES (?, ?, ?)",
            )?;
            let mut get_link_id = tx
                .conn()
                .prepare_cached("SELECT id FROM links WHERE source_id = ? AND target_id = ?")?;
            let mut insert_link_rel = tx
                .conn()
                .prepare_cached("INSERT INTO link_rels (link_id, rel) VALUES (?, ?)")?;
            let mut insert_author = tx
                .conn()
                .prepare_cached("INSERT OR IGNORE INTO authors (name) VALUES (?)")?;
            let mut insert_author_junction = tx.conn().prepare_cached(
                "INSERT INTO note_authors (note_id, author_id)
                 SELECT ?, id FROM authors WHERE name = ?",
            )?;
            let mut insert_speaker = tx
                .conn()
                .prepare_cached("INSERT OR IGNORE INTO speakers (name) VALUES (?)")?;
            let mut insert_speaker_junction = tx.conn().prepare_cached(
                "INSERT INTO note_speakers (note_id, speaker_id)
                 SELECT ?, id FROM speakers WHERE name = ?",
            )?;

            for (note, content_hash, path) in notes {
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

                // Serialize kind and metadata (errors swallowed - see upsert_note comment)
                let kind_str = note.kind().as_str();
                let metadata_json = note
                    .metadata()
                    .and_then(|m| m.to_value().ok())
                    .and_then(|v| serde_json::to_string(&v).ok());

                // 1. Upsert note
                insert_note.execute(rusqlite::params![
                    id_str,
                    path_str,
                    note.title(),
                    note.description(),
                    created_str,
                    modified_str,
                    hash_str,
                    aliases_text_opt,
                    kind_str,
                    metadata_json,
                ])?;

                // 2. Delete existing junctions
                delete_topics.execute([&id_str])?;
                delete_tags.execute([&id_str])?;
                delete_aliases.execute([&id_str])?;
                delete_links.execute([&id_str])?;
                delete_authors.execute([&id_str])?;
                delete_speakers.execute([&id_str])?;

                // 3. Insert topics
                for topic in note.topics() {
                    let topic_path = topic.to_string();
                    insert_topic.execute([&topic_path])?;
                    insert_topic_junction.execute([&id_str, &topic_path])?;
                }

                // 4. Insert tags
                for tag in note.tags() {
                    insert_tag.execute([tag.as_str()])?;
                    insert_tag_junction.execute([&id_str, tag.as_str()])?;
                }

                // 5. Insert aliases
                for alias in note.aliases() {
                    insert_alias.execute([&id_str, alias])?;
                }

                // 6. Insert authors and speakers from metadata
                if let Some(metadata) = note.metadata() {
                    for author in metadata.authors() {
                        insert_author.execute([author])?;
                        insert_author_junction.execute([&id_str, author])?;
                    }
                    for speaker in metadata.speakers() {
                        insert_speaker.execute([speaker])?;
                        insert_speaker_junction.execute([&id_str, speaker])?;
                    }
                }

                // 7. Insert links
                for link in note.links() {
                    let target_str = link.target().to_string();
                    let context = link.context();

                    insert_link.execute(rusqlite::params![id_str, target_str, context])?;

                    let link_id: i64 =
                        get_link_id.query_row([&id_str, &target_str], |row| row.get(0))?;

                    for rel in link.rel() {
                        insert_link_rel.execute(rusqlite::params![link_id, rel.as_str()])?;
                    }
                }
            }
        }

        tx.commit()
    }
}
