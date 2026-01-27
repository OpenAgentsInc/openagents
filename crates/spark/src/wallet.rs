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

use crate::{SparkError, SparkSigner};
use breez_sdk_spark::{
    BreezSdk, ClaimHtlcPaymentRequest, EventListener, Network as SdkNetwork,
    PrepareSendPaymentRequest, SdkBuilder, Seed, Storage, default_config,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Re-export SDK types that consumers need
pub use breez_sdk_spark::{
    BitcoinAddressDetails, BitcoinNetwork, Bolt11Invoice, Bolt11InvoiceDetails,
    CheckLightningAddressRequest, CheckMessageRequest, CheckMessageResponse, ClaimDepositRequest,
    ClaimDepositResponse, ClaimHtlcPaymentResponse, Config, DepositInfo, ExternalInputParser, Fee,
    GetInfoRequest, GetInfoResponse, GetPaymentRequest, GetPaymentResponse,
    GetTokensMetadataRequest, GetTokensMetadataResponse, InputType, KeySetType,
    LightningAddressInfo, ListFiatCurrenciesResponse, ListFiatRatesResponse, ListPaymentsRequest,
    ListPaymentsResponse, ListUnclaimedDepositsRequest, ListUnclaimedDepositsResponse,
    LnurlPayRequest, LnurlPayResponse, LnurlWithdrawRequest, LnurlWithdrawResponse, MaxFee,
    OptimizationConfig, OptimizationProgress, Payment, PaymentDetails, PaymentMethod,
    PaymentRequestSource, PaymentStatus, PaymentType, PrepareLnurlPayRequest,
    PrepareLnurlPayResponse, PrepareSendPaymentResponse, ReceivePaymentMethod,
    ReceivePaymentRequest, ReceivePaymentResponse, RecommendedFees, RefundDepositRequest,
    RefundDepositResponse, RegisterLightningAddressRequest, SendOnchainFeeQuote,
    SendOnchainSpeedFeeQuote, SendPaymentMethod, SendPaymentOptions, SendPaymentRequest,
    SendPaymentResponse, SignMessageRequest, SignMessageResponse, SparkHtlcDetails,
    SparkHtlcOptions, SparkHtlcStatus, SparkInvoiceDetails, SparkInvoicePaymentDetails,
    SyncWalletRequest, SyncWalletResponse, TokenBalance, TokenIssuer, TokenMetadata,
    UpdateUserSettingsRequest, UserSettings,
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
        self.spark_sats
            .saturating_add(self.lightning_sats)
            .saturating_add(self.onchain_sats)
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
        #[cfg(all(target_family = "wasm", target_os = "unknown"))]
        let storage_dir = std::path::PathBuf::from("openagents").join("spark");
        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        let storage_dir = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("openagents")
            .join("spark");

        Self {
            network: Network::Testnet,
            api_key: None,
            storage_dir,
        }
    }
}

/// Builder for advanced Spark wallet configuration
pub struct SparkWalletBuilder {
    signer: SparkSigner,
    config: WalletConfig,
    sdk_config: Option<Config>,
    key_set_type: Option<KeySetType>,
    use_address_index: bool,
    account_number: Option<u32>,
    storage: Option<Arc<dyn Storage>>,
}

impl SparkWalletBuilder {
    pub fn new(signer: SparkSigner, config: WalletConfig) -> Self {
        Self {
            signer,
            config,
            sdk_config: None,
            key_set_type: None,
            use_address_index: false,
            account_number: None,
            storage: None,
        }
    }

    /// Override the Breez SDK config (advanced use)
    pub fn with_sdk_config(mut self, sdk_config: Config) -> Self {
        self.sdk_config = Some(sdk_config);
        self
    }

    /// Override the key set and derivation path selection
    pub fn with_key_set(
        mut self,
        key_set_type: KeySetType,
        use_address_index: bool,
        account_number: Option<u32>,
    ) -> Self {
        self.key_set_type = Some(key_set_type);
        self.use_address_index = use_address_index;
        self.account_number = account_number;
        self
    }

    pub fn with_storage(mut self, storage: Arc<dyn Storage>) -> Self {
        self.storage = Some(storage);
        self
    }

