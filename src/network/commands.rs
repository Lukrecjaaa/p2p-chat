use super::{NetworkCommand, NetworkLayer, NetworkResponse};
use crate::types::{ChatRequest, MailboxRequest};
use anyhow::Result;
use libp2p::PeerId;
use tracing::debug;

impl NetworkLayer {
    pub(super) async fn handle_command(&mut self, command: NetworkCommand) -> Result<()> {
        match command {
            NetworkCommand::SendMessage {
                peer_id,
                message,
                response,
            } => {
                if !self.swarm.is_connected(&peer_id) {
                    debug!(
                        "Peer {} not connected, failing send request immediately.",
                        peer_id
                    );
                    let _ = response.send(NetworkResponse::Error("Peer not connected".to_string()));
                    return Ok(());
                }

                let request = ChatRequest::SendMessage { message };
                let request_id = self
                    .swarm
                    .behaviour_mut()
                    .chat
                    .send_request(&peer_id, request);
                self.pending_requests.insert(request_id, response);
            }

            NetworkCommand::MailboxPut {
                peer_id,
                recipient,
                message,
                response,
            } => {
                let request = MailboxRequest::Put { recipient, message };
                let request_id = self
                    .swarm
                    .behaviour_mut()
                    .mailbox
                    .send_request(&peer_id, request);
                self.pending_requests.insert(request_id, response);
            }

            NetworkCommand::MailboxFetch {
                peer_id,
                recipient,
                limit,
                response,
            } => {
                let request = MailboxRequest::Fetch { recipient, limit };
                let request_id = self
                    .swarm
                    .behaviour_mut()
                    .mailbox
                    .send_request(&peer_id, request);
                self.pending_requests.insert(request_id, response);
            }

            NetworkCommand::MailboxAck {
                peer_id,
                recipient,
                msg_ids,
                response,
            } => {
                let request = MailboxRequest::Ack { recipient, msg_ids };
                let request_id = self
                    .swarm
                    .behaviour_mut()
                    .mailbox
                    .send_request(&peer_id, request);
                self.pending_requests.insert(request_id, response);
            }

            NetworkCommand::GetConnectedPeers { response } => {
                let peers: Vec<PeerId> = self.swarm.connected_peers().cloned().collect();
                let _ = response.send(NetworkResponse::ConnectedPeers { peers });
            }

            NetworkCommand::StartDhtProviderQuery { key, response } => {
                let query_id = self.swarm.behaviour_mut().discovery.get_providers(key);
                let _ = response.send(Ok(query_id));
            }
        }

        Ok(())
    }
}
