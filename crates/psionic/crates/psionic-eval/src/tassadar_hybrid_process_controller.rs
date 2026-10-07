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
    tassadar_hybrid_process_controller_publication, TassadarHybridProcessControllerPublication,
};
use psionic_router::{
    build_tassadar_hybrid_process_controller_route_report,
    TassadarHybridProcessControllerRouteReport,
    TASSADAR_HYBRID_PROCESS_CONTROLLER_ROUTE_REPORT_REF,
};
use psionic_runtime::{
    build_tassadar_hybrid_process_controller_runtime_bundle,
    TassadarHybridProcessControllerRuntimeBundle,
    TASSADAR_HYBRID_PROCESS_CONTROLLER_RUNTIME_REPORT_REF,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_HYBRID_PROCESS_CONTROLLER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_hybrid_process_controller_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHybridProcessControllerReport {
    pub schema_version: u16,
    pub report_id: String,
    pub publication: TassadarHybridProcessControllerPublication,
    pub runtime_report: TassadarHybridProcessControllerRuntimeBundle,
    pub route_report_ref: String,
    pub verifier_positive_delta_case_ids: Vec<String>,
    pub challenge_green_case_ids: Vec<String>,
    pub unsupported_transition_case_ids: Vec<String>,
    pub compiled_exact_case_count: u32,
    pub hybrid_verifier_case_count: u32,
    pub refused_case_count: u32,
    pub served_publication_allowed: bool,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarHybridProcessControllerReportError {
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
pub fn build_tassadar_hybrid_process_controller_report() -> TassadarHybridProcessControllerReport {
    let publication = tassadar_hybrid_process_controller_publication();
    let runtime_report = build_tassadar_hybrid_process_controller_runtime_bundle();
    let route_report = build_tassadar_hybrid_process_controller_route_report();
    let verifier_positive_delta_case_ids = runtime_report
        .case_receipts
        .iter()
        .filter(|case| case.verifier_on_exactness_bps > case.verifier_off_exactness_bps)
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let challenge_green_case_ids = runtime_report
        .case_receipts
        .iter()
        .filter(|case| case.challenge_path_green)
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let unsupported_transition_case_ids = runtime_report
        .case_receipts
        .iter()
        .filter(|case| case.refusal_reason_id.as_deref() == Some("unsupported_effect_transition"))
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let mut generated_from_refs = vec![
        String::from(TASSADAR_HYBRID_PROCESS_CONTROLLER_RUNTIME_REPORT_REF),
        String::from(TASSADAR_HYBRID_PROCESS_CONTROLLER_ROUTE_REPORT_REF),
    ];
    generated_from_refs.sort();
    generated_from_refs.dedup();
    let mut report = TassadarHybridProcessControllerReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.hybrid_process_controller.report.v1"),
        publication,
        compiled_exact_case_count: runtime_report.compiled_exact_case_count,
        hybrid_verifier_case_count: runtime_report.hybrid_case_count,
        refused_case_count: runtime_report.refused_case_count,
        served_publication_allowed: false,
        generated_from_refs,
        runtime_report,
        route_report_ref: String::from(TASSADAR_HYBRID_PROCESS_CONTROLLER_ROUTE_REPORT_REF),
        verifier_positive_delta_case_ids,
        challenge_green_case_ids,
        unsupported_transition_case_ids,
        claim_boundary: String::from(
            "this eval report keeps the verifier-attached hybrid controller as a research-only architecture and routing surface. It does not widen served posture, arbitrary hybrid transitions, or broad internal-compute claims",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    let route_report: TassadarHybridProcessControllerRouteReport = route_report;
    report.summary = format!(
        "Hybrid process controller report covers compiled_exact_cases={}, hybrid_cases={}, refused_cases={}, verifier_positive_delta_cases={}, challenge_green_cases={}, served_publication_allowed={}, route_rows={}.",
        report.compiled_exact_case_count,
        report.hybrid_verifier_case_count,
        report.refused_case_count,
        report.verifier_positive_delta_case_ids.len(),
        report.challenge_green_case_ids.len(),
        report.served_publication_allowed,
        route_report.rows.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_hybrid_process_controller_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_hybrid_process_controller_report_path() -> PathBuf {
    repo_root().join(TASSADAR_HYBRID_PROCESS_CONTROLLER_REPORT_REF)
}

pub fn write_tassadar_hybrid_process_controller_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarHybridProcessControllerReport, TassadarHybridProcessControllerReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarHybridProcessControllerReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_hybrid_process_controller_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarHybridProcessControllerReportError::Write {
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
) -> Result<T, TassadarHybridProcessControllerReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarHybridProcessControllerReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarHybridProcessControllerReportError::Deserialize {
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
        build_tassadar_hybrid_process_controller_report, read_json,
        tassadar_hybrid_process_controller_report_path,
        write_tassadar_hybrid_process_controller_report, TassadarHybridProcessControllerReport,
    };

    #[test]
    fn hybrid_process_controller_report_keeps_verifier_delta_challenge_and_refusal_explicit() {
        let report = build_tassadar_hybrid_process_controller_report();

        assert_eq!(report.compiled_exact_case_count, 1);
        assert_eq!(report.hybrid_verifier_case_count, 2);
        assert_eq!(report.refused_case_count, 1);
        assert_eq!(report.verifier_positive_delta_case_ids.len(), 2);
        assert_eq!(report.unsupported_transition_case_ids.len(), 1);
        assert!(!report.served_publication_allowed);
    }

    #[test]
    fn hybrid_process_controller_report_matches_committed_truth() {
        let generated = build_tassadar_hybrid_process_controller_report();
        let committed: TassadarHybridProcessControllerReport =
            read_json(tassadar_hybrid_process_controller_report_path()).expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_hybrid_process_controller_report_persists_current_truth() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output_path = output_dir
            .path()
            .join("tassadar_hybrid_process_controller_report.json");
        let report =
            write_tassadar_hybrid_process_controller_report(&output_path).expect("write report");
        let written: TassadarHybridProcessControllerReport =
            serde_json::from_str(&std::fs::read_to_string(&output_path).expect("written file"))
                .expect("parse");
        assert_eq!(report, written);
    }
}
