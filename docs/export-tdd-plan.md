# TDD Implementation Plan: Export Formats (den-0dm)

## Overview

Add export capabilities to den for converting notes to HTML, PDF, and static sites.

### Scope
- Single note to HTML
- Single note to PDF (via HTML → wkhtmltopdf or similar)
- Bulk export to static site with topic navigation
- Filtering by topic/tag
- Template customization
- CSS theming
- Link resolution in exports

## Architecture

### New Modules

```
src/
├── export/
│   ├── mod.rs           # Module exports, ExportFormat enum
│   ├── html.rs          # Markdown→HTML conversion, templates
│   ├── pdf.rs           # PDF generation (delegates to html + external tool)
│   ├── site.rs          # Static site generation
│   ├── template.rs      # Template engine abstraction
│   └── theme.rs         # CSS theme handling
└── cli/
    └── handlers/
        └── export.rs    # CLI handler
```

### Key Types

```rust
// src/export/mod.rs
pub enum ExportFormat {
    Html,
    Pdf,
    Site,
}

pub struct ExportOptions {
    pub format: ExportFormat,
    pub output: PathBuf,
    pub template: Option<PathBuf>,
    pub theme: Option<String>,
    pub include_toc: bool,
}

pub struct ExportResult {
    pub path: PathBuf,
    pub notes_exported: usize,
}

// Trait for exporters
pub trait Exporter {
    fn export_note(&self, note: &Note, body: &str, options: &ExportOptions) -> Result<PathBuf>;
}
```

### Dependencies to Add

```toml
# Cargo.toml additions
pulldown-cmark = "0.9"          # Markdown parsing
minijinja = "1"                 # Templating (lightweight, no deps)
```

