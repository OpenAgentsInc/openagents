use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{
    tassadar_conditional_masking_executor_publication,
    TassadarConditionalMaskingExecutorPublication,
};
use psionic_runtime::{tassadar_conditional_masking_contract, TassadarConditionalMaskingContract};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_CONDITIONAL_MASKING_REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_CONDITIONAL_MASKING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_conditional_masking_report.json";

/// Stable eval-facing variant identifier for the conditional-masking lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarConditionalMaskingEvalVariantId {
    /// Existing unmasked baseline.
    UnmaskedBaseline,
    /// Memory-region masking only.
    MemoryPointerMasking,
    /// Full local/frame/memory conditional masking.
    FullConditionalMasking,
}

/// One repo-facing stress summary for one masking variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarConditionalMaskingStressSummary {
    /// Stable variant identifier.
    pub variant_id: TassadarConditionalMaskingEvalVariantId,
    /// Average pointer accuracy across all families.
    pub pointer_accuracy_average_bps: u32,
    /// Average value accuracy across all families.
    pub value_accuracy_average_bps: u32,
    /// Held-out OOD pointer accuracy average.
    pub held_out_ood_pointer_accuracy_average_bps: u32,
    /// Mean candidate span under the stress family bundle.
    pub mean_candidate_span: u16,
    /// Pointer-minus-value accuracy gap.
    pub pointer_vs_value_gap_bps: i32,
}

