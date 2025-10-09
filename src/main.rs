mod app;
mod cli;
mod crypto;
mod logging;
mod mailbox;
mod net;
mod network;
mod storage;
mod sync;
mod types;
mod ui;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::launch().await
}
