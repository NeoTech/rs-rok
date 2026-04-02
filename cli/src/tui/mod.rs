mod app;
pub mod events;
mod ui;

mod panels {
    pub mod log_view;
    pub mod tunnel_list;
}

mod overlays {
    pub mod deploy;
    pub mod endpoint_test;
    pub mod new_tunnel;
    pub mod profiles;
    pub mod settings;
    pub mod tunnel_manager;
}

use std::path::PathBuf;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use crate::config::Settings;
use app::App;
use events::Action;

/// Run the interactive TUI. Called when `rs-rok` is invoked with no subcommand in a TTY.
pub async fn run(settings_path: PathBuf, profile: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut settings = Settings::load(&settings_path);
    if let Some(ref name) = profile {
        if !settings.switch_active_by_name(name) {
            return Err(format!("profile '{}' not found", name).into());
        }
    }
    let mut app = App::new(settings, settings_path.clone());

    // Restore tunnels from the previous session.
    let saved = crate::saved_tunnels::load(&app.saved_tunnels_path);
    for entry in saved {
        if let Some(config) = entry.to_tunnel_config(&app.settings) {
            let profile_name = Some(entry.profile.clone());
            if entry.state == crate::saved_tunnels::SavedTunnelState::Running {
                app.spawn_tunnel(config, profile_name);
            } else {
                app.add_stopped_tunnel(config, profile_name);
            }
        }
    }

    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Install panic hook that restores terminal before printing panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(info);
    }));

    let result = run_loop(&mut terminal, &mut app).await;

    // Restore terminal
    terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Persist tunnel states before stopping so state=running/stopped is recorded correctly.
    app.persist_tunnels();

    // Stop all running tunnels
    app.stop_all_tunnels();

    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    // Debounce: track last handled key + timestamp to filter Windows ghost repeats
    let mut last_key: Option<(KeyCode, std::time::Instant)> = None;
    let debounce = std::time::Duration::from_millis(30);

    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        // Poll for crossterm events with a short timeout so we can also check tunnel events
        let timeout = std::time::Duration::from_millis(50);

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events (ignore Release/Repeat)
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Debounce: skip if same key arrived within 30ms (Windows ghost repeat)
                let now = std::time::Instant::now();
                if let Some((prev_code, prev_time)) = last_key {
                    if prev_code == key.code && now.duration_since(prev_time) < debounce {
                        continue;
                    }
                }
                last_key = Some((key.code, now));

                // Ctrl+C always quits
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.code == KeyCode::Char('c')
                {
                    return Ok(());
                }

                match events::handle_key(app, key) {
                    Action::None => {}
                    Action::Quit => return Ok(()),
                    Action::OpenOverlay(overlay) => app.overlay = Some(overlay),
                    Action::CloseOverlay => app.overlay = None,
                    Action::SpawnTunnel(config, profile_name) => {
                        app.spawn_tunnel(config, Some(profile_name));
                        app.overlay = None;
                    }
                    Action::StopTunnel(idx) => app.stop_tunnel(idx),
                    Action::StartTunnel(idx) => app.restart_tunnel(idx),
                    Action::DeleteTunnel(idx) => app.delete_tunnel(idx),
                    Action::SaveSettings => {
                        let _ = app.save_settings();
                    }
                    Action::SaveCfAccounts => {
                        if let Some(form) = &app.settings_form {
                            let cf_path = crate::cloudflare_config::CloudflareConfig::config_path();
                            let cfg = crate::cloudflare_config::CloudflareConfig {
                                accounts: form.cf_accounts.clone(),
                            };
                            let _ = cfg.save(&cf_path);
                        }
                    }
                    Action::SwitchProfile(idx) => {
                        app.switch_profile(idx);
                        let _ = app.save_settings();
                        app.overlay = None;
                    }
                    Action::OpenEndpointTest(idx) => {
                        if let Some(t) = app.tunnels.get_mut(idx) {
                            t.test_result = None;
                        }
                        app.endpoint_test_form = Some(crate::tui::app::EndpointTestForm::new());
                        app.overlay = Some(app::Overlay::EndpointTest);
                    }
                    Action::DeployWorker {
                        worker_name,
                        account_id,
                        api_token,
                        auth_token,
                    } => {
                        let cf_account = crate::cloudflare_config::CfAccount {
                            name: String::new(),
                            account_id,
                            api_token,
                        };

                        let (result_tx, result_rx) = tokio::sync::mpsc::unbounded_channel();
                        app.deploy_result_rx = Some(result_rx);
                        // Store the CF account so the result handler can persist it.
                        app.deploy_cf_account = Some(cf_account.clone());

                        // Show deploying status in the overlay (keep it open)
                        if let Some(form) = &mut app.deploy_form {
                            form.status_message = Some("Deploying...".into());
                        }

                        let name = worker_name.clone();
                        tokio::spawn(async move {
                            let outcome = crate::deploy::deploy_worker(&cf_account, &name, auth_token.as_deref()).await;
                            let msg = outcome.map_err(|e| e.to_string());
                            let _ = result_tx.send(msg);
                        });
                    }
                    Action::RunEndpointTest { tunnel_idx, method, path, body, headers } => {
                        if let Some(tunnel) = app.tunnels.get(tunnel_idx) {
                            if let Some(base_url) = tunnel.public_url.clone() {
                                let test_tx = tunnel.test_tx.clone();
                                tokio::spawn(async move {
                                    use reqwest::header::{HeaderName, HeaderValue};

                                    let client = reqwest::Client::builder()
                                        .timeout(std::time::Duration::from_secs(5))
                                        .redirect(reqwest::redirect::Policy::none())
                                        .build()
                                        .unwrap_or_else(|_| reqwest::Client::new());

                                    let sep = if path.starts_with('/') { "" } else { "/" };
                                    let full_url = format!("{}{sep}{path}", base_url.trim_end_matches('/'));

                                    let method_parsed = reqwest::Method::from_bytes(method.as_bytes())
                                        .unwrap_or(reqwest::Method::GET);
                                    let mut req = client.request(method_parsed, &full_url);

                                    for (k, v) in &headers {
                                        if let (Ok(name), Ok(val)) = (
                                            HeaderName::from_bytes(k.as_bytes()),
                                            HeaderValue::from_str(v),
                                        ) {
                                            req = req.header(name, val);
                                        }
                                    }

                                    if !body.is_empty() {
                                        req = req.body(body);
                                    }

                                    let start = std::time::Instant::now();
                                    match req.send().await {
                                        Ok(resp) => {
                                            let status = resp.status().as_u16();
                                            let latency_ms = start.elapsed().as_millis() as u64;
                                            let bytes = resp.bytes().await.unwrap_or_default();
                                            let snippet_end = bytes.len().min(400);
                                            let body_snippet = String::from_utf8_lossy(&bytes[..snippet_end]).to_string();
                                            let _ = test_tx.send(crate::tui::events::TunnelEvent::TestResult {
                                                status,
                                                latency_ms,
                                                body_snippet,
                                            });
                                        }
                                        Err(e) => {
                                            let _ = test_tx.send(crate::tui::events::TunnelEvent::Error(
                                                format!("Test request failed: {e}"),
                                            ));
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
            }
        }

        // Drain tunnel events
        app.poll_tunnel_events();

        // Check for deploy result
        if let Some(rx) = &mut app.deploy_result_rx {
            match rx.try_recv() {
                Ok(Ok(url)) => {
                    // Success — apply result to in-memory settings via shared method,
                    // save through the canonical app.save_settings() path, and
                    // also persist the CF account that was used (TUI previously skipped this).
                    let auth_token = app.deploy_form.as_ref().and_then(|f| {
                        if f.auth_token.is_empty() { None } else { Some(f.auth_token.clone()) }
                    });
                    app.settings.apply_deploy_result(&url, auth_token.as_deref());
                    let _ = app.save_settings();

                    // Persist CF credentials used for this deploy.
                    if let Some(cf_account) = app.deploy_cf_account.take() {
                        let cf_path = crate::cloudflare_config::CloudflareConfig::config_path();
                        let mut cf_cfg = crate::cloudflare_config::CloudflareConfig::load(&cf_path);
                        cf_cfg.upsert_account(cf_account);
                        let _ = cf_cfg.save(&cf_path);
                    }

                    app.deploy_result_rx = None;
                    app.overlay = None;
                }
                Ok(Err(msg)) => {
                    // Failure — show error in overlay
                    if let Some(form) = &mut app.deploy_form {
                        form.status_message = Some(format!("Deploy failed: {msg}"));
                    }
                    app.deploy_result_rx = None;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    app.deploy_result_rx = None;
                }
            }
        }

        // If endpoint test overlay is open and a result just arrived, clear is_running
        if matches!(app.overlay, Some(app::Overlay::EndpointTest)) {
            let idx = app.selected_tunnel;
            let has_result = app.tunnels.get(idx).map_or(false, |t| t.test_result.is_some());
            if has_result {
                if let Some(form) = &mut app.endpoint_test_form {
                    form.is_running = false;
                }
            }
        }
    }
}
