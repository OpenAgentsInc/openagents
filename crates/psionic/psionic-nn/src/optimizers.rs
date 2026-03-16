use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Module, ModuleParameter, ModuleParameterView, ModuleStateDict, ModuleStateEntry,
    ModuleStateEntryKind, ModuleStateError,
};
use psionic_core::{DType, Device, DeviceKind, TensorData, TensorSpec};
use psionic_train::{
    TrainingOptimizerConfig, TrainingOptimizerError, TrainingOptimizerKind, TrainingOptimizerState,
    TrainingOptimizerStepReport, TrainingSchedulerBinding, TrainingSchedulerConfig,
    TrainingSchedulerKind, TrainingSchedulerState, scheduled_learning_rate,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum OptimizerError {
    #[error(transparent)]
    ModuleState(#[from] ModuleStateError),
    #[error(transparent)]
    OptimizerMath(#[from] TrainingOptimizerError),
    #[error(
        "optimizer gradients target root module `{actual_module_id}` but expected `{expected_module_id}`"
    )]
    GradientRootMismatch {
        expected_module_id: String,
        actual_module_id: String,
    },
    #[error(
        "optimizer gradients target module kind `{actual_module_kind}` but expected `{expected_module_kind}`"
    )]
    GradientModuleKindMismatch {
        expected_module_kind: String,
        actual_module_kind: String,
    },
    #[error("optimizer is missing gradient for trainable parameter `{path}`")]
    MissingGradient { path: String },
    #[error("optimizer received unexpected gradient path `{path}`")]
    UnexpectedGradient { path: String },
    #[error("optimizer group id must be non-empty")]
    MissingGroupId,
    #[error("optimizer group `{group_id}` was defined more than once")]
    DuplicateGroupId { group_id: String },
    #[error("optimizer group `{group_id}` lists parameter path `{path}` more than once")]
    DuplicateGroupPath { group_id: String, path: String },
    #[error(
        "optimizer parameter path `{path}` is assigned to both `{first_group_id}` and `{second_group_id}`"
    )]
    DuplicateParameterAssignment {
        path: String,
        first_group_id: String,
        second_group_id: String,
    },
    #[error("trainable parameter `{path}` is not assigned to any optimizer group")]
    UnassignedTrainableParameter { path: String },
    #[error("optimizer gradient `{path}` must be a parameter entry, found `{kind:?}`")]
    GradientEntryKindMismatch {
        path: String,
        kind: ModuleStateEntryKind,
    },
    #[error(
        "optimizer `{role}` tensor `{path}` must be dense cpu f32, found dtype {dtype:?} on device {device}"
    )]
    UnsupportedTensor {
        role: &'static str,
        path: String,
        dtype: DType,
        device: Device,
    },
    #[error(
        "optimizer `{role}` tensor `{path}` expected {expected_len} dense values but found {actual_len}"
    )]
    TensorLengthMismatch {
        role: &'static str,
        path: String,
        expected_len: usize,
        actual_len: usize,
    },
    #[error(
        "optimizer gradient `{path}` tensor spec mismatch: expected {expected:?}, found {actual:?}"
    )]
    GradientTensorMismatch {
        path: String,
        expected: TensorSpec,
        actual: TensorSpec,
    },
    #[error("optimizer state for `{path}` expected tensor spec {expected:?} but found {actual:?}")]
    StateSpecMismatch {
        path: String,
        expected: TensorSpec,
        actual: TensorSpec,
    },
    #[error("optimizer state for `{path}` expects `{expected:?}` but carried `{actual:?}`")]
    StateKindMismatch {
        path: String,
        expected: OptimizerKind,
        actual: OptimizerKind,
    },
    #[error("optimizer snapshot config mismatch: expected {expected:?}, found {actual:?}")]
    SnapshotConfigMismatch {
        expected: OptimizerConfig,
        actual: OptimizerConfig,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizerKind {
    Sgd,
    Adam,
    AdamW,
    Lars,
    Lamb,
}

impl From<OptimizerKind> for TrainingOptimizerKind {
    fn from(value: OptimizerKind) -> Self {
        match value {
            OptimizerKind::Sgd => Self::Sgd,
            OptimizerKind::Adam => Self::Adam,
            OptimizerKind::AdamW => Self::AdamW,
            OptimizerKind::Lars => Self::Lars,
            OptimizerKind::Lamb => Self::Lamb,
        }
    }
}

impl From<TrainingOptimizerKind> for OptimizerKind {
    fn from(value: TrainingOptimizerKind) -> Self {
        match value {
            TrainingOptimizerKind::Sgd => Self::Sgd,
            TrainingOptimizerKind::Adam => Self::Adam,
            TrainingOptimizerKind::AdamW => Self::AdamW,
            TrainingOptimizerKind::Lars => Self::Lars,
            TrainingOptimizerKind::Lamb => Self::Lamb,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulerKind {
    Constant,
    StepLr,
    LinearWarmup,
    CosineAnnealing,
}

impl From<SchedulerKind> for TrainingSchedulerKind {
    fn from(value: SchedulerKind) -> Self {
        match value {
            SchedulerKind::Constant => Self::Constant,
            SchedulerKind::StepLr => Self::StepLr,
            SchedulerKind::LinearWarmup => Self::LinearWarmup,
            SchedulerKind::CosineAnnealing => Self::CosineAnnealing,
        }
    }
}

impl From<TrainingSchedulerKind> for SchedulerKind {
    fn from(value: TrainingSchedulerKind) -> Self {
        match value {
            TrainingSchedulerKind::Constant => Self::Constant,
            TrainingSchedulerKind::StepLr => Self::StepLr,
            TrainingSchedulerKind::LinearWarmup => Self::LinearWarmup,
            TrainingSchedulerKind::CosineAnnealing => Self::CosineAnnealing,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SchedulerConfig {
    Constant,
    StepLr {
        step_size: u64,
        gamma: f32,
    },
    LinearWarmup {
        warmup_steps: u64,
        start_factor: f32,
    },
    CosineAnnealing {
        total_steps: u64,
        min_learning_rate: f32,
    },
}

impl SchedulerConfig {
    #[must_use]
    pub const fn constant() -> Self {
        Self::Constant
    }

    #[must_use]
    pub const fn step_lr(step_size: u64, gamma: f32) -> Self {
        Self::StepLr { step_size, gamma }
    }

    #[must_use]
    pub const fn linear_warmup(warmup_steps: u64, start_factor: f32) -> Self {
        Self::LinearWarmup {
            warmup_steps,
            start_factor,
        }
    }

    #[must_use]
    pub const fn cosine_annealing(total_steps: u64, min_learning_rate: f32) -> Self {
        Self::CosineAnnealing {
            total_steps,
            min_learning_rate,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> SchedulerKind {
        match self {
            Self::Constant => SchedulerKind::Constant,
            Self::StepLr { .. } => SchedulerKind::StepLr,
            Self::LinearWarmup { .. } => SchedulerKind::LinearWarmup,
            Self::CosineAnnealing { .. } => SchedulerKind::CosineAnnealing,
        }
    }

    fn to_training(&self) -> TrainingSchedulerConfig {
        match self {
            Self::Constant => TrainingSchedulerConfig::constant(),
            Self::StepLr { step_size, gamma } => {
                TrainingSchedulerConfig::step_lr(*step_size, *gamma)
            }
            Self::LinearWarmup {
                warmup_steps,
                start_factor,
            } => TrainingSchedulerConfig::linear_warmup(*warmup_steps, *start_factor),
            Self::CosineAnnealing {
                total_steps,
                min_learning_rate,
            } => TrainingSchedulerConfig::cosine_annealing(*total_steps, *min_learning_rate),
        }
    }

    fn from_training(config: &TrainingSchedulerConfig) -> Self {
        match config {
            TrainingSchedulerConfig::Constant => Self::Constant,
            TrainingSchedulerConfig::StepLr { step_size, gamma } => Self::StepLr {
                step_size: *step_size,
                gamma: *gamma,
            },
            TrainingSchedulerConfig::LinearWarmup {
                warmup_steps,
                start_factor,
            } => Self::LinearWarmup {
                warmup_steps: *warmup_steps,
                start_factor: *start_factor,
            },
            TrainingSchedulerConfig::CosineAnnealing {
                total_steps,
                min_learning_rate,
            } => Self::CosineAnnealing {
                total_steps: *total_steps,
                min_learning_rate: *min_learning_rate,
            },
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SchedulerState {
    pub last_step: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_learning_rate: Option<f32>,
}

impl SchedulerState {
    fn from_training(state: &TrainingSchedulerState) -> Self {
        Self {
            last_step: state.last_step,
            last_learning_rate: state.last_learning_rate,
        }
    }

    fn to_training(&self) -> TrainingSchedulerState {
        TrainingSchedulerState {
            last_step: self.last_step,
            last_learning_rate: self.last_learning_rate,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SchedulerBinding {
    pub config: SchedulerConfig,
    pub state: SchedulerState,
}

impl SchedulerBinding {
    #[must_use]
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            config,
            state: SchedulerState::default(),
        }
    }

    fn to_training(&self) -> TrainingSchedulerBinding {
        TrainingSchedulerBinding {
            config: self.config.to_training(),
            state: self.state.to_training(),
        }
    }

    fn from_training(binding: &TrainingSchedulerBinding) -> Self {
        Self {
            config: SchedulerConfig::from_training(&binding.config),
            state: SchedulerState::from_training(&binding.state),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGroupSemantics {
    pub learning_rate_scale: f32,
    pub weight_decay_scale: f32,
}

impl ParameterGroupSemantics {
    #[must_use]
    pub const fn new(learning_rate_scale: f32, weight_decay_scale: f32) -> Self {
        Self {
            learning_rate_scale,
            weight_decay_scale,
        }
    }
}

impl Default for ParameterGroupSemantics {
    fn default() -> Self {
        Self::new(1.0, 1.0)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OptimizerConfig {
    pub kind: OptimizerKind,
    pub learning_rate: f32,
    pub weight_decay: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gradient_clip_norm: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub momentum: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beta1: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beta2: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epsilon: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_coefficient: Option<f32>,
}

impl OptimizerConfig {
    #[must_use]
    pub fn sgd(learning_rate: f32) -> Self {
        Self::from_training(TrainingOptimizerConfig::sgd(learning_rate))
    }

    #[must_use]
    pub fn adam(learning_rate: f32, beta1: f32, beta2: f32, epsilon: f32) -> Self {
        Self::from_training(TrainingOptimizerConfig::adam(
            learning_rate,
            beta1,
            beta2,
            epsilon,
        ))
    }

    #[must_use]
    pub fn adamw(learning_rate: f32, beta1: f32, beta2: f32, epsilon: f32) -> Self {
        Self::from_training(TrainingOptimizerConfig::adamw(
            learning_rate,
            beta1,
            beta2,
            epsilon,
        ))
    }

    #[must_use]
    pub fn lars(learning_rate: f32, momentum: f32, trust_coefficient: f32, epsilon: f32) -> Self {
        Self::from_training(TrainingOptimizerConfig::lars(
            learning_rate,
            momentum,
            trust_coefficient,
            epsilon,
        ))
    }

    #[must_use]
    pub fn lamb(learning_rate: f32, beta1: f32, beta2: f32, epsilon: f32) -> Self {
        Self::from_training(TrainingOptimizerConfig::lamb(
            learning_rate,
            beta1,
            beta2,
            epsilon,
        ))
    }

    #[must_use]
    pub fn with_weight_decay(mut self, weight_decay: f32) -> Self {
        self.weight_decay = weight_decay;
        self
    }

    #[must_use]
    pub fn with_gradient_clip_norm(mut self, gradient_clip_norm: f32) -> Self {
        self.gradient_clip_norm = Some(gradient_clip_norm);
        self
    }

    #[must_use]
    pub fn with_momentum(mut self, momentum: f32) -> Self {
        self.momentum = Some(momentum);
        self
    }

    #[must_use]
    pub fn with_trust_coefficient(mut self, trust_coefficient: f32) -> Self {
        self.trust_coefficient = Some(trust_coefficient);
        self
    }

    fn from_training(config: TrainingOptimizerConfig) -> Self {
        Self {
            kind: config.kind.into(),
            learning_rate: config.learning_rate,
            weight_decay: config.weight_decay,
            gradient_clip_norm: config.gradient_clip_norm,
            momentum: config.momentum,
            beta1: config.beta1,
            beta2: config.beta2,
            epsilon: config.epsilon,
            trust_coefficient: config.trust_coefficient,
        }
    }

    fn to_training(&self) -> TrainingOptimizerConfig {
        TrainingOptimizerConfig {
            kind: self.kind.into(),
            learning_rate: self.learning_rate,
            weight_decay: self.weight_decay,
            gradient_clip_norm: self.gradient_clip_norm,
            momentum: self.momentum,
            beta1: self.beta1,
            beta2: self.beta2,
            epsilon: self.epsilon,
            trust_coefficient: self.trust_coefficient,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OptimizerParameterState {
    pub spec: TensorSpec,
    pub train_state: TrainingOptimizerState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OptimizerStateSnapshot {
    pub config: OptimizerConfig,
    pub parameter_semantics: ParameterGroupSemantics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<SchedulerBinding>,
    pub parameter_states: BTreeMap<String, OptimizerParameterState>,
    pub state_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OptimizerParameterStepReport {
    pub path: String,
    pub spec: TensorSpec,
    pub optimizer: OptimizerKind,
    pub step_number: u64,
    pub effective_learning_rate: f32,
    pub effective_weight_decay: f32,
    pub update_values: Vec<f32>,
    pub update_norm_l2: f32,
    pub parameter_norm_l2_before: f32,
    pub parameter_norm_l2_after: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_ratio: Option<f32>,
}

impl OptimizerParameterStepReport {
    fn from_training(path: String, spec: TensorSpec, report: TrainingOptimizerStepReport) -> Self {
        Self {
            path,
            spec,
            optimizer: report.optimizer.into(),
            step_number: report.step_number,
            effective_learning_rate: report.effective_learning_rate,
            effective_weight_decay: report.effective_weight_decay,
            update_values: report.update_values,
            update_norm_l2: report.update_norm_l2,
            parameter_norm_l2_before: report.parameter_norm_l2_before,
            parameter_norm_l2_after: report.parameter_norm_l2_after,
            trust_ratio: report.trust_ratio,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OptimizerModuleStepReport {
    pub config: OptimizerConfig,
    pub parameter_semantics: ParameterGroupSemantics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_kind: Option<SchedulerKind>,
    pub step_number: u64,
    pub effective_learning_rate: f32,
    pub effective_weight_decay: f32,
    pub updated_paths: Vec<String>,
    pub ignored_frozen_gradient_paths: Vec<String>,
    pub pruned_state_paths: Vec<String>,
    pub parameter_reports: Vec<OptimizerParameterStepReport>,
    pub state_digest_before: String,
    pub state_digest_after: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Optimizer {
    pub config: OptimizerConfig,
    pub parameter_semantics: ParameterGroupSemantics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<SchedulerBinding>,
    parameter_states: BTreeMap<String, OptimizerParameterState>,
}

impl Optimizer {
    #[must_use]
    pub fn new(config: OptimizerConfig) -> Self {
        Self {
            config,
            parameter_semantics: ParameterGroupSemantics::default(),
            scheduler: None,
            parameter_states: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_parameter_semantics(mut self, semantics: ParameterGroupSemantics) -> Self {
        self.parameter_semantics = semantics;
        self
    }

    #[must_use]
    pub fn with_scheduler(mut self, scheduler: SchedulerConfig) -> Self {
        self.scheduler = Some(SchedulerBinding::new(scheduler));
        self
    }

    #[must_use]
    pub fn state_snapshot(&self) -> OptimizerStateSnapshot {
        let state_digest = stable_optimizer_state_digest(
            &self.config,
            self.parameter_semantics,
            self.scheduler.as_ref(),
            self.parameter_states.iter(),
        );
        OptimizerStateSnapshot {
            config: self.config.clone(),
            parameter_semantics: self.parameter_semantics,
            scheduler: self.scheduler.clone(),
            parameter_states: self.parameter_states.clone(),
            state_digest,
        }
    }

    pub fn restore_state(
        &mut self,
        snapshot: &OptimizerStateSnapshot,
    ) -> Result<(), OptimizerError> {
        if snapshot.config != self.config {
            return Err(OptimizerError::SnapshotConfigMismatch {
                expected: self.config.clone(),
                actual: snapshot.config.clone(),
            });
        }
        for (path, state) in &snapshot.parameter_states {
            validate_spec_support("optimizer_state", path, &state.spec)?;
            let actual_kind = OptimizerKind::from(state.train_state.kind());
            if actual_kind != self.config.kind {
                return Err(OptimizerError::StateKindMismatch {
                    path: path.clone(),
                    expected: self.config.kind,
                    actual: actual_kind,
                });
            }
        }
        self.parameter_semantics = snapshot.parameter_semantics;
        self.scheduler = snapshot.scheduler.clone();
        self.parameter_states = snapshot.parameter_states.clone();
        Ok(())
    }

    pub fn step_module(
        &mut self,
        module: &mut Module,
        gradients: &ModuleStateDict,
        step_number: u64,
    ) -> Result<OptimizerModuleStepReport, OptimizerError> {
        let selected_paths = module
            .named_parameters_with_view(ModuleParameterView::All)
            .into_iter()
            .map(|(path, _)| path)
            .collect::<BTreeSet<_>>();
        self.step_parameter_paths(module, gradients, step_number, &selected_paths)
    }

    #[must_use]
    pub fn state_paths(&self) -> Vec<String> {
        self.parameter_states.keys().cloned().collect()
    }

    fn step_parameter_paths(
        &mut self,
        module: &mut Module,
        gradients: &ModuleStateDict,
        step_number: u64,
        selected_paths: &BTreeSet<String>,
    ) -> Result<OptimizerModuleStepReport, OptimizerError> {
        verify_gradient_root(module, gradients)?;
        let state_digest_before = self.state_snapshot().state_digest;

        let trainable_paths = module
            .named_parameters_with_view(ModuleParameterView::TrainableOnly)
            .into_iter()
            .map(|(path, _)| path)
            .collect::<BTreeSet<_>>();
        let frozen_paths = module
            .named_parameters_with_view(ModuleParameterView::FrozenOnly)
            .into_iter()
            .map(|(path, _)| path)
            .collect::<BTreeSet<_>>();

        let mut selected_trainable = BTreeSet::new();
        let mut selected_frozen = BTreeSet::new();
        for path in selected_paths {
            if trainable_paths.contains(path) {
                selected_trainable.insert(path.clone());
            } else if frozen_paths.contains(path) {
                selected_frozen.insert(path.clone());
            } else {
                return Err(OptimizerError::ModuleState(
                    ModuleStateError::MissingParameter { path: path.clone() },
                ));
            }
        }

        let mut ignored_frozen_gradient_paths = Vec::new();
        for path in gradients.keys() {
            if selected_trainable.contains(&path) {
                continue;
            }
            if selected_frozen.contains(&path) {
                ignored_frozen_gradient_paths.push(path);
                continue;
            }
            return Err(OptimizerError::UnexpectedGradient { path });
        }

        let resolved = self.resolve_training_step(step_number)?;
        let mut parameter_reports = Vec::with_capacity(selected_trainable.len());
        for path in &selected_trainable {
            let gradient_entry = gradients
                .entry(path.as_str())
                .ok_or_else(|| OptimizerError::MissingGradient { path: path.clone() })?;
            if gradient_entry.kind != ModuleStateEntryKind::Parameter {
                return Err(OptimizerError::GradientEntryKindMismatch {
                    path: path.clone(),
                    kind: gradient_entry.kind,
                });
            }

            let parameter_spec = module.parameter(path.as_str())?.spec.clone();
            if gradient_entry.spec != parameter_spec {
                return Err(OptimizerError::GradientTensorMismatch {
                    path: path.clone(),
                    expected: parameter_spec,
                    actual: gradient_entry.spec.clone(),
                });
            }

            validate_entry_support("gradient", path.as_str(), gradient_entry)?;
            let gradient_values =
                dense_cpu_f32_from_entry("gradient", path.as_str(), gradient_entry)?;

            let parameter = parameter_mut(module, path.as_str())?;
            validate_tensor_support("parameter", path.as_str(), &parameter.spec, &parameter.data)?;
            let parameter_len = parameter.spec.shape().element_count();
            let slot = self
                .parameter_states
                .entry(path.clone())
                .or_insert_with(|| OptimizerParameterState {
                    spec: parameter.spec.clone(),
                    train_state: resolved.optimizer.initialize_state(parameter_len),
                });
            if slot.spec != parameter.spec {
                return Err(OptimizerError::StateSpecMismatch {
                    path: path.clone(),
                    expected: slot.spec.clone(),
                    actual: parameter.spec.clone(),
                });
            }

            let parameter_values = dense_cpu_f32_from_tensor_mut(
                "parameter",
                path.as_str(),
                &parameter.spec,
                &mut parameter.data,
            )?;
            let report = resolved.optimizer.apply_step(
                parameter_values,
                gradient_values,
                &mut slot.train_state,
                step_number,
            )?;
            parameter_reports.push(OptimizerParameterStepReport::from_training(
                path.clone(),
                parameter.spec.clone(),
                report,
            ));
        }

        let pruned_state_paths = self
            .parameter_states
            .keys()
            .filter(|path| !selected_trainable.contains(*path))
            .cloned()
            .collect::<Vec<_>>();
        for path in &pruned_state_paths {
            self.parameter_states.remove(path);
        }

        let state_digest_after = self.state_snapshot().state_digest;
        let updated_paths = parameter_reports
            .iter()
            .map(|report| report.path.clone())
            .collect::<Vec<_>>();
        Ok(OptimizerModuleStepReport {
            config: self.config.clone(),
            parameter_semantics: self.parameter_semantics,
            scheduler_kind: resolved.scheduler_kind,
            step_number,
            effective_learning_rate: resolved.effective_learning_rate,
            effective_weight_decay: resolved.effective_weight_decay,
            updated_paths,
            ignored_frozen_gradient_paths,
            pruned_state_paths,
            parameter_reports,
            state_digest_before,
            state_digest_after,
        })
    }

    fn resolve_training_step(
        &mut self,
        step_number: u64,
    ) -> Result<ResolvedOptimizerStep, OptimizerError> {
        let mut optimizer = self.config.to_training();
        optimizer.learning_rate *= self.parameter_semantics.learning_rate_scale;
        optimizer.weight_decay *= self.parameter_semantics.weight_decay_scale;
        let scheduler_kind = self.scheduler.as_ref().map(|binding| binding.config.kind());
        if let Some(binding) = &mut self.scheduler {
            let mut training_binding = binding.to_training();
            optimizer.learning_rate = scheduled_learning_rate(
                &mut training_binding,
                optimizer.learning_rate,
                step_number,
            )?;
            *binding = SchedulerBinding::from_training(&training_binding);
        }
        Ok(ResolvedOptimizerStep {
            effective_learning_rate: optimizer.learning_rate,
            effective_weight_decay: optimizer.weight_decay,
            scheduler_kind,
            optimizer,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OptimizerGroup {
    pub group_id: String,
    pub parameter_paths: BTreeSet<String>,
    pub optimizer: Optimizer,
}

impl OptimizerGroup {
    pub fn new(
        group_id: impl Into<String>,
        parameter_paths: impl IntoIterator<Item = impl Into<String>>,
        optimizer: Optimizer,
    ) -> Result<Self, OptimizerError> {
        let group_id = group_id.into();
        if group_id.trim().is_empty() {
            return Err(OptimizerError::MissingGroupId);
        }
        let mut unique_paths = BTreeSet::new();
        for path in parameter_paths {
            let path = path.into();
            if !unique_paths.insert(path.clone()) {
                return Err(OptimizerError::DuplicateGroupPath {
                    group_id: group_id.clone(),
                    path,
                });
            }
        }
        Ok(Self {
            group_id,
            parameter_paths: unique_paths,
            optimizer,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultiOptimizerGroupStepReport {
    pub group_id: String,
    pub parameter_paths: BTreeSet<String>,
    pub optimizer_report: OptimizerModuleStepReport,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultiOptimizerStepReport {
    pub step_number: u64,
    pub updated_paths: Vec<String>,
    pub ignored_frozen_gradient_paths: Vec<String>,
    pub group_reports: Vec<MultiOptimizerGroupStepReport>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultiOptimizer {
    pub groups: BTreeMap<String, OptimizerGroup>,
}

impl MultiOptimizer {
    pub fn new(groups: Vec<OptimizerGroup>) -> Result<Self, OptimizerError> {
        let mut map = BTreeMap::new();
        let mut assigned_paths = BTreeMap::new();
        for group in groups {
            if map.contains_key(&group.group_id) {
                return Err(OptimizerError::DuplicateGroupId {
                    group_id: group.group_id,
                });
            }
            map.insert(group.group_id.clone(), group.clone());
            for path in &group.parameter_paths {
                if let Some(existing_group_id) =
                    assigned_paths.insert(path.clone(), group.group_id.clone())
                {
                    return Err(OptimizerError::DuplicateParameterAssignment {
                        path: path.clone(),
                        first_group_id: existing_group_id,
                        second_group_id: group.group_id.clone(),
                    });
                }
            }
        }
        Ok(Self { groups: map })
    }

    pub fn step_module(
        &mut self,
        module: &mut Module,
        gradients: &ModuleStateDict,
        step_number: u64,
    ) -> Result<MultiOptimizerStepReport, OptimizerError> {
        verify_gradient_root(module, gradients)?;
        let trainable_paths = module
            .named_parameters_with_view(ModuleParameterView::TrainableOnly)
            .into_iter()
            .map(|(path, _)| path)
            .collect::<BTreeSet<_>>();
        let frozen_paths = module
            .named_parameters_with_view(ModuleParameterView::FrozenOnly)
            .into_iter()
            .map(|(path, _)| path)
            .collect::<BTreeSet<_>>();
        let assigned_paths = self
            .groups
            .values()
            .flat_map(|group| group.parameter_paths.iter().cloned())
            .collect::<BTreeSet<_>>();
        for path in &trainable_paths {
            if !assigned_paths.contains(path) {
                return Err(OptimizerError::UnassignedTrainableParameter { path: path.clone() });
            }
        }

        let mut ignored_frozen = BTreeSet::new();
        for path in gradients.keys() {
            if assigned_paths.contains(&path) {
                continue;
            }
            if frozen_paths.contains(&path) {
                ignored_frozen.insert(path);
                continue;
            }
            return Err(OptimizerError::UnexpectedGradient { path });
        }

        let mut group_reports = Vec::new();
        let mut updated_paths = BTreeSet::new();
        for group in self.groups.values_mut() {
            let subset = ModuleStateDict::new(
                gradients.root_module_id.clone(),
                gradients.root_module_kind.clone(),
                gradients.view,
                gradients
                    .entries
                    .iter()
                    .filter(|(path, _)| group.parameter_paths.contains(*path))
                    .map(|(path, entry)| (path.clone(), entry.clone()))
                    .collect::<BTreeMap<_, _>>(),
            )?;
            let report = group.optimizer.step_parameter_paths(
                module,
                &subset,
                step_number,
                &group.parameter_paths,
            )?;
            updated_paths.extend(report.updated_paths.iter().cloned());
            ignored_frozen.extend(report.ignored_frozen_gradient_paths.iter().cloned());
            group_reports.push(MultiOptimizerGroupStepReport {
                group_id: group.group_id.clone(),
                parameter_paths: group.parameter_paths.clone(),
                optimizer_report: report,
            });
        }

        Ok(MultiOptimizerStepReport {
            step_number,
            updated_paths: updated_paths.into_iter().collect(),
            ignored_frozen_gradient_paths: ignored_frozen.into_iter().collect(),
            group_reports,
        })
    }
}

#[derive(Clone, Debug)]
struct ResolvedOptimizerStep {
    effective_learning_rate: f32,
    effective_weight_decay: f32,
    scheduler_kind: Option<SchedulerKind>,
    optimizer: TrainingOptimizerConfig,
}

fn verify_gradient_root(
    module: &Module,
    gradients: &ModuleStateDict,
) -> Result<(), OptimizerError> {
    if gradients.root_module_id != module.module_id {
        return Err(OptimizerError::GradientRootMismatch {
            expected_module_id: module.module_id.clone(),
            actual_module_id: gradients.root_module_id.clone(),
        });
    }
    if gradients.root_module_kind != module.module_kind {
        return Err(OptimizerError::GradientModuleKindMismatch {
            expected_module_kind: module.module_kind.clone(),
            actual_module_kind: gradients.root_module_kind.clone(),
        });
    }
    Ok(())
}

fn parameter_mut<'a>(
    module: &'a mut Module,
    path: &str,
) -> Result<&'a mut ModuleParameter, ModuleStateError> {
    let (module_path, local_name) = split_parameter_path(path)?;
    let target = module.submodule_mut(module_path.as_str())?;
    target
        .parameters
        .get_mut(local_name.as_str())
        .ok_or_else(|| ModuleStateError::MissingParameter {
            path: String::from(path),
        })
}

fn split_parameter_path(path: &str) -> Result<(String, String), ModuleStateError> {
    if path.trim().is_empty() {
        return Err(ModuleStateError::MissingParameter {
            path: String::from(path),
        });
    }
    match path.rsplit_once('.') {
        Some((module_path, local_name)) => {
            Ok((String::from(module_path), String::from(local_name)))
        }
        None => Ok((String::new(), String::from(path))),
    }
}

fn validate_entry_support(
    role: &'static str,
    path: &str,
    entry: &ModuleStateEntry,
) -> Result<(), OptimizerError> {
    validate_tensor_support(role, path, &entry.spec, &entry.data)
}

fn validate_spec_support(
    role: &'static str,
    path: &str,
    spec: &TensorSpec,
) -> Result<(), OptimizerError> {
    if spec.dtype() != DType::F32 || spec.device().kind() != DeviceKind::Cpu {
        return Err(OptimizerError::UnsupportedTensor {
            role,
            path: String::from(path),
            dtype: spec.dtype(),
            device: spec.device().clone(),
        });
    }
    Ok(())
}

fn validate_tensor_support(
    role: &'static str,
    path: &str,
    spec: &TensorSpec,
    data: &TensorData,
) -> Result<(), OptimizerError> {
    validate_spec_support(role, path, spec)?;
    let values = data
        .as_f32_slice()
        .ok_or_else(|| OptimizerError::UnsupportedTensor {
            role,
            path: String::from(path),
            dtype: spec.dtype(),
            device: spec.device().clone(),
        })?;
    if values.len() != spec.shape().element_count() {
        return Err(OptimizerError::TensorLengthMismatch {
            role,
            path: String::from(path),
            expected_len: spec.shape().element_count(),
            actual_len: values.len(),
        });
    }
    Ok(())
}

fn dense_cpu_f32_from_entry<'a>(
    role: &'static str,
    path: &str,
    entry: &'a ModuleStateEntry,
) -> Result<&'a [f32], OptimizerError> {
    validate_entry_support(role, path, entry)?;
    Ok(entry.data.as_f32_slice().expect("validated dense cpu f32"))
}

fn dense_cpu_f32_from_tensor_mut<'a>(
    role: &'static str,
    path: &str,
    spec: &TensorSpec,
    data: &'a mut TensorData,
) -> Result<&'a mut [f32], OptimizerError> {
    validate_tensor_support(role, path, spec, data)?;
    Ok(match data {
        TensorData::F32(values) => values.as_mut_slice(),
        TensorData::QuantizedBlocks(_) => unreachable!("validated dense cpu f32"),
    })
}

fn stable_optimizer_state_digest<'a>(
    config: &OptimizerConfig,
    parameter_semantics: ParameterGroupSemantics,
    scheduler: Option<&SchedulerBinding>,
    parameter_states: impl Iterator<Item = (&'a String, &'a OptimizerParameterState)>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("kind={:?}", config.kind));
    hasher.update(format!("learning_rate={:.8e}", config.learning_rate));
    hasher.update(format!("weight_decay={:.8e}", config.weight_decay));
    hasher.update(format!(
        "gradient_clip_norm={:?}",
        config
            .gradient_clip_norm
            .map(|value| format!("{value:.8e}"))
    ));
    hasher.update(format!(
        "momentum={:?}",
        config.momentum.map(|value| format!("{value:.8e}"))
    ));
    hasher.update(format!(
        "beta1={:?}",
        config.beta1.map(|value| format!("{value:.8e}"))
    ));
    hasher.update(format!(
        "beta2={:?}",
        config.beta2.map(|value| format!("{value:.8e}"))
    ));
    hasher.update(format!(
        "epsilon={:?}",
        config.epsilon.map(|value| format!("{value:.8e}"))
    ));
    hasher.update(format!(
        "trust_coefficient={:?}",
        config.trust_coefficient.map(|value| format!("{value:.8e}"))
    ));
    hasher.update(format!(
        "lr_scale={:.8e}",
        parameter_semantics.learning_rate_scale
    ));
    hasher.update(format!(
        "weight_decay_scale={:.8e}",
        parameter_semantics.weight_decay_scale
    ));
    if let Some(binding) = scheduler {
        hasher.update(format!("scheduler_kind={:?}", binding.config.kind()));
        hasher.update(format!("scheduler_state_step={}", binding.state.last_step));
        hasher.update(format!(
            "scheduler_last_lr={:?}",
            binding
                .state
                .last_learning_rate
                .map(|value| format!("{value:.8e}"))
        ));
    }
    for (path, state) in parameter_states {
        hasher.update(format!("path={path}"));
        hasher.update(format!("shape={:?}", state.spec.shape().dims()));
        hasher.update(format!("dtype={:?}", state.spec.dtype()));
        hasher.update(format!("device={}", state.spec.device()));
        match &state.train_state {
            TrainingOptimizerState::Sgd { momentum_buffer } => {
                hasher.update("state=sgd");
                if let Some(buffer) = momentum_buffer {
                    for value in buffer {
                        hasher.update(format!("{value:.8e}"));
                    }
                }
            }
            TrainingOptimizerState::Adam {
                first_moment,
                second_moment,
            } => {
                hasher.update("state=adam");
                for value in first_moment {
                    hasher.update(format!("{value:.8e}"));
                }
                for value in second_moment {
                    hasher.update(format!("{value:.8e}"));
                }
            }
            TrainingOptimizerState::AdamW {
                first_moment,
                second_moment,
            } => {
                hasher.update("state=adamw");
                for value in first_moment {
                    hasher.update(format!("{value:.8e}"));
                }
                for value in second_moment {
                    hasher.update(format!("{value:.8e}"));
                }
            }
            TrainingOptimizerState::Lars { momentum_buffer } => {
                hasher.update("state=lars");
                if let Some(buffer) = momentum_buffer {
                    for value in buffer {
                        hasher.update(format!("{value:.8e}"));
                    }
                }
            }
            TrainingOptimizerState::Lamb {
                first_moment,
                second_moment,
            } => {
                hasher.update("state=lamb");
                for value in first_moment {
                    hasher.update(format!("{value:.8e}"));
                }
                for value in second_moment {
                    hasher.update(format!("{value:.8e}"));
                }
            }
        }
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        MultiOptimizer, Optimizer, OptimizerConfig, OptimizerError, OptimizerGroup,
        ParameterGroupSemantics, SchedulerConfig,
    };
    use crate::{
        Module, ModuleParameter, ModuleStateDict, ModuleStateEntry, ModuleStateEntryKind,
        ModuleStateView,
    };
    use psionic_core::{DType, Device, Shape, TensorData, TensorSpec};

    fn dense_parameter(
        dims: &[usize],
        values: Vec<f32>,
        requires_grad: bool,
    ) -> Result<ModuleParameter, Box<dyn std::error::Error>> {
        Ok(ModuleParameter::new(
            TensorSpec::new(Shape::new(dims.to_vec()), DType::F32, Device::cpu()),
            TensorData::F32(values),
            requires_grad,
        )?)
    }

    fn gradient_entry(path: &str, dims: &[usize], values: Vec<f32>) -> ModuleStateEntry {
        ModuleStateEntry {
            path: String::from(path),
            kind: ModuleStateEntryKind::Parameter,
            spec: TensorSpec::new(Shape::new(dims.to_vec()), DType::F32, Device::cpu()),
            data: TensorData::F32(values),
            requires_grad: true,
            persistent: true,
        }
    }

    fn gradient_dict(
        root: &Module,
        entries: Vec<ModuleStateEntry>,
    ) -> Result<ModuleStateDict, Box<dyn std::error::Error>> {
        Ok(ModuleStateDict::new(
            root.module_id.clone(),
            root.module_kind.clone(),
            ModuleStateView::PersistentOnly,
            entries
                .into_iter()
                .map(|entry| (entry.path.clone(), entry))
                .collect(),
        )?)
    }

    #[test]
    fn module_optimizer_updates_trainable_parameters_and_ignores_frozen_gradients()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut module = Module::new("model", "toy")?;
        module.insert_parameter("weight", dense_parameter(&[2], vec![1.0, -1.0], true)?)?;
        module.insert_parameter("bias", dense_parameter(&[2], vec![0.5, -0.5], false)?)?;

        let gradients = gradient_dict(
            &module,
            vec![
                gradient_entry("weight", &[2], vec![0.2, -0.2]),
                gradient_entry("bias", &[2], vec![0.4, -0.4]),
            ],
        )?;
        let mut optimizer =
            Optimizer::new(OptimizerConfig::adamw(0.05, 0.9, 0.999, 1e-8).with_weight_decay(0.01));
        let report = optimizer.step_module(&mut module, &gradients, 1)?;

        assert_eq!(report.updated_paths, vec![String::from("weight")]);
        assert_eq!(
            report.ignored_frozen_gradient_paths,
            vec![String::from("bias")]
        );
        assert!(report.pruned_state_paths.is_empty());
        assert_eq!(optimizer.state_paths(), vec![String::from("weight")]);
        assert_eq!(
            module.parameter("weight")?.data,
            TensorData::F32(vec![0.9495, -0.9495])
        );
        assert_eq!(
            module.parameter("bias")?.data,
            TensorData::F32(vec![0.5, -0.5])
        );
        Ok(())
    }

    #[test]
    fn module_optimizer_state_snapshot_roundtrips_without_losing_momentum()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut module_a = Module::new("model", "toy")?;
        module_a.insert_parameter("weight", dense_parameter(&[2], vec![1.0, -1.0], true)?)?;
        let gradients = gradient_dict(
            &module_a,
            vec![gradient_entry("weight", &[2], vec![0.25, -0.25])],
        )?;

        let mut optimizer_a = Optimizer::new(OptimizerConfig::sgd(0.1).with_momentum(0.9));
        optimizer_a.step_module(&mut module_a, &gradients, 1)?;
        let snapshot = optimizer_a.state_snapshot();
        let mut module_b = module_a.clone();

        let mut optimizer_b = Optimizer::new(OptimizerConfig::sgd(0.1).with_momentum(0.9));
        optimizer_b.restore_state(&snapshot)?;

        let report_a = optimizer_a.step_module(&mut module_a, &gradients, 2)?;
        let report_b = optimizer_b.step_module(&mut module_b, &gradients, 2)?;

        assert_eq!(module_a, module_b);
        assert_eq!(report_a.parameter_reports, report_b.parameter_reports);
        assert_eq!(
            optimizer_a.state_snapshot().state_digest,
            optimizer_b.state_snapshot().state_digest
        );
        Ok(())
    }

    #[test]
    fn module_optimizer_scheduler_and_parameter_semantics_scale_effective_rates()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut module = Module::new("model", "toy")?;
        module.insert_parameter("weight", dense_parameter(&[2], vec![1.0, -1.0], true)?)?;
        let gradients = gradient_dict(
            &module,
            vec![gradient_entry("weight", &[2], vec![0.25, -0.25])],
        )?;

        let mut optimizer = Optimizer::new(OptimizerConfig::sgd(0.1).with_weight_decay(0.2))
            .with_parameter_semantics(ParameterGroupSemantics::new(0.5, 2.0))
            .with_scheduler(SchedulerConfig::step_lr(2, 0.1));

        let step_one = optimizer.step_module(&mut module, &gradients, 1)?;
        assert!((step_one.effective_learning_rate - 0.05).abs() < 1e-6);
        assert!((step_one.effective_weight_decay - 0.4).abs() < 1e-6);
        assert_eq!(step_one.scheduler_kind, Some(super::SchedulerKind::StepLr));

        let step_two = optimizer.step_module(&mut module, &gradients, 2)?;
        assert!((step_two.effective_learning_rate - 0.05).abs() < 1e-6);

        let step_three = optimizer.step_module(&mut module, &gradients, 3)?;
        assert!((step_three.effective_learning_rate - 0.005).abs() < 1e-6);
        let scheduler = optimizer
            .state_snapshot()
            .scheduler
            .expect("scheduler should remain attached");
        assert_eq!(scheduler.state.last_step, 3);
        assert!(
            (scheduler
                .state
                .last_learning_rate
                .expect("scheduler should track last lr")
                - 0.005)
                .abs()
                < 1e-6
        );
        Ok(())
    }

    #[test]
    fn multi_optimizer_composes_disjoint_groups_and_refuses_overlap_or_unassigned_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut module = Module::new("model", "toy")?;
        module.insert_parameter("encoder", dense_parameter(&[2], vec![1.0, -1.0], true)?)?;
        module.insert_parameter("head", dense_parameter(&[2], vec![0.5, -0.5], true)?)?;
        let gradients = gradient_dict(
            &module,
            vec![
                gradient_entry("encoder", &[2], vec![0.25, -0.25]),
                gradient_entry("head", &[2], vec![0.2, -0.2]),
            ],
        )?;

        let encoder_group = OptimizerGroup::new(
            "encoder_group",
            [String::from("encoder")],
            Optimizer::new(OptimizerConfig::sgd(0.1).with_momentum(0.9))
                .with_scheduler(SchedulerConfig::linear_warmup(2, 0.5)),
        )?;
        let head_group = OptimizerGroup::new(
            "head_group",
            [String::from("head")],
            Optimizer::new(OptimizerConfig::adam(0.05, 0.9, 0.999, 1e-8)),
        )?;
        let mut multi = MultiOptimizer::new(vec![encoder_group, head_group])?;
        let report = multi.step_module(&mut module, &gradients, 1)?;

        assert_eq!(
            report.updated_paths,
            vec![String::from("encoder"), String::from("head")]
        );
        assert_eq!(report.group_reports.len(), 2);
        assert_eq!(
            module.parameter("encoder")?.data,
            TensorData::F32(vec![0.98125, -0.98125])
        );
        assert_eq!(
            module.parameter("head")?.data,
            TensorData::F32(vec![0.45, -0.45])
        );

        let overlapping = MultiOptimizer::new(vec![
            OptimizerGroup::new(
                "g0",
                [String::from("encoder")],
                Optimizer::new(OptimizerConfig::sgd(0.1)),
            )?,
            OptimizerGroup::new(
                "g1",
                [String::from("encoder")],
                Optimizer::new(OptimizerConfig::adam(0.05, 0.9, 0.999, 1e-8)),
            )?,
        ]);
        assert_eq!(
            overlapping,
            Err(OptimizerError::DuplicateParameterAssignment {
                path: String::from("encoder"),
                first_group_id: String::from("g0"),
                second_group_id: String::from("g1"),
            })
        );

        let mut unassigned = MultiOptimizer::new(vec![OptimizerGroup::new(
            "encoder_only",
            [String::from("encoder")],
            Optimizer::new(OptimizerConfig::sgd(0.1)),
        )?])?;
        assert_eq!(
            unassigned.step_module(&mut module, &gradients, 1),
            Err(OptimizerError::UnassignedTrainableParameter {
                path: String::from("head"),
            })
        );
        Ok(())
    }

    #[test]
    fn module_optimizer_refuses_missing_unknown_and_spec_drift_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut module = Module::new("model", "toy")?;
        module.insert_parameter("weight", dense_parameter(&[2], vec![1.0, -1.0], true)?)?;
        let mut optimizer = Optimizer::new(OptimizerConfig::adam(0.05, 0.9, 0.999, 1e-8));

        let missing = gradient_dict(&module, Vec::new())?;
        assert_eq!(
            optimizer.step_module(&mut module, &missing, 1),
            Err(OptimizerError::MissingGradient {
                path: String::from("weight"),
            })
        );

        let unknown = gradient_dict(
            &module,
            vec![gradient_entry("unknown", &[2], vec![0.1, -0.1])],
        )?;
        assert_eq!(
            optimizer.step_module(&mut module, &unknown, 1),
            Err(OptimizerError::UnexpectedGradient {
                path: String::from("unknown"),
            })
        );

        let valid = gradient_dict(
            &module,
            vec![gradient_entry("weight", &[2], vec![0.2, -0.2])],
        )?;
        optimizer.step_module(&mut module, &valid, 1)?;
        let snapshot = optimizer.state_snapshot();

        let mut resized = Module::new("model", "toy")?;
        resized.insert_parameter("weight", dense_parameter(&[3], vec![1.0, -1.0, 0.5], true)?)?;
        let mut restored = Optimizer::new(OptimizerConfig::adam(0.05, 0.9, 0.999, 1e-8));
        restored.restore_state(&snapshot)?;
        let resized_gradients = gradient_dict(
            &resized,
            vec![gradient_entry("weight", &[3], vec![0.2, -0.2, 0.1])],
        )?;
        assert_eq!(
            restored.step_module(&mut resized, &resized_gradients, 2),
            Err(OptimizerError::StateSpecMismatch {
                path: String::from("weight"),
                expected: TensorSpec::new(Shape::new(vec![2]), DType::F32, Device::cpu()),
                actual: TensorSpec::new(Shape::new(vec![3]), DType::F32, Device::cpu()),
            })
        );
        Ok(())
    }
}
