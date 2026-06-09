# OpenAgents Economy Kernel — Consolidated Spec (Normative)

Sections 1–7

This document defines the **normative** behavior of the OpenAgents Economy Kernel: the invariants, object model, trust zones, state machines, receipt rules, kernel modules, and the `/stats` observability contract. Implementation status, harness scripts, and operational runbooks are intentionally out of scope and belong in separate implementation docs.

---

## 1. Invariants

The following invariants are non-negotiable. Every kernel module, API, adapter, and extension must obey them. The rest of the spec assumes these invariants and may not restate them.

### 1.1 Authority transport

1. **All authority mutations MUST occur via authenticated HTTP requests.**
   “Authority mutation” means any action that can change:

   * money state (payments, escrow, settlement, refunds)
   * credit state (envelopes, commitments, underwriting limits)
   * collateral state (bond reserves, bond draws, bond releases)
   * verification state (verdict finalization that gates money)
   * liability state (warranties, claims, dispute resolutions)
   * pool accounting state (LP share mint/burn, solvency gates)

2. **No WS/Nostr/Spacetime lane MAY mutate authority state.**
   Those transports are allowed only for projection, coordination, and UI delivery.

3. **Every authority mutation MUST be attributable.**
   Requests must include:

   * an authenticated caller identity (service principal, agent, or operator identity)
   * `policy_bundle_id`
   * `idempotency_key`
   * `trace context` (see §2.1 and §5.1)

### 1.2 Determinism and replay safety

1. **Every authority mutation MUST be idempotent.**
   Replaying the same request with the same `idempotency_key` MUST:

   * not duplicate effects
   * return the same terminal state and receipt (or a stable reference to it)

2. **Every authority mutation MUST emit a deterministic Receipt.**
   “Deterministic” means the receipt’s canonical hash is a function of normalized fields defined in §5.

3. **Any non-deterministic external dependency MUST be “snapshotted” or “bound” before execution.**
   Examples:

   * LN payment uses `quote → pay` so the fee ceiling and expiry are bound.
   * FX uses `rfq → quote → select → settle` so the chosen quote is bound.
   * Verification uses a declared plan and evidence digests so the decision is bound.

4. **The kernel MUST provide explicit failure modes.**
   A failure is not “silent retry.” A failure is a distinct state with a typed reason and a receipt.

### 1.3 Policy-bounded execution

1. **Every authority mutation MUST include `policy_bundle_id`.**
   The kernel is not permitted to execute “best effort” outside an explicit policy context.

2. **Budgets, limits, and caps MUST be enforced at execution time.**
   If budgets are exceeded or risk breakers are active, the kernel MUST:

   * refuse execution (explicit failure state), or
   * withhold execution (explicit withheld state with an explanation)

3. **Policy evaluation MUST be explainable.**
   The kernel MUST include policy notes (as receipt evidence or stable tags) sufficient to explain why an action was allowed or denied.

### 1.4 Credit and risk posture

1. **No unscoped credit.**
   Credit can exist only as **bounded envelopes** tied to:

   * a scope (what it can pay for)
   * a cap (max amount)
   * an expiry
   * and a settlement condition (often a verification verdict)

2. **Circuit breakers MUST be explicit and receipted.**
   Breakers are not hidden throttles. When a breaker changes behavior, it must be observable and explainable.

3. **Risk is partitioned, not implicit.**
   Pool and collateral exposures must be attributable to partitions (liquidity, credit, bonds, reserves), so subsidies and losses are legible.

### 1.5 Observability contract

1. The system MUST expose a public operator-grade `/stats` endpoint (see §7).
2. `/stats` MUST be computed from receipts + snapshots and cached once per minute.
3. UI delivery MUST be subscription-driven (Convex realtime subscriptions). No polling for UI data is permitted as a design posture.

---

## 2. Kernel objects and roles

The Economy Kernel is organized around a small set of objects. These objects are the shared vocabulary across settlement, verification, credit, and liability. Extensions must reuse them rather than invent parallel nouns.

