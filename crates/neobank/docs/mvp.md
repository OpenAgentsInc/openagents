# Neobank MVP

**What we're building first, and why it matters.**

---

## The Core Problem

Autopilot agents spend money (API calls, compute). Today:

- Budgets are in sats, but operators think in dollars
- No spending limits—agent can burn through allocation
- No audit trail linking spend to reasoning
- Agents can't receive payments (no identity for receivables)
- Every payment is on-chain or LN—no privacy for micropayments

## MVP Scope

### What We Build

```
┌─────────────────────────────────────────────────────────────────┐
│                      NEOBANK MVP                                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  CORE TYPES                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           │
│  │  Money<C>    │  │   Account    │  │   Receipt    │           │
│  │  (BTC/USD)   │  │  (Operating) │  │ (with trace) │           │
│  └──────────────┘  └──────────────┘  └──────────────┘           │
│                                                                  │
│  SERVICES                                                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           │
│  │ ExchangeRate │  │   Budget     │  │    Cashu     │           │
│  │   Service    │  │   Enforcer   │  │   Client     │           │
│  └──────────────┘  └──────────────┘  └──────────────┘           │
│                                                                  │
│  INTEGRATION                                                     │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  Autopilot Integration: CostTracker uses neobank backend │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### What We Don't Build (Yet)

- Our own Cashu mint (use existing trusted mints)
- Taproot Assets stablecoins
- Full FROST threshold signing (single-key first)
- Lightning Address server (Phase 2)
- Fiat on/off ramps
- Complex approval workflows
- Multi-account hierarchies

---

## Components

### 1. Money Type

```rust
// Type-safe monetary amounts
let budget = Money::<Dollar>::new(50, UsdUnit::Usd);  // $50
let spent = Money::<Bitcoin>::new(15000, BtcUnit::Sat);  // 15,000 sats

// Convert at current rate
let spent_usd = spent.convert(rate);  // ~$15 at current rate
let remaining = budget.subtract(&spent_usd);  // ~$35
```

**Benefit:** Operators configure budgets in USD. System handles conversion internally. No more "how many sats is $50 today?"

### 2. Exchange Rate Service

```rust
let service = ExchangeRateService::new(vec![
    Box::new(MempoolSpaceProvider),
    Box::new(CoingeckoProvider),
    Box::new(CoinbaseProvider),
]);

// Tries providers in order, caches result
let rate = service.get_rate::<Bitcoin, Dollar>().await?;
```

**Benefit:** Reliable BTC/USD rates with automatic fallback. Rate used for budget evaluation and receipt generation.

### 3. Account Model (Simplified)

```rust
pub struct Account {
    pub id: AccountId,
    pub name: String,
    pub currency: Currency,  // BTC or USD
    pub balance: Money,
    pub daily_limit: Option<Money>,
    pub daily_spent: Money,
    pub last_reset: DateTime<Utc>,
}
```

**Benefit:** Each agent/workload has an account with tracked balance and optional daily cap.

### 4. Budget Enforcer

```rust
impl BudgetEnforcer {
    pub fn check_spend(&self, account: &Account, amount: Money) -> SpendDecision {
        // Reset daily spent if new day
        let account = self.maybe_reset_daily(account);

        // Check balance
        if amount > account.balance {
            return SpendDecision::Denied { reason: "Insufficient balance" };
        }

        // Check daily limit
        if let Some(limit) = account.daily_limit {
            if account.daily_spent + amount > limit {
                return SpendDecision::Denied { reason: "Daily limit exceeded" };
            }
        }

        SpendDecision::Approved
    }
}
```

**Benefit:** Agent physically cannot exceed budget. Operator sleeps at night.

### 5. Cashu Client

```rust
pub struct CashuClient {
    mint_url: Url,
    proofs: Vec<Proof>,  // Local proof storage
}

impl CashuClient {
    // Deposit: Pay LN invoice, receive proofs
    pub async fn deposit(&mut self, amount: Money<Bitcoin>) -> Result<()>;

    // Withdraw: Spend proofs, pay LN invoice
    pub async fn pay_invoice(&mut self, invoice: &str) -> Result<Receipt>;

    // Balance from local proofs
    pub fn balance(&self) -> Money<Bitcoin>;
}
```

**Benefit:** Payments via eCash—private, instant, no channel management. We connect to existing mints (Minibits, Cashu.me, etc.) rather than running our own.

### 6. Receipt with Trajectory Link

```rust
pub struct Receipt {
    pub id: ReceiptId,
    pub timestamp: DateTime<Utc>,
    pub amount: Money,
    pub fee: Money,

    // What was paid
    pub payment_type: PaymentType,  // LightningInvoice, CashuToken, etc.
    pub payment_proof: String,       // Preimage, txid, etc.

    // Why it was paid (the key innovation)
    pub trajectory_session_id: Option<String>,
    pub tool_call_id: Option<String>,
    pub description: String,

    // Policy
    pub policy_rule: String,  // "auto_approve_under_5_usd"
    pub account_id: AccountId,
}
```

**Benefit:** Every payment answers "why did the agent spend this?" Link to trajectory means full auditability.

---

## Autopilot Integration

### Current State

```rust
// In autopilot today
pub struct CostTracker {
    pub daily_input_tokens: u64,
    pub daily_output_tokens: u64,
    pub daily_cost_usd: f64,  // Estimated, not actual
    pub session_start: DateTime<Utc>,
}
```

Problems:
- `daily_cost_usd` is estimate based on token counts
- No actual payment tracking
- No budget enforcement (just logging)
- No link between cost and specific actions

### With Neobank MVP

```rust
// Autopilot with neobank
pub struct CostTracker {
    account: neobank::Account,
    budget_enforcer: neobank::BudgetEnforcer,
    cashu_client: neobank::CashuClient,
    rate_service: neobank::ExchangeRateService,
}

