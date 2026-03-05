use crate::app_state::{
    ActiveJobRecord, JobDemandSource, JobHistoryReceiptRow, JobHistoryStatus, JobLifecycleStage,
    PaneLoadState,
};
use crate::economy_kernel_receipts::{
    Asset, AuthAssuranceLevel, DriftSignalSummary, EvidenceRef, FeedbackLatencyClass, Money,
    MoneyAmount, PolicyContext, ProvenanceAttestationKind, ProvenanceGrade, Receipt,
    ReceiptBuilder, ReceiptHints, SeverityClass, TraceContext,
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
const REASON_CODE_JOB_FAILED: &str = "JOB_FAILED";
const REASON_CODE_POLICY_PREFLIGHT_REJECTED: &str = "POLICY_PREFLIGHT_REJECTED";
const REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE: &str = "PAYMENT_POINTER_NON_AUTHORITATIVE";
const REASON_CODE_AUTH_ASSURANCE_INSUFFICIENT: &str = "AUTH_ASSURANCE_INSUFFICIENT";
const REASON_CODE_PROVENANCE_REQUIREMENTS_UNMET: &str = "PROVENANCE_REQUIREMENTS_UNMET";
const REASON_CODE_IDEMPOTENCY_CONFLICT: &str = "IDEMPOTENCY_CONFLICT";
const REASON_CODE_POLICY_THROTTLE_TRIGGERED: &str = "POLICY_THROTTLE_TRIGGERED";
const REASON_CODE_INCIDENT_REPORTED: &str = "INCIDENT_REPORTED";
const REASON_CODE_INCIDENT_UPDATED: &str = "INCIDENT_UPDATED";
const REASON_CODE_INCIDENT_RESOLVED: &str = "INCIDENT_RESOLVED";
const REASON_CODE_DRIFT_SIGNAL_EMITTED: &str = "DRIFT_SIGNAL_EMITTED";
const REASON_CODE_DRIFT_ALERT_RAISED: &str = "DRIFT_ALERT_RAISED";
const REASON_CODE_DRIFT_FALSE_POSITIVE_CONFIRMED: &str = "DRIFT_FALSE_POSITIVE_CONFIRMED";
const DEFAULT_PRICING_SNAPSHOT_ID: &str = "snapshot.economy:unavailable";
const DEFAULT_PRICING_SNAPSHOT_HASH: &str = "sha256:unavailable";
const DRIFT_WINDOW_MS: i64 = 86_400_000;

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
}

