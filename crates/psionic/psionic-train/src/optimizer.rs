use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::core_loop::{
    TrainingOptimizerConfig, TrainingOptimizerKind, TrainingOptimizerState,
    TrainingSchedulerBinding, TrainingSchedulerConfig, TrainingSchedulerKind,
};

/// Error returned by the reusable optimizer surface.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrainingOptimizerError {
    /// Parameter and gradient vectors had different lengths.
    #[error(
        "training optimizer `{optimizer:?}` expected gradient length {parameter_len} but found {gradient_len}"
    )]
    GradientLengthMismatch {
        /// Optimizer family.
        optimizer: TrainingOptimizerKind,
        /// Parameter length.
        parameter_len: usize,
        /// Gradient length.
        gradient_len: usize,
    },
    /// The caller supplied the wrong state variant for the optimizer.
    #[error("training optimizer `{optimizer:?}` cannot use state variant `{state_kind:?}`")]
    StateKindMismatch {
        /// Optimizer family.
        optimizer: TrainingOptimizerKind,
        /// State variant currently attached.
        state_kind: TrainingOptimizerKind,
    },
    /// The state vectors did not match the parameter length.
    #[error(
        "training optimizer `{optimizer:?}` expected state length {expected_len} but found {actual_len}"
    )]
    StateLengthMismatch {
        /// Optimizer family.
        optimizer: TrainingOptimizerKind,
        /// Expected vector length.
        expected_len: usize,
        /// Actual vector length.
        actual_len: usize,
    },
    /// One scheduler binding carried an invalid configuration.
    #[error("training scheduler `{scheduler:?}` is invalid: {message}")]
    InvalidSchedulerConfig {
        /// Scheduler family.
        scheduler: TrainingSchedulerKind,
        /// Human-readable reason.
        message: String,
    },
}

/// Outcome status for one optimizer parity case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizerParityStatus {
    /// The optimizer matched the bounded expected contract.
    Supported,
    /// The optimizer refused explicitly under the bounded contract.
    Refused,
}

/// Compact typed snapshot of optimizer state after one parity case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OptimizerParityStateSnapshot {
    /// Optimizer family this state belongs to.
    pub optimizer: TrainingOptimizerKind,
    /// Optional momentum buffer for SGD or LARS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub momentum_buffer: Option<Vec<f32>>,
    /// First-moment state for Adam-family optimizers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_moment: Option<Vec<f32>>,
    /// Second-moment state for Adam-family optimizers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub second_moment: Option<Vec<f32>>,
}

/// One machine-readable seeded optimizer parity case result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OptimizerParityCaseResult {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable oracle family label.
    pub oracle_family: String,
    /// Stable optimizer label.
    pub optimizer: TrainingOptimizerKind,
    /// Stable capability-profile label.
    pub capability_profile: String,
    /// One-based step number used by the case.
    pub step_number: u64,
    /// Initial parameter values used for the case.
    pub initial_parameters: Vec<f32>,
    /// Gradient values used for the case.
    pub gradients: Vec<f32>,
    /// Expected parameter values after the step when supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_parameters_after: Option<Vec<f32>>,
    /// Actual parameter values after the step when supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_parameters_after: Option<Vec<f32>>,
    /// Expected step report when the case is supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_report: Option<TrainingOptimizerStepReport>,
    /// Actual step report when the case is supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_report: Option<TrainingOptimizerStepReport>,
    /// Expected optimizer-state snapshot after the step when supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_state_after: Option<OptimizerParityStateSnapshot>,
    /// Actual optimizer-state snapshot after the step when supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_state_after: Option<OptimizerParityStateSnapshot>,
    /// Expected refusal for intentionally unsupported behavior.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_refusal: Option<TrainingOptimizerError>,
    /// Actual refusal surfaced by the implementation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_refusal: Option<TrainingOptimizerError>,
    /// Stable parity outcome status.
    pub status: OptimizerParityStatus,
}

/// Machine-readable seeded optimizer parity matrix.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OptimizerParityMatrixReport {
    /// Stable schema version for the parity matrix report.
    pub schema_version: u32,
    /// Stable oracle family window label.
    pub oracle_family_window: String,
    /// Seeded parity case results.
    pub cases: Vec<OptimizerParityCaseResult>,
    /// Stable digest over the report contents.
    pub matrix_digest: String,
}

impl OptimizerParityMatrixReport {
    fn new(oracle_family_window: impl Into<String>, cases: Vec<OptimizerParityCaseResult>) -> Self {
        let oracle_family_window = oracle_family_window.into();
        let matrix_digest =
            stable_optimizer_parity_matrix_digest(oracle_family_window.as_str(), cases.as_slice());
        Self {
            schema_version: 1,
            oracle_family_window,
            cases,
            matrix_digest,
        }
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("oracle_family_window={}", self.oracle_family_window),
            format!("matrix_digest={}", self.matrix_digest),
        ];
        for case in &self.cases {
            lines.push(format!(
                "{}|{:?}|{:?}",
                case.case_id, case.optimizer, case.status
            ));
        }
        lines
    }
}

/// Inspectable result of one reusable optimizer step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingOptimizerStepReport {
    /// Optimizer family that applied the update.
    pub optimizer: TrainingOptimizerKind,
    /// One-based step count used for the update.
    pub step_number: u64,
    /// Effective learning rate for the step after group/scheduler resolution.
    pub effective_learning_rate: f32,
    /// Effective weight decay for the step after group resolution.
    pub effective_weight_decay: f32,
    /// Concrete update values applied to each parameter element.
    pub update_values: Vec<f32>,
    /// L2 norm of the update vector.
    pub update_norm_l2: f32,
    /// Parameter L2 norm before the update.
    pub parameter_norm_l2_before: f32,
    /// Parameter L2 norm after the update.
    pub parameter_norm_l2_after: f32,
    /// Effective trust ratio for LARS/LAMB when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_ratio: Option<f32>,
}

