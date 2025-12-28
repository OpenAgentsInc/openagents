//! Exchange - Agent-to-agent trading via NIP-69
//!
//! This module provides an exchange client for agents to post orders,
//! find counterparties, and settle trades. Uses NIP-69 P2P order events
//! for interoperability with Mostro, Robosats, lnp2pBot, and Peach.
//!
//! # Example
//!
//! ```ignore
//! use neobank::exchange::{ExchangeClient, OrderParams, OrderSide, SettlementMode};
//!
//! // Create exchange client with mock settlement
//! let exchange = ExchangeClient::new_mock(keypair, relays);
//!
//! // Treasury Agent posts a sell order
//! let order_id = exchange.post_order(OrderParams {
//!     side: OrderSide::Sell,
//!     amount_sats: 10_000,
//!     fiat_amount: 100,  // $1.00
//!     currency: "USD".to_string(),
//!     ..Default::default()
//! }).await?;
//!
//! // Another agent fetches and accepts
//! let orders = exchange.fetch_orders(None).await?;
//! let trade = exchange.accept_order(&orders[0].order_id).await?;
//!
//! // Settlement executes
//! let receipt = exchange.settle(&trade).await?;
//!
//! // Both publish attestations
//! exchange.attest_trade(&trade, TradeOutcome::Success).await?;
//! ```

use crate::error::{Error, Result};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Global counter for unique order IDs
static ORDER_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Event kind for P2P orders (NIP-69)
pub const P2P_ORDER_KIND: u16 = 38383;

/// Event kind for labels (NIP-32)
pub const LABEL_KIND: u16 = 1985;

/// Order side (from BTC perspective)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    /// Selling BTC for fiat
    Sell,
    /// Buying BTC with fiat
    Buy,
}

impl OrderSide {
    pub fn as_str(&self) -> &str {
        match self {
            OrderSide::Sell => "sell",
            OrderSide::Buy => "buy",
        }
    }
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    Pending,
    InProgress,
    Success,
    Canceled,
    Expired,
}

impl OrderStatus {
    pub fn as_str(&self) -> &str {
        match self {
            OrderStatus::Pending => "pending",
            OrderStatus::InProgress => "in-progress",
            OrderStatus::Success => "success",
            OrderStatus::Canceled => "canceled",
            OrderStatus::Expired => "expired",
        }
    }
}

/// Parameters for creating a new order
#[derive(Debug, Clone)]
pub struct OrderParams {
    /// Order side (buy or sell BTC)
    pub side: OrderSide,
    /// Amount in satoshis
    pub amount_sats: u64,
    /// Fiat amount (in cents to avoid floats)
    pub fiat_amount: u64,
    /// Currency code (ISO 4217)
    pub currency: String,
    /// Premium percentage (can be negative for discount)
    pub premium_pct: f64,
    /// Payment methods accepted
    pub payment_methods: Vec<String>,
    /// Order expiration duration
    pub expires_in: Duration,
}

