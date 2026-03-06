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

---

## Why this problem is bigger than “AI agents”

The problem is not only about AI agents.

It is about the **entire economic substrate that AI systems will run on.**

As AI becomes more capable, two things are happening simultaneously:

First, **software labor is exploding.**
Agents can now write code, analyze data, generate artifacts, run workflows, and operate systems.

Second, **compute is becoming the most important industrial input in the world.**
Modern AI systems run on enormous GPU clusters, specialized accelerators, and distributed inference infrastructure. As demand rises, compute capacity itself becomes a scarce commodity.

These two forces converge into a new economic structure:

* **AI agents produce work**
* **Compute infrastructure powers the work**
* **Markets allocate both labor and capacity**

Historically, economies developed specialized institutions to coordinate these layers.

* Payment rails like **Visa and Stripe** move money.
* Cloud platforms like **AWS** allocate compute.
* Commodity exchanges like **CME** price scarce resources.
* Insurance markets like **Lloyd’s of London** underwrite risk.

In the agent economy, these layers begin to merge.

Software agents must be able to:

* buy compute
* sell work
* verify outcomes
* hedge risk
* insure performance
* settle payments

All automatically.

This is why OpenAgents is not building just a marketplace or just an automation platform.

The Economy Kernel is intended to become the **economic substrate for machine work**.

You can think of it as a combination of:

* **Stripe** — programmable settlement and payment proofs
* **AWS** — programmable compute and service capacity
* **CME** — programmable commodity markets and price discovery
* **Lloyd’s** — programmable underwriting and liability markets

All built around **deterministic receipts, policy gating, and verifiable outcomes.**

---

## What the Economy Kernel does

The Economy Kernel is not a wallet app. It’s not a UI. It’s not an exchange.

It is the underlying **economic operating system** that other products (Autopilot, the marketplace, compute, skills) can program against.

It provides:

* **WorkUnits**: a standard way to describe work with acceptance criteria and traceability
* **Verification**: explicit verification plans, tiers, and evidence bundles
* **Contracts**: a lifecycle that binds *work → verdict → settlement → warranty window*
* **Liability**: warranties, claims, dispute resolution, and remedies
* **Settlement**: safe payments with proofs, explicit failure modes, and replay safety
* **Bounded credit**: “envelopes” that let agents spend without ever getting a blank check
* **Collateral**: bonds that make confidence real (skin in the game)
* **Observability**: a public `/stats` dashboard that shows the health of the economy

And, through extensions described later in this document:

* **Compute markets** that allocate scarce GPU capacity
* **Forward and futures instruments** that hedge future compute demand
* **Coverage markets** that price the risk of incorrect outcomes
* **Indices and metrics** that make the health of the agent economy measurable

The kernel itself does not implement user interfaces or application workflows. Instead, it exposes a deterministic economic substrate that higher-level products can rely on.

This kernel is built around a key principle:

> **If an agent can’t read it, it didn’t happen.**

That means every important action has explicit state, explicit constraints, and a deterministic receipt.

## The five-market taxonomy

The OpenAgents Marketplace consists of five interlocking markets built on this kernel:

- `Compute`
- `Data`
- `Labor`
- `Liquidity`
- `Risk`

This normative spec defines the shared kernel semantics that all five markets must terminate in: contracts, verification, liability, settlement, policy, and receipts.

### Status legend

- `implemented`: shipped in the current MVP or repo entry points
- `local prototype`: modeled in desktop-local kernel receipts, snapshots, or protocol notes, but not yet backed by authoritative backend services
- `planned`: target architecture, not yet shipped as a production market

### Market-to-object map

| Market | Purpose | Kernel-facing objects | Current repo status |
| --- | --- | --- | --- |
| `Compute` | Allocate machine capacity. | `ComputeProduct`, `CapacityLot`, `DeliveryProof`, `ComputeIndex`, `CapacityInstrument` | `implemented` for the compute-provider earn slice; `local prototype` for richer compute-market semantics; `planned` for full commodity instruments |
| `Data` | Price access to useful context under permission. | `DataAsset`, `AccessGrant`, `PermissionPolicy`, `DeliveryBundle`, `RevocationReceipt` | `planned` |
| `Labor` | Buy and sell machine work. | `WorkUnit`, `Contract`, `Submission`, `Verdict`, `Claim` | `local prototype` with a narrow MVP earn loop; `planned` as a generalized authoritative market |
| `Liquidity` | Move value between participants and rails. | `Quote`, `RoutePlan`, `Envelope`, `SettlementIntent`, `ReservePartition` | `local prototype`; `planned` as a production market |
| `Risk` | Price failure probability, verification difficulty, and liability. | `CoverageOffer`, `CoverageBinding`, `Claim`, `RiskSignal`, `CalibrationMetric` | `local prototype`; `planned` as a production market |

In current MVP terms, the visible product wedge is still compute-provider-first. Data, Liquidity, and Risk remain architectural markets rather than shipped product surfaces. Labor exists today as a narrow paid machine-work loop plus desktop-local kernel modeling, not yet as a generalized authoritative market.

Section 8 goes deepest on Compute because that is the first detailed market extension in this spec set. Companion docs describe the Data, Labor, Liquidity, and Risk surfaces. Their thinner treatment here reflects current maturity, not exclusion from the five-market architecture.

## The control variable: verifiable share

The kernel tracks a central quantity: **verifiable share**, written `sv`.

* `sv` is the fraction of work that is verified to an appropriate tier *before money is released* (or within a warranty window when long feedback loops are unavoidable).
* When `sv` is high, you are scaling a real economy: outputs are turning into verified outcomes.
* When `sv` is low, you’re scaling risk.

Autonomy is not gated by “how many tasks agents can do.”

Autonomy is gated by **how much of that work can be verified and insured**.

In other words:

> **The scaling limit of the agent economy is verification capacity.**

The Economy Kernel therefore treats verification as a **production resource**, not an afterthought.

## The second control: correlation risk

The most dangerous trap is “AI verifies AI.”

Not because AI cannot help verify work, but because **correlated checkers can manufacture false confidence**.

If the same model family checks the same model family, they often share blind spots.

The kernel therefore treats **verification independence as a first-class requirement**, recording:

* checker lineage
* correlation groups
* heterogeneity constraints
* adjudication steps

This ensures that “verified” actually means **independently verified**.

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

## Why compute markets belong in the same kernel

The same infrastructure that makes agent labor trustworthy also makes **compute markets trustworthy**.

Compute capacity is becoming a global commodity. But unlike oil or electricity, compute has complex characteristics:

* heterogeneous hardware
* variable performance
* geographic constraints
* unpredictable outages
* difficult verification

Without strong measurement and settlement infrastructure, compute markets are fragile.

The Economy Kernel solves this by treating compute capacity like any other economic object:

* capacity is defined by **standardized products**
* delivery is proven by **delivery proofs and cost attestations**
* failure triggers **bond draws and claims**
* market activity emits **deterministic receipts**

This allows the same substrate that coordinates agent labor to coordinate:

* spot compute markets
* forward capacity contracts
* GPU futures
* compute reservation options
* capacity indices

In other words, the kernel can coordinate both **machine labor** and the **machine infrastructure that powers it**.

## Why synthetic practice is core infrastructure

Human verification capacity is not infinite, and it is not automatically replaced by automation.

If you reduce “junior loops” (apprenticeship), you shrink future expertise.

The kernel therefore treats training and simulation as production infrastructure:

* every incident becomes a ground-truth case
* every ground-truth case becomes a simulation scenario
* verifiers gain qualifications by practice and measured performance

This is how verification scales without relying on a collapsing human pipeline.

---

Sections **1–7** below define the **core normative kernel** shared by all five markets.

Section **8** introduces the first detailed **compute-market extension** that allows the same infrastructure to power spot compute markets, forward capacity contracts, and hedging instruments.

Companion docs in `docs/kernel/` describe the Data, Labor, Liquidity, and Risk market surfaces and make their current implementation status explicit.

These extensions do not replace the kernel. They extend it.

Together they form a system capable of coordinating **software labor, compute capacity, and economic risk** in a unified, machine-readable economy.

## 1. Invariants (Non-Negotiable)

These invariants apply to all modules, flows, and extensions.

### 1.1 Authority transport

1. **All authority mutations MUST occur via authenticated HTTP only**, to TreasuryRouter or to the Kernel Authority API. (Clients such as Autopilot call TreasuryRouter; TreasuryRouter calls the Kernel Authority API.)
   Authority mutation includes any action that changes:

