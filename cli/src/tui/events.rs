use crossterm::event::{KeyCode, KeyEvent};

use super::app::{App, DeployForm, Focus, NewTunnelForm, Overlay, SettingsForm, TunnelStatus, TunnelTypeOption};
use crate::config::Profile;
use crate::tunnel::TunnelConfig;
use rs_rok_protocol::TunnelType;

#[derive(Debug, Clone)]
#[allow(dead_code)] // Variants will be constructed once tunnel.rs sends events
pub enum TunnelEvent {
    Connected { url: String },
    Request { method: String, path: String, status: u16, latency_ms: u64 },
    Disconnected { reason: String },
    Error(String),
    TestResult { status: u16, latency_ms: u64, body_snippet: String },
}

pub enum Action {
    None,
    Quit,
    OpenOverlay(Overlay),
    CloseOverlay,
    SpawnTunnel(TunnelConfig, String),
    StopTunnel(usize),
    StartTunnel(usize),
    DeleteTunnel(usize),
    SaveSettings,
    SaveCfAccounts,
    SwitchProfile(usize),
    DeployWorker {
        worker_name: String,
        account_id: String,
        api_token: String,
        auth_token: Option<String>,
    },
    OpenEndpointTest(usize),
    RunEndpointTest {
        tunnel_idx: usize,
        method: String,
        path: String,
        body: String,
        headers: Vec<(String, String)>,
    },
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> Action {
    // If an overlay is open, dispatch to overlay handler
    if let Some(overlay) = app.overlay {
        return match overlay {
            Overlay::NewTunnel => handle_new_tunnel_key(app, key),
            Overlay::Settings => handle_settings_key(app, key),
            Overlay::Profiles => handle_profiles_key(app, key),
            Overlay::Deploy => handle_deploy_key(app, key),
            Overlay::EndpointTest => handle_endpoint_test_key(app, key),
            Overlay::TunnelManager => handle_tunnel_manager_key(app, key),
        };
    }

    // Main view key handling
    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('n') => {
            app.new_tunnel_form = NewTunnelForm::new(app.settings.active_idx);
            Action::OpenOverlay(Overlay::NewTunnel)
        }
        KeyCode::Char('s') => {
            app.settings_form = Some(SettingsForm::new(&app.settings.profiles, app.settings.active_idx));
            Action::OpenOverlay(Overlay::Settings)
        }
        KeyCode::Char('p') => {
            app.profiles_selected = app.settings.active_idx;
            app.new_profile_input = None;
            Action::OpenOverlay(Overlay::Profiles)
        }
        KeyCode::Char('m') => {
            app.tunnel_manager_selected = app.selected_tunnel.min(app.tunnels.len().saturating_sub(1));
            Action::OpenOverlay(Overlay::TunnelManager)
        }
        KeyCode::Char('d') => {
            if !app.tunnels.is_empty() {
                Action::StopTunnel(app.selected_tunnel)
            } else {
                Action::None
            }
        }
        KeyCode::Char('r') => {
            let idx = app.selected_tunnel;
            if let Some(t) = app.tunnels.get(idx) {
                if t.status == TunnelStatus::Stopped {
                    Action::StartTunnel(idx)
                } else {
                    Action::None
                }
            } else {
                Action::None
            }
        }
        KeyCode::Char('x') => {
            if !app.tunnels.is_empty() {
                Action::DeleteTunnel(app.selected_tunnel)
            } else {
                Action::None
            }
        }
        KeyCode::Char('D') => {
            let auth_token = app.settings.active_profile().auth_token.clone().unwrap_or_default();
            app.deploy_form = Some(DeployForm::new(auth_token));
            Action::OpenOverlay(Overlay::Deploy)
        }
        KeyCode::Char('t') => {
            let idx = app.selected_tunnel;
            if let Some(t) = app.tunnels.get(idx) {
                if t.status == TunnelStatus::Active
                    && t.public_url.is_some()
                    && t.tunnel_type != TunnelType::Tcp
                {
                    Action::OpenEndpointTest(idx)
                } else {
                    Action::None
                }
            } else {
                Action::None
            }
        }
        KeyCode::Tab => {
            app.focus = match app.focus {
                Focus::List => Focus::Logs,
                Focus::Logs => Focus::List,
            };
            Action::None
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if app.focus == Focus::Logs {
                if let Some(t) = app.tunnels.get_mut(app.selected_tunnel) {
                    t.auto_scroll = false;
                    t.scroll_offset = t.scroll_offset.saturating_add(1);
                }
            } else {
                app.next_tunnel();
            }
            Action::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.focus == Focus::Logs {
                if let Some(t) = app.tunnels.get_mut(app.selected_tunnel) {
                    t.auto_scroll = false;
                    t.scroll_offset = t.scroll_offset.saturating_sub(1);
                }
            } else {
                app.prev_tunnel();
            }
            Action::None
        }
        KeyCode::Char('g') => {
            if let Some(t) = app.tunnels.get_mut(app.selected_tunnel) {
                t.scroll_offset = 0;
                t.auto_scroll = false;
            }
            Action::None
        }
        KeyCode::Char('G') => {
            if let Some(t) = app.tunnels.get_mut(app.selected_tunnel) {
                t.scroll_offset = t.logs.len().saturating_sub(1) as u16;
                t.auto_scroll = true;
            }
            Action::None
        }
        KeyCode::PageUp => {
            if let Some(t) = app.tunnels.get_mut(app.selected_tunnel) {
                t.auto_scroll = false;
                t.scroll_offset = t.scroll_offset.saturating_sub(20);
            }
            Action::None
        }
        KeyCode::PageDown => {
            if let Some(t) = app.tunnels.get_mut(app.selected_tunnel) {
                t.auto_scroll = false;
                t.scroll_offset = t.scroll_offset.saturating_add(20);
            }
            Action::None
        }
        _ => Action::None,
    }
}

