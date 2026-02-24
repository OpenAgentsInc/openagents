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

```proto
// proto/openagents/aegis/v1/aegis.proto
syntax = "proto3";

package openagents.aegis.v1;

import "google/protobuf/struct.proto";
import "google/protobuf/timestamp.proto";
import "openagents/protocol/v1/reasons.proto";
import "openagents/lightning/v1/wallet_executor.proto";

// Aegis — OpenAgents Augmented Economy (v1)
//
// Proto-first contracts for:
// - Verification Plane: classify -> plan -> verify -> receipt
// - Independence-aware verification tiers (correlation-aware)
// - Autonomy throttle + risk budgets (bind deployment to verification capacity)
// - Underwriting: warranties + bonds + claims/disputes (liability internalization)
// - Ground Truth Registry (precedent library)
// - Synthetic Practice / Simulator (verifier skill ladder)
//
// Norms (must hold in implementation):
// - Authority mutations are authenticated HTTP only (INV-02).
// - WS/Khala is delivery/projection only (non-authoritative).
// - Every authority effect is idempotent (idempotency_key required).
// - Every verification/warranty/claim action emits deterministic receipts.
// - v1 is additive-only (ADR-0002); do not change semantics of existing fields.
// - Canonical JSON hashing/signature pipelines are defined outside this proto;
//   fields include `canonical_json_sha256` as a stable pointer.

//
// -----------------------------
// Common enums + primitives
// -----------------------------

enum AegisErrorCode {
  AEGIS_ERROR_CODE_UNSPECIFIED = 0;

  AEGIS_ERROR_CODE_UNAUTHORIZED = 1;
  AEGIS_ERROR_CODE_FORBIDDEN = 2;
  AEGIS_ERROR_CODE_INVALID_REQUEST = 3;
  AEGIS_ERROR_CODE_NOT_FOUND = 4;
  AEGIS_ERROR_CODE_CONFLICT = 5;
  AEGIS_ERROR_CODE_RATE_LIMITED = 6;

  // Domain errors
  AEGIS_ERROR_CODE_CLASSIFICATION_UNAVAILABLE = 10;
  AEGIS_ERROR_CODE_VERIFICATION_PLAN_INVALID = 11;
  AEGIS_ERROR_CODE_VERIFICATION_FAILED = 12;
  AEGIS_ERROR_CODE_REQUIRED_TIER_NOT_MET = 13;

  AEGIS_ERROR_CODE_RISK_BUDGET_EXHAUSTED = 20;
  AEGIS_ERROR_CODE_AUTONOMY_THROTTLED = 21;

  AEGIS_ERROR_CODE_WARRANTY_QUOTE_EXPIRED = 30;
  AEGIS_ERROR_CODE_WARRANTY_NOT_ACTIVE = 31;
  AEGIS_ERROR_CODE_BOND_INSUFFICIENT = 32;
  AEGIS_ERROR_CODE_CLAIM_NOT_ALLOWED = 33;
  AEGIS_ERROR_CODE_DISPUTE_NOT_ALLOWED = 34;

  // Dependency / downstream failures (money movement is executed via Hydra + wallet executor).
  AEGIS_ERROR_CODE_HYDRA_UNAVAILABLE = 40;
  AEGIS_ERROR_CODE_HYDRA_REJECTED = 41;
  AEGIS_ERROR_CODE_WALLET_EXECUTOR_UNAVAILABLE = 42;
  AEGIS_ERROR_CODE_WALLET_EXECUTOR_REJECTED = 43;

  AEGIS_ERROR_CODE_INTERNAL_ERROR = 100;
}

message AegisError {
  AegisErrorCode code = 1;
  string message = 2;
  repeated string details = 3;
  uint32 retry_after_ms = 4;

  // Optional mapping to canonical policy/runtime reason taxonomy.
  openagents.protocol.v1.ReasonCode reason_code = 10;
  string reason_code_text = 11;

  // Request correlation.
  string request_id = 20;
}

// Generic money container.
// For Lightning settlement flows, use currency="msats" and amount=<msats>.
message AegisMoney {
  string currency = 1; // "msats" | "sats" | "usd_cents" | ...
  uint64 amount = 2;
}

// Explicit hierarchical budget scope (org/project/repo/issue).
message AegisBudgetScope {
  string org_id = 1;
  string project_id = 2;
  string repo_id = 3;
  string issue_id = 4;
}

// Stable linkage fields used across OpenAgents for auditability.
// These are pointers to authoritative artifacts/state elsewhere.
//
// Notes:
// - `trajectory_hash`, `job_hash`, `objective_hash` should be deterministic sha256:<hex> strings.
// - This is the “money/trust → why → proof” binding.
message AegisLinkage {
  string session_id = 1;
  string run_id = 2;
  string trajectory_hash = 3;

  // Deterministic compute/job hash from job registry.
  string job_hash = 4;

  // Deterministic objective hash (e.g., job request canonical hash).
  string objective_hash = 5;

  // Policy bundle ID for the compiled policy/routing configuration used.
  string policy_bundle_id = 6;

  google.protobuf.Struct metadata = 20; // non-contract hints
}

// Evidence reference (portable pointer) for replay logs, receipts, artifacts, etc.
message AegisEvidenceRef {
  string sha256 = 1;         // sha256:<hex> (recommended)
  string url = 2;            // optional URL to object storage / gateway
  string storage = 3;        // "gcs" | "s3" | "ipfs" | "local" | ...
  string content_type = 4;   // e.g. "application/json", "text/plain"
  uint64 size_bytes = 5;
  google.protobuf.Struct metadata = 20;
}

// Generic fee component decomposition to keep quotes/receipts machine-comparable.
message AegisFeeComponent {
  string component = 1; // "underwriter_fee" | "bond_cost" | "verification_cost" | ...
  AegisMoney amount = 2;
  string notes = 3;
}

// Receipt pointer for large receipts externalized to object storage.
message AegisReceiptRef {
  string receipt_id = 1;
  string canonical_json_sha256 = 2; // sha256:<hex> of receipt canonical JSON
  string receipt_url = 3;
}

//
// -----------------------------
// Work units + verifiability
// -----------------------------

enum AegisWorkUnitKind {
  AEGIS_WORK_UNIT_KIND_UNSPECIFIED = 0;

  // OpenAgents-native
  AEGIS_WORK_UNIT_KIND_VERIFIED_PATCH_BUNDLE = 1;
  AEGIS_WORK_UNIT_KIND_OBJECTIVE_COMPUTE_JOB = 2;   // e.g. oa.sandbox_run.v1
  AEGIS_WORK_UNIT_KIND_SUBJECTIVE_COMPUTE_JOB = 3;  // e.g. analysis/adjudicated jobs
  AEGIS_WORK_UNIT_KIND_SKILL_INVOCATION = 4;
  AEGIS_WORK_UNIT_KIND_L402_CALL = 5;

  // Generic external outcome
  AEGIS_WORK_UNIT_KIND_CUSTOM = 100;
}

enum AegisVerifiabilityClass {
  AEGIS_VERIFIABILITY_CLASS_UNSPECIFIED = 0;

  // Deterministic harness exists (tests/hashes/invariants).
  AEGIS_VERIFIABILITY_CLASS_OBJECTIVE = 1;

  // Judgment required; can be verified via redundancy/adjudication/human sampling.
  AEGIS_VERIFIABILITY_CLASS_SUBJECTIVE = 2;

  // Weakly verifiable / long feedback latency / tacit; requires higher friction.
  AEGIS_VERIFIABILITY_CLASS_LOW = 3;
}

enum AegisVerificationTier {
  AEGIS_VERIFICATION_TIER_UNSPECIFIED = 0;

  // Tier O (Objective): deterministic harness checks.
  AEGIS_VERIFICATION_TIER_OBJECTIVE = 1;

  // Tier 1 (Correlated): checker shares major blind spots with doer (cheap, low trust).
  AEGIS_VERIFICATION_TIER_CORRELATED = 2;

  // Tier 2 (Heterogeneous): checker is from a different provider/family.
  AEGIS_VERIFICATION_TIER_HETEROGENEOUS = 3;

  // Tier 3 (Redundancy + Adjudication): best-of-N with explicit adjudication.
  AEGIS_VERIFICATION_TIER_REDUNDANT_ADJUDICATED = 4;

  // Tier 4 (Human): explicit human review/underwriting/audit.
  AEGIS_VERIFICATION_TIER_HUMAN = 5;
}

enum AegisFeedbackLatencyClass {
  AEGIS_FEEDBACK_LATENCY_CLASS_UNSPECIFIED = 0;
  AEGIS_FEEDBACK_LATENCY_CLASS_MS = 1;
  AEGIS_FEEDBACK_LATENCY_CLASS_MINUTES = 2;
  AEGIS_FEEDBACK_LATENCY_CLASS_HOURS = 3;
  AEGIS_FEEDBACK_LATENCY_CLASS_DAYS = 4;
  AEGIS_FEEDBACK_LATENCY_CLASS_MONTHS = 5;
}

message AegisWorkUnitDescriptor {
  string work_unit_id = 1;      // server-generated stable id (optional in requests; Aegis can assign)
  AegisWorkUnitKind kind = 2;

  // Stable identifiers for cross-system linkage
  string job_type = 3;          // e.g. "oa.sandbox_run.v1" (optional)
  string objective_hash = 4;    // sha256:<hex> (recommended when applicable)
  string job_hash = 5;          // sha256:<hex> (recommended when applicable)

  // Artifact pointers (optional)
  repeated AegisEvidenceRef artifacts = 10;

  // Linkage to run/trajectory/policy
  AegisLinkage linkage = 20;
  AegisBudgetScope budget_scope = 21;

  google.protobuf.Struct metadata = 90;
}

message AegisClassification {
  string classification_id = 1;
  string work_unit_id = 2;

  AegisVerifiabilityClass verifiability_class = 10;
  AegisFeedbackLatencyClass feedback_latency = 11;

  // Recommended minimum tier for safe deployment.
  AegisVerificationTier recommended_min_tier = 20;

  // Optional risk score (0..1) for observability/debugging (non-binding).
  double risk_score = 30;

  repeated string notes = 40;

  int64 classified_at_ms = 50;

  string canonical_json_sha256 = 60;
  google.protobuf.Struct metadata = 90;
}

message AegisClassifyRequest {
  string request_id = 1;
  string idempotency_key = 2;

  AegisWorkUnitDescriptor work = 10;

  // Optional policy hints for classifier.
  google.protobuf.Struct policy_context = 20;
}

message AegisClassifyResult {
  bool ok = 1;
  bool idempotent_replay = 2;

  AegisClassification classification = 3;

  AegisError error = 4;
}

//
// -----------------------------
// Verification plans + execution
// -----------------------------

enum AegisVerificationActorKind {
  AEGIS_VERIFICATION_ACTOR_KIND_UNSPECIFIED = 0;
  AEGIS_VERIFICATION_ACTOR_KIND_SYSTEM = 1;     // harness / deterministic checks
  AEGIS_VERIFICATION_ACTOR_KIND_MODEL = 2;      // AI checker
  AEGIS_VERIFICATION_ACTOR_KIND_HUMAN = 3;      // human reviewer/underwriter
  AEGIS_VERIFICATION_ACTOR_KIND_EXTERNAL = 4;   // third-party auditor
}

enum AegisVerificationStepKind {
  AEGIS_VERIFICATION_STEP_KIND_UNSPECIFIED = 0;
  AEGIS_VERIFICATION_STEP_KIND_OBJECTIVE_HARNESS = 1;
  AEGIS_VERIFICATION_STEP_KIND_MODEL_CHECK = 2;
  AEGIS_VERIFICATION_STEP_KIND_ADJUDICATION = 3;
  AEGIS_VERIFICATION_STEP_KIND_HUMAN_REVIEW = 4;
  AEGIS_VERIFICATION_STEP_KIND_SECURITY_AUDIT = 5;
  AEGIS_VERIFICATION_STEP_KIND_CUSTOM = 100;
}

message AegisVerificationIndependencePolicy {
  // True if the checker must be from a different provider/family than the doer.
  bool require_heterogeneous = 1;

  // Minimum number of distinct checkers (for redundancy tiers).
  uint32 min_distinct_checkers = 2;

  // Optional allowed checker provider keys (strings, policy-defined).
  repeated string allowed_checker_providers = 3;

  // Optional disallowed checker provider keys.
  repeated string blocked_checker_providers = 4;

  google.protobuf.Struct metadata = 20;
}

message AegisVerificationPlan {
  string plan_id = 1;
  string work_unit_id = 2;

  // Minimum tier required by policy.
  AegisVerificationTier required_tier = 10;

  // Upper bound on tiers allowed (optional).
  optional AegisVerificationTier max_tier = 11;

  // Independence constraints.
  AegisVerificationIndependencePolicy independence = 20;

  // Optional sampling policy (for human sampling lanes).
  // Example: { "sample_rate": 0.01, "stratify_by": ["provider_id"] }
  google.protobuf.Struct sampling_policy = 30;

  // Optional time/effort bounds (non-binding).
  optional uint64 max_verification_cost_microusd = 40;
  optional uint32 max_verification_latency_ms = 41;

  int64 created_at_ms = 50;

  string canonical_json_sha256 = 60;

  google.protobuf.Struct metadata = 90;
}

message AegisVerificationStepRecord {
  string step_id = 1;
  string plan_id = 2;
  string work_unit_id = 3;

  AegisVerificationStepKind step_kind = 10;
  AegisVerificationActorKind actor_kind = 11;
  string actor_id = 12; // system/model/human identifier (opaque string)

  // For model checks, record provider/family keys to assess correlation.
  string checker_provider = 20; // e.g. "openai", "anthropic", "local_gpt_oss"
  string checker_model = 21;    // optional

  bool ok = 30;
  openagents.protocol.v1.ReasonCode reason_code = 31;
  string reason_code_text = 32;

  // Evidence pointers for this step (logs, reports, diffs, etc.)
  repeated AegisEvidenceRef evidence = 40;

  int64 started_at_ms = 50;
  int64 completed_at_ms = 51;
  uint64 latency_ms = 52;

  google.protobuf.Struct metadata = 90;
}

enum AegisVerificationOutcome {
  AEGIS_VERIFICATION_OUTCOME_UNSPECIFIED = 0;
  AEGIS_VERIFICATION_OUTCOME_PASSED = 1;
  AEGIS_VERIFICATION_OUTCOME_FAILED = 2;
  AEGIS_VERIFICATION_OUTCOME_INCONCLUSIVE = 3;
  AEGIS_VERIFICATION_OUTCOME_ESCALATED = 4; // escalated to higher tier
}

message AegisVerificationReceipt {
  string receipt_version = 1; // e.g. "openagents.aegis.verification_receipt.v1"
  string receipt_id = 2;
  string request_id = 3;

  string work_unit_id = 10;
  string classification_id = 11;
  string plan_id = 12;

  AegisVerifiabilityClass verifiability_class = 20;
  AegisVerificationTier required_tier = 21;
  AegisVerificationTier achieved_tier = 22;

  AegisVerificationOutcome outcome = 30;
  double confidence = 31; // 0..1, best-effort for subjective lanes

  // Steps performed (ordered).
  repeated AegisVerificationStepRecord steps = 40;

  // Portable evidence pointers (summary-level).
  repeated AegisEvidenceRef evidence = 50;

  // Linkage to run/job/policy bundle.
  AegisLinkage linkage = 60;
  AegisBudgetScope budget_scope = 61;

  int64 verified_at_ms = 70;

  string canonical_json_sha256 = 80;

  google.protobuf.Struct metadata = 90;
}

message AegisVerifyRequest {
  string request_id = 1;
  string idempotency_key = 2;

  // Work unit to verify.
  AegisWorkUnitDescriptor work = 10;

  // Optional classification override (if caller already classified).
  optional string classification_id = 11;

  // Optional plan override (if caller supplies explicit plan constraints).
  optional AegisVerificationPlan plan_override = 12;

  // Policy hints and context.
  google.protobuf.Struct policy_context = 20;
}

message AegisVerifyResult {
  bool ok = 1;
  bool idempotent_replay = 2;

  AegisVerificationReceipt receipt = 3;
  AegisReceiptRef receipt_ref = 4;

  AegisError error = 5;
}

//
// -----------------------------
// Risk budgets + autonomy throttle
// -----------------------------

enum AegisAutonomyThrottleState {
  AEGIS_AUTONOMY_THROTTLE_STATE_UNSPECIFIED = 0;
  AEGIS_AUTONOMY_THROTTLE_STATE_NORMAL = 1;
  AEGIS_AUTONOMY_THROTTLE_STATE_DEGRADED = 2;          // increase verification tiers / reduce spend
  AEGIS_AUTONOMY_THROTTLE_STATE_APPROVAL_REQUIRED = 3; // require operator approvals for risky actions
  AEGIS_AUTONOMY_THROTTLE_STATE_HALTED = 4;            // hard stop
}

message AegisRiskBudgetConfig {
  string budget_id = 1;
  AegisBudgetScope scope = 2;

  // Hard ceilings (per day) for unverified or low-trust work.
  uint32 max_unverified_work_units_per_day = 10;
  uint32 max_low_tier_work_units_per_day = 11; // includes correlated checks
  uint32 max_correlated_checks_per_day = 12;

  // Minimum verified share (0..1). If violated, throttle escalates.
  double min_verified_share = 20;

  // Required minimum tier for high-risk classes (policy-defined mapping).
  AegisVerificationTier min_tier_for_high_risk = 30;

  // Thresholds for throttle escalation.
  double dispute_rate_halt_threshold = 40; // 0..1
  double loss_rate_halt_threshold = 41;    // 0..1

  int64 created_at_ms = 50;
  int64 updated_at_ms = 51;

  google.protobuf.Struct metadata = 90;
}

message AegisRiskBudgetSetRequest {
  string request_id = 1;
  string idempotency_key = 2;

  AegisRiskBudgetConfig config = 10;
}

message AegisRiskBudgetSetResult {
  bool ok = 1;
  bool idempotent_replay = 2;

  AegisRiskBudgetConfig config = 3;
  AegisError error = 4;
}

message AegisRiskBudgetStatus {
  string budget_id = 1;
  AegisBudgetScope scope = 2;

  // Rolling counters (24h window, implementation-defined).
  uint32 unverified_work_units_24h = 10;
  uint32 low_tier_work_units_24h = 11;
  uint32 correlated_checks_24h = 12;

  // Measured shares (0..1).
  double verified_share_24h = 20;
  double objective_share_24h = 21;
  double heterogeneous_share_24h = 22;
  double human_share_24h = 23;

  // Rates (0..1).
  double dispute_rate_24h = 30;
  double loss_rate_24h = 31;

  // Current throttle state.
  AegisAutonomyThrottleState throttle_state = 40;
  string throttle_reason = 41;
  openagents.protocol.v1.ReasonCode throttle_reason_code = 42;

  int64 observed_at_ms = 50;
  google.protobuf.Struct metadata = 90;
}

message AegisRiskBudgetStatusRequest {
  string request_id = 1;
  AegisBudgetScope scope = 2;
}

message AegisRiskBudgetStatusResult {
  bool ok = 1;
  AegisRiskBudgetStatus status = 2;
  AegisError error = 3;
}

message AegisAutonomyThrottleDecision {
  string decision_id = 1;
  AegisBudgetScope scope = 2;

  AegisAutonomyThrottleState state = 10;
  string reason = 11;
  openagents.protocol.v1.ReasonCode reason_code = 12;

  // Optional linkages for audit.
  string policy_bundle_id = 20;
  string evaluation_hash = 21;

  int64 effective_at_ms = 30;
  int64 decided_at_ms = 31;

  google.protobuf.Struct metadata = 90;
}

//
// -----------------------------
// Underwriting: warranties + bonds + claims/disputes
// -----------------------------

enum AegisWarrantyStatus {
  AEGIS_WARRANTY_STATUS_UNSPECIFIED = 0;
  AEGIS_WARRANTY_STATUS_QUOTED = 1;
  AEGIS_WARRANTY_STATUS_ACTIVE = 2;
  AEGIS_WARRANTY_STATUS_EXPIRED = 3;
  AEGIS_WARRANTY_STATUS_CANCELED = 4;
  AEGIS_WARRANTY_STATUS_CLAIMED = 5;
}

message AegisWarrantyTerms {
  // What is being warranted (scope is policy-defined string key).
  string warranty_scope = 1; // e.g. "verified_patch_bundle", "objective_job_result"
  string exclusions = 2;     // human-readable exclusions (not contract-critical; may be structured later)

  // Required verification tier for warranty issuance.
  AegisVerificationTier required_tier = 10;

  // Coverage duration (ms) from issuance.
  uint64 duration_ms = 20;

  google.protobuf.Struct metadata = 90;
}

message AegisWarrantyQuoteRequest {
  string request_id = 1;
  string idempotency_key = 2;

  // Work unit to warrant.
  AegisWorkUnitDescriptor work = 10;

  // The verification receipt that supports warranty issuance (must meet required tier).
  optional string verification_receipt_id = 11;

  // Desired coverage cap (msats).
  uint64 desired_coverage_msats = 20;

  // Optional terms hints (policy may override).
  AegisWarrantyTerms terms_hint = 30;

  google.protobuf.Struct policy_context = 90;
}

message AegisWarrantyQuote {
  string quote_id = 1;

  string work_unit_id = 2;
  uint64 coverage_cap_msats = 3;

  // Bond/collateral required to back the warranty (msats).
  uint64 bond_required_msats = 10;

  // Fee charged to issue warranty (msats).
  uint64 warranty_fee_msats = 11;

  repeated AegisFeeComponent fees = 12;

  // Terms frozen into this quote.
  AegisWarrantyTerms terms = 20;

  int64 valid_until_ms = 30;
  int64 created_at_ms = 31;

  string canonical_json_sha256 = 40;

  google.protobuf.Struct metadata = 90;
}

message AegisWarrantyQuoteResult {
  bool ok = 1;
  bool idempotent_replay = 2;

  AegisWarrantyQuote quote = 3;

  AegisError error = 4;
}

// Bonds are reserved/committed via Hydra in implementation.
// This proto models the bond record and references settlement proofs.
//
// NOTE: We intentionally do not import Hydra proto here to avoid tight coupling;
// use `hydra_*` string refs for receipt pointers.
enum AegisBondStatus {
  AEGIS_BOND_STATUS_UNSPECIFIED = 0;
  AEGIS_BOND_STATUS_RESERVED = 1;
  AEGIS_BOND_STATUS_RELEASED = 2;
  AEGIS_BOND_STATUS_SLASHED = 3;
  AEGIS_BOND_STATUS_EXPIRED = 4;
}

message AegisBondRecord {
  string bond_id = 1;
  string work_unit_id = 2;

  uint64 reserved_msats = 10;
  AegisBondStatus status = 11;

  int64 reserved_at_ms = 20;
  int64 expires_at_ms = 21;
  int64 updated_at_ms = 22;

  // Hydra receipt pointers for bond reservation/release/slash operations.
  string hydra_receipt_id = 30;
  string hydra_receipt_sha256 = 31;
  string hydra_receipt_url = 32;

  google.protobuf.Struct metadata = 90;
}

message AegisWarrantyIssueRequest {
  string request_id = 1;
  string idempotency_key = 2;

  string quote_id = 10;

  // Principal requesting warranty issuance.
  string buyer_id = 20;

  // Underwriter identity (opaque string; may be system/human/third-party).
  string underwriter_id = 21;

  // Optional: provide an existing bond reservation id if pre-reserved.
  optional string bond_id = 30;

  google.protobuf.Struct policy_context = 90;
}

message AegisWarranty {
  string warranty_id = 1;
  string quote_id = 2;

  string work_unit_id = 3;

  AegisWarrantyStatus status = 10;

  uint64 coverage_cap_msats = 20;
  uint64 warranty_fee_msats = 21;

  // Bond backing this warranty.
  AegisBondRecord bond = 30;

  AegisWarrantyTerms terms = 40;

  // Linkage to verification support.
  optional string verification_receipt_id = 50;
  optional string verification_receipt_sha256 = 51;

  int64 issued_at_ms = 60;
  int64 expires_at_ms = 61;

  string canonical_json_sha256 = 70;

  google.protobuf.Struct metadata = 90;
}

message AegisWarrantyIssueReceipt {
  string receipt_version = 1; // "openagents.aegis.warranty_issue_receipt.v1"
  string receipt_id = 2;
  string request_id = 3;

  AegisWarranty warranty = 10;

  // Optional settlement proof for warranty fee payment (if charged immediately).
  optional openagents.lightning.v1.WalletExecutionReceipt wallet_receipt = 20;

  int64 issued_at_ms = 30;

  string canonical_json_sha256 = 40;
}

message AegisWarrantyIssueResult {
  bool ok = 1;
  bool idempotent_replay = 2;

  AegisWarrantyIssueReceipt receipt = 3;
  AegisReceiptRef receipt_ref = 4;

  AegisError error = 5;
}

//
// Claims and disputes
//

enum AegisClaimStatus {
  AEGIS_CLAIM_STATUS_UNSPECIFIED = 0;
  AEGIS_CLAIM_STATUS_OPEN = 1;
  AEGIS_CLAIM_STATUS_UNDER_REVIEW = 2;
  AEGIS_CLAIM_STATUS_RESOLVED = 3;
  AEGIS_CLAIM_STATUS_DENIED = 4;
  AEGIS_CLAIM_STATUS_CANCELED = 5;
}

enum AegisClaimOutcome {
  AEGIS_CLAIM_OUTCOME_UNSPECIFIED = 0;
  AEGIS_CLAIM_OUTCOME_APPROVED_PAYOUT = 1;
  AEGIS_CLAIM_OUTCOME_APPROVED_PARTIAL = 2;
  AEGIS_CLAIM_OUTCOME_DENIED = 3;
  AEGIS_CLAIM_OUTCOME_ESCALATED = 4;
}

message AegisClaimOpenRequest {
  string request_id = 1;
  string idempotency_key = 2;

  string warranty_id = 10;
  string claimant_id = 11;

  // Claimed loss amount (msats), capped by warranty.
  uint64 claimed_amount_msats = 20;

  string claim_text = 30;
  repeated AegisEvidenceRef evidence = 31;

  AegisLinkage linkage = 40;
  AegisBudgetScope budget_scope = 41;

  google.protobuf.Struct metadata = 90;
}

message AegisClaimRecord {
  string claim_id = 1;
  string warranty_id = 2;

  string claimant_id = 3;

  AegisClaimStatus status = 10;

  uint64 claimed_amount_msats = 20;

  string claim_text = 30;
  repeated AegisEvidenceRef evidence = 31;

  int64 opened_at_ms = 40;
  int64 updated_at_ms = 41;

  google.protobuf.Struct metadata = 90;
}

message AegisClaimOpenResult {
  bool ok = 1;
  bool idempotent_replay = 2;
  AegisClaimRecord claim = 3;
  AegisError error = 4;
}

message AegisClaimResolveRequest {
  string request_id = 1;
  string idempotency_key = 2;

  string claim_id = 10;
  string resolver_id = 11; // underwriter / arbitrator / system

  AegisClaimOutcome outcome = 20;

  // Payout amount (msats) if approved (0 if denied).
  uint64 payout_amount_msats = 21;

  string resolution_reason = 30;
  openagents.protocol.v1.ReasonCode reason_code = 31;

  // Optional: attach settlement proof if payout/refund executed.
  optional openagents.lightning.v1.WalletExecutionReceipt wallet_receipt = 40;

  // Optional: Hydra receipt refs if payout executed via Hydra lanes.
  optional string hydra_receipt_id = 50;
  optional string hydra_receipt_sha256 = 51;
  optional string hydra_receipt_url = 52;

  repeated AegisEvidenceRef evidence = 60;

  google.protobuf.Struct metadata = 90;
}

message AegisClaimResolutionReceipt {
  string receipt_version = 1; // "openagents.aegis.claim_resolution_receipt.v1"
  string receipt_id = 2;
  string request_id = 3;

  AegisClaimRecord claim = 10;

  AegisClaimOutcome outcome = 20;
  uint64 payout_amount_msats = 21;

  string resolution_reason = 30;
  openagents.protocol.v1.ReasonCode reason_code = 31;

  optional openagents.lightning.v1.WalletExecutionReceipt wallet_receipt = 40;

  optional string hydra_receipt_id = 50;
  optional string hydra_receipt_sha256 = 51;
  optional string hydra_receipt_url = 52;

  int64 resolved_at_ms = 60;

  string canonical_json_sha256 = 70;
}

message AegisClaimResolveResult {
  bool ok = 1;
  bool idempotent_replay = 2;

  AegisClaimResolutionReceipt receipt = 3;
  AegisReceiptRef receipt_ref = 4;

  AegisError error = 5;
}

// Disputes are general-purpose disputes that may exist without a warranty.
// They can reference a work unit or external commerce ids.
enum AegisDisputeStatus {
  AEGIS_DISPUTE_STATUS_UNSPECIFIED = 0;
  AEGIS_DISPUTE_STATUS_OPEN = 1;
  AEGIS_DISPUTE_STATUS_UNDER_REVIEW = 2;
  AEGIS_DISPUTE_STATUS_RESOLVED = 3;
  AEGIS_DISPUTE_STATUS_CANCELED = 4;
}

enum AegisDisputeOutcome {
  AEGIS_DISPUTE_OUTCOME_UNSPECIFIED = 0;
  AEGIS_DISPUTE_OUTCOME_BUYER_WINS = 1;
  AEGIS_DISPUTE_OUTCOME_PROVIDER_WINS = 2;
  AEGIS_DISPUTE_OUTCOME_SPLIT = 3;
  AEGIS_DISPUTE_OUTCOME_ESCALATED = 4;
}

message AegisDisputeOpenRequest {
  string request_id = 1;
  string idempotency_key = 2;

  // Optional warranty linkage.
  optional string warranty_id = 10;

  // Optional work unit linkage.
  optional string work_unit_id = 11;

  // Optional external identifiers (marketplace order ids, etc.).
  optional string external_order_id = 12;

  string opened_by = 20;
  string claim = 21;

  repeated AegisEvidenceRef evidence = 30;

  AegisLinkage linkage = 40;
  AegisBudgetScope budget_scope = 41;

  google.protobuf.Struct metadata = 90;
}

message AegisDisputeRecord {
  string dispute_id = 1;

  optional string warranty_id = 10;
  optional string work_unit_id = 11;
  optional string external_order_id = 12;

  string opened_by = 20;
  string claim = 21;

  AegisDisputeStatus status = 30;

  repeated AegisEvidenceRef evidence = 40;

  int64 opened_at_ms = 50;
  int64 updated_at_ms = 51;

  google.protobuf.Struct metadata = 90;
}

message AegisDisputeOpenResult {
  bool ok = 1;
  bool idempotent_replay = 2;

  AegisDisputeRecord dispute = 3;
  AegisError error = 4;
}

message AegisDisputeArbitrateRequest {
  string request_id = 1;
  string idempotency_key = 2;

  string dispute_id = 10;
  string arbitrator_id = 11;

  AegisDisputeOutcome outcome = 20;

  // Optional settlement amounts (msats) when dispute triggers money movement.
  uint64 buyer_amount_msats = 21;
  uint64 provider_amount_msats = 22;

  string decision_reason = 30;
  openagents.protocol.v1.ReasonCode reason_code = 31;

  // Optional settlement proofs
  optional openagents.lightning.v1.WalletExecutionReceipt wallet_receipt = 40;

  optional string hydra_receipt_id = 50;
  optional string hydra_receipt_sha256 = 51;
  optional string hydra_receipt_url = 52;

  repeated AegisEvidenceRef evidence = 60;

  google.protobuf.Struct metadata = 90;
}

message AegisDisputeArbitrationReceipt {
  string receipt_version = 1; // "openagents.aegis.dispute_arbitration_receipt.v1"
  string receipt_id = 2;
  string request_id = 3;

  AegisDisputeRecord dispute = 10;

  AegisDisputeOutcome outcome = 20;
  uint64 buyer_amount_msats = 21;
  uint64 provider_amount_msats = 22;

  string decision_reason = 30;
  openagents.protocol.v1.ReasonCode reason_code = 31;

  optional openagents.lightning.v1.WalletExecutionReceipt wallet_receipt = 40;

  optional string hydra_receipt_id = 50;
  optional string hydra_receipt_sha256 = 51;
  optional string hydra_receipt_url = 52;

  int64 decided_at_ms = 60;

  string canonical_json_sha256 = 70;
}

message AegisDisputeArbitrateResult {
  bool ok = 1;
  bool idempotent_replay = 2;

  AegisDisputeArbitrationReceipt receipt = 3;
  AegisReceiptRef receipt_ref = 4;

  AegisError error = 5;
}

//
// -----------------------------
// Ground Truth Registry
// -----------------------------

enum AegisGroundTruthKind {
  AEGIS_GROUND_TRUTH_KIND_UNSPECIFIED = 0;
  AEGIS_GROUND_TRUTH_KIND_INCIDENT = 1;
  AEGIS_GROUND_TRUTH_KIND_NEAR_MISS = 2;
  AEGIS_GROUND_TRUTH_KIND_POSTMORTEM = 3;
  AEGIS_GROUND_TRUTH_KIND_POLICY_REGRESSION = 4;
  AEGIS_GROUND_TRUTH_KIND_TOOLCHAIN_REGRESSION = 5;
  AEGIS_GROUND_TRUTH_KIND_CUSTOM = 100;
}

enum AegisGroundTruthSeverity {
  AEGIS_GROUND_TRUTH_SEVERITY_UNSPECIFIED = 0;
  AEGIS_GROUND_TRUTH_SEVERITY_LOW = 1;
  AEGIS_GROUND_TRUTH_SEVERITY_MEDIUM = 2;
  AEGIS_GROUND_TRUTH_SEVERITY_HIGH = 3;
  AEGIS_GROUND_TRUTH_SEVERITY_CRITICAL = 4;
}

message AegisGroundTruthCaseCreateRequest {
  string request_id = 1;
  string idempotency_key = 2;

  AegisGroundTruthKind kind = 10;
  AegisGroundTruthSeverity severity = 11;

  string title = 20;
  string summary = 21;

  // Linkages to relevant runs/jobs/policies.
  AegisLinkage linkage = 30;
  AegisBudgetScope budget_scope = 31;

  // Evidence pointers (replay logs, receipts, diffs, incident timelines).
  repeated AegisEvidenceRef evidence = 40;

  // Optional tags for retrieval.
  repeated string tags = 50;

  google.protobuf.Struct metadata = 90;
}

message AegisGroundTruthCase {
  string case_id = 1;

  AegisGroundTruthKind kind = 10;
  AegisGroundTruthSeverity severity = 11;

  string title = 20;
  string summary = 21;

  AegisLinkage linkage = 30;
  AegisBudgetScope budget_scope = 31;

  repeated AegisEvidenceRef evidence = 40;
  repeated string tags = 50;

  int64 created_at_ms = 60;
  int64 updated_at_ms = 61;

  string canonical_json_sha256 = 70;

  google.protobuf.Struct metadata = 90;
}

message AegisGroundTruthCaseResult {
  bool ok = 1;
  bool idempotent_replay = 2;

  AegisGroundTruthCase gt_case = 3;

  AegisError error = 4;
}

//
// -----------------------------
// Synthetic Practice / Simulator
// -----------------------------

enum AegisSimulationScenarioKind {
  AEGIS_SIMULATION_SCENARIO_KIND_UNSPECIFIED = 0;
  AEGIS_SIMULATION_SCENARIO_KIND_CODE_REVIEW = 1;
  AEGIS_SIMULATION_SCENARIO_KIND_SECURITY_REVIEW = 2;
  AEGIS_SIMULATION_SCENARIO_KIND_INCIDENT_RESPONSE = 3;
  AEGIS_SIMULATION_SCENARIO_KIND_DISPUTE_ADJUDICATION = 4;
  AEGIS_SIMULATION_SCENARIO_KIND_UNDERWRITING = 5;
  AEGIS_SIMULATION_SCENARIO_KIND_CUSTOM = 100;
}

message AegisSimulationScenario {
  string scenario_id = 1;

  AegisSimulationScenarioKind kind = 10;

  // Optional linkage to a real ground-truth case.
  optional string derived_case_id = 11;

  // Difficulty 1..10 (policy-defined).
  uint32 difficulty = 20;

  // Scenario instructions and artifacts.
  repeated AegisEvidenceRef materials = 30;

  // Scoring rules version key (implementation-defined).
  string scoring_version = 40;

  int64 created_at_ms = 50;

  google.protobuf.Struct metadata = 90;
}

enum AegisSimulationRunOutcome {
  AEGIS_SIMULATION_RUN_OUTCOME_UNSPECIFIED = 0;
  AEGIS_SIMULATION_RUN_OUTCOME_PASSED = 1;
  AEGIS_SIMULATION_RUN_OUTCOME_FAILED = 2;
  AEGIS_SIMULATION_RUN_OUTCOME_INCONCLUSIVE = 3;
}

message AegisSimulationRunRequest {
  string request_id = 1;
  string idempotency_key = 2;

  string scenario_id = 10;

  // Participant identity (human verifier / underwriter / agent).
  string participant_id = 20;
  string participant_kind = 21; // "human" | "agent" | "system" (string for flexibility)

  // Submission payload (answers, decisions, annotations). Structured later.
  google.protobuf.Struct submission = 30;

  google.protobuf.Struct metadata = 90;
}

message AegisSimulationRunRecord {
  string run_id = 1;
  string scenario_id = 2;

  string participant_id = 10;
  string participant_kind = 11;

  // Score 0..1 and optional breakdown.
  double score = 20;
  google.protobuf.Struct score_breakdown = 21;

  AegisSimulationRunOutcome outcome = 30;

  // Feedback to participant.
  string feedback = 40;
  repeated AegisEvidenceRef feedback_evidence = 41;

  int64 started_at_ms = 50;
  int64 completed_at_ms = 51;

  string canonical_json_sha256 = 60;

  google.protobuf.Struct metadata = 90;
}

message AegisSimulationRunResult {
  bool ok = 1;
  bool idempotent_replay = 2;
  AegisSimulationRunRecord run = 3;
  AegisError error = 4;
}

//
// -----------------------------
// Unified result envelope (optional convenience)
// -----------------------------
//
// Aegis APIs can return one typed envelope over HTTP, while still preserving
// deterministic receipts and idempotent semantics.

message AegisResultEnvelope {
  bool ok = 1;
  AegisError error = 2;

  oneof result {
    AegisClassifyResult classify = 10;
    AegisVerifyResult verify = 11;

    AegisRiskBudgetSetResult risk_budget_set = 20;
    AegisRiskBudgetStatusResult risk_budget_status = 21;

    AegisWarrantyQuoteResult warranty_quote = 30;
    AegisWarrantyIssueResult warranty_issue = 31;

    AegisClaimOpenResult claim_open = 40;
    AegisClaimResolveResult claim_resolve = 41;

    AegisDisputeOpenResult dispute_open = 50;
    AegisDisputeArbitrateResult dispute_arbitrate = 51;

    AegisGroundTruthCaseResult ground_truth_case = 60;

    AegisSimulationRunResult simulation_run = 70;
  }
}
```
