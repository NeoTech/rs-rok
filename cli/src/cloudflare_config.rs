use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::debug;

/// A single Cloudflare account entry.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CfAccount {
    #[serde(default)]
    pub name: String,
    pub account_id: String,
    pub api_token: String,
}

/// List of Cloudflare accounts stored in `~/.rs-rok/cloudflare.json`.
///
/// On disk the file is a JSON array: `[{name, account_id, api_token}, ...]`.
/// For backward compatibility, a single object `{account_id, api_token}` is
/// also accepted and migrated to the array format on next save.
#[derive(Debug, Clone)]
pub struct CloudflareConfig {
    pub accounts: Vec<CfAccount>,
}

impl CloudflareConfig {
    /// Resolve the config file path: `~/.rs-rok/cloudflare.json`.
    pub fn config_path() -> PathBuf {
        let home = dirs::home_dir().expect("cannot determine home directory");
        home.join(".rs-rok").join("cloudflare.json")
    }

    /// Load from disk. Supports both array and single-object formats.
    /// Applies env var overrides (`CF_ACCOUNT_ID`, `CF_API_TOKEN`) to the first account.
    pub fn load(path: &Path) -> Self {
        let mut cfg = Self { accounts: Vec::new() };

        if path.exists() {
            if let Ok(data) = std::fs::read_to_string(path) {
                let trimmed = data.trim();
                if trimmed.starts_with('[') {
                    if let Ok(accounts) = serde_json::from_str::<Vec<CfAccount>>(trimmed) {
                        cfg.accounts = accounts;
                    }
                } else if let Ok(single) = serde_json::from_str::<CfAccount>(trimmed) {
                    cfg.accounts = vec![single];
                    // Migrate to array format
                    let _ = cfg.save(path);
                }
            }
        }

        // Env var overrides on the first account
        let env_account = std::env::var("CF_ACCOUNT_ID").ok();
        let env_token = std::env::var("CF_API_TOKEN").ok();

        match (env_account, env_token) {
            (Some(id), Some(tok)) if cfg.accounts.is_empty() => {
                debug!("using CF_ACCOUNT_ID + CF_API_TOKEN env vars");
                cfg.accounts.push(CfAccount {
                    name: "env".to_string(),
                    account_id: id,
                    api_token: tok,
                });
            }
            (id_opt, tok_opt) if !cfg.accounts.is_empty() => {
                if let Some(id) = id_opt {
                    debug!("overriding first account_id from CF_ACCOUNT_ID env var");
                    cfg.accounts[0].account_id = id;
                }
                if let Some(tok) = tok_opt {
                    debug!("overriding first api_token from CF_API_TOKEN env var");
                    cfg.accounts[0].api_token = tok;
                }
            }
            _ => {}
        }

        cfg
    }

    /// Save to disk as a JSON array.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.accounts)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    /// Get the first account (convenience for single-account usage).
    pub fn first(&self) -> Option<&CfAccount> {
        self.accounts.first()
    }

    /// Insert or replace the first account with a matching account_id.
    /// If no match exists, replaces index 0 (or pushes if empty).
    pub fn upsert_account(&mut self, account: CfAccount) {
        if let Some(pos) = self
            .accounts
            .iter()
            .position(|a| a.account_id == account.account_id)
        {
            self.accounts[pos] = account;
        } else if self.accounts.is_empty() {
            self.accounts.push(account);
        } else {
            self.accounts[0] = account;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_save_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cloudflare.json");

        let config = CloudflareConfig {
            accounts: vec![CfAccount {
                name: "test".into(),
                account_id: "abc123".into(),
                api_token: "tok456".into(),
            }],
        };
        config.save(&path).unwrap();

        let loaded = CloudflareConfig::load(&path);
        assert_eq!(loaded.accounts.len(), 1);
        assert_eq!(loaded.accounts[0].account_id, "abc123");
        assert_eq!(loaded.accounts[0].api_token, "tok456");
        assert_eq!(loaded.accounts[0].name, "test");
    }

    #[test]
    fn load_legacy_single_object() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cloudflare.json");
        std::fs::write(&path, r#"{"account_id":"abc","api_token":"tok"}"#).unwrap();

        let loaded = CloudflareConfig::load(&path);
        assert_eq!(loaded.accounts.len(), 1);
        assert_eq!(loaded.accounts[0].account_id, "abc");
    }

    #[test]
    fn load_missing_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        std::env::remove_var("CF_ACCOUNT_ID");
        std::env::remove_var("CF_API_TOKEN");
        let loaded = CloudflareConfig::load(&path);
        assert!(loaded.accounts.is_empty());
    }
}
