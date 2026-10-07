use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    evaluate_parameter_golf_validation, ParameterGolfValidationEvalError,
    PARAMETER_GOLF_SUBMISSION_METRIC_ID,
};
use psionic_models::{
    ModelDescriptor, ParameterGolfAttentionWeights, ParameterGolfBlockWeights,
    ParameterGolfLinearWeights, ParameterGolfMlpWeights, ParameterGolfModelError,
    ParameterGolfReferenceModel,
};
use psionic_train::{
    benchmark_parameter_golf_local_reference, build_parameter_golf_non_record_submission_bundle,
    export_parameter_golf_int8_zlib_model_artifact, ParameterGolfBenchmarkBundleError,
    ParameterGolfLocalReferenceFixture, ParameterGolfNonRecordSubmissionConfig,
    ParameterGolfReferenceTrainingConfig, ParameterGolfReferenceTrainingError,
    ParameterGolfSubmissionError, PARAMETER_GOLF_NON_RECORD_SUBMISSION_VERSION,
    PARAMETER_GOLF_SUBMISSION_ARTIFACT_CAP_BYTES,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Canonical committed report for the first concrete Parameter Golf
/// post-parity architecture experiment queue.
pub const PARAMETER_GOLF_ARCHITECTURE_EXPERIMENT_QUEUE_REPORT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_architecture_experiment_queue_report.json";

/// Current status for one queue row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfArchitectureExperimentStatus {
    ImplementedResearchVariant,
    PlannedResearchCandidate,
}

/// Shared comparison surface reused by the first concrete architecture queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfArchitectureExperimentComparisonSurface {
    /// Stable harness report reused by this queue.
    pub harness_report_ref: String,
    /// Stable baseline run identifier.
    pub baseline_run_id: String,
    /// Stable training token digest.
    pub training_dataset_digest: String,
    /// Stable validation token digest.
    pub validation_dataset_digest: String,
    /// Canonical benchmark reference.
    pub benchmark_ref: String,
    /// Canonical submission metric identifier.
    pub submission_metric_id: String,
    /// Sequence length reused by the current bounded harness.
    pub sequence_length: usize,
    /// Public artifact cap in decimal bytes.
    pub artifact_cap_bytes: u64,
    /// Stable non-record package version that owns the counted-code posture.
    pub baseline_submission_package_version: String,
    /// Stable counted code bytes reused by the queue variants.
    pub baseline_code_bytes: u64,
    /// Stable entrypoint artifact digest reused by the queue variants.
    pub baseline_entrypoint_artifact_digest: String,
    /// Stable accounting receipt digest reused by the queue variants.
    pub baseline_accounting_receipt_digest: String,
}

/// Measured metric tuple for one implemented queue variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfArchitectureExperimentMetrics {
    /// Final validation mean loss on the bounded research harness.
    pub val_loss: f64,
    /// Final validation bits per byte on the bounded research harness.
    pub val_bpb: f64,
    /// Reused counted code bytes.
    pub bytes_code: u64,
    /// Measured compressed-model bytes for this variant.
    pub bytes_model_int8_zlib: u64,
    /// Total counted bytes under the unchanged non-record code surface.
    pub bytes_total: u64,
    /// Delta versus the frozen baseline row. Negative is better.
    pub delta_val_bpb_vs_baseline: f64,
    /// Delta versus the frozen baseline model artifact bytes. Negative is better.
    pub delta_model_bytes_vs_baseline: i64,
}

/// Explicit runtime facts for one implemented queue variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfArchitectureExperimentRuntimeFacts {
    /// Honest runtime-measurement posture.
    pub measurement_posture: String,
    /// Baseline logical training duration reused as the bounded harness budget.
    pub baseline_logical_training_ms: u64,
    /// Relative dense attention-score workload versus the baseline runtime path.
    pub relative_dense_attention_score_ops: f64,
    /// Relative count of unique decoder blocks retained by the variant.
    pub relative_unique_decoder_block_fraction: f64,
    /// Honest note about what this runtime row does and does not claim.
    pub runtime_note: String,
}

