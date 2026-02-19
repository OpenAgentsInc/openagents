//! Secure storage for seed phrase using AES-GCM encryption
//!
//! The seed phrase is encrypted with a key derived from a password using Argon2.

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use argon2::{
    Argon2, PasswordHasher,
    password_hash::{SaltString, rand_core::OsRng},
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;
use tokio::fs;

/// Size of the AES-GCM nonce in bytes
const NONCE_SIZE: usize = 12;

/// Errors that can occur during secure storage operations
#[derive(Debug, Error)]
pub enum SecureStoreError {
    #[error("encryption failed: {0}")]
    Encryption(String),

    #[error("decryption failed: {0}")]
    Decryption(String),

    #[error("key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("storage file not found")]
    NotFound,

    #[error("invalid password")]
    InvalidPassword,
}

/// Encrypted storage container
#[derive(Debug, Serialize, Deserialize)]
struct EncryptedData {
    /// Base64-encoded ciphertext
    ciphertext: String,
    /// Base64-encoded nonce
    nonce: String,
    /// Argon2 salt for key derivation
    salt: String,
    /// Version for future format changes
    version: u8,
    /// Argon2 parameters (stored to ensure future compatibility)
    #[serde(default)]
    argon2_params: Option<Argon2Params>,
}

/// Argon2 parameters for key derivation
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Argon2Params {
    /// Memory cost in KiB
    m_cost: u32,
    /// Time cost (iterations)
    t_cost: u32,
    /// Parallelism factor
    p_cost: u32,
}

impl Argon2Params {
    /// Extract parameters from an Argon2 instance
    fn from_argon2(argon2: &Argon2) -> Self {
        Self {
            m_cost: argon2.params().m_cost(),
            t_cost: argon2.params().t_cost(),
            p_cost: argon2.params().p_cost(),
        }
    }

    /// Create Argon2 instance with these parameters
    fn to_argon2(&self) -> Result<Argon2<'_>, SecureStoreError> {
        use argon2::ParamsBuilder;

        let params = ParamsBuilder::new()
            .m_cost(self.m_cost)
            .t_cost(self.t_cost)
            .p_cost(self.p_cost)
            .build()
            .map_err(|e| SecureStoreError::KeyDerivation(e.to_string()))?;

        Ok(Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            params,
        ))
    }
}

/// Secure storage for sensitive data like seed phrases
pub struct SecureStore {
    /// Path to the storage file
    path: PathBuf,
}

impl SecureStore {
    /// Create a new secure store at the given path
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Get the default storage path for the compute app
    pub fn default_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("openagents")
            .join("compute");

