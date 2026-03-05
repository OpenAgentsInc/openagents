Yes — I’d restructure it pretty aggressively.

Right now you have four documents that each do **three jobs at once**:

1. define primitives/invariants,
2. describe implementation status + roadmap,
3. introduce adjacent expansions (solvers, interop, outcomes markets) that reuse the same nouns but live in different “trust zones.”

That makes it hard for you (and contributors) to answer: *“What is the canonical contract? What’s the state machine? What’s optional? What’s shipping now?”*

If I were rewriting from scratch, I’d ship **one consolidated “Economy Kernel” spec** with a clean separation between:

* **Normative interface** (contracts, state machines, receipts)
* **Policy & risk** (what is allowed under which bundle)
* **Optional modules** (solvers, cross-rail, external bridges, AMMs)
* **Implementation status** (kept out of the normative spec)

Below is the structure I’d use.

---

## The new single spec: OpenAgents Economy Kernel

### 0. Purpose in one sentence

**Make agent work warrantable and payable at machine speed by turning “work → verification → liability → settlement” into deterministic, receipted state machines.**

### 1. Invariants (top-of-doc, non-negotiable)

Put every “doctrine” here, once, so the rest of the spec can assume it:

* Proto-first contracts
* **HTTP-only** authority mutations
* Deterministic receipts + canonical hashing
* Idempotency keys (safe retries, no double-settle)
* Budget-bounded by policy bundles
* No unscoped credit (envelopes only)
* No token dependence
* Public `/stats` is cached once per minute (Convex subscription-backed; no polling)

> Everything else becomes a corollary of these invariants.

---

## 2. Conceptual model (one vocabulary)

Define *one* set of nouns and use them everywhere:

**Actors**

* Agent
* Treasury / Operator
* Provider (worker)
* Underwriter (optional)
* Verifier / Adjudicator (optional)

**Objects**

* **WorkUnit** (a task/run/artifact with acceptance criteria)
* **Contract** (an enforceable agreement around an outcome)
* **Intent** (a request to move value under constraints)
* **Receipt** (the only truth of what happened)

This is where you “unify Hydra + Aegis”: not by merging code, but by sharing *the same state objects*.

---

## 3. Kernel modules (each is a state machine + receipts)

Instead of Hydra vs Aegis, define the kernel as these modules:

### 3.1 Settlement Engine (rails + quote/execute)

* `Quote → Execute → Receipt`
* LN first, on-chain later, others optional
* This is your existing LLP APIs, but framed as a kernel primitive

### 3.2 Liquidity & Reserves (pool accounting)

* channel liquidity health, rebalancing partition, snapshots
* LP share model is an **extension**, not part of the baseline

### 3.3 Credit Envelopes (bounded credit)

* `Intent → Offer → Envelope → Settle`
* scope-limited spend authority that **pays providers**, not “gives agents money”

### 3.4 Collateral & Bonds (ABP)

* `ReserveBond → ReleaseBond → DrawBond`
* used for warranties, self-bonds, dispute bonds, solver bonding (later)

### 3.5 Verification Engine

* verifiability classification + tier requirements
* `Plan → Verify → VerdictReceipt`
* independence tiers are an enforceable constraint, not guidance

### 3.6 Liability Engine (warranties, claims, disputes)

* `WarrantyIssue → WarrantyWindow → ClaimOpen → Adjudicate → Payout/Denial`
* this is where bonds and settlement come together

### 3.7 Markets (optional but native)

Put both under the same umbrella:

* **Work Outcome Markets** (bet-on-your-own-output + underwriting)
* **Belief Markets** (prediction markets)

This module consumes Verification + Collateral + Settlement. It doesn’t redefine them.

### 3.8 Routing & Risk

* deterministic scoring, exposure caps, circuit breakers
* routing decisions emit receipts (“why this route”) and are auditable

### 3.9 Observability

* `/stats` tables as a first-class contract
* everything in `/stats` is derived from receipts + snapshots and cached 60s

### 3.10 Interop (non-authoritative only)

* Bridge/Nostr mirroring of **summaries** only
* strictly no authority mutations outside HTTP

---

## 4. State machines (the heart of the spec)

In the consolidated spec, I’d include **four canonical state machines**, each with explicit terminal states and receipts:

1. **Payment**: `QUOTED → PAID | WITHHELD | FAILED`
2. **Envelope**: `ISSUED → COMMITTED → SETTLED | EXPIRED | REVOKED`
3. **Outcome Contract**:
   `CREATED → FUNDED → BONDED → SUBMITTED → VERIFYING → PASS/FAIL → SETTLED → (WARRANTY_ACTIVE) → FINALIZED`
4. **Claim**: `OPEN → REVIEW → APPROVED/DENIED → PAID/CLOSED`

Hydra X / solver / cross-rail can be an appendix state machine:

* `INTENT → MATCH → INITIATE → REDEEM/REFUND`

But keep it clearly labeled **Extension: Solvers & Cross-rail**.

---

## 5. Canonical receipt taxonomy

One section that defines:

* receipt types
* required linkage fields (`policy_bundle_id`, `idempotency_key`, `session_id`, `trajectory_hash`, etc.)
* canonical hashing rules
* how receipts cross-reference each other (Aegis verdict links to Hydra settlement receipts, etc.)

This avoids duplicated receipt definitions across Hydra/Aegis docs.

---

## 6. API layout (grouped by kernel module)

Instead of “Hydra endpoints” and “Aegis endpoints,” list APIs by module:

* Settlement: quote/pay/status
* Envelopes: intent/offer/envelope/settle
* Bonds: reserve/release/draw
* Verification: classify/verify/verdict
* Liability: warranty/claim/dispute
* Markets: create/trade/resolve (optional)
* Observability: `/stats`, receipts query

Then add a single “API posture” subsection:

* internal vs public
* what gets promoted to `/v1` and what stays `/internal/v1`

---

## 7. Economics (one coherent section)

Put all fees and flows in one place:

* settlement fees
* envelope fees / risk premium
* warranty premiums
* bond sizing policies
* LP returns (if enabled)
* platform fee (explicit only; receipted)

This will read *much* cleaner than scattering revenue assumptions across Hydra and Aegis.

---

## 8. Roadmap (kept separate from the normative spec)

I’d move your “implementation status snapshot” and MVP harnesses into:

* `ECONOMY_KERNEL_IMPLEMENTATION.md` (or `/docs/impl/economy-kernel.md`)

The canonical spec should not drift every time you land a commit.

---

# How I’d split it if “one doc” gets too big

If you end up wanting multiple files, I’d split like this:

1. **Economy Kernel (Normative)** — invariants, objects, state machines, receipts, APIs
2. **Risk & Policy** — underwriting rules, caps, breaker logic, tier gating
3. **Interop & Solvers (Optional Extensions)** — Hydra X, cross-rail, external adapters
4. **Operations & Metrics** — `/stats` contract, dashboard layout, runbooks, harnesses

But I’d still keep **one vocabulary** and **one receipt taxonomy**.

---

## Biggest change I’d make vs your current structure

Make **Outcome Contracts** the center of gravity.

Right now Hydra feels like “payments + credit” and Aegis feels like “verification + warranties,” and Outcomes Markets is “a cool new thing.”

In the rewritten spec, everything revolves around:

> **A Contract that binds work + verification + liability + settlement**.

Hydra becomes the *settlement/collateral runtime* for contracts.
Aegis becomes the *verification/liability runtime* for contracts.
Markets become a *pricing layer* on contracts.

---

If you want, I can also draft the **actual consolidated Table of Contents** with section-by-section bullet text (as if it’s the final doc), and point exactly where your existing Hydra FX + CEP + `/stats` contracts slot in without losing any of the good work you’ve already done.


---

Below is a **single consolidated spec outline** (Table of Contents + what each section says), written as if it’s the final doc. After the TOC, I included a **mapping** that shows exactly where your current **Hydra CEP + FX + `/stats`** work lands in this structure.

---

# OpenAgents Economy Kernel — Consolidated Spec (Draft)

**Purpose:** make agent work **warrantable and payable at machine speed** by turning
**work → verification → liability → settlement** into deterministic, receipted state machines.

**Normative scope:** proto contracts, state machines, receipts, authority rules, and public observability contracts.
**Non-normative scope:** implementation status, harness scripts, rollout plans, and operator runbooks (moved to appendices).

---

## 0. Orientation

### 0.1 One-sentence definition

* The Economy Kernel is a set of **authority-only HTTP services** that coordinate capital and trust for autonomous agents using **deterministic receipts**.

### 0.2 What this doc replaces

