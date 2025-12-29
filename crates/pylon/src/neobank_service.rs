//! Neobank service integration for Pylon
//!
//! Wraps neobank functionality for agent treasury management.

use compute::domain::UnifiedIdentity;
use neobank::{
    CashuWallet, Currency, EscrowService, ExchangeClient, MintConfig, MintTrustService,
    ReputationService, SettlementEngine, TradingPair, TreasuryAgent, TreasuryAgentConfig,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;
use url::Url;

/// Configuration for neobank service
#[derive(Debug, Clone)]
pub struct NeobankConfig {
    /// Data directory for wallet storage
    pub data_dir: PathBuf,
    /// BTC mint URL
    pub btc_mint_url: Url,
    /// USD mint URL (optional)
    pub usd_mint_url: Option<Url>,
    /// Whether treasury agent is enabled
    pub treasury_enabled: bool,
    /// Treasury spread in basis points
    pub treasury_spread_bps: u16,
    /// Settlement timeout in seconds
    pub settlement_timeout_secs: u64,
}

impl Default for NeobankConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from(".pylon/neobank"),
            btc_mint_url: MintConfig::default_btc_mint().url,
            usd_mint_url: None,
            treasury_enabled: false,
            treasury_spread_bps: 50, // 0.5% spread
            settlement_timeout_secs: 60,
        }
    }
}

/// Errors from neobank service
#[derive(Debug, Error)]
pub enum NeobankError {
    #[error("not initialized")]
    NotInitialized,

    #[error("wallet error: {0}")]
    Wallet(String),

    #[error("exchange error: {0}")]
    Exchange(String),

    #[error("settlement error: {0}")]
    Settlement(String),

    #[error("invalid currency: {0}")]
    InvalidCurrency(String),
}

/// Treasury status summary
#[derive(Debug, Clone)]
pub struct TreasuryStatus {
    /// BTC balance in satoshis
    pub btc_balance_sats: u64,
    /// USD balance in cents
    pub usd_balance_cents: u64,
    /// Whether treasury agent is active
    pub treasury_active: bool,
    /// Current BTC/USD rate if available
    pub btc_usd_rate: Option<f64>,
    /// Number of pending trades
    pub pending_trades: u32,
}

impl Default for TreasuryStatus {
    fn default() -> Self {
        Self {
            btc_balance_sats: 0,
            usd_balance_cents: 0,
            treasury_active: false,
            btc_usd_rate: None,
            pending_trades: 0,
        }
    }
}

/// Neobank service wrapper for Pylon
pub struct NeobankService {
    /// BTC wallet
    btc_wallet: Option<Arc<CashuWallet>>,
    /// USD wallet
    usd_wallet: Option<Arc<CashuWallet>>,
    /// Treasury agent for market making
    treasury: Option<Arc<RwLock<TreasuryAgent>>>,
    /// Settlement engine
    settlement: Option<SettlementEngine>,
    /// Exchange client
    exchange: Option<ExchangeClient>,
    /// Mint trust service
    mint_trust: MintTrustService,
    /// Reputation service
    reputation: ReputationService,
    /// Escrow service
    #[allow(dead_code)]
    escrow: EscrowService,
    /// Configuration
    config: NeobankConfig,
    /// Whether the service is initialized
    initialized: bool,
}

impl NeobankService {
    /// Create a new neobank service with configuration
    pub fn new(config: NeobankConfig) -> Self {
        Self {
            btc_wallet: None,
            usd_wallet: None,
            treasury: None,
            settlement: None,
            exchange: None,
            mint_trust: MintTrustService::new(),
            reputation: ReputationService::new(),
            escrow: EscrowService::new(),
            config,
            initialized: false,
        }
    }

