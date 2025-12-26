//! Secure keychain integration for mnemonic storage
//!
//! Uses OS-native keychains:
//! - macOS: Keychain Access
//! - Linux: Secret Service (GNOME Keyring, KWallet)
//! - Windows: Credential Manager

use anyhow::{Context, Result};
use argon2::Argon2;
use base64::engine::general_purpose::STANDARD as Base64;
use base64::Engine;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use keyring::Entry;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const SERVICE_NAME: &str = "openagents-wallet";
const MNEMONIC_KEY: &str = "mnemonic";
const KEYCHAIN_FILE_ENV: &str = "OPENAGENTS_KEYCHAIN_FILE";
pub const WALLET_PASSWORD_ENV: &str = "OPENAGENTS_WALLET_PASSWORD";

#[derive(Debug, Serialize, Deserialize)]
struct EncryptedMnemonic {
    version: u8,
    salt: String,
    nonce: String,
    ciphertext: String,
}

impl EncryptedMnemonic {
    fn encrypt(mnemonic: &str, password: &str) -> Result<Self> {
        if password.trim().is_empty() {
            anyhow::bail!("Password cannot be empty");
        }

        let mut salt = [0u8; 16];
        let mut nonce = [0u8; 12];
        let mut rng = rand::rng();
        rng.fill_bytes(&mut salt);
        rng.fill_bytes(&mut nonce);

        let key = derive_key(password, &salt)?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), mnemonic.as_bytes())
            .map_err(|_| anyhow::anyhow!("Failed to encrypt mnemonic"))?;

        Ok(Self {
            version: 1,
            salt: Base64.encode(salt),
            nonce: Base64.encode(nonce),
            ciphertext: Base64.encode(ciphertext),
        })
    }

    fn decrypt(&self, password: &str) -> Result<String> {
        if self.version != 1 {
            anyhow::bail!("Unsupported wallet encryption version");
        }

        let salt = Base64
            .decode(&self.salt)
            .context("Invalid wallet salt encoding")?;
        let nonce = Base64
            .decode(&self.nonce)
            .context("Invalid wallet nonce encoding")?;
        let ciphertext = Base64
            .decode(&self.ciphertext)
            .context("Invalid wallet ciphertext encoding")?;

        let key = derive_key(password, &salt)?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        let plaintext = cipher
            .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
            .map_err(|_| anyhow::anyhow!("Invalid wallet password"))?;

        let mnemonic = String::from_utf8(plaintext).context("Invalid mnemonic encoding")?;
        Ok(mnemonic)
    }

    fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if !trimmed.starts_with('{') {
            return None;
        }
        serde_json::from_str(trimmed).ok()
    }
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32]> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| anyhow::anyhow!("Failed to derive wallet key: {}", e))?;
    Ok(key)
}

fn file_override_path() -> Option<PathBuf> {
    std::env::var_os(KEYCHAIN_FILE_ENV).map(PathBuf::from)
}

fn ensure_parent(path: &PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create keychain override directory")?;
    }
    Ok(())
}

fn write_raw_mnemonic(value: &str) -> Result<()> {
    if let Some(path) = file_override_path() {
        ensure_parent(&path)?;
        std::fs::write(&path, value).context("Failed to write keychain override file")?;
        return Ok(());
    }

    let entry = Entry::new(SERVICE_NAME, MNEMONIC_KEY)?;
    entry.set_password(value)?;
    Ok(())
}

fn read_raw_mnemonic() -> Result<String> {
    if let Some(path) = file_override_path() {
        let mnemonic = std::fs::read_to_string(&path)
            .context("Failed to read keychain override file")?;
        return Ok(mnemonic);
    }

    let entry = Entry::new(SERVICE_NAME, MNEMONIC_KEY)?;
    let mnemonic = entry.get_password()?;
    Ok(mnemonic)
}

/// Secure keychain for storing sensitive data
pub struct SecureKeychain;

impl SecureKeychain {
    /// Store mnemonic in OS keychain
    pub fn store_mnemonic(mnemonic: &str) -> Result<()> {
        write_raw_mnemonic(mnemonic)
    }

    /// Store mnemonic encrypted with a wallet password
    pub fn store_mnemonic_encrypted(mnemonic: &str, password: &str) -> Result<()> {
        let encrypted = EncryptedMnemonic::encrypt(mnemonic, password)?;
        let payload = serde_json::to_string(&encrypted)
            .context("Failed to serialize encrypted wallet data")?;
        write_raw_mnemonic(&payload)
    }

    /// Retrieve mnemonic from OS keychain
    pub fn retrieve_mnemonic() -> Result<String> {
        let raw = read_raw_mnemonic()?;
        if EncryptedMnemonic::parse(&raw).is_some() {
            anyhow::bail!(
                "Wallet is password protected. Set {} to unlock.",
                WALLET_PASSWORD_ENV
            );
        }
        Ok(raw)
    }

    /// Retrieve mnemonic using a wallet password
    pub fn retrieve_mnemonic_with_password(password: &str) -> Result<String> {
        let raw = read_raw_mnemonic()?;
        if let Some(encrypted) = EncryptedMnemonic::parse(&raw) {
            return encrypted.decrypt(password);
        }
        Ok(raw)
    }

    /// Delete mnemonic from OS keychain
    pub fn delete_mnemonic() -> Result<()> {
        if let Some(path) = file_override_path() {
            if path.exists() {
                std::fs::remove_file(&path)
                    .context("Failed to delete keychain override file")?;
            }
            return Ok(());
        }

        let entry = Entry::new(SERVICE_NAME, MNEMONIC_KEY)?;
        entry.delete_credential()?;
        Ok(())
    }

    /// Check if mnemonic exists in keychain
    pub fn has_mnemonic() -> bool {
        if let Some(path) = file_override_path() {
            return path.is_file();
        }

        Entry::new(SERVICE_NAME, MNEMONIC_KEY)
            .and_then(|e| e.get_password())
            .is_ok()
    }

    /// Check if the mnemonic is protected by a password
    pub fn is_password_protected() -> bool {
        read_raw_mnemonic()
            .ok()
            .and_then(|raw| EncryptedMnemonic::parse(&raw))
            .is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires OS keychain access
    fn test_store_and_retrieve() {
        let test_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        SecureKeychain::store_mnemonic(test_mnemonic).unwrap();
        let retrieved = SecureKeychain::retrieve_mnemonic().unwrap();
        assert_eq!(retrieved, test_mnemonic);

        SecureKeychain::delete_mnemonic().unwrap();
    }
}
