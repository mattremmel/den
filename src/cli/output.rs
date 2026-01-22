//! Output format types for CLI commands.

use clap::ValueEnum;
use serde::Serialize;

/// Output format for command results.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable output (default)
    #[default]
    Human,
    /// JSON output for programmatic consumption
    Json,
    /// Plain file paths, one per line
    Paths,
}

/// Wrapper for serializable command output.
#[derive(Debug, Serialize)]
pub struct Output<T: Serialize> {
    pub data: T,
}

impl<T: Serialize> Output<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

/// A single note in listing output.
#[derive(Debug, Serialize)]
pub struct NoteListing {
    pub id: String,
    pub title: String,
    pub path: String,
}

/// A topic with optional count.
#[derive(Debug, Serialize)]
pub struct TopicListing {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
}

/// A tag with optional count.
#[derive(Debug, Serialize)]
pub struct TagListing {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
}

/// A relationship type with optional count.
#[derive(Debug, Serialize)]
pub struct RelListing {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
}

/// A search result in listing output.
#[derive(Debug, Serialize)]
pub struct SearchListing {
    pub id: String,
    pub title: String,
    pub path: String,
    pub rank: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_listing_serializes_to_json() {
        let listing = SearchListing {
            id: "01HQ3K5M".to_string(),
            title: "Test Note".to_string(),
            path: "test.md".to_string(),
            rank: 0.75,
            snippet: Some("matching <b>text</b>".to_string()),
        };
        let json = serde_json::to_string(&listing).unwrap();
        assert!(json.contains("\"rank\":0.75"));
        assert!(json.contains("\"snippet\":"));
    }

    #[test]
    fn search_listing_omits_none_snippet() {
        let listing = SearchListing {
            id: "01HQ3K5M".to_string(),
            title: "Test Note".to_string(),
            path: "test.md".to_string(),
            rank: 0.5,
            snippet: None,
        };
        let json = serde_json::to_string(&listing).unwrap();
        assert!(!json.contains("snippet"));
    }
}
