use std::borrow::Cow;

use crate::{
    Embedding, LayerError, Linear, Module, ModuleParameter, ModuleParameterView, ModuleStateError,
    NnTensor,
};
use psionic_core::{
    DType, Device, QuantizationMode, QuantizedBlockLayout, QuantizedTensorData, Shape, TensorData,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const INT8_SYMMETRIC_ELEMENTS_PER_BLOCK: usize = 32;
const INT8_SYMMETRIC_BYTES_PER_BLOCK: usize = 36;

/// Refusal posture for parameters that the current bounded quantizer cannot
/// represent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleQuantizeIneligiblePolicy {
    /// Keep unsupported parameters dense and report them explicitly.
    KeepDense,
    /// Refuse the quantize request if any targeted parameter is unsupported.
    Refuse,
}

/// Explicit public config for the bounded eval-oriented module quantizer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleQuantizeConfig {
    /// Quantization family requested by the caller.
    pub mode: QuantizationMode,
    /// Posture for parameters that the bounded public quantizer cannot encode.
    pub ineligible_policy: ModuleQuantizeIneligiblePolicy,
}

impl ModuleQuantizeConfig {
    /// Creates the default config for one requested mode.
    #[must_use]
    pub const fn new(mode: QuantizationMode) -> Self {
        Self {
            mode,
            ineligible_policy: ModuleQuantizeIneligiblePolicy::KeepDense,
        }
    }

    /// Creates a strict config that refuses unsupported target weights.
    #[must_use]
    pub const fn strict(mode: QuantizationMode) -> Self {
        Self {
            mode,
            ineligible_policy: ModuleQuantizeIneligiblePolicy::Refuse,
        }
    }
}

/// Machine-readable summary of one module quantize operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleQuantizeReport {
    /// Requested quantization family.
    pub mode: QuantizationMode,
    /// Ineligible-parameter posture.
    pub ineligible_policy: ModuleQuantizeIneligiblePolicy,
    /// Stable digest of the source module before quantization.
    pub source_module_digest: String,
    /// Stable digest of the quantized eval module.
    pub quantized_module_digest: String,
    /// Deterministic parameter paths that were encoded as quantized storage.
    pub quantized_paths: Vec<String>,
    /// Deterministic parameter paths that stayed dense.
    pub preserved_dense_paths: Vec<String>,
    /// Deterministic parameter paths frozen for eval-only quantized posture.
    pub frozen_paths: Vec<String>,
}

/// Error returned by the bounded quantized module surface.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum QuantizationError {
    /// Lower-level module tree error.
    #[error(transparent)]
    ModuleState(#[from] ModuleStateError),
    /// The public module quantizer does not support the requested mode yet.
    #[error("public module quantize mode `{mode:?}` is unsupported: {detail}")]
    UnsupportedMode {
        /// Requested mode.
        mode: QuantizationMode,
        /// Plain-language refusal detail.
        detail: &'static str,
    },
    /// One target module family is not part of the bounded quantized surface.
    #[error(
        "module quantize does not support module kind `{module_kind}` at `{module_path}` on the current public surface"
    )]
    UnsupportedModuleKind {
        /// Owning module path. The empty path is the root module.
        module_path: String,
        /// Human-readable module kind.
        module_kind: String,
    },
    /// One source parameter is not dense CPU `f32`.
    #[error(
        "quantized parameter `{path}` requires dense contiguous cpu f32 source, found dtype {dtype:?} on device {device}"
    )]
    UnsupportedSourceTensor {
        /// Stable parameter path.
        path: String,
        /// Actual source dtype.
        dtype: DType,
        /// Actual source device.
        device: Device,
    },
    /// One source tensor layout is not contiguous.
    #[error("quantized parameter `{path}` requires contiguous dense storage")]
    NonContiguousSourceTensor {
        /// Stable parameter path.
        path: String,
    },
    /// One source tensor does not fit the current block contract.
    #[error(
        "quantized parameter `{path}` with {element_count} elements is incompatible with mode {mode:?}; required multiple of {required_multiple}"
    )]
    ElementCountNotBlockAligned {
        /// Stable parameter path.
        path: String,
        /// Requested mode.
        mode: QuantizationMode,
        /// Actual element count.
        element_count: usize,
        /// Required block multiple.
        required_multiple: usize,
    },
    /// One pre-existing quantized tensor used a different mode than requested.
    #[error(
        "quantized parameter `{path}` already uses mode {actual:?}, which does not match requested mode {requested:?}"
    )]
    ExistingQuantizationModeMismatch {
        /// Stable parameter path.
        path: String,
        /// Requested mode.
        requested: QuantizationMode,
        /// Actual stored mode.
        actual: QuantizationMode,
    },
    /// One quantized wrapper was created from the wrong root module kind.
    #[error(
        "quantized wrapper `{wrapper}` requires root module kind `{expected}`, found `{actual}`"
    )]
    InvalidWrapperModuleKind {
        /// Wrapper label.
        wrapper: &'static str,
        /// Expected module kind.
        expected: &'static str,
        /// Actual module kind.
        actual: String,
    },
    /// One required parameter is missing from the wrapped module.
    #[error("quantized wrapper `{wrapper}` is missing required parameter `{path}`")]
    MissingParameter {
        /// Wrapper label.
        wrapper: &'static str,
        /// Missing stable path.
        path: String,
    },
    /// One required quantized weight stayed dense.
    #[error("quantized wrapper `{wrapper}` requires quantized weight parameter `{path}`")]
    MissingQuantizedWeight {
        /// Wrapper label.
        wrapper: &'static str,
        /// Expected quantized weight path.
        path: String,
    },
    /// One module weight shape did not match the wrapper contract.
    #[error(
        "quantized wrapper `{wrapper}` expected parameter `{path}` shape {expected}, found {actual:?}"
    )]
    InvalidParameterShape {
        /// Wrapper label.
        wrapper: &'static str,
        /// Stable parameter path.
        path: String,
        /// Human-readable expected shape.
        expected: String,
        /// Actual shape.
        actual: Vec<usize>,
    },
    /// One block payload used the wrong serialized byte length.
    #[error(
        "quantized parameter `{path}` byte length mismatch for mode {mode:?}: expected {expected_bytes}, found {actual_bytes}"
    )]
    QuantizedByteLengthMismatch {
        /// Stable parameter path.
        path: String,
        /// Stored mode.
        mode: QuantizationMode,
        /// Expected byte count.
        expected_bytes: usize,
        /// Actual byte count.
        actual_bytes: usize,
    },
    /// One quantized tensor used a layout incompatible with the bounded decoder.
    #[error("quantized parameter `{path}` used invalid layout for mode {mode:?}: {detail}")]
    InvalidQuantizedLayout {
        /// Stable parameter path.
        path: String,
        /// Stored mode.
        mode: QuantizationMode,
        /// Plain-language detail.
        detail: String,
    },
    /// One quantized module mixes multiple quantization families.
    #[error("quantized module mixes multiple quantization families: {modes:?}")]
    MixedQuantizationModes {
        /// Distinct quantization modes found in the module tree.
        modes: Vec<QuantizationMode>,
    },
    /// A caller tried to wrap a module that contains no quantized weights.
    #[error("module `{module_id}` does not contain any quantized weights")]
    MissingQuantizedPaths {
        /// Stable module identifier.
        module_id: String,
    },
}

