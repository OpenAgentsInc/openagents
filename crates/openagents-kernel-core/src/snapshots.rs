use crate::receipts::{
    AuthAssuranceLevel, DriftSignalSummary, EvidenceRef, FeedbackLatencyClass, Money,
    ProvenanceGrade, Receipt, SeverityClass, VerificationTier,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct MetricKey {
    pub category: String,
    pub tfb_class: FeedbackLatencyClass,
    pub severity: SeverityClass,
    pub verification_tier: VerificationTier,
    pub verification_correlated: bool,
    pub provenance_grade: ProvenanceGrade,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SvBreakdownRow {
    pub key: MetricKey,
    pub total_work_units: u64,
    pub verified_work_units: u64,
    pub sv: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AuthAssuranceDistributionRow {
    pub level: AuthAssuranceLevel,
    pub count: u64,
    pub share: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct IncidentBucketRow {
    pub taxonomy_code: String,
    pub severity: SeverityClass,
    pub incident_reports_24h: u64,
    pub near_misses_24h: u64,
    pub incident_rate: f64,
    pub near_miss_rate: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SafetySignalBucketRow {
    pub taxonomy_code: String,
    pub severity: SeverityClass,
    pub signal_count_24h: u64,
    pub incident_signals_24h: u64,
    pub drift_signals_24h: u64,
    pub adverse_signals_24h: u64,
    pub signal_rate: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CertificationDistributionRow {
    pub category: String,
    pub tfb_class: FeedbackLatencyClass,
    pub min_severity: SeverityClass,
    pub max_severity: SeverityClass,
    pub certification_level: String,
    pub active_count: u64,
    pub revoked_count: u64,
    pub expired_count: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AnchorBackendStatusRow {
    pub anchor_backend: String,
    pub publications_24h: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OutcomeDistributionRow {
    pub category: String,
    pub tfb_class: FeedbackLatencyClass,
    pub severity: SeverityClass,
    pub verdict_outcome: String,
    pub settlement_outcome: String,
    pub claim_outcome: String,
    pub remedy_outcome: String,
    pub count_24h: u64,
    pub share_24h: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OutcomeKeyRateRow {
    pub category: String,
    pub tfb_class: FeedbackLatencyClass,
    pub severity: SeverityClass,
    pub entries_24h: u64,
    pub settlement_success_rate: f64,
    pub claim_rate: f64,
    pub remedy_rate: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct RollbackReasonCodeRow {
    pub reason_code: String,
    pub count_24h: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EconomySnapshot {
    pub snapshot_id: String,
    pub as_of_ms: i64,
    pub snapshot_hash: String,
    pub sv: f64,
    #[serde(default)]
    pub sv_effective: f64,
    pub rho: f64,
    #[serde(default)]
    pub rho_effective: f64,
    pub n: u64,
    pub nv: f64,
    pub delta_m_hat: f64,
    pub xa_hat: f64,
    pub correlated_verification_share: f64,
    pub provenance_p0_share: f64,
    pub provenance_p1_share: f64,
    pub provenance_p2_share: f64,
    pub provenance_p3_share: f64,
    pub auth_assurance_distribution: Vec<AuthAssuranceDistributionRow>,
    pub personhood_verified_share: f64,
    pub liability_premiums_collected_24h: Money,
    pub claims_paid_24h: Money,
    pub bonded_exposure_24h: Money,
    pub capital_reserves_24h: Money,
    pub loss_ratio: f64,
    pub capital_coverage_ratio: f64,
    #[serde(default)]
    pub drift_alerts_24h: u64,
    #[serde(default)]
    pub drift_signals: Vec<DriftSignalSummary>,
    #[serde(default)]
    pub top_drift_signals: Vec<DriftSignalSummary>,
    #[serde(default)]
    pub incident_buckets: Vec<IncidentBucketRow>,
    #[serde(default)]
    pub safety_signal_buckets: Vec<SafetySignalBucketRow>,
    #[serde(default)]
    pub certification_distribution: Vec<CertificationDistributionRow>,
    #[serde(default)]
    pub uncertified_block_count_24h: u64,
    #[serde(default)]
    pub uncertified_block_rate: f64,
    #[serde(default)]
    pub exportable_simulation_scenarios: u64,
    #[serde(default)]
    pub simulation_scenario_backlog: u64,
    #[serde(default)]
    pub anchor_publications_24h: u64,
    #[serde(default)]
    pub anchored_snapshots_24h: u64,
    #[serde(default)]
    pub anchor_backend_distribution: Vec<AnchorBackendStatusRow>,
    #[serde(default)]
    pub outcome_distribution: Vec<OutcomeDistributionRow>,
    #[serde(default)]
    pub outcome_key_rates: Vec<OutcomeKeyRateRow>,
    #[serde(default)]
    pub rollback_attempts_24h: u64,
    #[serde(default)]
    pub rollback_successes_24h: u64,
    #[serde(default)]
    pub rollback_success_rate: f64,
    #[serde(default)]
    pub top_rollback_reason_codes: Vec<RollbackReasonCodeRow>,
    #[serde(default)]
    pub compute_products_active: u64,
    #[serde(default)]
    pub compute_capacity_lots_open: u64,
    #[serde(default)]
    pub compute_capacity_lots_delivering: u64,
    #[serde(default)]
    pub compute_instruments_active: u64,
    #[serde(default)]
    pub compute_delivery_proofs_24h: u64,
    #[serde(default)]
    pub compute_delivery_quantity_24h: u64,
    #[serde(default)]
    pub compute_indices_published_24h: u64,
    #[serde(default)]
    pub compute_index_corrections_24h: u64,
    #[serde(default)]
    pub compute_index_thin_windows_24h: u64,
    #[serde(default)]
    pub compute_index_settlement_eligible_24h: u64,
    #[serde(default)]
    pub compute_index_quality_score_24h: f64,
    #[serde(default)]
    pub liquidity_quotes_active: u64,
    #[serde(default)]
    pub liquidity_route_plans_active: u64,
    #[serde(default)]
    pub liquidity_envelopes_open: u64,
    #[serde(default)]
    pub liquidity_settlements_24h: u64,
    #[serde(default)]
    pub liquidity_reserve_partitions_active: u64,
    #[serde(default)]
    pub liquidity_value_moved_24h: u64,
    #[serde(default)]
    pub risk_coverage_offers_open: u64,
    #[serde(default)]
    pub risk_coverage_bindings_active: u64,
    #[serde(default)]
    pub risk_prediction_positions_open: u64,
    #[serde(default)]
    pub risk_claims_open: u64,
    #[serde(default)]
    pub risk_signals_active: u64,
    #[serde(default)]
    pub risk_implied_fail_probability_bps: u32,
    #[serde(default)]
    pub risk_calibration_score: f64,
    #[serde(default)]
    pub risk_coverage_concentration_hhi: f64,
    #[serde(default)]
    pub audit_package_public_digest: String,
    #[serde(default)]
    pub audit_package_restricted_digest: String,
    pub sv_breakdown: Vec<SvBreakdownRow>,
    pub inputs: Vec<EvidenceRef>,
}

#[derive(Clone, Debug)]
pub struct SnapshotComputeResult {
    pub snapshot: EconomySnapshot,
    pub input_evidence: Vec<EvidenceRef>,
}

pub fn snapshot_inputs<'a>(
    snapshot: &'a EconomySnapshot,
    _receipts: &'a [Receipt],
) -> &'a [EvidenceRef] {
    snapshot.inputs.as_slice()
}
