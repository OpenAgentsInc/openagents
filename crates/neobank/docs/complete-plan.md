# Plan: Complete Neobank Exchange Implementation

## Goal
Build a production-ready agent-to-agent exchange with real settlement, relay integration, and comprehensive test coverage.

## Current State (Phase 1 MVP ~30% Complete)

### Already Implemented:
- `exchange.rs` - ExchangeClient with mock settlement, NIP-69/32 tag builders
- `wallet.rs` - CashuWallet (balance, mint quotes, melt)
- `exchange_e2e.rs` - 4 basic tests
- NIP-SA integration test - Full agent lifecycle demo
- NIP-69, NIP-60, NIP-61, NIP-87 parsing in nostr/core

### Critical Gaps:
- No real settlement (only mock)
- No relay integration (orders stay in-memory)
- No wallet↔exchange connection
- No RFQ, Treasury Agent, Mint Trust, or Escrow modules

---

## Implementation Plan

### Phase 1: Settlement Infrastructure

#### 1.1 Create `settlement.rs` (~400 LOC)

```rust
// crates/neobank/src/settlement.rs

pub enum SettlementMode {
    Mock,
    ReputationBased { timeout: Duration },
    AtomicP2PK { htlc_timeout: Duration },
}

pub struct SettlementEngine {
    mode: SettlementMode,
    btc_wallet: Option<CashuWallet>,
    usd_wallet: Option<CashuWallet>,
    nostr_client: Option<NostrClient>,
}

impl SettlementEngine {
    // v0: Reputation-based (higher rep pays first)
    pub async fn settle_reputation(&self, trade: &Trade) -> Result<SettlementReceipt>;

    // v1: Atomic P2PK with HTLC
    pub async fn settle_atomic(&self, trade: &Trade) -> Result<SettlementReceipt>;

    // Proof transfer helpers
    async fn send_proofs(&self, to: &str, amount: u64, mint: &Url) -> Result<TokenId>;
    async fn receive_proofs(&self, token: &str) -> Result<Amount>;
    async fn lock_proofs_p2pk(&self, pubkey: &str, amount: u64) -> Result<LockedProof>;
    async fn unlock_proofs(&self, proof: &LockedProof, preimage: &[u8]) -> Result<Amount>;
}
```

**Files:**
- `crates/neobank/src/settlement.rs` (NEW)

#### 1.2 Extend CashuWallet for Proof Transfer (~200 LOC)

Add to `wallet.rs`:
```rust
// Proof transfer methods (for settlement)
pub async fn send_token(&self, amount: u64) -> Result<String>;  // Returns cashu token
pub async fn receive_token(&self, token: &str) -> Result<Amount>;
pub async fn verify_token(&self, token: &str) -> Result<bool>;
pub async fn split_proofs(&self, amount: u64) -> Result<Vec<Proof>>;

// P2PK support (NUT-11)
pub async fn lock_to_pubkey(&self, pubkey: &str, amount: u64) -> Result<LockedProof>;
pub async fn unlock_with_key(&self, proof: &LockedProof) -> Result<Amount>;
```

**Files:**
- `crates/neobank/src/wallet.rs` (MODIFY)

---

### Phase 2: Relay Integration

#### 2.1 Create `relay.rs` (~350 LOC)

```rust
// crates/neobank/src/relay.rs

pub struct ExchangeRelay {
    client: NostrClient,
    relays: Vec<String>,
}

impl ExchangeRelay {
    // Order operations
    pub async fn publish_order(&self, order: &Order, keys: &Keys) -> Result<EventId>;
    pub async fn subscribe_orders(&self, filter: OrderFilter) -> Result<Receiver<Order>>;
    pub async fn update_order_status(&self, order_id: &str, status: OrderStatus) -> Result<()>;

    // Attestation operations
    pub async fn publish_attestation(&self, attest: &TradeAttestation, keys: &Keys) -> Result<EventId>;
    pub async fn fetch_attestations(&self, pubkey: &str) -> Result<Vec<TradeAttestation>>;

    // DM coordination (NIP-17)
    pub async fn send_settlement_dm(&self, to: &str, msg: SettlementMessage) -> Result<()>;
    pub async fn receive_settlement_dms(&self) -> Result<Receiver<SettlementMessage>>;
}
```

