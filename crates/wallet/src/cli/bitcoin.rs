//! Bitcoin/Lightning CLI commands using Spark SDK

#![allow(dead_code)]

use super::error::{WalletError, format_error_with_hint};
use super::validation::{detect_and_validate_destination, validate_amount};
use crate::cli::load_mnemonic;
use crate::core::client::NostrClient;
use crate::core::identity::UnifiedIdentity;
use crate::core::nwc::{NwcService, build_connection, publish_info_event};
use crate::storage::address_book::AddressBook;
use crate::storage::config::WalletConfig as LocalWalletConfig;
use crate::storage::nwc::NwcConnectionStore;
use anyhow::{Context, Result};
use async_trait::async_trait;
use bech32::{Bech32, Hrp};
use bip39::Mnemonic;
use chrono;
use colored::Colorize;
use nostr::{
    Event, EventTemplate, Nip19Entity, ZAP_RECEIPT_KIND, ZAP_REQUEST_KIND, ZapReceipt,
    decode as decode_nip19,
};
use reqwest::{Client, Url};
use serde::Deserialize;
use spark::{EventListener, Network, SdkEvent, SparkSigner, SparkWallet, WalletConfig};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CLIPBOARD_FILE_ENV: &str = "OPENAGENTS_CLIPBOARD_FILE";
const NOTIFICATION_FILE_ENV: &str = "OPENAGENTS_NOTIFICATION_FILE";
const RETRY_PAGE_SIZE: u32 = 50;
const RETRY_MAX_PAGES: u32 = 20;
const NETWORK_STATUS_TIMEOUT_SECS: u64 = 5;

async fn get_wallet() -> Result<SparkWallet> {
    let mnemonic = load_mnemonic().map_err(|e| {
        let error = WalletError::WalletNotInitialized;
        eprintln!("{}", format_error_with_hint(&error));
        e
    })?;

    let signer = SparkSigner::from_mnemonic(&mnemonic, "").map_err(|e| {
        anyhow::anyhow!(
            "Failed to create signer from mnemonic: {}\n\n{}: Your recovery phrase may be corrupted. Try 'openagents wallet import' with your backup phrase.",
            e,
            "Hint".cyan()
        )
    })?;

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

    SparkWallet::new(signer, config).await.map_err(|e| {
        let error = WalletError::NetworkError(e.to_string());
        eprintln!("{}", format_error_with_hint(&error));
        anyhow::anyhow!("Failed to connect to Spark network: {}", e)
    })
}

pub fn balance() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;
        let balance = wallet.get_balance().await.map_err(|e| {
            let error = WalletError::NetworkError(format!("Failed to fetch balance: {}", e));
            eprintln!("{}", format_error_with_hint(&error));
            anyhow::anyhow!("{}", e)
        })?;

        let usd_rate = match fetch_btc_usd_rate().await {
            Ok(rate) => rate,
            Err(err) => {
                eprintln!("{} USD pricing unavailable: {}", "Warning:".yellow(), err);
                None
            }
        };
        let output = format_balance_display(&balance, usd_rate);
        print!("{}", output);

        Ok(())
    })
}

pub fn status() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;
        let report = wallet
            .network_status(std::time::Duration::from_secs(NETWORK_STATUS_TIMEOUT_SECS))
            .await;

        let output = format_network_status(report.clone(), wallet.config().network);
        print!("{}", output);

        if report.status != spark::NetworkStatus::Connected {
            eprintln!();
            eprintln!(
                "{}: If on mainnet, ensure BREEZ_API_KEY is set. Check your internet connection.",
                "Hint".cyan()
            );
        }

        Ok(())
    })
}

