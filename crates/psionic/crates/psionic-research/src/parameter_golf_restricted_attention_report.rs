use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{ParameterGolfSentencePieceByteLuts, ParameterGolfSentencePieceTokenEntry};
use psionic_eval::{
    evaluate_parameter_golf_validation, evaluate_parameter_golf_windowed_validation,
    ParameterGolfValidationEvalError, ParameterGolfWindowedValidationEvalError,
};
use psionic_train::{
    benchmark_parameter_golf_local_reference, build_parameter_golf_non_record_submission_bundle,
    export_parameter_golf_int8_zlib_model_artifact, ParameterGolfBenchmarkBundleError,
    ParameterGolfLocalReferenceFixture, ParameterGolfNonRecordSubmissionConfig,
    ParameterGolfReferenceTrainingConfig, ParameterGolfReferenceTrainingError,
    ParameterGolfSubmissionError, PARAMETER_GOLF_NON_RECORD_SUBMISSION_VERSION,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Canonical committed challenge-format validation fixture for restricted-attention evidence.
pub const PARAMETER_GOLF_SEQ1024_VALIDATION_FIXTURE_REF: &str =
    "fixtures/parameter_golf/parity/parameter_golf_seq1024_validation_fixture.json";
/// Canonical committed restricted-attention report.
pub const PARAMETER_GOLF_RESTRICTED_ATTENTION_REPORT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_restricted_attention_report.json";
/// Fixed restricted-attention candidate window recorded by the report.
pub const PARAMETER_GOLF_RESTRICTED_ATTENTION_WINDOW_SIZE: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ParameterGolfSeq1024ValidationFixture {
    fixture_id: String,
    claim_boundary: String,
    source_readme_ref: String,
    tokenizer_model_ref: String,
    tokenizer_model_digest: String,
    validation_shard_ref: String,
    validation_window_digest: String,
    sequence_length: usize,
    tokenizer_vocab_size: usize,
    validation_tokens: Vec<u16>,
    sentencepiece_entries: Vec<ParameterGolfSentencePieceTokenEntry>,
}

impl ParameterGolfSeq1024ValidationFixture {
    fn stable_digest(&self) -> String {
        stable_digest(
            b"psionic_parameter_golf_seq1024_validation_fixture|",
            self,
        )
    }

    fn byte_luts(&self) -> Result<ParameterGolfSentencePieceByteLuts, ParameterGolfRestrictedAttentionReportError> {
        Ok(ParameterGolfSentencePieceByteLuts::build(
            self.tokenizer_vocab_size,
            self.sentencepiece_entries.as_slice(),
        )?)
    }
}

/// Shared comparison surface for the restricted-attention report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfRestrictedAttentionComparisonSurface {
    /// Stable reference back to the architecture queue.
    pub architecture_queue_report_ref: String,
    /// Stable reference to the committed challenge-format validation fixture.
    pub validation_fixture_ref: String,
    /// Stable digest over that fixture.
    pub validation_fixture_digest: String,
    /// Stable tokenizer digest surfaced by the fixture.
    pub tokenizer_model_digest: String,
    /// Challenge-format sequence length used by the report.
    pub sequence_length: usize,
    /// Number of evaluated sequences in the fixture.
    pub evaluated_sequence_count: usize,
    /// Dense baseline attention window.
    pub dense_attention_window_size: usize,
    /// Restricted-attention candidate window.
    pub restricted_attention_window_size: usize,
    /// Stable non-record package version reused for code-byte posture.
    pub baseline_submission_package_version: String,
    /// Stable counted code bytes reused by both rows.
    pub baseline_code_bytes: u64,
    /// Stable entrypoint artifact digest reused by both rows.
    pub baseline_entrypoint_artifact_digest: String,
    /// Stable accounting receipt digest reused by both rows.
    pub baseline_accounting_receipt_digest: String,
}

/// Metric tuple for one dense or restricted-attention row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfRestrictedAttentionMetrics {
    /// Stable variant identifier.
    pub variant_id: String,
    /// Causal attention window used while scoring.
    pub attention_window_size: usize,
    /// Mean validation loss on the committed challenge-format slice.
    pub val_loss: f64,
    /// Validation bits per byte on the committed challenge-format slice.
    pub val_bpb: f64,
    /// Reused counted code bytes.
    pub bytes_code: u64,
    /// Compressed-model bytes under the unchanged export path.
    pub bytes_model_int8_zlib: u64,
    /// Total counted bytes.
    pub bytes_total: u64,
    /// Delta versus the dense baseline row. Negative is better.
    pub delta_val_bpb_vs_dense: f64,
    /// Delta versus the dense baseline model bytes.
    pub delta_model_bytes_vs_dense: i64,
}

