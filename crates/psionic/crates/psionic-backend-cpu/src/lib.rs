//! CPU backend for Psionic.

use std::{
    collections::{BTreeMap, HashMap},
    sync::atomic::{AtomicU64, Ordering},
};

use psionic_compiler::compile_graph;
use psionic_core::{
    BackendExtensionKind, BackendExtensionOp, DType, Device, QuantizationMode, Shape, TensorData,
    TensorId, TensorSpec, ViewSemantics,
    ggml_quantization::{
        GgmlBlockDecodeError, decode_iq3_s_block, decode_iq4_xs_block, decode_q3_k_block,
    },
};
use psionic_ir::{ExecutionOp, ExecutionPlan, ExecutionStep, Graph};
use psionic_runtime::{
    Allocator, AllocatorPoolMode, AllocatorPoolPolicy, AllocatorPoolReport, AllocatorPoolState,
    BackendExtensionSupport, BackendName, BackendRuntimeResources, BackendSelection, BufferHandle,
    BufferResidency, BufferStorageContract, BufferStorageIdentity, BufferStorageKind, CacheAction,
    CacheKind, CacheObservation, CompilePathEvidence, CompilePathTemperature, DeviceDescriptor,
    DeviceDiscovery, ExecutionBackend, ExecutionMetrics, ExecutionPlanCachePolicy,
    ExecutionPlanCacheReport, ExecutionPlanCacheState, ExecutionResult, HealthStatus,
    KernelCachePolicy, KernelCacheReport, KernelCacheState, QuantizationExecution,
    QuantizationLoadPath, QuantizationSupport, RuntimeError, RuntimeHealth,
};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "CPU reference backend";

const CPU_POOL_MAX_CACHED_BUFFERS: usize = 64;
const CPU_POOL_MAX_CACHED_BYTES: u64 = 8 * 1024 * 1024;
const CPU_EXECUTION_PLAN_CACHE_MAX_ENTRIES: usize = 64;
const CPU_EXECUTION_PLAN_CACHE_MAX_CACHED_BYTES: u64 = 1024 * 1024;
static NEXT_CPU_STORAGE_ID: AtomicU64 = AtomicU64::new(1);

/// Host-resident tensor buffer for CPU execution.
#[derive(Clone, Debug)]
pub struct CpuBuffer {
    spec: TensorSpec,
    storage_contract: BufferStorageContract,
    storage: CpuBufferStorage,
}

impl PartialEq for CpuBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.spec == other.spec && self.storage == other.storage
    }
}

#[derive(Clone, Debug, PartialEq)]
enum CpuBufferStorage {
    Dense(Vec<f32>),
    QuantizedBlocks {
        mode: QuantizationMode,
        layout: psionic_core::QuantizedBlockLayout,
        bytes: Vec<u8>,
    },
}

impl CpuBuffer {
    /// Builds a buffer from contiguous logical `f32` values.
    pub fn from_f32(spec: TensorSpec, data: impl Into<Vec<f32>>) -> Result<Self, RuntimeError> {
        let data = data.into();
        let expected = spec.element_count();
        if expected != data.len() {
            return Err(RuntimeError::Backend(format!(
                "buffer length mismatch: expected {expected}, actual {}",
                data.len()
            )));
        }
        if !spec.layout().is_contiguous() || spec.layout().offset() != 0 {
            return Err(RuntimeError::Backend(String::from(
                "from_f32 requires a contiguous zero-offset tensor spec",
            )));
        }
        Ok(Self {
            spec,
            storage_contract: owned_storage_contract(),
            storage: CpuBufferStorage::Dense(data),
        })
    }

    /// Builds a buffer from explicit backing storage.
    pub fn from_storage_f32(
        spec: TensorSpec,
        data: impl Into<Vec<f32>>,
    ) -> Result<Self, RuntimeError> {
        let data = data.into();
        let expected = spec.storage_size();
        if expected > data.len() {
            return Err(RuntimeError::Backend(format!(
                "storage length mismatch: required at least {expected}, actual {}",
                data.len()
            )));
        }
        Ok(Self {
            spec,
            storage_contract: owned_storage_contract(),
            storage: CpuBufferStorage::Dense(data),
        })
    }

    /// Builds a buffer from tensor data.
    pub fn from_tensor_data(spec: TensorSpec, data: &TensorData) -> Result<Self, RuntimeError> {
        match data {
            TensorData::F32(values) | TensorData::BF16(values) => {
                Self::from_f32(spec, values.clone())
            }
            TensorData::I32(_) => Err(RuntimeError::UnsupportedStep(String::from(
                "cpu dense buffer materialization for I32 tensors",
            ))),
            TensorData::QuantizedBlocks(data) => {
                Self::from_quantized_blocks(spec, data.mode, data.layout, data.bytes.clone())
            }
        }
    }

    /// Builds a buffer that preserves quantized GGML/GGUF blocks.
    pub fn from_quantized_blocks(
        spec: TensorSpec,
        mode: QuantizationMode,
        layout: psionic_core::QuantizedBlockLayout,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, RuntimeError> {
        if spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "quantized blocks require logical F32 dtype, actual {:?}",
                spec.dtype()
            )));
        }
        if !spec.layout().is_contiguous() || spec.layout().offset() != 0 {
            return Err(RuntimeError::Backend(String::from(
                "quantized blocks require a contiguous zero-offset tensor spec",
            )));
        }
        let Some(expected_layout) = mode.ggml_block_layout(spec.shape()) else {
            return Err(RuntimeError::Backend(format!(
                "shape {} is invalid for quantized mode {mode:?}",
                spec.shape()
            )));
        };
        if expected_layout != layout {
            return Err(RuntimeError::Backend(format!(
                "quantized layout mismatch: expected {:?}, actual {:?}",
                expected_layout, layout
            )));
        }
        let bytes = bytes.into();
        if bytes.len() != layout.byte_len() {
            return Err(RuntimeError::Backend(format!(
                "quantized byte length mismatch: expected {}, actual {}",
                layout.byte_len(),
                bytes.len()
            )));
        }
        Ok(Self {
            spec,
            storage_contract: owned_storage_contract(),
            storage: CpuBufferStorage::QuantizedBlocks {
                mode,
                layout,
                bytes,
            },
        })
    }

    /// Returns a zeroed buffer for a tensor spec.
    #[must_use]
    pub fn zeros(spec: &TensorSpec) -> Self {
        Self {
            spec: spec.clone(),
            storage_contract: owned_storage_contract(),
            storage: CpuBufferStorage::Dense(vec![0.0; spec.storage_size()]),
        }
    }

    /// Returns the dense backing storage when the buffer is materialized as `f32`.
    #[must_use]
    pub fn as_f32_slice(&self) -> Option<&[f32]> {
        match &self.storage {
            CpuBufferStorage::Dense(data) => Some(data.as_slice()),
            CpuBufferStorage::QuantizedBlocks { .. } => None,
        }
    }

    /// Returns logical values in row-major order.
    pub fn logical_values(&self) -> Result<Vec<f32>, RuntimeError> {
        match &self.storage {
            CpuBufferStorage::Dense(data) => {
                let mut output = Vec::with_capacity(self.spec.element_count());
                for_each_index(self.spec.shape(), |index| {
                    output.push(data[self.storage_index(index)]);
                });
                Ok(output)
            }
            CpuBufferStorage::QuantizedBlocks {
                mode,
                layout,
                bytes,
            } => decode_quantized_values(self.spec.shape(), *mode, *layout, bytes.as_slice()),
        }
    }

    fn storage_index(&self, logical_index: &[usize]) -> usize {
        self.spec.layout().offset()
            + logical_index
                .iter()
                .zip(self.spec.layout().strides().iter())
                .map(|(index, stride)| index * stride)
                .sum::<usize>()
    }

    fn view_of(source: &CpuBuffer, spec: TensorSpec) -> Result<Self, RuntimeError> {
        match &source.storage {
            CpuBufferStorage::Dense(data) => {
                let view_semantics = spec
                    .layout()
                    .view_semantics_relative_to(source.spec.layout())
                    .ok_or_else(|| {
                        RuntimeError::Backend(String::from(
                            "view spec must stay within source storage span",
                        ))
                    })?;
                Ok(Self {
                    spec,
                    storage_contract: BufferStorageContract {
                        identity: source.storage_contract.identity,
                        view_semantics,
                    },
                    storage: CpuBufferStorage::Dense(data.clone()),
                })
            }
            CpuBufferStorage::QuantizedBlocks { .. } => Err(RuntimeError::Backend(String::from(
                "views of quantized cpu buffers are unsupported",
            ))),
        }
    }

    fn quantized_blocks(
        &self,
    ) -> Option<(QuantizationMode, psionic_core::QuantizedBlockLayout, &[u8])> {
        match &self.storage {
            CpuBufferStorage::Dense(_) => None,
            CpuBufferStorage::QuantizedBlocks {
                mode,
                layout,
                bytes,
            } => Some((*mode, *layout, bytes.as_slice())),
        }
    }
}

impl BufferHandle for CpuBuffer {
    fn spec(&self) -> &TensorSpec {
        &self.spec
    }

    fn storage_kind(&self) -> BufferStorageKind {
        match &self.storage {
            CpuBufferStorage::Dense(_) => BufferStorageKind::DenseF32,
            CpuBufferStorage::QuantizedBlocks { mode, layout, .. } => {
                BufferStorageKind::QuantizedBlocks {
                    mode: *mode,
                    layout: *layout,
                    residency: BufferResidency::Host,
                }
            }
        }
    }

    fn storage_contract(&self) -> Option<BufferStorageContract> {
        Some(self.storage_contract)
    }
}

fn owned_storage_contract() -> BufferStorageContract {
    BufferStorageContract {
        identity: BufferStorageIdentity(NEXT_CPU_STORAGE_ID.fetch_add(1, Ordering::Relaxed)),
        view_semantics: ViewSemantics::Dense,
    }
}

/// CPU reference backend implementation.
#[derive(Clone, Debug)]
pub struct CpuBackend {
    pool: CpuAllocatorPool,
    execution_plan_cache: CpuExecutionPlanCache,
}

