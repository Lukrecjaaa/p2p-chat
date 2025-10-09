use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "p2p-messenger")]
#[command(about = "A P2P E2E encrypted messenger")]
pub struct AppArgs {
    #[arg(long, help = "Run in mailbox node mode")]
    pub mailbox: bool,

    #[arg(long, help = "Port to listen on (random free port if not specified)")]
    pub port: Option<u16>,

    #[arg(long, help = "Config file path")]
    pub config: Option<String>,

    #[arg(long, default_value = "data", help = "Data directory")]
    pub data_dir: String,

    #[arg(long, help = "Enable storage encryption")]
    pub encrypt: bool,

    #[arg(
        long = "encryption-password",
        help = "Password used for storage encryption (or set P2P_MESSENGER_PASSWORD)"
    )]
    pub encryption_password: Option<String>,
}

impl AppArgs {
    pub fn from_cli() -> Self {
        <Self as Parser>::parse()
    }
}
