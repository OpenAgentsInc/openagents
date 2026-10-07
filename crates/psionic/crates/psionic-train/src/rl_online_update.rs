use std::collections::BTreeMap;

use psionic_adapters::LmHeadLoraLoadError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    OpenAdapterArtifactExportRequest, OpenAdapterExportedArtifact, OpenAdapterSftError,
    OpenAdapterTrainingExecutionBackend, OpenAdapterTrainingExecutionError,
    OpenAdapterTrainingSamplerConfig, OpenAdapterWeightedTargetBatchRecord,
    OpenAdapterWeightedTargetBatchRequest, OpenAdapterWeightedTokenTarget, PolicyRevision,
    RolloutReceiptOutcome, TrainingCoreError, TrainingOrchestratorBatchRecord,
    TrainingOrchestratorWindow, TrainingSamplerLogprobRequest, TrainingSamplerServedRevision,
    TrainingSamplerService, TrainingSamplerServiceError, TrainingStepReceipt,
};

/// Policy knobs for the bounded live RL update path.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiveRlUpdatePolicy {
    /// Maximum importance ratio admitted into the weighted update.
    pub max_importance_ratio: f32,
    /// Reward term blended into the weighted chosen-token loss.
    pub reward_mix: f32,
    /// Teacher confidence blended into the chosen-token loss when present.
    pub teacher_target_blend: f32,
}

impl LiveRlUpdatePolicy {
    fn validate(self) -> Result<(), LiveRlUpdateError> {
        if !self.max_importance_ratio.is_finite() || self.max_importance_ratio <= 0.0 {
            return Err(LiveRlUpdateError::InvalidPolicy(
                "max_importance_ratio must be positive and finite",
            ));
        }
        if !self.reward_mix.is_finite() || self.reward_mix < 0.0 {
            return Err(LiveRlUpdateError::InvalidPolicy(
                "reward_mix must be finite and greater than or equal to zero",
            ));
        }
        if !self.teacher_target_blend.is_finite() || self.teacher_target_blend < 0.0 {
            return Err(LiveRlUpdateError::InvalidPolicy(
                "teacher_target_blend must be finite and greater than or equal to zero",
            ));
        }
        Ok(())
    }
}

impl Default for LiveRlUpdatePolicy {
    fn default() -> Self {
        Self {
            max_importance_ratio: 2.0,
            reward_mix: 0.25,
            teacher_target_blend: 0.1,
        }
    }
}

/// Sequence-side prompt input for one admitted rollout artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiveRlRolloutInput {
    /// Stable rollout artifact identifier.
    pub artifact_id: String,
    /// Prompt text used as the completion prefix.
    pub prompt: String,
    /// Optional chosen-token teacher logprobs aligned to completion tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teacher_logprobs: Option<Vec<f32>>,
}

impl LiveRlRolloutInput {
    /// Creates one prompt-side rollout input.
    #[must_use]
    pub fn new(artifact_id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            artifact_id: artifact_id.into(),
            prompt: prompt.into(),
            teacher_logprobs: None,
        }
    }

    /// Attaches optional chosen-token teacher logprobs.
    #[must_use]
    pub fn with_teacher_logprobs(mut self, teacher_logprobs: Vec<f32>) -> Self {
        self.teacher_logprobs = Some(teacher_logprobs);
        self
    }
}

/// One materialized completion token in the live RL update path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiveRlMaterializedToken {
    /// Zero-based completion position.
    pub position: usize,
    /// Prompt or prefix text used to score this token.
    pub prompt_prefix: String,
    /// Stable digest over the prompt prefix.
    pub prompt_prefix_digest: String,
    /// Selected token identifier.
    pub token_id: u32,
    /// Selected token text.
    pub token_text: String,
    /// Rollout-time logprob carried by the artifact.
    pub observed_logprob: f32,
    /// Current active-revision logprob used for the live update.
    pub live_logprob: f32,
    /// Token-level reward from the rollout artifact.
    pub reward: f32,
    /// Token-level advantage from the rollout artifact.
    pub advantage: f32,
    /// Importance ratio between the live revision and the rollout artifact.
    pub importance_ratio: f32,
    /// Clipped importance ratio admitted into the update.
    pub clipped_importance_ratio: f32,
    /// Final scalar loss weight used for the chosen-token update.
    pub loss_weight: f32,
    /// Optional chosen-token teacher logprob.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teacher_logprob: Option<f32>,
}

/// One materialized rollout ready for live RL token updates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiveRlMaterializedRollout {
    /// Stable rollout artifact identifier.
    pub artifact_id: String,
    /// Stable rollout artifact digest.
    pub artifact_digest: String,
    /// Worker id that produced the rollout.
    pub worker_id: String,
    /// Stable task identifier.
    pub task_id: String,
    /// Exact versus bounded off-policy admission outcome.
    pub admission_outcome: RolloutReceiptOutcome,
    /// Source policy revision that generated the rollout.
    pub source_policy_revision: PolicyRevision,
    /// Prompt text carried into the update path.
    pub prompt: String,
    /// Stable digest over the prompt.
    pub prompt_digest: String,
    /// Decoded completion text.
    pub completion_text: String,
    /// Token-level materialization records.
    pub tokens: Vec<LiveRlMaterializedToken>,
    /// Aggregate reward across completion tokens.
    pub reward_sum: f32,
    /// Aggregate advantage across completion tokens.
    pub advantage_sum: f32,
}

/// Inspectable live RL batch built from one orchestrator-owned trainer batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiveRlMaterializedBatch {
    /// Stable update identifier.
    pub update_id: String,
    /// Stable trainer batch identifier.
    pub batch_id: String,
    /// Stable trainer batch digest.
    pub batch_digest: String,
    /// Stable batch-request digest from the orchestrator.
    pub request_digest: String,
    /// Stable orchestrator window id.
    pub window_id: String,
    /// Target policy revision the live update is anchored to.
    pub target_policy_revision: PolicyRevision,
    /// Materialized rollout records.
    pub rollouts: Vec<LiveRlMaterializedRollout>,
    /// Accepted exact rollout count.
    pub exact_rollout_count: u32,
    /// Accepted off-policy rollout count.
    pub off_policy_rollout_count: u32,
    /// Completion token count across the materialized batch.
    pub completion_token_count: u64,
    /// Completion token count carrying teacher logprobs.
    pub teacher_token_count: u64,
    /// Aggregate reward across the materialized batch.
    pub reward_sum: f32,
    /// Aggregate advantage across the materialized batch.
    pub advantage_sum: f32,
    /// Mean rollout-time chosen-token logprob.
    pub mean_observed_logprob: f32,
    /// Mean live chosen-token logprob.
    pub mean_live_logprob: f32,
    /// Mean loss weight admitted into the update.
    pub mean_loss_weight: f32,
    /// Stable digest over the materialized live batch.
    pub materialization_digest: String,
}

