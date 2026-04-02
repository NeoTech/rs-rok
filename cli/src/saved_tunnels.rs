use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Settings;

/// Tunnel type as stored on disk (the protocol crate's TunnelType has no serde derives).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SavedTunnelType {
    Http,
    Https,
    Tcp,
}

impl From<rs_rok_protocol::TunnelType> for SavedTunnelType {
    fn from(t: rs_rok_protocol::TunnelType) -> Self {
        match t {
            rs_rok_protocol::TunnelType::Http => Self::Http,
            rs_rok_protocol::TunnelType::Https => Self::Https,
            rs_rok_protocol::TunnelType::Tcp => Self::Tcp,
        }
    }
}

impl From<SavedTunnelType> for rs_rok_protocol::TunnelType {
    fn from(t: SavedTunnelType) -> Self {
        match t {
            SavedTunnelType::Http => Self::Http,
            SavedTunnelType::Https => Self::Https,
            SavedTunnelType::Tcp => Self::Tcp,
        }
    }
}

/// Whether the tunnel was active when the session ended.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SavedTunnelState {
    #[default]
    Running,
    Stopped,
}

/// One persisted tunnel entry.
///
/// The profile name is stored so that the endpoint and auth token can be looked up
/// from [`Settings`] at restore time.  The TCP token is preserved so that client
/// connections configured against a specific token continue to work across restarts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedTunnel {
    /// Name of the settings profile to use (endpoint + auth_token come from here).
    pub profile: String,
    pub tunnel_type: SavedTunnelType,
    /// Local hostname, e.g. `"localhost"`.
    pub host: String,
    /// Local port number.
    pub port: u16,
    /// Whether the tunnel was running or stopped when the session ended.
    /// Defaults to `running` so files written before this field existed reconnect automatically.
    #[serde(default)]
    pub state: SavedTunnelState,
    /// Optional URL slug / display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// For TCP tunnels: the random token clients use to connect.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tcp_token: Option<String>,
}

impl SavedTunnel {
    /// Reconstruct a [`crate::tunnel::TunnelConfig`] using the named profile from `settings`.
    /// Returns `None` when the profile no longer exists.
    pub fn to_tunnel_config(
        &self,
        settings: &Settings,
    ) -> Option<crate::tunnel::TunnelConfig> {
        let profile = settings.profiles.iter().find(|p| p.name == self.profile)?;
        Some(crate::tunnel::TunnelConfig {
            endpoint: profile.endpoint.clone(),
            auth_token: profile.auth_token.clone().unwrap_or_default(),
            tunnel_type: self.tunnel_type.clone().into(),
            local_addr: format!("{}:{}", self.host, self.port),
            name: self.name.clone(),
            tcp_token: self.tcp_token.clone(),
            events_tx: None,
        })
    }
}

/// Path to `~/.rs-rok/tunnels.json` (or next to whatever settings file is in use).
pub fn config_path(settings_path: &Path) -> PathBuf {
    settings_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("tunnels.json")
}

/// Load the saved tunnel list.  Returns an empty list on any error.
pub fn load(path: &Path) -> Vec<SavedTunnel> {
    let Ok(data) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&data).unwrap_or_default()
}

/// Persist the current tunnel list.  Silently ignores write errors.
pub fn save(path: &Path, tunnels: &[SavedTunnel]) {
    if let Ok(json) = serde_json::to_string_pretty(tunnels) {
        let _ = std::fs::write(path, json);
    }
}
