//! CLI command definitions and handlers

pub mod config;
pub mod date_filter;
pub mod handlers;
pub mod output;

use clap::{ArgAction, Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;

use output::OutputFormat;

/// den - markdown notes with virtual folder organization
#[derive(Parser, Debug)]
#[command(name = "den", version, about, long_about = None)]
pub struct Cli {
    /// Notes directory (overrides config file and vault)
    #[arg(short = 'd', long, global = true)]
    pub dir: Option<PathBuf>,

    /// Use a named vault from config
    #[arg(long, global = true)]
    pub vault: Option<String>,

    /// Increase verbosity (-v, -vv, -vvv)
    #[arg(short, long, global = true, action = ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Rebuild or update the index
    Index(IndexArgs),

    /// List notes, optionally filtered by topic and tags
    #[command(name = "ls")]
    List(ListArgs),

    /// Full-text search across notes
    Search(SearchArgs),

    /// Create a new note
    New(NewArgs),

    /// Show a note's contents
    Show(ShowArgs),

    /// Edit a note in your editor
    Edit(EditArgs),

    /// List all topics in the hierarchy
    Topics(TopicsArgs),

    /// List all tags
    Tags(TagsArgs),

    /// Add a tag to a note
    Tag(TagArgs),

    /// Remove a tag from a note
    Untag(UntagArgs),

    /// Check for issues (broken links, orphans, etc.)
    Check(CheckArgs),

    /// Show notes that link to a given note
    Backlinks(BacklinksArgs),

    /// Create a link between notes
    Link(LinkArgs),

    /// Remove a link between notes
    Unlink(UnlinkArgs),

    /// List relationship types used in links
    Rels(RelsArgs),

    /// Generate shell completions
    Completions(CompletionsArgs),

    /// Move/rename a note (change title or topics)
    Mv(MvArgs),

    /// Archive a note (adds 'archived' tag)
    Archive(ArchiveArgs),

    /// Unarchive a note (removes 'archived' tag)
    Unarchive(UnarchiveArgs),

    /// Export notes to HTML, PDF, or static site
    Export(ExportArgs),

    /// List configured vaults
    Vaults(VaultsArgs),
}

/// Arguments for the `index` command
#[derive(Parser, Debug)]
pub struct IndexArgs {
    /// Force full rebuild instead of incremental update
    #[arg(long)]
    pub full: bool,
}

/// Arguments for the `ls` (list) command
#[derive(Parser, Debug)]
pub struct ListArgs {
    /// Topic to filter by (trailing / includes descendants)
    pub topic: Option<String>,

    /// Filter by tag (can be specified multiple times)
    #[arg(short, long = "tag", action = ArgAction::Append)]
    pub tags: Vec<String>,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,

    /// Filter by creation date (YYYY-MM-DD or relative like "7d")
    #[arg(long)]
    pub created: Option<String>,

    /// Filter by modification date (YYYY-MM-DD or relative like "7d")
    #[arg(long)]
    pub modified: Option<String>,

    /// Include archived notes in results
    #[arg(short = 'a', long)]
    pub include_archived: bool,
}

/// Arguments for the `search` command
#[derive(Parser, Debug)]
pub struct SearchArgs {
    /// Search query
    pub query: String,

    /// Restrict search to topic (trailing / includes descendants)
    #[arg(short = 'T', long)]
    pub topic: Option<String>,

    /// Filter results by tag (can be specified multiple times)
    #[arg(short, long = "tag", action = ArgAction::Append)]
    pub tags: Vec<String>,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,

    /// Include archived notes in results
    #[arg(short = 'a', long)]
    pub include_archived: bool,
}

/// Arguments for the `new` command
#[derive(Parser, Debug)]
pub struct NewArgs {
    /// Note title
    pub title: String,

    /// Topic for the note (can be specified multiple times)
    #[arg(short = 'T', long = "topic", action = ArgAction::Append)]
    pub topics: Vec<String>,

    /// Tag for the note (can be specified multiple times)
    #[arg(short, long = "tag", action = ArgAction::Append)]
    pub tags: Vec<String>,

    /// Short description
    #[arg(short = 'D', long)]
    pub desc: Option<String>,

    /// Open in editor after creation
    #[arg(short, long)]
    pub edit: bool,
}

/// Arguments for the `show` command
#[derive(Parser, Debug)]
pub struct ShowArgs {
    /// Note ID or title
    pub note: String,
}

/// Arguments for the `edit` command
#[derive(Parser, Debug)]
pub struct EditArgs {
    /// Note ID or title
    pub note: String,
}

/// Arguments for the `topics` command
#[derive(Parser, Debug)]
pub struct TopicsArgs {
    /// Show note counts for each topic
    #[arg(long)]
    pub counts: bool,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

/// Arguments for the `tags` command
#[derive(Parser, Debug)]
pub struct TagsArgs {
    /// Show note counts for each tag
    #[arg(long)]
    pub counts: bool,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

/// Arguments for the `tag` command (add tag to note)
#[derive(Parser, Debug)]
pub struct TagArgs {
    /// Note ID or title
    pub note: String,

    /// Tag to add
    pub tag: String,
}

/// Arguments for the `untag` command (remove tag from note)
#[derive(Parser, Debug)]
pub struct UntagArgs {
    /// Note ID or title
    pub note: String,

    /// Tag to remove
    pub tag: String,
}

/// Arguments for the `check` command
#[derive(Parser, Debug)]
pub struct CheckArgs {
    /// Attempt to fix issues automatically
    #[arg(long)]
    pub fix: bool,
}

/// Arguments for the `backlinks` command
#[derive(Parser, Debug)]
pub struct BacklinksArgs {
    /// Note ID or title
    pub note: String,

    /// Filter by relationship type
    #[arg(long)]
    pub rel: Option<String>,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

/// Arguments for the `link` command
#[derive(Parser, Debug)]
pub struct LinkArgs {
    /// Source note ID or title
    pub source: String,

    /// Target note ID or title
    pub target: String,

    /// Relationship type (can be specified multiple times)
    #[arg(long = "rel", action = ArgAction::Append)]
    pub rels: Vec<String>,

    /// Optional context note about the link
    #[arg(long)]
    pub note: Option<String>,
}

/// Arguments for the `unlink` command
#[derive(Parser, Debug)]
pub struct UnlinkArgs {
    /// Source note ID or title
    pub source: String,

    /// Target note ID or title
    pub target: String,
}

/// Arguments for the `rels` command
#[derive(Parser, Debug)]
pub struct RelsArgs {
    /// Show usage counts for each relationship type
    #[arg(long)]
    pub counts: bool,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

/// Arguments for the `completions` command
#[derive(Parser, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for (bash, zsh, fish)
    #[arg(value_enum)]
    pub shell: Shell,
}

/// Arguments for the `mv` command (move/rename note)
#[derive(Parser, Debug)]
pub struct MvArgs {
    /// Note ID or title
    pub note: String,

    /// New title for the note (triggers file rename)
    #[arg(long)]
    pub title: Option<String>,

    /// Topic for the note (replaces all topics, can be specified multiple times)
    #[arg(short = 'T', long = "topic", action = ArgAction::Append)]
    pub topics: Vec<String>,

    /// Remove all topics from the note
    #[arg(long)]
    pub clear_topics: bool,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

/// Arguments for the `archive` command
#[derive(Parser, Debug)]
pub struct ArchiveArgs {
    /// Note ID or title
    pub note: String,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

/// Arguments for the `unarchive` command
#[derive(Parser, Debug)]
pub struct UnarchiveArgs {
    /// Note ID or title
    pub note: String,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

/// Export format for the `export` command
#[derive(Clone, Debug, Default, clap::ValueEnum)]
pub enum ExportFormat {
    /// HTML document
    #[default]
    Html,
    /// PDF document (requires wkhtmltopdf or weasyprint)
    Pdf,
    /// Static site with navigation
    Site,
}

/// Arguments for the `export` command
#[derive(Parser, Debug)]
pub struct ExportArgs {
    /// Note to export (ID prefix or title). Required unless --all is used.
    #[arg(required_unless_present = "all")]
    pub note: Option<String>,

    /// Export all notes
    #[arg(long, conflicts_with = "note")]
    pub all: bool,

    /// Export format
    #[arg(short = 'F', long = "format", value_enum, default_value_t = ExportFormat::Html)]
    pub export_format: ExportFormat,

    /// Output path (stdout if not specified for single note)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Custom template file
    #[arg(long)]
    pub template: Option<PathBuf>,

    /// CSS theme (default, dark, or path to CSS file)
    #[arg(long)]
    pub theme: Option<String>,

    /// Filter by topic (trailing / includes descendants)
    #[arg(short = 'T', long)]
    pub topic: Option<String>,

    /// Filter by tag (can be specified multiple times)
    #[arg(short, long = "tag", action = ArgAction::Append)]
    pub tags: Vec<String>,

    /// Include archived notes
    #[arg(short = 'a', long)]
    pub include_archived: bool,

    /// Resolve internal note links (note ID references become HTML links)
    #[arg(short = 'r', long)]
    pub resolve_links: bool,

    /// CLI output format (for status messages, not export content)
    #[arg(long = "cli-format", value_enum, default_value_t = OutputFormat::Human)]
    pub cli_format: OutputFormat,
}

/// Arguments for the `vaults` command
#[derive(Parser, Debug)]
pub struct VaultsArgs {
    /// Output format
    #[arg(short = 'f', long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}
