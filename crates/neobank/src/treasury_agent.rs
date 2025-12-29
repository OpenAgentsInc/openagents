//! Treasury Agent - Liquidity provision for agent trading
//!
//! This module provides a treasury agent that can act as a market maker,
//! provide liquidity, respond to RFQs, and perform currency conversions.
//!
//! # Example
//!
//! ```ignore
//! use neobank::treasury_agent::{TreasuryAgent, TreasuryAgentConfig, TradingPair};
//!
//! // Configure treasury agent
//! let config = TreasuryAgentConfig::default()
//!     .with_pair(TradingPair::BtcUsd)
//!     .with_spread_bps(50) // 0.5% spread
//!     .with_max_trade(1_000_000);
//!
//! // Create agent with wallets
//! let agent = TreasuryAgent::new(config, btc_wallet, usd_wallet);
//!
//! // Post liquidity orders
//! let orders = agent.post_liquidity(TradingPair::BtcUsd, 500_000).await?;
//!
//! // Handle RFQ requests
//! if let Ok(quote) = agent.handle_rfq(&rfq_request).await {
//!     rfq_market.submit_quote(quote).await?;
//! }
//! ```

use crate::error::{Error, Result};
use crate::exchange::{ExchangeClient, Order, OrderParams, OrderSide};
use crate::relay::ExchangeRelay;
use crate::rfq::{RfqQuote, RfqRequest};
use crate::types::Currency;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Event kind for treasury service announcements (NIP-89)
pub const TREASURY_ANNOUNCEMENT_KIND: u16 = 31990;

/// Trading pair
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TradingPair {
    /// BTC/USD pair
    BtcUsd,
    /// BTC/EUR pair
    BtcEur,
}

impl TradingPair {
    /// Get base currency
    pub fn base(&self) -> Currency {
        match self {
            TradingPair::BtcUsd => Currency::Btc,
            TradingPair::BtcEur => Currency::Btc,
        }
    }

    /// Get quote currency
    pub fn quote(&self) -> Currency {
        match self {
            TradingPair::BtcUsd => Currency::Usd,
            TradingPair::BtcEur => Currency::Usd, // Treat EUR as USD variant for now
        }
    }

    /// Get currency code string
    pub fn quote_code(&self) -> &str {
        match self {
            TradingPair::BtcUsd => "USD",
            TradingPair::BtcEur => "EUR",
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            TradingPair::BtcUsd => "BTC/USD",
            TradingPair::BtcEur => "BTC/EUR",
        }
    }
}

/// Treasury agent configuration
#[derive(Debug, Clone)]
pub struct TreasuryAgentConfig {
    /// Agent's public key
    pub pubkey: String,
    /// Supported trading pairs
    pub supported_pairs: Vec<TradingPair>,
    /// Spread in basis points (100 = 1%)
    pub spread_bps: u16,
    /// Minimum trade size in sats
    pub min_trade_sats: u64,
    /// Maximum trade size in sats
    pub max_trade_sats: u64,
    /// Whether to auto-convert between currencies
    pub auto_convert: bool,
    /// Order expiration duration
    pub order_expiry: Duration,
    /// Quote expiration duration (for RFQ responses)
    pub quote_expiry: Duration,
    /// Minimum reputation required from counterparties
    pub min_counterparty_reputation: f64,
    /// Maximum exposure per currency (in sats equivalent)
    pub max_exposure_sats: u64,
}

impl Default for TreasuryAgentConfig {
    fn default() -> Self {
        Self {
            pubkey: String::new(),
            supported_pairs: vec![TradingPair::BtcUsd],
            spread_bps: 50, // 0.5%
            min_trade_sats: 1_000,
            max_trade_sats: 10_000_000, // 0.1 BTC
            auto_convert: false,
            order_expiry: Duration::from_secs(3600), // 1 hour
            quote_expiry: Duration::from_secs(60),   // 1 minute
            min_counterparty_reputation: 0.0,
            max_exposure_sats: 100_000_000, // 1 BTC
        }
    }
}