/// Eval-oriented module tree whose supported weight parameters are quantized.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuantizedModule {
    module: Module,
    report: ModuleQuantizeReport,
}

impl QuantizedModule {
    /// Creates a bounded eval-oriented quantized module from an existing module
    /// tree that already carries quantized weights.
    pub fn from_module(mut module: Module) -> Result<Self, QuantizationError> {
        let source_module_digest = module.stable_digest();
        let mut quantized_paths = Vec::new();
        let mut preserved_dense_paths = Vec::new();
        let mut modes = Vec::new();

        for (path, parameter) in module.named_parameters() {
            match &parameter.data {
                TensorData::QuantizedBlocks(quantized) => {
                    quantized_paths.push(path);
                    if !modes.contains(&quantized.mode) {
                        modes.push(quantized.mode);
                    }
                }
                TensorData::F32(_) => preserved_dense_paths.push(path),
            }
        }

        if quantized_paths.is_empty() {
            return Err(QuantizationError::MissingQuantizedPaths {
                module_id: module.module_id.clone(),
            });
        }
        if modes.len() > 1 {
            return Err(QuantizationError::MixedQuantizationModes { modes });
        }

        let _ = module.freeze();
        let frozen_paths = module
            .named_parameters_with_view(ModuleParameterView::FrozenOnly)
            .into_iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>();
        let quantized_module_digest = module.stable_digest();
        Ok(Self {
            module,
            report: ModuleQuantizeReport {
                mode: modes[0],
                ineligible_policy: ModuleQuantizeIneligiblePolicy::KeepDense,
                source_module_digest,
                quantized_module_digest,
                quantized_paths,
                preserved_dense_paths,
                frozen_paths,
            },
        })
    }

    fn quantize_from_module(
        source: &Module,
        config: ModuleQuantizeConfig,
    ) -> Result<Self, QuantizationError> {
        if config.mode != QuantizationMode::Int8Symmetric {
            return Err(QuantizationError::UnsupportedMode {
                mode: config.mode,
                detail: "the bounded public quantizer currently emits eval-only int8_symmetric blocks only",
            });
        }

        let source_module_digest = source.stable_digest();
        let mut module = source.clone();
        let mut quantized_paths = Vec::new();
        let mut preserved_dense_paths = Vec::new();

        for (path, parameter) in source.named_parameters() {
            let (module_path, _) = split_state_path(path.as_str())?;
            let module_kind = source.submodule(module_path.as_str())?.module_kind.clone();
            if should_quantize_parameter(module_kind.as_str(), path.as_str()) {
                match quantize_parameter_payload(path.as_str(), parameter, config.mode)? {
                    Some(data) => {
                        module_parameter_mut(&mut module, path.as_str())?.data = data;
                        quantized_paths.push(path);
                    }
                    None => match config.ineligible_policy {
                        ModuleQuantizeIneligiblePolicy::KeepDense => {
                            preserved_dense_paths.push(path);
                        }
                        ModuleQuantizeIneligiblePolicy::Refuse => {
                            return Err(QuantizationError::ElementCountNotBlockAligned {
                                path,
                                mode: config.mode,
                                element_count: parameter.spec.element_count(),
                                required_multiple: INT8_SYMMETRIC_ELEMENTS_PER_BLOCK,
                            });
                        }
                    },
                }
            } else {
                if has_weight_suffix(path.as_str())
                    && config.ineligible_policy == ModuleQuantizeIneligiblePolicy::Refuse
                {
                    return Err(QuantizationError::UnsupportedModuleKind {
                        module_path,
                        module_kind,
                    });
                }
                preserved_dense_paths.push(path);
            }
        }

        let _ = module.freeze();
        let frozen_paths = module
            .named_parameters_with_view(ModuleParameterView::FrozenOnly)
            .into_iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>();
        let quantized_module_digest = module.stable_digest();
        Ok(Self {
            module,
            report: ModuleQuantizeReport {
                mode: config.mode,
                ineligible_policy: config.ineligible_policy,
                source_module_digest,
                quantized_module_digest,
                quantized_paths,
                preserved_dense_paths,
                frozen_paths,
            },
        })
    }

