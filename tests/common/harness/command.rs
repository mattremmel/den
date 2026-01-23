//! Fluent wrapper around assert_cmd::Command.

// Allow dead code since this is a test utility with methods for future tests
#![allow(dead_code)]

use assert_cmd::Command;
use serde::de::DeserializeOwned;
use std::path::Path;

/// Fluent wrapper around `assert_cmd::Command` for the `den` binary.
///
/// Provides a builder-style API for constructing and executing CLI commands.
pub struct DenCommand {
    args: Vec<String>,
}

impl DenCommand {
    /// Creates a new command for the `den` binary.
    pub fn new() -> Self {
        Self { args: Vec::new() }
    }

    /// Sets the `--dir` option to specify the notes directory.
    pub fn dir(mut self, path: &Path) -> Self {
        self.args.push("--dir".to_string());
        self.args.push(path.to_string_lossy().to_string());
        self
    }

    /// Adds arguments to the command.
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.args
            .extend(args.into_iter().map(|s| s.as_ref().to_string()));
        self
    }

    /// Returns the current arguments (for testing).
    pub fn get_args(&self) -> &[String] {
        &self.args
    }

    /// Runs the command and returns an Assert for making assertions.
    #[allow(deprecated)]
    pub fn assert(self) -> assert_cmd::assert::Assert {
        let mut cmd = Command::cargo_bin("den").expect("Failed to find den binary");
        cmd.args(&self.args);
        cmd.assert()
    }

    /// Runs the command, expects success, and returns stdout as a string.
    pub fn output_success(self) -> String {
        let output = self.assert().success().get_output().stdout.clone();
        String::from_utf8(output).expect("Output was not valid UTF-8")
    }

    /// Runs the command, expects success, and parses stdout as JSON.
    pub fn output_json<T: DeserializeOwned>(self) -> T {
        let output = self.output_success();
        serde_json::from_str(&output).expect("Failed to parse output as JSON")
    }

    // ===========================================
    // Command Shortcuts
    // ===========================================

    /// Configures for the `index` command.
    pub fn index(self) -> Self {
        self.args(["index"])
    }

    /// Configures for the `ls` command.
    pub fn ls(self) -> Self {
        self.args(["ls"])
    }

    /// Configures for the `search` command with a query.
    pub fn search(self, query: &str) -> Self {
        self.args(["search", query])
    }

    /// Configures for the `show` command with an ID.
    pub fn show(self, id: &str) -> Self {
        self.args(["show", id])
    }

    /// Configures for the `tags` command.
    pub fn tags(self) -> Self {
        self.args(["tags"])
    }

    /// Configures for the `topics` command.
    pub fn topics(self) -> Self {
        self.args(["topics"])
    }

    /// Configures for the `new` command to create a note.
    pub fn new_note(self, title: &str) -> Self {
        self.args(["new", title])
    }

    /// Configures for the `edit` command.
    pub fn edit(self, note: &str) -> Self {
        self.args(["edit", note])
    }

    /// Configures for the `tag` command (add tag to note).
    pub fn tag_add(self, note: &str, tag: &str) -> Self {
        self.args(["tag", note, tag])
    }

    /// Configures for the `untag` command (remove tag from note).
    pub fn untag(self, note: &str, tag: &str) -> Self {
        self.args(["untag", note, tag])
    }

    /// Configures for the `backlinks` command.
    pub fn backlinks(self, note: &str) -> Self {
        self.args(["backlinks", note])
    }

    /// Configures for the `link` command.
    pub fn link(self, source: &str, target: &str) -> Self {
        self.args(["link", source, target])
    }

    /// Configures for the `unlink` command.
    pub fn unlink(self, source: &str, target: &str) -> Self {
        self.args(["unlink", source, target])
    }

    /// Configures for the `rels` command.
    pub fn rels(self) -> Self {
        self.args(["rels"])
    }

    /// Configures for the `check` command.
    pub fn check(self) -> Self {
        self.args(["check"])
    }

    /// Configures for the `mv` command.
    pub fn mv(self, note: &str) -> Self {
        self.args(["mv", note])
    }

    /// Configures for the `archive` command.
    pub fn archive(self, note: &str) -> Self {
        self.args(["archive", note])
    }

    /// Configures for the `unarchive` command.
    pub fn unarchive(self, note: &str) -> Self {
        self.args(["unarchive", note])
    }

    // ===========================================
    // Format Options
    // ===========================================

    /// Adds `--format json` to the command.
    pub fn format_json(self) -> Self {
        self.args(["--format", "json"])
    }

    /// Adds `--format paths` to the command.
    pub fn format_paths(self) -> Self {
        self.args(["--format", "paths"])
    }

    // ===========================================
    // Option Modifiers
    // ===========================================

    /// Adds `--rel <type>` to the command.
    pub fn with_rel(self, rel: &str) -> Self {
        self.args(["--rel", rel])
    }

    /// Adds `--counts` to the command.
    pub fn with_counts(self) -> Self {
        self.args(["--counts"])
    }

    /// Adds `--tag <tag>` to the command.
    pub fn with_tag(self, tag: &str) -> Self {
        self.args(["--tag", tag])
    }

    /// Adds `--topic <topic>` to the command (for search).
    pub fn with_topic(self, topic: &str) -> Self {
        self.args(["--topic", topic])
    }

    /// Adds `--full` to the command.
    pub fn with_full(self) -> Self {
        self.args(["--full"])
    }

    /// Adds `--desc <description>` to the command.
    pub fn with_desc(self, desc: &str) -> Self {
        self.args(["--desc", desc])
    }

    /// Adds `--note <context>` to the command.
    pub fn with_note(self, note: &str) -> Self {
        self.args(["--note", note])
    }

    /// Adds `--created <date>` to the command.
    pub fn with_created(self, date: &str) -> Self {
        self.args(["--created", date])
    }

    /// Adds `--modified <date>` to the command.
    pub fn with_modified(self, date: &str) -> Self {
        self.args(["--modified", date])
    }

    /// Adds `--title <title>` to the command (for mv).
    pub fn with_title(self, title: &str) -> Self {
        self.args(["--title", title])
    }

    /// Adds `--clear-topics` to the command (for mv).
    pub fn with_clear_topics(self) -> Self {
        self.args(["--clear-topics"])
    }

    /// Adds `--include-archived` / `-a` to the command.
    pub fn with_include_archived(self) -> Self {
        self.args(["--include-archived"])
    }
}

impl Default for DenCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ===========================================
    // Phase 5: DenCommand Basics
    // ===========================================

    #[test]
    fn test_command_runs_binary() {
        // Just verify the binary can be found and runs (with --help)
        DenCommand::new().args(["--help"]).assert().success();
    }

    #[test]
    fn test_command_with_dir() {
        let temp = TempDir::new().unwrap();
        let cmd = DenCommand::new().dir(temp.path());
        let args = cmd.get_args();
        assert_eq!(args[0], "--dir");
        assert_eq!(args[1], temp.path().to_string_lossy());
    }

    #[test]
    fn test_command_output_success() {
        let output = DenCommand::new().args(["--help"]).output_success();
        assert!(output.contains("den") || output.contains("notes"));
    }

    #[test]
    fn test_command_shortcuts() {
        let cmd = DenCommand::new().ls().format_json();
        let args = cmd.get_args();
        assert!(args.contains(&"ls".to_string()));
        assert!(args.contains(&"--format".to_string()));
        assert!(args.contains(&"json".to_string()));
    }
}
