# Economy Kernel vs arXiv:2602.20946v1 Gap Analysis

> Historical note: This audit is a point-in-time snapshot from its date. Current product and architecture authority lives in `README.md`, `docs/MVP.md`, `docs/OWNERSHIP.md`, and `docs/kernel/`. File paths, issue states, and implementation-status claims here may be superseded by later commits.


Date: 2026-03-05  
Author: Codex  
Status: Complete (docs + repo implementation review)

## Objective

Compare what OpenAgents has planned and implemented for the Economy Kernel against the Feb 25, 2026 arXiv:2602.20946v1 thesis and recommended infrastructure:

- Core macro-risk: execution scales faster than verification, so durable advantage shifts toward systems that can certify outcomes, insure/absorb liability, and make verification cheap via provenance.
- Missing machinery for an agent economy: verification + liability + provenance + measurement.
- Recommendations extend beyond a marketplace into: insurance boundary and risk-metered pricing, identity gates (including proof-of-personhood), standardized incident reporting + interoperable exports, rollback robustness, continuous drift detection, privacy-preserving safety signal sharing, and cross-border certification.

This audit treats the paper's ecosystem recommendations as "kernel-addressable primitives" even when governments/standards bodies ultimately implement the institutional layer.

## Sources Reviewed

Planned/spec authority:
- `docs/plans/economy-kernel.md`
- `docs/plans/economy-kernel-proto.md`
- `docs/MVP.md` (MVP scope constraints)

Implemented (MVP/Earn kernel subset):
- `apps/autopilot-desktop/src/economy_kernel_receipts.rs` (receipt envelope + canonical hashing)
- `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs` (Earn receipt stream, WorkUnit metadata mapping, idempotency, receipt export)
- `apps/autopilot-desktop/src/state/economy_snapshot.rs` (minute snapshot object derived from receipts)

