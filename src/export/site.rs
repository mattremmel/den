//! Static site generation for bulk note export.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::Result;
use minijinja::{context, Environment};
use serde::Serialize;

use crate::domain::Note;
use crate::export::html::markdown_to_html;
use crate::export::theme::get_theme_css;
use crate::index::IndexedNote;
use crate::infra::{read_note, slugify};

// =============================================================================
// JSON Index Types (for client-side filtering)
// =============================================================================

/// Entry for a note in the JSON index.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct NoteIndexEntry {
    pub id: String,
    pub title: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub topics: Vec<String>,
    pub tags: Vec<String>,
    pub created: String,
    pub modified: String,
}

/// Node in the topic tree.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct TopicNode {
    pub count: usize,
    pub children: Vec<String>,
}

/// Tag with count for the JSON index.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct TagCount {
    pub name: String,
    pub count: usize,
}

/// Complete site index for client-side search and filtering.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct SiteIndex {
    pub notes: Vec<NoteIndexEntry>,
    pub topics: BTreeMap<String, TopicNode>,
    pub tags: Vec<TagCount>,
}

/// Default template for the site index page.
pub const DEFAULT_INDEX_TEMPLATE: &str = r##"<!DOCTYPE html>
<html lang="en" class="no-js">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{{ site_title }}</title>
    <link rel="stylesheet" href="style.css">
    <script>document.documentElement.classList.replace('no-js', 'js');</script>
</head>
<body>
    <div class="site-layout">
        <aside class="sidebar" id="sidebar" aria-label="Filters">
            <button class="sidebar-close" id="sidebar-close" aria-label="Close sidebar">&times;</button>
            <div class="sidebar-search">
                <input type="search" id="search-input" placeholder="Search notes..." aria-label="Search notes">
            </div>
            <nav class="sidebar-topics" aria-label="Filter by topic">
                <h3>Topics</h3>
                <ul class="topic-tree" id="topic-tree">
                {% for topic in all_topics %}
                    <li class="topic-item{% if topic.children %} has-children{% endif %}" data-topic="{{ topic.path }}">
                        {% if topic.children %}<button class="topic-toggle" aria-expanded="false" aria-label="Expand {{ topic.name }}">▶</button>{% endif %}
                        <a href="{{ topic.path }}/index.html" class="topic-link" data-topic="{{ topic.path }}">{{ topic.name }} <span class="count">({{ topic.count }})</span></a>
                        {% if topic.children %}
                        <ul class="topic-children">
                        {% for child in topic.children %}
                            <li class="topic-item" data-topic="{{ child.path }}">
                                <a href="{{ child.path }}/index.html" class="topic-link" data-topic="{{ child.path }}">{{ child.name }} <span class="count">({{ child.count }})</span></a>
                            </li>
                        {% endfor %}
                        </ul>
                        {% endif %}
                    </li>
                {% endfor %}
                </ul>
            </nav>
            {% if all_tags %}
            <nav class="sidebar-tags" aria-label="Filter by tag">
                <h3>Tags</h3>
                <div class="tag-cloud" id="tag-cloud">
                {% for tag in all_tags %}
                    <button class="tag-filter" data-tag="{{ tag.name }}">{{ tag.name }} <span class="count">({{ tag.count }})</span></button>
                {% endfor %}
                </div>
            </nav>
            {% endif %}
            <div class="active-filters" id="active-filters" hidden>
                <h3>Active Filters</h3>
                <div class="filter-chips" id="filter-chips"></div>
                <button class="clear-filters" id="clear-filters">Clear All</button>
            </div>
        </aside>
        <div class="main-content">
            <header>
                <button class="sidebar-toggle" id="sidebar-toggle" aria-label="Open filters" aria-expanded="false">☰ Filters</button>
                <h1>{{ site_title }}</h1>
                <p class="note-count" id="note-count">{{ notes | length }} note{% if notes | length != 1 %}s{% endif %}</p>
            </header>
            <main>
                {% if topics %}
                <nav class="topics-nav no-js-only" aria-label="Browse by topic">
                    <h2>Topics</h2>
                    <ul>
                    {% for topic in topics %}
                        <li><a href="{{ topic.path }}/index.html">{{ topic.name }} <span class="count">({{ topic.count }})</span></a></li>
                    {% endfor %}
                    </ul>
                </nav>
                {% endif %}
                <section class="notes-list" aria-label="All notes">
                    <h2 class="js-only">Notes</h2>
                    <h2 class="no-js-only">All Notes</h2>
                    <ul id="notes-list">
                    {% for note in notes %}
                        <li class="note-item" data-id="{{ note.id }}" data-title="{{ note.title }}" data-description="{{ note.description }}" data-topics="{{ note.topics | join(',') }}" data-tags="{{ note.tags | join(',') }}">
                            <a href="{{ note.slug }}.html">{{ note.title }}</a>
                            {% if note.description %}<span class="description">{{ note.description }}</span>{% endif %}
                            {% if note.tags %}<div class="note-tags">{% for tag in note.tags %}<span class="tag">{{ tag }}</span>{% endfor %}</div>{% endif %}
                        </li>
                    {% endfor %}
                    </ul>
                    <p class="no-results" id="no-results" hidden>No notes match your filters.</p>
                </section>
            </main>
            <footer>
                <p>Generated with den</p>
            </footer>
        </div>
    </div>
    <div class="sidebar-backdrop" id="sidebar-backdrop" hidden></div>
    <script src="sidebar.js"></script>
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

