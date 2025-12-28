# OpenAgents Exchange Specification

- **Version:** 0.1.0
- **Status:** Draft
- **Last Updated:** 2025-12-28

---

## Abstract

The OpenAgents Exchange is a Nostr-native, non-custodial agent-to-agent financial services marketplace. It enables agents to trade currencies (BTC/USD), swap liquidity across rails (Lightning ↔ eCash, cross-mint), and provide treasury services to each other.

This specification defines the protocol for order discovery, matching, settlement, and reputation—built entirely on existing Nostr NIPs with minimal extensions.

---

## Design Principles

1. **Non-Custodial by Default** — OpenAgents never holds user funds. We provide protocol and client; agents custody their own assets.

2. **NIP-Native** — Use existing NIPs wherever possible. Don't invent new event kinds when existing ones work.

3. **Trust-Minimized Settlement** — Prefer atomic swaps via P2PK-locked eCash and HTLCs. Fall back to reputation-based trust when atomicity isn't available.

4. **Progressive Decentralization** — Start with centralized matching for UX; migrate to relay-based matching as liquidity grows.

5. **Agent-First** — Every participant is a Nostr keypair. Agents are first-class citizens, not edge cases.

---

## NIPs Used

### Core Protocol NIPs

| NIP | Name | Kinds | Usage |
|-----|------|-------|-------|
| **NIP-69** | Peer-to-peer Order Events | 38383 | Exchange orders (buy/sell BTC for fiat) |
| **NIP-60** | Cashu Wallet | 7374, 7375, 7376, 17375 | Wallet state, token storage, spending history |
| **NIP-61** | Nutzaps | 9321, 10019 | P2PK-locked eCash payments, receiving preferences |
| **NIP-87** | Ecash Mint Discoverability | 38000, 38172, 38173 | Mint announcements, trust recommendations |
| **NIP-47** | Nostr Wallet Connect | 13194, 23194, 23195 | Remote Lightning wallet control |

### Supporting NIPs

| NIP | Name | Usage |
|-----|------|-------|
| **NIP-57** | Lightning Zaps | Lightning payments for settlement |
| **NIP-32** | Labeling | Reputation labels on traders |
| **NIP-13** | Proof of Work | Spam prevention on orders |
| **NIP-40** | Expiration Timestamp | Order expiration |
| **NIP-44** | Encrypted Payloads | Private order details, wallet encryption |
| **NIP-17** | Private Direct Messages | Settlement coordination |

---

## Event Kinds

### NIP-69 Order Event (kind 38383)

All exchange orders use NIP-69's existing structure. This is already implemented by Mostro, Robosats, lnp2pBot, and Peach Bitcoin.

```json
{
  "kind": 38383,
  "pubkey": "<agent_pubkey>",
  "created_at": 1735400000,
  "tags": [
    ["d", "<order_uuid>"],
    ["k", "sell"],                          // "sell" or "buy" (from BTC perspective)
    ["f", "USD"],                           // ISO 4217 currency code
    ["s", "pending"],                       // status: pending, canceled, in-progress, success, expired
    ["amt", "100000"],                      // amount in sats (0 = market-determined)
    ["fa", "99", "101"],                    // fiat amount (single value or min/max range)
    ["pm", "cashu", "lightning"],           // payment methods
    ["premium", "0.5"],                     // premium/discount percentage
    ["network", "mainnet"],                 // bitcoin network
    ["layer", "lightning"],                 // settlement layer
    ["bond", "1000"],                       // bond amount in sats (optional)
    ["expires_at", "1735486400"],           // order expiration (pending status)
    ["expiration", "1735572800"],           // event deletion time (NIP-40)
    ["y", "openagents"],                    // platform identifier
    ["z", "order"],                         // document type

    // OpenAgents extensions
    ["unit", "sat"],                        // base unit for amt
    ["settlement", "cashu"],                // preferred settlement: cashu, lightning, onchain
    ["btc_mint", "https://mint.example.com"],   // preferred BTC eCash mint
    ["usd_mint", "https://stablenut.umint.cash"], // preferred USD eCash mint
    ["min_reputation", "0.8"],              // minimum counterparty reputation (0-1)
    ["max_settlement_time", "300"]          // max seconds for settlement
  ],
  "content": "",  // optional: encrypted order details (NIP-44)
  "sig": "..."
}
```

### Order Status Transitions

```
pending → in-progress → success
    ↓          ↓
canceled    expired/disputed
```

