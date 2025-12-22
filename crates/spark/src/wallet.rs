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
    BreezSdk, ConnectRequest, Network as SdkNetwork, PrepareSendPaymentRequest,
    PrepareSendPaymentResponse, ReceivePaymentMethod, ReceivePaymentRequest,
    ReceivePaymentResponse, SendPaymentRequest, SendPaymentResponse, Seed,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

impl From<Network> for SdkNetwork {
    fn from(network: Network) -> Self {
        match network {
            Network::Mainnet => SdkNetwork::Mainnet,
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

/// Configuration for initializing a Spark wallet
///
/// # Examples
///
/// ```
/// use openagents_spark::{WalletConfig, Network};
/// use std::path::PathBuf;
///
/// // Use default configuration (testnet, default storage)
/// let config = WalletConfig::default();
/// assert_eq!(config.network, Network::Testnet);
///
/// // Custom configuration for production
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
    /// Optional Breez API key (required for production)
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
        // Create storage directory if it doesn't exist
        std::fs::create_dir_all(&config.storage_dir)
            .map_err(|e| SparkError::InitializationFailed(format!("Failed to create storage directory: {}", e)))?;

        // Convert our network enum to SDK network
        let sdk_network: SdkNetwork = config.network.into();

        // Create SDK config with default settings for the network
        let mut sdk_config = breez_sdk_spark::default_config(sdk_network);

        // Apply API key if provided
        if let Some(api_key) = &config.api_key {
            sdk_config.api_key = Some(api_key.clone());
        }

        // Get mnemonic and passphrase from signer
        let mnemonic = signer.mnemonic();
        let passphrase = signer.passphrase();

        // Create seed from mnemonic
        let seed = Seed::Mnemonic {
            mnemonic: mnemonic.to_string(),
            passphrase: if passphrase.is_empty() {
                None
            } else {
                Some(passphrase.to_string())
            },
        };

        // Build connect request
        let connect_request = ConnectRequest {
            config: sdk_config,
            seed,
            storage_dir: config.storage_dir.to_string_lossy().to_string(),
        };

        // Connect to Breez SDK
        let sdk = breez_sdk_spark::connect(connect_request)
            .await
            .map_err(|e| SparkError::InitializationFailed(format!("Failed to connect to Breez SDK: {}", e)))?;

        Ok(Self {
            signer,
            config,
            sdk: Arc::new(sdk),
        })
    }

    /// Get the wallet's Spark address for receiving payments
    ///
    /// **NOTE**: Currently returns the public key hex as a placeholder.
    /// Proper Spark address generation requires Breez SDK integration (d-001).
    /// Spark addresses have a specific format for the protocol.
    pub fn get_spark_address(&self) -> String {
        self.signer.public_key_hex()
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
    pub fn sdk(&self) -> &Arc<BreezSdk> {
        &self.sdk
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

        self.sdk
            .prepare_send_payment(request)
            .await
            .map_err(|e| SparkError::PaymentFailed(format!("Failed to prepare payment: {}", e)))
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

        self.sdk
            .send_payment(request)
            .await
            .map_err(|e| SparkError::PaymentFailed(format!("Failed to send payment: {}", e)))
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

        self.sdk
            .receive_payment(request)
            .await
            .map_err(|e| SparkError::Wallet(format!("Failed to get Spark address: {}", e)))
    }

    /// Create a Spark invoice for receiving a specific amount
    ///
    /// Generates a Spark invoice (similar to Lightning BOLT-11) for receiving payments.
    ///
    /// # Arguments
    /// * `amount_sats` - Amount to receive in satoshis
    /// * `description` - Optional description to embed in the invoice
    /// * `expiry_seconds` - Optional expiry time in seconds (default: 3600)
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
        let request = ReceivePaymentRequest {
            payment_method: ReceivePaymentMethod::SparkInvoice {
                amount: Some(amount_sats as u128),
                token_identifier: None,
                expiry_time: expiry_seconds,
                description,
                sender_public_key: None,
            },
        };

        self.sdk
            .receive_payment(request)
            .await
            .map_err(|e| SparkError::Wallet(format!("Failed to create invoice: {}", e)))
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wallet_creation() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = SparkSigner::from_mnemonic(mnemonic, "").expect("should create signer");
        let config = WalletConfig::default();

        let wallet = SparkWallet::new(signer, config).await.expect("should create wallet");
        assert_eq!(wallet.config().network, Network::Testnet);
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
