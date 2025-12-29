//! RFQ - Request for Quote Market
//!
//! This module provides a request-for-quote system where agents can broadcast
//! their trading needs and receive competitive quotes from liquidity providers.
//!
//! # Example
//!
//! ```ignore
//! use neobank::rfq::{RfqMarket, RfqRequest};
//! use neobank::exchange::OrderSide;
//!
//! // Create RFQ market
//! let market = RfqMarket::new();
//!
//! // Request quotes for buying BTC
//! let request = RfqRequest::new(OrderSide::Buy, 100_000, "USD")
//!     .with_max_premium(2.0)
//!     .with_expiry_secs(300);
//!
//! // Broadcast and collect quotes
//! let quotes = market.broadcast_and_collect(&request, Duration::from_secs(30)).await?;
//!
//! // Accept best quote
//! if let Some(best) = market.best_quote(&quotes) {
//!     let trade = market.accept_quote(&best).await?;
//! }
//! ```

use crate::error::{Error, Result};
use crate::exchange::OrderSide;
use crate::relay::ExchangeRelay;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Event kind for RFQ requests (NIP-90 job request)
pub const RFQ_REQUEST_KIND: u16 = 5969;

/// Event kind for RFQ responses (NIP-90 job result)
pub const RFQ_RESPONSE_KIND: u16 = 6969;

/// Global counter for unique RFQ IDs
static RFQ_COUNTER: AtomicU64 = AtomicU64::new(0);

/// RFQ request - a broadcast request for trading quotes
#[derive(Debug, Clone)]
pub struct RfqRequest {
    /// Unique request ID
    pub id: String,
    /// Trade side (from requestor perspective)
    pub side: OrderSide,
    /// Amount in satoshis
    pub amount_sats: u64,
    /// Fiat currency code
    pub currency: String,
    /// Maximum acceptable premium percentage
    pub max_premium_pct: f64,
    /// Minimum acceptable premium (can be negative for discount)
    pub min_premium_pct: f64,
    /// Request expiration timestamp
    pub expires_at: u64,
    /// Preferred payment methods
    pub payment_methods: Vec<String>,
    /// Requestor's public key
    pub requestor_pubkey: String,
    /// Creation timestamp
    pub created_at: u64,
}

impl RfqRequest {
    /// Create a new RFQ request
    pub fn new(side: OrderSide, amount_sats: u64, currency: impl Into<String>) -> Self {
        let now = now_secs();
        let counter = RFQ_COUNTER.fetch_add(1, Ordering::Relaxed);

        Self {
            id: format!("rfq-{}-{}", now, counter),
            side,
            amount_sats,
            currency: currency.into(),
            max_premium_pct: 5.0, // Default 5% max premium
            min_premium_pct: -5.0, // Default -5% min (5% discount)
            expires_at: now + 300, // 5 minutes default
            payment_methods: vec!["cashu".to_string()],
            requestor_pubkey: String::new(),
            created_at: now,
        }
    }

    /// Set requestor pubkey
    pub fn with_pubkey(mut self, pubkey: impl Into<String>) -> Self {
        self.requestor_pubkey = pubkey.into();
        self
    }

    /// Set maximum premium
    pub fn with_max_premium(mut self, pct: f64) -> Self {
        self.max_premium_pct = pct;
        self
    }

    /// Set minimum premium (negative for discount)
    pub fn with_min_premium(mut self, pct: f64) -> Self {
        self.min_premium_pct = pct;
        self
    }

    /// Set expiration in seconds from now
    pub fn with_expiry_secs(mut self, secs: u64) -> Self {
        self.expires_at = now_secs() + secs;
        self
    }

    /// Set payment methods
    pub fn with_payment_methods(mut self, methods: Vec<String>) -> Self {
        self.payment_methods = methods;
        self
    }

    /// Check if request has expired
    pub fn is_expired(&self) -> bool {
        now_secs() > self.expires_at
    }

