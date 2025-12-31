//! CLI command implementations

use anyhow::{Context, Result};
use std::io::IsTerminal;

use crate::storage::identities::{DEFAULT_IDENTITY_NAME, current_identity};
use crate::storage::keychain::{SecureKeychain, WALLET_PASSWORD_ENV};

pub mod bitcoin;
pub mod error;
pub mod frostr;
pub mod identity;
pub mod password;
pub mod payee;
pub mod settings;
pub mod validation;

pub fn load_mnemonic() -> Result<String> {
    let identity = current_identity().unwrap_or_else(|_| DEFAULT_IDENTITY_NAME.to_string());
    if !SecureKeychain::has_mnemonic_for(&identity) {
        anyhow::bail!(
            "No wallet found for identity '{}'. Use 'openagents wallet init' or 'openagents wallet identity create'.",
            identity
        );
    }

    if SecureKeychain::is_password_protected_for(&identity) {
        if let Ok(password) = std::env::var(WALLET_PASSWORD_ENV) {
            return SecureKeychain::retrieve_mnemonic_with_password_for(&identity, &password)
                .context("Failed to unlock wallet with provided password");
        }

        if std::io::stdin().is_terminal() {
            let password = rpassword::prompt_password("Wallet password: ")
                .context("Failed to read wallet password")?;
            return SecureKeychain::retrieve_mnemonic_with_password_for(&identity, &password)
                .context("Failed to unlock wallet with provided password");
        }

        anyhow::bail!(
            "Wallet is password protected. Set {} to unlock.",
            WALLET_PASSWORD_ENV
        );
    }

    SecureKeychain::retrieve_mnemonic_for(&identity)
}
