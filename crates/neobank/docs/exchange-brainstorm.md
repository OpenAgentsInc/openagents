# OpenAgents Exchange

**Agent-to-agent financial services marketplace**

Neobank gives agents treasury management. Exchange gives agents a place to trade with each other.

---

## The Vision

Agents don't just spend money—they provide financial services to each other.

```
┌─────────────────────────────────────────────────────────────────────┐
│                     AGENT FINANCIAL STACK                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  EXCHANGE (agents trade with agents)                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐               │
│  │   FX Market  │  │  Liquidity   │  │   Treasury   │               │
│  │   (BTC/USD)  │  │    Pools     │  │   Services   │               │
│  └──────────────┘  └──────────────┘  └──────────────┘               │
│                                                                      │
│  NEOBANK (agent holds assets)                                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐               │
│  │  BTC Wallet  │  │  USD Wallet  │  │    Spark     │               │
│  │   (Cashu)    │  │   (Cashu)    │  │  (Lightning) │               │
│  └──────────────┘  └──────────────┘  └──────────────┘               │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Design Constraint: Minimize Custody

**If OpenAgents Exchange holds funds, it becomes a money transmitter.**

To stay "neobank, not bank," the exchange must default to:

### Non-Custodial Primitives

| Component | Custodial? | Alternative |
|-----------|------------|-------------|
| Order matching | No | Stateless relay/matcher, never touches funds |
| Settlement | Minimal | Peer-to-peer with optional escrow |
| Escrow | Brief | Time-locked, funds held < 1 hour |
| Treasury Agents | No | They custody their own funds, we don't |

### Architecture Principle

```
┌──────────────────────────────────────────────────────────────┐
│  OpenAgents provides: PROTOCOL + CLIENT                       │
│  - Order broadcast (Nostr events)                             │
│  - Matching service (stateless, fee-optional)                 │
│  - Settlement protocol spec                                   │
│  - Reference client implementation                            │
├──────────────────────────────────────────────────────────────┤
│  OpenAgents does NOT provide:                                 │
│  - Custody of user funds                                      │
│  - Counterparty position                                      │
│  - Guaranteed settlement (we facilitate, not guarantee)       │
└──────────────────────────────────────────────────────────────┘
```

Treasury Agents custody their own capital and take their own counterparty risk. We build the rails, not the bank.

---

## What Gets Traded

### 1. Currency Pairs (FX)

**BTC/USD** — The primary pair

```
Agent A: "Selling 100,000 sats for $98 USD"
Agent B: "Buying 100,000 sats, paying $99 USD"
    ↓
Match at $98.50
    ↓
Atomic swap: A sends BTC proofs, B sends USD proofs
```

Why agents trade FX:
- Lock in value (hedge volatility)
- Pay providers in their preferred currency
- Arbitrage between rates
- Speculation (yes, agents can speculate)

### 2. Liquidity

**Lightning ↔ eCash**

```
Agent A: "I have 500k sats in Cashu, need Lightning liquidity"
Agent B: "I'll pay your LN invoice for Cashu proofs + 0.5% fee"
```

Why:
- Some providers only accept Lightning
- Some agents prefer eCash privacy
- Liquidity has value

**Cross-mint liquidity**

```
Agent A: "I have proofs from Mint X, need proofs from Mint Y"
Agent B: "I'll swap 1:1 minus 0.3% fee"
```

Why:
- Mint diversification (don't keep all eggs in one basket)
- Some merchants only accept certain mints
- Mint reputation changes over time

### 3. Treasury Services

Not just swaps—full treasury management as a service.

**Offerings:**
- "I'll manage your treasury for 1% AUM/year"
- "I'll auto-convert your earnings to USD"
- "I'll optimize your payment routing"
- "I'll hedge your BTC exposure"

This is agents hiring agents for financial operations.

### 4. Payment Routing

**Problem:** Agent A has USD, needs to pay BTC invoice

**Service:** Treasury Agent routes the payment

```
Agent A: "Pay this 50,000 sat invoice from my USD balance"
Treasury Agent:
  1. Quotes rate: $51 USD
  2. Takes A's USD proofs
  3. Pays BTC invoice
  4. Keeps spread as profit
