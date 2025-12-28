# OpenAgents Neobank

**Treasury OS for humans and agent fleets.**

The neobank crate provides programmable treasury management, multi-rail payment routing, and budget enforcement for autonomous AI agents operating on Bitcoin.

## Overview

OpenAgents Neobank is not a bank. It's a **programmable treasury + payments router** for agents:

- **Self-custody by default** — Agent/human keys protected by FROST 2-of-3 threshold signatures
- **Stable unit-of-account** — Support for stablecoins where budgets and pricing need predictability
- **Multi-rail routing** — Intelligent routing across BTC Lightning, Taproot Assets, and eCash
- **Budget enforcement** — Per-agent daily caps, per-task limits, approval workflows
- **Auditable receipts** — Every payment links to trajectory logs and cryptographic proofs

## Core Types

### Money

Type-safe monetary amounts with currency enforcement:

```rust
/// Immutable monetary amount with currency type safety.
/// Internal storage in smallest unit (sat/cent) to avoid floating point errors.
pub struct Money<C: Currency> {
    amount: BigDecimal,
    _currency: PhantomData<C>,
}

impl<C: Currency> Money<C> {
    /// Create from amount in specified unit
    pub fn new(amount: impl Into<BigDecimal>, unit: C::Unit) -> Self;

    /// Arithmetic (returns new instance, never mutates)
    pub fn add(&self, other: &Self) -> Self;
    pub fn subtract(&self, other: &Self) -> Self;
    pub fn multiply(&self, factor: BigDecimal) -> Self;
    pub fn divide(&self, divisor: BigDecimal) -> Self;  // Rounds to unit precision

    /// Comparisons
    pub fn is_zero(&self) -> bool;
    pub fn is_positive(&self) -> bool;
    pub fn greater_than(&self, other: &Self) -> bool;

    /// Conversion to different currency (requires exchange rate)
    pub fn convert<D: Currency>(&self, rate: ExchangeRate<C, D>) -> Money<D>;

    /// Get amount in specified unit
    pub fn amount(&self, unit: C::Unit) -> BigDecimal;

    /// Format for display with locale
    pub fn format(&self, locale: &Locale, unit: Option<C::Unit>) -> String;
}

// Type aliases
pub type Btc = Money<Bitcoin>;
pub type Usd = Money<Dollar>;
```

### Currency Units

```rust
pub trait Currency {
    type Unit: CurrencyUnit;
    const BASE_UNIT: Self::Unit;
    const SMALLEST_UNIT: Self::Unit;
}

pub struct Bitcoin;
impl Currency for Bitcoin {
    type Unit = BtcUnit;
    const BASE_UNIT: BtcUnit = BtcUnit::Btc;
    const SMALLEST_UNIT: BtcUnit = BtcUnit::Msat;
}

pub enum BtcUnit {
    Btc,   // 1 BTC = 100_000_000 sat
    Sat,   // Base unit for most operations
    Msat,  // 1 sat = 1000 msat (Lightning precision)
}

pub struct Dollar;
impl Currency for Dollar {
    type Unit = UsdUnit;
    const BASE_UNIT: UsdUnit = UsdUnit::Usd;
    const SMALLEST_UNIT: UsdUnit = UsdUnit::Cent;
}

pub enum UsdUnit {
    Usd,   // Display unit
    Cent,  // Smallest unit (what Cashu USD mints use)
}
```

### Exchange Rates

```rust
/// Exchange rate between two currencies with timestamp.
pub struct ExchangeRate<From: Currency, To: Currency> {
    rate: BigDecimal,
    timestamp: DateTime<Utc>,
    source: RateSource,
    _from: PhantomData<From>,
    _to: PhantomData<To>,
}

/// Multi-provider rate service with fallback.
pub struct ExchangeRateService {
    providers: Vec<Box<dyn RateProvider>>,  // Priority order
    cache: Cache<CurrencyPair, ExchangeRate>,
    cache_ttl: Duration,
}

impl ExchangeRateService {
    /// Get rate, trying providers in order until one succeeds.
    pub async fn get_rate<F: Currency, T: Currency>(&self) -> Result<ExchangeRate<F, T>>;
}

/// Rate providers (implement fallback chain)
pub trait RateProvider {
    fn supported_pairs(&self) -> &[CurrencyPair];
    async fn fetch_rate(&self, pair: CurrencyPair) -> Result<BigDecimal>;
}

// Built-in providers: Mempool.space, Coingecko, Coinbase
```

## Money Rails

### Rail A: Native Bitcoin (Baseline)

