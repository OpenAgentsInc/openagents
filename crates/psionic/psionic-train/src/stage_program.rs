use std::time::{SystemTime, UNIX_EPOCH};

use psionic_environments::EnvironmentPackageKey;
use psionic_runtime::TrainingCheckpointReference;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// One canonical training stage in the multi-stage train program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingStageKind {
    /// Broad supervised fine-tuning over ordinary completion or long-context traces.
    GeneralSft,
    /// Agentic SFT over tool-call and long-context traces.
    AgenticSft,
    /// Reinforcement learning stage over rollout artifacts.
    Rl,
}

impl TrainingStageKind {
    /// Returns the only valid next stage, when one exists.
    #[must_use]
    pub const fn next_kind(self) -> Option<Self> {
        match self {
            Self::GeneralSft => Some(Self::AgenticSft),
            Self::AgenticSft => Some(Self::Rl),
            Self::Rl => None,
        }
    }
}

/// Current lifecycle status of one stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingStageStatus {
    /// Actively ingesting traces or producing rollouts.
    Active,
    /// Completed and ready for checkpoint promotion.
    Completed,
}

/// Typed SFT-trace family used by the stage program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingSftTraceKind {
    /// Ordinary supervised completion trace.
    PlainCompletion,
    /// Tool-call or tool-result trace.
    ToolCall,
    /// Long-context trace with explicit segment lineage.
    LongContext,
}

/// One tool-call digest inside an agentic trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingToolCallTraceStep {
    /// Stable tool name.
    pub tool_name: String,
    /// Stable digest over tool arguments.
    pub arguments_digest: String,
    /// Stable digest over tool results.
    pub result_digest: String,
}

/// Typed tool-call lineage for one agentic trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingToolCallTraceLineage {
    /// Ordered tool-call steps.
    pub steps: Vec<TrainingToolCallTraceStep>,
    /// Stable digest over the tool-call lineage.
    pub tool_trace_digest: String,
}

impl TrainingToolCallTraceLineage {
    /// Creates one tool-call lineage from stable digests.
    #[must_use]
    pub fn new(steps: Vec<TrainingToolCallTraceStep>) -> Self {
        let tool_trace_digest = stable_tool_trace_digest(steps.as_slice());
        Self {
            steps,
            tool_trace_digest,
        }
    }
}

/// Typed long-context lineage for one SFT trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingLongContextTraceLineage {
    /// Number of context tokens visible to the trace.
    pub context_window_tokens: u32,
    /// Stable digests over the ordered source segments.
    pub segment_digests: Vec<String>,
    /// Stable digest over the long-context lineage.
    pub long_context_digest: String,
}

impl TrainingLongContextTraceLineage {
    /// Creates one long-context lineage.
    #[must_use]
    pub fn new(context_window_tokens: u32, segment_digests: Vec<String>) -> Self {
        let long_context_digest =
            stable_long_context_digest(context_window_tokens, segment_digests.as_slice());
        Self {
            context_window_tokens,
            segment_digests,
            long_context_digest,
        }
    }
}

/// Typed SFT-trace artifact admitted by the multi-stage train program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingSftTraceArtifact {
    /// Stable trace identifier.
    pub trace_id: String,
    /// Environment package used to produce or score the trace.
    pub environment: EnvironmentPackageKey,
    /// Trace family.
    pub trace_kind: TrainingSftTraceKind,
    /// Stable digest over the prompt or trace input.
    pub input_digest: String,
    /// Stable digest over the target or assistant output.
    pub output_digest: String,
    /// Optional stable session summary digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_digest: Option<String>,
    /// Optional stable upstream dataset or task reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
    /// Optional tool-call lineage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_lineage: Option<TrainingToolCallTraceLineage>,
    /// Optional long-context lineage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_context_lineage: Option<TrainingLongContextTraceLineage>,
    /// Stable digest over the full trace lineage.
    pub lineage_digest: String,
}