```

This is payment routing as a service—similar to what banks do for international wire transfers.

### 5. Forward Contracts

**Problem:** Agent knows it needs to pay 100,000 sats in 7 days, wants to lock rate now

**Service:** Treasury Agent offers forward contract

```
Agent A: "Lock me 100,000 sats at $100 USD, settlement in 7 days"
Treasury Agent: "Done. You owe me $100 in 7 days, I owe you 100k sats"
```

Why:
- Budget certainty
- Hedging
- Treasury planning

### 6. Lending (Future)

**Problem:** Agent needs capital for a job but is short on funds

**Service:** Lending agent provides short-term loan

```
Agent A: "Need 100,000 sats for compute job, will repay in 24h"
Lender Agent: "Here's 100k sats. Repay 101k sats within 24h."
    ↓
Agent A completes job, earns 150k sats
    ↓
Agent A repays 101k sats
    ↓
Both profit
```

This requires:
- Reputation/collateral
- Smart escrow
- Default handling

---

## Participant Types

### Regular Agents (Takers)
- Need FX for payments
- Need liquidity occasionally
- Pay the spread
- Most agents are takers

### Treasury Agents (Makers)
- Hold both BTC and USD
- Quote two-sided markets
- Earn the spread
- Need significant capital
- Run 24/7

### Arbitrage Agents
- Find price differences
- Between mints
- Between exchange and mints
- Between different treasury agents
- Keep markets efficient

### Market Maker Agents
- Specialized treasury agents
- Always quote both sides
- Tight spreads, high volume
- Algorithmic pricing

### Routing Agents
- Specialize in payment routing
- Find cheapest path
- Handle multi-hop conversions
- Earn routing fees

---

## Technical Architecture

### Order Book (Nostr-Based)

```
kind: 38950  # Exchange Order

{
  "kind": 38950,
  "pubkey": "<agent_pubkey>",
  "content": "",
  "tags": [
    ["d", "<order_id>"],
    ["pair", "BTC/USD"],
    ["side", "sell"],           # sell BTC for USD
    ["amount", "100000"],       # 100,000 sats
    ["price", "0.00099"],       # $0.00099 per sat = $99/100k sats
    ["min_fill", "10000"],      # minimum 10k sats
    ["expiry", "1735500000"],   # unix timestamp
    ["settlement", "cashu"],    # settlement method
    ["btc_mint", "https://mint.minibits.cash"],
    ["usd_mint", "https://stablenut.cashu.network"]
  ]
}
```

### Order Matching

**Option A: Decentralized (Nostr relay matching)**
- Relays could run matching engines
- Orders published to relay
- Relay matches and notifies
- Settlement is peer-to-peer

**Option B: Centralized Matcher**
- Single matching engine (we run it)
- Better UX, faster matching
- Single point of failure
- Can migrate to decentralized later

**Option C: RFQ (Request for Quote)**
- Taker broadcasts: "Want to buy 100k sats"
- Makers respond with quotes
- Taker picks best quote
- No order book needed

### Settlement Protocol

#### Settlement v0 (MVP): Trust-Minimized but Not Atomic

Simple flow that works today without special mint support:

```
1. RFQ → Taker broadcasts request
2. Quote → Maker responds with price
3. Accept → Taker signals acceptance
4. Pay First → One side pays LN invoice (establishes trust direction)
5. Deliver → Other side transfers proofs
6. Attestation → Both publish reputation events
```

**Trust direction:**
- If Maker has reputation, Taker pays first
- If Taker has reputation, Maker delivers first
- If neither, require collateral or small test trade

**Risk:** Counterparty could take payment and not deliver. Mitigated by:
- Reputation system (attestations from past trades)
- Optional collateral escrow
- Starting with small amounts

#### Settlement v1: Practical Atomic (LN HTLC + Cashu P2PK)

Uses hashlock so receiver only gets spendable proofs if they reveal preimage:

```
1. Agree on trade (100k sats for $99 USD)
2. Taker generates secret S, sends hash(S)
3. Maker creates P2PK locked proofs: spendable only with S
4. Maker sends locked proofs to Taker
5. Taker creates HODL invoice locked to hash(S)
6. Maker pays invoice, receives preimage S
7. Maker uses S to unlock the Cashu proofs Taker sent
8. Taker already has Maker's proofs, unlockable with S
```

**Atomicity:** Either both sides complete (S revealed) or neither (invoice expires).

**Mint Requirements for v1:**
- NUT-10: Spending conditions (secret structure)
- NUT-11: P2PK (public key locked proofs)
- NUT-12: DLEQ proofs (verify blind signatures)

If mints don't support these, fall back to v0.

#### Cross-Mint Settlement

When parties use different mints:

```
Agent A: Has BTC on Mint X
Agent B: Has USD on Mint Y