pub fn receive(amount: Option<u64>, show_qr: bool, copy: bool, expiry: Option<u64>) -> Result<()> {
    if amount.is_none() && expiry.is_some() {
        eprintln!(
            "{}: --expiry requires --amount to create an invoice.",
            "Error".red()
        );
        eprintln!();
        eprintln!(
            "{}: Use 'openagents wallet receive --amount 1000 --expiry 3600' for a 1-hour invoice.",
            "Hint".cyan()
        );
        anyhow::bail!("--expiry requires --amount to create an invoice.");
    }

    if let Some(sats) = amount {
        if let Err(e) = validate_amount(sats) {
            eprintln!("{}", format_error_with_hint(&e));
            return Err(e.into());
        }
    }

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;

        match amount {
            Some(sats) => {
                let response = wallet
                    .create_invoice(sats, None, expiry)
                    .await
                    .map_err(|e| {
                        let error =
                            WalletError::NetworkError(format!("Failed to create invoice: {}", e));
                        eprintln!("{}", format_error_with_hint(&error));
                        anyhow::anyhow!("{}", e)
                    })?;

                let mut output = format_receive_invoice(sats, &response.payment_request, show_qr)?;
                if copy {
                    copy_to_clipboard(&response.payment_request)?;
                    output.push_str(&format!("{} Copied invoice to clipboard.\n", "✓".green()));
                }
                print!("{}", output);
            }
            None => {
                let address = wallet.get_spark_address().await.map_err(|e| {
                    let error =
                        WalletError::NetworkError(format!("Failed to get Spark address: {}", e));
                    eprintln!("{}", format_error_with_hint(&error));
                    anyhow::anyhow!("{}", e)
                })?;

                let mut output = format_receive_address(&address, show_qr)?;
                if copy {
                    copy_to_clipboard(&address)?;
                    output.push_str(&format!("{} Copied address to clipboard.\n", "✓".green()));
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

fn format_network_status(report: spark::NetworkStatusReport, network: spark::Network) -> String {
    let mut output = String::new();
    output.push_str("Wallet Network Status\n");
    output.push_str("────────────────────────────\n");
    output.push_str(&format!("  Status:  {}\n", report.status.as_str()));
    output.push_str(&format!("  Network: {:?}\n", network));
    if let Some(detail) = report.detail {
        output.push_str(&format!("  Reason:  {}\n", detail));
    }
    output.push('\n');
    output
}

fn payment_notification_message(payment: &spark::Payment) -> Option<String> {
    if payment.payment_type != spark::PaymentType::Receive
        || payment.status != spark::PaymentStatus::Completed
    {
        return None;
    }

    let method = format_payment_method(payment.method);
    Some(format!("Received {} sats via {}", payment.amount, method))
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
    let code = qrcode::QrCode::new(payload.as_bytes()).context("Failed to generate QR code")?;
    let width = code.width();
    let border = 2usize;
    let mut output = String::new();

    for y in 0..(width + border * 2) {
        for x in 0..(width + border * 2) {
            let module_is_dark =
                if x < border || y < border || x >= width + border || y >= width + border {
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

#[allow(dead_code)]
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
fn copy_with_command_if_exists(command: &str, args: &[&str], contents: &str) -> Result<bool> {
    use std::io::{ErrorKind, Write};
    use std::process::{Command, Stdio};

    let mut child = match Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
    {
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

pub fn send(
    destination: String,
    amount: u64,
    yes: bool,
    qr: Option<PathBuf>,
    payee: Option<String>,
) -> Result<()> {
    if let Err(e) = validate_amount(amount) {
        eprintln!("{}", format_error_with_hint(&e));
        return Err(e.into());
    }

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;
        let config = LocalWalletConfig::load()?;

        let destination = resolve_send_destination(destination, qr, payee)?;

        if let Err(e) = detect_and_validate_destination(&destination) {
            eprintln!("{}", format_error_with_hint(&e));
            return Err(e.into());
        }

        println!("Preparing Payment...");
        println!();

        let prepare_response = wallet
            .prepare_send_payment(&destination, Some(amount))
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.to_lowercase().contains("insufficient") {
                    let error = WalletError::InsufficientBalance {
                        required: amount,
                        available: 0,
                    };
                    eprintln!("{}", format_error_with_hint(&error));
                } else if msg.to_lowercase().contains("expired") {
                    let error = WalletError::InvoiceExpired;
                    eprintln!("{}", format_error_with_hint(&error));
                } else if msg.to_lowercase().contains("route") {
                    let error = WalletError::NoRouteFound;
                    eprintln!("{}", format_error_with_hint(&error));
                } else {
                    let error = WalletError::PaymentFailed(msg.clone());
                    eprintln!("{}", format_error_with_hint(&error));
                }
                anyhow::anyhow!("{}", e)
            })?;

        let amount_sats = amount_from_prepare(&prepare_response)?;
        enforce_transaction_limit(amount_sats, &config)?;
        let preview = build_send_preview(&destination, &prepare_response);
        let preview_text = format_send_preview(&preview);
        print!("{}", preview_text);

        if !confirm_send_for_amount(yes, amount_sats, &config)? {
            println!("Payment cancelled.");
            return Ok(());
        }

        println!();
        println!("Sending Payment...");
        println!();

        match wallet.send_payment(prepare_response, None).await {
            Ok(response) => {
                println!("{} Payment Sent!", "✓".green());
                println!("────────────────────────────────────────");
                println!("  Payment ID: {}", response.payment.id);
                Ok(())
            }
            Err(e) => {
                eprintln!("{} Payment Failed", "✗".red());
                eprintln!("────────────────────────────────────────");
                eprintln!("{}", e.user_friendly_message());
                if e.balance_unaffected() {
                    eprintln!();
                    eprintln!("{}", "ℹ️  Your balance was NOT deducted.".cyan());
                }
                eprintln!();
                eprintln!(
                    "{}: Use 'openagents wallet retry --last' to retry this payment.",
                    "Hint".cyan()
                );
                Err(e.into())
            }
        }
    })
}

pub fn retry(payment_id: Option<String>, last: bool, yes: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;
        let config = LocalWalletConfig::load()?;
        let context = if last {
            find_last_retryable_payment(&wallet).await.map_err(|e| {
                eprintln!("{}: {}", "Error".red(), e);
                eprintln!();
                eprintln!(
                    "{}: Use 'openagents wallet history' to find payment IDs to retry.",
                    "Hint".cyan()
                );
                e
            })?
        } else {
            let payment_id = payment_id.ok_or_else(|| {
                eprintln!("{}: Provide a payment ID or use --last.", "Error".red());
                eprintln!();
                eprintln!(
                    "{}: Use 'openagents wallet retry --last' to retry the most recent failed payment.",
                    "Hint".cyan()
                );
                anyhow::anyhow!("Provide a payment ID or use --last.")
            })?;
            find_retry_context_by_id(&wallet, &payment_id).await.map_err(|e| {
                eprintln!("{}: {}", "Error".red(), e);
                eprintln!();
                eprintln!(
                    "{}: Use 'openagents wallet history' to see your payment history.",
                    "Hint".cyan()
                );
                e
            })?
        };

        println!("Preparing Retry...");
        println!();

        let prepare_response = wallet
            .prepare_send_payment(
                &context.request.payment_request,
                context.request.amount_sats,
            )
            .await
            .map_err(|e| {
                let error = WalletError::PaymentFailed(e.to_string());
                eprintln!("{}", format_error_with_hint(&error));
                anyhow::anyhow!("{}", e)
            })?;

        let amount_sats = amount_from_prepare(&prepare_response)?;
        enforce_transaction_limit(amount_sats, &config)?;
        let preview = build_send_preview(&context.request.payment_request, &prepare_response);
        let preview_text = format_retry_preview(&context.payment, &preview);
        print!("{}", preview_text);

        if !confirm_send_for_amount(yes, amount_sats, &config)? {
            println!("Retry cancelled.");
            return Ok(());
        }

        println!();
        println!("Sending Payment...");
        println!();

        match wallet.send_payment(prepare_response, None).await {
            Ok(response) => {
                println!("{} Payment Sent!", "✓".green());
                println!("────────────────────────────────────────");
                println!("  Original Payment ID: {}", context.payment.id);
                println!("  New Payment ID:      {}", response.payment.id);
                Ok(())
            }
            Err(e) => {
                eprintln!("{} Payment Failed", "✗".red());
                eprintln!("────────────────────────────────────────");
                eprintln!("{}", e.user_friendly_message());
                if e.balance_unaffected() {
                    eprintln!();
                    eprintln!("{}", "ℹ️  Your balance was NOT deducted.".cyan());
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

#[derive(Debug)]
struct RetryRequest {
    payment_request: String,
    amount_sats: Option<u64>,
}

struct RetryContext {
    payment: spark::Payment,
    request: RetryRequest,
}

fn build_send_preview(
    destination: &str,
    prepare: &spark::wallet::PrepareSendPaymentResponse,
) -> SendPreview {
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
    format_preview("Send Payment Confirmation", None, preview)
}

fn format_retry_preview(payment: &spark::Payment, preview: &SendPreview) -> String {
    let extra = format!("  Original ID: {}\n", payment.id);
    format_preview("Retry Payment Confirmation", Some(extra.as_str()), preview)
}

fn format_preview(title: &str, extra_header: Option<&str>, preview: &SendPreview) -> String {
    let mut output = String::new();
    output.push_str(title);
    output.push('\n');
    output.push_str("────────────────────────────\n");
    if let Some(extra_header) = extra_header {
        output.push_str(extra_header);
    }
    output.push_str(&format_send_preview_body(preview));
    output
}

fn format_send_preview_body(preview: &SendPreview) -> String {
    let mut output = String::new();
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

async fn find_retry_context_by_id(wallet: &SparkWallet, payment_id: &str) -> Result<RetryContext> {
    let payment = find_payment_by_id(wallet, payment_id).await?;
    let request = retry_request_from_payment(&payment)?;
    Ok(RetryContext { payment, request })
}

async fn find_last_retryable_payment(wallet: &SparkWallet) -> Result<RetryContext> {
    let mut offset = 0u32;
    for _ in 0..RETRY_MAX_PAGES {
        let payments = wallet
            .list_payments(Some(RETRY_PAGE_SIZE), Some(offset))
            .await?;
        if payments.is_empty() {
            break;
        }

        for payment in &payments {
            if payment.payment_type != spark::PaymentType::Send
                || payment.status != spark::PaymentStatus::Failed
            {
                continue;
            }

            if let Ok(request) = retry_request_from_payment(&payment) {
                return Ok(RetryContext {
                    payment: payment.clone(),
                    request,
                });
            }
        }

        if payments.len() < RETRY_PAGE_SIZE as usize {
            break;
        }
        offset = offset.saturating_add(RETRY_PAGE_SIZE);
    }

    anyhow::bail!("No failed payments available to retry.");
}

async fn find_payment_by_id(wallet: &SparkWallet, payment_id: &str) -> Result<spark::Payment> {
    let mut offset = 0u32;
    for _ in 0..RETRY_MAX_PAGES {
        let payments = wallet
            .list_payments(Some(RETRY_PAGE_SIZE), Some(offset))
            .await?;
        if payments.is_empty() {
            break;
        }

        if let Some(payment) = payments.iter().find(|payment| payment.id == payment_id) {
            return Ok(payment.clone());
        }

        if payments.len() < RETRY_PAGE_SIZE as usize {
            break;
        }
        offset = offset.saturating_add(RETRY_PAGE_SIZE);
    }

    anyhow::bail!("Payment ID '{}' not found in recent history.", payment_id);
}

fn retry_request_from_payment(payment: &spark::Payment) -> Result<RetryRequest> {
    if payment.payment_type != spark::PaymentType::Send {
        anyhow::bail!("Only outgoing payments can be retried.");
    }

    if payment.status != spark::PaymentStatus::Failed {
        anyhow::bail!("Only failed payments can be retried.");
    }

    let amount_sats = payment_amount_sats(payment)?;
    let details = payment
        .details
        .as_ref()
        .context("Payment does not include invoice details required to retry.")?;

    let payment_request = match details {
        spark::wallet::PaymentDetails::Lightning { invoice, .. } => invoice.clone(),
        spark::wallet::PaymentDetails::Spark {
            invoice_details: Some(details),
            ..
        } => details.invoice.clone(),
        _ => {
            anyhow::bail!("Payment does not include a retryable invoice.");
        }
    };

    if payment_request.trim().is_empty() {
        anyhow::bail!("Payment invoice is empty; cannot retry.");
    }

    Ok(RetryRequest {
        payment_request,
        amount_sats,
    })
}

fn payment_amount_sats(payment: &spark::Payment) -> Result<Option<u64>> {
    if payment.amount == 0 {
        return Ok(None);
    }

    let amount =
        u64::try_from(payment.amount).context("Payment amount exceeds supported range.")?;
    Ok(Some(amount))
}

#[allow(dead_code)]
fn confirm_send(skip_confirm: bool) -> Result<bool> {
    use std::io::{self, IsTerminal};

    let is_terminal = io::stdin().is_terminal();
    let mut stdin = io::stdin();
    let mut reader = io::BufReader::new(&mut stdin);
    confirm_send_with_reader(
        skip_confirm,
        is_terminal,
        &mut reader,
        "Confirm payment? [y/N]: ",
    )
}

fn confirm_send_with_reader<R: std::io::BufRead>(
    skip_confirm: bool,
    is_terminal: bool,
    reader: &mut R,
    prompt: &str,
) -> Result<bool> {
    use std::io::{self, Write};

    if skip_confirm {
        return Ok(true);
    }

    if !is_terminal {
        anyhow::bail!("Non-interactive send requires --yes to confirm.");
    }

    print!("{}", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    reader.read_line(&mut input)?;
    let trimmed = input.trim();
    Ok(trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes"))
}

fn confirm_send_for_amount(
    skip_confirm: bool,
    amount: u64,
    config: &LocalWalletConfig,
) -> Result<bool> {
    let prompt = confirmation_prompt(amount, config);
    use std::io::{self, IsTerminal};
    let is_terminal = io::stdin().is_terminal();
    let mut stdin = io::stdin();
    let mut reader = io::BufReader::new(&mut stdin);
    confirm_send_with_reader(skip_confirm, is_terminal, &mut reader, &prompt)
}

fn confirmation_prompt(amount: u64, config: &LocalWalletConfig) -> String {
    if let Some(threshold) = config.security.confirm_large_sats {
        if amount >= threshold {
            return format!("Confirm large payment of {} sats? [y/N]: ", amount);
        }
    }
    "Confirm payment? [y/N]: ".to_string()
}

fn enforce_transaction_limit(amount: u64, config: &LocalWalletConfig) -> Result<()> {
    if let Some(limit) = config.security.max_send_sats {
        if amount > limit {
            anyhow::bail!(
                "Payment amount {} sats exceeds configured limit {} sats. Update with `openagents wallet settings set security.max_send_sats <amount>`.",
                amount,
                limit
            );
        }
    }
    Ok(())
}

fn amount_from_prepare(prepare: &spark::wallet::PrepareSendPaymentResponse) -> Result<u64> {
    u64::try_from(prepare.amount).context("Payment amount exceeds supported range.")
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
    let image =
        image::open(path).with_context(|| format!("Failed to open QR image {}", path.display()))?;
    let gray = image.to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(gray);
    let grids = prepared.detect_grids();

    for grid in grids {
        let (_meta, content) = grid.decode().context("Failed to decode QR code")?;
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

        let payments = wallet
            .list_payments(Some(limit as u32), None)
            .await
            .map_err(|e| {
                let error = WalletError::NetworkError(format!("Failed to fetch history: {}", e));
                eprintln!("{}", format_error_with_hint(&error));
                anyhow::anyhow!("{}", e)
            })?;

        match format {
            HistoryFormat::Table => {
                if output.is_some() {
                    eprintln!(
                        "{}: --output is only supported with --format csv.",
                        "Error".red()
                    );
                    eprintln!();
                    eprintln!(
                        "{}: Use 'openagents wallet history --format csv --output payments.csv'",
                        "Hint".cyan()
                    );
                    anyhow::bail!("--output is only supported with --format csv.");
                }
                let table = format_history_table(&payments);
                print!("{}", table);
            }
            HistoryFormat::Csv => {
                let csv = format_history_csv(&payments);
                if let Some(path) = output {
                    std::fs::write(&path, &csv).map_err(|e| {
                        let error = WalletError::FileError(format!(
                            "Failed to write CSV history to {}: {}",
                            path.display(),
                            e
                        ));
                        eprintln!("{}", format_error_with_hint(&error));
                        anyhow::anyhow!("{}", e)
                    })?;
                    println!("{} Saved CSV history to {}", "✓".green(), path.display());
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
        anyhow::bail!(
            "BTC/USD price request failed with status {}",
            response.status()
        );
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
    use crate::storage::address_book::{ADDRESS_BOOK_ENV, AddressBook};
    use spark::wallet::{
        BitcoinAddressDetails, BitcoinNetwork, Bolt11Invoice, Bolt11InvoiceDetails, PaymentDetails,
        PaymentMethod, PaymentRequestSource, PrepareSendPaymentResponse, SendOnchainFeeQuote,
        SendOnchainSpeedFeeQuote, SendPaymentMethod, SparkInvoicePaymentDetails,
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

    fn sample_failed_payment(method: PaymentMethod, details: PaymentDetails) -> spark::Payment {
        spark::Payment {
            id: "fail-1".to_string(),
            payment_type: spark::PaymentType::Send,
            status: spark::PaymentStatus::Failed,
            amount: 42,
            fees: 1,
            timestamp: 1_700_000_000,
            method,
            details: Some(details),
        }
    }

    fn lightning_details(invoice: &str) -> PaymentDetails {
        PaymentDetails::Lightning {
            description: Some("Retry me".to_string()),
            preimage: None,
            invoice: invoice.to_string(),
            payment_hash: "hash".to_string(),
            destination_pubkey: "02deadbeef".to_string(),
            lnurl_pay_info: None,
            lnurl_withdraw_info: None,
            lnurl_receive_metadata: None,
        }
    }

    fn spark_invoice_details(invoice: &str) -> PaymentDetails {
        PaymentDetails::Spark {
            invoice_details: Some(SparkInvoicePaymentDetails {
                description: None,
                invoice: invoice.to_string(),
            }),
            htlc_details: None,
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
    fn test_format_network_status_connected() {
        let report = spark::NetworkStatusReport::connected();
        let output = format_network_status(report, spark::Network::Regtest);
        assert!(output.contains("Status:  Connected"));
        assert!(output.contains("Network: Regtest"));
        assert!(!output.contains("Reason:"));
    }

    #[test]
    fn test_format_network_status_disconnected_includes_reason() {
        let report = spark::NetworkStatusReport::disconnected(Some("offline".to_string()));
        let output = format_network_status(report, spark::Network::Mainnet);
        assert!(output.contains("Status:  Disconnected"));
        assert!(output.contains("Reason:  offline"));
    }

    #[test]
    fn test_confirm_send_declines_on_no() {
        let mut input = std::io::Cursor::new("n\n");
        let confirmed =
            confirm_send_with_reader(false, true, &mut input, "Confirm payment? [y/N]: ").unwrap();
        assert!(!confirmed);
    }

    #[test]
    fn test_confirmation_prompt_for_large_amount() {
        let mut config = LocalWalletConfig::default();
        config.security.confirm_large_sats = Some(5_000);
        let prompt = confirmation_prompt(10_000, &config);
        assert!(prompt.contains("large payment"));
        assert!(prompt.contains("10000"));
    }

    #[test]
    fn test_enforce_transaction_limit_blocks_amount() {
        let mut config = LocalWalletConfig::default();
        config.security.max_send_sats = Some(1_000);
        let err = enforce_transaction_limit(2_000, &config).unwrap_err();
        assert!(err.to_string().contains("exceeds configured limit"));
        enforce_transaction_limit(500, &config).unwrap();
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
    fn test_retry_request_from_lightning_details() {
        let payment =
            sample_failed_payment(PaymentMethod::Lightning, lightning_details("lnbc1retry"));
        let retry = retry_request_from_payment(&payment).unwrap();
        assert_eq!(retry.payment_request, "lnbc1retry");
        assert_eq!(retry.amount_sats, Some(42));
    }

    #[test]
    fn test_retry_request_from_spark_invoice_details() {
        let payment =
            sample_failed_payment(PaymentMethod::Spark, spark_invoice_details("spark1retry"));
        let retry = retry_request_from_payment(&payment).unwrap();
        assert_eq!(retry.payment_request, "spark1retry");
        assert_eq!(retry.amount_sats, Some(42));
    }

    #[test]
    fn test_retry_request_rejects_non_failed_or_missing_invoice() {
        let mut completed =
            sample_failed_payment(PaymentMethod::Lightning, lightning_details("lnbc1retry"));
        completed.status = spark::PaymentStatus::Completed;
        let err = retry_request_from_payment(&completed).unwrap_err();
        assert!(err.to_string().contains("Only failed payments"));

        let missing_invoice = sample_failed_payment(
            PaymentMethod::Spark,
            PaymentDetails::Spark {
                invoice_details: None,
                htlc_details: None,
            },
        );
        let err = retry_request_from_payment(&missing_invoice).unwrap_err();
        assert!(err.to_string().contains("retryable invoice"));
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
                let is_dark =
                    if x < border || y < border || x >= width + border || y >= width + border {
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
        book.add("alice".to_string(), "lnbc1alice".to_string())
            .unwrap();
        book.save().unwrap();

        let destination =
            resolve_send_destination("-".to_string(), None, Some("alice".to_string())).unwrap();
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

    #[test]
    fn test_parse_note_reference_hex() {
        let note_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let note = parse_note_reference(note_id).unwrap();
        assert_eq!(note.event_id, note_id);
        assert!(note.relays.is_empty());
    }

    #[test]
    fn test_parse_note_reference_note() {
        let bytes = [7u8; 32];
        let note = nostr::encode_note(&bytes).unwrap();
        let reference = parse_note_reference(&note).unwrap();
        assert_eq!(reference.event_id, hex::encode(bytes));
    }

    #[test]
    fn test_extract_zap_targets_defaults_to_author() {
        let event = Event {
            id: "event".to_string(),
            pubkey: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            created_at: 0,
            kind: 1,
            tags: Vec::new(),
            content: String::new(),
            sig: "sig".to_string(),
        };
        let targets = extract_zap_targets(&event);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].pubkey, event.pubkey);
    }

    #[test]
    fn test_extract_zap_targets_parse() {
        let event = Event {
            id: "event".to_string(),
            pubkey: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string(),
            created_at: 0,
            kind: 1,
            tags: vec![vec![
                "zap".to_string(),
                "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string(),
                "wss://relay.example.com".to_string(),
                "2".to_string(),
            ]],
            content: String::new(),
            sig: "sig".to_string(),
        };
        let targets = extract_zap_targets(&event);
        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].pubkey,
            "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
        );
        assert_eq!(
            targets[0].relay_hint,
            Some("wss://relay.example.com".to_string())
        );
        assert_eq!(targets[0].weight, Some(2));
    }

    #[test]
    fn test_compute_zap_splits_equal() {
        let targets = vec![
            ZapTarget {
                pubkey: "aa".to_string(),
                relay_hint: None,
                weight: None,
            },
            ZapTarget {
                pubkey: "bb".to_string(),
                relay_hint: None,
                weight: None,
            },
        ];
        let splits = compute_zap_splits(&targets, 10_000).unwrap();
        assert_eq!(splits.len(), 2);
        assert_eq!(splits[0].amount_msats, 5_000);
        assert_eq!(splits[1].amount_msats, 5_000);
    }

    #[test]
    fn test_compute_zap_splits_weighted() {
        let targets = vec![
            ZapTarget {
                pubkey: "aa".to_string(),
                relay_hint: None,
                weight: Some(1),
            },
            ZapTarget {
                pubkey: "bb".to_string(),
                relay_hint: None,
                weight: Some(3),
            },
        ];
        let splits = compute_zap_splits(&targets, 4_000).unwrap();
        assert_eq!(splits[0].amount_msats, 1_000);
        assert_eq!(splits[1].amount_msats, 3_000);
    }

    #[test]
    fn test_lnurl_encode_decode_roundtrip() {
        let url = "https://example.com/.well-known/lnurlp/alice";
        let lnurl = encode_lnurl(url).unwrap();
        let decoded = decode_lnurl(&lnurl).unwrap();
        assert_eq!(decoded, url);
    }

    #[test]
    fn test_lnurl_from_profile_prefers_lud16() {
        let profile = Event {
            id: "event".to_string(),
            pubkey: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string(),
            created_at: 0,
            kind: 0,
            tags: Vec::new(),
            content: r#"{"lud16":"alice@example.com","lud06":"lnurl1dp68gurn8ghj7"}"#.to_string(),
            sig: "sig".to_string(),
        };
        let source = lnurl_from_profile(&profile).unwrap();
        let decoded = decode_lnurl(&source.lnurl).unwrap();
        assert_eq!(decoded, "https://example.com/.well-known/lnurlp/alice");
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct NoteReference {
    event_id: String,
    relays: Vec<String>,
    kind: Option<u16>,
}

#[derive(Debug, Clone)]
struct ZapTarget {
    pubkey: String,
    relay_hint: Option<String>,
    weight: Option<u64>,
}

#[derive(Debug, Clone)]
struct ZapSplit {
    target: ZapTarget,
    amount_msats: u64,
}

#[derive(Debug, Clone)]
struct LnurlSource {
    lnurl: String,
    url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LnurlPayResponse {
    callback: String,
    min_sendable: u64,
    max_sendable: u64,
    allows_nostr: Option<bool>,
    nostr_pubkey: Option<String>,
    tag: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct LnurlPayInfo {
    lnurl: String,
    lnurl_url: String,
    callback: String,
    min_sendable: u64,
    max_sendable: u64,
    nostr_pubkey: String,
}

fn parse_note_reference(note_id: &str) -> Result<NoteReference> {
    if note_id.starts_with("note") || note_id.starts_with("nevent") {
        let entity = decode_nip19(note_id)
            .with_context(|| format!("Failed to decode note reference '{}'", note_id))?;
        match entity {
            Nip19Entity::Note(id) => Ok(NoteReference {
                event_id: hex::encode(id),
                relays: Vec::new(),
                kind: None,
            }),
            Nip19Entity::Event(pointer) => Ok(NoteReference {
                event_id: hex::encode(pointer.id),
                relays: pointer.relays,
                kind: pointer.kind.and_then(|kind| u16::try_from(kind).ok()),
            }),
            _ => anyhow::bail!("Unsupported note reference: {}", note_id),
        }
    } else {
        let cleaned = note_id.trim();
        if cleaned.len() != 64 || hex::decode(cleaned).is_err() {
            anyhow::bail!(
                "Invalid note id '{}'. Expect 64-char hex or note/nevent.",
                note_id
            );
        }
        Ok(NoteReference {
            event_id: cleaned.to_lowercase(),
            relays: Vec::new(),
            kind: None,
        })
    }
}

fn is_hex_32_bytes(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

fn merge_relays(base: &[String], extras: &[String]) -> Vec<String> {
    let mut merged = Vec::new();
    let mut seen = HashSet::new();

    for relay in base.iter().chain(extras.iter()) {
        if seen.insert(relay.as_str()) {
            merged.push(relay.clone());
        }
    }

    merged
}

fn extract_zap_targets(event: &Event) -> Vec<ZapTarget> {
    let mut targets = Vec::new();

    for tag in &event.tags {
        if tag.first().map(String::as_str) != Some("zap") {
            continue;
        }

        if tag.len() < 2 {
            continue;
        }

        let pubkey = tag[1].clone();
        if !is_hex_32_bytes(&pubkey) {
            continue;
        }

        let relay_hint = tag.get(2).cloned().filter(|value| !value.is_empty());
        let weight = tag.get(3).and_then(|value| value.parse::<u64>().ok());

        targets.push(ZapTarget {
            pubkey,
            relay_hint,
            weight,
        });
    }

    if targets.is_empty() && is_hex_32_bytes(&event.pubkey) {
        targets.push(ZapTarget {
            pubkey: event.pubkey.clone(),
            relay_hint: None,
            weight: None,
        });
    }

    targets
}

fn compute_zap_splits(targets: &[ZapTarget], total_msats: u64) -> Result<Vec<ZapSplit>> {
    if targets.is_empty() {
        anyhow::bail!("No zap targets found for this note.");
    }

    if total_msats == 0 {
        anyhow::bail!("Zap amount must be greater than zero.");
    }

    let has_weights = targets.iter().any(|target| target.weight.is_some());
    let weights: Vec<u64> = targets
        .iter()
        .map(|target| {
            if has_weights {
                target.weight.unwrap_or(0)
            } else {
                1
            }
        })
        .collect();

    let total_weight: u64 = weights.iter().sum();
    if total_weight == 0 {
        anyhow::bail!("Zap weights sum to zero.");
    }

    let mut splits = Vec::new();
    let mut remainder = total_msats;
    for (target, weight) in targets.iter().cloned().zip(weights.iter().copied()) {
        let share = ((total_msats as u128) * (weight as u128) / (total_weight as u128)) as u64;
        splits.push(ZapSplit {
            target,
            amount_msats: share,
        });
        remainder = remainder.saturating_sub(share);
    }

    if remainder > 0 {
        for split in splits.iter_mut().filter(|split| split.amount_msats > 0) {
            if remainder == 0 {
                break;
            }
            split.amount_msats += 1;
            remainder -= 1;
        }
    }

    splits.retain(|split| split.amount_msats > 0);
    if splits.is_empty() {
        anyhow::bail!("Zap amount too small for selected recipients.");
    }

    Ok(splits)
}

fn encode_lnurl(url: &str) -> Result<String> {
    let hrp = Hrp::parse("lnurl").context("Failed to build LNURL hrp")?;
    let encoded =
        bech32::encode::<Bech32>(hrp, url.as_bytes()).context("Failed to encode LNURL")?;
    Ok(encoded)
}

fn decode_lnurl(lnurl: &str) -> Result<String> {
    let (hrp, data) = bech32::decode(&lnurl.to_lowercase()).context("Failed to decode LNURL")?;
    if hrp.to_string() != "lnurl" {
        anyhow::bail!("Invalid LNURL prefix: {}", hrp);
    }
    let url = String::from_utf8(data).context("LNURL payload is not valid UTF-8")?;
    Ok(url)
}

fn lnurl_from_lud16(address: &str) -> Result<LnurlSource> {
    let mut parts = address.split('@');
    let name = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if name.is_empty() || domain.is_empty() || parts.next().is_some() {
        anyhow::bail!("Invalid lightning address '{}'", address);
    }

    let url = format!("https://{}/.well-known/lnurlp/{}", domain, name);
    let lnurl = encode_lnurl(&url)?;

    Ok(LnurlSource { lnurl, url })
}

fn lnurl_from_lud06(lnurl: &str) -> Result<LnurlSource> {
    let url = decode_lnurl(lnurl)?;
    Ok(LnurlSource {
        lnurl: lnurl.to_lowercase(),
        url,
    })
}

fn lnurl_from_profile(profile: &Event) -> Result<LnurlSource> {
    let payload: serde_json::Value =
        serde_json::from_str(&profile.content).context("Failed to parse profile metadata")?;

    if let Some(lud16) = payload.get("lud16").and_then(|value| value.as_str()) {
        return lnurl_from_lud16(lud16);
    }

    if let Some(lud06) = payload.get("lud06").and_then(|value| value.as_str()) {
        return lnurl_from_lud06(lud06);
    }

    anyhow::bail!("Profile does not include a lightning address (lud16/lud06).");
}

async fn fetch_lnurl_pay_info(http: &Client, source: &LnurlSource) -> Result<LnurlPayInfo> {
    let response = http
        .get(&source.url)
        .header("Accept", "application/json")
        .send()
        .await
        .with_context(|| format!("Failed to fetch LNURL pay info from {}", source.url))?;

    if !response.status().is_success() {
        anyhow::bail!("LNURL pay request failed with status {}", response.status());
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse LNURL pay response")?;

    if let Some(status) = payload.get("status").and_then(|value| value.as_str()) {
        if status.eq_ignore_ascii_case("ERROR") {
            let reason = payload
                .get("reason")
                .and_then(|value| value.as_str())
                .unwrap_or("Unknown LNURL error");
            anyhow::bail!("LNURL error: {}", reason);
        }
    }

    let info: LnurlPayResponse = serde_json::from_value(payload)?;
    if info.tag.as_deref() != Some("payRequest") {
        anyhow::bail!("LNURL response missing payRequest tag.");
    }

    if !info.allows_nostr.unwrap_or(false) {
        anyhow::bail!("Recipient LNURL endpoint does not support Nostr zaps.");
    }

    let nostr_pubkey = info
        .nostr_pubkey
        .ok_or_else(|| anyhow::anyhow!("LNURL response missing nostrPubkey"))?;
    if !is_hex_32_bytes(&nostr_pubkey) {
        anyhow::bail!("LNURL nostrPubkey is not valid hex.");
    }

    Ok(LnurlPayInfo {
        lnurl: source.lnurl.clone(),
        lnurl_url: source.url.clone(),
        callback: info.callback,
        min_sendable: info.min_sendable,
        max_sendable: info.max_sendable,
        nostr_pubkey,
    })
}

async fn request_zap_invoice(
    http: &Client,
    callback: &str,
    amount_msats: u64,
    zap_request: &Event,
    lnurl: &str,
) -> Result<String> {
    let mut url =
        Url::parse(callback).with_context(|| format!("Invalid LNURL callback '{}'", callback))?;
    let zap_json = serde_json::to_string(zap_request).context("Failed to encode zap request")?;

    url.query_pairs_mut()
        .append_pair("amount", &amount_msats.to_string())
        .append_pair("nostr", &zap_json)
        .append_pair("lnurl", lnurl);

    let response = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await
        .context("Failed to fetch zap invoice")?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Zap invoice request failed with status {}",
            response.status()
        );
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse zap invoice response")?;

    if let Some(status) = payload.get("status").and_then(|value| value.as_str()) {
        if status.eq_ignore_ascii_case("ERROR") {
            let reason = payload
                .get("reason")
                .and_then(|value| value.as_str())
                .unwrap_or("Unknown LNURL error");
            anyhow::bail!("LNURL error: {}", reason);
        }
    }

    let invoice = payload
        .get("pr")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow::anyhow!("LNURL response missing invoice (pr)"))?;

    Ok(invoice.to_string())
}

async fn fetch_event_by_id(client: &NostrClient, event_id: &str) -> Result<Event> {
    let filter = serde_json::json!({
        "ids": [event_id],
        "limit": 1
    });
    let mut events = client.fetch_events(vec![filter]).await?;
    events
        .pop()
        .ok_or_else(|| anyhow::anyhow!("Note {} not found on configured relays.", event_id))
}

async fn fetch_profile(client: &NostrClient, pubkey: &str) -> Result<Event> {
    client
        .fetch_profile(pubkey)
        .await?
        .ok_or_else(|| anyhow::anyhow!("No profile found for {}", pubkey))
}

fn build_zap_request_event(
    identity: &UnifiedIdentity,
    note: &Event,
    recipient_pubkey: &str,
    amount_msats: u64,
    lnurl: &str,
    relays: &[String],
) -> Result<Event> {
    if relays.is_empty() {
        anyhow::bail!("No relays configured for zap receipts.");
    }

    let mut tags = Vec::new();
    let mut relay_tag = Vec::with_capacity(relays.len() + 1);
    relay_tag.push("relays".to_string());
    relay_tag.extend(relays.iter().cloned());
    tags.push(relay_tag);
    tags.push(vec!["amount".to_string(), amount_msats.to_string()]);
    tags.push(vec!["lnurl".to_string(), lnurl.to_string()]);
    tags.push(vec!["p".to_string(), recipient_pubkey.to_string()]);
    tags.push(vec!["e".to_string(), note.id.clone()]);
    tags.push(vec!["k".to_string(), note.kind.to_string()]);

    let template = EventTemplate {
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("System time error")?
            .as_secs(),
        kind: ZAP_REQUEST_KIND,
        tags,
        content: String::new(),
    };

    identity
        .sign_event(template)
        .context("Failed to sign zap request")
}

async fn parse_invoice_amount_msats(invoice: &str) -> Option<u64> {
    if let Ok(input) = breez_sdk_spark::parse_input(invoice, None).await {
        match input {
            breez_sdk_spark::InputType::Bolt11Invoice(details) => return details.amount_msat,
            breez_sdk_spark::InputType::SparkInvoice(details) => {
                return details
                    .amount
                    .and_then(|value| value.checked_mul(1000))
                    .and_then(|value| u64::try_from(value).ok());
            }
            _ => {}
        }
    }

    None
}

fn short_pubkey(pubkey: &str) -> String {
    if pubkey.len() <= 12 {
        pubkey.to_string()
    } else {
        format!("{}...{}", &pubkey[..8], &pubkey[pubkey.len() - 4..])
    }
}

/// Send a zap to a Nostr note
pub fn zap(note_id: String, amount: u64) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let config = LocalWalletConfig::load()?;
        let note_ref = parse_note_reference(&note_id)?;
        let relays = merge_relays(&config.nostr.relays, &note_ref.relays);
        let client = NostrClient::new(relays.clone());
        let note = fetch_event_by_id(&client, &note_ref.event_id).await?;

        let targets = extract_zap_targets(&note);
        let amount_msats = amount
            .checked_mul(1000)
            .ok_or_else(|| anyhow::anyhow!("Amount too large"))?;
        let splits = compute_zap_splits(&targets, amount_msats)?;

        let mnemonic = load_mnemonic()?;
        let identity = UnifiedIdentity::from_mnemonic(Mnemonic::parse(mnemonic)?)?;

        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .context("Failed to build HTTP client")?;

        let wallet = get_wallet().await?;

        println!(
            "Preparing zap for {} sats to {} recipient(s).",
            amount,
            splits.len()
        );
        for split in &splits {
            println!(
                "  {} sats -> {}",
                split.amount_msats / 1000,
                short_pubkey(&split.target.pubkey)
            );
        }

        for split in splits {
            let mut profile_relays = relays.clone();
            if let Some(relay) = &split.target.relay_hint {
                profile_relays = merge_relays(&profile_relays, &[relay.clone()]);
            }
            let profile_client = NostrClient::new(profile_relays);
            let profile = fetch_profile(&profile_client, &split.target.pubkey).await?;
            let lnurl_source = lnurl_from_profile(&profile)?;
            let lnurl_info = fetch_lnurl_pay_info(&http, &lnurl_source).await?;

            if split.amount_msats < lnurl_info.min_sendable
                || split.amount_msats > lnurl_info.max_sendable
            {
                anyhow::bail!(
                    "Zap amount {} msats outside LNURL limits ({}-{} msats).",
                    split.amount_msats,
                    lnurl_info.min_sendable,
                    lnurl_info.max_sendable
                );
            }

            let zap_request = build_zap_request_event(
                &identity,
                &note,
                &split.target.pubkey,
                split.amount_msats,
                &lnurl_info.lnurl,
                &relays,
            )?;

            let invoice = request_zap_invoice(
                &http,
                &lnurl_info.callback,
                split.amount_msats,
                &zap_request,
                &lnurl_info.lnurl,
            )
            .await?;

            println!(
                "Paying {} sats zap invoice for {}...",
                split.amount_msats / 1000,
                short_pubkey(&split.target.pubkey)
            );

            match wallet.send_payment_simple(&invoice, None).await {
                Ok(response) => {
                    println!("✓ Zap paid. Payment ID: {}", response.payment.id);
                }
                Err(err) => {
                    eprintln!("✗ Zap payment failed: {}", err.user_friendly_message());
                    if err.balance_unaffected() {
                        eprintln!("ℹ️  Your balance was NOT deducted.");
                    }
                    return Err(err.into());
                }
            }
        }

        Ok(())
    })
}

/// Query zaps on a Nostr note
pub fn zaps(note_id: String) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let config = LocalWalletConfig::load()?;
        let note_ref = parse_note_reference(&note_id)?;
        let relays = merge_relays(&config.nostr.relays, &note_ref.relays);
        let client = NostrClient::new(relays.clone());

        let filter = serde_json::json!({
            "kinds": [ZAP_RECEIPT_KIND],
            "#e": [note_ref.event_id],
        });
        let events = client.fetch_events(vec![filter]).await?;

        if events.is_empty() {
            println!("No zap receipts found for {}.", note_id);
            return Ok(());
        }

        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .context("Failed to build HTTP client")?;

        let mut lnurl_cache: HashMap<String, LnurlPayInfo> = HashMap::new();
        let mut total_msats = 0u64;
        let mut valid_count = 0usize;

        println!("Zap receipts for {}", note_id);
        println!("────────────────────────────────────────");

        for event in events {
            let receipt = match ZapReceipt::from_event(event) {
                Ok(receipt) => receipt,
                Err(err) => {
                    println!("✗ Invalid zap receipt: {}", err);
                    continue;
                }
            };

            let zap_request = receipt.get_zap_request();
            let amount_msats = zap_request
                .as_ref()
                .ok()
                .and_then(|request| request.amount_msats);
            let sender_pubkey = zap_request
                .as_ref()
                .ok()
                .map(|request| request.event.pubkey.clone())
                .or(receipt.sender_pubkey.clone());

            let amount_display = amount_msats
                .map(|value| format!("{} sats", value / 1000))
                .unwrap_or_else(|| "unknown sats".to_string());

            let sender_display = sender_pubkey
                .map(|value| short_pubkey(&value))
                .unwrap_or_else(|| "unknown sender".to_string());

            let info = if let Some(info) = lnurl_cache.get(&receipt.recipient_pubkey) {
                info.clone()
            } else {
                let profile = match fetch_profile(&client, &receipt.recipient_pubkey).await {
                    Ok(profile) => profile,
                    Err(err) => {
                        println!(
                            "✗ {} from {} (lnurl lookup failed: {})",
                            amount_display, sender_display, err
                        );
                        continue;
                    }
                };
                let lnurl_source = match lnurl_from_profile(&profile) {
                    Ok(source) => source,
                    Err(err) => {
                        println!(
                            "✗ {} from {} (lnurl lookup failed: {})",
                            amount_display, sender_display, err
                        );
                        continue;
                    }
                };
                let info = match fetch_lnurl_pay_info(&http, &lnurl_source).await {
                    Ok(info) => info,
                    Err(err) => {
                        println!(
                            "✗ {} from {} (lnurl lookup failed: {})",
                            amount_display, sender_display, err
                        );
                        continue;
                    }
                };
                lnurl_cache.insert(receipt.recipient_pubkey.clone(), info.clone());
                info
            };

            let invoice_msats = parse_invoice_amount_msats(&receipt.bolt11).await;
            let valid = receipt
                .validate(&info.nostr_pubkey, invoice_msats, Some(&info.lnurl))
                .is_ok();

            let amount_display = amount_msats
                .or(invoice_msats)
                .map(|value| format!("{} sats", value / 1000))
                .unwrap_or_else(|| "unknown sats".to_string());

            let comment = zap_request
                .as_ref()
                .ok()
                .map(|request| request.content.trim().to_string())
                .filter(|value| !value.is_empty());

            let status = if valid { "✓" } else { "✗" };
            if valid {
                valid_count += 1;
                if let Some(amount) = amount_msats.or(invoice_msats) {
                    total_msats = total_msats.saturating_add(amount);
                }
            }

            if let Some(comment) = comment {
                println!(
                    "{} {} from {} - {}",
                    status, amount_display, sender_display, comment
                );
            } else {
                println!("{} {} from {}", status, amount_display, sender_display);
            }
        }

        println!("────────────────────────────────────────");
        println!(
            "Valid zaps: {} (total {} sats)",
            valid_count,
            total_msats / 1000
        );

        Ok(())
    })
}

/// Create a Nostr Wallet Connect connection
pub fn nwc_create(name: Option<String>) -> Result<()> {
    let config = LocalWalletConfig::load()?;
    let relays = config.nostr.relays.clone();

    let output = build_connection(name, relays)?;
    let mut store = NwcConnectionStore::load()?;
    store.add(output.connection.clone())?;
    store.save()?;

    let rt = tokio::runtime::Runtime::new()?;
    if let Err(err) = rt.block_on(async { publish_info_event(&output.connection).await }) {
        eprintln!("Warning: failed to publish NWC info event: {}", err);
    }

    println!("NWC connection created:");
    println!("  ID:     {}", output.connection.id);
    if let Some(name) = &output.connection.name {
        println!("  Name:   {}", name);
    }
    println!("  Relays: {}", output.connection.relays.join(", "));
    println!();
    println!("Connection URI (keep this secret):");
    println!("{}", output.uri);

    Ok(())
}

/// List Nostr Wallet Connect connections
pub fn nwc_list() -> Result<()> {
    let store = NwcConnectionStore::load()?;

    if store.connections.is_empty() {
        println!("No NWC connections found. Create one with `openagents wallet nwc create`.");
        return Ok(());
    }

    println!("NWC connections:");
    for connection in store.connections {
        println!("- ID: {}", connection.id);
        if let Some(name) = &connection.name {
            println!("  Name:   {}", name);
        }
        println!("  Wallet pubkey: {}", connection.wallet_pubkey);
        println!("  Client pubkey: {}", connection.client_pubkey);
        println!("  Relays: {}", connection.relays.join(", "));
        println!("  Created: {}", connection.created_at);
    }
    Ok(())
}

/// Listen for Nostr Wallet Connect requests
pub fn nwc_listen() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;
        let config = LocalWalletConfig::load()?;
        let store = NwcConnectionStore::load()?;
        let service = NwcService::new(wallet, config, store.connections).await?;

        println!("Listening for NWC requests...");
        service.run().await
    })
}

/// Revoke a Nostr Wallet Connect connection
pub fn nwc_revoke(id: String) -> Result<()> {
    let mut store = NwcConnectionStore::load()?;
    let removed = store.remove(&id)?;
    store.save()?;

    println!("Revoked NWC connection {}", removed.id);
    Ok(())
}
