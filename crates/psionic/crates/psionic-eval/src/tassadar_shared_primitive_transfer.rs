use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    tassadar_shared_primitive_transfer_contract, TassadarSharedPrimitiveAlgorithmFamily,
    TassadarSharedPrimitiveKind, TASSADAR_SHARED_PRIMITIVE_TRANSFER_EVIDENCE_BUNDLE_REF,
    TASSADAR_SHARED_PRIMITIVE_TRANSFER_REPORT_REF,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Regime used for one held-out transfer case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSharedPrimitiveTransferRegime {
    ZeroShot,
    FewShot,
}

/// One held-out algorithm transfer receipt decoded from the train bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedPrimitiveTransferCase {
    pub case_id: String,
    pub held_out_algorithm_family: TassadarSharedPrimitiveAlgorithmFamily,
    pub regime: TassadarSharedPrimitiveTransferRegime,
    pub reused_primitive_ids: Vec<String>,
    pub primitive_reuse_bps: u32,
    pub final_task_exactness_bps: u32,
    pub evidence_refs: Vec<String>,
    pub detail: String,
}

/// One primitive-ablation receipt decoded from the train bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedPrimitiveAblationReceipt {
    pub primitive_id: String,
    pub primitive_kind: TassadarSharedPrimitiveKind,
    pub held_out_algorithm_families: Vec<TassadarSharedPrimitiveAlgorithmFamily>,
    pub mean_zero_shot_drop_bps: u32,
    pub mean_few_shot_drop_bps: u32,
    pub detail: String,
}

/// Train-side evidence bundle decoded by eval.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedPrimitiveTransferEvidenceBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub contract_digest: String,
    pub publication_digest: String,
    pub transfer_cases: Vec<TassadarSharedPrimitiveTransferCase>,
    pub primitive_ablations: Vec<TassadarSharedPrimitiveAblationReceipt>,
    pub claim_boundary: String,
    pub bundle_digest: String,
}

/// Failure layer surfaced by the shared primitive transfer report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSharedPrimitiveTransferFailureLayer {
    None,
    PrimitiveLayer,
    CompositionLayer,
    Mixed,
}

/// Eval-facing case report over one held-out transfer case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedPrimitiveTransferCaseReport {
    pub case_id: String,
    pub held_out_algorithm_family: TassadarSharedPrimitiveAlgorithmFamily,
    pub regime: TassadarSharedPrimitiveTransferRegime,
    pub primitive_reuse_bps: u32,
    pub final_task_exactness_bps: u32,
    pub composition_gap_bps: u32,
    pub failure_layer: TassadarSharedPrimitiveTransferFailureLayer,
    pub evidence_refs: Vec<String>,
    pub detail: String,
}

/// Eval-facing ablation summary for one primitive.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedPrimitiveAblationSummary {
    pub primitive_id: String,
    pub primitive_kind: TassadarSharedPrimitiveKind,
    pub held_out_algorithm_count: u32,
    pub mean_zero_shot_drop_bps: u32,
    pub mean_few_shot_drop_bps: u32,
    pub foundational: bool,
    pub detail: String,
}

/// Aggregate regime summary for the shared primitive transfer report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedPrimitiveTransferRegimeSummary {
    pub regime: TassadarSharedPrimitiveTransferRegime,
    pub case_count: u32,
    pub mean_primitive_reuse_bps: u32,
    pub mean_final_task_exactness_bps: u32,
    pub composition_bottleneck_count: u32,
}

