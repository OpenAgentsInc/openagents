//! Bitcoin/Lightning CLI commands using Spark SDK
//!
//! Provides wallet commands for balance, send, receive using the Breez Spark SDK.

use anyhow::{Context, Result};
use spark::{SparkSigner, SparkWallet, WalletConfig, Network};
use crate::storage::keychain::SecureKeychain;

/// Get or create the SparkWallet from keychain mnemonic
async fn get_wallet() -> Result<SparkWallet> {
    // Get mnemonic from keychain
    let mnemonic = SecureKeychain::retrieve_mnemonic()
        .context("No wallet found. Run 'openagents wallet init' first to create a wallet.")?;

    // Create signer from mnemonic
    let signer = SparkSigner::from_mnemonic(&mnemonic, "")
        .context("Failed to create signer from mnemonic")?;

    // Use regtest by default (no API key required)
    // For mainnet, set BREEZ_API_KEY env var
    let network = if std::env::var("MAINNET").is_ok() {
        Network::Mainnet
    } else {
        Network::Regtest
    };

    let config = WalletConfig {
        network,
        api_key: std::env::var("BREEZ_API_KEY").ok(),
        ..Default::default()
    };

    // Connect to Spark network
    SparkWallet::new(signer, config).await
        .context("Failed to connect to Spark network")
}

/// Query wallet balance
pub fn balance() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;
        let balance = wallet.get_balance().await?;

        println!("Wallet Balance");
        println!("────────────────────────────");
        println!("  Spark:     {} sats", balance.spark_sats);
        println!("  Lightning: {} sats", balance.lightning_sats);
        println!("  On-chain:  {} sats", balance.onchain_sats);
        println!("────────────────────────────");
        println!("  Total:     {} sats", balance.total_sats());

        Ok(())
    })
}

/// Generate a receive address or invoice
pub fn receive(amount: Option<u64>) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;

        match amount {
            Some(sats) => {
                // Create invoice for specific amount
                let response = wallet.create_invoice(sats, None, None).await?;
                println!("Lightning Invoice Created");
                println!("────────────────────────────────────────");
                println!("Amount: {} sats", sats);
                println!();
                println!("Invoice:");
                println!("{}", response.payment_request);
            }
            None => {
                // Get static Spark address
                let address = wallet.get_spark_address().await?;
                println!("Spark Address");
                println!("────────────────────────────────────────");
                println!("Send any amount to this address:");
                println!();
                println!("{}", address);
            }
        }

        Ok(())
    })
}

/// Send payment to address or pay invoice
pub fn send(destination: String, amount: u64) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;

        println!("Sending Payment...");
        println!("  To: {}", destination);
        println!("  Amount: {} sats", amount);
        println!();

        // Prepare and send payment
        let response = wallet.send_payment_simple(&destination, Some(amount)).await?;

        println!("Payment Sent!");
        println!("────────────────────────────────────────");
        println!("  Payment ID: {}", response.payment.id);

        Ok(())
    })
}

/// Pay a Lightning invoice
pub fn pay(invoice_str: String) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;

        println!("Paying Invoice...");

        // Pay the invoice (amount is encoded in invoice)
        let response = wallet.send_payment_simple(&invoice_str, None).await?;

        println!("Payment Sent!");
        println!("────────────────────────────────────────");
        println!("  Payment ID: {}", response.payment.id);

        Ok(())
    })
}

/// Show transaction history
pub fn history(_limit: usize) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let _wallet = get_wallet().await?;

        println!("Transaction History");
        println!("════════════════════════════════════════");
        println!();
        // TODO: List transactions requires list_payments() call
        println!("  (Transaction listing coming soon)");

        Ok(())
    })
}

/// Send a zap to a Nostr note
pub fn zap(note_id: String, amount: u64) -> Result<()> {
    // Zaps require looking up LNURL from note's author profile
    // This needs NIP-57 implementation
    anyhow::bail!(
        "Zap payments require NIP-57 implementation.\n\
        Note ID: {}\n\
        Amount: {} sats\n\n\
        To zap manually, use 'openagents wallet pay <invoice>' with the zap invoice.",
        note_id, amount
    )
}

/// Query zaps on a Nostr note
pub fn zaps(note_id: String) -> Result<()> {
    anyhow::bail!(
        "Zap queries require Nostr relay integration (d-002).\n\
        Note ID: {}",
        note_id
    )
}

/// Create a Nostr Wallet Connect connection
pub fn nwc_create(name: Option<String>) -> Result<()> {
    let connection_name = name.unwrap_or_else(|| "default".to_string());
    anyhow::bail!(
        "NIP-47 Nostr Wallet Connect requires additional implementation.\n\
        Connection name: {}\n\n\
        NWC allows external apps to request payments through your wallet.",
        connection_name
    )
}

/// List Nostr Wallet Connect connections
pub fn nwc_list() -> Result<()> {
    anyhow::bail!("NIP-47 Nostr Wallet Connect requires additional implementation.")
}

/// Revoke a Nostr Wallet Connect connection
pub fn nwc_revoke(id: String) -> Result<()> {
    anyhow::bail!(
        "NIP-47 Nostr Wallet Connect requires additional implementation.\n\
        Connection ID to revoke: {}",
        id
    )
}
