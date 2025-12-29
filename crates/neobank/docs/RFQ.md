# Request for Quote (RFQ) Protocol

Documentation for the neobank RFQ market, enabling agents to request and receive competitive quotes from liquidity providers.

## Overview

The RFQ system allows agents to:

1. **Broadcast Requests**: Ask for quotes on a specific trade
2. **Collect Quotes**: Receive competitive offers from Treasury Agents
3. **Select Best Quote**: Choose the optimal offer
4. **Execute Trade**: Convert the accepted quote into a trade

This is based on NIP-90 (Data Vending Machines) patterns for job request/result flows.

## RFQ Flow

```
┌─────────────┐     1. Broadcast RFQ      ┌──────────────┐
│   Requester │ ───────────────────────► │    Relay     │
│   (Worker)  │                          │              │
└──────┬──────┘                          └──────┬───────┘
       │                                        │
       │                                        │ 2. Distribute to
       │                                        │    Treasury Agents
       │                                        ▼
       │                        ┌───────────────────────────────┐
       │                        │   Treasury Agent 1            │
       │                        │   Treasury Agent 2            │
       │                        │   Treasury Agent 3            │
       │                        └───────────────────────────────┘
       │                                        │
       │                                        │ 3. Submit quotes
       │                                        ▼
       │     4. Collect quotes   ┌──────────────┐
       ◄─────────────────────────│    Relay     │
       │                         └──────────────┘
       │
       │ 5. Select best quote
       │
       │ 6. Accept quote ──────────────────────────────────────►
       │
       │ 7. Execute trade ◄────────────────────────────────────
       │
       ▼
   ┌─────────┐
   │ Success │
   └─────────┘
```

## Types

### RfqRequest

A request for quotes from liquidity providers.

```rust
pub struct RfqRequest {
    /// Unique request ID
    pub id: String,

    /// Requester's pubkey
    pub requester_pubkey: String,

    /// Order side (from requester perspective)
    /// Buy = wants to buy BTC with fiat
    /// Sell = wants to sell BTC for fiat
    pub side: OrderSide,

    /// Amount in satoshis
    pub amount_sats: u64,

    /// Target fiat currency
    pub currency: String,

    /// Maximum acceptable premium (for buys)
    pub max_premium_pct: f64,

    /// Minimum acceptable premium (for sells, can be negative)
    pub min_premium_pct: f64,

    /// Expiration timestamp (Unix seconds)
    pub expires_at: u64,

    /// Minimum provider reputation score
    pub min_reputation: f64,
}
```

### RfqQuote

A quote from a liquidity provider (Treasury Agent).

```rust
pub struct RfqQuote {
    /// Quote ID
    pub id: String,

    /// Request this quote responds to
    pub request_id: String,

    /// Provider's pubkey
    pub provider_pubkey: String,

    /// Exchange rate (fiat per BTC)
    pub rate: f64,

    /// Premium percentage
    pub premium_pct: f64,

    /// Fiat amount for the requested sats
    pub fiat_amount: u64,

    /// Quote expiration
    pub expires_at: u64,

    /// Minimum required reputation to trade with provider
    pub min_reputation: f64,
}
```

### RfqFilter

Filter for subscribing to RFQ requests.

```rust
pub struct RfqFilter {
    /// Filter by order side
    pub side: Option<OrderSide>,

    /// Minimum amount (sats)
    pub min_amount: Option<u64>,

    /// Maximum amount (sats)
    pub max_amount: Option<u64>,

    /// Filter by currency
    pub currency: Option<String>,

    /// Only from these pubkeys
    pub from_pubkeys: Option<Vec<String>>,
}
```

## RfqMarket API

### Construction

```rust
use neobank::rfq::RfqMarket;
use neobank::relay::ExchangeRelay;
use std::sync::Arc;

impl RfqMarket {
    /// Create a new RFQ market (mock mode)
    pub fn new() -> Self;

    /// Create an RFQ market with relay
    pub fn new_with_relay(
        relay: Arc<ExchangeRelay>,
        secret_key: [u8; 32],
    ) -> Self;
}
```

### Requester Operations