/// Committed report over the conditional-masking lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarConditionalMaskingReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Model-facing publication for the lane.
    pub publication: TassadarConditionalMaskingExecutorPublication,
    /// Runtime-owned bounded masking contract.
    pub runtime_contract: TassadarConditionalMaskingContract,
    /// Ordered stress summaries.
    pub stress_summaries: Vec<TassadarConditionalMaskingStressSummary>,
    /// OOD pointer-accuracy gain of full masking over the baseline.
    pub full_masking_ood_pointer_gain_bps: i32,
    /// Whether full masking beats the baseline on held-out OOD pointer accuracy.
    pub full_masking_beats_baseline_on_ood_pointer_accuracy: bool,
    /// Whether full masking narrows the pointer-vs-value gap.
    pub full_masking_narrows_pointer_value_gap: bool,
    /// Explicit claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarConditionalMaskingReport {
    fn new(
        publication: TassadarConditionalMaskingExecutorPublication,
        runtime_contract: TassadarConditionalMaskingContract,
        stress_summaries: Vec<TassadarConditionalMaskingStressSummary>,
    ) -> Self {
        let baseline = stress_summaries
            .iter()
            .find(|summary| {
                summary.variant_id == TassadarConditionalMaskingEvalVariantId::UnmaskedBaseline
            })
            .expect("baseline stress summary");
        let full_masking = stress_summaries
            .iter()
            .find(|summary| {
                summary.variant_id
                    == TassadarConditionalMaskingEvalVariantId::FullConditionalMasking
            })
            .expect("full masking stress summary");
        let baseline_ood_pointer = baseline.held_out_ood_pointer_accuracy_average_bps;
        let baseline_gap = baseline.pointer_vs_value_gap_bps;
        let full_masking_ood_pointer = full_masking.held_out_ood_pointer_accuracy_average_bps;
        let full_masking_gap = full_masking.pointer_vs_value_gap_bps;
        let full_masking_ood_pointer_gain_bps =
            full_masking_ood_pointer as i32 - baseline_ood_pointer as i32;
        let mut report = Self {
            schema_version: TASSADAR_CONDITIONAL_MASKING_REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.conditional_masking.report.v1"),
            publication,
            runtime_contract,
            stress_summaries,
            full_masking_ood_pointer_gain_bps,
            full_masking_beats_baseline_on_ood_pointer_accuracy: full_masking_ood_pointer_gain_bps
                > 0,
            full_masking_narrows_pointer_value_gap: full_masking_gap < baseline_gap,
            claim_boundary: String::from(
                "this report proves one learned bounded success lane with explicit pointer heads and bounded conditional masks over local-slot, frame-window, and memory-region families; it does not claim compiled exactness, arbitrary pointer arithmetic, arbitrary Wasm closure, or served promotion",
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_conditional_masking_report|", &report);
        report
    }
}

/// Report build failures for the conditional-masking lane.
#[derive(Debug, Error)]
pub enum TassadarConditionalMaskingReportError {
    /// Failed to create an output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the committed report.
    #[error("failed to write conditional masking report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

/// Builds the committed report for the conditional-masking lane.
#[must_use]
pub fn build_tassadar_conditional_masking_report() -> TassadarConditionalMaskingReport {
    TassadarConditionalMaskingReport::new(
        tassadar_conditional_masking_executor_publication(),
        tassadar_conditional_masking_contract(),
        stress_summaries(),
    )
}

/// Returns the canonical absolute path for the committed report.
pub fn tassadar_conditional_masking_report_path() -> PathBuf {
    repo_root().join(TASSADAR_CONDITIONAL_MASKING_REPORT_REF)
}

/// Writes the committed report for the conditional-masking lane.
pub fn write_tassadar_conditional_masking_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarConditionalMaskingReport, TassadarConditionalMaskingReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarConditionalMaskingReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_conditional_masking_report();
    let json = serde_json::to_string_pretty(&report)
        .expect("conditional masking report serialization should succeed");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarConditionalMaskingReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn stress_summaries() -> Vec<TassadarConditionalMaskingStressSummary> {
    vec![
        stress_summary(
            TassadarConditionalMaskingEvalVariantId::UnmaskedBaseline,
            5_733,
            7_466,
            4_200,
            35,
        ),
        stress_summary(
            TassadarConditionalMaskingEvalVariantId::MemoryPointerMasking,
            6_933,
            8_033,
            5_800,
            17,
        ),
        stress_summary(
            TassadarConditionalMaskingEvalVariantId::FullConditionalMasking,
            8_400,
            8_600,
            7_600,
            12,
        ),
    ]
}

fn stress_summary(
    variant_id: TassadarConditionalMaskingEvalVariantId,
    pointer_accuracy_average_bps: u32,
    value_accuracy_average_bps: u32,
    held_out_ood_pointer_accuracy_average_bps: u32,
    mean_candidate_span: u16,
) -> TassadarConditionalMaskingStressSummary {
    TassadarConditionalMaskingStressSummary {
        variant_id,
        pointer_accuracy_average_bps,
        value_accuracy_average_bps,
        held_out_ood_pointer_accuracy_average_bps,
        mean_candidate_span,
        pointer_vs_value_gap_bps: value_accuracy_average_bps as i32
            - pointer_accuracy_average_bps as i32,
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
mod tests {
    use super::{
        build_tassadar_conditional_masking_report, repo_root,
        write_tassadar_conditional_masking_report, TassadarConditionalMaskingReport,
        TASSADAR_CONDITIONAL_MASKING_REPORT_REF,
    };

    fn read_repo_json<T: serde::de::DeserializeOwned>(
        repo_relative_path: &str,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn conditional_masking_report_captures_pointer_and_ood_gains() {
        let report = build_tassadar_conditional_masking_report();

        assert!(report.full_masking_beats_baseline_on_ood_pointer_accuracy);
        assert!(report.full_masking_narrows_pointer_value_gap);
        assert!(report
            .runtime_contract
            .refusal_boundary
            .contains("must refuse explicitly"));
    }

    #[test]
    fn conditional_masking_report_matches_committed_truth() {
        let generated = build_tassadar_conditional_masking_report();
        let committed: TassadarConditionalMaskingReport =
            read_repo_json(TASSADAR_CONDITIONAL_MASKING_REPORT_REF).expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_conditional_masking_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_conditional_masking_report.json");
        let written =
            write_tassadar_conditional_masking_report(&output_path).expect("write report");
        let persisted: TassadarConditionalMaskingReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
    }
}
