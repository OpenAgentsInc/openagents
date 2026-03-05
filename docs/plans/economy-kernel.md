# OpenAgents Economy Kernel — Normative Spec

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

# Sections 1–7 (Normative)

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

* `authenticated caller identity (service principal, agent identity, or operator identity)`
* `idempotency_key`
* `policy_bundle_id`
* `trace context (session_id, trajectory_hash, job_hash, etc.)`

#### 1.X Proto-first source of truth

The **normative wire contract** for the Economy Kernel is the proto schema under `proto/openagents/**/v1/` (and generated artifacts derived from it).
This markdown document defines **normative semantics** (state machines, invariants, receipt linkage, and required behaviors), but it MUST NOT become a second schema source of truth.

Rules:

1. **If the proto and the markdown disagree on wire shape, the proto wins.**
2. **Do not duplicate proto messages in this doc.** Reference the proto package and describe semantics here.
3. Any change to externally observable fields, states, or receipts MUST be implemented in proto first, then reflected here.

### 1.2 Determinism, receipts, replay safety

1. **Every authority mutation MUST be idempotent.**
   Replaying the same request with the same idempotency key MUST not duplicate effects and MUST return the same terminal result and the same receipt (or a stable reference to it).

2. **Every authority mutation MUST emit a deterministic Receipt** (see §5), including failure and withheld paths.
   Receipts are the only truth source for:

* underwriting
* reputation
* routing priors
* `/stats`
* incident forensics

3. **Failure MUST be explicit.**
   No silent retries that change outcomes without durable receipts. If a step is retried, the retry must be a receipted state transition.

4. **Any non-deterministic external dependency MUST be snapshotted or bound before execution.**
   Examples:

* `LN payments bind fee ceilings/expiry via quote → execute.`
* `FX binds selection via rfq → quote → select → settle.`
* `Verification binds the decision via a declared plan and evidence digests.`

### 1.3 Policy-bounded execution

1. **Every authority mutation MUST execute under an explicit PolicyBundle** (`policy_bundle_id`).
   Budgets, caps, and constraints must be enforced at execution time.

2. **Denials and withholds MUST be receipted** with typed reasons (fee cap, expiry, breaker, insufficient budget, etc.).

3. **Policy evaluation MUST be explainable.**
   Receipts MUST include policy notes (as evidence refs or stable tags) sufficient to explain why an action was allowed, denied, or withheld.

#### 1.X PolicyBundle minimum capabilities and flapping control

PolicyBundles are not cosmetic. At minimum, a PolicyBundle MUST be able to express:

1. **Verification and provenance requirements** by `category × tfb × severity`:

   * minimum verification tier
   * independence/heterogeneity constraints
   * minimum provenance grade / provenance bundle requirements
2. **Autonomy modes** (e.g., normal / degraded / approval-required / halt) and the conditions that trigger mode transitions.
3. **Envelope and bond knobs** by `category × severity`:

   * envelope caps, expiries, and settlement conditions
   * bond sizing rules (self-bond, underwriter bond, dispute bond)
4. **Breaker windows and anti-flapping controls**:

   * evaluation windows (e.g., rolling 5m/1h/24h)
   * hysteresis thresholds and/or minimum-duration requirements before triggering
   * cooldown periods after triggering to prevent oscillation

All policy-triggered mode changes, breaker transitions, and throttles MUST be receipted and visible in `/stats`.

#### 1.X PolicyBundle deterministic evaluation semantics (normative)

To prevent divergent “compliant” implementations, policy evaluation MUST be deterministic.

**Rule matching precedence**
When matching policy rules by `category × tfb × severity`, implementations MUST use the following precedence order:

1. exact category + exact tfb + exact severity
2. exact category + exact severity + wildcard tfb
3. exact category + wildcard severity + wildcard tfb
4. wildcard category + exact tfb/severity
5. wildcard everything (global default)

**Tie-breaking**
If multiple rules match at the same precedence, the implementation MUST choose deterministically (e.g., lexicographic by rule id). The chosen rule identifier MUST be recorded in receipts as policy notes/tags so auditors can reproduce the decision.

