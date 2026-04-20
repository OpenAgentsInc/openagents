use crate::app_state::PaneLoadState;
use crate::economy_kernel_receipts::{
    Asset, AuthAssuranceLevel, DriftSignalSummary, EvidenceRef, FeedbackLatencyClass, Money,
    MoneyAmount, ProvenanceGrade, Receipt, SeverityClass, VerificationTier,
};
use bitcoin::hashes::{Hash, sha256};
pub use openagents_kernel_core::snapshots::{
    AnchorBackendStatusRow, AuthAssuranceDistributionRow, CertificationDistributionRow,
    EconomySnapshot, IncidentBucketRow, MetricKey, OutcomeDistributionRow, OutcomeKeyRateRow,
    RollbackReasonCodeRow, SafetySignalBucketRow, SnapshotComputeResult, SvBreakdownRow,
};
use openagents_kernel_core::time::{floor_to_minute_utc, snapshot_id_for_minute};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const ECONOMY_SNAPSHOT_SCHEMA_VERSION: u16 = 1;
const ECONOMY_SNAPSHOT_STREAM_ID: &str = "stream.economy_snapshots.v1";
const ECONOMY_SNAPSHOT_WINDOW_MS: i64 = 86_400_000;
const ECONOMY_SNAPSHOT_RETENTION_LIMIT: usize = 10_080;
const DRIFT_SIGNAL_TOP_LIMIT: usize = 5;
const DRIFT_THRESHOLD_SV_FLOOR: f64 = 0.70;
const DRIFT_THRESHOLD_CORRELATED_SHARE_MAX: f64 = 0.55;
const DRIFT_THRESHOLD_PAYOUT_SUCCESS_MIN: f64 = 0.80;
const DRIFT_THRESHOLD_DISPUTE_CLAIM_SHARE_MAX: f64 = 0.08;
const DRIFT_THRESHOLD_INCIDENT_SHARE_MAX: f64 = 0.05;
const DRIFT_THRESHOLD_XA_MAX: f64 = 0.25;
const SV_EFFECTIVE_LINEAGE_OFFSET_WEIGHT: f64 = 0.5;
const REASON_CODE_DIGITAL_BORDER_BLOCK_UNCERTIFIED: &str = "DIGITAL_BORDER_BLOCK_UNCERTIFIED";
const REASON_CODE_CERTIFICATION_REQUIRED: &str = "CERTIFICATION_REQUIRED";

#[derive(Debug, Serialize, Deserialize)]
struct EconomySnapshotDocumentV1 {
    schema_version: u16,
    stream_id: String,
    snapshots: Vec<EconomySnapshot>,
}

pub struct EconomySnapshotState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub latest_snapshot_id: Option<String>,
    pub latest_snapshot_hash: Option<String>,
    pub latest_snapshot: Option<EconomySnapshot>,
    pub subscription_seq: u64,
    pub snapshots: Vec<EconomySnapshot>,
    snapshot_file_path: PathBuf,
}

impl Default for EconomySnapshotState {
    fn default() -> Self {
        let snapshot_file_path = economy_snapshot_file_path();
        Self::from_snapshot_file_path(snapshot_file_path)
    }
}

impl EconomySnapshotState {
    fn from_snapshot_file_path(snapshot_file_path: PathBuf) -> Self {
        let (snapshots, load_state, last_error, last_action) =
            match load_economy_snapshots(snapshot_file_path.as_path()) {
                Ok(snapshots) => (
                    snapshots,
                    PaneLoadState::Ready,
                    None,
                    Some("Loaded economy snapshot stream".to_string()),
                ),
                Err(error) => (
                    Vec::new(),
                    PaneLoadState::Error,
                    Some(error),
                    Some("Economy snapshot stream load failed".to_string()),
                ),
            };
        let latest_snapshot = snapshots.first().cloned();
        let latest_snapshot_id = latest_snapshot
            .as_ref()
            .map(|snapshot| snapshot.snapshot_id.clone());
        let latest_snapshot_hash = latest_snapshot
            .as_ref()
            .map(|snapshot| snapshot.snapshot_hash.clone());
        Self {
            load_state,
            last_error,
            last_action,
            latest_snapshot_id,
            latest_snapshot_hash,
            latest_snapshot,
            subscription_seq: 0,
            snapshots,
            snapshot_file_path,
        }
    }

    #[cfg(test)]
    fn from_snapshot_path_for_tests(snapshot_file_path: PathBuf) -> Self {
        Self::from_snapshot_file_path(snapshot_file_path)
    }

    pub fn get_snapshot(&self, snapshot_id: &str) -> Option<&EconomySnapshot> {
        self.snapshots
            .iter()
            .find(|snapshot| snapshot.snapshot_id == snapshot_id)
    }

    pub fn compute_minute_snapshot(
        &mut self,
        now_epoch_ms: i64,
        receipts: &[Receipt],
    ) -> Option<SnapshotComputeResult> {
        let as_of_ms = floor_to_minute_utc(now_epoch_ms.max(0));
        let snapshot_id = snapshot_id_for_minute(as_of_ms);
        if let Some(existing) = self.get_snapshot(snapshot_id.as_str()).cloned() {
            self.set_latest(existing, false);
            return None;
        }

        let snapshot = match build_snapshot(snapshot_id.as_str(), as_of_ms, receipts) {
            Ok(value) => value,
            Err(error) => {
                self.last_error = Some(error);
                self.last_action = Some("Economy snapshot compute failed".to_string());
                self.load_state = PaneLoadState::Error;
                return None;
            }
        };
        let result = SnapshotComputeResult {
            input_evidence: snapshot.inputs.clone(),
            snapshot: snapshot.clone(),
        };

        self.snapshots.push(snapshot.clone());
        self.snapshots = normalize_snapshots(std::mem::take(&mut self.snapshots));
        if let Err(error) =
            persist_economy_snapshots(self.snapshot_file_path.as_path(), self.snapshots.as_slice())
        {
            self.last_error = Some(error);
            self.last_action = Some("Economy snapshot persist failed".to_string());
            self.load_state = PaneLoadState::Error;
            return None;
        }

        self.set_latest(snapshot.clone(), true);
        self.last_error = None;
        self.last_action = Some(format!(
            "Computed minute snapshot {} ({})",
            snapshot.snapshot_id, snapshot.snapshot_hash
        ));
        self.load_state = PaneLoadState::Ready;
        Some(result)
    }

    pub fn apply_authoritative_snapshot(
        &mut self,
        snapshot: EconomySnapshot,
        source_tag: &str,
    ) -> bool {
        let changed = self
            .snapshots
            .iter()
            .find(|existing| existing.snapshot_id == snapshot.snapshot_id)
            .is_none_or(|existing| existing.snapshot_hash != snapshot.snapshot_hash);

        self.snapshots.retain(|existing| {
            existing.snapshot_id != snapshot.snapshot_id
                || existing.snapshot_hash == snapshot.snapshot_hash
        });
        if changed {
            self.snapshots.push(snapshot.clone());
            self.snapshots = normalize_snapshots(std::mem::take(&mut self.snapshots));
            if let Err(error) = persist_economy_snapshots(
                self.snapshot_file_path.as_path(),
                self.snapshots.as_slice(),
            ) {
                self.last_error = Some(error);
                self.last_action = Some("Economy snapshot persist failed".to_string());
                self.load_state = PaneLoadState::Error;
                return false;
            }
        }

        self.set_latest(snapshot.clone(), changed);
        self.last_error = None;
        self.last_action = Some(format!(
            "Projected authoritative snapshot {} via {}",
            snapshot.snapshot_id, source_tag
        ));
        self.load_state = PaneLoadState::Ready;
        changed
    }

    fn set_latest(&mut self, snapshot: EconomySnapshot, from_new_compute: bool) {
        let changed = self
            .latest_snapshot
            .as_ref()
            .is_none_or(|latest| latest.snapshot_id != snapshot.snapshot_id);
        if changed || from_new_compute {
            self.subscription_seq = self.subscription_seq.saturating_add(1);
        }
        self.latest_snapshot_id = Some(snapshot.snapshot_id.clone());
        self.latest_snapshot_hash = Some(snapshot.snapshot_hash.clone());
        self.latest_snapshot = Some(snapshot);
    }
}

