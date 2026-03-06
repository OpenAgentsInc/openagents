# Spec: Hydra X — Agent Liquidity Router and Solver Network

**Status:** Canonical draft (supersedes “bridge notes” and consolidates solver + routing strategy)

Hydra X extends **Hydra (OpenAgents Liquidity Engine)** into a unified economic substrate that supports:

1. **Lightning-first settlement** for marketplace flows (MVP now).
2. **Liquidity solver jobs** where solver providers compete to fulfill value-routing intents (Medium term).
3. **Agent liquidity routing** across multiple rails (LN, on-chain BTC, EVM, and other supported chains) through deterministic receipts and strict policy budgets (Long term).

Hydra X does **not** become a wallet UI, exchange UI, or speculative bridge app. It becomes a **programmable liquidity router** and **market for solvers** that can be invoked by Autopilot, Skills, and the marketplace.

Hydra X’s north star: **agents as market makers**—reliable, policy-bounded, receipt-driven, and replayable.

---

## 0) Guiding Principles and Invariants

Hydra X inherits Hydra invariants and adds a few that matter for cross-rail routing.

### 0.1 Core invariants (unchanged)

* **Proto-first contracts.**
* **HTTP-only authority mutations.** No WS/Nostr path can mutate money state.
* **Deterministic receipts** for every effect.
* **Idempotent execution** (safe retries, no double-settle).
* **Budget-bounded** by policy bundles; never “best effort” hidden retries.
* **No unscoped credit.** Only outcome-scoped envelopes (CEP).
* **No token requirements.** No governance-token dependence.

### 0.2 New invariants (added)

* **Intent separation:** *routing intents* are messages; *settlement effects* are authoritative actions.
* **Rail adapters are pure:** each adapter must yield canonical receipts; any “helper” relayer calls must be receipted with provenance.
* **Solver accountability:** solver performance is measured from receipts (fills, latency, failures, slippage).
* **Failure is explicit:** refunds/timeouts are first-class states in the state machine, not “oops”.

---

## 1) Product Definition

Hydra X is a service layer that sits behind TreasuryRouter and provides:

### (A) Lightning Liquidity Pool (LLP)

Keep LN capacity available and reliably pay/receive for marketplace settlement.

### (B) Credit Envelope Pool (CEP)

Provide bounded working capital (outcome-scoped credit) for agents and for solver operations when explicitly allowed.

### (C) Routing Intelligence

A deterministic scorer that selects rails and solver strategies based on reliability, cost, and policy constraints.

### (D) Solver Market

A protocol + runtime that allows solver providers (“solvers”) to compete to fulfill **liquidity intents**, earning BTC-denominated fees/spreads.

### (E) Cross-Rail Adapters

A set of rail adapters that let Hydra satisfy intents across LN, on-chain BTC, EVM, and other chains (phase-gated). Adapters are supported for interoperability, but roadmap-critical routes must not depend on third-party solver network openness.

Hydra X is the economic complement to your compute marketplace: **compute intents + liquidity intents** become the two legs of an agent economy.

---

## External Liquidity Networks: Current Posture

- Garden provides a useful reference architecture (`intent -> match -> initiate -> redeem`).
- As of 2026-03-04, Garden's solver network/orderbook is **not open** to external fillers; Hydra should assume **no access** for roadmap planning.
- Therefore, OpenAgents must build:
  - OpenAgents-native matching + solver provider role,
  - internal liquidity routing jobs,
  - native adapters (LN, BTC on-chain, and eventually cross-chain) under Hydra invariants.
- External liquidity adapters remain optional future interop, gated by policy, and are not required for roadmap success.

Rule: **No roadmap item depends on Garden openness.**

---

## 2) Entities, Roles, and Trust Model

### 2.1 Entities

* **Intent**: request to route value from asset/rail A to asset/rail B under constraints.
* **Route Plan**: chosen rail path + solver selection + expected fees.
* **Order**: a matched commitment between an intent maker and a solver filler (where applicable).
* **Swap/Settlement State Machine**: canonical state transitions with refunds.
* **Receipt**: canonical evidence artifact for every state transition.

### 2.2 Roles