impl CpuBackend {
    /// Creates a CPU backend.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pool: CpuAllocatorPool::new(cpu_allocator_pool_policy()),
            execution_plan_cache: CpuExecutionPlanCache::new(cpu_execution_plan_cache_policy()),
        }
    }

    /// Creates a host input buffer on the default CPU device.
    pub fn input_buffer(
        &self,
        shape: Shape,
        values: impl Into<Vec<f32>>,
    ) -> Result<CpuBuffer, RuntimeError> {
        CpuBuffer::from_f32(TensorSpec::new(shape, DType::F32, Device::cpu()), values)
    }

    /// Compiles and executes a graph in one step.
    pub fn compile_and_execute(
        &mut self,
        graph: &Graph,
        inputs: &BTreeMap<TensorId, CpuBuffer>,
    ) -> Result<ExecutionResult<CpuBuffer>, RuntimeError> {
        let (plan, plan_digest, compile_path) =
            self.execution_plan_cache.lookup_or_compile(graph)?;
        let mut result = self.execute(&plan, inputs)?;
        result.metrics.execution_plan_digest = Some(plan_digest);
        result.metrics.compile_path = Some(compile_path);
        result.metrics.plan_cache_hits = usize::from(matches!(
            result
                .metrics
                .compile_path
                .as_ref()
                .map(|value| value.temperature),
            Some(CompilePathTemperature::WarmReuse)
        ));
        result.metrics.plan_cache_misses = usize::from(matches!(
            result
                .metrics
                .compile_path
                .as_ref()
                .map(|value| value.temperature),
            Some(CompilePathTemperature::ColdCompile)
        ));
        Ok(result)
    }

    /// Returns truthful provider/runtime backend selection for a CPU product path.
    pub fn backend_selection(
        &self,
        supported_ops: &[&str],
    ) -> Result<BackendSelection, RuntimeError> {
        BackendSelection::from_backend(self, supported_ops)
    }

    fn materialize_step(
        &self,
        step: &ExecutionStep,
        values: &mut BTreeMap<TensorId, CpuBuffer>,
    ) -> Result<(), RuntimeError> {
        let buffer = match &step.op {
            ExecutionOp::Input { .. } => {
                let input = values
                    .get(&step.output)
                    .ok_or(RuntimeError::MissingInput(step.output))?;
                if input.spec() != &step.spec {
                    return Err(RuntimeError::InvalidBuffer {
                        tensor: step.output,
                        expected: step.spec.clone(),
                        actual: input.spec().clone(),
                    });
                }
                input.clone()
            }
            ExecutionOp::Constant { data } => CpuBuffer::from_tensor_data(step.spec.clone(), data)?,
            ExecutionOp::Detach => self.input(step, values, 0)?.clone(),
            ExecutionOp::Add => {
                let (left, right) = self.binary_inputs(step, values)?;
                let output = left
                    .logical_values()?
                    .into_iter()
                    .zip(right.logical_values()?)
                    .map(|(lhs, rhs)| lhs + rhs)
                    .collect::<Vec<_>>();
                CpuBuffer::from_f32(step.spec.clone(), output)?
            }
            ExecutionOp::Mul => {
                let (left, right) = self.binary_inputs(step, values)?;
                let output = left
                    .logical_values()?
                    .into_iter()
                    .zip(right.logical_values()?)
                    .map(|(lhs, rhs)| lhs * rhs)
                    .collect::<Vec<_>>();
                CpuBuffer::from_f32(step.spec.clone(), output)?
            }
            ExecutionOp::BackendExtension { op } => self.backend_extension(step, values, op)?,
            ExecutionOp::Matmul => self.matmul(step, values)?,
            ExecutionOp::Reshape => {
                let source = self.input(step, values, 0)?.logical_values()?;
                CpuBuffer::from_f32(step.spec.clone(), source)?
            }
            ExecutionOp::Permute { .. }
            | ExecutionOp::Slice { .. }
            | ExecutionOp::Select { .. }
            | ExecutionOp::Expand { .. } => {
                CpuBuffer::view_of(self.input(step, values, 0)?, step.spec.clone())?
            }
            ExecutionOp::Cast { .. } => {
                let source = self.input(step, values, 0)?.logical_values()?;
                CpuBuffer::from_f32(step.spec.clone(), source)?
            }
            ExecutionOp::Concat { axis } => self.concat(step, values, *axis)?,
            ExecutionOp::ReduceSum { axis } => self.reduce_sum(step, values, *axis)?,
        };

        values.insert(step.output, buffer);
        Ok(())
    }

    fn backend_extension(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
        op: &BackendExtensionOp,
    ) -> Result<CpuBuffer, RuntimeError> {
        match op {
            BackendExtensionOp::ReluSquared => self.relu_squared(step, values),
            BackendExtensionOp::ReluSquaredBackward => self.relu_squared_backward(step, values),
            BackendExtensionOp::ReluSquaredBackwardFromOutput => {
                self.relu_squared_backward_from_output(step, values)
            }
            BackendExtensionOp::LeakyReluSquared { negative_slope } => {
                self.leaky_relu_squared(step, values, negative_slope.to_f32())
            }
            BackendExtensionOp::LeakyReluSquaredBackward { negative_slope } => {
                self.leaky_relu_squared_backward(step, values, negative_slope.to_f32())
            }
            BackendExtensionOp::Silu => self.silu(step, values),
            BackendExtensionOp::SiluBackward => self.silu_backward(step, values),
            BackendExtensionOp::RmsNorm { epsilon } => {
                self.rms_norm(step, values, epsilon.to_f32())
            }
            BackendExtensionOp::RmsNormInputBackward { epsilon } => {
                self.rms_norm_input_backward(step, values, epsilon.to_f32())
            }
            BackendExtensionOp::RmsNormWeightBackward { epsilon } => {
                self.rms_norm_weight_backward(step, values, epsilon.to_f32())
            }
            BackendExtensionOp::LayerNorm { epsilon } => {
                self.layer_norm(step, values, epsilon.to_f32())
            }
            BackendExtensionOp::RotaryEmbedding { interleaved } => {
                self.rotary_embedding(step, values, *interleaved)
            }
            BackendExtensionOp::RotaryEmbeddingBackward { .. } => {
                Err(RuntimeError::UnsupportedStep(op.label().to_string()))
            }
            BackendExtensionOp::ScaledDotProductAttention { scale, causal } => {
                self.scaled_dot_product_attention(step, values, scale.to_f32(), *causal)
            }
            BackendExtensionOp::ScaledDotProductAttentionQueryBackward { .. }
            | BackendExtensionOp::ScaledDotProductAttentionKeyBackward { .. }
            | BackendExtensionOp::ScaledDotProductAttentionValueBackward { .. } => {
                Err(RuntimeError::UnsupportedStep(op.label().to_string()))
            }
            BackendExtensionOp::QuantizedMatmul { rhs_mode } => {
                self.quantized_matmul(step, values, *rhs_mode)
            }
            BackendExtensionOp::ParameterGolfTokenEmbeddingLookup
            | BackendExtensionOp::ParameterGolfBankedLinear { .. }
            | BackendExtensionOp::ParameterGolfBankedLinearInputBackward { .. }
            | BackendExtensionOp::ParameterGolfBankedLinearWeightBackward { .. }
            | BackendExtensionOp::ParameterGolfTokenEmbeddingLookupBackward
            | BackendExtensionOp::ParameterGolfProjectionLoss { .. }
            | BackendExtensionOp::ParameterGolfProjectionTokenLosses { .. }
            | BackendExtensionOp::ParameterGolfProjectionLossBackward { .. } => {
                Err(RuntimeError::UnsupportedStep(op.label().to_string()))
            }
        }
    }

    fn input<'a>(
        &self,
        step: &ExecutionStep,
        values: &'a BTreeMap<TensorId, CpuBuffer>,
        index: usize,
    ) -> Result<&'a CpuBuffer, RuntimeError> {
        let Some(tensor_id) = step.inputs.get(index).copied() else {
            return Err(RuntimeError::Backend(format!(
                "missing input {index} for step {}",
                step.output
            )));
        };
        values
            .get(&tensor_id)
            .ok_or(RuntimeError::MissingInput(tensor_id))
    }

    fn binary_inputs<'a>(
        &self,
        step: &ExecutionStep,
        values: &'a BTreeMap<TensorId, CpuBuffer>,
    ) -> Result<(&'a CpuBuffer, &'a CpuBuffer), RuntimeError> {
        Ok((self.input(step, values, 0)?, self.input(step, values, 1)?))
    }

    fn matmul(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
    ) -> Result<CpuBuffer, RuntimeError> {
        let (left, right) = self.binary_inputs(step, values)?;
        let left_shape = left.spec().shape().dims();
        let right_shape = right.spec().shape().dims();
        if left_shape.len() != 2 || right_shape.len() != 2 || left_shape[1] != right_shape[0] {
            return Err(RuntimeError::Backend(String::from(
                "invalid matmul shapes at runtime",
            )));
        }

        let m = left_shape[0];
        let k = left_shape[1];
        let n = right_shape[1];
        let mut output = vec![0.0; m * n];
        if left.quantized_blocks().is_some() || right.quantized_blocks().is_some() {
            return Err(RuntimeError::Backend(String::from(
                "dense matmul does not accept quantized block storage; use quantized_matmul",
            )));
        }
        let left_values = left.logical_values()?;
        let right_values = right.logical_values()?;

        for row in 0..m {
            for col in 0..n {
                let mut sum = 0.0;
                for inner in 0..k {
                    let lhs = left_values[row * k + inner];
                    let rhs = right_values[inner * n + col];
                    sum += lhs * rhs;
                }
                output[row * n + col] = sum;
            }
        }

        CpuBuffer::from_f32(step.spec.clone(), output)
    }

    fn concat(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
        axis: usize,
    ) -> Result<CpuBuffer, RuntimeError> {
        let input_buffers = step
            .inputs
            .iter()
            .map(|tensor_id| {
                values
                    .get(tensor_id)
                    .cloned()
                    .ok_or(RuntimeError::MissingInput(*tensor_id))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let output_dims = step.spec.shape().dims();
        let inner = product(&output_dims[axis + 1..]);
        let outer = product(&output_dims[..axis]);
        let mut output = Vec::with_capacity(step.spec.element_count());

        let logical_inputs = input_buffers
            .iter()
            .map(CpuBuffer::logical_values)
            .collect::<Result<Vec<_>, _>>()?;

        for outer_index in 0..outer {
            for (buffer, logical) in input_buffers.iter().zip(logical_inputs.iter()) {
                let axis_len = buffer.spec().shape().dims()[axis];
                let chunk = axis_len * inner;
                let start = outer_index * chunk;
                output.extend_from_slice(&logical[start..start + chunk]);
            }
        }

        CpuBuffer::from_f32(step.spec.clone(), output)
    }

    fn reduce_sum(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
        axis: Option<usize>,
    ) -> Result<CpuBuffer, RuntimeError> {
        let input = self.input(step, values, 0)?;
        let logical = input.logical_values()?;
        let output = match axis {
            None => vec![logical.iter().copied().sum()],
            Some(axis) => reduce_sum_axis(&logical, input.spec().shape().dims(), axis)?,
        };
        CpuBuffer::from_f32(step.spec.clone(), output)
    }

    fn relu_squared(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
    ) -> Result<CpuBuffer, RuntimeError> {
        let input = self.input(step, values, 0)?.logical_values()?;
        let output = input
            .iter()
            .map(|value| {
                let positive = value.max(0.0);
                positive * positive
            })
            .collect::<Vec<_>>();
        CpuBuffer::from_f32(step.spec.clone(), output)
    }

    fn relu_squared_backward(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
    ) -> Result<CpuBuffer, RuntimeError> {
        let input = self.input(step, values, 0)?.logical_values()?;
        let grad_output = self.input(step, values, 1)?.logical_values()?;
        if input.len() != grad_output.len() {
            return Err(RuntimeError::Backend(format!(
                "cpu relu_squared_backward requires matching input and grad_output lengths, actual {} and {}",
                input.len(),
                grad_output.len()
            )));
        }
        let output = input
            .iter()
            .zip(grad_output.iter())
            .map(|(value, grad)| {
                if *value > 0.0 {
                    grad * (2.0 * value)
                } else {
                    0.0
                }
            })
            .collect::<Vec<_>>();
        CpuBuffer::from_f32(step.spec.clone(), output)
    }

    fn relu_squared_backward_from_output(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
    ) -> Result<CpuBuffer, RuntimeError> {
        let output = self.input(step, values, 0)?.logical_values()?;
        let grad_output = self.input(step, values, 1)?.logical_values()?;
        if output.len() != grad_output.len() {
            return Err(RuntimeError::Backend(format!(
                "cpu relu_squared_backward_from_output requires matching output and grad_output lengths, actual {} and {}",
                output.len(),
                grad_output.len()
            )));
        }
        let input_gradient = output
            .iter()
            .zip(grad_output.iter())
            .map(|(value, grad)| {
                if *value > 0.0 {
                    grad * (2.0 * value.sqrt())
                } else {
                    0.0
                }
            })
            .collect::<Vec<_>>();
        CpuBuffer::from_f32(step.spec.clone(), input_gradient)
    }

    fn leaky_relu_squared(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
        negative_slope: f32,
    ) -> Result<CpuBuffer, RuntimeError> {
        let input = self.input(step, values, 0)?.logical_values()?;
        let output = input
            .iter()
            .map(|value| {
                let activated = if *value >= 0.0 {
                    *value
                } else {
                    value * negative_slope
                };
                activated * activated
            })
            .collect::<Vec<_>>();
        CpuBuffer::from_f32(step.spec.clone(), output)
    }

    fn leaky_relu_squared_backward(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
        negative_slope: f32,
    ) -> Result<CpuBuffer, RuntimeError> {
        let input = self.input(step, values, 0)?.logical_values()?;
        let grad_output = self.input(step, values, 1)?.logical_values()?;
        if input.len() != grad_output.len() {
            return Err(RuntimeError::Backend(format!(
                "cpu leaky_relu_squared_backward requires matching input and grad_output lengths, actual {} and {}",
                input.len(),
                grad_output.len()
            )));
        }
        let output = input
            .iter()
            .zip(grad_output.iter())
            .map(|(value, grad)| {
                let derivative = if *value >= 0.0 {
                    2.0 * value
                } else {
                    2.0 * negative_slope * negative_slope * value
                };
                grad * derivative
            })
            .collect::<Vec<_>>();
        CpuBuffer::from_f32(step.spec.clone(), output)
    }

    fn silu(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
    ) -> Result<CpuBuffer, RuntimeError> {
        let input = self.input(step, values, 0)?.logical_values()?;
        let output = input
            .iter()
            .map(|value| {
                let sigmoid = 1.0 / (1.0 + (-value).exp());
                value * sigmoid
            })
            .collect::<Vec<_>>();
        CpuBuffer::from_f32(step.spec.clone(), output)
    }

    fn silu_backward(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
    ) -> Result<CpuBuffer, RuntimeError> {
        let input = self.input(step, values, 0)?.logical_values()?;
        let grad_output = self.input(step, values, 1)?.logical_values()?;
        if input.len() != grad_output.len() {
            return Err(RuntimeError::Backend(format!(
                "cpu silu_backward requires matching input and grad_output lengths, actual {} and {}",
                input.len(),
                grad_output.len()
            )));
        }
        let output = input
            .iter()
            .zip(grad_output.iter())
            .map(|(value, grad)| {
                let sigmoid = 1.0 / (1.0 + (-value).exp());
                let derivative = sigmoid * (1.0 + value * (1.0 - sigmoid));
                grad * derivative
            })
            .collect::<Vec<_>>();
        CpuBuffer::from_f32(step.spec.clone(), output)
    }

    fn rms_norm(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
        epsilon: f32,
    ) -> Result<CpuBuffer, RuntimeError> {
        let input = self.input(step, values, 0)?.logical_values()?;
        let weight = self.input(step, values, 1)?.logical_values()?;
        let last_dim = weight.len();
        let mut output = vec![0.0; input.len()];
        for (src_row, dst_row) in input
            .chunks_exact(last_dim)
            .zip(output.chunks_exact_mut(last_dim))
        {
            let mean_square =
                src_row.iter().map(|value| value * value).sum::<f32>() / last_dim as f32;
            let inv = (mean_square + epsilon).sqrt().recip();
            for ((dst, value), scale) in dst_row.iter_mut().zip(src_row.iter()).zip(weight.iter()) {
                *dst = *value * inv * *scale;
            }
        }
        CpuBuffer::from_f32(step.spec.clone(), output)
    }

    fn rms_norm_input_backward(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
        epsilon: f32,
    ) -> Result<CpuBuffer, RuntimeError> {
        let input = self.input(step, values, 0)?.logical_values()?;
        let weight = self.input(step, values, 1)?.logical_values()?;
        let grad_output = self.input(step, values, 2)?.logical_values()?;
        let last_dim = weight.len();
        let mut output = vec![0.0_f32; input.len()];
        for ((src_row, grad_row), dst_row) in input
            .chunks_exact(last_dim)
            .zip(grad_output.chunks_exact(last_dim))
            .zip(output.chunks_exact_mut(last_dim))
        {
            let mean_square =
                src_row.iter().map(|value| value * value).sum::<f32>() / last_dim as f32;
            let inv = (mean_square + epsilon).sqrt().recip();
            let inv_cubed = inv * inv * inv;
            let weighted_dot = src_row
                .iter()
                .zip(weight.iter())
                .zip(grad_row.iter())
                .map(|((value, scale), grad)| value * scale * grad)
                .sum::<f32>();
            for (((dst, value), scale), grad) in dst_row
                .iter_mut()
                .zip(src_row.iter())
                .zip(weight.iter())
                .zip(grad_row.iter())
            {
                *dst = (grad * scale * inv) - (value * inv_cubed * weighted_dot / last_dim as f32);
            }
        }
        CpuBuffer::from_f32(step.spec.clone(), output)
    }

    fn rms_norm_weight_backward(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
        epsilon: f32,
    ) -> Result<CpuBuffer, RuntimeError> {
        let input = self.input(step, values, 0)?.logical_values()?;
        let grad_output = self.input(step, values, 1)?.logical_values()?;
        let last_dim = step.spec.shape().dims()[0];
        let mut output = vec![0.0_f32; last_dim];
        for (src_row, grad_row) in input
            .chunks_exact(last_dim)
            .zip(grad_output.chunks_exact(last_dim))
        {
            let mean_square =
                src_row.iter().map(|value| value * value).sum::<f32>() / last_dim as f32;
            let inv = (mean_square + epsilon).sqrt().recip();
            for ((accumulated, value), grad) in
                output.iter_mut().zip(src_row.iter()).zip(grad_row.iter())
            {
                *accumulated += grad * value * inv;
            }
        }
        CpuBuffer::from_f32(step.spec.clone(), output)
    }

    fn layer_norm(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
        epsilon: f32,
    ) -> Result<CpuBuffer, RuntimeError> {
        let input = self.input(step, values, 0)?.logical_values()?;
        let weight = self.input(step, values, 1)?.logical_values()?;
        let bias = self.input(step, values, 2)?.logical_values()?;
        let last_dim = weight.len();
        let mut output = vec![0.0; input.len()];
        for (src_row, dst_row) in input
            .chunks_exact(last_dim)
            .zip(output.chunks_exact_mut(last_dim))
        {
            let mean = src_row.iter().sum::<f32>() / last_dim as f32;
            let variance = src_row
                .iter()
                .map(|value| {
                    let centered = *value - mean;
                    centered * centered
                })
                .sum::<f32>()
                / last_dim as f32;
            let inv = (variance + epsilon).sqrt().recip();
            for (((dst, value), scale), bias) in dst_row
                .iter_mut()
                .zip(src_row.iter())
                .zip(weight.iter())
                .zip(bias.iter())
            {
                *dst = (*value - mean) * inv * *scale + *bias;
            }
        }
        CpuBuffer::from_f32(step.spec.clone(), output)
    }

    fn rotary_embedding(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
        interleaved: bool,
    ) -> Result<CpuBuffer, RuntimeError> {
        let input = self.input(step, values, 0)?.logical_values()?;
        let cos_buffer = self.input(step, values, 1)?;
        let sin_buffer = self.input(step, values, 2)?;
        let cos = cos_buffer.logical_values()?;
        let sin = sin_buffer.logical_values()?;
        let dims = step.spec.shape().dims();
        let batch = dims[0];
        let heads = dims[1];
        let seq_len = dims[2];
        let head_dim = dims[3];
        let half_dim = head_dim / 2;
        let cos_dims = cos_buffer.spec().shape().dims();
        let batched_cos = cos_dims.len() == 3;
        let mut output = input.clone();

        for batch_index in 0..batch {
            for head_index in 0..heads {
                for position in 0..seq_len {
                    let base = ((batch_index * heads + head_index) * seq_len + position) * head_dim;
                    for pair in 0..half_dim {
                        let cos_index = if batched_cos {
                            (batch_index * seq_len + position) * half_dim + pair
                        } else {
                            position * half_dim + pair
                        };
                        let cosine = cos[cos_index];
                        let sine = sin[cos_index];
                        let (left_index, right_index) = if interleaved {
                            (base + pair * 2, base + pair * 2 + 1)
                        } else {
                            (base + pair, base + half_dim + pair)
                        };
                        let left = input[left_index];
                        let right = input[right_index];
                        output[left_index] = left * cosine + right * sine;
                        output[right_index] = (-left * sine) + right * cosine;
                    }
                }
            }
        }

        CpuBuffer::from_f32(step.spec.clone(), output)
    }

    fn scaled_dot_product_attention(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
        scale: f32,
        causal: bool,
    ) -> Result<CpuBuffer, RuntimeError> {
        let query = self.input(step, values, 0)?.logical_values()?;
        let key_buffer = self.input(step, values, 1)?;
        let value_buffer = self.input(step, values, 2)?;
        let key = key_buffer.logical_values()?;
        let value = value_buffer.logical_values()?;
        let query_dims = self.input(step, values, 0)?.spec().shape().dims().to_vec();
        let key_dims = key_buffer.spec().shape().dims().to_vec();
        let value_dims = value_buffer.spec().shape().dims().to_vec();
        let batch = query_dims[0];
        let query_heads = query_dims[1];
        let key_heads = key_dims[1];
        let query_seq = query_dims[2];
        let key_seq = key_dims[2];
        let head_dim = query_dims[3];
        let value_dim = value_dims[3];
        let group_size = query_heads / key_heads;
        let mut output = vec![0.0; batch * query_heads * query_seq * value_dim];
        let mut scores = vec![0.0; key_seq];
        let mut weights = vec![0.0; key_seq];

        for batch_index in 0..batch {
            for head_index in 0..query_heads {
                let kv_head = head_index / group_size;
                for query_index in 0..query_seq {
                    let mut max_score = f32::NEG_INFINITY;
                    let mut valid_scores = 0usize;
                    for key_index in 0..key_seq {
                        if causal && key_index > query_index {
                            scores[key_index] = f32::NEG_INFINITY;
                            continue;
                        }
                        let mut dot = 0.0;
                        let query_base = ((batch_index * query_heads + head_index) * query_seq
                            + query_index)
                            * head_dim;
                        let key_base =
                            ((batch_index * key_heads + kv_head) * key_seq + key_index) * head_dim;
                        for dim in 0..head_dim {
                            dot += query[query_base + dim] * key[key_base + dim];
                        }
                        let score = dot * scale;
                        scores[key_index] = score;
                        max_score = max_score.max(score);
                        valid_scores += 1;
                    }

                    if valid_scores == 0 {
                        continue;
                    }

                    let mut weight_sum = 0.0;
                    for key_index in 0..key_seq {
                        if !scores[key_index].is_finite() {
                            weights[key_index] = 0.0;
                            continue;
                        }
                        let weight = (scores[key_index] - max_score).exp();
                        weights[key_index] = weight;
                        weight_sum += weight;
                    }
                    if weight_sum <= 0.0 {
                        continue;
                    }

                    let output_base = ((batch_index * query_heads + head_index) * query_seq
                        + query_index)
                        * value_dim;
                    for key_index in 0..key_seq {
                        let normalized = weights[key_index] / weight_sum;
                        if normalized == 0.0 {
                            continue;
                        }
                        let value_base =
                            ((batch_index * key_heads + kv_head) * key_seq + key_index) * value_dim;
                        for dim in 0..value_dim {
                            output[output_base + dim] += normalized * value[value_base + dim];
                        }
                    }
                }
            }
        }

        CpuBuffer::from_f32(step.spec.clone(), output)
    }

    fn quantized_matmul(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CpuBuffer>,
        rhs_mode: QuantizationMode,
    ) -> Result<CpuBuffer, RuntimeError> {
        if rhs_mode == QuantizationMode::None {
            return Err(RuntimeError::Backend(String::from(
                "quantized_matmul requires a non-dense rhs quantization mode",
            )));
        }
        let left = self.input(step, values, 0)?;
        let right = self.input(step, values, 1)?;
        let left_shape = left.spec().shape().dims();
        let right_shape = right.spec().shape().dims();
        if left_shape.len() != 2 || right_shape.len() != 2 || left_shape[1] != right_shape[1] {
            return Err(RuntimeError::Backend(String::from(
                "invalid quantized_matmul shapes at runtime",
            )));
        }

        let left_values = left.logical_values()?;
        let Some((stored_mode, layout, bytes)) = right.quantized_blocks() else {
            return Err(RuntimeError::Backend(String::from(
                "quantized_matmul requires quantized rhs block storage",
            )));
        };
        if stored_mode != rhs_mode {
            return Err(RuntimeError::Backend(format!(
                "quantized_matmul rhs mode mismatch: requested {rhs_mode:?}, actual {stored_mode:?}",
            )));
        }

        let m = left_shape[0];
        let k = left_shape[1];
        let n = right_shape[0];
        let row_bytes = quantized_row_byte_len(right.spec().shape(), layout)?;
        let mut output = vec![0.0; m * n];

        for row in 0..m {
            let lhs_row = &left_values[row * k..(row + 1) * k];
            for col in 0..n {
                let row_start = col * row_bytes;
                let row_end = row_start + row_bytes;
                output[row * n + col] =
                    quantized_row_dot(lhs_row, rhs_mode, &bytes[row_start..row_end])?;
            }
        }

        CpuBuffer::from_f32(step.spec.clone(), output)
    }
}

impl Default for CpuBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
struct CpuAllocatorPool {
    policy: AllocatorPoolPolicy,
    cached: HashMap<TensorSpec, Vec<CachedDenseCpuBuffer>>,
    state: AllocatorPoolState,
}

#[derive(Clone, Debug)]
struct CachedDenseCpuBuffer {
    identity: BufferStorageIdentity,
    data: Vec<f32>,
}

impl CpuAllocatorPool {
    fn new(policy: AllocatorPoolPolicy) -> Self {
        Self {
            policy,
            cached: HashMap::new(),
            state: AllocatorPoolState::default(),
        }
    }

    fn allocate(&mut self, spec: &TensorSpec) -> CpuBuffer {
        if self.policy.mode == AllocatorPoolMode::ExactTensorSpec {
            if let Some(entries) = self.cached.get_mut(spec) {
                if let Some(mut cached) = entries.pop() {
                    if entries.is_empty() {
                        self.cached.remove(spec);
                    }
                    cached.data.fill(0.0);
                    self.state.cached_buffers = self.state.cached_buffers.saturating_sub(1);
                    self.state.cached_bytes = self
                        .state
                        .cached_bytes
                        .saturating_sub(buffer_bytes_from_len(cached.data.len()));
                    return CpuBuffer {
                        spec: spec.clone(),
                        storage_contract: BufferStorageContract {
                            identity: cached.identity,
                            view_semantics: ViewSemantics::Dense,
                        },
                        storage: CpuBufferStorage::Dense(cached.data),
                    };
                }
            }
        }
        CpuBuffer::zeros(spec)
    }

    fn recycle(&mut self, buffer: CpuBuffer) {
        if self.policy.mode != AllocatorPoolMode::ExactTensorSpec {
            return;
        }
        if buffer.storage_contract.view_semantics != ViewSemantics::Dense {
            return;
        }
        let CpuBufferStorage::Dense(data) = buffer.storage else {
            return;
        };
        let bytes = buffer_bytes_from_len(data.len());
        if self.state.cached_buffers >= self.policy.max_cached_buffers
            || self.state.cached_bytes.saturating_add(bytes) > self.policy.max_cached_bytes
        {
            return;
        }
        self.cached
            .entry(buffer.spec)
            .or_default()
            .push(CachedDenseCpuBuffer {
                identity: buffer.storage_contract.identity,
                data,
            });
        self.state.cached_buffers += 1;
        self.state.cached_bytes = self.state.cached_bytes.saturating_add(bytes);
    }

    fn report(&self) -> AllocatorPoolReport {
        AllocatorPoolReport {
            policy: self.policy.clone(),
            state: self.state.clone(),
        }
    }
}

fn cpu_allocator_pool_policy() -> AllocatorPoolPolicy {
    AllocatorPoolPolicy::exact_tensor_spec(CPU_POOL_MAX_CACHED_BUFFERS, CPU_POOL_MAX_CACHED_BYTES)
}

fn cpu_execution_plan_cache_policy() -> ExecutionPlanCachePolicy {
    ExecutionPlanCachePolicy::bounded(
        CPU_EXECUTION_PLAN_CACHE_MAX_ENTRIES,
        Some(CPU_EXECUTION_PLAN_CACHE_MAX_CACHED_BYTES),
    )
}

fn cpu_kernel_cache_report() -> KernelCacheReport {
    KernelCacheReport {
        policy: KernelCachePolicy::disabled(),
        state: KernelCacheState::default(),
    }
}

#[derive(Clone, Debug)]
struct CachedCpuExecutionPlan {
    plan: ExecutionPlan,
    plan_digest: String,
}

#[derive(Clone, Debug)]
struct CpuExecutionPlanCache {
    policy: ExecutionPlanCachePolicy,
    cached: HashMap<String, CachedCpuExecutionPlan>,
    state: ExecutionPlanCacheState,
}

impl CpuExecutionPlanCache {
    fn new(policy: ExecutionPlanCachePolicy) -> Self {
        Self {
            policy,
            cached: HashMap::new(),
            state: ExecutionPlanCacheState::default(),
        }
    }

    fn report(&self) -> ExecutionPlanCacheReport {
        ExecutionPlanCacheReport {
            policy: self.policy.clone(),
            state: self.state.clone(),
        }
    }

    fn lookup_or_compile(
        &mut self,
        graph: &Graph,
    ) -> Result<(ExecutionPlan, String, CompilePathEvidence), RuntimeError> {
        let cache_key = graph.stable_digest();
        if let Some(cached) = self.cached.get(&cache_key) {
            return Ok((
                cached.plan.clone(),
                cached.plan_digest.clone(),
                CompilePathEvidence {
                    temperature: CompilePathTemperature::WarmReuse,
                    execution_plan_cache: CacheObservation::new(
                        CacheKind::ExecutionPlan,
                        CacheAction::Reuse,
                        format!(
                            "reused cached cpu execution plan for graph {}",
                            graph.stable_digest()
                        ),
                    ),
                    kernel_cache: CacheObservation::new(
                        CacheKind::KernelCache,
                        CacheAction::Bypass,
                        "cpu backend does not retain a kernel cache",
                    ),
                },
            ));
        }

        let plan =
            compile_graph(graph).map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let plan_digest = plan.stable_digest();
        let estimated_bytes = estimate_execution_plan_bytes(&plan, &plan_digest);
        if self.policy.enabled
            && self.cached.len() < self.policy.max_cached_entries
            && self
                .policy
                .max_cached_bytes
                .map(|limit| self.state.cached_bytes.saturating_add(estimated_bytes) <= limit)
                .unwrap_or(true)
        {
            self.cached.insert(
                cache_key,
                CachedCpuExecutionPlan {
                    plan: plan.clone(),
                    plan_digest: plan_digest.clone(),
                },
            );
            self.state.cached_entries = self.cached.len();
            self.state.cached_bytes = self.state.cached_bytes.saturating_add(estimated_bytes);
        }
        Ok((
            plan,
            plan_digest,
            CompilePathEvidence {
                temperature: CompilePathTemperature::ColdCompile,
                execution_plan_cache: CacheObservation::new(
                    CacheKind::ExecutionPlan,
                    CacheAction::Rebuild,
                    format!(
                        "compiled a new cpu execution plan for graph {}",
                        graph.stable_digest()
                    ),
                ),
                kernel_cache: CacheObservation::new(
                    CacheKind::KernelCache,
                    CacheAction::Bypass,
                    "cpu backend does not retain a kernel cache",
                ),
            },
        ))
    }
}

fn estimate_execution_plan_bytes(plan: &ExecutionPlan, plan_digest: &str) -> u64 {
    plan.stable_debug()
        .len()
        .saturating_add(plan_digest.len())
        .try_into()
        .unwrap_or(u64::MAX)
}

fn plan_output_bytes(plan: &ExecutionPlan) -> u64 {
    plan.steps
        .iter()
        .map(|step| {
            step.spec
                .storage_size()
                .saturating_mul(step.spec.dtype().element_size_bytes())
                .try_into()
                .unwrap_or(u64::MAX)
        })
        .sum()
}

fn buffer_bytes_from_len(len: usize) -> u64 {
    len.saturating_mul(std::mem::size_of::<f32>())
        .try_into()
        .unwrap_or(u64::MAX)
}

/// Returns the byte length of one logical row for GGML/GGUF block storage.
pub fn quantized_row_byte_len(
    shape: &Shape,
    layout: psionic_core::QuantizedBlockLayout,
) -> Result<usize, RuntimeError> {
    let dims = shape.dims();
    let Some(&row_width) = dims.last() else {
        return Err(RuntimeError::Backend(String::from(
            "quantized storage requires a non-scalar shape",
        )));
    };
    if row_width == 0 || row_width % layout.elements_per_block != 0 {
        return Err(RuntimeError::Backend(format!(
            "quantized row width {row_width} is not aligned to {}",
            layout.elements_per_block
        )));
    }
    Ok((row_width / layout.elements_per_block) * layout.bytes_per_block)
}

/// Computes one dense-by-quantized row dot product without dequantizing the full row.
pub fn quantized_row_dot(
    lhs: &[f32],
    mode: QuantizationMode,
    bytes: &[u8],
) -> Result<f32, RuntimeError> {
    let Some((elements_per_block, bytes_per_block)) = mode.ggml_block_spec() else {
        return Err(RuntimeError::Backend(format!(
            "quantized mode {mode:?} does not use GGML blocks"
        )));
    };
    if !lhs.len().is_multiple_of(elements_per_block) {
        return Err(RuntimeError::Backend(format!(
            "lhs row width {} is not divisible by {elements_per_block}",
            lhs.len()
        )));
    }
    if bytes.len() != (lhs.len() / elements_per_block) * bytes_per_block {
        return Err(RuntimeError::Backend(format!(
            "rhs row byte length mismatch: expected {}, actual {}",
            (lhs.len() / elements_per_block) * bytes_per_block,
            bytes.len()
        )));
    }

    let mut sum = 0.0;
    for (block_index, block_bytes) in bytes.chunks_exact(bytes_per_block).enumerate() {
        let lhs_block_start = block_index * elements_per_block;
        let lhs_block = &lhs[lhs_block_start..lhs_block_start + elements_per_block];
        sum += match mode {
            QuantizationMode::GgmlMxfp4 => dot_mxfp4_block(lhs_block, block_bytes)?,
            QuantizationMode::GgmlQ4_0 => dot_q4_0_block(lhs_block, block_bytes)?,
            QuantizationMode::GgmlQ4_1 => dot_q4_1_block(lhs_block, block_bytes)?,
            QuantizationMode::GgmlQ5_0 => dot_q5_0_block(lhs_block, block_bytes)?,
            QuantizationMode::GgmlQ3K => {
                dot_shared_super_block(lhs_block, block_bytes, decode_q3_k_block)?
            }
            QuantizationMode::GgmlQ5K => dot_q5_k_block(lhs_block, block_bytes)?,
            QuantizationMode::GgmlQ4K => dot_q4_k_block(lhs_block, block_bytes)?,
            QuantizationMode::GgmlQ6K => dot_q6_k_block(lhs_block, block_bytes)?,
            QuantizationMode::GgmlQ8_0 => dot_q8_0_block(lhs_block, block_bytes)?,
            QuantizationMode::GgmlIq3S => {
                dot_shared_super_block(lhs_block, block_bytes, decode_iq3_s_block)?
            }
            QuantizationMode::GgmlIq4Xs => {
                dot_shared_super_block(lhs_block, block_bytes, decode_iq4_xs_block)?
            }
            QuantizationMode::None | QuantizationMode::Int8Symmetric => {
                return Err(RuntimeError::Backend(format!(
                    "unsupported quantized matmul mode {mode:?}",
                )));
            }
        };
    }
    Ok(sum)
}

fn decode_quantized_values(
    shape: &Shape,
    mode: QuantizationMode,
    layout: psionic_core::QuantizedBlockLayout,
    bytes: &[u8],
) -> Result<Vec<f32>, RuntimeError> {
    let dims = shape.dims();
    let Some(&row_width) = dims.last() else {
        return Err(RuntimeError::Backend(String::from(
            "quantized storage requires a non-scalar shape",
        )));
    };
    let row_count = if dims.len() == 1 {
        1
    } else {
        dims[..dims.len() - 1].iter().product()
    };
    let row_bytes = quantized_row_byte_len(shape, layout)?;
    if bytes.len() != row_count * row_bytes {
        return Err(RuntimeError::Backend(format!(
            "quantized tensor byte length mismatch: expected {}, actual {}",
            row_count * row_bytes,
            bytes.len()
        )));
    }

    let mut output = Vec::with_capacity(shape.element_count());
    for row_bytes in bytes.chunks_exact(row_bytes) {
        decode_quantized_row_into(mode, row_bytes, &mut output)?;
    }
    if output.len() != shape.element_count() {
        return Err(RuntimeError::Backend(format!(
            "quantized tensor decode length mismatch: expected {}, actual {}",
            shape.element_count(),
            output.len()
        )));
    }
    if row_width == 0 {
        return Err(RuntimeError::Backend(String::from(
            "quantized row width must be non-zero",
        )));
    }
    Ok(output)
}

/// Decodes one quantized GGML/GGUF row into `output`.
pub fn decode_quantized_row_into(
    mode: QuantizationMode,
    bytes: &[u8],
    output: &mut Vec<f32>,
) -> Result<(), RuntimeError> {
    let Some((_, bytes_per_block)) = mode.ggml_block_spec() else {
        return Err(RuntimeError::Backend(format!(
            "quantized mode {mode:?} does not use GGML blocks"
        )));
    };
    for block_bytes in bytes.chunks_exact(bytes_per_block) {
        match mode {
            QuantizationMode::GgmlMxfp4 => decode_mxfp4_block_into(block_bytes, output)?,
            QuantizationMode::GgmlQ4_0 => decode_q4_0_block_into(block_bytes, output)?,
            QuantizationMode::GgmlQ4_1 => decode_q4_1_block_into(block_bytes, output)?,
            QuantizationMode::GgmlQ5_0 => decode_q5_0_block_into(block_bytes, output)?,
            QuantizationMode::GgmlQ3K => {
                decode_shared_super_block_into(block_bytes, output, decode_q3_k_block)?
            }
            QuantizationMode::GgmlQ5K => decode_q5_k_block_into(block_bytes, output)?,
            QuantizationMode::GgmlQ4K => decode_q4_k_block_into(block_bytes, output)?,
            QuantizationMode::GgmlQ6K => decode_q6_k_block_into(block_bytes, output)?,
            QuantizationMode::GgmlQ8_0 => decode_q8_0_block_into(block_bytes, output)?,
            QuantizationMode::GgmlIq3S => {
                decode_shared_super_block_into(block_bytes, output, decode_iq3_s_block)?
            }
            QuantizationMode::GgmlIq4Xs => {
                decode_shared_super_block_into(block_bytes, output, decode_iq4_xs_block)?
            }
            QuantizationMode::None | QuantizationMode::Int8Symmetric => {
                return Err(RuntimeError::Backend(format!(
                    "unsupported quantized decode mode {mode:?}",
                )));
            }
        }
    }
    Ok(())
}

fn dot_shared_super_block(
    lhs: &[f32],
    bytes: &[u8],
    decode: fn(&[u8]) -> Result<[f32; 256], GgmlBlockDecodeError>,
) -> Result<f32, RuntimeError> {
    if lhs.len() != 256 {
        return Err(RuntimeError::Backend(format!(
            "GGML super-block dot requires 256 lhs values, found {}",
            lhs.len()
        )));
    }
    let values = decode(bytes).map_err(|error| RuntimeError::Backend(error.to_string()))?;
    Ok(lhs.iter().zip(values).map(|(lhs, rhs)| lhs * rhs).sum())
}

fn decode_shared_super_block_into(
    bytes: &[u8],
    output: &mut Vec<f32>,
    decode: fn(&[u8]) -> Result<[f32; 256], GgmlBlockDecodeError>,
) -> Result<(), RuntimeError> {
    let values = decode(bytes).map_err(|error| RuntimeError::Backend(error.to_string()))?;
    output.extend(values);
    Ok(())
}

fn dot_mxfp4_block(lhs: &[f32], bytes: &[u8]) -> Result<f32, RuntimeError> {
    const KVALUES: [i8; 16] = [0, 1, 2, 3, 4, 6, 8, 12, 0, -1, -2, -3, -4, -6, -8, -12];

    if bytes.len() != 17 || lhs.len() != 32 {
        return Err(RuntimeError::Backend(String::from(
            "mxfp4 block dot requires 32 lhs values and 17 bytes",
        )));
    }
    let scale = decode_e8m0_to_fp32_half(bytes[0]) * 0.5;
    let quants = &bytes[1..];
    let mut sum = 0.0;
    for (pair_index, quant) in quants.iter().copied().enumerate() {
        let low = f32::from(KVALUES[usize::from(quant & 0x0f)]) * scale;
        let high = f32::from(KVALUES[usize::from((quant >> 4) & 0x0f)]) * scale;
        sum += lhs[pair_index] * low;
        sum += lhs[pair_index + 16] * high;
    }
    Ok(sum)
}

fn dot_q4_0_block(lhs: &[f32], bytes: &[u8]) -> Result<f32, RuntimeError> {
    if bytes.len() != 18 || lhs.len() != 32 {
        return Err(RuntimeError::Backend(String::from(
            "q4_0 block dot requires 32 lhs values and 18 bytes",
        )));
    }
    let scale = decode_f16_le(bytes[0], bytes[1]);
    let quants = &bytes[2..];
    let mut sum = 0.0;
    for (pair_index, quant) in quants.iter().copied().enumerate() {
        let low = f32::from((quant & 0x0f) as i8 - 8);
        let high = f32::from((quant >> 4) as i8 - 8);
        sum += lhs[pair_index] * (low * scale);
        sum += lhs[pair_index + 16] * (high * scale);
    }
    Ok(sum)
}

fn dot_q4_1_block(lhs: &[f32], bytes: &[u8]) -> Result<f32, RuntimeError> {
    if bytes.len() != 20 || lhs.len() != 32 {
        return Err(RuntimeError::Backend(String::from(
            "q4_1 block dot requires 32 lhs values and 20 bytes",
        )));
    }
    let scale = decode_f16_le(bytes[0], bytes[1]);
    let min = decode_f16_le(bytes[2], bytes[3]);
    let quants = &bytes[4..];
    let mut sum = 0.0;
    for (pair_index, quant) in quants.iter().copied().enumerate() {
        let low = min + f32::from(quant & 0x0f) * scale;
        let high = min + f32::from(quant >> 4) * scale;
        sum += lhs[pair_index] * low;
        sum += lhs[pair_index + 16] * high;
    }
    Ok(sum)
}

fn dot_q5_0_block(lhs: &[f32], bytes: &[u8]) -> Result<f32, RuntimeError> {
    if bytes.len() != 22 || lhs.len() != 32 {
        return Err(RuntimeError::Backend(String::from(
            "q5_0 block dot requires 32 lhs values and 22 bytes",
        )));
    }
    let scale = decode_f16_le(bytes[0], bytes[1]);
    let qh = u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
    let quants = &bytes[6..22];
    let mut sum = 0.0;
    for (pair_index, quant) in quants.iter().copied().enumerate() {
        let high_bit_low = (((qh >> pair_index) << 4) & 0x10) as u8;
        let high_bit_high = ((qh >> (pair_index + 12)) & 0x10) as u8;
        let low = (((quant & 0x0f) | high_bit_low) as i32 - 16) as f32;
        let high = (((quant >> 4) | high_bit_high) as i32 - 16) as f32;
        sum += lhs[pair_index] * (low * scale);
        sum += lhs[pair_index + 16] * (high * scale);
    }
    Ok(sum)
}

fn dot_q4_k_block(lhs: &[f32], bytes: &[u8]) -> Result<f32, RuntimeError> {
    if bytes.len() != 144 || lhs.len() != 256 {
        return Err(RuntimeError::Backend(String::from(
            "q4_k block dot requires 256 lhs values and 144 bytes",
        )));
    }
    let scale = decode_f16_le(bytes[0], bytes[1]);
    let minimum = decode_f16_le(bytes[2], bytes[3]);
    let scales = &bytes[4..16];
    let quants = &bytes[16..144];
    let mut sum = 0.0;
    let mut scale_index = 0usize;
    let mut lhs_offset = 0usize;
    for quant_chunk in quants.chunks_exact(32) {
        let (low_scale, low_min) = decode_q4_k_scale_min(scale_index, scales);
        let low_scale = scale * f32::from(low_scale);
        let low_min = minimum * f32::from(low_min);
        for (lhs_value, quant) in lhs[lhs_offset..lhs_offset + 32]
            .iter()
            .copied()
            .zip(quant_chunk.iter().copied())
        {
            sum += lhs_value * (low_scale * f32::from(quant & 0x0f) - low_min);
        }
        lhs_offset = lhs_offset.saturating_add(32);
        scale_index = scale_index.saturating_add(1);

        let (high_scale, high_min) = decode_q4_k_scale_min(scale_index, scales);
        let high_scale = scale * f32::from(high_scale);
        let high_min = minimum * f32::from(high_min);
        for (lhs_value, quant) in lhs[lhs_offset..lhs_offset + 32]
            .iter()
            .copied()
            .zip(quant_chunk.iter().copied())
        {
            sum += lhs_value * (high_scale * f32::from((quant >> 4) & 0x0f) - high_min);
        }
        lhs_offset = lhs_offset.saturating_add(32);
        scale_index = scale_index.saturating_add(1);
    }
    Ok(sum)
}

fn dot_q5_k_block(lhs: &[f32], bytes: &[u8]) -> Result<f32, RuntimeError> {
    if bytes.len() != 176 || lhs.len() != 256 {
        return Err(RuntimeError::Backend(String::from(
            "q5_k block dot requires 256 lhs values and 176 bytes",
        )));
    }
    let scale = decode_f16_le(bytes[0], bytes[1]);
    let minimum = decode_f16_le(bytes[2], bytes[3]);
    let scales = &bytes[4..16];
    let high_bits = &bytes[16..48];
    let quants = &bytes[48..176];
    let mut sum = 0.0;
    let mut scale_index = 0usize;
    let mut lhs_offset = 0usize;
    let mut low_mask = 1u8;
    let mut high_mask = 2u8;
    for quant_chunk in quants.chunks_exact(32) {
        let (low_scale_value, low_min_value) = decode_q4_k_scale_min(scale_index, scales);
        let low_scale = scale * f32::from(low_scale_value);
        let low_min = minimum * f32::from(low_min_value);
        for ((lhs_value, quant), high_bits) in lhs[lhs_offset..lhs_offset + 32]
            .iter()
            .copied()
            .zip(quant_chunk.iter().copied())
            .zip(high_bits.iter().copied())
        {
            let lifted = if (high_bits & low_mask) != 0 {
                16u8
            } else {
                0u8
            };
            sum += lhs_value * (low_scale * f32::from((quant & 0x0f) + lifted) - low_min);
        }
        lhs_offset = lhs_offset.saturating_add(32);
        scale_index = scale_index.saturating_add(1);

        let (high_scale_value, high_min_value) = decode_q4_k_scale_min(scale_index, scales);
        let high_scale = scale * f32::from(high_scale_value);
        let high_min = minimum * f32::from(high_min_value);
        for ((lhs_value, quant), high_bits) in lhs[lhs_offset..lhs_offset + 32]
            .iter()
            .copied()
            .zip(quant_chunk.iter().copied())
            .zip(high_bits.iter().copied())
        {
            let lifted = if (high_bits & high_mask) != 0 {
                16u8
            } else {
                0u8
            };
            sum += lhs_value * (high_scale * f32::from(((quant >> 4) & 0x0f) + lifted) - high_min);
        }
        lhs_offset = lhs_offset.saturating_add(32);
        scale_index = scale_index.saturating_add(1);
        low_mask <<= 2;
        high_mask <<= 2;
    }
    Ok(sum)
}

fn dot_q6_k_block(lhs: &[f32], bytes: &[u8]) -> Result<f32, RuntimeError> {
    if bytes.len() != 210 || lhs.len() != 256 {
        return Err(RuntimeError::Backend(String::from(
            "q6_k block dot requires 256 lhs values and 210 bytes",
        )));
    }
    let ql = &bytes[0..128];
    let qh = &bytes[128..192];
    let scales = &bytes[192..208];
    let scale = decode_f16_le(bytes[208], bytes[209]);
    let mut sum = 0.0;
    for chunk_index in 0..2 {
        let ql_chunk = &ql[chunk_index * 64..(chunk_index + 1) * 64];
        let qh_chunk = &qh[chunk_index * 32..(chunk_index + 1) * 32];
        let scale_chunk = &scales[chunk_index * 8..(chunk_index + 1) * 8];
        let lhs_chunk = &lhs[chunk_index * 128..(chunk_index + 1) * 128];
        for l in 0..32 {
            let is = l / 16;
            let q1 = (((ql_chunk[l] & 0x0f) | (((qh_chunk[l] >> 0) & 0x03) << 4)) as i8) - 32;
            let q2 = (((ql_chunk[l + 32] & 0x0f) | (((qh_chunk[l] >> 2) & 0x03) << 4)) as i8) - 32;
            let q3 = (((ql_chunk[l] >> 4) | (((qh_chunk[l] >> 4) & 0x03) << 4)) as i8) - 32;
            let q4 = (((ql_chunk[l + 32] >> 4) | (((qh_chunk[l] >> 6) & 0x03) << 4)) as i8) - 32;
            sum += lhs_chunk[l] * (scale * f32::from(scale_chunk[is] as i8) * f32::from(q1));
            sum +=
                lhs_chunk[l + 32] * (scale * f32::from(scale_chunk[is + 2] as i8) * f32::from(q2));
            sum +=
                lhs_chunk[l + 64] * (scale * f32::from(scale_chunk[is + 4] as i8) * f32::from(q3));
            sum +=
                lhs_chunk[l + 96] * (scale * f32::from(scale_chunk[is + 6] as i8) * f32::from(q4));
        }
    }
    Ok(sum)
}

fn dot_q8_0_block(lhs: &[f32], bytes: &[u8]) -> Result<f32, RuntimeError> {
    if bytes.len() != 34 || lhs.len() != 32 {
        return Err(RuntimeError::Backend(String::from(
            "q8_0 block dot requires 32 lhs values and 34 bytes",
        )));
    }
    let scale = decode_f16_le(bytes[0], bytes[1]);
    let mut sum = 0.0;
    for (lhs, quant) in lhs.iter().zip(bytes[2..].iter().copied()) {
        let quant = i8::from_le_bytes([quant]);
        sum += lhs * (f32::from(quant) * scale);
    }
    Ok(sum)
}

fn decode_mxfp4_block_into(bytes: &[u8], output: &mut Vec<f32>) -> Result<(), RuntimeError> {
    const KVALUES: [i8; 16] = [0, 1, 2, 3, 4, 6, 8, 12, 0, -1, -2, -3, -4, -6, -8, -12];

    if bytes.len() != 17 {
        return Err(RuntimeError::Backend(String::from(
            "mxfp4 block decode requires 17 bytes",
        )));
    }
    let scale = decode_e8m0_to_fp32_half(bytes[0]) * 0.5;
    let quants = &bytes[1..];
    let start = output.len();
    output.resize(start + 32, 0.0);
    for (pair_index, quant) in quants.iter().copied().enumerate() {
        output[start + pair_index] = f32::from(KVALUES[usize::from(quant & 0x0f)]) * scale;
        output[start + pair_index + 16] =
            f32::from(KVALUES[usize::from((quant >> 4) & 0x0f)]) * scale;
    }
    Ok(())
}

fn decode_q4_0_block_into(bytes: &[u8], output: &mut Vec<f32>) -> Result<(), RuntimeError> {
    if bytes.len() != 18 {
        return Err(RuntimeError::Backend(String::from(
            "q4_0 block decode requires 18 bytes",
        )));
    }
    let scale = decode_f16_le(bytes[0], bytes[1]);
    let quants = &bytes[2..];
    let start = output.len();
    output.resize(start + 32, 0.0);
    for (pair_index, quant) in quants.iter().copied().enumerate() {
        output[start + pair_index] = f32::from((quant & 0x0f) as i8 - 8) * scale;
        output[start + pair_index + 16] = f32::from((quant >> 4) as i8 - 8) * scale;
    }
    Ok(())
}

fn decode_q4_1_block_into(bytes: &[u8], output: &mut Vec<f32>) -> Result<(), RuntimeError> {
    if bytes.len() != 20 {
        return Err(RuntimeError::Backend(String::from(
            "q4_1 block decode requires 20 bytes",
        )));
    }
    let scale = decode_f16_le(bytes[0], bytes[1]);
    let min = decode_f16_le(bytes[2], bytes[3]);
    let quants = &bytes[4..];
    let start = output.len();
    output.resize(start + 32, 0.0);
    for (pair_index, quant) in quants.iter().copied().enumerate() {
        output[start + pair_index] = min + f32::from(quant & 0x0f) * scale;
        output[start + pair_index + 16] = min + f32::from(quant >> 4) * scale;
    }
    Ok(())
}

fn decode_q5_0_block_into(bytes: &[u8], output: &mut Vec<f32>) -> Result<(), RuntimeError> {
    if bytes.len() != 22 {
        return Err(RuntimeError::Backend(String::from(
            "q5_0 block decode requires 22 bytes",
        )));
    }
    let scale = decode_f16_le(bytes[0], bytes[1]);
    let qh = u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
    let start = output.len();
    output.resize(start + 32, 0.0);
    for (pair_index, quant) in bytes[6..22].iter().copied().enumerate() {
        let high_bit_low = (((qh >> pair_index) << 4) & 0x10) as u8;
        let high_bit_high = ((qh >> (pair_index + 12)) & 0x10) as u8;
        let low = i32::from((quant & 0x0f) | high_bit_low) - 16;
        let high = i32::from((quant >> 4) | high_bit_high) - 16;
        output[start + pair_index] = (low as f32) * scale;
        output[start + pair_index + 16] = (high as f32) * scale;
    }
    Ok(())
}

fn decode_q4_k_block_into(bytes: &[u8], output: &mut Vec<f32>) -> Result<(), RuntimeError> {
    if bytes.len() != 144 {
        return Err(RuntimeError::Backend(String::from(
            "q4_k block decode requires 144 bytes",
        )));
    }
    let scale = decode_f16_le(bytes[0], bytes[1]);
    let minimum = decode_f16_le(bytes[2], bytes[3]);
    let scales = &bytes[4..16];
    let quants = &bytes[16..144];
    let mut scale_index = 0usize;
    for quant_chunk in quants.chunks_exact(32) {
        let (low_scale, low_min) = decode_q4_k_scale_min(scale_index, scales);
        let low_scale = scale * f32::from(low_scale);
        let low_min = minimum * f32::from(low_min);
        for quant in quant_chunk {
            output.push(low_scale * f32::from(quant & 0x0f) - low_min);
        }
        scale_index = scale_index.saturating_add(1);

        let (high_scale, high_min) = decode_q4_k_scale_min(scale_index, scales);
        let high_scale = scale * f32::from(high_scale);
        let high_min = minimum * f32::from(high_min);
        for quant in quant_chunk {
            output.push(high_scale * f32::from((quant >> 4) & 0x0f) - high_min);
        }
        scale_index = scale_index.saturating_add(1);
    }
    Ok(())
}

fn decode_q5_k_block_into(bytes: &[u8], output: &mut Vec<f32>) -> Result<(), RuntimeError> {
    if bytes.len() != 176 {
        return Err(RuntimeError::Backend(String::from(
            "q5_k block decode requires 176 bytes",
        )));
    }
    let scale = decode_f16_le(bytes[0], bytes[1]);
    let minimum = decode_f16_le(bytes[2], bytes[3]);
    let scales = &bytes[4..16];
    let high_bits = &bytes[16..48];
    let quants = &bytes[48..176];
    let mut scale_index = 0usize;
    let mut low_mask = 1u8;
    let mut high_mask = 2u8;
    for quant_chunk in quants.chunks_exact(32) {
        let (low_scale_value, low_min_value) = decode_q4_k_scale_min(scale_index, scales);
        let low_scale = scale * f32::from(low_scale_value);
        let low_min = minimum * f32::from(low_min_value);
        for (quant, high_bits) in quant_chunk.iter().copied().zip(high_bits.iter().copied()) {
            let lifted = if (high_bits & low_mask) != 0 {
                16u8
            } else {
                0u8
            };
            output.push(low_scale * f32::from((quant & 0x0f) + lifted) - low_min);
        }
        scale_index = scale_index.saturating_add(1);

        let (high_scale_value, high_min_value) = decode_q4_k_scale_min(scale_index, scales);
        let high_scale = scale * f32::from(high_scale_value);
        let high_min = minimum * f32::from(high_min_value);
        for (quant, high_bits) in quant_chunk.iter().copied().zip(high_bits.iter().copied()) {
            let lifted = if (high_bits & high_mask) != 0 {
                16u8
            } else {
                0u8
            };
            output.push(high_scale * f32::from(((quant >> 4) & 0x0f) + lifted) - high_min);
        }
        scale_index = scale_index.saturating_add(1);
        low_mask <<= 2;
        high_mask <<= 2;
    }
    Ok(())
}

fn decode_q6_k_block_into(bytes: &[u8], output: &mut Vec<f32>) -> Result<(), RuntimeError> {
    if bytes.len() != 210 {
        return Err(RuntimeError::Backend(String::from(
            "q6_k block decode requires 210 bytes",
        )));
    }
    let ql = &bytes[0..128];
    let qh = &bytes[128..192];
    let scales = &bytes[192..208];
    let scale = decode_f16_le(bytes[208], bytes[209]);
    let start = output.len();
    output.resize(start + 256, 0.0);
    for chunk_index in 0..2 {
        let ql_chunk = &ql[chunk_index * 64..(chunk_index + 1) * 64];
        let qh_chunk = &qh[chunk_index * 32..(chunk_index + 1) * 32];
        let scale_chunk = &scales[chunk_index * 8..(chunk_index + 1) * 8];
        let chunk_start = start + chunk_index * 128;
        for l in 0..32 {
            let is = l / 16;
            let q1 = (((ql_chunk[l] & 0x0f) | (((qh_chunk[l] >> 0) & 0x03) << 4)) as i8) - 32;
            let q2 = (((ql_chunk[l + 32] & 0x0f) | (((qh_chunk[l] >> 2) & 0x03) << 4)) as i8) - 32;
            let q3 = (((ql_chunk[l] >> 4) | (((qh_chunk[l] >> 4) & 0x03) << 4)) as i8) - 32;
            let q4 = (((ql_chunk[l + 32] >> 4) | (((qh_chunk[l] >> 6) & 0x03) << 4)) as i8) - 32;
            output[chunk_start + l] = scale * f32::from(scale_chunk[is] as i8) * f32::from(q1);
            output[chunk_start + l + 32] =
                scale * f32::from(scale_chunk[is + 2] as i8) * f32::from(q2);
            output[chunk_start + l + 64] =
                scale * f32::from(scale_chunk[is + 4] as i8) * f32::from(q3);
            output[chunk_start + l + 96] =
                scale * f32::from(scale_chunk[is + 6] as i8) * f32::from(q4);
        }
    }
    Ok(())
}

fn decode_q4_k_scale_min(index: usize, packed: &[u8]) -> (u8, u8) {
    if index < 4 {
        (packed[index] & 63, packed[index + 4] & 63)
    } else {
        (
            (packed[index + 4] & 0x0f) | ((packed[index - 4] >> 6) << 4),
            (packed[index + 4] >> 4) | ((packed[index] >> 6) << 4),
        )
    }
}

fn decode_q8_0_block_into(bytes: &[u8], output: &mut Vec<f32>) -> Result<(), RuntimeError> {
    if bytes.len() != 34 {
        return Err(RuntimeError::Backend(String::from(
            "q8_0 block decode requires 34 bytes",
        )));
    }
    let scale = decode_f16_le(bytes[0], bytes[1]);
    output.extend(
        bytes[2..]
            .iter()
            .copied()
            .map(|quant| f32::from(i8::from_le_bytes([quant])) * scale),
    );
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

    let value = match exponent {
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
                sign | (((exponent + 127) as u32) << 23) | (mantissa << 13)
            }
        }
        0x1f => sign | 0x7f80_0000 | (u32::from(mantissa) << 13),
        _ => sign | ((u32::from(exponent) + 112) << 23) | (u32::from(mantissa) << 13),
    };
    f32::from_bits(value)
}

