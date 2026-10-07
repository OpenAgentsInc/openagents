use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::PARAMETER_GOLF_SUBMISSION_METRIC_ID;
use psionic_train::{
    PARAMETER_GOLF_NON_RECORD_SUBMISSION_VERSION, PARAMETER_GOLF_SUBMISSION_ARTIFACT_CAP_BYTES,
    ParameterGolfLocalReferenceFixture, ParameterGolfNonRecordSubmissionConfig,
    ParameterGolfReferenceTrainingConfig, ParameterGolfReferenceTrainingError,
    benchmark_parameter_golf_local_reference, build_parameter_golf_non_record_submission_bundle,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Canonical committed report for the Parameter Golf post-parity research harness.
pub const PARAMETER_GOLF_RESEARCH_HARNESS_REPORT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_research_harness_report.json";

/// High-level family for one post-parity Parameter Golf variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfResearchVariantFamily {
    BaselineControl,
    SharedDepthRecurrence,
    StrongerParameterTying,
    CompressionQuantization,
}

/// Current status for one research-harness variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfResearchVariantStatus {
    ImplementedBaseline,
    PlannedResearchCandidate,
}

/// One measured baseline metric tuple reused by the research harness.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfResearchMeasuredMetrics {
    /// Final roundtrip validation loss.
    pub val_loss: f64,
    /// Final roundtrip validation bits-per-byte.
    pub val_bpb: f64,
    /// Total counted submission bytes.
    pub bytes_total: u64,
    /// Counted code bytes.
    pub bytes_code: u64,
    /// Counted compressed-model bytes.
    pub bytes_model_int8_zlib: u64,
    /// Stable logical training duration from the fixed local-reference config.
    pub logical_training_ms: u64,
}

/// Shared oracle and accounting surface that every research candidate must reuse.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfResearchComparisonSurface {
    /// Stable training-token digest for the fixed baseline fixture.
    pub training_dataset_digest: String,
    /// Stable validation-token digest for the fixed baseline fixture.
    pub validation_dataset_digest: String,
    /// Canonical benchmark reference reused by the baseline package.
    pub benchmark_ref: String,
    /// Canonical submission metric identifier.
    pub submission_metric_id: String,
    /// Stable batch sequence length for the baseline harness.
    pub sequence_length: usize,
    /// Public artifact cap in decimal bytes.
    pub artifact_cap_bytes: u64,
    /// Ordered counted-component identifiers used by the packaging lane.
    pub counted_component_ids: Vec<String>,
    /// Baseline package version that owns the current accounting posture.
    pub baseline_submission_package_version: String,
    /// Stable digest of the shipped submission entrypoint.
    pub baseline_entrypoint_artifact_digest: String,
    /// Stable digest of the counted compressed-model artifact.
    pub baseline_model_artifact_digest: String,
    /// Baseline accounting receipt digest.
    pub baseline_accounting_receipt_digest: String,
}

/// One research candidate or measured baseline variant inside the harness.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfResearchVariantReport {
    /// Stable variant identifier.
    pub variant_id: String,
    /// Stable family kind.
    pub family: ParameterGolfResearchVariantFamily,
    /// Current variant status.
    pub status: ParameterGolfResearchVariantStatus,
    /// Human-readable description.
    pub description: String,
    /// Ordered changed surfaces for the candidate.
    pub changed_surfaces: Vec<String>,
    /// Oracle guardrails that must remain fixed versus baseline.
    pub oracle_guardrails: Vec<String>,
    /// Accounting guardrails that must remain fixed versus baseline.
    pub accounting_guardrails: Vec<String>,
    /// Honest boundary note for the variant.
    pub boundary_note: String,
    /// Measured metrics when the variant is the current baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_metrics: Option<ParameterGolfResearchMeasuredMetrics>,
}

/// Committed report for the Parameter Golf post-parity research harness.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfResearchHarnessReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Explicit claim class for the harness.
    pub claim_class: String,
    /// Canonical benchmark reference.
    pub benchmark_ref: String,
    /// Stable baseline run identifier.
    pub baseline_run_id: String,
    /// Shared comparison surface every variant must reuse.
    pub comparison_surface: ParameterGolfResearchComparisonSurface,
    /// Ordered baseline or candidate variants.
    pub variants: Vec<ParameterGolfResearchVariantReport>,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl ParameterGolfResearchHarnessReport {
    fn new(
        baseline_run_id: impl Into<String>,
        comparison_surface: ParameterGolfResearchComparisonSurface,
        variants: Vec<ParameterGolfResearchVariantReport>,
    ) -> Self {
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("parameter_golf.research_harness.v1"),
            claim_class: String::from("research_only_variant_harness"),
            benchmark_ref: comparison_surface.benchmark_ref.clone(),
            baseline_run_id: baseline_run_id.into(),
            comparison_surface,
            variants,
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_parameter_golf_research_harness_report|", &report);
        report
    }
}