/// Request for one bounded live RL update over a current adapter revision.
#[derive(Clone, Debug, PartialEq)]
pub struct LiveRlUpdateRequest {
    /// Stable update identifier.
    pub update_id: String,
    /// Orchestrator batch record that selected the admitted rollouts.
    pub batch_record: TrainingOrchestratorBatchRecord,
    /// Orchestrator window carrying the accepted rollout receipts.
    pub window: TrainingOrchestratorWindow,
    /// Prompt-side rollout inputs keyed by artifact id.
    pub rollout_inputs: Vec<LiveRlRolloutInput>,
    /// Currently active served revision that should receive the update.
    pub current_revision: TrainingSamplerServedRevision,
    /// Policy revision that should be emitted after the update.
    pub promoted_policy_revision: PolicyRevision,
    /// Adapter export identity for the promoted revision.
    pub export_request: OpenAdapterArtifactExportRequest,
    /// Live update weighting policy.
    pub policy: LiveRlUpdatePolicy,
    /// Logical step start.
    pub started_at_ms: u64,
    /// Logical step finish.
    pub finished_at_ms: u64,
}

/// Machine-legible receipt for one live RL update cycle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiveRlUpdateReceipt {
    /// Stable update identifier.
    pub update_id: String,
    /// Stable trainer batch identifier.
    pub batch_id: String,
    /// Stable orchestrator window id.
    pub window_id: String,
    /// Source policy revision id.
    pub source_revision_id: String,
    /// Promoted policy revision id.
    pub promoted_revision_id: String,
    /// Promoted policy revision number when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promoted_revision_number: Option<u64>,
    /// Exact rollout count consumed by the update.
    pub exact_rollout_count: u32,
    /// Off-policy rollout count consumed by the update.
    pub off_policy_rollout_count: u32,
    /// Completion token count consumed by the update.
    pub completion_token_count: u64,
    /// Teacher-token count consumed by the update.
    pub teacher_token_count: u64,
    /// Aggregate reward across the materialized batch.
    pub reward_sum: f32,
    /// Aggregate advantage across the materialized batch.
    pub advantage_sum: f32,
    /// Mean rollout-time chosen-token logprob.
    pub mean_observed_logprob: f32,
    /// Mean live chosen-token logprob.
    pub mean_live_logprob: f32,
    /// Mean loss weight admitted into the update.
    pub mean_loss_weight: f32,
    /// Mean weighted chosen-token loss applied by the adapter backend.
    pub mean_weighted_loss: f32,
    /// Stable adapter identity digest for the promoted artifact.
    pub adapter_identity_digest: String,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Full bounded live RL update outcome.
#[derive(Clone, Debug, PartialEq)]
pub struct LiveRlUpdateOutcome {
    /// Materialized live RL batch.
    pub materialized_batch: LiveRlMaterializedBatch,
    /// Weighted token-target gradient record consumed by the trainer core.
    pub weighted_batch_record: OpenAdapterWeightedTargetBatchRecord,
    /// Trainer-step receipt emitted by the fixed-budget core.
    pub step_receipt: TrainingStepReceipt,
    /// Exported adapter artifact ready for sampler adoption.
    pub exported_artifact: OpenAdapterExportedArtifact,
    /// Served revision payload ready for `TrainingSamplerService::refresh_revision`.
    pub promoted_revision: TrainingSamplerServedRevision,
    /// Final live-update receipt.
    pub receipt: LiveRlUpdateReceipt,
}

