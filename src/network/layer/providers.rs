//! This module contains functions for interacting with the Kademlia DHT.
use anyhow::Result;
use tokio::sync::mpsc;

use crate::cli::commands::UiNotification;
use crate::mailbox::{make_mailbox_provider_key, make_recipient_mailbox_key};
use crate::sync::SyncEvent;

use super::NetworkLayer;

impl NetworkLayer {
    /// Sets the sender for synchronization events.
    pub fn set_sync_event_sender(&mut self, sender: mpsc::UnboundedSender<SyncEvent>) {
        self.sync_event_tx = Some(sender);
    }

    /// Sets the sender for UI notifications.
    pub fn set_ui_notify_sender(&mut self, sender: mpsc::UnboundedSender<UiNotification>) {
        self.ui_notify_tx = Some(sender);
    }

    /// Bootstraps the Kademlia DHT.
    ///
    /// # Errors
    ///
    /// This function will return an error if the bootstrap process fails.
    pub fn bootstrap_dht(&mut self) -> Result<()> {
        self.swarm.behaviour_mut().discovery.bootstrap()
    }

    /// Starts providing the general mailbox provider key in the Kademlia DHT.
    ///
    /// # Errors
    ///
    /// This function will return an error if the providing process fails to start.
    pub fn start_providing_mailbox(&mut self) -> Result<()> {
        let key = make_mailbox_provider_key();
        self.swarm.behaviour_mut().discovery.start_providing(key)
    }

    /// Starts providing a key for a specific recipient in the Kademlia DHT.
    ///
    /// # Arguments
    ///
    /// * `recipient_hash` - The hash of the recipient's public key.
    ///
    /// # Errors
    ///
    /// This function will return an error if the providing process fails to start.
    pub fn start_providing_for_recipient(&mut self, recipient_hash: [u8; 32]) -> Result<()> {
        let key = make_recipient_mailbox_key(recipient_hash);
        self.swarm.behaviour_mut().discovery.start_providing(key)
    }
}