impl TrainingSftTraceArtifact {
    /// Creates one typed SFT-trace artifact.
    #[must_use]
    pub fn new(
        trace_id: impl Into<String>,
        environment: EnvironmentPackageKey,
        trace_kind: TrainingSftTraceKind,
        input_digest: impl Into<String>,
        output_digest: impl Into<String>,
    ) -> Self {
        let mut trace = Self {
            trace_id: trace_id.into(),
            environment,
            trace_kind,
            input_digest: input_digest.into(),
            output_digest: output_digest.into(),
            session_digest: None,
            source_ref: None,
            tool_call_lineage: None,
            long_context_lineage: None,
            lineage_digest: String::new(),
        };
        trace.refresh_lineage_digest();
        trace
    }

    /// Attaches an upstream session digest.
    #[must_use]
    pub fn with_session_digest(mut self, session_digest: impl Into<String>) -> Self {
        self.session_digest = Some(session_digest.into());
        self.refresh_lineage_digest();
        self
    }

    /// Attaches an upstream source ref.
    #[must_use]
    pub fn with_source_ref(mut self, source_ref: impl Into<String>) -> Self {
        self.source_ref = Some(source_ref.into());
        self.refresh_lineage_digest();
        self
    }

    /// Attaches tool-call lineage.
    #[must_use]
    pub fn with_tool_call_lineage(mut self, lineage: TrainingToolCallTraceLineage) -> Self {
        self.tool_call_lineage = Some(lineage);
        self.refresh_lineage_digest();
        self
    }

    /// Attaches long-context lineage.
    #[must_use]
    pub fn with_long_context_lineage(mut self, lineage: TrainingLongContextTraceLineage) -> Self {
        self.long_context_lineage = Some(lineage);
        self.refresh_lineage_digest();
        self
    }

    /// Validates the typed trace contract.
    pub fn validate(&self) -> Result<(), TrainingStageProgramError> {
        if self.trace_id.trim().is_empty() {
            return Err(TrainingStageProgramError::MissingTraceId);
        }
        if self.input_digest.trim().is_empty() {
            return Err(TrainingStageProgramError::MissingInputDigest {
                trace_id: self.trace_id.clone(),
            });
        }
        if self.output_digest.trim().is_empty() {
            return Err(TrainingStageProgramError::MissingOutputDigest {
                trace_id: self.trace_id.clone(),
            });
        }
        match self.trace_kind {
            TrainingSftTraceKind::PlainCompletion => {
                if self.tool_call_lineage.is_some() {
                    return Err(TrainingStageProgramError::UnexpectedToolCallLineage {
                        trace_id: self.trace_id.clone(),
                    });
                }
            }
            TrainingSftTraceKind::ToolCall => {
                let Some(lineage) = &self.tool_call_lineage else {
                    return Err(TrainingStageProgramError::MissingToolCallLineage {
                        trace_id: self.trace_id.clone(),
                    });
                };
                if lineage.steps.is_empty() {
                    return Err(TrainingStageProgramError::EmptyToolCallLineage {
                        trace_id: self.trace_id.clone(),
                    });
                }
            }
            TrainingSftTraceKind::LongContext => {
                let Some(lineage) = &self.long_context_lineage else {
                    return Err(TrainingStageProgramError::MissingLongContextLineage {
                        trace_id: self.trace_id.clone(),
                    });
                };
                if lineage.context_window_tokens == 0 || lineage.segment_digests.is_empty() {
                    return Err(TrainingStageProgramError::InvalidLongContextLineage {
                        trace_id: self.trace_id.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    fn refresh_lineage_digest(&mut self) {
        self.lineage_digest = stable_trace_lineage_digest(self);
    }
}

/// Trace-ingestion receipt emitted by the stage program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingSftTraceIngestionReceipt {
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Trace id that was accepted.
    pub trace_id: String,
    /// Trace family.
    pub trace_kind: TrainingSftTraceKind,
    /// Stable lineage digest.
    pub lineage_digest: String,
    /// Logical acceptance timestamp.
    pub accepted_at_ms: u64,
}

/// Completion receipt for one stage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingStageCompletionReceipt {
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Stage kind.
    pub kind: TrainingStageKind,
    /// Number of accepted traces in the stage.
    pub ingested_trace_count: u32,
    /// Logical completion timestamp.
    pub completed_at_ms: u64,
    /// Stable digest over the completion record.
    pub completion_digest: String,
}

/// Checkpoint-promotion receipt between stages.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingCheckpointPromotionReceipt {
    /// Stable run id.
    pub run_id: String,
    /// Stage id that produced the checkpoint.
    pub from_stage_id: String,
    /// Current stage kind.
    pub from_stage_kind: TrainingStageKind,
    /// Next stage kind.
    pub to_stage_kind: TrainingStageKind,
    /// Promoted checkpoint reference.
    pub checkpoint: TrainingCheckpointReference,
    /// Stable promotion digest.
    pub promotion_digest: String,
}

/// Transition receipt for entering one stage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingStageTransitionReceipt {
    /// Stable run id.
    pub run_id: String,
    /// Previous stage id when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_stage_id: Option<String>,
    /// New stage id.
    pub next_stage_id: String,
    /// New stage kind.
    pub next_stage_kind: TrainingStageKind,
    /// Base checkpoint used to enter the stage when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_checkpoint: Option<TrainingCheckpointReference>,
    /// Logical transition timestamp.
    pub transitioned_at_ms: u64,
    /// Stable transition digest.
    pub transition_digest: String,
}

/// Compound receipt for a checkpoint promotion plus stage entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingStageAdvanceReceipt {
    /// Checkpoint promotion proving continuity between stages.
    pub promotion: TrainingCheckpointPromotionReceipt,
    /// Transition into the new stage.
    pub transition: TrainingStageTransitionReceipt,
}

/// Persistent state for one stage in the train program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingStageState {
    /// Stable stage id.
    pub stage_id: String,
    /// Stage kind.
    pub kind: TrainingStageKind,
    /// Environment package bound to the stage.
    pub environment: EnvironmentPackageKey,
    /// Current stage status.
    pub status: TrainingStageStatus,
    /// Base checkpoint used to enter the stage when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_checkpoint: Option<TrainingCheckpointReference>,
    /// Accepted trace ingestion receipts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ingested_traces: Vec<TrainingSftTraceIngestionReceipt>,
}