    /// Returns the quantized eval module.
    #[must_use]
    pub fn module(&self) -> &Module {
        &self.module
    }

    /// Returns the machine-readable quantize report.
    #[must_use]
    pub fn report(&self) -> &ModuleQuantizeReport {
        &self.report
    }

    /// Returns the quantization mode carried by the module.
    #[must_use]
    pub const fn mode(&self) -> QuantizationMode {
        self.report.mode
    }

    /// Consumes the wrapper and returns the owned module tree.
    #[must_use]
    pub fn into_module(self) -> Module {
        self.module
    }

    /// Materializes the module tree back into dense frozen `f32` parameters for
    /// CPU-reference inspection.
    pub fn dequantize_to_module(&self) -> Result<Module, QuantizationError> {
        let mut dense = self.module.clone();
        let parameter_paths = dense
            .named_parameters()
            .into_iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>();
        for path in parameter_paths {
            let (spec, data) = {
                let parameter = dense.parameter(path.as_str())?;
                (parameter.spec.clone(), parameter.data.clone())
            };
            let values = match data {
                TensorData::F32(values) => values,
                TensorData::QuantizedBlocks(_) => {
                    dequantize_tensor_data(path.as_str(), &spec, &data)?.into_owned()
                }
            };
            module_parameter_mut(&mut dense, path.as_str())?.data = TensorData::F32(values);
        }
        Ok(dense)
    }
}

impl Module {
    /// Quantizes the supported weight submodules using the default
    /// keep-ineligible-dense posture.
    pub fn quantize(&self, mode: QuantizationMode) -> Result<QuantizedModule, QuantizationError> {
        self.quantize_with_config(ModuleQuantizeConfig::new(mode))
    }

    /// Quantizes the supported weight submodules using the requested config.
    pub fn quantize_with_config(
        &self,
        config: ModuleQuantizeConfig,
    ) -> Result<QuantizedModule, QuantizationError> {
        QuantizedModule::quantize_from_module(self, config)
    }
}

/// Eval-oriented quantized `Linear` wrapper above the shared module/state
/// substrate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuantizedLinear {
    quantized: QuantizedModule,
    in_features: usize,
    out_features: usize,
    use_bias: bool,
}

impl QuantizedLinear {
    /// Quantizes one dense linear layer using the bounded public quantizer.
    pub fn from_linear(linear: &Linear, mode: QuantizationMode) -> Result<Self, QuantizationError> {
        let quantized = linear
            .module()
            .quantize_with_config(ModuleQuantizeConfig::strict(mode))?;
        Self::from_quantized_module(quantized)
    }

    /// Wraps one already-quantized linear module tree.
    pub fn from_module(module: Module) -> Result<Self, QuantizationError> {
        Self::from_quantized_module(QuantizedModule::from_module(module)?)
    }

    fn from_quantized_module(quantized: QuantizedModule) -> Result<Self, QuantizationError> {
        if quantized.module().module_kind != "linear" {
            return Err(QuantizationError::InvalidWrapperModuleKind {
                wrapper: "quantized_linear",
                expected: "linear",
                actual: quantized.module().module_kind.clone(),
            });
        }
        let (out_features, in_features, use_bias) = {
            let weight = quantized.module().parameter("weight").map_err(|_| {
                QuantizationError::MissingParameter {
                    wrapper: "quantized_linear",
                    path: String::from("weight"),
                }
            })?;
            if !matches!(weight.data, TensorData::QuantizedBlocks(_)) {
                return Err(QuantizationError::MissingQuantizedWeight {
                    wrapper: "quantized_linear",
                    path: String::from("weight"),
                });
            }
            let dims = weight.spec.shape().dims();
            if dims.len() != 2 {
                return Err(QuantizationError::InvalidParameterShape {
                    wrapper: "quantized_linear",
                    path: String::from("weight"),
                    expected: String::from("[out_features, in_features]"),
                    actual: dims.to_vec(),
                });
            }
            let use_bias = quantized.module().parameters.contains_key("bias");
            if use_bias {
                let bias = quantized.module().parameter("bias").map_err(|_| {
                    QuantizationError::MissingParameter {
                        wrapper: "quantized_linear",
                        path: String::from("bias"),
                    }
                })?;
                if bias.spec.shape().dims() != [dims[0]] {
                    return Err(QuantizationError::InvalidParameterShape {
                        wrapper: "quantized_linear",
                        path: String::from("bias"),
                        expected: format!("[{}]", dims[0]),
                        actual: bias.spec.shape().dims().to_vec(),
                    });
                }
            }
            (dims[0], dims[1], use_bias)
        };
        Ok(Self {
            quantized,
            in_features,
            out_features,
            use_bias,
        })
    }

