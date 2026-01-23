//! CSS theme handling for exports.

use std::path::Path;

use anyhow::{anyhow, Result};

/// Unified theme CSS with CSS custom properties for automatic light/dark mode.
///
/// Uses `prefers-color-scheme` media query for seamless theme switching.
/// Design inspired by modern documentation sites (Stripe, Apple, GitHub).
pub const THEME_CSS: &str = r#"
/* ==========================================================================
   CSS Custom Properties (Design Tokens)
   ========================================================================== */

:root {
    /* Typography */
    --font-sans: 'Inter', system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    --font-mono: 'JetBrains Mono', 'SF Mono', Monaco, 'Cascadia Code', 'Fira Code', monospace;

    /* Type Scale (1.25 modular scale) */
    --text-xs: 0.75rem;
    --text-sm: 0.875rem;
    --text-base: 1rem;
    --text-lg: 1.125rem;
    --text-xl: 1.25rem;
    --text-2xl: 1.563rem;
    --text-3xl: 1.953rem;
    --text-4xl: 2.441rem;

    /* Spacing */
    --space-1: 0.25rem;
    --space-2: 0.5rem;
    --space-3: 0.75rem;
    --space-4: 1rem;
    --space-5: 1.25rem;
    --space-6: 1.5rem;
    --space-8: 2rem;
    --space-10: 2.5rem;
    --space-12: 3rem;
    --space-16: 4rem;

    /* Light theme colors */
    --color-bg: #fafafa;
    --color-bg-subtle: #f4f4f5;
    --color-bg-muted: #e4e4e7;
    --color-bg-code: #f4f4f5;
    --color-text: #18181b;
    --color-text-muted: #52525b;
    --color-text-subtle: #71717a;
    --color-border: #e4e4e7;
    --color-border-muted: #d4d4d8;
    --color-link: #0969da;
    --color-link-hover: #0550ae;
    --color-accent: #2563eb;
    --color-accent-subtle: #dbeafe;
    --color-tag-bg: #eff6ff;
    --color-tag-text: #1d4ed8;
    --color-blockquote-border: #3b82f6;
    --color-blockquote-bg: #f8fafc;

    /* Shadows */
    --shadow-sm: 0 1px 2px 0 rgb(0 0 0 / 0.05);
    --shadow-md: 0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1);

    /* Border radius */
    --radius-sm: 0.25rem;
    --radius-md: 0.375rem;
    --radius-lg: 0.5rem;
    --radius-xl: 0.75rem;
    --radius-full: 9999px;
}

@media (prefers-color-scheme: dark) {
    :root {
        --color-bg: #0d1117;
        --color-bg-subtle: #161b22;
        --color-bg-muted: #21262d;
        --color-bg-code: #161b22;
        --color-text: #e6edf3;
        --color-text-muted: #8b949e;
        --color-text-subtle: #6e7681;
        --color-border: #30363d;
        --color-border-muted: #21262d;
        --color-link: #58a6ff;
        --color-link-hover: #79c0ff;
        --color-accent: #58a6ff;
        --color-accent-subtle: #1f2937;
        --color-tag-bg: #1f2937;
        --color-tag-text: #7dd3fc;
        --color-blockquote-border: #3b82f6;
        --color-blockquote-bg: #161b22;
        --shadow-sm: 0 1px 2px 0 rgb(0 0 0 / 0.3);
        --shadow-md: 0 4px 6px -1px rgb(0 0 0 / 0.4), 0 2px 4px -2px rgb(0 0 0 / 0.3);
    }
}

/* ==========================================================================
   Base Styles
   ========================================================================== */

*, *::before, *::after {
    box-sizing: border-box;
}

html {
    font-size: 16px;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
}

body {
    font-family: var(--font-sans);
    font-size: var(--text-base);
    line-height: 1.7;
    color: var(--color-text);
    background-color: var(--color-bg);
    max-width: 70ch;
    margin: 0 auto;
    padding: var(--space-8) var(--space-6);
}

/* ==========================================================================
   Typography
   ========================================================================== */

h1, h2, h3, h4, h5, h6 {
    font-weight: 600;
    line-height: 1.4;
    margin-top: var(--space-10);
    margin-bottom: var(--space-4);
    color: var(--color-text);
    letter-spacing: -0.02em;
}

h1 {
    font-size: var(--text-4xl);
    font-weight: 700;
    margin-top: 0;
    margin-bottom: var(--space-3);
    letter-spacing: -0.03em;
}

h2 {
    font-size: var(--text-2xl);
    padding-bottom: var(--space-3);
    border-bottom: 1px solid var(--color-border);
}

h3 { font-size: var(--text-xl); }
h4 { font-size: var(--text-lg); }
h5, h6 { font-size: var(--text-base); }

p {
    margin-top: 0;
    margin-bottom: var(--space-4);
}

/* ==========================================================================
   Links
   ========================================================================== */

a {
    color: var(--color-link);
    text-decoration: none;
    transition: color 0.15s ease;
}

a:hover {
    color: var(--color-link-hover);
    text-decoration: underline;
    text-underline-offset: 2px;
}

