//! Handler for the `vaults` command.

use anyhow::Result;

use crate::cli::config::Config;
use crate::cli::output::OutputFormat;
use crate::cli::VaultsArgs;

/// Handle the vaults command - list configured vaults or set default.
pub fn handle_vaults(args: &VaultsArgs, config: &Config) -> Result<()> {
    // Handle --set-default
    if let Some(ref vault_name) = args.set_default {
        Config::set_default_vault(vault_name)?;
        println!("Default vault set to '{}'", vault_name);
        return Ok(());
    }

    // List vaults
    let vaults = config.list_vaults();
    let default_vault = config.default_vault.as_deref();

    match args.format {
        OutputFormat::Human => {
            if vaults.is_empty() {
                println!("No vaults configured.");
                println!();
                println!("Add vaults to your config file (~/.config/notes/config.toml):");
                println!();
                println!("  [vaults]");
                println!("  personal = \"/path/to/personal/notes\"");
                println!("  work = \"/path/to/work/notes\"");
                println!();
                println!("  # Optional: set a default vault");
                println!("  default_vault = \"personal\"");
            } else {
                for (name, path) in &vaults {
                    let marker = if Some(*name) == default_vault {
                        " (default)"
                    } else {
                        ""
                    };
                    println!("{}{}: {}", name, marker, path.display());
                }
            }
        }
        OutputFormat::Json => {
            let output: Vec<serde_json::Value> = vaults
                .iter()
                .map(|(name, path)| {
                    serde_json::json!({
                        "name": name,
                        "path": path.to_string_lossy(),
                        "default": Some(*name) == default_vault,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Paths => {
            // Output just vault names, one per line
            for (name, _) in &vaults {
                println!("{}", name);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_config_with_vaults() -> Config {
        let mut vaults = HashMap::new();
        vaults.insert(
            "personal".to_string(),
            PathBuf::from("/home/user/notes/personal"),
        );
        vaults.insert("work".to_string(), PathBuf::from("/home/user/notes/work"));
        Config {
            dir: None,
            editor: None,
            default_vault: Some("personal".to_string()),
            vaults,
        }
    }

    #[test]
    fn handle_vaults_runs_without_error() {
        let config = make_config_with_vaults();
        let args = VaultsArgs {
            set_default: None,
            format: OutputFormat::Human,
        };
        // Just verify it doesn't panic/error
        let result = handle_vaults(&args, &config);
        assert!(result.is_ok());
    }

    #[test]
    fn handle_vaults_empty_config() {
        let config = Config::default();
        let args = VaultsArgs {
            set_default: None,
            format: OutputFormat::Human,
        };
        let result = handle_vaults(&args, &config);
        assert!(result.is_ok());
    }
}
