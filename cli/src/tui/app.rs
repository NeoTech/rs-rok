use std::collections::VecDeque;
use std::path::PathBuf;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::config::{Profile, Settings};
use crate::tunnel::TunnelConfig;

use super::events::TunnelEvent;

const MAX_LOG_LINES: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    List,
    Logs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    NewTunnel,
    Settings,
    Profiles,
    Deploy,
    EndpointTest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelStatus {
    Connecting,
    Active,
    Stopped,
}

pub struct TunnelHandle {
    pub name: String,
    pub config_summary: String,
    pub status: TunnelStatus,
    pub tunnel_type: rs_rok_protocol::TunnelType,
    pub events_rx: mpsc::UnboundedReceiver<TunnelEvent>,
    pub test_tx: mpsc::UnboundedSender<TunnelEvent>,
    pub task_handle: JoinHandle<()>,
    pub logs: VecDeque<LogLine>,
    pub scroll_offset: u16,
    pub auto_scroll: bool,
    pub public_url: Option<String>,
    pub test_result: Option<EndpointTestResult>,
    /// Base config used to restart this tunnel (events_tx is always None here).
    pub base_config: TunnelConfig,
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub timestamp: String,
    pub text: String,
    pub style: LogStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStyle {
    In,       // incoming request  → green  [IN ]
    Out,      // outgoing response → orange [OUT]
    OutError, // error response    → red    [ERR]
    System,   // info / lifecycle  → gray   [---]
    Error,    // fatal error       → red    [ERR]
}

// -- New tunnel form state --

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelTypeOption {
    Http,
    Https,
    Tcp,
}

impl TunnelTypeOption {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Http => "HTTP",
            Self::Https => "HTTPS",
            Self::Tcp => "TCP",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Http => Self::Https,
            Self::Https => Self::Tcp,
            Self::Tcp => Self::Http,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Http => Self::Tcp,
            Self::Https => Self::Http,
            Self::Tcp => Self::Https,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewTunnelForm {
    pub profile_idx: usize,   // index into Settings.profiles
    pub tunnel_type: TunnelTypeOption,
    pub port: String,
    pub name: String,
    pub host: String,
    pub focused_field: usize, // 0=profile, 1=type, 2=port, 3=name, 4=host
}

impl NewTunnelForm {
    pub const FIELD_COUNT: usize = 5;

    pub fn new(active_idx: usize) -> Self {
        Self {
            profile_idx: active_idx,
            tunnel_type: TunnelTypeOption::Http,
            port: String::new(),
            name: String::new(),
            host: "localhost".to_string(),
            focused_field: 0, // Start on profile selector
        }
    }

    pub fn focus_next(&mut self) {
        self.focused_field = (self.focused_field + 1) % Self::FIELD_COUNT;
    }

    pub fn focus_prev(&mut self) {
        self.focused_field = (self.focused_field + Self::FIELD_COUNT - 1) % Self::FIELD_COUNT;
    }
}

// -- Settings form state --

#[derive(Debug, Clone)]
pub struct SettingsForm {
    // -- Profile section --
    pub profile_idx: usize,
    pub profile_fields: Vec<(String, String)>,

    // -- CF account section --
    pub cf_account_idx: usize,
    pub cf_accounts: Vec<crate::cloudflare_config::CfAccount>,
    pub cf_fields: Vec<(String, String)>,

    /// Total field count: 1 (profile selector) + profile_fields + 1 (CF selector) + cf_fields
    pub focused_field: usize,
    pub editing: bool,
}

impl SettingsForm {
    /// Number of profile data fields (Name, Endpoint, Auth Token, Region).
    const PROFILE_FIELD_COUNT: usize = 4;
    /// Number of CF account data fields (Name, Account ID, API Token).
    const CF_FIELD_COUNT: usize = 3;

    /// Index where the CF account selector row lives.
    pub fn cf_selector_idx(&self) -> usize {
        1 + Self::PROFILE_FIELD_COUNT // profile selector + 4 fields
    }

