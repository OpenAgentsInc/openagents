//! In-memory orderbook state management
//!
//! Tracks NIP-69 orders using coordinate-based deduplication.
//! (kind, pubkey, d-tag) uniquely identifies an addressable event.

use crate::market::MarketKey;
use crate::parser::ParsedOrder;
use std::collections::{HashMap, VecDeque};

/// Order coordinate: (kind, pubkey, d-tag) - uniquely identifies an addressable event
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct OrderCoord {
    pub kind: u16,
    pub pubkey: String,
    pub d_tag: String,
}

impl OrderCoord {
    pub fn new(kind: u16, pubkey: String, d_tag: String) -> Self {
        Self {
            kind,
            pubkey,
            d_tag,
        }
    }

    /// Format as "kind:pubkey:d_tag" (NIP-33 style)
    pub fn to_addr(&self) -> String {
        format!(
            "{}:{}:{}",
            self.kind,
            &self.pubkey[..8.min(self.pubkey.len())],
            &self.d_tag
        )
    }
}

impl std::fmt::Display for OrderCoord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_addr())
    }
}

/// Statistics about the orderbook state
#[derive(Debug, Clone, Default)]
pub struct OrderbookStats {
    pub total_events_processed: u64,
    pub active_orders: usize,
    pub orders_by_status: HashMap<String, usize>,
    pub orders_by_market: HashMap<MarketKey, usize>,
}

/// In-memory orderbook state
pub struct OrderbookState {
    /// Latest state per order coordinate
    orders: HashMap<OrderCoord, ParsedOrder>,
    /// History per order coordinate (most recent first)
    history: HashMap<OrderCoord, Vec<ParsedOrder>>,
    /// Raw event feed (most recent first, limited size)
    raw_feed: VecDeque<ParsedOrder>,
    /// Maximum size of raw feed
    max_feed_size: usize,
    /// Sequence counter for events processed
    sequence: u64,
}

impl OrderbookState {
    /// Create a new orderbook state
    pub fn new() -> Self {
        Self::with_feed_size(1000)
    }

    /// Create with custom feed size limit
    pub fn with_feed_size(max_feed_size: usize) -> Self {
        Self {
            orders: HashMap::new(),
            history: HashMap::new(),
            raw_feed: VecDeque::new(),
            max_feed_size,
            sequence: 0,
        }
    }

    /// Process a new order, updating state and history
    ///
    /// Returns true if this is a new/updated order, false if it's older than existing
    pub fn process_order(&mut self, order: ParsedOrder) -> bool {
        self.sequence += 1;

        let coord = order.coord.clone();

        // Add to raw feed (most recent first)
        self.raw_feed.push_front(order.clone());
        if self.raw_feed.len() > self.max_feed_size {
            self.raw_feed.pop_back();
        }

        // Check if we should update the latest state
        let should_update = match self.orders.get(&coord) {
            Some(existing) => order.created_at >= existing.created_at,
            None => true,
        };

        if should_update {
            // Move old order to history if it exists
            if let Some(old) = self.orders.remove(&coord) {
                self.history.entry(coord.clone()).or_default().push(old);
            }

            // Insert new order as latest
            self.orders.insert(coord, order);
            true
        } else {
            // Older event - add to history only
            self.history.entry(coord).or_default().push(order);
            false
        }
    }

    /// Get the latest state for an order coordinate
    pub fn get_order(&self, coord: &OrderCoord) -> Option<&ParsedOrder> {
        self.orders.get(coord)
    }

    /// Get all active orders (status = pending)
    pub fn get_active_orders(&self) -> Vec<&ParsedOrder> {
        self.orders
            .values()
            .filter(|o| o.status.as_deref() == Some("pending"))
            .collect()
    }

    /// Get orders filtered by market key
    pub fn get_orders_by_market(&self, market: &MarketKey) -> Vec<&ParsedOrder> {
        self.orders
            .values()
            .filter(|o| &o.market_key() == market)
            .collect()
    }

    /// Get all unique market keys in the orderbook
    pub fn get_markets(&self) -> Vec<MarketKey> {
        let mut markets: Vec<_> = self
            .orders
            .values()
            .map(|o| o.market_key())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        markets.sort();
        markets
    }

    /// Get the raw feed (most recent first)
    pub fn get_raw_feed(&self, limit: usize) -> Vec<&ParsedOrder> {
        self.raw_feed.iter().take(limit).collect()
    }