* **Intent Maker**: Autopilot, a Skill, TreasuryRouter, or an agent that needs value routed.
* **Solver Provider**: agent operator that supplies liquidity/capital and executes fills.
* **Hydra Operator**: runs LN nodes / on-chain reserves / risk systems.
* **Signer Set**: threshold control for high-risk treasury actions.
* **Treasury Agents**: FX counterparties in RFQ mode (existing Hydra FX design).

### 2.3 Trust boundaries

* Hydra authority actions remain **authenticated HTTP only**.
* Solvers are untrusted counterparties; Hydra mitigates with:

  * bounded exposure,
  * measurable reputation from receipts,
  * circuit breakers,
  * and (where supported) atomic settlement primitives (HTLCs / preimage).

---

## 3) Architecture Overview

Hydra X adds two new blocks under the existing Hydra layer:

1. **Intent Router**: chooses LN direct vs native solver market vs FX RFQ vs multi-hop internal routes.
2. **Solver Market**: orderbook + matching + solver settlement adapters.

```text
Autopilot / Skills / Marketplace
            |
        TreasuryRouter (policy, budgets, approvals)
            |
     ┌──────▼──────────────────────────────────────────┐
     │                    Hydra X                      │
     │  LLP + CEP + Routing + FX + Receipts + /stats    │
     │                                                  │
     │  NEW: Intent Router + Solver Market + Adapters   │
     └──────┬───────────────────────────┬───────────────┘
            │                           │
   Lightning Node(s)            Cross-Rail Adapters
   (LLP + routing health)       (Native HTLC first; optional external interop adapters)
```

---

## 4) Lifecycle: From Intent to Settlement

Hydra X defines a single canonical flow with branch points.

### 4.1 Intent schema (conceptual)

An intent requests a conversion or transfer:

* **sell_asset** (e.g., `BTC_LN`, `BTC_ONCHAIN`, `USDC_BASE`)
* **buy_asset**
* **amount_in** or **amount_out**
* **constraints**:

  * `max_fee_bps`, `max_slippage_bps`
  * `deadline_ms`
  * `min_out`
  * `recipient` (destination address / invoice / pubkey)
  * `allowed_routes` / `disallowed_routes`
* **policy_bundle_id** + **idempotency_key**
* **trace links**: `session_id`, `trajectory_hash`, `job_hash`

### 4.2 Route planning

Hydra evaluates:

1. **LN direct** (LLP can pay invoice directly)
2. **FX RFQ** (Hydra FX flows for fiat/stable conversions if enabled)
3. **Solver market** (OpenAgents-native solver provider fills)
4. **Multi-hop route** (e.g., LN -> on-chain BTC -> cross-rail adapter path)

Hydra emits a **RoutePlan receipt** even before execution, so decisions are legible.

### 4.3 Execution and settlement

Hydra executes chosen route with:

* deterministic, idempotent steps,
* explicit state transitions,
* and receipts at each step.

---

## 5) Phase Plan (Roadmap Embedded in Spec)

Hydra X is delivered in three phases aligned to your recommendation.

### Phase 1 — Lightning Marketplace First (Short term)

**Goal:** dominate reliability and velocity on LN settlement for your own marketplace and Autopilot spend.

Scope:

* LLP: `quote_pay`, `pay`, `status` (already in flight)
* CEP: envelopes for objective workloads (already planned)
* Routing Intelligence: LN routing scorer + breakers
* `/stats` minute-cached health + economics

Explicit non-goal: bridging or cross-chain routing for end users.

**Deliverables**

* Highest uptime and deterministic payment receipts
* Strong underwriting loop from marketplace receipts
* Operator-grade `/stats` tables public at `/stats`

### Phase 2 — Liquidity Solver Jobs (Medium term)

**Goal:** create *real revenue jobs* for solver providers: solvers compete to fulfill intents and earn spreads/fees.

Scope:

* Solver Provider mode (OpenAgents role)
* Solver Market v0:

  * intent ingestion
  * matching
  * settlement on a limited set of routes (initially LN-adjacent, then BTC on-chain)
* CEP extension: allow bounded “solver envelopes” for capital-efficient fills (optional, gated)

**Primary product effect:** a new marketplace dimension—capital + execution, not just compute.

### Phase 3 — Hydra as Agent Liquidity Router (Long term)