        config_dir.join("identity.enc")
    }

    /// Create a secure store at the default location
    pub fn with_default_path() -> Self {
        Self::new(Self::default_path())
    }

    /// Check if storage file exists
    pub async fn exists(&self) -> bool {
        fs::try_exists(&self.path).await.unwrap_or(false)
    }

    /// Path for unencrypted mnemonic (for auto-generated wallets)
    fn plaintext_path(&self) -> PathBuf {
        self.path.with_extension("seed")
    }

    /// Check if plaintext seed exists
    pub async fn plaintext_exists(&self) -> bool {
        fs::try_exists(self.plaintext_path()).await.unwrap_or(false)
    }

    /// Store mnemonic in plaintext (for auto-generated wallets before backup)
    ///
    /// Security measures:
    /// - Sets file permissions to 0600 (owner read/write only) on Unix
    /// - Warns if permissions cannot be set
    pub async fn store_plaintext(&self, mnemonic: &str) -> Result<(), SecureStoreError> {
        // Ensure directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let path = self.plaintext_path();

        // Write the mnemonic
        fs::write(&path, mnemonic).await?;

        // Harden file permissions on Unix systems (0600 = owner read/write only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(&path)?;
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o600); // owner read/write, no access for group/others
            std::fs::set_permissions(&path, permissions)?;
        }

        // On Windows, ACLs would be needed for similar protection
        #[cfg(windows)]
        {
            tracing::warn!(
                "Plaintext seed stored without permission hardening on Windows. \
                 Consider implementing ACL restrictions."
            );
        }

        Ok(())
    }

    /// Load plaintext mnemonic
    pub async fn load_plaintext(&self) -> Result<String, SecureStoreError> {
        if !self.plaintext_exists().await {
            return Err(SecureStoreError::NotFound);
        }
        let mnemonic = fs::read_to_string(self.plaintext_path()).await?;
        Ok(mnemonic.trim().to_string())
    }

    /// Delete plaintext seed (after backup is confirmed)
    pub async fn delete_plaintext(&self) -> Result<(), SecureStoreError> {
        if self.plaintext_exists().await {
            fs::remove_file(self.plaintext_path()).await?;
        }
        Ok(())
    }

    /// Store the seed phrase encrypted with a password
    pub async fn store(&self, mnemonic: &str, password: &str) -> Result<(), SecureStoreError> {
        // Generate salt for key derivation
        let salt = SaltString::generate(&mut OsRng);

        // Get Argon2 parameters and derive encryption key
        let argon2 = Argon2::default();
        let params = Argon2Params::from_argon2(&argon2);
        let key = derive_key_with_params(password, salt.as_str(), &argon2)?;

        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt the mnemonic
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| SecureStoreError::Encryption(e.to_string()))?;

        let ciphertext = cipher
            .encrypt(nonce, mnemonic.as_bytes())
            .map_err(|e| SecureStoreError::Encryption(e.to_string()))?;

        // Create encrypted data container with stored Argon2 parameters
        let data = EncryptedData {
            ciphertext: base64_encode(&ciphertext),
            nonce: base64_encode(&nonce_bytes),
            salt: salt.to_string(),
            version: 1,
            argon2_params: Some(params),
        };

        // Ensure directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Write to file
        let json = serde_json::to_string_pretty(&data)?;
        fs::write(&self.path, json).await?;

        Ok(())
    }

    /// Load and decrypt the seed phrase
    pub async fn load(&self, password: &str) -> Result<String, SecureStoreError> {
        if !self.exists().await {
            return Err(SecureStoreError::NotFound);
        }

        // Read encrypted data
        let json = fs::read_to_string(&self.path).await?;
        let data: EncryptedData = serde_json::from_str(&json)?;

        // Derive key using stored parameters (or defaults for old files)
        let key = if let Some(params) = &data.argon2_params {
            let argon2 = params.to_argon2()?;
            derive_key_with_params(password, &data.salt, &argon2)?
        } else {
            // Legacy: use defaults for files created before parameters were stored
            derive_key(password, &data.salt)?
        };

        // Decode ciphertext and nonce
        let ciphertext = base64_decode(&data.ciphertext)?;
        let nonce_bytes = base64_decode(&data.nonce)?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Decrypt
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| SecureStoreError::Decryption(e.to_string()))?;

        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| SecureStoreError::InvalidPassword)?;

        String::from_utf8(plaintext).map_err(|e| SecureStoreError::Decryption(e.to_string()))
    }

    /// Delete the stored identity
    pub async fn delete(&self) -> Result<(), SecureStoreError> {
        if self.exists().await {
            fs::remove_file(&self.path).await?;
        }
        Ok(())
    }

    /// Change the password for the stored identity
    pub async fn change_password(
        &self,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), SecureStoreError> {
        // Load with old password
        let mnemonic = self.load(old_password).await?;

        // Re-store with new password
        self.store(&mnemonic, new_password).await
    }
}

/// Derive a 32-byte key from password using Argon2 with specific parameters
fn derive_key_with_params(
    password: &str,
    salt: &str,
    argon2: &Argon2,
) -> Result<[u8; 32], SecureStoreError> {
    let salt =
        SaltString::from_b64(salt).map_err(|e| SecureStoreError::KeyDerivation(e.to_string()))?;

    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| SecureStoreError::KeyDerivation(e.to_string()))?;

    // Extract the hash output (first 32 bytes)
    let hash_bytes = hash
        .hash
        .ok_or_else(|| SecureStoreError::KeyDerivation("no hash output".to_string()))?;

    let bytes = hash_bytes.as_bytes();
    if bytes.len() < 32 {
        return Err(SecureStoreError::KeyDerivation(
            "hash output too short".to_string(),
        ));
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes[..32]);
    Ok(key)
}

/// Derive a 32-byte key from password using Argon2 (default parameters for legacy support)
fn derive_key(password: &str, salt: &str) -> Result<[u8; 32], SecureStoreError> {
    let argon2 = Argon2::default();
    derive_key_with_params(password, salt, &argon2)
}

/// Base64 encode bytes
fn base64_encode(data: &[u8]) -> String {
    use base64::engine::general_purpose::STANDARD;
    STANDARD.encode(data)
}