/// JavaScript for interactive sidebar filtering.
pub const SIDEBAR_JS: &str = r##"// Sidebar interactive filtering for den static sites
(function() {
    'use strict';

    // State
    let searchQuery = '';
    let selectedTopic = null;
    let selectedTags = new Set();

    // DOM elements
    const searchInput = document.getElementById('search-input');
    const topicTree = document.getElementById('topic-tree');
    const tagCloud = document.getElementById('tag-cloud');
    const notesList = document.getElementById('notes-list');
    const noteCount = document.getElementById('note-count');
    const noResults = document.getElementById('no-results');
    const activeFilters = document.getElementById('active-filters');
    const filterChips = document.getElementById('filter-chips');
    const clearFilters = document.getElementById('clear-filters');
    const sidebar = document.getElementById('sidebar');
    const sidebarToggle = document.getElementById('sidebar-toggle');
    const sidebarClose = document.getElementById('sidebar-close');
    const sidebarBackdrop = document.getElementById('sidebar-backdrop');

    // Debounce helper
    function debounce(fn, ms) {
        let timeout;
        return function(...args) {
            clearTimeout(timeout);
            timeout = setTimeout(() => fn.apply(this, args), ms);
        };
    }

    // Filter notes based on current state
    function filterNotes() {
        const items = notesList.querySelectorAll('.note-item');
        let visibleCount = 0;
        const query = searchQuery.toLowerCase();

        items.forEach(item => {
            const title = (item.dataset.title || '').toLowerCase();
            const description = (item.dataset.description || '').toLowerCase();
            const topics = (item.dataset.topics || '').split(',').filter(Boolean);
            const tags = (item.dataset.tags || '').split(',').filter(Boolean);

            // Search filter
            const matchesSearch = !query ||
                title.includes(query) ||
                description.includes(query);

            // Topic filter (includes descendants)
            const matchesTopic = !selectedTopic ||
                topics.some(t => t === selectedTopic || t.startsWith(selectedTopic + '/'));

            // Tag filter (AND logic - must have all selected tags)
            const matchesTags = selectedTags.size === 0 ||
                [...selectedTags].every(tag => tags.includes(tag));

            const visible = matchesSearch && matchesTopic && matchesTags;
            item.hidden = !visible;
            if (visible) visibleCount++;
        });

        // Update count and no-results message
        const noun = visibleCount === 1 ? 'note' : 'notes';
        noteCount.textContent = visibleCount + ' ' + noun;
        noResults.hidden = visibleCount > 0;

        // Update active filters display
        updateActiveFilters();
        updateURLHash();
    }

    // Update the active filters chip display
    function updateActiveFilters() {
        const hasFilters = searchQuery || selectedTopic || selectedTags.size > 0;
        activeFilters.hidden = !hasFilters;

        if (!hasFilters) return;

        filterChips.innerHTML = '';

        if (searchQuery) {
            const chip = createChip('search', '"' + searchQuery + '"', () => {
                searchQuery = '';
                searchInput.value = '';
                filterNotes();
            });
            filterChips.appendChild(chip);
        }

        if (selectedTopic) {
            const chip = createChip('topic', selectedTopic, () => {
                clearTopicSelection();
                filterNotes();
            });
            filterChips.appendChild(chip);
        }

        selectedTags.forEach(tag => {
            const chip = createChip('tag', tag, () => {
                selectedTags.delete(tag);
                const btn = tagCloud.querySelector('[data-tag="' + tag + '"]');
                if (btn) btn.classList.remove('active');
                filterNotes();
            });
            filterChips.appendChild(chip);
        });
    }

    // Create a filter chip element
    function createChip(type, value, onRemove) {
        const chip = document.createElement('span');
        chip.className = 'filter-chip';
        chip.innerHTML = '<span class="chip-label">' + type + ': ' + escapeHtml(value) + '</span><button class="chip-remove" aria-label="Remove filter">&times;</button>';
        chip.querySelector('.chip-remove').addEventListener('click', onRemove);
        return chip;
    }

    // Escape HTML for safe display
    function escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    // Clear topic selection
    function clearTopicSelection() {
        selectedTopic = null;
        topicTree.querySelectorAll('.topic-link.active').forEach(el => el.classList.remove('active'));
    }

    // Update URL hash with current filter state
    function updateURLHash() {
        const params = new URLSearchParams();
        if (searchQuery) params.set('q', searchQuery);
        if (selectedTopic) params.set('topic', selectedTopic);
        if (selectedTags.size > 0) params.set('tags', [...selectedTags].join(','));

        const hash = params.toString();
        if (hash) {
            history.replaceState(null, '', '#' + hash);
        } else {
            history.replaceState(null, '', window.location.pathname);
        }
    }

    // Parse URL hash and restore filter state
    function parseURLHash() {
        const hash = window.location.hash.slice(1);
        if (!hash) return;

        const params = new URLSearchParams(hash);

        if (params.has('q')) {
            searchQuery = params.get('q');
            searchInput.value = searchQuery;
        }

        if (params.has('topic')) {
            selectedTopic = params.get('topic');
            const link = topicTree.querySelector('[data-topic="' + selectedTopic + '"]');
            if (link) link.classList.add('active');
        }

        if (params.has('tags')) {
            params.get('tags').split(',').forEach(tag => {
                selectedTags.add(tag);
                const btn = tagCloud.querySelector('[data-tag="' + tag + '"]');
                if (btn) btn.classList.add('active');
            });
        }

        filterNotes();
    }

    // Mobile sidebar toggle
    function openSidebar() {
        sidebar.classList.add('open');
        sidebarBackdrop.hidden = false;
        sidebarToggle.setAttribute('aria-expanded', 'true');
        document.body.style.overflow = 'hidden';
    }

    function closeSidebar() {
        sidebar.classList.remove('open');
        sidebarBackdrop.hidden = true;
        sidebarToggle.setAttribute('aria-expanded', 'false');
        document.body.style.overflow = '';
    }

    // Event listeners
    if (searchInput) {
        searchInput.addEventListener('input', debounce(function() {
            searchQuery = this.value.trim();
            filterNotes();
        }, 200));
    }

    if (topicTree) {
        // Topic toggle buttons
        topicTree.querySelectorAll('.topic-toggle').forEach(btn => {
            btn.addEventListener('click', function(e) {
                e.stopPropagation();
                const item = this.closest('.topic-item');
                const expanded = this.getAttribute('aria-expanded') === 'true';
                this.setAttribute('aria-expanded', !expanded);
                item.classList.toggle('expanded');
            });
        });

        // Topic links (filter, don't navigate)
        topicTree.querySelectorAll('.topic-link').forEach(link => {
            link.addEventListener('click', function(e) {
                e.preventDefault();
                const topic = this.dataset.topic;

                if (selectedTopic === topic) {
                    // Toggle off
                    clearTopicSelection();
                } else {
                    clearTopicSelection();
                    selectedTopic = topic;
                    this.classList.add('active');
                }
                filterNotes();
            });
        });
    }

    if (tagCloud) {
        tagCloud.querySelectorAll('.tag-filter').forEach(btn => {
            btn.addEventListener('click', function() {
                const tag = this.dataset.tag;

                if (selectedTags.has(tag)) {
                    selectedTags.delete(tag);
                    this.classList.remove('active');
                } else {
                    selectedTags.add(tag);
                    this.classList.add('active');
                }
                filterNotes();
            });
        });
    }

    if (clearFilters) {
        clearFilters.addEventListener('click', function() {
            searchQuery = '';
            selectedTopic = null;
            selectedTags.clear();

            if (searchInput) searchInput.value = '';
            topicTree.querySelectorAll('.topic-link.active').forEach(el => el.classList.remove('active'));
            tagCloud.querySelectorAll('.tag-filter.active').forEach(el => el.classList.remove('active'));

            filterNotes();
        });
    }

    // Mobile sidebar
    if (sidebarToggle) {
        sidebarToggle.addEventListener('click', openSidebar);
    }
    if (sidebarClose) {
        sidebarClose.addEventListener('click', closeSidebar);
    }
    if (sidebarBackdrop) {
        sidebarBackdrop.addEventListener('click', closeSidebar);
    }

    // Handle escape key
    document.addEventListener('keydown', function(e) {
        if (e.key === 'Escape' && sidebar.classList.contains('open')) {
            closeSidebar();
        }
    });

    // Initialize from URL hash
    parseURLHash();

    // Handle browser back/forward
    window.addEventListener('hashchange', parseURLHash);
})();
"##;

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
    id: String,
    title: String,
    slug: String,
    description: Option<String>,
    topics: Vec<String>,
    tags: Vec<String>,
    created: String,
    modified: String,
}