Option 1: Intermediate hop
  A melts on Mint X → LN → B mints on Mint Y

Option 2: Treasury Agent bridges
  Treasury Agent holds on both mints, quotes spread
```

#### Settlement Timeout & Failure

```rust
pub struct SettlementConfig {
    /// Maximum time for counterparty to deliver after payment
    pub delivery_timeout: Duration,

    /// Grace period before reputation penalty
    pub grace_period: Duration,

    /// Automatic dispute if no delivery
    pub auto_dispute: bool,
}

impl Default for SettlementConfig {
    fn default() -> Self {
        Self {
            delivery_timeout: Duration::from_secs(300),  // 5 min
            grace_period: Duration::from_secs(60),       // 1 min grace
            auto_dispute: true,
        }
    }
}
```

### Reputation System

Counterparty risk is real. Need reputation:

```
kind: 38955  # Trade Completion Attestation

{
  "kind": 38955,
  "pubkey": "<counterparty>",
  "tags": [
    ["p", "<trader_pubkey>"],
    ["trade", "<trade_id>"],
    ["outcome", "success"],      # or "default", "dispute"
    ["amount", "100000"],
    ["pair", "BTC/USD"],
    ["latency_ms", "1500"]       # settlement time
  ]
}
```

Aggregate into reputation score:
- Trade count
- Volume traded
- Success rate
- Average settlement time
- Dispute rate

### Collateral/Margin

For larger trades or unknown counterparties:

```
Trade: 1,000,000 sats ($1000)
Required collateral: 10% = $100

Agent A posts $100 collateral to escrow
Agent B posts $100 collateral to escrow
Trade executes
Collateral returned to both
```

Collateral forfeited if:
- Party fails to deliver
- Settlement timeout
- Dispute resolved against them

---

## Exchange Services

### 1. Spot Market

Basic order book:
- Limit orders
- Market orders
- Partial fills
- Order cancellation

### 2. RFQ Service

For agents that don't want to manage orders:

```
POST /rfq
{
  "pair": "BTC/USD",
  "side": "buy",
  "amount": 100000,
  "settlement": "cashu"
}

Response:
{
  "quotes": [
    { "maker": "agent_1", "price": 0.00098, "expires": 30 },
    { "maker": "agent_2", "price": 0.000985, "expires": 60 }
  ]
}
```

### 3. Treasury-as-a-Service

API for agents that want managed treasury:

```rust
pub trait TreasuryService {
    /// Deposit assets for management
    async fn deposit(&self, proofs: Vec<Proof>) -> Result<DepositReceipt>;

    /// Request withdrawal
    async fn withdraw(&self, amount: Money, currency: Currency) -> Result<Proof>;

    /// Check managed balance
    async fn balance(&self) -> Result<ManagedBalance>;

    /// Set policy (e.g., "keep 80% in USD")
    async fn set_policy(&self, policy: TreasuryPolicy) -> Result<()>;

    /// Get performance report
    async fn report(&self, period: TimePeriod) -> Result<TreasuryReport>;
}
```

### 4. Payment Router

API for "pay this invoice from my balance":

```rust
pub trait PaymentRouter {
    /// Route payment, auto-converting if needed
    async fn pay(
        &self,
        invoice: &str,
        from_currency: Option<Currency>,  // None = auto-select
        max_fee_pct: f64,
    ) -> Result<PaymentReceipt>;

    /// Preview routing (show conversion path and fees)
    async fn preview(&self, invoice: &str) -> Result<RoutingPreview>;
}
```

### 5. Hedging Service

For agents that want volatility protection:

```rust
pub trait HedgingService {
    /// Lock in rate for future payment
    async fn create_forward(
        &self,
        amount: Money,
        target_currency: Currency,
        settlement_date: DateTime,
    ) -> Result<ForwardContract>;

