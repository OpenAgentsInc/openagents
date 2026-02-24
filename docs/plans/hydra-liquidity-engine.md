# Hydra — OpenAgents Liquidity Engine (Spec)

**Hydra** is the agent-native liquidity, routing, and bounded-credit layer for the OpenAgents economy. It exists so autonomous agents can reliably **pay**, **get paid**, and **route value** at machine speed while staying **budget-bounded**, **receipt-driven**, and **replayable** under OpenAgents invariants (proto-first contracts, HTTP-only authority mutations, deterministic receipts).

Hydra is not a wallet UI and not “node ops tooling.” It is the **capital substrate** that the rest of the system (Autopilot, OpenAgents Compute, Skills, Exchange) can program against.

**Status:** Canonical draft (supersedes prior liquidity pool draft notes).

---

## 1) Goals and Non-Goals

### Goals

Hydra provides three primitives that agents can depend on:

1. **Liquidity (LLP)** — keep Lightning payment capacity available so marketplace settlement doesn’t stall.
2. **Outcome-scoped credit (CEP)** — provide bounded working capital so agents aren’t blocked on top-ups or timing gaps.
3. **Routing intelligence** — choose reliable, low-fee payment execution paths and surface deterministic, audit-friendly decisions.

Hydra must make money movement **legible to machines**: every action is quotable, policy-checked, idempotent, and produces receipts tied to trajectories/jobs/policy bundles.

### Non-goals

* **No governance token.** No emissions, no “yield promises,” no monetary policy theater.
* **No unscoped credit.** Hydra never offers open-ended lines of credit or “agent overdrafts.”
* **No opaque execution.** Hydra will not silently retry, re-route, or batch settlement without durable receipts that explain what happened.
* **No client-side custody.** Hydra integrates with the wallet-executor custody boundary; user clients consume receipts and projections.
* **No long-term lockups.** If/when external LP deposits exist, liquidity must remain withdrawable with bounded delay (subject to solvency and safety controls).

---

## 2) Product Definition

Hydra is a **service layer** that sits behind TreasuryRouter. TreasuryRouter decides *what should happen* (rail choice, budgets, approvals, risk posture). Hydra executes *how it happens* for liquidity/credit-dependent flows, and emits proof.

Hydra is designed for “agentic commerce reality”:

* Agents don’t tolerate flaky payments. A stalled invoice is a stalled workflow.
* Agents can act at high frequency; failures must be recoverable without double-spending.
* Operators need governance-by-policy, not governance-by-attention: budgets, approvals, and deterministic audit trails.

---

## 3) Architecture Overview

```text
                ┌─────────────────────────────┐
                │          Autopilot          │
                │  (Guaranteed Buyer, Runs)   │
                └────────────┬────────────────┘
                             │
                ┌────────────▼─────────────┐
                │       TreasuryRouter     │
                │ (policy + budgets + FX)  │
                └────────────┬─────────────┘
                             │   (HTTP authority commands)
        ┌────────────────────▼─────────────────────┐
        │                 Hydra                     │
        │  Liquidity + Credit + Routing + Proof     │
        │                                           │
        │  - LLP: LN Liquidity Pool                 │
        │  - CEP: Credit Envelope Pool              │
        │  - RRP: Rebalancing/Reserve Partition     │
        │  - Routing Scorer/Optimizer               │
        │  - FX RFQ hook (Treasury Agents)          │
        │  - Reputation Index                       │
        │  - Receipt Emitter                        │
        └───────────────┬───────────────┬──────────┘
                        │               │
         ┌──────────────▼───┐     ┌────▼──────────────────┐
         │ Lightning Node(s) │     │ Wallet Executor        │
         │ (LND Phase 0)     │     │ (Spark send/pay/recv)  │
         └───────────────────┘     └────────────────────────┘
```

Hydra is intentionally split across two execution authorities:

* **Lightning node backend (LLP)**: operator-managed channel liquidity and routing capacity.
* **Wallet executor (per-user / per-treasury)**: custody boundary for spend authority + canonical receipts.

Clarification:

* **Spark is for quick user wallets** (send/receive/pay via wallet-executor custody). It is not the channel/liquidity engine.
* **LLP liquidity is channel-backed and node-backed**. Hydra starts with operator-managed **LND nodes** and will later support balancing across multiple nodes (and potentially other backend types) without changing the quote/receipt surface.

Hydra orchestrates these into a single economic interface for agents.

---

## 3.1 Entities and Roles

### Entities

* **Pool**: an accounting object backed by BTC held across:
  * Lightning channel balances (local + remote reserves)
  * on-chain BTC reserves (channel opens/rebalances/safety exits)
  * optional operational balances (e.g., Spark, if explicitly enabled)
* **Partitions** (segregated ledgers): `LLP` + `CEP` + optional `RRP`.

### Roles

* **Pool Operator**: runs Lightning node(s), rebalancing, routing policy enforcement, accounting, and incident response.
* **Signer Set**: threshold keys controlling sensitive treasury actions (channel opens/closes, large withdrawals, on-chain spends).
* **Depositor (LP)**: provides BTC capital (optional early; required only once external deposits are enabled).
* **Consumers**: Autopilot + marketplace settlement flows consuming liquidity services.
* **Borrowers**: agents consuming bounded working-capital via CEP envelopes (Hydra pays providers directly under constraints).

---

## 3.2 Trust Model and Keying

Hydra is money-adjacent authority. It must preserve OpenAgents invariants:

* **Authority mutations are authenticated HTTP only** (no WS/Nostr authority mutation lanes).
* **Every effect is receipted + idempotent** (safe retries, no double spend).

Execution posture:

* **Treasury control**: threshold signing for high-impact actions; operational hot paths are bounded by caps and circuit breakers.
* **Operational hot wallet**: Lightning node operations require hot access to channel state; mitigate with:
  * channel sizing rules and peer exposure caps
  * sweeping policies and operating-balance limits
  * rate limits and circuit breakers
  * alarms + runbooks
* **Agent safety**: agents never receive free-floating pool funds. They access liquidity only via:
  * LLP quote → pay (invoice execution service)
  * CEP envelope settlement (pay-after-verify by default for objective workloads)

---

## 3.3 Accounting and LP Mode (Optional)

Hydra can run operator-funded until external LP deposits are production-safe. When LP mode is enabled:

* **Share model**: internal shares minted/burned against deposits/withdrawals; share price moves only on realized PnL/loss with explicit marking rules.
* **Segregated ledgers**: LLP/CEP/RRP partitions are tracked separately to keep risk and subsidies explicit.
  * LP exposure can be partitioned (LLP-only, CEP-only, blended) once external deposits are enabled.
