//! Market key grouping for orderbook views
//!
//! Groups orders by (currency, network, layer) for aggregated display.

/// Market key for grouping orders
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct MarketKey {
    /// Fiat currency (ISO 4217)
    pub currency: String,
    /// Bitcoin network (mainnet, testnet, signet)
    pub network: String,
    /// Bitcoin layer (lightning, onchain, liquid)
    pub layer: String,
}

impl MarketKey {
    pub fn new(currency: String, network: String, layer: String) -> Self {
        Self {
            currency,
            network,
            layer,
        }
    }

    /// Create from optional fields with defaults
    pub fn from_optional(
        currency: Option<&str>,
        network: Option<&str>,
        layer: Option<&str>,
    ) -> Self {
        Self {
            currency: currency.unwrap_or("UNKNOWN").to_uppercase(),
            network: network.unwrap_or("mainnet").to_lowercase(),
            layer: layer.unwrap_or("lightning").to_lowercase(),
        }
    }
}

impl std::fmt::Display for MarketKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}/{}", self.currency, self.layer, self.network)
    }
}

impl Ord for MarketKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&self.currency, &self.layer, &self.network).cmp(&(
            &other.currency,
            &other.layer,
            &other.network,
        ))
    }
}

impl PartialOrd for MarketKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Order depth at a price level
#[derive(Debug, Clone)]
pub struct DepthLevel {
    /// Premium percentage (effective price)
    pub premium: f64,
    /// Total sats at this level
    pub total_sats: u64,
    /// Number of orders
    pub order_count: usize,
}

/// Aggregated market view
#[derive(Debug, Clone)]
pub struct MarketDepth {
    pub market: MarketKey,
    /// Buy orders (bids) sorted by premium descending (best first)
    pub bids: Vec<DepthLevel>,
    /// Sell orders (asks) sorted by premium ascending (best first)
    pub asks: Vec<DepthLevel>,
}

impl MarketDepth {
    pub fn new(market: MarketKey) -> Self {
        Self {
            market,
            bids: Vec::new(),
            asks: Vec::new(),
        }
    }

    /// Best bid premium (highest)
    pub fn best_bid(&self) -> Option<f64> {
        self.bids.first().map(|l| l.premium)
    }

    /// Best ask premium (lowest)
    pub fn best_ask(&self) -> Option<f64> {
        self.asks.first().map(|l| l.premium)
    }

    /// Spread in premium points
    pub fn spread(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_key_display() {
        let key = MarketKey::new(
            "USD".to_string(),
            "mainnet".to_string(),
            "lightning".to_string(),
        );
        assert_eq!(key.to_string(), "USD/lightning/mainnet");
    }

    #[test]
    fn test_market_key_from_optional() {
        let key = MarketKey::from_optional(Some("eur"), None, Some("onchain"));
        assert_eq!(key.currency, "EUR");
        assert_eq!(key.network, "mainnet");
        assert_eq!(key.layer, "onchain");
    }

    #[test]
    fn test_market_key_ordering() {
        let key1 = MarketKey::new(
            "EUR".to_string(),
            "mainnet".to_string(),
            "lightning".to_string(),
        );
        let key2 = MarketKey::new(
            "USD".to_string(),
            "mainnet".to_string(),
            "lightning".to_string(),
        );
        let key3 = MarketKey::new(
            "USD".to_string(),
            "mainnet".to_string(),
            "onchain".to_string(),
        );

        assert!(key1 < key2);
        assert!(key2 < key3);
    }
}