* payments, settlement, refunds, escrow, FX settlement
* credit envelope issuance/commit/settle/revoke
* bond/collateral reserve/draw/release
* verification verdict finalization (because it gates money)
* warranty issuance, claim resolution, dispute arbitration
* pool accounting that affects solvency or withdrawals
* circuit breaker and throttle states

2. **WebSocket, Nostr, and Spacetime MAY be used only for non-authoritative projection and coordination.** (Nostr = protocol for relays/identity/coordination; Spacetime = sync/presence/projection backend.)
   No money/credit/liability/verdict changes may occur through those lanes.

3. **Every authority request MUST include:**

* `authenticated caller identity (service principal, agent identity, or operator identity)`
* `auth_assurance_level` (for actions where policy requires identity gating)
* `credential/proof references` (for actions where policy requires personhood/org proof)
* `idempotency_key`
* `policy_bundle_id`
* `policy_version`
* `trace context (session_id, trajectory_hash, job_hash, etc.)`

#### 1.1.1 Proto-first source of truth

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
   Receipts MUST include policy notes as hash-bound fields and/or hash-bound `EvidenceRef` / `ReceiptRef` sufficient to explain why an action was allowed, denied, or withheld.

#### 1.3.1 PolicyBundle minimum capabilities and flapping control

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
5. **Identity assurance requirements** by `category x tfb x severity` and by role:

   * minimum authentication assurance level
   * whether personhood / org-vetting proofs are mandatory
   * trusted issuer/revocation trust-profile reference rules (or explicit external verification reference requirements)
6. **Monitoring and drift rules**:

   * required drift detectors/signals by slice
   * drift thresholds and policy actions when breached
7. **Liability pricing and insurability controls**:

   * liability premium/risk-charge parameters by slice
   * proof-of-coverage / disclosure preconditions where required
8. **Certification and rollback requirements**:

   * certification requirements and safe-harbor relaxations by slice
   * rollback/compensating-action plan requirements for high-severity lanes

All policy-triggered mode changes, breaker transitions, and throttles MUST be receipted and visible in `/stats`.

#### 1.3.2 PolicyBundle deterministic evaluation semantics

To prevent divergent “compliant” implementations, policy evaluation MUST be deterministic.

**Rule matching precedence**
When matching policy rules by `category × tfb × severity`, implementations MUST use the following precedence order:

1. exact category + exact tfb + exact severity
2. exact category + exact severity + wildcard tfb
3. exact category + wildcard severity + wildcard tfb
4. wildcard category + exact tfb/severity
5. wildcard everything (global default)

**Tie-breaking**
If multiple rules match at the same precedence, the implementation MUST choose deterministically (e.g., lexicographic by rule id). The chosen rule identifier MUST be recorded in receipts as hash-bound policy notes so auditors can reproduce the decision.

**Autonomy throttle action order**
When policy triggers multiple actions (mode changes, tier raises, provenance raises, envelope tightening, warranty disabling), they MUST be applied in this deterministic order:

1. autonomy mode transition (normal → degraded → approval-required → halt)
2. raise required verification tier / require human step
3. raise required provenance grade / require attestations
4. tighten or halt envelope issuance
5. disable warranties or cap warranty coverage

**Policy complexity and failure guardrails**

Implementations MUST enforce deterministic policy evaluation guardrails, including:

1. configured maximum active rules and/or maximum match candidates per evaluation,
2. configured maximum evaluation time budget per authority action,
3. deterministic failure behavior when limits are exceeded (`WITHHELD`/deny with stable reason code such as `POLICY_EVAL_LIMIT_EXCEEDED`, plus hash-bound policy evidence),
4. deterministic fallback posture (fail-closed for authority mutations unless policy explicitly declares a safer bounded fallback lane).

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

#### 1.5.1 Outcome resolution-path invariant

No outcome may become “settled truth” (i.e., unlock settlement, warranty issuance, or finalization) without an explicit resolution path: **objective harness**, **declared adjudication policy**, or **explicit human underwriting**. If resolution is ambiguous or long-latency, that ambiguity MUST be disclosed in the contract terms and priced via stricter tiers, collateral, or warranty exclusions.

### 1.6 Observability posture

1. `/stats` MUST be public and operator-grade.
2. `/stats` MUST be computed once per minute and cached (same view for all).
3. UI delivery MUST be subscription-driven via the system’s realtime sync mechanism (server-pushed updates), not polling.

### 1.7 Non-goals

The Economy Kernel MUST NOT evolve into any of the following:

1. **A token-dependent system.** No governance token, emissions, or token-gated safety properties are required for correctness or security.
2. **Opaque execution.** The kernel MUST NOT silently retry, reroute, batch, or net actions without explicit state transitions and receipts.
3. **Hidden fees.** All fees and spreads that affect settlement outcomes MUST be explicit in quotes, selections, and receipts.
4. **Client-side custody.** Client/UI code MUST NOT be entrusted with authority to move pooled funds; custody boundaries remain server-side and receipt-driven.
5. **Unscoped credit lines.** Credit remains bounded envelopes only (see §1.4 / §4.2).

### 1.8 Normative language

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

**CoverageBinding (market instrument)**
A CoverageBinding is a policy-gated liability instrument that binds underwriter offers, collateral, premium terms, and a deterministic resolution path to a specific contract/warranty window.

**CostProofBundle**
A CostProofBundle is a content-addressed evidence object for compute-integrity metering (resource usage + runtime/attestation linkage) that can be required by policy for compute-like lanes.

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

### 2.1.2 Verifier assignment, payout linkage, and liability posture (normative)

Verification assignment MUST be explicit, deterministic, and hash-bound before verdict finalization.

**VerificationAssignment modes**

* `POLICY_SELECTED_SET` (default if unspecified)
* `BUYER_SELECTED`
* `VERIFIER_MARKET` (optional lane, policy-gated)

**Required semantics**

1. Before `FinalizeVerdict`, the contract MUST reference an assignment object/receipt (or equivalent hash-bound reference) that includes assignment mode, assigned verifier/adjudicator identities (or market binding ref), and assignment timestamp.
2. Finalizing a verdict without assignment linkage MUST be denied/withheld with stable `reason_code = VERIFICATION_ASSIGNMENT_MISSING`.
3. Where verifier work is paid, payout receipts MUST link to both the assignment reference and the final verdict receipt.
4. Verifier compensation MUST debit the explicit `verification_fee` lane (or explicitly record zero-fee policy posture); implicit payouts are not allowed.
5. Verifier liability posture MUST be explicit per slice:
   * default posture: verifier is economically non-liable and reputationally accountable only,
   * optional posture: verifier bond/slash enabled by policy, with explicit reserve/draw/release receipts and stable slash reason codes.
6. If verifier bond/slash posture is enabled, contract/policy terms MUST include exposure cap and slash-policy linkage; otherwise verifier liability defaults to non-liable.

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
* `CRITICAL` (catastrophic blast radius; default policy should require Tier 4 or objective harness + strong provenance/attestation)

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

**2.4.5 Market-backed and cost-backed modifiers (new)**
The kernel MAY use two additional machine-legible modifiers to improve verification capacity and measurability where policy allows:

* **coverage-backed confidence modifiers** derived from bounded underwriting markets (coverage depth, concentration, calibration),
* **cost-integrity modifiers** derived from CostProofBundle evidence (attestation level, anomaly rates, variance bands).

These modifiers MUST be receipt-derived, policy-bounded, and traceable to snapshot evidence; they MUST NOT bypass explicit resolution-path requirements.

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

### 3.1.1 Matching boundary (normative)

By default, kernel authority semantics are post-match execution semantics.

1. Worker/verifier discovery or matchmaking protocols are out of kernel mutation scope unless an optional market module is explicitly enabled by policy.
2. Regardless of discovery path, authority actions MUST receive explicit worker and verifier-assignment identities/references as immutable request inputs.
3. Discovery/matching events MUST NOT mutate money, liability, verdict, breaker state, or snapshot state unless they are converted into authenticated HTTP authority actions with receipts.

### 3.2 Service responsibilities

* **TreasuryRouter:** decides what should happen under policy
* **Kernel:** performs authority actions and emits receipts
* **Wallet Executor:** canonical custody boundary, produces settlement proofs
* **Liquidity backends:** internal plumbing; never the external interface