/* ==========================================================================
   Article Header & Metadata
   ========================================================================== */

article > header {
    margin-bottom: var(--space-10);
}

.description {
    font-size: var(--text-lg);
    color: var(--color-text-muted);
    margin-top: var(--space-2);
    margin-bottom: var(--space-4);
    line-height: 1.6;
}

.metadata {
    font-size: var(--text-sm);
    color: var(--color-text-subtle);
    margin-top: var(--space-4);
}

.metadata time {
    font-variant-numeric: tabular-nums;
}

/* ==========================================================================
   Topics (Breadcrumb Navigation)
   ========================================================================== */

.topics, .breadcrumb {
    font-size: var(--text-sm);
    color: var(--color-text-muted);
    margin-bottom: var(--space-4);
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1);
}

.topics a, .breadcrumb a {
    color: var(--color-text-muted);
    transition: color 0.15s ease;
}

.topics a:hover, .breadcrumb a:hover {
    color: var(--color-link);
    text-decoration: none;
}

.topics a::after {
    content: '/';
    margin-left: var(--space-2);
    color: var(--color-text-subtle);
}

.topics a:last-child::after {
    content: '';
}

/* ==========================================================================
   Tags
   ========================================================================== */

.tags {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-top: var(--space-3);
    margin-bottom: var(--space-4);
}

.tag {
    display: inline-flex;
    align-items: center;
    padding: var(--space-1) var(--space-3);
    font-size: var(--text-xs);
    font-weight: 500;
    background-color: var(--color-tag-bg);
    color: var(--color-tag-text);
    border-radius: var(--radius-full);
    transition: background-color 0.15s ease;
}

.tag:hover {
    background-color: var(--color-accent-subtle);
}

/* ==========================================================================
   Code & Pre
   ========================================================================== */

code {
    font-family: var(--font-mono);
    font-size: 0.875em;
}

:not(pre) > code {
    padding: 0.125em 0.375em;
    background-color: var(--color-bg-code);
    border-radius: var(--radius-sm);
    font-size: 0.85em;
}

pre {
    margin: var(--space-6) 0;
    padding: var(--space-4) var(--space-5);
    background-color: var(--color-bg-subtle);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    overflow-x: auto;
    font-size: var(--text-sm);
    line-height: 1.6;
}

pre code {
    padding: 0;
    background: none;
    font-size: inherit;
}

/* ==========================================================================
   Blockquotes
   ========================================================================== */

blockquote {
    margin: var(--space-6) 0;
    padding: var(--space-4) var(--space-5);
    padding-left: var(--space-5);
    border-left: 4px solid var(--color-blockquote-border);
    background-color: var(--color-blockquote-bg);
    border-radius: 0 var(--radius-md) var(--radius-md) 0;
    color: var(--color-text-muted);
}

blockquote p:last-child {
    margin-bottom: 0;
}

blockquote cite {
    display: block;
    margin-top: var(--space-3);
    font-size: var(--text-sm);
    font-style: normal;
    color: var(--color-text-subtle);
}

/* ==========================================================================
   Tables
   ========================================================================== */

table {
    width: 100%;
    margin: var(--space-6) 0;
    border-collapse: collapse;
    font-size: var(--text-sm);
}

th, td {
    padding: var(--space-3) var(--space-4);
    text-align: left;
    border-bottom: 1px solid var(--color-border);
}

th {
    font-weight: 600;
    color: var(--color-text);
    background-color: var(--color-bg-subtle);
}

tr:hover td {
    background-color: var(--color-bg-subtle);
}

/* ==========================================================================
   Lists
   ========================================================================== */

ul, ol {
    margin: var(--space-4) 0;
    padding-left: var(--space-6);
}

li {
    margin-bottom: var(--space-2);
}

li > ul, li > ol {
    margin-top: var(--space-2);
    margin-bottom: 0;
}

/* ==========================================================================
   Images
   ========================================================================== */

img {
    max-width: 100%;
    height: auto;
    border-radius: var(--radius-md);
}

/* ==========================================================================
   Horizontal Rules
   ========================================================================== */

hr {
    border: none;
    height: 1px;
    background-color: var(--color-border);
    margin: var(--space-10) 0;
}

/* ==========================================================================
   Site Navigation (for static site)
   ========================================================================== */

body > header {
    margin-bottom: var(--space-10);
    padding-bottom: var(--space-6);
    border-bottom: 1px solid var(--color-border);
}

body > header h1 {
    margin-bottom: var(--space-2);
}

.note-count {
    color: var(--color-text-subtle);
    font-size: var(--text-sm);
    margin: 0;
}

/* ==========================================================================
   Topic Navigation (for static site)
   ========================================================================== */

.topics-nav, .subtopics {
    margin-bottom: var(--space-8);
}

.topics-nav h2, .subtopics h2 {
    font-size: var(--text-lg);
    margin-bottom: var(--space-4);
    border-bottom: none;
    padding-bottom: 0;
}

.topics-nav ul, .subtopics ul {
    list-style: none;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
}

.topics-nav li, .subtopics li {
    margin: 0;
}

