//! Static site generation for bulk note export.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use minijinja::{context, Environment};

use crate::domain::Note;
use crate::export::html::markdown_to_html;
use crate::export::theme::get_theme_css;
use crate::index::IndexedNote;
use crate::infra::{read_note, slugify};

/// Default template for the site index page.
pub const DEFAULT_INDEX_TEMPLATE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{{ site_title }}</title>
    <link rel="stylesheet" href="style.css">
</head>
<body>
    <header>
        <h1>{{ site_title }}</h1>
        <p class="note-count">{{ notes | length }} note{% if notes | length != 1 %}s{% endif %}</p>
    </header>
    <main>
        {% if topics %}
        <nav class="topics-nav" aria-label="Browse by topic">
            <h2>Topics</h2>
            <ul>
            {% for topic in topics %}
                <li><a href="{{ topic.path }}/index.html">{{ topic.name }} <span class="count">({{ topic.count }})</span></a></li>
            {% endfor %}
            </ul>
        </nav>
        {% endif %}
        <section class="notes-list" aria-label="All notes">
            <h2>All Notes</h2>
            <ul>
            {% for note in notes %}
                <li>
                    <a href="{{ note.slug }}.html">{{ note.title }}</a>
                    {% if note.description %}<span class="description">{{ note.description }}</span>{% endif %}
                </li>
            {% endfor %}
            </ul>
        </section>
    </main>
    <footer>
        <p>Generated with den</p>
    </footer>
</body>
</html>"##;

/// Default template for topic index pages.
pub const DEFAULT_TOPIC_TEMPLATE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{{ topic }} - {{ site_title }}</title>
    <link rel="stylesheet" href="{{ root_path }}style.css">
</head>
<body>
    <header>
        <nav class="breadcrumb" aria-label="Breadcrumb">
            <a href="{{ root_path }}index.html">Home</a>
            {% for crumb in breadcrumbs %}
            <span aria-hidden="true">/</span>
            <a href="{{ crumb.path }}">{{ crumb.name }}</a>
            {% endfor %}
        </nav>
        <h1>{{ topic }}</h1>
        <p class="note-count">{{ notes | length }} note{% if notes | length != 1 %}s{% endif %}</p>
    </header>
    <main>
        {% if subtopics %}
        <nav class="subtopics" aria-label="Subtopics">
            <h2>Subtopics</h2>
            <ul>
            {% for sub in subtopics %}
                <li><a href="{{ sub.path }}/index.html">{{ sub.name }} <span class="count">({{ sub.count }})</span></a></li>
            {% endfor %}
            </ul>
        </nav>
        {% endif %}
        <section class="notes-list" aria-label="Notes in this topic">
            <h2>Notes</h2>
            <ul>
            {% for note in notes %}
                <li>
                    <a href="{{ root_path }}{{ note.slug }}.html">{{ note.title }}</a>
                    {% if note.description %}<span class="description">{{ note.description }}</span>{% endif %}
                </li>
            {% endfor %}
            </ul>
        </section>
    </main>
    <footer>
        <a href="{{ root_path }}index.html">&larr; Back to index</a>
    </footer>
</body>
</html>"##;

/// Default template for individual note pages in a site.
///
/// Features:
/// - Semantic HTML structure
/// - highlight.js for syntax highlighting (auto light/dark via media queries)
/// - Breadcrumb navigation back to topics
/// - Clean typography and metadata display
pub const DEFAULT_SITE_NOTE_TEMPLATE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{{ title }} - {{ site_title }}</title>
    <link rel="stylesheet" href="style.css">
    <!-- Syntax highlighting: GitHub theme with automatic light/dark switching -->
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/github.min.css" media="(prefers-color-scheme: light), (prefers-color-scheme: no-preference)">
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/github-dark.min.css" media="(prefers-color-scheme: dark)">
</head>
<body>
    <article>
        <header>
            <nav class="breadcrumb" aria-label="Breadcrumb">
                <a href="index.html">Home</a>
                {% for topic in topics %}
                <span aria-hidden="true">/</span>
                <a href="{{ topic.path }}/index.html">{{ topic.name }}</a>
                {% endfor %}
            </nav>
            <h1>{{ title }}</h1>
            {% if description %}
            <p class="description">{{ description }}</p>
            {% endif %}
            {% if tags %}
            <div class="tags" role="list" aria-label="Tags">
                {% for tag in tags %}<span class="tag" role="listitem">{{ tag }}</span>{% endfor %}
            </div>
            {% endif %}
            <div class="metadata">
                <time datetime="{{ created_iso }}">{{ created }}</time>
                {% if modified != created %}
                <span aria-hidden="true"> · </span>
                <span>Updated <time datetime="{{ modified_iso }}">{{ modified }}</time></span>
                {% endif %}
            </div>
        </header>
        <main>
            {{ content }}
        </main>
    </article>
    <footer>
        <a href="index.html">&larr; Back to index</a>
    </footer>
    <!-- Syntax highlighting initialization -->
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js"></script>
    <script>hljs.highlightAll();</script>
