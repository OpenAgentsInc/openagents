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
use breez_sdk_spark::{BreezSdk, ConnectRequest, Network as SdkNetwork, Seed};
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

    // Tests for get_balance, get_info, and sync have been removed per d-012
    // since those methods are now commented out (stub code).
    // When Breez SDK integration (d-001) is complete, add proper tests here.

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