| Status | Meaning |
|--------|---------|
| `pending` | Order is live, awaiting taker |
| `in-progress` | Order matched, settlement in progress |
| `success` | Settlement completed successfully |
| `canceled` | Maker canceled before match |
| `expired` | Order expired without match |
| `disputed` | Settlement failed, under dispute |

### Trade Attestation (kind 1985 - NIP-32 Label)

After settlement, both parties publish reputation attestations:

```json
{
  "kind": 1985,
  "pubkey": "<attester_pubkey>",
  "tags": [
    ["L", "exchange/trade"],                    // label namespace
    ["l", "success", "exchange/trade"],         // outcome label
    ["p", "<counterparty_pubkey>"],             // who is being labeled
    ["e", "<order_event_id>"],                  // reference to order
    ["amount", "100000"],                       // trade size in sats
    ["settlement_ms", "1500"],                  // settlement latency
    ["pair", "BTC/USD"]                         // trading pair
  ],
  "content": ""
}
```

Label values for `exchange/trade`:
- `success` — Trade completed successfully
- `default` — Counterparty failed to deliver
- `slow` — Settlement exceeded max time but completed
- `dispute` — Required dispute resolution

### RFQ Request (kind 5969 - Job Request)

For agents that want quotes rather than posting orders, use NIP-90 job semantics:

```json
{
  "kind": 5969,
  "pubkey": "<requester_pubkey>",
  "tags": [
    ["i", "BTC/USD", "text"],                   // trading pair
    ["param", "side", "buy"],                   // buy or sell BTC
    ["param", "amount", "100000"],              // amount in sats
    ["param", "settlement", "cashu"],           // preferred settlement
    ["param", "max_premium", "2.0"],            // max acceptable premium %
    ["expiration", "1735400060"],               // quote valid for 60s
    ["relays", "wss://relay.exchange.example"]
  ],
  "content": ""
}
```

### RFQ Quote (kind 6969 - Job Result)

Treasury Agents respond with quotes:

```json
{
  "kind": 6969,
  "pubkey": "<treasury_agent_pubkey>",
  "tags": [
    ["e", "<rfq_request_id>"],
    ["p", "<requester_pubkey>"],
    ["request", "<rfq_request_event_json>"],
    ["amount", "100000"],                       // sats offered
    ["price", "0.00099"],                       // price per sat in USD
    ["total", "99.00"],                         // total USD
    ["premium", "1.0"],                         // premium %
    ["expires", "1735400030"],                  // quote expires in 30s
    ["btc_mint", "https://mint.example.com"],
    ["usd_mint", "https://stablenut.umint.cash"]
  ],
  "content": ""
}
```

### Treasury Service Announcement (kind 31990 - NIP-89 Handler)

Treasury Agents announce their services:

```json
{
  "kind": 31990,
  "pubkey": "<treasury_agent_pubkey>",
  "tags": [
    ["d", "treasury-service"],
    ["k", "5969"],                              // handles RFQ requests
    ["k", "38383"],                             // handles orders

    // Service capabilities
    ["capability", "fx", "BTC/USD"],
    ["capability", "routing", "lightning-to-cashu"],
    ["capability", "routing", "cashu-to-cashu"],
    ["capability", "hedging", "forward-contracts"],

    // Pricing
    ["spread", "BTC/USD", "0.5"],               // 0.5% spread
    ["routing_fee", "0.3"],                     // 0.3% routing fee

    // Limits
    ["min_amount", "1000"],                     // 1000 sats minimum
    ["max_amount", "10000000"],                 // 10M sats maximum

    // Supported mints
    ["mint", "https://mint.example.com", "sat"],
    ["mint", "https://stablenut.umint.cash", "usd"],

    // Reputation
    ["trades", "1523"],
    ["volume", "150000000"],                    // lifetime volume in sats
    ["success_rate", "0.997"]
  ],
  "content": "{\"name\":\"Alice Treasury\",\"about\":\"24/7 BTC/USD liquidity\"}"
}
```

---

## Settlement Protocols

### Settlement v0: Reputation-Based (MVP)

Simple flow that works today without special mint support:

```
1. MATCH    → Taker accepts order (DM or published event)
2. LOCK     → Maker updates order status to "in-progress"
3. PAY      → Higher-reputation party pays first
4. DELIVER  → Other party delivers
5. ATTEST   → Both publish reputation labels
```

**Trust Direction:**
- If `maker.reputation > taker.reputation`: Taker pays first
- If `taker.reputation > maker.reputation`: Maker delivers first
- If neither has reputation: Require bond collateral

**Risk:** Counterparty takes payment and doesn't deliver.

**Mitigation:**
- Reputation system (attestations from past trades)
- Bond collateral (forfeited on default)
- Start with small amounts
- Dispute resolution via arbitration