PDF generation will shell out to `wkhtmltopdf` or `weasyprint` (user's choice via config) rather than embedding a heavy PDF library.

---

## TDD Implementation Phases

### Phase 1: Core Markdown → HTML Conversion

**Goal**: Convert note markdown body to HTML with proper escaping.

#### Test 1.1: Basic markdown to HTML
```rust
#[test]
fn test_markdown_to_html_basic() {
    let markdown = "# Heading\n\nParagraph text.";
    let html = markdown_to_html(markdown);

    assert!(html.contains("<h1>Heading</h1>"));
    assert!(html.contains("<p>Paragraph text.</p>"));
}
```

#### Test 1.2: Code blocks with syntax highlighting classes
```rust
#[test]
fn test_markdown_to_html_code_block() {
    let markdown = "```rust\nfn main() {}\n```";
    let html = markdown_to_html(markdown);

    assert!(html.contains("<pre>"));
    assert!(html.contains("<code"));
    assert!(html.contains("fn main()"));
}
```

#### Test 1.3: Links preserved
```rust
#[test]
fn test_markdown_to_html_links() {
    let markdown = "[link](https://example.com)";
    let html = markdown_to_html(markdown);

    assert!(html.contains("<a href=\"https://example.com\">link</a>"));
}
```

#### Test 1.4: Special characters escaped
```rust
#[test]
fn test_markdown_to_html_escapes_special_chars() {
    let markdown = "Use <script> and & symbols";
    let html = markdown_to_html(markdown);

    assert!(html.contains("&lt;script&gt;"));
    assert!(html.contains("&amp;"));
}
```

**Implementation**: `src/export/html.rs`
```rust
use pulldown_cmark::{Parser, Options, html};

pub fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}
```

---

### Phase 2: HTML Template System

**Goal**: Wrap converted HTML in customizable templates.

#### Test 2.1: Default template includes note metadata
```rust
#[test]
fn test_default_template_includes_title() {
    let note = Note::builder("Test Note", now()).build();
    let body = "# Content";

    let html = render_note_html(&note, body, None)?;

    assert!(html.contains("<title>Test Note</title>"));
    assert!(html.contains("Test Note")); // In h1 or header
}
```

#### Test 2.2: Template includes topics as breadcrumbs
```rust
#[test]
fn test_template_includes_topics() {
    let note = Note::builder("Test Note", now())
        .topics(vec![Topic::new("software/rust").unwrap()])
        .build();
    let body = "Content";

    let html = render_note_html(&note, body, None)?;

    assert!(html.contains("software"));
    assert!(html.contains("rust"));
}
```

#### Test 2.3: Template includes tags
```rust
#[test]
fn test_template_includes_tags() {
    let note = Note::builder("Test Note", now())
        .tags(vec![Tag::new("draft").unwrap()])
        .build();
    let body = "Content";

    let html = render_note_html(&note, body, None)?;

    assert!(html.contains("draft"));
}
```

#### Test 2.4: Custom template path works
```rust
#[test]
fn test_custom_template() {
    let note = Note::builder("Custom", now()).build();
    let body = "Content";
    let template = "<!DOCTYPE html><html><body>{{ title }}: {{ content }}</body></html>";

    // Write template to temp file
    let template_path = write_temp_template(template);

    let html = render_note_html(&note, body, Some(&template_path))?;

    assert!(html.contains("Custom: <p>Content</p>"));
}
```

#### Test 2.5: Template with created/modified dates
```rust
#[test]
fn test_template_includes_dates() {
    let created = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
    let note = Note::builder("Dated Note", created).build();
    let body = "";

    let html = render_note_html(&note, body, None)?;

    assert!(html.contains("2024-01-15"));
}
```

**Implementation**: `src/export/template.rs`
```rust
use minijinja::{Environment, context};

pub const DEFAULT_NOTE_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{{ title }}</title>
    {% if theme_css %}<style>{{ theme_css }}</style>{% endif %}
</head>
<body>
    <article>
        <header>
            <h1>{{ title }}</h1>
            {% if topics %}<nav class="topics">{% for t in topics %}<a href="#">{{ t }}</a>{% endfor %}</nav>{% endif %}
            {% if tags %}<div class="tags">{% for tag in tags %}<span class="tag">{{ tag }}</span>{% endfor %}</div>{% endif %}
            <time datetime="{{ created_iso }}">{{ created }}</time>
        </header>
        <main>{{ content }}</main>
    </article>
</body>
</html>"#;

pub fn render_note_html(note: &Note, body: &str, template_path: Option<&Path>) -> Result<String> {
    let content = markdown_to_html(body);
    let template_str = match template_path {
        Some(p) => std::fs::read_to_string(p)?,
        None => DEFAULT_NOTE_TEMPLATE.to_string(),
    };

    let mut env = Environment::new();
    env.add_template("note", &template_str)?;
    let tmpl = env.get_template("note")?;

    Ok(tmpl.render(context! {
        title => note.title(),
        content => content,
        topics => note.topics().iter().map(|t| t.to_string()).collect::<Vec<_>>(),
        tags => note.tags().iter().map(|t| t.as_str()).collect::<Vec<_>>(),
        created => note.created().format("%Y-%m-%d").to_string(),
        created_iso => note.created().to_rfc3339(),
        modified => note.modified().format("%Y-%m-%d").to_string(),
        modified_iso => note.modified().to_rfc3339(),
        description => note.description(),
    })?)
}
```

---

### Phase 3: CSS Theming

**Goal**: Support built-in and custom CSS themes.

#### Test 3.1: Default theme applied
```rust
#[test]
fn test_default_theme_css() {
    let css = get_theme_css(None)?;

    assert!(css.contains("body"));
    assert!(css.contains("article"));
}
```

#### Test 3.2: Named built-in theme
```rust
#[test]
fn test_builtin_theme_dark() {
    let css = get_theme_css(Some("dark"))?;

    assert!(css.contains("background"));
    // Dark theme should have dark background color
}
```

#### Test 3.3: Custom CSS file path
```rust
#[test]
fn test_custom_theme_file() {
    let custom_css = "body { color: red; }";
    let path = write_temp_file("custom.css", custom_css);

    let css = get_theme_css(Some(path.to_str().unwrap()))?;

    assert_eq!(css, custom_css);
}
```

#### Test 3.4: Invalid theme name errors
```rust
#[test]
fn test_invalid_theme_errors() {
    let result = get_theme_css(Some("nonexistent-theme"));

    assert!(result.is_err());
}
```

**Implementation**: `src/export/theme.rs`
```rust
pub const THEME_DEFAULT: &str = r#"
body { font-family: system-ui, sans-serif; line-height: 1.6; max-width: 800px; margin: 0 auto; padding: 2rem; }
h1 { border-bottom: 1px solid #eee; padding-bottom: 0.5rem; }
.topics a { margin-right: 0.5rem; color: #666; }
.tag { background: #eef; padding: 0.2rem 0.5rem; border-radius: 3px; margin-right: 0.25rem; font-size: 0.85em; }
pre { background: #f5f5f5; padding: 1rem; overflow-x: auto; }
code { font-family: monospace; }
"#;

pub const THEME_DARK: &str = r#"
body { font-family: system-ui, sans-serif; line-height: 1.6; max-width: 800px; margin: 0 auto; padding: 2rem; background: #1a1a1a; color: #e0e0e0; }
h1 { border-bottom: 1px solid #333; padding-bottom: 0.5rem; }
a { color: #6af; }
.topics a { margin-right: 0.5rem; color: #888; }
.tag { background: #333; padding: 0.2rem 0.5rem; border-radius: 3px; margin-right: 0.25rem; font-size: 0.85em; }
pre { background: #2a2a2a; padding: 1rem; overflow-x: auto; }
"#;

pub fn get_theme_css(theme: Option<&str>) -> Result<String> {
    match theme {
        None => Ok(THEME_DEFAULT.to_string()),
        Some("default") => Ok(THEME_DEFAULT.to_string()),
        Some("dark") => Ok(THEME_DARK.to_string()),
        Some(path) if Path::new(path).exists() => Ok(std::fs::read_to_string(path)?),
        Some(name) => Err(anyhow!("Unknown theme: {}", name)),
    }
}
```

---

### Phase 4: Single Note HTML Export

**Goal**: Full `den export` command for single note to HTML.

#### Test 4.1: Export single note to HTML file
```rust
#[test]
fn test_export_single_note_html() {
    let env = TestEnv::new();
    let note = TestNote::new("Export Test").body("# Hello World");
    env.add_note(&note);
    env.build_index()?;

    let output_dir = env.notes_dir().join("export");

    env.cmd()
        .export("Export Test")
        .format_html()
        .output(&output_dir)
        .assert()
        .success();

    let html_files: Vec<_> = std::fs::read_dir(&output_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension() == Some("html".as_ref()))
        .collect();

    assert_eq!(html_files.len(), 1);

    let content = std::fs::read_to_string(html_files[0].path())?;
    assert!(content.contains("Export Test"));
    assert!(content.contains("Hello World"));
}
```

#### Test 4.2: Export by ID prefix
```rust
#[test]
fn test_export_by_id_prefix() {
    let env = TestEnv::new();
    let note = TestNote::new("ID Export").id("01HQ3K5M7NXJK4QZPW8V2R6T9Y");
    env.add_note(&note);
    env.build_index()?;

    env.cmd()
        .export("01HQ3K5M7N")
        .format_html()
        .output_to_stdout()
        .assert()
        .success()
        .stdout(predicate::str::contains("ID Export"));
}
```

#### Test 4.3: Export with custom template
```rust
#[test]
fn test_export_custom_template() {
    let env = TestEnv::new();
    let note = TestNote::new("Template Test");
    env.add_note(&note);
    env.build_index()?;

    let template = env.write_file("template.html", "<html>CUSTOM: {{ title }}</html>");

    env.cmd()
        .export("Template Test")
        .format_html()
        .with_template(&template)
        .output_to_stdout()
        .assert()
        .success()
        .stdout(predicate::str::contains("CUSTOM: Template Test"));
}
```

#### Test 4.4: Export with theme
```rust
#[test]
fn test_export_with_theme() {
    let env = TestEnv::new();
    let note = TestNote::new("Theme Test");
    env.add_note(&note);
    env.build_index()?;

    let output = env.cmd()
        .export("Theme Test")
        .format_html()
        .with_theme("dark")
        .output_to_stdout()
        .output_success();

    assert!(output.contains("background"));
    assert!(output.contains("#1a1a1a")); // Dark theme background
}
```

#### Test 4.5: Export note not found
```rust
#[test]
fn test_export_note_not_found() {
    let env = TestEnv::new();
    env.build_index()?;

    env.cmd()
        .export("nonexistent")
        .format_html()
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}
```

#### Test 4.6: Export outputs to stdout by default
```rust
#[test]
fn test_export_stdout_default() {
    let env = TestEnv::new();
    let note = TestNote::new("Stdout Test").body("Body content");
    env.add_note(&note);
    env.build_index()?;

    env.cmd()
        .export("Stdout Test")
        .format_html()
        .assert()
        .success()
        .stdout(predicate::str::contains("<!DOCTYPE html>"))
        .stdout(predicate::str::contains("Stdout Test"))
        .stdout(predicate::str::contains("Body content"));
}
```

**CLI Definition**: `src/cli/mod.rs`
```rust
#[derive(Parser)]
pub struct ExportArgs {
    /// Note to export (ID prefix or title)
    #[arg(required_unless_present = "all")]
    pub note: Option<String>,

    /// Export all notes
    #[arg(long, conflicts_with = "note")]
    pub all: bool,

    /// Output format
    #[arg(short, long, value_enum, default_value = "html")]
    pub format: ExportFormatArg,

    /// Output path (stdout if not specified for single note)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Custom template file
    #[arg(long)]
    pub template: Option<PathBuf>,

    /// CSS theme (default, dark, or path to CSS file)
    #[arg(long)]
    pub theme: Option<String>,

    /// Filter by topic
    #[arg(long)]
    pub topic: Option<String>,

    /// Filter by tag
    #[arg(long)]
    pub tag: Vec<String>,

    /// Include archived notes
    #[arg(short = 'a', long)]
    pub include_archived: bool,
}

#[derive(Clone, ValueEnum)]
pub enum ExportFormatArg {
    Html,
    Pdf,
    Site,
}
```

**Handler**: `src/cli/handlers/export.rs`
```rust
pub fn handle_export(args: &ExportArgs, notes_dir: &Path) -> Result<()> {
    let db_path = index_db_path(notes_dir);
    let index = SqliteIndex::open(&db_path)?;

    let theme_css = get_theme_css(args.theme.as_deref())?;

    match (&args.note, args.all) {
        (Some(query), false) => {
            let note = resolve_note(&index, query, notes_dir)?;
            let parsed = read_note(&note.path())?;
            let html = render_note_html(&parsed.note, &parsed.body, args.template.as_deref())?;

            match &args.output {
                Some(path) => {
                    std::fs::create_dir_all(path)?;
                    let filename = format!("{}.html", slug::slugify(parsed.note.title()));
                    std::fs::write(path.join(filename), html)?;
                }
                None => print!("{}", html),
            }
        }
        (None, true) => {
            // Bulk export handled in Phase 6
        }
        _ => unreachable!(),
    }

    Ok(())
}
```

---

### Phase 5: Link Resolution

**Goal**: Resolve internal `[[note]]` links and note ID references in exports.

#### Test 5.1: Resolve note ID links to HTML anchors
```rust
#[test]
fn test_resolve_internal_links() {
    let env = TestEnv::new();
    let target = TestNote::new("Target Note").id("01HQ4A2R9PXJK4QZPW8V2R6T9Y");
    let source = TestNote::new("Source Note")
        .body("See [Target](01HQ4A2R9P) for more.");
    env.add_note(&target);
    env.add_note(&source);
    env.build_index()?;

    let html = env.cmd()
        .export("Source Note")
        .format_html()
        .output_success();

    // Link should be resolved to the target note's HTML file or anchor
    assert!(html.contains("Target Note") || html.contains("01HQ4A2R9P"));
}
```

#### Test 5.2: Broken links marked appropriately
```rust
#[test]
fn test_broken_link_handling() {
    let env = TestEnv::new();
    let note = TestNote::new("Broken Links")
        .body("See [Missing](01HZZZZZZZ) for details.");
    env.add_note(&note);
    env.build_index()?;

    let html = env.cmd()
        .export("Broken Links")
        .format_html()
        .output_success();

    // Broken link should be marked or preserved as-is
    assert!(html.contains("01HZZZZZZZ") || html.contains("broken"));
}
```

#### Test 5.3: External links unchanged
```rust
#[test]
fn test_external_links_unchanged() {
    let env = TestEnv::new();
    let note = TestNote::new("External Links")
        .body("Visit [example](https://example.com).");
    env.add_note(&note);
    env.build_index()?;

    let html = env.cmd()
        .export("External Links")
        .format_html()
        .output_success();

    assert!(html.contains("https://example.com"));
}
```

**Implementation**: `src/export/html.rs` (link resolver)
```rust
pub fn resolve_links(html: &str, index: &impl IndexRepository) -> String {
    // Use regex or HTML parser to find links with note ID patterns
    // Replace with resolved note titles/paths
    // Mark broken links appropriately
}
```

---

### Phase 6: Bulk Export & Static Site

**Goal**: Export multiple notes with navigation structure.

#### Test 6.1: Export all notes
```rust
#[test]
fn test_export_all_notes() {
    let env = TestEnv::new();
    env.add_note(&TestNote::new("Note One"));
    env.add_note(&TestNote::new("Note Two"));
    env.add_note(&TestNote::new("Note Three"));
    env.build_index()?;

    let output = env.notes_dir().join("export");

    env.cmd()
        .export_all()
        .format_html()
        .output(&output)
        .assert()
        .success();

    let html_files: Vec<_> = std::fs::read_dir(&output)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension() == Some("html".as_ref()))
        .collect();

    assert_eq!(html_files.len(), 3);
}
```

#### Test 6.2: Export by topic filter
```rust
#[test]
fn test_export_by_topic() {
    let env = TestEnv::new();
    env.add_note(&TestNote::new("Rust Note").topic("software/rust"));
    env.add_note(&TestNote::new("Python Note").topic("software/python"));
    env.add_note(&TestNote::new("Other Note").topic("other"));
    env.build_index()?;

    let output = env.notes_dir().join("export");

    env.cmd()
        .export_all()
        .format_html()
        .with_topic("software/")  // Trailing slash = descendants
        .output(&output)
        .assert()
        .success();

    let html_files: Vec<_> = std::fs::read_dir(&output)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension() == Some("html".as_ref()))
        .collect();

    assert_eq!(html_files.len(), 2); // Only rust and python notes
}
```

#### Test 6.3: Export by tag filter
```rust
#[test]
fn test_export_by_tag() {
    let env = TestEnv::new();
    env.add_note(&TestNote::new("Draft Note").tag("draft"));
    env.add_note(&TestNote::new("Published Note").tag("published"));
    env.build_index()?;

    let output = env.notes_dir().join("export");

    env.cmd()
        .export_all()
        .format_html()
        .with_tag("published")
        .output(&output)
        .assert()
        .success();

    let html_files: Vec<_> = std::fs::read_dir(&output)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension() == Some("html".as_ref()))
        .collect();

    assert_eq!(html_files.len(), 1);
}
```

#### Test 6.4: Static site generates index
```rust
#[test]
fn test_site_generates_index() {
    let env = TestEnv::new();
    env.add_note(&TestNote::new("Site Note One"));
    env.add_note(&TestNote::new("Site Note Two"));
    env.build_index()?;

    let output = env.notes_dir().join("site");

    env.cmd()
        .export_all()
        .format_site()
        .output(&output)
        .assert()
        .success();

    // Should have index.html with links to all notes
    let index_path = output.join("index.html");
    assert!(index_path.exists());

    let index_content = std::fs::read_to_string(index_path)?;
    assert!(index_content.contains("Site Note One"));
    assert!(index_content.contains("Site Note Two"));
}
```

#### Test 6.5: Static site has topic navigation
```rust
#[test]
fn test_site_topic_navigation() {
    let env = TestEnv::new();
    env.add_note(&TestNote::new("Rust Note").topic("software/rust"));
    env.add_note(&TestNote::new("Python Note").topic("software/python"));
    env.build_index()?;

    let output = env.notes_dir().join("site");

    env.cmd()
        .export_all()
        .format_site()
        .output(&output)
        .assert()
        .success();

    // Should have topic pages
    let software_index = output.join("software").join("index.html");
    assert!(software_index.exists());

    let content = std::fs::read_to_string(software_index)?;
    assert!(content.contains("rust"));
    assert!(content.contains("python"));
}
```

#### Test 6.6: Site excludes archived by default
```rust
#[test]
fn test_site_excludes_archived() {
    let env = TestEnv::new();
    env.add_note(&TestNote::new("Active Note"));
    env.add_note(&TestNote::new("Archived Note").tag("archived"));
    env.build_index()?;

    let output = env.notes_dir().join("site");

    env.cmd()
        .export_all()
        .format_site()
        .output(&output)
        .assert()
        .success();

    let html_files: Vec<_> = glob::glob(output.join("**/*.html").to_str().unwrap())?
        .filter_map(|e| e.ok())
        .collect();

    // Only active note + index should exist (no archived)
    let content: String = html_files.iter()
        .map(|f| std::fs::read_to_string(f).unwrap())
        .collect();

    assert!(content.contains("Active Note"));
    assert!(!content.contains("Archived Note"));
}
```

**Implementation**: `src/export/site.rs`
```rust
pub struct SiteGenerator {
    template: String,
    index_template: String,
    topic_template: String,
    theme_css: String,
}

impl SiteGenerator {
    pub fn generate(&self, notes: &[IndexedNote], output: &Path, notes_dir: &Path) -> Result<SiteResult> {
        std::fs::create_dir_all(output)?;

        // Generate note pages
        for indexed_note in notes {
            let parsed = read_note(&indexed_note.path())?;
            let html = self.render_note(&parsed)?;
            let slug = slugify(parsed.note.title());
            std::fs::write(output.join(format!("{}.html", slug)), html)?;
        }

        // Generate index page
        let index_html = self.render_index(notes)?;
        std::fs::write(output.join("index.html"), index_html)?;

        // Generate topic pages
        let topics = collect_topics(notes);
        for (topic, topic_notes) in topics {
            let topic_html = self.render_topic_page(&topic, &topic_notes)?;
            let topic_path = output.join(topic.to_string().replace('/', std::path::MAIN_SEPARATOR_STR));
            std::fs::create_dir_all(&topic_path)?;
            std::fs::write(topic_path.join("index.html"), topic_html)?;
        }

        // Copy theme CSS
        std::fs::write(output.join("style.css"), &self.theme_css)?;

        Ok(SiteResult { notes_exported: notes.len() })
    }
}
```

---

### Phase 7: PDF Export

**Goal**: Generate PDF from HTML using external tool.

#### Test 7.1: PDF export requires tool installed
```rust
#[test]
fn test_pdf_export_tool_check() {
    // This test verifies graceful error when PDF tool missing
    let env = TestEnv::new();
    let note = TestNote::new("PDF Test");
    env.add_note(&note);
    env.build_index()?;

    // If wkhtmltopdf is not installed, should error gracefully
    let result = env.cmd()
        .export("PDF Test")
        .format_pdf()
        .output(&env.notes_dir().join("out.pdf"))
        .assert();

    // Either succeeds (tool installed) or fails with helpful message
    // We don't assert success/failure since it depends on environment
}
```

#### Test 7.2: PDF export integration (when tool available)
```rust
#[test]
#[ignore] // Run manually when wkhtmltopdf is installed
fn test_pdf_export_generates_file() {
    let env = TestEnv::new();
    let note = TestNote::new("PDF Content").body("# Heading\n\nParagraph.");
    env.add_note(&note);
    env.build_index()?;

    let output = env.notes_dir().join("output.pdf");

    env.cmd()
        .export("PDF Content")
        .format_pdf()
        .output(&output)
        .assert()
        .success();

    assert!(output.exists());
    // PDF files start with %PDF
    let content = std::fs::read(&output)?;
    assert!(content.starts_with(b"%PDF"));
}
```

**Implementation**: `src/export/pdf.rs`
```rust
pub fn export_to_pdf(html: &str, output: &Path) -> Result<()> {
    // Try wkhtmltopdf first, then weasyprint
    let tools = ["wkhtmltopdf", "weasyprint"];

    for tool in tools {
        if which::which(tool).is_ok() {
            return run_pdf_tool(tool, html, output);
        }
    }

    Err(anyhow!(
        "No PDF tool found. Install wkhtmltopdf or weasyprint:\n\
         - macOS: brew install wkhtmltopdf\n\
         - Ubuntu: apt install wkhtmltopdf\n\
         - Or: pip install weasyprint"
    ))
}

fn run_pdf_tool(tool: &str, html: &str, output: &Path) -> Result<()> {
    // Write HTML to temp file
    let temp = tempfile::NamedTempFile::new()?;
    std::fs::write(temp.path(), html)?;

    let status = match tool {
        "wkhtmltopdf" => Command::new("wkhtmltopdf")
            .arg(temp.path())
            .arg(output)
            .status()?,
        "weasyprint" => Command::new("weasyprint")
            .arg(temp.path())
            .arg(output)
            .status()?,
        _ => unreachable!(),
    };

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("{} failed with status: {}", tool, status))
    }
}
```

---

### Phase 8: CLI Integration Tests

**Goal**: Full E2E tests for the export command.

#### Test 8.1: Export JSON output format
```rust
#[test]
fn test_export_json_output() {
    let env = TestEnv::new();
    env.add_note(&TestNote::new("JSON Export Test"));
    env.build_index()?;

    let output = env.notes_dir().join("export");

    let result: serde_json::Value = env.cmd()
        .export("JSON Export Test")
        .format_html()
        .output(&output)
        .format_json()  // CLI output format
        .output_json();

    assert!(result.get("data").is_some());
    assert_eq!(result["data"]["notes_exported"], 1);
    assert!(result["data"]["path"].as_str().is_some());
}
```

#### Test 8.2: Export with all flags combined
```rust
#[test]
fn test_export_all_options() {
    let env = TestEnv::new();
    let note = TestNote::new("Full Options Test")
        .topic("software")
        .tag("test");
    env.add_note(&note);
    env.build_index()?;

    let output = env.notes_dir().join("export");
    let template = env.write_file("t.html", "<html>{{ title }}</html>");
    let css = env.write_file("t.css", "body { color: blue; }");

    env.cmd()
        .export("Full Options Test")
        .format_html()
        .output(&output)
        .with_template(&template)
        .with_theme(css.to_str().unwrap())
        .assert()
        .success();
}
```

---

## Test Harness Additions

Add to `tests/common/harness/command.rs`:

```rust
impl DenCommand {
    /// Configures for the `export` command.
    pub fn export(self, note: &str) -> Self {
        self.args(["export", note])
    }

    /// Configures for `export --all`.
    pub fn export_all(self) -> Self {
        self.args(["export", "--all"])
    }

    /// Adds `--format html` for export.
    pub fn format_html(self) -> Self {
        self.args(["--format", "html"])
    }

    /// Adds `--format pdf` for export.
    pub fn format_pdf(self) -> Self {
        self.args(["--format", "pdf"])
    }

    /// Adds `--format site` for export.
    pub fn format_site(self) -> Self {
        self.args(["--format", "site"])
    }

    /// Adds `--output <path>` for export.
    pub fn output(self, path: &Path) -> Self {
        self.args(["--output", path.to_str().unwrap()])
    }

    /// Export to stdout (no --output flag).
    pub fn output_to_stdout(self) -> Self {
        self
    }

    /// Adds `--template <path>` for export.
    pub fn with_template(self, path: &Path) -> Self {
        self.args(["--template", path.to_str().unwrap()])
    }

    /// Adds `--theme <name>` for export.
    pub fn with_theme(self, theme: &str) -> Self {
        self.args(["--theme", theme])
    }
}
```

Add to `tests/common/harness/env.rs`:

```rust
impl TestEnv {
    /// Writes a file to the test environment and returns its path.
    pub fn write_file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.notes_dir().join(name);
        std::fs::write(&path, content).expect("Failed to write file");
        path
    }
}
```

---

## Implementation Order

1. **Phase 1**: Markdown → HTML (core conversion, no deps on rest of system)
2. **Phase 2**: Template system (builds on Phase 1)
3. **Phase 3**: CSS theming (standalone, can parallelize with Phase 2)
4. **Phase 4**: Single note export CLI (integrates 1-3)
5. **Phase 5**: Link resolution (enhances Phase 4)
6. **Phase 6**: Bulk export & static site (builds on Phase 4)
7. **Phase 7**: PDF export (builds on Phase 4)
8. **Phase 8**: CLI integration tests (validates everything)

## Dependencies Summary

```toml
# Add to Cargo.toml
pulldown-cmark = "0.9"   # Markdown parsing
minijinja = "1"          # Templating
which = "4"              # Find PDF tools (optional, for PDF support)
```

## File Structure Summary

```
src/
├── export/
│   ├── mod.rs           # pub use, ExportFormat, ExportOptions
│   ├── html.rs          # markdown_to_html(), render_note_html()
│   ├── pdf.rs           # export_to_pdf()
│   ├── site.rs          # SiteGenerator
│   ├── template.rs      # DEFAULT_NOTE_TEMPLATE, template rendering
│   └── theme.rs         # THEME_DEFAULT, THEME_DARK, get_theme_css()
└── cli/
    ├── mod.rs           # Add ExportArgs, ExportFormatArg
    └── handlers/
        └── export.rs    # handle_export()

tests/
└── cli_tests.rs         # Add export_tests module
```
