# Exchange API Reference

Complete API documentation for the neobank exchange module, enabling agent-to-agent trading via NIP-69 P2P orders.

## Overview

The exchange module provides:

- **Order Management**: Post, fetch, accept, and cancel orders
- **Settlement**: Execute trades with mock or real settlement
- **Reputation**: NIP-32 attestations for trade outcomes
- **NIP-69 Compatibility**: Interoperable with Mostro, Robosats, lnp2pBot, Peach

## Quick Start

```rust
use neobank::{
    ExchangeClient, OrderParams, OrderSide, OrderStatus,
    Trade, TradeOutcome, SettlementReceipt,
};
use std::time::Duration;

// 1. Create exchange client
let treasury = ExchangeClient::new_mock("treasury_agent_pubkey");
let worker = ExchangeClient::new_mock("worker_agent_pubkey");

// 2. Treasury posts sell order
let order_id = treasury.post_order(OrderParams {
    side: OrderSide::Sell,
    amount_sats: 10_000,
    fiat_amount: 100,  // $1.00 in cents
    currency: "USD".to_string(),
    payment_methods: vec!["cashu".to_string()],
    ..Default::default()
}).await?;

// 3. Worker accepts order (after fetching from relay)
let order = treasury.get_order(&order_id)?.unwrap();
worker.inject_order(order)?;  // Simulates relay sync
let trade = worker.accept_order(&order_id).await?;

// 4. Settlement
let receipt = worker.settle(&trade).await?;

// 5. Both publish attestations
treasury.attest_trade(&trade, TradeOutcome::Success, receipt.duration.as_millis() as u64).await?;
worker.attest_trade(&trade, TradeOutcome::Success, receipt.duration.as_millis() as u64).await?;
```

## Types

### OrderSide

```rust
pub enum OrderSide {
    /// Selling BTC for fiat
    Sell,
    /// Buying BTC with fiat
    Buy,
}
```

### OrderStatus

```rust
pub enum OrderStatus {
    /// Order is open and can be accepted
    Pending,
    /// Order has been accepted, trade in progress
    InProgress,
    /// Trade completed successfully
    Success,
    /// Order was canceled by maker
    Canceled,
    /// Order expired without being accepted
    Expired,
}
```

### OrderParams

Parameters for creating a new order.

```rust
pub struct OrderParams {
    /// Order side (buy or sell BTC)
    pub side: OrderSide,

    /// Amount in satoshis
    pub amount_sats: u64,

    /// Fiat amount in cents (to avoid floating point)
    pub fiat_amount: u64,

    /// Currency code (ISO 4217, e.g., "USD", "EUR")
    pub currency: String,

    /// Premium percentage (negative for discount)
    pub premium_pct: f64,

    /// Accepted payment methods
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
            expires_in: Duration::from_secs(3600),
        }
    }
}
```

### Order

An order in the exchange order book.

```rust
pub struct Order {
    /// Unique order ID
    pub order_id: String,

    /// Maker's public key (hex)
    pub maker_pubkey: String,

    /// Order side
    pub side: OrderSide,

    /// Amount in satoshis
    pub amount_sats: u64,

    /// Fiat amount in cents
    pub fiat_amount: u64,

    /// Currency code
    pub currency: String,

    /// Premium percentage
    pub premium_pct: f64,

    /// Payment methods
    pub payment_methods: Vec<String>,

    /// Current status
    pub status: OrderStatus,

    /// Created timestamp (Unix seconds)
    pub created_at: u64,

    /// Expires timestamp (Unix seconds)
    pub expires_at: u64,
}
```

### Trade

A matched trade between maker and taker.