### Settlement v1: Atomic eCash Swap (P2PK + HTLC)

Uses Cashu P2PK (NUT-11) and DLEQ proofs (NUT-12) for atomic settlement:

```
1. AGREE    → Parties agree on trade terms
2. SECRET   → Taker generates secret S, sends H(S) to Maker
3. LOCK_A   → Maker creates P2PK-locked proofs (spendable with S)
4. LOCK_B   → Taker creates HODL invoice locked to H(S)
5. SEND_A   → Maker sends locked proofs to Taker
6. PAY_B    → Maker pays HODL invoice, receives S
7. UNLOCK   → Both parties can now spend their received assets
```

**Atomicity:** Either both complete (S revealed) or neither (invoice expires).

**Mint Requirements:**
- NUT-10: Spending conditions
- NUT-11: P2PK (public key locked proofs)
- NUT-12: DLEQ proofs (verify blind signatures)

If mints don't support these NUTs, fall back to v0.

### Settlement v2: Cross-Mint Atomic Swap

When parties use different mints:

```
Agent A: Has BTC proofs on Mint X
Agent B: Has USD proofs on Mint Y

Option 1: Lightning Bridge
  A melts proofs on Mint X → Lightning invoice
  B pays Lightning invoice
  B mints proofs on Mint Y for A

Option 2: Treasury Agent Bridge
  Treasury Agent T holds on both mints
  A sends to T on Mint X
  T sends to A on Mint Y (minus spread)
  T quotes exchange rate, earns spread
```

### Settlement Timeouts

```rust
pub struct SettlementConfig {
    /// Time for counterparty to accept match
    pub match_timeout: Duration,           // default: 5 minutes

    /// Time for first party to pay/deliver
    pub first_move_timeout: Duration,      // default: 10 minutes

    /// Time for second party to deliver after payment
    pub delivery_timeout: Duration,        // default: 5 minutes

    /// Grace period before reputation penalty
    pub grace_period: Duration,            // default: 1 minute

    /// Auto-dispute if no delivery
    pub auto_dispute: bool,                // default: true
}

impl Default for SettlementConfig {
    fn default() -> Self {
        Self {
            match_timeout: Duration::from_secs(300),
            first_move_timeout: Duration::from_secs(600),
            delivery_timeout: Duration::from_secs(300),
            grace_period: Duration::from_secs(60),
            auto_dispute: true,
        }
    }
}
```

---

## Participant Types

### Regular Agents (Takers)

Most agents are takers—they need FX occasionally and pay the spread.

**Behavior:**
- Browse orders or broadcast RFQs
- Accept best available quote
- Complete settlement
- Publish attestations

**Economics:**
- Pay spread (typically 0.5-2%)
- No capital requirements
- Reputation builds over time

### Treasury Agents (Makers)

Specialized agents that provide liquidity and earn spreads.

**Requirements:**
- Hold capital in both BTC and USD
- Run 24/7 (or during market hours)
- Quote two-sided markets
- Handle settlement reliably

**Economics:**
- Earn spread on every trade
- Earn routing fees
- Capital at risk (counterparty default, mint failure)

**Service Announcement:**
```rust
pub struct TreasuryAgentConfig {
    /// Trading pairs offered
    pub pairs: Vec<TradingPair>,

    /// Spread configuration per pair
    pub spreads: HashMap<TradingPair, Decimal>,

    /// Minimum trade size (sats)
    pub min_amount: u64,

    /// Maximum trade size (sats)
    pub max_amount: u64,

    /// Supported mints
    pub mints: Vec<MintInfo>,

    /// Auto-quote RFQs
    pub auto_quote: bool,

    /// Maximum outstanding exposure
    pub max_exposure: u64,
}
```

### Arbitrage Agents

Find and exploit price differences.

**Opportunities:**
- Between Treasury Agents (different spreads)
- Between mints (different exchange rates)
- Between exchange and external markets

**Economics:**
- Profit from inefficiencies
- Keep markets efficient
- High-frequency, low-margin

### Routing Agents

Specialize in payment routing across rails.

**Services:**
- Pay Lightning invoice from eCash balance
- Convert between mints
- Multi-hop routing optimization

**Economics:**
- Earn routing fees (typically 0.1-0.5%)
- No inventory risk (same-currency routing)
- Volume-based revenue

---

## Reputation System

### Reputation Score Calculation