    /// First index of an editable CF field.
    pub fn cf_fields_start(&self) -> usize {
        self.cf_selector_idx() + 1
    }

    /// Total number of navigable rows.
    pub fn total_rows(&self) -> usize {
        // profile selector + profile fields + cf selector + cf fields
        1 + Self::PROFILE_FIELD_COUNT + 1 + Self::CF_FIELD_COUNT
    }

    /// Whether the currently focused row is the profile selector (row 0).
    pub fn on_profile_selector(&self) -> bool {
        self.focused_field == 0
    }

    /// Whether the currently focused row is the CF account selector.
    pub fn on_cf_selector(&self) -> bool {
        self.focused_field == self.cf_selector_idx()
    }

    /// Whether the currently focused row is an editable profile field.
    pub fn on_profile_field(&self) -> bool {
        self.focused_field >= 1 && self.focused_field < self.cf_selector_idx()
    }

    /// Whether the currently focused row is an editable CF field.
    pub fn on_cf_field(&self) -> bool {
        let start = self.cf_fields_start();
        self.focused_field >= start && self.focused_field < start + Self::CF_FIELD_COUNT
    }

    /// Index into `profile_fields` for the currently focused profile field.
    pub fn profile_field_idx(&self) -> usize {
        self.focused_field - 1
    }

    /// Index into `cf_fields` for the currently focused CF field.
    pub fn cf_field_idx(&self) -> usize {
        self.focused_field - self.cf_fields_start()
    }

    pub fn new(profiles: &[Profile], active_idx: usize) -> Self {
        let cf_path = crate::cloudflare_config::CloudflareConfig::config_path();
        let cf = crate::cloudflare_config::CloudflareConfig::load(&cf_path);

        let profile_fields = Self::build_profile_fields(&profiles[active_idx]);
        let cf_fields = Self::build_cf_fields(cf.accounts.first());

        Self {
            profile_idx: active_idx,
            profile_fields,
            cf_account_idx: 0,
            cf_accounts: cf.accounts,
            cf_fields,
            focused_field: 0,
            editing: false,
        }
    }

    fn build_profile_fields(profile: &Profile) -> Vec<(String, String)> {
        vec![
            ("Name".to_string(), profile.name.clone()),
            ("Endpoint".to_string(), profile.endpoint.clone()),
            (
                "Auth Token".to_string(),
                profile.auth_token.clone().unwrap_or_default(),
            ),
            ("Region".to_string(), profile.default_region.clone()),
        ]
    }

    fn build_cf_fields(account: Option<&crate::cloudflare_config::CfAccount>) -> Vec<(String, String)> {
        match account {
            Some(a) => vec![
                ("Name".to_string(), a.name.clone()),
                ("Account ID".to_string(), a.account_id.clone()),
                ("API Token".to_string(), a.api_token.clone()),
            ],
            None => vec![
                ("Name".to_string(), String::new()),
                ("Account ID".to_string(), String::new()),
                ("API Token".to_string(), String::new()),
            ],
        }
    }

    /// Reload profile fields from the given profile.
    pub fn load_profile(&mut self, profile: &Profile) {
        self.profile_fields = Self::build_profile_fields(profile);
    }

    /// Reload CF fields from the currently selected account.
    pub fn load_cf_account(&mut self) {
        self.cf_fields = Self::build_cf_fields(self.cf_accounts.get(self.cf_account_idx));
    }

    pub fn apply_to_profile(&self, profile: &mut Profile) {
        if let Some((_, v)) = self.profile_fields.get(0) {
            profile.name = v.clone();
        }
        if let Some((_, v)) = self.profile_fields.get(1) {
            profile.endpoint = v.clone();
        }
        if let Some((_, v)) = self.profile_fields.get(2) {
            profile.auth_token = if v.is_empty() { None } else { Some(v.clone()) };
        }
        if let Some((_, v)) = self.profile_fields.get(3) {
            profile.default_region = v.clone();
        }
    }