impl TrainingOptimizerKind {
    /// Returns whether this optimizer carries a momentum buffer.
    #[must_use]
    pub const fn uses_momentum_buffer(self) -> bool {
        matches!(self, Self::Sgd | Self::Lars)
    }

    /// Returns whether this optimizer carries Adam-family moments.
    #[must_use]
    pub const fn uses_adam_moments(self) -> bool {
        matches!(self, Self::Adam | Self::AdamW | Self::Lamb)
    }
}

impl TrainingOptimizerState {
    /// Returns the optimizer family this state belongs to.
    #[must_use]
    pub const fn kind(&self) -> TrainingOptimizerKind {
        match self {
            Self::Sgd { .. } => TrainingOptimizerKind::Sgd,
            Self::Adam { .. } => TrainingOptimizerKind::Adam,
            Self::AdamW { .. } => TrainingOptimizerKind::AdamW,
            Self::Lars { .. } => TrainingOptimizerKind::Lars,
            Self::Lamb { .. } => TrainingOptimizerKind::Lamb,
        }
    }
}

impl TrainingOptimizerConfig {
    /// Initializes typed optimizer state for one parameter vector.
    #[must_use]
    pub fn initialize_state(&self, parameter_len: usize) -> TrainingOptimizerState {
        TrainingOptimizerState::new(self.kind, parameter_len, self.momentum)
    }

    /// Applies one optimizer step through the reusable optimizer surface.
    pub fn apply_step(
        &self,
        parameter_values: &mut [f32],
        gradient_values: &[f32],
        optimizer_state: &mut TrainingOptimizerState,
        step_number: u64,
    ) -> Result<TrainingOptimizerStepReport, TrainingOptimizerError> {
        apply_training_optimizer_step(
            parameter_values,
            gradient_values,
            self,
            optimizer_state,
            step_number,
        )
    }
}

/// Returns the seeded optimizer parity matrix report for the reusable Psionic
/// training optimizer surface.
pub fn builtin_optimizer_parity_matrix_report()
-> Result<OptimizerParityMatrixReport, TrainingOptimizerError> {
    let cases = vec![
        run_optimizer_parity_supported_case(
            "pytorch.sgd.momentum_step1",
            "single_tensor_step",
            TrainingOptimizerConfig::sgd(0.1).with_momentum(0.9),
            vec![1.0, -1.0],
            vec![0.25, -0.25],
            1,
            vec![0.975, -0.975],
            TrainingOptimizerStepReport {
                optimizer: TrainingOptimizerKind::Sgd,
                step_number: 1,
                effective_learning_rate: 0.1,
                effective_weight_decay: 0.0,
                update_values: vec![0.025, -0.025],
                update_norm_l2: 0.03535534,
                parameter_norm_l2_before: 1.4142135,
                parameter_norm_l2_after: 1.3788582,
                trust_ratio: None,
            },
            OptimizerParityStateSnapshot {
                optimizer: TrainingOptimizerKind::Sgd,
                momentum_buffer: Some(vec![0.25, -0.25]),
                first_moment: None,
                second_moment: None,
            },
        )?,
        run_optimizer_parity_supported_case(
            "pytorch.adam.step1",
            "single_tensor_step",
            TrainingOptimizerConfig::adam(0.05, 0.9, 0.999, 1e-8),
            vec![1.0, -1.0],
            vec![0.2, -0.2],
            1,
            vec![0.95, -0.95],
            TrainingOptimizerStepReport {
                optimizer: TrainingOptimizerKind::Adam,
                step_number: 1,
                effective_learning_rate: 0.05,
                effective_weight_decay: 0.0,
                update_values: vec![0.05, -0.05],
                update_norm_l2: 0.070710675,
                parameter_norm_l2_before: 1.4142135,
                parameter_norm_l2_after: 1.3435029,
                trust_ratio: None,
            },
            OptimizerParityStateSnapshot {
                optimizer: TrainingOptimizerKind::Adam,
                momentum_buffer: None,
                first_moment: Some(vec![0.02, -0.02]),
                second_moment: Some(vec![0.00004, 0.00004]),
            },
        )?,
        run_optimizer_parity_supported_case(
            "pytorch.adamw.decoupled_weight_decay_step1",
            "single_tensor_step",
            TrainingOptimizerConfig::adamw(0.05, 0.9, 0.999, 1e-8).with_weight_decay(0.01),
            vec![1.0, -1.0],
            vec![0.2, -0.2],
            1,
            vec![0.9495, -0.9495],
            TrainingOptimizerStepReport {
                optimizer: TrainingOptimizerKind::AdamW,
                step_number: 1,
                effective_learning_rate: 0.05,
                effective_weight_decay: 0.01,
                update_values: vec![0.0505, -0.0505],
                update_norm_l2: 0.07141778,
                parameter_norm_l2_before: 1.4142135,
                parameter_norm_l2_after: 1.3427957,
                trust_ratio: None,
            },
            OptimizerParityStateSnapshot {
                optimizer: TrainingOptimizerKind::AdamW,
                momentum_buffer: None,
                first_moment: Some(vec![0.02, -0.02]),
                second_moment: Some(vec![0.00004, 0.00004]),
            },
        )?,
        run_optimizer_parity_supported_case(
            "pytorch.lars.trust_ratio_step1",
            "trust_ratio_step",
            TrainingOptimizerConfig::lars(0.1, 0.9, 0.001, 1e-8).with_weight_decay(0.01),
            vec![1.0, -1.0],
            vec![0.2, -0.2],
            1,
            vec![0.9999, -0.9999],
            TrainingOptimizerStepReport {
                optimizer: TrainingOptimizerKind::Lars,
                step_number: 1,
                effective_learning_rate: 0.1,
                effective_weight_decay: 0.01,
                update_values: vec![0.0001, -0.0001],
                update_norm_l2: 0.00014142135,
                parameter_norm_l2_before: 1.4142135,
                parameter_norm_l2_after: 1.4140722,
                trust_ratio: Some(0.0047619046),
            },
            OptimizerParityStateSnapshot {
                optimizer: TrainingOptimizerKind::Lars,
                momentum_buffer: Some(vec![0.001, -0.001]),
                first_moment: None,
                second_moment: None,
            },
        )?,
        run_optimizer_parity_supported_case(
            "pytorch.lamb.trust_ratio_step1",
            "trust_ratio_step",
            TrainingOptimizerConfig::lamb(0.05, 0.9, 0.999, 1e-6).with_weight_decay(0.01),
            vec![1.0, -1.0],
            vec![0.2, -0.2],
            1,
            vec![0.95, -0.95],
            TrainingOptimizerStepReport {
                optimizer: TrainingOptimizerKind::Lamb,
                step_number: 1,
                effective_learning_rate: 0.05,
                effective_weight_decay: 0.01,
                update_values: vec![0.05, -0.05],
                update_norm_l2: 0.07071063,
                parameter_norm_l2_before: 1.4142135,
                parameter_norm_l2_after: 1.3435029,
                trust_ratio: Some(0.99010324),
            },
            OptimizerParityStateSnapshot {
                optimizer: TrainingOptimizerKind::Lamb,
                momentum_buffer: None,
                first_moment: Some(vec![0.02, -0.02]),
                second_moment: Some(vec![0.00004, 0.00004]),
            },
        )?,
        run_optimizer_parity_refusal_case(
            "pytorch.adamw.state_kind_mismatch",
            "state_kind_match_required",
            TrainingOptimizerConfig::adamw(0.05, 0.9, 0.999, 1e-8).with_weight_decay(0.01),
            vec![1.0, -1.0],
            vec![0.2, -0.2],
            TrainingOptimizerState::Sgd {
                momentum_buffer: None,
            },
            1,
            TrainingOptimizerError::StateKindMismatch {
                optimizer: TrainingOptimizerKind::AdamW,
                state_kind: TrainingOptimizerKind::Sgd,
            },
        ),
    ];

    Ok(OptimizerParityMatrixReport::new(
        "pytorch_optim_db_seed_v0",
        cases,
    ))
}

