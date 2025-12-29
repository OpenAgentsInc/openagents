# OpenAgents Orderbook Specification

- **Version:** 0.1.0
- **Status:** Draft
- **Last Updated:** 2025-12-29

---

## Abstract

This specification defines the orderbook data structure, matching engine, and order management protocols for the OpenAgents Exchange. The orderbook aggregates NIP-69 peer-to-peer order events into a tradeable market, enabling price discovery, order matching, and efficient execution.

The design prioritizes:
1. **NIP-69 Compatibility** — Orders are standard NIP-69 events (kind 38383)
2. **Decentralization Path** — Centralized matching for v1; relay-based matching planned
3. **Agent-First** — Optimized for high-frequency agent trading patterns
4. **Verifiable Execution** — All matches produce cryptographic proofs

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         ORDERBOOK SYSTEM                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────┐       ┌─────────────────┐                     │
│  │  Order Intake   │       │    Orderbook    │                     │
│  │   (Validator)   │──────▶│   (Price-Time   │                     │
│  │                 │       │    Priority)    │                     │
│  └─────────────────┘       └────────┬────────┘                     │
│          ▲                          │                              │
│          │                          ▼                              │
│          │                 ┌─────────────────┐                     │
│          │                 │ Matching Engine │                     │
│          │                 │  (Continuous)   │                     │
│          │                 └────────┬────────┘                     │
│          │                          │                              │
│          │                          ▼                              │
│  ┌───────┴─────────┐       ┌─────────────────┐                     │
│  │  Nostr Relays   │◀──────│   Execution &   │                     │
│  │  (NIP-69 events)│       │   Settlement    │                     │
│  └─────────────────┘       └─────────────────┘                     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Components

| Component | Responsibility |
|-----------|----------------|
| **Order Intake** | Validate NIP-69 events, signature verification, anti-spam |
| **Orderbook** | Maintain sorted bid/ask levels, aggregate quantities |
| **Matching Engine** | Execute price-time priority matching |
| **Execution** | Generate fills, update order states, trigger settlement |

---

## Orderbook Data Structure

### Price Level

A price level aggregates all orders at the same price:

```rust
/// A single price level in the orderbook
#[derive(Debug, Clone)]
pub struct PriceLevel {
    /// Price in quote currency (e.g., cents per sat for BTC/USD)
    pub price: Decimal,
    
    /// Total quantity at this level (in base currency, e.g., sats)
    pub total_quantity: u64,
    
    /// Number of orders at this level
    pub order_count: u32,
    
    /// Orders sorted by time (FIFO within price level)
    pub orders: VecDeque<OrderId>,
}

impl PriceLevel {
    pub fn new(price: Decimal) -> Self {
        Self {
            price,
            total_quantity: 0,
            order_count: 0,
            orders: VecDeque::new(),
        }
    }

    /// Add an order to this level (appends to back for FIFO)
    pub fn add_order(&mut self, order_id: OrderId, quantity: u64) {
        self.orders.push_back(order_id);
        self.total_quantity += quantity;
        self.order_count += 1;
    }

    /// Remove an order from this level
    pub fn remove_order(&mut self, order_id: &OrderId, quantity: u64) -> bool {
        if let Some(pos) = self.orders.iter().position(|id| id == order_id) {
            self.orders.remove(pos);
            self.total_quantity = self.total_quantity.saturating_sub(quantity);
            self.order_count = self.order_count.saturating_sub(1);
            true
        } else {
            false
        }
    }

    /// Get the first order at this level (for matching)
    pub fn front(&self) -> Option<&OrderId> {
        self.orders.front()
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }
}
```

### Orderbook Side

One side (bid or ask) of the orderbook:

```rust
/// One side of the orderbook (bids or asks)
#[derive(Debug)]
pub struct OrderbookSide {
    /// Price levels sorted by price
    /// Bids: descending (best bid first)
    /// Asks: ascending (best ask first)
    levels: BTreeMap<OrderedDecimal, PriceLevel>,
    
    /// Side indicator
    side: Side,
    
    /// Quick lookup: order_id -> price
    order_prices: HashMap<OrderId, Decimal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Bid,  // Buy orders
    Ask,  // Sell orders
}

impl OrderbookSide {
    pub fn new(side: Side) -> Self {
        Self {
            levels: BTreeMap::new(),
            side,
            order_prices: HashMap::new(),
        }
    }

    /// Best price on this side
    pub fn best_price(&self) -> Option<Decimal> {
        match self.side {
            Side::Bid => self.levels.keys().next_back().map(|k| k.0),
            Side::Ask => self.levels.keys().next().map(|k| k.0),
        }
    }

    /// Best price level
    pub fn best_level(&self) -> Option<&PriceLevel> {
        match self.side {
            Side::Bid => self.levels.values().next_back(),
            Side::Ask => self.levels.values().next(),
        }
    }

    /// Mutable best price level (for matching)
    pub fn best_level_mut(&mut self) -> Option<&mut PriceLevel> {
        match self.side {
            Side::Bid => self.levels.values_mut().next_back(),
            Side::Ask => self.levels.values_mut().next(),
        }
    }

    /// Add an order to this side
    pub fn add_order(&mut self, order_id: OrderId, price: Decimal, quantity: u64) {
        let key = OrderedDecimal(price);
        self.levels
            .entry(key)
            .or_insert_with(|| PriceLevel::new(price))
            .add_order(order_id.clone(), quantity);
        self.order_prices.insert(order_id, price);
    }

    /// Remove an order from this side
    pub fn remove_order(&mut self, order_id: &OrderId) -> Option<(Decimal, u64)> {
        if let Some(price) = self.order_prices.remove(order_id) {
            let key = OrderedDecimal(price);
            if let Some(level) = self.levels.get_mut(&key) {
                // Note: we need to track quantity separately
                // This is simplified; real impl tracks in order storage
                level.remove_order(order_id, 0);
                if level.is_empty() {
                    self.levels.remove(&key);
                }
            }
            Some((price, 0)) // Return removed order info
        } else {
            None
        }
    }

    /// Get depth up to N levels
    pub fn depth(&self, max_levels: usize) -> Vec<(Decimal, u64, u32)> {
        let iter: Box<dyn Iterator<Item = _>> = match self.side {
            Side::Bid => Box::new(self.levels.values().rev()),
            Side::Ask => Box::new(self.levels.values()),
        };

        iter.take(max_levels)
            .map(|level| (level.price, level.total_quantity, level.order_count))
            .collect()
    }

    /// Total quantity across all levels
    pub fn total_quantity(&self) -> u64 {
        self.levels.values().map(|l| l.total_quantity).sum()
    }
}

/// Wrapper for Decimal that implements Ord (for BTreeMap)
#[derive(Debug, Clone, PartialEq, Eq)]
struct OrderedDecimal(Decimal);

impl Ord for OrderedDecimal {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for OrderedDecimal {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
```

