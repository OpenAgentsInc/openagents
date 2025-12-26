//! Bitcoin/Lightning CLI commands using Spark SDK
//!
//! Provides wallet commands for balance, send, receive using the Breez Spark SDK.

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono;
use spark::{EventListener, Network, SdkEvent, SparkSigner, SparkWallet, WalletConfig};
use std::path::{Path, PathBuf};
use crate::cli::load_mnemonic;
use crate::storage::address_book::AddressBook;

const CLIPBOARD_FILE_ENV: &str = "OPENAGENTS_CLIPBOARD_FILE";
const NOTIFICATION_FILE_ENV: &str = "OPENAGENTS_NOTIFICATION_FILE";

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
pub fn receive(amount: Option<u64>, show_qr: bool, copy: bool, expiry: Option<u64>) -> Result<()> {
    if amount.is_none() && expiry.is_some() {
        anyhow::bail!("--expiry requires --amount to create an invoice.");
    }

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;

        match amount {
            Some(sats) => {
                // Create invoice for specific amount
                let response = wallet.create_invoice(sats, None, expiry).await?;
                let mut output = format_receive_invoice(sats, &response.payment_request, show_qr)?;
                if copy {
                    copy_to_clipboard(&response.payment_request)?;
                    output.push_str("Copied invoice to clipboard.\n");
                }
                print!("{}", output);
            }
            None => {
                // Get static Spark address
                let address = wallet.get_spark_address().await?;
                let mut output = format_receive_address(&address, show_qr)?;
                if copy {
                    copy_to_clipboard(&address)?;
                    output.push_str("Copied address to clipboard.\n");
                }
                print!("{}", output);
            }
        }

        Ok(())
    })
}

/// Listen for incoming payments and show notifications
pub fn notify() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let listener_id = wallet
            .add_event_listener(Box::new(PaymentNotificationListener { sender: tx }))
            .await?;

        println!("Listening for incoming payments. Press Ctrl+C to stop.");

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    println!("Stopping payment notifications.");
                    break;
                }
                maybe_payment = rx.recv() => {
                    let Some(payment) = maybe_payment else {
                        break;
                    };
                    if let Some(message) = payment_notification_message(&payment) {
                        println!("{}", message);
                        let _ = send_notification("Payment received", &message)?;
                    }
                }
            }
        }

        let _ = wallet.remove_event_listener(&listener_id).await?;
        Ok(())
    })
}

struct PaymentNotificationListener {
    sender: tokio::sync::mpsc::UnboundedSender<spark::Payment>,
}

#[async_trait]
impl EventListener for PaymentNotificationListener {
    async fn on_event(&self, event: SdkEvent) {
        if let SdkEvent::PaymentSucceeded { payment } = event {
            let _ = self.sender.send(payment);
        }
    }
}

fn format_receive_invoice(amount: u64, invoice: &str, show_qr: bool) -> Result<String> {
    let mut output = String::new();
    output.push_str("Lightning Invoice Created\n");
    output.push_str("────────────────────────────────────────\n");
    output.push_str(&format!("Amount: {} sats\n", amount));
    output.push('\n');
    output.push_str("Invoice:\n");
    output.push_str(invoice);
    output.push('\n');

    if show_qr {
        output.push('\n');
        output.push_str("QR Code:\n");
        output.push_str(&generate_qr_ascii(invoice)?);
        output.push('\n');
    }

    Ok(output)
}

fn format_receive_address(address: &str, show_qr: bool) -> Result<String> {
    let mut output = String::new();
    output.push_str("Reusable Spark Address\n");
    output.push_str("────────────────────────────────────────\n");
    output.push_str("Send any amount to this reusable address:\n");
    output.push('\n');
    output.push_str(address);
    output.push('\n');

    if show_qr {
        output.push('\n');
        output.push_str("QR Code:\n");
        output.push_str(&generate_qr_ascii(address)?);
        output.push('\n');
    }

    Ok(output)
}

fn payment_notification_message(payment: &spark::Payment) -> Option<String> {
    if payment.payment_type != spark::PaymentType::Receive
        || payment.status != spark::PaymentStatus::Completed
    {
        return None;
    }

    let method = format_payment_method(payment.method);
    Some(format!(
        "Received {} sats via {}",
        payment.amount, method
    ))
}