* Supersedes “Hydra vs Aegis vs Hydra X vs Outcomes Markets” as separate conceptual pillars.
* Recasts them as **modules** over shared primitives (WorkUnit, Contract, Intent, Receipt).

### 0.3 Terminology quick map

* “Hydra” becomes: **Settlement + Liquidity + Envelopes + FX + Bonds**
* “Aegis” becomes: **Verification + Liability (warranties/claims) + Autonomy throttle**
* “Outcomes markets” becomes: **Contracts + pricing layer** on top of the same primitives
* “Hydra X” becomes: **Optional extension**: solver matching + cross-rail adapters

---

## 1. Invariants (non-negotiable)

### 1.1 Authority transport

* **All authority mutations are authenticated HTTP only.**
* No WS/Nostr/Spacetime path can mutate money state, credit state, bond state, or verdicts.

### 1.2 Determinism and replay safety

* Every authority action is **idempotent** via `idempotency_key`.
* Every effect emits a canonical **Receipt** with deterministic hashing.
* Retrying an identical request must not double-spend or double-settle.

### 1.3 Policy-bounded execution

* Every authority action includes `policy_bundle_id`.
* Budgets and caps are enforced at execution time (not “best effort”).

### 1.4 Credit and risk posture

* **No unscoped credit.** Only outcome- or scope-bounded envelopes.
* Circuit breakers must be explicit states with receipts.

### 1.5 Observability contract

* `/stats` is public operator-grade telemetry, **cached once per minute** (same view for everyone).
* UI fetches via Convex subscriptions (no polling); server recomputes snapshot once per minute.

---

## 2. Kernel objects and roles (single vocabulary)

### 2.1 Core objects

* **WorkUnit:** a job/run/artifact bundle with acceptance criteria + trace links.
* **Contract:** binds a WorkUnit to an outcome definition (PASS/FAIL or multi-grade), optional warranty.
* **Intent:** request to move value under constraints (payment, FX, solver route).
* **Bond:** locked collateral backing warranties, self-stakes, disputes.
* **Receipt:** canonical record of any authority effect; the only truth used for reputation/risk.

### 2.2 Roles

* Buyer / Operator
* Worker (Provider agent)
* Verifier (objective harness, heterogeneous checker, human)
* Underwriter (optional)
* Adjudicator (optional)
* TreasuryRouter (policy brain) vs Kernel services (execution brain)

---

## 3. System architecture (how the modules compose)

### 3.1 Core flow (the “economic pipeline”)

* Create WorkUnit → create Contract → fund + bond → submit output → verify → settle → warranty window → claims → finalize

### 3.2 Service boundaries

* **TreasuryRouter:** chooses what should happen (policy/budgets/approvals)
* **Kernel services:** execute how it happens; emit receipts
* **Wallet Executor:** custody boundary and canonical rail receipts
* **LN node backend (LLP):** operator liquidity surface

### 3.3 Trust zones

* Class 1 (authority): HTTP-only, signed, receipted, idempotent
* Class 2 (coordination): WS/Spacetime allowed, non-authoritative projections only

---

## 4. State machines (normative)

### 4.1 Payment state machine (Settlement)

* `QUOTED → PAID | WITHHELD | FAILED`
* Receipt types:

  * quote receipt (binding fee ceiling / expiry / policy)
  * payment receipt (rail proof + trace linkage)
  * withheld receipt (explicit reason: expiry, fee cap, breaker, etc.)

### 4.2 Credit envelope state machine (Envelopes)

* `INTENT → OFFER → ENVELOPE_ISSUED → COMMITTED → SETTLED | EXPIRED | REVOKED`
* Envelopes authorize the kernel to pay **providers** under constraints; agents do not receive free-floating funds.

### 4.3 Contract outcome state machine (Work outcomes)

* `CREATED → FUNDED → BONDED → SUBMITTED → VERIFYING → PASS/FAIL → SETTLED → (WARRANTY_ACTIVE) → FINALIZED`
* Links verification receipts to settlement receipts.

### 4.4 Claim/dispute state machine (Liability)

* `OPEN → REVIEW → APPROVED/DENIED/PARTIAL → PAID/CLOSED`
* Bond draws and releases are explicit transitions with Hydra receipts.

### 4.5 Optional extension state machine (Solvers / cross-rail)

