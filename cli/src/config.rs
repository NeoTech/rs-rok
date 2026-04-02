use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::debug;

const DEFAULT_ENDPOINT: &str = "http://localhost:8787";

/// A single named profile. The settings file on disk is `Vec<Profile>`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Profile {
    #[serde(default)]
    pub name: String,

    pub endpoint: String,

    #[serde(default)]
    pub auth_token: Option<String>,

    #[serde(default = "default_region")]
    pub default_region: String,

    /// Cloudflare Account ID for deploy (optional, per-profile).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cf_account_id: Option<String>,

    /// Cloudflare API Token for deploy (optional, per-profile).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cf_api_token: Option<String>,
}

impl Profile {
    pub fn new(name: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            endpoint: endpoint.into(),
            auth_token: None,
            default_region: default_region(),
            cf_account_id: None,
            cf_api_token: None,
        }
    }

    pub fn default_profile() -> Self {
        Self::new("default", default_endpoint())
    }
}

/// In-memory settings: a list of profiles with an active index.
///
/// On disk the file is a plain JSON array: `[{...}, {...}]`.
/// The first profile in the array is treated as the active one unless
/// overridden by `--profile` or `switch_active()`.
#[derive(Debug, Clone)]
pub struct Settings {
    pub profiles: Vec<Profile>,
    /// Index into `profiles` for the currently active profile.
    pub active_idx: usize,
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
            profiles: vec![Profile::default_profile()],
            active_idx: 0,
        }
    }
}

// -- Legacy format structs for migration --

/// Old format v2: `{ "active_profile": "...", "profiles": [...] }`
#[derive(Deserialize)]
struct LegacyWrapped {
    #[serde(default)]
    active_profile: String,
    profiles: Vec<Profile>,
}