### 2.1 Core objects

**WorkUnit**
A WorkUnit is a unit of work whose outcome can be evaluated and tied to money. Examples:

* a compute job result
* a skill invocation result
* an Autopilot artifact bundle (patch + tests)
* an L402 call series result

A WorkUnit MUST have:

* a stable identifier (or stable hash)
* acceptance criteria (objective harness reference or subjective rubric reference)
* trace context (session/run/trajectory/job linkage)

**Contract**
A Contract binds a WorkUnit to an outcome definition and (optionally) a warranty/liability posture. Minimal form is binary: PASS/FAIL, but multi-grade is allowed later.

A Contract MUST define:

* the parties (buyer, worker/provider; optional underwriter)
* the payment terms (price, when payment is released)
* the verification requirements (tier, evidence rules)
* the bond/collateral requirements (if any)
* optional warranty terms (window, remedy, cap)
* dispute/claim procedure (if warranty is enabled or if subjective verification is used)

**Intent**
An Intent is a request to move value under constraints. Examples:

* pay this invoice within fee cap by deadline
* convert USD budget to sats under RFQ policy and settle
* reserve collateral to back a warranty

Intents MUST specify:

* what value movement is desired
* constraints (fee cap, slippage cap, deadline)
* trace + policy + idempotency

**Bond**
A Bond is locked collateral backing an obligation. Bonds are used for:

* worker “bet on your own output” self-bonds
* underwriter collateral
* dispute bonds (anti-spam, loser-pays)
* warranty reserves (if the warranty model requires dedicated reserve)

Bonds MUST be:

* reservable, drawable, and releasable via authority-only API
* linked to the relevant Contract/WorkUnit/Claim
* accounted in a dedicated collateral partition (ABP)

**Receipt**
A Receipt is the canonical record of an authority effect. Receipts are:

* the only source of truth for what happened
* the substrate for reputation and underwriting
* the substrate for `/stats`

Every authority mutation must produce exactly one Receipt, even in failure/withheld paths.

### 2.2 Roles

**Buyer / Operator**
Defines what the system is trying to accomplish, chooses risk posture via policy bundles, and funds work. In enterprise settings this is often the “operator” of an Autopilot instance.

**Worker / Provider Agent**
Produces the WorkUnit output. May optionally post a self-bond and/or sell a warranty.

**Verifier**
Executes verification according to the required tier (objective harness, heterogeneous checker set, adjudication). Verifiers do not control money; they produce verdict receipts.

**Underwriter**
Optionally posts collateral to back warranties or outcome claims. Underwriters earn premiums when outcomes hold and pay when they do not.

**Adjudicator**
Resolves disputes under a declared adjudication policy. Adjudication is an authority act and must be receipted.

**TreasuryRouter**
The policy brain: chooses budgets, approvals, and which kernel modules to call. TreasuryRouter does not “do” settlement; it directs the kernel to execute under policy.

---

## 3. System architecture and trust zones

### 3.1 The economic pipeline

The kernel is designed so “agent work” becomes economically real only when it passes through an explicit pipeline:

1. A WorkUnit is created with acceptance criteria and trace context.
2. A Contract is created binding payment, verification, and optional warranty terms.
3. Funding is reserved (direct spend or credit envelope). Optional bonds are reserved.
4. Output is submitted as artifacts + evidence, with digests.
5. Verification is executed under a declared tier and produces a verdict receipt.
6. Settlement is executed only as allowed by the verdict and the policy bundle.
7. If warranty exists, a warranty window opens. Claims/disputes can be filed.
8. At the end of the window (or after claim resolution), the contract finalizes.

The kernel’s job is to keep this pipeline deterministic, policy-bounded, and replay-safe.

### 3.2 Separation of concerns

**Kernel services are execution, not product UI.**
The kernel is a service layer invoked by Autopilot, Marketplace, and TreasuryRouter.

**Wallet Executor is the custody boundary.**
Kernel receipts may reference wallet-executor receipts as settlement proofs. The kernel does not silently assume payment occurred; it requires canonical proof.

