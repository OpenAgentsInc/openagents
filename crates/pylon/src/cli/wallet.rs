//! Wallet CLI commands
//!
//! Commands for managing Spark wallet (Lightning payments).

use clap::{Parser, Subcommand};
use spark::{Network, SparkSigner, SparkWallet, WalletConfig};
use std::path::PathBuf;

/// Wallet management commands
#[derive(Parser)]
pub struct WalletArgs {
    #[command(subcommand)]
    pub command: WalletCommand,
}

/// Available wallet commands
#[derive(Subcommand)]
pub enum WalletCommand {
    /// Check wallet balance
    Balance,
    /// Show wallet status and info
    Status,
    /// Get Spark address for receiving payments
    Address,
    /// Create an invoice to receive payment
    Invoice {
        /// Amount in satoshis
        amount: u64,
        /// Optional description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Pay a Lightning invoice or Spark address
    Pay {
        /// Lightning invoice (BOLT-11) or Spark address
        invoice: String,
        /// Amount in sats (required for zero-amount invoices)
        #[arg(short, long)]
        amount: Option<u64>,
    },
    /// List recent payments
    History {
        /// Number of payments to show
        #[arg(short, long, default_value = "10")]
        limit: u32,
    },
    /// Get regtest sats from faucet
    Fund {
        /// Amount in satoshis (default: 100000)
        #[arg(short, long, default_value = "100000")]
        amount: u64,
    },
}

/// Get pylon data directory
fn data_dir() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    Ok(home.join(".openagents").join("pylon"))
}

/// Load mnemonic from identity file
fn load_mnemonic() -> anyhow::Result<String> {
    let identity_file = data_dir()?.join("identity.mnemonic");
    if !identity_file.exists() {
        anyhow::bail!(
            "No identity found. Run 'pylon init' first.\n  Expected: {:?}",
            identity_file
        );
    }
    let mnemonic = std::fs::read_to_string(&identity_file)?;
    Ok(mnemonic.trim().to_string())
}