**Files:**
- `crates/neobank/src/relay.rs` (NEW)

#### 2.2 Update ExchangeClient for Relay Mode (~150 LOC)

Modify `exchange.rs`:
```rust
pub struct ExchangeClient {
    pubkey: String,
    settlement: SettlementEngine,
    relay: Option<ExchangeRelay>,  // NEW
    // ... existing fields
}

impl ExchangeClient {
    pub fn new_with_relay(pubkey: &str, relay: ExchangeRelay, settlement: SettlementEngine) -> Self;

    // Modified to use relay when available
    pub async fn post_order(&self, params: OrderParams) -> Result<String>;
    pub async fn fetch_orders(&self, filter: Option<OrderFilter>) -> Result<Vec<Order>>;
}
```

**Files:**
- `crates/neobank/src/exchange.rs` (MODIFY)

---

### Phase 3: Reputation System

#### 3.1 Create `reputation.rs` (~300 LOC)

```rust
// crates/neobank/src/reputation.rs

pub struct ReputationService {
    relay: ExchangeRelay,
    cache: Cache<String, ReputationScore>,
}

pub struct ReputationScore {
    pub pubkey: String,
    pub success_rate: f64,
    pub total_trades: u64,
    pub total_volume_sats: u64,
    pub avg_settlement_ms: u64,
    pub dispute_rate: f64,
    pub last_trade: u64,
}

impl ReputationService {
    // Fetch from relays
    pub async fn fetch_reputation(&self, pubkey: &str) -> Result<ReputationScore>;
    pub async fn fetch_attestations(&self, pubkey: &str, limit: usize) -> Result<Vec<TradeAttestation>>;

    // Calculate scores
    pub fn calculate_score(&self, attestations: &[TradeAttestation]) -> ReputationScore;
    pub fn calculate_wot_score(&self, pubkey: &str, follows: &[String]) -> Result<f64>;

    // Decision helpers
    pub fn should_pay_first(&self, my_rep: &ReputationScore, their_rep: &ReputationScore) -> bool;
    pub fn min_reputation_for_amount(&self, amount_sats: u64) -> f64;
}
```

**Files:**
- `crates/neobank/src/reputation.rs` (NEW)

---

### Phase 4: RFQ Market

#### 4.1 Create `rfq.rs` (~350 LOC)

```rust
// crates/neobank/src/rfq.rs

// NIP-90 kinds for RFQ
pub const RFQ_REQUEST_KIND: u16 = 5969;
pub const RFQ_RESPONSE_KIND: u16 = 6969;

pub struct RfqRequest {
    pub id: String,
    pub side: OrderSide,
    pub amount_sats: u64,
    pub currency: String,
    pub max_premium_pct: f64,
    pub expires_at: u64,
}

pub struct RfqQuote {
    pub request_id: String,
    pub provider: String,
    pub rate: f64,
    pub premium_pct: f64,
    pub expires_at: u64,
    pub min_reputation: f64,
}

pub struct RfqMarket {
    relay: ExchangeRelay,
}

impl RfqMarket {
    // Requester side
    pub async fn broadcast_rfq(&self, req: RfqRequest, keys: &Keys) -> Result<EventId>;
    pub async fn collect_quotes(&self, request_id: &str, timeout: Duration) -> Result<Vec<RfqQuote>>;
    pub async fn accept_quote(&self, quote: &RfqQuote) -> Result<Trade>;

    // Provider side (Treasury Agent)
    pub async fn subscribe_rfqs(&self, filter: RfqFilter) -> Result<Receiver<RfqRequest>>;
    pub async fn respond_to_rfq(&self, request_id: &str, quote: RfqQuote, keys: &Keys) -> Result<EventId>;

    // Tag builders
    pub fn build_rfq_tags(&self, req: &RfqRequest) -> Vec<Vec<String>>;
    pub fn build_quote_tags(&self, quote: &RfqQuote) -> Vec<Vec<String>>;
}
```

**Files:**
- `crates/neobank/src/rfq.rs` (NEW)

---

### Phase 5: Treasury Agent Services

#### 5.1 Create `treasury_agent.rs` (~400 LOC)