```rust
pub struct Trade {
    /// Trade ID (same as order ID)
    pub trade_id: String,

    /// The matched order
    pub order: Order,

    /// Taker's public key (hex)
    pub taker_pubkey: String,

    /// Trade status
    pub status: TradeStatus,

    /// When the trade was matched
    pub matched_at: Instant,
}

pub enum TradeStatus {
    Matched,    // Awaiting settlement
    Settling,   // Settlement in progress
    Completed,  // Successfully completed
    Disputed,   // Under dispute
    Canceled,   // Canceled
}
```

### SettlementReceipt

Proof that a trade was settled.

```rust
pub struct SettlementReceipt {
    /// Trade ID
    pub trade_id: String,

    /// Settlement method used
    pub method: SettlementMethod,

    /// Amount settled (sats)
    pub amount_sats: u64,

    /// How long settlement took
    pub duration: Duration,

    /// Method-specific proof (e.g., preimage, txid)
    pub proof: Option<String>,
}

pub enum SettlementMethod {
    Mock,             // For testing
    ReputationBased,  // v0: trust-based
    AtomicCashu,      // v1: atomic P2PK+HTLC
}
```

### TradeOutcome

Possible outcomes for trade attestations.

```rust
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
```

### TradeAttestation

A reputation attestation for a trade.

```rust
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
```

## ExchangeClient

The main client for exchange operations.

### Construction

```rust
impl ExchangeClient {
    /// Create a new exchange client with mock settlement.
    ///
    /// # Arguments
    /// * `pubkey` - Your Nostr public key (hex string)
    pub fn new_mock(pubkey: impl Into<String>) -> Self;

    /// Get our public key
    pub fn pubkey(&self) -> &str;
}
```

### Order Management

```rust
impl ExchangeClient {
    /// Post a new order to the exchange.
    ///
    /// Returns the order ID on success.
    pub async fn post_order(&self, params: OrderParams) -> Result<String>;

    /// Fetch orders from the exchange.
    ///
    /// # Arguments
    /// * `side_filter` - Optional filter by order side
    pub async fn fetch_orders(&self, side_filter: Option<OrderSide>) -> Result<Vec<Order>>;

    /// Accept an order as taker.
    ///
    /// # Arguments
    /// * `order_id` - The order to accept
    ///
    /// Returns the created Trade on success.
    pub async fn accept_order(&self, order_id: &str) -> Result<Trade>;

    /// Cancel an order.
    ///
    /// Only the maker can cancel their own order.
    pub async fn cancel_order(&self, order_id: &str) -> Result<()>;
}
```

### Settlement

```rust
impl ExchangeClient {
    /// Settle a trade.
    ///
    /// In mock mode, simulates settlement with a small delay.
    /// In real mode, would execute eCash transfers.
    pub async fn settle(&self, trade: &Trade) -> Result<SettlementReceipt>;
}
```

### Attestations and Reputation

```rust
impl ExchangeClient {
    /// Publish a trade attestation (NIP-32 label).
    ///
    /// # Arguments
    /// * `trade` - The trade to attest
    /// * `outcome` - Trade outcome (Success, Default, etc.)
    /// * `settlement_ms` - How long settlement took
    pub async fn attest_trade(
        &self,
        trade: &Trade,
        outcome: TradeOutcome,
        settlement_ms: u64,
    ) -> Result<String>;

    /// Get attestations for a pubkey.
    pub fn get_attestations(&self, pubkey: &str) -> Result<Vec<TradeAttestation>>;

    /// Calculate reputation score from attestations.
    ///
    /// Returns a score from 0.0 to 1.0 based on success rate.
    pub fn calculate_reputation(&self, pubkey: &str) -> Result<f64>;
}
```

### Testing Helpers

These methods simulate relay synchronization for testing:

```rust
impl ExchangeClient {
    /// Get a copy of an order (for syncing between clients).
    pub fn get_order(&self, order_id: &str) -> Result<Option<Order>>;

    /// Inject an order (simulates receiving from relay).
    pub fn inject_order(&self, order: Order) -> Result<()>;

    /// Get a copy of a trade (for syncing between clients).
    pub fn get_trade(&self, trade_id: &str) -> Result<Option<Trade>>;

    /// Inject a trade (simulates receiving from relay).
    pub fn inject_trade(&self, trade: Trade) -> Result<()>;
}
```