Recent GitHub issues (via `gh issue list` on 2026-03-05):
- No open issues in the repo at time of check.
- Recently updated issues are all CLOSED; the most relevant cluster is the Earn Kernel compliance program (#2945 and #2946-#2954).

## Current Alignment (What Already Matches The Paper's Core Thesis)

The kernel plan matches the paper's core technical prescription:

- Control variable framing: maximize verified output (kernel: `sv`, `NV = rho * N`) and gate autonomy on measured trust capacity (`sv`, correlation risk, `XA_hat`, `delta_m_hat`). (`docs/plans/economy-kernel.md` sections 2.4, 6.8, 7)
- Explicit pipeline: work definition -> verification -> liability assignment -> settlement, with receipts as canonical truth and deterministic replay safety. (`docs/plans/economy-kernel.md` sections 1-5)
- Correlation-aware verification: verification tiering and independence metadata are first-class, not an afterthought. (`docs/plans/economy-kernel.md` sections 2.3, 4.5; `apps/autopilot-desktop/src/economy_kernel_receipts.rs`)
- Measurement substrate exists in MVP form: receipt stream + deterministic minute snapshots with `sv` breakdown and correlation/provenance headlines. (`apps/autopilot-desktop/src/state/economy_snapshot.rs`, snapshot receipts in `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs`)

The remaining gaps are primarily the paper's "policy/ecosystem" layer: explicit insurance-boundary primitives (risk charges), identity/personhood gates, standardized incident reporting and export formats, rollback planning/receipts, continuous drift detection, privacy-preserving safety signals, and certification constructs.

## Crosswalk: Paper Recommendations -> Kernel -> Final Gaps

### A) Gate deployment by verifiable share; price liability; drift detection

Paper: bind scaled deployment to verified share (sv-weighted scale), price products by metered risk/liability assumed, and make continuous drift detection a first-class control.

Kernel today: `sv`, `XA_hat`, `delta_m_hat` + deterministic throttles, verification tiers, receipts, and minute snapshots exist in spec; the Earn kernel implements the receipt/snapshot substrate.

Final gaps: risk-metered liability pricing and continuous drift detection are not explicit kernel objects/receipt types; `delta_m_hat`/`XA_hat` are present but not yet computed in the Earn snapshot implementation.

### B) Cryptographic provenance expands easy verification; include verifiable inference + identity gates

Paper: provenance is process verification (model/version, tools, data sources/permissioning, verifiable inference, signatures/attestations); identity gates (including proof-of-personhood) are a primary defense against synthetic participation and a key economic filter for verifiers/underwriters.

Kernel today: provenance bundles/grades and correlation constraints are specified; implementation has a receipt envelope but does not implement typed provenance bundles or identity assurance levels.

Final gaps: policy cannot require named "verifiable inference / execution attestation" classes, and receipts/policy do not carry an explicit assurance level (personhood, org-KYC, hardware-bound, etc).

### C) Standardized incident reporting + interoperable exports are prerequisites for insurability

Paper: a liability regime needs standardized incident reporting and auditable traces; interoperability of incident taxonomies and audit formats is a public measurement prerequisite.

Kernel today: GroundTruthCase and incident/near-miss concepts exist normatively; Earn implements receipt bundles and receipt export, but incidents are not first-class objects.

Final gaps: incident reporting is not proto-first/standardized (taxonomy + linkage + export); there is no deterministic audit-package export that covers receipts + incidents + certifications with redaction modes.

### D) Robust rollback mechanisms

Paper: robust rollback mechanisms reduce oversight burden and correlated failure risk.

Kernel today: rollbacks appear as adverse signals (for `XA_hat`) and compensating actions exist in routing semantics.

Final gaps: rollback is currently observed, not required/planned/receipted as a first-class contract capability.

### E) Synthetic practice as a public good (portable benchmarks and scenario exports)

Paper: fund synthetic practice environments and open verification benchmarks; enforce audit format interoperability.

Kernel today: GroundTruthCase -> SimulationScenario pipeline is a first-class kernel concept.

Final gaps: there is no export surface for sanitized ground-truth cases/scenarios (privacy-preserving, policy-gated, stable hashes).

### F) Neutral rails + privacy-preserving safety signal sharing (optional anchoring)

Paper: permissionless rails and crypto primitives can support neutral coordination and privacy-preserving sharing of safety signals.

Kernel today: receipt hashing and snapshot hashing are present; public `/stats` is specified with redaction rules.

Final gaps: no explicit (optional) anchoring mechanism exists to publish snapshot hashes or receipt Merkle roots to a neutral public ledger without revealing private payloads.

## Gap Matrix (Paper + Final-pass Deltas vs Plan/Proto/Implementation)

Status legend:
- Spec: Economy Kernel normative spec coverage.
- Proto plan: `economy-kernel-proto.md` coverage (note: protos are currently a plan doc; there is no `proto/` tree in this repo).
- Impl: current repo implementation coverage (MVP/Earn kernel subset).

| Area (paper recommendations) | Spec | Proto plan | Impl | Notes |
|---|---:|---:|---:|---|
| 1) Insurability preconditions (incident reporting + disclosures + proof-of-coverage/capital hooks) | Partial | No | No | Spec has liability/bonds/warranties and GroundTruthCase, but not standardized IncidentReport/DisclosureBundle/ProofOfCoverage primitives. |
| 2) Risk-metered liability pricing (insurance boundary) | Partial | Partial | No | Proto plan has `warranty_premium`, but pricing is not explicitly split into execution price vs liability premium/risk charge, and settlement receipts do not carry risk charges. |
| 3) Continuous monitoring + drift detection (drift receipts + policy triggers) | Weak | No | No | Spec has drift indicators, but no first-class monitoring/drift module or drift receipt types. |
| 4) Standardized incident reporting + interoperable incident taxonomy + deterministic audit package export | Partial | No | No | GroundTruthCase exists in prose/spec, but there is no proto-first incident schema (taxonomy + linkage + export) covering incidents/near-misses. |
| 5) Interoperable audit export formats (receipts + incidents + certifications + outcomes) with redaction modes | Partial | No | Partial | Impl exports Earn receipt bundles, but there is no open, versioned cross-service export format or redaction tiers. |
| 6) Outcome registries as first-class objects | No | No | No | No `OutcomeRegistryEntry` primitive exists; GroundTruthCase is adjacent but not an outcome registry object. |
| 7) Privacy-preserving safety signals channel (public aggregate + restricted sharing) | Partial | No | No | Spec has public `/stats` redaction rules; no restricted feed/channel primitive. |
| 8) Identity assurance + proof-of-personhood gates (role-based) | Weak | No | Weak | Spec only requires "authenticated caller identity"; impl stores `approved_by` as a string and does not record assurance level or personhood proofs. |
| 9) Provenance upgrades (verifiable inference/exec attestation + explicit data source/permissioning refs) | Partial | Partial | Weak | Spec/proto plan include attestations and provenance grades, but not explicit permissioning/data-source refs or named policy requirement classes for verifiable inference/execution. |
| 10) Certification + safe harbor + digital border compatibility | No | No | No | Not present as kernel primitives (cert issuance/revocation receipts, policy gating). |
| 11) White-hat auditing + bounty workflows (kernel-native patterns) | No | No | No | Not defined as WorkUnit templates/flows; UI has unrelated "bounty badge" components but no economy-kernel semantics. |
| 12) Rollback robustness (rollback plans + rollback receipts) | Partial | No | No | Spec references rollbacks as signals and compensating actions in routing, but rollback is not a required/receipted contract capability. |
| 13) Exportable synthetic practice/benchmarks (portable scenarios + redaction receipts) | Partial | No | No | Spec mandates GroundTruthCase -> SimulationScenario, but does not define portable packages or a policy-gated export surface. |
| 14) Correlation-adjusted effective verifiable share | Partial | No | Partial | Spec emphasizes correlation; impl computes correlated share but does not compute/report a correlation-discounted `sv_effective`. |
| 15) Optional anchoring (publish snapshot hashes / receipt roots to a neutral public ledger) | No | No | No | Snapshot/receipt hashing exists, but there is no anchoring mechanism or anchoring receipt type. |

