//! Bitcoin/Lightning CLI commands using Spark SDK
//!
//! Provides wallet commands for balance, send, receive using the Breez Spark SDK.

use anyhow::{Context, Result};
use chrono;
use spark::{Network, SparkSigner, SparkWallet, WalletConfig};
use std::path::PathBuf;
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
pub fn send(destination: String, amount: u64, yes: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;

        println!("Preparing Payment...");
        println!();

        let prepare_response = wallet.prepare_send_payment(&destination, Some(amount)).await?;
        let preview = build_send_preview(&destination, &prepare_response);
        let preview_text = format_send_preview(&preview);
        print!("{}", preview_text);

        if !confirm_send(yes)? {
            println!("Payment cancelled.");
            return Ok(());
        }

        println!();
        println!("Sending Payment...");
        println!();

        // Prepare and send payment
        match wallet.send_payment(prepare_response, None).await {
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

struct SendPreview {
    destination: String,
    payment_kind: &'static str,
    amount_sats: u128,
    fee_lines: Vec<String>,
}

fn build_send_preview(destination: &str, prepare: &spark::wallet::PrepareSendPaymentResponse) -> SendPreview {
    let (payment_kind, fee_lines) = match &prepare.payment_method {
        spark::wallet::SendPaymentMethod::Bolt11Invoice {
            spark_transfer_fee_sats,
            lightning_fee_sats,
            ..
        } => {
            let mut lines = vec![format!("Lightning: {} sats", lightning_fee_sats)];
            if let Some(spark_fee) = spark_transfer_fee_sats {
                lines.push(format!("Spark transfer: {} sats", spark_fee));
            }
            ("Lightning Invoice", lines)
        }
        spark::wallet::SendPaymentMethod::BitcoinAddress { fee_quote, .. } => {
            let lines = vec![
                format!("Fast: {} sats", fee_quote.speed_fast.total_fee_sat()),
                format!("Medium: {} sats", fee_quote.speed_medium.total_fee_sat()),
                format!("Slow: {} sats", fee_quote.speed_slow.total_fee_sat()),
            ];
            ("On-chain Address", lines)
        }
        spark::wallet::SendPaymentMethod::SparkAddress { fee, .. } => {
            ("Spark Address", vec![format!("Spark: {} sats", fee)])
        }
        spark::wallet::SendPaymentMethod::SparkInvoice { fee, .. } => {
            ("Spark Invoice", vec![format!("Spark: {} sats", fee)])
        }
    };

    SendPreview {
        destination: destination.to_string(),
        payment_kind,
        amount_sats: prepare.amount,
        fee_lines,
    }
}

fn format_send_preview(preview: &SendPreview) -> String {
    let mut output = String::new();
    output.push_str("Send Payment Confirmation\n");
    output.push_str("────────────────────────────\n");
    output.push_str(&format!("  To:     {}\n", preview.destination));
    output.push_str(&format!("  Type:   {}\n", preview.payment_kind));
    output.push_str(&format!("  Amount: {} sats\n", preview.amount_sats));

    if preview.fee_lines.is_empty() {
        output.push_str("  Fees:   Unknown\n");
    } else if preview.fee_lines.len() == 1 {
        output.push_str(&format!("  Fees:   {}\n", preview.fee_lines[0]));
    } else {
        output.push_str("  Fees:\n");
        for line in &preview.fee_lines {
            output.push_str(&format!("    {}\n", line));
        }
    }

    output.push('\n');
    output
}

fn confirm_send(skip_confirm: bool) -> Result<bool> {
    use std::io::{self, Write};
    use std::io::IsTerminal;

    if skip_confirm {
        return Ok(true);
    }

    if !io::stdin().is_terminal() {
        anyhow::bail!("Non-interactive send requires --yes to confirm.");
    }

    print!("Confirm payment? [y/N]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();
    Ok(trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes"))
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryFormat {
    Table,
    Csv,
}

impl HistoryFormat {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "table" => Ok(Self::Table),
            "csv" => Ok(Self::Csv),
            _ => anyhow::bail!("Invalid history format: {}. Use 'table' or 'csv'.", value),
        }
    }
}

pub fn history(limit: usize, format: HistoryFormat, output: Option<PathBuf>) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;

        let payments = wallet.list_payments(Some(limit as u32), None).await?;

        match format {
            HistoryFormat::Table => {
                if output.is_some() {
                    anyhow::bail!("--output is only supported with --format csv.");
                }
                let table = format_history_table(&payments);
                print!("{}", table);
            }
            HistoryFormat::Csv => {
                let csv = format_history_csv(&payments);
                if let Some(path) = output {
                    std::fs::write(&path, csv)
                        .with_context(|| format!("Failed to write CSV history to {}", path.display()))?;
                    println!("Saved CSV history to {}", path.display());
                } else {
                    print!("{}", csv);
                }
            }
        }

        Ok(())
    })
}

