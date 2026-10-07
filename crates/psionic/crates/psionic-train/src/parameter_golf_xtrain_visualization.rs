use std::{collections::BTreeMap, fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_remote_training_run_index_v2, build_remote_training_visualization_bundle_v2,
    RemoteTrainingArtifactSourceKind, RemoteTrainingComparabilityClassV2,
    RemoteTrainingEmissionMode, RemoteTrainingEventSample, RemoteTrainingEventSeverity,
    RemoteTrainingExecutionClassV2, RemoteTrainingLossSample, RemoteTrainingMathSample,
    RemoteTrainingPrimaryScoreV2, RemoteTrainingPromotionGatePostureV2, RemoteTrainingProvider,
    RemoteTrainingPublicEquivalenceClassV2, RemoteTrainingRefreshContract,
    RemoteTrainingResultClassification, RemoteTrainingRunIndexEntryV2, RemoteTrainingRunIndexV2,
    RemoteTrainingScoreCloseoutPostureV2, RemoteTrainingScoreDeltaV2,
    RemoteTrainingScoreDirectionV2, RemoteTrainingScoreSurfaceV2, RemoteTrainingSeriesStatus,
    RemoteTrainingSourceArtifact, RemoteTrainingTimelineEntry, RemoteTrainingTrackFamilyV2,
    RemoteTrainingTrackSemanticsV2, RemoteTrainingVisualizationBundleV2,
    RemoteTrainingVisualizationError, RemoteTrainingVisualizationSummary,
};

pub const PARAMETER_GOLF_XTRAIN_QUICK_EVAL_REPORT_FIXTURE_PATH: &str =
    "fixtures/parameter_golf/reports/parameter_golf_xtrain_quick_eval_report.json";
pub const PARAMETER_GOLF_XTRAIN_VISUALIZATION_BUNDLE_V2_FIXTURE_PATH: &str =
    "fixtures/training_visualization/parameter_golf_xtrain_remote_training_visualization_bundle_v2.json";

const XTRAIN_QUICK_EVAL_REPORT_SCHEMA_VERSION: &str =
    "psionic.parameter_golf_xtrain_quick_eval_report.v1";
const XTRAIN_REPORT_ID: &str = "parameter-golf-xtrain-quick-eval-window1-retained";
const XTRAIN_BUNDLE_ID: &str = "parameter-golf-xtrain-bounded-train-infer-v2";
const XTRAIN_PROFILE_ID: &str = "psion_small_decoder_pgolf_core_v0";
const XTRAIN_LANE_ID: &str = "parameter_golf.promoted_general_xtrain_quick_eval";
const XTRAIN_RUN_ID: &str = "parameter-golf-promoted-general-xtrain-baseline";
const XTRAIN_REPO_REVISION: &str = "fixtures@parameter_golf_xtrain_quick_eval_report.v1";
const XTRAIN_TRACK_ID: &str = "parameter_golf.promoted_general_xtrain.quick_eval_window1.v1";
const XTRAIN_TRACK_DOC_REF: &str = "docs/PARAMETER_GOLF_XTRAIN_TRACK.md";
const XTRAIN_AUDIT_REF: &str =
    "docs/audits/2026-03-27-xtrain-pgolf-fastfd-window1-quick-eval-audit.md";
