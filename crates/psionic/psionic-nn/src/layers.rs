use crate::{Module, ModuleParameter, ModuleStateError, QuantizationError};
use psionic_core::{DType, Device, DeviceKind, Shape, TensorData, TensorSpec};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum LayerError {
    #[error(transparent)]
    ModuleState(#[from] ModuleStateError),
    #[error(transparent)]
    Quantization(#[from] QuantizationError),
    #[error("layer `{layer}` invalid configuration: {message}")]
    InvalidConfiguration {
        layer: &'static str,
        message: String,
    },
    #[error(
        "layer `{layer}` requires dense cpu f32 tensor `{tensor}`, found dtype {dtype:?} on device {device:?}"
    )]
    UnsupportedTensor {
        layer: &'static str,
        tensor: &'static str,
        dtype: DType,
        device: Device,
    },
    #[error("layer `{layer}` shape mismatch for `{tensor}`: expected {expected}, found {actual:?}")]
    ShapeMismatch {
        layer: &'static str,
        tensor: &'static str,
        expected: String,
        actual: Vec<usize>,
    },
    #[error("layer `{layer}` index {index} is out of range for embedding table size {upper_bound}")]
    IndexOutOfRange {
        layer: &'static str,
        index: usize,
        upper_bound: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NnTensor {
    pub spec: TensorSpec,
    pub data: TensorData,
}

impl NnTensor {
    pub fn f32(shape: Shape, values: Vec<f32>) -> Result<Self, LayerError> {
        let spec = TensorSpec::new(shape.clone(), DType::F32, Device::cpu());
        validate_dense_cpu_f32(
            "nn_tensor",
            "tensor",
            &spec,
            &TensorData::F32(values.clone()),
        )?;
        Ok(Self {
            spec,
            data: TensorData::F32(values),
        })
    }

    pub fn dims(&self) -> &[usize] {
        self.spec.shape().dims()
    }

    pub fn as_f32_slice(&self) -> Result<&[f32], LayerError> {
        validate_dense_cpu_f32("nn_tensor", "tensor", &self.spec, &self.data)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationKind {
    Relu,
    Gelu,
    Silu,
    Tanh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoolMode {
    Max,
    Average,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Linear {
    module: Module,
    in_features: usize,
    out_features: usize,
    use_bias: bool,
}

impl Linear {
    pub fn new(
        module_id: impl Into<String>,
        in_features: usize,
        out_features: usize,
        use_bias: bool,
    ) -> Result<Self, LayerError> {
        let weight = vec![0.0; in_features * out_features];
        let bias = use_bias.then(|| vec![0.0; out_features]);
        Self::from_f32_parts(module_id, in_features, out_features, weight, bias)
    }

    pub fn from_f32_parts(
        module_id: impl Into<String>,
        in_features: usize,
        out_features: usize,
        weight: Vec<f32>,
        bias: Option<Vec<f32>>,
    ) -> Result<Self, LayerError> {
        ensure_positive("linear", "in_features", in_features)?;
        ensure_positive("linear", "out_features", out_features)?;
        let mut module = Module::new(module_id, "linear")?;
        module.insert_parameter(
            "weight",
            dense_parameter(&[out_features, in_features], weight, true)?,
        )?;
        if let Some(bias) = bias {
            module.insert_parameter("bias", dense_parameter(&[out_features], bias, true)?)?;
        }
        let use_bias = module.parameters.contains_key("bias");
        Ok(Self {
            module,
            in_features,
            out_features,
            use_bias,
        })
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    #[must_use]
    pub const fn in_features(&self) -> usize {
        self.in_features
    }

    #[must_use]
    pub const fn out_features(&self) -> usize {
        self.out_features
    }

    #[must_use]
    pub const fn uses_bias(&self) -> bool {
        self.use_bias
    }

    pub fn weight_f32(&self) -> Result<&[f32], LayerError> {
        parameter_f32(
            &self.module,
            "linear",
            "weight",
            &[self.out_features, self.in_features],
        )
    }

    pub fn bias_f32(&self) -> Result<Option<&[f32]>, LayerError> {
        if self.use_bias {
            Ok(Some(parameter_f32(
                &self.module,
                "linear",
                "bias",
                &[self.out_features],
            )?))
        } else {
            Ok(None)
        }
    }

    pub fn forward(&self, input: &NnTensor) -> Result<NnTensor, LayerError> {
        let values = input.as_f32_slice()?;
        let dims = input.dims();
        if dims.is_empty() {
            return Err(shape_mismatch(
                "linear",
                "input",
                format!("rank >= 1 with trailing dimension {}", self.in_features),
                dims,
            ));
        }
        if dims[dims.len() - 1] != self.in_features {
            return Err(shape_mismatch(
                "linear",
                "input",
                format!("last dimension {}", self.in_features),
                dims,
            ));
        }

        let rows = dims[..dims.len() - 1].iter().product::<usize>().max(1);
        let weight = parameter_f32(
            &self.module,
            "linear",
            "weight",
            &[self.out_features, self.in_features],
        )?;
        let bias = if self.use_bias {
            Some(parameter_f32(
                &self.module,
                "linear",
                "bias",
                &[self.out_features],
            )?)
        } else {
            None
        };
        let mut output = vec![0.0; rows * self.out_features];
        for row in 0..rows {
            let input_offset = row * self.in_features;
            let output_offset = row * self.out_features;
            for out_index in 0..self.out_features {
                let mut sum = bias.map_or(0.0, |bias| bias[out_index]);
                let weight_offset = out_index * self.in_features;
                for in_index in 0..self.in_features {
                    sum += values[input_offset + in_index] * weight[weight_offset + in_index];
                }
                output[output_offset + out_index] = sum;
            }
        }

        let mut output_dims = dims.to_vec();
        *output_dims.last_mut().expect("validated non-empty dims") = self.out_features;
        NnTensor::f32(Shape::new(output_dims), output)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Embedding {
    module: Module,
    vocab_size: usize,
    embedding_dim: usize,
}

impl Embedding {
    pub fn new(
        module_id: impl Into<String>,
        vocab_size: usize,
        embedding_dim: usize,
    ) -> Result<Self, LayerError> {
        let weight = vec![0.0; vocab_size * embedding_dim];
        Self::from_f32_table(module_id, vocab_size, embedding_dim, weight)
    }

    pub fn from_f32_table(
        module_id: impl Into<String>,
        vocab_size: usize,
        embedding_dim: usize,
        weight: Vec<f32>,
    ) -> Result<Self, LayerError> {
        ensure_positive("embedding", "vocab_size", vocab_size)?;
        ensure_positive("embedding", "embedding_dim", embedding_dim)?;
        let mut module = Module::new(module_id, "embedding")?;
        module.insert_parameter(
            "weight",
            dense_parameter(&[vocab_size, embedding_dim], weight, true)?,
        )?;
        Ok(Self {
            module,
            vocab_size,
            embedding_dim,
        })
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn forward(&self, indices: &[usize]) -> Result<NnTensor, LayerError> {
        self.forward_with_shape(Shape::new(vec![indices.len()]), indices)
    }

    pub fn forward_with_shape(
        &self,
        index_shape: Shape,
        indices: &[usize],
    ) -> Result<NnTensor, LayerError> {
        if index_shape.element_count() != indices.len() {
            return Err(LayerError::InvalidConfiguration {
                layer: "embedding",
                message: format!(
                    "index shape {:?} expects {} elements but received {} indices",
                    index_shape.dims(),
                    index_shape.element_count(),
                    indices.len()
                ),
            });
        }
        let table = parameter_f32(
            &self.module,
            "embedding",
            "weight",
            &[self.vocab_size, self.embedding_dim],
        )?;
        let mut output = vec![0.0; indices.len() * self.embedding_dim];
        for (slot, &index) in indices.iter().enumerate() {
            if index >= self.vocab_size {
                return Err(LayerError::IndexOutOfRange {
                    layer: "embedding",
                    index,
                    upper_bound: self.vocab_size,
                });
            }
            let src_offset = index * self.embedding_dim;
            let dst_offset = slot * self.embedding_dim;
            output[dst_offset..dst_offset + self.embedding_dim]
                .copy_from_slice(&table[src_offset..src_offset + self.embedding_dim]);
        }
        let mut dims = index_shape.dims().to_vec();
        dims.push(self.embedding_dim);
        NnTensor::f32(Shape::new(dims), output)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerNorm {
    module: Module,
    feature_size: usize,
    epsilon: f32,
}

impl LayerNorm {
    pub fn new(
        module_id: impl Into<String>,
        feature_size: usize,
        epsilon: f32,
    ) -> Result<Self, LayerError> {
        ensure_positive("layer_norm", "feature_size", feature_size)?;
        ensure_positive_f32("layer_norm", "epsilon", epsilon)?;
        let mut module = Module::new(module_id, "layer_norm")?;
        module.insert_parameter(
            "weight",
            dense_parameter(&[feature_size], vec![1.0; feature_size], true)?,
        )?;
        module.insert_parameter(
            "bias",
            dense_parameter(&[feature_size], vec![0.0; feature_size], true)?,
        )?;
        Ok(Self {
            module,
            feature_size,
            epsilon,
        })
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn forward(&self, input: &NnTensor) -> Result<NnTensor, LayerError> {
        let dims = input.dims();
        if dims.is_empty() || dims[dims.len() - 1] != self.feature_size {
            return Err(shape_mismatch(
                "layer_norm",
                "input",
                format!("last dimension {}", self.feature_size),
                dims,
            ));
        }
        let values = input.as_f32_slice()?;
        let weight = parameter_f32(&self.module, "layer_norm", "weight", &[self.feature_size])?;
        let bias = parameter_f32(&self.module, "layer_norm", "bias", &[self.feature_size])?;
        let rows = dims[..dims.len() - 1].iter().product::<usize>().max(1);
        let mut output = vec![0.0; values.len()];
        for row in 0..rows {
            let offset = row * self.feature_size;
            let chunk = &values[offset..offset + self.feature_size];
            let mean = chunk.iter().sum::<f32>() / self.feature_size as f32;
            let variance = chunk
                .iter()
                .map(|value| {
                    let centered = value - mean;
                    centered * centered
                })
                .sum::<f32>()
                / self.feature_size as f32;
            let scale = 1.0 / (variance + self.epsilon).sqrt();
            for index in 0..self.feature_size {
                let centered = chunk[index] - mean;
                output[offset + index] = centered * scale * weight[index] + bias[index];
            }
        }
        NnTensor::f32(Shape::new(dims.to_vec()), output)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RmsNorm {
    module: Module,
    feature_size: usize,
    epsilon: f32,
}

impl RmsNorm {
    pub fn new(
        module_id: impl Into<String>,
        feature_size: usize,
        epsilon: f32,
    ) -> Result<Self, LayerError> {
        ensure_positive("rms_norm", "feature_size", feature_size)?;
        ensure_positive_f32("rms_norm", "epsilon", epsilon)?;
        let mut module = Module::new(module_id, "rms_norm")?;
        module.insert_parameter(
            "weight",
            dense_parameter(&[feature_size], vec![1.0; feature_size], true)?,
        )?;
        Ok(Self {
            module,
            feature_size,
            epsilon,
        })
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn forward(&self, input: &NnTensor) -> Result<NnTensor, LayerError> {
        let dims = input.dims();
        if dims.is_empty() || dims[dims.len() - 1] != self.feature_size {
            return Err(shape_mismatch(
                "rms_norm",
                "input",
                format!("last dimension {}", self.feature_size),
                dims,
            ));
        }
        let values = input.as_f32_slice()?;
        let weight = parameter_f32(&self.module, "rms_norm", "weight", &[self.feature_size])?;
        let rows = dims[..dims.len() - 1].iter().product::<usize>().max(1);
        let mut output = vec![0.0; values.len()];
        for row in 0..rows {
            let offset = row * self.feature_size;
            let chunk = &values[offset..offset + self.feature_size];
            let mean_square =
                chunk.iter().map(|value| value * value).sum::<f32>() / self.feature_size as f32;
            let scale = 1.0 / (mean_square + self.epsilon).sqrt();
            for index in 0..self.feature_size {
                output[offset + index] = chunk[index] * scale * weight[index];
            }
        }
        NnTensor::f32(Shape::new(dims.to_vec()), output)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Activation {
    module: Module,
    kind: ActivationKind,
}

impl Activation {
    pub fn new(module_id: impl Into<String>, kind: ActivationKind) -> Result<Self, LayerError> {
        Ok(Self {
            module: Module::new(module_id, format!("{kind:?}").to_ascii_lowercase())?,
            kind,
        })
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn forward(&self, input: &NnTensor) -> Result<NnTensor, LayerError> {
        let values = input.as_f32_slice()?;
        let output = values
            .iter()
            .map(|&value| apply_activation(self.kind, value))
            .collect::<Vec<_>>();
        NnTensor::f32(Shape::new(input.dims().to_vec()), output)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dropout {
    module: Module,
    probability: f32,
}

impl Dropout {
    pub fn new(module_id: impl Into<String>, probability: f32) -> Result<Self, LayerError> {
        if !(0.0..1.0).contains(&probability) {
            return Err(LayerError::InvalidConfiguration {
                layer: "dropout",
                message: format!("probability must satisfy 0 <= p < 1; found {probability}"),
            });
        }
        Ok(Self {
            module: Module::new(module_id, "dropout")?,
            probability,
        })
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn forward_eval(&self, input: &NnTensor) -> Result<NnTensor, LayerError> {
        NnTensor::f32(
            Shape::new(input.dims().to_vec()),
            input.as_f32_slice()?.to_vec(),
        )
    }

    pub fn forward_train(&self, input: &NnTensor, seed: u64) -> Result<NnTensor, LayerError> {
        let values = input.as_f32_slice()?;
        if self.probability == 0.0 {
            return self.forward_eval(input);
        }
        let mut state = seed.max(1);
        let scale = 1.0 / (1.0 - self.probability);
        let output = values
            .iter()
            .map(|&value| {
                let draw = next_uniform(&mut state);
                if draw < self.probability {
                    0.0
                } else {
                    value * scale
                }
            })
            .collect::<Vec<_>>();
        NnTensor::f32(Shape::new(input.dims().to_vec()), output)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Conv1d {
    module: Module,
    in_channels: usize,
    out_channels: usize,
    kernel_size: usize,
    stride: usize,
    padding: usize,
    use_bias: bool,
}

impl Conv1d {
    pub fn new(
        module_id: impl Into<String>,
        in_channels: usize,
        out_channels: usize,
        kernel_size: usize,
        stride: usize,
        padding: usize,
        use_bias: bool,
    ) -> Result<Self, LayerError> {
        ensure_positive("conv1d", "in_channels", in_channels)?;
        ensure_positive("conv1d", "out_channels", out_channels)?;
        ensure_positive("conv1d", "kernel_size", kernel_size)?;
        ensure_positive("conv1d", "stride", stride)?;
        let weight = vec![0.0; out_channels * in_channels * kernel_size];
        let bias = use_bias.then(|| vec![0.0; out_channels]);
        Self::from_f32_parts(
            module_id,
            in_channels,
            out_channels,
            kernel_size,
            stride,
            padding,
            weight,
            bias,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_f32_parts(
        module_id: impl Into<String>,
        in_channels: usize,
        out_channels: usize,
        kernel_size: usize,
        stride: usize,
        padding: usize,
        weight: Vec<f32>,
        bias: Option<Vec<f32>>,
    ) -> Result<Self, LayerError> {
        ensure_positive("conv1d", "in_channels", in_channels)?;
        ensure_positive("conv1d", "out_channels", out_channels)?;
        ensure_positive("conv1d", "kernel_size", kernel_size)?;
        ensure_positive("conv1d", "stride", stride)?;
        let mut module = Module::new(module_id, "conv1d")?;
        module.insert_parameter(
            "weight",
            dense_parameter(&[out_channels, in_channels, kernel_size], weight, true)?,
        )?;
        let use_bias = bias.is_some();
        if let Some(bias) = bias {
            module.insert_parameter("bias", dense_parameter(&[out_channels], bias, true)?)?;
        }
        Ok(Self {
            module,
            in_channels,
            out_channels,
            kernel_size,
            stride,
            padding,
            use_bias,
        })
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn forward(&self, input: &NnTensor) -> Result<NnTensor, LayerError> {
        let dims = input.dims();
        if dims.len() != 3 || dims[1] != self.in_channels {
            return Err(shape_mismatch(
                "conv1d",
                "input",
                format!("[batch, {}, length]", self.in_channels),
                dims,
            ));
        }
        let input_values = input.as_f32_slice()?;
        let batch = dims[0];
        let input_len = dims[2];
        let output_len = output_extent_1d(
            "conv1d",
            input_len,
            self.kernel_size,
            self.stride,
            self.padding,
        )?;
        let weight = parameter_f32(
            &self.module,
            "conv1d",
            "weight",
            &[self.out_channels, self.in_channels, self.kernel_size],
        )?;
        let bias = if self.use_bias {
            Some(parameter_f32(
                &self.module,
                "conv1d",
                "bias",
                &[self.out_channels],
            )?)
        } else {
            None
        };

        let mut output = vec![0.0; batch * self.out_channels * output_len];
        for batch_index in 0..batch {
            for out_channel in 0..self.out_channels {
                for out_position in 0..output_len {
                    let mut sum = bias.map_or(0.0, |bias| bias[out_channel]);
                    for in_channel in 0..self.in_channels {
                        for kernel_index in 0..self.kernel_size {
                            let source = out_position * self.stride + kernel_index;
                            let padded = source as isize - self.padding as isize;
                            if !(0..input_len as isize).contains(&padded) {
                                continue;
                            }
                            let input_offset = ((batch_index * self.in_channels + in_channel)
                                * input_len)
                                + padded as usize;
                            let weight_offset = ((out_channel * self.in_channels + in_channel)
                                * self.kernel_size)
                                + kernel_index;
                            sum += input_values[input_offset] * weight[weight_offset];
                        }
                    }
                    let output_offset = ((batch_index * self.out_channels + out_channel)
                        * output_len)
                        + out_position;
                    output[output_offset] = sum;
                }
            }
        }

        NnTensor::f32(
            Shape::new(vec![batch, self.out_channels, output_len]),
            output,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Conv2d {
    module: Module,
    in_channels: usize,
    out_channels: usize,
    kernel: [usize; 2],
    stride: [usize; 2],
    padding: [usize; 2],
    use_bias: bool,
}

impl Conv2d {
    pub fn new(
        module_id: impl Into<String>,
        in_channels: usize,
        out_channels: usize,
        kernel: [usize; 2],
        stride: [usize; 2],
        padding: [usize; 2],
        use_bias: bool,
    ) -> Result<Self, LayerError> {
        ensure_positive("conv2d", "in_channels", in_channels)?;
        ensure_positive("conv2d", "out_channels", out_channels)?;
        ensure_positive("conv2d", "kernel_height", kernel[0])?;
        ensure_positive("conv2d", "kernel_width", kernel[1])?;
        ensure_positive("conv2d", "stride_height", stride[0])?;
        ensure_positive("conv2d", "stride_width", stride[1])?;
        let weight = vec![0.0; out_channels * in_channels * kernel[0] * kernel[1]];
        let bias = use_bias.then(|| vec![0.0; out_channels]);
        Self::from_f32_parts(
            module_id,
            in_channels,
            out_channels,
            kernel,
            stride,
            padding,
            weight,
            bias,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_f32_parts(
        module_id: impl Into<String>,
        in_channels: usize,
        out_channels: usize,
        kernel: [usize; 2],
        stride: [usize; 2],
        padding: [usize; 2],
        weight: Vec<f32>,
        bias: Option<Vec<f32>>,
    ) -> Result<Self, LayerError> {
        ensure_positive("conv2d", "in_channels", in_channels)?;
        ensure_positive("conv2d", "out_channels", out_channels)?;
        ensure_positive("conv2d", "kernel_height", kernel[0])?;
        ensure_positive("conv2d", "kernel_width", kernel[1])?;
        ensure_positive("conv2d", "stride_height", stride[0])?;
        ensure_positive("conv2d", "stride_width", stride[1])?;
        let mut module = Module::new(module_id, "conv2d")?;
        module.insert_parameter(
            "weight",
            dense_parameter(
                &[out_channels, in_channels, kernel[0], kernel[1]],
                weight,
                true,
            )?,
        )?;
        let use_bias = bias.is_some();
        if let Some(bias) = bias {
            module.insert_parameter("bias", dense_parameter(&[out_channels], bias, true)?)?;
        }
        Ok(Self {
            module,
            in_channels,
            out_channels,
            kernel,
            stride,
            padding,
            use_bias,
        })
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn forward(&self, input: &NnTensor) -> Result<NnTensor, LayerError> {
        let dims = input.dims();
        if dims.len() != 4 || dims[1] != self.in_channels {
            return Err(shape_mismatch(
                "conv2d",
                "input",
                format!("[batch, {}, height, width]", self.in_channels),
                dims,
            ));
        }
        let input_values = input.as_f32_slice()?;
        let batch = dims[0];
        let input_h = dims[2];
        let input_w = dims[3];
        let output_h = output_extent_1d(
            "conv2d",
            input_h,
            self.kernel[0],
            self.stride[0],
            self.padding[0],
        )?;
        let output_w = output_extent_1d(
            "conv2d",
            input_w,
            self.kernel[1],
            self.stride[1],
            self.padding[1],
        )?;
        let weight = parameter_f32(
            &self.module,
            "conv2d",
            "weight",
            &[
                self.out_channels,
                self.in_channels,
                self.kernel[0],
                self.kernel[1],
            ],
        )?;
        let bias = if self.use_bias {
            Some(parameter_f32(
                &self.module,
                "conv2d",
                "bias",
                &[self.out_channels],
            )?)
        } else {
            None
        };

        let mut output = vec![0.0; batch * self.out_channels * output_h * output_w];
        for batch_index in 0..batch {
            for out_channel in 0..self.out_channels {
                for out_h in 0..output_h {
                    for out_w in 0..output_w {
                        let mut sum = bias.map_or(0.0, |bias| bias[out_channel]);
                        for in_channel in 0..self.in_channels {
                            for kernel_h in 0..self.kernel[0] {
                                for kernel_w in 0..self.kernel[1] {
                                    let src_h = out_h * self.stride[0] + kernel_h;
                                    let src_w = out_w * self.stride[1] + kernel_w;
                                    let padded_h = src_h as isize - self.padding[0] as isize;
                                    let padded_w = src_w as isize - self.padding[1] as isize;
                                    if !(0..input_h as isize).contains(&padded_h)
                                        || !(0..input_w as isize).contains(&padded_w)
                                    {
                                        continue;
                                    }
                                    let input_offset =
                                        (((batch_index * self.in_channels + in_channel) * input_h
                                            + padded_h as usize)
                                            * input_w)
                                            + padded_w as usize;
                                    let weight_offset = (((out_channel * self.in_channels
                                        + in_channel)
                                        * self.kernel[0]
                                        + kernel_h)
                                        * self.kernel[1])
                                        + kernel_w;
                                    sum += input_values[input_offset] * weight[weight_offset];
                                }
                            }
                        }
                        let output_offset =
                            (((batch_index * self.out_channels + out_channel) * output_h + out_h)
                                * output_w)
                                + out_w;
                        output[output_offset] = sum;
                    }
                }
            }
        }

        NnTensor::f32(
            Shape::new(vec![batch, self.out_channels, output_h, output_w]),
            output,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pool1d {
    module: Module,
    mode: PoolMode,
    kernel_size: usize,
    stride: usize,
    padding: usize,
}

impl Pool1d {
    pub fn new(
        module_id: impl Into<String>,
        mode: PoolMode,
        kernel_size: usize,
        stride: usize,
        padding: usize,
    ) -> Result<Self, LayerError> {
        ensure_positive("pool1d", "kernel_size", kernel_size)?;
        ensure_positive("pool1d", "stride", stride)?;
        Ok(Self {
            module: Module::new(
                module_id,
                match mode {
                    PoolMode::Max => "max_pool1d",
                    PoolMode::Average => "avg_pool1d",
                },
            )?,
            mode,
            kernel_size,
            stride,
            padding,
        })
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn forward(&self, input: &NnTensor) -> Result<NnTensor, LayerError> {
        let dims = input.dims();
        if dims.len() != 3 {
            return Err(shape_mismatch(
                "pool1d",
                "input",
                String::from("[batch, channels, length]"),
                dims,
            ));
        }
        let values = input.as_f32_slice()?;
        let batch = dims[0];
        let channels = dims[1];
        let input_len = dims[2];
        let output_len = output_extent_1d(
            "pool1d",
            input_len,
            self.kernel_size,
            self.stride,
            self.padding,
        )?;
        let mut output = vec![0.0; batch * channels * output_len];
        for batch_index in 0..batch {
            for channel in 0..channels {
                for out_index in 0..output_len {
                    let mut acc = match self.mode {
                        PoolMode::Max => f32::NEG_INFINITY,
                        PoolMode::Average => 0.0,
                    };
                    for kernel_index in 0..self.kernel_size {
                        let source = out_index * self.stride + kernel_index;
                        let padded = source as isize - self.padding as isize;
                        let value = if (0..input_len as isize).contains(&padded) {
                            let input_offset =
                                ((batch_index * channels + channel) * input_len) + padded as usize;
                            values[input_offset]
                        } else {
                            0.0
                        };
                        match self.mode {
                            PoolMode::Max => acc = acc.max(value),
                            PoolMode::Average => acc += value,
                        }
                    }
                    if self.mode == PoolMode::Average {
                        acc /= self.kernel_size as f32;
                    }
                    let output_offset =
                        ((batch_index * channels + channel) * output_len) + out_index;
                    output[output_offset] = acc;
                }
            }
        }
        NnTensor::f32(Shape::new(vec![batch, channels, output_len]), output)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pool2d {
    module: Module,
    mode: PoolMode,
    kernel: [usize; 2],
    stride: [usize; 2],
    padding: [usize; 2],
}

impl Pool2d {
    pub fn new(
        module_id: impl Into<String>,
        mode: PoolMode,
        kernel: [usize; 2],
        stride: [usize; 2],
        padding: [usize; 2],
    ) -> Result<Self, LayerError> {
        ensure_positive("pool2d", "kernel_height", kernel[0])?;
        ensure_positive("pool2d", "kernel_width", kernel[1])?;
        ensure_positive("pool2d", "stride_height", stride[0])?;
        ensure_positive("pool2d", "stride_width", stride[1])?;
        Ok(Self {
            module: Module::new(
                module_id,
                match mode {
                    PoolMode::Max => "max_pool2d",
                    PoolMode::Average => "avg_pool2d",
                },
            )?,
            mode,
            kernel,
            stride,
            padding,
        })
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn forward(&self, input: &NnTensor) -> Result<NnTensor, LayerError> {
        let dims = input.dims();
        if dims.len() != 4 {
            return Err(shape_mismatch(
                "pool2d",
                "input",
                String::from("[batch, channels, height, width]"),
                dims,
            ));
        }
        let values = input.as_f32_slice()?;
        let batch = dims[0];
        let channels = dims[1];
        let input_h = dims[2];
        let input_w = dims[3];
        let output_h = output_extent_1d(
            "pool2d",
            input_h,
            self.kernel[0],
            self.stride[0],
            self.padding[0],
        )?;
        let output_w = output_extent_1d(
            "pool2d",
            input_w,
            self.kernel[1],
            self.stride[1],
            self.padding[1],
        )?;
        let mut output = vec![0.0; batch * channels * output_h * output_w];
        for batch_index in 0..batch {
            for channel in 0..channels {
                for out_h in 0..output_h {
                    for out_w in 0..output_w {
                        let mut acc = match self.mode {
                            PoolMode::Max => f32::NEG_INFINITY,
                            PoolMode::Average => 0.0,
                        };
                        for kernel_h in 0..self.kernel[0] {
                            for kernel_w in 0..self.kernel[1] {
                                let src_h = out_h * self.stride[0] + kernel_h;
                                let src_w = out_w * self.stride[1] + kernel_w;
                                let padded_h = src_h as isize - self.padding[0] as isize;
                                let padded_w = src_w as isize - self.padding[1] as isize;
                                let value = if (0..input_h as isize).contains(&padded_h)
                                    && (0..input_w as isize).contains(&padded_w)
                                {
                                    let input_offset = (((batch_index * channels + channel)
                                        * input_h
                                        + padded_h as usize)
                                        * input_w)
                                        + padded_w as usize;
                                    values[input_offset]
                                } else {
                                    0.0
                                };
                                match self.mode {
                                    PoolMode::Max => acc = acc.max(value),
                                    PoolMode::Average => acc += value,
                                }
                            }
                        }
                        if self.mode == PoolMode::Average {
                            acc /= (self.kernel[0] * self.kernel[1]) as f32;
                        }
                        let output_offset =
                            (((batch_index * channels + channel) * output_h + out_h) * output_w)
                                + out_w;
                        output[output_offset] = acc;
                    }
                }
            }
        }
        NnTensor::f32(
            Shape::new(vec![batch, channels, output_h, output_w]),
            output,
        )
    }
}

fn dense_parameter(
    dims: &[usize],
    values: Vec<f32>,
    requires_grad: bool,
) -> Result<ModuleParameter, LayerError> {
    Ok(ModuleParameter::new(
        TensorSpec::new(Shape::new(dims.to_vec()), DType::F32, Device::cpu()),
        TensorData::F32(values),
        requires_grad,
    )?)
}

fn validate_dense_cpu_f32<'a>(
    layer: &'static str,
    tensor: &'static str,
    spec: &TensorSpec,
    data: &'a TensorData,
) -> Result<&'a [f32], LayerError> {
    if spec.dtype() != DType::F32 || data.as_f32_slice().is_none() {
        return Err(LayerError::UnsupportedTensor {
            layer,
            tensor,
            dtype: spec.dtype(),
            device: spec.device().clone(),
        });
    }
    if spec.device().kind() != DeviceKind::Cpu {
        return Err(LayerError::UnsupportedTensor {
            layer,
            tensor,
            dtype: spec.dtype(),
            device: spec.device().clone(),
        });
    }
    let values = data.as_f32_slice().expect("validated dense f32");
    if values.len() != spec.shape().element_count() {
        return Err(LayerError::ShapeMismatch {
            layer,
            tensor,
            expected: format!(
                "{} dense values for shape {:?}",
                spec.shape().element_count(),
                spec.shape().dims()
            ),
            actual: vec![values.len()],
        });
    }
    Ok(values)
}

fn parameter_f32<'a>(
    module: &'a Module,
    layer: &'static str,
    name: &'static str,
    expected_dims: &[usize],
) -> Result<&'a [f32], LayerError> {
    let parameter = module.parameters.get(name).ok_or_else(|| {
        LayerError::ModuleState(ModuleStateError::MissingParameter {
            path: String::from(name),
        })
    })?;
    if parameter.spec.shape().dims() != expected_dims {
        return Err(shape_mismatch(
            layer,
            name,
            format!("{expected_dims:?}"),
            parameter.spec.shape().dims(),
        ));
    }
    validate_dense_cpu_f32(layer, name, &parameter.spec, &parameter.data)
}

fn ensure_positive(
    layer: &'static str,
    field: &'static str,
    value: usize,
) -> Result<(), LayerError> {
    if value == 0 {
        return Err(LayerError::InvalidConfiguration {
            layer,
            message: format!("`{field}` must be greater than zero"),
        });
    }
    Ok(())
}

fn ensure_positive_f32(
    layer: &'static str,
    field: &'static str,
    value: f32,
) -> Result<(), LayerError> {
    if value <= 0.0 {
        return Err(LayerError::InvalidConfiguration {
            layer,
            message: format!("`{field}` must be greater than zero"),
        });
    }
    Ok(())
}

fn shape_mismatch(
    layer: &'static str,
    tensor: &'static str,
    expected: String,
    actual: &[usize],
) -> LayerError {
    LayerError::ShapeMismatch {
        layer,
        tensor,
        expected,
        actual: actual.to_vec(),
    }
}

fn output_extent_1d(
    layer: &'static str,
    input: usize,
    kernel: usize,
    stride: usize,
    padding: usize,
) -> Result<usize, LayerError> {
    ensure_positive(layer, "kernel", kernel)?;
    ensure_positive(layer, "stride", stride)?;
    let padded = input + (padding * 2);
    if padded < kernel {
        return Err(LayerError::InvalidConfiguration {
            layer,
            message: format!(
                "input extent {input} with padding {padding} is smaller than kernel {kernel}"
            ),
        });
    }
    Ok(((padded - kernel) / stride) + 1)
}

fn apply_activation(kind: ActivationKind, value: f32) -> f32 {
    match kind {
        ActivationKind::Relu => value.max(0.0),
        ActivationKind::Gelu => {
            let cubic = value * value * value;
            let inner = (std::f32::consts::FRAC_2_PI.sqrt()) * (value + 0.044_715 * cubic);
            0.5 * value * (1.0 + inner.tanh())
        }
        ActivationKind::Silu => value / (1.0 + (-value).exp()),
        ActivationKind::Tanh => value.tanh(),
    }
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
        Activation, ActivationKind, Conv1d, Conv2d, Dropout, Embedding, LayerNorm, Linear,
        NnTensor, Pool1d, Pool2d, PoolMode, RmsNorm,
    };
    use psionic_core::Shape;

    type LayerTensor = NnTensor;

    fn approx_eq(left: &[f32], right: &[f32]) {
        assert_eq!(left.len(), right.len());
        for (left, right) in left.iter().zip(right.iter()) {
            assert!((left - right).abs() < 1e-4, "{left} != {right}");
        }
    }

    #[test]
    fn linear_forward_applies_affine_projection() -> Result<(), Box<dyn std::error::Error>> {
        let layer = Linear::from_f32_parts(
            "linear0",
            2,
            2,
            vec![1.0, 2.0, 3.0, 4.0],
            Some(vec![0.5, -0.5]),
        )?;
        let input = LayerTensor::f32(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])?;
        let output = layer.forward(&input)?;
        assert_eq!(output.dims(), &[2, 2]);
        approx_eq(output.as_f32_slice()?, &[5.5, 10.5, 11.5, 24.5]);
        assert_eq!(
            layer.module().save_weights().keys(),
            vec![String::from("bias"), String::from("weight")]
        );
        Ok(())
    }

    #[test]
    fn embedding_lookup_preserves_index_shape_and_bounds() -> Result<(), Box<dyn std::error::Error>>
    {
        let layer = Embedding::from_f32_table("embed0", 3, 2, vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0])?;
        let output = layer.forward_with_shape(Shape::new(vec![1, 3]), &[2, 0, 1])?;
        assert_eq!(output.dims(), &[1, 3, 2]);
        approx_eq(output.as_f32_slice()?, &[4.0, 5.0, 0.0, 1.0, 2.0, 3.0]);

        let refusal = layer
            .forward(&[3])
            .expect_err("out-of-range index should refuse");
        assert!(refusal.to_string().contains("out of range"));
        Ok(())
    }

    #[test]
    fn norm_activation_and_dropout_layers_match_bounded_reference()
    -> Result<(), Box<dyn std::error::Error>> {
        let layer_norm = LayerNorm::new("ln0", 2, 1e-5)?;
        let rms_norm = RmsNorm::new("rms0", 2, 1e-5)?;
        let activation = Activation::new("act0", ActivationKind::Relu)?;
        let dropout = Dropout::new("drop0", 0.5)?;
        let input = LayerTensor::f32(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])?;

        let normalized = layer_norm.forward(&input)?;
        approx_eq(
            normalized.as_f32_slice()?,
            &[-0.99998, 0.99998, -0.99998, 0.99998],
        );

        let rms_input = LayerTensor::f32(Shape::new(vec![1, 2]), vec![3.0, 4.0])?;
        let rms = rms_norm.forward(&rms_input)?;
        approx_eq(rms.as_f32_slice()?, &[0.848528, 1.131371]);

        let activated = activation.forward(&normalized)?;
        approx_eq(activated.as_f32_slice()?, &[0.0, 0.99998, 0.0, 0.99998]);

        let train_one = dropout.forward_train(&activated, 7)?;
        let train_two = dropout.forward_train(&activated, 7)?;
        assert_eq!(train_one, train_two);
        for value in train_one.as_f32_slice()? {
            assert!(*value == 0.0 || (*value - 1.99996).abs() < 1e-4);
        }
        assert_eq!(dropout.forward_eval(&activated)?, activated);
        Ok(())
    }

    #[test]
    fn conv1d_and_pool1d_match_reference_windows() -> Result<(), Box<dyn std::error::Error>> {
        let conv = Conv1d::from_f32_parts("conv1", 1, 1, 3, 1, 0, vec![1.0, 0.0, -1.0], None)?;
        let input = LayerTensor::f32(Shape::new(vec![1, 1, 5]), vec![1.0, 2.0, 3.0, 4.0, 5.0])?;
        let conv_out = conv.forward(&input)?;
        approx_eq(conv_out.as_f32_slice()?, &[-2.0, -2.0, -2.0]);

        let max_pool = Pool1d::new("maxp1", PoolMode::Max, 2, 2, 0)?;
        let avg_pool = Pool1d::new("avgp1", PoolMode::Average, 2, 2, 0)?;
        let pool_input = LayerTensor::f32(Shape::new(vec![1, 1, 4]), vec![1.0, 3.0, 2.0, 4.0])?;
        approx_eq(max_pool.forward(&pool_input)?.as_f32_slice()?, &[3.0, 4.0]);
        approx_eq(avg_pool.forward(&pool_input)?.as_f32_slice()?, &[2.0, 3.0]);
        Ok(())
    }

    #[test]
    fn conv2d_and_pool2d_match_reference_windows() -> Result<(), Box<dyn std::error::Error>> {
        let conv = Conv2d::from_f32_parts(
            "conv2",
            1,
            1,
            [2, 2],
            [1, 1],
            [0, 0],
            vec![1.0, 0.0, 0.0, -1.0],
            None,
        )?;
        let input = LayerTensor::f32(
            Shape::new(vec![1, 1, 3, 3]),
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
        )?;
        let conv_out = conv.forward(&input)?;
        approx_eq(conv_out.as_f32_slice()?, &[-4.0, -4.0, -4.0, -4.0]);

        let max_pool = Pool2d::new("maxp2", PoolMode::Max, [2, 2], [1, 1], [0, 0])?;
        let avg_pool = Pool2d::new("avgp2", PoolMode::Average, [2, 2], [1, 1], [0, 0])?;
        approx_eq(
            max_pool.forward(&input)?.as_f32_slice()?,
            &[5.0, 6.0, 8.0, 9.0],
        );
        approx_eq(
            avg_pool.forward(&input)?.as_f32_slice()?,
            &[3.0, 4.0, 6.0, 7.0],
        );
        Ok(())
    }
}