// -- New Tunnel overlay --

fn handle_new_tunnel_key(app: &mut App, key: KeyEvent) -> Action {
    let form = &mut app.new_tunnel_form;
    let profile_count = app.settings.profiles.len();

    match key.code {
        KeyCode::Esc => Action::CloseOverlay,
        KeyCode::Down => {
            form.focus_next();
            Action::None
        }
        KeyCode::Up => {
            form.focus_prev();
            Action::None
        }
        KeyCode::Left => {
            match form.focused_field {
                0 => {
                    // Cycle profile backward
                    form.profile_idx = (form.profile_idx + profile_count - 1) % profile_count;
                }
                1 => {
                    form.tunnel_type = form.tunnel_type.prev();
                }
                _ => {}
            }
            Action::None
        }
        KeyCode::Right => {
            match form.focused_field {
                0 => {
                    // Cycle profile forward
                    form.profile_idx = (form.profile_idx + 1) % profile_count;
                }
                1 => {
                    form.tunnel_type = form.tunnel_type.next();
                }
                _ => {}
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match form.focused_field {
                0 | 1 => { /* selector fields: ignore char input, use arrows */ }
                2 => {
                    if c.is_ascii_digit() {
                        form.port.push(c);
                    }
                }
                3 => {
                    // Disallow '/' — it breaks the URL slug
                    if c != '/' {
                        form.name.push(c);
                    }
                }
                4 => form.host.push(c),
                _ => {}
            }
            Action::None
        }
        KeyCode::Backspace => {
            match form.focused_field {
                2 => { form.port.pop(); }
                3 => { form.name.pop(); }
                4 => { form.host.pop(); }
                _ => {}
            }
            Action::None
        }
        KeyCode::Enter => {
            // Validate port
            let port: u16 = match form.port.parse() {
                Ok(p) if p > 0 => p,
                _ => return Action::None, // Invalid port, stay on form
            };

            let tunnel_type = match form.tunnel_type {
                TunnelTypeOption::Http => TunnelType::Http,
                TunnelTypeOption::Https => TunnelType::Https,
                TunnelTypeOption::Tcp => TunnelType::Tcp,
            };

            let host = if form.host.is_empty() {
                "localhost".to_string()
            } else {
                form.host.clone()
            };

            let name = if form.name.is_empty() {
                None
            } else {
                Some(form.name.clone())
            };

            // Use the selected profile, not just the active one
            let profile = &app.settings.profiles[form.profile_idx];

            let tcp_token = if tunnel_type == TunnelType::Tcp {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let bytes: [u8; 16] = rng.gen();
                Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
            } else {
                None
            };

            let config = TunnelConfig {
                endpoint: profile.endpoint.clone(),
                auth_token: profile.auth_token.clone().unwrap_or_default(),
                tunnel_type,
                local_addr: format!("{host}:{port}"),
                name,
                tcp_token,
                events_tx: None, // Will be set by spawn_tunnel
            };

            Action::SpawnTunnel(config, profile.name.clone())
        }
        _ => Action::None,
    }
}

// -- Settings overlay --

fn handle_settings_key(app: &mut App, key: KeyEvent) -> Action {
    let Some(form) = &mut app.settings_form else {
        return Action::CloseOverlay;
    };

    match key.code {
        KeyCode::Esc => {
            if form.editing {
                form.editing = false;
                Action::None
            } else {
                app.settings_form = None;
                Action::CloseOverlay
            }
        }
        KeyCode::Down if !form.editing => {
            form.focus_next();
            Action::None
        }
        KeyCode::Up if !form.editing => {
            form.focus_prev();
            Action::None
        }
        KeyCode::Left if !form.editing => {
            if form.on_profile_selector() {
                let count = app.settings.profiles.len();
                form.profile_idx = (form.profile_idx + count - 1) % count;
                form.load_profile(&app.settings.profiles[form.profile_idx]);
                app.settings.switch_active(form.profile_idx);
                return Action::SaveSettings;
            } else if form.on_cf_selector() && !form.cf_accounts.is_empty() {
                let count = form.cf_accounts.len();
                form.cf_account_idx = (form.cf_account_idx + count - 1) % count;
                form.load_cf_account();
            }
            Action::None
        }
        KeyCode::Right if !form.editing => {
            if form.on_profile_selector() {
                let count = app.settings.profiles.len();
                form.profile_idx = (form.profile_idx + 1) % count;
                form.load_profile(&app.settings.profiles[form.profile_idx]);
                app.settings.switch_active(form.profile_idx);
                return Action::SaveSettings;
            } else if form.on_cf_selector() && !form.cf_accounts.is_empty() {
                let count = form.cf_accounts.len();
                form.cf_account_idx = (form.cf_account_idx + 1) % count;
                form.load_cf_account();
            }
            Action::None
        }
        KeyCode::Enter => {
            if form.editing {
                form.editing = false;
                if form.on_profile_field() {
                    let form_snapshot = form.clone();
                    let profile = &mut app.settings.profiles[form_snapshot.profile_idx];
                    form_snapshot.apply_to_profile(profile);
                    return Action::SaveSettings;
                } else if form.on_cf_field() {
                    // Apply CF field edits back into the accounts vec
                    if let Some(updated) = form.apply_to_cf_account() {
                        if form.cf_account_idx < form.cf_accounts.len() {
                            form.cf_accounts[form.cf_account_idx] = updated;
                        } else {
                            form.cf_accounts.push(updated);
                            form.cf_account_idx = form.cf_accounts.len() - 1;
                        }
                    }
                    return Action::SaveCfAccounts;
                }
                Action::None
            } else if form.on_profile_field() || form.on_cf_field() {
                form.editing = true;
                Action::None
            } else {
                // On a selector row, do nothing on Enter
                Action::None
            }
        }
        KeyCode::Char(c) if form.editing => {
            if form.on_profile_field() {
                let idx = form.profile_field_idx();
                if let Some((_, val)) = form.profile_fields.get_mut(idx) {
                    val.push(c);
                }
            } else if form.on_cf_field() {
                let idx = form.cf_field_idx();
                if let Some((_, val)) = form.cf_fields.get_mut(idx) {
                    val.push(c);
                }
            }
            Action::None
        }
        KeyCode::Backspace if form.editing => {
            if form.on_profile_field() {
                let idx = form.profile_field_idx();
                if let Some((_, val)) = form.profile_fields.get_mut(idx) {
                    val.pop();
                }
            } else if form.on_cf_field() {
                let idx = form.cf_field_idx();
                if let Some((_, val)) = form.cf_fields.get_mut(idx) {
                    val.pop();
                }
            }
            Action::None
        }
        _ => Action::None,
    }
}

// -- Profiles overlay --

fn handle_profiles_key(app: &mut App, key: KeyEvent) -> Action {
    // If we're in new profile input mode
    if let Some(ref mut input) = app.new_profile_input {
        return match key.code {
            KeyCode::Esc => {
                app.new_profile_input = None;
                Action::None
            }
            KeyCode::Char(c) => {
                input.push(c);
                Action::None
            }
            KeyCode::Backspace => {
                input.pop();
                Action::None
            }
            KeyCode::Enter => {
                let name = input.clone();
                app.new_profile_input = None;
                if !name.is_empty()
                    && !app.settings.profiles.iter().any(|p| p.name == name)
                {
                    app.settings.profiles.push(Profile::new(
                        name.clone(),
                        "http://localhost:8787",
                    ));
                    let _ = app.save_settings();
                    app.profiles_selected = app.settings.profiles.len() - 1;
                }
                Action::None
            }
            _ => Action::None,
        };
    }

    match key.code {
        KeyCode::Esc => Action::CloseOverlay,
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.settings.profiles.is_empty() {
                app.profiles_selected =
                    (app.profiles_selected + 1) % app.settings.profiles.len();
            }
            Action::None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if !app.settings.profiles.is_empty() {
                app.profiles_selected = (app.profiles_selected
                    + app.settings.profiles.len()
                    - 1)
                    % app.settings.profiles.len();
            }
            Action::None
        }
        KeyCode::Enter => Action::SwitchProfile(app.profiles_selected),
        KeyCode::Char('n') => {
            app.new_profile_input = Some(String::new());
            Action::None
        }
        KeyCode::Char('d') => {
            // Don't delete the last profile
            if app.settings.profiles.len() > 1 {
                let removed_idx = app.profiles_selected;
                app.settings.profiles.remove(removed_idx);
                if app.profiles_selected >= app.settings.profiles.len() {
                    app.profiles_selected = app.settings.profiles.len() - 1;
                }
                // If we deleted the active profile, switch to first
                if removed_idx == app.settings.active_idx {
                    app.settings.active_idx = 0;
                } else if app.settings.active_idx > removed_idx {
                    app.settings.active_idx -= 1;
                }
                let _ = app.save_settings();
            }
            Action::None
        }
        _ => Action::None,
    }
}

