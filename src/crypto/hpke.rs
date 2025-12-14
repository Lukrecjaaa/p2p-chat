//! This module implements a simplified version of Hybrid Public Key Encryption (HPKE).
//!
//! It uses X25519 for key exchange and ChaCha20Poly1305 for symmetric encryption.
//! The key derivation is done using SHA-256.
use anyhow::{anyhow, Result};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use rand_core::OsRng;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, SharedSecret, StaticSecret};

/// A context for performing E2E encryption using X25519 and ChaCha20Poly1305.
pub struct HpkeContext {
    private_key: StaticSecret,
}

impl HpkeContext {
    /// Generates a new keypair.
    ///
    /// This will create a new `HpkeContext` with a randomly generated X25519 keypair.
    ///
    /// # Errors
    ///
    /// This function will return an error if the key generation fails.
    pub fn new() -> Result<Self> {
        let private_key = StaticSecret::random_from_rng(OsRng);
        Ok(Self { private_key })
    }

    /// Loads a keypair from existing private key bytes.
    ///
    /// # Arguments
    ///
    /// * `private_key_bytes` - A 32-byte slice representing the private key.
    ///
    /// # Errors
    ///
    /// This function will return an error if the private key is not 32 bytes long.
    pub fn from_private_key(private_key_bytes: &[u8]) -> Result<Self> {
        let key_array: [u8; 32] = private_key_bytes
            .try_into()
            .map_err(|_| anyhow!("Private key must be 32 bytes"))?;

        let private_key = StaticSecret::from(key_array);
        Ok(Self { private_key })
    }

    /// Returns the public key bytes corresponding to our private key.
    pub fn public_key_bytes(&self) -> Vec<u8> {
        PublicKey::from(&self.private_key).as_bytes().to_vec()
    }

    /// Returns the private key bytes.
    pub fn private_key_bytes(&self) -> Vec<u8> {
        self.private_key.to_bytes().to_vec()
    }

    /// Derives a shared secret and uses it to encrypt a message for a recipient.
    ///
    /// # Arguments
    ///
    /// * `recipient_pub` - The public key of the recipient.
    /// * `plaintext` - The data to encrypt.
    ///
    /// # Returns
    ///
    /// A vector containing the nonce and the ciphertext.
    ///
    /// # Errors
    ///
    /// This function will return an error if encryption fails.
    pub fn seal(&self, recipient_pub: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
        let recipient_pk = self.parse_public_key(recipient_pub)?;
        let shared_secret = self.private_key.diffie_hellman(&recipient_pk);

        let key = self.derive_symmetric_key(&shared_secret);

        let cipher = ChaCha20Poly1305::new(&key);
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);

        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        let mut result = nonce.to_vec();
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Derives a shared secret and uses it to decrypt a message from a sender.
    ///
    /// # Arguments
    ///
    /// * `sender_pub` - The public key of the sender.
    /// * `ciphertext` - The data to decrypt (nonce prepended).
    ///
    /// # Returns
    ///
    /// A vector containing the plaintext.
    ///
    /// # Errors
    ///
    /// This function will return an error if decryption fails.
    pub fn open(&self, sender_pub: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
        if ciphertext.len() < 12 {
            return Err(anyhow!("Ciphertext is too short"));
        }

        let (nonce_bytes, encrypted_data) = ciphertext.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let sender_pk = self.parse_public_key(sender_pub)?;
        let shared_secret = self.private_key.diffie_hellman(&sender_pk);

        let key = self.derive_symmetric_key(&shared_secret);

        let cipher = ChaCha20Poly1305::new(&key);

        let plaintext = cipher
            .decrypt(nonce, encrypted_data)
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;

        Ok(plaintext)
    }

    /// Parses a public key from a byte slice.
    fn parse_public_key(&self, key_bytes: &[u8]) -> Result<PublicKey> {
        let key_array: [u8; 32] = key_bytes
            .try_into()
            .map_err(|_| anyhow!("Public key must be 32 bytes"))?;

        Ok(PublicKey::from(key_array))
    }

    /// Derives a symmetric key from a shared secret.
    fn derive_symmetric_key(&self, shared_secret: &SharedSecret) -> Key {
        let mut hasher = Sha256::new();
        hasher.update(shared_secret.as_bytes());
        hasher.finalize()
    }
}