**Lightning node backend is liquidity plumbing, not the interface.**
The kernel exposes quote/pay semantics and receipts; node ops details are internal.

### 3.3 Trust zones

**Class 1: Authority (HTTP-only)**
Includes:

* payment execution
* envelope issuance and settlement
* bond reserve/release/draw
* verdict finalization (because it gates money)
* warranty issuance and claim resolution
* circuit breaker state transitions

**Class 2: Non-authoritative coordination (WS/Spacetime allowed)**
Includes:

* streaming progress
* UI projections
* non-binding recommendations
* intermediate verifier notes
* logs and telemetry delivery

Class 2 messages may be lost or duplicated without correctness impact, because they cannot mutate authority state.

---

## 4. State machines (normative)

This section defines canonical state machines. Implementations may add internal sub-states for operational detail, but the external states and their receipts must remain consistent.

### 4.1 Payment state machine (Settlement Engine)

**Purpose:** execute a bounded, auditable payment on a settlement rail.

**States**

* `QUOTED`: a binding preflight exists (fee ceiling, expiry, constraints).
* `PAID`: the payment executed successfully; proof exists.
* `WITHHELD`: the system refused to execute due to expiry, caps, or breakers (not a transient “try later” without explanation).
* `FAILED`: execution attempted but did not succeed.

**Normative behavior**

* A payment MUST be preceded by a quote when the rail has uncertainty (LN routing, FX selection).
* `WITHHELD` MUST be used when policy or constraints prohibit execution (fee cap exceeded, quote expired, breaker active).
* `FAILED` MUST be used when execution is allowed but fails due to operational realities (routing failure, node error, provider error).
* Every terminal transition MUST emit a receipt referencing:

  * the quote receipt (if applicable)
  * the underlying settlement proof (preimage, txid, etc.) when PAID

### 4.2 Credit envelope state machine (Envelopes / CEP)

**Purpose:** allow bounded working capital and pay-after-verify flows without handing agents unscoped funds.

**States**

* `INTENT_CREATED`: requester declares need and scope.
* `OFFERED`: underwriting returns a priced offer (caps, fees, conditions).
* `ENVELOPE_ISSUED`: envelope exists with cap/expiry/scope.
* `COMMITTED`: envelope capacity is reserved against a specific provider payment.
* `SETTLED`: funds were paid to the provider under constraints.
* `EXPIRED`: envelope expired without settlement.
* `REVOKED`: envelope invalidated by policy/risk controls.

**Normative behavior**

* Envelope settlement MUST be tied to explicit conditions (often a verification verdict receipt hash).
* Envelope MUST specify the maximum spend and allowed destinations (payees, invoice domain, route class).
* Envelope MUST have a short expiry and MUST NOT roll automatically.
* Underwriting MUST be policy-driven and must be explainable via receipts.

### 4.3 Contract outcome state machine (Work Outcomes)

**Purpose:** bind “work + verification + settlement + optional warranty” into one enforceable lifecycle.

**States**

* `CREATED`: contract exists; terms are fixed and hashed.
* `FUNDED`: price is reserved or envelope issued.
* `BONDED`: required bonds are reserved (self-bond, underwriter, dispute bond if pre-required).
* `SUBMITTED`: worker delivered outputs with artifact digests and evidence refs.
* `VERIFYING`: verification is executing under the declared plan.
* `VERDICT_PASS` / `VERDICT_FAIL`: verdict is finalized and receipted.
* `SETTLED`: payment/refund executed according to verdict and terms.
* `WARRANTY_ACTIVE`: optional window during which claims can be filed.
* `FINALIZED`: contract is closed; remaining bonds are released (unless slashed/drawn).

**Normative behavior**

* Settlement MUST NOT occur without a verdict if the contract is pay-after-verify.
* The verdict receipt MUST include achieved verification tier and evidence digests.
* If the contract includes a warranty, the warranty window MUST be explicit and bounded, and MUST specify remedies/caps.
* Bond releases and bond draws MUST be explicit authority actions with receipts.