</body>
</html>"##;

/// Configuration for site generation.
pub struct SiteConfig<'a> {
    /// Site title for index pages.
    pub site_title: &'a str,
    /// Theme name or path to CSS file.
    pub theme: Option<&'a str>,
    /// Custom template for note pages.
    pub note_template: Option<&'a Path>,
}

impl Default for SiteConfig<'_> {
    fn default() -> Self {
        Self {
            site_title: "Notes",
            theme: None,
            note_template: None,
        }
    }
}

/// Result of site generation.
pub struct SiteResult {
    /// Number of notes exported.
    pub notes_exported: usize,
    /// Number of topic pages generated.
    pub topic_pages: usize,
}

/// Information about a note for template rendering.
#[derive(Clone)]
struct NoteInfo {
    title: String,
    slug: String,
    description: Option<String>,
}

/// Information about a topic for template rendering.
struct TopicInfo {
    name: String,
    path: String,
    count: usize,
}

/// Generates a static site from a list of notes.
pub fn generate_site(
    notes: &[IndexedNote],
    output_dir: &Path,
    notes_dir: &Path,
    config: &SiteConfig,
) -> Result<SiteResult> {
    std::fs::create_dir_all(output_dir)?;

    // Get theme CSS
    let theme_css = get_theme_css(config.theme)?;
    std::fs::write(output_dir.join("style.css"), &theme_css)?;

    // Collect note info and topic mapping
    let mut note_infos: Vec<NoteInfo> = Vec::new();
    let mut topic_notes: BTreeMap<String, Vec<NoteInfo>> = BTreeMap::new();

    // Export each note
    for indexed_note in notes {
        let file_path = notes_dir.join(indexed_note.path());
        let parsed = read_note(&file_path)?;

        let slug = slugify(parsed.note.title());
        let note_info = NoteInfo {
            title: parsed.note.title().to_string(),
            slug: slug.clone(),
            description: parsed.note.description().map(String::from),
        };

        note_infos.push(note_info.clone());

        // Map to topics
        for topic in parsed.note.topics() {
            topic_notes
                .entry(topic.to_string())
                .or_default()
                .push(note_info.clone());
        }

        // Render note page
        let html = render_site_note(&parsed.note, &parsed.body, config)?;
        std::fs::write(output_dir.join(format!("{}.html", slug)), html)?;
    }

    // Sort notes by title
    note_infos.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));

    // Collect top-level topics for index
    let mut top_topics: Vec<TopicInfo> = Vec::new();
    for (topic_path, topic_note_list) in &topic_notes {
        // Only include top-level topics in main index
        if !topic_path.contains('/') {
            top_topics.push(TopicInfo {
                name: topic_path.clone(),
                path: topic_path.clone(),
                count: topic_note_list.len(),
            });
        }
    }
    top_topics.sort_by(|a, b| a.name.cmp(&b.name));

    // Generate index page
    let index_html = render_index(&note_infos, &top_topics, config)?;
    std::fs::write(output_dir.join("index.html"), index_html)?;

    // Generate topic pages
    let mut topic_pages = 0;
    for (topic_path, topic_note_list) in &topic_notes {
        let topic_dir = output_dir.join(topic_path.replace('/', std::path::MAIN_SEPARATOR_STR));
        std::fs::create_dir_all(&topic_dir)?;

        // Find subtopics
        let prefix = format!("{}/", topic_path);
        let subtopics: Vec<TopicInfo> = topic_notes
            .keys()
            .filter(|k| k.starts_with(&prefix) && !k[prefix.len()..].contains('/'))
            .map(|k| {
                let name = k[prefix.len()..].to_string();
                TopicInfo {
                    name: name.clone(),
                    path: name,
                    count: topic_notes.get(k).map(|v| v.len()).unwrap_or(0),
                }
            })
            .collect();

        // Calculate root path (../ for each level of nesting)
        let depth = topic_path.matches('/').count() + 1;
        let root_path = "../".repeat(depth);

        // Build breadcrumbs
        let parts: Vec<&str> = topic_path.split('/').collect();
        let breadcrumbs: Vec<_> = parts
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let path = if i == parts.len() - 1 {
                    "index.html".to_string()
                } else {
                    let ups = "../".repeat(parts.len() - i - 1);
                    format!("{}index.html", ups)
                };
                serde_json::json!({
                    "name": name,
                    "path": path
                })
            })
            .collect();

        let topic_html = render_topic_page(
            topic_path,
            topic_note_list,
            &subtopics,
            &breadcrumbs,
            &root_path,
            config,
        )?;
        std::fs::write(topic_dir.join("index.html"), topic_html)?;
        topic_pages += 1;
    }

    Ok(SiteResult {
        notes_exported: notes.len(),
        topic_pages,
    })
}