/// Committed eval report for the shared primitive transfer substrate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedPrimitiveTransferReport {
    pub schema_version: u16,
    pub report_id: String,
    pub contract_digest: String,
    pub evidence_bundle: TassadarSharedPrimitiveTransferEvidenceBundle,
    pub case_reports: Vec<TassadarSharedPrimitiveTransferCaseReport>,
    pub ablation_summaries: Vec<TassadarSharedPrimitiveAblationSummary>,
    pub regime_summaries: Vec<TassadarSharedPrimitiveTransferRegimeSummary>,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarSharedPrimitiveTransferReport {
    fn new(
        evidence_bundle: TassadarSharedPrimitiveTransferEvidenceBundle,
        case_reports: Vec<TassadarSharedPrimitiveTransferCaseReport>,
        ablation_summaries: Vec<TassadarSharedPrimitiveAblationSummary>,
        regime_summaries: Vec<TassadarSharedPrimitiveTransferRegimeSummary>,
    ) -> Self {
        let contract = tassadar_shared_primitive_transfer_contract();
        let composition_bottlenecks = case_reports
            .iter()
            .filter(|report| {
                matches!(
                    report.failure_layer,
                    TassadarSharedPrimitiveTransferFailureLayer::CompositionLayer
                        | TassadarSharedPrimitiveTransferFailureLayer::Mixed
                )
            })
            .count();
        let foundational_primitive_count = ablation_summaries
            .iter()
            .filter(|summary| summary.foundational)
            .count();
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.shared_primitive_transfer.report.v1"),
            contract_digest: contract.contract_digest,
            evidence_bundle,
            case_reports,
            ablation_summaries,
            regime_summaries,
            generated_from_refs: vec![String::from(
                TASSADAR_SHARED_PRIMITIVE_TRANSFER_EVIDENCE_BUNDLE_REF,
            )],
            claim_boundary: String::from(
                "this report keeps primitive-layer reuse, final-task exactness, and composition gaps explicit across the published held-out algorithm families only. It does not widen any served claim or treat primitive transfer as proof of full executor closure",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Shared primitive transfer now freezes {} held-out cases across {} regimes, with {} composition-bottleneck cases and {} foundational primitives kept explicit.",
            report.case_reports.len(),
            report.regime_summaries.len(),
            composition_bottlenecks,
            foundational_primitive_count,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_shared_primitive_transfer_report|",
            &report,
        );
        report
    }
}

/// Shared primitive transfer report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarSharedPrimitiveTransferReportError {
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

/// Builds the committed shared primitive transfer report.
pub fn build_tassadar_shared_primitive_transfer_report(
) -> Result<TassadarSharedPrimitiveTransferReport, TassadarSharedPrimitiveTransferReportError> {
    let evidence_bundle: TassadarSharedPrimitiveTransferEvidenceBundle =
        read_repo_json(TASSADAR_SHARED_PRIMITIVE_TRANSFER_EVIDENCE_BUNDLE_REF)?;
    let case_reports = evidence_bundle
        .transfer_cases
        .iter()
        .map(build_case_report)
        .collect::<Vec<_>>();
    let ablation_summaries = evidence_bundle
        .primitive_ablations
        .iter()
        .map(build_ablation_summary)
        .collect::<Vec<_>>();
    let regime_summaries = build_regime_summaries(case_reports.as_slice());
    Ok(TassadarSharedPrimitiveTransferReport::new(
        evidence_bundle,
        case_reports,
        ablation_summaries,
        regime_summaries,
    ))
}

/// Returns the canonical absolute path for the committed report.
#[must_use]
pub fn tassadar_shared_primitive_transfer_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SHARED_PRIMITIVE_TRANSFER_REPORT_REF)
}

/// Writes the committed shared primitive transfer report.
pub fn write_tassadar_shared_primitive_transfer_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSharedPrimitiveTransferReport, TassadarSharedPrimitiveTransferReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSharedPrimitiveTransferReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_shared_primitive_transfer_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSharedPrimitiveTransferReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_case_report(
    case: &TassadarSharedPrimitiveTransferCase,
) -> TassadarSharedPrimitiveTransferCaseReport {
    let composition_gap_bps = case
        .primitive_reuse_bps
        .saturating_sub(case.final_task_exactness_bps);
    TassadarSharedPrimitiveTransferCaseReport {
        case_id: case.case_id.clone(),
        held_out_algorithm_family: case.held_out_algorithm_family,
        regime: case.regime,
        primitive_reuse_bps: case.primitive_reuse_bps,
        final_task_exactness_bps: case.final_task_exactness_bps,
        composition_gap_bps,
        failure_layer: classify_failure_layer(case.primitive_reuse_bps, composition_gap_bps),
        evidence_refs: case.evidence_refs.clone(),
        detail: case.detail.clone(),
    }
}

fn build_ablation_summary(
    receipt: &TassadarSharedPrimitiveAblationReceipt,
) -> TassadarSharedPrimitiveAblationSummary {
    TassadarSharedPrimitiveAblationSummary {
        primitive_id: receipt.primitive_id.clone(),
        primitive_kind: receipt.primitive_kind,
        held_out_algorithm_count: receipt.held_out_algorithm_families.len() as u32,
        mean_zero_shot_drop_bps: receipt.mean_zero_shot_drop_bps,
        mean_few_shot_drop_bps: receipt.mean_few_shot_drop_bps,
        foundational: receipt.mean_zero_shot_drop_bps >= 900
            || (receipt.mean_few_shot_drop_bps >= 650
                && receipt.held_out_algorithm_families.len() >= 3),
        detail: receipt.detail.clone(),
    }
}