**Autonomy throttle action order**
When policy triggers multiple actions (mode changes, tier raises, provenance raises, envelope tightening, warranty disabling), they MUST be applied in this deterministic order:

1. autonomy mode transition (normal → degraded → approval-required → halt)
2. raise required verification tier / require human step
3. raise required provenance grade / require attestations
4. tighten or halt envelope issuance
5. disable warranties or cap warranty coverage

### 1.4 Credit and risk posture

1. The system MUST NOT provide open-ended lines of credit.
   All credit is via **bounded envelopes** with:

* strict scope
* strict cap
* strict expiry
* explicit settlement conditions

2. **Circuit breakers MUST be explicit and receipted.**
   Breakers are not hidden throttles. When a breaker changes behavior, it must be observable and explainable.

3. **Risk is partitioned, not implicit.**
   Pool and collateral exposures must be attributable to partitions (liquidity, credit, bonds, reserves) so subsidies and losses are legible.

### 1.5 Verification is production infrastructure

1. The system MUST treat verification capacity as a scarce production resource.
2. Autonomy MUST be gated by:

* verifiable share (`sv`)
* correlation risk
* measured loss/claim signals
  (See §2.4 and §6.8.)

#### 1.X Outcome resolution-path invariant

No outcome may become “settled truth” (i.e., unlock settlement, warranty issuance, or finalization) without an explicit resolution path: **objective harness**, **declared adjudication policy**, or **explicit human underwriting**. If resolution is ambiguous or long-latency, that ambiguity MUST be disclosed in the contract terms and priced via stricter tiers, collateral, or warranty exclusions.

### 1.6 Observability posture

1. `/stats` MUST be public and operator-grade.
2. `/stats` MUST be computed once per minute and cached (same view for all).
3. UI delivery MUST be subscription-driven via the system’s realtime sync mechanism (server-pushed updates), not polling.

#### 1.7 Non-goals (Normative)

The Economy Kernel MUST NOT evolve into any of the following:

1. **A token-dependent system.** No governance token, emissions, or token-gated safety properties are required for correctness or security.
2. **Opaque execution.** The kernel MUST NOT silently retry, reroute, batch, or net actions without explicit state transitions and receipts.
3. **Hidden fees.** All fees and spreads that affect settlement outcomes MUST be explicit in quotes, selections, and receipts.
4. **Client-side custody.** Client/UI code MUST NOT be entrusted with authority to move pooled funds; custody boundaries remain server-side and receipt-driven.
5. **Unscoped credit lines.** Credit remains bounded envelopes only (see §1.4 / §4.2).

#### 1.8 Normative language

The keywords **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are to be interpreted as normative requirements (RFC 2119-style). When in conflict, **MUST/MUST NOT** take precedence over **SHOULD/SHOULD NOT**.

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

### 2.1.1 Roles and authority boundaries (normative)

The Economy Kernel is operated by distinct roles. These roles exist to make authority boundaries explicit and auditable.

**Roles**

* **Buyer / Operator:** creates WorkUnits, initiates Contracts, funds escrow/envelopes, and chooses policy posture. May open claims.
* **Worker / Provider:** submits outputs and evidence; may post a self-bond and/or offer warranties (if policy allows).
* **Verifier:** performs verification steps and produces/verifies evidence; cannot directly move money. Verdict finalization is an authority action but is performed only under policy and must be receipted.
* **Underwriter / Predictor:** posts collateral to back warranties/outcomes; earns premiums when outcomes hold; pays when claims are upheld.
* **Claimant / Respondent:** parties to a claim/dispute (often Buyer vs Worker; Underwriter may become Respondent).
* **Adjudicator:** resolves disputed outcomes under a declared adjudication policy; claim resolution is an authority action and must be receipted.
* **Pool Operator:** operates liquidity backends and executes operational actions under strict policy (no ad-hoc admin); cannot bypass receipts.
* **Signer Set:** threshold-controlled authority for high-impact treasury actions (e.g., large withdrawals, pool parameter changes); actions must be explicit and receipted.