/// Fail-closed errors for the bounded live RL update path.
#[derive(Debug, Error)]
pub enum LiveRlUpdateError {
    #[error("live RL update is missing `update_id`")]
    MissingUpdateId,
    #[error("live RL update policy is invalid: {0}")]
    InvalidPolicy(&'static str),
    #[error(
        "live RL sampler/backend mismatch for `{field}`: expected `{expected}`, found `{actual}`"
    )]
    SamplerBackendMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },
    #[error(
        "live RL update batch window `{batch_window_id}` does not match provided window `{window_id}`"
    )]
    BatchWindowMismatch {
        batch_window_id: String,
        window_id: String,
    },
    #[error(
        "live RL update current revision `{actual_revision_id}` does not match batch target `{expected_revision_id}`"
    )]
    CurrentRevisionMismatch {
        expected_revision_id: String,
        actual_revision_id: String,
    },
    #[error(
        "live RL update promoted policy family `{actual_policy_family}` does not match current family `{expected_policy_family}`"
    )]
    PromotedPolicyFamilyMismatch {
        expected_policy_family: String,
        actual_policy_family: String,
    },
    #[error(
        "live RL update promoted revision number {promoted_revision_number} is not newer than current {current_revision_number}"
    )]
    NonMonotonicPromotedRevision {
        current_revision_number: u64,
        promoted_revision_number: u64,
    },
    #[error(
        "live RL update promoted produced_at_ms {promoted_produced_at_ms} is not newer than current {current_produced_at_ms}"
    )]
    NonMonotonicPromotedTimestamp {
        current_produced_at_ms: u64,
        promoted_produced_at_ms: u64,
    },
    #[error("live RL update rollout input `{artifact_id}` is duplicated")]
    DuplicateRolloutInput { artifact_id: String },
    #[error("live RL update is missing rollout input for `{artifact_id}`")]
    MissingRolloutInput { artifact_id: String },
    #[error("live RL update received rollout input for unknown artifact ids {artifact_ids:?}")]
    UnknownRolloutInputs { artifact_ids: Vec<String> },
    #[error("live RL update rollout `{artifact_id}` prompt is empty")]
    EmptyPrompt { artifact_id: String },
    #[error(
        "live RL update rollout `{artifact_id}` teacher logprob count mismatch: expected {expected}, found {actual}"
    )]
    TeacherLogprobCountMismatch {
        artifact_id: String,
        expected: usize,
        actual: usize,
    },
    #[error("live RL update window is missing accepted rollout `{artifact_id}`")]
    MissingAcceptedRollout { artifact_id: String },
    #[error(
        "live RL update rollout `{artifact_id}` is not batch-eligible because admission outcome is `{outcome}`"
    )]
    NonBatchEligibleRollout {
        artifact_id: String,
        outcome: String,
    },
    #[error("live RL update rollout `{artifact_id}` could not decode token `{token_id}`")]
    UnknownTokenText { artifact_id: String, token_id: u32 },
    #[error(transparent)]
    Sampler(#[from] TrainingSamplerServiceError),
    #[error(transparent)]
    AdapterLoad(#[from] LmHeadLoraLoadError),
    #[error(transparent)]
    TrainingExecution(#[from] OpenAdapterTrainingExecutionError),
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
    #[error(transparent)]
    Export(#[from] OpenAdapterSftError),
}

/// Bounded live RL update executor over the open-adapter trainer and sampler lanes.
#[derive(Clone, Debug)]
pub struct OpenAdapterLiveRlUpdateExecutor {
    backend: OpenAdapterTrainingExecutionBackend,
    sampler_config: OpenAdapterTrainingSamplerConfig,
}

impl OpenAdapterLiveRlUpdateExecutor {
    /// Creates one bounded live RL update executor after validating backend and
    /// sampler compatibility.
    pub fn new(
        backend: OpenAdapterTrainingExecutionBackend,
        sampler_config: OpenAdapterTrainingSamplerConfig,
    ) -> Result<Self, LiveRlUpdateError> {
        validate_sampler_backend_alignment(&backend, &sampler_config)?;
        Ok(Self {
            backend,
            sampler_config,
        })
    }

    /// Runs one full live update cycle from admitted rollout batch to promoted
    /// adapter revision.
    pub fn run_update(
        &self,
        request: &LiveRlUpdateRequest,
    ) -> Result<LiveRlUpdateOutcome, LiveRlUpdateError> {
        if request.update_id.trim().is_empty() {
            return Err(LiveRlUpdateError::MissingUpdateId);
        }
        request.policy.validate()?;
        if request.batch_record.request.window_id != request.window.window_id {
            return Err(LiveRlUpdateError::BatchWindowMismatch {
                batch_window_id: request.batch_record.request.window_id.clone(),
                window_id: request.window.window_id.clone(),
            });
        }

        let batch_target = &request.batch_record.batch.policy_lineage.target_revision;
        if batch_target.revision_id != request.current_revision.policy_revision.revision_id {
            return Err(LiveRlUpdateError::CurrentRevisionMismatch {
                expected_revision_id: batch_target.revision_id.clone(),
                actual_revision_id: request.current_revision.policy_revision.revision_id.clone(),
            });
        }
        if request.promoted_policy_revision.policy_family
            != request.current_revision.policy_revision.policy_family
        {
            return Err(LiveRlUpdateError::PromotedPolicyFamilyMismatch {
                expected_policy_family: request
                    .current_revision
                    .policy_revision
                    .policy_family
                    .clone(),
                actual_policy_family: request.promoted_policy_revision.policy_family.clone(),
            });
        }
        if let (Some(current), Some(promoted)) = (
            request.current_revision.policy_revision.revision_number,
            request.promoted_policy_revision.revision_number,
        ) {
            if promoted <= current {
                return Err(LiveRlUpdateError::NonMonotonicPromotedRevision {
                    current_revision_number: current,
                    promoted_revision_number: promoted,
                });
            }
        }
        if request.promoted_policy_revision.produced_at_ms
            <= request.current_revision.policy_revision.produced_at_ms
        {
            return Err(LiveRlUpdateError::NonMonotonicPromotedTimestamp {
                current_produced_at_ms: request.current_revision.policy_revision.produced_at_ms,
                promoted_produced_at_ms: request.promoted_policy_revision.produced_at_ms,
            });
        }
        request.export_request.validate()?;

        let mut rollout_inputs = request
            .rollout_inputs
            .iter()
            .map(|input| {
                (
                    input.artifact_id.clone(),
                    LiveRlRolloutInput {
                        artifact_id: input.artifact_id.clone(),
                        prompt: input.prompt.clone(),
                        teacher_logprobs: input.teacher_logprobs.clone(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        if rollout_inputs.len() != request.rollout_inputs.len() {
            let duplicate = find_duplicate_rollout_input(request.rollout_inputs.as_slice())
                .unwrap_or_else(|| String::from("unknown"));
            return Err(LiveRlUpdateError::DuplicateRolloutInput {
                artifact_id: duplicate,
            });
        }

        let mut sampler = TrainingSamplerService::new(self.sampler_config.clone())?;
        let _ =
            sampler.refresh_revision(request.current_revision.clone(), request.started_at_ms)?;

        let accepted_by_id = request
            .window
            .accepted_rollouts
            .iter()
            .map(|record| (record.reference.artifact_id.clone(), record))
            .collect::<BTreeMap<_, _>>();
        let mut weighted_targets = Vec::new();
        let mut materialized_rollouts = Vec::new();
        let mut exact_rollout_count = 0_u32;
        let mut off_policy_rollout_count = 0_u32;
        let mut completion_token_count = 0_u64;
        let mut teacher_token_count = 0_u64;
        let mut reward_sum = 0.0_f32;
        let mut advantage_sum = 0.0_f32;
        let mut observed_logprob_sum = 0.0_f32;
        let mut live_logprob_sum = 0.0_f32;
        let mut loss_weight_sum = 0.0_f32;

        for artifact_id in &request.batch_record.batch.rollout_ids {
            let accepted = accepted_by_id.get(artifact_id).ok_or_else(|| {
                LiveRlUpdateError::MissingAcceptedRollout {
                    artifact_id: artifact_id.clone(),
                }
            })?;
            let input = rollout_inputs.remove(artifact_id).ok_or_else(|| {
                LiveRlUpdateError::MissingRolloutInput {
                    artifact_id: artifact_id.clone(),
                }
            })?;
            if input.prompt.trim().is_empty() {
                return Err(LiveRlUpdateError::EmptyPrompt {
                    artifact_id: artifact_id.clone(),
                });
            }
            let teacher_logprobs = input.teacher_logprobs.unwrap_or_default();
            if !teacher_logprobs.is_empty()
                && teacher_logprobs.len() != accepted.artifact.samples.len()
            {
                return Err(LiveRlUpdateError::TeacherLogprobCountMismatch {
                    artifact_id: artifact_id.clone(),
                    expected: accepted.artifact.samples.len(),
                    actual: teacher_logprobs.len(),
                });
            }

            match accepted.receipt.outcome {
                RolloutReceiptOutcome::AcceptedExact => {
                    exact_rollout_count = exact_rollout_count.saturating_add(1);
                }
                RolloutReceiptOutcome::AcceptedOffPolicy => {
                    off_policy_rollout_count = off_policy_rollout_count.saturating_add(1);
                }
                outcome => {
                    return Err(LiveRlUpdateError::NonBatchEligibleRollout {
                        artifact_id: artifact_id.clone(),
                        outcome: rollout_receipt_outcome_label(outcome).to_string(),
                    });
                }
            }

            let continuation_token_ids = accepted
                .artifact
                .samples
                .iter()
                .map(|sample| sample.token_id)
                .collect::<Vec<_>>();
            let live_logprob_response = sampler.token_logprobs(&TrainingSamplerLogprobRequest {
                request_id: format!("{}-{}-logprobs", request.update_id, artifact_id),
                prompt: input.prompt.trim().to_string(),
                continuation_token_ids,
                requested_revision_id: Some(
                    request.current_revision.policy_revision.revision_id.clone(),
                ),
                top_logprobs: Some(1),
                requested_at_ms: request.started_at_ms,
            })?;

            let teacher_iter = if teacher_logprobs.is_empty() {
                vec![None; accepted.artifact.samples.len()]
            } else {
                teacher_logprobs
                    .into_iter()
                    .map(Some)
                    .collect::<Vec<Option<f32>>>()
            };
            let mut prompt_prefix = input.prompt.trim().to_string();
            let mut completion_text = String::new();
            let mut materialized_tokens = Vec::with_capacity(accepted.artifact.samples.len());
            for ((position, sample), (live_token, teacher_logprob)) in
                accepted.artifact.samples.iter().enumerate().zip(
                    live_logprob_response
                        .tokens
                        .iter()
                        .zip(teacher_iter.into_iter()),
                )
            {
                let importance_ratio = (live_token.logprob - sample.logprob).exp();
                let clipped_importance_ratio =
                    importance_ratio.clamp(0.0, request.policy.max_importance_ratio);
                let loss_weight = (clipped_importance_ratio * sample.advantage)
                    + (request.policy.reward_mix * sample.reward);
                if teacher_logprob.is_some() {
                    teacher_token_count = teacher_token_count.saturating_add(1);
                }
                observed_logprob_sum += sample.logprob;
                live_logprob_sum += live_token.logprob;
                loss_weight_sum += loss_weight;
                weighted_targets.push(attach_teacher_logprob(
                    OpenAdapterWeightedTokenTarget::new(
                        format!("{artifact_id}:token:{position}"),
                        self.sampler_config
                            .encode_prompt_hidden_state(prompt_prefix.as_str()),
                        sample.token_id,
                        loss_weight,
                    ),
                    teacher_logprob,
                ));
                materialized_tokens.push(LiveRlMaterializedToken {
                    position,
                    prompt_prefix_digest: stable_prompt_digest(prompt_prefix.as_str()),
                    prompt_prefix: prompt_prefix.clone(),
                    token_id: sample.token_id,
                    token_text: live_token.token_text.clone(),
                    observed_logprob: sample.logprob,
                    live_logprob: live_token.logprob,
                    reward: sample.reward,
                    advantage: sample.advantage,
                    importance_ratio,
                    clipped_importance_ratio,
                    loss_weight,
                    teacher_logprob,
                });
                prompt_prefix =
                    append_token_to_prompt(prompt_prefix.as_str(), live_token.token_text.as_str());
                completion_text = append_token_to_prompt(
                    completion_text.as_str(),
                    live_token.token_text.as_str(),
                );
            }

            completion_token_count =
                completion_token_count.saturating_add(materialized_tokens.len() as u64);
            reward_sum += accepted.artifact.reward_sum();
            advantage_sum += accepted.artifact.advantage_sum();
            materialized_rollouts.push(LiveRlMaterializedRollout {
                artifact_id: accepted.artifact.artifact_id.clone(),
                artifact_digest: accepted.artifact.artifact_digest.clone(),
                worker_id: accepted.artifact.worker_id.clone(),
                task_id: accepted.artifact.task_id.clone(),
                admission_outcome: accepted.receipt.outcome,
                source_policy_revision: accepted.artifact.source_policy_revision.clone(),
                prompt: input.prompt.trim().to_string(),
                prompt_digest: stable_prompt_digest(input.prompt.trim()),
                completion_text,
                tokens: materialized_tokens,
                reward_sum: accepted.artifact.reward_sum(),
                advantage_sum: accepted.artifact.advantage_sum(),
            });
        }

        if !rollout_inputs.is_empty() {
            return Err(LiveRlUpdateError::UnknownRolloutInputs {
                artifact_ids: rollout_inputs.into_keys().collect(),
            });
        }

        let token_scale = completion_token_count.max(1) as f32;
        let materialized_batch = LiveRlMaterializedBatch {
            update_id: request.update_id.clone(),
            batch_id: request.batch_record.batch.batch_id.clone(),
            batch_digest: request.batch_record.batch.batch_digest.clone(),
            request_digest: request.batch_record.request.request_digest.clone(),
            window_id: request.window.window_id.clone(),
            target_policy_revision: request
                .batch_record
                .batch
                .policy_lineage
                .target_revision
                .clone(),
            rollouts: materialized_rollouts,
            exact_rollout_count,
            off_policy_rollout_count,
            completion_token_count,
            teacher_token_count,
            reward_sum,
            advantage_sum,
            mean_observed_logprob: observed_logprob_sum / token_scale,
            mean_live_logprob: live_logprob_sum / token_scale,
            mean_loss_weight: loss_weight_sum / token_scale,
            materialization_digest: stable_materialized_batch_digest(
                request.update_id.as_str(),
                request.batch_record.batch.batch_id.as_str(),
                request.batch_record.batch.batch_digest.as_str(),
                request.batch_record.request.request_digest.as_str(),
                materialized_rollouts_digest(
                    request.batch_record.batch.rollout_ids.as_slice(),
                    &request.batch_record.batch.batch_digest,
                    &reward_sum.to_bits().to_string(),
                ),
                exact_rollout_count,
                off_policy_rollout_count,
                completion_token_count,
                teacher_token_count,
                reward_sum,
                advantage_sum,
                observed_logprob_sum / token_scale,
                live_logprob_sum / token_scale,
                loss_weight_sum / token_scale,
            ),
        };

        let loaded_adapter = psionic_adapters::LmHeadLoraAdapterArtifact::from_safetensors_bytes(
            request.current_revision.adapter_bytes.as_slice(),
            request.current_revision.adapter_identity.clone(),
            request.current_revision.adapter_alpha,
        )?;
        let mut run = self
            .backend
            .initialize_run_from_loaded_adapter(&loaded_adapter)?;
        let weighted_batch_request = OpenAdapterWeightedTargetBatchRequest::new(
            format!("{}-weighted-targets", request.update_id),
            weighted_targets,
            request.policy.teacher_target_blend,
        );
        let (step_input, weighted_batch_record) = self.backend.produce_weighted_target_step_input(
            &run,
            &weighted_batch_request,
            request.started_at_ms,
            request.finished_at_ms,
        )?;
        let step_receipt = run.apply_step(step_input)?;
        let exported_artifact = self
            .backend
            .export_run_artifact(&run, &request.export_request)?;
        let mut promoted_policy_revision = request.promoted_policy_revision.clone();
        promoted_policy_revision.policy_digest = exported_artifact.adapter_artifact_digest.clone();
        promoted_policy_revision.parent_revision_id =
            Some(request.current_revision.policy_revision.revision_id.clone());
        let promoted_revision = TrainingSamplerServedRevision::new(
            promoted_policy_revision.clone(),
            exported_artifact.adapter_identity.clone(),
            exported_artifact.adapter_alpha,
            exported_artifact.adapter_bytes.clone(),
        );
        let receipt = LiveRlUpdateReceipt {
            update_id: request.update_id.clone(),
            batch_id: request.batch_record.batch.batch_id.clone(),
            window_id: request.window.window_id.clone(),
            source_revision_id: request.current_revision.policy_revision.revision_id.clone(),
            promoted_revision_id: promoted_policy_revision.revision_id.clone(),
            promoted_revision_number: promoted_policy_revision.revision_number,
            exact_rollout_count,
            off_policy_rollout_count,
            completion_token_count,
            teacher_token_count,
            reward_sum,
            advantage_sum,
            mean_observed_logprob: materialized_batch.mean_observed_logprob,
            mean_live_logprob: materialized_batch.mean_live_logprob,
            mean_loss_weight: materialized_batch.mean_loss_weight,
            mean_weighted_loss: weighted_batch_record.mean_weighted_loss,
            adapter_identity_digest: exported_artifact.adapter_identity_digest.clone(),
            receipt_digest: stable_live_update_receipt_digest(
                request.update_id.as_str(),
                request.batch_record.batch.batch_id.as_str(),
                request.window.window_id.as_str(),
                request
                    .current_revision
                    .policy_revision
                    .revision_id
                    .as_str(),
                promoted_policy_revision.revision_id.as_str(),
                exact_rollout_count,
                off_policy_rollout_count,
                completion_token_count,
                teacher_token_count,
                reward_sum,
                advantage_sum,
                materialized_batch.mean_observed_logprob,
                materialized_batch.mean_live_logprob,
                materialized_batch.mean_loss_weight,
                weighted_batch_record.mean_weighted_loss,
                exported_artifact.adapter_identity_digest.as_str(),
                materialized_batch.materialization_digest.as_str(),
                step_receipt.receipt_digest.as_str(),
                weighted_batch_record.execution_digest.as_str(),
            ),
        };

        Ok(LiveRlUpdateOutcome {
            materialized_batch,
            weighted_batch_record,
            step_receipt,
            exported_artifact,
            promoted_revision,
            receipt,
        })
    }
}

fn validate_sampler_backend_alignment(
    backend: &OpenAdapterTrainingExecutionBackend,
    sampler_config: &OpenAdapterTrainingSamplerConfig,
) -> Result<(), LiveRlUpdateError> {
    validate_sampler_backend_field(
        "base_model_id",
        backend.config().model.base_model_id.as_str(),
        sampler_config.model.base_model_id.as_str(),
    )?;
    validate_sampler_backend_field(
        "base_model_revision",
        backend.config().model.base_model_revision.as_str(),
        sampler_config.model.base_model_revision.as_str(),
    )?;
    validate_sampler_backend_field(
        "base_served_artifact_digest",
        backend.config().model.base_served_artifact_digest.as_str(),
        sampler_config.model.base_served_artifact_digest.as_str(),
    )?;
    if backend.config().model.hidden_size != sampler_config.model.hidden_size {
        return Err(LiveRlUpdateError::SamplerBackendMismatch {
            field: "hidden_size",
            expected: backend.config().model.hidden_size.to_string(),
            actual: sampler_config.model.hidden_size.to_string(),
        });
    }
    if backend.config().model.vocab_size != sampler_config.model.vocab_size {
        return Err(LiveRlUpdateError::SamplerBackendMismatch {
            field: "vocab_size",
            expected: backend.config().model.vocab_size.to_string(),
            actual: sampler_config.model.vocab_size.to_string(),
        });
    }
    Ok(())
}

fn validate_sampler_backend_field(
    field: &'static str,
    expected: &str,
    actual: &str,
) -> Result<(), LiveRlUpdateError> {
    if expected != actual {
        return Err(LiveRlUpdateError::SamplerBackendMismatch {
            field,
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn find_duplicate_rollout_input(inputs: &[LiveRlRolloutInput]) -> Option<String> {
    let mut seen = std::collections::BTreeSet::new();
    for input in inputs {
        if !seen.insert(input.artifact_id.clone()) {
            return Some(input.artifact_id.clone());
        }
    }
    None
}

fn attach_teacher_logprob(
    target: OpenAdapterWeightedTokenTarget,
    teacher_logprob: Option<f32>,
) -> OpenAdapterWeightedTokenTarget {
    match teacher_logprob {
        Some(teacher_logprob) => target.with_teacher_target_logprob(teacher_logprob),
        None => target,
    }
}

fn append_token_to_prompt(prefix: &str, token_text: &str) -> String {
    if prefix.trim().is_empty() {
        token_text.to_string()
    } else {
        format!("{prefix} {token_text}")
    }
}

fn stable_prompt_digest(prompt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_live_rl_prompt|");
    hasher.update(prompt.trim().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_materialized_batch_digest(
    update_id: &str,
    batch_id: &str,
    batch_digest: &str,
    request_digest: &str,
    rollouts_digest: String,
    exact_rollout_count: u32,
    off_policy_rollout_count: u32,
    completion_token_count: u64,
    teacher_token_count: u64,
    reward_sum: f32,
    advantage_sum: f32,
    mean_observed_logprob: f32,
    mean_live_logprob: f32,
    mean_loss_weight: f32,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_live_rl_materialized_batch|");
    hasher.update(update_id.as_bytes());
    hasher.update(b"|");
    hasher.update(batch_id.as_bytes());
    hasher.update(b"|");
    hasher.update(batch_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(request_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(rollouts_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(exact_rollout_count.to_le_bytes());
    hasher.update(b"|");
    hasher.update(off_policy_rollout_count.to_le_bytes());
    hasher.update(b"|");
    hasher.update(completion_token_count.to_le_bytes());
    hasher.update(b"|");
    hasher.update(teacher_token_count.to_le_bytes());
    hasher.update(b"|");
    hasher.update(reward_sum.to_bits().to_le_bytes());
    hasher.update(b"|");
    hasher.update(advantage_sum.to_bits().to_le_bytes());
    hasher.update(b"|");
    hasher.update(mean_observed_logprob.to_bits().to_le_bytes());
    hasher.update(b"|");
    hasher.update(mean_live_logprob.to_bits().to_le_bytes());
    hasher.update(b"|");
    hasher.update(mean_loss_weight.to_bits().to_le_bytes());
    hex::encode(hasher.finalize())
}

fn materialized_rollouts_digest(
    rollout_ids: &[String],
    batch_digest: &str,
    reward_marker: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_live_rl_rollouts|");
    hasher.update(batch_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(reward_marker.as_bytes());
    for rollout_id in rollout_ids {
        hasher.update(b"|");
        hasher.update(rollout_id.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_live_update_receipt_digest(
    update_id: &str,
    batch_id: &str,
    window_id: &str,
    source_revision_id: &str,
    promoted_revision_id: &str,
    exact_rollout_count: u32,
    off_policy_rollout_count: u32,
    completion_token_count: u64,
    teacher_token_count: u64,
    reward_sum: f32,
    advantage_sum: f32,
    mean_observed_logprob: f32,
    mean_live_logprob: f32,
    mean_loss_weight: f32,
    mean_weighted_loss: f32,
    adapter_identity_digest: &str,
    materialization_digest: &str,
    step_receipt_digest: &str,
    weighted_execution_digest: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_live_rl_update_receipt|");
    hasher.update(update_id.as_bytes());
    hasher.update(b"|");
    hasher.update(batch_id.as_bytes());
    hasher.update(b"|");
    hasher.update(window_id.as_bytes());
    hasher.update(b"|");
    hasher.update(source_revision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(promoted_revision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(exact_rollout_count.to_le_bytes());
    hasher.update(b"|");
    hasher.update(off_policy_rollout_count.to_le_bytes());
    hasher.update(b"|");
    hasher.update(completion_token_count.to_le_bytes());
    hasher.update(b"|");
    hasher.update(teacher_token_count.to_le_bytes());
    hasher.update(b"|");
    hasher.update(reward_sum.to_bits().to_le_bytes());
    hasher.update(b"|");
    hasher.update(advantage_sum.to_bits().to_le_bytes());
    hasher.update(b"|");
    hasher.update(mean_observed_logprob.to_bits().to_le_bytes());
    hasher.update(b"|");
    hasher.update(mean_live_logprob.to_bits().to_le_bytes());
    hasher.update(b"|");
    hasher.update(mean_loss_weight.to_bits().to_le_bytes());
    hasher.update(b"|");
    hasher.update(mean_weighted_loss.to_bits().to_le_bytes());
    hasher.update(b"|");
    hasher.update(adapter_identity_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(materialization_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(step_receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(weighted_execution_digest.as_bytes());
    hex::encode(hasher.finalize())
}

fn rollout_receipt_outcome_label(outcome: RolloutReceiptOutcome) -> &'static str {
    match outcome {
        RolloutReceiptOutcome::AcceptedExact => "accepted_exact",
        RolloutReceiptOutcome::AcceptedOffPolicy => "accepted_off_policy",
        RolloutReceiptOutcome::Quarantined => "quarantined",
        RolloutReceiptOutcome::Discarded => "discarded",
    }
}

#[cfg(test)]
mod tests {
    use psionic_data::{TokenizerDigest, TokenizerFamily};
    use psionic_environments::EnvironmentPackageKey;
    use psionic_runtime::TrainingCheckpointReference;

    use super::*;
    use crate::{
        AcceptedRolloutRecord, OpenAdapterAdmissibleModelFamily, OpenAdapterExecutionConfig,
        OpenAdapterHiddenStateSample, OpenAdapterLmHeadTarget, OpenAdapterPrecisionPolicy,
        OpenAdapterReferenceModel, OpenAdapterSftRunRequest, OpenAdapterTrainingSamplerConfig,
        RolloutAdmissionReceipt, RolloutArtifact, RolloutArtifactRef, RolloutProofKind,
        RolloutProofReference, RolloutSample, RolloutTerminationReason, TrainerBatch,
        TrainerBatchAssemblyRequest, TrainingLoopBudget, TrainingOptimizerConfig,
        TrainingOptimizerResidencyPolicy, TrainingSamplerService, TrainingSamplerServicePolicy,
        TrainingSamplerVocabularyToken,
    };

    fn tokenizer() -> TokenizerDigest {
        TokenizerDigest::new(TokenizerFamily::BytePairEncoding, "gpt-oss-tok-v1", 32)
            .with_template_digest("gpt-oss-template-v1")
    }

    fn backend_config() -> OpenAdapterExecutionConfig {
        OpenAdapterExecutionConfig {
            run_id: "live-rl-open-adapter".to_string(),
            checkpoint_family: "open.adapter.policy".to_string(),
            execution_backend_label: crate::OPEN_ADAPTER_CUDA_BACKEND_LABEL.to_string(),
            admissible_model_family: OpenAdapterAdmissibleModelFamily::GptOssDecoderLmHeadLora,
            budget: TrainingLoopBudget::new(1, 1, 1).expect("budget"),
            batch_size: 2,
            precision_policy: OpenAdapterPrecisionPolicy::F32Reference,
            model: OpenAdapterReferenceModel {
                base_model_id: "gpt-oss-20b".to_string(),
                base_model_revision: "2026-03".to_string(),
                base_served_artifact_digest: "sha256:gpt-oss-base".to_string(),
                tokenizer: tokenizer(),
                hidden_size: 4,
                vocab_size: 4,
                target: OpenAdapterLmHeadTarget {
                    target_id: "lm_head".to_string(),
                    lora_rank: 2,
                    lora_alpha: 8.0,
                    optimizer: TrainingOptimizerConfig::adamw(0.15, 0.9, 0.99, 1e-8)
                        .with_gradient_clip_norm(1.0),
                    optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
                },
            },
        }
    }

    fn backend_samples() -> Vec<OpenAdapterHiddenStateSample> {
        vec![
            OpenAdapterHiddenStateSample::new("sample-paris-a", vec![1.0, 0.0, 0.0, 0.0], 2, 8)
                .expect("sample"),
            OpenAdapterHiddenStateSample::new("sample-paris-b", vec![1.0, 0.0, 0.0, 0.0], 2, 7)
                .expect("sample"),
            OpenAdapterHiddenStateSample::new("sample-berlin-a", vec![0.0, 1.0, 0.0, 0.0], 3, 8)
                .expect("sample"),
            OpenAdapterHiddenStateSample::new("sample-berlin-b", vec![0.0, 1.0, 0.0, 0.0], 3, 7)
                .expect("sample"),
        ]
    }

    fn sampler_config() -> OpenAdapterTrainingSamplerConfig {
        OpenAdapterTrainingSamplerConfig {
            service_id: "live-rl-sampler".to_string(),
            policy_family: "open.adapter.policy".to_string(),
            model: backend_config().model,
            vocabulary: vec![
                TrainingSamplerVocabularyToken::new(0, "cloud"),
                TrainingSamplerVocabularyToken::new(1, "storm"),
                TrainingSamplerVocabularyToken::new(2, "sunny"),
                TrainingSamplerVocabularyToken::new(3, "rain"),
            ],
            prompt_feature_lexicon: vec![
                crate::TrainingSamplerPromptFeature::new("paris", vec![1.0, 0.0, 0.0, 0.0]),
                crate::TrainingSamplerPromptFeature::new("berlin", vec![0.0, 1.0, 0.0, 0.0]),
            ],
            policy: TrainingSamplerServicePolicy {
                max_prompt_chars: 256,
                max_completion_tokens: 8,
                max_logprob_tokens: 8,
                max_top_logprobs: 3,
                freshness_budget_ms: 10_000,
                stop_token_id: None,
            },
        }
    }

    fn current_revision(
        backend: &OpenAdapterTrainingExecutionBackend,
    ) -> Result<TrainingSamplerServedRevision, Box<dyn std::error::Error>> {
        let outcome = crate::run_open_adapter_sft_export(
            backend,
            &OpenAdapterSftRunRequest {
                dataset_ref: "dataset://weather/live-rl-reference@2026.04".to_string(),
                validator_policy_ref: "policy://validator/weather/live-rl".to_string(),
                adapter_id: "weather-live-rl".to_string(),
                adapter_revision: "r1".to_string(),
                started_at_ms: 1_000,
                step_duration_ms: 10,
            },
        )?;
        Ok(TrainingSamplerServedRevision::new(
            PolicyRevision::new(
                "open.adapter.policy",
                "policy-r1",
                outcome.summary.adapter_artifact_digest.clone(),
                1_200,
            )
            .with_revision_number(1)
            .with_checkpoint(TrainingCheckpointReference::new(
                "open.adapter.policy",
                "stream://policy-r1",
                "manifest://policy-r1",
                "object://policy-r1",
                "trainer-a",
                1,
                "cluster-digest-r1",
                "topology-digest-r1",
                1_200,
            )),
            outcome.adapter_identity,
            outcome.summary.lora_alpha,
            outcome.adapter_bytes,
        ))
    }

    fn rollout_artifact(
        artifact_id: &str,
        worker_id: &str,
        source_policy_revision: PolicyRevision,
        token_ids: &[u32],
        logprobs: &[f32],
        rewards: &[f32],
        advantages: &[f32],
        created_at_ms: u64,
    ) -> Result<RolloutArtifact, Box<dyn std::error::Error>> {
        let samples = token_ids
            .iter()
            .copied()
            .zip(logprobs.iter().copied())
            .zip(rewards.iter().copied())
            .zip(advantages.iter().copied())
            .map(|(((token_id, logprob), reward), advantage)| {
                RolloutSample::new(token_id, logprob, reward, advantage)
            })
            .collect::<Vec<_>>();
        Ok(RolloutArtifact::new(
            artifact_id,
            worker_id,
            EnvironmentPackageKey::new("oa.weather.agent", "2026.03"),
            format!("task://{artifact_id}"),
            source_policy_revision,
            samples,
            RolloutTerminationReason::Completed,
            vec![RolloutProofReference::new(
                RolloutProofKind::ExecutionProof,
                format!("proof-{artifact_id}"),
                format!("exec://{artifact_id}"),
            )],
            created_at_ms,
        )?)
    }

    fn accepted_record(
        artifact: &RolloutArtifact,
        outcome: RolloutReceiptOutcome,
        target_policy_revision_id: &str,
        observed_at_ms: u64,
    ) -> AcceptedRolloutRecord {
        AcceptedRolloutRecord {
            receipt: RolloutAdmissionReceipt {
                receipt_id: format!("receipt-{}", artifact.artifact_id),
                run_id: "run-live-rl".to_string(),
                stage_id: "stage-rl".to_string(),
                window_id: "window-live-rl".to_string(),
                artifact_id: artifact.artifact_id.clone(),
                artifact_digest: artifact.artifact_digest.clone(),
                worker_id: artifact.worker_id.clone(),
                environment_key: artifact.environment.storage_key(),
                target_policy_revision_id: target_policy_revision_id.to_string(),
                source_policy_revision_id: artifact.source_policy_revision.revision_id.clone(),
                source_policy_digest: artifact.source_policy_revision.policy_digest.clone(),
                outcome,
                revision_drift: artifact
                    .source_policy_revision
                    .revision_number
                    .zip(Some(1_u64))
                    .map(|(source, target)| target.saturating_sub(source)),
                policy_age_ms: Some(
                    observed_at_ms.saturating_sub(artifact.source_policy_revision.produced_at_ms),
                ),
                rollout_age_ms: observed_at_ms.saturating_sub(artifact.created_at_ms),
                signals: Vec::new(),
                token_count: artifact.token_count(),
                reward_sum: artifact.reward_sum(),
                termination_reason: artifact.termination_reason,
                observed_at_ms,
                receipt_digest: format!("receipt-digest-{}", artifact.artifact_id),
            },
            reference: RolloutArtifactRef {
                artifact_id: artifact.artifact_id.clone(),
                artifact_digest: artifact.artifact_digest.clone(),
                worker_id: artifact.worker_id.clone(),
                policy_revision_id: artifact.source_policy_revision.revision_id.clone(),
                task_id: artifact.task_id.clone(),
                token_count: artifact.token_count(),
                proof_digests: artifact
                    .proof_references
                    .iter()
                    .map(|proof| proof.digest.clone())
                    .collect(),
                reference_digest: format!("reference-digest-{}", artifact.artifact_id),
            },
            artifact: artifact.clone(),
        }
    }

    #[test]
    fn live_rl_update_materializes_rollouts_and_promotes_new_revision(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let backend =
            OpenAdapterTrainingExecutionBackend::new(backend_config(), backend_samples())?;
        let sampler_config = sampler_config();
        let current_revision = current_revision(&backend)?;
        let previous_revision = PolicyRevision::new(
            "open.adapter.policy",
            "policy-r0",
            "policy-digest-r0",
            1_100,
        )
        .with_revision_number(0);
        let rollout_exact = rollout_artifact(
            "rollout-exact",
            "worker-a",
            current_revision.policy_revision.clone(),
            &[2, 2],
            &[-0.25, -0.21],
            &[0.9, 0.8],
            &[0.7, 0.6],
            1_250,
        )?;
        let rollout_off_policy = rollout_artifact(
            "rollout-off-policy",
            "worker-b",
            previous_revision,
            &[3],
            &[-0.35],
            &[0.4],
            &[0.2],
            1_251,
        )?;
        let batch = TrainerBatch::assemble(
            "trainer-batch-live-rl",
            current_revision.policy_revision.clone(),
            vec![rollout_exact.clone(), rollout_off_policy.clone()],
            1_300,
        )?;
        let batch_record = TrainingOrchestratorBatchRecord {
            request: TrainerBatchAssemblyRequest {
                batch_id: batch.batch_id.clone(),
                window_id: "window-live-rl".to_string(),
                contributor_set_revision_id: "contributors-r1".to_string(),
                policy_revision_id: current_revision.policy_revision.revision_id.clone(),
                rollout_ids: batch.rollout_ids.clone(),
                rollout_digests: batch.rollout_digests.clone(),
                policy_weight_broadcast_digest: "broadcast-r1".to_string(),
                request_digest: "request-digest-live-rl".to_string(),
            },
            batch: batch.clone(),
        };
        let window = TrainingOrchestratorWindow {
            window_id: "window-live-rl".to_string(),
            contributor_set_revision_id: "contributors-r1".to_string(),
            assignment_posture: crate::TrainingWindowAssignmentPosture {
                assignment_seed: 77,
                policy_revision_id: current_revision.policy_revision.revision_id.clone(),
                policy_weight_broadcast_digest: "broadcast-r1".to_string(),
                posture_digest: "posture-digest-r1".to_string(),
            },
            rollout_assignments: Vec::new(),
            eval_assignments: Vec::new(),
            accepted_rollouts: vec![
                accepted_record(
                    &rollout_exact,
                    RolloutReceiptOutcome::AcceptedExact,
                    current_revision.policy_revision.revision_id.as_str(),
                    1_305,
                ),
                accepted_record(
                    &rollout_off_policy,
                    RolloutReceiptOutcome::AcceptedOffPolicy,
                    current_revision.policy_revision.revision_id.as_str(),
                    1_305,
                ),
            ],
            quarantined_rollouts: Vec::new(),
            discarded_rollout_receipts: Vec::new(),
            rollout_telemetry: crate::RolloutIngestionTelemetry {
                accepted_exact_rollout_count: 1,
                accepted_off_policy_rollout_count: 1,
                quarantined_rollout_count: 0,
                discarded_rollout_count: 0,
                accepted_token_count: 3,
                quarantined_token_count: 0,
                discarded_token_count: 0,
            },
            trainer_batches: vec![batch_record.clone()],
        };
        let executor =
            OpenAdapterLiveRlUpdateExecutor::new(backend.clone(), sampler_config.clone())?;

        let mut sampler = TrainingSamplerService::new(sampler_config.clone())?;
        sampler.refresh_revision(current_revision.clone(), 1_310)?;
        let before = sampler.token_logprobs(&TrainingSamplerLogprobRequest {
            request_id: "before".to_string(),
            prompt: "Paris".to_string(),
            continuation_token_ids: vec![2],
            requested_revision_id: Some(current_revision.policy_revision.revision_id.clone()),
            top_logprobs: Some(1),
            requested_at_ms: 1_315,
        })?;

        let outcome = executor.run_update(&LiveRlUpdateRequest {
            update_id: "update-live-rl-1".to_string(),
            batch_record,
            window,
            rollout_inputs: vec![
                LiveRlRolloutInput::new("rollout-exact", "Paris")
                    .with_teacher_logprobs(vec![-0.05, -0.03]),
                LiveRlRolloutInput::new("rollout-off-policy", "Berlin"),
            ],
            current_revision: current_revision.clone(),
            promoted_policy_revision: PolicyRevision::new(
                "open.adapter.policy",
                "policy-r2",
                "placeholder",
                1_400,
            )
            .with_revision_number(2),
            export_request: OpenAdapterArtifactExportRequest::new(
                "dataset://weather/live-rl-reference@2026.04",
                "policy://validator/weather/live-rl",
                "weather-live-rl",
                "r2",
            ),
            policy: LiveRlUpdatePolicy::default(),
            started_at_ms: 1_320,
            finished_at_ms: 1_340,
        })?;

        assert_eq!(outcome.materialized_batch.exact_rollout_count, 1);
        assert_eq!(outcome.materialized_batch.off_policy_rollout_count, 1);
        assert_eq!(outcome.materialized_batch.completion_token_count, 3);
        assert_eq!(outcome.materialized_batch.teacher_token_count, 2);
        assert_eq!(
            outcome.promoted_revision.policy_revision.revision_id,
            "policy-r2"
        );
        assert_eq!(outcome.receipt.promoted_revision_number, Some(2));

        sampler.refresh_revision(outcome.promoted_revision.clone(), 1_350)?;
        let after = sampler.token_logprobs(&TrainingSamplerLogprobRequest {
            request_id: "after".to_string(),
            prompt: "Paris".to_string(),
            continuation_token_ids: vec![2],
            requested_revision_id: Some(String::from("policy-r2")),
            top_logprobs: Some(1),
            requested_at_ms: 1_360,
        })?;
        assert!(after.tokens[0].logprob > before.tokens[0].logprob);
        Ok(())
    }

    #[test]
    fn live_rl_update_refuses_teacher_logprob_length_mismatch(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let backend =
            OpenAdapterTrainingExecutionBackend::new(backend_config(), backend_samples())?;
        let current_revision = current_revision(&backend)?;
        let rollout = rollout_artifact(
            "rollout-exact",
            "worker-a",
            current_revision.policy_revision.clone(),
            &[2, 2],
            &[-0.25, -0.21],
            &[0.9, 0.8],
            &[0.7, 0.6],
            1_250,
        )?;
        let batch = TrainerBatch::assemble(
            "trainer-batch-live-rl",
            current_revision.policy_revision.clone(),
            vec![rollout.clone()],
            1_300,
        )?;
        let batch_record = TrainingOrchestratorBatchRecord {
            request: TrainerBatchAssemblyRequest {
                batch_id: batch.batch_id.clone(),
                window_id: "window-live-rl".to_string(),
                contributor_set_revision_id: "contributors-r1".to_string(),
                policy_revision_id: current_revision.policy_revision.revision_id.clone(),
                rollout_ids: batch.rollout_ids.clone(),
                rollout_digests: batch.rollout_digests.clone(),
                policy_weight_broadcast_digest: "broadcast-r1".to_string(),
                request_digest: "request-digest-live-rl".to_string(),
            },
            batch,
        };
        let window = TrainingOrchestratorWindow {
            window_id: "window-live-rl".to_string(),
            contributor_set_revision_id: "contributors-r1".to_string(),
            assignment_posture: crate::TrainingWindowAssignmentPosture {
                assignment_seed: 77,
                policy_revision_id: current_revision.policy_revision.revision_id.clone(),
                policy_weight_broadcast_digest: "broadcast-r1".to_string(),
                posture_digest: "posture-digest-r1".to_string(),
            },
            rollout_assignments: Vec::new(),
            eval_assignments: Vec::new(),
            accepted_rollouts: vec![accepted_record(
                &rollout,
                RolloutReceiptOutcome::AcceptedExact,
                current_revision.policy_revision.revision_id.as_str(),
                1_305,
            )],
            quarantined_rollouts: Vec::new(),
            discarded_rollout_receipts: Vec::new(),
            rollout_telemetry: crate::RolloutIngestionTelemetry::default(),
            trainer_batches: Vec::new(),
        };
        let executor = OpenAdapterLiveRlUpdateExecutor::new(backend, sampler_config())?;
        let error = executor
            .run_update(&LiveRlUpdateRequest {
                update_id: "update-live-rl-1".to_string(),
                batch_record,
                window,
                rollout_inputs: vec![LiveRlRolloutInput::new("rollout-exact", "Paris")
                    .with_teacher_logprobs(vec![-0.05])],
                current_revision,
                promoted_policy_revision: PolicyRevision::new(
                    "open.adapter.policy",
                    "policy-r2",
                    "placeholder",
                    1_400,
                )
                .with_revision_number(2),
                export_request: OpenAdapterArtifactExportRequest::new(
                    "dataset://weather/live-rl-reference@2026.04",
                    "policy://validator/weather/live-rl",
                    "weather-live-rl",
                    "r2",
                ),
                policy: LiveRlUpdatePolicy::default(),
                started_at_ms: 1_320,
                finished_at_ms: 1_340,
            })
            .expect_err("teacher logprob length mismatch should fail");

        assert!(matches!(
            error,
            LiveRlUpdateError::TeacherLogprobCountMismatch {
                artifact_id,
                expected: 2,
                actual: 1,
            } if artifact_id == "rollout-exact"
        ));
        Ok(())
    }
}