// -- Deploy overlay --

fn handle_deploy_key(app: &mut App, key: KeyEvent) -> Action {
    // Block input while a deploy is in progress (only Esc cancels the view)
    if app.deploy_result_rx.is_some() {
        if key.code == KeyCode::Esc {
            app.deploy_result_rx = None;
            return Action::CloseOverlay;
        }
        return Action::None;
    }

    let Some(form) = &mut app.deploy_form else {
        return Action::CloseOverlay;
    };

    match key.code {
        KeyCode::Esc => Action::CloseOverlay,
        KeyCode::Tab | KeyCode::Down => {
            form.focused_field = (form.focused_field + 1) % 2;
            Action::None
        }
        KeyCode::Up => {
            form.focused_field = (form.focused_field + 1) % 2;
            Action::None
        }
        KeyCode::Left => {
            if form.focused_field == 0 && !form.cf_accounts.is_empty() {
                form.selected_account =
                    (form.selected_account + form.cf_accounts.len() - 1) % form.cf_accounts.len();
            }
            Action::None
        }
        KeyCode::Right => {
            if form.focused_field == 0 && !form.cf_accounts.is_empty() {
                form.selected_account = (form.selected_account + 1) % form.cf_accounts.len();
            }
            Action::None
        }
        KeyCode::Char(c) => {
            if form.focused_field == 0 {
                form.worker_name.push(c);
            } else {
                form.auth_token.push(c);
            }
            Action::None
        }
        KeyCode::Backspace => {
            if form.focused_field == 0 {
                form.worker_name.pop();
            } else {
                form.auth_token.pop();
            }
            Action::None
        }
        KeyCode::Enter => {
            if form.worker_name.is_empty() {
                form.status_message = Some("Worker name is required".into());
                return Action::None;
            }
            let Some(account) = form.active_account().cloned() else {
                form.status_message = Some("No CF account configured -- add to cloudflare.json".into());
                return Action::None;
            };

            let worker_name = form.worker_name.clone();
            let account_id = account.account_id;
            let api_token = account.api_token;
            let auth_token = if form.auth_token.is_empty() {
                None
            } else {
                Some(form.auth_token.clone())
            };

            Action::DeployWorker {
                worker_name,
                account_id,
                api_token,
                auth_token,
            }
        }
        _ => Action::None,
    }
}

