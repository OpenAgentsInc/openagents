use std::collections::BTreeMap;

use psionic_core::{DType, TensorData, TensorSpec};
use psionic_datastream::DatastreamManifestRef;
use psionic_runtime::TrainingCheckpointReference;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::TrainingSessionState;

/// Error returned by the fixed-budget training core.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TrainingCoreError {
    /// The loop budget carried an invalid zero-valued field.
    #[error("training loop budget field `{field}` must be greater than zero")]
    InvalidBudgetField {
        /// Stable field label.
        field: &'static str,
    },
    /// The run did not receive any parameter groups.
    #[error("training run requires at least one parameter group")]
    EmptyParameterGroups,
    /// Two parameter groups used the same identifier.
    #[error("training parameter group `{group_id}` was defined more than once")]
    DuplicateParameterGroup {
        /// Stable group identifier.
        group_id: String,
    },
    /// The tensor dtype is unsupported by the current reference path.
    #[error(
        "training tensor `{group_id}` must be `f32` for the current reference path; found `{dtype:?}`"
    )]
    UnsupportedTensorDType {
        /// Stable group identifier.
        group_id: String,
        /// Observed dtype.
        dtype: DType,
    },
    /// The provided tensor payload length mismatched the tensor specification.
    #[error(
        "training tensor `{group_id}` length mismatch: expected {expected_len}, found {actual_len}"
    )]
    TensorLengthMismatch {
        /// Stable group identifier.
        group_id: String,
        /// Expected tensor payload length.
        expected_len: usize,
        /// Actual payload length.
        actual_len: usize,
    },
    /// The step input omitted one required group gradient.
    #[error("training batch `{batch_id}` is missing gradient for group `{group_id}`")]
    MissingGradient {
        /// Stable batch identifier.
        batch_id: String,
        /// Stable group identifier.
        group_id: String,
    },
    /// The gradient tensor metadata mismatched the parameter tensor metadata.
    #[error(
        "training batch `{batch_id}` gradient tensor mismatch for group `{group_id}`: expected {expected:?}, found {actual:?}"
    )]
    GradientTensorMismatch {
        /// Stable batch identifier.
        batch_id: String,
        /// Stable group identifier.
        group_id: String,
        /// Expected tensor metadata.
        expected: TensorSpec,
        /// Actual tensor metadata.
        actual: TensorSpec,
    },
    /// The step timestamps were impossible.
    #[error(
        "training step timestamps are invalid: started_at_ms={started_at_ms}, finished_at_ms={finished_at_ms}"
    )]
    InvalidStepTiming {
        /// Requested step start.
        started_at_ms: u64,
        /// Requested step finish.
        finished_at_ms: u64,
    },
    /// The caller tried to execute beyond the fixed budget.
    #[error("training run `{run_id}` exhausted its fixed budget at step {max_steps}")]
    BudgetExhausted {
        /// Stable run identifier.
        run_id: String,
        /// Fixed maximum step count.
        max_steps: u64,
    },
    /// The caller requested a checkpoint restore when no durable checkpoint existed.
    #[error(
        "training session `{checkpoint_family}` has no durable checkpoint available for restore"
    )]
    CheckpointRestoreUnavailable {
        /// Stable checkpoint family.
        checkpoint_family: String,
    },
    /// The session had a durable checkpoint but did not keep its manifest reference.
    #[error(
        "training session `{checkpoint_family}` is missing the durable manifest required for restore"
    )]
    CheckpointManifestMissing {
        /// Stable checkpoint family.
        checkpoint_family: String,
    },
    /// The reusable optimizer layer refused one group update.
    #[error("training optimizer step failed for group `{group_id}`: {message}")]
    OptimizerStepFailed {
        /// Stable parameter-group identifier.
        group_id: String,
        /// Optimizer failure detail.
        message: String,
    },
}

/// Stable parameter-group family used by the training core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingParameterClass {
    /// Token or position embeddings.
    Embedding,
    /// Large matrix parameters such as feed-forward or attention projections.
    Matrix,
    /// Output heads or other classifier projections.
    Head,
    /// Small scalar control parameters.
    Scalar,
    /// Bias vectors or offsets.
    Bias,
}

/// Optimizer family owned by the reference training core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingOptimizerKind {
    /// Vanilla stochastic gradient descent with optional momentum.
    Sgd,
    /// Adam with coupled weight decay semantics.
    Adam,
    /// AdamW with decoupled weight decay.
    AdamW,
    /// Layer-wise adaptive rate scaling with optional momentum.
    Lars,
    /// Layer-wise adaptive moments.
    Lamb,
}

/// Typed optimizer configuration for one parameter group.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingOptimizerConfig {
    /// Optimizer family.
    pub kind: TrainingOptimizerKind,
    /// Base learning rate applied to this group.
    pub learning_rate: f32,
    /// Optional weight-decay coefficient.
    pub weight_decay: f32,
    /// Optional gradient-clipping threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gradient_clip_norm: Option<f32>,
    /// Optional SGD momentum coefficient.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub momentum: Option<f32>,
    /// AdamW beta1 coefficient.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beta1: Option<f32>,
    /// AdamW beta2 coefficient.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beta2: Option<f32>,
    /// AdamW epsilon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epsilon: Option<f32>,
    /// LARS/LAMB trust coefficient when the family uses one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_coefficient: Option<f32>,
}

impl TrainingOptimizerConfig {
    /// Creates an SGD optimizer config.
    #[must_use]
    pub fn sgd(learning_rate: f32) -> Self {
        Self {
            kind: TrainingOptimizerKind::Sgd,
            learning_rate,
            weight_decay: 0.0,
            gradient_clip_norm: None,
            momentum: None,
            beta1: None,
            beta2: None,
            epsilon: None,
            trust_coefficient: None,
        }
    }

