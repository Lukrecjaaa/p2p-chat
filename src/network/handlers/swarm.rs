use super::super::{NetworkLayer, P2PBehaviourEvent};
use crate::sync::SyncEvent;
use crate::types::Message;
use anyhow::Result;
use libp2p::swarm::SwarmEvent;
use tokio::sync::mpsc;
use tracing::{info, trace, warn};

impl NetworkLayer {
    pub(crate) async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<P2PBehaviourEvent>,
        incoming_messages: &mpsc::UnboundedSender<Message>,
    ) -> Result<()> {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on: {}", address);
            }

            SwarmEvent::Behaviour(P2PBehaviourEvent::Chat(chat_event)) => {
                self.handle_chat_event(chat_event, incoming_messages)
                    .await?;
            }

            SwarmEvent::Behaviour(P2PBehaviourEvent::Mailbox(mailbox_event)) => {
                self.handle_mailbox_event(mailbox_event).await?;
            }

            SwarmEvent::Behaviour(P2PBehaviourEvent::Discovery(discovery_event)) => {
                self.handle_discovery_event(discovery_event).await?;
            }

            SwarmEvent::Behaviour(P2PBehaviourEvent::Ping(ping_event)) => {
                let libp2p::ping::Event { peer, result, .. } = ping_event;
                match result {
                    Ok(rtt) => {
                        trace!("Ping to {} successful: RTT is {:?}", peer, rtt);
                    }
                    Err(failure) => {
                        warn!("Ping to {} failed: {:?}", peer, failure);
                    }
                }
            }

            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                info!("Connection established with peer: {}", peer_id);
                if let Some(ref sync_tx) = self.sync_event_tx {
                    let _ = sync_tx.send(SyncEvent::PeerConnected(peer_id));
                }
            }

            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                info!("Disconnected from peer: {} (cause: {:?})", peer_id, cause);
            }

            SwarmEvent::IncomingConnection { .. } => {
                trace!("Incoming connection");
            }

            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                warn!("Outgoing connection error to {:?}: {}", peer_id, error);

                if let Some(peer_id) = peer_id {
                    if let Some(ref sync_tx) = self.sync_event_tx {
                        let _ = sync_tx.send(SyncEvent::PeerConnectionFailed(peer_id));
                    }
                }
            }

            SwarmEvent::IncomingConnectionError { error, .. } => {
                warn!("Incoming connection error: {}", error);
            }

            _ => {}
        }

        Ok(())
    }
}