    /// Returns the wrapped quantized module.
    #[must_use]
    pub fn quantized_module(&self) -> &QuantizedModule {
        &self.quantized
    }

    /// Returns the underlying module tree.
    #[must_use]
    pub fn module(&self) -> &Module {
        self.quantized.module()
    }

    /// Returns the quantization family used by the wrapper.
    #[must_use]
    pub const fn quantization_mode(&self) -> QuantizationMode {
        self.quantized.mode()
    }

    /// Runs bounded CPU-reference forward by explicitly dequantizing the weight
    /// view to `f32`.
    pub fn forward(&self, input: &NnTensor) -> Result<NnTensor, LayerError> {
        let values = input.as_f32_slice()?;
        let dims = input.dims();
        if dims.is_empty() {
            return Err(LayerError::ShapeMismatch {
                layer: "quantized_linear",
                tensor: "input",
                expected: format!("rank >= 1 with trailing dimension {}", self.in_features),
                actual: dims.to_vec(),
            });
        }
        if dims[dims.len() - 1] != self.in_features {
            return Err(LayerError::ShapeMismatch {
                layer: "quantized_linear",
                tensor: "input",
                expected: format!("last dimension {}", self.in_features),
                actual: dims.to_vec(),
            });
        }

        let rows = dims[..dims.len() - 1].iter().product::<usize>().max(1);
        let weight = parameter_values_f32(
            self.module(),
            "quantized_linear",
            "weight",
            &[self.out_features, self.in_features],
        )?;
        let bias = if self.use_bias {
            Some(parameter_values_f32(
                self.module(),
                "quantized_linear",
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
                let mut sum = bias.as_ref().map_or(0.0, |bias| bias[out_index]);
                let weight_offset = out_index * self.in_features;
                for in_index in 0..self.in_features {
                    sum += values[input_offset + in_index] * weight[weight_offset + in_index];
                }
                output[output_offset + out_index] = sum;
            }
        }

        let mut output_dims = dims.to_vec();
        output_dims[dims.len() - 1] = self.out_features;
        NnTensor::f32(Shape::new(output_dims), output)
    }
}

/// Eval-oriented quantized `Embedding` wrapper above the shared module/state
/// substrate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuantizedEmbedding {
    quantized: QuantizedModule,
    vocab_size: usize,
    embedding_dim: usize,
}

impl QuantizedEmbedding {
    /// Quantizes one dense embedding table using the bounded public quantizer.
    pub fn from_embedding(
        embedding: &Embedding,
        mode: QuantizationMode,
    ) -> Result<Self, QuantizationError> {
        let quantized = embedding
            .module()
            .quantize_with_config(ModuleQuantizeConfig::strict(mode))?;
        Self::from_quantized_module(quantized)
    }

    /// Wraps one already-quantized embedding module tree.
    pub fn from_module(module: Module) -> Result<Self, QuantizationError> {
        Self::from_quantized_module(QuantizedModule::from_module(module)?)
    }

    fn from_quantized_module(quantized: QuantizedModule) -> Result<Self, QuantizationError> {
        if quantized.module().module_kind != "embedding" {
            return Err(QuantizationError::InvalidWrapperModuleKind {
                wrapper: "quantized_embedding",
                expected: "embedding",
                actual: quantized.module().module_kind.clone(),
            });
        }
        let (vocab_size, embedding_dim) = {
            let weight = quantized.module().parameter("weight").map_err(|_| {
                QuantizationError::MissingParameter {
                    wrapper: "quantized_embedding",
                    path: String::from("weight"),
                }
            })?;
            if !matches!(weight.data, TensorData::QuantizedBlocks(_)) {
                return Err(QuantizationError::MissingQuantizedWeight {
                    wrapper: "quantized_embedding",
                    path: String::from("weight"),
                });
            }
            let dims = weight.spec.shape().dims();
            if dims.len() != 2 {
                return Err(QuantizationError::InvalidParameterShape {
                    wrapper: "quantized_embedding",
                    path: String::from("weight"),
                    expected: String::from("[vocab_size, embedding_dim]"),
                    actual: dims.to_vec(),
                });
            }
            (dims[0], dims[1])
        };
        Ok(Self {
            quantized,
            vocab_size,
            embedding_dim,
        })
    }

    /// Returns the wrapped quantized module.
    #[must_use]
    pub fn quantized_module(&self) -> &QuantizedModule {
        &self.quantized
    }

    /// Returns the underlying module tree.
    #[must_use]
    pub fn module(&self) -> &Module {
        self.quantized.module()
    }

    /// Returns the quantization family used by the wrapper.
    #[must_use]
    pub const fn quantization_mode(&self) -> QuantizationMode {
        self.quantized.mode()
    }