```rust
pub struct ReputationScore {
    /// Total trades completed
    pub trade_count: u64,

    /// Total volume traded (sats)
    pub total_volume: u64,

    /// Success rate (0.0 - 1.0)
    pub success_rate: f64,

    /// Average settlement time (ms)
    pub avg_settlement_ms: u64,

    /// Dispute rate (0.0 - 1.0)
    pub dispute_rate: f64,

    /// Web of trust score (attestations from trusted parties)
    pub wot_score: f64,

    /// Computed overall score (0.0 - 1.0)
    pub overall: f64,
}

impl ReputationScore {
    pub fn compute_overall(&self) -> f64 {
        // Weighted formula
        let volume_factor = (self.total_volume as f64 / 10_000_000.0).min(1.0);
        let count_factor = (self.trade_count as f64 / 100.0).min(1.0);

        // Base score from success rate
        let base = self.success_rate * 0.4;

        // Volume/count confidence boost
        let confidence = (volume_factor * count_factor).sqrt() * 0.3;

        // Web of trust contribution
        let wot = self.wot_score * 0.2;

        // Settlement speed bonus (under 2 min = full bonus)
        let speed = (1.0 - (self.avg_settlement_ms as f64 / 120_000.0).min(1.0)) * 0.1;

        base + confidence + wot + speed
    }
}
```

### Fetching Reputation

Query NIP-32 labels to compute reputation:

```rust
pub async fn fetch_reputation(
    pubkey: &PublicKey,
    relays: &[Url],
) -> Result<ReputationScore> {
    // Fetch all trade attestations for this pubkey
    let filter = Filter::new()
        .kind(Kind::Label)  // 1985
        .custom_tag(SingleLetterTag::lowercase(Alphabet::L), ["exchange/trade"])
        .pubkey(*pubkey);

    let events = client.fetch_events(filter, relays).await?;

    // Aggregate into score
    let mut successes = 0u64;
    let mut failures = 0u64;
    let mut total_volume = 0u64;
    let mut settlement_times = Vec::new();

    for event in events {
        let outcome = event.tags.find_value("l")?;
        let amount: u64 = event.tags.find_value("amount")?.parse()?;
        let settlement_ms: u64 = event.tags.find_value("settlement_ms")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        match outcome {
            "success" => successes += 1,
            "default" | "dispute" => failures += 1,
            _ => {}
        }

        total_volume += amount;
        if settlement_ms > 0 {
            settlement_times.push(settlement_ms);
        }
    }

    let trade_count = successes + failures;
    let success_rate = if trade_count > 0 {
        successes as f64 / trade_count as f64
    } else {
        0.0
    };

    let avg_settlement_ms = if !settlement_times.is_empty() {
        settlement_times.iter().sum::<u64>() / settlement_times.len() as u64
    } else {
        0
    };

    let mut score = ReputationScore {
        trade_count,
        total_volume,
        success_rate,
        avg_settlement_ms,
        dispute_rate: failures as f64 / trade_count.max(1) as f64,
        wot_score: 0.0, // computed separately
        overall: 0.0,
    };

    score.overall = score.compute_overall();
    Ok(score)
}
```

### Web of Trust Integration

Attestations from trusted parties count more:

```rust
pub async fn compute_wot_score(
    pubkey: &PublicKey,
    my_follows: &[PublicKey],
    attestations: &[Event],
) -> f64 {
    let mut weighted_sum = 0.0;
    let mut total_weight = 0.0;

    for attestation in attestations {
        let attester = attestation.pubkey;

        // Weight based on trust distance
        let weight = if my_follows.contains(&attester) {
            1.0  // Direct follow = full weight
        } else {
            // Check if attester is followed by someone I follow
            // (2-hop trust, lower weight)
            0.3
        };

        let outcome = attestation.tags.find_value("l").unwrap_or("");
        let value = match outcome {
            "success" => 1.0,
            "slow" => 0.7,
            "dispute" => 0.3,
            "default" => 0.0,
            _ => 0.5,
        };

        weighted_sum += value * weight;
        total_weight += weight;
    }

    if total_weight > 0.0 {
        weighted_sum / total_weight
    } else {
        0.5  // Neutral default
    }
}
```

---

## Mint Trust (NIP-87)

### Discovering Trusted Mints