    /// Creates an Adam optimizer config.
    #[must_use]
    pub fn adam(learning_rate: f32, beta1: f32, beta2: f32, epsilon: f32) -> Self {
        Self {
            kind: TrainingOptimizerKind::Adam,
            learning_rate,
            weight_decay: 0.0,
            gradient_clip_norm: None,
            momentum: None,
            beta1: Some(beta1),
            beta2: Some(beta2),
            epsilon: Some(epsilon),
            trust_coefficient: None,
        }
    }

    /// Creates an AdamW optimizer config.
    #[must_use]
    pub fn adamw(learning_rate: f32, beta1: f32, beta2: f32, epsilon: f32) -> Self {
        Self {
            kind: TrainingOptimizerKind::AdamW,
            learning_rate,
            weight_decay: 0.0,
            gradient_clip_norm: None,
            momentum: None,
            beta1: Some(beta1),
            beta2: Some(beta2),
            epsilon: Some(epsilon),
            trust_coefficient: None,
        }
    }

    /// Creates a LARS optimizer config.
    #[must_use]
    pub fn lars(learning_rate: f32, momentum: f32, trust_coefficient: f32, epsilon: f32) -> Self {
        Self {
            kind: TrainingOptimizerKind::Lars,
            learning_rate,
            weight_decay: 0.0,
            gradient_clip_norm: None,
            momentum: Some(momentum),
            beta1: None,
            beta2: None,
            epsilon: Some(epsilon),
            trust_coefficient: Some(trust_coefficient),
        }
    }

    /// Creates a LAMB optimizer config.
    #[must_use]
    pub fn lamb(learning_rate: f32, beta1: f32, beta2: f32, epsilon: f32) -> Self {
        Self {
            kind: TrainingOptimizerKind::Lamb,
            learning_rate,
            weight_decay: 0.0,
            gradient_clip_norm: None,
            momentum: None,
            beta1: Some(beta1),
            beta2: Some(beta2),
            epsilon: Some(epsilon),
            trust_coefficient: Some(1.0),
        }
    }

    /// Attaches weight decay.
    #[must_use]
    pub fn with_weight_decay(mut self, weight_decay: f32) -> Self {
        self.weight_decay = weight_decay;
        self
    }

    /// Attaches gradient clipping.
    #[must_use]
    pub fn with_gradient_clip_norm(mut self, gradient_clip_norm: f32) -> Self {
        self.gradient_clip_norm = Some(gradient_clip_norm);
        self
    }

    /// Attaches momentum for SGD.
    #[must_use]
    pub fn with_momentum(mut self, momentum: f32) -> Self {
        self.momentum = Some(momentum);
        self
    }

    /// Attaches a trust coefficient for LARS or LAMB style optimizers.
    #[must_use]
    pub fn with_trust_coefficient(mut self, trust_coefficient: f32) -> Self {
        self.trust_coefficient = Some(trust_coefficient);
        self
    }
}

/// Optimizer-state residency class for one parameter group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizerStateResidency {
    /// State is kept in host memory.
    HostResident,
    /// State is kept device-resident for the active execution path.
    DeviceResident,
    /// State is offloaded away from the active step path.
    Offloaded,
}

/// Reason for a residency transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizerResidencyTransitionReason {
    /// The group was restored from checkpoint state.
    RecoveryRestore,
    /// The group was prefetched into the active step posture.
    PrefetchForStep,
    /// The group was moved back to its idle posture after the step.
    OffloadAfterStep,
}

/// Preferred active-step versus idle residency for one parameter group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingOptimizerResidencyPolicy {
    /// Residency expected while a step is applying updates.
    pub step_residency: OptimizerStateResidency,
    /// Residency expected when the group is idle between steps.
    pub idle_residency: OptimizerStateResidency,
}

impl TrainingOptimizerResidencyPolicy {
    /// Creates an explicit residency policy.
    #[must_use]
    pub const fn new(
        step_residency: OptimizerStateResidency,
        idle_residency: OptimizerStateResidency,
    ) -> Self {
        Self {
            step_residency,
            idle_residency,
        }
    }

    /// Returns a host-only residency policy.
    #[must_use]
    pub const fn host_only() -> Self {
        Self::new(
            OptimizerStateResidency::HostResident,
            OptimizerStateResidency::HostResident,
        )
    }

    /// Returns a policy that stages into device residency during a step and
    /// offloads afterward.
    #[must_use]
    pub const fn device_step_offload_idle() -> Self {
        Self::new(
            OptimizerStateResidency::DeviceResident,
            OptimizerStateResidency::Offloaded,
        )
    }
}

impl Default for TrainingOptimizerResidencyPolicy {
    fn default() -> Self {
        Self::host_only()
    }
}

/// One machine-legible residency transition for one training step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizerResidencyTransition {
    /// Stable parameter-group identifier.
    pub group_id: String,
    /// Zero- or one-based step count after the transition owner step started.
    pub global_step: u64,
    /// Residency before the transition.
    pub from: OptimizerStateResidency,
    /// Residency after the transition.
    pub to: OptimizerStateResidency,
    /// Why the transition happened.
    pub reason: OptimizerResidencyTransitionReason,
}

/// Tensor payload carried by the fixed-budget training core.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingTensorBuffer {
    /// Tensor metadata.
    pub spec: TensorSpec,
    /// Host-visible payload.
    pub data: TensorData,
}

impl TrainingTensorBuffer {
    /// Creates a contiguous `f32` tensor payload for the reference training path.
    pub fn from_f32(
        group_id: impl Into<String>,
        spec: TensorSpec,
        values: Vec<f32>,
    ) -> Result<Self, TrainingCoreError> {
        let group_id = group_id.into();
        if spec.dtype() != DType::F32 {
            return Err(TrainingCoreError::UnsupportedTensorDType {
                group_id,
                dtype: spec.dtype(),
            });
        }
        let expected_len = spec.storage_size();
        let actual_len = values.len();
        if actual_len != expected_len {
            return Err(TrainingCoreError::TensorLengthMismatch {
                group_id,
                expected_len,
                actual_len,
            });
        }
        Ok(Self {
            spec,
            data: TensorData::F32(values),
        })
    }