impl Default for OrderParams {
    fn default() -> Self {
        Self {
            side: OrderSide::Sell,
            amount_sats: 0,
            fiat_amount: 0,
            currency: "USD".to_string(),
            premium_pct: 0.0,
            payment_methods: vec!["cashu".to_string()],
            expires_in: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// An order in the exchange
#[derive(Debug, Clone)]
pub struct Order {
    /// Unique order ID
    pub order_id: String,
    /// Maker's public key (hex)
    pub maker_pubkey: String,
    /// Order side
    pub side: OrderSide,
    /// Amount in satoshis
    pub amount_sats: u64,
    /// Fiat amount (in cents)
    pub fiat_amount: u64,
    /// Currency code
    pub currency: String,
    /// Premium percentage
    pub premium_pct: f64,
    /// Payment methods
    pub payment_methods: Vec<String>,
    /// Current status
    pub status: OrderStatus,
    /// Created timestamp
    pub created_at: u64,
    /// Expires timestamp
    pub expires_at: u64,
}

/// A matched trade between maker and taker
#[derive(Debug, Clone)]
pub struct Trade {
    /// Trade ID (same as order ID)
    pub trade_id: String,
    /// Order that was matched
    pub order: Order,
    /// Taker's public key (hex)
    pub taker_pubkey: String,
    /// Trade status
    pub status: TradeStatus,
    /// When the trade was matched
    pub matched_at: Instant,
}

/// Trade status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeStatus {
    /// Matched, awaiting settlement
    Matched,
    /// Settlement in progress
    Settling,
    /// Successfully completed
    Completed,
    /// Disputed
    Disputed,
    /// Canceled
    Canceled,
}

/// Settlement receipt proving a trade completed
#[derive(Debug, Clone)]
pub struct SettlementReceipt {
    /// Trade ID
    pub trade_id: String,
    /// Settlement method used
    pub method: SettlementMethod,
    /// Amount settled (sats)
    pub amount_sats: u64,
    /// Settlement duration
    pub duration: Duration,
    /// Proof (method-specific)
    pub proof: Option<String>,
}

/// Settlement method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementMethod {
    /// Mock settlement for testing
    Mock,
    /// Reputation-based (v0)
    ReputationBased,
    /// Atomic eCash swap (v1)
    AtomicCashu,
}

/// Trade outcome for attestations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeOutcome {
    /// Trade completed successfully
    Success,
    /// Counterparty defaulted
    Default,
    /// Trade was disputed
    Dispute,
    /// Settlement was slow but completed
    Slow,
}

impl TradeOutcome {
    pub fn as_str(&self) -> &str {
        match self {
            TradeOutcome::Success => "success",
            TradeOutcome::Default => "default",
            TradeOutcome::Dispute => "dispute",
            TradeOutcome::Slow => "slow",
        }
    }
}

/// Trade attestation (NIP-32 label)
#[derive(Debug, Clone)]
pub struct TradeAttestation {
    /// Attestation event ID
    pub event_id: String,
    /// Trade that was attested
    pub trade_id: String,
    /// Counterparty pubkey
    pub counterparty: String,
    /// Outcome
    pub outcome: TradeOutcome,
    /// Settlement duration in ms
    pub settlement_ms: u64,
    /// Amount traded (sats)
    pub amount_sats: u64,
}

/// Settlement mode configuration
#[derive(Debug, Clone)]
pub enum SettlementMode {
    /// Mock settlement for testing (always succeeds)
    Mock,
    // TODO: Add real settlement modes
    // Real { btc_wallet: CashuWallet, usd_wallet: CashuWallet },
}

/// Exchange client for agent-to-agent trading
pub struct ExchangeClient {
    /// Our public key (hex)
    pubkey: String,
    /// Settlement mode
    settlement_mode: SettlementMode,
    /// Local order book (for mock mode)
    orders: Arc<RwLock<HashMap<String, Order>>>,
    /// Active trades
    trades: Arc<RwLock<HashMap<String, Trade>>>,
    /// Published attestations
    attestations: Arc<RwLock<Vec<TradeAttestation>>>,
}