impl CostTracker {
    pub async fn record_api_cost(
        &mut self,
        provider: &str,
        tokens: TokenUsage,
        trajectory_id: &str,
        tool_call_id: &str,
    ) -> Result<Receipt> {
        // Calculate cost in USD
        let cost = self.calculate_cost(provider, tokens);

        // Check budget
        let decision = self.budget_enforcer.check_spend(&self.account, cost);
        if decision.is_denied() {
            return Err(BudgetExceeded);
        }

        // If provider accepts Lightning, pay via Cashu
        let receipt = self.cashu_client.pay_invoice(
            &provider_invoice,
            trajectory_id,
            tool_call_id,
        ).await?;

        // Update account
        self.account.record_spend(cost, receipt.id);

        Ok(receipt)
    }
}
```

---

## Benefits Unlocked

### For Operators

| Before | After |
|--------|-------|
| "Set budget to 500,000 sats" | "Set budget to $50/day" |
| Agent can spend unlimited | Agent stops at limit |
| "What did it spend on?" | Click receipt → see trajectory |
| Trust agent won't overspend | Cryptographic enforcement |
| Manual top-ups | Balance alerts, easy deposits |

### For Agents

| Before | After |
|--------|-------|
| No spending identity | Account with balance |
| Can't receive payments | (Phase 2: Lightning Address) |
| Every payment visible on-chain | eCash privacy for micropayments |
| Single pool of funds | Per-task/per-workload accounts |

### For the Platform

| Before | After |
|--------|-------|
| Agents are cost centers | Agents can have P&L |
| No payment data | Rich payment telemetry |
| Can't price agent services | Foundation for marketplace |
| BTC-only thinking | Multi-currency foundation |

---

## Implementation Plan

### Phase 1: Core Types (Week 1-2)

```
□ Money<C> type with BTC and USD
□ Currency trait and unit conversions
□ ExchangeRateService with 3 providers
□ Basic Account struct
□ Receipt struct with trajectory fields
```

Deliverable: `cargo test -p neobank` passes with type tests.

### Phase 2: Budget Enforcement (Week 2-3)

```
□ BudgetEnforcer with daily limits
□ SpendDecision enum
□ Account persistence (SQLite)
□ Daily reset logic
```

Deliverable: Can create account, check budgets, persist state.

### Phase 3: Cashu Integration (Week 3-4)

```
□ CashuClient connecting to external mint
□ Deposit flow (LN → proofs)
□ Withdrawal flow (proofs → LN invoice)
□ Proof storage and management
□ Receipt generation with proofs
```

Deliverable: Can deposit sats, pay invoices via eCash.

### Phase 4: Autopilot Integration (Week 4-5)

```
□ Replace CostTracker internals with neobank
□ Wire budget checks into agent loop
□ Generate receipts for API calls
□ Link receipts to trajectory sessions
□ Surface budget status in dashboard
```

Deliverable: Autopilot uses real budgets with enforcement.

### Phase 5: Polish (Week 5-6)

```
□ CLI commands for account management
□ Budget alerts and notifications
□ Receipt export (CSV, JSON)
□ Documentation and examples
□ Error handling and edge cases
```

Deliverable: Production-ready MVP.

---

## What's NOT in MVP

These come later:

**Phase 2 Features:**
- Lightning Address for agent receivables
- USD-denominated Cashu (requires mint that supports it)
- Multiple accounts per agent
- Approval workflows for large spends

**Phase 3 Features:**
- FROST threshold signing
- Running our own mint
- Taproot Assets integration
- Fiat on/off ramps

**Phase 4 Features:**
- Multi-agent treasury hierarchies
- Policy engine with complex rules
- Compliance adapters
- Enterprise SSO/audit integration

---

## Success Criteria

MVP is successful when:

1. **Operators can set USD budgets** and agents respect them
2. **Every payment has a receipt** linked to trajectory
3. **Agents use eCash** for micropayments (not raw LN)
4. **Budget exhaustion stops the agent** (not just logs warning)
5. **Dashboard shows spend** in dollars with drill-down to receipts

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| External mint unreliable | Support multiple mints, fallback to direct LN |
| Exchange rate stale | Short cache TTL, multiple providers |
| Proof storage corruption | Backup proofs, recovery from mint |
| Budget race conditions | Pessimistic locking on spend checks |
| USD mint doesn't exist | Start BTC-only, convert at display time |

---

## Open Questions

1. **Which Cashu mints to trust initially?**
   - Candidates: Minibits, Cashu.me, LNbits instances
   - Need: Reliability, uptime, fee structure

2. **How to handle partial failures?**
   - Invoice paid but receipt not stored
   - Proofs received but balance not updated

3. **Where to persist account state?**
   - Same SQLite as autopilot?
   - Separate neobank database?

4. **How to bootstrap accounts?**
   - Operator deposits via CLI?
   - Auto-deposit from connected LN wallet?

---

## Summary

The MVP gives us:

- **Type-safe money** (no more raw i64 sats everywhere)
- **USD budgets** (operators think in dollars)
- **Actual enforcement** (not just logging)
- **Audit trail** (receipt → trajectory → reasoning)
- **eCash payments** (privacy for micropayments)

This is the foundation. Everything else builds on having proper money types, accounts, and receipts.