### 4.4 Claim/dispute state machine (Liability)

**Purpose:** provide a standardized way to contest outcomes and trigger remedies.

**States**

* `OPEN`: claim filed with evidence refs and (optional) dispute bond.
* `UNDER_REVIEW`: verifiers/adjudicators are evaluating.
* `APPROVED` / `DENIED` / `PARTIALLY_APPROVED`: outcome of adjudication.
* `PAID`: payout executed (refund/damages/rework credit).
* `CLOSED`: claim is finished; bonds are released or forfeited.

**Normative behavior**

* Claims MUST be evidence-driven and must reference relevant receipts (contract terms hash, verdict receipt hash, settlement receipts).
* Dispute bonds MAY be required by policy to mitigate spam; this requirement must be declared by policy and reflected in receipts.
* Approved claims MUST map deterministically to bond draws and/or refunds with explicit settlement receipts.

### 4.5 Optional extension: Solver/cross-rail routing (Hydra X class)

This state machine is optional and must not be required for kernel correctness. If enabled, it must obey all invariants.

**States**

* `INTENT_CREATED → MATCHED → INITIATED → REDEEMED | REFUNDED | EXPIRED`

**Normative behavior**

* Matching decisions must be receipted and trace-linked.
* Atomicity guarantees (HTLCs, preimages) must be explicit. If not atomic, the refund and timeout behavior must be first-class states.

---

## 5. Receipts and canonical hashing (normative)

Receipts are the kernel’s foundational artifact. They are how autonomy becomes safe and auditable. Every downstream metric, reputation index, underwriting decision, and public health signal must derive from receipts.

### 5.1 Receipt envelope

Every receipt MUST include:

* `receipt_type`: stable string identifier for the receipt schema version
* `receipt_id`: unique identifier
* `created_at_ms`: epoch ms
* `canonical_hash`: digest of canonical receipt projection (see §5.3)
* `idempotency_key`: replay key that binds to the authority mutation
* `trace`: trace context (session, trajectory hash, job hash, and relevant ids)
* `policy`: `policy_bundle_id` at minimum
* `inputs_hash`: canonical hash of the normalized request inputs
* `outputs_hash`: canonical hash of normalized outputs (state changes, proofs, ids)
* `evidence[]`: evidence refs containing at least `digest` and `uri` where applicable

Receipts MAY also include tags/metadata that do not affect canonical hashing.

### 5.2 Cross-receipt linkage rules

Receipts must be linkable across the pipeline:

* A **verdict receipt** MUST reference:

  * `work_unit_id` and `contract_id`
  * artifact digests for submitted outputs (or pointers to a bundle with digest)
  * verification evidence digests (harness output, adjudication notes)
  * achieved tier

* A **settlement receipt** MUST reference:

  * the relevant `contract_id` when payment is contract-driven
  * the verdict receipt hash when settlement is gated by verification
  * the underlying rail proof where applicable (preimage/txid)

* A **bond receipt** MUST reference:

  * the related contract/work unit/claim
  * bond reason and party role
  * amount reserved/drawn/released

* A **claim resolution receipt** MUST reference:

  * contract terms hash
  * claim evidence bundle digests
  * which bonds were drawn and which settlement receipts executed

These linkage rules exist so any party can reconstruct “why money moved” without reading internal logs.

### 5.3 Canonical hashing rules

Canonical hashing MUST be stable across platforms and implementations.

At minimum:

* Canonical projection must:

  * normalize optional/empty fields
  * sort map keys
  * use stable field ordering
  * exclude non-normative metadata/tags
* Hash algorithm must be stable (e.g., `sha256`) and encoded as `sha256:<hex>`.

Two hashes are important and distinct:

* `inputs_hash`: hash over normalized request payload and binding references (e.g., quote id)
* `canonical_hash`: hash over normalized receipt fields (including outputs)

Any field that can vary nondeterministically (timestamps outside `created_at_ms`, log ordering, transient errors) must not affect canonical_hash unless it is explicitly part of the effect.

