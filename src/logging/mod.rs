//! This module contains the logging infrastructure for the application.
//!
//! It includes a `tracing` layer for collecting logs and a buffer for storing
//! them and sending them to the UI in batches.
pub mod buffer;
pub mod collector;

pub use buffer::LogBuffer;
pub use collector::TUILogCollector;