impl ExchangeClient {
    /// Create a new exchange client with mock settlement
    pub fn new_mock(pubkey: impl Into<String>) -> Self {
        Self {
            pubkey: pubkey.into(),
            settlement_mode: SettlementMode::Mock,
            orders: Arc::new(RwLock::new(HashMap::new())),
            trades: Arc::new(RwLock::new(HashMap::new())),
            attestations: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get our public key
    pub fn pubkey(&self) -> &str {
        &self.pubkey
    }

    /// Post a new order to the exchange
    pub async fn post_order(&self, params: OrderParams) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let counter = ORDER_COUNTER.fetch_add(1, Ordering::Relaxed);
        let order_id = format!(
            "order-{}-{}-{}",
            &self.pubkey[..8.min(self.pubkey.len())],
            now,
            counter
        );

        let order = Order {
            order_id: order_id.clone(),
            maker_pubkey: self.pubkey.clone(),
            side: params.side,
            amount_sats: params.amount_sats,
            fiat_amount: params.fiat_amount,
            currency: params.currency,
            premium_pct: params.premium_pct,
            payment_methods: params.payment_methods,
            status: OrderStatus::Pending,
            created_at: now,
            expires_at: now + params.expires_in.as_secs(),
        };

        // Store locally (in real impl, would publish to relay)
        self.orders
            .write()
            .map_err(|e| Error::Database(e.to_string()))?
            .insert(order_id.clone(), order);

        Ok(order_id)
    }

    /// Fetch orders from the exchange
    pub async fn fetch_orders(&self, side_filter: Option<OrderSide>) -> Result<Vec<Order>> {
        let orders = self
            .orders
            .read()
            .map_err(|e| Error::Database(e.to_string()))?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(orders
            .values()
            .filter(|o| {
                // Filter by status and expiration
                o.status == OrderStatus::Pending && o.expires_at > now
            })
            .filter(|o| {
                // Filter by side if specified
                side_filter.map_or(true, |s| o.side == s)
            })
            .cloned()
            .collect())
    }

    /// Accept an order as taker
    pub async fn accept_order(&self, order_id: &str) -> Result<Trade> {
        let mut orders = self
            .orders
            .write()
            .map_err(|e| Error::Database(e.to_string()))?;

        let order = orders
            .get_mut(order_id)
            .ok_or_else(|| Error::Database(format!("Order not found: {}", order_id)))?;

        if order.status != OrderStatus::Pending {
            return Err(Error::Database(format!(
                "Order not pending: {}",
                order.status.as_str()
            )));
        }

        // Update order status
        order.status = OrderStatus::InProgress;

        let trade = Trade {
            trade_id: order_id.to_string(),
            order: order.clone(),
            taker_pubkey: self.pubkey.clone(),
            status: TradeStatus::Matched,
            matched_at: Instant::now(),
        };

        // Store trade
        drop(orders); // Release lock before acquiring another
        self.trades
            .write()
            .map_err(|e| Error::Database(e.to_string()))?
            .insert(order_id.to_string(), trade.clone());

        Ok(trade)
    }

    /// Cancel an order
    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let mut orders = self
            .orders
            .write()
            .map_err(|e| Error::Database(e.to_string()))?;

        let order = orders
            .get_mut(order_id)
            .ok_or_else(|| Error::Database(format!("Order not found: {}", order_id)))?;

        if order.maker_pubkey != self.pubkey {
            return Err(Error::Database("Not order maker".to_string()));
        }

        order.status = OrderStatus::Canceled;
        Ok(())
    }

    /// Settle a trade
    pub async fn settle(&self, trade: &Trade) -> Result<SettlementReceipt> {
        let start = Instant::now();

        // Update trade status
        {
            let mut trades = self
                .trades
                .write()
                .map_err(|e| Error::Database(e.to_string()))?;

            if let Some(t) = trades.get_mut(&trade.trade_id) {
                t.status = TradeStatus::Settling;
            }
        }

        // Execute settlement based on mode
        let (method, proof) = match &self.settlement_mode {
            SettlementMode::Mock => {
                // Mock settlement: just simulate delay
                tokio::time::sleep(Duration::from_millis(100)).await;
                (SettlementMethod::Mock, None)
            }
        };

        // Update trade to completed
        {
            let mut trades = self
                .trades
                .write()
                .map_err(|e| Error::Database(e.to_string()))?;

            if let Some(t) = trades.get_mut(&trade.trade_id) {
                t.status = TradeStatus::Completed;
            }
        }

        // Update order status
        {
            let mut orders = self
                .orders
                .write()
                .map_err(|e| Error::Database(e.to_string()))?;

            if let Some(o) = orders.get_mut(&trade.trade_id) {
                o.status = OrderStatus::Success;
            }
        }

        Ok(SettlementReceipt {
            trade_id: trade.trade_id.clone(),
            method,
            amount_sats: trade.order.amount_sats,
            duration: start.elapsed(),
            proof,
        })
    }

