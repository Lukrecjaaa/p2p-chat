//! This module contains all the cryptographic-related logic for the application.
//!
//! It includes modules for:
//! * `hpke`: A simplified implementation of Hybrid Public Key Encryption.
//! * `identity`: Management of the user's identity, including libp2p and HPKE keypairs.
//! * `storage`: Encryption of data at rest.
pub mod hpke;
pub mod identity;
pub mod storage;

pub use hpke::HpkeContext;
pub use identity::Identity;
pub use storage::StorageEncryption;
