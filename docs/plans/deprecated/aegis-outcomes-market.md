# Spec: Aegis Outcomes Markets — Warranted Work + Belief Markets (Hydra-settled, Aegis-verified)

**Status:** Canonical draft (new).
**Purpose:** Define the mechanism that turns **agent output quality** into **priced risk** and **guaranteed outcomes**, using **Aegis** for verification/liability and **Hydra** for escrow/bonds/settlement.

This spec covers two closely related market types:

1. **Work Outcome Markets** (the compute marketplace version): agents *do work* and **stake/bond** on the correctness of their own outputs; third parties can underwrite/bet on outcomes; buyers can optionally purchase warranties; disputes/claims are first-class.
2. **Belief Markets** (the prediction-market version): agents publish probabilistic beliefs about external events; the system aggregates beliefs; markets price probabilities; participants can trade; forecasts become a sellable “probability feed.”

Both are the same economic machine: **uncertainty is priced**, **verification resolves**, and **capital flows to calibrated agents**.

---

## 0) Recommended restructuring of your existing docs

You already have strong separation between **Hydra (capital)** and **Aegis (verification/liability)**. Keep that. Add this as a third spec that composes them.

Recommended doc layout:

* **Hydra Core** (LLP + CEP + routing + receipts) — stable substrate
* **Hydra X** (solvers + cross-rail intents) — optional expansion track (keep separate)
* **Aegis Core** (verification tiers + autonomy throttle + warranties/claims + simulator) — stable substrate
* **Aegis Outcomes Markets** (this doc) — the “risk market” product that uses Hydra + Aegis

This avoids “Hydra becoming the market” or “Aegis becoming the wallet.” Outcomes Markets are *the mechanism*, not the substrate.

---

## 1) High-level product definition

### What Outcomes Markets does

Outcomes Markets introduces an enforceable economic contract around work:

> “If you claim your output is correct, prove it (Aegis) or **pay** (Hydra).
> If you underwrite someone else’s claim and you’re wrong, you **pay**.
> If you’re right, you **earn** the premium.”

This replaces “trust me bro” with a legible pipeline:

* **work produced** → **verification** → **settlement** → **warranty window** → **claims/disputes** → **finalization**

### What Outcomes Markets is not

* Not a governance token / emission scheme.
* Not “AI verifies AI” as a default.
* Not a requirement to integrate Polymarket/Kalshi/etc.
* Not a retail consumer market. It’s an **agent economy primitive**.

---

## 2) Core invariants (inherits Hydra + Aegis)

**Authority / money invariants (Hydra):**

* Proto-first contracts
* **HTTP-only** authority mutations for money state
* Deterministic receipts for every effect
* Idempotent execution (safe retries, no double-settle)
* No unscoped credit (only CEP envelopes)

**Verification / liability invariants (Aegis):**

* Explicit verifiability classification per WorkUnit
* Independence tiers are explicit and receipted
* Warranties only with explicit terms + collateralization
* Disputes/claims are evidence-driven and receipted
* Autonomy throttle binds deployment to verification capacity

Outcomes Markets introduces one additional invariant:

> **No outcome can become “settled truth” without a verifiable resolution path** (objective harness, adjudication policy, or explicit human underwriting). If resolution is ambiguous, the contract must say so up front (and price it).

---

## 3) Entities, roles, and ledgers

### 3.1 Entities

**WorkUnit**
A job/run/artifact with a clear acceptance criterion (or explicitly subjective criterion with an adjudication process). It has hashes and replay linkage.

**OutcomeContract**
A specific claim about a WorkUnit. Minimal form is binary:

* `PASS` (meets spec) vs `FAIL` (does not meet spec)

You can later support multi-outcome contracts (graded rubrics), but MVP should be binary.

**Warranty**
Optional guarantee that if the outcome is ultimately `FAIL` (within a warranty window), the claimant receives a defined remedy (refund, damages, rework credit).

**Bond / Stake**
Locked collateral posted by participants (worker, underwriter, predictor) that funds remedies when they’re wrong.

**Claim**
Invokes warranty or disputes the result; triggers adjudication and potential payout.

### 3.2 Roles

**Buyer**
Creates WorkUnit and pays for outcome (often via CEP pay-after-verify).

**Worker (Provider Agent)**
Produces output; may post a self-bond (“I bet on my own work”).

**Predictor / Underwriter**
Third party that posts collateral to back an outcome claim (or to bet against it).

**Verifier**
Executes verification plan (objective harness, heterogeneous checks, human review).

**Adjudicator**
Resolves disputes for subjective lanes or contested claims.

### 3.3 Ledgers (Hydra partitions)

Hydra already plans partitioned accounting (LLP/CEP/RRP). Outcomes Markets needs one additional explicit partition:

* **ABP — Aegis Bond Partition**
  Collateral locked for warranties, worker self-bonds, and claim payouts.

ABP is not required day 1 if you run everything operator-funded, but the accounting model should assume it exists so exposure and solvency are legible.

---

## 4) Two market types

### 4.1 Work Outcome Markets (compute marketplace + “bet on your own output”)

The Work Outcome Market sits inside your compute marketplace flow.

**Concept:** every job can optionally be wrapped in an OutcomeContract and optionally a Warranty. Workers and underwriters post bonds; Aegis verifies; Hydra settles.

The “market” dimension can be as lightweight as:

* worker self-bond + optional third-party co-bond + scoring
  …or as advanced as:
* continuous pricing of “pass probability” via an AMM (LMSR) and tradable positions.

MVP should start lightweight (self-bond + co-bond), then upgrade to AMM.

### 4.2 Belief Markets (prediction markets + forecasting agents)

Belief Markets are for external events and produce:

* market price (probability)
* scored agent forecasts
* a high-quality probability feed

They’re optional for MVP of Work Outcome Markets, but the primitives are shared:

* contracts
* collateral
* resolution
* scoring
* receipts

---

## 5) Work Outcome Market lifecycle (canonical state machine)