### NIP Tag Builders

Generate Nostr event tags for NIP-69 orders and NIP-32 attestations:

```rust
impl ExchangeClient {
    /// Build NIP-69 order event tags.
    ///
    /// Returns tags for a kind 38383 event.
    pub fn build_order_tags(&self, params: &OrderParams) -> Vec<Vec<String>>;

    /// Build NIP-32 trade attestation tags.
    ///
    /// Returns tags for a kind 1985 label event.
    pub fn build_attestation_tags(
        &self,
        trade: &Trade,
        outcome: TradeOutcome,
        settlement_ms: u64,
    ) -> Vec<Vec<String>>;
}
```

## NIP-69 Order Tags

The `build_order_tags` method generates these NIP-69 compatible tags:

| Tag | Description | Example |
|-----|-------------|---------|
| `d` | Order identifier | `order-abc123-1703100000-0` |
| `k` | Order type (buy/sell) | `sell` |
| `f` | Fiat currency | `USD` |
| `s` | Status | `pending` |
| `amt` | Amount in sats | `10000` |
| `fa` | Fiat amount (cents) | `100` |
| `pm` | Payment methods | `cashu`, `lightning` |
| `premium` | Premium percentage | `0` |
| `network` | Bitcoin network | `mainnet` |
| `layer` | Settlement layer | `lightning` |
| `expires_at` | Order expiration | Unix timestamp |
| `expiration` | Event expiration | Unix timestamp |
| `y` | Platform tag | `openagents` |
| `z` | Event type | `order` |

Example:
```rust
let tags = exchange.build_order_tags(&OrderParams {
    side: OrderSide::Sell,
    amount_sats: 10_000,
    fiat_amount: 100,
    currency: "USD".to_string(),
    ..Default::default()
});

// Creates Nostr event:
// {
//   "kind": 38383,
//   "content": "",
//   "tags": [
//     ["d", "order-treasury-1703100000-0"],
//     ["k", "sell"],
//     ["f", "USD"],
//     ["s", "pending"],
//     ["amt", "10000"],
//     ["fa", "100"],
//     ["pm", "cashu"],
//     ["premium", "0"],
//     ["network", "mainnet"],
//     ["layer", "lightning"],
//     ["expires_at", "1703103600"],
//     ["expiration", "1703190000"],
//     ["y", "openagents"],
//     ["z", "order"]
//   ]
// }
```

## NIP-32 Attestation Tags

The `build_attestation_tags` method generates these NIP-32 label tags:

| Tag | Description | Example |
|-----|-------------|---------|
| `L` | Label namespace | `exchange/trade` |
| `l` | Label value | `success`, `exchange/trade` |
| `p` | Counterparty pubkey | hex pubkey |
| `e` | Trade event ID | trade ID |
| `amount` | Trade amount (sats) | `10000` |
| `settlement_ms` | Settlement duration | `150` |
| `pair` | Trading pair | `BTC/USD` |

Example:
```rust
let tags = exchange.build_attestation_tags(&trade, TradeOutcome::Success, 150);

// Creates Nostr event:
// {
//   "kind": 1985,
//   "content": "",
//   "tags": [
//     ["L", "exchange/trade"],
//     ["l", "success", "exchange/trade"],
//     ["p", "counterparty_pubkey_hex"],
//     ["e", "trade-123"],
//     ["amount", "10000"],
//     ["settlement_ms", "150"],
//     ["pair", "BTC/USD"]
//   ]
// }
```

## Integration with NIP-SA

The exchange integrates with NIP-SA (Sovereign Agents) for full agent lifecycle:

```rust
use nostr::{
    AgentProfile, AgentProfileContent, AgentState, AgentStateContent,
    TrajectoryEvent, TrajectoryEventContent, StepType,
    finalize_event, EventTemplate,
};
use neobank::{ExchangeClient, OrderParams, OrderSide, TradeOutcome};

// 1. Create sovereign agents with profiles
let treasury_profile = AgentProfile::new(
    AgentProfileContent::new("TreasuryAgent", "Provides liquidity", ...),
    threshold_config,
    &operator_pubkey,
);

// 2. Publish agent profiles (kind 39200)
relay.publish_event(&profile_event).await?;

// 3. Post exchange order (kind 38383)
let order_id = treasury_exchange.post_order(params).await?;
let order_tags = treasury_exchange.build_order_tags(&params);
let order_event = finalize_event(&EventTemplate {
    kind: 38383,
    content: String::new(),
    tags: order_tags,
    ..
}, &secret_key)?;
relay.publish_event(&order_event).await?;

// 4. Execute trade and settle
let trade = worker_exchange.accept_order(&order_id).await?;
let receipt = worker_exchange.settle(&trade).await?;

// 5. Publish attestations (kind 1985)
let attest_tags = treasury_exchange.build_attestation_tags(&trade, TradeOutcome::Success, ...);
let attest_event = finalize_event(&EventTemplate {
    kind: 1985,
    content: String::new(),
    tags: attest_tags,
    ..
}, &secret_key)?;
relay.publish_event(&attest_event).await?;

// 6. Update agent state with new balance (kind 39201)
let mut state_content = AgentStateContent::new();
state_content.update_balance(new_balance);
state_content.add_memory(MemoryEntry::new("trade", "Sold 10k sats"));
// Encrypt and publish...

// 7. Publish trajectory event (kind 39231)
let traj_content = TrajectoryEventContent {
    step_type: StepType::ToolUse,
    data: json!({"action": "post_order", "order_id": order_id}),
};
```

## Error Handling

All methods return `Result<T>` with the crate's error type:

```rust
use neobank::{Error, Result};

match exchange.accept_order(&order_id).await {
    Ok(trade) => println!("Trade created: {}", trade.trade_id),
    Err(Error::Database(msg)) => println!("Database error: {}", msg),
    Err(e) => println!("Error: {:?}", e),
}
```

Common errors:
- `Error::Database("Order not found: ...")` - Order doesn't exist
- `Error::Database("Order not pending: ...")` - Order already accepted
- `Error::Database("Not order maker")` - Can't cancel someone else's order

## Testing

Run the exchange tests:

```bash
# Unit tests
cargo test -p neobank --lib exchange

# E2E demo tests
cargo test -p neobank --test exchange_e2e

# NIP-SA integration test
cargo test -p nostr-integration-tests test_sovereign_agent_exchange_flow
```

## Event Kinds

| Kind | NIP | Description |
|------|-----|-------------|
| 38383 | NIP-69 | P2P Order |
| 1985 | NIP-32 | Label (attestation) |
| 39200 | NIP-SA | Agent Profile |
| 39201 | NIP-SA | Agent State |
| 39231 | NIP-SA | Trajectory Event |

## Relay Integration

The `ExchangeRelay` provides real Nostr relay connectivity for publishing orders and attestations.

```rust
use neobank::{ExchangeClient, ExchangeRelay, SettlementEngine};
use std::sync::Arc;

// Connect to relays
let relay = Arc::new(ExchangeRelay::connect(&[
    "wss://relay.damus.io",
    "wss://relay.nostr.band",
]).await?);

// Create exchange with relay
let secret_key = [1u8; 32]; // Your nostr secret key
let settlement = SettlementEngine::new_mock();
let exchange = ExchangeClient::new_with_relay(
    "my_pubkey",
    secret_key,
    settlement,
    relay,
);

// Orders are now published to real relays
let order_id = exchange.post_order(params).await?;

// Fetch orders with filtering
use neobank::relay::OrderFilter;
let filter = OrderFilter {
    side: Some(OrderSide::Sell),
    currency: Some("USD".to_string()),
    ..Default::default()
};
let orders = exchange.fetch_orders_with_filter(filter).await?;
```