/// Renders a note page for the static site.
fn render_site_note(note: &Note, body: &str, config: &SiteConfig) -> Result<String> {
    let content = markdown_to_html(body);

    let template_str = match config.note_template {
        Some(p) => std::fs::read_to_string(p)?,
        None => DEFAULT_SITE_NOTE_TEMPLATE.to_string(),
    };

    let mut env = Environment::new();
    env.add_template("note", &template_str)?;
    let tmpl = env.get_template("note")?;

    let topics: Vec<_> = note
        .topics()
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.to_string(),
                "path": t.to_string()
            })
        })
        .collect();

    let tags: Vec<&str> = note.tags().iter().map(|t| t.as_str()).collect();

    let html = tmpl.render(context! {
        site_title => config.site_title,
        title => note.title(),
        description => note.description(),
        content => content,
        topics => topics,
        tags => tags,
        created => note.created().format("%Y-%m-%d").to_string(),
        created_iso => note.created().to_rfc3339(),
        modified => note.modified().format("%Y-%m-%d").to_string(),
        modified_iso => note.modified().to_rfc3339(),
    })?;

    Ok(html)
}

/// Renders the main index page.
fn render_index(notes: &[NoteInfo], topics: &[TopicInfo], config: &SiteConfig) -> Result<String> {
    let mut env = Environment::new();
    env.add_template("index", DEFAULT_INDEX_TEMPLATE)?;
    let tmpl = env.get_template("index")?;

    let notes_json: Vec<_> = notes
        .iter()
        .map(|n| {
            serde_json::json!({
                "title": n.title,
                "slug": n.slug,
                "description": n.description
            })
        })
        .collect();

    let topics_json: Vec<_> = topics
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "path": t.path,
                "count": t.count
            })
        })
        .collect();

    let html = tmpl.render(context! {
        site_title => config.site_title,
        notes => notes_json,
        topics => topics_json,
    })?;

    Ok(html)
}