### Full Orderbook

```rust
/// Complete orderbook for a trading pair
#[derive(Debug)]
pub struct Orderbook {
    /// Trading pair (e.g., BTC/USD)
    pub pair: TradingPair,
    
    /// Buy orders (bids)
    pub bids: OrderbookSide,
    
    /// Sell orders (asks)
    pub asks: OrderbookSide,
    
    /// Order storage (full order details)
    orders: HashMap<OrderId, Order>,
    
    /// Sequence number for updates
    sequence: u64,
    
    /// Last update timestamp
    last_update: DateTime<Utc>,
}

impl Orderbook {
    pub fn new(pair: TradingPair) -> Self {
        Self {
            pair,
            bids: OrderbookSide::new(Side::Bid),
            asks: OrderbookSide::new(Side::Ask),
            orders: HashMap::new(),
            sequence: 0,
            last_update: Utc::now(),
        }
    }

    /// Best bid price
    pub fn best_bid(&self) -> Option<Decimal> {
        self.bids.best_price()
    }

    /// Best ask price
    pub fn best_ask(&self) -> Option<Decimal> {
        self.asks.best_price()
    }

    /// Spread (ask - bid)
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        }
    }

    /// Spread as percentage of mid price
    pub fn spread_bps(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => {
                let mid = (bid + ask) / Decimal::from(2);
                if mid.is_zero() {
                    None
                } else {
                    Some(((ask - bid) / mid) * Decimal::from(10000))
                }
            }
            _ => None,
        }
    }

    /// Mid price
    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some((bid + ask) / Decimal::from(2)),
            _ => None,
        }
    }

    /// Add an order to the book
    pub fn add_order(&mut self, order: Order) -> OrderbookUpdate {
        let order_id = order.id.clone();
        let side = order.side;
        let price = order.price;
        let quantity = order.remaining_quantity;

        match side {
            Side::Bid => self.bids.add_order(order_id.clone(), price, quantity),
            Side::Ask => self.asks.add_order(order_id.clone(), price, quantity),
        }

        self.orders.insert(order_id.clone(), order);
        self.sequence += 1;
        self.last_update = Utc::now();

        OrderbookUpdate {
            sequence: self.sequence,
            timestamp: self.last_update,
            update_type: UpdateType::Add,
            order_id,
            side,
            price,
            quantity,
        }
    }

    /// Remove an order from the book
    pub fn remove_order(&mut self, order_id: &OrderId) -> Option<OrderbookUpdate> {
        if let Some(order) = self.orders.remove(order_id) {
            let side = order.side;
            match side {
                Side::Bid => self.bids.remove_order(order_id),
                Side::Ask => self.asks.remove_order(order_id),
            };

            self.sequence += 1;
            self.last_update = Utc::now();

            Some(OrderbookUpdate {
                sequence: self.sequence,
                timestamp: self.last_update,
                update_type: UpdateType::Remove,
                order_id: order_id.clone(),
                side,
                price: order.price,
                quantity: order.remaining_quantity,
            })
        } else {
            None
        }
    }

    /// Get an order by ID
    pub fn get_order(&self, order_id: &OrderId) -> Option<&Order> {
        self.orders.get(order_id)
    }

    /// Get mutable order by ID
    pub fn get_order_mut(&mut self, order_id: &OrderId) -> Option<&mut Order> {
        self.orders.get_mut(order_id)
    }

    /// Get orderbook snapshot
    pub fn snapshot(&self, depth: usize) -> OrderbookSnapshot {
        OrderbookSnapshot {
            pair: self.pair.clone(),
            sequence: self.sequence,
            timestamp: self.last_update,
            bids: self.bids.depth(depth),
            asks: self.asks.depth(depth),
        }
    }
}
```

---

## Order Types

### Limit Order

Standard limit order that rests on the book until filled or canceled:

```rust
pub struct Order {
    /// Unique order ID (UUID)
    pub id: OrderId,
    
    /// NIP-69 event ID (if from Nostr)
    pub event_id: Option<EventId>,
    
    /// Maker's Nostr pubkey
    pub maker_pubkey: PublicKey,
    
    /// Trading pair
    pub pair: TradingPair,
    
    /// Order side (bid/ask)
    pub side: Side,
    
    /// Order type
    pub order_type: OrderType,
    
    /// Limit price (in quote currency per base unit)
    pub price: Decimal,
    
    /// Original quantity (in base currency, e.g., sats)
    pub original_quantity: u64,
    
    /// Remaining unfilled quantity
    pub remaining_quantity: u64,
    
    /// Filled quantity
    pub filled_quantity: u64,
    
    /// Order status
    pub status: OrderStatus,
    
    /// Time in force
    pub time_in_force: TimeInForce,
    
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    
    /// Expiration timestamp (from NIP-69 expires_at tag)
    pub expires_at: Option<DateTime<Utc>>,
    
    /// Settlement method preference
    pub settlement: SettlementMethod,
    
    /// Minimum counterparty reputation
    pub min_reputation: Option<f64>,
    
    /// Fills for this order
    pub fills: Vec<Fill>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    /// Standard limit order
    Limit,
    
    /// Market order (takes liquidity at any price)
    Market,
    
    /// Limit order that must be fully filled immediately or canceled
    FillOrKill,
    
    /// Limit order that fills what it can immediately, cancels rest
    ImmediateOrCancel,
    
    /// Limit order that must add liquidity (no immediate matching)
    PostOnly,
    
    /// Stop-limit order (activates at trigger price)
    StopLimit { trigger_price: Decimal },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    /// Good till canceled (rests until filled or canceled)
    GTC,
    
    /// Good till date (expires at specified time)
    GTD,
    
    /// Immediate or cancel
    IOC,
    
    /// Fill or kill
    FOK,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    /// Order is active on the book
    Open,
    
    /// Order is partially filled
    PartiallyFilled,
    
    /// Order is completely filled
    Filled,
    
    /// Order was canceled by maker
    Canceled,
    
    /// Order expired (reached expires_at)
    Expired,
    
    /// Order rejected (failed validation)
    Rejected,
}
```

### NIP-69 Order Mapping

