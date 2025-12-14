//! This module defines the events that can be sent to the `SyncEngine`.
use libp2p::{kad, PeerId};
use std::collections::HashSet;

/// Events that can be sent to the `SyncEngine` to trigger synchronization logic.
#[derive(Debug)]
pub enum SyncEvent {
    /// A peer has successfully connected to the local node.
    PeerConnected(PeerId),
    /// A connection attempt to a peer has failed.
    PeerConnectionFailed(PeerId),
    /// The result of a Kademlia DHT query.
    DhtQueryResult {
        /// The ID of the query.
        query_id: kad::QueryId,
        /// The result of the query.
        result: DhtQueryResult,
    },
}

/// The result of a Kademlia DHT query.
#[derive(Debug)]
pub enum DhtQueryResult {
    /// Providers for a key were found.
    ProvidersFound {
        /// The set of found providers.
        providers: HashSet<PeerId>,
        /// Whether this is the final result for the query.
        finished: bool,
    },
    /// The DHT query failed.
    QueryFailed {
        /// A description of the error.
        error: String,
    },
}
