//! CSS theme handling for exports.

use std::path::Path;

use anyhow::{anyhow, Result};

/// Default light theme CSS.
pub const THEME_DEFAULT: &str = r#"
body {
    font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    line-height: 1.6;
    max-width: 800px;
    margin: 0 auto;
    padding: 2rem;
    color: #333;
}
h1 { border-bottom: 1px solid #eee; padding-bottom: 0.5rem; }
h1, h2, h3, h4, h5, h6 { margin-top: 1.5em; margin-bottom: 0.5em; }
a { color: #0066cc; text-decoration: none; }
a:hover { text-decoration: underline; }
.topics { margin-bottom: 0.5rem; }
.topics a { margin-right: 0.5rem; color: #666; font-size: 0.9em; }
.tags { margin-bottom: 1rem; }
.tag {
    display: inline-block;
    background: #eef;
    padding: 0.2rem 0.5rem;
    border-radius: 3px;
    margin-right: 0.25rem;
    font-size: 0.85em;
}
pre {
    background: #f5f5f5;
    padding: 1rem;
    overflow-x: auto;
    border-radius: 4px;
}
code {
    font-family: 'SF Mono', Monaco, 'Cascadia Code', monospace;
    font-size: 0.9em;
}
:not(pre) > code {
    background: #f0f0f0;
    padding: 0.1rem 0.3rem;
    border-radius: 3px;
}
blockquote {
    border-left: 3px solid #ddd;
    margin-left: 0;
    padding-left: 1rem;
    color: #666;
}
table { border-collapse: collapse; width: 100%; }
th, td { border: 1px solid #ddd; padding: 0.5rem; text-align: left; }
th { background: #f5f5f5; }
img { max-width: 100%; height: auto; }
.metadata { color: #666; font-size: 0.9em; margin-bottom: 1rem; }
"#;

/// Dark theme CSS.
pub const THEME_DARK: &str = r#"
body {
    font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    line-height: 1.6;
    max-width: 800px;
    margin: 0 auto;
    padding: 2rem;
    background: #1a1a1a;
    color: #e0e0e0;
}
h1 { border-bottom: 1px solid #333; padding-bottom: 0.5rem; }
h1, h2, h3, h4, h5, h6 { margin-top: 1.5em; margin-bottom: 0.5em; }
a { color: #6af; text-decoration: none; }
a:hover { text-decoration: underline; }
.topics { margin-bottom: 0.5rem; }
.topics a { margin-right: 0.5rem; color: #888; font-size: 0.9em; }
.tags { margin-bottom: 1rem; }
.tag {
    display: inline-block;
    background: #333;
    padding: 0.2rem 0.5rem;
    border-radius: 3px;
    margin-right: 0.25rem;
    font-size: 0.85em;
}
pre {
    background: #2a2a2a;
    padding: 1rem;
    overflow-x: auto;
    border-radius: 4px;
}
code {
    font-family: 'SF Mono', Monaco, 'Cascadia Code', monospace;
    font-size: 0.9em;
}
:not(pre) > code {
    background: #333;
    padding: 0.1rem 0.3rem;
    border-radius: 3px;
}
blockquote {
    border-left: 3px solid #444;
    margin-left: 0;
    padding-left: 1rem;
    color: #aaa;
}
table { border-collapse: collapse; width: 100%; }
th, td { border: 1px solid #444; padding: 0.5rem; text-align: left; }
th { background: #2a2a2a; }
img { max-width: 100%; height: auto; }
.metadata { color: #888; font-size: 0.9em; margin-bottom: 1rem; }
"#;

/// Gets CSS for the specified theme.
///
/// # Arguments
///
/// * `theme` - Theme name ("default", "dark") or path to custom CSS file.
///   If None, returns the default theme.
///
/// # Errors
///
/// Returns an error if the theme name is unknown or the CSS file cannot be read.
pub fn get_theme_css(theme: Option<&str>) -> Result<String> {
    match theme {
        None | Some("default") => Ok(THEME_DEFAULT.to_string()),
        Some("dark") => Ok(THEME_DARK.to_string()),
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
        assert!(css.contains("#333")); // Light theme text color
    }

    #[test]
    fn test_dark_theme() {
        let css = get_theme_css(Some("dark")).unwrap();

        assert!(css.contains("background"));
        assert!(css.contains("#1a1a1a")); // Dark theme background
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
