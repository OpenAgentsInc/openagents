# Spec: Hydra X — Agent Liquidity Router and Solver Network

**Status:** Canonical draft (supersedes “bridge notes” and consolidates solver + routing strategy)

Hydra X extends **Hydra (OpenAgents Liquidity Engine)** into a unified economic substrate that supports:

1. **Lightning-first settlement** for marketplace flows (MVP now).
2. **Liquidity solver jobs** where agent operators compete to fulfill value-routing intents (Medium term).
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

A protocol + runtime that allows agent operators (“solvers”) to compete to fulfill **liquidity intents**, earning BTC-denominated fees/spreads.

### (E) Cross-Rail Adapters

A set of rail adapters that let Hydra satisfy intents across LN, on-chain BTC, EVM, and other chains (phase-gated). Garden integration is treated as one adapter path.

Hydra X is the economic complement to your compute marketplace: **compute intents + liquidity intents** become the two legs of an agent economy.

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

1. **Intent Router**: chooses LN direct vs solver vs FX RFQ vs hybrid.
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
   (LLP + routing health)       (Garden adapter, HTLC, etc.)
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
3. **Solver market** (internal or external solver fills)
4. **Hybrid** (e.g., LN → on-chain BTC → cross-rail)

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

**Goal:** create *real revenue jobs* for agents: solvers compete to fulfill intents and earn spreads/fees.

Scope:

* Solver Provider mode (OpenAgents role)
* Solver Market v0:

  * intent ingestion
  * matching
  * settlement on a limited set of routes (initially LN-adjacent, then BTC on-chain)
* CEP extension: allow bounded “solver envelopes” for capital-efficient fills (optional, gated)

**Primary product effect:** a new marketplace dimension—capital + execution, not just compute.

### Phase 3 — Hydra as Agent Liquidity Router (Long term)

**Goal:** Hydra becomes a multi-rail liquidity router rivaling systems like THORChain/Garden/CoW in function, but agent-native and receipt-driven.

Scope:

* Cross-rail adapters:

  * Garden adapter (as one path)
  * Native HTLC adapters (BTC ↔ EVM and others)
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

### 7.2 Garden Adapter (Integration)

Hydra X can integrate Garden as a cross-rail adapter, using the cookbook flow.

**Key: Garden is treated as an external orderbook + relayer path.**

Hydra X calls Garden endpoints through an adapter service:

* strategy selection
* price quote
* attested order generation
* order creation
* polling matched order
* initiation (EVM via typed signing + relayer; BTC via HTLC funding tx)
* destination initiation observation
* redeem (EVM via relayer; BTC via signed redeem tx; optional gasless redemption)

**Receipts required**
Hydra must emit canonical receipts that wrap Garden evidence:

* `hydra.routeplan_receipt` (selected Garden strategy)
* `hydra.external_order_receipt` (Garden order_id, matched order hash)
* `hydra.chain_initiate_receipt` (EVM tx hash or BTC funding txid)
* `hydra.chain_redeem_receipt` (EVM redeem tx hash or BTC redeem txid)
* `hydra.settlement_final_receipt` (amounts + fees + completion)

Hydra stores:

* request/response digests (hashes) for Garden calls
* any relayer IDs
* EIP-712 signed payload hash
* Bitcoin tx hex hash and txid

**Policy gating**
Garden adapter can be disabled by default and enabled only for:

* certain treasuries,
* certain max sizes,
* certain chains,
* and only when LN direct cannot satisfy an intent.

### 7.3 Native HTLC Adapters (Build-your-own)

Hydra X supports implementing native HTLC settlement without Garden’s servers. This is the “our own version” path.

Initial scope:

* BTC on-chain ↔ EVM HTLC
* Later: Starknet/Sui/Solana equivalents where viable

Components:

* Nostr intent broadcast (optional interop)
* Hydra matching engine (internal)
* chain adapters for HTLC initiate/redeem/refund
* solver operators run the filler logic (like Garden COBI)

Benefits vs Garden:

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

* `POST /internal/v1/hydra/adapters/garden/*`
* `POST /internal/v1/hydra/adapters/htlc/*`

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