    /// Lookup with explicit CPU-reference dequantization.
    pub fn forward(&self, indices: &[usize]) -> Result<NnTensor, LayerError> {
        self.forward_with_shape(Shape::new(vec![indices.len()]), indices)
    }

    /// Lookup with an explicit input index shape.
    pub fn forward_with_shape(
        &self,
        index_shape: Shape,
        indices: &[usize],
    ) -> Result<NnTensor, LayerError> {
        if index_shape.element_count() != indices.len() {
            return Err(LayerError::InvalidConfiguration {
                layer: "quantized_embedding",
                message: format!(
                    "index shape {:?} expects {} elements but received {} indices",
                    index_shape.dims(),
                    index_shape.element_count(),
                    indices.len()
                ),
            });
        }
        let table = parameter_values_f32(
            self.module(),
            "quantized_embedding",
            "weight",
            &[self.vocab_size, self.embedding_dim],
        )?;
        let mut output = vec![0.0; indices.len() * self.embedding_dim];
        for (slot, &index) in indices.iter().enumerate() {
            if index >= self.vocab_size {
                return Err(LayerError::IndexOutOfRange {
                    layer: "quantized_embedding",
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

impl Linear {
    /// Quantizes one linear layer into the bounded eval-oriented public wrapper.
    pub fn quantize(&self, mode: QuantizationMode) -> Result<QuantizedLinear, QuantizationError> {
        QuantizedLinear::from_linear(self, mode)
    }
}

impl Embedding {
    /// Quantizes one embedding layer into the bounded eval-oriented public wrapper.
    pub fn quantize(
        &self,
        mode: QuantizationMode,
    ) -> Result<QuantizedEmbedding, QuantizationError> {
        QuantizedEmbedding::from_embedding(self, mode)
    }
}

fn should_quantize_parameter(module_kind: &str, path: &str) -> bool {
    has_weight_suffix(path) && matches!(module_kind, "linear" | "embedding")
}

fn has_weight_suffix(path: &str) -> bool {
    path == "weight" || path.ends_with(".weight")
}

fn split_state_path(path: &str) -> Result<(String, String), QuantizationError> {
    if path.trim().is_empty() {
        return Err(ModuleStateError::InvalidLocalName {
            name: String::from(path),
        }
        .into());
    }
    let mut segments = path.rsplitn(2, '.');
    let local_name = segments.next().unwrap_or_default();
    let module_path = segments.next().unwrap_or_default();
    Ok((String::from(module_path), String::from(local_name)))
}

fn module_parameter_mut<'a>(
    module: &'a mut Module,
    path: &str,
) -> Result<&'a mut ModuleParameter, QuantizationError> {
    let (module_path, local_name) = split_state_path(path)?;
    let owner = module.submodule_mut(module_path.as_str())?;
    owner
        .parameters
        .get_mut(local_name.as_str())
        .ok_or_else(|| ModuleStateError::MissingParameter {
            path: String::from(path),
        })
        .map_err(Into::into)
}

fn quantize_parameter_payload(
    path: &str,
    parameter: &ModuleParameter,
    mode: QuantizationMode,
) -> Result<Option<TensorData>, QuantizationError> {
    match &parameter.data {
        TensorData::F32(values) => {
            if parameter.spec.dtype() != DType::F32 || parameter.spec.device() != &Device::cpu() {
                return Err(QuantizationError::UnsupportedSourceTensor {
                    path: String::from(path),
                    dtype: parameter.spec.dtype(),
                    device: parameter.spec.device().clone(),
                });
            }
            if !parameter.spec.layout().is_contiguous() {
                return Err(QuantizationError::NonContiguousSourceTensor {
                    path: String::from(path),
                });
            }
            if values.len() % INT8_SYMMETRIC_ELEMENTS_PER_BLOCK != 0 {
                return Ok(None);
            }
            Ok(Some(TensorData::QuantizedBlocks(encode_int8_symmetric(
                values.as_slice(),
            ))))
        }
        TensorData::QuantizedBlocks(existing) => {
            if existing.mode == mode {
                Ok(Some(TensorData::QuantizedBlocks(existing.clone())))
            } else {
                Err(QuantizationError::ExistingQuantizationModeMismatch {
                    path: String::from(path),
                    requested: mode,
                    actual: existing.mode,
                })
            }
        }
    }
}

fn encode_int8_symmetric(values: &[f32]) -> QuantizedTensorData {
    let block_count = values.len() / INT8_SYMMETRIC_ELEMENTS_PER_BLOCK;
    let mut bytes = Vec::with_capacity(block_count * INT8_SYMMETRIC_BYTES_PER_BLOCK);
    for block in values.chunks_exact(INT8_SYMMETRIC_ELEMENTS_PER_BLOCK) {
        let scale = block
            .iter()
            .map(|value| value.abs())
            .fold(0.0_f32, f32::max)
            / 127.0_f32;
        let scale = if scale == 0.0 { 1.0 } else { scale };
        bytes.extend_from_slice(&scale.to_le_bytes());
        for value in block {
            let quantized = (value / scale).round().clamp(-127.0, 127.0) as i8;
            bytes.push(quantized.to_le_bytes()[0]);
        }
    }
    QuantizedTensorData::new(
        QuantizationMode::Int8Symmetric,
        QuantizedBlockLayout::new(
            INT8_SYMMETRIC_ELEMENTS_PER_BLOCK,
            INT8_SYMMETRIC_BYTES_PER_BLOCK,
            block_count,
        ),
        bytes,
    )
}

fn parameter_values_f32<'a>(
    module: &'a Module,
    layer: &'static str,
    path: &str,
    expected_shape: &[usize],
) -> Result<Cow<'a, [f32]>, LayerError> {
    let parameter = module.parameter(path)?;
    if parameter.spec.shape().dims() != expected_shape {
        return Err(LayerError::ShapeMismatch {
            layer,
            tensor: "parameter",
            expected: format!("{expected_shape:?}"),
            actual: parameter.spec.shape().dims().to_vec(),
        });
    }
    if parameter.spec.dtype() != DType::F32 || parameter.spec.device() != &Device::cpu() {
        return Err(QuantizationError::UnsupportedSourceTensor {
            path: String::from(path),
            dtype: parameter.spec.dtype(),
            device: parameter.spec.device().clone(),
        }
        .into());
    }
    dequantize_tensor_data(path, &parameter.spec, &parameter.data).map_err(Into::into)
}

fn dequantize_tensor_data<'a>(
    path: &str,
    spec: &psionic_core::TensorSpec,
    data: &'a TensorData,
) -> Result<Cow<'a, [f32]>, QuantizationError> {
    match data {
        TensorData::F32(values) => Ok(Cow::Borrowed(values.as_slice())),
        TensorData::QuantizedBlocks(quantized) => {
            if spec.element_count() != quantized.layout.element_count() {
                return Err(QuantizationError::InvalidQuantizedLayout {
                    path: String::from(path),
                    mode: quantized.mode,
                    detail: format!(
                        "logical element count {} does not match layout element count {}",
                        spec.element_count(),
                        quantized.layout.element_count()
                    ),
                });
            }
            let values = decode_quantized_values(path, quantized)?;
            Ok(Cow::Owned(values))
        }
    }
}

