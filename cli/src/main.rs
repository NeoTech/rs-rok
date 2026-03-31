mod cli;
mod config;
mod proxy;
mod tunnel;

use clap::Parser;
use cli::{Cli, Command, ConfigAction};
use config::Settings;
use rs_rok_protocol::TunnelType;
use tracing::error;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Init tracing
    let env_filter = tracing_subscriber::EnvFilter::try_new(&cli.log_level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let config_path = Settings::config_path(cli.config_path.as_deref());

    match cli.command {
        Command::Config { action } => {
            let mut settings = Settings::load(&config_path);
            match action {
                ConfigAction::AddToken { token } => {
                    settings.auth_token = Some(token);
                    if let Err(e) = settings.save(&config_path) {
                        error!("failed to save config: {e}");
                        std::process::exit(1);
                    }
                    println!("Token saved to {}", config_path.display());
                }
                ConfigAction::Show => {
                    let json = serde_json::to_string_pretty(&settings)
                        .expect("failed to serialize settings");
                    println!("{json}");
                }
                ConfigAction::SetEndpoint { url } => {
                    settings.endpoint = url;
                    if let Err(e) = settings.save(&config_path) {
                        error!("failed to save config: {e}");
                        std::process::exit(1);
                    }
                    println!("Endpoint saved to {}", config_path.display());
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
    }
}

async fn start_tunnel(
    tunnel_type: TunnelType,
    port: u16,
    host: &str,
    name: Option<String>,
    config_path: &std::path::Path,
) {
    let settings = Settings::load(config_path);
    let auth_token = settings.auth_token.unwrap_or_default();
    let local_addr = format!("{host}:{port}");

    let tunnel_config = tunnel::TunnelConfig {
        endpoint: settings.endpoint,
        auth_token,
        tunnel_type,
        local_addr,
        name,
    };

    if let Err(e) = tunnel::run(tunnel_config).await {
        error!("tunnel error: {e}");
        std::process::exit(1);
    }
}
