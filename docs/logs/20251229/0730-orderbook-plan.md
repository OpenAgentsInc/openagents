# NIP-69 Orderbook Viewer MVP Implementation Plan

## Overview

Build a standalone CLI tool that connects to Nostr relays, subscribes to NIP-69 order events (kind 38383), maintains in-memory orderbook state, and displays live order flows.

## Design Decisions (User Confirmed)

- **Location**: New crate at `crates/orderbook/`
- **Default relays**: Mostro relays (wss://relay.mostro.network)
- **Persistence**: In-memory only (no SQLite for MVP)
- **NIP-32 labels**: Skip for MVP, focus on core NIP-69 orders

## Crate Structure

```
crates/orderbook/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Core library exports
│   ├── main.rs             # CLI entry point
│   ├── state.rs            # OrderbookState: in-memory order tracking
│   ├── viewer.rs           # Terminal display (raw feed + aggregated view)
│   ├── parser.rs           # Lenient NIP-69 event parsing with validation notes
│   └── market.rs           # Market key grouping (f, network, layer)
```

## Implementation Steps

### Step 1: Create Crate Skeleton

Create `crates/orderbook/Cargo.toml`:
```toml
[package]
name = "orderbook"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "orderbook"
path = "src/main.rs"

[dependencies]
nostr = { path = "../nostr/core" }
nostr-client = { path = "../nostr/client" }
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = "0.4"
tracing = "0.1"
tracing-subscriber = "0.3"
```

Add to workspace `Cargo.toml`:
```toml
members = [..., "crates/orderbook"]
```

### Step 2: Define OrderCoord and State Types (`src/state.rs`)

```rust
/// Order coordinate: (kind, pubkey, d-tag) - uniquely identifies an addressable event
pub struct OrderCoord {
    pub kind: u16,
    pub pubkey: String,
    pub d_tag: String,
}

/// Parsed order with validation notes
pub struct ParsedOrder {
    pub event_id: String,
    pub coord: OrderCoord,
    pub created_at: u64,
    pub relay_url: String,
    // NIP-69 fields (all optional for lenient parsing)
    pub side: Option<String>,      // "buy" or "sell"
    pub currency: Option<String>,  // ISO 4217
    pub status: Option<String>,    // pending, canceled, in-progress, success, expired
    pub amount_sats: Option<u64>,
    pub fiat_amount: Vec<u64>,     // single value or range
    pub premium: Option<f64>,
    pub payment_methods: Vec<String>,
    pub network: Option<String>,
    pub layer: Option<String>,
    pub expires_at: Option<u64>,
    pub expiration: Option<u64>,
    pub platform: Option<String>,
    pub source: Option<String>,
    pub name: Option<String>,
    pub geohash: Option<String>,
    pub bond: Option<u64>,
    // Validation
    pub validation_errors: Vec<String>,
    pub is_valid: bool,
}

/// In-memory orderbook state
pub struct OrderbookState {
    /// Latest state per order coordinate
    orders: HashMap<OrderCoord, ParsedOrder>,
    /// History per order coordinate (optional)
    history: HashMap<OrderCoord, Vec<ParsedOrder>>,
    /// Raw event feed (chronological)
    raw_feed: VecDeque<ParsedOrder>,
    /// Sequence counter
    sequence: u64,
}
```

Key methods:
- `process_event(&mut self, event: Event, relay_url: &str)` - parse and update state
- `get_order(&self, coord: &OrderCoord) -> Option<&ParsedOrder>`
- `get_orders_by_market(&self, market: &MarketKey) -> Vec<&ParsedOrder>`
- `get_raw_feed(&self, limit: usize) -> Vec<&ParsedOrder>`

### Step 3: Lenient Parser (`src/parser.rs`)

Reuse `nostr::nip69::P2POrder::from_event()` but wrap with lenient fallback:

```rust
pub fn parse_order_lenient(event: &Event, relay_url: &str) -> ParsedOrder {
    let mut errors = Vec::new();

    // Extract d-tag (required for coordinate)
    let d_tag = extract_tag(&event.tags, "d");
    if d_tag.is_none() {
        errors.push("Missing required 'd' tag".to_string());
    }

    // Try strict parsing first
    if let Ok(strict) = nostr::nip69::P2POrder::from_event(event.clone()) {
        return ParsedOrder::from_strict(strict, relay_url, vec![]);
    }

    // Fall back to lenient field-by-field extraction
    ParsedOrder {
        event_id: event.id.clone(),
        coord: OrderCoord { kind: event.kind, pubkey: event.pubkey.clone(), d_tag: d_tag.unwrap_or_default() },
        // ... extract each field with individual error tracking
        validation_errors: errors,
        is_valid: errors.is_empty(),
    }
}
```

### Step 4: Market Grouping (`src/market.rs`)

```rust
/// Market key for grouping orders
#[derive(Hash, Eq, PartialEq, Clone)]
pub struct MarketKey {
    pub currency: String,    // "USD", "EUR", etc.
    pub network: String,     // "mainnet", "testnet"
    pub layer: String,       // "lightning", "onchain", "liquid"
}

impl ParsedOrder {
    pub fn market_key(&self) -> MarketKey {
        MarketKey {
            currency: self.currency.clone().unwrap_or("UNKNOWN".to_string()),
            network: self.network.clone().unwrap_or("mainnet".to_string()),
            layer: self.layer.clone().unwrap_or("lightning".to_string()),
        }
    }
}
```

### Step 5: CLI Entry Point (`src/main.rs`)

```rust
#[derive(Parser)]
#[command(name = "orderbook")]
#[command(about = "NIP-69 orderbook viewer - watch live P2P order flows from Nostr")]
struct Args {
    /// Relay URLs to connect to
    #[arg(long, default_values_t = vec![
        "wss://relay.mostro.network".to_string(),
    ])]
    relays: Vec<String>,

    /// Filter by fiat currency (ISO 4217)
    #[arg(long)]
    currency: Option<String>,

    /// Filter by network (mainnet, testnet, signet)
    #[arg(long)]
    network: Option<String>,

    /// Filter by layer (onchain, lightning, liquid)
    #[arg(long)]
    layer: Option<String>,

    /// Show canceled/expired orders
    #[arg(long)]
    show_inactive: bool,

    /// Only show orders created after this timestamp
    #[arg(long)]
    since: Option<u64>,

    /// Output as JSON (one event per line)
    #[arg(long)]
    json: bool,

    /// Refresh interval for aggregated view (ms)
    #[arg(long, default_value = "1000")]
    refresh_ms: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::init();

    // Create state
    let state = Arc::new(RwLock::new(OrderbookState::new()));

    // Connect to relays and subscribe
    let filter = build_filter(&args);
    for relay_url in &args.relays {
        let relay = RelayConnection::new(relay_url)?;
        relay.connect().await?;

        let rx = relay.subscribe_with_channel("orders", &[filter.clone()]).await?;

        // Spawn event processor
        let state_clone = state.clone();
        let relay_url_clone = relay_url.clone();
        tokio::spawn(async move {
            process_events(rx, state_clone, relay_url_clone).await;
        });
    }

    // Run display loop
    run_display(state, &args).await
}
```

### Step 6: Terminal Display (`src/viewer.rs`)

Two modes:
1. **Raw feed mode** (default): Stream events as they arrive
2. **Aggregated view mode** (`--aggregate`): Periodic refresh showing top-of-book per market

Raw feed output per event:
```
[2025-12-29 10:15:23] NEW ORDER
  Event: abc123... | Relay: wss://relay.mostro.network
  Coord: 38383:pubkey123...:order-456
  Side: SELL | Status: pending | Currency: USD
  Amount: 10,000 sats | Fiat: $100-500 | Premium: +2.5%
  Payment: bank_transfer, wise | Layer: lightning | Network: mainnet
  Platform: mostro | Expires: 2025-12-29 11:15:23
  ⚠️ Warnings: None
```

Aggregated view (periodic refresh):
```
=== NIP-69 Orderbook Viewer ===
Connected: 1 relay(s) | Events: 47 | Orders: 23 active

--- USD/lightning/mainnet ---
  BIDS (BUY)           ASKS (SELL)
  10,000 @ +1.5%       5,000 @ +2.0%
  25,000 @ +1.0%       15,000 @ +2.5%
  ...                  ...

--- EUR/lightning/mainnet ---
  (no orders)
```

### Step 7: Workspace Integration

Update root `Cargo.toml`:
```toml
[workspace]
members = [
    # ... existing members
    "crates/orderbook",
]
```

## Key Files to Create

| File | Purpose |
|------|---------|
| `crates/orderbook/Cargo.toml` | Crate manifest |
| `crates/orderbook/src/lib.rs` | Library exports |
| `crates/orderbook/src/main.rs` | CLI entry point with clap |
| `crates/orderbook/src/state.rs` | OrderbookState, OrderCoord, ParsedOrder |
| `crates/orderbook/src/parser.rs` | Lenient NIP-69 parsing with validation |
| `crates/orderbook/src/market.rs` | MarketKey grouping |
| `crates/orderbook/src/viewer.rs` | Terminal display (raw + aggregated) |

## Key Files to Reuse

| File | What to Reuse |
|------|---------------|
| `crates/nostr/core/src/nip69.rs` | `P2POrder`, `OrderType`, `OrderStatus`, `BitcoinLayer` |
| `crates/nostr/core/src/nip01.rs` | `Event` type |
| `crates/nostr/client/src/relay.rs` | `RelayConnection`, `subscribe_with_channel()` |

## Testing Strategy

1. **Unit tests** (`src/parser.rs`):
   - Parse valid NIP-69 events
   - Handle missing tags gracefully
   - Validate coordinate extraction

2. **Integration tests** (`tests/`):
   - Connect to mock relay
   - Process event stream
   - Verify state updates and coordinate replacement

3. **Manual testing**:
   - Run against wss://relay.mostro.network
   - Verify live order display

## CLI Usage Examples

```bash
# Basic usage - connect to default Mostro relay
cargo run --bin orderbook

# Connect to multiple relays
cargo run --bin orderbook -- --relays wss://relay.mostro.network --relays wss://nos.lol

# Filter by currency
cargo run --bin orderbook -- --currency USD

# JSON output for piping
cargo run --bin orderbook -- --json | jq '.status'

# Show inactive orders
cargo run --bin orderbook -- --show-inactive

# Filter by layer
cargo run --bin orderbook -- --layer lightning --network mainnet
```

## Order of Implementation

1. Create crate structure and Cargo.toml
2. Implement `OrderCoord`, `ParsedOrder`, `OrderbookState` in state.rs
3. Implement lenient parser in parser.rs
4. Implement MarketKey in market.rs
5. Implement CLI with clap in main.rs
6. Implement raw feed display in viewer.rs
7. Add aggregated view display
8. Add unit tests for parser
9. Test against live Mostro relay


