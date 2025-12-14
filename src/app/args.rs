//! This module defines the command-line arguments for the application.
use clap::Parser;

/// Defines the command-line arguments for the application.
///
/// This struct is used by the `clap` crate to parse command-line arguments.
#[derive(Parser, Debug, Clone)]
#[command(name = "p2p-messenger")]
#[command(about = "A P2P E2E encrypted messenger")]
pub struct AppArgs {
    /// If set, the application will run in mailbox node mode.
    #[arg(long, help = "Run in mailbox node mode")]
    pub mailbox: bool,

    /// The port to listen on.
    /// If not specified, a random free port will be used.
    #[arg(long, help = "Port to listen on (random free port if not specified)")]
    pub port: Option<u16>,

    /// The directory where data will be stored.
    #[arg(long, default_value = "data", help = "Data directory")]
    pub data_dir: String,

    /// If set, storage encryption will be enabled.
    #[arg(long, help = "Enable storage encryption")]
    pub encrypt: bool,

    /// The password to use for storage encryption.
    /// This can also be set using the `P2P_MESSENGER_PASSWORD` environment variable.
    #[arg(
        long = "encryption-password",
        help = "Password used for storage encryption (or set P2P_MESSENGER_PASSWORD)"
    )]
    pub encryption_password: Option<String>,

    /// The port for the Web UI.
    /// If not specified, a random free port will be used.
    #[arg(long, help = "Web UI port (random free port if not specified)")]
    pub web_port: Option<u16>,
}

impl AppArgs {
    /// Parses command-line arguments from the environment.
    ///
    /// This function uses the `clap` crate to parse the command-line arguments
    /// provided to the application and returns a new `AppArgs` instance.
    pub fn from_cli() -> Self {
        <Self as Parser>::parse()
    }
}
