//! This module manages the user's identity, which consists of a libp2p keypair
//! and an HPKE keypair.
use crate::crypto::HpkeContext;
use anyhow::Result;
use libp2p::{identity, PeerId};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// A serializable representation of the user's keypairs.
#[derive(Serialize, Deserialize)]
pub struct KeyPair {
    /// The serialized Ed25519 keypair for libp2p.
    pub libp2p_keypair: Vec<u8>,
    /// The X25519 private key for HPKE.
    pub hpke_private_key: Vec<u8>,
    /// The X25519 public key for HPKE.
    pub hpke_public_key: Vec<u8>,
}

/// Represents the user's identity, including their libp2p and HPKE keypairs.
pub struct Identity {
    /// The user's peer ID, derived from the libp2p public key.
    pub peer_id: PeerId,
    /// The libp2p keypair.
    pub libp2p_keypair: identity::Keypair,
    /// The HPKE context, containing the HPKE keypair.
    pub hpke_context: HpkeContext,
}

impl Identity {
    /// Generates a new identity.
    ///
    /// This will create a new `Identity` with a randomly generated libp2p Ed25519
    /// keypair and an HPKE X25519 keypair.
    ///
    /// # Errors
    ///
    /// This function will return an error if key generation fails.
    pub fn generate() -> Result<Self> {
        // Generate libp2p Ed25519 keypair.
        let libp2p_keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(libp2p_keypair.public());

        // Generate HPKE X25519 keypair.
        let hpke_context = HpkeContext::new()?;

        Ok(Self {
            peer_id,
            libp2p_keypair,
            hpke_context,
        })
    }

    /// Loads an identity from a file, or generates a new one if the file doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the identity file.
    ///
    /// # Errors
    ///
    /// This function will return an error if loading or generating the identity fails.
    pub fn load_or_generate(path: &str) -> Result<Self> {
        if Path::new(path).exists() {
            Self::load(path)
        } else {
            let identity = Self::generate()?;
            identity.save(path)?;
            Ok(identity)
        }
    }

    /// Loads an identity from a file.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the identity file.
    ///
    /// # Errors
    ///
    /// This function will return an error if the file cannot be read or if the
    /// keypair data is invalid.
    pub fn load(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let keypair_data: KeyPair = serde_json::from_str(&content)?;

        // Reconstruct libp2p keypair.
        let libp2p_keypair =
            identity::Keypair::from_protobuf_encoding(&keypair_data.libp2p_keypair)?;
        let peer_id = PeerId::from(libp2p_keypair.public());

        // Reconstruct HPKE context.
        let hpke_context = HpkeContext::from_private_key(&keypair_data.hpke_private_key)?;

        Ok(Self {
            peer_id,
            libp2p_keypair,
            hpke_context,
        })
    }

    /// Saves the identity to a file.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the identity file.
    ///
    /// # Errors
    ///
    /// This function will return an error if the identity cannot be saved.
    pub fn save(&self, path: &str) -> Result<()> {
        let keypair_data = KeyPair {
            libp2p_keypair: self.libp2p_keypair.to_protobuf_encoding()?,
            hpke_private_key: self.hpke_context.private_key_bytes(),
            hpke_public_key: self.hpke_context.public_key_bytes(),
        };

        let content = serde_json::to_string_pretty(&keypair_data)?;

        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, content)?;
        Ok(())
    }

    /// Returns the HPKE public key bytes.
    pub fn hpke_public_key(&self) -> Vec<u8> {
        self.hpke_context.public_key_bytes()
    }

    /// Encrypts a message for a recipient using their public key.
    ///
    /// # Arguments
    ///
    /// * `recipient_public_key` - The public key of the recipient.
    /// * `plaintext` - The data to encrypt.
    /// This function will return an error if encryption fails.
    pub fn encrypt_for(&self, recipient_public_key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
        self.hpke_context.seal(recipient_public_key, plaintext)
    }

    /// Decrypts a message from a sender using their public key.
    ///
    /// # Arguments
    ///
    /// * `sender_public_key` - The public key of the sender.
    /// * `ciphertext` - The data to decrypt.
    ///
    /// # Errors
    ///
    /// This function will return an error if decryption fails.
    pub fn decrypt_from(&self, sender_public_key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
        self.hpke_context.open(sender_public_key, ciphertext)
    }
}