A WorkUnit can be created either:

* as a plain marketplace job, or
* as a job with OutcomeContract/Warranty terms attached.

### 5.1 State machine

**States**

* `CREATED`
* `FUNDED` (buyer escrow reserved; optionally via CEP)
* `BONDED` (worker/underwriter bonds locked)
* `SUBMITTED` (output delivered with hashes)
* `VERIFYING`
* `VERDICT_PASS` / `VERDICT_FAIL` (Aegis verification receipt)
* `SETTLED` (Hydra payment released / refunds executed)
* `WARRANTY_ACTIVE` (optional window)
* `CLAIM_OPEN` (optional)
* `CLAIM_RESOLVED` (payout/denial)
* `FINALIZED`

Every transition emits a receipt (Aegis for verification/liability, Hydra for money).

### 5.2 Default “pay-after-verify” (recommended)

For objective-verifiable lanes, default settlement is:

* buyer funds escrow (or CEP envelope created)
* worker submits output
* Aegis verifies (Tier O objective)
* Hydra releases payment on PASS
* on FAIL, payment is not released and remedy is executed (refund + bond draw if applicable)

This is the cleanest alignment with your CEP envelope model.

---

## 6) Economic mechanics (how it makes money)

Outcomes Markets is a **risk pricing engine**. Money comes from three places:

### 6.1 Buyer-paid premiums (warranted outcomes)

Buyers pay an explicit premium to get a warranty:

* “Pay X sats extra, and you’re guaranteed a remedy up to Y sats if the outcome fails within window W.”

Premium is distributed to:

* worker (if self-warranting),
* underwriters (if third-party underwriting is used),
* verifiers/adjudicators (verification fees),
* and optionally a small platform fee (explicit, receipted).

### 6.2 Spread / fees from market making (optional later)

If you introduce an AMM (e.g., LMSR) for pass/fail shares:

* LPs earn spread/fees
* the market maker has bounded loss (parameterized)
* you can charge explicit taker fees

This is phase 2+.

### 6.3 Selling the “probability feed” (Belief Markets)

Once you have credible resolution + scoring:

* the aggregate probability stream becomes a product (API, dashboards, alerts)
* customers pay for intelligence, not just trades

This is the “Bloomberg of probabilities” angle.

---

## 7) Pricing and collateral policy (Aegis + Hydra integration)

Aegis is responsible for determining required verification tier and minimum bond profile.

Hydra is responsible for enforcing the bond locks and settlement.

### 7.1 Verifiability class → required protections

Aegis classifies each WorkUnit:

| Class             | Example                             | Default verification           | Default collateral posture                  |
| ----------------- | ----------------------------------- | ------------------------------ | ------------------------------------------- |
| Objective         | tests, hashes, deterministic checks | Tier O                         | pay-after-verify; low bonds optional        |
| Subjective        | design review, writing quality      | Tier 2/3 + adjudication        | bonds required; warranty priced higher      |
| Low-verifiability | long feedback loops, vague specs    | Tier 4 (human) + strict gating | require approvals; bonds + limited autonomy |

### 7.2 Bond types

**Worker Self-Bond (WSB)**
The worker locks collateral. If FAIL/claim upheld, collateral funds remedy.

**Co-Bond / Underwriter Bond (UB)**
Third parties lock collateral to “vouch” for the worker/outcome and earn premium.

**Dispute Bond (DB)**
Optional bonds from claimant/respondent to deter spam disputes (returned to the winner).

### 7.3 Collateral locking via Hydra

Hydra ABP exposes:

* reserve bond
* release bond
* draw bond (payout on claim)
* slash bond (policy-defined; must be receipted)

All are HTTP authority calls, idempotent, with deterministic receipts.

---

## 8) Preventing cheating and collusion (mechanism design)

This is the hard part. The system must assume agents will game incentives.

### 8.1 Independence tiers are enforced, not vibes

Aegis verification tiers (O/1/2/3/4) become mandatory constraints in OutcomeContract:

* A worker cannot “self-verify” high-stakes work.
* Correlated checkers (Tier 1) can’t unlock large payouts or warranties.

### 8.2 Commit–reveal for predictor positions (optional)

To prevent copy-trading/coordination at submission time, predictors can:

* commit a hash of (prediction, stake, salt)
* reveal later
  This is optional for MVP but worth designing in.

### 8.3 Separation of roles at match time

For higher risk classes:

* the worker cannot be the adjudicator
* the underwriter cannot be the verifier
* the verifier set must include heterogeneity (Tier 2/3)

### 8.4 Exposure caps and circuit breakers

Aegis autonomy throttle + Hydra risk breakers combine:

* cap per worker
* cap per underwriter
* cap per WorkUnit category
* freeze new warranties if claim rate spikes
* degrade to pay-after-verify only if uncertainty rises

Everything is driven from receipts and published in `/stats`.

---

## 9) Belief Markets: agent forecasts + aggregation + trading (optional, but aligned)

Belief Markets reuse the same contract primitives with a different “WorkUnit”:

* event definition
* resolution authority and evidence rules
* scoring window

### 9.1 Agent forecast submission (non-authoritative)

Agents submit forecasts (probabilities + evidence pointers). These are not money mutations.

### 9.2 Aggregation (reputation-weighted)

An aggregator computes a consensus probability using:

* past calibration scores
* topic specialization
* independence weighting

### 9.3 Trading / market making (optional)

If you run internal markets:

* use LMSR or a CLOB
* collateral and settlement remain Hydra-controlled
* resolution is Aegis-controlled

External bridges can mirror prices or positions later, but are not required.

---

## 10) Receipts (normative types)

Outcomes Markets must be receipted end-to-end. Minimum new receipt classes:

### Aegis receipts

* `aegis.outcomes.contract_create_receipt.v1`
* `aegis.outcomes.verification_plan_receipt.v1`
* `aegis.outcomes.verdict_receipt.v1`
* `aegis.outcomes.warranty_issue_receipt.v1`
* `aegis.outcomes.claim_open_receipt.v1`
* `aegis.outcomes.claim_resolution_receipt.v1`
* `aegis.outcomes.adjudication_receipt.v1`

