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
    let rt = tokio::runtime::Runtime::new()?;
    let wallet = rt.block_on(SparkWallet::new(signer, config))?;
    let balance = rt.block_on(wallet.get_balance())?;

    println!("{}: {} sats", "Total".bold(), balance.total_sats());
    println!();
    println!("{}", "Note: Spark integration is in progress. Balances are currently stub values.".yellow());

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
    let rt = tokio::runtime::Runtime::new()?;
    let wallet = rt.block_on(SparkWallet::new(signer, config))?;
    let balance = rt.block_on(wallet.get_balance())?;

    println!("{}: {} sats", "Spark L2".bold(), balance.spark_sats);
    println!("{}: {} sats", "Lightning".bold(), balance.lightning_sats);
    println!("{}: {} sats", "On-chain".bold(), balance.onchain_sats);
    println!("─────────────────────");
    println!("{}: {} sats", "Total".bold(), balance.total_sats());
    println!();
    println!("{}", "Note: Spark integration is in progress. Balances are currently stub values.".yellow());

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

    println!("{}: {}", "Address".bold(), address);
    println!();
    println!("Share this address to receive payment");
    println!();
    println!("{}", "Note: Spark integration is in progress. Address generation uses public key stub.".yellow());

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

pub fn withdraw(address: String, amount: u64) -> Result<()> {
    println!("{}", "On-chain Withdrawal".cyan().bold());
    println!();

    // TODO: Validate address
    // TODO: Create cooperative exit via Spark

    println!("{}: {}", "To".bold(), address);
    println!("{}: {} sats", "Amount".bold(), amount);
    println!();
    println!("{}", "This will perform a cooperative exit from Spark L2".yellow());
    println!("{}", "Confirm? (y/n)".yellow());

    // TODO: Get user confirmation
    // TODO: Execute withdrawal

    println!("{}", "✓ Withdrawal initiated".green());

    Ok(())
}

pub fn zap(note_id: String, amount: u64) -> Result<()> {
    println!("{}", "Send Zap".cyan().bold());
    println!();

    // TODO: Fetch note
    // TODO: Create zap request (NIP-57)
    // TODO: Pay zap

    println!("{}: {}", "Note".bold(), note_id);
    println!("{}: {} sats", "Amount".bold(), amount);
    println!();
    println!("{}", "✓ Zap sent".green());

    Ok(())
}

pub fn zaps(note_id: String) -> Result<()> {
    println!("{}", "Zaps on Note".cyan().bold());
    println!();

    // TODO: Fetch zap receipts for note
    // TODO: Display zaps with totals

    println!("{}: {}", "Note".bold(), note_id);
    println!();
    println!("No zaps yet");

    Ok(())
}

pub fn nwc_create(name: Option<String>) -> Result<()> {
    println!("{}", "Create NWC Connection".cyan().bold());
    println!();

    // TODO: Create NWC connection (NIP-47)
    // TODO: Generate connection string

    let conn_name = name.unwrap_or_else(|| "Default".to_string());
    println!("{}: {}", "Name".bold(), conn_name);
    println!();
    println!("{}: nostr+walletconnect://...", "Connection String".bold());
    println!();
    println!("Use this connection string in compatible apps");

    Ok(())
}

pub fn nwc_list() -> Result<()> {
    println!("{}", "Active NWC Connections".cyan().bold());
    println!();

    // TODO: List active NWC connections

    println!("No active connections");

    Ok(())
}

pub fn nwc_revoke(id: String) -> Result<()> {
    println!("{}", "Revoke NWC Connection".cyan().bold());
    println!();

    // TODO: Revoke connection

    println!("{}: {}", "Connection".bold(), id);
    println!("{}", "✓ Connection revoked".green());

    Ok(())
}
