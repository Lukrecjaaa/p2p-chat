pub mod args;
mod client;
mod mailbox;
mod setup;

pub use args::AppArgs;

use anyhow::Result;

pub async fn launch() -> Result<()> {
    launch_with_args(AppArgs::from_cli()).await
}

pub async fn launch_with_args(args: AppArgs) -> Result<()> {
    let setup::PreparedApp {
        args,
        port,
        identity,
        db,
        encryption,
    } = setup::prepare(args)?;

    if args.mailbox {
        mailbox::run(identity, db, encryption, port).await
    } else {
        client::run(identity, db, encryption, port).await
    }
}
