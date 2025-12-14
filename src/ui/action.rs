//! This module defines the actions that can be performed in the UI.
#[derive(Debug)]
pub enum UIAction {
    /// Sends a message to a recipient.
    ///
    /// The first `String` is the recipient's PeerId, the second is the message content.
    SendMessage(String, String),
    /// Executes a command entered by the user.
    ExecuteCommand(String),
    /// Exits the application.
    Exit,
}
