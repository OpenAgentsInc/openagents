Below are the two implementation-facing rewrites:

1. a **revised proto plan** (packages, file layout, and updated/added protos) that bakes in: `tfb`, severity, verification budget hints, provenance bundles, correlation groups, and **sv / Δm_hat / XA_hat** “metrics receipts” (minute snapshots).

2. a **revised PolicyBundle schema** (proto + semantics) with the knobs needed to gate autonomy based on `sv`, `XA_hat`, and correlation risk.

I’m keeping this aligned with your existing `openagents/*/v1` naming and with the kernel invariants (HTTP-only authority, deterministic receipts, idempotent).

---

## 1) Proto plan rewrite

### 1.1 Goals of the proto reshape

**What changes vs your earlier protos:**

* Move *kernel-wide* enums/types (verification tiers, `tfb`, severity, provenance grade, independence/correlation metadata) into **common** so every service can attach consistent hints into receipts.
* Add **identity assurance** and credential-proof references as first-class receipt/policy inputs (including personhood/org-vetting gates where policy requires them).
* Make provenance a **first-class typed bundle** (not just “some evidence ref”), so you can compute a deterministic `Pgrade`.
* Make provenance explicitly carry **data source** and **permissioning** references and support policy-gated verifiable inference/execution attestations.
* Make verification independence a **first-class constraint and report** (lineages + correlation groups + min distinct lineages).
* Add a canonical **EconomySnapshot** artifact emitted once per minute with a receipt (your “sv/Δm/XA metrics receipts”) so `/stats` is derived from receipts/snapshots exactly as the spec says.
* Add proto-first **incident reporting**, **safety signals**, **certification**, and **audit package exports** so insurability and interoperability are machine-legible.
* Add explicit **insurance-boundary pricing** fields (execution vs liability premium/risk charge) and **rollback/monitoring** contract hooks.
* Add a separate **policy package** that defines the PolicyBundle schema (see Part 2).

### 1.2 Proposed file layout

```text
proto/
  openagents/
    common/v1/
      common.proto                 # existing + expanded (tfb, severity, tiers, provenance, identity assurance)
    hydra/v1/
      abp_bonds.proto              # ABP reserve/release/draw (unchanged except minor links)
    aegis/outcomes/v1/
      outcomes_work.proto          # WorkUnit/Contract updated for pricing split, rollback, monitoring, provenance, independence
    aegis/incidents/v1/
      incidents.proto              # IncidentReport/NearMiss/GroundTruthCase + taxonomy linkage
    audit/v1/
      audit_package.proto          # deterministic export bundles + redaction tiers
    compliance/v1/
      certification.proto          # SafetyCertification issuance/revocation
    safety/v1/
      safety_signals.proto         # public aggregate + restricted signal feed
    economy/v1/
      economy_snapshot.proto       # minute snapshots: sv/sv_effective, Δm_hat, XA_hat, insurer + drift + identity metrics
    policy/v1/
      policy_bundle.proto          # auth, monitoring, risk pricing, certification, rollback knobs
```

### 1.3 Updated `proto/openagents/common/v1/common.proto`

This replaces your earlier common proto with the missing paper primitives: `tfb`, severity, tiers, provenance, and correlation metadata—plus optional receipt hints so `/stats` can be derived without fragile parsing.

