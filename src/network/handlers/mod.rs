//! This module contains the event handlers for the various network behaviours.
//!
//! The individual modules extend the `NetworkLayer` implementation with
//! specialized handlers for each of the behaviours.
mod chat;
mod discovery;
mod mailbox;
mod swarm;
