use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{
    tassadar_precision_attention_audit_publication, TassadarPrecisionAttentionAuditPublication,
    TASSADAR_PRECISION_ATTENTION_ROBUSTNESS_AUDIT_REPORT_REF,
};
use psionic_runtime::{
    build_tassadar_precision_attention_runtime_audit_report,
    TassadarPrecisionAttentionRuntimeAuditError, TassadarPrecisionAttentionRuntimeAuditReport,
    TassadarRuntimeAttentionSemanticsFamily, TassadarRuntimeNumericRegime,
    TassadarRuntimeRobustnessDriftClass,
    TASSADAR_PRECISION_ATTENTION_RUNTIME_AUDIT_REPORT_REF,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_efficient_attention_baseline_matrix_report,
    TassadarEfficientAttentionBaselineFamilyKind,
    TassadarEfficientAttentionBaselineMatrixError,
    TassadarEfficientAttentionBaselineMatrixReport,
    TASSADAR_EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Aggregate summary for one numeric-regime and attention-family combination.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarPrecisionAttentionRegimeSummary {
    /// Audited numeric regime.
    pub numeric_regime: TassadarRuntimeNumericRegime,
    /// Audited attention family.
    pub attention_family: TassadarRuntimeAttentionSemanticsFamily,
    /// Exact workload count under the regime.
    pub exact_workload_count: u32,
    /// Approximate bounded workload count under the regime.
    pub approximate_workload_count: u32,
    /// Refused workload count under the regime.
    pub refused_workload_count: u32,
    /// Mean exactness across the regime receipts.
    pub average_exactness_bps: f64,
    /// Same-harness attention baseline row used for alignment.
    pub baseline_alignment_family: TassadarEfficientAttentionBaselineFamilyKind,
    /// Plain-language note.
    pub note: String,
}

/// Aggregate summary for one workload family across all audited regimes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPrecisionAttentionWorkloadSummary {
    /// Stable workload family identifier.
    pub workload_family_id: String,
    /// Exact regime count for the workload.
    pub exact_regime_count: u32,
    /// Approximate bounded regime count for the workload.
    pub approximate_regime_count: u32,
    /// Refused regime count for the workload.
    pub refused_regime_count: u32,
    /// Ordered exact regime labels.
    pub exact_regimes: Vec<String>,
    /// First refusal reason observed for the workload when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_refusal_reason: Option<String>,
    /// Plain-language note.
    pub note: String,
}