/// Error returned by the multi-stage train program.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TrainingStageProgramError {
    /// The run id is missing.
    #[error("training stage program is missing `run_id`")]
    MissingRunId,
    /// The checkpoint family is missing.
    #[error("training stage program is missing `checkpoint_family`")]
    MissingCheckpointFamily,
    /// The trace id is missing.
    #[error("training stage trace is missing `trace_id`")]
    MissingTraceId,
    /// The trace omitted the input digest.
    #[error("training stage trace `{trace_id}` is missing `input_digest`")]
    MissingInputDigest { trace_id: String },
    /// The trace omitted the output digest.
    #[error("training stage trace `{trace_id}` is missing `output_digest`")]
    MissingOutputDigest { trace_id: String },
    /// Tool-call lineage is required for the trace kind.
    #[error("training stage trace `{trace_id}` requires tool-call lineage")]
    MissingToolCallLineage { trace_id: String },
    /// Tool-call lineage was empty.
    #[error("training stage trace `{trace_id}` has empty tool-call lineage")]
    EmptyToolCallLineage { trace_id: String },
    /// Tool-call lineage was attached to a non-tool-call trace.
    #[error("training stage trace `{trace_id}` attached unexpected tool-call lineage")]
    UnexpectedToolCallLineage { trace_id: String },
    /// Long-context lineage is required.
    #[error("training stage trace `{trace_id}` requires long-context lineage")]
    MissingLongContextLineage { trace_id: String },
    /// Long-context lineage was malformed.
    #[error("training stage trace `{trace_id}` has invalid long-context lineage")]
    InvalidLongContextLineage { trace_id: String },
    /// No active stage exists.
    #[error("training stage program has no active stage")]
    NoActiveStage,
    /// The stage order is invalid.
    #[error("training stage transition from `{from:?}` to `{to:?}` is not allowed")]
    InvalidStageOrder {
        from: TrainingStageKind,
        to: TrainingStageKind,
    },
    /// The initial stage must be general SFT.
    #[error("initial training stage must be `general_sft`, found `{kind:?}`")]
    InvalidInitialStage { kind: TrainingStageKind },
    /// A checkpoint is required to enter the stage.
    #[error("training stage `{kind:?}` requires a promoted checkpoint")]
    MissingBaseCheckpoint { kind: TrainingStageKind },
    /// The checkpoint family does not match the program.
    #[error(
        "training stage program checkpoint family mismatch: expected `{expected}`, found `{actual}`"
    )]
    CheckpointFamilyMismatch { expected: String, actual: String },
    /// The current stage must be completed before advancing.
    #[error("training stage `{stage_id}` must be completed before it can advance")]
    ActiveStageNotCompleted { stage_id: String },
    /// The stage requires at least one trace before completion.
    #[error("training stage `{stage_id}` requires at least one ingested trace")]
    StageRequiresTrace { stage_id: String },
    /// The trace kind is not allowed in the current stage.
    #[error("training stage `{stage_id}` does not admit trace kind `{trace_kind:?}`")]
    TraceKindNotAllowed {
        stage_id: String,
        trace_kind: TrainingSftTraceKind,
    },
}

