use crate::app_state::{
    ActiveJobRecord, JobDemandSource, JobHistoryReceiptRow, JobHistoryStatus, JobLifecycleStage,
    PaneLoadState,
};
use crate::economy_kernel_receipts::{
    Asset, AuthAssuranceLevel, DriftSignalSummary, EvidenceRef, FeedbackLatencyClass, Money,
    MoneyAmount, PolicyContext, ProvenanceAttestationKind, ProvenanceGrade, Receipt,
    ReceiptBuilder, ReceiptHints, SeverityClass, TraceContext, VerificationTier,
};
use crate::state::job_inbox::{JobInboxNetworkRequest, JobInboxRequest};
use bitcoin::hashes::{Hash, sha256};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::path::{Path, PathBuf};

const EARN_KERNEL_RECEIPT_SCHEMA_VERSION: u16 = 1;
const EARN_KERNEL_RECEIPT_STREAM_ID: &str = "stream.earn_kernel_receipts.v1";
const EARN_KERNEL_RECEIPT_AUTHORITY: &str = "kernel.authority";
const EARN_KERNEL_RECEIPT_ROW_LIMIT: usize = 2048;
const EARN_WORK_UNIT_METADATA_ROW_LIMIT: usize = 2048;
const EARN_IDEMPOTENCY_RECORD_ROW_LIMIT: usize = 4096;
const INCIDENT_OBJECT_ROW_LIMIT: usize = 4096;
const INCIDENT_TAXONOMY_ROW_LIMIT: usize = 1024;
const OUTCOME_REGISTRY_ROW_LIMIT: usize = 4096;
const CERTIFICATION_OBJECT_ROW_LIMIT: usize = 4096;
const SIMULATION_SCENARIO_ROW_LIMIT: usize = 8192;
const SAFETY_SIGNAL_ROW_LIMIT: usize = 8192;
const SAFETY_SIGNAL_BUCKET_ROW_LIMIT: usize = 2048;
const AUDIT_LINKAGE_ROW_LIMIT: usize = 65_536;
const REASON_CODE_JOB_FAILED: &str = "JOB_FAILED";
const REASON_CODE_POLICY_PREFLIGHT_REJECTED: &str = "POLICY_PREFLIGHT_REJECTED";
const REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE: &str = "PAYMENT_POINTER_NON_AUTHORITATIVE";
const REASON_CODE_AUTH_ASSURANCE_INSUFFICIENT: &str = "AUTH_ASSURANCE_INSUFFICIENT";
const REASON_CODE_PROVENANCE_REQUIREMENTS_UNMET: &str = "PROVENANCE_REQUIREMENTS_UNMET";
const REASON_CODE_ROLLBACK_PLAN_REQUIRED: &str = "ROLLBACK_PLAN_REQUIRED";
const REASON_CODE_CERTIFICATION_REQUIRED: &str = "CERTIFICATION_REQUIRED";
const REASON_CODE_DIGITAL_BORDER_BLOCK_UNCERTIFIED: &str = "DIGITAL_BORDER_BLOCK_UNCERTIFIED";
const REASON_CODE_IDEMPOTENCY_CONFLICT: &str = "IDEMPOTENCY_CONFLICT";
const REASON_CODE_POLICY_THROTTLE_TRIGGERED: &str = "POLICY_THROTTLE_TRIGGERED";
const REASON_CODE_INCIDENT_REPORTED: &str = "INCIDENT_REPORTED";
const REASON_CODE_INCIDENT_UPDATED: &str = "INCIDENT_UPDATED";
const REASON_CODE_INCIDENT_RESOLVED: &str = "INCIDENT_RESOLVED";
const REASON_CODE_ROLLBACK_EXECUTED: &str = "ROLLBACK_EXECUTED";
const REASON_CODE_ROLLBACK_FAILED: &str = "ROLLBACK_FAILED";
const REASON_CODE_COMPENSATING_ACTION_EXECUTED: &str = "COMPENSATING_ACTION_EXECUTED";
const REASON_CODE_DRIFT_SIGNAL_EMITTED: &str = "DRIFT_SIGNAL_EMITTED";
const REASON_CODE_DRIFT_ALERT_RAISED: &str = "DRIFT_ALERT_RAISED";
const REASON_CODE_DRIFT_FALSE_POSITIVE_CONFIRMED: &str = "DRIFT_FALSE_POSITIVE_CONFIRMED";
const REASON_CODE_OUTCOME_REGISTRY_CREATED: &str = "OUTCOME_REGISTRY_CREATED";
const REASON_CODE_OUTCOME_REGISTRY_UPDATED: &str = "OUTCOME_REGISTRY_UPDATED";
const REASON_CODE_SIMULATION_SCENARIO_EXPORTED: &str = "SIMULATION_SCENARIO_EXPORTED";
const REASON_CODE_REDACTION_POLICY_APPLIED: &str = "REDACTION_POLICY_APPLIED";
const REASON_CODE_ANCHOR_PUBLISHED: &str = "ANCHOR_PUBLISHED";
const REASON_CODE_WORK_UNIT_TEMPLATE_REGISTERED: &str = "WORK_UNIT_TEMPLATE_REGISTERED";
const REASON_CODE_BOUNTY_DISPUTE_OPENED: &str = "BOUNTY_DISPUTE_OPENED";
const REASON_CODE_BOUNTY_SETTLEMENT_FINALIZED: &str = "BOUNTY_SETTLEMENT_FINALIZED";
const REASON_CODE_BOUNTY_SETTLEMENT_WITHHELD: &str = "BOUNTY_SETTLEMENT_WITHHELD";
const DEFAULT_PRICING_SNAPSHOT_ID: &str = "snapshot.economy:unavailable";
const DEFAULT_PRICING_SNAPSHOT_HASH: &str = "sha256:unavailable";
const DRIFT_WINDOW_MS: i64 = 86_400_000;
const SAFE_HARBOR_LIABILITY_PREMIUM_DISCOUNT_NUMERATOR: u64 = 80;
const SAFE_HARBOR_LIABILITY_PREMIUM_DISCOUNT_DENOMINATOR: u64 = 100;

#[derive(Clone)]
struct PolicyDecision {
    rule_id: String,
    decision: &'static str,
    notes: String,
}

#[derive(Clone, Copy)]
struct PolicyRule {
    rule_id: &'static str,
    decision: &'static str,
    action: Option<&'static str>,
    category: Option<&'static str>,
    severity: Option<SeverityClass>,
    reason_code: Option<&'static str>,
    note: &'static str,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct PolicySliceRule {
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    tfb_class: Option<FeedbackLatencyClass>,
    #[serde(default)]
    severity: Option<SeverityClass>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct AuthenticationPolicyRule {
    rule_id: String,
    #[serde(flatten)]
    slice: PolicySliceRule,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    min_auth_assurance: Option<String>,
    #[serde(default)]
    require_personhood: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct ProvenancePolicyRule {
    rule_id: String,
    #[serde(flatten)]
    slice: PolicySliceRule,
    #[serde(default)]
    min_grade: Option<ProvenanceGrade>,
    #[serde(default)]
    require_provenance_bundle: bool,
    #[serde(default)]
    require_permissioning_refs: bool,
    #[serde(default)]
    required_attestation_kinds: Vec<ProvenanceAttestationKind>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[serde(rename_all = "snake_case")]
enum ThrottleActionKind {
    SetModeDegraded,
    SetModeApprovalRequired,
    SetModeHalt,
    RaiseRequiredTier,
    RequireHumanStep,
    RaiseProvenanceGrade,
    TightenEnvelope,
    HaltNewEnvelopes,
    DisableWarranties,
    CapWarrantyCoverage,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct MonitoringPolicyRule {
    rule_id: String,
    #[serde(flatten)]
    slice: PolicySliceRule,
    #[serde(default)]
    required_detectors: Vec<String>,
    #[serde(default)]
    drift_alert_threshold_24h: Option<u64>,
    #[serde(default)]
    actions: Vec<ThrottleActionKind>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct RiskPricingPolicyRule {
    rule_id: String,
    #[serde(flatten)]
    slice: PolicySliceRule,
    #[serde(default)]
    base_liability_premium_bps: Option<u32>,
    #[serde(default)]
    xa_multiplier: Option<f64>,
    #[serde(default)]
    drift_multiplier: Option<f64>,
    #[serde(default)]
    correlated_share_multiplier: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct CertificationPolicyRule {
    rule_id: String,
    #[serde(flatten)]
    slice: PolicySliceRule,
    #[serde(default)]
    require_certification: bool,
    #[serde(default)]
    accepted_levels: Vec<String>,
    #[serde(default)]
    enable_safe_harbor_relaxations: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct RollbackPolicyRule {
    rule_id: String,
    #[serde(flatten)]
    slice: PolicySliceRule,
    #[serde(default)]
    require_rollback_plan: bool,
    #[serde(default)]
    allow_compensating_action_only: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct AutonomyPolicyRule {
    rule_id: String,
    #[serde(flatten)]
    slice: PolicySliceRule,
    #[serde(default)]
    min_sv: Option<f64>,
    #[serde(default)]
    min_sv_effective: Option<f64>,
    #[serde(default)]
    max_xa_hat: Option<f64>,
    #[serde(default)]
    max_delta_m_hat: Option<f64>,
    #[serde(default)]
    max_correlated_share: Option<f64>,
    #[serde(default)]
    max_drift_alerts_24h: Option<u64>,
    #[serde(default)]
    actions: Vec<ThrottleActionKind>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct AnchoringPolicyConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    allowed_backends: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct PolicyBundleConfig {
    #[serde(default)]
    authentication_rules: Vec<AuthenticationPolicyRule>,
    #[serde(default)]
    provenance_rules: Vec<ProvenancePolicyRule>,
    #[serde(default)]
    monitoring_rules: Vec<MonitoringPolicyRule>,
    #[serde(default)]
    risk_pricing_rules: Vec<RiskPricingPolicyRule>,
    #[serde(default)]
    certification_rules: Vec<CertificationPolicyRule>,
    #[serde(default)]
    rollback_rules: Vec<RollbackPolicyRule>,
    #[serde(default)]
    autonomy_rules: Vec<AutonomyPolicyRule>,
    #[serde(default)]
    anchoring: AnchoringPolicyConfig,
}

#[derive(Clone, Copy, Default)]
struct SnapshotPolicyMetrics {
    sv: f64,
    sv_effective: f64,
    rho_effective: f64,
    xa_hat: f64,
    delta_m_hat: f64,
    correlated_verification_share: f64,
    drift_alerts_24h: u64,
}

#[derive(Clone)]
struct PricingSnapshotContext {
    snapshot_id: String,
    snapshot_hash: String,
    metrics: SnapshotPolicyMetrics,
}

#[derive(Clone)]
struct LiabilityPricingBreakdown {
    execution_price_sats: u64,
    verification_fee_sats: Option<u64>,
    liability_premium_sats: u64,
    risk_charge_sats: u64,
    effective_liability_premium_bps: u32,
    pricing_snapshot_id: String,
    pricing_snapshot_hash: String,
    pricing_metrics: SnapshotPolicyMetrics,
    risk_pricing_rule_id: String,
    base_liability_premium_bps: u32,
    xa_multiplier: f64,
    drift_multiplier: f64,
    correlated_share_multiplier: f64,
    safe_harbor_relaxation_applied: bool,
    safe_harbor_discount_numerator: u64,
    safe_harbor_discount_denominator: u64,
    certification_id: Option<String>,
    certification_level: Option<String>,
}

#[derive(Clone)]
struct TriggeredPolicyAction {
    rule_id: String,
    rule_kind: &'static str,
    action: ThrottleActionKind,
    notes: String,
}

#[derive(Clone, Default)]
struct ProvenanceFeatures {
    has_provenance_bundle: bool,
    data_source_ref_count: u64,
    permissioning_ref_count: u64,
    attestation_kinds: BTreeSet<ProvenanceAttestationKind>,
}

#[derive(Clone)]
struct CertificationGateContext {
    decision: PolicyDecision,
    certification: SafetyCertification,
    safe_harbor_relaxation_applied: bool,
}

#[derive(Clone)]
struct CertificationGateFailure {
    decision: PolicyDecision,
    reason_code: &'static str,
}

#[derive(Debug, Serialize, Deserialize)]
struct EarnKernelReceiptDocumentV1 {
    schema_version: u16,
    stream_id: String,
    authority: String,
    #[serde(default)]
    receipts: Vec<Receipt>,
    #[serde(default)]
    work_units: Vec<WorkUnitMetadata>,
    #[serde(default)]
    idempotency_records: Vec<IdempotencyRecord>,
    #[serde(default)]
    incident_objects: Vec<IncidentObject>,
    #[serde(default)]
    outcome_registry_entries: Vec<OutcomeRegistryEntry>,
    #[serde(default)]
    incident_taxonomy_registry: Vec<IncidentTaxonomyEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct WorkUnitMetadata {
    pub work_unit_id: String,
    pub category: String,
    pub tfb_class: FeedbackLatencyClass,
    pub severity: SeverityClass,
    pub verification_budget_hint_sats: u64,
    #[serde(default)]
    pub template_kind: Option<WhiteHatWorkUnitKind>,
    #[serde(default)]
    pub acceptance_criteria_ref: Option<String>,
    #[serde(default)]
    pub coordinated_disclosure_ref: Option<String>,
    #[serde(default)]
    pub mandatory_provenance: bool,
    #[serde(default)]
    pub rollback_plan_ref: Option<String>,
    #[serde(default)]
    pub compensating_action_plan_ref: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum WhiteHatWorkUnitKind {
    Audit,
    Redteam,
    IncidentRepro,
}

impl WhiteHatWorkUnitKind {
    fn label(self) -> &'static str {
        match self {
            WhiteHatWorkUnitKind::Audit => "audit",
            WhiteHatWorkUnitKind::Redteam => "redteam",
            WhiteHatWorkUnitKind::IncidentRepro => "incident_repro",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct WhiteHatWorkUnitTemplateDraft {
    pub work_unit_id: String,
    pub idempotency_key: String,
    pub kind: WhiteHatWorkUnitKind,
    pub tfb_class: FeedbackLatencyClass,
    pub severity: SeverityClass,
    pub acceptance_criteria_ref: String,
    pub coordinated_disclosure_ref: String,
    #[serde(default)]
    pub mandatory_provenance: bool,
    #[serde(default)]
    pub verification_budget_hint_sats: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct WhiteHatBountySettlementDraft {
    pub work_unit_id: String,
    pub idempotency_key: String,
    pub finding_id: String,
    pub verdict_receipt_id: String,
    pub payout_sats: u64,
    pub dispute_bond_sats: u64,
    #[serde(default)]
    pub disputed: bool,
    #[serde(default)]
    pub dispute_reason: Option<String>,
    #[serde(default)]
    pub escrow_receipt_id: Option<String>,
    #[serde(default)]
    pub incident_report: Option<IncidentReportDraft>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct WhiteHatBountySettlementResult {
    pub settlement_receipt_id: String,
    pub dispute_receipt_id: Option<String>,
    pub outcome_entry_id: String,
    pub incident_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum RollbackReceiptType {
    RollbackExecuted,
    RollbackFailed,
    CompensatingActionExecuted,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct RollbackActionDraft {
    pub work_unit_id: String,
    pub idempotency_key: String,
    pub rollback_receipt_type: RollbackReceiptType,
    pub incident_id: Option<String>,
    pub linked_receipt_ids: Vec<String>,
    pub reason_code: Option<String>,
    pub summary: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum IncidentKind {
    IncidentKindUnspecified,
    Incident,
    NearMiss,
    GroundTruthCase,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum IncidentStatus {
    IncidentStatusUnspecified,
    Open,
    Resolved,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum IncidentExportRedactionTier {
    Public,
    Restricted,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum AuditExportRedactionTier {
    Public,
    Restricted,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum SimulationScenarioExportRedactionTier {
    Public,
    Restricted,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum SafetySignalExportMode {
    PublicAggregate,
    RestrictedFeed,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SafetySignal {
    pub signal_id: String,
    pub signal_digest: String,
    pub source_receipt_id: String,
    pub source_receipt_type: String,
    pub signal_class: String,
    pub source_kind: String,
    pub taxonomy_code: String,
    pub signal_code: String,
    pub category: String,
    pub tfb_class: FeedbackLatencyClass,
    pub severity: SeverityClass,
    pub hashed_indicators: Vec<String>,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SafetySignalBucketRow {
    pub taxonomy_code: String,
    pub severity: SeverityClass,
    pub signal_count: u64,
    pub incident_signal_count: u64,
    pub drift_signal_count: u64,
    pub adverse_signal_count: u64,
    pub signal_rate: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SafetySignalFeed {
    pub schema_version: u16,
    pub generated_at_ms: i64,
    pub stream_id: String,
    pub authority: String,
    pub export_mode: SafetySignalExportMode,
    pub query: ReceiptQuery,
    pub signal_count: usize,
    pub bucket_count: usize,
    pub signals: Vec<SafetySignal>,
    pub buckets: Vec<SafetySignalBucketRow>,
    pub package_hash: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum CertificationState {
    CertificationStateUnspecified,
    Active,
    Revoked,
    Expired,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct CertificationScope {
    pub category: String,
    pub tfb_class: FeedbackLatencyClass,
    pub min_severity: SeverityClass,
    pub max_severity: SeverityClass,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SafetyCertification {
    pub certification_id: String,
    pub certification_digest: String,
    pub revision: u32,
    pub state: CertificationState,
    pub certification_level: String,
    pub scope: Vec<CertificationScope>,
    pub valid_from_ms: i64,
    pub valid_until_ms: i64,
    pub issuer_credential_kind: String,
    pub issuer_credential_digest: String,
    pub required_evidence_digests: Vec<String>,
    pub linked_receipt_ids: Vec<String>,
    pub issued_at_ms: i64,
    pub updated_at_ms: i64,
    pub revoked_reason_code: Option<String>,
    pub policy_bundle_id: String,
    pub policy_version: String,
    pub supersedes_digest: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SafetyCertificationDraft {
    pub certification_id: String,
    pub idempotency_key: String,
    pub certification_level: String,
    pub scope: Vec<CertificationScope>,
    pub valid_from_ms: i64,
    pub valid_until_ms: i64,
    pub issuer_identity: String,
    #[serde(default)]
    pub issuer_auth_assurance_level: Option<AuthAssuranceLevel>,
    #[serde(default)]
    pub required_evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub linked_receipt_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SafetyCertificationRevocationDraft {
    pub certification_id: String,
    pub idempotency_key: String,
    pub reason_code: String,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct IncidentTaxonomyEntry {
    pub taxonomy_id: String,
    pub taxonomy_version: String,
    pub code: String,
    pub stable_meaning: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct IncidentObject {
    pub incident_id: String,
    pub incident_digest: String,
    pub revision: u32,
    pub incident_kind: IncidentKind,
    pub incident_status: IncidentStatus,
    pub taxonomy_id: String,
    pub taxonomy_version: String,
    pub taxonomy_code: String,
    pub severity: SeverityClass,
    pub summary: String,
    pub reported_at_ms: i64,
    pub updated_at_ms: i64,
    pub policy_bundle_id: String,
    pub policy_version: String,
    pub linked_receipt_ids: Vec<String>,
    pub rollback_receipt_ids: Vec<String>,
    pub evidence_digests: Vec<String>,
    pub supersedes_digest: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct IncidentReportDraft {
    pub idempotency_key: String,
    pub incident_kind: IncidentKind,
    pub taxonomy_id: String,
    pub taxonomy_version: String,
    pub taxonomy_code: String,
    pub severity: SeverityClass,
    pub summary: String,
    pub linked_receipt_ids: Vec<String>,
    pub rollback_receipt_ids: Vec<String>,
    pub evidence_digests: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct IncidentUpdateDraft {
    pub incident_id: String,
    pub idempotency_key: String,
    pub summary: Option<String>,
    pub linked_receipt_ids: Vec<String>,
    pub rollback_receipt_ids: Vec<String>,
    pub evidence_digests: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct IncidentResolutionDraft {
    pub incident_id: String,
    pub idempotency_key: String,
    pub resolution_summary: Option<String>,
    pub rollback_receipt_ids: Vec<String>,
    pub evidence_digests: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct OutcomeRegistryEntry {
    pub entry_id: String,
    pub entry_digest: String,
    pub revision: u32,
    pub category: String,
    pub tfb_class: FeedbackLatencyClass,
    pub severity: SeverityClass,
    pub verdict_outcome: String,
    pub settlement_outcome: String,
    pub claim_outcome: Option<String>,
    pub remedy_outcome: Option<String>,
    pub incident_tags: Vec<String>,
    pub linked_receipt_ids: Vec<String>,
    pub evidence_digests: Vec<String>,
    pub reported_at_ms: i64,
    pub updated_at_ms: i64,
    pub policy_bundle_id: String,
    pub policy_version: String,
    pub supersedes_digest: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct OutcomeRegistryEntryDraft {
    pub idempotency_key: String,
    pub category: String,
    pub tfb_class: FeedbackLatencyClass,
    pub severity: SeverityClass,
    pub verdict_outcome: String,
    pub settlement_outcome: String,
    pub claim_outcome: Option<String>,
    pub remedy_outcome: Option<String>,
    pub incident_tags: Vec<String>,
    pub linked_receipt_ids: Vec<String>,
    pub evidence_digests: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct OutcomeRegistryUpdateDraft {
    pub entry_id: String,
    pub idempotency_key: String,
    pub verdict_outcome: Option<String>,
    pub settlement_outcome: Option<String>,
    pub claim_outcome: Option<String>,
    pub remedy_outcome: Option<String>,
    pub incident_tags: Vec<String>,
    pub linked_receipt_ids: Vec<String>,
    pub evidence_digests: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct IncidentAuditPackage {
    pub schema_version: u16,
    pub generated_at_ms: i64,
    pub stream_id: String,
    pub authority: String,
    pub redaction_tier: IncidentExportRedactionTier,
    pub incident_count: usize,
    pub taxonomy_count: usize,
    pub incidents: Vec<IncidentObject>,
    pub taxonomy_registry: Vec<IncidentTaxonomyEntry>,
    pub incident_receipts: Vec<Receipt>,
    pub package_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SimulationScenario {
    pub scenario_id: String,
    pub scenario_digest: String,
    pub ground_truth_case_id: String,
    pub ground_truth_case_digest: String,
    pub taxonomy_id: String,
    pub taxonomy_version: String,
    pub taxonomy_code: String,
    pub severity: SeverityClass,
    pub evidence_digests: Vec<String>,
    pub linked_receipt_ids: Vec<String>,
    pub linked_receipt_digests: Vec<String>,
    pub rollback_receipt_ids: Vec<String>,
    pub rollback_receipt_digests: Vec<String>,
    pub harness_ref: EvidenceRef,
    pub scoring_rubric_ref: EvidenceRef,
    pub derived_from_receipt_ids: Vec<String>,
    pub derived_from_receipt_digests: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SimulationScenarioPackage {
    pub schema_version: u16,
    pub generated_at_ms: i64,
    pub stream_id: String,
    pub authority: String,
    pub redaction_tier: SimulationScenarioExportRedactionTier,
    pub redaction_policy_receipt_id: String,
    pub export_receipt_id: String,
    pub scenario_count: usize,
    pub scenarios: Vec<SimulationScenario>,
    pub package_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct AnchorPublicationDraft {
    pub snapshot_id: String,
    pub snapshot_hash: String,
    pub anchor_backend: String,
    pub external_anchor_reference: String,
    #[serde(default)]
    pub receipt_root_hash: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct AuditAnchorEntry {
    pub receipt_id: String,
    pub anchor_backend: String,
    pub snapshot_id: String,
    pub snapshot_hash: String,
    pub receipt_root_hash: Option<String>,
    pub anchor_proof_ref: EvidenceRef,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct AuditCertificationEntry {
    pub receipt_id: String,
    pub receipt_type: String,
    pub category: Option<String>,
    pub severity: Option<SeverityClass>,
    pub digest: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct AuditCertificationObject {
    pub certification_id: String,
    pub certification_digest: String,
    pub revision: u32,
    pub state: CertificationState,
    pub certification_level: String,
    pub scope: Vec<CertificationScope>,
    pub valid_from_ms: i64,
    pub valid_until_ms: i64,
    pub issuer_credential_kind: String,
    pub issuer_credential_digest: String,
    pub required_evidence_digests: Vec<String>,
    pub linked_receipt_ids: Vec<String>,
    pub revoked_reason_code: Option<String>,
    pub policy_bundle_id: String,
    pub policy_version: String,
    pub supersedes_digest: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct AuditOutcomeRegistryEntry {
    pub receipt_id: String,
    pub receipt_type: String,
    pub category: Option<String>,
    pub tfb_class: Option<FeedbackLatencyClass>,
    pub severity: Option<SeverityClass>,
    pub digest: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct AuditOutcomeRegistryObject {
    pub entry_id: String,
    pub entry_digest: String,
    pub revision: u32,
    pub category: String,
    pub tfb_class: FeedbackLatencyClass,
    pub severity: SeverityClass,
    pub verdict_outcome: String,
    pub settlement_outcome: String,
    pub claim_outcome: Option<String>,
    pub remedy_outcome: Option<String>,
    pub incident_tags: Vec<String>,
    pub linked_receipt_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct AuditSnapshotBinding {
    pub snapshot_id: String,
    pub snapshot_hash: String,
    pub linked_receipt_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct AuditLinkageEdge {
    pub from_receipt_id: String,
    pub to_receipt_id: String,
    pub relation_kind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct AuditPackage {
    pub schema_version: u16,
    pub generated_at_ms: i64,
    pub stream_id: String,
    pub authority: String,
    pub redaction_tier: AuditExportRedactionTier,
    pub query: ReceiptQuery,
    pub receipt_count: usize,
    pub incident_count: usize,
    pub certification_count: usize,
    pub certification_object_count: usize,
    pub anchor_count: usize,
    pub outcome_registry_count: usize,
    pub outcome_registry_object_count: usize,
    pub snapshot_binding_count: usize,
    pub linkage_edge_count: usize,
    pub receipts: Vec<Receipt>,
    pub incidents: Vec<IncidentObject>,
    pub certifications: Vec<AuditCertificationEntry>,
    pub certification_objects: Vec<AuditCertificationObject>,
    pub anchors: Vec<AuditAnchorEntry>,
    pub outcome_registry_entries: Vec<AuditOutcomeRegistryEntry>,
    pub outcome_registry_objects: Vec<AuditOutcomeRegistryObject>,
    pub snapshot_bindings: Vec<AuditSnapshotBinding>,
    pub linkage_edges: Vec<AuditLinkageEdge>,
    pub package_hash: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct ReceiptQuery {
    pub start_inclusive_ms: Option<i64>,
    pub end_inclusive_ms: Option<i64>,
    pub work_unit_id: Option<String>,
    pub receipt_type: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ReceiptBundle {
    pub schema_version: u16,
    pub generated_at_ms: i64,
    pub stream_id: String,
    pub authority: String,
    pub query: ReceiptQuery,
    pub receipt_count: usize,
    pub receipt_ids: Vec<String>,
    pub bundle_hash: String,
    pub receipts: Vec<Receipt>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct IdempotencyRecord {
    pub scope: String,
    pub idempotency_key: String,
    pub inputs_hash: String,
    pub receipt_id: String,
    pub receipt_type: String,
    pub canonical_hash: String,
    pub created_at_ms: i64,
}

#[derive(Clone)]
struct ResolvedWorkMetadata {
    category: String,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
    verification_budget_hint_sats: u64,
    template_kind: Option<WhiteHatWorkUnitKind>,
    acceptance_criteria_ref: Option<String>,
    coordinated_disclosure_ref: Option<String>,
    mandatory_provenance: bool,
    rollback_plan_ref: Option<String>,
    compensating_action_plan_ref: Option<String>,
}

struct LoadedReceiptState {
    receipts: Vec<Receipt>,
    work_units: BTreeMap<String, WorkUnitMetadata>,
    idempotency_index: BTreeMap<String, IdempotencyRecord>,
    incident_objects: Vec<IncidentObject>,
    outcome_registry_entries: Vec<OutcomeRegistryEntry>,
    incident_taxonomy_registry: BTreeMap<String, IncidentTaxonomyEntry>,
}

pub struct EarnKernelReceiptState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub stream_id: String,
    pub authority: String,
    pub receipts: Vec<Receipt>,
    pub work_units: BTreeMap<String, WorkUnitMetadata>,
    pub idempotency_index: BTreeMap<String, IdempotencyRecord>,
    pub incident_objects: Vec<IncidentObject>,
    pub outcome_registry_entries: Vec<OutcomeRegistryEntry>,
    pub incident_taxonomy_registry: BTreeMap<String, IncidentTaxonomyEntry>,
    receipt_file_path: PathBuf,
}

impl Default for EarnKernelReceiptState {
    fn default() -> Self {
        let receipt_file_path = earn_kernel_receipts_file_path();
        Self::from_receipt_file_path(receipt_file_path)
    }
}

impl EarnKernelReceiptState {
    pub fn apply_authoritative_receipt(&mut self, receipt: Receipt, source_tag: &str) {
        self.append_receipt(Ok(receipt), source_tag);
    }

    fn from_receipt_file_path(receipt_file_path: PathBuf) -> Self {
        let (loaded, load_state, last_error, last_action) =
            match load_earn_kernel_receipts(receipt_file_path.as_path()) {
                Ok(loaded) => (
                    loaded,
                    PaneLoadState::Ready,
                    None,
                    Some("Loaded economy-kernel receipt stream".to_string()),
                ),
                Err(error) => (
                    LoadedReceiptState {
                        receipts: Vec::new(),
                        work_units: BTreeMap::new(),
                        idempotency_index: BTreeMap::new(),
                        incident_objects: Vec::new(),
                        outcome_registry_entries: Vec::new(),
                        incident_taxonomy_registry: default_incident_taxonomy_registry(),
                    },
                    PaneLoadState::Error,
                    Some(error),
                    Some("Economy-kernel receipt stream load failed".to_string()),
                ),
            };
        Self {
            load_state,
            last_error,
            last_action,
            stream_id: EARN_KERNEL_RECEIPT_STREAM_ID.to_string(),
            authority: EARN_KERNEL_RECEIPT_AUTHORITY.to_string(),
            receipts: loaded.receipts,
            work_units: loaded.work_units,
            idempotency_index: loaded.idempotency_index,
            incident_objects: loaded.incident_objects,
            outcome_registry_entries: loaded.outcome_registry_entries,
            incident_taxonomy_registry: loaded.incident_taxonomy_registry,
            receipt_file_path,
        }
    }

    pub fn record_ingress_request(
        &mut self,
        request: &JobInboxNetworkRequest,
        occurred_at_epoch_seconds: u64,
        source_tag: &str,
    ) {
        let job_id = format!("job-{}", request.request_id);
        let metadata = self.resolve_or_create_work_unit_metadata(
            job_id.as_str(),
            request.demand_source,
            request.price_sats,
            Some(request.ttl_seconds),
        );
        let receipt_id = lifecycle_receipt_id(
            job_id.as_str(),
            JobLifecycleStage::Received,
            request.request_id.as_str(),
        );
        let mut evidence = vec![
            EvidenceRef::new(
                "nostr_request",
                format!("oa://nip90/request/{}", request.request_id),
                digest_for_text(request.request_id.as_str()),
            ),
            EvidenceRef::new(
                "request_shape",
                format!("oa://nip90/request/{}/shape", request.request_id),
                digest_for_text(
                    request
                        .parsed_event_shape
                        .as_deref()
                        .unwrap_or("shape:unknown"),
                ),
            ),
        ];
        if let Some(event_id) = request.sa_tick_request_event_id.as_deref() {
            evidence.push(EvidenceRef::new(
                "sa_tick_request_event",
                format!("oa://sa/tick/request/{event_id}"),
                digest_for_text(event_id),
            ));
        }

        let policy_decision =
            allow_policy_decision("ingress", metadata.category.as_str(), metadata.severity);

        let hints = ReceiptHints {
            category: Some(metadata.category.clone()),
            tfb_class: Some(metadata.tfb_class),
            severity: Some(metadata.severity),
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: None,
            auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
            personhood_proved: Some(false),
            reason_code: None,
            notional: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(request.price_sats),
            }),
            liability_premium: None,
        };
        evidence.push(policy_decision_evidence(&policy_decision));

        let receipt = ReceiptBuilder::new(
            receipt_id,
            "earn.job.ingress_request.v1",
            epoch_seconds_to_ms(occurred_at_epoch_seconds),
            lifecycle_idempotency_key(
                "ingress_request",
                job_id.as_str(),
                request.request_id.as_str(),
            ),
            trace_for_job(job_id.as_str(), Some(request.request_id.as_str()), None),
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "request_id": request.request_id,
            "request_kind": request.request_kind,
            "requester": request.requester,
            "demand_source": request.demand_source.label(),
            "capability": request.capability,
            "price_sats": request.price_sats,
            "ttl_seconds": request.ttl_seconds,
            "skill_scope_id": request.skill_scope_id,
            "ac_envelope_event_id": request.ac_envelope_event_id,
            "work_unit": work_unit_metadata_payload(job_id.as_str(), &metadata),
        }))
        .with_outputs_payload(json!({
            "stage": JobLifecycleStage::Received.label(),
            "source_tag": source_tag,
            "status": "accepted_for_inbox_projection",
            "work_unit": work_unit_metadata_payload(job_id.as_str(), &metadata),
            "policy_rule_id": policy_decision.rule_id,
            "policy_decision": policy_decision.decision,
            "policy_notes": policy_decision.notes,
        }))
        .with_evidence(evidence)
        .with_hints(hints)
        .build();

        self.append_receipt(receipt, source_tag);
    }

    pub fn record_network_preflight_rejection(
        &mut self,
        request: &JobInboxNetworkRequest,
        reason: &str,
        occurred_at_epoch_seconds: u64,
        source_tag: &str,
    ) {
        self.record_preflight_rejection_common(
            request.request_id.as_str(),
            request.requester.as_str(),
            request.demand_source,
            request.request_kind,
            request.capability.as_str(),
            request.price_sats,
            request.ttl_seconds,
            reason,
            occurred_at_epoch_seconds,
            source_tag,
            false,
        );
    }

    pub fn record_preflight_rejection(
        &mut self,
        request: &JobInboxRequest,
        reason: &str,
        occurred_at_epoch_seconds: u64,
        source_tag: &str,
    ) {
        self.record_preflight_rejection_common(
            request.request_id.as_str(),
            request.requester.as_str(),
            request.demand_source,
            request.request_kind,
            request.capability.as_str(),
            request.price_sats,
            request.ttl_seconds,
            reason,
            occurred_at_epoch_seconds,
            source_tag,
            true,
        );
    }

    fn record_preflight_rejection_common(
        &mut self,
        request_id: &str,
        requester: &str,
        demand_source: JobDemandSource,
        request_kind: u16,
        capability: &str,
        price_sats: u64,
        ttl_seconds: u64,
        reason: &str,
        occurred_at_epoch_seconds: u64,
        source_tag: &str,
        link_ingress_receipt: bool,
    ) {
        let job_id = format!("job-{request_id}");
        let metadata = self.resolve_or_create_work_unit_metadata(
            job_id.as_str(),
            demand_source,
            price_sats,
            Some(ttl_seconds),
        );
        let rule_action = "preflight_reject";
        let policy_decision = deny_policy_decision(
            rule_action,
            metadata.category.as_str(),
            metadata.severity,
            REASON_CODE_POLICY_PREFLIGHT_REJECTED,
        );
        let authority_key = format!("preflight-reject:{request_id}");

        let mut evidence = vec![
            EvidenceRef::new(
                "nostr_request",
                format!("oa://nip90/request/{request_id}"),
                digest_for_text(request_id),
            ),
            EvidenceRef::new(
                "preflight_reason",
                format!("oa://earn/jobs/{job_id}/preflight_reject"),
                digest_for_text(reason),
            ),
            policy_decision_evidence(&policy_decision),
        ];
        if link_ingress_receipt {
            let ingress_receipt_id =
                lifecycle_receipt_id(job_id.as_str(), JobLifecycleStage::Received, request_id);
            self.append_receipt_reference_links(&mut evidence, &[ingress_receipt_id]);
        }

        let receipt = ReceiptBuilder::new(
            lifecycle_receipt_id(
                job_id.as_str(),
                JobLifecycleStage::Received,
                authority_key.as_str(),
            ),
            "earn.job.preflight_rejected.v1",
            epoch_seconds_to_ms(occurred_at_epoch_seconds),
            lifecycle_idempotency_key("preflight_reject", job_id.as_str(), request_id),
            trace_for_job(job_id.as_str(), Some(request_id), None),
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "job_id": job_id,
            "request_id": request_id,
            "requester": requester,
            "demand_source": demand_source.label(),
            "request_kind": request_kind,
            "capability": capability,
            "price_sats": price_sats,
            "ttl_seconds": ttl_seconds,
            "preflight_reason": reason,
            "work_unit": work_unit_metadata_payload(job_id.as_str(), &metadata),
        }))
        .with_outputs_payload(json!({
            "stage": JobLifecycleStage::Received.label(),
            "source_tag": source_tag,
            "status": "denied",
            "reason_code": REASON_CODE_POLICY_PREFLIGHT_REJECTED,
            "work_unit": work_unit_metadata_payload(job_id.as_str(), &metadata),
            "policy_rule_id": policy_decision.rule_id,
            "policy_decision": policy_decision.decision,
            "policy_notes": policy_decision.notes,
        }))
        .with_evidence(evidence)
        .with_hints(ReceiptHints {
            category: Some(metadata.category.clone()),
            tfb_class: Some(metadata.tfb_class),
            severity: Some(metadata.severity),
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: None,
            auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
            personhood_proved: Some(false),
            reason_code: Some(REASON_CODE_POLICY_PREFLIGHT_REJECTED.to_string()),
            notional: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(price_sats),
            }),
            liability_premium: None,
        })
        .build();
        self.append_receipt(receipt, source_tag);
    }

    pub fn record_active_job_stage(
        &mut self,
        job: &ActiveJobRecord,
        stage: JobLifecycleStage,
        occurred_at_epoch_seconds: u64,
        source_tag: &str,
    ) {
        let mut authority_key = match stage {
            JobLifecycleStage::Received | JobLifecycleStage::Accepted => job.request_id.as_str(),
            JobLifecycleStage::Running => job
                .sa_tick_request_event_id
                .as_deref()
                .unwrap_or(job.request_id.as_str()),
            JobLifecycleStage::Delivered => job
                .sa_tick_result_event_id
                .as_deref()
                .unwrap_or(job.request_id.as_str()),
            JobLifecycleStage::Paid => job
                .payment_id
                .as_deref()
                .or(job.ac_settlement_event_id.as_deref())
                .unwrap_or(job.request_id.as_str()),
            JobLifecycleStage::Failed => job
                .ac_default_event_id
                .as_deref()
                .or(job.failure_reason.as_deref())
                .unwrap_or(job.request_id.as_str()),
        }
        .to_string();

        let payment_pointer = job
            .payment_id
            .as_deref()
            .or(job.invoice_id.as_deref())
            .unwrap_or("");
        let paid_pointer_authoritative =
            is_wallet_authoritative_payment_pointer(Some(payment_pointer));

        let metadata = self.resolve_or_create_work_unit_metadata(
            job.job_id.as_str(),
            job.demand_source,
            job.quoted_price_sats,
            None,
        );
        let auth_assurance = auth_assurance_for_identity(job.requester.as_str());
        let personhood_proved =
            personhood_proved_for_identity(job.requester.as_str(), auth_assurance);
        let policy_bundle = current_policy_bundle();
        let stage_certification_eval = if stage == JobLifecycleStage::Paid {
            Some(evaluate_certification_gate(
                &policy_bundle,
                self.receipts.as_slice(),
                metadata.category.as_str(),
                metadata.tfb_class,
                metadata.severity,
                epoch_seconds_to_ms(occurred_at_epoch_seconds),
            ))
        } else {
            None
        };
        let stage_certification_gate = stage_certification_eval
            .as_ref()
            .and_then(|evaluation| evaluation.as_ref().err().cloned());
        let stage_certification_context = stage_certification_eval
            .as_ref()
            .and_then(|evaluation| evaluation.as_ref().ok().cloned());
        let stage_auth_gate = if stage == JobLifecycleStage::Paid {
            evaluate_authentication_gate(
                &policy_bundle,
                metadata.category.as_str(),
                metadata.tfb_class,
                metadata.severity,
                "verifier",
                auth_assurance,
                personhood_proved,
            )
            .err()
        } else {
            None
        };
        let mut provenance_probe_evidence = Vec::<EvidenceRef>::new();
        append_provenance_evidence_for_job_stage(&mut provenance_probe_evidence, job, stage);
        let observed_provenance_grade = provenance_grade_from_features(
            &provenance_features_from_evidence(provenance_probe_evidence.as_slice()),
        );
        let stage_provenance_gate = if stage == JobLifecycleStage::Paid {
            evaluate_provenance_gate(
                &policy_bundle,
                metadata.category.as_str(),
                metadata.tfb_class,
                metadata.severity,
                provenance_probe_evidence.as_slice(),
            )
            .err()
        } else {
            None
        };
        let stage_rollback_gate = if stage == JobLifecycleStage::Paid {
            evaluate_rollback_gate(
                &policy_bundle,
                metadata.category.as_str(),
                metadata.tfb_class,
                metadata.severity,
                job.job_id.as_str(),
                &metadata,
            )
            .err()
        } else {
            None
        };
        let stage_pricing = if stage == JobLifecycleStage::Paid {
            Some(compute_liability_pricing_for_settlement(
                self.receipts.as_slice(),
                &policy_bundle,
                metadata.category.as_str(),
                metadata.tfb_class,
                metadata.severity,
                job.quoted_price_sats,
                metadata.verification_budget_hint_sats,
                stage_certification_context.as_ref(),
            ))
        } else {
            None
        };
        let (receipt_type, reason_code, status, policy_decision): (
            &'static str,
            Option<&'static str>,
            &'static str,
            PolicyDecision,
        ) = if stage == JobLifecycleStage::Paid && stage_certification_gate.is_some() {
            authority_key = format!("withheld-certification:{}", job.request_id);
            (
                "earn.job.withheld.v1",
                Some(
                    stage_certification_gate
                        .as_ref()
                        .map(|failure| failure.reason_code)
                        .expect("certification gate checked"),
                ),
                "withheld",
                stage_certification_gate
                    .as_ref()
                    .expect("certification gate checked")
                    .decision
                    .clone(),
            )
        } else if stage == JobLifecycleStage::Paid && stage_auth_gate.is_some() {
            authority_key = format!("withheld-auth:{}", job.request_id);
            (
                "earn.job.withheld.v1",
                Some(REASON_CODE_AUTH_ASSURANCE_INSUFFICIENT),
                "withheld",
                stage_auth_gate.clone().expect("auth gate checked"),
            )
        } else if stage == JobLifecycleStage::Paid && stage_provenance_gate.is_some() {
            authority_key = format!("withheld-provenance:{}", job.request_id);
            (
                "earn.job.withheld.v1",
                Some(REASON_CODE_PROVENANCE_REQUIREMENTS_UNMET),
                "withheld",
                stage_provenance_gate
                    .clone()
                    .expect("provenance gate checked"),
            )
        } else if stage == JobLifecycleStage::Paid && stage_rollback_gate.is_some() {
            authority_key = format!("withheld-rollback:{}", job.request_id);
            (
                "earn.job.withheld.v1",
                Some(REASON_CODE_ROLLBACK_PLAN_REQUIRED),
                "withheld",
                stage_rollback_gate.clone().expect("rollback gate checked"),
            )
        } else if stage == JobLifecycleStage::Paid && !paid_pointer_authoritative {
            authority_key = format!("withheld:{}", job.request_id);
            (
                "earn.job.withheld.v1",
                Some(REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE),
                "withheld",
                withhold_policy_decision(
                    "paid_transition_requires_wallet_proof",
                    metadata.category.as_str(),
                    metadata.severity,
                    REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE,
                ),
            )
        } else if stage == JobLifecycleStage::Failed {
            (
                "earn.job.failed.v1",
                Some(REASON_CODE_JOB_FAILED),
                "failed",
                deny_policy_decision(
                    "execution_failure",
                    metadata.category.as_str(),
                    metadata.severity,
                    REASON_CODE_JOB_FAILED,
                ),
            )
        } else {
            (
                match stage {
                    JobLifecycleStage::Accepted => "earn.job.accepted.v1",
                    JobLifecycleStage::Running => "earn.job.executed.v1",
                    JobLifecycleStage::Delivered => "earn.job.result_published.v1",
                    JobLifecycleStage::Paid => "earn.job.settlement_observed.v1",
                    JobLifecycleStage::Received => "earn.job.received.v1",
                    JobLifecycleStage::Failed => "earn.job.failed.v1",
                },
                None,
                "ok",
                allow_policy_decision(stage.label(), metadata.category.as_str(), metadata.severity),
            )
        };
        let receipt_id = lifecycle_receipt_id(job.job_id.as_str(), stage, authority_key.as_str());

        let mut evidence = vec![EvidenceRef::new(
            "request_id",
            format!("oa://nip90/request/{}", job.request_id),
            digest_for_text(job.request_id.as_str()),
        )];
        if let Some(event_id) = job.sa_tick_request_event_id.as_deref() {
            evidence.push(EvidenceRef::new(
                "sa_tick_request_event",
                format!("oa://sa/tick/request/{event_id}"),
                digest_for_text(event_id),
            ));
        }
        if let Some(event_id) = job.sa_tick_result_event_id.as_deref() {
            evidence.push(EvidenceRef::new(
                "sa_tick_result_event",
                format!("oa://sa/tick/result/{event_id}"),
                digest_for_text(event_id),
            ));
        }
        append_provenance_evidence_for_job_stage(&mut evidence, job, stage);
        append_rollback_plan_evidence(&mut evidence, job.job_id.as_str(), &metadata);
        if let Some(pricing) = stage_pricing.as_ref() {
            append_pricing_evidence(&mut evidence, pricing);
        }
        if stage == JobLifecycleStage::Paid && paid_pointer_authoritative {
            evidence.push(EvidenceRef::new(
                "wallet_settlement_proof",
                format!("oa://wallet/payments/{payment_pointer}"),
                digest_for_text(payment_pointer),
            ));
            if let Some(event_id) = job.ac_settlement_event_id.as_deref() {
                evidence.push(EvidenceRef::new(
                    "settlement_feedback_event",
                    format!("oa://nip90/feedback/{event_id}"),
                    digest_for_text(event_id),
                ));
            }
        }
        if stage == JobLifecycleStage::Paid && !paid_pointer_authoritative {
            evidence.push(EvidenceRef::new(
                "withheld_reason",
                format!("oa://earn/jobs/{}/withheld", job.job_id),
                digest_for_text(REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE),
            ));
        }
        if let Some(context) = stage_certification_context.as_ref() {
            evidence.push(certification_reference_evidence(&context.certification));
            if context.safe_harbor_relaxation_applied {
                evidence.push(EvidenceRef::new(
                    "safe_harbor_relaxation",
                    format!(
                        "oa://economy/certifications/{}/safe_harbor",
                        context.certification.certification_id
                    ),
                    digest_for_text(
                        format!(
                            "{}:{}",
                            context.certification.certification_id,
                            context.certification.certification_level
                        )
                        .as_str(),
                    ),
                ));
            }
        }
        if let Some(failure) = stage_certification_gate.as_ref() {
            evidence.push(EvidenceRef::new(
                "withheld_reason",
                format!("oa://earn/jobs/{}/withheld_certification", job.job_id),
                digest_for_text(failure.reason_code),
            ));
        }
        if stage == JobLifecycleStage::Paid && stage_auth_gate.is_some() {
            evidence.push(EvidenceRef::new(
                "withheld_reason",
                format!("oa://earn/jobs/{}/withheld_auth", job.job_id),
                digest_for_text(REASON_CODE_AUTH_ASSURANCE_INSUFFICIENT),
            ));
            evidence.push(credential_ref_for_identity(
                job.requester.as_str(),
                auth_assurance,
            ));
        }
        if stage == JobLifecycleStage::Paid && stage_provenance_gate.is_some() {
            evidence.push(EvidenceRef::new(
                "withheld_reason",
                format!("oa://earn/jobs/{}/withheld_provenance", job.job_id),
                digest_for_text(REASON_CODE_PROVENANCE_REQUIREMENTS_UNMET),
            ));
        }
        if stage == JobLifecycleStage::Paid && stage_rollback_gate.is_some() {
            evidence.push(EvidenceRef::new(
                "withheld_reason",
                format!("oa://earn/jobs/{}/withheld_rollback", job.job_id),
                digest_for_text(REASON_CODE_ROLLBACK_PLAN_REQUIRED),
            ));
        }
        if stage == JobLifecycleStage::Failed {
            let reason = job
                .failure_reason
                .as_deref()
                .unwrap_or("unknown_failure_reason");
            evidence.push(EvidenceRef::new(
                "failure_reason",
                format!("oa://earn/jobs/{}/failure", job.job_id),
                digest_for_text(reason),
            ));
        }
        self.append_receipt_reference_links(
            &mut evidence,
            active_job_link_candidate_receipt_ids(job, stage).as_slice(),
        );
        evidence.push(policy_decision_evidence(&policy_decision));

        let hints = ReceiptHints {
            category: Some(metadata.category.clone()),
            tfb_class: Some(metadata.tfb_class),
            severity: Some(metadata.severity),
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: Some(observed_provenance_grade),
            auth_assurance_level: Some(auth_assurance),
            personhood_proved: Some(personhood_proved),
            reason_code: reason_code.map(ToString::to_string),
            notional: Some(btc_sats_money(job.quoted_price_sats)),
            liability_premium: stage_pricing
                .as_ref()
                .map(|pricing| btc_sats_money(pricing.liability_premium_sats)),
        };
        let receipt_tags = ollama_execution_receipt_tags(job.execution_provenance.as_ref());

        let receipt = ReceiptBuilder::new(
            receipt_id,
            receipt_type,
            epoch_seconds_to_ms(occurred_at_epoch_seconds),
            lifecycle_idempotency_key(stage.label(), job.job_id.as_str(), authority_key.as_str()),
            trace_for_job(
                job.job_id.as_str(),
                Some(job.request_id.as_str()),
                job.sa_trajectory_session_id.as_deref(),
            ),
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "job_id": job.job_id,
            "request_id": job.request_id,
            "requester": job.requester,
            "demand_source": job.demand_source.label(),
            "capability": job.capability,
            "stage": stage.label(),
            "quoted_price_sats": job.quoted_price_sats,
            "payment_pointer": if payment_pointer.is_empty() { None::<String> } else { Some(payment_pointer.to_string()) },
            "work_unit": work_unit_metadata_payload(job.job_id.as_str(), &metadata),
            "liability_pricing": stage_pricing.as_ref().map(pricing_payload),
            "local_execution_provenance": job
                .execution_provenance
                .as_ref()
                .map(|provenance| provenance.receipt_payload()),
        }))
        .with_outputs_payload(json!({
            "stage": stage.label(),
            "source_tag": source_tag,
            "status": status,
            "reason_code": reason_code,
            "work_unit": work_unit_metadata_payload(job.job_id.as_str(), &metadata),
            "policy_rule_id": policy_decision.rule_id,
            "policy_decision": policy_decision.decision,
            "policy_notes": policy_decision.notes,
            "liability_pricing": stage_pricing.as_ref().map(pricing_payload),
            "local_execution_provenance": job
                .execution_provenance
                .as_ref()
                .map(|provenance| provenance.receipt_payload()),
        }))
        .with_evidence(evidence)
        .with_hints(hints)
        .with_tags(receipt_tags)
        .build();

        self.append_receipt(receipt, source_tag);
    }

    pub fn record_history_receipt(
        &mut self,
        row: &JobHistoryReceiptRow,
        occurred_at_epoch_seconds: u64,
        source_tag: &str,
    ) {
        let request_id = infer_request_id_from_job_id(row.job_id.as_str());
        let metadata = self.resolve_or_create_work_unit_metadata(
            row.job_id.as_str(),
            row.demand_source,
            row.payout_sats,
            None,
        );
        let auth_assurance = AuthAssuranceLevel::Authenticated;
        let personhood_proved = false;
        let policy_bundle = current_policy_bundle();
        let history_certification_eval = if row.status == JobHistoryStatus::Succeeded {
            Some(evaluate_certification_gate(
                &policy_bundle,
                self.receipts.as_slice(),
                metadata.category.as_str(),
                metadata.tfb_class,
                metadata.severity,
                epoch_seconds_to_ms(occurred_at_epoch_seconds),
            ))
        } else {
            None
        };
        let history_certification_gate = history_certification_eval
            .as_ref()
            .and_then(|evaluation| evaluation.as_ref().err().cloned());
        let history_certification_context = history_certification_eval
            .as_ref()
            .and_then(|evaluation| evaluation.as_ref().ok().cloned());
        let history_auth_gate = if row.status == JobHistoryStatus::Succeeded {
            evaluate_authentication_gate(
                &policy_bundle,
                metadata.category.as_str(),
                metadata.tfb_class,
                metadata.severity,
                "verifier",
                auth_assurance,
                personhood_proved,
            )
            .err()
        } else {
            None
        };
        let mut history_provenance_probe = Vec::<EvidenceRef>::new();
        append_provenance_evidence_for_history(
            &mut history_provenance_probe,
            row,
            JobLifecycleStage::Paid,
        );
        let observed_provenance_grade = provenance_grade_from_features(
            &provenance_features_from_evidence(history_provenance_probe.as_slice()),
        );
        let history_provenance_gate = if row.status == JobHistoryStatus::Succeeded {
            evaluate_provenance_gate(
                &policy_bundle,
                metadata.category.as_str(),
                metadata.tfb_class,
                metadata.severity,
                history_provenance_probe.as_slice(),
            )
            .err()
        } else {
            None
        };
        let history_rollback_gate = if row.status == JobHistoryStatus::Succeeded {
            evaluate_rollback_gate(
                &policy_bundle,
                metadata.category.as_str(),
                metadata.tfb_class,
                metadata.severity,
                row.job_id.as_str(),
                &metadata,
            )
            .err()
        } else {
            None
        };
        let payment_pointer_authoritative =
            is_wallet_authoritative_payment_pointer(Some(row.payment_pointer.as_str()));
        let history_pricing = if row.status == JobHistoryStatus::Succeeded {
            Some(compute_liability_pricing_for_settlement(
                self.receipts.as_slice(),
                &policy_bundle,
                metadata.category.as_str(),
                metadata.tfb_class,
                metadata.severity,
                row.payout_sats,
                metadata.verification_budget_hint_sats,
                history_certification_context.as_ref(),
            ))
        } else {
            None
        };
        let (stage, receipt_type, reason_code, status, authority_key, policy_decision): (
            JobLifecycleStage,
            &'static str,
            Option<&'static str>,
            &'static str,
            String,
            PolicyDecision,
        ) = if row.status == JobHistoryStatus::Succeeded
            && payment_pointer_authoritative
            && history_certification_gate.is_some()
        {
            (
                JobLifecycleStage::Paid,
                "earn.job.withheld.v1",
                Some(
                    history_certification_gate
                        .as_ref()
                        .map(|failure| failure.reason_code)
                        .expect("certification gate checked"),
                ),
                "withheld",
                format!("withheld-certification:{request_id}"),
                history_certification_gate
                    .as_ref()
                    .expect("certification gate checked")
                    .decision
                    .clone(),
            )
        } else if row.status == JobHistoryStatus::Succeeded
            && payment_pointer_authoritative
            && history_auth_gate.is_some()
        {
            (
                JobLifecycleStage::Paid,
                "earn.job.withheld.v1",
                Some(REASON_CODE_AUTH_ASSURANCE_INSUFFICIENT),
                "withheld",
                format!("withheld-auth:{request_id}"),
                history_auth_gate.clone().expect("auth gate checked"),
            )
        } else if row.status == JobHistoryStatus::Succeeded
            && payment_pointer_authoritative
            && history_provenance_gate.is_some()
        {
            (
                JobLifecycleStage::Paid,
                "earn.job.withheld.v1",
                Some(REASON_CODE_PROVENANCE_REQUIREMENTS_UNMET),
                "withheld",
                format!("withheld-provenance:{request_id}"),
                history_provenance_gate
                    .clone()
                    .expect("provenance gate checked"),
            )
        } else if row.status == JobHistoryStatus::Succeeded
            && payment_pointer_authoritative
            && history_rollback_gate.is_some()
        {
            (
                JobLifecycleStage::Paid,
                "earn.job.withheld.v1",
                Some(REASON_CODE_ROLLBACK_PLAN_REQUIRED),
                "withheld",
                format!("withheld-rollback:{request_id}"),
                history_rollback_gate
                    .clone()
                    .expect("rollback gate checked"),
            )
        } else if row.status == JobHistoryStatus::Succeeded && payment_pointer_authoritative {
            (
                JobLifecycleStage::Paid,
                "earn.job.settlement_observed.v1",
                None,
                "succeeded",
                row.payment_pointer.clone(),
                allow_policy_decision(
                    "history_paid",
                    metadata.category.as_str(),
                    metadata.severity,
                ),
            )
        } else if row.status == JobHistoryStatus::Succeeded {
            (
                JobLifecycleStage::Paid,
                "earn.job.withheld.v1",
                Some(REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE),
                "withheld",
                format!("withheld:{request_id}"),
                withhold_policy_decision(
                    "history_paid_requires_wallet_proof",
                    metadata.category.as_str(),
                    metadata.severity,
                    REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE,
                ),
            )
        } else {
            (
                JobLifecycleStage::Failed,
                "earn.job.failed.v1",
                Some(REASON_CODE_JOB_FAILED),
                "failed",
                row.result_hash.clone(),
                deny_policy_decision(
                    "history_failed",
                    metadata.category.as_str(),
                    metadata.severity,
                    REASON_CODE_JOB_FAILED,
                ),
            )
        };
        let link_candidates =
            history_row_link_candidate_receipt_ids(row, stage, request_id.as_str());
        let receipt_tags = ollama_execution_receipt_tags(row.execution_provenance.as_ref());

        let receipt = ReceiptBuilder::new(
            lifecycle_receipt_id(row.job_id.as_str(), stage, authority_key.as_str()),
            receipt_type,
            epoch_seconds_to_ms(occurred_at_epoch_seconds),
            lifecycle_idempotency_key(
                "history_receipt",
                row.job_id.as_str(),
                authority_key.as_str(),
            ),
            trace_for_job(
                row.job_id.as_str(),
                Some(request_id.as_str()),
                row.sa_trajectory_session_id.as_deref(),
            ),
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "job_id": row.job_id,
            "status": row.status.label(),
            "demand_source": row.demand_source.label(),
            "payout_sats": row.payout_sats,
            "payment_pointer": row.payment_pointer,
            "result_hash": row.result_hash,
            "failure_reason": row.failure_reason,
            "work_unit": work_unit_metadata_payload(row.job_id.as_str(), &metadata),
            "liability_pricing": history_pricing.as_ref().map(pricing_payload),
            "local_execution_provenance": row
                .execution_provenance
                .as_ref()
                .map(|provenance| provenance.receipt_payload()),
        }))
        .with_outputs_payload(json!({
            "stage": stage.label(),
            "source_tag": source_tag,
            "status": status,
            "wallet_settlement_authoritative": payment_pointer_authoritative,
            "reason_code": reason_code,
            "work_unit": work_unit_metadata_payload(row.job_id.as_str(), &metadata),
            "policy_rule_id": policy_decision.rule_id,
            "policy_decision": policy_decision.decision,
            "policy_notes": policy_decision.notes,
            "liability_pricing": history_pricing.as_ref().map(pricing_payload),
            "local_execution_provenance": row
                .execution_provenance
                .as_ref()
                .map(|provenance| provenance.receipt_payload()),
        }))
        .with_evidence({
            let mut evidence = vec![EvidenceRef::new(
                "history_result_hash",
                format!("oa://earn/jobs/{}/result", row.job_id),
                normalize_digest(row.result_hash.as_str()),
            )];
            append_provenance_evidence_for_history(&mut evidence, row, stage);
            append_rollback_plan_evidence(&mut evidence, row.job_id.as_str(), &metadata);
            if let Some(pricing) = history_pricing.as_ref() {
                append_pricing_evidence(&mut evidence, pricing);
            }
            if let Some(context) = history_certification_context.as_ref() {
                evidence.push(certification_reference_evidence(&context.certification));
                if context.safe_harbor_relaxation_applied {
                    evidence.push(EvidenceRef::new(
                        "safe_harbor_relaxation",
                        format!(
                            "oa://economy/certifications/{}/safe_harbor",
                            context.certification.certification_id
                        ),
                        digest_for_text(
                            format!(
                                "{}:{}",
                                context.certification.certification_id,
                                context.certification.certification_level
                            )
                            .as_str(),
                        ),
                    ));
                }
            }
            if stage == JobLifecycleStage::Paid && payment_pointer_authoritative {
                evidence.push(EvidenceRef::new(
                    "wallet_settlement_proof",
                    format!("oa://wallet/payments/{}", row.payment_pointer),
                    digest_for_text(row.payment_pointer.as_str()),
                ));
            } else if stage == JobLifecycleStage::Paid && history_certification_gate.is_some() {
                let reason = history_certification_gate
                    .as_ref()
                    .map(|failure| failure.reason_code)
                    .unwrap_or(REASON_CODE_DIGITAL_BORDER_BLOCK_UNCERTIFIED);
                evidence.push(EvidenceRef::new(
                    "withheld_reason",
                    format!("oa://earn/jobs/{}/withheld_certification", row.job_id),
                    digest_for_text(reason),
                ));
            } else if stage == JobLifecycleStage::Paid
                && reason_code == Some(REASON_CODE_AUTH_ASSURANCE_INSUFFICIENT)
            {
                evidence.push(EvidenceRef::new(
                    "withheld_reason",
                    format!("oa://earn/jobs/{}/withheld_auth", row.job_id),
                    digest_for_text(REASON_CODE_AUTH_ASSURANCE_INSUFFICIENT),
                ));
                evidence.push(credential_ref_for_identity(
                    "history_projection",
                    auth_assurance,
                ));
            } else if stage == JobLifecycleStage::Paid
                && reason_code == Some(REASON_CODE_PROVENANCE_REQUIREMENTS_UNMET)
            {
                evidence.push(EvidenceRef::new(
                    "withheld_reason",
                    format!("oa://earn/jobs/{}/withheld_provenance", row.job_id),
                    digest_for_text(REASON_CODE_PROVENANCE_REQUIREMENTS_UNMET),
                ));
            } else if stage == JobLifecycleStage::Paid
                && reason_code == Some(REASON_CODE_ROLLBACK_PLAN_REQUIRED)
            {
                evidence.push(EvidenceRef::new(
                    "withheld_reason",
                    format!("oa://earn/jobs/{}/withheld_rollback", row.job_id),
                    digest_for_text(REASON_CODE_ROLLBACK_PLAN_REQUIRED),
                ));
            } else if stage == JobLifecycleStage::Paid {
                evidence.push(EvidenceRef::new(
                    "withheld_reason",
                    format!("oa://earn/jobs/{}/withheld", row.job_id),
                    digest_for_text(REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE),
                ));
            } else {
                evidence.push(EvidenceRef::new(
                    "failure_reason",
                    format!("oa://earn/jobs/{}/failure", row.job_id),
                    digest_for_text(
                        row.failure_reason
                            .as_deref()
                            .unwrap_or("unknown_failure_reason"),
                    ),
                ));
            }
            self.append_receipt_reference_links(&mut evidence, link_candidates.as_slice());
            evidence.push(policy_decision_evidence(&policy_decision));
            evidence
        })
        .with_hints(ReceiptHints {
            category: Some(metadata.category.clone()),
            tfb_class: Some(metadata.tfb_class),
            severity: Some(metadata.severity),
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: Some(observed_provenance_grade),
            auth_assurance_level: Some(auth_assurance),
            personhood_proved: Some(personhood_proved),
            reason_code: reason_code.map(ToString::to_string),
            notional: Some(btc_sats_money(row.payout_sats)),
            liability_premium: history_pricing
                .as_ref()
                .map(|pricing| btc_sats_money(pricing.liability_premium_sats)),
        })
        .with_tags(receipt_tags)
        .build();

        self.append_receipt(receipt, source_tag);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_economy_snapshot_receipt(
        &mut self,
        snapshot_id: &str,
        as_of_ms: i64,
        snapshot_hash: &str,
        sv: f64,
        sv_effective: f64,
        rho: f64,
        rho_effective: f64,
        n: u64,
        nv: f64,
        delta_m_hat: f64,
        xa_hat: f64,
        correlated_verification_share: f64,
        liability_premiums_collected_24h_sats: u64,
        claims_paid_24h_sats: u64,
        bonded_exposure_24h_sats: u64,
        capital_reserves_24h_sats: u64,
        loss_ratio: f64,
        capital_coverage_ratio: f64,
        drift_alerts_24h: u64,
        drift_signals: Vec<DriftSignalSummary>,
        top_drift_signals: Vec<DriftSignalSummary>,
        rollback_attempts_24h: u64,
        rollback_successes_24h: u64,
        rollback_success_rate: f64,
        top_rollback_reason_codes: Vec<(String, u64)>,
        audit_package_public_digest: String,
        audit_package_restricted_digest: String,
        mut input_evidence: Vec<EvidenceRef>,
        source_tag: &str,
    ) {
        if snapshot_id.trim().is_empty() || snapshot_hash.trim().is_empty() {
            self.last_error =
                Some("Cannot emit economy snapshot receipt: missing snapshot id/hash".to_string());
            self.load_state = PaneLoadState::Error;
            return;
        }
        let receipt_id = format!("receipt.economy.snapshot:{}", as_of_ms.max(0));
        let idempotency_key = format!("idemp.economy.snapshot:{}", as_of_ms.max(0));
        input_evidence.push(EvidenceRef::new(
            "economy_snapshot_artifact",
            format!("oa://economy/snapshots/{snapshot_id}"),
            snapshot_hash.to_string(),
        ));
        let snapshot_metrics_digest = digest_for_text(
            format!(
                "{snapshot_id}:{sv}:{sv_effective}:{rho}:{rho_effective}:{n}:{nv}:{delta_m_hat}:{xa_hat}:{correlated_verification_share}:{liability_premiums_collected_24h_sats}:{claims_paid_24h_sats}:{bonded_exposure_24h_sats}:{capital_reserves_24h_sats}:{loss_ratio}:{capital_coverage_ratio}:{drift_alerts_24h}:{rollback_attempts_24h}:{rollback_successes_24h}:{rollback_success_rate}:{}:{}:{audit_package_public_digest}:{audit_package_restricted_digest}"
                ,
                drift_signal_digest_material(drift_signals.as_slice()),
                top_rollback_reason_codes
                    .iter()
                    .map(|(code, count)| format!("{code}:{count}"))
                    .collect::<Vec<_>>()
                    .join("|")
            )
            .as_str(),
        );
        let mut snapshot_metrics = EvidenceRef::new(
            "snapshot_metrics",
            format!("oa://economy/snapshots/{snapshot_id}/metrics"),
            snapshot_metrics_digest,
        );
        snapshot_metrics.meta.insert("sv".to_string(), json!(sv));
        snapshot_metrics
            .meta
            .insert("sv_effective".to_string(), json!(sv_effective));
        snapshot_metrics.meta.insert("rho".to_string(), json!(rho));
        snapshot_metrics
            .meta
            .insert("rho_effective".to_string(), json!(rho_effective));
        snapshot_metrics.meta.insert("n".to_string(), json!(n));
        snapshot_metrics.meta.insert("nv".to_string(), json!(nv));
        snapshot_metrics
            .meta
            .insert("delta_m_hat".to_string(), json!(delta_m_hat));
        snapshot_metrics
            .meta
            .insert("xa_hat".to_string(), json!(xa_hat));
        snapshot_metrics.meta.insert(
            "correlated_verification_share".to_string(),
            json!(correlated_verification_share),
        );
        snapshot_metrics.meta.insert(
            "liability_premiums_collected_24h_sats".to_string(),
            json!(liability_premiums_collected_24h_sats),
        );
        snapshot_metrics.meta.insert(
            "claims_paid_24h_sats".to_string(),
            json!(claims_paid_24h_sats),
        );
        snapshot_metrics.meta.insert(
            "bonded_exposure_24h_sats".to_string(),
            json!(bonded_exposure_24h_sats),
        );
        snapshot_metrics.meta.insert(
            "capital_reserves_24h_sats".to_string(),
            json!(capital_reserves_24h_sats),
        );
        snapshot_metrics
            .meta
            .insert("loss_ratio".to_string(), json!(loss_ratio));
        snapshot_metrics.meta.insert(
            "capital_coverage_ratio".to_string(),
            json!(capital_coverage_ratio),
        );
        snapshot_metrics
            .meta
            .insert("drift_alerts_24h".to_string(), json!(drift_alerts_24h));
        snapshot_metrics
            .meta
            .insert("drift_signals".to_string(), json!(drift_signals));
        snapshot_metrics
            .meta
            .insert("top_drift_signals".to_string(), json!(top_drift_signals));
        snapshot_metrics.meta.insert(
            "rollback_attempts_24h".to_string(),
            json!(rollback_attempts_24h),
        );
        snapshot_metrics.meta.insert(
            "rollback_successes_24h".to_string(),
            json!(rollback_successes_24h),
        );
        snapshot_metrics.meta.insert(
            "rollback_success_rate".to_string(),
            json!(rollback_success_rate),
        );
        snapshot_metrics.meta.insert(
            "top_rollback_reason_codes".to_string(),
            json!(top_rollback_reason_codes),
        );
        snapshot_metrics.meta.insert(
            "audit_package_public_digest".to_string(),
            json!(audit_package_public_digest),
        );
        snapshot_metrics.meta.insert(
            "audit_package_restricted_digest".to_string(),
            json!(audit_package_restricted_digest),
        );
        input_evidence.push(snapshot_metrics);

        let receipt = ReceiptBuilder::new(
            receipt_id,
            "economy.stats.snapshot_receipt.v1",
            as_of_ms.max(0),
            idempotency_key,
            TraceContext {
                session_id: None,
                trajectory_hash: None,
                job_hash: None,
                run_id: Some(format!("economy_snapshot:{as_of_ms}")),
                work_unit_id: None,
                contract_id: None,
                claim_id: None,
            },
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "snapshot_id": snapshot_id,
            "as_of_ms": as_of_ms,
            "inputs_count": input_evidence.len(),
        }))
        .with_outputs_payload(json!({
            "snapshot_id": snapshot_id,
            "snapshot_hash": snapshot_hash,
            "as_of_ms": as_of_ms,
            "status": "computed",
            "sv": sv,
            "sv_effective": sv_effective,
            "rho": rho,
            "rho_effective": rho_effective,
            "N": n,
            "NV": nv,
            "delta_m_hat": delta_m_hat,
            "xa_hat": xa_hat,
            "correlated_verification_share": correlated_verification_share,
            "liability_premiums_collected_24h_sats": liability_premiums_collected_24h_sats,
            "claims_paid_24h_sats": claims_paid_24h_sats,
            "bonded_exposure_24h_sats": bonded_exposure_24h_sats,
            "capital_reserves_24h_sats": capital_reserves_24h_sats,
            "loss_ratio": loss_ratio,
            "capital_coverage_ratio": capital_coverage_ratio,
            "drift_alerts_24h": drift_alerts_24h,
            "top_drift_signals": top_drift_signals,
            "rollback_attempts_24h": rollback_attempts_24h,
            "rollback_successes_24h": rollback_successes_24h,
            "rollback_success_rate": rollback_success_rate,
            "top_rollback_reason_codes": top_rollback_reason_codes,
            "audit_package_public_digest": audit_package_public_digest,
            "audit_package_restricted_digest": audit_package_restricted_digest,
            "source_tag": source_tag,
        }))
        .with_evidence(input_evidence)
        .with_hints(ReceiptHints {
            category: Some("compute".to_string()),
            tfb_class: None,
            severity: None,
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: None,
            auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
            personhood_proved: Some(false),
            reason_code: None,
            notional: None,
            liability_premium: None,
        })
        .build();

        self.append_receipt(receipt, source_tag);
        if self.load_state == PaneLoadState::Error {
            return;
        }

        self.emit_drift_receipts_for_snapshot(
            snapshot_id,
            as_of_ms,
            snapshot_hash,
            drift_signals.as_slice(),
            source_tag,
        );
        if self.load_state == PaneLoadState::Error {
            return;
        }

        let policy_bundle = current_policy_bundle();
        self.emit_policy_throttle_receipts_for_snapshot(
            snapshot_id,
            as_of_ms,
            snapshot_hash,
            sv,
            sv_effective,
            xa_hat,
            delta_m_hat,
            correlated_verification_share,
            drift_alerts_24h,
            &policy_bundle,
            source_tag,
        );
    }

    fn emit_drift_receipts_for_snapshot(
        &mut self,
        snapshot_id: &str,
        as_of_ms: i64,
        snapshot_hash: &str,
        drift_signals: &[DriftSignalSummary],
        source_tag: &str,
    ) {
        let mut ordered_signals = drift_signals.to_vec();
        ordered_signals.sort_by(|lhs, rhs| {
            lhs.detector_id
                .cmp(&rhs.detector_id)
                .then_with(|| lhs.signal_code.cmp(&rhs.signal_code))
        });
        let previous_active = active_drift_alert_detectors_before(
            self.receipts.as_slice(),
            as_of_ms.saturating_sub(1),
        );
        let mut current_active = BTreeSet::<String>::new();

        for signal in ordered_signals {
            let normalized_detector = normalize_key(signal.detector_id.as_str());
            let signal_receipt_id = format!(
                "receipt.economy.drift_signal:{}:{}",
                normalize_key(snapshot_id),
                normalized_detector
            );
            let signal_receipt = ReceiptBuilder::new(
                signal_receipt_id.clone(),
                "economy.drift.signal_emitted.v1",
                as_of_ms.max(0),
                format!(
                    "idemp.economy.drift_signal:{}:{}",
                    normalize_key(snapshot_id),
                    normalized_detector
                ),
                TraceContext {
                    session_id: None,
                    trajectory_hash: None,
                    job_hash: None,
                    run_id: Some(format!("economy_drift:{snapshot_id}")),
                    work_unit_id: None,
                    contract_id: None,
                    claim_id: None,
                },
                current_policy_context(),
            )
            .with_inputs_payload(json!({
                "snapshot_id": snapshot_id,
                "snapshot_hash": snapshot_hash,
                "detector_id": signal.detector_id,
                "signal_code": signal.signal_code,
                "ratio": signal.ratio,
                "threshold": signal.threshold,
                "count_24h": signal.count_24h,
                "score": signal.score,
                "alert": signal.alert,
            }))
            .with_outputs_payload(json!({
                "status": "signal_emitted",
                "snapshot_id": snapshot_id,
                "snapshot_hash": snapshot_hash,
                "detector_id": signal.detector_id,
                "signal_code": signal.signal_code,
                "count_24h": signal.count_24h,
                "score": signal.score,
                "alert": signal.alert,
                "source_tag": source_tag,
            }))
            .with_evidence(vec![
                EvidenceRef::new(
                    "snapshot_ref",
                    format!("oa://economy/snapshots/{snapshot_id}"),
                    snapshot_hash.to_string(),
                ),
                drift_detector_evidence(&signal),
                drift_signal_summary_evidence(snapshot_id, &signal),
            ])
            .with_hints(ReceiptHints {
                category: Some("compute".to_string()),
                tfb_class: None,
                severity: Some(if signal.alert {
                    SeverityClass::High
                } else {
                    SeverityClass::Low
                }),
                achieved_verification_tier: None,
                verification_correlated: None,
                provenance_grade: None,
                auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
                personhood_proved: Some(false),
                reason_code: Some(REASON_CODE_DRIFT_SIGNAL_EMITTED.to_string()),
                notional: None,
                liability_premium: None,
            })
            .build();
            self.append_receipt(signal_receipt, source_tag);
            if self.load_state == PaneLoadState::Error {
                return;
            }

            if !signal.alert {
                continue;
            }
            current_active.insert(signal.detector_id.clone());
            let alert_receipt = ReceiptBuilder::new(
                format!(
                    "receipt.economy.drift_alert:{}:{}",
                    normalize_key(snapshot_id),
                    normalized_detector
                ),
                "economy.drift.alert_raised.v1",
                as_of_ms.max(0),
                format!(
                    "idemp.economy.drift_alert:{}:{}",
                    normalize_key(snapshot_id),
                    normalized_detector
                ),
                TraceContext {
                    session_id: None,
                    trajectory_hash: None,
                    job_hash: None,
                    run_id: Some(format!("economy_drift:{snapshot_id}")),
                    work_unit_id: None,
                    contract_id: None,
                    claim_id: None,
                },
                current_policy_context(),
            )
            .with_inputs_payload(json!({
                "snapshot_id": snapshot_id,
                "snapshot_hash": snapshot_hash,
                "detector_id": signal.detector_id,
                "signal_code": signal.signal_code,
                "score": signal.score,
                "count_24h": signal.count_24h,
            }))
            .with_outputs_payload(json!({
                "status": "alert_raised",
                "snapshot_id": snapshot_id,
                "snapshot_hash": snapshot_hash,
                "detector_id": signal.detector_id,
                "signal_code": signal.signal_code,
                "score": signal.score,
                "count_24h": signal.count_24h,
                "source_tag": source_tag,
            }))
            .with_evidence(vec![
                EvidenceRef::new(
                    "snapshot_ref",
                    format!("oa://economy/snapshots/{snapshot_id}"),
                    snapshot_hash.to_string(),
                ),
                drift_detector_evidence(&signal),
                EvidenceRef::new(
                    "receipt_ref",
                    format!("oa://receipts/{signal_receipt_id}"),
                    self.get_receipt(signal_receipt_id.as_str())
                        .map(|receipt| receipt.canonical_hash.clone())
                        .unwrap_or_else(|| digest_for_text(signal_receipt_id.as_str())),
                ),
            ])
            .with_hints(ReceiptHints {
                category: Some("compute".to_string()),
                tfb_class: None,
                severity: Some(SeverityClass::High),
                achieved_verification_tier: None,
                verification_correlated: None,
                provenance_grade: None,
                auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
                personhood_proved: Some(false),
                reason_code: Some(REASON_CODE_DRIFT_ALERT_RAISED.to_string()),
                notional: None,
                liability_premium: None,
            })
            .build();
            self.append_receipt(alert_receipt, source_tag);
            if self.load_state == PaneLoadState::Error {
                return;
            }
        }

        let mut cleared = previous_active
            .difference(&current_active)
            .cloned()
            .collect::<Vec<_>>();
        cleared.sort();
        for detector_id in cleared {
            let normalized_detector = normalize_key(detector_id.as_str());
            let fallback_signal = DriftSignalSummary {
                detector_id: detector_id.clone(),
                signal_code: "alert_cleared".to_string(),
                count_24h: 0,
                ratio: 0.0,
                threshold: 0.0,
                score: 0.0,
                alert: false,
            };
            let signal = drift_signals
                .iter()
                .find(|candidate| candidate.detector_id == detector_id)
                .cloned()
                .unwrap_or(fallback_signal);
            let false_positive_receipt = ReceiptBuilder::new(
                format!(
                    "receipt.economy.drift_false_positive:{}:{}",
                    normalize_key(snapshot_id),
                    normalized_detector
                ),
                "economy.drift.false_positive_confirmed.v1",
                as_of_ms.max(0),
                format!(
                    "idemp.economy.drift_false_positive:{}:{}",
                    normalize_key(snapshot_id),
                    normalized_detector
                ),
                TraceContext {
                    session_id: None,
                    trajectory_hash: None,
                    job_hash: None,
                    run_id: Some(format!("economy_drift:{snapshot_id}")),
                    work_unit_id: None,
                    contract_id: None,
                    claim_id: None,
                },
                current_policy_context(),
            )
            .with_inputs_payload(json!({
                "snapshot_id": snapshot_id,
                "snapshot_hash": snapshot_hash,
                "detector_id": detector_id,
                "signal_code": signal.signal_code,
            }))
            .with_outputs_payload(json!({
                "status": "false_positive_confirmed",
                "snapshot_id": snapshot_id,
                "snapshot_hash": snapshot_hash,
                "detector_id": detector_id,
                "source_tag": source_tag,
            }))
            .with_evidence(vec![
                EvidenceRef::new(
                    "snapshot_ref",
                    format!("oa://economy/snapshots/{snapshot_id}"),
                    snapshot_hash.to_string(),
                ),
                drift_detector_evidence(&signal),
            ])
            .with_hints(ReceiptHints {
                category: Some("compute".to_string()),
                tfb_class: None,
                severity: Some(SeverityClass::Medium),
                achieved_verification_tier: None,
                verification_correlated: None,
                provenance_grade: None,
                auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
                personhood_proved: Some(false),
                reason_code: Some(REASON_CODE_DRIFT_FALSE_POSITIVE_CONFIRMED.to_string()),
                notional: None,
                liability_premium: None,
            })
            .build();
            self.append_receipt(false_positive_receipt, source_tag);
            if self.load_state == PaneLoadState::Error {
                return;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_policy_throttle_receipts_for_snapshot(
        &mut self,
        snapshot_id: &str,
        as_of_ms: i64,
        snapshot_hash: &str,
        sv: f64,
        sv_effective: f64,
        xa_hat: f64,
        delta_m_hat: f64,
        correlated_verification_share: f64,
        drift_alerts_24h: u64,
        policy_bundle: &PolicyBundleConfig,
        source_tag: &str,
    ) {
        let category = "compute";
        let tfb_class = FeedbackLatencyClass::Short;
        let severity = SeverityClass::High;
        let _ =
            select_authentication_rule(policy_bundle, category, tfb_class, severity, "operator");
        let _ = select_monitoring_rule(policy_bundle, category, tfb_class, severity);
        let _ = select_risk_pricing_rule(policy_bundle, category, tfb_class, severity);
        let _ = select_certification_rule(policy_bundle, category, tfb_class, severity);
        let _ = select_rollback_rule(policy_bundle, category, tfb_class, severity);
        let actions = evaluate_triggered_policy_actions(
            policy_bundle,
            category,
            tfb_class,
            severity,
            SnapshotPolicyMetrics {
                sv,
                sv_effective,
                rho_effective: sv_effective,
                xa_hat,
                delta_m_hat,
                correlated_verification_share,
                drift_alerts_24h,
            },
        );
        if actions.is_empty() {
            return;
        }

        for (index, action) in actions.into_iter().enumerate() {
            let decision = PolicyDecision {
                rule_id: action.rule_id.clone(),
                decision: "throttle",
                notes: format!(
                    "{} action={} snapshot_id={} snapshot_hash={} source_tag={} order={}",
                    action.notes,
                    action.action.label(),
                    snapshot_id,
                    snapshot_hash,
                    source_tag,
                    index,
                ),
            };
            let receipt = ReceiptBuilder::new(
                format!(
                    "receipt.economy.policy_throttle:{}:{}:{}",
                    normalize_key(snapshot_id),
                    normalize_key(action.rule_id.as_str()),
                    action.action.label(),
                ),
                "economy.policy.throttle_action_applied.v1",
                as_of_ms.max(0),
                format!(
                    "idemp.economy.policy_throttle:{}:{}:{}",
                    normalize_key(snapshot_id),
                    normalize_key(action.rule_id.as_str()),
                    action.action.label(),
                ),
                TraceContext {
                    session_id: None,
                    trajectory_hash: None,
                    job_hash: None,
                    run_id: Some(format!("economy_policy_throttle:{snapshot_id}")),
                    work_unit_id: None,
                    contract_id: None,
                    claim_id: None,
                },
                current_policy_context(),
            )
            .with_inputs_payload(json!({
                "snapshot_id": snapshot_id,
                "snapshot_hash": snapshot_hash,
                "sv": sv,
                "sv_effective": sv_effective,
                "xa_hat": xa_hat,
                "delta_m_hat": delta_m_hat,
                "correlated_verification_share": correlated_verification_share,
                "drift_alerts_24h": drift_alerts_24h,
            }))
            .with_outputs_payload(json!({
                "status": "triggered",
                "action_order": index,
                "policy_rule_id": action.rule_id,
                "policy_rule_kind": action.rule_kind,
                "policy_action": action.action.label(),
                "snapshot_id": snapshot_id,
                "snapshot_hash": snapshot_hash,
                "source_tag": source_tag,
            }))
            .with_evidence(vec![
                EvidenceRef::new(
                    "snapshot_ref",
                    format!("oa://economy/snapshots/{snapshot_id}"),
                    snapshot_hash,
                ),
                policy_decision_evidence(&decision),
            ])
            .with_hints(ReceiptHints {
                category: Some(category.to_string()),
                tfb_class: Some(tfb_class),
                severity: Some(severity),
                achieved_verification_tier: None,
                verification_correlated: Some(correlated_verification_share > 0.0),
                provenance_grade: None,
                auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
                personhood_proved: Some(false),
                reason_code: Some(REASON_CODE_POLICY_THROTTLE_TRIGGERED.to_string()),
                notional: None,
                liability_premium: None,
            })
            .build();
            self.append_receipt(receipt, source_tag);
            if self.load_state == PaneLoadState::Error {
                return;
            }
        }
    }

    pub fn record_wallet_withdraw_send_attempt(
        &mut self,
        caller_identity: &str,
        payment_request: &str,
        amount_sats: Option<u64>,
        occurred_at_epoch_ms: i64,
        source_tag: &str,
    ) -> Result<String, String> {
        let policy_bundle = current_policy_bundle();
        self.record_wallet_withdraw_send_attempt_with_policy(
            caller_identity,
            payment_request,
            amount_sats,
            occurred_at_epoch_ms,
            source_tag,
            &policy_bundle,
        )
    }

    fn record_wallet_withdraw_send_attempt_with_policy(
        &mut self,
        caller_identity: &str,
        payment_request: &str,
        amount_sats: Option<u64>,
        occurred_at_epoch_ms: i64,
        source_tag: &str,
        policy_bundle: &PolicyBundleConfig,
    ) -> Result<String, String> {
        let caller_identity = caller_identity.trim();
        if caller_identity.is_empty() {
            return Err("caller_identity cannot be empty".to_string());
        }
        let payment_request = payment_request.trim();
        if payment_request.is_empty() {
            return Err("payment_request cannot be empty".to_string());
        }

        let payment_request_digest = digest_for_text(payment_request);
        let amount_sats = amount_sats.unwrap_or(0);
        let idempotency_key = format!(
            "idemp.wallet.withdraw_send:{}:{}",
            normalize_key(caller_identity),
            normalize_key(payment_request_digest.as_str()),
        );
        let success_receipt_id = format!(
            "receipt.earn:wallet_withdraw:{}:{}",
            normalize_key(caller_identity),
            normalize_key(payment_request_digest.as_str())
        );
        let mut policy = current_policy_context();
        policy.approved_by = caller_identity.to_string();
        let severity = if amount_sats >= 100_000 {
            SeverityClass::Critical
        } else if amount_sats >= 10_000 {
            SeverityClass::High
        } else if amount_sats >= 1_000 {
            SeverityClass::Medium
        } else {
            SeverityClass::Low
        };
        let auth_assurance = auth_assurance_for_identity(caller_identity);
        let personhood_proved = personhood_proved_for_identity(caller_identity, auth_assurance);
        let auth_evidence = credential_ref_for_identity(caller_identity, auth_assurance);
        let auth_policy_result = evaluate_authentication_gate(
            policy_bundle,
            "compute",
            FeedbackLatencyClass::Instant,
            severity,
            "operator",
            auth_assurance,
            personhood_proved,
        );
        let policy_decision = match auth_policy_result {
            Ok(decision) => decision,
            Err(decision) => {
                let withheld_receipt = ReceiptBuilder::new(
                    format!(
                        "receipt.earn:wallet_withdraw_withheld:{}:{}",
                        normalize_key(caller_identity),
                        normalize_key(payment_request_digest.as_str())
                    ),
                    "earn.wallet.withdraw_withheld.v1",
                    occurred_at_epoch_ms.max(0),
                    format!("{idempotency_key}:auth_withheld"),
                    TraceContext {
                        session_id: None,
                        trajectory_hash: None,
                        job_hash: None,
                        run_id: Some(format!("wallet_withdraw:{caller_identity}")),
                        work_unit_id: None,
                        contract_id: None,
                        claim_id: None,
                    },
                    policy,
                )
                .with_inputs_payload(json!({
                    "caller_identity": caller_identity,
                    "payment_request_digest": payment_request_digest,
                    "amount_sats": if amount_sats == 0 { None::<u64> } else { Some(amount_sats) },
                }))
                .with_outputs_payload(json!({
                    "status": "withheld",
                    "reason_code": REASON_CODE_AUTH_ASSURANCE_INSUFFICIENT,
                    "policy_rule_id": decision.rule_id,
                    "policy_decision": decision.decision,
                    "policy_notes": decision.notes,
                    "source_tag": source_tag,
                }))
                .with_evidence(vec![
                    EvidenceRef::new(
                        "wallet_send_request",
                        format!("oa://wallet/withdraw/{caller_identity}"),
                        payment_request_digest.clone(),
                    ),
                    auth_evidence,
                    policy_decision_evidence(&decision),
                ])
                .with_hints(ReceiptHints {
                    category: Some("compute".to_string()),
                    tfb_class: Some(FeedbackLatencyClass::Instant),
                    severity: Some(severity),
                    achieved_verification_tier: None,
                    verification_correlated: None,
                    provenance_grade: None,
                    auth_assurance_level: Some(auth_assurance),
                    personhood_proved: Some(personhood_proved),
                    reason_code: Some(REASON_CODE_AUTH_ASSURANCE_INSUFFICIENT.to_string()),
                    notional: if amount_sats == 0 {
                        None
                    } else {
                        Some(Money {
                            asset: Asset::Btc,
                            amount: MoneyAmount::AmountSats(amount_sats),
                        })
                    },
                    liability_premium: None,
                })
                .build();
                self.append_receipt(withheld_receipt, source_tag);
                return Err(format!(
                    "{}: caller_identity={} required_by_rule={}",
                    REASON_CODE_AUTH_ASSURANCE_INSUFFICIENT, caller_identity, decision.rule_id
                ));
            }
        };

        let receipt = ReceiptBuilder::new(
            success_receipt_id.clone(),
            "earn.wallet.withdraw_submitted.v1",
            occurred_at_epoch_ms.max(0),
            idempotency_key,
            TraceContext {
                session_id: None,
                trajectory_hash: None,
                job_hash: None,
                run_id: Some(format!("wallet_withdraw:{caller_identity}")),
                work_unit_id: None,
                contract_id: None,
                claim_id: None,
            },
            policy,
        )
        .with_inputs_payload(json!({
            "caller_identity": caller_identity,
            "payment_request_digest": payment_request_digest,
            "amount_sats": if amount_sats == 0 { None::<u64> } else { Some(amount_sats) },
        }))
        .with_outputs_payload(json!({
            "status": "submitted",
            "policy_rule_id": policy_decision.rule_id.clone(),
            "policy_decision": policy_decision.decision,
            "policy_notes": policy_decision.notes.clone(),
            "source_tag": source_tag,
        }))
        .with_evidence(vec![
            EvidenceRef::new(
                "wallet_send_request",
                format!("oa://wallet/withdraw/{caller_identity}"),
                payment_request_digest,
            ),
            auth_evidence,
            policy_decision_evidence(&policy_decision),
        ])
        .with_hints(ReceiptHints {
            category: Some("compute".to_string()),
            tfb_class: Some(FeedbackLatencyClass::Instant),
            severity: Some(severity),
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: None,
            auth_assurance_level: Some(auth_assurance),
            personhood_proved: Some(personhood_proved),
            reason_code: None,
            notional: if amount_sats == 0 {
                None
            } else {
                Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(amount_sats),
                })
            },
            liability_premium: None,
        })
        .build();
        self.append_receipt(receipt, source_tag);
        if self.load_state == PaneLoadState::Error {
            return Err(self
                .last_error
                .clone()
                .unwrap_or_else(|| "failed to emit wallet withdraw receipt".to_string()));
        }
        Ok(success_receipt_id)
    }

    pub fn record_swap_execute_attempt(
        &mut self,
        caller_identity: &str,
        goal_id: &str,
        quote_id: &str,
        worker_request_id: u64,
        occurred_at_epoch_ms: i64,
        source_tag: &str,
    ) -> Result<String, String> {
        let caller_identity = caller_identity.trim();
        if caller_identity.is_empty() {
            return Err("caller_identity cannot be empty".to_string());
        }
        let goal_id = goal_id.trim();
        if goal_id.is_empty() {
            return Err("goal_id cannot be empty".to_string());
        }
        let quote_id = quote_id.trim();
        if quote_id.is_empty() {
            return Err("quote_id cannot be empty".to_string());
        }

        let action_digest = digest_for_text(format!("{goal_id}:{quote_id}").as_str());
        let idempotency_key = format!(
            "idemp.swap.execute:{}:{}",
            normalize_key(caller_identity),
            normalize_key(action_digest.as_str())
        );
        let receipt_id = format!(
            "receipt.earn:swap_execute:{}:{}",
            normalize_key(goal_id),
            normalize_key(quote_id)
        );
        let mut policy = current_policy_context();
        policy.approved_by = caller_identity.to_string();

        let receipt = ReceiptBuilder::new(
            receipt_id.clone(),
            "earn.swap.execute_submitted.v1",
            occurred_at_epoch_ms.max(0),
            idempotency_key,
            TraceContext {
                session_id: None,
                trajectory_hash: None,
                job_hash: None,
                run_id: Some(format!("swap_execute:{goal_id}:{quote_id}")),
                work_unit_id: None,
                contract_id: None,
                claim_id: None,
            },
            policy,
        )
        .with_inputs_payload(json!({
            "caller_identity": caller_identity,
            "goal_id": goal_id,
            "quote_id": quote_id,
            "worker_request_id": worker_request_id,
        }))
        .with_outputs_payload(json!({
            "status": "submitted",
            "source_tag": source_tag,
        }))
        .with_evidence(vec![EvidenceRef::new(
            "swap_execute_intent",
            format!("oa://swap/execute/{goal_id}/{quote_id}"),
            action_digest,
        )])
        .with_hints(ReceiptHints {
            category: Some("compute".to_string()),
            tfb_class: Some(FeedbackLatencyClass::Short),
            severity: Some(SeverityClass::Medium),
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: None,
            auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
            personhood_proved: Some(false),
            reason_code: None,
            notional: None,
            liability_premium: None,
        })
        .build();

        self.append_receipt(receipt, source_tag);
        if self.load_state == PaneLoadState::Error {
            return Err(self
                .last_error
                .clone()
                .unwrap_or_else(|| "failed to emit swap execute receipt".to_string()));
        }
        Ok(receipt_id)
    }

    fn resolve_or_create_work_unit_metadata(
        &mut self,
        work_unit_id: &str,
        demand_source: JobDemandSource,
        notional_sats: u64,
        ttl_seconds: Option<u64>,
    ) -> ResolvedWorkMetadata {
        if let Some(existing) = self.work_units.get(work_unit_id) {
            return ResolvedWorkMetadata {
                category: existing.category.clone(),
                tfb_class: existing.tfb_class,
                severity: existing.severity,
                verification_budget_hint_sats: existing.verification_budget_hint_sats,
                template_kind: existing.template_kind,
                acceptance_criteria_ref: existing.acceptance_criteria_ref.clone(),
                coordinated_disclosure_ref: existing.coordinated_disclosure_ref.clone(),
                mandatory_provenance: existing.mandatory_provenance,
                rollback_plan_ref: existing.rollback_plan_ref.clone(),
                compensating_action_plan_ref: existing.compensating_action_plan_ref.clone(),
            };
        }

        let category = work_category_for_demand_source(demand_source).to_string();
        let tfb_class = ttl_seconds.map_or(FeedbackLatencyClass::Short, tfb_class_for_ttl_seconds);
        let severity = severity_for_notional_sats(notional_sats);
        let verification_budget_hint_sats =
            verification_budget_hint_sats(category.as_str(), tfb_class, severity);
        self.work_units.insert(
            work_unit_id.to_string(),
            WorkUnitMetadata {
                work_unit_id: work_unit_id.to_string(),
                category: category.clone(),
                tfb_class,
                severity,
                verification_budget_hint_sats,
                template_kind: None,
                acceptance_criteria_ref: None,
                coordinated_disclosure_ref: None,
                mandatory_provenance: false,
                rollback_plan_ref: None,
                compensating_action_plan_ref: None,
            },
        );
        normalize_work_units(&mut self.work_units);
        ResolvedWorkMetadata {
            category,
            tfb_class,
            severity,
            verification_budget_hint_sats,
            template_kind: None,
            acceptance_criteria_ref: None,
            coordinated_disclosure_ref: None,
            mandatory_provenance: false,
            rollback_plan_ref: None,
            compensating_action_plan_ref: None,
        }
    }

    pub fn set_work_unit_rollback_terms(
        &mut self,
        work_unit_id: &str,
        rollback_plan_ref: Option<&str>,
        compensating_action_plan_ref: Option<&str>,
    ) -> Result<(), String> {
        let work_unit_id = work_unit_id.trim();
        if work_unit_id.is_empty() {
            return Err("work_unit_id cannot be empty".to_string());
        }
        let rollback_plan_ref = sanitize_optional_ref(rollback_plan_ref);
        let compensating_action_plan_ref = sanitize_optional_ref(compensating_action_plan_ref);
        if rollback_plan_ref.is_none() && compensating_action_plan_ref.is_none() {
            return Err(
                "at least one rollback or compensating_action plan reference is required"
                    .to_string(),
            );
        }

        let Some(metadata) = self.work_units.get_mut(work_unit_id) else {
            return Err(format!("work_unit metadata not found for {work_unit_id}"));
        };
        metadata.rollback_plan_ref = rollback_plan_ref;
        metadata.compensating_action_plan_ref = compensating_action_plan_ref;
        normalize_work_units(&mut self.work_units);
        self.persist_current_state()?;
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(format!(
            "Configured rollback terms for work unit {}",
            work_unit_id
        ));
        Ok(())
    }

    pub fn register_white_hat_work_unit_template(
        &mut self,
        draft: WhiteHatWorkUnitTemplateDraft,
        registered_at_ms: i64,
        source_tag: &str,
    ) -> Result<String, String> {
        let work_unit_id = draft.work_unit_id.trim();
        if work_unit_id.is_empty() {
            return Err("work_unit_id cannot be empty".to_string());
        }
        let idempotency_key = draft.idempotency_key.trim();
        if idempotency_key.is_empty() {
            return Err("idempotency_key cannot be empty".to_string());
        }
        if !draft.mandatory_provenance {
            return Err("white-hat template requires mandatory_provenance=true".to_string());
        }
        let acceptance_criteria_ref =
            sanitize_optional_ref(Some(draft.acceptance_criteria_ref.as_str()))
                .ok_or_else(|| "acceptance_criteria_ref cannot be empty".to_string())?;
        let coordinated_disclosure_ref =
            sanitize_optional_ref(Some(draft.coordinated_disclosure_ref.as_str()))
                .ok_or_else(|| "coordinated_disclosure_ref cannot be empty".to_string())?;
        let category = draft.kind.label().to_string();
        let verification_budget_hint_sats =
            draft.verification_budget_hint_sats.unwrap_or_else(|| {
                verification_budget_hint_sats(category.as_str(), draft.tfb_class, draft.severity)
            });
        let metadata = WorkUnitMetadata {
            work_unit_id: work_unit_id.to_string(),
            category: category.clone(),
            tfb_class: draft.tfb_class,
            severity: draft.severity,
            verification_budget_hint_sats,
            template_kind: Some(draft.kind),
            acceptance_criteria_ref: Some(acceptance_criteria_ref.clone()),
            coordinated_disclosure_ref: Some(coordinated_disclosure_ref.clone()),
            mandatory_provenance: true,
            rollback_plan_ref: None,
            compensating_action_plan_ref: None,
        };
        if let Some(existing) = self.work_units.get(work_unit_id)
            && existing != &metadata
        {
            return Err(format!(
                "work_unit {} already exists with conflicting template metadata",
                work_unit_id
            ));
        }
        self.work_units
            .insert(work_unit_id.to_string(), metadata.clone());
        normalize_work_units(&mut self.work_units);

        let policy_decision = allow_policy_decision(
            "white_hat_template_register",
            category.as_str(),
            draft.severity,
        );
        let mut evidence = vec![policy_decision_evidence(&policy_decision)];
        evidence.push(EvidenceRef::new(
            "acceptance_criteria_ref",
            acceptance_criteria_ref.clone(),
            digest_for_text(acceptance_criteria_ref.as_str()),
        ));
        evidence.push(EvidenceRef::new(
            "coordinated_disclosure_ref",
            coordinated_disclosure_ref.clone(),
            digest_for_text(coordinated_disclosure_ref.as_str()),
        ));

        let receipt_id = format!(
            "receipt.economy.work_unit.template:{}:{}",
            normalize_key(category.as_str()),
            normalize_key(work_unit_id)
        );
        let resolved_metadata = ResolvedWorkMetadata {
            category: metadata.category.clone(),
            tfb_class: metadata.tfb_class,
            severity: metadata.severity,
            verification_budget_hint_sats: metadata.verification_budget_hint_sats,
            template_kind: metadata.template_kind,
            acceptance_criteria_ref: metadata.acceptance_criteria_ref.clone(),
            coordinated_disclosure_ref: metadata.coordinated_disclosure_ref.clone(),
            mandatory_provenance: metadata.mandatory_provenance,
            rollback_plan_ref: metadata.rollback_plan_ref.clone(),
            compensating_action_plan_ref: metadata.compensating_action_plan_ref.clone(),
        };
        let receipt = ReceiptBuilder::new(
            receipt_id.clone(),
            "economy.work_unit.template_registered.v1",
            registered_at_ms.max(0),
            format!(
                "idemp.economy.work_unit.template:{}",
                normalize_key(idempotency_key)
            ),
            TraceContext {
                session_id: None,
                trajectory_hash: None,
                job_hash: None,
                run_id: Some(format!("white_hat_template:{work_unit_id}")),
                work_unit_id: Some(work_unit_id.to_string()),
                contract_id: None,
                claim_id: None,
            },
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "work_unit_id": work_unit_id,
            "template_kind": draft.kind.label(),
            "acceptance_criteria_digest": digest_for_text(acceptance_criteria_ref.as_str()),
            "coordinated_disclosure_digest": digest_for_text(coordinated_disclosure_ref.as_str()),
            "mandatory_provenance": true,
            "work_unit": work_unit_metadata_payload(work_unit_id, &resolved_metadata),
        }))
        .with_outputs_payload(json!({
            "status": "template_registered",
            "work_unit_id": work_unit_id,
            "template_kind": draft.kind.label(),
            "source_tag": source_tag,
        }))
        .with_evidence(evidence)
        .with_hints(ReceiptHints {
            category: Some(category),
            tfb_class: Some(draft.tfb_class),
            severity: Some(draft.severity),
            achieved_verification_tier: Some(VerificationTier::Tier2Heterogeneous),
            verification_correlated: Some(false),
            provenance_grade: Some(ProvenanceGrade::P2Lineage),
            auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
            personhood_proved: Some(false),
            reason_code: Some(REASON_CODE_WORK_UNIT_TEMPLATE_REGISTERED.to_string()),
            notional: None,
            liability_premium: None,
        })
        .build();
        self.append_receipt(receipt, source_tag);
        if self.load_state == PaneLoadState::Error {
            return Err(self
                .last_error
                .clone()
                .unwrap_or_else(|| "failed to register white-hat work unit template".to_string()));
        }
        Ok(receipt_id)
    }

    pub fn settle_white_hat_bounty(
        &mut self,
        draft: WhiteHatBountySettlementDraft,
        settled_at_ms: i64,
        source_tag: &str,
    ) -> Result<WhiteHatBountySettlementResult, String> {
        let work_unit_id = draft.work_unit_id.trim();
        if work_unit_id.is_empty() {
            return Err("work_unit_id cannot be empty".to_string());
        }
        let idempotency_key = draft.idempotency_key.trim();
        if idempotency_key.is_empty() {
            return Err("idempotency_key cannot be empty".to_string());
        }
        let finding_id = draft.finding_id.trim();
        if finding_id.is_empty() {
            return Err("finding_id cannot be empty".to_string());
        }
        if draft.verdict_receipt_id.trim().is_empty() {
            return Err("verdict_receipt_id cannot be empty".to_string());
        }
        if !draft.disputed && draft.payout_sats == 0 {
            return Err("payout_sats must be > 0 for non-disputed bounty settlement".to_string());
        }
        if draft.disputed {
            if draft.dispute_bond_sats == 0 {
                return Err("disputed bounty settlement requires dispute_bond_sats > 0".to_string());
            }
            if draft
                .dispute_reason
                .as_deref()
                .map(str::trim)
                .is_none_or(str::is_empty)
            {
                return Err("disputed bounty settlement requires dispute_reason".to_string());
            }
        }
        let metadata = self
            .work_units
            .get(work_unit_id)
            .cloned()
            .ok_or_else(|| format!("work_unit metadata not found for {work_unit_id}"))?;
        if !is_white_hat_category(metadata.category.as_str()) {
            return Err(format!(
                "work_unit {} is not a white-hat template category",
                work_unit_id
            ));
        }
        if !metadata.mandatory_provenance {
            return Err(format!(
                "work_unit {} does not enforce mandatory provenance",
                work_unit_id
            ));
        }
        let verdict_receipt = self
            .get_receipt(draft.verdict_receipt_id.as_str())
            .cloned()
            .ok_or_else(|| format!("verdict receipt {} not found", draft.verdict_receipt_id))?;
        if !is_verdict_receipt_type(verdict_receipt.receipt_type.as_str()) {
            return Err(format!(
                "receipt {} is not a verdict receipt",
                draft.verdict_receipt_id
            ));
        }
        let escrow_receipt_id = sanitize_optional_ref(draft.escrow_receipt_id.as_deref());
        if let Some(escrow_receipt_id) = escrow_receipt_id.as_deref()
            && self.get_receipt(escrow_receipt_id).is_none()
        {
            return Err(format!("escrow receipt {} not found", escrow_receipt_id));
        }

        let settled_at_ms = settled_at_ms.max(0);
        let finding_digest = digest_for_text(finding_id);
        let finding_key = finding_digest.strip_prefix("sha256:").unwrap_or("finding");
        let normalized_work_unit_id = normalize_key(work_unit_id);
        let normalized_idempotency_key = normalize_key(idempotency_key);
        let policy_decision = allow_policy_decision(
            "white_hat_bounty_settlement",
            metadata.category.as_str(),
            metadata.severity,
        );

        let mut dispute_receipt_id = None::<String>;
        if draft.disputed {
            let dispute_id = format!(
                "receipt.economy.bounty.dispute:{}:{}",
                normalized_work_unit_id, finding_key
            );
            let dispute_reason = draft.dispute_reason.as_deref().unwrap_or_default().trim();
            let mut evidence = vec![
                policy_decision_evidence(&policy_decision),
                EvidenceRef::new(
                    "finding_ref",
                    format!("oa://economy/findings/{finding_digest}"),
                    finding_digest.clone(),
                ),
            ];
            let mut linked = vec![draft.verdict_receipt_id.clone()];
            if let Some(escrow_receipt_id) = escrow_receipt_id.as_deref() {
                linked.push(escrow_receipt_id.to_string());
            }
            self.append_receipt_reference_links(&mut evidence, linked.as_slice());
            let dispute_receipt = ReceiptBuilder::new(
                dispute_id.clone(),
                "economy.bounty.dispute.opened.v1",
                settled_at_ms,
                format!(
                    "idemp.economy.bounty.dispute:{}:{}",
                    normalized_idempotency_key, finding_key
                ),
                TraceContext {
                    session_id: None,
                    trajectory_hash: None,
                    job_hash: None,
                    run_id: Some(format!(
                        "white_hat_bounty_dispute:{work_unit_id}:{finding_key}"
                    )),
                    work_unit_id: Some(work_unit_id.to_string()),
                    contract_id: None,
                    claim_id: None,
                },
                current_policy_context(),
            )
            .with_inputs_payload(json!({
                "work_unit_id": work_unit_id,
                "finding_digest": finding_digest,
                "verdict_receipt_id": draft.verdict_receipt_id,
                "escrow_receipt_id": escrow_receipt_id,
                "dispute_reason": dispute_reason,
                "dispute_bond_sats": draft.dispute_bond_sats,
            }))
            .with_outputs_payload(json!({
                "status": "dispute_opened",
                "work_unit_id": work_unit_id,
                "finding_digest": finding_digest,
                "source_tag": source_tag,
            }))
            .with_evidence(evidence)
            .with_hints(ReceiptHints {
                category: Some(metadata.category.clone()),
                tfb_class: Some(metadata.tfb_class),
                severity: Some(metadata.severity),
                achieved_verification_tier: Some(VerificationTier::Tier2Heterogeneous),
                verification_correlated: Some(false),
                provenance_grade: Some(ProvenanceGrade::P2Lineage),
                auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
                personhood_proved: Some(false),
                reason_code: Some(REASON_CODE_BOUNTY_DISPUTE_OPENED.to_string()),
                notional: Some(btc_sats_money(draft.dispute_bond_sats)),
                liability_premium: None,
            })
            .build();
            self.append_receipt(dispute_receipt, source_tag);
            if self.load_state == PaneLoadState::Error {
                return Err(self
                    .last_error
                    .clone()
                    .unwrap_or_else(|| "failed to emit bounty dispute receipt".to_string()));
            }
            dispute_receipt_id = Some(dispute_id);
        }

        let settlement_outcome = if draft.disputed { "withheld" } else { "paid" };
        let settlement_reason_code = if draft.disputed {
            REASON_CODE_BOUNTY_SETTLEMENT_WITHHELD
        } else {
            REASON_CODE_BOUNTY_SETTLEMENT_FINALIZED
        };
        let payout_sats = if draft.disputed { 0 } else { draft.payout_sats };
        let settlement_receipt_id = format!(
            "receipt.economy.bounty.settlement:{}:{}",
            normalized_work_unit_id, finding_key
        );
        let mut settlement_evidence = vec![
            policy_decision_evidence(&policy_decision),
            EvidenceRef::new(
                "finding_ref",
                format!("oa://economy/findings/{finding_digest}"),
                finding_digest.clone(),
            ),
        ];
        let mut linked_receipt_ids = vec![draft.verdict_receipt_id.clone()];
        if let Some(dispute_receipt_id) = dispute_receipt_id.as_ref() {
            linked_receipt_ids.push(dispute_receipt_id.clone());
        }
        if let Some(escrow_receipt_id) = escrow_receipt_id.as_ref() {
            linked_receipt_ids.push(escrow_receipt_id.clone());
        }
        self.append_receipt_reference_links(
            &mut settlement_evidence,
            linked_receipt_ids.as_slice(),
        );
        let settlement_receipt = ReceiptBuilder::new(
            settlement_receipt_id.clone(),
            "economy.bounty.settlement.finalized.v1",
            settled_at_ms,
            format!(
                "idemp.economy.bounty.settlement:{}:{}",
                normalized_idempotency_key, finding_key
            ),
            TraceContext {
                session_id: None,
                trajectory_hash: None,
                job_hash: None,
                run_id: Some(format!(
                    "white_hat_bounty_settlement:{work_unit_id}:{finding_key}"
                )),
                work_unit_id: Some(work_unit_id.to_string()),
                contract_id: None,
                claim_id: None,
            },
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "work_unit_id": work_unit_id,
            "finding_digest": finding_digest,
            "verdict_receipt_id": draft.verdict_receipt_id,
            "dispute_receipt_id": dispute_receipt_id,
            "escrow_receipt_id": escrow_receipt_id,
            "payout_requested_sats": draft.payout_sats,
            "dispute_bond_sats": draft.dispute_bond_sats,
            "disputed": draft.disputed,
        }))
        .with_outputs_payload(json!({
            "status": "settlement_finalized",
            "work_unit_id": work_unit_id,
            "finding_digest": finding_digest,
            "settlement_outcome": settlement_outcome,
            "payout_sats": payout_sats,
            "source_tag": source_tag,
        }))
        .with_evidence(settlement_evidence)
        .with_hints(ReceiptHints {
            category: Some(metadata.category.clone()),
            tfb_class: Some(metadata.tfb_class),
            severity: Some(metadata.severity),
            achieved_verification_tier: Some(VerificationTier::Tier2Heterogeneous),
            verification_correlated: Some(false),
            provenance_grade: Some(ProvenanceGrade::P2Lineage),
            auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
            personhood_proved: Some(false),
            reason_code: Some(settlement_reason_code.to_string()),
            notional: Some(btc_sats_money(payout_sats)),
            liability_premium: None,
        })
        .build();
        self.append_receipt(settlement_receipt, source_tag);
        if self.load_state == PaneLoadState::Error {
            return Err(self
                .last_error
                .clone()
                .unwrap_or_else(|| "failed to emit bounty settlement receipt".to_string()));
        }

        let mut incident_tags = vec!["safety.white_hat_finding".to_string()];
        if draft.disputed {
            incident_tags.push("finance.claim_dispute".to_string());
        }
        let outcome_entry_id = self.record_outcome_registry_entry(
            OutcomeRegistryEntryDraft {
                idempotency_key: format!("{idempotency_key}:bounty_outcome"),
                category: metadata.category.clone(),
                tfb_class: metadata.tfb_class,
                severity: metadata.severity,
                verdict_outcome: if draft.disputed {
                    "finding_disputed".to_string()
                } else {
                    "finding_verified".to_string()
                },
                settlement_outcome: settlement_outcome.to_string(),
                claim_outcome: Some(if draft.disputed {
                    "disputed".to_string()
                } else {
                    "none".to_string()
                }),
                remedy_outcome: Some(if draft.disputed {
                    "dispute_open".to_string()
                } else {
                    "bounty_paid".to_string()
                }),
                incident_tags,
                linked_receipt_ids: linked_receipt_ids
                    .into_iter()
                    .chain([settlement_receipt_id.clone()])
                    .collect::<Vec<_>>(),
                evidence_digests: vec![finding_digest.clone()],
            },
            settled_at_ms,
            source_tag,
        )?;

        let incident_id = if let Some(mut incident_report) = draft.incident_report {
            let mut incident_links = incident_report.linked_receipt_ids.clone();
            incident_links.push(settlement_receipt_id.clone());
            incident_links.push(draft.verdict_receipt_id.clone());
            if let Some(dispute_receipt_id) = dispute_receipt_id.as_ref() {
                incident_links.push(dispute_receipt_id.clone());
            }
            incident_report.linked_receipt_ids = canonical_receipt_ids(incident_links.as_slice());
            Some(self.report_incident(incident_report, settled_at_ms, source_tag)?)
        } else {
            None
        };

        Ok(WhiteHatBountySettlementResult {
            settlement_receipt_id,
            dispute_receipt_id,
            outcome_entry_id,
            incident_id,
        })
    }

    fn normalized_work_units(&self) -> Vec<WorkUnitMetadata> {
        let mut rows = self.work_units.values().cloned().collect::<Vec<_>>();
        rows.sort_by(|lhs, rhs| lhs.work_unit_id.cmp(&rhs.work_unit_id));
        rows.truncate(EARN_WORK_UNIT_METADATA_ROW_LIMIT);
        rows
    }

    fn normalized_idempotency_records(&self) -> Vec<IdempotencyRecord> {
        let mut rows = self.idempotency_index.values().cloned().collect::<Vec<_>>();
        rows.sort_by(|lhs, rhs| {
            lhs.scope
                .cmp(&rhs.scope)
                .then_with(|| lhs.idempotency_key.cmp(&rhs.idempotency_key))
        });
        rows.truncate(EARN_IDEMPOTENCY_RECORD_ROW_LIMIT);
        rows
    }

    fn normalized_incident_taxonomy_entries(&self) -> Vec<IncidentTaxonomyEntry> {
        let mut rows = self
            .incident_taxonomy_registry
            .values()
            .cloned()
            .collect::<Vec<_>>();
        rows.sort_by(|lhs, rhs| incident_taxonomy_key(lhs).cmp(&incident_taxonomy_key(rhs)));
        rows.truncate(INCIDENT_TAXONOMY_ROW_LIMIT);
        rows
    }

    fn normalized_outcome_registry_entries(&self) -> Vec<OutcomeRegistryEntry> {
        let mut rows = self.outcome_registry_entries.clone();
        rows.sort_by(|lhs, rhs| {
            lhs.entry_id
                .cmp(&rhs.entry_id)
                .then_with(|| rhs.revision.cmp(&lhs.revision))
                .then_with(|| rhs.updated_at_ms.cmp(&lhs.updated_at_ms))
        });
        rows.truncate(OUTCOME_REGISTRY_ROW_LIMIT);
        rows
    }

    fn persist_current_state(&mut self) -> Result<(), String> {
        persist_earn_kernel_receipts(
            self.receipt_file_path.as_path(),
            self.receipts.as_slice(),
            self.normalized_work_units().as_slice(),
            self.normalized_idempotency_records().as_slice(),
            self.incident_objects.as_slice(),
            self.normalized_outcome_registry_entries().as_slice(),
            self.normalized_incident_taxonomy_entries().as_slice(),
        )
    }

    pub fn get_receipt(&self, receipt_id: &str) -> Option<&Receipt> {
        self.receipts
            .iter()
            .find(|receipt| receipt.receipt_id == receipt_id)
    }

    pub fn query_receipts<'a>(&'a self, query: &ReceiptQuery) -> Vec<&'a Receipt> {
        let start = query.start_inclusive_ms.unwrap_or(i64::MIN);
        let end = query.end_inclusive_ms.unwrap_or(i64::MAX);
        let work_unit_id = query.work_unit_id.as_deref().map(str::trim);
        let receipt_type = query.receipt_type.as_deref().map(str::trim);
        let mut rows = self
            .receipts
            .iter()
            .filter(|receipt| receipt.created_at_ms >= start && receipt.created_at_ms <= end)
            .filter(|receipt| {
                work_unit_id.is_none_or(|value| {
                    receipt.trace.work_unit_id.as_deref().map(str::trim) == Some(value)
                })
            })
            .filter(|receipt| receipt_type.is_none_or(|value| receipt.receipt_type == value))
            .collect::<Vec<_>>();
        rows.sort_by(|lhs, rhs| {
            lhs.created_at_ms
                .cmp(&rhs.created_at_ms)
                .then_with(|| lhs.receipt_id.cmp(&rhs.receipt_id))
        });
        rows
    }

    pub fn export_receipt_bundle(
        &self,
        query: &ReceiptQuery,
        generated_at_ms: i64,
    ) -> Result<ReceiptBundle, String> {
        let receipts = self
            .query_receipts(query)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let receipt_ids = receipts
            .iter()
            .map(|receipt| receipt.receipt_id.clone())
            .collect::<Vec<_>>();
        let bundle_hash = hash_receipt_bundle(query, receipts.as_slice())?;
        Ok(ReceiptBundle {
            schema_version: EARN_KERNEL_RECEIPT_SCHEMA_VERSION,
            generated_at_ms: generated_at_ms.max(0),
            stream_id: self.stream_id.clone(),
            authority: self.authority.clone(),
            query: query.clone(),
            receipt_count: receipts.len(),
            receipt_ids,
            bundle_hash,
            receipts,
        })
    }

    pub fn export_receipt_bundle_to_path(
        &self,
        query: &ReceiptQuery,
        generated_at_ms: i64,
        path: &Path,
    ) -> Result<ReceiptBundle, String> {
        let bundle = self.export_receipt_bundle(query, generated_at_ms)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create receipt bundle dir: {error}"))?;
        }
        let payload = serde_json::to_string_pretty(&bundle)
            .map_err(|error| format!("Failed to encode receipt bundle: {error}"))?;
        let temp_path = path.with_extension("tmp");
        std::fs::write(&temp_path, payload)
            .map_err(|error| format!("Failed to write receipt bundle temp file: {error}"))?;
        std::fs::rename(&temp_path, path)
            .map_err(|error| format!("Failed to persist receipt bundle: {error}"))?;
        Ok(bundle)
    }

    pub fn export_audit_package(
        &self,
        query: &ReceiptQuery,
        tier: AuditExportRedactionTier,
        generated_at_ms: i64,
    ) -> Result<AuditPackage, String> {
        let receipts = self
            .query_receipts(query)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let included_receipt_ids = receipts
            .iter()
            .map(|receipt| receipt.receipt_id.clone())
            .collect::<BTreeSet<_>>();

        let mut incidents = self
            .incident_objects
            .iter()
            .filter(|incident| {
                incident
                    .linked_receipt_ids
                    .iter()
                    .any(|receipt_id| included_receipt_ids.contains(receipt_id))
                    || incident
                        .rollback_receipt_ids
                        .iter()
                        .any(|receipt_id| included_receipt_ids.contains(receipt_id))
            })
            .cloned()
            .collect::<Vec<_>>();
        incidents = normalize_incident_objects(incidents);

        let mut certifications = receipts
            .iter()
            .filter(|receipt| receipt.receipt_type.starts_with("economy.certification."))
            .map(|receipt| AuditCertificationEntry {
                receipt_id: receipt.receipt_id.clone(),
                receipt_type: receipt.receipt_type.clone(),
                category: receipt.hints.category.clone(),
                severity: receipt.hints.severity,
                digest: receipt.canonical_hash.clone(),
            })
            .collect::<Vec<_>>();
        certifications.sort_by(|lhs, rhs| lhs.receipt_id.cmp(&rhs.receipt_id));
        let mut certification_objects =
            latest_certification_objects_as_of(self.receipts.as_slice(), generated_at_ms.max(0))
                .into_iter()
                .filter(|certification| {
                    certification
                        .linked_receipt_ids
                        .iter()
                        .any(|receipt_id| included_receipt_ids.contains(receipt_id))
                })
                .map(|certification| AuditCertificationObject {
                    certification_id: certification.certification_id,
                    certification_digest: certification.certification_digest,
                    revision: certification.revision,
                    state: certification.state,
                    certification_level: certification.certification_level,
                    scope: certification.scope,
                    valid_from_ms: certification.valid_from_ms,
                    valid_until_ms: certification.valid_until_ms,
                    issuer_credential_kind: certification.issuer_credential_kind,
                    issuer_credential_digest: certification.issuer_credential_digest,
                    required_evidence_digests: certification.required_evidence_digests,
                    linked_receipt_ids: certification.linked_receipt_ids,
                    revoked_reason_code: certification.revoked_reason_code,
                    policy_bundle_id: certification.policy_bundle_id,
                    policy_version: certification.policy_version,
                    supersedes_digest: certification.supersedes_digest,
                })
                .collect::<Vec<_>>();
        certification_objects.sort_by(|lhs, rhs| {
            lhs.certification_id
                .cmp(&rhs.certification_id)
                .then_with(|| rhs.revision.cmp(&lhs.revision))
                .then_with(|| lhs.certification_digest.cmp(&rhs.certification_digest))
        });
        let mut anchors = receipts
            .iter()
            .filter(|receipt| receipt.receipt_type == "economy.anchor.published.v1")
            .filter_map(audit_anchor_entry_from_receipt)
            .collect::<Vec<_>>();
        anchors.sort_by(|lhs, rhs| lhs.receipt_id.cmp(&rhs.receipt_id));

        let mut outcome_registry_entries = receipts
            .iter()
            .filter(|receipt| {
                receipt
                    .receipt_type
                    .starts_with("economy.outcome_registry.")
            })
            .map(|receipt| AuditOutcomeRegistryEntry {
                receipt_id: receipt.receipt_id.clone(),
                receipt_type: receipt.receipt_type.clone(),
                category: receipt.hints.category.clone(),
                tfb_class: receipt.hints.tfb_class,
                severity: receipt.hints.severity,
                digest: receipt.canonical_hash.clone(),
            })
            .collect::<Vec<_>>();
        outcome_registry_entries.sort_by(|lhs, rhs| lhs.receipt_id.cmp(&rhs.receipt_id));

        let mut outcome_registry_objects = self
            .outcome_registry_entries
            .iter()
            .filter(|entry| {
                entry
                    .linked_receipt_ids
                    .iter()
                    .any(|receipt_id| included_receipt_ids.contains(receipt_id))
            })
            .cloned()
            .map(|entry| AuditOutcomeRegistryObject {
                entry_id: entry.entry_id,
                entry_digest: entry.entry_digest,
                revision: entry.revision,
                category: entry.category,
                tfb_class: entry.tfb_class,
                severity: entry.severity,
                verdict_outcome: entry.verdict_outcome,
                settlement_outcome: entry.settlement_outcome,
                claim_outcome: entry.claim_outcome,
                remedy_outcome: entry.remedy_outcome,
                incident_tags: entry.incident_tags,
                linked_receipt_ids: entry.linked_receipt_ids,
            })
            .collect::<Vec<_>>();
        outcome_registry_objects.sort_by(|lhs, rhs| {
            lhs.entry_id
                .cmp(&rhs.entry_id)
                .then_with(|| rhs.revision.cmp(&lhs.revision))
                .then_with(|| lhs.entry_digest.cmp(&rhs.entry_digest))
        });

        let snapshot_bindings = audit_snapshot_bindings(receipts.as_slice());
        let linkage_edges = audit_linkage_edges(
            receipts.as_slice(),
            incidents.as_slice(),
            certification_objects.as_slice(),
            anchors.as_slice(),
            outcome_registry_objects.as_slice(),
        );

        let mut package_receipts = receipts.clone();
        if tier == AuditExportRedactionTier::Public {
            redact_receipts_for_public_export(&mut package_receipts);
            redact_incidents_for_public_export(&mut incidents);
            for certification in &mut certification_objects {
                certification.linked_receipt_ids.clear();
                certification.issuer_credential_digest = digest_for_text("redacted");
            }
            for outcome in &mut outcome_registry_objects {
                outcome.linked_receipt_ids.clear();
            }
        }

        let package_hash = hash_audit_package(
            query,
            tier,
            package_receipts.as_slice(),
            incidents.as_slice(),
            certifications.as_slice(),
            certification_objects.as_slice(),
            anchors.as_slice(),
            outcome_registry_entries.as_slice(),
            outcome_registry_objects.as_slice(),
            snapshot_bindings.as_slice(),
            linkage_edges.as_slice(),
        )?;
        Ok(AuditPackage {
            schema_version: EARN_KERNEL_RECEIPT_SCHEMA_VERSION,
            generated_at_ms: generated_at_ms.max(0),
            stream_id: self.stream_id.clone(),
            authority: self.authority.clone(),
            redaction_tier: tier,
            query: query.clone(),
            receipt_count: package_receipts.len(),
            incident_count: incidents.len(),
            certification_count: certifications.len(),
            certification_object_count: certification_objects.len(),
            anchor_count: anchors.len(),
            outcome_registry_count: outcome_registry_entries.len(),
            outcome_registry_object_count: outcome_registry_objects.len(),
            snapshot_binding_count: snapshot_bindings.len(),
            linkage_edge_count: linkage_edges.len(),
            receipts: package_receipts,
            incidents,
            certifications,
            certification_objects,
            anchors,
            outcome_registry_entries,
            outcome_registry_objects,
            snapshot_bindings,
            linkage_edges,
            package_hash,
        })
    }

    pub fn export_audit_package_to_path(
        &self,
        query: &ReceiptQuery,
        tier: AuditExportRedactionTier,
        generated_at_ms: i64,
        path: &Path,
    ) -> Result<AuditPackage, String> {
        let package = self.export_audit_package(query, tier, generated_at_ms)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create audit package dir: {error}"))?;
        }
        let payload = serde_json::to_string_pretty(&package)
            .map_err(|error| format!("Failed to encode audit package: {error}"))?;
        let temp_path = path.with_extension("tmp");
        std::fs::write(&temp_path, payload)
            .map_err(|error| format!("Failed to write audit package temp file: {error}"))?;
        std::fs::rename(&temp_path, path)
            .map_err(|error| format!("Failed to persist audit package: {error}"))?;
        Ok(package)
    }

    pub fn export_safety_signal_feed(
        &self,
        query: &ReceiptQuery,
        mode: SafetySignalExportMode,
        generated_at_ms: i64,
        reader_role: Option<&str>,
        caller_identity: Option<&str>,
    ) -> Result<SafetySignalFeed, String> {
        let receipts = self
            .query_receipts(query)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let all_signals = derive_safety_signals(receipts.as_slice());
        let buckets = aggregate_safety_signal_buckets(all_signals.as_slice());

        if mode == SafetySignalExportMode::RestrictedFeed {
            let role = canonical_safety_signal_reader_role(reader_role);
            let caller_identity = caller_identity.unwrap_or("").trim();
            enforce_restricted_safety_signal_access(
                all_signals.as_slice(),
                role.as_str(),
                caller_identity,
            )?;
        }

        let mut signals = all_signals.clone();
        if mode == SafetySignalExportMode::PublicAggregate {
            signals.clear();
        }
        let package_hash =
            hash_safety_signal_feed(query, mode, signals.as_slice(), buckets.as_slice())?;
        Ok(SafetySignalFeed {
            schema_version: EARN_KERNEL_RECEIPT_SCHEMA_VERSION,
            generated_at_ms: generated_at_ms.max(0),
            stream_id: self.stream_id.clone(),
            authority: self.authority.clone(),
            export_mode: mode,
            query: query.clone(),
            signal_count: signals.len(),
            bucket_count: buckets.len(),
            signals,
            buckets,
            package_hash,
        })
    }

    pub fn export_safety_signal_feed_to_path(
        &self,
        query: &ReceiptQuery,
        mode: SafetySignalExportMode,
        generated_at_ms: i64,
        reader_role: Option<&str>,
        caller_identity: Option<&str>,
        path: &Path,
    ) -> Result<SafetySignalFeed, String> {
        let feed = self.export_safety_signal_feed(
            query,
            mode,
            generated_at_ms,
            reader_role,
            caller_identity,
        )?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create safety signal feed dir: {error}"))?;
        }
        let payload = serde_json::to_string_pretty(&feed)
            .map_err(|error| format!("Failed to encode safety signal feed: {error}"))?;
        let temp_path = path.with_extension("tmp");
        std::fs::write(&temp_path, payload)
            .map_err(|error| format!("Failed to write safety signal feed temp file: {error}"))?;
        std::fs::rename(&temp_path, path)
            .map_err(|error| format!("Failed to persist safety signal feed: {error}"))?;
        Ok(feed)
    }

    pub fn export_simulation_scenario_package(
        &mut self,
        tier: SimulationScenarioExportRedactionTier,
        generated_at_ms: i64,
        source_tag: &str,
    ) -> Result<SimulationScenarioPackage, String> {
        let as_of_ms = generated_at_ms.max(0);
        let scenarios = simulation_scenarios_as_of(
            self.incident_objects.as_slice(),
            self.receipts.as_slice(),
            as_of_ms,
        );
        let scenarios = normalize_simulation_scenarios(scenarios);
        let scenario_digests = scenarios
            .iter()
            .map(|scenario| scenario.scenario_digest.clone())
            .collect::<Vec<_>>();
        let export_seed_digest =
            digest_for_text(format!("{}:{}", tier.label(), scenario_digests.join("|")).as_str());
        let export_seed = normalize_key(export_seed_digest.as_str());
        let redaction_receipt_id = format!(
            "receipt.economy.simulation_scenario.redaction:{}:{}",
            tier.label(),
            export_seed
        );
        let export_receipt_id = format!(
            "receipt.economy.simulation_scenario.export:{}:{}",
            tier.label(),
            export_seed
        );
        let package_scenarios = redact_simulation_scenarios(scenarios.as_slice(), tier);
        let package_hash = hash_simulation_scenario_package(
            tier,
            package_scenarios.as_slice(),
            redaction_receipt_id.as_str(),
            export_receipt_id.as_str(),
        )?;
        let policy = current_policy_context();
        let policy_decision = allow_policy_decision(
            "simulation_scenario_export",
            "safety",
            SeverityClass::Medium,
        );
        let redaction_receipt = ReceiptBuilder::new(
            redaction_receipt_id.clone(),
            "economy.simulation_scenario.redaction_policy_applied.v1",
            as_of_ms,
            format!("simulation_scenario_redaction:{}:{}", tier.label(), export_seed),
            trace_for_job("work_unit:economy.simulation_scenario_export", None, None),
            policy.clone(),
        )
        .with_inputs_payload(json!({
            "redaction_tier": tier.label(),
            "scenario_count": package_scenarios.len(),
            "scenario_digests": scenario_digests,
        }))
        .with_outputs_payload(json!({
            "status": "applied",
            "reason_code": REASON_CODE_REDACTION_POLICY_APPLIED,
            "redaction_profile": {
                "redact_linked_receipt_ids": tier == SimulationScenarioExportRedactionTier::Public,
                "redact_rollback_receipt_ids": tier == SimulationScenarioExportRedactionTier::Public,
                "redact_derived_receipt_ids": tier == SimulationScenarioExportRedactionTier::Public,
            },
            "policy_rule_id": policy_decision.rule_id.clone(),
            "policy_decision": policy_decision.decision,
            "policy_notes": policy_decision.notes.clone(),
        }))
        .with_evidence(vec![
            policy_decision_evidence(&policy_decision),
            EvidenceRef::new(
                "redaction_policy_ref",
                format!(
                    "oa://economy/redaction/simulation_scenario/tier/{}",
                    tier.label()
                ),
                digest_for_text(
                    format!("simulation_scenario_redaction_policy:{}", tier.label()).as_str(),
                ),
            ),
            EvidenceRef::new(
                "simulation_scenario_package_seed",
                format!("oa://economy/simulation_scenario_package_seed/{export_seed}"),
                export_seed_digest.clone(),
            ),
        ])
        .with_hints(ReceiptHints {
            category: Some("safety".to_string()),
            tfb_class: Some(FeedbackLatencyClass::Long),
            severity: Some(SeverityClass::Medium),
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: Some(ProvenanceGrade::P2Lineage),
            auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
            personhood_proved: Some(false),
            reason_code: Some(REASON_CODE_REDACTION_POLICY_APPLIED.to_string()),
            notional: None,
            liability_premium: None,
        })
        .build();
        self.append_receipt(redaction_receipt, source_tag);
        if self.load_state == PaneLoadState::Error {
            return Err(self
                .last_error
                .clone()
                .unwrap_or_else(|| "failed to emit simulation redaction receipt".to_string()));
        }
        let redaction_receipt_digest = self
            .get_receipt(redaction_receipt_id.as_str())
            .map(|receipt| receipt.canonical_hash.clone())
            .unwrap_or_else(|| digest_for_text(redaction_receipt_id.as_str()));

        let mut export_evidence = vec![
            policy_decision_evidence(&policy_decision),
            EvidenceRef::new(
                "redaction_policy_receipt_ref",
                format!("oa://receipts/{redaction_receipt_id}"),
                redaction_receipt_digest,
            ),
            EvidenceRef::new(
                "simulation_scenario_package_ref",
                format!("oa://economy/simulation_scenario_packages/{export_seed}"),
                package_hash.clone(),
            ),
        ];
        for scenario in &package_scenarios {
            let mut scenario_ref = EvidenceRef::new(
                "simulation_scenario_ref",
                format!("oa://economy/simulation_scenarios/{}", scenario.scenario_id),
                scenario.scenario_digest.clone(),
            );
            scenario_ref.meta.insert(
                "ground_truth_case_id".to_string(),
                json!(scenario.ground_truth_case_id.clone()),
            );
            scenario_ref.meta.insert(
                "ground_truth_case_digest".to_string(),
                json!(scenario.ground_truth_case_digest.clone()),
            );
            scenario_ref.meta.insert(
                "taxonomy_code".to_string(),
                json!(scenario.taxonomy_code.clone()),
            );
            scenario_ref
                .meta
                .insert("severity".to_string(), json!(scenario.severity.label()));
            scenario_ref
                .meta
                .insert("redaction_tier".to_string(), json!(tier.label()));
            export_evidence.push(scenario_ref);
        }

        let export_receipt = ReceiptBuilder::new(
            export_receipt_id.clone(),
            "economy.simulation_scenario.exported.v1",
            as_of_ms,
            format!(
                "simulation_scenario_export:{}:{}",
                tier.label(),
                export_seed
            ),
            trace_for_job("work_unit:economy.simulation_scenario_export", None, None),
            policy,
        )
        .with_inputs_payload(json!({
            "redaction_tier": tier.label(),
            "scenario_count": package_scenarios.len(),
            "redaction_policy_receipt_id": redaction_receipt_id,
        }))
        .with_outputs_payload(json!({
            "status": "exported",
            "package_hash": package_hash,
            "reason_code": REASON_CODE_SIMULATION_SCENARIO_EXPORTED,
            "policy_rule_id": policy_decision.rule_id.clone(),
            "policy_decision": policy_decision.decision,
            "policy_notes": policy_decision.notes.clone(),
        }))
        .with_evidence(export_evidence)
        .with_hints(ReceiptHints {
            category: Some("safety".to_string()),
            tfb_class: Some(FeedbackLatencyClass::Long),
            severity: Some(SeverityClass::Medium),
            achieved_verification_tier: Some(VerificationTier::Tier2Heterogeneous),
            verification_correlated: Some(false),
            provenance_grade: Some(ProvenanceGrade::P2Lineage),
            auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
            personhood_proved: Some(false),
            reason_code: Some(REASON_CODE_SIMULATION_SCENARIO_EXPORTED.to_string()),
            notional: None,
            liability_premium: None,
        })
        .build();
        self.append_receipt(export_receipt, source_tag);
        if self.load_state == PaneLoadState::Error {
            return Err(self
                .last_error
                .clone()
                .unwrap_or_else(|| "failed to emit simulation export receipt".to_string()));
        }

        Ok(SimulationScenarioPackage {
            schema_version: EARN_KERNEL_RECEIPT_SCHEMA_VERSION,
            generated_at_ms: as_of_ms,
            stream_id: self.stream_id.clone(),
            authority: self.authority.clone(),
            redaction_tier: tier,
            redaction_policy_receipt_id: redaction_receipt_id,
            export_receipt_id,
            scenario_count: package_scenarios.len(),
            scenarios: package_scenarios,
            package_hash,
        })
    }

    pub fn export_simulation_scenario_package_to_path(
        &mut self,
        tier: SimulationScenarioExportRedactionTier,
        generated_at_ms: i64,
        source_tag: &str,
        path: &Path,
    ) -> Result<SimulationScenarioPackage, String> {
        let package = self.export_simulation_scenario_package(tier, generated_at_ms, source_tag)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("Failed to create simulation scenario package dir: {error}")
            })?;
        }
        let payload = serde_json::to_string_pretty(&package)
            .map_err(|error| format!("Failed to encode simulation scenario package: {error}"))?;
        let temp_path = path.with_extension("tmp");
        std::fs::write(&temp_path, payload).map_err(|error| {
            format!("Failed to write simulation scenario package temp file: {error}")
        })?;
        std::fs::rename(&temp_path, path)
            .map_err(|error| format!("Failed to persist simulation scenario package: {error}"))?;
        Ok(package)
    }

    pub fn publish_anchor_receipt(
        &mut self,
        draft: AnchorPublicationDraft,
        occurred_at_ms: i64,
        source_tag: &str,
    ) -> Result<String, String> {
        let snapshot_id = draft.snapshot_id.trim();
        if snapshot_id.is_empty() {
            return Err("snapshot_id cannot be empty".to_string());
        }
        let snapshot_hash = normalize_digest(draft.snapshot_hash.as_str());
        if !snapshot_hash.starts_with("sha256:") {
            return Err("snapshot_hash must be a sha256 digest".to_string());
        }
        let anchor_backend = normalize_key(draft.anchor_backend.as_str());
        if anchor_backend.is_empty() {
            return Err("anchor_backend cannot be empty".to_string());
        }
        let policy_bundle = current_policy_bundle();
        if !anchoring_backend_allowed(&policy_bundle, anchor_backend.as_str()) {
            return Err(format!(
                "anchoring backend {anchor_backend} is not allowed by policy"
            ));
        }
        let external_anchor_reference = draft.external_anchor_reference.trim();
        if external_anchor_reference.is_empty() {
            return Err("external_anchor_reference cannot be empty".to_string());
        }
        let external_anchor_digest = if external_anchor_reference.starts_with("sha256:") {
            normalize_digest(external_anchor_reference)
        } else {
            digest_for_text(external_anchor_reference)
        };
        let receipt_root_hash = draft.receipt_root_hash.and_then(|value| {
            let normalized = normalize_digest(value.as_str());
            if normalized.starts_with("sha256:") {
                Some(normalized)
            } else {
                None
            }
        });
        let receipt_id = format!(
            "receipt.economy.anchor.published:{}:{}",
            anchor_backend,
            normalize_key(snapshot_id)
        );
        let idempotency_key = format!(
            "economy_anchor_publish:{}:{}",
            anchor_backend,
            normalize_key(snapshot_id)
        );
        let policy_decision = allow_policy_decision("anchor_publish", "safety", SeverityClass::Low);
        let mut evidence = vec![
            policy_decision_evidence(&policy_decision),
            EvidenceRef::new(
                "snapshot_ref",
                format!("oa://economy/snapshots/{snapshot_id}"),
                snapshot_hash.clone(),
            ),
        ];
        if let Some(root_hash) = receipt_root_hash.clone() {
            evidence.push(EvidenceRef::new(
                "receipt_root_ref",
                format!("oa://economy/receipt_roots/{snapshot_id}"),
                root_hash,
            ));
        }
        let mut anchor_proof_ref = EvidenceRef::new(
            "anchor_proof_ref",
            format!("oa://anchors/{anchor_backend}/proof/{external_anchor_digest}"),
            external_anchor_digest.clone(),
        );
        anchor_proof_ref
            .meta
            .insert("anchor_backend".to_string(), json!(anchor_backend.clone()));
        anchor_proof_ref
            .meta
            .insert("snapshot_id".to_string(), json!(snapshot_id.to_string()));
        anchor_proof_ref
            .meta
            .insert("snapshot_hash".to_string(), json!(snapshot_hash.clone()));
        evidence.push(anchor_proof_ref);

        let receipt = ReceiptBuilder::new(
            receipt_id.clone(),
            "economy.anchor.published.v1",
            occurred_at_ms.max(0),
            idempotency_key,
            trace_for_job("work_unit:economy.anchor_publish", None, None),
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "snapshot_id": snapshot_id,
            "snapshot_hash": snapshot_hash,
            "anchor_backend": anchor_backend,
            "external_anchor_digest": external_anchor_digest,
            "receipt_root_hash": receipt_root_hash,
        }))
        .with_outputs_payload(json!({
            "status": "anchor_published",
            "reason_code": REASON_CODE_ANCHOR_PUBLISHED,
            "policy_rule_id": policy_decision.rule_id,
            "policy_decision": policy_decision.decision,
            "policy_notes": policy_decision.notes,
        }))
        .with_evidence(evidence)
        .with_hints(ReceiptHints {
            category: Some("safety".to_string()),
            tfb_class: Some(FeedbackLatencyClass::Long),
            severity: Some(SeverityClass::Low),
            achieved_verification_tier: Some(VerificationTier::Tier1Correlated),
            verification_correlated: Some(true),
            provenance_grade: Some(ProvenanceGrade::P1Toolchain),
            auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
            personhood_proved: Some(false),
            reason_code: Some(REASON_CODE_ANCHOR_PUBLISHED.to_string()),
            notional: None,
            liability_premium: None,
        })
        .build();
        self.append_receipt(receipt, source_tag);
        if self.load_state == PaneLoadState::Error {
            return Err(self
                .last_error
                .clone()
                .unwrap_or_else(|| "failed to publish anchor receipt".to_string()));
        }
        Ok(receipt_id)
    }

    pub fn issue_safety_certification(
        &mut self,
        draft: SafetyCertificationDraft,
        occurred_at_ms: i64,
        source_tag: &str,
    ) -> Result<String, String> {
        let certification_id = draft.certification_id.trim();
        if certification_id.is_empty() {
            return Err("certification_id cannot be empty".to_string());
        }
        let idempotency_key = draft.idempotency_key.trim();
        if idempotency_key.is_empty() {
            return Err("idempotency_key cannot be empty".to_string());
        }
        let certification_level = normalize_key(draft.certification_level.as_str());
        if certification_level.is_empty() {
            return Err("certification_level cannot be empty".to_string());
        }
        let scope = normalize_certification_scopes(draft.scope.as_slice())?;
        if scope.is_empty() {
            return Err("certification scope cannot be empty".to_string());
        }
        if draft.valid_until_ms <= draft.valid_from_ms {
            return Err("valid_until_ms must be greater than valid_from_ms".to_string());
        }
        let issuer_identity = draft.issuer_identity.trim();
        if issuer_identity.is_empty() {
            return Err("issuer_identity cannot be empty".to_string());
        }
        let linked_receipt_ids = canonical_receipt_ids(draft.linked_receipt_ids.as_slice());
        for receipt_id in &linked_receipt_ids {
            if self.get_receipt(receipt_id.as_str()).is_none() {
                return Err(format!("linked receipt {receipt_id} not found"));
            }
        }

        let as_of_ms = occurred_at_ms.max(0);
        let latest_existing =
            latest_certification_objects_as_of(self.receipts.as_slice(), as_of_ms)
                .into_iter()
                .find(|certification| certification.certification_id == certification_id);
        let revision = latest_existing
            .as_ref()
            .map_or(1, |certification| certification.revision.saturating_add(1));
        let supersedes_digest = latest_existing
            .as_ref()
            .map(|certification| certification.certification_digest.clone());

        let receipt_id = format!(
            "receipt.economy.certification.issue:{}:{}",
            normalize_key(certification_id),
            normalize_key(idempotency_key),
        );
        let issuer_auth_assurance = draft
            .issuer_auth_assurance_level
            .unwrap_or_else(|| auth_assurance_for_identity(issuer_identity));
        let issuer_credential_ref =
            credential_ref_for_identity(issuer_identity, issuer_auth_assurance);
        let mut required_evidence = draft.required_evidence.clone();
        required_evidence.sort_by(|lhs, rhs| {
            lhs.kind
                .cmp(&rhs.kind)
                .then_with(|| lhs.digest.cmp(&rhs.digest))
                .then_with(|| lhs.uri.cmp(&rhs.uri))
        });
        let required_evidence_digests = required_evidence
            .iter()
            .map(|evidence| normalize_digest(evidence.digest.as_str()))
            .collect::<Vec<_>>();
        let mut certification_linked_receipt_ids = linked_receipt_ids.clone();
        certification_linked_receipt_ids.push(receipt_id.clone());
        certification_linked_receipt_ids =
            canonical_receipt_ids(certification_linked_receipt_ids.as_slice());
        let policy = current_policy_context();
        let mut certification = SafetyCertification {
            certification_id: certification_id.to_string(),
            certification_digest: String::new(),
            revision,
            state: CertificationState::Active,
            certification_level: certification_level.clone(),
            scope: scope.clone(),
            valid_from_ms: draft.valid_from_ms.max(0),
            valid_until_ms: draft.valid_until_ms.max(0),
            issuer_credential_kind: issuer_credential_ref.kind.clone(),
            issuer_credential_digest: issuer_credential_ref.digest.clone(),
            required_evidence_digests,
            linked_receipt_ids: certification_linked_receipt_ids,
            issued_at_ms: as_of_ms,
            updated_at_ms: as_of_ms,
            revoked_reason_code: None,
            policy_bundle_id: policy.policy_bundle_id.clone(),
            policy_version: policy.policy_version.clone(),
            supersedes_digest,
        };
        certification.certification_digest = certification_digest_for(
            certification.certification_id.as_str(),
            certification.revision,
            certification.state,
            certification.certification_level.as_str(),
            certification.scope.as_slice(),
            certification.valid_from_ms,
            certification.valid_until_ms,
            certification.issuer_credential_kind.as_str(),
            certification.issuer_credential_digest.as_str(),
            certification.required_evidence_digests.as_slice(),
            certification.linked_receipt_ids.as_slice(),
            certification.issued_at_ms,
            certification.updated_at_ms,
            certification.revoked_reason_code.as_deref(),
            certification.policy_bundle_id.as_str(),
            certification.policy_version.as_str(),
            certification.supersedes_digest.as_deref(),
        );
        let certification_evidence = certification_object_evidence(&certification);
        let (hint_category, hint_tfb_class, hint_severity) =
            certification_hint_fields(certification.scope.as_slice());

        let mut evidence = Vec::<EvidenceRef>::new();
        evidence.push(certification_evidence);
        evidence.push(issuer_credential_ref);
        evidence.extend(required_evidence);
        self.append_receipt_reference_links(&mut evidence, linked_receipt_ids.as_slice());

        let receipt = ReceiptBuilder::new(
            receipt_id.clone(),
            "economy.certification.issued.v1",
            as_of_ms,
            idempotency_key.to_string(),
            TraceContext {
                session_id: None,
                trajectory_hash: None,
                job_hash: None,
                run_id: Some(format!(
                    "economy_certification_issue:{}",
                    normalize_key(certification_id)
                )),
                work_unit_id: None,
                contract_id: None,
                claim_id: None,
            },
            policy,
        )
        .with_inputs_payload(json!({
            "certification_id": certification_id,
            "certification_level": certification_level,
            "scope": certification_scope_payload(certification.scope.as_slice()),
            "valid_from_ms": certification.valid_from_ms,
            "valid_until_ms": certification.valid_until_ms,
            "issuer_identity_ref": normalize_key(issuer_identity),
            "required_evidence_count": certification.required_evidence_digests.len(),
            "linked_receipt_ids": linked_receipt_ids,
        }))
        .with_outputs_payload(json!({
            "status": "issued",
            "state": certification.state.label(),
            "revision": certification.revision,
            "certification_digest": certification.certification_digest,
            "safe_harbor_eligible": false,
            "source_tag": source_tag,
        }))
        .with_evidence(evidence)
        .with_hints(ReceiptHints {
            category: hint_category,
            tfb_class: hint_tfb_class,
            severity: hint_severity,
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: None,
            auth_assurance_level: Some(issuer_auth_assurance),
            personhood_proved: Some(personhood_proved_for_identity(
                issuer_identity,
                issuer_auth_assurance,
            )),
            reason_code: None,
            notional: None,
            liability_premium: None,
        })
        .build();
        self.append_receipt(receipt, source_tag);
        if self.load_state == PaneLoadState::Error {
            return Err(self
                .last_error
                .clone()
                .unwrap_or_else(|| "failed to emit certification issuance receipt".to_string()));
        }
        Ok(receipt_id)
    }

    pub fn revoke_safety_certification(
        &mut self,
        draft: SafetyCertificationRevocationDraft,
        occurred_at_ms: i64,
        source_tag: &str,
    ) -> Result<String, String> {
        let certification_id = draft.certification_id.trim();
        if certification_id.is_empty() {
            return Err("certification_id cannot be empty".to_string());
        }
        let idempotency_key = draft.idempotency_key.trim();
        if idempotency_key.is_empty() {
            return Err("idempotency_key cannot be empty".to_string());
        }
        let revoked_reason_code = canonical_reason_code(draft.reason_code.as_str());
        if revoked_reason_code.is_empty() {
            return Err("reason_code cannot be empty".to_string());
        }

        let as_of_ms = occurred_at_ms.max(0);
        let latest_existing =
            latest_certification_objects_as_of(self.receipts.as_slice(), as_of_ms)
                .into_iter()
                .find(|certification| certification.certification_id == certification_id)
                .ok_or_else(|| format!("certification {certification_id} not found"))?;
        let revision = latest_existing.revision.saturating_add(1);
        let receipt_id = format!(
            "receipt.economy.certification.revoke:{}:{}",
            normalize_key(certification_id),
            normalize_key(idempotency_key),
        );
        let mut linked_receipt_ids = latest_existing.linked_receipt_ids.clone();
        linked_receipt_ids.push(receipt_id.clone());
        linked_receipt_ids = canonical_receipt_ids(linked_receipt_ids.as_slice());

        let policy = current_policy_context();
        let mut certification = SafetyCertification {
            certification_id: certification_id.to_string(),
            certification_digest: String::new(),
            revision,
            state: CertificationState::Revoked,
            certification_level: latest_existing.certification_level.clone(),
            scope: latest_existing.scope.clone(),
            valid_from_ms: latest_existing.valid_from_ms,
            valid_until_ms: latest_existing.valid_until_ms,
            issuer_credential_kind: latest_existing.issuer_credential_kind.clone(),
            issuer_credential_digest: latest_existing.issuer_credential_digest.clone(),
            required_evidence_digests: latest_existing.required_evidence_digests.clone(),
            linked_receipt_ids,
            issued_at_ms: latest_existing.issued_at_ms,
            updated_at_ms: as_of_ms,
            revoked_reason_code: Some(revoked_reason_code.clone()),
            policy_bundle_id: policy.policy_bundle_id.clone(),
            policy_version: policy.policy_version.clone(),
            supersedes_digest: Some(latest_existing.certification_digest.clone()),
        };
        certification.certification_digest = certification_digest_for(
            certification.certification_id.as_str(),
            certification.revision,
            certification.state,
            certification.certification_level.as_str(),
            certification.scope.as_slice(),
            certification.valid_from_ms,
            certification.valid_until_ms,
            certification.issuer_credential_kind.as_str(),
            certification.issuer_credential_digest.as_str(),
            certification.required_evidence_digests.as_slice(),
            certification.linked_receipt_ids.as_slice(),
            certification.issued_at_ms,
            certification.updated_at_ms,
            certification.revoked_reason_code.as_deref(),
            certification.policy_bundle_id.as_str(),
            certification.policy_version.as_str(),
            certification.supersedes_digest.as_deref(),
        );
        let certification_evidence = certification_object_evidence(&certification);
        let (hint_category, hint_tfb_class, hint_severity) =
            certification_hint_fields(certification.scope.as_slice());

        let mut evidence = Vec::<EvidenceRef>::new();
        evidence.push(certification_evidence);
        self.append_receipt_reference_links(
            &mut evidence,
            latest_existing.linked_receipt_ids.as_slice(),
        );
        for evidence_ref in draft.evidence {
            evidence.push(evidence_ref);
        }
        evidence.push(EvidenceRef::new(
            "certification_revocation_reason",
            format!("oa://economy/certifications/{certification_id}/revocation"),
            digest_for_text(revoked_reason_code.as_str()),
        ));

        let receipt = ReceiptBuilder::new(
            receipt_id.clone(),
            "economy.certification.revoked.v1",
            as_of_ms,
            idempotency_key.to_string(),
            TraceContext {
                session_id: None,
                trajectory_hash: None,
                job_hash: None,
                run_id: Some(format!(
                    "economy_certification_revoke:{}",
                    normalize_key(certification_id)
                )),
                work_unit_id: None,
                contract_id: None,
                claim_id: None,
            },
            policy,
        )
        .with_inputs_payload(json!({
            "certification_id": certification_id,
            "reason_code": revoked_reason_code,
        }))
        .with_outputs_payload(json!({
            "status": "revoked",
            "state": certification.state.label(),
            "revision": certification.revision,
            "certification_digest": certification.certification_digest,
            "reason_code": certification.revoked_reason_code,
            "source_tag": source_tag,
        }))
        .with_evidence(evidence)
        .with_hints(ReceiptHints {
            category: hint_category,
            tfb_class: hint_tfb_class,
            severity: hint_severity,
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: None,
            auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
            personhood_proved: Some(false),
            reason_code: Some(revoked_reason_code),
            notional: None,
            liability_premium: None,
        })
        .build();
        self.append_receipt(receipt, source_tag);
        if self.load_state == PaneLoadState::Error {
            return Err(self
                .last_error
                .clone()
                .unwrap_or_else(|| "failed to emit certification revocation receipt".to_string()));
        }
        Ok(receipt_id)
    }

    pub fn record_rollback_action(
        &mut self,
        draft: RollbackActionDraft,
        occurred_at_ms: i64,
        source_tag: &str,
    ) -> Result<String, String> {
        let work_unit_id = draft.work_unit_id.trim();
        if work_unit_id.is_empty() {
            return Err("rollback action work_unit_id cannot be empty".to_string());
        }
        let idempotency_key = draft.idempotency_key.trim();
        if idempotency_key.is_empty() {
            return Err("rollback action idempotency_key cannot be empty".to_string());
        }

        let linked_receipt_ids = canonical_receipt_ids(draft.linked_receipt_ids.as_slice());
        let incident_id = draft
            .incident_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        if incident_id.is_none() && linked_receipt_ids.is_empty() {
            return Err(
                "rollback action requires incident_id or at least one linked_receipt_id"
                    .to_string(),
            );
        }

        for receipt_id in &linked_receipt_ids {
            if self.get_receipt(receipt_id.as_str()).is_none() {
                return Err(format!("linked receipt {receipt_id} not found"));
            }
        }
        let incident = if let Some(incident_id) = incident_id.as_deref() {
            Some(
                self.latest_incident_by_id(incident_id)
                    .cloned()
                    .ok_or_else(|| format!("incident {incident_id} not found"))?,
            )
        } else {
            None
        };
        let metadata = self
            .work_units
            .get(work_unit_id)
            .cloned()
            .ok_or_else(|| format!("work_unit metadata not found for {work_unit_id}"))?;
        let resolved_metadata = ResolvedWorkMetadata {
            category: metadata.category,
            tfb_class: metadata.tfb_class,
            severity: metadata.severity,
            verification_budget_hint_sats: metadata.verification_budget_hint_sats,
            template_kind: metadata.template_kind,
            acceptance_criteria_ref: metadata.acceptance_criteria_ref,
            coordinated_disclosure_ref: metadata.coordinated_disclosure_ref,
            mandatory_provenance: metadata.mandatory_provenance,
            rollback_plan_ref: metadata.rollback_plan_ref,
            compensating_action_plan_ref: metadata.compensating_action_plan_ref,
        };

        let expected_reason_code = draft.rollback_receipt_type.reason_code();
        if draft
            .reason_code
            .as_deref()
            .map(str::trim)
            .is_some_and(|provided| provided != expected_reason_code)
        {
            return Err(format!(
                "rollback action reason_code must be {expected_reason_code}"
            ));
        }
        let canonical_summary = draft
            .summary
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| draft.rollback_receipt_type.default_summary());
        let linkage_material = format!(
            "{}:{}:{}:{}",
            work_unit_id,
            draft.rollback_receipt_type.label(),
            incident_id.clone().unwrap_or_else(|| "none".to_string()),
            linked_receipt_ids.join("|")
        );
        let receipt_key = normalize_key(digest_for_text(linkage_material.as_str()).as_str());
        let receipt_id = format!(
            "receipt.economy.rollback:{}:{}:{}",
            draft.rollback_receipt_type.label(),
            normalize_key(work_unit_id),
            receipt_key
        );
        let mut evidence = Vec::<EvidenceRef>::new();
        append_rollback_plan_evidence(&mut evidence, work_unit_id, &resolved_metadata);
        evidence.push(EvidenceRef::new(
            "rollback_summary",
            format!(
                "oa://economy/rollback/{}/summary",
                normalize_key(work_unit_id)
            ),
            digest_for_text(canonical_summary),
        ));
        if let Some(incident) = incident.as_ref() {
            evidence.push(incident_object_evidence(incident));
        }
        self.append_receipt_reference_links(&mut evidence, linked_receipt_ids.as_slice());
        for receipt_id in &linked_receipt_ids {
            let Some(receipt) = self.get_receipt(receipt_id.as_str()) else {
                continue;
            };
            if receipt.receipt_type.to_ascii_lowercase().contains("claim") {
                evidence.push(EvidenceRef::new(
                    "trigger_claim_ref",
                    format!("oa://receipts/{}", receipt.receipt_id),
                    receipt.canonical_hash.clone(),
                ));
            }
        }
        let linked_claim_id = linked_receipt_ids.iter().find_map(|receipt_id| {
            self.get_receipt(receipt_id.as_str())
                .and_then(|receipt| receipt.trace.claim_id.clone())
        });

        let receipt = ReceiptBuilder::new(
            receipt_id.clone(),
            draft.rollback_receipt_type.receipt_type(),
            occurred_at_ms.max(0),
            format!(
                "idemp.economy.rollback:{}:{}",
                draft.rollback_receipt_type.label(),
                normalize_key(idempotency_key)
            ),
            TraceContext {
                session_id: None,
                trajectory_hash: None,
                job_hash: None,
                run_id: Some(format!(
                    "rollback:{}:{}",
                    normalize_key(work_unit_id),
                    draft.rollback_receipt_type.label()
                )),
                work_unit_id: Some(work_unit_id.to_string()),
                contract_id: None,
                claim_id: linked_claim_id,
            },
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "work_unit_id": work_unit_id,
            "incident_id": incident_id,
            "linked_receipt_ids": linked_receipt_ids,
            "rollback_receipt_type": draft.rollback_receipt_type.label(),
            "summary": canonical_summary,
            "work_unit": work_unit_metadata_payload(work_unit_id, &resolved_metadata),
        }))
        .with_outputs_payload(json!({
            "work_unit_id": work_unit_id,
            "rollback_receipt_type": draft.rollback_receipt_type.label(),
            "status": draft.rollback_receipt_type.status_label(),
            "reason_code": expected_reason_code,
            "source_tag": source_tag,
        }))
        .with_evidence(evidence)
        .with_hints(ReceiptHints {
            category: Some(resolved_metadata.category.clone()),
            tfb_class: Some(resolved_metadata.tfb_class),
            severity: Some(resolved_metadata.severity),
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: None,
            auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
            personhood_proved: Some(false),
            reason_code: Some(expected_reason_code.to_string()),
            notional: None,
            liability_premium: None,
        })
        .build();
        self.append_receipt(receipt, source_tag);
        if self.load_state == PaneLoadState::Error {
            return Err(self
                .last_error
                .clone()
                .unwrap_or_else(|| "failed to emit rollback action receipt".to_string()));
        }
        Ok(receipt_id)
    }

    pub fn register_incident_taxonomy_entry(
        &mut self,
        entry: IncidentTaxonomyEntry,
    ) -> Result<(), String> {
        upsert_incident_taxonomy_entry(&mut self.incident_taxonomy_registry, entry)?;
        if let Err(error) = self.persist_current_state() {
            self.last_error = Some(error.clone());
            self.last_action = Some("Incident taxonomy registry persist failed".to_string());
            self.load_state = PaneLoadState::Error;
            return Err(error);
        }
        self.last_error = None;
        self.last_action = Some("Incident taxonomy entry registered".to_string());
        self.load_state = PaneLoadState::Ready;
        Ok(())
    }

    pub fn report_incident(
        &mut self,
        draft: IncidentReportDraft,
        reported_at_ms: i64,
        source_tag: &str,
    ) -> Result<String, String> {
        self.validate_incident_draft_linkage(
            draft.linked_receipt_ids.as_slice(),
            draft.rollback_receipt_ids.as_slice(),
            true,
        )?;
        let taxonomy_key = taxonomy_lookup_key(
            draft.taxonomy_id.as_str(),
            draft.taxonomy_version.as_str(),
            draft.taxonomy_code.as_str(),
        );
        if !self
            .incident_taxonomy_registry
            .contains_key(taxonomy_key.as_str())
        {
            return Err(format!(
                "unknown incident taxonomy {}:{}:{}",
                draft.taxonomy_id, draft.taxonomy_version, draft.taxonomy_code
            ));
        }
        if draft.idempotency_key.trim().is_empty() {
            return Err("incident report idempotency_key cannot be empty".to_string());
        }
        let incident_id = format!("incident:{}", normalize_key(draft.idempotency_key.as_str()));
        let linked_receipt_ids = canonical_receipt_ids(draft.linked_receipt_ids.as_slice());
        let rollback_receipt_ids = canonical_receipt_ids(draft.rollback_receipt_ids.as_slice());
        let evidence_digests = canonical_evidence_digests(draft.evidence_digests.as_slice())?;
        let policy = current_policy_context();
        let incident = IncidentObject {
            incident_id: incident_id.clone(),
            incident_digest: incident_digest_for(
                incident_id.as_str(),
                1,
                draft.incident_kind,
                IncidentStatus::Open,
                draft.taxonomy_id.as_str(),
                draft.taxonomy_version.as_str(),
                draft.taxonomy_code.as_str(),
                draft.severity,
                draft.summary.as_str(),
                reported_at_ms.max(0),
                reported_at_ms.max(0),
                policy.policy_bundle_id.as_str(),
                policy.policy_version.as_str(),
                linked_receipt_ids.as_slice(),
                rollback_receipt_ids.as_slice(),
                evidence_digests.as_slice(),
                None,
            ),
            revision: 1,
            incident_kind: draft.incident_kind,
            incident_status: IncidentStatus::Open,
            taxonomy_id: draft.taxonomy_id,
            taxonomy_version: draft.taxonomy_version,
            taxonomy_code: draft.taxonomy_code,
            severity: draft.severity,
            summary: draft.summary,
            reported_at_ms: reported_at_ms.max(0),
            updated_at_ms: reported_at_ms.max(0),
            policy_bundle_id: policy.policy_bundle_id.clone(),
            policy_version: policy.policy_version.clone(),
            linked_receipt_ids,
            rollback_receipt_ids,
            evidence_digests,
            supersedes_digest: None,
        };
        if let Some(existing) = self.latest_incident_by_id(incident_id.as_str()) {
            if existing.incident_digest == incident.incident_digest {
                return Ok(incident_id);
            }
            return Err(format!("incident idempotency conflict for {}", incident_id));
        }
        self.emit_incident_receipt(
            "economy.incident.reported.v1",
            REASON_CODE_INCIDENT_REPORTED,
            incident.clone(),
            draft.idempotency_key.as_str(),
            source_tag,
        )?;
        self.persist_incident_object(incident)?;
        Ok(incident_id)
    }

    pub fn update_incident(
        &mut self,
        draft: IncidentUpdateDraft,
        updated_at_ms: i64,
        source_tag: &str,
    ) -> Result<String, String> {
        let latest = self
            .latest_incident_by_id(draft.incident_id.as_str())
            .cloned()
            .ok_or_else(|| format!("unknown incident_id {}", draft.incident_id))?;
        self.validate_incident_draft_linkage(
            if draft.linked_receipt_ids.is_empty() {
                latest.linked_receipt_ids.as_slice()
            } else {
                draft.linked_receipt_ids.as_slice()
            },
            if draft.rollback_receipt_ids.is_empty() {
                latest.rollback_receipt_ids.as_slice()
            } else {
                draft.rollback_receipt_ids.as_slice()
            },
            true,
        )?;
        let linked_receipt_ids = if draft.linked_receipt_ids.is_empty() {
            latest.linked_receipt_ids.clone()
        } else {
            canonical_receipt_ids(draft.linked_receipt_ids.as_slice())
        };
        let rollback_receipt_ids = if draft.rollback_receipt_ids.is_empty() {
            latest.rollback_receipt_ids.clone()
        } else {
            canonical_receipt_ids(draft.rollback_receipt_ids.as_slice())
        };
        let evidence_digests = if draft.evidence_digests.is_empty() {
            latest.evidence_digests.clone()
        } else {
            canonical_evidence_digests(draft.evidence_digests.as_slice())?
        };
        let summary = draft.summary.unwrap_or_else(|| latest.summary.clone());
        let policy = current_policy_context();
        let incident = IncidentObject {
            incident_id: latest.incident_id.clone(),
            incident_digest: incident_digest_for(
                latest.incident_id.as_str(),
                latest.revision.saturating_add(1),
                latest.incident_kind,
                latest.incident_status,
                latest.taxonomy_id.as_str(),
                latest.taxonomy_version.as_str(),
                latest.taxonomy_code.as_str(),
                latest.severity,
                summary.as_str(),
                latest.reported_at_ms,
                updated_at_ms.max(0),
                policy.policy_bundle_id.as_str(),
                policy.policy_version.as_str(),
                linked_receipt_ids.as_slice(),
                rollback_receipt_ids.as_slice(),
                evidence_digests.as_slice(),
                Some(latest.incident_digest.as_str()),
            ),
            revision: latest.revision.saturating_add(1),
            incident_kind: latest.incident_kind,
            incident_status: latest.incident_status,
            taxonomy_id: latest.taxonomy_id,
            taxonomy_version: latest.taxonomy_version,
            taxonomy_code: latest.taxonomy_code,
            severity: latest.severity,
            summary,
            reported_at_ms: latest.reported_at_ms,
            updated_at_ms: updated_at_ms.max(0),
            policy_bundle_id: policy.policy_bundle_id.clone(),
            policy_version: policy.policy_version.clone(),
            linked_receipt_ids,
            rollback_receipt_ids,
            evidence_digests,
            supersedes_digest: Some(latest.incident_digest),
        };
        self.emit_incident_receipt(
            "economy.incident.updated.v1",
            REASON_CODE_INCIDENT_UPDATED,
            incident.clone(),
            draft.idempotency_key.as_str(),
            source_tag,
        )?;
        self.persist_incident_object(incident)?;
        Ok(draft.incident_id)
    }

    pub fn resolve_incident(
        &mut self,
        draft: IncidentResolutionDraft,
        resolved_at_ms: i64,
        source_tag: &str,
    ) -> Result<String, String> {
        let latest = self
            .latest_incident_by_id(draft.incident_id.as_str())
            .cloned()
            .ok_or_else(|| format!("unknown incident_id {}", draft.incident_id))?;
        let rollback_receipt_ids = if draft.rollback_receipt_ids.is_empty() {
            latest.rollback_receipt_ids.clone()
        } else {
            canonical_receipt_ids(draft.rollback_receipt_ids.as_slice())
        };
        self.validate_incident_draft_linkage(
            latest.linked_receipt_ids.as_slice(),
            rollback_receipt_ids.as_slice(),
            false,
        )?;
        let evidence_digests = if draft.evidence_digests.is_empty() {
            latest.evidence_digests.clone()
        } else {
            canonical_evidence_digests(draft.evidence_digests.as_slice())?
        };
        let summary = draft
            .resolution_summary
            .unwrap_or_else(|| latest.summary.clone());
        let policy = current_policy_context();
        let incident = IncidentObject {
            incident_id: latest.incident_id.clone(),
            incident_digest: incident_digest_for(
                latest.incident_id.as_str(),
                latest.revision.saturating_add(1),
                latest.incident_kind,
                IncidentStatus::Resolved,
                latest.taxonomy_id.as_str(),
                latest.taxonomy_version.as_str(),
                latest.taxonomy_code.as_str(),
                latest.severity,
                summary.as_str(),
                latest.reported_at_ms,
                resolved_at_ms.max(0),
                policy.policy_bundle_id.as_str(),
                policy.policy_version.as_str(),
                latest.linked_receipt_ids.as_slice(),
                rollback_receipt_ids.as_slice(),
                evidence_digests.as_slice(),
                Some(latest.incident_digest.as_str()),
            ),
            revision: latest.revision.saturating_add(1),
            incident_kind: latest.incident_kind,
            incident_status: IncidentStatus::Resolved,
            taxonomy_id: latest.taxonomy_id,
            taxonomy_version: latest.taxonomy_version,
            taxonomy_code: latest.taxonomy_code,
            severity: latest.severity,
            summary,
            reported_at_ms: latest.reported_at_ms,
            updated_at_ms: resolved_at_ms.max(0),
            policy_bundle_id: policy.policy_bundle_id.clone(),
            policy_version: policy.policy_version.clone(),
            linked_receipt_ids: latest.linked_receipt_ids,
            rollback_receipt_ids,
            evidence_digests,
            supersedes_digest: Some(latest.incident_digest),
        };
        self.emit_incident_receipt(
            "economy.incident.resolved.v1",
            REASON_CODE_INCIDENT_RESOLVED,
            incident.clone(),
            draft.idempotency_key.as_str(),
            source_tag,
        )?;
        self.persist_incident_object(incident)?;
        Ok(draft.incident_id)
    }

    pub fn record_outcome_registry_entry(
        &mut self,
        draft: OutcomeRegistryEntryDraft,
        recorded_at_ms: i64,
        source_tag: &str,
    ) -> Result<String, String> {
        if draft.idempotency_key.trim().is_empty() {
            return Err("outcome registry idempotency_key cannot be empty".to_string());
        }
        self.validate_incident_draft_linkage(draft.linked_receipt_ids.as_slice(), &[], true)?;
        let category = canonical_outcome_value(draft.category.as_str())
            .ok_or_else(|| "outcome registry category cannot be empty".to_string())?;
        let verdict_outcome = canonical_outcome_value(draft.verdict_outcome.as_str())
            .ok_or_else(|| "verdict_outcome cannot be empty".to_string())?;
        let settlement_outcome = canonical_outcome_value(draft.settlement_outcome.as_str())
            .ok_or_else(|| "settlement_outcome cannot be empty".to_string())?;
        let claim_outcome = canonical_optional_outcome_value(draft.claim_outcome.as_deref());
        let remedy_outcome = canonical_optional_outcome_value(draft.remedy_outcome.as_deref());
        let incident_tags = canonical_incident_tags(draft.incident_tags.as_slice());
        let linked_receipt_ids = canonical_receipt_ids(draft.linked_receipt_ids.as_slice());
        let evidence_digests = canonical_evidence_digests(draft.evidence_digests.as_slice())?;
        let policy = current_policy_context();
        let entry_id = format!("outcome:{}", normalize_key(draft.idempotency_key.as_str()));
        let entry = OutcomeRegistryEntry {
            entry_id: entry_id.clone(),
            entry_digest: outcome_registry_digest_for(
                entry_id.as_str(),
                1,
                category.as_str(),
                draft.tfb_class,
                draft.severity,
                verdict_outcome.as_str(),
                settlement_outcome.as_str(),
                claim_outcome.as_deref(),
                remedy_outcome.as_deref(),
                incident_tags.as_slice(),
                linked_receipt_ids.as_slice(),
                evidence_digests.as_slice(),
                recorded_at_ms.max(0),
                recorded_at_ms.max(0),
                policy.policy_bundle_id.as_str(),
                policy.policy_version.as_str(),
                None,
            ),
            revision: 1,
            category,
            tfb_class: draft.tfb_class,
            severity: draft.severity,
            verdict_outcome,
            settlement_outcome,
            claim_outcome,
            remedy_outcome,
            incident_tags,
            linked_receipt_ids,
            evidence_digests,
            reported_at_ms: recorded_at_ms.max(0),
            updated_at_ms: recorded_at_ms.max(0),
            policy_bundle_id: policy.policy_bundle_id.clone(),
            policy_version: policy.policy_version.clone(),
            supersedes_digest: None,
        };
        if let Some(existing) = self.latest_outcome_registry_entry_by_id(entry_id.as_str()) {
            if existing.entry_digest == entry.entry_digest {
                return Ok(entry_id);
            }
            return Err(format!(
                "outcome registry idempotency conflict for {}",
                entry_id
            ));
        }
        self.emit_outcome_registry_receipt(
            "economy.outcome_registry.created.v1",
            REASON_CODE_OUTCOME_REGISTRY_CREATED,
            entry.clone(),
            draft.idempotency_key.as_str(),
            source_tag,
        )?;
        self.persist_outcome_registry_entry(entry)?;
        Ok(entry_id)
    }

    pub fn update_outcome_registry_entry(
        &mut self,
        draft: OutcomeRegistryUpdateDraft,
        updated_at_ms: i64,
        source_tag: &str,
    ) -> Result<String, String> {
        if draft.idempotency_key.trim().is_empty() {
            return Err("outcome registry update idempotency_key cannot be empty".to_string());
        }
        let latest = self
            .latest_outcome_registry_entry_by_id(draft.entry_id.as_str())
            .cloned()
            .ok_or_else(|| format!("unknown outcome registry entry_id {}", draft.entry_id))?;
        let linked_receipt_ids = if draft.linked_receipt_ids.is_empty() {
            latest.linked_receipt_ids.clone()
        } else {
            canonical_receipt_ids(draft.linked_receipt_ids.as_slice())
        };
        self.validate_incident_draft_linkage(linked_receipt_ids.as_slice(), &[], true)?;
        let verdict_outcome = draft
            .verdict_outcome
            .as_deref()
            .and_then(canonical_outcome_value)
            .unwrap_or_else(|| latest.verdict_outcome.clone());
        let settlement_outcome = draft
            .settlement_outcome
            .as_deref()
            .and_then(canonical_outcome_value)
            .unwrap_or_else(|| latest.settlement_outcome.clone());
        let claim_outcome = if draft.claim_outcome.is_some() {
            canonical_optional_outcome_value(draft.claim_outcome.as_deref())
        } else {
            latest.claim_outcome.clone()
        };
        let remedy_outcome = if draft.remedy_outcome.is_some() {
            canonical_optional_outcome_value(draft.remedy_outcome.as_deref())
        } else {
            latest.remedy_outcome.clone()
        };
        let incident_tags = if draft.incident_tags.is_empty() {
            latest.incident_tags.clone()
        } else {
            canonical_incident_tags(draft.incident_tags.as_slice())
        };
        let evidence_digests = if draft.evidence_digests.is_empty() {
            latest.evidence_digests.clone()
        } else {
            canonical_evidence_digests(draft.evidence_digests.as_slice())?
        };
        let policy = current_policy_context();
        let entry = OutcomeRegistryEntry {
            entry_id: latest.entry_id.clone(),
            entry_digest: outcome_registry_digest_for(
                latest.entry_id.as_str(),
                latest.revision.saturating_add(1),
                latest.category.as_str(),
                latest.tfb_class,
                latest.severity,
                verdict_outcome.as_str(),
                settlement_outcome.as_str(),
                claim_outcome.as_deref(),
                remedy_outcome.as_deref(),
                incident_tags.as_slice(),
                linked_receipt_ids.as_slice(),
                evidence_digests.as_slice(),
                latest.reported_at_ms,
                updated_at_ms.max(0),
                policy.policy_bundle_id.as_str(),
                policy.policy_version.as_str(),
                Some(latest.entry_digest.as_str()),
            ),
            revision: latest.revision.saturating_add(1),
            category: latest.category.clone(),
            tfb_class: latest.tfb_class,
            severity: latest.severity,
            verdict_outcome,
            settlement_outcome,
            claim_outcome,
            remedy_outcome,
            incident_tags,
            linked_receipt_ids,
            evidence_digests,
            reported_at_ms: latest.reported_at_ms,
            updated_at_ms: updated_at_ms.max(0),
            policy_bundle_id: policy.policy_bundle_id.clone(),
            policy_version: policy.policy_version.clone(),
            supersedes_digest: Some(latest.entry_digest),
        };
        self.emit_outcome_registry_receipt(
            "economy.outcome_registry.updated.v1",
            REASON_CODE_OUTCOME_REGISTRY_UPDATED,
            entry.clone(),
            draft.idempotency_key.as_str(),
            source_tag,
        )?;
        self.persist_outcome_registry_entry(entry)?;
        Ok(draft.entry_id)
    }

    pub fn export_incident_audit_package(
        &self,
        tier: IncidentExportRedactionTier,
        generated_at_ms: i64,
    ) -> Result<IncidentAuditPackage, String> {
        let mut incidents = self.incident_objects.clone();
        incidents = normalize_incident_objects(incidents);
        if tier == IncidentExportRedactionTier::Public {
            for incident in &mut incidents {
                incident.summary = "[redacted]".to_string();
                incident.linked_receipt_ids.clear();
                incident.rollback_receipt_ids.clear();
            }
        }
        let mut taxonomy_registry = self.normalized_incident_taxonomy_entries();
        let mut incident_receipt_map = self
            .receipts
            .iter()
            .filter(|receipt| receipt.receipt_type.starts_with("economy.incident."))
            .map(|receipt| (receipt.receipt_id.clone(), receipt.clone()))
            .collect::<BTreeMap<_, _>>();
        for incident in &incidents {
            for rollback_receipt_id in &incident.rollback_receipt_ids {
                if let Some(receipt) = self.get_receipt(rollback_receipt_id.as_str()) {
                    incident_receipt_map.insert(receipt.receipt_id.clone(), receipt.clone());
                }
            }
            for linked_receipt_id in &incident.linked_receipt_ids {
                if let Some(receipt) = self.get_receipt(linked_receipt_id.as_str()) {
                    if receipt.receipt_type.to_ascii_lowercase().contains("claim") {
                        incident_receipt_map.insert(receipt.receipt_id.clone(), receipt.clone());
                    }
                }
            }
        }
        let mut incident_receipts = incident_receipt_map.into_values().collect::<Vec<_>>();
        incident_receipts.sort_by(|lhs, rhs| {
            lhs.created_at_ms
                .cmp(&rhs.created_at_ms)
                .then_with(|| lhs.receipt_id.cmp(&rhs.receipt_id))
        });
        if tier == IncidentExportRedactionTier::Public {
            for receipt in &mut incident_receipts {
                receipt.evidence.iter_mut().for_each(|evidence| {
                    evidence.uri = "oa://redacted".to_string();
                });
            }
        }
        taxonomy_registry
            .sort_by(|lhs, rhs| incident_taxonomy_key(lhs).cmp(&incident_taxonomy_key(rhs)));
        let package_hash = digest_for_text(
            format!(
                "{}:{}:{}:{}",
                incidents
                    .iter()
                    .map(|incident| incident.incident_digest.clone())
                    .collect::<Vec<_>>()
                    .join("|"),
                taxonomy_registry
                    .iter()
                    .map(incident_taxonomy_key)
                    .collect::<Vec<_>>()
                    .join("|"),
                incident_receipts
                    .iter()
                    .map(|receipt| receipt.canonical_hash.clone())
                    .collect::<Vec<_>>()
                    .join("|"),
                match tier {
                    IncidentExportRedactionTier::Public => "public",
                    IncidentExportRedactionTier::Restricted => "restricted",
                }
            )
            .as_str(),
        );
        Ok(IncidentAuditPackage {
            schema_version: EARN_KERNEL_RECEIPT_SCHEMA_VERSION,
            generated_at_ms: generated_at_ms.max(0),
            stream_id: self.stream_id.clone(),
            authority: self.authority.clone(),
            redaction_tier: tier,
            incident_count: incidents.len(),
            taxonomy_count: taxonomy_registry.len(),
            incidents,
            taxonomy_registry,
            incident_receipts,
            package_hash,
        })
    }

    fn latest_incident_by_id(&self, incident_id: &str) -> Option<&IncidentObject> {
        self.incident_objects
            .iter()
            .filter(|incident| incident.incident_id == incident_id)
            .max_by(|lhs, rhs| {
                lhs.revision
                    .cmp(&rhs.revision)
                    .then_with(|| lhs.updated_at_ms.cmp(&rhs.updated_at_ms))
            })
    }

    fn latest_outcome_registry_entry_by_id(&self, entry_id: &str) -> Option<&OutcomeRegistryEntry> {
        self.outcome_registry_entries
            .iter()
            .filter(|entry| entry.entry_id == entry_id)
            .max_by(|lhs, rhs| {
                lhs.revision
                    .cmp(&rhs.revision)
                    .then_with(|| lhs.updated_at_ms.cmp(&rhs.updated_at_ms))
            })
    }

    fn validate_incident_draft_linkage(
        &self,
        linked_receipt_ids: &[String],
        rollback_receipt_ids: &[String],
        require_linked_receipts: bool,
    ) -> Result<(), String> {
        let linked_ids = canonical_receipt_ids(linked_receipt_ids);
        if require_linked_receipts && linked_ids.is_empty() {
            return Err("incident linkage requires at least one linked receipt".to_string());
        }
        for receipt_id in linked_ids {
            if self.get_receipt(receipt_id.as_str()).is_none() {
                return Err(format!("linked receipt {receipt_id} not found"));
            }
        }
        for receipt_id in canonical_receipt_ids(rollback_receipt_ids) {
            let Some(receipt) = self.get_receipt(receipt_id.as_str()) else {
                return Err(format!("rollback receipt {receipt_id} not found"));
            };
            let normalized_type = receipt.receipt_type.to_ascii_lowercase();
            let rollback_like = normalized_type.contains("rollback")
                || normalized_type.contains("compensating_action")
                || receipt
                    .hints
                    .reason_code
                    .as_deref()
                    .is_some_and(|reason| reason.to_ascii_lowercase().contains("rollback"));
            if !rollback_like {
                return Err(format!(
                    "receipt {receipt_id} is not rollback/compensating-action linked"
                ));
            }
        }
        Ok(())
    }

    fn persist_incident_object(&mut self, incident: IncidentObject) -> Result<(), String> {
        if self
            .incident_objects
            .iter()
            .any(|existing| existing.incident_digest == incident.incident_digest)
        {
            return Ok(());
        }
        self.incident_objects.push(incident);
        self.incident_objects =
            normalize_incident_objects(std::mem::take(&mut self.incident_objects));
        if let Err(error) = self.persist_current_state() {
            self.last_error = Some(error.clone());
            self.last_action = Some("Incident object persist failed".to_string());
            self.load_state = PaneLoadState::Error;
            return Err(error);
        }
        self.last_error = None;
        self.last_action = Some("Incident object persisted".to_string());
        self.load_state = PaneLoadState::Ready;
        Ok(())
    }

    fn persist_outcome_registry_entry(
        &mut self,
        entry: OutcomeRegistryEntry,
    ) -> Result<(), String> {
        if self
            .outcome_registry_entries
            .iter()
            .any(|existing| existing.entry_digest == entry.entry_digest)
        {
            return Ok(());
        }
        self.outcome_registry_entries.push(entry);
        self.outcome_registry_entries =
            normalize_outcome_registry_entries(std::mem::take(&mut self.outcome_registry_entries));
        if let Err(error) = self.persist_current_state() {
            self.last_error = Some(error.clone());
            self.last_action = Some("Outcome registry entry persist failed".to_string());
            self.load_state = PaneLoadState::Error;
            return Err(error);
        }
        self.last_error = None;
        self.last_action = Some("Outcome registry entry persisted".to_string());
        self.load_state = PaneLoadState::Ready;
        Ok(())
    }

    fn emit_incident_receipt(
        &mut self,
        receipt_type: &str,
        reason_code: &str,
        incident: IncidentObject,
        idempotency_key: &str,
        source_tag: &str,
    ) -> Result<(), String> {
        if idempotency_key.trim().is_empty() {
            return Err("incident receipt idempotency_key cannot be empty".to_string());
        }
        let mut evidence = vec![incident_object_evidence(&incident)];
        self.append_receipt_reference_links(&mut evidence, incident.linked_receipt_ids.as_slice());
        self.append_receipt_reference_links(
            &mut evidence,
            incident.rollback_receipt_ids.as_slice(),
        );
        let receipt = ReceiptBuilder::new(
            format!(
                "receipt.economy.incident:{}:{}:{}",
                normalize_key(receipt_type),
                normalize_key(incident.incident_id.as_str()),
                incident.revision
            ),
            receipt_type,
            incident.updated_at_ms.max(0),
            format!(
                "idemp.economy.incident:{}:{}",
                normalize_key(receipt_type),
                normalize_key(idempotency_key)
            ),
            TraceContext {
                session_id: None,
                trajectory_hash: None,
                job_hash: None,
                run_id: Some(format!(
                    "economy_incident:{}:{}",
                    incident.incident_id, incident.revision
                )),
                work_unit_id: incident.linked_receipt_ids.first().cloned(),
                contract_id: None,
                claim_id: None,
            },
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "incident_id": incident.incident_id,
            "incident_digest": incident.incident_digest,
            "revision": incident.revision,
            "incident_kind": incident.incident_kind.label(),
            "incident_status": incident.incident_status.label(),
            "taxonomy_id": incident.taxonomy_id,
            "taxonomy_version": incident.taxonomy_version,
            "taxonomy_code": incident.taxonomy_code,
            "linked_receipt_ids": incident.linked_receipt_ids,
            "rollback_receipt_ids": incident.rollback_receipt_ids,
            "evidence_digests": incident.evidence_digests,
        }))
        .with_outputs_payload(json!({
            "status": "recorded",
            "incident_id": incident.incident_id,
            "incident_digest": incident.incident_digest,
            "revision": incident.revision,
            "incident_status": incident.incident_status.label(),
            "taxonomy_id": incident.taxonomy_id,
            "taxonomy_version": incident.taxonomy_version,
            "taxonomy_code": incident.taxonomy_code,
            "source_tag": source_tag,
        }))
        .with_evidence(evidence)
        .with_hints(ReceiptHints {
            category: Some("compute".to_string()),
            tfb_class: None,
            severity: Some(incident.severity),
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: None,
            auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
            personhood_proved: Some(false),
            reason_code: Some(reason_code.to_string()),
            notional: None,
            liability_premium: None,
        })
        .build();
        self.append_receipt(receipt, source_tag);
        if self.load_state == PaneLoadState::Error {
            return Err(self
                .last_error
                .clone()
                .unwrap_or_else(|| "incident receipt append failed".to_string()));
        }
        Ok(())
    }

    fn emit_outcome_registry_receipt(
        &mut self,
        receipt_type: &str,
        reason_code: &str,
        entry: OutcomeRegistryEntry,
        idempotency_key: &str,
        source_tag: &str,
    ) -> Result<(), String> {
        if idempotency_key.trim().is_empty() {
            return Err("outcome registry receipt idempotency_key cannot be empty".to_string());
        }
        let entry_id = entry.entry_id.clone();
        let entry_digest = entry.entry_digest.clone();
        let revision = entry.revision;
        let category = entry.category.clone();
        let tfb_class = entry.tfb_class;
        let severity = entry.severity;
        let verdict_outcome = entry.verdict_outcome.clone();
        let settlement_outcome = entry.settlement_outcome.clone();
        let claim_outcome = entry.claim_outcome.clone();
        let remedy_outcome = entry.remedy_outcome.clone();
        let incident_tags = entry.incident_tags.clone();
        let linked_receipt_ids = entry.linked_receipt_ids.clone();
        let evidence_digests = entry.evidence_digests.clone();
        let mut evidence = vec![outcome_registry_entry_evidence(&entry)];
        self.append_receipt_reference_links(&mut evidence, linked_receipt_ids.as_slice());
        let receipt = ReceiptBuilder::new(
            format!(
                "receipt.economy.outcome_registry:{}:{}:{}",
                normalize_key(receipt_type),
                normalize_key(entry_id.as_str()),
                revision
            ),
            receipt_type,
            entry.updated_at_ms.max(0),
            format!(
                "idemp.economy.outcome_registry:{}:{}",
                normalize_key(receipt_type),
                normalize_key(idempotency_key)
            ),
            TraceContext {
                session_id: None,
                trajectory_hash: None,
                job_hash: None,
                run_id: Some(format!(
                    "economy_outcome_registry:{}:{}",
                    entry_id, revision
                )),
                work_unit_id: linked_receipt_ids.first().cloned(),
                contract_id: None,
                claim_id: None,
            },
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "entry_id": entry_id,
            "entry_digest": entry_digest,
            "revision": revision,
            "category": category,
            "tfb_class": tfb_class.label(),
            "severity": severity.label(),
            "verdict_outcome": verdict_outcome,
            "settlement_outcome": settlement_outcome,
            "claim_outcome": claim_outcome,
            "remedy_outcome": remedy_outcome,
            "incident_tags": incident_tags,
            "linked_receipt_ids": linked_receipt_ids,
            "evidence_digests": evidence_digests,
        }))
        .with_outputs_payload(json!({
            "status": "recorded",
            "entry_id": entry.entry_id,
            "entry_digest": entry.entry_digest,
            "revision": entry.revision,
            "source_tag": source_tag,
        }))
        .with_evidence(evidence)
        .with_hints(ReceiptHints {
            category: Some(entry.category),
            tfb_class: Some(entry.tfb_class),
            severity: Some(entry.severity),
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: None,
            auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
            personhood_proved: Some(false),
            reason_code: Some(reason_code.to_string()),
            notional: None,
            liability_premium: None,
        })
        .build();
        self.append_receipt(receipt, source_tag);
        if self.load_state == PaneLoadState::Error {
            return Err(self
                .last_error
                .clone()
                .unwrap_or_else(|| "outcome registry receipt append failed".to_string()));
        }
        Ok(())
    }

    pub fn receipts_for_job(&self, job_id: &str) -> Vec<&Receipt> {
        self.query_receipts(&ReceiptQuery {
            start_inclusive_ms: None,
            end_inclusive_ms: None,
            work_unit_id: Some(job_id.to_string()),
            receipt_type: None,
        })
    }

    pub fn record_correction_receipt(
        &mut self,
        superseded_receipt_ids: &[String],
        correction_note: &str,
        occurred_at_epoch_ms: i64,
        source_tag: &str,
    ) -> Result<String, String> {
        if superseded_receipt_ids.is_empty() {
            return Err("superseded_receipt_ids cannot be empty".to_string());
        }
        let correction_note = correction_note.trim();
        if correction_note.is_empty() {
            return Err("correction_note cannot be empty".to_string());
        }

        let mut canonical_superseded = superseded_receipt_ids
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        canonical_superseded.sort();
        canonical_superseded.dedup();
        if canonical_superseded.is_empty() {
            return Err("superseded_receipt_ids cannot be empty".to_string());
        }

        let linked_receipts = canonical_superseded
            .iter()
            .map(|receipt_id| {
                self.get_receipt(receipt_id.as_str())
                    .cloned()
                    .ok_or_else(|| format!("missing superseded receipt {receipt_id}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let exemplar = linked_receipts
            .first()
            .ok_or_else(|| "missing superseded receipt exemplar".to_string())?;

        let correction_token = digest_for_text(
            format!(
                "{}|{}",
                canonical_superseded.join("|"),
                correction_note.to_ascii_lowercase()
            )
            .as_str(),
        );
        let correction_key = normalize_key(correction_token.as_str());
        let occurred_at_epoch_ms = occurred_at_epoch_ms.max(0);
        let receipt_id = format!("receipt.earn:correction:{correction_key}");
        let mut evidence = vec![EvidenceRef::new(
            "correction_note",
            format!("oa://receipts/{receipt_id}/correction"),
            digest_for_text(correction_note),
        )];
        for prior in &linked_receipts {
            let mut meta = std::collections::BTreeMap::new();
            meta.insert(
                "receipt_type".to_string(),
                serde_json::Value::String(prior.receipt_type.clone()),
            );
            evidence.push(EvidenceRef {
                kind: "receipt_ref".to_string(),
                uri: format!("oa://receipts/{}", prior.receipt_id),
                digest: prior.canonical_hash.clone(),
                meta,
            });
        }

        let metadata = self.resolve_or_create_work_unit_metadata(
            exemplar
                .trace
                .work_unit_id
                .as_deref()
                .unwrap_or("work-unit:correction"),
            JobDemandSource::OpenNetwork,
            exemplar
                .hints
                .notional
                .as_ref()
                .and_then(|value| match value.amount {
                    MoneyAmount::AmountSats(sats) => Some(sats),
                    MoneyAmount::AmountMsats(msats) => Some(msats / 1_000),
                })
                .unwrap_or(0),
            None,
        );

        let receipt = ReceiptBuilder::new(
            receipt_id.clone(),
            "earn.receipt.correction.v1",
            occurred_at_epoch_ms,
            format!("idemp.earn:receipt_correction:{correction_key}"),
            TraceContext {
                session_id: exemplar.trace.session_id.clone(),
                trajectory_hash: exemplar.trace.trajectory_hash.clone(),
                job_hash: exemplar.trace.job_hash.clone(),
                run_id: Some(format!("correction:{correction_key}")),
                work_unit_id: exemplar.trace.work_unit_id.clone(),
                contract_id: exemplar.trace.contract_id.clone(),
                claim_id: exemplar.trace.claim_id.clone(),
            },
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "superseded_receipt_ids": canonical_superseded,
            "correction_note": correction_note,
            "work_unit": work_unit_metadata_payload(
                exemplar
                    .trace
                    .work_unit_id
                    .as_deref()
                    .unwrap_or("work-unit:correction"),
                &metadata,
            ),
        }))
        .with_outputs_payload(json!({
            "status": "superseded",
            "reason_code": "RECEIPT_SUPERSEDED",
            "source_tag": source_tag,
            "work_unit": work_unit_metadata_payload(
                exemplar
                    .trace
                    .work_unit_id
                    .as_deref()
                    .unwrap_or("work-unit:correction"),
                &metadata,
            ),
        }))
        .with_evidence(evidence)
        .with_hints(ReceiptHints {
            category: Some(metadata.category.clone()),
            tfb_class: Some(metadata.tfb_class),
            severity: Some(metadata.severity),
            achieved_verification_tier: exemplar.hints.achieved_verification_tier,
            verification_correlated: exemplar.hints.verification_correlated,
            provenance_grade: exemplar.hints.provenance_grade,
            auth_assurance_level: exemplar.hints.auth_assurance_level,
            personhood_proved: exemplar.hints.personhood_proved,
            reason_code: Some("RECEIPT_SUPERSEDED".to_string()),
            notional: exemplar.hints.notional.clone(),
            liability_premium: exemplar.hints.liability_premium.clone(),
        })
        .build();

        self.append_receipt(receipt, source_tag);
        if self.load_state == PaneLoadState::Error {
            return Err(self
                .last_error
                .clone()
                .unwrap_or_else(|| "failed to emit correction receipt".to_string()));
        }
        Ok(receipt_id)
    }

    pub fn settlement_lineage_receipt_ids(
        &self,
        settlement_receipt_id: &str,
    ) -> Result<Vec<String>, String> {
        if settlement_receipt_id.trim().is_empty() {
            return Err("settlement_receipt_id cannot be empty".to_string());
        }
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        queue.push_back(settlement_receipt_id.to_string());

        while let Some(receipt_id) = queue.pop_front() {
            if !visited.insert(receipt_id.clone()) {
                continue;
            }
            let Some(receipt) = self.get_receipt(receipt_id.as_str()) else {
                return Err(format!("missing linked receipt {receipt_id}"));
            };
            for evidence in &receipt.evidence {
                if evidence.kind != "receipt_ref" {
                    continue;
                }
                let Some(linked_receipt_id) = parse_receipt_ref_uri(evidence.uri.as_str()) else {
                    continue;
                };
                if !visited.contains(linked_receipt_id) {
                    queue.push_back(linked_receipt_id.to_string());
                }
            }
        }

        let mut ids: Vec<String> = visited.into_iter().collect();
        ids.sort();
        Ok(ids)
    }

    fn append_receipt(&mut self, receipt: Result<Receipt, String>, source_tag: &str) {
        let receipt = match receipt {
            Ok(value) => value,
            Err(error) => {
                self.last_error = Some(error);
                self.load_state = PaneLoadState::Error;
                return;
            }
        };
        let scope = idempotency_scope_for_receipt(&receipt);
        let idempotency_lookup_key =
            idempotency_lookup_key(scope.as_str(), receipt.idempotency_key.as_str());
        if let Some(existing) = self.idempotency_index.get(idempotency_lookup_key.as_str()) {
            if existing.inputs_hash == receipt.inputs_hash {
                self.last_error = None;
                self.load_state = PaneLoadState::Ready;
                self.last_action = Some(format!(
                    "Idempotent replay for {} -> {} ({})",
                    receipt.idempotency_key, existing.receipt_id, existing.receipt_type
                ));
                return;
            }
            self.last_error = Some(format!(
                "{}: scope={} idempotency_key={} original_receipt_id={} original_receipt_type={}",
                REASON_CODE_IDEMPOTENCY_CONFLICT,
                scope,
                receipt.idempotency_key,
                existing.receipt_id,
                existing.receipt_type
            ));
            self.last_action = Some(format!(
                "Rejected {} via {} due to idempotency conflict with {}",
                receipt.receipt_type, source_tag, existing.receipt_id
            ));
            self.load_state = PaneLoadState::Error;
            return;
        }

        if self
            .receipts
            .iter()
            .any(|existing| existing.receipt_id == receipt.receipt_id)
        {
            self.idempotency_index.insert(
                idempotency_lookup_key.clone(),
                IdempotencyRecord {
                    scope,
                    idempotency_key: receipt.idempotency_key.clone(),
                    inputs_hash: receipt.inputs_hash.clone(),
                    receipt_id: receipt.receipt_id.clone(),
                    receipt_type: receipt.receipt_type.clone(),
                    canonical_hash: receipt.canonical_hash.clone(),
                    created_at_ms: receipt.created_at_ms,
                },
            );
            normalize_idempotency_records(&mut self.idempotency_index);
            if let Err(error) = persist_earn_kernel_receipts(
                self.receipt_file_path.as_path(),
                self.receipts.as_slice(),
                self.normalized_work_units().as_slice(),
                self.normalized_idempotency_records().as_slice(),
                self.incident_objects.as_slice(),
                self.normalized_outcome_registry_entries().as_slice(),
                self.normalized_incident_taxonomy_entries().as_slice(),
            ) {
                self.last_error = Some(error);
                self.load_state = PaneLoadState::Error;
                return;
            }
            self.last_error = None;
            self.load_state = PaneLoadState::Ready;
            self.last_action = Some(format!(
                "Receipt {} already recorded (idempotent replay)",
                receipt.receipt_id
            ));
            return;
        }

        self.receipts.push(receipt.clone());
        self.receipts = normalize_receipts(std::mem::take(&mut self.receipts));
        self.idempotency_index.insert(
            idempotency_lookup_key,
            IdempotencyRecord {
                scope,
                idempotency_key: receipt.idempotency_key.clone(),
                inputs_hash: receipt.inputs_hash.clone(),
                receipt_id: receipt.receipt_id.clone(),
                receipt_type: receipt.receipt_type.clone(),
                canonical_hash: receipt.canonical_hash.clone(),
                created_at_ms: receipt.created_at_ms,
            },
        );
        normalize_idempotency_records(&mut self.idempotency_index);
        if let Err(error) = persist_earn_kernel_receipts(
            self.receipt_file_path.as_path(),
            self.receipts.as_slice(),
            self.normalized_work_units().as_slice(),
            self.normalized_idempotency_records().as_slice(),
            self.incident_objects.as_slice(),
            self.normalized_outcome_registry_entries().as_slice(),
            self.normalized_incident_taxonomy_entries().as_slice(),
        ) {
            self.last_error = Some(error);
            self.load_state = PaneLoadState::Error;
            return;
        }
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(format!(
            "Emitted {} via {} ({})",
            receipt.receipt_type, source_tag, receipt.receipt_id
        ));
    }

    fn append_receipt_reference_links(
        &self,
        evidence: &mut Vec<EvidenceRef>,
        candidate_receipt_ids: &[String],
    ) {
        let mut seen = HashSet::<String>::new();
        for candidate_id in candidate_receipt_ids {
            if !seen.insert(candidate_id.clone()) {
                continue;
            }
            let Some(linked_receipt) = self.get_receipt(candidate_id.as_str()) else {
                continue;
            };
            let mut meta = std::collections::BTreeMap::new();
            meta.insert(
                "receipt_type".to_string(),
                serde_json::Value::String(linked_receipt.receipt_type.clone()),
            );
            evidence.push(EvidenceRef {
                kind: "receipt_ref".to_string(),
                uri: format!("oa://receipts/{}", linked_receipt.receipt_id),
                digest: linked_receipt.canonical_hash.clone(),
                meta,
            });
        }
    }
}

fn work_category_for_demand_source(source: JobDemandSource) -> &'static str {
    match source {
        JobDemandSource::OpenNetwork | JobDemandSource::StarterDemand => "compute",
    }
}

fn is_white_hat_category(category: &str) -> bool {
    matches!(
        normalize_key(category).as_str(),
        "audit" | "redteam" | "incident_repro"
    )
}

fn is_verdict_receipt_type(receipt_type: &str) -> bool {
    normalize_key(receipt_type).contains("verdict")
}

fn idempotency_scope_for_receipt(receipt: &Receipt) -> String {
    format!(
        "{}:{}",
        normalize_key(receipt.receipt_type.as_str()),
        normalize_key(receipt.policy.approved_by.as_str())
    )
}

fn idempotency_lookup_key(scope: &str, idempotency_key: &str) -> String {
    format!(
        "{}:{}",
        normalize_key(scope),
        normalize_key(idempotency_key)
    )
}

fn work_unit_metadata_payload(
    work_unit_id: &str,
    metadata: &ResolvedWorkMetadata,
) -> serde_json::Value {
    json!({
        "work_unit_id": work_unit_id,
        "category": metadata.category.as_str(),
        "tfb_class": metadata.tfb_class.label(),
        "severity": metadata.severity.label(),
        "verification_budget_hint": {
            "asset": "btc",
            "amount_sats": metadata.verification_budget_hint_sats,
        },
        "template_kind": metadata.template_kind.map(WhiteHatWorkUnitKind::label),
        "acceptance_criteria_ref": metadata.acceptance_criteria_ref.clone(),
        "coordinated_disclosure_ref": metadata.coordinated_disclosure_ref.clone(),
        "mandatory_provenance": metadata.mandatory_provenance,
        "rollback_plan_ref": metadata.rollback_plan_ref.clone(),
        "compensating_action_plan_ref": metadata.compensating_action_plan_ref.clone(),
    })
}

fn verification_budget_hint_sats(
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
) -> u64 {
    let base: u64 = match severity {
        SeverityClass::SeverityClassUnspecified | SeverityClass::Low => 100,
        SeverityClass::Medium => 250,
        SeverityClass::High => 1_000,
        SeverityClass::Critical => 2_500,
    };
    let tfb_multiplier: u64 = match tfb_class {
        FeedbackLatencyClass::FeedbackLatencyClassUnspecified | FeedbackLatencyClass::Short => 1,
        FeedbackLatencyClass::Instant => 1,
        FeedbackLatencyClass::Medium => 2,
        FeedbackLatencyClass::Long => 3,
    };
    let category_multiplier: u64 = match category {
        "compute" => 1,
        _ => 2,
    };
    base.saturating_mul(tfb_multiplier)
        .saturating_mul(category_multiplier)
}

fn allow_policy_decision(action: &str, category: &str, severity: SeverityClass) -> PolicyDecision {
    evaluate_policy_decision(action, category, severity, None, "allow")
}

fn deny_policy_decision(
    action: &str,
    category: &str,
    severity: SeverityClass,
    reason_code: &'static str,
) -> PolicyDecision {
    evaluate_policy_decision(action, category, severity, Some(reason_code), "deny")
}

fn withhold_policy_decision(
    action: &str,
    category: &str,
    severity: SeverityClass,
    reason_code: &'static str,
) -> PolicyDecision {
    evaluate_policy_decision(action, category, severity, Some(reason_code), "withhold")
}

fn evaluate_policy_decision(
    action: &str,
    category: &str,
    severity: SeverityClass,
    reason_code: Option<&str>,
    decision: &'static str,
) -> PolicyDecision {
    let selected = policy_rules()
        .iter()
        .filter(|rule| rule.decision == decision)
        .filter(|rule| match rule.action {
            Some(rule_action) => rule_action == action,
            None => true,
        })
        .filter(|rule| match rule.category {
            Some(rule_category) => rule_category == category,
            None => true,
        })
        .filter(|rule| match rule.severity {
            Some(rule_severity) => rule_severity == severity,
            None => true,
        })
        .filter(|rule| match (rule.reason_code, reason_code) {
            (Some(rule_reason_code), Some(input_reason_code)) => {
                rule_reason_code == input_reason_code
            }
            (Some(_), None) => false,
            (None, _) => true,
        })
        .map(|rule| {
            let precedence = policy_rule_precedence(rule, action, category, severity, reason_code);
            (precedence, rule)
        })
        .min_by(|lhs, rhs| {
            lhs.0
                .cmp(&rhs.0)
                .then_with(|| lhs.1.rule_id.cmp(rhs.1.rule_id))
        });

    let (rule_id, notes) = if let Some((precedence, rule)) = selected {
        (
            rule.rule_id.to_string(),
            format!(
                "policy_rule={} precedence={} decision={} action={} category={} severity={} reason_code={} note={}",
                rule.rule_id,
                precedence,
                decision,
                action,
                category,
                severity.label(),
                reason_code.unwrap_or("NONE"),
                rule.note,
            ),
        )
    } else {
        let synthesized_rule = format!(
            "policy.earn.{}.{}.{}.{}",
            decision,
            normalize_key(category),
            normalize_key(action),
            normalize_key(reason_code.unwrap_or("none")),
        );
        (
            synthesized_rule.clone(),
            format!(
                "policy_rule={} precedence=fallback decision={} action={} category={} severity={} reason_code={} note=fallback deterministic mapping",
                synthesized_rule,
                decision,
                action,
                category,
                severity.label(),
                reason_code.unwrap_or("NONE"),
            ),
        )
    };

    PolicyDecision {
        rule_id,
        decision,
        notes,
    }
}

fn policy_rules() -> &'static [PolicyRule] {
    &[
        PolicyRule {
            rule_id: "policy.earn.compute.preflight_reject.v1",
            decision: "deny",
            action: Some("preflight_reject"),
            category: Some("compute"),
            severity: None,
            reason_code: Some(REASON_CODE_POLICY_PREFLIGHT_REJECTED),
            note: "Reject requests that fail policy preflight checks.",
        },
        PolicyRule {
            rule_id: "policy.earn.compute.execution_failure.v1",
            decision: "deny",
            action: Some("execution_failure"),
            category: Some("compute"),
            severity: None,
            reason_code: Some(REASON_CODE_JOB_FAILED),
            note: "Record failed execution outcomes for accountability.",
        },
        PolicyRule {
            rule_id: "policy.earn.compute.history_failed.v1",
            decision: "deny",
            action: Some("history_failed"),
            category: Some("compute"),
            severity: None,
            reason_code: Some(REASON_CODE_JOB_FAILED),
            note: "Replay historical failures as explicit denied outcomes.",
        },
        PolicyRule {
            rule_id: "policy.earn.compute.paid_requires_wallet_proof.v1",
            decision: "withhold",
            action: Some("paid_transition_requires_wallet_proof"),
            category: Some("compute"),
            severity: None,
            reason_code: Some(REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE),
            note: "Do not mark settlement paid without wallet-authoritative proof.",
        },
        PolicyRule {
            rule_id: "policy.earn.compute.history_paid_requires_wallet_proof.v1",
            decision: "withhold",
            action: Some("history_paid_requires_wallet_proof"),
            category: Some("compute"),
            severity: None,
            reason_code: Some(REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE),
            note: "Historical paid rows require wallet-authoritative payment pointers.",
        },
        PolicyRule {
            rule_id: "policy.earn.compute.rollback_plan_required.v1",
            decision: "withhold",
            action: Some("paid_transition_requires_rollback_plan"),
            category: Some("compute"),
            severity: Some(SeverityClass::High),
            reason_code: Some(REASON_CODE_ROLLBACK_PLAN_REQUIRED),
            note: "High-severity paid transitions require rollback or compensating-action plans.",
        },
        PolicyRule {
            rule_id: "policy.earn.compute.allow_ingress.v1",
            decision: "allow",
            action: Some("ingress"),
            category: Some("compute"),
            severity: None,
            reason_code: None,
            note: "Allow ingress into the provider inbox under default earn policy.",
        },
        PolicyRule {
            rule_id: "policy.earn.compute.allow_history_paid.v1",
            decision: "allow",
            action: Some("history_paid"),
            category: Some("compute"),
            severity: None,
            reason_code: None,
            note: "Allow paid history projection when settlement proof is authoritative.",
        },
        PolicyRule {
            rule_id: "policy.earn.compute.allow_stage.v1",
            decision: "allow",
            action: None,
            category: Some("compute"),
            severity: None,
            reason_code: None,
            note: "Allow standard compute lifecycle transitions by default.",
        },
        PolicyRule {
            rule_id: "policy.earn.default.allow.v1",
            decision: "allow",
            action: None,
            category: None,
            severity: None,
            reason_code: None,
            note: "Global fallback allow.",
        },
        PolicyRule {
            rule_id: "policy.earn.default.deny.v1",
            decision: "deny",
            action: None,
            category: None,
            severity: None,
            reason_code: None,
            note: "Global fallback deny.",
        },
        PolicyRule {
            rule_id: "policy.earn.default.withhold.v1",
            decision: "withhold",
            action: None,
            category: None,
            severity: None,
            reason_code: None,
            note: "Global fallback withhold.",
        },
    ]
}

fn policy_rule_precedence(
    rule: &PolicyRule,
    action: &str,
    category: &str,
    severity: SeverityClass,
    reason_code: Option<&str>,
) -> u8 {
    let mut score = 0u8;
    if rule.action == Some(action) {
        score = score.saturating_add(1);
    }
    if rule.category == Some(category) {
        score = score.saturating_add(1);
    }
    if rule.severity == Some(severity) {
        score = score.saturating_add(1);
    }
    if let (Some(rule_reason_code), Some(input_reason_code)) = (rule.reason_code, reason_code) {
        if rule_reason_code == input_reason_code {
            score = score.saturating_add(1);
        }
    }
    10u8.saturating_sub(score)
}

fn current_policy_bundle() -> PolicyBundleConfig {
    let parsed = std::env::var("OPENAGENTS_EARN_POLICY_BUNDLE_JSON")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .and_then(|value| serde_json::from_str::<PolicyBundleConfig>(&value).ok());
    normalize_policy_bundle(parsed.unwrap_or_else(default_policy_bundle))
}

fn default_policy_bundle() -> PolicyBundleConfig {
    PolicyBundleConfig {
        authentication_rules: vec![
            AuthenticationPolicyRule {
                rule_id: "policy.earn.auth.default.operator.v1".to_string(),
                slice: PolicySliceRule {
                    category: Some("compute".to_string()),
                    tfb_class: None,
                    severity: None,
                },
                role: Some("operator".to_string()),
                min_auth_assurance: Some("authenticated".to_string()),
                require_personhood: false,
            },
            AuthenticationPolicyRule {
                rule_id: "policy.earn.auth.default.safety_signal_reader.v1".to_string(),
                slice: PolicySliceRule {
                    category: Some("compute".to_string()),
                    tfb_class: None,
                    severity: None,
                },
                role: Some("safety_signal_reader".to_string()),
                min_auth_assurance: Some("authenticated".to_string()),
                require_personhood: false,
            },
        ],
        provenance_rules: vec![ProvenancePolicyRule {
            rule_id: "policy.earn.provenance.high_severity.v1".to_string(),
            slice: PolicySliceRule {
                category: Some("compute".to_string()),
                tfb_class: None,
                severity: Some(SeverityClass::High),
            },
            min_grade: Some(ProvenanceGrade::P2Lineage),
            require_provenance_bundle: true,
            require_permissioning_refs: true,
            required_attestation_kinds: vec![
                ProvenanceAttestationKind::ModelVersion,
                ProvenanceAttestationKind::RuntimeIntegrity,
            ],
        }],
        monitoring_rules: vec![MonitoringPolicyRule {
            rule_id: "policy.earn.monitoring.default_drift.v1".to_string(),
            slice: PolicySliceRule {
                category: Some("compute".to_string()),
                tfb_class: None,
                severity: None,
            },
            required_detectors: vec!["detector.drift.core".to_string()],
            drift_alert_threshold_24h: Some(50),
            actions: vec![
                ThrottleActionKind::SetModeDegraded,
                ThrottleActionKind::RequireHumanStep,
            ],
        }],
        risk_pricing_rules: vec![RiskPricingPolicyRule {
            rule_id: "policy.earn.risk_pricing.default.compute.v1".to_string(),
            slice: PolicySliceRule {
                category: Some("compute".to_string()),
                tfb_class: None,
                severity: None,
            },
            base_liability_premium_bps: Some(150),
            xa_multiplier: Some(1.0),
            drift_multiplier: Some(0.5),
            correlated_share_multiplier: Some(0.75),
        }],
        certification_rules: vec![CertificationPolicyRule {
            rule_id: "policy.earn.certification.high_severity.v1".to_string(),
            slice: PolicySliceRule {
                category: Some("compute".to_string()),
                tfb_class: None,
                severity: Some(SeverityClass::High),
            },
            require_certification: true,
            accepted_levels: vec!["level_2".to_string(), "level_3".to_string()],
            enable_safe_harbor_relaxations: true,
        }],
        rollback_rules: vec![
            RollbackPolicyRule {
                rule_id: "policy.earn.rollback.high_severity.v1".to_string(),
                slice: PolicySliceRule {
                    category: Some("compute".to_string()),
                    tfb_class: None,
                    severity: Some(SeverityClass::High),
                },
                require_rollback_plan: true,
                allow_compensating_action_only: false,
            },
            RollbackPolicyRule {
                rule_id: "policy.earn.rollback.critical_severity.v1".to_string(),
                slice: PolicySliceRule {
                    category: Some("compute".to_string()),
                    tfb_class: None,
                    severity: Some(SeverityClass::Critical),
                },
                require_rollback_plan: true,
                allow_compensating_action_only: false,
            },
        ],
        autonomy_rules: vec![
            AutonomyPolicyRule {
                rule_id: "policy.earn.autonomy.xa_elevated.v1".to_string(),
                slice: PolicySliceRule {
                    category: Some("compute".to_string()),
                    tfb_class: None,
                    severity: None,
                },
                min_sv: None,
                min_sv_effective: None,
                max_xa_hat: Some(0.15),
                max_delta_m_hat: None,
                max_correlated_share: Some(0.60),
                max_drift_alerts_24h: None,
                actions: vec![
                    ThrottleActionKind::SetModeDegraded,
                    ThrottleActionKind::RaiseRequiredTier,
                    ThrottleActionKind::TightenEnvelope,
                ],
            },
            AutonomyPolicyRule {
                rule_id: "policy.earn.autonomy.xa_critical.v1".to_string(),
                slice: PolicySliceRule {
                    category: Some("compute".to_string()),
                    tfb_class: None,
                    severity: Some(SeverityClass::High),
                },
                min_sv: Some(0.40),
                min_sv_effective: Some(0.40),
                max_xa_hat: Some(0.40),
                max_delta_m_hat: Some(0.35),
                max_correlated_share: Some(0.85),
                max_drift_alerts_24h: Some(150),
                actions: vec![
                    ThrottleActionKind::SetModeApprovalRequired,
                    ThrottleActionKind::RequireHumanStep,
                    ThrottleActionKind::HaltNewEnvelopes,
                    ThrottleActionKind::DisableWarranties,
                ],
            },
        ],
        anchoring: AnchoringPolicyConfig {
            enabled: true,
            allowed_backends: vec!["bitcoin".to_string(), "nostr".to_string()],
        },
    }
}

fn normalize_policy_bundle(mut bundle: PolicyBundleConfig) -> PolicyBundleConfig {
    bundle
        .authentication_rules
        .sort_by(|lhs, rhs| lhs.rule_id.cmp(&rhs.rule_id));
    bundle
        .provenance_rules
        .sort_by(|lhs, rhs| lhs.rule_id.cmp(&rhs.rule_id));
    bundle
        .monitoring_rules
        .sort_by(|lhs, rhs| lhs.rule_id.cmp(&rhs.rule_id));
    bundle
        .risk_pricing_rules
        .sort_by(|lhs, rhs| lhs.rule_id.cmp(&rhs.rule_id));
    bundle
        .certification_rules
        .sort_by(|lhs, rhs| lhs.rule_id.cmp(&rhs.rule_id));
    bundle
        .rollback_rules
        .sort_by(|lhs, rhs| lhs.rule_id.cmp(&rhs.rule_id));
    bundle
        .autonomy_rules
        .sort_by(|lhs, rhs| lhs.rule_id.cmp(&rhs.rule_id));
    bundle.anchoring.allowed_backends = bundle
        .anchoring
        .allowed_backends
        .iter()
        .map(|backend| normalize_key(backend))
        .filter(|backend| !backend.is_empty())
        .collect::<Vec<_>>();
    bundle.anchoring.allowed_backends.sort();
    bundle.anchoring.allowed_backends.dedup();
    bundle
}

fn anchoring_backend_allowed(bundle: &PolicyBundleConfig, anchor_backend: &str) -> bool {
    if !bundle.anchoring.enabled {
        return false;
    }
    let normalized_backend = normalize_key(anchor_backend);
    if normalized_backend.is_empty() {
        return false;
    }
    bundle
        .anchoring
        .allowed_backends
        .iter()
        .any(|backend| backend == &normalized_backend)
}

fn slice_rule_precedence(
    slice: &PolicySliceRule,
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
) -> Option<u8> {
    if slice
        .category
        .as_deref()
        .is_some_and(|rule_category| rule_category != category)
    {
        return None;
    }
    if slice
        .tfb_class
        .is_some_and(|rule_tfb_class| rule_tfb_class != tfb_class)
    {
        return None;
    }
    if slice
        .severity
        .is_some_and(|rule_severity| rule_severity != severity)
    {
        return None;
    }

    let mut specificity = 0u8;
    if slice.category.is_some() {
        specificity = specificity.saturating_add(1);
    }
    if slice.tfb_class.is_some() {
        specificity = specificity.saturating_add(1);
    }
    if slice.severity.is_some() {
        specificity = specificity.saturating_add(1);
    }
    Some(10u8.saturating_sub(specificity))
}

fn select_best_slice_rule<'a, T, FSlice, FId>(
    rules: &'a [T],
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
    slice_of: FSlice,
    id_of: FId,
) -> Option<(&'a T, u8)>
where
    FSlice: Fn(&T) -> &PolicySliceRule,
    FId: Fn(&T) -> &str,
{
    rules
        .iter()
        .filter_map(|rule| {
            let precedence = slice_rule_precedence(slice_of(rule), category, tfb_class, severity)?;
            Some((rule, precedence))
        })
        .min_by(|lhs, rhs| {
            lhs.1
                .cmp(&rhs.1)
                .then_with(|| id_of(lhs.0).cmp(id_of(rhs.0)))
        })
}

fn select_authentication_rule<'a>(
    bundle: &'a PolicyBundleConfig,
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
    role: &str,
) -> Option<&'a AuthenticationPolicyRule> {
    let role = role.trim().to_ascii_lowercase();
    bundle
        .authentication_rules
        .iter()
        .filter(|rule| {
            rule.role
                .as_deref()
                .is_none_or(|rule_role| rule_role.eq_ignore_ascii_case(role.as_str()))
        })
        .filter_map(|rule| {
            let precedence = slice_rule_precedence(&rule.slice, category, tfb_class, severity)?;
            Some((rule, precedence))
        })
        .min_by(|lhs, rhs| {
            lhs.1
                .cmp(&rhs.1)
                .then_with(|| lhs.0.rule_id.cmp(&rhs.0.rule_id))
        })
        .map(|(rule, _)| rule)
}

fn parse_auth_assurance_level(value: &str) -> Option<AuthAssuranceLevel> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => None,
        "anon" | "anonymous" => Some(AuthAssuranceLevel::Anon),
        "authenticated" | "auth" => Some(AuthAssuranceLevel::Authenticated),
        "org_kyc" | "org-kyc" | "kyc_org" => Some(AuthAssuranceLevel::OrgKyc),
        "personhood" | "proof_of_personhood" | "person" => Some(AuthAssuranceLevel::Personhood),
        "gov_id" | "gov-id" | "government_id" => Some(AuthAssuranceLevel::GovId),
        "hardware_bound" | "hardware-bound" | "hw_bound" => Some(AuthAssuranceLevel::HardwareBound),
        _ => None,
    }
}

fn sanitize_optional_ref(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(ToString::to_string)
}

fn auth_assurance_rank(level: AuthAssuranceLevel) -> u8 {
    match level {
        AuthAssuranceLevel::AuthAssuranceLevelUnspecified => 0,
        AuthAssuranceLevel::Anon => 1,
        AuthAssuranceLevel::Authenticated => 2,
        AuthAssuranceLevel::OrgKyc => 3,
        AuthAssuranceLevel::Personhood => 4,
        AuthAssuranceLevel::GovId => 5,
        AuthAssuranceLevel::HardwareBound => 6,
    }
}

fn auth_assurance_for_identity(caller_identity: &str) -> AuthAssuranceLevel {
    let caller_identity = caller_identity.trim().to_ascii_lowercase();
    if caller_identity.is_empty() {
        return AuthAssuranceLevel::Anon;
    }
    if caller_identity.starts_with("hw:")
        || caller_identity.contains(":hardware")
        || caller_identity.contains("hardware_bound")
    {
        return AuthAssuranceLevel::HardwareBound;
    }
    if caller_identity.starts_with("govid:") || caller_identity.contains("gov_id") {
        return AuthAssuranceLevel::GovId;
    }
    if caller_identity.starts_with("personhood:") || caller_identity.contains("personhood") {
        return AuthAssuranceLevel::Personhood;
    }
    if caller_identity.starts_with("orgkyc:")
        || caller_identity.starts_with("org_kyc:")
        || caller_identity.contains("org_kyc")
    {
        return AuthAssuranceLevel::OrgKyc;
    }
    AuthAssuranceLevel::Authenticated
}

fn personhood_proved_for_identity(caller_identity: &str, level: AuthAssuranceLevel) -> bool {
    if matches!(
        level,
        AuthAssuranceLevel::Personhood
            | AuthAssuranceLevel::GovId
            | AuthAssuranceLevel::HardwareBound
    ) {
        return true;
    }
    caller_identity.to_ascii_lowercase().contains("personhood")
}

fn credential_ref_for_identity(caller_identity: &str, level: AuthAssuranceLevel) -> EvidenceRef {
    let caller_identity = caller_identity.trim();
    let credential_kind = match level {
        AuthAssuranceLevel::AuthAssuranceLevelUnspecified | AuthAssuranceLevel::Anon => {
            "credential_ref_anonymous"
        }
        AuthAssuranceLevel::Authenticated => "credential_ref_authenticated",
        AuthAssuranceLevel::OrgKyc => "credential_ref_org_kyc",
        AuthAssuranceLevel::Personhood => "credential_ref_personhood",
        AuthAssuranceLevel::GovId => "credential_ref_gov_id",
        AuthAssuranceLevel::HardwareBound => "credential_ref_hardware_bound",
    };
    let digest_value = if caller_identity.is_empty() {
        "anonymous".to_string()
    } else {
        caller_identity.to_string()
    };
    EvidenceRef::new(
        credential_kind,
        format!(
            "oa://identity/credentials/{}",
            normalize_key(digest_value.as_str())
        ),
        digest_for_text(digest_value.as_str()),
    )
}

fn evaluate_authentication_gate(
    bundle: &PolicyBundleConfig,
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
    role: &str,
    observed_level: AuthAssuranceLevel,
    personhood_proved: bool,
) -> Result<PolicyDecision, PolicyDecision> {
    let Some(rule) = select_authentication_rule(bundle, category, tfb_class, severity, role) else {
        return Ok(PolicyDecision {
            rule_id: format!(
                "policy.earn.auth.fallback.{}.{}.{}",
                normalize_key(category),
                normalize_key(role),
                severity.label()
            ),
            decision: "allow",
            notes: format!(
                "No authentication rule matched; allowing {} for category={} tfb={} severity={} (fallback deterministic mapping)",
                role,
                category,
                tfb_class.label(),
                severity.label(),
            ),
        });
    };

    let required_level = rule
        .min_auth_assurance
        .as_deref()
        .and_then(parse_auth_assurance_level)
        .unwrap_or(AuthAssuranceLevel::Authenticated);
    let required_personhood = rule.require_personhood;
    let level_ok = auth_assurance_rank(observed_level) >= auth_assurance_rank(required_level);
    let personhood_ok = !required_personhood || personhood_proved;

    let notes = format!(
        "policy_rule={} role={} required_level={:?} observed_level={:?} required_personhood={} observed_personhood={} category={} tfb={} severity={}",
        rule.rule_id,
        role,
        required_level,
        observed_level,
        required_personhood,
        personhood_proved,
        category,
        tfb_class.label(),
        severity.label(),
    );
    if level_ok && personhood_ok {
        return Ok(PolicyDecision {
            rule_id: rule.rule_id.clone(),
            decision: "allow",
            notes,
        });
    }

    Err(PolicyDecision {
        rule_id: rule.rule_id.clone(),
        decision: "withhold",
        notes,
    })
}

fn select_provenance_rule<'a>(
    bundle: &'a PolicyBundleConfig,
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
) -> Option<&'a ProvenancePolicyRule> {
    select_best_slice_rule(
        bundle.provenance_rules.as_slice(),
        category,
        tfb_class,
        severity,
        |rule| &rule.slice,
        |rule| rule.rule_id.as_str(),
    )
    .map(|(rule, _)| rule)
}

fn provenance_features_from_evidence(evidence: &[EvidenceRef]) -> ProvenanceFeatures {
    let mut features = ProvenanceFeatures::default();
    for evidence_ref in evidence {
        match evidence_ref.kind.as_str() {
            "provenance_bundle" => {
                features.has_provenance_bundle = true;
            }
            "data_source_ref" => {
                features.data_source_ref_count = features.data_source_ref_count.saturating_add(1);
            }
            "permissioning_ref" => {
                features.permissioning_ref_count =
                    features.permissioning_ref_count.saturating_add(1);
            }
            kind => {
                if let Some(attestation_kind) = provenance_attestation_kind_from_evidence_kind(kind)
                {
                    features.attestation_kinds.insert(attestation_kind);
                }
            }
        }
    }
    features
}

fn provenance_attestation_kind_from_evidence_kind(kind: &str) -> Option<ProvenanceAttestationKind> {
    match kind {
        "attestation:model_version" => Some(ProvenanceAttestationKind::ModelVersion),
        "attestation:runtime_integrity" => Some(ProvenanceAttestationKind::RuntimeIntegrity),
        _ => None,
    }
}

fn provenance_grade_from_features(features: &ProvenanceFeatures) -> ProvenanceGrade {
    if !features.has_provenance_bundle {
        return ProvenanceGrade::P0Minimal;
    }
    if features.data_source_ref_count == 0 {
        return ProvenanceGrade::P0Minimal;
    }
    if features.permissioning_ref_count == 0 {
        return ProvenanceGrade::P1Toolchain;
    }
    if features
        .attestation_kinds
        .contains(&ProvenanceAttestationKind::ModelVersion)
        && features
            .attestation_kinds
            .contains(&ProvenanceAttestationKind::RuntimeIntegrity)
    {
        return ProvenanceGrade::P3Attested;
    }
    ProvenanceGrade::P2Lineage
}

fn evaluate_provenance_gate(
    bundle: &PolicyBundleConfig,
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
    evidence: &[EvidenceRef],
) -> Result<ProvenanceGrade, PolicyDecision> {
    let features = provenance_features_from_evidence(evidence);
    let observed_grade = provenance_grade_from_features(&features);
    let Some(rule) = select_provenance_rule(bundle, category, tfb_class, severity) else {
        return Ok(observed_grade);
    };
    if rule.require_provenance_bundle && !features.has_provenance_bundle {
        return Err(PolicyDecision {
            rule_id: rule.rule_id.clone(),
            decision: "withhold",
            notes: format!(
                "policy_rule={} missing provenance_bundle category={} tfb={} severity={}",
                rule.rule_id,
                category,
                tfb_class.label(),
                severity.label(),
            ),
        });
    }
    if rule.require_permissioning_refs && features.permissioning_ref_count == 0 {
        return Err(PolicyDecision {
            rule_id: rule.rule_id.clone(),
            decision: "withhold",
            notes: format!(
                "policy_rule={} missing permissioning_ref category={} tfb={} severity={}",
                rule.rule_id,
                category,
                tfb_class.label(),
                severity.label(),
            ),
        });
    }
    let missing_attestation = rule
        .required_attestation_kinds
        .iter()
        .find(|required| !features.attestation_kinds.contains(required))
        .copied();
    if let Some(missing_attestation) = missing_attestation {
        return Err(PolicyDecision {
            rule_id: rule.rule_id.clone(),
            decision: "withhold",
            notes: format!(
                "policy_rule={} missing required_attestation={:?} category={} tfb={} severity={}",
                rule.rule_id,
                missing_attestation,
                category,
                tfb_class.label(),
                severity.label(),
            ),
        });
    }
    if let Some(min_grade) = rule.min_grade {
        if observed_grade < min_grade {
            return Err(PolicyDecision {
                rule_id: rule.rule_id.clone(),
                decision: "withhold",
                notes: format!(
                    "policy_rule={} observed_provenance_grade={:?} below min_grade={:?} category={} tfb={} severity={}",
                    rule.rule_id,
                    observed_grade,
                    min_grade,
                    category,
                    tfb_class.label(),
                    severity.label(),
                ),
            });
        }
    }
    Ok(observed_grade)
}

fn evaluate_rollback_gate(
    bundle: &PolicyBundleConfig,
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
    work_unit_id: &str,
    metadata: &ResolvedWorkMetadata,
) -> Result<PolicyDecision, PolicyDecision> {
    let Some(rule) = select_rollback_rule(bundle, category, tfb_class, severity) else {
        return Ok(PolicyDecision {
            rule_id: format!(
                "policy.earn.rollback.fallback.{}.{}.{}",
                normalize_key(category),
                tfb_class.label(),
                severity.label()
            ),
            decision: "allow",
            notes: format!(
                "No rollback rule matched; allowing category={} tfb={} severity={} work_unit={}",
                category,
                tfb_class.label(),
                severity.label(),
                work_unit_id,
            ),
        });
    };
    if !rule.require_rollback_plan {
        return Ok(PolicyDecision {
            rule_id: rule.rule_id.clone(),
            decision: "allow",
            notes: format!(
                "policy_rule={} rollback_plan_not_required category={} tfb={} severity={} work_unit={}",
                rule.rule_id,
                category,
                tfb_class.label(),
                severity.label(),
                work_unit_id,
            ),
        });
    }

    let has_rollback_plan = metadata.rollback_plan_ref.is_some();
    let has_compensating_action_plan = metadata.compensating_action_plan_ref.is_some();
    let plans_satisfy_rule = if rule.allow_compensating_action_only {
        has_compensating_action_plan
    } else {
        has_rollback_plan || has_compensating_action_plan
    };
    let notes = format!(
        "policy_rule={} require_rollback_plan={} allow_compensating_action_only={} has_rollback_plan={} has_compensating_action_plan={} category={} tfb={} severity={} work_unit={}",
        rule.rule_id,
        rule.require_rollback_plan,
        rule.allow_compensating_action_only,
        has_rollback_plan,
        has_compensating_action_plan,
        category,
        tfb_class.label(),
        severity.label(),
        work_unit_id,
    );
    if plans_satisfy_rule {
        return Ok(PolicyDecision {
            rule_id: rule.rule_id.clone(),
            decision: "allow",
            notes,
        });
    }
    Err(PolicyDecision {
        rule_id: rule.rule_id.clone(),
        decision: "withhold",
        notes,
    })
}

fn append_rollback_plan_evidence(
    evidence: &mut Vec<EvidenceRef>,
    work_unit_id: &str,
    metadata: &ResolvedWorkMetadata,
) {
    if let Some(rollback_plan_ref) = metadata.rollback_plan_ref.as_deref() {
        evidence.push(EvidenceRef::new(
            "rollback_plan_ref",
            rollback_plan_ref,
            digest_for_text(format!("{}:{}", work_unit_id, rollback_plan_ref).as_str()),
        ));
    }
    if let Some(compensating_action_plan_ref) = metadata.compensating_action_plan_ref.as_deref() {
        evidence.push(EvidenceRef::new(
            "compensating_action_plan_ref",
            compensating_action_plan_ref,
            digest_for_text(format!("{}:{}", work_unit_id, compensating_action_plan_ref).as_str()),
        ));
    }
}

fn append_provenance_evidence_for_job_stage(
    evidence: &mut Vec<EvidenceRef>,
    job: &ActiveJobRecord,
    stage: JobLifecycleStage,
) {
    if !matches!(
        stage,
        JobLifecycleStage::Running | JobLifecycleStage::Delivered | JobLifecycleStage::Paid
    ) {
        return;
    }
    let stage_label = stage.label();
    evidence.push(EvidenceRef::new(
        "provenance_bundle",
        format!(
            "oa://provenance/jobs/{}/{stage_label}",
            normalize_key(job.job_id.as_str())
        ),
        digest_for_text(format!("{}:{}:{}", job.job_id, stage_label, job.capability).as_str()),
    ));
    evidence.push(EvidenceRef::new(
        "data_source_ref",
        format!("oa://nip90/request/{}", job.request_id),
        digest_for_text(job.request_id.as_str()),
    ));
    evidence.push(EvidenceRef::new(
        "permissioning_ref",
        format!(
            "oa://permissions/capabilities/{}",
            normalize_key(job.capability.as_str())
        ),
        digest_for_text(job.capability.as_str()),
    ));
    if let Some(provenance) = job.execution_provenance.as_ref() {
        append_ollama_execution_provenance_evidence(evidence, job.job_id.as_str(), provenance);
    } else {
        evidence.push(EvidenceRef::new(
            "attestation:model_version",
            format!(
                "oa://attestations/model/{}",
                normalize_key(job.capability.as_str())
            ),
            digest_for_text(format!("model-version:{}:v1", job.capability).as_str()),
        ));
    }
    if stage == JobLifecycleStage::Paid {
        evidence.push(EvidenceRef::new(
            "attestation:runtime_integrity",
            format!(
                "oa://attestations/runtime/{}",
                normalize_key(job.job_id.as_str())
            ),
            digest_for_text(format!("runtime-integrity:{}:v1", job.job_id).as_str()),
        ));
    }
}

fn append_provenance_evidence_for_history(
    evidence: &mut Vec<EvidenceRef>,
    row: &JobHistoryReceiptRow,
    stage: JobLifecycleStage,
) {
    if !matches!(stage, JobLifecycleStage::Paid | JobLifecycleStage::Failed) {
        return;
    }
    evidence.push(EvidenceRef::new(
        "provenance_bundle",
        format!(
            "oa://provenance/jobs/{}/history",
            normalize_key(row.job_id.as_str())
        ),
        digest_for_text(format!("{}:history", row.job_id).as_str()),
    ));
    evidence.push(EvidenceRef::new(
        "data_source_ref",
        format!(
            "oa://earn/jobs/{}/result",
            normalize_key(row.job_id.as_str())
        ),
        normalize_digest(row.result_hash.as_str()),
    ));
    evidence.push(EvidenceRef::new(
        "permissioning_ref",
        format!(
            "oa://permissions/history_projection/{}",
            normalize_key(row.job_id.as_str())
        ),
        digest_for_text("history_projection"),
    ));
    if let Some(provenance) = row.execution_provenance.as_ref() {
        append_ollama_execution_provenance_evidence(evidence, row.job_id.as_str(), provenance);
    } else {
        evidence.push(EvidenceRef::new(
            "attestation:model_version",
            format!(
                "oa://attestations/model/history/{}",
                normalize_key(row.job_id.as_str())
            ),
            digest_for_text(format!("history-model:{}:v1", row.job_id).as_str()),
        ));
    }
    if stage == JobLifecycleStage::Paid {
        evidence.push(EvidenceRef::new(
            "attestation:runtime_integrity",
            format!(
                "oa://attestations/runtime/history/{}",
                normalize_key(row.job_id.as_str())
            ),
            digest_for_text(format!("history-runtime:{}:v1", row.job_id).as_str()),
        ));
    }
}

fn append_ollama_execution_provenance_evidence(
    evidence: &mut Vec<EvidenceRef>,
    job_id: &str,
    provenance: &crate::local_inference_runtime::LocalInferenceExecutionProvenance,
) {
    let normalized_job_id = normalize_key(job_id);
    evidence.push(EvidenceRef::new(
        "execution_backend_ref",
        format!("oa://autopilot/jobs/{normalized_job_id}/execution/backend"),
        digest_for_text(provenance.base_url.as_str()),
    ));
    evidence.push(EvidenceRef::new(
        "attestation:model_version",
        format!(
            "oa://attestations/model/{}",
            normalize_key(provenance.served_model.as_str())
        ),
        digest_for_text(provenance.served_model.as_str()),
    ));
    evidence.push(EvidenceRef::new(
        "execution_prompt_digest",
        format!("oa://autopilot/jobs/{normalized_job_id}/execution/prompt"),
        provenance.normalized_prompt_digest.clone(),
    ));
    evidence.push(EvidenceRef::new(
        "execution_options_digest",
        format!("oa://autopilot/jobs/{normalized_job_id}/execution/options"),
        provenance.normalized_options_digest.clone(),
    ));
    if let Some(warm_start) = provenance.warm_start {
        let state = if warm_start { "warm" } else { "cold" };
        evidence.push(EvidenceRef::new(
            "execution_warm_state",
            format!("oa://autopilot/jobs/{normalized_job_id}/execution/{state}"),
            digest_for_text(state),
        ));
    }
}

fn ollama_execution_receipt_tags(
    provenance: Option<&crate::local_inference_runtime::LocalInferenceExecutionProvenance>,
) -> BTreeMap<String, String> {
    let Some(provenance) = provenance else {
        return BTreeMap::new();
    };

    let mut tags = BTreeMap::from([("execution.backend".to_string(), provenance.backend.clone())]);
    if let Some(requested_model) = provenance.requested_model.as_deref() {
        tags.insert(
            "execution.model.requested".to_string(),
            requested_model.to_string(),
        );
    }
    tags.insert(
        "execution.model.served".to_string(),
        provenance.served_model.clone(),
    );
    tags.insert(
        "execution.prompt_digest".to_string(),
        provenance.normalized_prompt_digest.clone(),
    );
    tags.insert(
        "execution.options_digest".to_string(),
        provenance.normalized_options_digest.clone(),
    );
    tags.insert(
        "execution.base_url".to_string(),
        provenance.base_url.clone(),
    );
    if let Some(warm_start) = provenance.warm_start {
        tags.insert(
            "execution.warm_start".to_string(),
            if warm_start { "true" } else { "false" }.to_string(),
        );
    }
    if let Some(total_duration_ns) = provenance.total_duration_ns {
        tags.insert(
            "execution.total_duration_ns".to_string(),
            total_duration_ns.to_string(),
        );
    }
    if let Some(load_duration_ns) = provenance.load_duration_ns {
        tags.insert(
            "execution.load_duration_ns".to_string(),
            load_duration_ns.to_string(),
        );
    }
    if let Some(prompt_token_count) = provenance.prompt_token_count {
        tags.insert(
            "execution.prompt_tokens".to_string(),
            prompt_token_count.to_string(),
        );
    }
    if let Some(generated_token_count) = provenance.generated_token_count {
        tags.insert(
            "execution.generated_tokens".to_string(),
            generated_token_count.to_string(),
        );
    }
    tags
}

fn select_monitoring_rule<'a>(
    bundle: &'a PolicyBundleConfig,
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
) -> Option<&'a MonitoringPolicyRule> {
    select_best_slice_rule(
        bundle.monitoring_rules.as_slice(),
        category,
        tfb_class,
        severity,
        |rule| &rule.slice,
        |rule| rule.rule_id.as_str(),
    )
    .map(|(rule, _)| rule)
}

fn select_risk_pricing_rule<'a>(
    bundle: &'a PolicyBundleConfig,
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
) -> Option<&'a RiskPricingPolicyRule> {
    select_best_slice_rule(
        bundle.risk_pricing_rules.as_slice(),
        category,
        tfb_class,
        severity,
        |rule| &rule.slice,
        |rule| rule.rule_id.as_str(),
    )
    .map(|(rule, _)| rule)
}

fn pricing_snapshot_context_from_receipt(receipt: &Receipt) -> Option<PricingSnapshotContext> {
    if receipt.receipt_type != "economy.stats.snapshot_receipt.v1" {
        return None;
    }

    let artifact = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "economy_snapshot_artifact")?;
    let snapshot_id = parse_snapshot_id_from_uri(artifact.uri.as_str())
        .unwrap_or(DEFAULT_PRICING_SNAPSHOT_ID)
        .to_string();
    let snapshot_hash = if artifact.digest.trim().is_empty() {
        DEFAULT_PRICING_SNAPSHOT_HASH.to_string()
    } else {
        artifact.digest.clone()
    };
    let mut metrics = SnapshotPolicyMetrics::default();
    if let Some(metrics_evidence) = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "snapshot_metrics")
    {
        metrics.sv = extract_f64_meta(metrics_evidence, "sv").unwrap_or(0.0);
        metrics.sv_effective =
            extract_f64_meta(metrics_evidence, "sv_effective").unwrap_or(metrics.sv);
        metrics.rho_effective =
            extract_f64_meta(metrics_evidence, "rho_effective").unwrap_or(metrics.sv_effective);
        metrics.xa_hat = extract_f64_meta(metrics_evidence, "xa_hat").unwrap_or(0.0);
        metrics.delta_m_hat = extract_f64_meta(metrics_evidence, "delta_m_hat").unwrap_or(0.0);
        metrics.correlated_verification_share =
            extract_f64_meta(metrics_evidence, "correlated_verification_share").unwrap_or(0.0);
        metrics.drift_alerts_24h =
            extract_u64_meta(metrics_evidence, "drift_alerts_24h").unwrap_or(0);
    }

    Some(PricingSnapshotContext {
        snapshot_id,
        snapshot_hash,
        metrics,
    })
}

fn latest_pricing_snapshot_context(receipts: &[Receipt]) -> PricingSnapshotContext {
    receipts
        .iter()
        .rev()
        .find_map(pricing_snapshot_context_from_receipt)
        .unwrap_or_else(|| PricingSnapshotContext {
            snapshot_id: DEFAULT_PRICING_SNAPSHOT_ID.to_string(),
            snapshot_hash: DEFAULT_PRICING_SNAPSHOT_HASH.to_string(),
            metrics: SnapshotPolicyMetrics::default(),
        })
}

fn compute_liability_pricing_for_settlement(
    receipts: &[Receipt],
    policy_bundle: &PolicyBundleConfig,
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
    execution_price_sats: u64,
    verification_budget_hint_sats: u64,
    certification_context: Option<&CertificationGateContext>,
) -> LiabilityPricingBreakdown {
    let snapshot_context = latest_pricing_snapshot_context(receipts);
    let (
        risk_pricing_rule_id,
        base_liability_premium_bps,
        xa_multiplier,
        drift_multiplier,
        correlated_share_multiplier,
    ) = if let Some(rule) = select_risk_pricing_rule(policy_bundle, category, tfb_class, severity) {
        (
            rule.rule_id.clone(),
            rule.base_liability_premium_bps.unwrap_or(0),
            rule.xa_multiplier.unwrap_or(0.0),
            rule.drift_multiplier.unwrap_or(0.0),
            rule.correlated_share_multiplier.unwrap_or(0.0),
        )
    } else {
        (
            "policy.earn.risk_pricing.none".to_string(),
            0,
            0.0,
            0.0,
            0.0,
        )
    };

    let normalized_drift = (snapshot_context.metrics.drift_alerts_24h as f64 / 100.0).max(0.0);
    let dynamic_multiplier = (1.0
        + (xa_multiplier * snapshot_context.metrics.xa_hat.max(0.0))
        + (drift_multiplier * normalized_drift)
        + (correlated_share_multiplier
            * snapshot_context
                .metrics
                .correlated_verification_share
                .max(0.0)))
    .max(0.0);
    let safe_harbor_relaxation_applied =
        certification_context.is_some_and(|context| context.safe_harbor_relaxation_applied);
    let safe_harbor_multiplier = if safe_harbor_relaxation_applied {
        SAFE_HARBOR_LIABILITY_PREMIUM_DISCOUNT_NUMERATOR as f64
            / SAFE_HARBOR_LIABILITY_PREMIUM_DISCOUNT_DENOMINATOR as f64
    } else {
        1.0
    };
    let effective_liability_premium_bps = ((base_liability_premium_bps as f64) * dynamic_multiplier)
        .mul_add(safe_harbor_multiplier, 0.0)
        .round()
        .clamp(0.0, u32::MAX as f64) as u32;
    let liability_premium_sats =
        compute_bps_charge_sats(execution_price_sats, effective_liability_premium_bps);
    let verification_fee_sats =
        compute_verification_fee_sats(execution_price_sats, verification_budget_hint_sats);

    LiabilityPricingBreakdown {
        execution_price_sats,
        verification_fee_sats,
        liability_premium_sats,
        risk_charge_sats: liability_premium_sats,
        effective_liability_premium_bps,
        pricing_snapshot_id: snapshot_context.snapshot_id,
        pricing_snapshot_hash: snapshot_context.snapshot_hash,
        pricing_metrics: snapshot_context.metrics,
        risk_pricing_rule_id,
        base_liability_premium_bps,
        xa_multiplier,
        drift_multiplier,
        correlated_share_multiplier,
        safe_harbor_relaxation_applied,
        safe_harbor_discount_numerator: SAFE_HARBOR_LIABILITY_PREMIUM_DISCOUNT_NUMERATOR,
        safe_harbor_discount_denominator: SAFE_HARBOR_LIABILITY_PREMIUM_DISCOUNT_DENOMINATOR,
        certification_id: certification_context
            .map(|context| context.certification.certification_id.clone()),
        certification_level: certification_context
            .map(|context| context.certification.certification_level.clone()),
    }
}

fn compute_bps_charge_sats(amount_sats: u64, bps: u32) -> u64 {
    if amount_sats == 0 || bps == 0 {
        return 0;
    }
    let numerator = (amount_sats as u128)
        .saturating_mul(bps as u128)
        .saturating_add(9_999);
    let sats = numerator / 10_000;
    sats.min(u64::MAX as u128) as u64
}

fn compute_verification_fee_sats(
    execution_price_sats: u64,
    verification_budget_hint_sats: u64,
) -> Option<u64> {
    if execution_price_sats == 0 || verification_budget_hint_sats == 0 {
        return None;
    }
    let baseline = (execution_price_sats / 20).max(1);
    Some(baseline.min(verification_budget_hint_sats))
}

fn btc_sats_money(amount_sats: u64) -> Money {
    Money {
        asset: Asset::Btc,
        amount: MoneyAmount::AmountSats(amount_sats),
    }
}

fn append_pricing_evidence(evidence: &mut Vec<EvidenceRef>, pricing: &LiabilityPricingBreakdown) {
    evidence.push(EvidenceRef::new(
        "pricing_snapshot_ref",
        format!("oa://economy/snapshots/{}", pricing.pricing_snapshot_id),
        pricing.pricing_snapshot_hash.clone(),
    ));
    evidence.push(EvidenceRef::new(
        "risk_pricing_rule_ref",
        format!(
            "oa://policy/risk_pricing/{}",
            normalize_key(pricing.risk_pricing_rule_id.as_str())
        ),
        digest_for_text(pricing.risk_pricing_rule_id.as_str()),
    ));
}

fn pricing_payload(pricing: &LiabilityPricingBreakdown) -> serde_json::Value {
    json!({
        "execution_price_sats": pricing.execution_price_sats,
        "verification_fee_sats": pricing.verification_fee_sats,
        "liability_premium_sats": pricing.liability_premium_sats,
        "risk_charge_sats": pricing.risk_charge_sats,
        "effective_liability_premium_bps": pricing.effective_liability_premium_bps,
        "pricing_snapshot_ref": {
            "snapshot_id": pricing.pricing_snapshot_id,
            "snapshot_hash": pricing.pricing_snapshot_hash,
        },
        "pricing_metrics": {
            "sv": pricing.pricing_metrics.sv,
            "sv_effective": pricing.pricing_metrics.sv_effective,
            "rho_effective": pricing.pricing_metrics.rho_effective,
            "xa_hat": pricing.pricing_metrics.xa_hat,
            "delta_m_hat": pricing.pricing_metrics.delta_m_hat,
            "correlated_verification_share": pricing.pricing_metrics.correlated_verification_share,
            "drift_alerts_24h": pricing.pricing_metrics.drift_alerts_24h,
        },
        "risk_pricing_rule": {
            "rule_id": pricing.risk_pricing_rule_id,
            "base_liability_premium_bps": pricing.base_liability_premium_bps,
            "xa_multiplier": pricing.xa_multiplier,
            "drift_multiplier": pricing.drift_multiplier,
            "correlated_share_multiplier": pricing.correlated_share_multiplier,
        },
        "safe_harbor": {
            "relaxation_applied": pricing.safe_harbor_relaxation_applied,
            "discount_numerator": pricing.safe_harbor_discount_numerator,
            "discount_denominator": pricing.safe_harbor_discount_denominator,
            "certification_id": pricing.certification_id,
            "certification_level": pricing.certification_level,
        }
    })
}

fn drift_signal_digest_material(signals: &[DriftSignalSummary]) -> String {
    let mut rows = signals.to_vec();
    rows.sort_by(|lhs, rhs| {
        lhs.detector_id
            .cmp(&rhs.detector_id)
            .then_with(|| lhs.signal_code.cmp(&rhs.signal_code))
    });
    rows.into_iter()
        .map(|signal| {
            format!(
                "{}:{}:{}:{:.6}:{:.6}:{:.6}:{}",
                signal.detector_id,
                signal.signal_code,
                signal.count_24h,
                signal.ratio,
                signal.threshold,
                signal.score,
                signal.alert
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn drift_detector_evidence(signal: &DriftSignalSummary) -> EvidenceRef {
    let mut evidence = EvidenceRef::new(
        "drift_detector_ref",
        format!(
            "oa://economy/drift/detectors/{}",
            normalize_key(signal.detector_id.as_str())
        ),
        digest_for_text(signal.detector_id.as_str()),
    );
    evidence
        .meta
        .insert("detector_id".to_string(), json!(signal.detector_id.clone()));
    evidence
        .meta
        .insert("signal_code".to_string(), json!(signal.signal_code.clone()));
    evidence
}

fn drift_signal_summary_evidence(snapshot_id: &str, signal: &DriftSignalSummary) -> EvidenceRef {
    let mut evidence = EvidenceRef::new(
        "drift_signal_summary",
        format!(
            "oa://economy/drift/snapshots/{}/{}",
            normalize_key(snapshot_id),
            normalize_key(signal.detector_id.as_str())
        ),
        digest_for_text(drift_signal_digest_material(std::slice::from_ref(signal)).as_str()),
    );
    evidence
        .meta
        .insert("detector_id".to_string(), json!(signal.detector_id.clone()));
    evidence
        .meta
        .insert("signal_code".to_string(), json!(signal.signal_code.clone()));
    evidence
        .meta
        .insert("count_24h".to_string(), json!(signal.count_24h));
    evidence
        .meta
        .insert("ratio".to_string(), json!(signal.ratio));
    evidence
        .meta
        .insert("threshold".to_string(), json!(signal.threshold));
    evidence
        .meta
        .insert("score".to_string(), json!(signal.score));
    evidence
        .meta
        .insert("alert".to_string(), json!(signal.alert));
    evidence
}

fn active_drift_alert_detectors_before(receipts: &[Receipt], as_of_ms: i64) -> BTreeSet<String> {
    let window_start_ms = as_of_ms.saturating_sub(DRIFT_WINDOW_MS);
    let mut ordered = receipts
        .iter()
        .filter(|receipt| {
            receipt.created_at_ms <= as_of_ms && receipt.created_at_ms > window_start_ms
        })
        .collect::<Vec<_>>();
    ordered.sort_by(|lhs, rhs| {
        lhs.created_at_ms
            .cmp(&rhs.created_at_ms)
            .then_with(|| lhs.receipt_id.cmp(&rhs.receipt_id))
    });
    let mut active = BTreeSet::<String>::new();
    for receipt in ordered {
        let Some(detector_id) = drift_detector_id_from_receipt(receipt) else {
            continue;
        };
        if receipt.receipt_type == "economy.drift.alert_raised.v1" {
            active.insert(detector_id);
        } else if receipt.receipt_type == "economy.drift.false_positive_confirmed.v1" {
            active.remove(detector_id.as_str());
        }
    }
    active
}

fn drift_detector_id_from_receipt(receipt: &Receipt) -> Option<String> {
    let detector_ref = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "drift_detector_ref")?;
    if let Some(value) = detector_ref.meta.get("detector_id").and_then(Value::as_str) {
        if !value.trim().is_empty() {
            return Some(value.to_string());
        }
    }
    detector_ref
        .uri
        .strip_prefix("oa://economy/drift/detectors/")
        .and_then(|value| {
            if value.trim().is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        })
}

fn parse_snapshot_id_from_uri(uri: &str) -> Option<&str> {
    uri.strip_prefix("oa://economy/snapshots/")
        .and_then(|suffix| {
            let value = suffix.split('/').next().unwrap_or(suffix);
            if value.trim().is_empty() {
                None
            } else {
                Some(value)
            }
        })
}

fn extract_f64_meta(evidence: &EvidenceRef, key: &str) -> Option<f64> {
    evidence.meta.get(key).and_then(Value::as_f64)
}

fn extract_u64_meta(evidence: &EvidenceRef, key: &str) -> Option<u64> {
    evidence.meta.get(key).and_then(Value::as_u64)
}

fn taxonomy_lookup_key(taxonomy_id: &str, taxonomy_version: &str, taxonomy_code: &str) -> String {
    incident_taxonomy_key(&IncidentTaxonomyEntry {
        taxonomy_id: taxonomy_id.to_string(),
        taxonomy_version: taxonomy_version.to_string(),
        code: taxonomy_code.to_string(),
        stable_meaning: "lookup".to_string(),
    })
}

fn canonical_receipt_ids(values: &[String]) -> Vec<String> {
    let mut rows = values
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    rows.sort();
    rows.dedup();
    rows
}

fn canonical_evidence_digests(values: &[String]) -> Result<Vec<String>, String> {
    let mut rows = values
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return Err("evidence_digests cannot be empty".to_string());
    }
    rows.sort();
    rows.dedup();
    if rows.iter().any(|value| !value.starts_with("sha256:")) {
        return Err("evidence_digests must be sha256 digests".to_string());
    }
    Ok(rows)
}

fn canonical_outcome_value(value: &str) -> Option<String> {
    let normalized = normalize_key(value);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn canonical_optional_outcome_value(value: Option<&str>) -> Option<String> {
    value.and_then(canonical_outcome_value)
}

fn canonical_incident_tags(values: &[String]) -> Vec<String> {
    let mut rows = values
        .iter()
        .filter_map(|value| canonical_outcome_value(value.as_str()))
        .collect::<Vec<_>>();
    rows.sort();
    rows.dedup();
    rows
}

#[allow(clippy::too_many_arguments)]
fn incident_digest_for(
    incident_id: &str,
    revision: u32,
    incident_kind: IncidentKind,
    incident_status: IncidentStatus,
    taxonomy_id: &str,
    taxonomy_version: &str,
    taxonomy_code: &str,
    severity: SeverityClass,
    summary: &str,
    reported_at_ms: i64,
    updated_at_ms: i64,
    policy_bundle_id: &str,
    policy_version: &str,
    linked_receipt_ids: &[String],
    rollback_receipt_ids: &[String],
    evidence_digests: &[String],
    supersedes_digest: Option<&str>,
) -> String {
    digest_for_text(
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            incident_id,
            revision,
            incident_kind.label(),
            incident_status.label(),
            taxonomy_id,
            taxonomy_version,
            taxonomy_code,
            severity.label(),
            summary,
            reported_at_ms.max(0),
            updated_at_ms.max(0),
            policy_bundle_id,
            policy_version,
            linked_receipt_ids.join("|"),
            rollback_receipt_ids.join("|"),
            evidence_digests.join("|"),
            supersedes_digest.unwrap_or("none")
        )
        .as_str(),
    )
}

fn incident_object_evidence(incident: &IncidentObject) -> EvidenceRef {
    let mut evidence = EvidenceRef::new(
        "incident_object_ref",
        format!(
            "oa://economy/incidents/{}/revisions/{}",
            incident.incident_id, incident.revision
        ),
        incident.incident_digest.clone(),
    );
    evidence.meta.insert(
        "incident_id".to_string(),
        json!(incident.incident_id.clone()),
    );
    evidence
        .meta
        .insert("revision".to_string(), json!(incident.revision));
    evidence.meta.insert(
        "incident_kind".to_string(),
        json!(incident.incident_kind.label()),
    );
    evidence.meta.insert(
        "incident_status".to_string(),
        json!(incident.incident_status.label()),
    );
    evidence.meta.insert(
        "taxonomy_id".to_string(),
        json!(incident.taxonomy_id.clone()),
    );
    evidence.meta.insert(
        "taxonomy_version".to_string(),
        json!(incident.taxonomy_version.clone()),
    );
    evidence.meta.insert(
        "taxonomy_code".to_string(),
        json!(incident.taxonomy_code.clone()),
    );
    evidence
}

fn simulation_scenarios_as_of(
    incidents: &[IncidentObject],
    receipts: &[Receipt],
    as_of_ms: i64,
) -> Vec<SimulationScenario> {
    let mut latest_ground_truth_cases = BTreeMap::<String, IncidentObject>::new();
    for incident in incidents
        .iter()
        .filter(|incident| {
            incident.incident_kind == IncidentKind::GroundTruthCase
                && incident.updated_at_ms <= as_of_ms.max(0)
        })
        .cloned()
    {
        let keep_new = latest_ground_truth_cases
            .get(incident.incident_id.as_str())
            .is_none_or(|existing| {
                incident.revision > existing.revision
                    || (incident.revision == existing.revision
                        && incident.updated_at_ms > existing.updated_at_ms)
            });
        if keep_new {
            latest_ground_truth_cases.insert(incident.incident_id.clone(), incident);
        }
    }
    let receipt_digest_by_id = receipts
        .iter()
        .filter(|receipt| receipt.created_at_ms <= as_of_ms.max(0))
        .map(|receipt| (receipt.receipt_id.clone(), receipt.canonical_hash.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut receipt_refs_by_ground_truth_case = BTreeMap::<String, BTreeSet<String>>::new();
    for receipt in receipts
        .iter()
        .filter(|receipt| receipt.created_at_ms <= as_of_ms.max(0))
    {
        if !receipt.receipt_type.starts_with("economy.incident.") {
            continue;
        }
        let Some(incident_ref) = receipt
            .evidence
            .iter()
            .find(|evidence| evidence.kind == "incident_object_ref")
        else {
            continue;
        };
        let Some(incident_kind) = incident_ref
            .meta
            .get("incident_kind")
            .and_then(Value::as_str)
        else {
            continue;
        };
        if incident_kind != "ground_truth_case" {
            continue;
        }
        let Some(incident_id) = incident_ref.meta.get("incident_id").and_then(Value::as_str) else {
            continue;
        };
        let entry = receipt_refs_by_ground_truth_case
            .entry(incident_id.to_string())
            .or_default();
        entry.insert(receipt.receipt_id.clone());
    }

    let mut scenarios = latest_ground_truth_cases
        .into_values()
        .map(|ground_truth_case| {
            let linked_receipt_ids =
                canonical_receipt_ids(ground_truth_case.linked_receipt_ids.as_slice());
            let rollback_receipt_ids =
                canonical_receipt_ids(ground_truth_case.rollback_receipt_ids.as_slice());
            let linked_receipt_digests = linked_receipt_ids
                .iter()
                .filter_map(|receipt_id| receipt_digest_by_id.get(receipt_id).cloned())
                .collect::<Vec<_>>();
            let rollback_receipt_digests = rollback_receipt_ids
                .iter()
                .filter_map(|receipt_id| receipt_digest_by_id.get(receipt_id).cloned())
                .collect::<Vec<_>>();
            let derived_from_receipt_ids = receipt_refs_by_ground_truth_case
                .get(ground_truth_case.incident_id.as_str())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>();
            let derived_from_receipt_digests = derived_from_receipt_ids
                .iter()
                .filter_map(|receipt_id| receipt_digest_by_id.get(receipt_id).cloned())
                .collect::<Vec<_>>();
            let harness_ref = simulation_harness_ref(&ground_truth_case);
            let scoring_rubric_ref = simulation_scoring_rubric_ref(&ground_truth_case);
            let scenario_id = format!(
                "simulation:{}:rev{}",
                normalize_key(ground_truth_case.incident_id.as_str()),
                ground_truth_case.revision
            );
            let scenario_digest = simulation_scenario_digest(
                scenario_id.as_str(),
                ground_truth_case.incident_id.as_str(),
                ground_truth_case.incident_digest.as_str(),
                ground_truth_case.taxonomy_id.as_str(),
                ground_truth_case.taxonomy_version.as_str(),
                ground_truth_case.taxonomy_code.as_str(),
                ground_truth_case.severity,
                ground_truth_case.evidence_digests.as_slice(),
                linked_receipt_digests.as_slice(),
                rollback_receipt_digests.as_slice(),
                harness_ref.digest.as_str(),
                scoring_rubric_ref.digest.as_str(),
                derived_from_receipt_digests.as_slice(),
            );
            SimulationScenario {
                scenario_id,
                scenario_digest,
                ground_truth_case_id: ground_truth_case.incident_id,
                ground_truth_case_digest: ground_truth_case.incident_digest,
                taxonomy_id: ground_truth_case.taxonomy_id,
                taxonomy_version: ground_truth_case.taxonomy_version,
                taxonomy_code: ground_truth_case.taxonomy_code,
                severity: ground_truth_case.severity,
                evidence_digests: ground_truth_case.evidence_digests,
                linked_receipt_ids,
                linked_receipt_digests,
                rollback_receipt_ids,
                rollback_receipt_digests,
                harness_ref,
                scoring_rubric_ref,
                derived_from_receipt_ids,
                derived_from_receipt_digests,
            }
        })
        .collect::<Vec<_>>();
    scenarios.sort_by(|lhs, rhs| {
        lhs.scenario_id
            .cmp(&rhs.scenario_id)
            .then_with(|| lhs.ground_truth_case_id.cmp(&rhs.ground_truth_case_id))
    });
    scenarios.truncate(SIMULATION_SCENARIO_ROW_LIMIT);
    scenarios
}

fn simulation_harness_ref(ground_truth_case: &IncidentObject) -> EvidenceRef {
    let mut harness_ref = EvidenceRef::new(
        "simulation_harness_ref",
        format!(
            "oa://benchmarks/harness/{}/{}/{}",
            normalize_key(ground_truth_case.taxonomy_id.as_str()),
            normalize_key(ground_truth_case.taxonomy_version.as_str()),
            normalize_key(ground_truth_case.taxonomy_code.as_str()),
        ),
        digest_for_text(
            format!(
                "simulation_harness:{}:{}:{}",
                ground_truth_case.taxonomy_id,
                ground_truth_case.taxonomy_version,
                ground_truth_case.taxonomy_code
            )
            .as_str(),
        ),
    );
    harness_ref.meta.insert(
        "taxonomy_code".to_string(),
        json!(ground_truth_case.taxonomy_code.clone()),
    );
    harness_ref
}

fn simulation_scoring_rubric_ref(ground_truth_case: &IncidentObject) -> EvidenceRef {
    let mut rubric_ref = EvidenceRef::new(
        "simulation_scoring_rubric_ref",
        format!(
            "oa://benchmarks/rubric/{}/{}/{}",
            normalize_key(ground_truth_case.taxonomy_id.as_str()),
            normalize_key(ground_truth_case.taxonomy_version.as_str()),
            normalize_key(ground_truth_case.taxonomy_code.as_str()),
        ),
        digest_for_text(
            format!(
                "simulation_scoring_rubric:{}:{}:{}:{}",
                ground_truth_case.taxonomy_id,
                ground_truth_case.taxonomy_version,
                ground_truth_case.taxonomy_code,
                ground_truth_case.severity.label(),
            )
            .as_str(),
        ),
    );
    rubric_ref.meta.insert(
        "severity".to_string(),
        json!(ground_truth_case.severity.label()),
    );
    rubric_ref
}

#[allow(clippy::too_many_arguments)]
fn simulation_scenario_digest(
    scenario_id: &str,
    ground_truth_case_id: &str,
    ground_truth_case_digest: &str,
    taxonomy_id: &str,
    taxonomy_version: &str,
    taxonomy_code: &str,
    severity: SeverityClass,
    evidence_digests: &[String],
    linked_receipt_digests: &[String],
    rollback_receipt_digests: &[String],
    harness_digest: &str,
    scoring_rubric_digest: &str,
    derived_from_receipt_digests: &[String],
) -> String {
    digest_for_text(
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            scenario_id,
            ground_truth_case_id,
            ground_truth_case_digest,
            taxonomy_id,
            taxonomy_version,
            taxonomy_code,
            severity.label(),
            evidence_digests.join("|"),
            linked_receipt_digests.join("|"),
            rollback_receipt_digests.join("|"),
            harness_digest,
            format!(
                "{}:{}",
                scoring_rubric_digest,
                derived_from_receipt_digests.join("|")
            ),
        )
        .as_str(),
    )
}

fn normalize_simulation_scenarios(
    mut scenarios: Vec<SimulationScenario>,
) -> Vec<SimulationScenario> {
    scenarios.sort_by(|lhs, rhs| {
        lhs.scenario_id
            .cmp(&rhs.scenario_id)
            .then_with(|| lhs.scenario_digest.cmp(&rhs.scenario_digest))
    });
    scenarios.dedup_by(|lhs, rhs| lhs.scenario_digest == rhs.scenario_digest);
    scenarios.truncate(SIMULATION_SCENARIO_ROW_LIMIT);
    scenarios
}

fn redact_simulation_scenarios(
    scenarios: &[SimulationScenario],
    tier: SimulationScenarioExportRedactionTier,
) -> Vec<SimulationScenario> {
    let mut redacted = scenarios.to_vec();
    if tier == SimulationScenarioExportRedactionTier::Public {
        for scenario in &mut redacted {
            scenario.linked_receipt_ids.clear();
            scenario.rollback_receipt_ids.clear();
            scenario.derived_from_receipt_ids.clear();
        }
    }
    redacted
}

#[derive(Serialize)]
struct CanonicalSimulationScenarioPackagePayload<'a> {
    redaction_tier: SimulationScenarioExportRedactionTier,
    redaction_policy_receipt_id: &'a str,
    export_receipt_id: &'a str,
    scenarios: &'a [SimulationScenario],
}

fn hash_simulation_scenario_package(
    tier: SimulationScenarioExportRedactionTier,
    scenarios: &[SimulationScenario],
    redaction_policy_receipt_id: &str,
    export_receipt_id: &str,
) -> Result<String, String> {
    let payload = CanonicalSimulationScenarioPackagePayload {
        redaction_tier: tier,
        redaction_policy_receipt_id,
        export_receipt_id,
        scenarios,
    };
    let value = serde_json::to_value(payload).map_err(|error| {
        format!("Failed to encode simulation scenario package payload: {error}")
    })?;
    let bytes = serde_json::to_vec(&value).map_err(|error| {
        format!("Failed to encode simulation scenario package hash payload: {error}")
    })?;
    let digest = sha256::Hash::hash(bytes.as_slice());
    Ok(format!("sha256:{digest}"))
}

#[allow(clippy::too_many_arguments)]
fn outcome_registry_digest_for(
    entry_id: &str,
    revision: u32,
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
    verdict_outcome: &str,
    settlement_outcome: &str,
    claim_outcome: Option<&str>,
    remedy_outcome: Option<&str>,
    incident_tags: &[String],
    linked_receipt_ids: &[String],
    evidence_digests: &[String],
    reported_at_ms: i64,
    updated_at_ms: i64,
    policy_bundle_id: &str,
    policy_version: &str,
    supersedes_digest: Option<&str>,
) -> String {
    digest_for_text(
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            entry_id,
            revision,
            category,
            tfb_class.label(),
            severity.label(),
            verdict_outcome,
            settlement_outcome,
            claim_outcome.unwrap_or("none"),
            remedy_outcome.unwrap_or("none"),
            incident_tags.join("|"),
            linked_receipt_ids.join("|"),
            evidence_digests.join("|"),
            reported_at_ms.max(0),
            updated_at_ms.max(0),
            policy_bundle_id,
            policy_version,
            supersedes_digest.unwrap_or("none")
        )
        .as_str(),
    )
}

fn outcome_registry_entry_evidence(entry: &OutcomeRegistryEntry) -> EvidenceRef {
    let mut evidence = EvidenceRef::new(
        "outcome_registry_entry_ref",
        format!(
            "oa://economy/outcome_registry/{}/revisions/{}",
            entry.entry_id, entry.revision
        ),
        entry.entry_digest.clone(),
    );
    evidence
        .meta
        .insert("entry_id".to_string(), json!(entry.entry_id.clone()));
    evidence
        .meta
        .insert("revision".to_string(), json!(entry.revision));
    evidence
        .meta
        .insert("category".to_string(), json!(entry.category.clone()));
    evidence
        .meta
        .insert("tfb_class".to_string(), json!(entry.tfb_class.label()));
    evidence
        .meta
        .insert("severity".to_string(), json!(entry.severity.label()));
    evidence.meta.insert(
        "verdict_outcome".to_string(),
        json!(entry.verdict_outcome.clone()),
    );
    evidence.meta.insert(
        "settlement_outcome".to_string(),
        json!(entry.settlement_outcome.clone()),
    );
    evidence.meta.insert(
        "claim_outcome".to_string(),
        json!(entry.claim_outcome.clone()),
    );
    evidence.meta.insert(
        "remedy_outcome".to_string(),
        json!(entry.remedy_outcome.clone()),
    );
    evidence.meta.insert(
        "incident_tags".to_string(),
        json!(entry.incident_tags.clone()),
    );
    evidence.meta.insert(
        "linked_receipt_ids".to_string(),
        json!(entry.linked_receipt_ids.clone()),
    );
    evidence
}

fn normalize_certification_scopes(
    scopes: &[CertificationScope],
) -> Result<Vec<CertificationScope>, String> {
    let mut rows = scopes
        .iter()
        .map(|scope| CertificationScope {
            category: normalize_key(scope.category.as_str()),
            tfb_class: scope.tfb_class,
            min_severity: scope.min_severity,
            max_severity: scope.max_severity,
        })
        .collect::<Vec<_>>();
    if rows.iter().any(|scope| scope.category.is_empty()) {
        return Err("certification scope category cannot be empty".to_string());
    }
    if rows
        .iter()
        .any(|scope| scope.min_severity > scope.max_severity)
    {
        return Err("certification scope min_severity must be <= max_severity".to_string());
    }
    rows.sort_by(|lhs, rhs| {
        lhs.category
            .cmp(&rhs.category)
            .then_with(|| lhs.tfb_class.cmp(&rhs.tfb_class))
            .then_with(|| lhs.min_severity.cmp(&rhs.min_severity))
            .then_with(|| lhs.max_severity.cmp(&rhs.max_severity))
    });
    rows.dedup();
    Ok(rows)
}

fn certification_scope_payload(scopes: &[CertificationScope]) -> Vec<serde_json::Value> {
    scopes
        .iter()
        .map(|scope| {
            json!({
                "category": scope.category,
                "tfb_class": scope.tfb_class.label(),
                "min_severity": scope.min_severity.label(),
                "max_severity": scope.max_severity.label(),
            })
        })
        .collect::<Vec<_>>()
}

#[allow(clippy::too_many_arguments)]
fn certification_digest_for(
    certification_id: &str,
    revision: u32,
    state: CertificationState,
    certification_level: &str,
    scope: &[CertificationScope],
    valid_from_ms: i64,
    valid_until_ms: i64,
    issuer_credential_kind: &str,
    issuer_credential_digest: &str,
    required_evidence_digests: &[String],
    linked_receipt_ids: &[String],
    issued_at_ms: i64,
    updated_at_ms: i64,
    revoked_reason_code: Option<&str>,
    policy_bundle_id: &str,
    policy_version: &str,
    supersedes_digest: Option<&str>,
) -> String {
    let scope_material = scope
        .iter()
        .map(|scope| {
            format!(
                "{}:{}:{}:{}",
                scope.category,
                scope.tfb_class.label(),
                scope.min_severity.label(),
                scope.max_severity.label(),
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    digest_for_text(
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            certification_id,
            revision,
            state.label(),
            certification_level,
            scope_material,
            valid_from_ms.max(0),
            valid_until_ms.max(0),
            issuer_credential_kind,
            issuer_credential_digest,
            required_evidence_digests.join("|"),
            linked_receipt_ids.join("|"),
            issued_at_ms.max(0),
            updated_at_ms.max(0),
            revoked_reason_code.unwrap_or("none"),
            policy_bundle_id,
            policy_version,
            supersedes_digest.unwrap_or("none"),
        )
        .as_str(),
    )
}

fn certification_object_evidence(certification: &SafetyCertification) -> EvidenceRef {
    let mut evidence = EvidenceRef::new(
        "certification_object_ref",
        format!(
            "oa://economy/certifications/{}/revisions/{}",
            certification.certification_id, certification.revision
        ),
        certification.certification_digest.clone(),
    );
    evidence.meta.insert(
        "certification_id".to_string(),
        json!(certification.certification_id.clone()),
    );
    evidence
        .meta
        .insert("revision".to_string(), json!(certification.revision));
    evidence.meta.insert(
        "state".to_string(),
        json!(certification.state.label().to_string()),
    );
    evidence.meta.insert(
        "certification_level".to_string(),
        json!(certification.certification_level.clone()),
    );
    evidence.meta.insert(
        "scope".to_string(),
        json!(certification_scope_payload(certification.scope.as_slice())),
    );
    evidence.meta.insert(
        "valid_from_ms".to_string(),
        json!(certification.valid_from_ms),
    );
    evidence.meta.insert(
        "valid_until_ms".to_string(),
        json!(certification.valid_until_ms),
    );
    evidence.meta.insert(
        "issuer_credential_kind".to_string(),
        json!(certification.issuer_credential_kind.clone()),
    );
    evidence.meta.insert(
        "issuer_credential_digest".to_string(),
        json!(certification.issuer_credential_digest.clone()),
    );
    evidence.meta.insert(
        "required_evidence_digests".to_string(),
        json!(certification.required_evidence_digests.clone()),
    );
    evidence.meta.insert(
        "linked_receipt_ids".to_string(),
        json!(certification.linked_receipt_ids.clone()),
    );
    evidence.meta.insert(
        "issued_at_ms".to_string(),
        json!(certification.issued_at_ms),
    );
    evidence.meta.insert(
        "updated_at_ms".to_string(),
        json!(certification.updated_at_ms),
    );
    evidence.meta.insert(
        "revoked_reason_code".to_string(),
        json!(certification.revoked_reason_code.clone()),
    );
    evidence.meta.insert(
        "policy_bundle_id".to_string(),
        json!(certification.policy_bundle_id.clone()),
    );
    evidence.meta.insert(
        "policy_version".to_string(),
        json!(certification.policy_version.clone()),
    );
    evidence.meta.insert(
        "supersedes_digest".to_string(),
        json!(certification.supersedes_digest.clone()),
    );
    evidence
}

fn certification_reference_evidence(certification: &SafetyCertification) -> EvidenceRef {
    let mut evidence = EvidenceRef::new(
        "certification_ref",
        format!(
            "oa://economy/certifications/{}",
            certification.certification_id
        ),
        certification.certification_digest.clone(),
    );
    evidence.meta.insert(
        "certification_id".to_string(),
        json!(certification.certification_id.clone()),
    );
    evidence.meta.insert(
        "certification_level".to_string(),
        json!(certification.certification_level.clone()),
    );
    evidence.meta.insert(
        "state".to_string(),
        json!(certification.state.label().to_string()),
    );
    evidence
}

fn certification_hint_fields(
    scope: &[CertificationScope],
) -> (
    Option<String>,
    Option<FeedbackLatencyClass>,
    Option<SeverityClass>,
) {
    let mut normalized = scope.to_vec();
    normalized.sort_by(|lhs, rhs| {
        lhs.category
            .cmp(&rhs.category)
            .then_with(|| lhs.tfb_class.cmp(&rhs.tfb_class))
            .then_with(|| lhs.min_severity.cmp(&rhs.min_severity))
            .then_with(|| lhs.max_severity.cmp(&rhs.max_severity))
    });
    if let Some(primary) = normalized.first() {
        return (
            Some(primary.category.clone()),
            Some(primary.tfb_class),
            Some(primary.max_severity),
        );
    }
    (None, None, None)
}

fn latest_certification_objects_as_of(
    receipts: &[Receipt],
    as_of_ms: i64,
) -> Vec<SafetyCertification> {
    let mut ordered = receipts
        .iter()
        .filter(|receipt| receipt.created_at_ms <= as_of_ms)
        .collect::<Vec<_>>();
    ordered.sort_by(|lhs, rhs| {
        lhs.created_at_ms
            .cmp(&rhs.created_at_ms)
            .then_with(|| lhs.receipt_id.cmp(&rhs.receipt_id))
    });
    let mut latest = BTreeMap::<String, SafetyCertification>::new();
    for receipt in ordered {
        let Some(candidate) = certification_object_from_receipt(receipt) else {
            continue;
        };
        let keep_new = latest
            .get(candidate.certification_id.as_str())
            .is_none_or(|existing| {
                candidate.revision > existing.revision
                    || (candidate.revision == existing.revision
                        && candidate.updated_at_ms > existing.updated_at_ms)
                    || (candidate.revision == existing.revision
                        && candidate.updated_at_ms == existing.updated_at_ms
                        && candidate.certification_digest > existing.certification_digest)
            });
        if keep_new {
            latest.insert(candidate.certification_id.clone(), candidate);
        }
    }

    let mut rows = latest.into_values().collect::<Vec<_>>();
    rows.sort_by(|lhs, rhs| {
        lhs.certification_id
            .cmp(&rhs.certification_id)
            .then_with(|| rhs.revision.cmp(&lhs.revision))
            .then_with(|| lhs.certification_digest.cmp(&rhs.certification_digest))
    });
    rows.truncate(CERTIFICATION_OBJECT_ROW_LIMIT);
    rows
}

fn certification_object_from_receipt(receipt: &Receipt) -> Option<SafetyCertification> {
    let evidence = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "certification_object_ref")?;
    let certification_id = evidence.meta.get("certification_id")?.as_str()?.trim();
    if certification_id.is_empty() {
        return None;
    }
    let revision = evidence
        .meta
        .get("revision")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(u32::MAX as u64) as u32;
    let state = evidence
        .meta
        .get("state")
        .and_then(Value::as_str)
        .and_then(certification_state_from_label)
        .unwrap_or(CertificationState::CertificationStateUnspecified);
    let certification_level = evidence
        .meta
        .get("certification_level")
        .and_then(Value::as_str)
        .map(normalize_key)
        .unwrap_or_else(|| "unknown".to_string());
    let scope = evidence
        .meta
        .get("scope")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|value| {
                    let obj = value.as_object()?;
                    let category = obj.get("category")?.as_str().map(normalize_key)?;
                    let tfb_class = obj
                        .get("tfb_class")
                        .and_then(Value::as_str)
                        .and_then(tfb_class_from_label)
                        .unwrap_or(FeedbackLatencyClass::FeedbackLatencyClassUnspecified);
                    let min_severity = obj
                        .get("min_severity")
                        .and_then(Value::as_str)
                        .and_then(severity_from_label_strict)
                        .unwrap_or(SeverityClass::SeverityClassUnspecified);
                    let max_severity = obj
                        .get("max_severity")
                        .and_then(Value::as_str)
                        .and_then(severity_from_label_strict)
                        .unwrap_or(SeverityClass::SeverityClassUnspecified);
                    Some(CertificationScope {
                        category,
                        tfb_class,
                        min_severity,
                        max_severity,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let valid_from_ms = evidence
        .meta
        .get("valid_from_ms")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0);
    let valid_until_ms = evidence
        .meta
        .get("valid_until_ms")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0);
    let issuer_credential_kind = evidence
        .meta
        .get("issuer_credential_kind")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| "credential_ref_anonymous".to_string());
    let issuer_credential_digest = evidence
        .meta
        .get("issuer_credential_digest")
        .and_then(Value::as_str)
        .map(normalize_digest)
        .unwrap_or_else(|| digest_for_text("unknown_issuer"));
    let required_evidence_digests = evidence
        .meta
        .get("required_evidence_digests")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(Value::as_str)
                .map(normalize_digest)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let linked_receipt_ids = evidence
        .meta
        .get("linked_receipt_ids")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![receipt.receipt_id.clone()]);
    let issued_at_ms = evidence
        .meta
        .get("issued_at_ms")
        .and_then(Value::as_i64)
        .unwrap_or(receipt.created_at_ms)
        .max(0);
    let updated_at_ms = evidence
        .meta
        .get("updated_at_ms")
        .and_then(Value::as_i64)
        .unwrap_or(receipt.created_at_ms)
        .max(0);
    let revoked_reason_code = evidence
        .meta
        .get("revoked_reason_code")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(canonical_reason_code);
    let policy_bundle_id = evidence
        .meta
        .get("policy_bundle_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| receipt.policy.policy_bundle_id.clone());
    let policy_version = evidence
        .meta
        .get("policy_version")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| receipt.policy.policy_version.clone());
    let supersedes_digest = evidence
        .meta
        .get("supersedes_digest")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(normalize_digest);
    Some(SafetyCertification {
        certification_id: certification_id.to_string(),
        certification_digest: evidence.digest.clone(),
        revision,
        state,
        certification_level,
        scope,
        valid_from_ms,
        valid_until_ms,
        issuer_credential_kind,
        issuer_credential_digest,
        required_evidence_digests,
        linked_receipt_ids: canonical_receipt_ids(linked_receipt_ids.as_slice()),
        issued_at_ms,
        updated_at_ms,
        revoked_reason_code,
        policy_bundle_id,
        policy_version,
        supersedes_digest,
    })
}

fn certification_state_from_label(value: &str) -> Option<CertificationState> {
    match normalize_key(value).as_str() {
        "active" => Some(CertificationState::Active),
        "revoked" => Some(CertificationState::Revoked),
        "expired" => Some(CertificationState::Expired),
        "certification_state_unspecified" | "unspecified" => {
            Some(CertificationState::CertificationStateUnspecified)
        }
        _ => None,
    }
}

fn tfb_class_from_label(value: &str) -> Option<FeedbackLatencyClass> {
    match normalize_key(value).as_str() {
        "instant" => Some(FeedbackLatencyClass::Instant),
        "short" => Some(FeedbackLatencyClass::Short),
        "medium" => Some(FeedbackLatencyClass::Medium),
        "long" => Some(FeedbackLatencyClass::Long),
        "feedback_latency_class_unspecified" | "unspecified" => {
            Some(FeedbackLatencyClass::FeedbackLatencyClassUnspecified)
        }
        _ => None,
    }
}

fn severity_from_label_strict(value: &str) -> Option<SeverityClass> {
    match normalize_key(value).as_str() {
        "low" => Some(SeverityClass::Low),
        "medium" => Some(SeverityClass::Medium),
        "high" => Some(SeverityClass::High),
        "critical" => Some(SeverityClass::Critical),
        "severity_class_unspecified" | "unspecified" => {
            Some(SeverityClass::SeverityClassUnspecified)
        }
        _ => None,
    }
}

fn canonical_reason_code(value: &str) -> String {
    normalize_key(value).to_ascii_uppercase()
}

fn select_certification_rule<'a>(
    bundle: &'a PolicyBundleConfig,
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
) -> Option<&'a CertificationPolicyRule> {
    select_best_slice_rule(
        bundle.certification_rules.as_slice(),
        category,
        tfb_class,
        severity,
        |rule| &rule.slice,
        |rule| rule.rule_id.as_str(),
    )
    .map(|(rule, _)| rule)
}

fn evaluate_certification_gate(
    bundle: &PolicyBundleConfig,
    receipts: &[Receipt],
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
    as_of_ms: i64,
) -> Result<CertificationGateContext, CertificationGateFailure> {
    let Some(rule) = select_certification_rule(bundle, category, tfb_class, severity) else {
        let certification = fallback_certification(category, tfb_class, severity, as_of_ms);
        return Ok(CertificationGateContext {
            decision: PolicyDecision {
                rule_id: format!(
                    "policy.earn.certification.fallback.{}.{}.{}",
                    normalize_key(category),
                    tfb_class.label(),
                    severity.label()
                ),
                decision: "allow",
                notes: format!(
                    "No certification rule matched; allowing category={} tfb={} severity={} (fallback deterministic mapping)",
                    category,
                    tfb_class.label(),
                    severity.label(),
                ),
            },
            certification,
            safe_harbor_relaxation_applied: false,
        });
    };

    let certifications = latest_certification_objects_as_of(receipts, as_of_ms.max(0));
    let mut matching = certifications
        .into_iter()
        .filter(|certification| {
            certification.state == CertificationState::Active
                && certification
                    .scope
                    .iter()
                    .any(|scope| certification_scope_matches(scope, category, tfb_class, severity))
        })
        .collect::<Vec<_>>();
    matching.sort_by(|lhs, rhs| {
        lhs.certification_id
            .cmp(&rhs.certification_id)
            .then_with(|| rhs.revision.cmp(&lhs.revision))
            .then_with(|| lhs.certification_level.cmp(&rhs.certification_level))
    });

    if !rule.require_certification {
        let certification = matching
            .first()
            .cloned()
            .unwrap_or_else(|| fallback_certification(category, tfb_class, severity, as_of_ms));
        return Ok(CertificationGateContext {
            decision: PolicyDecision {
                rule_id: rule.rule_id.clone(),
                decision: "allow",
                notes: format!(
                    "policy_rule={} certification_optional=true category={} tfb={} severity={} certification_count={}",
                    rule.rule_id,
                    category,
                    tfb_class.label(),
                    severity.label(),
                    matching.len(),
                ),
            },
            certification,
            safe_harbor_relaxation_applied: false,
        });
    }

    if matching.is_empty() {
        return Err(CertificationGateFailure {
            decision: PolicyDecision {
                rule_id: rule.rule_id.clone(),
                decision: "withhold",
                notes: format!(
                    "policy_rule={} missing_active_certification category={} tfb={} severity={}",
                    rule.rule_id,
                    category,
                    tfb_class.label(),
                    severity.label(),
                ),
            },
            reason_code: REASON_CODE_DIGITAL_BORDER_BLOCK_UNCERTIFIED,
        });
    }

    let accepted_levels = rule
        .accepted_levels
        .iter()
        .map(|value| normalize_key(value))
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>();
    let accepted = if accepted_levels.is_empty() {
        matching.first().cloned()
    } else {
        matching
            .iter()
            .find(|certification| accepted_levels.contains(&certification.certification_level))
            .cloned()
    };
    let Some(certification) = accepted else {
        return Err(CertificationGateFailure {
            decision: PolicyDecision {
                rule_id: rule.rule_id.clone(),
                decision: "withhold",
                notes: format!(
                    "policy_rule={} certification_level_not_accepted accepted_levels={} category={} tfb={} severity={}",
                    rule.rule_id,
                    accepted_levels
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("|"),
                    category,
                    tfb_class.label(),
                    severity.label(),
                ),
            },
            reason_code: REASON_CODE_CERTIFICATION_REQUIRED,
        });
    };

    let safe_harbor_relaxation_applied = rule.enable_safe_harbor_relaxations;
    Ok(CertificationGateContext {
        decision: PolicyDecision {
            rule_id: rule.rule_id.clone(),
            decision: "allow",
            notes: format!(
                "policy_rule={} certification_id={} certification_level={} safe_harbor_relaxation_applied={} category={} tfb={} severity={}",
                rule.rule_id,
                certification.certification_id,
                certification.certification_level,
                safe_harbor_relaxation_applied,
                category,
                tfb_class.label(),
                severity.label(),
            ),
        },
        certification,
        safe_harbor_relaxation_applied,
    })
}

fn fallback_certification(
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
    as_of_ms: i64,
) -> SafetyCertification {
    let scope = vec![CertificationScope {
        category: category.to_string(),
        tfb_class,
        min_severity: severity,
        max_severity: severity,
    }];
    let valid_from_ms = as_of_ms.max(0);
    let valid_until_ms = as_of_ms.max(0);
    let certification_id = format!(
        "certification.fallback.{}.{}.{}",
        normalize_key(category),
        tfb_class.label(),
        severity.label(),
    );
    let certification_level = "none".to_string();
    let linked_receipt_ids = Vec::<String>::new();
    let required_evidence_digests = Vec::<String>::new();
    let digest = certification_digest_for(
        certification_id.as_str(),
        0,
        CertificationState::CertificationStateUnspecified,
        certification_level.as_str(),
        scope.as_slice(),
        valid_from_ms,
        valid_until_ms,
        "credential_ref_anonymous",
        digest_for_text("fallback").as_str(),
        required_evidence_digests.as_slice(),
        linked_receipt_ids.as_slice(),
        valid_from_ms,
        valid_from_ms,
        None,
        "policy.earn.default",
        "1",
        None,
    );
    SafetyCertification {
        certification_id,
        certification_digest: digest,
        revision: 0,
        state: CertificationState::CertificationStateUnspecified,
        certification_level,
        scope,
        valid_from_ms,
        valid_until_ms,
        issuer_credential_kind: "credential_ref_anonymous".to_string(),
        issuer_credential_digest: digest_for_text("fallback"),
        required_evidence_digests,
        linked_receipt_ids,
        issued_at_ms: valid_from_ms,
        updated_at_ms: valid_from_ms,
        revoked_reason_code: None,
        policy_bundle_id: "policy.earn.default".to_string(),
        policy_version: "1".to_string(),
        supersedes_digest: None,
    }
}

fn certification_scope_matches(
    scope: &CertificationScope,
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
) -> bool {
    scope.category == category
        && scope.tfb_class == tfb_class
        && severity >= scope.min_severity
        && severity <= scope.max_severity
}

fn select_rollback_rule<'a>(
    bundle: &'a PolicyBundleConfig,
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
) -> Option<&'a RollbackPolicyRule> {
    select_best_slice_rule(
        bundle.rollback_rules.as_slice(),
        category,
        tfb_class,
        severity,
        |rule| &rule.slice,
        |rule| rule.rule_id.as_str(),
    )
    .map(|(rule, _)| rule)
}

fn autonomy_rule_triggered(rule: &AutonomyPolicyRule, metrics: SnapshotPolicyMetrics) -> bool {
    let mut has_threshold = false;
    let mut triggered = false;

    if let Some(min_sv) = rule.min_sv {
        has_threshold = true;
        if metrics.sv_effective < min_sv {
            triggered = true;
        }
    }
    if let Some(min_sv_effective) = rule.min_sv_effective {
        has_threshold = true;
        if metrics.sv_effective < min_sv_effective {
            triggered = true;
        }
    }
    if let Some(max_xa_hat) = rule.max_xa_hat {
        has_threshold = true;
        if metrics.xa_hat > max_xa_hat {
            triggered = true;
        }
    }
    if let Some(max_delta_m_hat) = rule.max_delta_m_hat {
        has_threshold = true;
        if metrics.delta_m_hat > max_delta_m_hat {
            triggered = true;
        }
    }
    if let Some(max_correlated_share) = rule.max_correlated_share {
        has_threshold = true;
        if metrics.correlated_verification_share > max_correlated_share {
            triggered = true;
        }
    }
    if let Some(max_drift_alerts_24h) = rule.max_drift_alerts_24h {
        has_threshold = true;
        if metrics.drift_alerts_24h > max_drift_alerts_24h {
            triggered = true;
        }
    }

    has_threshold && triggered
}

fn evaluate_triggered_policy_actions(
    bundle: &PolicyBundleConfig,
    category: &str,
    tfb_class: FeedbackLatencyClass,
    severity: SeverityClass,
    metrics: SnapshotPolicyMetrics,
) -> Vec<TriggeredPolicyAction> {
    let mut triggered = Vec::<TriggeredPolicyAction>::new();
    let mut matched_autonomy_rules = bundle
        .autonomy_rules
        .iter()
        .filter_map(|rule| {
            let precedence = slice_rule_precedence(&rule.slice, category, tfb_class, severity)?;
            if !autonomy_rule_triggered(rule, metrics) {
                return None;
            }
            Some((precedence, rule))
        })
        .collect::<Vec<_>>();
    matched_autonomy_rules.sort_by(|lhs, rhs| {
        lhs.0
            .cmp(&rhs.0)
            .then_with(|| lhs.1.rule_id.cmp(&rhs.1.rule_id))
    });
    for (_, rule) in matched_autonomy_rules {
        for action in &rule.actions {
            triggered.push(TriggeredPolicyAction {
                rule_id: rule.rule_id.clone(),
                rule_kind: "autonomy_rule",
                action: *action,
                notes: format!(
                    "matched autonomy rule {} for category={} tfb={} severity={}",
                    rule.rule_id,
                    category,
                    tfb_class.label(),
                    severity.label()
                ),
            });
        }
    }

    let mut matched_monitoring_rules = bundle
        .monitoring_rules
        .iter()
        .filter_map(|rule| {
            let precedence = slice_rule_precedence(&rule.slice, category, tfb_class, severity)?;
            if rule
                .drift_alert_threshold_24h
                .is_none_or(|threshold| metrics.drift_alerts_24h <= threshold)
            {
                return None;
            }
            Some((precedence, rule))
        })
        .collect::<Vec<_>>();
    matched_monitoring_rules.sort_by(|lhs, rhs| {
        lhs.0
            .cmp(&rhs.0)
            .then_with(|| lhs.1.rule_id.cmp(&rhs.1.rule_id))
    });
    for (_, rule) in matched_monitoring_rules {
        for action in &rule.actions {
            triggered.push(TriggeredPolicyAction {
                rule_id: rule.rule_id.clone(),
                rule_kind: "monitoring_rule",
                action: *action,
                notes: format!(
                    "matched monitoring rule {} drift_alerts_24h={} threshold={}",
                    rule.rule_id,
                    metrics.drift_alerts_24h,
                    rule.drift_alert_threshold_24h.unwrap_or(0)
                ),
            });
        }
    }

    let mut seen = BTreeSet::<(String, ThrottleActionKind)>::new();
    triggered.retain(|action| seen.insert((action.rule_id.clone(), action.action)));
    triggered.sort_by(|lhs, rhs| {
        throttle_action_order(lhs.action)
            .cmp(&throttle_action_order(rhs.action))
            .then_with(|| lhs.rule_id.cmp(&rhs.rule_id))
            .then_with(|| lhs.rule_kind.cmp(rhs.rule_kind))
    });
    triggered
}

fn throttle_action_order(action: ThrottleActionKind) -> u8 {
    match action {
        ThrottleActionKind::SetModeDegraded => 10,
        ThrottleActionKind::SetModeApprovalRequired => 11,
        ThrottleActionKind::SetModeHalt => 12,
        ThrottleActionKind::RaiseRequiredTier => 20,
        ThrottleActionKind::RequireHumanStep => 21,
        ThrottleActionKind::RaiseProvenanceGrade => 30,
        ThrottleActionKind::TightenEnvelope => 40,
        ThrottleActionKind::HaltNewEnvelopes => 41,
        ThrottleActionKind::DisableWarranties => 50,
        ThrottleActionKind::CapWarrantyCoverage => 51,
    }
}

impl ThrottleActionKind {
    fn label(self) -> &'static str {
        match self {
            ThrottleActionKind::SetModeDegraded => "set_mode_degraded",
            ThrottleActionKind::SetModeApprovalRequired => "set_mode_approval_required",
            ThrottleActionKind::SetModeHalt => "set_mode_halt",
            ThrottleActionKind::RaiseRequiredTier => "raise_required_tier",
            ThrottleActionKind::RequireHumanStep => "require_human_step",
            ThrottleActionKind::RaiseProvenanceGrade => "raise_provenance_grade",
            ThrottleActionKind::TightenEnvelope => "tighten_envelope",
            ThrottleActionKind::HaltNewEnvelopes => "halt_new_envelopes",
            ThrottleActionKind::DisableWarranties => "disable_warranties",
            ThrottleActionKind::CapWarrantyCoverage => "cap_warranty_coverage",
        }
    }
}

fn policy_decision_evidence(decision: &PolicyDecision) -> EvidenceRef {
    let mut evidence = EvidenceRef::new(
        "policy_decision",
        format!("oa://policy/rules/{}", decision.rule_id),
        digest_for_text(decision.notes.as_str()),
    );
    evidence.meta.insert(
        "rule_id".to_string(),
        serde_json::Value::String(decision.rule_id.clone()),
    );
    evidence.meta.insert(
        "decision".to_string(),
        serde_json::Value::String(decision.decision.to_string()),
    );
    evidence.meta.insert(
        "notes".to_string(),
        serde_json::Value::String(decision.notes.clone()),
    );
    evidence
}

fn tfb_class_for_ttl_seconds(ttl_seconds: u64) -> FeedbackLatencyClass {
    if ttl_seconds <= 60 {
        FeedbackLatencyClass::Instant
    } else if ttl_seconds <= 300 {
        FeedbackLatencyClass::Short
    } else if ttl_seconds <= 1_800 {
        FeedbackLatencyClass::Medium
    } else {
        FeedbackLatencyClass::Long
    }
}

fn severity_for_notional_sats(amount_sats: u64) -> SeverityClass {
    if amount_sats >= 100_000 {
        SeverityClass::Critical
    } else if amount_sats >= 10_000 {
        SeverityClass::High
    } else if amount_sats >= 1_000 {
        SeverityClass::Medium
    } else {
        SeverityClass::Low
    }
}

impl CertificationState {
    fn label(self) -> &'static str {
        match self {
            CertificationState::CertificationStateUnspecified => "unspecified",
            CertificationState::Active => "active",
            CertificationState::Revoked => "revoked",
            CertificationState::Expired => "expired",
        }
    }
}

impl IncidentKind {
    fn label(self) -> &'static str {
        match self {
            IncidentKind::IncidentKindUnspecified => "unspecified",
            IncidentKind::Incident => "incident",
            IncidentKind::NearMiss => "near_miss",
            IncidentKind::GroundTruthCase => "ground_truth_case",
        }
    }
}

impl IncidentStatus {
    fn label(self) -> &'static str {
        match self {
            IncidentStatus::IncidentStatusUnspecified => "unspecified",
            IncidentStatus::Open => "open",
            IncidentStatus::Resolved => "resolved",
        }
    }
}

impl SimulationScenarioExportRedactionTier {
    fn label(self) -> &'static str {
        match self {
            SimulationScenarioExportRedactionTier::Public => "public",
            SimulationScenarioExportRedactionTier::Restricted => "restricted",
        }
    }
}

impl RollbackReceiptType {
    fn label(self) -> &'static str {
        match self {
            RollbackReceiptType::RollbackExecuted => "rollback_executed",
            RollbackReceiptType::RollbackFailed => "rollback_failed",
            RollbackReceiptType::CompensatingActionExecuted => "compensating_action_executed",
        }
    }

    fn receipt_type(self) -> &'static str {
        match self {
            RollbackReceiptType::RollbackExecuted => "economy.rollback.executed.v1",
            RollbackReceiptType::RollbackFailed => "economy.rollback.failed.v1",
            RollbackReceiptType::CompensatingActionExecuted => {
                "economy.compensating_action.executed.v1"
            }
        }
    }

    fn reason_code(self) -> &'static str {
        match self {
            RollbackReceiptType::RollbackExecuted => REASON_CODE_ROLLBACK_EXECUTED,
            RollbackReceiptType::RollbackFailed => REASON_CODE_ROLLBACK_FAILED,
            RollbackReceiptType::CompensatingActionExecuted => {
                REASON_CODE_COMPENSATING_ACTION_EXECUTED
            }
        }
    }

    fn status_label(self) -> &'static str {
        match self {
            RollbackReceiptType::RollbackExecuted => "rollback_executed",
            RollbackReceiptType::RollbackFailed => "rollback_failed",
            RollbackReceiptType::CompensatingActionExecuted => "compensating_action_executed",
        }
    }

    fn default_summary(self) -> &'static str {
        match self {
            RollbackReceiptType::RollbackExecuted => "Rollback executed against incident/claim.",
            RollbackReceiptType::RollbackFailed => "Rollback attempt failed.",
            RollbackReceiptType::CompensatingActionExecuted => {
                "Compensating action executed against incident/claim."
            }
        }
    }
}

fn current_policy_context() -> PolicyContext {
    PolicyContext {
        policy_bundle_id: std::env::var("OPENAGENTS_EARN_POLICY_BUNDLE_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "policy.earn.default".to_string()),
        policy_version: std::env::var("OPENAGENTS_EARN_POLICY_VERSION")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "1".to_string()),
        approved_by: std::env::var("OPENAGENTS_EARN_POLICY_APPROVED_BY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "autopilot-desktop".to_string()),
    }
}

fn trace_for_job(
    job_id: &str,
    request_id: Option<&str>,
    trajectory_session: Option<&str>,
) -> TraceContext {
    TraceContext {
        session_id: trajectory_session.map(ToString::to_string),
        trajectory_hash: trajectory_session.map(digest_for_text),
        job_hash: request_id.map(digest_for_text),
        run_id: request_id.map(|request_id| format!("run:{request_id}")),
        work_unit_id: Some(job_id.to_string()),
        contract_id: None,
        claim_id: None,
    }
}

fn lifecycle_receipt_id(job_id: &str, stage: JobLifecycleStage, authority_key: &str) -> String {
    format!(
        "receipt.earn:{}:{}:{}",
        normalize_key(job_id),
        stage.label(),
        normalize_key(authority_key)
    )
}

fn lifecycle_idempotency_key(action: &str, job_id: &str, authority_key: &str) -> String {
    format!(
        "idemp.earn:{}:{}:{}",
        normalize_key(action),
        normalize_key(job_id),
        normalize_key(authority_key)
    )
}

fn normalize_key(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| if ch.is_whitespace() { '_' } else { ch })
        .collect()
}

fn active_job_link_candidate_receipt_ids(
    job: &ActiveJobRecord,
    stage: JobLifecycleStage,
) -> Vec<String> {
    let ingress_id = lifecycle_receipt_id(
        job.job_id.as_str(),
        JobLifecycleStage::Received,
        job.request_id.as_str(),
    );
    let accepted_id = lifecycle_receipt_id(
        job.job_id.as_str(),
        JobLifecycleStage::Accepted,
        job.request_id.as_str(),
    );
    let running_authority = job
        .sa_tick_request_event_id
        .as_deref()
        .unwrap_or(job.request_id.as_str());
    let running_id = lifecycle_receipt_id(
        job.job_id.as_str(),
        JobLifecycleStage::Running,
        running_authority,
    );
    let delivered_authority = job
        .sa_tick_result_event_id
        .as_deref()
        .unwrap_or(job.request_id.as_str());
    let delivered_id = lifecycle_receipt_id(
        job.job_id.as_str(),
        JobLifecycleStage::Delivered,
        delivered_authority,
    );

    match stage {
        JobLifecycleStage::Received => Vec::new(),
        JobLifecycleStage::Accepted => vec![ingress_id],
        JobLifecycleStage::Running => vec![accepted_id, ingress_id],
        JobLifecycleStage::Delivered => vec![running_id, accepted_id, ingress_id],
        JobLifecycleStage::Paid | JobLifecycleStage::Failed => {
            vec![delivered_id, running_id, accepted_id, ingress_id]
        }
    }
}

fn history_row_link_candidate_receipt_ids(
    row: &JobHistoryReceiptRow,
    stage: JobLifecycleStage,
    request_id: &str,
) -> Vec<String> {
    let ingress_id =
        lifecycle_receipt_id(row.job_id.as_str(), JobLifecycleStage::Received, request_id);
    let accepted_id =
        lifecycle_receipt_id(row.job_id.as_str(), JobLifecycleStage::Accepted, request_id);
    let running_id =
        lifecycle_receipt_id(row.job_id.as_str(), JobLifecycleStage::Running, request_id);
    let delivered_authority = row.sa_tick_result_event_id.as_deref().unwrap_or(request_id);
    let delivered_id = lifecycle_receipt_id(
        row.job_id.as_str(),
        JobLifecycleStage::Delivered,
        delivered_authority,
    );

    match stage {
        JobLifecycleStage::Paid | JobLifecycleStage::Failed => {
            vec![delivered_id, running_id, accepted_id, ingress_id]
        }
        _ => Vec::new(),
    }
}

fn infer_request_id_from_job_id(job_id: &str) -> String {
    job_id
        .strip_prefix("job-")
        .map(ToString::to_string)
        .unwrap_or_else(|| job_id.to_string())
}

fn epoch_seconds_to_ms(epoch_seconds: u64) -> i64 {
    epoch_seconds.saturating_mul(1_000).min(i64::MAX as u64) as i64
}

fn digest_for_text(value: &str) -> String {
    let digest = sha256::Hash::hash(value.as_bytes());
    format!("sha256:{digest}")
}

fn normalize_digest(value: &str) -> String {
    if value.starts_with("sha256:") {
        value.to_ascii_lowercase()
    } else {
        digest_for_text(value)
    }
}

#[derive(Serialize)]
struct CanonicalReceiptBundlePayload<'a> {
    query: &'a ReceiptQuery,
    receipts: &'a [Receipt],
}

fn hash_receipt_bundle(query: &ReceiptQuery, receipts: &[Receipt]) -> Result<String, String> {
    let value = serde_json::to_value(CanonicalReceiptBundlePayload { query, receipts })
        .map_err(|error| format!("Failed to encode receipt bundle payload: {error}"))?;
    let payload = serde_json::to_vec(&value)
        .map_err(|error| format!("Failed to encode receipt bundle hash payload: {error}"))?;
    let digest = sha256::Hash::hash(payload.as_slice());
    Ok(format!("sha256:{digest}"))
}

fn audit_snapshot_bindings(receipts: &[Receipt]) -> Vec<AuditSnapshotBinding> {
    let mut bindings = BTreeMap::<(String, String), BTreeSet<String>>::new();
    for receipt in receipts {
        for evidence in &receipt.evidence {
            let snapshot_id = parse_snapshot_id_for_audit(evidence);
            let snapshot_hash = parse_snapshot_hash_for_audit(evidence);
            let (Some(snapshot_id), Some(snapshot_hash)) = (snapshot_id, snapshot_hash) else {
                continue;
            };
            let entry = bindings.entry((snapshot_id, snapshot_hash)).or_default();
            entry.insert(receipt.receipt_id.clone());
        }
    }
    let mut rows = bindings
        .into_iter()
        .map(
            |((snapshot_id, snapshot_hash), linked_receipt_ids)| AuditSnapshotBinding {
                snapshot_id,
                snapshot_hash,
                linked_receipt_ids: linked_receipt_ids.into_iter().collect::<Vec<_>>(),
            },
        )
        .collect::<Vec<_>>();
    rows.sort_by(|lhs, rhs| {
        lhs.snapshot_id
            .cmp(&rhs.snapshot_id)
            .then_with(|| lhs.snapshot_hash.cmp(&rhs.snapshot_hash))
    });
    rows
}

fn audit_anchor_entry_from_receipt(receipt: &Receipt) -> Option<AuditAnchorEntry> {
    let snapshot_ref = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "snapshot_ref")?;
    let snapshot_id = snapshot_ref
        .uri
        .strip_prefix("oa://economy/snapshots/")
        .map(|value| value.split('/').next().unwrap_or(value).to_string())
        .filter(|value| !value.trim().is_empty())?;
    if !snapshot_ref.digest.starts_with("sha256:") {
        return None;
    }
    let anchor_proof_ref = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "anchor_proof_ref")
        .cloned()?;
    let anchor_backend = anchor_proof_ref
        .meta
        .get("anchor_backend")
        .and_then(Value::as_str)
        .map(normalize_key)
        .filter(|value| !value.is_empty())?;
    let receipt_root_hash = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "receipt_root_ref")
        .map(|evidence| normalize_digest(evidence.digest.as_str()))
        .filter(|digest| digest.starts_with("sha256:"));
    Some(AuditAnchorEntry {
        receipt_id: receipt.receipt_id.clone(),
        anchor_backend,
        snapshot_id,
        snapshot_hash: normalize_digest(snapshot_ref.digest.as_str()),
        receipt_root_hash,
        anchor_proof_ref,
    })
}

fn audit_linkage_edges(
    receipts: &[Receipt],
    incidents: &[IncidentObject],
    certifications: &[AuditCertificationObject],
    anchors: &[AuditAnchorEntry],
    outcomes: &[AuditOutcomeRegistryObject],
) -> Vec<AuditLinkageEdge> {
    let mut edges = BTreeSet::<(String, String, String)>::new();
    for receipt in receipts {
        for evidence in &receipt.evidence {
            if evidence.kind == "receipt_ref" {
                if let Some(to_receipt_id) = parse_receipt_ref_uri(evidence.uri.as_str()) {
                    edges.insert((
                        receipt.receipt_id.clone(),
                        to_receipt_id.to_string(),
                        "receipt_ref".to_string(),
                    ));
                }
            }
        }
    }
    for incident in incidents {
        for receipt_id in &incident.linked_receipt_ids {
            edges.insert((
                format!("incident:{}", incident.incident_id),
                receipt_id.clone(),
                "incident_linked_receipt".to_string(),
            ));
        }
        for receipt_id in &incident.rollback_receipt_ids {
            edges.insert((
                format!("incident:{}", incident.incident_id),
                receipt_id.clone(),
                "incident_rollback_receipt".to_string(),
            ));
        }
    }
    for outcome in outcomes {
        for receipt_id in &outcome.linked_receipt_ids {
            edges.insert((
                format!("outcome_registry:{}", outcome.entry_id),
                receipt_id.clone(),
                "outcome_linked_receipt".to_string(),
            ));
        }
    }
    for certification in certifications {
        for receipt_id in &certification.linked_receipt_ids {
            edges.insert((
                format!("certification:{}", certification.certification_id),
                receipt_id.clone(),
                "certification_linked_receipt".to_string(),
            ));
        }
    }
    for anchor in anchors {
        edges.insert((
            format!("anchor:{}", anchor.receipt_id),
            format!("snapshot:{}", anchor.snapshot_id),
            "anchor_snapshot".to_string(),
        ));
        if let Some(receipt_root_hash) = &anchor.receipt_root_hash {
            edges.insert((
                format!("anchor:{}", anchor.receipt_id),
                format!("receipt_root:{receipt_root_hash}"),
                "anchor_receipt_root".to_string(),
            ));
        }
    }
    let mut rows = edges
        .into_iter()
        .map(
            |(from_receipt_id, to_receipt_id, relation_kind)| AuditLinkageEdge {
                from_receipt_id,
                to_receipt_id,
                relation_kind,
            },
        )
        .collect::<Vec<_>>();
    rows.sort_by(|lhs, rhs| {
        lhs.from_receipt_id
            .cmp(&rhs.from_receipt_id)
            .then_with(|| lhs.to_receipt_id.cmp(&rhs.to_receipt_id))
            .then_with(|| lhs.relation_kind.cmp(&rhs.relation_kind))
    });
    rows.truncate(AUDIT_LINKAGE_ROW_LIMIT);
    rows
}

fn parse_snapshot_id_for_audit(evidence: &EvidenceRef) -> Option<String> {
    if evidence.kind == "snapshot_ref" || evidence.kind == "economy_snapshot_artifact" {
        return evidence
            .uri
            .strip_prefix("oa://economy/snapshots/")
            .map(|value| value.split('/').next().unwrap_or(value).to_string())
            .filter(|value| !value.trim().is_empty());
    }
    if evidence.kind == "pricing_snapshot_ref" {
        if let Some(snapshot_id) = evidence.meta.get("snapshot_id").and_then(Value::as_str) {
            if !snapshot_id.trim().is_empty() {
                return Some(snapshot_id.to_string());
            }
        }
    }
    None
}

fn parse_snapshot_hash_for_audit(evidence: &EvidenceRef) -> Option<String> {
    if evidence.kind == "snapshot_ref" || evidence.kind == "economy_snapshot_artifact" {
        if evidence.digest.starts_with("sha256:") {
            return Some(evidence.digest.clone());
        }
    }
    if evidence.kind == "pricing_snapshot_ref" {
        if let Some(snapshot_hash) = evidence.meta.get("snapshot_hash").and_then(Value::as_str) {
            if snapshot_hash.starts_with("sha256:") {
                return Some(snapshot_hash.to_string());
            }
        }
    }
    None
}

fn redact_receipts_for_public_export(receipts: &mut [Receipt]) {
    for receipt in receipts {
        for evidence in &mut receipt.evidence {
            if should_redact_public_evidence_uri(evidence.kind.as_str()) {
                evidence.uri = "oa://redacted".to_string();
            }
        }
    }
}

fn redact_incidents_for_public_export(incidents: &mut [IncidentObject]) {
    for incident in incidents {
        incident.summary = "[redacted]".to_string();
        incident.linked_receipt_ids.clear();
        incident.rollback_receipt_ids.clear();
    }
}

fn should_redact_public_evidence_uri(kind: &str) -> bool {
    matches!(
        kind,
        "wallet_settlement_proof"
            | "wallet_send_request"
            | "credential_ref_anonymous"
            | "credential_ref_authenticated"
            | "credential_ref_org_kyc"
            | "credential_ref_personhood"
            | "credential_ref_gov_id"
            | "credential_ref_hardware_bound"
            | "withheld_reason"
            | "rollback_summary"
            | "snapshot_metrics"
            | "request_id"
            | "nostr_request"
            | "request_shape"
    ) || kind.starts_with("credential_ref_")
}

#[derive(Serialize)]
struct CanonicalAuditPackagePayload<'a> {
    query: &'a ReceiptQuery,
    redaction_tier: AuditExportRedactionTier,
    receipts: &'a [Receipt],
    incidents: &'a [IncidentObject],
    certifications: &'a [AuditCertificationEntry],
    certification_objects: &'a [AuditCertificationObject],
    anchors: &'a [AuditAnchorEntry],
    outcome_registry_entries: &'a [AuditOutcomeRegistryEntry],
    outcome_registry_objects: &'a [AuditOutcomeRegistryObject],
    snapshot_bindings: &'a [AuditSnapshotBinding],
    linkage_edges: &'a [AuditLinkageEdge],
}

#[allow(clippy::too_many_arguments)]
fn hash_audit_package(
    query: &ReceiptQuery,
    redaction_tier: AuditExportRedactionTier,
    receipts: &[Receipt],
    incidents: &[IncidentObject],
    certifications: &[AuditCertificationEntry],
    certification_objects: &[AuditCertificationObject],
    anchors: &[AuditAnchorEntry],
    outcome_registry_entries: &[AuditOutcomeRegistryEntry],
    outcome_registry_objects: &[AuditOutcomeRegistryObject],
    snapshot_bindings: &[AuditSnapshotBinding],
    linkage_edges: &[AuditLinkageEdge],
) -> Result<String, String> {
    let value = serde_json::to_value(CanonicalAuditPackagePayload {
        query,
        redaction_tier,
        receipts,
        incidents,
        certifications,
        certification_objects,
        anchors,
        outcome_registry_entries,
        outcome_registry_objects,
        snapshot_bindings,
        linkage_edges,
    })
    .map_err(|error| format!("Failed to encode audit package payload: {error}"))?;
    let payload = serde_json::to_vec(&value)
        .map_err(|error| format!("Failed to encode audit package hash payload: {error}"))?;
    let digest = sha256::Hash::hash(payload.as_slice());
    Ok(format!("sha256:{digest}"))
}

fn canonical_safety_signal_reader_role(reader_role: Option<&str>) -> String {
    let role = reader_role
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "auditor".to_string());
    match role.as_str() {
        "auditor" | "verifier" | "underwriter" => "safety_signal_reader".to_string(),
        _ => role,
    }
}

fn enforce_restricted_safety_signal_access(
    signals: &[SafetySignal],
    role: &str,
    caller_identity: &str,
) -> Result<(), String> {
    let policy_bundle = current_policy_bundle();
    let observed_level = auth_assurance_for_identity(caller_identity);
    let personhood_proved = personhood_proved_for_identity(caller_identity, observed_level);
    let mut slices = BTreeSet::<(String, FeedbackLatencyClass, SeverityClass)>::new();
    for signal in signals {
        slices.insert((signal.category.clone(), signal.tfb_class, signal.severity));
    }
    if slices.is_empty() {
        slices.insert((
            "compute".to_string(),
            FeedbackLatencyClass::FeedbackLatencyClassUnspecified,
            SeverityClass::SeverityClassUnspecified,
        ));
    }
    for (category, tfb_class, severity) in slices {
        if let Err(decision) = evaluate_authentication_gate(
            &policy_bundle,
            category.as_str(),
            tfb_class,
            severity,
            role,
            observed_level,
            personhood_proved,
        ) {
            return Err(format!(
                "restricted safety signal feed denied for role={} category={} tfb={} severity={} (rule={}): {}",
                role,
                category,
                tfb_class.label(),
                severity.label(),
                decision.rule_id,
                decision.notes
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DerivedSafetySignalClass {
    Incident,
    Drift,
    Adverse,
}

impl DerivedSafetySignalClass {
    fn label(self) -> &'static str {
        match self {
            DerivedSafetySignalClass::Incident => "incident",
            DerivedSafetySignalClass::Drift => "drift",
            DerivedSafetySignalClass::Adverse => "adverse",
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum IncidentSignalKind {
    Incident,
    NearMiss,
    GroundTruthCase,
}

impl IncidentSignalKind {
    fn source_kind(self) -> &'static str {
        match self {
            IncidentSignalKind::Incident => "incident_reported",
            IncidentSignalKind::NearMiss => "near_miss_reported",
            IncidentSignalKind::GroundTruthCase => "ground_truth_case_recorded",
        }
    }

    fn signal_code(self) -> &'static str {
        match self {
            IncidentSignalKind::Incident => "incident_reported",
            IncidentSignalKind::NearMiss => "near_miss_reported",
            IncidentSignalKind::GroundTruthCase => "ground_truth_case_recorded",
        }
    }
}

fn derive_safety_signals(receipts: &[Receipt]) -> Vec<SafetySignal> {
    let mut ordered = receipts.to_vec();
    ordered.sort_by(|lhs, rhs| {
        lhs.created_at_ms
            .cmp(&rhs.created_at_ms)
            .then_with(|| lhs.receipt_id.cmp(&rhs.receipt_id))
    });
    let mut by_id = BTreeMap::<String, SafetySignal>::new();
    for receipt in &ordered {
        let Some((signal_class, source_kind, taxonomy_code, severity, signal_code)) =
            safety_signal_descriptor_for_receipt(receipt)
        else {
            continue;
        };
        let category = receipt
            .hints
            .category
            .clone()
            .unwrap_or_else(|| "compute".to_string());
        let tfb_class = receipt
            .hints
            .tfb_class
            .unwrap_or(FeedbackLatencyClass::FeedbackLatencyClassUnspecified);
        let signal_id = format!(
            "safety_signal:{}:{}:{}:{}",
            signal_class.label(),
            normalize_key(receipt.receipt_id.as_str()),
            normalize_key(taxonomy_code.as_str()),
            normalize_key(signal_code.as_str()),
        );
        let signal_digest = digest_for_text(
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}",
                signal_id,
                receipt.canonical_hash,
                source_kind,
                taxonomy_code,
                signal_code,
                category,
                tfb_class.label(),
                severity.label()
            )
            .as_str(),
        );
        let hashed_indicators =
            safety_signal_hashed_indicators(receipt, taxonomy_code.as_str(), signal_code.as_str());
        by_id
            .entry(signal_id.clone())
            .or_insert_with(|| SafetySignal {
                signal_id,
                signal_digest,
                source_receipt_id: receipt.receipt_id.clone(),
                source_receipt_type: receipt.receipt_type.clone(),
                signal_class: signal_class.label().to_string(),
                source_kind,
                taxonomy_code,
                signal_code,
                category,
                tfb_class,
                severity,
                hashed_indicators,
                created_at_ms: receipt.created_at_ms.max(0),
            });
    }
    let mut rows = by_id.into_values().collect::<Vec<_>>();
    rows.sort_by(|lhs, rhs| {
        lhs.created_at_ms
            .cmp(&rhs.created_at_ms)
            .then_with(|| lhs.signal_id.cmp(&rhs.signal_id))
    });
    rows.truncate(SAFETY_SIGNAL_ROW_LIMIT);
    rows
}

fn aggregate_safety_signal_buckets(signals: &[SafetySignal]) -> Vec<SafetySignalBucketRow> {
    let mut counts = BTreeMap::<(String, SeverityClass), (u64, u64, u64, u64)>::new();
    for signal in signals {
        let key = (signal.taxonomy_code.clone(), signal.severity);
        let entry = counts.entry(key).or_insert((0, 0, 0, 0));
        entry.0 = entry.0.saturating_add(1);
        match signal.signal_class.as_str() {
            "incident" => entry.1 = entry.1.saturating_add(1),
            "drift" => entry.2 = entry.2.saturating_add(1),
            _ => entry.3 = entry.3.saturating_add(1),
        }
    }
    let total_signal_count = counts
        .values()
        .map(|(count, _, _, _)| *count)
        .fold(0u64, u64::saturating_add);
    let mut rows = counts
        .into_iter()
        .map(
            |(
                (taxonomy_code, severity),
                (signal_count, incident_signal_count, drift_signal_count, adverse_signal_count),
            )| SafetySignalBucketRow {
                taxonomy_code,
                severity,
                signal_count,
                incident_signal_count,
                drift_signal_count,
                adverse_signal_count,
                signal_rate: ratio_u64(signal_count, total_signal_count),
            },
        )
        .collect::<Vec<_>>();
    rows.sort_by(|lhs, rhs| {
        lhs.taxonomy_code
            .cmp(&rhs.taxonomy_code)
            .then_with(|| lhs.severity.cmp(&rhs.severity))
    });
    rows.truncate(SAFETY_SIGNAL_BUCKET_ROW_LIMIT);
    rows
}

fn safety_signal_descriptor_for_receipt(
    receipt: &Receipt,
) -> Option<(
    DerivedSafetySignalClass,
    String,
    String,
    SeverityClass,
    String,
)> {
    if let Some((kind, taxonomy_code, severity)) = incident_signal_descriptor(receipt) {
        return Some((
            DerivedSafetySignalClass::Incident,
            kind.source_kind().to_string(),
            taxonomy_code,
            severity,
            kind.signal_code().to_string(),
        ));
    }
    if let Some((source_kind, taxonomy_code, severity, signal_code)) =
        drift_signal_descriptor(receipt)
    {
        return Some((
            DerivedSafetySignalClass::Drift,
            source_kind,
            taxonomy_code,
            severity,
            signal_code,
        ));
    }
    if let Some((source_kind, taxonomy_code, severity, signal_code)) =
        adverse_signal_descriptor(receipt)
    {
        return Some((
            DerivedSafetySignalClass::Adverse,
            source_kind,
            taxonomy_code,
            severity,
            signal_code,
        ));
    }
    None
}

fn incident_signal_descriptor(
    receipt: &Receipt,
) -> Option<(IncidentSignalKind, String, SeverityClass)> {
    let has_incident_ref = receipt
        .evidence
        .iter()
        .any(|evidence| evidence.kind == "incident_object_ref");
    if !receipt.receipt_type.starts_with("economy.incident.") && !has_incident_ref {
        return None;
    }
    let kind = incident_signal_kind_from_receipt(receipt);
    let taxonomy_code =
        incident_taxonomy_code_from_receipt(receipt).unwrap_or_else(|| "unknown".to_string());
    let severity = incident_severity_from_receipt(receipt);
    Some((kind, taxonomy_code, severity))
}

fn incident_signal_kind_from_receipt(receipt: &Receipt) -> IncidentSignalKind {
    let kind_from_meta = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "incident_object_ref")
        .and_then(|evidence| evidence.meta.get("incident_kind"))
        .and_then(Value::as_str)
        .unwrap_or("");
    match kind_from_meta {
        "near_miss" => IncidentSignalKind::NearMiss,
        "ground_truth_case" => IncidentSignalKind::GroundTruthCase,
        _ => {
            let lower_type = receipt.receipt_type.to_ascii_lowercase();
            if lower_type.contains("near_miss") {
                IncidentSignalKind::NearMiss
            } else if lower_type.contains("ground_truth") {
                IncidentSignalKind::GroundTruthCase
            } else {
                IncidentSignalKind::Incident
            }
        }
    }
}

fn incident_taxonomy_code_from_receipt(receipt: &Receipt) -> Option<String> {
    receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "incident_object_ref")
        .and_then(|evidence| evidence.meta.get("taxonomy_code"))
        .and_then(Value::as_str)
        .and_then(normalized_safety_signal_label)
}

fn incident_severity_from_receipt(receipt: &Receipt) -> SeverityClass {
    if let Some(severity_label) = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "incident_object_ref")
        .and_then(|evidence| evidence.meta.get("severity"))
        .and_then(Value::as_str)
    {
        return severity_from_label(severity_label);
    }
    receipt
        .hints
        .severity
        .unwrap_or(SeverityClass::SeverityClassUnspecified)
}

fn drift_signal_descriptor(receipt: &Receipt) -> Option<(String, String, SeverityClass, String)> {
    let has_drift_ref = receipt
        .evidence
        .iter()
        .any(|evidence| evidence.kind == "drift_detector_ref");
    if !receipt.receipt_type.starts_with("economy.drift.") && !has_drift_ref {
        return None;
    }
    let signal_code = drift_signal_code_from_receipt(receipt);
    let taxonomy_code = format!("drift.{}", signal_code);
    let source_kind = if receipt.receipt_type == "economy.drift.alert_raised.v1" {
        "drift_alert_raised".to_string()
    } else if receipt.receipt_type == "economy.drift.false_positive_confirmed.v1" {
        "drift_false_positive_confirmed".to_string()
    } else {
        "drift_signal_emitted".to_string()
    };
    let severity = drift_severity_from_receipt(receipt);
    Some((source_kind, taxonomy_code, severity, signal_code))
}

fn drift_signal_code_from_receipt(receipt: &Receipt) -> String {
    let from_meta = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "drift_signal_summary")
        .and_then(|evidence| evidence.meta.get("signal_code"))
        .and_then(Value::as_str)
        .and_then(normalized_safety_signal_label);
    if let Some(signal_code) = from_meta {
        return signal_code;
    }
    let from_detector = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "drift_detector_ref")
        .and_then(|evidence| evidence.meta.get("signal_code"))
        .and_then(Value::as_str)
        .and_then(normalized_safety_signal_label);
    if let Some(signal_code) = from_detector {
        return signal_code;
    }
    if let Some(reason_code) = receipt
        .hints
        .reason_code
        .as_deref()
        .and_then(normalized_safety_signal_label)
    {
        return reason_code;
    }
    "unknown".to_string()
}

fn drift_severity_from_receipt(receipt: &Receipt) -> SeverityClass {
    if receipt.receipt_type == "economy.drift.alert_raised.v1" {
        return SeverityClass::High;
    }
    if receipt.receipt_type == "economy.drift.false_positive_confirmed.v1" {
        return SeverityClass::Low;
    }
    if let Some(score) = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "drift_signal_summary")
        .and_then(|evidence| evidence.meta.get("score"))
        .and_then(Value::as_f64)
    {
        if score >= 0.85 {
            return SeverityClass::Critical;
        }
        if score >= 0.55 {
            return SeverityClass::High;
        }
        if score >= 0.20 {
            return SeverityClass::Medium;
        }
        return SeverityClass::Low;
    }
    receipt.hints.severity.unwrap_or(SeverityClass::Medium)
}

fn adverse_signal_descriptor(receipt: &Receipt) -> Option<(String, String, SeverityClass, String)> {
    if receipt.receipt_type == "economy.policy.throttle_action_applied.v1"
        || receipt
            .hints
            .reason_code
            .as_deref()
            .is_some_and(|reason| reason == REASON_CODE_POLICY_THROTTLE_TRIGGERED)
    {
        return Some((
            "policy_throttle".to_string(),
            "policy.throttle".to_string(),
            receipt.hints.severity.unwrap_or(SeverityClass::High),
            "policy_throttle_triggered".to_string(),
        ));
    }
    let lower_receipt_type = receipt.receipt_type.to_ascii_lowercase();
    let reason_code = receipt
        .hints
        .reason_code
        .as_deref()
        .and_then(normalized_safety_signal_label)
        .unwrap_or_else(|| "unknown".to_string());
    if lower_receipt_type.contains("rollback")
        || lower_receipt_type.contains("compensating_action")
        || reason_code.contains("rollback")
        || reason_code.contains("compensating")
    {
        let signal_code = if reason_code == "unknown" {
            "rollback_event".to_string()
        } else {
            reason_code
        };
        return Some((
            "rollback_action".to_string(),
            "rollback.action".to_string(),
            receipt.hints.severity.unwrap_or(SeverityClass::High),
            signal_code,
        ));
    }
    if lower_receipt_type.contains("claim")
        || lower_receipt_type.contains("dispute")
        || receipt
            .evidence
            .iter()
            .any(|evidence| evidence.kind == "claim_payout_proof")
    {
        return Some((
            "claim_dispute".to_string(),
            "finance.claim_dispute".to_string(),
            receipt.hints.severity.unwrap_or(SeverityClass::High),
            "claim_dispute_observed".to_string(),
        ));
    }
    None
}

fn normalized_safety_signal_label(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase().replace(' ', "_");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn severity_from_label(value: &str) -> SeverityClass {
    match value.trim().to_ascii_lowercase().as_str() {
        "critical" => SeverityClass::Critical,
        "high" => SeverityClass::High,
        "medium" => SeverityClass::Medium,
        "low" => SeverityClass::Low,
        _ => SeverityClass::SeverityClassUnspecified,
    }
}

fn safety_signal_hashed_indicators(
    receipt: &Receipt,
    taxonomy_code: &str,
    signal_code: &str,
) -> Vec<String> {
    let mut indicators = BTreeSet::<String>::new();
    indicators.insert(normalize_digest(receipt.canonical_hash.as_str()));
    indicators.insert(digest_for_text(
        format!("taxonomy:{taxonomy_code}:signal:{signal_code}").as_str(),
    ));
    for evidence in &receipt.evidence {
        if matches!(
            evidence.kind.as_str(),
            "incident_object_ref"
                | "drift_detector_ref"
                | "drift_signal_summary"
                | "claim_payout_proof"
                | "outcome_registry_entry_ref"
        ) {
            indicators.insert(normalize_digest(evidence.digest.as_str()));
        }
    }
    indicators.into_iter().collect::<Vec<_>>()
}

fn ratio_u64(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

#[derive(Serialize)]
struct CanonicalSafetySignalFeedPayload<'a> {
    query: &'a ReceiptQuery,
    export_mode: SafetySignalExportMode,
    signals: &'a [SafetySignal],
    buckets: &'a [SafetySignalBucketRow],
}

fn hash_safety_signal_feed(
    query: &ReceiptQuery,
    export_mode: SafetySignalExportMode,
    signals: &[SafetySignal],
    buckets: &[SafetySignalBucketRow],
) -> Result<String, String> {
    let value = serde_json::to_value(CanonicalSafetySignalFeedPayload {
        query,
        export_mode,
        signals,
        buckets,
    })
    .map_err(|error| format!("Failed to encode safety signal feed payload: {error}"))?;
    let payload = serde_json::to_vec(&value)
        .map_err(|error| format!("Failed to encode safety signal feed hash payload: {error}"))?;
    let digest = sha256::Hash::hash(payload.as_slice());
    Ok(format!("sha256:{digest}"))
}

fn parse_receipt_ref_uri(uri: &str) -> Option<&str> {
    uri.strip_prefix("oa://receipts/")
}

fn is_wallet_authoritative_payment_pointer(pointer: Option<&str>) -> bool {
    let Some(pointer) = pointer else {
        return false;
    };
    let pointer = pointer.trim();
    !pointer.is_empty()
        && !pointer.starts_with("pending:")
        && !pointer.starts_with("pay:")
        && !pointer.starts_with("inv-")
        && !pointer.starts_with("pay-req-")
}

fn earn_kernel_receipts_file_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-earn-kernel-receipts-v1.json")
}

fn normalize_receipts(mut receipts: Vec<Receipt>) -> Vec<Receipt> {
    receipts.sort_by(|lhs, rhs| {
        rhs.created_at_ms
            .cmp(&lhs.created_at_ms)
            .then_with(|| lhs.receipt_id.cmp(&rhs.receipt_id))
    });
    receipts.truncate(EARN_KERNEL_RECEIPT_ROW_LIMIT);
    receipts
}

fn normalize_work_units(work_units: &mut BTreeMap<String, WorkUnitMetadata>) {
    if work_units.len() <= EARN_WORK_UNIT_METADATA_ROW_LIMIT {
        return;
    }
    let mut keys = work_units.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    let to_remove = keys.len().saturating_sub(EARN_WORK_UNIT_METADATA_ROW_LIMIT);
    for key in keys.into_iter().take(to_remove) {
        work_units.remove(key.as_str());
    }
}

fn normalize_idempotency_records(records: &mut BTreeMap<String, IdempotencyRecord>) {
    if records.len() <= EARN_IDEMPOTENCY_RECORD_ROW_LIMIT {
        return;
    }
    let mut keys = records.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    let to_remove = keys.len().saturating_sub(EARN_IDEMPOTENCY_RECORD_ROW_LIMIT);
    for key in keys.into_iter().take(to_remove) {
        records.remove(key.as_str());
    }
}

fn normalize_incident_objects(mut incidents: Vec<IncidentObject>) -> Vec<IncidentObject> {
    incidents.sort_by(|lhs, rhs| {
        rhs.updated_at_ms
            .cmp(&lhs.updated_at_ms)
            .then_with(|| lhs.incident_id.cmp(&rhs.incident_id))
            .then_with(|| lhs.revision.cmp(&rhs.revision))
    });
    incidents.truncate(INCIDENT_OBJECT_ROW_LIMIT);
    incidents
}

fn normalize_outcome_registry_entries(
    mut entries: Vec<OutcomeRegistryEntry>,
) -> Vec<OutcomeRegistryEntry> {
    entries.sort_by(|lhs, rhs| {
        rhs.updated_at_ms
            .cmp(&lhs.updated_at_ms)
            .then_with(|| lhs.entry_id.cmp(&rhs.entry_id))
            .then_with(|| lhs.revision.cmp(&rhs.revision))
    });
    entries.truncate(OUTCOME_REGISTRY_ROW_LIMIT);
    entries
}

fn normalize_incident_taxonomy_registry(registry: &mut BTreeMap<String, IncidentTaxonomyEntry>) {
    if registry.len() <= INCIDENT_TAXONOMY_ROW_LIMIT {
        return;
    }
    let mut keys = registry.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    let to_remove = keys.len().saturating_sub(INCIDENT_TAXONOMY_ROW_LIMIT);
    for key in keys.into_iter().take(to_remove) {
        registry.remove(key.as_str());
    }
}

fn incident_taxonomy_key(entry: &IncidentTaxonomyEntry) -> String {
    format!(
        "{}:{}:{}",
        normalize_key(entry.taxonomy_id.as_str()),
        normalize_key(entry.taxonomy_version.as_str()),
        normalize_key(entry.code.as_str())
    )
}

fn default_incident_taxonomy_registry() -> BTreeMap<String, IncidentTaxonomyEntry> {
    let defaults = vec![
        IncidentTaxonomyEntry {
            taxonomy_id: "oa.incident.taxonomy".to_string(),
            taxonomy_version: "2026.02".to_string(),
            code: "ops.execution_failure".to_string(),
            stable_meaning: "Execution failure with user-visible impact".to_string(),
        },
        IncidentTaxonomyEntry {
            taxonomy_id: "oa.incident.taxonomy".to_string(),
            taxonomy_version: "2026.02".to_string(),
            code: "safety.near_miss".to_string(),
            stable_meaning: "Near miss caught before external impact".to_string(),
        },
        IncidentTaxonomyEntry {
            taxonomy_id: "oa.incident.taxonomy".to_string(),
            taxonomy_version: "2026.02".to_string(),
            code: "safety.ground_truth_case".to_string(),
            stable_meaning: "Ground truth case for benchmark/simulation replay".to_string(),
        },
        IncidentTaxonomyEntry {
            taxonomy_id: "oa.incident.taxonomy".to_string(),
            taxonomy_version: "2026.02".to_string(),
            code: "finance.claim_dispute".to_string(),
            stable_meaning: "Claim or dispute event with financial impact".to_string(),
        },
    ];
    let mut registry = BTreeMap::<String, IncidentTaxonomyEntry>::new();
    for entry in defaults {
        registry.insert(incident_taxonomy_key(&entry), entry);
    }
    registry
}

fn upsert_incident_taxonomy_entry(
    registry: &mut BTreeMap<String, IncidentTaxonomyEntry>,
    entry: IncidentTaxonomyEntry,
) -> Result<(), String> {
    if entry.taxonomy_id.trim().is_empty()
        || entry.taxonomy_version.trim().is_empty()
        || entry.code.trim().is_empty()
        || entry.stable_meaning.trim().is_empty()
    {
        return Err("incident taxonomy entry fields must be non-empty".to_string());
    }
    let key = incident_taxonomy_key(&entry);
    if let Some(existing) = registry.get(key.as_str()) {
        if existing.stable_meaning != entry.stable_meaning {
            return Err(format!(
                "taxonomy meaning changed for {}:{}:{}",
                entry.taxonomy_id, entry.taxonomy_version, entry.code
            ));
        }
        return Ok(());
    }
    registry.insert(key, entry);
    normalize_incident_taxonomy_registry(registry);
    Ok(())
}

fn persist_earn_kernel_receipts(
    path: &Path,
    receipts: &[Receipt],
    work_units: &[WorkUnitMetadata],
    idempotency_records: &[IdempotencyRecord],
    incident_objects: &[IncidentObject],
    outcome_registry_entries: &[OutcomeRegistryEntry],
    incident_taxonomy_registry: &[IncidentTaxonomyEntry],
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create economy-kernel receipt dir: {error}"))?;
    }

    let document = EarnKernelReceiptDocumentV1 {
        schema_version: EARN_KERNEL_RECEIPT_SCHEMA_VERSION,
        stream_id: EARN_KERNEL_RECEIPT_STREAM_ID.to_string(),
        authority: EARN_KERNEL_RECEIPT_AUTHORITY.to_string(),
        receipts: normalize_receipts(receipts.to_vec()),
        work_units: work_units.to_vec(),
        idempotency_records: idempotency_records.to_vec(),
        incident_objects: normalize_incident_objects(incident_objects.to_vec()),
        outcome_registry_entries: normalize_outcome_registry_entries(
            outcome_registry_entries.to_vec(),
        ),
        incident_taxonomy_registry: incident_taxonomy_registry.to_vec(),
    };
    let payload = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Failed to encode economy-kernel receipts: {error}"))?;
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, payload)
        .map_err(|error| format!("Failed to write economy-kernel receipts temp file: {error}"))?;
    std::fs::rename(&temp_path, path)
        .map_err(|error| format!("Failed to persist economy-kernel receipts: {error}"))?;
    Ok(())
}

fn load_earn_kernel_receipts(path: &Path) -> Result<LoadedReceiptState, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LoadedReceiptState {
                receipts: Vec::new(),
                work_units: BTreeMap::new(),
                idempotency_index: BTreeMap::new(),
                incident_objects: Vec::new(),
                outcome_registry_entries: Vec::new(),
                incident_taxonomy_registry: default_incident_taxonomy_registry(),
            });
        }
        Err(error) => {
            return Err(format!("Failed to read economy-kernel receipts: {error}"));
        }
    };
    let document = serde_json::from_str::<EarnKernelReceiptDocumentV1>(&raw)
        .map_err(|error| format!("Failed to parse economy-kernel receipts: {error}"))?;
    if document.schema_version != EARN_KERNEL_RECEIPT_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported economy-kernel receipt schema version: {}",
            document.schema_version
        ));
    }
    if document.stream_id != EARN_KERNEL_RECEIPT_STREAM_ID {
        return Err(format!(
            "Unsupported economy-kernel receipt stream id: {}",
            document.stream_id
        ));
    }
    if document.authority != EARN_KERNEL_RECEIPT_AUTHORITY {
        return Err(format!(
            "Unsupported economy-kernel receipt authority marker: {}",
            document.authority
        ));
    }
    let mut work_units = document
        .work_units
        .into_iter()
        .filter(|metadata| !metadata.work_unit_id.trim().is_empty())
        .map(|metadata| (metadata.work_unit_id.clone(), metadata))
        .collect::<BTreeMap<_, _>>();
    normalize_work_units(&mut work_units);
    let mut idempotency_index = document
        .idempotency_records
        .into_iter()
        .filter(|record| {
            !record.scope.trim().is_empty() && !record.idempotency_key.trim().is_empty()
        })
        .map(|record| {
            (
                idempotency_lookup_key(record.scope.as_str(), record.idempotency_key.as_str()),
                record,
            )
        })
        .collect::<BTreeMap<_, _>>();
    normalize_idempotency_records(&mut idempotency_index);
    let incident_objects = normalize_incident_objects(document.incident_objects);
    let outcome_registry_entries =
        normalize_outcome_registry_entries(document.outcome_registry_entries);
    let mut incident_taxonomy_registry = default_incident_taxonomy_registry();
    for entry in document.incident_taxonomy_registry {
        upsert_incident_taxonomy_entry(&mut incident_taxonomy_registry, entry)?;
    }
    normalize_incident_taxonomy_registry(&mut incident_taxonomy_registry);
    Ok(LoadedReceiptState {
        receipts: normalize_receipts(document.receipts),
        work_units,
        idempotency_index,
        incident_objects,
        outcome_registry_entries,
        incident_taxonomy_registry,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_ollama_provenance()
    -> crate::local_inference_runtime::LocalInferenceExecutionProvenance {
        crate::local_inference_runtime::LocalInferenceExecutionProvenance {
            backend: "ollama".to_string(),
            requested_model: Some("llama3.2:latest".to_string()),
            served_model: "llama3.2:latest".to_string(),
            normalized_prompt_digest: "sha256:prompt".to_string(),
            normalized_options_json: "{\"num_predict\":64,\"top_k\":16}".to_string(),
            normalized_options_digest: "sha256:options".to_string(),
            base_url: "http://127.0.0.1:11434".to_string(),
            total_duration_ns: Some(1_200_000),
            load_duration_ns: Some(0),
            prompt_token_count: Some(11),
            generated_token_count: Some(7),
            warm_start: Some(true),
        }
    }

    fn fixture_ingress_request() -> JobInboxNetworkRequest {
        JobInboxNetworkRequest {
            request_id: "req-123".to_string(),
            requester: "npub1abc".to_string(),
            demand_source: JobDemandSource::OpenNetwork,
            request_kind: 5000,
            capability: "text_generation".to_string(),
            execution_input: Some("Generate text for req-123".to_string()),
            execution_prompt: Some("Generate text for req-123".to_string()),
            execution_params: Vec::new(),
            requested_model: Some("llama3.2:latest".to_string()),
            requested_output_mime: Some("text/plain".to_string()),
            target_provider_pubkeys: vec!["npub1target".to_string()],
            encrypted: false,
            encrypted_payload: None,
            parsed_event_shape: Some("shape".to_string()),
            raw_event_json: Some("{\"kind\":5000}".to_string()),
            skill_scope_id: Some("skill.scope".to_string()),
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: None,
            sa_tick_result_event_id: None,
            ac_envelope_event_id: Some("ac-env-1".to_string()),
            price_sats: 42,
            ttl_seconds: 120,
            validation: crate::state::job_inbox::JobInboxValidation::Valid,
        }
    }

    fn fixture_history_row(payment_pointer: &str) -> JobHistoryReceiptRow {
        JobHistoryReceiptRow {
            job_id: "job-req-123".to_string(),
            status: JobHistoryStatus::Succeeded,
            demand_source: JobDemandSource::OpenNetwork,
            completed_at_epoch_seconds: 1_762_000_000,
            skill_scope_id: Some("skill.scope".to_string()),
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_result_event_id: Some("result-evt".to_string()),
            sa_trajectory_session_id: Some("traj:123".to_string()),
            ac_envelope_event_id: Some("ac-env-1".to_string()),
            ac_settlement_event_id: Some("fb-evt".to_string()),
            ac_default_event_id: None,
            delivery_proof_id: Some("delivery.req-123".to_string()),
            delivery_metering_rule_id: Some("meter.ollama.inference.v1".to_string()),
            delivery_proof_status_label: Some("accepted".to_string()),
            delivery_metered_quantity: Some(1),
            delivery_accepted_quantity: Some(1),
            delivery_variance_reason_label: None,
            delivery_rejection_reason_label: None,
            payout_sats: 42,
            result_hash: "sha256:abc".to_string(),
            payment_pointer: payment_pointer.to_string(),
            failure_reason: None,
            execution_provenance: Some(fixture_ollama_provenance()),
        }
    }

    fn fixture_active_job(payment_pointer: &str) -> ActiveJobRecord {
        ActiveJobRecord {
            job_id: "job-req-123".to_string(),
            request_id: "req-123".to_string(),
            requester: "npub1abc".to_string(),
            demand_source: JobDemandSource::OpenNetwork,
            request_kind: 5000,
            capability: "text_generation".to_string(),
            execution_input: Some("Generate text for req-123".to_string()),
            execution_prompt: Some("Generate text for req-123".to_string()),
            execution_params: Vec::new(),
            requested_model: Some("llama3.2:latest".to_string()),
            execution_provenance: Some(fixture_ollama_provenance()),
            skill_scope_id: Some("skill.scope".to_string()),
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: Some("request-evt".to_string()),
            sa_tick_result_event_id: Some("result-evt".to_string()),
            sa_trajectory_session_id: Some("traj:123".to_string()),
            ac_envelope_event_id: Some("ac-env-1".to_string()),
            ac_settlement_event_id: Some("fb-evt".to_string()),
            ac_default_event_id: None,
            compute_product_id: Some("ollama.text_generation".to_string()),
            capacity_lot_id: Some(
                "lot.online.npub1abc.ollama.text_generation.1762000000000".to_string(),
            ),
            capacity_instrument_id: Some("instrument.req-123".to_string()),
            delivery_proof_id: Some("delivery.req-123".to_string()),
            delivery_metering_rule_id: Some("meter.ollama.inference.v1".to_string()),
            delivery_proof_status_label: Some("accepted".to_string()),
            delivery_metered_quantity: Some(1),
            delivery_accepted_quantity: Some(1),
            delivery_variance_reason_label: None,
            delivery_rejection_reason_label: None,
            quoted_price_sats: 42,
            ttl_seconds: 120,
            stage: JobLifecycleStage::Paid,
            invoice_id: None,
            payment_id: Some(payment_pointer.to_string()),
            failure_reason: None,
            events: Vec::new(),
        }
    }

    fn fixture_certification_draft(
        certification_id: &str,
        idempotency_key: &str,
    ) -> SafetyCertificationDraft {
        SafetyCertificationDraft {
            certification_id: certification_id.to_string(),
            idempotency_key: idempotency_key.to_string(),
            certification_level: "level_2".to_string(),
            scope: vec![CertificationScope {
                category: "compute".to_string(),
                tfb_class: FeedbackLatencyClass::Short,
                min_severity: SeverityClass::High,
                max_severity: SeverityClass::Critical,
            }],
            valid_from_ms: 1_762_000_000_000,
            valid_until_ms: 1_762_010_000_000,
            issuer_identity: "npub1auditorpersonhood".to_string(),
            issuer_auth_assurance_level: Some(AuthAssuranceLevel::Personhood),
            required_evidence: vec![
                EvidenceRef::new(
                    "audit_attestation_ref",
                    "oa://audits/high-severity/attestation",
                    digest_for_text("audit_attestation_ref"),
                ),
                EvidenceRef::new(
                    "incident_history_summary",
                    "oa://economy/incidents/summary/compute/high",
                    digest_for_text("incident_history_summary"),
                ),
            ],
            linked_receipt_ids: vec![],
        }
    }

    fn fixture_drift_signals(alert: bool) -> Vec<DriftSignalSummary> {
        vec![DriftSignalSummary {
            detector_id: "detector.drift.sv_floor".to_string(),
            signal_code: "sv_below_floor".to_string(),
            count_24h: if alert { 3 } else { 0 },
            ratio: if alert { 0.55 } else { 0.95 },
            threshold: 0.70,
            score: if alert { 0.2142857142857142 } else { 0.0 },
            alert,
        }]
    }

    fn fixture_drift_alert_receipt(
        receipt_id: &str,
        linked_work_unit_id: &str,
        created_at_ms: i64,
    ) -> Receipt {
        let mut drift_detector = EvidenceRef::new(
            "drift_detector_ref",
            "oa://economy/drift/detectors/detector.drift.sv_floor",
            digest_for_text("detector.drift.sv_floor"),
        );
        drift_detector
            .meta
            .insert("detector_id".to_string(), json!("detector.drift.sv_floor"));
        drift_detector
            .meta
            .insert("signal_code".to_string(), json!("sv_below_floor"));
        let mut drift_summary = EvidenceRef::new(
            "drift_signal_summary",
            "oa://economy/drift/snapshots/snapshot.economy.1762000060000/detector.drift.sv_floor",
            digest_for_text("drift-summary-sv-below-floor"),
        );
        drift_summary
            .meta
            .insert("signal_code".to_string(), json!("sv_below_floor"));
        drift_summary.meta.insert("score".to_string(), json!(0.91));
        drift_summary.meta.insert("alert".to_string(), json!(true));

        ReceiptBuilder::new(
            receipt_id.to_string(),
            "economy.drift.alert_raised.v1".to_string(),
            created_at_ms,
            format!("idemp:test:{receipt_id}"),
            TraceContext {
                work_unit_id: Some(linked_work_unit_id.to_string()),
                ..TraceContext::default()
            },
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "snapshot_id": "snapshot.economy:1762000060000",
            "detector_id": "detector.drift.sv_floor",
        }))
        .with_outputs_payload(json!({
            "status": "alert_raised",
            "signal_code": "sv_below_floor",
        }))
        .with_evidence(vec![drift_detector, drift_summary])
        .with_hints(ReceiptHints {
            category: Some("compute".to_string()),
            tfb_class: Some(FeedbackLatencyClass::Short),
            severity: Some(SeverityClass::High),
            reason_code: Some(REASON_CODE_DRIFT_ALERT_RAISED.to_string()),
            auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
            personhood_proved: Some(false),
            ..ReceiptHints::default()
        })
        .build()
        .expect("drift alert fixture receipt should build")
    }

    fn fixture_incident_draft(
        linked_receipt_id: &str,
        idempotency_key: &str,
    ) -> IncidentReportDraft {
        IncidentReportDraft {
            idempotency_key: idempotency_key.to_string(),
            incident_kind: IncidentKind::Incident,
            taxonomy_id: "oa.incident.taxonomy".to_string(),
            taxonomy_version: "2026.02".to_string(),
            taxonomy_code: "ops.execution_failure".to_string(),
            severity: SeverityClass::High,
            summary: "provider execution failed after acceptance".to_string(),
            linked_receipt_ids: vec![linked_receipt_id.to_string()],
            rollback_receipt_ids: vec![],
            evidence_digests: vec!["sha256:evidence-1".to_string()],
        }
    }

    fn fixture_ground_truth_case_draft(
        linked_receipt_id: &str,
        idempotency_key: &str,
    ) -> IncidentReportDraft {
        let mut draft = fixture_incident_draft(linked_receipt_id, idempotency_key);
        draft.incident_kind = IncidentKind::GroundTruthCase;
        draft.taxonomy_code = "safety.ground_truth_case".to_string();
        draft.summary = "ground truth replay case for simulation export".to_string();
        draft
    }

    fn fixture_anchor_publication_draft(
        snapshot_id: &str,
        snapshot_hash: &str,
        backend: &str,
        external_anchor_reference: &str,
    ) -> AnchorPublicationDraft {
        AnchorPublicationDraft {
            snapshot_id: snapshot_id.to_string(),
            snapshot_hash: snapshot_hash.to_string(),
            anchor_backend: backend.to_string(),
            external_anchor_reference: external_anchor_reference.to_string(),
            receipt_root_hash: Some("sha256:receipt-root-proof".to_string()),
        }
    }

    fn fixture_outcome_registry_draft(
        linked_receipt_id: &str,
        idempotency_key: &str,
    ) -> OutcomeRegistryEntryDraft {
        OutcomeRegistryEntryDraft {
            idempotency_key: idempotency_key.to_string(),
            category: "compute".to_string(),
            tfb_class: FeedbackLatencyClass::Short,
            severity: SeverityClass::High,
            verdict_outcome: "verified".to_string(),
            settlement_outcome: "settled".to_string(),
            claim_outcome: Some("none".to_string()),
            remedy_outcome: Some("none".to_string()),
            incident_tags: vec!["ops.execution_failure".to_string()],
            linked_receipt_ids: vec![linked_receipt_id.to_string()],
            evidence_digests: vec!["sha256:outcome-evidence-1".to_string()],
        }
    }

    fn fixture_white_hat_template_draft(
        work_unit_id: &str,
        idempotency_key: &str,
        kind: WhiteHatWorkUnitKind,
    ) -> WhiteHatWorkUnitTemplateDraft {
        WhiteHatWorkUnitTemplateDraft {
            work_unit_id: work_unit_id.to_string(),
            idempotency_key: idempotency_key.to_string(),
            kind,
            tfb_class: FeedbackLatencyClass::Long,
            severity: SeverityClass::High,
            acceptance_criteria_ref: format!("oa://audits/{work_unit_id}/acceptance"),
            coordinated_disclosure_ref: format!("oa://audits/{work_unit_id}/disclosure"),
            mandatory_provenance: true,
            verification_budget_hint_sats: Some(2_000),
        }
    }

    fn fixture_white_hat_bounty_settlement_draft(
        work_unit_id: &str,
        idempotency_key: &str,
        verdict_receipt_id: &str,
    ) -> WhiteHatBountySettlementDraft {
        WhiteHatBountySettlementDraft {
            work_unit_id: work_unit_id.to_string(),
            idempotency_key: idempotency_key.to_string(),
            finding_id: format!("finding:{work_unit_id}:critical-path"),
            verdict_receipt_id: verdict_receipt_id.to_string(),
            payout_sats: 1_500,
            dispute_bond_sats: 250,
            disputed: false,
            dispute_reason: None,
            escrow_receipt_id: None,
            incident_report: None,
        }
    }

    fn append_synthetic_verdict_receipt(
        state: &mut EarnKernelReceiptState,
        receipt_id: &str,
        work_unit_id: &str,
        created_at_ms: i64,
    ) {
        let receipt = ReceiptBuilder::new(
            receipt_id.to_string(),
            "economy.verdict.finalized.v1".to_string(),
            created_at_ms,
            format!("idemp.synthetic.verdict:{receipt_id}"),
            TraceContext {
                work_unit_id: Some(work_unit_id.to_string()),
                ..TraceContext::default()
            },
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "work_unit_id": work_unit_id,
            "verdict": "finding_verified",
        }))
        .with_outputs_payload(json!({
            "status": "finalized",
        }))
        .with_evidence(vec![EvidenceRef::new(
            "verdict_evidence_ref",
            format!("oa://verdicts/{receipt_id}"),
            digest_for_text(receipt_id),
        )])
        .with_hints(ReceiptHints {
            category: Some("audit".to_string()),
            tfb_class: Some(FeedbackLatencyClass::Long),
            severity: Some(SeverityClass::High),
            achieved_verification_tier: Some(VerificationTier::Tier2Heterogeneous),
            verification_correlated: Some(false),
            provenance_grade: Some(ProvenanceGrade::P2Lineage),
            auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
            personhood_proved: Some(false),
            reason_code: None,
            notional: None,
            liability_premium: None,
        })
        .build();
        state.append_receipt(receipt, "test.synthetic.verdict");
        assert_eq!(state.load_state, PaneLoadState::Ready);
    }

    fn append_synthetic_linked_receipt(
        state: &mut EarnKernelReceiptState,
        receipt_id: &str,
        receipt_type: &str,
        created_at_ms: i64,
        linked_receipt_ids: &[&str],
    ) {
        let mut evidence = linked_receipt_ids
            .iter()
            .map(|receipt_ref_id| {
                let linked = state
                    .get_receipt(receipt_ref_id)
                    .expect("linked receipt should exist for synthetic fixture");
                let mut meta = std::collections::BTreeMap::new();
                meta.insert(
                    "receipt_type".to_string(),
                    serde_json::Value::String(linked.receipt_type.clone()),
                );
                EvidenceRef {
                    kind: "receipt_ref".to_string(),
                    uri: format!("oa://receipts/{}", linked.receipt_id),
                    digest: linked.canonical_hash.clone(),
                    meta,
                }
            })
            .collect::<Vec<_>>();
        evidence.push(EvidenceRef::new(
            "synthetic_evidence",
            format!("oa://evidence/{receipt_id}"),
            digest_for_text(receipt_type),
        ));

        state.append_receipt(
            ReceiptBuilder::new(
                receipt_id.to_string(),
                receipt_type.to_string(),
                created_at_ms,
                format!("idemp.synthetic:{receipt_id}"),
                TraceContext {
                    work_unit_id: Some("job-req-123".to_string()),
                    ..TraceContext::default()
                },
                current_policy_context(),
            )
            .with_inputs_payload(json!({
                "fixture": "synthetic_linked_receipt",
                "receipt_id": receipt_id,
            }))
            .with_outputs_payload(json!({
                "receipt_type": receipt_type,
            }))
            .with_evidence(evidence)
            .with_hints(ReceiptHints {
                category: Some("compute".to_string()),
                tfb_class: Some(FeedbackLatencyClass::Short),
                severity: Some(SeverityClass::High),
                ..ReceiptHints::default()
            })
            .build(),
            "test.synthetic",
        );
    }

    #[test]
    fn paid_history_receipt_without_wallet_proof_is_withheld() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);

        state.record_history_receipt(
            &fixture_history_row("pending:abc"),
            1_762_000_010,
            "test.history",
        );

        assert_eq!(state.load_state, PaneLoadState::Ready);
        let withheld = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.withheld.v1")
            .expect("withheld receipt");
        assert_eq!(
            withheld.hints.reason_code.as_deref(),
            Some(REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE)
        );
        assert!(
            withheld
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "policy_decision")
        );
    }

    #[test]
    fn ingress_receipt_and_history_settlement_receipt_are_emitted() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);

        let ingress = fixture_ingress_request();
        state.record_ingress_request(&ingress, 1_762_000_000, "test.ingress");
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-1"),
            1_762_000_010,
            "test.history",
        );

        assert_eq!(state.load_state, PaneLoadState::Ready);
        assert_eq!(state.receipts.len(), 2);
        assert!(
            state
                .receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "earn.job.ingress_request.v1")
        );
        let settlement = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt");
        assert!(
            settlement
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "wallet_settlement_proof")
        );
        assert!(
            settlement
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "pricing_snapshot_ref")
        );
        assert!(
            settlement
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "risk_pricing_rule_ref")
        );
        assert_eq!(
            settlement.hints.tfb_class,
            Some(FeedbackLatencyClass::Short)
        );
        assert_eq!(settlement.hints.severity, Some(SeverityClass::Low));
        assert_eq!(
            settlement.hints.provenance_grade,
            Some(ProvenanceGrade::P3Attested)
        );
        assert!(settlement.hints.liability_premium.is_some());
        let work_unit = state
            .work_units
            .get("job-req-123")
            .expect("work-unit metadata");
        assert_eq!(work_unit.category, "compute");
        assert_eq!(work_unit.tfb_class, FeedbackLatencyClass::Short);
        assert_eq!(work_unit.severity, SeverityClass::Low);
        assert_eq!(work_unit.verification_budget_hint_sats, 100);
    }

    #[test]
    fn work_unit_metadata_persists_across_restarts() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path.clone());
        let ingress = fixture_ingress_request();
        state.record_ingress_request(&ingress, 1_762_000_000, "test.ingress");

        let reloaded = EarnKernelReceiptState::from_receipt_file_path(state_path);
        let work_unit = reloaded
            .work_units
            .get("job-req-123")
            .expect("work-unit metadata");
        assert_eq!(work_unit.category, "compute");
        assert_eq!(work_unit.tfb_class, FeedbackLatencyClass::Short);
        assert_eq!(work_unit.severity, SeverityClass::Low);
        assert_eq!(work_unit.verification_budget_hint_sats, 100);
    }

    #[test]
    fn receipt_lookup_by_id_survives_restart() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path.clone());
        let ingress = fixture_ingress_request();
        state.record_ingress_request(&ingress, 1_762_000_000, "test.ingress");

        let receipt_id = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.ingress_request.v1")
            .expect("ingress receipt")
            .receipt_id
            .clone();
        let reloaded = EarnKernelReceiptState::from_receipt_file_path(state_path);
        assert!(reloaded.get_receipt(receipt_id.as_str()).is_some());
    }

    #[test]
    fn correction_receipt_supersedes_prior_receipts_without_mutation() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);

        let ingress = fixture_ingress_request();
        state.record_ingress_request(&ingress, 1_762_000_000, "test.ingress");
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-1"),
            1_762_000_010,
            "test.history",
        );
        let original_receipt_id = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt")
            .receipt_id
            .clone();
        let prior_count = state.receipts.len();

        let correction_receipt_id = state
            .record_correction_receipt(
                &[original_receipt_id.clone()],
                "wallet proof reclassified after reconciliation",
                1_762_000_020_000,
                "test.correction",
            )
            .expect("correction receipt");

        assert_eq!(state.receipts.len(), prior_count + 1);
        let correction = state
            .get_receipt(correction_receipt_id.as_str())
            .expect("correction receipt exists");
        assert_eq!(correction.receipt_type, "earn.receipt.correction.v1");
        assert!(correction.evidence.iter().any(|evidence| {
            evidence.kind == "receipt_ref"
                && evidence.uri == format!("oa://receipts/{original_receipt_id}")
        }));
        assert!(state.get_receipt(original_receipt_id.as_str()).is_some());
    }

    #[test]
    fn export_receipt_bundle_is_deterministic() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let bundle_path = temp_dir.path().join("bundle.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);

        let ingress = fixture_ingress_request();
        state.record_ingress_request(&ingress, 1_762_000_000, "test.ingress");
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-1"),
            1_762_000_010,
            "test.history",
        );

        let query = ReceiptQuery {
            start_inclusive_ms: Some(1_762_000_000_000),
            end_inclusive_ms: Some(1_762_000_020_000),
            work_unit_id: Some("job-req-123".to_string()),
            receipt_type: None,
        };
        let first = state
            .export_receipt_bundle_to_path(&query, 1_762_000_030_000, bundle_path.as_path())
            .expect("bundle export");
        let second = state
            .export_receipt_bundle(&query, 1_762_000_030_000)
            .expect("bundle export");

        assert_eq!(first.bundle_hash, second.bundle_hash);
        assert_eq!(first.receipt_ids, second.receipt_ids);
        assert_eq!(first.receipt_count, second.receipt_count);
        assert!(bundle_path.exists());
    }

    #[test]
    fn export_audit_package_is_deterministic_and_preserves_linkage_invariants() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);

        let contract_receipt_id = "receipt.contract.synthetic";
        let verdict_receipt_id = "receipt.verdict.synthetic";
        let settlement_receipt_id = "receipt.settlement.synthetic";
        let certification_receipt_id = "receipt.certification.synthetic";
        let outcome_receipt_id = "receipt.outcome.synthetic";

        append_synthetic_linked_receipt(
            &mut state,
            contract_receipt_id,
            "economy.contract.created.v1",
            1_762_000_000_000,
            &[],
        );
        append_synthetic_linked_receipt(
            &mut state,
            verdict_receipt_id,
            "economy.verdict.finalized.v1",
            1_762_000_010_000,
            &[contract_receipt_id],
        );
        append_synthetic_linked_receipt(
            &mut state,
            settlement_receipt_id,
            "economy.settlement.finalized.v1",
            1_762_000_020_000,
            &[verdict_receipt_id],
        );
        let incident_id = state
            .report_incident(
                fixture_incident_draft(settlement_receipt_id, "incident-audit-linkage"),
                1_762_000_030_000,
                "test.incident.audit.linkage",
            )
            .expect("incident report should succeed");
        append_synthetic_linked_receipt(
            &mut state,
            certification_receipt_id,
            "economy.certification.issued.v1",
            1_762_000_040_000,
            &[settlement_receipt_id],
        );
        append_synthetic_linked_receipt(
            &mut state,
            outcome_receipt_id,
            "economy.outcome_registry.recorded.v1",
            1_762_000_050_000,
            &[settlement_receipt_id],
        );

        let query = ReceiptQuery::default();
        let first = state
            .export_audit_package(
                &query,
                AuditExportRedactionTier::Restricted,
                1_762_000_090_000,
            )
            .expect("first audit package export");
        let second = state
            .export_audit_package(
                &query,
                AuditExportRedactionTier::Restricted,
                1_762_000_120_000,
            )
            .expect("second audit package export");

        assert_eq!(first.schema_version, EARN_KERNEL_RECEIPT_SCHEMA_VERSION);
        assert_eq!(first.package_hash, second.package_hash);
        assert_ne!(first.generated_at_ms, second.generated_at_ms);
        assert_eq!(first.redaction_tier, AuditExportRedactionTier::Restricted);
        assert_eq!(first.certification_count, 1);
        assert_eq!(first.outcome_registry_count, 1);
        assert!(
            first
                .certifications
                .iter()
                .any(|entry| entry.receipt_id == certification_receipt_id)
        );
        assert!(
            first
                .outcome_registry_entries
                .iter()
                .any(|entry| entry.receipt_id == outcome_receipt_id)
        );
        assert!(first.linkage_edges.iter().any(|edge| {
            edge.from_receipt_id == verdict_receipt_id
                && edge.to_receipt_id == contract_receipt_id
                && edge.relation_kind == "receipt_ref"
        }));
        assert!(first.linkage_edges.iter().any(|edge| {
            edge.from_receipt_id == settlement_receipt_id
                && edge.to_receipt_id == verdict_receipt_id
                && edge.relation_kind == "receipt_ref"
        }));
        assert!(first.linkage_edges.iter().any(|edge| {
            edge.from_receipt_id == format!("incident:{incident_id}")
                && edge.to_receipt_id == settlement_receipt_id
                && edge.relation_kind == "incident_linked_receipt"
        }));
    }

    #[test]
    fn export_audit_package_public_tier_redacts_sensitive_fields() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-public-redaction"),
            1_762_000_010,
            "test.history",
        );
        let linked_receipt_id = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt")
            .receipt_id
            .clone();
        state
            .report_incident(
                fixture_incident_draft(linked_receipt_id.as_str(), "incident-audit-redaction"),
                1_762_000_060_000,
                "test.incident.report",
            )
            .expect("incident report");

        let query = ReceiptQuery::default();
        let restricted = state
            .export_audit_package(
                &query,
                AuditExportRedactionTier::Restricted,
                1_762_000_090_000,
            )
            .expect("restricted audit package export");
        let public = state
            .export_audit_package(&query, AuditExportRedactionTier::Public, 1_762_000_090_000)
            .expect("public audit package export");

        assert_ne!(restricted.package_hash, public.package_hash);
        assert!(
            restricted
                .receipts
                .iter()
                .flat_map(|receipt| receipt.evidence.iter())
                .filter(|evidence| evidence.kind == "wallet_settlement_proof")
                .all(|evidence| evidence.uri != "oa://redacted")
        );
        assert!(
            public
                .receipts
                .iter()
                .flat_map(|receipt| receipt.evidence.iter())
                .filter(|evidence| evidence.kind == "wallet_settlement_proof")
                .all(|evidence| evidence.uri == "oa://redacted")
        );
        assert!(
            restricted
                .incidents
                .iter()
                .all(|incident| incident.summary != "[redacted]")
        );
        assert!(public.incidents.iter().all(|incident| {
            incident.summary == "[redacted]"
                && incident.linked_receipt_ids.is_empty()
                && incident.rollback_receipt_ids.is_empty()
        }));
    }

    #[test]
    fn audit_package_exports_certification_objects_and_redacts_issuer_digest() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-cert-audit"),
            1_762_000_010,
            "test.history",
        );
        let settlement_receipt_id = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt")
            .receipt_id
            .clone();
        let mut draft = fixture_certification_draft("cert.audit.export", "issue-cert-audit-export");
        draft.linked_receipt_ids = vec![settlement_receipt_id.clone()];
        state
            .issue_safety_certification(draft, 1_762_000_020_000, "test.cert.issue")
            .expect("certification issued");

        let restricted = state
            .export_audit_package(
                &ReceiptQuery::default(),
                AuditExportRedactionTier::Restricted,
                1_762_000_090_000,
            )
            .expect("restricted audit package");
        let public = state
            .export_audit_package(
                &ReceiptQuery::default(),
                AuditExportRedactionTier::Public,
                1_762_000_090_000,
            )
            .expect("public audit package");

        assert_eq!(restricted.certification_object_count, 1);
        assert_eq!(public.certification_object_count, 1);
        let restricted_object = restricted
            .certification_objects
            .iter()
            .find(|object| object.certification_id == "cert.audit.export")
            .expect("restricted certification object");
        let public_object = public
            .certification_objects
            .iter()
            .find(|object| object.certification_id == "cert.audit.export")
            .expect("public certification object");
        assert_ne!(
            restricted_object.issuer_credential_digest,
            digest_for_text("redacted")
        );
        assert_eq!(
            public_object.issuer_credential_digest,
            digest_for_text("redacted")
        );
        assert!(public_object.linked_receipt_ids.is_empty());
    }

    #[test]
    fn publish_result_replay_conflict_returns_idempotency_conflict() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        let baseline = fixture_active_job("wallet-payment-1");
        state.record_active_job_stage(
            &baseline,
            JobLifecycleStage::Delivered,
            1_762_000_010,
            "test.delivered",
        );
        let receipt_count = state.receipts.len();

        let mut conflicting = baseline.clone();
        conflicting.capability = "different-capability".to_string();
        state.record_active_job_stage(
            &conflicting,
            JobLifecycleStage::Delivered,
            1_762_000_011,
            "test.delivered.conflict",
        );

        assert_eq!(state.receipts.len(), receipt_count);
        assert!(state.last_error.as_deref().is_some_and(|error| {
            error.contains(REASON_CODE_IDEMPOTENCY_CONFLICT)
                && error.contains("earn.job.result_published.v1")
        }));
    }

    #[test]
    fn wallet_withdraw_send_replay_and_conflict_follow_idempotency_contract() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        let payment_request = "lnbc1exampleinvoice";
        let caller = "npub1walletuser";
        let first_receipt_id = state
            .record_wallet_withdraw_send_attempt(
                caller,
                payment_request,
                Some(1_000),
                1_762_000_010_000,
                "test.wallet.send",
            )
            .expect("first wallet send receipt");
        let receipt_count = state.receipts.len();
        let reloaded_state_path = state.receipt_file_path.clone();
        drop(state);
        let mut state = EarnKernelReceiptState::from_receipt_file_path(reloaded_state_path);
        assert_eq!(state.receipts.len(), receipt_count);

        let replay_receipt_id = state
            .record_wallet_withdraw_send_attempt(
                caller,
                payment_request,
                Some(1_000),
                1_762_000_010_200,
                "test.wallet.send.replay",
            )
            .expect("idempotent replay");
        assert_eq!(replay_receipt_id, first_receipt_id);
        assert_eq!(state.receipts.len(), receipt_count);

        let conflict = state.record_wallet_withdraw_send_attempt(
            caller,
            payment_request,
            Some(2_000),
            1_762_000_010_400,
            "test.wallet.send.conflict",
        );
        assert!(conflict.is_err());
        assert_eq!(state.receipts.len(), receipt_count);
        assert!(state.last_error.as_deref().is_some_and(|error| {
            error.contains(REASON_CODE_IDEMPOTENCY_CONFLICT)
                && error.contains("earn.wallet.withdraw_submitted.v1")
        }));
    }

    #[test]
    fn wallet_withdraw_with_insufficient_auth_is_withheld_and_idempotent() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        let policy_bundle = PolicyBundleConfig {
            authentication_rules: vec![AuthenticationPolicyRule {
                rule_id: "policy.test.auth.personhood_required.v1".to_string(),
                slice: PolicySliceRule {
                    category: Some("compute".to_string()),
                    tfb_class: Some(FeedbackLatencyClass::Instant),
                    severity: Some(SeverityClass::Critical),
                },
                role: Some("operator".to_string()),
                min_auth_assurance: Some("personhood".to_string()),
                require_personhood: true,
            }],
            ..PolicyBundleConfig::default()
        };

        let first = state.record_wallet_withdraw_send_attempt_with_policy(
            "npub1walletuser",
            "lnbc1restrictedinvoice",
            Some(250_000),
            1_762_000_010_000,
            "test.wallet.auth",
            &policy_bundle,
        );
        assert!(first.is_err());
        assert!(
            first
                .err()
                .is_some_and(|error| error.contains(REASON_CODE_AUTH_ASSURANCE_INSUFFICIENT))
        );
        let first_count = state.receipts.len();

        let second = state.record_wallet_withdraw_send_attempt_with_policy(
            "npub1walletuser",
            "lnbc1restrictedinvoice",
            Some(250_000),
            1_762_000_010_500,
            "test.wallet.auth.replay",
            &policy_bundle,
        );
        assert!(second.is_err());
        assert_eq!(state.receipts.len(), first_count);

        let withheld = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.wallet.withdraw_withheld.v1")
            .expect("withheld auth receipt should exist");
        assert_eq!(
            withheld.hints.reason_code.as_deref(),
            Some(REASON_CODE_AUTH_ASSURANCE_INSUFFICIENT)
        );
        assert_eq!(
            withheld.hints.auth_assurance_level,
            Some(AuthAssuranceLevel::Authenticated)
        );
        assert_eq!(withheld.hints.personhood_proved, Some(false));
        assert!(
            withheld
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "policy_decision")
        );
        assert!(
            withheld
                .evidence
                .iter()
                .any(|evidence| evidence.kind.starts_with("credential_ref_"))
        );
    }

    #[test]
    fn high_severity_settlement_requires_rollback_terms() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        let mut high_severity_job = fixture_active_job("wallet-payment-high-severity");
        high_severity_job.quoted_price_sats = 10_000;
        state
            .issue_safety_certification(
                SafetyCertificationDraft {
                    certification_id: "cert.high-severity-compute".to_string(),
                    idempotency_key: "issue-cert-high-severity-rollback-test".to_string(),
                    certification_level: "level_2".to_string(),
                    scope: vec![CertificationScope {
                        category: "compute".to_string(),
                        tfb_class: FeedbackLatencyClass::Short,
                        min_severity: SeverityClass::High,
                        max_severity: SeverityClass::Critical,
                    }],
                    valid_from_ms: 1_762_000_000_000,
                    valid_until_ms: 1_762_010_000_000,
                    issuer_identity: "npub1auditorpersonhood".to_string(),
                    issuer_auth_assurance_level: Some(AuthAssuranceLevel::Personhood),
                    required_evidence: vec![EvidenceRef::new(
                        "audit_attestation_ref",
                        "oa://audits/cert.high-severity-compute",
                        digest_for_text("audit:cert.high-severity-compute"),
                    )],
                    linked_receipt_ids: vec![],
                },
                1_762_000_000_500,
                "test.certification.issue",
            )
            .expect("certification issued");

        state.record_active_job_stage(
            &high_severity_job,
            JobLifecycleStage::Paid,
            1_762_000_010,
            "test.rollback_gate.without_terms",
        );
        let withheld = state
            .receipts
            .iter()
            .find(|receipt| {
                receipt.receipt_type == "earn.job.withheld.v1"
                    && receipt.hints.reason_code.as_deref()
                        == Some(REASON_CODE_ROLLBACK_PLAN_REQUIRED)
            })
            .expect("rollback withhold receipt");
        assert!(
            withheld
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "policy_decision")
        );
        assert!(
            !withheld
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "rollback_plan_ref")
        );

        state
            .set_work_unit_rollback_terms(
                high_severity_job.job_id.as_str(),
                Some("oa://rollback/plans/high-severity"),
                None,
            )
            .expect("rollback terms set");
        state.record_active_job_stage(
            &high_severity_job,
            JobLifecycleStage::Paid,
            1_762_000_011,
            "test.rollback_gate.with_terms",
        );

        let settled = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt");
        assert!(
            settled
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "rollback_plan_ref")
        );
    }

    #[test]
    fn safety_certification_issue_and_revoke_emit_append_only_receipts() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);

        let issue_receipt_id = state
            .issue_safety_certification(
                fixture_certification_draft(
                    "cert.issue-revoke.append-only",
                    "issue-cert-issue-revoke-1",
                ),
                1_762_000_100_000,
                "test.certification.issue",
            )
            .expect("certification issue receipt");
        let issue_replay_receipt_id = state
            .issue_safety_certification(
                fixture_certification_draft(
                    "cert.issue-revoke.append-only",
                    "issue-cert-issue-revoke-1",
                ),
                1_762_000_100_500,
                "test.certification.issue.replay",
            )
            .expect("certification issue replay");
        assert_eq!(issue_receipt_id, issue_replay_receipt_id);
        assert_eq!(
            state
                .receipts
                .iter()
                .filter(|receipt| receipt.receipt_type == "economy.certification.issued.v1")
                .count(),
            1
        );

        let revoke_receipt_id = state
            .revoke_safety_certification(
                SafetyCertificationRevocationDraft {
                    certification_id: "cert.issue-revoke.append-only".to_string(),
                    idempotency_key: "revoke-cert-issue-revoke-1".to_string(),
                    reason_code: "issuer_revoked".to_string(),
                    evidence: vec![EvidenceRef::new(
                        "revocation_notice",
                        "oa://audits/revocations/cert.issue-revoke.append-only",
                        digest_for_text("revocation_notice"),
                    )],
                },
                1_762_000_101_000,
                "test.certification.revoke",
            )
            .expect("certification revoke receipt");
        let revoked = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_id == revoke_receipt_id)
            .expect("revoke receipt should exist");
        assert_eq!(revoked.receipt_type, "economy.certification.revoked.v1");

        let latest =
            latest_certification_objects_as_of(state.receipts.as_slice(), 1_762_000_102_000)
                .into_iter()
                .find(|certification| {
                    certification.certification_id == "cert.issue-revoke.append-only"
                })
                .expect("latest certification state");
        assert_eq!(latest.state, CertificationState::Revoked);
        assert_eq!(latest.revision, 2);
        assert_eq!(
            latest.revoked_reason_code.as_deref(),
            Some("ISSUER_REVOKED")
        );
    }

    #[test]
    fn high_severity_settlement_without_certification_is_border_blocked() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        let mut high_severity_job = fixture_active_job("wallet-payment-high-severity-no-cert");
        high_severity_job.quoted_price_sats = 10_000;
        state.record_active_job_stage(
            &high_severity_job,
            JobLifecycleStage::Accepted,
            1_762_000_109,
            "test.accepted",
        );

        state
            .set_work_unit_rollback_terms(
                high_severity_job.job_id.as_str(),
                Some("oa://rollback/plans/high-severity"),
                None,
            )
            .expect("rollback terms set");
        state.record_active_job_stage(
            &high_severity_job,
            JobLifecycleStage::Paid,
            1_762_000_110,
            "test.certification.withhold",
        );

        let withheld = state
            .receipts
            .iter()
            .find(|receipt| {
                receipt.receipt_type == "earn.job.withheld.v1"
                    && receipt.hints.reason_code.as_deref()
                        == Some(REASON_CODE_DIGITAL_BORDER_BLOCK_UNCERTIFIED)
            })
            .expect("withheld certification receipt");
        assert!(
            withheld
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "policy_decision")
        );
    }

    #[test]
    fn certified_high_severity_settlement_emits_safe_harbor_and_certification_refs() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        let mut high_severity_job = fixture_active_job("wallet-payment-high-severity-certified");
        high_severity_job.quoted_price_sats = 10_000;
        state.record_active_job_stage(
            &high_severity_job,
            JobLifecycleStage::Accepted,
            1_762_000_119,
            "test.accepted",
        );

        state
            .set_work_unit_rollback_terms(
                high_severity_job.job_id.as_str(),
                Some("oa://rollback/plans/high-severity"),
                None,
            )
            .expect("rollback terms set");
        state
            .issue_safety_certification(
                fixture_certification_draft(
                    "cert.high-severity.safe-harbor",
                    "issue-cert-safe-harbor-1",
                ),
                1_762_000_120_000,
                "test.certification.issue",
            )
            .expect("certification issue");

        state.record_active_job_stage(
            &high_severity_job,
            JobLifecycleStage::Paid,
            1_762_000_121,
            "test.certification.safe_harbor",
        );
        let settlement = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt");
        assert!(
            settlement
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "certification_ref")
        );
        assert!(
            settlement
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "safe_harbor_relaxation")
        );
        assert!(
            settlement
                .hints
                .liability_premium
                .as_ref()
                .is_some_and(|premium| match premium.amount {
                    MoneyAmount::AmountSats(value) => value > 0,
                    MoneyAmount::AmountMsats(value) => value > 0,
                })
        );
    }

    #[test]
    fn settlement_receipt_contains_receipt_refs_and_transitive_lineage() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);

        let ingress = fixture_ingress_request();
        state.record_ingress_request(&ingress, 1_762_000_000, "test.ingress");
        let job = fixture_active_job("wallet-payment-1");
        state.record_active_job_stage(
            &job,
            JobLifecycleStage::Accepted,
            1_762_000_001,
            "test.accepted",
        );
        state.record_active_job_stage(
            &job,
            JobLifecycleStage::Running,
            1_762_000_002,
            "test.running",
        );
        state.record_active_job_stage(
            &job,
            JobLifecycleStage::Delivered,
            1_762_000_003,
            "test.delivered",
        );
        state.record_active_job_stage(&job, JobLifecycleStage::Paid, 1_762_000_004, "test.paid");

        let settlement_receipt = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt");
        assert!(
            settlement_receipt
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "receipt_ref")
        );

        let lineage = state
            .settlement_lineage_receipt_ids(settlement_receipt.receipt_id.as_str())
            .expect("lineage should resolve");
        let ingress_receipt_id = lifecycle_receipt_id(
            job.job_id.as_str(),
            JobLifecycleStage::Received,
            job.request_id.as_str(),
        );
        assert!(lineage.contains(&ingress_receipt_id));
        assert!(lineage.contains(&settlement_receipt.receipt_id));
        assert!(lineage.len() >= 4);
    }

    #[test]
    fn settlement_receipt_carries_ollama_execution_tags_and_evidence() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        let job = fixture_active_job("wallet-payment-ollama");

        state.record_active_job_stage(&job, JobLifecycleStage::Paid, 1_762_000_050, "test.paid");

        let settlement_receipt = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt");
        assert_eq!(
            settlement_receipt
                .tags
                .get("execution.backend")
                .map(String::as_str),
            Some("ollama")
        );
        assert_eq!(
            settlement_receipt
                .tags
                .get("execution.model.served")
                .map(String::as_str),
            Some("llama3.2:latest")
        );
        assert_eq!(
            settlement_receipt
                .tags
                .get("execution.prompt_digest")
                .map(String::as_str),
            Some("sha256:prompt")
        );
        assert_eq!(
            settlement_receipt
                .tags
                .get("execution.options_digest")
                .map(String::as_str),
            Some("sha256:options")
        );
        assert_eq!(
            settlement_receipt
                .tags
                .get("execution.warm_start")
                .map(String::as_str),
            Some("true")
        );
        assert!(settlement_receipt.evidence.iter().any(|evidence| {
            evidence.kind == "execution_prompt_digest" && evidence.digest == "sha256:prompt"
        }));
        assert!(settlement_receipt.evidence.iter().any(|evidence| {
            evidence.kind == "execution_options_digest" && evidence.digest == "sha256:options"
        }));
        assert!(settlement_receipt.evidence.iter().any(|evidence| {
            evidence.kind == "execution_backend_ref"
                && evidence.uri == "oa://autopilot/jobs/job-req-123/execution/backend"
        }));
    }

    #[test]
    fn preflight_rejection_emits_coded_denial_receipt() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        let request = JobInboxRequest {
            request_id: "req-preflight".to_string(),
            requester: "npub1reject".to_string(),
            demand_source: JobDemandSource::OpenNetwork,
            request_kind: 5000,
            capability: "text_generation".to_string(),
            execution_input: Some("Rejectable execution payload".to_string()),
            execution_prompt: Some("Rejectable execution payload".to_string()),
            execution_params: Vec::new(),
            requested_model: Some("llama3.2:latest".to_string()),
            requested_output_mime: Some("text/plain".to_string()),
            target_provider_pubkeys: Vec::new(),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: None,
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats: 75,
            ttl_seconds: 60,
            validation: crate::state::job_inbox::JobInboxValidation::Valid,
            arrival_seq: 1,
            decision: crate::state::job_inbox::JobInboxDecision::Pending,
        };

        state.record_preflight_rejection(
            &request,
            "failed policy preflight",
            1_762_000_000,
            "test.reject",
        );

        let rejection = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.preflight_rejected.v1")
            .expect("preflight rejection receipt");
        assert_eq!(
            rejection.hints.reason_code.as_deref(),
            Some(REASON_CODE_POLICY_PREFLIGHT_REJECTED)
        );
        assert!(
            rejection
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "policy_decision")
        );
        assert_eq!(rejection.policy.policy_bundle_id, "policy.earn.default");
        assert_eq!(rejection.policy.policy_version, "1");
    }

    #[test]
    fn economy_snapshot_receipt_is_emitted_with_input_refs() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        let input = EvidenceRef::new(
            "receipt_window",
            "oa://receipts/window/1762000000000-1762000060000",
            "sha256:window",
        );

        state.record_economy_snapshot_receipt(
            "snapshot.economy:1762000060000",
            1_762_000_060_000,
            "sha256:snapshot",
            0.5,
            0.5,
            0.5,
            0.5,
            2,
            1.0,
            0.0,
            0.0,
            0.0,
            125,
            25,
            2_000,
            100,
            0.2,
            0.05,
            0,
            vec![],
            vec![],
            0,
            0,
            0.0,
            vec![],
            "sha256:audit-public".to_string(),
            "sha256:audit-restricted".to_string(),
            vec![input],
            "test.snapshot",
        );

        let receipt = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "economy.stats.snapshot_receipt.v1")
            .expect("snapshot receipt");
        assert!(
            receipt
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "receipt_window")
        );
        assert!(
            receipt
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "economy_snapshot_artifact")
        );
        let snapshot_metrics = receipt
            .evidence
            .iter()
            .find(|evidence| evidence.kind == "snapshot_metrics")
            .expect("snapshot metrics evidence");
        assert_eq!(
            snapshot_metrics
                .meta
                .get("liability_premiums_collected_24h_sats"),
            Some(&json!(125))
        );
        assert_eq!(snapshot_metrics.meta.get("sv_effective"), Some(&json!(0.5)));
        assert_eq!(
            snapshot_metrics.meta.get("rho_effective"),
            Some(&json!(0.5))
        );
        assert_eq!(
            snapshot_metrics.meta.get("claims_paid_24h_sats"),
            Some(&json!(25))
        );
        assert_eq!(
            snapshot_metrics.meta.get("drift_alerts_24h"),
            Some(&json!(0))
        );
        assert_eq!(
            snapshot_metrics.meta.get("top_drift_signals"),
            Some(&json!([]))
        );
        assert_eq!(
            snapshot_metrics.meta.get("rollback_attempts_24h"),
            Some(&json!(0))
        );
        assert_eq!(
            snapshot_metrics.meta.get("rollback_successes_24h"),
            Some(&json!(0))
        );
        assert_eq!(
            snapshot_metrics.meta.get("top_rollback_reason_codes"),
            Some(&json!([]))
        );
        assert_eq!(
            receipt.idempotency_key,
            "idemp.economy.snapshot:1762000060000"
        );
    }

    #[test]
    fn incident_reporting_is_idempotent_and_linkage_enforced() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-incident"),
            1_762_000_010,
            "test.history",
        );
        let linked_receipt_id = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt")
            .receipt_id
            .clone();

        let first = state
            .report_incident(
                fixture_incident_draft(linked_receipt_id.as_str(), "incident-alpha"),
                1_762_000_060_000,
                "test.incident",
            )
            .expect("incident report should succeed");
        let second = state
            .report_incident(
                fixture_incident_draft(linked_receipt_id.as_str(), "incident-alpha"),
                1_762_000_060_000,
                "test.incident.replay",
            )
            .expect("incident replay should be idempotent");
        assert_eq!(first, second);
        assert_eq!(state.incident_objects.len(), 1);
        assert_eq!(
            state
                .receipts
                .iter()
                .filter(|receipt| receipt.receipt_type == "economy.incident.reported.v1")
                .count(),
            1
        );

        let missing_linkage = state.report_incident(
            fixture_incident_draft("receipt.missing", "incident-missing"),
            1_762_000_061_000,
            "test.incident.missing",
        );
        assert!(missing_linkage.is_err());

        let mut unknown_taxonomy =
            fixture_incident_draft(linked_receipt_id.as_str(), "incident-unknown-taxonomy");
        unknown_taxonomy.taxonomy_code = "unknown.code".to_string();
        let unknown =
            state.report_incident(unknown_taxonomy, 1_762_000_062_000, "test.incident.unknown");
        assert!(unknown.is_err());
    }

    #[test]
    fn rollback_action_receipts_are_idempotent_and_linked() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-rollback"),
            1_762_000_010,
            "test.history",
        );
        let linked_receipt_id = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt")
            .receipt_id
            .clone();
        state
            .set_work_unit_rollback_terms(
                "job-req-123",
                Some("oa://rollback/plans/default"),
                Some("oa://rollback/compensating/default"),
            )
            .expect("rollback terms configured");
        let incident_id = state
            .report_incident(
                fixture_incident_draft(linked_receipt_id.as_str(), "incident-rollback"),
                1_762_000_060_000,
                "test.incident",
            )
            .expect("incident report");

        let draft = RollbackActionDraft {
            work_unit_id: "job-req-123".to_string(),
            idempotency_key: "rollback-action-1".to_string(),
            rollback_receipt_type: RollbackReceiptType::RollbackExecuted,
            incident_id: Some(incident_id),
            linked_receipt_ids: vec![linked_receipt_id],
            reason_code: Some(REASON_CODE_ROLLBACK_EXECUTED.to_string()),
            summary: Some("rollback executed via automation playbook".to_string()),
        };
        let first = state
            .record_rollback_action(draft.clone(), 1_762_000_120_000, "test.rollback")
            .expect("first rollback receipt");
        let second = state
            .record_rollback_action(draft, 1_762_000_120_250, "test.rollback.replay")
            .expect("idempotent rollback replay");

        assert_eq!(first, second);
        let rollback_receipts = state
            .receipts
            .iter()
            .filter(|receipt| receipt.receipt_type == "economy.rollback.executed.v1")
            .collect::<Vec<_>>();
        assert_eq!(rollback_receipts.len(), 1);
        let rollback_receipt = rollback_receipts[0];
        assert_eq!(
            rollback_receipt.hints.reason_code.as_deref(),
            Some(REASON_CODE_ROLLBACK_EXECUTED)
        );
        assert!(
            rollback_receipt
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "incident_object_ref")
        );
        assert!(
            rollback_receipt
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "receipt_ref")
        );
    }

    #[test]
    fn incident_taxonomy_registry_enforces_stable_meaning() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        let entry = IncidentTaxonomyEntry {
            taxonomy_id: "oa.incident.taxonomy".to_string(),
            taxonomy_version: "2026.02".to_string(),
            code: "custom.edge_case".to_string(),
            stable_meaning: "Custom edge case for validation".to_string(),
        };
        state
            .register_incident_taxonomy_entry(entry.clone())
            .expect("first taxonomy insert");
        state
            .register_incident_taxonomy_entry(entry.clone())
            .expect("same meaning should be idempotent");

        let mut changed = entry;
        changed.stable_meaning = "Changed meaning should be rejected".to_string();
        let result = state.register_incident_taxonomy_entry(changed);
        assert!(result.is_err());
    }

    #[test]
    fn incident_update_and_resolution_append_revisions_and_receipts() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-update"),
            1_762_000_010,
            "test.history",
        );
        let linked_receipt_id = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt")
            .receipt_id
            .clone();
        let incident_id = state
            .report_incident(
                fixture_incident_draft(linked_receipt_id.as_str(), "incident-beta"),
                1_762_000_060_000,
                "test.incident.report",
            )
            .expect("incident report");

        state
            .update_incident(
                IncidentUpdateDraft {
                    incident_id: incident_id.clone(),
                    idempotency_key: "incident-beta-update".to_string(),
                    summary: Some("Updated summary after triage".to_string()),
                    linked_receipt_ids: vec![linked_receipt_id.clone()],
                    rollback_receipt_ids: vec![],
                    evidence_digests: vec!["sha256:evidence-update".to_string()],
                },
                1_762_000_120_000,
                "test.incident.update",
            )
            .expect("incident update");
        state
            .resolve_incident(
                IncidentResolutionDraft {
                    incident_id: incident_id.clone(),
                    idempotency_key: "incident-beta-resolve".to_string(),
                    resolution_summary: Some("Resolved after remediation".to_string()),
                    rollback_receipt_ids: vec![],
                    evidence_digests: vec!["sha256:evidence-resolve".to_string()],
                },
                1_762_000_180_000,
                "test.incident.resolve",
            )
            .expect("incident resolve");

        assert!(
            state
                .receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "economy.incident.updated.v1")
        );
        assert!(
            state
                .receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "economy.incident.resolved.v1")
        );
        let latest = state
            .latest_incident_by_id(incident_id.as_str())
            .expect("latest incident");
        assert_eq!(latest.incident_status, IncidentStatus::Resolved);
        assert_eq!(latest.revision, 3);
        assert!(latest.supersedes_digest.is_some());
    }

    #[test]
    fn outcome_registry_entries_are_deterministic_and_emit_update_receipts() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-outcome"),
            1_762_000_010,
            "test.history",
        );
        let linked_receipt_id = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt")
            .receipt_id
            .clone();
        let draft = fixture_outcome_registry_draft(linked_receipt_id.as_str(), "outcome-alpha");

        let first_entry_id = state
            .record_outcome_registry_entry(draft.clone(), 1_762_000_060_000, "test.outcome.create")
            .expect("outcome entry should be recorded");
        let replay_entry_id = state
            .record_outcome_registry_entry(draft, 1_762_000_060_000, "test.outcome.replay")
            .expect("outcome entry replay should be idempotent");
        assert_eq!(first_entry_id, replay_entry_id);

        let latest_before_update = state
            .latest_outcome_registry_entry_by_id(first_entry_id.as_str())
            .expect("latest outcome entry")
            .clone();
        assert_eq!(latest_before_update.revision, 1);
        assert_eq!(latest_before_update.verdict_outcome, "verified");

        state
            .update_outcome_registry_entry(
                OutcomeRegistryUpdateDraft {
                    entry_id: first_entry_id.clone(),
                    idempotency_key: "outcome-alpha-update".to_string(),
                    verdict_outcome: Some("contested".to_string()),
                    settlement_outcome: Some("paid".to_string()),
                    claim_outcome: Some("paid".to_string()),
                    remedy_outcome: Some("rollback_executed".to_string()),
                    incident_tags: vec!["finance.claim_dispute".to_string()],
                    linked_receipt_ids: vec![linked_receipt_id.clone()],
                    evidence_digests: vec!["sha256:outcome-evidence-2".to_string()],
                },
                1_762_000_090_000,
                "test.outcome.update",
            )
            .expect("outcome entry update");
        let latest_after_update = state
            .latest_outcome_registry_entry_by_id(first_entry_id.as_str())
            .expect("latest updated outcome entry")
            .clone();
        assert_eq!(latest_after_update.revision, 2);
        assert_eq!(latest_after_update.verdict_outcome, "contested");
        assert_eq!(latest_after_update.settlement_outcome, "paid");
        assert_eq!(latest_after_update.claim_outcome.as_deref(), Some("paid"));
        assert_eq!(
            latest_after_update.remedy_outcome.as_deref(),
            Some("rollback_executed")
        );
        assert_eq!(
            latest_after_update.incident_tags,
            vec!["finance.claim_dispute".to_string()]
        );
        assert!(latest_after_update.supersedes_digest.is_some());
        assert!(
            state
                .receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "economy.outcome_registry.created.v1")
        );
        assert!(
            state
                .receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "economy.outcome_registry.updated.v1")
        );
    }

    #[test]
    fn audit_export_includes_outcome_registry_objects_with_public_redaction() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-outcome-export"),
            1_762_000_010,
            "test.history",
        );
        let linked_receipt_id = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt")
            .receipt_id
            .clone();
        state
            .record_outcome_registry_entry(
                fixture_outcome_registry_draft(linked_receipt_id.as_str(), "outcome-export-object"),
                1_762_000_060_000,
                "test.outcome.export",
            )
            .expect("outcome entry recorded");

        let query = ReceiptQuery::default();
        let restricted = state
            .export_audit_package(
                &query,
                AuditExportRedactionTier::Restricted,
                1_762_000_090_000,
            )
            .expect("restricted audit export");
        let public = state
            .export_audit_package(&query, AuditExportRedactionTier::Public, 1_762_000_090_000)
            .expect("public audit export");
        assert_eq!(restricted.outcome_registry_object_count, 1);
        assert_eq!(public.outcome_registry_object_count, 1);
        assert!(
            restricted
                .outcome_registry_objects
                .iter()
                .all(|object| !object.linked_receipt_ids.is_empty())
        );
        assert!(
            public
                .outcome_registry_objects
                .iter()
                .all(|object| object.linked_receipt_ids.is_empty())
        );
        assert!(restricted.linkage_edges.iter().any(|edge| {
            edge.relation_kind == "outcome_linked_receipt"
                && edge
                    .from_receipt_id
                    .starts_with("outcome_registry:outcome:")
                && edge.to_receipt_id == linked_receipt_id
        }));
    }

    #[test]
    fn safety_signal_feed_is_deterministic_and_public_mode_is_aggregate_only() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-safety-signal"),
            1_762_000_010,
            "test.history",
        );
        let linked_receipt_id = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt")
            .receipt_id
            .clone();
        state
            .report_incident(
                fixture_incident_draft(linked_receipt_id.as_str(), "incident-safety-signal"),
                1_762_000_060_000,
                "test.incident",
            )
            .expect("incident report");
        let drift_alert = fixture_drift_alert_receipt(
            "receipt.drift.alert.safety-signal",
            "job-req-123",
            1_762_000_065_000,
        );
        state.append_receipt(Ok(drift_alert), "test.drift");

        let query = ReceiptQuery::default();
        let restricted_a = state
            .export_safety_signal_feed(
                &query,
                SafetySignalExportMode::RestrictedFeed,
                1_762_000_090_000,
                Some("auditor"),
                Some("auditor:alice"),
            )
            .expect("restricted safety feed");
        let restricted_b = state
            .export_safety_signal_feed(
                &query,
                SafetySignalExportMode::RestrictedFeed,
                1_762_000_090_000,
                Some("auditor"),
                Some("auditor:alice"),
            )
            .expect("restricted safety feed replay");
        let public = state
            .export_safety_signal_feed(
                &query,
                SafetySignalExportMode::PublicAggregate,
                1_762_000_090_000,
                None,
                None,
            )
            .expect("public safety feed");

        assert!(!restricted_a.signals.is_empty());
        assert!(
            restricted_a
                .signals
                .iter()
                .all(|signal| !signal.hashed_indicators.is_empty())
        );
        assert_eq!(restricted_a, restricted_b);
        assert!(public.signals.is_empty());
        assert_eq!(public.bucket_count, restricted_a.bucket_count);
        assert_ne!(public.package_hash, restricted_a.package_hash);
    }

    #[test]
    fn restricted_safety_signal_feed_enforces_policy_authentication_gate() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-safety-gate"),
            1_762_000_010,
            "test.history",
        );
        let linked_receipt_id = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt")
            .receipt_id
            .clone();
        state
            .report_incident(
                fixture_incident_draft(linked_receipt_id.as_str(), "incident-safety-gate"),
                1_762_000_060_000,
                "test.incident",
            )
            .expect("incident report");

        let query = ReceiptQuery::default();
        let denied = state.export_safety_signal_feed(
            &query,
            SafetySignalExportMode::RestrictedFeed,
            1_762_000_090_000,
            Some("auditor"),
            Some(""),
        );
        assert!(denied.is_err());
        let error = denied.err().unwrap_or_default();
        assert!(error.contains("restricted safety signal feed denied"));

        let allowed = state.export_safety_signal_feed(
            &query,
            SafetySignalExportMode::RestrictedFeed,
            1_762_000_090_000,
            Some("auditor"),
            Some("auditor:authenticated"),
        );
        assert!(allowed.is_ok());
    }

    #[test]
    fn incident_audit_export_supports_public_and_restricted_redaction_tiers() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-export"),
            1_762_000_010,
            "test.history",
        );
        let linked_receipt_id = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt")
            .receipt_id
            .clone();
        let incident_id = state
            .report_incident(
                fixture_incident_draft(linked_receipt_id.as_str(), "incident-export"),
                1_762_000_060_000,
                "test.incident.report",
            )
            .expect("incident report");
        let rollback_receipt_id = state
            .record_rollback_action(
                RollbackActionDraft {
                    work_unit_id: "job-req-123".to_string(),
                    idempotency_key: "rollback-export-link".to_string(),
                    rollback_receipt_type: RollbackReceiptType::RollbackExecuted,
                    incident_id: Some(incident_id.clone()),
                    linked_receipt_ids: vec![linked_receipt_id.clone()],
                    reason_code: Some(REASON_CODE_ROLLBACK_EXECUTED.to_string()),
                    summary: Some("rollback executed for export linkage".to_string()),
                },
                1_762_000_120_000,
                "test.rollback.export",
            )
            .expect("rollback receipt");
        state
            .update_incident(
                IncidentUpdateDraft {
                    incident_id,
                    idempotency_key: "incident-export-link-rollback".to_string(),
                    summary: None,
                    linked_receipt_ids: vec![linked_receipt_id.clone()],
                    rollback_receipt_ids: vec![rollback_receipt_id.clone()],
                    evidence_digests: vec!["sha256:incident-export-link".to_string()],
                },
                1_762_000_150_000,
                "test.incident.link.rollback",
            )
            .expect("incident linkage update");

        let restricted = state
            .export_incident_audit_package(
                IncidentExportRedactionTier::Restricted,
                1_762_000_200_000,
            )
            .expect("restricted export");
        let public = state
            .export_incident_audit_package(IncidentExportRedactionTier::Public, 1_762_000_200_000)
            .expect("public export");
        assert_eq!(restricted.incident_count, public.incident_count);
        assert_ne!(restricted.package_hash, public.package_hash);
        assert!(
            restricted
                .incidents
                .iter()
                .all(|incident| incident.summary != "[redacted]")
        );
        assert!(
            public
                .incidents
                .iter()
                .all(|incident| incident.summary == "[redacted]")
        );
        assert!(
            restricted
                .incident_receipts
                .iter()
                .any(|receipt| receipt.receipt_id == rollback_receipt_id)
        );
        assert!(public.incident_receipts.iter().all(|receipt| {
            receipt
                .evidence
                .iter()
                .all(|evidence| evidence.uri == "oa://redacted")
        }));
    }

    #[test]
    fn simulation_scenario_export_is_deterministic_and_receipted() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-simulation-export"),
            1_762_000_010,
            "test.history",
        );
        let linked_receipt_id = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt")
            .receipt_id
            .clone();
        let ground_truth_case_id = state
            .report_incident(
                fixture_ground_truth_case_draft(
                    linked_receipt_id.as_str(),
                    "ground-truth-case-sim-export",
                ),
                1_762_000_060_000,
                "test.incident.ground_truth",
            )
            .expect("ground truth case");

        let first = state
            .export_simulation_scenario_package(
                SimulationScenarioExportRedactionTier::Restricted,
                1_762_000_120_000,
                "test.simulation.export",
            )
            .expect("first simulation export");
        let second = state
            .export_simulation_scenario_package(
                SimulationScenarioExportRedactionTier::Restricted,
                1_762_000_120_000,
                "test.simulation.export.replay",
            )
            .expect("replay simulation export");
        assert_eq!(first.package_hash, second.package_hash);
        assert_eq!(first.scenario_count, 1);
        assert_eq!(
            first.redaction_tier,
            SimulationScenarioExportRedactionTier::Restricted
        );
        let scenario = first.scenarios.first().expect("scenario should exist");
        assert_eq!(scenario.ground_truth_case_id, ground_truth_case_id);
        assert_eq!(scenario.linked_receipt_ids, vec![linked_receipt_id.clone()]);
        assert!(!scenario.linked_receipt_digests.is_empty());
        assert!(!scenario.derived_from_receipt_ids.is_empty());

        let redaction_receipt = state
            .get_receipt(first.redaction_policy_receipt_id.as_str())
            .expect("redaction policy receipt");
        assert_eq!(
            redaction_receipt.receipt_type,
            "economy.simulation_scenario.redaction_policy_applied.v1"
        );
        assert_eq!(
            redaction_receipt.hints.reason_code.as_deref(),
            Some(REASON_CODE_REDACTION_POLICY_APPLIED)
        );
        let export_receipt = state
            .get_receipt(first.export_receipt_id.as_str())
            .expect("simulation export receipt");
        assert_eq!(
            export_receipt.receipt_type,
            "economy.simulation_scenario.exported.v1"
        );
        assert_eq!(
            export_receipt.hints.reason_code.as_deref(),
            Some(REASON_CODE_SIMULATION_SCENARIO_EXPORTED)
        );
        assert!(
            export_receipt
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "redaction_policy_receipt_ref")
        );
        assert!(
            export_receipt
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "simulation_scenario_ref")
        );
    }

    #[test]
    fn simulation_scenario_export_public_tier_redacts_receipt_ids() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-simulation-redaction"),
            1_762_000_010,
            "test.history",
        );
        let linked_receipt_id = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt")
            .receipt_id
            .clone();
        state
            .report_incident(
                fixture_ground_truth_case_draft(
                    linked_receipt_id.as_str(),
                    "ground-truth-case-sim-redaction",
                ),
                1_762_000_060_000,
                "test.incident.ground_truth",
            )
            .expect("ground truth case");

        let restricted = state
            .export_simulation_scenario_package(
                SimulationScenarioExportRedactionTier::Restricted,
                1_762_000_120_000,
                "test.simulation.restricted",
            )
            .expect("restricted simulation export");
        let public = state
            .export_simulation_scenario_package(
                SimulationScenarioExportRedactionTier::Public,
                1_762_000_120_000,
                "test.simulation.public",
            )
            .expect("public simulation export");
        assert_eq!(restricted.scenario_count, public.scenario_count);
        assert_ne!(restricted.package_hash, public.package_hash);
        let restricted_scenario = restricted.scenarios.first().expect("restricted scenario");
        let public_scenario = public.scenarios.first().expect("public scenario");
        assert!(!restricted_scenario.linked_receipt_ids.is_empty());
        assert!(public_scenario.linked_receipt_ids.is_empty());
        assert!(public_scenario.rollback_receipt_ids.is_empty());
        assert!(public_scenario.derived_from_receipt_ids.is_empty());
        assert_eq!(
            restricted_scenario.linked_receipt_digests,
            public_scenario.linked_receipt_digests
        );
    }

    #[test]
    fn anchor_publication_receipt_is_idempotent_per_snapshot_backend() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        let draft = fixture_anchor_publication_draft(
            "snapshot.economy:1762000060000",
            "sha256:snapshot-anchor",
            "bitcoin",
            "txid:abc123",
        );

        let first_receipt_id = state
            .publish_anchor_receipt(draft.clone(), 1_762_000_120_000, "test.anchor")
            .expect("first anchor publication");
        let second_receipt_id = state
            .publish_anchor_receipt(draft, 1_762_000_120_000, "test.anchor.replay")
            .expect("anchor replay");
        assert_eq!(first_receipt_id, second_receipt_id);
        assert_eq!(
            state
                .receipts
                .iter()
                .filter(|receipt| receipt.receipt_type == "economy.anchor.published.v1")
                .count(),
            1
        );
        let receipt = state
            .get_receipt(first_receipt_id.as_str())
            .expect("anchor receipt");
        assert_eq!(
            receipt.hints.reason_code.as_deref(),
            Some(REASON_CODE_ANCHOR_PUBLISHED)
        );
        let anchor_proof_ref = receipt
            .evidence
            .iter()
            .find(|evidence| evidence.kind == "anchor_proof_ref")
            .expect("anchor proof ref");
        assert!(anchor_proof_ref.digest.starts_with("sha256:"));
        assert!(!anchor_proof_ref.uri.contains("txid:abc123"));
    }

    #[test]
    fn audit_export_includes_anchor_entries_and_preserves_hash_stability() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        state
            .publish_anchor_receipt(
                fixture_anchor_publication_draft(
                    "snapshot.economy:1762000060000",
                    "sha256:snapshot-anchor",
                    "bitcoin",
                    "txid:anchor-proof-1",
                ),
                1_762_000_120_000,
                "test.anchor.audit",
            )
            .expect("anchor publication");

        let query = ReceiptQuery::default();
        let first = state
            .export_audit_package(
                &query,
                AuditExportRedactionTier::Restricted,
                1_762_000_180_000,
            )
            .expect("first restricted audit export");
        let second = state
            .export_audit_package(
                &query,
                AuditExportRedactionTier::Restricted,
                1_762_000_180_000,
            )
            .expect("second restricted audit export");
        assert_eq!(first.package_hash, second.package_hash);
        assert_eq!(first.anchor_count, 1);
        assert_eq!(first.anchors.len(), 1);
        let anchor = first.anchors.first().expect("anchor entry");
        assert_eq!(anchor.anchor_backend, "bitcoin");
        assert_eq!(anchor.snapshot_id, "snapshot.economy:1762000060000");
        assert!(anchor.snapshot_hash.starts_with("sha256:"));
        assert!(anchor.anchor_proof_ref.digest.starts_with("sha256:"));
        assert!(
            first
                .linkage_edges
                .iter()
                .any(|edge| edge.relation_kind == "anchor_snapshot")
        );
    }

    #[test]
    fn white_hat_template_registration_is_idempotent_and_receipted() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        let draft = fixture_white_hat_template_draft(
            "work-unit:audit:001",
            "white-hat-template-audit-001",
            WhiteHatWorkUnitKind::Audit,
        );

        let first_receipt_id = state
            .register_white_hat_work_unit_template(
                draft.clone(),
                1_762_000_090_000,
                "test.white_hat.template",
            )
            .expect("template registration");
        let second_receipt_id = state
            .register_white_hat_work_unit_template(
                draft,
                1_762_000_090_000,
                "test.white_hat.template.replay",
            )
            .expect("template idempotent replay");
        assert_eq!(first_receipt_id, second_receipt_id);
        assert_eq!(
            state
                .receipts
                .iter()
                .filter(|receipt| receipt.receipt_type == "economy.work_unit.template_registered.v1")
                .count(),
            1
        );
        let metadata = state
            .work_units
            .get("work-unit:audit:001")
            .expect("work unit metadata");
        assert_eq!(metadata.category, "audit");
        assert_eq!(metadata.template_kind, Some(WhiteHatWorkUnitKind::Audit));
        assert!(metadata.mandatory_provenance);
        assert!(metadata.acceptance_criteria_ref.is_some());
        assert!(metadata.coordinated_disclosure_ref.is_some());
    }

    #[test]
    fn white_hat_bounty_dispute_flow_links_receipts_and_exports_outcomes() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        state
            .register_white_hat_work_unit_template(
                fixture_white_hat_template_draft(
                    "work-unit:redteam:001",
                    "white-hat-template-redteam-001",
                    WhiteHatWorkUnitKind::Redteam,
                ),
                1_762_000_100_000,
                "test.white_hat.template",
            )
            .expect("template registration");
        append_synthetic_verdict_receipt(
            &mut state,
            "receipt.verdict.whitehat.001",
            "work-unit:redteam:001",
            1_762_000_110_000,
        );

        let mut draft = fixture_white_hat_bounty_settlement_draft(
            "work-unit:redteam:001",
            "white-hat-bounty-settlement-001",
            "receipt.verdict.whitehat.001",
        );
        draft.disputed = true;
        draft.dispute_reason = Some("potential duplicate finding; opening dispute".to_string());
        draft.incident_report = Some(fixture_incident_draft(
            "receipt.verdict.whitehat.001",
            "white-hat-dispute-incident-001",
        ));

        let result = state
            .settle_white_hat_bounty(draft, 1_762_000_120_000, "test.white_hat.bounty")
            .expect("bounty settlement");
        let dispute_receipt_id = result
            .dispute_receipt_id
            .clone()
            .expect("dispute receipt should exist");
        let settlement_receipt = state
            .get_receipt(result.settlement_receipt_id.as_str())
            .expect("settlement receipt");
        assert_eq!(
            settlement_receipt.hints.reason_code.as_deref(),
            Some(REASON_CODE_BOUNTY_SETTLEMENT_WITHHELD)
        );
        assert!(settlement_receipt.evidence.iter().any(|evidence| {
            evidence.kind == "receipt_ref"
                && evidence.uri == "oa://receipts/receipt.verdict.whitehat.001"
        }));
        assert!(settlement_receipt.evidence.iter().any(|evidence| {
            evidence.kind == "receipt_ref"
                && evidence.uri == format!("oa://receipts/{dispute_receipt_id}")
        }));
        let dispute_receipt = state
            .get_receipt(dispute_receipt_id.as_str())
            .expect("dispute receipt");
        assert_eq!(
            dispute_receipt.hints.reason_code.as_deref(),
            Some(REASON_CODE_BOUNTY_DISPUTE_OPENED)
        );
        let outcome = state
            .latest_outcome_registry_entry_by_id(result.outcome_entry_id.as_str())
            .expect("outcome entry");
        assert_eq!(outcome.settlement_outcome, "withheld");
        assert_eq!(outcome.claim_outcome.as_deref(), Some("disputed"));
        assert!(
            outcome
                .linked_receipt_ids
                .iter()
                .any(|receipt_id| receipt_id == result.settlement_receipt_id.as_str())
        );
        let incident_id = result.incident_id.expect("incident export");
        assert!(state.latest_incident_by_id(incident_id.as_str()).is_some());
    }

    #[test]
    fn white_hat_bounty_settlement_is_idempotent() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        state
            .register_white_hat_work_unit_template(
                fixture_white_hat_template_draft(
                    "work-unit:incident-repro:001",
                    "white-hat-template-incident-repro-001",
                    WhiteHatWorkUnitKind::IncidentRepro,
                ),
                1_762_000_100_000,
                "test.white_hat.template",
            )
            .expect("template registration");
        append_synthetic_verdict_receipt(
            &mut state,
            "receipt.verdict.whitehat.002",
            "work-unit:incident-repro:001",
            1_762_000_110_000,
        );
        let draft = fixture_white_hat_bounty_settlement_draft(
            "work-unit:incident-repro:001",
            "white-hat-bounty-settlement-002",
            "receipt.verdict.whitehat.002",
        );

        let first = state
            .settle_white_hat_bounty(draft.clone(), 1_762_000_120_000, "test.white_hat.bounty")
            .expect("first settlement");
        let second = state
            .settle_white_hat_bounty(draft, 1_762_000_120_000, "test.white_hat.bounty.replay")
            .expect("settlement replay");
        assert_eq!(first.settlement_receipt_id, second.settlement_receipt_id);
        assert_eq!(first.outcome_entry_id, second.outcome_entry_id);
        assert_eq!(first.dispute_receipt_id, None);
        assert_eq!(
            state
                .receipts
                .iter()
                .filter(|receipt| receipt.receipt_type == "economy.bounty.settlement.finalized.v1")
                .count(),
            1
        );
        assert_eq!(
            state
                .outcome_registry_entries
                .iter()
                .filter(|entry| entry.entry_id == first.outcome_entry_id)
                .count(),
            1
        );
    }

    #[test]
    fn drift_receipts_are_emitted_once_per_snapshot_window() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        let drift_signals = fixture_drift_signals(true);
        let top_drift_signals = drift_signals.clone();

        state.record_economy_snapshot_receipt(
            "snapshot.economy:1762000060000",
            1_762_000_060_000,
            "sha256:snapshot",
            0.5,
            0.5,
            0.5,
            0.5,
            2,
            1.0,
            0.0,
            0.0,
            0.0,
            125,
            25,
            2_000,
            100,
            0.2,
            0.05,
            3,
            drift_signals.clone(),
            top_drift_signals.clone(),
            0,
            0,
            0.0,
            vec![],
            "sha256:audit-public".to_string(),
            "sha256:audit-restricted".to_string(),
            vec![],
            "test.snapshot",
        );
        state.record_economy_snapshot_receipt(
            "snapshot.economy:1762000060000",
            1_762_000_060_000,
            "sha256:snapshot",
            0.5,
            0.5,
            0.5,
            0.5,
            2,
            1.0,
            0.0,
            0.0,
            0.0,
            125,
            25,
            2_000,
            100,
            0.2,
            0.05,
            3,
            drift_signals,
            top_drift_signals,
            0,
            0,
            0.0,
            vec![],
            "sha256:audit-public".to_string(),
            "sha256:audit-restricted".to_string(),
            vec![],
            "test.snapshot.replay",
        );

        assert_eq!(
            state
                .receipts
                .iter()
                .filter(|receipt| receipt.receipt_type == "economy.stats.snapshot_receipt.v1")
                .count(),
            1
        );
        assert_eq!(
            state
                .receipts
                .iter()
                .filter(|receipt| receipt.receipt_type == "economy.drift.signal_emitted.v1")
                .count(),
            1
        );
        assert_eq!(
            state
                .receipts
                .iter()
                .filter(|receipt| receipt.receipt_type == "economy.drift.alert_raised.v1")
                .count(),
            1
        );
        let signal = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "economy.drift.signal_emitted.v1")
            .expect("drift signal receipt");
        assert_eq!(
            signal.hints.reason_code.as_deref(),
            Some(REASON_CODE_DRIFT_SIGNAL_EMITTED)
        );
        assert!(
            signal
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "snapshot_ref")
        );
        assert!(
            signal
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "drift_detector_ref")
        );
    }

    #[test]
    fn drift_false_positive_receipt_is_emitted_when_alert_clears() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);

        let active_signals = fixture_drift_signals(true);
        state.record_economy_snapshot_receipt(
            "snapshot.economy:1762000060000",
            1_762_000_060_000,
            "sha256:snapshot-a",
            0.5,
            0.5,
            0.5,
            0.5,
            2,
            1.0,
            0.0,
            0.0,
            0.0,
            125,
            25,
            2_000,
            100,
            0.2,
            0.05,
            3,
            active_signals.clone(),
            active_signals,
            0,
            0,
            0.0,
            vec![],
            "sha256:audit-public".to_string(),
            "sha256:audit-restricted".to_string(),
            vec![],
            "test.snapshot.a",
        );

        let cleared_signals = fixture_drift_signals(false);
        state.record_economy_snapshot_receipt(
            "snapshot.economy:1762000120000",
            1_762_000_120_000,
            "sha256:snapshot-b",
            0.8,
            0.8,
            0.8,
            0.8,
            2,
            1.6,
            0.0,
            0.0,
            0.0,
            125,
            25,
            2_000,
            100,
            0.2,
            0.05,
            0,
            cleared_signals,
            vec![],
            0,
            0,
            0.0,
            vec![],
            "sha256:audit-public".to_string(),
            "sha256:audit-restricted".to_string(),
            vec![],
            "test.snapshot.b",
        );

        let false_positive = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "economy.drift.false_positive_confirmed.v1")
            .expect("false positive receipt");
        assert_eq!(
            false_positive.hints.reason_code.as_deref(),
            Some(REASON_CODE_DRIFT_FALSE_POSITIVE_CONFIRMED)
        );
        assert!(
            false_positive
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "snapshot_ref")
        );
        assert!(
            false_positive
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "drift_detector_ref")
        );
    }

    #[test]
    fn liability_pricing_is_deterministic_given_snapshot_and_policy_inputs() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);

        state.record_economy_snapshot_receipt(
            "snapshot.economy:1762000060000",
            1_762_000_060_000,
            "sha256:snapshot",
            0.55,
            0.55,
            0.55,
            0.55,
            4,
            2.2,
            0.1,
            0.2,
            0.3,
            400,
            100,
            2_000,
            300,
            0.25,
            0.15,
            3,
            vec![
                DriftSignalSummary {
                    detector_id: "detector.drift.sv_floor".to_string(),
                    signal_code: "sv_below_floor".to_string(),
                    count_24h: 2,
                    ratio: 0.55,
                    threshold: 0.70,
                    score: 0.2142857142857142,
                    alert: true,
                },
                DriftSignalSummary {
                    detector_id: "detector.drift.dispute_claim_spike".to_string(),
                    signal_code: "dispute_claim_share_high".to_string(),
                    count_24h: 1,
                    ratio: 0.1,
                    threshold: 0.08,
                    score: 0.021739130434782594,
                    alert: true,
                },
            ],
            vec![
                DriftSignalSummary {
                    detector_id: "detector.drift.sv_floor".to_string(),
                    signal_code: "sv_below_floor".to_string(),
                    count_24h: 2,
                    ratio: 0.55,
                    threshold: 0.70,
                    score: 0.2142857142857142,
                    alert: true,
                },
                DriftSignalSummary {
                    detector_id: "detector.drift.dispute_claim_spike".to_string(),
                    signal_code: "dispute_claim_share_high".to_string(),
                    count_24h: 1,
                    ratio: 0.1,
                    threshold: 0.08,
                    score: 0.021739130434782594,
                    alert: true,
                },
            ],
            0,
            0,
            0.0,
            vec![],
            "sha256:audit-public".to_string(),
            "sha256:audit-restricted".to_string(),
            vec![],
            "test.snapshot",
        );
        let bundle = default_policy_bundle();
        let first = compute_liability_pricing_for_settlement(
            state.receipts.as_slice(),
            &bundle,
            "compute",
            FeedbackLatencyClass::Short,
            SeverityClass::Low,
            10_000,
            1_000,
            None,
        );
        let second = compute_liability_pricing_for_settlement(
            state.receipts.as_slice(),
            &bundle,
            "compute",
            FeedbackLatencyClass::Short,
            SeverityClass::Low,
            10_000,
            1_000,
            None,
        );

        assert_eq!(first.liability_premium_sats, second.liability_premium_sats);
        assert_eq!(
            first.effective_liability_premium_bps,
            second.effective_liability_premium_bps
        );
        assert_eq!(first.pricing_snapshot_id, second.pricing_snapshot_id);
        assert_eq!(first.pricing_snapshot_hash, second.pricing_snapshot_hash);
        assert_eq!(first.risk_pricing_rule_id, second.risk_pricing_rule_id);
        assert_eq!(
            first.pricing_snapshot_id,
            "snapshot.economy:1762000060000".to_string()
        );
        assert_eq!(first.pricing_snapshot_hash, "sha256:snapshot".to_string());
        assert!(first.liability_premium_sats > 0);
    }

    #[test]
    fn exported_settlement_bundle_keeps_pricing_refs_digest_only() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        state.record_economy_snapshot_receipt(
            "snapshot.economy:1762000060000",
            1_762_000_060_000,
            "sha256:snapshot",
            0.5,
            0.5,
            0.5,
            0.5,
            2,
            1.0,
            0.0,
            0.1,
            0.2,
            125,
            25,
            2_000,
            100,
            0.2,
            0.05,
            0,
            vec![],
            vec![],
            0,
            0,
            0.0,
            vec![],
            "sha256:audit-public".to_string(),
            "sha256:audit-restricted".to_string(),
            vec![],
            "test.snapshot",
        );
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-1"),
            1_762_000_010,
            "test.history",
        );

        let query = ReceiptQuery {
            start_inclusive_ms: Some(1_762_000_000_000),
            end_inclusive_ms: Some(1_762_000_080_000),
            work_unit_id: Some("job-req-123".to_string()),
            receipt_type: Some("earn.job.settlement_observed.v1".to_string()),
        };
        let bundle = state
            .export_receipt_bundle(&query, 1_762_000_090_000)
            .expect("bundle export");
        assert_eq!(bundle.receipt_count, 1);
        let settlement = &bundle.receipts[0];
        assert!(
            settlement
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "pricing_snapshot_ref")
        );
        assert!(
            settlement
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "risk_pricing_rule_ref")
        );
        assert!(
            !settlement
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "snapshot_metrics")
        );
        assert!(settlement.hints.liability_premium.is_some());
    }

    #[test]
    fn policy_rule_selection_prefers_specificity_then_rule_id() {
        let bundle = PolicyBundleConfig {
            authentication_rules: vec![
                AuthenticationPolicyRule {
                    rule_id: "rule.global".to_string(),
                    slice: PolicySliceRule {
                        category: None,
                        tfb_class: None,
                        severity: None,
                    },
                    role: Some("operator".to_string()),
                    min_auth_assurance: Some("authenticated".to_string()),
                    require_personhood: false,
                },
                AuthenticationPolicyRule {
                    rule_id: "rule.specific.b".to_string(),
                    slice: PolicySliceRule {
                        category: Some("compute".to_string()),
                        tfb_class: Some(FeedbackLatencyClass::Short),
                        severity: Some(SeverityClass::High),
                    },
                    role: Some("operator".to_string()),
                    min_auth_assurance: Some("personhood".to_string()),
                    require_personhood: true,
                },
                AuthenticationPolicyRule {
                    rule_id: "rule.specific.a".to_string(),
                    slice: PolicySliceRule {
                        category: Some("compute".to_string()),
                        tfb_class: Some(FeedbackLatencyClass::Short),
                        severity: Some(SeverityClass::High),
                    },
                    role: Some("operator".to_string()),
                    min_auth_assurance: Some("personhood".to_string()),
                    require_personhood: true,
                },
            ],
            ..PolicyBundleConfig::default()
        };

        let selected_specific = select_authentication_rule(
            &bundle,
            "compute",
            FeedbackLatencyClass::Short,
            SeverityClass::High,
            "operator",
        )
        .expect("specific rule should match");
        assert_eq!(selected_specific.rule_id, "rule.specific.a");

        let selected_global = select_authentication_rule(
            &bundle,
            "router",
            FeedbackLatencyClass::Instant,
            SeverityClass::Low,
            "operator",
        )
        .expect("global fallback should match");
        assert_eq!(selected_global.rule_id, "rule.global");
    }

    #[test]
    fn provenance_grade_is_deterministic_for_equivalent_evidence_sets() {
        let evidence_a = vec![
            EvidenceRef::new(
                "provenance_bundle",
                "oa://provenance/a",
                "sha256:bundle_a".to_string(),
            ),
            EvidenceRef::new("data_source_ref", "oa://data/source", "sha256:data"),
            EvidenceRef::new(
                "permissioning_ref",
                "oa://permissions/capability/text_generation",
                "sha256:perm",
            ),
            EvidenceRef::new(
                "attestation:model_version",
                "oa://attestation/model",
                "sha256:model",
            ),
            EvidenceRef::new(
                "attestation:runtime_integrity",
                "oa://attestation/runtime",
                "sha256:runtime",
            ),
        ];
        let evidence_b = vec![
            EvidenceRef::new(
                "attestation:runtime_integrity",
                "oa://attestation/runtime",
                "sha256:runtime",
            ),
            EvidenceRef::new(
                "permissioning_ref",
                "oa://permissions/capability/text_generation",
                "sha256:perm",
            ),
            EvidenceRef::new(
                "provenance_bundle",
                "oa://provenance/a",
                "sha256:bundle_a".to_string(),
            ),
            EvidenceRef::new(
                "attestation:model_version",
                "oa://attestation/model",
                "sha256:model",
            ),
            EvidenceRef::new("data_source_ref", "oa://data/source", "sha256:data"),
        ];

        let grade_a = provenance_grade_from_features(&provenance_features_from_evidence(
            evidence_a.as_slice(),
        ));
        let grade_b = provenance_grade_from_features(&provenance_features_from_evidence(
            evidence_b.as_slice(),
        ));
        assert_eq!(grade_a, ProvenanceGrade::P3Attested);
        assert_eq!(grade_b, ProvenanceGrade::P3Attested);
        assert_eq!(grade_a, grade_b);
    }

    #[test]
    fn provenance_policy_enforcement_requires_attestations_and_permissioning() {
        let bundle = PolicyBundleConfig {
            provenance_rules: vec![ProvenancePolicyRule {
                rule_id: "policy.test.provenance.requirements.v1".to_string(),
                slice: PolicySliceRule {
                    category: Some("compute".to_string()),
                    tfb_class: Some(FeedbackLatencyClass::Short),
                    severity: Some(SeverityClass::High),
                },
                min_grade: Some(ProvenanceGrade::P3Attested),
                require_provenance_bundle: true,
                require_permissioning_refs: true,
                required_attestation_kinds: vec![
                    ProvenanceAttestationKind::ModelVersion,
                    ProvenanceAttestationKind::RuntimeIntegrity,
                ],
            }],
            ..PolicyBundleConfig::default()
        };
        let missing_runtime = vec![
            EvidenceRef::new("provenance_bundle", "oa://provenance/a", "sha256:bundle"),
            EvidenceRef::new("data_source_ref", "oa://data/source", "sha256:data"),
            EvidenceRef::new(
                "permissioning_ref",
                "oa://permissions/capability/text_generation",
                "sha256:perm",
            ),
            EvidenceRef::new(
                "attestation:model_version",
                "oa://attestation/model",
                "sha256:model",
            ),
        ];
        let failed = evaluate_provenance_gate(
            &bundle,
            "compute",
            FeedbackLatencyClass::Short,
            SeverityClass::High,
            missing_runtime.as_slice(),
        );
        assert!(failed.is_err());
        assert!(
            failed.err().is_some_and(
                |decision| decision.rule_id == "policy.test.provenance.requirements.v1"
            )
        );

        let mut complete = missing_runtime;
        complete.push(EvidenceRef::new(
            "attestation:runtime_integrity",
            "oa://attestation/runtime",
            "sha256:runtime",
        ));
        let passed = evaluate_provenance_gate(
            &bundle,
            "compute",
            FeedbackLatencyClass::Short,
            SeverityClass::High,
            complete.as_slice(),
        );
        assert_eq!(passed.ok(), Some(ProvenanceGrade::P3Attested));
    }

    #[test]
    fn triggered_policy_actions_follow_deterministic_order() {
        let bundle = PolicyBundleConfig {
            autonomy_rules: vec![AutonomyPolicyRule {
                rule_id: "policy.autonomy.a".to_string(),
                slice: PolicySliceRule {
                    category: Some("compute".to_string()),
                    tfb_class: Some(FeedbackLatencyClass::Short),
                    severity: Some(SeverityClass::High),
                },
                min_sv: Some(0.80),
                min_sv_effective: Some(0.80),
                max_xa_hat: Some(0.10),
                max_delta_m_hat: None,
                max_correlated_share: None,
                max_drift_alerts_24h: None,
                actions: vec![
                    ThrottleActionKind::DisableWarranties,
                    ThrottleActionKind::SetModeDegraded,
                    ThrottleActionKind::RaiseRequiredTier,
                ],
            }],
            monitoring_rules: vec![MonitoringPolicyRule {
                rule_id: "policy.monitoring.z".to_string(),
                slice: PolicySliceRule {
                    category: Some("compute".to_string()),
                    tfb_class: Some(FeedbackLatencyClass::Short),
                    severity: Some(SeverityClass::High),
                },
                required_detectors: vec!["detector.core".to_string()],
                drift_alert_threshold_24h: Some(5),
                actions: vec![
                    ThrottleActionKind::HaltNewEnvelopes,
                    ThrottleActionKind::RequireHumanStep,
                ],
            }],
            ..PolicyBundleConfig::default()
        };

        let actions = evaluate_triggered_policy_actions(
            &bundle,
            "compute",
            FeedbackLatencyClass::Short,
            SeverityClass::High,
            SnapshotPolicyMetrics {
                sv: 0.30,
                sv_effective: 0.30,
                rho_effective: 0.30,
                xa_hat: 0.20,
                delta_m_hat: 0.0,
                correlated_verification_share: 0.0,
                drift_alerts_24h: 20,
            },
        );

        let labels = actions
            .into_iter()
            .map(|action| action.action.label().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec![
                "set_mode_degraded".to_string(),
                "raise_required_tier".to_string(),
                "require_human_step".to_string(),
                "halt_new_envelopes".to_string(),
                "disable_warranties".to_string(),
            ]
        );
    }

    #[test]
    fn autonomy_thresholds_use_sv_effective_not_raw_sv() {
        let bundle = PolicyBundleConfig {
            autonomy_rules: vec![AutonomyPolicyRule {
                rule_id: "policy.autonomy.sv_effective_floor.v1".to_string(),
                slice: PolicySliceRule {
                    category: Some("compute".to_string()),
                    tfb_class: Some(FeedbackLatencyClass::Short),
                    severity: Some(SeverityClass::High),
                },
                min_sv: Some(0.80),
                min_sv_effective: Some(0.80),
                max_xa_hat: None,
                max_delta_m_hat: None,
                max_correlated_share: None,
                max_drift_alerts_24h: None,
                actions: vec![ThrottleActionKind::SetModeDegraded],
            }],
            ..PolicyBundleConfig::default()
        };

        let actions = evaluate_triggered_policy_actions(
            &bundle,
            "compute",
            FeedbackLatencyClass::Short,
            SeverityClass::High,
            SnapshotPolicyMetrics {
                sv: 0.95,
                sv_effective: 0.50,
                rho_effective: 0.50,
                xa_hat: 0.0,
                delta_m_hat: 0.0,
                correlated_verification_share: 0.0,
                drift_alerts_24h: 0,
            },
        );
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action, ThrottleActionKind::SetModeDegraded);
    }

    #[test]
    fn policy_throttle_receipts_are_snapshot_bound_and_receipted() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        let bundle = PolicyBundleConfig {
            autonomy_rules: vec![AutonomyPolicyRule {
                rule_id: "policy.autonomy.trigger.v1".to_string(),
                slice: PolicySliceRule {
                    category: Some("compute".to_string()),
                    tfb_class: Some(FeedbackLatencyClass::Short),
                    severity: Some(SeverityClass::High),
                },
                min_sv: Some(0.90),
                min_sv_effective: Some(0.90),
                max_xa_hat: None,
                max_delta_m_hat: None,
                max_correlated_share: None,
                max_drift_alerts_24h: None,
                actions: vec![ThrottleActionKind::SetModeDegraded],
            }],
            ..PolicyBundleConfig::default()
        };

        state.emit_policy_throttle_receipts_for_snapshot(
            "snapshot.economy:1762000060000",
            1_762_000_060_000,
            "sha256:snapshot",
            0.2,
            0.0,
            0.0,
            0.0,
            0.0,
            0,
            &bundle,
            "test.throttle",
        );

        let throttle_receipts = state
            .receipts
            .iter()
            .filter(|receipt| receipt.receipt_type == "economy.policy.throttle_action_applied.v1")
            .collect::<Vec<_>>();
        assert_eq!(throttle_receipts.len(), 1);
        let throttle = throttle_receipts[0];
        assert!(
            throttle
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "snapshot_ref")
        );
        assert!(
            throttle
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "policy_decision")
        );
        assert_eq!(
            throttle.hints.reason_code.as_deref(),
            Some(REASON_CODE_POLICY_THROTTLE_TRIGGERED)
        );
    }
}