/// One implemented or queued row in the first concrete architecture queue.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfArchitectureExperimentVariantReport {
    /// Stable issue reference tied to the row.
    pub issue_ref: String,
    /// Stable variant identifier.
    pub variant_id: String,
    /// Current implementation status.
    pub status: ParameterGolfArchitectureExperimentStatus,
    /// Mechanism being tested.
    pub mechanism: String,
    /// Ordered changed surfaces.
    pub changed_surfaces: Vec<String>,
    /// Honest claim posture for the row.
    pub claim_posture: String,
    /// Honest boundary note for the row.
    pub boundary_note: String,
    /// Dedicated evidence reports that own row-specific measurement surfaces.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_report_refs: Vec<String>,
    /// Measured metrics when the row is implemented today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<ParameterGolfArchitectureExperimentMetrics>,
    /// Runtime facts when the row is implemented today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_facts: Option<ParameterGolfArchitectureExperimentRuntimeFacts>,
    /// Stable benchmark plan for queued rows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub benchmark_plan: Vec<String>,
}

/// Committed report for the first concrete post-parity architecture queue.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfArchitectureExperimentQueueReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Report-wide claim boundary.
    pub claim_boundary: String,
    /// Shared comparison surface.
    pub comparison_surface: ParameterGolfArchitectureExperimentComparisonSurface,
    /// Frozen baseline row.
    pub baseline_control: ParameterGolfArchitectureExperimentVariantReport,
    /// Ordered implemented or queued variants.
    pub variants: Vec<ParameterGolfArchitectureExperimentVariantReport>,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl ParameterGolfArchitectureExperimentQueueReport {
    fn new(
        comparison_surface: ParameterGolfArchitectureExperimentComparisonSurface,
        baseline_control: ParameterGolfArchitectureExperimentVariantReport,
        variants: Vec<ParameterGolfArchitectureExperimentVariantReport>,
    ) -> Self {
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("parameter_golf.architecture_experiment_queue.v1"),
            claim_boundary: String::from(
                "this report freezes the first concrete post-parity Parameter Golf architecture queue on the bounded local-reference harness. The implemented rows are post-train value-sharing probes over the frozen baseline control; they do not claim full retraining, challenge-speed improvement, single-H100 closure, or record-track readiness.",
            ),
            comparison_surface,
            baseline_control,
            variants,
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_parameter_golf_architecture_experiment_queue_report|",
            &report,
        );
        report
    }
}

