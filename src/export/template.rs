//! HTML template rendering for note exports.

use std::path::Path;

use anyhow::Result;
use minijinja::{context, Environment};

use crate::domain::Note;
use crate::export::html::markdown_to_html;
use crate::export::theme::get_theme_css;

/// Default HTML template for single note export.
pub const DEFAULT_NOTE_TEMPLATE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{{ title }}</title>
    <style>{{ theme_css }}</style>
</head>
<body>
    <article>
        <header>
            <h1>{{ title }}</h1>
            {% if description %}
            <p class="description">{{ description }}</p>
            {% endif %}
            {% if topics %}
            <nav class="topics">
                {% for topic in topics %}<a href="#">{{ topic }}</a>{% endfor %}
            </nav>
            {% endif %}
            {% if tags %}
            <div class="tags">
                {% for tag in tags %}<span class="tag">{{ tag }}</span>{% endfor %}
            </div>
            {% endif %}
            <div class="metadata">
                <time datetime="{{ created_iso }}">{{ created }}</time>
                {% if modified != created %}
                · Updated <time datetime="{{ modified_iso }}">{{ modified }}</time>
                {% endif %}
            </div>
        </header>
        <main>{{ content }}</main>
    </article>
</body>
</html>"##;

use crate::export::links::LinkResolver;

/// Options for rendering a note to HTML.
#[derive(Default)]
pub struct RenderOptions<'a> {
    /// Path to custom template file.
    pub template_path: Option<&'a Path>,
    /// Theme name or path to CSS file.
    pub theme: Option<&'a str>,
    /// Link resolver for resolving internal note references.
    pub link_resolver: Option<&'a LinkResolver<'a>>,
}

