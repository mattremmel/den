# Pipeline Usage Guide: `notes new`

Self-contained reference for creating notes programmatically via the `notes new` CLI.
Covers every flag, body input method, metadata schema, validation rule, output format, and concurrency guarantee.

---

## 1. Overview

`notes new` creates a markdown note file with YAML frontmatter and an optional body, then indexes it in a local SQLite database.

**Pipeline contract:**
- Set all frontmatter fields via flags (title, topics, tags, kind, metadata, etc.)
- Supply body content via `--stdin` or `--file`
- Always pass `--no-edit` to suppress the interactive editor
- Use `-f json` for machine-parseable output

---

## 2. Quick Reference

Canonical pipeline invocation with every flag:

```bash
echo "Body content here" | notes new "My Note Title" \
  --topic software/architecture \
  --topic reference \
  --tag draft \
  --tag needs-review \
  --desc "Short description of the note" \
  --alias "Alternate Name" \
  --kind transcript \
  --metadata '{"source":"https://example.com/video","speakers":["Alice","Bob"],"duration_seconds":3600}' \
  --into transcripts \
  --mkdir \
  --stdin \
  --no-edit \
  -f json
```

Output:

```json
{"data":{"id":"01JKM5P8X7QZYN3GT6ABCDEF01","title":"My Note Title","path":"/home/user/notes/transcripts/01JKM5P8X7-my-note-title.md"}}
```

---

## 3. Flags Reference

| Flag | Short | Type | Required | Description |
|------|-------|------|----------|-------------|
| `<TITLE>` | — | string (positional) | **yes** | Note title |
| `--topic` | `-T` | string (repeatable) | no | Hierarchical topic path (e.g. `software/architecture`) |
| `--tag` | `-t` | string (repeatable) | no | Flat tag (auto-lowercased) |
| `--desc` | `-D` | string | no | Short description |
| `--alias` | `-a` | string (repeatable) | no | Alternate name for the note |
| `--kind` | `-k` | string | no | Note kind (default: `generic`) |
| `--metadata` | — | JSON string | no | Kind-specific metadata as a JSON object |
| `--into` | — | relative path | no | Subdirectory within vault to place the note |
| `--mkdir` | — | flag | no | Create the `--into` subdirectory if missing (requires `--into`) |
| `--no-edit` | — | flag | no | Skip opening the editor after creation |
| `--stdin` | — | flag | no | Read note body from stdin |
| `--file` | — | path | no | Import body from a file (conflicts with `--stdin`) |
| `--format` | `-f` | `human` \| `json` \| `paths` | no | Output format (default: `human`) |
| `--dir` | `-d` | path | no | Override notes directory (global flag) |
| `--vault` | — | string | no | Use a named vault from config (global flag) |

---

## 4. Body Content Input

| Method | Flag | Behavior |
|--------|------|----------|
| Pipe from stdin | `--stdin` | Reads all of stdin as the note body |
| Import from file | `--file <path>` | Reads the file; auto-strips YAML frontmatter if present |
| Empty body | _(neither)_ | Creates the note with an empty body |

- `--stdin` and `--file` conflict — you cannot use both.
- When using `--stdin` or `--file`, the editor is automatically skipped (same as `--no-edit`).
- **Pipelines should always pass `--no-edit`** explicitly to be safe, even when using `--stdin` or `--file`.

### Frontmatter stripping (`--file`)

If the imported file starts with a `---` delimited YAML frontmatter block, it is automatically removed. Only the body after the closing `---` is used. If no valid frontmatter is detected, the entire file content is used as the body.

---

## 5. Kind Values

Accepted values (case-insensitive, strict parsing — unknown values are rejected):

| Value | Description |
|-------|-------------|
| `generic` | General-purpose note (default) |
| `book` | Book or book chapter |
| `paper` | Academic paper or research article |
| `transcript` | Audio/video transcript (podcast, interview, lecture) |
| `article` | Web article or blog post |

Invalid kind values produce an error:
```
Error: invalid kind 'podcast': unknown kind 'podcast': valid kinds are generic, book, paper, transcript, article
```