**Where TreasuryRouter and the Kernel run.** They are **server-side services** (backend infrastructure), not on the user’s machine and not on Nostr or Spacetime. The desktop app (Autopilot) runs on the user’s computer and sends authority requests over HTTPS to TreasuryRouter; TreasuryRouter and the Kernel Authority API run in an environment the client can reach (e.g. operator-hosted or self-hosted). Nostr is used for coordination and identity; Spacetime for sync/presence. Authority lives only in that HTTP-accessible backend.

**How Autopilot connects to the kernel.** (1) The desktop app sends authority requests (create work, fund, submit, settle) over authenticated HTTPS to TreasuryRouter, which forwards them to the Kernel Authority API. (2) The app consumes the receipt stream and economy snapshots (today from local file and local compute; later from sync or kernel-published stats). (3) Progress and coordination use Nostr and Spacetime only—no money or verdicts over those channels.

**Runtime vs Kernel.** The OpenAgents Runtime is the worker-side execution environment where jobs run and provenance is produced. The kernel is distinct: it evaluates runtime-produced evidence, applies policy, settles value, and emits canonical receipts. Runtime-local execution state MUST NOT by itself finalize verdicts or mutate economic truth.

**Kernel services are authority execution, not product UI.** The kernel is invoked by Autopilot, Marketplace, and TreasuryRouter; it does not define UI behavior.

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
* **Typed reason codes are mandatory.** Any denied attempt, and any `WITHHELD` or `FAILED` transition, MUST include a stable, machine-readable `reason_code` (not only prose), and that code MUST be recorded in the corresponding receipt.
* `WITHHELD` MUST be used for policy/constraint denials (expiry, fee cap, breaker, budget).
* `FAILED` MUST be used for allowed executions that fail operationally.
* Receipts MUST include underlying proof when PAID (preimage/txid/etc.)

### 4.1.1 Time semantics (deadlines and expiries)

1. **All expiries and deadlines MUST be absolute epoch milliseconds.** Relative durations may be used in policy, but authority actions must bind absolute `expiry_ms` / `deadline_ms` values into receipts.
2. **Post-expiry behavior is constrained.** Once an action’s bound expiry/deadline has passed:

   * settlement MUST transition to `WITHHELD`,
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
* **Typed reason codes are mandatory for denials and terminal transitions.** Any denied issuance/commit/settle attempt, and any `REVOKED` or `EXPIRED` transition, MUST include a stable, machine-readable `reason_code` recorded in the corresponding receipt.
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

**Additional states (wire-visible)**

* `CLAIM_OPEN` MAY be used as a derived/summary state indicating at least one claim exists in `OPEN` or `UNDER_REVIEW`. It MUST NOT unlock settlement or alter verification requirements.
* `CANCELLED` MAY be used only under explicit policy-defined cancellation rules. Cancellation MUST be receipted and MUST NOT result in hidden netting; any refunds/withholds/bond releases required by cancellation MUST be explicit authority actions with receipts.

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
* Final truth in a dispute lane MUST come only from one declared path: objective harness output, declared adjudication policy, or explicit human underwriting.
* Unreceipted external updates MUST NOT silently finalize/override claim outcomes. Any external truth input must enter via receipted evidence objects and a receipted resolution action.

### 4.5 Verification plan and tier semantics (tightened)

The kernel defines tiers as both:

* a *quality level*, and
* an *independence constraint*.

Tier semantics:

* Tier O: objective harness proofs
* Tier 1: correlated AI checks (allowed only for low severity / short tfb unless overridden)
* Tier 2: heterogeneous checks (required for many subjective/medium risk lanes)
* Tier 3: redundancy + adjudication (required for contested subjective lanes)
* Tier 4: human underwriting/sampling (required for HIGH/CRITICAL severity or long tfb without objective harness)

Policy MUST be able to require:

* “Tier 2 must include ≥2 distinct checker lineages”
* “Tier 1 cannot unlock warranty issuance above X”
* “LONG tfb + HIGH/CRITICAL severity requires Tier 4 unless objective harness exists”

The remaining sections in §4 are optional extensions. Core kernel implementations do not require §4.6 or §4.7.

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

### 4.7 Optional Extension: Liability Markets and Cost Integrity

This section defines optional extensions that remain policy-gated:

* **MKT (liability markets):** coverage auctions and optional belief markets for bounded, receipted risk pricing.
* **cost integrity lane:** compute cost proofs as first-class evidence for pricing, drift, and fraud detection.

**Coverage Binding lifecycle (minimum canonical state machine)**
`OFFERING_OPEN → BINDING_PROPOSED → BOUND → ACTIVE → (CLAIM_TRIGGERED | EXPIRED) → SETTLED`

**Belief position lifecycle (optional later instrument)**
`OPEN → MARGIN_LOCKED → REDUCED/CLOSED → SETTLED`

**Normative requirements**

1. **Resolution MUST be contract-native and unambiguous.**
   Any market payoff or coverage settlement MUST bind to a deterministic `ResolutionRef` that resolves via objective harness evidence, verdict receipts, claim resolution receipts, or explicit human underwriting receipts.
2. **Coverage is bounded liability, not open credit.**
   Coverage offers/bindings MUST declare coverage caps, premium terms, warranty window binding, and collateral linkage via bond receipts.
3. **No non-receipted market transitions.**
   Offer placement, binding, activation, claim trigger, and settlement MUST be authority actions with receipts.
4. **Deterministic clearing is required.**
   If order/offer books are used, clearing MUST be deterministic. Recommended default is minute-batch clearing with canonical offer ordering by price, timestamp, and stable tie-break.
5. **Safe-harbor relaxations are policy-bounded.**
   Any verification/provenance relaxation using market-backed signals MUST be explicitly policy-triggered, bound to snapshot references, and receipted.

**Safe-harbor constraints (minimum checklist)**

Market-backed verification relaxations MAY be applied only when all required policy thresholds pass for the active slice (`category × tfb × severity`), including:

* minimum bound coverage cap,
* minimum distinct underwriters,
* no disallowed correlation-group violations,
* maximum implied failure probability,
* minimum market calibration score,
* maximum coverage concentration.

Any applied relaxation MUST be recorded as a policy action receipt and MUST reference the snapshot id/hash used for the decision.

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

### 5.1.1 Evidence immutability contract (normative)

1. Evidence referenced by receipts MUST resolve to immutable, content-addressed artifacts.
2. `EvidenceRef.digest` (for example `sha256:<hex>`) is authoritative; `EvidenceRef.uri` is only a transport pointer.
3. If retrieved bytes do not match the declared digest, the evidence is invalid for authority decisions and audit replay.
4. Evidence corrections/supersession MUST be represented as new evidence objects and new receipts; prior evidence links are never overwritten.

### 5.1.2 Evidence size and embedding guidance (normative)

1. Large evidence payloads SHOULD be externalized and referenced by digest + URI.
2. Receipts SHOULD avoid embedding large opaque payload bodies directly in normative fields.
3. When compact summaries are embedded, they MUST still link to the full externalized evidence artifact for replay/audit.

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

#### 5.5.1 CostProof bundles (compute-integrity evidence)

For compute-like WorkUnit lanes, policy MAY require a **CostProofBundle** evidence type.

Minimum requirements:

* cost proof MUST be content-addressed (`sha256:<hex>`) and trace-bound to `work_unit_id`/`contract_id`/`job_hash` as applicable,
* cost proof MUST include attestation posture (`C0_SELF_REPORTED`, `C1_METERED`, `C2_HARDWARE_ATTESTED`, optional `C3_ZK_PROVEN`),
* policy MAY require minimum cost-proof level by `category × tfb × severity`,
* cost anomalies MUST be representable as explicit drift/evidence signals (no hidden heuristics).

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

#### 5.6.1 Optional neutral-ledger anchoring (normative, policy-gated)

The kernel MAY publish snapshot hashes and/or receipt Merkle roots to a neutral public ledger (or equivalent immutable substrate) to increase cross-org trust.

If anchoring is enabled:

1. Anchoring actions MUST emit append-only anchoring receipts with:

   * anchored object reference (`snapshot_id` or receipt-root identifier)
   * anchored hash
   * external proof reference (txid/event id or equivalent)
2. Anchoring receipts MUST NOT include secrets or private payloads; hashes/proof references only.
3. Anchoring MUST be policy-gated and idempotent per anchored object.

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

The endpoint/action name MUST be canonicalized exactly as stored in the idempotency index (for example stable RPC full-name or normalized HTTP method+route). Equivalent aliases MUST normalize to the same action identity before idempotency lookup.

Reusing an `idempotency_key` outside this scope MUST NOT collide. Reusing the same key within this scope with different normalized request inputs MUST trigger `IDEMPOTENCY_CONFLICT`.

