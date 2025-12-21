//! Bitcoin/Lightning CLI commands

use anyhow::Result;
use colored::Colorize;
use crate::storage::keychain::SecureKeychain;
use bip39::Mnemonic;

pub fn balance() -> Result<()> {
    use spark::{SparkSigner, SparkWallet, WalletConfig, Network};

    println!("{}", "Balance".cyan().bold());
    println!();

    // Check if wallet exists
    if !SecureKeychain::has_mnemonic() {
        anyhow::bail!("No wallet found. Use 'wallet init' to create one.");
    }

    // Load identity and create spark wallet
    let mnemonic_phrase = SecureKeychain::retrieve_mnemonic()?;
    let mnemonic = Mnemonic::parse(&mnemonic_phrase)?;

    let signer = SparkSigner::from_mnemonic(&mnemonic.to_string(), "")?;
    let config = WalletConfig {
        network: Network::Testnet,
        ..Default::default()
    };

    // Create wallet and get balance (async)
    // Note: This will fail until Breez SDK is integrated (see directive d-001)
    let rt = tokio::runtime::Runtime::new()?;
    let wallet = rt.block_on(SparkWallet::new(signer, config))?;
    let balance = rt.block_on(wallet.get_balance())?;

    println!("{}: {} sats", "Total".bold(), balance.total_sats());

    Ok(())
}

pub fn balance_detailed() -> Result<()> {
    use spark::{SparkSigner, SparkWallet, WalletConfig, Network};

    println!("{}", "Balance Breakdown".cyan().bold());
    println!();

    // Check if wallet exists
    if !SecureKeychain::has_mnemonic() {
        anyhow::bail!("No wallet found. Use 'wallet init' to create one.");
    }

    // Load identity and create spark wallet
    let mnemonic_phrase = SecureKeychain::retrieve_mnemonic()?;
    let mnemonic = Mnemonic::parse(&mnemonic_phrase)?;

    let signer = SparkSigner::from_mnemonic(&mnemonic.to_string(), "")?;
    let config = WalletConfig {
        network: Network::Testnet,
        ..Default::default()
    };

    // Create wallet and get balance (async)
    // Note: This will fail until Breez SDK is integrated (see directive d-001)
    let rt = tokio::runtime::Runtime::new()?;
    let wallet = rt.block_on(SparkWallet::new(signer, config))?;
    let balance = rt.block_on(wallet.get_balance())?;

    println!("{}: {} sats", "Spark L2".bold(), balance.spark_sats);
    println!("{}: {} sats", "Lightning".bold(), balance.lightning_sats);
    println!("{}: {} sats", "On-chain".bold(), balance.onchain_sats);
    println!("─────────────────────");
    println!("{}: {} sats", "Total".bold(), balance.total_sats());

    Ok(())
}

pub fn receive(amount: Option<u64>) -> Result<()> {
    use spark::{SparkSigner, SparkWallet, WalletConfig, Network};

    println!("{}", "Receive Payment".cyan().bold());
    println!();

    // Check if wallet exists
    if !SecureKeychain::has_mnemonic() {
        anyhow::bail!("No wallet found. Use 'wallet init' to create one.");
    }

    // Load identity and create spark wallet
    let mnemonic_phrase = SecureKeychain::retrieve_mnemonic()?;
    let mnemonic = Mnemonic::parse(&mnemonic_phrase)?;

    let signer = SparkSigner::from_mnemonic(&mnemonic.to_string(), "")?;
    let config = WalletConfig {
        network: Network::Testnet,
        ..Default::default()
    };

    // Create wallet and get address (async)
    let rt = tokio::runtime::Runtime::new()?;
    let wallet = rt.block_on(SparkWallet::new(signer, config))?;
    let address = wallet.get_spark_address();

    if let Some(amt) = amount {
        println!("{}: {} sats", "Amount".bold(), amt);
        println!();
    }

    println!("{}: {}", "Address (Public Key)".bold(), address);
    println!();
    println!("{}", "WARNING: This is a temporary public key representation, not a proper Spark address.".yellow());
    println!("{}", "Full Spark address generation requires Breez SDK integration (directive d-001).".yellow());

    Ok(())
}

pub fn send(_address: String, _amount: u64) -> Result<()> {
    anyhow::bail!("Send payments require Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn invoice(_amount: u64, _description: Option<String>) -> Result<()> {
    anyhow::bail!("Lightning invoice generation requires Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn pay(_invoice: String) -> Result<()> {
    anyhow::bail!("Lightning invoice payment requires Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn history(_limit: usize) -> Result<()> {
    anyhow::bail!("Transaction history requires Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn deposit() -> Result<()> {
    anyhow::bail!("On-chain deposits require Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn withdraw(_address: String, _amount: u64) -> Result<()> {
    anyhow::bail!("On-chain withdrawals require Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn zap(_note_id: String, _amount: u64) -> Result<()> {
    anyhow::bail!("Zap payments require Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn zaps(_note_id: String) -> Result<()> {
    anyhow::bail!("Zap queries require Nostr relay integration. See directive d-002 for implementation.")
}

pub fn nwc_create(_name: Option<String>) -> Result<()> {
    anyhow::bail!("NIP-47 Nostr Wallet Connect requires Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn nwc_list() -> Result<()> {
    anyhow::bail!("NIP-47 Nostr Wallet Connect requires Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn nwc_revoke(_id: String) -> Result<()> {
    anyhow::bail!("NIP-47 Nostr Wallet Connect requires Breez SDK integration. See directive d-001 for integration roadmap.")
}
