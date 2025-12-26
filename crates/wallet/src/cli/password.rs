//! Wallet password management

use anyhow::{Context, Result};
use colored::Colorize;
use std::io::IsTerminal;

use crate::storage::keychain::{SecureKeychain, WALLET_PASSWORD_ENV};

pub fn set(new_password: Option<String>, current_password: Option<String>) -> Result<()> {
    if !SecureKeychain::has_mnemonic() {
        anyhow::bail!("No wallet found. Use 'openagents wallet init' to create one.");
    }

    let mnemonic = if SecureKeychain::is_password_protected() {
        let current = match current_password {
            Some(password) => password,
            None => std::env::var(WALLET_PASSWORD_ENV)
                .unwrap_or(read_password("Current wallet password: ")?),
        };

        SecureKeychain::retrieve_mnemonic_with_password(&current)
            .context("Failed to unlock wallet with current password")?
    } else {
        SecureKeychain::retrieve_mnemonic()?
    };

    let new_password = match new_password {
        Some(password) => password,
        None => {
            let first = read_password("New wallet password: ")?;
            let confirm = read_password("Confirm wallet password: ")?;
            if first != confirm {
                anyhow::bail!("Passwords do not match");
            }
            first
        }
    };

    SecureKeychain::store_mnemonic_encrypted(&mnemonic, &new_password)
        .context("Failed to store encrypted wallet data")?;

    println!("{}", "✓ Wallet password set".green());
    Ok(())
}

fn read_password(prompt: &str) -> Result<String> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "Cannot read password from stdin. Use --password or set {}.",
            WALLET_PASSWORD_ENV
        );
    }

    let password = rpassword::prompt_password(prompt)
        .context("Failed to read password")?;
    if password.trim().is_empty() {
        anyhow::bail!("Password cannot be empty");
    }
    Ok(password)
}
