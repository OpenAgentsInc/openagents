//! Spark wallet wrapper for the Breez SDK
//!
//! This module provides a high-level wallet interface for Bitcoin/Lightning payments
//! through the Breez Spark SDK. It wraps the underlying SDK to provide a simpler API
//! for OpenAgents applications.
//!
//! # Architecture
//!
//! ```text
//! SparkWallet
//!   ├─ SparkSigner (our BIP44 key derivation)
//!   ├─ Breez SDK Client (network ops)
//!   └─ Local persistence (wallet state)
//! ```

use crate::{SparkSigner, SparkError};
use breez_sdk_spark::{
    BreezSdk, connect, default_config,
    ConnectRequest, Network as SdkNetwork, Seed,
    GetInfoRequest,
    PrepareSendPaymentRequest, SendPaymentRequest,
    ReceivePaymentRequest, ReceivePaymentMethod,
    ListPaymentsRequest,
    SyncWalletRequest,
    EventListener,
    ClaimHtlcPaymentRequest,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Re-export SDK types that consumers need
pub use breez_sdk_spark::{
    PrepareSendPaymentResponse,
    SendPaymentMethod,
    SendPaymentResponse,
    SendPaymentOptions,
    ReceivePaymentResponse,
    Payment,
    PaymentStatus,
    PaymentType,
    PaymentMethod,
    PaymentDetails,
    BitcoinAddressDetails,
    BitcoinNetwork,
    PaymentRequestSource,
    Bolt11InvoiceDetails,
    Bolt11Invoice,
    SparkInvoiceDetails,
    SparkInvoicePaymentDetails,
    SparkHtlcDetails,
    SparkHtlcStatus,
    SparkHtlcOptions,
    ClaimHtlcPaymentResponse,
    SendOnchainFeeQuote,
    SendOnchainSpeedFeeQuote,
};

/// Bitcoin network to use for Spark wallet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Network {
    /// Bitcoin mainnet
    Mainnet,
    /// Bitcoin testnet
    #[default]
    Testnet,
    /// Bitcoin signet (staging)
    Signet,
    /// Bitcoin regtest (local development)
    Regtest,
}

impl Network {
    /// Convert to SDK network type
    pub fn to_sdk_network(self) -> SdkNetwork {
        match self {
            Network::Mainnet => SdkNetwork::Mainnet,
            // The Breez SDK only supports Mainnet and Regtest.
            // All test networks (Testnet, Signet, Regtest) map to SdkNetwork::Regtest.
            Network::Testnet | Network::Signet | Network::Regtest => SdkNetwork::Regtest,
        }
    }
}

/// Wallet balance information
///
/// Represents the total balance across all layers of the Spark wallet:
/// Spark Layer 2, Lightning Network, and on-chain Bitcoin.
///
/// # Examples
///
/// ```
/// use openagents_spark::Balance;
///
/// let balance = Balance {
///     spark_sats: 100_000,
///     lightning_sats: 50_000,
///     onchain_sats: 0,
/// };
///
/// assert_eq!(balance.total_sats(), 150_000);
/// assert!(!balance.is_empty());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Balance {
    /// Spark layer 2 balance in satoshis
    pub spark_sats: u64,
    /// Lightning balance in satoshis
    pub lightning_sats: u64,
    /// On-chain balance in satoshis (funds in cooperative exit)
    pub onchain_sats: u64,
}

impl Balance {
    /// Get total balance across all layers
    pub fn total_sats(&self) -> u64 {
        self.spark_sats.saturating_add(self.lightning_sats).saturating_add(self.onchain_sats)
    }

    /// Check if wallet has any funds
    pub fn is_empty(&self) -> bool {
        self.total_sats() == 0
    }
}

/// Wallet information and status
///
/// # Examples
///
/// ```
/// use openagents_spark::{WalletInfo, Network};
///
/// let info = WalletInfo {
///     public_key: "02a1b2c3...".to_string(),
///     network: Network::Testnet,
///     synced: true,
///     pending_ops: 0,
/// };
///
/// assert!(info.synced);
/// assert_eq!(info.pending_ops, 0);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    /// Public key of the wallet (for receiving)
    pub public_key: String,
    /// Network the wallet is operating on
    pub network: Network,
    /// Whether wallet is synced with operators
    pub synced: bool,
    /// Number of pending operations
    pub pending_ops: u32,
}

