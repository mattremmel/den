//! Configuration file support.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Application configuration loaded from config file.
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    /// Default notes directory (legacy, still supported)
    pub dir: Option<PathBuf>,

    /// Editor command for editing notes
    pub editor: Option<String>,

    /// Default vault name when no --dir or --vault specified
    pub default_vault: Option<String>,

    /// Named vault mappings
    #[serde(default)]
    pub vaults: HashMap<String, PathBuf>,
}

/// Result of resolving the notes directory.
#[derive(Debug, Clone)]
pub struct ResolvedDir {
    /// The resolved path to the notes directory
    pub path: PathBuf,
    /// The vault name if resolved from a vault, None if from --dir or legacy config
    pub vault_name: Option<String>,
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
    #[deprecated(note = "Use resolve_notes_dir instead for vault support")]
    pub fn notes_dir(&self, cli_dir: Option<&PathBuf>) -> PathBuf {
        cli_dir
            .cloned()
            .or_else(|| self.dir.clone())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Resolve the notes directory with vault support.
    ///
    /// Precedence order:
    /// 1. `--dir` CLI argument (explicit path) - highest
    /// 2. `--vault` CLI argument (vault name lookup)
    /// 3. `default_vault` config (vault name lookup)
    /// 4. `dir` config (legacy direct path)
    /// 5. Current working directory - lowest
    pub fn resolve_notes_dir(
        &self,
        cli_dir: Option<&PathBuf>,
        cli_vault: Option<&str>,
    ) -> Result<ResolvedDir> {
        // 1. CLI --dir takes highest precedence
        if let Some(dir) = cli_dir {
            return Ok(ResolvedDir {
                path: dir.clone(),
                vault_name: None,
            });
        }

        // 2. CLI --vault
        if let Some(vault_name) = cli_vault {
            return self.resolve_vault(vault_name);
        }

        // 3. default_vault config
        if let Some(ref default_vault) = self.default_vault {
            return self.resolve_vault(default_vault);
        }

        // 4. Legacy dir config
        if let Some(ref dir) = self.dir {
            return Ok(ResolvedDir {
                path: dir.clone(),
                vault_name: None,
            });
        }

        // 5. Current working directory
        Ok(ResolvedDir {
            path: PathBuf::from("."),
            vault_name: None,
        })
    }

    /// Resolve a vault name to its path.
    pub fn resolve_vault(&self, name: &str) -> Result<ResolvedDir> {
        match self.vaults.get(name) {
            Some(path) => Ok(ResolvedDir {
                path: path.clone(),
                vault_name: Some(name.to_string()),
            }),
            None => {
                let available = self.list_vault_names();
                if available.is_empty() {
                    bail!("vault '{}' not found (no vaults configured)", name);
                } else {
                    bail!(
                        "vault '{}' not found. Available vaults: {}",
                        name,
                        available.join(", ")
                    );
                }
            }
        }
    }

    /// List all configured vault names.
    pub fn list_vault_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.vaults.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// List all configured vaults as (name, path) pairs.
    pub fn list_vaults(&self) -> Vec<(&str, &Path)> {
        let mut vaults: Vec<(&str, &Path)> = self
            .vaults
            .iter()
            .map(|(name, path)| (name.as_str(), path.as_path()))
            .collect();
        vaults.sort_by(|a, b| a.0.cmp(b.0));
        vaults
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
            .filter(|s| !s.is_empty())
            .or_else(|| std::env::var("EDITOR").ok().filter(|s| !s.is_empty()))
            .or_else(|| std::env::var("VISUAL").ok().filter(|s| !s.is_empty()))
            .unwrap_or_else(|| "vi".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config_with_vaults() -> Config {
        let mut vaults = HashMap::new();
        vaults.insert("personal".to_string(), PathBuf::from("/home/user/notes/personal"));
        vaults.insert("work".to_string(), PathBuf::from("/home/user/notes/work"));
        Config {
            dir: Some(PathBuf::from("/legacy/notes")),
            editor: None,
            default_vault: Some("personal".to_string()),
            vaults,
        }
    }

    #[test]
    fn default_config_has_no_dir() {
        let config = Config::default();
        assert!(config.dir.is_none());
    }

    #[test]
    #[allow(deprecated)]
    fn notes_dir_prefers_cli_arg() {
        let config = Config {
            dir: Some(PathBuf::from("/config/notes")),
            editor: None,
            default_vault: None,
            vaults: HashMap::new(),
        };
        let cli_dir = PathBuf::from("/cli/notes");
        assert_eq!(
            config.notes_dir(Some(&cli_dir)),
            PathBuf::from("/cli/notes")
        );
    }

    #[test]
    #[allow(deprecated)]
    fn notes_dir_falls_back_to_config() {
        let config = Config {
            dir: Some(PathBuf::from("/config/notes")),
            editor: None,
            default_vault: None,
            vaults: HashMap::new(),
        };
        assert_eq!(config.notes_dir(None), PathBuf::from("/config/notes"));
    }

    #[test]
    #[allow(deprecated)]
    fn notes_dir_falls_back_to_cwd() {
        let config = Config::default();
        assert_eq!(config.notes_dir(None), PathBuf::from("."));
    }

    #[test]
    fn config_path_is_in_config_dir() {
        let path = Config::config_path();
        assert!(path.ends_with("den/config.toml"));
    }

    #[test]
    fn editor_uses_config_setting() {
        let config = Config {
            dir: None,
            editor: Some("nvim".to_string()),
            default_vault: None,
            vaults: HashMap::new(),
        };
        assert_eq!(config.editor(), "nvim");
    }

    #[test]
    fn editor_skips_empty_config_setting() {
        let config = Config {
            dir: None,
            editor: Some("".to_string()),
            default_vault: None,
            vaults: HashMap::new(),
        };
        // Should fall through to env vars or default, not use empty string
        // This test verifies empty config editor is skipped
        let editor = config.editor();
        assert!(!editor.is_empty());
    }

    #[test]
    fn editor_returns_non_empty_value() {
        // With default config, should return something (either from env or "vi")
        let config = Config::default();
        let result = config.editor();
        assert!(
            !result.is_empty(),
            "editor should never return empty string"
        );
    }

    // Vault resolution tests

    #[test]
    fn resolve_notes_dir_cli_dir_takes_precedence() {
        let config = make_config_with_vaults();
        let cli_dir = PathBuf::from("/cli/override");
        let resolved = config.resolve_notes_dir(Some(&cli_dir), Some("work")).unwrap();
        assert_eq!(resolved.path, PathBuf::from("/cli/override"));
        assert!(resolved.vault_name.is_none());
    }

    #[test]
    fn resolve_notes_dir_cli_vault_second() {
        let config = make_config_with_vaults();
        let resolved = config.resolve_notes_dir(None, Some("work")).unwrap();
        assert_eq!(resolved.path, PathBuf::from("/home/user/notes/work"));
        assert_eq!(resolved.vault_name, Some("work".to_string()));
    }

    #[test]
    fn resolve_notes_dir_default_vault_third() {
        let config = make_config_with_vaults();
        let resolved = config.resolve_notes_dir(None, None).unwrap();
        assert_eq!(resolved.path, PathBuf::from("/home/user/notes/personal"));
        assert_eq!(resolved.vault_name, Some("personal".to_string()));
    }

    #[test]
    fn resolve_notes_dir_legacy_dir_fourth() {
        let config = Config {
            dir: Some(PathBuf::from("/legacy/notes")),
            editor: None,
            default_vault: None,
            vaults: HashMap::new(),
        };
        let resolved = config.resolve_notes_dir(None, None).unwrap();
        assert_eq!(resolved.path, PathBuf::from("/legacy/notes"));
        assert!(resolved.vault_name.is_none());
    }

    #[test]
    fn resolve_notes_dir_cwd_fallback() {
        let config = Config::default();
        let resolved = config.resolve_notes_dir(None, None).unwrap();
        assert_eq!(resolved.path, PathBuf::from("."));
        assert!(resolved.vault_name.is_none());
    }

    #[test]
    fn resolve_vault_success() {
        let config = make_config_with_vaults();
        let resolved = config.resolve_vault("work").unwrap();
        assert_eq!(resolved.path, PathBuf::from("/home/user/notes/work"));
        assert_eq!(resolved.vault_name, Some("work".to_string()));
    }

    #[test]
    fn resolve_vault_not_found_with_suggestions() {
        let config = make_config_with_vaults();
        let err = config.resolve_vault("nonexistent").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not found"));
        assert!(msg.contains("personal"));
        assert!(msg.contains("work"));
    }

    #[test]
    fn resolve_vault_not_found_no_vaults() {
        let config = Config::default();
        let err = config.resolve_vault("any").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("no vaults configured"));
    }

    #[test]
    fn list_vault_names_sorted() {
        let config = make_config_with_vaults();
        let names = config.list_vault_names();
        assert_eq!(names, vec!["personal", "work"]);
    }

    #[test]
    fn list_vaults_sorted() {
        let config = make_config_with_vaults();
        let vaults = config.list_vaults();
        assert_eq!(vaults.len(), 2);
        assert_eq!(vaults[0].0, "personal");
        assert_eq!(vaults[1].0, "work");
    }
}