/// Committed eval report over numeric and attention robustness.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarPrecisionAttentionRobustnessAuditReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Public publication anchoring the audited regimes.
    pub publication: TassadarPrecisionAttentionAuditPublication,
    /// Runtime audit report reused by this eval surface.
    pub runtime_audit_report: TassadarPrecisionAttentionRuntimeAuditReport,
    /// Efficient-attention matrix ref used for attention-family alignment.
    pub efficient_attention_matrix_ref: String,
    /// Efficient-attention matrix digest used for attention-family alignment.
    pub efficient_attention_matrix_digest: String,
    /// Ordered regime summaries.
    pub regime_summaries: Vec<TassadarPrecisionAttentionRegimeSummary>,
    /// Ordered workload summaries.
    pub workload_summaries: Vec<TassadarPrecisionAttentionWorkloadSummary>,
    /// Ordered refs used to generate the report.
    pub generated_from_refs: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Report summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarPrecisionAttentionRobustnessAuditReport {
    fn new(
        publication: TassadarPrecisionAttentionAuditPublication,
        runtime_audit_report: TassadarPrecisionAttentionRuntimeAuditReport,
        efficient_attention_matrix: &TassadarEfficientAttentionBaselineMatrixReport,
        regime_summaries: Vec<TassadarPrecisionAttentionRegimeSummary>,
        workload_summaries: Vec<TassadarPrecisionAttentionWorkloadSummary>,
    ) -> Self {
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.precision_attention_robustness_audit.report.v1"),
            publication,
            runtime_audit_report,
            efficient_attention_matrix_ref: String::from(
                TASSADAR_EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF,
            ),
            efficient_attention_matrix_digest: efficient_attention_matrix.report_digest.clone(),
            regime_summaries,
            workload_summaries,
            generated_from_refs: vec![
                String::from(TASSADAR_PRECISION_ATTENTION_RUNTIME_AUDIT_REPORT_REF),
                String::from(TASSADAR_EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF),
            ],
            claim_boundary: String::from(
                "this report joins the deterministic runtime precision/attention audit with the same-harness efficient-attention matrix so numeric regimes and hard/sparse/soft attention semantics stay explicit. Exact, approximate_bounded, and refused remain separate outcomes, and proxy alignment here does not widen served capability claims",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        let regime_count = report.regime_summaries.len();
        let workload_count = report.workload_summaries.len();
        let fragile_workload_count = report
            .workload_summaries
            .iter()
            .filter(|summary| summary.exact_regime_count <= 3)
            .count();
        report.summary = format!(
            "Precision/attention robustness audit now summarizes {} numeric/attention regimes across {} workloads, with {} workloads exact under at most three audited regimes and the rest carrying broader finite-precision headroom only where the audit stays explicit.",
            regime_count,
            workload_count,
            fragile_workload_count,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_precision_attention_robustness_audit_report|",
            &report,
        );
        report
    }
}

/// Robustness-audit build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarPrecisionAttentionRobustnessAuditError {
    /// Building the runtime audit failed.
    #[error(transparent)]
    Runtime(#[from] TassadarPrecisionAttentionRuntimeAuditError),
    /// Building the efficient-attention matrix failed.
    #[error(transparent)]
    EfficientAttention(#[from] TassadarEfficientAttentionBaselineMatrixError),
    /// Failed to create an output directory.
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the report.
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// Failed to read one committed artifact.
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    /// Failed to decode one committed artifact.
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    /// JSON serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed robustness audit report.
pub fn build_tassadar_precision_attention_robustness_audit_report(
) -> Result<TassadarPrecisionAttentionRobustnessAuditReport, TassadarPrecisionAttentionRobustnessAuditError>
{
    let publication = tassadar_precision_attention_audit_publication();
    let runtime_audit_report = build_tassadar_precision_attention_runtime_audit_report();
    let efficient_attention_matrix = build_tassadar_efficient_attention_baseline_matrix_report()?;
    let regime_summaries = build_regime_summaries(&runtime_audit_report);
    let workload_summaries = build_workload_summaries(&runtime_audit_report);
    Ok(TassadarPrecisionAttentionRobustnessAuditReport::new(
        publication,
        runtime_audit_report,
        &efficient_attention_matrix,
        regime_summaries,
        workload_summaries,
    ))
}

/// Returns the canonical absolute path for the committed robustness audit report.
#[must_use]
pub fn tassadar_precision_attention_robustness_audit_report_path() -> PathBuf {
    repo_root().join(TASSADAR_PRECISION_ATTENTION_ROBUSTNESS_AUDIT_REPORT_REF)
}

/// Writes the committed robustness audit report.
pub fn write_tassadar_precision_attention_robustness_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarPrecisionAttentionRobustnessAuditReport, TassadarPrecisionAttentionRobustnessAuditError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPrecisionAttentionRobustnessAuditError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_precision_attention_robustness_audit_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPrecisionAttentionRobustnessAuditError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_regime_summaries(
    runtime_audit_report: &TassadarPrecisionAttentionRuntimeAuditReport,
) -> Vec<TassadarPrecisionAttentionRegimeSummary> {
    let mut grouped =
        BTreeMap::<(TassadarRuntimeNumericRegime, TassadarRuntimeAttentionSemanticsFamily), Vec<_>>::new();
    for receipt in &runtime_audit_report.receipts {
        grouped
            .entry((receipt.numeric_regime, receipt.attention_family))
            .or_default()
            .push(receipt);
    }

    grouped
        .into_iter()
        .map(|((numeric_regime, attention_family), receipts)| {
            let exact_workload_count = receipts
                .iter()
                .filter(|receipt| receipt.drift_class == TassadarRuntimeRobustnessDriftClass::Exact)
                .count() as u32;
            let approximate_workload_count = receipts
                .iter()
                .filter(|receipt| {
                    receipt.drift_class
                        == TassadarRuntimeRobustnessDriftClass::ApproximateBounded
                })
                .count() as u32;
            let refused_workload_count = receipts
                .iter()
                .filter(|receipt| receipt.drift_class == TassadarRuntimeRobustnessDriftClass::Refused)
                .count() as u32;
            let average_exactness_bps = receipts
                .iter()
                .map(|receipt| receipt.exactness_bps as f64)
                .sum::<f64>()
                / receipts.len() as f64;
            TassadarPrecisionAttentionRegimeSummary {
                numeric_regime,
                attention_family,
                exact_workload_count,
                approximate_workload_count,
                refused_workload_count,
                average_exactness_bps: round_metric(average_exactness_bps),
                baseline_alignment_family: alignment_family(attention_family),
                note: format!(
                    "regime `{}` plus `{}` aligns to `{}` in the same-harness attention matrix while keeping finite-precision drift explicit",
                    numeric_regime.as_str(),
                    attention_family.as_str(),
                    alignment_family(attention_family).label(),
                ),
            }
        })
        .collect()
}

fn build_workload_summaries(
    runtime_audit_report: &TassadarPrecisionAttentionRuntimeAuditReport,
) -> Vec<TassadarPrecisionAttentionWorkloadSummary> {
    let mut grouped = BTreeMap::<String, Vec<_>>::new();
    for receipt in &runtime_audit_report.receipts {
        grouped
            .entry(receipt.workload_family_id.clone())
            .or_default()
            .push(receipt);
    }

    grouped
        .into_iter()
        .map(|(workload_family_id, receipts)| {
            let exact_regimes = receipts
                .iter()
                .filter(|receipt| receipt.drift_class == TassadarRuntimeRobustnessDriftClass::Exact)
                .map(|receipt| {
                    format!(
                        "{}:{}",
                        receipt.numeric_regime.as_str(),
                        receipt.attention_family.as_str()
                    )
                })
                .collect::<Vec<_>>();
            let first_refusal_reason = receipts
                .iter()
                .find_map(|receipt| receipt.refusal_reason.clone());
            let exact_regime_count = exact_regimes.len() as u32;
            let approximate_regime_count = receipts
                .iter()
                .filter(|receipt| {
                    receipt.drift_class
                        == TassadarRuntimeRobustnessDriftClass::ApproximateBounded
                })
                .count() as u32;
            let refused_regime_count = receipts
                .iter()
                .filter(|receipt| receipt.drift_class == TassadarRuntimeRobustnessDriftClass::Refused)
                .count() as u32;
            TassadarPrecisionAttentionWorkloadSummary {
                workload_family_id: workload_family_id.clone(),
                exact_regime_count,
                approximate_regime_count,
                refused_regime_count,
                exact_regimes,
                first_refusal_reason,
                note: format!(
                    "workload `{}` stays exact under {} audited regimes, approximate under {}, and refused under {}",
                    workload_family_id,
                    exact_regime_count,
                    approximate_regime_count,
                    refused_regime_count,
                ),
            }
        })
        .collect()
}

fn alignment_family(
    attention_family: TassadarRuntimeAttentionSemanticsFamily,
) -> TassadarEfficientAttentionBaselineFamilyKind {
    match attention_family {
        TassadarRuntimeAttentionSemanticsFamily::HardSelectionReference => {
            TassadarEfficientAttentionBaselineFamilyKind::DenseReferenceLinear
        }
        TassadarRuntimeAttentionSemanticsFamily::SparseValidated => {
            TassadarEfficientAttentionBaselineFamilyKind::SparseTopKValidated
        }
        TassadarRuntimeAttentionSemanticsFamily::SoftProxy => {
            TassadarEfficientAttentionBaselineFamilyKind::ReformerChunkedProxy
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarPrecisionAttentionRobustnessAuditError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarPrecisionAttentionRobustnessAuditError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPrecisionAttentionRobustnessAuditError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

fn round_metric(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
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
        build_tassadar_precision_attention_robustness_audit_report, read_repo_json,
        tassadar_precision_attention_robustness_audit_report_path,
        write_tassadar_precision_attention_robustness_audit_report,
        TassadarPrecisionAttentionRobustnessAuditReport,
    };
    use psionic_models::{
        TASSADAR_PRECISION_ATTENTION_ROBUSTNESS_AUDIT_REPORT_REF,
    };
    use psionic_runtime::{TassadarRuntimeAttentionSemanticsFamily, TassadarRuntimeNumericRegime};

    #[test]
    fn precision_attention_robustness_audit_tracks_regime_survival_and_refusal() {
        let report = build_tassadar_precision_attention_robustness_audit_report()
            .expect("robustness report");

        assert!(report.regime_summaries.iter().any(|summary| {
            summary.numeric_regime == TassadarRuntimeNumericRegime::Fp32Reference
                && summary.attention_family
                    == TassadarRuntimeAttentionSemanticsFamily::HardSelectionReference
                && summary.exact_workload_count == 6
        }));
        assert!(report.workload_summaries.iter().any(|summary| {
            summary.workload_family_id == "sudoku_class" && summary.refused_regime_count > 0
        }));
    }

    #[test]
    fn precision_attention_robustness_audit_matches_committed_truth() {
        let generated = build_tassadar_precision_attention_robustness_audit_report()
            .expect("robustness report");
        let committed: TassadarPrecisionAttentionRobustnessAuditReport =
            read_repo_json(TASSADAR_PRECISION_ATTENTION_ROBUSTNESS_AUDIT_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_precision_attention_robustness_audit_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_precision_attention_robustness_audit.json");
        let written = write_tassadar_precision_attention_robustness_audit_report(&output_path)
            .expect("write robustness report");
        let persisted: TassadarPrecisionAttentionRobustnessAuditReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_precision_attention_robustness_audit_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_precision_attention_robustness_audit.json")
        );
    }
}