* `INTENT → MATCH → INITIATE → REDEEM | REFUND | EXPIRE`
* Only used if/when solver market is enabled; must still satisfy invariants (idempotency + receipts).

---

## 5. Receipts and canonical hashing (normative)

### 5.1 Receipt envelope (shared schema)

* Required fields:

  * `receipt_type`, `receipt_id`, `created_at_ms`
  * `idempotency_key`
  * `policy_bundle_id`
  * `trace` (`session_id`, `trajectory_hash`, `job_hash`, etc.)
  * `inputs_hash`, `outputs_hash` (canonical)
  * evidence pointers (hash + URI)

### 5.2 Cross-receipt linkage rules

* Verification receipts must reference:

  * WorkUnit id + Contract id
  * artifact hashes and/or replay bundle hashes
* Settlement receipts must reference:

  * Contract id and verdict receipt hash (when pay-after-verify)
* Claims must reference:

  * warranty terms hash + evidence bundle hash + prior receipts

### 5.3 Deterministic serialization rules

* Canonical field order, normalization rules, and stable hashing scheme
* “Tags/meta” never influence `canonical_hash`

---

## 6. Kernel modules (normative APIs + semantics)

### 6.1 Settlement Engine (Lightning-first)

* `quote_pay`, `pay`, `status`
* Fee ceilings, expiries, and withholds are explicit
* Emits receipts designed for machine audit and safe retries

**Where your current Hydra work fits:**

* `liquidity: quote_pay, pay, status` are this module.

### 6.2 Liquidity & Reserves (pool accounting)

* Pool partitions and health snapshots
* Rebalancing/Reserve partition (optional but modeled)
* Signed snapshots (if/when LP mode exists)

**Where your current Hydra work fits:**

* Your channel health + routing health + observability metrics feed this module’s snapshots.

### 6.3 Credit Envelopes (CEP)

* Outcome/scoped credit envelopes
* Underwriting policy hooks (reputation-weighted caps)
* Default: pay-after-verify for objective lanes

**Where your current Hydra work fits:**

* `credit: intent, offer, envelope, settle, health` map here directly.

### 6.4 Bonds & Collateral (ABP)

* `reserve`, `release`, `draw`
* Supports:

  * worker self-bonds
  * underwriter bonds
  * dispute bonds
  * warranty reserves

**Where this fits relative to today:**

* This is the missing “ABP” partition you’d add alongside LLP/CEP/RRP.

### 6.5 Verification Engine (Aegis core)

* Verifiability classifier per WorkUnit
* Verification tiers (O/1/2/3/4) are enforceable requirements
* Verdict finalization produces a receipt that can trigger settlement

**Where your Aegis spec fits:**

* All “verification plane + tiering + receipts” land here.

### 6.6 Liability Engine (Warranties, claims, adjudication)

* Warranty issuance (opt-in, terms hashed)
* Claim opening and adjudication
* Payouts and slashing are executed via bond draws + settlement engine

**Where your Aegis spec fits:**

* All “warranties/bonds/claims/disputes” land here.

### 6.7 FX RFQ & Settlement (Treasury Agents)

* RFQ → quote → select → settle
* Provenance receipts: quote id, expiry behavior, selection rationale, settlement proofs
* Explicit “withheld” behavior and replay safety

**Where your current Hydra FX work fits:**

* Your existing endpoints slot here verbatim:

  * `POST /internal/v1/hydra/fx/rfq`
  * `POST /internal/v1/hydra/fx/quote`
  * `POST /internal/v1/hydra/fx/select`
  * `POST /internal/v1/hydra/fx/settle`
  * `GET /internal/v1/hydra/fx/rfq/:rfq_id`
* Your deterministic settlement receipts and quote-expiry-withheld behavior are the normative semantics of this module.

### 6.8 Routing & Risk (deterministic scorer + breakers)

* Routing score is a machine-readable decision artifact
* Circuit breakers and throttles are explicit state
* Feeds underwriting (envelopes) and settlement selection (LN route posture)

**Where your current Hydra work fits:**

* `routing score`, `risk health`, breaker state, and confidence buckets map here.

### 6.9 Markets (optional module)

Two submodules, same primitives:

**6.9.1 Work Outcome Markets**

* Contracts + bonds + underwriting pools
* Optional “market pricing” later; MVP is self-bond + co-bond

**6.9.2 Belief Markets**

* Internal prediction markets with deterministic resolution
* Optional LMSR/CLOB; not required for initial kernel success

