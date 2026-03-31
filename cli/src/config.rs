use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::debug;

const DEFAULT_ENDPOINT: &str = "http://localhost:8787";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    #[serde(default)]
    pub auth_token: Option<String>,

    #[serde(default = "default_endpoint")]
    pub endpoint: String,

    #[serde(default = "default_region")]
    pub default_region: String,
}

fn default_endpoint() -> String {
    DEFAULT_ENDPOINT.to_string()
}

fn default_region() -> String {
    "auto".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auth_token: None,
            endpoint: default_endpoint(),
            default_region: default_region(),
        }
    }
}

impl Settings {
    /// Resolve the config file path: CLI override > default (~/.rs-rok/settings.json).
    pub fn config_path(override_path: Option<&str>) -> PathBuf {
        if let Some(p) = override_path {
            return PathBuf::from(p);
        }
        let home = dirs::home_dir().expect("cannot determine home directory");
        home.join(".rs-rok").join("settings.json")
    }

    /// Load settings from disk, applying env var overrides on top.
    pub fn load(path: &Path) -> Self {
        let mut settings = if path.exists() {
            let data = std::fs::read_to_string(path).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Self::default()
        };

        // Environment variable overrides
        if let Ok(token) = std::env::var("RS_ROK_TOKEN") {
            debug!("overriding auth_token from RS_ROK_TOKEN env var");
            settings.auth_token = Some(token);
        }
        if let Ok(endpoint) = std::env::var("RS_ROK_ENDPOINT") {
            debug!("overriding endpoint from RS_ROK_ENDPOINT env var");
            settings.endpoint = endpoint;
        }

        settings
    }

    /// Save settings to disk, creating the parent directory if needed.
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
    fn default_settings() {
        let s = Settings::default();
        assert!(s.auth_token.is_none());
        assert_eq!(s.endpoint, "http://localhost:8787");
        assert_eq!(s.default_region, "auto");
    }

    #[test]
    fn round_trip_save_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let mut settings = Settings::default();
        settings.auth_token = Some("test-token".into());
        settings.endpoint = "https://example.workers.dev".into();
        settings.save(&path).unwrap();

        let loaded = Settings::load(&path);
        assert_eq!(loaded.auth_token.as_deref(), Some("test-token"));
        assert_eq!(loaded.endpoint, "https://example.workers.dev");
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.json");
        let settings = Settings::load(&path);
        assert!(settings.auth_token.is_none());
        assert_eq!(settings.endpoint, "http://localhost:8787");
    }
}