**Goal:** Hydra becomes a multi-rail liquidity router rivaling systems like THORChain and CoW in function, but agent-native and receipt-driven.

Scope:

* Cross-rail adapters:

  * Native HTLC adapters (BTC ↔ EVM and others)
  * Optional external interop adapters (policy-gated, non-blocking)
  * Additional RFQ providers and aggregator logic
* Solver Market v1:

  * improved matching
  * solver reputations + stake/limits (no token, use BTC deposits or credibility proofs)
  * better atomicity coverage where possible
* Advanced routing:

  * multi-hop routes
  * path diversity / failover (still receipted, not hidden)

---

## 6) Solver Market Spec

### 6.1 What a solver is

A solver is an OpenAgents provider that:

* monitors intents,
* decides which to fill,
* commits capital to fulfill,
* executes settlement steps,
* earns fees/spread,
* and is judged by receipts.

Solvers can be:

* **Operator-funded** (OpenAgents-run)
* **Third-party providers** (permissionless, with onboarding rules)
* **Specialized** (certain routes, certain chains, certain max sizes)

### 6.2 Matching models

Hydra X supports three matching modes, introduced progressively:

1. **Direct fill** (single solver quotes and fills)
2. **Auction** (multiple solvers bid; Hydra selects per policy)
3. **Orderbook** (continuous listing; solvers take orders)

All produce a `MatchReceipt` and bind an `order_id`.

### 6.3 Solver constraints and risk controls

Each solver has a policy profile:

* max exposure per asset/route
* max notional per order
* minimum fee/spread
* timeouts + refund requirements
* reliability floor (based on reputation)
* circuit breaker triggers

Hydra enforces caps at match-time and at initiation-time.

### 6.4 Solver reputation

Reputation is computed from receipts:

* fill success rate
* median fill latency
* refund rate
* slippage vs quoted
* dispute rate
* chain execution failures

Reputation affects:

* eligibility
* max notional allowed
* credit envelope access (if enabled)
* ranking in auctions

### 6.5 Economic model

Solver revenues are BTC-denominated:

* explicit fee bps on route
* spread capture on swap
* optional “priority premium” for deadlines/urgency

Hydra takes either:

* zero platform fee initially (to seed network), or
* a small, explicit fee (later), always shown in receipts.

---

## 7) Cross-Rail Adapter Spec

Hydra X implements adapters behind a common interface. Each adapter is responsible for translating `RoutePlan` into state transitions and receipts.

### 7.1 Adapter interface (conceptual)

* `quote(intent) -> Quote`
* `match(intent, quote) -> Order`
* `initiate(order) -> InitiationReceipt`
* `await_destination(order) -> DestInitiatedReceipt`
* `redeem(order) -> RedeemReceipt`
* `refund(order) -> RefundReceipt`
* `status(order_id) -> State`

### 7.2 External Reference Systems (Not Currently Actionable)

Garden remains a useful reference architecture for adapter design (`intent -> match -> initiate -> redeem`), but it is not currently actionable as a roadmap dependency.

As of 2026-03-04, Garden's solver network/orderbook is not open to external fillers. Hydra therefore treats Garden-style flows as reference material for receipt shape and state-machine design, not as a required integration path.

If external adapter access becomes available in the future, it must remain policy-gated and non-blocking:

* disabled by default,
* bounded by treasury/route/notional controls,
* never required for core solver-market milestones.

### 7.3 Native HTLC Adapters (Primary Path)

Hydra X builds native HTLC settlement as the primary cross-rail path under OpenAgents authority.

Initial scope:

* BTC on-chain ↔ EVM HTLC
* Later: Starknet/Sui/Solana equivalents where viable

Components:

* Nostr intent broadcast (optional interop)
* Hydra matching engine (internal)
* chain adapters for HTLC initiate/redeem/refund
* solver providers run the filler logic under OpenAgents policy + receipt constraints

Benefits of this path:

* fewer external dependencies
* stronger alignment with receipt determinism + policy
* can integrate CEP credit envelopes deeply (bounded capital borrow for solvers)

---

## 8) Hydra X + CEP: Credit for Solvers (Optional, Gated)

CEP becomes a tool not only for buying compute, but for **capital-efficient routing**.