    /// Initialize the neobank service with an identity
    pub async fn init(&mut self, identity: &UnifiedIdentity) -> Result<(), NeobankError> {
        // Create wallet directory
        let wallet_dir = self.config.data_dir.clone();
        std::fs::create_dir_all(&wallet_dir)
            .map_err(|e| NeobankError::Wallet(format!("Failed to create wallet dir: {}", e)))?;

        // Derive seed from identity
        let seed = derive_wallet_seed(identity);

        // Create BTC wallet
        let btc_db_path = wallet_dir.join("btc_wallet.redb");
        let btc_wallet = CashuWallet::new(
            self.config.btc_mint_url.clone(),
            Currency::Btc,
            &seed,
            &btc_db_path,
        )
        .await
        .map_err(|e| NeobankError::Wallet(e.to_string()))?;

        self.btc_wallet = Some(Arc::new(btc_wallet));

        // Create USD wallet if configured
        if let Some(ref usd_url) = self.config.usd_mint_url {
            let usd_db_path = wallet_dir.join("usd_wallet.redb");
            let usd_wallet =
                CashuWallet::new(usd_url.clone(), Currency::Usd, &seed, &usd_db_path)
                    .await
                    .map_err(|e| NeobankError::Wallet(e.to_string()))?;

            self.usd_wallet = Some(Arc::new(usd_wallet));
        }

        // Create settlement engine with reputation-based settlement (if both wallets available)
        if let (Some(btc_wallet), Some(usd_wallet)) =
            (self.btc_wallet.clone(), self.usd_wallet.clone())
        {
            let settlement = SettlementEngine::new_reputation_based(
                btc_wallet,
                usd_wallet,
                Duration::from_secs(self.config.settlement_timeout_secs),
            );
            self.settlement = Some(settlement);
        } else {
            // Create mock settlement if only BTC wallet is available
            let settlement = SettlementEngine::new_mock();
            self.settlement = Some(settlement);
        }

        // Create exchange client (mock mode for now)
        let pubkey = identity
            .npub()
            .map_err(|e| NeobankError::Wallet(format!("Failed to get npub: {}", e)))?;
        let exchange = ExchangeClient::new_mock(&pubkey);
        self.exchange = Some(exchange);

        // Create treasury agent if enabled
        if self.config.treasury_enabled {
            let treasury_config = TreasuryAgentConfig::new(&pubkey)
                .with_pair(TradingPair::BtcUsd)
                .with_spread_bps(self.config.treasury_spread_bps)
                .with_min_trade(1000)
                .with_max_trade(1_000_000);

            let treasury = TreasuryAgent::new(treasury_config);
            self.treasury = Some(Arc::new(RwLock::new(treasury)));
        }

        self.initialized = true;
        tracing::info!("Neobank service initialized");

        Ok(())
    }

    /// Get balance for a currency
    pub async fn get_balance(&self, currency: Currency) -> Result<u64, NeobankError> {
        match currency {
            Currency::Btc => {
                let wallet = self
                    .btc_wallet
                    .as_ref()
                    .ok_or(NeobankError::NotInitialized)?;
                let balance = wallet
                    .balance()
                    .await
                    .map_err(|e| NeobankError::Wallet(e.to_string()))?;
                Ok(balance.value)
            }
            Currency::Usd => {
                let wallet = self
                    .usd_wallet
                    .as_ref()
                    .ok_or(NeobankError::Wallet("USD wallet not configured".to_string()))?;
                let balance = wallet
                    .balance()
                    .await
                    .map_err(|e| NeobankError::Wallet(e.to_string()))?;
                Ok(balance.value)
            }
        }
    }

    /// Get treasury status
    pub async fn get_treasury_status(&self) -> Result<TreasuryStatus, NeobankError> {
        let btc_balance_sats = self.get_balance(Currency::Btc).await.unwrap_or(0);
        let usd_balance_cents = self.get_balance(Currency::Usd).await.unwrap_or(0);

        let (treasury_active, btc_usd_rate) = if let Some(ref treasury) = self.treasury {
            let t = treasury.read().await;
            let rate = t.get_rate(TradingPair::BtcUsd).await.map(|r| r.rate);
            (true, rate)
        } else {
            (false, None)
        };

        Ok(TreasuryStatus {
            btc_balance_sats,
            usd_balance_cents,
            treasury_active,
            btc_usd_rate,
            pending_trades: 0, // TODO: track from exchange
        })
    }