impl TreasuryAgentConfig {
    /// Create a new config with a pubkey
    pub fn new(pubkey: impl Into<String>) -> Self {
        Self {
            pubkey: pubkey.into(),
            ..Default::default()
        }
    }

    /// Add a supported trading pair
    pub fn with_pair(mut self, pair: TradingPair) -> Self {
        if !self.supported_pairs.contains(&pair) {
            self.supported_pairs.push(pair);
        }
        self
    }

    /// Set spread in basis points
    pub fn with_spread_bps(mut self, bps: u16) -> Self {
        self.spread_bps = bps;
        self
    }

    /// Set minimum trade size
    pub fn with_min_trade(mut self, sats: u64) -> Self {
        self.min_trade_sats = sats;
        self
    }

    /// Set maximum trade size
    pub fn with_max_trade(mut self, sats: u64) -> Self {
        self.max_trade_sats = sats;
        self
    }

    /// Enable auto-conversion
    pub fn with_auto_convert(mut self, enabled: bool) -> Self {
        self.auto_convert = enabled;
        self
    }

    /// Set order expiry
    pub fn with_order_expiry(mut self, duration: Duration) -> Self {
        self.order_expiry = duration;
        self
    }

    /// Set minimum counterparty reputation
    pub fn with_min_reputation(mut self, rep: f64) -> Self {
        self.min_counterparty_reputation = rep;
        self
    }

    /// Calculate spread percentage
    pub fn spread_pct(&self) -> f64 {
        self.spread_bps as f64 / 100.0
    }
}

/// Current market rate source
#[derive(Debug, Clone)]
pub struct MarketRate {
    /// Rate (fiat per BTC)
    pub rate: f64,
    /// Timestamp
    pub timestamp: u64,
    /// Source (e.g., "internal", "oracle")
    pub source: String,
}

impl Default for MarketRate {
    fn default() -> Self {
        Self {
            rate: 50_000.0, // Default placeholder rate
            timestamp: 0,
            source: "default".to_string(),
        }
    }
}

/// Treasury agent position
#[derive(Debug, Clone, Default)]
pub struct Position {
    /// BTC balance in sats
    pub btc_sats: u64,
    /// USD balance in cents
    pub usd_cents: u64,
    /// Open buy orders (sats)
    pub open_buy_sats: u64,
    /// Open sell orders (sats)
    pub open_sell_sats: u64,
}

impl Position {
    /// Net BTC exposure (positive = long, negative = short)
    pub fn net_btc_sats(&self) -> i64 {
        self.btc_sats as i64 + self.open_buy_sats as i64 - self.open_sell_sats as i64
    }

    /// Check if position is balanced (within tolerance)
    pub fn is_balanced(&self, tolerance_pct: f64) -> bool {
        let total = self.btc_sats + self.open_buy_sats + self.open_sell_sats;
        if total == 0 {
            return true;
        }
        let imbalance = (self.open_buy_sats as i64 - self.open_sell_sats as i64).unsigned_abs();
        (imbalance as f64 / total as f64) <= tolerance_pct
    }
}

/// Treasury agent for market making and liquidity provision
pub struct TreasuryAgent {
    /// Configuration
    config: TreasuryAgentConfig,
    /// Exchange client for order management
    exchange: Option<Arc<ExchangeClient>>,
    /// Relay for publishing (optional)
    relay: Option<Arc<ExchangeRelay>>,
    /// Current market rates
    rates: Arc<RwLock<HashMap<TradingPair, MarketRate>>>,
    /// Current position
    position: Arc<RwLock<Position>>,
    /// Active orders (order_id -> order)
    active_orders: Arc<RwLock<HashMap<String, Order>>>,
}