### Hydra receipts (ABP + settlement)

* `hydra.abp.bond_reserve_receipt.v1`
* `hydra.abp.bond_release_receipt.v1`
* `hydra.abp.bond_draw_receipt.v1`
* `hydra.invoice_pay_receipt` (existing)
* `hydra.cep.envelope_*` (existing)

Every receipt must link:

* `work_unit_id`, `contract_id`
* `session_id`, `trajectory_hash`, `job_hash`
* `policy_bundle_id`
* canonical hashes of artifacts/evidence bundles

---

## 11) API surface (conceptual; proto-first source of truth)

### Aegis Outcomes (authority mutations: HTTP only)

* `POST /v1/aegis/outcomes/work_unit/create`
* `POST /v1/aegis/outcomes/contract/create`
* `POST /v1/aegis/outcomes/warranty/quote`
* `POST /v1/aegis/outcomes/warranty/issue`
* `POST /v1/aegis/outcomes/verify/start`
* `POST /v1/aegis/outcomes/verdict/finalize`
* `POST /v1/aegis/outcomes/claim/open`
* `POST /v1/aegis/outcomes/claim/resolve`
* `GET  /v1/aegis/outcomes/status/:work_unit_id`

### Hydra ABP (authority mutations: HTTP only)

* `POST /v1/hydra/abp/bond/reserve`
* `POST /v1/hydra/abp/bond/release`
* `POST /v1/hydra/abp/bond/draw`

Hydra CEP and LLP remain as-is and are invoked by TreasuryRouter per policy.

---

## 12) `/stats` additions (minute cache; table-first; public)

Add a new section (or table block) for Outcomes Markets. Keep it within the “~50 metrics at a glance” rule you already set.

Suggested top-line rows:

**Verified economy**

* Work units created / verified / failed (5m/1h/24h)
* Verified share by tier (O/1/2/3/4)
* Verification latency p50/p95
* Pay-after-verify utilization rate

**Liability**

* Warranties issued (count, sats coverage)
* Bonded exposure (worker vs underwriter)
* Claims opened / resolved (24h)
* Claims paid (sats) and loss rate (rolling)
* Dispute rate and adjudication latency

**Capital**

* ABP total collateral locked
* ABP utilization %
* CEP utilization for pay-after-verify flows
* Breaker states (warranty halted, dispute flood protection)

This makes solvency and trust legible to operators and to agents.

---

## 13) Launch strategy: OpenAgents-native first, external bridges optional

You asked: can we launch only on OpenAgents infra and let people provide liquidity later?

Yes—this spec is designed for that.

### Phase 0: Operator-funded, internal-only

* No external LPs
* No AMM trading
* Just: pay-after-verify + worker self-bond + claims
* This already creates a huge trust upgrade for your marketplace

### Phase 1: Add third-party underwriting (permissioned)

* Allow a small set of underwriter agents to co-bond
* Premium split rules + underwriting reputation
* Tight caps

### Phase 2: Open ABP deposits (LP mode)

* External LPs can deposit into ABP partitions with withdraw queues
* Share accounting + signed snapshots (align with Hydra’s LP roadmap)
* Still no requirement to connect to external markets

### Phase 3: Optional bridges (Polymarket etc.)

* Mirror select markets or probabilities outward
* Or route external liquidity inward for specific contracts
* Policy-gated and non-blocking (same rule as your Garden posture)

---

## 14) MVP definition (the version that proves it works)

If you want the sharpest MVP that demonstrates the whole thesis:

1. **Objective-verifiable WorkUnits** only (Tier O)
2. **Pay-after-verify** as default settlement (CEP-friendly)
3. **Worker self-bond** required above a size threshold
4. **Claims window** + simple adjudication only when verification is contested
5. Public `/stats` showing verified share, bonded exposure, claim rate, loss rate

This yields:

* buyers trust outcomes
* workers compete on “skin in the game”
* the network starts building reputation grounded in receipts
* Hydra/Aegis flywheels actually compound

---

## 15) How this connects back to Hydra + Aegis “north star”

* **Hydra** makes capital programmable: escrow, bonds, payouts, envelopes, routing, receipts.
* **Aegis** makes trust programmable: verification tiers, adjudication, warranties, claims, autonomy throttle, receipts.
* **Outcomes Markets** makes *risk* programmable: agents can **price and trade liability**, and buyers can purchase **warranted outcomes**.

This is the clean “Outcomes Protocol” translation into your architecture:

* not a separate chain,
* not a separate token,
* not a separate app,
  but a new contract primitive your agents and marketplace can program against.

---

Below is a concrete proto set you can drop into your repo. I wrote it in a way that:

* preserves your **HTTP-only authority** rule (all mutating RPCs have HTTP annotations),
* keeps **deterministic, idempotent receipts** first-class (every authority mutation returns a receipt),
* cleanly composes **Aegis (verification/liability)** with **Hydra (escrow/bonds/settlement)**,
* supports **Work Outcome Markets** (bet-on-your-own-output + warranties + claims),
* and includes an optional **Belief Market** (LMSR-style AMM) without requiring Polymarket.

> If you want these endpoints to stay internal-only initially, just change the HTTP paths from `/v1/...` → `/internal/v1/...` in the annotations.

---

## 1) `proto/openagents/common/v1/common.proto`