### 5.4 Idempotency and error semantics

* If a request with an `idempotency_key` has already completed, the kernel MUST return the same receipt (or a stable reference) regardless of retry timing.
* If a request is replayed with the same `idempotency_key` but different inputs, the kernel MUST reject it with a deterministic “idempotency conflict” error and emit (or reference) an error receipt or typed error response.
* “Silent partial success” is not allowed. Partial outcomes must be representable as explicit states and receipts.

---

## 6. Kernel modules (normative APIs and semantics)

This section defines the kernel modules and their authoritative responsibilities. The normative wire contract lives in `proto/openagents/**/v1/` and generated crates; this section describes required semantics rather than duplicating proto.

### 6.1 Settlement Engine (Lightning-first)

**Responsibility:** execute payments on supported rails with bounded cost and explicit proofs.

**Normative semantics**

* Must support `quote → execute` for rails with uncertain fees/latency.
* Must provide explicit `WITHHELD` behavior for expired quotes, fee cap violations, and active breakers.
* Must emit settlement receipts referencing underlying rail proofs.

**Canonical API posture**

* Quote: binds fee cap, expiry, urgency, and trace/policy context.
* Execute: uses quote id (or embedded quote hash) and returns a receipt.
* Status: returns terminal status and references to receipts.

This corresponds directly to your existing LN `quote_pay`, `pay`, `status` surfaces.

### 6.2 Liquidity & Reserves (pool snapshots)

**Responsibility:** keep settlement reliable by maintaining liquidity health and exposing auditable snapshots.

**Normative semantics**

* Maintain partitioned accounting (liquidity, reserves, and other partitions as enabled).
* Track liquidity health indicators (inbound/outbound, channel health, failure rates) in a snapshot that can be used by routing and underwriting.
* If/when LP mode exists, mint/burn shares and produce signed snapshots; until then, operator funding is acceptable but must still produce the same health signals.

### 6.3 Credit Envelopes (bounded credit / CEP)

**Responsibility:** allow bounded working capital and pay-after-verify without unscoped funds.

**Normative semantics**

* Envelope must specify:

  * scope (allowed spend class)
  * cap
  * expiry
  * allowed destinations (payee constraints)
  * policy bundle linkage
* Settlement must be receipted and replay-safe.
* Underwriting must enforce per-agent exposure and breaker hooks.

This corresponds directly to your `intent`, `offer`, `envelope`, `settle`, and `health` surfaces.

### 6.4 Bonds & Collateral (ABP)

**Responsibility:** reserve, release, and draw collateral linked to contracts and claims.

**Normative semantics**

* Bond actions are authority mutations and must be HTTP-only and idempotent.
* Bond receipts must link to contract/work unit/claim and specify party role and reason.
* Bond draws may trigger downstream settlement (LN pay / on-chain tx); if so, the bond draw receipt must reference the settlement receipt.

### 6.5 Verification Engine (verifiability + tiered verification)

**Responsibility:** classify work and produce verdict receipts under explicit independence tiers.

**Normative semantics**

* Every WorkUnit must have a verification plan stating minimum required tier:

  * Tier O objective harness
  * Tier 2+ heterogeneous checking
  * Tier 3 adjudication for subjective lanes
  * Tier 4 human underwriting where required
* Verdict finalization is an authority act (because it gates money) and must be HTTP-only and receipted.
* The verdict receipt must include evidence digests sufficient to reproduce the decision in audit/replay.

### 6.6 Liability Engine (warranties, claims, adjudication)

**Responsibility:** make outcomes warrantable and internalize tail risk.

**Normative semantics**

* Warranty issuance must:

  * specify terms (window, remedy type, cap, exclusions)
  * link to collateral requirements
  * be receipted with a terms hash
* Claims must:

  * reference evidence bundles and relevant receipts
  * follow declared adjudication rules
  * resolve into explicit payout/denial outcomes with receipts
* Any payout must route through settlement and/or bond draw operations with receipts.

### 6.7 FX RFQ & Settlement (Treasury Agents)