/// Old format v1 (flat): `{ "auth_token": "...", "endpoint": "...", ... }`
#[derive(Deserialize)]
struct LegacyFlat {
    #[serde(default)]
    auth_token: Option<String>,
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default)]
    default_region: Option<String>,
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

    /// Load settings from disk, detecting and migrating legacy formats.
    ///
    /// Supported on-disk formats (tried in order):
    /// 1. Array of profiles: `[{ "name": "x", "endpoint": "..." }, ...]`
    /// 2. Wrapped object:    `{ "active_profile": "x", "profiles": [...] }`
    /// 3. Flat object:       `{ "endpoint": "...", "auth_token": "..." }`
    ///
    /// After migration the file is re-saved as format 1.
    pub fn load(path: &Path) -> Self {
        if !path.exists() {
            return Self::default();
        }

        let data = std::fs::read_to_string(path).unwrap_or_default();
        let trimmed = data.trim();
        if trimmed.is_empty() {
            return Self::default();
        }

        let (mut settings, migrated) = if trimmed.starts_with('[') {
            // Format 1: array -- the canonical format
            match serde_json::from_str::<Vec<Profile>>(trimmed) {
                Ok(profiles) if !profiles.is_empty() => (
                    Self {
                        profiles,
                        active_idx: 0,
                    },
                    false,
                ),
                _ => (Self::default(), false),
            }
        } else if let Ok(wrapped) = serde_json::from_str::<LegacyWrapped>(trimmed) {
            // Format 2: wrapped { active_profile, profiles }
            if wrapped.profiles.is_empty() {
                (Self::default(), true)
            } else {
                let active_idx = wrapped
                    .profiles
                    .iter()
                    .position(|p| p.name == wrapped.active_profile)
                    .unwrap_or(0);
                (
                    Self {
                        profiles: wrapped.profiles,
                        active_idx,
                    },
                    true,
                )
            }
        } else if let Ok(flat) = serde_json::from_str::<LegacyFlat>(trimmed) {
            // Format 3: flat { endpoint, auth_token, default_region }
            let profile = Profile {
                name: "default".to_string(),
                endpoint: flat.endpoint.unwrap_or_else(default_endpoint),
                auth_token: flat.auth_token,
                default_region: flat.default_region.unwrap_or_else(default_region),
                cf_account_id: None,
                cf_api_token: None,
            };
            (
                Self {
                    profiles: vec![profile],
                    active_idx: 0,
                },
                true,
            )
        } else {
            (Self::default(), false)
        };

        // Auto-save migrated format
        if migrated {
            let _ = settings.save(path);
        }

        // Environment variable overrides on the active profile
        if let Ok(token) = std::env::var("RS_ROK_TOKEN") {
            debug!("overriding auth_token from RS_ROK_TOKEN env var");
            settings.active_profile_mut().auth_token = Some(token);
        }
        if let Ok(ep) = std::env::var("RS_ROK_ENDPOINT") {
            debug!("overriding endpoint from RS_ROK_ENDPOINT env var");
            settings.active_profile_mut().endpoint = ep;
        }

        settings
    }

    /// Save settings to disk as a JSON array of profiles.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.profiles)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    /// Reference to the active profile.
    pub fn active_profile(&self) -> &Profile {
        &self.profiles[self.active_idx]
    }

    /// Mutable reference to the active profile.
    pub fn active_profile_mut(&mut self) -> &mut Profile {
        &mut self.profiles[self.active_idx]
    }

    /// Switch active profile by index.
    pub fn switch_active(&mut self, idx: usize) {
        if idx < self.profiles.len() {
            self.active_idx = idx;
        }
    }

    /// Switch active profile by name. Returns true if found.
    pub fn switch_active_by_name(&mut self, name: &str) -> bool {
        if let Some(idx) = self.profiles.iter().position(|p| p.name == name) {
            self.active_idx = idx;
            true
        } else {
            false
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings() {
        let s = Settings::default();
        assert_eq!(s.active_idx, 0);
        assert_eq!(s.profiles.len(), 1);
        let p = s.active_profile();
        assert!(p.auth_token.is_none());
        assert_eq!(p.endpoint, "http://localhost:8787");
        assert_eq!(p.default_region, "auto");
    }

    #[test]
    fn round_trip_array_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let mut settings = Settings::default();
        settings.profiles[0].auth_token = Some("test-token".into());
        settings.profiles[0].endpoint = "https://example.workers.dev".into();
        settings.save(&path).unwrap();

        // Verify it's saved as a JSON array
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.trim().starts_with('['), "should be a JSON array on disk");

        let loaded = Settings::load(&path);
        let p = loaded.active_profile();
        assert_eq!(p.auth_token.as_deref(), Some("test-token"));
        assert_eq!(p.endpoint, "https://example.workers.dev");
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.json");
        let settings = Settings::load(&path);
        let p = settings.active_profile();
        assert!(p.auth_token.is_none());
        assert_eq!(p.endpoint, "http://localhost:8787");
    }

    #[test]
    fn migrate_legacy_flat_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let legacy = r#"{"auth_token":"old-token","endpoint":"https://old.example.dev","default_region":"us"}"#;
        std::fs::write(&path, legacy).unwrap();

        let loaded = Settings::load(&path);
        let p = loaded.active_profile();
        assert_eq!(p.auth_token.as_deref(), Some("old-token"));
        assert_eq!(p.endpoint, "https://old.example.dev");
        assert_eq!(p.default_region, "us");
        assert_eq!(p.name, "default");

        // Verify migrated to array on disk
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.trim().starts_with('['));
    }

    #[test]
    fn migrate_wrapped_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let wrapped = r#"{"active_profile":"staging","profiles":[{"name":"default","endpoint":"http://localhost:8787","default_region":"auto"},{"name":"staging","endpoint":"https://staging.dev","default_region":"eu"}]}"#;
        std::fs::write(&path, wrapped).unwrap();

        let loaded = Settings::load(&path);
        assert_eq!(loaded.profiles.len(), 2);
        assert_eq!(loaded.active_idx, 1);
        assert_eq!(loaded.active_profile().name, "staging");
        assert_eq!(loaded.active_profile().endpoint, "https://staging.dev");

        // Verify migrated to array
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.trim().starts_with('['));
    }

    #[test]
    fn load_user_array_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let array = r#"[
            {"name": "production", "endpoint": "https://rs-rok.workers.dev", "default_region": "auto"},
            {"name": "local", "endpoint": "http://localhost:8787", "default_region": "auto"}
        ]"#;
        std::fs::write(&path, array).unwrap();

        let loaded = Settings::load(&path);
        assert_eq!(loaded.profiles.len(), 2);
        assert_eq!(loaded.active_idx, 0);
        assert_eq!(loaded.profiles[0].name, "production");
        assert_eq!(loaded.profiles[1].name, "local");
    }

    #[test]
    fn multiple_profiles_with_cf_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let mut settings = Settings::default();
        settings.profiles[0].cf_account_id = Some("abc123".into());
        settings.profiles[0].cf_api_token = Some("tok456".into());
        settings.profiles.push(Profile::new("staging", "https://staging.dev"));
        settings.save(&path).unwrap();

        let loaded = Settings::load(&path);
        assert_eq!(loaded.profiles.len(), 2);
        assert_eq!(loaded.profiles[0].cf_account_id.as_deref(), Some("abc123"));
        assert!(loaded.profiles[1].cf_account_id.is_none());
    }

    #[test]
    fn switch_active_by_name() {
        let mut settings = Settings::default();
        settings.profiles.push(Profile::new("staging", "https://staging.dev"));

        assert_eq!(settings.active_idx, 0);
        assert!(settings.switch_active_by_name("staging"));
        assert_eq!(settings.active_idx, 1);
        assert_eq!(settings.active_profile().name, "staging");

        assert!(!settings.switch_active_by_name("nonexistent"));
        assert_eq!(settings.active_idx, 1);
    }
}