```rust
// crates/neobank/src/treasury_agent.rs

pub const TREASURY_ANNOUNCEMENT_KIND: u16 = 31990;

pub struct TreasuryAgentConfig {
    pub pubkey: String,
    pub supported_pairs: Vec<TradingPair>,
    pub spread_bps: u16,
    pub min_trade_sats: u64,
    pub max_trade_sats: u64,
    pub auto_convert: bool,
}

pub struct TreasuryAgent {
    config: TreasuryAgentConfig,
    exchange: ExchangeClient,
    btc_wallet: CashuWallet,
    usd_wallet: CashuWallet,
    relay: ExchangeRelay,
}

impl TreasuryAgent {
    // Service announcements (NIP-89)
    pub async fn publish_announcement(&self, keys: &Keys) -> Result<EventId>;

    // Market making
    pub async fn post_liquidity(&self, pair: TradingPair, amount: u64) -> Result<Vec<String>>;
    pub async fn handle_rfq(&self, req: &RfqRequest) -> Result<RfqQuote>;

    // Auto-conversion
    pub async fn convert(&self, from: Currency, to: Currency, amount: u64) -> Result<Amount>;

    // Position management
    pub fn calculate_spread(&self, pair: TradingPair, volume_24h: u64) -> f64;
    pub async fn rebalance(&self) -> Result<()>;
}
```

**Files:**
- `crates/neobank/src/treasury_agent.rs` (NEW)

---

### Phase 6: Mint Trust (NIP-87)

#### 6.1 Create `mint_trust.rs` (~300 LOC)

```rust
// crates/neobank/src/mint_trust.rs

pub struct MintTrustService {
    relay: ExchangeRelay,
    known_mints: HashMap<Url, MintInfo>,
}

pub struct MintInfo {
    pub url: Url,
    pub pubkey: String,
    pub supported_nuts: Vec<u8>,
    pub network: MintNetwork,
    pub trust_score: f64,
    pub recommendations: u64,
    pub last_seen: u64,
}

impl MintTrustService {
    // Discovery (NIP-87 kind 38172)
    pub async fn discover_mints(&self) -> Result<Vec<MintInfo>>;
    pub async fn fetch_recommendations(&self, follows: &[String]) -> Result<Vec<MintRecommendation>>;

    // Health monitoring
    pub async fn check_mint_health(&self, mint_url: &Url) -> Result<MintHealth>;
    pub async fn probe_nut_support(&self, mint_url: &Url) -> Result<Vec<u8>>;

    // Trust scoring
    pub fn calculate_trust(&self, mint: &MintInfo, recommendations: &[MintRecommendation]) -> f64;
    pub fn select_mint(&self, currency: Currency, min_trust: f64) -> Result<Url>;

    // Allowlist/blocklist
    pub fn is_allowed(&self, mint_url: &Url) -> bool;
    pub fn add_to_allowlist(&mut self, mint_url: Url);
    pub fn add_to_blocklist(&mut self, mint_url: Url);
}
```

**Files:**
- `crates/neobank/src/mint_trust.rs` (NEW)

---

### Phase 7: Escrow/Collateral System

#### 7.1 Create `escrow.rs` (~350 LOC)

```rust
// crates/neobank/src/escrow.rs

pub struct EscrowService {
    wallet: CashuWallet,
    relay: ExchangeRelay,
}

pub struct Bond {
    pub id: String,
    pub trader: String,
    pub amount_sats: u64,
    pub locked_proofs: LockedProof,
    pub expires_at: u64,
    pub trade_id: Option<String>,
}

pub struct Escrow {
    pub id: String,
    pub trade_id: String,
    pub maker_bond: Option<Bond>,
    pub taker_bond: Option<Bond>,
    pub status: EscrowStatus,
}

pub enum EscrowStatus {
    Pending,
    Funded,
    Released,
    Disputed,
    Slashed,
}

impl EscrowService {
    // Bond management
    pub async fn create_bond(&self, amount: u64, expires_at: u64) -> Result<Bond>;
    pub async fn lock_bond(&self, bond: &Bond, trade_id: &str) -> Result<()>;
    pub async fn release_bond(&self, bond: &Bond) -> Result<Amount>;
    pub async fn slash_bond(&self, bond: &Bond, to: &str) -> Result<()>;

    // Escrow for trades
    pub async fn create_escrow(&self, trade: &Trade, bond_pct: f64) -> Result<Escrow>;
    pub async fn fund_escrow(&self, escrow: &mut Escrow, side: TradeSide) -> Result<()>;
    pub async fn release_escrow(&self, escrow: &Escrow) -> Result<()>;

    // Dispute handling
    pub async fn initiate_dispute(&self, escrow: &Escrow, reason: &str) -> Result<DisputeId>;
    pub async fn resolve_dispute(&self, dispute_id: &DisputeId, winner: &str) -> Result<()>;
}
```

