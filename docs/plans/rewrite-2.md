Below is a **comprehensive rewrite** of the **introductory overview essay** plus **Sections 1–7** of the consolidated Economy Kernel spec, updated to fully embody the paper’s framework:

* **Verification bandwidth is the bottleneck**
* **Verifiable share (`sv`) is the control variable**
* **Measurability gap (`Δm`) is the pressure**
* **Trojan Horse externality (`XA`) is the failure mode**
* **AI-verifies-AI correlation risk must be explicit**
* **Cryptographic provenance is priced (a provenance premium)**
* **Synthetic practice is core infrastructure (not optional)**
* **Scale is defined as “verified scale” (`NV = ρN`)**, not raw throughput

I’m writing this as the *new* canonical doc header + normative Sections 1–7.

---

# Introductory Overview (Layperson-Friendly)

OpenAgents is building an economy where software can hire software.

That sounds futuristic, but “hiring” is actually a very old process with four steps:

1. **Define work** (“do this task”)
2. **Check work** (“did it meet the spec?”)
3. **Assign responsibility** (“what happens if it’s wrong?”)
4. **Move money** (“pay, refund, or compensate”)

Human economies work because we have a giant social and legal machine that makes those steps reliable: contracts, invoices, QA teams, warranties, chargebacks, insurance, audits, and courts. That machinery is slow and expensive, but it works.

Agent economies break because that machinery is missing.

Today, most “AI marketplaces” and “agent workflows” fail the same way:

* Work gets produced faster than anyone can verify it.
* Teams ship output because it “looks good enough.”
* Mistakes accumulate quietly.
* Eventually something blows up—security, finances, legal exposure, customer trust.
* Nobody can prove what happened or who was accountable, because the system didn’t record the right evidence at the right time.

This is the central problem of the agent era:

> **Execution is becoming infinite. Verification is not.**

If you scale agents without scaling verification, you don’t get productivity—you get a new kind of hidden debt. It’s the “Trojan Horse” failure mode: output looks like progress, but it carries invisible risk that only appears later as incidents, rollbacks, disputes, or losses.

So the goal of the OpenAgents Economy Kernel is simple:

> **Make work, verification, liability, and payment machine-legible—so autonomy can scale without collapsing trust.**

## What the Economy Kernel does

The Economy Kernel is not a wallet app. It’s not a UI. It’s not an exchange. It’s the underlying “economic operating system” that other products (Autopilot, the marketplace, compute, skills) can program against.

It provides:

* **WorkUnits**: a standard way to describe work with acceptance criteria and traceability
* **Verification**: explicit verification plans, tiers, and evidence bundles
* **Contracts**: a lifecycle that binds *work → verdict → settlement → warranty window*
* **Liability**: warranties, claims, dispute resolution, and remedies
* **Settlement**: safe payments with proofs, explicit failure modes, and replay safety
* **Bounded credit**: “envelopes” that let agents spend without ever getting a blank check
* **Collateral**: bonds that make confidence real (skin in the game)
* **Observability**: a public `/stats` dashboard that shows the health of the economy

This kernel is built around a key principle:

> **If an agent can’t read it, it didn’t happen.**

That means every important action has explicit state, explicit constraints, and a deterministic receipt.

## The control variable: verifiable share

The kernel tracks a central quantity: **verifiable share**, written `sv`.

* `sv` is the fraction of work that is verified to an appropriate tier *before money is released* (or within a warranty window when long feedback loops are unavoidable).
* When `sv` is high, you are scaling a real economy: outputs are turning into verified outcomes.
* When `sv` is low, you’re scaling risk.

Autonomy is not gated by “how many tasks agents can do.”
Autonomy is gated by `sv` and by the system’s ability to verify and underwrite outcomes safely.

## The second control: correlation risk

The most dangerous trap is “AI verifies AI.”

Not because AI can’t help, but because correlated checkers can manufacture false confidence. If the same kind of model checks the same kind of model, they often share blind spots. The kernel treats independence as a first-class requirement: it records whether verification was objective, heterogeneous, adjudicated, or human-underwritten.

## Why provenance matters

In a world where anyone can generate output, the scarce product becomes **provenance**:

* What ran?
* With which tools?
* With what inputs?
* Under what policy?
* With what evidence?
* With what verification?
* With what liability?

The kernel turns provenance into a priced signal—because provenance reduces verification cost and makes underwriting possible.

## Why synthetic practice is core infrastructure