```proto
syntax = "proto3";

package openagents.common.v1;

import "google/protobuf/struct.proto";

option java_multiple_files = true;

// NOTE: We intentionally use explicit ms epoch fields and string digests
// (e.g. "sha256:<hex>") because you already standardize on deterministic
// receipts and canonical hashing across services.

enum Asset {
  ASSET_UNSPECIFIED = 0;
  BTC_LN = 1;        // Lightning BTC
  BTC_ONCHAIN = 2;   // On-chain BTC
  USD = 3;
  USDC = 4;
}

message Money {
  Asset asset = 1;

  // Use exactly one. (msats preferred for LN; sats acceptable for on-chain)
  oneof amount {
    uint64 amount_msats = 2;
    uint64 amount_sats = 3;
  }
}

message TraceContext {
  // Session grouping across the system (Autopilot runs, marketplace flows).
  string session_id = 1;

  // Deterministic identifiers you already use across receipts.
  string trajectory_hash = 2;
  string job_hash = 3;

  // Optional: run/job ids if you have stable ids distinct from hashes.
  string run_id = 4;
  string work_unit_id = 5;
  string contract_id = 6;
}

message PolicyContext {
  // Required for all authority mutations.
  string policy_bundle_id = 1;

  // Optional but useful for replay/debug (commit hash, semver, etc).
  string policy_version = 2;

  // Who/what approved (npub, org id, service id).
  string approved_by = 3;
}

message ArtifactRef {
  // e.g. "s3://...", "ipfs://...", "file://...", "oa://replay/..."
  string uri = 1;

  // e.g. "sha256:<hex>"
  string digest = 2;

  string mime_type = 3;
  uint64 size_bytes = 4;

  // Free-form metadata, but must not be relied on for determinism.
  google.protobuf.Struct meta = 10;
}

message EvidenceRef {
  string kind = 1;   // e.g. "replay_bundle", "test_report", "adjudication_note"
  string uri = 2;    // pointer to evidence bundle
  string digest = 3; // "sha256:<hex>"

  google.protobuf.Struct meta = 10;
}

message ReceiptRef {
  string receipt_id = 1;
  string receipt_type = 2;   // stable type string
  string canonical_hash = 3; // "sha256:<hex>"
}

message Receipt {
  string receipt_id = 1;
  string receipt_type = 2;
  int64 created_at_ms = 3;

  // Deterministic hash computed from a canonical projection of this receipt.
  string canonical_hash = 4; // "sha256:<hex>"

  // Idempotency for safe retries across HTTP.
  string idempotency_key = 5;

  TraceContext trace = 10;
  PolicyContext policy = 11;

  // Optional: for auditability.
  string inputs_hash = 20;  // "sha256:<hex>" over canonical inputs
  string outputs_hash = 21; // "sha256:<hex>" over canonical outputs

  repeated EvidenceRef evidence = 30;

  // Optional tags for indexing. Must not affect canonical_hash.
  map<string, string> tags = 40;
}
```

---

## 2) `proto/openagents/hydra/v1/abp_bonds.proto`

This is Hydra’s **Aegis Bond Partition (ABP)** API: reserve/release/draw collateral, with receipts.

```proto
syntax = "proto3";

package openagents.hydra.v1;

import "google/api/annotations.proto";
import "openagents/common/v1/common.proto";

option java_multiple_files = true;

enum BondReason {
  BOND_REASON_UNSPECIFIED = 0;

  // Work Outcome Markets
  WORKER_SELF_BOND = 1;
  UNDERWRITER_BOND = 2;

  // Claims/Disputes
  DISPUTE_BOND = 3;

  // Warranty collateral reserve (if you model that explicitly).
  WARRANTY_RESERVE = 4;
}

enum BondStatus {
  BOND_STATUS_UNSPECIFIED = 0;
  RESERVED = 1;
  RELEASED = 2;
  DRAWN = 3;
}

enum BondPartyRole {
  BOND_PARTY_ROLE_UNSPECIFIED = 0;
  PARTY_WORKER = 1;
  PARTY_UNDERWRITER = 2;
  PARTY_BUYER = 3;
  PARTY_CLAIMANT = 4;
  PARTY_RESPONDENT = 5;
  PARTY_SYSTEM = 6;
}

message Bond {
  string bond_id = 1;

  openagents.common.v1.Money amount_reserved = 2;
  openagents.common.v1.Money amount_available = 3; // reserved minus drawn

  BondReason reason = 4;
  BondPartyRole party_role = 5;

  // Owner of the bond funds (npub / agent id / treasury id).
  string owner_id = 6;

  // Linkage for audit / underwriting.
  string related_work_unit_id = 10;
  string related_contract_id = 11;
  string related_claim_id = 12;

  BondStatus status = 20;

  int64 created_at_ms = 30;
  int64 updated_at_ms = 31;
}

message ReserveBondRequest {
  // Required.
  openagents.common.v1.Money amount = 1;
  BondReason reason = 2;
  BondPartyRole party_role = 3;

  string owner_id = 4; // npub / agent id / treasury id

  // Optional linkage.
  string related_work_unit_id = 10;
  string related_contract_id = 11;

  string memo = 12;

  // Required invariants.
  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message ReserveBondResponse {
  Bond bond = 1;

  // The authoritative Hydra receipt for the reserve action.
  openagents.common.v1.Receipt receipt = 2;
}

message ReleaseBondRequest {
  string bond_id = 1;

  // Optional: partial release if you want it. If unset, release all remaining.
  openagents.common.v1.Money amount = 2;

  string memo = 3;

  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message ReleaseBondResponse {
  Bond bond = 1;
  openagents.common.v1.Receipt receipt = 2;
}

message DrawDestination {
  oneof destination {
    string bolt11_invoice = 1;    // LN payout
    string onchain_address = 2;   // on-chain payout
    string internal_account = 3;  // optional internal ledger account id
  }
}

message DrawBondRequest {
  string bond_id = 1;

  // Amount to draw (must be <= amount_available).
  openagents.common.v1.Money amount = 2;

  DrawDestination destination = 3;

  // Optional linkage.
  string related_claim_id = 10;
  string memo = 11;

  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message DrawBondResponse {
  Bond bond = 1;

  // Receipt for bond draw (ABP accounting + authorization).
  openagents.common.v1.Receipt receipt = 2;

  // Optional: if draw triggers an actual rail settlement (LN pay / on-chain tx),
  // return a reference to the settlement receipt (LLP pay receipt / on-chain tx receipt).
  openagents.common.v1.ReceiptRef settlement_receipt_ref = 3;
}

message GetBondRequest {
  string bond_id = 1;
}

message GetBondResponse {
  Bond bond = 1;
}

service AbpBondService {
  rpc ReserveBond(ReserveBondRequest) returns (ReserveBondResponse) {
    option (google.api.http) = {
      post: "/v1/hydra/abp/bonds/reserve"
      body: "*"
    };
  }

  rpc ReleaseBond(ReleaseBondRequest) returns (ReleaseBondResponse) {
    option (google.api.http) = {
      post: "/v1/hydra/abp/bonds/release"
      body: "*"
    };
  }

  rpc DrawBond(DrawBondRequest) returns (DrawBondResponse) {
    option (google.api.http) = {
      post: "/v1/hydra/abp/bonds/draw"
      body: "*"
    };
  }

  rpc GetBond(GetBondRequest) returns (GetBondResponse) {
    option (google.api.http) = {
      get: "/v1/hydra/abp/bonds/{bond_id}"
    };
  }
}
```