fn decode_quantized_values(
    path: &str,
    quantized: &QuantizedTensorData,
) -> Result<Vec<f32>, QuantizationError> {
    let expected_bytes = quantized.layout.byte_len();
    if quantized.bytes.len() != expected_bytes {
        return Err(QuantizationError::QuantizedByteLengthMismatch {
            path: String::from(path),
            mode: quantized.mode,
            expected_bytes,
            actual_bytes: quantized.bytes.len(),
        });
    }
    match quantized.mode {
        QuantizationMode::Int8Symmetric => decode_int8_symmetric_blocks(path, quantized),
        QuantizationMode::GgmlMxfp4 => decode_mxfp4_blocks(path, quantized),
        QuantizationMode::GgmlQ4_0 => decode_q4_0_blocks(path, quantized),
        QuantizationMode::GgmlQ4_1 => decode_q4_1_blocks(path, quantized),
        QuantizationMode::GgmlQ8_0 => decode_q8_0_blocks(path, quantized),
        QuantizationMode::None => Err(QuantizationError::UnsupportedMode {
            mode: QuantizationMode::None,
            detail: "quantized wrappers require a real quantization family",
        }),
    }
}

fn decode_int8_symmetric_blocks(
    path: &str,
    quantized: &QuantizedTensorData,
) -> Result<Vec<f32>, QuantizationError> {
    if quantized.layout.elements_per_block != INT8_SYMMETRIC_ELEMENTS_PER_BLOCK
        || quantized.layout.bytes_per_block != INT8_SYMMETRIC_BYTES_PER_BLOCK
    {
        return Err(QuantizationError::InvalidQuantizedLayout {
            path: String::from(path),
            mode: quantized.mode,
            detail: format!(
                "expected int8_symmetric layout ({INT8_SYMMETRIC_ELEMENTS_PER_BLOCK}, {INT8_SYMMETRIC_BYTES_PER_BLOCK}, blocks), found ({}, {}, {})",
                quantized.layout.elements_per_block,
                quantized.layout.bytes_per_block,
                quantized.layout.block_count
            ),
        });
    }

    let mut output = Vec::with_capacity(quantized.layout.element_count());
    for block in quantized.bytes.chunks_exact(INT8_SYMMETRIC_BYTES_PER_BLOCK) {
        let scale = f32::from_le_bytes([block[0], block[1], block[2], block[3]]);
        output.extend(
            block[4..]
                .iter()
                .copied()
                .map(|value| f32::from(i8::from_le_bytes([value])) * scale),
        );
    }
    Ok(output)
}

fn decode_mxfp4_blocks(
    path: &str,
    quantized: &QuantizedTensorData,
) -> Result<Vec<f32>, QuantizationError> {
    const KVALUES: [i8; 16] = [0, 1, 2, 3, 4, 6, 8, 12, 0, -1, -2, -3, -4, -6, -8, -12];

    validate_ggml_block_layout(path, quantized, 32, 17)?;
    let mut output = Vec::with_capacity(quantized.layout.element_count());
    for block in quantized.bytes.chunks_exact(17) {
        let scale = decode_e8m0_to_fp32_half(block[0]) * 0.5;
        let start = output.len();
        output.resize(start + 32, 0.0);
        for (pair_index, packed) in block[1..].iter().copied().enumerate() {
            output[start + pair_index] = f32::from(KVALUES[usize::from(packed & 0x0f)]) * scale;
            output[start + pair_index + 16] =
                f32::from(KVALUES[usize::from((packed >> 4) & 0x0f)]) * scale;
        }
    }
    Ok(output)
}