/// Create a Spark wallet from the stored identity
async fn create_wallet() -> anyhow::Result<SparkWallet> {
    let mnemonic = load_mnemonic()?;

    let signer = SparkSigner::from_mnemonic(&mnemonic, "")
        .map_err(|e| anyhow::anyhow!("Failed to create signer: {}", e))?;

    let config = WalletConfig {
        network: Network::Regtest, // Use regtest for testing
        api_key: None,             // Not needed for regtest
        storage_dir: data_dir()?.join("spark"),
    };

    println!("Connecting to Spark network (regtest)...");

    let wallet = SparkWallet::new(signer, config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to initialize wallet: {}", e))?;

    Ok(wallet)
}

/// Execute a wallet command
pub async fn run(args: WalletArgs) -> anyhow::Result<()> {
    match args.command {
        WalletCommand::Balance => {
            let wallet = create_wallet().await?;
            let balance = wallet
                .get_balance()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to get balance: {}", e))?;

            println!("\nWallet Balance");
            println!("==============");
            println!(
                "Spark:     {:>12} sats",
                format_sats(balance.spark_sats)
            );
            println!(
                "Lightning: {:>12} sats",
                format_sats(balance.lightning_sats)
            );
            println!(
                "On-chain:  {:>12} sats",
                format_sats(balance.onchain_sats)
            );
            println!("─────────────────────────");
            println!(
                "Total:     {:>12} sats ({:.8} BTC)",
                format_sats(balance.total_sats()),
                balance.total_sats() as f64 / 100_000_000.0
            );
        }

        WalletCommand::Status => {
            let wallet = create_wallet().await?;
            let balance = wallet
                .get_balance()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to get balance: {}", e))?;

            println!("\nWallet Status");
            println!("=============");
            println!("Network:   regtest");
            println!("Balance:   {} sats", format_sats(balance.total_sats()));

            // Check network connectivity
            let status = wallet
                .network_status(std::time::Duration::from_secs(5))
                .await;
            println!("Network:   {}", status.status.as_str());
            if let Some(detail) = status.detail {
                println!("           {}", detail);
            }
        }

        WalletCommand::Address => {
            let wallet = create_wallet().await?;
            let address = wallet
                .get_spark_address()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to get address: {}", e))?;

            println!("\nSpark Address (for receiving payments):");
            println!("{}", address);
        }

        WalletCommand::Invoice { amount, description } => {
            let wallet = create_wallet().await?;
            let response = wallet
                .create_invoice(amount, description, None)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create invoice: {}", e))?;

            println!("\nInvoice Created");
            println!("===============");
            println!("Amount: {} sats", format_sats(amount));
            println!("\nPayment Request:");
            println!("{}", response.payment_request);
        }

        WalletCommand::Pay { invoice, amount } => {
            let wallet = create_wallet().await?;

            println!("Preparing payment...");
            let prepare = wallet
                .prepare_send_payment(&invoice, amount)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to prepare payment: {}", e))?;

            println!("Sending payment...");
            let response = wallet
                .send_payment(prepare, None)
                .await
                .map_err(|e| anyhow::anyhow!("Payment failed: {}", e))?;

            println!("\nPayment Sent!");
            println!("=============");
            println!("ID:     {}", response.payment.id);
            println!("Amount: {} sats", format_sats(response.payment.amount as u64));
            println!("Status: {:?}", response.payment.status);
        }

        WalletCommand::History { limit } => {
            let wallet = create_wallet().await?;
            let payments = wallet
                .list_payments(Some(limit), None)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to list payments: {}", e))?;

            println!("\nPayment History");
            println!("===============");

            if payments.is_empty() {
                println!("No payments yet.");
            } else {
                for payment in payments {
                    let direction = match payment.payment_type {
                        spark::PaymentType::Send => "→",
                        spark::PaymentType::Receive => "←",
                    };
                    println!(
                        "{} {:>10} sats  {:?}  {}",
                        direction,
                        format_sats(payment.amount as u64),
                        payment.status,
                        &payment.id[..16]
                    );
                }
            }
        }

        WalletCommand::Fund { amount } => {
            let wallet = create_wallet().await?;
            let address = wallet
                .get_spark_address()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to get address: {}", e))?;

            println!("\nRequesting {} sats from regtest faucet...", format_sats(amount));
            println!("Address: {}", address);

            // Check for faucet credentials
            let faucet_url = std::env::var("FAUCET_URL")
                .unwrap_or_else(|_| "https://api.lightspark.com/graphql/spark/rc".to_string());
            let faucet_username = std::env::var("FAUCET_USERNAME").ok();
            let faucet_password = std::env::var("FAUCET_PASSWORD").ok();

            // Build request with optional basic auth
            let client = reqwest::Client::new();
            let mut request = client
                .post(&faucet_url)
                .header("Content-Type", "application/json")
                .json(&serde_json::json!({
                    "operationName": "RequestRegtestFunds",
                    "variables": {
                        "address": address,
                        "amount_sats": amount
                    },
                    "query": "mutation RequestRegtestFunds($address: String!, $amount_sats: Long!) { request_regtest_funds(input: {address: $address, amount_sats: $amount_sats}) { transaction_hash }}"
                }));

            // Add basic auth if credentials are provided
            if let (Some(username), Some(password)) = (&faucet_username, &faucet_password) {
                request = request.basic_auth(username, Some(password));
            }

            let response = request
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("Faucet request failed: {}", e))?;

            let result: serde_json::Value = response
                .json()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

            if let Some(errors) = result.get("errors") {
                if let Some(err) = errors.as_array().and_then(|a| a.first()) {
                    let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
                    if msg.contains("Not logged in") || msg.contains("auth") {
                        anyhow::bail!(
                            "Faucet error: {}\n\n\
                            The regtest faucet requires authentication.\n\
                            Set environment variables:\n  \
                            FAUCET_USERNAME=<your-username>\n  \
                            FAUCET_PASSWORD=<your-password>\n\n\
                            See: crates/spark/docs/REGTEST.md",
                            msg
                        );
                    }
                    anyhow::bail!("Faucet error: {}", msg);
                }
            }

            if let Some(data) = result.get("data").and_then(|d| d.get("request_regtest_funds")) {
                let txid = data.get("transaction_hash").and_then(|t| t.as_str()).unwrap_or("unknown");
                println!("\nFunding successful!");
                println!("Transaction: {}", txid);
                println!("\nWait a moment for the transaction to confirm, then check balance:");
                println!("  pylon wallet balance");
            } else {
                println!("\nFaucet response: {}", serde_json::to_string_pretty(&result)?);
            }
        }
    }

    Ok(())
}

/// Format sats with thousands separators
fn format_sats(sats: u64) -> String {
    let s = sats.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}
