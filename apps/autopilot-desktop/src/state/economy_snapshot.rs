use crate::app_state::PaneLoadState;
use crate::economy_kernel_receipts::{
    Asset, AuthAssuranceLevel, DriftSignalSummary, EvidenceRef, FeedbackLatencyClass, Money,
    MoneyAmount, ProvenanceGrade, Receipt, SeverityClass, VerificationTier,
};
use bitcoin::hashes::{Hash, sha256};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::BTreeMap;
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
    pub rho: f64,
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
    pub rollback_attempts_24h: u64,
    #[serde(default)]
    pub rollback_successes_24h: u64,
    #[serde(default)]
    pub rollback_success_rate: f64,
    #[serde(default)]
    pub top_rollback_reason_codes: Vec<RollbackReasonCodeRow>,
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
        let snapshot_id = snapshot_id_for(as_of_ms);
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
    let window_start_ms = as_of_ms.saturating_sub(ECONOMY_SNAPSHOT_WINDOW_MS);
    let mut scoped_receipts = receipts
        .iter()
        .filter(|receipt| {
            receipt.created_at_ms <= as_of_ms && receipt.created_at_ms > window_start_ms
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
    let rho = sv;
    let nv = rho * (total_work_units as f64);
    let correlated_verification_share = ratio(correlated_verified, verified_work_units);
    let provenance_p0_share = ratio(provenance_counts[0], total_work_units);
    let provenance_p1_share = ratio(provenance_counts[1], total_work_units);
    let provenance_p2_share = ratio(provenance_counts[2], total_work_units);
    let provenance_p3_share = ratio(provenance_counts[3], total_work_units);
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
        rho,
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
        rho,
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
        rollback_attempts_24h: rollback_signal_count,
        rollback_successes_24h,
        rollback_success_rate,
        top_rollback_reason_codes,
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
    rho: f64,
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
    rho: f64,
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
        rho,
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

fn floor_to_minute_utc(value_ms: i64) -> i64 {
    value_ms.div_euclid(60_000) * 60_000
}

fn snapshot_id_for(as_of_ms: i64) -> String {
    format!("snapshot.economy:{as_of_ms}")
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
            "taxonomy_code".to_string(),
            serde_json::json!(taxonomy_code),
        );
        incident_ref
            .meta
            .insert("severity".to_string(), serde_json::json!(severity_label));
        receipt.evidence.push(incident_ref);
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