Map between NIP-69 event tags and internal Order structure:

```rust
impl Order {
    /// Parse order from NIP-69 event (kind 38383)
    pub fn from_nip69_event(event: &Event) -> Result<Self, OrderError> {
        // Validate event kind
        if event.kind != Kind::Custom(38383) {
            return Err(OrderError::InvalidKind);
        }

        // Parse tags
        let d_tag = event.tags.find_value("d")
            .ok_or(OrderError::MissingTag("d"))?;
        
        let side = match event.tags.find_value("k") {
            Some("buy") => Side::Bid,
            Some("sell") => Side::Ask,
            _ => return Err(OrderError::InvalidSide),
        };

        let currency = event.tags.find_value("f")
            .ok_or(OrderError::MissingTag("f"))?;

        let status = match event.tags.find_value("s") {
            Some("pending") => OrderStatus::Open,
            Some("in-progress") => OrderStatus::PartiallyFilled,
            Some("success") => OrderStatus::Filled,
            Some("canceled") => OrderStatus::Canceled,
            Some("expired") => OrderStatus::Expired,
            _ => OrderStatus::Open,
        };

        let amount_sats: u64 = event.tags.find_value("amt")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let fiat_amount = parse_fiat_amount(event.tags.find_value("fa"))?;
        
        let premium: Decimal = event.tags.find_value("premium")
            .and_then(|s| s.parse().ok())
            .unwrap_or(Decimal::ZERO);

        let expires_at = event.tags.find_value("expires_at")
            .and_then(|s| s.parse::<i64>().ok())
            .map(|ts| DateTime::from_timestamp(ts, 0))
            .flatten();

        // Calculate price from amount and fiat_amount
        let price = calculate_price(amount_sats, &fiat_amount, premium)?;

        // Parse OpenAgents extensions
        let settlement = parse_settlement(event.tags.find_value("settlement"));
        let min_reputation = event.tags.find_value("min_reputation")
            .and_then(|s| s.parse().ok());

        Ok(Order {
            id: OrderId(d_tag.to_string()),
            event_id: Some(event.id),
            maker_pubkey: event.pubkey,
            pair: TradingPair::new("BTC", currency),
            side,
            order_type: OrderType::Limit,
            price,
            original_quantity: amount_sats,
            remaining_quantity: amount_sats,
            filled_quantity: 0,
            status,
            time_in_force: TimeInForce::GTD,
            created_at: DateTime::from_timestamp(event.created_at.as_i64(), 0)
                .unwrap_or_else(Utc::now),
            updated_at: Utc::now(),
            expires_at,
            settlement,
            min_reputation,
            fills: vec![],
        })
    }

    /// Convert order to NIP-69 event tags
    pub fn to_nip69_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["d".to_string(), self.id.0.clone()],
            vec!["k".to_string(), match self.side {
                Side::Bid => "buy".to_string(),
                Side::Ask => "sell".to_string(),
            }],
            vec!["f".to_string(), self.pair.quote.clone()],
            vec!["s".to_string(), match self.status {
                OrderStatus::Open => "pending".to_string(),
                OrderStatus::PartiallyFilled => "in-progress".to_string(),
                OrderStatus::Filled => "success".to_string(),
                OrderStatus::Canceled => "canceled".to_string(),
                OrderStatus::Expired => "expired".to_string(),
                OrderStatus::Rejected => "canceled".to_string(),
            }],
            vec!["amt".to_string(), self.original_quantity.to_string()],
            vec!["network".to_string(), "mainnet".to_string()],
            vec!["layer".to_string(), "lightning".to_string()],
            vec!["y".to_string(), "openagents".to_string()],
            vec!["z".to_string(), "order".to_string()],
        ];

        if let Some(exp) = self.expires_at {
            tags.push(vec!["expires_at".to_string(), exp.timestamp().to_string()]);
            // NIP-40 expiration (relay deletion hint)
            let expiration = exp + chrono::Duration::days(1);
            tags.push(vec!["expiration".to_string(), expiration.timestamp().to_string()]);
        }

        if let Some(min_rep) = self.min_reputation {
            tags.push(vec!["min_reputation".to_string(), min_rep.to_string()]);
        }

        tags
    }
}
```

---

## Matching Engine

### Price-Time Priority (FIFO)

The matching engine uses price-time priority:
1. **Price Priority**: Best price executes first
2. **Time Priority**: At same price, earlier orders execute first

