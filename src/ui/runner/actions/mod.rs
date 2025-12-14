//! This module defines the actions that can be performed in the UI and their dispatching logic.
mod commands;
mod context;
mod dispatch;
mod execute;
mod resolver;

pub use dispatch::handle_ui_action;