fn decode_q4_0_blocks(
    path: &str,
    quantized: &QuantizedTensorData,
) -> Result<Vec<f32>, QuantizationError> {
    validate_ggml_block_layout(path, quantized, 32, 18)?;
    let mut output = Vec::with_capacity(quantized.layout.element_count());
    for block in quantized.bytes.chunks_exact(18) {
        let scale = decode_f16_le(block[0], block[1]);
        let start = output.len();
        output.resize(start + 32, 0.0);
        for (pair_index, packed) in block[2..].iter().copied().enumerate() {
            output[start + pair_index] = f32::from((packed & 0x0f) as i8 - 8) * scale;
            output[start + pair_index + 16] = f32::from((packed >> 4) as i8 - 8) * scale;
        }
    }
    Ok(output)
}

fn decode_q4_1_blocks(
    path: &str,
    quantized: &QuantizedTensorData,
) -> Result<Vec<f32>, QuantizationError> {
    validate_ggml_block_layout(path, quantized, 32, 20)?;
    let mut output = Vec::with_capacity(quantized.layout.element_count());
    for block in quantized.bytes.chunks_exact(20) {
        let scale = decode_f16_le(block[0], block[1]);
        let min = decode_f16_le(block[2], block[3]);
        let start = output.len();
        output.resize(start + 32, 0.0);
        for (pair_index, packed) in block[4..].iter().copied().enumerate() {
            output[start + pair_index] = min + f32::from(packed & 0x0f) * scale;
            output[start + pair_index + 16] = min + f32::from(packed >> 4) * scale;
        }
    }
    Ok(output)
}

fn decode_q8_0_blocks(
    path: &str,
    quantized: &QuantizedTensorData,
) -> Result<Vec<f32>, QuantizationError> {
    validate_ggml_block_layout(path, quantized, 32, 34)?;
    let mut output = Vec::with_capacity(quantized.layout.element_count());
    for block in quantized.bytes.chunks_exact(34) {
        let scale = decode_f16_le(block[0], block[1]);
        output.extend(
            block[2..]
                .iter()
                .copied()
                .map(|value| f32::from(i8::from_le_bytes([value])) * scale),
        );
    }
    Ok(output)
}

fn validate_ggml_block_layout(
    path: &str,
    quantized: &QuantizedTensorData,
    expected_elements_per_block: usize,
    expected_bytes_per_block: usize,
) -> Result<(), QuantizationError> {
    if quantized.layout.elements_per_block != expected_elements_per_block
        || quantized.layout.bytes_per_block != expected_bytes_per_block
    {
        return Err(QuantizationError::InvalidQuantizedLayout {
            path: String::from(path),
            mode: quantized.mode,
            detail: format!(
                "expected GGML layout ({expected_elements_per_block}, {expected_bytes_per_block}, blocks), found ({}, {}, {})",
                quantized.layout.elements_per_block,
                quantized.layout.bytes_per_block,
                quantized.layout.block_count
            ),
        });
    }
    Ok(())
}

fn decode_f16_le(low: u8, high: u8) -> f32 {
    half_to_f32(u16::from_le_bytes([low, high]))
}

fn decode_e8m0_to_fp32_half(value: u8) -> f32 {
    let bits = if value == 0 {
        0x0040_0000_u32
    } else {
        u32::from(value) << 23
    };
    f32::from_bits(bits)
}