    /// Publish a trade attestation (NIP-32 label)
    pub async fn attest_trade(
        &self,
        trade: &Trade,
        outcome: TradeOutcome,
        settlement_ms: u64,
    ) -> Result<String> {
        let event_id = format!("attest-{}-{}", &trade.trade_id, outcome.as_str());

        // Determine counterparty
        let counterparty = if trade.order.maker_pubkey == self.pubkey {
            trade.taker_pubkey.clone()
        } else {
            trade.order.maker_pubkey.clone()
        };

        let attestation = TradeAttestation {
            event_id: event_id.clone(),
            trade_id: trade.trade_id.clone(),
            counterparty,
            outcome,
            settlement_ms,
            amount_sats: trade.order.amount_sats,
        };

        self.attestations
            .write()
            .map_err(|e| Error::Database(e.to_string()))?
            .push(attestation);

        Ok(event_id)
    }

    /// Get attestations for a pubkey
    pub fn get_attestations(&self, pubkey: &str) -> Result<Vec<TradeAttestation>> {
        let attestations = self
            .attestations
            .read()
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(attestations
            .iter()
            .filter(|a| a.counterparty == pubkey)
            .cloned()
            .collect())
    }

    /// Calculate reputation score from attestations
    pub fn calculate_reputation(&self, pubkey: &str) -> Result<f64> {
        let attestations = self.get_attestations(pubkey)?;

        if attestations.is_empty() {
            return Ok(0.0);
        }

        let total = attestations.len() as f64;
        let successes = attestations
            .iter()
            .filter(|a| a.outcome == TradeOutcome::Success)
            .count() as f64;

        Ok(successes / total)
    }

    // --- Testing helpers (simulates relay sync) ---

    /// Get a copy of an order (for syncing between clients in tests)
    pub fn get_order(&self, order_id: &str) -> Result<Option<Order>> {
        let orders = self
            .orders
            .read()
            .map_err(|e| Error::Database(e.to_string()))?;
        Ok(orders.get(order_id).cloned())
    }

    /// Inject an order (simulates receiving from relay)
    pub fn inject_order(&self, order: Order) -> Result<()> {
        self.orders
            .write()
            .map_err(|e| Error::Database(e.to_string()))?
            .insert(order.order_id.clone(), order);
        Ok(())
    }

    /// Get a copy of a trade (for syncing between clients in tests)
    pub fn get_trade(&self, trade_id: &str) -> Result<Option<Trade>> {
        let trades = self
            .trades
            .read()
            .map_err(|e| Error::Database(e.to_string()))?;
        Ok(trades.get(trade_id).cloned())
    }

    /// Inject a trade (simulates receiving from relay)
    pub fn inject_trade(&self, trade: Trade) -> Result<()> {
        self.trades
            .write()
            .map_err(|e| Error::Database(e.to_string()))?
            .insert(trade.trade_id.clone(), trade);
        Ok(())
    }

    // --- Utility methods for building NIP-69 tags ---

    /// Build NIP-69 order event tags
    pub fn build_order_tags(&self, params: &OrderParams) -> Vec<Vec<String>> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let order_id = format!("order-{}-{}", &self.pubkey[..8.min(self.pubkey.len())], now);

        let mut tags = vec![
            vec!["d".to_string(), order_id],
            vec!["k".to_string(), params.side.as_str().to_string()],
            vec!["f".to_string(), params.currency.clone()],
            vec!["s".to_string(), "pending".to_string()],
            vec!["amt".to_string(), params.amount_sats.to_string()],
            vec!["fa".to_string(), params.fiat_amount.to_string()],
        ];

        // Payment methods
        let mut pm_tag = vec!["pm".to_string()];
        pm_tag.extend(params.payment_methods.clone());
        tags.push(pm_tag);

