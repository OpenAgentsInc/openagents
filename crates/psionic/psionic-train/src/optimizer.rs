use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::core_loop::{TrainingOptimizerConfig, TrainingOptimizerKind, TrainingOptimizerState};

/// Error returned by the reusable optimizer surface.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
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
}

/// Inspectable result of one reusable optimizer step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingOptimizerStepReport {
    /// Optimizer family that applied the update.
    pub optimizer: TrainingOptimizerKind,
    /// One-based step count used for the update.
    pub step_number: u64,
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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use serde_json::json;

    use crate::{
        TrainingOptimizerConfig, TrainingOptimizerKind, TrainingOptimizerState,
        apply_training_optimizer_step,
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
}
