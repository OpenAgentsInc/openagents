## Spec: OpenAgents Liquidity Pool

### Goal

Provide **BTC-only, non-custodial-ish** (operator-custodied with cryptographic controls) liquidity that:

1. funds **Lightning channel liquidity + just-in-time payments** for agent commerce,
2. funds **NIP-AC outcome-scoped credit envelopes**,
3. earns **BTC-denominated revenue** from real network activity (routing + marketplace fees + credit fees),
4. plugs cleanly into **TreasuryRouter / Budgets / Receipts / Nostr**.

### Non-goals

* No governance token, no emissions, no “yield promises.”
* No permissioned custody assumptions beyond explicit signer/guardian policies.
* No long-term lockups; liquidity must remain withdrawable with bounded delay.

---

## 1) Entities

### 1.1 Pool

A Pool is an on-ledger accounting object backed by BTC held across:

* Lightning node/channel balances (local + remote reserves)
* On-chain BTC reserves (for rebalancing / channel opens)
* Optional Spark balances (if used as an operational rail)

### 1.2 Roles

* **Depositor (LP):** provides BTC capital.
* **Pool Operator:** runs LN node(s), rebalancing, routing, accounting, and enforcement. (May be an OpenAgents-operated service or third-party operator.)
* **Signer Set:** threshold keys that control pool treasury actions (channel opens/closes, large withdrawals, on-chain spends).
* **Borrowers:** agents requesting liquidity via **NIP-AC envelopes** or routing liquidity indirectly through payments.
* **Consumers:** Autopilot / marketplace flows that generate payment volume and fees.

---

## 2) Pool Types

Define three pool “products” (can be separate pools or one pool with partitions):

### A) LN Liquidity Pool (LLP)

Purpose: maintain inbound/outbound liquidity to settle marketplace payments and enable high throughput.

Uses capital for:

* channel opens
* channel reserves
* rebalancing (circular payments)
* JIT routing services where appropriate

Revenue sources:

* routing fees
* settlement spreads (optional, transparent)
* service fees for “liquidity guarantees” (optional)

### B) Credit Envelope Pool (CEP)

Purpose: fund **NIP-AC Outcome-Scoped Credit Envelopes (OSCE)** for agents.

Uses capital for:

* paying providers upon objective verification (NIP-90, L402, skill invocation)
* bridging timing gaps between “agent needs resource now” and “agent earns later”

Revenue sources:

* envelope fee (bps or sats)
* late penalties (discouraged; prefer reputation throttling)

### C) Rebalancing / Reserve Pool (RRP)

Purpose: keep the system alive and smooth.

* emergency channel rebalances
* fallback liquidity when routes are constrained
* cover temporary liquidity mismatches

Revenue sources:

* none required; funded by small skim from other pools

---

## 3) Trust Model and Keying

### 3.1 Treasury Control

Pool treasury actions require threshold signing (examples):

* **2-of-3**: Operator + Guardian + Independent Signer
* **3-of-5** for larger pools

### 3.2 Operational Hot Wallet

LN node operations require hot access to channel states. Mitigate by:

* limiting hot wallet exposure via:

  * channel sizing rules
  * sweeping policies
  * rate limits
  * automated alarms
* retaining most reserves in cold/threshold-controlled on-chain treasury

### 3.3 Agent Safety

Agents never get free-floating pool funds.
Agents access liquidity only through:

* paying invoices via the pool’s LN node (service)
* receiving OSCE-funded payments where issuer pays provider directly

---

## 4) Accounting Model

### 4.1 Pool Shares

Represent LP ownership as internal shares:

* `shares = deposit_sats / share_price_sats`
* share price floats with realized PnL

### 4.2 Segregated Ledgers

Maintain ledgers per pool partition:

* LLP ledger (routing PnL)
* CEP ledger (credit fees - losses)
* RRP ledger (funding only)

LPs can choose exposure:

* LLP-only
* CEP-only
* blended

### 4.3 Marking and Proof

Pool publishes periodic proof snapshots:

* total assets (by rail):

  * on-chain UTXOs
  * LN channel balances
  * Spark balance (if used)
* total liabilities:

  * LP share value
  * pending withdrawals
  * reserved envelope commitments

Snapshots are:

* signed by pool signer set
* posted to `/stats` and optionally mirrored to Nostr as an event

---

## 5) Deposits & Withdrawals