#[derive(Clone, Copy, Default)]
struct SnapshotPolicyMetrics {
    sv: f64,
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
    incident_taxonomy_registry: Vec<IncidentTaxonomyEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct WorkUnitMetadata {
    pub work_unit_id: String,
    pub category: String,
    pub tfb_class: FeedbackLatencyClass,
    pub severity: SeverityClass,
    pub verification_budget_hint_sats: u64,
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
}

struct LoadedReceiptState {
    receipts: Vec<Receipt>,
    work_units: BTreeMap<String, WorkUnitMetadata>,
    idempotency_index: BTreeMap<String, IdempotencyRecord>,
    incident_objects: Vec<IncidentObject>,
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
        let stage_pricing = if stage == JobLifecycleStage::Paid {
            Some(compute_liability_pricing_for_settlement(
                self.receipts.as_slice(),
                &policy_bundle,
                metadata.category.as_str(),
                metadata.tfb_class,
                metadata.severity,
                job.quoted_price_sats,
                metadata.verification_budget_hint_sats,
            ))
        } else {
            None
        };
        let (receipt_type, reason_code, status, policy_decision): (
            &'static str,
            Option<&'static str>,
            &'static str,
            PolicyDecision,
        ) = if stage == JobLifecycleStage::Paid && stage_auth_gate.is_some() {
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
        }))
        .with_evidence(evidence)
        .with_hints(hints)
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
        }))
        .with_evidence({
            let mut evidence = vec![EvidenceRef::new(
                "history_result_hash",
                format!("oa://earn/jobs/{}/result", row.job_id),
                normalize_digest(row.result_hash.as_str()),
            )];
            append_provenance_evidence_for_history(&mut evidence, row, stage);
            if let Some(pricing) = history_pricing.as_ref() {
                append_pricing_evidence(&mut evidence, pricing);
            }
            if stage == JobLifecycleStage::Paid && payment_pointer_authoritative {
                evidence.push(EvidenceRef::new(
                    "wallet_settlement_proof",
                    format!("oa://wallet/payments/{}", row.payment_pointer),
                    digest_for_text(row.payment_pointer.as_str()),
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
        rho: f64,
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
                "{snapshot_id}:{sv}:{rho}:{n}:{nv}:{delta_m_hat}:{xa_hat}:{correlated_verification_share}:{liability_premiums_collected_24h_sats}:{claims_paid_24h_sats}:{bonded_exposure_24h_sats}:{capital_reserves_24h_sats}:{loss_ratio}:{capital_coverage_ratio}:{drift_alerts_24h}:{}"
                ,
                drift_signal_digest_material(drift_signals.as_slice())
            )
            .as_str(),
        );
        let mut snapshot_metrics = EvidenceRef::new(
            "snapshot_metrics",
            format!("oa://economy/snapshots/{snapshot_id}/metrics"),
            snapshot_metrics_digest,
        );
        snapshot_metrics.meta.insert("sv".to_string(), json!(sv));
        snapshot_metrics.meta.insert("rho".to_string(), json!(rho));
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
            "rho": rho,
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
            },
        );
        normalize_work_units(&mut self.work_units);
        ResolvedWorkMetadata {
            category,
            tfb_class,
            severity,
            verification_budget_hint_sats,
        }
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

    fn persist_current_state(&mut self) -> Result<(), String> {
        persist_earn_kernel_receipts(
            self.receipt_file_path.as_path(),
            self.receipts.as_slice(),
            self.normalized_work_units().as_slice(),
            self.normalized_idempotency_records().as_slice(),
            self.incident_objects.as_slice(),
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
        let mut incident_receipts = self
            .receipts
            .iter()
            .filter(|receipt| receipt.receipt_type.starts_with("economy.incident."))
            .cloned()
            .collect::<Vec<_>>();
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
        }
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
        authentication_rules: vec![AuthenticationPolicyRule {
            rule_id: "policy.earn.auth.default.operator.v1".to_string(),
            slice: PolicySliceRule {
                category: Some("compute".to_string()),
                tfb_class: None,
                severity: None,
            },
            role: Some("operator".to_string()),
            min_auth_assurance: Some("authenticated".to_string()),
            require_personhood: false,
        }],
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
        rollback_rules: vec![RollbackPolicyRule {
            rule_id: "policy.earn.rollback.high_severity.v1".to_string(),
            slice: PolicySliceRule {
                category: Some("compute".to_string()),
                tfb_class: None,
                severity: Some(SeverityClass::High),
            },
            require_rollback_plan: true,
            allow_compensating_action_only: false,
        }],
        autonomy_rules: vec![
            AutonomyPolicyRule {
                rule_id: "policy.earn.autonomy.xa_elevated.v1".to_string(),
                slice: PolicySliceRule {
                    category: Some("compute".to_string()),
                    tfb_class: None,
                    severity: None,
                },
                min_sv: None,
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
    bundle
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
    evidence.push(EvidenceRef::new(
        "attestation:model_version",
        format!(
            "oa://attestations/model/{}",
            normalize_key(job.capability.as_str())
        ),
        digest_for_text(format!("model-version:{}:v1", job.capability).as_str()),
    ));
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
    evidence.push(EvidenceRef::new(
        "attestation:model_version",
        format!(
            "oa://attestations/model/history/{}",
            normalize_key(row.job_id.as_str())
        ),
        digest_for_text(format!("history-model:{}:v1", row.job_id).as_str()),
    ));
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
    let effective_liability_premium_bps = ((base_liability_premium_bps as f64) * dynamic_multiplier)
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
        return Err("incident evidence_digests cannot be empty".to_string());
    }
    rows.sort();
    rows.dedup();
    if rows.iter().any(|value| !value.starts_with("sha256:")) {
        return Err("incident evidence_digests must be sha256 digests".to_string());
    }
    Ok(rows)
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
        if metrics.sv < min_sv {
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

impl FeedbackLatencyClass {
    fn label(self) -> &'static str {
        match self {
            FeedbackLatencyClass::FeedbackLatencyClassUnspecified => "unspecified",
            FeedbackLatencyClass::Instant => "instant",
            FeedbackLatencyClass::Short => "short",
            FeedbackLatencyClass::Medium => "medium",
            FeedbackLatencyClass::Long => "long",
        }
    }
}

impl SeverityClass {
    fn label(self) -> &'static str {
        match self {
            SeverityClass::SeverityClassUnspecified => "unspecified",
            SeverityClass::Low => "low",
            SeverityClass::Medium => "medium",
            SeverityClass::High => "high",
            SeverityClass::Critical => "critical",
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
        incident_taxonomy_registry,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_ingress_request() -> JobInboxNetworkRequest {
        JobInboxNetworkRequest {
            request_id: "req-123".to_string(),
            requester: "npub1abc".to_string(),
            demand_source: JobDemandSource::OpenNetwork,
            request_kind: 5000,
            capability: "text_generation".to_string(),
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
            payout_sats: 42,
            result_hash: "sha256:abc".to_string(),
            payment_pointer: payment_pointer.to_string(),
            failure_reason: None,
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
            skill_scope_id: Some("skill.scope".to_string()),
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: Some("request-evt".to_string()),
            sa_tick_result_event_id: Some("result-evt".to_string()),
            sa_trajectory_session_id: Some("traj:123".to_string()),
            ac_envelope_event_id: Some("ac-env-1".to_string()),
            ac_settlement_event_id: Some("fb-evt".to_string()),
            ac_default_event_id: None,
            quoted_price_sats: 42,
            stage: JobLifecycleStage::Paid,
            invoice_id: None,
            payment_id: Some(payment_pointer.to_string()),
            failure_reason: None,
            events: Vec::new(),
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
        state
            .report_incident(
                fixture_incident_draft(linked_receipt_id.as_str(), "incident-export"),
                1_762_000_060_000,
                "test.incident.report",
            )
            .expect("incident report");

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
        assert!(public.incident_receipts.iter().all(|receipt| {
            receipt
                .evidence
                .iter()
                .all(|evidence| evidence.uri == "oa://redacted")
        }));
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
            vec![],
            "test.snapshot",
        );
        state.record_economy_snapshot_receipt(
            "snapshot.economy:1762000060000",
            1_762_000_060_000,
            "sha256:snapshot",
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
        );
        let second = compute_liability_pricing_for_settlement(
            state.receipts.as_slice(),
            &bundle,
            "compute",
            FeedbackLatencyClass::Short,
            SeverityClass::Low,
            10_000,
            1_000,
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