fn format_payment_method(method: spark::PaymentMethod) -> &'static str {
    match method {
        spark::PaymentMethod::Lightning => "Lightning",
        spark::PaymentMethod::Spark => "Spark",
        spark::PaymentMethod::Token => "Token",
        spark::PaymentMethod::Deposit => "Deposit",
        spark::PaymentMethod::Withdraw => "Withdraw",
        spark::PaymentMethod::Unknown => "Unknown",
    }
}

fn generate_qr_ascii(payload: &str) -> Result<String> {
    let code = qrcode::QrCode::new(payload.as_bytes())
        .context("Failed to generate QR code")?;
    let width = code.width();
    let border = 2usize;
    let mut output = String::new();

    for y in 0..(width + border * 2) {
        for x in 0..(width + border * 2) {
            let module_is_dark = if x < border || y < border || x >= width + border || y >= width + border {
                false
            } else {
                code[(x - border, y - border)] == qrcode::types::Color::Dark
            };
            if module_is_dark {
                output.push_str("##");
            } else {
                output.push_str("  ");
            }
        }
        output.push('\n');
    }

    Ok(output)
}

fn copy_to_clipboard(contents: &str) -> Result<()> {
    if let Ok(path) = std::env::var(CLIPBOARD_FILE_ENV) {
        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write clipboard contents to {}", path))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        return copy_with_command("pbcopy", &[], contents);
    }

    #[cfg(target_os = "linux")]
    {
        if copy_with_command_if_exists("wl-copy", &[], contents)? {
            return Ok(());
        }
        if copy_with_command_if_exists("xclip", &["-selection", "clipboard"], contents)? {
            return Ok(());
        }
        if copy_with_command_if_exists("xsel", &["--clipboard", "--input"], contents)? {
            return Ok(());
        }

        anyhow::bail!("Clipboard copy failed. Install wl-copy or xclip.");
    }

    #[cfg(target_os = "windows")]
    {
        return copy_with_command("cmd", &["/C", "clip"], contents);
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        anyhow::bail!("Clipboard copy is not supported on this platform.");
    }
}

fn copy_with_command(command: &str, args: &[&str], contents: &str) -> Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to start {}", command))?;

    let Some(stdin) = child.stdin.as_mut() else {
        anyhow::bail!("Failed to open stdin for {}", command);
    };
    stdin
        .write_all(contents.as_bytes())
        .with_context(|| format!("Failed to write to {}", command))?;

    let status = child
        .wait()
        .with_context(|| format!("Failed to wait for {}", command))?;
    if !status.success() {
        anyhow::bail!("{} exited with {}", command, status);
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn copy_with_command_if_exists(
    command: &str,
    args: &[&str],
    contents: &str,
) -> Result<bool> {
    use std::io::{ErrorKind, Write};
    use std::process::{Command, Stdio};

    let mut child = match Command::new(command).args(args).stdin(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err.into()),
    };

    let Some(stdin) = child.stdin.as_mut() else {
        anyhow::bail!("Failed to open stdin for {}", command);
    };
    stdin
        .write_all(contents.as_bytes())
        .with_context(|| format!("Failed to write to {}", command))?;

    let status = child
        .wait()
        .with_context(|| format!("Failed to wait for {}", command))?;
    if !status.success() {
        anyhow::bail!("{} exited with {}", command, status);
    }

    Ok(true)
}

fn send_notification(title: &str, message: &str) -> Result<bool> {
    if let Ok(path) = std::env::var(NOTIFICATION_FILE_ENV) {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("Failed to open notification log {}", path))?;
        use std::io::Write;
        writeln!(file, "{}\t{}", title, message)
            .with_context(|| format!("Failed to write notification log {}", path))?;
        return Ok(true);
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            escape_osascript(message),
            escape_osascript(title),
        );
        return notify_with_command_if_exists("osascript", &["-e", &script]);
    }

    #[cfg(target_os = "linux")]
    {
        return notify_with_command_if_exists("notify-send", &[title, message]);
    }

    #[cfg(target_os = "windows")]
    {
        return Ok(false);
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Ok(false)
    }
}