    pub async fn build(self) -> Result<SparkWallet, SparkError> {
        let seed = seed_from_signer(&self.signer)?;

        let mut sdk_config = match self.sdk_config {
            Some(config) => {
                let expected = self.config.network.to_sdk_network();
                // breez_sdk_spark::Network does not implement PartialEq.
                if std::mem::discriminant(&config.network) != std::mem::discriminant(&expected) {
                    return Err(SparkError::InitializationFailed(format!(
                        "SDK config network {:?} does not match wallet network {:?}",
                        config.network, expected
                    )));
                }
                config
            }
            None => {
                let mut config = default_config(self.config.network.to_sdk_network());
                if self.config.api_key.is_some() {
                    config.api_key = self.config.api_key.clone();
                } else {
                    config.real_time_sync_server_url = None;
                }
                config
            }
        };

        if sdk_config.api_key.is_none() {
            sdk_config.api_key = self.config.api_key.clone();
        }

        #[cfg(all(target_family = "wasm", target_os = "unknown"))]
        {
            // Web builds use in-memory storage and disable realtime sync by default.
            sdk_config.real_time_sync_server_url = None;
        }

        let mut builder = SdkBuilder::new(sdk_config, seed);

        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        {
            builder =
                builder.with_default_storage(self.config.storage_dir.to_string_lossy().to_string());
        }

        #[cfg(all(target_family = "wasm", target_os = "unknown"))]
        {
            let storage = self
                .storage
                .unwrap_or_else(|| Arc::new(crate::wasm_storage::MemoryStorage::new()));
            builder = builder.with_storage(storage);
        }

        let configure_key_set =
            self.key_set_type.is_some() || self.account_number.is_some() || self.use_address_index;
        if configure_key_set {
            builder = builder.with_key_set(
                self.key_set_type.unwrap_or(KeySetType::Default),
                self.use_address_index,
                self.account_number,
            );
        }

        let sdk = builder
            .build()
            .await
            .map_err(|e| SparkError::InitializationFailed(e.to_string()))?;

        Ok(SparkWallet {
            signer: self.signer,
            config: self.config,
            sdk: Arc::new(sdk),
        })
    }
}

fn seed_from_signer(signer: &SparkSigner) -> Result<Seed, SparkError> {
    if !signer.mnemonic().is_empty() {
        let passphrase = if signer.passphrase().is_empty() {
            None
        } else {
            Some(signer.passphrase().to_string())
        };
        return Ok(Seed::Mnemonic {
            mnemonic: signer.mnemonic().to_string(),
            passphrase,
        });
    }

    if let Some(entropy) = signer.seed_entropy() {
        return Ok(Seed::Entropy(entropy.to_vec()));
    }

    Err(SparkError::InitializationFailed(
        "Missing seed material for Spark signer".to_string(),
    ))
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
    #[expect(dead_code)]
    sdk: Arc<BreezSdk>,
}