/// Network connectivity status for the Spark wallet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkStatus {
    Connected,
    Disconnected,
}

impl NetworkStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            NetworkStatus::Connected => "Connected",
            NetworkStatus::Disconnected => "Disconnected",
        }
    }
}

/// Network status result with optional detail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatusReport {
    pub status: NetworkStatus,
    pub detail: Option<String>,
}

impl NetworkStatusReport {
    pub fn connected() -> Self {
        Self {
            status: NetworkStatus::Connected,
            detail: None,
        }
    }

    pub fn disconnected(detail: Option<String>) -> Self {
        Self {
            status: NetworkStatus::Disconnected,
            detail,
        }
    }
}

/// Configuration for initializing a Spark wallet
///
/// # API Key Requirements
///
/// - **Mainnet**: API key is **required** (get from Breez)
/// - **Regtest/Testnet**: API key is **optional** (for local testing)
///
/// # Examples
///
/// ```
/// use openagents_spark::{WalletConfig, Network};
/// use std::path::PathBuf;
///
/// // Default: Testnet, no API key needed
/// let config = WalletConfig::default();
/// assert_eq!(config.network, Network::Testnet);
///
/// // Production: Mainnet requires API key
/// let config = WalletConfig {
///     network: Network::Mainnet,
///     api_key: Some("your-breez-api-key".to_string()),
///     storage_dir: PathBuf::from("/var/lib/openagents/spark"),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct WalletConfig {
    /// Network to operate on
    pub network: Network,
    /// Breez API key (required for Mainnet, optional for Regtest)
    pub api_key: Option<String>,
    /// Storage directory for wallet data
    pub storage_dir: std::path::PathBuf,
}

impl Default for WalletConfig {
    fn default() -> Self {
        Self {
            network: Network::Testnet,
            api_key: None,
            storage_dir: dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("openagents")
                .join("spark"),
        }
    }
}

/// Spark wallet for Bitcoin/Lightning payments
///
/// This wraps the Breez SDK to provide self-custodial Bitcoin payments
/// via Lightning, Spark Layer 2, and on-chain.
///
/// # Examples
///
/// ```no_run
/// use openagents_spark::{SparkSigner, SparkWallet, WalletConfig, Network};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create signer from mnemonic
/// let signer = SparkSigner::from_mnemonic(
///     "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
///     ""
/// )?;
///
/// // Configure wallet
/// let config = WalletConfig {
///     network: Network::Testnet,
///     ..Default::default()
/// };
///
/// // Initialize wallet
/// let wallet = SparkWallet::new(signer, config).await?;
///
/// // Create invoice to receive 1000 sats
/// let invoice = wallet.create_invoice(
///     1000,
///     Some("Coffee payment".to_string()),
///     None
/// ).await?;
/// println!("Pay this invoice: {}", invoice.payment_request);
///
/// // Send payment
/// let payment = wallet.send_payment_simple(
///     "lnbc1...",  // Lightning invoice
///     None
/// ).await?;
/// println!("Payment sent: {}", payment.payment.id);
/// # Ok(())
/// # }
/// ```
pub struct SparkWallet {
    signer: SparkSigner,
    config: WalletConfig,
    #[allow(dead_code)]
    sdk: Arc<BreezSdk>,
}

