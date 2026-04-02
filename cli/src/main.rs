mod cli;
mod cloudflare_config;
mod config;
mod deploy;
mod proxy;
mod tcp_client;
mod tui;
mod tunnel;
mod worker_bundle;

use clap::Parser;
use cli::{Cli, Command, ConfigAction};
use cloudflare_config::CloudflareConfig;
use config::Settings;
use rs_rok_protocol::TunnelType;
use std::io::IsTerminal;
use tracing::error;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let config_path = Settings::config_path(cli.config_path.as_deref());

    // No subcommand + interactive TTY -> launch TUI
    if cli.command.is_none() && std::io::stdout().is_terminal() {
        if let Err(e) = tui::run(config_path, cli.profile).await {
            eprintln!("TUI error: {e}");
            std::process::exit(1);
        }
        return;
    }

    // Init tracing (only for CLI mode — TUI handles its own output)
    let env_filter = tracing_subscriber::EnvFilter::try_new(&cli.log_level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let Some(command) = cli.command else {
        // Not a TTY and no subcommand — print help
        use clap::CommandFactory;
        Cli::command().print_help().ok();
        std::process::exit(0);
    };

    match command {
        Command::Config { action } => {
            let mut settings = Settings::load(&config_path);
            if let Some(ref name) = cli.profile {
                if !settings.switch_active_by_name(name) {
                    error!("profile '{}' not found", name);
                    std::process::exit(1);
                }
            }
            match action {
                ConfigAction::AddToken { token } => {
                    settings.active_profile_mut().auth_token = Some(token);
                    if let Err(e) = settings.save(&config_path) {
                        error!("failed to save config: {e}");
                        std::process::exit(1);
                    }
                    println!("Token saved to {}", config_path.display());
                }
                ConfigAction::Show => {
                    let json = serde_json::to_string_pretty(&settings.profiles)
                        .expect("failed to serialize settings");
                    println!("{json}");
                }
                ConfigAction::SetEndpoint { url } => {
                    settings.active_profile_mut().endpoint = url;
                    if let Err(e) = settings.save(&config_path) {
                        error!("failed to save config: {e}");
                        std::process::exit(1);
                    }
                    println!("Endpoint saved to {}", config_path.display());
                }
                ConfigAction::SetCfCredentials {
                    account_id,
                    api_token,
                } => {
                    let cf_path = CloudflareConfig::config_path();
                    let mut cfg = CloudflareConfig::load(&cf_path);
                    cfg.accounts.push(crate::cloudflare_config::CfAccount {
                        name: String::new(),
                        account_id,
                        api_token,
                    });
                    if let Err(e) = cfg.save(&cf_path) {
                        error!("failed to save cloudflare config: {e}");
                        std::process::exit(1);
                    }
                    println!("Cloudflare credentials saved to {}", cf_path.display());
                }
            }
        }
        Command::Http {
            port, host, name,
        } => {
            start_tunnel(TunnelType::Http, port, &host, name, &config_path).await;
        }
        Command::Https {
            port, host, name,
        } => {
            start_tunnel(TunnelType::Https, port, &host, name, &config_path).await;
        }
        Command::Deploy {
            account_id,
            api_token,
            name,
        } => {
            deploy_worker(account_id, api_token, &name, &config_path).await;
        }
        Command::Tcp {
            port, host, name,
        } => {
            start_tcp_tunnel(port, &host, name, &config_path).await;
        }
        Command::Connect {
            slug, token, port, host,
        } => {
            start_tcp_client(&slug, &token, port, &host, &config_path).await;
        }
    }
}

/// If --profile was passed, switch to that profile (or exit on unknown name).
fn apply_profile_flag(settings: &mut Settings, profile_name: Option<&str>) {
    if let Some(name) = profile_name {
        if !settings.switch_active_by_name(name) {
            error!("profile '{}' not found", name);
            std::process::exit(1);
        }
    }
}