    /// Get current forward rates
    async fn forward_rates(&self, settlement_date: DateTime) -> Result<ForwardRates>;
}
```

---

## Revenue Model

### Exchange Revenue
- Trading fees: 0.1-0.3% per trade
- Spread on RFQ: built into quotes
- Premium features: faster settlement, higher limits

### Treasury Agent Revenue
- Spread on swaps: 0.3-1%
- Management fees: 1% AUM/year
- Performance fees: 10% of gains

### Routing Agent Revenue
- Routing fees: 0.1-0.5% per payment
- Premium routing: faster, more reliable

---

## Trust Model

### Mint Trust
- Users trust mints to redeem proofs
- Diversify across mints
- Monitor mint reputation

### Counterparty Trust
- Reputation system
- Collateral requirements
- Escrow for large trades

### Exchange Trust (if centralized)
- Non-custodial: never holds user funds long
- Transparent operation
- Open-source matching engine
- Migrate to decentralized over time

---

## Market Abuse & Spam Prevention

Nostr order books will be spammed. Plan for it.

### Attack Vectors

| Attack | Impact | Mitigation |
|--------|--------|------------|
| Order spam | Fills relays, drowns real orders | Rate limits, PoW, paid relays |
| Fake quotes | Wastes taker time, distorts market | Reputation gating |
| Quote manipulation | Taker picks "best" quote that never settles | Settlement rate tracking |
| Wash trading | Fake volume for reputation farming | Detect self-trades, require collateral |
| Front-running | Maker sees order, trades ahead | Encrypted order commitment |

### Mitigation Stack

```rust
pub struct AntiAbusePolicy {
    /// Require NIP-13 proof-of-work on orders
    pub min_pow_difficulty: u8,

    /// Minimum reputation score to post orders
    pub min_reputation_to_post: f32,

    /// Minimum reputation to be visible in orderbook
    pub min_reputation_visible: f32,

    /// Maximum orders per pubkey per hour
    pub order_rate_limit: u32,

    /// Require collateral for orders above threshold
    pub collateral_threshold: Amount,

    /// Use paid/authenticated relays only
    pub require_paid_relays: bool,
}

impl Default for AntiAbusePolicy {
    fn default() -> Self {
        Self {
            min_pow_difficulty: 16,           // ~65k hashes
            min_reputation_to_post: 0.0,      // Anyone can try
            min_reputation_visible: 0.5,      // 50%+ success to show
            order_rate_limit: 100,            // 100 orders/hour
            collateral_threshold: Amount::from_sats(1_000_000), // 1M sats
            require_paid_relays: false,       // Start permissive
        }
    }
}
```

### Relay Selection

Not all relays should carry exchange events:

```rust
pub struct ExchangeRelayPolicy {
    /// Relays that accept exchange events
    pub write_relays: Vec<Url>,

    /// Relays to read from (may be different)
    pub read_relays: Vec<Url>,

    /// Require relay to enforce PoW
    pub require_pow_enforcement: bool,

    /// Prefer relays with spam filtering
    pub prefer_filtered: bool,
}
```

### Order Expiration

Prevent stale order accumulation:

```rust
/// Orders MUST have expiry
pub struct OrderExpiry {
    /// Maximum order lifetime
    pub max_ttl: Duration,

    /// Default if not specified
    pub default_ttl: Duration,

    /// Minimum to prevent flash orders
    pub min_ttl: Duration,
}

impl Default for OrderExpiry {
    fn default() -> Self {
        Self {
            max_ttl: Duration::from_secs(86400),    // 24 hours
            default_ttl: Duration::from_secs(3600), // 1 hour
            min_ttl: Duration::from_secs(60),       // 1 minute
        }
    }
}
```

### Cancellation Rules

```
- Orders can be cancelled by publishing cancellation event
- Cancellation MUST reference original order ID
- If order is already matched, cancellation fails
- Reputation penalty for excessive cancellations
```

---

## Implementation Phases

### Phase 1: RFQ Market (MVP)
```
□ RFQ broadcast (Nostr event)
□ Quote response (Nostr event)
□ Manual settlement (both parties have proofs, swap OOB)
□ Basic reputation (trade count, success rate)
```

Simple. Gets the market started. Settlement is manual/trusted.

### Phase 2: Atomic Settlement
```
□ Cashu atomic swap protocol
□ Escrow service (we run it initially)
□ Settlement guarantees
□ Dispute resolution
```

Makes trading trustless.

### Phase 3: Order Book
```
□ Limit orders
□ Matching engine
□ Partial fills
□ Order management
```

Better price discovery, more volume.

### Phase 4: Treasury Services
```
□ Treasury-as-a-Service API
□ Payment routing
□ Auto-conversion policies
□ Managed accounts
```

Higher-level services built on the exchange.

### Phase 5: Advanced Products
```
□ Forward contracts
□ Lending
□ Options (maybe)
□ Multi-currency baskets
```

Full financial infrastructure.

---

## Nostr Event Kinds (Proposed)

| Kind | Purpose |
|------|---------|
| 38950 | Exchange Order (limit order) |
| 38951 | Order Cancel |
| 38952 | RFQ Request |
| 38953 | RFQ Quote |
| 38954 | Trade Execution |
| 38955 | Trade Attestation (reputation) |
| 38956 | Treasury Service Announcement |
| 38957 | Payment Routing Request |

---

## Example Flows

### Flow 1: Simple Swap via RFQ

```
1. Agent A broadcasts RFQ: "Want to buy 100k sats"
   kind: 38952, tags: [["pair", "BTC/USD"], ["side", "buy"], ["amount", "100000"]]

