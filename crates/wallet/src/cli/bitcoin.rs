//! Bitcoin/Lightning CLI commands using Spark SDK
//!
//! Provides wallet commands for balance, send, receive using the Breez Spark SDK.

use anyhow::{Context, Result};
use chrono;
use spark::{SparkSigner, SparkWallet, WalletConfig, Network};
use crate::cli::load_mnemonic;

/// Get or create the SparkWallet from keychain mnemonic
async fn get_wallet() -> Result<SparkWallet> {
    // Get mnemonic from keychain
    let mnemonic = load_mnemonic()?;

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

        let usd_rate = match fetch_btc_usd_rate().await {
            Ok(rate) => rate,
            Err(err) => {
                eprintln!("Warning: USD pricing unavailable: {}", err);
                None
            }
        };
        let output = format_balance_display(&balance, usd_rate);
        print!("{}", output);

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
        match wallet.send_payment_simple(&destination, Some(amount)).await {
            Ok(response) => {
                println!("✓ Payment Sent!");
                println!("────────────────────────────────────────");
                println!("  Payment ID: {}", response.payment.id);
                Ok(())
            }
            Err(e) => {
                eprintln!("✗ Payment Failed");
                eprintln!("────────────────────────────────────────");
                eprintln!("{}", e.user_friendly_message());
                if e.balance_unaffected() {
                    eprintln!();
                    eprintln!("ℹ️  Your balance was NOT deducted.");
                }
                Err(e.into())
            }
        }
    })
}

/// Pay a Lightning invoice
pub fn pay(invoice_str: String) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;

        println!("Paying Invoice...");

        // Pay the invoice (amount is encoded in invoice)
        match wallet.send_payment_simple(&invoice_str, None).await {
            Ok(response) => {
                println!("✓ Payment Sent!");
                println!("────────────────────────────────────────");
                println!("  Payment ID: {}", response.payment.id);
                Ok(())
            }
            Err(e) => {
                eprintln!("✗ Payment Failed");
                eprintln!("────────────────────────────────────────");
                eprintln!("{}", e.user_friendly_message());
                if e.balance_unaffected() {
                    eprintln!();
                    eprintln!("ℹ️  Your balance was NOT deducted.");
                }
                Err(e.into())
            }
        }
    })
}

/// Show transaction history
pub fn history(limit: usize) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;

        println!("Transaction History");
        println!("════════════════════════════════════════════════════════════════════════");
        println!();

        let payments = wallet.list_payments(Some(limit as u32), None).await?;

        if payments.is_empty() {
            println!("  No transactions yet.");
            return Ok(());
        }

        // Table header
        println!(
            "  {:<8} {:<10} {:>12} {:>10} {:<19}",
            "Type", "Status", "Amount", "Fee", "Date"
        );
        println!("  {}", "─".repeat(68));

        for payment in &payments {
            // Format payment type
            let type_str = match payment.payment_type {
                spark::PaymentType::Send => "SENT",
                spark::PaymentType::Receive => "RECV",
            };

            // Format status with color hints
            let status_str = match payment.status {
                spark::PaymentStatus::Completed => "✓ Done",
                spark::PaymentStatus::Pending => "⏳ Pending",
                spark::PaymentStatus::Failed => "✗ Failed",
            };

            // Format amount (u128 to u64 for display, show in sats)
            let amount_sats = payment.amount as u64;
            let amount_str = format!("{} sats", amount_sats);

            // Format fee
            let fee_sats = payment.fees as u64;
            let fee_str = if fee_sats > 0 {
                format!("{} sats", fee_sats)
            } else {
                "-".to_string()
            };

            // Format timestamp (Unix timestamp to readable date)
            let datetime = chrono::DateTime::from_timestamp(payment.timestamp as i64, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            println!(
                "  {:<8} {:<10} {:>12} {:>10} {:<19}",
                type_str, status_str, amount_str, fee_str, datetime
            );
        }

        println!();
        println!("  Showing {} most recent transaction(s)", payments.len());

        Ok(())
    })
}

async fn fetch_btc_usd_rate() -> Result<Option<f64>> {
    if let Ok(value) = std::env::var("OPENAGENTS_BTC_USD") {
        let rate = value
            .parse::<f64>()
            .context("Invalid OPENAGENTS_BTC_USD value")?;
        return Ok(Some(rate));
    }

    let response = reqwest::Client::new()
        .get("https://api.coinbase.com/v2/prices/BTC-USD/spot")
        .send()
        .await
        .context("Failed to fetch BTC/USD price")?;

    if !response.status().is_success() {
        anyhow::bail!("BTC/USD price request failed with status {}", response.status());
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse BTC/USD price response")?;

    let amount = payload["data"]["amount"]
        .as_str()
        .context("BTC/USD price missing from response")?;
    let rate = amount
        .parse::<f64>()
        .context("BTC/USD price is not a number")?;

    Ok(Some(rate))
}

fn format_balance_display(balance: &spark::Balance, usd_rate: Option<f64>) -> String {
    let mut output = String::new();
    output.push_str("Wallet Balance\n");
    output.push_str("────────────────────────────\n");
    output.push_str(&format!("  Spark:     {} sats\n", balance.spark_sats));
    output.push_str(&format!("  Lightning: {} sats\n", balance.lightning_sats));
    output.push_str(&format!("  On-chain:  {} sats\n", balance.onchain_sats));
    output.push_str("────────────────────────────\n");
    output.push_str(&format!("  Total:     {} sats", balance.total_sats()));

    if let Some(rate) = usd_rate {
        if rate.is_finite() && rate > 0.0 {
            let usd_total = sats_to_usd(balance.total_sats(), rate);
            output.push_str(&format!(" (${:.2})", usd_total));
        }
    }

    output.push('\n');
    output
}

fn sats_to_usd(sats: u64, usd_rate: f64) -> f64 {
    let btc = sats as f64 / 100_000_000.0;
    btc * usd_rate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_balance_includes_layer_breakdown() {
        let balance = spark::Balance {
            spark_sats: 10,
            lightning_sats: 20,
            onchain_sats: 30,
        };
        let output = format_balance_display(&balance, None);

        assert!(output.contains("Spark:     10 sats"));
        assert!(output.contains("Lightning: 20 sats"));
        assert!(output.contains("On-chain:  30 sats"));
        assert!(output.contains("Total:     60 sats"));
    }

    #[test]
    fn test_format_balance_includes_usd_total() {
        let balance = spark::Balance {
            spark_sats: 100_000_000,
            lightning_sats: 0,
            onchain_sats: 0,
        };
        let output = format_balance_display(&balance, Some(25_000.0));
        assert!(output.contains("Total:     100000000 sats"));
        assert!(output.contains("$25000.00"));
    }
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