**Files:**
- `crates/neobank/src/escrow.rs` (NEW)

---

### Phase 8: Comprehensive Tests

#### 8.1 Settlement Tests (`tests/settlement_test.rs`) (~400 LOC)

```rust
// Happy paths
#[tokio::test] async fn test_mock_settlement() { }
#[tokio::test] async fn test_reputation_based_settlement_higher_rep_pays_first() { }
#[tokio::test] async fn test_reputation_based_settlement_equal_rep() { }
#[tokio::test] async fn test_atomic_p2pk_settlement() { }

// Error paths
#[tokio::test] async fn test_settlement_timeout() { }
#[tokio::test] async fn test_settlement_counterparty_disappears() { }
#[tokio::test] async fn test_settlement_invalid_proofs() { }
#[tokio::test] async fn test_settlement_insufficient_balance() { }

// Edge cases
#[tokio::test] async fn test_settlement_exact_amount() { }
#[tokio::test] async fn test_settlement_with_change() { }
#[tokio::test] async fn test_settlement_cross_mint() { }
```

#### 8.2 Relay Integration Tests (`tests/relay_test.rs`) (~300 LOC)

```rust
#[tokio::test] async fn test_publish_order_to_relay() { }
#[tokio::test] async fn test_subscribe_orders_from_relay() { }
#[tokio::test] async fn test_order_sync_between_agents() { }
#[tokio::test] async fn test_attestation_publish_and_fetch() { }
#[tokio::test] async fn test_dm_settlement_coordination() { }
#[tokio::test] async fn test_relay_reconnection() { }
#[tokio::test] async fn test_multiple_relays_redundancy() { }
```

#### 8.3 RFQ Tests (`tests/rfq_test.rs`) (~250 LOC)

```rust
#[tokio::test] async fn test_broadcast_rfq_request() { }
#[tokio::test] async fn test_collect_multiple_quotes() { }
#[tokio::test] async fn test_accept_best_quote() { }
#[tokio::test] async fn test_rfq_expiration() { }
#[tokio::test] async fn test_treasury_agent_responds_to_rfq() { }
```

#### 8.4 Reputation Tests (`tests/reputation_test.rs`) (~250 LOC)

```rust
#[tokio::test] async fn test_fetch_reputation_from_relay() { }
#[tokio::test] async fn test_wot_scoring() { }
#[tokio::test] async fn test_reputation_decay_over_time() { }
#[tokio::test] async fn test_dispute_rate_penalty() { }
#[tokio::test] async fn test_volume_confidence_factor() { }
```

#### 8.5 Escrow Tests (`tests/escrow_test.rs`) (~300 LOC)

```rust
#[tokio::test] async fn test_create_and_fund_bond() { }
#[tokio::test] async fn test_release_bond_on_success() { }
#[tokio::test] async fn test_slash_bond_on_default() { }
#[tokio::test] async fn test_escrow_dispute_flow() { }
#[tokio::test] async fn test_bond_expiration() { }
```

#### 8.6 Edge Case Tests (`tests/edge_cases_test.rs`) (~350 LOC)

```rust
#[tokio::test] async fn test_expired_order_not_fetchable() { }
#[tokio::test] async fn test_negative_premium_discount() { }
#[tokio::test] async fn test_self_trade_prevented() { }
#[tokio::test] async fn test_order_expires_during_acceptance() { }
#[tokio::test] async fn test_concurrent_trades_stress() { }
#[tokio::test] async fn test_malformed_nip69_tags() { }
#[tokio::test] async fn test_contradictory_attestations() { }
#[tokio::test] async fn test_max_amount_limits() { }
```