```proto
syntax = "proto3";

package openagents.common.v1;

import "google/protobuf/struct.proto";

option java_multiple_files = true;

// -----------------------------
// Economy-wide enums
// -----------------------------

enum Asset {
  ASSET_UNSPECIFIED = 0;
  BTC_LN = 1;
  BTC_ONCHAIN = 2;
  USD = 3;
  USDC = 4;
}

enum FeedbackLatencyClass {
  FEEDBACK_LATENCY_CLASS_UNSPECIFIED = 0;
  INSTANT = 1;  // seconds/minutes
  SHORT = 2;    // hours/days
  LONG = 3;     // weeks/months
  UNKNOWN = 4;  // treated as LONG by default policy
}

enum SeverityClass {
  SEVERITY_CLASS_UNSPECIFIED = 0;
  LOW = 1;
  MEDIUM = 2;
  HIGH = 3;
  CRITICAL = 4;
}

enum VerificationTier {
  VERIFICATION_TIER_UNSPECIFIED = 0;
  TIER_O_OBJECTIVE = 1;
  TIER_1_CORRELATED = 2;
  TIER_2_HETEROGENEOUS = 3;
  TIER_3_ADJUDICATION = 4;
  TIER_4_HUMAN = 5;
}

enum ProvenanceGrade {
  PROVENANCE_GRADE_UNSPECIFIED = 0;
  P0_MINIMAL = 1;   // minimal receipts
  P1_TOOLCHAIN = 2; // artifacts + tool chain evidence
  P2_LINEAGE = 3;   // + model/checker lineage metadata
  P3_ATTESTED = 4;  // + attestations (signers/hardware/runtime)
}

enum AuthAssuranceLevel {
  AUTH_ASSURANCE_LEVEL_UNSPECIFIED = 0;
  ANON = 1;
  AUTHENTICATED = 2;
  ORG_KYC = 3;
  PERSONHOOD = 4;
  GOV_ID = 5;
  HARDWARE_BOUND = 6;
}

// -----------------------------
// Money and trace/policy context
// -----------------------------

message Money {
  Asset asset = 1;

  oneof amount {
    uint64 amount_msats = 2;
    uint64 amount_sats = 3;
  }
}

message TraceContext {
  string session_id = 1;
  string trajectory_hash = 2;
  string job_hash = 3;

  string run_id = 4;
  string work_unit_id = 5;
  string contract_id = 6;
  string claim_id = 7;
}

message PolicyContext {
  string policy_bundle_id = 1;
  string policy_version = 2;
  string approved_by = 3; // npub / org id / service principal
}

message CredentialRef {
  string issuer = 1;
  string subject = 2;
  string credential_type = 3; // did_vc, org_kyc, personhood_proof, gov_id, hardware_attestation
  string uri = 4;
  string digest = 5; // "sha256:<hex>"
  google.protobuf.Struct meta = 10;
}

message IdentityAssurance {
  string subject_id = 1;
  AuthAssuranceLevel level = 2;
  CredentialRef credential = 3;
}

// -----------------------------
// Evidence and artifacts
// -----------------------------

message ArtifactRef {
  string uri = 1;       // s3://, ipfs://, oa://, file://
  string digest = 2;    // "sha256:<hex>"
  string mime_type = 3;
  uint64 size_bytes = 4;
  google.protobuf.Struct meta = 10;
}

message EvidenceRef {
  string kind = 1;      // e.g. "replay_bundle", "test_report", "provenance_bundle"
  string uri = 2;
  string digest = 3;    // "sha256:<hex>"
  google.protobuf.Struct meta = 10;
}

// -----------------------------
// Provenance (typed bundle)
// -----------------------------

message ModelIdentity {
  string provider = 1;     // openai, anthropic, local, etc.
  string family = 2;       // gpt-5, claude-4, llama, etc.
  string name = 3;         // model name
  string version = 4;      // build tag / semver / commit
  string digest = 5;       // optional: weights digest, image digest, etc.
  google.protobuf.Struct meta = 10;
}

message ToolInvocation {
  string tool_id = 1;          // "oa.git.apply_patch", "oa.sandbox_run", ...
  string tool_version = 2;     // semver/commit
  string request_digest = 3;   // "sha256:<hex>" of canonical request
  string response_digest = 4;  // "sha256:<hex>" of canonical response
  int64 started_at_ms = 5;
  int64 ended_at_ms = 6;
  google.protobuf.Struct meta = 10;
}

message AttestationRef {
  string kind = 1;     // "threshold_sig", "tpm_quote", "sigstore", "human_signoff"
  string uri = 2;
  string digest = 3;   // "sha256:<hex>"
  google.protobuf.Struct meta = 10;
}

message ProvenanceBundle {
  // This bundle itself should be stored as an evidence object with a digest;
  // receipts should reference it by EvidenceRef (kind="provenance_bundle").
  repeated ArtifactRef inputs = 1;
  repeated ArtifactRef outputs = 2;

  repeated ToolInvocation tool_invocations = 3;

  // Who produced the output (worker model/tooling) and who verified it (checker lineages).
  repeated ModelIdentity producer_lineage = 4;
  repeated ModelIdentity checker_lineage = 5;

  // Correlation groups allow policy to reject "AI verifies AI" setups.
  repeated string correlation_groups = 6; // e.g. "same_provider", "same_finetune", "shared_toolchain"

  repeated AttestationRef attestations = 7;

  // Explicit process-verification references.
  repeated EvidenceRef data_source_refs = 8;
  repeated EvidenceRef permissioning_refs = 9;
  repeated IdentityAssurance identities_involved = 10;

  google.protobuf.Struct meta = 20;
}

// -----------------------------
// Verification independence constraints + report
// -----------------------------

message IndependenceRequirement {
  // Minimum distinct checker lineages required (by identity digest or provider+family).
  uint32 min_distinct_checker_lineages = 1;

  // Correlation groups that are disallowed for unlocking settlement/warranty.
  repeated string disallowed_correlation_groups = 2;

  // If true, at least one Tier 4 human step is required.
  bool require_human_step = 3;
}

message IndependenceReport {
  repeated ModelIdentity checkers_used = 1;
  repeated string correlation_groups_present = 2;

  uint32 distinct_checker_lineages_observed = 3;

  // True iff correlation_groups_present intersects disallowed list (or other policy checks fail).
  bool correlation_violation = 4;
}

// -----------------------------
// Receipt primitives
// -----------------------------

message ReceiptRef {
  string receipt_id = 1;
  string receipt_type = 2;
  string canonical_hash = 3; // "sha256:<hex>"
}

message ReceiptHints {
  // Optional standardized hints so /stats can aggregate reliably.
  string category = 1; // domain/category, e.g. "compute", "autopilot", "legal", "ops"
  FeedbackLatencyClass tfb_class = 2;
  SeverityClass severity = 3;

  // Verification/provenance hints where applicable.
  VerificationTier achieved_verification_tier = 4;
  bool verification_correlated = 5;
  ProvenanceGrade provenance_grade = 6;
  AuthAssuranceLevel auth_assurance_level = 7;
  bool personhood_proved = 8;
  string reason_code = 9; // stable code, e.g. QUOTE_EXPIRED, BREAKER_ACTIVE, INSUFFICIENT_BUDGET

  // Optional: for economics rollups.
  Money notional = 10;
  Money liability_premium = 11;
}

message Receipt {
  string receipt_id = 1;
  string receipt_type = 2;
  int64 created_at_ms = 3;

  string canonical_hash = 4; // "sha256:<hex>"
  string idempotency_key = 5;

  TraceContext trace = 10;
  PolicyContext policy = 11;

  string inputs_hash = 20;  // "sha256:<hex>"
  string outputs_hash = 21; // "sha256:<hex>"

  repeated EvidenceRef evidence = 30;

  ReceiptHints hints = 40;

  map<string, string> tags = 50; // non-normative, excluded from canonical_hash
}

// -----------------------------
// Verification budget hint (paper B / capacity)
// -----------------------------

message VerificationBudgetHint {
  // Optional. This is a *hint* used for planning/gating; policy remains authoritative.
  // Include one or more; leaving empty means policy defaults.
  Money max_spend = 1;              // sats/msats budget for verification/underwriting
  uint32 max_human_minutes = 2;      // max human review minutes (if tracked)
  uint32 max_checker_runs = 3;       // max automated verification runs
  google.protobuf.Struct meta = 10;
}
```

Additional requirement for attestation semantics:

* `AttestationRef.kind` and/or `CredentialRef.credential_type` MUST support named classes for:
  * model version attestation (verifiable inference)
  * runtime integrity attestation (execution attestation)
  so policy can require them by `category/tfb/severity` deterministically.

---

### 1.4 Updated `proto/openagents/aegis/outcomes/v1/outcomes_work.proto`

Key changes:

* WorkUnit now requires `tfb_class`, `severity`, and a `verification_budget_hint`.
* Contract + VerificationPlan now include **provenance requirements** and **independence requirements**.
* Submission includes a **provenance bundle evidence ref** (typed by `kind="provenance_bundle"`).
* Verdict includes computed `provenance_grade` and an `IndependenceReport`, and sets correlation flags into receipts.