/// Information about a topic for template rendering.
struct TopicInfo {
    name: String,
    path: String,
    count: usize,
}

/// Information about a topic with children for sidebar tree rendering.
struct TopicTreeItem {
    name: String,
    path: String,
    count: usize,
    children: Vec<TopicTreeItem>,
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

    // Write sidebar JavaScript
    std::fs::write(output_dir.join("sidebar.js"), SIDEBAR_JS)?;

    // Collect note info, topic mapping, and tag counts
    let mut note_infos: Vec<NoteInfo> = Vec::new();
    let mut topic_notes: BTreeMap<String, Vec<NoteInfo>> = BTreeMap::new();
    let mut tag_counts: HashMap<String, usize> = HashMap::new();

    // Export each note
    for indexed_note in notes {
        let file_path = notes_dir.join(indexed_note.path());
        let parsed = read_note(&file_path)?;

        let slug = slugify(parsed.note.title());
        let topics_strs: Vec<String> = parsed.note.topics().iter().map(|t| t.to_string()).collect();
        let tags_strs: Vec<String> = parsed.note.tags().iter().map(|t| t.to_string()).collect();

        let note_info = NoteInfo {
            id: parsed.note.id().prefix().to_string(),
            title: parsed.note.title().to_string(),
            slug: slug.clone(),
            description: parsed.note.description().map(String::from),
            topics: topics_strs.clone(),
            tags: tags_strs.clone(),
            created: parsed.note.created().format("%Y-%m-%d").to_string(),
            modified: parsed.note.modified().format("%Y-%m-%d").to_string(),
        };

        note_infos.push(note_info.clone());

        // Count tags
        for tag in &tags_strs {
            *tag_counts.entry(tag.clone()).or_insert(0) += 1;
        }

        // Map to topics
        for topic in &topics_strs {
            topic_notes
                .entry(topic.clone())
                .or_default()
                .push(note_info.clone());
        }

        // Render note page
        let html = render_site_note(&parsed.note, &parsed.body, config)?;
        std::fs::write(output_dir.join(format!("{}.html", slug)), html)?;
    }