---

## 3) `proto/openagents/aegis/outcomes/v1/outcomes_work.proto`

This is the **Work Outcome Market** API (jobs with outcome contracts, verification plans, verdicts, warranties, claims).

```proto
syntax = "proto3";

package openagents.aegis.outcomes.v1;

import "google/api/annotations.proto";
import "openagents/common/v1/common.proto";
import "openagents/hydra/v1/abp_bonds.proto";

option java_multiple_files = true;

enum WorkUnitKind {
  WORK_UNIT_KIND_UNSPECIFIED = 0;

  // Marketplace / compute.
  COMPUTE_JOB = 1;

  // Skill invocations (NIP-SKL like).
  SKILL_INVOCATION = 2;

  // Autopilot artifacts (PRs, patch bundles, build outputs).
  ARTIFACT_BUNDLE = 3;

  // L402 call series.
  L402_SERIES = 4;
}

enum VerifiabilityClass {
  VERIFIABILITY_CLASS_UNSPECIFIED = 0;
  OBJECTIVE = 1;      // deterministic harness exists
  SUBJECTIVE = 2;     // adjudication/redundancy needed
  LOW_VERIFIABILITY = 3; // long latency / tacit; requires higher friction
}

enum VerificationTier {
  VERIFICATION_TIER_UNSPECIFIED = 0;

  // Tier O: objective harness (tests/hashes/invariants).
  TIER_O_OBJECTIVE = 1;

  // Tier 1: correlated checker (same family/provider).
  TIER_1_CORRELATED = 2;

  // Tier 2: heterogeneous checker (different family/provider).
  TIER_2_HETEROGENEOUS = 3;

  // Tier 3: redundancy + adjudication policy.
  TIER_3_ADJUDICATION = 4;

  // Tier 4: human underwriting/sampling.
  TIER_4_HUMAN = 5;
}

enum BinaryOutcome {
  BINARY_OUTCOME_UNSPECIFIED = 0;
  PASS = 1;
  FAIL = 2;
}

enum ContractState {
  CONTRACT_STATE_UNSPECIFIED = 0;
  CREATED = 1;
  FUNDED = 2;
  BONDED = 3;
  SUBMITTED = 4;
  VERIFYING = 5;
  VERDICT_PASS = 6;
  VERDICT_FAIL = 7;
  SETTLED = 8;
  WARRANTY_ACTIVE = 9;
  CLAIM_OPEN = 10;
  FINALIZED = 11;
  CANCELLED = 12;
}

enum RemedyType {
  REMEDY_TYPE_UNSPECIFIED = 0;
  REFUND = 1;
  REWORK_CREDIT = 2;
  DAMAGES = 3;
}

enum ClaimState {
  CLAIM_STATE_UNSPECIFIED = 0;
  OPEN = 1;
  UNDER_REVIEW = 2;
  APPROVED = 3;
  DENIED = 4;
  PARTIALLY_APPROVED = 5;
  PAID = 6;
  CLOSED = 7;
}

message AcceptanceCriteria {
  // Human-readable spec pointer (not normative).
  string description = 1;

  // Optional deterministic anchors.
  repeated openagents.common.v1.ArtifactRef required_artifacts = 2;
  repeated openagents.common.v1.EvidenceRef required_evidence = 3;

  // Optional: objective harness reference.
  openagents.common.v1.ArtifactRef harness_ref = 10;
}

message WorkUnit {
  string work_unit_id = 1;
  WorkUnitKind kind = 2;

  string title = 3;
  string description = 4;

  // Who created it (buyer / operator).
  string creator_id = 5; // npub / agent id / org id

  AcceptanceCriteria acceptance = 10;

  // Linkage.
  openagents.common.v1.TraceContext trace = 20;
  openagents.common.v1.PolicyContext policy = 21;

  int64 created_at_ms = 30;
}

message BondRequirement {
  openagents.hydra.v1.BondPartyRole party_role = 1;
  openagents.hydra.v1.BondReason reason = 2;

  // Minimum bond required.
  openagents.common.v1.Money min_amount = 3;

  // Optional: cap for this role.
  openagents.common.v1.Money max_amount = 4;
}

message WarrantyTerms {
  RemedyType remedy = 1;

  // Coverage cap on remedy payouts (msats/sats in Money).
  openagents.common.v1.Money coverage_cap = 2;

  // Warranty window after settlement.
  uint64 warranty_window_ms = 3;

  // Optional exclusions / constraints.
  repeated string exclusions = 10;
}

message VerificationPlan {
  VerifiabilityClass verifiability = 1;

  // Minimum required tier to settle/warrant.
  VerificationTier required_tier = 2;

  // Suggested execution plan (not authoritative).
  repeated openagents.common.v1.EvidenceRef planned_checks = 10;

  // If subjective: adjudication policy pointer.
  openagents.common.v1.EvidenceRef adjudication_policy_ref = 11;
}

message OutcomeContract {
  string contract_id = 1;
  string work_unit_id = 2;

  // Parties.
  string buyer_id = 10;
  string worker_id = 11;       // provider agent id (npub)
  string underwriter_group_id = 12; // optional cohort identifier

  // Funding (buyer escrow) and premiums.
  openagents.common.v1.Money price = 20;
  openagents.common.v1.Money warranty_premium = 21; // 0 if no warranty

  // Verification + bonds.
  VerificationPlan verification_plan = 30;
  repeated BondRequirement bond_requirements = 31;

  // Optional warranty.
  bool warranty_enabled = 40;
  WarrantyTerms warranty_terms = 41;

  // State.
  ContractState state = 50;

  // Linkage.
  openagents.common.v1.TraceContext trace = 60;
  openagents.common.v1.PolicyContext policy = 61;

  int64 created_at_ms = 70;
  int64 updated_at_ms = 71;
}

message BondLink {
  // Hydra ABP bond id reserved under Hydra.
  string bond_id = 1;

  openagents.hydra.v1.BondPartyRole party_role = 2;
  openagents.hydra.v1.BondReason reason = 3;

  openagents.common.v1.Money amount_reserved = 4;
}

message Submission {
  // Output artifacts for verification.
  repeated openagents.common.v1.ArtifactRef outputs = 1;

  // Evidence bundles (logs, test reports, replay, etc).
  repeated openagents.common.v1.EvidenceRef evidence = 2;

  int64 submitted_at_ms = 10;
}

message Verdict {
  BinaryOutcome outcome = 1;

  // Tier achieved (must satisfy plan.required_tier for settlement/warranty).
  VerificationTier achieved_tier = 2;

  // Evidence pointers (harness output, adjudication notes).
  repeated openagents.common.v1.EvidenceRef evidence = 3;

  // Optional: references to Hydra receipts produced as a result of verdict finalization
  // (e.g., CEP settle, invoice release, refunds, bond release).
  repeated openagents.common.v1.ReceiptRef settlement_receipts = 10;

  int64 decided_at_ms = 20;
}

message Warranty {
  bool active = 1;
  WarrantyTerms terms = 2;
  int64 starts_at_ms = 3;
  int64 ends_at_ms = 4;
}

message Claim {
  string claim_id = 1;
  string contract_id = 2;

  string claimant_id = 10; // npub/agent id
  string reason = 11;

  repeated openagents.common.v1.EvidenceRef evidence = 12;

  ClaimState state = 20;

  // Optional: claim requires dispute bond (anti-spam).
  BondLink dispute_bond = 30;

  int64 opened_at_ms = 40;
  int64 updated_at_ms = 41;
}

message ClaimResolution {
  ClaimState final_state = 1;

  // Payouts (if any). Note: actual money movement is captured via settlement_receipts.
  openagents.common.v1.Money payout = 2;

  // Which bonds were drawn (if any).
  repeated BondLink bonds_drawn = 3;

  repeated openagents.common.v1.EvidenceRef evidence = 10;

  repeated openagents.common.v1.ReceiptRef settlement_receipts = 20;

  int64 resolved_at_ms = 30;
}

message CreateWorkUnitRequest {
  WorkUnitKind kind = 1;
  string title = 2;
  string description = 3;

  string creator_id = 4;

  AcceptanceCriteria acceptance = 10;

  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message CreateWorkUnitResponse {
  WorkUnit work_unit = 1;
  openagents.common.v1.Receipt receipt = 2; // aegis.outcomes.work_unit_create_receipt.v1
}

message GetWorkUnitRequest { string work_unit_id = 1; }
message GetWorkUnitResponse { WorkUnit work_unit = 1; }

message CreateContractRequest {
  string work_unit_id = 1;

  string buyer_id = 10;
  string worker_id = 11;

  openagents.common.v1.Money price = 20;

  // Proposed verification plan (Aegis may modify based on policy).
  VerificationPlan verification_plan = 30;

  // Bond requirements (Aegis may enforce minimums).
  repeated BondRequirement bond_requirements = 31;

  // Optional warranty.
  bool warranty_enabled = 40;
  WarrantyTerms warranty_terms = 41;
  openagents.common.v1.Money warranty_premium = 42;

  string idempotency_key = 50;
  openagents.common.v1.TraceContext trace = 51;
  openagents.common.v1.PolicyContext policy = 52;
}

message CreateContractResponse {
  OutcomeContract contract = 1;
  openagents.common.v1.Receipt receipt = 2; // aegis.outcomes.contract_create_receipt.v1
}

message GetContractRequest { string contract_id = 1; }
message GetContractResponse { OutcomeContract contract = 1; }

message AttachBondRequest {
  string contract_id = 1;

  // Bond reserved via Hydra ABP.
  string bond_id = 2;

  openagents.hydra.v1.BondPartyRole party_role = 3;
  openagents.hydra.v1.BondReason reason = 4;

  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message AttachBondResponse {
  repeated BondLink bonds = 1;
  openagents.common.v1.Receipt receipt = 2; // aegis.outcomes.attach_bond_receipt.v1
}

message SubmitOutputRequest {
  string contract_id = 1;

  Submission submission = 2;

  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message SubmitOutputResponse {
  OutcomeContract contract = 1;
  openagents.common.v1.Receipt receipt = 2; // aegis.outcomes.submission_receipt.v1
}

message StartVerificationRequest {
  string contract_id = 1;

  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message StartVerificationResponse {
  OutcomeContract contract = 1;
  openagents.common.v1.Receipt receipt = 2; // aegis.outcomes.verify_start_receipt.v1
}

message FinalizeVerdictRequest {
  string contract_id = 1;

  Verdict verdict = 2;

  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message FinalizeVerdictResponse {
  OutcomeContract contract = 1;
  Verdict verdict = 2;

  // Receipt for finalization (ties verification -> settlement links).
  openagents.common.v1.Receipt receipt = 3; // aegis.outcomes.verdict_finalize_receipt.v1
}

message OpenClaimRequest {
  string contract_id = 1;

  string claimant_id = 10;
  string reason = 11;
  repeated openagents.common.v1.EvidenceRef evidence = 12;

  // Optional: attach dispute bond (if policy requires it).
  string dispute_bond_id = 20; // Hydra ABP bond id

  string idempotency_key = 30;
  openagents.common.v1.TraceContext trace = 31;
  openagents.common.v1.PolicyContext policy = 32;
}

message OpenClaimResponse {
  Claim claim = 1;
  openagents.common.v1.Receipt receipt = 2; // aegis.outcomes.claim_open_receipt.v1
}

message ResolveClaimRequest {
  string claim_id = 1;

  // APPROVED / DENIED / PARTIALLY_APPROVED.
  ClaimState final_state = 2;

  // If approved: payout and bond draws will be executed (Hydra receipts referenced).
  openagents.common.v1.Money payout = 3;

  repeated openagents.common.v1.EvidenceRef evidence = 10;

  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message ResolveClaimResponse {
  Claim claim = 1;
  ClaimResolution resolution = 2;
  openagents.common.v1.Receipt receipt = 3; // aegis.outcomes.claim_resolution_receipt.v1
}

message GetStatusRequest { string contract_id = 1; }

message ContractStatus {
  OutcomeContract contract = 1;
  repeated BondLink bonds = 2;

  Submission last_submission = 3;

  bool has_verdict = 10;
  Verdict verdict = 11;

  bool warranty_active = 20;
  Warranty warranty = 21;

  repeated Claim claims = 30;
}

message GetStatusResponse { ContractStatus status = 1; }

service OutcomesWorkService {
  rpc CreateWorkUnit(CreateWorkUnitRequest) returns (CreateWorkUnitResponse) {
    option (google.api.http) = { post: "/v1/aegis/outcomes/work_units" body: "*" };
  }

  rpc GetWorkUnit(GetWorkUnitRequest) returns (GetWorkUnitResponse) {
    option (google.api.http) = { get: "/v1/aegis/outcomes/work_units/{work_unit_id}" };
  }

  rpc CreateContract(CreateContractRequest) returns (CreateContractResponse) {
    option (google.api.http) = { post: "/v1/aegis/outcomes/contracts" body: "*" };
  }

  rpc GetContract(GetContractRequest) returns (GetContractResponse) {
    option (google.api.http) = { get: "/v1/aegis/outcomes/contracts/{contract_id}" };
  }

  rpc AttachBond(AttachBondRequest) returns (AttachBondResponse) {
    option (google.api.http) = { post: "/v1/aegis/outcomes/contracts/{contract_id}/bonds/attach" body: "*" };
  }

  rpc SubmitOutput(SubmitOutputRequest) returns (SubmitOutputResponse) {
    option (google.api.http) = { post: "/v1/aegis/outcomes/contracts/{contract_id}/submit" body: "*" };
  }

  rpc StartVerification(StartVerificationRequest) returns (StartVerificationResponse) {
    option (google.api.http) = { post: "/v1/aegis/outcomes/contracts/{contract_id}/verify/start" body: "*" };
  }

  rpc FinalizeVerdict(FinalizeVerdictRequest) returns (FinalizeVerdictResponse) {
    option (google.api.http) = { post: "/v1/aegis/outcomes/contracts/{contract_id}/verdict/finalize" body: "*" };
  }

  rpc OpenClaim(OpenClaimRequest) returns (OpenClaimResponse) {
    option (google.api.http) = { post: "/v1/aegis/outcomes/contracts/{contract_id}/claims" body: "*" };
  }

  rpc ResolveClaim(ResolveClaimRequest) returns (ResolveClaimResponse) {
    option (google.api.http) = { post: "/v1/aegis/outcomes/claims/{claim_id}/resolve" body: "*" };
  }

  rpc GetStatus(GetStatusRequest) returns (GetStatusResponse) {
    option (google.api.http) = { get: "/v1/aegis/outcomes/contracts/{contract_id}/status" };
  }
}
```