### 5.1 Deposits

Deposit methods:

* Lightning invoice (small deposits)
* On-chain deposit address (large deposits)

Deposit flow:

1. LP requests deposit quote (sats + expiry)
2. LP pays
3. Pool mints shares at current share_price
4. Receipt generated (see §9)

### 5.2 Withdrawals

Withdrawals must not break LN liquidity.
Mechanism:

* **T+Δ** withdrawal window (e.g., 24–72 hours) with a queue
* dynamic availability based on:

  * channel health
  * outstanding envelope commitments
  * reserve thresholds

Withdrawal flow:

1. LP requests withdrawal (shares or sats)
2. Pool returns earliest settlement time + estimated fee
3. Pool settles via:

   * LN payout (if liquidity allows)
   * on-chain payout (batched)
4. Shares burned; receipt emitted

Emergency withdrawal mode:

* only if pool is solvent and reserves allow
* can be disabled during incidents

---

## 6) Risk Controls (Hard Requirements)

### 6.1 LLP Controls

* max channel size per peer
* peer allowlists / reputation thresholds
* min inbound liquidity ratio target
* max daily rebalancing spend
* route selection constraints (avoid pathological routes)

### 6.2 CEP Controls

* OSCE only (no unscoped credit)
* max sats per envelope
* max outstanding envelopes per agent
* reputation-weighted credit limits (see §7)
* objective verification requirement for “auto-pay” (default)
* escrow / pay-after-verify by default for NIP-90

### 6.3 Global Controls

* circuit breakers:

  * if loss_rate > threshold → halt new envelopes
  * if LN failure rate spikes → halt large withdrawals
* operator rate limits
* “known-good” recovery playbook for LN node failures

---

## 7) NIP-AC Integration (CEP)

### 7.1 Envelope Underwriting

Issuer (pool) computes credit offer terms using:

* agent reputation (NIP-32 labels)
* objective success rate (job receipts)
* recency-weighted history
* current outstanding exposure
* requested scope type (nip90 vs l402 vs skill)

Example underwriting formula:

* `limit_sats = base + k * sqrt(success_volume_sats_30d)`
* `fee_bps = clamp(min_fee, max_fee, risk_score * scaler)`

### 7.2 Settlement Modes

**Mode 1: Issuer Pays Provider (recommended)**

* Agent obtains envelope
* Provider delivers outcome (NIP-90 result, etc.)
* Verifier checks objectively
* Pool pays provider directly (LN)
* Pool emits settlement receipt and reputation label

**Mode 2: Provider Pulls (optional)**

* Provider requests payment using signed spend authorization reference
* Pool validates envelope and outcome proof before paying

### 7.3 Defaults

* Envelope max expiry short (minutes-hours)
* No rolling credit lines
* Any default reduces limits quickly (reputation throttling)

---

## 8) Lightning Liquidity Integration (LLP)

### 8.1 Service Interface

Expose a **Liquidity Service API** to TreasuryRouter:

* `quote_invoice_pay(bolt11, max_fee, urgency, policy_context)`
* `pay_invoice(quote_id)`
* returns receipts: preimage hash pointer, route fees, timestamps

### 8.2 JIT Payments

For agents with low balance but strong reputation:

* TreasuryRouter can request a CEP envelope to cover the payment
* Pool pays invoice from LLP liquidity
* CEP ledger records it as envelope spend, settled by future agent earnings or by immediate fee charge

This links LLP ↔ CEP.

### 8.3 Rebalancing Strategy

* maintain target inbound/outbound bands
* pay for rebalances only within budget
* prefer internal network rebalances where possible (lower leakage)

---

## 9) Receipts & Observability

Every action produces a receipt object:

* `deposit_receipt`
* `withdraw_receipt`
* `invoice_pay_receipt`
* `envelope_issue_receipt`
* `envelope_settlement_receipt`
* `default_notice`

Receipts must be:

* idempotent
* signed
* linkable to:

  * agent run / trajectory session (`rlog` / ReplayBundle id)
  * policy that authorized it (Budget/Approval rule)
  * Nostr events (if relevant)

Publish:

* aggregate metrics to `/stats` (minute cache)
* optionally mirror summarized receipts to Nostr for public auditability

---

## 10) Protocol Surfaces and APIs

### 10.1 Runtime APIs (authoritative)