impl SparkWallet {
    /// Create a new Spark wallet with the given signer and configuration
    ///
    /// This method initializes the Breez SDK with the wallet's mnemonic and configuration,
    /// connecting to the Spark network and starting background synchronization.
    ///
    /// # Arguments
    /// * `signer` - The SparkSigner for signing transactions
    /// * `config` - Wallet configuration (network, API key, storage)
    ///
    /// # Example
    /// ```rust,ignore
    /// use spark::{SparkSigner, SparkWallet, WalletConfig, Network};
    ///
    /// let signer = SparkSigner::from_mnemonic("your mnemonic here", "")?;
    /// let config = WalletConfig {
    ///     network: Network::Testnet,
    ///     ..Default::default()
    /// };
    /// let wallet = SparkWallet::new(signer, config).await?;
    /// ```
    pub async fn new(signer: SparkSigner, config: WalletConfig) -> Result<Self, SparkError> {
        // Create SDK config for the target network
        let mut sdk_config = default_config(config.network.to_sdk_network());

        // Set API key if provided
        if config.api_key.is_some() {
            sdk_config.api_key = config.api_key.clone();
        } else {
            // Disable real-time sync when no API key is provided
            // This prevents "invalid auth header" errors on regtest
            sdk_config.real_time_sync_server_url = None;
        }

        // Build connect request with mnemonic seed
        let connect_request = ConnectRequest {
            config: sdk_config,
            seed: Seed::Mnemonic {
                mnemonic: signer.mnemonic().to_string(),
                passphrase: None,
            },
            storage_dir: config.storage_dir.to_string_lossy().to_string(),
        };

        // Connect to the Breez SDK
        let sdk = connect(connect_request)
            .await
            .map_err(|e| SparkError::InitializationFailed(e.to_string()))?;

        Ok(Self {
            signer,
            config,
            sdk: Arc::new(sdk),
        })
    }

    /// Get the wallet's Spark address for receiving payments
    ///
    /// This calls the Breez SDK's receive_payment API with SparkAddress method
    /// to get a properly formatted Spark address string.
    pub async fn get_spark_address(&self) -> Result<String, SparkError> {
        let request = ReceivePaymentRequest {
            payment_method: ReceivePaymentMethod::SparkAddress,
        };

        let response = self.sdk.receive_payment(request)
            .await
            .map_err(|e| SparkError::GetAddressFailed(e.to_string()))?;

        Ok(response.payment_request)
    }

    /// Get a Bitcoin on-chain deposit address for funding the wallet
    ///
    /// This requests a static deposit address from the SDK. Use this when
    /// funding wallets via regtest faucets or on-chain deposits.
    pub async fn get_bitcoin_address(&self) -> Result<String, SparkError> {
        let request = ReceivePaymentRequest {
            payment_method: ReceivePaymentMethod::BitcoinAddress,
        };

        let response = self.sdk.receive_payment(request)
            .await
            .map_err(|e| SparkError::GetAddressFailed(e.to_string()))?;

        Ok(response.payment_request)
    }

    /// Get the underlying signer
    pub fn signer(&self) -> &SparkSigner {
        &self.signer
    }

    /// Get the wallet configuration
    pub fn config(&self) -> &WalletConfig {
        &self.config
    }

    /// Get the Breez SDK instance
    ///
    /// This provides direct access to the underlying Breez SDK for advanced operations.
    #[allow(dead_code)]
    pub(crate) fn sdk(&self) -> &Arc<BreezSdk> {
        &self.sdk
    }

    /// Register an SDK event listener
    pub async fn add_event_listener(
        &self,
        listener: Box<dyn EventListener>,
    ) -> Result<String, SparkError> {
        Ok(self.sdk.add_event_listener(listener).await)
    }

    /// Remove a previously registered event listener
    pub async fn remove_event_listener(&self, id: &str) -> Result<bool, SparkError> {
        Ok(self.sdk.remove_event_listener(id).await)
    }

    /// Get the current wallet balance
    ///
    /// Returns the total balance across all layers (Spark Layer 2, Lightning, on-chain).
    /// Note: Currently only returns Spark Layer 2 balance from the SDK. Lightning and
    /// on-chain balances require additional SDK integration.
    ///
    /// # Example
    /// ```rust,ignore
    /// let balance = wallet.get_balance().await?;
    /// println!("Total: {} sats", balance.total_sats());
    /// ```
    pub async fn get_balance(&self) -> Result<Balance, SparkError> {
        let request = GetInfoRequest {
            ensure_synced: Some(true),
        };

        let info = self.sdk.get_info(request)
            .await
            .map_err(|e| SparkError::BalanceQueryFailed(e.to_string()))?;

        Ok(Balance {
            spark_sats: info.balance_sats,
            lightning_sats: 0, // Spark SDK handles Lightning internally
            onchain_sats: 0,   // On-chain shown separately via deposits
        })
    }