    pub fn apply_to_cf_account(&self) -> Option<crate::cloudflare_config::CfAccount> {
        if self.cf_accounts.is_empty() && self.cf_fields.iter().all(|(_, v)| v.is_empty()) {
            return None;
        }
        Some(crate::cloudflare_config::CfAccount {
            name: self.cf_fields.get(0).map(|(_, v)| v.clone()).unwrap_or_default(),
            account_id: self.cf_fields.get(1).map(|(_, v)| v.clone()).unwrap_or_default(),
            api_token: self.cf_fields.get(2).map(|(_, v)| v.clone()).unwrap_or_default(),
        })
    }

    pub fn focus_next(&mut self) {
        self.focused_field = (self.focused_field + 1) % self.total_rows();
    }

    pub fn focus_prev(&mut self) {
        self.focused_field =
            (self.focused_field + self.total_rows() - 1) % self.total_rows();
    }
}

// -- Deploy form state --

#[derive(Debug, Clone)]
pub struct DeployForm {
    pub worker_name: String,
    pub cf_accounts: Vec<crate::cloudflare_config::CfAccount>,
    pub selected_account: usize,
    pub status_message: Option<String>,
}

impl DeployForm {
    /// Load CF accounts from cloudflare.json.
    pub fn new() -> Self {
        let cf_path = crate::cloudflare_config::CloudflareConfig::config_path();
        let cf = crate::cloudflare_config::CloudflareConfig::load(&cf_path);

        Self {
            worker_name: String::new(),
            cf_accounts: cf.accounts,
            selected_account: 0,
            status_message: None,
        }
    }

    pub fn active_account(&self) -> Option<&crate::cloudflare_config::CfAccount> {
        self.cf_accounts.get(self.selected_account)
    }
}

// -- Endpoint test form state --

pub const HTTP_METHODS: [&str; 5] = ["GET", "POST", "PUT", "DELETE", "PATCH"];

#[derive(Debug, Clone)]
pub struct EndpointTestResult {
    pub status: u16,
    pub latency_ms: u64,
    pub body_snippet: String,
}

#[derive(Debug, Clone)]
pub struct EndpointTestForm {
    pub http_method_idx: usize,
    pub path: String,
    pub body: String,
    /// Custom request headers as (name, value) pairs.
    pub headers: Vec<(String, String)>,
    /// 0=method, 1=path, 2=body, 3+2*i=header[i].key, 4+2*i=header[i].value
    pub focused_field: usize,
    pub is_running: bool,
}

impl EndpointTestForm {
    pub fn new() -> Self {
        Self {
            http_method_idx: 0,
            path: "/".to_string(),
            body: String::new(),
            // Pre-populate three empty header slots.
            headers: vec![
                (String::new(), String::new()),
                (String::new(), String::new()),
                (String::new(), String::new()),
            ],
            focused_field: 1, // start on path
            is_running: false,
        }
    }

    pub fn method(&self) -> &'static str {
        HTTP_METHODS[self.http_method_idx]
    }

    pub fn total_fields(&self) -> usize {
        3 + self.headers.len() * 2
    }

    pub fn focus_next(&mut self) {
        let total = self.total_fields();
        if total > 0 {
            self.focused_field = (self.focused_field + 1) % total;
        }
    }

    pub fn focus_prev(&mut self) {
        let total = self.total_fields();
        if total > 0 {
            self.focused_field = (self.focused_field + total - 1) % total;
        }
    }

