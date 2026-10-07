use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    build_tassadar_article_fast_route_throughput_bundle,
    tassadar_article_fast_route_throughput_root_path,
    write_tassadar_article_fast_route_throughput_bundle, TassadarArticleFastRouteThroughputBundle,
    TassadarArticleFastRouteThroughputBundleError,
    TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_BUNDLE_FILE,
    TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_ROOT_REF,
};

use crate::{
    build_tassadar_article_cpu_reproducibility_report,
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_article_fast_route_architecture_selection_report,
    build_tassadar_article_fast_route_exactness_report, TassadarArticleCpuReproducibilityReport,
    TassadarArticleCpuReproducibilityReportError, TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleFastRouteArchitectureSelectionError,
    TassadarArticleFastRouteArchitectureSelectionReport, TassadarArticleFastRouteExactnessReport,
    TassadarArticleFastRouteExactnessReportError, TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
    TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_REPORT_REF,
};

pub const TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_throughput_floor_report.json";
pub const TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_CHECKER_REF: &str =
    "scripts/check-tassadar-article-fast-route-throughput-floor.sh";

const TIED_REQUIREMENT_ID: &str = "TAS-175";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteThroughputFloorAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteThroughputSelectionPrerequisite {
    pub report_ref: String,
    pub selected_candidate_kind: String,
    pub fast_route_selection_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteThroughputExactnessPrerequisite {
    pub report_ref: String,
    pub exactness_green: bool,
    pub article_equivalence_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteThroughputCrossMachineDriftReview {
    pub report_ref: String,
    pub current_host_machine_class_id: String,
    pub supported_machine_class_ids: Vec<String>,
    pub allowed_floor_drift_bps: u16,
    pub drift_policy_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteThroughputFloorReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleFastRouteThroughputFloorAcceptanceGateTie,
    pub selection_prerequisite: TassadarArticleFastRouteThroughputSelectionPrerequisite,
    pub exactness_prerequisite: TassadarArticleFastRouteThroughputExactnessPrerequisite,
    pub cross_machine_drift_review: TassadarArticleFastRouteThroughputCrossMachineDriftReview,
    pub throughput_bundle_ref: String,
    pub throughput_bundle: TassadarArticleFastRouteThroughputBundle,
    pub demo_public_floor_pass_count: u32,
    pub demo_internal_floor_pass_count: u32,
    pub kernel_floor_pass_count: u32,
    pub throughput_floor_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleFastRouteThroughputFloorReportError {
    #[error(transparent)]
    Runtime(#[from] TassadarArticleFastRouteThroughputBundleError),
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    Selection(#[from] TassadarArticleFastRouteArchitectureSelectionError),
    #[error(transparent)]
    Exactness(#[from] TassadarArticleFastRouteExactnessReportError),
    #[error(transparent)]
    CpuReproducibility(#[from] TassadarArticleCpuReproducibilityReportError),
    #[error("internal TAS-175 invariant failed: {detail}")]
    Invariant { detail: String },
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

pub fn build_tassadar_article_fast_route_throughput_floor_report() -> Result<
    TassadarArticleFastRouteThroughputFloorReport,
    TassadarArticleFastRouteThroughputFloorReportError,
> {
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let selection_report = build_tassadar_article_fast_route_architecture_selection_report()?;
    let exactness_report = build_tassadar_article_fast_route_exactness_report()?;
    let cpu_reproducibility_report = build_tassadar_article_cpu_reproducibility_report()?;
    let throughput_bundle = build_tassadar_article_fast_route_throughput_bundle()?;

    let acceptance_gate_tie = build_acceptance_gate_tie(&acceptance_gate)?;
    let selection_prerequisite = build_selection_prerequisite(&selection_report);
    let exactness_prerequisite = build_exactness_prerequisite(&exactness_report);
    let cross_machine_drift_review = build_cross_machine_drift_review(&cpu_reproducibility_report);
    let throughput_floor_green = acceptance_gate_tie.tied_requirement_satisfied
        && selection_prerequisite.fast_route_selection_green
        && exactness_prerequisite.exactness_green
        && cross_machine_drift_review.drift_policy_green
        && throughput_bundle.throughput_floor_green;

    let mut report = TassadarArticleFastRouteThroughputFloorReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_fast_route_throughput_floor.report.v1"),
        checker_script_ref: String::from(
            TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_CHECKER_REF,
        ),
        acceptance_gate_tie: acceptance_gate_tie.clone(),
        selection_prerequisite,
        exactness_prerequisite,
        cross_machine_drift_review,
        throughput_bundle_ref: format!(
            "{}/{}",
            TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_ROOT_REF,
            TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_BUNDLE_FILE
        ),
        demo_public_floor_pass_count: throughput_bundle.demo_public_floor_pass_count,
        demo_internal_floor_pass_count: throughput_bundle.demo_internal_floor_pass_count,
        kernel_floor_pass_count: throughput_bundle.kernel_floor_pass_count,
        throughput_bundle,
        throughput_floor_green,
        article_equivalence_green: acceptance_gate_tie.blocked_issue_ids.is_empty()
            && throughput_floor_green,
        claim_boundary: String::from(
            "this report closes TAS-175 only. It ties the selected HullCache fast route to explicit throughput-floor evidence, current-host cross-machine drift bounds, and the earlier TAS-172/TAS-174 prerequisites without claiming final Hungarian demo parity, Arto closure, benchmark-wide hard-Sudoku closure, no-spill single-run closure, or article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Fast-route throughput-floor report now records tied_requirement_satisfied={}, selection_green={}, exactness_green={}, drift_policy_green={}, demo_public_floor_passes={}/{}, demo_internal_floor_passes={}/{}, kernel_floor_passes={}/{}, and throughput_floor_green={}.",
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.selection_prerequisite.fast_route_selection_green,
        report.exactness_prerequisite.exactness_green,
        report.cross_machine_drift_review.drift_policy_green,
        report.demo_public_floor_pass_count,
        report.throughput_bundle.demo_receipts.len(),
        report.demo_internal_floor_pass_count,
        report.throughput_bundle.demo_receipts.len(),
        report.kernel_floor_pass_count,
        report.throughput_bundle.kernel_receipts.len(),
        report.throughput_floor_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_fast_route_throughput_floor_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_article_fast_route_throughput_floor_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF)
}

pub fn write_tassadar_article_fast_route_throughput_floor_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleFastRouteThroughputFloorReport,
    TassadarArticleFastRouteThroughputFloorReportError,
> {
    write_tassadar_article_fast_route_throughput_bundle(
        tassadar_article_fast_route_throughput_root_path(),
    )?;
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleFastRouteThroughputFloorReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_fast_route_throughput_floor_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleFastRouteThroughputFloorReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_acceptance_gate_tie(
    acceptance_gate: &TassadarArticleEquivalenceAcceptanceGateReport,
) -> Result<
    TassadarArticleFastRouteThroughputFloorAcceptanceGateTie,
    TassadarArticleFastRouteThroughputFloorReportError,
> {
    let tied_requirement_satisfied = acceptance_gate
        .green_requirement_ids
        .iter()
        .any(|requirement_id| requirement_id == TIED_REQUIREMENT_ID);
    if !tied_requirement_satisfied {
        return Err(
            TassadarArticleFastRouteThroughputFloorReportError::Invariant {
                detail: format!(
                    "acceptance gate does not yet treat `{}` as green",
                    TIED_REQUIREMENT_ID
                ),
            },
        );
    }
    Ok(TassadarArticleFastRouteThroughputFloorAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied,
        acceptance_status: acceptance_gate.acceptance_status,
        blocked_issue_ids: acceptance_gate.blocked_issue_ids.clone(),
    })
}

fn build_selection_prerequisite(
    selection_report: &TassadarArticleFastRouteArchitectureSelectionReport,
) -> TassadarArticleFastRouteThroughputSelectionPrerequisite {
    TassadarArticleFastRouteThroughputSelectionPrerequisite {
        report_ref: String::from(
            TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
        ),
        selected_candidate_kind: selection_report.selected_candidate_kind.label().to_string(),
        fast_route_selection_green: selection_report.fast_route_selection_green,
        detail: String::from(
            "TAS-175 stays tied to the already-selected HullCache fast path rather than introducing a second canonical route for throughput closure",
        ),
    }
}

fn build_exactness_prerequisite(
    exactness_report: &TassadarArticleFastRouteExactnessReport,
) -> TassadarArticleFastRouteThroughputExactnessPrerequisite {
    TassadarArticleFastRouteThroughputExactnessPrerequisite {
        report_ref: String::from(TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_REPORT_REF),
        exactness_green: exactness_report.exactness_green,
        article_equivalence_green: exactness_report.article_equivalence_green,
        detail: String::from(
            "TAS-175 depends on TAS-174 staying exact and fallback-free on the selected fast route before runtime floors can count as honest closure",
        ),
    }
}

fn build_cross_machine_drift_review(
    cpu_reproducibility_report: &TassadarArticleCpuReproducibilityReport,
) -> TassadarArticleFastRouteThroughputCrossMachineDriftReview {
    let supported_machine_class_ids = cpu_reproducibility_report
        .supported_machine_class_ids
        .clone();
    let drift_policy_green = cpu_reproducibility_report
        .matrix
        .current_host_measured_green
        && supported_machine_class_ids
            == vec![
                String::from("host_cpu_aarch64"),
                String::from("host_cpu_x86_64"),
            ];
    TassadarArticleFastRouteThroughputCrossMachineDriftReview {
        report_ref: String::from(TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF),
        current_host_machine_class_id: cpu_reproducibility_report
            .matrix
            .current_host_machine_class_id
            .clone(),
        supported_machine_class_ids,
        allowed_floor_drift_bps: 0,
        drift_policy_green,
        detail: String::from(
            "the fast-route throughput floors stay identical across the declared `host_cpu_aarch64` and `host_cpu_x86_64` classes with zero allowed floor drift; other CPU classes remain outside the TAS-175 closure envelope",
        ),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleFastRouteThroughputFloorReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleFastRouteThroughputFloorReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleFastRouteThroughputFloorReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_fast_route_throughput_floor_report, read_repo_json,
        tassadar_article_fast_route_throughput_floor_report_path,
        write_tassadar_article_fast_route_throughput_floor_report,
        TassadarArticleFastRouteThroughputFloorReport,
        TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF,
    };

    fn normalized_report_value(
        report: &TassadarArticleFastRouteThroughputFloorReport,
    ) -> serde_json::Value {
        let mut value = serde_json::to_value(report).expect("report serializes");
        value["report_digest"] = serde_json::Value::Null;
        value["throughput_bundle"]["bundle_digest"] = serde_json::Value::Null;
        for receipt in value["throughput_bundle"]["demo_receipts"]
            .as_array_mut()
            .expect("demo_receipts")
        {
            receipt["measured_run_time_seconds"] = serde_json::Value::Null;
            receipt["steps_per_second"] = serde_json::Value::Null;
            receipt["tokens_per_second"] = serde_json::Value::Null;
            receipt["lines_per_second"] = serde_json::Value::Null;
        }
        for receipt in value["throughput_bundle"]["kernel_receipts"]
            .as_array_mut()
            .expect("kernel_receipts")
        {
            receipt["measured_run_time_seconds"] = serde_json::Value::Null;
            receipt["steps_per_second"] = serde_json::Value::Null;
        }
        value
    }

    #[test]
    fn throughput_floor_report_tracks_fast_route_closure_without_final_green() {
        let report = build_tassadar_article_fast_route_throughput_floor_report().expect("report");

        assert_eq!(report.acceptance_gate_tie.tied_requirement_id, "TAS-175");
        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert_eq!(
            report.selection_prerequisite.selected_candidate_kind,
            "hull_cache_runtime"
        );
        assert!(report.selection_prerequisite.fast_route_selection_green);
        assert!(report.exactness_prerequisite.exactness_green);
        assert!(report.cross_machine_drift_review.drift_policy_green);
        assert_eq!(report.demo_public_floor_pass_count, 2);
        assert_eq!(report.demo_internal_floor_pass_count, 2);
        assert_eq!(report.kernel_floor_pass_count, 4);
        assert!(report.throughput_floor_green);
        assert!(report.article_equivalence_green);
        assert!(report
            .throughput_bundle
            .demo_receipts
            .iter()
            .all(|receipt| receipt.exactness_bps == 10_000));
        assert!(report
            .throughput_bundle
            .kernel_receipts
            .iter()
            .all(|receipt| receipt.exactness_bps == 10_000));
    }

    #[test]
    fn throughput_floor_report_matches_committed_truth() {
        let generated =
            build_tassadar_article_fast_route_throughput_floor_report().expect("report");
        let committed: TassadarArticleFastRouteThroughputFloorReport = read_repo_json(
            TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF,
            "article_fast_route_throughput_floor_report",
        )
        .expect("committed report");
        assert_eq!(
            normalized_report_value(&generated),
            normalized_report_value(&committed)
        );
    }

    #[test]
    fn write_throughput_floor_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_fast_route_throughput_floor_report.json");
        let written = write_tassadar_article_fast_route_throughput_floor_report(&output_path)
            .expect("write report");
        let persisted: TassadarArticleFastRouteThroughputFloorReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(
            normalized_report_value(&written),
            normalized_report_value(&persisted)
        );
        assert_eq!(
            tassadar_article_fast_route_throughput_floor_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_fast_route_throughput_floor_report.json")
        );
    }
}
