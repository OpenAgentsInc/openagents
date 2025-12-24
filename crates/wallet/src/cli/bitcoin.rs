//! Bitcoin/Lightning CLI commands using Spark SDK
//!
//! These commands use the Spark SDK for Bitcoin/Lightning payments.
//! The wallet mnemonic is loaded from the OS keychain.

use anyhow::{Context, Result};
use colored::Colorize;
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

    // Use testnet by default - can be configured later
    let config = WalletConfig {
        network: Network::Testnet,
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

        println!("{}", "Wallet Balance".bold().green());
        println!("────────────────────────────");
        println!("  Spark:     {} sats", balance.spark_sats.to_string().yellow());
        println!("  Lightning: {} sats", balance.lightning_sats);
        println!("  On-chain:  {} sats", balance.onchain_sats);
        println!("────────────────────────────");
        println!("  Total:     {} sats", balance.total_sats().to_string().bold());

        Ok(())
    })
}

/// Query detailed wallet balance with pending amounts
pub fn balance_detailed() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;
        let balance = wallet.get_balance().await?;
        let address = wallet.get_spark_address().await?;

        println!("{}", "Detailed Wallet Status".bold().green());
        println!("════════════════════════════════════════");
        println!();
        println!("{}", "Address".bold());
        println!("  Spark: {}", address);
        println!();
        println!("{}", "Balances".bold());
        println!("  Spark Layer 2: {} sats", balance.spark_sats.to_string().yellow());
        println!("  Lightning:     {} sats", balance.lightning_sats);
        println!("  On-chain:      {} sats", balance.onchain_sats);
        println!("────────────────────────────────────────");
        println!("  Total:         {} sats", balance.total_sats().to_string().bold());
        println!();
        println!("{}", "Network".bold());
        println!("  Connected to: {:?}", wallet.config().network);

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
                println!("{}", "Lightning Invoice Created".bold().green());
                println!("────────────────────────────────────────");
                println!("Amount: {} sats", sats.to_string().yellow());
                println!();
                println!("{}", "Invoice:".bold());
                println!("{}", response.payment_request);
            }
            None => {
                // Get static Spark address
                let address = wallet.get_spark_address().await?;
                println!("{}", "Spark Address".bold().green());
                println!("────────────────────────────────────────");
                println!("Send any amount to this address:");
                println!();
                println!("{}", address.yellow());
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

        println!("{}", "Sending Payment...".bold().yellow());
        println!("  To: {}", destination);
        println!("  Amount: {} sats", amount);
        println!();

        // Prepare and send payment
        let response = wallet.send_payment_simple(&destination, Some(amount)).await?;

        println!("{}", "Payment Sent!".bold().green());
        println!("────────────────────────────────────────");
        println!("  Payment ID: {}", response.payment.id);
        println!("  Status: {:?}", response.payment.status);

        Ok(())
    })
}

/// Create a Lightning invoice
pub fn invoice(amount: u64, description: Option<String>) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;

        let response = wallet.create_invoice(amount, description.clone(), None).await?;

        println!("{}", "Invoice Created".bold().green());
        println!("────────────────────────────────────────");
        println!("Amount: {} sats", amount.to_string().yellow());
        if let Some(desc) = description {
            println!("Description: {}", desc);
        }
        println!();
        println!("{}", "Invoice:".bold());
        println!("{}", response.payment_request);

        Ok(())
    })
}

/// Pay a Lightning invoice
pub fn pay(invoice_str: String) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;

        println!("{}", "Paying Invoice...".bold().yellow());

        // Pay the invoice (amount is encoded in invoice)
        let response = wallet.send_payment_simple(&invoice_str, None).await?;

        println!("{}", "Payment Sent!".bold().green());
        println!("────────────────────────────────────────");
        println!("  Payment ID: {}", response.payment.id);
        println!("  Status: {:?}", response.payment.status);

        Ok(())
    })
}

/// Show transaction history
pub fn history(limit: usize) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let _wallet = get_wallet().await?;

        println!("{}", "Transaction History".bold().green());
        println!("════════════════════════════════════════");
        println!();
        // TODO: List transactions requires SDK integration
        // The Breez SDK has list_payments() method
        println!("  Showing last {} transactions:", limit);
        println!("  (Transaction listing requires additional SDK integration)");

        Ok(())
    })
}

/// Get on-chain deposit address
pub fn deposit() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;

        let address = wallet.get_spark_address().await?;

        println!("{}", "Deposit Address".bold().green());
        println!("────────────────────────────────────────");
        println!("Send Bitcoin to this Spark address:");
        println!();
        println!("{}", address.yellow());
        println!();
        println!("{}","Note: Funds will be available in your Spark Layer 2 balance.".dimmed());

        Ok(())
    })
}

/// Withdraw to on-chain Bitcoin address
pub fn withdraw(address: String, amount: u64) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;

        println!("{}", "Withdrawing to On-Chain...".bold().yellow());
        println!("  To: {}", address);
        println!("  Amount: {} sats", amount);
        println!();

        // Use send_payment_simple - Spark SDK handles on-chain vs Lightning routing
        let response = wallet.send_payment_simple(&address, Some(amount)).await?;

        println!("{}", "Withdrawal Initiated!".bold().green());
        println!("────────────────────────────────────────");
        println!("  Payment ID: {}", response.payment.id);
        println!("  Status: {:?}", response.payment.status);

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