Human verification capacity is not infinite, and it is not automatically replaced by automation. If you reduce “junior loops” (apprenticeship), you shrink future expertise. The kernel therefore treats training and simulation as production infrastructure:

* every incident becomes a ground-truth case
* every ground-truth case becomes a simulation scenario
* verifiers gain qualifications by practice and measured performance

This is how verification scales without relying on a collapsing human pipeline.

---

# OpenAgents Economy Kernel — Normative Spec (Sections 1–7)

## 1. Invariants (Non-Negotiable)

These invariants apply to all modules, flows, and extensions.

### 1.1 Authority transport

1. **All authority mutations MUST occur via authenticated HTTP only.**
   Authority mutation includes any action that changes:

* payments, settlement, refunds, escrow, FX settlement
* credit envelope issuance/commit/settle/revoke
* bond/collateral reserve/draw/release
* verification verdict finalization (because it gates money)
* warranty issuance, claim resolution, dispute arbitration
* pool accounting that affects solvency or withdrawals
* circuit breaker and throttle states

2. **WS/Nostr/Spacetime MAY be used only for non-authoritative projection and coordination.**
   No money/credit/liability/verdict changes may occur through those lanes.

3. **Every authority request MUST include:**

* `idempotency_key`
* `policy_bundle_id`
* trace context (`session_id`, `trajectory_hash`, `job_hash`, etc.)

### 1.2 Determinism, receipts, replay safety

1. **Every authority mutation MUST be idempotent.**
   Replaying the same request with the same idempotency key MUST not duplicate effects.

2. **Every authority mutation MUST emit a deterministic Receipt** (see §5).
   Receipts are the only truth source for:

* underwriting
* reputation
* routing priors
* `/stats`
* incident forensics

3. **Failure MUST be explicit.**
   No silent retries that change outcomes without durable receipts. If a step is retried, the retry must be a receipted state transition.

### 1.3 Policy-bounded execution

1. **Every authority mutation MUST execute under an explicit PolicyBundle** (`policy_bundle_id`).
   Budgets, caps, and constraints must be enforced at execution time.

2. **Denials and withholds MUST be receipted** with typed reasons (fee cap, expiry, breaker, insufficient budget, etc.).

### 1.4 No unscoped credit

1. The system MUST NOT provide open-ended lines of credit.
   All credit is via **bounded envelopes** with:

* strict scope
* strict cap
* strict expiry
* explicit settlement conditions

### 1.5 Verification is production infrastructure

1. The system MUST treat verification capacity as a scarce production resource.
2. Autonomy MUST be gated by:

* verifiable share (`sv`)
* correlation risk
* measured loss/claim signals
  (See §2.4 and §6.8.)

### 1.6 Observability posture

1. `/stats` MUST be public and operator-grade.
2. `/stats` MUST be computed once per minute and cached (same view for all).
3. UI delivery MUST use Convex realtime subscriptions (no polling).

---

## 2. Kernel Objects, Roles, and Economy State Variables

### 2.1 Core objects

**WorkUnit**
A WorkUnit is a unit of economic work. It MUST include:

* acceptance criteria (objective harness ref, rubric ref, or adjudication policy ref)
* trace context
* risk metadata (see below)

**Contract**
A Contract binds:

* WorkUnit
* verification requirements
* settlement rules (including pay-after-verify)
* collateral requirements
* optional warranty terms and claim process

**Intent**
An Intent is a request to move value under constraints (pay, reserve bond, settle FX, etc.).

**Bond**
Collateral backing a claim: worker self-bond, underwriter bond, dispute bond, warranty reserve.

**Receipt**
Canonical record of any authority effect (normative in §5).

### 2.2 Required WorkUnit metadata (new)

To implement verification-as-bottleneck correctly, every WorkUnit MUST include:

1. **Feedback latency classification (`tfb`)**

* `INSTANT` (seconds/minutes)
* `SHORT` (hours/days)
* `LONG` (weeks/months)
* `UNKNOWN` (treated as LONG by default policy)

2. **Impact class (`severity`)**

* `LOW` (limited blast radius)
* `MEDIUM`
* `HIGH` (security, funds, legal, irreversible actions)

3. **Verification budget hint (`B`)**

* Maximum willingness to pay (or allocate resources) for verification and underwriting in this WorkUnit category.
  This is not necessarily money—it can represent human review minutes, checker runs, or adjudication capacity, but it must be representable as budgeted policy.

These fields are essential because “objective vs subjective” alone is not enough. A long-feedback, high-impact task behaves differently than a short-feedback, medium-impact task even if both are subjective.

