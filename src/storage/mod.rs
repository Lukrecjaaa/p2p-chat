//! This module defines the storage interfaces and implementations for various
//! application data, including friends, message history, mailboxes, and seen messages.
pub mod friends;
pub mod history;
pub mod known_mailboxes;
pub mod mailbox;
pub mod outbox;
pub mod seen;

pub use friends::{FriendsStore, SledFriendsStore};
pub use history::{MessageHistory, MessageStore};
pub use known_mailboxes::{KnownMailbox, KnownMailboxesStore, SledKnownMailboxesStore};
pub use mailbox::{MailboxStore, SledMailboxStore};
pub use outbox::{OutboxStore, SledOutboxStore};
pub use seen::{SeenTracker, SledSeenTracker};
