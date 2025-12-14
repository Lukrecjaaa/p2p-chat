//! This module contains logic for directly retrying messages in the outbox
//! for a specific peer.
use anyhow::Result;
use libp2p::PeerId;
use tracing::{debug, info};

use super::super::SyncEngine;

impl SyncEngine {
    /// Retries sending pending messages from the outbox to a specific peer.
    ///
    /// This function fetches all pending messages and attempts to send those
    /// destined for `target_peer`. Successfully sent messages are removed
    /// from the outbox.
    ///
    /// # Arguments
    ///
    /// * `target_peer` - The `PeerId` of the peer to retry sending messages to.
    ///
    /// # Errors
    ///
    /// This function will return an error if there are issues accessing the
    /// outbox or sending messages via the network.
    pub async fn retry_outbox_for_peer(&self, target_peer: &PeerId) -> Result<()> {
        let pending_messages = self.outbox.get_pending().await?;

        if pending_messages.is_empty() {
            return Ok(());
        }

        let Some(network) = &self.network else {
            debug!("No network handle available for outbox retry");
            return Ok(());
        };

        debug!(
            "Retrying {} pending messages for peer {}",
            pending_messages.len(),
            target_peer
        );

        for message in pending_messages {
            if message.recipient != *target_peer {
                continue;
            }

            match network
                .send_message(message.recipient, message.clone())
                .await
            {
                Ok(()) => {
                    self.outbox.remove_pending(&message.id).await?;
                    info!(
                        "Successfully delivered message {} to {}",
                        message.id, message.recipient
                    );
                }
                Err(e) => {
                    debug!(
                        "Failed to deliver message {} to {}: {}",
                        message.id, message.recipient, e
                    );
                }
            }
        }

        Ok(())
    }
}