    // Sort notes by title
    note_infos.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));

    // Build tag list sorted by count (descending), then name
    let mut all_tags: Vec<TagCount> = tag_counts
        .into_iter()
        .map(|(name, count)| TagCount { name, count })
        .collect();
    all_tags.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));

    // Collect top-level topics for index (no-JS fallback)
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

    // Build topic tree for sidebar (top-level with immediate children)
    let topic_tree = build_topic_tree(&topic_notes);

    // Write JSON index for client-side filtering
    write_index_json(&note_infos, &topic_notes, &all_tags, output_dir)?;

    // Generate index page
    let index_html = render_index(&note_infos, &top_topics, &topic_tree, &all_tags, config)?;
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

/// Builds a hierarchical topic tree for the sidebar.
fn build_topic_tree(topic_notes: &BTreeMap<String, Vec<NoteInfo>>) -> Vec<TopicTreeItem> {
    let mut tree: Vec<TopicTreeItem> = Vec::new();

    // Find all top-level topics
    let top_level: BTreeSet<&str> = topic_notes
        .keys()
        .filter_map(|k| {
            if k.contains('/') {
                k.split('/').next()
            } else {
                Some(k.as_str())
            }
        })
        .collect();

    for top_name in top_level {
        // Count notes in this topic
        let count = topic_notes
            .get(top_name)
            .map(|v| v.len())
            .unwrap_or(0);

        // Find immediate children
        let prefix = format!("{}/", top_name);
        let mut children: Vec<TopicTreeItem> = topic_notes
            .keys()
            .filter(|k| k.starts_with(&prefix) && !k[prefix.len()..].contains('/'))
            .map(|k| {
                let child_name = k[prefix.len()..].to_string();
                TopicTreeItem {
                    name: child_name,
                    path: k.clone(),
                    count: topic_notes.get(k).map(|v| v.len()).unwrap_or(0),
                    children: Vec::new(), // Only show 2 levels in sidebar
                }
            })
            .collect();

        children.sort_by(|a, b| a.name.cmp(&b.name));

        tree.push(TopicTreeItem {
            name: top_name.to_string(),
            path: top_name.to_string(),
            count,
            children,
        });
    }

    tree.sort_by(|a, b| a.name.cmp(&b.name));
    tree
}