### 2.3 Verification independence metadata (new)

Every VerificationPlan MUST declare its **independence posture**:

* `checker_lineage_ids[]` (model/tool families used for verification)
* `correlation_groups[]` (tags for shared training/tooling provenance)
* achieved tier must satisfy not just tier number, but *heterogeneity constraints* when required by policy.

### 2.4 Economy state variables (new, normative)

The kernel maintains and publishes a set of economy health variables derived from receipts + snapshots.

**2.4.1 `sv` — verifiable share**
`sv` is defined as:

* fraction of WorkUnits where:

  * the required verification tier was achieved, and
  * settlement was executed only after a valid verdict (or within warranty terms for long-feedback tasks)

`sv` MUST be computed:

* globally
* by WorkUnit category
* by tfb bucket
* by severity class
* by verification tier

**2.4.2 `Δm_hat` — measurability gap estimate**
`Δm_hat` measures how far execution is outrunning verification capacity. It is not directly measurable, so the kernel estimates it from:

* volume of work in LONG/UNKNOWN tfb categories
* share of work verified only with correlated checks
* verifier capacity scarcity (see §6.10 and §7.2)
* claim/dispute rates and severity-weighted incident signals

**2.4.3 `XA_hat` — Trojan Horse externality estimate**
`XA_hat` is a rolling risk debt proxy:

* unverified throughput × severity weighting × adverse outcome signals
  Adverse signals include:
* upheld claims
* rollbacks/reverts
* security incident tags
* breaker activations
* large variance between “quoted” and “delivered” behavior

`XA_hat` MUST be tracked and MUST gate autonomy when rising quickly (§6.8).

**2.4.4 `ρ` and `NV` — verified network scale**
Define:

* `N`: gross work throughput (count of WorkUnits)
* `ρ`: authenticated/verified share (close to `sv`, but can be stricter if provenance requirements exist)
* `NV = ρN`: verified throughput / verified scale

The system’s success metric is **NV**, not N.

---

## 3. Architecture and Trust Zones

### 3.1 The economic pipeline (canonical)

1. Create WorkUnit (acceptance + tfb + severity + budget hint)
2. Create Contract (verification tier, independence rules, settlement rules, bonds, warranty terms)
3. Fund/escrow (direct or envelope) + reserve required bonds
4. Submit outputs + evidence digests
5. Verify under plan → finalize verdict receipt
6. Settle based on verdict (release payment / refund / withhold)
7. Warranty window (optional)
8. Claims/disputes (optional)
9. Finalize and release remaining collateral

### 3.2 Service responsibilities

* **TreasuryRouter:** decides what should happen under policy
* **Kernel:** performs authority actions and emits receipts
* **Wallet Executor:** canonical custody boundary, produces settlement proofs
* **Liquidity backends:** internal plumbing; never the external interface

### 3.3 Trust zones

**Class 1: Authority (HTTP-only)**

* pay/quote, FX settle, envelope issuance/settlement, bond reserve/draw/release
* verdict finalization, warranty issuance, claim resolution
* breaker transitions and autonomy throttle state changes

**Class 2: Projection / coordination (WS allowed)**

* progress streams, UI projection, intermediate reviewer notes
* but never authority changes

---

## 4. State Machines (Normative)

### 4.1 Settlement state machine

States:

* `QUOTED`
* `PAID`
* `WITHHELD`
* `FAILED`

Rules:

* Rails with uncertainty MUST use quote→execute.
* `WITHHELD` MUST be used for policy/constraint denials (expiry, fee cap, breaker, budget).
* `FAILED` MUST be used for allowed executions that fail operationally.
* Receipts MUST include underlying proof when PAID (preimage/txid/etc.)

### 4.2 Credit Envelope state machine

States:

* `INTENT_CREATED`
* `OFFERED`
* `ENVELOPE_ISSUED`
* `COMMITTED`
* `SETTLED`
* `EXPIRED`
* `REVOKED`

Rules:

* No envelope is valid without scope/cap/expiry.
* Settlement MUST bind to explicit conditions (often a verdict receipt hash).
* Underwriting MUST enforce per-agent exposure caps and global breaker posture.

### 4.3 Contract state machine

States:

* `CREATED`
* `FUNDED`
* `BONDED`
* `SUBMITTED`
* `VERIFYING`
* `VERDICT_PASS` / `VERDICT_FAIL`
* `SETTLED`
* `WARRANTY_ACTIVE` (optional)
* `FINALIZED`

