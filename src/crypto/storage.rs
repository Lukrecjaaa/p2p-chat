use anyhow::{anyhow, Result};
use argon2::{Argon2, Params};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit},
    ChaCha20Poly1305, Nonce, Key,
};

#[derive(Clone)]
pub struct StorageEncryption {
    key: [u8; 32], // Derived via Argon2id
}

impl StorageEncryption {
    pub fn new(password: &str, salt: &[u8]) -> Result<Self> {
        if salt.len() != 16 {
            return Err(anyhow!("Salt must be 16 bytes"));
        }

        let params = Params::new(15000, 2, 1, Some(32))
            .map_err(|e| anyhow!("Argon2 params error: {:?}", e))?;
        let argon2 = Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            params,
        );

        let mut key = [0u8; 32];
        argon2.hash_password_into(
            password.as_bytes(),
            salt,
            &mut key,
        ).map_err(|e| anyhow!("Argon2 key derivation failed: {}", e))?;

        Ok(Self { key })
    }

    pub fn generate_salt() -> [u8; 16] {
        let mut salt = [0u8; 16];
        getrandom::getrandom(&mut salt).expect("Failed to generate random salt");
        salt
    }

    pub fn encrypt_value(&self, data: &[u8]) -> Result<Vec<u8>> {
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.key));
        let nonce = ChaCha20Poly1305::generate_nonce(&mut rand::rngs::OsRng);
        
        let ciphertext = cipher
            .encrypt(&nonce, data)
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        let mut result = nonce.to_vec();
        result.extend_from_slice(&ciphertext);
        
        Ok(result)
    }

    pub fn decrypt_value(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        if ciphertext.len() < 12 {
            return Err(anyhow!("Ciphertext too short"));
        }

        let (nonce_bytes, encrypted_data) = ciphertext.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.key));
        
        let plaintext = cipher
            .decrypt(nonce, encrypted_data)
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;

        Ok(plaintext)
    }

    pub fn derive_recipient_hash(public_key: &[u8]) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(b"p2p-messenger-recipient-");
        hasher.update(public_key);
        hasher.finalize().into()
    }
}