* **Deposits/withdrawals**:
  * deposits via Lightning invoice (small) and on-chain address (large)
  * withdrawals via queue + scheduling semantics (T+Δ windows), dynamically throttled based on:
    * channel health/liquidity bands
    * outstanding CEP commitments
    * reserve thresholds and circuit breaker state
* **Signed snapshots**: periodic `HydraPoolSnapshot` signed by the signer set; surfaced through `/stats` and optionally mirrored via Bridge for public audit in summary form.

---

## 4) Hydra Subsystems

### 4.1 LLP — Lightning Liquidity Pool

**Purpose:** Maintain inbound/outbound LN liquidity so settlement is reliable at scale, even under bursty agent traffic.

Hydra’s LLP is not “a node.” It is a **pool** with explicit accounting, risk controls, and snapshots. Its job is to keep the system alive when liquidity fragmentation would otherwise cause sporadic routing failure.

**Backed by:**

* Lightning node backend (Phase 0: LND via REST (gRPC gateway))
* On-chain reserve (for channel opens/rebalances)
* Optional operational rail balances (e.g., Spark balances used for user-wallet custody or treasury ops if needed; not used as LLP channel liquidity)

**Core capabilities:**

* Quote and pay invoices with explicit fee caps
* Track channel health + liquidity bands
* Provide coarse liquidity snapshots for `/stats` and policy routing

**Authoritative APIs (HTTP, receipted, idempotent):**

* `POST /v1/liquidity/quote_pay`
* `POST /v1/liquidity/pay`
* `GET  /v1/liquidity/status`

**Why “quote then pay”:** agents and policies need a binding pre-flight. The quote locks the *intent* (fee ceiling, urgency, idempotency key, policy context) so retries can be safe and comparable.

---

### 4.1.1 RRP — Rebalancing / Reserve Partition (Optional)

**Purpose:** keep LLP reliable under stress without turning normal operations into “panic mode” changes.

Used for:

* emergency rebalances within explicit budgets
* fallback liquidity for high-priority settlement when channels are fragmented
* safe unwinds (channel closes/sweeps) during incidents

Funding and accounting:

* funded by a small, explicit skim from LLP/CEP fees (or operator subsidy)
* tracked as a separate partition so any subsidy is legible and policy-controlled

---

### 4.2 CEP — Credit Envelope Pool (Outcome-Scoped Credit)

**Purpose:** Give agents bounded working capital that is **scope-limited** and **verification-coupled**, so they can proceed without waiting for manual top-ups while still keeping blast radius tight.

A CEP “envelope” is not money handed to the agent. It is an authorization for Hydra to **pay on the agent’s behalf** under strict constraints:

* Scope (what this can pay for)
* Cap (max sats)
* Expiry (short-lived)
* Policy linkage (who approved and why)
* Proof linkage (what outcome justified settlement)

This is the mechanism that makes “autonomous spend” deployable in real systems: it is credit with guardrails that match machine workflows, not human credit products.

**Envelope example:**

```json
{
  "envelope_id": "env_123",
  "agent_id": "npub...",
  "scope": "nip90_sandbox_run",
  "max_sats": 50000,
  "expiry_ms": 1710000000000,
  "fee_bps": 25,
  "policy_bundle_id": "pb_2026_02_24_a"
}
```

**Defaults (hard stance):**

* Short expiries (minutes → hours)
* No rolling credit lines
* Objective verification required for auto-pay (by default)
* Reputation-weighted caps (limits rise only with proven outcomes)

---

### 4.3 Routing Intelligence (Deterministic, Agent-Consumable)

**Purpose:** Let agents and TreasuryRouter choose a payment execution strategy based on **measured reliability**, **expected fees**, and **liquidity conditions**, not vibes.

Routing intelligence is used in two places:

1. **Pre-flight scoring**: how likely is a payment to succeed under constraints?
2. **Post-flight learning**: update confidence based on real receipts and outcomes.

**API:**

* `POST /v1/routing/score`

Input:

```json
{
  "invoice": "bolt11",
  "urgency": "low|normal|high",
  "max_fee_msats": 1500
}
```

Output:

```json
{
  "expected_fee_msats": 900,
  "confidence": 0.97,
  "liquidity_score": 0.88,
  "policy_notes": ["fee_cap_ok", "peer_health_ok"]
}
```

This scorer is intentionally simple in Phase 0: it uses observable signals (channel health, historical failures, fee estimates) and never claims certainty. Its value is that it produces a *machine-readable decision* that can be logged, audited, and improved.

---

### 4.4 FX + Liquidity Routing (Treasury Agents)

**Purpose:** Allow a USD-denominated budget world (enterprise reality) to settle into sats (Lightning reality) without breaking budget guarantees.

Hydra does not try to become a full exchange by default. It provides a routing hook:

* If the agent’s available asset doesn’t match what the invoice requires, TreasuryRouter can request an RFQ path and settle via Treasury Agents.

**Flow (high level):**

1. TreasuryRouter asks for FX quote(s): `POST /v1/fx/rfq`
2. Treasury Agents respond (NIP-69 compatible order semantics as applicable)
3. TreasuryRouter selects the best quote by policy (price/latency/trust)
4. Settlement executes (v0 reputation-first → v1 atomic where supported)
5. Hydra emits an FX provenance receipt (rate source, quote id, linkage to spend)

This preserves the key OpenAgents principle: budgets remain stable in the unit humans care about, while settlement remains on the rail agents can execute on.

---

### 4.5 Reputation Index (Economic Proof, Not Social Karma)

**Purpose:** Provide a routing prior and credit underwriting prior that is grounded in **delivered outcomes**.

Hydra reputation is computed from:

* Lightning settlement success rates and latency distributions
* Envelope repayment / settlement integrity
* Objective verification pass rates (for pay-after-verify workloads)
* Delivered-vs-quoted variance (bait-and-switch detection)
* Failure spikes (risk signals)

Hydra stores this as an internal index (for routing/underwriting), and can optionally mirror summary labels via NIP-32 for interop. The point is not to build “social trust.” The point is to build **economic trust** from receipts.

---

### 4.6 Receipts (Canonical, Replayable, Non-Negotiable)

Every Hydra action emits receipts that satisfy OpenAgents receipt requirements:

* Deterministic hashing / canonical field sets
* Linkage to policy bundle, session/trajectory, job hash
* Idempotency keys so retries don’t duplicate effects