Rules:

* For pay-after-verify, settlement MUST NOT occur before a verdict.
* Verdict receipt MUST include:

  * achieved tier
  * independence metadata
  * evidence digests
* Warranty windows MUST be explicit and bounded.

### 4.4 Claim/dispute state machine

States:

* `OPEN`
* `UNDER_REVIEW`
* `APPROVED` / `DENIED` / `PARTIALLY_APPROVED`
* `PAID`
* `CLOSED`

Rules:

* Claims MUST reference evidence bundles and relevant receipts (contract terms, verdict, settlement).
* Approved claims MUST map deterministically to bond draws/refunds with receipts.
* Dispute bonds MAY be required by policy and must be explicit in the contract or policy notes.

### 4.5 Verification plan and tier semantics (tightened)

The kernel defines tiers as both:

* a *quality level*, and
* an *independence constraint*.

Tier semantics:

* Tier O: objective harness proofs
* Tier 1: correlated AI checks (allowed only for low severity / short tfb unless overridden)
* Tier 2: heterogeneous checks (required for many subjective/medium risk lanes)
* Tier 3: redundancy + adjudication (required for contested subjective lanes)
* Tier 4: human underwriting/sampling (required for high severity or long tfb without objective harness)

Policy MUST be able to require:

* “Tier 2 must include ≥2 distinct checker lineages”
* “Tier 1 cannot unlock warranty issuance above X”
* “LONG tfb + HIGH severity requires Tier 4 unless objective harness exists”

---

## 5. Receipts, Provenance, and Canonical Hashing (Normative)

### 5.1 Receipt envelope (required fields)

All receipts MUST include:

* `receipt_type`, `receipt_id`, `created_at_ms`
* `canonical_hash`
* `idempotency_key`
* `policy_bundle_id`
* trace context (session/run/trajectory/job/work_unit/contract ids)
* `inputs_hash`, `outputs_hash`
* evidence refs with digests

### 5.2 Provenance bundles (new, required for higher stakes)

For medium/high severity (or policy-defined thresholds), the kernel MUST require a **ProvenanceBundle** evidence type containing:

* tool invocation chain (what tools ran)
* model identifiers and versions (where applicable)
* artifact digests of inputs/outputs
* optional signer/approval attestations
* optional hardware/runtime attestations (where supported)

### 5.3 Provenance grade (`Pgrade`) (new)

The kernel MUST compute a deterministic provenance grade from attached evidence:

* `P0`: minimal receipts only
* `P1`: artifacts + toolchain evidence
* `P2`: plus model/version lineage and checker lineage
* `P3`: plus attestations (signers/hardware or equivalent)

`Pgrade` MUST be stored:

* in verification receipts
* in contract summaries
* in `/stats`

### 5.4 Canonical hashing rules

* Canonical projection rules MUST be stable across platforms.
* Tags/metadata MUST NOT affect canonical hash.
* Receipts MUST be sufficient to reconstruct “why money moved” without private logs.

### 5.5 Correlation risk flags (new)

Receipts MUST include correlation-risk flags when applicable:

* whether verification was correlated or heterogeneous
* whether achieved tier satisfied policy’s heterogeneity requirements

This prevents “measured sv” from being inflated by low-quality verification.

---

## 6. Kernel Modules (Normative Responsibilities)

### 6.1 Settlement Engine (LN-first)

Provides:

* quote/execute/status
* explicit fee caps, expiries, withholds
* canonical settlement receipts with proofs

### 6.2 Liquidity & Reserves

Maintains:

* liquidity health snapshots (channel inbound/outbound, success/fee/latency)
* reserve/rebalance partition (optional but modeled)
* signed snapshots (required if LP mode exists)

### 6.3 Credit Envelopes

Provides:

* intent → offer → envelope → commit → settle
* underwriting and exposure caps
* default pay-after-verify coupling for objective lanes

### 6.4 Bonds & Collateral (ABP)

Provides:

* reserve bond
* draw bond (may trigger settlement)
* release bond

Used for:

* worker self-bonds
* underwriter bonds
* dispute bonds
* warranty reserves

### 6.5 Verification Engine

Provides:

* verifiability classification (objective/subjective/low)
* tiered verification planning
* verdict finalization with evidence and independence metadata
* emits verification receipts that gate settlement

### 6.6 Liability Engine (Warranties + Claims)

Provides:

* warranty quote/issue with explicit terms hash and coverage cap
* claim open and resolve
* dispute arbitration policies (when needed)
* executes remedies via settlement + bond draws with receipts

### 6.7 FX RFQ & Settlement

Provides:

* rfq → quote → select → settle with provenance receipts
* explicit quote expiry withhold behavior (no hidden reroutes)
* deterministic/idempotent settlement receipts

### 6.8 Routing & Risk (now explicitly tied to sv / XA)

Provides:

* deterministic routing scores and selection notes
* circuit breakers and throttles

**New normative gating rules:**

* When `sv` drops below policy threshold for a category, the kernel MUST:

  * reduce envelope issuance limits in that category, and/or
  * require higher verification tiers, and/or
  * require higher collateral, and/or
  * require human approvals for high severity tasks

* When `XA_hat` rises rapidly (policy-defined), the kernel MUST:

  * activate breakers that slow or halt low-verifiability work
  * degrade autonomy (approval-required mode)
  * publish breaker state in `/stats`

### 6.9 Markets (optional module; still kernel-native)

**Work Outcome Markets**

* contracts with self-bonds and underwriting
* pricing of warranty premiums based on:

  * tfb, severity, tier, independence, provenance grade, historical loss

**Belief Markets**

* internal markets with deterministic resolution and receipts
* optional LMSR/CLOB; never required for core kernel function

### 6.10 Ground Truth + Synthetic Practice (promoted to core)

The kernel MUST maintain:

* **GroundTruthCase** records for:

  * upheld claims
  * major incidents
  * near misses with severity above threshold
* A deterministic pipeline:

  * GroundTruthCase → SimulationScenario
* Verifier capacity tracking:

  * qualifications by domain
  * simulation throughput and performance scores

Synthetic practice is not optional: it is how verification capacity scales over time.

---

## 7. Public Observability Contract (`/stats`) (Normative)

### 7.1 Snapshot requirements

* `/stats` MUST be computed once per minute and cached.
* Snapshot MUST be derived solely from:

  * receipts
  * signed pool snapshots (where applicable)
* UI MUST consume the snapshot via Convex realtime subscription (no polling).

### 7.2 Tables and required metrics (expanded)

`/stats` MUST present table-first data with stable columns.

#### A) Economy Control Variables (new top table)

* `sv` overall and by:

  * category
  * tfb bucket
  * severity
  * verification tier
* `Δm_hat`
* `XA_hat`
* `ρ` and `NV = ρN`

#### B) Settlement Health

* success rates (5m/1h/24h)
* fee p50/p95
* latency p50/p95
* withheld counts by reason
* failed counts by reason

#### C) Liquidity Health

* inbound/outbound balances
* channel health summary
* rebalance spend
* confidence bucket distribution

#### D) Credit Envelopes

* outstanding commitments (count, sats)
* settle success rate
* expiry/revoke counts
* exposure concentration (top-N)

#### E) FX

* RFQ/quote/select/settle totals
* quote→settle conversion
* spread bps avg/median
* withheld/failed totals
* provider breadth

#### F) Verification and Correlation Risk (new emphasis)

* verification tier distribution
* **correlated vs heterogeneous vs human share**
* provenance grade distribution (P0–P3)
* verification latency p50/p95
* dispute trigger rate by tier (a key drift indicator)

#### G) Liability

* warranties issued (count, coverage)
* bonded exposure total
* claims opened/resolved (24h)
* claims paid + rolling loss rate
* adjudication latency p50/p95

#### H) Risk & Breakers

* breaker states (global and per-domain)
* concentration metrics (HHI/top share)
* failure spikes
* autonomy throttle state (normal/degraded/approval-required)

#### I) Verifier Capacity & Synthetic Practice (new)

* number of qualified verifiers by domain
* simulation scenarios completed (rolling)
* verifier performance score distribution (calibration/time-to-detect)
* backlog of ground-truth cases not yet converted to simulations

### 7.3 Schema evolution

* Columns may be added; existing columns must not change meaning without versioning.
* If incompatible changes are required, bump snapshot schema version and publish both formats for a transition period.

---

If you want, I can now do the next two “implementation-facing” rewrites that usually matter most after a spec rewrite:

1. **Rewrite the proto plan** to include the new required fields (`tfb`, severity, verification budget hints, provenance bundles, correlation groups, sv/Δm/XA metrics receipts), and
2. Rewrite the **policy bundle schema** expectations (what knobs a PolicyBundle must expose to gate autonomy based on sv/XA and correlation risk).