## Detailed Gaps (Kernel Primitives Needed To Fully Cover The Paper)

### 1) Insurance Boundary (Risk-Metered Pricing + Insurability Preconditions)

**What the paper wants:** scaled deployment gated by verified share, plus an insurance boundary where liability can be priced and absorbed (premiums, reserves/capital), grounded in standardized incident data and auditable traces.

**What we have:**
- Spec: warranties/claims/bonds exist as primitives; underwriters earn premiums and pay when claims hold; GroundTruthCase is the precedent/incident substrate. (`docs/plans/economy-kernel.md` sections 2.4, 6.10)
- Proto plan: contract includes `price` and `warranty_premium`, but does not explicitly separate execution price vs risk charge, and does not bind pricing to snapshot windows. (`docs/plans/economy-kernel-proto.md` OutcomeContract)
- Impl: Earn kernel receipts/snapshots exist, but do not implement liability premium/risk charge fields.

**Gaps to close (make these explicit, first-class):**
- Risk-metered liability pricing: contract and settlement receipts need explicit liability premium/risk charge fields (even when warranties are disabled).
- Insurability prerequisites: standardized IncidentReport/GroundTruthCase schema + versioned taxonomy; DisclosureBundle and ProofOfCoverage hooks; exportability/redaction rules so underwriters can price tail risk from portable artifacts (not private logs).

**Kernel-addressable primitives to add:**
- Contract/settlement: `liability_premium` / `risk_charge` fields, and a `pricing_snapshot_ref` (binds premium computation to a snapshot window/hash).
- Preconditions: `IncidentReport` (including near-miss), `IncidentTaxonomyCode`, `DisclosureBundleRef`, `ProofOfCoverageRef`.

### 2) Continuous Monitoring + Drift Detection (Receipted)

**What the paper wants:** continuous drift detection as a first-class control loop that can trigger the same deterministic throttle ladder used for `sv`/`XA_hat`.

**What we have:**
- Spec: `/stats` includes drift indicators (example: dispute trigger rate by tier) but does not define a drift module or drift receipt types. (`docs/plans/economy-kernel.md` section 7.2-F)
- Impl: no drift receipts or monitoring plans.

**Gaps:**
- No Monitoring/Drift module with deterministic drift receipts that can feed policy gates and breakers.