fn build_regime_summaries(
    case_reports: &[TassadarSharedPrimitiveTransferCaseReport],
) -> Vec<TassadarSharedPrimitiveTransferRegimeSummary> {
    let mut grouped = BTreeMap::<
        TassadarSharedPrimitiveTransferRegime,
        Vec<&TassadarSharedPrimitiveTransferCaseReport>,
    >::new();
    for report in case_reports {
        grouped.entry(report.regime).or_default().push(report);
    }

    grouped
        .into_iter()
        .map(
            |(regime, reports)| TassadarSharedPrimitiveTransferRegimeSummary {
                regime,
                case_count: reports.len() as u32,
                mean_primitive_reuse_bps: mean_u32(
                    reports.iter().map(|report| report.primitive_reuse_bps),
                ),
                mean_final_task_exactness_bps: mean_u32(
                    reports.iter().map(|report| report.final_task_exactness_bps),
                ),
                composition_bottleneck_count: reports
                    .iter()
                    .filter(|report| {
                        matches!(
                            report.failure_layer,
                            TassadarSharedPrimitiveTransferFailureLayer::CompositionLayer
                                | TassadarSharedPrimitiveTransferFailureLayer::Mixed
                        )
                    })
                    .count() as u32,
            },
        )
        .collect()
}

fn classify_failure_layer(
    primitive_reuse_bps: u32,
    composition_gap_bps: u32,
) -> TassadarSharedPrimitiveTransferFailureLayer {
    match (primitive_reuse_bps < 7_500, composition_gap_bps >= 1_000) {
        (false, false) => TassadarSharedPrimitiveTransferFailureLayer::None,
        (true, false) => TassadarSharedPrimitiveTransferFailureLayer::PrimitiveLayer,
        (false, true) => TassadarSharedPrimitiveTransferFailureLayer::CompositionLayer,
        (true, true) => TassadarSharedPrimitiveTransferFailureLayer::Mixed,
    }
}

fn mean_u32(values: impl Iterator<Item = u32>) -> u32 {
    let collected = values.collect::<Vec<_>>();
    if collected.is_empty() {
        0
    } else {
        collected.iter().copied().sum::<u32>() / collected.len() as u32
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarSharedPrimitiveTransferReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarSharedPrimitiveTransferReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSharedPrimitiveTransferReportError::Deserialize {
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
        build_tassadar_shared_primitive_transfer_report, read_repo_json,
        tassadar_shared_primitive_transfer_report_path,
        write_tassadar_shared_primitive_transfer_report,
        TassadarSharedPrimitiveTransferFailureLayer, TassadarSharedPrimitiveTransferRegime,
        TassadarSharedPrimitiveTransferReport,
    };
    use psionic_data::{
        TassadarSharedPrimitiveAlgorithmFamily, TASSADAR_SHARED_PRIMITIVE_TRANSFER_REPORT_REF,
    };

    #[test]
    fn shared_primitive_transfer_report_keeps_primitive_and_composition_failures_separate() {
        let report = build_tassadar_shared_primitive_transfer_report().expect("report");

        assert_eq!(report.case_reports.len(), 12);
        assert!(report
            .ablation_summaries
            .iter()
            .any(|summary| summary.foundational));
        let sudoku_few = report
            .case_reports
            .iter()
            .find(|case| {
                case.held_out_algorithm_family
                    == TassadarSharedPrimitiveAlgorithmFamily::SudokuSearch
                    && case.regime == TassadarSharedPrimitiveTransferRegime::FewShot
            })
            .expect("sudoku few-shot");
        assert_eq!(
            sudoku_few.failure_layer,
            TassadarSharedPrimitiveTransferFailureLayer::CompositionLayer
        );
    }

    #[test]
    fn shared_primitive_transfer_report_matches_committed_truth() {
        let generated = build_tassadar_shared_primitive_transfer_report().expect("report");
        let committed: TassadarSharedPrimitiveTransferReport =
            read_repo_json(TASSADAR_SHARED_PRIMITIVE_TRANSFER_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_shared_primitive_transfer_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_shared_primitive_transfer_report.json");
        let written =
            write_tassadar_shared_primitive_transfer_report(&output_path).expect("write report");
        let persisted: TassadarSharedPrimitiveTransferReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_shared_primitive_transfer_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_shared_primitive_transfer_report.json")
        );
    }
}