impl TreasuryAgent {
    /// Create a new treasury agent
    pub fn new(config: TreasuryAgentConfig) -> Self {
        Self {
            config,
            exchange: None,
            relay: None,
            rates: Arc::new(RwLock::new(HashMap::new())),
            position: Arc::new(RwLock::new(Position::default())),
            active_orders: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with exchange client
    pub fn with_exchange(mut self, exchange: Arc<ExchangeClient>) -> Self {
        self.exchange = Some(exchange);
        self
    }

    /// Create with relay
    pub fn with_relay(mut self, relay: Arc<ExchangeRelay>) -> Self {
        self.relay = Some(relay);
        self
    }

    /// Get the configuration
    pub fn config(&self) -> &TreasuryAgentConfig {
        &self.config
    }

    // ============================================================
    // Rate Management
    // ============================================================

    /// Set current market rate
    pub async fn set_rate(&self, pair: TradingPair, rate: f64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.rates.write().await.insert(
            pair,
            MarketRate {
                rate,
                timestamp: now,
                source: "manual".to_string(),
            },
        );
    }

    /// Get current market rate
    pub async fn get_rate(&self, pair: TradingPair) -> Option<MarketRate> {
        self.rates.read().await.get(&pair).cloned()
    }

    /// Calculate bid/ask prices with spread
    pub async fn get_bid_ask(&self, pair: TradingPair) -> Option<(f64, f64)> {
        let rate = self.get_rate(pair).await?;
        let half_spread = self.config.spread_pct() / 2.0;

        let bid = rate.rate * (1.0 - half_spread / 100.0);
        let ask = rate.rate * (1.0 + half_spread / 100.0);

        Some((bid, ask))
    }

    // ============================================================
    // Liquidity Provision
    // ============================================================

    /// Post liquidity orders for a trading pair
    ///
    /// Creates both a buy and sell order at bid/ask prices.
    pub async fn post_liquidity(
        &self,
        pair: TradingPair,
        amount_sats: u64,
    ) -> Result<Vec<String>> {
        // Validate amount
        if amount_sats < self.config.min_trade_sats {
            return Err(Error::Database(format!(
                "Amount {} below minimum {}",
                amount_sats, self.config.min_trade_sats
            )));
        }
        if amount_sats > self.config.max_trade_sats {
            return Err(Error::Database(format!(
                "Amount {} exceeds maximum {}",
                amount_sats, self.config.max_trade_sats
            )));
        }

        // Get current rate
        let (bid, ask) = self
            .get_bid_ask(pair)
            .await
            .ok_or_else(|| Error::Database("No rate available".to_string()))?;

        let exchange = self
            .exchange
            .as_ref()
            .ok_or_else(|| Error::Database("No exchange configured".to_string()))?;

        let mut order_ids = Vec::new();

        // Calculate fiat amounts
        let btc_amount = amount_sats as f64 / 100_000_000.0;
        let bid_fiat = (btc_amount * bid * 100.0) as u64; // cents
        let ask_fiat = (btc_amount * ask * 100.0) as u64;

        // Post sell order (we sell BTC at ask price)
        let sell_order_id = exchange
            .post_order(OrderParams {
                side: OrderSide::Sell,
                amount_sats,
                fiat_amount: ask_fiat,
                currency: pair.quote_code().to_string(),
                premium_pct: self.config.spread_pct() / 2.0,
                payment_methods: vec!["cashu".to_string()],
                expires_in: self.config.order_expiry,
            })
            .await?;

        order_ids.push(sell_order_id.clone());

        // Track in position
        {
            let mut pos = self.position.write().await;
            pos.open_sell_sats += amount_sats;
        }

        // Post buy order (we buy BTC at bid price)
        let buy_order_id = exchange
            .post_order(OrderParams {
                side: OrderSide::Buy,
                amount_sats,
                fiat_amount: bid_fiat,
                currency: pair.quote_code().to_string(),
                premium_pct: -self.config.spread_pct() / 2.0, // Negative premium = discount
                payment_methods: vec!["cashu".to_string()],
                expires_in: self.config.order_expiry,
            })
            .await?;

        order_ids.push(buy_order_id.clone());

        // Track in position
        {
            let mut pos = self.position.write().await;
            pos.open_buy_sats += amount_sats;
        }

        Ok(order_ids)
    }

    /// Cancel all active orders
    pub async fn cancel_all_orders(&self) -> Result<u32> {
        let exchange = self
            .exchange
            .as_ref()
            .ok_or_else(|| Error::Database("No exchange configured".to_string()))?;

        let orders: Vec<String> = self
            .active_orders
            .read()
            .await
            .keys()
            .cloned()
            .collect();

        let mut cancelled = 0;
        for order_id in orders {
            if exchange.cancel_order(&order_id).await.is_ok() {
                cancelled += 1;
            }
        }

        // Clear position tracking
        {
            let mut pos = self.position.write().await;
            pos.open_buy_sats = 0;
            pos.open_sell_sats = 0;
        }

        self.active_orders.write().await.clear();

        Ok(cancelled)
    }

    // ============================================================
    // RFQ Handling
    // ============================================================

    /// Handle an RFQ request
    ///
    /// Returns a quote if the request matches our parameters.
    pub async fn handle_rfq(&self, request: &RfqRequest) -> Result<RfqQuote> {
        // Validate request
        if request.is_expired() {
            return Err(Error::Database("RFQ request expired".to_string()));
        }

        // Check if we support this pair
        let pair = match request.currency.to_uppercase().as_str() {
            "USD" => TradingPair::BtcUsd,
            "EUR" => TradingPair::BtcEur,
            _ => return Err(Error::Database(format!("Unsupported currency: {}", request.currency))),
        };

        if !self.config.supported_pairs.contains(&pair) {
            return Err(Error::Database(format!("Pair not supported: {}", pair.as_str())));
        }

        // Validate amount
        if request.amount_sats < self.config.min_trade_sats {
            return Err(Error::Database("Amount below minimum".to_string()));
        }
        if request.amount_sats > self.config.max_trade_sats {
            return Err(Error::Database("Amount exceeds maximum".to_string()));
        }

        // Get rate
        let rate = self
            .get_rate(pair)
            .await
            .ok_or_else(|| Error::Database("No rate available".to_string()))?;

        // Calculate premium based on side
        // When they BUY BTC, we SELL (use ask/positive premium)
        // When they SELL BTC, we BUY (use bid/negative premium)
        let premium_pct = match request.side {
            OrderSide::Buy => self.config.spread_pct() / 2.0,
            OrderSide::Sell => -self.config.spread_pct() / 2.0,
        };

        // Check if premium is within their acceptable range
        if !request.accepts_premium(premium_pct) {
            return Err(Error::Database(format!(
                "Premium {} outside acceptable range",
                premium_pct
            )));
        }

        // Create quote
        let quote = RfqQuote::new(request, rate.rate, premium_pct)
            .with_provider(self.config.pubkey.clone())
            .with_expiry_secs(self.config.quote_expiry.as_secs())
            .with_min_reputation(self.config.min_counterparty_reputation);

        Ok(quote)
    }

    // ============================================================
    // Position Management
    // ============================================================

    /// Get current position
    pub async fn position(&self) -> Position {
        self.position.read().await.clone()
    }

    /// Update position from wallet balances
    pub async fn sync_position(&self, btc_balance: u64, usd_balance: u64) {
        let mut pos = self.position.write().await;
        pos.btc_sats = btc_balance;
        pos.usd_cents = usd_balance;
    }

    /// Calculate dynamic spread based on volume
    ///
    /// Higher volume = tighter spread to attract more trades.
    pub fn calculate_spread(&self, _pair: TradingPair, volume_24h_sats: u64) -> f64 {
        let base_spread = self.config.spread_pct();

        // Volume tiers (in BTC equivalent)
        let volume_btc = volume_24h_sats as f64 / 100_000_000.0;

        let multiplier = if volume_btc >= 10.0 {
            0.5 // High volume: 50% of base spread
        } else if volume_btc >= 1.0 {
            0.75 // Medium volume: 75%
        } else {
            1.0 // Low volume: full spread
        };

        base_spread * multiplier
    }

    /// Check if rebalancing is needed
    pub async fn needs_rebalance(&self) -> bool {
        let pos = self.position.read().await;
        !pos.is_balanced(0.2) // 20% tolerance
    }

    // ============================================================
    // Service Announcements
    // ============================================================

    /// Build NIP-89 service announcement tags
    pub fn build_announcement_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["d".to_string(), "treasury-agent".to_string()],
            vec!["k".to_string(), "38383".to_string()], // P2P order kind we handle
            vec![
                "name".to_string(),
                "OpenAgents Treasury Agent".to_string(),
            ],
            vec![
                "description".to_string(),
                "Automated liquidity provider for BTC/USD trading".to_string(),
            ],
            vec!["version".to_string(), "1.0".to_string()],
        ];

        // Add supported pairs
        for pair in &self.config.supported_pairs {
            tags.push(vec!["pair".to_string(), pair.as_str().to_string()]);
        }

        // Add trading parameters
        tags.push(vec![
            "min_trade".to_string(),
            self.config.min_trade_sats.to_string(),
        ]);
        tags.push(vec![
            "max_trade".to_string(),
            self.config.max_trade_sats.to_string(),
        ]);
        tags.push(vec![
            "spread_bps".to_string(),
            self.config.spread_bps.to_string(),
        ]);

        tags
    }
}