```proto
syntax = "proto3";

package openagents.aegis.outcomes.v1;

import "google/api/annotations.proto";
import "openagents/common/v1/common.proto";
import "openagents/hydra/v1/abp_bonds.proto";

option java_multiple_files = true;

enum WorkUnitKind {
  WORK_UNIT_KIND_UNSPECIFIED = 0;
  COMPUTE_JOB = 1;
  SKILL_INVOCATION = 2;
  ARTIFACT_BUNDLE = 3;
  L402_SERIES = 4;
}

enum VerifiabilityClass {
  VERIFIABILITY_CLASS_UNSPECIFIED = 0;
  OBJECTIVE = 1;
  SUBJECTIVE = 2;
  LOW_VERIFIABILITY = 3;
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
  ROLLBACK = 4;
  COMPENSATING_ACTION = 5;
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
  string description = 1;

  // Optional: objective harness or rubric pointers.
  openagents.common.v1.ArtifactRef harness_ref = 10;           // for Tier O
  openagents.common.v1.EvidenceRef rubric_ref = 11;            // for subjective scoring
  openagents.common.v1.EvidenceRef adjudication_policy_ref = 12;

  repeated openagents.common.v1.ArtifactRef required_artifacts = 20;
  repeated openagents.common.v1.EvidenceRef required_evidence = 21;

  // Required by policy for high-severity irreversible lanes.
  RollbackPlan rollback_plan = 30;
  MonitoringPlan monitoring_plan = 31;
}

message RollbackPlan {
  bool required = 1;
  openagents.common.v1.EvidenceRef playbook_ref = 2;
  uint64 deadline_ms = 3;
}

message MonitoringPlan {
  repeated openagents.common.v1.EvidenceRef drift_detectors = 1;
  repeated string monitored_metrics = 2;
}

message WorkUnit {
  string work_unit_id = 1;
  WorkUnitKind kind = 2;

  string category = 3;    // required for sv/Δm/XA breakdowns
  string title = 4;
  string description = 5;

  string creator_id = 6;  // buyer/operator

  AcceptanceCriteria acceptance = 10;

  // --- Paper-critical metadata (normative in the kernel spec) ---
  openagents.common.v1.FeedbackLatencyClass tfb_class = 20;
  openagents.common.v1.SeverityClass severity = 21;
  openagents.common.v1.VerificationBudgetHint verification_budget_hint = 22;

  openagents.common.v1.TraceContext trace = 30;
  openagents.common.v1.PolicyContext policy = 31;

  int64 created_at_ms = 40;
}

message BondRequirement {
  openagents.hydra.v1.BondPartyRole party_role = 1;
  openagents.hydra.v1.BondReason reason = 2;
  openagents.common.v1.Money min_amount = 3;
  openagents.common.v1.Money max_amount = 4;
}

message WarrantyTerms {
  RemedyType remedy = 1;
  openagents.common.v1.Money coverage_cap = 2;
  uint64 warranty_window_ms = 3;
  repeated string exclusions = 10;
}

message ProvenanceRequirement {
  // Minimum provenance grade needed to unlock settlement / warranty issuance.
  openagents.common.v1.ProvenanceGrade min_grade = 1;

  // Require provenance bundle evidence ref on submission.
  bool require_provenance_bundle = 2;

  // Require at least one attestation in the provenance bundle (P3 paths).
  bool require_attestation = 3;
}

message VerificationPlan {
  VerifiabilityClass verifiability = 1;

  // Required minimum tier to unlock settlement/warranty.
  openagents.common.v1.VerificationTier required_tier = 2;

  // Independence constraints (heterogeneity / correlation gating).
  openagents.common.v1.IndependenceRequirement independence_requirement = 3;

  // Provenance constraints.
  ProvenanceRequirement provenance_requirement = 4;

  repeated openagents.common.v1.EvidenceRef planned_checks = 10;
  openagents.common.v1.EvidenceRef adjudication_policy_ref = 11;
}

message OutcomeContract {
  string contract_id = 1;
  string work_unit_id = 2;

  string buyer_id = 10;
  string worker_id = 11;
  string underwriter_group_id = 12;

  // Explicit pricing split (insurance boundary).
  openagents.common.v1.Money execution_price = 20;
  openagents.common.v1.Money verification_fee = 21;
  openagents.common.v1.Money liability_premium = 22;
  openagents.common.v1.ReceiptRef pricing_snapshot_ref = 23;

  VerificationPlan verification_plan = 30;
  repeated BondRequirement bond_requirements = 31;

  bool warranty_enabled = 40;
  WarrantyTerms warranty_terms = 41;

  ContractState state = 50;

  openagents.common.v1.TraceContext trace = 60;
  openagents.common.v1.PolicyContext policy = 61;

  int64 created_at_ms = 70;
  int64 updated_at_ms = 71;
}

message BondLink {
  string bond_id = 1;
  openagents.hydra.v1.BondPartyRole party_role = 2;
  openagents.hydra.v1.BondReason reason = 3;
  openagents.common.v1.Money amount_reserved = 4;
}

message Submission {
  repeated openagents.common.v1.ArtifactRef outputs = 1;
  repeated openagents.common.v1.EvidenceRef evidence = 2;

  // REQUIRED when provenance_requirement.require_provenance_bundle = true
  openagents.common.v1.EvidenceRef provenance_bundle = 3; // kind must be "provenance_bundle"

  int64 submitted_at_ms = 10;
}

message Verdict {
  BinaryOutcome outcome = 1;

  openagents.common.v1.VerificationTier achieved_tier = 2;

  // Computed from provenance bundle contents (deterministic).
  openagents.common.v1.ProvenanceGrade provenance_grade = 3;

  // Required: independence report of checkers used.
  openagents.common.v1.IndependenceReport independence_report = 4;

  // Evidence pointers (harness output, adjudication notes, etc.)
  repeated openagents.common.v1.EvidenceRef evidence = 10;

  // References to Hydra/wallet settlement receipts that happened as a consequence.
  repeated openagents.common.v1.ReceiptRef settlement_receipts = 20;

  int64 decided_at_ms = 30;
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

  string claimant_id = 10;
  string reason = 11;

  repeated openagents.common.v1.EvidenceRef evidence = 12;

  ClaimState state = 20;

  BondLink dispute_bond = 30;

  int64 opened_at_ms = 40;
  int64 updated_at_ms = 41;
}

message ClaimResolution {
  ClaimState final_state = 1;
  openagents.common.v1.Money payout = 2;

  repeated BondLink bonds_drawn = 3;
  repeated openagents.common.v1.EvidenceRef evidence = 10;
  repeated openagents.common.v1.ReceiptRef settlement_receipts = 20;

  int64 resolved_at_ms = 30;
}

// ---- RPCs ----

message CreateWorkUnitRequest {
  WorkUnitKind kind = 1;
  string category = 2;
  string title = 3;
  string description = 4;
  string creator_id = 5;

  AcceptanceCriteria acceptance = 10;

  // Required paper-critical metadata.
  openagents.common.v1.FeedbackLatencyClass tfb_class = 20;
  openagents.common.v1.SeverityClass severity = 21;
  openagents.common.v1.VerificationBudgetHint verification_budget_hint = 22;

  string idempotency_key = 50;
  openagents.common.v1.TraceContext trace = 51;
  openagents.common.v1.PolicyContext policy = 52;
}

message CreateWorkUnitResponse {
  WorkUnit work_unit = 1;
  openagents.common.v1.Receipt receipt = 2; // hints MUST include category/tfb/severity
}

message GetWorkUnitRequest { string work_unit_id = 1; }
message GetWorkUnitResponse { WorkUnit work_unit = 1; }

message CreateContractRequest {
  string work_unit_id = 1;

  string buyer_id = 10;
  string worker_id = 11;

  openagents.common.v1.Money execution_price = 20;
  openagents.common.v1.Money verification_fee = 21;
  openagents.common.v1.Money liability_premium = 22;
  openagents.common.v1.ReceiptRef pricing_snapshot_ref = 23;

  VerificationPlan verification_plan = 30;
  repeated BondRequirement bond_requirements = 31;

  bool warranty_enabled = 40;
  WarrantyTerms warranty_terms = 41;

  string idempotency_key = 60;
  openagents.common.v1.TraceContext trace = 61;
  openagents.common.v1.PolicyContext policy = 62;
}

message CreateContractResponse {
  OutcomeContract contract = 1;
  openagents.common.v1.Receipt receipt = 2;
}

message AttachBondRequest {
  string contract_id = 1;
  string bond_id = 2;
  openagents.hydra.v1.BondPartyRole party_role = 3;
  openagents.hydra.v1.BondReason reason = 4;

  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message AttachBondResponse {
  repeated BondLink bonds = 1;
  openagents.common.v1.Receipt receipt = 2;
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
  openagents.common.v1.Receipt receipt = 2; // MUST include provenance grade once computed (if available)
}

message StartVerificationRequest {
  string contract_id = 1;

  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message StartVerificationResponse {
  OutcomeContract contract = 1;
  openagents.common.v1.Receipt receipt = 2;
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
  openagents.common.v1.Receipt receipt = 3; // hints MUST include tier + correlated flag + provenance grade
}

message OpenClaimRequest {
  string contract_id = 1;
  string claimant_id = 10;
  string reason = 11;
  repeated openagents.common.v1.EvidenceRef evidence = 12;

  string dispute_bond_id = 20; // optional Hydra ABP bond

  string idempotency_key = 30;
  openagents.common.v1.TraceContext trace = 31;
  openagents.common.v1.PolicyContext policy = 32;
}

message OpenClaimResponse {
  Claim claim = 1;
  openagents.common.v1.Receipt receipt = 2;
}

message ResolveClaimRequest {
  string claim_id = 1;
  ClaimState final_state = 2;
  openagents.common.v1.Money payout = 3;
  repeated openagents.common.v1.EvidenceRef evidence = 10;

  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message ResolveClaimResponse {
  Claim claim = 1;
  ClaimResolution resolution = 2;
  openagents.common.v1.Receipt receipt = 3;
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

### 1.5 New: `proto/openagents/aegis/incidents/v1/incidents.proto`

This makes incident reporting proto-first and insurability-ready (versioned taxonomy + mandatory receipt/evidence linkage + outcome registry entries).

Normative implementation notes:

* Incident objects are append-only, hash-addressed revisions; updates/resolutions supersede prior digests, not overwrite them.
* Taxonomy meaning is immutable for a `(taxonomy_id, taxonomy_version, code)` tuple.
* Export surfaces MUST support at least `public` and `restricted` redaction tiers for incident audit packages.

```proto
syntax = "proto3";