/// Persistent multi-stage train-program state over SFT, agentic SFT, and RL.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingStageProgramState {
    /// Stable run id.
    pub run_id: String,
    /// Checkpoint family shared across stage promotions.
    pub checkpoint_family: String,
    /// Stage history in order.
    pub stages: Vec<TrainingStageState>,
    /// Completion receipts for completed stages.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub completions: Vec<TrainingStageCompletionReceipt>,
    /// Checkpoint promotions between stages.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub promotions: Vec<TrainingCheckpointPromotionReceipt>,
    /// Transition receipts into stages.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<TrainingStageTransitionReceipt>,
}

impl TrainingStageProgramState {
    /// Creates one empty stage program.
    pub fn new(
        run_id: impl Into<String>,
        checkpoint_family: impl Into<String>,
    ) -> Result<Self, TrainingStageProgramError> {
        let run_id = run_id.into();
        if run_id.trim().is_empty() {
            return Err(TrainingStageProgramError::MissingRunId);
        }
        let checkpoint_family = checkpoint_family.into();
        if checkpoint_family.trim().is_empty() {
            return Err(TrainingStageProgramError::MissingCheckpointFamily);
        }
        Ok(Self {
            run_id,
            checkpoint_family,
            stages: Vec::new(),
            completions: Vec::new(),
            promotions: Vec::new(),
            transitions: Vec::new(),
        })
    }

    /// Starts the first `general_sft` stage.
    pub fn start_initial_stage(
        &mut self,
        environment: EnvironmentPackageKey,
    ) -> Result<TrainingStageTransitionReceipt, TrainingStageProgramError> {
        self.start_stage(TrainingStageKind::GeneralSft, environment, None)
    }

    /// Ingests one typed SFT trace into the active stage.
    pub fn ingest_trace(
        &mut self,
        trace: &TrainingSftTraceArtifact,
    ) -> Result<TrainingSftTraceIngestionReceipt, TrainingStageProgramError> {
        trace.validate()?;
        let stage = self
            .stages
            .last_mut()
            .ok_or(TrainingStageProgramError::NoActiveStage)?;
        if stage.status != TrainingStageStatus::Active {
            return Err(TrainingStageProgramError::NoActiveStage);
        }
        if !stage_kind_allows_trace(stage.kind, trace.trace_kind) {
            return Err(TrainingStageProgramError::TraceKindNotAllowed {
                stage_id: stage.stage_id.clone(),
                trace_kind: trace.trace_kind,
            });
        }
        let receipt = TrainingSftTraceIngestionReceipt {
            run_id: self.run_id.clone(),
            stage_id: stage.stage_id.clone(),
            trace_id: trace.trace_id.clone(),
            trace_kind: trace.trace_kind,
            lineage_digest: trace.lineage_digest.clone(),
            accepted_at_ms: now_epoch_ms(),
        };
        stage.ingested_traces.push(receipt.clone());
        Ok(receipt)
    }

