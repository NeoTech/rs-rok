use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::debug;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CloudflareConfig {
    pub account_id: String,
    pub api_token: String,
}

impl CloudflareConfig {
    /// Resolve the config file path: `~/.rs-rok/cloudflare.json`.
    pub fn config_path() -> PathBuf {
        let home = dirs::home_dir().expect("cannot determine home directory");
        home.join(".rs-rok").join("cloudflare.json")
    }

    /// Load from disk, applying env var overrides (`CF_ACCOUNT_ID`, `CF_API_TOKEN`).
    /// Returns `None` if file doesn't exist and env vars aren't set.
    pub fn load(path: &Path) -> Option<Self> {
        let mut config: Option<Self> = if path.exists() {
            let data = std::fs::read_to_string(path).ok()?;
            serde_json::from_str(&data).ok()
        } else {
            None
        };

        let env_account = std::env::var("CF_ACCOUNT_ID").ok();
        let env_token = std::env::var("CF_API_TOKEN").ok();

        match (&mut config, env_account, env_token) {
            (Some(c), Some(id), _) => {
                debug!("overriding account_id from CF_ACCOUNT_ID env var");
                c.account_id = id;
            }
            (Some(c), _, Some(tok)) => {
                debug!("overriding api_token from CF_API_TOKEN env var");
                c.api_token = tok;
            }
            (None, Some(id), Some(tok)) => {
                debug!("using CF_ACCOUNT_ID + CF_API_TOKEN env vars");
                config = Some(Self {
                    account_id: id,
                    api_token: tok,
                });
            }
            _ => {}
        }

        config
    }

    /// Save to disk, creating the parent directory if needed.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
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
            account_id: "abc123".into(),
            api_token: "tok456".into(),
        };
        config.save(&path).unwrap();

        let loaded = CloudflareConfig::load(&path).expect("should load");
        assert_eq!(loaded.account_id, "abc123");
        assert_eq!(loaded.api_token, "tok456");
    }

    #[test]
    fn load_missing_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        // Clear env vars to ensure they don't interfere
        std::env::remove_var("CF_ACCOUNT_ID");
        std::env::remove_var("CF_API_TOKEN");
        assert!(CloudflareConfig::load(&path).is_none());
    }
}
