//! Secure keychain integration for mnemonic storage
//!
//! Uses OS-native keychains:
//! - macOS: Keychain Access
//! - Linux: Secret Service (GNOME Keyring, KWallet)
//! - Windows: Credential Manager

use anyhow::Result;
use keyring::Entry;

const SERVICE_NAME: &str = "openagents-wallet";
const MNEMONIC_KEY: &str = "mnemonic";

/// Secure keychain for storing sensitive data
pub struct SecureKeychain;

impl SecureKeychain {
    /// Store mnemonic in OS keychain
    pub fn store_mnemonic(mnemonic: &str) -> Result<()> {
        let entry = Entry::new(SERVICE_NAME, MNEMONIC_KEY)?;
        entry.set_password(mnemonic)?;
        Ok(())
    }

    /// Retrieve mnemonic from OS keychain
    pub fn retrieve_mnemonic() -> Result<String> {
        let entry = Entry::new(SERVICE_NAME, MNEMONIC_KEY)?;
        let mnemonic = entry.get_password()?;
        Ok(mnemonic)
    }

    /// Delete mnemonic from OS keychain
    pub fn delete_mnemonic() -> Result<()> {
        let entry = Entry::new(SERVICE_NAME, MNEMONIC_KEY)?;
        entry.delete_credential()?;
        Ok(())
    }

    /// Check if mnemonic exists in keychain
    pub fn has_mnemonic() -> bool {
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