// -- Endpoint Test overlay --

fn handle_endpoint_test_key(app: &mut App, key: KeyEvent) -> Action {
    let Some(form) = &mut app.endpoint_test_form else {
        return Action::CloseOverlay;
    };

    // Block input while a request is in flight
    if form.is_running {
        if key.code == KeyCode::Esc {
            app.endpoint_test_form = None;
            return Action::CloseOverlay;
        }
        return Action::None;
    }

    match key.code {
        KeyCode::Esc => {
            app.endpoint_test_form = None;
            Action::CloseOverlay
        }
        KeyCode::Down => {
            form.focus_next();
            Action::None
        }
        KeyCode::Up => {
            form.focus_prev();
            Action::None
        }
        KeyCode::Left if form.focused_field == 0 => {
            let count = crate::tui::app::HTTP_METHODS.len();
            form.http_method_idx = (form.http_method_idx + count - 1) % count;
            Action::None
        }
        KeyCode::Right if form.focused_field == 0 => {
            let count = crate::tui::app::HTTP_METHODS.len();
            form.http_method_idx = (form.http_method_idx + 1) % count;
            Action::None
        }
        KeyCode::Char(c) if form.focused_field != 0 => {
            if let Some((h_idx, is_val)) = form.focused_header() {
                if let Some(pair) = form.headers.get_mut(h_idx) {
                    if is_val { pair.1.push(c); } else { pair.0.push(c); }
                }
            } else {
                match form.focused_field {
                    1 => form.path.push(c),
                    2 => form.body.push(c),
                    _ => {}
                }
            }
            Action::None
        }
        KeyCode::Backspace if form.focused_field != 0 => {
            if let Some((h_idx, is_val)) = form.focused_header() {
                if let Some(pair) = form.headers.get_mut(h_idx) {
                    if is_val { pair.1.pop(); } else { pair.0.pop(); }
                }
            } else {
                match form.focused_field {
                    1 => { form.path.pop(); }
                    2 => { form.body.pop(); }
                    _ => {}
                }
            }
            Action::None
        }
        KeyCode::Enter => {
            let tunnel_idx = app.selected_tunnel;
            let method = form.method().to_string();
            let path = form.path.clone();
            let body = form.body.clone();
            let headers: Vec<(String, String)> = form
                .headers
                .iter()
                .filter(|(k, _)| !k.is_empty())
                .cloned()
                .collect();
            form.is_running = true;
            Action::RunEndpointTest { tunnel_idx, method, path, body, headers }
        }
        _ => Action::None,
    }
}
// -- Tunnel Manager overlay --

fn handle_tunnel_manager_key(app: &mut App, key: KeyEvent) -> Action {
    if app.tunnels.is_empty() {
        if key.code == KeyCode::Esc {
            return Action::CloseOverlay;
        }
        return Action::None;
    }

    match key.code {
        KeyCode::Esc => Action::CloseOverlay,
        KeyCode::Up | KeyCode::Char('k') => {
            if app.tunnel_manager_selected > 0 {
                app.tunnel_manager_selected -= 1;
            }
            Action::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.tunnel_manager_selected + 1 < app.tunnels.len() {
                app.tunnel_manager_selected += 1;
            }
            Action::None
        }
        KeyCode::Char('x') | KeyCode::Delete => {
            let idx = app.tunnel_manager_selected;
            Action::DeleteTunnel(idx)
        }
        KeyCode::Char('r') => {
            let idx = app.tunnel_manager_selected;
            if let Some(t) = app.tunnels.get(idx) {
                if t.status == TunnelStatus::Stopped {
                    return Action::StartTunnel(idx);
                }
            }
            Action::None
        }
        _ => Action::None,
    }
}