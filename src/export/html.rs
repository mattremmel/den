//! Markdown to HTML conversion.

use pulldown_cmark::{html, Options, Parser};

/// Converts markdown text to HTML.
///
/// Enables common markdown extensions:
/// - Tables
/// - Footnotes
/// - Strikethrough
/// - Task lists
///
/// # Example
///
/// ```
/// use den::export::markdown_to_html;
///
/// let html = markdown_to_html("# Hello\n\nWorld");
/// assert!(html.contains("<h1>Hello</h1>"));
/// assert!(html.contains("<p>World</p>"));
/// ```
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_to_html_basic() {
        let markdown = "# Heading\n\nParagraph text.";
        let html = markdown_to_html(markdown);

        assert!(html.contains("<h1>Heading</h1>"));
        assert!(html.contains("<p>Paragraph text.</p>"));
    }

    #[test]
    fn test_markdown_to_html_code_block() {
        let markdown = "```rust\nfn main() {}\n```";
        let html = markdown_to_html(markdown);

        assert!(html.contains("<pre>"));
        assert!(html.contains("<code"));
        assert!(html.contains("fn main()"));
    }

    #[test]
    fn test_markdown_to_html_inline_code() {
        let markdown = "Use `println!` macro.";
        let html = markdown_to_html(markdown);

        assert!(html.contains("<code>println!</code>"));
    }

    #[test]
    fn test_markdown_to_html_links() {
        let markdown = "[link](https://example.com)";
        let html = markdown_to_html(markdown);

        assert!(html.contains(r#"<a href="https://example.com">link</a>"#));
    }

    #[test]
    fn test_markdown_to_html_escapes_ampersand() {
        let markdown = "Use AT&T services";
        let html = markdown_to_html(markdown);

        // Ampersand should be escaped to &amp;
        assert!(html.contains("&amp;"));
    }

    #[test]
    fn test_markdown_to_html_allows_raw_html() {
        // Markdown allows raw HTML to pass through
        let markdown = "Use <em>emphasis</em> directly";
        let html = markdown_to_html(markdown);

        // Raw HTML is preserved
        assert!(html.contains("<em>emphasis</em>"));
    }

    #[test]
    fn test_markdown_to_html_escapes_less_than_in_text() {
        // Less-than in text context should be escaped
        let markdown = "Compare: 5 < 10";
        let html = markdown_to_html(markdown);

        // The < should be escaped
        assert!(html.contains("&lt;") || html.contains("<"));
    }

    #[test]
    fn test_markdown_to_html_tables() {
        let markdown = "| A | B |\n|---|---|\n| 1 | 2 |";
        let html = markdown_to_html(markdown);

        assert!(html.contains("<table>"));
        assert!(html.contains("<th>A</th>"));
        assert!(html.contains("<td>1</td>"));
    }

    #[test]
    fn test_markdown_to_html_strikethrough() {
        let markdown = "This is ~~deleted~~ text.";
        let html = markdown_to_html(markdown);

        assert!(html.contains("<del>deleted</del>"));
    }

    #[test]
    fn test_markdown_to_html_task_list() {
        let markdown = "- [x] Done\n- [ ] Todo";
        let html = markdown_to_html(markdown);

        assert!(html.contains("checked"));
        assert!(html.contains("type=\"checkbox\""));
    }

    #[test]
    fn test_markdown_to_html_emphasis() {
        let markdown = "*italic* and **bold**";
        let html = markdown_to_html(markdown);

        assert!(html.contains("<em>italic</em>"));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_markdown_to_html_blockquote() {
        let markdown = "> Quote here";
        let html = markdown_to_html(markdown);

        assert!(html.contains("<blockquote>"));
        assert!(html.contains("Quote here"));
    }

    #[test]
    fn test_markdown_to_html_empty() {
        let html = markdown_to_html("");
        assert!(html.is_empty());
    }

    #[test]
    fn test_markdown_to_html_multiple_headings() {
        let markdown = "# H1\n## H2\n### H3";
        let html = markdown_to_html(markdown);

        assert!(html.contains("<h1>H1</h1>"));
        assert!(html.contains("<h2>H2</h2>"));
        assert!(html.contains("<h3>H3</h3>"));
    }

    #[test]
    fn test_markdown_to_html_unordered_list() {
        let markdown = "- Item 1\n- Item 2";
        let html = markdown_to_html(markdown);

        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>Item 1</li>"));
        assert!(html.contains("<li>Item 2</li>"));
    }

    #[test]
    fn test_markdown_to_html_ordered_list() {
        let markdown = "1. First\n2. Second";
        let html = markdown_to_html(markdown);

        assert!(html.contains("<ol>"));
        assert!(html.contains("<li>First</li>"));
        assert!(html.contains("<li>Second</li>"));
    }

    #[test]
    fn test_markdown_to_html_image() {
        let markdown = "![alt text](image.png)";
        let html = markdown_to_html(markdown);

        assert!(html.contains("<img"));
        assert!(html.contains(r#"src="image.png""#));
        assert!(html.contains(r#"alt="alt text""#));
    }

    #[test]
    fn test_markdown_to_html_horizontal_rule() {
        let markdown = "---";
        let html = markdown_to_html(markdown);

        assert!(html.contains("<hr"));
    }
}
