use libp2p::{kad, PeerId};
use std::collections::HashSet;

#[derive(Debug)]
pub enum SyncEvent {
    PeerConnected(PeerId),
    PeerConnectionFailed(PeerId),
    DhtQueryResult {
        query_id: kad::QueryId,
        result: DhtQueryResult,
    },
}

#[derive(Debug)]
pub enum DhtQueryResult {
    ProvidersFound {
        providers: HashSet<PeerId>,
        finished: bool,
    },
    QueryFailed {
        error: String,
    },
}