```rust
impl RfqMarket {
    /// Broadcast an RFQ request
    ///
    /// Publishes to relays and returns the request ID.
    pub async fn broadcast_rfq(&self, request: RfqRequest) -> Result<String>;

    /// Collect quotes for a request
    ///
    /// Returns all quotes received for the given request ID.
    pub async fn collect_quotes(&self, request_id: &str) -> Result<Vec<RfqQuote>>;

    /// Get the best quote for the request
    ///
    /// For buy orders: lowest premium
    /// For sell orders: highest premium
    pub fn best_quote(&self, quotes: &[RfqQuote], side: OrderSide) -> Option<RfqQuote>;

    /// Accept a quote and create a trade
    ///
    /// Returns the trade ID.
    pub async fn accept_quote(&self, quote: &RfqQuote) -> Result<String>;
}
```

### Provider Operations (Treasury Agent)

```rust
impl RfqMarket {
    /// Subscribe to RFQ requests
    ///
    /// Returns a channel that receives matching requests.
    pub async fn subscribe_rfqs(&self, filter: RfqFilter) -> Result<mpsc::Receiver<RfqRequest>>;

    /// Submit a quote for a request
    pub async fn submit_quote(&self, quote: RfqQuote) -> Result<String>;
}
```

### Query Operations

```rust
impl RfqMarket {
    /// Get an RFQ request by ID
    pub async fn get_request(&self, request_id: &str) -> Result<Option<RfqRequest>>;

    /// Get quotes for a request
    pub async fn get_quotes(&self, request_id: &str) -> Result<Vec<RfqQuote>>;

    /// Check if a request has expired
    pub fn is_expired(&self, request: &RfqRequest) -> bool;
}
```

### Tag Builders

```rust
impl RfqMarket {
    /// Build NIP-90 request tags
    pub fn build_rfq_tags(&self, request: &RfqRequest) -> Vec<Vec<String>>;

    /// Build NIP-90 quote tags
    pub fn build_quote_tags(&self, quote: &RfqQuote) -> Vec<Vec<String>>;
}
```

## Usage Examples

### Requester (Worker Agent)

```rust
use neobank::{RfqMarket, RfqRequest, OrderSide};
use std::time::Duration;

// Create market
let market = RfqMarket::new();

// Create RFQ request
let request = RfqRequest {
    id: uuid::Uuid::new_v4().to_string(),
    requester_pubkey: "my_pubkey".to_string(),
    side: OrderSide::Buy,
    amount_sats: 100_000,
    currency: "USD".to_string(),
    max_premium_pct: 2.0,  // Accept up to 2% premium
    min_premium_pct: -5.0, // Or discounts up to 5%
    expires_at: now() + 60,
    min_reputation: 0.5,
};

// Broadcast to relays
let request_id = market.broadcast_rfq(request.clone()).await?;

// Wait for quotes
tokio::time::sleep(Duration::from_secs(5)).await;

// Collect quotes
let quotes = market.collect_quotes(&request_id).await?;
println!("Received {} quotes", quotes.len());

// Select best quote
if let Some(best) = market.best_quote(&quotes, OrderSide::Buy) {
    println!(
        "Best quote: {} sats at {}% premium from {}",
        request.amount_sats,
        best.premium_pct,
        best.provider_pubkey
    );

    // Accept and trade
    let trade_id = market.accept_quote(&best).await?;
    println!("Trade created: {}", trade_id);
}
```

### Provider (Treasury Agent)

```rust
use neobank::{
    TreasuryAgent, TreasuryAgentConfig, TradingPair,
    RfqMarket, RfqFilter, RfqQuote, OrderSide,
};

// Create treasury agent
let config = TreasuryAgentConfig {
    pubkey: "treasury_pubkey".to_string(),
    supported_pairs: vec![TradingPair::BtcUsd],
    spread_bps: 50,  // 0.5% spread
    min_trade_sats: 10_000,
    max_trade_sats: 10_000_000,
    ..Default::default()
};

let agent = TreasuryAgent::new(config);
agent.set_rate(TradingPair::BtcUsd, 42_000.0).await;

// Create RFQ market
let market = RfqMarket::new();

// Subscribe to RFQ requests for USD
let filter = RfqFilter {
    currency: Some("USD".to_string()),
    min_amount: Some(10_000),
    max_amount: Some(10_000_000),
    ..Default::default()
};

let mut rx = market.subscribe_rfqs(filter).await?;

// Handle incoming requests
while let Some(request) = rx.recv().await {
    // Generate quote using treasury agent
    match agent.handle_rfq(&request).await {
        Ok(quote) => {
            // Submit quote
            market.submit_quote(quote).await?;
            println!("Submitted quote for request {}", request.id);
        }
        Err(e) => {
            eprintln!("Cannot quote request {}: {}", request.id, e);
        }
    }
}
```