    /// Check network connectivity by forcing a sync with a timeout
    pub async fn network_status(&self, timeout: Duration) -> NetworkStatusReport {
        let request = SyncWalletRequest {};
        match tokio::time::timeout(timeout, self.sdk.sync_wallet(request)).await {
            Ok(Ok(_)) => NetworkStatusReport::connected(),
            Ok(Err(err)) => NetworkStatusReport::disconnected(Some(err.to_string())),
            Err(_) => NetworkStatusReport::disconnected(Some(format!(
                "Timed out after {} seconds",
                timeout.as_secs()
            ))),
        }
    }

    /// Prepare a payment by validating the payment request and calculating fees
    ///
    /// This method validates the payment request (Lightning invoice, Spark address, etc.)
    /// and returns fee information before executing the payment.
    ///
    /// # Arguments
    /// * `payment_request` - A Lightning invoice (BOLT-11), Spark address, or other payment identifier
    /// * `amount` - Optional amount in satoshis (required for zero-amount invoices)
    ///
    /// # Returns
    /// A `PrepareSendPaymentResponse` containing payment details and fee estimate
    ///
    /// # Example
    /// ```rust,ignore
    /// let prepare_response = wallet
    ///     .prepare_send_payment("lnbc1...", None)
    ///     .await?;
    /// println!("Fee: {} sats", prepare_response.fee_sat);
    /// ```
    pub async fn prepare_send_payment(
        &self,
        payment_request: &str,
        amount: Option<u64>,
    ) -> Result<PrepareSendPaymentResponse, SparkError> {
        let request = PrepareSendPaymentRequest {
            payment_request: payment_request.to_string(),
            amount: amount.map(|a| a as u128),
            token_identifier: None,
        };

        self.sdk.prepare_send_payment(request)
            .await
            .map_err(|e| SparkError::PaymentFailed(e.to_string()))
    }

    /// Send a payment using Lightning or Spark
    ///
    /// Executes a payment that was previously prepared with `prepare_send_payment`.
    /// This method will attempt to route the payment through Lightning or Spark Layer 2.
    ///
    /// # Arguments
    /// * `prepare_response` - The response from `prepare_send_payment`
    /// * `idempotency_key` - Optional UUID for idempotent payment submission
    ///
    /// # Returns
    /// A `SendPaymentResponse` containing the payment details and status
    ///
    /// # Example
    /// ```rust,ignore
    /// let prepare_response = wallet.prepare_send_payment("lnbc1...", None).await?;
    /// let payment_response = wallet.send_payment(prepare_response, None).await?;
    /// println!("Payment ID: {}", payment_response.payment.id);
    /// ```
    pub async fn send_payment(
        &self,
        prepare_response: PrepareSendPaymentResponse,
        idempotency_key: Option<String>,
    ) -> Result<SendPaymentResponse, SparkError> {
        let request = SendPaymentRequest {
            prepare_response,
            options: None,
            idempotency_key,
        };

        self.sdk.send_payment(request)
            .await
            .map_err(|e| SparkError::PaymentFailed(e.to_string()))
    }

    /// Send a payment with explicit options (Spark HTLC, Spark address, etc.)
    pub async fn send_payment_with_options(
        &self,
        prepare_response: PrepareSendPaymentResponse,
        options: SendPaymentOptions,
        idempotency_key: Option<String>,
    ) -> Result<SendPaymentResponse, SparkError> {
        let request = SendPaymentRequest {
            prepare_response,
            options: Some(options),
            idempotency_key,
        };

        self.sdk.send_payment(request)
            .await
            .map_err(|e| SparkError::PaymentFailed(e.to_string()))
    }

    /// Send a payment in one step (prepare + send)
    ///
    /// This is a convenience method that combines `prepare_send_payment` and `send_payment`.
    ///
    /// # Arguments
    /// * `payment_request` - A Lightning invoice (BOLT-11), Spark address, or other payment identifier
    /// * `amount` - Optional amount in satoshis (required for zero-amount invoices)
    ///
    /// # Returns
    /// A `SendPaymentResponse` containing the payment details and status
    ///
    /// # Example
    /// ```rust,ignore
    /// let payment_response = wallet
    ///     .send_payment_simple("lnbc1...", None)
    ///     .await?;
    /// println!("Sent payment: {}", payment_response.payment.id);
    /// ```
    pub async fn send_payment_simple(
        &self,
        payment_request: &str,
        amount: Option<u64>,
    ) -> Result<SendPaymentResponse, SparkError> {
        let prepare_response = self.prepare_send_payment(payment_request, amount).await?;
        self.send_payment(prepare_response, None).await
    }