* Introduce Garden adapter for selected treasuries and intents
* Add native HTLC adapters as you migrate away from external dependencies
* Expand routing to multi-hop and more assets

---

## 14) Why This Rivals THORChain / Garden / CoW (But Agent-Native)

* Like **CoW**, Hydra X treats requests as **intents** and uses **solvers**.
* Like **Garden**, Hydra X can do **atomic swap style settlement** and solver fills.
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

* at least one cross-rail adapter live (Garden and/or native HTLC)
* multi-rail routing decisions are legible, reproducible, and receipted
* solver market supports multiple independent operators with bounded risk

---

# Addendum A — External Liquidity Adapters and Hybrid Solver Strategy

**Status:** Canonical addendum to *Hydra — OpenAgents Liquidity Engine (Spec)*

This addendum defines Hydra’s strategy for integrating external liquidity networks (e.g., Garden) while building the OpenAgents-native solver market described in Hydra X.

Hydra adopts a **hybrid architecture**:

1. **Aggregate external liquidity networks where advantageous.**
2. **Develop a native solver market for agent-operated liquidity routing.**
3. **Gradually internalize high-volume routes as solver liquidity matures.**

This approach allows Hydra to **bootstrap liquidity and route coverage immediately** while preserving long-term **sovereignty, reliability, and protocol invariants**.

---

# A.1 Design Principle

Hydra treats external liquidity networks as **route providers**, not as trusted settlement authorities.

Hydra remains the **authoritative economic layer** responsible for:

* policy enforcement
* budgeting and credit envelopes
* routing decisions
* receipt emission
* settlement verification
* failure recovery

External networks provide **liquidity execution capacity**, but Hydra retains full control over:

* which routes are used
* when they are used
* exposure limits
* failover behavior

This ensures Hydra remains the **canonical settlement ledger for the OpenAgents economy**.

---

# A.2 External Liquidity Adapters

Hydra X introduces the concept of **External Liquidity Adapters**.

Adapters allow Hydra to interact with external routing or swap systems without compromising Hydra’s deterministic execution model.

Examples include:

* Garden
* RFQ liquidity providers
* external swap routers
* chain-native HTLC systems

Each adapter implements the standard Hydra adapter interface:

```
quote(intent)
match(intent, quote)
initiate(order)
await_destination(order)
redeem(order)
refund(order)
status(order_id)
```

Adapters must produce sufficient evidence for Hydra to generate canonical receipts.

Adapters do **not** mutate Hydra state directly.

All authoritative mutations occur through Hydra HTTP authority endpoints.

---

# A.3 Garden Adapter (Bootstrap Liquidity)

Hydra integrates Garden through a **Garden Adapter** that treats Garden as a route provider.

Hydra interacts with Garden’s infrastructure using the documented flow:

1. Fetch strategies
2. Obtain price quote
3. Request attested order
4. Create order
5. Wait for filler match
6. Initiate source chain swap
7. Wait for destination initiation
8. Redeem funds

Hydra wraps each stage with canonical receipts.

Minimum evidence stored for each Garden operation includes:

* Garden `strategy_id`
* Garden `order_id`
* request/response digests
* EIP-712 payload hash (for EVM initiation)
* Bitcoin transaction hex hash
* Bitcoin funding txid
* Bitcoin redeem txid
* destination chain tx hash
* final settlement amounts

Hydra therefore maintains **full provenance** even when using external infrastructure.

---

# A.4 Policy and Exposure Controls

External adapters are subject to strict policy gating.

Default policy for external liquidity routes:

* disabled unless explicitly allowed
* maximum notional per intent
* maximum daily exposure
* allowed asset pairs
* allowed destination chains
* routing score minimum thresholds
* circuit breaker triggers

Hydra routing intelligence may choose external routes only when:

1. LN direct settlement is not possible, or
2. external route cost and reliability exceed internal solver routes.

External routes are therefore **fallback or expansion routes**, not the primary settlement path.

---

# A.5 Hybrid Routing Strategy

Hydra’s routing layer evaluates routes in priority order:

1. **LLP (Lightning direct)**
   Primary settlement rail for the OpenAgents marketplace.