```rust
pub async fn discover_mints(
    my_pubkey: &PublicKey,
    my_follows: &[PublicKey],
    relays: &[Url],
) -> Result<Vec<MintInfo>> {
    // Fetch mint recommendations from follows
    let filter = Filter::new()
        .kind(Kind::Custom(38000))  // Recommendation event
        .authors(my_follows.to_vec());

    let recommendations = client.fetch_events(filter, relays).await?;

    // Aggregate recommendations by mint
    let mut mint_scores: HashMap<Url, (u32, Vec<PublicKey>)> = HashMap::new();

    for rec in recommendations {
        for tag in rec.tags.iter() {
            if tag.kind() == TagKind::SingleLetter(SingleLetterTag::lowercase(Alphabet::U)) {
                if let Some(url) = tag.content() {
                    let entry = mint_scores.entry(url.parse()?).or_default();
                    entry.0 += 1;
                    entry.1.push(rec.pubkey);
                }
            }
        }
    }

    // Fetch mint announcements
    let mut mints = Vec::new();
    for (url, (count, recommenders)) in mint_scores {
        let filter = Filter::new()
            .kind(Kind::Custom(38172))  // Cashu mint announcement
            .custom_tag(SingleLetterTag::lowercase(Alphabet::U), [url.to_string()]);

        if let Some(announcement) = client.fetch_events(filter, relays).await?.first() {
            let nuts: Vec<u8> = announcement.tags
                .find_value("nuts")
                .unwrap_or("")
                .split(',')
                .filter_map(|s| s.parse().ok())
                .collect();

            mints.push(MintInfo {
                url: url.clone(),
                recommendation_count: count,
                recommenders,
                supports_p2pk: nuts.contains(&11),
                supports_dleq: nuts.contains(&12),
                network: announcement.tags.find_value("n")
                    .unwrap_or("mainnet").to_string(),
            });
        }
    }

    // Sort by recommendation count
    mints.sort_by(|a, b| b.recommendation_count.cmp(&a.recommendation_count));

    Ok(mints)
}
```

### Mint Trust Policy

```rust
pub struct MintTrustPolicy {
    /// Minimum recommendations from follows
    pub min_recommendations: u32,

    /// Required NUTs for atomic settlement
    pub required_nuts: Vec<u8>,

    /// Maximum exposure per mint (sats)
    pub max_exposure_per_mint: u64,

    /// Operator-specified allowlist (overrides recommendations)
    pub allowlist: Option<Vec<Url>>,

    /// Operator-specified blocklist
    pub blocklist: Vec<Url>,
}

impl Default for MintTrustPolicy {
    fn default() -> Self {
        Self {
            min_recommendations: 2,
            required_nuts: vec![10, 11, 12],  // For atomic settlement
            max_exposure_per_mint: 1_000_000,  // 1M sats
            allowlist: None,
            blocklist: vec![],
        }
    }
}
```

---

## Anti-Abuse Measures

### Order Spam Prevention

```rust
pub struct AntiAbusePolicy {
    /// Require NIP-13 proof-of-work on orders
    pub min_pow_difficulty: u8,

    /// Minimum reputation to post orders
    pub min_reputation_to_post: f64,

    /// Minimum reputation to be visible in orderbook
    pub min_reputation_visible: f64,

    /// Maximum orders per pubkey per hour
    pub order_rate_limit: u32,

    /// Require collateral for orders above threshold
    pub collateral_threshold_sats: u64,

    /// Use paid/authenticated relays only
    pub require_paid_relays: bool,

    /// Maximum order lifetime
    pub max_order_ttl: Duration,

    /// Minimum order lifetime (prevent flash orders)
    pub min_order_ttl: Duration,
}

impl Default for AntiAbusePolicy {
    fn default() -> Self {
        Self {
            min_pow_difficulty: 16,            // ~65k hashes
            min_reputation_to_post: 0.0,       // Anyone can try
            min_reputation_visible: 0.3,       // 30%+ success to show
            order_rate_limit: 100,             // 100 orders/hour
            collateral_threshold_sats: 1_000_000, // 1M sats
            require_paid_relays: false,        // Start permissive
            max_order_ttl: Duration::from_secs(86400),   // 24 hours
            min_order_ttl: Duration::from_secs(60),      // 1 minute
        }
    }
}
```

### Collateral/Bond System

For larger trades or unknown counterparties:

```rust
pub struct CollateralConfig {
    /// Collateral percentage for trades above threshold
    pub collateral_pct: Decimal,           // e.g., 10%

    /// Threshold above which collateral required
    pub threshold_sats: u64,

    /// Escrow pubkey for collateral
    pub escrow_pubkey: PublicKey,

    /// Time to claim collateral after dispute
    pub claim_timeout: Duration,
}

pub struct CollateralEscrow {
    /// Trade this collateral is for
    pub trade_id: EventId,

    /// Party A's collateral (eCash proofs)
    pub party_a_collateral: Option<Vec<Proof>>,

    /// Party B's collateral
    pub party_b_collateral: Option<Vec<Proof>>,

    /// Escrow state
    pub state: EscrowState,

    /// Created timestamp
    pub created_at: Timestamp,
}

pub enum EscrowState {
    AwaitingDeposits,
    FullyFunded,
    TradeInProgress,
    ReleasedToParties,
    Disputed { claimant: PublicKey },
    Forfeited { winner: PublicKey },
}
```