### 6.10 Reputation Index (receipt-derived)

* Reputation is computed from receipts (settlement success, claim rate, calibration)
* Used only as priors for underwriting and routing; not social scoring

---

## 7. Public observability contract (`/stats`) (normative)

### 7.1 Snapshot model

* Single minute-cached snapshot shared across all viewers
* Derived only from receipts + signed pool snapshots, never computed per request

### 7.2 Tables (stable schema)

Top tables:

* Settlement health: success rate, fees, latencies
* Liquidity health: inbound/outbound sats, channel health, rebalance burn
* Envelopes: outstanding commitments, settle rates, loss/withhold counts
* FX: RFQ/quote/settle totals, conversion, spreads, withheld/failed
* Verification: verified share by tier, latency, drift indicators
* Liability: warranties issued, bonded exposure, claims opened/paid/denied, loss rate
* Risk: breaker states, concentration metrics, failure spikes

**Where your current `/stats` work fits:**

* Keep your existing Hydra FX block and expand with verification/liability blocks.

---

## 8. Economics (normative fee disclosure + flows)

### 8.1 Fee principles

* All fees explicit in quotes and receipts
* No hidden routing fees

### 8.2 Revenue streams

* Settlement premiums (optional, explicit)
* Envelope fees / risk premiums (CEP)
* FX spreads (Treasury Agents) with provenance
* Warranty premiums / underwriting premiums
* Optional market-maker fees (if/when markets enabled)

### 8.3 Solvency rules (if external LPs enabled)

* Partitioned accounting
* Withdrawal queues and throttles
* Signed snapshots and safety bands

---

## 9. Interop and external networks (non-normative extension, but rules are normative)

### 9.1 Hard rule

* No roadmap dependency on third-party solver openness (Garden posture preserved)
* External adapters are optional and policy-gated

### 9.2 What can be mirrored externally

* Non-authoritative summaries: receipts summaries, reputation labels, market odds
* Never authority mutations

---

## 10. Implementation profile and rollout (non-normative)

### 10.1 Phases (clean and aligned to what you’ve built)

* Phase A: LN settlement + CEP + FX + `/stats` (operator-funded)
* Phase B: Verification plane + pay-after-verify default for objective lanes
* Phase C: Bonds + warranties + claims
* Phase D: Solver market + cross-rail (optional)
* Phase E: External LP deposits (optional), external interop (optional)

### 10.2 Harnesses and CI gates

* Reference your existing:

  * `./scripts/vignette-hydra-mvp2.sh`
  * `./scripts/vignette-hydra-mvp3.sh`
* Add Aegis harness equivalents for verification and claims

---

# Mapping: your existing work into the consolidated spec

### Hydra CEP (already implemented in `main`)

* Lands in **6.3 Credit Envelopes**
* Endpoints:

  * `intent`, `offer`, `envelope`, `settle`, `health`, per-agent exposure
* Semantics preserved:

  * outcome-scoped spend authority
  * underwriting + breaker hooks
  * deterministic receipts

### Hydra FX MVP-3 (already implemented)

* Lands in **6.7 FX RFQ & Settlement**
* Endpoints preserved exactly:

  * `POST /internal/v1/hydra/fx/rfq`
  * `POST /internal/v1/hydra/fx/quote`
  * `POST /internal/v1/hydra/fx/select`
  * `POST /internal/v1/hydra/fx/settle`
  * `GET /internal/v1/hydra/fx/rfq/:rfq_id`
* Your current behaviors become normative:

  * reservation enforcement
  * quote expiry withheld
  * idempotency drift conflict handling
  * provenance receipts

### Hydra routing score + risk health + observability

* Lands in **6.8 Routing & Risk** and **7 `/stats`**
* Your confidence buckets and breaker transitions become the standard “risk signaling” contract.

### `/stats` minute-cached dashboard

* Lands in **7 Public observability contract**
* Your current Hydra metrics become the top block; we add verification/liability blocks later.
* Keep the “60s cache / same for everyone” rule as normative.

---

If you want, next I can do one of these (without changing structure):

1. Draft the **actual full text** of sections 1–7 (the normative kernel), or
2. Draft the **proto package plan** for the consolidated kernel (file paths, package names, what gets merged, what stays separate), preserving your existing `proto/openagents/hydra/v1` and adding only what’s missing (ABP + outcomes).
