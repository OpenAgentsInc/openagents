//! Secure keychain integration for mnemonic storage
//!
//! Uses OS-native keychains:
//! - macOS: Keychain Access
//! - Linux: Secret Service (GNOME Keyring, KWallet)
//! - Windows: Credential Manager

#![allow(dead_code)]

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
pub const DEFAULT_IDENTITY_NAME: &str = "default";

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

fn file_override_path(identity: &str) -> Option<PathBuf> {
    let base = PathBuf::from(std::env::var_os(KEYCHAIN_FILE_ENV)?);
    if base.is_dir() {
        return Some(base.join(format!("{}.txt", identity)));
    }

    if identity == DEFAULT_IDENTITY_NAME {
        return Some(base);
    }

    let file_name = base
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(MNEMONIC_KEY);
    let mut with_identity = base.clone();
    with_identity.set_file_name(format!("{}.{}", file_name, identity));
    Some(with_identity)
}

fn ensure_parent(path: &PathBuf) -> Result<()> {
    if path.is_dir() {
        std::fs::create_dir_all(path).context("Failed to create keychain override directory")?;
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create keychain override directory")?;
    }
    Ok(())
}

fn keychain_key(identity: &str) -> String {
    if identity == DEFAULT_IDENTITY_NAME {
        MNEMONIC_KEY.to_string()
    } else {
        format!("{}:{}", MNEMONIC_KEY, identity)
    }
}

fn keychain_entry(identity: &str) -> Result<Entry> {
    let key = keychain_key(identity);
    Ok(Entry::new(SERVICE_NAME, &key)?)
}

fn write_raw_mnemonic_for(identity: &str, value: &str) -> Result<()> {
    if let Some(path) = file_override_path(identity) {
        ensure_parent(&path)?;
        std::fs::write(&path, value).context("Failed to write keychain override file")?;
        return Ok(());
    }

    let entry = keychain_entry(identity)?;
    entry.set_password(value)?;
    Ok(())
}

fn read_raw_mnemonic_for(identity: &str) -> Result<String> {
    if let Some(path) = file_override_path(identity) {
        let mnemonic = std::fs::read_to_string(&path)
            .context("Failed to read keychain override file")?;
        return Ok(mnemonic);
    }

    let entry = keychain_entry(identity)?;
    let mnemonic = entry.get_password()?;
    Ok(mnemonic)
}

/// Secure keychain for storing sensitive data
pub struct SecureKeychain;

impl SecureKeychain {
    /// Store mnemonic in OS keychain
    pub fn store_mnemonic(mnemonic: &str) -> Result<()> {
        Self::store_mnemonic_for(DEFAULT_IDENTITY_NAME, mnemonic)
    }

    /// Store mnemonic encrypted with a wallet password
    pub fn store_mnemonic_encrypted(mnemonic: &str, password: &str) -> Result<()> {
        Self::store_mnemonic_encrypted_for(DEFAULT_IDENTITY_NAME, mnemonic, password)
    }

    pub fn store_mnemonic_for(identity: &str, mnemonic: &str) -> Result<()> {
        write_raw_mnemonic_for(identity, mnemonic)
    }

    pub fn store_mnemonic_encrypted_for(
        identity: &str,
        mnemonic: &str,
        password: &str,
    ) -> Result<()> {
        let encrypted = EncryptedMnemonic::encrypt(mnemonic, password)?;
        let payload = serde_json::to_string(&encrypted)
            .context("Failed to serialize encrypted wallet data")?;
        write_raw_mnemonic_for(identity, &payload)
    }

    /// Retrieve mnemonic from OS keychain
    pub fn retrieve_mnemonic() -> Result<String> {
        Self::retrieve_mnemonic_for(DEFAULT_IDENTITY_NAME)
    }

    /// Retrieve mnemonic using a wallet password
    pub fn retrieve_mnemonic_with_password(password: &str) -> Result<String> {
        Self::retrieve_mnemonic_with_password_for(DEFAULT_IDENTITY_NAME, password)
    }

    pub fn retrieve_mnemonic_for(identity: &str) -> Result<String> {
        let raw = read_raw_mnemonic_for(identity)?;
        if EncryptedMnemonic::parse(&raw).is_some() {
            anyhow::bail!(
                "Wallet is password protected. Set {} to unlock.",
                WALLET_PASSWORD_ENV
            );
        }
        Ok(raw)
    }

    pub fn retrieve_mnemonic_with_password_for(
        identity: &str,
        password: &str,
    ) -> Result<String> {
        let raw = read_raw_mnemonic_for(identity)?;
        if let Some(encrypted) = EncryptedMnemonic::parse(&raw) {
            return encrypted.decrypt(password);
        }
        Ok(raw)
    }

    /// Delete mnemonic from OS keychain
    pub fn delete_mnemonic() -> Result<()> {
        Self::delete_mnemonic_for(DEFAULT_IDENTITY_NAME)
    }

    pub fn delete_mnemonic_for(identity: &str) -> Result<()> {
        if let Some(path) = file_override_path(identity) {
            if path.exists() {
                std::fs::remove_file(&path)
                    .context("Failed to delete keychain override file")?;
            }
            return Ok(());
        }

        let entry = keychain_entry(identity)?;
        entry.delete_credential()?;
        Ok(())
    }

    /// Check if mnemonic exists in keychain
    pub fn has_mnemonic() -> bool {
        Self::has_mnemonic_for(DEFAULT_IDENTITY_NAME)
    }

    /// Check if the mnemonic is protected by a password
    pub fn is_password_protected() -> bool {
        Self::is_password_protected_for(DEFAULT_IDENTITY_NAME)
    }

    pub fn has_mnemonic_for(identity: &str) -> bool {
        if let Some(path) = file_override_path(identity) {
            return path.is_file();
        }

        match keychain_entry(identity) {
            Ok(entry) => entry.get_password().is_ok(),
            Err(_) => false,
        }
    }

    pub fn is_password_protected_for(identity: &str) -> bool {
        read_raw_mnemonic_for(identity)
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
