//! den - markdown notes with virtual folder organization

pub mod cli;
pub mod domain;
pub mod index;
pub mod infra;

use anyhow::Result;
use clap::Parser;

use cli::{
    Cli, Command,
    config::Config,
    handlers::{
        handle_backlinks, handle_check, handle_edit, handle_index, handle_link, handle_list,
        handle_new, handle_rels, handle_search, handle_show, handle_tag, handle_tags,
        handle_topics, handle_unlink, handle_untag,
    },
};

/// Main entry point for the CLI application.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;
    let notes_dir = config.notes_dir(cli.dir.as_ref());
    let verbose = cli.verbose > 0;

    match &cli.command {
        Command::Index(args) => handle_index(args, &notes_dir, verbose),
        Command::List(args) => handle_list(args, &notes_dir),
        Command::Search(args) => handle_search(args, &notes_dir),
        Command::New(args) => handle_new(args, &notes_dir, &config),
        Command::Show(args) => handle_show(args, &notes_dir),
        Command::Edit(args) => handle_edit(args, &notes_dir, &config),
        Command::Topics(args) => handle_topics(args, &notes_dir),
        Command::Tags(args) => handle_tags(args, &notes_dir),
        Command::Tag(args) => handle_tag(args, &notes_dir),
        Command::Untag(args) => handle_untag(args, &notes_dir),
        Command::Check(args) => handle_check(args),
        Command::Backlinks(args) => handle_backlinks(args, &notes_dir),
        Command::Link(args) => handle_link(args, &notes_dir),
        Command::Unlink(args) => handle_unlink(args, &notes_dir),
        Command::Rels(args) => handle_rels(args, &notes_dir),
    }
}