fn build_snapshot(
    snapshot_id: &str,
    as_of_ms: i64,
    receipts: &[Receipt],
) -> Result<EconomySnapshot, String> {
    let window_end_ms = as_of_ms.saturating_add(60_000);
    let window_start_ms = window_end_ms.saturating_sub(ECONOMY_SNAPSHOT_WINDOW_MS);
    let mut scoped_receipts = receipts
        .iter()
        .filter(|receipt| {
            receipt.created_at_ms < window_end_ms && receipt.created_at_ms >= window_start_ms
        })
        .collect::<Vec<_>>();
    scoped_receipts.sort_by(|lhs, rhs| {
        lhs.created_at_ms
            .cmp(&rhs.created_at_ms)
            .then_with(|| lhs.receipt_id.cmp(&rhs.receipt_id))
    });

    let mut terminal_by_work_unit = BTreeMap::<String, &Receipt>::new();
    for receipt in scoped_receipts
        .iter()
        .copied()
        .filter(|receipt| is_terminal_work_unit_receipt(receipt))
    {
        let work_unit_id = receipt
            .trace
            .work_unit_id
            .clone()
            .unwrap_or_else(|| format!("work_unit:{}", receipt.receipt_id));
        let keep_new = terminal_by_work_unit
            .get(work_unit_id.as_str())
            .is_none_or(|existing| {
                receipt.created_at_ms > existing.created_at_ms
                    || (receipt.created_at_ms == existing.created_at_ms
                        && receipt.receipt_id > existing.receipt_id)
            });
        if keep_new {
            terminal_by_work_unit.insert(work_unit_id, receipt);
        }
    }

    let mut breakdown = BTreeMap::<MetricKey, (u64, u64)>::new();
    let mut total_work_units = 0u64;
    let mut verified_work_units = 0u64;
    let mut settlement_terminal_count = 0u64;
    let mut correlated_verified = 0u64;
    let mut provenance_counts = [0u64; 4];
    let mut auth_assurance_counts = BTreeMap::<AuthAssuranceLevel, u64>::new();
    let mut personhood_verified = 0u64;
    let mut liability_premiums_collected_sats = 0u64;
    let mut bonded_exposure_sats = 0u64;

    for receipt in terminal_by_work_unit.values() {
        total_work_units = total_work_units.saturating_add(1);
        let verified = is_verified_terminal_receipt(receipt);
        if verified {
            verified_work_units = verified_work_units.saturating_add(1);
            if is_settlement_terminal_receipt(receipt) {
                settlement_terminal_count = settlement_terminal_count.saturating_add(1);
            }
            if receipt.hints.verification_correlated.unwrap_or(false) {
                correlated_verified = correlated_verified.saturating_add(1);
            }
        } else if is_settlement_terminal_receipt(receipt) {
            settlement_terminal_count = settlement_terminal_count.saturating_add(1);
        }
        provenance_counts[provenance_bucket(receipt.hints.provenance_grade)] =
            provenance_counts[provenance_bucket(receipt.hints.provenance_grade)].saturating_add(1);
        let auth_level = receipt
            .hints
            .auth_assurance_level
            .unwrap_or(AuthAssuranceLevel::AuthAssuranceLevelUnspecified);
        let auth_entry = auth_assurance_counts.entry(auth_level).or_insert(0);
        *auth_entry = auth_entry.saturating_add(1);
        if receipt.hints.personhood_proved.unwrap_or(false) {
            personhood_verified = personhood_verified.saturating_add(1);
        }
        bonded_exposure_sats =
            bonded_exposure_sats.saturating_add(money_as_sats(receipt.hints.notional.as_ref()));
        liability_premiums_collected_sats = liability_premiums_collected_sats
            .saturating_add(money_as_sats(receipt.hints.liability_premium.as_ref()));

        let key = metric_key_for_receipt(receipt);
        let entry = breakdown.entry(key).or_insert((0, 0));
        entry.0 = entry.0.saturating_add(1);
        if verified {
            entry.1 = entry.1.saturating_add(1);
        }
    }

    let sv_breakdown = breakdown
        .into_iter()
        .map(|(key, (total, verified))| SvBreakdownRow {
            key,
            total_work_units: total,
            verified_work_units: verified,
            sv: ratio(verified, total),
        })
        .collect::<Vec<_>>();
    let sv = ratio(verified_work_units, total_work_units);
    let correlated_verification_share = ratio(correlated_verified, verified_work_units);
    let provenance_p0_share = ratio(provenance_counts[0], total_work_units);
    let provenance_p1_share = ratio(provenance_counts[1], total_work_units);
    let provenance_p2_share = ratio(provenance_counts[2], total_work_units);
    let provenance_p3_share = ratio(provenance_counts[3], total_work_units);
    let sv_effective = correlation_adjusted_sv_effective(
        sv,
        correlated_verification_share,
        provenance_p2_share,
        provenance_p3_share,
    );
    let rho = sv;
    let rho_effective = sv_effective;
    let nv = rho * (total_work_units as f64);
    let auth_assurance_distribution = auth_assurance_counts
        .into_iter()
        .map(|(level, count)| AuthAssuranceDistributionRow {
            level,
            count,
            share: ratio(count, total_work_units),
        })
        .collect::<Vec<_>>();
    let personhood_verified_share = ratio(personhood_verified, total_work_units);
    let terminal_receipts = terminal_by_work_unit.values().copied().collect::<Vec<_>>();
    let long_feedback_terminal_count = terminal_receipts
        .iter()
        .filter(|receipt| is_long_feedback_terminal_receipt(receipt))
        .count() as u64;
    let adverse_terminal_count = terminal_receipts
        .iter()
        .filter(|receipt| is_adverse_terminal_receipt(receipt))
        .count() as u64;
    let (total_severity_weight, unverified_severity_weight, adverse_severity_weight) =
        severity_weight_totals(terminal_receipts.as_slice());
    let incident_signal_count = scoped_receipts
        .iter()
        .copied()
        .filter(|receipt| is_incident_or_near_miss_receipt(receipt))
        .count() as u64;
    let breaker_signal_count = scoped_receipts
        .iter()
        .copied()
        .filter(|receipt| is_breaker_or_throttle_receipt(receipt))
        .count() as u64;
    let rollback_receipts = scoped_receipts
        .iter()
        .copied()
        .filter(|receipt| is_rollback_receipt(receipt))
        .collect::<Vec<_>>();
    let rollback_signal_count = rollback_receipts.len() as u64;
    let rollback_successes_24h = rollback_receipts
        .iter()
        .copied()
        .filter(|receipt| is_successful_rollback_receipt(receipt))
        .count() as u64;
    let rollback_success_rate = ratio(rollback_successes_24h, rollback_signal_count);
    let mut rollback_reason_counts = BTreeMap::<String, u64>::new();
    for receipt in &rollback_receipts {
        let redacted_reason_code = redacted_rollback_reason_code(receipt);
        let entry = rollback_reason_counts
            .entry(redacted_reason_code)
            .or_insert(0);
        *entry = entry.saturating_add(1);
    }
    let mut top_rollback_reason_codes = rollback_reason_counts
        .into_iter()
        .map(|(reason_code, count_24h)| RollbackReasonCodeRow {
            reason_code,
            count_24h,
        })
        .collect::<Vec<_>>();
    top_rollback_reason_codes.sort_by(|lhs, rhs| {
        rhs.count_24h
            .cmp(&lhs.count_24h)
            .then_with(|| lhs.reason_code.cmp(&rhs.reason_code))
    });
    top_rollback_reason_codes.truncate(DRIFT_SIGNAL_TOP_LIMIT);
    let claims_paid_sats = scoped_receipts
        .iter()
        .copied()
        .filter(|receipt| is_claim_paid_receipt(receipt))
        .map(|receipt| money_as_sats(receipt.hints.notional.as_ref()))
        .fold(0u64, u64::saturating_add);
    let claims_signal_count = scoped_receipts
        .iter()
        .copied()
        .filter(|receipt| is_claim_paid_receipt(receipt))
        .count() as u64;
    let dispute_or_claim_count = scoped_receipts
        .iter()
        .copied()
        .filter(|receipt| is_dispute_or_claim_receipt(receipt))
        .count() as u64;
    let mut incident_bucket_counts = BTreeMap::<(String, SeverityClass), (u64, u64)>::new();
    for receipt in scoped_receipts
        .iter()
        .copied()
        .filter(|receipt| is_incident_receipt(receipt))
    {
        let taxonomy_code =
            incident_taxonomy_code_for_receipt(receipt).unwrap_or_else(|| "unknown".to_string());
        let severity = incident_severity_for_receipt(receipt);
        let key = (taxonomy_code, severity);
        let entry = incident_bucket_counts.entry(key).or_insert((0, 0));
        match incident_kind_for_receipt(receipt) {
            IncidentReceiptKind::NearMiss => {
                entry.1 = entry.1.saturating_add(1);
            }
            IncidentReceiptKind::Incident | IncidentReceiptKind::GroundTruthCase => {
                entry.0 = entry.0.saturating_add(1);
            }
            IncidentReceiptKind::Unknown => {}
        }
    }
    let mut incident_buckets = incident_bucket_counts
        .into_iter()
        .map(
            |((taxonomy_code, severity), (incident_reports_24h, near_misses_24h))| {
                IncidentBucketRow {
                    taxonomy_code,
                    severity,
                    incident_reports_24h,
                    near_misses_24h,
                    incident_rate: ratio(incident_reports_24h, total_work_units),
                    near_miss_rate: ratio(near_misses_24h, total_work_units),
                }
            },
        )
        .collect::<Vec<_>>();
    incident_buckets.sort_by(|lhs, rhs| {
        lhs.taxonomy_code
            .cmp(&rhs.taxonomy_code)
            .then_with(|| lhs.severity.cmp(&rhs.severity))
    });
    let mut safety_signal_counts = BTreeMap::<(String, SeverityClass), (u64, u64, u64, u64)>::new();
    for receipt in scoped_receipts.iter().copied() {
        let Some(meta) = safety_signal_meta_for_receipt(receipt) else {
            continue;
        };
        let key = (meta.taxonomy_code, meta.severity);
        let entry = safety_signal_counts.entry(key).or_insert((0, 0, 0, 0));
        entry.0 = entry.0.saturating_add(1);
        match meta.signal_class {
            SafetySignalClass::Incident => {
                entry.1 = entry.1.saturating_add(1);
            }
            SafetySignalClass::Drift => {
                entry.2 = entry.2.saturating_add(1);
            }
            SafetySignalClass::Adverse => {
                entry.3 = entry.3.saturating_add(1);
            }
        }
    }
    let mut safety_signal_buckets = safety_signal_counts
        .into_iter()
        .map(
            |(
                (taxonomy_code, severity),
                (signal_count_24h, incident_signals_24h, drift_signals_24h, adverse_signals_24h),
            )| SafetySignalBucketRow {
                taxonomy_code,
                severity,
                signal_count_24h,
                incident_signals_24h,
                drift_signals_24h,
                adverse_signals_24h,
                signal_rate: ratio(signal_count_24h, total_work_units),
            },
        )
        .collect::<Vec<_>>();
    safety_signal_buckets.sort_by(|lhs, rhs| {
        lhs.taxonomy_code
            .cmp(&rhs.taxonomy_code)
            .then_with(|| lhs.severity.cmp(&rhs.severity))
    });
    let certification_distribution = build_certification_distribution(receipts, as_of_ms);
    let uncertified_block_count_24h = terminal_receipts
        .iter()
        .copied()
        .filter(|receipt| is_uncertified_border_block_receipt(receipt))
        .count() as u64;
    let uncertified_block_rate = ratio(uncertified_block_count_24h, total_work_units);
    let (exportable_simulation_scenarios, simulation_scenario_backlog) =
        simulation_scenario_backlog_counts(receipts, as_of_ms);
    let anchor_publications_24h = scoped_receipts
        .iter()
        .copied()
        .filter(|receipt| is_anchor_publication_receipt(receipt))
        .count() as u64;
    let mut anchored_snapshots = BTreeSet::<String>::new();
    let mut anchor_backend_counts = BTreeMap::<String, u64>::new();
    for receipt in scoped_receipts
        .iter()
        .copied()
        .filter(|receipt| is_anchor_publication_receipt(receipt))
    {
        let backend = anchor_backend_for_receipt(receipt).unwrap_or_else(|| "unknown".to_string());
        let entry = anchor_backend_counts.entry(backend).or_insert(0);
        *entry = entry.saturating_add(1);
        if let Some(snapshot_id) = anchor_snapshot_id_for_receipt(receipt) {
            anchored_snapshots.insert(snapshot_id);
        }
    }
    let anchored_snapshots_24h = anchored_snapshots.len() as u64;
    let mut anchor_backend_distribution = anchor_backend_counts
        .into_iter()
        .map(
            |(anchor_backend, publications_24h)| AnchorBackendStatusRow {
                anchor_backend,
                publications_24h,
            },
        )
        .collect::<Vec<_>>();
    anchor_backend_distribution.sort_by(|lhs, rhs| lhs.anchor_backend.cmp(&rhs.anchor_backend));
    let mut outcome_distribution_counts = BTreeMap::<
        (
            String,
            FeedbackLatencyClass,
            SeverityClass,
            String,
            String,
            String,
            String,
        ),
        u64,
    >::new();
    let mut outcome_key_rate_counts =
        BTreeMap::<(String, FeedbackLatencyClass, SeverityClass), (u64, u64, u64, u64)>::new();
    for receipt in scoped_receipts
        .iter()
        .copied()
        .filter(|receipt| is_outcome_registry_receipt(receipt))
    {
        let Some(meta) = outcome_meta_for_receipt(receipt) else {
            continue;
        };
        let category = receipt
            .hints
            .category
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let tfb_class = receipt
            .hints
            .tfb_class
            .unwrap_or(FeedbackLatencyClass::FeedbackLatencyClassUnspecified);
        let severity = receipt
            .hints
            .severity
            .unwrap_or(SeverityClass::SeverityClassUnspecified);
        let claim_outcome = meta
            .claim_outcome
            .clone()
            .unwrap_or_else(|| "none".to_string());
        let remedy_outcome = meta
            .remedy_outcome
            .clone()
            .unwrap_or_else(|| "none".to_string());
        let distribution_key = (
            category.clone(),
            tfb_class,
            severity,
            meta.verdict_outcome.clone(),
            meta.settlement_outcome.clone(),
            claim_outcome,
            remedy_outcome,
        );
        let distribution_entry = outcome_distribution_counts
            .entry(distribution_key)
            .or_insert(0);
        *distribution_entry = distribution_entry.saturating_add(1);

        let rates_entry = outcome_key_rate_counts
            .entry((category, tfb_class, severity))
            .or_insert((0, 0, 0, 0));
        rates_entry.0 = rates_entry.0.saturating_add(1);
        if is_success_outcome_label(meta.settlement_outcome.as_str()) {
            rates_entry.1 = rates_entry.1.saturating_add(1);
        }
        if meta
            .claim_outcome
            .as_deref()
            .is_some_and(is_present_outcome_label)
        {
            rates_entry.2 = rates_entry.2.saturating_add(1);
        }
        if meta
            .remedy_outcome
            .as_deref()
            .is_some_and(is_present_outcome_label)
        {
            rates_entry.3 = rates_entry.3.saturating_add(1);
        }
    }
    let total_outcome_entries = outcome_distribution_counts
        .values()
        .copied()
        .fold(0u64, u64::saturating_add);
    let mut outcome_distribution = outcome_distribution_counts
        .into_iter()
        .map(
            |(
                (
                    category,
                    tfb_class,
                    severity,
                    verdict_outcome,
                    settlement_outcome,
                    claim_outcome,
                    remedy_outcome,
                ),
                count_24h,
            )| OutcomeDistributionRow {
                category,
                tfb_class,
                severity,
                verdict_outcome,
                settlement_outcome,
                claim_outcome,
                remedy_outcome,
                count_24h,
                share_24h: ratio(count_24h, total_outcome_entries),
            },
        )
        .collect::<Vec<_>>();
    outcome_distribution.sort_by(|lhs, rhs| {
        lhs.category
            .cmp(&rhs.category)
            .then_with(|| lhs.tfb_class.cmp(&rhs.tfb_class))
            .then_with(|| lhs.severity.cmp(&rhs.severity))
            .then_with(|| lhs.verdict_outcome.cmp(&rhs.verdict_outcome))
            .then_with(|| lhs.settlement_outcome.cmp(&rhs.settlement_outcome))
            .then_with(|| lhs.claim_outcome.cmp(&rhs.claim_outcome))
            .then_with(|| lhs.remedy_outcome.cmp(&rhs.remedy_outcome))
    });
    let mut outcome_key_rates = outcome_key_rate_counts
        .into_iter()
        .map(
            |((category, tfb_class, severity), (entries_24h, settled, claims, remedies))| {
                OutcomeKeyRateRow {
                    category,
                    tfb_class,
                    severity,
                    entries_24h,
                    settlement_success_rate: ratio(settled, entries_24h),
                    claim_rate: ratio(claims, entries_24h),
                    remedy_rate: ratio(remedies, entries_24h),
                }
            },
        )
        .collect::<Vec<_>>();
    outcome_key_rates.sort_by(|lhs, rhs| {
        lhs.category
            .cmp(&rhs.category)
            .then_with(|| lhs.tfb_class.cmp(&rhs.tfb_class))
            .then_with(|| lhs.severity.cmp(&rhs.severity))
    });
    let total_scoped_receipts = scoped_receipts.len() as u64;
    let long_feedback_share = ratio(long_feedback_terminal_count, total_work_units);
    let unverified_share = ratio(
        total_work_units.saturating_sub(verified_work_units),
        total_work_units,
    );
    let adverse_terminal_share = ratio(adverse_terminal_count, total_work_units);
    let incident_signal_share = ratio(
        incident_signal_count
            .saturating_add(rollback_signal_count)
            .saturating_add(claims_signal_count),
        total_scoped_receipts,
    );
    let unverified_severity_share = ratio_f64(unverified_severity_weight, total_severity_weight);
    let adverse_severity_share = ratio_f64(adverse_severity_weight, total_severity_weight);
    let breaker_signal_share = ratio(breaker_signal_count, total_scoped_receipts);
    let claim_signal_share = ratio(claims_signal_count, total_scoped_receipts);
    let rollback_signal_share = ratio(rollback_signal_count, total_scoped_receipts);
    let quote_variance_share = quote_delivery_variance_share(scoped_receipts.as_slice());
    let payout_success_share = ratio(settlement_terminal_count, total_work_units);
    let dispute_or_claim_share = ratio(dispute_or_claim_count, total_scoped_receipts);
    let delta_m_hat = clamp_unit_interval(
        (0.30 * long_feedback_share)
            + (0.25 * correlated_verification_share)
            + (0.20 * unverified_share)
            + (0.15 * adverse_terminal_share)
            + (0.10 * incident_signal_share),
    );
    let xa_hat = clamp_unit_interval(
        (0.40 * unverified_severity_share)
            + (0.20 * adverse_severity_share)
            + (0.15 * breaker_signal_share)
            + (0.10 * claim_signal_share)
            + (0.10 * rollback_signal_share)
            + (0.05 * quote_variance_share),
    );
    let incident_alert_count = incident_signal_count
        .saturating_add(rollback_signal_count)
        .saturating_add(claims_signal_count);
    let (drift_alerts_24h, drift_signals, top_drift_signals) = build_drift_signals(
        sv,
        correlated_verification_share,
        payout_success_share,
        dispute_or_claim_share,
        incident_signal_share,
        xa_hat,
        total_work_units.saturating_sub(verified_work_units),
        correlated_verified,
        adverse_terminal_count,
        dispute_or_claim_count,
        incident_alert_count,
    );
    let capital_reserves_sats = liability_premiums_collected_sats.saturating_sub(claims_paid_sats);
    let loss_ratio = if liability_premiums_collected_sats == 0 {
        0.0
    } else {
        claims_paid_sats as f64 / liability_premiums_collected_sats as f64
    };
    let capital_coverage_ratio = if bonded_exposure_sats == 0 {
        1.0
    } else {
        capital_reserves_sats as f64 / bonded_exposure_sats as f64
    };
    let audit_package_public_digest =
        snapshot_audit_package_digest(scoped_receipts.as_slice(), "public");
    let audit_package_restricted_digest =
        snapshot_audit_package_digest(scoped_receipts.as_slice(), "restricted");

    let inputs = snapshot_input_evidence(window_start_ms, as_of_ms, scoped_receipts.as_slice());
    let snapshot_hash = snapshot_hash_for(
        snapshot_id,
        as_of_ms,
        sv,
        sv_effective,
        rho,
        rho_effective,
        total_work_units,
        nv,
        delta_m_hat,
        xa_hat,
        correlated_verification_share,
        provenance_p0_share,
        provenance_p1_share,
        provenance_p2_share,
        provenance_p3_share,
        personhood_verified_share,
        liability_premiums_collected_sats,
        claims_paid_sats,
        bonded_exposure_sats,
        capital_reserves_sats,
        loss_ratio,
        capital_coverage_ratio,
        drift_alerts_24h,
        drift_signals.as_slice(),
        top_drift_signals.as_slice(),
        incident_buckets.as_slice(),
        safety_signal_buckets.as_slice(),
        certification_distribution.as_slice(),
        uncertified_block_count_24h,
        uncertified_block_rate,
        exportable_simulation_scenarios,
        simulation_scenario_backlog,
        anchor_publications_24h,
        anchored_snapshots_24h,
        anchor_backend_distribution.as_slice(),
        outcome_distribution.as_slice(),
        outcome_key_rates.as_slice(),
        rollback_signal_count,
        rollback_successes_24h,
        rollback_success_rate,
        top_rollback_reason_codes.as_slice(),
        audit_package_public_digest.as_str(),
        audit_package_restricted_digest.as_str(),
        auth_assurance_distribution.as_slice(),
        sv_breakdown.as_slice(),
        inputs.as_slice(),
    )?;

    Ok(EconomySnapshot {
        snapshot_id: snapshot_id.to_string(),
        as_of_ms,
        snapshot_hash,
        sv,
        sv_effective,
        rho,
        rho_effective,
        n: total_work_units,
        nv,
        delta_m_hat,
        xa_hat,
        correlated_verification_share,
        provenance_p0_share,
        provenance_p1_share,
        provenance_p2_share,
        provenance_p3_share,
        auth_assurance_distribution,
        personhood_verified_share,
        liability_premiums_collected_24h: btc_sats_money(liability_premiums_collected_sats),
        claims_paid_24h: btc_sats_money(claims_paid_sats),
        bonded_exposure_24h: btc_sats_money(bonded_exposure_sats),
        capital_reserves_24h: btc_sats_money(capital_reserves_sats),
        loss_ratio,
        capital_coverage_ratio,
        drift_alerts_24h,
        drift_signals,
        top_drift_signals,
        incident_buckets,
        safety_signal_buckets,
        certification_distribution,
        uncertified_block_count_24h,
        uncertified_block_rate,
        exportable_simulation_scenarios,
        simulation_scenario_backlog,
        anchor_publications_24h,
        anchored_snapshots_24h,
        anchor_backend_distribution,
        outcome_distribution,
        outcome_key_rates,
        rollback_attempts_24h: rollback_signal_count,
        rollback_successes_24h,
        rollback_success_rate,
        top_rollback_reason_codes,
        compute_products_active: 0,
        compute_capacity_lots_open: 0,
        compute_capacity_lots_delivering: 0,
        compute_instruments_active: 0,
        compute_inventory_quantity_open: 0,
        compute_inventory_quantity_reserved: 0,
        compute_inventory_quantity_delivering: 0,
        compute_delivery_proofs_24h: 0,
        compute_delivery_quantity_24h: 0,
        compute_delivery_rejections_24h: 0,
        compute_delivery_variances_24h: 0,
        compute_validator_challenges_open: 0,
        compute_validator_challenges_queued: 0,
        compute_validator_challenges_verified_24h: 0,
        compute_validator_challenges_rejected_24h: 0,
        compute_validator_challenges_timed_out_24h: 0,
        compute_delivery_accept_rate_24h: 0.0,
        compute_fill_ratio_24h: 0.0,
        compute_priced_instruments_24h: 0,
        compute_indices_published_24h: 0,
        compute_index_corrections_24h: 0,
        compute_index_thin_windows_24h: 0,
        compute_index_settlement_eligible_24h: 0,
        compute_index_quality_score_24h: 0.0,
        compute_active_provider_count: 0,
        compute_provider_concentration_hhi: 0.0,
        compute_forward_physical_instruments_active: 0,
        compute_forward_physical_open_quantity: 0,
        compute_forward_physical_defaults_24h: 0,
        compute_future_cash_instruments_active: 0,
        compute_future_cash_open_interest: 0,
        compute_future_cash_cash_settlements_24h: 0,
        compute_future_cash_cash_flow_24h: 0,
        compute_future_cash_defaults_24h: 0,
        compute_future_cash_collateral_shortfall_24h: 0,
        compute_structured_instruments_active: 0,
        compute_structured_instruments_closed_24h: 0,
        compute_max_buyer_concentration_share: 0.0,
        compute_paper_to_physical_ratio: 0.0,
        compute_deliverable_coverage_ratio: 0.0,
        compute_breakers_tripped: 0,
        compute_breakers_guarded: 0,
        compute_breaker_states: Vec::new(),
        compute_rollout_gates: Vec::new(),
        compute_truth_labels: Vec::new(),
        compute_reconciliation_gap_24h: 0,
        compute_policy_bundle_id: String::new(),
        compute_policy_version: String::new(),
        liquidity_quotes_active: 0,
        liquidity_route_plans_active: 0,
        liquidity_envelopes_open: 0,
        liquidity_settlements_24h: 0,
        liquidity_reserve_partitions_active: 0,
        liquidity_value_moved_24h: 0,
        risk_coverage_offers_open: 0,
        risk_coverage_bindings_active: 0,
        risk_prediction_positions_open: 0,
        risk_claims_open: 0,
        risk_signals_active: 0,
        risk_implied_fail_probability_bps: 0,
        risk_calibration_score: 0.0,
        risk_coverage_concentration_hhi: 0.0,
        audit_package_public_digest,
        audit_package_restricted_digest,
        sv_breakdown,
        inputs,
    })
}

