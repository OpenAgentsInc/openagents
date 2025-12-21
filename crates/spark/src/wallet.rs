//! Spark wallet wrapper for the Breez SDK
//!
//! This module provides a high-level wallet interface for Bitcoin/Lightning payments
//! through the Breez Spark SDK. It wraps the underlying SDK to provide a simpler API
//! for OpenAgents applications.
//!
//! # Status
//!
//! **STUB IMPLEMENTATION** - This module currently contains placeholder methods.
//! Full Breez SDK integration is planned for Phase 2 of directive d-001.
//!
//! To complete this integration, we need to:
//! 1. Add Breez spark-wallet dependency (from GitHub or crates.io when published)
//! 2. Implement actual wallet initialization and state management
//! 3. Wire up real balance queries and payment operations
//! 4. Add persistence for wallet data
//! 5. Configure Spark operator connections
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
use serde::{Deserialize, Serialize};

/// Bitcoin network to use for Spark wallet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Network {
    /// Bitcoin mainnet
    Mainnet,
    /// Bitcoin testnet
    Testnet,
    /// Bitcoin signet (staging)
    Signet,
    /// Bitcoin regtest (local development)
    Regtest,
}

impl Default for Network {
    fn default() -> Self {
        Network::Testnet
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
    // TODO: Add actual Breez SDK client when we integrate the dependency
    // client: Arc<BreezServices>,
}

impl SparkWallet {
    /// Create a new Spark wallet with the given signer and configuration
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
        // TODO: Initialize Breez SDK client
        // This will require:
        // 1. Setting up SDK configuration
        // 2. Connecting to Spark operators
        // 3. Loading or creating wallet state
        // 4. Starting background sync

        Ok(Self {
            signer,
            config,
        })
    }

    /// Get the current wallet balance across all layers
    ///
    /// Returns balances for:
    /// - Spark Layer 2
    /// - Lightning channels
    /// - On-chain (cooperative exit funds)
    ///
    /// # Errors
    ///
    /// Returns error until Breez SDK integration is complete (see directive d-001).
    pub async fn get_balance(&self) -> Result<Balance, SparkError> {
        Err(SparkError::NotImplemented(
            "Balance querying requires Breez SDK integration. See directive d-001 for integration roadmap.".to_string()
        ))
    }

    /// Get wallet information and status
    ///
    /// # Errors
    ///
    /// Returns error until Breez SDK integration is complete (see directive d-001).
    pub async fn get_info(&self) -> Result<WalletInfo, SparkError> {
        Err(SparkError::NotImplemented(
            "Wallet info requires Breez SDK integration. See directive d-001 for integration roadmap.".to_string()
        ))
    }

    /// Force sync wallet state with Spark operators
    ///
    /// # Errors
    ///
    /// Returns error until Breez SDK integration is complete (see directive d-001).
    pub async fn sync(&self) -> Result<(), SparkError> {
        Err(SparkError::NotImplemented(
            "Wallet sync requires Breez SDK integration. See directive d-001 for integration roadmap.".to_string()
        ))
    }

    /// Get the wallet's Spark address for receiving payments
    ///
    /// **STUB**: Currently returns the public key hex. Will generate proper
    /// Spark address once Breez SDK is integrated.
    pub fn get_spark_address(&self) -> String {
        // TODO: Generate proper Spark address from Breez SDK
        // Spark addresses have a specific format for the protocol
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
    async fn test_get_balance_not_implemented() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = SparkSigner::from_mnemonic(mnemonic, "").expect("should create signer");
        let wallet = SparkWallet::new(signer, WalletConfig::default()).await.expect("should create wallet");

        // Should return NotImplemented error until Breez SDK is integrated
        let result = wallet.get_balance().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not implemented"));
    }

    #[tokio::test]
    async fn test_get_info_not_implemented() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = SparkSigner::from_mnemonic(mnemonic, "").expect("should create signer");
        let wallet = SparkWallet::new(signer, WalletConfig::default()).await.expect("should create wallet");

        // Should return NotImplemented error until Breez SDK is integrated
        let result = wallet.get_info().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not implemented"));
    }

    #[tokio::test]
    async fn test_sync_not_implemented() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = SparkSigner::from_mnemonic(mnemonic, "").expect("should create signer");
        let wallet = SparkWallet::new(signer, WalletConfig::default()).await.expect("should create wallet");

        // Should return NotImplemented error until Breez SDK is integrated
        let result = wallet.sync().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not implemented"));
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
