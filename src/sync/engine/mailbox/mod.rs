//! This module contains mailbox-related logic for the synchronization engine.
//!
//! It handles fetching messages, acknowledging them, and managing the reliability
//! of mailbox interactions.
mod ack;
mod fetch;
mod processing;
mod reliability;