/// Base64 decode string
fn base64_decode(s: &str) -> Result<Vec<u8>, SecureStoreError> {
    use base64::engine::general_purpose::STANDARD;
    STANDARD
        .decode(s)
        .map_err(|e| SecureStoreError::Decryption(e.to_string()))
}

// Add base64 dependency for encoding/decoding
// This is a lightweight alternative to pulling in another crate
mod base64 {
    pub mod engine {
        pub mod general_purpose {
            pub struct StandardEngine;
            pub const STANDARD: StandardEngine = StandardEngine;

            impl StandardEngine {
                pub fn encode(&self, data: &[u8]) -> String {
                    const ALPHABET: &[u8] =
                        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

                    let mut result = String::new();
                    let chunks = data.chunks(3);

                    for chunk in chunks {
                        let mut buf = [0u8; 3];
                        buf[..chunk.len()].copy_from_slice(chunk);

                        let n = ((buf[0] as u32) << 16) | ((buf[1] as u32) << 8) | (buf[2] as u32);

                        result.push(ALPHABET[(n >> 18) as usize & 0x3F] as char);
                        result.push(ALPHABET[(n >> 12) as usize & 0x3F] as char);

                        if chunk.len() > 1 {
                            result.push(ALPHABET[(n >> 6) as usize & 0x3F] as char);
                        } else {
                            result.push('=');
                        }

                        if chunk.len() > 2 {
                            result.push(ALPHABET[n as usize & 0x3F] as char);
                        } else {
                            result.push('=');
                        }
                    }

                    result
                }

                pub fn decode(&self, s: &str) -> Result<Vec<u8>, &'static str> {
                    const DECODE_TABLE: [i8; 128] = [
                        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
                        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
                        -1, -1, -1, -1, -1, 62, -1, -1, -1, 63, 52, 53, 54, 55, 56, 57, 58, 59, 60,
                        61, -1, -1, -1, -1, -1, -1, -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
                        13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, -1, -1, -1, -1, -1, -1,
                        26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44,
                        45, 46, 47, 48, 49, 50, 51, -1, -1, -1, -1, -1,
                    ];

                    let s = s.trim_end_matches('=');
                    let mut result = Vec::with_capacity(s.len() * 3 / 4);

                    let chunks: Vec<char> = s.chars().collect();
                    for chunk in chunks.chunks(4) {
                        let mut n = 0u32;
                        for (i, &c) in chunk.iter().enumerate() {
                            let idx = c as usize;
                            if idx >= 128 {
                                return Err("invalid character");
                            }
                            let val = DECODE_TABLE[idx];
                            if val < 0 {
                                return Err("invalid character");
                            }
                            n |= (val as u32) << (18 - i * 6);
                        }

                        result.push((n >> 16) as u8);
                        if chunk.len() > 2 {
                            result.push((n >> 8) as u8);
                        }
                        if chunk.len() > 3 {
                            result.push(n as u8);
                        }
                    }

                    Ok(result)
                }
            }
        }
    }

    // StandardEngine exposes encode/decode directly.
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_store_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.enc");
        let store = SecureStore::new(path);

        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let password = "test_password_123";

        // Store
        store.store(mnemonic, password).await.unwrap();
        assert!(store.exists().await);

        // Load
        let loaded = store.load(password).await.unwrap();
        assert_eq!(loaded, mnemonic);
    }

    #[tokio::test]
    async fn test_wrong_password() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.enc");
        let store = SecureStore::new(path);

        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        store.store(mnemonic, "correct_password").await.unwrap();

        let result = store.load("wrong_password").await;
        assert!(matches!(result, Err(SecureStoreError::InvalidPassword)));
    }

    #[tokio::test]
    async fn test_not_found() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.enc");
        let store = SecureStore::new(path);

        let result = store.load("password").await;
        assert!(matches!(result, Err(SecureStoreError::NotFound)));
    }

    #[tokio::test]
    async fn test_change_password() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.enc");
        let store = SecureStore::new(path);

        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        store.store(mnemonic, "old_password").await.unwrap();
        store
            .change_password("old_password", "new_password")
            .await
            .unwrap();

        // Old password should fail
        let result = store.load("old_password").await;
        assert!(matches!(result, Err(SecureStoreError::InvalidPassword)));

        // New password should work
        let loaded = store.load("new_password").await.unwrap();
        assert_eq!(loaded, mnemonic);
    }

    #[test]
    fn test_base64_roundtrip() {
        let data = b"Hello, World!";
        let encoded = base64_encode(data);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }
}