    /// Check if a quote premium is within acceptable range
    pub fn accepts_premium(&self, premium_pct: f64) -> bool {
        premium_pct >= self.min_premium_pct && premium_pct <= self.max_premium_pct
    }
}

/// Quote response to an RFQ
#[derive(Debug, Clone)]
pub struct RfqQuote {
    /// Quote ID
    pub id: String,
    /// Reference to original request
    pub request_id: String,
    /// Provider's public key
    pub provider_pubkey: String,
    /// Exchange rate (fiat per BTC)
    pub rate: f64,
    /// Premium percentage (can be negative for discount)
    pub premium_pct: f64,
    /// Amount in satoshis (may differ from request)
    pub amount_sats: u64,
    /// Fiat amount in cents
    pub fiat_amount: u64,
    /// Quote expiration timestamp
    pub expires_at: u64,
    /// Minimum reputation required from requestor
    pub min_reputation: f64,
    /// Creation timestamp
    pub created_at: u64,
}

impl RfqQuote {
    /// Create a new quote for a request
    pub fn new(request: &RfqRequest, rate: f64, premium_pct: f64) -> Self {
        let now = now_secs();
        let counter = RFQ_COUNTER.fetch_add(1, Ordering::Relaxed);

        // Calculate fiat amount based on rate and premium
        let adjusted_rate = rate * (1.0 + premium_pct / 100.0);
        let fiat_amount = ((request.amount_sats as f64 / 100_000_000.0) * adjusted_rate * 100.0) as u64;

        Self {
            id: format!("quote-{}-{}", now, counter),
            request_id: request.id.clone(),
            provider_pubkey: String::new(),
            rate,
            premium_pct,
            amount_sats: request.amount_sats,
            fiat_amount,
            expires_at: now + 60, // 1 minute default
            min_reputation: 0.0,
            created_at: now,
        }
    }

    /// Set provider pubkey
    pub fn with_provider(mut self, pubkey: impl Into<String>) -> Self {
        self.provider_pubkey = pubkey.into();
        self
    }

    /// Set expiration in seconds
    pub fn with_expiry_secs(mut self, secs: u64) -> Self {
        self.expires_at = now_secs() + secs;
        self
    }

    /// Set minimum reputation requirement
    pub fn with_min_reputation(mut self, rep: f64) -> Self {
        self.min_reputation = rep;
        self
    }

    /// Check if quote has expired
    pub fn is_expired(&self) -> bool {
        now_secs() > self.expires_at
    }

    /// Calculate effective price per sat
    pub fn price_per_sat(&self) -> f64 {
        if self.amount_sats == 0 {
            return 0.0;
        }
        self.fiat_amount as f64 / self.amount_sats as f64
    }
}

/// Filter for RFQ subscriptions
#[derive(Debug, Clone, Default)]
pub struct RfqFilter {
    /// Filter by side
    pub side: Option<OrderSide>,
    /// Filter by currency
    pub currency: Option<String>,
    /// Minimum amount in sats
    pub min_amount: Option<u64>,
    /// Maximum amount in sats
    pub max_amount: Option<u64>,
    /// Only include active (non-expired) requests
    pub only_active: bool,
}

/// RFQ market for broadcasting requests and collecting quotes
pub struct RfqMarket {
    /// Relay for publishing (optional)
    relay: Option<Arc<ExchangeRelay>>,
    /// Secret key for signing (optional)
    secret_key: Option<[u8; 32]>,
    /// Local request cache
    requests: Arc<RwLock<HashMap<String, RfqRequest>>>,
    /// Quotes per request
    quotes: Arc<RwLock<HashMap<String, Vec<RfqQuote>>>>,
}