```rust
/// Matching engine for the orderbook
pub struct MatchingEngine {
    /// The orderbook being matched
    orderbook: Orderbook,
    
    /// Matching configuration
    config: MatchingConfig,
    
    /// Fee calculator
    fee_calculator: FeeCalculator,
    
    /// Reputation service (for filtering)
    reputation: Arc<ReputationService>,
}

#[derive(Debug, Clone)]
pub struct MatchingConfig {
    /// Minimum order size (sats)
    pub min_order_size: u64,
    
    /// Maximum order size (sats)
    pub max_order_size: u64,
    
    /// Price tick size (minimum price increment)
    pub tick_size: Decimal,
    
    /// Lot size (minimum quantity increment)
    pub lot_size: u64,
    
    /// Enable self-trade prevention
    pub self_trade_prevention: SelfTradePrevention,
    
    /// Maximum price deviation from mid (anti-manipulation)
    pub max_price_deviation_pct: Decimal,
}

#[derive(Debug, Clone, Copy)]
pub enum SelfTradePrevention {
    /// No prevention (same maker can match with themselves)
    None,
    /// Cancel incoming order if it would self-trade
    CancelIncoming,
    /// Cancel resting order if it would self-trade
    CancelResting,
    /// Cancel both orders
    CancelBoth,
}

impl MatchingEngine {
    pub fn new(
        pair: TradingPair,
        config: MatchingConfig,
        reputation: Arc<ReputationService>,
    ) -> Self {
        Self {
            orderbook: Orderbook::new(pair),
            config,
            fee_calculator: FeeCalculator::default(),
            reputation,
        }
    }

    /// Process an incoming order
    pub async fn process_order(&mut self, order: Order) -> MatchResult {
        // Validate order
        if let Err(e) = self.validate_order(&order) {
            return MatchResult {
                order_id: order.id,
                status: OrderStatus::Rejected,
                fills: vec![],
                remaining_quantity: order.remaining_quantity,
                reason: Some(e.to_string()),
            };
        }

        match order.order_type {
            OrderType::Market => self.match_market_order(order).await,
            OrderType::Limit => self.match_limit_order(order).await,
            OrderType::FillOrKill => self.match_fok_order(order).await,
            OrderType::ImmediateOrCancel => self.match_ioc_order(order).await,
            OrderType::PostOnly => self.process_post_only(order).await,
            OrderType::StopLimit { trigger_price } => {
                self.process_stop_limit(order, trigger_price).await
            }
        }
    }

    /// Match a limit order (may rest on book)
    async fn match_limit_order(&mut self, mut order: Order) -> MatchResult {
        let mut fills = Vec::new();
        
        // Determine which side to match against
        let opposite_side = match order.side {
            Side::Bid => &mut self.orderbook.asks,
            Side::Ask => &mut self.orderbook.bids,
        };

        // Match against resting orders
        while order.remaining_quantity > 0 {
            let Some(best_level) = opposite_side.best_level_mut() else {
                break;
            };

            // Check price: bid must be >= ask, ask must be <= bid
            let crosses = match order.side {
                Side::Bid => order.price >= best_level.price,
                Side::Ask => order.price <= best_level.price,
            };

            if !crosses {
                break;
            }

            // Match at best level
            while order.remaining_quantity > 0 && !best_level.is_empty() {
                let resting_order_id = best_level.front().unwrap().clone();
                
                // Get resting order
                let resting_order = self.orderbook.orders.get_mut(&resting_order_id)
                    .expect("Order in level must exist in orders map");

                // Self-trade prevention
                if self.would_self_trade(&order, resting_order) {
                    match self.config.self_trade_prevention {
                        SelfTradePrevention::CancelIncoming => {
                            return MatchResult {
                                order_id: order.id,
                                status: OrderStatus::Canceled,
                                fills,
                                remaining_quantity: order.remaining_quantity,
                                reason: Some("Self-trade prevention".to_string()),
                            };
                        }
                        SelfTradePrevention::CancelResting => {
                            best_level.remove_order(&resting_order_id, resting_order.remaining_quantity);
                            continue;
                        }
                        SelfTradePrevention::CancelBoth => {
                            best_level.remove_order(&resting_order_id, resting_order.remaining_quantity);
                            return MatchResult {
                                order_id: order.id,
                                status: OrderStatus::Canceled,
                                fills,
                                remaining_quantity: order.remaining_quantity,
                                reason: Some("Self-trade prevention".to_string()),
                            };
                        }
                        SelfTradePrevention::None => {}
                    }
                }

                // Reputation check
                if !self.passes_reputation_check(&order, resting_order).await {
                    // Skip this order, try next
                    best_level.orders.pop_front();
                    continue;
                }

                // Calculate fill quantity
                let fill_quantity = order.remaining_quantity
                    .min(resting_order.remaining_quantity);
                
                // Execute price is the resting order's price (price improvement for taker)
                let fill_price = resting_order.price;

                // Calculate fees
                let (maker_fee, taker_fee) = self.fee_calculator
                    .calculate_fees(fill_quantity, fill_price);

                // Create fill
                let fill = Fill {
                    id: FillId::new(),
                    taker_order_id: order.id.clone(),
                    maker_order_id: resting_order_id.clone(),
                    quantity: fill_quantity,
                    price: fill_price,
                    taker_side: order.side,
                    maker_fee,
                    taker_fee,
                    timestamp: Utc::now(),
                    settlement_status: SettlementStatus::Pending,
                };

                // Update quantities
                order.remaining_quantity -= fill_quantity;
                order.filled_quantity += fill_quantity;
                resting_order.remaining_quantity -= fill_quantity;
                resting_order.filled_quantity += fill_quantity;

                // Update resting order status
                if resting_order.remaining_quantity == 0 {
                    resting_order.status = OrderStatus::Filled;
                    best_level.remove_order(&resting_order_id, 0);
                } else {
                    resting_order.status = OrderStatus::PartiallyFilled;
                    best_level.total_quantity -= fill_quantity;
                }

                fills.push(fill);
            }

            // Remove empty level
            if best_level.is_empty() {
                let price = best_level.price;
                match order.side {
                    Side::Bid => self.orderbook.asks.levels.remove(&OrderedDecimal(price)),
                    Side::Ask => self.orderbook.bids.levels.remove(&OrderedDecimal(price)),
                };
            }
        }

        // Determine final status
        let status = if order.remaining_quantity == 0 {
            OrderStatus::Filled
        } else if order.filled_quantity > 0 {
            // Rest remaining on book
            order.status = OrderStatus::PartiallyFilled;
            self.orderbook.add_order(order.clone());
            OrderStatus::PartiallyFilled
        } else {
            // No fills, add to book
            order.status = OrderStatus::Open;
            self.orderbook.add_order(order.clone());
            OrderStatus::Open
        };

        MatchResult {
            order_id: order.id,
            status,
            fills,
            remaining_quantity: order.remaining_quantity,
            reason: None,
        }
    }

    /// Match a market order (must execute immediately)
    async fn match_market_order(&mut self, mut order: Order) -> MatchResult {
        // Set extreme price to ensure matching
        order.price = match order.side {
            Side::Bid => Decimal::MAX,
            Side::Ask => Decimal::ZERO,
        };

        let result = self.match_limit_order(order.clone()).await;

        // Market orders don't rest - cancel any remainder
        if result.remaining_quantity > 0 {
            MatchResult {
                status: if result.fills.is_empty() {
                    OrderStatus::Canceled
                } else {
                    OrderStatus::PartiallyFilled
                },
                reason: Some("Insufficient liquidity".to_string()),
                ..result
            }
        } else {
            result
        }
    }

    /// Fill-or-kill: must fill entirely or not at all
    async fn match_fok_order(&mut self, order: Order) -> MatchResult {
        // Check if full fill is possible
        let available = self.available_liquidity_at_price(
            order.side.opposite(),
            order.price,
        );

        if available < order.original_quantity {
            return MatchResult {
                order_id: order.id,
                status: OrderStatus::Canceled,
                fills: vec![],
                remaining_quantity: order.original_quantity,
                reason: Some("FOK: insufficient liquidity".to_string()),
            };
        }

        // Execute as limit order
        self.match_limit_order(order).await
    }

    /// Immediate-or-cancel: fill what's available, cancel rest
    async fn match_ioc_order(&mut self, order: Order) -> MatchResult {
        let result = self.match_limit_order(order.clone()).await;

        // Don't rest on book - remove if partially filled
        if result.remaining_quantity > 0 && result.status == OrderStatus::PartiallyFilled {
            self.orderbook.remove_order(&order.id);
        }

        MatchResult {
            status: match result.fills.len() {
                0 => OrderStatus::Canceled,
                _ if result.remaining_quantity == 0 => OrderStatus::Filled,
                _ => OrderStatus::Canceled, // IOC cancels unfilled portion
            },
            ..result
        }
    }

    /// Post-only: must add liquidity (no immediate matching)
    async fn process_post_only(&mut self, order: Order) -> MatchResult {
        // Check if order would match
        let would_match = match order.side {
            Side::Bid => self.orderbook.best_ask()
                .map(|ask| order.price >= ask)
                .unwrap_or(false),
            Side::Ask => self.orderbook.best_bid()
                .map(|bid| order.price <= bid)
                .unwrap_or(false),
        };

        if would_match {
            return MatchResult {
                order_id: order.id,
                status: OrderStatus::Canceled,
                fills: vec![],
                remaining_quantity: order.original_quantity,
                reason: Some("Post-only: would take liquidity".to_string()),
            };
        }

        // Add to book
        self.orderbook.add_order(order.clone());

        MatchResult {
            order_id: order.id,
            status: OrderStatus::Open,
            fills: vec![],
            remaining_quantity: order.remaining_quantity,
            reason: None,
        }
    }

    fn would_self_trade(&self, incoming: &Order, resting: &Order) -> bool {
        incoming.maker_pubkey == resting.maker_pubkey
    }

    async fn passes_reputation_check(&self, incoming: &Order, resting: &Order) -> bool {
        // Check incoming order's min_reputation against resting maker
        if let Some(min_rep) = incoming.min_reputation {
            if let Ok(rep) = self.reputation.get(&resting.maker_pubkey).await {
                if rep.overall < min_rep {
                    return false;
                }
            }
        }

        // Check resting order's min_reputation against incoming maker
        if let Some(min_rep) = resting.min_reputation {
            if let Ok(rep) = self.reputation.get(&incoming.maker_pubkey).await {
                if rep.overall < min_rep {
                    return false;
                }
            }
        }

        true
    }

    fn available_liquidity_at_price(&self, side: Side, price: Decimal) -> u64 {
        let book_side = match side {
            Side::Bid => &self.orderbook.bids,
            Side::Ask => &self.orderbook.asks,
        };

        book_side.levels.iter()
            .filter(|(level_price, _)| match side {
                Side::Bid => level_price.0 >= price,
                Side::Ask => level_price.0 <= price,
            })
            .map(|(_, level)| level.total_quantity)
            .sum()
    }

    fn validate_order(&self, order: &Order) -> Result<(), OrderError> {
        // Size limits
        if order.original_quantity < self.config.min_order_size {
            return Err(OrderError::BelowMinSize);
        }
        if order.original_quantity > self.config.max_order_size {
            return Err(OrderError::AboveMaxSize);
        }

        // Lot size
        if order.original_quantity % self.config.lot_size != 0 {
            return Err(OrderError::InvalidLotSize);
        }

        // Tick size (price precision)
        // Implementation depends on Decimal precision handling

        // Price deviation from mid (anti-manipulation)
        if let Some(mid) = self.orderbook.mid_price() {
            let deviation = ((order.price - mid) / mid).abs() * Decimal::from(100);
            if deviation > self.config.max_price_deviation_pct {
                return Err(OrderError::PriceDeviationTooLarge);
            }
        }

        // Expiration check
        if let Some(exp) = order.expires_at {
            if exp <= Utc::now() {
                return Err(OrderError::AlreadyExpired);
            }
        }

        Ok(())
    }
}
```