For machine-to-machine micropayments, censorship resistance, and settlement finality.

- **Lightning** — NIP-57 zaps, L402 pay-per-call APIs
- **On-chain BTC** — Treasury settlement, large transfers, long-term reserves
- **Integration**: `crates/spark` (Breez SDK)

### Rail B: USD Stablecoins

For stable budgeting, enterprise procurement, payroll, and predictable pricing.

**Available Today: Cashu USD Mints**

Cashu mints can issue dollar-denominated eCash backed by BTC. The mint handles the volatility; users hold stable-value tokens.

- Amounts in cents (smallest unit)
- Same privacy properties as BTC eCash
- Instant settlement within mint
- Lightning send/receive with automatic BTC↔USD conversion

**Future: Taproot Assets**

- Hold/send/receive Taproot Assets on-chain and over Lightning
- Track assets by `group_key` for fungibility across issuance batches
- Use `AddressV2` for reusable stablecoin receive addresses
- Support for USDT-on-Taproot-Assets (and future stablecoins)

### Rail C: eCash (Cashu + Fedimint)

For privacy, cash-like UX, tips, and low-trust small-value payments.

- **NIP-87** — Mint discovery and reputation
- **NIP-60** — Wallet state portability (relay-synced)
- **NIP-61** — Nutzaps (payment-as-receipt)
- **Multi-currency** — Same mint can issue BTC and USD tokens

## Account Model

### Entities

| Entity | Role |
|--------|------|
| **Human Operator** | Funds the system, sets policy, reviews exceptions |
| **Agent** | Autonomous actor with identity, wallets, and budget |
| **Guardian** | Recovery/safety signer (threshold share) |
| **Policy Signer** | Marketplace/compliance signer enforcing constraints |

### Accounts (Wallet Partitions)

| Account | Purpose |
|---------|---------|
| **Treasury** | Org-level long-term holdings, top-ups, reserves |
| **Operating** | Per-agent or per-workload day-to-day spending |
| **Escrow** | Pay-after-verify, disputes |
| **Payroll/Rewards** | Bounties, skill revenue splits, contributor payouts |

## Key Management

Default 2-of-3 FROST topology per account:

- **Share A**: Agent runtime enclave (or agent's secure module)
- **Share B**: Policy signer (enforces budgets/allowlists)
- **Share C**: Guardian/recovery (human-controlled)

Operators cannot extract keys. The agent truly owns its identity.

### Deterministic Derivation

All secrets derived from a single seed via BIP-32 paths:

```rust
/// Derivation paths for different key types
pub struct DerivationPaths {
    /// Identity keys: m/44'/0'/0'
    pub identity: &'static str,
    /// Encryption keys: m/10111099'/0' (10111099 = 'enc' in ASCII)
    pub encryption: &'static str,
    /// Cashu secrets: m/129372'/0'/{keyset_id}'/{counter}
    pub cashu: &'static str,
    /// Spark wallet: m/44'/0'/1' (account 1 for mainnet)
    pub spark: &'static str,
}

/// Per-keyset counters for deterministic Cashu secret generation.
/// Enables full wallet recovery from seed.
pub struct KeysetCounters {
    counters: HashMap<KeysetId, u64>,
}
```

## Payment State Machine

Every payment follows a formal state machine:

```
┌─────────┐
│ CREATED │  Quote generated, not yet confirmed
└────┬────┘
     │ user confirms / agent authorizes
     ▼
┌─────────┐
│ UNPAID  │  Proofs reserved, ready to execute
└────┬────┘
     │ submit to mint/network
     ▼
┌─────────┐
│ PENDING │  In-flight, awaiting confirmation
└────┬────┘
     │
     ├─────────────┬─────────────┐
     ▼             ▼             ▼
┌─────────┐  ┌─────────┐  ┌─────────┐
│  PAID   │  │ FAILED  │  │ EXPIRED │
└─────────┘  └─────────┘  └─────────┘
```

### Quote Types

```rust
/// Send quote: prepared outbound payment
pub struct SendQuote<C: Currency> {
    pub id: QuoteId,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub account_id: AccountId,

    /// What the user requested to send
    pub amount_requested: Money<C>,
    /// What the receiver will get (after fees)
    pub amount_to_receive: Money<C>,
    /// Lightning routing fee reserve
    pub lightning_fee_reserve: Money<C>,
    /// Mint/protocol fee
    pub protocol_fee: Money<C>,

    /// Reserved proofs (eCash) or UTXOs
    pub reserved_funds: ReservedFunds,

    pub state: QuoteState,
    pub version: u64,  // Optimistic locking
}

/// Receive quote: prepared inbound payment
pub struct ReceiveQuote<C: Currency> {
    pub id: QuoteId,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub account_id: AccountId,

    /// BOLT11 invoice or other payment request
    pub payment_request: String,
    /// Amount that will be credited (after fees)
    pub amount_to_receive: Money<C>,
    /// Minting fee (if applicable)
    pub minting_fee: Option<Money<C>>,

    pub state: QuoteState,
}

pub enum QuoteState {
    Created,
    Unpaid,
    Pending,
    Paid { preimage: Option<String>, receipt: Receipt },
    Failed { reason: String },
    Expired,
}
```

### Optimistic Locking

All mutable records include a `version` field. Updates must match current version:

```rust
pub async fn update_quote(&self, quote: &SendQuote) -> Result<()> {
    let rows = sqlx::query!(
        "UPDATE quotes SET state = $1, version = version + 1
         WHERE id = $2 AND version = $3",
        quote.state, quote.id, quote.version
    ).execute(&self.pool).await?;

    if rows.rows_affected() == 0 {
        return Err(Error::ConcurrentModification);
    }
    Ok(())
}
```

## Proof Lifecycle (eCash)

eCash proofs ("coins") have their own state machine:

```
┌──────────┐
│ UNSPENT  │  Available for spending
└────┬─────┘
     │ quote created, proofs selected
     ▼
┌──────────┐
│ RESERVED │  Locked for pending payment
└────┬─────┘
     │
     ├─────────────┬─────────────┐
     │             │             │
     ▼             ▼             ▼
┌──────────┐  ┌──────────┐  (returned to UNSPENT
│  SPENT   │  │ PENDING  │   if quote expires/fails)
└──────────┘  └────┬─────┘
                   │ mint confirms
                   ▼
              ┌──────────┐
              │  SPENT   │
              └──────────┘
```

### Proof Storage

```rust
/// A Cashu proof (blind-signed token)
pub struct Proof {
    pub id: ProofId,
    pub account_id: AccountId,

    pub keyset_id: KeysetId,
    pub amount: u64,  // In smallest unit (sat or cent)

    /// Encrypted at rest (ECIES to user's encryption pubkey)
    pub secret: EncryptedSecret,
    pub unblinded_signature: String,

    /// DLEQ proof for signature verification
    pub dleq: Option<DleqProof>,

    pub state: ProofState,
    pub version: u64,

    pub created_at: DateTime<Utc>,
    pub reserved_at: Option<DateTime<Utc>>,
    pub spent_at: Option<DateTime<Utc>>,
}

pub enum ProofState {
    Unspent,
    Reserved { quote_id: QuoteId },
    Pending,
    Spent,
}
```

## Agent Payment Addresses

Agents receive payments via Lightning Addresses (LNURL-pay, LUD-16):

```
agent-solver-7x3k@treasury.openagents.com
                   ↓
/.well-known/lnurlp/agent-solver-7x3k
                   ↓
Returns: { callback, minSendable, maxSendable, metadata }
                   ↓
Callback creates receive quote → returns BOLT11 invoice
```

### AgentPaymentProfile

```rust
/// Published to Nostr for discoverability
pub struct AgentPaymentProfile {
    pub agent_pubkey: PublicKey,
    pub lightning_address: String,

    /// Supported currencies (may include USD via Cashu mint)
    pub supported_currencies: Vec<CurrencyInfo>,

    /// Default currency for amountless invoices
    pub default_currency: Currency,

    /// Min/max receivable amounts
    pub min_receivable: Money<Bitcoin>,
    pub max_receivable: Money<Bitcoin>,

    /// Optional: required payment metadata
    pub required_metadata: Vec<MetadataField>,
}

/// Cross-currency receive handling
pub async fn handle_lnurl_callback(
    &self,
    agent_id: &str,
    amount: Money<Bitcoin>,
) -> Result<LnurlPayResponse> {
    let agent = self.get_agent(agent_id).await?;
    let account = agent.default_account();

    // Convert if agent prefers different currency
    let receive_amount = if account.currency() != Currency::Btc {
        let rate = self.rates.get_rate::<Bitcoin, Dollar>().await?;
        amount.convert(rate)
    } else {
        amount
    };

    let quote = self.create_receive_quote(account, receive_amount).await?;

    Ok(LnurlPayResponse {
        pr: quote.payment_request,
        verify: format!("{}/verify/{}", self.base_url, quote.id),
        routes: vec![],
    })
}
```

## Privacy by Default

### Client-Side Encryption

Sensitive data is encrypted before storage using ECIES (Elliptic Curve Integrated Encryption Scheme):

```rust
/// All proof secrets encrypted to user's encryption pubkey.
/// Server never sees plaintext secrets.
pub struct EncryptedProofStore {
    encryption_pubkey: PublicKey,
}

impl EncryptedProofStore {
    pub fn encrypt_proof(&self, proof: &Proof) -> EncryptedProof {
        // ECIES with ChaCha20-Poly1305
        // Ephemeral key generated per encryption
        let ciphertext = ecies_encrypt(
            &proof.secret,
            &self.encryption_pubkey,
        );
        EncryptedProof {
            id: proof.id,
            keyset_id: proof.keyset_id,
            amount: proof.amount,  // Amount is public (for balance calculation)
            encrypted_secret: ciphertext,
            // ... other fields
        }
    }

    pub fn decrypt_proof(
        &self,
        encrypted: &EncryptedProof,
        private_key: &SecretKey,
    ) -> Result<Proof> {
        let secret = ecies_decrypt(&encrypted.encrypted_secret, private_key)?;
        // Reconstruct full proof
    }
}
```

### Batch Encryption

For efficiency, batch operations share an ephemeral key:

```rust
/// Encrypt multiple items with shared ephemeral key.
/// Items are linkable (same batch) but faster than individual encryption.
pub fn encrypt_batch<T: Serialize>(
    items: &[T],
    pubkey: &PublicKey,
) -> Vec<Ciphertext> {
    let ephemeral = SecretKey::random();
    let shared_secret = ecdh(&ephemeral, pubkey);

    items.iter().map(|item| {
        encrypt_with_shared_secret(item, &shared_secret, &ephemeral.public_key())
    }).collect()
}
```

### What's Encrypted

| Data | Encrypted? | Reason |
|------|------------|--------|
| Proof secrets | Yes | Spending authority |
| Transaction details | Yes | Payment metadata |
| Amounts | No | Needed for balance queries |
| Timestamps | No | Needed for ordering |
| Account names | Optional | User preference |

## TreasuryRouter

The core policy engine that decides:

- **Which rail** — BTC LN vs stable LN vs on-chain vs eCash
- **Which asset** — BTC vs USD stable
- **Which limits** — Daily, per-merchant, per-task, per-provider
- **When approvals** — Required thresholds
- **How receipts** — Recorded and published

### Example Policy Rules

```
Under $5 equivalent     → Allow eCash or Lightning automatically
Under $200              → Allow stablecoin LN if invoice is stable-denominated
Over $200               → Require human approval or guardian co-sign
Compute providers only  → Must have past verification + minimum reputation
```

## Receipts and Statements

Every payment yields:

1. **Cryptographic receipt** — Preimage / txid / taproot-assets proof / cashu proof ref
2. **Trajectory link** — "This spend happened during this agent session; here's why"
3. **Policy attestation** — Which rule allowed it, who co-signed

This is the "bank statement" equivalent for autonomous systems.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                     NEOBANK TREASURY LAYER                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐     │
│  │ TreasuryRouter  │  │  PolicyEngine   │  │  ReceiptLedger  │     │
│  │ (rail selection)│  │ (budget/limits) │  │ (audit trail)   │     │
│  └────────┬────────┘  └────────┬────────┘  └────────┬────────┘     │
│           └────────────────────┴────────────────────┘               │
│                               │                                     │
│  RAILS ──────────────────────────────────────────────────────────  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │   Bitcoin    │  │   Taproot    │  │    eCash     │              │
│  │  Lightning   │  │   Assets     │  │ Cashu/Fedimint│             │
│  │ (crates/spark)│ │  (planned)   │  │  (NIP-60/61) │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│                                                                     │
│  ADAPTERS ───────────────────────────────────────────────────────  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │ FiatRampAdapter│ │ CardAdapter │  │ComplianceAdapter│           │
│  │ (KYC + bank)  │  │(virtual/physical)│ (sanctions/risk)│         │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Relationship to Existing Crates

| Crate | Relationship |
|-------|--------------|
| `crates/spark` | Bitcoin/Lightning rail implementation (Breez SDK) |
| `crates/wallet` | User-facing wallet application, identity management |
| `crates/nostr/core` | Protocol types, event signing, NIP implementations |
| `crates/frostr` | FROST threshold signatures for key protection |
| `crates/marketplace` | Compute/skills marketplace (consumer of treasury services) |

## Graceful Degradation

Accounts track connectivity status for resilient operation:

```rust
pub struct Account {
    pub id: AccountId,
    pub name: String,
    pub account_type: AccountType,
    pub currency: Currency,
    pub is_online: bool,  // Can we reach the mint/service?
    // ...
}

pub enum AccountType {
    /// eCash account backed by a Cashu mint
    Cashu {
        mint_url: Url,
        is_test_mint: bool,
        keyset_counters: KeysetCounters,
        proofs: Vec<Proof>,
    },
    /// Lightning account via Spark L2
    Spark {
        network: SparkNetwork,
        balance: Option<Money<Bitcoin>>,  // None if offline
    },
}
```

### Offline Handling

```rust
impl AccountRepository {
    pub async fn get_with_timeout(&self, id: AccountId) -> Account {
        let mint_data = tokio::time::timeout(
            Duration::from_secs(10),
            self.fetch_mint_info(&account.mint_url),
        ).await;

        match mint_data {
            Ok(Ok(info)) => Account { is_online: true, ..account },
            Ok(Err(_)) | Err(_) => {
                // Mint unreachable - return cached data
                Account {
                    is_online: false,
                    // Proofs still available from local DB
                    // Balance calculation still works
                    // Spending disabled until reconnect
                    ..account
                }
            }
        }
    }
}
```

### What Works Offline

| Operation | Offline? | Notes |
|-----------|----------|-------|
| View balance | Yes | Calculated from local proofs |
| View history | Yes | Transactions stored locally |
| Receive payment | No | Need mint to create invoice |
| Send payment | No | Need mint to melt proofs |
| Generate address | Partial | Can show static address |

## Planned Development

### Phase 1: Core Types and Policy Engine

- [ ] `Money<C>` type with BigDecimal and currency safety
- [ ] `ExchangeRateService` with provider fallback
- [ ] Account types (Treasury, Operating, Escrow, Payroll)
- [ ] Quote/Payment state machines
- [ ] Proof lifecycle management
- [ ] TreasuryRouter with configurable policy rules
- [ ] Receipt generation with trajectory links

### Phase 2: Multi-Rail Routing

- [ ] Integration with `crates/spark` for BTC Lightning
- [ ] Cashu mint integration (BTC and USD)
- [ ] eCash proof encryption (ECIES)
- [ ] Rail selection logic based on amount/privacy/speed
- [ ] Cross-currency conversion with rate service

### Phase 3: Agent Payment Infrastructure

- [ ] Lightning Address server (LUD-16)
- [ ] AgentPaymentProfile Nostr events
- [ ] Cross-currency receive handling
- [ ] Payment verification callbacks

### Phase 4: Taproot Assets

- [ ] Stablecoin hold/send/receive
- [ ] Universe endpoint integration
- [ ] AddressV2 reusable addresses

### Phase 5: Nostr Protocol Extensions

- [ ] AssetRegistry events
- [ ] UniverseAnnouncement events
- [ ] Mint reputation aggregation (NIP-87)

### Phase 6: Fiat Adapters

- [ ] FiatRampAdapter interface
- [ ] CardAdapter interface
- [ ] ComplianceAdapter interface

## Killer Features

### A) Programmable Budgets for Autonomous Entities

Traditional neobanks give humans controls. Agents need:

- Per-agent daily caps
- Per-task caps
- Per-provider allowlists
- Approval workflows
- Velocity-aware throttles ("if APM spikes and failure rate rises, clamp spend")

### B) Receipts That Include "Why"

A bank statement says *what* you spent. An agentic neobank says:

- **Which agent** executed the payment
- **Which trajectory** it was part of
- **Which tool result verification** preceded it
- **Which policy allowed it**
- **Who co-signed it** (if threshold)

### C) Multi-Rail Routing as First-Class Primitive

Pick the best rail per context:

- LN BTC for tiny machine payments
- Taproot Asset stable LN for "USD pricing" at scale
- eCash for privacy / content tips / offline-ish workflows

## Documentation

- **[Research Document](docs/research.md)** — Full specification and ecosystem analysis
- **[Spark Integration](../spark/README.md)** — Bitcoin/Lightning rail
- **[SYNTHESIS.md](../../SYNTHESIS.md)** — How neobank fits the broader vision

## References

- [Taproot Assets Protocol](https://docs.lightning.engineering/the-lightning-network/taproot-assets/taproot-assets-protocol)
- [NIP-87: Ecash Mint Discoverability](https://nips.nostr.com/87)
- [NIP-60: Cashu Wallets](https://nips.nostr.com/60)
- [NIP-61: Nutzaps](https://nips.nostr.com/61)
- [Cashu Protocol](https://cashu.space/)
- [Fedimint](https://fedimint.org/)