fn is_terminal_work_unit_receipt(receipt: &Receipt) -> bool {
    matches!(
        receipt.receipt_type.as_str(),
        "earn.job.settlement_observed.v1" | "earn.job.withheld.v1" | "earn.job.failed.v1"
    )
}

fn is_settlement_terminal_receipt(receipt: &Receipt) -> bool {
    receipt.receipt_type == "earn.job.settlement_observed.v1"
}

fn is_verified_terminal_receipt(receipt: &Receipt) -> bool {
    is_settlement_terminal_receipt(receipt)
        && receipt
            .evidence
            .iter()
            .any(|evidence| evidence.kind == "wallet_settlement_proof")
}

fn is_long_feedback_terminal_receipt(receipt: &Receipt) -> bool {
    matches!(
        receipt
            .hints
            .tfb_class
            .unwrap_or(FeedbackLatencyClass::FeedbackLatencyClassUnspecified),
        FeedbackLatencyClass::Long
            | FeedbackLatencyClass::Medium
            | FeedbackLatencyClass::FeedbackLatencyClassUnspecified
    )
}

fn is_adverse_terminal_receipt(receipt: &Receipt) -> bool {
    matches!(
        receipt.receipt_type.as_str(),
        "earn.job.withheld.v1" | "earn.job.failed.v1"
    ) || receipt.hints.reason_code.as_deref().is_some_and(|reason| {
        let normalized = reason.to_ascii_lowercase();
        normalized.contains("failed")
            || normalized.contains("withheld")
            || normalized.contains("rollback")
    })
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum CertificationStateMeta {
    Active,
    Revoked,
    Expired,
    Unspecified,
}

#[derive(Clone)]
struct CertificationScopeMeta {
    category: String,
    tfb_class: FeedbackLatencyClass,
    min_severity: SeverityClass,
    max_severity: SeverityClass,
}

#[derive(Clone)]
struct CertificationObjectMeta {
    certification_id: String,
    revision: u32,
    certification_level: String,
    state: CertificationStateMeta,
    valid_until_ms: i64,
    scope: Vec<CertificationScopeMeta>,
}

fn build_certification_distribution(
    receipts: &[Receipt],
    as_of_ms: i64,
) -> Vec<CertificationDistributionRow> {
    let certifications = latest_certification_objects(receipts, as_of_ms);
    let mut counts = BTreeMap::<
        (
            String,
            FeedbackLatencyClass,
            SeverityClass,
            SeverityClass,
            String,
        ),
        (u64, u64, u64),
    >::new();
    for certification in certifications {
        let mut state = certification.state;
        if state == CertificationStateMeta::Active && certification.valid_until_ms < as_of_ms {
            state = CertificationStateMeta::Expired;
        }
        for scope in certification.scope {
            let key = (
                scope.category,
                scope.tfb_class,
                scope.min_severity,
                scope.max_severity,
                certification.certification_level.clone(),
            );
            let entry = counts.entry(key).or_insert((0, 0, 0));
            match state {
                CertificationStateMeta::Active => entry.0 = entry.0.saturating_add(1),
                CertificationStateMeta::Revoked => entry.1 = entry.1.saturating_add(1),
                CertificationStateMeta::Expired => entry.2 = entry.2.saturating_add(1),
                CertificationStateMeta::Unspecified => {}
            }
        }
    }
    let mut rows = counts
        .into_iter()
        .map(
            |(
                (category, tfb_class, min_severity, max_severity, certification_level),
                (active_count, revoked_count, expired_count),
            )| CertificationDistributionRow {
                category,
                tfb_class,
                min_severity,
                max_severity,
                certification_level,
                active_count,
                revoked_count,
                expired_count,
            },
        )
        .collect::<Vec<_>>();
    rows.sort_by(|lhs, rhs| {
        lhs.category
            .cmp(&rhs.category)
            .then_with(|| lhs.tfb_class.cmp(&rhs.tfb_class))
            .then_with(|| lhs.min_severity.cmp(&rhs.min_severity))
            .then_with(|| lhs.max_severity.cmp(&rhs.max_severity))
            .then_with(|| lhs.certification_level.cmp(&rhs.certification_level))
    });
    rows
}

fn latest_certification_objects(
    receipts: &[Receipt],
    as_of_ms: i64,
) -> Vec<CertificationObjectMeta> {
    let mut ordered = receipts
        .iter()
        .filter(|receipt| receipt.created_at_ms <= as_of_ms)
        .collect::<Vec<_>>();
    ordered.sort_by(|lhs, rhs| {
        lhs.created_at_ms
            .cmp(&rhs.created_at_ms)
            .then_with(|| lhs.receipt_id.cmp(&rhs.receipt_id))
    });
    let mut latest = BTreeMap::<String, CertificationObjectMeta>::new();
    for receipt in ordered {
        let Some(candidate) = certification_object_from_receipt(receipt) else {
            continue;
        };
        let keep_new = latest
            .get(candidate.certification_id.as_str())
            .is_none_or(|existing| {
                candidate.revision > existing.revision
                    || (candidate.revision == existing.revision
                        && candidate.certification_level > existing.certification_level)
            });
        if keep_new {
            latest.insert(candidate.certification_id.clone(), candidate);
        }
    }
    latest.into_values().collect::<Vec<_>>()
}

fn certification_object_from_receipt(receipt: &Receipt) -> Option<CertificationObjectMeta> {
    let evidence = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "certification_object_ref")?;
    let certification_id = evidence
        .meta
        .get("certification_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let revision = evidence
        .meta
        .get("revision")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(u32::MAX as u64) as u32;
    let certification_level = evidence
        .meta
        .get("certification_level")
        .and_then(Value::as_str)
        .map(normalize_label)
        .unwrap_or_else(|| "unknown".to_string());
    let state = evidence
        .meta
        .get("state")
        .and_then(Value::as_str)
        .map(certification_state_from_label)
        .unwrap_or(CertificationStateMeta::Unspecified);
    let valid_until_ms = evidence
        .meta
        .get("valid_until_ms")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0);
    let scope = evidence
        .meta
        .get("scope")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(Value::as_object)
                .map(|row| CertificationScopeMeta {
                    category: row
                        .get("category")
                        .and_then(Value::as_str)
                        .map(normalize_label)
                        .unwrap_or_else(|| "unknown".to_string()),
                    tfb_class: row
                        .get("tfb_class")
                        .and_then(Value::as_str)
                        .and_then(feedback_latency_from_label)
                        .unwrap_or(FeedbackLatencyClass::FeedbackLatencyClassUnspecified),
                    min_severity: row
                        .get("min_severity")
                        .and_then(Value::as_str)
                        .and_then(severity_from_label_strict)
                        .unwrap_or(SeverityClass::SeverityClassUnspecified),
                    max_severity: row
                        .get("max_severity")
                        .and_then(Value::as_str)
                        .and_then(severity_from_label_strict)
                        .unwrap_or(SeverityClass::SeverityClassUnspecified),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some(CertificationObjectMeta {
        certification_id,
        revision,
        certification_level,
        state,
        valid_until_ms,
        scope,
    })
}

fn certification_state_from_label(value: &str) -> CertificationStateMeta {
    match normalize_label(value).as_str() {
        "active" => CertificationStateMeta::Active,
        "revoked" => CertificationStateMeta::Revoked,
        "expired" => CertificationStateMeta::Expired,
        _ => CertificationStateMeta::Unspecified,
    }
}

fn feedback_latency_from_label(value: &str) -> Option<FeedbackLatencyClass> {
    match normalize_label(value).as_str() {
        "instant" => Some(FeedbackLatencyClass::Instant),
        "short" => Some(FeedbackLatencyClass::Short),
        "medium" => Some(FeedbackLatencyClass::Medium),
        "long" => Some(FeedbackLatencyClass::Long),
        "unspecified" => Some(FeedbackLatencyClass::FeedbackLatencyClassUnspecified),
        _ => None,
    }
}

fn severity_from_label_strict(value: &str) -> Option<SeverityClass> {
    match normalize_label(value).as_str() {
        "low" => Some(SeverityClass::Low),
        "medium" => Some(SeverityClass::Medium),
        "high" => Some(SeverityClass::High),
        "critical" => Some(SeverityClass::Critical),
        "unspecified" => Some(SeverityClass::SeverityClassUnspecified),
        _ => None,
    }
}

fn normalize_label(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn is_uncertified_border_block_receipt(receipt: &Receipt) -> bool {
    if receipt.receipt_type != "earn.job.withheld.v1" {
        return false;
    }
    receipt.hints.reason_code.as_deref().is_some_and(|reason| {
        matches!(
            reason,
            REASON_CODE_DIGITAL_BORDER_BLOCK_UNCERTIFIED | REASON_CODE_CERTIFICATION_REQUIRED
        )
    })
}

fn severity_weight(severity: Option<SeverityClass>) -> f64 {
    match severity.unwrap_or(SeverityClass::SeverityClassUnspecified) {
        SeverityClass::Low => 0.25,
        SeverityClass::Medium => 0.50,
        SeverityClass::High => 0.75,
        SeverityClass::Critical => 1.00,
        SeverityClass::SeverityClassUnspecified => 0.50,
    }
}

fn severity_weight_totals(receipts: &[&Receipt]) -> (f64, f64, f64) {
    let mut total = 0.0;
    let mut unverified = 0.0;
    let mut adverse = 0.0;
    for receipt in receipts {
        let weight = severity_weight(receipt.hints.severity);
        total += weight;
        if !is_verified_terminal_receipt(receipt) {
            unverified += weight;
        }
        if is_adverse_terminal_receipt(receipt) {
            adverse += weight;
        }
    }
    (total, unverified, adverse)
}

fn is_incident_or_near_miss_receipt(receipt: &Receipt) -> bool {
    let lower_type = receipt.receipt_type.to_ascii_lowercase();
    if lower_type.contains("incident") || lower_type.contains("near_miss") {
        return true;
    }
    receipt.evidence.iter().any(|evidence| {
        let lower_kind = evidence.kind.to_ascii_lowercase();
        lower_kind.contains("incident") || lower_kind.contains("near_miss")
    })
}

fn is_breaker_or_throttle_receipt(receipt: &Receipt) -> bool {
    if receipt.receipt_type == "economy.policy.throttle_action_applied.v1" {
        return true;
    }
    if receipt
        .hints
        .reason_code
        .as_deref()
        .is_some_and(|reason| reason == "POLICY_THROTTLE_TRIGGERED")
    {
        return true;
    }
    receipt
        .evidence
        .iter()
        .any(|evidence| evidence.kind == "breaker_transition" || evidence.kind == "snapshot_ref")
}

fn is_rollback_receipt(receipt: &Receipt) -> bool {
    let lower_type = receipt.receipt_type.to_ascii_lowercase();
    if lower_type.contains("rollback") || lower_type.contains("compensating_action") {
        return true;
    }
    receipt.hints.reason_code.as_deref().is_some_and(|reason| {
        let lower = reason.to_ascii_lowercase();
        lower.contains("rollback") || lower.contains("compensating")
    })
}

fn is_successful_rollback_receipt(receipt: &Receipt) -> bool {
    matches!(
        receipt.receipt_type.as_str(),
        "economy.rollback.executed.v1" | "economy.compensating_action.executed.v1"
    ) || receipt.hints.reason_code.as_deref().is_some_and(|reason| {
        matches!(reason, "ROLLBACK_EXECUTED" | "COMPENSATING_ACTION_EXECUTED")
    })
}

fn redacted_rollback_reason_code(receipt: &Receipt) -> String {
    let reason_code = receipt
        .hints
        .reason_code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("UNKNOWN_ROLLBACK_REASON")
        .to_ascii_uppercase();
    match reason_code.as_str() {
        "ROLLBACK_EXECUTED" | "ROLLBACK_FAILED" | "COMPENSATING_ACTION_EXECUTED" => reason_code,
        _ => {
            let digest = digest_for_text(reason_code.as_str());
            let suffix = digest
                .strip_prefix("sha256:")
                .unwrap_or(digest.as_str())
                .chars()
                .take(12)
                .collect::<String>();
            format!("redacted:{suffix}")
        }
    }
}

fn quote_delivery_variance_share(scoped_receipts: &[&Receipt]) -> f64 {
    let mut quoted_by_work_unit = BTreeMap::<String, u64>::new();
    let mut delivered_by_work_unit = BTreeMap::<String, u64>::new();

    for receipt in scoped_receipts {
        let Some(work_unit_id) = receipt.trace.work_unit_id.as_ref() else {
            continue;
        };
        if receipt.receipt_type == "earn.job.ingress_request.v1" {
            let quoted = money_as_sats(receipt.hints.notional.as_ref());
            if quoted > 0 {
                quoted_by_work_unit
                    .entry(work_unit_id.clone())
                    .or_insert(quoted);
            }
            continue;
        }
        if is_terminal_work_unit_receipt(receipt) {
            let delivered = money_as_sats(receipt.hints.notional.as_ref());
            if delivered > 0 {
                delivered_by_work_unit.insert(work_unit_id.clone(), delivered);
            }
        }
    }

    let mut samples = 0u64;
    let mut total_variance = 0.0;
    for (work_unit_id, quoted) in quoted_by_work_unit {
        if quoted == 0 {
            continue;
        }
        let Some(delivered) = delivered_by_work_unit.get(work_unit_id.as_str()) else {
            continue;
        };
        let variance = if delivered >= &quoted {
            (*delivered - quoted) as f64 / quoted as f64
        } else {
            (quoted - *delivered) as f64 / quoted as f64
        };
        samples = samples.saturating_add(1);
        total_variance += variance;
    }
    if samples == 0 {
        return 0.0;
    }
    clamp_unit_interval(total_variance / samples as f64)
}

fn metric_key_for_receipt(receipt: &Receipt) -> MetricKey {
    MetricKey {
        category: receipt
            .hints
            .category
            .clone()
            .unwrap_or_else(|| "compute".to_string()),
        tfb_class: receipt
            .hints
            .tfb_class
            .unwrap_or(FeedbackLatencyClass::FeedbackLatencyClassUnspecified),
        severity: receipt
            .hints
            .severity
            .unwrap_or(SeverityClass::SeverityClassUnspecified),
        verification_tier: receipt
            .hints
            .achieved_verification_tier
            .unwrap_or(VerificationTier::VerificationTierUnspecified),
        verification_correlated: receipt.hints.verification_correlated.unwrap_or(false),
        provenance_grade: receipt
            .hints
            .provenance_grade
            .unwrap_or(ProvenanceGrade::ProvenanceGradeUnspecified),
    }
}

fn provenance_bucket(grade: Option<ProvenanceGrade>) -> usize {
    match grade.unwrap_or(ProvenanceGrade::ProvenanceGradeUnspecified) {
        ProvenanceGrade::ProvenanceGradeUnspecified | ProvenanceGrade::P0Minimal => 0,
        ProvenanceGrade::P1Toolchain => 1,
        ProvenanceGrade::P2Lineage => 2,
        ProvenanceGrade::P3Attested => 3,
    }
}

fn snapshot_input_evidence(
    window_start_ms: i64,
    as_of_ms: i64,
    scoped_receipts: &[&Receipt],
) -> Vec<EvidenceRef> {
    let receipt_digest_payload = if scoped_receipts.is_empty() {
        "empty".to_string()
    } else {
        scoped_receipts
            .iter()
            .map(|receipt| format!("{}:{}", receipt.receipt_id, receipt.canonical_hash))
            .collect::<Vec<_>>()
            .join("|")
    };
    let mut evidence = EvidenceRef::new(
        "receipt_window",
        format!("oa://receipts/window/{window_start_ms}-{as_of_ms}"),
        digest_for_text(receipt_digest_payload.as_str()),
    );
    evidence.meta.insert(
        "receipt_count".to_string(),
        Value::Number((scoped_receipts.len() as u64).into()),
    );
    if let Some(first) = scoped_receipts.first() {
        evidence.meta.insert(
            "first_receipt_id".to_string(),
            Value::String(first.receipt_id.clone()),
        );
    }
    if let Some(last) = scoped_receipts.last() {
        evidence.meta.insert(
            "last_receipt_id".to_string(),
            Value::String(last.receipt_id.clone()),
        );
    }
    vec![evidence]
}

fn snapshot_audit_package_digest(scoped_receipts: &[&Receipt], redaction_tier: &str) -> String {
    let canonical_material = scoped_receipts
        .iter()
        .map(|receipt| format!("{}:{}", receipt.receipt_id, receipt.canonical_hash))
        .collect::<Vec<_>>()
        .join("|");
    digest_for_text(format!("{redaction_tier}:{canonical_material}").as_str())
}

#[derive(Serialize)]
struct CanonicalSnapshotPayload<'a> {
    snapshot_id: &'a str,
    as_of_ms: i64,
    sv: f64,
    sv_effective: f64,
    rho: f64,
    rho_effective: f64,
    n: u64,
    nv: f64,
    delta_m_hat: f64,
    xa_hat: f64,
    correlated_verification_share: f64,
    provenance_p0_share: f64,
    provenance_p1_share: f64,
    provenance_p2_share: f64,
    provenance_p3_share: f64,
    personhood_verified_share: f64,
    liability_premiums_collected_24h_sats: u64,
    claims_paid_24h_sats: u64,
    bonded_exposure_24h_sats: u64,
    capital_reserves_24h_sats: u64,
    loss_ratio: f64,
    capital_coverage_ratio: f64,
    drift_alerts_24h: u64,
    drift_signals: &'a [DriftSignalSummary],
    top_drift_signals: &'a [DriftSignalSummary],
    incident_buckets: &'a [IncidentBucketRow],
    safety_signal_buckets: &'a [SafetySignalBucketRow],
    certification_distribution: &'a [CertificationDistributionRow],
    uncertified_block_count_24h: u64,
    uncertified_block_rate: f64,
    exportable_simulation_scenarios: u64,
    simulation_scenario_backlog: u64,
    anchor_publications_24h: u64,
    anchored_snapshots_24h: u64,
    anchor_backend_distribution: &'a [AnchorBackendStatusRow],
    outcome_distribution: &'a [OutcomeDistributionRow],
    outcome_key_rates: &'a [OutcomeKeyRateRow],
    rollback_attempts_24h: u64,
    rollback_successes_24h: u64,
    rollback_success_rate: f64,
    top_rollback_reason_codes: &'a [RollbackReasonCodeRow],
    audit_package_public_digest: &'a str,
    audit_package_restricted_digest: &'a str,
    auth_assurance_distribution: &'a [AuthAssuranceDistributionRow],
    sv_breakdown: &'a [SvBreakdownRow],
    inputs: &'a [EvidenceRef],
}

#[allow(clippy::too_many_arguments)]
fn snapshot_hash_for(
    snapshot_id: &str,
    as_of_ms: i64,
    sv: f64,
    sv_effective: f64,
    rho: f64,
    rho_effective: f64,
    n: u64,
    nv: f64,
    delta_m_hat: f64,
    xa_hat: f64,
    correlated_verification_share: f64,
    provenance_p0_share: f64,
    provenance_p1_share: f64,
    provenance_p2_share: f64,
    provenance_p3_share: f64,
    personhood_verified_share: f64,
    liability_premiums_collected_24h_sats: u64,
    claims_paid_24h_sats: u64,
    bonded_exposure_24h_sats: u64,
    capital_reserves_24h_sats: u64,
    loss_ratio: f64,
    capital_coverage_ratio: f64,
    drift_alerts_24h: u64,
    drift_signals: &[DriftSignalSummary],
    top_drift_signals: &[DriftSignalSummary],
    incident_buckets: &[IncidentBucketRow],
    safety_signal_buckets: &[SafetySignalBucketRow],
    certification_distribution: &[CertificationDistributionRow],
    uncertified_block_count_24h: u64,
    uncertified_block_rate: f64,
    exportable_simulation_scenarios: u64,
    simulation_scenario_backlog: u64,
    anchor_publications_24h: u64,
    anchored_snapshots_24h: u64,
    anchor_backend_distribution: &[AnchorBackendStatusRow],
    outcome_distribution: &[OutcomeDistributionRow],
    outcome_key_rates: &[OutcomeKeyRateRow],
    rollback_attempts_24h: u64,
    rollback_successes_24h: u64,
    rollback_success_rate: f64,
    top_rollback_reason_codes: &[RollbackReasonCodeRow],
    audit_package_public_digest: &str,
    audit_package_restricted_digest: &str,
    auth_assurance_distribution: &[AuthAssuranceDistributionRow],
    sv_breakdown: &[SvBreakdownRow],
    inputs: &[EvidenceRef],
) -> Result<String, String> {
    let payload = CanonicalSnapshotPayload {
        snapshot_id,
        as_of_ms,
        sv,
        sv_effective,
        rho,
        rho_effective,
        n,
        nv,
        delta_m_hat,
        xa_hat,
        correlated_verification_share,
        provenance_p0_share,
        provenance_p1_share,
        provenance_p2_share,
        provenance_p3_share,
        personhood_verified_share,
        liability_premiums_collected_24h_sats,
        claims_paid_24h_sats,
        bonded_exposure_24h_sats,
        capital_reserves_24h_sats,
        loss_ratio,
        capital_coverage_ratio,
        drift_alerts_24h,
        drift_signals,
        top_drift_signals,
        incident_buckets,
        safety_signal_buckets,
        certification_distribution,
        uncertified_block_count_24h,
        uncertified_block_rate,
        exportable_simulation_scenarios,
        simulation_scenario_backlog,
        anchor_publications_24h,
        anchored_snapshots_24h,
        anchor_backend_distribution,
        outcome_distribution,
        outcome_key_rates,
        rollback_attempts_24h,
        rollback_successes_24h,
        rollback_success_rate,
        top_rollback_reason_codes,
        audit_package_public_digest,
        audit_package_restricted_digest,
        auth_assurance_distribution,
        sv_breakdown,
        inputs,
    };
    let value = serde_json::to_value(payload)
        .map_err(|error| format!("Failed to encode snapshot payload: {error}"))?;
    hash_value(&canonicalize_value(value))
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        return 0.0;
    }
    numerator as f64 / denominator as f64
}

fn ratio_f64(numerator: f64, denominator: f64) -> f64 {
    if denominator <= 0.0 {
        return 0.0;
    }
    numerator / denominator
}

fn correlation_adjusted_sv_effective(
    sv: f64,
    correlated_verification_share: f64,
    provenance_p2_share: f64,
    provenance_p3_share: f64,
) -> f64 {
    let normalized_sv = clamp_unit_interval(sv);
    let normalized_correlated_share = clamp_unit_interval(correlated_verification_share);
    let lineage_diversity = clamp_unit_interval(provenance_p2_share + provenance_p3_share);
    let correlation_penalty = clamp_unit_interval(
        normalized_correlated_share
            * (1.0 - (SV_EFFECTIVE_LINEAGE_OFFSET_WEIGHT * lineage_diversity)),
    );
    clamp_unit_interval(normalized_sv * (1.0 - correlation_penalty))
}

fn clamp_unit_interval(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

#[allow(clippy::too_many_arguments)]
fn build_drift_signals(
    sv: f64,
    correlated_verification_share: f64,
    payout_success_share: f64,
    dispute_or_claim_share: f64,
    incident_signal_share: f64,
    xa_hat: f64,
    unverified_count_24h: u64,
    correlated_verified_count_24h: u64,
    payout_failure_count_24h: u64,
    dispute_or_claim_count_24h: u64,
    incident_count_24h: u64,
) -> (u64, Vec<DriftSignalSummary>, Vec<DriftSignalSummary>) {
    let mut drift_signals = vec![
        drift_signal_low_guard(
            "detector.drift.sv_floor",
            "sv_below_floor",
            unverified_count_24h,
            sv,
            DRIFT_THRESHOLD_SV_FLOOR,
        ),
        drift_signal_high_guard(
            "detector.drift.correlation_pressure",
            "correlated_verification_high",
            correlated_verified_count_24h,
            correlated_verification_share,
            DRIFT_THRESHOLD_CORRELATED_SHARE_MAX,
        ),
        drift_signal_low_guard(
            "detector.drift.payout_success",
            "payout_success_low",
            payout_failure_count_24h,
            payout_success_share,
            DRIFT_THRESHOLD_PAYOUT_SUCCESS_MIN,
        ),
        drift_signal_high_guard(
            "detector.drift.dispute_claim_spike",
            "dispute_claim_share_high",
            dispute_or_claim_count_24h,
            dispute_or_claim_share,
            DRIFT_THRESHOLD_DISPUTE_CLAIM_SHARE_MAX,
        ),
        drift_signal_high_guard(
            "detector.drift.incident_pressure",
            "incident_share_high",
            incident_count_24h,
            incident_signal_share,
            DRIFT_THRESHOLD_INCIDENT_SHARE_MAX,
        ),
        drift_signal_high_guard(
            "detector.drift.xa_pressure",
            "xa_hat_high",
            incident_count_24h.max(payout_failure_count_24h),
            xa_hat,
            DRIFT_THRESHOLD_XA_MAX,
        ),
    ];

    drift_signals.sort_by(compare_drift_signals);
    let drift_alerts_24h = drift_signals
        .iter()
        .filter(|signal| signal.alert)
        .fold(0u64, |total, signal| {
            total.saturating_add(signal.count_24h.max(1))
        });
    let top_drift_signals = drift_signals
        .iter()
        .filter(|signal| signal.score > 0.0)
        .take(DRIFT_SIGNAL_TOP_LIMIT)
        .cloned()
        .collect::<Vec<_>>();

    (drift_alerts_24h, drift_signals, top_drift_signals)
}

fn compare_drift_signals(lhs: &DriftSignalSummary, rhs: &DriftSignalSummary) -> Ordering {
    rhs.score
        .partial_cmp(&lhs.score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| rhs.count_24h.cmp(&lhs.count_24h))
        .then_with(|| lhs.detector_id.cmp(&rhs.detector_id))
        .then_with(|| lhs.signal_code.cmp(&rhs.signal_code))
}

fn drift_signal_high_guard(
    detector_id: &str,
    signal_code: &str,
    count_24h: u64,
    ratio: f64,
    threshold: f64,
) -> DriftSignalSummary {
    let ratio = clamp_unit_interval(ratio);
    let threshold = clamp_unit_interval(threshold);
    let alert = ratio > threshold;
    let score = if ratio <= threshold {
        0.0
    } else if threshold >= 1.0 {
        1.0
    } else {
        clamp_unit_interval((ratio - threshold) / (1.0 - threshold))
    };
    DriftSignalSummary {
        detector_id: detector_id.to_string(),
        signal_code: signal_code.to_string(),
        count_24h,
        ratio,
        threshold,
        score,
        alert,
    }
}

fn drift_signal_low_guard(
    detector_id: &str,
    signal_code: &str,
    count_24h: u64,
    ratio: f64,
    threshold: f64,
) -> DriftSignalSummary {
    let ratio = clamp_unit_interval(ratio);
    let threshold = clamp_unit_interval(threshold);
    let alert = ratio < threshold;
    let score = if ratio >= threshold || threshold <= 0.0 {
        0.0
    } else {
        clamp_unit_interval((threshold - ratio) / threshold)
    };
    DriftSignalSummary {
        detector_id: detector_id.to_string(),
        signal_code: signal_code.to_string(),
        count_24h,
        ratio,
        threshold,
        score,
        alert,
    }
}

fn money_as_sats(value: Option<&Money>) -> u64 {
    let Some(value) = value else {
        return 0;
    };
    if value.asset != Asset::Btc {
        return 0;
    }
    match value.amount {
        MoneyAmount::AmountSats(sats) => sats,
        MoneyAmount::AmountMsats(msats) => msats / 1_000,
    }
}

fn btc_sats_money(amount_sats: u64) -> Money {
    Money {
        asset: Asset::Btc,
        amount: MoneyAmount::AmountSats(amount_sats),
    }
}

fn is_claim_paid_receipt(receipt: &Receipt) -> bool {
    matches!(
        receipt.receipt_type.as_str(),
        "earn.claim.paid.v1"
            | "earn.claim.settlement_paid.v1"
            | "earn.job.rollback_compensation_paid.v1"
    ) || receipt
        .evidence
        .iter()
        .any(|evidence| evidence.kind == "claim_payout_proof")
}

fn is_dispute_or_claim_receipt(receipt: &Receipt) -> bool {
    if is_claim_paid_receipt(receipt) {
        return true;
    }
    let receipt_type = receipt.receipt_type.to_ascii_lowercase();
    if receipt_type.contains("dispute") || receipt_type.contains("chargeback") {
        return true;
    }
    receipt.evidence.iter().any(|evidence| {
        let kind = evidence.kind.to_ascii_lowercase();
        kind.contains("dispute") || kind.contains("chargeback")
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IncidentReceiptKind {
    Incident,
    NearMiss,
    GroundTruthCase,
    Unknown,
}

fn is_incident_receipt(receipt: &Receipt) -> bool {
    receipt.receipt_type.starts_with("economy.incident.")
        || receipt
            .evidence
            .iter()
            .any(|evidence| evidence.kind == "incident_object_ref")
}

fn incident_kind_for_receipt(receipt: &Receipt) -> IncidentReceiptKind {
    let from_meta = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "incident_object_ref")
        .and_then(|evidence| evidence.meta.get("incident_kind"))
        .and_then(Value::as_str)
        .unwrap_or("");
    match from_meta {
        "near_miss" => IncidentReceiptKind::NearMiss,
        "ground_truth_case" => IncidentReceiptKind::GroundTruthCase,
        "incident" => IncidentReceiptKind::Incident,
        _ => {
            let lower_type = receipt.receipt_type.to_ascii_lowercase();
            if lower_type.contains("near_miss") {
                IncidentReceiptKind::NearMiss
            } else if lower_type.contains("ground_truth") {
                IncidentReceiptKind::GroundTruthCase
            } else if lower_type.contains("incident") {
                IncidentReceiptKind::Incident
            } else {
                IncidentReceiptKind::Unknown
            }
        }
    }
}

fn incident_taxonomy_code_for_receipt(receipt: &Receipt) -> Option<String> {
    let from_meta = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "incident_object_ref")
        .and_then(|evidence| evidence.meta.get("taxonomy_code"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    if from_meta
        .as_ref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return from_meta;
    }
    receipt
        .hints
        .reason_code
        .as_ref()
        .and_then(|reason| reason.strip_prefix("incident_taxonomy:"))
        .map(ToString::to_string)
}

fn incident_severity_for_receipt(receipt: &Receipt) -> SeverityClass {
    if let Some(value) = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "incident_object_ref")
        .and_then(|evidence| evidence.meta.get("severity"))
        .and_then(Value::as_str)
    {
        return match value {
            "critical" => SeverityClass::Critical,
            "high" => SeverityClass::High,
            "medium" => SeverityClass::Medium,
            "low" => SeverityClass::Low,
            _ => receipt
                .hints
                .severity
                .unwrap_or(SeverityClass::SeverityClassUnspecified),
        };
    }
    receipt
        .hints
        .severity
        .unwrap_or(SeverityClass::SeverityClassUnspecified)
}

fn simulation_scenario_backlog_counts(receipts: &[Receipt], as_of_ms: i64) -> (u64, u64) {
    let mut ordered = receipts
        .iter()
        .filter(|receipt| receipt.created_at_ms <= as_of_ms)
        .collect::<Vec<_>>();
    ordered.sort_by(|lhs, rhs| {
        lhs.created_at_ms
            .cmp(&rhs.created_at_ms)
            .then_with(|| lhs.receipt_id.cmp(&rhs.receipt_id))
    });
    let mut latest_ground_truth_case_by_id = BTreeMap::<String, String>::new();
    for receipt in ordered
        .iter()
        .copied()
        .filter(|receipt| is_incident_receipt(receipt))
    {
        if incident_kind_for_receipt(receipt) != IncidentReceiptKind::GroundTruthCase {
            continue;
        }
        let Some((ground_truth_case_id, ground_truth_case_digest)) =
            ground_truth_case_ref_for_receipt(receipt)
        else {
            continue;
        };
        latest_ground_truth_case_by_id.insert(ground_truth_case_id, ground_truth_case_digest);
    }
    let mut exported_ground_truth_case_ids = BTreeSet::<String>::new();
    for receipt in ordered
        .iter()
        .copied()
        .filter(|receipt| is_simulation_scenario_export_receipt(receipt))
    {
        for (ground_truth_case_id, ground_truth_case_digest) in
            exported_ground_truth_case_refs_from_receipt(receipt)
        {
            if latest_ground_truth_case_by_id
                .get(ground_truth_case_id.as_str())
                .is_some_and(|current_digest| current_digest == &ground_truth_case_digest)
            {
                exported_ground_truth_case_ids.insert(ground_truth_case_id);
            }
        }
    }
    let exportable_simulation_scenarios = latest_ground_truth_case_by_id.len() as u64;
    let simulation_scenario_backlog = exportable_simulation_scenarios
        .saturating_sub(exported_ground_truth_case_ids.len().min(u64::MAX as usize) as u64);
    (exportable_simulation_scenarios, simulation_scenario_backlog)
}

fn ground_truth_case_ref_for_receipt(receipt: &Receipt) -> Option<(String, String)> {
    let incident_ref = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "incident_object_ref")?;
    let incident_id = incident_ref
        .meta
        .get("incident_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let incident_digest = incident_ref.digest.trim().to_string();
    if incident_digest.is_empty() {
        return None;
    }
    Some((incident_id, incident_digest))
}

fn is_simulation_scenario_export_receipt(receipt: &Receipt) -> bool {
    receipt.receipt_type == "economy.simulation_scenario.exported.v1"
        || receipt
            .evidence
            .iter()
            .any(|evidence| evidence.kind == "simulation_scenario_ref")
}

fn exported_ground_truth_case_refs_from_receipt(receipt: &Receipt) -> Vec<(String, String)> {
    let mut refs = receipt
        .evidence
        .iter()
        .filter(|evidence| evidence.kind == "simulation_scenario_ref")
        .filter_map(|evidence| {
            let ground_truth_case_id = evidence
                .meta
                .get("ground_truth_case_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?
                .to_string();
            let ground_truth_case_digest = evidence
                .meta
                .get("ground_truth_case_digest")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?
                .to_string();
            Some((ground_truth_case_id, ground_truth_case_digest))
        })
        .collect::<Vec<_>>();
    refs.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0).then_with(|| lhs.1.cmp(&rhs.1)));
    refs.dedup();
    refs
}

fn is_anchor_publication_receipt(receipt: &Receipt) -> bool {
    receipt.receipt_type == "economy.anchor.published.v1"
        || receipt
            .evidence
            .iter()
            .any(|evidence| evidence.kind == "anchor_proof_ref")
}

fn anchor_backend_for_receipt(receipt: &Receipt) -> Option<String> {
    receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "anchor_proof_ref")
        .and_then(|evidence| evidence.meta.get("anchor_backend"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn anchor_snapshot_id_for_receipt(receipt: &Receipt) -> Option<String> {
    receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "snapshot_ref")
        .and_then(|evidence| evidence.uri.strip_prefix("oa://economy/snapshots/"))
        .map(|suffix| suffix.split('/').next().unwrap_or(suffix))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SafetySignalClass {
    Incident,
    Drift,
    Adverse,
}

#[derive(Clone, Debug)]
struct SafetySignalReceiptMeta {
    taxonomy_code: String,
    severity: SeverityClass,
    signal_class: SafetySignalClass,
}

fn safety_signal_meta_for_receipt(receipt: &Receipt) -> Option<SafetySignalReceiptMeta> {
    if is_incident_receipt(receipt) {
        return Some(SafetySignalReceiptMeta {
            taxonomy_code: incident_taxonomy_code_for_receipt(receipt)
                .unwrap_or_else(|| "unknown".to_string()),
            severity: incident_severity_for_receipt(receipt),
            signal_class: SafetySignalClass::Incident,
        });
    }
    if is_drift_signal_receipt(receipt) {
        return Some(SafetySignalReceiptMeta {
            taxonomy_code: drift_taxonomy_code_for_receipt(receipt),
            severity: drift_severity_for_receipt(receipt),
            signal_class: SafetySignalClass::Drift,
        });
    }
    adverse_signal_meta_for_receipt(receipt)
}

fn is_drift_signal_receipt(receipt: &Receipt) -> bool {
    receipt.receipt_type.starts_with("economy.drift.")
        || receipt
            .evidence
            .iter()
            .any(|evidence| evidence.kind == "drift_detector_ref")
}

fn drift_taxonomy_code_for_receipt(receipt: &Receipt) -> String {
    let signal_code = drift_signal_code_for_receipt(receipt);
    format!("drift.{signal_code}")
}

fn drift_signal_code_for_receipt(receipt: &Receipt) -> String {
    let from_summary = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "drift_signal_summary")
        .and_then(|evidence| evidence.meta.get("signal_code"))
        .and_then(Value::as_str)
        .and_then(normalized_signal_label);
    if let Some(signal_code) = from_summary {
        return signal_code;
    }
    let from_detector = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "drift_detector_ref")
        .and_then(|evidence| evidence.meta.get("signal_code"))
        .and_then(Value::as_str)
        .and_then(normalized_signal_label);
    if let Some(signal_code) = from_detector {
        return signal_code;
    }
    if let Some(reason_code) = receipt
        .hints
        .reason_code
        .as_deref()
        .and_then(normalized_signal_label)
    {
        return reason_code;
    }
    "unknown".to_string()
}

fn drift_severity_for_receipt(receipt: &Receipt) -> SeverityClass {
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

fn adverse_signal_meta_for_receipt(receipt: &Receipt) -> Option<SafetySignalReceiptMeta> {
    let receipt_type = receipt.receipt_type.to_ascii_lowercase();
    let reason_code = receipt
        .hints
        .reason_code
        .as_deref()
        .and_then(normalized_signal_label)
        .unwrap_or_else(|| "unknown".to_string());
    if receipt.receipt_type == "economy.policy.throttle_action_applied.v1"
        || reason_code == "policy_throttle_triggered"
    {
        return Some(SafetySignalReceiptMeta {
            taxonomy_code: "policy.throttle".to_string(),
            severity: receipt.hints.severity.unwrap_or(SeverityClass::High),
            signal_class: SafetySignalClass::Adverse,
        });
    }
    if receipt_type.contains("rollback")
        || receipt_type.contains("compensating_action")
        || reason_code.contains("rollback")
        || reason_code.contains("compensating")
    {
        return Some(SafetySignalReceiptMeta {
            taxonomy_code: "rollback.action".to_string(),
            severity: receipt.hints.severity.unwrap_or(SeverityClass::High),
            signal_class: SafetySignalClass::Adverse,
        });
    }
    if receipt_type.contains("claim")
        || receipt_type.contains("dispute")
        || receipt
            .evidence
            .iter()
            .any(|evidence| evidence.kind == "claim_payout_proof")
    {
        return Some(SafetySignalReceiptMeta {
            taxonomy_code: "finance.claim_dispute".to_string(),
            severity: receipt.hints.severity.unwrap_or(SeverityClass::High),
            signal_class: SafetySignalClass::Adverse,
        });
    }
    None
}

fn normalized_signal_label(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase().replace(' ', "_");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

#[derive(Clone, Debug)]
struct OutcomeReceiptMeta {
    verdict_outcome: String,
    settlement_outcome: String,
    claim_outcome: Option<String>,
    remedy_outcome: Option<String>,
}

fn is_outcome_registry_receipt(receipt: &Receipt) -> bool {
    receipt
        .receipt_type
        .starts_with("economy.outcome_registry.")
        || receipt
            .evidence
            .iter()
            .any(|evidence| evidence.kind == "outcome_registry_entry_ref")
}

fn outcome_meta_for_receipt(receipt: &Receipt) -> Option<OutcomeReceiptMeta> {
    let evidence = receipt
        .evidence
        .iter()
        .find(|evidence| evidence.kind == "outcome_registry_entry_ref")?;
    let verdict_outcome = evidence
        .meta
        .get("verdict_outcome")
        .and_then(Value::as_str)
        .and_then(normalized_outcome_label)?;
    let settlement_outcome = evidence
        .meta
        .get("settlement_outcome")
        .and_then(Value::as_str)
        .and_then(normalized_outcome_label)?;
    let claim_outcome = evidence
        .meta
        .get("claim_outcome")
        .and_then(Value::as_str)
        .and_then(normalized_outcome_label);
    let remedy_outcome = evidence
        .meta
        .get("remedy_outcome")
        .and_then(Value::as_str)
        .and_then(normalized_outcome_label);
    Some(OutcomeReceiptMeta {
        verdict_outcome,
        settlement_outcome,
        claim_outcome,
        remedy_outcome,
    })
}

fn normalized_outcome_label(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase().replace(' ', "_");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn is_success_outcome_label(value: &str) -> bool {
    matches!(
        value,
        "settled" | "paid" | "success" | "succeeded" | "completed"
    )
}

fn is_present_outcome_label(value: &str) -> bool {
    !matches!(
        value,
        "none" | "unknown" | "not_applicable" | "not-applicable" | "no_claim" | "no_remedy"
    )
}

fn canonicalize_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = BTreeMap::new();
            for (key, entry) in map {
                sorted.insert(key, canonicalize_value(entry));
            }
            let mut canonical = serde_json::Map::new();
            for (key, entry) in sorted {
                canonical.insert(key, entry);
            }
            Value::Object(canonical)
        }
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(canonicalize_value)
                .collect::<Vec<_>>(),
        ),
        scalar => scalar,
    }
}

fn hash_value(value: &Value) -> Result<String, String> {
    let payload = serde_json::to_vec(value)
        .map_err(|error| format!("Failed to encode canonical snapshot payload: {error}"))?;
    let digest = sha256::Hash::hash(payload.as_slice());
    Ok(format!("sha256:{digest}"))
}

fn digest_for_text(value: &str) -> String {
    let digest = sha256::Hash::hash(value.as_bytes());
    format!("sha256:{digest}")
}

fn economy_snapshot_file_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-economy-snapshots-v1.json")
}