    fn as_f32_slice(&self, group_id: &str) -> Result<&[f32], TrainingCoreError> {
        match &self.data {
            TensorData::F32(values) => Ok(values.as_slice()),
            TensorData::QuantizedBlocks(_) => Err(TrainingCoreError::UnsupportedTensorDType {
                group_id: String::from(group_id),
                dtype: self.spec.dtype(),
            }),
        }
    }

    fn as_f32_slice_mut(&mut self, group_id: &str) -> Result<&mut [f32], TrainingCoreError> {
        match &mut self.data {
            TensorData::F32(values) => Ok(values.as_mut_slice()),
            TensorData::QuantizedBlocks(_) => Err(TrainingCoreError::UnsupportedTensorDType {
                group_id: String::from(group_id),
                dtype: self.spec.dtype(),
            }),
        }
    }
}

/// Optimizer-state payload for one parameter group.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TrainingOptimizerState {
    /// SGD state with optional momentum buffer.
    Sgd {
        /// Optional momentum buffer.
        #[serde(skip_serializing_if = "Option::is_none")]
        momentum_buffer: Option<Vec<f32>>,
    },
    /// Adam state with first and second moments.
    Adam {
        /// Exponential moving average of gradients.
        first_moment: Vec<f32>,
        /// Exponential moving average of squared gradients.
        second_moment: Vec<f32>,
    },
    /// AdamW state with first and second moments.
    AdamW {
        /// Exponential moving average of gradients.
        first_moment: Vec<f32>,
        /// Exponential moving average of squared gradients.
        second_moment: Vec<f32>,
    },
    /// LARS state with optional momentum buffer.
    Lars {
        /// Optional momentum buffer.
        #[serde(skip_serializing_if = "Option::is_none")]
        momentum_buffer: Option<Vec<f32>>,
    },
    /// LAMB state with first and second moments.
    Lamb {
        /// Exponential moving average of gradients.
        first_moment: Vec<f32>,
        /// Exponential moving average of squared gradients.
        second_moment: Vec<f32>,
    },
}

impl TrainingOptimizerState {
    pub(crate) fn new(
        kind: TrainingOptimizerKind,
        parameter_len: usize,
        momentum: Option<f32>,
    ) -> Self {
        match kind {
            TrainingOptimizerKind::Sgd => Self::Sgd {
                momentum_buffer: momentum.map(|_| vec![0.0; parameter_len]),
            },
            TrainingOptimizerKind::Adam => Self::Adam {
                first_moment: vec![0.0; parameter_len],
                second_moment: vec![0.0; parameter_len],
            },
            TrainingOptimizerKind::AdamW => Self::AdamW {
                first_moment: vec![0.0; parameter_len],
                second_moment: vec![0.0; parameter_len],
            },
            TrainingOptimizerKind::Lars => Self::Lars {
                momentum_buffer: momentum.map(|_| vec![0.0; parameter_len]),
            },
            TrainingOptimizerKind::Lamb => Self::Lamb {
                first_moment: vec![0.0; parameter_len],
                second_moment: vec![0.0; parameter_len],
            },
        }
    }
}

/// Mutable parameter-group state owned by one training run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingParameterGroupState {
    /// Stable parameter-group identifier.
    pub group_id: String,
    /// High-level group family.
    pub class: TrainingParameterClass,
    /// Parameter tensor and values.
    pub parameter: TrainingTensorBuffer,
    /// Optimizer config for this group.
    pub optimizer: TrainingOptimizerConfig,
    /// Mutable optimizer state.
    pub optimizer_state: TrainingOptimizerState,
    /// Preferred residency policy.
    pub optimizer_residency_policy: TrainingOptimizerResidencyPolicy,
    /// Current residency posture.
    pub optimizer_residency: OptimizerStateResidency,
    /// Number of updates already applied to this group.
    pub applied_steps: u64,
}

impl TrainingParameterGroupState {
    /// Creates one parameter group with typed optimizer ownership.
    pub fn new(
        group_id: impl Into<String>,
        class: TrainingParameterClass,
        parameter: TrainingTensorBuffer,
        optimizer: TrainingOptimizerConfig,
        optimizer_residency_policy: TrainingOptimizerResidencyPolicy,
    ) -> Result<Self, TrainingCoreError> {
        let group_id = group_id.into();
        let parameter_len = parameter.as_f32_slice(group_id.as_str())?.len();
        Ok(Self {
            group_id,
            class,
            optimizer_state: TrainingOptimizerState::new(
                optimizer.kind,
                parameter_len,
                optimizer.momentum,
            ),
            optimizer,
            optimizer_residency_policy,
            optimizer_residency: optimizer_residency_policy.idle_residency,
            parameter,
            applied_steps: 0,
        })
    }
}

/// One explicit gradient contribution for a fixed-budget step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingGradientBatch {
    /// Stable batch identifier.
    pub batch_id: String,
    /// Logical loss or objective value attached to the batch.
    pub loss: f32,
    /// Number of samples represented by the batch.
    pub sample_count: u32,
    /// Per-group explicit gradients.
    pub gradients: BTreeMap<String, TrainingTensorBuffer>,
}

impl TrainingGradientBatch {
    /// Creates a gradient batch from explicit gradients.
    #[must_use]
    pub fn new(
        batch_id: impl Into<String>,
        loss: f32,
        sample_count: u32,
        gradients: BTreeMap<String, TrainingTensorBuffer>,
    ) -> Self {
        Self {
            batch_id: batch_id.into(),
            loss,
            sample_count,
            gradients,
        }
    }
}

/// One step input, including explicit timing for replay-safe testing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingStepInput {
    /// Explicit gradients and batch metadata.
    pub batch: TrainingGradientBatch,
    /// Logical step start time.
    pub started_at_ms: u64,
    /// Logical step finish time.
    pub finished_at_ms: u64,
}

impl TrainingStepInput {
    /// Creates one step input from explicit batch data and timestamps.
    #[must_use]
    pub fn new(batch: TrainingGradientBatch, started_at_ms: u64, finished_at_ms: u64) -> Self {
        Self {
            batch,
            started_at_ms,
            finished_at_ms,
        }
    }
}

