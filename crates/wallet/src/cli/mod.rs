//! CLI command implementations

use anyhow::{Context, Result};
use std::io::IsTerminal;

use crate::storage::keychain::{SecureKeychain, WALLET_PASSWORD_ENV};

pub mod bitcoin;
pub mod frostr;
pub mod identity;
pub mod password;
pub mod settings;

pub fn load_mnemonic() -> Result<String> {
    if !SecureKeychain::has_mnemonic() {
        anyhow::bail!("No wallet found. Use 'openagents wallet init' to create one.");
    }

    if SecureKeychain::is_password_protected() {
        if let Ok(password) = std::env::var(WALLET_PASSWORD_ENV) {
            return SecureKeychain::retrieve_mnemonic_with_password(&password)
                .context("Failed to unlock wallet with provided password");
        }

        if std::io::stdin().is_terminal() {
            let password = rpassword::prompt_password("Wallet password: ")
                .context("Failed to read wallet password")?;
            return SecureKeychain::retrieve_mnemonic_with_password(&password)
                .context("Failed to unlock wallet with provided password");
        }

        anyhow::bail!("Wallet is password protected. Set {} to unlock.", WALLET_PASSWORD_ENV);
    }

    SecureKeychain::retrieve_mnemonic()
}