### 5.8 Correlation risk flags (new)

Receipts MUST include correlation-risk flags when applicable:

* whether verification was correlated or heterogeneous
* whether achieved tier satisfied policy’s heterogeneity requirements

This prevents “measured sv” from being inflated by low-quality verification.

### 5.9 Reason codes (normative)

Any `reason_code` referenced in this spec MUST come from a **versioned, documented, machine-readable code set** (e.g., an enum in the proto wire contract or an equivalent versioned registry). Reason codes MUST be stable over time; new codes may be added, but existing codes MUST NOT change meaning.

**Hashed-decision requirement (normative)**
Any data required by this spec to justify or reproduce a decision (e.g., `reason_code`, selected policy rule id(s), snapshot bindings, solver match selection, breaker trigger details) MUST be recorded in the **normative, hash-bound portion** of the receipt payload (i.e., it MUST affect `inputs_hash`/`outputs_hash` and thus `canonical_hash`), or be recorded as a hash-bound `EvidenceRef` / `ReceiptRef`. It MUST NOT be stored only in non-normative `tags`.

### 5.10 Interoperable audit exports (normative)

Anything required for underwriting, incident forensics, certification, or dispute replay MUST be exportable in an open, versioned format with deterministic hashing and explicit redaction tiers.

At minimum, export formats MUST cover:

* receipts and snapshot bindings
* incident/near-miss records and taxonomy codes
* outcome registry entries
* certification state transitions
* policy decision linkage needed to reproduce allow/deny/withhold outcomes

Public exports MUST use redacted aggregate-safe views; restricted exports MAY include additional references, but still MUST avoid secret credential material.

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

**Verifier assignment and payment requirements (normative)**

1. Verification execution MUST bind to an explicit assignment reference created before verdict finalization.
2. Verdict receipts MUST include hash-bound assignment linkage.
3. If verification compensation is enabled, settlement receipts MUST include verifier payout linkage and explicit `verification_fee` accounting.
4. Verifier liability posture (non-liable default or bond/slash-enabled) MUST be policy-addressable and receipted.

### 6.6 Liability Engine (Warranties + Claims)

Provides:

* warranty quote/issue with explicit terms hash and coverage cap
* claim open and resolve
* dispute arbitration policies (when needed)
* executes remedies via settlement + bond draws with receipts

**Insurance-boundary requirements (normative)**

1. Contracts MUST separate execution price from liability premium/risk charge.
2. Liability premium/risk charge MUST be policy-derived and reproducible from receipted inputs.
3. When pricing is dynamic, pricing receipts MUST bind to the specific snapshot window/hash used.
4. `/stats` MUST expose insurer-relevant aggregate metrics (premiums collected, claims paid, loss ratio, capital coverage posture) without leaking sensitive payloads.

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

Markets in this kernel are **liability and verification-capacity instruments**, not unbounded speculation.

**Instrument 1: Coverage auction (recommended first instrument)**

* underwriters post collateralized coverage offers for a contract/warranty lane,
* binding selects a deterministic set of offers into a CoverageBinding with explicit cap/premium terms,
* premiums and claim payouts are executed via authority receipts and bond draws/releases.

**Instrument 2: Belief market (optional later instrument)**

* bounded PASS/FAIL positions collateralized under policy,
* payoff resolution MUST bind to deterministic `ResolutionRef` outcomes,
* outputs include market-implied failure probability, confidence, and calibration signals.

**Normative requirements**

1. **Coverage and market actions MUST be receipt-native.**
   Placement, cancellation, binding, clearing, activation, and settlement are authority actions with receipts.
2. **Resolution references are mandatory.**
   Coverage and belief settlement MUST bind to objective/verdict/claim/human-underwrite resolution references.
3. **Concentration/correlation controls apply.**
   Policy MUST be able to enforce minimum underwriter diversity and concentration ceilings before safe-harbor relaxations are allowed.
4. **Deterministic clearing is mandatory.**
   When clearing books, implement deterministic ordering and emit clearing receipts.

**Verification-capacity posture**

Policy MAY treat market-backed coverage as a partial substitute for high-cost verification only when explicit thresholds pass (coverage cap, diversity, concentration, calibration, and implied risk posture). Any relaxation MUST be bound to snapshot ids/hashes and receipted.

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

**GroundTruthEvidence contract for long-latency lanes (normative)**
When truth arrives from delayed/external systems (for example chargeback windows, legal rulings, external audits), GroundTruthCase updates MUST include explicit GroundTruthEvidence objects with at least:

* `source_id` (who/what system produced the evidence),
* `evidence_ref` + digest (hash-bound),
* `received_at_ms` (when the kernel ingested it),
* `confidence/posture` metadata (for example final/provisional/contested),
* linkage to relevant `contract` / `verdict` / `claim` receipt refs.

GroundTruthEvidence objects MUST be append-only and receipted; late-arriving evidence may refine state but MUST NOT mutate prior receipts in place.

**Simulation derivation governance (normative)**
GroundTruthCase → SimulationScenario derivation MUST be machine-legible:

1. Derivation MUST emit a receipt (or equivalent hash-bound reference) identifying source incident digest(s), generator identity, and generation timestamp.
2. Validation/approval MUST emit a separate receipt (or hash-bound reference) identifying validator identity and validation posture.
3. Scenario exports MUST carry derivation and validation linkage so third parties can replay provenance without private side logs.

Synthetic practice is not optional: it is how verification capacity scales over time.

**Standardized incident reporting requirements (normative)**

GroundTruthCase and related incident records MUST be proto-first, versioned objects with:

* versioned incident taxonomy codes
* incident/near-miss classification
* mandatory linkage to receipts/evidence digests and policy version
* explicit rollback-attempt fields for lanes where rollback applies

Incident object lifecycle requirements:

* incident objects MUST be append-only and hash-addressed (`incident_digest`) with deterministic revisioning
* taxonomy key (`taxonomy_id + taxonomy_version + code`) meanings are immutable once registered
* lifecycle receipts MUST exist for report/update/resolve transitions and include hash-bound incident object linkage:
  * `economy.incident.reported.v1`
  * `economy.incident.updated.v1`
  * `economy.incident.resolved.v1`

Incident reporting and GroundTruthCase records are insurability prerequisites; implementations MUST NOT rely on unstructured postmortem prose as the sole source of truth.

### 6.11 Rollback and compensating actions (normative)

For high-severity categories, contracts MUST include at least one of:

* objective rollback plan
* compensating-action plan
* explicit human underwriting with disclosed exclusions

Rollback or compensating action attempts MUST emit explicit receipts (executed/failed) with stable reason codes and receipt linkage to the triggering incident/claim.

Required rollback receipt types:

* `economy.rollback.executed.v1`
* `economy.rollback.failed.v1`
* `economy.compensating_action.executed.v1`

For slices where rollback plans are required, missing rollback/compensating plans MUST withhold authority mutation with a stable reason code (`ROLLBACK_PLAN_REQUIRED`) and hash-bound policy evidence.

### 6.12 Monitoring and drift detection (normative)

The kernel MUST support continuous drift detection as a first-class control loop.

Minimum requirements:

1. Drift signals and alerts MUST be receipted as deterministic state transitions.
2. Drift-triggered policy actions MUST follow the same deterministic throttle action order defined in Section 1.
3. Drift-triggered actions MUST bind to the specific snapshot id/hash used.
4. Drift signals and alert rates MUST be visible in `/stats`.

Receipt requirements (normative):

* Implementations MUST emit:
  * `economy.drift.signal_emitted.v1`
  * `economy.drift.alert_raised.v1`
  * `economy.drift.false_positive_confirmed.v1`
* Drift receipt idempotency MUST be keyed by `(snapshot_id + detector_id + receipt_type)` so replaying the same snapshot window cannot produce duplicate drift state transitions.
* Every drift receipt MUST include hash-bound `snapshot_ref` linkage and detector linkage (`detector_id`, `signal_code`).
* Reason codes for drift transitions MUST be stable: `DRIFT_SIGNAL_EMITTED`, `DRIFT_ALERT_RAISED`, `DRIFT_FALSE_POSITIVE_CONFIRMED`.

`/stats` drift surface (normative):

* `drift_alerts_24h` (aggregate pressure used by policy gates)
* `top_drift_signals` with machine-readable rows:
  * `detector_id`
  * `signal_code`
  * `count_24h`
  * `ratio`
  * `threshold`
  * `score`
  * `alert`

### 6.13 Safety signals, certification, and interoperability (normative)