async fn start_tunnel(
    tunnel_type: TunnelType,
    port: u16,
    host: &str,
    name: Option<String>,
    config_path: &std::path::Path,
) {
    let mut settings = Settings::load(config_path);
    apply_profile_flag(&mut settings, None);
    let profile = settings.active_profile();
    let auth_token = profile.auth_token.clone().unwrap_or_default();
    let local_addr = format!("{host}:{port}");

    let tunnel_config = tunnel::TunnelConfig {
        endpoint: profile.endpoint.clone(),
        auth_token,
        tunnel_type,
        local_addr,
        name,
        tcp_token: None,
        events_tx: None,
    };

    if let Err(e) = tunnel::run(tunnel_config).await {
        error!("tunnel error: {e}");
        std::process::exit(1);
    }
}

async fn start_tcp_tunnel(
    port: u16,
    host: &str,
    name: Option<String>,
    config_path: &std::path::Path,
) {
    use rand::Rng;

    let mut settings = Settings::load(config_path);
    apply_profile_flag(&mut settings, None);
    let profile = settings.active_profile();
    let auth_token = profile.auth_token.clone().unwrap_or_default();
    let local_addr = format!("{host}:{port}");

    // Generate a random 32-char hex token
    let mut rng = rand::thread_rng();
    let token_bytes: [u8; 16] = rng.gen();
    let tcp_token: String = token_bytes.iter().map(|b| format!("{b:02x}")).collect();

    let slug = name.as_deref().unwrap_or("__root__");
    println!();
    println!("TCP tunnel token: {tcp_token}");
    println!("Connect with:     rs-rok connect {slug} --token {tcp_token} --port <local-port>");
    println!();

    let tunnel_config = tunnel::TunnelConfig {
        endpoint: profile.endpoint.clone(),
        auth_token,
        tunnel_type: TunnelType::Tcp,
        local_addr,
        name,
        tcp_token: Some(tcp_token),
        events_tx: None,
    };

    if let Err(e) = tunnel::run(tunnel_config).await {
        error!("tunnel error: {e}");
        std::process::exit(1);
    }
}

async fn start_tcp_client(
    slug: &str,
    token: &str,
    port: u16,
    host: &str,
    config_path: &std::path::Path,
) {
    let mut settings = Settings::load(config_path);
    apply_profile_flag(&mut settings, None);

    let client_config = tcp_client::TcpClientConfig {
        endpoint: settings.active_profile().endpoint.clone(),
        slug: slug.to_string(),
        token: token.to_string(),
        local_addr: format!("{host}:{port}"),
    };

    if let Err(e) = tcp_client::run(client_config).await {
        error!("connect error: {e}");
        std::process::exit(1);
    }
}

async fn deploy_worker(
    account_id: Option<String>,
    api_token: Option<String>,
    worker_name: &str,
    config_path: &std::path::Path,
) {
    let cf_path = CloudflareConfig::config_path();
    let cf_cfg = CloudflareConfig::load(&cf_path);
    let mut cf = cf_cfg.first().cloned().unwrap_or(crate::cloudflare_config::CfAccount {
        name: String::new(),
        account_id: String::new(),
        api_token: String::new(),
    });

    // CLI flags override stored/env config
    if let Some(id) = account_id {
        cf.account_id = id;
    }
    if let Some(tok) = api_token {
        cf.api_token = tok;
    }

    if cf.account_id.is_empty() {
        error!("Cloudflare Account ID required. Provide --account-id or set CF_ACCOUNT_ID, or run: rs-rok config set-cf-credentials");
        std::process::exit(1);
    }
    if cf.api_token.is_empty() {
        error!("Cloudflare API Token required. Provide --api-token or set CF_API_TOKEN, or run: rs-rok config set-cf-credentials");
        std::process::exit(1);
    }

    println!("Deploying worker '{worker_name}'...");

    match deploy::deploy_worker(&cf, worker_name).await {
        Ok(url) => {
            println!("Worker deployed successfully!");
            println!("URL: {url}");

            // Save credentials for future deploys
            let save_cfg = CloudflareConfig { accounts: vec![cf.clone()] };
            if let Err(e) = save_cfg.save(&cf_path) {
                error!("warning: could not save credentials: {e}");
            }

            // Update endpoint in active profile
            let mut settings = Settings::load(config_path);
            settings.active_profile_mut().endpoint = url.clone();
            if let Err(e) = settings.save(config_path) {
                error!("warning: could not update endpoint in settings: {e}");
            } else {
                println!("Endpoint updated to {url}");
            }
        }
        Err(e) => {
            error!("deploy failed: {e}");
            std::process::exit(1);
        }
    }
}