**Authority mapping**

* **Money movement (settlement/refunds/bond draws):** MUST be executed only by kernel authority endpoints under policy and MUST emit receipts.
* **Verification:** Verifiers may produce evidence, but **only the kernel’s verdict finalization** may unlock settlement/warranty.
* **Disputes:** Claims may be opened by Buyer/Operator (and others if policy allows), but resolution MUST be performed by Adjudicator policy lanes and must emit claim-resolution receipts.
* **Pool-impacting actions:** MUST be guarded by Signer Set thresholds when above policy-defined risk limits.

### 2.2 Required WorkUnit metadata (new)

To implement verification-as-bottleneck correctly, every WorkUnit MUST include:

0. **Work category (`category`)**

* A stable, machine-readable category label used for policy matching and `/stats` breakdowns (e.g., `compute`, `autopilot`, `legal`, `ops`, `security`).

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

**Kernel services are execution, not product UI.** The kernel is invoked by Autopilot, Marketplace, and TreasuryRouter; it does not define UI behavior.

**Wallet Executor is the custody boundary.** Kernel receipts may reference wallet-executor receipts as settlement proofs. The kernel must not assume payment occurred without canonical proof.

**Liquidity backends are plumbing, not the interface.** The kernel exposes quote/execute semantics and receipts; node ops and backend details remain internal.

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
* Quote linkage is mandatory. When a rail uses `quote → execute`, every terminal payment receipt (PAID / WITHHELD / FAILED) MUST reference the quote receipt (or quote hash) that bound fee ceilings and expiry.
* **Typed reason codes are mandatory.** Any `WITHHELD`, `FAILED`, `REVOKED`, `EXPIRED`, or denied transition MUST include a stable, machine-readable `reason_code` (not only prose), and that code MUST be recorded in the corresponding receipt.
* `WITHHELD` MUST be used for policy/constraint denials (expiry, fee cap, breaker, budget).
* `FAILED` MUST be used for allowed executions that fail operationally.
* Receipts MUST include underlying proof when PAID (preimage/txid/etc.)

### 4.1.1 Time semantics (deadlines and expiries)

1. **All expiries and deadlines MUST be absolute epoch milliseconds.** Relative durations may be used in policy, but authority actions must bind absolute `expiry_ms` / `deadline_ms` values into receipts.
2. **Post-expiry behavior is constrained.** Once an action’s bound expiry/deadline has passed:

   * settlement MUST transition to `WITHHELD` or `EXPIRED` (as appropriate),
   * the system MUST NOT “try anyway” under a stale quote/envelope,
   * and the receipt MUST include `reason_code = QUOTE_EXPIRED` / `ENVELOPE_EXPIRED` (or equivalent stable code).
3. **Expiry must be bound.** Any quote/envelope/selection that can expire MUST be explicitly bound to an expiry field and referenced by later receipts.

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
* Allowed destinations MUST be explicit. Every envelope MUST specify destination/payee constraints (e.g., whitelisted payees, invoice domain constraints, route class constraints). An envelope cannot be “cap only.”
* **Typed reason codes are mandatory.** Any `WITHHELD`, `FAILED`, `REVOKED`, `EXPIRED`, or denied transition MUST include a stable, machine-readable `reason_code` (not only prose), and that code MUST be recorded in the corresponding receipt.
* Settlement MUST bind to explicit conditions (often a verdict receipt hash).
* No rolling credit lines. Envelopes MUST be short-lived by default and MUST NOT roll automatically. Renewal requires an explicit new issuance under policy, with a new receipt.
* Commit binds an envelope to a specific settlement intent. COMMITTED state MUST identify the intended payee/recipient/invoice (or destination constraint snapshot) so settlement cannot drift into a different target.
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

### 4.6 Optional Extension: Solver / Cross-Rail Routing (Intent → Match → Settle)

This section defines an optional extension for intent-driven routing across rails and/or solver providers. It is **not required** for the core kernel to function and must remain **policy-gated**. No roadmap item depends on any third-party solver network openness.