See [relay.rs](../src/relay.rs) for the full API.

## Settlement Modes

Choose from multiple settlement strategies:

| Mode | Use Case | Security |
|------|----------|----------|
| Mock | Testing | None |
| ReputationBased | Established traders | Trust-based |
| AtomicP2PK | High-value trades | Cryptographic |

```rust
use neobank::settlement::{SettlementEngine, SettlementMode};

// Mock for testing
let mock_engine = SettlementEngine::new_mock();

// Reputation-based (higher rep pays first)
let rep_engine = SettlementEngine::new_reputation_based(
    wallet,
    Duration::from_secs(60),
);

// Atomic P2PK (cryptographically secure)
let atomic_engine = SettlementEngine::new_atomic_p2pk(
    wallet,
    Duration::from_secs(300),
);
```

See [SETTLEMENT.md](./SETTLEMENT.md) for detailed documentation.

## Reputation System

Track and query trader reputation based on NIP-32 attestations:

```rust
use neobank::{ReputationService, ReputationConfig, TrustLevel};

let config = ReputationConfig::default();
let rep_service = ReputationService::new(config);

// Fetch reputation
let score = rep_service.fetch_reputation("counterparty_pubkey").await?;
println!("Reputation: {:.2}", score.composite_score());
println!("Trust level: {:?}", rep_service.trust_level(&score));

// Decision helpers
if rep_service.should_pay_first(&my_rep, &their_rep) {
    // We should pay first in reputation-based settlement
}

// Minimum reputation for amount
let min_rep = rep_service.min_reputation_for_amount(1_000_000);
if their_rep.composite_score() < min_rep {
    return Err("Insufficient reputation for this trade amount");
}
```

See [reputation.rs](../src/reputation.rs) for the full API.

## RFQ Market

Request and receive competitive quotes from liquidity providers:

```rust
use neobank::{RfqMarket, RfqRequest, OrderSide};

let market = RfqMarket::new();

// Broadcast RFQ
let request = RfqRequest {
    id: uuid::Uuid::new_v4().to_string(),
    requester_pubkey: "my_pubkey".to_string(),
    side: OrderSide::Buy,
    amount_sats: 100_000,
    currency: "USD".to_string(),
    max_premium_pct: 2.0,
    min_premium_pct: -5.0,
    expires_at: now() + 60,
    min_reputation: 0.5,
};

let request_id = market.broadcast_rfq(request).await?;

// Wait and collect quotes
tokio::time::sleep(Duration::from_secs(5)).await;
let quotes = market.collect_quotes(&request_id).await?;

// Select best quote
if let Some(best) = market.best_quote(&quotes, OrderSide::Buy) {
    let trade_id = market.accept_quote(&best).await?;
}
```

See [RFQ.md](./RFQ.md) for detailed documentation.

## Treasury Agent

Market-making agent for liquidity provision:

```rust
use neobank::{TreasuryAgent, TreasuryAgentConfig, TradingPair};

let config = TreasuryAgentConfig {
    pubkey: "treasury_pubkey".to_string(),
    supported_pairs: vec![TradingPair::BtcUsd],
    spread_bps: 50,  // 0.5% spread
    min_trade_sats: 10_000,
    max_trade_sats: 10_000_000,
    ..Default::default()
};

let agent = TreasuryAgent::new(config);

// Set market rate
agent.set_rate(TradingPair::BtcUsd, 42_000.0).await;

// Get bid/ask
let (bid, ask) = agent.get_bid_ask(TradingPair::BtcUsd).await.unwrap();
println!("Bid: ${:.2}, Ask: ${:.2}", bid, ask);

// Handle incoming RFQ
let quote = agent.handle_rfq(&rfq_request).await?;
```