Receipt classes include:

* `hydra.deposit_receipt`
* `hydra.withdraw_receipt`
* `hydra.invoice_pay_receipt`
* `hydra.envelope_issue_receipt`
* `hydra.envelope_settlement_receipt`
* `hydra.fx_settlement_receipt`

Minimum payment receipt shape (aligned with `docs/protocol/PROTOCOL_SURFACE.md`):

```json
{
  "rail": "lightning",
  "asset_id": "BTC_LN",
  "amount_msats": 123000,
  "payment_proof": { "type": "lightning_preimage", "value": "<hex>" },
  "session_id": "<session_id>",
  "trajectory_hash": "<trajectory_hash>",
  "policy_bundle_id": "<policy_bundle_id>",
  "job_hash": "<job_hash>"
}
```

Hydra must also preserve the wallet-executor canonical receipt contract (where wallet executor performed the spend) and attach Hydra-level context (policy + routing + envelope metadata) as a higher-level receipt envelope.

---

### 4.7 `/stats` Public Observability (Minute Cache)

Hydra publishes a **public, operator-grade** set of metrics to `/stats`, cached at **60 seconds** (same value shown to all users, no per-request recomputation).

This serves three roles:

* Trust building: “the system is healthy and solvent”
* Debugging: fast diagnosis of failure modes
* Market signal: liquidity depth, reliability, cost trends

**Liquidity Health (top section):**

* Total pool sats (by rail: on-chain, LN channel totals, optional Spark)
* Channel outbound/inbound sats
* Channel count / connected channel count
* Routing success rate (5m/1h/24h)
* Median fee msats (5m/1h/24h)
* Rebalance spend (24h)
* Outstanding envelope commitments (count, sats)
* CEP loss/default rate (rolling)

**Marketplace Impact (mid section):**

* Autopilot compute spend (24h)
* Marketplace settlement volume
* FX volume (if enabled)
* Treasury Agent spread (avg bps)

**Risk + Concentration (bottom section):**

* Liquidity concentration metrics (HHI/top-share)
* Peer failure spike detector state
* Circuit breaker states (envelopes halted? withdrawals slowed?)

All metrics are emitted as tables with stable column names so agents can read `/stats` directly.

---

## 5) Trust Zones + Message Classes

Hydra obeys the OpenAgents transport doctrine:

### Class 1 — Authority mutations (must be attributable, signed, receipted, idempotent)

Examples:

* Issue an envelope
* Settle an invoice
* Finalize verification pass → release payment
* Update pool accounting / share mint/burn
* Trigger circuit breakers

**Transport:** Authenticated HTTP only.

### Class 2 — Ephemeral coordination (session-authenticated, not per-message signed)

Examples:

* Routing hints
* Health telemetry streaming
* Progress updates

**Transport:** Nexus/WS lanes allowed, but no authority changes.

Hydra is strict about this separation because it is the difference between “fast” and “fast but unsafe.”

---

## 6) Revenue Model (BTC-native, legible)

Hydra earns BTC-denominated revenue from real activity:

**LLP revenue:**

* Routing fees (where applicable)
* Explicit liquidity guarantee premium (optional, transparent)

**CEP revenue:**

* Envelope fee (bps or sats)
* Controlled risk premium (bounded by policy)

**FX routing revenue (if enabled):**

* Spread captured by Treasury Agents (not by opaque platform extraction)

Hydra must make fees explicit in quotes and receipts. “Hidden fees” are routing poison in agentic commerce.

---

## 7) Phased Build Plan

### MVP-0 — Liquidity execution baseline

* LND backend integration for channel health snapshots
* `quote_pay` + `pay` + canonical receipts
* `/stats` liquidity health tables (minute cache)

### MVP-1 — Credit envelopes for objective workloads

* CEP envelopes for **objective** pay-after-verify flows (e.g., `oa.sandbox_run.v1`)
* Envelope issuance + verification coupling
* Basic underwriting: reputation-weighted caps

### MVP-2 — Routing intelligence + richer risk controls

* Routing scorer integrated into TreasuryRouter decision path
* Circuit breakers (loss spikes, failure spikes)
* Withdrawal/availability throttles (if LP pool share model enabled)

### MVP-3 — FX RFQ integration + Treasury Agents

* RFQ ingestion + quote selection surfaces
* Settlement v0 (reputation-first) → v1 (atomic where supported)
* FX provenance receipts

### MVP-4 — Interop surfaces (Bridge/Nostr)

* Mirror **portable receipts** + **summary reputation labels**
* Keep high-rate coordination inside Nexus

---

## 8) “Why Hydra is Agent-Native”

Hydra is built for agents because:

* It treats **liquidity as an API**, not a dashboard.
* It treats **credit as outcome-scoped envelopes**, not trust-me balances.
* It treats **settlement as receipts + replay**, not “it probably paid.”
* It treats **routing decisions as logged, machine-readable artifacts** that can be optimized and audited.

This is how you get predictable autonomy at scale: the money layer behaves like an operating system subsystem, not a fintech add-on.

---

## 9) The Hydra Flywheel (Compounding Loop)

1. Autopilot buys compute and services →
2. Hydra provides reliable settlement + bounded credit →
3. Receipts improve underwriting + routing priors →
4. Agents unblock faster (higher APM, higher throughput) →
5. More volume generates more real BTC fees →
6. Pool deepens, routing gets cheaper and more reliable →
7. Autopilot becomes cheaper/better →
8. More users → repeat

Hydra is “liquidity-first” infrastructure: it makes the entire market feel real.

---

## 10) API Surface Summary (Authoritative)

**Liquidity**

* `POST /v1/liquidity/quote_pay`
* `POST /v1/liquidity/pay`
* `GET  /v1/liquidity/status`

**Credit envelopes**

* `POST /v1/credit/intent`
* `POST /v1/credit/offer`
* `POST /v1/credit/envelope`
* `POST /v1/credit/settle`

**Routing intelligence**

* `POST /v1/routing/score`

**FX (optional)**

* `POST /v1/fx/rfq`
* `POST /v1/fx/settle`

**Observability**

* `GET /stats` (minute cache)
* `GET /v1/hydra/receipts` (filtered by type/time/agent)

---

