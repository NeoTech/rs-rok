use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "rs-rok", version, about = "Expose local services to the internet")]
pub struct Cli {
    /// Path to config file (default: ~/.rs-rok/settings.json)
    #[arg(long = "config", global = true)]
    pub config_path: Option<String>,

    /// Log level: trace, debug, info, warn, error
    #[arg(long = "log", global = true, default_value = "info")]
    pub log_level: String,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Expose a local HTTP service
    Http {
        /// Local port to forward traffic to
        port: u16,

        /// Stable tunnel name (e.g. --name myapp → /tunnel/myapp)
        #[arg(long)]
        name: Option<String>,

        /// Local hostname to forward to
        #[arg(long, default_value = "localhost")]
        host: String,
    },

    /// Expose a local HTTPS service (TLS terminated at edge)
    Https {
        /// Local port to forward traffic to
        port: u16,

        /// Stable tunnel name (e.g. --name myapp → /tunnel/myapp)
        #[arg(long)]
        name: Option<String>,

        /// Local hostname to forward to
        #[arg(long, default_value = "localhost")]
        host: String,
    },

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Store an authentication token
    AddToken {
        /// The auth token to store
        token: String,
    },

    /// Print current configuration
    Show,

    /// Set the worker endpoint URL
    SetEndpoint {
        /// The worker endpoint URL
        url: String,
    },
}