/// Failure while building or persisting the Parameter Golf research harness.
#[derive(Debug, Error)]
pub enum ParameterGolfResearchHarnessError {
    #[error(transparent)]
    ReferenceTraining(#[from] ParameterGolfReferenceTrainingError),
    #[error(transparent)]
    Training(#[from] psionic_train::ParameterGolfBenchmarkBundleError),
    #[error(transparent)]
    Submission(#[from] psionic_train::ParameterGolfSubmissionError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        artifact_kind: String,
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed Parameter Golf post-parity research harness report.
pub fn build_parameter_golf_research_harness_report(
) -> Result<ParameterGolfResearchHarnessReport, ParameterGolfResearchHarnessError> {
    let fixture = ParameterGolfLocalReferenceFixture::reference()?;
    let training_config = ParameterGolfReferenceTrainingConfig::local_reference();
    let benchmark_bundle = benchmark_parameter_golf_local_reference(&fixture, &training_config)?;
    let submission_bundle = build_parameter_golf_non_record_submission_bundle(
        &benchmark_bundle,
        &ParameterGolfNonRecordSubmissionConfig::local_reference_defaults(),
    )?;

    let oracle_guardrails = vec![
        format!(
            "reuse fixed training_dataset_digest={}",
            fixture.training_digest()
        ),
        format!(
            "reuse fixed validation_dataset_digest={}",
            fixture.validation_digest()
        ),
        format!(
            "reuse benchmark_ref={}",
            benchmark_bundle.benchmark_receipt.benchmark_ref
        ),
        format!("reuse submission_metric_id={}", PARAMETER_GOLF_SUBMISSION_METRIC_ID),
        String::from("reuse the same exact val_loss and val_bpb oracle and byte-accounting path"),
    ];
    let accounting_guardrails = submission_bundle
        .accounting_receipt
        .counted_components
        .iter()
        .map(|component| {
            format!(
                "preserve counted component `{}` with explicit bytes accounting",
                component.component_id
            )
        })
        .collect::<Vec<_>>();

    let comparison_surface = ParameterGolfResearchComparisonSurface {
        training_dataset_digest: fixture.training_digest(),
        validation_dataset_digest: fixture.validation_digest(),
        benchmark_ref: benchmark_bundle.benchmark_receipt.benchmark_ref.clone(),
        submission_metric_id: String::from(PARAMETER_GOLF_SUBMISSION_METRIC_ID),
        sequence_length: training_config.geometry.train_sequence_length,
        artifact_cap_bytes: PARAMETER_GOLF_SUBMISSION_ARTIFACT_CAP_BYTES,
        counted_component_ids: submission_bundle
            .accounting_receipt
            .counted_components
            .iter()
            .map(|component| component.component_id.clone())
            .collect(),
        baseline_submission_package_version: String::from(
            PARAMETER_GOLF_NON_RECORD_SUBMISSION_VERSION,
        ),
        baseline_entrypoint_artifact_digest: submission_bundle
            .artifact("train_gpt.py")
            .expect("submission entrypoint should exist")
            .artifact_digest
            .clone(),
        baseline_model_artifact_digest: benchmark_bundle
            .training_outcome
            .int8_zlib_model_artifact
            .artifact_digest
            .clone(),
        baseline_accounting_receipt_digest: submission_bundle
            .accounting_receipt
            .receipt_digest
            .clone(),
    };

    let variants = vec![
        ParameterGolfResearchVariantReport {
            variant_id: String::from("baseline_control"),
            family: ParameterGolfResearchVariantFamily::BaselineControl,
            status: ParameterGolfResearchVariantStatus::ImplementedBaseline,
            description: String::from(
                "Current Psionic local-reference control bound to the same oracle, benchmark receipt, and non-record submission accounting surface.",
            ),
            changed_surfaces: vec![
                String::from("public_9x512_decoder"),
                String::from("current_muon_plus_adam_optimizer_split"),
                String::from("current_int8_zlib_export"),
            ],
            oracle_guardrails: oracle_guardrails.clone(),
            accounting_guardrails: accounting_guardrails.clone(),
            boundary_note: String::from(
                "This is the measured local-reference control for research comparison only; it is not a challenge-speed 8xH100 result.",
            ),
            measured_metrics: Some(ParameterGolfResearchMeasuredMetrics {
                val_loss: submission_bundle.submission_manifest.val_loss,
                val_bpb: submission_bundle.submission_manifest.val_bpb,
                bytes_total: submission_bundle.submission_manifest.bytes_total,
                bytes_code: submission_bundle.submission_manifest.bytes_code,
                bytes_model_int8_zlib: submission_bundle
                    .submission_manifest
                    .bytes_model_int8_zlib,
                logical_training_ms: training_config
                    .max_steps
                    .saturating_mul(training_config.step_duration_ms),
            }),
        },
        ParameterGolfResearchVariantReport {
            variant_id: String::from("shared_depth_recurrence_candidate"),
            family: ParameterGolfResearchVariantFamily::SharedDepthRecurrence,
            status: ParameterGolfResearchVariantStatus::PlannedResearchCandidate,
            description: String::from(
                "Research-only candidate that reuses one or more decoder blocks across recurrent refinement steps or explicit shared-depth passes.",
            ),
            changed_surfaces: vec![
                String::from("shared_depth_decoder_block"),
                String::from("recurrent_refinement_schedule"),
                String::from("fixed_or_dynamic_refinement_budget"),
            ],
            oracle_guardrails: oracle_guardrails.clone(),
            accounting_guardrails: accounting_guardrails.clone(),
            boundary_note: String::from(
                "Do not compare any shared-depth or recurrent candidate unless it reuses the same oracle digests and the same counted-byte vocabulary as baseline.",
            ),
            measured_metrics: None,
        },
        ParameterGolfResearchVariantReport {
            variant_id: String::from("stronger_parameter_tying_candidate"),
            family: ParameterGolfResearchVariantFamily::StrongerParameterTying,
            status: ParameterGolfResearchVariantStatus::PlannedResearchCandidate,
            description: String::from(
                "Research-only candidate that pushes stronger parameter tying across embeddings, blocks, control tensors, or output surfaces while keeping the same scoreboard contract.",
            ),
            changed_surfaces: vec![
                String::from("embedding_and_output_parameter_tying"),
                String::from("cross_block_weight_sharing"),
                String::from("control_tensor_sharing"),
            ],
            oracle_guardrails: oracle_guardrails.clone(),
            accounting_guardrails: accounting_guardrails.clone(),
            boundary_note: String::from(
                "Parameter-tying experiments are only valid inside this harness when the score still comes from the fixed FineWeb validation oracle and the same package accounting surface.",
            ),
            measured_metrics: None,
        },
        ParameterGolfResearchVariantReport {
            variant_id: String::from("compression_quantization_candidate"),
            family: ParameterGolfResearchVariantFamily::CompressionQuantization,
            status: ParameterGolfResearchVariantStatus::PlannedResearchCandidate,
            description: String::from(
                "Research-only candidate that widens artifact compression, post-train quantization, or future low-precision export recipes without changing the comparison oracle.",
            ),
            changed_surfaces: vec![
                String::from("post_train_quantization_recipe"),
                String::from("artifact_codec"),
                String::from("compressed_model_layout"),
            ],
            oracle_guardrails,
            accounting_guardrails,
            boundary_note: String::from(
                "Compression or quantization changes only belong in this harness when they still report the same counted components and final roundtrip metric as the baseline lane.",
            ),
            measured_metrics: None,
        },
    ];

    Ok(ParameterGolfResearchHarnessReport::new(
        benchmark_bundle.run_bundle.run_id,
        comparison_surface,
        variants,
    ))
}

/// Returns the canonical absolute path for the committed Parameter Golf research report.
#[must_use]
pub fn parameter_golf_research_harness_report_path() -> PathBuf {
    repo_root().join(PARAMETER_GOLF_RESEARCH_HARNESS_REPORT_REF)
}

/// Writes the committed Parameter Golf research harness report.
pub fn write_parameter_golf_research_harness_report(
    output_path: impl AsRef<Path>,
) -> Result<ParameterGolfResearchHarnessReport, ParameterGolfResearchHarnessError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfResearchHarnessError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_parameter_golf_research_harness_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        ParameterGolfResearchHarnessError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
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
) -> Result<T, ParameterGolfResearchHarnessError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| ParameterGolfResearchHarnessError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| ParameterGolfResearchHarnessError::Deserialize {
        artifact_kind: String::from("parameter_golf_research_harness_report"),
        path: path.display().to_string(),
        error,
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
        PARAMETER_GOLF_RESEARCH_HARNESS_REPORT_REF, ParameterGolfResearchHarnessReport,
        build_parameter_golf_research_harness_report, parameter_golf_research_harness_report_path,
        read_repo_json, write_parameter_golf_research_harness_report,
    };

    #[test]
    fn parameter_golf_research_harness_carries_measured_control_and_planned_candidates() {
        let report = build_parameter_golf_research_harness_report().expect("build report");

        assert_eq!(report.variants.len(), 4);
        assert!(report
            .variants
            .iter()
            .any(|variant| variant.variant_id == "baseline_control"
                && variant.measured_metrics.is_some()));
        assert!(report
            .variants
            .iter()
            .filter(|variant| variant.variant_id != "baseline_control")
            .all(|variant| variant.measured_metrics.is_none()));
        assert_eq!(
            report.comparison_surface.counted_component_ids.len(),
            5
        );
    }

    #[test]
    fn parameter_golf_research_harness_report_matches_committed_truth() {
        let generated = build_parameter_golf_research_harness_report().expect("build report");
        let committed: ParameterGolfResearchHarnessReport =
            read_repo_json(PARAMETER_GOLF_RESEARCH_HARNESS_REPORT_REF).expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_parameter_golf_research_harness_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("parameter_golf_research_harness_report.json");
        let written =
            write_parameter_golf_research_harness_report(&output_path).expect("write report");
        let persisted: ParameterGolfResearchHarnessReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            parameter_golf_research_harness_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("parameter_golf_research_harness_report.json")
        );
    }
}