```proto
// proto/openagents/hydra/v1/hydra.proto
syntax = "proto3";

package openagents.hydra.v1;

import "google/protobuf/struct.proto";
import "google/protobuf/timestamp.proto";
import "openagents/protocol/v1/reasons.proto";
import "openagents/lightning/v1/wallet_executor.proto";

// Hydra — OpenAgents Liquidity Engine (v1)
//
// This file defines proto-first, portable contracts for Hydra’s authority surfaces:
//
// - LLP: Lightning Liquidity Pool (quote/pay + liquidity status + pool snapshots)
// - CEP: Outcome-scoped Credit Envelopes (intent/offer/issue/settle)
// - Routing intelligence (score invoice payment feasibility)
// - Optional FX RFQ/settlement scaffolding (for Treasury Agents)
//
// Norms:
// - Authority mutations MUST remain authenticated HTTP only (INV-02).
// - Live delivery lanes (Khala) are WS-only and non-authoritative.
// - All mutation flows MUST be idempotent.
// - Receipts MUST be deterministic and replay-verifiable.
// - v1 is additive-only (ADR-0002); do not change semantics of existing fields.
//
// Canonical hashing/signature pipelines are defined outside this proto and referenced via
// `canonical_json_sha256` fields where applicable.

//
// -----------------------------
// Common enums + primitives
// -----------------------------

enum HydraErrorCode {
  HYDRA_ERROR_CODE_UNSPECIFIED = 0;

  HYDRA_ERROR_CODE_UNAUTHORIZED = 1;
  HYDRA_ERROR_CODE_FORBIDDEN = 2;
  HYDRA_ERROR_CODE_INVALID_REQUEST = 3;
  HYDRA_ERROR_CODE_NOT_FOUND = 4;
  HYDRA_ERROR_CODE_CONFLICT = 5;
  HYDRA_ERROR_CODE_RATE_LIMITED = 6;

  // Domain errors (Hydra-specific)
  HYDRA_ERROR_CODE_QUOTE_EXPIRED = 10;
  HYDRA_ERROR_CODE_INSUFFICIENT_LIQUIDITY = 11;
  HYDRA_ERROR_CODE_BUDGET_EXHAUSTED = 12;
  HYDRA_ERROR_CODE_POLICY_DENIED = 13;

  // Dependency / downstream failures
  HYDRA_ERROR_CODE_WALLET_EXECUTOR_UNAVAILABLE = 20;
  HYDRA_ERROR_CODE_WALLET_EXECUTOR_REJECTED = 21;
  HYDRA_ERROR_CODE_LIGHTNING_BACKEND_UNAVAILABLE = 22;
  HYDRA_ERROR_CODE_LIGHTNING_PAYMENT_FAILED = 23;

  HYDRA_ERROR_CODE_INTERNAL_ERROR = 100;
}

message HydraError {
  HydraErrorCode code = 1;
  string message = 2;
  repeated string details = 3;
  uint32 retry_after_ms = 4;

  // Optional mapping to canonical policy/runtime reason taxonomy.
  openagents.protocol.v1.ReasonCode reason_code = 10;
  string reason_code_text = 11;

  // Request correlation.
  string request_id = 20;
}

enum HydraUrgency {
  HYDRA_URGENCY_UNSPECIFIED = 0;
  HYDRA_URGENCY_LOW = 1;
  HYDRA_URGENCY_NORMAL = 2;
  HYDRA_URGENCY_HIGH = 3;
}

// Generic money container.
// For Lightning flows, set currency="msats" and amount=<msats>.
// For pool accounting, set currency="sats" and amount=<sats>.
message HydraMoney {
  string currency = 1; // "msats" | "sats" | "usd_cents" | ...
  uint64 amount = 2;
}

// Stable linkage fields used throughout OpenAgents for auditability.
// These are pointers to authoritative artifacts/state elsewhere.
//
// Notes:
// - `trajectory_hash`, `job_hash`, `objective_hash` should be deterministic sha256:<hex> strings.
// - These fields are essential for “money → why → proof” legibility.
message HydraLinkage {
  string session_id = 1;
  string run_id = 2;
  string trajectory_hash = 3;

  // Deterministic compute/job hash from job registry.
  string job_hash = 4;

  // Deterministic objective hash (e.g., job request canonical hash).
  string objective_hash = 5;

  // Policy bundle ID for the compiled policy/routing configuration used.
  string policy_bundle_id = 6;

  // Optional human/admin correlation hints (non-contract-critical).
  google.protobuf.Struct metadata = 20;
}

// Budget scope is intentionally explicit: Hydra is used by fleet operators
// who need org/repo/issue-level spend isolation.
message HydraBudgetScope {
  string org_id = 1;
  string project_id = 2;
  string repo_id = 3;
  string issue_id = 4;
}

// Fee component decomposition to make quotes and receipts machine-comparable.
// (Mirrors the pattern used in marketplace commerce grammar.)
message HydraFeeComponent {
  string component = 1;        // "provider" | "operator_fee" | "policy_adder" | ...
  HydraMoney amount = 2;       // usually msats for Lightning settlement
  string notes = 3;            // optional human notes (non-critical)
}

// Canonical receipt linkage for money movement.
// WalletExecutorReceipt is the canonical settlement proof for Lightning payments.
// Hydra receipts wrap it with policy/linkage context and additional deterministic fields.
message HydraTreasuryReceiptRef {
  string receipt_sha256 = 1;   // sha256:<hex> of canonical JSON view (Hydra-level)
  string receipt_url = 2;      // optional URL in object storage
  string receipt_id = 3;       // optional stable id derived from sha prefix (e.g., hydra_<...>)
}

//
// -----------------------------
// LLP: Lightning Liquidity Pool
// -----------------------------

enum HydraLightningBackend {
  HYDRA_LIGHTNING_BACKEND_UNSPECIFIED = 0;
  HYDRA_LIGHTNING_BACKEND_NOOP = 1;
  HYDRA_LIGHTNING_BACKEND_LND = 2; // Phase 0
  HYDRA_LIGHTNING_BACKEND_LDK = 3; // Later
}

// Coarse liquidity health summary used for routing and /stats.
message HydraLightningLiquiditySummary {
  HydraLightningBackend backend = 1;

  // Sats-level totals (coarse, for health/monitoring; not spend authority).
  uint64 channel_total_sats = 10;
  uint64 channel_outbound_sats = 11;
  uint64 channel_inbound_sats = 12;

  uint32 channel_count = 20;
  uint32 connected_channel_count = 21;

  // Optional last error observed when querying backend.
  string last_error = 30;
  int64 observed_at_ms = 31;
}

// A pay-quote is a binding pre-flight intent used to ensure retry safety.
// Quote IDs are server-generated and stable; quote expiry is authoritative.
enum HydraPayQuoteStatus {
  HYDRA_PAY_QUOTE_STATUS_UNSPECIFIED = 0;
  HYDRA_PAY_QUOTE_STATUS_CREATED = 1;
  HYDRA_PAY_QUOTE_STATUS_USED = 2;
  HYDRA_PAY_QUOTE_STATUS_EXPIRED = 3;
  HYDRA_PAY_QUOTE_STATUS_CANCELED = 4;
}

message HydraLiquidityPayQuoteRequest {
  // Correlation.
  string request_id = 1;

  // Required idempotency key for authority mutation application.
  // Multiple identical requests with same idempotency_key MUST return the same quote.
  string idempotency_key = 2;

  // Which pool/partition to route through (default "default").
  string pool_id = 3;

  // BOLT11 invoice to pay.
  string bolt11 = 4;

  // Fee ceiling (msats).
  uint64 max_fee_msats = 5;

  HydraUrgency urgency = 6;

  // Optional “all-in” ceiling (invoice + fees). If set, Hydra MUST not exceed it.
  optional uint64 max_total_msats = 7;

  // Optional linkage for receipts and audit.
  HydraLinkage linkage = 10;
  HydraBudgetScope budget_scope = 11;

  // Optional policy context (non-contract-critical hints).
  google.protobuf.Struct policy_context = 20;
}

message HydraLiquidityPayQuote {
  string quote_id = 1;
  string pool_id = 2;

  // Echo of request intent.
  string bolt11 = 3;
  uint64 max_fee_msats = 4;
  HydraUrgency urgency = 5;
  optional uint64 max_total_msats = 6;

  // Parsed invoice fields.
  string invoice_hash = 10;
  uint64 quoted_amount_msats = 11;

  // Estimates (best-effort, non-binding except as constrained by max_fee/max_total).
  uint64 expected_fee_msats = 20;
  uint64 expected_total_msats = 21;

  // Scoring outputs for routing decisions (best-effort).
  double confidence = 30;       // 0..1
  double liquidity_score = 31;  // 0..1

  HydraPayQuoteStatus status = 40;

  int64 created_at_ms = 50;
  int64 valid_until_ms = 51;

  // Deterministic hash pointer for the canonical JSON view of the quote (if enforced).
  string canonical_json_sha256 = 60;

  // Optional linkage captured at quote creation.
  HydraLinkage linkage = 70;
  HydraBudgetScope budget_scope = 71;

  google.protobuf.Struct metadata = 90;
}

message HydraLiquidityPayQuoteResult {
  bool ok = 1;
  bool idempotent_replay = 2;
  HydraLiquidityPayQuote quote = 3;
  HydraError error = 4;
}

// Commit executes the payment. This is an authority mutation.
// It MUST be idempotent on idempotency_key.
message HydraLiquidityPayCommitRequest {
  string request_id = 1;
  string idempotency_key = 2;

  string quote_id = 3;

  // Optional override: allow smaller fee ceiling for this commit (tighten only).
  optional uint64 max_fee_msats_override = 4;

  // Optional linkage (if not already embedded in quote).
  HydraLinkage linkage = 10;
  HydraBudgetScope budget_scope = 11;

  google.protobuf.Struct policy_context = 20;
}

// Hydra-level payment receipt that wraps wallet-executor canonical receipt and adds
// policy/linkage context. This is the durable “what paid, why, under what constraints” artifact.
message HydraLiquidityPayReceipt {
  string receipt_version = 1;      // e.g. "openagents.hydra.liquidity_pay_receipt.v1"
  string receipt_id = 2;           // derived from canonical_json_sha256 prefix (recommended)
  string request_id = 3;

  string pool_id = 4;
  string quote_id = 5;

  string host = 10;                // lowercased invoice destination host (best-effort)
  string invoice_hash = 11;

  uint64 quoted_amount_msats = 20;
  uint64 settled_amount_msats = 21;
  uint64 fee_paid_msats = 22;
  uint64 total_paid_msats = 23;

  int64 paid_at_ms = 30;

  // Canonical Lightning settlement proof (required when Lightning is used).
  openagents.lightning.v1.WalletExecutionReceipt wallet_receipt = 40;

  // Linkages for auditability.
  HydraLinkage linkage = 50;
  HydraBudgetScope budget_scope = 51;

  // Optional fee decomposition (if known/meaningful).
  repeated HydraFeeComponent fees = 60;

  // Deterministic canonical hash pointer for this receipt.
  string canonical_json_sha256 = 70;
}

message HydraLiquidityPayCommitResult {
  bool ok = 1;
  bool idempotent_replay = 2;

  HydraLiquidityPayReceipt receipt = 3;

  // Optional pointer to an externalized receipt object (for large receipts).
  HydraTreasuryReceiptRef receipt_ref = 4;

  HydraError error = 5;
}

message HydraLiquidityStatusRequest {
  string request_id = 1;
  string pool_id = 2;
}

message HydraCircuitBreakerState {
  string breaker_id = 1;
  string kind = 2; // stable string key, e.g. "envelopes_halt_new", "withdrawals_throttled"
  bool active = 3;

  openagents.protocol.v1.ReasonCode reason_code = 4;
  string reason = 5;

  int64 activated_at_ms = 6;
  int64 updated_at_ms = 7;

  google.protobuf.Struct metadata = 20;
}

message HydraLiquidityStatus {
  string pool_id = 1;

  HydraLightningLiquiditySummary lightning = 10;

  // Aggregates (msats) for credit exposure.
  uint64 outstanding_envelopes_msats = 20;
  uint32 outstanding_envelope_count = 21;

  // Rolling health.
  double routing_success_rate_5m = 30;   // 0..1
  double routing_success_rate_1h = 31;   // 0..1
  double routing_success_rate_24h = 32;  // 0..1

  uint64 median_fee_msats_5m = 40;
  uint64 median_fee_msats_1h = 41;
  uint64 median_fee_msats_24h = 42;

  // Circuit breakers that affect Hydra behavior.
  repeated HydraCircuitBreakerState circuit_breakers = 50;

  int64 observed_at_ms = 60;
  google.protobuf.Struct metadata = 90;
}

message HydraLiquidityStatusResult {
  bool ok = 1;
  HydraLiquidityStatus status = 2;
  HydraError error = 3;
}

//
// -----------------------------
// CEP: Outcome-scoped Credit Envelopes
// -----------------------------

enum HydraEnvelopeScope {
  HYDRA_ENVELOPE_SCOPE_UNSPECIFIED = 0;

  // Objective verification flows (default for CEP bootstrap).
  HYDRA_ENVELOPE_SCOPE_NIP90_OBJECTIVE_JOB = 1;

  // Future scopes (enabled later).
  HYDRA_ENVELOPE_SCOPE_L402_CALL = 2;
  HYDRA_ENVELOPE_SCOPE_SKILL_INVOCATION = 3;
  HYDRA_ENVELOPE_SCOPE_FX_SWAP = 4;

  HYDRA_ENVELOPE_SCOPE_CUSTOM = 100;
}

enum HydraEnvelopeStatus {
  HYDRA_ENVELOPE_STATUS_UNSPECIFIED = 0;
  HYDRA_ENVELOPE_STATUS_INTENT_CREATED = 1;
  HYDRA_ENVELOPE_STATUS_OFFERED = 2;
  HYDRA_ENVELOPE_STATUS_ISSUED = 3;
  HYDRA_ENVELOPE_STATUS_SETTLED = 4;
  HYDRA_ENVELOPE_STATUS_EXPIRED = 5;
  HYDRA_ENVELOPE_STATUS_CANCELED = 6;
  HYDRA_ENVELOPE_STATUS_DEFAULTED = 7;
}

message HydraCreditIntentRequest {
  string request_id = 1;
  string idempotency_key = 2;

  string pool_id = 3;

  // Agent principal requesting envelope underwriting.
  string agent_id = 4;

  HydraEnvelopeScope scope = 5;

  // Outcome scoping (required for objective jobs; optional otherwise).
  string objective_hash = 6;
  string job_hash = 7;

  // Maximum exposure requested (msats).
  uint64 requested_max_msats = 8;

  // Desired expiry window. Hydra may shorten it by policy.
  int64 desired_expires_at_ms = 9;

  HydraLinkage linkage = 10;
  HydraBudgetScope budget_scope = 11;

  google.protobuf.Struct policy_context = 20;
}

message HydraCreditIntent {
  string intent_id = 1;
  string pool_id = 2;

  string agent_id = 3;
  HydraEnvelopeScope scope = 4;

  string objective_hash = 5;
  string job_hash = 6;

  uint64 requested_max_msats = 10;
  int64 desired_expires_at_ms = 11;

  HydraEnvelopeStatus status = 20;

  int64 created_at_ms = 30;
  int64 updated_at_ms = 31;

  HydraLinkage linkage = 40;
  HydraBudgetScope budget_scope = 41;

  google.protobuf.Struct metadata = 90;
}

message HydraCreditIntentResult {
  bool ok = 1;
  bool idempotent_replay = 2;
  HydraCreditIntent intent = 3;
  HydraError error = 4;
}

// Offer is Hydra’s underwriting response.
// It is a bounded, short-lived set of terms, not a rolling line.
message HydraCreditOfferRequest {
  string request_id = 1;
  string idempotency_key = 2;

  string intent_id = 3;

  // Optional acceptance hints (non-binding; can be used for negotiation later).
  google.protobuf.Struct preferences = 10;
}

message HydraCreditOffer {
  string offer_id = 1;
  string intent_id = 2;
  string pool_id = 3;

  string agent_id = 4;
  HydraEnvelopeScope scope = 5;

  // Approved cap (msats) and fee schedule.
  uint64 approved_max_msats = 10;

  // Fee in basis points charged on settlement (policy-defined).
  uint32 fee_bps = 11;

  // Optional risk score for observability/debugging (0..1).
  double risk_score = 12;

  // Binding window for accepting this offer.
  int64 valid_until_ms = 20;

  // Expiry for the envelope that would be issued from this offer.
  int64 envelope_expires_at_ms = 21;

  // Optional fee breakdown for agent legibility.
  repeated HydraFeeComponent fees = 30;

  HydraEnvelopeStatus status = 40; // typically OFFERED

  int64 created_at_ms = 50;

  string canonical_json_sha256 = 60;

  HydraLinkage linkage = 70;
  HydraBudgetScope budget_scope = 71;

  google.protobuf.Struct metadata = 90;
}

message HydraCreditOfferResult {
  bool ok = 1;
  bool idempotent_replay = 2;
  HydraCreditOffer offer = 3;
  HydraError error = 4;
}

// Envelope issuance is the moment Hydra commits working-capital availability.
// This is an authority mutation and MUST be idempotent.
message HydraCreditEnvelopeIssueRequest {
  string request_id = 1;
  string idempotency_key = 2;

  string offer_id = 3;

  // Explicit acceptance (proof-of-accept) fields can be added later.
  google.protobuf.Struct acceptance = 10;
}

message HydraCreditEnvelope {
  string envelope_id = 1;
  string offer_id = 2;
  string intent_id = 3;
  string pool_id = 4;

  string agent_id = 5;
  HydraEnvelopeScope scope = 6;

  string objective_hash = 7;
  string job_hash = 8;

  uint64 max_msats = 10;
  uint32 fee_bps = 11;

  HydraEnvelopeStatus status = 20;

  int64 issued_at_ms = 30;
  int64 expires_at_ms = 31;
  int64 updated_at_ms = 32;

  HydraLinkage linkage = 40;
  HydraBudgetScope budget_scope = 41;

  string canonical_json_sha256 = 60;

  google.protobuf.Struct metadata = 90;
}

message HydraCreditEnvelopeResult {
  bool ok = 1;
  bool idempotent_replay = 2;
  HydraCreditEnvelope envelope = 3;
  HydraError error = 4;
}

// Settlement consumes an envelope to pay a Lightning invoice after verification.
// This is the heart of pay-after-verify for objective jobs.
message HydraCreditEnvelopeSettleRequest {
  string request_id = 1;
  string idempotency_key = 2;

  string envelope_id = 3;

  // Provider invoice to pay (typically included in a compute job result).
  string bolt11 = 4;

  // Verification evidence pointers (portable).
  string verification_receipt_sha256 = 10; // sha256:<hex>
  string verification_receipt_url = 11;

  // Optional “delivered vs quoted” integrity measures (when quote exists).
  optional uint64 quoted_total_msats = 20;

  // Optional linkage (if not already embedded in envelope).
  HydraLinkage linkage = 30;
  HydraBudgetScope budget_scope = 31;

  google.protobuf.Struct policy_context = 40;
}

message HydraCreditEnvelopeSettlementReceipt {
  string receipt_version = 1; // e.g. "openagents.hydra.envelope_settlement_receipt.v1"
  string receipt_id = 2;
  string request_id = 3;

  string pool_id = 4;
  string envelope_id = 5;

  string invoice_hash = 10;
  uint64 settled_amount_msats = 11;
  uint64 fee_paid_msats = 12;
  uint64 total_paid_msats = 13;

  int64 settled_at_ms = 20;

  // Canonical Lightning settlement proof.
  openagents.lightning.v1.WalletExecutionReceipt wallet_receipt = 30;

  // Verification evidence pointers (portable).
  string verification_receipt_sha256 = 40;
  string verification_receipt_url = 41;

  // Policy + linkage.
  HydraLinkage linkage = 50;
  HydraBudgetScope budget_scope = 51;

  repeated HydraFeeComponent fees = 60;
  string canonical_json_sha256 = 70;
}

message HydraCreditEnvelopeSettleResult {
  bool ok = 1;
  bool idempotent_replay = 2;

  HydraCreditEnvelopeSettlementReceipt receipt = 3;
  HydraTreasuryReceiptRef receipt_ref = 4;

  HydraError error = 5;
}

//
// -----------------------------
// Routing intelligence
// -----------------------------

message HydraRoutingScoreRequest {
  string request_id = 1;

  string pool_id = 2;
  string bolt11 = 3;

  uint64 max_fee_msats = 4;
  HydraUrgency urgency = 5;

  // Optional linkage (for audit and offline model training later).
  HydraLinkage linkage = 10;

  google.protobuf.Struct policy_context = 20;
}

message HydraRoutingScore {
  string pool_id = 1;
  string invoice_hash = 2;

  uint64 max_fee_msats = 3;

  // Best-effort estimates for routing decision-making.
  uint64 expected_fee_msats = 10;
  uint64 expected_total_msats = 11;

  double confidence = 20;       // 0..1
  double liquidity_score = 21;  // 0..1

  repeated string policy_notes = 30;

  int64 scored_at_ms = 40;

  google.protobuf.Struct metadata = 90;
}

message HydraRoutingScoreResult {
  bool ok = 1;
  HydraRoutingScore score = 2;
  HydraError error = 3;
}

//
// -----------------------------
// Optional: FX RFQ scaffolding (Treasury Agents)
// -----------------------------

message HydraFxRfqRequest {
  string request_id = 1;
  string idempotency_key = 2;

  string rfq_id = 3;
  string buyer_id = 4;

  // Sell and desired buy constraints.
  HydraMoney sell = 10;
  string buy_currency = 11;        // e.g. "msats" for BTC_LN, or "usd_cents"
  optional uint64 min_buy_amount = 12;

  int64 valid_until_ms = 20;

  HydraLinkage linkage = 30;
  HydraBudgetScope budget_scope = 31;

  google.protobuf.Struct constraints = 40;
}

message HydraFxQuote {
  string quote_id = 1;
  string rfq_id = 2;

  string maker_id = 3;

  HydraMoney sell = 10;
  HydraMoney buy = 11;

  repeated HydraFeeComponent fees = 20;
  HydraMoney total_cost = 21;

  int64 valid_until_ms = 30;

  double confidence = 40; // maker confidence (optional)
  google.protobuf.Struct metadata = 90;
}

message HydraFxRfqResult {
  bool ok = 1;
  bool idempotent_replay = 2;

  string rfq_id = 3;
  repeated HydraFxQuote quotes = 4;

  HydraError error = 5;
}

message HydraFxSettleRequest {
  string request_id = 1;
  string idempotency_key = 2;

  string quote_id = 3;

  HydraLinkage linkage = 10;
  HydraBudgetScope budget_scope = 11;

  google.protobuf.Struct policy_context = 20;
}

message HydraFxSettlementReceipt {
  string receipt_version = 1; // "openagents.hydra.fx_settlement_receipt.v1"
  string receipt_id = 2;
  string request_id = 3;

  string quote_id = 4;
  string rfq_id = 5;

  HydraMoney sell = 10;
  HydraMoney buy = 11;

  repeated HydraFeeComponent fees = 20;

  // Optional linkage to underlying Lightning spend receipts (when applicable).
  optional openagents.lightning.v1.WalletExecutionReceipt wallet_receipt = 30;

  int64 settled_at_ms = 40;

  HydraLinkage linkage = 50;
  HydraBudgetScope budget_scope = 51;

  string canonical_json_sha256 = 60;
}

message HydraFxSettleResult {
  bool ok = 1;
  bool idempotent_replay = 2;

  HydraFxSettlementReceipt receipt = 3;
  HydraTreasuryReceiptRef receipt_ref = 4;

  HydraError error = 5;
}

//
// -----------------------------
// Pool accounting: snapshots + deposit/withdraw scaffolding
// -----------------------------

enum HydraDepositMethod {
  HYDRA_DEPOSIT_METHOD_UNSPECIFIED = 0;
  HYDRA_DEPOSIT_METHOD_LIGHTNING_INVOICE = 1;
  HYDRA_DEPOSIT_METHOD_ONCHAIN_ADDRESS = 2;
  HYDRA_DEPOSIT_METHOD_SPARK_TRANSFER = 3; // optional
}

message HydraPoolAssets {
  // Sats-based accounting.
  uint64 onchain_sats = 1;
  uint64 lightning_channel_sats = 2;
  uint64 spark_sats = 3;

  // Coarse breakdown.
  uint64 lightning_outbound_sats = 10;
  uint64 lightning_inbound_sats = 11;
}

message HydraPoolLiabilities {
  // Optional LP share/liability model (may be 0 in early phases).
  uint64 lp_shares_outstanding = 1;

  // Pending withdrawals and commitments.
  uint64 pending_withdraw_sats = 10;
  uint64 reserved_envelope_msats = 11;
}

message HydraPoolSnapshot {
  string snapshot_id = 1;
  string pool_id = 2;

  HydraPoolAssets assets = 10;
  HydraPoolLiabilities liabilities = 11;

  HydraLightningLiquiditySummary lightning = 12;

  int64 observed_at_ms = 20;

  // Deterministic canonical hash pointer for signed snapshots.
  string canonical_json_sha256 = 30;

  // Optional signature references (actual signature scheme managed elsewhere).
  repeated string signer_fingerprints = 40;

  google.protobuf.Struct metadata = 90;
}

message HydraPoolSnapshotResult {
  bool ok = 1;
  HydraPoolSnapshot snapshot = 2;
  HydraError error = 3;
}

message HydraPoolDepositQuoteRequest {
  string request_id = 1;
  string idempotency_key = 2;

  string pool_id = 3;
  HydraDepositMethod method = 4;

  // Desired deposit amount in sats (recommended) or msats (optional).
  uint64 amount_sats = 10;
  optional uint64 amount_msats = 11;

  // Optional linkage for audit.
  HydraLinkage linkage = 20;
}

message HydraPoolDepositQuote {
  string deposit_quote_id = 1;
  string pool_id = 2;

  HydraDepositMethod method = 3;

  uint64 amount_sats = 10;
  optional uint64 amount_msats = 11;

  // Method-specific payment target.
  optional string bolt11 = 20;
  optional string onchain_address = 21;

  int64 valid_until_ms = 30;
  int64 created_at_ms = 31;

  // Optional LP share model fields (may be zero/unused initially).
  optional uint64 share_price_sats = 40;
  optional uint64 shares_to_mint = 41;

  string canonical_json_sha256 = 50;

  HydraLinkage linkage = 60;

  google.protobuf.Struct metadata = 90;
}

message HydraPoolDepositQuoteResult {
  bool ok = 1;
  bool idempotent_replay = 2;
  HydraPoolDepositQuote quote = 3;
  HydraError error = 4;
}

message HydraPoolDepositReceipt {
  string receipt_version = 1; // "openagents.hydra.deposit_receipt.v1"
  string receipt_id = 2;
  string request_id = 3;

  string pool_id = 4;
  string deposit_quote_id = 5;

  uint64 amount_sats = 10;

  // Optional LP share mint fields.
  optional uint64 share_price_sats = 20;
  optional uint64 shares_minted = 21;

  // Settlement proof references:
  // - For Lightning deposits, wallet_receipt is present.
  // - For onchain deposits, txid_ref is present.
  optional openagents.lightning.v1.WalletExecutionReceipt wallet_receipt = 30;
  optional string onchain_txid = 31;

  int64 settled_at_ms = 40;

  HydraLinkage linkage = 50;

  string canonical_json_sha256 = 60;
}

message HydraPoolDepositReceiptResult {
  bool ok = 1;
  bool idempotent_replay = 2;
  HydraPoolDepositReceipt receipt = 3;
  HydraTreasuryReceiptRef receipt_ref = 4;
  HydraError error = 5;
}

enum HydraWithdrawalStatus {
  HYDRA_WITHDRAWAL_STATUS_UNSPECIFIED = 0;
  HYDRA_WITHDRAWAL_STATUS_PENDING = 1;
  HYDRA_WITHDRAWAL_STATUS_SCHEDULED = 2;
  HYDRA_WITHDRAWAL_STATUS_SETTLED = 3;
  HYDRA_WITHDRAWAL_STATUS_FAILED = 4;
  HYDRA_WITHDRAWAL_STATUS_CANCELED = 5;
}

message HydraPoolWithdrawRequest {
  string request_id = 1;
  string idempotency_key = 2;

  string pool_id = 3;

  // Either specify shares or sats. Oneof to prevent ambiguity.
  oneof amount {
    uint64 withdraw_sats = 10;
    uint64 withdraw_shares = 11;
  }

  // Destination (either Lightning payout or on-chain).
  oneof destination {
    string destination_bolt11 = 20;
    string destination_onchain_address = 21;
  }

  HydraLinkage linkage = 30;

  google.protobuf.Struct policy_context = 40;
}

message HydraPoolWithdrawRecord {
  string withdraw_id = 1;
  string pool_id = 2;

  HydraWithdrawalStatus status = 3;

  oneof amount {
    uint64 withdraw_sats = 10;
    uint64 withdraw_shares = 11;
  }

  oneof destination {
    string destination_bolt11 = 20;
    string destination_onchain_address = 21;
  }

  // Scheduling controls (queue semantics / T+Δ windows).
  optional int64 earliest_settle_at_ms = 30;
  optional int64 scheduled_settle_at_ms = 31;

  int64 created_at_ms = 40;
  int64 updated_at_ms = 41;

  google.protobuf.Struct metadata = 90;
}

message HydraPoolWithdrawRequestResult {
  bool ok = 1;
  bool idempotent_replay = 2;
  HydraPoolWithdrawRecord withdraw = 3;
  HydraError error = 4;
}

message HydraPoolWithdrawReceipt {
  string receipt_version = 1; // "openagents.hydra.withdraw_receipt.v1"
  string receipt_id = 2;
  string request_id = 3;

  string pool_id = 4;
  string withdraw_id = 5;

  uint64 settled_sats = 10;

  // Settlement proofs (one of these should be present depending on rail).
  optional openagents.lightning.v1.WalletExecutionReceipt wallet_receipt = 20;
  optional string onchain_txid = 21;

  int64 settled_at_ms = 30;

  HydraLinkage linkage = 40;

  string canonical_json_sha256 = 50;
}

message HydraPoolWithdrawReceiptResult {
  bool ok = 1;
  bool idempotent_replay = 2;
  HydraPoolWithdrawReceipt receipt = 3;
  HydraTreasuryReceiptRef receipt_ref = 4;
  HydraError error = 5;
}

//
// -----------------------------
// Unified result envelope (optional convenience)
// -----------------------------
//
// Hydra APIs can return one result envelope type over HTTP to simplify client parsing,
// while still preserving typed payloads and deterministic receipts.

message HydraResultEnvelope {
  bool ok = 1;
  HydraError error = 2;

  oneof result {
    HydraLiquidityPayQuoteResult liquidity_quote = 10;
    HydraLiquidityPayCommitResult liquidity_pay = 11;
    HydraLiquidityStatusResult liquidity_status = 12;

    HydraCreditIntentResult credit_intent = 20;
    HydraCreditOfferResult credit_offer = 21;
    HydraCreditEnvelopeResult credit_envelope = 22;
    HydraCreditEnvelopeSettleResult credit_settle = 23;

    HydraRoutingScoreResult routing_score = 30;

    HydraFxRfqResult fx_rfq = 40;
    HydraFxSettleResult fx_settle = 41;

    HydraPoolSnapshotResult pool_snapshot = 50;
    HydraPoolDepositQuoteResult pool_deposit_quote = 51;
    HydraPoolDepositReceiptResult pool_deposit_receipt = 52;
    HydraPoolWithdrawRequestResult pool_withdraw_request = 53;
    HydraPoolWithdrawReceiptResult pool_withdraw_receipt = 54;
  }
}
```