**Lifecycle (minimum canonical state machine)**
`INTENT_CREATED → MATCHED → INITIATED → (REDEEMED | REFUNDED | EXPIRED | FAILED)`

* **INTENT_CREATED:** intent is accepted with constraints (asset pair, amount, deadline, max fee/slippage), trace context, and policy bundle.
* **MATCHED:** a solver/provider and route plan are selected.
* **INITIATED:** source-side initiation has occurred (e.g., HTLC created, on-chain tx broadcast, LN payment attempt started) and is receipted.
* **REDEEMED:** destination-side value transfer completes and is proven.
* **REFUNDED:** funds are returned due to timeout, failure, or policy-driven unwind.
* **EXPIRED:** intent times out before initiation or before completion under declared deadlines.
* **FAILED:** operational failure not recovered by refund semantics (must be explicit and receipted).

**Normative requirements**

1. **Match decisions MUST be receipted and trace-linked.**
   The match receipt must include solver/provider identity, route plan, constraint snapshot, and a stable explanation (“policy notes”) sufficient to audit why the match was selected.
2. **Atomicity and refund semantics MUST be explicit.**

   * If the route is atomic (e.g., HTLC/preimage-based), receipts must encode the atomic primitive used and link proofs.
   * If the route is not atomic, the spec requires first-class **REFUNDED** and **EXPIRED** behavior with bounded timeouts, explicit compensating actions, and receipts for each transition.
3. **No silent reroutes.**
   Any reroute, retry, or fallback is a state transition with a receipt, not an implicit background behavior.

**Extension invariants (non-negotiable)**

1. **Intent/effect separation.** Intents are messages (requests and plans). **Settlement effects** (actual money movement, escrow changes, refunds) are authority actions and MUST occur only via authenticated HTTP with receipts.
2. **Adapter purity and provenance.** Rail adapters MUST yield canonical receipts for every external action they cause. Any helper/relayer calls MUST be receipted with provenance (what was called, why, and what proof came back).
3. **Solver accountability from receipts.** Solver performance MUST be measured from receipts (fills, latency, failures, refunds, slippage vs quoted). Solver selection and continued eligibility MUST be policy-driven using those receipt-derived metrics.
4. **Failure is explicit.** Refunds, timeouts, expiries, and retries are first-class states with receipts. No hidden “best effort” reroutes.

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

Receipts MAY include non-normative tags/metadata that do not affect canonical hashing.

### 5.2 Receipt immutability and correction

1. **Receipts are append-only.** Once emitted, a receipt MUST NOT be mutated or overwritten.
2. **Corrections are new receipts.** If an earlier receipt must be superseded (e.g., reclassification, post-facto evidence, administrative reversal), the system MUST emit a new receipt that:

   * references the prior receipt(s) by `ReceiptRef`,
   * explains the correction as evidence/policy notes,
   * and results in an explicit state transition (never silent edits).
3. **Receipts are durable.** Receipts MUST remain retrievable by `receipt_id` for audit and replay (retention policy may be time-bounded, but must be explicitly documented and consistent).

### 5.3 Cross-receipt linkage rules (normative)

Receipts must form a navigable graph so any party (human or agent) can answer: **what happened, why, and what evidence justified it** without private logs.

The following linkages are REQUIRED:

**A) Verdict / Verification receipt MUST reference**

* `work_unit_id` and `contract_id`
* achieved verification tier and independence report (including correlation flags)
* artifact digests for submitted outputs (or a pointer to a bundle with digest)
* verification evidence digests (objective harness output, adjudication notes, checker summaries)
* computed provenance grade (when provenance requirements apply)

**B) Settlement (payment/refund) receipt MUST reference**

* `contract_id` if contract-driven
* the verdict receipt hash when settlement is gated by verification
* the quote receipt reference when the rail uses `quote → execute`
* the underlying rail proof when PAID (e.g., LN preimage, on-chain txid)

**C) Bond (reserve/release/draw) receipt MUST reference**

