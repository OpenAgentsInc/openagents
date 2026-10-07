use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_SIMD_PROFILE_REPORT_REF, TassadarSimdProfileReport, TassadarSimdProfileReportError,
    build_tassadar_simd_profile_report,
};
use psionic_runtime::{
    TASSADAR_RELAXED_SIMD_RUNTIME_REPORT_REF, TassadarRelaxedSimdRuntimeReport,
    build_tassadar_relaxed_simd_runtime_report,
};

pub const TASSADAR_RELAXED_SIMD_RESEARCH_LADDER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_relaxed_simd_research_ladder_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRelaxedSimdResearchLadderReport {
    pub schema_version: u16,
    pub report_id: String,
    pub deterministic_profile_report_ref: String,
    pub deterministic_profile_report: TassadarSimdProfileReport,
    pub runtime_report_ref: String,
    pub runtime_report: TassadarRelaxedSimdRuntimeReport,
    pub public_promotion_allowed: bool,
    pub default_served_profile_allowed: bool,
    pub exact_anchor_backend_ids: Vec<String>,
    pub approximate_backend_ids: Vec<String>,
    pub refused_backend_ids: Vec<String>,
    pub non_promotion_gate_reason_ids: Vec<String>,
    pub overall_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarRelaxedSimdResearchLadderReportError {
    #[error(transparent)]
    SimdProfile(#[from] TassadarSimdProfileReportError),
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

pub fn build_tassadar_relaxed_simd_research_ladder_report()
-> Result<TassadarRelaxedSimdResearchLadderReport, TassadarRelaxedSimdResearchLadderReportError> {
    let deterministic_profile_report = build_tassadar_simd_profile_report()?;
    let runtime_report = build_tassadar_relaxed_simd_runtime_report();
    let exact_anchor_backend_ids = runtime_report.exact_anchor_backend_ids.clone();
    let approximate_backend_ids = runtime_report.approximate_backend_ids.clone();
    let refused_backend_ids = runtime_report.refused_backend_ids.clone();
    let non_promotion_gate_reason_ids = runtime_report.non_promotion_gate_reason_ids.clone();
    let public_promotion_allowed = false;
    let default_served_profile_allowed = false;
    let mut report = TassadarRelaxedSimdResearchLadderReport {
        schema_version: 1,
        report_id: String::from("tassadar.relaxed_simd.research_ladder.report.v1"),
        deterministic_profile_report_ref: String::from(TASSADAR_SIMD_PROFILE_REPORT_REF),
        deterministic_profile_report,
        runtime_report_ref: String::from(TASSADAR_RELAXED_SIMD_RUNTIME_REPORT_REF),
        runtime_report,
        public_promotion_allowed,
        default_served_profile_allowed,
        exact_anchor_backend_ids,
        approximate_backend_ids,
        refused_backend_ids,
        non_promotion_gate_reason_ids,
        overall_green: false,
        claim_boundary: String::from(
            "this eval report keeps relaxed-SIMD and accelerator-specific semantics on a research-only ladder. Deterministic SIMD stays the public named profile, relaxed accelerator rows stay bounded-drift research evidence, and unstable or nonportable rows stay refused. Nothing here widens the public SIMD profile or the default served lane",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.overall_green = report.deterministic_profile_report.overall_green
        && !report.public_promotion_allowed
        && !report.default_served_profile_allowed
        && report.exact_anchor_backend_ids.len() == 1
        && report.approximate_backend_ids.len() == 2
        && report.refused_backend_ids.len() == 2;
    report.summary = format!(
        "Relaxed-SIMD research ladder keeps exact_anchor_backends={}, approximate_backends={}, refused_backends={}, non_promotion_gates={}, public_promotion_allowed={}, overall_green={}.",
        report.exact_anchor_backend_ids.len(),
        report.approximate_backend_ids.len(),
        report.refused_backend_ids.len(),
        report.non_promotion_gate_reason_ids.len(),
        report.public_promotion_allowed,
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_relaxed_simd_research_ladder_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_relaxed_simd_research_ladder_report_path() -> PathBuf {
    repo_root().join(TASSADAR_RELAXED_SIMD_RESEARCH_LADDER_REPORT_REF)
}

pub fn write_tassadar_relaxed_simd_research_ladder_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarRelaxedSimdResearchLadderReport, TassadarRelaxedSimdResearchLadderReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarRelaxedSimdResearchLadderReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_relaxed_simd_research_ladder_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarRelaxedSimdResearchLadderReportError::Write {
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
) -> Result<T, TassadarRelaxedSimdResearchLadderReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarRelaxedSimdResearchLadderReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarRelaxedSimdResearchLadderReportError::Deserialize {
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
        TassadarRelaxedSimdResearchLadderReport,
        build_tassadar_relaxed_simd_research_ladder_report, read_json,
        tassadar_relaxed_simd_research_ladder_report_path,
    };

    #[test]
    fn relaxed_simd_research_ladder_keeps_non_promotion_explicit() {
        let report = build_tassadar_relaxed_simd_research_ladder_report().expect("report");

        assert!(report.overall_green);
        assert!(!report.public_promotion_allowed);
        assert!(!report.default_served_profile_allowed);
        assert_eq!(report.exact_anchor_backend_ids.len(), 1);
        assert_eq!(report.approximate_backend_ids.len(), 2);
        assert_eq!(report.refused_backend_ids.len(), 2);
    }

    #[test]
    fn relaxed_simd_research_ladder_matches_committed_truth() {
        let generated = build_tassadar_relaxed_simd_research_ladder_report().expect("report");
        let committed: TassadarRelaxedSimdResearchLadderReport =
            read_json(tassadar_relaxed_simd_research_ladder_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }
}