**Responsibility:** convert budget units into settlement units while preserving budget guarantees and provenance.

**Normative semantics**

* RFQ must bind:

  * desired amount and asset pair
  * time bounds and selection policy constraints
* Quote selection must be explicit (no silent “best quote” without receipt).
* Settlement must be idempotent and replay-safe.
* Quote expiry behavior must be explicit: if a quote expires before settlement, the kernel must withhold and emit a withheld receipt rather than attempting silent reroutes.
* Provenance receipts must record:

  * rfq id, quote id, selection rationale, expiry behavior
  * and references to final settlement proofs

This maps directly onto your existing FX `rfq`, `quote`, `select`, `settle` endpoints and the deterministic/idempotent behavior you already implemented.

### 6.8 Routing & Risk (scoring, breakers, exposure)

**Responsibility:** produce deterministic routing decisions and enforce safety posture.

**Normative semantics**

* Routing decisions must be:

  * computed from snapshots + receipts + declared constraints
  * emitted as machine-readable outputs (score + expected fee + confidence + policy notes)
  * trace-linked and auditable
* Breakers must be explicit and appear in receipts and `/stats`.
* Exposure controls must exist at:

  * per-agent level
  * per-route/provider level (where applicable)
  * per-partition level (liquidity, credit, bonds)

---

## 7. Public observability contract (`/stats`) (normative)

### 7.1 Snapshot model

`/stats` is a public, operator-grade view of kernel health and economics. It exists to make the system legible to machines and operators.

**Normative requirements**

* `/stats` MUST be computed from receipts and signed snapshots (where relevant), not from live per-request calculations.
* `/stats` MUST be cached once per minute and served as the same snapshot to all viewers.
* The snapshot should be stored in a durable, queryable system (Convex) and delivered via realtime subscriptions. Clients must not poll.

### 7.2 Table-first schema (stable columns)

`/stats` must present stable tables whose column names are suitable for both humans and agents. A “table-first” schema is preferred over narrative dashboards because machines should be able to parse it.

The following tables are normative (names are illustrative; column names must be stable once shipped):

**Settlement Health**

* success rate (5m/1h/24h)
* median fee and p95 fee
* median latency and p95 latency
* withheld counts by reason
* failed counts by reason

**Liquidity Health**

* channel inbound/outbound totals
* channel count and connected peer count
* rebalance spend (24h)
* routing confidence bucket distribution

**Credit Envelopes**

* outstanding envelope commitments (count, sats/msats)
* envelope settle success rate
* envelope expiry/revoke counts
* per-agent exposure top-N (capped list)

**FX**

* RFQs, quotes, selections, settlements (5m/1h/24h)
* quote→settlement conversion
* spread bps avg/median
* withheld and failed settlement totals
* provider breadth

**Verification**

* verified share by tier (O/1/2/3/4)
* verification latency p50/p95
* drift indicators (increased low-tier share, increased dispute rate)

**Liability**

* warranties issued (count, coverage)
* bonded exposure total
* claims opened/resolved (24h)
* claims paid (amount) and rolling loss rate
* adjudication latency p50/p95

**Risk & Concentration**

* breaker states (global + module-specific)
* concentration metrics (top share / HHI where applicable)
* failure spike indicators

### 7.3 Data provenance

Every metric row in `/stats` must be derivable from:

* receipt streams (canonical, append-only)
* signed snapshots (for pool accounting where applicable)

If a metric cannot be derived from receipts/snapshots, it does not belong in `/stats`.

### 7.4 Compatibility and evolution

* Columns may be added over time, but existing columns must not change meaning without a version bump.
* If you need incompatible changes, publish a versioned table name or add a schema version field in the snapshot.

---

If you want, I can now turn this into a single Markdown file exactly as it would live in your repo (with front-matter, a glossary appendix, and explicit “MUST/SHOULD/MAY” language conventions), and I can also draft the “implementation doc” that cleanly replaces your current “Implementation Status Snapshot” sections without contaminating the normative kernel spec.