    /// Create a deposit invoice (mint quote)
    pub async fn create_deposit_invoice(
        &self,
        amount_sats: u64,
        currency: Currency,
    ) -> Result<String, NeobankError> {
        let wallet = match currency {
            Currency::Btc => self.btc_wallet.as_ref(),
            Currency::Usd => self.usd_wallet.as_ref(),
        };

        let wallet = wallet.ok_or(NeobankError::NotInitialized)?;

        let quote = wallet
            .create_mint_quote(amount_sats)
            .await
            .map_err(|e| NeobankError::Wallet(e.to_string()))?;

        Ok(quote.bolt11)
    }

    /// Pay a Lightning invoice (melt)
    pub async fn pay_invoice(&self, bolt11: &str) -> Result<String, NeobankError> {
        let wallet = self
            .btc_wallet
            .as_ref()
            .ok_or(NeobankError::NotInitialized)?;

        let quote = wallet
            .create_melt_quote(bolt11)
            .await
            .map_err(|e| NeobankError::Wallet(e.to_string()))?;

        let result = wallet
            .melt(&quote.id)
            .await
            .map_err(|e| NeobankError::Wallet(e.to_string()))?;

        Ok(result.preimage.unwrap_or_else(|| "paid".to_string()))
    }

    /// Send Cashu tokens to another agent
    pub async fn send_tokens(
        &self,
        amount_sats: u64,
        currency: Currency,
    ) -> Result<String, NeobankError> {
        let wallet = match currency {
            Currency::Btc => self.btc_wallet.as_ref(),
            Currency::Usd => self.usd_wallet.as_ref(),
        };

        let wallet = wallet.ok_or(NeobankError::NotInitialized)?;

        let token = wallet
            .send_token(amount_sats)
            .await
            .map_err(|e| NeobankError::Wallet(e.to_string()))?;

        Ok(token)
    }

    /// Receive Cashu tokens from another agent
    pub async fn receive_tokens(&self, token: &str) -> Result<u64, NeobankError> {
        // Try BTC wallet first, then USD
        if let Some(ref wallet) = self.btc_wallet {
            if let Ok(amount) = wallet.receive_token(token).await {
                return Ok(amount.value);
            }
        }

        if let Some(ref wallet) = self.usd_wallet {
            if let Ok(amount) = wallet.receive_token(token).await {
                return Ok(amount.value);
            }
        }

        Err(NeobankError::Wallet(
            "Failed to receive token in any wallet".to_string(),
        ))
    }

    /// Check if the service is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get the exchange client
    pub fn exchange(&self) -> Option<&ExchangeClient> {
        self.exchange.as_ref()
    }

    /// Get the BTC wallet
    pub fn btc_wallet(&self) -> Option<&Arc<CashuWallet>> {
        self.btc_wallet.as_ref()
    }

    /// Get the USD wallet
    pub fn usd_wallet(&self) -> Option<&Arc<CashuWallet>> {
        self.usd_wallet.as_ref()
    }

    /// Get the mint trust service
    pub fn mint_trust(&self) -> &MintTrustService {
        &self.mint_trust
    }

    /// Get the reputation service
    pub fn reputation(&self) -> &ReputationService {
        &self.reputation
    }
}

/// Derive a wallet seed from the unified identity
fn derive_wallet_seed(identity: &UnifiedIdentity) -> [u8; 32] {
    // Use the nostr private key bytes as the seed
    // In production, this should use proper key derivation (BIP-32)
    *identity.private_key_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neobank_config_default() {
        let config = NeobankConfig::default();
        assert!(!config.treasury_enabled);
        assert_eq!(config.treasury_spread_bps, 50);
    }

    #[test]
    fn test_treasury_status_default() {
        let status = TreasuryStatus::default();
        assert_eq!(status.btc_balance_sats, 0);
        assert!(!status.treasury_active);
    }

    #[test]
    fn test_neobank_service_new() {
        let config = NeobankConfig::default();
        let service = NeobankService::new(config);
        assert!(!service.is_initialized());
    }
}