package openagents.aegis.incidents.v1;

import "google/api/annotations.proto";
import "openagents/common/v1/common.proto";

option java_multiple_files = true;

enum IncidentKind {
  INCIDENT_KIND_UNSPECIFIED = 0;
  INCIDENT = 1;
  NEAR_MISS = 2;
  GROUND_TRUTH_CASE = 3;
}

enum IncidentState {
  INCIDENT_STATE_UNSPECIFIED = 0;
  OPEN = 1;
  TRIAGED = 2;
  MITIGATED = 3;
  RESOLVED = 4;
}

message IncidentTaxonomyCode {
  string taxonomy_id = 1;      // "oa.incident.taxonomy"
  string taxonomy_version = 2; // "2026.02"
  string code = 3;             // e.g. PAYOUT_POINTER_MISMATCH
  string label = 4;
}

message DisclosureBundleRef {
  openagents.common.v1.EvidenceRef disclosure_artifact = 1;
  string schema_version = 2;
}

message RollbackAttemptRef {
  string rollback_receipt_id = 1;
  string rollback_reason_code = 2;
  bool rollback_succeeded = 3;
}

message IncidentReport {
  string incident_id = 1;
  IncidentKind kind = 2;
  IncidentState state = 3;
  IncidentTaxonomyCode taxonomy = 4;
  openagents.common.v1.SeverityClass severity = 5;
  string category = 6;
  openagents.common.v1.FeedbackLatencyClass tfb_class = 7;

  int64 occurred_at_ms = 10;
  int64 reported_at_ms = 11;
  int64 resolved_at_ms = 12;

  string summary = 20;
  string impact_notes = 21;
  double estimated_loss_ratio_delta = 22;

  repeated openagents.common.v1.ReceiptRef linked_receipts = 30;   // contract/verdict/settlement
  repeated openagents.common.v1.EvidenceRef evidence_digests = 31; // no raw payload
  repeated RollbackAttemptRef rollback_attempts = 32;
  DisclosureBundleRef disclosure_bundle = 33;

  openagents.common.v1.PolicyContext policy = 40;
  map<string, string> tags = 50;
}

