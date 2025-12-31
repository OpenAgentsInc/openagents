//! Exchange Relay - Nostr relay integration for agent trading
//!
//! This module provides relay connectivity for publishing orders,
//! subscribing to order updates, and exchanging settlement messages.
//!
//! # Example
//!
//! ```ignore
//! use neobank::relay::{ExchangeRelay, OrderFilter};
//!
//! // Create relay connection
//! let relay = ExchangeRelay::new(&["wss://relay.example.com"]).await?;
//!
//! // Fetch sell orders
//! let orders = relay.fetch_orders(OrderFilter {
//!     side: Some(OrderSide::Sell),
//!     currency: Some("USD".to_string()),
//!     ..Default::default()
//! }).await?;
//!
//! // Process orders
//! for order in orders {
//!     println!("Order: {:?}", order);
//! }
//! ```

use crate::error::{Error, Result};
use crate::exchange::{Order, OrderSide, OrderStatus, TradeAttestation, TradeOutcome};
use nostr::{Event, EventTemplate, finalize_event};
use nostr_client::{PoolConfig, RelayPool};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tracing::warn;

/// Event kind for P2P orders (NIP-69)
pub const P2P_ORDER_KIND: u16 = 38383;

/// Event kind for labels/attestations (NIP-32)
pub const LABEL_KIND: u16 = 1985;

const ORDER_FETCH_LIMIT: u64 = 500;
const ORDER_FETCH_TIMEOUT_MS: u64 = 800;
static ORDER_SUB_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Filter for fetching orders
#[derive(Debug, Clone, Default)]
pub struct OrderFilter {
    /// Filter by order side
    pub side: Option<OrderSide>,
    /// Filter by currency
    pub currency: Option<String>,
    /// Filter by maker pubkey
    pub maker: Option<String>,
    /// Minimum amount in sats
    pub min_amount: Option<u64>,
    /// Maximum amount in sats
    pub max_amount: Option<u64>,
    /// Only active (pending) orders
    pub only_active: bool,
}

/// Settlement coordination message types
#[derive(Debug, Clone)]
pub enum SettlementMessage {
    /// Request to start settlement
    StartSettlement {
        trade_id: String,
        /// Sender's reputation score
        reputation: f64,
    },
    /// Send a token to counterparty
    SendToken { trade_id: String, token: String },
    /// Acknowledge receipt of token
    TokenReceived { trade_id: String, amount: u64 },
    /// Report settlement complete
    SettlementComplete {
        trade_id: String,
        receipt_hash: String,
    },
    /// Report an issue
    DisputeInitiated { trade_id: String, reason: String },
}

/// Exchange relay client for Nostr connectivity
pub struct ExchangeRelay {
    /// Relay pool for connections
    pool: Arc<RelayPool>,
    /// Connected relay URLs
    relays: Vec<String>,
    /// Local order cache
    order_cache: Arc<RwLock<HashMap<String, Order>>>,
    /// Active subscriptions
    _subscriptions: Arc<RwLock<HashMap<String, mpsc::Sender<Order>>>>,
}