impl RfqMarket {
    /// Create a new RFQ market
    pub fn new() -> Self {
        Self {
            relay: None,
            secret_key: None,
            requests: Arc::new(RwLock::new(HashMap::new())),
            quotes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with relay integration
    pub fn new_with_relay(relay: Arc<ExchangeRelay>, secret_key: [u8; 32]) -> Self {
        Self {
            relay: Some(relay),
            secret_key: Some(secret_key),
            requests: Arc::new(RwLock::new(HashMap::new())),
            quotes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // ============================================================
    // Requestor Side
    // ============================================================

    /// Broadcast an RFQ request
    ///
    /// Publishes the request to relays (if configured) and stores locally.
    pub async fn broadcast_rfq(&self, request: RfqRequest) -> Result<String> {
        let request_id = request.id.clone();

        // Store locally
        self.requests
            .write()
            .await
            .insert(request_id.clone(), request.clone());

        // Initialize quotes list
        self.quotes
            .write()
            .await
            .insert(request_id.clone(), Vec::new());

        // TODO: Publish to relay if configured
        // For now, just return the local ID

        Ok(request_id)
    }

    /// Collect quotes for a request
    ///
    /// Returns quotes received so far for the given request.
    pub async fn collect_quotes(&self, request_id: &str) -> Result<Vec<RfqQuote>> {
        let quotes = self.quotes.read().await;

        let request_quotes = quotes
            .get(request_id)
            .cloned()
            .unwrap_or_default();

        // Filter out expired quotes
        let active_quotes: Vec<RfqQuote> = request_quotes
            .into_iter()
            .filter(|q| !q.is_expired())
            .collect();

        Ok(active_quotes)
    }

    /// Broadcast and wait for quotes
    pub async fn broadcast_and_collect(
        &self,
        request: RfqRequest,
        timeout: Duration,
    ) -> Result<Vec<RfqQuote>> {
        let request_id = self.broadcast_rfq(request).await?;

        // Wait for timeout
        tokio::time::sleep(timeout).await;

        self.collect_quotes(&request_id).await
    }

    /// Get the best quote from a list
    ///
    /// For BUY orders: lowest premium is best
    /// For SELL orders: highest premium is best
    pub fn best_quote(&self, quotes: &[RfqQuote], side: OrderSide) -> Option<RfqQuote> {
        if quotes.is_empty() {
            return None;
        }

        let best = match side {
            OrderSide::Buy => quotes
                .iter()
                .min_by(|a, b| a.premium_pct.partial_cmp(&b.premium_pct).unwrap()),
            OrderSide::Sell => quotes
                .iter()
                .max_by(|a, b| a.premium_pct.partial_cmp(&b.premium_pct).unwrap()),
        };

        best.cloned()
    }

    /// Accept a quote and create a trade
    ///
    /// Returns an order_id that can be used with ExchangeClient.
    pub async fn accept_quote(&self, quote: &RfqQuote) -> Result<String> {
        if quote.is_expired() {
            return Err(Error::Database("Quote has expired".to_string()));
        }

        // Get the original request
        let requests = self.requests.read().await;
        let _request = requests
            .get(&quote.request_id)
            .ok_or_else(|| Error::Database("Request not found".to_string()))?;

        // TODO: In real implementation, would:
        // 1. Send acceptance message to provider
        // 2. Create order based on quote
        // 3. Return trade ID

        // For now, return quote ID as order ID
        Ok(quote.id.clone())
    }

    // ============================================================
    // Provider Side
    // ============================================================

    /// Get pending RFQ requests matching a filter
    pub async fn get_requests(&self, filter: RfqFilter) -> Result<Vec<RfqRequest>> {
        let requests = self.requests.read().await;

        let filtered: Vec<RfqRequest> = requests
            .values()
            .filter(|r| self.request_matches_filter(r, &filter))
            .cloned()
            .collect();

        Ok(filtered)
    }

    /// Submit a quote for an RFQ
    pub async fn submit_quote(&self, quote: RfqQuote) -> Result<String> {
        let quote_id = quote.id.clone();
        let request_id = quote.request_id.clone();

        // Verify request exists
        {
            let requests = self.requests.read().await;
            if !requests.contains_key(&request_id) {
                return Err(Error::Database("Request not found".to_string()));
            }
        }

        // Add quote
        {
            let mut quotes = self.quotes.write().await;
            quotes
                .entry(request_id)
                .or_default()
                .push(quote);
        }

        // TODO: Publish to relay if configured

        Ok(quote_id)
    }

    /// Inject an RFQ request (for testing/mock relay sync)
    pub async fn inject_request(&self, request: RfqRequest) {
        let request_id = request.id.clone();
        self.requests.write().await.insert(request_id.clone(), request);
        self.quotes.write().await.insert(request_id, Vec::new());
    }

    // ============================================================
    // Tag Builders
    // ============================================================

    /// Build NIP-90 tags for an RFQ request
    pub fn build_rfq_request_tags(&self, req: &RfqRequest) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["i".to_string(), req.id.clone(), "text".to_string()],
            vec!["param".to_string(), "side".to_string(), req.side.as_str().to_string()],
            vec!["param".to_string(), "amount_sats".to_string(), req.amount_sats.to_string()],
            vec!["param".to_string(), "currency".to_string(), req.currency.clone()],
            vec!["param".to_string(), "max_premium".to_string(), req.max_premium_pct.to_string()],
            vec!["expiration".to_string(), req.expires_at.to_string()],
        ];

        // Add payment methods
        if !req.payment_methods.is_empty() {
            let mut pm_tag = vec!["param".to_string(), "payment_methods".to_string()];
            pm_tag.extend(req.payment_methods.clone());
            tags.push(pm_tag);
        }

        tags
    }

    /// Build NIP-90 tags for an RFQ quote response
    pub fn build_rfq_quote_tags(&self, quote: &RfqQuote) -> Vec<Vec<String>> {
        vec![
            vec!["e".to_string(), quote.request_id.clone()],
            vec!["p".to_string(), quote.provider_pubkey.clone()],
            vec!["amount".to_string(), quote.amount_sats.to_string()],
            vec!["rate".to_string(), quote.rate.to_string()],
            vec!["premium".to_string(), quote.premium_pct.to_string()],
            vec!["fiat_amount".to_string(), quote.fiat_amount.to_string()],
            vec!["expiration".to_string(), quote.expires_at.to_string()],
            vec!["min_reputation".to_string(), quote.min_reputation.to_string()],
        ]
    }

    // ============================================================
    // Helpers
    // ============================================================

    fn request_matches_filter(&self, req: &RfqRequest, filter: &RfqFilter) -> bool {
        if filter.only_active && req.is_expired() {
            return false;
        }
        if let Some(ref side) = filter.side {
            if &req.side != side {
                return false;
            }
        }
        if let Some(ref currency) = filter.currency {
            if &req.currency != currency {
                return false;
            }
        }
        if let Some(min) = filter.min_amount {
            if req.amount_sats < min {
                return false;
            }
        }
        if let Some(max) = filter.max_amount {
            if req.amount_sats > max {
                return false;
            }
        }
        true
    }

    /// Clear all requests and quotes
    pub async fn clear(&self) {
        self.requests.write().await.clear();
        self.quotes.write().await.clear();
    }
}

impl Default for RfqMarket {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current Unix timestamp in seconds
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfq_request_creation() {
        let req = RfqRequest::new(OrderSide::Buy, 100_000, "USD");

        assert!(req.id.starts_with("rfq-"));
        assert_eq!(req.side, OrderSide::Buy);
        assert_eq!(req.amount_sats, 100_000);
        assert_eq!(req.currency, "USD");
        assert!(!req.is_expired());
    }

    #[test]
    fn test_rfq_request_builder() {
        let req = RfqRequest::new(OrderSide::Sell, 50_000, "EUR")
            .with_pubkey("test_pubkey")
            .with_max_premium(3.0)
            .with_min_premium(-2.0)
            .with_payment_methods(vec!["lightning".to_string()]);

        assert_eq!(req.requestor_pubkey, "test_pubkey");
        assert_eq!(req.max_premium_pct, 3.0);
        assert_eq!(req.min_premium_pct, -2.0);
        assert_eq!(req.payment_methods, vec!["lightning".to_string()]);
    }

    #[test]
    fn test_rfq_request_accepts_premium() {
        let req = RfqRequest::new(OrderSide::Buy, 100_000, "USD")
            .with_max_premium(5.0)
            .with_min_premium(-3.0);

        assert!(req.accepts_premium(0.0));
        assert!(req.accepts_premium(5.0));
        assert!(req.accepts_premium(-3.0));
        assert!(!req.accepts_premium(6.0)); // Too high
        assert!(!req.accepts_premium(-4.0)); // Too low
    }

    #[test]
    fn test_rfq_quote_creation() {
        let req = RfqRequest::new(OrderSide::Buy, 1_000_000, "USD"); // 0.01 BTC
        let quote = RfqQuote::new(&req, 50000.0, 1.0); // $50k rate, 1% premium

        assert!(quote.id.starts_with("quote-"));
        assert_eq!(quote.request_id, req.id);
        assert_eq!(quote.rate, 50000.0);
        assert_eq!(quote.premium_pct, 1.0);
        assert_eq!(quote.amount_sats, 1_000_000);
        // Fiat: (0.01 BTC * $50k * 1.01) * 100 cents = $505 = 50500 cents
        assert_eq!(quote.fiat_amount, 50500);
    }

    #[test]
    fn test_rfq_quote_price_per_sat() {
        let req = RfqRequest::new(OrderSide::Buy, 100_000, "USD");
        let mut quote = RfqQuote::new(&req, 50000.0, 0.0);
        quote.fiat_amount = 5000; // 50 cents per 100k sats

        let pps = quote.price_per_sat();
        assert_eq!(pps, 0.05); // 0.05 cents per sat
    }

    #[tokio::test]
    async fn test_rfq_market_broadcast() {
        let market = RfqMarket::new();

        let req = RfqRequest::new(OrderSide::Buy, 100_000, "USD")
            .with_pubkey("buyer");

        let request_id = market.broadcast_rfq(req).await.unwrap();
        assert!(request_id.starts_with("rfq-"));

        // Should have empty quotes initially
        let quotes = market.collect_quotes(&request_id).await.unwrap();
        assert!(quotes.is_empty());
    }

    #[tokio::test]
    async fn test_rfq_market_submit_quote() {
        let market = RfqMarket::new();

        // Broadcast request
        let req = RfqRequest::new(OrderSide::Buy, 100_000, "USD");
        let request_id = market.broadcast_rfq(req.clone()).await.unwrap();

        // Submit quote
        let quote = RfqQuote::new(&req, 50000.0, 0.5)
            .with_provider("provider_1");
        market.submit_quote(quote).await.unwrap();

        // Collect quotes
        let quotes = market.collect_quotes(&request_id).await.unwrap();
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].premium_pct, 0.5);
    }