    /// Completes the active stage.
    pub fn complete_current_stage(
        &mut self,
    ) -> Result<TrainingStageCompletionReceipt, TrainingStageProgramError> {
        let stage = self
            .stages
            .last_mut()
            .ok_or(TrainingStageProgramError::NoActiveStage)?;
        if stage.status != TrainingStageStatus::Active {
            return Err(TrainingStageProgramError::NoActiveStage);
        }
        if matches!(
            stage.kind,
            TrainingStageKind::GeneralSft | TrainingStageKind::AgenticSft
        ) && stage.ingested_traces.is_empty()
        {
            return Err(TrainingStageProgramError::StageRequiresTrace {
                stage_id: stage.stage_id.clone(),
            });
        }
        stage.status = TrainingStageStatus::Completed;
        let completed_at_ms = now_epoch_ms();
        let completion_receipt = TrainingStageCompletionReceipt {
            run_id: self.run_id.clone(),
            stage_id: stage.stage_id.clone(),
            kind: stage.kind,
            ingested_trace_count: stage.ingested_traces.len() as u32,
            completed_at_ms,
            completion_digest: stable_stage_completion_digest(
                self.run_id.as_str(),
                stage.stage_id.as_str(),
                stage.kind,
                stage.ingested_traces.len() as u32,
                completed_at_ms,
            ),
        };
        self.completions.push(completion_receipt.clone());
        Ok(completion_receipt)
    }

    /// Promotes the completed stage checkpoint into the next stage and opens it.
    pub fn advance_stage(
        &mut self,
        next_kind: TrainingStageKind,
        environment: EnvironmentPackageKey,
        checkpoint: TrainingCheckpointReference,
    ) -> Result<TrainingStageAdvanceReceipt, TrainingStageProgramError> {
        let previous_stage = self
            .stages
            .last()
            .cloned()
            .ok_or(TrainingStageProgramError::NoActiveStage)?;
        if previous_stage.status != TrainingStageStatus::Completed {
            return Err(TrainingStageProgramError::ActiveStageNotCompleted {
                stage_id: previous_stage.stage_id,
            });
        }
        let expected_next = previous_stage.kind.next_kind().ok_or(
            TrainingStageProgramError::InvalidStageOrder {
                from: previous_stage.kind,
                to: next_kind,
            },
        )?;
        if expected_next != next_kind {
            return Err(TrainingStageProgramError::InvalidStageOrder {
                from: previous_stage.kind,
                to: next_kind,
            });
        }
        if checkpoint.checkpoint_family != self.checkpoint_family {
            return Err(TrainingStageProgramError::CheckpointFamilyMismatch {
                expected: self.checkpoint_family.clone(),
                actual: checkpoint.checkpoint_family,
            });
        }
        let promotion = TrainingCheckpointPromotionReceipt {
            run_id: self.run_id.clone(),
            from_stage_id: previous_stage.stage_id.clone(),
            from_stage_kind: previous_stage.kind,
            to_stage_kind: next_kind,
            promotion_digest: stable_stage_promotion_digest(
                self.run_id.as_str(),
                previous_stage.stage_id.as_str(),
                previous_stage.kind,
                next_kind,
                &checkpoint,
            ),
            checkpoint: checkpoint.clone(),
        };
        self.promotions.push(promotion.clone());
        let transition = self.start_stage(next_kind, environment, Some(checkpoint))?;
        Ok(TrainingStageAdvanceReceipt {
            promotion,
            transition,
        })
    }

    /// Returns the active stage when one exists.
    #[must_use]
    pub fn current_stage(&self) -> Option<&TrainingStageState> {
        self.stages.last()
    }