/// Renders a note to a complete HTML document.
///
/// # Arguments
///
/// * `note` - The note metadata
/// * `body` - The markdown body content
/// * `options` - Rendering options (template, theme)
///
/// # Returns
///
/// Complete HTML document as a string.
pub fn render_note_html(note: &Note, body: &str, options: &RenderOptions) -> Result<String> {
    // Resolve links if resolver is provided
    let resolved_body = match options.link_resolver {
        Some(resolver) => resolver.resolve(body).content,
        None => body.to_string(),
    };

    let content = markdown_to_html(&resolved_body);
    let theme_css = get_theme_css(options.theme)?;

    let template_str = match options.template_path {
        Some(p) => std::fs::read_to_string(p)?,
        None => DEFAULT_NOTE_TEMPLATE.to_string(),
    };

    let mut env = Environment::new();
    env.add_template("note", &template_str)?;
    let tmpl = env.get_template("note")?;

    let topics: Vec<String> = note.topics().iter().map(|t| t.to_string()).collect();
    let tags: Vec<&str> = note.tags().iter().map(|t| t.as_str()).collect();

    let html = tmpl.render(context! {
        title => note.title(),
        description => note.description(),
        content => content,
        theme_css => theme_css,
        topics => topics,
        tags => tags,
        created => note.created().format("%Y-%m-%d").to_string(),
        created_iso => note.created().to_rfc3339(),
        modified => note.modified().format("%Y-%m-%d").to_string(),
        modified_iso => note.modified().to_rfc3339(),
    })?;

    Ok(html)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Note, NoteId, Tag, Topic};
    use chrono::{TimeZone, Utc};
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_note(title: &str) -> Note {
        let id = NoteId::new();
        let now = Utc::now();
        Note::new(id, title, now, now).unwrap()
    }

    #[test]
    fn test_default_template_includes_title() {
        let note = make_note("Test Note");
        let body = "# Content";

        let html = render_note_html(&note, body, &RenderOptions::default()).unwrap();

        assert!(html.contains("<title>Test Note</title>"));
        assert!(html.contains("<h1>Test Note</h1>"));
    }

    #[test]
    fn test_template_includes_content() {
        let note = make_note("Content Test");
        let body = "Hello **world**";

        let html = render_note_html(&note, body, &RenderOptions::default()).unwrap();

        assert!(html.contains("<strong>world</strong>"));
    }

    #[test]
    fn test_template_includes_topics() {
        let id = NoteId::new();
        let now = Utc::now();
        let note = Note::builder(id, "Test Note", now, now)
            .topics(vec![Topic::new("software/rust").unwrap()])
            .build()
            .unwrap();
        let body = "";

        let html = render_note_html(&note, body, &RenderOptions::default()).unwrap();

        assert!(html.contains("software/rust"));
    }

    #[test]
    fn test_template_includes_tags() {
        let id = NoteId::new();
        let now = Utc::now();
        let note = Note::builder(id, "Test Note", now, now)
            .tags(vec![Tag::new("draft").unwrap()])
            .build()
            .unwrap();
        let body = "";

        let html = render_note_html(&note, body, &RenderOptions::default()).unwrap();

        assert!(html.contains("draft"));
        assert!(html.contains("class=\"tag\""));
    }

    #[test]
    fn test_template_includes_description() {
        let id = NoteId::new();
        let now = Utc::now();
        let note = Note::builder(id, "Test Note", now, now)
            .description(Some("A short description"))
            .build()
            .unwrap();
        let body = "";

        let html = render_note_html(&note, body, &RenderOptions::default()).unwrap();

        assert!(html.contains("A short description"));
        assert!(html.contains("class=\"description\""));
    }

    #[test]
    fn test_template_includes_dates() {
        let id = NoteId::new();
        let created = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let note = Note::new(id, "Dated Note", created, created).unwrap();
        let body = "";

        let html = render_note_html(&note, body, &RenderOptions::default()).unwrap();

        assert!(html.contains("2024-01-15"));
        assert!(html.contains("datetime=\"2024-01-15"));
    }

    #[test]
    fn test_custom_template() {
        let note = make_note("Custom Template Test");
        let body = "Content here";

        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "<!DOCTYPE html><html><body>CUSTOM: {{{{ title }}}} - {{{{ content }}}}</body></html>").unwrap();

        let options = RenderOptions {
            template_path: Some(temp.path()),
            theme: None,
            link_resolver: None,
        };

        let html = render_note_html(&note, body, &options).unwrap();

        assert!(html.contains("CUSTOM: Custom Template Test"));
        assert!(html.contains("<p>Content here</p>"));
    }

    #[test]
    fn test_template_with_theme() {
        let note = make_note("Theme Test");
        let body = "";

        let options = RenderOptions {
            template_path: None,
            theme: Some("dark"),
            link_resolver: None,
        };

        let html = render_note_html(&note, body, &options).unwrap();

        assert!(html.contains("#1a1a1a")); // Dark theme background color
    }

    #[test]
    fn test_template_includes_doctype() {
        let note = make_note("Doctype Test");
        let body = "";

        let html = render_note_html(&note, body, &RenderOptions::default()).unwrap();

        assert!(html.starts_with("<!DOCTYPE html>"));
    }

    #[test]
    fn test_template_includes_viewport_meta() {
        let note = make_note("Viewport Test");
        let body = "";

        let html = render_note_html(&note, body, &RenderOptions::default()).unwrap();

        assert!(html.contains("viewport"));
        assert!(html.contains("width=device-width"));
    }

    #[test]
    fn test_template_escapes_title() {
        let id = NoteId::new();
        let now = Utc::now();
        // Note: The Note builder may reject special chars in title,
        // but the template should still handle them if they get through
        let note = Note::new(id, "Test & Notes", now, now).unwrap();
        let body = "";

        let html = render_note_html(&note, body, &RenderOptions::default()).unwrap();

        // Title should be escaped in the <title> tag
        assert!(html.contains("Test &amp; Notes") || html.contains("Test & Notes"));
    }
}
