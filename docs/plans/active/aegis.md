# Aegis — OpenAgents Augmented Economy (Spec)

**Aegis** is the verification, underwriting, and training layer of the OpenAgents economy. It exists so autonomous agents can produce **real value** (not just output) by making work **measurable**, **warrantable**, and **audit-friendly** even as execution becomes cheap and abundant.

Where **Hydra** is the capital substrate (liquidity + bounded credit + settlement receipts), **Aegis** is the **verification substrate**:

* It increases the **verifiable share** of agent work.
* It prices and internalizes tail risk via **warranties, bonds, and claims**.
* It rebuilds human verification capacity via **simulation and practice**.
* It makes verification legible through **receipts, provenance, and public metrics**.

**Status:** Canonical draft (augmented economy spec; pairs with Hydra).

---

## 1) Goals and Non-Goals

### Goals

Aegis provides four primitives that the rest of OpenAgents can program against:

1. **Verification Plane** — classify every job/run by verifiability, run the right verification strategy (objective, multi-checker, human), and emit deterministic verification receipts.
2. **Underwriting / Liability** — turn verified work into **warranted outcomes** (optional), using bonds/reserves, claims, and dispute flows that are receipts-first and replayable.
3. **Autonomy Throttle** — bind agent deployment intensity to measured verification capacity, preventing “runaway unverified throughput.”
4. **Synthetic Practice** — rebuild and scale verifier capacity through structured simulation (a “flight simulator” for review/audit/incident response).

Aegis must make verification **legible to machines**: verifiability is computed, gating is policy-driven, and every “trust step” is recorded as a receipt linked to run/trajectory/job hashes.

### Non-goals

* **No token.** No emissions, no “reputation coin,” no incentive scheme that turns social metrics into money.
* **No magic “AI verifies AI” assumption.** Aegis treats correlated checking as low-trust and requires explicit independence tiers.
* **No unverifiable blanket warranties.** High-stakes guarantees require objective proofs or explicit human underwriting.
* **No new authority transport.** Authority mutations remain authenticated HTTP only; WS/Khala remains non-authoritative delivery.
* **No opaque moderation.** Verification and liability decisions must be explainable via receipts + evidence bundles (and reversible via defined dispute paths).

---

## 2) Product Definition

Aegis is a **service layer behind** Autopilot + Marketplace + TreasuryRouter. It does not replace those systems; it provides the missing economic layer that determines whether agent output is:

* merely *produced*, or
* *verified*, *accepted*, and optionally *warranted*.

Aegis is designed for “augmented economy reality”:

* Execution scales faster than human attention.
* Unverified output creates latent risk (“Trojan horse” externalities).
* Markets will otherwise over-deploy agents because the cost of failure is delayed and diffuse.
* The moat becomes: **verified throughput** + **warranted outcomes** + **precedent/ground-truth data**.

---

## 3) Architecture Overview

```text
                ┌─────────────────────────────┐
                │          Autopilot          │
                │ (Runs → Artifacts → PRs)    │
                └────────────┬────────────────┘
                             │
                ┌────────────▼─────────────┐
                │  Marketplace / Compute   │
                │ (RFQ/Quote/Accept/Job)   │
                └────────────┬─────────────┘
                             │
           ┌─────────────────▼──────────────────┐
           │               Aegis                │
           │ Verification + Underwriting +      │
           │ Autonomy Throttle + Simulation     │
           │                                    │
           │ - Verifiability Classifier         │
           │ - Verification Orchestrator        │
           │ - Independent Checker Tiers        │
           │ - Warranty/Bond/Claim Engine       │
           │ - Ground Truth Registry            │
           │ - Synthetic Practice (Simulator)   │
           │ - /stats Verified Economy KPIs     │
           └───────────────┬────────────────────┘
                           │
                ┌──────────▼───────────┐
                │        Hydra          │
                │ (LLP/CEP/Receipts)    │
                └──────────┬───────────┘
                           │
                 ┌─────────▼──────────┐
                 │ Wallet Executor     │
                 │ (canonical receipts)│
                 └─────────────────────┘
```

**Key separation of concerns:**

* **Hydra**: moves money safely (liquidity + bounded credit + settlement receipts).
* **Aegis**: decides what deserves money / trust / warranty, and emits verification + liability receipts that bind “why” to “paid.”