impl DeviceDiscovery for CpuBackend {
    fn backend_name(&self) -> BackendName {
        "cpu"
    }

    fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError> {
        Ok(vec![DeviceDescriptor {
            backend: String::from(self.backend_name()),
            device: Device::cpu(),
            device_name: Some(String::from("host cpu")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: vec![
                QuantizationSupport {
                    mode: psionic_core::QuantizationMode::None,
                    load_path: QuantizationLoadPath::DenseF32,
                    execution: QuantizationExecution::Native,
                },
                QuantizationSupport {
                    mode: psionic_core::QuantizationMode::Int8Symmetric,
                    load_path: QuantizationLoadPath::DequantizedF32,
                    execution: QuantizationExecution::DequantizeToF32,
                },
                QuantizationSupport {
                    mode: psionic_core::QuantizationMode::GgmlQ4_0,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::Native,
                },
                QuantizationSupport {
                    mode: psionic_core::QuantizationMode::GgmlQ4_1,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::Native,
                },
                QuantizationSupport {
                    mode: psionic_core::QuantizationMode::GgmlQ8_0,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::Native,
                },
            ],
            memory_capacity_bytes: None,
            unified_memory: Some(true),
            feature_flags: vec![String::from("host_memory")],
            amd_metadata: None,
            nvidia_metadata: None,
        }])
    }

    fn health(&self) -> RuntimeHealth {
        RuntimeHealth {
            status: HealthStatus::Ready,
            message: String::from("cpu backend ready"),
        }
    }

    fn runtime_resources(&self) -> Option<BackendRuntimeResources> {
        Some(BackendRuntimeResources {
            execution_plan_cache: self.execution_plan_cache.report(),
            allocator_pool: self.pool.report(),
            kernel_cache: cpu_kernel_cache_report(),
            device_memory_budget: None,
        })
    }

    fn extension_support(&self) -> Vec<BackendExtensionSupport> {
        vec![
            BackendExtensionSupport::reference(BackendExtensionKind::ReluSquared),
            BackendExtensionSupport::reference(BackendExtensionKind::LeakyReluSquared),
            BackendExtensionSupport::reference(BackendExtensionKind::Silu),
            BackendExtensionSupport::reference(BackendExtensionKind::RmsNorm),
            BackendExtensionSupport::reference(BackendExtensionKind::LayerNorm),
            BackendExtensionSupport::reference(BackendExtensionKind::RotaryEmbedding),
            BackendExtensionSupport::reference(BackendExtensionKind::ScaledDotProductAttention),
            BackendExtensionSupport::reference(BackendExtensionKind::QuantizedMatmul),
        ]
    }
}

impl Allocator for CpuBackend {
    type Buffer = CpuBuffer;

