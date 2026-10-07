use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_BROAD_FAMILY_SPECIALIZATION_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_family_specialization_runtime_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadFamilySpecializationRuntimeStatus {
    StableSpecializable,
    BenchmarkOnly,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadFamilySpecializationRuntimeRow {
    pub family_id: String,
    pub runtime_status: TassadarBroadFamilySpecializationRuntimeStatus,
    pub specialization_artifact_id: String,
    pub lineage_green: bool,
    pub decompilable: bool,
    pub portability_envelope_id: String,
    pub safety_gate_ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadFamilySpecializationRuntimeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub rows: Vec<TassadarBroadFamilySpecializationRuntimeRow>,
    pub stable_specializable_count: u32,
    pub benchmark_only_count: u32,
    pub refused_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarBroadFamilySpecializationRuntimeReportError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn build_tassadar_broad_family_specialization_runtime_report(
) -> TassadarBroadFamilySpecializationRuntimeReport {
    let rows = vec![
        row(
            "state_machine_bundle",
            TassadarBroadFamilySpecializationRuntimeStatus::StableSpecializable,
            "specialized.state_machine_bundle.weights.v1",
            true,
            true,
            "cpu_reference_current_host",
            true,
            None,
            "state-machine bundles stay lineage-clean, decompilable, and safety-gate ready under the current bounded specialization family",
        ),
        row(
            "search_frontier_bundle",
            TassadarBroadFamilySpecializationRuntimeStatus::BenchmarkOnly,
            "specialized.search_frontier_bundle.weights.v1",
            true,
            true,
            "cpu_reference_current_host",
            false,
            None,
            "search-frontier bundles stay benchmark-worthy but remain below the safety gate because structure varies across retrains",
        ),
        row(
            "linked_worker_bundle",
            TassadarBroadFamilySpecializationRuntimeStatus::BenchmarkOnly,
            "specialized.linked_worker_bundle.weights.v1",
            true,
            true,
            "cpu_reference_current_host",
            false,
            None,
            "linked-worker bundles remain benchmark-only because their portability and helper-surface envelope is still too narrow for safe promotion",
        ),
        row(
            "effectful_resume_bundle",
            TassadarBroadFamilySpecializationRuntimeStatus::Refused,
            "specialized.effectful_resume_bundle.weights.v1",
            false,
            false,
            "cpu_reference_current_host",
            false,
            Some("non_decompilable_effect_receipts"),
            "effectful resume bundles refuse the safety gate because the current artifact family is not decompilable enough to stay challengeable",
        ),
    ];
    let stable_specializable_count = rows
        .iter()
        .filter(|row| {
            row.runtime_status
                == TassadarBroadFamilySpecializationRuntimeStatus::StableSpecializable
        })
        .count() as u32;
    let benchmark_only_count = rows
        .iter()
        .filter(|row| {
            row.runtime_status == TassadarBroadFamilySpecializationRuntimeStatus::BenchmarkOnly
        })
        .count() as u32;
    let refused_count = rows
        .iter()
        .filter(|row| row.runtime_status == TassadarBroadFamilySpecializationRuntimeStatus::Refused)
        .count() as u32;
    let mut report = TassadarBroadFamilySpecializationRuntimeReport {
        schema_version: 1,
        report_id: String::from("tassadar.broad_family_specialization.runtime.report.v1"),
        rows,
        stable_specializable_count,
        benchmark_only_count,
        refused_count,
        claim_boundary: String::from(
            "this runtime report freezes one bounded broad-family specialization gate over named reusable families only. It does not imply arbitrary program-to-weights closure, arbitrary Wasm specialization, or served promotion",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Broad-family specialization runtime report covers stable_specializable={}, benchmark_only={}, refused={}.",
        report.stable_specializable_count,
        report.benchmark_only_count,
        report.refused_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_broad_family_specialization_runtime_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_broad_family_specialization_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_BROAD_FAMILY_SPECIALIZATION_RUNTIME_REPORT_REF)
}

pub fn write_tassadar_broad_family_specialization_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarBroadFamilySpecializationRuntimeReport,
    TassadarBroadFamilySpecializationRuntimeReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarBroadFamilySpecializationRuntimeReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_broad_family_specialization_runtime_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarBroadFamilySpecializationRuntimeReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn row(
    family_id: &str,
    runtime_status: TassadarBroadFamilySpecializationRuntimeStatus,
    specialization_artifact_id: &str,
    lineage_green: bool,
    decompilable: bool,
    portability_envelope_id: &str,
    safety_gate_ready: bool,
    refusal_reason_id: Option<&str>,
    note: &str,
) -> TassadarBroadFamilySpecializationRuntimeRow {
    TassadarBroadFamilySpecializationRuntimeRow {
        family_id: String::from(family_id),
        runtime_status,
        specialization_artifact_id: String::from(specialization_artifact_id),
        lineage_green,
        decompilable,
        portability_envelope_id: String::from(portability_envelope_id),
        safety_gate_ready,
        refusal_reason_id: refusal_reason_id.map(String::from),
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
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarBroadFamilySpecializationRuntimeReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarBroadFamilySpecializationRuntimeReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarBroadFamilySpecializationRuntimeReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_broad_family_specialization_runtime_report, read_json,
        tassadar_broad_family_specialization_runtime_report_path,
        TassadarBroadFamilySpecializationRuntimeReport,
        TassadarBroadFamilySpecializationRuntimeStatus,
    };

    #[test]
    fn broad_family_specialization_runtime_report_keeps_safety_gate_refusals_explicit() {
        let report = build_tassadar_broad_family_specialization_runtime_report();

        assert_eq!(report.stable_specializable_count, 1);
        assert_eq!(report.benchmark_only_count, 2);
        assert_eq!(report.refused_count, 1);
        assert!(report.rows.iter().any(|row| {
            row.family_id == "effectful_resume_bundle"
                && row.runtime_status == TassadarBroadFamilySpecializationRuntimeStatus::Refused
                && row.refusal_reason_id.as_deref() == Some("non_decompilable_effect_receipts")
        }));
    }

    #[test]
    fn broad_family_specialization_runtime_report_matches_committed_truth() {
        let generated = build_tassadar_broad_family_specialization_runtime_report();
        let committed: TassadarBroadFamilySpecializationRuntimeReport =
            read_json(tassadar_broad_family_specialization_runtime_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }
}
