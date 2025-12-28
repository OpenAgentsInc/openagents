# Neobank MVP - Multi-Currency Agent Treasury

## THE FEATURE: Agents Hold and Swap BTC ↔ USD

Agents can hold both Bitcoin AND dollar stablecoins. They swap between them.

**Example flows:**

```
Agent earns 50,000 sats for completing a bounty
    ↓
Agent swaps to $50 USD (locks in value, no volatility)
    ↓
Agent holds stable while idle
    ↓
Agent needs to pay compute provider (BTC only)
    ↓
Agent swaps $10 USD → ~10,000 sats
    ↓
Agent pays provider
```

```
Operator funds agent with $500 USD
    ↓
Agent auto-converts to BTC when paying (at current rate)
    ↓
Agent converts earnings back to USD (volatility protection)
    ↓
Agent's P&L is in USD (makes sense to operator)
```

**Why this is neobank:**
- Multi-currency treasury (not just sats)
- FX conversion on demand
- Volatility hedging
- Asset allocation policies
- Real financial operations

---

## How It Works

### USD Cashu Mints Exist Today

`stablenut.cashu.network` issues dollar-denominated eCash:
- Amounts in cents (not sats)
- Same Cashu protocol
- Same privacy properties
- Backed by BTC, volatility absorbed by mint

### Agent Wallet Structure

```rust
pub struct AgentTreasury {
    /// BTC balance (Cashu proofs from BTC mint)
    pub btc: CashuWallet,

    /// USD balance (Cashu proofs from USD mint)
    pub usd: CashuWallet,

    /// Spark wallet for Lightning send/receive
    pub spark: SparkWallet,

    /// Preferred holding currency (where to park earnings)
    pub default_currency: Currency,
}

impl AgentTreasury {
    /// Swap between currencies
    pub async fn swap(&mut self, from: Currency, to: Currency, amount: Money) -> Result<SwapReceipt>;

    /// Pay invoice, auto-selecting currency
    pub async fn pay(&mut self, invoice: &str) -> Result<PaymentReceipt>;

    /// Total balance in preferred currency
    pub fn total_balance(&self, in_currency: Currency) -> Money;
}
```

### Swap Mechanics

```
BTC → USD swap:
1. Agent has BTC Cashu proofs
2. Redeem proofs at BTC mint → mint pays LN invoice to USD mint
3. USD mint issues USD proofs to agent
4. Agent now holds USD

USD → BTC swap:
1. Agent has USD Cashu proofs
2. Redeem proofs at USD mint → mint pays LN invoice to BTC mint
3. BTC mint issues BTC proofs to agent
4. Agent now holds BTC
```

This uses existing Cashu melt/mint operations. Mints handle the exchange.

---

## What We Already Have

### Spark Wallet ✅
- Lightning send/receive
- Balance tracking
- Payment history
- Use for: funding treasury, withdrawals, direct LN payments

### NIP-60 Types ✅
- `CashuProof` struct
- `TokenEvent`, `WalletEvent`
- Use for: proof storage format

### Budget Enforcement ✅
- Daily limits, per-tick limits
- Use for: spending controls (now in any currency)

---

## What We Build

### Phase 1: Cashu Client (3-4 days)

