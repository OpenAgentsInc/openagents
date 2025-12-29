//! GUI-specific state management

use crate::market::MarketKey;
use crate::state::OrderbookState;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Connection status for a relay
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayStatus {
    Connecting,
    Connected,
    Disconnected,
    Error(String),
}

/// GUI-specific state
pub struct GuiState {
    /// Core orderbook state (shared with relay tasks)
    pub orderbook: Arc<RwLock<OrderbookState>>,

    /// Currently selected market
    pub selected_market: Option<MarketKey>,

    /// Available markets (derived from orderbook)
    pub markets: Vec<MarketKey>,

    /// Connection status per relay
    pub relay_status: HashMap<String, RelayStatus>,

    /// Last event timestamp for UI updates
    pub last_event_time: Instant,

    /// Event counter for display
    pub event_count: u64,

    /// Connected relay count
    pub connected_count: usize,

    /// Total relay count
    pub relay_count: usize,
}

impl GuiState {
    pub fn new(orderbook: Arc<RwLock<OrderbookState>>) -> Self {
        Self {
            orderbook,
            selected_market: None,
            markets: Vec::new(),
            relay_status: HashMap::new(),
            last_event_time: Instant::now(),
            event_count: 0,
            connected_count: 0,
            relay_count: 0,
        }
    }

    /// Update markets list from orderbook state
    pub fn update_markets(&mut self, markets: Vec<MarketKey>) {
        self.markets = markets;
        // Auto-select first market if none selected
        if self.selected_market.is_none() && !self.markets.is_empty() {
            self.selected_market = Some(self.markets[0].clone());
        }
    }

    /// Set selected market by index
    pub fn select_market(&mut self, index: usize) {
        if index < self.markets.len() {
            self.selected_market = Some(self.markets[index].clone());
        }
    }

    /// Get index of selected market
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_market.as_ref().and_then(|selected| {
            self.markets.iter().position(|m| m == selected)
        })
    }

    /// Update relay connection status
    pub fn set_relay_status(&mut self, url: &str, status: RelayStatus) {
        self.relay_status.insert(url.to_string(), status);
        self.update_connection_counts();
    }

    fn update_connection_counts(&mut self) {
        self.relay_count = self.relay_status.len();
        self.connected_count = self.relay_status
            .values()
            .filter(|s| matches!(s, RelayStatus::Connected))
            .count();
    }
}