* related `contract_id` / `work_unit_id` / `claim_id` (as applicable)
* bond party role and bond reason
* amount reserved/drawn/released and remaining available amount after the action

**D) Claim resolution receipt MUST reference**

* contract terms hash (warranty terms + exclusions + window)
* evidence digests submitted with the claim
* which bonds were drawn (bond ids + amounts) and which were released
* which settlement receipts executed (refunds, damages, rework credits), as receipt references

**Transitive navigability requirement**
Given any settlement receipt, it MUST be possible to reach (by following receipt references) the contract terms, the governing verdict (if any), and the evidence digests that justified the verdict and/or claim resolution.

### 5.4 Provenance bundles (new, required for higher stakes)

For medium/high severity (or policy-defined thresholds), the kernel MUST require a **ProvenanceBundle** evidence type containing:

* tool invocation chain (what tools ran)
* model identifiers and versions (where applicable)
* artifact digests of inputs/outputs
* optional signer/approval attestations
* optional hardware/runtime attestations (where supported)

### 5.5 Provenance grade (`Pgrade`) (new)

The kernel MUST compute a deterministic provenance grade from attached evidence:

* `P0`: minimal receipts only
* `P1`: artifacts + toolchain evidence
* `P2`: plus model/version lineage and checker lineage
* `P3`: plus attestations (signers/hardware or equivalent)

`Pgrade` MUST be stored:

* in verification receipts
* in contract summaries
* in `/stats`

### 5.6 Canonical hashing rules (concrete)

Canonical hashing MUST be stable across platforms, languages, and execution environments.

Minimum requirements:

1. **Normalize empty/optional fields.**
   Empty strings, unset optionals, and empty arrays must canonicalize deterministically (no “sometimes absent, sometimes empty” ambiguity).
2. **Stable ordering.**

   * Field order must be deterministic.
   * Map keys MUST be sorted.
   * Repeated fields that represent sets MUST be sorted by a deterministic key (or encoded as ordered lists by definition).
3. **Exclude non-normative metadata.**
   Any `tags`, UI metadata, debugging notes, or non-normative attributes MUST NOT affect `canonical_hash`.
4. **Separate hashes with distinct meanings.**

   * `inputs_hash` = hash of normalized request inputs (including bound references like quote id).
   * `outputs_hash` = hash of normalized outputs (state changes, proofs, ids).
   * `canonical_hash` = hash of the normalized receipt envelope + normative payload.
5. **No nondeterministic fields.**
   Fields that can vary nondeterministically (log ordering, transient error strings, internal timestamps beyond `created_at_ms`) MUST NOT affect canonical hashing.
6. **Hash encoding.**
   All digests MUST be encoded as `sha256:<hex>` (or a versioned equivalent) consistently.

### 5.7 Idempotency conflict and error semantics

Idempotency is not “best effort”; it defines replay safety.

1. **Same key, same result.**
   If a request with an `idempotency_key` has already completed, the kernel MUST return the same terminal result and the same receipt (or a stable reference to it) regardless of retry timing.
2. **Same key, different inputs MUST deterministically error.**
   If the same `idempotency_key` is reused with materially different inputs, the kernel MUST return a deterministic `IDEMPOTENCY_CONFLICT` error and MUST NOT execute any effect.

   * **Idempotency conflicts must be coded.** `IDEMPOTENCY_CONFLICT` MUST be returned as a typed error with a stable `reason_code` and MUST reference the original receipt or action id that claimed the idempotency key.
3. **No silent partial success.**
   Partial outcomes must be representable as explicit states with receipts. The kernel MUST NOT “partially do the thing” and then return an opaque error.
4. **Terminality is explicit.**
   If an action is already finalized (e.g., contract already settled, claim already resolved), replay attempts MUST return a deterministic “already finalized” response, not a new effect.

**Idempotency key scope (normative)**
Idempotency MUST be evaluated within a deterministic scope consisting of:

* the authority endpoint/action name, and
* the authenticated caller identity.

Reusing an `idempotency_key` outside this scope MUST NOT collide. Reusing the same key within this scope with different normalized request inputs MUST trigger `IDEMPOTENCY_CONFLICT`.