message OutcomeRegistryEntry {
  string entry_id = 1;
  string work_unit_id = 2;
  string category = 3;
  openagents.common.v1.FeedbackLatencyClass tfb_class = 4;
  openagents.common.v1.SeverityClass severity = 5;

  string verdict_outcome = 10;
  string settlement_outcome = 11;
  string claim_outcome = 12;
  string remedy_outcome = 13;

  repeated IncidentTaxonomyCode incident_tags = 20;
  repeated openagents.common.v1.ReceiptRef linked_receipts = 21;
  repeated openagents.common.v1.EvidenceRef evidence_digests = 22;
}

message ReportIncidentRequest {
  IncidentReport incident = 1;
  string idempotency_key = 10;
  openagents.common.v1.TraceContext trace = 11;
  openagents.common.v1.PolicyContext policy = 12;
}

message ReportIncidentResponse {
  IncidentReport incident = 1;
  openagents.common.v1.Receipt receipt = 2; // economy.incident.reported.v1 / updated / resolved
}

message UpsertOutcomeRegistryEntryRequest {
  OutcomeRegistryEntry entry = 1;
  string idempotency_key = 10;
  openagents.common.v1.TraceContext trace = 11;
  openagents.common.v1.PolicyContext policy = 12;
}

message UpsertOutcomeRegistryEntryResponse {
  OutcomeRegistryEntry entry = 1;
  openagents.common.v1.Receipt receipt = 2; // economy.outcome_registry.upserted.v1
}

service IncidentService {
  rpc ReportIncident(ReportIncidentRequest) returns (ReportIncidentResponse) {
    option (google.api.http) = { post: "/v1/aegis/incidents:report" body: "*" };
  }

  rpc UpsertOutcomeRegistryEntry(UpsertOutcomeRegistryEntryRequest)
      returns (UpsertOutcomeRegistryEntryResponse) {
    option (google.api.http) = { post: "/v1/aegis/incidents/outcome_registry:upsert" body: "*" };
  }
}
```

---

### 1.6 New: `proto/openagents/compliance/v1/certification.proto`

This adds first-class certification state that can be enforced by policy and exported for cross-border/safe-harbor checks.

```proto
syntax = "proto3";

package openagents.compliance.v1;

import "google/api/annotations.proto";
import "openagents/common/v1/common.proto";

option java_multiple_files = true;

enum CertificationState {
  CERTIFICATION_STATE_UNSPECIFIED = 0;
  ACTIVE = 1;
  REVOKED = 2;
  EXPIRED = 3;
}

message CertificationScope {
  string category = 1;
  openagents.common.v1.FeedbackLatencyClass tfb_class = 2;
  openagents.common.v1.SeverityClass min_severity = 3;
  openagents.common.v1.SeverityClass max_severity = 4;
}

message SafetyCertification {
  string certification_id = 1;
  CertificationState state = 2;
  string certification_level = 3;
  repeated CertificationScope scope = 4;

  int64 valid_from_ms = 10;
  int64 valid_until_ms = 11;

  openagents.common.v1.CredentialRef issuer = 20;
  repeated openagents.common.v1.EvidenceRef required_evidence = 21; // audits/provenance/incidents summary

  openagents.common.v1.PolicyContext policy = 30;
  map<string, string> tags = 40;
}

message IssueCertificationRequest {
  SafetyCertification certification = 1;
  string idempotency_key = 10;
  openagents.common.v1.TraceContext trace = 11;
  openagents.common.v1.PolicyContext policy = 12;
}

message IssueCertificationResponse {
  SafetyCertification certification = 1;
  openagents.common.v1.Receipt receipt = 2; // economy.certification.issued.v1
}

message RevokeCertificationRequest {
  string certification_id = 1;
  string reason_code = 2;
  repeated openagents.common.v1.EvidenceRef evidence = 10;
  string idempotency_key = 20;
  openagents.common.v1.TraceContext trace = 21;
  openagents.common.v1.PolicyContext policy = 22;
}

message RevokeCertificationResponse {
  SafetyCertification certification = 1;
  openagents.common.v1.Receipt receipt = 2; // economy.certification.revoked.v1
}
```

---

### 1.7 New: `proto/openagents/safety/v1/safety_signals.proto`

This enables privacy-preserving safety signal sharing: public aggregates and restricted feeds from the same deterministic source objects.

```proto
syntax = "proto3";

package openagents.safety.v1;

import "google/api/annotations.proto";
import "openagents/common/v1/common.proto";

option java_multiple_files = true;

enum SafetySignalVisibility {
  SAFETY_SIGNAL_VISIBILITY_UNSPECIFIED = 0;
  PUBLIC_AGGREGATE = 1;
  RESTRICTED_FEED = 2;
}

message SafetySignal {
  string signal_id = 1;
  int64 emitted_at_ms = 2;

  string category = 10;
  openagents.common.v1.FeedbackLatencyClass tfb_class = 11;
  openagents.common.v1.SeverityClass severity = 12;
  string incident_taxonomy_code = 13;

  repeated string hashed_indicators = 20; // no raw payloads
  repeated openagents.common.v1.ReceiptRef source_receipts = 21;
  repeated openagents.common.v1.EvidenceRef source_evidence = 22;

  SafetySignalVisibility visibility = 30;
  string restricted_audience_class = 31; // auditor / underwriter / verifier
}

message PublishSafetySignalRequest {
  SafetySignal signal = 1;
  string idempotency_key = 10;
  openagents.common.v1.TraceContext trace = 11;
  openagents.common.v1.PolicyContext policy = 12;
}

message PublishSafetySignalResponse {
  SafetySignal signal = 1;
  openagents.common.v1.Receipt receipt = 2; // economy.safety_signal.published.v1
}
```

---

### 1.8 New: `proto/openagents/audit/v1/audit_package.proto`

This defines deterministic interoperable exports (public/restricted redaction tiers) for underwriting, incident forensics, and certification replay.

```proto
syntax = "proto3";

package openagents.audit.v1;

import "google/api/annotations.proto";
import "openagents/common/v1/common.proto";
import "openagents/aegis/incidents/v1/incidents.proto";
import "openagents/compliance/v1/certification.proto";

option java_multiple_files = true;

enum AuditRedactionTier {
  AUDIT_REDACTION_TIER_UNSPECIFIED = 0;
  PUBLIC = 1;
  RESTRICTED = 2;
}