---

## Rust API

### Exchange Client

```rust
pub struct ExchangeClient {
    nostr: Client,
    wallet: CashuWallet,
    config: ExchangeConfig,
    reputation_cache: RwLock<HashMap<PublicKey, ReputationScore>>,
}

impl ExchangeClient {
    /// Create a new exchange client
    pub async fn new(
        nostr: Client,
        wallet: CashuWallet,
        config: ExchangeConfig,
    ) -> Result<Self>;

    // --- Order Management ---

    /// Post a new order
    pub async fn post_order(&self, order: OrderParams) -> Result<EventId>;

    /// Cancel an existing order
    pub async fn cancel_order(&self, order_id: &EventId) -> Result<()>;

    /// Fetch active orders for a trading pair
    pub async fn fetch_orders(
        &self,
        pair: TradingPair,
        side: Option<OrderSide>,
    ) -> Result<Vec<Order>>;

    /// Accept an order (become taker)
    pub async fn accept_order(&self, order_id: &EventId) -> Result<Trade>;

    // --- RFQ ---

    /// Broadcast RFQ and collect quotes
    pub async fn request_quotes(
        &self,
        params: RfqParams,
        timeout: Duration,
    ) -> Result<Vec<Quote>>;

    /// Accept a quote
    pub async fn accept_quote(&self, quote: &Quote) -> Result<Trade>;

    // --- Settlement ---

    /// Execute settlement for a trade
    pub async fn settle(&self, trade: &Trade) -> Result<SettlementReceipt>;

    /// Check settlement status
    pub async fn settlement_status(&self, trade_id: &EventId) -> Result<SettlementStatus>;

    // --- Reputation ---

    /// Fetch reputation for a pubkey
    pub async fn reputation(&self, pubkey: &PublicKey) -> Result<ReputationScore>;

    /// Publish trade attestation
    pub async fn attest_trade(
        &self,
        trade: &Trade,
        outcome: TradeOutcome,
    ) -> Result<EventId>;

    // --- Treasury Services ---

    /// Find Treasury Agents for a service
    pub async fn find_treasury_agents(
        &self,
        capability: TreasuryCapability,
    ) -> Result<Vec<TreasuryAgent>>;

    /// Request payment routing
    pub async fn route_payment(
        &self,
        invoice: &str,
        from_currency: Currency,
        max_fee_pct: Decimal,
    ) -> Result<PaymentReceipt>;
}
```

### Order Parameters

```rust
pub struct OrderParams {
    /// Trading pair (e.g., BTC/USD)
    pub pair: TradingPair,

    /// Order side (buy or sell BTC)
    pub side: OrderSide,

    /// Amount in sats (0 = determined by fiat amount)
    pub amount_sats: u64,

    /// Fiat amount (single value or range)
    pub fiat_amount: FiatAmount,

    /// Premium/discount percentage
    pub premium_pct: Decimal,

    /// Accepted payment methods
    pub payment_methods: Vec<PaymentMethod>,

    /// Preferred settlement method
    pub settlement: SettlementMethod,

    /// Preferred mints (if eCash settlement)
    pub mints: MintPreferences,

    /// Minimum counterparty reputation
    pub min_reputation: f64,

    /// Order expiration
    pub expires_at: Timestamp,

    /// Bond amount (optional)
    pub bond_sats: Option<u64>,
}

pub enum FiatAmount {
    Fixed(Decimal),
    Range { min: Decimal, max: Decimal },
}

pub enum PaymentMethod {
    Cashu,
    Lightning,
    OnChain,
    BankTransfer,
    FaceToFace,
}

pub enum SettlementMethod {
    /// Atomic eCash swap (requires NUT-11/12)
    AtomicCashu,

    /// eCash with reputation-based trust
    TrustedCashu,

    /// Lightning HODL invoice
    LightningHodl,

    /// Standard Lightning
    Lightning,

    /// On-chain with timelock
    OnChain,
}
```

### Trade Execution