impl SparkWallet {
    /// Create a builder for advanced configuration
    pub fn builder(signer: SparkSigner, config: WalletConfig) -> SparkWalletBuilder {
        SparkWalletBuilder::new(signer, config)
    }

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
        SparkWalletBuilder::new(signer, config).build().await
    }

    /// Get the wallet's Spark address for receiving payments
    ///
    /// This calls the Breez SDK's receive_payment API with SparkAddress method
    /// to get a properly formatted Spark address string.
    pub async fn get_spark_address(&self) -> Result<String, SparkError> {
        let request = ReceivePaymentRequest {
            payment_method: ReceivePaymentMethod::SparkAddress,
        };

        let response = self
            .sdk
            .receive_payment(request)
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

        let response = self
            .sdk
            .receive_payment(request)
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
    #[expect(dead_code)]
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
        let info = self.get_info(true).await?;

        Ok(Balance {
            spark_sats: info.balance_sats,
            lightning_sats: 0, // Spark SDK handles Lightning internally
            onchain_sats: 0,   // On-chain shown separately via deposits
        })
    }

    /// Get raw wallet info from the Breez SDK
    pub async fn get_info(&self, ensure_synced: bool) -> Result<GetInfoResponse, SparkError> {
        let request = GetInfoRequest {
            ensure_synced: Some(ensure_synced),
        };

        self.sdk
            .get_info(request)
            .await
            .map_err(|e| SparkError::BalanceQueryFailed(e.to_string()))
    }

    /// Check network connectivity by forcing a sync with a timeout
    pub async fn network_status(&self, timeout: Duration) -> NetworkStatusReport {
        let request = SyncWalletRequest {};
        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        {
            match tokio::time::timeout(timeout, self.sdk.sync_wallet(request)).await {
                Ok(Ok(_)) => NetworkStatusReport::connected(),
                Ok(Err(err)) => NetworkStatusReport::disconnected(Some(err.to_string())),
                Err(_) => NetworkStatusReport::disconnected(Some(format!(
                    "Timed out after {} seconds",
                    timeout.as_secs()
                ))),
            }
        }

        #[cfg(all(target_family = "wasm", target_os = "unknown"))]
        {
            let _ = timeout;
            match self.sdk.sync_wallet(request).await {
                Ok(_) => NetworkStatusReport::connected(),
                Err(err) => NetworkStatusReport::disconnected(Some(err.to_string())),
            }
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

        self.prepare_send_payment_request(request).await
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

        self.sdk
            .send_payment(request)
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

        self.sdk
            .claim_htlc_payment(request)
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

        self.sdk
            .receive_payment(request)
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

        self.sdk
            .receive_payment(request)
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

        let response = self.list_payments_request(request).await?;

        Ok(response.payments)
    }

    /// List payments with full filter control
    pub async fn list_payments_request(
        &self,
        request: ListPaymentsRequest,
    ) -> Result<ListPaymentsResponse, SparkError> {
        self.sdk
            .list_payments(request)
            .await
            .map_err(|e| SparkError::Wallet(format!("Failed to list payments: {}", e)))
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

        let response = self
            .list_payments_request(request)
            .await
            .map_err(|e| SparkError::Wallet(format!("Failed to list HTLC payments: {}", e)))?;

        Ok(response.payments)
    }

    /// Sync wallet state with operators
    pub async fn sync_wallet(
        &self,
        request: SyncWalletRequest,
    ) -> Result<SyncWalletResponse, SparkError> {
        self.sdk
            .sync_wallet(request)
            .await
            .map_err(|e| SparkError::Network(e.to_string()))
    }

    /// Disconnect the Breez SDK and stop background tasks
    pub async fn disconnect(&self) -> Result<(), SparkError> {
        self.sdk
            .disconnect()
            .await
            .map_err(|e| SparkError::Network(e.to_string()))
    }

    /// Parse a payment input using the SDK configuration
    pub async fn parse_input(&self, input: &str) -> Result<InputType, SparkError> {
        self.sdk
            .parse(input)
            .await
            .map_err(|e| SparkError::InvalidAddress(e.to_string()))
    }

    /// Prepare a payment with full request support (tokens, spark address options)
    pub async fn prepare_send_payment_request(
        &self,
        request: PrepareSendPaymentRequest,
    ) -> Result<PrepareSendPaymentResponse, SparkError> {
        self.sdk
            .prepare_send_payment(request)
            .await
            .map_err(|e| SparkError::PaymentFailed(e.to_string()))
    }

    /// Receive a payment using a full request (Spark, Bitcoin, or BOLT-11)
    pub async fn receive_payment(
        &self,
        request: ReceivePaymentRequest,
    ) -> Result<ReceivePaymentResponse, SparkError> {
        self.sdk
            .receive_payment(request)
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Create a BOLT-11 invoice for receiving Lightning payments
    pub async fn create_bolt11_invoice(
        &self,
        amount_sats: Option<u64>,
        description: String,
    ) -> Result<ReceivePaymentResponse, SparkError> {
        let request = ReceivePaymentRequest {
            payment_method: ReceivePaymentMethod::Bolt11Invoice {
                description,
                amount_sats,
                expiry_secs: None,
            },
        };

        self.receive_payment(request).await
    }

    /// Fetch a single payment by ID
    pub async fn get_payment(
        &self,
        request: GetPaymentRequest,
    ) -> Result<GetPaymentResponse, SparkError> {
        self.sdk
            .get_payment(request)
            .await
            .map_err(|e| SparkError::Wallet(format!("Failed to get payment: {}", e)))
    }

    /// Prepare an LNURL-Pay request
    pub async fn prepare_lnurl_pay(
        &self,
        request: PrepareLnurlPayRequest,
    ) -> Result<PrepareLnurlPayResponse, SparkError> {
        self.sdk
            .prepare_lnurl_pay(request)
            .await
            .map_err(|e| SparkError::PaymentFailed(e.to_string()))
    }

    /// Send an LNURL-Pay payment
    pub async fn lnurl_pay(
        &self,
        request: LnurlPayRequest,
    ) -> Result<LnurlPayResponse, SparkError> {
        self.sdk
            .lnurl_pay(request)
            .await
            .map_err(|e| SparkError::PaymentFailed(e.to_string()))
    }

    /// Perform an LNURL-Withdraw operation
    pub async fn lnurl_withdraw(
        &self,
        request: LnurlWithdrawRequest,
    ) -> Result<LnurlWithdrawResponse, SparkError> {
        self.sdk
            .lnurl_withdraw(request)
            .await
            .map_err(|e| SparkError::PaymentFailed(e.to_string()))
    }

    /// List unclaimed on-chain deposits
    pub async fn list_unclaimed_deposits(
        &self,
        request: ListUnclaimedDepositsRequest,
    ) -> Result<ListUnclaimedDepositsResponse, SparkError> {
        self.sdk
            .list_unclaimed_deposits(request)
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Claim an on-chain deposit
    pub async fn claim_deposit(
        &self,
        request: ClaimDepositRequest,
    ) -> Result<ClaimDepositResponse, SparkError> {
        self.sdk
            .claim_deposit(request)
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Refund an on-chain deposit
    pub async fn refund_deposit(
        &self,
        request: RefundDepositRequest,
    ) -> Result<RefundDepositResponse, SparkError> {
        self.sdk
            .refund_deposit(request)
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Fetch recommended on-chain fees
    pub async fn recommended_fees(&self) -> Result<RecommendedFees, SparkError> {
        self.sdk
            .recommended_fees()
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Check Lightning address availability
    pub async fn check_lightning_address_available(
        &self,
        request: CheckLightningAddressRequest,
    ) -> Result<bool, SparkError> {
        self.sdk
            .check_lightning_address_available(request)
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Get the currently registered Lightning address
    pub async fn get_lightning_address(&self) -> Result<Option<LightningAddressInfo>, SparkError> {
        self.sdk
            .get_lightning_address()
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Register a Lightning address
    pub async fn register_lightning_address(
        &self,
        request: RegisterLightningAddressRequest,
    ) -> Result<LightningAddressInfo, SparkError> {
        self.sdk
            .register_lightning_address(request)
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Delete the registered Lightning address
    pub async fn delete_lightning_address(&self) -> Result<(), SparkError> {
        self.sdk
            .delete_lightning_address()
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// List supported fiat currencies
    pub async fn list_fiat_currencies(&self) -> Result<ListFiatCurrenciesResponse, SparkError> {
        self.sdk
            .list_fiat_currencies()
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// List fiat exchange rates
    pub async fn list_fiat_rates(&self) -> Result<ListFiatRatesResponse, SparkError> {
        self.sdk
            .list_fiat_rates()
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Fetch token metadata for specific tokens
    pub async fn get_tokens_metadata(
        &self,
        request: GetTokensMetadataRequest,
    ) -> Result<GetTokensMetadataResponse, SparkError> {
        self.sdk
            .get_tokens_metadata(request)
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Access the token issuer API
    pub fn get_token_issuer(&self) -> TokenIssuer {
        self.sdk.get_token_issuer()
    }

    /// Sign an arbitrary message with the wallet key
    pub async fn sign_message(
        &self,
        request: SignMessageRequest,
    ) -> Result<SignMessageResponse, SparkError> {
        self.sdk
            .sign_message(request)
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Verify a message signature
    pub async fn check_message(
        &self,
        request: CheckMessageRequest,
    ) -> Result<CheckMessageResponse, SparkError> {
        self.sdk
            .check_message(request)
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Get current user settings
    pub async fn get_user_settings(&self) -> Result<UserSettings, SparkError> {
        self.sdk
            .get_user_settings()
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Update user settings
    pub async fn update_user_settings(
        &self,
        request: UpdateUserSettingsRequest,
    ) -> Result<(), SparkError> {
        self.sdk
            .update_user_settings(request)
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Start leaf optimization in the background
    pub fn start_leaf_optimization(&self) {
        self.sdk.start_leaf_optimization();
    }

    /// Cancel leaf optimization
    pub async fn cancel_leaf_optimization(&self) -> Result<(), SparkError> {
        self.sdk
            .cancel_leaf_optimization()
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))
    }

    /// Get leaf optimization progress
    pub fn get_leaf_optimization_progress(&self) -> OptimizationProgress {
        self.sdk.get_leaf_optimization_progress()
    }
}

/// Parse a payment input without initializing a wallet
pub async fn parse_input(
    input: &str,
    parsers: Option<Vec<ExternalInputParser>>,
) -> Result<InputType, SparkError> {
    breez_sdk_spark::parse_input(input, parsers)
        .await
        .map_err(|e| SparkError::InvalidAddress(e.to_string()))
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
        let request =
            build_receive_request(4_200, description.clone(), Some(60)).expect("build request");
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

        let _wallet = SparkWallet::new(signer, config)
            .await
            .expect("should create wallet");
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