fn format_payment_type(payment_type: spark::PaymentType) -> &'static str {
    match payment_type {
        spark::PaymentType::Send => "SENT",
        spark::PaymentType::Receive => "RECV",
    }
}

fn format_payment_status(status: spark::PaymentStatus) -> &'static str {
    match status {
        spark::PaymentStatus::Completed => "✓ Done",
        spark::PaymentStatus::Pending => "⏳ Pending",
        spark::PaymentStatus::Failed => "✗ Failed",
    }
}

fn format_payment_status_csv(status: spark::PaymentStatus) -> &'static str {
    match status {
        spark::PaymentStatus::Completed => "completed",
        spark::PaymentStatus::Pending => "pending",
        spark::PaymentStatus::Failed => "failed",
    }
}

fn format_timestamp_display(timestamp: u64) -> String {
    chrono::DateTime::from_timestamp(timestamp as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn format_timestamp_csv(timestamp: u64) -> String {
    chrono::DateTime::from_timestamp(timestamp as i64, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn format_history_table(payments: &[spark::Payment]) -> String {
    let mut output = String::new();
    output.push_str("Transaction History\n");
    output.push_str("════════════════════════════════════════════════════════════════════════\n");
    output.push('\n');

    if payments.is_empty() {
        output.push_str("  No transactions yet.\n");
        return output;
    }

    output.push_str(&format!(
        "  {:<8} {:<10} {:>12} {:>10} {:<19}\n",
        "Type", "Status", "Amount", "Fee", "Date"
    ));
    output.push_str(&format!("  {}\n", "─".repeat(68)));

    for payment in payments {
        let type_str = format_payment_type(payment.payment_type);
        let status_str = format_payment_status(payment.status);
        let amount_str = format!("{} sats", payment.amount);
        let fee_str = if payment.fees > 0 {
            format!("{} sats", payment.fees)
        } else {
            "-".to_string()
        };
        let datetime = format_timestamp_display(payment.timestamp);

        output.push_str(&format!(
            "  {:<8} {:<10} {:>12} {:>10} {:<19}\n",
            type_str, status_str, amount_str, fee_str, datetime
        ));
    }

    output.push('\n');
    output.push_str(&format!(
        "  Showing {} most recent transaction(s)\n",
        payments.len()
    ));
    output
}

fn format_history_csv(payments: &[spark::Payment]) -> String {
    let mut csv = String::from("id,type,status,amount_sats,fees_sats,timestamp\n");
    for payment in payments {
        let timestamp = format_timestamp_csv(payment.timestamp);
        csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            payment.id,
            format_payment_type(payment.payment_type),
            format_payment_status_csv(payment.status),
            payment.amount,
            payment.fees,
            timestamp
        ));
    }
    csv
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
    use spark::wallet::{
        Bolt11Invoice, Bolt11InvoiceDetails, BitcoinAddressDetails, BitcoinNetwork,
        PaymentMethod, PaymentRequestSource, PrepareSendPaymentResponse, SendOnchainFeeQuote,
        SendOnchainSpeedFeeQuote, SendPaymentMethod,
    };

    fn sample_payment(status: spark::PaymentStatus) -> spark::Payment {
        spark::Payment {
            id: "pay-1".to_string(),
            payment_type: spark::PaymentType::Send,
            status,
            amount: 42,
            fees: 3,
            timestamp: 1_700_000_000,
            method: PaymentMethod::Lightning,
            details: None,
        }
    }

    fn sample_bolt11_details() -> Bolt11InvoiceDetails {
        Bolt11InvoiceDetails {
            amount_msat: Some(1_000_000),
            description: Some("Test invoice".to_string()),
            description_hash: None,
            expiry: 3600,
            invoice: Bolt11Invoice {
                bolt11: "lnbc1testinvoice".to_string(),
                source: PaymentRequestSource::default(),
            },
            min_final_cltv_expiry_delta: 18,
            network: BitcoinNetwork::Regtest,
            payee_pubkey: "02deadbeef".to_string(),
            payment_hash: "hash".to_string(),
            payment_secret: "secret".to_string(),
            routing_hints: Vec::new(),
            timestamp: 1_700_000_000,
        }
    }

    fn sample_onchain_fee_quote() -> SendOnchainFeeQuote {
        SendOnchainFeeQuote {
            id: "fee-1".to_string(),
            expires_at: 0,
            speed_fast: SendOnchainSpeedFeeQuote {
                user_fee_sat: 10,
                l1_broadcast_fee_sat: 5,
            },
            speed_medium: SendOnchainSpeedFeeQuote {
                user_fee_sat: 6,
                l1_broadcast_fee_sat: 4,
            },
            speed_slow: SendOnchainSpeedFeeQuote {
                user_fee_sat: 3,
                l1_broadcast_fee_sat: 2,
            },
        }
    }

    fn prepare_response(method: SendPaymentMethod) -> PrepareSendPaymentResponse {
        PrepareSendPaymentResponse {
            payment_method: method,
            amount: 2_500,
            token_identifier: None,
        }
    }

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

    #[test]
    fn test_format_history_table_marks_pending() {
        let payment = sample_payment(spark::PaymentStatus::Pending);
        let output = format_history_table(&[payment]);
        assert!(output.contains("⏳ Pending"));
    }

    #[test]
    fn test_format_history_csv_includes_rows() {
        let payment = sample_payment(spark::PaymentStatus::Pending);
        let csv = format_history_csv(&[payment]);
        assert!(csv.starts_with("id,type,status,amount_sats,fees_sats,timestamp"));
        assert!(csv.contains("pay-1,SENT,pending,42,3,"));
    }

    #[test]
    fn test_send_preview_lightning_invoice() {
        let prepare = prepare_response(SendPaymentMethod::Bolt11Invoice {
            invoice_details: sample_bolt11_details(),
            spark_transfer_fee_sats: Some(2),
            lightning_fee_sats: 7,
        });
        let preview = build_send_preview("lnbc1invoice", &prepare);
        let output = format_send_preview(&preview);
        assert!(output.contains("Send Payment Confirmation"));
        assert!(output.contains("Lightning Invoice"));
        assert!(output.contains("Lightning: 7 sats"));
        assert!(output.contains("Spark transfer: 2 sats"));
    }

    #[test]
    fn test_send_preview_onchain_fees() {
        let prepare = prepare_response(SendPaymentMethod::BitcoinAddress {
            address: BitcoinAddressDetails {
                address: "bcrt1qaddress".to_string(),
                network: BitcoinNetwork::Regtest,
                source: PaymentRequestSource::default(),
            },
            fee_quote: sample_onchain_fee_quote(),
        });
        let preview = build_send_preview("bcrt1qaddress", &prepare);
        let output = format_send_preview(&preview);
        assert!(output.contains("On-chain Address"));
        assert!(output.contains("Fast: 15 sats"));
        assert!(output.contains("Medium: 10 sats"));
        assert!(output.contains("Slow: 5 sats"));
    }

    #[test]
    fn test_send_preview_spark_address() {
        let prepare = prepare_response(SendPaymentMethod::SparkAddress {
            address: "spark1test".to_string(),
            fee: 9,
            token_identifier: None,
        });
        let preview = build_send_preview("spark1test", &prepare);
        let output = format_send_preview(&preview);
        assert!(output.contains("Spark Address"));
        assert!(output.contains("Spark: 9 sats"));
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