impl ExchangeRelay {
    /// Create a new exchange relay client
    ///
    /// # Arguments
    /// * `relay_urls` - List of relay URLs to connect to
    pub async fn new(relay_urls: &[&str]) -> Result<Self> {
        let config = PoolConfig {
            max_relays: relay_urls.len(),
            connection_timeout: Duration::from_secs(10),
            ..Default::default()
        };

        let pool = RelayPool::new(config);

        // Add relays to pool
        for url in relay_urls {
            pool.add_relay(url)
                .await
                .map_err(|e| Error::Database(format!("Failed to add relay {}: {}", url, e)))?;
        }

        // Connect to all relays
        pool.connect_all()
            .await
            .map_err(|e| Error::Database(format!("Failed to connect to relays: {}", e)))?;

        Ok(Self {
            pool: Arc::new(pool),
            relays: relay_urls.iter().map(|s| s.to_string()).collect(),
            order_cache: Arc::new(RwLock::new(HashMap::new())),
            _subscriptions: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create a new exchange relay client without connecting
    /// Useful for testing
    pub fn new_mock() -> Self {
        let config = PoolConfig::default();
        let pool = RelayPool::new(config);

        Self {
            pool: Arc::new(pool),
            relays: vec![],
            order_cache: Arc::new(RwLock::new(HashMap::new())),
            _subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get connected relay URLs
    pub fn relays(&self) -> &[String] {
        &self.relays
    }

    // ============================================================
    // Order Operations
    // ============================================================

    /// Publish an order to relays
    ///
    /// Creates and publishes a NIP-69 order event.
    ///
    /// # Arguments
    /// * `order` - The order to publish
    /// * `secret_key` - 32-byte secret key for signing
    ///
    /// # Returns
    /// The event ID of the published order
    pub async fn publish_order(&self, order: &Order, secret_key: &[u8; 32]) -> Result<String> {
        // Build NIP-69 order tags
        let tags = self.build_order_tags(order);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Create event template
        let template = EventTemplate {
            kind: P2P_ORDER_KIND,
            tags,
            content: String::new(),
            created_at: now,
        };

        // Sign and finalize
        let event = finalize_event(&template, secret_key)
            .map_err(|e| Error::Database(format!("Failed to sign event: {:?}", e)))?;

        let event_id = event.id.clone();

        // Publish to all relays (skip if no relays connected - mock mode)
        if !self.relays.is_empty() {
            self.pool
                .publish(&event)
                .await
                .map_err(|e| Error::Database(format!("Failed to publish order: {}", e)))?;
        }

        // Cache locally
        self.order_cache
            .write()
            .await
            .insert(order.order_id.clone(), order.clone());

        Ok(event_id)
    }

    /// Fetch orders from local cache
    ///
    /// Queries relays when available, then returns cached orders matching the filter.
    pub async fn fetch_orders(&self, filter: OrderFilter) -> Result<Vec<Order>> {
        if !self.relays.is_empty() {
            if let Err(err) = self.refresh_orders_from_relays(&filter).await {
                warn!("Relay order fetch failed: {}", err);
            }
        }

        let cache = self.order_cache.read().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let orders: Vec<Order> = cache
            .values()
            .filter(|order| order.expires_at > now)
            .filter(|order| self.order_matches_filter(order, &filter))
            .cloned()
            .collect();

        Ok(orders)
    }

    /// Inject an order into cache (for testing/mock relay sync)
    pub async fn inject_order(&self, order: Order) {
        self.order_cache
            .write()
            .await
            .insert(order.order_id.clone(), order);
    }

    /// Get an order from cache
    pub async fn get_order(&self, order_id: &str) -> Option<Order> {
        self.order_cache.read().await.get(order_id).cloned()
    }

    // ============================================================
    // Attestation Operations
    // ============================================================

    /// Publish a trade attestation
    ///
    /// Creates and publishes a NIP-32 label event for trade outcome.
    pub async fn publish_attestation(
        &self,
        attestation: &TradeAttestation,
        secret_key: &[u8; 32],
    ) -> Result<String> {
        let tags = self.build_attestation_tags(attestation);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let template = EventTemplate {
            kind: LABEL_KIND,
            tags,
            content: String::new(),
            created_at: now,
        };

        let event = finalize_event(&template, secret_key)
            .map_err(|e| Error::Database(format!("Failed to sign attestation: {:?}", e)))?;

        let event_id = event.id.clone();

        // Publish to all relays (skip if no relays connected - mock mode)
        if !self.relays.is_empty() {
            self.pool
                .publish(&event)
                .await
                .map_err(|e| Error::Database(format!("Failed to publish attestation: {}", e)))?;
        }

        Ok(event_id)
    }

    // ============================================================
    // Tag Builders
    // ============================================================

    /// Build NIP-69 order tags from an Order
    fn build_order_tags(&self, order: &Order) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["d".to_string(), order.order_id.clone()],
            vec!["k".to_string(), order.side.as_str().to_string()],
            vec!["f".to_string(), order.currency.clone()],
            vec!["s".to_string(), order.status.as_str().to_string()],
            vec!["amt".to_string(), order.amount_sats.to_string()],
            vec!["fa".to_string(), order.fiat_amount.to_string()],
        ];

        // Payment methods
        if !order.payment_methods.is_empty() {
            let mut pm_tag = vec!["pm".to_string()];
            pm_tag.extend(order.payment_methods.clone());
            tags.push(pm_tag);
        }

        // Additional tags
        tags.push(vec!["premium".to_string(), order.premium_pct.to_string()]);
        tags.push(vec!["network".to_string(), "mainnet".to_string()]);
        tags.push(vec!["layer".to_string(), "lightning".to_string()]);
        tags.push(vec!["expires_at".to_string(), order.expires_at.to_string()]);
        tags.push(vec!["y".to_string(), "openagents".to_string()]);
        tags.push(vec!["z".to_string(), "order".to_string()]);

        tags
    }

    /// Build NIP-32 attestation tags
    fn build_attestation_tags(&self, attestation: &TradeAttestation) -> Vec<Vec<String>> {
        vec![
            vec!["L".to_string(), "exchange/trade".to_string()],
            vec![
                "l".to_string(),
                attestation.outcome.as_str().to_string(),
                "exchange/trade".to_string(),
            ],
            vec!["p".to_string(), attestation.counterparty.clone()],
            vec!["e".to_string(), attestation.trade_id.clone()],
            vec!["amount".to_string(), attestation.amount_sats.to_string()],
            vec![
                "settlement_ms".to_string(),
                attestation.settlement_ms.to_string(),
            ],
        ]
    }

    // ============================================================
    // Event Parsers
    // ============================================================

    /// Parse a NIP-69 order event into an Order
    pub fn parse_order_event(&self, event: &Event) -> Option<Order> {
        let tags = &event.tags;

        // Extract required fields from tags
        let order_id = get_tag_value(tags, "d")?;
        let side_str = get_tag_value(tags, "k")?;
        let currency = get_tag_value(tags, "f")?;
        let status_str = get_tag_value(tags, "s")?;
        let amount_sats: u64 = get_tag_value(tags, "amt")?.parse().ok()?;
        let fiat_amount: u64 = get_tag_value(tags, "fa")?.parse().ok()?;

        let side = match side_str.as_str() {
            "sell" => OrderSide::Sell,
            "buy" => OrderSide::Buy,
            _ => return None,
        };

        let status = match status_str.as_str() {
            "pending" => OrderStatus::Pending,
            "in-progress" => OrderStatus::InProgress,
            "success" => OrderStatus::Success,
            "canceled" => OrderStatus::Canceled,
            "expired" => OrderStatus::Expired,
            _ => return None,
        };

        // Optional fields
        let premium_pct: f64 = get_tag_value(tags, "premium")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        let expires_at: u64 = get_tag_value(tags, "expires_at")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let payment_methods = get_tag_values(tags, "pm");

        Some(Order {
            order_id,
            maker_pubkey: event.pubkey.clone(),
            side,
            amount_sats,
            fiat_amount,
            currency,
            premium_pct,
            payment_methods,
            status,
            created_at: event.created_at,
            expires_at,
        })
    }

    /// Parse a NIP-32 attestation event
    pub fn parse_attestation_event(&self, event: &Event) -> Option<TradeAttestation> {
        let tags = &event.tags;

        // Check it's a trade attestation
        let namespace = get_tag_value(tags, "L")?;
        if namespace != "exchange/trade" {
            return None;
        }

        let outcome_str = get_tag_value(tags, "l")?;
        let counterparty = get_tag_value(tags, "p")?;
        let trade_id = get_tag_value(tags, "e")?;
        let amount_sats: u64 = get_tag_value(tags, "amount")?.parse().ok()?;
        let settlement_ms: u64 = get_tag_value(tags, "settlement_ms")?.parse().ok()?;

        let outcome = match outcome_str.as_str() {
            "success" => TradeOutcome::Success,
            "default" => TradeOutcome::Default,
            "dispute" => TradeOutcome::Dispute,
            "slow" => TradeOutcome::Slow,
            _ => return None,
        };

        Some(TradeAttestation {
            event_id: event.id.clone(),
            trade_id,
            counterparty,
            outcome,
            settlement_ms,
            amount_sats,
        })
    }

    /// Check if an order matches a filter
    fn order_matches_filter(&self, order: &Order, filter: &OrderFilter) -> bool {
        if let Some(ref side) = filter.side {
            if &order.side != side {
                return false;
            }
        }
        if let Some(ref currency) = filter.currency {
            if &order.currency != currency {
                return false;
            }
        }
        if let Some(ref maker) = filter.maker {
            if &order.maker_pubkey != maker {
                return false;
            }
        }
        if let Some(min) = filter.min_amount {
            if order.amount_sats < min {
                return false;
            }
        }
        if let Some(max) = filter.max_amount {
            if order.amount_sats > max {
                return false;
            }
        }
        if filter.only_active && order.status != OrderStatus::Pending {
            return false;
        }
        true
    }

    async fn refresh_orders_from_relays(&self, filter: &OrderFilter) -> Result<()> {
        let mut filter_map = serde_json::Map::new();
        filter_map.insert("kinds".to_string(), json!([P2P_ORDER_KIND as u64]));
        filter_map.insert("limit".to_string(), json!(ORDER_FETCH_LIMIT));

        if let Some(ref maker) = filter.maker {
            filter_map.insert("authors".to_string(), json!([maker]));
        }
        if let Some(ref currency) = filter.currency {
            filter_map.insert("#f".to_string(), json!([currency]));
        }
        if let Some(side) = filter.side {
            filter_map.insert("#k".to_string(), json!([side.as_str()]));
        }
        if filter.only_active {
            filter_map.insert("#s".to_string(), json!(["pending"]));
        }

        let filters = vec![serde_json::Value::Object(filter_map)];
        let subscription_id = next_order_subscription_id();
        let mut rx = self.pool.subscribe(&subscription_id, &filters).await?;

        let fetch_timeout = Duration::from_millis(ORDER_FETCH_TIMEOUT_MS);
        let start = tokio::time::Instant::now();
        let mut incoming: HashMap<String, Order> = HashMap::new();

        loop {
            let elapsed = start.elapsed();
            if elapsed >= fetch_timeout {
                break;
            }

            let remaining = fetch_timeout - elapsed;
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Some(event)) => {
                    if let Some(order) = self.parse_order_event(&event) {
                        let replace = match incoming.get(&order.order_id) {
                            Some(existing) => order.created_at >= existing.created_at,
                            None => true,
                        };
                        if replace {
                            incoming.insert(order.order_id.clone(), order);
                        }
                    }
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }

        if let Err(err) = self.pool.unsubscribe(&subscription_id).await {
            warn!(
                "Failed to close order subscription {}: {}",
                subscription_id, err
            );
        }

        if incoming.is_empty() {
            return Ok(());
        }

        let mut cache = self.order_cache.write().await;
        for (order_id, order) in incoming {
            let replace = match cache.get(&order_id) {
                Some(existing) => order.created_at >= existing.created_at,
                None => true,
            };
            if replace {
                cache.insert(order_id, order);
            }
        }

        Ok(())
    }
}

// ============================================================
// Helper Functions
// ============================================================

/// Get a tag value by key
fn get_tag_value(tags: &[Vec<String>], key: &str) -> Option<String> {
    for tag in tags {
        if tag.len() >= 2 && tag[0] == key {
            return Some(tag[1].clone());
        }
    }
    None
}

/// Get all values for a tag key (excluding the key itself)
fn get_tag_values(tags: &[Vec<String>], key: &str) -> Vec<String> {
    for tag in tags {
        if !tag.is_empty() && tag[0] == key {
            return tag[1..].to_vec();
        }
    }
    Vec::new()
}

fn next_order_subscription_id() -> String {
    let counter = ORDER_SUB_COUNTER.fetch_add(1, Ordering::Relaxed);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("exchange-orders-{}-{}", now, counter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_filter_default() {
        let filter = OrderFilter::default();
        assert!(filter.side.is_none());
        assert!(filter.currency.is_none());
        assert!(!filter.only_active);
    }

    #[test]
    fn test_order_matches_filter() {
        let order = Order {
            order_id: "test-123".to_string(),
            maker_pubkey: "pubkey_hex".to_string(),
            side: OrderSide::Sell,
            amount_sats: 10_000,
            fiat_amount: 100,
            currency: "USD".to_string(),
            premium_pct: 0.0,
            payment_methods: vec!["cashu".to_string()],
            status: OrderStatus::Pending,
            created_at: 0,
            expires_at: u64::MAX,
        };

        // Create a mock relay for testing
        let relay = ExchangeRelay::new_mock();

        // Empty filter matches all
        assert!(relay.order_matches_filter(&order, &OrderFilter::default()));

        // Side filter
        assert!(relay.order_matches_filter(
            &order,
            &OrderFilter {
                side: Some(OrderSide::Sell),
                ..Default::default()
            }
        ));
        assert!(!relay.order_matches_filter(
            &order,
            &OrderFilter {
                side: Some(OrderSide::Buy),
                ..Default::default()
            }
        ));

        // Currency filter
        assert!(relay.order_matches_filter(
            &order,
            &OrderFilter {
                currency: Some("USD".to_string()),
                ..Default::default()
            }
        ));
        assert!(!relay.order_matches_filter(
            &order,
            &OrderFilter {
                currency: Some("EUR".to_string()),
                ..Default::default()
            }
        ));

        // Amount filter
        assert!(relay.order_matches_filter(
            &order,
            &OrderFilter {
                min_amount: Some(5_000),
                max_amount: Some(20_000),
                ..Default::default()
            }
        ));
        assert!(!relay.order_matches_filter(
            &order,
            &OrderFilter {
                min_amount: Some(15_000),
                ..Default::default()
            }
        ));
    }

    #[test]
    fn test_settlement_message_variants() {
        let msg = SettlementMessage::StartSettlement {
            trade_id: "trade-123".to_string(),
            reputation: 0.95,
        };
        match msg {
            SettlementMessage::StartSettlement {
                trade_id,
                reputation,
            } => {
                assert_eq!(trade_id, "trade-123");
                assert_eq!(reputation, 0.95);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_get_tag_value() {
        let tags = vec![
            vec!["d".to_string(), "order-123".to_string()],
            vec!["k".to_string(), "sell".to_string()],
            vec!["f".to_string(), "USD".to_string()],
        ];

        assert_eq!(get_tag_value(&tags, "d"), Some("order-123".to_string()));
        assert_eq!(get_tag_value(&tags, "k"), Some("sell".to_string()));
        assert_eq!(get_tag_value(&tags, "f"), Some("USD".to_string()));
        assert_eq!(get_tag_value(&tags, "x"), None);
    }

    #[test]
    fn test_get_tag_values() {
        let tags = vec![
            vec![
                "pm".to_string(),
                "cashu".to_string(),
                "lightning".to_string(),
            ],
            vec!["d".to_string(), "order-123".to_string()],
        ];

        let pm = get_tag_values(&tags, "pm");
        assert_eq!(pm, vec!["cashu".to_string(), "lightning".to_string()]);

        let d = get_tag_values(&tags, "d");
        assert_eq!(d, vec!["order-123".to_string()]);

        let x = get_tag_values(&tags, "x");
        assert!(x.is_empty());
    }

    #[test]
    fn test_build_order_tags() {
        let relay = ExchangeRelay::new_mock();

        let order = Order {
            order_id: "test-order-123".to_string(),
            maker_pubkey: "maker_pubkey".to_string(),
            side: OrderSide::Sell,
            amount_sats: 10_000,
            fiat_amount: 100,
            currency: "USD".to_string(),
            premium_pct: 0.5,
            payment_methods: vec!["cashu".to_string()],
            status: OrderStatus::Pending,
            created_at: 0,
            expires_at: 1234567890,
        };

        let tags = relay.build_order_tags(&order);

        // Check required tags exist
        assert!(tags.iter().any(|t| t[0] == "d" && t[1] == "test-order-123"));
        assert!(tags.iter().any(|t| t[0] == "k" && t[1] == "sell"));
        assert!(tags.iter().any(|t| t[0] == "f" && t[1] == "USD"));
        assert!(tags.iter().any(|t| t[0] == "s" && t[1] == "pending"));
        assert!(tags.iter().any(|t| t[0] == "amt" && t[1] == "10000"));
        assert!(tags.iter().any(|t| t[0] == "fa" && t[1] == "100"));
        assert!(
            tags.iter()
                .any(|t| t[0] == "pm" && t.contains(&"cashu".to_string()))
        );
        assert!(tags.iter().any(|t| t[0] == "y" && t[1] == "openagents"));
    }

    #[test]
    fn test_build_attestation_tags() {
        let relay = ExchangeRelay::new_mock();

        let attestation = TradeAttestation {
            event_id: "event-123".to_string(),
            trade_id: "trade-456".to_string(),
            counterparty: "counterparty_pubkey".to_string(),
            outcome: TradeOutcome::Success,
            settlement_ms: 150,
            amount_sats: 10_000,
        };

        let tags = relay.build_attestation_tags(&attestation);

        assert!(tags.iter().any(|t| t[0] == "L" && t[1] == "exchange/trade"));
        assert!(tags.iter().any(|t| t[0] == "l" && t[1] == "success"));
        assert!(
            tags.iter()
                .any(|t| t[0] == "p" && t[1] == "counterparty_pubkey")
        );
        assert!(tags.iter().any(|t| t[0] == "e" && t[1] == "trade-456"));
        assert!(tags.iter().any(|t| t[0] == "amount" && t[1] == "10000"));
        assert!(
            tags.iter()
                .any(|t| t[0] == "settlement_ms" && t[1] == "150")
        );
    }
}