const XTRAIN_SCORE_METRIC_ID: &str = "parameter_golf.validation_bits_per_byte";
const XTRAIN_SCORE_UNIT: &str = "bits_per_byte";
const XTRAIN_PROMPT_TEXT: &str = "abcd";
const XTRAIN_RETAINED_STARTED_AT_MS: u64 = 1_774_607_044_000;
const XTRAIN_RETAINED_SCORE_AT_MS: u64 = 1_774_607_104_000;
const XTRAIN_RETAINED_PARITY_AT_MS: u64 = 1_774_607_106_000;
const XTRAIN_MAX_STEPS: u64 = 16;
const XTRAIN_STEP_DURATION_MS: u64 = 75;
const XTRAIN_BOUNDED_ATTENTION_WINDOW_TOKENS: usize = 1;
const XTRAIN_THROUGHPUT_REPETITIONS: usize = 2;
const XTRAIN_GENERATED_TOKENS_PER_RUNTIME_EVAL: usize = 16;
const XTRAIN_PROOF_VALIDATION_MEAN_LOSS: f64 = 8.605_988_740_921_02;
const XTRAIN_PROOF_VALIDATION_BITS_PER_BYTE: f64 = 9.932_653_822_778_41;
const XTRAIN_VALIDATION_MEAN_LOSS: f64 = 3.641_620_635_986_328;
const XTRAIN_VALIDATION_BITS_PER_BYTE: f64 = 4.202_998_425_869_111;
const XTRAIN_DIRECT_RUNTIME_TOKENS_PER_SECOND: f64 = 100.623_999_608_773_89;
const XTRAIN_SERVED_RUNTIME_TOKENS_PER_SECOND: f64 = 100.217_254_722_935_44;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfXtrainQuickEvalReport {
    pub schema_version: String,
    pub report_id: String,
    pub profile_id: String,
    pub xtrain_run_id: String,
    pub prompt_text: String,
    pub prompt_tokens: Vec<u32>,
    pub expected_tokens: Vec<u32>,
    pub proof_validation_mean_loss: f64,
    pub proof_validation_bits_per_byte: f64,
    pub xtrain_validation_mean_loss: f64,
    pub xtrain_validation_bits_per_byte: f64,
    pub xtrain_generated_tokens: Vec<u32>,
    pub exact_prefix_match_tokens: usize,
    pub exact_cycle_match: bool,
    pub bounded_attention_window_tokens: usize,
    pub max_steps: u64,
    pub step_duration_ms: u64,
    pub throughput_repetitions: usize,
    pub generated_tokens_per_runtime_eval: usize,
    pub direct_runtime_tokens_per_second: f64,
    pub served_runtime_tokens_per_second: f64,
    pub direct_served_match: bool,
    pub source_audit_ref: String,
    pub detail: String,
    pub report_digest: String,
}

impl ParameterGolfXtrainQuickEvalReport {
    pub fn validate(&self) -> Result<(), ParameterGolfXtrainVisualizationError> {
        if self.schema_version != XTRAIN_QUICK_EVAL_REPORT_SCHEMA_VERSION {
            return Err(ParameterGolfXtrainVisualizationError::InvalidReport(
                "schema_version",
                format!(
                    "expected `{XTRAIN_QUICK_EVAL_REPORT_SCHEMA_VERSION}`, got `{}`",
                    self.schema_version
                ),
            ));
        }
        ensure_nonempty(self.report_id.as_str(), "report_id")?;
        ensure_nonempty(self.profile_id.as_str(), "profile_id")?;
        ensure_nonempty(self.xtrain_run_id.as_str(), "xtrain_run_id")?;
        ensure_nonempty(self.prompt_text.as_str(), "prompt_text")?;
        ensure_nonempty(self.source_audit_ref.as_str(), "source_audit_ref")?;
        ensure_nonempty(self.detail.as_str(), "detail")?;
        ensure_nonempty(self.report_digest.as_str(), "report_digest")?;
        ensure_tokens(self.prompt_tokens.as_slice(), "prompt_tokens")?;
        ensure_tokens(self.expected_tokens.as_slice(), "expected_tokens")?;
        ensure_tokens(
            self.xtrain_generated_tokens.as_slice(),
            "xtrain_generated_tokens",
        )?;
        ensure_finite(
            self.proof_validation_mean_loss,
            "proof_validation_mean_loss",
        )?;
        ensure_finite(
            self.proof_validation_bits_per_byte,
            "proof_validation_bits_per_byte",
        )?;
        ensure_finite(
            self.xtrain_validation_mean_loss,
            "xtrain_validation_mean_loss",
        )?;
        ensure_finite(
            self.xtrain_validation_bits_per_byte,
            "xtrain_validation_bits_per_byte",
        )?;
        ensure_finite(
            self.direct_runtime_tokens_per_second,
            "direct_runtime_tokens_per_second",
        )?;
        ensure_finite(
            self.served_runtime_tokens_per_second,
            "served_runtime_tokens_per_second",
        )?;
        if self.bounded_attention_window_tokens == 0 {
            return Err(ParameterGolfXtrainVisualizationError::InvalidReport(
                "bounded_attention_window_tokens",
                String::from("must stay positive"),
            ));
        }
        if self.max_steps == 0 {
            return Err(ParameterGolfXtrainVisualizationError::InvalidReport(
                "max_steps",
                String::from("must stay positive"),
            ));
        }
        if self.step_duration_ms == 0 {
            return Err(ParameterGolfXtrainVisualizationError::InvalidReport(
                "step_duration_ms",
                String::from("must stay positive"),
            ));
        }
        if self.throughput_repetitions == 0 {
            return Err(ParameterGolfXtrainVisualizationError::InvalidReport(
                "throughput_repetitions",
                String::from("must stay positive"),
            ));
        }
        if self.generated_tokens_per_runtime_eval == 0 {
            return Err(ParameterGolfXtrainVisualizationError::InvalidReport(
                "generated_tokens_per_runtime_eval",
                String::from("must stay positive"),
            ));
        }
        if self.report_digest != stable_xtrain_report_digest(self) {
            return Err(ParameterGolfXtrainVisualizationError::InvalidReport(
                "report_digest",
                String::from("report_digest must match the stable retained digest"),
            ));
        }
        Ok(())
    }

