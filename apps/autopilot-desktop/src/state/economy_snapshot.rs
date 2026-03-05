use crate::app_state::PaneLoadState;
use crate::economy_kernel_receipts::{
    AuthAssuranceLevel, EvidenceRef, FeedbackLatencyClass, ProvenanceGrade, Receipt, SeverityClass,
    VerificationTier,
};
use bitcoin::hashes::{sha256, Hash};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const ECONOMY_SNAPSHOT_SCHEMA_VERSION: u16 = 1;
const ECONOMY_SNAPSHOT_STREAM_ID: &str = "stream.economy_snapshots.v1";
const ECONOMY_SNAPSHOT_WINDOW_MS: i64 = 86_400_000;
const ECONOMY_SNAPSHOT_RETENTION_LIMIT: usize = 10_080;

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
    let mut correlated_verified = 0u64;
    let mut provenance_counts = [0u64; 4];
    let mut auth_assurance_counts = BTreeMap::<AuthAssuranceLevel, u64>::new();
    let mut personhood_verified = 0u64;

    for receipt in terminal_by_work_unit.values() {
        total_work_units = total_work_units.saturating_add(1);
        let verified = is_verified_terminal_receipt(receipt);
        if verified {
            verified_work_units = verified_work_units.saturating_add(1);
            if receipt.hints.verification_correlated.unwrap_or(false) {
                correlated_verified = correlated_verified.saturating_add(1);
            }
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

    let inputs = snapshot_input_evidence(window_start_ms, as_of_ms, scoped_receipts.as_slice());
    let snapshot_hash = snapshot_hash_for(
        snapshot_id,
        as_of_ms,
        sv,
        rho,
        total_work_units,
        nv,
        0.0,
        0.0,
        correlated_verification_share,
        provenance_p0_share,
        provenance_p1_share,
        provenance_p2_share,
        provenance_p3_share,
        personhood_verified_share,
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
        delta_m_hat: 0.0,
        xa_hat: 0.0,
        correlated_verification_share,
        provenance_p0_share,
        provenance_p1_share,
        provenance_p2_share,
        provenance_p3_share,
        auth_assurance_distribution,
        personhood_verified_share,
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

fn is_verified_terminal_receipt(receipt: &Receipt) -> bool {
    receipt.receipt_type == "earn.job.settlement_observed.v1"
        && receipt
            .evidence
            .iter()
            .any(|evidence| evidence.kind == "wallet_settlement_proof")
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
        })
        .build()
        .expect("fixture receipt should build")
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
        assert!(reloaded
            .get_snapshot("snapshot.economy:1761999960000")
            .is_some());
    }

    #[test]
    fn snapshot_metrics_are_derived_from_terminal_receipts() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("snapshots.json");
        let mut state = EconomySnapshotState::from_snapshot_path_for_tests(path);
        let receipts = vec![
            fixture_receipt(
                "receipt-paid-1",
                "earn.job.settlement_observed.v1",
                1_762_000_010_000,
                "job-1",
                true,
            ),
            fixture_receipt(
                "receipt-paid-2",
                "earn.job.settlement_observed.v1",
                1_762_000_011_000,
                "job-2",
                true,
            ),
            fixture_receipt(
                "receipt-failed-1",
                "earn.job.failed.v1",
                1_762_000_012_000,
                "job-3",
                false,
            ),
        ];

        let computed = state
            .compute_minute_snapshot(1_762_000_060_000, receipts.as_slice())
            .expect("snapshot should compute");
        let snapshot = computed.snapshot;
        assert_eq!(snapshot.n, 3);
        assert!((snapshot.sv - (2.0 / 3.0)).abs() < 1e-9);
        assert_eq!(snapshot.sv_breakdown.len(), 1);
        assert_eq!(snapshot.sv_breakdown[0].total_work_units, 3);
        assert_eq!(snapshot.sv_breakdown[0].verified_work_units, 2);
        assert_eq!(snapshot.delta_m_hat, 0.0);
        assert_eq!(snapshot.xa_hat, 0.0);
        assert_eq!(snapshot.auth_assurance_distribution.len(), 1);
        assert_eq!(
            snapshot.auth_assurance_distribution[0].level,
            AuthAssuranceLevel::Authenticated
        );
        assert_eq!(snapshot.auth_assurance_distribution[0].count, 3);
        assert_eq!(snapshot.personhood_verified_share, 0.0);
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
        assert!(computed
            .snapshot
            .inputs
            .iter()
            .all(|input| !input.uri.contains("wallet/payments")));
        assert!(computed
            .snapshot
            .inputs
            .iter()
            .all(|input| !input.uri.contains("invoice")));
        assert!(computed
            .snapshot
            .inputs
            .iter()
            .all(|input| !input.uri.contains("preimage")));
    }
}