Connect to existing Cashu mints (don't run our own).

```rust
// crates/neobank/src/cashu/client.rs

pub struct CashuWallet {
    mint_url: Url,
    unit: CurrencyUnit,  // Sat or Cent
    proofs: Vec<Proof>,
    keyset_counters: HashMap<KeysetId, u64>,
}

impl CashuWallet {
    /// Create mint quote (get LN invoice to pay)
    pub async fn create_mint_quote(&self, amount: u64) -> Result<MintQuote>;

    /// Mint proofs after paying invoice
    pub async fn mint(&mut self, quote: &MintQuote) -> Result<Vec<Proof>>;

    /// Create melt quote (get LN invoice we'll pay)
    pub async fn create_melt_quote(&self, invoice: &str) -> Result<MeltQuote>;

    /// Melt proofs to pay invoice
    pub async fn melt(&mut self, quote: &MeltQuote) -> Result<MeltResult>;

    /// Local balance from proofs
    pub fn balance(&self) -> u64;
}
```

**Mints to support:**
- BTC: `https://mint.minibits.cash` or `https://8333.space`
- USD: `https://stablenut.cashu.network`

### Phase 2: Multi-Currency Treasury (2-3 days)

```rust
// crates/neobank/src/treasury.rs

pub struct AgentTreasury {
    pub btc_wallet: CashuWallet,  // BTC mint
    pub usd_wallet: CashuWallet,  // USD mint
    pub spark: SparkWallet,       // Existing
    pub config: TreasuryConfig,
}

pub struct TreasuryConfig {
    /// Where to hold idle funds
    pub default_currency: Currency,

    /// Auto-convert earnings to default currency
    pub auto_convert_earnings: bool,

    /// Minimum BTC to keep for payments
    pub btc_reserve_sats: u64,
}

impl AgentTreasury {
    /// Deposit from Spark to Cashu (pick currency)
    pub async fn deposit(&mut self, amount_sats: u64, to: Currency) -> Result<()>;

    /// Withdraw from Cashu to Spark
    pub async fn withdraw(&mut self, amount_sats: u64) -> Result<()>;

    /// Swap between currencies
    pub async fn swap(&mut self, amount: Money, to: Currency) -> Result<SwapReceipt>;

    /// Pay Lightning invoice (auto-select source)
    pub async fn pay_invoice(&mut self, bolt11: &str) -> Result<PaymentReceipt>;

    /// Total balance in specified currency
    pub fn balance(&self, currency: Currency) -> Money;
}
```

### Phase 3: Swap Engine (2-3 days)

```rust
// crates/neobank/src/swap.rs

pub struct SwapEngine {
    btc_mint: CashuWallet,
    usd_mint: CashuWallet,
    rate_service: ExchangeRateService,
}

impl SwapEngine {
    /// Swap BTC → USD
    /// 1. Melt BTC proofs → get sats as LN
    /// 2. LN pays USD mint invoice
    /// 3. Mint USD proofs
    pub async fn btc_to_usd(&mut self, amount_sats: u64) -> Result<SwapResult>;

    /// Swap USD → BTC
    /// 1. Melt USD proofs → get LN payment
    /// 2. LN pays BTC mint invoice
    /// 3. Mint BTC proofs
    pub async fn usd_to_btc(&mut self, amount_cents: u64) -> Result<SwapResult>;

    /// Get current rate
    pub async fn get_rate(&self) -> Result<ExchangeRate>;

    /// Preview swap (show what you'd get)
    pub async fn preview(&self, from: Money, to: Currency) -> Result<SwapPreview>;
}
```

### Phase 4: CLI & Integration (2-3 days)

```bash
# Check balances
openagents treasury balance
# BTC: 50,000 sats ($50.00)
# USD: $125.00
# Total: $175.00

# Swap
openagents treasury swap 50000 sats to usd
# Swapped 50,000 sats → $49.85 USD (rate: $100,234/BTC, fee: 0.3%)

# Pay invoice from treasury
openagents treasury pay lnbc1...
# Paid 10,000 sats from BTC balance

# Set default holding currency
openagents treasury config set default-currency usd

# Deposit from Spark to treasury
openagents treasury deposit 100000 --to btc
```

Wire into Autopilot:
- Agent uses treasury for all payments
- Earnings auto-convert to default currency
- Budget enforcement works across currencies

---

## Reliability Requirements

### Idempotency

Every operation must be safe to retry:

```rust
impl CashuWallet {
    /// Idempotent melt - safe to call multiple times with same quote
    pub async fn melt_idempotent(&mut self, quote: &MeltQuote) -> Result<MeltResult> {
        // Check if already completed
        if let Some(result) = self.get_completed_melt(&quote.id).await? {
            return Ok(result);
        }

        // Execute melt
        let result = self.melt_internal(quote).await?;

        // Persist result before returning
        self.store_melt_result(&quote.id, &result).await?;

        Ok(result)
    }
}
```

### Crash Recovery

On startup, resolve incomplete operations:

```rust
impl AgentTreasury {
    /// Called on every startup
    pub async fn recover(&mut self) -> Result<RecoveryReport> {
        // 1. Find pending quotes, resolve state
        // 2. Release orphaned proof reservations
        // 3. Sync keyset counters
        // 4. Verify balance consistency
        // See mvp.md ReconciliationService for full implementation
    }
}
```

### Proof Persistence Order

**Critical:** Persist proofs BEFORE any network call that depends on them.

```
WRONG:
1. Reserve proofs in memory
2. Call mint.melt()
3. Persist proof state  ← Crash here = lost proofs

RIGHT:
1. Reserve proofs
2. Persist reservation to DB
3. Call mint.melt()
4. Persist completion to DB
```

---

## Exchange Rate Service

```rust
// crates/neobank/src/rates.rs

pub struct ExchangeRateService {
    providers: Vec<Box<dyn RateProvider>>,
    cache: Cache<Rate>,
}

impl ExchangeRateService {
    pub async fn get_btc_usd(&self) -> Result<f64>;
}

// Providers: Mempool.space, Coingecko, Coinbase (fallback chain)
```

---

## Files to Create

```
crates/neobank/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── cashu/
│   │   ├── mod.rs
│   │   ├── client.rs      # Cashu wallet operations
│   │   ├── proof.rs       # Proof storage
│   │   └── mint.rs        # Mint configuration
│   ├── treasury.rs        # Multi-currency treasury
│   ├── swap.rs            # BTC ↔ USD swap engine
│   ├── rates.rs           # Exchange rate service
│   └── cli.rs             # CLI commands
```

---

## Timeline

| Phase | Days | Deliverable |
|-------|------|-------------|
| 1. Cashu Client | 3-4 | Connect to BTC + USD mints |
| 2. Treasury | 2-3 | Multi-currency balance tracking |
| 3. Swap Engine | 2-3 | BTC ↔ USD conversion |
| 4. CLI + Integration | 2-3 | Commands, Autopilot wiring |
| **Total** | **9-13 days** | Agents with multi-currency treasury |

---

## The Pitch

**Before:** Agent has sats. Sats go up/down. Operator confused.

**After:** Agent has treasury with BTC and USD. Holds earnings in USD (stable). Converts to BTC to pay providers. Operator sees P&L in dollars.

**This is real financial infrastructure for autonomous agents.**