    fn allocate(&mut self, spec: &TensorSpec) -> Result<Self::Buffer, RuntimeError> {
        Ok(self.pool.allocate(spec))
    }
}

impl ExecutionBackend for CpuBackend {
    type Buffer = CpuBuffer;

    fn execute(
        &mut self,
        plan: &ExecutionPlan,
        inputs: &BTreeMap<TensorId, Self::Buffer>,
    ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError> {
        let mut values = inputs.clone();
        for step in &plan.steps {
            self.materialize_step(step, &mut values)?;
        }

        let mut outputs = BTreeMap::new();
        let output_ids = plan
            .outputs
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        for output in &plan.outputs {
            let Some(buffer) = values.remove(output) else {
                return Err(RuntimeError::MissingInput(*output));
            };
            outputs.insert(*output, buffer);
        }
        for (tensor_id, buffer) in values {
            if !output_ids.contains(&tensor_id) {
                self.pool.recycle(buffer);
            }
        }

        Ok(ExecutionResult {
            outputs,
            metrics: ExecutionMetrics {
                steps_executed: plan.steps.len(),
                kernel_count: plan.steps.len(),
                bytes_moved: plan_output_bytes(plan),
                plan_cache_hits: 0,
                plan_cache_misses: 0,
                execution_plan_digest: None,
                compile_path: None,
            },
        })
    }
}

fn reduce_sum_axis(logical: &[f32], dims: &[usize], axis: usize) -> Result<Vec<f32>, RuntimeError> {
    if axis >= dims.len() {
        return Err(RuntimeError::Backend(format!(
            "reduce axis {axis} out of range for rank {}",
            dims.len()
        )));
    }

    let outer = product(&dims[..axis]);
    let axis_len = dims[axis];
    let inner = product(&dims[axis + 1..]);
    let mut output = vec![0.0; outer * inner];
    for outer_index in 0..outer {
        for inner_index in 0..inner {
            let mut sum = 0.0;
            for axis_index in 0..axis_len {
                let source_index = ((outer_index * axis_len) + axis_index) * inner + inner_index;
                sum += logical[source_index];
            }
            output[(outer_index * inner) + inner_index] = sum;
        }
    }
    Ok(output)
}

fn product(dims: &[usize]) -> usize {
    if dims.is_empty() {
        1
    } else {
        dims.iter().product()
    }
}

fn for_each_index(shape: &Shape, mut f: impl FnMut(&[usize])) {
    if shape.rank() == 0 {
        f(&[]);
        return;
    }

    let dims = shape.dims();
    let mut index = vec![0; dims.len()];
    loop {
        f(&index);

        let mut axis = dims.len();
        while axis > 0 {
            axis -= 1;
            index[axis] += 1;
            if index[axis] < dims[axis] {
                break;
            }
            index[axis] = 0;
            if axis == 0 {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic_in_result_fn)]

    use std::collections::BTreeMap;

    use psionic_backend_tests::{GraphBackendConformanceHarness, run_graph_backend_conformance};
    use psionic_core::{
        BackendExtensionKind, DType, Device, QuantizationMode, Shape, TensorSpec, ViewSemantics,
    };
    use psionic_ir::{Graph, GraphBuilder};
    use psionic_runtime::{
        Allocator, AllocatorPoolMode, BackendSelectionState, BufferHandle, DeviceDiscovery,
        ExecutionResult, HealthStatus, RuntimeError, ServedProductBackendPolicy,
    };

    use super::{CpuAllocatorPool, CpuBackend, CpuBuffer, cpu_allocator_pool_policy};

    impl GraphBackendConformanceHarness for CpuBackend {
        type Buffer = CpuBuffer;

        fn backend_selection(
            &self,
            supported_ops: &[&str],
        ) -> Result<psionic_runtime::BackendSelection, RuntimeError> {
            CpuBackend::backend_selection(self, supported_ops)
        }

        fn input_buffer(
            &mut self,
            shape: Shape,
            values: Vec<f32>,
        ) -> Result<Self::Buffer, RuntimeError> {
            CpuBackend::input_buffer(self, shape, values)
        }

        fn compile_and_execute(
            &mut self,
            graph: &Graph,
            inputs: &BTreeMap<psionic_core::TensorId, Self::Buffer>,
        ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError> {
            CpuBackend::compile_and_execute(self, graph, inputs)
        }

        fn dense_values(&self, buffer: &Self::Buffer) -> Result<Vec<f32>, RuntimeError> {
            buffer
                .as_f32_slice()
                .map(ToOwned::to_owned)
                .ok_or_else(|| RuntimeError::Backend(String::from("expected dense f32 cpu buffer")))
        }

        fn known_unsupported_case(&mut self) -> Result<String, RuntimeError> {
            let mut builder = GraphBuilder::new(Device::cpu());
            let logits = builder.input("logits", Shape::new(vec![1, 2, 4]), DType::F32);
            let target_ids = builder.input("target_ids", Shape::new(vec![1, 2]), DType::I32);
            let loss = builder
                .parameter_golf_projection_loss(&logits, &target_ids, 30.0)
                .map_err(|error| RuntimeError::Backend(error.to_string()))?;
            let graph = builder.finish(vec![loss]);
            let inputs = BTreeMap::from([
                (
                    logits.id(),
                    self.input_buffer(
                        Shape::new(vec![1, 2, 4]),
                        vec![0.1_f32, -0.2, 0.3, 0.7, -0.4, 0.5, -0.1, 0.2],
                    )?,
                ),
                (
                    target_ids.id(),
                    CpuBuffer::from_f32(
                        TensorSpec::new(Shape::new(vec![1, 2]), DType::I32, Device::cpu()),
                        vec![3.0_f32, 1.0],
                    )?,
                ),
            ]);
            match self.compile_and_execute(&graph, &inputs) {
                Err(RuntimeError::UnsupportedStep(step))
                    if step == "parameter_golf_projection_loss" =>
                {
                    Ok(String::from(
                        "parameter_golf_projection_loss remained an explicit unsupported-step refusal",
                    ))
                }
                Err(other) => Err(RuntimeError::Backend(format!(
                    "expected parameter_golf_projection_loss refusal, found {other}"
                ))),
                Ok(_) => Err(RuntimeError::Backend(String::from(
                    "parameter_golf_projection_loss executed successfully instead of refusing",
                ))),
            }
        }
    }

    #[test]
    fn cpu_backend_shared_conformance_harness_has_no_failures() {
        let mut backend = CpuBackend::new();
        let report = run_graph_backend_conformance(&mut backend);
        assert_eq!(report.backend, "cpu");
        assert_eq!(report.surface, "graph_execution");
        assert!(!report.has_failures(), "{report:?}");
    }

    #[test]
    fn cpu_backend_reports_default_device() -> Result<(), psionic_runtime::RuntimeError> {
        let backend = CpuBackend::new();
        let devices = backend.discover_devices()?;
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device, Device::cpu());
        assert_eq!(devices[0].device_name.as_deref(), Some("host cpu"));
        assert_eq!(devices[0].supported_dtypes, vec![DType::F32]);
        assert_eq!(devices[0].unified_memory, Some(true));
        assert_eq!(devices[0].feature_flags, vec![String::from("host_memory")]);
        assert_eq!(
            devices[0].supported_quantization,
            vec![
                super::QuantizationSupport {
                    mode: psionic_core::QuantizationMode::None,
                    load_path: super::QuantizationLoadPath::DenseF32,
                    execution: super::QuantizationExecution::Native,
                },
                super::QuantizationSupport {
                    mode: psionic_core::QuantizationMode::Int8Symmetric,
                    load_path: super::QuantizationLoadPath::DequantizedF32,
                    execution: super::QuantizationExecution::DequantizeToF32,
                },
                super::QuantizationSupport {
                    mode: psionic_core::QuantizationMode::GgmlQ4_0,
                    load_path: super::QuantizationLoadPath::BackendQuantized,
                    execution: super::QuantizationExecution::Native,
                },
                super::QuantizationSupport {
                    mode: psionic_core::QuantizationMode::GgmlQ4_1,
                    load_path: super::QuantizationLoadPath::BackendQuantized,
                    execution: super::QuantizationExecution::Native,
                },
                super::QuantizationSupport {
                    mode: psionic_core::QuantizationMode::GgmlQ8_0,
                    load_path: super::QuantizationLoadPath::BackendQuantized,
                    execution: super::QuantizationExecution::Native,
                },
            ]
        );
        Ok(())
    }

    #[test]
    fn cpu_backend_selection_reports_direct_cpu_execution() -> Result<(), RuntimeError> {
        let backend = CpuBackend::new();
        let selection = backend.backend_selection(&["input", "constant", "matmul", "add"])?;
        assert_eq!(selection.requested_backend, "cpu");
        assert_eq!(selection.effective_backend, "cpu");
        assert_eq!(
            selection
                .selected_device
                .as_ref()
                .map(|device| device.device_name.as_deref()),
            Some(Some("host cpu"))
        );
        assert_eq!(
            selection.supported_ops,
            vec![
                String::from("input"),
                String::from("constant"),
                String::from("matmul"),
                String::from("add")
            ]
        );
        assert_eq!(
            selection.policy,
            ServedProductBackendPolicy::same_backend_only()
        );
        assert_eq!(selection.selection_state, BackendSelectionState::Direct);
        assert!(selection.fallback_reason.is_none());
        assert!(selection.degraded_reason.is_none());
        assert_eq!(
            selection
                .backend_extensions
                .iter()
                .map(|support| support.kind)
                .collect::<Vec<_>>(),
            vec![
                BackendExtensionKind::ReluSquared,
                BackendExtensionKind::LeakyReluSquared,
                BackendExtensionKind::Silu,
                BackendExtensionKind::RmsNorm,
                BackendExtensionKind::LayerNorm,
                BackendExtensionKind::RotaryEmbedding,
                BackendExtensionKind::ScaledDotProductAttention,
                BackendExtensionKind::QuantizedMatmul,
            ]
        );
        let runtime_resources = selection
            .runtime_resources
            .as_ref()
            .ok_or_else(|| RuntimeError::Backend(String::from("cpu runtime resources")))?;
        assert_eq!(
            runtime_resources.allocator_pool.policy.mode,
            AllocatorPoolMode::ExactTensorSpec
        );
        assert!(!runtime_resources.kernel_cache.policy.enabled);
        assert!(runtime_resources.device_memory_budget.is_none());
        assert_eq!(backend.health().status, HealthStatus::Ready);
        Ok(())
    }

    #[test]
    fn cpu_backend_executes_matmul_add_graph() -> Result<(), RuntimeError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![2, 2]), DType::F32);
        let weights = builder
            .constant_f32(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let bias = builder
            .constant_f32(Shape::new(vec![2, 2]), vec![0.5, 0.5, 0.5, 0.5])
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let projected = builder
            .matmul(&input, &weights)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let shifted = builder
            .add(&projected, &bias)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let graph = builder.finish(vec![shifted.clone()]);

        let mut backend = CpuBackend::new();
        let mut inputs = BTreeMap::new();
        inputs.insert(
            input.id(),
            backend.input_buffer(Shape::new(vec![2, 2]), vec![1.0, 0.0, 0.0, 1.0])?,
        );

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let Some(output) = result.outputs.get(&shifted.id()) else {
            return Err(RuntimeError::Backend(String::from("missing output")));
        };
        assert_eq!(output.as_f32_slice(), Some(&[1.5, 2.5, 3.5, 4.5][..]));
        assert_eq!(result.metrics.steps_executed, 5);
        Ok(())
    }