/// Fixed-budget scheduling policy for a training run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingLoopBudget {
    /// Maximum number of trainer steps the loop may execute.
    pub max_steps: u64,
    /// Inner steps per logical window.
    pub steps_per_window: u64,
    /// Windows per outer cadence.
    pub windows_per_cadence: u64,
}

impl TrainingLoopBudget {
    /// Creates and validates a fixed-budget scheduling policy.
    pub fn new(
        max_steps: u64,
        steps_per_window: u64,
        windows_per_cadence: u64,
    ) -> Result<Self, TrainingCoreError> {
        if max_steps == 0 {
            return Err(TrainingCoreError::InvalidBudgetField { field: "max_steps" });
        }
        if steps_per_window == 0 {
            return Err(TrainingCoreError::InvalidBudgetField {
                field: "steps_per_window",
            });
        }
        if windows_per_cadence == 0 {
            return Err(TrainingCoreError::InvalidBudgetField {
                field: "windows_per_cadence",
            });
        }
        Ok(Self {
            max_steps,
            steps_per_window,
            windows_per_cadence,
        })
    }
}

/// Step schedule emitted for each receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingStepSchedule {
    /// One-based global step number.
    pub global_step: u64,
    /// One-based window number.
    pub window_index: u64,
    /// One-based inner-step index inside the current window.
    pub step_in_window: u64,
    /// One-based cadence number.
    pub cadence_index: u64,
    /// One-based window index inside the current cadence.
    pub window_in_cadence: u64,
}

impl TrainingStepSchedule {
    fn from_budget(budget: TrainingLoopBudget, completed_steps: u64) -> Self {
        let global_step = completed_steps.saturating_add(1);
        let window_index = ((global_step - 1) / budget.steps_per_window) + 1;
        let step_in_window = ((global_step - 1) % budget.steps_per_window) + 1;
        let cadence_index = ((window_index - 1) / budget.windows_per_cadence) + 1;
        let window_in_cadence = ((window_index - 1) % budget.windows_per_cadence) + 1;
        Self {
            global_step,
            window_index,
            step_in_window,
            cadence_index,
            window_in_cadence,
        }
    }
}

/// Checkpoint lineage used to bootstrap a training run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingRestoreSource {
    /// Stable checkpoint identity.
    pub checkpoint: TrainingCheckpointReference,
    /// Stable manifest identity that can restage the checkpoint payload.
    pub manifest: DatastreamManifestRef,
}

/// High-level step execution mode for the current reference path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingStepExecutionMode {
    /// Gradients were supplied explicitly rather than produced by a full autodiff engine.
    ExplicitGradientBatch,
}

/// One group-level telemetry record for a trainer step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingGroupTelemetry {
    /// Stable group identifier.
    pub group_id: String,
    /// High-level group family.
    pub class: TrainingParameterClass,
    /// Optimizer family.
    pub optimizer: TrainingOptimizerKind,
    /// Gradient norm before clipping.
    pub gradient_norm_l2: f32,
    /// Gradient norm after clipping.
    pub clipped_gradient_norm_l2: f32,
    /// Ratio between clipped and unclipped gradient norms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clipping_ratio: Option<f32>,
    /// L2 norm of the applied parameter update.
    pub update_norm_l2: f32,
    /// L2 norm of the parameters after the update.
    pub parameter_norm_l2: f32,
    /// Residency before the step touched the optimizer state.
    pub residency_before: OptimizerStateResidency,
    /// Residency after the step completed.
    pub residency_after: OptimizerStateResidency,
}

/// Timing telemetry for one training step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingStepTiming {
    /// Logical step start.
    pub started_at_ms: u64,
    /// Logical step finish.
    pub finished_at_ms: u64,
    /// Total step duration.
    pub duration_ms: u64,
}

/// Machine-legible receipt emitted after one trainer step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingStepReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable training run identifier.
    pub run_id: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Explicit step execution mode.
    pub execution_mode: TrainingStepExecutionMode,
    /// Fixed-budget schedule identity.
    pub schedule: TrainingStepSchedule,
    /// Stable batch identifier.
    pub batch_id: String,
    /// Loss attached to the batch.
    pub loss: f32,
    /// Batch sample count.
    pub sample_count: u32,
    /// Restore lineage used to bootstrap the run when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restore_source: Option<TrainingRestoreSource>,
    /// Group-level telemetry.
    pub group_telemetry: Vec<TrainingGroupTelemetry>,
    /// Residency transitions emitted during the step.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub residency_transitions: Vec<OptimizerResidencyTransition>,
    /// Step timing.
    pub timing: TrainingStepTiming,
    /// Stable digest over the receipt contents.
    pub receipt_digest: String,
}

/// Final machine-legible run summary for one fixed-budget training pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingRunSummary {
    /// Stable run identifier.
    pub run_id: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Total number of completed steps.
    pub completed_steps: u64,
    /// Fixed budget carried by the run.
    pub budget: TrainingLoopBudget,
    /// Whether the run reached its fixed budget.
    pub budget_reached: bool,
    /// Restore lineage when the run bootstrapped from a durable checkpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restore_source: Option<TrainingRestoreSource>,
    /// Final parameter norms keyed by group identifier.
    pub final_parameter_norms_l2: BTreeMap<String, f32>,
    /// Stable identifier for the last step receipt when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_receipt_id: Option<String>,
}

/// Receipts and summary for one fixed-budget training execution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingRunOutcome {
    /// Step receipts emitted during the run.
    pub receipts: Vec<TrainingStepReceipt>,
    /// Final run summary.
    pub summary: TrainingRunSummary,
}

/// Mutable fixed-budget training run with explicit parameter groups and
/// optimizer-state ownership.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FixedBudgetTrainingRun {
    run_id: String,
    checkpoint_family: String,
    budget: TrainingLoopBudget,
    restore_source: Option<TrainingRestoreSource>,
    parameter_groups: BTreeMap<String, TrainingParameterGroupState>,
    completed_steps: u64,
    last_receipt: Option<TrainingStepReceipt>,
}

