//! This module contains outbox-related logic for the synchronization engine.
//!
//! It handles the direct sending of messages, forwarding to mailboxes, and
//! retrying failed message deliveries.
mod direct;
mod forward;
mod retry;