    #[test]
    fn cpu_backend_refuses_parameter_golf_projection_loss_backend_extension()
    -> Result<(), RuntimeError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let logits = builder.input("logits", Shape::new(vec![1, 2, 4]), DType::F32);
        let target_ids = builder.input("target_ids", Shape::new(vec![1, 2]), DType::I32);
        let loss = builder
            .parameter_golf_projection_loss(&logits, &target_ids, 30.0)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let graph = builder.finish(vec![loss.clone()]);

        let mut backend = CpuBackend::new();
        let inputs = BTreeMap::from([
            (
                logits.id(),
                backend.input_buffer(
                    Shape::new(vec![1, 2, 4]),
                    vec![0.1_f32, -0.2, 0.3, 0.7, -0.4, 0.5, -0.1, 0.2],
                )?,
            ),
            (
                target_ids.id(),
                CpuBuffer::from_f32(
                    TensorSpec::new(Shape::new(vec![1, 2]), DType::I32, Device::cpu()),
                    vec![3.0_f32, 1.0],
                )?,
            ),
        ]);

        let error = backend
            .compile_and_execute(&graph, &inputs)
            .expect_err("cpu backend should refuse parameter golf projection loss");
        assert_eq!(
            error,
            RuntimeError::UnsupportedStep(String::from("parameter_golf_projection_loss"))
        );
        Ok(())
    }

