//! Identity discovery - check for pylon identity and wallet.

use crate::manifest::IdentityManifest;
use nostr::derive_keypair;
use std::path::PathBuf;

/// Get the pylon data directory.
fn pylon_data_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".openagents").join("pylon"))
}

/// Discover identity status.
pub async fn discover_identity() -> anyhow::Result<IdentityManifest> {
    let data_dir = match pylon_data_dir() {
        Some(d) => d,
        None => {
            return Ok(IdentityManifest {
                initialized: false,
                npub: None,
                wallet_balance_sats: None,
                network: None,
            })
        }
    };

    // Check if identity file exists
    let identity_file = data_dir.join("identity.mnemonic");
    let initialized = identity_file.exists();

    // If initialized, try to derive npub
    let npub = if initialized {
        derive_npub(&identity_file).ok()
    } else {
        None
    };

    // Check config for network
    let config_file = data_dir.join("config.toml");
    let network = if config_file.exists() {
        read_network_from_config(&config_file).ok()
    } else {
        None
    };

    // We'd need to actually connect to wallet to get balance
    // For now, just indicate if we have identity
    Ok(IdentityManifest {
        initialized,
        npub,
        wallet_balance_sats: None, // Would need wallet connection
        network,
    })
}

/// Derive npub from identity mnemonic using NIP-06.
fn derive_npub(identity_file: &PathBuf) -> anyhow::Result<String> {
    let mnemonic = std::fs::read_to_string(identity_file)?;
    let mnemonic = mnemonic.trim();

    // Derive keypair using NIP-06 standard derivation path
    let keypair = derive_keypair(mnemonic)?;
    let npub = keypair.npub()?;

    Ok(npub)
}

/// Read network setting from config.
fn read_network_from_config(config_file: &PathBuf) -> anyhow::Result<String> {
    let content = std::fs::read_to_string(config_file)?;

    // Simple parsing - look for network setting
    for line in content.lines() {
        if line.trim().starts_with("network") {
            if line.contains("mainnet") {
                return Ok("mainnet".to_string());
            } else if line.contains("regtest") {
                return Ok("regtest".to_string());
            } else if line.contains("signet") {
                return Ok("signet".to_string());
            }
        }
    }

    Ok("regtest".to_string()) // default
}