fn normalize_snapshots(mut snapshots: Vec<EconomySnapshot>) -> Vec<EconomySnapshot> {
    snapshots.sort_by(|lhs, rhs| {
        rhs.as_of_ms
            .cmp(&lhs.as_of_ms)
            .then_with(|| lhs.snapshot_id.cmp(&rhs.snapshot_id))
    });
    snapshots.truncate(ECONOMY_SNAPSHOT_RETENTION_LIMIT);
    snapshots
}

fn persist_economy_snapshots(path: &Path, snapshots: &[EconomySnapshot]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create economy snapshot dir: {error}"))?;
    }

    let document = EconomySnapshotDocumentV1 {
        schema_version: ECONOMY_SNAPSHOT_SCHEMA_VERSION,
        stream_id: ECONOMY_SNAPSHOT_STREAM_ID.to_string(),
        snapshots: normalize_snapshots(snapshots.to_vec()),
    };
    let payload = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Failed to encode economy snapshots: {error}"))?;
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, payload)
        .map_err(|error| format!("Failed to write economy snapshots temp file: {error}"))?;
    std::fs::rename(&temp_path, path)
        .map_err(|error| format!("Failed to persist economy snapshots: {error}"))?;
    Ok(())
}

fn load_economy_snapshots(path: &Path) -> Result<Vec<EconomySnapshot>, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!("Failed to read economy snapshots: {error}"));
        }
    };
    let document = serde_json::from_str::<EconomySnapshotDocumentV1>(&raw)
        .map_err(|error| format!("Failed to parse economy snapshots: {error}"))?;
    if document.schema_version != ECONOMY_SNAPSHOT_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported economy snapshot schema version: {}",
            document.schema_version
        ));
    }
    if document.stream_id != ECONOMY_SNAPSHOT_STREAM_ID {
        return Err(format!(
            "Unsupported economy snapshot stream id: {}",
            document.stream_id
        ));
    }
    Ok(normalize_snapshots(document.snapshots))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy_kernel_receipts::{
        PolicyContext, ReceiptBuilder, ReceiptHints, TraceContext,
    };

    fn fixture_receipt(
        receipt_id: &str,
        receipt_type: &str,
        created_at_ms: i64,
        work_unit_id: &str,
        include_wallet_proof: bool,
    ) -> Receipt {
        let mut evidence = vec![EvidenceRef::new(
            "request_id",
            format!("oa://nip90/request/{receipt_id}"),
            digest_for_text(receipt_id),
        )];
        if include_wallet_proof {
            evidence.push(EvidenceRef::new(
                "wallet_settlement_proof",
                format!("oa://wallet/payments/{receipt_id}"),
                digest_for_text(receipt_id),
            ));
        }
        ReceiptBuilder::new(
            receipt_id,
            receipt_type,
            created_at_ms,
            format!("idemp:{receipt_id}"),
            TraceContext {
                session_id: None,
                trajectory_hash: None,
                job_hash: None,
                run_id: None,
                work_unit_id: Some(work_unit_id.to_string()),
                contract_id: None,
                claim_id: None,
            },
            PolicyContext {
                policy_bundle_id: "policy.earn.default".to_string(),
                policy_version: "1".to_string(),
                approved_by: "autopilot-desktop".to_string(),
            },
        )
        .with_inputs_payload(serde_json::json!({
            "work_unit_id": work_unit_id,
        }))
        .with_outputs_payload(serde_json::json!({
            "status": "terminal",
        }))
        .with_evidence(evidence)
        .with_hints(ReceiptHints {
            category: Some("compute".to_string()),
            tfb_class: Some(FeedbackLatencyClass::Short),
            severity: Some(SeverityClass::Low),
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: None,
            auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
            personhood_proved: Some(false),
            reason_code: None,
            notional: None,
            liability_premium: None,
        })
        .build()
        .expect("fixture receipt should build")
    }

    fn fixture_incident_receipt(
        receipt_id: &str,
        created_at_ms: i64,
        incident_kind: &str,
        taxonomy_code: &str,
        severity_label: &str,
    ) -> Receipt {
        let mut receipt = fixture_receipt(
            receipt_id,
            "economy.incident.reported.v1",
            created_at_ms,
            "incident-work-unit",
            false,
        );
        receipt.hints.severity = Some(match severity_label {
            "critical" => SeverityClass::Critical,
            "high" => SeverityClass::High,
            "medium" => SeverityClass::Medium,
            _ => SeverityClass::Low,
        });
        let mut incident_ref = EvidenceRef::new(
            "incident_object_ref",
            format!("oa://economy/incidents/{receipt_id}/revisions/1"),
            digest_for_text(receipt_id),
        );
        incident_ref.meta.insert(
            "incident_kind".to_string(),
            serde_json::json!(incident_kind),
        );
        incident_ref.meta.insert(
            "incident_id".to_string(),
            serde_json::json!(format!("incident:{receipt_id}")),
        );
        incident_ref.meta.insert(
            "taxonomy_code".to_string(),
            serde_json::json!(taxonomy_code),
        );
        incident_ref
            .meta
            .insert("severity".to_string(), serde_json::json!(severity_label));
        receipt.evidence.push(incident_ref);
        receipt
    }

    fn fixture_simulation_scenario_export_receipt(
        receipt_id: &str,
        created_at_ms: i64,
        ground_truth_case_id: &str,
        ground_truth_case_digest: &str,
    ) -> Receipt {
        let mut receipt = fixture_receipt(
            receipt_id,
            "economy.simulation_scenario.exported.v1",
            created_at_ms,
            "simulation-work-unit",
            false,
        );
        let mut scenario_ref = EvidenceRef::new(
            "simulation_scenario_ref",
            format!("oa://economy/simulation_scenarios/{receipt_id}"),
            digest_for_text(receipt_id),
        );
        scenario_ref.meta.insert(
            "ground_truth_case_id".to_string(),
            serde_json::json!(ground_truth_case_id),
        );
        scenario_ref.meta.insert(
            "ground_truth_case_digest".to_string(),
            serde_json::json!(ground_truth_case_digest),
        );
        receipt.evidence.push(scenario_ref);
        receipt
    }

    fn fixture_anchor_publication_receipt(
        receipt_id: &str,
        created_at_ms: i64,
        anchor_backend: &str,
        snapshot_id: &str,
    ) -> Receipt {
        let mut receipt = fixture_receipt(
            receipt_id,
            "economy.anchor.published.v1",
            created_at_ms,
            "anchor-work-unit",
            false,
        );
        receipt.evidence.push(EvidenceRef::new(
            "snapshot_ref",
            format!("oa://economy/snapshots/{snapshot_id}"),
            digest_for_text(format!("{snapshot_id}:{receipt_id}").as_str()),
        ));
        let mut proof_ref = EvidenceRef::new(
            "anchor_proof_ref",
            format!(
                "oa://anchors/{}/proof/{}",
                anchor_backend,
                digest_for_text(receipt_id)
            ),
            digest_for_text(format!("proof:{receipt_id}").as_str()),
        );
        proof_ref.meta.insert(
            "anchor_backend".to_string(),
            serde_json::json!(anchor_backend),
        );
        proof_ref
            .meta
            .insert("snapshot_id".to_string(), serde_json::json!(snapshot_id));
        receipt.evidence.push(proof_ref);
        receipt
    }

    fn fixture_rollback_receipt(
        receipt_id: &str,
        receipt_type: &str,
        created_at_ms: i64,
        work_unit_id: &str,
        reason_code: &str,
    ) -> Receipt {
        let mut receipt =
            fixture_receipt(receipt_id, receipt_type, created_at_ms, work_unit_id, false);
        receipt.hints.reason_code = Some(reason_code.to_string());
        receipt.evidence.push(EvidenceRef::new(
            "rollback_plan_ref",
            format!("oa://rollback/plans/{work_unit_id}"),
            digest_for_text(work_unit_id),
        ));
        receipt
    }

    fn fixture_outcome_registry_receipt(
        receipt_id: &str,
        created_at_ms: i64,
        work_unit_id: &str,
        verdict_outcome: &str,
        settlement_outcome: &str,
        claim_outcome: Option<&str>,
        remedy_outcome: Option<&str>,
    ) -> Receipt {
        let mut receipt = fixture_receipt(
            receipt_id,
            "economy.outcome_registry.created.v1",
            created_at_ms,
            work_unit_id,
            false,
        );
        let mut outcome_ref = EvidenceRef::new(
            "outcome_registry_entry_ref",
            format!("oa://economy/outcome_registry/{receipt_id}/revisions/1"),
            digest_for_text(receipt_id),
        );
        outcome_ref.meta.insert(
            "verdict_outcome".to_string(),
            serde_json::json!(verdict_outcome),
        );
        outcome_ref.meta.insert(
            "settlement_outcome".to_string(),
            serde_json::json!(settlement_outcome),
        );
        if let Some(value) = claim_outcome {
            outcome_ref
                .meta
                .insert("claim_outcome".to_string(), serde_json::json!(value));
        }
        if let Some(value) = remedy_outcome {
            outcome_ref
                .meta
                .insert("remedy_outcome".to_string(), serde_json::json!(value));
        }
        receipt.evidence.push(outcome_ref);
        receipt
    }

    fn fixture_certification_receipt(
        receipt_id: &str,
        receipt_type: &str,
        created_at_ms: i64,
        certification_id: &str,
        revision: u32,
        state: &str,
        valid_until_ms: i64,
    ) -> Receipt {
        let mut receipt = fixture_receipt(
            receipt_id,
            receipt_type,
            created_at_ms,
            "certification-work-unit",
            false,
        );
        receipt.hints.category = Some("compute".to_string());
        receipt.hints.tfb_class = Some(FeedbackLatencyClass::Short);
        receipt.hints.severity = Some(SeverityClass::High);
        let mut certification_ref = EvidenceRef::new(
            "certification_object_ref",
            format!("oa://economy/certifications/{certification_id}/revisions/{revision}"),
            digest_for_text(format!("{certification_id}:{revision}:{state}").as_str()),
        );
        certification_ref.meta.insert(
            "certification_id".to_string(),
            serde_json::json!(certification_id),
        );
        certification_ref
            .meta
            .insert("revision".to_string(), serde_json::json!(revision));
        certification_ref
            .meta
            .insert("state".to_string(), serde_json::json!(state));
        certification_ref.meta.insert(
            "certification_level".to_string(),
            serde_json::json!("level_2"),
        );
        certification_ref.meta.insert(
            "scope".to_string(),
            serde_json::json!([{
                "category": "compute",
                "tfb_class": "short",
                "min_severity": "high",
                "max_severity": "critical"
            }]),
        );
        certification_ref.meta.insert(
            "valid_until_ms".to_string(),
            serde_json::json!(valid_until_ms),
        );
        receipt.evidence.push(certification_ref);
        receipt
    }

    fn fixture_drift_alert_receipt(receipt_id: &str, created_at_ms: i64) -> Receipt {
        let mut receipt = fixture_receipt(
            receipt_id,
            "economy.drift.alert_raised.v1",
            created_at_ms,
            "drift-work-unit",
            false,
        );
        receipt.hints.severity = Some(SeverityClass::High);
        let mut detector_ref = EvidenceRef::new(
            "drift_detector_ref",
            format!("oa://economy/drift/detectors/{receipt_id}"),
            digest_for_text(receipt_id),
        );
        detector_ref.meta.insert(
            "detector_id".to_string(),
            serde_json::json!("detector.drift.sv_floor"),
        );
        detector_ref.meta.insert(
            "signal_code".to_string(),
            serde_json::json!("sv_below_floor"),
        );
        let mut summary_ref = EvidenceRef::new(
            "drift_signal_summary",
            format!("oa://economy/drift/signals/{receipt_id}"),
            digest_for_text(format!("{receipt_id}:summary").as_str()),
        );
        summary_ref.meta.insert(
            "signal_code".to_string(),
            serde_json::json!("sv_below_floor"),
        );
        summary_ref
            .meta
            .insert("score".to_string(), serde_json::json!(0.9));
        receipt.evidence.push(detector_ref);
        receipt.evidence.push(summary_ref);
        receipt
    }

    fn fixture_policy_throttle_receipt(receipt_id: &str, created_at_ms: i64) -> Receipt {
        let mut receipt = fixture_receipt(
            receipt_id,
            "economy.policy.throttle_action_applied.v1",
            created_at_ms,
            "policy-work-unit",
            false,
        );
        receipt.hints.severity = Some(SeverityClass::High);
        receipt.hints.reason_code = Some("POLICY_THROTTLE_TRIGGERED".to_string());
        receipt
    }

    #[test]
    fn computes_minute_snapshot_once_and_reuses_same_snapshot_id() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("snapshots.json");
        let mut state = EconomySnapshotState::from_snapshot_path_for_tests(path.clone());
        let receipts = vec![fixture_receipt(
            "receipt-paid-1",
            "earn.job.settlement_observed.v1",
            1_762_000_010_000,
            "job-1",
            true,
        )];

        let first = state
            .compute_minute_snapshot(1_762_000_012_345, receipts.as_slice())
            .expect("first compute should emit");
        assert_eq!(first.snapshot.as_of_ms, 1_761_999_960_000);

        let second = state.compute_minute_snapshot(1_762_000_018_000, receipts.as_slice());
        assert!(second.is_none());
        assert_eq!(
            state.latest_snapshot_id.as_deref(),
            Some("snapshot.economy:1761999960000")
        );

        let reloaded = EconomySnapshotState::from_snapshot_path_for_tests(path);
        assert!(
            reloaded
                .get_snapshot("snapshot.economy:1761999960000")
                .is_some()
        );
    }

    #[test]
    fn snapshot_metrics_are_derived_from_terminal_receipts() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("snapshots.json");
        let mut state = EconomySnapshotState::from_snapshot_path_for_tests(path);
        let mut receipt_paid_1 = fixture_receipt(
            "receipt-paid-1",
            "earn.job.settlement_observed.v1",
            1_762_000_010_000,
            "job-1",
            true,
        );
        receipt_paid_1.hints.notional = Some(btc_sats_money(200));
        receipt_paid_1.hints.liability_premium = Some(btc_sats_money(20));

        let mut receipt_paid_2 = fixture_receipt(
            "receipt-paid-2",
            "earn.job.settlement_observed.v1",
            1_762_000_011_000,
            "job-2",
            true,
        );
        receipt_paid_2.hints.notional = Some(btc_sats_money(100));
        receipt_paid_2.hints.liability_premium = Some(btc_sats_money(10));

        let mut receipt_failed_1 = fixture_receipt(
            "receipt-failed-1",
            "earn.job.failed.v1",
            1_762_000_012_000,
            "job-3",
            false,
        );
        receipt_failed_1.hints.notional = Some(btc_sats_money(50));
        receipt_failed_1.hints.liability_premium = Some(btc_sats_money(0));

        let mut claim_paid = fixture_receipt(
            "receipt-claim-1",
            "earn.claim.paid.v1",
            1_762_000_012_500,
            "claim-1",
            false,
        );
        claim_paid.hints.notional = Some(btc_sats_money(15));
        claim_paid.evidence.push(EvidenceRef::new(
            "claim_payout_proof",
            "oa://claims/payout/receipt-claim-1",
            "sha256:claim",
        ));

        let receipts = vec![receipt_paid_1, receipt_paid_2, receipt_failed_1, claim_paid];

        let computed = state
            .compute_minute_snapshot(1_762_000_060_000, receipts.as_slice())
            .expect("snapshot should compute");
        let snapshot = computed.snapshot;
        assert_eq!(snapshot.n, 3);
        assert!((snapshot.sv - (2.0 / 3.0)).abs() < 1e-9);
        assert!((snapshot.sv_effective - snapshot.sv).abs() < 1e-9);
        assert!((snapshot.rho_effective - snapshot.sv_effective).abs() < 1e-9);
        assert_eq!(snapshot.sv_breakdown.len(), 1);
        assert_eq!(snapshot.sv_breakdown[0].total_work_units, 3);
        assert_eq!(snapshot.sv_breakdown[0].verified_work_units, 2);
        assert!(snapshot.delta_m_hat > 0.0);
        assert!(snapshot.xa_hat > 0.0);
        assert_eq!(snapshot.auth_assurance_distribution.len(), 1);
        assert_eq!(
            snapshot.auth_assurance_distribution[0].level,
            AuthAssuranceLevel::Authenticated
        );
        assert_eq!(snapshot.auth_assurance_distribution[0].count, 3);
        assert_eq!(snapshot.personhood_verified_share, 0.0);
        assert_eq!(
            snapshot.liability_premiums_collected_24h,
            btc_sats_money(30)
        );
        assert_eq!(snapshot.claims_paid_24h, btc_sats_money(15));
        assert_eq!(snapshot.bonded_exposure_24h, btc_sats_money(350));
        assert_eq!(snapshot.capital_reserves_24h, btc_sats_money(15));
        assert!((snapshot.loss_ratio - 0.5).abs() < 1e-9);
        assert!((snapshot.capital_coverage_ratio - (15.0 / 350.0)).abs() < 1e-9);
        assert!(snapshot.drift_alerts_24h > 0);
        assert!(!snapshot.drift_signals.is_empty());
        assert!(!snapshot.top_drift_signals.is_empty());
        assert!(snapshot.top_drift_signals.len() <= snapshot.drift_signals.len());
        assert_eq!(snapshot.rollback_attempts_24h, 0);
        assert_eq!(snapshot.rollback_successes_24h, 0);
        assert_eq!(snapshot.rollback_success_rate, 0.0);
        assert!(snapshot.top_rollback_reason_codes.is_empty());
    }

    #[test]
    fn sv_effective_discount_function_handles_edge_cases() {
        let no_correlation = correlation_adjusted_sv_effective(1.0, 0.0, 0.0, 0.0);
        assert!((no_correlation - 1.0).abs() < 1e-9);

        let maximal_penalty = correlation_adjusted_sv_effective(1.0, 1.0, 0.0, 0.0);
        assert!((maximal_penalty - 0.0).abs() < 1e-9);

        let lineage_offset = correlation_adjusted_sv_effective(0.8, 0.5, 0.5, 0.5);
        assert!((lineage_offset - 0.6).abs() < 1e-9);

        let clamped = correlation_adjusted_sv_effective(1.5, 2.0, 2.0, 2.0);
        assert!((clamped - 0.5).abs() < 1e-9);
    }

    #[test]
    fn snapshot_applies_correlation_adjusted_sv_effective() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("snapshots-sv-effective.json");
        let mut state = EconomySnapshotState::from_snapshot_path_for_tests(path);

        let mut paid_correlated = fixture_receipt(
            "receipt-paid-correlated",
            "earn.job.settlement_observed.v1",
            1_762_000_010_000,
            "job-correlated",
            true,
        );
        paid_correlated.hints.verification_correlated = Some(true);

        let paid_uncorrelated = fixture_receipt(
            "receipt-paid-uncorrelated",
            "earn.job.settlement_observed.v1",
            1_762_000_011_000,
            "job-uncorrelated",
            true,
        );
        let failed = fixture_receipt(
            "receipt-failed-uncorrelated",
            "earn.job.failed.v1",
            1_762_000_012_000,
            "job-failed",
            false,
        );

        let snapshot = state
            .compute_minute_snapshot(
                1_762_000_060_000,
                &[paid_correlated, paid_uncorrelated, failed],
            )
            .expect("snapshot should compute")
            .snapshot;

        assert!((snapshot.sv - (2.0 / 3.0)).abs() < 1e-9);
        assert!((snapshot.correlated_verification_share - 0.5).abs() < 1e-9);
        assert!((snapshot.sv_effective - (1.0 / 3.0)).abs() < 1e-9);
        assert!(snapshot.sv_effective < snapshot.sv);
        assert!((snapshot.rho_effective - snapshot.sv_effective).abs() < 1e-9);
    }

    #[test]
    fn snapshot_tracks_personhood_share_from_terminal_receipts() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("snapshots.json");
        let mut state = EconomySnapshotState::from_snapshot_path_for_tests(path);
        let mut personhood_receipt = fixture_receipt(
            "receipt-personhood-1",
            "earn.job.settlement_observed.v1",
            1_762_000_013_000,
            "job-4",
            true,
        );
        personhood_receipt.hints.auth_assurance_level = Some(AuthAssuranceLevel::Personhood);
        personhood_receipt.hints.personhood_proved = Some(true);
        let mut authenticated_receipt = fixture_receipt(
            "receipt-auth-1",
            "earn.job.settlement_observed.v1",
            1_762_000_014_000,
            "job-5",
            true,
        );
        authenticated_receipt.hints.auth_assurance_level = Some(AuthAssuranceLevel::Authenticated);
        authenticated_receipt.hints.personhood_proved = Some(false);

        let computed = state
            .compute_minute_snapshot(
                1_762_000_060_000,
                &[personhood_receipt, authenticated_receipt],
            )
            .expect("snapshot should compute");
        let snapshot = computed.snapshot;
        assert!((snapshot.personhood_verified_share - 0.5).abs() < 1e-9);
        assert_eq!(snapshot.auth_assurance_distribution.len(), 2);
    }

    #[test]
    fn estimators_are_deterministic_for_equivalent_receipt_sets() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path_a = temp_dir.path().join("snapshots-a.json");
        let path_b = temp_dir.path().join("snapshots-b.json");
        let mut state_a = EconomySnapshotState::from_snapshot_path_for_tests(path_a);
        let mut state_b = EconomySnapshotState::from_snapshot_path_for_tests(path_b);

        let paid = fixture_receipt(
            "receipt-paid-1",
            "earn.job.settlement_observed.v1",
            1_762_000_010_000,
            "job-1",
            true,
        );
        let failed = fixture_receipt(
            "receipt-failed-1",
            "earn.job.failed.v1",
            1_762_000_011_000,
            "job-2",
            false,
        );
        let mut throttle = fixture_receipt(
            "receipt-throttle-1",
            "economy.policy.throttle_action_applied.v1",
            1_762_000_012_000,
            "job-3",
            false,
        );
        throttle.hints.reason_code = Some("POLICY_THROTTLE_TRIGGERED".to_string());

        let first = state_a
            .compute_minute_snapshot(
                1_762_000_060_000,
                &[paid.clone(), failed.clone(), throttle.clone()],
            )
            .expect("snapshot A should compute");
        let second = state_b
            .compute_minute_snapshot(1_762_000_060_000, &[throttle, failed, paid])
            .expect("snapshot B should compute");

        assert_eq!(first.snapshot.delta_m_hat, second.snapshot.delta_m_hat);
        assert_eq!(first.snapshot.xa_hat, second.snapshot.xa_hat);
        assert_eq!(
            first.snapshot.drift_alerts_24h,
            second.snapshot.drift_alerts_24h
        );
        assert_eq!(
            first.snapshot.top_drift_signals,
            second.snapshot.top_drift_signals
        );
        assert_eq!(first.snapshot.snapshot_hash, second.snapshot.snapshot_hash);
    }

    #[test]
    fn estimators_respect_windowing_and_increase_with_recent_adverse_signals() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let outside_path = temp_dir.path().join("snapshots-window-outside.json");
        let inside_path = temp_dir.path().join("snapshots-window-inside.json");
        let mut outside_state = EconomySnapshotState::from_snapshot_path_for_tests(outside_path);
        let mut inside_state = EconomySnapshotState::from_snapshot_path_for_tests(inside_path);
        let as_of_ms = 1_762_000_080_000;

        let paid = fixture_receipt(
            "receipt-paid-1",
            "earn.job.settlement_observed.v1",
            1_762_000_010_000,
            "job-1",
            true,
        );
        let mut stale_failed = fixture_receipt(
            "receipt-failed-stale",
            "earn.job.failed.v1",
            as_of_ms - 90_000_000,
            "job-2",
            false,
        );
        stale_failed.hints.reason_code = Some("JOB_FAILED".to_string());

        let mut recent_failed = fixture_receipt(
            "receipt-failed-recent",
            "earn.job.failed.v1",
            as_of_ms - 10_000,
            "job-2",
            false,
        );
        recent_failed.hints.reason_code = Some("JOB_FAILED".to_string());

        let outside_snapshot = outside_state
            .compute_minute_snapshot(as_of_ms, &[paid.clone(), stale_failed])
            .expect("outside snapshot should compute")
            .snapshot;
        let inside_snapshot = inside_state
            .compute_minute_snapshot(as_of_ms, &[paid, recent_failed])
            .expect("inside snapshot should compute")
            .snapshot;

        assert!(
            inside_snapshot.delta_m_hat > outside_snapshot.delta_m_hat,
            "inside delta_m_hat={} outside delta_m_hat={}",
            inside_snapshot.delta_m_hat,
            outside_snapshot.delta_m_hat
        );
        assert!(
            inside_snapshot.xa_hat > outside_snapshot.xa_hat,
            "inside xa_hat={} outside xa_hat={}",
            inside_snapshot.xa_hat,
            outside_snapshot.xa_hat
        );
        assert!(
            inside_snapshot.drift_alerts_24h >= outside_snapshot.drift_alerts_24h,
            "inside drift_alerts_24h={} outside drift_alerts_24h={}",
            inside_snapshot.drift_alerts_24h,
            outside_snapshot.drift_alerts_24h
        );
    }

    #[test]
    fn top_drift_signals_are_sorted_by_risk_score() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("snapshots-drift-ordering.json");
        let mut state = EconomySnapshotState::from_snapshot_path_for_tests(path);
        let mut failed = fixture_receipt(
            "receipt-failed-order-1",
            "earn.job.failed.v1",
            1_762_000_010_000,
            "job-order-1",
            false,
        );
        failed.hints.reason_code = Some("JOB_FAILED".to_string());
        let mut claim = fixture_receipt(
            "receipt-claim-order-1",
            "earn.claim.paid.v1",
            1_762_000_011_000,
            "job-order-1",
            false,
        );
        claim.hints.notional = Some(btc_sats_money(25));
        claim.evidence.push(EvidenceRef::new(
            "claim_payout_proof",
            "oa://claims/payout/receipt-claim-order-1",
            "sha256:claim-order-1",
        ));

        let snapshot = state
            .compute_minute_snapshot(1_762_000_060_000, &[failed, claim])
            .expect("snapshot should compute")
            .snapshot;
        assert!(!snapshot.top_drift_signals.is_empty());
        let mut previous = f64::INFINITY;
        for signal in &snapshot.top_drift_signals {
            assert!(signal.score <= previous + 1e-9);
            previous = signal.score;
        }
    }

    #[test]
    fn snapshot_aggregates_incident_buckets_by_taxonomy_and_severity() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("snapshots-incident-buckets.json");
        let mut state = EconomySnapshotState::from_snapshot_path_for_tests(path);
        let paid = fixture_receipt(
            "receipt-paid-incident-bucket",
            "earn.job.settlement_observed.v1",
            1_762_000_010_000,
            "job-incident-bucket",
            true,
        );
        let incident = fixture_incident_receipt(
            "receipt-incident-1",
            1_762_000_011_000,
            "incident",
            "ops.execution_failure",
            "high",
        );
        let near_miss = fixture_incident_receipt(
            "receipt-near-miss-1",
            1_762_000_012_000,
            "near_miss",
            "ops.execution_failure",
            "high",
        );
        let snapshot = state
            .compute_minute_snapshot(1_762_000_060_000, &[paid, incident, near_miss])
            .expect("snapshot should compute")
            .snapshot;
        assert_eq!(snapshot.incident_buckets.len(), 1);
        let bucket = &snapshot.incident_buckets[0];
        assert_eq!(bucket.taxonomy_code, "ops.execution_failure");
        assert_eq!(bucket.severity, SeverityClass::High);
        assert_eq!(bucket.incident_reports_24h, 1);
        assert_eq!(bucket.near_misses_24h, 1);
        assert!((bucket.incident_rate - 1.0).abs() < 1e-9);
        assert!((bucket.near_miss_rate - 1.0).abs() < 1e-9);
    }

    #[test]
    fn snapshot_aggregates_safety_signal_buckets_by_taxonomy_and_class() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("snapshots-safety-signals.json");
        let mut state = EconomySnapshotState::from_snapshot_path_for_tests(path);
        let paid = fixture_receipt(
            "receipt-paid-safety-signal",
            "earn.job.settlement_observed.v1",
            1_762_000_010_000,
            "job-safety-signal",
            true,
        );
        let incident = fixture_incident_receipt(
            "receipt-incident-safety-signal",
            1_762_000_011_000,
            "incident",
            "ops.execution_failure",
            "high",
        );
        let drift = fixture_drift_alert_receipt("receipt-drift-safety-signal", 1_762_000_012_000);
        let throttle =
            fixture_policy_throttle_receipt("receipt-throttle-safety-signal", 1_762_000_013_000);

        let snapshot = state
            .compute_minute_snapshot(1_762_000_060_000, &[paid, incident, drift, throttle])
            .expect("snapshot should compute")
            .snapshot;
        assert_eq!(snapshot.safety_signal_buckets.len(), 3);
        assert!(snapshot.safety_signal_buckets.iter().any(|bucket| {
            bucket.taxonomy_code == "ops.execution_failure"
                && bucket.incident_signals_24h == 1
                && bucket.drift_signals_24h == 0
                && bucket.adverse_signals_24h == 0
        }));
        assert!(snapshot.safety_signal_buckets.iter().any(|bucket| {
            bucket.taxonomy_code == "drift.sv_below_floor"
                && bucket.incident_signals_24h == 0
                && bucket.drift_signals_24h == 1
                && bucket.adverse_signals_24h == 0
        }));
        assert!(snapshot.safety_signal_buckets.iter().any(|bucket| {
            bucket.taxonomy_code == "policy.throttle"
                && bucket.incident_signals_24h == 0
                && bucket.drift_signals_24h == 0
                && bucket.adverse_signals_24h == 1
        }));
    }

    #[test]
    fn snapshot_aggregates_certification_distribution_and_uncertified_block_rate() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir
            .path()
            .join("snapshots-certification-distribution.json");
        let mut state = EconomySnapshotState::from_snapshot_path_for_tests(path);
        let paid = fixture_receipt(
            "receipt-paid-certification-distribution",
            "earn.job.settlement_observed.v1",
            1_762_000_010_000,
            "job-certification-distribution-1",
            true,
        );
        let mut withheld = fixture_receipt(
            "receipt-withheld-certification-distribution",
            "earn.job.withheld.v1",
            1_762_000_011_000,
            "job-certification-distribution-2",
            false,
        );
        withheld.hints.reason_code = Some(REASON_CODE_DIGITAL_BORDER_BLOCK_UNCERTIFIED.to_string());
        let cert_active = fixture_certification_receipt(
            "receipt-cert-active",
            "economy.certification.issued.v1",
            1_762_000_012_000,
            "cert.active",
            1,
            "active",
            1_762_010_000_000,
        );
        let cert_revoked = fixture_certification_receipt(
            "receipt-cert-revoked",
            "economy.certification.revoked.v1",
            1_762_000_013_000,
            "cert.revoked",
            2,
            "revoked",
            1_762_010_000_000,
        );
        let cert_expired = fixture_certification_receipt(
            "receipt-cert-expired",
            "economy.certification.issued.v1",
            1_762_000_014_000,
            "cert.expired",
            1,
            "active",
            1_762_000_040_000,
        );

        let snapshot = state
            .compute_minute_snapshot(
                1_762_000_120_000,
                &[paid, withheld, cert_active, cert_revoked, cert_expired],
            )
            .expect("snapshot should compute")
            .snapshot;
        assert_eq!(snapshot.uncertified_block_count_24h, 1);
        assert!((snapshot.uncertified_block_rate - 0.5).abs() < 1e-9);
        assert!(
            snapshot
                .certification_distribution
                .iter()
                .any(|row| row.category == "compute"
                    && row.tfb_class == FeedbackLatencyClass::Short
                    && row.min_severity == SeverityClass::High
                    && row.max_severity == SeverityClass::Critical
                    && row.certification_level == "level_2")
        );
        let total_active = snapshot
            .certification_distribution
            .iter()
            .map(|row| row.active_count)
            .fold(0u64, u64::saturating_add);
        let total_revoked = snapshot
            .certification_distribution
            .iter()
            .map(|row| row.revoked_count)
            .fold(0u64, u64::saturating_add);
        let total_expired = snapshot
            .certification_distribution
            .iter()
            .map(|row| row.expired_count)
            .fold(0u64, u64::saturating_add);
        assert_eq!(total_active, 1);
        assert_eq!(total_revoked, 1);
        assert_eq!(total_expired, 1);
    }

    #[test]
    fn snapshot_aggregates_simulation_scenario_exportable_and_backlog_counts() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("snapshots-simulation-scenarios.json");
        let mut state = EconomySnapshotState::from_snapshot_path_for_tests(path);
        let paid = fixture_receipt(
            "receipt-paid-simulation-scenario-counts",
            "earn.job.settlement_observed.v1",
            1_762_000_010_000,
            "job-simulation-scenario-counts",
            true,
        );
        let mut ground_truth_a = fixture_incident_receipt(
            "receipt-ground-truth-a",
            1_762_000_011_000,
            "ground_truth_case",
            "safety.ground_truth_case",
            "high",
        );
        let mut ground_truth_b = fixture_incident_receipt(
            "receipt-ground-truth-b",
            1_762_000_012_000,
            "ground_truth_case",
            "safety.ground_truth_case",
            "medium",
        );
        let ground_truth_a_ref = ground_truth_a
            .evidence
            .iter_mut()
            .find(|evidence| evidence.kind == "incident_object_ref")
            .expect("ground truth A incident ref");
        ground_truth_a_ref.meta.insert(
            "incident_id".to_string(),
            serde_json::json!("ground_truth.case.a"),
        );
        let ground_truth_a_digest = ground_truth_a_ref.digest.clone();
        let ground_truth_b_ref = ground_truth_b
            .evidence
            .iter_mut()
            .find(|evidence| evidence.kind == "incident_object_ref")
            .expect("ground truth B incident ref");
        ground_truth_b_ref.meta.insert(
            "incident_id".to_string(),
            serde_json::json!("ground_truth.case.b"),
        );
        let export_a = fixture_simulation_scenario_export_receipt(
            "receipt-simulation-export-a",
            1_762_000_013_000,
            "ground_truth.case.a",
            ground_truth_a_digest.as_str(),
        );

        let snapshot = state
            .compute_minute_snapshot(
                1_762_000_120_000,
                &[paid, ground_truth_a, ground_truth_b, export_a],
            )
            .expect("snapshot should compute")
            .snapshot;
        assert_eq!(snapshot.exportable_simulation_scenarios, 2);
        assert_eq!(snapshot.simulation_scenario_backlog, 1);
    }

    #[test]
    fn snapshot_aggregates_anchor_publication_status() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("snapshots-anchor-status.json");
        let mut state = EconomySnapshotState::from_snapshot_path_for_tests(path);
        let paid = fixture_receipt(
            "receipt-paid-anchor-status",
            "earn.job.settlement_observed.v1",
            1_762_000_010_000,
            "job-anchor-status",
            true,
        );
        let anchor_a = fixture_anchor_publication_receipt(
            "receipt-anchor-bitcoin-1",
            1_762_000_011_000,
            "bitcoin",
            "snapshot.economy:1762000060000",
        );
        let anchor_b = fixture_anchor_publication_receipt(
            "receipt-anchor-nostr-1",
            1_762_000_012_000,
            "nostr",
            "snapshot.economy:1762000120000",
        );

        let snapshot = state
            .compute_minute_snapshot(1_762_000_120_000, &[paid, anchor_a, anchor_b])
            .expect("snapshot should compute")
            .snapshot;
        assert_eq!(snapshot.anchor_publications_24h, 2);
        assert_eq!(snapshot.anchored_snapshots_24h, 2);
        assert!(
            snapshot
                .anchor_backend_distribution
                .iter()
                .any(|row| row.anchor_backend == "bitcoin" && row.publications_24h == 1)
        );
        assert!(
            snapshot
                .anchor_backend_distribution
                .iter()
                .any(|row| row.anchor_backend == "nostr" && row.publications_24h == 1)
        );
    }

    #[test]
    fn snapshot_aggregates_rollback_attempts_success_rate_and_reason_codes() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("snapshots-rollbacks.json");
        let mut state = EconomySnapshotState::from_snapshot_path_for_tests(path);
        let paid = fixture_receipt(
            "receipt-paid-rollback-bucket",
            "earn.job.settlement_observed.v1",
            1_762_000_010_000,
            "job-rollback-bucket",
            true,
        );
        let rollback_executed = fixture_rollback_receipt(
            "receipt-rollback-executed-1",
            "economy.rollback.executed.v1",
            1_762_000_012_000,
            "job-rollback-bucket",
            "ROLLBACK_EXECUTED",
        );
        let rollback_failed = fixture_rollback_receipt(
            "receipt-rollback-failed-1",
            "economy.rollback.failed.v1",
            1_762_000_013_000,
            "job-rollback-bucket",
            "ROLLBACK_FAILED",
        );
        let compensating = fixture_rollback_receipt(
            "receipt-compensating-action-1",
            "economy.compensating_action.executed.v1",
            1_762_000_014_000,
            "job-rollback-bucket",
            "COMPENSATING_ACTION_EXECUTED",
        );

        let snapshot = state
            .compute_minute_snapshot(
                1_762_000_060_000,
                &[paid, rollback_executed, rollback_failed, compensating],
            )
            .expect("snapshot should compute")
            .snapshot;
        assert_eq!(snapshot.rollback_attempts_24h, 3);
        assert_eq!(snapshot.rollback_successes_24h, 2);
        assert!((snapshot.rollback_success_rate - (2.0 / 3.0)).abs() < 1e-9);
        assert_eq!(snapshot.top_rollback_reason_codes.len(), 3);
        assert!(
            snapshot
                .top_rollback_reason_codes
                .iter()
                .any(|row| row.reason_code == "ROLLBACK_EXECUTED" && row.count_24h == 1)
        );
        assert!(
            snapshot
                .top_rollback_reason_codes
                .iter()
                .any(|row| row.reason_code == "ROLLBACK_FAILED" && row.count_24h == 1)
        );
    }

    #[test]
    fn snapshot_aggregates_outcome_distribution_and_key_rates() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("snapshots-outcomes.json");
        let mut state = EconomySnapshotState::from_snapshot_path_for_tests(path);
        let paid = fixture_receipt(
            "receipt-paid-outcome-bucket",
            "earn.job.settlement_observed.v1",
            1_762_000_010_000,
            "job-outcome-bucket",
            true,
        );
        let outcome_a = fixture_outcome_registry_receipt(
            "receipt-outcome-a",
            1_762_000_011_000,
            "job-outcome-bucket",
            "verified",
            "settled",
            Some("none"),
            Some("none"),
        );
        let outcome_b = fixture_outcome_registry_receipt(
            "receipt-outcome-b",
            1_762_000_012_000,
            "job-outcome-bucket",
            "contested",
            "failed",
            Some("paid"),
            Some("rollback_executed"),
        );

        let snapshot = state
            .compute_minute_snapshot(1_762_000_060_000, &[paid, outcome_a, outcome_b])
            .expect("snapshot should compute")
            .snapshot;
        assert_eq!(snapshot.outcome_distribution.len(), 2);
        assert!(
            snapshot
                .outcome_distribution
                .iter()
                .all(|row| (row.share_24h - 0.5).abs() < 1e-9)
        );
        assert_eq!(snapshot.outcome_key_rates.len(), 1);
        let row = &snapshot.outcome_key_rates[0];
        assert_eq!(row.category, "compute");
        assert_eq!(row.tfb_class, FeedbackLatencyClass::Short);
        assert_eq!(row.severity, SeverityClass::Low);
        assert_eq!(row.entries_24h, 2);
        assert!((row.settlement_success_rate - 0.5).abs() < 1e-9);
        assert!((row.claim_rate - 0.5).abs() < 1e-9);
        assert!((row.remedy_rate - 0.5).abs() < 1e-9);
    }

    #[test]
    fn snapshot_inputs_are_public_safe_and_redacted() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("snapshots.json");
        let mut state = EconomySnapshotState::from_snapshot_path_for_tests(path);
        let receipts = vec![fixture_receipt(
            "receipt-paid-1",
            "earn.job.settlement_observed.v1",
            1_762_000_010_000,
            "job-1",
            true,
        )];

        let computed = state
            .compute_minute_snapshot(1_762_000_012_000, receipts.as_slice())
            .expect("snapshot should compute");
        assert!(
            computed
                .snapshot
                .inputs
                .iter()
                .all(|input| !input.uri.contains("wallet/payments"))
        );
        assert!(
            computed
                .snapshot
                .inputs
                .iter()
                .all(|input| !input.uri.contains("invoice"))
        );
        assert!(
            computed
                .snapshot
                .inputs
                .iter()
                .all(|input| !input.uri.contains("preimage"))
        );
    }
}
