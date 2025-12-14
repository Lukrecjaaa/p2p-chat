//! This module contains the primary logic for the application.
//!
//! It is responsible for parsing command-line arguments, setting up the
//! application environment, and launching either a client or a mailbox node.
pub mod args;
mod client;
mod mailbox;
mod setup;

pub use args::AppArgs;

use anyhow::Result;

/// Launches the application.
///
/// This function parses command-line arguments and then calls `launch_with_args`.
///
/// # Errors
///
/// This function will return an error if the application fails to launch.
pub async fn launch() -> Result<()> {
    launch_with_args(AppArgs::from_cli()).await
}

/// Launches the application with the given arguments.
///
/// This function prepares the application environment and then runs either a
/// client or a mailbox node, depending on the provided arguments.
///
/// # Arguments
///
/// * `args` - The command-line arguments to use.
///
/// # Errors
///
/// This function will return an error if the application fails to launch.
pub async fn launch_with_args(args: AppArgs) -> Result<()> {
    let setup::PreparedApp {
        args,
        port,
        web_port,
        identity,
        db,
        encryption,
    } = setup::prepare(args)?;

    if args.mailbox {
        mailbox::run(identity, db, encryption, port).await
    } else {
        client::run(identity, db, encryption, port, web_port).await
    }
}
