//! This module contains the synchronization logic for the application.
//!
//! It includes mechanisms for exponential backoff, the core synchronization
//! engine, and retry policies for network operations.
pub mod backoff;
pub mod engine;
pub mod retry;

pub use engine::{DhtQueryResult, SyncEngine, SyncEvent, SyncStores};