fn half_to_f32(bits: u16) -> f32 {
    let sign = u32::from(bits & 0x8000) << 16;
    let exponent = (bits >> 10) & 0x1f;
    let mantissa = bits & 0x03ff;

    let float_bits = match exponent {
        0 => {
            if mantissa == 0 {
                sign
            } else {
                let mut mantissa = u32::from(mantissa);
                let mut exponent = -14_i32;
                while (mantissa & 0x0400) == 0 {
                    mantissa <<= 1;
                    exponent -= 1;
                }
                mantissa &= 0x03ff;
                let exponent_bits = u32::try_from(exponent + 127).unwrap_or(0) << 23;
                sign | exponent_bits | (mantissa << 13)
            }
        }
        0x1f => sign | 0x7f80_0000 | (u32::from(mantissa) << 13),
        _ => {
            let exponent_bits = (u32::from(exponent) + 112) << 23;
            sign | exponent_bits | (u32::from(mantissa) << 13)
        }
    };
    f32::from_bits(float_bits)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

    use super::{
        Embedding, Linear, Module, ModuleQuantizeConfig, ModuleQuantizeIneligiblePolicy,
        QuantizationError, QuantizationMode, QuantizedEmbedding, QuantizedLinear,
    };

    fn aligned_linear() -> Result<Linear, Box<dyn std::error::Error>> {
        let weight = (0..64)
            .map(|index| ((index % 13) as f32 - 6.0) / 4.0)
            .collect::<Vec<_>>();
        let bias = vec![0.25, -0.5];
        Ok(Linear::from_f32_parts("proj", 32, 2, weight, Some(bias))?)
    }

    fn aligned_embedding() -> Result<Embedding, Box<dyn std::error::Error>> {
        let table = (0..64)
            .map(|index| ((index % 9) as f32 - 4.0) / 3.0)
            .collect::<Vec<_>>();
        Ok(Embedding::from_f32_table("tok", 2, 32, table)?)
    }

    #[test]
    fn module_quantize_reports_quantized_and_dense_paths_and_freezes_eval_copy()
    -> Result<(), Box<dyn std::error::Error>> {
        let linear = aligned_linear()?;
        let quantized = linear.module().quantize(QuantizationMode::Int8Symmetric)?;

        assert_eq!(quantized.report().mode, QuantizationMode::Int8Symmetric);
        assert_eq!(
            quantized.report().ineligible_policy,
            ModuleQuantizeIneligiblePolicy::KeepDense
        );
        assert_eq!(
            quantized.report().quantized_paths,
            vec![String::from("weight")]
        );
        assert_eq!(
            quantized.report().preserved_dense_paths,
            vec![String::from("bias")]
        );
        assert_eq!(
            quantized.report().frozen_paths,
            vec![String::from("bias"), String::from("weight")]
        );
        assert_ne!(
            quantized.report().source_module_digest,
            quantized.report().quantized_module_digest
        );
        assert!(matches!(
            &quantized.module().parameter("weight")?.data,
            psionic_core::TensorData::QuantizedBlocks(quantized)
                if quantized.mode == QuantizationMode::Int8Symmetric
        ));
        Ok(())
    }

    #[test]
    fn strict_quantize_refuses_unsupported_weight_families()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut norm = Module::new("norm0", "layer_norm")?;
        norm.insert_parameter(
            "weight",
            crate::ModuleParameter::new(
                psionic_core::TensorSpec::new(
                    psionic_core::Shape::new(vec![32]),
                    psionic_core::DType::F32,
                    psionic_core::Device::cpu(),
                ),
                psionic_core::TensorData::F32(vec![1.0; 32]),
                true,
            )?,
        )?;

        let error = norm
            .quantize_with_config(ModuleQuantizeConfig::strict(
                QuantizationMode::Int8Symmetric,
            ))
            .expect_err("layer_norm weight should refuse strict public quantize");
        assert!(matches!(
            error,
            QuantizationError::UnsupportedModuleKind { module_kind, .. } if module_kind == "layer_norm"
        ));
        Ok(())
    }

    #[test]
    fn quantized_linear_forward_tracks_dense_reference() -> Result<(), Box<dyn std::error::Error>> {
        let linear = aligned_linear()?;
        let quantized = linear.quantize(QuantizationMode::Int8Symmetric)?;
        let input = crate::NnTensor::f32(
            psionic_core::Shape::new(vec![2, 32]),
            (0..64)
                .map(|index| ((index % 7) as f32 - 3.0) / 5.0)
                .collect::<Vec<_>>(),
        )?;

        let dense_output = linear.forward(&input)?;
        let quantized_output = quantized.forward(&input)?;
        let dense = dense_output.as_f32_slice()?;
        let quantized = quantized_output.as_f32_slice()?;
        for (dense, quantized) in dense.iter().zip(quantized.iter()) {
            assert!((dense - quantized).abs() <= 0.05);
        }
        Ok(())
    }

    #[test]
    fn quantized_embedding_lookup_tracks_dense_reference() -> Result<(), Box<dyn std::error::Error>>
    {
        let embedding = aligned_embedding()?;
        let quantized = embedding.quantize(QuantizationMode::Int8Symmetric)?;

        let dense_output =
            embedding.forward_with_shape(psionic_core::Shape::new(vec![2]), &[1, 0])?;
        let quantized_output =
            quantized.forward_with_shape(psionic_core::Shape::new(vec![2]), &[1, 0])?;
        let dense = dense_output.as_f32_slice()?;
        let quantized = quantized_output.as_f32_slice()?;
        assert_eq!(dense.len(), quantized.len());
        for (dense, quantized) in dense.iter().zip(quantized.iter()) {
            assert!((dense - quantized).abs() <= 0.05);
        }
        Ok(())
    }

    #[test]
    fn quantized_linear_roundtrips_through_module_state_load()
    -> Result<(), Box<dyn std::error::Error>> {
        let linear = aligned_linear()?;
        let quantized = linear.quantize(QuantizationMode::Int8Symmetric)?;
        let weights = quantized.module().save_weights();

        let mut target = linear.module().clone();
        target.load_weights(&weights)?;
        let loaded = QuantizedLinear::from_module(target)?;
        assert_eq!(loaded.quantization_mode(), QuantizationMode::Int8Symmetric);
        assert_eq!(
            loaded.module().state_dict().state_dict_digest,
            weights.state_dict_digest
        );
        Ok(())
    }

    #[test]
    fn quantized_embedding_can_wrap_loaded_quantized_module()
    -> Result<(), Box<dyn std::error::Error>> {
        let embedding = aligned_embedding()?;
        let quantized = embedding.quantize(QuantizationMode::Int8Symmetric)?;
        let rewrapped =
            QuantizedEmbedding::from_module(quantized.quantized_module().clone().into_module())?;
        assert_eq!(
            rewrapped.quantization_mode(),
            QuantizationMode::Int8Symmetric
        );
        Ok(())
    }
}