---

## 4) `proto/openagents/aegis/outcomes/v1/belief_markets.proto` (optional, internal-first LMSR)

This is the minimal Belief Market (internal) that can later bridge outward. It’s intentionally LMSR-first (always-liquid), but still receipt-driven and HTTP-only for authority mutations.

```proto
syntax = "proto3";

package openagents.aegis.outcomes.v1;

import "google/api/annotations.proto";
import "openagents/common/v1/common.proto";

option java_multiple_files = true;

enum MarketOutcome {
  MARKET_OUTCOME_UNSPECIFIED = 0;
  YES = 1;
  NO = 2;
}

enum MarketState {
  MARKET_STATE_UNSPECIFIED = 0;
  OPEN = 1;
  CLOSED = 2;
  RESOLVED = 3;
  CANCELLED = 4;
}

enum ResolutionMethod {
  RESOLUTION_METHOD_UNSPECIFIED = 0;

  // Resolution by objective oracle (API, deterministic data source).
  OBJECTIVE_ORACLE = 1;

  // Resolution by adjudication policy / committee / human lane.
  ADJUDICATION = 2;
}

message LmsrParams {
  // LMSR liquidity parameter b, denominated in msats (or sats) for bounded loss.
  // Higher b => deeper liquidity, less price movement per share.
  openagents.common.v1.Money b = 1;
}

message BeliefMarket {
  string market_id = 1;

  string question = 2;
  string category = 3; // e.g. "crypto", "politics", "internal_ops"

  ResolutionMethod resolution_method = 10;

  // For objective oracle: evidence ref describes source + query.
  openagents.common.v1.EvidenceRef resolution_source_ref = 11;

  // For adjudication: pointer to policy.
  openagents.common.v1.EvidenceRef adjudication_policy_ref = 12;

  int64 resolve_by_ms = 20;

  // Market maker parameters (LMSR MVP).
  LmsrParams lmsr = 30;

  MarketState state = 40;

  openagents.common.v1.TraceContext trace = 50;
  openagents.common.v1.PolicyContext policy = 51;

  int64 created_at_ms = 60;
  int64 updated_at_ms = 61;
}

message BeliefSubmission {
  string market_id = 1;

  // Who is forecasting.
  string forecaster_id = 2; // npub/agent id

  // Probability that YES resolves.
  double prob_yes = 3; // [0,1]

  // Optional: confidence interval width (bps) or other calibration info.
  uint32 ci_width_bps = 4;

  repeated openagents.common.v1.EvidenceRef evidence = 10;

  int64 submitted_at_ms = 20;
}

message AggregatedBelief {
  string market_id = 1;

  double prob_yes = 2;

  // Optional: diagnostics for the ensemble.
  uint32 contributing_forecasters = 10;
  double entropy = 11;

  int64 computed_at_ms = 20;
}

message CreateMarketRequest {
  string question = 1;
  string category = 2;

  ResolutionMethod resolution_method = 10;
  openagents.common.v1.EvidenceRef resolution_source_ref = 11;
  openagents.common.v1.EvidenceRef adjudication_policy_ref = 12;

  int64 resolve_by_ms = 20;

  LmsrParams lmsr = 30;

  string idempotency_key = 40;
  openagents.common.v1.TraceContext trace = 41;
  openagents.common.v1.PolicyContext policy = 42;
}

message CreateMarketResponse {
  BeliefMarket market = 1;
  openagents.common.v1.Receipt receipt = 2; // aegis.beliefs.market_create_receipt.v1
}

message GetMarketRequest { string market_id = 1; }
message GetMarketResponse { BeliefMarket market = 1; }

message SubmitBeliefRequest {
  BeliefSubmission submission = 1;

  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message SubmitBeliefResponse {
  openagents.common.v1.Receipt receipt = 1; // aegis.beliefs.submission_receipt.v1
}

message GetAggregateRequest { string market_id = 1; }
message GetAggregateResponse { AggregatedBelief aggregate = 1; }

// --- LMSR trading (authority mutation, receipt-driven)
// Shares are fixed-point (1e8). 1 share pays 1 unit on resolution.
message Shares {
  uint64 shares_e8 = 1; // shares * 1e8
}

message TradeQuoteRequest {
  string market_id = 1;
  MarketOutcome outcome = 2;

  Shares buy_shares = 3; // buy shares of outcome

  // Optional: cap to refuse quote beyond.
  openagents.common.v1.Money max_cost = 10;

  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message TradeQuoteResponse {
  openagents.common.v1.Money cost = 1;
  double implied_prob_yes_after = 2;
  openagents.common.v1.Receipt receipt = 3; // aegis.beliefs.trade_quote_receipt.v1
}

message ExecuteTradeRequest {
  string market_id = 1;
  MarketOutcome outcome = 2;
  Shares buy_shares = 3;

  // Who pays for this trade.
  string trader_id = 10; // npub/agent id

  // Must be >= quoted cost; prevents slippage surprises.
  openagents.common.v1.Money max_cost = 11;

  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message ExecuteTradeResponse {
  // Receipt for trade execution (includes references to money movement receipts).
  openagents.common.v1.Receipt receipt = 1; // aegis.beliefs.trade_execute_receipt.v1

  // Optional: reference to Hydra receipt that funded the trade into market escrow.
  openagents.common.v1.ReceiptRef settlement_receipt_ref = 2;

  double implied_prob_yes_after = 3;
}

message ResolveMarketRequest {
  string market_id = 1;
  MarketOutcome resolved_outcome = 2;

  // Evidence backing resolution (oracle output, adjudication decision).
  repeated openagents.common.v1.EvidenceRef evidence = 10;

  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message ResolveMarketResponse {
  BeliefMarket market = 1;
  openagents.common.v1.Receipt receipt = 2; // aegis.beliefs.market_resolve_receipt.v1

  // Optional: if you implement automated payouts at resolution time, include refs.
  repeated openagents.common.v1.ReceiptRef payout_receipts = 3;
}

service BeliefMarketsService {
  rpc CreateMarket(CreateMarketRequest) returns (CreateMarketResponse) {
    option (google.api.http) = { post: "/v1/aegis/beliefs/markets" body: "*" };
  }

  rpc GetMarket(GetMarketRequest) returns (GetMarketResponse) {
    option (google.api.http) = { get: "/v1/aegis/beliefs/markets/{market_id}" };
  }

  rpc SubmitBelief(SubmitBeliefRequest) returns (SubmitBeliefResponse) {
    option (google.api.http) = { post: "/v1/aegis/beliefs/markets/{submission.market_id}/beliefs" body: "*" };
  }

  rpc GetAggregate(GetAggregateRequest) returns (GetAggregateResponse) {
    option (google.api.http) = { get: "/v1/aegis/beliefs/markets/{market_id}/aggregate" };
  }

  rpc TradeQuote(TradeQuoteRequest) returns (TradeQuoteResponse) {
    option (google.api.http) = { post: "/v1/aegis/beliefs/markets/{market_id}/trade/quote" body: "*" };
  }

  rpc ExecuteTrade(ExecuteTradeRequest) returns (ExecuteTradeResponse) {
    option (google.api.http) = { post: "/v1/aegis/beliefs/markets/{market_id}/trade/execute" body: "*" };
  }

  rpc ResolveMarket(ResolveMarketRequest) returns (ResolveMarketResponse) {
    option (google.api.http) = { post: "/v1/aegis/beliefs/markets/{market_id}/resolve" body: "*" };
  }
}
```

---

## Quick integration notes (so these compile + generate cleanly)

* Add these files to your buf/module include paths.
* Ensure you already vendored googleapis protos (for `google/api/annotations.proto`) or fetch via buf.
* You’ll likely want to map `openagents.common.v1.Receipt` into your existing receipt infra (if you already have a canonical receipt message). If so, replace my `Receipt` with your canonical type and keep the fields.