    #[tokio::test]
    async fn test_rfq_market_best_quote() {
        let market = RfqMarket::new();

        let req = RfqRequest::new(OrderSide::Buy, 100_000, "USD");
        let request_id = market.broadcast_rfq(req.clone()).await.unwrap();

        // Submit multiple quotes
        let quote1 = RfqQuote::new(&req, 50000.0, 2.0).with_provider("p1");
        let quote2 = RfqQuote::new(&req, 50000.0, 0.5).with_provider("p2");
        let quote3 = RfqQuote::new(&req, 50000.0, 1.5).with_provider("p3");

        market.submit_quote(quote1).await.unwrap();
        market.submit_quote(quote2).await.unwrap();
        market.submit_quote(quote3).await.unwrap();

        let quotes = market.collect_quotes(&request_id).await.unwrap();

        // For BUY orders, lowest premium is best
        let best = market.best_quote(&quotes, OrderSide::Buy).unwrap();
        assert_eq!(best.premium_pct, 0.5);
        assert_eq!(best.provider_pubkey, "p2");

        // For SELL orders, highest premium is best
        let best = market.best_quote(&quotes, OrderSide::Sell).unwrap();
        assert_eq!(best.premium_pct, 2.0);
        assert_eq!(best.provider_pubkey, "p1");
    }