### Match Result

```rust
/// Result of order matching
#[derive(Debug)]
pub struct MatchResult {
    /// The order that was processed
    pub order_id: OrderId,
    
    /// Final status
    pub status: OrderStatus,
    
    /// Fills generated
    pub fills: Vec<Fill>,
    
    /// Remaining unfilled quantity
    pub remaining_quantity: u64,
    
    /// Rejection reason (if status is Canceled/Rejected)
    pub reason: Option<String>,
}

/// A single fill (partial or complete execution)
#[derive(Debug, Clone)]
pub struct Fill {
    /// Unique fill ID
    pub id: FillId,
    
    /// Taker order ID
    pub taker_order_id: OrderId,
    
    /// Maker order ID
    pub maker_order_id: OrderId,
    
    /// Quantity filled (in base currency)
    pub quantity: u64,
    
    /// Execution price
    pub price: Decimal,
    
    /// Taker's side
    pub taker_side: Side,
    
    /// Maker fee (sats)
    pub maker_fee: u64,
    
    /// Taker fee (sats)
    pub taker_fee: u64,
    
    /// Fill timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Settlement status
    pub settlement_status: SettlementStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementStatus {
    /// Settlement not yet initiated
    Pending,
    /// Settlement in progress
    InProgress,
    /// Settlement completed
    Settled,
    /// Settlement failed
    Failed,
}
```

---

## Orderbook Updates & Streaming

### Update Events

```rust
/// Orderbook update event (for streaming)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookUpdate {
    /// Sequence number for ordering updates
    pub sequence: u64,
    
    /// Update timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Type of update
    pub update_type: UpdateType,
    
    /// Affected order ID
    pub order_id: OrderId,
    
    /// Affected side
    pub side: Side,
    
    /// Price level
    pub price: Decimal,
    
    /// Quantity change
    pub quantity: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum UpdateType {
    /// New order added
    Add,
    /// Order removed (canceled, filled, expired)
    Remove,
    /// Order quantity changed (partial fill)
    Modify,
    /// Trade executed
    Trade,
}
```

### Orderbook Snapshot

```rust
/// Point-in-time orderbook snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookSnapshot {
    /// Trading pair
    pub pair: TradingPair,
    
    /// Sequence number
    pub sequence: u64,
    
    /// Snapshot timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Bid levels: (price, quantity, order_count)
    pub bids: Vec<(Decimal, u64, u32)>,
    
    /// Ask levels: (price, quantity, order_count)
    pub asks: Vec<(Decimal, u64, u32)>,
}

impl OrderbookSnapshot {
    /// Convert to L2 market data format
    pub fn to_l2(&self) -> L2Snapshot {
        L2Snapshot {
            pair: self.pair.clone(),
            timestamp: self.timestamp,
            bids: self.bids.iter()
                .map(|(p, q, _)| (*p, *q))
                .collect(),
            asks: self.asks.iter()
                .map(|(p, q, _)| (*p, *q))
                .collect(),
        }
    }
}

/// L2 snapshot (price/quantity only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2Snapshot {
    pub pair: TradingPair,
    pub timestamp: DateTime<Utc>,
    pub bids: Vec<(Decimal, u64)>,
    pub asks: Vec<(Decimal, u64)>,
}
```