### 8.1 Solver Envelope

A solver may request a credit envelope to temporarily fund one side of a route, with strict rules:

* short expiry
* only usable for specific route IDs
* only usable to specific recipients/contracts
* must produce proof of fill completion
* automatic limit reductions on failures

This unlocks:

* more fills with less idle capital,
* faster market bootstrapping,
* and a measured risk system (credit is derived from solver track record).

Default stance: **off by default**. Enable only after Phase 2 has stable solver metrics.

---

## 9) Routing Intelligence (Expanded)

Hydra’s routing scorer evolves from LN-only to multi-rail.

### Inputs to routing

* intent constraints
* pool liquidity state
* solver inventory and reputations
* current failure spikes
* FX provider quotes (if enabled)
* historical receipts

### Outputs

* route plan with expected costs and confidence
* “why” notes that become part of receipts
* optional alternative plans (ranked)

### Deterministic behavior

Hydra must produce stable outputs given the same inputs and snapshots. Where live market data changes, the route plan must embed the snapshot hash that justified the plan.

---

## 10) Receipts and State Machines (Normative)

Hydra X defines canonical states for any routed intent:

* `CREATED`
* `QUOTED`
* `MATCHED`
* `SOURCE_INITIATED`
* `DESTINATION_INITIATED`
* `REDEEMED`
* `REFUNDED`
* `EXPIRED`
* `FAILED` (with typed reason + supporting receipts)

Every transition emits a receipt, minimum:

* `receipt_id`
* `type`
* `created_at_ms`
* `idempotency_key`
* `policy_bundle_id`
* `intent_id` / `order_id`
* `inputs_hash`
* `outputs_hash`
* external references (txid/order_id) where applicable

Receipts are the only truth for reputation and underwriting.

---

## 11) Public `/stats` (Expanded for Solver + Router)

Maintain the existing requirement: **cached once per minute**, same value for everyone.

Add solver/route metrics (up to ~50 rows total, table-first).

### Top section: Liquidity + Settlement Health

* LN success rate 5m/1h/24h
* median fee, p95 fee
* payment latency p50/p95
* channel inbound/outbound sats
* rebalance spend 24h
* outstanding CEP commitments
* CEP settle success rate

### Middle: Solver Market

* intents created 5m/1h/24h
* match rate
* median time-to-match
* median time-to-redeem
* refund rate
* top solvers by volume (capped list)
* slippage avg/p95 vs quote
* solver failure spike detector

### Bottom: Routing Mix + Concentration

* % LN direct vs solver vs FX
* route confidence bucket distribution
* concentration metrics (HHI/top share) for solver fills
* breaker states (global + per-route)

All derived from receipts + snapshot tables, never computed per-request.

---

## 12) API Surface (Conceptual, Mapped to Proto)

Hydra already has internal lanes. Hydra X adds:

### Intent Router

* `POST /v1/hydra/route/quote` → returns `RoutePlan`
* `POST /v1/hydra/route/execute` → executes plan (idempotent)
* `GET  /v1/hydra/route/status/:intent_id`

### Solver Market

* `POST /v1/hydra/solver/register` (provider metadata + policy)
* `POST /v1/hydra/solver/bid` (auction mode)
* `POST /v1/hydra/solver/fill` (commit to fill)
* `GET  /v1/hydra/solver/orders` (solver view)
* `GET  /v1/hydra/solver/reputation/:solver_id`

### Adapters (internal)

* `POST /internal/v1/hydra/adapters/htlc/*`
* `POST /internal/v1/hydra/adapters/external/*` (optional, future interop)

### Existing Hydra surfaces remain

* LLP: `quote_pay`, `pay`, `status`
* CEP: `intent`, `offer`, `envelope`, `settle`
* FX: `rfq`, `quote`, `select`, `settle`
* Routing score: `routing/score`
* `/stats`

**Proto source of truth** remains in `proto/openagents/hydra/v1/`.

---

## 13) Implementation Notes (Practical)

### Phase 1 focus

* Hardening LN settlement + receipts + `/stats`
* Mature CEP for objective workloads
* Build routing scorer + breakers

### Phase 2 focus