/// Runtime facts for one dense or restricted-attention row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfRestrictedAttentionRuntimeFacts {
    /// Honest runtime-measurement posture.
    pub measurement_posture: String,
    /// Analytic attention-score term count per full forward over one sequence.
    pub attention_score_terms_per_forward: u64,
    /// Relative attention-score term count versus the dense baseline.
    pub relative_attention_score_terms_vs_dense: f64,
    /// Honest note about what the runtime row does and does not claim.
    pub runtime_note: String,
}

/// Dense or restricted-attention row in the committed report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfRestrictedAttentionVariantReport {
    /// Stable issue reference tied to the row.
    pub issue_ref: String,
    /// Mechanism being tested.
    pub mechanism: String,
    /// Honest claim posture for the row.
    pub claim_posture: String,
    /// Honest boundary note for the row.
    pub boundary_note: String,
    /// Measured metrics for the row.
    pub metrics: ParameterGolfRestrictedAttentionMetrics,
    /// Runtime facts for the row.
    pub runtime_facts: ParameterGolfRestrictedAttentionRuntimeFacts,
}

/// Committed restricted-attention report on one real seq_len=1024 validation slice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfRestrictedAttentionReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Report-wide claim boundary.
    pub claim_boundary: String,
    /// Shared comparison surface.
    pub comparison_surface: ParameterGolfRestrictedAttentionComparisonSurface,
    /// Dense baseline row on the committed challenge-format slice.
    pub dense_baseline: ParameterGolfRestrictedAttentionVariantReport,
    /// Restricted-attention candidate row on the committed challenge-format slice.
    pub restricted_attention_candidate: ParameterGolfRestrictedAttentionVariantReport,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl ParameterGolfRestrictedAttentionReport {
    fn new(
        comparison_surface: ParameterGolfRestrictedAttentionComparisonSurface,
        dense_baseline: ParameterGolfRestrictedAttentionVariantReport,
        restricted_attention_candidate: ParameterGolfRestrictedAttentionVariantReport,
    ) -> Self {
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("parameter_golf.restricted_attention.report.v1"),
            claim_boundary: String::from(
                "this report measures one bounded restricted-attention candidate against the frozen dense baseline on one committed seq_len=1024 challenge-format validation slice. It keeps byte accounting, artifact bytes, and runtime deltas explicit, and it does not claim retraining closure, single-H100 closure, or record-track readiness.",
            ),
            comparison_surface,
            dense_baseline,
            restricted_attention_candidate,
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_parameter_golf_restricted_attention_report|", &report);
        report
    }
}