The kernel MUST support interoperability-focused public safety infrastructure:

* privacy-preserving safety signals (public aggregate + restricted sharing modes)
* certification objects with issuance/revocation receipts
* outcome registry entries linkable to verdict/settlement/incident records
* synthetic practice scenario packages exportable under policy-gated redaction rules

**Outcome registry privacy profile (normative)**

1. Public outcome-registry exports MUST include only redacted/aggregate-safe fields (for example slice keys, normalized outcomes, taxonomy buckets, counts, and non-sensitive timestamps).
2. Restricted exports MAY include linked receipt refs and evidence digests needed for independent verification, but MUST still exclude secret credentials, raw private payloads, and sensitive external identifiers unless policy explicitly permits and receipts record that allowance.
3. Export tier selection (`public` vs `restricted`) MUST be explicit, policy-gated, and receipted.

**Identity trust-profile boundary (normative)**

Trusted issuer lists and revocation-check responsibility MUST be explicit:

* either the kernel policy requires a trust-profile reference (`trust_profile_id` or equivalent) and enforces it in authority decisions,
* or trust verification is performed out-of-kernel, in which case authority receipts MUST include hash-bound proof references to the external trust/revocation check used.

### 6.14 Reputation index (receipt-derived priors)

Reputation is an internal measurement derived strictly from receipts. It is not social scoring and MUST NOT be treated as a governance or identity system.

**Normative constraints**

1. **Receipt-derived only.** Reputation signals MUST be computed only from receipted outcomes (settlement success/latency, envelope settle integrity, claim/chargeback rates, slippage vs quote, dispute outcomes, failure spikes).
2. **Used only as priors.** Reputation MAY be used as an input prior for:

   * routing decisions
   * envelope underwriting limits
   * warranty pricing and eligibility
     It MUST NOT directly unlock authority mutations without policy checks.
3. **Formula boundary is policy-defined.** Reputation formula, decay, and weighting are policy-defined and non-universal; implementations MUST NOT assume a global canonical reputation function.
4. **Explainable and auditable.** Any decision that materially depends on reputation MUST include receipt linkage to the relevant measurement window(s), metric set, and policy rule reference used.
5. **Non-transferable meaning.** Reputation metrics are contextual (category/tfb/severity) and MUST NOT be collapsed into a single “trust score” that ignores domain differences.
6. **Interop is summary-only.** If mirrored externally, only summary labels/aggregates may be exported; raw internal evidence pointers must not be exposed.

### 6.15 Cost Integrity and Proof-of-Cost (normative)

Proof-of-cost is a machine-legible compute-integrity signal, not proof-of-work.

Minimum requirements:

1. **CostProofBundle is first-class evidence.**
   For policy-gated compute lanes, submissions MUST include a cost proof reference with attestation level and resource-usage measurements.
2. **Cost proofs are trace-bound and hash-bound.**
   Cost proof evidence MUST bind to execution identity (work unit/contract/job trace) and be represented via immutable digests.
3. **Cost anomalies are explicit signals.**
   Significant variance between expected and measured cost MUST emit explicit anomaly/drift signals with receipts.
4. **Cost signals are policy inputs only.**
   Cost integrity MAY influence pricing, underwriting limits, and throttles, but MUST NOT bypass resolution-path invariants.
5. **Cost anomalies require explicit receipts.**
   Breaches of policy-defined expected-vs-measured variance MUST emit a typed anomaly/drift receipt (for example `economy.cost.anomaly_detected.v1`) with stable reason codes.

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
* Any breaker activation, autonomy mode transition, or `WITHHELD` decision driven by `sv`, `Δm_hat`, `XA_hat`, or correlated-verification share MUST reference the **specific** `snapshot_id` and `snapshot_hash` used to make that decision (recorded in the corresponding receipt).
* The snapshot time boundary MUST be deterministic: `as_of_ms` MUST be rounded down to the start of the minute (UTC), and the same boundary MUST be used across the system.
* Snapshot derivation MUST use a deterministic total order over included receipts: default order is ascending `created_at_ms`, then ascending `receipt_id` as stable tie-break.

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
* correlation-adjusted `sv_effective` (or equivalent `ρ_effective`)

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
* liability premiums/risk charges collected (5m/1h/24h)
* capital coverage ratio (reserves/capital vs bonded and claim exposure)

#### H) Risk & Breakers

* breaker states (global and per-domain)
* concentration metrics (HHI/top share)
* failure spikes
* autonomy throttle state (normal/degraded/approval-required)
* drift alert counts and active drift alerts by slice

#### I) Incidents, Rollback, and Certification (new)

* incident and near-miss rates by taxonomy + severity
* rollback/compensating-action attempt and success rates
* top rollback reason codes (public-redacted)
* certification distribution and uncertified-block counts by slice

#### J) Verifier Capacity & Synthetic Practice (new)

* number of qualified verifiers by domain
* simulation scenarios completed (rolling)
* verifier performance score distribution (calibration/time-to-detect)
* backlog of ground-truth cases not yet converted to simulations

#### K) Identity Assurance (new)

* authentication assurance distribution by role and slice
* personhood-verified share for policy-gated lanes

#### L) Markets and Coverage (new)

* `coverage_bound_share` by slice
* average underwriters per bound contract
* coverage concentration (`hhi` / top share)
* implied fail probability p50/p95 (where belief instrument is enabled)
* market calibration score (windowed)
* coverage premiums collected and coverage claims paid (24h/7d)
* market disagreement rate (market signal vs verdict/claim outcome)
* market manipulation flags (24h)

#### M) Cost Proofs and Compute Integrity (new)

* cost-proof share by slice
* cost attestation level distribution (`C0`–`C3`)
* cost anomaly rate (24h)
* measured-vs-expected cost variance p50/p95
* provider cost-integrity score distribution
* price-vs-cost spread bps bands

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

The following supplemental normative section extends, rather than replaces, the core kernel. It treats GPU/compute markets as a first-class economic surface built on the same primitives already defined: WorkUnits, Contracts, receipts, bounded credit, bonds, verification, liability, `/stats`, and policy-gated authority.

---

## 8. Supplemental Spec — Compute Commodities, Capacity Markets, and Derivatives

This section defines how the OpenAgents Economy Kernel extends into a full compute market: spot capacity, forward capacity sales, futures-style instruments, options-style reservation rights, structured hedges, indices, and settlement semantics for physical or cash-settled compute products.

The intent is not to bolt on speculative finance as an afterthought. The intent is to recognize that once compute becomes scarce, fungible enough to trade, and costly enough to hedge, it naturally becomes a commodity market. The kernel therefore supports not only **buying work** and **verifying work**, but also **buying future capacity**, **locking future prices**, **underwriting performance risk**, and **settling deviations between promised compute and delivered compute**.

The system’s design principle remains unchanged:

> **Every market instrument must terminate in machine-legible obligations, machine-legible measurement, and deterministic receipts.**

This means compute markets in OpenAgents are not abstract financial toys. They are bounded claims on measurable capacity, delivery quality, and liability windows.

### 8.1 Goals

This extension exists to support five classes of economic behavior:

First, buyers need a way to purchase compute immediately for jobs, inference, and short training runs.

Second, providers need a way to pre-sell future capacity so they can finance infrastructure with lower uncertainty.

Third, buyers need a way to hedge future compute cost before they know every exact runtime detail.

Fourth, the system needs market-native price discovery so `/stats`, routing, underwriting, envelope issuance, and policy can reason about real supply and real forward risk.

Fifth, all of the above must remain bounded by the same kernel invariants: authenticated HTTP authority, deterministic receipts, explicit failure, policy-gated risk, no hidden insolvency, and no resolution without a declared path.

### 8.2 Non-goals

This extension MUST NOT introduce unbounded leverage, open-ended margin lending, token-dependent clearing, or opaque off-ledger obligations.

It MUST NOT permit synthetic instruments whose settlement cannot be reduced to a deterministic `ResolutionRef`, index reference, or delivery proof.

It MUST NOT allow “paper” compute exposure to silently exceed policy-defined bounds on deliverable supply, collateral capacity, or cash settlement capacity.

It MUST NOT bypass verification, cost-proof, or liability requirements simply because an instrument is financially settled.

### 8.3 Compute as a commodity domain

For the purposes of the kernel, compute is modeled as a commodity domain composed of standardized slices of capacity and quality. A tradable compute product MUST be defined in terms precise enough that a buyer, provider, verifier, underwriter, and adjudicator can all answer the same question: what exactly was promised?

Every tradable compute product MUST bind at minimum:

* resource class
* capacity unit
* delivery window
* delivery region or region set
* quality/SLA terms
* metering method
* attestation posture
* settlement mode
* fallback and failure semantics

This product definition is the foundation for spot, forwards, futures-style contracts, options-style reservation rights, and indices.

### 8.4 Core compute-market objects

This extension introduces additional kernel-native objects.

#### 8.4.1 ComputeProduct

A `ComputeProduct` is a standardized definition of a tradable compute slice.

A ComputeProduct MUST include:

* `product_id`
* `resource_class`
* `capacity_unit`
* `window_spec`
* `region_spec`
* `performance_band`
* `sla_terms_ref`
* `cost_proof_requirement`
* `attestation_requirement`
* `settlement_mode`
* `index_eligibility`
* `version`

Examples of `resource_class` include GPU model families, GPU-memory bands, mixed accelerator pools, CPU pools, storage-IO classes, or bundled training/inference classes. The kernel does not require one universal taxonomy, but any active product taxonomy MUST be versioned, explicit, and policy-bound.

#### 8.4.2 CapacityLot

A `CapacityLot` is a concrete offered inventory unit for a ComputeProduct during a specific delivery interval.

A CapacityLot MUST include:

* provider identity
* product reference
* delivery start/end
* quantity
* minimum acceptable price
* region and location constraints
* measurement and attestation posture
* cancellation/curtailment terms
* reserve state
* offer expiry

A CapacityLot may back a spot offer, a forward sale, a futures delivery obligation, or an option exercise.

#### 8.4.3 ComputeIndex

A `ComputeIndex` is a deterministic reference price or reference condition used for cash settlement, policy, routing, or observability.

A ComputeIndex MUST define:

* the product slice it represents
* the observation window
* the eligible contributing observations
* the aggregation function
* the outlier and manipulation filters
* the publication cadence
* the revision and correction rules
* the anchoring and audit posture

A ComputeIndex is not merely informational. It may become part of a `ResolutionRef` for financially settled instruments.

#### 8.4.4 DeliveryProof

A `DeliveryProof` is the canonical evidence object for physical delivery of compute.

A DeliveryProof MUST link:

* the instrument or contract that required delivery
* the CapacityLot or lots used
* metered usage
* attestation posture
* performance/SLA observations
* accepted delivery quantity
* accepted delivery quality
* variance against promised terms
* verifier/adjudicator linkage where required

DeliveryProofs are content-addressed evidence and MUST participate in receipt linkage the same way verdict evidence does elsewhere in the kernel.

#### 8.4.5 CapacityInstrument

A `CapacityInstrument` is a market-tradable obligation or right referencing compute capacity. Minimum supported classes are:

* `SPOT`
* `FORWARD_PHYSICAL`
* `FUTURE_CASH`
* `FUTURE_PHYSICAL`
* `CALL_OPTION_RESERVATION`
* `PUT_OPTION_RELEASE`
* `SWAP_FIXED_FLOATING`
* `STRUCTURED_STRIP`

Each instrument MUST declare whether it resolves by physical delivery, cash settlement, or buyer election under declared rules.

### 8.5 Instrument families

#### 8.5.1 Spot capacity

Spot is immediate or near-immediate compute procurement. It is the closest extension of the existing marketplace model.

Spot instruments MUST bind:

* exact or bounded delivery start
* delivery duration or quantity
* price or max price
* region flexibility
* SLA and attestation floor
* acceptable substitution rules if any

Spot settlement MAY use the existing WorkUnit/Contract path directly, but when standardized spot lots are traded through a book or auction, they MUST also emit market receipts for listing, match, allocation, and delivery.

#### 8.5.2 Forward physical contracts

A forward physical contract is a bilateral agreement to deliver compute capacity in a future window at a fixed agreed price.

Forward physical contracts are useful for providers financing new GPU purchases and for buyers locking future access.

A forward physical contract MUST include:

* buyer and provider identities or clearing references
* ComputeProduct reference
* delivery month/week/window
* quantity
* fixed price
* minimum attestation and cost-proof posture
* acceptable substitution rules
* performance/SLA remedies
* curtailment and non-delivery remedies
* collateral rules
* whether novation to a cleared form is allowed

A forward physical contract MUST be backed by explicit provider collateral, buyer prepayment or credit posture, or both, as policy requires.

#### 8.5.3 Futures-style physical contracts

A futures-style physical contract is a more standardized forward that clears under deterministic market rules. It may be traded before expiry and may settle by allocating physical delivery at expiry.

The kernel MAY support physical futures only where product standardization is sufficiently strong and delivery proof semantics are robust enough to avoid endless subjective disputes.

Physical futures MUST define:

* canonical contract size
* canonical delivery month/week
* last trade / exercise / notice times
* delivery matching and assignment rules
* approved deliverable substitution set
* quality adjustment rules
* failure-to-deliver remedies
* margin/collateral rules if enabled
* breaker posture when deliverability deteriorates

#### 8.5.4 Futures-style cash-settled contracts

A cash-settled compute future resolves against a ComputeIndex rather than actual delivery. This is the simplest way to let market participants hedge future GPU prices without fully resolving exact machine assignment in advance.

Cash-settled futures MUST bind to a deterministic ComputeIndex and a deterministic settlement formula.

They MUST include:

* reference index
* contract unit
* settlement window
* tick size or price precision
* long/short exposure rules
* collateral rules
* settlement formula
* correction policy in case of index supersession

Cash settlement MUST occur only through authenticated HTTP authority actions and MUST emit deterministic receipts. Index publication alone is not settlement.

#### 8.5.5 Options-style reservation rights

A call-style reservation right gives the holder the right, but not the obligation, to secure capacity at a fixed strike or capped rate within a defined future window.

This is useful when a buyer knows they may need compute but does not yet know the exact workload shape.

A reservation option MUST declare:

* underlying ComputeProduct
* strike or pricing formula
* exercise window
* delivery window
* quantity cap
* premium
* exercise procedure
* substitution rights
* expiry semantics

Exercising a reservation right MUST become an authority mutation that either creates a physical delivery obligation or mints a settlement entitlement under declared terms.

#### 8.5.6 Swaps and strips

A fixed-floating compute swap lets one party pay a fixed rate and receive floating index exposure, or vice versa, over one or more delivery windows.

A strip is a deterministic bundle of monthly or weekly contracts, often used to hedge a longer period, such as six months or thirty-six months of expected compute demand.

These instruments are appropriate when a provider wants to pre-sell long arcs of capacity or when a buyer wants to lock a compute budget over time.

All swaps and strips MUST be decomposable into a deterministic set of underlying instrument legs for receipting, exposure tracking, and dispute replay.

### 8.6 Market structure modes

The kernel MAY support multiple market structure modes, each policy-gated.

#### 8.6.1 Bilateral RFQ mode

This is the simplest mode and fits your existing kernel style best.

A buyer asks for quotes for a future capacity slice. Providers respond. TreasuryRouter selects under policy. The kernel binds the selected quote into an instrument and emits receipts.

This mode is recommended for early deployment.

#### 8.6.2 Deterministic auction mode

Providers and buyers submit offers into a time-bounded auction. Clearing occurs at a deterministic batch boundary. Clearing receipts allocate quantities and prices.

This mode is appropriate for spot lots, forward auctions, and initial coverage of standardized monthly strips.

#### 8.6.3 Central book / periodic clearing mode

For more mature products, the kernel MAY maintain a central order book or periodic batch book. Continuous matching is allowed only if deterministic ordering, replay, and tie-break rules are explicit and auditable.

Recommended default remains periodic batch clearing because it is easier to replay and less vulnerable to hidden microstructure divergence.

### 8.7 Instrument lifecycle semantics

Every capacity instrument MUST follow a canonical lifecycle. Implementations MAY expose instrument-specific derived states, but the minimum state machine is:

`CREATED → OPEN → PARTIALLY_FILLED/FILLED → ACTIVE → (DELIVERING | CASH_SETTLING) → SETTLED | DEFAULTED | EXPIRED | CANCELLED`

Where margin-like collateral is enabled, additional collateral states MAY exist, but they MUST remain explicit and receipted.

#### 8.7.1 Listing and quote states

Instrument creation and listing are distinct.

A proposed instrument or quote is not yet a binding obligation until it passes all required policy, collateral, and identity checks and transitions into an active bound state.

#### 8.7.2 Activation

An instrument becomes `ACTIVE` only when all preconditions have been met, including:

* policy evaluation
* collateral reservation
* identity assurance checks where required
* reference index binding where required
* delivery rules binding
* idempotency and replay checks

#### 8.7.3 Settlement fork

At maturity or exercise, the lifecycle forks into either physical delivery or cash settlement.

That fork MUST already be specified in the instrument terms. It MUST NOT be improvised later except where buyer election is explicitly allowed and receipted within a bounded election window.

### 8.8 Physical delivery semantics

Physical delivery in this domain means provision of actual compute capacity consistent with the instrument’s product definition.

Physical settlement MUST produce:

* allocation receipt(s)
* delivery-start receipt
* delivery-progress receipts where relevant
* delivery-proof evidence
* final acceptance or rejection receipt
* price adjustment and remedy receipts if deviations occurred

Physical delivery MAY settle directly into WorkUnits if the buyer immediately uses the capacity for kernel-native work. In that case, the instrument’s delivery proof and the WorkUnit’s runtime/cost proof SHOULD be linked so the system can trace a line from market hedge to actual executed work.

#### 8.8.1 Substitution and deliverability

Because compute is heterogeneous, physical settlement MUST explicitly address substitution.

Every product or instrument MUST state whether substitution is:

* not allowed
* allowed within a narrow equivalence class
* allowed with quality adjustments
* allowed only with buyer approval
* allowed under emergency policy break-glass rules

Hidden substitution is forbidden.

#### 8.8.2 Curtailment and interruption

Providers may fail to deliver full capacity because of facility outages, hardware failures, geopolitical restrictions, network isolation, or internal overcommitment.

Curtailment semantics MUST be explicit and include:

* minimum notice rules if known in advance
* partial-delivery rules
* replacement attempt rules
* compensation formula
* bond draw or insurance trigger
* whether the event qualifies as ordinary failure or force-majeure-like exclusion under policy

### 8.9 Cash settlement semantics

Cash-settled instruments resolve against a deterministic reference and settle the difference between contracted terms and market-observed terms.

A cash-settlement receipt MUST include:

* reference instrument
* reference index id/version
* observation window
* final reference value
* settlement formula
* payer/payee
* gross amount
* fees
* collateral sources if relevant
* proof of settlement

The reference value MUST come from a receipted, immutable ComputeIndex publication or a specific index snapshot hash.

### 8.10 Compute indices and index governance

The system’s financial layer depends heavily on trustworthy indices. Therefore ComputeIndex objects are first-class kernel objects, not informal dashboard numbers.

#### 8.10.1 Eligible observations

An index MUST define exactly which observations are eligible, such as:

* cleared spot trades
* accepted forward trades
* delivered and accepted physical capacity
* qualified RFQ responses
* provider posted offers, if policy permits
* external market references, if allowed by policy and bound as evidence

The kernel SHOULD favor observations backed by actual fills and actual delivery over mere indicative quotes.

#### 8.10.2 Manipulation resistance

Index methodology MUST define how it mitigates thin-market and manipulation risk, including:

* minimum trade count or notional thresholds
* provider diversity minimums
* outlier trimming
* self-trade exclusion
* affiliate concentration limits
* fallback behavior when the market is too thin

If an index window fails minimum quality thresholds, cash-settled instruments referencing that window MUST follow predetermined fallback rules. They MUST NOT silently use ad hoc operator judgment.

#### 8.10.3 Publication and corrections

Index publication MUST be receipted and append-only. If an index is corrected, the correction MUST be a new publication with supersession linkage.

Already-settled instruments MUST follow predeclared correction rules. In many cases, correction after final settlement SHOULD NOT retroactively mutate prior receipts; instead it MAY affect future confidence, policy, or reserve accounting.

### 8.11 Collateral, margin-like controls, and bounded leverage

The kernel does not support unbounded leverage. However, some standardized contracts may require periodic mark-to-market or bounded variation collateral to keep obligations credible.

This section therefore permits bounded collateral adjustment, but only under strict policy.

#### 8.11.1 Allowed collateral postures

The following collateral postures MAY exist:

* full pre-funding
* initial bond only
* initial bond plus bounded variation collateral
* cleared net collateral across a strictly defined portfolio partition

Any enabled posture MUST declare maximum leverage, maximum collateral call frequency, default handling, and breaker triggers.

#### 8.11.2 Variation settlement

When allowed, variation settlement MUST be explicit authority actions with receipts. It MUST NOT happen as an invisible internal accounting drift.

#### 8.11.3 Portfolio netting

Netting is permitted only within explicit clearing partitions defined by policy and only when replay remains deterministic. Hidden cross-partition subsidy is prohibited.

### 8.12 Default, closeout, and failure semantics

Compute market instruments MUST define what happens when a party fails to perform.

Minimum default paths include:

* payment default
* non-delivery default
* under-delivery default
* SLA breach default
* collateral shortfall default
* index failure fallback
* market-halt expiration or forced closeout

Each default type MUST have stable reason codes and deterministic remedy order.

Recommended remedy order is:

1. use reserved collateral in the instrument partition
2. apply insurance/coverage if bound
3. apply contractually defined replacement or cash compensation
4. escalate to claim/dispute path
5. emit default closure receipt and update `/stats`

### 8.13 Relationship to WorkUnits and Contracts

This extension does not replace WorkUnits. It adds a market layer above them.

A compute hedge or capacity purchase may exist before any specific workload is known. Later, when the buyer knows what exact training or inference workload they want, the market instrument can be bound into one or more WorkUnits or Contracts.

Three canonical relationships are supported.

First, a spot instrument MAY directly create a WorkUnit-ready capacity allocation.

Second, a forward or future MAY later be exercised or assigned into one or more concrete capacity reservations that back WorkUnits.

Third, a cash-settled hedge MAY remain purely financial and never back direct delivery; in that case it offsets budget variance rather than serving as actual runtime capacity.

The kernel MUST make this relationship explicit so `/stats` can distinguish:

* raw compute trading volume
* physically delivered compute
* financially hedged compute
* compute actually consumed in WorkUnits

### 8.14 Relationship to verification and CostProofBundle

This extension depends heavily on your existing Cost Integrity module.

For compute-market products, CostProofBundle semantics become part of delivery and dispute resolution, not just observability.

#### 8.14.1 Delivery verification

Where physical delivery occurs, policy MAY require DeliveryProof to include or reference CostProofBundle evidence sufficient to confirm:

* quantity delivered
* duration delivered
* resource class delivered
* utilization or available capacity posture, depending on product definition
* attestation posture
* anomaly detection results

#### 8.14.2 Cash vs physical divergence

The system MUST keep separate notions of:

* index price
* offered price
* delivered price
* measured cost
* quoted cost

This matters because a market can clear at one number while actual delivered cost integrity tells a different story. Both signals are valuable; neither should erase the other.

#### 8.14.3 Fraud and wash prevention

If a provider attempts to manipulate spot or index prices using self-crossing, fake fills, fake capacity, or misleading metering, the kernel MUST be able to surface this through:

* market manipulation flags
* cost anomalies
* attestation downgrade signals
* coverage tightening
* breaker activations

All such actions MUST be receipted.

### 8.15 Relationship to liability markets and underwriting

This extension and Section 6.9 are deeply connected.

Compute markets introduce at least four underwritable risk classes:

* non-delivery risk
* SLA/performance degradation risk
* index/reference methodology risk
* curtailment/interruption risk

CoverageBindings MAY therefore attach to compute instruments directly, not only to WorkUnit warranties.

A compute instrument MAY bind optional coverage for:

* delivery failure
* under-delivery
* quality degradation beyond stated bands
* interruption during delivery window
* replacement cost escalation

Each coverage product MUST remain bounded and receipted just like other liability instruments.

### 8.16 Safe-harbor use of market signals

Market prices contain information, but they are not truth. Therefore market signals MAY influence policy only in bounded ways.

The kernel MAY use market-backed signals for:

* routing preference
* envelope limits
* underwriter pricing
* reserve sizing
* autonomy throttles in compute lanes
* optional verification relaxations where extremely strong market and delivery evidence exists

The kernel MUST NOT treat price alone as delivery proof or verification proof.

Policy may define safe-harbor relaxations based on market depth, delivery history, cost-integrity posture, and coverage quality. Any such relaxation MUST reference a specific snapshot and MUST be receipted.

### 8.17 Additional state variables for compute markets

The kernel SHOULD maintain the following compute-market state variables.

#### 8.17.1 `compute_open_interest`

Outstanding notional exposure by product slice and maturity window.

#### 8.17.2 `deliverable_coverage_ratio`

