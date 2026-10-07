use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    TassadarSharedPrimitiveAlgorithmFamily, TASSADAR_SHARED_PRIMITIVE_TRANSFER_SUMMARY_REPORT_REF,
};
use psionic_eval::{
    build_tassadar_shared_primitive_transfer_report, TassadarSharedPrimitiveTransferFailureLayer,
    TassadarSharedPrimitiveTransferRegime, TassadarSharedPrimitiveTransferReport,
    TassadarSharedPrimitiveTransferReportError,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Research-facing summary for the shared primitive transfer substrate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedPrimitiveTransferSummaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub transfer_report: TassadarSharedPrimitiveTransferReport,
    pub foundational_primitive_ids: Vec<String>,
    pub composition_bottleneck_algorithm_families: Vec<String>,
    pub primitive_layer_bottleneck_algorithm_families: Vec<String>,
    pub few_shot_rescue_algorithm_families: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarSharedPrimitiveTransferSummaryReport {
    fn new(transfer_report: TassadarSharedPrimitiveTransferReport) -> Self {
        let foundational_primitive_ids = transfer_report
            .ablation_summaries
            .iter()
            .filter(|summary| summary.foundational)
            .map(|summary| summary.primitive_id.clone())
            .collect::<Vec<_>>();
        let composition_bottleneck_algorithm_families =
            collect_algorithm_families(&transfer_report, |failure_layer| {
                matches!(
                    failure_layer,
                    TassadarSharedPrimitiveTransferFailureLayer::CompositionLayer
                        | TassadarSharedPrimitiveTransferFailureLayer::Mixed
                )
            });
        let primitive_layer_bottleneck_algorithm_families =
            collect_algorithm_families(&transfer_report, |failure_layer| {
                matches!(
                    failure_layer,
                    TassadarSharedPrimitiveTransferFailureLayer::PrimitiveLayer
                        | TassadarSharedPrimitiveTransferFailureLayer::Mixed
                )
            });
        let few_shot_rescue_algorithm_families = few_shot_rescues(&transfer_report);
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.shared_primitive_transfer.summary.v1"),
            transfer_report,
            foundational_primitive_ids,
            composition_bottleneck_algorithm_families,
            primitive_layer_bottleneck_algorithm_families,
            few_shot_rescue_algorithm_families,
            claim_boundary: String::from(
                "this summary keeps shared primitive transfer as a research-only architecture surface. Foundational primitives, primitive-layer bottlenecks, and composition bottlenecks stay explicit and do not widen served executor claims",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Shared primitive transfer summary now marks {} foundational primitives, {} composition bottleneck families, {} primitive-layer bottleneck families, and {} few-shot rescue families.",
            report.foundational_primitive_ids.len(),
            report.composition_bottleneck_algorithm_families.len(),
            report.primitive_layer_bottleneck_algorithm_families.len(),
            report.few_shot_rescue_algorithm_families.len(),
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_shared_primitive_transfer_summary_report|",
            &report,
        );
        report
    }
}

/// Shared primitive transfer summary build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarSharedPrimitiveTransferSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarSharedPrimitiveTransferReportError),
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

/// Builds the committed shared primitive transfer summary report.
pub fn build_tassadar_shared_primitive_transfer_summary_report(
) -> Result<TassadarSharedPrimitiveTransferSummaryReport, TassadarSharedPrimitiveTransferSummaryError>
{
    let transfer_report = build_tassadar_shared_primitive_transfer_report()?;
    Ok(TassadarSharedPrimitiveTransferSummaryReport::new(
        transfer_report,
    ))
}

/// Returns the canonical absolute path for the committed summary report.
#[must_use]
pub fn tassadar_shared_primitive_transfer_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SHARED_PRIMITIVE_TRANSFER_SUMMARY_REPORT_REF)
}