See [treasury_agent.rs](../src/treasury_agent.rs) for the full API.

## Mint Trust (NIP-87)

Discover and evaluate Cashu mints:

```rust
use neobank::{MintTrustService, MintNetwork, Currency};
use url::Url;

let service = MintTrustService::new();

// Add known mints
service.add_mint(
    "https://mint.cashu.space",
    Currency::Btc,
    MintNetwork::Mainnet,
).await?;

// Check health
let health = service.check_mint_health(
    &Url::parse("https://mint.cashu.space")?
).await?;
println!("Mint healthy: {}", health.is_online);

// Select best mint
let mint_url = service.select_mint(Currency::Btc, 0.7).await?;

// Allowlist/blocklist
service.add_to_allowlist(Url::parse("https://trusted.mint")?).await;
service.add_to_blocklist(Url::parse("https://sketchy.mint")?).await;
```

See [mint_trust.rs](../src/mint_trust.rs) for the full API.

## Escrow System

Collateral and dispute handling for trades:

```rust
use neobank::{EscrowService, EscrowConfig, TradeSide};

let config = EscrowConfig::default();
let escrow_service = EscrowService::new(config);

// Create escrow with 5% bond
let escrow = escrow_service.create_escrow(
    &trade_id,
    trade_amount,
    Some(0.05),  // 5% bond
).await?;

// Both parties fund their bonds
escrow_service.fund_escrow(&escrow.id, TradeSide::Maker, &maker_pubkey).await?;
escrow_service.fund_escrow(&escrow.id, TradeSide::Taker, &taker_pubkey).await?;

// On successful trade
escrow_service.release_escrow(&escrow.id).await?;

// Or if dispute needed
let dispute_id = escrow_service.initiate_dispute(
    &escrow.id,
    &initiator_pubkey,
    "Counterparty did not deliver",
).await?;

// Resolve dispute (by oracle/mediator)
escrow_service.resolve_dispute(
    &dispute_id,
    &winner_pubkey,
    Some("Evidence reviewed"),
).await?;
```

See [escrow.rs](../src/escrow.rs) for the full API.

## Complete Module Reference

| Module | Purpose | Documentation |
|--------|---------|---------------|
| `exchange` | Core order/trade management | This document |
| `settlement` | Fund transfer protocols | [SETTLEMENT.md](./SETTLEMENT.md) |
| `relay` | Nostr relay integration | [relay.rs](../src/relay.rs) |
| `reputation` | Trader reputation scoring | [reputation.rs](../src/reputation.rs) |
| `rfq` | Request for quote market | [RFQ.md](./RFQ.md) |
| `treasury_agent` | Market making services | [treasury_agent.rs](../src/treasury_agent.rs) |
| `mint_trust` | Mint discovery (NIP-87) | [mint_trust.rs](../src/mint_trust.rs) |
| `escrow` | Collateral and disputes | [escrow.rs](../src/escrow.rs) |
| `wallet` | Cashu wallet operations | [wallet.rs](../src/wallet.rs) |

## See Also

- [EXCHANGE-SPEC.md](./EXCHANGE-SPEC.md) - Design specification
- [SETTLEMENT.md](./SETTLEMENT.md) - Settlement protocol documentation
- [RFQ.md](./RFQ.md) - RFQ market documentation
- [NIP-69](https://github.com/nostr-protocol/nips/blob/master/69.md) - P2P Order Events
- [NIP-32](https://github.com/nostr-protocol/nips/blob/master/32.md) - Labeling
- [NIP-87](https://github.com/nostr-protocol/nips/blob/master/87.md) - Mint Discovery
- [NIP-90](https://github.com/nostr-protocol/nips/blob/master/90.md) - Data Vending Machines
- [nip_sa.rs](../../nostr/tests/integration/nip_sa.rs) - Integration tests