### 5.8 Correlation risk flags (new)

Receipts MUST include correlation-risk flags when applicable:

* whether verification was correlated or heterogeneous
* whether achieved tier satisfied policy’s heterogeneity requirements

This prevents “measured sv” from being inflated by low-quality verification.

### 5.9 Reason codes (normative)

Any `reason_code` referenced in this spec MUST come from a **versioned, documented, machine-readable code set** (e.g., an enum in the proto wire contract or an equivalent versioned registry). Reason codes MUST be stable over time; new codes may be added, but existing codes MUST NOT change meaning.

---

## 6. Kernel Modules (Normative Responsibilities)

### 6.1 Settlement Engine (LN-first)

Provides:

* quote/execute/status
* explicit fee caps, expiries, withholds
* canonical settlement receipts with proofs
* quote linkage (when `quote → execute` is used, terminal receipts MUST reference the quote receipt or quote hash)

### 6.2 Liquidity & Reserves

Maintains:

* liquidity health snapshots (channel inbound/outbound, success/fee/latency)
* reserve/rebalance partition (optional but modeled)
* signed snapshots (required if LP mode exists)

#### 6.2.1 LP-mode solvency and withdrawal guardrails (normative)

Operator-funded pools are permitted. However, if external liquidity providers (LPs) can deposit/withdraw, the following guardrails are mandatory:

1. **Partitioned accounting is enforced.** Liquidity, credit exposure, bonds/collateral, and reserves MUST be tracked in distinct partitions so risk is legible.
2. **Share mint/burn and marking posture is explicit.** If LP shares exist, the rules for minting/burning shares and updating share price MUST be explicit and receipted (no opaque revaluations).
3. **Withdrawals are queued and throttled.** LP withdrawals MUST use a queue with bounded delay and MUST be throttleable based on:

   * solvency bands / reserve thresholds
   * outstanding envelope commitments
   * outstanding bonded exposure
   * active breakers or failure spikes
     Throttle state must be visible in `/stats`.
4. **High-impact treasury actions require Signer Set thresholds.** Actions such as large withdrawals, pool parameter changes, channel open/close sweeps, and reserve reallocation above policy thresholds MUST require Signer Set approval and must emit receipts.
5. **No hidden insolvency.** If solvency bands are breached, the system MUST enter an explicit breaker mode that halts or throttles withdrawals and publishes the state in `/stats`.

### 6.3 Credit Envelopes

Provides:

* intent → offer → envelope → commit → settle
* underwriting and exposure caps
* default pay-after-verify coupling for objective lanes
* destination/payee constraints (envelopes cannot be “cap only”)
* non-rolling issuance posture (envelopes MUST NOT roll automatically)

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
* RFQ binding is required (amount, asset pair, deadline/expiry, selection constraints)
* quote selection is explicit (no silent “best quote”; selection is an action with a receipt under policy)
* provenance receipts must record selection rationale (“policy notes”) and settlement proof references
* explicit quote expiry withhold behavior (no hidden reroutes)
* deterministic/idempotent settlement receipts

### 6.8 Routing & Risk (now explicitly tied to sv / XA)

Provides:

* deterministic routing scores and selection notes
* circuit breakers and throttles

**Additional normative requirements:**

* Routing outputs are machine-readable. The routing scorer MUST output at least:

  * expected fee (or fee range)
  * confidence / success probability estimate
  * liquidity/health score (where applicable)
  * explicit policy notes explaining constraints and blockers
    These outputs MUST be trace-linked, and any “selected route” decision SHOULD emit (or reference) a receipt so it’s auditable.
* Exposure controls exist at three granularities:

  1. per-agent exposure (credit + settlement rate limits)
  2. per-route/provider exposure (solver/provider caps, peer caps, FX provider caps)
  3. per-partition exposure (LLP vs CEP vs ABP vs reserves)
     Breaches MUST trigger explicit withholds/breakers with receipts and must be visible in `/stats`.

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