/// Renders a topic index page.
fn render_topic_page(
    topic: &str,
    notes: &[NoteInfo],
    subtopics: &[TopicInfo],
    breadcrumbs: &[serde_json::Value],
    root_path: &str,
    config: &SiteConfig,
) -> Result<String> {
    let mut env = Environment::new();
    env.add_template("topic", DEFAULT_TOPIC_TEMPLATE)?;
    let tmpl = env.get_template("topic")?;

    let notes_json: Vec<_> = notes
        .iter()
        .map(|n| {
            serde_json::json!({
                "title": n.title,
                "slug": n.slug,
                "description": n.description
            })
        })
        .collect();

    let subtopics_json: Vec<_> = subtopics
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "path": t.path,
                "count": t.count
            })
        })
        .collect();

    let html = tmpl.render(context! {
        site_title => config.site_title,
        topic => topic,
        notes => notes_json,
        subtopics => subtopics_json,
        breadcrumbs => breadcrumbs,
        root_path => root_path,
    })?;

    Ok(html)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{NoteId, Topic};
    use crate::infra::ContentHash;
    use chrono::Utc;
    use tempfile::TempDir;

    /// Helper to create an indexed note for testing.
    fn create_test_note(
        notes_dir: &Path,
        title: &str,
        body: &str,
        topics: &[&str],
    ) -> IndexedNote {
        let id = NoteId::new();
        let now = Utc::now();

        let topic_objs: Vec<Topic> = topics.iter().map(|t| Topic::new(t).unwrap()).collect();

        let note = Note::builder(id.clone(), title, now, now)
            .topics(topic_objs.clone())
            .build()
            .unwrap();

        let filename = format!("{}-{}.md", id.prefix(), slugify(title));
        let file_path = notes_dir.join(&filename);
        crate::infra::write_note(&file_path, &note, body).unwrap();

        let content_hash = ContentHash::compute(std::fs::read_to_string(&file_path).unwrap().as_bytes());

        IndexedNote::builder(id, title, now, now, filename.into(), content_hash)
            .topics(topic_objs)
            .build()
    }

    #[test]
    fn test_generate_site_creates_files() {
        let temp_dir = TempDir::new().unwrap();
        let notes_dir = TempDir::new().unwrap();

        let indexed = create_test_note(notes_dir.path(), "Test Note", "# Hello\n\nWorld", &[]);

        let config = SiteConfig::default();
        let result = generate_site(&[indexed], temp_dir.path(), notes_dir.path(), &config).unwrap();

        assert_eq!(result.notes_exported, 1);
        assert!(temp_dir.path().join("index.html").exists());
        assert!(temp_dir.path().join("style.css").exists());
        assert!(temp_dir.path().join("test-note.html").exists());
    }

    #[test]
    fn test_generate_site_with_topics() {
        let temp_dir = TempDir::new().unwrap();
        let notes_dir = TempDir::new().unwrap();

        let indexed1 =
            create_test_note(notes_dir.path(), "Rust Guide", "Rust content", &["software/rust"]);
        let indexed2 = create_test_note(
            notes_dir.path(),
            "Python Guide",
            "Python content",
            &["software/python"],
        );

        let config = SiteConfig::default();
        let result =
            generate_site(&[indexed1, indexed2], temp_dir.path(), notes_dir.path(), &config)
                .unwrap();

        assert_eq!(result.notes_exported, 2);
        assert!(result.topic_pages > 0);

        // Check topic directories exist
        assert!(temp_dir.path().join("software").join("rust").exists());
        assert!(temp_dir.path().join("software").join("python").exists());
    }

    #[test]
    fn test_index_page_contains_notes() {
        let temp_dir = TempDir::new().unwrap();
        let notes_dir = TempDir::new().unwrap();

        let indexed = create_test_note(notes_dir.path(), "Important Note", "Content", &[]);

        let config = SiteConfig::default();
        generate_site(&[indexed], temp_dir.path(), notes_dir.path(), &config).unwrap();

        let index_content = std::fs::read_to_string(temp_dir.path().join("index.html")).unwrap();
        assert!(index_content.contains("Important Note"));
        assert!(index_content.contains("important-note.html"));
    }

    #[test]
    fn test_site_with_dark_theme() {
        let temp_dir = TempDir::new().unwrap();
        let notes_dir = TempDir::new().unwrap();

        let indexed = create_test_note(notes_dir.path(), "Dark Note", "Content", &[]);

        let config = SiteConfig {
            site_title: "Dark Site",
            theme: Some("dark"),
            note_template: None,
        };
        generate_site(&[indexed], temp_dir.path(), notes_dir.path(), &config).unwrap();

        let css_content = std::fs::read_to_string(temp_dir.path().join("style.css")).unwrap();
        // Unified theme with CSS custom properties and automatic dark mode
        assert!(css_content.contains("--color-bg: #0d1117")); // Dark mode background in media query
        assert!(css_content.contains("prefers-color-scheme: dark"));
    }
}