    pub fn validation_loss_improvement(&self) -> f64 {
        self.proof_validation_mean_loss - self.xtrain_validation_mean_loss
    }

    pub fn bits_per_byte_improvement(&self) -> f64 {
        self.proof_validation_bits_per_byte - self.xtrain_validation_bits_per_byte
    }

    pub fn bits_per_byte_delta_vs_proof(&self) -> f64 {
        self.xtrain_validation_bits_per_byte - self.proof_validation_bits_per_byte
    }

    pub fn direct_runtime_tokens_per_second_rounded(&self) -> u64 {
        self.direct_runtime_tokens_per_second.round() as u64
    }

    pub fn direct_runtime_evaluation_ms(&self) -> u64 {
        runtime_eval_ms(
            self.generated_tokens_per_runtime_eval,
            self.throughput_repetitions,
            self.direct_runtime_tokens_per_second,
        )
    }

    pub fn served_runtime_evaluation_ms(&self) -> u64 {
        runtime_eval_ms(
            self.generated_tokens_per_runtime_eval,
            self.throughput_repetitions,
            self.served_runtime_tokens_per_second,
        )
    }
}

#[derive(Debug, Error)]
pub enum ParameterGolfXtrainVisualizationError {
    #[error("invalid retained XTRAIN report field `{0}`: {1}")]
    InvalidReport(&'static str, String),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Visualization(#[from] RemoteTrainingVisualizationError),
}

pub fn build_parameter_golf_xtrain_quick_eval_report(
) -> Result<ParameterGolfXtrainQuickEvalReport, ParameterGolfXtrainVisualizationError> {
    let mut report = ParameterGolfXtrainQuickEvalReport {
        schema_version: String::from(XTRAIN_QUICK_EVAL_REPORT_SCHEMA_VERSION),
        report_id: String::from(XTRAIN_REPORT_ID),
        profile_id: String::from(XTRAIN_PROFILE_ID),
        xtrain_run_id: String::from(XTRAIN_RUN_ID),
        prompt_text: String::from(XTRAIN_PROMPT_TEXT),
        prompt_tokens: vec![1, 2, 3, 4],
        expected_tokens: vec![5, 6, 7, 8, 1, 2, 3, 4],
        proof_validation_mean_loss: XTRAIN_PROOF_VALIDATION_MEAN_LOSS,
        proof_validation_bits_per_byte: XTRAIN_PROOF_VALIDATION_BITS_PER_BYTE,
        xtrain_validation_mean_loss: XTRAIN_VALIDATION_MEAN_LOSS,
        xtrain_validation_bits_per_byte: XTRAIN_VALIDATION_BITS_PER_BYTE,
        xtrain_generated_tokens: vec![6, 8, 6, 8, 6, 8, 6, 8],
        exact_prefix_match_tokens: 0,
        exact_cycle_match: false,
        bounded_attention_window_tokens: XTRAIN_BOUNDED_ATTENTION_WINDOW_TOKENS,
        max_steps: XTRAIN_MAX_STEPS,
        step_duration_ms: XTRAIN_STEP_DURATION_MS,
        throughput_repetitions: XTRAIN_THROUGHPUT_REPETITIONS,
        generated_tokens_per_runtime_eval: XTRAIN_GENERATED_TOKENS_PER_RUNTIME_EVAL,
        direct_runtime_tokens_per_second: XTRAIN_DIRECT_RUNTIME_TOKENS_PER_SECOND,
        served_runtime_tokens_per_second: XTRAIN_SERVED_RUNTIME_TOKENS_PER_SECOND,
        direct_served_match: true,
        source_audit_ref: String::from(XTRAIN_AUDIT_REF),
        detail: String::from(
            "The retained bounded XTRAIN quick-eval report records the current PGOLF-shaped train-to-infer lane: a closed-out local-reference BPB, one-token bounded decode posture, direct-versus-served runtime parity, and an explicit non-public promotion hold.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_xtrain_report_digest(&report);
    report.validate()?;
    Ok(report)
}

pub fn build_parameter_golf_xtrain_visualization_bundle_v2(
) -> Result<RemoteTrainingVisualizationBundleV2, ParameterGolfXtrainVisualizationError> {
    let report = build_parameter_golf_xtrain_quick_eval_report()?;
    Ok(build_remote_training_visualization_bundle_v2(
        RemoteTrainingVisualizationBundleV2 {
            schema_version: String::new(),
            bundle_id: String::from(XTRAIN_BUNDLE_ID),
            provider: RemoteTrainingProvider::LocalHybrid,
            profile_id: String::from(XTRAIN_PROFILE_ID),
            lane_id: String::from(XTRAIN_LANE_ID),
            run_id: report.xtrain_run_id.clone(),
            repo_revision: String::from(XTRAIN_REPO_REVISION),
            track_semantics: RemoteTrainingTrackSemanticsV2 {
                track_family: RemoteTrainingTrackFamilyV2::Xtrain,
                track_id: String::from(XTRAIN_TRACK_ID),
                execution_class: RemoteTrainingExecutionClassV2::BoundedTrainToInfer,
                comparability_class: RemoteTrainingComparabilityClassV2::SameTrackComparable,
                proof_posture: crate::RemoteTrainingProofPostureV2::BoundedTrainToInfer,
                public_equivalence_class:
                    RemoteTrainingPublicEquivalenceClassV2::NotPublicEquivalent,
                score_law_ref: Some(String::from(XTRAIN_TRACK_DOC_REF)),
                artifact_cap_bytes: None,
                wallclock_cap_seconds: None,
                semantic_summary: String::from(
                    "Bounded XTRAIN records a local-reference train-to-infer score lane for the promoted PGOLF family. It is comparable only inside the same XTRAIN track and does not claim public or HOMEGOLF-equivalent score posture.",
                ),
            },
            primary_score: Some(RemoteTrainingPrimaryScoreV2 {
                score_metric_id: String::from(XTRAIN_SCORE_METRIC_ID),
                score_direction: RemoteTrainingScoreDirectionV2::LowerIsBetter,
                score_unit: String::from(XTRAIN_SCORE_UNIT),
                score_value: report.xtrain_validation_bits_per_byte,
                score_value_observed_at_ms: XTRAIN_RETAINED_SCORE_AT_MS,
                score_summary: String::from(
                    "The retained bounded XTRAIN quick-eval lane closes one local-reference validation BPB for the promoted PGOLF family under the current one-token attention-window posture.",
                ),
            }),
            score_surface: Some(RemoteTrainingScoreSurfaceV2 {
                score_closeout_posture: RemoteTrainingScoreCloseoutPostureV2::ScoreClosedOut,
                promotion_gate_posture: RemoteTrainingPromotionGatePostureV2::Held,
                delta_rows: vec![RemoteTrainingScoreDeltaV2 {
                    reference_id: String::from("promoted_proof_baseline"),
                    score_metric_id: String::from(XTRAIN_SCORE_METRIC_ID),
                    reference_score_value: report.proof_validation_bits_per_byte,
                    delta_value: report.bits_per_byte_delta_vs_proof(),
                    delta_summary: String::from(
                        "The bounded XTRAIN lane improves BPB versus the promoted proof baseline, but the result remains non-public and does not yet upgrade into HOMEGOLF-critical score posture.",
                    ),
                }],
                semantic_summary: String::from(
                    "Bounded XTRAIN closes a private local-reference score, preserves its delta versus the proof baseline, and keeps promotion held because the lane is not public-equivalent and does not yet prove HOMEGOLF score relevance.",
                ),
            }),
            result_classification: RemoteTrainingResultClassification::CompletedSuccess,
            refresh_contract: RemoteTrainingRefreshContract {
                target_ui_update_interval_ms: crate::REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
                emission_mode: RemoteTrainingEmissionMode::PostRunOnly,
                last_heartbeat_at_ms: None,
                heartbeat_seq: 0,
            },
            series_status: RemoteTrainingSeriesStatus::Partial,
            series_unavailable_reason: Some(String::from(
                "The retained bounded XTRAIN lane closes score and runtime parity but does not yet publish one append-only live step series in the shared visualization family.",
            )),
            timeline: vec![
                RemoteTrainingTimelineEntry {
                    observed_at_ms: XTRAIN_RETAINED_STARTED_AT_MS,
                    phase: String::from("training"),
                    subphase: Some(String::from("bounded_local_reference_xtrain")),
                    detail: String::from(
                        "The promoted PGOLF local-reference trainer ran the bounded XTRAIN coordinate budget and preserved one non-public train-to-infer lane.",
                    ),
                },
                RemoteTrainingTimelineEntry {
                    observed_at_ms: XTRAIN_RETAINED_SCORE_AT_MS,
                    phase: String::from("score_closeout"),
                    subphase: Some(String::from("quick_eval_validation")),
                    detail: String::from(
                        "The quick-eval lane sealed one retained validation BPB for the bounded XTRAIN family under the one-token inference window.",
                    ),
                },
                RemoteTrainingTimelineEntry {
                    observed_at_ms: XTRAIN_RETAINED_PARITY_AT_MS,
                    phase: String::from("parity"),
                    subphase: Some(String::from("direct_vs_served_runtime")),
                    detail: String::from(
                        "The same retained XTRAIN bundle was decoded through direct runtime and psionic-serve with exact token parity.",
                    ),
                },
            ],
            summary: RemoteTrainingVisualizationSummary {
                total_steps_completed: report.max_steps,
                latest_global_step: Some(report.max_steps),
                latest_train_loss: None,
                latest_ema_loss: None,
                latest_validation_loss: Some(report.xtrain_validation_mean_loss as f32),
                latest_tokens_per_second: Some(report.direct_runtime_tokens_per_second_rounded()),
                latest_samples_per_second_milli: None,
                accumulated_cost_microusd: None,
                latest_checkpoint_ref: None,
                detail: String::from(
                    "Bounded XTRAIN now emits one shared track-aware artifact with retained BPB, proof delta, one-token decode posture, and direct-versus-served runtime parity.",
                ),
            },
            heartbeat_series: Vec::new(),
            loss_series: vec![RemoteTrainingLossSample {
                global_step: Some(report.max_steps),
                elapsed_ms: report.max_steps.saturating_mul(report.step_duration_ms),
                train_loss: None,
                ema_loss: None,
                validation_loss: Some(report.xtrain_validation_mean_loss as f32),
            }],
            math_series: vec![RemoteTrainingMathSample {
                observed_at_ms: XTRAIN_RETAINED_SCORE_AT_MS,
                global_step: Some(report.max_steps),
                learning_rate: None,
                gradient_norm: None,
                parameter_norm: None,
                update_norm: None,
                clip_fraction: None,
                clip_event_count: None,
                loss_scale: None,
                non_finite_count: 0,
                model_specific_diagnostics: BTreeMap::from([
                    (
                        String::from("proof_validation_mean_loss"),
                        report.proof_validation_mean_loss as f32,
                    ),
                    (
                        String::from("proof_validation_bits_per_byte"),
                        report.proof_validation_bits_per_byte as f32,
                    ),
                    (
                        String::from("validation_loss_improvement_vs_proof"),
                        report.validation_loss_improvement() as f32,
                    ),
                    (
                        String::from("bits_per_byte_improvement_vs_proof"),
                        report.bits_per_byte_improvement() as f32,
                    ),
                    (
                        String::from("served_runtime_tokens_per_second"),
                        report.served_runtime_tokens_per_second as f32,
                    ),
                    (
                        String::from("bounded_attention_window_tokens"),
                        report.bounded_attention_window_tokens as f32,
                    ),
                    (
                        String::from("exact_prefix_match_tokens"),
                        report.exact_prefix_match_tokens as f32,
                    ),
                ]),
            }],
            runtime_series: vec![crate::RemoteTrainingRuntimeSample {
                observed_at_ms: XTRAIN_RETAINED_PARITY_AT_MS,
                data_wait_ms: None,
                forward_ms: None,
                backward_ms: None,
                optimizer_ms: None,
                checkpoint_ms: None,
                evaluation_ms: Some(report.direct_runtime_evaluation_ms()),
                tokens_per_second: Some(report.direct_runtime_tokens_per_second_rounded()),
                samples_per_second_milli: None,
            }],
            gpu_series: Vec::new(),
            distributed_series: Vec::new(),
            event_series: vec![
                RemoteTrainingEventSample {
                    observed_at_ms: XTRAIN_RETAINED_SCORE_AT_MS,
                    severity: RemoteTrainingEventSeverity::Info,
                    event_kind: String::from("xtrain_score_closeout_measured"),
                    detail: format!(
                        "Bounded XTRAIN closed out retained validation BPB {:.6} after {} local-reference steps.",
                        report.xtrain_validation_bits_per_byte,
                        report.max_steps
                    ),
                },
                RemoteTrainingEventSample {
                    observed_at_ms: XTRAIN_RETAINED_SCORE_AT_MS + 1,
                    severity: RemoteTrainingEventSeverity::Info,
                    event_kind: String::from("xtrain_delta_vs_proof_baseline"),
                    detail: format!(
                        "Bounded XTRAIN improved retained BPB versus the proof baseline by {:.6}.",
                        report.bits_per_byte_improvement()
                    ),
                },
                RemoteTrainingEventSample {
                    observed_at_ms: XTRAIN_RETAINED_PARITY_AT_MS,
                    severity: RemoteTrainingEventSeverity::Info,
                    event_kind: String::from("xtrain_direct_served_parity"),
                    detail: format!(
                        "Direct runtime and served runtime kept exact token parity at {:.3} tok/s and {:.3} tok/s.",
                        report.direct_runtime_tokens_per_second,
                        report.served_runtime_tokens_per_second
                    ),
                },
                RemoteTrainingEventSample {
                    observed_at_ms: XTRAIN_RETAINED_PARITY_AT_MS + 1,
                    severity: RemoteTrainingEventSeverity::Warning,
                    event_kind: String::from("xtrain_promotion_held"),
                    detail: String::from(
                        "Promotion remains held because bounded XTRAIN is not public-equivalent and still does not prove HOMEGOLF score relevance.",
                    ),
                },
                RemoteTrainingEventSample {
                    observed_at_ms: XTRAIN_RETAINED_PARITY_AT_MS + 2,
                    severity: RemoteTrainingEventSeverity::Warning,
                    event_kind: String::from("xtrain_exact_cycle_not_reached"),
                    detail: String::from(
                        "The bounded XTRAIN lane improved BPB but still failed exact-cycle recovery on the retained toy prompt.",
                    ),
                },
            ],
            source_artifacts: vec![
                RemoteTrainingSourceArtifact {
                    artifact_role: String::from("xtrain_quick_eval_report"),
                    artifact_uri: String::from(
                        PARAMETER_GOLF_XTRAIN_QUICK_EVAL_REPORT_FIXTURE_PATH,
                    ),
                    artifact_digest: Some(report.report_digest.clone()),
                    source_kind: RemoteTrainingArtifactSourceKind::FinalizerOwned,
                    authoritative: true,
                    source_receipt_ids: vec![String::from(
                        "psionic.parameter_golf_xtrain_quick_eval_report.v1",
                    )],
                    detail: String::from(
                        "The retained XTRAIN quick-eval report is authoritative for the bounded local-reference score, prompt behavior, and direct-versus-served runtime parity.",
                    ),
                },
                RemoteTrainingSourceArtifact {
                    artifact_role: String::from("xtrain_track_contract"),
                    artifact_uri: String::from(XTRAIN_TRACK_DOC_REF),
                    artifact_digest: None,
                    source_kind: RemoteTrainingArtifactSourceKind::FinalizerOwned,
                    authoritative: true,
                    source_receipt_ids: Vec::new(),
                    detail: String::from(
                        "The XTRAIN track contract is authoritative for score-law semantics, comparability boundaries, and the explicit non-public promotion hold.",
                    ),
                },
                RemoteTrainingSourceArtifact {
                    artifact_role: String::from("parameter_golf_promoted_family_contract"),
                    artifact_uri: String::from("docs/PARAMETER_GOLF_PROMOTED_FAMILY_CONTRACT.md"),
                    artifact_digest: None,
                    source_kind: RemoteTrainingArtifactSourceKind::FinalizerOwned,
                    authoritative: false,
                    source_receipt_ids: Vec::new(),
                    detail: String::from(
                        "The promoted family contract remains authoritative for the PGOLF-shaped model family and bundle handoff semantics beneath the bounded XTRAIN lane.",
                    ),
                },
                RemoteTrainingSourceArtifact {
                    artifact_role: String::from("xtrain_quick_eval_audit"),
                    artifact_uri: String::from(XTRAIN_AUDIT_REF),
                    artifact_digest: None,
                    source_kind: RemoteTrainingArtifactSourceKind::LocalMirror,
                    authoritative: false,
                    source_receipt_ids: Vec::new(),
                    detail: String::from(
                        "The quick-eval audit records the retained operator interpretation around the same bounded XTRAIN score lane.",
                    ),
                },
            ],
            bundle_digest: String::new(),
        },
    )?)
}

pub fn build_parameter_golf_xtrain_run_index_entry_v2(
    bundle: &RemoteTrainingVisualizationBundleV2,
) -> Result<RemoteTrainingRunIndexEntryV2, ParameterGolfXtrainVisualizationError> {
    let entry = RemoteTrainingRunIndexEntryV2 {
        provider: bundle.provider,
        profile_id: bundle.profile_id.clone(),
        lane_id: bundle.lane_id.clone(),
        run_id: bundle.run_id.clone(),
        repo_revision: bundle.repo_revision.clone(),
        track_semantics: bundle.track_semantics.clone(),
        primary_score: bundle.primary_score.clone(),
        score_surface: bundle.score_surface.clone(),
        result_classification: bundle.result_classification,
        series_status: bundle.series_status,
        series_unavailable_reason: bundle.series_unavailable_reason.clone(),
        last_heartbeat_at_ms: bundle.refresh_contract.last_heartbeat_at_ms,
        bundle_artifact_uri: Some(String::from(
            PARAMETER_GOLF_XTRAIN_VISUALIZATION_BUNDLE_V2_FIXTURE_PATH,
        )),
        bundle_digest: Some(bundle.bundle_digest.clone()),
        semantic_summary: String::from(
            "Bounded XTRAIN now appears in the shared track-aware run index with explicit proof posture, retained BPB, and a held non-public promotion gate.",
        ),
    };
    entry.validate()?;
    Ok(entry)
}

pub fn write_parameter_golf_xtrain_visualization_artifacts_v2(
    report_path: &Path,
    bundle_path: &Path,
    run_index_path: &Path,
) -> Result<
    (
        ParameterGolfXtrainQuickEvalReport,
        RemoteTrainingVisualizationBundleV2,
        RemoteTrainingRunIndexV2,
    ),
    ParameterGolfXtrainVisualizationError,
> {
    let report = build_parameter_golf_xtrain_quick_eval_report()?;
    let bundle = build_parameter_golf_xtrain_visualization_bundle_v2()?;
    let entry = build_parameter_golf_xtrain_run_index_entry_v2(&bundle)?;
    let run_index = build_remote_training_run_index_v2(RemoteTrainingRunIndexV2 {
        schema_version: String::new(),
        index_id: String::from("parameter-golf-xtrain-run-index-v2"),
        generated_at_ms: XTRAIN_RETAINED_PARITY_AT_MS,
        entries: vec![entry],
        detail: String::from(
            "Bounded XTRAIN now emits one shared v2 run-index row instead of leaving train-to-infer score posture in audits only.",
        ),
        index_digest: String::new(),
    })?;
    write_json(report_path, &report)?;
    write_json(bundle_path, &bundle)?;
    write_json(run_index_path, &run_index)?;
    Ok((report, bundle, run_index))
}

fn runtime_eval_ms(
    generated_tokens_per_runtime_eval: usize,
    throughput_repetitions: usize,
    tokens_per_second: f64,
) -> u64 {
    let total_tokens = generated_tokens_per_runtime_eval
        .saturating_mul(throughput_repetitions)
        .max(1);
    ((total_tokens as f64 / tokens_per_second) * 1_000.0).round() as u64
}

fn stable_xtrain_report_digest(report: &ParameterGolfXtrainQuickEvalReport) -> String {
    let mut canonical = report.clone();
    canonical.report_digest.clear();
    let bytes = serde_json::to_vec(&canonical)
        .expect("ParameterGolfXtrainQuickEvalReport should serialize");
    let mut hasher = Sha256::new();
    hasher.update(b"parameter_golf_xtrain_quick_eval_report|");
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn ensure_nonempty(
    value: &str,
    field: &'static str,
) -> Result<(), ParameterGolfXtrainVisualizationError> {
    if value.trim().is_empty() {
        return Err(ParameterGolfXtrainVisualizationError::InvalidReport(
            field,
            String::from("must not be empty"),
        ));
    }
    Ok(())
}

fn ensure_tokens(
    value: &[u32],
    field: &'static str,
) -> Result<(), ParameterGolfXtrainVisualizationError> {
    if value.is_empty() {
        return Err(ParameterGolfXtrainVisualizationError::InvalidReport(
            field,
            String::from("must not be empty"),
        ));
    }
    Ok(())
}

fn ensure_finite(
    value: f64,
    field: &'static str,
) -> Result<(), ParameterGolfXtrainVisualizationError> {
    if !value.is_finite() {
        return Err(ParameterGolfXtrainVisualizationError::InvalidReport(
            field,
            String::from("must be finite"),
        ));
    }
    Ok(())
}

fn write_json<T: Serialize>(
    output_path: &Path,
    value: &T,
) -> Result<(), ParameterGolfXtrainVisualizationError> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfXtrainVisualizationError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(
        output_path,
        format!("{}\n", serde_json::to_string_pretty(value)?),
    )
    .map_err(|error| ParameterGolfXtrainVisualizationError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_xtrain_bundle() -> RemoteTrainingVisualizationBundleV2 {
        serde_json::from_str(include_str!(
            "../../../fixtures/training_visualization/parameter_golf_xtrain_remote_training_visualization_bundle_v2.json"
        ))
        .expect("XTRAIN v2 bundle should parse")
    }

    fn sample_xtrain_report() -> ParameterGolfXtrainQuickEvalReport {
        serde_json::from_str(include_str!(
            "../../../fixtures/parameter_golf/reports/parameter_golf_xtrain_quick_eval_report.json"
        ))
        .expect("XTRAIN quick-eval report should parse")
    }

    #[test]
    fn xtrain_quick_eval_report_stays_valid() -> Result<(), ParameterGolfXtrainVisualizationError> {
        sample_xtrain_report().validate()?;
        Ok(())
    }

    #[test]
    fn xtrain_v2_bundle_stays_valid() -> Result<(), ParameterGolfXtrainVisualizationError> {
        sample_xtrain_bundle().validate()?;
        Ok(())
    }

    #[test]
    fn xtrain_v2_bundle_carries_bounded_proof_posture(
    ) -> Result<(), ParameterGolfXtrainVisualizationError> {
        let bundle = build_parameter_golf_xtrain_visualization_bundle_v2()?;
        assert_eq!(
            bundle.track_semantics.track_family,
            RemoteTrainingTrackFamilyV2::Xtrain
        );
        assert_eq!(
            bundle.track_semantics.proof_posture,
            crate::RemoteTrainingProofPostureV2::BoundedTrainToInfer
        );
        assert_eq!(
            bundle
                .score_surface
                .as_ref()
                .expect("XTRAIN bundle should carry score_surface")
                .promotion_gate_posture,
            RemoteTrainingPromotionGatePostureV2::Held
        );
        Ok(())
    }
}