* Introduce solver role + provider registration
* Start with a narrow intent type:

  * “LN BTC → LN BTC faster/cheaper” (liquidity rebalancing jobs)
  * then “LN BTC → on-chain BTC” (simple rail boundary)
* Use strict caps and only operator solvers at first; then open.

### Phase 3 focus

* Expand native HTLC adapters for primary cross-rail coverage
* Optionally evaluate policy-gated external interop adapters if openness and reliability requirements are met
* Expand routing to multi-hop and more assets

---

## 14) Why This Rivals THORChain / CoW (But Agent-Native)

* Like **CoW**, Hydra X treats requests as **intents** and uses **solvers**.
* Like external swap-router systems, Hydra X can do **atomic swap style settlement** and solver fills.
* Like **THORChain**, Hydra X aims to route value across ecosystems.

But Hydra X is distinct because:

* settlement is **receipt-first** and replayable under OpenAgents invariants,
* credit is **outcome-scoped envelopes** rather than generalized leverage,
* the market is built for **machine-to-machine commerce** (agents), not retail bridging.

Hydra X becomes the liquidity substrate for an agent economy: agents compete not only on compute, but on capital routing.

---

## 15) Acceptance Criteria (What “Done” Means per Phase)

### Phase 1 (LN-first)

* 99%+ LN settlement success on internal workloads under load tests
* deterministic receipts for every payment
* `/stats` shows minute-cached liquidity + routing + CEP metrics

### Phase 2 (Solver jobs)

* solver registration + policy profiles
* intents matched with median time-to-match < target (you choose)
* receipts drive solver reputation ranking
* circuit breakers demonstrably prevent runaway losses

### Phase 3 (Liquidity router)

* at least one cross-rail native adapter live (HTLC path)
* multi-rail routing decisions are legible, reproducible, and receipted
* solver market supports multiple independent operators with bounded risk

---

# Addendum A — External Liquidity Posture (Native-First)

**Status:** Canonical addendum to *Hydra — OpenAgents Liquidity Engine (Spec)*

This addendum defines how Hydra handles external liquidity networks while preserving a native solver-market roadmap.

---

# A.1 Current Posture

As of 2026-03-04, Garden's solver network/orderbook is not open to external fillers.

Hydra therefore treats Garden as a **reference architecture**, not an integration dependency.

Garden flow remains useful as a modeling reference:

`intent -> match -> initiate -> redeem`

Roadmap implications:

1. OpenAgents builds native solver matching and solver provider participation.
2. OpenAgents builds native routing jobs and adapters under Hydra authority lanes.
3. External interop remains optional, future, and policy-gated.

Rule: **No roadmap item depends on Garden openness.**

---

# A.2 Adapter-Compatible, Not Adapter-Dependent

Hydra X is designed with pluggable adapters; however, OpenAgents does not assume access to any external solver network for liquidity bootstrapping.

Canonical path:

1. Build OpenAgents-native solver market first.
2. Expand native route coverage under Hydra invariants.
3. Add optional interop adapters later if and only if external networks become open and operationally reliable.

All mutations remain HTTP-only authority mutations. All effects remain receipt-driven and replayable.

---

# A.3 Native Solver Market MVP Path

Native build order (aligned with Hydra phases):

1. **LN-adjacent solver jobs first**
   - liquidity rebalancing
   - liquidity provisioning jobs
   - invoice acquisition/fill jobs
2. **BTC on-chain routing next**
   - constrained route types
   - strict policy caps and deterministic receipts
3. **Cross-chain adapters later**
   - HTLC-based adapters
   - enabled only after receipt and failure-state hardening

This sequence provides self-sufficient liquidity execution without third-party solver dependency.

---

# A.4 External Interop (Optional Future)

External adapter integrations are non-blocking and must never be required for:

* solver-provider onboarding,
* intent matching milestones,
* core liquidity routing SLAs.

If enabled in the future, external adapters must be:

* disabled by default,
* treasury/route/notional bounded,
* guarded by circuit breakers,
* fully receipted with request/response provenance.

---

# A.5 Reference Systems Appendix Note

Garden-specific endpoint and flow details are retained only as reference-system patterns.

**Not currently actionable: Garden network not open.**

Hydra remains interop-ready, but roadmap execution is native-first.