## Quote Selection Strategies

### Best Premium

The default strategy selects by premium:

```rust
// For buy orders: lowest premium wins (cheapest for requester)
// For sell orders: highest premium wins (best price for requester)
let best = market.best_quote(&quotes, OrderSide::Buy);
```

### Reputation-Weighted

Consider reputation in selection:

```rust
use neobank::ReputationService;

let reputation_service = ReputationService::new();

let best = quotes
    .iter()
    .filter(|q| {
        let rep = reputation_service.fetch_reputation(&q.provider_pubkey).await.ok();
        rep.map_or(false, |r| r.composite_score() >= 0.7)
    })
    .min_by(|a, b| a.premium_pct.partial_cmp(&b.premium_pct).unwrap());
```

### Speed-Prioritized

For urgent trades, accept first valid quote:

```rust
// Subscribe and accept first quote immediately
let mut rx = market.subscribe_quotes(&request_id).await?;
if let Some(quote) = rx.recv().await {
    if quote.premium_pct <= request.max_premium_pct {
        let trade_id = market.accept_quote(&quote).await?;
        return Ok(trade_id);
    }
}
```

## NIP-90 Event Structure

### RFQ Request (Kind 5969)

```json
{
  "kind": 5969,
  "content": "",
  "tags": [
    ["d", "rfq-abc123"],
    ["i", "100000", "amount_sats"],
    ["i", "USD", "currency"],
    ["param", "side", "buy"],
    ["param", "max_premium", "2.0"],
    ["param", "min_reputation", "0.5"],
    ["expiration", "1703190000"]
  ]
}
```

### RFQ Quote (Kind 6969)

```json
{
  "kind": 6969,
  "content": "",
  "tags": [
    ["d", "quote-xyz789"],
    ["e", "rfq-abc123"],
    ["p", "requester_pubkey"],
    ["amount", "100000"],
    ["rate", "42000.00"],
    ["premium", "1.5"],
    ["fiat_amount", "4263"],
    ["expiration", "1703189900"]
  ]
}
```

## Integration with Treasury Agent

The Treasury Agent handles RFQ responses automatically:

```rust
use neobank::{TreasuryAgent, TreasuryAgentConfig, TradingPair};

let config = TreasuryAgentConfig {
    pubkey: "treasury".to_string(),
    supported_pairs: vec![TradingPair::BtcUsd],
    spread_bps: 50,
    min_trade_sats: 1_000,
    max_trade_sats: 1_000_000,
    volume_discount_threshold: 500_000,
    volume_discount_bps: 25,
    ..Default::default()
};

let agent = TreasuryAgent::new(config);

// Set market rate
agent.set_rate(TradingPair::BtcUsd, 42_000.0).await;

// Handle incoming RFQ
let quote = agent.handle_rfq(&rfq_request).await?;

// Quote is automatically calculated with:
// - Bid/ask spread applied
// - Volume discounts if applicable
// - Position-based adjustments
```

## Error Handling

```rust
use neobank::{Error, Result};

match market.broadcast_rfq(request).await {
    Ok(request_id) => {
        println!("RFQ broadcast: {}", request_id);
    }
    Err(Error::Network(msg)) => {
        eprintln!("Network error: {}", msg);
    }
    Err(Error::Database(msg)) => {
        // Invalid request parameters
        eprintln!("Invalid request: {}", msg);
    }
    Err(e) => {
        eprintln!("Error: {:?}", e);
    }
}
```

## Best Practices

1. **Set Reasonable Expiration**: 30-60 seconds for active markets
2. **Include Reputation Requirements**: Filter out low-quality providers
3. **Use Premium Ranges**: Set both max and min to capture discounts
4. **Handle Expiration**: Check `is_expired()` before accepting quotes
5. **Verify Provider Reputation**: Always check before large trades

## Testing

```bash
# Run RFQ tests
cargo test -p neobank rfq

# Run integration tests
cargo test -p neobank --test integration_tests test_rfq
```

## See Also

- [EXCHANGE-API.md](./EXCHANGE-API.md) - Exchange client documentation
- [SETTLEMENT.md](./SETTLEMENT.md) - Settlement protocol
- [treasury_agent.rs](../src/treasury_agent.rs) - Treasury Agent implementation
- [NIP-90](https://github.com/nostr-protocol/nips/blob/master/90.md) - Data Vending Machines