Ratio of physically credible deliverable supply to outstanding physical obligations in a slice.

#### 8.17.3 `hedged_share`

Fraction of compute consumption or future planned consumption that is hedged by active instruments.

#### 8.17.4 `paper_to_physical_ratio`

Ratio of financial notional exposure to physically deliverable capacity. Policy SHOULD gate this aggressively.

#### 8.17.5 `index_quality_score`

A machine-legible index integrity score derived from depth, diversity, realized delivery linkage, and manipulation signals.

#### 8.17.6 `delivery_default_rate`

Rolling default rate by provider, product slice, and maturity bucket.

#### 8.17.7 `curve_shape`

Term-structure summary for compute prices: contango, backwardation, or flat, by slice.

#### 8.17.8 `replacement_cost_gap`

Observed difference between contracted delivery price and replacement procurement price after delivery failure.

These variables SHOULD be published in `/stats` and SHOULD influence policy in a bounded, transparent way.

### 8.18 Additional `/stats` requirements for compute markets

If this extension is enabled, `/stats` MUST add a compute-markets surface with stable tables.

#### A) Spot market table

Must include by major slice:

* traded quantity
* fill rate
* realized price p50/p95
* provider breadth
* delivery acceptance rate
* attestation mix
* cost anomaly rate

#### B) Forward/futures table

Must include by maturity bucket and slice:

* open interest
* traded volume
* settlement mode split
* average collateralization
* default rate
* curve shape
* top concentration metrics

#### C) Delivery integrity table

Must include:

* deliverable coverage ratio
* accepted vs rejected delivery share
* under-delivery rate
* interruption rate
* replacement cost gap p50/p95
* top stable reason codes for delivery failure

#### D) Index integrity table

Must include:

* index quality score
* number of eligible observations
* fill-backed observation share
* provider diversity
* outlier discard rate
* manipulation flags
* correction count

#### E) Hedging posture table

Must include:

* hedged share of future compute demand
* physical vs cash-settled mix
* strip coverage by horizon
* average premium / carry by horizon
* buyer concentration and provider concentration

### 8.19 Market policy requirements

If compute markets are enabled, PolicyBundles MUST be able to express at least:

* which product slices are allowed
* which instrument classes are allowed
* which settlement modes are allowed
* who may issue or trade which instruments
* maximum maturity horizon
* maximum open interest by slice
* maximum paper-to-physical ratio
* minimum collateralization by slice and role
* minimum attestation and cost-proof posture for physical settlement
* minimum index quality thresholds for cash settlement
* default handling posture
* breaker rules for thin markets, manipulation signals, or deliverability deterioration
* whether external reference indices are allowed
* whether buyer election between cash and physical is allowed

All policy-triggered market halts, contract disables, maturity restrictions, or collateral hikes MUST be receipted and visible in `/stats`.

### 8.20 Breakers specific to compute markets

In addition to general kernel breakers, the following compute-market breakers SHOULD exist where relevant:

* `INDEX_QUALITY_BREAKER`
* `DELIVERABILITY_BREAKER`
* `PAPER_EXPOSURE_BREAKER`
* `CURTAILMENT_SPIKE_BREAKER`
* `MANIPULATION_SIGNAL_BREAKER`
* `ATTESTATION_DEGRADATION_BREAKER`

When triggered, policy MAY:

* halt new issuance in affected slices
* force full-collateral posture
* disable cash settlement for low-quality indices
* restrict maturities
* require buyer-only closing trades
* disable safe-harbor relaxations
* require human approvals

### 8.21 Deterministic clearing requirements

Where books or auctions are used, clearing MUST be deterministic.

At minimum, clearing rules MUST define:

* batch boundary
* ordering by price, time, and stable tie-break
* partial fill allocation
* self-trade handling
* affiliate handling if applicable
* minimum quantity increments
* cancellation cutoff rules
* clearing price rules
* publication and correction posture

Clearing decisions MUST be represented by explicit clearing receipts that are replayable from submitted offers and the declared rules.

### 8.22 Receipt requirements for compute instruments

The following receipt families SHOULD exist if this extension is implemented:

* `economy.compute_product.registered.v1`
* `economy.compute_lot.offered.v1`
* `economy.compute_lot.cancelled.v1`
* `economy.compute_index.published.v1`
* `economy.compute_index.corrected.v1`
* `economy.capacity_instrument.created.v1`
* `economy.capacity_instrument.bound.v1`
* `economy.capacity_instrument.cleared.v1`
* `economy.capacity_instrument.collateral_reserved.v1`
* `economy.capacity_instrument.variation_settled.v1`
* `economy.capacity_instrument.delivery_assigned.v1`
* `economy.capacity_instrument.delivery_started.v1`
* `economy.capacity_instrument.delivery_proven.v1`
* `economy.capacity_instrument.cash_settled.v1`
* `economy.capacity_instrument.defaulted.v1`
* `economy.capacity_instrument.closed.v1`
* `economy.compute_market.breaker_activated.v1`
* `economy.compute_market.breaker_cleared.v1`
* `economy.compute_market.manipulation_flagged.v1`

All such receipts MUST obey the same hashing, idempotency, linkage, and evidence rules as the rest of the kernel.

### 8.23 ResolutionRef requirements for compute instruments

Every compute instrument MUST declare one of the following resolution classes:

* `PHYSICAL_DELIVERY_PROOF`
* `COMPUTE_INDEX_SNAPSHOT`
* `CLAIM_RESOLUTION_RECEIPT`
* `HUMAN_UNDERWRITTEN_EXCEPTION`

The declared class determines what can settle the instrument. Nothing else may silently settle it.

For physical contracts, accepted DeliveryProof plus any necessary adjudication receipts is authoritative.

For cash-settled contracts, the bound ComputeIndex snapshot is authoritative unless fallback procedures are triggered.

For exceptional/manual lanes, the human-underwritten exception MUST still be receipted and bounded by explicit policy.

### 8.24 Interoperability posture

This market extension SHOULD be interoperable with external cloud providers, neo-clouds, exchanges, and broker/dealer-like orchestration layers, but interoperability MUST never weaken kernel authority semantics.

External systems MAY contribute offers, fills, delivery evidence, or index data only if those contributions are ingested through authenticated authority or evidence paths and turned into canonical receipts.

The kernel remains the source of truth for its own economic state, even when external venues are involved.

### 8.25 Recommended rollout order

Normatively, all instrument classes are possible. Practically, the rollout should be staged.

Phase one SHOULD be bilateral RFQ spot and bilateral RFQ forward physical contracts for standardized slices, backed by CostProofBundle and simple provider bonds.

Phase two SHOULD add deterministic auctions for forward monthly strips and optional compute indices based only on actual fills and actual accepted delivery.

Phase three SHOULD add cash-settled futures-style contracts for the most liquid standardized slices, with strict index quality and paper-exposure controls.

Phase four MAY add options-style reservation rights and swap/strip products.

Belief-market overlays, more expressive clearing partitions, and broader portfolio collateral SHOULD come later, not earlier.

### 8.26 Normative design thesis

This extension exists because compute is becoming scarce enough, important enough, and financeable enough to need market structure. But the kernel’s thesis is not merely that compute should be tradeable.

The thesis is stronger:

> **Compute markets become trustworthy only when delivery, measurement, liability, and settlement are all machine-legible.**

That is the difference between a marketing layer for cloud capacity and a real economic substrate.

In OpenAgents, the same system that can pay for work, verify work, insure work, and settle disputes can also:

* buy spot compute,
* pre-sell future capacity,
* hedge future prices,
* publish auditable compute indices,
* settle physical or cash compute contracts,
* and expose the whole market through receipts and `/stats`.

That gives you a path to build not just a compute marketplace, but a full **compute commodities stack** inside the same kernel.

---

### 8.A Optional Appendix — concise mapping to existing sections

This supplemental section plugs into the existing spec as follows.

`§2 Core objects` gains `ComputeProduct`, `CapacityLot`, `ComputeIndex`, `DeliveryProof`, and `CapacityInstrument`.

`§4 State machines` gains instrument lifecycle semantics for spot, forwards, futures, options, swaps, and strips.

`§5 Receipts` gains compute-product, index, clearing, delivery, cash-settlement, collateral-adjustment, and default receipt families.

`§6 Modules` gains a Compute Markets module, or extends Markets, Cost Integrity, Liability, Routing & Risk, and `/stats` to cover compute-specific surfaces.

`§7 /stats` gains spot, forwards/futures, delivery integrity, index integrity, and hedging posture tables.