**Files:**
- `crates/neobank/tests/settlement_test.rs` (NEW)
- `crates/neobank/tests/relay_test.rs` (NEW)
- `crates/neobank/tests/rfq_test.rs` (NEW)
- `crates/neobank/tests/reputation_test.rs` (NEW)
- `crates/neobank/tests/escrow_test.rs` (NEW)
- `crates/neobank/tests/edge_cases_test.rs` (NEW)

---

### Phase 9: Documentation

#### 9.1 Update EXCHANGE-API.md (~500 LOC additions)

Add sections for:
- Real settlement modes (v0, v1 with examples)
- Relay integration guide
- RFQ workflow
- Treasury Agent setup
- Mint trust configuration
- Escrow/bond system
- Error handling reference
- Troubleshooting guide

#### 9.2 Create SETTLEMENT.md (~300 LOC)

Document:
- Settlement protocol versions
- Reputation-based flow diagram
- Atomic P2PK flow diagram
- Timeout handling
- Dispute resolution

#### 9.3 Create RFQ.md (~200 LOC)

Document:
- RFQ request/response format
- Quote selection strategies
- Integration with Treasury Agents

#### 9.4 Update README.md (~100 LOC additions)

Add:
- Production setup guide
- Relay configuration
- Settlement mode selection

**Files:**
- `crates/neobank/docs/EXCHANGE-API.md` (MODIFY)
- `crates/neobank/docs/SETTLEMENT.md` (NEW)
- `crates/neobank/docs/RFQ.md` (NEW)
- `crates/neobank/README.md` (MODIFY)

---

## File Summary

### New Files (10)
| File | LOC | Purpose |
|------|-----|---------|
| `settlement.rs` | ~400 | Settlement engine (mock, reputation, atomic) |
| `relay.rs` | ~350 | Nostr relay integration |
| `reputation.rs` | ~300 | Reputation scoring and fetching |
| `rfq.rs` | ~350 | RFQ market (NIP-90) |
| `treasury_agent.rs` | ~400 | Treasury Agent services |
| `mint_trust.rs` | ~300 | Mint discovery (NIP-87) |
| `escrow.rs` | ~350 | Bond/escrow system |
| `SETTLEMENT.md` | ~300 | Settlement documentation |
| `RFQ.md` | ~200 | RFQ documentation |

### Modified Files (4)
| File | Changes |
|------|---------|
| `wallet.rs` | +200 LOC (proof transfer, P2PK) |
| `exchange.rs` | +150 LOC (relay mode, settlement engine) |
| `lib.rs` | +20 LOC (exports) |
| `EXCHANGE-API.md` | +500 LOC (full API docs) |
| `README.md` | +100 LOC (production guide) |

### New Test Files (6)
| File | Tests |
|------|-------|
| `settlement_test.rs` | ~12 tests |
| `relay_test.rs` | ~7 tests |
| `rfq_test.rs` | ~5 tests |
| `reputation_test.rs` | ~5 tests |
| `escrow_test.rs` | ~5 tests |
| `edge_cases_test.rs` | ~8 tests |

**Total: ~4,500 new LOC + ~42 new tests**

---

## Implementation Order

1. **settlement.rs** + wallet.rs extensions (blocking everything)
2. **relay.rs** + exchange.rs modifications (enables real operation)
3. **reputation.rs** (enables trust-based settlement)
4. **rfq.rs** (market discovery)
5. **treasury_agent.rs** (liquidity provision)
6. **mint_trust.rs** (mint selection)
7. **escrow.rs** (collateral system)
8. **Tests** (comprehensive coverage)
9. **Documentation** (full API reference)

---

## Dependencies

- `nostr-client` crate for relay connectivity
- CDK with NUT-11 support for P2PK (may need to verify)
- Test relay from existing NIP-SA tests

---

## Success Criteria

1. All three settlement modes working
2. Orders published to and fetched from real relays
3. RFQ market operational
4. Treasury Agent can provide liquidity
5. Mint trust scoring functional
6. Escrow protects large trades
7. 42+ tests passing
8. Complete documentation
