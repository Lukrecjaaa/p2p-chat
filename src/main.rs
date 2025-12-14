//! The main entry point for the p2p-chat application.
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
mod web;

use anyhow::Result;

/// The main function of the application.
///
/// This function is the entry point for the p2p-chat application. It
/// initializes the application and launches it in either client or mailbox
/// mode based on command-line arguments.
///
/// # Errors
///
/// Returns an error if the application fails to launch or encounters
/// a critical error during execution.
#[tokio::main]
async fn main() -> Result<()> {
    app::launch().await
}