---

## 6. Metadata JSON Schemas

Pass as a single JSON string via `--metadata`. All fields are optional. Extra arbitrary keys are accepted and preserved in an `extra` map.

### `transcript`

```json
{
  "source": "string (URL of the source video/audio)",
  "speakers": ["string"],
  "duration_seconds": 3600,
  "chapters": [
    {"title": "string", "start_seconds": 0}
  ]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `source` | string | Source URL (YouTube, podcast feed, etc.) |
| `speakers` | string[] | Names of speakers in the transcript |
| `duration_seconds` | u32 | Total duration in seconds |
| `chapters` | object[] | Chapter markers with `title` (string) and `start_seconds` (u32) |

### `book`

```json
{
  "authors": ["string"],
  "isbn": "string",
  "publisher": "string",
  "year": 2024
}
```

| Field | Type | Description |
|-------|------|-------------|
| `authors` | string[] | Authors of the book |
| `isbn` | string | ISBN (10 or 13 digit) |
| `publisher` | string | Publisher name |
| `year` | u16 | Publication year |

### `paper`

```json
{
  "authors": ["string"],
  "doi": "string",
  "journal": "string",
  "year": 2024,
  "arxiv": "string"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `authors` | string[] | Authors of the paper |
| `doi` | string | Digital Object Identifier |
| `journal` | string | Journal or conference name |
| `year` | u16 | Publication year |
| `arxiv` | string | ArXiv identifier |

### `article`

```json
{
  "authors": ["string"],
  "url": "string",
  "publication": "string",
  "date": "YYYY-MM-DD"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `authors` | string[] | Authors of the article |
| `url` | string | Source URL |
| `publication` | string | Publication or site name |
| `date` | string | Publication date (ISO 8601 date) |

### `generic`

Accepts any JSON object. All keys are stored as arbitrary key-value pairs.

```json
{
  "any_key": "any_value",
  "nested": {"inner": true}
}
```

---

## 7. Validation Rules

### Title
- Must be non-empty after trimming whitespace.
- Error: `title cannot be empty`

### Topics
- Segments separated by `/`.
- Each segment: `[a-zA-Z0-9_-]` only (alphanumeric, hyphens, underscores).
- Case-sensitive (`Software` and `software` are different topics).
- Leading/trailing slashes and consecutive slashes are normalized away.
- Error: `invalid topic 'soft@ware': topics must contain only alphanumeric characters, hyphens, underscores, and forward slashes`

### Tags
- Flat (no `/` allowed).
- Characters: `[a-zA-Z0-9_-]` only.
- Auto-lowercased (`Draft` becomes `draft`).
- Error: `invalid tag 'needs review': tags must contain only alphanumeric characters, hyphens, and underscores (no spaces)`

### Kind
- Must exactly match one of: `generic`, `book`, `paper`, `transcript`, `article` (case-insensitive).
- Unknown values are rejected (strict parsing).
- Error: `unknown kind 'podcast': valid kinds are generic, book, paper, transcript, article`

### Metadata
- Must be valid JSON.
- Fields are validated against the kind's schema via serde deserialization.
- Error (invalid JSON): `invalid metadata JSON: {not json}`
- Error (schema mismatch): `failed to parse metadata for kind`

### `--into` path
- Must be a relative path (no absolute paths).
- Cannot contain `..` (no path traversal).
- Target directory must exist, or `--mkdir` must be passed.
- Error: `directory does not exist: /path/to/vault/subdir\n  hint: use --mkdir to create it`

---

## 8. Output Formats

### `-f json` (recommended for pipelines)

```json
{"data":{"id":"01JKM5P8X7QZYN3GT6ABCDEF01","title":"My Note Title","path":"/absolute/path/to/01JKM5P8X7-my-note-title.md"}}
```

Fields:
- `data.id` — Full 26-character ULID
- `data.title` — Note title as provided
- `data.path` — Absolute path to the created file

Parse with: `jq -r '.data.id'`, `jq -r '.data.path'`, etc.

### `-f paths`

Prints only the absolute file path on stdout, no other text:

```
/absolute/path/to/01JKM5P8X7-my-note-title.md
```

### `-f human` (default)

```
Created: My Note Title [01JKM5P8X7]
  /absolute/path/to/01JKM5P8X7-my-note-title.md
```

Not recommended for parsing — use `json` or `paths` instead.

---

## 9. Error Handling

- **Exit code**: Non-zero on any error.
- **Error output**: All error messages go to **stderr**.
- **Success output**: Goes to **stdout** in the selected format.

Common errors:

| Cause | Error message (stderr) |
|-------|----------------------|
| Empty title | `title cannot be empty` |
| Invalid topic chars | `invalid topic '...': topics must contain only alphanumeric characters, hyphens, underscores, and forward slashes` |
| Invalid tag chars | `invalid tag '...': tags must contain only alphanumeric characters, hyphens, and underscores (no spaces)` |
| Unknown kind | `unknown kind '...': valid kinds are generic, book, paper, transcript, article` |
| Invalid metadata JSON | `invalid metadata JSON: ...` |
| Missing `--into` dir | `directory does not exist: ...\n  hint: use --mkdir to create it` |
| Absolute `--into` | `--into must be a relative path, not absolute: ...` |
| `--into` with `..` | `--into cannot contain '..': ...` |
| `--file` not found | `failed to read file: ...` |
| Notes dir missing | `notes directory does not exist: ...` |

---

## 10. Concurrency

- **Safe for concurrent use.** Multiple `notes new` processes can run simultaneously.
- SQLite uses WAL journal mode with a 5-second busy timeout.
- Each note gets a unique ULID, so there are no filename collisions.
- Index updates are incremental and handle concurrent writes via SQLite's locking.

---

## 11. Full Pipeline Examples

### Example 1: Transcript pipeline

Process a video and create a transcript note with full metadata:

```bash
#!/usr/bin/env bash
set -euo pipefail

VIDEO_URL="https://youtube.com/watch?v=dQw4w9WgXcQ"
TITLE="Rick Astley Interview on Modern Music"

# Step 1: Generate transcript (your tool here)
TRANSCRIPT=$(yt-transcript "$VIDEO_URL")

# Step 2: Create the note
RESULT=$(echo "$TRANSCRIPT" | notes new "$TITLE" \
  --topic media/transcripts \
  --tag interview \
  --tag music \
  --desc "Transcript of Rick Astley discussing modern music trends" \
  --kind transcript \
  --metadata "$(jq -n \
    --arg src "$VIDEO_URL" \
    '{source: $src, speakers: ["Interviewer", "Rick Astley"], duration_seconds: 2400, chapters: [{title: "Intro", start_seconds: 0}, {title: "Career Reflections", start_seconds: 300}]}'
  )" \
  --into transcripts \
  --mkdir \
  --stdin \
  --no-edit \
  -f json)

# Step 3: Extract the note ID and path
NOTE_ID=$(echo "$RESULT" | jq -r '.data.id')
NOTE_PATH=$(echo "$RESULT" | jq -r '.data.path')

echo "Created note $NOTE_ID at $NOTE_PATH"
```

### Example 2: Article pipeline

Scrape a web article and create an article note:

```bash
#!/usr/bin/env bash
set -euo pipefail

URL="https://example.com/great-article"
TITLE="The Future of Systems Programming"

# Step 1: Extract article content (your tool here)
BODY=$(readability-cli "$URL")

# Step 2: Create the note
NOTE_PATH=$(echo "$BODY" | notes new "$TITLE" \
  --topic software/articles \
  --tag rust \
  --tag systems-programming \
  --desc "Overview of modern systems programming languages and trends" \
  --kind article \
  --metadata "$(jq -n \
    --arg url "$URL" \
    '{authors: ["Jane Doe"], url: $url, publication: "Tech Blog", date: "2025-12-01"}'
  )" \
  --stdin \
  --no-edit \
  -f paths)

echo "Created: $NOTE_PATH"
```