fn notify_with_command_if_exists(command: &str, args: &[&str]) -> Result<bool> {
    use std::io::ErrorKind;
    use std::process::Command;

    let status = match Command::new(command).args(args).status() {
        Ok(status) => status,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err.into()),
    };

    if status.success() {
        Ok(true)
    } else {
        anyhow::bail!("{} exited with {}", command, status);
    }
}

#[cfg(target_os = "macos")]
fn escape_osascript(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Send payment to address or pay invoice
pub fn send(
    destination: String,
    amount: u64,
    yes: bool,
    qr: Option<PathBuf>,
    payee: Option<String>,
) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;

        let destination = resolve_send_destination(destination, qr, payee)?;

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
    use std::io::{self, IsTerminal};

    let is_terminal = io::stdin().is_terminal();
    let mut stdin = io::stdin();
    let mut reader = io::BufReader::new(&mut stdin);
    confirm_send_with_reader(skip_confirm, is_terminal, &mut reader)
}

fn confirm_send_with_reader<R: std::io::BufRead>(
    skip_confirm: bool,
    is_terminal: bool,
    reader: &mut R,
) -> Result<bool> {
    use std::io::{self, Write};

    if skip_confirm {
        return Ok(true);
    }

    if !is_terminal {
        anyhow::bail!("Non-interactive send requires --yes to confirm.");
    }

    print!("Confirm payment? [y/N]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    reader.read_line(&mut input)?;
    let trimmed = input.trim();
    Ok(trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes"))
}

fn resolve_send_destination(
    destination: String,
    qr: Option<PathBuf>,
    payee: Option<String>,
) -> Result<String> {
    if qr.is_some() && payee.is_some() {
        anyhow::bail!("Use either --qr or --payee, not both.");
    }

    if let Some(name) = payee {
        let book = AddressBook::load()?;
        let entry = book
            .find(&name)
            .with_context(|| format!("Payee '{}' not found.", name))?;
        return Ok(entry.address.clone());
    }

    if let Some(path) = qr {
        return decode_qr_from_path(&path);
    }

    if destination.trim() == "-" {
        anyhow::bail!("Provide an address, --payee, or --qr.");
    }

    Ok(destination)
}

fn decode_qr_from_path(path: &Path) -> Result<String> {
    let image = image::open(path)
        .with_context(|| format!("Failed to open QR image {}", path.display()))?;
    let gray = image.to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(gray);
    let grids = prepared.detect_grids();

    for grid in grids {
        let (_meta, content) = grid
            .decode()
            .context("Failed to decode QR code")?;
        return Ok(content);
    }

    anyhow::bail!("No QR code found in {}", path.display());
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
    use crate::storage::address_book::{AddressBook, ADDRESS_BOOK_ENV};
    use spark::wallet::{
        Bolt11Invoice, Bolt11InvoiceDetails, BitcoinAddressDetails, BitcoinNetwork,
        PaymentMethod, PaymentRequestSource, PrepareSendPaymentResponse, SendOnchainFeeQuote,
        SendOnchainSpeedFeeQuote, SendPaymentMethod,
    };
    use std::sync::Mutex;

    static CLIPBOARD_ENV_LOCK: Mutex<()> = Mutex::new(());
    static ADDRESS_BOOK_ENV_LOCK: Mutex<()> = Mutex::new(());
    static NOTIFICATION_ENV_LOCK: Mutex<()> = Mutex::new(());

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

    fn sample_receive_payment(status: spark::PaymentStatus) -> spark::Payment {
        spark::Payment {
            id: "recv-1".to_string(),
            payment_type: spark::PaymentType::Receive,
            status,
            amount: 84,
            fees: 0,
            timestamp: 1_700_000_000,
            method: PaymentMethod::Spark,
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
    fn test_confirm_send_declines_on_no() {
        let mut input = std::io::Cursor::new("n\n");
        let confirmed = confirm_send_with_reader(false, true, &mut input).unwrap();
        assert!(!confirmed);
    }

    #[test]
    fn test_payment_notification_message_for_receive_completed() {
        let payment = sample_receive_payment(spark::PaymentStatus::Completed);
        let message = payment_notification_message(&payment).unwrap();
        assert!(message.contains("Received 84"));
        assert!(message.contains("Spark"));

        let pending = sample_receive_payment(spark::PaymentStatus::Pending);
        assert!(payment_notification_message(&pending).is_none());

        let sent = sample_payment(spark::PaymentStatus::Completed);
        assert!(payment_notification_message(&sent).is_none());
    }

    #[test]
    fn test_send_notification_file_env() {
        let _guard = NOTIFICATION_ENV_LOCK.lock().unwrap();
        let temp = tempfile::NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();
        let original = std::env::var(NOTIFICATION_FILE_ENV).ok();

        unsafe {
            std::env::set_var(NOTIFICATION_FILE_ENV, &path);
        }

        let delivered = send_notification("Payment received", "100 sats").unwrap();
        assert!(delivered);

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("Payment received\t100 sats"));

        if let Some(value) = original {
            unsafe {
                std::env::set_var(NOTIFICATION_FILE_ENV, value);
            }
        } else {
            unsafe {
                std::env::remove_var(NOTIFICATION_FILE_ENV);
            }
        }
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

    #[test]
    fn test_format_receive_invoice_with_qr() {
        let output = format_receive_invoice(1500, "lnbc1invoice", true).unwrap();
        assert!(output.contains("Lightning Invoice Created"));
        assert!(output.contains("Amount: 1500 sats"));
        assert!(output.contains("lnbc1invoice"));
        assert!(output.contains("QR Code:"));
        assert!(output.contains("##"));
    }

    #[test]
    fn test_format_receive_address_without_qr() {
        let output = format_receive_address("spark1address", false).unwrap();
        assert!(output.contains("Reusable Spark Address"));
        assert!(output.contains("spark1address"));
        assert!(!output.contains("QR Code:"));
    }

    #[test]
    fn test_copy_to_clipboard_file_env() {
        let _guard = CLIPBOARD_ENV_LOCK.lock().unwrap();
        let temp = tempfile::NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();
        let original = std::env::var(CLIPBOARD_FILE_ENV).ok();

        unsafe {
            std::env::set_var(CLIPBOARD_FILE_ENV, &path);
        }
        copy_to_clipboard("clipboard-test").unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "clipboard-test");

        if let Some(value) = original {
            unsafe {
                std::env::set_var(CLIPBOARD_FILE_ENV, value);
            }
        } else {
            unsafe {
                std::env::remove_var(CLIPBOARD_FILE_ENV);
            }
        }
    }

    #[test]
    fn test_decode_qr_from_path_roundtrip() {
        let payload = "lnbc1qrtest";
        let code = qrcode::QrCode::new(payload.as_bytes()).unwrap();
        let width = code.width() as u32;
        let border = 4u32;
        let scale = 4u32;
        let size = (width + border * 2) * scale;
        let mut image = image::GrayImage::new(size, size);

        for y in 0..(width + border * 2) {
            for x in 0..(width + border * 2) {
                let is_dark = if x < border || y < border || x >= width + border || y >= width + border {
                    false
                } else {
                    code[(x as usize - border as usize, y as usize - border as usize)]
                        == qrcode::types::Color::Dark
                };
                let pixel = if is_dark { 0 } else { 255 };
                let start_x = x * scale;
                let start_y = y * scale;
                for dy in 0..scale {
                    for dx in 0..scale {
                        image.put_pixel(start_x + dx, start_y + dy, image::Luma([pixel]));
                    }
                }
            }
        }

        let temp = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        image.save(temp.path()).unwrap();

        let decoded = decode_qr_from_path(temp.path()).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn test_resolve_send_destination_from_payee() {
        let _guard = ADDRESS_BOOK_ENV_LOCK.lock().unwrap();
        let temp = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
        let original = std::env::var(ADDRESS_BOOK_ENV).ok();

        unsafe {
            std::env::set_var(ADDRESS_BOOK_ENV, temp.path());
        }

        let mut book = AddressBook::default();
        book.add("alice".to_string(), "lnbc1alice".to_string()).unwrap();
        book.save().unwrap();

        let destination = resolve_send_destination(
            "-".to_string(),
            None,
            Some("alice".to_string()),
        )
        .unwrap();
        assert_eq!(destination, "lnbc1alice");

        if let Some(value) = original {
            unsafe {
                std::env::set_var(ADDRESS_BOOK_ENV, value);
            }
        } else {
            unsafe {
                std::env::remove_var(ADDRESS_BOOK_ENV);
            }
        }
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
