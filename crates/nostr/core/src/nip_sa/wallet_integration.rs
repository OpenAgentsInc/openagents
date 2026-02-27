//! Wallet integration for agent state
//!
//! This module provides integration between NIP-SA agent state and the Spark wallet,
//! enabling agents to query their balance and include it in encrypted state updates.

use super::state::AgentStateContent;

#[cfg(feature = "spark-integration")]
use std::sync::Arc;

#[cfg(feature = "spark-integration")]
use tokio::sync::OnceCell;

#[cfg(feature = "spark-integration")]
use spark::{Network, SparkSigner, SparkWallet, WalletConfig};

/// Global wallet instance (initialized once at startup)
#[cfg(feature = "spark-integration")]
static WALLET_INSTANCE: OnceCell<Arc<SparkWallet>> = OnceCell::const_new();

/// Error type for wallet integration operations
#[cfg(feature = "spark-integration")]
#[derive(Debug, thiserror::Error)]
pub enum WalletIntegrationError {
    #[error("Wallet error: {0}")]
    WalletError(String),
}

/// Wallet balance breakdown
#[derive(Debug, Clone)]
pub struct WalletBalance {
    /// Spark layer 2 balance in satoshis
    pub spark_sats: u64,
    /// Lightning balance in satoshis
    pub lightning_sats: u64,
    /// On-chain balance in satoshis
    pub onchain_sats: u64,
}

impl WalletBalance {
    /// Get total balance across all layers
    pub fn total_sats(&self) -> u64 {
        self.spark_sats
            .saturating_add(self.lightning_sats)
            .saturating_add(self.onchain_sats)
    }
}

/// Initialize the global wallet instance
///
/// This must be called once at application startup before any wallet operations.
/// The wallet is initialized with the provided mnemonic and configuration.
///
/// # Arguments
/// * `mnemonic` - BIP39 mnemonic phrase
/// * `network` - Network to connect to (Mainnet or Regtest)
///
/// # Example
/// ```ignore
/// use nostr::nip_sa::wallet_integration::init_wallet;
/// use spark::Network;
///
/// init_wallet("abandon abandon ... about", Network::Regtest).await?;
/// ```
#[cfg(feature = "spark-integration")]
pub async fn init_wallet(mnemonic: &str, network: Network) -> Result<(), WalletIntegrationError> {
    let signer = SparkSigner::from_mnemonic(mnemonic, "").map_err(|e| {
        WalletIntegrationError::WalletError(format!("Failed to create signer: {}", e))
    })?;

    let config = WalletConfig {
        network,
        api_key: std::env::var("BREEZ_API_KEY").ok(),
        ..Default::default()
    };

    let wallet = SparkWallet::new(signer, config).await.map_err(|e| {
        WalletIntegrationError::WalletError(format!("Failed to connect wallet: {}", e))
    })?;

    WALLET_INSTANCE.set(Arc::new(wallet)).map_err(|_| {
        WalletIntegrationError::WalletError("Wallet already initialized".to_string())
    })?;

    Ok(())
}

/// Query wallet balance from Spark SDK and update agent state
///
/// This function queries the current balance from the Spark wallet and updates
/// the agent state's wallet_balance_sats field.
///
/// # Arguments
/// * `state` - Mutable reference to agent state content
///
/// # Returns
/// The updated wallet balance
///
/// # Example
/// ```ignore
/// use nostr::nip_sa::{AgentStateContent, update_wallet_balance};
///
/// let mut state = AgentStateContent::new();
/// let balance = update_wallet_balance(&mut state).await?;
/// println!("Total balance: {} sats", balance.total_sats());
/// ```
#[cfg(feature = "spark-integration")]
pub async fn update_wallet_balance(
    state: &mut AgentStateContent,
) -> Result<WalletBalance, WalletIntegrationError> {
    // Get wallet instance
    let wallet = get_wallet_instance()?;

    // Query balance from Spark SDK
    let balance = wallet
        .get_balance()
        .await
        .map_err(|e| WalletIntegrationError::WalletError(format!("balance query failed: {}", e)))?;

    // Update state with total balance
    state.update_balance(balance.total_sats());

    Ok(WalletBalance {
        spark_sats: balance.spark_sats,
        lightning_sats: balance.lightning_sats,
        onchain_sats: balance.onchain_sats,
    })
}

/// Query wallet balance without updating state
///
/// This function queries the current balance from the Spark wallet without
/// modifying the agent state. Useful for read-only balance checks.
///
/// # Returns
/// The current wallet balance
#[cfg(feature = "spark-integration")]
pub async fn query_wallet_balance() -> Result<WalletBalance, WalletIntegrationError> {
    let wallet = get_wallet_instance()?;

    let balance = wallet
        .get_balance()
        .await
        .map_err(|e| WalletIntegrationError::WalletError(format!("balance query failed: {}", e)))?;

    Ok(WalletBalance {
        spark_sats: balance.spark_sats,
        lightning_sats: balance.lightning_sats,
        onchain_sats: balance.onchain_sats,
    })
}

/// Get the initialized wallet instance
///
/// Returns the global wallet singleton. The wallet must be initialized
/// by calling `init_wallet()` before this function is used.
#[cfg(feature = "spark-integration")]
fn get_wallet_instance() -> Result<Arc<SparkWallet>, WalletIntegrationError> {
    WALLET_INSTANCE.get().cloned().ok_or_else(|| {
        WalletIntegrationError::WalletError(
            "Wallet not initialized - call init_wallet() first".to_string(),
        )
    })
}

/// Update wallet balance with manual balance value
///
/// This is a non-async fallback for testing or when Spark integration is not available.
/// Updates the agent state with a manually provided balance.
///
/// # Arguments
/// * `state` - Mutable reference to agent state content
/// * `balance_sats` - Total balance in satoshis
pub fn update_wallet_balance_manual(state: &mut AgentStateContent, balance_sats: u64) {
    state.update_balance(balance_sats);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nip_sa::state::AgentStateContent;

    #[test]
    fn test_wallet_balance_total() {
        let balance = WalletBalance {
            spark_sats: 100_000,
            lightning_sats: 50_000,
            onchain_sats: 25_000,
        };

        assert_eq!(balance.total_sats(), 175_000);
    }

    #[test]
    fn test_wallet_balance_overflow_protection() {
        let balance = WalletBalance {
            spark_sats: u64::MAX,
            lightning_sats: 1,
            onchain_sats: 0,
        };

        // Should saturate instead of overflowing
        assert_eq!(balance.total_sats(), u64::MAX);
    }

    #[test]
    fn test_update_wallet_balance_manual() {
        let mut state = AgentStateContent::new();
        assert_eq!(state.wallet_balance_sats, 0);

        update_wallet_balance_manual(&mut state, 100_000);
        assert_eq!(state.wallet_balance_sats, 100_000);

        update_wallet_balance_manual(&mut state, 50_000);
        assert_eq!(state.wallet_balance_sats, 50_000);
    }
}