**Kernel-addressable primitives to add:**
- Drift receipts: `drift_signal_emitted`, `drift_alert_raised`, `drift_false_positive_confirmed` (linked into GroundTruthCase when confirmed).
- Policy knobs: thresholds on drift receipt rates that trigger the same deterministic throttle order defined for other breakers.

### 3) Identity Assurance + Proof-of-Personhood Gates

**What the paper wants:** identity gates as a core defense against synthetic participation and as a filter for high-trust roles (verifiers/underwriters) and high-risk categories.

**What we have:**
- Spec: requires authenticated caller identity, but does not define assurance levels or personhood proofs. (`docs/plans/economy-kernel.md` section 1.1)
- Impl: `PolicyContext.approved_by` is a string; no assurance level or credential linkage is recorded. (`apps/autopilot-desktop/src/economy_kernel_receipts.rs`)

**Gaps:**
- No `AuthAssuranceLevel` dimension in receipts/policy (anon vs authenticated vs org-KYC vs personhood vs hardware-bound).
- No role-based policy rules (e.g., "verdict finalization for HIGH severity requires personhood/org-KYC").

**Kernel-addressable primitives to add:**
- Receipt fields: `auth_assurance_level` and `CredentialRef`/proof references for required actions (WorkUnit creation, verdict finalization, underwriting/bond posting).
- Policy rules: assurance requirements by `category x tfb x severity` and by role.

### 4) Provenance: Make "Verifiable Inference" And Permissioning Explicit

**What the paper wants:** provenance as process verification, including verifiable inference/execution attestation and explicit data source/permissioning evidence.

**What we have:**
- Spec/proto plan: provenance bundles, attestations, tool/model lineage, and provenance grades exist; policy can require provenance bundles and attestations at high severity. (`docs/plans/economy-kernel.md` sections 5.4-5.5; `docs/plans/economy-kernel-proto.md` common + outcomes_work)
- Impl: evidence is generic `EvidenceRef` and does not enforce typed provenance bundles for Earn.

**Gaps:**
- "Verifiable inference / execution attestation" is not a named requirement class in policy; it cannot be required deterministically by slice today.
- Data source refs and permissioning refs are not explicit; they are forced into opaque metadata if captured at all.

**Kernel-addressable primitives to add:**
- ProvenanceBundle expansions: explicit `data_source_refs` and `permissioning_refs`.
- Policy knobs: named requirement classes for verifiable inference/execution attestation (model version and runtime integrity) by slice.

### 5) Standardized Incident Reporting + Deterministic Audit Packages

**What the paper wants:** standardized incident reporting + interoperable formats as prerequisites for insurability and public measurement infrastructure.

**What we have:**
- Spec: GroundTruthCase linkage requirements exist and are audit-grade in prose. (`docs/plans/economy-kernel.md` section 6.10)
- Impl: Earn kernel can export receipt bundles, but incidents/ground-truth cases are not modeled as first-class objects.

**Gaps:**
- GroundTruthCase/Incident reporting is not proto-first or versioned; taxonomy + linkage + export are underspecified.
- No deterministic "AuditPackage" export format that can include receipts + incidents + certifications with redaction tiers.

**Kernel-addressable primitives to add:**
- Incident proto/schema: `IncidentReport`, `GroundTruthCase` (specialized incident), versioned taxonomy codes, mandatory receipt/evidence linkage.
- Export: a deterministic AuditPackage format with redaction modes (public vs restricted), stable hashes, and required linkage invariants.

### 6) Outcome Registries

**What the paper wants:** outcome registries as public measurement infrastructure and ground truth substrate (avoid proprietary lock-in).

**What we have:** no outcome registry object.

**Kernel-addressable primitives to add:**
- `OutcomeRegistryEntry` object linked to WorkUnit/contract/verdict/claim/remedy receipts with a public-aggregation shape and restricted-detail shape.

### 7) Rollback Robustness (Plans + Receipts)

**What the paper wants:** robust rollback mechanisms as part of safe harbors and lower-oversight regimes.