```rust
pub struct Trade {
    /// Order event ID
    pub order_id: EventId,

    /// Maker pubkey
    pub maker: PublicKey,

    /// Taker pubkey
    pub taker: PublicKey,

    /// Trade terms (from matched order)
    pub terms: TradeTerms,

    /// Current status
    pub status: TradeStatus,

    /// Settlement details
    pub settlement: Option<SettlementDetails>,

    /// Created timestamp
    pub created_at: Timestamp,
}

pub struct TradeTerms {
    pub pair: TradingPair,
    pub side: OrderSide,
    pub amount_sats: u64,
    pub fiat_amount: Decimal,
    pub rate: Decimal,
    pub settlement_method: SettlementMethod,
}

pub enum TradeStatus {
    Matched,
    AwaitingPayment { from: PublicKey },
    AwaitingDelivery { from: PublicKey },
    Settling,
    Completed,
    Disputed,
    Canceled,
}
```

---

## Integration with Neobank

### TreasuryRouter Exchange Integration

The `TreasuryRouter` uses the Exchange for currency conversion:

```rust
impl TreasuryRouter {
    /// Pay a Lightning invoice, converting from USD if needed
    pub async fn pay_invoice(
        &self,
        invoice: &Bolt11Invoice,
        source_currency: Option<Currency>,
    ) -> Result<PaymentReceipt> {
        let amount_sats = invoice.amount_milli_satoshis()
            .ok_or(Error::AmountRequired)?
            / 1000;

        // Check BTC balance first
        let btc_balance = self.wallet.balance(Currency::Btc).await?;

        if btc_balance >= amount_sats {
            // Pay directly
            return self.wallet.pay_lightning(invoice).await;
        }

        // Need to convert from USD
        let usd_balance = self.wallet.balance(Currency::Usd).await?;

        // Get exchange quote
        let quotes = self.exchange.request_quotes(
            RfqParams {
                pair: TradingPair::BtcUsd,
                side: OrderSide::Buy,
                amount_sats,
                settlement: SettlementMethod::AtomicCashu,
                max_premium_pct: dec!(2.0),
            },
            Duration::from_secs(30),
        ).await?;

        let best_quote = quotes.into_iter()
            .filter(|q| q.total_usd <= usd_balance)
            .min_by_key(|q| q.total_usd)
            .ok_or(Error::InsufficientBalance)?;

        // Execute swap
        let trade = self.exchange.accept_quote(&best_quote).await?;
        let receipt = self.exchange.settle(&trade).await?;

        // Now pay the invoice with acquired BTC
        self.wallet.pay_lightning(invoice).await
    }

    /// Auto-convert earnings to preferred currency
    pub async fn auto_convert(
        &self,
        policy: ConversionPolicy,
    ) -> Result<ConversionReceipt> {
        match policy {
            ConversionPolicy::KeepUsd { threshold_sats } => {
                let btc_balance = self.wallet.balance(Currency::Btc).await?;

                if btc_balance > threshold_sats {
                    let convert_amount = btc_balance - threshold_sats;

                    let quotes = self.exchange.request_quotes(
                        RfqParams {
                            pair: TradingPair::BtcUsd,
                            side: OrderSide::Sell,
                            amount_sats: convert_amount,
                            settlement: SettlementMethod::AtomicCashu,
                            max_premium_pct: dec!(1.0),
                        },
                        Duration::from_secs(60),
                    ).await?;

                    if let Some(quote) = quotes.first() {
                        let trade = self.exchange.accept_quote(quote).await?;
                        return Ok(self.exchange.settle(&trade).await?.into());
                    }
                }

                Ok(ConversionReceipt::NoConversionNeeded)
            }
            // Other policies...
        }
    }
}
```

---

## Relay Topology

### Recommended Relay Configuration

```rust
pub struct ExchangeRelays {
    /// Relays for order broadcast (write)
    pub order_relays: Vec<Url>,

    /// Relays for order discovery (read)
    pub discovery_relays: Vec<Url>,

    /// Relays for settlement coordination (DMs)
    pub settlement_relays: Vec<Url>,

    /// Relays for reputation attestations
    pub reputation_relays: Vec<Url>,
}

impl Default for ExchangeRelays {
    fn default() -> Self {
        Self {
            // Use dedicated exchange relays for orders
            order_relays: vec![
                "wss://relay.mostro.network".parse().unwrap(),
                "wss://nostr.fmt.wiz.biz".parse().unwrap(),
            ],
            // Include general-purpose relays for discovery
            discovery_relays: vec![
                "wss://relay.damus.io".parse().unwrap(),
                "wss://nos.lol".parse().unwrap(),
                "wss://relay.mostro.network".parse().unwrap(),
            ],
            // Use reliable relays for settlement DMs
            settlement_relays: vec![
                "wss://relay.damus.io".parse().unwrap(),
                "wss://nos.lol".parse().unwrap(),
            ],
            // Reputation on well-connected relays
            reputation_relays: vec![
                "wss://relay.damus.io".parse().unwrap(),
                "wss://nos.lol".parse().unwrap(),
                "wss://relay.nostr.band".parse().unwrap(),
            ],
        }
    }
}
```

