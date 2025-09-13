use anyhow::Result;
use libp2p::{identity, PeerId};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use crate::crypto::HpkeContext;

#[derive(Serialize, Deserialize)]
pub struct KeyPair {
    pub libp2p_keypair: Vec<u8>, // Serialized Ed25519 keypair
    pub hpke_private_key: Vec<u8>, // X25519 private key
    pub hpke_public_key: Vec<u8>,  // X25519 public key
}

pub struct Identity {
    pub peer_id: PeerId,
    pub libp2p_keypair: identity::Keypair,
    pub hpke_context: HpkeContext,
}

impl Identity {
    pub fn generate() -> Result<Self> {
        // Generate libp2p Ed25519 keypair
        let libp2p_keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(libp2p_keypair.public());

        // Generate HPKE X25519 keypair
        let hpke_context = HpkeContext::new()?;

        Ok(Self {
            peer_id,
            libp2p_keypair,
            hpke_context,
        })
    }

    pub fn load_or_generate(path: &str) -> Result<Self> {
        if Path::new(path).exists() {
            Self::load(path)
        } else {
            let identity = Self::generate()?;
            identity.save(path)?;
            Ok(identity)
        }
    }

    pub fn load(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let keypair_data: KeyPair = serde_json::from_str(&content)?;

        // Reconstruct libp2p keypair
        let libp2p_keypair = identity::Keypair::from_protobuf_encoding(&keypair_data.libp2p_keypair)?;
        let peer_id = PeerId::from(libp2p_keypair.public());

        // Reconstruct HPKE context
        let hpke_context = HpkeContext::from_private_key(&keypair_data.hpke_private_key)?;

        Ok(Self {
            peer_id,
            libp2p_keypair,
            hpke_context,
        })
    }

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

    pub fn hpke_public_key(&self) -> Vec<u8> {
        self.hpke_context.public_key_bytes()
    }

    /// Encrypt a message for a recipient using their public key.
    pub fn encrypt_for(&self, recipient_public_key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
        self.hpke_context.seal(recipient_public_key, plaintext)
    }

    /// Decrypt a message from a sender using their public key.
    pub fn decrypt_from(&self, sender_public_key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
        self.hpke_context.open(sender_public_key, ciphertext)
    }
}