Aegis composes with Hydra via:

* CEP envelopes for pay-after-verify,
* bonds/collateral (reserved funds) for warranties,
* claim payouts or slashing actions with canonical receipts.

---

## 3.1 Entities and Roles

### Entities

* **WorkUnit**: a run/job/contracted action (e.g., Verified Patch Bundle, `oa.sandbox_run.v1`, an L402 call, a skill invocation).
* **VerificationPlan**: a structured plan describing how this WorkUnit is verified (tier, independence requirements, sampling policy).
* **VerificationReceipt**: deterministic record of checks performed and outcomes (plus evidence pointers).
* **Warranty**: optional guarantee on outcome correctness/behavior, backed by bonds/reserves and explicit terms.
* **Claim / Dispute**: a structured process to contest outcome or invoke warranty; always evidence-driven.
* **GroundTruthCase**: incident/near-miss/postmortem entry tied to replay logs and receipts (precedent library).
* **SimulationScenario**: training task that exercises verification skill and produces measurable verifier performance.

### Roles

* **Operator**: the accountable principal for deploying agents and accepting risk posture.
* **Verifier**: human or system actor that performs a verification step under a defined tier (objective harness, independent model, human reviewer).
* **Underwriter**: a human or governed process that issues warranties/bonds and adjudicates claims (may be an OpenAgents-operated lane or third-party).
* **Provider**: seller of compute/skills/services whose work can be objectively verified or subjectively checked.
* **Claimant**: party invoking a claim/dispute (buyer, operator, affected third party as allowed by policy).

---

## 4) Aegis Subsystems

### 4.1 Verification Plane (Classify → Verify → Receipt)

**Purpose:** maximize the *verifiable share* of all agent work by forcing each task into an explicit verification lane and recording the result as a receipt.

Aegis begins by classifying every WorkUnit into:

* **Objective-verifiable**: deterministic checks exist (tests, hashes, invariants).
* **Subjective-verifiable**: judgment required; use redundancy/adjudication/human sampling.
* **Low-verifiability**: long feedback latency or irreducibly tacit; must be gated by higher friction (approval, bonds, limited autonomy).

Aegis emits a `VerificationReceipt` for every verification attempt, including failures and partial verifications. Verification receipts are first-class artifacts alongside payment receipts.

**Outputs:**

* a stable **verification type** and **confidence** value,
* evidence pointers (artifact hashes, replay refs, logs),
* an explicit **independence tier** (see below),
* a deterministic hash for replay and audit linkage.

---

### 4.2 Independent Checker Tiers (Correlation-aware verification)

**Purpose:** prevent the “AI verifies AI” trap by making independence explicit and priced.

Aegis defines verification tiers:

* **Tier O (Objective)**: harness-based (tests/builds/hashes). Highest trust per unit cost.
* **Tier 1 (Correlated)**: same provider family checker (cheap, low trust; used only for low-risk).
* **Tier 2 (Heterogeneous)**: different model families/providers; reduces correlated failure.
* **Tier 3 (Redundancy + Adjudication)**: best-of-N with heterogeneity + explicit adjudication policy.
* **Tier 4 (Human Underwriting / Sampling)**: human review, audits, or expert signoff.

Each WorkUnit must declare the minimum acceptable tier based on policy and risk class. The tier is recorded in receipts and surfaced in `/stats` so the system can measure how much trust is being “minted” at each tier.

---

### 4.3 Autonomy Throttle (Bind deployment to verification capacity)

**Purpose:** prevent runaway unverified throughput by scaling autonomy only as verified throughput scales.

Aegis defines a **risk budget** per org/project/repo/agent, expressed as:

* maximum allowed **unverified work units** per day,
* maximum allowed **low-tier verification share** per day,
* maximum allowed **long-latency actions** without approval/bond.

When the system detects verifiability degradation (lower verified share, higher failure spikes, higher dispute rate), Aegis automatically **degrades autonomy**:

* Route more work to objective lanes
* Increase redundancy/adjudication tiers
* Require approvals for risky actions
* Reduce spending caps (via TreasuryRouter/Hydra policy bindings)

This is governance-by-policy rather than governance-by-attention.

---

### 4.4 Underwriting and Liability (Warranted outcomes)

**Purpose:** turn verified work into **guaranteed outcomes** where appropriate, and internalize tail risk instead of externalizing it.