/// Failure while building or persisting the restricted-attention report.
#[derive(Debug, Error)]
pub enum ParameterGolfRestrictedAttentionReportError {
    #[error(transparent)]
    Benchmark(#[from] ParameterGolfBenchmarkBundleError),
    #[error(transparent)]
    Eval(#[from] ParameterGolfValidationEvalError),
    #[error(transparent)]
    WindowedEval(#[from] ParameterGolfWindowedValidationEvalError),
    #[error(transparent)]
    Submission(#[from] ParameterGolfSubmissionError),
    #[error(transparent)]
    Training(#[from] ParameterGolfReferenceTrainingError),
    #[error(transparent)]
    Data(#[from] psionic_data::ParameterGolfDataError),
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

/// Builds the committed restricted-attention report.
pub fn build_parameter_golf_restricted_attention_report(
) -> Result<ParameterGolfRestrictedAttentionReport, ParameterGolfRestrictedAttentionReportError> {
    let fixture = load_seq1024_validation_fixture()?;
    let byte_luts = fixture.byte_luts()?;
    let training_fixture = ParameterGolfLocalReferenceFixture::reference()?;
    let training_config = ParameterGolfReferenceTrainingConfig::local_reference();
    let benchmark_bundle =
        benchmark_parameter_golf_local_reference(&training_fixture, &training_config)?;
    let submission_bundle = build_parameter_golf_non_record_submission_bundle(
        &benchmark_bundle,
        &ParameterGolfNonRecordSubmissionConfig::local_reference_defaults(),
    )?;
    let baseline_model = &benchmark_bundle.training_outcome.trained_model;
    let model_artifact = export_parameter_golf_int8_zlib_model_artifact(
        baseline_model,
        "parameter-golf-restricted-attention-proxy",
        training_config.max_steps,
    )?;
    let bytes_model_int8_zlib = model_artifact.bytes.len() as u64;
    let baseline_code_bytes = submission_bundle.submission_manifest.bytes_code;
    let bytes_total = baseline_code_bytes.saturating_add(bytes_model_int8_zlib);
    let dense_eval = evaluate_parameter_golf_validation(
        baseline_model,
        fixture.validation_tokens.as_slice(),
        fixture.sequence_length,
        fixture.sequence_length,
        &byte_luts,
    )?;
    let restricted_eval = evaluate_parameter_golf_windowed_validation(
        baseline_model,
        fixture.validation_tokens.as_slice(),
        fixture.sequence_length,
        fixture.sequence_length,
        PARAMETER_GOLF_RESTRICTED_ATTENTION_WINDOW_SIZE,
        &byte_luts,
    )?;

    let comparison_surface = ParameterGolfRestrictedAttentionComparisonSurface {
        architecture_queue_report_ref: String::from(
            "fixtures/parameter_golf/reports/parameter_golf_architecture_experiment_queue_report.json",
        ),
        validation_fixture_ref: String::from(PARAMETER_GOLF_SEQ1024_VALIDATION_FIXTURE_REF),
        validation_fixture_digest: fixture.stable_digest(),
        tokenizer_model_digest: fixture.tokenizer_model_digest.clone(),
        sequence_length: fixture.sequence_length,
        evaluated_sequence_count: dense_eval.evaluated_sequence_count,
        dense_attention_window_size: fixture.sequence_length,
        restricted_attention_window_size: PARAMETER_GOLF_RESTRICTED_ATTENTION_WINDOW_SIZE,
        baseline_submission_package_version: String::from(
            PARAMETER_GOLF_NON_RECORD_SUBMISSION_VERSION,
        ),
        baseline_code_bytes,
        baseline_entrypoint_artifact_digest: submission_bundle
            .artifact("train_gpt.py")
            .expect("baseline entrypoint should exist")
            .artifact_digest
            .clone(),
        baseline_accounting_receipt_digest: submission_bundle
            .accounting_receipt
            .receipt_digest
            .clone(),
    };

    let dense_attention_terms = attention_score_terms_per_forward(
        baseline_model.descriptor().config.num_heads,
        fixture.sequence_length,
        fixture.sequence_length,
    );
    let restricted_attention_terms = attention_score_terms_per_forward(
        baseline_model.descriptor().config.num_heads,
        fixture.sequence_length,
        PARAMETER_GOLF_RESTRICTED_ATTENTION_WINDOW_SIZE,
    );
    let dense_baseline = ParameterGolfRestrictedAttentionVariantReport {
        issue_ref: String::from("PGOLF-613/#257"),
        mechanism: String::from(
            "Frozen dense causal attention on one committed seq_len=1024 challenge-format validation slice.",
        ),
        claim_posture: String::from("research_only_dense_slice_control"),
        boundary_note: String::from(
            "This row is still the frozen local-reference-trained dense control. It does not imply challenge-speed closure or retrained long-context optimality.",
        ),
        metrics: ParameterGolfRestrictedAttentionMetrics {
            variant_id: String::from("dense_slice_control"),
            attention_window_size: fixture.sequence_length,
            val_loss: round_report_f64(dense_eval.mean_loss),
            val_bpb: round_report_f64(dense_eval.bits_per_byte),
            bytes_code: baseline_code_bytes,
            bytes_model_int8_zlib,
            bytes_total,
            delta_val_bpb_vs_dense: 0.0,
            delta_model_bytes_vs_dense: 0,
        },
        runtime_facts: ParameterGolfRestrictedAttentionRuntimeFacts {
            measurement_posture: String::from("analytic_attention_terms_on_committed_slice"),
            attention_score_terms_per_forward: dense_attention_terms,
            relative_attention_score_terms_vs_dense: 1.0,
            runtime_note: String::from(
                "This row keeps the current dense runtime path as the challenge-format slice control.",
            ),
        },
    };
    let restricted_attention_candidate = ParameterGolfRestrictedAttentionVariantReport {
        issue_ref: String::from("PGOLF-613/#257"),
        mechanism: String::from(
            "One fixed local-attention proxy that limits every target token to the most recent 256 source positions while leaving weights and exported artifact bytes unchanged.",
        ),
        claim_posture: String::from("research_only_restricted_attention_proxy"),
        boundary_note: String::from(
            "This row is an eval-time restricted-attention proxy over the frozen dense baseline weights. It preserves negative evidence explicitly if the locality cut improves analytic attention cost while harming val_bpb.",
        ),
        metrics: ParameterGolfRestrictedAttentionMetrics {
            variant_id: String::from("restricted_attention_window_256"),
            attention_window_size: PARAMETER_GOLF_RESTRICTED_ATTENTION_WINDOW_SIZE,
            val_loss: round_report_f64(restricted_eval.mean_loss),
            val_bpb: round_report_f64(restricted_eval.bits_per_byte),
            bytes_code: baseline_code_bytes,
            bytes_model_int8_zlib,
            bytes_total,
            delta_val_bpb_vs_dense: round_report_f64(
                restricted_eval.bits_per_byte - dense_eval.bits_per_byte,
            ),
            delta_model_bytes_vs_dense: 0,
        },
        runtime_facts: ParameterGolfRestrictedAttentionRuntimeFacts {
            measurement_posture: String::from("analytic_attention_terms_on_committed_slice"),
            attention_score_terms_per_forward: restricted_attention_terms,
            relative_attention_score_terms_vs_dense: round_report_f64(
                restricted_attention_terms as f64 / dense_attention_terms as f64,
            ),
            runtime_note: String::from(
                "This runtime row is analytic, not a measured CUDA speed claim. It captures only the attention-score term reduction from the fixed 256-token causal window under the unchanged dense block stack.",
            ),
        },
    };

    Ok(ParameterGolfRestrictedAttentionReport::new(
        comparison_surface,
        dense_baseline,
        restricted_attention_candidate,
    ))
}

/// Returns the canonical absolute path for the committed restricted-attention report.
#[must_use]
pub fn parameter_golf_restricted_attention_report_path() -> PathBuf {
    repo_root().join(PARAMETER_GOLF_RESTRICTED_ATTENTION_REPORT_REF)
}

/// Writes the committed restricted-attention report.
pub fn write_parameter_golf_restricted_attention_report(
    output_path: impl AsRef<Path>,
) -> Result<ParameterGolfRestrictedAttentionReport, ParameterGolfRestrictedAttentionReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfRestrictedAttentionReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_parameter_golf_restricted_attention_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        ParameterGolfRestrictedAttentionReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn load_seq1024_validation_fixture(
) -> Result<ParameterGolfSeq1024ValidationFixture, ParameterGolfRestrictedAttentionReportError> {
    read_repo_json(PARAMETER_GOLF_SEQ1024_VALIDATION_FIXTURE_REF)
}

fn attention_score_terms_per_forward(num_heads: usize, sequence_length: usize, window: usize) -> u64 {
    let effective_window = window.min(sequence_length);
    let per_head = (0..sequence_length)
        .map(|position| position.saturating_add(1).min(effective_window) as u64)
        .sum::<u64>();
    per_head.saturating_mul(num_heads as u64)
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
) -> Result<T, ParameterGolfRestrictedAttentionReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| ParameterGolfRestrictedAttentionReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        ParameterGolfRestrictedAttentionReportError::Deserialize {
            artifact_kind: String::from("parameter_golf_restricted_attention_report"),
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(not(test))]
fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
) -> Result<T, ParameterGolfRestrictedAttentionReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| ParameterGolfRestrictedAttentionReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        ParameterGolfRestrictedAttentionReportError::Deserialize {
            artifact_kind: String::from("parameter_golf_restricted_attention_report"),
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

fn round_report_f64(value: f64) -> f64 {
    (value * 1_000_000_000_000_000.0).round() / 1_000_000_000_000_000.0
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use super::{
        build_parameter_golf_restricted_attention_report,
        parameter_golf_restricted_attention_report_path, read_repo_json,
        write_parameter_golf_restricted_attention_report,
        ParameterGolfRestrictedAttentionReport, PARAMETER_GOLF_RESTRICTED_ATTENTION_REPORT_REF,
    };

    fn built_report() -> &'static ParameterGolfRestrictedAttentionReport {
        static REPORT: OnceLock<ParameterGolfRestrictedAttentionReport> = OnceLock::new();
        REPORT.get_or_init(|| {
            build_parameter_golf_restricted_attention_report().expect("build report")
        })
    }

    #[test]
    fn parameter_golf_restricted_attention_report_keeps_byte_surface_constant_and_runtime_explicit()
    {
        let report = built_report();
        assert_eq!(
            report.dense_baseline.metrics.bytes_total,
            report.restricted_attention_candidate.metrics.bytes_total
        );
        assert!(
            report
                .restricted_attention_candidate
                .runtime_facts
                .relative_attention_score_terms_vs_dense
                < 1.0
        );
        assert_eq!(
            report.restricted_attention_candidate.metrics.attention_window_size,
            256
        );
    }

    #[test]
    fn parameter_golf_restricted_attention_report_matches_committed_truth() {
        let generated = built_report();
        let committed: ParameterGolfRestrictedAttentionReport =
            read_repo_json(PARAMETER_GOLF_RESTRICTED_ATTENTION_REPORT_REF)
                .expect("committed report");
        assert_eq!(*generated, committed);
    }

    #[test]
    fn write_parameter_golf_restricted_attention_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("parameter_golf_restricted_attention_report.json");
        let written =
            write_parameter_golf_restricted_attention_report(&output_path).expect("write report");
        let persisted: ParameterGolfRestrictedAttentionReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written.report_digest, persisted.report_digest);
        assert_eq!(
            parameter_golf_restricted_attention_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("parameter_golf_restricted_attention_report.json")
        );
    }
}