### Streaming Protocol

Orderbook updates can be streamed via:

1. **Nostr Events** — Publish updates as ephemeral events
2. **WebSocket** — Direct connection to matching engine
3. **Server-Sent Events** — HTTP streaming for web clients

```rust
/// Orderbook streaming service
pub struct OrderbookStream {
    /// Update channel
    tx: broadcast::Sender<OrderbookUpdate>,
    
    /// Snapshot channel (periodic snapshots)
    snapshot_tx: broadcast::Sender<OrderbookSnapshot>,
}

impl OrderbookStream {
    /// Subscribe to updates
    pub fn subscribe(&self) -> broadcast::Receiver<OrderbookUpdate> {
        self.tx.subscribe()
    }

    /// Subscribe to periodic snapshots
    pub fn subscribe_snapshots(&self) -> broadcast::Receiver<OrderbookSnapshot> {
        self.snapshot_tx.subscribe()
    }

    /// Publish update
    pub fn publish(&self, update: OrderbookUpdate) {
        let _ = self.tx.send(update);
    }

    /// Publish snapshot
    pub fn publish_snapshot(&self, snapshot: OrderbookSnapshot) {
        let _ = self.snapshot_tx.send(snapshot);
    }
}
```

### Nostr Orderbook Events (Extension)

For decentralized orderbook distribution, publish aggregated updates:

```json
{
  "kind": 38384,
  "pubkey": "<matcher_pubkey>",
  "tags": [
    ["d", "orderbook:BTC/USD"],
    ["pair", "BTC/USD"],
    ["sequence", "12345"],
    ["best_bid", "41950.00"],
    ["best_ask", "42050.00"],
    ["spread_bps", "24"]
  ],
  "content": "{\"bids\":[[41950,100000],[41900,250000]],\"asks\":[[42050,150000],[42100,200000]]}",
  "created_at": 1735400000
}
```

---

## Fee Structure

### Fee Calculation

```rust
/// Fee calculator for trades
pub struct FeeCalculator {
    /// Maker fee rate (basis points)
    pub maker_fee_bps: u32,
    
    /// Taker fee rate (basis points)
    pub taker_fee_bps: u32,
    
    /// Volume discount tiers
    pub volume_tiers: Vec<VolumeTier>,
}

#[derive(Debug, Clone)]
pub struct VolumeTier {
    /// Minimum 30-day volume (sats) for this tier
    pub min_volume: u64,
    
    /// Maker fee at this tier (bps)
    pub maker_fee_bps: u32,
    
    /// Taker fee at this tier (bps)
    pub taker_fee_bps: u32,
}

impl Default for FeeCalculator {
    fn default() -> Self {
        Self {
            // Default: 0.1% maker, 0.3% taker
            maker_fee_bps: 10,
            taker_fee_bps: 30,
            volume_tiers: vec![
                VolumeTier {
                    min_volume: 10_000_000,  // 0.1 BTC
                    maker_fee_bps: 8,
                    taker_fee_bps: 25,
                },
                VolumeTier {
                    min_volume: 100_000_000,  // 1 BTC
                    maker_fee_bps: 5,
                    taker_fee_bps: 20,
                },
                VolumeTier {
                    min_volume: 1_000_000_000,  // 10 BTC
                    maker_fee_bps: 2,
                    taker_fee_bps: 15,
                },
            ],
        }
    }
}

impl FeeCalculator {
    /// Calculate fees for a fill
    pub fn calculate_fees(&self, quantity: u64, price: Decimal) -> (u64, u64) {
        // Fee is calculated on the quote currency amount
        let notional = Decimal::from(quantity) * price;
        
        let maker_fee = (notional * Decimal::from(self.maker_fee_bps) 
            / Decimal::from(10000)).to_u64().unwrap_or(0);
        let taker_fee = (notional * Decimal::from(self.taker_fee_bps) 
            / Decimal::from(10000)).to_u64().unwrap_or(0);
        
        (maker_fee, taker_fee)
    }

    /// Calculate fees with volume discount
    pub fn calculate_fees_with_discount(
        &self,
        quantity: u64,
        price: Decimal,
        maker_30d_volume: u64,
        taker_30d_volume: u64,
    ) -> (u64, u64) {
        let maker_tier = self.get_tier(maker_30d_volume);
        let taker_tier = self.get_tier(taker_30d_volume);
        
        let notional = Decimal::from(quantity) * price;
        
        let maker_fee_bps = maker_tier.map(|t| t.maker_fee_bps)
            .unwrap_or(self.maker_fee_bps);
        let taker_fee_bps = taker_tier.map(|t| t.taker_fee_bps)
            .unwrap_or(self.taker_fee_bps);
        
        let maker_fee = (notional * Decimal::from(maker_fee_bps) 
            / Decimal::from(10000)).to_u64().unwrap_or(0);
        let taker_fee = (notional * Decimal::from(taker_fee_bps) 
            / Decimal::from(10000)).to_u64().unwrap_or(0);
        
        (maker_fee, taker_fee)
    }

    fn get_tier(&self, volume: u64) -> Option<&VolumeTier> {
        self.volume_tiers.iter()
            .rev()
            .find(|t| volume >= t.min_volume)
    }
}
```

### Fee Distribution

```rust
/// How fees are distributed
#[derive(Debug, Clone)]
pub struct FeeDistribution {
    /// Platform share (basis points of fee)
    pub platform_bps: u32,
    
    /// Liquidity provider rewards (basis points of fee)
    pub lp_rewards_bps: u32,
    
    /// Insurance fund (basis points of fee)
    pub insurance_bps: u32,
}

impl Default for FeeDistribution {
    fn default() -> Self {
        Self {
            platform_bps: 8000,   // 80% to platform
            lp_rewards_bps: 1500, // 15% to LP rewards
            insurance_bps: 500,   // 5% to insurance
        }
    }
}
```

---

## Settlement Integration

After matching, fills must be settled:

```rust
/// Settlement trigger from matching engine
pub struct SettlementTrigger {
    /// Fill to settle
    pub fill: Fill,
    
    /// Maker order details
    pub maker_order: Order,
    
    /// Taker order details
    pub taker_order: Order,
    
    /// Settlement method (from order preferences)
    pub method: SettlementMethod,
}

/// Settlement coordinator
pub struct SettlementCoordinator {
    exchange_client: ExchangeClient,
}

impl SettlementCoordinator {
    /// Initiate settlement for a fill
    pub async fn settle(&self, trigger: SettlementTrigger) -> Result<SettlementReceipt> {
        // Determine settlement method based on both parties' preferences
        let method = self.negotiate_settlement_method(
            &trigger.maker_order,
            &trigger.taker_order,
        );

        match method {
            SettlementMethod::AtomicCashu => {
                self.settle_atomic_cashu(&trigger).await
            }
            SettlementMethod::TrustedCashu => {
                self.settle_trusted_cashu(&trigger).await
            }
            SettlementMethod::Lightning => {
                self.settle_lightning(&trigger).await
            }
            _ => Err(Error::UnsupportedSettlement),
        }
    }

    fn negotiate_settlement_method(
        &self,
        maker: &Order,
        taker: &Order,
    ) -> SettlementMethod {
        // Prefer atomic if both support it
        if maker.settlement == SettlementMethod::AtomicCashu 
            && taker.settlement == SettlementMethod::AtomicCashu {
            SettlementMethod::AtomicCashu
        } else if maker.settlement == SettlementMethod::Lightning 
            || taker.settlement == SettlementMethod::Lightning {
            SettlementMethod::Lightning
        } else {
            SettlementMethod::TrustedCashu
        }
    }
}
```

---

## Anti-Manipulation Measures

### Order Validation

```rust
/// Anti-manipulation checks
pub struct ManipulationGuard {
    /// Maximum orders per pubkey per pair
    pub max_orders_per_pubkey: u32,
    
    /// Maximum order rate (orders per minute)
    pub max_order_rate: u32,
    
    /// Minimum order lifetime (prevent flash orders)
    pub min_order_lifetime: Duration,
    
    /// Maximum price deviation from reference
    pub max_price_deviation_pct: Decimal,
    
    /// Require proof of work on orders
    pub require_pow: bool,
    
    /// Minimum PoW difficulty (NIP-13)
    pub min_pow_difficulty: u8,
}

impl ManipulationGuard {
    /// Check if order passes anti-manipulation filters
    pub fn validate(&self, order: &Order, context: &OrderContext) -> Result<(), ManipulationError> {
        // Rate limiting
        if context.orders_last_minute >= self.max_order_rate {
            return Err(ManipulationError::RateLimited);
        }

        // Order count per pubkey
        if context.active_orders_count >= self.max_orders_per_pubkey {
            return Err(ManipulationError::TooManyOrders);
        }

        // Minimum lifetime
        if let Some(exp) = order.expires_at {
            let lifetime = exp - order.created_at;
            if lifetime < chrono::Duration::from_std(self.min_order_lifetime).unwrap() {
                return Err(ManipulationError::OrderTooShort);
            }
        }

        // Price deviation
        if let Some(ref_price) = context.reference_price {
            let deviation = ((order.price - ref_price) / ref_price).abs();
            if deviation > self.max_price_deviation_pct / Decimal::from(100) {
                return Err(ManipulationError::PriceDeviationTooLarge);
            }
        }

        // Proof of work (NIP-13)
        if self.require_pow {
            if let Some(event_id) = &order.event_id {
                let difficulty = calculate_pow_difficulty(event_id);
                if difficulty < self.min_pow_difficulty {
                    return Err(ManipulationError::InsufficientPoW);
                }
            }
        }

        Ok(())
    }
}

pub struct OrderContext {
    /// Orders from this pubkey in last minute
    pub orders_last_minute: u32,
    
    /// Active orders from this pubkey
    pub active_orders_count: u32,
    
    /// Reference price (e.g., external oracle or recent trade)
    pub reference_price: Option<Decimal>,
}
```

### Wash Trading Detection

```rust
/// Detect potential wash trading
pub fn detect_wash_trading(fills: &[Fill], window: Duration) -> Vec<WashTradingAlert> {
    let mut alerts = Vec::new();
    
    // Group fills by maker/taker pairs
    let mut pair_fills: HashMap<(PublicKey, PublicKey), Vec<&Fill>> = HashMap::new();
    
    for fill in fills {
        // Need to look up pubkeys from order_id
        // This is simplified - real impl needs order storage access
    }

    // Detect patterns:
    // 1. Same maker/taker trading frequently
    // 2. Round-trip trades (A sells to B, B sells back to A)
    // 3. Orders that cross themselves (same pubkey)

    alerts
}

pub struct WashTradingAlert {
    pub alert_type: WashTradingType,
    pub pubkeys: Vec<PublicKey>,
    pub fills: Vec<FillId>,
    pub total_volume: u64,
    pub confidence: f64,
}

pub enum WashTradingType {
    SelfTrade,
    CircularTrade,
    HighFrequencyPair,
}
```

---

## Persistence & Recovery

### Orderbook Persistence

```rust
/// Orderbook persistence layer
pub struct OrderbookStore {
    pool: SqlitePool,
}

impl OrderbookStore {
    /// Save order to database
    pub async fn save_order(&self, order: &Order) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO orders (
                id, event_id, maker_pubkey, pair, side, order_type,
                price, original_quantity, remaining_quantity, filled_quantity,
                status, time_in_force, created_at, updated_at, expires_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                remaining_quantity = excluded.remaining_quantity,
                filled_quantity = excluded.filled_quantity,
                status = excluded.status,
                updated_at = excluded.updated_at
            "#,
            order.id.0,
            order.event_id.map(|e| e.to_hex()),
            order.maker_pubkey.to_hex(),
            order.pair.to_string(),
            order.side.to_string(),
            order.order_type.to_string(),
            order.price.to_string(),
            order.original_quantity as i64,
            order.remaining_quantity as i64,
            order.filled_quantity as i64,
            order.status.to_string(),
            order.time_in_force.to_string(),
            order.created_at.timestamp(),
            order.updated_at.timestamp(),
            order.expires_at.map(|e| e.timestamp()),
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Load all active orders for a pair
    pub async fn load_active_orders(&self, pair: &TradingPair) -> Result<Vec<Order>> {
        let rows = sqlx::query!(
            r#"
            SELECT * FROM orders 
            WHERE pair = ? 
              AND status IN ('open', 'partially_filled')
              AND (expires_at IS NULL OR expires_at > ?)
            ORDER BY created_at ASC
            "#,
            pair.to_string(),
            Utc::now().timestamp(),
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| Order::from_row(&row))
            .collect()
    }

    /// Save fill to database
    pub async fn save_fill(&self, fill: &Fill) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO fills (
                id, taker_order_id, maker_order_id, quantity, price,
                taker_side, maker_fee, taker_fee, timestamp, settlement_status
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            fill.id.0,
            fill.taker_order_id.0,
            fill.maker_order_id.0,
            fill.quantity as i64,
            fill.price.to_string(),
            fill.taker_side.to_string(),
            fill.maker_fee as i64,
            fill.taker_fee as i64,
            fill.timestamp.timestamp(),
            fill.settlement_status.to_string(),
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
```