**What we have:**
- Spec: rollbacks/reverts are adverse signals and compensating actions exist in routing semantics. (`docs/plans/economy-kernel.md` sections 2.4.3, 4.6)

**Gaps:**
- No RollbackPlan/CompensatingActionPlan in contract terms for high severity WorkUnits.
- No rollback receipts (`rollback_executed`, `rollback_failed`) with stable reason codes and linkage to the triggering incident/claim.

**Kernel-addressable primitives to add:**
- Contract terms: `RollbackPlan` (playbook ref, deadline, required flag) and/or compensating action plan.
- Receipts: rollback execution receipts with reason codes; policy can require plans by slice.

### 8) Synthetic Practice Exports (Portable Benchmarks)

**What the paper wants:** synthetic practice and open benchmarks as public goods, plus interoperability of formats.

**What we have:**
- Spec: GroundTruthCase -> SimulationScenario pipeline is mandatory in concept. (`docs/plans/economy-kernel.md` section 6.10)

**Gaps:**
- No portable scenario/benchmark package format and no policy-gated export surface for sanitized scenarios.

**Kernel-addressable primitives to add:**
- `SimulationScenarioPackage` plus an optional export service that emits redaction receipts and stable hashes for scenario bundles.

### 9) Privacy-Preserving Safety Signals (Public + Restricted)

**What the paper wants:** privacy-preserving sharing of safety signals (public aggregates plus restricted sharing to certified parties).

**What we have:**
- Spec: `/stats` with redaction rules. (`docs/plans/economy-kernel.md` section 7.4)
- Impl: snapshot inputs are redacted (receipt-window digest only). (`apps/autopilot-desktop/src/state/economy_snapshot.rs`)

**Gaps:**
- No explicit SafetySignal object/feed and no restricted sharing mode.

**Kernel-addressable primitives to add:**
- `SafetySignal` objects derived from receipts/incidents (taxonomy, severity, affected slices, hashed indicators), with public aggregate and restricted feed publication modes.

### 10) Certification + Safe Harbor + "Digital Borders"

**What the paper wants:** cross-border certification and the ability to enforce standards (or block uncertified services) for high-risk classes.

**What we have:** policy-driven gating exists, but certification is not modeled as a kernel object/receipt type.

**Kernel-addressable primitives to add:**
- `SafetyCertification` object and receipts: `certification_issued`, `certification_revoked`.
- Policy rules: require certs for specified slices and allow safe-harbor relaxations for certified actors.

### 11) White-Hat Auditing + Bounties As Kernel-Native Patterns

**What the paper wants:** independent auditing/red-teaming with publishable findings and pay-on-verified-finding patterns.

**What we have:** no standard audit/redteam WorkUnit templates or bounty settlement flows.

**Kernel-addressable primitives to add:**
- WorkUnit templates for `audit`/`redteam`/`incident_repro` with mandatory provenance.
- Canonical bounty contract flow: verdict-gated settlement, optional escrow, dispute bond anti-spam.

### 12) Correlation-Adjusted "Effective sv"

**What the paper wants:** discount correlated verification to avoid false confidence and correlated failures.

**What we have:**
- Spec: correlation is first-class; impl reports correlated verification share and includes correlation in the sv breakdown key.

**Gaps:**
- No correlation-adjusted `sv_effective` (or `rho_effective`) metric for routing/autonomy gating.

**Kernel-addressable primitives to add:**
- Add `sv_effective` to snapshots and define a deterministic discount function keyed to observed correlation signals.

### 13) Optional Anchoring (Cross-Org Trust Without Payload Sharing)

**What the paper wants:** an immutable substrate that increases trust across orgs without forcing private payload disclosure.

**What we have:** snapshot hashes and receipt hashes exist, but are not anchored externally.

**Kernel-addressable primitives to add:**
- Optional anchoring receipts that publish snapshot hashes and/or receipt Merkle roots to a neutral public ledger (hashes only, policy-gated, no secrets).

## Minimal Spec/Proto Deltas (One Final Pass)