impl FixedBudgetTrainingRun {
    /// Creates a cold-start training run without checkpoint restore lineage.
    pub fn new(
        run_id: impl Into<String>,
        checkpoint_family: impl Into<String>,
        budget: TrainingLoopBudget,
        parameter_groups: Vec<TrainingParameterGroupState>,
    ) -> Result<Self, TrainingCoreError> {
        Self::new_with_restore(
            run_id.into(),
            checkpoint_family.into(),
            budget,
            parameter_groups,
            None,
        )
    }

    fn new_with_restore(
        run_id: String,
        checkpoint_family: String,
        budget: TrainingLoopBudget,
        parameter_groups: Vec<TrainingParameterGroupState>,
        restore_source: Option<TrainingRestoreSource>,
    ) -> Result<Self, TrainingCoreError> {
        if parameter_groups.is_empty() {
            return Err(TrainingCoreError::EmptyParameterGroups);
        }
        let mut groups = BTreeMap::new();
        for group in parameter_groups {
            let group_id = group.group_id.clone();
            if groups.insert(group_id.clone(), group).is_some() {
                return Err(TrainingCoreError::DuplicateParameterGroup { group_id });
            }
        }
        Ok(Self {
            run_id,
            checkpoint_family,
            budget,
            restore_source,
            parameter_groups: groups,
            completed_steps: 0,
            last_receipt: None,
        })
    }

    /// Returns the stable run identifier.
    #[must_use]
    pub fn run_id(&self) -> &str {
        self.run_id.as_str()
    }

    /// Returns the current restore lineage when one exists.
    #[must_use]
    pub fn restore_source(&self) -> Option<&TrainingRestoreSource> {
        self.restore_source.as_ref()
    }

    /// Returns the current parameter-group state by identifier.
    #[must_use]
    pub fn parameter_group(&self, group_id: &str) -> Option<&TrainingParameterGroupState> {
        self.parameter_groups.get(group_id)
    }

    /// Returns the number of completed steps.
    #[must_use]
    pub const fn completed_steps(&self) -> u64 {
        self.completed_steps
    }

    /// Applies one explicit-gradient trainer step and emits a typed receipt.
    pub fn apply_step(
        &mut self,
        input: TrainingStepInput,
    ) -> Result<TrainingStepReceipt, TrainingCoreError> {
        if self.completed_steps >= self.budget.max_steps {
            return Err(TrainingCoreError::BudgetExhausted {
                run_id: self.run_id.clone(),
                max_steps: self.budget.max_steps,
            });
        }
        if input.finished_at_ms < input.started_at_ms {
            return Err(TrainingCoreError::InvalidStepTiming {
                started_at_ms: input.started_at_ms,
                finished_at_ms: input.finished_at_ms,
            });
        }

        let schedule = TrainingStepSchedule::from_budget(self.budget, self.completed_steps);
        let mut group_telemetry = Vec::new();
        let mut residency_transitions = Vec::new();
        for (group_id, group) in &mut self.parameter_groups {
            let gradient = input.batch.gradients.get(group_id).ok_or_else(|| {
                TrainingCoreError::MissingGradient {
                    batch_id: input.batch.batch_id.clone(),
                    group_id: group_id.clone(),
                }
            })?;
            validate_gradient_tensor(
                input.batch.batch_id.as_str(),
                group_id.as_str(),
                &group.parameter.spec,
                &gradient.spec,
            )?;
            let gradient_values = gradient.as_f32_slice(group_id.as_str())?;
            let group_receipt = apply_group_step(
                group,
                gradient_values,
                schedule.global_step,
                self.restore_source.is_some() && self.completed_steps == 0,
            )?;
            residency_transitions.extend(group_receipt.transitions.clone());
            group_telemetry.push(group_receipt.telemetry);
        }

        let timing = TrainingStepTiming {
            started_at_ms: input.started_at_ms,
            finished_at_ms: input.finished_at_ms,
            duration_ms: input.finished_at_ms - input.started_at_ms,
        };
        let receipt_id = format!("{}-step-{}", self.run_id, schedule.global_step);
        let mut receipt = TrainingStepReceipt {
            receipt_digest: String::new(),
            receipt_id: receipt_id.clone(),
            run_id: self.run_id.clone(),
            checkpoint_family: self.checkpoint_family.clone(),
            execution_mode: TrainingStepExecutionMode::ExplicitGradientBatch,
            schedule,
            batch_id: input.batch.batch_id,
            loss: input.batch.loss,
            sample_count: input.batch.sample_count,
            restore_source: self.restore_source.clone(),
            group_telemetry,
            residency_transitions,
            timing,
        };
        receipt.receipt_digest = stable_training_step_receipt_digest(&receipt);
        self.completed_steps = self.completed_steps.saturating_add(1);
        self.last_receipt = Some(receipt.clone());
        Ok(receipt)
    }

    /// Runs step inputs until the budget is reached or inputs are exhausted.
    pub fn run_fixed_budget<I>(
        &mut self,
        inputs: I,
    ) -> Result<TrainingRunOutcome, TrainingCoreError>
    where
        I: IntoIterator<Item = TrainingStepInput>,
    {
        let remaining = self.budget.max_steps.saturating_sub(self.completed_steps) as usize;
        let mut receipts = Vec::new();
        for input in inputs.into_iter().take(remaining) {
            receipts.push(self.apply_step(input)?);
        }
        let summary = self.summary();
        Ok(TrainingRunOutcome { receipts, summary })
    }