.topics-nav a, .subtopics a {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-4);
    background-color: var(--color-bg-subtle);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    font-size: var(--text-sm);
    transition: all 0.15s ease;
}

.topics-nav a:hover, .subtopics a:hover {
    background-color: var(--color-accent-subtle);
    border-color: var(--color-link);
    text-decoration: none;
}

/* ==========================================================================
   Notes List (for static site)
   ========================================================================== */

.notes-list h2 {
    font-size: var(--text-lg);
    margin-bottom: var(--space-4);
    border-bottom: none;
    padding-bottom: 0;
}

.notes-list ul {
    list-style: none;
    padding: 0;
    margin: 0;
}

.notes-list li {
    margin: 0;
    padding: var(--space-4) 0;
    border-bottom: 1px solid var(--color-border);
}

.notes-list li:first-child {
    padding-top: 0;
}

.notes-list li:last-child {
    border-bottom: none;
}

.notes-list a {
    font-weight: 500;
    font-size: var(--text-base);
}

.notes-list .description {
    display: block;
    font-size: var(--text-sm);
    color: var(--color-text-muted);
    margin-top: var(--space-1);
    line-height: 1.5;
}

/* ==========================================================================
   Footer
   ========================================================================== */

body > footer, article + footer {
    margin-top: var(--space-12);
    padding-top: var(--space-6);
    border-top: 1px solid var(--color-border);
    font-size: var(--text-sm);
    color: var(--color-text-subtle);
}

/* ==========================================================================
   Responsive Adjustments
   ========================================================================== */

@media (max-width: 640px) {
    body {
        padding: var(--space-4);
    }

    h1 {
        font-size: var(--text-3xl);
    }

    h2 {
        font-size: var(--text-xl);
    }

    pre {
        padding: var(--space-3);
        font-size: var(--text-xs);
        border-radius: var(--radius-md);
    }
}

/* ==========================================================================
   Print Styles
   ========================================================================== */

@media print {
    body {
        max-width: 100%;
        padding: 0;
        color: #000;
        background: #fff;
    }

    a {
        color: #000;
        text-decoration: underline;
    }

    pre, blockquote {
        border: 1px solid #ddd;
        page-break-inside: avoid;
    }

    h1, h2, h3 {
        page-break-after: avoid;
    }
}
"#;

/// Legacy alias for backward compatibility with tests.
pub const THEME_DEFAULT: &str = THEME_CSS;

/// Legacy dark theme - now returns the unified theme (dark mode is automatic).
pub const THEME_DARK: &str = THEME_CSS;

/// Gets CSS for the specified theme.
///
/// The default theme uses CSS custom properties with `prefers-color-scheme`
/// media query for automatic light/dark mode switching. The "dark" theme
/// name is accepted for backwards compatibility but returns the same
/// unified theme (dark mode is handled automatically via media queries).
///
/// # Arguments
///
/// * `theme` - Theme name ("default", "dark") or path to custom CSS file.
///   If None, returns the default theme with automatic light/dark support.
///
/// # Errors
///
/// Returns an error if the theme name is unknown or the CSS file cannot be read.
pub fn get_theme_css(theme: Option<&str>) -> Result<String> {
    match theme {
        // Both default and dark now return the unified theme with automatic switching
        None | Some("default") | Some("dark") => Ok(THEME_CSS.to_string()),
        Some(path) => {
            let path = Path::new(path);
            if path.exists() {
                Ok(std::fs::read_to_string(path)?)
            } else {
                Err(anyhow!("Unknown theme: '{}'. Use 'default', 'dark', or a path to a CSS file.", path.display()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_theme_css() {
        let css = get_theme_css(None).unwrap();

        assert!(css.contains("body"));
        assert!(css.contains("font-family"));
    }

    #[test]
    fn test_explicit_default_theme() {
        let css = get_theme_css(Some("default")).unwrap();

        assert!(css.contains("body"));
        assert!(css.contains(":root")); // CSS custom properties
        assert!(css.contains("--color-text")); // Design tokens
    }

    #[test]
    fn test_dark_theme_returns_unified() {
        // Dark theme now returns the same unified CSS with automatic switching
        let css = get_theme_css(Some("dark")).unwrap();

        assert!(css.contains("prefers-color-scheme: dark"));
        assert!(css.contains("--color-bg: #0d1117")); // Dark mode background
    }

    #[test]
    fn test_unified_theme_has_light_and_dark() {
        let css = get_theme_css(None).unwrap();

        // Light theme colors
        assert!(css.contains("--color-bg: #fafafa"));
        // Dark theme colors (in media query)
        assert!(css.contains("--color-bg: #0d1117"));
        // Media query for automatic switching
        assert!(css.contains("@media (prefers-color-scheme: dark)"));
    }

    #[test]
    fn test_custom_theme_file() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "body {{ color: red; }}").unwrap();

        let css = get_theme_css(Some(temp.path().to_str().unwrap())).unwrap();

        assert_eq!(css.trim(), "body { color: red; }");
    }

    #[test]
    fn test_invalid_theme_errors() {
        let result = get_theme_css(Some("nonexistent-theme"));

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown theme"));
    }
}