message AuditPackage {
  string package_id = 1;
  string package_hash = 2; // deterministic hash over canonical export payload
  int64 generated_at_ms = 3;
  AuditRedactionTier redaction_tier = 4;

  repeated openagents.common.v1.Receipt receipts = 10;
  repeated openagents.aegis.incidents.v1.IncidentReport incidents = 11;
  repeated openagents.aegis.incidents.v1.OutcomeRegistryEntry outcome_registry_entries = 12;
  repeated openagents.compliance.v1.SafetyCertification certifications = 13;

  repeated openagents.common.v1.EvidenceRef snapshot_refs = 20;
  repeated openagents.common.v1.EvidenceRef anchor_refs = 21; // hash-only anchoring proofs
}

message ExportAuditPackageRequest {
  int64 start_inclusive_ms = 1;
  int64 end_inclusive_ms = 2;
  string category = 3;
  AuditRedactionTier redaction_tier = 4;

  string idempotency_key = 10;
  openagents.common.v1.TraceContext trace = 11;
  openagents.common.v1.PolicyContext policy = 12;
}

message ExportAuditPackageResponse {
  AuditPackage package = 1;
  openagents.common.v1.Receipt receipt = 2; // economy.audit_package.exported.v1
}
```

---

### 1.9 Updated: `proto/openagents/economy/v1/economy_snapshot.proto` (sv / Δm_hat / XA_hat receipts)

This expands the minute snapshot to include insurer/monitoring/interoperability metrics and the correlation-adjusted trust headline used for autonomy gating.

```proto
syntax = "proto3";

package openagents.economy.v1;

import "google/api/annotations.proto";
import "openagents/common/v1/common.proto";

option java_multiple_files = true;

message MetricKey {
  string category = 1;
  openagents.common.v1.FeedbackLatencyClass tfb_class = 2;
  openagents.common.v1.SeverityClass severity = 3;
  openagents.common.v1.VerificationTier verification_tier = 4;
  bool verification_correlated = 5;
  openagents.common.v1.ProvenanceGrade provenance_grade = 6;
}

message SvBreakdownRow {
  MetricKey key = 1;
  uint64 total_work_units = 2;
  uint64 verified_work_units = 3;
  double sv = 4;
}

message DistributionRow {
  string label = 1;
  uint64 count = 2;
  double share = 3;
}

message IncidentBucket {
  string taxonomy_code = 1;
  openagents.common.v1.SeverityClass severity = 2;
  uint64 incidents_24h = 3;
  uint64 near_misses_24h = 4;
  double incident_rate = 5;
  double near_miss_rate = 6;
}

message DriftSignalRow {
  string detector_id = 1;
  string signal_code = 2;
  uint64 count_24h = 3;
  double ratio = 4;
  double threshold = 5;
  double score = 6;
  bool alert = 7;
}

message EconomySnapshot {
  string snapshot_id = 1;
  int64 as_of_ms = 2;
  string snapshot_hash = 3; // "sha256:<hex>"

  double sv = 10;
  double sv_effective = 11; // correlation-adjusted verifiable share
  double rho = 12;
  double rho_effective = 13;
  uint64 N = 14;
  double NV = 15;

  double delta_m_hat = 20;
  double xa_hat = 21;
  double correlated_verification_share = 22;

  double provenance_p0_share = 30;
  double provenance_p1_share = 31;
  double provenance_p2_share = 32;
  double provenance_p3_share = 33;

  openagents.common.v1.Money liability_premiums_collected_24h = 40;
  openagents.common.v1.Money claims_paid_24h = 41;
  double loss_ratio = 42;
  double capital_coverage_ratio = 43;

  uint64 drift_alerts_24h = 50;
  uint64 rollback_attempts_24h = 51;
  uint64 rollback_successes_24h = 52;
  repeated DriftSignalRow top_drift_signals = 53;

  repeated IncidentBucket incident_buckets = 60;
  repeated DistributionRow auth_assurance_distribution = 61;
  double personhood_verified_share = 62;
  repeated DistributionRow certification_distribution = 63;

  repeated SvBreakdownRow sv_breakdown = 100;
  repeated openagents.common.v1.EvidenceRef inputs = 200;
}

message ComputeSnapshotRequest {
  int64 as_of_ms = 1;
  string idempotency_key = 10;
  openagents.common.v1.TraceContext trace = 11;
  openagents.common.v1.PolicyContext policy = 12;
}

message ComputeSnapshotResponse {
  EconomySnapshot snapshot = 1;
  openagents.common.v1.Receipt receipt = 2; // economy.stats.snapshot_receipt.v1
}
```

**Notes:**

* `sv_effective`/`rho_effective` are the policy-gating trust variables when correlated checker risk is non-trivial.
* Snapshot receipts and any throttle/withhold actions MUST reference the exact `snapshot_id`/`snapshot_hash` used to make the decision.

---

## 2) PolicyBundle schema rewrite (proto + semantics)

You need a PolicyBundle that can **bind autonomy to sv/XA and correlation risk**, plus set default verification/provenance rules by `category`, `tfb`, and `severity`.

### 2.1 What the policy must be able to express

At minimum, a PolicyBundle must let you set:

1. **Verification requirements**

* required tier by (category, tfb, severity)
* independence constraints (min distinct checker lineages; disallowed correlation groups)
* when Tier 1 is allowed (usually only LOW + INSTANT/SHORT)

2. **Provenance requirements**

* minimum provenance grade by (category, tfb, severity)
* whether provenance bundle is mandatory
* whether attestations are required at high severity

3. **Autonomy throttle rules** (the paper’s key enforcement)

* thresholds on `sv`, `sv_effective`, `xa_hat`, `delta_m_hat`, correlated verification share, drift alerts
* actions when thresholds are crossed:

  * increase required tier
  * require human step
  * raise required provenance grade
  * reduce or halt envelope issuance for a category
  * require approvals for high-severity work
  * disable warranties or cap warranty issuance

4. **Envelope and bond parameters**

* envelope caps, expiries, fee bps by category/severity
* bond multipliers for warranties and disputes

5. **Authentication and personhood gates**

* minimum `AuthAssuranceLevel` by `(category, tfb, severity)` and by role (`VERIFIER`, `UNDERWRITER`, `OPERATOR`, `REQUESTER`)
* explicit personhood requirement for sensitive actions (e.g. verdict finalization, high-severity settlement/withdraw)

6. **Risk pricing / insurance boundary**

* base liability premium (bps) and dynamic multipliers keyed to snapshot metrics
* reserve/capital requirements by slice and autonomy mode
* explicit linkage to `pricing_snapshot_ref`

7. **Monitoring / drift rules**

* detector requirements by slice
* deterministic trigger thresholds and action ordering for drift-induced throttles

8. **Certification + rollback requirements**

* required certification scopes for high-severity slices (“digital border”)
* mandatory rollback or compensating-action plans for irreversible effect classes
* safe-harbor relaxations only when certification + observed metrics satisfy policy

9. **Interpretation rules**

* which time windows to use (5m/1h/24h) for gating
* hysteresis / cooldown to avoid flapping breakers

### 2.2 `proto/openagents/policy/v1/policy_bundle.proto`

```proto
syntax = "proto3";