        tags.push(vec!["premium".to_string(), params.premium_pct.to_string()]);
        tags.push(vec!["network".to_string(), "mainnet".to_string()]);
        tags.push(vec!["layer".to_string(), "lightning".to_string()]);
        tags.push(vec![
            "expires_at".to_string(),
            (now + params.expires_in.as_secs()).to_string(),
        ]);
        tags.push(vec![
            "expiration".to_string(),
            (now + params.expires_in.as_secs() + 86400).to_string(),
        ]);
        tags.push(vec!["y".to_string(), "openagents".to_string()]);
        tags.push(vec!["z".to_string(), "order".to_string()]);

        tags
    }

    /// Build NIP-32 trade attestation tags
    pub fn build_attestation_tags(
        &self,
        trade: &Trade,
        outcome: TradeOutcome,
        settlement_ms: u64,
    ) -> Vec<Vec<String>> {
        let counterparty = if trade.order.maker_pubkey == self.pubkey {
            &trade.taker_pubkey
        } else {
            &trade.order.maker_pubkey
        };

        vec![
            vec!["L".to_string(), "exchange/trade".to_string()],
            vec![
                "l".to_string(),
                outcome.as_str().to_string(),
                "exchange/trade".to_string(),
            ],
            vec!["p".to_string(), counterparty.clone()],
            vec!["e".to_string(), trade.trade_id.clone()],
            vec!["amount".to_string(), trade.order.amount_sats.to_string()],
            vec!["settlement_ms".to_string(), settlement_ms.to_string()],
            vec![
                "pair".to_string(),
                format!("BTC/{}", trade.order.currency),
            ],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_post_and_fetch_order() {
        let exchange = ExchangeClient::new_mock("alice_pubkey_hex");

        // Post order
        let order_id = exchange
            .post_order(OrderParams {
                side: OrderSide::Sell,
                amount_sats: 10_000,
                fiat_amount: 100, // $1.00
                currency: "USD".to_string(),
                ..Default::default()
            })
            .await
            .unwrap();

        assert!(order_id.starts_with("order-alice_pu"));

        // Fetch orders
        let orders = exchange.fetch_orders(None).await.unwrap();
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].amount_sats, 10_000);
        assert_eq!(orders[0].fiat_amount, 100);
    }

    #[tokio::test]
    async fn test_accept_order() {
        let maker = ExchangeClient::new_mock("maker_pubkey");
        let taker = ExchangeClient::new_mock("taker_pubkey");

        // Maker posts order
        let order_id = maker
            .post_order(OrderParams {
                side: OrderSide::Sell,
                amount_sats: 50_000,
                fiat_amount: 500,
                ..Default::default()
            })
            .await
            .unwrap();

        // Copy order to taker's view (in real impl, would fetch from relay)
        {
            let orders = maker.orders.read().unwrap();
            let order = orders.get(&order_id).unwrap().clone();
            taker.orders.write().unwrap().insert(order_id.clone(), order);
        }

        // Taker accepts
        let trade = taker.accept_order(&order_id).await.unwrap();

        assert_eq!(trade.trade_id, order_id);
        assert_eq!(trade.taker_pubkey, "taker_pubkey");
        assert_eq!(trade.status, TradeStatus::Matched);
    }

    #[tokio::test]
    async fn test_settle_trade() {
        let maker = ExchangeClient::new_mock("maker_pubkey");

        // Post and accept order
        let order_id = maker
            .post_order(OrderParams {
                side: OrderSide::Sell,
                amount_sats: 10_000,
                fiat_amount: 100,
                ..Default::default()
            })
            .await
            .unwrap();

        let trade = maker.accept_order(&order_id).await.unwrap();

        // Settle
        let receipt = maker.settle(&trade).await.unwrap();

        assert_eq!(receipt.trade_id, order_id);
        assert_eq!(receipt.method, SettlementMethod::Mock);
        assert_eq!(receipt.amount_sats, 10_000);
    }

    #[tokio::test]
    async fn test_trade_attestation() {
        let maker = ExchangeClient::new_mock("maker_pubkey");

        let order_id = maker
            .post_order(OrderParams {
                side: OrderSide::Sell,
                amount_sats: 10_000,
                fiat_amount: 100,
                ..Default::default()
            })
            .await
            .unwrap();

        // Simulate taker accepting
        {
            let mut orders = maker.orders.write().unwrap();
            let order = orders.get_mut(&order_id).unwrap();
            order.status = OrderStatus::InProgress;
        }

        let trade = Trade {
            trade_id: order_id.clone(),
            order: maker.orders.read().unwrap().get(&order_id).unwrap().clone(),
            taker_pubkey: "taker_pubkey".to_string(),
            status: TradeStatus::Completed,
            matched_at: Instant::now(),
        };

        // Publish attestation
        let attest_id = maker
            .attest_trade(&trade, TradeOutcome::Success, 150)
            .await
            .unwrap();

        assert!(attest_id.contains("success"));

        // Check reputation
        let rep = maker.calculate_reputation("taker_pubkey").unwrap();
        assert_eq!(rep, 1.0); // 100% success rate
    }

    #[tokio::test]
    async fn test_build_order_tags() {
        let exchange = ExchangeClient::new_mock("test_pubkey_1234567890");

        let tags = exchange.build_order_tags(&OrderParams {
            side: OrderSide::Sell,
            amount_sats: 10_000,
            fiat_amount: 100,
            currency: "USD".to_string(),
            premium_pct: 0.5,
            payment_methods: vec!["cashu".to_string(), "lightning".to_string()],
            expires_in: Duration::from_secs(3600),
        });

        // Verify required tags exist
        assert!(tags.iter().any(|t| t[0] == "d"));
        assert!(tags.iter().any(|t| t[0] == "k" && t[1] == "sell"));
        assert!(tags.iter().any(|t| t[0] == "f" && t[1] == "USD"));
        assert!(tags.iter().any(|t| t[0] == "s" && t[1] == "pending"));
        assert!(tags.iter().any(|t| t[0] == "amt" && t[1] == "10000"));
        assert!(tags.iter().any(|t| t[0] == "fa" && t[1] == "100"));
        assert!(tags.iter().any(|t| t[0] == "pm" && t.contains(&"cashu".to_string())));
        assert!(tags.iter().any(|t| t[0] == "y" && t[1] == "openagents"));
        assert!(tags.iter().any(|t| t[0] == "z" && t[1] == "order"));
    }

    #[tokio::test]
    async fn test_build_attestation_tags() {
        let exchange = ExchangeClient::new_mock("maker_pubkey");

        let trade = Trade {
            trade_id: "test-trade-123".to_string(),
            order: Order {
                order_id: "test-trade-123".to_string(),
                maker_pubkey: "maker_pubkey".to_string(),
                side: OrderSide::Sell,
                amount_sats: 10_000,
                fiat_amount: 100,
                currency: "USD".to_string(),
                premium_pct: 0.0,
                payment_methods: vec!["cashu".to_string()],
                status: OrderStatus::Success,
                created_at: 0,
                expires_at: 0,
            },
            taker_pubkey: "taker_pubkey".to_string(),
            status: TradeStatus::Completed,
            matched_at: Instant::now(),
        };

        let tags = exchange.build_attestation_tags(&trade, TradeOutcome::Success, 150);

        assert!(tags.iter().any(|t| t[0] == "L" && t[1] == "exchange/trade"));
        assert!(tags
            .iter()
            .any(|t| t[0] == "l" && t[1] == "success" && t[2] == "exchange/trade"));
        assert!(tags.iter().any(|t| t[0] == "p" && t[1] == "taker_pubkey"));
        assert!(tags.iter().any(|t| t[0] == "e" && t[1] == "test-trade-123"));
        assert!(tags.iter().any(|t| t[0] == "amount" && t[1] == "10000"));
        assert!(tags.iter().any(|t| t[0] == "settlement_ms" && t[1] == "150"));
        assert!(tags.iter().any(|t| t[0] == "pair" && t[1] == "BTC/USD"));
    }
}