/// Failure while building or persisting the architecture queue report.
#[derive(Debug, Error)]
pub enum ParameterGolfArchitectureExperimentQueueError {
    #[error(transparent)]
    Benchmark(#[from] ParameterGolfBenchmarkBundleError),
    #[error(transparent)]
    Eval(#[from] ParameterGolfValidationEvalError),
    #[error(transparent)]
    Submission(#[from] ParameterGolfSubmissionError),
    #[error(transparent)]
    Training(#[from] ParameterGolfReferenceTrainingError),
    #[error(transparent)]
    Model(#[from] ParameterGolfModelError),
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

/// Builds the first concrete post-parity architecture experiment queue report.
pub fn build_parameter_golf_architecture_experiment_queue_report() -> Result<
    ParameterGolfArchitectureExperimentQueueReport,
    ParameterGolfArchitectureExperimentQueueError,
> {
    let fixture = ParameterGolfLocalReferenceFixture::reference()?;
    let training_config = ParameterGolfReferenceTrainingConfig::local_reference();
    let benchmark_bundle = benchmark_parameter_golf_local_reference(&fixture, &training_config)?;
    let submission_bundle = build_parameter_golf_non_record_submission_bundle(
        &benchmark_bundle,
        &ParameterGolfNonRecordSubmissionConfig::local_reference_defaults(),
    )?;
    let baseline_code_bytes = submission_bundle.submission_manifest.bytes_code;
    let baseline_model_bytes = submission_bundle.submission_manifest.bytes_model_int8_zlib;
    let baseline_logical_training_ms = training_config
        .max_steps
        .saturating_mul(training_config.step_duration_ms);

    let comparison_surface = ParameterGolfArchitectureExperimentComparisonSurface {
        harness_report_ref: String::from(
            "fixtures/parameter_golf/reports/parameter_golf_research_harness_report.json",
        ),
        baseline_run_id: benchmark_bundle.run_bundle.run_id.clone(),
        training_dataset_digest: fixture.training_digest(),
        validation_dataset_digest: fixture.validation_digest(),
        benchmark_ref: benchmark_bundle.benchmark_receipt.benchmark_ref.clone(),
        submission_metric_id: String::from(PARAMETER_GOLF_SUBMISSION_METRIC_ID),
        sequence_length: training_config.geometry.train_sequence_length,
        artifact_cap_bytes: PARAMETER_GOLF_SUBMISSION_ARTIFACT_CAP_BYTES,
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

    let baseline_control = ParameterGolfArchitectureExperimentVariantReport {
        issue_ref: String::from("PGOLF-402/#173"),
        variant_id: String::from("baseline_control"),
        status: ParameterGolfArchitectureExperimentStatus::ImplementedResearchVariant,
        mechanism: String::from(
            "Frozen local-reference baseline control with the current dense decoder, current Muon-plus-Adam split, and current int8-plus-zlib export.",
        ),
        changed_surfaces: vec![
            String::from("public_9x512_decoder"),
            String::from("current_muon_plus_adam_optimizer_split"),
            String::from("current_int8_zlib_export"),
        ],
        claim_posture: String::from("research_only_baseline_control"),
        boundary_note: String::from(
            "This row remains the bounded local-reference control only. It does not imply single-H100 or 8xH100 challenge closure.",
        ),
        metrics: Some(ParameterGolfArchitectureExperimentMetrics {
            val_loss: submission_bundle.submission_manifest.val_loss,
            val_bpb: submission_bundle.submission_manifest.val_bpb,
            bytes_code: baseline_code_bytes,
            bytes_model_int8_zlib: baseline_model_bytes,
            bytes_total: submission_bundle.submission_manifest.bytes_total,
            delta_val_bpb_vs_baseline: 0.0,
            delta_model_bytes_vs_baseline: 0,
        }),
        runtime_facts: Some(ParameterGolfArchitectureExperimentRuntimeFacts {
            measurement_posture: String::from("bounded_local_reference_control"),
            baseline_logical_training_ms,
            relative_dense_attention_score_ops: 1.0,
            relative_unique_decoder_block_fraction: 1.0,
            runtime_note: String::from(
                "This row is the frozen dense-runtime control for the current bounded harness.",
            ),
        }),
        evidence_report_refs: Vec::new(),
        benchmark_plan: Vec::new(),
    };

    let baseline_model = &benchmark_bundle.training_outcome.trained_model;
    let shared_depth_model = shared_depth_decoder_value_tying_proxy(baseline_model)?;
    let shared_depth_metrics = measure_variant_metrics(
        &shared_depth_model,
        &fixture,
        &training_config,
        baseline_code_bytes,
        baseline_control
            .metrics
            .as_ref()
            .expect("baseline metrics should exist"),
        "parameter-golf-shared-depth-decoder-value-tying-proxy",
    )?;
    let mirrored_tying_model = mirrored_block_pair_tying_variant(baseline_model)?;
    let mirrored_tying_metrics = measure_variant_metrics(
        &mirrored_tying_model,
        &fixture,
        &training_config,
        baseline_code_bytes,
        baseline_control
            .metrics
            .as_ref()
            .expect("baseline metrics should exist"),
        "parameter-golf-mirrored-block-pair-tying-proxy",
    )?;

    let decoder_count = baseline_model.descriptor().config.num_decoder_layers();
    let variants = vec![
        ParameterGolfArchitectureExperimentVariantReport {
            issue_ref: String::from("PGOLF-611/#255"),
            variant_id: String::from("shared_depth_decoder_value_tying_proxy"),
            status: ParameterGolfArchitectureExperimentStatus::ImplementedResearchVariant,
            mechanism: String::from(
                "Reuse one canonical decoder block by value across every decoder-depth position while leaving the current dense runtime path unchanged.",
            ),
            changed_surfaces: vec![
                String::from("decoder_block_value_reuse"),
                String::from("post_train_decoder_half_weight_aliasing_proxy"),
                String::from("unchanged_dense_forward_runtime"),
            ],
            claim_posture: String::from("research_only_shared_depth_proxy"),
            boundary_note: String::from(
                "This is a post-train shared-depth proxy, not a retrained recurrent runtime. It measures whether decoder-depth value reuse helps the shipped frontier under the same oracle and accounting surface.",
            ),
            evidence_report_refs: Vec::new(),
            metrics: Some(shared_depth_metrics),
            runtime_facts: Some(ParameterGolfArchitectureExperimentRuntimeFacts {
                measurement_posture: String::from("post_train_value_tying_proxy"),
                baseline_logical_training_ms,
                relative_dense_attention_score_ops: 1.0,
                relative_unique_decoder_block_fraction: 1.0 / decoder_count as f64,
                runtime_note: String::from(
                    "All decoder-depth executions still run, but only one decoder block remains unique by value in this proxy row.",
                ),
            }),
            benchmark_plan: Vec::new(),
        },
        ParameterGolfArchitectureExperimentVariantReport {
            issue_ref: String::from("PGOLF-612/#256"),
            variant_id: String::from("mirrored_block_pair_tying_proxy"),
            status: ParameterGolfArchitectureExperimentStatus::ImplementedResearchVariant,
            mechanism: String::from(
                "Tie mirrored encoder-decoder block pairs by value so each pair shares one averaged block while keeping the current dense runtime path and exported-folder code surface unchanged.",
            ),
            changed_surfaces: vec![
                String::from("mirrored_block_pair_weight_tying"),
                String::from("cross_half_parameter_reuse"),
                String::from("unchanged_dense_forward_runtime"),
            ],
            claim_posture: String::from("research_only_parameter_tying_proxy"),
            boundary_note: String::from(
                "This row measures stronger block-level parameter tying on the frozen baseline family. It does not claim that the same tying scheme would remain optimal after full retraining.",
            ),
            evidence_report_refs: Vec::new(),
            metrics: Some(mirrored_tying_metrics),
            runtime_facts: Some(ParameterGolfArchitectureExperimentRuntimeFacts {
                measurement_posture: String::from("post_train_value_tying_proxy"),
                baseline_logical_training_ms,
                relative_dense_attention_score_ops: 1.0,
                relative_unique_decoder_block_fraction: (decoder_count.saturating_sub(1) as f64)
                    / decoder_count as f64,
                runtime_note: String::from(
                    "The dense runtime path stays unchanged; the only runtime-facing difference here is reduced unique block-state entropy in the exported artifact.",
                ),
            }),
            benchmark_plan: Vec::new(),
        },
        ParameterGolfArchitectureExperimentVariantReport {
            issue_ref: String::from("PGOLF-613/#257"),
            variant_id: String::from("restricted_attention_window_candidate"),
            status: ParameterGolfArchitectureExperimentStatus::ImplementedResearchVariant,
            mechanism: String::from(
                "Evaluate one fixed local-attention or restricted-attention window on the frozen baseline family under the same metric and accounting surface.",
            ),
            changed_surfaces: vec![
                String::from("causal_attention_window_mask"),
                String::from("sequence_length_1024_eval_slice"),
                String::from("unchanged_exported_artifact_surface"),
            ],
            claim_posture: String::from("implemented_restricted_attention_proxy"),
            boundary_note: String::from(
                "This row is now implemented via the dedicated restricted-attention report on one committed seq_len=1024 challenge-format validation slice. The evidence remains research-only and does not claim retraining closure, single-H100 closure, or record-track readiness.",
            ),
            evidence_report_refs: vec![String::from(
                "fixtures/parameter_golf/reports/parameter_golf_restricted_attention_report.json",
            )],
            metrics: None,
            runtime_facts: None,
            benchmark_plan: Vec::new(),
        },
    ];

    Ok(ParameterGolfArchitectureExperimentQueueReport::new(
        comparison_surface,
        baseline_control,
        variants,
    ))
}

/// Returns the canonical absolute path for the committed architecture queue report.
#[must_use]
pub fn parameter_golf_architecture_experiment_queue_report_path() -> PathBuf {
    repo_root().join(PARAMETER_GOLF_ARCHITECTURE_EXPERIMENT_QUEUE_REPORT_REF)
}

/// Writes the committed architecture queue report.
pub fn write_parameter_golf_architecture_experiment_queue_report(
    output_path: impl AsRef<Path>,
) -> Result<
    ParameterGolfArchitectureExperimentQueueReport,
    ParameterGolfArchitectureExperimentQueueError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfArchitectureExperimentQueueError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_parameter_golf_architecture_experiment_queue_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        ParameterGolfArchitectureExperimentQueueError::Write {
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

fn measure_variant_metrics(
    model: &ParameterGolfReferenceModel,
    fixture: &ParameterGolfLocalReferenceFixture,
    training_config: &ParameterGolfReferenceTrainingConfig,
    baseline_code_bytes: u64,
    baseline_metrics: &ParameterGolfArchitectureExperimentMetrics,
    run_id: &str,
) -> Result<ParameterGolfArchitectureExperimentMetrics, ParameterGolfArchitectureExperimentQueueError>
{
    let eval = evaluate_parameter_golf_validation(
        model,
        fixture.validation_tokens.as_slice(),
        training_config.geometry.train_sequence_length,
        training_config.geometry.local_validation_batch_tokens(),
        &fixture.byte_luts()?,
    )?;
    let artifact =
        export_parameter_golf_int8_zlib_model_artifact(model, run_id, training_config.max_steps)?;
    let bytes_model_int8_zlib = artifact.bytes.len() as u64;
    let bytes_total = baseline_code_bytes.saturating_add(bytes_model_int8_zlib);
    Ok(ParameterGolfArchitectureExperimentMetrics {
        val_loss: eval.mean_loss,
        val_bpb: eval.bits_per_byte,
        bytes_code: baseline_code_bytes,
        bytes_model_int8_zlib,
        bytes_total,
        delta_val_bpb_vs_baseline: eval.bits_per_byte - baseline_metrics.val_bpb,
        delta_model_bytes_vs_baseline: bytes_model_int8_zlib as i64
            - baseline_metrics.bytes_model_int8_zlib as i64,
    })
}

fn shared_depth_decoder_value_tying_proxy(
    baseline_model: &ParameterGolfReferenceModel,
) -> Result<ParameterGolfReferenceModel, ParameterGolfArchitectureExperimentQueueError> {
    let config = baseline_model.descriptor().config.clone();
    let mut weights = baseline_model.weights().clone();
    let decoder_start = config.num_encoder_layers();
    let canonical_decoder_block = weights.blocks[decoder_start].clone();
    for block in weights.blocks.iter_mut().skip(decoder_start) {
        *block = canonical_decoder_block.clone();
    }
    Ok(ParameterGolfReferenceModel::new(
        ModelDescriptor::new(
            "parameter-golf-shared-depth-decoder-value-tying-proxy",
            baseline_model.descriptor().model.family.as_str(),
            "research-2026-03-19",
        ),
        config,
        weights,
    )?)
}

fn mirrored_block_pair_tying_variant(
    baseline_model: &ParameterGolfReferenceModel,
) -> Result<ParameterGolfReferenceModel, ParameterGolfArchitectureExperimentQueueError> {
    let config = baseline_model.descriptor().config.clone();
    let mut weights = baseline_model.weights().clone();
    let total_layers = weights.blocks.len();
    for left_index in 0..config.num_encoder_layers() {
        let right_index = total_layers - 1 - left_index;
        let tied = average_block_pair(&weights.blocks[left_index], &weights.blocks[right_index]);
        weights.blocks[left_index] = tied.clone();
        weights.blocks[right_index] = tied;
    }
    Ok(ParameterGolfReferenceModel::new(
        ModelDescriptor::new(
            "parameter-golf-mirrored-block-pair-tying-proxy",
            baseline_model.descriptor().model.family.as_str(),
            "research-2026-03-19",
        ),
        config,
        weights,
    )?)
}

fn average_block_pair(
    left: &ParameterGolfBlockWeights,
    right: &ParameterGolfBlockWeights,
) -> ParameterGolfBlockWeights {
    ParameterGolfBlockWeights {
        attention: ParameterGolfAttentionWeights {
            q_proj: average_linear(&left.attention.q_proj, &right.attention.q_proj),
            k_proj: average_linear(&left.attention.k_proj, &right.attention.k_proj),
            v_proj: average_linear(&left.attention.v_proj, &right.attention.v_proj),
            out_proj: average_linear(&left.attention.out_proj, &right.attention.out_proj),
            q_gain: average_values(
                left.attention.q_gain.as_slice(),
                right.attention.q_gain.as_slice(),
            ),
        },
        mlp: ParameterGolfMlpWeights {
            fc: average_linear(&left.mlp.fc, &right.mlp.fc),
            proj: average_linear(&left.mlp.proj, &right.mlp.proj),
        },
        attn_scale: average_values(left.attn_scale.as_slice(), right.attn_scale.as_slice()),
        mlp_scale: average_values(left.mlp_scale.as_slice(), right.mlp_scale.as_slice()),
        resid_mix: average_values(left.resid_mix.as_slice(), right.resid_mix.as_slice()),
    }
}

fn average_linear(
    left: &ParameterGolfLinearWeights,
    right: &ParameterGolfLinearWeights,
) -> ParameterGolfLinearWeights {
    ParameterGolfLinearWeights {
        out_features: left.out_features,
        in_features: left.in_features,
        weight: average_values(left.weight.as_slice(), right.weight.as_slice()),
    }
}

fn average_values(left: &[f32], right: &[f32]) -> Vec<f32> {
    left.iter()
        .zip(right.iter())
        .map(|(left_value, right_value)| (left_value + right_value) * 0.5)
        .collect()
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, ParameterGolfArchitectureExperimentQueueError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| ParameterGolfArchitectureExperimentQueueError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        ParameterGolfArchitectureExperimentQueueError::Deserialize {
            artifact_kind: String::from("parameter_golf_architecture_experiment_queue_report"),
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
        build_parameter_golf_architecture_experiment_queue_report,
        parameter_golf_architecture_experiment_queue_report_path, read_repo_json,
        write_parameter_golf_architecture_experiment_queue_report,
        ParameterGolfArchitectureExperimentQueueReport,
        PARAMETER_GOLF_ARCHITECTURE_EXPERIMENT_QUEUE_REPORT_REF,
    };

    #[test]
    fn parameter_golf_architecture_experiment_queue_has_two_measured_rows_and_one_open_queue_row() {
        let report =
            build_parameter_golf_architecture_experiment_queue_report().expect("build report");

        assert_eq!(report.variants.len(), 3);
        assert_eq!(
            report
                .variants
                .iter()
                .filter(|variant| variant.metrics.is_some())
                .count(),
            2
        );
        assert!(report.variants.iter().any(|variant| {
            variant.variant_id == "restricted_attention_window_candidate"
                && variant.metrics.is_none()
                && variant
                    .evidence_report_refs
                    .iter()
                    .any(|reference| reference.ends_with("parameter_golf_restricted_attention_report.json"))
        }));
    }

    #[test]
    fn parameter_golf_architecture_experiment_queue_report_matches_committed_truth() {
        let generated =
            build_parameter_golf_architecture_experiment_queue_report().expect("build report");
        let committed: ParameterGolfArchitectureExperimentQueueReport =
            read_repo_json(PARAMETER_GOLF_ARCHITECTURE_EXPERIMENT_QUEUE_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_parameter_golf_architecture_experiment_queue_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("parameter_golf_architecture_experiment_queue_report.json");
        let written = write_parameter_golf_architecture_experiment_queue_report(&output_path)
            .expect("write report");
        let persisted: ParameterGolfArchitectureExperimentQueueReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            parameter_golf_architecture_experiment_queue_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("parameter_golf_architecture_experiment_queue_report.json")
        );
    }
}
