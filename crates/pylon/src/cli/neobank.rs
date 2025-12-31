//! Neobank CLI commands
//!
//! Commands for managing multi-currency treasury via neobank.

use crate::daemon::{ControlClient, DaemonResponse};
use clap::{Parser, Subcommand};

/// Neobank treasury management commands
#[derive(Parser)]
pub struct NeobankArgs {
    #[command(subcommand)]
    pub command: NeobankCommand,
}

/// Available neobank commands
#[derive(Subcommand)]
pub enum NeobankCommand {
    /// Check wallet balance
    Balance {
        /// Currency to check (btc or usd)
        #[arg(long, default_value = "btc")]
        currency: String,
    },
    /// Get treasury status (all currencies)
    Status,
    /// Pay a Lightning invoice
    Pay {
        /// BOLT11 Lightning invoice to pay
        bolt11: String,
    },
    /// Send Cashu tokens to another agent
    Send {
        /// Amount in satoshis
        amount: u64,
        /// Currency (btc or usd)
        #[arg(long, default_value = "btc")]
        currency: String,
    },
    /// Receive Cashu tokens from another agent
    Receive {
        /// Cashu token string
        token: String,
    },
}

/// Execute a neobank command
pub async fn run(args: NeobankArgs) -> anyhow::Result<()> {
    let socket_path = crate::socket_path()?;

    if !socket_path.exists() {
        println!("Pylon daemon is not running. Start it with 'pylon start'.");
        return Ok(());
    }

    let client = ControlClient::new(socket_path);

    match args.command {
        NeobankCommand::Balance { currency } => match client.neobank_balance(&currency)? {
            DaemonResponse::NeobankBalance { sats } => {
                let currency_upper = currency.to_uppercase();
                if currency_upper == "BTC" {
                    println!(
                        "Balance: {} sats ({:.8} BTC)",
                        sats,
                        sats as f64 / 100_000_000.0
                    );
                } else {
                    println!("Balance: {} cents (${:.2})", sats, sats as f64 / 100.0);
                }
            }
            DaemonResponse::Error(e) => {
                println!("Error: {}", e);
            }
            _ => {
                println!("Unexpected response from daemon");
            }
        },
        NeobankCommand::Status => match client.neobank_status()? {
            DaemonResponse::NeobankStatus {
                btc_balance_sats,
                usd_balance_cents,
                treasury_active,
                btc_usd_rate,
            } => {
                println!("Neobank Treasury Status");
                println!("=======================");
                println!();
                println!(
                    "BTC Balance: {} sats ({:.8} BTC)",
                    btc_balance_sats,
                    btc_balance_sats as f64 / 100_000_000.0
                );
                println!(
                    "USD Balance: {} cents (${:.2})",
                    usd_balance_cents,
                    usd_balance_cents as f64 / 100.0
                );
                println!();
                println!(
                    "Treasury Agent: {}",
                    if treasury_active {
                        "Active"
                    } else {
                        "Inactive"
                    }
                );
                if let Some(rate) = btc_usd_rate {
                    println!("BTC/USD Rate: ${:.2}", rate);
                }
            }
            DaemonResponse::Error(e) => {
                println!("Error: {}", e);
            }
            _ => {
                println!("Unexpected response from daemon");
            }
        },
        NeobankCommand::Pay { bolt11 } => {
            println!("Paying invoice...");
            match client.neobank_pay(&bolt11)? {
                DaemonResponse::NeobankPayment { preimage } => {
                    println!("Payment successful!");
                    println!("Preimage: {}", preimage);
                }
                DaemonResponse::Error(e) => {
                    println!("Payment failed: {}", e);
                }
                _ => {
                    println!("Unexpected response from daemon");
                }
            }
        }
        NeobankCommand::Send { amount, currency } => {
            println!("Sending {} sats ({})...", amount, currency.to_uppercase());
            match client.neobank_send(amount, &currency)? {
                DaemonResponse::NeobankSend { token } => {
                    println!("Tokens created successfully!");
                    println!();
                    println!("Share this token with the recipient:");
                    println!("{}", token);
                }
                DaemonResponse::Error(e) => {
                    println!("Send failed: {}", e);
                }
                _ => {
                    println!("Unexpected response from daemon");
                }
            }
        }
        NeobankCommand::Receive { token } => {
            println!("Receiving tokens...");
            match client.neobank_receive(&token)? {
                DaemonResponse::NeobankReceive { amount_sats } => {
                    println!("Received {} sats successfully!", amount_sats);
                }
                DaemonResponse::Error(e) => {
                    println!("Receive failed: {}", e);
                }
                _ => {
                    println!("Unexpected response from daemon");
                }
            }
        }
    }

    Ok(())
}