    fn start_stage(
        &mut self,
        kind: TrainingStageKind,
        environment: EnvironmentPackageKey,
        base_checkpoint: Option<TrainingCheckpointReference>,
    ) -> Result<TrainingStageTransitionReceipt, TrainingStageProgramError> {
        let previous_stage = self.stages.last().cloned();
        match previous_stage.as_ref() {
            None => {
                if kind != TrainingStageKind::GeneralSft {
                    return Err(TrainingStageProgramError::InvalidInitialStage { kind });
                }
            }
            Some(previous_stage) => {
                if previous_stage.kind.next_kind() != Some(kind) {
                    return Err(TrainingStageProgramError::InvalidStageOrder {
                        from: previous_stage.kind,
                        to: kind,
                    });
                }
                let Some(checkpoint) = base_checkpoint.as_ref() else {
                    return Err(TrainingStageProgramError::MissingBaseCheckpoint { kind });
                };
                if checkpoint.checkpoint_family != self.checkpoint_family {
                    return Err(TrainingStageProgramError::CheckpointFamilyMismatch {
                        expected: self.checkpoint_family.clone(),
                        actual: checkpoint.checkpoint_family.clone(),
                    });
                }
            }
        }

        let stage_id = format!(
            "{}-stage-{}-{}",
            self.run_id,
            self.stages.len().saturating_add(1),
            training_stage_kind_label(kind)
        );
        let transitioned_at_ms = now_epoch_ms();
        let transition = TrainingStageTransitionReceipt {
            run_id: self.run_id.clone(),
            previous_stage_id: previous_stage.as_ref().map(|stage| stage.stage_id.clone()),
            next_stage_id: stage_id.clone(),
            next_stage_kind: kind,
            base_checkpoint: base_checkpoint.clone(),
            transitioned_at_ms,
            transition_digest: stable_stage_transition_digest(
                self.run_id.as_str(),
                previous_stage.as_ref().map(|stage| stage.stage_id.as_str()),
                stage_id.as_str(),
                kind,
                base_checkpoint.as_ref(),
                transitioned_at_ms,
            ),
        };
        self.stages.push(TrainingStageState {
            stage_id,
            kind,
            environment,
            status: TrainingStageStatus::Active,
            base_checkpoint,
            ingested_traces: Vec::new(),
        });
        self.transitions.push(transition.clone());
        Ok(transition)
    }
}

fn training_stage_kind_label(kind: TrainingStageKind) -> &'static str {
    match kind {
        TrainingStageKind::GeneralSft => "general_sft",
        TrainingStageKind::AgenticSft => "agentic_sft",
        TrainingStageKind::Rl => "rl",
    }
}

fn stage_kind_allows_trace(
    stage_kind: TrainingStageKind,
    trace_kind: TrainingSftTraceKind,
) -> bool {
    match stage_kind {
        TrainingStageKind::GeneralSft => matches!(
            trace_kind,
            TrainingSftTraceKind::PlainCompletion | TrainingSftTraceKind::LongContext
        ),
        TrainingStageKind::AgenticSft => matches!(
            trace_kind,
            TrainingSftTraceKind::ToolCall | TrainingSftTraceKind::LongContext
        ),
        TrainingStageKind::Rl => false,
    }
}

