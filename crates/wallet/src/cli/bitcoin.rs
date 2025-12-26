//! Bitcoin/Lightning CLI commands using Spark SDK
//!
//! Provides wallet commands for balance, send, receive using the Breez Spark SDK.

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono;
use spark::{EventListener, Network, SdkEvent, SparkSigner, SparkWallet, WalletConfig};
use std::path::{Path, PathBuf};
use crate::cli::load_mnemonic;
use crate::core::nwc::{build_connection, publish_info_event, NwcService};
use crate::storage::address_book::AddressBook;
use crate::storage::config::WalletConfig as LocalWalletConfig;
use crate::storage::nwc::NwcConnectionStore;

const CLIPBOARD_FILE_ENV: &str = "OPENAGENTS_CLIPBOARD_FILE";
const NOTIFICATION_FILE_ENV: &str = "OPENAGENTS_NOTIFICATION_FILE";
const RETRY_PAGE_SIZE: u32 = 50;
const RETRY_MAX_PAGES: u32 = 20;
const NETWORK_STATUS_TIMEOUT_SECS: u64 = 5;

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

/// Show Spark network connectivity status
pub fn status() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;
        let report = wallet
            .network_status(std::time::Duration::from_secs(NETWORK_STATUS_TIMEOUT_SECS))
            .await;
        let output = format_network_status(report, wallet.config().network);
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
        let config = LocalWalletConfig::load()?;

        let destination = resolve_send_destination(destination, qr, payee)?;

        println!("Preparing Payment...");
        println!();

        let prepare_response = wallet.prepare_send_payment(&destination, Some(amount)).await?;
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

/// Retry a failed payment using the original invoice details
pub fn retry(payment_id: Option<String>, last: bool, yes: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let wallet = get_wallet().await?;
        let config = LocalWalletConfig::load()?;
        let context = if last {
            find_last_retryable_payment(&wallet).await?
        } else {
            let payment_id = payment_id.context("Provide a payment ID or use --last.")?;
            find_retry_context_by_id(&wallet, &payment_id).await?
        };

        println!("Preparing Retry...");
        println!();

        let prepare_response = wallet
            .prepare_send_payment(
                &context.request.payment_request,
                context.request.amount_sats,
            )
            .await?;
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
                println!("✓ Payment Sent!");
                println!("────────────────────────────────────────");
                println!("  Original Payment ID: {}", context.payment.id);
                println!("  New Payment ID:      {}", response.payment.id);
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

#[derive(Debug)]
struct RetryRequest {
    payment_request: String,
    amount_sats: Option<u64>,
}

struct RetryContext {
    payment: spark::Payment,
    request: RetryRequest,
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
        let payments = wallet.list_payments(Some(RETRY_PAGE_SIZE), Some(offset)).await?;
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
        let payments = wallet.list_payments(Some(RETRY_PAGE_SIZE), Some(offset)).await?;
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

    let amount = u64::try_from(payment.amount)
        .context("Payment amount exceeds supported range.")?;
    Ok(Some(amount))
}

fn confirm_send(skip_confirm: bool) -> Result<bool> {
    use std::io::{self, IsTerminal};

    let is_terminal = io::stdin().is_terminal();
    let mut stdin = io::stdin();
    let mut reader = io::BufReader::new(&mut stdin);
    confirm_send_with_reader(skip_confirm, is_terminal, &mut reader, "Confirm payment? [y/N]: ")
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
            return format!(
                "Confirm large payment of {} sats? [y/N]: ",
                amount
            );
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
        PaymentDetails, PaymentMethod, PaymentRequestSource, PrepareSendPaymentResponse,
        SendOnchainFeeQuote, SendOnchainSpeedFeeQuote, SendPaymentMethod,
        SparkInvoicePaymentDetails,
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
        let confirmed = confirm_send_with_reader(
            false,
            true,
            &mut input,
            "Confirm payment? [y/N]: ",
        )
        .unwrap();
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
        let payment = sample_failed_payment(
            PaymentMethod::Lightning,
            lightning_details("lnbc1retry"),
        );
        let retry = retry_request_from_payment(&payment).unwrap();
        assert_eq!(retry.payment_request, "lnbc1retry");
        assert_eq!(retry.amount_sats, Some(42));
    }

    #[test]
    fn test_retry_request_from_spark_invoice_details() {
        let payment = sample_failed_payment(
            PaymentMethod::Spark,
            spark_invoice_details("spark1retry"),
        );
        let retry = retry_request_from_payment(&payment).unwrap();
        assert_eq!(retry.payment_request, "spark1retry");
        assert_eq!(retry.amount_sats, Some(42));
    }

    #[test]
    fn test_retry_request_rejects_non_failed_or_missing_invoice() {
        let mut completed = sample_failed_payment(
            PaymentMethod::Lightning,
            lightning_details("lnbc1retry"),
        );
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
