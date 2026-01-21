//! Configuration file support.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

/// Application configuration loaded from config file.
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    /// Default notes directory
    pub dir: Option<PathBuf>,

    /// Editor command for editing notes
    pub editor: Option<String>,
}

impl Config {
    /// Load configuration from the default config file location.
    ///
    /// Returns default config if the file doesn't exist.
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let contents = std::fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read config file: {}", config_path.display()))?;

        toml::from_str(&contents)
            .with_context(|| format!("failed to parse config file: {}", config_path.display()))
    }

    /// Returns the path to the config file.
    ///
    /// Default: `~/.config/den/config.toml`
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("den")
            .join("config.toml")
    }

    /// Resolve the notes directory, with CLI argument taking precedence.
    ///
    /// Precedence order:
    /// 1. CLI `--dir` argument
    /// 2. Config file `dir` setting
    /// 3. Current working directory
    pub fn notes_dir(&self, cli_dir: Option<&PathBuf>) -> PathBuf {
        cli_dir
            .cloned()
            .or_else(|| self.dir.clone())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Resolve the editor command.
    ///
    /// Precedence order:
    /// 1. Config file `editor` setting
    /// 2. $EDITOR environment variable
    /// 3. $VISUAL environment variable
    /// 4. "vi" as fallback
    pub fn editor(&self) -> String {
        self.editor
            .clone()
            .or_else(|| std::env::var("EDITOR").ok())
            .or_else(|| std::env::var("VISUAL").ok())
            .unwrap_or_else(|| "vi".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_no_dir() {
        let config = Config::default();
        assert!(config.dir.is_none());
    }

    #[test]
    fn notes_dir_prefers_cli_arg() {
        let config = Config {
            dir: Some(PathBuf::from("/config/notes")),
            editor: None,
        };
        let cli_dir = PathBuf::from("/cli/notes");
        assert_eq!(
            config.notes_dir(Some(&cli_dir)),
            PathBuf::from("/cli/notes")
        );
    }

    #[test]
    fn notes_dir_falls_back_to_config() {
        let config = Config {
            dir: Some(PathBuf::from("/config/notes")),
            editor: None,
        };
        assert_eq!(config.notes_dir(None), PathBuf::from("/config/notes"));
    }

    #[test]
    fn notes_dir_falls_back_to_cwd() {
        let config = Config::default();
        assert_eq!(config.notes_dir(None), PathBuf::from("."));
    }

    #[test]
    fn config_path_is_in_config_dir() {
        let path = Config::config_path();
        assert!(path.ends_with("den/config.toml"));
    }
}