### Recovery Procedure

```rust
impl MatchingEngine {
    /// Recover orderbook state from database
    pub async fn recover(&mut self, store: &OrderbookStore) -> Result<()> {
        // Load all active orders
        let orders = store.load_active_orders(&self.orderbook.pair).await?;
        
        // Rebuild orderbook
        for order in orders {
            self.orderbook.add_order(order);
        }

        // Verify consistency
        self.verify_consistency()?;

        Ok(())
    }

    fn verify_consistency(&self) -> Result<()> {
        // Check that all orders in levels exist in orders map
        for (_, level) in self.orderbook.bids.levels.iter() {
            for order_id in &level.orders {
                if !self.orderbook.orders.contains_key(order_id) {
                    return Err(Error::InconsistentState);
                }
            }
        }

        for (_, level) in self.orderbook.asks.levels.iter() {
            for order_id in &level.orders {
                if !self.orderbook.orders.contains_key(order_id) {
                    return Err(Error::InconsistentState);
                }
            }
        }

        Ok(())
    }
}
```

---

## API Reference

### OrderbookService

```rust
/// High-level orderbook service
pub struct OrderbookService {
    /// Matching engines by pair
    engines: HashMap<TradingPair, Arc<RwLock<MatchingEngine>>>,
    
    /// Order store
    store: OrderbookStore,
    
    /// Update stream
    stream: OrderbookStream,
    
    /// Nostr client for NIP-69 events
    nostr: Client,
}

impl OrderbookService {
    /// Submit a new order
    pub async fn submit_order(&self, order: Order) -> Result<MatchResult> {
        let engine = self.get_engine(&order.pair)?;
        let mut engine = engine.write().await;
        
        let result = engine.process_order(order.clone()).await;
        
        // Persist order
        self.store.save_order(&order).await?;
        
        // Persist fills
        for fill in &result.fills {
            self.store.save_fill(fill).await?;
        }
        
        // Publish update
        self.stream.publish(OrderbookUpdate {
            sequence: engine.orderbook.sequence,
            timestamp: Utc::now(),
            update_type: match result.status {
                OrderStatus::Filled | OrderStatus::PartiallyFilled => UpdateType::Trade,
                OrderStatus::Open => UpdateType::Add,
                _ => UpdateType::Remove,
            },
            order_id: order.id,
            side: order.side,
            price: order.price,
            quantity: result.remaining_quantity,
        });
        
        Ok(result)
    }

    /// Cancel an order
    pub async fn cancel_order(&self, order_id: &OrderId) -> Result<()> {
        // Find which engine has this order
        for (pair, engine) in &self.engines {
            let mut engine = engine.write().await;
            if let Some(update) = engine.orderbook.remove_order(order_id) {
                // Update order status in store
                if let Some(order) = self.store.get_order(order_id).await? {
                    let mut order = order;
                    order.status = OrderStatus::Canceled;
                    self.store.save_order(&order).await?;
                }
                
                self.stream.publish(update);
                return Ok(());
            }
        }
        
        Err(Error::OrderNotFound)
    }

    /// Get orderbook snapshot
    pub async fn get_snapshot(
        &self,
        pair: &TradingPair,
        depth: usize,
    ) -> Result<OrderbookSnapshot> {
        let engine = self.get_engine(pair)?;
        let engine = engine.read().await;
        Ok(engine.orderbook.snapshot(depth))
    }

    /// Subscribe to orderbook updates
    pub fn subscribe_updates(&self) -> broadcast::Receiver<OrderbookUpdate> {
        self.stream.subscribe()
    }

    /// Get order by ID
    pub async fn get_order(&self, order_id: &OrderId) -> Result<Option<Order>> {
        self.store.get_order(order_id).await
    }

    /// Get fills for an order
    pub async fn get_fills(&self, order_id: &OrderId) -> Result<Vec<Fill>> {
        self.store.get_fills(order_id).await
    }

    fn get_engine(&self, pair: &TradingPair) -> Result<&Arc<RwLock<MatchingEngine>>> {
        self.engines.get(pair).ok_or(Error::PairNotSupported)
    }
}
```

---

## Implementation Phases

### Phase 1: Basic Orderbook (MVP)

- [x] Order data structure with NIP-69 mapping
- [ ] Price level aggregation (bid/ask sides)
- [ ] Limit order matching (price-time priority)
- [ ] Order add/remove/modify
- [ ] Basic fee calculation
- [ ] SQLite persistence

### Phase 2: Order Types & Validation

- [ ] Market orders
- [ ] Fill-or-kill (FOK)
- [ ] Immediate-or-cancel (IOC)
- [ ] Post-only orders
- [ ] Self-trade prevention
- [ ] Anti-manipulation guards

### Phase 3: Streaming & Performance

- [ ] Orderbook update streaming
- [ ] L2 snapshot generation
- [ ] Volume discount tiers
- [ ] Order book depth queries
- [ ] Performance optimization (hot path)

### Phase 4: Decentralization

- [ ] Nostr-based order discovery (NIP-69)
- [ ] Distributed matching (relay-based)
- [ ] Multi-matcher consensus
- [ ] Order book synchronization

---

## References

- [NIP-69: Peer-to-peer Order Events](https://github.com/nostr-protocol/nips/blob/master/69.md)
- [NIP-60: Cashu Wallet](https://github.com/nostr-protocol/nips/blob/master/60.md)
- [NIP-61: Nutzaps](https://github.com/nostr-protocol/nips/blob/master/61.md)
- [NIP-32: Labeling](https://github.com/nostr-protocol/nips/blob/master/32.md)
- [NIP-13: Proof of Work](https://github.com/nostr-protocol/nips/blob/master/13.md)
- [EXCHANGE-SPEC.md](./EXCHANGE-SPEC.md) — Exchange protocol specification
- [SETTLEMENT.md](./SETTLEMENT.md) — Settlement protocol
- [RFQ.md](./RFQ.md) — Request for Quote protocol