    /// Send a Spark HTLC transfer for escrow-style payments.
    ///
    /// The receiver must provide the preimage before expiry to claim the funds.
    pub async fn send_htlc_payment(
        &self,
        payment_request: &str,
        amount_sats: u64,
        payment_hash: &str,
        expiry_duration_secs: u64,
        idempotency_key: Option<String>,
    ) -> Result<SendPaymentResponse, SparkError> {
        let prepare_response = self
            .prepare_send_payment(payment_request, Some(amount_sats))
            .await?;

        let options = SendPaymentOptions::SparkAddress {
            htlc_options: Some(SparkHtlcOptions {
                payment_hash: payment_hash.to_string(),
                expiry_duration_secs,
            }),
        };

        self.send_payment_with_options(prepare_response, options, idempotency_key)
            .await
    }

    /// Claim an HTLC transfer by providing the preimage.
    pub async fn claim_htlc_payment(
        &self,
        preimage: &str,
    ) -> Result<ClaimHtlcPaymentResponse, SparkError> {
        let request = ClaimHtlcPaymentRequest {
            preimage: preimage.to_string(),
        };

        self.sdk.claim_htlc_payment(request)
            .await
            .map_err(|e| SparkError::PaymentFailed(e.to_string()))
    }

    /// Get the wallet's Spark address for receiving payments
    ///
    /// Returns a static Spark address that can be used to receive payments.
    /// This address is tied to the wallet's identity and can be reused.
    ///
    /// # Returns
    /// A `ReceivePaymentResponse` containing the Spark address
    ///
    /// # Example
    /// ```rust,ignore
    /// let response = wallet.get_spark_address().await?;
    /// println!("Send to: {}", response.payment_request);
    /// ```
    pub async fn get_receive_address(&self) -> Result<ReceivePaymentResponse, SparkError> {
        let request = ReceivePaymentRequest {
            payment_method: ReceivePaymentMethod::SparkAddress,
        };

        self.sdk.receive_payment(request)
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Create a Spark invoice for receiving a specific amount
    ///
    /// Generates a Spark invoice (similar to Lightning BOLT-11) for receiving payments.
    ///
    /// # Arguments
    /// * `amount_sats` - Amount to receive in satoshis
    /// * `description` - Optional description to embed in the invoice
    /// * `expiry_seconds` - Optional expiry time in seconds from now
    ///
    /// # Returns
    /// A `ReceivePaymentResponse` containing the Spark invoice
    ///
    /// # Example
    /// ```rust,ignore
    /// let response = wallet
    ///     .create_invoice(1000, Some("Coffee".to_string()), None)
    ///     .await?;
    /// println!("Invoice: {}", response.payment_request);
    /// println!("Fee: {} sats", response.fee);
    /// ```
    pub async fn create_invoice(
        &self,
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u64>,
    ) -> Result<ReceivePaymentResponse, SparkError> {
        let request = build_receive_request(amount_sats, description, expiry_seconds)
            .map_err(|e| SparkError::Wallet(e.to_string()))?;

        self.sdk.receive_payment(request)
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Create a Lightning invoice for receiving a specific amount
    ///
    /// This is an alias for `create_invoice` that provides Lightning-style naming.
    ///
    /// # Arguments
    /// * `amount_sats` - Amount to receive in satoshis
    /// * `description` - Optional description to embed in the invoice
    ///
    /// # Returns
    /// A `ReceivePaymentResponse` containing the invoice
    pub async fn create_lightning_invoice(
        &self,
        amount_sats: u64,
        description: Option<String>,
    ) -> Result<ReceivePaymentResponse, SparkError> {
        self.create_invoice(amount_sats, description, None).await
    }

    /// List payment history
    ///
    /// Returns a list of payments with optional filtering and pagination.
    ///
    /// # Arguments
    /// * `limit` - Maximum number of payments to return
    /// * `offset` - Number of payments to skip (for pagination)
    ///
    /// # Returns
    /// A vector of `Payment` objects
    ///
    /// # Example
    /// ```rust,ignore
    /// let payments = wallet.list_payments(20, 0).await?;
    /// for payment in payments {
    ///     println!("{}: {} sats", payment.id, payment.amount_sat);
    /// }
    /// ```
    pub async fn list_payments(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<Payment>, SparkError> {
        let request = ListPaymentsRequest {
            limit,
            offset,
            sort_ascending: Some(false), // newest first
            ..Default::default()
        };

        let response = self.sdk.list_payments(request)
            .await
            .map_err(|e| SparkError::Wallet(format!("Failed to list payments: {}", e)))?;

        Ok(response.payments)
    }

    /// List Spark HTLC payments with optional status filtering.
    pub async fn list_htlc_payments(
        &self,
        status_filter: Option<Vec<SparkHtlcStatus>>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<Payment>, SparkError> {
        let filter = status_filter.unwrap_or_else(|| {
            vec![
                SparkHtlcStatus::WaitingForPreimage,
                SparkHtlcStatus::PreimageShared,
                SparkHtlcStatus::Returned,
            ]
        });
        let request = ListPaymentsRequest {
            spark_htlc_status_filter: Some(filter),
            limit,
            offset,
            sort_ascending: Some(false),
            ..Default::default()
        };

        let response = self.sdk.list_payments(request)
            .await
            .map_err(|e| SparkError::Wallet(format!("Failed to list HTLC payments: {}", e)))?;

        Ok(response.payments)
    }
}

fn build_receive_request(
    amount_sats: u64,
    description: Option<String>,
    expiry_seconds: Option<u64>,
) -> Result<ReceivePaymentRequest, String> {
    let expiry_time = match expiry_seconds {
        Some(seconds) => Some(compute_expiry_time(seconds)?),
        None => None,
    };

    Ok(ReceivePaymentRequest {
        payment_method: ReceivePaymentMethod::SparkInvoice {
            amount: Some(amount_sats as u128),
            token_identifier: None,
            expiry_time,
            description,
            sender_public_key: None,
        },
    })
}

fn compute_expiry_time(expiry_seconds: u64) -> Result<u64, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("System time error: {}", e))?;
    now.as_secs()
        .checked_add(expiry_seconds)
        .ok_or_else(|| "Invoice expiry time overflowed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_receive_request_with_expiry() {
        let description = Some("Coffee".to_string());
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_secs();
        let request = build_receive_request(4_200, description.clone(), Some(60))
            .expect("build request");
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_secs();

        match request.payment_method {
            ReceivePaymentMethod::SparkInvoice {
                amount,
                token_identifier,
                expiry_time,
                description: request_description,
                sender_public_key,
            } => {
                assert_eq!(amount, Some(4_200u128));
                assert_eq!(token_identifier, None);
                assert_eq!(request_description, description);
                assert_eq!(sender_public_key, None);

                let expiry_time = expiry_time.expect("expiry time");
                assert!(expiry_time >= before + 60);
                assert!(expiry_time <= after + 60);
            }
            _ => panic!("expected SparkInvoice request"),
        }
    }

    #[test]
    fn test_build_receive_request_without_expiry() {
        let request = build_receive_request(1, None, None).expect("build request");
        match request.payment_method {
            ReceivePaymentMethod::SparkInvoice {
                amount,
                expiry_time,
                description,
                ..
            } => {
                assert_eq!(amount, Some(1u128));
                assert!(expiry_time.is_none());
                assert!(description.is_none());
            }
            _ => panic!("expected SparkInvoice request"),
        }
    }

    #[tokio::test]
    #[ignore] // Requires network connection and API key
    async fn test_wallet_creation() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = SparkSigner::from_mnemonic(mnemonic, "").expect("should create signer");
        let config = WalletConfig::default();

        let _wallet = SparkWallet::new(signer, config).await.expect("should create wallet");
    }

    #[tokio::test]
    async fn test_balance_operations() {
        let balance = Balance {
            spark_sats: 100,
            lightning_sats: 200,
            onchain_sats: 300,
        };

        assert_eq!(balance.total_sats(), 600);
        assert!(!balance.is_empty());

        let empty = Balance::default();
        assert!(empty.is_empty());
    }
}