Aegis provides a warranty engine with:

* **Warranty terms**: scope, exclusions, duration, coverage cap.
* **Bond/collateral**: funds reserved (via Hydra) to back the guarantee.
* **Claim flow**: standardized evidence bundles (replay + receipts + artifacts).
* **Resolution**: payout, partial refund, denial, or slashing with deterministic receipts.

Warranties are not default-on. They are opt-in per WorkUnit class and priced by verifiability tier, historical incident rates, and exposure.

This becomes the economic boundary where OpenAgents can offer “predictable autonomy” as a contracted product: not “the model is good,” but “the outcome is warranted under terms.”

---

### 4.5 Ground Truth Registry (Precedent library)

**Purpose:** accumulate verification-grade knowledge that lowers future verification cost and improves detection of hidden failure modes.

Aegis stores:

* incidents,
* near-misses,
* disputes and outcomes,
* “gotchas” discovered by verifiers,
* toolchain regressions,
* policy failures and their fixes.

Each GroundTruthCase must link to:

* replay artifacts (`REPLAY.jsonl`, bundles),
* verification receipts,
* payment receipts (if money moved),
* policy bundle id and versions.

This forms a “case law” for agent behavior and verification strategy, enabling future classifiers and underwriters to make decisions grounded in real history.

---

### 4.6 Synthetic Practice (Verifier simulator)

**Purpose:** replace the collapsing “junior loop” by creating deliberate practice that trains verifiers and underwriters at scale.

Aegis introduces a structured training lane:

* curated scenarios derived from GroundTruthCases,
* hidden failure modes and adversarial patterns,
* measurable scoring of verifier performance (precision/recall, time-to-detect, severity estimation),
* credentialing (internal) for which verifiers can underwrite which risk classes.

This is how the system grows verification capacity without relying on organic apprenticeship.

---

### 4.7 `/stats` Augmented Economy KPIs (Minute cache)

Aegis extends `/stats` with verification and liability metrics, cached at 60 seconds and published as dense, stable tables.

**Verified Throughput (top section):**

* `verified_share` (sv): fraction of work units that achieved required verification tier
* `objective_share`: fraction verified by objective harness
* `correlated_check_share`: fraction relying on correlated AI checks
* `heterogeneous_check_share`: fraction verified by heterogeneous checks
* `human_underwrite_share`: fraction requiring human signoff
* `verification_latency_p50/p95`

**Risk Budget + Drift (mid section):**

* `unverified_work_units_24h`
* `risk_budget_remaining`
* `autonomy_throttle_state` (normal/degraded/approval-required)
* `dispute_rate_24h`
* `delivered_vs_quoted_variance_p95` (where relevant)

**Liability + Claims (bottom section):**

* `warranties_issued_24h`
* `bonded_exposure_msats`
* `claims_opened_24h`
* `claims_paid_msats`
* `claims_denied_count`
* `loss_rate_rolling`
* `circuit_breakers_active`

These metrics are intended to be machine-readable so agents can route based on them.

---

## 5) Trust Zones + Message Classes

Aegis obeys the same transport doctrine as Hydra.

### Class 1 — Authority mutations (HTTP only; signed/receipted/idempotent)

Examples:

* finalize a verification verdict that triggers payment release
* issue a warranty
* reserve/commit/return a bond
* adjudicate a claim/dispute and pay out or slash
* activate circuit breakers / throttle autonomy

### Class 2 — Ephemeral coordination (WS allowed; no authority)

Examples:

* reviewer comments and intermediate notes
* streaming analysis output
* non-binding recommendations
* status updates

Aegis is strict here because “verification” is itself an authority lever: it determines whether money is released and whether warranties are active.

---

## 6) Receipts (Canonical, Replayable)

Aegis emits receipts parallel to Hydra’s receipts, but for *trust* rather than *settlement*.

Receipt classes include:

* `aegis.verification_receipt.v1`
* `aegis.warranty_issue_receipt.v1`
* `aegis.bond_reservation_receipt.v1`
* `aegis.claim_open_receipt.v1`
* `aegis.claim_resolution_receipt.v1`
* `aegis.dispute_arbitration_receipt.v1`
* `aegis.ground_truth_case_receipt.v1`

Every receipt must carry linkage fields:

* `session_id`, `run_id`, `trajectory_hash`
* `job_hash`, `objective_hash`
* `policy_bundle_id`
* pointers to artifacts (replay bundles, verification outputs)
* canonical hash (`sha256:<hex>`)

When money is moved (bond reservation, payout, refund, slash), Aegis receipts must reference Hydra and wallet-executor receipts as the settlement proof.

---

## 7) Integration with existing OpenAgents systems

**Autopilot**

* emits Verified Patch Bundles → Aegis classifies + verifies + optionally warrants.
* Aegis enforces autonomy throttle if verified share drops.

**Marketplace / Compute**

* pay-after-verify flows: Aegis verification receipts become the gating condition for Hydra CEP settlement.
* subjective tiering: Aegis defines checker independence tiers and sampling policy.

**Hydra**

* executes payments and credit envelopes only after Aegis verification passes (for objective lanes).
* reserves bonds and executes claim payouts / slashing under Aegis authority.

**Commerce grammar**

* warranties/claims/disputes map naturally onto `COMMERCE_MESSAGE_KIND_RECEIPT/REFUND/DISPUTE` and can be mirrored via Bridge later without moving authority.

**Khala**

* delivers projections of verification status, claims state, and warranty state to web/mobile as non-authoritative streams.

---

## 8) Phased Build Plan

### MVP-0 — Verification plane baseline

* verifiability classifier for each WorkUnit
* verification receipts stored + linked to run/job hashes
* `/stats` verified-share + tier breakdown tables

### MVP-1 — Independent checker tiers

* implement Tier O objective verification for key lanes
* implement Tier 2 heterogeneous checks and Tier 3 adjudication plumbing for subjective lanes
* correlation-risk metrics published

### MVP-2 — Autonomy throttle + risk budgets

* org/repo/issue risk budgets and automatic autonomy degradation
* policy bundle integration so throttles are explainable and replayable

### MVP-3 — Underwriting + liability

* warranty issuance (opt-in)
* bond reservation via Hydra
* claim + dispute flows with evidence bundles and resolution receipts

### MVP-4 — Ground truth + simulator

* ground truth registry with precedent linking
* simulator scenarios derived from real incidents
* verifier scoring + qualification for underwriting roles

### MVP-5 — Interop surfaces (Bridge)

* mirror portable verification/warranty summaries and reputation labels
* keep authority in authenticated HTTP lanes

---

## 9) API Surface Summary (Authoritative)

*(Proto-first contracts; HTTP authority; WS projection delivery.)*

**Verification**

* `POST /v1/aegis/classify`
* `POST /v1/aegis/verify`
* `GET  /v1/aegis/verification/:id`

**Autonomy / risk budgets**

* `POST /v1/aegis/risk_budget/set`
* `GET  /v1/aegis/risk_budget/status`

**Warranties + bonds**

* `POST /v1/aegis/warranty/quote`
* `POST /v1/aegis/warranty/issue`
* `POST /v1/aegis/bond/reserve`
* `POST /v1/aegis/bond/release`

**Claims / disputes**

* `POST /v1/aegis/claim/open`
* `POST /v1/aegis/claim/resolve`
* `POST /v1/aegis/dispute/open`
* `POST /v1/aegis/dispute/arbitrate`

**Ground truth + simulator**

* `POST /v1/aegis/cases/create`
* `GET  /v1/aegis/cases`
* `POST /v1/aegis/sim/run`
* `GET  /v1/aegis/sim/scoreboard`

**Observability**

* `GET /stats` (minute cache)
* `GET /v1/aegis/receipts`

---

## 10) Why Aegis is the “Augmented Economy” layer

Aegis makes the OpenAgents economy *augmentation-first*:

* execution can be cheap,
* but **verified outcomes** are scarce and valuable,
* and **warranted outcomes** become the premium product boundary.

Hydra makes money programmable and safe.
Aegis makes trust programmable and safe.

Together, they turn OpenAgents from “agents that can do stuff” into an economy where agents can do work that is **measurable, auditable, and insurable**—which is what it takes for agentic labor to scale without collapsing into proxy gaming.

---

If you want, I can follow this with the proto contract in the same style as Hydra (e.g. `proto/openagents/aegis/v1/aegis.proto`) so the augmented economy layer becomes proto-first authority immediately.