    #[test]
    fn cpu_backend_supports_reshape_and_reduce_sum() -> Result<(), RuntimeError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![2, 2]), DType::F32);
        let reshaped = builder
            .reshape(&input, Shape::new(vec![4]))
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let reduced = builder.reduce_sum(&reshaped);
        let graph = builder.finish(vec![reduced.clone()]);

        let mut backend = CpuBackend::new();
        let mut inputs = BTreeMap::new();
        inputs.insert(
            input.id(),
            backend.input_buffer(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])?,
        );

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let Some(output) = result.outputs.get(&reduced.id()) else {
            return Err(RuntimeError::Backend(String::from("missing output")));
        };
        assert_eq!(output.as_f32_slice(), Some(&[10.0][..]));
        Ok(())
    }

    #[test]
    fn cpu_backend_executes_layout_views_and_axis_reduction() -> Result<(), RuntimeError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![2, 3]), DType::F32);
        let permuted = builder
            .permute(&input, vec![1, 0])
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let sliced = builder
            .slice(&permuted, 0, 1, 3)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let selected = builder
            .select(&sliced, 1, 0)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let expanded = builder
            .expand(&selected, Shape::new(vec![2, 2]))
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let concatenated = builder
            .concat(&[expanded.clone(), expanded], 0)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let reduced = builder
            .reduce_sum_axis(&concatenated, 0)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let graph = builder.finish(vec![reduced.clone()]);

        let mut backend = CpuBackend::new();
        let mut inputs = BTreeMap::new();
        inputs.insert(
            input.id(),
            backend.input_buffer(Shape::new(vec![2, 3]), vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0])?,
        );

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let Some(output) = result.outputs.get(&reduced.id()) else {
            return Err(RuntimeError::Backend(String::from("missing output")));
        };
        assert_eq!(output.as_f32_slice(), Some(&[8.0, 12.0][..]));
        Ok(())
    }

    #[test]
    fn cpu_backend_executes_broadcast_add_over_index_views() -> Result<(), RuntimeError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![2, 3]), DType::F32);
        let row = builder
            .select(&input, 0, 0)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let shifted = builder
            .add(&input, &row)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let reduced = builder.reduce_sum_axis(&shifted, 1).map_err(|error| {
            RuntimeError::Backend(format!("axis reduction should remain valid: {error}"))
        })?;
        let graph = builder.finish(vec![shifted.clone(), reduced.clone()]);

        let mut backend = CpuBackend::new();
        let mut inputs = BTreeMap::new();
        inputs.insert(
            input.id(),
            backend.input_buffer(Shape::new(vec![2, 3]), vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0])?,
        );

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let Some(shifted_output) = result.outputs.get(&shifted.id()) else {
            return Err(RuntimeError::Backend(String::from(
                "missing shifted output",
            )));
        };
        let Some(reduced_output) = result.outputs.get(&reduced.id()) else {
            return Err(RuntimeError::Backend(String::from(
                "missing reduced output",
            )));
        };

        assert_eq!(
            shifted_output.as_f32_slice(),
            Some(&[2.0, 4.0, 6.0, 5.0, 7.0, 9.0][..])
        );
        assert_eq!(reduced_output.as_f32_slice(), Some(&[12.0, 21.0][..]));
        Ok(())
    }

    #[test]
    fn cpu_buffer_views_preserve_storage_identity_and_view_semantics() -> Result<(), RuntimeError> {
        let source = CpuBuffer::from_f32(
            TensorSpec::new(Shape::new(vec![2, 3]), DType::F32, Device::cpu()),
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        )?;
        let view_spec =
            TensorSpec::from_layout(
                source.spec().layout().selected(0, 0).ok_or_else(|| {
                    RuntimeError::Backend(String::from("select view should exist"))
                })?,
                DType::F32,
                Device::cpu(),
            );
        let view = CpuBuffer::view_of(&source, view_spec)?;
        let broadcast_spec = TensorSpec::from_layout(
            view.spec()
                .layout()
                .expanded(&Shape::new(vec![2, 3]))
                .ok_or_else(|| {
                    RuntimeError::Backend(String::from("broadcast view should exist"))
                })?,
            DType::F32,
            Device::cpu(),
        );
        let broadcast = CpuBuffer::view_of(&view, broadcast_spec)?;

        let source_contract = source.storage_contract();
        let view_contract = view.storage_contract();
        let broadcast_contract = broadcast.storage_contract();
        assert!(source_contract.is_some());
        assert!(view_contract.is_some());
        assert!(broadcast_contract.is_some());
        let Some(source_contract) = source_contract else {
            return Err(RuntimeError::Backend(String::from(
                "source contract missing",
            )));
        };
        let Some(view_contract) = view_contract else {
            return Err(RuntimeError::Backend(String::from("view contract missing")));
        };
        let Some(broadcast_contract) = broadcast_contract else {
            return Err(RuntimeError::Backend(String::from(
                "broadcast contract missing",
            )));
        };

        assert_eq!(source_contract.view_semantics, ViewSemantics::Dense);
        assert_eq!(view_contract.identity, source_contract.identity);
        assert_eq!(view_contract.view_semantics, ViewSemantics::AliasView);
        assert_eq!(broadcast_contract.identity, source_contract.identity);
        assert_eq!(
            broadcast_contract.view_semantics,
            ViewSemantics::BroadcastView
        );
        Ok(())
    }

    #[test]
    fn cpu_allocator_pool_reuses_dense_storage_identity() -> Result<(), RuntimeError> {
        let spec = TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu());
        let mut pool = CpuAllocatorPool::new(cpu_allocator_pool_policy());
        let buffer = pool.allocate(&spec);
        let first_contract = buffer.storage_contract().ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "allocated buffer should expose storage contract",
            ))
        })?;
        pool.recycle(buffer);

        let reused = pool.allocate(&spec);
        let reused_contract = reused.storage_contract().ok_or_else(|| {
            RuntimeError::Backend(String::from("reused buffer should expose storage contract"))
        })?;

        assert_eq!(reused_contract.identity, first_contract.identity);
        assert_eq!(reused_contract.view_semantics, ViewSemantics::Dense);
        Ok(())
    }

    #[test]
    fn cpu_allocator_creates_zeroed_buffer() -> Result<(), psionic_runtime::RuntimeError> {
        let mut backend = CpuBackend::new();
        let spec = TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu());
        let buffer = backend.allocate(&spec)?;
        assert_eq!(buffer.as_f32_slice(), Some(&[0.0, 0.0, 0.0, 0.0][..]));
        assert_eq!(buffer.spec(), &spec);
        Ok(())
    }

    #[test]
    fn cpu_backend_reuses_intermediate_buffers_via_allocator_pool() -> Result<(), RuntimeError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![2, 2]), DType::F32);
        let weights = builder
            .constant_f32(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let projected = builder
            .matmul(&input, &weights)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let graph = builder.finish(vec![projected.clone()]);

        let mut backend = CpuBackend::new();
        let mut inputs = BTreeMap::new();
        inputs.insert(
            input.id(),
            backend.input_buffer(Shape::new(vec![2, 2]), vec![1.0, 0.0, 0.0, 1.0])?,
        );
        let _ = backend.compile_and_execute(&graph, &inputs)?;

        let runtime_resources = backend
            .runtime_resources()
            .ok_or_else(|| RuntimeError::Backend(String::from("cpu runtime resources")))?;
        let before = runtime_resources.allocator_pool.state.cached_buffers;
        assert!(before >= 1);

        let spec = TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu());
        let buffer = backend.allocate(&spec)?;
        let runtime_resources = backend
            .runtime_resources()
            .ok_or_else(|| RuntimeError::Backend(String::from("cpu runtime resources")))?;
        let after = runtime_resources.allocator_pool.state.cached_buffers;

        assert!(after < before);
        assert_eq!(buffer.as_f32_slice(), Some(&[0.0, 0.0, 0.0, 0.0][..]));
        Ok(())
    }

    #[test]
    fn cpu_backend_executes_backend_extension_reference_ops() -> Result<(), RuntimeError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![1, 1, 2, 32]), DType::F32);
        let norm_weight = builder
            .constant_f32(Shape::new(vec![32]), vec![1.0; 32])
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let output_weight = builder
            .constant_f32(Shape::new(vec![4]), vec![1.0, 1.0, 1.0, 1.0])
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let output_bias = builder
            .constant_f32(Shape::new(vec![4]), vec![0.1, 0.1, 0.1, 0.1])
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let cos = builder
            .constant_f32(Shape::new(vec![2, 16]), vec![1.0f32; 32])
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let sin = builder
            .constant_f32(Shape::new(vec![2, 16]), vec![0.0f32; 32])
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let rhs = builder
            .constant_quantized_blocks(
                Shape::new(vec![4, 32]),
                QuantizationMode::GgmlQ4_0,
                sample_repeated_q4_0_rows(4),
            )
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;

        let normed = builder
            .rms_norm(&input, &norm_weight, 1e-5)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let roped = builder
            .rope(&normed, &cos, &sin, true)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let attended = builder
            .scaled_dot_product_attention(&roped, &roped, &roped, 0.5, true)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let flattened = builder
            .reshape(&attended, Shape::new(vec![2, 32]))
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let quantized = builder
            .quantized_matmul(&flattened, &rhs, QuantizationMode::GgmlQ4_0)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let output = builder
            .layer_norm(&quantized, &output_weight, &output_bias, 1e-5)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let graph = builder.finish(vec![output.clone()]);

        let mut backend = CpuBackend::new();
        let mut inputs = BTreeMap::new();
        inputs.insert(
            input.id(),
            backend.input_buffer(
                Shape::new(vec![1, 1, 2, 32]),
                (1..=64).map(|value| value as f32).collect::<Vec<_>>(),
            )?,
        );

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output_buffer = result
            .outputs
            .get(&output.id())
            .ok_or_else(|| RuntimeError::Backend(String::from("extension output")))?;
        let values = output_buffer.logical_values()?;
        assert_eq!(values.len(), 8);
        assert!(values.iter().all(|value| value.is_finite()));
        Ok(())
    }

    #[test]
    fn cpu_backend_executes_leaky_relu_squared_reference_op() -> Result<(), RuntimeError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![4]), DType::F32);
        let output = builder
            .leaky_relu_squared(&input, 0.5)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let graph = builder.finish(vec![output.clone()]);

        let mut backend = CpuBackend::new();
        let mut inputs = BTreeMap::new();
        inputs.insert(
            input.id(),
            backend.input_buffer(Shape::new(vec![4]), vec![-2.0_f32, -0.5, 0.75, 3.0])?,
        );

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output = result
            .outputs
            .get(&output.id())
            .ok_or_else(|| RuntimeError::Backend(String::from("leaky relu squared output")))?;
        assert_eq!(
            output.as_f32_slice(),
            Some(&[1.0_f32, 0.0625, 0.5625, 9.0][..])
        );
        Ok(())
    }

    #[test]
    fn cpu_backend_executes_quantized_matmul_for_supported_ggml_modes() -> Result<(), RuntimeError>
    {
        assert_quantized_matmul_matches_dense_reference(
            QuantizationMode::GgmlMxfp4,
            sample_repeated_mxfp4_rows(3),
        )?;
        assert_quantized_matmul_matches_dense_reference(
            QuantizationMode::GgmlQ4_0,
            sample_repeated_q4_0_rows(3),
        )?;
        assert_quantized_matmul_matches_dense_reference(
            QuantizationMode::GgmlQ4_1,
            sample_repeated_q4_1_rows(3),
        )?;
        assert_quantized_matmul_matches_dense_reference(
            QuantizationMode::GgmlQ5_0,
            sample_repeated_q5_0_rows(3),
        )?;
        assert_quantized_matmul_matches_dense_reference(
            QuantizationMode::GgmlQ8_0,
            sample_repeated_q8_0_rows(3),
        )?;
        Ok(())
    }

    #[test]
    fn cpu_quantized_row_helpers_cover_mxfp4() -> Result<(), RuntimeError> {
        let shape = Shape::new(vec![2, 32]);
        let layout = QuantizationMode::GgmlMxfp4
            .ggml_block_layout(&shape)
            .ok_or_else(|| RuntimeError::Backend(String::from("mxfp4 layout")))?;
        assert_eq!(super::quantized_row_byte_len(&shape, layout)?, 17);

        let row_bytes = sample_q8_0_like_reference_vector();
        let quantized = sample_mxfp4_row();
        let mut decoded = Vec::new();
        super::decode_quantized_row_into(QuantizationMode::GgmlMxfp4, &quantized, &mut decoded)?;
        let dot = super::quantized_row_dot(
            row_bytes.as_slice(),
            QuantizationMode::GgmlMxfp4,
            &quantized,
        )?;
        let expected: f32 = row_bytes
            .iter()
            .zip(decoded.iter())
            .map(|(lhs, rhs)| lhs * rhs)
            .sum();
        assert!((dot - expected).abs() <= 1e-5);
        assert_eq!(decoded.len(), 32);
        Ok(())
    }

    #[test]
    fn cpu_quantized_row_helpers_cover_ggml_k_modes() -> Result<(), RuntimeError> {
        let reference = sample_reference_vector_256();

        let q4_k = sample_q4_k_row(0.5, 0.125, 3);
        let mut decoded_q4_k = Vec::new();
        super::decode_quantized_row_into(QuantizationMode::GgmlQ4K, &q4_k, &mut decoded_q4_k)?;
        let dot_q4_k = super::quantized_row_dot(&reference, QuantizationMode::GgmlQ4K, &q4_k)?;
        let expected_q4_k: f32 = reference
            .iter()
            .zip(decoded_q4_k.iter())
            .map(|(lhs, rhs)| lhs * rhs)
            .sum();
        assert!((dot_q4_k - expected_q4_k).abs() <= 1e-4);
        assert_eq!(decoded_q4_k.len(), 256);

        let q6_k = sample_q6_k_row(0.25, -3);
        let mut decoded_q6_k = Vec::new();
        super::decode_quantized_row_into(QuantizationMode::GgmlQ6K, &q6_k, &mut decoded_q6_k)?;
        let dot_q6_k = super::quantized_row_dot(&reference, QuantizationMode::GgmlQ6K, &q6_k)?;
        let expected_q6_k: f32 = reference
            .iter()
            .zip(decoded_q6_k.iter())
            .map(|(lhs, rhs)| lhs * rhs)
            .sum();
        assert!((dot_q6_k - expected_q6_k).abs() <= 1e-4);
        assert_eq!(decoded_q6_k.len(), 256);

        let q5_k = sample_q5_k_row(0.5, 0.125, 3);
        let mut decoded_q5_k = Vec::new();
        super::decode_quantized_row_into(QuantizationMode::GgmlQ5K, &q5_k, &mut decoded_q5_k)?;
        let dot_q5_k = super::quantized_row_dot(&reference, QuantizationMode::GgmlQ5K, &q5_k)?;
        let expected_q5_k: f32 = reference
            .iter()
            .zip(decoded_q5_k.iter())
            .map(|(lhs, rhs)| lhs * rhs)
            .sum();
        assert!((dot_q5_k - expected_q5_k).abs() <= 1e-4);
        assert_eq!(decoded_q5_k.len(), 256);
        Ok(())
    }

    #[test]
    fn cpu_quantized_row_helpers_cover_qwen38_super_blocks() -> Result<(), RuntimeError> {
        let reference = sample_reference_vector_256();
        for (mode, bytes) in [
            (QuantizationMode::GgmlQ3K, vec![0_u8; 110]),
            (QuantizationMode::GgmlIq3S, vec![0_u8; 110]),
            (QuantizationMode::GgmlIq4Xs, vec![0_u8; 136]),
        ] {
            let mut decoded = Vec::new();
            super::decode_quantized_row_into(mode, &bytes, &mut decoded)?;
            let dot = super::quantized_row_dot(&reference, mode, &bytes)?;
            let expected: f32 = reference
                .iter()
                .zip(decoded.iter())
                .map(|(lhs, rhs)| lhs * rhs)
                .sum();
            assert!((dot - expected).abs() <= 1e-4, "mode {mode:?}");
            assert_eq!(decoded.len(), 256, "mode {mode:?}");
        }
        Ok(())
    }

    #[test]
    fn cpu_quantized_row_helpers_cover_q5_0() -> Result<(), RuntimeError> {
        let reference = sample_q8_0_like_reference_vector();
        let q5_0 = sample_q5_0_row();
        let mut decoded = Vec::new();
        super::decode_quantized_row_into(QuantizationMode::GgmlQ5_0, &q5_0, &mut decoded)?;
        let dot = super::quantized_row_dot(&reference, QuantizationMode::GgmlQ5_0, &q5_0)?;
        let expected: f32 = reference
            .iter()
            .zip(decoded.iter())
            .map(|(lhs, rhs)| lhs * rhs)
            .sum();
        assert!((dot - expected).abs() <= 1e-4);
        assert_eq!(decoded.len(), 32);
        Ok(())
    }

    #[test]
    fn cpu_backend_outputs_quantized_constant_storage_truth() -> Result<(), RuntimeError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let rhs = builder
            .constant_quantized_blocks(
                Shape::new(vec![2, 32]),
                QuantizationMode::GgmlQ8_0,
                sample_repeated_q8_0_rows(2),
            )
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let graph = builder.finish(vec![rhs.clone()]);

        let mut backend = CpuBackend::new();
        let result = backend.compile_and_execute(&graph, &BTreeMap::new())?;
        let output = result
            .outputs
            .get(&rhs.id())
            .ok_or_else(|| RuntimeError::Backend(String::from("quantized constant output")))?;
        assert_eq!(
            output.storage_kind(),
            psionic_runtime::BufferStorageKind::QuantizedBlocks {
                mode: QuantizationMode::GgmlQ8_0,
                layout: QuantizationMode::GgmlQ8_0
                    .ggml_block_layout(&Shape::new(vec![2, 32]))
                    .ok_or_else(|| RuntimeError::Backend(String::from("q8 layout")))?,
                residency: psionic_runtime::BufferResidency::Host,
            }
        );
        Ok(())
    }

    fn assert_quantized_matmul_matches_dense_reference(
        mode: QuantizationMode,
        bytes: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        let right_shape = Shape::new(vec![3, 32]);
        let left_shape = Shape::new(vec![2, 32]);
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", left_shape.clone(), DType::F32);
        let rhs = builder
            .constant_quantized_blocks(right_shape.clone(), mode, bytes.clone())
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let output = builder
            .quantized_matmul(&input, &rhs, mode)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let graph = builder.finish(vec![output.clone()]);

        let mut backend = CpuBackend::new();
        let left_values = (0..64)
            .map(|index| (index as f32 / 8.0) - 4.0)
            .collect::<Vec<_>>();
        let mut inputs = BTreeMap::new();
        inputs.insert(
            input.id(),
            backend.input_buffer(left_shape.clone(), left_values.clone())?,
        );

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output = result
            .outputs
            .get(&output.id())
            .ok_or_else(|| RuntimeError::Backend(String::from("quantized matmul output")))?;
        let actual = output
            .as_f32_slice()
            .ok_or_else(|| RuntimeError::Backend(String::from("dense quantized output")))?;

        let layout = mode
            .ggml_block_layout(&right_shape)
            .ok_or_else(|| RuntimeError::Backend(String::from("quantized rhs layout")))?;
        let dequantized = super::decode_quantized_values(&right_shape, mode, layout, &bytes)?;
        let expected = dense_reference_quantized_rhs(&left_values, 2, &dequantized, 3, 32);

        for (actual, expected) in actual.iter().zip(expected.iter()) {
            let diff = (actual - expected).abs();
            assert!(
                diff <= 0.01,
                "mode {mode:?} drifted by {diff}: actual={actual} expected={expected}"
            );
        }
        Ok(())
    }

    fn dense_reference_quantized_rhs(
        lhs: &[f32],
        lhs_rows: usize,
        rhs_rows: &[f32],
        rhs_row_count: usize,
        width: usize,
    ) -> Vec<f32> {
        let mut output = vec![0.0; lhs_rows * rhs_row_count];
        for row in 0..lhs_rows {
            let lhs_row = &lhs[row * width..(row + 1) * width];
            for rhs_row in 0..rhs_row_count {
                let rhs = &rhs_rows[rhs_row * width..(rhs_row + 1) * width];
                output[row * rhs_row_count + rhs_row] = lhs_row
                    .iter()
                    .zip(rhs.iter())
                    .map(|(lhs, rhs)| lhs * rhs)
                    .sum();
            }
        }
        output
    }

    fn sample_repeated_q4_0_rows(rows: usize) -> Vec<u8> {
        sample_q4_0_row()
            .into_iter()
            .cycle()
            .take(rows * 18)
            .collect()
    }

    fn sample_repeated_mxfp4_rows(rows: usize) -> Vec<u8> {
        sample_mxfp4_row()
            .into_iter()
            .cycle()
            .take(rows * 17)
            .collect()
    }

    fn sample_repeated_q4_1_rows(rows: usize) -> Vec<u8> {
        sample_q4_1_row()
            .into_iter()
            .cycle()
            .take(rows * 20)
            .collect()
    }

    fn sample_repeated_q8_0_rows(rows: usize) -> Vec<u8> {
        sample_q8_0_row()
            .into_iter()
            .cycle()
            .take(rows * 34)
            .collect()
    }

    fn sample_repeated_q5_0_rows(rows: usize) -> Vec<u8> {
        sample_q5_0_row()
            .into_iter()
            .cycle()
            .take(rows * 22)
            .collect()
    }

    fn sample_q4_0_row() -> Vec<u8> {
        [0x00_u8, 0x40]
            .into_iter()
            .chain(
                [0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]
                    .into_iter()
                    .cycle()
                    .take(16),
            )
            .collect()
    }

    fn sample_mxfp4_row() -> Vec<u8> {
        std::iter::once(128_u8)
            .chain(
                [0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]
                    .into_iter()
                    .cycle()
                    .take(16),
            )
            .collect()
    }

    fn sample_q4_1_row() -> Vec<u8> {
        [0x00_u8, 0x40, 0x00, 0xbc]
            .into_iter()
            .chain(
                [0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]
                    .into_iter()
                    .cycle()
                    .take(16),
            )
            .collect()
    }

    fn sample_q5_0_row() -> Vec<u8> {
        [0x00_u8, 0x40, 0x00, 0x00, 0x00, 0x00]
            .into_iter()
            .chain(
                [0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]
                    .into_iter()
                    .cycle()
                    .take(16),
            )
            .collect()
    }

    fn sample_q8_0_row() -> Vec<u8> {
        std::iter::once(0x00)
            .chain(std::iter::once(0x40))
            .chain((1_i8..=32).map(|value| value.to_le_bytes()[0]))
            .collect()
    }

    fn sample_q8_0_like_reference_vector() -> Vec<f32> {
        (0..32).map(|index| index as f32 / 8.0 - 2.0).collect()
    }

    fn sample_reference_vector_256() -> Vec<f32> {
        (0..256)
            .map(|index| ((index as i32 * 13) % 29 - 14) as f32 / 3.0)
            .collect()
    }

    fn f32_to_f16_bits(value: f32) -> u16 {
        let bits = value.to_bits();
        let sign = ((bits >> 31) as u16) << 15;
        let exponent = ((bits >> 23) & 0xff) as i32;
        let mantissa = bits & 0x7f_ffff;
        if exponent == 0xff {
            let nan_payload = if mantissa == 0 { 0 } else { 0x0200 };
            return sign | 0x7c00 | nan_payload;
        }
        let half_exponent = exponent - 127 + 15;
        if half_exponent >= 0x1f {
            return sign | 0x7c00;
        }
        if half_exponent <= 0 {
            if half_exponent < -10 {
                return sign;
            }
            let mantissa_with_hidden = mantissa | 0x80_0000;
            let shift = (14 - half_exponent) as u32;
            let mut half_mantissa = (mantissa_with_hidden >> shift) as u16;
            let round_bit = 1_u32 << (shift.saturating_sub(1));
            let remainder_mask = round_bit.saturating_sub(1);
            let round_remainder = mantissa_with_hidden & remainder_mask;
            let round = (mantissa_with_hidden & round_bit) != 0
                && (round_remainder != 0 || (half_mantissa & 1) != 0);
            if round {
                half_mantissa = half_mantissa.wrapping_add(1);
            }
            return sign | half_mantissa;
        }
        let mut half_mantissa = (mantissa >> 13) as u16;
        let round = (mantissa & 0x1fff) > 0x1000
            || ((mantissa & 0x1fff) == 0x1000 && (half_mantissa & 1) != 0);
        let mut half_exponent_u16 = half_exponent as u16;
        if round {
            half_mantissa = half_mantissa.wrapping_add(1);
            if half_mantissa == 0x0400 {
                half_mantissa = 0;
                half_exponent_u16 = half_exponent_u16.wrapping_add(1);
                if half_exponent_u16 >= 0x1f {
                    return sign | 0x7c00;
                }
            }
        }
        sign | (half_exponent_u16 << 10) | half_mantissa
    }

    fn sample_q4_k_row(scale: f32, minimum: f32, offset: u8) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(144);
        bytes.extend_from_slice(&f32_to_f16_bits(scale).to_le_bytes());
        bytes.extend_from_slice(&f32_to_f16_bits(minimum).to_le_bytes());
        for index in 0..12_u8 {
            bytes.push(offset.wrapping_add(index.wrapping_mul(7)));
        }
        for index in 0..128_u8 {
            let low = index & 0x0f;
            let high = (15_u8).wrapping_sub(index & 0x0f) & 0x0f;
            bytes.push(low | (high << 4));
        }
        bytes
    }

    fn sample_q5_k_row(scale: f32, minimum: f32, offset: u8) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(176);
        bytes.extend_from_slice(&f32_to_f16_bits(scale).to_le_bytes());
        bytes.extend_from_slice(&f32_to_f16_bits(minimum).to_le_bytes());
        for index in 0..12_u8 {
            bytes.push(offset.wrapping_add(index.wrapping_mul(7)));
        }
        for index in 0..32_u8 {
            bytes.push(index.rotate_left(1).wrapping_add(offset));
        }
        for index in 0..128_u8 {
            let low = index & 0x0f;
            let high = (15_u8).wrapping_sub(index & 0x0f) & 0x0f;
            bytes.push(low | (high << 4));
        }
        bytes
    }

    fn sample_q6_k_row(scale: f32, offset: i8) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(210);
        for index in 0..128_u8 {
            let low = index & 0x0f;
            let high = (index.wrapping_mul(3)) & 0x0f;
            bytes.push(low | (high << 4));
        }
        for index in 0..64_u8 {
            let b0 = index & 0x03;
            let b1 = (index.wrapping_add(1)) & 0x03;
            let b2 = (index.wrapping_add(2)) & 0x03;
            let b3 = (index.wrapping_add(3)) & 0x03;
            bytes.push(b0 | (b1 << 2) | (b2 << 4) | (b3 << 6));
        }
        for index in 0..16_i8 {
            bytes.push(offset.saturating_add(index).to_le_bytes()[0]);
        }
        bytes.extend_from_slice(&f32_to_f16_bits(scale).to_le_bytes());
        bytes
    }
}