Spec deltas (normative text-level patches to `docs/plans/economy-kernel.md`):
- Add a new invariant: identity assurance is policy-bounded and receipted (auth assurance level recorded for key actions; policy can require by slice and by role).
- Add a Monitoring/Drift module: drift receipts are first-class and can trigger the same deterministic throttle ladder as `sv`/`XA_hat`.
- Make risk-metered liability pricing explicit: separate execution price from liability premium/risk charge, and bind pricing to snapshot windows/hashes.
- Make insurability prerequisites explicit: standardized incident reporting (taxonomy + linkage + export), disclosure bundles, proof-of-coverage hooks.
- Make rollback robustness explicit: high-severity lanes require a rollback/compensating-action plan; rollback attempts are receipted.
- Add optional anchoring: policy-gated anchoring receipts that publish snapshot hashes and/or receipt roots to a neutral public ledger (hashes only).

Proto plan deltas (surgical additions to `docs/plans/economy-kernel-proto.md` plan):
- `openagents/common/v1`: add `AuthAssuranceLevel` + credential references; extend receipt hints with assurance level; extend provenance bundles with explicit data source/permissioning refs and verifiable inference/execution attestation hooks.
- `openagents/aegis/outcomes/v1/outcomes_work.proto`: add explicit liability premium/risk charge fields (separate from execution price), plus pricing snapshot refs; add RollbackPlan and MonitoringPlan hooks into contract terms.
- New incidents proto: `IncidentReport` / `GroundTruthCase` with versioned taxonomy codes, mandatory receipt/evidence linkage, and rollback fields.
- `openagents/economy/v1/economy_snapshot.proto`: add insurer-relevant headline metrics (liability premiums collected, claims paid, loss ratio, capital coverage), drift alert counts, and assurance distribution.
- `openagents/policy/v1/policy_bundle.proto`: add rule types for authentication/personhood requirements, risk pricing rules, monitoring/drift rules, and explicit rollback/certification requirements by slice.

## Recent Issues (Evidence Of What The Repo Just Landed)

As of 2026-03-05:
- Open issues: none.
- Recently updated CLOSED issues show the repo has just landed the MVP/Earn subset of the kernel machinery:
  - #2945 [Earn Kernel] Program: Economy Kernel compliance for Autopilot Earn
  - #2946 Receipt envelope + canonical hashing foundation (desktop)
  - #2947 Emit kernel receipts for Earn job lifecycle + wallet settlement evidence
  - #2948 Cross-receipt linkage graph + transitive navigability (Earn)
  - #2949 PolicyContext + reason_code (hash-bound) for Earn authority effects
  - #2950 EconomySnapshot minute snapshots for Earn (redacted, subscription-driven)
  - #2952 WorkUnit metadata mapping for Earn jobs (category/tfb/severity/B)
  - #2953 Receipt store/index + correction/supersession semantics (desktop)
  - #2954 Idempotency keys + IDEMPOTENCY_CONFLICT semantics

None of the paper's broader ecosystem primitives (incident reporting objects, disclosure bundles, certification, safety signals feed, identity assurance levels) appear as first-class tracked issues yet.

## Bottom Line

The Economy Kernel spec and the Earn kernel implementation already embody the paper's core technical thesis (verified output as the objective; verification as production capacity; receipts/provenance as truth; correlation-aware gating).

To fully encompass the paper's ecosystem recommendations, the kernel still needs explicit, exportable primitives for:

- insurance boundary primitives (risk-metered liability pricing, proof-of-coverage/capital hooks)
- continuous monitoring and drift detection receipts (policy-triggerable)
- identity assurance levels and personhood gates (role-based)
- provenance requirement classes (verifiable inference/execution attestation) plus explicit data source/permissioning refs
- standardized incident reporting and deterministic audit package exports (versioned taxonomy + linkage)
- interoperable export formats and outcome registries
- rollback plans and rollback execution receipts (by high-severity slice)
- privacy-preserving safety signals (public aggregate plus restricted sharing)
- certification/safe-harbor/digital-border compatible policy hooks
- white-hat audit/bounty WorkUnit patterns
- portable synthetic practice packages (exportable scenario/benchmark bundles)
- correlation-adjusted effective verifiable share metrics
- optional anchoring of snapshot hashes / receipt roots to a neutral public ledger
