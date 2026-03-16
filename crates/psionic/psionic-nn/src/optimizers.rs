use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Module, ModuleParameterView, ModuleStateDict, ModuleStateEntry, ModuleStateEntryKind,
    ModuleStateError,
};
use psionic_core::{DType, Device, DeviceKind, TensorData, TensorSpec};
use psionic_train::{
    TrainingOptimizerConfig, TrainingOptimizerError, TrainingOptimizerKind, TrainingOptimizerState,
    TrainingOptimizerStepReport,
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
    pub step_number: u64,
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
    parameter_states: BTreeMap<String, OptimizerParameterState>,
}

impl Optimizer {
    #[must_use]
    pub fn new(config: OptimizerConfig) -> Self {
        Self {
            config,
            parameter_states: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn state_snapshot(&self) -> OptimizerStateSnapshot {
        let state_digest =
            stable_optimizer_state_digest(&self.config, self.parameter_states.iter());
        OptimizerStateSnapshot {
            config: self.config.clone(),
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
        self.parameter_states = snapshot.parameter_states.clone();
        Ok(())
    }

    pub fn step_module(
        &mut self,
        module: &mut Module,
        gradients: &ModuleStateDict,
        step_number: u64,
    ) -> Result<OptimizerModuleStepReport, OptimizerError> {
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

        let mut ignored_frozen_gradient_paths = Vec::new();
        for path in gradients.keys() {
            if trainable_paths.contains(&path) {
                continue;
            }
            if frozen_paths.contains(&path) {
                ignored_frozen_gradient_paths.push(path);
                continue;
            }
            return Err(OptimizerError::UnexpectedGradient { path });
        }

        let training_config = self.config.to_training();
        let mut parameter_reports = Vec::with_capacity(trainable_paths.len());
        for path in &trainable_paths {
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

            let parameter = module.parameter_mut(path.as_str())?;
            validate_tensor_support("parameter", path.as_str(), &parameter.spec, &parameter.data)?;
            let parameter_len = parameter.spec.shape().element_count();
            let slot = self
                .parameter_states
                .entry(path.clone())
                .or_insert_with(|| OptimizerParameterState {
                    spec: parameter.spec.clone(),
                    train_state: training_config.initialize_state(parameter_len),
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
            let report = training_config.apply_step(
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
            .filter(|path| !trainable_paths.contains(*path))
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
            step_number,
            updated_paths,
            ignored_frozen_gradient_paths,
            pruned_state_paths,
            parameter_reports,
            state_digest_before,
            state_digest_after,
        })
    }

    #[must_use]
    pub fn state_paths(&self) -> Vec<String> {
        self.parameter_states.keys().cloned().collect()
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
    use super::{Optimizer, OptimizerConfig, OptimizerError};
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
