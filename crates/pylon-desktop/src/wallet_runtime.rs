//! Async wallet runtime for pylon-desktop
//!
//! Bridges async SparkWallet with synchronous winit event loop using channels.

use bip39::Mnemonic;
use spark::{
    Network, PaymentStatus, PaymentType, SparkSigner, SparkWallet, WalletConfig,
};
use std::fs;
use std::path::PathBuf;
use tokio::sync::mpsc;

/// Price per job in satoshis
pub const SATS_PER_JOB: u64 = 10;

/// Events sent from wallet runtime to UI
#[derive(Debug, Clone)]
pub enum WalletEvent {
    /// Wallet initialized successfully
    Initialized {
        balance_sats: u64,
        _spark_address: String,
    },
    /// Wallet initialization failed
    InitFailed(String),
    /// Balance updated
    BalanceUpdated { balance_sats: u64 },
    /// Invoice created for a job
    InvoiceCreated {
        job_id: String,
        bolt11: String,
        amount_sats: u64,
    },
    /// Invoice creation failed
    InvoiceCreationFailed { job_id: String, error: String },
    /// Payment received (invoice paid)
    PaymentReceived {
        _payment_id: String,
        amount_sats: u64,
    },
    /// Payment sent successfully
    PaymentSent {
        _payment_id: String,
        amount_sats: u64,
    },
    /// Payment failed
    PaymentFailed { error: String },
}

/// Commands sent from UI to wallet runtime
#[derive(Debug)]
#[allow(dead_code)]
pub enum WalletCommand {
    /// Create invoice for a job we served
    CreateInvoice {
        job_id: String,
        description: String,
    },
    /// Pay an invoice (for job we requested)
    PayInvoice { bolt11: String },
    /// Get current balance
    GetBalance,
    /// Poll for incoming payments
    PollPayments,
}

/// Wallet runtime handle for communication with background thread
pub struct WalletRuntime {
    cmd_tx: mpsc::Sender<WalletCommand>,
    pub event_rx: mpsc::Receiver<WalletEvent>,
}

impl WalletRuntime {
    /// Create new wallet runtime with background thread
    ///
    /// Loads or generates a mnemonic and initializes the SparkWallet.
    pub fn new(network: Network) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<WalletCommand>(32);
        let (event_tx, event_rx) = mpsc::channel::<WalletEvent>(256);

        // Spawn background thread with tokio runtime
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            rt.block_on(run_wallet_loop(cmd_rx, event_tx, network));
        });

        Self { cmd_tx, event_rx }
    }

    /// Create invoice for a job we served
    pub fn create_invoice(&self, job_id: &str, description: &str) {
        let _ = self.cmd_tx.try_send(WalletCommand::CreateInvoice {
            job_id: job_id.to_string(),
            description: description.to_string(),
        });
    }

    /// Pay an invoice
    #[allow(dead_code)]
    pub fn pay_invoice(&self, bolt11: &str) {
        let _ = self.cmd_tx.try_send(WalletCommand::PayInvoice {
            bolt11: bolt11.to_string(),
        });
    }

    /// Request balance update
    #[allow(dead_code)]
    pub fn get_balance(&self) {
        let _ = self.cmd_tx.try_send(WalletCommand::GetBalance);
    }

    /// Poll for incoming payments
    pub fn poll_payments(&self) {
        let _ = self.cmd_tx.try_send(WalletCommand::PollPayments);
    }
}

impl Default for WalletRuntime {
    fn default() -> Self {
        Self::new(Network::Testnet)
    }
}

/// Get the path to store the wallet mnemonic
fn mnemonic_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("openagents")
        .join("pylon")
        .join("wallet_mnemonic")
}

/// Load or generate a BIP39 mnemonic
fn load_or_generate_mnemonic() -> Result<String, String> {
    let path = mnemonic_path();

    // Try to load existing mnemonic
    if path.exists() {
        return fs::read_to_string(&path)
            .map(|s| s.trim().to_string())
            .map_err(|e| format!("Failed to read mnemonic: {}", e));
    }

    // Generate new mnemonic from random entropy
    use rand::Rng;
    let mut rng = rand::rng();
    let mut entropy = [0u8; 16]; // 128 bits for 12 words
    rng.fill(&mut entropy);

    let mnemonic = Mnemonic::from_entropy(&entropy)
        .map_err(|e| format!("Failed to generate mnemonic: {}", e))?;

    let mnemonic_str = mnemonic.to_string();

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create wallet directory: {}", e))?;
    }

    // Save mnemonic
    fs::write(&path, &mnemonic_str)
        .map_err(|e| format!("Failed to save mnemonic: {}", e))?;

    Ok(mnemonic_str)
}