    /// Returns a current machine-legible summary.
    #[must_use]
    pub fn summary(&self) -> TrainingRunSummary {
        let final_parameter_norms_l2 = self
            .parameter_groups
            .iter()
            .map(|(group_id, group)| {
                let norm = group
                    .parameter
                    .as_f32_slice(group_id.as_str())
                    .map_or(0.0, norm_l2);
                (group_id.clone(), norm)
            })
            .collect::<BTreeMap<_, _>>();
        TrainingRunSummary {
            run_id: self.run_id.clone(),
            checkpoint_family: self.checkpoint_family.clone(),
            completed_steps: self.completed_steps,
            budget: self.budget,
            budget_reached: self.completed_steps >= self.budget.max_steps,
            restore_source: self.restore_source.clone(),
            final_parameter_norms_l2,
            last_receipt_id: self
                .last_receipt
                .as_ref()
                .map(|receipt| receipt.receipt_id.clone()),
        }
    }
}

impl TrainingSessionState {
    /// Builds a fixed-budget training run anchored to the latest durable checkpoint.
    pub fn restore_fixed_budget_run(
        &self,
        run_id: impl Into<String>,
        budget: TrainingLoopBudget,
        parameter_groups: Vec<TrainingParameterGroupState>,
    ) -> Result<FixedBudgetTrainingRun, TrainingCoreError> {
        let checkpoint_family = self.checkpoint_family.clone();
        let checkpoint = self.latest_durable_checkpoint.clone().ok_or_else(|| {
            TrainingCoreError::CheckpointRestoreUnavailable {
                checkpoint_family: checkpoint_family.clone(),
            }
        })?;
        let manifest = self.latest_durable_manifest.clone().ok_or_else(|| {
            TrainingCoreError::CheckpointManifestMissing {
                checkpoint_family: checkpoint_family.clone(),
            }
        })?;
        FixedBudgetTrainingRun::new_with_restore(
            run_id.into(),
            checkpoint_family,
            budget,
            parameter_groups,
            Some(TrainingRestoreSource {
                checkpoint,
                manifest,
            }),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
struct GroupStepApplication {
    telemetry: TrainingGroupTelemetry,
    transitions: Vec<OptimizerResidencyTransition>,
}

fn validate_gradient_tensor(
    batch_id: &str,
    group_id: &str,
    expected: &TensorSpec,
    actual: &TensorSpec,
) -> Result<(), TrainingCoreError> {
    if expected != actual {
        return Err(TrainingCoreError::GradientTensorMismatch {
            batch_id: String::from(batch_id),
            group_id: String::from(group_id),
            expected: expected.clone(),
            actual: actual.clone(),
        });
    }
    Ok(())
}

fn apply_group_step(
    group: &mut TrainingParameterGroupState,
    gradient_values: &[f32],
    global_step: u64,
    restored_first_step: bool,
) -> Result<GroupStepApplication, TrainingCoreError> {
    let mut transitions = Vec::new();
    let residency_before = group.optimizer_residency;

    if restored_first_step {
        transitions.push(OptimizerResidencyTransition {
            group_id: group.group_id.clone(),
            global_step,
            from: group.optimizer_residency_policy.idle_residency,
            to: group.optimizer_residency_policy.idle_residency,
            reason: OptimizerResidencyTransitionReason::RecoveryRestore,
        });
    }
    maybe_transition_group(
        group,
        group.optimizer_residency_policy.step_residency,
        global_step,
        OptimizerResidencyTransitionReason::PrefetchForStep,
        &mut transitions,
    );

    let parameter_values = group.parameter.as_f32_slice_mut(group.group_id.as_str())?;
    let gradient_norm_l2 = norm_l2(gradient_values);
    let (clipped_gradients, clipped_gradient_norm_l2, clipping_ratio) =
        clipped_gradients(gradient_values, group.optimizer.gradient_clip_norm);
    let optimizer_report = crate::optimizer::apply_training_optimizer_step(
        parameter_values,
        clipped_gradients.as_slice(),
        &group.optimizer,
        &mut group.optimizer_state,
        group.applied_steps.saturating_add(1),
    )
    .map_err(|error| TrainingCoreError::OptimizerStepFailed {
        group_id: group.group_id.clone(),
        message: error.to_string(),
    })?;
    let update_norm_l2 = optimizer_report.update_norm_l2;
    let parameter_norm_l2 = optimizer_report.parameter_norm_l2_after;
    group.applied_steps = group.applied_steps.saturating_add(1);

    maybe_transition_group(
        group,
        group.optimizer_residency_policy.idle_residency,
        global_step,
        OptimizerResidencyTransitionReason::OffloadAfterStep,
        &mut transitions,
    );

    Ok(GroupStepApplication {
        telemetry: TrainingGroupTelemetry {
            group_id: group.group_id.clone(),
            class: group.class,
            optimizer: group.optimizer.kind,
            gradient_norm_l2,
            clipped_gradient_norm_l2,
            clipping_ratio,
            update_norm_l2,
            parameter_norm_l2,
            residency_before,
            residency_after: group.optimizer_residency,
        },
        transitions,
    })
}

fn maybe_transition_group(
    group: &mut TrainingParameterGroupState,
    target: OptimizerStateResidency,
    global_step: u64,
    reason: OptimizerResidencyTransitionReason,
    transitions: &mut Vec<OptimizerResidencyTransition>,
) {
    if group.optimizer_residency == target {
        return;
    }
    let transition = OptimizerResidencyTransition {
        group_id: group.group_id.clone(),
        global_step,
        from: group.optimizer_residency,
        to: target,
        reason,
    };
    group.optimizer_residency = target;
    transitions.push(transition);
}

fn clipped_gradients(gradients: &[f32], clip_norm: Option<f32>) -> (Vec<f32>, f32, Option<f32>) {
    let gradient_norm_l2 = norm_l2(gradients);
    let Some(clip_norm) = clip_norm else {
        return (gradients.to_vec(), gradient_norm_l2, None);
    };
    if gradient_norm_l2 <= clip_norm || gradient_norm_l2 == 0.0 {
        return (gradients.to_vec(), gradient_norm_l2, Some(1.0));
    }
    let scale = clip_norm / gradient_norm_l2;
    (
        gradients.iter().map(|value| value * scale).collect(),
        clip_norm,
        Some(scale),
    )
}

fn norm_l2(values: &[f32]) -> f32 {
    values.iter().map(|value| value * value).sum::<f32>().sqrt()
}

fn stable_training_step_receipt_digest(receipt: &TrainingStepReceipt) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_step_receipt|");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.checkpoint_family.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.batch_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.schedule.global_step.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.loss.to_bits().to_le_bytes());
    for telemetry in &receipt.group_telemetry {
        hasher.update(b"|group|");
        hasher.update(telemetry.group_id.as_bytes());
        hasher.update(b"|");
        hasher.update(telemetry.gradient_norm_l2.to_bits().to_le_bytes());
        hasher.update(b"|");
        hasher.update(telemetry.clipped_gradient_norm_l2.to_bits().to_le_bytes());
        hasher.update(b"|");
        hasher.update(telemetry.update_norm_l2.to_bits().to_le_bytes());
        hasher.update(b"|");
        hasher.update(telemetry.parameter_norm_l2.to_bits().to_le_bytes());
    }
    for transition in &receipt.residency_transitions {
        hasher.update(b"|transition|");
        hasher.update(transition.group_id.as_bytes());
        hasher.update(b"|");
        hasher.update(transition.global_step.to_string().as_bytes());
    }
    if let Some(restore_source) = &receipt.restore_source {
        hasher.update(b"|restore|");
        hasher.update(restore_source.checkpoint.stream_id.as_bytes());
        hasher.update(b"|");
        hasher.update(restore_source.manifest.manifest_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

    use std::{
        collections::BTreeMap,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };

    use psionic_cluster::{
        AdmissionToken, ClusterId, ClusterMembershipRecord, ClusterMembershipStatus,
        ClusterNamespace, ClusterNodeIdentity, ClusterSnapshot, ClusterState, NodeEpoch, NodeId,
        NodeRole,
    };
    use psionic_core::{DType, Device, Shape, TensorSpec};
    use psionic_datastream::{
        DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifest, DatastreamSubjectKind,
    };

    use super::{
        FixedBudgetTrainingRun, OptimizerResidencyTransitionReason, OptimizerStateResidency,
        TrainingCoreError, TrainingGradientBatch, TrainingLoopBudget, TrainingOptimizerConfig,
        TrainingOptimizerKind, TrainingOptimizerResidencyPolicy, TrainingParameterClass,
        TrainingParameterGroupState, TrainingSessionState, TrainingStepInput, TrainingTensorBuffer,
    };

    fn training_spec(width: usize) -> TensorSpec {
        TensorSpec::new(Shape::new(vec![width]), DType::F32, Device::cpu())
    }

    fn group(
        group_id: &str,
        class: TrainingParameterClass,
        values: Vec<f32>,
        optimizer: TrainingOptimizerConfig,
        residency: TrainingOptimizerResidencyPolicy,
    ) -> Result<TrainingParameterGroupState, TrainingCoreError> {
        TrainingParameterGroupState::new(
            group_id,
            class,
            TrainingTensorBuffer::from_f32(group_id, training_spec(values.len()), values)?,
            optimizer,
            residency,
        )
    }

    fn gradients(
        values: &[(&str, Vec<f32>)],
    ) -> Result<BTreeMap<String, TrainingTensorBuffer>, TrainingCoreError> {
        values
            .iter()
            .map(|(group_id, gradient)| {
                Ok((
                    String::from(*group_id),
                    TrainingTensorBuffer::from_f32(
                        *group_id,
                        training_spec(gradient.len()),
                        gradient.clone(),
                    )?,
                ))
            })
            .collect()
    }

    fn cluster_id() -> ClusterId {
        ClusterId::new(
            &ClusterNamespace::new("train-core-cluster"),
            &AdmissionToken::new("shared-secret"),
        )
    }

    fn membership(
        cluster_id: &ClusterId,
        node_id: &str,
        status: ClusterMembershipStatus,
    ) -> ClusterMembershipRecord {
        ClusterMembershipRecord::new(
            ClusterNodeIdentity {
                cluster_id: cluster_id.clone(),
                node_id: NodeId::new(node_id),
                node_epoch: NodeEpoch::initial(),
                role: NodeRole::Mixed,
                auth_public_key: format!("{node_id}-pub"),
                attestation: None,
            },
            Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9_000)),
            status,
        )
    }

    fn cluster_state(records: &[(&str, ClusterMembershipStatus)]) -> ClusterState {
        let cluster_id = cluster_id();
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships = records
            .iter()
            .map(|(node_id, status)| {
                (
                    NodeId::new(*node_id),
                    membership(&cluster_id, node_id, *status),
                )
            })
            .collect::<BTreeMap<_, _>>();
        ClusterState::from_snapshot(snapshot)
    }

    #[test]
    fn fixed_budget_training_loop_applies_updates_and_tracks_telemetry()
    -> Result<(), Box<dyn std::error::Error>> {
        let budget = TrainingLoopBudget::new(3, 2, 2)?;
        let embedding_group = group(
            "token_embed",
            TrainingParameterClass::Embedding,
            vec![1.0, -1.0],
            TrainingOptimizerConfig::adamw(0.1, 0.9, 0.99, 1e-8).with_gradient_clip_norm(1.0),
            TrainingOptimizerResidencyPolicy::device_step_offload_idle(),
        )?;
        let head_group = group(
            "lm_head",
            TrainingParameterClass::Head,
            vec![0.5, -0.5],
            TrainingOptimizerConfig::sgd(0.2).with_weight_decay(0.1),
            TrainingOptimizerResidencyPolicy::host_only(),
        )?;
        let mut run = FixedBudgetTrainingRun::new(
            "train-run-1",
            "train.decoder",
            budget,
            vec![embedding_group, head_group],
        )?;
        let inputs = vec![
            TrainingStepInput::new(
                TrainingGradientBatch::new(
                    "batch-1",
                    1.25,
                    4,
                    gradients(&[
                        ("token_embed", vec![0.2, -0.2]),
                        ("lm_head", vec![0.1, -0.1]),
                    ])?,
                ),
                1_000,
                1_025,
            ),
            TrainingStepInput::new(
                TrainingGradientBatch::new(
                    "batch-2",
                    0.95,
                    4,
                    gradients(&[
                        ("token_embed", vec![0.2, -0.2]),
                        ("lm_head", vec![0.1, -0.1]),
                    ])?,
                ),
                1_100,
                1_130,
            ),
            TrainingStepInput::new(
                TrainingGradientBatch::new(
                    "batch-3",
                    0.75,
                    4,
                    gradients(&[
                        ("token_embed", vec![0.2, -0.2]),
                        ("lm_head", vec![0.1, -0.1]),
                    ])?,
                ),
                1_200,
                1_245,
            ),
        ];

        let outcome = run.run_fixed_budget(inputs)?;

        assert_eq!(outcome.receipts.len(), 3);
        assert!(outcome.summary.budget_reached);
        assert_eq!(outcome.summary.completed_steps, 3);
        assert_eq!(outcome.receipts[1].schedule.window_index, 1);
        assert_eq!(outcome.receipts[2].schedule.window_index, 2);
        assert_eq!(outcome.receipts[2].schedule.cadence_index, 1);
        assert_eq!(
            outcome.receipts[0].group_telemetry[0].optimizer,
            TrainingOptimizerKind::Sgd
        );
        assert!(
            outcome.receipts[0]
                .residency_transitions
                .iter()
                .any(|transition| {
                    transition.group_id == "token_embed"
                        && transition.reason == OptimizerResidencyTransitionReason::PrefetchForStep
                        && transition.from == OptimizerStateResidency::Offloaded
                        && transition.to == OptimizerStateResidency::DeviceResident
                })
        );

        let token_embed = run.parameter_group("token_embed").expect("embed group");
        let embed_values = token_embed
            .parameter
            .as_f32_slice("token_embed")
            .expect("embed values");
        assert!((embed_values[0] - 0.7).abs() < 0.0001);
        assert!((embed_values[1] + 0.7).abs() < 0.0001);

        let lm_head = run.parameter_group("lm_head").expect("head group");
        let head_values = lm_head
            .parameter
            .as_f32_slice("lm_head")
            .expect("head values");
        assert!((head_values[0] - 0.411788).abs() < 0.0001);
        assert!((head_values[1] + 0.411788).abs() < 0.0001);
        assert!(
            outcome.summary.final_parameter_norms_l2["token_embed"]
                > outcome.summary.final_parameter_norms_l2["lm_head"]
        );
        Ok(())
    }

    #[test]
    fn fixed_budget_training_loop_can_restore_from_latest_durable_checkpoint()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = cluster_state(&[
            ("worker-a", ClusterMembershipStatus::Ready),
            ("worker-b", ClusterMembershipStatus::Ready),
        ]);
        let manifest = DatastreamManifest::from_bytes(
            "checkpoint-stream",
            DatastreamSubjectKind::Checkpoint,
            b"checkpoint-bytes",
            4,
            DatastreamEncoding::Safetensors,
        )
        .with_checkpoint_binding(
            DatastreamCheckpointBinding::new("train.decoder")
                .with_checkpoint_ref("step-12")
                .with_step(12),
        );
        let mut session = TrainingSessionState::new(state.cluster_id().as_str(), "train.decoder");
        let write =
            session.begin_async_checkpoint(&state, &manifest, &NodeId::new("worker-a"), 1_000)?;
        session.mark_checkpoint_durable(write.write_id.as_str(), 1_030)?;

        let budget = TrainingLoopBudget::new(1, 1, 1)?;
        let restored_group = group(
            "decoder_matrix",
            TrainingParameterClass::Matrix,
            vec![0.25, -0.25],
            TrainingOptimizerConfig::adamw(0.05, 0.9, 0.99, 1e-8),
            TrainingOptimizerResidencyPolicy::device_step_offload_idle(),
        )?;
        let mut run =
            session.restore_fixed_budget_run("train-run-restore", budget, vec![restored_group])?;

        let receipt = run.apply_step(TrainingStepInput::new(
            TrainingGradientBatch::new(
                "batch-restore",
                0.4,
                2,
                gradients(&[("decoder_matrix", vec![0.1, -0.1])])?,
            ),
            2_000,
            2_050,
        ))?;

        let restore_source = run.restore_source().expect("restore source");
        assert_eq!(
            restore_source.checkpoint.checkpoint_ref.as_deref(),
            Some("step-12")
        );
        assert_eq!(restore_source.manifest.stream_id, "checkpoint-stream");
        assert_eq!(
            receipt
                .restore_source
                .as_ref()
                .and_then(|source| source.checkpoint.step),
            Some(12)
        );
        assert!(receipt.residency_transitions.iter().any(|transition| {
            transition.reason == OptimizerResidencyTransitionReason::RecoveryRestore
        }));
        Ok(())
    }

    #[test]
    fn fixed_budget_training_loop_refuses_missing_gradients()
    -> Result<(), Box<dyn std::error::Error>> {
        let budget = TrainingLoopBudget::new(1, 1, 1)?;
        let parameter_group = group(
            "decoder_bias",
            TrainingParameterClass::Bias,
            vec![0.0],
            TrainingOptimizerConfig::sgd(0.1),
            TrainingOptimizerResidencyPolicy::host_only(),
        )?;
        let mut run = FixedBudgetTrainingRun::new(
            "train-run-missing-grad",
            "train.decoder",
            budget,
            vec![parameter_group],
        )?;
        let error = run
            .apply_step(TrainingStepInput::new(
                TrainingGradientBatch::new("batch-missing", 1.0, 1, BTreeMap::new()),
                10,
                20,
            ))
            .expect_err("missing gradient should be refused");

        assert_eq!(
            error,
            TrainingCoreError::MissingGradient {
                batch_id: String::from("batch-missing"),
                group_id: String::from("decoder_bias"),
            }
        );
        Ok(())
    }
}