    /// If currently focused on a header field, returns `(header_idx, is_value_col)`.
    pub fn focused_header(&self) -> Option<(usize, bool)> {
        if self.focused_field < 3 {
            return None;
        }
        let rel = self.focused_field - 3;
        Some((rel / 2, rel % 2 == 1))
    }
}

// -- Main app state --

pub struct App {
    pub settings: Settings,
    pub settings_path: PathBuf,
    pub tunnels: Vec<TunnelHandle>,
    pub selected_tunnel: usize,
    pub focus: Focus,
    pub overlay: Option<Overlay>,
    pub new_tunnel_form: NewTunnelForm,
    pub settings_form: Option<SettingsForm>,
    pub deploy_form: Option<DeployForm>,
    pub profiles_selected: usize,
    pub new_profile_input: Option<String>,
    pub endpoint_test_form: Option<EndpointTestForm>,
    /// Receives the result of an in-progress deploy (Ok(url) or Err(message)).
    pub deploy_result_rx: Option<tokio::sync::mpsc::UnboundedReceiver<Result<String, String>>>,
}

impl App {
    pub fn new(settings: Settings, settings_path: PathBuf) -> Self {
        Self {
            settings,
            settings_path,
            tunnels: Vec::new(),
            selected_tunnel: 0,
            focus: Focus::List,
            overlay: None,
            new_tunnel_form: NewTunnelForm::new(0),
            settings_form: None,
            deploy_form: None,
            profiles_selected: 0,
            new_profile_input: None,
            endpoint_test_form: None,
            deploy_result_rx: None,
        }
    }

    pub fn next_tunnel(&mut self) {
        if !self.tunnels.is_empty() {
            self.selected_tunnel = (self.selected_tunnel + 1) % self.tunnels.len();
        }
    }

    pub fn prev_tunnel(&mut self) {
        if !self.tunnels.is_empty() {
            self.selected_tunnel = (self.selected_tunnel + self.tunnels.len() - 1)
                % self.tunnels.len();
        }
    }

    pub fn spawn_tunnel(&mut self, config: TunnelConfig) {
        let (tx, rx) = mpsc::unbounded_channel();
        let test_tx = tx.clone();
        let name = config
            .name
            .clone()
            .unwrap_or_else(|| format!(":{}", config.local_addr.split(':').last().unwrap_or("?")));
        let tunnel_type = config.tunnel_type;
        let type_label = match tunnel_type {
            rs_rok_protocol::TunnelType::Http => "HTTP",
            rs_rok_protocol::TunnelType::Https => "HTTPS",
            rs_rok_protocol::TunnelType::Tcp => "TCP",
        };
        let summary = format!("{} {}", type_label, config.local_addr);
        let events_tx = tx;

        let mut tunnel_config = config;
        tunnel_config.events_tx = Some(events_tx);

        let mut base_config = tunnel_config.clone();
        base_config.events_tx = None;

        let handle = tokio::spawn(async move {
            if let Err(e) = crate::tunnel::run(tunnel_config).await {
                let _ = e;
            }
        });

        self.tunnels.push(TunnelHandle {
            name,
            config_summary: summary,
            status: TunnelStatus::Connecting,
            tunnel_type,
            events_rx: rx,
            test_tx,
            task_handle: handle,
            logs: VecDeque::with_capacity(MAX_LOG_LINES),
            scroll_offset: 0,
            auto_scroll: true,
            public_url: None,
            test_result: None,
            base_config,
        });

        self.selected_tunnel = self.tunnels.len() - 1;
    }

    pub fn restart_tunnel(&mut self, idx: usize) {
        if let Some(tunnel) = self.tunnels.get_mut(idx) {
            tunnel.task_handle.abort();

            let (tx, rx) = mpsc::unbounded_channel();
            let test_tx = tx.clone();

            let mut config = tunnel.base_config.clone();
            config.events_tx = Some(tx);

            let handle = tokio::spawn(async move {
                if let Err(e) = crate::tunnel::run(config).await {
                    let _ = e;
                }
            });

            tunnel.task_handle = handle;
            tunnel.events_rx = rx;
            tunnel.test_tx = test_tx;
            tunnel.status = TunnelStatus::Connecting;
            tunnel.public_url = None;
            tunnel.test_result = None;

            let now = chrono_now();
            tunnel.push_log(LogLine {
                timestamp: now,
                text: "Tunnel restarted".to_string(),
                style: LogStyle::System,
            });
        }
    }

