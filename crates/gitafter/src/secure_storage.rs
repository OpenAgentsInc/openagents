//! Secure storage for mnemonics using OS keychain
//!
//! Stores sensitive data (mnemonic seed phrases) in the operating system's
//! secure credential storage:
//! - macOS: Keychain
//! - Windows: Credential Manager
//! - Linux: Secret Service (libsecret)

use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE_NAME: &str = "com.openagents.gitafter";
const MNEMONIC_KEY: &str = "mnemonic";

/// Save mnemonic to secure storage
///
/// # Arguments
/// * `mnemonic` - BIP39 mnemonic phrase to store
///
/// # Returns
/// Ok(()) on success
///
/// # Errors
/// Returns error if keychain access fails or storage is unavailable
pub fn save_mnemonic(mnemonic: &str) -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, MNEMONIC_KEY)
        .context("Failed to create keyring entry")?;

    entry
        .set_password(mnemonic)
        .context("Failed to save mnemonic to keychain")?;

    tracing::info!("Mnemonic saved to secure storage");
    Ok(())
}

/// Load mnemonic from secure storage
///
/// # Returns
/// The stored mnemonic phrase, or None if not found
///
/// # Errors
/// Returns error if keychain access fails (but not if entry doesn't exist)
pub fn load_mnemonic() -> Result<Option<String>> {
    let entry = Entry::new(SERVICE_NAME, MNEMONIC_KEY)
        .context("Failed to create keyring entry")?;

    match entry.get_password() {
        Ok(mnemonic) => {
            tracing::info!("Loaded mnemonic from secure storage");
            Ok(Some(mnemonic))
        }
        Err(keyring::Error::NoEntry) => {
            tracing::debug!("No mnemonic found in secure storage");
            Ok(None)
        }
        Err(e) => {
            Err(e).context("Failed to load mnemonic from keychain")
        }
    }
}

/// Delete mnemonic from secure storage
///
/// # Returns
/// Ok(()) on success or if entry doesn't exist
///
/// # Errors
/// Returns error if keychain access fails
pub fn delete_mnemonic() -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, MNEMONIC_KEY)
        .context("Failed to create keyring entry")?;

    match entry.delete_credential() {
        Ok(()) => {
            tracing::info!("Deleted mnemonic from secure storage");
            Ok(())
        }
        Err(keyring::Error::NoEntry) => {
            tracing::debug!("No mnemonic to delete");
            Ok(())
        }
        Err(e) => {
            Err(e).context("Failed to delete mnemonic from keychain")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires OS keychain access
    fn test_save_and_load_mnemonic() {
        let test_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        // Clean up any existing entry
        let _ = delete_mnemonic();

        // Save mnemonic
        save_mnemonic(test_mnemonic).expect("Failed to save mnemonic");

        // Load it back
        let loaded = load_mnemonic().expect("Failed to load mnemonic");
        assert_eq!(loaded, Some(test_mnemonic.to_string()));

        // Clean up
        delete_mnemonic().expect("Failed to delete mnemonic");
    }

    #[test]
    #[ignore] // Requires OS keychain access
    fn test_load_nonexistent_mnemonic() {
        // Ensure no mnemonic exists
        let _ = delete_mnemonic();

        // Should return None, not error
        let loaded = load_mnemonic().expect("Failed to check for mnemonic");
        assert_eq!(loaded, None);
    }

    #[test]
    #[ignore] // Requires OS keychain access
    fn test_delete_mnemonic() {
        let test_mnemonic = "test test test test test test test test test test test junk";

        // Save mnemonic
        save_mnemonic(test_mnemonic).expect("Failed to save mnemonic");

        // Delete it
        delete_mnemonic().expect("Failed to delete mnemonic");

        // Verify it's gone
        let loaded = load_mnemonic().expect("Failed to check for mnemonic");
        assert_eq!(loaded, None);
    }
}
