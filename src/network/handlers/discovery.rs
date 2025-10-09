use super::super::NetworkLayer;
use crate::net::discovery::DiscoveryBehaviourEvent;
use crate::sync::{DhtQueryResult, SyncEvent};
use anyhow::Result;
use libp2p::kad;
use std::collections::HashSet;
use tracing::{debug, error, info, trace};

impl NetworkLayer {
    pub(super) async fn handle_discovery_event(
        &mut self,
        event: DiscoveryBehaviourEvent,
    ) -> Result<()> {
        match event {
            DiscoveryBehaviourEvent::Mdns(mdns_event) => match mdns_event {
                libp2p::mdns::Event::Discovered(list) => {
                    for (peer_id, multiaddr) in list {
                        info!("Discovered peer via mDNS: {} at {}", peer_id, multiaddr);

                        if self.blocked_peers.contains_key(&peer_id) {
                            debug!("Skipping mDNS discovery for blocked peer {}", peer_id);
                            continue;
                        }

                        self.swarm
                            .behaviour_mut()
                            .discovery
                            .add_peer_address(peer_id, multiaddr.clone());

                        if let Err(e) = self.swarm.dial(multiaddr) {
                            trace!(
                                "Failed to proactively dial discovered peer {}: {}",
                                peer_id,
                                e
                            );
                        }
                    }
                }
                libp2p::mdns::Event::Expired(list) => {
                    for (peer_id, _) in list {
                        trace!("mDNS record expired for peer: {}", peer_id);
                    }
                }
            },
            DiscoveryBehaviourEvent::Kademlia(kad_event) => {
                self.handle_kademlia_event(kad_event).await?;
            }
        }

        Ok(())
    }

    async fn handle_kademlia_event(&mut self, event: kad::Event) -> Result<()> {
        match event {
            kad::Event::OutboundQueryProgressed { id, result, .. } => match result {
                kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
                    key,
                    providers,
                    ..
                })) => {
                    if !providers.is_empty() {
                        trace!("Found {} providers for key: {:?}", providers.len(), key);
                    }

                    if let Some(sync_tx) = &self.sync_event_tx {
                        let dht_result = DhtQueryResult::ProvidersFound {
                            providers: providers.into_iter().collect(),
                            finished: false,
                        };
                        let _ = sync_tx.send(SyncEvent::DhtQueryResult {
                            query_id: id,
                            result: dht_result,
                        });
                    }
                }
                kad::QueryResult::GetProviders(Ok(
                    kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. },
                )) => {
                    trace!("DHT query {} finished with no additional providers", id);

                    if let Some(sync_tx) = &self.sync_event_tx {
                        let dht_result = DhtQueryResult::ProvidersFound {
                            providers: HashSet::new(),
                            finished: true,
                        };
                        let _ = sync_tx.send(SyncEvent::DhtQueryResult {
                            query_id: id,
                            result: dht_result,
                        });
                    }
                }
                kad::QueryResult::GetProviders(Err(e)) => {
                    error!("DHT provider query {} failed: {:?}", id, e);

                    if let Some(sync_tx) = &self.sync_event_tx {
                        let dht_result = DhtQueryResult::QueryFailed {
                            error: format!("{:?}", e),
                        };
                        let _ = sync_tx.send(SyncEvent::DhtQueryResult {
                            query_id: id,
                            result: dht_result,
                        });
                    }
                }
                _ => {}
            },
            kad::Event::RoutingUpdated { peer, .. } => {
                trace!("Kademlia routing table updated for peer: {}", peer);
            }
            _ => {}
        }

        Ok(())
    }
}