    pub fn delete_tunnel(&mut self, idx: usize) {
        if idx < self.tunnels.len() {
            self.tunnels[idx].task_handle.abort();
            self.tunnels.remove(idx);
            if self.tunnels.is_empty() {
                self.selected_tunnel = 0;
            } else if self.selected_tunnel >= self.tunnels.len() {
                self.selected_tunnel = self.tunnels.len() - 1;
            }
        }
    }

    pub fn stop_tunnel(&mut self, idx: usize) {
        if let Some(tunnel) = self.tunnels.get_mut(idx) {
            tunnel.task_handle.abort();
            tunnel.status = TunnelStatus::Stopped;
            let now = chrono_now();
            tunnel.push_log(LogLine {
                timestamp: now,
                text: "Tunnel stopped".to_string(),
                style: LogStyle::System,
            });
        }
    }

    pub fn stop_all_tunnels(&mut self) {
        for tunnel in &mut self.tunnels {
            tunnel.task_handle.abort();
            tunnel.status = TunnelStatus::Stopped;
        }
    }

    pub fn poll_tunnel_events(&mut self) {
        for tunnel in &mut self.tunnels {
            while let Ok(event) = tunnel.events_rx.try_recv() {
                let now = chrono_now();
                match event {
                    TunnelEvent::Connected { url } => {
                        tunnel.status = TunnelStatus::Active;
                        tunnel.public_url = Some(url.clone());
                        tunnel.push_log(LogLine {
                            timestamp: now,
                            text: format!("Connected: {url}"),
                            style: LogStyle::System,
                        });
                    }
                    TunnelEvent::Request {
                        method,
                        path,
                        status,
                        latency_ms,
                    } => {
                        // OUT: the request going out to the local service
                        tunnel.push_log(LogLine {
                            timestamp: now.clone(),
                            text: format!("{:<6} {}", method, path),
                            style: LogStyle::Out,
                        });
                        // IN: the response coming back in
                        let out_style = if status >= 400 {
                            LogStyle::OutError
                        } else {
                            LogStyle::In
                        };
                        tunnel.push_log(LogLine {
                            timestamp: now,
                            text: format!("{status}  {latency_ms}ms"),
                            style: out_style,
                        });
                    }
                    TunnelEvent::Disconnected { reason } => {
                        tunnel.status = TunnelStatus::Stopped;
                        tunnel.push_log(LogLine {
                            timestamp: now,
                            text: format!("Disconnected: {reason}"),
                            style: LogStyle::System,
                        });
                    }
                    TunnelEvent::Error(msg) => {
                        tunnel.status = TunnelStatus::Stopped;
                        tunnel.push_log(LogLine {
                            timestamp: now,
                            text: format!("Error: {msg}"),
                            style: LogStyle::Error,
                        });
                    }
                    TunnelEvent::TestResult { status, latency_ms, ref body_snippet } => {
                        let out_style = if status >= 400 { LogStyle::OutError } else { LogStyle::In };
                        tunnel.test_result = Some(EndpointTestResult {
                            status,
                            latency_ms,
                            body_snippet: body_snippet.clone(),
                        });
                        tunnel.push_log(LogLine {
                            timestamp: now,
                            text: format!("TEST {status}  {latency_ms}ms  {body_snippet}"),
                            style: out_style,
                        });
                    }
                }
            }
        }
    }

    pub fn save_settings(&self) -> Result<(), std::io::Error> {
        self.settings.save(&self.settings_path)
    }

    pub fn switch_profile(&mut self, idx: usize) {
        self.settings.switch_active(idx);
    }
}

impl TunnelHandle {
    fn push_log(&mut self, line: LogLine) {
        if self.logs.len() >= MAX_LOG_LINES {
            self.logs.pop_front();
        }
        self.logs.push_back(line);
    }
}

fn chrono_now() -> String {
    let now = std::time::SystemTime::now();
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}