/// Applies one reusable optimizer step over canonical Psionic parameter/gradient buffers.
pub fn apply_training_optimizer_step(
    parameter_values: &mut [f32],
    gradient_values: &[f32],
    optimizer: &TrainingOptimizerConfig,
    optimizer_state: &mut TrainingOptimizerState,
    step_number: u64,
) -> Result<TrainingOptimizerStepReport, TrainingOptimizerError> {
    if parameter_values.len() != gradient_values.len() {
        return Err(TrainingOptimizerError::GradientLengthMismatch {
            optimizer: optimizer.kind,
            parameter_len: parameter_values.len(),
            gradient_len: gradient_values.len(),
        });
    }
    if optimizer.kind != optimizer_state.kind() {
        return Err(TrainingOptimizerError::StateKindMismatch {
            optimizer: optimizer.kind,
            state_kind: optimizer_state.kind(),
        });
    }

    let parameter_norm_l2_before = norm_l2(parameter_values);
    let (update_values, trust_ratio) = match optimizer_state {
        TrainingOptimizerState::Sgd { momentum_buffer } => {
            if let Some(buffer) = momentum_buffer.as_ref() {
                validate_state_len(optimizer.kind, parameter_values.len(), buffer.len())?;
            }
            (
                sgd_like_updates(
                    parameter_values,
                    gradient_values,
                    optimizer,
                    momentum_buffer,
                ),
                None,
            )
        }
        TrainingOptimizerState::Adam {
            first_moment,
            second_moment,
        } => {
            validate_adam_state_len(
                optimizer.kind,
                parameter_values.len(),
                first_moment,
                second_moment,
            )?;
            (
                adam_family_updates(
                    parameter_values,
                    gradient_values,
                    optimizer,
                    first_moment,
                    second_moment,
                    step_number,
                    AdamWeightDecayMode::Coupled,
                ),
                None,
            )
        }
        TrainingOptimizerState::AdamW {
            first_moment,
            second_moment,
        } => {
            validate_adam_state_len(
                optimizer.kind,
                parameter_values.len(),
                first_moment,
                second_moment,
            )?;
            (
                adam_family_updates(
                    parameter_values,
                    gradient_values,
                    optimizer,
                    first_moment,
                    second_moment,
                    step_number,
                    AdamWeightDecayMode::Decoupled,
                ),
                None,
            )
        }
        TrainingOptimizerState::Lars { momentum_buffer } => {
            if let Some(buffer) = momentum_buffer.as_ref() {
                validate_state_len(optimizer.kind, parameter_values.len(), buffer.len())?;
            }
            let (updates, trust_ratio) = lars_updates(
                parameter_values,
                gradient_values,
                optimizer,
                momentum_buffer,
            );
            (updates, Some(trust_ratio))
        }
        TrainingOptimizerState::Lamb {
            first_moment,
            second_moment,
        } => {
            validate_adam_state_len(
                optimizer.kind,
                parameter_values.len(),
                first_moment,
                second_moment,
            )?;
            let (updates, trust_ratio) = lamb_updates(
                parameter_values,
                gradient_values,
                optimizer,
                first_moment,
                second_moment,
                step_number,
            );
            (updates, Some(trust_ratio))
        }
    };

    Ok(TrainingOptimizerStepReport {
        optimizer: optimizer.kind,
        step_number,
        effective_learning_rate: optimizer.learning_rate,
        effective_weight_decay: optimizer.weight_decay,
        update_norm_l2: norm_l2(update_values.as_slice()),
        parameter_norm_l2_before,
        parameter_norm_l2_after: norm_l2(parameter_values),
        update_values,
        trust_ratio,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AdamWeightDecayMode {
    Coupled,
    Decoupled,
}

/// Resolves the current learning rate for one scheduler binding and advances its state.
pub fn scheduled_learning_rate(
    binding: &mut TrainingSchedulerBinding,
    base_learning_rate: f32,
    step_number: u64,
) -> Result<f32, TrainingOptimizerError> {
    let learning_rate =
        resolve_scheduler_learning_rate(&binding.config, base_learning_rate, step_number)?;
    binding.state.last_step = step_number;
    binding.state.last_learning_rate = Some(learning_rate);
    Ok(learning_rate)
}

fn resolve_scheduler_learning_rate(
    config: &TrainingSchedulerConfig,
    base_learning_rate: f32,
    step_number: u64,
) -> Result<f32, TrainingOptimizerError> {
    match config {
        TrainingSchedulerConfig::Constant => Ok(base_learning_rate),
        TrainingSchedulerConfig::StepLr { step_size, gamma } => {
            if *step_size == 0 {
                return Err(TrainingOptimizerError::InvalidSchedulerConfig {
                    scheduler: TrainingSchedulerKind::StepLr,
                    message: String::from("step_size must be greater than zero"),
                });
            }
            let decay_events = step_number.saturating_sub(1) / *step_size;
            Ok(base_learning_rate * gamma.powf(decay_events as f32))
        }
        TrainingSchedulerConfig::LinearWarmup {
            warmup_steps,
            start_factor,
        } => {
            if *warmup_steps == 0 {
                return Err(TrainingOptimizerError::InvalidSchedulerConfig {
                    scheduler: TrainingSchedulerKind::LinearWarmup,
                    message: String::from("warmup_steps must be greater than zero"),
                });
            }
            let progress = (step_number.min(*warmup_steps) as f32) / (*warmup_steps as f32);
            Ok(base_learning_rate * (start_factor + ((1.0 - start_factor) * progress)))
        }
        TrainingSchedulerConfig::CosineAnnealing {
            total_steps,
            min_learning_rate,
        } => {
            if *total_steps == 0 {
                return Err(TrainingOptimizerError::InvalidSchedulerConfig {
                    scheduler: TrainingSchedulerKind::CosineAnnealing,
                    message: String::from("total_steps must be greater than zero"),
                });
            }
            let capped_step = step_number
                .saturating_sub(1)
                .min(total_steps.saturating_sub(1));
            let progress = if *total_steps <= 1 {
                1.0
            } else {
                (capped_step as f32) / ((total_steps - 1) as f32)
            };
            let cosine = (std::f32::consts::PI * progress).cos();
            Ok(min_learning_rate
                + ((base_learning_rate - min_learning_rate) * 0.5 * (1.0 + cosine)))
        }
    }
}

fn sgd_like_updates(
    parameter_values: &mut [f32],
    gradient_values: &[f32],
    optimizer: &TrainingOptimizerConfig,
    momentum_buffer: &mut Option<Vec<f32>>,
) -> Vec<f32> {
    let mut updates = vec![0.0; parameter_values.len()];
    for index in 0..parameter_values.len() {
        let effective_gradient =
            gradient_values[index] + (optimizer.weight_decay * parameter_values[index]);
        let velocity = if let Some(buffer) = momentum_buffer.as_mut() {
            let momentum = optimizer.momentum.unwrap_or(0.0);
            buffer[index] = (momentum * buffer[index]) + effective_gradient;
            buffer[index]
        } else {
            effective_gradient
        };
        let update = optimizer.learning_rate * velocity;
        parameter_values[index] -= update;
        updates[index] = update;
    }
    updates
}

fn adam_family_updates(
    parameter_values: &mut [f32],
    gradient_values: &[f32],
    optimizer: &TrainingOptimizerConfig,
    first_moment: &mut [f32],
    second_moment: &mut [f32],
    step_number: u64,
    weight_decay_mode: AdamWeightDecayMode,
) -> Vec<f32> {
    let beta1 = optimizer.beta1.unwrap_or(0.9);
    let beta2 = optimizer.beta2.unwrap_or(0.999);
    let epsilon = optimizer.epsilon.unwrap_or(1e-8);
    let bias_correction1 = 1.0 - beta1.powf(step_number as f32);
    let bias_correction2 = 1.0 - beta2.powf(step_number as f32);
    let mut updates = vec![0.0; parameter_values.len()];
    for index in 0..parameter_values.len() {
        let gradient = match weight_decay_mode {
            AdamWeightDecayMode::Coupled => {
                gradient_values[index] + (optimizer.weight_decay * parameter_values[index])
            }
            AdamWeightDecayMode::Decoupled => gradient_values[index],
        };
        first_moment[index] = (beta1 * first_moment[index]) + ((1.0 - beta1) * gradient);
        second_moment[index] =
            (beta2 * second_moment[index]) + ((1.0 - beta2) * gradient * gradient);
        let m_hat = first_moment[index] / bias_correction1.max(f32::EPSILON);
        let v_hat = second_moment[index] / bias_correction2.max(f32::EPSILON);
        let direction = m_hat / (v_hat.sqrt() + epsilon);
        let update = optimizer.learning_rate
            * match weight_decay_mode {
                AdamWeightDecayMode::Coupled => direction,
                AdamWeightDecayMode::Decoupled => {
                    direction + (optimizer.weight_decay * parameter_values[index])
                }
            };
        parameter_values[index] -= update;
        updates[index] = update;
    }
    updates
}

fn lars_updates(
    parameter_values: &mut [f32],
    gradient_values: &[f32],
    optimizer: &TrainingOptimizerConfig,
    momentum_buffer: &mut Option<Vec<f32>>,
) -> (Vec<f32>, f32) {
    let epsilon = optimizer.epsilon.unwrap_or(1e-8);
    let trust_coefficient = optimizer.trust_coefficient.unwrap_or(0.001);
    let effective_gradients = parameter_values
        .iter()
        .zip(gradient_values.iter())
        .map(|(parameter, gradient)| gradient + (optimizer.weight_decay * parameter))
        .collect::<Vec<_>>();
    let parameter_norm = norm_l2(parameter_values);
    let gradient_norm = norm_l2(effective_gradients.as_slice());
    let trust_ratio = if parameter_norm > 0.0 && gradient_norm > 0.0 {
        (trust_coefficient * parameter_norm) / (gradient_norm + epsilon)
    } else {
        1.0
    };

    let mut updates = vec![0.0; parameter_values.len()];
    for index in 0..parameter_values.len() {
        let scaled_gradient = trust_ratio * effective_gradients[index];
        let velocity = if let Some(buffer) = momentum_buffer.as_mut() {
            let momentum = optimizer.momentum.unwrap_or(0.0);
            buffer[index] = (momentum * buffer[index]) + scaled_gradient;
            buffer[index]
        } else {
            scaled_gradient
        };
        let update = optimizer.learning_rate * velocity;
        parameter_values[index] -= update;
        updates[index] = update;
    }
    (updates, trust_ratio)
}

fn lamb_updates(
    parameter_values: &mut [f32],
    gradient_values: &[f32],
    optimizer: &TrainingOptimizerConfig,
    first_moment: &mut [f32],
    second_moment: &mut [f32],
    step_number: u64,
) -> (Vec<f32>, f32) {
    let beta1 = optimizer.beta1.unwrap_or(0.9);
    let beta2 = optimizer.beta2.unwrap_or(0.999);
    let epsilon = optimizer.epsilon.unwrap_or(1e-6);
    let trust_coefficient = optimizer.trust_coefficient.unwrap_or(1.0);
    let bias_correction1 = 1.0 - beta1.powf(step_number as f32);
    let bias_correction2 = 1.0 - beta2.powf(step_number as f32);

    let mut directions = vec![0.0; parameter_values.len()];
    for index in 0..parameter_values.len() {
        let gradient = gradient_values[index];
        first_moment[index] = (beta1 * first_moment[index]) + ((1.0 - beta1) * gradient);
        second_moment[index] =
            (beta2 * second_moment[index]) + ((1.0 - beta2) * gradient * gradient);
        let m_hat = first_moment[index] / bias_correction1.max(f32::EPSILON);
        let v_hat = second_moment[index] / bias_correction2.max(f32::EPSILON);
        directions[index] =
            (m_hat / (v_hat.sqrt() + epsilon)) + (optimizer.weight_decay * parameter_values[index]);
    }

    let parameter_norm = norm_l2(parameter_values);
    let direction_norm = norm_l2(directions.as_slice());
    let trust_ratio = if parameter_norm > 0.0 && direction_norm > 0.0 {
        (trust_coefficient * parameter_norm) / (direction_norm + epsilon)
    } else {
        1.0
    };

    let mut updates = vec![0.0; parameter_values.len()];
    for index in 0..parameter_values.len() {
        let update = optimizer.learning_rate * trust_ratio * directions[index];
        parameter_values[index] -= update;
        updates[index] = update;
    }
    (updates, trust_ratio)
}

fn validate_state_len(
    optimizer: TrainingOptimizerKind,
    expected_len: usize,
    actual_len: usize,
) -> Result<(), TrainingOptimizerError> {
    if expected_len == actual_len {
        Ok(())
    } else {
        Err(TrainingOptimizerError::StateLengthMismatch {
            optimizer,
            expected_len,
            actual_len,
        })
    }
}

fn validate_adam_state_len(
    optimizer: TrainingOptimizerKind,
    expected_len: usize,
    first_moment: &[f32],
    second_moment: &[f32],
) -> Result<(), TrainingOptimizerError> {
    validate_state_len(optimizer, expected_len, first_moment.len())?;
    validate_state_len(optimizer, expected_len, second_moment.len())
}

fn norm_l2(values: &[f32]) -> f32 {
    values.iter().map(|value| value * value).sum::<f32>().sqrt()
}

fn run_optimizer_parity_supported_case(
    case_id: &str,
    capability_profile: &str,
    optimizer: TrainingOptimizerConfig,
    initial_parameters: Vec<f32>,
    gradients: Vec<f32>,
    step_number: u64,
    expected_parameters_after: Vec<f32>,
    expected_report: TrainingOptimizerStepReport,
    expected_state_after: OptimizerParityStateSnapshot,
) -> Result<OptimizerParityCaseResult, TrainingOptimizerError> {
    let mut parameters = initial_parameters.clone();
    let mut state = optimizer.initialize_state(parameters.len());
    let actual_report = optimizer.apply_step(
        parameters.as_mut_slice(),
        gradients.as_slice(),
        &mut state,
        step_number,
    )?;
    Ok(OptimizerParityCaseResult {
        case_id: String::from(case_id),
        oracle_family: String::from("pytorch_optim_db_seed"),
        optimizer: optimizer.kind,
        capability_profile: String::from(capability_profile),
        step_number,
        initial_parameters,
        gradients,
        expected_parameters_after: Some(expected_parameters_after),
        actual_parameters_after: Some(parameters),
        expected_report: Some(expected_report),
        actual_report: Some(actual_report),
        expected_state_after: Some(expected_state_after),
        actual_state_after: Some(snapshot_optimizer_state(&state)),
        expected_refusal: None,
        actual_refusal: None,
        status: OptimizerParityStatus::Supported,
    })
}

fn run_optimizer_parity_refusal_case(
    case_id: &str,
    capability_profile: &str,
    optimizer: TrainingOptimizerConfig,
    initial_parameters: Vec<f32>,
    gradients: Vec<f32>,
    mut state: TrainingOptimizerState,
    step_number: u64,
    expected_refusal: TrainingOptimizerError,
) -> OptimizerParityCaseResult {
    let mut parameters = initial_parameters.clone();
    let actual_refusal = optimizer
        .apply_step(
            parameters.as_mut_slice(),
            gradients.as_slice(),
            &mut state,
            step_number,
        )
        .err();
    let status = if actual_refusal.is_some() {
        OptimizerParityStatus::Refused
    } else {
        OptimizerParityStatus::Supported
    };
    OptimizerParityCaseResult {
        case_id: String::from(case_id),
        oracle_family: String::from("pytorch_optim_db_seed"),
        optimizer: optimizer.kind,
        capability_profile: String::from(capability_profile),
        step_number,
        initial_parameters,
        gradients,
        expected_parameters_after: None,
        actual_parameters_after: None,
        expected_report: None,
        actual_report: None,
        expected_state_after: None,
        actual_state_after: None,
        expected_refusal: Some(expected_refusal),
        actual_refusal,
        status,
    }
}

fn snapshot_optimizer_state(state: &TrainingOptimizerState) -> OptimizerParityStateSnapshot {
    match state {
        TrainingOptimizerState::Sgd { momentum_buffer } => OptimizerParityStateSnapshot {
            optimizer: TrainingOptimizerKind::Sgd,
            momentum_buffer: momentum_buffer.clone(),
            first_moment: None,
            second_moment: None,
        },
        TrainingOptimizerState::Adam {
            first_moment,
            second_moment,
        } => OptimizerParityStateSnapshot {
            optimizer: TrainingOptimizerKind::Adam,
            momentum_buffer: None,
            first_moment: Some(first_moment.clone()),
            second_moment: Some(second_moment.clone()),
        },
        TrainingOptimizerState::AdamW {
            first_moment,
            second_moment,
        } => OptimizerParityStateSnapshot {
            optimizer: TrainingOptimizerKind::AdamW,
            momentum_buffer: None,
            first_moment: Some(first_moment.clone()),
            second_moment: Some(second_moment.clone()),
        },
        TrainingOptimizerState::Lars { momentum_buffer } => OptimizerParityStateSnapshot {
            optimizer: TrainingOptimizerKind::Lars,
            momentum_buffer: momentum_buffer.clone(),
            first_moment: None,
            second_moment: None,
        },
        TrainingOptimizerState::Lamb {
            first_moment,
            second_moment,
        } => OptimizerParityStateSnapshot {
            optimizer: TrainingOptimizerKind::Lamb,
            momentum_buffer: None,
            first_moment: Some(first_moment.clone()),
            second_moment: Some(second_moment.clone()),
        },
    }
}

fn stable_optimizer_parity_matrix_digest(
    oracle_family_window: &str,
    cases: &[OptimizerParityCaseResult],
) -> String {
    let mut lines = vec![format!("oracle_family_window={oracle_family_window}")];
    for case in cases {
        lines.push(format!(
            "{}|{:?}|{}|{}|{:?}",
            case.case_id, case.optimizer, case.capability_profile, case.step_number, case.status
        ));
        lines.push(format!(
            "initial_parameters={}",
            format_float_slice(case.initial_parameters.as_slice())
        ));
        lines.push(format!(
            "gradients={}",
            format_float_slice(case.gradients.as_slice())
        ));
        if let Some(parameters) = &case.expected_parameters_after {
            lines.push(format!(
                "expected_parameters_after={}",
                format_float_slice(parameters.as_slice())
            ));
        }
        if let Some(parameters) = &case.actual_parameters_after {
            lines.push(format!(
                "actual_parameters_after={}",
                format_float_slice(parameters.as_slice())
            ));
        }
        if let Some(report) = &case.expected_report {
            push_report_lines("expected_report", report, &mut lines);
        }
        if let Some(report) = &case.actual_report {
            push_report_lines("actual_report", report, &mut lines);
        }
        if let Some(state) = &case.expected_state_after {
            push_state_lines("expected_state", state, &mut lines);
        }
        if let Some(state) = &case.actual_state_after {
            push_state_lines("actual_state", state, &mut lines);
        }
        if let Some(refusal) = &case.expected_refusal {
            lines.push(format!("expected_refusal={:?}", refusal));
        }
        if let Some(refusal) = &case.actual_refusal {
            lines.push(format!("actual_refusal={:?}", refusal));
        }
    }
    lines.sort();
    let mut hasher = sha2::Sha256::new();
    use sha2::Digest;
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

fn push_report_lines(prefix: &str, report: &TrainingOptimizerStepReport, lines: &mut Vec<String>) {
    lines.push(format!("{prefix}.optimizer={:?}", report.optimizer));
    lines.push(format!("{prefix}.step_number={}", report.step_number));
    lines.push(format!(
        "{prefix}.effective_learning_rate={}",
        format_float(report.effective_learning_rate)
    ));
    lines.push(format!(
        "{prefix}.effective_weight_decay={}",
        format_float(report.effective_weight_decay)
    ));
    lines.push(format!(
        "{prefix}.update_values={}",
        format_float_slice(report.update_values.as_slice())
    ));
    lines.push(format!(
        "{prefix}.update_norm_l2={}",
        format_float(report.update_norm_l2)
    ));
    lines.push(format!(
        "{prefix}.parameter_norm_l2_before={}",
        format_float(report.parameter_norm_l2_before)
    ));
    lines.push(format!(
        "{prefix}.parameter_norm_l2_after={}",
        format_float(report.parameter_norm_l2_after)
    ));
    if let Some(trust_ratio) = report.trust_ratio {
        lines.push(format!(
            "{prefix}.trust_ratio={}",
            format_float(trust_ratio)
        ));
    }
}

fn push_state_lines(prefix: &str, state: &OptimizerParityStateSnapshot, lines: &mut Vec<String>) {
    lines.push(format!("{prefix}.optimizer={:?}", state.optimizer));
    if let Some(buffer) = &state.momentum_buffer {
        lines.push(format!(
            "{prefix}.momentum_buffer={}",
            format_float_slice(buffer.as_slice())
        ));
    }
    if let Some(first_moment) = &state.first_moment {
        lines.push(format!(
            "{prefix}.first_moment={}",
            format_float_slice(first_moment.as_slice())
        ));
    }
    if let Some(second_moment) = &state.second_moment {
        lines.push(format!(
            "{prefix}.second_moment={}",
            format_float_slice(second_moment.as_slice())
        ));
    }
}

fn format_float_slice(values: &[f32]) -> String {
    values
        .iter()
        .map(|value| format_float(*value))
        .collect::<Vec<_>>()
        .join(",")
}

fn format_float(value: f32) -> String {
    format!("{value:.8}")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use serde_json::json;

    use crate::{
        TrainingOptimizerConfig, TrainingOptimizerKind, TrainingOptimizerState,
        TrainingSchedulerBinding, TrainingSchedulerConfig, TrainingSchedulerKind,
        apply_training_optimizer_step, builtin_optimizer_parity_matrix_report,
    };

    #[test]
    fn reusable_optimizer_surface_advances_small_model_with_sgd_and_adam()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut sgd_parameters = vec![1.0_f32, -1.0];
        let sgd_gradients = vec![0.25_f32, -0.25];
        let sgd_optimizer = TrainingOptimizerConfig::sgd(0.1).with_momentum(0.9);
        let mut sgd_state = sgd_optimizer.initialize_state(sgd_parameters.len());
        let sgd_step_one = apply_training_optimizer_step(
            sgd_parameters.as_mut_slice(),
            sgd_gradients.as_slice(),
            &sgd_optimizer,
            &mut sgd_state,
            1,
        )?;
        let sgd_step_two = apply_training_optimizer_step(
            sgd_parameters.as_mut_slice(),
            sgd_gradients.as_slice(),
            &sgd_optimizer,
            &mut sgd_state,
            2,
        )?;
        assert!(sgd_step_one.update_norm_l2 > 0.0);
        assert!(sgd_step_two.update_norm_l2 > sgd_step_one.update_norm_l2);
        assert!((sgd_step_one.effective_learning_rate - 0.1).abs() < 0.0001);
        assert!((sgd_step_one.effective_weight_decay - 0.0).abs() < 0.0001);
        assert!(sgd_parameters[0] < 1.0);

        let mut adam_parameters = vec![1.0_f32, -1.0];
        let adam_gradients = vec![0.25_f32, -0.25];
        let adam_optimizer = TrainingOptimizerConfig::adam(0.05, 0.9, 0.999, 1e-8);
        let mut adam_state = adam_optimizer.initialize_state(adam_parameters.len());
        let adam_step_one = apply_training_optimizer_step(
            adam_parameters.as_mut_slice(),
            adam_gradients.as_slice(),
            &adam_optimizer,
            &mut adam_state,
            1,
        )?;
        let adam_step_two = apply_training_optimizer_step(
            adam_parameters.as_mut_slice(),
            adam_gradients.as_slice(),
            &adam_optimizer,
            &mut adam_state,
            2,
        )?;
        assert!(adam_step_one.update_norm_l2 > 0.0);
        assert!(adam_step_two.update_norm_l2 > 0.0);
        assert!(adam_parameters[0] < 1.0);
        assert_eq!(adam_step_two.optimizer, TrainingOptimizerKind::Adam);
        assert!((adam_step_two.effective_learning_rate - 0.05).abs() < 0.0001);

        Ok(())
    }

    #[test]
    fn reusable_optimizer_surface_supports_all_declared_optimizer_families()
    -> Result<(), Box<dyn std::error::Error>> {
        let cases = vec![
            TrainingOptimizerConfig::sgd(0.1).with_momentum(0.9),
            TrainingOptimizerConfig::adam(0.05, 0.9, 0.999, 1e-8),
            TrainingOptimizerConfig::adamw(0.05, 0.9, 0.999, 1e-8).with_weight_decay(0.01),
            TrainingOptimizerConfig::lars(0.1, 0.9, 0.001, 1e-8).with_weight_decay(0.01),
            TrainingOptimizerConfig::lamb(0.05, 0.9, 0.999, 1e-6).with_weight_decay(0.01),
        ];

        for optimizer in cases {
            let mut parameters = vec![1.0_f32, -1.0, 0.5, -0.5];
            let gradients = vec![0.2_f32, -0.2, 0.1, -0.1];
            let mut state = optimizer.initialize_state(parameters.len());
            let report = optimizer.apply_step(
                parameters.as_mut_slice(),
                gradients.as_slice(),
                &mut state,
                1,
            )?;
            assert!(report.update_norm_l2 > 0.0);
            assert_eq!(state.kind(), optimizer.kind);
            let encoded = serde_json::to_value(&state)?;
            match optimizer.kind {
                TrainingOptimizerKind::Sgd | TrainingOptimizerKind::Lars => {
                    assert_eq!(encoded["kind"], json!(optimizer.kind));
                }
                TrainingOptimizerKind::Adam
                | TrainingOptimizerKind::AdamW
                | TrainingOptimizerKind::Lamb => {
                    assert!(encoded["first_moment"].is_array());
                    assert!(encoded["second_moment"].is_array());
                }
            }
        }

        Ok(())
    }

    #[test]
    fn reusable_optimizer_surface_refuses_state_kind_mismatch() {
        let optimizer = TrainingOptimizerConfig::adam(0.05, 0.9, 0.999, 1e-8);
        let mut parameters = vec![1.0_f32, -1.0];
        let gradients = vec![0.2_f32, -0.2];
        let mut state = TrainingOptimizerState::Sgd {
            momentum_buffer: None,
        };

        let error = optimizer
            .apply_step(
                parameters.as_mut_slice(),
                gradients.as_slice(),
                &mut state,
                1,
            )
            .expect_err("state mismatch should refuse");
        assert_eq!(
            error,
            super::TrainingOptimizerError::StateKindMismatch {
                optimizer: TrainingOptimizerKind::Adam,
                state_kind: TrainingOptimizerKind::Sgd,
            }
        );
    }

    #[test]
    fn reusable_optimizer_scheduler_surface_tracks_state_and_refuses_invalid_configs()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut step_lr = TrainingSchedulerBinding::new(TrainingSchedulerConfig::step_lr(2, 0.5));
        let step_one = super::scheduled_learning_rate(&mut step_lr, 0.1, 1)?;
        let step_three = super::scheduled_learning_rate(&mut step_lr, 0.1, 3)?;
        assert!((step_one - 0.1).abs() < 0.0001);
        assert!((step_three - 0.05).abs() < 0.0001);
        assert_eq!(step_lr.state.last_step, 3);
        assert_eq!(step_lr.state.last_learning_rate, Some(0.05));

        let mut invalid = TrainingSchedulerBinding::new(TrainingSchedulerConfig::step_lr(0, 0.5));
        let error = super::scheduled_learning_rate(&mut invalid, 0.1, 1)
            .expect_err("zero step size should refuse");
        assert_eq!(
            error,
            super::TrainingOptimizerError::InvalidSchedulerConfig {
                scheduler: TrainingSchedulerKind::StepLr,
                message: String::from("step_size must be greater than zero"),
            }
        );
        Ok(())
    }

    #[test]
    fn optimizer_parity_matrix_report_tracks_seeded_supported_and_refusal_cases()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = builtin_optimizer_parity_matrix_report()?;
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.oracle_family_window, "pytorch_optim_db_seed_v0");
        assert!(
            report
                .stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("matrix_digest="))
        );

        for case in report
            .cases
            .iter()
            .filter(|case| case.status == super::OptimizerParityStatus::Supported)
        {
            let expected_parameters = case
                .expected_parameters_after
                .as_ref()
                .expect("supported case should publish expected parameters");
            let actual_parameters = case
                .actual_parameters_after
                .as_ref()
                .expect("supported case should publish actual parameters");
            assert_float_slice_close(expected_parameters, actual_parameters);

            let expected_report = case
                .expected_report
                .as_ref()
                .expect("supported case should publish expected report");
            let actual_report = case
                .actual_report
                .as_ref()
                .expect("supported case should publish actual report");
            assert_step_report_close(expected_report, actual_report);

            let expected_state = case
                .expected_state_after
                .as_ref()
                .expect("supported case should publish expected state");
            let actual_state = case
                .actual_state_after
                .as_ref()
                .expect("supported case should publish actual state");
            assert_state_snapshot_close(expected_state, actual_state);
        }

        let refusal_case = report
            .cases
            .iter()
            .find(|case| case.case_id == "pytorch.adamw.state_kind_mismatch")
            .expect("missing optimizer refusal case");
        assert_eq!(refusal_case.status, super::OptimizerParityStatus::Refused);
        assert_eq!(refusal_case.expected_refusal, refusal_case.actual_refusal);

        Ok(())
    }

    fn assert_step_report_close(
        expected: &super::TrainingOptimizerStepReport,
        actual: &super::TrainingOptimizerStepReport,
    ) {
        assert_eq!(expected.optimizer, actual.optimizer);
        assert_eq!(expected.step_number, actual.step_number);
        assert_float_close(
            expected.effective_learning_rate,
            actual.effective_learning_rate,
        );
        assert_float_close(
            expected.effective_weight_decay,
            actual.effective_weight_decay,
        );
        assert_float_slice_close(
            expected.update_values.as_slice(),
            actual.update_values.as_slice(),
        );
        assert_float_close(expected.update_norm_l2, actual.update_norm_l2);
        assert_float_close(
            expected.parameter_norm_l2_before,
            actual.parameter_norm_l2_before,
        );
        assert_float_close(
            expected.parameter_norm_l2_after,
            actual.parameter_norm_l2_after,
        );
        match (expected.trust_ratio, actual.trust_ratio) {
            (Some(expected), Some(actual)) => assert_float_close(expected, actual),
            (None, None) => {}
            mismatch => panic!("trust-ratio mismatch: {mismatch:?}"),
        }
    }

    fn assert_state_snapshot_close(
        expected: &super::OptimizerParityStateSnapshot,
        actual: &super::OptimizerParityStateSnapshot,
    ) {
        assert_eq!(expected.optimizer, actual.optimizer);
        match (
            expected.momentum_buffer.as_ref(),
            actual.momentum_buffer.as_ref(),
        ) {
            (Some(expected), Some(actual)) => assert_float_slice_close(expected, actual),
            (None, None) => {}
            mismatch => panic!("momentum-buffer mismatch: {mismatch:?}"),
        }
        match (expected.first_moment.as_ref(), actual.first_moment.as_ref()) {
            (Some(expected), Some(actual)) => assert_float_slice_close(expected, actual),
            (None, None) => {}
            mismatch => panic!("first-moment mismatch: {mismatch:?}"),
        }
        match (
            expected.second_moment.as_ref(),
            actual.second_moment.as_ref(),
        ) {
            (Some(expected), Some(actual)) => assert_float_slice_close(expected, actual),
            (None, None) => {}
            mismatch => panic!("second-moment mismatch: {mismatch:?}"),
        }
    }

    fn assert_float_slice_close(expected: &[f32], actual: &[f32]) {
        assert_eq!(expected.len(), actual.len());
        for (expected, actual) in expected.iter().zip(actual.iter()) {
            assert_float_close(*expected, *actual);
        }
    }

    fn assert_float_close(expected: f32, actual: f32) {
        assert!(
            (expected - actual).abs() <= 1e-5,
            "expected {expected}, found {actual}"
        );
    }
}