* `POST /v1/pools/:pool_id/deposit_quote`
* `POST /v1/pools/:pool_id/withdraw_request`
* `GET /v1/pools/:pool_id/status`
* `POST /v1/liquidity/quote_pay`
* `POST /v1/liquidity/pay`
* `POST /v1/credit/intent` (NIP-AC intent ingestion optional)
* `POST /v1/credit/offer`
* `POST /v1/credit/envelope`
* `POST /v1/credit/settle`

### 10.2 Nostr Events (interop)

* Mirror NIP-AC kinds (39240–39245) as defined
* Publish NIP-32 labels for success/default
* Optional: publish pool snapshots (signed) as a dedicated kind (or use a standard addressable event scheme)

---

## 11) Mapping to Your Existing Architecture

### TreasuryRouter (Neobank)

* decides:

  * pay from agent balance vs request CEP envelope
  * pay via LN direct vs Cashu vs Spark (if present)
  * enforce caps and approvals
* attaches:

  * policy ids
  * trajectory/run id
  * spend reason

MVP status in-repo:

* `crates/neobank/src/router.rs` exposes `quote_and_pay_bolt11(...)` and route policy selection (direct liquidity vs CEP).
* `crates/neobank/src/budgets.rs` provides reservation/finalization hooks with idempotent in-memory enforcement.
* `crates/neobank/src/receipts.rs` emits `openagents.neobank.payment_attempt_receipt.v1` with canonical hash + optional signatures.
* `crates/neobank/src/rails/runtime.rs` implements typed Runtime internal API calls for liquidity + credit lanes.
* `apps/runtime/src/bin/vignette-neobank-pay-bolt11.rs` is the end-to-end harness proving direct + CEP payment routing.

### Budgets + Approvals

* CEP issuance is just another spend path:

  * envelope issuance requires policy approval above thresholds
  * auto-approve envelopes under `X sats` for trusted agents

### Receipts

* every pool action emits typed receipts into your existing receipt pipeline
* receipts are what your `/stats` dashboard aggregates

### Exchange / FX (optional)

* later: pool can host treasury agents that quote FX and use LLP to settle

### NIP-90 compute marketplace

* CEP mode 1 is perfect for pay-after-verify:

  * provider submits result
  * verifier checks objective outputs
  * pool pays provider

---

# Recursive Growth Flywheel: AC + Lightning Liquidity

This is the compounding loop you want:

## Step 1 — Liquidity enables autonomy

* LN liquidity means agents can always pay for:

  * compute
  * skills
  * APIs
* Autonomy increases APM and throughput.

## Step 2 — Autonomy produces receipts + reputation

* More completed NIP-90 jobs, more verified outcomes.
* Receipts + NIP-32 labels improve agent creditworthiness.

## Step 3 — Credit expands effective working capital

* With NIP-AC OSCE envelopes, agents can:

  * buy resources *before* they have cash-on-hand
  * still remain bounded (scope+cap+expiry)
* This prevents “top-up bottleneck.”

## Step 4 — More work volume creates more fees

* More payments → more routing fees (LLP revenue)
* More envelopes → more credit fees (CEP revenue)
* More marketplace volume → more marketplace rake

All in BTC.

## Step 5 — Revenue grows pool size

* Pool share price increases → attracts more LP deposits
* Larger pool → more liquidity → better routing → cheaper fees → more usage

## Step 6 — Better liquidity improves marketplace quality

* Providers get paid faster and more reliably
* More providers join, prices improve
* Autopilot becomes cheaper/better → more customers

Loop repeats.

**The key:**

* credit is bounded + outcome-scoped (so risk doesn’t explode)
* liquidity improves reliability (so marketplace becomes real)
* receipts create measurable underwriting (so credit expands rationally)

---

# MVP Plan (tight)

### MVP-0 (1–2 weeks)

* LLP only: pool-run LN node + invoice pay API + receipts
* `/stats` shows:

  * total liquidity
  * routing fees
  * pay success rate
  * channel health summary

### MVP-1

* CEP issuance for **NIP-90 objective jobs only**
* pay-after-verify
* reputation throttling

### MVP-2

* CEP expands to L402 and skill invocation scopes
* add withdrawal queue + share price accounting

---

If you want, I can also produce:

* the exact receipt schemas (JSON)
* the underwriting scoring function + risk limits
* the `/stats` table layout (top 50 metrics) specific to pool health + credit health