package openagents.policy.v1;

import "openagents/common/v1/common.proto";

option java_multiple_files = true;

enum AutonomyMode {
  AUTONOMY_MODE_UNSPECIFIED = 0;
  NORMAL = 1;
  DEGRADED = 2;           // tighter caps, higher tiers
  APPROVAL_REQUIRED = 3;  // human approvals for certain classes
  HALT = 4;               // stop new work in a domain/category
}

enum ActorRole {
  ACTOR_ROLE_UNSPECIFIED = 0;
  REQUESTER = 1;
  WORKER = 2;
  VERIFIER = 3;
  UNDERWRITER = 4;
  OPERATOR = 5;
}

message WindowSpec {
  // Rolling windows used for sv/xa calculations.
  bool use_5m = 1;
  bool use_1h = 2;
  bool use_24h = 3;
}

message TierRule {
  // Match conditions
  string category = 1; // empty => wildcard
  openagents.common.v1.FeedbackLatencyClass tfb_class = 2; // UNSPEC => wildcard
  openagents.common.v1.SeverityClass severity = 3;         // UNSPEC => wildcard

  // Required tier to unlock settlement/warranty.
  openagents.common.v1.VerificationTier required_tier = 10;

  // Independence constraints (heterogeneity/correlation).
  openagents.common.v1.IndependenceRequirement independence_requirement = 11;

  // Allow correlated Tier 1 checks to count? (usually only for LOW/INSTANT)
  bool allow_correlated_verification = 12;
}

message ProvenanceRule {
  string category = 1; // wildcard if empty
  openagents.common.v1.FeedbackLatencyClass tfb_class = 2;
  openagents.common.v1.SeverityClass severity = 3;

  openagents.common.v1.ProvenanceGrade min_grade = 10;
  bool require_provenance_bundle = 11;
  bool require_attestation = 12;
}

message EnvelopeRule {
  string category = 1;
  openagents.common.v1.SeverityClass severity = 2;

  // Envelope caps/expiries and fees.
  openagents.common.v1.Money max_amount = 10;
  uint64 ttl_ms = 11; // duration from issuance; authority receipts bind absolute expiry_ms
  uint32 fee_bps = 12;

  // Whether verdict is required for settlement (pay-after-verify default).
  bool require_verdict = 20;
}

message BondRule {
  string category = 1;
  openagents.common.v1.SeverityClass severity = 2;

  // Multipliers applied to contract price to size default bonds.
  // e.g. worker_self_bond = 0.10 means 10% of price.
  double worker_self_bond_fraction = 10;
  double underwriter_bond_fraction = 11;

  // Dispute bond sizing (anti-spam).
  double dispute_bond_fraction = 20;
}

message AutonomyCondition {
  // Match conditions
  string category = 1; // empty => wildcard

  // Thresholds (if unset, ignored).
  double min_sv = 10;                    // if sv falls below this
  double min_sv_effective = 11;          // correlation-adjusted trust floor
  double max_xa_hat = 12;                // if xa exceeds this
  double max_delta_m_hat = 13;           // if measurability gap too high
  double max_correlated_share = 14;      // if correlated verification share too high
  uint64 max_drift_alerts_24h = 15;      // drift detector pressure

  // Optional: enforce by severity/tfb slice (if set).
  openagents.common.v1.FeedbackLatencyClass tfb_class = 20;
  openagents.common.v1.SeverityClass severity = 21;
}

message AutonomyAction {
  oneof action {
    AutonomyMode set_mode = 1;

    // Tighten verification requirements dynamically.
    openagents.common.v1.VerificationTier raise_required_tier_to = 2;
    bool require_human_step = 3;

    // Tighten provenance requirements dynamically.
    openagents.common.v1.ProvenanceGrade raise_provenance_grade_to = 4;

    // Tighten spending dynamically.
    double envelope_max_amount_multiplier = 5; // e.g. 0.5 halves caps
    bool halt_new_envelopes = 6;

    // Warranty gating
    bool disable_warranties = 7;
    double warranty_coverage_cap_multiplier = 8;
  }
}

message AutonomyRule {
  AutonomyCondition condition = 1;
  repeated AutonomyAction actions = 2;

  // Hysteresis/cooldown to avoid flapping.
  uint64 min_duration_ms = 10; // condition must hold for this long
  uint64 cooldown_ms = 11;     // after triggering, ignore for this long
}

message AutonomyThrottlePolicy {
  WindowSpec windows = 1;

  // Default global thresholds.
  double global_min_sv = 10;
  double global_min_sv_effective = 11;
  double global_max_xa_hat = 12;
  double global_max_delta_m_hat = 13;
  double global_max_correlated_share = 14;
  uint64 global_max_drift_alerts_24h = 15;

  // Domain/category rules.
  repeated AutonomyRule rules = 20;
}

message AuthenticationRule {
  string rule_id = 1;
  string category = 2;
  openagents.common.v1.FeedbackLatencyClass tfb_class = 3;
  openagents.common.v1.SeverityClass severity = 4;
  ActorRole role = 5;

  openagents.common.v1.AuthAssuranceLevel min_auth_assurance = 10;
  bool require_personhood = 11;
}

message MonitoringRule {
  string rule_id = 1;
  string category = 2;
  openagents.common.v1.FeedbackLatencyClass tfb_class = 3;
  openagents.common.v1.SeverityClass severity = 4;

  repeated string required_detectors = 10;
  uint64 drift_alert_threshold_24h = 11;
  repeated AutonomyAction threshold_actions = 12; // deterministic action order applies
}