fn stable_tool_trace_digest(steps: &[TrainingToolCallTraceStep]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_tool_trace|");
    for step in steps {
        hasher.update(step.tool_name.as_bytes());
        hasher.update(b"|");
        hasher.update(step.arguments_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(step.result_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_long_context_digest(context_window_tokens: u32, segment_digests: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_long_context|");
    hasher.update(context_window_tokens.to_string().as_bytes());
    for segment_digest in segment_digests {
        hasher.update(b"|");
        hasher.update(segment_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_trace_lineage_digest(trace: &TrainingSftTraceArtifact) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_sft_trace|");
    hasher.update(trace.trace_id.as_bytes());
    hasher.update(b"|");
    hasher.update(trace.environment.storage_key().as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", trace.trace_kind).as_bytes());
    hasher.update(b"|");
    hasher.update(trace.input_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(trace.output_digest.as_bytes());
    if let Some(session_digest) = &trace.session_digest {
        hasher.update(b"|session|");
        hasher.update(session_digest.as_bytes());
    }
    if let Some(source_ref) = &trace.source_ref {
        hasher.update(b"|source|");
        hasher.update(source_ref.as_bytes());
    }
    if let Some(tool_call_lineage) = &trace.tool_call_lineage {
        hasher.update(b"|tool|");
        hasher.update(tool_call_lineage.tool_trace_digest.as_bytes());
    }
    if let Some(long_context_lineage) = &trace.long_context_lineage {
        hasher.update(b"|long|");
        hasher.update(long_context_lineage.long_context_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_stage_completion_digest(
    run_id: &str,
    stage_id: &str,
    kind: TrainingStageKind,
    ingested_trace_count: u32,
    completed_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_stage_completion|");
    hasher.update(run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(stage_id.as_bytes());
    hasher.update(b"|");
    hasher.update(training_stage_kind_label(kind).as_bytes());
    hasher.update(b"|");
    hasher.update(ingested_trace_count.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(completed_at_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_stage_promotion_digest(
    run_id: &str,
    from_stage_id: &str,
    from_stage_kind: TrainingStageKind,
    to_stage_kind: TrainingStageKind,
    checkpoint: &TrainingCheckpointReference,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_stage_promotion|");
    hasher.update(run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(from_stage_id.as_bytes());
    hasher.update(b"|");
    hasher.update(training_stage_kind_label(from_stage_kind).as_bytes());
    hasher.update(b"|");
    hasher.update(training_stage_kind_label(to_stage_kind).as_bytes());
    hasher.update(b"|");
    hasher.update(checkpoint.checkpoint_family.as_bytes());
    hasher.update(b"|");
    hasher.update(checkpoint.stream_id.as_bytes());
    hasher.update(b"|");
    hasher.update(checkpoint.object_digest.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_stage_transition_digest(
    run_id: &str,
    previous_stage_id: Option<&str>,
    next_stage_id: &str,
    next_stage_kind: TrainingStageKind,
    base_checkpoint: Option<&TrainingCheckpointReference>,
    transitioned_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_stage_transition|");
    hasher.update(run_id.as_bytes());
    if let Some(previous_stage_id) = previous_stage_id {
        hasher.update(b"|previous|");
        hasher.update(previous_stage_id.as_bytes());
    }
    hasher.update(b"|next|");
    hasher.update(next_stage_id.as_bytes());
    hasher.update(b"|");
    hasher.update(training_stage_kind_label(next_stage_kind).as_bytes());
    if let Some(base_checkpoint) = base_checkpoint {
        hasher.update(b"|checkpoint|");
        hasher.update(base_checkpoint.stream_id.as_bytes());
        hasher.update(b"|");
        hasher.update(base_checkpoint.object_digest.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(transitioned_at_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::{
        TrainingLongContextTraceLineage, TrainingSftTraceArtifact, TrainingSftTraceKind,
        TrainingStageKind, TrainingStageProgramError, TrainingStageProgramState,
        TrainingToolCallTraceLineage, TrainingToolCallTraceStep,
    };
    use psionic_environments::EnvironmentPackageKey;
    use psionic_runtime::TrainingCheckpointReference;

    fn checkpoint(step: u64) -> TrainingCheckpointReference {
        TrainingCheckpointReference::new(
            "train.weather",
            format!("stream-{step}"),
            format!("manifest-{step}"),
            format!("object-{step}"),
            "node-a",
            1,
            "cluster-digest",
            "topology-digest",
            1_000 + step,
        )
        .with_checkpoint_ref(format!("checkpoint://{step}"))
        .with_step(step)
        .with_durable_at_ms(2_000 + step)
    }

    #[test]
    fn stage_program_advances_from_general_sft_to_agentic_sft_to_rl(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let environment = EnvironmentPackageKey::new("env.weather", "2026.03.14");
        let mut program = TrainingStageProgramState::new("run-weather", "train.weather")?;
        let initial = program.start_initial_stage(environment.clone())?;
        assert_eq!(initial.next_stage_kind, TrainingStageKind::GeneralSft);

        let plain_trace = TrainingSftTraceArtifact::new(
            "trace-plain",
            environment.clone(),
            TrainingSftTraceKind::PlainCompletion,
            "input-a",
            "output-a",
        )
        .with_session_digest("session-a");
        let long_context_trace = TrainingSftTraceArtifact::new(
            "trace-long",
            environment.clone(),
            TrainingSftTraceKind::LongContext,
            "input-b",
            "output-b",
        )
        .with_long_context_lineage(TrainingLongContextTraceLineage::new(
            32_768,
            vec![String::from("segment-1"), String::from("segment-2")],
        ));
        program.ingest_trace(&plain_trace)?;
        program.ingest_trace(&long_context_trace)?;
        let completed = program.complete_current_stage()?;
        assert_eq!(completed.ingested_trace_count, 2);

        let agentic = program.advance_stage(
            TrainingStageKind::AgenticSft,
            environment.clone(),
            checkpoint(100),
        )?;
        assert_eq!(
            agentic.promotion.to_stage_kind,
            TrainingStageKind::AgenticSft
        );
        assert_eq!(
            agentic.transition.next_stage_kind,
            TrainingStageKind::AgenticSft
        );

        let tool_trace = TrainingSftTraceArtifact::new(
            "trace-tool",
            environment.clone(),
            TrainingSftTraceKind::ToolCall,
            "input-c",
            "output-c",
        )
        .with_tool_call_lineage(TrainingToolCallTraceLineage::new(vec![
            TrainingToolCallTraceStep {
                tool_name: String::from("get_weather"),
                arguments_digest: String::from("args-a"),
                result_digest: String::from("result-a"),
            },
        ]))
        .with_source_ref("dataset://weather/tool-use");
        program.ingest_trace(&tool_trace)?;
        let completed = program.complete_current_stage()?;
        assert_eq!(completed.kind, TrainingStageKind::AgenticSft);

        let rl = program.advance_stage(TrainingStageKind::Rl, environment, checkpoint(200))?;
        assert_eq!(rl.transition.next_stage_kind, TrainingStageKind::Rl);
        assert_eq!(
            program.current_stage().map(|stage| stage.kind),
            Some(TrainingStageKind::Rl)
        );
        assert_eq!(program.promotions.len(), 2);
        Ok(())
    }

    #[test]
    fn stage_program_refuses_agentic_trace_before_agentic_stage(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let environment = EnvironmentPackageKey::new("env.weather", "2026.03.14");
        let mut program = TrainingStageProgramState::new("run-weather", "train.weather")?;
        program.start_initial_stage(environment.clone())?;
        let tool_trace = TrainingSftTraceArtifact::new(
            "trace-tool",
            environment,
            TrainingSftTraceKind::ToolCall,
            "input",
            "output",
        )
        .with_tool_call_lineage(TrainingToolCallTraceLineage::new(vec![
            TrainingToolCallTraceStep {
                tool_name: String::from("get_weather"),
                arguments_digest: String::from("args"),
                result_digest: String::from("result"),
            },
        ]));
        assert_eq!(
            program
                .ingest_trace(&tool_trace)
                .expect_err("general SFT should refuse tool-call traces"),
            TrainingStageProgramError::TraceKindNotAllowed {
                stage_id: String::from("run-weather-stage-1-general_sft"),
                trace_kind: TrainingSftTraceKind::ToolCall,
            }
        );
        Ok(())
    }

    #[test]
    fn stage_program_refuses_long_context_trace_without_lineage() {
        let trace = TrainingSftTraceArtifact::new(
            "trace-long",
            EnvironmentPackageKey::new("env.weather", "2026.03.14"),
            TrainingSftTraceKind::LongContext,
            "input",
            "output",
        );
        assert_eq!(
            trace
                .validate()
                .expect_err("long-context traces require lineage"),
            TrainingStageProgramError::MissingLongContextLineage {
                trace_id: String::from("trace-long"),
            }
        );
    }
}
