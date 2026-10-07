use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_models::{
    tassadar_broad_family_specialization_publication, TassadarBroadFamilySpecializationPublication,
};
use psionic_runtime::{
    build_tassadar_broad_family_specialization_runtime_report,
    TassadarBroadFamilySpecializationRuntimeReport, TassadarBroadFamilySpecializationRuntimeStatus,
    TASSADAR_BROAD_FAMILY_SPECIALIZATION_RUNTIME_REPORT_REF,
};

pub const TASSADAR_BROAD_FAMILY_SPECIALIZATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_family_specialization_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadFamilySpecializationReport {
    pub schema_version: u16,
    pub report_id: String,
    pub publication: TassadarBroadFamilySpecializationPublication,
    pub runtime_report: TassadarBroadFamilySpecializationRuntimeReport,
    pub stable_specializable_family_ids: Vec<String>,
    pub unstable_family_ids: Vec<String>,
    pub non_decompilable_family_ids: Vec<String>,
    pub lineage_green_family_ids: Vec<String>,
    pub safety_gate_green_family_ids: Vec<String>,
    pub served_publication_allowed: bool,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarBroadFamilySpecializationReportError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn build_tassadar_broad_family_specialization_report() -> TassadarBroadFamilySpecializationReport
{
    let publication = tassadar_broad_family_specialization_publication();
    let runtime_report = build_tassadar_broad_family_specialization_runtime_report();
    let stable_specializable_family_ids = runtime_report
        .rows
        .iter()
        .filter(|row| {
            row.runtime_status
                == TassadarBroadFamilySpecializationRuntimeStatus::StableSpecializable
        })
        .map(|row| row.family_id.clone())
        .collect::<Vec<_>>();
    let unstable_family_ids = runtime_report
        .rows
        .iter()
        .filter(|row| {
            row.runtime_status == TassadarBroadFamilySpecializationRuntimeStatus::BenchmarkOnly
        })
        .map(|row| row.family_id.clone())
        .collect::<Vec<_>>();
    let non_decompilable_family_ids = runtime_report
        .rows
        .iter()
        .filter(|row| !row.decompilable)
        .map(|row| row.family_id.clone())
        .collect::<Vec<_>>();
    let lineage_green_family_ids = runtime_report
        .rows
        .iter()
        .filter(|row| row.lineage_green)
        .map(|row| row.family_id.clone())
        .collect::<Vec<_>>();
    let safety_gate_green_family_ids = runtime_report
        .rows
        .iter()
        .filter(|row| row.safety_gate_ready)
        .map(|row| row.family_id.clone())
        .collect::<Vec<_>>();
    let mut generated_from_refs = vec![String::from(
        TASSADAR_BROAD_FAMILY_SPECIALIZATION_RUNTIME_REPORT_REF,
    )];
    generated_from_refs.extend(publication.baseline_refs.iter().cloned());
    generated_from_refs.sort();
    generated_from_refs.dedup();
    let mut report = TassadarBroadFamilySpecializationReport {
        schema_version: 1,
        report_id: String::from("tassadar.broad_family_specialization.report.v1"),
        publication,
        runtime_report,
        stable_specializable_family_ids,
        unstable_family_ids,
        non_decompilable_family_ids,
        lineage_green_family_ids,
        safety_gate_green_family_ids,
        served_publication_allowed: false,
        generated_from_refs,
        claim_boundary: String::from(
            "this report keeps broad-family specialization as a research-only architecture and promotion-discipline surface. It does not widen served posture, arbitrary program-to-weights closure, or broad internal-compute claims",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Broad-family specialization report covers stable_specializable_families={}, unstable_families={}, non_decompilable_families={}, safety_gate_green_families={}, served_publication_allowed={}.",
        report.stable_specializable_family_ids.len(),
        report.unstable_family_ids.len(),
        report.non_decompilable_family_ids.len(),
        report.safety_gate_green_family_ids.len(),
        report.served_publication_allowed,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_broad_family_specialization_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_broad_family_specialization_report_path() -> PathBuf {
    repo_root().join(TASSADAR_BROAD_FAMILY_SPECIALIZATION_REPORT_REF)
}

pub fn write_tassadar_broad_family_specialization_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarBroadFamilySpecializationReport, TassadarBroadFamilySpecializationReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarBroadFamilySpecializationReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_broad_family_specialization_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarBroadFamilySpecializationReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
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
) -> Result<T, TassadarBroadFamilySpecializationReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarBroadFamilySpecializationReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarBroadFamilySpecializationReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_broad_family_specialization_report, read_json,
        tassadar_broad_family_specialization_report_path, TassadarBroadFamilySpecializationReport,
    };

    #[test]
    fn broad_family_specialization_report_keeps_stable_unstable_and_refused_families_explicit() {
        let report = build_tassadar_broad_family_specialization_report();

        assert_eq!(report.stable_specializable_family_ids.len(), 1);
        assert_eq!(report.unstable_family_ids.len(), 2);
        assert_eq!(report.non_decompilable_family_ids.len(), 1);
        assert_eq!(report.safety_gate_green_family_ids.len(), 1);
        assert!(!report.served_publication_allowed);
    }

    #[test]
    fn broad_family_specialization_report_matches_committed_truth() {
        let generated = build_tassadar_broad_family_specialization_report();
        let committed: TassadarBroadFamilySpecializationReport =
            read_json(tassadar_broad_family_specialization_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }
}
