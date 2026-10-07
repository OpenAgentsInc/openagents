use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_SIMD_PROFILE_ID: &str = "tassadar.proposal_profile.simd_deterministic.v1";
pub const TASSADAR_SIMD_PROFILE_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_simd_profile_runtime_report.json";

/// Backend posture for one bounded SIMD row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSimdBackendPosture {
    Exact,
    ScalarFallback,
    Refused,
}

/// One backend row in the bounded SIMD matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSimdBackendRow {
    pub backend_id: String,
    pub workload_family: String,
    pub vector_op_id: String,
    pub posture: TassadarSimdBackendPosture,
    pub exact_output_parity: bool,
    pub exact_trace_parity: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_reason_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

/// Runtime report for the bounded SIMD deterministic profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSimdProfileRuntimeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_id: String,
    pub exact_backend_ids: Vec<String>,
    pub fallback_backend_ids: Vec<String>,
    pub refused_backend_ids: Vec<String>,
    pub exact_case_count: u32,
    pub fallback_case_count: u32,
    pub refusal_case_count: u32,
    pub rows: Vec<TassadarSimdBackendRow>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Builds the canonical runtime report for the bounded SIMD deterministic profile.
#[must_use]
pub fn build_tassadar_simd_profile_runtime_report() -> TassadarSimdProfileRuntimeReport {
    let rows = vec![
        row(
            "cpu_reference_current_host",
            "simd_reduce_kernel",
            "i32x4_add_reduce",
            TassadarSimdBackendPosture::Exact,
            true,
            true,
            None,
            None,
            &["fixtures/tassadar/reports/tassadar_decode_scaling_report.json"],
            "cpu-reference SIMD stays exact on the bounded lane and is the only backend admitted as exact profile truth",
        ),
        row(
            "metal_served",
            "simd_reduce_kernel",
            "i32x4_add_reduce",
            TassadarSimdBackendPosture::ScalarFallback,
            true,
            false,
            Some("vector_backend_not_yet_frozen"),
            None,
            &["fixtures/tassadar/reports/tassadar_decode_scaling_report.json"],
            "metal stays explicit as scalar-fallback posture while vector-lane semantics are not frozen for public exactness",
        ),
        row(
            "cuda_served",
            "simd_reduce_kernel",
            "i32x4_add_reduce",
            TassadarSimdBackendPosture::ScalarFallback,
            true,
            false,
            Some("vector_backend_not_yet_frozen"),
            None,
            &["fixtures/tassadar/reports/tassadar_decode_scaling_report.json"],
            "cuda stays explicit as scalar-fallback posture while vector-lane semantics are not frozen for public exactness",
        ),
        row(
            "accelerator_specific_unbounded",
            "simd_lane_shuffle_kernel",
            "i8x16_lane_shuffle",
            TassadarSimdBackendPosture::Refused,
            false,
            false,
            None,
            Some("accelerator_specific_simd_semantics"),
            &["fixtures/tassadar/reports/tassadar_decode_scaling_report.json"],
            "accelerator-specific SIMD semantics remain typed refusal truth instead of widening from bounded cpu-reference exactness",
        ),
    ];
    let exact_backend_ids = rows
        .iter()
        .filter(|row| row.posture == TassadarSimdBackendPosture::Exact)
        .map(|row| row.backend_id.clone())
        .collect::<Vec<_>>();
    let fallback_backend_ids = rows
        .iter()
        .filter(|row| row.posture == TassadarSimdBackendPosture::ScalarFallback)
        .map(|row| row.backend_id.clone())
        .collect::<Vec<_>>();
    let refused_backend_ids = rows
        .iter()
        .filter(|row| row.posture == TassadarSimdBackendPosture::Refused)
        .map(|row| row.backend_id.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarSimdProfileRuntimeReport {
        schema_version: 1,
        report_id: String::from("tassadar.simd_profile.runtime_report.v1"),
        profile_id: String::from(TASSADAR_SIMD_PROFILE_ID),
        exact_case_count: exact_backend_ids.len() as u32,
        fallback_case_count: fallback_backend_ids.len() as u32,
        refusal_case_count: refused_backend_ids.len() as u32,
        exact_backend_ids,
        fallback_backend_ids,
        refused_backend_ids,
        rows,
        claim_boundary: String::from(
            "this runtime report freezes one bounded SIMD deterministic profile with exact cpu-reference truth, explicit scalar-fallback accelerator rows, and typed accelerator-specific refusal truth. It does not claim arbitrary SIMD closure, accelerator-invariant vector exactness, or broader served publication",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "SIMD runtime report covers {} rows with exact_cases={}, fallback_cases={}, refusal_cases={}.",
        report.rows.len(),
        report.exact_case_count,
        report.fallback_case_count,
        report.refusal_case_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_simd_profile_runtime_report|", &report);
    report
}

#[must_use]
pub fn tassadar_simd_profile_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SIMD_PROFILE_RUNTIME_REPORT_REF)
}

pub fn write_tassadar_simd_profile_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSimdProfileRuntimeReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_simd_profile_runtime_report();
    let json = serde_json::to_string_pretty(&report).expect("simd runtime report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_simd_profile_runtime_report(
    path: impl AsRef<Path>,
) -> Result<TassadarSimdProfileRuntimeReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

#[allow(clippy::too_many_arguments)]
fn row(
    backend_id: &str,
    workload_family: &str,
    vector_op_id: &str,
    posture: TassadarSimdBackendPosture,
    exact_output_parity: bool,
    exact_trace_parity: bool,
    fallback_reason_id: Option<&str>,
    refusal_reason_id: Option<&str>,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarSimdBackendRow {
    TassadarSimdBackendRow {
        backend_id: String::from(backend_id),
        workload_family: String::from(workload_family),
        vector_op_id: String::from(vector_op_id),
        posture,
        exact_output_parity,
        exact_trace_parity,
        fallback_reason_id: fallback_reason_id.map(String::from),
        refusal_reason_id: refusal_reason_id.map(String::from),
        benchmark_refs: benchmark_refs.iter().map(|value| String::from(*value)).collect(),
        note: String::from(note),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, Box<dyn std::error::Error>> {
    let path = path.as_ref();
    let json = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        TASSADAR_SIMD_PROFILE_ID, build_tassadar_simd_profile_runtime_report,
        load_tassadar_simd_profile_runtime_report, tassadar_simd_profile_runtime_report_path,
        write_tassadar_simd_profile_runtime_report,
    };

    #[test]
    fn simd_profile_runtime_report_keeps_exact_fallback_and_refusal_rows_explicit() {
        let report = build_tassadar_simd_profile_runtime_report();

        assert_eq!(report.profile_id, TASSADAR_SIMD_PROFILE_ID);
        assert_eq!(report.exact_case_count, 1);
        assert_eq!(report.fallback_case_count, 2);
        assert_eq!(report.refusal_case_count, 1);
        assert!(report
            .fallback_backend_ids
            .contains(&String::from("metal_served")));
        assert!(report
            .refused_backend_ids
            .contains(&String::from("accelerator_specific_unbounded")));
    }

    #[test]
    fn simd_profile_runtime_report_matches_committed_truth() {
        let generated = build_tassadar_simd_profile_runtime_report();
        let committed = load_tassadar_simd_profile_runtime_report(
            tassadar_simd_profile_runtime_report_path(),
        )
        .expect("committed simd runtime report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_simd_profile_runtime_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir.path().join("tassadar_simd_profile_runtime_report.json");
        let report =
            write_tassadar_simd_profile_runtime_report(&output_path).expect("write report");
        let persisted = load_tassadar_simd_profile_runtime_report(&output_path)
            .expect("persisted simd runtime report");

        assert_eq!(report, persisted);
    }
}