/// Writes the JSON index file for client-side search and filtering.
fn write_index_json(
    notes: &[NoteInfo],
    topic_notes: &BTreeMap<String, Vec<NoteInfo>>,
    tags: &[TagCount],
    output_dir: &Path,
) -> Result<()> {
    // Build note entries
    let note_entries: Vec<NoteIndexEntry> = notes
        .iter()
        .map(|n| NoteIndexEntry {
            id: n.id.clone(),
            title: n.title.clone(),
            slug: n.slug.clone(),
            description: n.description.clone(),
            topics: n.topics.clone(),
            tags: n.tags.clone(),
            created: n.created.clone(),
            modified: n.modified.clone(),
        })
        .collect();

    // Build topic tree structure
    let mut topics_map: BTreeMap<String, TopicNode> = BTreeMap::new();

    // Collect all top-level topics with their children
    let top_level: BTreeSet<&str> = topic_notes
        .keys()
        .filter_map(|k| {
            if k.contains('/') {
                k.split('/').next()
            } else {
                Some(k.as_str())
            }
        })
        .collect();

    for top_name in top_level {
        let count = topic_notes.get(top_name).map(|v| v.len()).unwrap_or(0);
        let prefix = format!("{}/", top_name);
        let children: Vec<String> = topic_notes
            .keys()
            .filter(|k| k.starts_with(&prefix) && !k[prefix.len()..].contains('/'))
            .map(|k| k[prefix.len()..].to_string())
            .collect();

        topics_map.insert(
            top_name.to_string(),
            TopicNode { count, children },
        );
    }

    let index = SiteIndex {
        notes: note_entries,
        topics: topics_map,
        tags: tags.to_vec(),
    };

    let json = serde_json::to_string_pretty(&index)?;
    std::fs::write(output_dir.join("index.json"), json)?;

    Ok(())
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
fn render_index(
    notes: &[NoteInfo],
    topics: &[TopicInfo],
    topic_tree: &[TopicTreeItem],
    all_tags: &[TagCount],
    config: &SiteConfig,
) -> Result<String> {
    let mut env = Environment::new();
    env.add_template("index", DEFAULT_INDEX_TEMPLATE)?;
    let tmpl = env.get_template("index")?;

    // Notes with full metadata for data attributes
    let notes_json: Vec<_> = notes
        .iter()
        .map(|n| {
            serde_json::json!({
                "id": n.id,
                "title": n.title,
                "slug": n.slug,
                "description": n.description,
                "topics": n.topics,
                "tags": n.tags,
            })
        })
        .collect();

    // Top-level topics for no-JS fallback
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

    // Topic tree for sidebar
    let topic_tree_json: Vec<_> = topic_tree
        .iter()
        .map(|t| {
            let children: Vec<_> = t
                .children
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "name": c.name,
                        "path": c.path,
                        "count": c.count
                    })
                })
                .collect();
            serde_json::json!({
                "name": t.name,
                "path": t.path,
                "count": t.count,
                "children": if children.is_empty() { None } else { Some(children) }
            })
        })
        .collect();

    // Tags for sidebar
    let tags_json: Vec<_> = all_tags
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "count": t.count
            })
        })
        .collect();

    let html = tmpl.render(context! {
        site_title => config.site_title,
        notes => notes_json,
        topics => topics_json,
        all_topics => topic_tree_json,
        all_tags => tags_json,
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
        create_test_note_with_tags(notes_dir, title, body, topics, &[])
    }

    /// Helper to create an indexed note with tags for testing.
    fn create_test_note_with_tags(
        notes_dir: &Path,
        title: &str,
        body: &str,
        topics: &[&str],
        tags: &[&str],
    ) -> IndexedNote {
        use crate::domain::Tag;

        let id = NoteId::new();
        let now = Utc::now();

        let topic_objs: Vec<Topic> = topics.iter().map(|t| Topic::new(t).unwrap()).collect();
        let tag_objs: Vec<Tag> = tags.iter().map(|t| Tag::new(t).unwrap()).collect();

        let note = Note::builder(id.clone(), title, now, now)
            .topics(topic_objs.clone())
            .tags(tag_objs.clone())
            .build()
            .unwrap();

        let filename = format!("{}-{}.md", id.prefix(), slugify(title));
        let file_path = notes_dir.join(&filename);
        crate::infra::write_note(&file_path, &note, body).unwrap();

        let content_hash = ContentHash::compute(std::fs::read_to_string(&file_path).unwrap().as_bytes());

        IndexedNote::builder(id, title, now, now, filename.into(), content_hash)
            .topics(topic_objs)
            .tags(tag_objs)
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

    #[test]
    fn test_site_generates_sidebar_files() {
        let temp_dir = TempDir::new().unwrap();
        let notes_dir = TempDir::new().unwrap();

        let indexed = create_test_note(notes_dir.path(), "Test Note", "Content", &[]);

        let config = SiteConfig::default();
        generate_site(&[indexed], temp_dir.path(), notes_dir.path(), &config).unwrap();

        // Sidebar JavaScript file should be created
        assert!(temp_dir.path().join("sidebar.js").exists());

        // JSON index should be created
        assert!(temp_dir.path().join("index.json").exists());

        // CSS should include sidebar styles
        let css_content = std::fs::read_to_string(temp_dir.path().join("style.css")).unwrap();
        assert!(css_content.contains(".sidebar"));
        assert!(css_content.contains(".topic-tree"));
        assert!(css_content.contains(".tag-cloud"));
    }

    #[test]
    fn test_index_page_has_sidebar_structure() {
        let temp_dir = TempDir::new().unwrap();
        let notes_dir = TempDir::new().unwrap();

        let indexed = create_test_note(notes_dir.path(), "Test Note", "Content", &["docs"]);

        let config = SiteConfig::default();
        generate_site(&[indexed], temp_dir.path(), notes_dir.path(), &config).unwrap();

        let index_content = std::fs::read_to_string(temp_dir.path().join("index.html")).unwrap();

        // Should have sidebar structure
        assert!(index_content.contains(r#"class="sidebar""#));
        assert!(index_content.contains(r#"id="search-input""#));
        assert!(index_content.contains(r#"class="topic-tree""#));

        // Should have JS-enable script
        assert!(index_content.contains("classList.replace('no-js', 'js')"));

        // Should have note with data attributes
        assert!(index_content.contains("data-topics="));
    }

    #[test]
    fn test_json_index_structure() {
        let temp_dir = TempDir::new().unwrap();
        let notes_dir = TempDir::new().unwrap();

        let indexed = create_test_note(notes_dir.path(), "Test Note", "Content", &["software/rust"]);

        let config = SiteConfig::default();
        generate_site(&[indexed], temp_dir.path(), notes_dir.path(), &config).unwrap();

        let json_content = std::fs::read_to_string(temp_dir.path().join("index.json")).unwrap();
        let index: SiteIndex = serde_json::from_str(&json_content).unwrap();

        // Should have one note
        assert_eq!(index.notes.len(), 1);
        assert_eq!(index.notes[0].title, "Test Note");
        assert_eq!(index.notes[0].topics, vec!["software/rust"]);

        // Should have topic tree with parent-child
        assert!(index.topics.contains_key("software"));
        assert!(index.topics["software"].children.contains(&"rust".to_string()));
    }

    #[test]
    fn test_json_index_includes_tags() {
        let temp_dir = TempDir::new().unwrap();
        let notes_dir = TempDir::new().unwrap();

        let indexed = create_test_note_with_tags(
            notes_dir.path(),
            "Tagged Note",
            "Content",
            &[],
            &["reference", "important"],
        );

        let config = SiteConfig::default();
        generate_site(&[indexed], temp_dir.path(), notes_dir.path(), &config).unwrap();

        let json_content = std::fs::read_to_string(temp_dir.path().join("index.json")).unwrap();
        let index: SiteIndex = serde_json::from_str(&json_content).unwrap();

        // Note should have tags
        assert_eq!(index.notes[0].tags, vec!["reference", "important"]);

        // Tags list should have counts
        assert_eq!(index.tags.len(), 2);
        assert!(index.tags.iter().any(|t| t.name == "reference" && t.count == 1));
        assert!(index.tags.iter().any(|t| t.name == "important" && t.count == 1));
    }

    #[test]
    fn test_index_page_shows_tags_in_sidebar() {
        let temp_dir = TempDir::new().unwrap();
        let notes_dir = TempDir::new().unwrap();

        let indexed = create_test_note_with_tags(
            notes_dir.path(),
            "Test Note",
            "Content",
            &[],
            &["draft", "review"],
        );

        let config = SiteConfig::default();
        generate_site(&[indexed], temp_dir.path(), notes_dir.path(), &config).unwrap();

        let index_content = std::fs::read_to_string(temp_dir.path().join("index.html")).unwrap();

        // Should have tag cloud in sidebar
        assert!(index_content.contains(r#"class="tag-cloud""#));
        assert!(index_content.contains(r#"data-tag="draft""#));
        assert!(index_content.contains(r#"data-tag="review""#));
    }
}
