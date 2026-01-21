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