/// Writes the committed shared primitive transfer summary report.
pub fn write_tassadar_shared_primitive_transfer_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSharedPrimitiveTransferSummaryReport, TassadarSharedPrimitiveTransferSummaryError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSharedPrimitiveTransferSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_shared_primitive_transfer_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSharedPrimitiveTransferSummaryError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn collect_algorithm_families(
    report: &TassadarSharedPrimitiveTransferReport,
    include: impl Fn(TassadarSharedPrimitiveTransferFailureLayer) -> bool,
) -> Vec<String> {
    report
        .case_reports
        .iter()
        .filter(|case| include(case.failure_layer))
        .map(|case| String::from(case.held_out_algorithm_family.as_str()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn few_shot_rescues(report: &TassadarSharedPrimitiveTransferReport) -> Vec<String> {
    [
        TassadarSharedPrimitiveAlgorithmFamily::SortMerge,
        TassadarSharedPrimitiveAlgorithmFamily::ClrsShortestPath,
        TassadarSharedPrimitiveAlgorithmFamily::ClrsWasmShortestPath,
        TassadarSharedPrimitiveAlgorithmFamily::HungarianMatching,
        TassadarSharedPrimitiveAlgorithmFamily::SudokuSearch,
        TassadarSharedPrimitiveAlgorithmFamily::VerifierSearchKernel,
    ]
    .into_iter()
    .filter_map(|family| {
        let zero_shot = report.case_reports.iter().find(|case| {
            case.held_out_algorithm_family == family
                && case.regime == TassadarSharedPrimitiveTransferRegime::ZeroShot
        })?;
        let few_shot = report.case_reports.iter().find(|case| {
            case.held_out_algorithm_family == family
                && case.regime == TassadarSharedPrimitiveTransferRegime::FewShot
        })?;
        let improved_exactness =
            few_shot.final_task_exactness_bps as i32 - zero_shot.final_task_exactness_bps as i32;
        (improved_exactness >= 700 && few_shot.primitive_reuse_bps >= 8_000)
            .then(|| String::from(family.as_str()))
    })
    .collect()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-research crate dir")
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarSharedPrimitiveTransferSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarSharedPrimitiveTransferSummaryError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSharedPrimitiveTransferSummaryError::Deserialize {
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
        build_tassadar_shared_primitive_transfer_summary_report, read_repo_json,
        tassadar_shared_primitive_transfer_summary_report_path,
        write_tassadar_shared_primitive_transfer_summary_report,
        TassadarSharedPrimitiveTransferSummaryReport,
    };
    use psionic_data::TASSADAR_SHARED_PRIMITIVE_TRANSFER_SUMMARY_REPORT_REF;

    #[test]
    fn shared_primitive_transfer_summary_marks_foundational_primitives_and_bottlenecks() {
        let report =
            build_tassadar_shared_primitive_transfer_summary_report().expect("summary report");

        assert!(report
            .foundational_primitive_ids
            .contains(&String::from("tassadar.primitive.bounded_backtrack.v1")));
        assert!(report
            .composition_bottleneck_algorithm_families
            .contains(&String::from("sudoku_search")));
        assert!(report
            .primitive_layer_bottleneck_algorithm_families
            .contains(&String::from("hungarian_matching")));
    }

    #[test]
    fn shared_primitive_transfer_summary_matches_committed_truth() {
        let generated =
            build_tassadar_shared_primitive_transfer_summary_report().expect("summary report");
        let committed: TassadarSharedPrimitiveTransferSummaryReport =
            read_repo_json(TASSADAR_SHARED_PRIMITIVE_TRANSFER_SUMMARY_REPORT_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_shared_primitive_transfer_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_shared_primitive_transfer_summary.json");
        let written = write_tassadar_shared_primitive_transfer_summary_report(&output_path)
            .expect("write summary");
        let persisted: TassadarSharedPrimitiveTransferSummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_shared_primitive_transfer_summary_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_shared_primitive_transfer_summary.json")
        );
    }
}