2. **Internal solver market**
   Agent providers fulfilling liquidity intents within the OpenAgents economy.

3. **External liquidity adapters (e.g., Garden)**
   Used for route coverage and liquidity bootstrapping.

4. **FX RFQ providers**
   Used where fiat or stable asset conversion is required.

Hydra chooses routes using deterministic scoring based on:

* expected cost
* reliability history
* latency
* solver reputation
* liquidity availability
* policy constraints

Hydra emits a **RoutePlan receipt** documenting the chosen route and the factors influencing the decision.

---

# A.6 Internalization Strategy

Hydra will gradually internalize routes as internal liquidity improves.

Internalization occurs when the internal solver market meets reliability and liquidity thresholds.

Examples:

| Route                 | Initial Strategy  | Long-term Strategy      |
| --------------------- | ----------------- | ----------------------- |
| LN → LN               | LLP direct        | LLP direct              |
| LN → BTC on-chain     | External adapters | Internal solvers        |
| BTC → EVM stablecoins | Garden adapter    | Internal HTLC adapters  |
| BTC → other chains    | Garden adapter    | Internal solver network |

Hydra continuously measures route performance using settlement receipts.

Internalization decisions are based on:

* volume concentration
* solver participation
* reliability metrics
* capital efficiency
* operational risk

Garden may remain available as a **long-tail fallback route** even after internalization.

---

# A.7 Benefits of Hybrid Architecture

The hybrid strategy provides several advantages.

### Immediate route coverage

Hydra gains instant access to cross-chain liquidity through Garden.

This allows OpenAgents to support cross-rail settlement before its own solver market is fully developed.

### Liquidity bootstrapping

Hydra can observe real market behavior from existing solver networks, including:

* spread distributions
* fill latency
* failure modes
* liquidity availability

This data informs Hydra’s routing intelligence and underwriting models.

### Gradual sovereignty

Hydra can progressively internalize routes without disrupting the interface exposed to Autopilot and Skills.

Because Hydra controls the intent and receipt layers, changing adapters does not affect higher layers of the system.

### Risk containment

External adapters are constrained by policy caps and circuit breakers, limiting systemic risk while liquidity is being bootstrapped.

---

# A.8 Relationship to the Solver Market

Hydra’s solver market and external adapters coexist.

External routes provide baseline liquidity while internal solvers grow.

Over time, internal solvers may outperform external providers on:

* latency
* reliability
* fees
* capital efficiency

Hydra routing intelligence will naturally shift order flow toward internal solvers as their performance improves.

This creates a **positive feedback loop**:

1. Internal solvers earn more volume.
2. Solver capital increases.
3. Liquidity deepens.
4. Internal routes become cheaper and more reliable.
5. External dependency decreases.

---

# A.9 Deterministic Receipts for External Routes

Hydra must ensure that external routes produce receipts equivalent in strength to internal settlement receipts.

External adapter receipts include:

```
hydra.routeplan_receipt
hydra.external_order_receipt
hydra.chain_initiate_receipt
hydra.chain_redeem_receipt
hydra.settlement_final_receipt
```

These receipts contain:

* adapter identifier
* external order identifiers
* transaction hashes
* proof artifacts
* settlement amounts
* routing decisions
* policy bundle identifiers

This preserves OpenAgents’ core invariant:

**all economic activity must be replayable from receipts.**

---

# A.10 Long-Term Outcome

Under the hybrid architecture, Hydra evolves through three stages:

**Stage 1 — Lightning settlement engine**

Hydra provides reliable LN settlement and credit envelopes for the OpenAgents marketplace.

**Stage 2 — Solver marketplace**

Agent providers compete to fulfill liquidity intents and earn routing fees.

**Stage 3 — Agent liquidity router**

Hydra routes value across multiple rails using both internal solvers and external liquidity networks.

At maturity, Hydra functions as a **machine-native liquidity layer** where:

* agents provide compute
* agents provide capital
* agents provide routing

and all of it settles through deterministic receipts.

External liquidity networks remain integrated as optional route providers, but the OpenAgents solver market becomes the **primary liquidity engine** of the ecosystem.
