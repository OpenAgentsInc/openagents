# Hydra — OpenAgents Liquidity Engine (Spec)

**Hydra** is the agent-native liquidity, routing, and bounded-credit layer for the OpenAgents economy. It exists so autonomous agents can reliably **pay**, **get paid**, and **route value** at machine speed while staying **budget-bounded**, **receipt-driven**, and **replayable** under OpenAgents invariants (proto-first contracts, HTTP-only authority mutations, deterministic receipts).

Hydra is not a wallet UI and not “node ops tooling.” It is the **capital substrate** that the rest of the system (Autopilot, OpenAgents Compute, Skills, Exchange) can program against.

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
        │  - Routing Scorer/Optimizer               │
        │  - FX RFQ hook (Treasury Agents)          │
        │  - Reputation Index                       │
        │  - Receipt Emitter                        │
        └───────────────┬───────────────┬──────────┘
                        │               │
         ┌──────────────▼───┐     ┌────▼──────────────────┐
         │ Lightning Node(s) │     │ Wallet Executor        │
         │ (CLN Phase 0)     │     │ (Spark send/pay/recv)  │
         └───────────────────┘     └────────────────────────┘
```

Hydra is intentionally split across two execution authorities:

* **Lightning node backend (LLP)**: operator-managed channel liquidity and routing capacity.
* **Wallet executor (per-user / per-treasury)**: custody boundary for spend authority + canonical receipts.

Hydra orchestrates these into a single economic interface for agents.

---

## 4) Hydra Subsystems

### 4.1 LLP — Lightning Liquidity Pool

**Purpose:** Maintain inbound/outbound LN liquidity so settlement is reliable at scale, even under bursty agent traffic.

Hydra’s LLP is not “a node.” It is a **pool** with explicit accounting, risk controls, and snapshots. Its job is to keep the system alive when liquidity fragmentation would otherwise cause sporadic routing failure.

**Backed by:**

* Lightning node backend (Phase 0: CLN via JSON-RPC)
* On-chain reserve (for channel opens/rebalances)
* Optional operational rail balances (e.g., Spark balance used for treasury ops if needed)

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

* CLN backend integration for channel health snapshots
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