message RiskPricingRule {
  string rule_id = 1;
  string category = 2;
  openagents.common.v1.FeedbackLatencyClass tfb_class = 3;
  openagents.common.v1.SeverityClass severity = 4;

  uint32 base_liability_premium_bps = 10;
  double xa_multiplier = 11;
  double drift_multiplier = 12;
  double correlated_share_multiplier = 13;

  double min_capital_coverage_ratio = 20;
}

message CertificationRule {
  string rule_id = 1;
  string category = 2;
  openagents.common.v1.FeedbackLatencyClass tfb_class = 3;
  openagents.common.v1.SeverityClass severity = 4;

  bool require_certification = 10;
  repeated string accepted_certification_levels = 11;
  bool enable_safe_harbor_relaxations = 12;
}

message RollbackRule {
  string rule_id = 1;
  string category = 2;
  openagents.common.v1.FeedbackLatencyClass tfb_class = 3;
  openagents.common.v1.SeverityClass severity = 4;

  bool require_rollback_plan = 10;
  bool allow_compensating_action_only = 11;
}

message PolicyBundle {
  string policy_bundle_id = 1;
  string name = 2;
  string description = 3;

  int64 created_at_ms = 10;
  string version = 11;

  // Default static requirements
  repeated TierRule tier_rules = 20;
  repeated ProvenanceRule provenance_rules = 21;

  // Capital rules
  repeated EnvelopeRule envelope_rules = 30;
  repeated BondRule bond_rules = 31;

  // Identity / monitoring / pricing / certification controls
  repeated AuthenticationRule authentication_rules = 32;
  repeated MonitoringRule monitoring_rules = 33;
  repeated RiskPricingRule risk_pricing_rules = 34;
  repeated CertificationRule certification_rules = 35;
  repeated RollbackRule rollback_rules = 36;

  // Dynamic gating
  AutonomyThrottlePolicy autonomy = 40;

  // Non-normative metadata
  map<string, string> tags = 50;
}
```

### 2.3 Policy semantics (how services must apply it)

This is the “expectations” part: how the kernel must interpret the policy bundle.

#### A) Matching rules

Every rule uses `(category, tfb_class, severity)` matching with wildcards. Matching precedence MUST be deterministic:

1. exact category + exact tfb + exact severity
2. exact category + exact severity + wildcard tfb
3. exact category + wildcard severity + wildcard tfb
4. wildcard category + exact tfb/severity
5. wildcard everything (global default)

If multiple rules match at the same precedence, the kernel MUST choose deterministically (e.g., lexicographic by rule id) and include that rule id in hash-bound receipt evidence and/or hash-bound receipt payload (not only non-normative tags).

#### B) Verification gating

When creating a Contract or starting Verification, Aegis MUST:

* compute the required tier and independence requirement from `tier_rules`
* reject (or downgrade) any plan that violates policy
* record the required tier + independence requirement in the contract terms hash

When finalizing a verdict, Aegis MUST:

* compute whether the `IndependenceReport` violates policy
* if it violates, the verdict may still be recorded, but settlement MUST be **WITHHELD** unless policy explicitly allows it
* emit receipts with `hints.verification_correlated` and `hints.achieved_verification_tier`

#### C) Provenance gating

When accepting submission, Aegis MUST:

* require `provenance_bundle` evidence if required by the policy or plan
* compute `Pgrade` deterministically
* if `Pgrade` is below min grade for the WorkUnit’s slice, settlement and/or warranty issuance MUST be withheld or denied per policy.

#### D) Autonomy throttle (paper implementation)

The autonomy throttle is the runtime bridge from the paper’s variables to behavior.

* The EconomySnapshot job computes `sv`, `sv_effective`, `xa_hat`, `delta_m_hat`, correlated share, and drift alert pressure per slice.
* TreasuryRouter and/or the kernel services MUST read the latest snapshot.
* When conditions match, actions apply in a deterministic order:

  1. mode changes (NORMAL → DEGRADED → APPROVAL_REQUIRED → HALT)
  2. raising verification tier / requiring human step
  3. raising provenance grade
  4. tightening envelope limits / halting envelopes
  5. disabling warranties or capping coverage

All triggered actions MUST be:

* recorded in a breaker/throttle state object
* included in receipts as policy notes
* bound to the exact `snapshot_id`/`snapshot_hash`
* surfaced in `/stats` (“autonomy mode” and breaker states)

#### E) Envelope enforcement

Hydra MUST enforce envelope rules using:

* WorkUnit category/severity from trace links (WorkUnit referenced by Contract)
* current autonomy mode and throttle actions
* latest EconomySnapshot values

If policy says “halt_new_envelopes” for a category, Hydra MUST respond `WITHHELD` and emit a receipt with the exact reason and snapshot id.

#### F) Authentication/personhood enforcement

For actions with an `ActorRole` and category slice:

* the selected `AuthenticationRule` MUST be deterministic using the same precedence rules.
* kernel services MUST compare `ReceiptHints.auth_assurance_level` and `personhood_proved` against policy.
* if insufficient, action MUST be denied/withheld with stable reason code `AUTH_ASSURANCE_INSUFFICIENT`, plus hash-bound policy decision evidence.

#### G) Monitoring/drift enforcement

When drift signals breach `MonitoringRule.drift_alert_threshold_24h`:

* services MUST emit deterministic drift receipts:
  * `economy.drift.signal_emitted.v1`
  * `economy.drift.alert_raised.v1`
  * `economy.drift.false_positive_confirmed.v1`
* idempotency MUST be keyed by `(detector_id + snapshot window + receipt_type)`,
* then apply `threshold_actions` in the same deterministic autonomy action order,
* and include triggering detector ids in hash-bound receipt evidence.

#### H) Risk pricing enforcement

For contract creation/update:

* `RiskPricingRule` MUST compute liability premium deterministically from snapshot values and configured multipliers.
* contract and settlement receipts MUST separate `execution_price`, `verification_fee`, and `liability_premium`.
* receipts MUST include `pricing_snapshot_ref` so underwriters can reproduce pricing inputs.

#### I) Certification and rollback enforcement

For high-severity or irreversible slices:

* `CertificationRule.require_certification=true` MUST block uncertified actions (`WITHHELD` / border-block reason code).
* `RollbackRule.require_rollback_plan=true` MUST require rollback or compensating-action plan references before authority mutation.
* safe-harbor relaxations are allowed only when certification validity + observed risk metrics satisfy policy.

---
