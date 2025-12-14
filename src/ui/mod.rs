//! This module contains the user interface (UI) logic for the application.
//!
//! It includes definitions for UI actions, different UI modes (chat and log),
//! rendering logic, state management, and terminal interaction.
pub mod action;
pub mod chat_mode;
pub mod completers;
pub mod event;
pub mod log_entry;
pub mod log_mode;
pub mod mode;
pub mod runner;
pub mod state;
pub mod terminal;

pub use action::UIAction;
pub use chat_mode::ChatMode;
pub use event::UIEvent;
pub use log_entry::LogEntry;
pub use log_mode::LogMode;
pub use mode::UIMode;
pub use runner::run_tui;
pub use state::UIState;
pub use terminal::TerminalUI;