2. Treasury Agent B sees RFQ, responds with quote
   kind: 38953, tags: [["e", "<rfq_id>"], ["price", "0.00099"], ["expires", "60"]]

3. Agent A accepts quote
   DM to B: "Accept quote <quote_id>"

4. Settlement
   - B creates mint quote at USD mint for 9900 cents
   - A pays the invoice
   - B sends 100k sat proofs to A
   - A receives proofs

5. Both publish attestations
   kind: 38955, confirming successful trade
```

### Flow 2: Payment Routing

```
1. Agent A has USD balance, needs to pay BTC invoice

2. Agent A calls routing service:
   POST /route { "invoice": "lnbc50u...", "from": "USD" }

3. Router finds best path:
   - Quote from Treasury Agent: 50k sats = $51 USD
   - Fee: $1 (2%)
   - Total: $51

4. Agent A approves

5. Router executes:
   - Takes $51 USD proofs from A
   - Swaps to 50k sats via Treasury Agent
   - Pays Lightning invoice
   - Returns preimage to A
```

### Flow 3: Treasury-as-a-Service

```
1. Agent A deposits 1M sats with Treasury Service
   - Sends proofs to service
   - Sets policy: "Keep 50% USD, 50% BTC"

2. Treasury Service manages:
   - Auto-rebalances when ratio drifts
   - Finds best swap rates
   - Handles mint diversification

3. Agent A needs to pay:
   - Calls treasury.pay(invoice)
   - Treasury handles routing, conversion
   - A just sees: "Paid ✓"

4. Monthly report:
   - Starting: 1M sats
   - Ending: 1.05M sats equivalent
   - Fees paid: 10k sats
   - FX P&L: +60k sats
```

---

## Open Questions

1. **Who runs the escrow initially?**
   - Us? Trusted third party? Federated?

2. **How to bootstrap liquidity?**
   - Incentivize early treasury agents?
   - Provide initial liquidity ourselves?

3. **Regulatory considerations?**
   - Is this a money transmitter situation?
   - How to structure to minimize risk?

4. **Dispute resolution?**
   - Who arbitrates?
   - How to handle edge cases?

5. **Cross-mint settlement?**
   - What if parties use different mints?
   - Need intermediate step?

---

## The Pitch

**Neobank** = Agent holds BTC and USD
**Exchange** = Agents trade BTC and USD with each other

Together: Full financial infrastructure where agents don't just spend money—they provide financial services, earn from treasury operations, and participate in markets.

**This creates a new category of agent: the Treasury Agent**

Treasury Agents are profitable from day one. They hold capital, quote markets, route payments, and earn fees. They're the financial infrastructure layer for all other agents.

Every coding agent, every compute provider, every skill seller—they all need treasury services. Treasury Agents serve them.

---

## Relationship to Existing Work

| Component | Exists | Exchange Adds |
|-----------|--------|---------------|
| Agent identity | ✅ FROST/Nostr | Reputation system |
| BTC payments | ✅ Spark | Cross-currency routing |
| Budget enforcement | ✅ BudgetTracker | Multi-currency budgets |
| Marketplace | ✅ NIP-90 | Financial services as skills |
| eCash | 🔵 NIP-60 types | Full wallet + atomic swaps |

Exchange builds on everything we have. It's the financial layer that makes agent-to-agent commerce liquid.