* All `sv`/`XA_hat`-driven gating actions MUST be bound to a specific `/stats` snapshot (by `snapshot_id`/`snapshot_hash`) and that binding MUST be included in the emitted receipts.

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

**GroundTruthCase linkage requirements (audit-grade)**
Every GroundTruthCase MUST link to:

* replay artifacts (or deterministic replay bundle digests)
* the relevant verification receipts (verdicts and checker evidence)
* the relevant settlement receipts (payments/refunds/bond draws/releases)
* the PolicyBundle id and version in effect at the time of the event
* any follow-on correction/supersession receipts (if applicable)

A SimulationScenario derived from a GroundTruthCase MUST reference the GroundTruthCase id and the same underlying evidence digests.

Synthetic practice is not optional: it is how verification capacity scales over time.

### 6.11 Reputation index (receipt-derived priors)

Reputation is an internal measurement derived strictly from receipts. It is not social scoring and MUST NOT be treated as a governance or identity system.

**Normative constraints**

1. **Receipt-derived only.** Reputation signals MUST be computed only from receipted outcomes (settlement success/latency, envelope settle integrity, claim/chargeback rates, slippage vs quote, dispute outcomes, failure spikes).
2. **Used only as priors.** Reputation MAY be used as an input prior for:

   * routing decisions
   * envelope underwriting limits
   * warranty pricing and eligibility
     It MUST NOT directly unlock authority mutations without policy checks.
3. **Explainable and auditable.** Any decision that materially depends on reputation SHOULD include a receipt note pointing to the relevant measurement window(s) and the metrics used.
4. **Non-transferable meaning.** Reputation metrics are contextual (category/tfb/severity) and MUST NOT be collapsed into a single “trust score” that ignores domain differences.
5. **Interop is summary-only.** If mirrored externally, only summary labels/aggregates may be exported; raw internal evidence pointers must not be exposed.

---

## 7. Public Observability Contract (`/stats`) (Normative)

### 7.1 Snapshot requirements

* `/stats` MUST be computed once per minute and cached.
* Snapshot MUST be derived solely from:

  * receipts
  * signed pool snapshots (where applicable)
* The minute snapshot MUST be persisted durably and retrievable by snapshot id for audit/replay.
* UI MUST consume the snapshot via the system’s realtime subscription mechanism (server-pushed updates), not polling.
* Snapshot computation MUST be **idempotent and receipted** (authority-like): recomputing the same minute snapshot must yield the same `snapshot_id`/`snapshot_hash` (or a stable reference) and must not produce conflicting snapshots for the same time boundary.
* Any breaker activation, autonomy mode transition, or `WITHHELD/EXPIRED` decision driven by `sv`, `Δm_hat`, `XA_hat`, or correlated-verification share MUST reference the **specific** `snapshot_id` and `snapshot_hash` used to make that decision (recorded in the corresponding receipt).
* The snapshot time boundary MUST be deterministic: `as_of_ms` MUST be rounded down to the start of the minute (UTC), and the same boundary MUST be used across the system.

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

### 7.3 Data provenance rule (hard requirement)

Every metric row published in `/stats` MUST be derivable from:

* receipt streams (canonical, append-only), and
* signed snapshots (where applicable for pool accounting).

If a metric cannot be derived from receipts/snapshots (or cannot be explained by them), it does not belong in `/stats`.

### 7.4 Public data safety and redaction

`/stats` is public, but it MUST NOT leak secrets or sensitive payloads. Therefore:

1. `/stats` MUST NOT include raw payment proofs (e.g., LN preimages), raw invoices, private evidence URIs, or any credential material.
2. `/stats` SHOULD publish only aggregated metrics and, when identifiers are necessary (e.g., top-N concentration), MUST publish only policy-approved identifiers (e.g., public agent ids) or hashed/pseudonymous forms.
3. Any metric requiring access to non-public evidence MUST be represented in `/stats` only via aggregate counts or derived indicators, not direct links.

### 7.5 Schema evolution

* Columns may be added; existing columns must not change meaning without versioning.
* If incompatible changes are required, bump snapshot schema version and publish both formats for a transition period.