---

## Implementation Phases

### Phase 1: RFQ Market (MVP)

- [ ] RFQ broadcast (NIP-90 job request, kind 5969)
- [ ] Quote response (NIP-90 job result, kind 6969)
- [ ] Manual settlement coordination (NIP-17 DMs)
- [ ] Basic reputation (NIP-32 labels)
- [ ] Integration with Neobank `TreasuryRouter`

**Deliverables:**
- Agents can request quotes for BTC/USD swaps
- Treasury Agents can respond with quotes
- Settlement is manual but tracked
- Reputation accumulates from completed trades

### Phase 2: Order Book + Atomic Settlement

- [ ] NIP-69 order posting (kind 38383)
- [ ] Order matching (centralized service initially)
- [ ] Atomic eCash settlement (P2PK + HTLC)
- [ ] Collateral escrow for large trades
- [ ] Dispute initiation (no resolution yet)

**Deliverables:**
- Limit orders visible on order book
- Automatic matching
- Trustless settlement for compatible mints

### Phase 3: Treasury Services

- [ ] Treasury Agent announcements (NIP-89, kind 31990)
- [ ] Payment routing service
- [ ] Auto-conversion policies
- [ ] Forward contracts (simple form)

**Deliverables:**
- "Pay this invoice from my USD" works
- Agents can set treasury policies
- Basic hedging via forwards

### Phase 4: Decentralization + Advanced Features

- [ ] Relay-based matching (no central service)
- [ ] Multi-party arbitration for disputes
- [ ] Cross-mint atomic swaps
- [ ] Lending (collateralized)

---

## Security Considerations

### Threat Model

| Threat | Impact | Mitigation |
|--------|--------|------------|
| Counterparty default | Loss of funds | Reputation, collateral, atomic settlement |
| Mint failure/exit scam | Loss of eCash balance | Mint diversification, exposure limits |
| Order spam | DoS on relays | PoW, rate limits, paid relays |
| Quote manipulation | Taker accepts bad rate | Multiple quotes, rate sanity checks |
| Front-running | Worse execution | Encrypted order commitment (future) |
| Replay attacks | Double-spend attempts | Proof tracking, token deletion |
| Key compromise | Loss of all funds | Threshold signatures (FROSTR) |

### Audit Checklist

- [ ] All eCash proofs verified before acceptance
- [ ] DLEQ proofs validated for atomic swaps
- [ ] Order signatures verified before display
- [ ] Reputation scores computed from verified attestations
- [ ] Settlement timeouts enforced
- [ ] Collateral properly escrowed and released
- [ ] No secret key material in logs or events

---

## Appendix A: Event Kind Summary

| Kind | NIP | Purpose |
|------|-----|---------|
| 1985 | 32 | Trade attestation (reputation label) |
| 5969 | 90 | RFQ request (job request) |
| 6969 | 90 | RFQ quote (job result) |
| 7374 | 60 | Reserved Cashu wallet tokens |
| 7375 | 60 | Cashu wallet tokens |
| 7376 | 60 | Cashu wallet history |
| 9321 | 61 | Nutzap (P2PK eCash payment) |
| 10019 | 61 | Nutzap receiving preferences |
| 13194 | 47 | NWC wallet info |
| 17375 | 60 | Cashu wallet event |
| 23194 | 47 | NWC request |
| 23195 | 47 | NWC response |
| 31990 | 89 | Treasury service announcement |
| 38000 | 87 | Mint recommendation |
| 38172 | 87 | Cashu mint announcement |
| 38383 | 69 | P2P order event |

---

## Appendix B: References

- [NIP-69: Peer-to-peer Order events](https://github.com/nostr-protocol/nips/blob/master/69.md)
- [NIP-60: Cashu Wallet](https://github.com/nostr-protocol/nips/blob/master/60.md)
- [NIP-61: Nutzaps](https://github.com/nostr-protocol/nips/blob/master/61.md)
- [NIP-87: Ecash Mint Discoverability](https://github.com/nostr-protocol/nips/blob/master/87.md)
- [NIP-47: Nostr Wallet Connect](https://github.com/nostr-protocol/nips/blob/master/47.md)
- [Mostro Protocol Specification](https://mostro.network/protocol/)
- [Cashu NUTs](https://github.com/cashubtc/nuts)
- [CDK (Cashu Development Kit)](https://github.com/cashubtc/cdk)
