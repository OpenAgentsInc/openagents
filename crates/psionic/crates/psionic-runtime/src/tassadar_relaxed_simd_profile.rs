use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_RELAXED_SIMD_RESEARCH_PROFILE_ID: &str =
    "tassadar.research_profile.relaxed_simd_accelerator.v1";
pub const TASSADAR_RELAXED_SIMD_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_relaxed_simd_runtime_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRelaxedSimdBackendPosture {
    ExactAnchor,
    ApproximateBounded,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRelaxedSimdBackendRow {
    pub backend_id: String,
    pub workload_family: String,
    pub semantics_family_id: String,
    pub posture: TassadarRelaxedSimdBackendPosture,
    pub drift_bps: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRelaxedSimdRuntimeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub research_profile_id: String,
    pub exact_anchor_backend_ids: Vec<String>,
    pub approximate_backend_ids: Vec<String>,
    pub refused_backend_ids: Vec<String>,
    pub rows: Vec<TassadarRelaxedSimdBackendRow>,
    pub non_promotion_gate_reason_ids: Vec<String>,
    pub overall_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[must_use]
pub fn build_tassadar_relaxed_simd_runtime_report() -> TassadarRelaxedSimdRuntimeReport {
    let rows = vec![
        row(
            "cpu_reference_anchor",
            "simd_reduce_kernel",
            "deterministic_scalar_anchor",
            TassadarRelaxedSimdBackendPosture::ExactAnchor,
            0,
            None,
            &["fixtures/tassadar/reports/tassadar_simd_profile_runtime_report.json"],
            "cpu-reference scalar anchor stays exact and is the baseline the relaxed-SIMD ladder is measured against",
        ),
        row(
            "metal_relaxed_simd_candidate",
            "simd_reduce_kernel",
            "relaxed_simd_fused_reduce",
            TassadarRelaxedSimdBackendPosture::ApproximateBounded,
            12,
            None,
            &["fixtures/tassadar/reports/tassadar_simd_profile_runtime_report.json"],
            "metal relaxed-SIMD stays research-only with explicit bounded drift instead of claiming deterministic parity",
        ),
        row(
            "cuda_relaxed_simd_candidate",
            "simd_reduce_kernel",
            "relaxed_simd_fused_reduce",
            TassadarRelaxedSimdBackendPosture::ApproximateBounded,
            18,
            None,
            &["fixtures/tassadar/reports/tassadar_simd_profile_runtime_report.json"],
            "cuda relaxed-SIMD stays research-only with explicit bounded drift instead of claiming deterministic parity",
        ),
        row(
            "metal_lane_shuffle_relaxed",
            "simd_lane_shuffle_kernel",
            "relaxed_lane_shuffle",
            TassadarRelaxedSimdBackendPosture::Refused,
            0,
            Some("relaxed_lane_semantics_unstable"),
            &["fixtures/tassadar/reports/tassadar_simd_profile_runtime_report.json"],
            "lane-shuffle relaxed semantics remain typed refusal because backend-specific reorder rules are not stable enough for even research-ladder promotion",
        ),
        row(
            "cross_backend_relaxed_simd",
            "simd_reduce_kernel",
            "cross_backend_equivalence",
            TassadarRelaxedSimdBackendPosture::Refused,
            0,
            Some("accelerator_specific_nonportable"),
            &["fixtures/tassadar/reports/tassadar_simd_profile_runtime_report.json"],
            "cross-backend relaxed-SIMD equivalence remains typed refusal because accelerator-specific semantics are not portable enough to flatten into one claim",
        ),
    ];
    let exact_anchor_backend_ids = rows
        .iter()
        .filter(|row| row.posture == TassadarRelaxedSimdBackendPosture::ExactAnchor)
        .map(|row| row.backend_id.clone())
        .collect::<Vec<_>>();
    let approximate_backend_ids = rows
        .iter()
        .filter(|row| row.posture == TassadarRelaxedSimdBackendPosture::ApproximateBounded)
        .map(|row| row.backend_id.clone())
        .collect::<Vec<_>>();
    let refused_backend_ids = rows
        .iter()
        .filter(|row| row.posture == TassadarRelaxedSimdBackendPosture::Refused)
        .map(|row| row.backend_id.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarRelaxedSimdRuntimeReport {
        schema_version: 1,
        report_id: String::from("tassadar.relaxed_simd.runtime_report.v1"),
        research_profile_id: String::from(TASSADAR_RELAXED_SIMD_RESEARCH_PROFILE_ID),
        exact_anchor_backend_ids,
        approximate_backend_ids,
        refused_backend_ids,
        rows,
        non_promotion_gate_reason_ids: vec![
            String::from("approximate_accelerator_drift"),
            String::from("relaxed_lane_semantics_unstable"),
            String::from("accelerator_specific_nonportable"),
        ],
        overall_green: true,
        claim_boundary: String::from(
            "this runtime report freezes one research-only relaxed-SIMD ladder with an exact cpu-reference anchor, bounded accelerator drift rows, and typed refusal on unstable lane semantics and nonportable cross-backend equivalence. It does not claim deterministic relaxed-SIMD closure, public profile promotion, or broader served publication",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Relaxed-SIMD runtime report covers exact_anchor_backends={}, approximate_backends={}, refused_backends={}, non_promotion_gates={}.",
        report.exact_anchor_backend_ids.len(),
        report.approximate_backend_ids.len(),
        report.refused_backend_ids.len(),
        report.non_promotion_gate_reason_ids.len(),
    );
    report.report_digest = stable_digest(b"psionic_tassadar_relaxed_simd_runtime_report|", &report);
    report
}

#[must_use]
pub fn tassadar_relaxed_simd_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_RELAXED_SIMD_RUNTIME_REPORT_REF)
}

pub fn write_tassadar_relaxed_simd_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarRelaxedSimdRuntimeReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_relaxed_simd_runtime_report();
    let json = serde_json::to_string_pretty(&report).expect("report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn row(
    backend_id: &str,
    workload_family: &str,
    semantics_family_id: &str,
    posture: TassadarRelaxedSimdBackendPosture,
    drift_bps: u32,
    refusal_reason_id: Option<&str>,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarRelaxedSimdBackendRow {
    TassadarRelaxedSimdBackendRow {
        backend_id: String::from(backend_id),
        workload_family: String::from(workload_family),
        semantics_family_id: String::from(semantics_family_id),
        posture,
        drift_bps,
        refusal_reason_id: refusal_reason_id.map(String::from),
        benchmark_refs: benchmark_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, Box<dyn std::error::Error>> {
    let path = path.as_ref();
    let json = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        TASSADAR_RELAXED_SIMD_RESEARCH_PROFILE_ID, TassadarRelaxedSimdRuntimeReport,
        build_tassadar_relaxed_simd_runtime_report, read_json,
        tassadar_relaxed_simd_runtime_report_path, write_tassadar_relaxed_simd_runtime_report,
    };

    #[test]
    fn relaxed_simd_runtime_report_keeps_anchor_drift_and_refusal_rows_explicit() {
        let report = build_tassadar_relaxed_simd_runtime_report();

        assert_eq!(
            report.research_profile_id,
            TASSADAR_RELAXED_SIMD_RESEARCH_PROFILE_ID
        );
        assert_eq!(
            report.exact_anchor_backend_ids,
            vec![String::from("cpu_reference_anchor")]
        );
        assert_eq!(report.approximate_backend_ids.len(), 2);
        assert_eq!(report.refused_backend_ids.len(), 2);
    }

    #[test]
    fn relaxed_simd_runtime_report_matches_committed_truth() {
        let generated = build_tassadar_relaxed_simd_runtime_report();
        let committed: TassadarRelaxedSimdRuntimeReport =
            read_json(tassadar_relaxed_simd_runtime_report_path()).expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_relaxed_simd_runtime_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_relaxed_simd_runtime_report.json");
        let report =
            write_tassadar_relaxed_simd_runtime_report(&output_path).expect("write report");
        let persisted: TassadarRelaxedSimdRuntimeReport =
            read_json(&output_path).expect("persisted report");
        assert_eq!(report, persisted);
    }
}