    #[tokio::test]
    async fn test_rfq_filter() {
        let market = RfqMarket::new();

        // Inject some requests
        market
            .inject_request(
                RfqRequest::new(OrderSide::Buy, 100_000, "USD")
                    .with_expiry_secs(300),
            )
            .await;

        market
            .inject_request(
                RfqRequest::new(OrderSide::Sell, 200_000, "USD")
                    .with_expiry_secs(300),
            )
            .await;

        market
            .inject_request(
                RfqRequest::new(OrderSide::Buy, 50_000, "EUR")
                    .with_expiry_secs(300),
            )
            .await;

        // Filter by side
        let buy_requests = market
            .get_requests(RfqFilter {
                side: Some(OrderSide::Buy),
                only_active: true,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(buy_requests.len(), 2);

        // Filter by currency
        let usd_requests = market
            .get_requests(RfqFilter {
                currency: Some("USD".to_string()),
                only_active: true,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(usd_requests.len(), 2);

        // Filter by amount
        let large_requests = market
            .get_requests(RfqFilter {
                min_amount: Some(150_000),
                only_active: true,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(large_requests.len(), 1);
    }

    #[test]
    fn test_build_rfq_request_tags() {
        let market = RfqMarket::new();
        let req = RfqRequest::new(OrderSide::Buy, 100_000, "USD")
            .with_max_premium(5.0)
            .with_payment_methods(vec!["cashu".to_string(), "lightning".to_string()]);

        let tags = market.build_rfq_request_tags(&req);

        assert!(tags.iter().any(|t| t[0] == "i"));
        assert!(tags.iter().any(|t| t[0] == "param" && t[1] == "side" && t[2] == "buy"));
        assert!(tags.iter().any(|t| t[0] == "param" && t[1] == "amount_sats" && t[2] == "100000"));
        assert!(tags.iter().any(|t| t[0] == "param" && t[1] == "currency" && t[2] == "USD"));
        assert!(tags.iter().any(|t| t[0] == "expiration"));
    }

    #[test]
    fn test_build_rfq_quote_tags() {
        let market = RfqMarket::new();
        let req = RfqRequest::new(OrderSide::Buy, 100_000, "USD");
        let quote = RfqQuote::new(&req, 50000.0, 1.0)
            .with_provider("provider_pubkey")
            .with_min_reputation(0.5);

        let tags = market.build_rfq_quote_tags(&quote);

        assert!(tags.iter().any(|t| t[0] == "e"));
        assert!(tags.iter().any(|t| t[0] == "p" && t[1] == "provider_pubkey"));
        assert!(tags.iter().any(|t| t[0] == "rate" && t[1] == "50000"));
        assert!(tags.iter().any(|t| t[0] == "premium" && t[1] == "1"));
        assert!(tags.iter().any(|t| t[0] == "min_reputation" && t[1] == "0.5"));
    }

    #[tokio::test]
    async fn test_accept_quote() {
        let market = RfqMarket::new();

        let req = RfqRequest::new(OrderSide::Buy, 100_000, "USD");
        let _request_id = market.broadcast_rfq(req.clone()).await.unwrap();

        let quote = RfqQuote::new(&req, 50000.0, 0.5)
            .with_provider("provider")
            .with_expiry_secs(60);

        market.submit_quote(quote.clone()).await.unwrap();

        let order_id = market.accept_quote(&quote).await.unwrap();
        assert!(order_id.starts_with("quote-"));
    }

    #[tokio::test]
    async fn test_accept_expired_quote_fails() {
        let market = RfqMarket::new();

        let req = RfqRequest::new(OrderSide::Buy, 100_000, "USD");
        market.broadcast_rfq(req.clone()).await.unwrap();

        // Create expired quote
        let mut quote = RfqQuote::new(&req, 50000.0, 0.5);
        quote.expires_at = 0; // Already expired

        let result = market.accept_quote(&quote).await;
        assert!(result.is_err());
    }
}