impl Default for TreasuryAgent {
    fn default() -> Self {
        Self::new(TreasuryAgentConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trading_pair() {
        assert_eq!(TradingPair::BtcUsd.base(), Currency::Btc);
        assert_eq!(TradingPair::BtcUsd.quote(), Currency::Usd);
        assert_eq!(TradingPair::BtcUsd.quote_code(), "USD");
        assert_eq!(TradingPair::BtcUsd.as_str(), "BTC/USD");
    }

    #[test]
    fn test_config_defaults() {
        let config = TreasuryAgentConfig::default();

        assert_eq!(config.spread_bps, 50);
        assert_eq!(config.min_trade_sats, 1_000);
        assert_eq!(config.max_trade_sats, 10_000_000);
        assert!(!config.auto_convert);
        assert_eq!(config.spread_pct(), 0.5);
    }

    #[test]
    fn test_config_builder() {
        let config = TreasuryAgentConfig::new("test_pubkey")
            .with_pair(TradingPair::BtcUsd)
            .with_pair(TradingPair::BtcEur)
            .with_spread_bps(100)
            .with_min_trade(10_000)
            .with_max_trade(5_000_000)
            .with_auto_convert(true);

        assert_eq!(config.pubkey, "test_pubkey");
        assert_eq!(config.supported_pairs.len(), 2);
        assert_eq!(config.spread_bps, 100);
        assert_eq!(config.spread_pct(), 1.0);
        assert!(config.auto_convert);
    }

    #[test]
    fn test_position() {
        let mut pos = Position::default();
        assert_eq!(pos.net_btc_sats(), 0);
        assert!(pos.is_balanced(0.1));

        pos.btc_sats = 100_000;
        pos.open_buy_sats = 50_000;
        pos.open_sell_sats = 30_000;

        // Net = 100k + 50k - 30k = 120k
        assert_eq!(pos.net_btc_sats(), 120_000);

        // Imbalance = |50k - 30k| = 20k out of 180k total = 11%
        assert!(pos.is_balanced(0.15)); // 15% tolerance OK
        assert!(!pos.is_balanced(0.05)); // 5% tolerance not OK
    }

    #[tokio::test]
    async fn test_set_and_get_rate() {
        let agent = TreasuryAgent::new(TreasuryAgentConfig::default());

        // No rate initially
        assert!(agent.get_rate(TradingPair::BtcUsd).await.is_none());

        // Set rate
        agent.set_rate(TradingPair::BtcUsd, 50_000.0).await;

        let rate = agent.get_rate(TradingPair::BtcUsd).await.unwrap();
        assert_eq!(rate.rate, 50_000.0);
        assert_eq!(rate.source, "manual");
    }

    #[tokio::test]
    async fn test_bid_ask_calculation() {
        let config = TreasuryAgentConfig::default().with_spread_bps(100); // 1% spread
        let agent = TreasuryAgent::new(config);

        agent.set_rate(TradingPair::BtcUsd, 50_000.0).await;

        let (bid, ask) = agent.get_bid_ask(TradingPair::BtcUsd).await.unwrap();

        // 1% spread = 0.5% each side
        // Bid = 50000 * 0.995 = 49750
        // Ask = 50000 * 1.005 = 50250
        assert!((bid - 49_750.0).abs() < 0.1);
        assert!((ask - 50_250.0).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_handle_rfq() {
        let config = TreasuryAgentConfig::new("provider")
            .with_pair(TradingPair::BtcUsd)
            .with_spread_bps(100);

        let agent = TreasuryAgent::new(config);
        agent.set_rate(TradingPair::BtcUsd, 50_000.0).await;

        let request = RfqRequest::new(OrderSide::Buy, 100_000, "USD")
            .with_pubkey("requester")
            .with_max_premium(5.0);

        let quote = agent.handle_rfq(&request).await.unwrap();

        assert_eq!(quote.request_id, request.id);
        assert_eq!(quote.provider_pubkey, "provider");
        assert_eq!(quote.rate, 50_000.0);
        // Buying BTC = we sell = positive premium
        assert!(quote.premium_pct > 0.0);
        assert!(quote.premium_pct <= 1.0); // Half of 1% spread
    }

    #[tokio::test]
    async fn test_handle_rfq_unsupported_currency() {
        let agent = TreasuryAgent::new(TreasuryAgentConfig::default());

        let request = RfqRequest::new(OrderSide::Buy, 100_000, "GBP");

        let result = agent.handle_rfq(&request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_rfq_amount_too_small() {
        let config = TreasuryAgentConfig::default().with_min_trade(10_000);
        let agent = TreasuryAgent::new(config);
        agent.set_rate(TradingPair::BtcUsd, 50_000.0).await;

        let request = RfqRequest::new(OrderSide::Buy, 1_000, "USD"); // Below minimum

        let result = agent.handle_rfq(&request).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_spread() {
        let config = TreasuryAgentConfig::default().with_spread_bps(100); // 1% base
        let agent = TreasuryAgent::new(config);

        // Low volume: full spread
        let spread_low = agent.calculate_spread(TradingPair::BtcUsd, 10_000_000); // 0.1 BTC
        assert_eq!(spread_low, 1.0);

        // Medium volume: 75%
        let spread_med = agent.calculate_spread(TradingPair::BtcUsd, 200_000_000); // 2 BTC
        assert_eq!(spread_med, 0.75);

        // High volume: 50%
        let spread_high = agent.calculate_spread(TradingPair::BtcUsd, 1_500_000_000); // 15 BTC
        assert_eq!(spread_high, 0.5);
    }

    #[test]
    fn test_build_announcement_tags() {
        let config = TreasuryAgentConfig::new("test_pubkey")
            .with_pair(TradingPair::BtcUsd)
            .with_spread_bps(50)
            .with_min_trade(1000)
            .with_max_trade(1_000_000);

        let agent = TreasuryAgent::new(config);
        let tags = agent.build_announcement_tags();

        assert!(tags.iter().any(|t| t[0] == "d" && t[1] == "treasury-agent"));
        assert!(tags.iter().any(|t| t[0] == "k" && t[1] == "38383"));
        assert!(tags.iter().any(|t| t[0] == "pair" && t[1] == "BTC/USD"));
        assert!(tags.iter().any(|t| t[0] == "spread_bps" && t[1] == "50"));
    }

    #[tokio::test]
    async fn test_sync_position() {
        let agent = TreasuryAgent::new(TreasuryAgentConfig::default());

        agent.sync_position(1_000_000, 50_000_00).await;

        let pos = agent.position().await;
        assert_eq!(pos.btc_sats, 1_000_000);
        assert_eq!(pos.usd_cents, 50_000_00);
    }

    #[tokio::test]
    async fn test_needs_rebalance() {
        let agent = TreasuryAgent::new(TreasuryAgentConfig::default());

        // Balanced position
        assert!(!agent.needs_rebalance().await);

        // Unbalanced position
        {
            let mut pos = agent.position.write().await;
            pos.open_buy_sats = 100_000;
            pos.open_sell_sats = 10_000; // 90k imbalance
        }

        assert!(agent.needs_rebalance().await);
    }
}
