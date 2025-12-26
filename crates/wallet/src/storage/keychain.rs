//! Secure keychain integration for mnemonic storage
//!
//! Uses OS-native keychains:
//! - macOS: Keychain Access
//! - Linux: Secret Service (GNOME Keyring, KWallet)
//! - Windows: Credential Manager

use anyhow::{Context, Result};
use keyring::Entry;
use std::path::PathBuf;

const SERVICE_NAME: &str = "openagents-wallet";
const MNEMONIC_KEY: &str = "mnemonic";
const KEYCHAIN_FILE_ENV: &str = "OPENAGENTS_KEYCHAIN_FILE";

fn file_override_path() -> Option<PathBuf> {
    std::env::var_os(KEYCHAIN_FILE_ENV).map(PathBuf::from)
}

fn ensure_parent(path: &PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create keychain override directory")?;
    }
    Ok(())
}

/// Secure keychain for storing sensitive data
pub struct SecureKeychain;

impl SecureKeychain {
    /// Store mnemonic in OS keychain
    pub fn store_mnemonic(mnemonic: &str) -> Result<()> {
        if let Some(path) = file_override_path() {
            ensure_parent(&path)?;
            std::fs::write(&path, mnemonic).context("Failed to write keychain override file")?;
            return Ok(());
        }

        let entry = Entry::new(SERVICE_NAME, MNEMONIC_KEY)?;
        entry.set_password(mnemonic)?;
        Ok(())
    }

    /// Retrieve mnemonic from OS keychain
    pub fn retrieve_mnemonic() -> Result<String> {
        if let Some(path) = file_override_path() {
            let mnemonic = std::fs::read_to_string(&path)
                .context("Failed to read keychain override file")?;
            return Ok(mnemonic);
        }

        let entry = Entry::new(SERVICE_NAME, MNEMONIC_KEY)?;
        let mnemonic = entry.get_password()?;
        Ok(mnemonic)
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
