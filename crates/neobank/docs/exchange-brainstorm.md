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

### Settlement

**Atomic Swap via Cashu**

Both parties have Cashu proofs. Swap atomically:

```
1. Agree on trade (100k sats for $99 USD)
2. Agent A creates: BlindedMessage for 100k sats
3. Agent B creates: BlindedMessage for 9900 cents
4. Exchange (or escrow): Holds both, swaps signatures
5. Agent A gets USD proofs, Agent B gets BTC proofs
```

This requires either:
- Trusted escrow (exchange holds briefly)
- Multi-party computation (complex)
- HTLC-style conditional proofs (if mints support)

**Lightning-Based Settlement**

For cases where one side needs Lightning:

```
1. Agent A wants to sell BTC proofs for Lightning
2. Agent B creates HODL invoice (hash-locked)
3. Agent A reveals preimage upon receiving invoice
4. Agent B gets preimage, claims BTC proofs
5. Atomic: either both happen or neither
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