/// Main async loop running in background thread
async fn run_wallet_loop(
    mut cmd_rx: mpsc::Receiver<WalletCommand>,
    event_tx: mpsc::Sender<WalletEvent>,
    network: Network,
) {
    // Load or generate mnemonic
    let mnemonic = match load_or_generate_mnemonic() {
        Ok(m) => m,
        Err(e) => {
            let _ = event_tx.send(WalletEvent::InitFailed(e)).await;
            return;
        }
    };

    // Create signer
    let signer = match SparkSigner::from_mnemonic(&mnemonic, "") {
        Ok(s) => s,
        Err(e) => {
            let _ = event_tx
                .send(WalletEvent::InitFailed(format!(
                    "Failed to create signer: {}",
                    e
                )))
                .await;
            return;
        }
    };

    // Configure wallet
    let config = WalletConfig {
        network,
        ..Default::default()
    };

    // Initialize wallet (this connects to Breez SDK)
    let wallet = match SparkWallet::new(signer, config).await {
        Ok(w) => w,
        Err(e) => {
            let _ = event_tx
                .send(WalletEvent::InitFailed(format!(
                    "Failed to initialize wallet: {}",
                    e
                )))
                .await;
            return;
        }
    };

    // Get initial balance and address
    let balance = wallet.get_balance().await.unwrap_or_default();
    let spark_address = wallet.get_spark_address().await.unwrap_or_default();

    let _ = event_tx
        .send(WalletEvent::Initialized {
            balance_sats: balance.total_sats(),
            _spark_address: spark_address,
        })
        .await;

    // Track last known payments to detect new ones
    let mut known_payment_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Initialize known payments
    if let Ok(payments) = wallet.list_payments(Some(50), None).await {
        for payment in payments {
            known_payment_ids.insert(payment.id.clone());
        }
    }

    // Process commands
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            WalletCommand::CreateInvoice { job_id, description } => {
                handle_create_invoice(&wallet, &event_tx, &job_id, &description).await;
            }
            WalletCommand::PayInvoice { bolt11 } => {
                handle_pay_invoice(&wallet, &event_tx, &bolt11).await;
            }
            WalletCommand::GetBalance => {
                handle_get_balance(&wallet, &event_tx).await;
            }
            WalletCommand::PollPayments => {
                handle_poll_payments(&wallet, &event_tx, &mut known_payment_ids).await;
            }
        }
    }
}

/// Handle invoice creation
async fn handle_create_invoice(
    wallet: &SparkWallet,
    event_tx: &mpsc::Sender<WalletEvent>,
    job_id: &str,
    description: &str,
) {
    match wallet
        .create_invoice(SATS_PER_JOB, Some(description.to_string()), Some(3600))
        .await
    {
        Ok(response) => {
            let _ = event_tx
                .send(WalletEvent::InvoiceCreated {
                    job_id: job_id.to_string(),
                    bolt11: response.payment_request,
                    amount_sats: SATS_PER_JOB,
                })
                .await;
        }
        Err(e) => {
            let _ = event_tx
                .send(WalletEvent::InvoiceCreationFailed {
                    job_id: job_id.to_string(),
                    error: e.to_string(),
                })
                .await;
        }
    }
}

/// Handle paying an invoice
async fn handle_pay_invoice(
    wallet: &SparkWallet,
    event_tx: &mpsc::Sender<WalletEvent>,
    bolt11: &str,
) {
    match wallet.send_payment_simple(bolt11, None).await {
        Ok(response) => {
            let _ = event_tx
                .send(WalletEvent::PaymentSent {
                    _payment_id: response.payment.id.clone(),
                    amount_sats: response.payment.amount as u64,
                })
                .await;

            // Update balance after payment
            if let Ok(balance) = wallet.get_balance().await {
                let _ = event_tx
                    .send(WalletEvent::BalanceUpdated {
                        balance_sats: balance.total_sats(),
                    })
                    .await;
            }
        }
        Err(e) => {
            let _ = event_tx
                .send(WalletEvent::PaymentFailed {
                    error: e.to_string(),
                })
                .await;
        }
    }
}

/// Handle balance query
async fn handle_get_balance(wallet: &SparkWallet, event_tx: &mpsc::Sender<WalletEvent>) {
    if let Ok(balance) = wallet.get_balance().await {
        let _ = event_tx
            .send(WalletEvent::BalanceUpdated {
                balance_sats: balance.total_sats(),
            })
            .await;
    }
}

/// Handle polling for incoming payments
async fn handle_poll_payments(
    wallet: &SparkWallet,
    event_tx: &mpsc::Sender<WalletEvent>,
    known_payment_ids: &mut std::collections::HashSet<String>,
) {
    // Get recent payments
    if let Ok(payments) = wallet.list_payments(Some(20), None).await {
        for payment in payments {
            // Check if this is a new completed incoming payment
            if !known_payment_ids.contains(&payment.id) {
                known_payment_ids.insert(payment.id.clone());

                // Only notify about completed incoming payments
                if payment.payment_type == PaymentType::Receive
                    && payment.status == PaymentStatus::Completed
                {
                    let amount_sats = payment.amount as u64;
                    let _ = event_tx
                        .send(WalletEvent::PaymentReceived {
                            _payment_id: payment.id,
                            amount_sats,
                        })
                        .await;
                }
            }
        }
    }

    // Always update balance after polling
    if let Ok(balance) = wallet.get_balance().await {
        let _ = event_tx
            .send(WalletEvent::BalanceUpdated {
                balance_sats: balance.total_sats(),
            })
            .await;
    }
}