    /// Get history for an order coordinate (most recent first)
    pub fn get_history(&self, coord: &OrderCoord) -> Vec<&ParsedOrder> {
        self.history
            .get(coord)
            .map(|h| h.iter().rev().collect())
            .unwrap_or_default()
    }

    /// Get statistics about the orderbook
    pub fn stats(&self) -> OrderbookStats {
        let mut orders_by_status = HashMap::new();
        let mut orders_by_market = HashMap::new();

        for order in self.orders.values() {
            let status = order
                .status
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            *orders_by_status.entry(status).or_insert(0) += 1;

            let market = order.market_key();
            *orders_by_market.entry(market).or_insert(0) += 1;
        }

        let active_orders = self
            .orders
            .values()
            .filter(|o| o.status.as_deref() == Some("pending"))
            .count();

        OrderbookStats {
            total_events_processed: self.sequence,
            active_orders,
            orders_by_status,
            orders_by_market,
        }
    }

    /// Get the sequence number (total events processed)
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Get total number of unique order coordinates
    pub fn order_count(&self) -> usize {
        self.orders.len()
    }
}

impl Default for OrderbookState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_order(pubkey: &str, d_tag: &str, created_at: u64, status: &str) -> ParsedOrder {
        ParsedOrder {
            event_id: format!("event-{}", created_at),
            coord: OrderCoord::new(38383, pubkey.to_string(), d_tag.to_string()),
            created_at,
            relay_url: "wss://test.relay".to_string(),
            side: Some("sell".to_string()),
            currency: Some("USD".to_string()),
            status: Some(status.to_string()),
            amount_sats: Some(10000),
            fiat_amount: vec![100],
            premium: Some(0.0),
            payment_methods: vec!["cashu".to_string()],
            network: Some("mainnet".to_string()),
            layer: Some("lightning".to_string()),
            expires_at: None,
            expiration: None,
            platform: Some("test".to_string()),
            source: None,
            name: None,
            geohash: None,
            bond: None,
            validation_errors: vec![],
            is_valid: true,
        }
    }

    #[test]
    fn test_process_new_order() {
        let mut state = OrderbookState::new();
        let order = make_order("pubkey1", "order-1", 1000, "pending");

        assert!(state.process_order(order.clone()));
        assert_eq!(state.order_count(), 1);
        assert_eq!(state.sequence(), 1);

        let retrieved = state.get_order(&order.coord).unwrap();
        assert_eq!(retrieved.event_id, "event-1000");
    }

    #[test]
    fn test_update_existing_order() {
        let mut state = OrderbookState::new();

        let order1 = make_order("pubkey1", "order-1", 1000, "pending");
        let order2 = make_order("pubkey1", "order-1", 2000, "in-progress");

        state.process_order(order1.clone());
        state.process_order(order2.clone());

        assert_eq!(state.order_count(), 1);
        assert_eq!(state.sequence(), 2);

        // Latest should be order2
        let retrieved = state.get_order(&order1.coord).unwrap();
        assert_eq!(retrieved.created_at, 2000);
        assert_eq!(retrieved.status, Some("in-progress".to_string()));

        // History should have order1
        let history = state.get_history(&order1.coord);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].created_at, 1000);
    }

    #[test]
    fn test_ignore_older_event() {
        let mut state = OrderbookState::new();

        let order1 = make_order("pubkey1", "order-1", 2000, "pending");
        let order2 = make_order("pubkey1", "order-1", 1000, "pending"); // older

        state.process_order(order1.clone());
        assert!(!state.process_order(order2)); // Should return false

        // Latest should still be order1
        let retrieved = state.get_order(&order1.coord).unwrap();
        assert_eq!(retrieved.created_at, 2000);
    }

    #[test]
    fn test_raw_feed_limit() {
        let mut state = OrderbookState::with_feed_size(3);

        for i in 0..5 {
            let order = make_order("pubkey1", &format!("order-{}", i), i, "pending");
            state.process_order(order);
        }

        let feed = state.get_raw_feed(10);
        assert_eq!(feed.len(), 3);
        // Most recent should be first
        assert_eq!(feed[0].coord.d_tag, "order-4");
    }

    #[test]
    fn test_get_active_orders() {
        let mut state = OrderbookState::new();

        state.process_order(make_order("pk1", "o1", 1000, "pending"));
        state.process_order(make_order("pk2", "o2", 1001, "canceled"));
        state.process_order(make_order("pk3", "o3", 1002, "pending"));

        let active = state.get_active_orders();
        assert_eq!(active.len(), 2);
    }
}
