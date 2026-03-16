use crate::{LayerError, ModuleParameter, ModuleStateError, NnTensor};
use psionic_core::{DType, Device, Shape, TensorData, TensorSpec};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const EPSILON: f32 = 1e-6;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum NnTrainingError {
    #[error(transparent)]
    Layer(#[from] LayerError),
    #[error(transparent)]
    ModuleState(#[from] ModuleStateError),
    #[error("training helper `{operation}` expected matching shapes, found {left:?} and {right:?}")]
    ShapePairMismatch {
        operation: &'static str,
        left: Vec<usize>,
        right: Vec<usize>,
    },
    #[error("training helper `{operation}` invalid configuration: {message}")]
    InvalidConfiguration {
        operation: &'static str,
        message: String,
    },
    #[error("training helper `{operation}` expected {expected} targets but found {actual}")]
    TargetCountMismatch {
        operation: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error(
        "training helper `{operation}` class index {index} is out of range for {class_count} classes"
    )]
    ClassIndexOutOfRange {
        operation: &'static str,
        index: usize,
        class_count: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LossReduction {
    None,
    Mean,
    Sum,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InitKind {
    Zeros,
    Ones,
    Constant { value: f32 },
    Uniform { low: f32, high: f32, seed: u64 },
    Normal { mean: f32, std: f32, seed: u64 },
    XavierUniform { gain: f32, seed: u64 },
    XavierNormal { gain: f32, seed: u64 },
    KaimingUniform { negative_slope: f32, seed: u64 },
    KaimingNormal { negative_slope: f32, seed: u64 },
}

pub fn mse_loss(
    input: &NnTensor,
    target: &NnTensor,
    reduction: LossReduction,
) -> Result<NnTensor, NnTrainingError> {
    ensure_same_shape("mse_loss", input, target)?;
    let losses = input
        .as_f32_slice()?
        .iter()
        .zip(target.as_f32_slice()?.iter())
        .map(|(input, target)| {
            let delta = input - target;
            delta * delta
        })
        .collect::<Vec<_>>();
    reduce_values(&input.spec.shape().clone(), losses, reduction)
}

pub fn l1_loss(
    input: &NnTensor,
    target: &NnTensor,
    reduction: LossReduction,
) -> Result<NnTensor, NnTrainingError> {
    ensure_same_shape("l1_loss", input, target)?;
    let losses = input
        .as_f32_slice()?
        .iter()
        .zip(target.as_f32_slice()?.iter())
        .map(|(input, target)| (input - target).abs())
        .collect::<Vec<_>>();
    reduce_values(&input.spec.shape().clone(), losses, reduction)
}

pub fn binary_cross_entropy_loss(
    prediction: &NnTensor,
    target: &NnTensor,
    reduction: LossReduction,
) -> Result<NnTensor, NnTrainingError> {
    ensure_same_shape("binary_cross_entropy_loss", prediction, target)?;
    let losses = prediction
        .as_f32_slice()?
        .iter()
        .zip(target.as_f32_slice()?.iter())
        .map(|(prediction, target)| {
            let clipped = prediction.clamp(EPSILON, 1.0 - EPSILON);
            -(target * clipped.ln() + (1.0 - target) * (1.0 - clipped).ln())
        })
        .collect::<Vec<_>>();
    reduce_values(&prediction.spec.shape().clone(), losses, reduction)
}

pub fn cross_entropy_loss(
    logits: &NnTensor,
    target_classes: &[usize],
    reduction: LossReduction,
) -> Result<NnTensor, NnTrainingError> {
    ensure_rank_at_least_one("cross_entropy_loss", logits.spec.shape())?;
    let class_count =
        *logits
            .dims()
            .last()
            .ok_or_else(|| NnTrainingError::InvalidConfiguration {
                operation: "cross_entropy_loss",
                message: String::from("logits must have a class dimension"),
            })?;
    if class_count == 0 {
        return Err(NnTrainingError::InvalidConfiguration {
            operation: "cross_entropy_loss",
            message: String::from("logits class dimension must be greater than zero"),
        });
    }

    let rows = logits.spec.shape().element_count() / class_count;
    if target_classes.len() != rows {
        return Err(NnTrainingError::TargetCountMismatch {
            operation: "cross_entropy_loss",
            expected: rows,
            actual: target_classes.len(),
        });
    }

    let log_probs = log_softmax_last_dim(logits)?;
    let values = log_probs.as_f32_slice()?;
    let mut losses = Vec::with_capacity(rows);
    for (row, class_index) in target_classes.iter().enumerate() {
        if *class_index >= class_count {
            return Err(NnTrainingError::ClassIndexOutOfRange {
                operation: "cross_entropy_loss",
                index: *class_index,
                class_count,
            });
        }
        losses.push(-values[row * class_count + class_index]);
    }

    let output_shape = Shape::new(logits.dims()[..logits.dims().len().saturating_sub(1)].to_vec());
    reduce_values(&output_shape, losses, reduction)
}

pub fn sigmoid(input: &NnTensor) -> Result<NnTensor, NnTrainingError> {
    let output = input
        .as_f32_slice()?
        .iter()
        .map(|value| 1.0 / (1.0 + (-value).exp()))
        .collect::<Vec<_>>();
    NnTensor::f32(input.spec.shape().clone(), output).map_err(Into::into)
}

pub fn softmax_last_dim(input: &NnTensor) -> Result<NnTensor, NnTrainingError> {
    let output = softmax_like("softmax_last_dim", input, false)?;
    NnTensor::f32(input.spec.shape().clone(), output).map_err(Into::into)
}

pub fn log_softmax_last_dim(input: &NnTensor) -> Result<NnTensor, NnTrainingError> {
    let output = softmax_like("log_softmax_last_dim", input, true)?;
    NnTensor::f32(input.spec.shape().clone(), output).map_err(Into::into)
}

pub fn one_hot(indices: &[usize], class_count: usize) -> Result<NnTensor, NnTrainingError> {
    if class_count == 0 {
        return Err(NnTrainingError::InvalidConfiguration {
            operation: "one_hot",
            message: String::from("class_count must be greater than zero"),
        });
    }
    let mut values = vec![0.0; indices.len() * class_count];
    for (row, index) in indices.iter().enumerate() {
        if *index >= class_count {
            return Err(NnTrainingError::ClassIndexOutOfRange {
                operation: "one_hot",
                index: *index,
                class_count,
            });
        }
        values[row * class_count + index] = 1.0;
    }
    NnTensor::f32(Shape::new(vec![indices.len(), class_count]), values).map_err(Into::into)
}

pub fn init_tensor(shape: Shape, kind: InitKind) -> Result<NnTensor, NnTrainingError> {
    let values = init_values(&shape, kind)?;
    NnTensor::f32(shape, values).map_err(Into::into)
}

pub fn init_parameter(
    shape: Shape,
    kind: InitKind,
    requires_grad: bool,
) -> Result<ModuleParameter, NnTrainingError> {
    let values = init_values(&shape, kind)?;
    ModuleParameter::new(
        TensorSpec::new(shape, DType::F32, Device::cpu()),
        TensorData::F32(values),
        requires_grad,
    )
    .map_err(Into::into)
}

fn init_values(shape: &Shape, kind: InitKind) -> Result<Vec<f32>, NnTrainingError> {
    let len = shape.element_count();
    match kind {
        InitKind::Zeros => Ok(vec![0.0; len]),
        InitKind::Ones => Ok(vec![1.0; len]),
        InitKind::Constant { value } => Ok(vec![value; len]),
        InitKind::Uniform { low, high, seed } => sample_uniform(len, low, high, seed),
        InitKind::Normal { mean, std, seed } => sample_normal(len, mean, std, seed),
        InitKind::XavierUniform { gain, seed } => {
            let (fan_in, fan_out) = fan_in_and_fan_out(shape)?;
            let bound = gain * (6.0 / (fan_in + fan_out) as f32).sqrt();
            sample_uniform(len, -bound, bound, seed)
        }
        InitKind::XavierNormal { gain, seed } => {
            let (fan_in, fan_out) = fan_in_and_fan_out(shape)?;
            let std = gain * (2.0 / (fan_in + fan_out) as f32).sqrt();
            sample_normal(len, 0.0, std, seed)
        }
        InitKind::KaimingUniform {
            negative_slope,
            seed,
        } => {
            let (fan_in, _) = fan_in_and_fan_out(shape)?;
            let gain = (2.0 / (1.0 + (negative_slope * negative_slope))).sqrt();
            let std = gain / (fan_in as f32).sqrt();
            sample_uniform(len, -(3.0_f32).sqrt() * std, (3.0_f32).sqrt() * std, seed)
        }
        InitKind::KaimingNormal {
            negative_slope,
            seed,
        } => {
            let (fan_in, _) = fan_in_and_fan_out(shape)?;
            let gain = (2.0 / (1.0 + (negative_slope * negative_slope))).sqrt();
            let std = gain / (fan_in as f32).sqrt();
            sample_normal(len, 0.0, std, seed)
        }
    }
}

fn softmax_like(
    operation: &'static str,
    input: &NnTensor,
    return_log: bool,
) -> Result<Vec<f32>, NnTrainingError> {
    ensure_rank_at_least_one(operation, input.spec.shape())?;
    let class_count =
        *input
            .dims()
            .last()
            .ok_or_else(|| NnTrainingError::InvalidConfiguration {
                operation,
                message: String::from("input must have a last dimension"),
            })?;
    if class_count == 0 {
        return Err(NnTrainingError::InvalidConfiguration {
            operation,
            message: String::from("last dimension must be greater than zero"),
        });
    }
    let values = input.as_f32_slice()?;
    let rows = input.spec.shape().element_count() / class_count;
    let mut output = vec![0.0; values.len()];
    for row in 0..rows {
        let offset = row * class_count;
        let slice = &values[offset..offset + class_count];
        let max_value = slice.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let sum_exp = slice
            .iter()
            .map(|value| (value - max_value).exp())
            .sum::<f32>();
        let log_sum_exp = max_value + sum_exp.ln();
        for (class_index, value) in slice.iter().enumerate() {
            output[offset + class_index] = if return_log {
                value - log_sum_exp
            } else {
                (*value - log_sum_exp).exp()
            };
        }
    }
    Ok(output)
}

fn ensure_same_shape(
    operation: &'static str,
    left: &NnTensor,
    right: &NnTensor,
) -> Result<(), NnTrainingError> {
    if left.dims() != right.dims() {
        return Err(NnTrainingError::ShapePairMismatch {
            operation,
            left: left.dims().to_vec(),
            right: right.dims().to_vec(),
        });
    }
    Ok(())
}

fn reduce_values(
    original_shape: &Shape,
    values: Vec<f32>,
    reduction: LossReduction,
) -> Result<NnTensor, NnTrainingError> {
    match reduction {
        LossReduction::None => NnTensor::f32(original_shape.clone(), values).map_err(Into::into),
        LossReduction::Mean => {
            if values.is_empty() {
                return Err(NnTrainingError::InvalidConfiguration {
                    operation: "loss_reduction",
                    message: String::from("cannot compute mean over zero elements"),
                });
            }
            let mean = values.iter().sum::<f32>() / values.len() as f32;
            NnTensor::f32(Shape::scalar(), vec![mean]).map_err(Into::into)
        }
        LossReduction::Sum => {
            let sum = values.iter().sum::<f32>();
            NnTensor::f32(Shape::scalar(), vec![sum]).map_err(Into::into)
        }
    }
}

fn ensure_rank_at_least_one(operation: &'static str, shape: &Shape) -> Result<(), NnTrainingError> {
    if shape.rank() == 0 {
        return Err(NnTrainingError::InvalidConfiguration {
            operation,
            message: String::from("input must have rank at least one"),
        });
    }
    Ok(())
}

fn fan_in_and_fan_out(shape: &Shape) -> Result<(usize, usize), NnTrainingError> {
    if shape.rank() < 2 {
        return Err(NnTrainingError::InvalidConfiguration {
            operation: "init",
            message: format!(
                "fan_in/fan_out initializers require rank >= 2, found {:?}",
                shape.dims()
            ),
        });
    }
    let receptive_field = shape.dims()[2..].iter().product::<usize>().max(1);
    Ok((
        shape.dims()[1] * receptive_field,
        shape.dims()[0] * receptive_field,
    ))
}

fn sample_uniform(len: usize, low: f32, high: f32, seed: u64) -> Result<Vec<f32>, NnTrainingError> {
    if !matches!(low.partial_cmp(&high), Some(std::cmp::Ordering::Less)) {
        return Err(NnTrainingError::InvalidConfiguration {
            operation: "init",
            message: format!("uniform bounds must satisfy low < high, found {low} >= {high}"),
        });
    }
    let span = high - low;
    let mut state = seed.max(1);
    Ok((0..len)
        .map(|_| low + (next_uniform(&mut state) * span))
        .collect())
}

fn sample_normal(len: usize, mean: f32, std: f32, seed: u64) -> Result<Vec<f32>, NnTrainingError> {
    if std <= 0.0 {
        return Err(NnTrainingError::InvalidConfiguration {
            operation: "init",
            message: format!("normal std must be greater than zero, found {std}"),
        });
    }
    let mut state = seed.max(1);
    let mut values = Vec::with_capacity(len);
    while values.len() < len {
        let u1 = next_uniform(&mut state).max(EPSILON);
        let u2 = next_uniform(&mut state);
        let radius = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * std::f32::consts::PI * u2;
        values.push(mean + std * radius * theta.cos());
        if values.len() < len {
            values.push(mean + std * radius * theta.sin());
        }
    }
    Ok(values)
}

fn next_uniform(state: &mut u64) -> f32 {
    *state ^= *state >> 12;
    *state ^= *state << 25;
    *state ^= *state >> 27;
    let random = state.wrapping_mul(2_685_821_657_736_338_717);
    ((random >> 40) as f32) / ((1_u32 << 24) as f32)
}

#[cfg(test)]
mod tests {
    use super::{
        InitKind, LossReduction, binary_cross_entropy_loss, cross_entropy_loss, init_parameter,
        init_tensor, l1_loss, log_softmax_last_dim, mse_loss, one_hot, sigmoid, softmax_last_dim,
    };
    use crate::NnTensor;
    use psionic_core::{Shape, TensorData};

    fn approx_eq(left: &[f32], right: &[f32]) {
        assert_eq!(left.len(), right.len());
        for (left, right) in left.iter().zip(right.iter()) {
            assert!((left - right).abs() < 1e-4, "{left} != {right}");
        }
    }

    #[test]
    fn mse_and_l1_losses_match_reference_reductions() -> Result<(), Box<dyn std::error::Error>> {
        let input = NnTensor::f32(Shape::new(vec![2, 2]), vec![1.0, 3.0, -2.0, 4.0])?;
        let target = NnTensor::f32(Shape::new(vec![2, 2]), vec![0.0, 1.0, -1.0, 2.0])?;

        let mse_none = mse_loss(&input, &target, LossReduction::None)?;
        assert_eq!(mse_none.dims(), &[2, 2]);
        approx_eq(mse_none.as_f32_slice()?, &[1.0, 4.0, 1.0, 4.0]);
        approx_eq(
            mse_loss(&input, &target, LossReduction::Mean)?.as_f32_slice()?,
            &[2.5],
        );
        approx_eq(
            mse_loss(&input, &target, LossReduction::Sum)?.as_f32_slice()?,
            &[10.0],
        );

        let l1_none = l1_loss(&input, &target, LossReduction::None)?;
        assert_eq!(l1_none.dims(), &[2, 2]);
        approx_eq(l1_none.as_f32_slice()?, &[1.0, 2.0, 1.0, 2.0]);
        approx_eq(
            l1_loss(&input, &target, LossReduction::Mean)?.as_f32_slice()?,
            &[1.5],
        );
        approx_eq(
            l1_loss(&input, &target, LossReduction::Sum)?.as_f32_slice()?,
            &[6.0],
        );
        Ok(())
    }

    #[test]
    fn classification_losses_and_helpers_match_reference() -> Result<(), Box<dyn std::error::Error>>
    {
        let logits = NnTensor::f32(Shape::new(vec![2, 3]), vec![2.0, 1.0, 0.0, 0.5, 1.5, -1.0])?;
        let softmax = softmax_last_dim(&logits)?;
        approx_eq(
            softmax.as_f32_slice()?,
            &[
                0.66524094, 0.24472848, 0.09003057, 0.25371617, 0.6896721, 0.05661174,
            ],
        );

        let log_softmax = log_softmax_last_dim(&logits)?;
        approx_eq(
            log_softmax.as_f32_slice()?,
            &[
                -0.40760595,
                -1.4076059,
                -2.407606,
                -1.371539,
                -0.371539,
                -2.871539,
            ],
        );

        let ce = cross_entropy_loss(&logits, &[0, 1], LossReduction::None)?;
        assert_eq!(ce.dims(), &[2]);
        approx_eq(ce.as_f32_slice()?, &[0.40760595, 0.371539]);

        let one_hot = one_hot(&[2, 0, 1], 4)?;
        assert_eq!(one_hot.dims(), &[3, 4]);
        approx_eq(
            one_hot.as_f32_slice()?,
            &[0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        );
        Ok(())
    }

    #[test]
    fn binary_cross_entropy_and_sigmoid_match_reference() -> Result<(), Box<dyn std::error::Error>>
    {
        let logits = NnTensor::f32(Shape::new(vec![3]), vec![-2.0, 0.0, 2.0])?;
        let targets = NnTensor::f32(Shape::new(vec![3]), vec![0.0, 1.0, 1.0])?;
        let probabilities = sigmoid(&logits)?;
        approx_eq(probabilities.as_f32_slice()?, &[0.11920292, 0.5, 0.880797]);

        let bce = binary_cross_entropy_loss(&probabilities, &targets, LossReduction::None)?;
        approx_eq(bce.as_f32_slice()?, &[0.126928, 0.6931472, 0.12692806]);
        approx_eq(
            binary_cross_entropy_loss(&probabilities, &targets, LossReduction::Mean)?
                .as_f32_slice()?,
            &[0.31566775],
        );
        Ok(())
    }

    #[test]
    fn init_families_are_deterministic_and_parameter_ready()
    -> Result<(), Box<dyn std::error::Error>> {
        let shape = Shape::new(vec![2, 3]);
        let xavier_one = init_tensor(
            shape.clone(),
            InitKind::XavierUniform { gain: 1.0, seed: 7 },
        )?;
        let xavier_two = init_tensor(
            shape.clone(),
            InitKind::XavierUniform { gain: 1.0, seed: 7 },
        )?;
        assert_eq!(xavier_one, xavier_two);

        let kaiming_one = init_tensor(
            Shape::new(vec![4, 3, 2, 2]),
            InitKind::KaimingNormal {
                negative_slope: 0.1,
                seed: 9,
            },
        )?;
        let kaiming_two = init_tensor(
            Shape::new(vec![4, 3, 2, 2]),
            InitKind::KaimingNormal {
                negative_slope: 0.1,
                seed: 9,
            },
        )?;
        assert_eq!(kaiming_one, kaiming_two);

        let parameter = init_parameter(
            Shape::new(vec![2, 2]),
            InitKind::Constant { value: 0.25 },
            true,
        )?;
        assert!(parameter.requires_grad);
        assert_eq!(
            parameter.data,
            TensorData::F32(vec![0.25, 0.25, 0.25, 0.25])
        );
        Ok(())
    }
}
