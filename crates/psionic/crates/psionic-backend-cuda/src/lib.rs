//! CUDA backend discovery, allocation, submission, and dense execution surface
//! for Psionic.

#![allow(
    clippy::manual_is_multiple_of,
    clippy::result_large_err,
    clippy::too_many_arguments
)]
#![cfg_attr(
    test,
    allow(
        clippy::bool_to_int_with_if,
        clippy::expect_used,
        clippy::manual_slice_size_calculation,
        clippy::panic,
        clippy::panic_in_result_fn
    )
)]

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fmt,
    fs::OpenOptions,
    io::{ErrorKind, Write},
    process::Command,
    time::{Duration, Instant},
};

use half::{bf16, f16};
use psionic_compiler::compile_graph;
use psionic_core::{
    BackendExtensionKind, BackendExtensionOp, DType, Device, DeviceKind, QuantizationMode, Shape,
    TensorData, TensorId, TensorSpec,
};
use psionic_ir::{ExecutionOp, ExecutionPlan, ExecutionStep, Graph};
use psionic_runtime::{
    Allocator, AllocatorPoolPolicy, AllocatorPoolReport, AllocatorPoolState, BackendDegradedPolicy,
    BackendExtensionSupport, BackendName, BackendRuntimeResources, BackendSelection, BufferHandle,
    CacheAction, CacheKind, CacheObservation, CompilePathEvidence, CompilePathTemperature,
    DeviceDescriptor, DeviceDiscovery, DeviceMemoryBudget, ExecutionBackend, ExecutionMetrics,
    ExecutionPlanCachePolicy, ExecutionPlanCacheReport, ExecutionPlanCacheState, ExecutionResult,
    HealthStatus, KernelCachePolicy, KernelCacheReport, KernelCacheState, NvidiaBackendReport,
    NvidiaDeviceMetadata, NvidiaRecoveryAction, NvidiaRecoveryProfile, NvidiaRiskLevel,
    NvidiaRiskProfile, NvidiaTopologyInfo, RuntimeError, RuntimeHealth, ServedProductBackendPolicy,
};
use rayon::prelude::*;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "CUDA backend discovery, allocation, and submission";

const NVIDIA_SMI_BINARY: &str = "nvidia-smi";
const INVENTORY_QUERY: &str = concat!(
    "index,name,pci.bus_id,memory.total,compute_cap,display_mode,",
    "mig.mode.current,persistence_mode,addressing_mode"
);
const OFFLINE_NO_DRIVER_MESSAGE: &str =
    "cuda backend unavailable: nvidia-smi is not installed or the NVIDIA driver is not reachable";
const CUDA_POOL_MAX_CACHED_BUFFERS: usize = 128;
const CUDA_POOL_MAX_CACHED_BYTES: u64 = 64 * 1024 * 1024;
const CUDA_EXECUTION_PLAN_CACHE_MAX_ENTRIES: usize = 64;
const CUDA_EXECUTION_PLAN_CACHE_MAX_CACHED_BYTES: u64 = 1024 * 1024;
const HOST_FALLBACK_PARALLEL_MIN_ELEMENTS: usize = 16 * 1024;
const HOST_FALLBACK_PARALLEL_MIN_CHUNK_ELEMENTS: usize = 1024;
const CUDA_CUBLASLT_F16_TO_F32_MAX_ROWS: usize = 32;

/// Exact plan surface currently covered by the first CUDA-backed served-product
/// milestone.
pub const SUPPORTED_OPS: &[&str] = &[
    "input",
    "constant",
    "cast",
    "matmul",
    "add",
    "mul",
    "relu_squared",
    "relu_squared_backward",
    "relu_squared_backward_from_output",
    "leaky_relu_squared",
    "leaky_relu_squared_backward",
    "silu",
    "silu_backward",
    "parameter_golf_token_embedding_lookup",
    "parameter_golf_banked_linear",
    "parameter_golf_banked_linear_input_backward",
    "parameter_golf_banked_linear_weight_backward",
    "parameter_golf_token_embedding_lookup_backward",
    "parameter_golf_projection_loss",
    "parameter_golf_projection_token_losses",
    "parameter_golf_projection_loss_backward",
    "rms_norm",
    "rotary_embedding",
    "rotary_embedding_backward",
    "scaled_dot_product_attention",
    "scaled_dot_product_attention_query_backward",
    "scaled_dot_product_attention_key_backward",
    "scaled_dot_product_attention_value_backward",
];

/// Dense op surface currently covered for the first CUDA-backed embeddings
/// product path.
pub const EMBEDDINGS_SUPPORTED_OPS: &[&str] = SUPPORTED_OPS;

/// CUDA-side quantized primitive surface required by the GPT-OSS text-generation
/// path.
pub const TEXT_GENERATION_SUPPORTED_OPS: &[&str] =
    &["quantized_matvec_q8_0", "quantized_matvec_mxfp4"];
pub const GGML_Q8_1_BLOCK_ELEMENTS: usize = 32;
pub const GGML_Q8_1_BLOCK_BYTES: usize = 36;
const CUDA_PUBLIC_ATTENTION_MAX_SEQUENCE: usize = 1024;
const CUDA_PUBLIC_STANDARD_ROPE_BASE: f32 = 10_000.0;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CudaAllocatorPoolTelemetry {
    /// Number of fresh device allocations taken on the admitted lane.
    pub cold_allocations: u64,
    /// Number of exact-spec buffers reused from the pool.
    pub reuse_hits: u64,
    /// Number of buffers retained into the pool on drop.
    pub returned_buffers: u64,
    /// Number of returned buffers evicted instead of cached.
    pub evicted_returns: u64,
    /// Bytes currently held by Psionic-owned device allocations, including cached buffers.
    pub resident_device_bytes: u64,
    /// High-water mark for Psionic-owned device allocations, including cached buffers.
    pub peak_resident_device_bytes: u64,
}

/// Live device-memory values reported by the selected CUDA runtime device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CudaDeviceMemoryInfo {
    /// Currently free device memory in bytes.
    pub free_bytes: u64,
    /// Total device memory visible to the runtime in bytes.
    pub total_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CudaGemmTuningScope {
    pub model_family: String,
    pub op_kind: String,
}

impl CudaGemmTuningScope {
    #[must_use]
    pub fn new(model_family: impl Into<String>, op_kind: impl Into<String>) -> Self {
        Self {
            model_family: model_family.into(),
            op_kind: op_kind.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CudaGemmTuningShape {
    pub rows: usize,
    pub inner: usize,
    pub cols: usize,
}

impl CudaGemmTuningShape {
    #[must_use]
    pub const fn new(rows: usize, inner: usize, cols: usize) -> Self {
        Self { rows, inner, cols }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CudaGemmTuningPlanReport {
    pub model_family: String,
    pub op_kind: String,
    pub rows: usize,
    pub inner: usize,
    pub cols: usize,
    pub input_dtype: String,
    pub output_dtype: String,
    pub backend_route: String,
    pub workspace_bytes: u64,
    pub mean_time_us: u64,
    pub algorithm_fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CudaGemmTuningReport {
    pub tuning_status: String,
    pub plan_cache_scope: String,
    pub selected_plan_count: usize,
    pub tuned_shape_count: usize,
    pub fallback_shape_count: usize,
    pub max_workspace_bytes: u64,
    pub selected_plans: Vec<CudaGemmTuningPlanReport>,
}

#[derive(Debug)]
struct CudaAllocatorPool {
    policy: AllocatorPoolPolicy,
    state: AllocatorPoolState,
    telemetry: CudaAllocatorPoolTelemetry,
    cached: BTreeMap<String, Vec<(usize, usize)>>,
}

impl CudaAllocatorPool {
    fn new(policy: AllocatorPoolPolicy) -> Self {
        Self {
            policy,
            state: AllocatorPoolState::default(),
            telemetry: CudaAllocatorPoolTelemetry::default(),
            cached: BTreeMap::new(),
        }
    }

    fn report(&self) -> AllocatorPoolReport {
        AllocatorPoolReport {
            policy: self.policy.clone(),
            state: self.state.clone(),
        }
    }

    fn telemetry(&self) -> CudaAllocatorPoolTelemetry {
        self.telemetry.clone()
    }

    fn checkout(&mut self, cache_key: &str, byte_len: usize) -> Option<usize> {
        if self.policy.mode != psionic_runtime::AllocatorPoolMode::ExactTensorSpec {
            return None;
        }
        let cached = self.cached.get_mut(cache_key)?;
        let (cached_len, device_ptr) = cached.pop()?;
        debug_assert_eq!(cached_len, byte_len);
        self.state.cached_buffers = self.state.cached_buffers.saturating_sub(1);
        self.state.cached_bytes = self.state.cached_bytes.saturating_sub(cached_len as u64);
        self.telemetry.reuse_hits = self.telemetry.reuse_hits.saturating_add(1);
        if cached.is_empty() {
            self.cached.remove(cache_key);
        }
        Some(device_ptr)
    }

    fn record_cold_allocation(&mut self, byte_len: usize) {
        self.telemetry.cold_allocations = self.telemetry.cold_allocations.saturating_add(1);
        self.telemetry.resident_device_bytes = self
            .telemetry
            .resident_device_bytes
            .saturating_add(byte_len.try_into().unwrap_or(u64::MAX));
        self.telemetry.peak_resident_device_bytes = self
            .telemetry
            .peak_resident_device_bytes
            .max(self.telemetry.resident_device_bytes);
    }

    fn record_device_free(&mut self, byte_len: usize) {
        self.telemetry.resident_device_bytes = self
            .telemetry
            .resident_device_bytes
            .saturating_sub(byte_len.try_into().unwrap_or(u64::MAX));
    }

    fn checkin(&mut self, cache_key: String, byte_len: usize, device_ptr: usize) -> bool {
        if self.policy.mode != psionic_runtime::AllocatorPoolMode::ExactTensorSpec {
            self.telemetry.evicted_returns = self.telemetry.evicted_returns.saturating_add(1);
            return false;
        }
        let within_buffer_limit = self.state.cached_buffers < self.policy.max_cached_buffers;
        let within_byte_limit =
            self.state.cached_bytes.saturating_add(byte_len as u64) <= self.policy.max_cached_bytes;
        if !within_buffer_limit || !within_byte_limit {
            self.telemetry.evicted_returns = self.telemetry.evicted_returns.saturating_add(1);
            return false;
        }
        self.cached
            .entry(cache_key)
            .or_default()
            .push((byte_len, device_ptr));
        self.state.cached_buffers = self.state.cached_buffers.saturating_add(1);
        self.state.cached_bytes = self.state.cached_bytes.saturating_add(byte_len as u64);
        self.telemetry.returned_buffers = self.telemetry.returned_buffers.saturating_add(1);
        true
    }
}

fn allocator_pool_cache_key(spec: &TensorSpec) -> String {
    format!(
        "{:?}|{:?}|{}|{}",
        spec.shape().dims(),
        spec.dtype(),
        spec.device().kind(),
        spec.device().ordinal()
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScaledDotProductAttentionBackwardTarget {
    Query,
    Key,
    Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ScaledDotProductAttentionBackwardCacheKey {
    query: TensorId,
    key: TensorId,
    value: TensorId,
    grad_output: TensorId,
    scale_bits: u32,
    causal: bool,
}

#[derive(Clone)]
struct ScaledDotProductAttentionBackwardCachedOutputs {
    query: CudaBuffer,
    key: CudaBuffer,
    value: CudaBuffer,
    _scratch: Vec<CudaBuffer>,
}

impl ScaledDotProductAttentionBackwardCachedOutputs {
    fn output(&self, target: ScaledDotProductAttentionBackwardTarget) -> CudaBuffer {
        match target {
            ScaledDotProductAttentionBackwardTarget::Query => self.query.clone(),
            ScaledDotProductAttentionBackwardTarget::Key => self.key.clone(),
            ScaledDotProductAttentionBackwardTarget::Value => self.value.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ScaledDotProductAttentionForwardGemmLayout {
    batch_count: usize,
    group_rows: usize,
    query_stride_elements: usize,
    kv_stride_elements: usize,
    score_stride_elements: usize,
    output_stride_elements: usize,
    total_score_rows: usize,
}

fn scaled_dot_product_attention_forward_gemm_layout(
    batch_size: usize,
    query_heads: usize,
    kv_heads: usize,
    sequence_length: usize,
    head_dim: usize,
) -> Result<ScaledDotProductAttentionForwardGemmLayout, RuntimeError> {
    let group_size = query_heads.checked_div(kv_heads).ok_or_else(|| {
        RuntimeError::Backend(String::from(
            "cuda scaled_dot_product_attention received zero kv head count on the GEMM lane",
        ))
    })?;
    let batch_count = batch_size.checked_mul(kv_heads).ok_or_else(|| {
        RuntimeError::Backend(String::from(
            "cuda scaled_dot_product_attention GEMM batch-count overflow",
        ))
    })?;
    let group_rows = group_size.checked_mul(sequence_length).ok_or_else(|| {
        RuntimeError::Backend(String::from(
            "cuda scaled_dot_product_attention GEMM row-count overflow",
        ))
    })?;
    let query_stride_elements = group_rows.checked_mul(head_dim).ok_or_else(|| {
        RuntimeError::Backend(String::from(
            "cuda scaled_dot_product_attention GEMM query stride overflow",
        ))
    })?;
    let kv_stride_elements = sequence_length.checked_mul(head_dim).ok_or_else(|| {
        RuntimeError::Backend(String::from(
            "cuda scaled_dot_product_attention GEMM kv stride overflow",
        ))
    })?;
    let score_stride_elements = group_rows.checked_mul(sequence_length).ok_or_else(|| {
        RuntimeError::Backend(String::from(
            "cuda scaled_dot_product_attention GEMM score stride overflow",
        ))
    })?;
    let output_stride_elements = group_rows.checked_mul(head_dim).ok_or_else(|| {
        RuntimeError::Backend(String::from(
            "cuda scaled_dot_product_attention GEMM output stride overflow",
        ))
    })?;
    let total_score_rows = batch_size
        .checked_mul(query_heads)
        .and_then(|value| value.checked_mul(sequence_length))
        .ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda scaled_dot_product_attention GEMM total row-count overflow",
            ))
        })?;
    Ok(ScaledDotProductAttentionForwardGemmLayout {
        batch_count,
        group_rows,
        query_stride_elements,
        kv_stride_elements,
        score_stride_elements,
        output_stride_elements,
        total_score_rows,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReduceSumCudaStrategy {
    Rows {
        row_count: usize,
        column_count: usize,
    },
    Axis0 {
        axis0_extent: usize,
        row_width: usize,
    },
    Axis1Rank3 {
        dim0: usize,
        dim1: usize,
        dim2: usize,
    },
}

/// Returns the device scratch-buffer size for contiguous GGML `Q8_1` rows.
pub fn ggml_q8_1_storage_bytes(rows: usize, cols: usize) -> Result<usize, RuntimeError> {
    if cols == 0 || cols % GGML_Q8_1_BLOCK_ELEMENTS != 0 {
        return Err(RuntimeError::Backend(format!(
            "ggml q8_1 scratch requires block-aligned width {cols}",
        )));
    }
    Ok(rows
        .saturating_mul(cols / GGML_Q8_1_BLOCK_ELEMENTS)
        .saturating_mul(GGML_Q8_1_BLOCK_BYTES))
}

const ATTENTION_FA3_MAX_SPLITS: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
struct NvidiaInventoryRow {
    ordinal: u16,
    name: String,
    pci_bdf: Option<String>,
    memory_bytes: Option<u64>,
    compute_capability: Option<String>,
    display_attached: Option<bool>,
    mig_profile: Option<String>,
    persistence_mode_enabled: Option<bool>,
    addressing_mode: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NvidiaQueryErrorKind {
    NotInstalled,
    Failed,
    InvalidUtf8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NvidiaQueryError {
    kind: NvidiaQueryErrorKind,
    message: String,
}

/// CUDA-visible backing memory class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CudaMemorySpace {
    /// Ordinary device-only memory allocated by the CUDA runtime.
    Device,
}

/// How long Psionic should wait after a CUDA submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CudaCommandWait {
    /// Wait until the CUDA stream completes.
    Completed,
}

/// Stable submission lifecycle state exposed by Psionic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CudaCommandStatus {
    /// The submission was recorded onto a CUDA stream.
    Submitted,
    /// The CUDA stream completed successfully.
    Completed,
    /// The CUDA stream failed.
    Error,
}

/// Submission metadata returned after a CUDA stream is synchronized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CudaSubmissionReport {
    /// Final stream status observed by Psionic.
    pub status: CudaCommandStatus,
    /// Number of explicit operations recorded in the submission.
    pub encoded_operations: usize,
}

/// Reusable captured CUDA graph executable for one fixed submission shape.
pub struct CudaGraphExec {
    encoded_operations: usize,
    platform: platform::PlatformGraphExec,
}

impl fmt::Debug for CudaGraphExec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CudaGraphExec")
            .field("encoded_operations", &self.encoded_operations)
            .field("platform", &"<cuda graph exec>")
            .finish()
    }
}

impl CudaGraphExec {
    /// Launches the captured CUDA graph and optionally waits for completion.
    pub fn launch(&self, wait: CudaCommandWait) -> Result<CudaSubmissionReport, RuntimeError> {
        let status = self.platform.launch(wait)?;
        Ok(CudaSubmissionReport {
            status,
            encoded_operations: self.encoded_operations,
        })
    }
}

/// Explicit per-call counters for one quantized CUDA matvec request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CudaQuantizedMatvecStats {
    /// Bytes uploaded from host to device for the activation input.
    pub host_to_device_bytes: u64,
    /// Bytes read back from device to host for the output vector.
    pub device_to_host_bytes: u64,
    /// Number of CUDA submissions used by the call.
    pub submission_count: usize,
    /// Number of CUDA stream synchronizations used by the call.
    pub sync_count: usize,
    /// Number of quantized CUDA kernels launched by the call.
    pub kernel_launches: usize,
}

impl CudaQuantizedMatvecStats {
    /// Accumulates another stats snapshot into this one.
    pub fn accumulate(&mut self, other: &Self) {
        self.host_to_device_bytes = self
            .host_to_device_bytes
            .saturating_add(other.host_to_device_bytes);
        self.device_to_host_bytes = self
            .device_to_host_bytes
            .saturating_add(other.device_to_host_bytes);
        self.submission_count = self.submission_count.saturating_add(other.submission_count);
        self.sync_count = self.sync_count.saturating_add(other.sync_count);
        self.kernel_launches = self.kernel_launches.saturating_add(other.kernel_launches);
    }
}

/// Result payload for one profiled quantized CUDA matvec call.
#[derive(Clone, Debug, PartialEq)]
pub struct CudaQuantizedMatvecResult {
    /// Returned dense output values.
    pub values: Vec<f32>,
    /// Explicit transfer and submission counters for the call.
    pub stats: CudaQuantizedMatvecStats,
}

/// Flattened top-k selection result returned by the CUDA backend.
#[derive(Clone, Debug, PartialEq)]
pub struct CudaTopKResult {
    /// Number of rows processed from the source logits buffer.
    pub row_count: usize,
    /// Number of selected elements per row.
    pub top_k: usize,
    /// Row-major selected indices.
    pub indices: Vec<u32>,
    /// Row-major selected values aligned with `indices`.
    pub values: Vec<f32>,
}

/// CUDA-backed tensor buffer.
#[derive(Clone)]
pub struct CudaBuffer {
    spec: TensorSpec,
    byte_len: usize,
    memory_space: CudaMemorySpace,
    host_visible: bool,
    platform: platform::PlatformBuffer,
}

impl fmt::Debug for CudaBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CudaBuffer")
            .field("spec", &self.spec)
            .field("byte_len", &self.byte_len)
            .field("memory_space", &self.memory_space)
            .field("host_visible", &self.host_visible)
            .field("platform", &"<cuda platform buffer>")
            .finish()
    }
}

impl CudaBuffer {
    /// Returns the backing allocation size in bytes.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// Returns the CUDA memory space backing the buffer.
    #[must_use]
    pub const fn memory_space(&self) -> CudaMemorySpace {
        self.memory_space
    }

    /// Returns whether the CPU can directly map the backing storage.
    #[must_use]
    pub const fn host_visible(&self) -> bool {
        self.host_visible
    }

    /// Returns a stable identity for the current device allocation.
    #[must_use]
    pub fn allocation_identity(&self) -> usize {
        self.platform.allocation_identity()
    }

    fn alias_with_spec(&self, spec: &TensorSpec) -> Result<Self, RuntimeError> {
        let expected_byte_len = spec
            .storage_size()
            .checked_mul(size_of_dtype(spec.dtype()))
            .ok_or_else(|| {
                RuntimeError::Backend(format!(
                    "cuda buffer alias size overflow for tensor storage size {}",
                    spec.storage_size()
                ))
            })?;
        if spec.dtype() != self.spec.dtype()
            || spec.storage_size() != self.spec.storage_size()
            || spec.device() != self.spec.device()
            || expected_byte_len != self.byte_len
        {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer alias requires identical dtype, storage size, device, and byte length: source={:?} alias={:?}",
                self.spec, spec
            )));
        }
        Ok(Self {
            spec: spec.clone(),
            byte_len: self.byte_len,
            memory_space: self.memory_space,
            host_visible: self.host_visible,
            platform: self.platform.clone(),
        })
    }

    fn contiguous_subbuffer_with_spec(
        &self,
        spec: &TensorSpec,
        byte_offset: usize,
    ) -> Result<Self, RuntimeError> {
        if spec.dtype() != self.spec.dtype() || spec.device() != self.spec.device() {
            return Err(RuntimeError::Backend(format!(
                "cuda sub-buffer requires identical dtype and device: source={:?} sub_buffer={:?}",
                self.spec, spec
            )));
        }
        if !spec.layout().is_contiguous() || spec.layout().offset() != 0 {
            return Err(RuntimeError::Backend(format!(
                "cuda sub-buffer requires a zero-offset contiguous spec, actual {:?}",
                spec.layout()
            )));
        }
        let byte_len = spec
            .storage_size()
            .checked_mul(size_of_dtype(spec.dtype()))
            .ok_or_else(|| {
                RuntimeError::Backend(format!(
                    "cuda sub-buffer size overflow for tensor storage size {}",
                    spec.storage_size()
                ))
            })?;
        if byte_offset.saturating_add(byte_len) > self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda sub-buffer exceeds allocation: offset={} len={} allocation={}",
                byte_offset, byte_len, self.byte_len
            )));
        }
        Ok(Self {
            spec: spec.clone(),
            byte_len,
            memory_space: self.memory_space,
            host_visible: self.host_visible,
            platform: self.platform.sub_buffer(byte_offset)?,
        })
    }

    /// Writes raw bytes into the CUDA buffer via an explicit host-to-device transfer.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        if bytes.len() != self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer write length mismatch: expected {}, actual {}",
                self.byte_len,
                bytes.len()
            )));
        }
        self.platform.write_bytes(bytes)
    }

    /// Writes raw bytes into a region of the CUDA buffer via an explicit host-to-device transfer.
    pub fn write_bytes_at_offset(
        &mut self,
        byte_offset: usize,
        bytes: &[u8],
    ) -> Result<(), RuntimeError> {
        if byte_offset.saturating_add(bytes.len()) > self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer region write exceeds allocation: offset={} len={} allocation={}",
                byte_offset,
                bytes.len(),
                self.byte_len
            )));
        }
        self.platform.write_bytes_at_offset(byte_offset, bytes)
    }

    /// Reads raw bytes from the CUDA buffer via an explicit device-to-host transfer.
    pub fn read_bytes(&self) -> Result<Vec<u8>, RuntimeError> {
        self.platform.read_bytes(self.byte_len)
    }

    /// Reads raw bytes from a region of the CUDA buffer via an explicit device-to-host transfer.
    pub fn read_bytes_at_offset(
        &self,
        byte_offset: usize,
        byte_len: usize,
    ) -> Result<Vec<u8>, RuntimeError> {
        if byte_offset.saturating_add(byte_len) > self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer region read exceeds allocation: offset={} len={} allocation={}",
                byte_offset, byte_len, self.byte_len
            )));
        }
        self.platform.read_bytes_at_offset(byte_offset, byte_len)
    }

    /// Writes contiguous `f32` values into an `f32` buffer.
    pub fn write_f32(&mut self, values: &[f32]) -> Result<(), RuntimeError> {
        if self.spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "write_f32 requires F32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        if values.len() != self.spec.storage_size() {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer write length mismatch: expected {} values, actual {}",
                self.spec.storage_size(),
                values.len()
            )));
        }
        let mut bytes = Vec::with_capacity(self.byte_len);
        for value in values {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        self.write_bytes(&bytes)
    }

    /// Writes contiguous `i32` values into an `i32` buffer.
    pub fn write_i32(&mut self, values: &[i32]) -> Result<(), RuntimeError> {
        if self.spec.dtype() != DType::I32 {
            return Err(RuntimeError::Backend(format!(
                "write_i32 requires I32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        if values.len() != self.spec.storage_size() {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer write length mismatch: expected {} values, actual {}",
                self.spec.storage_size(),
                values.len()
            )));
        }
        let mut bytes = Vec::with_capacity(self.byte_len);
        for value in values {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        self.write_bytes(&bytes)
    }

    /// Writes contiguous `f32` values into a region of an `f32` buffer.
    pub fn write_f32_at_offset(
        &mut self,
        element_offset: usize,
        values: &[f32],
    ) -> Result<(), RuntimeError> {
        if self.spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "write_f32_at_offset requires F32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        let expected_end = element_offset.saturating_add(values.len());
        if expected_end > self.spec.storage_size() {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer f32 region write exceeds allocation: end={} allocation={}",
                expected_end,
                self.spec.storage_size()
            )));
        }
        let mut bytes = Vec::with_capacity(values.len().saturating_mul(size_of_dtype(DType::F32)));
        for value in values {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        self.write_bytes_at_offset(
            element_offset.saturating_mul(size_of_dtype(DType::F32)),
            bytes.as_slice(),
        )
    }

    /// Writes contiguous `i32` values into a region of an `i32` buffer.
    pub fn write_i32_at_offset(
        &mut self,
        element_offset: usize,
        values: &[i32],
    ) -> Result<(), RuntimeError> {
        if self.spec.dtype() != DType::I32 {
            return Err(RuntimeError::Backend(format!(
                "write_i32_at_offset requires I32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        let expected_end = element_offset.saturating_add(values.len());
        if expected_end > self.spec.storage_size() {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer i32 region write exceeds allocation: end={} allocation={}",
                expected_end,
                self.spec.storage_size()
            )));
        }
        let mut bytes = Vec::with_capacity(values.len().saturating_mul(size_of::<i32>()));
        for value in values {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        self.write_bytes_at_offset(
            element_offset.saturating_mul(size_of::<i32>()),
            bytes.as_slice(),
        )
    }

    /// Writes contiguous `f32` values into a `bf16` buffer with explicit
    /// round-to-bf16 conversion on the host staging path.
    pub fn write_bf16_from_f32(&mut self, values: &[f32]) -> Result<(), RuntimeError> {
        if self.spec.dtype() != DType::BF16 {
            return Err(RuntimeError::Backend(format!(
                "write_bf16_from_f32 requires BF16 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        if values.len() != self.spec.storage_size() {
            return Err(RuntimeError::Backend(format!(
                "cuda bf16 buffer write length mismatch: expected {} values, actual {}",
                self.spec.storage_size(),
                values.len()
            )));
        }
        let mut bytes = Vec::with_capacity(self.byte_len);
        for value in values {
            bytes.extend_from_slice(&bf16::from_f32(*value).to_bits().to_ne_bytes());
        }
        self.write_bytes(bytes.as_slice())
    }

    /// Writes contiguous `f32` values into an `f16` buffer with explicit
    /// round-to-f16 conversion on the host staging path.
    pub fn write_f16_from_f32(&mut self, values: &[f32]) -> Result<(), RuntimeError> {
        if self.spec.dtype() != DType::F16 {
            return Err(RuntimeError::Backend(format!(
                "write_f16_from_f32 requires F16 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        if values.len() != self.spec.storage_size() {
            return Err(RuntimeError::Backend(format!(
                "cuda f16 buffer write length mismatch: expected {} values, actual {}",
                self.spec.storage_size(),
                values.len()
            )));
        }
        let mut bytes = Vec::with_capacity(self.byte_len);
        for value in values {
            bytes.extend_from_slice(&f16::from_f32(*value).to_bits().to_ne_bytes());
        }
        self.write_bytes(bytes.as_slice())
    }

    /// Writes contiguous prepacked `bf16` bit patterns into a `bf16` buffer.
    pub fn write_bf16_bits(&mut self, values: &[u16]) -> Result<(), RuntimeError> {
        if self.spec.dtype() != DType::BF16 {
            return Err(RuntimeError::Backend(format!(
                "write_bf16_bits requires BF16 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        if values.len() != self.spec.storage_size() {
            return Err(RuntimeError::Backend(format!(
                "cuda bf16 buffer write length mismatch: expected {} values, actual {}",
                self.spec.storage_size(),
                values.len()
            )));
        }
        let mut bytes = Vec::with_capacity(self.byte_len);
        for value in values {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        self.write_bytes(bytes.as_slice())
    }

    /// Reads contiguous `f32` values from an `f32` buffer.
    pub fn read_f32(&self) -> Result<Vec<f32>, RuntimeError> {
        if self.spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "read_f32 requires F32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        let bytes = self.read_bytes()?;
        let mut values = Vec::with_capacity(bytes.len() / size_of_dtype(self.spec.dtype()));
        for chunk in bytes.chunks_exact(size_of_dtype(self.spec.dtype())) {
            values.push(f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        Ok(values)
    }

    /// Reads contiguous `i32` values from an `i32` buffer.
    pub fn read_i32(&self) -> Result<Vec<i32>, RuntimeError> {
        if self.spec.dtype() != DType::I32 {
            return Err(RuntimeError::Backend(format!(
                "read_i32 requires I32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        let bytes = self.read_bytes()?;
        let mut values = Vec::with_capacity(bytes.len() / size_of_dtype(self.spec.dtype()));
        for chunk in bytes.chunks_exact(size_of_dtype(self.spec.dtype())) {
            values.push(i32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        Ok(values)
    }

    /// Reads contiguous `bf16` values and widens them to `f32`.
    pub fn read_bf16_to_f32(&self) -> Result<Vec<f32>, RuntimeError> {
        if self.spec.dtype() != DType::BF16 {
            return Err(RuntimeError::Backend(format!(
                "read_bf16_to_f32 requires BF16 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        let bytes = self.read_bytes()?;
        let mut values = Vec::with_capacity(bytes.len() / size_of_dtype(self.spec.dtype()));
        for chunk in bytes.chunks_exact(size_of_dtype(self.spec.dtype())) {
            values.push(bf16::from_bits(u16::from_ne_bytes([chunk[0], chunk[1]])).to_f32());
        }
        Ok(values)
    }

    /// Reads contiguous `f32` values from a region of an `f32` buffer.
    pub fn read_f32_at_offset(
        &self,
        element_offset: usize,
        element_count: usize,
    ) -> Result<Vec<f32>, RuntimeError> {
        if self.spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "read_f32_at_offset requires F32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        let expected_end = element_offset.saturating_add(element_count);
        if expected_end > self.spec.storage_size() {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer f32 region read exceeds allocation: end={} allocation={}",
                expected_end,
                self.spec.storage_size()
            )));
        }
        let bytes = self.read_bytes_at_offset(
            element_offset.saturating_mul(size_of_dtype(DType::F32)),
            element_count.saturating_mul(size_of_dtype(DType::F32)),
        )?;
        let mut values = Vec::with_capacity(element_count);
        for chunk in bytes.chunks_exact(size_of_dtype(DType::F32)) {
            values.push(f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        Ok(values)
    }
}

/// Page-locked host buffer for stream-owned CUDA staging transfers.
#[derive(Clone)]
pub struct CudaHostBuffer {
    byte_len: usize,
    platform: platform::PlatformHostBuffer,
}

impl fmt::Debug for CudaHostBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CudaHostBuffer")
            .field("byte_len", &self.byte_len)
            .field("platform", &"<cuda pinned host buffer>")
            .finish()
    }
}

impl CudaHostBuffer {
    /// Returns the allocation size in bytes.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// Writes raw bytes into the pinned host buffer.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        if bytes.len() != self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda host buffer write length mismatch: expected {}, actual {}",
                self.byte_len,
                bytes.len()
            )));
        }
        self.platform.write_bytes(bytes)
    }

    /// Reads raw bytes from the pinned host buffer.
    pub fn read_bytes(&self) -> Result<Vec<u8>, RuntimeError> {
        self.platform.read_bytes(self.byte_len)
    }

    /// Reads a byte prefix from the pinned host buffer into `output`.
    pub fn read_bytes_prefix_into(&self, output: &mut [u8]) -> Result<(), RuntimeError> {
        if output.len() > self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda host buffer prefix read exceeds allocation: requested {} bytes, allocation {}",
                output.len(),
                self.byte_len
            )));
        }
        self.platform.read_bytes_prefix_into(output)
    }

    /// Writes contiguous `f32` values into the pinned host buffer.
    pub fn write_f32(&mut self, values: &[f32]) -> Result<(), RuntimeError> {
        if values.len().saturating_mul(size_of_dtype(DType::F32)) != self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda host buffer f32 write length mismatch: expected {} bytes, actual {}",
                self.byte_len,
                values.len().saturating_mul(size_of_dtype(DType::F32))
            )));
        }
        let mut bytes = Vec::with_capacity(self.byte_len);
        for value in values {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        self.write_bytes(bytes.as_slice())
    }

    /// Writes contiguous `i32` values into the pinned host buffer.
    pub fn write_i32(&mut self, values: &[i32]) -> Result<(), RuntimeError> {
        if values.len().saturating_mul(size_of::<i32>()) != self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda host buffer i32 write length mismatch: expected {} bytes, actual {}",
                self.byte_len,
                values.len().saturating_mul(size_of::<i32>())
            )));
        }
        let mut bytes = Vec::with_capacity(self.byte_len);
        for value in values {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        self.write_bytes(bytes.as_slice())
    }

    /// Reads one `i32` from the pinned host buffer.
    pub fn read_i32(&self) -> Result<i32, RuntimeError> {
        if self.byte_len != size_of::<i32>() {
            return Err(RuntimeError::Backend(format!(
                "cuda host buffer i32 read requires {} bytes, actual {}",
                size_of::<i32>(),
                self.byte_len
            )));
        }
        let bytes = self.read_bytes()?;
        Ok(i32::from_ne_bytes(
            bytes[..size_of::<i32>()].try_into().map_err(|_| {
                RuntimeError::Backend(String::from("cuda host buffer returned invalid i32 bytes"))
            })?,
        ))
    }
}

impl BufferHandle for CudaBuffer {
    fn spec(&self) -> &TensorSpec {
        &self.spec
    }
}

/// CUDA stream submission that keeps fill/copy operations explicit.
#[derive(Clone)]
struct CudaSubmissionScratchBuffer {
    label: &'static str,
    buffer: CudaBuffer,
}

pub struct CudaSubmission {
    encoded_operations: usize,
    capturing: bool,
    retained_buffers: Vec<CudaBuffer>,
    scratch_buffers: Vec<CudaSubmissionScratchBuffer>,
    platform: platform::PlatformSubmission,
}

impl CudaSubmission {
    fn allocate_scratch_buffer(
        &mut self,
        label: &'static str,
        spec: &TensorSpec,
    ) -> Result<CudaBuffer, RuntimeError> {
        if let Some(buffer) = self
            .scratch_buffers
            .iter()
            .find(|entry| entry.label == label && entry.buffer.spec() == spec)
            .map(|entry| entry.buffer.clone())
        {
            return Ok(buffer);
        }

        let byte_len = spec
            .storage_size()
            .checked_mul(size_of_dtype(spec.dtype()))
            .ok_or_else(|| {
                RuntimeError::Backend(format!(
                    "cuda submission scratch allocation overflow for {label}"
                ))
            })?;
        let buffer = CudaBuffer {
            spec: spec.clone(),
            byte_len,
            memory_space: CudaMemorySpace::Device,
            host_visible: false,
            platform: self.platform.allocate(byte_len)?,
        };
        // One CUDA submission runs on one stream, so later ops can reuse a
        // scratch allocation after the earlier op on that stream has consumed
        // it.
        self.retained_buffers.push(buffer.clone());
        self.scratch_buffers.push(CudaSubmissionScratchBuffer {
            label,
            buffer: buffer.clone(),
        });
        Ok(buffer)
    }

    /// Fills a buffer with a constant byte value using an async CUDA memset.
    pub fn fill_buffer(&mut self, buffer: &CudaBuffer, value: u8) -> Result<(), RuntimeError> {
        self.platform
            .fill_buffer(&buffer.platform, buffer.byte_len, value)?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Copies one CUDA buffer into another with explicit size checking.
    pub fn copy_buffer(
        &mut self,
        source: &CudaBuffer,
        destination: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        if source.byte_len != destination.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer copy length mismatch: source {}, destination {}",
                source.byte_len, destination.byte_len
            )));
        }
        self.platform
            .copy_buffer(&source.platform, &destination.platform, source.byte_len)?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Copies a pinned host staging buffer into device memory on this stream.
    pub fn copy_host_to_device(
        &mut self,
        source: &CudaHostBuffer,
        destination: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        if source.byte_len != destination.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda host-to-device copy length mismatch: source {}, destination {}",
                source.byte_len, destination.byte_len
            )));
        }
        self.platform.copy_host_to_device(
            &source.platform,
            &destination.platform,
            destination.byte_len,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Copies device memory back into a pinned host staging buffer on this stream.
    pub fn copy_device_to_host(
        &mut self,
        source: &CudaBuffer,
        destination: &CudaHostBuffer,
    ) -> Result<(), RuntimeError> {
        if source.byte_len != destination.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda device-to-host copy length mismatch: source {}, destination {}",
                source.byte_len, destination.byte_len
            )));
        }
        self.platform.copy_device_to_host(
            &source.platform,
            &destination.platform,
            source.byte_len,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Copies a byte range between CUDA buffers.
    pub fn copy_buffer_region(
        &mut self,
        source: &CudaBuffer,
        source_byte_offset: usize,
        destination: &CudaBuffer,
        destination_byte_offset: usize,
        byte_len: usize,
    ) -> Result<(), RuntimeError> {
        if source_byte_offset.saturating_add(byte_len) > source.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda source region copy exceeds allocation: offset={} len={} allocation={}",
                source_byte_offset, byte_len, source.byte_len
            )));
        }
        if destination_byte_offset.saturating_add(byte_len) > destination.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda destination region copy exceeds allocation: offset={} len={} allocation={}",
                destination_byte_offset, byte_len, destination.byte_len
            )));
        }
        self.platform.copy_buffer_region(
            &source.platform,
            source_byte_offset,
            &destination.platform,
            destination_byte_offset,
            byte_len,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one quantized row-wise matrix-vector product.
    pub fn quantized_matvec(
        &mut self,
        weights: &CudaBuffer,
        byte_offset: usize,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input: &CudaBuffer,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_quantized_matvec(
            &weights.platform,
            byte_offset,
            mode,
            rows,
            cols,
            &input.platform,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Quantizes contiguous `f32` rows into GGML `Q8_1` scratch storage.
    pub fn quantize_f32_to_q8_1(
        &mut self,
        input: &CudaBuffer,
        rows: usize,
        cols: usize,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let required_bytes = ggml_q8_1_storage_bytes(rows, cols)?;
        if output.byte_len() < required_bytes {
            return Err(RuntimeError::Backend(format!(
                "cuda q8_1 scratch buffer too small: need {required_bytes} bytes for {rows}x{cols}, have {}",
                output.byte_len()
            )));
        }
        self.platform
            .encode_quantize_f32_to_q8_1(&input.platform, rows, cols, &output.platform)?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Dequantizes one GGML row directly into a device `f32` vector.
    pub fn dequantize_row_to_f32(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        rows: usize,
        row_stride: usize,
        cols: usize,
        decode_params: &CudaBuffer,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_dequantize_row_to_f32(
            &weights.platform,
            mode,
            rows,
            row_stride,
            cols,
            &decode_params.platform,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Gathers one dense row-major `f16` row directly into a device `f32`
    /// vector using the token id stored in the decode params buffer.
    pub fn gather_f16_row_to_f32(
        &mut self,
        input: &CudaBuffer,
        rows: usize,
        cols: usize,
        decode_params: &CudaBuffer,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_gather_f16_row_to_f32(
            &input.platform,
            rows,
            cols,
            &decode_params.platform,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one quantized row-wise matrix-vector product using a device
    /// `Q8_1` activation buffer.
    pub fn quantized_matvec_q8_1(
        &mut self,
        weights: &CudaBuffer,
        byte_offset: usize,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input_q8_1: &CudaBuffer,
        bias: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_quantized_matvec_q8_1(
            &weights.platform,
            byte_offset,
            mode,
            rows,
            cols,
            &input_q8_1.platform,
            bias.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one quantized row-wise matrix-vector product using a device
    /// `Q8_1` activation buffer and writes the greedy argmax directly.
    pub fn quantized_matvec_q8_1_argmax(
        &mut self,
        weights: &CudaBuffer,
        byte_offset: usize,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input_q8_1: &CudaBuffer,
        bias: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_quantized_matvec_q8_1_argmax(
            &weights.platform,
            byte_offset,
            mode,
            rows,
            cols,
            &input_q8_1.platform,
            bias.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one dense row-major matrix multiply using cuBLAS.
    pub fn matmul(
        &mut self,
        left: &CudaBuffer,
        right: &CudaBuffer,
        output: &CudaBuffer,
        rows: usize,
        inner: usize,
        cols: usize,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_matmul(
            &left.platform,
            &right.platform,
            &output.platform,
            rows,
            inner,
            cols,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one dense row-major matrix multiply with an `f16` rhs and
    /// `f32` accumulate/output.
    pub fn matmul_f16_to_f32(
        &mut self,
        left: &CudaBuffer,
        right: &CudaBuffer,
        output: &CudaBuffer,
        rows: usize,
        inner: usize,
        cols: usize,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_matmul_f16_to_f32(
            &left.platform,
            &right.platform,
            &output.platform,
            rows,
            inner,
            cols,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one dense row-major matrix multiply with `bf16` inputs and
    /// `f32` accumulate/output.
    pub fn matmul_bf16_to_f32(
        &mut self,
        left: &CudaBuffer,
        right: &CudaBuffer,
        output: &CudaBuffer,
        rows: usize,
        inner: usize,
        cols: usize,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_matmul_bf16_to_f32(
            &left.platform,
            &right.platform,
            &output.platform,
            rows,
            inner,
            cols,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one dense row-major matrix multiply with `f32` activations,
    /// `bf16` weights, and `f32` accumulate/output.
    pub fn matmul_f32_bf16_to_f32(
        &mut self,
        left: &CudaBuffer,
        right: &CudaBuffer,
        output: &CudaBuffer,
        rows: usize,
        inner: usize,
        cols: usize,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_matmul_f32_bf16_to_f32(
            &left.platform,
            &right.platform,
            &output.platform,
            rows,
            inner,
            cols,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one strided-batched dense row-major matrix multiply with
    /// `bf16` inputs and `f32` accumulate/output. Transpose flags are applied
    /// to the row-major logical matrices, not the underlying cuBLAS column-
    /// major views.
    pub fn matmul_bf16_strided_batched_to_f32(
        &mut self,
        left: &CudaBuffer,
        right: &CudaBuffer,
        output: &CudaBuffer,
        rows: usize,
        inner: usize,
        cols: usize,
        batch_count: usize,
        transpose_left: bool,
        transpose_right: bool,
    ) -> Result<(), RuntimeError> {
        let left_stride_elements = rows.checked_mul(inner).ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda bf16 strided batched matmul lhs stride overflow",
            ))
        })?;
        let right_stride_elements = inner.checked_mul(cols).ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda bf16 strided batched matmul rhs stride overflow",
            ))
        })?;
        let output_stride_elements = rows.checked_mul(cols).ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda bf16 strided batched matmul output stride overflow",
            ))
        })?;
        self.matmul_bf16_strided_batched_to_f32_with_strides(
            left,
            right,
            output,
            rows,
            inner,
            cols,
            batch_count,
            left_stride_elements,
            right_stride_elements,
            output_stride_elements,
            transpose_left,
            transpose_right,
        )
    }

    /// Launches one strided-batched dense row-major matrix multiply with
    /// `bf16` inputs and `f32` accumulate/output, using explicit element
    /// strides between adjacent batch entries.
    pub fn matmul_bf16_strided_batched_to_f32_with_strides(
        &mut self,
        left: &CudaBuffer,
        right: &CudaBuffer,
        output: &CudaBuffer,
        rows: usize,
        inner: usize,
        cols: usize,
        batch_count: usize,
        left_stride_elements: usize,
        right_stride_elements: usize,
        output_stride_elements: usize,
        transpose_left: bool,
        transpose_right: bool,
    ) -> Result<(), RuntimeError> {
        self.platform
            .encode_matmul_bf16_strided_batched_to_f32_with_strides(
                &left.platform,
                &right.platform,
                &output.platform,
                rows,
                inner,
                cols,
                batch_count,
                left_stride_elements,
                right_stride_elements,
                output_stride_elements,
                transpose_left,
                transpose_right,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one strided-batched dense row-major matrix multiply with
    /// `f32` activations, `bf16` weights, and `f32` accumulate/output.
    /// Transpose flags are applied to the row-major logical matrices.
    pub fn matmul_f32_bf16_strided_batched_to_f32(
        &mut self,
        left: &CudaBuffer,
        right: &CudaBuffer,
        output: &CudaBuffer,
        rows: usize,
        inner: usize,
        cols: usize,
        batch_count: usize,
        transpose_left: bool,
        transpose_right: bool,
    ) -> Result<(), RuntimeError> {
        let left_stride_elements = rows.checked_mul(inner).ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda mixed strided batched matmul lhs stride overflow",
            ))
        })?;
        let right_stride_elements = inner.checked_mul(cols).ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda mixed strided batched matmul rhs stride overflow",
            ))
        })?;
        let output_stride_elements = rows.checked_mul(cols).ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda mixed strided batched matmul output stride overflow",
            ))
        })?;
        self.matmul_f32_bf16_strided_batched_to_f32_with_strides(
            left,
            right,
            output,
            rows,
            inner,
            cols,
            batch_count,
            left_stride_elements,
            right_stride_elements,
            output_stride_elements,
            transpose_left,
            transpose_right,
        )
    }

    /// Launches one strided-batched dense row-major matrix multiply with
    /// `f32` activations, `bf16` weights, and `f32` accumulate/output, using
    /// explicit element strides between adjacent batch entries.
    pub fn matmul_f32_bf16_strided_batched_to_f32_with_strides(
        &mut self,
        left: &CudaBuffer,
        right: &CudaBuffer,
        output: &CudaBuffer,
        rows: usize,
        inner: usize,
        cols: usize,
        batch_count: usize,
        left_stride_elements: usize,
        right_stride_elements: usize,
        output_stride_elements: usize,
        transpose_left: bool,
        transpose_right: bool,
    ) -> Result<(), RuntimeError> {
        let matrix_left_elements = rows.checked_mul(inner).ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda mixed strided batched matmul lhs matrix size overflow",
            ))
        })?;
        let matrix_right_elements = inner.checked_mul(cols).ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda mixed strided batched matmul rhs matrix size overflow",
            ))
        })?;
        let matrix_output_elements = rows.checked_mul(cols).ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda mixed strided batched matmul output matrix size overflow",
            ))
        })?;
        let left_required = required_batched_elements(
            matrix_left_elements,
            batch_count,
            left_stride_elements,
            "lhs",
        )?;
        let right_required = required_batched_elements(
            matrix_right_elements,
            batch_count,
            right_stride_elements,
            "rhs",
        )?;
        let output_required = required_batched_elements(
            matrix_output_elements,
            batch_count,
            output_stride_elements,
            "output",
        )?;
        if left.spec.storage_size() < left_required {
            return Err(RuntimeError::Backend(format!(
                "cuda mixed strided batched matmul lhs storage too small: need {left_required} elements, actual {}",
                left.spec.storage_size()
            )));
        }
        if right.spec.storage_size() < right_required {
            return Err(RuntimeError::Backend(format!(
                "cuda mixed strided batched matmul rhs storage too small: need {right_required} elements, actual {}",
                right.spec.storage_size()
            )));
        }
        if output.spec.storage_size() < output_required {
            return Err(RuntimeError::Backend(format!(
                "cuda mixed strided batched matmul output storage too small: need {output_required} elements, actual {}",
                output.spec.storage_size()
            )));
        }

        let scratch_spec = TensorSpec::new(
            left.spec.shape().clone(),
            DType::BF16,
            left.spec.device().clone(),
        );
        let scratch = self.allocate_scratch_buffer(
            "mixed_f32_bf16_strided_batched_matmul_lhs_cast",
            &scratch_spec,
        )?;
        self.cast_f32_to_bf16(left, &scratch, left_required)?;
        self.matmul_bf16_strided_batched_to_f32_with_strides(
            &scratch,
            right,
            output,
            rows,
            inner,
            cols,
            batch_count,
            left_stride_elements,
            right_stride_elements,
            output_stride_elements,
            transpose_left,
            transpose_right,
        )?;
        Ok(())
    }

    /// Casts contiguous `f32` values to contiguous `f16` values on CUDA.
    pub fn cast_f32_to_f16(
        &mut self,
        input: &CudaBuffer,
        output: &CudaBuffer,
        element_count: usize,
    ) -> Result<(), RuntimeError> {
        self.platform
            .encode_cast_f32_to_f16(&input.platform, &output.platform, element_count)?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Casts contiguous `f32` values to contiguous `bf16` values on CUDA.
    pub fn cast_f32_to_bf16(
        &mut self,
        input: &CudaBuffer,
        output: &CudaBuffer,
        element_count: usize,
    ) -> Result<(), RuntimeError> {
        self.platform
            .encode_cast_f32_to_bf16(&input.platform, &output.platform, element_count)?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Casts contiguous `bf16` values to contiguous `f32` values on CUDA.
    pub fn cast_bf16_to_f32(
        &mut self,
        input: &CudaBuffer,
        output: &CudaBuffer,
        element_count: usize,
    ) -> Result<(), RuntimeError> {
        self.platform
            .encode_cast_bf16_to_f32(&input.platform, &output.platform, element_count)?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Reduces each contiguous `f32` row to its argmax index on CUDA.
    pub fn argmax_f32(
        &mut self,
        input: &CudaBuffer,
        row_count: usize,
        column_count: usize,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_argmax_f32(
            &input.platform,
            row_count,
            column_count,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Selects the bounded top-k values from each contiguous `f32` row on CUDA.
    pub fn top_k_f32(
        &mut self,
        input: &CudaBuffer,
        row_count: usize,
        column_count: usize,
        top_k: usize,
        selected_indices: &CudaBuffer,
        selected_values: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_top_k_f32(
            &input.platform,
            row_count,
            column_count,
            top_k,
            &selected_indices.platform,
            &selected_values.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Gathers one sparse set of `f32` values from a contiguous `f32` input row on CUDA.
    pub fn gather_f32_by_indices(
        &mut self,
        input: &CudaBuffer,
        input_len: usize,
        indices: &CudaBuffer,
        index_count: usize,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_gather_f32_by_indices(
            &input.platform,
            input_len,
            &indices.platform,
            index_count,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Selects the bounded top-k values from one contiguous `f32` row using multiple CUDA blocks.
    pub fn top_k_f32_one_row_partitioned(
        &mut self,
        input: &CudaBuffer,
        column_count: usize,
        top_k: usize,
        partial_block_count: usize,
        partial_indices: &CudaBuffer,
        partial_values: &CudaBuffer,
        selected_indices: &CudaBuffer,
        selected_values: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_top_k_f32_one_row_partitioned(
            &input.platform,
            column_count,
            top_k,
            partial_block_count,
            &partial_indices.platform,
            &partial_values.platform,
            &selected_indices.platform,
            &selected_values.platform,
        )?;
        self.encoded_operations += 3;
        Ok(())
    }

    /// Applies repeat, presence, and frequency penalties to sparse token ids on one `f32` logits row.
    pub fn apply_sampling_penalties_f32_sparse(
        &mut self,
        logits: &CudaBuffer,
        vocab_size: usize,
        token_ids: &CudaBuffer,
        token_counts: &CudaBuffer,
        active_token_count: usize,
        repeat_penalty: f32,
        presence_penalty: f32,
        frequency_penalty: f32,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_apply_sampling_penalties_f32_sparse(
            &logits.platform,
            vocab_size,
            &token_ids.platform,
            &token_counts.platform,
            active_token_count,
            repeat_penalty,
            presence_penalty,
            frequency_penalty,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Selects the bounded top-k values from one contiguous `f32` row using device radix sort.
    pub fn top_k_f32_one_row_radix_sort(
        &mut self,
        input: &CudaBuffer,
        column_count: usize,
        top_k: usize,
        input_indices: &CudaBuffer,
        sorted_values: &CudaBuffer,
        sorted_indices: &CudaBuffer,
        temp_storage: &CudaBuffer,
        temp_storage_bytes: usize,
        selected_indices: &CudaBuffer,
        selected_values: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_top_k_f32_one_row_radix_sort(
            &input.platform,
            column_count,
            top_k,
            &input_indices.platform,
            &sorted_values.platform,
            &sorted_indices.platform,
            &temp_storage.platform,
            temp_storage_bytes,
            &selected_indices.platform,
            &selected_values.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one RMSNorm kernel.
    pub fn rms_norm(
        &mut self,
        input: &CudaBuffer,
        weight: &CudaBuffer,
        output: &CudaBuffer,
        element_count: usize,
        epsilon: f32,
    ) -> Result<(), RuntimeError> {
        let feature_count = weight.spec().storage_size();
        if feature_count == 0 {
            return Err(RuntimeError::Backend(String::from(
                "cuda rms_norm requires a non-empty weight tensor",
            )));
        }
        if input.spec().storage_size() < element_count
            || output.spec().storage_size() < element_count
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda rms_norm requires input and output buffers large enough for element_count",
            )));
        }
        if element_count % feature_count != 0 {
            return Err(RuntimeError::Backend(String::from(
                "cuda rms_norm requires output size to be a multiple of the weight size",
            )));
        }
        self.platform.encode_rms_norm(
            &input.platform,
            &weight.platform,
            &output.platform,
            element_count,
            feature_count,
            epsilon,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one RMSNorm kernel on a source region and writes into a
    /// destination region.
    pub fn rms_norm_region(
        &mut self,
        input: &CudaBuffer,
        input_offset: usize,
        weight: &CudaBuffer,
        output: &CudaBuffer,
        output_offset: usize,
        element_count: usize,
        epsilon: f32,
    ) -> Result<(), RuntimeError> {
        let feature_count = weight.spec().storage_size();
        if feature_count == 0 {
            return Err(RuntimeError::Backend(String::from(
                "cuda rms_norm_region requires a non-empty weight tensor",
            )));
        }
        if input.spec().storage_size() < input_offset.saturating_add(element_count)
            || output.spec().storage_size() < output_offset.saturating_add(element_count)
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda rms_norm_region requires input and output buffers large enough for the requested region",
            )));
        }
        if element_count % feature_count != 0 {
            return Err(RuntimeError::Backend(String::from(
                "cuda rms_norm_region requires region size to be a multiple of the weight size",
            )));
        }
        self.platform.encode_rms_norm_region(
            &input.platform,
            input_offset,
            &weight.platform,
            &output.platform,
            output_offset,
            element_count,
            feature_count,
            epsilon,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies RMSNorm and quantizes the normalized output into GGML `Q8_1`.
    pub fn rms_norm_q8_1(
        &mut self,
        input: &CudaBuffer,
        weight: &CudaBuffer,
        output_q8_1: &CudaBuffer,
        element_count: usize,
        epsilon: f32,
    ) -> Result<(), RuntimeError> {
        let required_bytes = ggml_q8_1_storage_bytes(1, element_count)?;
        if output_q8_1.byte_len() < required_bytes {
            return Err(RuntimeError::Backend(format!(
                "cuda q8_1 norm buffer too small: need {required_bytes} bytes for 1x{element_count}, have {}",
                output_q8_1.byte_len()
            )));
        }
        self.platform.encode_rms_norm_q8_1(
            &input.platform,
            &weight.platform,
            &output_q8_1.platform,
            element_count,
            epsilon,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Adds a residual vector and applies RMSNorm in one CUDA kernel.
    pub fn add_residual_rms_norm(
        &mut self,
        input: &CudaBuffer,
        residual: &CudaBuffer,
        input_bias: Option<&CudaBuffer>,
        weight: &CudaBuffer,
        summed_output: &CudaBuffer,
        normalized_output: &CudaBuffer,
        element_count: usize,
        epsilon: f32,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_add_residual_rms_norm(
            &input.platform,
            &residual.platform,
            input_bias.map(|buffer| &buffer.platform),
            &weight.platform,
            &summed_output.platform,
            &normalized_output.platform,
            element_count,
            epsilon,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Adds a residual vector, applies RMSNorm, and quantizes the normalized
    /// output into GGML `Q8_1` in one CUDA kernel.
    pub fn add_residual_rms_norm_q8_1(
        &mut self,
        input: &CudaBuffer,
        residual: &CudaBuffer,
        input_bias: Option<&CudaBuffer>,
        weight: &CudaBuffer,
        summed_output: &CudaBuffer,
        normalized_output: &CudaBuffer,
        quantized_output: &CudaBuffer,
        element_count: usize,
        epsilon: f32,
    ) -> Result<(), RuntimeError> {
        let required_bytes = ggml_q8_1_storage_bytes(1, element_count)?;
        if quantized_output.byte_len() < required_bytes {
            return Err(RuntimeError::Backend(format!(
                "cuda q8_1 norm buffer too small: need {required_bytes} bytes for 1x{element_count}, have {}",
                quantized_output.byte_len()
            )));
        }
        self.platform.encode_add_residual_rms_norm_q8_1(
            &input.platform,
            &residual.platform,
            input_bias.map(|buffer| &buffer.platform),
            &weight.platform,
            &summed_output.platform,
            &normalized_output.platform,
            &quantized_output.platform,
            element_count,
            epsilon,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Adds a residual vector, applies RMSNorm, quantizes the normalized
    /// output into GGML `Q8_1`, and computes router top-k on the normalized
    /// vector in one CUDA kernel.
    #[allow(clippy::too_many_arguments)]
    pub fn add_residual_rms_norm_q8_1_router_topk(
        &mut self,
        input: &CudaBuffer,
        residual: &CudaBuffer,
        input_bias: Option<&CudaBuffer>,
        weight: &CudaBuffer,
        summed_output: &CudaBuffer,
        normalized_output: &CudaBuffer,
        quantized_output: &CudaBuffer,
        router_weights: &CudaBuffer,
        router_bias: Option<&CudaBuffer>,
        expert_count: usize,
        top_k: usize,
        selected_ids: &CudaBuffer,
        selected_weights: &CudaBuffer,
        element_count: usize,
        epsilon: f32,
    ) -> Result<(), RuntimeError> {
        let required_bytes = ggml_q8_1_storage_bytes(1, element_count)?;
        if quantized_output.byte_len() < required_bytes {
            return Err(RuntimeError::Backend(format!(
                "cuda q8_1 norm buffer too small: need {required_bytes} bytes for 1x{element_count}, have {}",
                quantized_output.byte_len()
            )));
        }
        self.platform
            .encode_add_residual_rms_norm_q8_1_router_topk(
                &input.platform,
                &residual.platform,
                input_bias.map(|buffer| &buffer.platform),
                &weight.platform,
                &summed_output.platform,
                &normalized_output.platform,
                &quantized_output.platform,
                &router_weights.platform,
                router_bias.map(|buffer| &buffer.platform),
                expert_count,
                top_k,
                &selected_ids.platform,
                &selected_weights.platform,
                element_count,
                epsilon,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Adds one dense vector into a region of a destination buffer in place.
    pub fn add_f32_in_place(
        &mut self,
        destination: &CudaBuffer,
        element_offset: usize,
        rhs: &CudaBuffer,
        element_count: usize,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_add_f32_in_place(
            &destination.platform,
            element_offset,
            &rhs.platform,
            element_count,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies SiLU to one source region and multiplies it elementwise by a
    /// second source region.
    pub fn silu_mul_f32(
        &mut self,
        activation_input: &CudaBuffer,
        activation_offset: usize,
        rhs: &CudaBuffer,
        rhs_offset: usize,
        element_count: usize,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_silu_mul_f32(
            &activation_input.platform,
            activation_offset,
            &rhs.platform,
            rhs_offset,
            element_count,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies SiLU to one source region, multiplies it elementwise by a
    /// second source region, and quantizes the result into GGML `Q8_1`.
    pub fn silu_mul_q8_1(
        &mut self,
        activation_input: &CudaBuffer,
        activation_offset: usize,
        rhs: &CudaBuffer,
        rhs_offset: usize,
        element_count: usize,
        output_q8_1: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let required_bytes = ggml_q8_1_storage_bytes(1, element_count)?;
        if output_q8_1.byte_len() < required_bytes {
            return Err(RuntimeError::Backend(format!(
                "cuda q8_1 activation buffer too small: need {required_bytes} bytes for 1x{element_count}, have {}",
                output_q8_1.byte_len()
            )));
        }
        self.platform.encode_silu_mul_q8_1(
            &activation_input.platform,
            activation_offset,
            &rhs.platform,
            rhs_offset,
            element_count,
            &output_q8_1.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies sigmoid to one source region and multiplies it elementwise by a
    /// second source region.
    pub fn sigmoid_mul_f32(
        &mut self,
        values: &CudaBuffer,
        values_offset: usize,
        gate: &CudaBuffer,
        gate_offset: usize,
        element_count: usize,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_sigmoid_mul_f32(
            &values.platform,
            values_offset,
            &gate.platform,
            gate_offset,
            element_count,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies sigmoid to one source region, multiplies it elementwise by a
    /// second source region, and quantizes the result into GGML `Q8_1`.
    pub fn sigmoid_mul_q8_1(
        &mut self,
        values: &CudaBuffer,
        values_offset: usize,
        gate: &CudaBuffer,
        gate_offset: usize,
        element_count: usize,
        output_q8_1: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let required_bytes = ggml_q8_1_storage_bytes(1, element_count)?;
        if output_q8_1.byte_len() < required_bytes {
            return Err(RuntimeError::Backend(format!(
                "cuda q8_1 activation buffer too small: need {required_bytes} bytes for 1x{element_count}, have {}",
                output_q8_1.byte_len()
            )));
        }
        self.platform.encode_sigmoid_mul_q8_1(
            &values.platform,
            values_offset,
            &gate.platform,
            gate_offset,
            element_count,
            &output_q8_1.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Splits qwen35-style per-head query/gate interleaving into two
    /// contiguous output vectors.
    pub fn split_interleaved_query_gate_f32(
        &mut self,
        input: &CudaBuffer,
        head_count: usize,
        head_dim: usize,
        query_output: &CudaBuffer,
        gate_output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_split_interleaved_query_gate_f32(
            &input.platform,
            head_count,
            head_dim,
            &query_output.platform,
            &gate_output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Splits qwen35-style per-head query/gate interleaving and RMS-normalizes
    /// the query output in one kernel.
    pub fn split_interleaved_query_gate_rms_norm_f32(
        &mut self,
        input: &CudaBuffer,
        head_count: usize,
        head_dim: usize,
        weight: &CudaBuffer,
        epsilon: f32,
        query_output: &CudaBuffer,
        gate_output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let element_count = head_count.saturating_mul(head_dim);
        if weight.spec().storage_size() != head_dim {
            return Err(RuntimeError::Backend(format!(
                "cuda qwen35 fused query/gate split rms norm requires per-head weight width {head_dim}, have {}",
                weight.spec().storage_size(),
            )));
        }
        if input.spec().storage_size() < element_count.saturating_mul(2)
            || query_output.spec().storage_size() < element_count
            || gate_output.spec().storage_size() < element_count
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda qwen35 fused query/gate split rms norm requires buffers large enough for the requested layout",
            )));
        }
        self.platform
            .encode_split_interleaved_query_gate_rms_norm_f32(
                &input.platform,
                head_count,
                head_dim,
                &weight.platform,
                epsilon,
                &query_output.platform,
                &gate_output.platform,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// RMS-normalizes qwen35 full-attention keys per KV head and copies values
    /// into the packed qkv output buffer in one kernel.
    #[allow(clippy::too_many_arguments)]
    pub fn pack_qwen35_key_value_rms_norm_f32(
        &mut self,
        input: &CudaBuffer,
        key_offset: usize,
        value_offset: usize,
        kv_head_count: usize,
        head_dim: usize,
        weight: &CudaBuffer,
        epsilon: f32,
        output: &CudaBuffer,
        output_key_offset: usize,
        output_value_offset: usize,
    ) -> Result<(), RuntimeError> {
        let kv_width = kv_head_count.saturating_mul(head_dim);
        if weight.spec().storage_size() != head_dim {
            return Err(RuntimeError::Backend(format!(
                "cuda qwen35 fused key/value pack rms norm requires per-head weight width {head_dim}, have {}",
                weight.spec().storage_size(),
            )));
        }
        if input.spec().storage_size() < value_offset.saturating_add(kv_width)
            || output.spec().storage_size() < output_value_offset.saturating_add(kv_width)
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda qwen35 fused key/value pack rms norm requires buffers large enough for the requested layout",
            )));
        }
        self.platform.encode_pack_qwen35_key_value_rms_norm_f32(
            &input.platform,
            key_offset,
            value_offset,
            kv_head_count,
            head_dim,
            &weight.platform,
            epsilon,
            &output.platform,
            output_key_offset,
            output_value_offset,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// RMS-normalizes qwen35 hybrid q and k regions per state group and copies
    /// the v region into the packed qkv output buffer in one kernel.
    #[allow(clippy::too_many_arguments)]
    pub fn pack_qwen35_hybrid_qkv_rms_norm_f32(
        &mut self,
        input: &CudaBuffer,
        q_offset: usize,
        k_offset: usize,
        v_offset: usize,
        group_count: usize,
        state_size: usize,
        v_size: usize,
        q_weight: &CudaBuffer,
        k_weight: &CudaBuffer,
        epsilon: f32,
        output: &CudaBuffer,
        output_q_offset: usize,
        output_k_offset: usize,
        output_v_offset: usize,
    ) -> Result<(), RuntimeError> {
        let q_size = group_count.saturating_mul(state_size);
        let k_size = q_size;
        if q_weight.spec().storage_size() != state_size {
            return Err(RuntimeError::Backend(format!(
                "cuda qwen35 fused hybrid q rms norm requires state width {state_size}, have {}",
                q_weight.spec().storage_size(),
            )));
        }
        if k_weight.spec().storage_size() != state_size {
            return Err(RuntimeError::Backend(format!(
                "cuda qwen35 fused hybrid k rms norm requires state width {state_size}, have {}",
                k_weight.spec().storage_size(),
            )));
        }
        if input.spec().storage_size() < v_offset.saturating_add(v_size)
            || output.spec().storage_size() < output_v_offset.saturating_add(v_size)
            || input.spec().storage_size() < q_offset.saturating_add(q_size)
            || output.spec().storage_size() < output_q_offset.saturating_add(q_size)
            || input.spec().storage_size() < k_offset.saturating_add(k_size)
            || output.spec().storage_size() < output_k_offset.saturating_add(k_size)
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda qwen35 fused hybrid qkv pack requires buffers large enough for the requested layout",
            )));
        }
        self.platform.encode_pack_qwen35_hybrid_qkv_rms_norm_f32(
            &input.platform,
            q_offset,
            k_offset,
            v_offset,
            group_count,
            state_size,
            v_size,
            &q_weight.platform,
            &k_weight.platform,
            epsilon,
            &output.platform,
            output_q_offset,
            output_k_offset,
            output_v_offset,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes one qwen35-style depthwise causal conv1d decode step.
    pub fn depthwise_causal_conv1d_step_f32(
        &mut self,
        input: &CudaBuffer,
        state: &CudaBuffer,
        weights: &CudaBuffer,
        channels: usize,
        kernel_size: usize,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_depthwise_causal_conv1d_step_f32(
            &input.platform,
            &state.platform,
            &weights.platform,
            channels,
            kernel_size,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes one qwen35-style depthwise causal conv1d decode step and
    /// applies SiLU to the result in one CUDA kernel.
    pub fn depthwise_causal_conv1d_step_silu_f32(
        &mut self,
        input: &CudaBuffer,
        state: &CudaBuffer,
        weights: &CudaBuffer,
        channels: usize,
        kernel_size: usize,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_depthwise_causal_conv1d_step_silu_f32(
            &input.platform,
            &state.platform,
            &weights.platform,
            channels,
            kernel_size,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Derives qwen35 SSM decay and beta vectors from projected alpha/beta
    /// logits and static SSM parameters.
    pub fn qwen35_ssm_decay_beta_f32(
        &mut self,
        input: &CudaBuffer,
        alpha_offset: usize,
        beta_offset: usize,
        ssm_a: &CudaBuffer,
        ssm_dt: &CudaBuffer,
        element_count: usize,
        decay_output: &CudaBuffer,
        beta_output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_qwen35_ssm_decay_beta_f32(
            &input.platform,
            alpha_offset,
            beta_offset,
            &ssm_a.platform,
            &ssm_dt.platform,
            element_count,
            &decay_output.platform,
            &beta_output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes one qwen35-style recurrent gated-delta decode step.
    #[allow(clippy::too_many_arguments)]
    pub fn gated_delta_step_f32(
        &mut self,
        qkv: &CudaBuffer,
        query_offset: usize,
        key_offset: usize,
        value_offset: usize,
        decay: &CudaBuffer,
        beta: &CudaBuffer,
        state: &CudaBuffer,
        key_head_count: usize,
        value_head_count: usize,
        key_dim: usize,
        value_dim: usize,
        v_head_reordered: bool,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_gated_delta_step_f32(
            &qkv.platform,
            query_offset,
            key_offset,
            value_offset,
            &decay.platform,
            &beta.platform,
            &state.platform,
            key_head_count,
            value_head_count,
            key_dim,
            value_dim,
            v_head_reordered,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies GPT-OSS NEOX-style RoPE in place to one buffer region.
    pub fn rope_neox_in_place(
        &mut self,
        values: &CudaBuffer,
        element_offset: usize,
        head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        position: usize,
        freq_scale: f32,
        ext_factor: f32,
        corr_dims: [f32; 2],
        theta_scale: f32,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_rope_neox_in_place(
            &values.platform,
            element_offset,
            head_count,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies the bounded NEOX-style RoPE backward transform to one rank-4
    /// gradient buffer on CUDA.
    pub fn rotary_embedding_backward(
        &mut self,
        grad_output: &CudaBuffer,
        cos: &CudaBuffer,
        sin: &CudaBuffer,
        batch_size: usize,
        head_count: usize,
        sequence_length: usize,
        head_dim: usize,
        batched_tables: bool,
        grad_input: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_rotary_embedding_backward(
            &grad_output.platform,
            &cos.platform,
            &sin.platform,
            batch_size,
            head_count,
            sequence_length,
            head_dim,
            batched_tables,
            &grad_input.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Permutes one rank-2 contiguous `f32` tensor through transpose on CUDA.
    pub fn permute_rank2_transpose_f32(
        &mut self,
        input: &CudaBuffer,
        output: &CudaBuffer,
        rows: usize,
        cols: usize,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_permute_rank2_transpose_f32(
            &input.platform,
            rows,
            cols,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Permutes one rank-4 contiguous `f32` tensor through the bounded
    /// decoder `[0, 2, 1, 3]` layout swap on CUDA.
    pub fn permute_rank4_swap_middle_axes_f32(
        &mut self,
        input: &CudaBuffer,
        output: &CudaBuffer,
        dim0: usize,
        dim1: usize,
        dim2: usize,
        dim3: usize,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_permute_rank4_swap_middle_axes_f32(
            &input.platform,
            dim0,
            dim1,
            dim2,
            dim3,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Reduces each contiguous `f32` row across its last axis on CUDA.
    pub fn reduce_sum_rows_f32(
        &mut self,
        input: &CudaBuffer,
        output: &CudaBuffer,
        row_count: usize,
        column_count: usize,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_reduce_sum_rows_f32(
            &input.platform,
            row_count,
            column_count,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Reduces a contiguous `f32` tensor across axis `0` on CUDA.
    pub fn reduce_sum_axis0_f32(
        &mut self,
        input: &CudaBuffer,
        output: &CudaBuffer,
        axis0_extent: usize,
        row_width: usize,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_reduce_sum_axis0_f32(
            &input.platform,
            axis0_extent,
            row_width,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Reduces a contiguous rank-3 `f32` tensor across axis `1` on CUDA.
    pub fn reduce_sum_axis1_rank3_f32(
        &mut self,
        input: &CudaBuffer,
        output: &CudaBuffer,
        dim0: usize,
        dim1: usize,
        dim2: usize,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_reduce_sum_axis1_rank3_f32(
            &input.platform,
            dim0,
            dim1,
            dim2,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Expands one rank-3 contiguous `f32` tensor through bounded broadcast on
    /// CUDA.
    pub fn expand_rank3_f32(
        &mut self,
        input: &CudaBuffer,
        output: &CudaBuffer,
        input_dims: [usize; 3],
        output_dims: [usize; 3],
    ) -> Result<(), RuntimeError> {
        self.platform.encode_expand_rank3_f32(
            &input.platform,
            input_dims,
            output_dims,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Expands one rank-4 contiguous `f32` tensor through bounded broadcast on
    /// CUDA.
    pub fn expand_rank4_f32(
        &mut self,
        input: &CudaBuffer,
        output: &CudaBuffer,
        input_dims: [usize; 4],
        output_dims: [usize; 4],
    ) -> Result<(), RuntimeError> {
        self.platform.encode_expand_rank4_f32(
            &input.platform,
            input_dims,
            output_dims,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies one bounded full-sequence causal grouped-query attention
    /// forward pass on contiguous `f32` rank-4 tensors.
    pub fn attention_causal_sequence_f32(
        &mut self,
        query: &CudaBuffer,
        key: &CudaBuffer,
        value: &CudaBuffer,
        batch_size: usize,
        head_count: usize,
        kv_head_count: usize,
        sequence_length: usize,
        head_dim: usize,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_attention_causal_sequence_f32(
            &query.platform,
            &key.platform,
            &value.platform,
            batch_size,
            head_count,
            kv_head_count,
            sequence_length,
            head_dim,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies one bounded full-sequence causal grouped-query attention
    /// forward pass on contiguous `bf16` rank-4 tensors.
    pub fn attention_causal_sequence_bf16(
        &mut self,
        query: &CudaBuffer,
        key: &CudaBuffer,
        value: &CudaBuffer,
        batch_size: usize,
        head_count: usize,
        kv_head_count: usize,
        sequence_length: usize,
        head_dim: usize,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_attention_causal_sequence_bf16(
            &query.platform,
            &key.platform,
            &value.platform,
            batch_size,
            head_count,
            kv_head_count,
            sequence_length,
            head_dim,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies one in-place causal row-softmax over contiguous `f32`
    /// `[row_count, sequence_length]` logits.
    pub fn attention_causal_row_softmax_in_place_f32(
        &mut self,
        logits: &CudaBuffer,
        row_count: usize,
        sequence_length: usize,
        scale: f32,
    ) -> Result<(), RuntimeError> {
        self.platform
            .encode_attention_causal_row_softmax_in_place_f32(
                &logits.platform,
                row_count,
                sequence_length,
                scale,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies one in-place causal row-softmax backward over contiguous `f32`
    /// probability and gradient matrices.
    pub fn attention_causal_row_softmax_backward_in_place_f32(
        &mut self,
        probabilities: &CudaBuffer,
        grad_probabilities: &CudaBuffer,
        row_count: usize,
        sequence_length: usize,
        post_scale: f32,
    ) -> Result<(), RuntimeError> {
        self.platform
            .encode_attention_causal_row_softmax_backward_in_place_f32(
                &probabilities.platform,
                &grad_probabilities.platform,
                row_count,
                sequence_length,
                post_scale,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies one bounded full-sequence causal grouped-query attention
    /// backward pass on contiguous `f32` rank-4 tensors and writes all three
    /// gradient surfaces on device.
    pub fn attention_causal_sequence_backward_f32(
        &mut self,
        query: &CudaBuffer,
        key: &CudaBuffer,
        value: &CudaBuffer,
        grad_output: &CudaBuffer,
        batch_size: usize,
        head_count: usize,
        kv_head_count: usize,
        sequence_length: usize,
        head_dim: usize,
        query_gradient: &CudaBuffer,
        key_gradient: &CudaBuffer,
        value_gradient: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform
            .encode_attention_causal_sequence_backward_f32(
                &query.platform,
                &key.platform,
                &value.platform,
                &grad_output.platform,
                batch_size,
                head_count,
                kv_head_count,
                sequence_length,
                head_dim,
                &query_gradient.platform,
                &key_gradient.platform,
                &value_gradient.platform,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies one bounded full-sequence causal grouped-query attention
    /// backward pass on contiguous `bf16` rank-4 tensors and writes all three
    /// gradient surfaces into explicit `f32` buffers on device.
    pub fn attention_causal_sequence_backward_bf16_to_f32(
        &mut self,
        query: &CudaBuffer,
        key: &CudaBuffer,
        value: &CudaBuffer,
        grad_output: &CudaBuffer,
        batch_size: usize,
        head_count: usize,
        kv_head_count: usize,
        sequence_length: usize,
        head_dim: usize,
        query_gradient: &CudaBuffer,
        key_gradient: &CudaBuffer,
        value_gradient: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform
            .encode_attention_causal_sequence_backward_bf16_to_f32(
                &query.platform,
                &key.platform,
                &value.platform,
                &grad_output.platform,
                batch_size,
                head_count,
                kv_head_count,
                sequence_length,
                head_dim,
                &query_gradient.platform,
                &key_gradient.platform,
                &value_gradient.platform,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies GPT-OSS NEOX-style RoPE, writes the current KV entry, and runs
    /// decode attention in one CUDA kernel.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_decode_rope_cache(
        &mut self,
        qkv: &CudaBuffer,
        query_offset: usize,
        key_offset: usize,
        value_offset: usize,
        cache_keys: &CudaBuffer,
        cache_values: &CudaBuffer,
        cache_width: usize,
        layer_offset: usize,
        past_tokens: usize,
        sliding_window: usize,
        head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        position: usize,
        freq_scale: f32,
        ext_factor: f32,
        corr_dims: [f32; 2],
        theta_scale: f32,
        attention_sinks: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_attention_decode_rope_cache(
            &qkv.platform,
            query_offset,
            key_offset,
            value_offset,
            &cache_keys.platform,
            &cache_values.platform,
            cache_width,
            layer_offset,
            past_tokens,
            sliding_window,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            attention_sinks.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Variant of fused GPT-OSS decode attention that keeps the device KV
    /// mirror in `f16`, closer to the llama.cpp CUDA path.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_decode_rope_cache_f16_kv(
        &mut self,
        qkv: &CudaBuffer,
        query_offset: usize,
        key_offset: usize,
        value_offset: usize,
        cache_keys: &CudaBuffer,
        cache_values: &CudaBuffer,
        cache_width: usize,
        layer_offset: usize,
        past_tokens: usize,
        sliding_window: usize,
        head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        position: usize,
        freq_scale: f32,
        ext_factor: f32,
        corr_dims: [f32; 2],
        theta_scale: f32,
        attention_sinks: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_attention_decode_rope_cache_f16_kv(
            &qkv.platform,
            query_offset,
            key_offset,
            value_offset,
            &cache_keys.platform,
            &cache_values.platform,
            cache_width,
            layer_offset,
            past_tokens,
            sliding_window,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            attention_sinks.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies three-axis MRoPE, writes the current F16 KV entry, and runs
    /// decode attention in one CUDA kernel.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_decode_mrope_cache_f16_kv(
        &mut self,
        qkv: &CudaBuffer,
        query_offset: usize,
        key_offset: usize,
        value_offset: usize,
        cache_keys: &CudaBuffer,
        cache_values: &CudaBuffer,
        cache_width: usize,
        layer_offset: usize,
        past_tokens: usize,
        sliding_window: usize,
        head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        positions: [usize; 3],
        sections: [usize; 4],
        interleaved: bool,
        freq_scale: f32,
        ext_factor: f32,
        corr_dims: [f32; 2],
        theta_scale: f32,
        attention_sinks: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_attention_decode_mrope_cache_f16_kv(
            &qkv.platform,
            query_offset,
            key_offset,
            value_offset,
            &cache_keys.platform,
            &cache_values.platform,
            cache_width,
            layer_offset,
            past_tokens,
            sliding_window,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            positions,
            sections,
            interleaved,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            attention_sinks.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Variant of fused GPT-OSS decode attention that reads one TurboQuant
    /// prototype KV cache encoded as contiguous GGML `Q8_1` rows.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_decode_rope_cache_turboquant_kv(
        &mut self,
        qkv: &CudaBuffer,
        query_offset: usize,
        key_offset: usize,
        value_offset: usize,
        cache_keys: &CudaBuffer,
        cache_values: &CudaBuffer,
        cache_width: usize,
        layer_offset: usize,
        past_tokens: usize,
        sliding_window: usize,
        head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        position: usize,
        freq_scale: f32,
        ext_factor: f32,
        corr_dims: [f32; 2],
        theta_scale: f32,
        attention_sinks: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        if cache_width % GGML_Q8_1_BLOCK_ELEMENTS != 0 {
            return Err(RuntimeError::Backend(format!(
                "cuda TurboQuant KV cache width must be divisible by {}, actual {cache_width}",
                GGML_Q8_1_BLOCK_ELEMENTS,
            )));
        }
        if layer_offset % GGML_Q8_1_BLOCK_ELEMENTS != 0 {
            return Err(RuntimeError::Backend(format!(
                "cuda TurboQuant KV layer offset must be divisible by {}, actual {layer_offset}",
                GGML_Q8_1_BLOCK_ELEMENTS,
            )));
        }
        if head_dim % GGML_Q8_1_BLOCK_ELEMENTS != 0 {
            return Err(RuntimeError::Backend(format!(
                "cuda TurboQuant KV head dim must be divisible by {}, actual {head_dim}",
                GGML_Q8_1_BLOCK_ELEMENTS,
            )));
        }
        self.platform
            .encode_attention_decode_rope_cache_turboquant_kv(
                &qkv.platform,
                query_offset,
                key_offset,
                value_offset,
                &cache_keys.platform,
                &cache_values.platform,
                cache_width,
                layer_offset,
                past_tokens,
                sliding_window,
                head_count,
                kv_head_count,
                head_dim,
                rotary_dim,
                position,
                freq_scale,
                ext_factor,
                corr_dims,
                theta_scale,
                attention_sinks.map(|buffer| &buffer.platform),
                &output.platform,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Variant of fused GPT-OSS decode attention that writes the attention
    /// output directly into contiguous GGML `Q8_1` blocks.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_decode_rope_cache_f16_kv_q8_1(
        &mut self,
        qkv: &CudaBuffer,
        query_offset: usize,
        key_offset: usize,
        value_offset: usize,
        cache_keys: &CudaBuffer,
        cache_values: &CudaBuffer,
        cache_width: usize,
        layer_offset: usize,
        past_tokens: usize,
        sliding_window: usize,
        head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        position: usize,
        freq_scale: f32,
        ext_factor: f32,
        corr_dims: [f32; 2],
        theta_scale: f32,
        attention_sinks: Option<&CudaBuffer>,
        output_q8_1: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let element_count = head_count.checked_mul(head_dim).ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda q8_1 attention output element count overflow",
            ))
        })?;
        let required_bytes = ggml_q8_1_storage_bytes(1, element_count)?;
        if output_q8_1.byte_len() < required_bytes {
            return Err(RuntimeError::Backend(format!(
                "cuda q8_1 attention output buffer too small: need {required_bytes} bytes for 1x{element_count}, have {}",
                output_q8_1.byte_len()
            )));
        }
        self.platform
            .encode_attention_decode_rope_cache_f16_kv_q8_1(
                &qkv.platform,
                query_offset,
                key_offset,
                value_offset,
                &cache_keys.platform,
                &cache_values.platform,
                cache_width,
                layer_offset,
                past_tokens,
                sliding_window,
                head_count,
                kv_head_count,
                head_dim,
                rotary_dim,
                position,
                freq_scale,
                ext_factor,
                corr_dims,
                theta_scale,
                attention_sinks.map(|buffer| &buffer.platform),
                &output_q8_1.platform,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Graph-capture-friendly variant of fused GPT-OSS decode attention that
    /// reads the dynamic `past_tokens` and `position` values from device memory.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_decode_rope_cache_f16_kv_graph(
        &mut self,
        qkv: &CudaBuffer,
        query_offset: usize,
        key_offset: usize,
        value_offset: usize,
        cache_keys: &CudaBuffer,
        cache_values: &CudaBuffer,
        cache_width: usize,
        layer_offset: usize,
        decode_params: &CudaBuffer,
        sliding_window: usize,
        head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        freq_scale: f32,
        ext_factor: f32,
        corr_dims: [f32; 2],
        theta_scale: f32,
        attention_sinks: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform
            .encode_attention_decode_rope_cache_f16_kv_graph(
                &qkv.platform,
                query_offset,
                key_offset,
                value_offset,
                &cache_keys.platform,
                &cache_values.platform,
                cache_width,
                layer_offset,
                &decode_params.platform,
                sliding_window,
                head_count,
                kv_head_count,
                head_dim,
                rotary_dim,
                freq_scale,
                ext_factor,
                corr_dims,
                theta_scale,
                attention_sinks.map(|buffer| &buffer.platform),
                &output.platform,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// FA3-class graph-capture-friendly decode attention with split-KV
    /// partial-workspace reduction and explicit final combine.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_decode_rope_cache_f16_kv_graph_fa3(
        &mut self,
        qkv: &CudaBuffer,
        query_offset: usize,
        key_offset: usize,
        value_offset: usize,
        cache_keys: &CudaBuffer,
        cache_values: &CudaBuffer,
        cache_width: usize,
        layer_offset: usize,
        decode_params: &CudaBuffer,
        sliding_window: usize,
        head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        freq_scale: f32,
        ext_factor: f32,
        corr_dims: [f32; 2],
        theta_scale: f32,
        attention_sinks: Option<&CudaBuffer>,
        partial_output: &CudaBuffer,
        partial_max: &CudaBuffer,
        partial_sum: &CudaBuffer,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let required_output_elements = ATTENTION_FA3_MAX_SPLITS
            .checked_mul(head_count)
            .and_then(|value| value.checked_mul(head_dim))
            .ok_or_else(|| {
                RuntimeError::Backend(String::from(
                    "cuda fa3 attention partial output element count overflow",
                ))
            })?;
        let required_head_elements = ATTENTION_FA3_MAX_SPLITS
            .checked_mul(head_count)
            .ok_or_else(|| {
                RuntimeError::Backend(String::from(
                    "cuda fa3 attention partial head element count overflow",
                ))
            })?;
        if partial_output.spec().dtype() != DType::F32
            || partial_output.spec().storage_size() < required_output_elements
        {
            return Err(RuntimeError::Backend(format!(
                "cuda fa3 attention partial output buffer too small: need at least {required_output_elements} f32 elements, have {} {:?}",
                partial_output.spec().storage_size(),
                partial_output.spec().dtype(),
            )));
        }
        if partial_max.spec().dtype() != DType::F32
            || partial_max.spec().storage_size() < required_head_elements
        {
            return Err(RuntimeError::Backend(format!(
                "cuda fa3 attention partial max buffer too small: need at least {required_head_elements} f32 elements, have {} {:?}",
                partial_max.spec().storage_size(),
                partial_max.spec().dtype(),
            )));
        }
        if partial_sum.spec().dtype() != DType::F32
            || partial_sum.spec().storage_size() < required_head_elements
        {
            return Err(RuntimeError::Backend(format!(
                "cuda fa3 attention partial sum buffer too small: need at least {required_head_elements} f32 elements, have {} {:?}",
                partial_sum.spec().storage_size(),
                partial_sum.spec().dtype(),
            )));
        }
        self.platform
            .encode_attention_decode_rope_cache_f16_kv_graph_fa3(
                &qkv.platform,
                query_offset,
                key_offset,
                value_offset,
                &cache_keys.platform,
                &cache_values.platform,
                cache_width,
                layer_offset,
                &decode_params.platform,
                sliding_window,
                head_count,
                kv_head_count,
                head_dim,
                rotary_dim,
                freq_scale,
                ext_factor,
                corr_dims,
                theta_scale,
                attention_sinks.map(|buffer| &buffer.platform),
                &partial_output.platform,
                &partial_max.platform,
                &partial_sum.platform,
                &output.platform,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Graph-capture-friendly variant of TurboQuant KV decode attention that
    /// reads the dynamic `past_tokens` and `position` values from device
    /// memory.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_decode_rope_cache_turboquant_kv_graph(
        &mut self,
        qkv: &CudaBuffer,
        query_offset: usize,
        key_offset: usize,
        value_offset: usize,
        cache_keys: &CudaBuffer,
        cache_values: &CudaBuffer,
        cache_width: usize,
        layer_offset: usize,
        decode_params: &CudaBuffer,
        sliding_window: usize,
        head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        freq_scale: f32,
        ext_factor: f32,
        corr_dims: [f32; 2],
        theta_scale: f32,
        attention_sinks: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        if cache_width % GGML_Q8_1_BLOCK_ELEMENTS != 0 {
            return Err(RuntimeError::Backend(format!(
                "cuda TurboQuant KV cache width must be divisible by {}, actual {cache_width}",
                GGML_Q8_1_BLOCK_ELEMENTS,
            )));
        }
        if layer_offset % GGML_Q8_1_BLOCK_ELEMENTS != 0 {
            return Err(RuntimeError::Backend(format!(
                "cuda TurboQuant KV layer offset must be divisible by {}, actual {layer_offset}",
                GGML_Q8_1_BLOCK_ELEMENTS,
            )));
        }
        if head_dim % GGML_Q8_1_BLOCK_ELEMENTS != 0 {
            return Err(RuntimeError::Backend(format!(
                "cuda TurboQuant KV head dim must be divisible by {}, actual {head_dim}",
                GGML_Q8_1_BLOCK_ELEMENTS,
            )));
        }
        self.platform
            .encode_attention_decode_rope_cache_turboquant_kv_graph(
                &qkv.platform,
                query_offset,
                key_offset,
                value_offset,
                &cache_keys.platform,
                &cache_values.platform,
                cache_width,
                layer_offset,
                &decode_params.platform,
                sliding_window,
                head_count,
                kv_head_count,
                head_dim,
                rotary_dim,
                freq_scale,
                ext_factor,
                corr_dims,
                theta_scale,
                attention_sinks.map(|buffer| &buffer.platform),
                &output.platform,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Graph-capture-friendly variant of fused GPT-OSS decode attention that
    /// writes the attention output directly into contiguous GGML `Q8_1` blocks.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_decode_rope_cache_f16_kv_graph_q8_1(
        &mut self,
        qkv: &CudaBuffer,
        query_offset: usize,
        key_offset: usize,
        value_offset: usize,
        cache_keys: &CudaBuffer,
        cache_values: &CudaBuffer,
        cache_width: usize,
        layer_offset: usize,
        decode_params: &CudaBuffer,
        sliding_window: usize,
        head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        freq_scale: f32,
        ext_factor: f32,
        corr_dims: [f32; 2],
        theta_scale: f32,
        attention_sinks: Option<&CudaBuffer>,
        output_q8_1: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let element_count = head_count.checked_mul(head_dim).ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda q8_1 attention output element count overflow",
            ))
        })?;
        let required_bytes = ggml_q8_1_storage_bytes(1, element_count)?;
        if output_q8_1.byte_len() < required_bytes {
            return Err(RuntimeError::Backend(format!(
                "cuda q8_1 attention output buffer too small: need {required_bytes} bytes for 1x{element_count}, have {}",
                output_q8_1.byte_len()
            )));
        }
        self.platform
            .encode_attention_decode_rope_cache_f16_kv_graph_q8_1(
                &qkv.platform,
                query_offset,
                key_offset,
                value_offset,
                &cache_keys.platform,
                &cache_values.platform,
                cache_width,
                layer_offset,
                &decode_params.platform,
                sliding_window,
                head_count,
                kv_head_count,
                head_dim,
                rotary_dim,
                freq_scale,
                ext_factor,
                corr_dims,
                theta_scale,
                attention_sinks.map(|buffer| &buffer.platform),
                &output_q8_1.platform,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes single-token GPT-OSS attention against a device-resident KV cache.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_decode(
        &mut self,
        query: &CudaBuffer,
        query_offset: usize,
        current_key: &CudaBuffer,
        key_offset: usize,
        current_value: &CudaBuffer,
        value_offset: usize,
        cache_keys: &CudaBuffer,
        cache_values: &CudaBuffer,
        cache_width: usize,
        layer_offset: usize,
        past_tokens: usize,
        sliding_window: usize,
        head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        attention_sinks: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_attention_decode(
            &query.platform,
            query_offset,
            &current_key.platform,
            key_offset,
            &current_value.platform,
            value_offset,
            &cache_keys.platform,
            &cache_values.platform,
            cache_width,
            layer_offset,
            past_tokens,
            sliding_window,
            head_count,
            kv_head_count,
            head_dim,
            attention_sinks.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Computes router logits on device, selects the top experts, and writes
    /// normalized routing weights for the selected set.
    pub fn router_topk_softmax(
        &mut self,
        weights: &CudaBuffer,
        bias: Option<&CudaBuffer>,
        input: &CudaBuffer,
        expert_count: usize,
        input_size: usize,
        top_k: usize,
        selected_ids: &CudaBuffer,
        selected_weights: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_router_topk_softmax(
            &weights.platform,
            bias.map(|buffer| &buffer.platform),
            &input.platform,
            expert_count,
            input_size,
            top_k,
            &selected_ids.platform,
            &selected_weights.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Selects the top experts from precomputed router logits and applies
    /// delayed softmax normalization across only the selected set.
    pub fn router_topk_delayed_softmax(
        &mut self,
        logits: &CudaBuffer,
        expert_count: usize,
        top_k: usize,
        selected_ids: &CudaBuffer,
        selected_weights: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_router_topk_delayed_softmax(
            &logits.platform,
            expert_count,
            top_k,
            &selected_ids.platform,
            &selected_weights.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes the packed GPT-OSS gate/up expert projection on device and
    /// writes the post-SwiGLU activations for the selected experts.
    #[allow(clippy::too_many_arguments)]
    pub fn moe_gate_up_swiglu(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        columns: usize,
        gate_rows: usize,
        up_rows: usize,
        selected_ids: &CudaBuffer,
        selected_count: usize,
        input: &CudaBuffer,
        gate_bias: Option<&CudaBuffer>,
        up_bias: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_moe_gate_up_swiglu(
            &weights.platform,
            mode,
            row_stride,
            rows_per_expert,
            columns,
            gate_rows,
            up_rows,
            &selected_ids.platform,
            selected_count,
            &input.platform,
            gate_bias.map(|buffer| &buffer.platform),
            up_bias.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes the packed GPT-OSS gate/up expert projection on device using a
    /// `Q8_1` activation buffer.
    #[allow(clippy::too_many_arguments)]
    pub fn moe_gate_up_swiglu_q8_1(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        columns: usize,
        gate_rows: usize,
        up_rows: usize,
        selected_ids: &CudaBuffer,
        selected_count: usize,
        input_q8_1: &CudaBuffer,
        gate_bias: Option<&CudaBuffer>,
        up_bias: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_moe_gate_up_swiglu_q8_1(
            &weights.platform,
            mode,
            row_stride,
            rows_per_expert,
            columns,
            gate_rows,
            up_rows,
            &selected_ids.platform,
            selected_count,
            &input_q8_1.platform,
            gate_bias.map(|buffer| &buffer.platform),
            up_bias.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes the selected4 GPT-OSS gate/up expert projection and writes the
    /// activated expert outputs directly as `Q8_1` blocks.
    #[allow(clippy::too_many_arguments)]
    pub fn moe_gate_up_swiglu_q8_1_selected4_quantized(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        columns: usize,
        gate_rows: usize,
        up_rows: usize,
        selected_ids: &CudaBuffer,
        selected_count: usize,
        input_q8_1: &CudaBuffer,
        gate_bias: Option<&CudaBuffer>,
        up_bias: Option<&CudaBuffer>,
        output_q8_1: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform
            .encode_moe_gate_up_swiglu_q8_1_selected4_quantized(
                &weights.platform,
                mode,
                row_stride,
                rows_per_expert,
                columns,
                gate_rows,
                up_rows,
                &selected_ids.platform,
                selected_count,
                &input_q8_1.platform,
                gate_bias.map(|buffer| &buffer.platform),
                up_bias.map(|buffer| &buffer.platform),
                &output_q8_1.platform,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes the ids-driven GPT-OSS gate/up expert path and writes the
    /// activated expert outputs directly as `Q8_1` blocks.
    #[allow(clippy::too_many_arguments)]
    pub fn expert_gate_up_swiglu_q8_1_ids(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        columns: usize,
        gate_rows: usize,
        up_rows: usize,
        selected_ids: &CudaBuffer,
        selected_count: usize,
        input_q8_1: &CudaBuffer,
        gate_bias: Option<&CudaBuffer>,
        up_bias: Option<&CudaBuffer>,
        output_q8_1: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.moe_gate_up_swiglu_q8_1_selected4_quantized(
            weights,
            mode,
            row_stride,
            rows_per_expert,
            columns,
            gate_rows,
            up_rows,
            selected_ids,
            selected_count,
            input_q8_1,
            gate_bias,
            up_bias,
            output_q8_1,
        )
    }

    /// Executes the GPT-OSS down projection and route-weighted expert
    /// aggregation on device for the selected experts.
    #[allow(clippy::too_many_arguments)]
    pub fn moe_down_aggregate(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows: usize,
        columns: usize,
        selected_ids: &CudaBuffer,
        selected_weights: &CudaBuffer,
        selected_count: usize,
        activated: &CudaBuffer,
        bias: Option<&CudaBuffer>,
        residual: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_moe_down_aggregate(
            &weights.platform,
            mode,
            row_stride,
            rows,
            columns,
            &selected_ids.platform,
            &selected_weights.platform,
            selected_count,
            &activated.platform,
            bias.map(|buffer| &buffer.platform),
            residual.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes the GPT-OSS down projection and route-weighted expert
    /// aggregation on device using row-wise `Q8_1` activations.
    #[allow(clippy::too_many_arguments)]
    pub fn moe_down_aggregate_q8_1(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows: usize,
        columns: usize,
        selected_ids: &CudaBuffer,
        selected_weights: &CudaBuffer,
        selected_count: usize,
        activated_q8_1: &CudaBuffer,
        bias: Option<&CudaBuffer>,
        residual: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_moe_down_aggregate_q8_1(
            &weights.platform,
            mode,
            row_stride,
            rows,
            columns,
            &selected_ids.platform,
            &selected_weights.platform,
            selected_count,
            &activated_q8_1.platform,
            bias.map(|buffer| &buffer.platform),
            residual.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes the GPT-OSS down projection and route-weighted expert
    /// aggregation on device by quantizing selected expert activations from
    /// `f32` into shared `Q8_1` blocks inside the selected-4 fast path.
    #[allow(clippy::too_many_arguments)]
    pub fn moe_down_aggregate_q8_1_f32(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows: usize,
        columns: usize,
        selected_ids: &CudaBuffer,
        selected_weights: &CudaBuffer,
        selected_count: usize,
        activated: &CudaBuffer,
        bias: Option<&CudaBuffer>,
        residual: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_moe_down_aggregate_q8_1_f32(
            &weights.platform,
            mode,
            row_stride,
            rows,
            columns,
            &selected_ids.platform,
            &selected_weights.platform,
            selected_count,
            &activated.platform,
            bias.map(|buffer| &buffer.platform),
            residual.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes the GPT-OSS selected4 down projection into expert-specific
    /// output rows, leaving route-weighted accumulation as a separate step.
    #[allow(clippy::too_many_arguments)]
    pub fn moe_down_project_q8_1_selected4(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows: usize,
        columns: usize,
        selected_ids: &CudaBuffer,
        selected_count: usize,
        activated_q8_1: &CudaBuffer,
        bias: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_moe_down_project_q8_1_selected4(
            &weights.platform,
            mode,
            row_stride,
            rows,
            columns,
            &selected_ids.platform,
            selected_count,
            &activated_q8_1.platform,
            bias.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes one ids-driven grouped expert projection from row-wise `Q8_1`
    /// activations into expert-specific `f32` output rows.
    #[allow(clippy::too_many_arguments)]
    pub fn expert_matvec_q8_1_ids(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows: usize,
        columns: usize,
        selected_ids: &CudaBuffer,
        selected_count: usize,
        input_q8_1: &CudaBuffer,
        bias: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.moe_down_project_q8_1_selected4(
            weights,
            mode,
            row_stride,
            rows,
            columns,
            selected_ids,
            selected_count,
            input_q8_1,
            bias,
            output,
        )
    }

    /// Accumulates up to four expert-specific output rows using route weights
    /// and an optional residual vector.
    pub fn accumulate_selected4(
        &mut self,
        input: &CudaBuffer,
        selected_weights: &CudaBuffer,
        selected_count: usize,
        rows: usize,
        residual: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_accumulate_selected4(
            &input.platform,
            &selected_weights.platform,
            selected_count,
            rows,
            residual.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Accumulates grouped expert output rows using route weights and an
    /// optional residual vector.
    pub fn accumulate_expert_outputs(
        &mut self,
        input: &CudaBuffer,
        selected_weights: &CudaBuffer,
        selected_count: usize,
        rows: usize,
        residual: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.accumulate_selected4(
            input,
            selected_weights,
            selected_count,
            rows,
            residual,
            output,
        )
    }

    /// Synchronizes the CUDA stream and returns explicit submission metadata.
    pub fn commit(self, wait: CudaCommandWait) -> Result<CudaSubmissionReport, RuntimeError> {
        if self.capturing {
            return Err(RuntimeError::Backend(String::from(
                "captured cuda submissions must use commit_captured",
            )));
        }
        let status = self.platform.commit(wait)?;
        Ok(CudaSubmissionReport {
            status,
            encoded_operations: self.encoded_operations,
        })
    }

    /// Finalizes stream capture, instantiates a reusable CUDA graph, launches it
    /// once for the current step, and returns the executable for later replays.
    pub fn commit_captured(
        self,
        wait: CudaCommandWait,
    ) -> Result<(CudaSubmissionReport, CudaGraphExec), RuntimeError> {
        if !self.capturing {
            return Err(RuntimeError::Backend(String::from(
                "ordinary cuda submissions must use commit",
            )));
        }
        let (status, graph_exec) = self.platform.commit_captured(wait)?;
        Ok((
            CudaSubmissionReport {
                status,
                encoded_operations: self.encoded_operations,
            },
            CudaGraphExec {
                encoded_operations: self.encoded_operations,
                platform: graph_exec,
            },
        ))
    }
}

enum CudaBackendState {
    Available(Box<AvailableCudaBackend>),
    Unavailable(RuntimeHealth),
}

struct AvailableCudaBackend {
    descriptor: DeviceDescriptor,
    platform: platform::ConfiguredBackend,
    execution_plan_cache: CudaExecutionPlanCache,
    kernel_cache: KernelCacheReport,
}

/// CUDA backend probe backed by `nvidia-smi` discovery plus `libcudart` buffer
/// and stream substrate.
pub struct CudaBackend {
    state: CudaBackendState,
}

impl Default for CudaBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CudaBackend {
    /// Creates a CUDA backend and selects the first discovered device when the
    /// CUDA runtime substrate is available.
    #[must_use]
    pub fn new() -> Self {
        match discovery_report_internal() {
            Ok(report) => {
                let Some(descriptor) = report.devices.first().cloned() else {
                    return Self {
                        state: CudaBackendState::Unavailable(report.health),
                    };
                };
                let allocator_pool = cuda_allocator_pool_policy();
                let kernel_cache = KernelCacheReport {
                    policy: KernelCachePolicy::disabled(),
                    state: KernelCacheState::default(),
                };
                match platform::configure_backend(descriptor.clone(), allocator_pool) {
                    Ok(platform) => Self {
                        state: CudaBackendState::Available(Box::new(AvailableCudaBackend {
                            descriptor,
                            platform,
                            execution_plan_cache: CudaExecutionPlanCache::new(
                                cuda_execution_plan_cache_policy(),
                            ),
                            kernel_cache,
                        })),
                    },
                    Err(error) => Self {
                        state: CudaBackendState::Unavailable(RuntimeHealth {
                            status: report.health.status,
                            message: format!("cuda runtime substrate unavailable: {error}"),
                        }),
                    },
                }
            }
            Err(error) => Self {
                state: CudaBackendState::Unavailable(RuntimeHealth {
                    status: HealthStatus::Offline,
                    message: error.to_string(),
                }),
            },
        }
    }

    /// Returns the backend-local NVIDIA discovery report.
    pub fn discovery_report(&self) -> Result<NvidiaBackendReport, RuntimeError> {
        discovery_report_internal()
    }

    /// Returns the device selected for allocation/submission, when available.
    #[must_use]
    pub fn selected_device(&self) -> Option<&DeviceDescriptor> {
        match &self.state {
            CudaBackendState::Available(backend) => Some(&backend.descriptor),
            CudaBackendState::Unavailable(_) => None,
        }
    }

    /// Returns explicit runtime resource posture for the selected CUDA device.
    #[must_use]
    pub fn runtime_resources(&self) -> Option<BackendRuntimeResources> {
        match &self.state {
            CudaBackendState::Available(backend) => {
                let allocator_pool = backend.platform.allocator_pool_report();
                Some(BackendRuntimeResources {
                    execution_plan_cache: backend.execution_plan_cache.report(),
                    allocator_pool: allocator_pool.clone(),
                    kernel_cache: backend.kernel_cache.clone(),
                    device_memory_budget: Some(DeviceMemoryBudget::new(
                        backend.descriptor.memory_capacity_bytes,
                        allocator_pool.policy.max_cached_bytes,
                        backend
                            .kernel_cache
                            .policy
                            .max_cached_bytes
                            .unwrap_or(backend.kernel_cache.state.cached_bytes),
                    )),
                })
            }
            CudaBackendState::Unavailable(_) => None,
        }
    }

    /// Returns explicit allocator-pool telemetry for the selected CUDA device.
    #[must_use]
    pub fn allocator_pool_telemetry(&self) -> Option<CudaAllocatorPoolTelemetry> {
        match &self.state {
            CudaBackendState::Available(backend) => {
                Some(backend.platform.allocator_pool_telemetry())
            }
            CudaBackendState::Unavailable(_) => None,
        }
    }

    /// Returns live free and total memory for the selected CUDA runtime device.
    pub fn device_memory_info(&self) -> Result<Option<CudaDeviceMemoryInfo>, RuntimeError> {
        match &self.state {
            CudaBackendState::Available(backend) => backend.platform.device_memory_info().map(Some),
            CudaBackendState::Unavailable(_) => Ok(None),
        }
    }

    /// Returns explicit cuBLASLt GEMM tuning evidence for the selected CUDA device.
    #[must_use]
    pub fn cuda_gemm_tuning_report(&self) -> Option<CudaGemmTuningReport> {
        match &self.state {
            CudaBackendState::Available(backend) => {
                Some(backend.platform.cuda_gemm_tuning_report())
            }
            CudaBackendState::Unavailable(_) => None,
        }
    }

    /// Returns whether the bounded public causal scaled-dot-product-attention
    /// lane is available on the selected CUDA device.
    #[must_use]
    pub fn supports_bounded_scaled_dot_product_attention(&self) -> bool {
        self.selected_device().is_some()
    }

    /// Returns an explicit refusal reason for the bounded public causal
    /// scaled-dot-product-attention lane when CUDA is unavailable.
    #[must_use]
    pub fn bounded_scaled_dot_product_attention_refusal_reason(&self) -> Option<String> {
        (!self.supports_bounded_scaled_dot_product_attention()).then(|| self.health().message)
    }

    /// Registers one transposed-f16 weight buffer with an explicit serving scope
    /// so the runtime can use scope-keyed cuBLASLt plans instead of a generic
    /// shape-only fallback.
    pub fn register_cublaslt_weight_scope(&self, weight: &CudaBuffer, scope: CudaGemmTuningScope) {
        if let CudaBackendState::Available(backend) = &self.state {
            backend
                .platform
                .register_cublaslt_weight_scope(weight.allocation_identity(), scope);
        }
    }

    /// Bounded startup tuning for one hot `f16 -> f32` GEMM weight on the
    /// admitted CUDA serving lane.
    pub fn autotune_cublaslt_f16_to_f32_weight_rows(
        &mut self,
        weight: &CudaBuffer,
        scope: CudaGemmTuningScope,
        inner: usize,
        cols: usize,
        row_ladder: &[usize],
    ) -> Result<(), RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Ok(());
        };
        self.register_cublaslt_weight_scope(weight, scope);
        let mut unique_rows = BTreeSet::new();
        for rows in row_ladder.iter().copied() {
            if rows == 0 || rows > CUDA_CUBLASLT_F16_TO_F32_MAX_ROWS {
                continue;
            }
            unique_rows.insert(rows);
        }
        for rows in unique_rows {
            let left_elements = rows.checked_mul(inner).ok_or_else(|| {
                RuntimeError::Backend(String::from(
                    "cuda cuBLASLt startup tune left-buffer size overflow",
                ))
            })?;
            let left_spec =
                TensorSpec::new(Shape::new(vec![left_elements]), DType::F16, device.clone());
            let mut left = self.allocate(&left_spec)?;
            let left_values = (0..left_elements)
                .map(|index| (((index % 17) as f32) - 8.0) * 0.0625)
                .collect::<Vec<_>>();
            left.write_f16_from_f32(left_values.as_slice())?;
            let output = self.f32_buffer(rows.checked_mul(cols).ok_or_else(|| {
                RuntimeError::Backend(String::from(
                    "cuda cuBLASLt startup tune output-buffer size overflow",
                ))
            })?)?;
            let mut submission = self.begin_submission()?;
            submission.matmul_f16_to_f32(&left, weight, &output, rows, inner, cols)?;
            submission.commit(CudaCommandWait::Completed)?;
        }
        Ok(())
    }

    /// Creates a dense `f32` input buffer on the selected CUDA device.
    pub fn input_buffer(
        &mut self,
        shape: Shape,
        values: impl Into<Vec<f32>>,
    ) -> Result<CudaBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "cuda backend unavailable: no selected execution device",
            )));
        };
        let mut buffer = self.allocate(&TensorSpec::new(shape, DType::F32, device))?;
        buffer.write_f32(values.into().as_slice())?;
        Ok(buffer)
    }

    /// Creates a dense `bf16` input buffer on the selected CUDA device.
    pub fn input_bf16_buffer(
        &mut self,
        shape: Shape,
        values: impl Into<Vec<f32>>,
    ) -> Result<CudaBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "cuda backend unavailable: no selected execution device",
            )));
        };
        let mut buffer = self.allocate(&TensorSpec::new(shape, DType::BF16, device))?;
        buffer.write_bf16_from_f32(values.into().as_slice())?;
        Ok(buffer)
    }

    /// Creates a dense `i32` input buffer on the selected CUDA device.
    pub fn input_i32_buffer(
        &mut self,
        shape: Shape,
        values: impl Into<Vec<i32>>,
    ) -> Result<CudaBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "cuda backend unavailable: no selected execution device",
            )));
        };
        let mut buffer = self.allocate(&TensorSpec::new(shape, DType::I32, device))?;
        buffer.write_i32(values.into().as_slice())?;
        Ok(buffer)
    }

    /// Allocates an uninitialized dense `f32` buffer on the selected CUDA device.
    pub fn f32_buffer(&mut self, len: usize) -> Result<CudaBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "cuda backend unavailable: no selected execution device",
            )));
        };
        self.allocate_buffer(&TensorSpec::new(Shape::new(vec![len]), DType::F32, device))
    }

    /// Allocates an uninitialized dense `f16` buffer on the selected CUDA device.
    pub fn f16_buffer(&mut self, len: usize) -> Result<CudaBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "cuda backend unavailable: no selected execution device",
            )));
        };
        self.allocate_buffer(&TensorSpec::new(Shape::new(vec![len]), DType::F16, device))
    }

    /// Allocates an uninitialized dense `bf16` buffer on the selected CUDA device.
    pub fn bf16_buffer(&mut self, len: usize) -> Result<CudaBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "cuda backend unavailable: no selected execution device",
            )));
        };
        self.allocate_buffer(&TensorSpec::new(Shape::new(vec![len]), DType::BF16, device))
    }

    /// Allocates an uninitialized dense `i32` buffer on the selected CUDA device.
    pub fn i32_buffer(&mut self, len: usize) -> Result<CudaBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "cuda backend unavailable: no selected execution device",
            )));
        };
        self.allocate_buffer(&TensorSpec::new(Shape::new(vec![len]), DType::I32, device))
    }

    /// Allocates a page-locked host buffer for stream-owned staging transfers.
    pub fn host_buffer(&mut self, byte_len: usize) -> Result<CudaHostBuffer, RuntimeError> {
        let Some(backend) = self.selected_backend() else {
            return Err(RuntimeError::Backend(self.health().message));
        };
        Ok(CudaHostBuffer {
            byte_len,
            platform: backend.platform.allocate_host(byte_len)?,
        })
    }

    /// Returns the temporary device-storage requirement for one-row radix-sort top-k selection.
    pub fn top_k_f32_one_row_radix_sort_temp_storage_bytes(
        &self,
        column_count: usize,
    ) -> Result<usize, RuntimeError> {
        let Some(backend) = self.selected_backend() else {
            return Err(RuntimeError::Backend(self.health().message));
        };
        backend
            .platform
            .top_k_f32_one_row_radix_sort_temp_storage_bytes(column_count)
    }

    /// Returns whether the Psionic-owned CUDA quantized text-generation kernels are built.
    #[must_use]
    pub fn quantized_kernels_available(&self) -> bool {
        platform::quantized_kernels_compiled()
    }

    /// Uploads raw bytes into device memory for backend-owned quantized storage.
    pub fn byte_buffer(&mut self, bytes: &[u8]) -> Result<CudaBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "cuda backend unavailable: no selected execution device",
            )));
        };
        let spec = TensorSpec::new(Shape::new(vec![bytes.len()]), DType::I8, device);
        let Some(backend) = self.selected_backend() else {
            return Err(RuntimeError::Backend(self.health().message));
        };
        let mut buffer = backend.allocate(&spec)?;
        buffer.write_bytes(bytes)?;
        Ok(buffer)
    }

    /// Executes one quantized row-wise matrix-vector product on CUDA.
    pub fn quantized_matvec(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input: &[f32],
    ) -> Result<Vec<f32>, RuntimeError> {
        Ok(self
            .quantized_matvec_profiled(weights, mode, rows, cols, input)?
            .values)
    }

    /// Executes one profiled quantized row-wise matrix-vector product on CUDA.
    pub fn quantized_matvec_profiled(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input: &[f32],
    ) -> Result<CudaQuantizedMatvecResult, RuntimeError> {
        self.quantized_matvec_with_offset_profiled(weights, 0, mode, rows, cols, input)
    }

    /// Executes one quantized row-wise matrix-vector product from a byte offset.
    pub fn quantized_matvec_with_offset(
        &mut self,
        weights: &CudaBuffer,
        byte_offset: usize,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input: &[f32],
    ) -> Result<Vec<f32>, RuntimeError> {
        Ok(self
            .quantized_matvec_with_offset_profiled(weights, byte_offset, mode, rows, cols, input)?
            .values)
    }

    /// Executes one profiled quantized row-wise matrix-vector product from a byte offset.
    pub fn quantized_matvec_with_offset_profiled(
        &mut self,
        weights: &CudaBuffer,
        byte_offset: usize,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input: &[f32],
    ) -> Result<CudaQuantizedMatvecResult, RuntimeError> {
        if !self.quantized_kernels_available() {
            return Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels are not available in this build",
            )));
        }
        if weights.spec().dtype() != DType::I8 {
            return Err(RuntimeError::Backend(format!(
                "cuda quantized matvec requires raw byte weights, actual {:?}",
                weights.spec().dtype()
            )));
        }
        let Some((elements_per_block, bytes_per_block)) = mode.ggml_block_spec() else {
            return Err(RuntimeError::Backend(format!(
                "cuda quantized matvec does not support mode {mode:?}"
            )));
        };
        if cols == 0 || cols % elements_per_block != 0 {
            return Err(RuntimeError::Backend(format!(
                "cuda quantized matvec requires block-aligned width {cols} for {mode:?}"
            )));
        }
        if input.len() != cols {
            return Err(RuntimeError::Backend(format!(
                "cuda quantized matvec input width mismatch: expected {cols}, actual {}",
                input.len()
            )));
        }
        let row_stride = (cols / elements_per_block) * bytes_per_block;
        let required_bytes = rows.saturating_mul(row_stride);
        let expected_end = byte_offset.saturating_add(required_bytes);
        if weights.byte_len() < expected_end {
            return Err(RuntimeError::Backend(format!(
                "cuda quantized matvec byte length mismatch: required {expected_end}, actual {}",
                weights.byte_len(),
            )));
        }

        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "cuda backend unavailable: no selected execution device",
            )));
        };
        let input_buffer = self.input_buffer(Shape::new(vec![cols]), input.to_vec())?;
        let output =
            self.allocate_buffer(&TensorSpec::new(Shape::new(vec![rows]), DType::F32, device))?;
        let mut submission = self.begin_submission()?;
        submission.quantized_matvec(
            weights,
            byte_offset,
            mode,
            rows,
            cols,
            &input_buffer,
            &output,
        )?;
        let _ = submission.commit(CudaCommandWait::Completed)?;
        let values = output.read_f32()?;
        Ok(CudaQuantizedMatvecResult {
            values,
            stats: CudaQuantizedMatvecStats {
                host_to_device_bytes: (input.len())
                    .saturating_mul(size_of_dtype(DType::F32))
                    .try_into()
                    .unwrap_or(u64::MAX),
                device_to_host_bytes: (rows)
                    .saturating_mul(size_of_dtype(DType::F32))
                    .try_into()
                    .unwrap_or(u64::MAX),
                submission_count: 1,
                sync_count: 1,
                kernel_launches: 1,
            },
        })
    }

    /// Compiles and executes a graph on the supported dense CUDA surface.
    pub fn compile_and_execute(
        &mut self,
        graph: &Graph,
        inputs: &BTreeMap<TensorId, CudaBuffer>,
    ) -> Result<ExecutionResult<CudaBuffer>, RuntimeError> {
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(self.health().message));
        };
        let (plan, plan_digest, compile_path) = backend.lookup_or_compile(graph)?;
        let mut result = backend.execute(&plan, inputs)?;
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

    /// Returns truthful backend-selection data for a supported CUDA product path.
    pub fn backend_selection(
        &self,
        supported_ops: &[&str],
    ) -> Result<BackendSelection, RuntimeError> {
        let policy = ServedProductBackendPolicy::fallback_to_compatible_backend(
            BackendDegradedPolicy::AllowSameBackend,
        );
        match &self.state {
            CudaBackendState::Available(backend) => {
                let supported_ops = supported_ops
                    .iter()
                    .map(|label| String::from(*label))
                    .collect();
                let health = self.health();
                match health.status {
                    HealthStatus::Ready => Ok(BackendSelection::direct_with_policy(
                        self.backend_name(),
                        Some(backend.descriptor.clone()),
                        supported_ops,
                        policy,
                    )
                    .with_runtime_resources(self.runtime_resources())
                    .with_backend_extensions(self.extension_support())),
                    HealthStatus::Degraded => Ok(BackendSelection::degraded(
                        self.backend_name(),
                        Some(backend.descriptor.clone()),
                        supported_ops,
                        policy,
                        health.message,
                    )
                    .with_runtime_resources(self.runtime_resources())
                    .with_backend_extensions(self.extension_support())),
                    HealthStatus::Offline => Err(RuntimeError::Backend(format!(
                        "cuda backend unavailable: {}",
                        health.message
                    ))),
                }
            }
            CudaBackendState::Unavailable(health) => Err(RuntimeError::Backend(format!(
                "cuda backend unavailable: {}",
                health.message
            ))),
        }
    }

    /// Returns an explicit fallback selection when CUDA cannot execute the
    /// requested product path on the local machine.
    pub fn fallback_selection<B>(
        &self,
        fallback_backend: &B,
        supported_ops: &[&str],
    ) -> Result<BackendSelection, RuntimeError>
    where
        B: DeviceDiscovery + ?Sized,
    {
        let policy = ServedProductBackendPolicy::fallback_to_compatible_backend(
            BackendDegradedPolicy::AllowSameBackend,
        );
        match &self.state {
            CudaBackendState::Available(_) => self.backend_selection(supported_ops),
            CudaBackendState::Unavailable(health) => Ok(BackendSelection::fallback_with_policy(
                self.backend_name(),
                fallback_backend.backend_name(),
                fallback_backend.discover_devices()?.into_iter().next(),
                supported_ops
                    .iter()
                    .map(|label| String::from(*label))
                    .collect(),
                policy,
                format!("cuda backend unavailable: {}", health.message),
            )
            .with_runtime_resources(fallback_backend.runtime_resources())
            .with_backend_extensions(fallback_backend.extension_support())),
        }
    }

    /// Allocates a CUDA buffer for the provided tensor specification.
    pub fn allocate_buffer(&mut self, spec: &TensorSpec) -> Result<CudaBuffer, RuntimeError> {
        let Some(backend) = self.selected_backend() else {
            let message = match &self.state {
                CudaBackendState::Unavailable(health) => health.message.clone(),
                CudaBackendState::Available(_) => String::from("cuda backend unavailable"),
            };
            return Err(RuntimeError::Backend(message));
        };
        if spec.dtype() != DType::F32
            && spec.dtype() != DType::F16
            && spec.dtype() != DType::BF16
            && spec.dtype() != DType::I32
        {
            return Err(RuntimeError::Backend(format!(
                "cuda dense surface only supports F32/F16/BF16/I32 buffers, actual {:?}",
                spec.dtype()
            )));
        }
        if spec.device().kind() != DeviceKind::Cuda {
            return Err(RuntimeError::Backend(format!(
                "cuda allocator requires a CUDA tensor spec, actual device kind {}",
                spec.device().kind()
            )));
        }
        if spec.device().ordinal() != backend.descriptor.device.ordinal() {
            return Err(RuntimeError::Backend(format!(
                "cuda allocator requires device ordinal {}, actual {}",
                backend.descriptor.device.ordinal(),
                spec.device().ordinal()
            )));
        }
        let byte_len = spec
            .storage_size()
            .checked_mul(size_of_dtype(spec.dtype()))
            .ok_or_else(|| {
                RuntimeError::Backend(format!(
                    "cuda buffer size overflow for tensor storage size {}",
                    spec.storage_size()
                ))
            })?;
        let platform_buffer = backend.platform.allocate(spec, byte_len)?;
        Ok(CudaBuffer {
            spec: spec.clone(),
            byte_len,
            memory_space: CudaMemorySpace::Device,
            host_visible: false,
            platform: platform_buffer,
        })
    }

    /// Begins a CUDA submission on a fresh stream.
    pub fn begin_submission(&self) -> Result<CudaSubmission, RuntimeError> {
        let Some(backend) = self.selected_backend() else {
            return Err(RuntimeError::Backend(self.health().message));
        };
        Ok(CudaSubmission {
            encoded_operations: 0,
            capturing: false,
            retained_buffers: Vec::new(),
            scratch_buffers: Vec::new(),
            platform: backend.platform.begin_submission()?,
        })
    }

    /// Begins recording a CUDA submission into a reusable captured graph.
    pub fn begin_captured_submission(&self) -> Result<CudaSubmission, RuntimeError> {
        let Some(backend) = self.selected_backend() else {
            return Err(RuntimeError::Backend(self.health().message));
        };
        Ok(CudaSubmission {
            encoded_operations: 0,
            capturing: true,
            retained_buffers: Vec::new(),
            scratch_buffers: Vec::new(),
            platform: backend.platform.begin_capture_submission()?,
        })
    }

    fn selected_backend(&self) -> Option<&AvailableCudaBackend> {
        match &self.state {
            CudaBackendState::Available(backend) => Some(backend),
            CudaBackendState::Unavailable(_) => None,
        }
    }

    fn selected_backend_mut(&mut self) -> Option<&mut AvailableCudaBackend> {
        match &mut self.state {
            CudaBackendState::Available(backend) => Some(backend),
            CudaBackendState::Unavailable(_) => None,
        }
    }
}

impl DeviceDiscovery for CudaBackend {
    fn backend_name(&self) -> BackendName {
        "cuda"
    }

    fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError> {
        self.discovery_report().map(|report| report.devices)
    }

    fn health(&self) -> RuntimeHealth {
        match &self.state {
            CudaBackendState::Available(_) => match self.discovery_report() {
                Ok(report) => report.health,
                Err(error) => RuntimeHealth {
                    status: HealthStatus::Degraded,
                    message: format!("cuda discovery failed: {error}"),
                },
            },
            CudaBackendState::Unavailable(health) => health.clone(),
        }
    }

    fn runtime_resources(&self) -> Option<BackendRuntimeResources> {
        CudaBackend::runtime_resources(self)
    }

    fn extension_support(&self) -> Vec<BackendExtensionSupport> {
        vec![
            BackendExtensionSupport::backend_specialized(
                BackendExtensionKind::ParameterGolfTokenEmbeddingLookup,
            ),
            BackendExtensionSupport::backend_specialized(BackendExtensionKind::ReluSquared),
            BackendExtensionSupport::backend_specialized(BackendExtensionKind::LeakyReluSquared),
            BackendExtensionSupport::backend_specialized(BackendExtensionKind::Silu),
            BackendExtensionSupport::backend_specialized(
                BackendExtensionKind::ParameterGolfProjectionLoss,
            ),
            BackendExtensionSupport::backend_specialized(
                BackendExtensionKind::ParameterGolfProjectionTokenLosses,
            ),
            BackendExtensionSupport::backend_specialized(BackendExtensionKind::RmsNorm),
            BackendExtensionSupport::backend_specialized(BackendExtensionKind::RotaryEmbedding),
            BackendExtensionSupport::backend_specialized(
                BackendExtensionKind::ScaledDotProductAttention,
            ),
        ]
    }
}

impl Allocator for CudaBackend {
    type Buffer = CudaBuffer;

    fn allocate(&mut self, spec: &TensorSpec) -> Result<Self::Buffer, RuntimeError> {
        self.allocate_buffer(spec)
    }
}

impl ExecutionBackend for CudaBackend {
    type Buffer = CudaBuffer;

    fn execute(
        &mut self,
        plan: &ExecutionPlan,
        inputs: &BTreeMap<TensorId, Self::Buffer>,
    ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError> {
        validate_supported_plan(plan)?;
        match &self.state {
            CudaBackendState::Available(backend) => backend.execute(plan, inputs),
            CudaBackendState::Unavailable(health) => Err(RuntimeError::Backend(format!(
                "cuda backend unavailable: {}",
                health.message
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct CudaHostFallbackOpProfile {
    count: u64,
    logical_values: u64,
    elapsed_ms: u64,
    cases: BTreeMap<String, CudaHostFallbackCaseProfile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CudaHostFallbackCaseProfile {
    detail: String,
    output_shape: Vec<usize>,
    output_dtype: DType,
    count: u64,
    logical_values: u64,
    elapsed_ms: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct CudaHostFallbackExecutionProfile {
    ops: BTreeMap<String, CudaHostFallbackOpProfile>,
}

impl CudaHostFallbackExecutionProfile {
    fn record(&mut self, step: &ExecutionStep, elapsed: Duration) {
        let label = step.op.label();
        let logical_values = step.spec.shape().element_count() as u64;
        let entry = self.ops.entry(label.to_string()).or_default();
        entry.count = entry.count.saturating_add(1);
        entry.logical_values = entry.logical_values.saturating_add(logical_values);
        entry.elapsed_ms = entry
            .elapsed_ms
            .saturating_add(elapsed.as_millis().try_into().unwrap_or(u64::MAX));
        let detail = host_fallback_step_detail(&step.op);
        let case_key = format!(
            "{}|{:?}|{:?}",
            detail,
            step.spec.shape().dims(),
            step.spec.dtype()
        );
        let case = entry
            .cases
            .entry(case_key)
            .or_insert_with(|| CudaHostFallbackCaseProfile {
                detail,
                output_shape: step.spec.shape().dims().to_vec(),
                output_dtype: step.spec.dtype(),
                count: 0,
                logical_values: 0,
                elapsed_ms: 0,
            });
        case.count = case.count.saturating_add(1);
        case.logical_values = case.logical_values.saturating_add(logical_values);
        case.elapsed_ms = case
            .elapsed_ms
            .saturating_add(elapsed.as_millis().try_into().unwrap_or(u64::MAX));
    }

    fn total_elapsed_ms(&self) -> u64 {
        self.ops
            .values()
            .map(|profile| profile.elapsed_ms)
            .sum::<u64>()
    }
}

fn host_fallback_profile_path() -> Option<String> {
    env::var("PSIONIC_CUDA_HOST_FALLBACK_PROFILE_PATH")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn required_batched_elements(
    matrix_elements: usize,
    batch_count: usize,
    stride_elements: usize,
    label: &str,
) -> Result<usize, RuntimeError> {
    if batch_count == 0 {
        return Ok(0);
    }
    let last_offset = stride_elements
        .checked_mul(batch_count - 1)
        .ok_or_else(|| {
            RuntimeError::Backend(format!("cuda {label} batched matrix stride overflow"))
        })?;
    last_offset.checked_add(matrix_elements).ok_or_else(|| {
        RuntimeError::Backend(format!(
            "cuda {label} batched matrix required element count overflow"
        ))
    })
}

fn append_host_fallback_profile_report(
    path: &str,
    plan: &ExecutionPlan,
    profile: &CudaHostFallbackExecutionProfile,
) -> Result<(), RuntimeError> {
    if profile.ops.is_empty() {
        return Ok(());
    }
    let mut report = format!(
        "{{\"scope_window\":\"psionic_cuda_host_fallback_profile_v2\",\"pid\":{},\"plan_steps\":{},\"plan_outputs\":{},\"total_host_fallback_ms\":{},\"ops\":[",
        std::process::id(),
        plan.steps.len(),
        plan.outputs.len(),
        profile.total_elapsed_ms(),
    );
    for (index, (label, op_profile)) in profile.ops.iter().enumerate() {
        if index > 0 {
            report.push(',');
        }
        report.push_str(&format!(
            "{{\"label\":\"{}\",\"count\":{},\"logical_values\":{},\"elapsed_ms\":{},\"cases\":[",
            label, op_profile.count, op_profile.logical_values, op_profile.elapsed_ms
        ));
        for (case_index, case) in op_profile.cases.values().enumerate() {
            if case_index > 0 {
                report.push(',');
            }
            report.push_str(&format!(
                "{{\"detail\":\"{}\",\"output_shape\":{},\"output_dtype\":\"{}\",\"count\":{},\"logical_values\":{},\"elapsed_ms\":{}}}",
                case.detail,
                json_usize_array(case.output_shape.as_slice()),
                dtype_label(case.output_dtype),
                case.count,
                case.logical_values,
                case.elapsed_ms
            ));
        }
        report.push_str("]}");
    }
    report.push_str("]}");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| {
            RuntimeError::Backend(format!(
                "failed to open cuda host fallback profile sink `{path}`: {error}"
            ))
        })?;
    writeln!(file, "{report}").map_err(|error| {
        RuntimeError::Backend(format!(
            "failed to append cuda host fallback profile report `{path}`: {error}"
        ))
    })
}

fn execute_profiled_host_fallback<F>(
    profile: &mut Option<CudaHostFallbackExecutionProfile>,
    step: &ExecutionStep,
    op: F,
) -> Result<CudaBuffer, RuntimeError>
where
    F: FnOnce() -> Result<CudaBuffer, RuntimeError>,
{
    let started = Instant::now();
    let output = op()?;
    if let Some(profile) = profile.as_mut() {
        profile.record(step, started.elapsed());
    }
    Ok(output)
}

fn host_fallback_step_detail(op: &ExecutionOp) -> String {
    match op {
        ExecutionOp::Permute { axes } => format!("axes={}", json_usize_csv(axes)),
        ExecutionOp::ReduceSum { axis } => match axis {
            Some(axis) => format!("axis={axis}"),
            None => String::from("axis=all"),
        },
        ExecutionOp::Cast { dtype } => format!("target_dtype={}", dtype_label(*dtype)),
        ExecutionOp::BackendExtension { op } => format!("{op:?}"),
        _ => String::from("default"),
    }
}

fn json_usize_csv(values: &[usize]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn json_usize_array(values: &[usize]) -> String {
    format!("[{}]", json_usize_csv(values))
}

fn dtype_label(dtype: DType) -> &'static str {
    match dtype {
        DType::F32 => "f32",
        DType::F16 => "f16",
        DType::BF16 => "bf16",
        DType::I32 => "i32",
        DType::I8 => "i8",
    }
}

impl AvailableCudaBackend {
    fn lookup_or_compile(
        &mut self,
        graph: &Graph,
    ) -> Result<(ExecutionPlan, String, CompilePathEvidence), RuntimeError> {
        let (plan, plan_digest, plan_cache_hit) =
            self.execution_plan_cache.lookup_or_compile(graph)?;
        let kernel_cache = if self.kernel_cache.policy.enabled {
            CacheObservation::new(
                CacheKind::KernelCache,
                CacheAction::Reuse,
                "reused the configured cuda kernel cache",
            )
        } else {
            CacheObservation::new(
                CacheKind::KernelCache,
                CacheAction::Bypass,
                "cuda kernel cache is disabled for this backend path",
            )
        };
        Ok((
            plan,
            plan_digest,
            CompilePathEvidence {
                temperature: if plan_cache_hit {
                    CompilePathTemperature::WarmReuse
                } else {
                    CompilePathTemperature::ColdCompile
                },
                execution_plan_cache: if plan_cache_hit {
                    CacheObservation::new(
                        CacheKind::ExecutionPlan,
                        CacheAction::Reuse,
                        "reused a cached cuda execution plan",
                    )
                } else {
                    CacheObservation::new(
                        CacheKind::ExecutionPlan,
                        CacheAction::Rebuild,
                        "compiled a new cuda execution plan",
                    )
                },
                kernel_cache,
            },
        ))
    }

    fn allocate(&self, spec: &TensorSpec) -> Result<CudaBuffer, RuntimeError> {
        let byte_len = spec
            .storage_size()
            .checked_mul(size_of_dtype(spec.dtype()))
            .ok_or_else(|| {
                RuntimeError::Backend(format!(
                    "cuda buffer size overflow for tensor storage size {}",
                    spec.storage_size()
                ))
            })?;
        let platform_buffer = self.platform.allocate(spec, byte_len)?;
        Ok(CudaBuffer {
            spec: spec.clone(),
            byte_len,
            memory_space: CudaMemorySpace::Device,
            host_visible: false,
            platform: platform_buffer,
        })
    }

    fn buffer_from_tensor_data(
        &self,
        spec: &TensorSpec,
        data: &TensorData,
    ) -> Result<CudaBuffer, RuntimeError> {
        if data.len() != spec.storage_size() {
            return Err(RuntimeError::Backend(String::from(
                "cuda constant payload length mismatch",
            )));
        }
        let mut buffer = self.allocate(spec)?;
        match data {
            TensorData::F32(values) => buffer.write_f32(values)?,
            TensorData::BF16(values) => buffer.write_bf16_from_f32(values)?,
            TensorData::I32(values) => buffer.write_i32(values)?,
            TensorData::QuantizedBlocks(_) => {
                return Err(RuntimeError::Backend(String::from(
                    "cuda constant storage must use dense f32, bf16, or i32 payloads",
                )));
            }
        }
        Ok(buffer)
    }

    fn materialize_host_view_step(
        &self,
        step: &ExecutionStep,
        values: &[f32],
    ) -> Result<CudaBuffer, RuntimeError> {
        let contiguous = TensorSpec::new(
            step.spec.shape().clone(),
            step.spec.dtype(),
            step.spec.device().clone(),
        );
        if values.len() != contiguous.element_count() {
            return Err(RuntimeError::Backend(format!(
                "{} host fallback expected {} logical values, found {}",
                step.op.label(),
                contiguous.element_count(),
                values.len()
            )));
        }
        let tensor_data = match step.spec.dtype() {
            DType::F32 => TensorData::F32(values.to_vec()),
            DType::BF16 => TensorData::BF16(values.to_vec()),
            actual => {
                return Err(RuntimeError::UnsupportedStep(format!(
                    "{} host fallback currently supports only f32/bf16 tensors, actual {:?}",
                    step.op.label(),
                    actual
                )));
            }
        };
        self.buffer_from_tensor_data(&contiguous, &tensor_data)
    }

    fn read_float_buffer_to_f32(&self, buffer: &CudaBuffer) -> Result<Vec<f32>, RuntimeError> {
        let storage_values = match buffer.spec().dtype() {
            DType::F32 => buffer.read_f32(),
            DType::BF16 => buffer.read_bf16_to_f32(),
            actual => Err(RuntimeError::Backend(format!(
                "cuda float readback requires F32 or BF16 buffer, actual {:?}",
                actual
            ))),
        }?;
        if buffer.spec().layout().is_contiguous()
            && buffer.spec().layout().offset() == 0
            && buffer.spec().storage_size() == buffer.spec().element_count()
        {
            return Ok(storage_values);
        }
        let layout = buffer.spec().layout();
        let logical_shape = buffer.spec().shape().dims();
        let logical_strides = layout.strides();
        let logical_count = buffer.spec().element_count();
        let mut values = vec![0.0_f32; logical_count];
        let contiguous = contiguous_strides(logical_shape);
        let mut coords = vec![0_usize; logical_shape.len()];
        for (logical_index, value) in values.iter_mut().enumerate() {
            linear_to_coords_in_place(logical_index, logical_shape, &contiguous, &mut coords);
            let storage_index = layout.offset()
                + coords
                    .iter()
                    .zip(logical_strides.iter())
                    .map(|(index, stride)| index.saturating_mul(*stride))
                    .sum::<usize>();
            *value = storage_values[storage_index];
        }
        Ok(values)
    }

    fn execute_rotary_embedding_step(
        &self,
        submission: &mut CudaSubmission,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
        interleaved: bool,
    ) -> Result<CudaBuffer, RuntimeError> {
        if interleaved {
            return Err(RuntimeError::Backend(String::from(
                "cuda rotary_embedding only supports the non-interleaved NEOX-style lane",
            )));
        }
        let input = step_input(step, values, 0)?;
        let cos = step_input(step, values, 1)?;
        let sin = step_input(step, values, 2)?;
        let dims = input.spec().shape().dims();
        if dims.len() != 4 || dims[3] == 0 || !dims[3].is_multiple_of(2) {
            return Err(RuntimeError::Backend(String::from(
                "cuda rotary_embedding requires rank-4 tensors with an even head dimension",
            )));
        }
        let batch_size = dims[0];
        let head_count = dims[1];
        let sequence_length = dims[2];
        let head_dim = dims[3];
        let half_dim = head_dim / 2;
        let _ =
            validate_standard_neox_rope_tables(cos, sin, batch_size, sequence_length, half_dim)?;

        let output = self.allocate(&step.spec)?;
        let scratch_spec = TensorSpec::new(
            Shape::new(vec![head_count, head_dim]),
            DType::F32,
            step.spec.device().clone(),
        );
        let scratch = self.allocate(&scratch_spec)?;
        let theta_scale = CUDA_PUBLIC_STANDARD_ROPE_BASE.powf(-2.0_f32 / head_dim as f32);
        let corr_dims = [0.0_f32, head_dim as f32 - 1.0];
        for batch_index in 0..batch_size {
            for position in 0..sequence_length {
                pack_rank4_token_to_head_major_scratch(
                    submission,
                    input,
                    batch_index,
                    position,
                    &scratch,
                )?;
                submission.rope_neox_in_place(
                    &scratch,
                    0,
                    head_count,
                    head_dim,
                    head_dim,
                    position,
                    1.0,
                    0.0,
                    corr_dims,
                    theta_scale,
                )?;
                scatter_head_major_scratch_to_rank4_token(
                    submission,
                    &scratch,
                    &output,
                    batch_index,
                    position,
                )?;
            }
        }
        Ok(output)
    }

    fn execute_silu_step(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
    ) -> Result<CudaBuffer, RuntimeError> {
        let input = step_input(step, values, 0)?;
        let input_values = input.read_f32()?;
        let output_values = silu_forward_values(&input_values);
        self.buffer_from_tensor_data(&step.spec, &TensorData::F32(output_values))
    }

    fn execute_relu_squared_step(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
    ) -> Result<CudaBuffer, RuntimeError> {
        let input = step_input(step, values, 0)?;
        let input_values = input.read_f32()?;
        let output_values = relu_squared_forward_values(&input_values);
        self.buffer_from_tensor_data(&step.spec, &TensorData::F32(output_values))
    }

    fn execute_relu_squared_backward_step(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
    ) -> Result<CudaBuffer, RuntimeError> {
        let input = step_input(step, values, 0)?;
        let grad_output = step_input(step, values, 1)?;
        let input_values = input.read_f32()?;
        let grad_output_values = grad_output.read_f32()?;
        let output_values = relu_squared_backward_values(&input_values, &grad_output_values);
        self.buffer_from_tensor_data(&step.spec, &TensorData::F32(output_values))
    }

    fn execute_relu_squared_backward_from_output_step(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
    ) -> Result<CudaBuffer, RuntimeError> {
        let output = step_input(step, values, 0)?;
        let grad_output = step_input(step, values, 1)?;
        let output_values = output.read_f32()?;
        let grad_output_values = grad_output.read_f32()?;
        let input_gradient_values =
            relu_squared_backward_from_output_values(&output_values, &grad_output_values);
        self.buffer_from_tensor_data(&step.spec, &TensorData::F32(input_gradient_values))
    }

    fn execute_leaky_relu_squared_step(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
        negative_slope: f32,
    ) -> Result<CudaBuffer, RuntimeError> {
        let input = step_input(step, values, 0)?;
        let input_values = input.read_f32()?;
        let output_values = leaky_relu_squared_forward_values(&input_values, negative_slope);
        self.buffer_from_tensor_data(&step.spec, &TensorData::F32(output_values))
    }

    fn execute_leaky_relu_squared_backward_step(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
        negative_slope: f32,
    ) -> Result<CudaBuffer, RuntimeError> {
        let input = step_input(step, values, 0)?;
        let grad_output = step_input(step, values, 1)?;
        let input_values = input.read_f32()?;
        let grad_output_values = grad_output.read_f32()?;
        let output_values =
            leaky_relu_squared_backward_values(&input_values, &grad_output_values, negative_slope);
        self.buffer_from_tensor_data(&step.spec, &TensorData::F32(output_values))
    }

    fn execute_silu_backward_step(
        &self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
    ) -> Result<CudaBuffer, RuntimeError> {
        let input = step_input(step, values, 0)?;
        let grad_output = step_input(step, values, 1)?;
        let input_values = input.read_f32()?;
        let grad_output_values = grad_output.read_f32()?;
        let output_values = silu_backward_values(&input_values, &grad_output_values);
        self.buffer_from_tensor_data(&step.spec, &TensorData::F32(output_values))
    }

    fn execute_parameter_golf_token_embedding_lookup_step(
        &self,
        submission: &mut CudaSubmission,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
    ) -> Result<CudaBuffer, RuntimeError> {
        let token_ids = step_input(step, values, 0)?;
        let token_embedding = step_input(step, values, 1)?;
        let token_id_dims = token_ids.spec().shape().dims();
        let token_embedding_dims = token_embedding.spec().shape().dims();
        if token_ids.spec().dtype() != DType::I32 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_token_embedding_lookup requires I32 token ids",
            )));
        }
        if token_embedding.spec().dtype() != DType::F32
            && token_embedding.spec().dtype() != DType::BF16
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_token_embedding_lookup requires F32 or BF16 token embedding weights",
            )));
        }
        if token_id_dims.len() != 2 || token_embedding_dims.len() != 2 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_token_embedding_lookup requires rank-2 token ids and token embedding tensors",
            )));
        }
        let row_count = token_id_dims[0].saturating_mul(token_id_dims[1]);
        let vocab_size = token_embedding_dims[0];
        let width = token_embedding_dims[1];
        let output = self.allocate(&step.spec)?;
        match token_embedding.spec().dtype() {
            DType::F32 => submission
                .platform
                .encode_parameter_golf_token_embedding_lookup(
                    &token_ids.platform,
                    &token_embedding.platform,
                    row_count,
                    vocab_size,
                    width,
                    &output.platform,
                )?,
            DType::BF16 => submission
                .platform
                .encode_parameter_golf_token_embedding_lookup_bf16_to_f32(
                    &token_ids.platform,
                    &token_embedding.platform,
                    row_count,
                    vocab_size,
                    width,
                    &output.platform,
                )?,
            actual => {
                return Err(RuntimeError::Backend(format!(
                    "cuda parameter_golf_token_embedding_lookup does not support token embedding dtype {actual:?}",
                )));
            }
        }
        submission.encoded_operations += 1;
        Ok(output)
    }

    fn execute_parameter_golf_banked_linear_step(
        &self,
        submission: &mut CudaSubmission,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
        bank_index: usize,
    ) -> Result<CudaBuffer, RuntimeError> {
        let input = step_input(step, values, 0)?;
        let matrix_bank = step_input(step, values, 1)?;
        let input_dims = input.spec().shape().dims();
        let bank_dims = matrix_bank.spec().shape().dims();
        if (input.spec().dtype() != DType::F32 && input.spec().dtype() != DType::BF16)
            || (matrix_bank.spec().dtype() != DType::F32
                && matrix_bank.spec().dtype() != DType::BF16)
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_banked_linear requires F32 or BF16 activations and weights",
            )));
        }
        if input_dims.len() != 3 || bank_dims.len() != 3 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_banked_linear requires rank-3 input [batch, seq, in_features] and rank-3 matrix_bank [bank_len, out_features, in_features]",
            )));
        }
        if input_dims[2] != bank_dims[2] {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_banked_linear requires input in_features to match matrix_bank in_features",
            )));
        }
        if bank_index >= bank_dims[0] {
            return Err(RuntimeError::Backend(format!(
                "cuda parameter_golf_banked_linear bank_index {bank_index} exceeds bank length {}",
                bank_dims[0]
            )));
        }

        let rows = input_dims[0].saturating_mul(input_dims[1]);
        let cols = bank_dims[1];
        let inner = bank_dims[2];
        let input_flat = input.alias_with_spec(&TensorSpec::new(
            Shape::new(vec![rows, inner]),
            input.spec().dtype(),
            input.spec().device().clone(),
        ))?;
        let output = self.allocate(&step.spec)?;
        let output_flat = output.alias_with_spec(&TensorSpec::new(
            Shape::new(vec![rows, cols]),
            DType::F32,
            output.spec().device().clone(),
        ))?;
        let matrix_shape = Shape::new(vec![cols, inner]);
        let matrix_spec = TensorSpec::new(
            matrix_shape.clone(),
            matrix_bank.spec().dtype(),
            matrix_bank.spec().device().clone(),
        );
        let matrix_byte_len = matrix_spec
            .storage_size()
            .checked_mul(size_of_dtype(matrix_spec.dtype()))
            .ok_or_else(|| {
                RuntimeError::Backend(format!(
                    "cuda parameter_golf_banked_linear bank-slice size overflow for tensor storage size {}",
                    matrix_spec.storage_size()
                ))
            })?;
        let source_byte_offset = bank_index.checked_mul(matrix_byte_len).ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda parameter_golf_banked_linear source offset overflow",
            ))
        })?;
        let matrix =
            matrix_bank.contiguous_subbuffer_with_spec(&matrix_spec, source_byte_offset)?;

        match (input.spec().dtype(), matrix_bank.spec().dtype()) {
            (DType::F32, DType::BF16) => submission
                .matmul_f32_bf16_strided_batched_to_f32_with_strides(
                    &input_flat,
                    &matrix,
                    &output_flat,
                    rows,
                    inner,
                    cols,
                    1,
                    rows.saturating_mul(inner),
                    cols.saturating_mul(inner),
                    rows.saturating_mul(cols),
                    false,
                    true,
                )?,
            (DType::BF16, DType::BF16) => submission
                .matmul_bf16_strided_batched_to_f32_with_strides(
                    &input_flat,
                    &matrix,
                    &output_flat,
                    rows,
                    inner,
                    cols,
                    1,
                    rows.saturating_mul(inner),
                    cols.saturating_mul(inner),
                    rows.saturating_mul(cols),
                    false,
                    true,
                )?,
            (input_dtype, DType::F32)
                if input_dtype == DType::F32 || input_dtype == DType::BF16 =>
            {
                let input_f32 = if input_dtype == DType::F32 {
                    input_flat.clone()
                } else {
                    let casted = self.allocate(&TensorSpec::new(
                        Shape::new(vec![rows, inner]),
                        DType::F32,
                        input.spec().device().clone(),
                    ))?;
                    submission.cast_bf16_to_f32(
                        &input_flat,
                        &casted,
                        rows.saturating_mul(inner),
                    )?;
                    casted
                };
                let matrix_transposed = self.allocate(&TensorSpec::new(
                    Shape::new(vec![inner, cols]),
                    DType::F32,
                    matrix_bank.spec().device().clone(),
                ))?;
                submission.permute_rank2_transpose_f32(&matrix, &matrix_transposed, cols, inner)?;
                submission.matmul(
                    &input_f32,
                    &matrix_transposed,
                    &output_flat,
                    rows,
                    inner,
                    cols,
                )?;
            }
            (input_dtype, weight_dtype) => {
                return Err(RuntimeError::Backend(format!(
                    "cuda parameter_golf_banked_linear does not support dtype posture input={input_dtype:?} matrix_bank={weight_dtype:?}",
                )));
            }
        }
        Ok(output)
    }

    fn execute_parameter_golf_banked_linear_input_backward_step(
        &self,
        submission: &mut CudaSubmission,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
        bank_index: usize,
    ) -> Result<CudaBuffer, RuntimeError> {
        let grad_output = step_input(step, values, 0)?;
        let matrix_bank = step_input(step, values, 1)?;
        let grad_dims = grad_output.spec().shape().dims();
        let bank_dims = matrix_bank.spec().shape().dims();
        if grad_output.spec().dtype() != DType::F32
            || (matrix_bank.spec().dtype() != DType::F32
                && matrix_bank.spec().dtype() != DType::BF16)
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_banked_linear_input_backward requires F32 grad_output and F32 or BF16 weights",
            )));
        }
        if grad_dims.len() != 3 || bank_dims.len() != 3 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_banked_linear_input_backward requires rank-3 grad_output [batch, seq, out_features] and rank-3 matrix_bank [bank_len, out_features, in_features]",
            )));
        }
        if grad_dims[2] != bank_dims[1] {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_banked_linear_input_backward requires grad_output out_features to match matrix_bank out_features",
            )));
        }
        if bank_index >= bank_dims[0] {
            return Err(RuntimeError::Backend(format!(
                "cuda parameter_golf_banked_linear_input_backward bank_index {bank_index} exceeds bank length {}",
                bank_dims[0]
            )));
        }

        let rows = grad_dims[0].saturating_mul(grad_dims[1]);
        let out_features = grad_dims[2];
        let in_features = bank_dims[2];
        let grad_flat = grad_output.alias_with_spec(&TensorSpec::new(
            Shape::new(vec![rows, out_features]),
            DType::F32,
            grad_output.spec().device().clone(),
        ))?;
        let output = self.allocate(&step.spec)?;
        let output_flat = output.alias_with_spec(&TensorSpec::new(
            Shape::new(vec![rows, in_features]),
            DType::F32,
            output.spec().device().clone(),
        ))?;
        let matrix_shape = Shape::new(vec![out_features, in_features]);
        let matrix_spec = TensorSpec::new(
            matrix_shape.clone(),
            matrix_bank.spec().dtype(),
            matrix_bank.spec().device().clone(),
        );
        let matrix_byte_len = matrix_spec
            .storage_size()
            .checked_mul(size_of_dtype(matrix_spec.dtype()))
            .ok_or_else(|| {
                RuntimeError::Backend(format!(
                    "cuda parameter_golf_banked_linear_input_backward bank-slice size overflow for tensor storage size {}",
                    matrix_spec.storage_size()
                ))
            })?;
        let source_byte_offset = bank_index.checked_mul(matrix_byte_len).ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda parameter_golf_banked_linear_input_backward source offset overflow",
            ))
        })?;
        let matrix =
            matrix_bank.contiguous_subbuffer_with_spec(&matrix_spec, source_byte_offset)?;

        match matrix_bank.spec().dtype() {
            DType::BF16 => submission.matmul_f32_bf16_strided_batched_to_f32_with_strides(
                &grad_flat,
                &matrix,
                &output_flat,
                rows,
                out_features,
                in_features,
                1,
                rows.saturating_mul(out_features),
                out_features.saturating_mul(in_features),
                rows.saturating_mul(in_features),
                false,
                false,
            )?,
            DType::F32 => submission.matmul(
                &grad_flat,
                &matrix,
                &output_flat,
                rows,
                out_features,
                in_features,
            )?,
            actual => {
                return Err(RuntimeError::Backend(format!(
                    "cuda parameter_golf_banked_linear_input_backward does not support matrix_bank dtype {actual:?}",
                )));
            }
        }
        Ok(output)
    }

    fn execute_parameter_golf_banked_linear_weight_backward_step(
        &self,
        submission: &mut CudaSubmission,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
        bank_index: usize,
    ) -> Result<CudaBuffer, RuntimeError> {
        let input = step_input(step, values, 0)?;
        let matrix_bank = step_input(step, values, 1)?;
        let grad_output = step_input(step, values, 2)?;
        let input_dims = input.spec().shape().dims();
        let bank_dims = matrix_bank.spec().shape().dims();
        let grad_dims = grad_output.spec().shape().dims();
        if (input.spec().dtype() != DType::F32 && input.spec().dtype() != DType::BF16)
            || grad_output.spec().dtype() != DType::F32
            || (matrix_bank.spec().dtype() != DType::F32
                && matrix_bank.spec().dtype() != DType::BF16)
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_banked_linear_weight_backward requires F32 grad_output and F32 or BF16 activations and weights",
            )));
        }
        if input_dims.len() != 3 || bank_dims.len() != 3 || grad_dims.len() != 3 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_banked_linear_weight_backward requires rank-3 input [batch, seq, in_features], rank-3 matrix_bank [bank_len, out_features, in_features], and rank-3 grad_output [batch, seq, out_features]",
            )));
        }
        if input_dims[0] != grad_dims[0] || input_dims[1] != grad_dims[1] {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_banked_linear_weight_backward requires input and grad_output batch/seq dimensions to match",
            )));
        }
        if input_dims[2] != bank_dims[2] || grad_dims[2] != bank_dims[1] {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_banked_linear_weight_backward requires input and grad_output features to match matrix_bank",
            )));
        }
        if bank_index >= bank_dims[0] {
            return Err(RuntimeError::Backend(format!(
                "cuda parameter_golf_banked_linear_weight_backward bank_index {bank_index} exceeds bank length {}",
                bank_dims[0]
            )));
        }

        let rows = input_dims[0].saturating_mul(input_dims[1]);
        let in_features = input_dims[2];
        let out_features = grad_dims[2];
        let input_flat = input.alias_with_spec(&TensorSpec::new(
            Shape::new(vec![rows, in_features]),
            input.spec().dtype(),
            input.spec().device().clone(),
        ))?;
        let grad_flat = grad_output.alias_with_spec(&TensorSpec::new(
            Shape::new(vec![rows, out_features]),
            DType::F32,
            grad_output.spec().device().clone(),
        ))?;
        let grad_transposed = self.allocate(&TensorSpec::new(
            Shape::new(vec![out_features, rows]),
            DType::F32,
            grad_output.spec().device().clone(),
        ))?;
        submission.permute_rank2_transpose_f32(&grad_flat, &grad_transposed, rows, out_features)?;
        let gradient_slice_spec = TensorSpec::new(
            Shape::new(vec![out_features, in_features]),
            DType::F32,
            grad_output.spec().device().clone(),
        );

        let output = self.allocate(&step.spec)?;
        submission.fill_buffer(&output, 0)?;
        let gradient_slice_byte_len = gradient_slice_spec
            .storage_size()
            .checked_mul(size_of_dtype(gradient_slice_spec.dtype()))
            .ok_or_else(|| {
                RuntimeError::Backend(format!(
                    "cuda parameter_golf_banked_linear_weight_backward gradient-slice size overflow for tensor storage size {}",
                    gradient_slice_spec.storage_size()
                ))
            })?;
        let destination_byte_offset = bank_index.checked_mul(gradient_slice_byte_len).ok_or_else(
            || {
                RuntimeError::Backend(String::from(
                    "cuda parameter_golf_banked_linear_weight_backward destination offset overflow",
                ))
            },
        )?;
        let gradient_slice =
            output.contiguous_subbuffer_with_spec(&gradient_slice_spec, destination_byte_offset)?;
        match input.spec().dtype() {
            DType::F32 => submission.matmul(
                &grad_transposed,
                &input_flat,
                &gradient_slice,
                out_features,
                rows,
                in_features,
            )?,
            DType::BF16 => submission.matmul_f32_bf16_strided_batched_to_f32_with_strides(
                &grad_transposed,
                &input_flat,
                &gradient_slice,
                out_features,
                rows,
                in_features,
                1,
                out_features.saturating_mul(rows),
                rows.saturating_mul(in_features),
                out_features.saturating_mul(in_features),
                false,
                false,
            )?,
            actual => {
                return Err(RuntimeError::Backend(format!(
                    "cuda parameter_golf_banked_linear_weight_backward does not support input dtype {actual:?}",
                )));
            }
        }
        Ok(output)
    }

    fn execute_parameter_golf_token_embedding_lookup_backward_step(
        &self,
        submission: &mut CudaSubmission,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
    ) -> Result<CudaBuffer, RuntimeError> {
        let token_ids = step_input(step, values, 0)?;
        let token_embedding = step_input(step, values, 1)?;
        let grad_output = step_input(step, values, 2)?;
        let token_id_dims = token_ids.spec().shape().dims();
        let token_embedding_dims = token_embedding.spec().shape().dims();
        let grad_output_dims = grad_output.spec().shape().dims();
        if token_ids.spec().dtype() != DType::I32 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_token_embedding_lookup_backward requires I32 token ids",
            )));
        }
        if token_embedding.spec().dtype() != DType::F32
            && token_embedding.spec().dtype() != DType::BF16
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_token_embedding_lookup_backward requires F32 or BF16 embedding weights",
            )));
        }
        if grad_output.spec().dtype() != DType::F32 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_token_embedding_lookup_backward requires F32 grad_output",
            )));
        }
        if token_id_dims.len() != 2
            || token_embedding_dims.len() != 2
            || grad_output_dims.len() != 3
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_token_embedding_lookup_backward requires rank-2 token ids and token embedding tensors plus rank-3 grad_output",
            )));
        }
        let row_count = token_id_dims[0].saturating_mul(token_id_dims[1]);
        let vocab_size = token_embedding_dims[0];
        let width = token_embedding_dims[1];
        let output = self.allocate(&step.spec)?;
        submission
            .platform
            .encode_parameter_golf_token_embedding_lookup_backward(
                &token_ids.platform,
                &grad_output.platform,
                row_count,
                vocab_size,
                width,
                &output.platform,
            )?;
        submission.encoded_operations += 1;
        Ok(output)
    }

    fn execute_parameter_golf_projection_loss_step(
        &self,
        submission: &mut CudaSubmission,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
        logit_softcap: f32,
    ) -> Result<CudaBuffer, RuntimeError> {
        let logits = step_input(step, values, 0)?;
        let target_ids = step_input(step, values, 1)?;
        let logits_dims = logits.spec().shape().dims();
        let target_dims = target_ids.spec().shape().dims();
        if target_ids.spec().dtype() != DType::I32 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_projection_loss requires I32 target_ids",
            )));
        }
        if logits_dims.len() != 3 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_projection_loss requires rank-3 logits [batch, seq, vocab]",
            )));
        }
        if target_dims.len() != 2 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_projection_loss requires rank-2 target_ids [batch, seq]",
            )));
        }
        if logits_dims[0] != target_dims[0] || logits_dims[1] != target_dims[1] {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_projection_loss requires target_ids batch/seq dims to match logits",
            )));
        }
        let row_count = logits_dims[0].saturating_mul(logits_dims[1]);
        let vocab_size = logits_dims[2];
        let output = self.allocate(&step.spec)?;
        submission.platform.encode_parameter_golf_projection_loss(
            &logits.platform,
            &target_ids.platform,
            &output.platform,
            row_count,
            vocab_size,
            logit_softcap,
        )?;
        submission.encoded_operations += 1;
        Ok(output)
    }

    fn execute_parameter_golf_projection_token_losses_step(
        &self,
        submission: &mut CudaSubmission,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
        logit_softcap: f32,
    ) -> Result<CudaBuffer, RuntimeError> {
        let logits = step_input(step, values, 0)?;
        let target_ids = step_input(step, values, 1)?;
        let logits_dims = logits.spec().shape().dims();
        let target_dims = target_ids.spec().shape().dims();
        if target_ids.spec().dtype() != DType::I32 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_projection_token_losses requires I32 target_ids",
            )));
        }
        if logits_dims.len() != 3 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_projection_token_losses requires rank-3 logits [batch, seq, vocab]",
            )));
        }
        if target_dims.len() != 2 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_projection_token_losses requires rank-2 target_ids [batch, seq]",
            )));
        }
        if logits_dims[0] != target_dims[0] || logits_dims[1] != target_dims[1] {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_projection_token_losses requires target_ids batch/seq dims to match logits",
            )));
        }
        let row_count = logits_dims[0].saturating_mul(logits_dims[1]);
        let vocab_size = logits_dims[2];
        let output = self.allocate(&step.spec)?;
        submission
            .platform
            .encode_parameter_golf_projection_token_losses(
                &logits.platform,
                &target_ids.platform,
                &output.platform,
                row_count,
                vocab_size,
                logit_softcap,
            )?;
        submission.encoded_operations += 1;
        Ok(output)
    }

    fn execute_parameter_golf_projection_loss_backward_step(
        &self,
        submission: &mut CudaSubmission,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
        logit_softcap: f32,
    ) -> Result<CudaBuffer, RuntimeError> {
        let logits = step_input(step, values, 0)?;
        let target_ids = step_input(step, values, 1)?;
        let grad_output = step_input(step, values, 2)?;
        let logits_dims = logits.spec().shape().dims();
        let target_dims = target_ids.spec().shape().dims();
        if target_ids.spec().dtype() != DType::I32 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_projection_loss_backward requires I32 target_ids",
            )));
        }
        if logits_dims.len() != 3 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_projection_loss_backward requires rank-3 logits [batch, seq, vocab]",
            )));
        }
        if target_dims.len() != 2 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_projection_loss_backward requires rank-2 target_ids [batch, seq]",
            )));
        }
        if logits_dims[0] != target_dims[0] || logits_dims[1] != target_dims[1] {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_projection_loss_backward requires target_ids batch/seq dims to match logits",
            )));
        }
        if grad_output.spec().storage_size() != 1 {
            return Err(RuntimeError::Backend(String::from(
                "cuda parameter_golf_projection_loss_backward requires scalar grad_output",
            )));
        }
        let row_count = logits_dims[0].saturating_mul(logits_dims[1]);
        let vocab_size = logits_dims[2];
        let output = self.allocate(&step.spec)?;
        submission
            .platform
            .encode_parameter_golf_projection_loss_backward(
                &logits.platform,
                &target_ids.platform,
                &grad_output.platform,
                &output.platform,
                row_count,
                vocab_size,
                logit_softcap,
            )?;
        submission.encoded_operations += 1;
        Ok(output)
    }

    fn execute_scaled_dot_product_attention_step(
        &self,
        submission: &mut CudaSubmission,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
        scale: f32,
        causal: bool,
    ) -> Result<CudaBuffer, RuntimeError> {
        if !causal {
            return Err(RuntimeError::Backend(String::from(
                "cuda scaled_dot_product_attention only supports the bounded causal self-attention lane",
            )));
        }
        let query = step_input(step, values, 0)?;
        let key = step_input(step, values, 1)?;
        let value = step_input(step, values, 2)?;
        let query_dims = query.spec().shape().dims();
        let key_dims = key.spec().shape().dims();
        let value_dims = value.spec().shape().dims();
        if query_dims.len() != 4 || key_dims.len() != 4 || value_dims.len() != 4 {
            return Err(RuntimeError::Backend(String::from(
                "cuda scaled_dot_product_attention requires rank-4 query/key/value tensors",
            )));
        }
        let batch_size = query_dims[0];
        let query_heads = query_dims[1];
        let sequence_length = query_dims[2];
        let head_dim = query_dims[3];
        let kv_heads = key_dims[1];
        if batch_size == 0
            || query_heads == 0
            || kv_heads == 0
            || head_dim == 0
            || sequence_length == 0
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda scaled_dot_product_attention requires non-empty batch, head, sequence, and head-dimension extents",
            )));
        }
        if batch_size != key_dims[0]
            || batch_size != value_dims[0]
            || sequence_length != key_dims[2]
            || sequence_length != value_dims[2]
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda scaled_dot_product_attention only supports bounded causal self-attention with matching batch and sequence extents",
            )));
        }
        if head_dim != key_dims[3] || head_dim != value_dims[3] {
            return Err(RuntimeError::Backend(String::from(
                "cuda scaled_dot_product_attention requires value and key head dimensions to match query head dimensions on the bounded lane",
            )));
        }
        if kv_heads != value_dims[1] || !query_heads.is_multiple_of(kv_heads) {
            return Err(RuntimeError::Backend(String::from(
                "cuda scaled_dot_product_attention requires grouped-query head counts with query_heads divisible by kv_heads",
            )));
        }
        if sequence_length > CUDA_PUBLIC_ATTENTION_MAX_SEQUENCE {
            return Err(RuntimeError::Backend(format!(
                "cuda scaled_dot_product_attention only supports sequence lengths up to {CUDA_PUBLIC_ATTENTION_MAX_SEQUENCE} on the bounded public lane",
            )));
        }
        let expected_scale = 1.0_f32 / (head_dim as f32).sqrt();
        if (scale - expected_scale).abs() > 1e-6 {
            return Err(RuntimeError::Backend(format!(
                "cuda scaled_dot_product_attention only supports the baseline scale 1/sqrt(head_dim)={expected_scale}, actual {scale}",
            )));
        }

        let output = self.allocate(&step.spec)?;
        match (
            query.spec().dtype(),
            key.spec().dtype(),
            value.spec().dtype(),
            step.spec.dtype(),
        ) {
            (DType::F32, DType::F32, DType::F32, DType::F32) => {
                submission.attention_causal_sequence_f32(
                    query,
                    key,
                    value,
                    batch_size,
                    query_heads,
                    kv_heads,
                    sequence_length,
                    head_dim,
                    &output,
                )?;
            }
            (DType::BF16, DType::BF16, DType::BF16, DType::BF16) => {
                self.execute_scaled_dot_product_attention_bf16_gemm_forward(
                    submission,
                    query,
                    key,
                    value,
                    batch_size,
                    query_heads,
                    kv_heads,
                    sequence_length,
                    head_dim,
                    scale,
                    &output,
                )?;
            }
            (query_dtype, key_dtype, value_dtype, output_dtype) => {
                return Err(RuntimeError::Backend(format!(
                    "cuda scaled_dot_product_attention only supports contiguous F32 or BF16 query/key/value/output tensors on the bounded causal sequence lane, actual left={query_dtype:?} key={key_dtype:?} value={value_dtype:?} output={output_dtype:?}",
                )));
            }
        }
        Ok(output)
    }

    fn finish_scaled_dot_product_attention_gemm_forward(
        &self,
        submission: &mut CudaSubmission,
        scores: &CudaBuffer,
        value: &CudaBuffer,
        layout: ScaledDotProductAttentionForwardGemmLayout,
        sequence_length: usize,
        head_dim: usize,
        scale: f32,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        submission.attention_causal_row_softmax_in_place_f32(
            scores,
            layout.total_score_rows,
            sequence_length,
            scale,
        )?;
        submission.matmul_f32_bf16_strided_batched_to_f32_with_strides(
            scores,
            value,
            output,
            layout.group_rows,
            sequence_length,
            head_dim,
            layout.batch_count,
            layout.score_stride_elements,
            layout.kv_stride_elements,
            layout.output_stride_elements,
            false,
            false,
        )?;
        Ok(())
    }

    fn execute_scaled_dot_product_attention_bf16_gemm_forward(
        &self,
        submission: &mut CudaSubmission,
        query: &CudaBuffer,
        key: &CudaBuffer,
        value: &CudaBuffer,
        batch_size: usize,
        query_heads: usize,
        kv_heads: usize,
        sequence_length: usize,
        head_dim: usize,
        scale: f32,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let layout = scaled_dot_product_attention_forward_gemm_layout(
            batch_size,
            query_heads,
            kv_heads,
            sequence_length,
            head_dim,
        )?;
        let score_spec = TensorSpec::new(
            Shape::new(vec![layout.batch_count, layout.group_rows, sequence_length]),
            DType::F32,
            query.spec().device().clone(),
        );
        let scores = submission
            .allocate_scratch_buffer("scaled_dot_product_attention_gemm_scores", &score_spec)?;
        let output_f32_spec = TensorSpec::from_layout(
            output.spec().layout().clone(),
            DType::F32,
            output.spec().device().clone(),
        );
        let output_f32 = submission.allocate_scratch_buffer(
            "scaled_dot_product_attention_gemm_output_f32",
            &output_f32_spec,
        )?;
        submission.matmul_bf16_strided_batched_to_f32_with_strides(
            query,
            key,
            &scores,
            layout.group_rows,
            head_dim,
            sequence_length,
            layout.batch_count,
            layout.query_stride_elements,
            layout.kv_stride_elements,
            layout.score_stride_elements,
            false,
            true,
        )?;
        self.finish_scaled_dot_product_attention_gemm_forward(
            submission,
            &scores,
            value,
            layout,
            sequence_length,
            head_dim,
            scale,
            &output_f32,
        )?;
        submission.cast_f32_to_bf16(&output_f32, output, output.spec().storage_size())?;
        Ok(())
    }

    fn execute_scaled_dot_product_attention_bf16_gemm_backward(
        &self,
        submission: &mut CudaSubmission,
        query: &CudaBuffer,
        key: &CudaBuffer,
        value: &CudaBuffer,
        grad_output: &CudaBuffer,
        batch_size: usize,
        query_heads: usize,
        kv_heads: usize,
        sequence_length: usize,
        head_dim: usize,
        scale: f32,
    ) -> Result<ScaledDotProductAttentionBackwardCachedOutputs, RuntimeError> {
        let layout = scaled_dot_product_attention_forward_gemm_layout(
            batch_size,
            query_heads,
            kv_heads,
            sequence_length,
            head_dim,
        )?;
        let score_spec = TensorSpec::new(
            Shape::new(vec![layout.batch_count, layout.group_rows, sequence_length]),
            DType::F32,
            query.spec().device().clone(),
        );
        let scores = submission
            .allocate_scratch_buffer("scaled_dot_product_attention_gemm_scores", &score_spec)?;
        let grad_scores = submission.allocate_scratch_buffer(
            "scaled_dot_product_attention_gemm_grad_scores",
            &score_spec,
        )?;
        let query_gradient = self.allocate(query.spec())?;
        let key_gradient = self.allocate(key.spec())?;
        let value_gradient = self.allocate(value.spec())?;
        let query_gradient_f32_spec = TensorSpec::from_layout(
            query.spec().layout().clone(),
            DType::F32,
            query.spec().device().clone(),
        );
        let query_gradient_f32 = submission.allocate_scratch_buffer(
            "scaled_dot_product_attention_gemm_query_gradient_f32",
            &query_gradient_f32_spec,
        )?;
        let key_gradient_f32_spec = TensorSpec::from_layout(
            key.spec().layout().clone(),
            DType::F32,
            key.spec().device().clone(),
        );
        let key_gradient_f32 = submission.allocate_scratch_buffer(
            "scaled_dot_product_attention_gemm_key_gradient_f32",
            &key_gradient_f32_spec,
        )?;
        let value_gradient_f32_spec = TensorSpec::from_layout(
            value.spec().layout().clone(),
            DType::F32,
            value.spec().device().clone(),
        );
        let value_gradient_f32 = submission.allocate_scratch_buffer(
            "scaled_dot_product_attention_gemm_value_gradient_f32",
            &value_gradient_f32_spec,
        )?;

        submission.matmul_bf16_strided_batched_to_f32_with_strides(
            query,
            key,
            &scores,
            layout.group_rows,
            head_dim,
            sequence_length,
            layout.batch_count,
            layout.query_stride_elements,
            layout.kv_stride_elements,
            layout.score_stride_elements,
            false,
            true,
        )?;
        submission.attention_causal_row_softmax_in_place_f32(
            &scores,
            layout.total_score_rows,
            sequence_length,
            scale,
        )?;
        submission.matmul_bf16_strided_batched_to_f32_with_strides(
            grad_output,
            value,
            &grad_scores,
            layout.group_rows,
            head_dim,
            sequence_length,
            layout.batch_count,
            layout.query_stride_elements,
            layout.kv_stride_elements,
            layout.score_stride_elements,
            false,
            true,
        )?;
        submission.attention_causal_row_softmax_backward_in_place_f32(
            &scores,
            &grad_scores,
            layout.total_score_rows,
            sequence_length,
            scale,
        )?;
        submission.matmul_f32_bf16_strided_batched_to_f32_with_strides(
            &grad_scores,
            key,
            &query_gradient_f32,
            layout.group_rows,
            sequence_length,
            head_dim,
            layout.batch_count,
            layout.score_stride_elements,
            layout.kv_stride_elements,
            layout.output_stride_elements,
            false,
            false,
        )?;
        submission.matmul_f32_bf16_strided_batched_to_f32_with_strides(
            &grad_scores,
            query,
            &key_gradient_f32,
            sequence_length,
            layout.group_rows,
            head_dim,
            layout.batch_count,
            layout.score_stride_elements,
            layout.query_stride_elements,
            layout.kv_stride_elements,
            true,
            false,
        )?;
        submission.matmul_f32_bf16_strided_batched_to_f32_with_strides(
            &scores,
            grad_output,
            &value_gradient_f32,
            sequence_length,
            layout.group_rows,
            head_dim,
            layout.batch_count,
            layout.score_stride_elements,
            layout.query_stride_elements,
            layout.kv_stride_elements,
            true,
            false,
        )?;
        submission.cast_f32_to_bf16(
            &query_gradient_f32,
            &query_gradient,
            query_gradient.spec().storage_size(),
        )?;
        submission.cast_f32_to_bf16(
            &key_gradient_f32,
            &key_gradient,
            key_gradient.spec().storage_size(),
        )?;
        submission.cast_f32_to_bf16(
            &value_gradient_f32,
            &value_gradient,
            value_gradient.spec().storage_size(),
        )?;

        Ok(ScaledDotProductAttentionBackwardCachedOutputs {
            query: query_gradient,
            key: key_gradient,
            value: value_gradient,
            _scratch: Vec::new(),
        })
    }

    fn execute_rotary_embedding_backward_step(
        &self,
        submission: &mut CudaSubmission,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
        interleaved: bool,
    ) -> Result<CudaBuffer, RuntimeError> {
        if interleaved {
            return Err(RuntimeError::Backend(String::from(
                "cuda rotary_embedding_backward only supports the non-interleaved NEOX-style lane",
            )));
        }
        let grad_output = step_input(step, values, 0)?;
        let cos = step_input(step, values, 1)?;
        let sin = step_input(step, values, 2)?;
        let grad_dims = grad_output.spec().shape().dims();
        if grad_dims.len() != 4 || grad_dims[3] == 0 || !grad_dims[3].is_multiple_of(2) {
            return Err(RuntimeError::Backend(String::from(
                "cuda rotary_embedding_backward requires rank-4 tensors with an even head dimension",
            )));
        }
        let batch_size = grad_dims[0];
        let head_count = grad_dims[1];
        let sequence_length = grad_dims[2];
        let head_dim = grad_dims[3];
        let half_dim = head_dim / 2;
        let batched_tables =
            validate_standard_neox_rope_tables(cos, sin, batch_size, sequence_length, half_dim)?;
        let output = self.allocate(&step.spec)?;
        submission.rotary_embedding_backward(
            grad_output,
            cos,
            sin,
            batch_size,
            head_count,
            sequence_length,
            head_dim,
            batched_tables,
            &output,
        )?;
        Ok(output)
    }

    fn execute_scaled_dot_product_attention_backward_step(
        &self,
        submission: &mut CudaSubmission,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, CudaBuffer>,
        attention_backward_cache: &mut BTreeMap<
            ScaledDotProductAttentionBackwardCacheKey,
            ScaledDotProductAttentionBackwardCachedOutputs,
        >,
        scale: f32,
        causal: bool,
        target: ScaledDotProductAttentionBackwardTarget,
    ) -> Result<CudaBuffer, RuntimeError> {
        let cache_key = scaled_dot_product_attention_backward_cache_key(step, scale, causal)?;
        if let Some(cached) = attention_backward_cache.get(&cache_key) {
            return Ok(cached.output(target));
        }
        if !causal {
            return Err(RuntimeError::Backend(String::from(
                "cuda scaled_dot_product_attention backward only supports the bounded causal self-attention lane",
            )));
        }
        let query = step_input(step, values, 0)?;
        let key = step_input(step, values, 1)?;
        let value = step_input(step, values, 2)?;
        let grad_output = step_input(step, values, 3)?;
        let query_dims = query.spec().shape().dims();
        let key_dims = key.spec().shape().dims();
        let value_dims = value.spec().shape().dims();
        let grad_output_dims = grad_output.spec().shape().dims();
        if query_dims.len() != 4
            || key_dims.len() != 4
            || value_dims.len() != 4
            || grad_output_dims.len() != 4
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda scaled_dot_product_attention backward requires rank-4 query/key/value/grad_output tensors",
            )));
        }
        let batch_size = query_dims[0];
        let query_heads = query_dims[1];
        let sequence_length = query_dims[2];
        let head_dim = query_dims[3];
        let kv_heads = key_dims[1];
        if batch_size == 0
            || query_heads == 0
            || kv_heads == 0
            || head_dim == 0
            || sequence_length == 0
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda scaled_dot_product_attention backward requires non-empty batch, head, sequence, and head-dimension extents",
            )));
        }
        if batch_size != key_dims[0]
            || batch_size != value_dims[0]
            || batch_size != grad_output_dims[0]
            || sequence_length != key_dims[2]
            || sequence_length != value_dims[2]
            || sequence_length != grad_output_dims[2]
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda scaled_dot_product_attention backward only supports bounded causal self-attention with matching batch and sequence extents",
            )));
        }
        if head_dim != key_dims[3] || head_dim != value_dims[3] || head_dim != grad_output_dims[3] {
            return Err(RuntimeError::Backend(String::from(
                "cuda scaled_dot_product_attention backward requires value, key, and grad_output head dimensions to match query head dimensions on the bounded lane",
            )));
        }
        if query_heads != grad_output_dims[1]
            || kv_heads != value_dims[1]
            || !query_heads.is_multiple_of(kv_heads)
        {
            return Err(RuntimeError::Backend(String::from(
                "cuda scaled_dot_product_attention backward requires grouped-query head counts with query_heads divisible by kv_heads",
            )));
        }
        if sequence_length > CUDA_PUBLIC_ATTENTION_MAX_SEQUENCE {
            return Err(RuntimeError::Backend(format!(
                "cuda scaled_dot_product_attention backward only supports sequence lengths up to {CUDA_PUBLIC_ATTENTION_MAX_SEQUENCE} on the bounded public lane",
            )));
        }
        let expected_scale = 1.0_f32 / (head_dim as f32).sqrt();
        if (scale - expected_scale).abs() > 1e-6 {
            return Err(RuntimeError::Backend(format!(
                "cuda scaled_dot_product_attention backward only supports the baseline scale 1/sqrt(head_dim)={expected_scale}, actual {scale}",
            )));
        }

        let cached = if query.spec().dtype() == DType::F32
            && key.spec().dtype() == DType::F32
            && value.spec().dtype() == DType::F32
            && grad_output.spec().dtype() == DType::F32
        {
            let query_gradient = self.allocate(query.spec())?;
            let key_gradient = self.allocate(key.spec())?;
            let value_gradient = self.allocate(value.spec())?;
            submission.fill_buffer(&key_gradient, 0)?;
            submission.fill_buffer(&value_gradient, 0)?;
            submission.attention_causal_sequence_backward_f32(
                query,
                key,
                value,
                grad_output,
                batch_size,
                query_heads,
                kv_heads,
                sequence_length,
                head_dim,
                &query_gradient,
                &key_gradient,
                &value_gradient,
            )?;
            ScaledDotProductAttentionBackwardCachedOutputs {
                query: query_gradient,
                key: key_gradient,
                value: value_gradient,
                _scratch: Vec::new(),
            }
        } else if query.spec().dtype() == DType::BF16
            && key.spec().dtype() == DType::BF16
            && value.spec().dtype() == DType::BF16
            && grad_output.spec().dtype() == DType::BF16
        {
            self.execute_scaled_dot_product_attention_bf16_gemm_backward(
                submission,
                query,
                key,
                value,
                grad_output,
                batch_size,
                query_heads,
                kv_heads,
                sequence_length,
                head_dim,
                scale,
            )?
        } else {
            let query_values = query.read_f32()?;
            let key_values = key.read_f32()?;
            let value_values = value.read_f32()?;
            let grad_output_values = grad_output.read_f32()?;
            let gradients = scaled_dot_product_attention_backward_host_values(
                &query_values,
                query_dims,
                &key_values,
                key_dims,
                &value_values,
                value_dims,
                &grad_output_values,
                scale,
            )?;
            ScaledDotProductAttentionBackwardCachedOutputs {
                query: self
                    .buffer_from_tensor_data(query.spec(), &TensorData::F32(gradients.query))?,
                key: self.buffer_from_tensor_data(key.spec(), &TensorData::F32(gradients.key))?,
                value: self
                    .buffer_from_tensor_data(value.spec(), &TensorData::F32(gradients.value))?,
                _scratch: Vec::new(),
            }
        };
        let output = cached.output(target);
        attention_backward_cache.insert(cache_key, cached);
        Ok(output)
    }

    fn execute(
        &self,
        plan: &ExecutionPlan,
        inputs: &BTreeMap<TensorId, CudaBuffer>,
    ) -> Result<ExecutionResult<CudaBuffer>, RuntimeError> {
        let mut submission = CudaSubmission {
            encoded_operations: 0,
            capturing: false,
            retained_buffers: Vec::new(),
            scratch_buffers: Vec::new(),
            platform: self.platform.begin_submission()?,
        };
        let mut values = BTreeMap::new();
        let mut retain_counts = execution_value_retain_counts(plan);
        let profile_path = host_fallback_profile_path();
        let mut host_fallback_profile = profile_path
            .as_ref()
            .map(|_| CudaHostFallbackExecutionProfile::default());
        let mut attention_backward_cache: BTreeMap<
            ScaledDotProductAttentionBackwardCacheKey,
            ScaledDotProductAttentionBackwardCachedOutputs,
        > = BTreeMap::new();

        for step in &plan.steps {
            match &step.op {
                ExecutionOp::Input { .. } => {
                    let input = inputs
                        .get(&step.output)
                        .ok_or(RuntimeError::MissingInput(step.output))?;
                    if input.spec() != &step.spec {
                        return Err(RuntimeError::InvalidBuffer {
                            tensor: step.output,
                            expected: step.spec.clone(),
                            actual: input.spec().clone(),
                        });
                    }
                    values.insert(step.output, input.clone());
                }
                ExecutionOp::Constant { data } => {
                    values.insert(step.output, self.buffer_from_tensor_data(&step.spec, data)?);
                }
                ExecutionOp::Detach => {
                    let input = step_input(step, &values, 0)?;
                    let output = input.alias_with_spec(&step.spec)?;
                    values.insert(step.output, output);
                }
                ExecutionOp::Reshape => {
                    let input = step_input(step, &values, 0)?;
                    let output = input.alias_with_spec(&step.spec)?;
                    values.insert(step.output, output);
                }
                ExecutionOp::Permute { axes } => {
                    let input = step_input(step, &values, 0)?;
                    let output = if input.spec().dtype() == DType::F32
                        && step.spec.dtype() == DType::F32
                        && supports_cuda_rank2_transpose(input.spec().shape().dims(), axes)
                    {
                        let dims = input.spec().shape().dims();
                        let output = self.allocate(&step.spec)?;
                        submission.permute_rank2_transpose_f32(input, &output, dims[0], dims[1])?;
                        output
                    } else if input.spec().dtype() == DType::F32
                        && step.spec.dtype() == DType::F32
                        && supports_cuda_rank4_swap_middle_axes(input.spec().shape().dims(), axes)
                    {
                        let dims = input.spec().shape().dims();
                        let output = self.allocate(&step.spec)?;
                        submission.permute_rank4_swap_middle_axes_f32(
                            input, &output, dims[0], dims[1], dims[2], dims[3],
                        )?;
                        output
                    } else {
                        execute_profiled_host_fallback(&mut host_fallback_profile, step, || {
                            let input_values = self.read_float_buffer_to_f32(input)?;
                            let values_out = permute_contiguous_values(
                                &input_values,
                                input.spec().shape().dims(),
                                axes,
                            )?;
                            self.materialize_host_view_step(step, &values_out)
                        })?
                    };
                    values.insert(step.output, output);
                }
                ExecutionOp::Slice { axis, start, end } => {
                    let output =
                        execute_profiled_host_fallback(&mut host_fallback_profile, step, || {
                            let input = step_input(step, &values, 0)?;
                            let input_values = self.read_float_buffer_to_f32(input)?;
                            let values_out = slice_contiguous_values(
                                &input_values,
                                input.spec().shape().dims(),
                                *axis,
                                *start,
                                *end,
                            )?;
                            self.materialize_host_view_step(step, &values_out)
                        })?;
                    values.insert(step.output, output);
                }
                ExecutionOp::Select { axis, index } => {
                    let output =
                        execute_profiled_host_fallback(&mut host_fallback_profile, step, || {
                            let input = step_input(step, &values, 0)?;
                            let input_values = self.read_float_buffer_to_f32(input)?;
                            let values_out = select_contiguous_values(
                                &input_values,
                                input.spec().shape().dims(),
                                *axis,
                                *index,
                            )?;
                            self.materialize_host_view_step(step, &values_out)
                        })?;
                    values.insert(step.output, output);
                }
                ExecutionOp::Concat { axis } => {
                    let output =
                        execute_profiled_host_fallback(&mut host_fallback_profile, step, || {
                            let tensors = step
                                .inputs
                                .iter()
                                .map(|tensor_id| {
                                    values
                                        .get(tensor_id)
                                        .ok_or(RuntimeError::MissingInput(*tensor_id))
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            let value_slices = tensors
                                .iter()
                                .map(|buffer| self.read_float_buffer_to_f32(buffer))
                                .collect::<Result<Vec<_>, _>>()?;
                            let shape_slices = tensors
                                .iter()
                                .map(|buffer| buffer.spec().shape().dims().to_vec())
                                .collect::<Vec<_>>();
                            let value_refs =
                                value_slices.iter().map(Vec::as_slice).collect::<Vec<_>>();
                            let shape_refs =
                                shape_slices.iter().map(Vec::as_slice).collect::<Vec<_>>();
                            let values_out = concat_contiguous_values(
                                value_refs.as_slice(),
                                shape_refs.as_slice(),
                                *axis,
                            )?;
                            self.materialize_host_view_step(step, &values_out)
                        })?;
                    values.insert(step.output, output);
                }
                ExecutionOp::Expand { shape } => {
                    let input = step_input(step, &values, 0)?;
                    let output = if input.spec().dtype() == DType::F32
                        && step.spec.dtype() == DType::F32
                        && supports_cuda_rank3_expand_f32(input.spec().shape().dims(), shape.dims())
                    {
                        let dims = input.spec().shape().dims();
                        let contiguous = TensorSpec::new(
                            step.spec.shape().clone(),
                            step.spec.dtype(),
                            step.spec.device().clone(),
                        );
                        let output = self.allocate(&contiguous)?;
                        submission.expand_rank3_f32(
                            input,
                            &output,
                            [dims[0], dims[1], dims[2]],
                            [shape.dims()[0], shape.dims()[1], shape.dims()[2]],
                        )?;
                        output
                    } else if input.spec().dtype() == DType::F32
                        && step.spec.dtype() == DType::F32
                        && supports_cuda_rank4_expand_f32(input.spec().shape().dims(), shape.dims())
                    {
                        let dims = input.spec().shape().dims();
                        let contiguous = TensorSpec::new(
                            step.spec.shape().clone(),
                            step.spec.dtype(),
                            step.spec.device().clone(),
                        );
                        let output = self.allocate(&contiguous)?;
                        submission.expand_rank4_f32(
                            input,
                            &output,
                            [dims[0], dims[1], dims[2], dims[3]],
                            [
                                shape.dims()[0],
                                shape.dims()[1],
                                shape.dims()[2],
                                shape.dims()[3],
                            ],
                        )?;
                        output
                    } else {
                        execute_profiled_host_fallback(&mut host_fallback_profile, step, || {
                            let input = step_input(step, &values, 0)?;
                            let input_values = self.read_float_buffer_to_f32(input)?;
                            let values_out = expand_contiguous_values(
                                &input_values,
                                input.spec().shape().dims(),
                                shape.dims(),
                            )?;
                            self.materialize_host_view_step(step, &values_out)
                        })?
                    };
                    values.insert(step.output, output);
                }
                ExecutionOp::Cast { dtype } => {
                    let input = step_input(step, &values, 0)?;
                    let output = if input.spec().dtype() == *dtype {
                        input.alias_with_spec(&step.spec)?
                    } else {
                        match (input.spec().dtype(), *dtype) {
                            (DType::F32, DType::BF16) => {
                                let output = self.allocate(&step.spec)?;
                                submission.cast_f32_to_bf16(
                                    input,
                                    &output,
                                    input.spec().element_count(),
                                )?;
                                output
                            }
                            (DType::BF16, DType::F32) => {
                                let output = self.allocate(&step.spec)?;
                                submission.cast_bf16_to_f32(
                                    input,
                                    &output,
                                    input.spec().element_count(),
                                )?;
                                output
                            }
                            _ => execute_profiled_host_fallback(
                                &mut host_fallback_profile,
                                step,
                                || {
                                    let input_values = self.read_float_buffer_to_f32(input)?;
                                    let values_out = cast_contiguous_values(
                                        &input_values,
                                        input.spec().dtype(),
                                        *dtype,
                                    )?;
                                    self.materialize_host_view_step(step, &values_out)
                                },
                            )?,
                        }
                    };
                    values.insert(step.output, output);
                }
                ExecutionOp::ReduceSum { axis } => {
                    let input = step_input(step, &values, 0)?;
                    let output = if input.spec().dtype() == DType::F32
                        && step.spec.dtype() == DType::F32
                    {
                        match reduce_sum_cuda_strategy(input.spec().shape().dims(), *axis) {
                            Some(ReduceSumCudaStrategy::Rows {
                                row_count,
                                column_count,
                            }) => {
                                let output = self.allocate(&step.spec)?;
                                submission.reduce_sum_rows_f32(
                                    input,
                                    &output,
                                    row_count,
                                    column_count,
                                )?;
                                output
                            }
                            Some(ReduceSumCudaStrategy::Axis0 {
                                axis0_extent,
                                row_width,
                            }) => {
                                let output = self.allocate(&step.spec)?;
                                submission.reduce_sum_axis0_f32(
                                    input,
                                    &output,
                                    axis0_extent,
                                    row_width,
                                )?;
                                output
                            }
                            Some(ReduceSumCudaStrategy::Axis1Rank3 { dim0, dim1, dim2 }) => {
                                let output = self.allocate(&step.spec)?;
                                submission
                                    .reduce_sum_axis1_rank3_f32(input, &output, dim0, dim1, dim2)?;
                                output
                            }
                            None => execute_profiled_host_fallback(
                                &mut host_fallback_profile,
                                step,
                                || {
                                    let input_values = self.read_float_buffer_to_f32(input)?;
                                    let values_out = reduce_sum_contiguous_values(
                                        &input_values,
                                        input.spec().shape().dims(),
                                        *axis,
                                    )?;
                                    self.materialize_host_view_step(step, &values_out)
                                },
                            )?,
                        }
                    } else {
                        execute_profiled_host_fallback(&mut host_fallback_profile, step, || {
                            let input_values = self.read_float_buffer_to_f32(input)?;
                            let values_out = reduce_sum_contiguous_values(
                                &input_values,
                                input.spec().shape().dims(),
                                *axis,
                            )?;
                            self.materialize_host_view_step(step, &values_out)
                        })?
                    };
                    values.insert(step.output, output);
                }
                ExecutionOp::Add => {
                    let left = step_input(step, &values, 0)?;
                    let right = step_input(step, &values, 1)?;
                    let output = if left.spec() == &step.spec
                        && right.spec() == &step.spec
                        && left.spec().shape() == step.spec.shape()
                        && right.spec().shape() == step.spec.shape()
                        && left.spec().dtype() == DType::F32
                        && right.spec().dtype() == DType::F32
                    {
                        let contiguous = TensorSpec::new(
                            step.spec.shape().clone(),
                            step.spec.dtype(),
                            step.spec.device().clone(),
                        );
                        let left = if left.spec() == &contiguous {
                            left.clone()
                        } else {
                            self.buffer_from_tensor_data(
                                &contiguous,
                                &TensorData::F32(self.read_float_buffer_to_f32(left)?),
                            )?
                        };
                        let right = if right.spec() == &contiguous {
                            right.clone()
                        } else {
                            self.buffer_from_tensor_data(
                                &contiguous,
                                &TensorData::F32(self.read_float_buffer_to_f32(right)?),
                            )?
                        };
                        let output = self.allocate(&contiguous)?;
                        submission.platform.encode_add(
                            &left.platform,
                            &right.platform,
                            &output.platform,
                            contiguous.element_count(),
                        )?;
                        submission.encoded_operations += 1;
                        output
                    } else {
                        execute_profiled_host_fallback(&mut host_fallback_profile, step, || {
                            let left_values = self.read_float_buffer_to_f32(left)?;
                            let right_values = self.read_float_buffer_to_f32(right)?;
                            let values_out = broadcast_binary_float_values(
                                &left_values,
                                left.spec().shape().dims(),
                                &right_values,
                                right.spec().shape().dims(),
                                step.spec.shape().dims(),
                                |left, right| left + right,
                            )?;
                            self.materialize_host_view_step(step, &values_out)
                        })?
                    };
                    values.insert(step.output, output);
                }
                ExecutionOp::Mul => {
                    let left = step_input(step, &values, 0)?;
                    let right = step_input(step, &values, 1)?;
                    let output = if left.spec() == &step.spec
                        && right.spec() == &step.spec
                        && left.spec().dtype() == DType::F32
                        && right.spec().dtype() == DType::F32
                    {
                        let output = self.allocate(&step.spec)?;
                        submission.platform.encode_mul(
                            &left.platform,
                            &right.platform,
                            &output.platform,
                            step.spec.element_count(),
                        )?;
                        submission.encoded_operations += 1;
                        output
                    } else {
                        execute_profiled_host_fallback(&mut host_fallback_profile, step, || {
                            let left_values = self.read_float_buffer_to_f32(left)?;
                            let right_values = self.read_float_buffer_to_f32(right)?;
                            let values_out = broadcast_binary_float_values(
                                &left_values,
                                left.spec().shape().dims(),
                                &right_values,
                                right.spec().shape().dims(),
                                step.spec.shape().dims(),
                                |left, right| left * right,
                            )?;
                            self.materialize_host_view_step(step, &values_out)
                        })?
                    };
                    values.insert(step.output, output);
                }
                ExecutionOp::Matmul => {
                    let (left, right) = binary_inputs(step, &values)?;
                    let left_dims = left.spec().shape().dims();
                    let right_dims = right.spec().shape().dims();
                    if left_dims.len() != 2
                        || right_dims.len() != 2
                        || left_dims[1] != right_dims[0]
                    {
                        return Err(RuntimeError::Backend(String::from(
                            "invalid matmul shapes at runtime",
                        )));
                    }
                    let output = self.allocate(&step.spec)?;
                    match (left.spec().dtype(), right.spec().dtype(), step.spec.dtype()) {
                        (DType::F32, DType::F32, DType::F32) => {
                            submission.platform.encode_matmul(
                                &left.platform,
                                &right.platform,
                                &output.platform,
                                left_dims[0],
                                left_dims[1],
                                right_dims[1],
                            )?;
                        }
                        (DType::F16, DType::F16, DType::F32) => {
                            submission.platform.encode_matmul_f16_to_f32(
                                &left.platform,
                                &right.platform,
                                &output.platform,
                                left_dims[0],
                                left_dims[1],
                                right_dims[1],
                            )?;
                        }
                        (DType::BF16, DType::BF16, DType::F32) => {
                            submission.platform.encode_matmul_bf16_to_f32(
                                &left.platform,
                                &right.platform,
                                &output.platform,
                                left_dims[0],
                                left_dims[1],
                                right_dims[1],
                            )?;
                        }
                        (DType::F32, DType::BF16, DType::F32) => {
                            let left_bf16 = self.allocate(&TensorSpec::new(
                                left.spec().shape().clone(),
                                DType::BF16,
                                left.spec().device().clone(),
                            ))?;
                            submission.cast_f32_to_bf16(
                                left,
                                &left_bf16,
                                left.spec().element_count(),
                            )?;
                            submission.matmul_bf16_to_f32(
                                &left_bf16,
                                right,
                                &output,
                                left_dims[0],
                                left_dims[1],
                                right_dims[1],
                            )?;
                        }
                        (left_dtype, right_dtype, output_dtype) => {
                            return Err(RuntimeError::Backend(format!(
                                "cuda matmul does not support dtype posture left={left_dtype:?} right={right_dtype:?} output={output_dtype:?}",
                            )));
                        }
                    }
                    submission.encoded_operations += 1;
                    values.insert(step.output, output);
                }
                ExecutionOp::BackendExtension { op } => match op {
                    BackendExtensionOp::ParameterGolfTokenEmbeddingLookup => {
                        let output = self.execute_parameter_golf_token_embedding_lookup_step(
                            &mut submission,
                            step,
                            &values,
                        )?;
                        values.insert(step.output, output);
                    }
                    BackendExtensionOp::ParameterGolfBankedLinear { bank_index } => {
                        let output = self.execute_parameter_golf_banked_linear_step(
                            &mut submission,
                            step,
                            &values,
                            *bank_index,
                        )?;
                        values.insert(step.output, output);
                    }
                    BackendExtensionOp::ParameterGolfBankedLinearInputBackward { bank_index } => {
                        let output = self
                            .execute_parameter_golf_banked_linear_input_backward_step(
                                &mut submission,
                                step,
                                &values,
                                *bank_index,
                            )?;
                        values.insert(step.output, output);
                    }
                    BackendExtensionOp::ParameterGolfBankedLinearWeightBackward { bank_index } => {
                        let output = self
                            .execute_parameter_golf_banked_linear_weight_backward_step(
                                &mut submission,
                                step,
                                &values,
                                *bank_index,
                            )?;
                        values.insert(step.output, output);
                    }
                    BackendExtensionOp::ParameterGolfTokenEmbeddingLookupBackward => {
                        let output = self
                            .execute_parameter_golf_token_embedding_lookup_backward_step(
                                &mut submission,
                                step,
                                &values,
                            )?;
                        values.insert(step.output, output);
                    }
                    BackendExtensionOp::ReluSquared => {
                        values.insert(step.output, self.execute_relu_squared_step(step, &values)?);
                    }
                    BackendExtensionOp::ReluSquaredBackward => {
                        values.insert(
                            step.output,
                            self.execute_relu_squared_backward_step(step, &values)?,
                        );
                    }
                    BackendExtensionOp::ReluSquaredBackwardFromOutput => {
                        values.insert(
                            step.output,
                            self.execute_relu_squared_backward_from_output_step(step, &values)?,
                        );
                    }
                    BackendExtensionOp::LeakyReluSquared { negative_slope } => {
                        values.insert(
                            step.output,
                            self.execute_leaky_relu_squared_step(
                                step,
                                &values,
                                negative_slope.to_f32(),
                            )?,
                        );
                    }
                    BackendExtensionOp::LeakyReluSquaredBackward { negative_slope } => {
                        values.insert(
                            step.output,
                            self.execute_leaky_relu_squared_backward_step(
                                step,
                                &values,
                                negative_slope.to_f32(),
                            )?,
                        );
                    }
                    BackendExtensionOp::Silu => {
                        values.insert(step.output, self.execute_silu_step(step, &values)?);
                    }
                    BackendExtensionOp::SiluBackward => {
                        values.insert(step.output, self.execute_silu_backward_step(step, &values)?);
                    }
                    BackendExtensionOp::ParameterGolfProjectionLoss { logit_softcap } => {
                        let output = self.execute_parameter_golf_projection_loss_step(
                            &mut submission,
                            step,
                            &values,
                            logit_softcap.to_f32(),
                        )?;
                        values.insert(step.output, output);
                    }
                    BackendExtensionOp::ParameterGolfProjectionTokenLosses { logit_softcap } => {
                        let output = self.execute_parameter_golf_projection_token_losses_step(
                            &mut submission,
                            step,
                            &values,
                            logit_softcap.to_f32(),
                        )?;
                        values.insert(step.output, output);
                    }
                    BackendExtensionOp::ParameterGolfProjectionLossBackward { logit_softcap } => {
                        let output = self.execute_parameter_golf_projection_loss_backward_step(
                            &mut submission,
                            step,
                            &values,
                            logit_softcap.to_f32(),
                        )?;
                        values.insert(step.output, output);
                    }
                    BackendExtensionOp::RmsNorm { epsilon } => {
                        let input = step_input(step, &values, 0)?;
                        let weight = step_input(step, &values, 1)?;
                        let feature_count = weight.spec().storage_size();
                        if feature_count == 0 {
                            return Err(RuntimeError::Backend(String::from(
                                "cuda rms_norm requires a non-empty weight tensor",
                            )));
                        }
                        let element_count = step.spec.storage_size();
                        if input.spec().storage_size() < element_count {
                            return Err(RuntimeError::Backend(String::from(
                                "cuda rms_norm requires an input buffer large enough for the output size",
                            )));
                        }
                        if element_count % feature_count != 0 {
                            return Err(RuntimeError::Backend(String::from(
                                "cuda rms_norm requires output size to be a multiple of the weight size",
                            )));
                        }
                        let output = self.allocate(&step.spec)?;
                        submission.platform.encode_rms_norm(
                            &input.platform,
                            &weight.platform,
                            &output.platform,
                            element_count,
                            feature_count,
                            epsilon.to_f32(),
                        )?;
                        submission.encoded_operations += 1;
                        values.insert(step.output, output);
                    }
                    BackendExtensionOp::RmsNormInputBackward { epsilon } => {
                        let input = step_input(step, &values, 0)?;
                        let weight = step_input(step, &values, 1)?;
                        let grad_output = step_input(step, &values, 2)?;
                        let feature_count = weight.spec().storage_size();
                        if feature_count == 0 {
                            return Err(RuntimeError::Backend(String::from(
                                "cuda rms_norm_input_backward requires a non-empty weight tensor",
                            )));
                        }
                        let element_count = step.spec.storage_size();
                        if input.spec().storage_size() != element_count
                            || grad_output.spec().storage_size() != element_count
                        {
                            return Err(RuntimeError::Backend(String::from(
                                "cuda rms_norm_input_backward requires matching input, grad_output, and output sizes",
                            )));
                        }
                        if element_count % feature_count != 0 {
                            return Err(RuntimeError::Backend(String::from(
                                "cuda rms_norm_input_backward requires output size to be a multiple of the weight size",
                            )));
                        }
                        let output = self.allocate(&step.spec)?;
                        submission.platform.encode_rms_norm_input_backward(
                            &input.platform,
                            &weight.platform,
                            &grad_output.platform,
                            &output.platform,
                            element_count,
                            feature_count,
                            epsilon.to_f32(),
                        )?;
                        submission.encoded_operations += 1;
                        values.insert(step.output, output);
                    }
                    BackendExtensionOp::RmsNormWeightBackward { epsilon } => {
                        let input = step_input(step, &values, 0)?;
                        let grad_output = step_input(step, &values, 1)?;
                        let feature_count = step.spec.storage_size();
                        if feature_count == 0 {
                            return Err(RuntimeError::Backend(String::from(
                                "cuda rms_norm_weight_backward requires a non-empty output tensor",
                            )));
                        }
                        let element_count = input.spec().storage_size();
                        if grad_output.spec().storage_size() != element_count {
                            return Err(RuntimeError::Backend(String::from(
                                "cuda rms_norm_weight_backward requires matching input and grad_output sizes",
                            )));
                        }
                        if element_count % feature_count != 0 {
                            return Err(RuntimeError::Backend(String::from(
                                "cuda rms_norm_weight_backward requires input size to be a multiple of the output size",
                            )));
                        }
                        let output = self.allocate(&step.spec)?;
                        submission.platform.encode_rms_norm_weight_backward(
                            &input.platform,
                            &grad_output.platform,
                            &output.platform,
                            element_count,
                            feature_count,
                            epsilon.to_f32(),
                        )?;
                        submission.encoded_operations += 1;
                        values.insert(step.output, output);
                    }
                    BackendExtensionOp::RotaryEmbedding { interleaved } => {
                        let output = self.execute_rotary_embedding_step(
                            &mut submission,
                            step,
                            &values,
                            *interleaved,
                        )?;
                        values.insert(step.output, output);
                    }
                    BackendExtensionOp::RotaryEmbeddingBackward { interleaved } => {
                        let output = self.execute_rotary_embedding_backward_step(
                            &mut submission,
                            step,
                            &values,
                            *interleaved,
                        )?;
                        values.insert(step.output, output);
                    }
                    BackendExtensionOp::ScaledDotProductAttention { scale, causal } => {
                        let output = self.execute_scaled_dot_product_attention_step(
                            &mut submission,
                            step,
                            &values,
                            scale.to_f32(),
                            *causal,
                        )?;
                        values.insert(step.output, output);
                    }
                    BackendExtensionOp::ScaledDotProductAttentionQueryBackward {
                        scale,
                        causal,
                    } => {
                        let scale = scale.to_f32();
                        let cache_key =
                            scaled_dot_product_attention_backward_cache_key(step, scale, *causal)?;
                        let use_device_path =
                            supports_cuda_scaled_dot_product_attention_backward_device_path(
                                step, &values,
                            )?;
                        let output = if let Some(cached) = attention_backward_cache.get(&cache_key)
                        {
                            cached.output(ScaledDotProductAttentionBackwardTarget::Query)
                        } else if use_device_path {
                            self.execute_scaled_dot_product_attention_backward_step(
                                &mut submission,
                                step,
                                &values,
                                &mut attention_backward_cache,
                                scale,
                                *causal,
                                ScaledDotProductAttentionBackwardTarget::Query,
                            )?
                        } else {
                            execute_profiled_host_fallback(
                                &mut host_fallback_profile,
                                step,
                                || {
                                    self.execute_scaled_dot_product_attention_backward_step(
                                        &mut submission,
                                        step,
                                        &values,
                                        &mut attention_backward_cache,
                                        scale,
                                        *causal,
                                        ScaledDotProductAttentionBackwardTarget::Query,
                                    )
                                },
                            )?
                        };
                        values.insert(step.output, output);
                    }
                    BackendExtensionOp::ScaledDotProductAttentionKeyBackward { scale, causal } => {
                        let scale = scale.to_f32();
                        let cache_key =
                            scaled_dot_product_attention_backward_cache_key(step, scale, *causal)?;
                        let use_device_path =
                            supports_cuda_scaled_dot_product_attention_backward_device_path(
                                step, &values,
                            )?;
                        let output = if let Some(cached) = attention_backward_cache.get(&cache_key)
                        {
                            cached.output(ScaledDotProductAttentionBackwardTarget::Key)
                        } else if use_device_path {
                            self.execute_scaled_dot_product_attention_backward_step(
                                &mut submission,
                                step,
                                &values,
                                &mut attention_backward_cache,
                                scale,
                                *causal,
                                ScaledDotProductAttentionBackwardTarget::Key,
                            )?
                        } else {
                            execute_profiled_host_fallback(
                                &mut host_fallback_profile,
                                step,
                                || {
                                    self.execute_scaled_dot_product_attention_backward_step(
                                        &mut submission,
                                        step,
                                        &values,
                                        &mut attention_backward_cache,
                                        scale,
                                        *causal,
                                        ScaledDotProductAttentionBackwardTarget::Key,
                                    )
                                },
                            )?
                        };
                        values.insert(step.output, output);
                    }
                    BackendExtensionOp::ScaledDotProductAttentionValueBackward {
                        scale,
                        causal,
                    } => {
                        let scale = scale.to_f32();
                        let cache_key =
                            scaled_dot_product_attention_backward_cache_key(step, scale, *causal)?;
                        let use_device_path =
                            supports_cuda_scaled_dot_product_attention_backward_device_path(
                                step, &values,
                            )?;
                        let output = if let Some(cached) = attention_backward_cache.get(&cache_key)
                        {
                            cached.output(ScaledDotProductAttentionBackwardTarget::Value)
                        } else if use_device_path {
                            self.execute_scaled_dot_product_attention_backward_step(
                                &mut submission,
                                step,
                                &values,
                                &mut attention_backward_cache,
                                scale,
                                *causal,
                                ScaledDotProductAttentionBackwardTarget::Value,
                            )?
                        } else {
                            execute_profiled_host_fallback(
                                &mut host_fallback_profile,
                                step,
                                || {
                                    self.execute_scaled_dot_product_attention_backward_step(
                                        &mut submission,
                                        step,
                                        &values,
                                        &mut attention_backward_cache,
                                        scale,
                                        *causal,
                                        ScaledDotProductAttentionBackwardTarget::Value,
                                    )
                                },
                            )?
                        };
                        values.insert(step.output, output);
                    }
                    _ => {
                        return Err(RuntimeError::UnsupportedStep(step.op.label().to_string()));
                    }
                },
            }
            release_dead_execution_values(&mut values, &mut retain_counts, step);
        }

        let _report = submission.commit(CudaCommandWait::Completed)?;
        if let (Some(path), Some(profile)) =
            (profile_path.as_deref(), host_fallback_profile.as_ref())
        {
            let _ = append_host_fallback_profile_report(path, plan, profile);
        }
        let mut outputs = BTreeMap::new();
        for output_id in &plan.outputs {
            let Some(buffer) = values.remove(output_id) else {
                return Err(RuntimeError::MissingInput(*output_id));
            };
            outputs.insert(*output_id, buffer);
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

fn execution_value_retain_counts(plan: &ExecutionPlan) -> BTreeMap<TensorId, usize> {
    let mut retain_counts = BTreeMap::new();
    for step in &plan.steps {
        for input in &step.inputs {
            *retain_counts.entry(*input).or_insert(0) += 1;
        }
    }
    for output in &plan.outputs {
        *retain_counts.entry(*output).or_insert(0) += 1;
    }
    retain_counts
}

fn release_dead_execution_values(
    values: &mut BTreeMap<TensorId, CudaBuffer>,
    retain_counts: &mut BTreeMap<TensorId, usize>,
    step: &ExecutionStep,
) {
    for input in &step.inputs {
        let Some(count) = retain_counts.get_mut(input) else {
            continue;
        };
        *count = count.saturating_sub(1);
        if *count == 0 {
            retain_counts.remove(input);
            values.remove(input);
        }
    }

    if retain_counts.get(&step.output).copied().unwrap_or(0) == 0 {
        values.remove(&step.output);
    }
}

fn cuda_execution_plan_cache_policy() -> ExecutionPlanCachePolicy {
    ExecutionPlanCachePolicy::bounded(
        CUDA_EXECUTION_PLAN_CACHE_MAX_ENTRIES,
        Some(CUDA_EXECUTION_PLAN_CACHE_MAX_CACHED_BYTES),
    )
}

#[derive(Clone, Debug)]
struct CachedCudaExecutionPlan {
    plan: ExecutionPlan,
    plan_digest: String,
}

#[derive(Clone, Debug)]
struct CudaExecutionPlanCache {
    policy: ExecutionPlanCachePolicy,
    cached: std::collections::HashMap<String, CachedCudaExecutionPlan>,
    state: ExecutionPlanCacheState,
}

impl CudaExecutionPlanCache {
    fn new(policy: ExecutionPlanCachePolicy) -> Self {
        Self {
            policy,
            cached: std::collections::HashMap::new(),
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
    ) -> Result<(ExecutionPlan, String, bool), RuntimeError> {
        let cache_key = graph.stable_digest();
        if let Some(cached) = self.cached.get(&cache_key) {
            return Ok((cached.plan.clone(), cached.plan_digest.clone(), true));
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
                CachedCudaExecutionPlan {
                    plan: plan.clone(),
                    plan_digest: plan_digest.clone(),
                },
            );
            self.state.cached_entries = self.cached.len();
            self.state.cached_bytes = self.state.cached_bytes.saturating_add(estimated_bytes);
        }
        Ok((plan, plan_digest, false))
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

impl NvidiaInventoryRow {
    fn into_device_descriptor(self) -> DeviceDescriptor {
        let architecture = architecture_from_compute_capability(self.compute_capability.as_deref());
        let risk = risk_profile(
            self.display_attached,
            self.mig_profile.as_deref(),
            self.persistence_mode_enabled,
        );
        let recovery = recovery_profile(query_recovery_action(self.ordinal).ok().as_deref());

        let mut feature_flags = vec![String::from("cuda_architecture_surface")];
        if self.display_attached == Some(true) {
            feature_flags.push(String::from("display_attached"));
        }
        if self.mig_profile.is_some() {
            feature_flags.push(String::from("mig_enabled"));
        }
        if let Some(enabled) = self.persistence_mode_enabled {
            feature_flags.push(if enabled {
                String::from("persistence_mode_enabled")
            } else {
                String::from("persistence_mode_disabled")
            });
        }
        if let Some(addressing_mode) = self.addressing_mode.as_deref() {
            feature_flags.push(format!(
                "addressing_mode_{}",
                addressing_mode.to_ascii_lowercase()
            ));
        }

        DeviceDescriptor {
            backend: String::from("cuda"),
            device: Device::new(
                DeviceKind::Cuda,
                self.ordinal,
                Some(format!("cuda:{}", self.ordinal)),
            ),
            device_name: Some(self.name),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: self.memory_bytes,
            unified_memory: Some(false),
            feature_flags,
            amd_metadata: None,
            nvidia_metadata: Some(NvidiaDeviceMetadata {
                topology: NvidiaTopologyInfo {
                    architecture,
                    compute_capability: self.compute_capability,
                    pci_bdf: self.pci_bdf,
                    sm_count: None,
                    vram_bytes: self.memory_bytes,
                    mig_profile: self.mig_profile,
                },
                risk,
                recovery,
            }),
        }
    }
}

fn cuda_allocator_pool_policy() -> AllocatorPoolPolicy {
    AllocatorPoolPolicy::exact_tensor_spec(CUDA_POOL_MAX_CACHED_BUFFERS, CUDA_POOL_MAX_CACHED_BYTES)
}

fn size_of_dtype(dtype: DType) -> usize {
    dtype.element_size_bytes()
}

fn validate_supported_plan(plan: &ExecutionPlan) -> Result<(), RuntimeError> {
    for step in &plan.steps {
        validate_supported_step(step)?;
    }
    Ok(())
}

fn validate_supported_step(step: &ExecutionStep) -> Result<(), RuntimeError> {
    match &step.op {
        ExecutionOp::Input { .. } => {
            ensure_supported_input_spec(&step.spec)?;
            if !step.inputs.is_empty() {
                return Err(RuntimeError::Backend(format!(
                    "cuda input step {} unexpectedly has inputs",
                    step.output
                )));
            }
        }
        ExecutionOp::Constant { data } => {
            ensure_supported_input_spec(&step.spec)?;
            let expected_len = step.spec.storage_size();
            match data {
                TensorData::F32(values) if step.spec.dtype() == DType::F32 => {
                    if values.len() != expected_len {
                        return Err(RuntimeError::Backend(format!(
                            "cuda constant {} payload length mismatch",
                            step.output
                        )));
                    }
                }
                TensorData::BF16(values) if step.spec.dtype() == DType::BF16 => {
                    if values.len() != expected_len {
                        return Err(RuntimeError::Backend(format!(
                            "cuda constant {} payload length mismatch",
                            step.output
                        )));
                    }
                }
                TensorData::I32(values) if step.spec.dtype() == DType::I32 => {
                    if values.len() != expected_len {
                        return Err(RuntimeError::Backend(format!(
                            "cuda constant {} payload length mismatch",
                            step.output
                        )));
                    }
                }
                _ => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda constant {} must use dense storage matching its dtype",
                        step.output
                    )));
                }
            }
        }
        ExecutionOp::Detach
        | ExecutionOp::Reshape
        | ExecutionOp::Permute { .. }
        | ExecutionOp::Slice { .. }
        | ExecutionOp::Select { .. }
        | ExecutionOp::Expand { .. }
        | ExecutionOp::ReduceSum { .. } => {
            ensure_supported_float_spec(&step.spec)?;
            if step.inputs.len() != 1 {
                return Err(RuntimeError::Backend(format!(
                    "cuda {} step {} requires one input",
                    step.op.label(),
                    step.output
                )));
            }
        }
        ExecutionOp::Concat { .. } => {
            ensure_supported_float_spec(&step.spec)?;
            if step.inputs.is_empty() {
                return Err(RuntimeError::Backend(format!(
                    "cuda concat step {} requires at least one input",
                    step.output
                )));
            }
        }
        ExecutionOp::Cast { dtype } => {
            ensure_supported_float_spec(&step.spec)?;
            if step.inputs.len() != 1 {
                return Err(RuntimeError::Backend(format!(
                    "cuda cast step {} requires one input",
                    step.output
                )));
            }
            if *dtype != DType::F32 && *dtype != DType::BF16 {
                return Err(RuntimeError::UnsupportedStep(format!(
                    "cast {:?}->{:?}",
                    DType::F32,
                    dtype
                )));
            }
        }
        ExecutionOp::Add => {
            ensure_supported_float_spec(&step.spec)?;
            if step.inputs.len() != 2 {
                return Err(RuntimeError::Backend(format!(
                    "cuda add step {} requires two inputs",
                    step.output
                )));
            }
        }
        ExecutionOp::Mul => {
            ensure_supported_float_spec(&step.spec)?;
            if step.inputs.len() != 2 {
                return Err(RuntimeError::Backend(format!(
                    "cuda mul step {} requires two inputs",
                    step.output
                )));
            }
        }
        ExecutionOp::Matmul => {
            ensure_supported_f32_spec(&step.spec)?;
            if step.inputs.len() != 2 {
                return Err(RuntimeError::Backend(format!(
                    "cuda matmul step {} requires two inputs",
                    step.output
                )));
            }
            let dims = step.spec.shape().dims();
            if dims.len() != 2 {
                return Err(RuntimeError::Backend(format!(
                    "cuda matmul step {} requires a rank-2 output, actual rank {}",
                    step.output,
                    dims.len()
                )));
            }
        }
        ExecutionOp::BackendExtension { op } => match op {
            BackendExtensionOp::ParameterGolfTokenEmbeddingLookup => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 2 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda parameter_golf_token_embedding_lookup step {} requires two inputs",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::ParameterGolfBankedLinear { .. } => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 2 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda parameter_golf_banked_linear step {} requires two inputs",
                        step.output
                    )));
                }
                if step.spec.shape().dims().len() != 3 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda parameter_golf_banked_linear step {} requires a rank-3 output, actual rank {}",
                        step.output,
                        step.spec.shape().dims().len()
                    )));
                }
            }
            BackendExtensionOp::ParameterGolfBankedLinearInputBackward { .. } => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 2 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda parameter_golf_banked_linear_input_backward step {} requires two inputs",
                        step.output
                    )));
                }
                if step.spec.shape().dims().len() != 3 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda parameter_golf_banked_linear_input_backward step {} requires a rank-3 output, actual rank {}",
                        step.output,
                        step.spec.shape().dims().len()
                    )));
                }
            }
            BackendExtensionOp::ParameterGolfBankedLinearWeightBackward { .. } => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 3 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda parameter_golf_banked_linear_weight_backward step {} requires three inputs",
                        step.output
                    )));
                }
                if step.spec.shape().dims().len() != 3 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda parameter_golf_banked_linear_weight_backward step {} requires a rank-3 output, actual rank {}",
                        step.output,
                        step.spec.shape().dims().len()
                    )));
                }
            }
            BackendExtensionOp::ParameterGolfTokenEmbeddingLookupBackward => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 3 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda parameter_golf_token_embedding_lookup_backward step {} requires three inputs",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::ReluSquared => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 1 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda relu_squared step {} requires one input",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::ReluSquaredBackward => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 2 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda relu_squared_backward step {} requires two inputs",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::ReluSquaredBackwardFromOutput => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 2 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda relu_squared_backward_from_output step {} requires two inputs",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::LeakyReluSquared { .. } => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 1 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda leaky_relu_squared step {} requires one input",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::LeakyReluSquaredBackward { .. } => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 2 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda leaky_relu_squared_backward step {} requires two inputs",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::Silu => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 1 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda silu step {} requires one input",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::SiluBackward => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 2 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda silu_backward step {} requires two inputs",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::ParameterGolfProjectionLoss { .. } => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 2 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda parameter_golf_projection_loss step {} requires two inputs",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::ParameterGolfProjectionTokenLosses { .. } => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 2 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda parameter_golf_projection_token_losses step {} requires two inputs",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::ParameterGolfProjectionLossBackward { .. } => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 3 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda parameter_golf_projection_loss_backward step {} requires three inputs",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::RmsNorm { .. } => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 2 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda rms_norm step {} requires two inputs",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::RmsNormInputBackward { .. } => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 3 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda rms_norm_input_backward step {} requires three inputs",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::RmsNormWeightBackward { .. } => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 2 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda rms_norm_weight_backward step {} requires two inputs",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::RotaryEmbedding { interleaved } => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 3 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda rotary_embedding step {} requires three inputs",
                        step.output
                    )));
                }
                if *interleaved {
                    return Err(RuntimeError::UnsupportedStep(String::from(
                        "rotary_embedding_interleaved",
                    )));
                }
            }
            BackendExtensionOp::RotaryEmbeddingBackward { interleaved } => {
                ensure_supported_f32_spec(&step.spec)?;
                if step.inputs.len() != 3 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda rotary_embedding_backward step {} requires three inputs",
                        step.output
                    )));
                }
                if *interleaved {
                    return Err(RuntimeError::UnsupportedStep(String::from(
                        "rotary_embedding_backward_interleaved",
                    )));
                }
            }
            BackendExtensionOp::ScaledDotProductAttention { causal, .. } => {
                ensure_supported_float_spec(&step.spec)?;
                if step.inputs.len() != 3 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda scaled_dot_product_attention step {} requires three inputs",
                        step.output
                    )));
                }
                if !causal {
                    return Err(RuntimeError::UnsupportedStep(String::from(
                        "scaled_dot_product_attention_non_causal",
                    )));
                }
            }
            BackendExtensionOp::ScaledDotProductAttentionQueryBackward { causal, .. }
            | BackendExtensionOp::ScaledDotProductAttentionKeyBackward { causal, .. }
            | BackendExtensionOp::ScaledDotProductAttentionValueBackward { causal, .. } => {
                ensure_supported_float_spec(&step.spec)?;
                if step.inputs.len() != 4 {
                    return Err(RuntimeError::Backend(format!(
                        "cuda {} step {} requires four inputs",
                        op.label(),
                        step.output
                    )));
                }
                if !causal {
                    return Err(RuntimeError::UnsupportedStep(format!(
                        "{}_non_causal",
                        op.label()
                    )));
                }
            }
            _ => {
                return Err(RuntimeError::UnsupportedStep(step.op.label().to_string()));
            }
        },
    }
    Ok(())
}

fn cast_contiguous_values(
    values: &[f32],
    input_dtype: DType,
    output_dtype: DType,
) -> Result<Vec<f32>, RuntimeError> {
    if input_dtype != DType::F32 || output_dtype != DType::F32 {
        return Err(RuntimeError::UnsupportedStep(format!(
            "cast {:?}->{:?}",
            input_dtype, output_dtype
        )));
    }
    Ok(values.to_vec())
}

fn permute_contiguous_values(
    values: &[f32],
    input_shape: &[usize],
    axes: &[usize],
) -> Result<Vec<f32>, RuntimeError> {
    if input_shape.len() != axes.len() {
        return Err(RuntimeError::Backend(format!(
            "permute rank mismatch: shape rank {} axes {}",
            input_shape.len(),
            axes.len()
        )));
    }
    let output_shape = axes
        .iter()
        .map(|&axis| input_shape[axis])
        .collect::<Vec<_>>();
    if axes
        .iter()
        .enumerate()
        .all(|(output_axis, &input_axis)| output_axis == input_axis)
    {
        return Ok(values.to_vec());
    }
    if let Some(output) = permute_rank2_transpose_fast_path(values, input_shape, axes) {
        return Ok(output);
    }
    if let Some(output) = permute_rank4_swap_middle_axes_fast_path(values, input_shape, axes) {
        return Ok(output);
    }
    let input_strides = contiguous_strides(input_shape);
    let output_strides = contiguous_strides(&output_shape);
    let mut output = vec![0.0_f32; output_shape.iter().product()];
    let chunk_len = host_fallback_chunk_len(output.len());
    let fill_chunk = |chunk_index: usize, chunk: &mut [f32]| {
        let start_linear = chunk_index.saturating_mul(chunk_len);
        let last_offset = chunk.len().saturating_sub(1);
        let mut output_coords = vec![0_usize; output_shape.len()];
        linear_to_coords_in_place(
            start_linear,
            &output_shape,
            &output_strides,
            output_coords.as_mut_slice(),
        );
        for (offset, output_value) in chunk.iter_mut().enumerate() {
            let mut input_linear = 0_usize;
            for (output_axis, &input_axis) in axes.iter().enumerate() {
                input_linear = input_linear.saturating_add(
                    output_coords[output_axis].saturating_mul(input_strides[input_axis]),
                );
            }
            *output_value = values[input_linear];
            if offset != last_offset {
                advance_coords(output_coords.as_mut_slice(), &output_shape);
            }
        }
    };
    if output.len() >= HOST_FALLBACK_PARALLEL_MIN_ELEMENTS {
        output
            .par_chunks_mut(chunk_len)
            .enumerate()
            .for_each(|(chunk_index, chunk)| fill_chunk(chunk_index, chunk));
    } else {
        for (chunk_index, chunk) in output.chunks_mut(chunk_len).enumerate() {
            fill_chunk(chunk_index, chunk);
        }
    }
    Ok(output)
}

fn permute_rank2_transpose_fast_path(
    values: &[f32],
    input_shape: &[usize],
    axes: &[usize],
) -> Option<Vec<f32>> {
    if input_shape.len() != 2 || axes != [1, 0] {
        return None;
    }
    let rows = input_shape[0];
    let cols = input_shape[1];
    let mut output = vec![0.0_f32; values.len()];
    if rows == 0 || cols == 0 {
        return Some(output);
    }
    if output.len() >= HOST_FALLBACK_PARALLEL_MIN_ELEMENTS {
        output
            .par_chunks_mut(rows)
            .enumerate()
            .for_each(|(col, output_row)| {
                for row in 0..rows {
                    output_row[row] = values[row.saturating_mul(cols).saturating_add(col)];
                }
            });
    } else {
        for (col, output_row) in output.chunks_mut(rows).enumerate() {
            for row in 0..rows {
                output_row[row] = values[row.saturating_mul(cols).saturating_add(col)];
            }
        }
    }
    Some(output)
}

fn permute_rank4_swap_middle_axes_fast_path(
    values: &[f32],
    input_shape: &[usize],
    axes: &[usize],
) -> Option<Vec<f32>> {
    if input_shape.len() != 4 || axes != [0, 2, 1, 3] {
        return None;
    }
    let outer = input_shape[0];
    let left = input_shape[1];
    let middle = input_shape[2];
    let inner = input_shape[3];
    let output_chunk_len = left.saturating_mul(inner);
    let mut output = vec![0.0_f32; values.len()];
    if output_chunk_len == 0 {
        return Some(output);
    }
    let fill_chunk = |chunk_index: usize, chunk: &mut [f32]| {
        let outer_index = chunk_index / middle;
        let middle_index = chunk_index % middle;
        for left_index in 0..left {
            let input_offset = (((outer_index * left + left_index) * middle + middle_index)
                * inner)
                .min(values.len());
            let output_offset = left_index * inner;
            let copy_len = inner
                .min(values.len().saturating_sub(input_offset))
                .min(chunk.len().saturating_sub(output_offset));
            chunk[output_offset..output_offset + copy_len]
                .copy_from_slice(&values[input_offset..input_offset + copy_len]);
        }
    };
    let chunk_count = outer.saturating_mul(middle);
    if output.len() >= HOST_FALLBACK_PARALLEL_MIN_ELEMENTS {
        output
            .par_chunks_mut(output_chunk_len)
            .take(chunk_count)
            .enumerate()
            .for_each(|(chunk_index, chunk)| fill_chunk(chunk_index, chunk));
    } else {
        for (chunk_index, chunk) in output
            .chunks_mut(output_chunk_len)
            .take(chunk_count)
            .enumerate()
        {
            fill_chunk(chunk_index, chunk);
        }
    }
    Some(output)
}

fn slice_contiguous_values(
    values: &[f32],
    input_shape: &[usize],
    axis: usize,
    start: usize,
    end: usize,
) -> Result<Vec<f32>, RuntimeError> {
    if axis >= input_shape.len() || start > end || end > input_shape[axis] {
        return Err(RuntimeError::Backend(format!(
            "invalid slice axis={axis} start={start} end={end} for shape {input_shape:?}"
        )));
    }
    let mut output_shape = input_shape.to_vec();
    output_shape[axis] = end - start;
    let input_strides = contiguous_strides(input_shape);
    let output_strides = contiguous_strides(&output_shape);
    let mut output = vec![0.0_f32; output_shape.iter().product()];
    for output_linear in 0..output.len() {
        let mut input_coords = linear_to_coords(output_linear, &output_shape, &output_strides);
        input_coords[axis] += start;
        let input_linear = coords_to_linear(&input_coords, &input_strides);
        output[output_linear] = values[input_linear];
    }
    Ok(output)
}

fn select_contiguous_values(
    values: &[f32],
    input_shape: &[usize],
    axis: usize,
    index: usize,
) -> Result<Vec<f32>, RuntimeError> {
    if axis >= input_shape.len() || index >= input_shape[axis] {
        return Err(RuntimeError::Backend(format!(
            "invalid select axis={axis} index={index} for shape {input_shape:?}"
        )));
    }
    let mut output_shape = input_shape.to_vec();
    output_shape.remove(axis);
    let input_strides = contiguous_strides(input_shape);
    let output_strides = contiguous_strides(&output_shape);
    let mut output = vec![0.0_f32; output_shape.iter().product::<usize>().max(1)];
    for output_linear in 0..output.len() {
        let output_coords = linear_to_coords(output_linear, &output_shape, &output_strides);
        let mut input_coords = Vec::with_capacity(input_shape.len());
        let mut output_axis = 0;
        for input_axis in 0..input_shape.len() {
            if input_axis == axis {
                input_coords.push(index);
            } else {
                input_coords.push(output_coords[output_axis]);
                output_axis += 1;
            }
        }
        let input_linear = coords_to_linear(&input_coords, &input_strides);
        output[output_linear] = values[input_linear];
    }
    Ok(output)
}

fn concat_contiguous_values(
    values: &[&[f32]],
    shapes: &[&[usize]],
    axis: usize,
) -> Result<Vec<f32>, RuntimeError> {
    let Some(first_shape) = shapes.first() else {
        return Err(RuntimeError::Backend(String::from(
            "concat requires at least one input",
        )));
    };
    if axis >= first_shape.len() {
        return Err(RuntimeError::Backend(format!(
            "invalid concat axis={axis} for shape {first_shape:?}"
        )));
    }
    let rank = first_shape.len();
    if shapes.iter().any(|shape| shape.len() != rank) {
        return Err(RuntimeError::Backend(String::from(
            "concat requires matching input ranks",
        )));
    }
    let mut output_shape = first_shape.to_vec();
    output_shape[axis] = 0;
    for shape in shapes {
        for check_axis in 0..rank {
            if check_axis != axis && shape[check_axis] != first_shape[check_axis] {
                return Err(RuntimeError::Backend(String::from(
                    "concat requires matching non-concatenated dimensions",
                )));
            }
        }
        output_shape[axis] += shape[axis];
    }
    let output_strides = contiguous_strides(&output_shape);
    let input_strides = shapes
        .iter()
        .map(|shape| contiguous_strides(shape))
        .collect::<Vec<_>>();
    let mut output = vec![0.0_f32; output_shape.iter().product()];
    let mut axis_offset = 0;
    for ((input_values, input_shape), input_stride) in values.iter().zip(shapes).zip(&input_strides)
    {
        let input_len = input_shape.iter().product();
        for input_linear in 0..input_len {
            let mut output_coords = linear_to_coords(input_linear, input_shape, input_stride);
            output_coords[axis] += axis_offset;
            let output_linear = coords_to_linear(&output_coords, &output_strides);
            output[output_linear] = input_values[input_linear];
        }
        axis_offset += input_shape[axis];
    }
    Ok(output)
}

fn expand_contiguous_values(
    values: &[f32],
    input_shape: &[usize],
    output_shape: &[usize],
) -> Result<Vec<f32>, RuntimeError> {
    if output_shape.len() < input_shape.len() {
        return Err(RuntimeError::Backend(format!(
            "expand requires output rank >= input rank, found input={input_shape:?} output={output_shape:?}"
        )));
    }
    let input_padding = output_shape.len() - input_shape.len();
    let padded_input = std::iter::repeat_n(1_usize, input_padding)
        .chain(input_shape.iter().copied())
        .collect::<Vec<_>>();
    for (&input_dim, &output_dim) in padded_input.iter().zip(output_shape) {
        if input_dim != output_dim && input_dim != 1 {
            return Err(RuntimeError::Backend(format!(
                "invalid expand from {input_shape:?} to {output_shape:?}"
            )));
        }
    }
    let input_strides = contiguous_strides(&padded_input);
    let output_strides = contiguous_strides(output_shape);
    let mut output = vec![0.0_f32; output_shape.iter().product()];
    let chunk_len = host_fallback_chunk_len(output.len());
    let fill_chunk = |chunk_index: usize, chunk: &mut [f32]| {
        let start_linear = chunk_index.saturating_mul(chunk_len);
        let last_offset = chunk.len().saturating_sub(1);
        let mut output_coords = vec![0_usize; output_shape.len()];
        linear_to_coords_in_place(
            start_linear,
            output_shape,
            &output_strides,
            output_coords.as_mut_slice(),
        );
        for (offset, output_value) in chunk.iter_mut().enumerate() {
            let mut input_linear = 0_usize;
            for axis in 0..output_shape.len() {
                if padded_input[axis] != 1 {
                    input_linear = input_linear
                        .saturating_add(output_coords[axis].saturating_mul(input_strides[axis]));
                }
            }
            *output_value = values[input_linear];
            if offset != last_offset {
                advance_coords(output_coords.as_mut_slice(), output_shape);
            }
        }
    };
    if output.len() >= HOST_FALLBACK_PARALLEL_MIN_ELEMENTS {
        output
            .par_chunks_mut(chunk_len)
            .enumerate()
            .for_each(|(chunk_index, chunk)| fill_chunk(chunk_index, chunk));
    } else {
        for (chunk_index, chunk) in output.chunks_mut(chunk_len).enumerate() {
            fill_chunk(chunk_index, chunk);
        }
    }
    Ok(output)
}

fn broadcast_binary_float_values<F>(
    left_values: &[f32],
    left_shape: &[usize],
    right_values: &[f32],
    right_shape: &[usize],
    output_shape: &[usize],
    combine: F,
) -> Result<Vec<f32>, RuntimeError>
where
    F: Fn(f32, f32) -> f32,
{
    let left = if left_shape == output_shape {
        left_values.to_vec()
    } else {
        expand_contiguous_values(left_values, left_shape, output_shape)?
    };
    let right = if right_shape == output_shape {
        right_values.to_vec()
    } else {
        expand_contiguous_values(right_values, right_shape, output_shape)?
    };
    Ok(left
        .into_iter()
        .zip(right)
        .map(|(left, right)| combine(left, right))
        .collect())
}

fn reduce_sum_contiguous_values(
    values: &[f32],
    input_shape: &[usize],
    axis: Option<usize>,
) -> Result<Vec<f32>, RuntimeError> {
    let Some(axis) = axis else {
        return Ok(vec![parallel_sum_f32(values)]);
    };
    if axis >= input_shape.len() {
        return Err(RuntimeError::Backend(format!(
            "invalid reduce_sum axis={axis} for shape {input_shape:?}"
        )));
    }
    if axis + 1 == input_shape.len() {
        return Ok(reduce_sum_last_axis_fast_path(values, input_shape));
    }
    if axis == 0 {
        return Ok(reduce_sum_first_axis_fast_path(values, input_shape));
    }
    let mut output_shape = input_shape.to_vec();
    output_shape.remove(axis);
    let input_strides = contiguous_strides(input_shape);
    let output_strides = contiguous_strides(&output_shape);
    let mut output = vec![0.0_f32; output_shape.iter().product::<usize>().max(1)];
    let reduction_len = input_shape[axis];
    let reduction_stride = input_strides[axis];
    let chunk_len = host_fallback_chunk_len(output.len());
    let fill_chunk = |chunk_index: usize, chunk: &mut [f32]| {
        let start_linear = chunk_index.saturating_mul(chunk_len);
        let last_offset = chunk.len().saturating_sub(1);
        let mut output_coords = vec![0_usize; output_shape.len()];
        linear_to_coords_in_place(
            start_linear,
            &output_shape,
            &output_strides,
            output_coords.as_mut_slice(),
        );
        for (offset, output_value) in chunk.iter_mut().enumerate() {
            let mut base_linear = 0_usize;
            let mut output_axis = 0;
            for input_axis in 0..input_shape.len() {
                if input_axis == axis {
                    continue;
                }
                base_linear = base_linear.saturating_add(
                    output_coords[output_axis].saturating_mul(input_strides[input_axis]),
                );
                output_axis += 1;
            }

            let mut sum = 0.0_f32;
            for reduced_index in 0..reduction_len {
                sum += values
                    [base_linear.saturating_add(reduced_index.saturating_mul(reduction_stride))];
            }
            *output_value = sum;
            if offset != last_offset {
                advance_coords(output_coords.as_mut_slice(), &output_shape);
            }
        }
    };
    if output.len() >= HOST_FALLBACK_PARALLEL_MIN_ELEMENTS {
        output
            .par_chunks_mut(chunk_len)
            .enumerate()
            .for_each(|(chunk_index, chunk)| fill_chunk(chunk_index, chunk));
    } else {
        for (chunk_index, chunk) in output.chunks_mut(chunk_len).enumerate() {
            fill_chunk(chunk_index, chunk);
        }
    }
    Ok(output)
}

fn parallel_sum_f32(values: &[f32]) -> f32 {
    if values.len() >= HOST_FALLBACK_PARALLEL_MIN_ELEMENTS {
        values
            .par_chunks(host_fallback_chunk_len(values.len()))
            .map(|chunk| chunk.iter().copied().sum::<f32>())
            .sum()
    } else {
        values.iter().copied().sum()
    }
}

fn reduce_sum_last_axis_fast_path(values: &[f32], input_shape: &[usize]) -> Vec<f32> {
    let reduction_len = input_shape.last().copied().unwrap_or(1);
    let output_len = input_shape[..input_shape.len().saturating_sub(1)]
        .iter()
        .product::<usize>()
        .max(1);
    let mut output = vec![0.0_f32; output_len];
    if reduction_len == 0 {
        return output;
    }
    if output_len >= HOST_FALLBACK_PARALLEL_MIN_ELEMENTS {
        output
            .par_iter_mut()
            .enumerate()
            .for_each(|(output_index, sum)| {
                let start = output_index.saturating_mul(reduction_len);
                *sum = values[start..start + reduction_len].iter().copied().sum();
            });
    } else {
        for (output_index, sum) in output.iter_mut().enumerate() {
            let start = output_index.saturating_mul(reduction_len);
            *sum = values[start..start + reduction_len].iter().copied().sum();
        }
    }
    output
}

fn reduce_sum_first_axis_fast_path(values: &[f32], input_shape: &[usize]) -> Vec<f32> {
    let reduction_len = input_shape[0];
    let output_len = input_shape[1..].iter().product::<usize>().max(1);
    let mut output = vec![0.0_f32; output_len];
    if reduction_len == 0 {
        return output;
    }
    if output_len >= HOST_FALLBACK_PARALLEL_MIN_ELEMENTS {
        output
            .par_iter_mut()
            .enumerate()
            .for_each(|(output_index, sum)| {
                let mut total = 0.0_f32;
                for reduced_index in 0..reduction_len {
                    total += values[reduced_index
                        .saturating_mul(output_len)
                        .saturating_add(output_index)];
                }
                *sum = total;
            });
    } else {
        for (output_index, sum) in output.iter_mut().enumerate() {
            let mut total = 0.0_f32;
            for reduced_index in 0..reduction_len {
                total += values[reduced_index
                    .saturating_mul(output_len)
                    .saturating_add(output_index)];
            }
            *sum = total;
        }
    }
    output
}

fn contiguous_strides(shape: &[usize]) -> Vec<usize> {
    let mut strides = vec![0; shape.len()];
    let mut running = 1;
    for (index, dim) in shape.iter().enumerate().rev() {
        strides[index] = running;
        running *= *dim;
    }
    strides
}

fn host_fallback_chunk_len(total_elements: usize) -> usize {
    if total_elements <= 1 {
        return 1;
    }
    let target_chunks = rayon::current_num_threads().max(1).saturating_mul(4);
    total_elements
        .div_ceil(target_chunks)
        .max(HOST_FALLBACK_PARALLEL_MIN_CHUNK_ELEMENTS)
        .min(total_elements)
}

fn linear_to_coords_in_place(
    linear: usize,
    shape: &[usize],
    strides: &[usize],
    coords: &mut [usize],
) {
    if shape.is_empty() {
        return;
    }
    let mut remaining = linear;
    for axis in 0..shape.len() {
        coords[axis] = remaining / strides[axis];
        remaining %= strides[axis];
    }
}

fn advance_coords(coords: &mut [usize], shape: &[usize]) {
    for axis in (0..coords.len()).rev() {
        coords[axis] += 1;
        if coords[axis] < shape[axis] {
            return;
        }
        coords[axis] = 0;
    }
}

fn linear_to_coords(linear: usize, shape: &[usize], strides: &[usize]) -> Vec<usize> {
    let mut coords = vec![0; shape.len()];
    linear_to_coords_in_place(linear, shape, strides, coords.as_mut_slice());
    coords
}

fn coords_to_linear(coords: &[usize], strides: &[usize]) -> usize {
    coords
        .iter()
        .zip(strides.iter())
        .map(|(coord, stride)| coord.saturating_mul(*stride))
        .sum()
}

fn relu_squared_forward_values(input: &[f32]) -> Vec<f32> {
    input
        .iter()
        .map(|value| {
            let positive = value.max(0.0);
            positive * positive
        })
        .collect()
}

fn relu_squared_backward_values(input: &[f32], grad_output: &[f32]) -> Vec<f32> {
    input
        .iter()
        .zip(grad_output.iter())
        .map(|(value, grad)| {
            if *value > 0.0 {
                grad * (2.0 * value)
            } else {
                0.0
            }
        })
        .collect()
}

fn relu_squared_backward_from_output_values(output: &[f32], grad_output: &[f32]) -> Vec<f32> {
    output
        .iter()
        .zip(grad_output.iter())
        .map(|(value, grad)| {
            if *value > 0.0 {
                grad * (2.0 * value.sqrt())
            } else {
                0.0
            }
        })
        .collect()
}

fn leaky_relu_squared_forward_values(input: &[f32], negative_slope: f32) -> Vec<f32> {
    input
        .iter()
        .map(|value| {
            let activated = if *value >= 0.0 {
                *value
            } else {
                value * negative_slope
            };
            activated * activated
        })
        .collect()
}

fn leaky_relu_squared_backward_values(
    input: &[f32],
    grad_output: &[f32],
    negative_slope: f32,
) -> Vec<f32> {
    input
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
        .collect()
}

fn ensure_supported_float_spec(spec: &TensorSpec) -> Result<(), RuntimeError> {
    if spec.dtype() != DType::F32 && spec.dtype() != DType::BF16 {
        return Err(RuntimeError::Backend(format!(
            "cuda dense surface only supports F32/BF16 tensors, actual {:?}",
            spec.dtype()
        )));
    }
    if spec.device().kind() != DeviceKind::Cuda {
        return Err(RuntimeError::Backend(format!(
            "cuda dense surface requires CUDA tensor specs, actual device kind {}",
            spec.device().kind()
        )));
    }
    if !spec.layout().is_contiguous() || spec.layout().offset() != 0 {
        return Err(RuntimeError::Backend(String::from(
            "cuda dense surface requires contiguous zero-offset tensors",
        )));
    }
    Ok(())
}

fn ensure_supported_f32_spec(spec: &TensorSpec) -> Result<(), RuntimeError> {
    if spec.dtype() != DType::F32 {
        return Err(RuntimeError::Backend(format!(
            "cuda dense surface only supports F32 tensors for this operation, actual {:?}",
            spec.dtype()
        )));
    }
    ensure_supported_float_spec(spec)
}

fn ensure_supported_input_spec(spec: &TensorSpec) -> Result<(), RuntimeError> {
    if spec.dtype() != DType::F32 && spec.dtype() != DType::BF16 && spec.dtype() != DType::I32 {
        return Err(RuntimeError::Backend(format!(
            "cuda dense input surface only supports F32/BF16/I32 tensors, actual {:?}",
            spec.dtype()
        )));
    }
    if spec.device().kind() != DeviceKind::Cuda {
        return Err(RuntimeError::Backend(format!(
            "cuda dense surface requires CUDA tensor specs, actual device kind {}",
            spec.device().kind()
        )));
    }
    if !spec.layout().is_contiguous() || spec.layout().offset() != 0 {
        return Err(RuntimeError::Backend(String::from(
            "cuda dense surface requires contiguous zero-offset tensors",
        )));
    }
    Ok(())
}

fn silu_forward_values(input: &[f32]) -> Vec<f32> {
    input
        .iter()
        .map(|value| {
            let sigmoid = 1.0 / (1.0 + (-value).exp());
            value * sigmoid
        })
        .collect()
}

fn silu_backward_values(input: &[f32], grad_output: &[f32]) -> Vec<f32> {
    input
        .iter()
        .zip(grad_output.iter())
        .map(|(value, grad)| {
            let sigmoid = 1.0 / (1.0 + (-value).exp());
            let derivative = sigmoid * (1.0 + value * (1.0 - sigmoid));
            grad * derivative
        })
        .collect()
}

fn byte_len_for_f32_elements(element_count: usize) -> Result<usize, RuntimeError> {
    element_count
        .checked_mul(size_of_dtype(DType::F32))
        .ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda f32 byte length overflow on bounded public runtime path",
            ))
        })
}

fn rank4_token_head_element_offset(
    dims: &[usize],
    batch_index: usize,
    head_index: usize,
    position: usize,
) -> Result<usize, RuntimeError> {
    if dims.len() != 4 {
        return Err(RuntimeError::Backend(String::from(
            "cuda rank-4 helper requires a rank-4 tensor",
        )));
    }
    if batch_index >= dims[0] || head_index >= dims[1] || position >= dims[2] {
        return Err(RuntimeError::Backend(String::from(
            "cuda rank-4 helper index exceeds the source tensor extent",
        )));
    }
    batch_index
        .checked_mul(dims[1])
        .and_then(|value| value.checked_add(head_index))
        .and_then(|value| value.checked_mul(dims[2]))
        .and_then(|value| value.checked_add(position))
        .and_then(|value| value.checked_mul(dims[3]))
        .ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda rank-4 helper offset overflow on bounded public runtime path",
            ))
        })
}

fn pack_rank4_token_to_head_major_scratch(
    submission: &mut CudaSubmission,
    source: &CudaBuffer,
    batch_index: usize,
    position: usize,
    scratch: &CudaBuffer,
) -> Result<(), RuntimeError> {
    let source_dims = source.spec().shape().dims();
    let scratch_dims = scratch.spec().shape().dims();
    if source_dims.len() != 4
        || scratch_dims.len() != 2
        || scratch_dims[0] != source_dims[1]
        || scratch_dims[1] != source_dims[3]
    {
        return Err(RuntimeError::Backend(String::from(
            "cuda rank-4 packing requires a [heads, head_dim] scratch tensor",
        )));
    }
    let row_bytes = byte_len_for_f32_elements(source_dims[3])?;
    for head_index in 0..source_dims[1] {
        let source_offset = byte_len_for_f32_elements(rank4_token_head_element_offset(
            source_dims,
            batch_index,
            head_index,
            position,
        )?)?;
        let scratch_offset = head_index.checked_mul(row_bytes).ok_or_else(|| {
            RuntimeError::Backend(String::from("cuda rank-4 packing scratch offset overflow"))
        })?;
        submission.copy_buffer_region(source, source_offset, scratch, scratch_offset, row_bytes)?;
    }
    Ok(())
}

fn scatter_head_major_scratch_to_rank4_token(
    submission: &mut CudaSubmission,
    scratch: &CudaBuffer,
    destination: &CudaBuffer,
    batch_index: usize,
    position: usize,
) -> Result<(), RuntimeError> {
    let destination_dims = destination.spec().shape().dims();
    let scratch_dims = scratch.spec().shape().dims();
    if destination_dims.len() != 4
        || scratch_dims.len() != 2
        || scratch_dims[0] != destination_dims[1]
        || scratch_dims[1] != destination_dims[3]
    {
        return Err(RuntimeError::Backend(String::from(
            "cuda rank-4 scatter requires a [heads, head_dim] scratch tensor",
        )));
    }
    let row_bytes = byte_len_for_f32_elements(destination_dims[3])?;
    for head_index in 0..destination_dims[1] {
        let scratch_offset = head_index.checked_mul(row_bytes).ok_or_else(|| {
            RuntimeError::Backend(String::from("cuda rank-4 scatter scratch offset overflow"))
        })?;
        let destination_offset = byte_len_for_f32_elements(rank4_token_head_element_offset(
            destination_dims,
            batch_index,
            head_index,
            position,
        )?)?;
        submission.copy_buffer_region(
            scratch,
            scratch_offset,
            destination,
            destination_offset,
            row_bytes,
        )?;
    }
    Ok(())
}

fn validate_standard_neox_rope_tables(
    cos: &CudaBuffer,
    sin: &CudaBuffer,
    batch_size: usize,
    sequence_length: usize,
    half_dim: usize,
) -> Result<bool, RuntimeError> {
    let cos_dims = cos.spec().shape().dims();
    let sin_dims = sin.spec().shape().dims();
    if cos_dims != sin_dims {
        return Err(RuntimeError::Backend(String::from(
            "cuda rotary_embedding requires matching cos and sin table shapes",
        )));
    }
    let batched = match cos_dims {
        [seq, dim] if *seq == sequence_length && *dim == half_dim => false,
        [batch, seq, dim]
            if *batch == batch_size && *seq == sequence_length && *dim == half_dim =>
        {
            true
        }
        _ => {
            return Err(RuntimeError::Backend(String::from(
                "cuda rotary_embedding only supports standard Parameter Golf rope tables",
            )));
        }
    };
    let cos_values = cos.read_f32()?;
    let sin_values = sin.read_f32()?;
    let head_dim = half_dim.checked_mul(2).ok_or_else(|| {
        RuntimeError::Backend(String::from(
            "cuda rotary_embedding head dimension overflow while validating rope tables",
        ))
    })?;
    let batch_extent = if batched { batch_size } else { 1 };
    for batch_index in 0..batch_extent {
        for position in 0..sequence_length {
            for feature in 0..half_dim {
                let exponent = (2 * feature) as f32 / head_dim as f32;
                let inv_freq = 1.0_f32 / CUDA_PUBLIC_STANDARD_ROPE_BASE.powf(exponent);
                let angle = position as f32 * inv_freq;
                let expected_cos = angle.cos();
                let expected_sin = angle.sin();
                let table_index = if batched {
                    (batch_index * sequence_length + position) * half_dim + feature
                } else {
                    position * half_dim + feature
                };
                if (cos_values[table_index] - expected_cos).abs() > 1e-5
                    || (sin_values[table_index] - expected_sin).abs() > 1e-5
                {
                    return Err(RuntimeError::Backend(String::from(
                        "cuda rotary_embedding only supports the standard Parameter Golf rope table values on the bounded public lane",
                    )));
                }
            }
        }
    }
    Ok(batched)
}

struct ScaledDotProductAttentionBackwardHostValues {
    query: Vec<f32>,
    key: Vec<f32>,
    value: Vec<f32>,
}

fn scaled_dot_product_attention_backward_host_values(
    query: &[f32],
    query_dims: &[usize],
    key: &[f32],
    key_dims: &[usize],
    value: &[f32],
    _value_dims: &[usize],
    grad_output: &[f32],
    scale: f32,
) -> Result<ScaledDotProductAttentionBackwardHostValues, RuntimeError> {
    let query_heads = query_dims[1];
    let sequence_length = query_dims[2];
    let head_dim = query_dims[3];
    let kv_heads = key_dims[1];
    let group_size = query_heads / kv_heads;
    let mut query_gradient = vec![0.0_f32; query.len()];
    let mut key_gradient = vec![0.0_f32; key.len()];
    let mut value_gradient = vec![0.0_f32; value.len()];
    let query_batch_width = query_heads
        .checked_mul(sequence_length)
        .and_then(|value| value.checked_mul(head_dim))
        .ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda scaled_dot_product_attention backward query batch width overflow",
            ))
        })?;
    let kv_batch_width = kv_heads
        .checked_mul(sequence_length)
        .and_then(|value| value.checked_mul(head_dim))
        .ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda scaled_dot_product_attention backward kv batch width overflow",
            ))
        })?;

    query_gradient
        .par_chunks_mut(query_batch_width)
        .zip(key_gradient.par_chunks_mut(kv_batch_width))
        .zip(value_gradient.par_chunks_mut(kv_batch_width))
        .enumerate()
        .try_for_each(
            |(batch_index, ((query_gradient_batch, key_gradient_batch), value_gradient_batch))| {
                let query_batch_start = batch_index
                    .checked_mul(query_batch_width)
                    .ok_or_else(|| {
                        RuntimeError::Backend(String::from(
                            "cuda scaled_dot_product_attention backward query batch offset overflow",
                        ))
                    })?;
                let kv_batch_start = batch_index.checked_mul(kv_batch_width).ok_or_else(|| {
                    RuntimeError::Backend(String::from(
                        "cuda scaled_dot_product_attention backward kv batch offset overflow",
                    ))
                })?;
                let query_batch = &query[query_batch_start..query_batch_start + query_batch_width];
                let key_batch = &key[kv_batch_start..kv_batch_start + kv_batch_width];
                let value_batch = &value[kv_batch_start..kv_batch_start + kv_batch_width];
                let grad_output_batch =
                    &grad_output[query_batch_start..query_batch_start + query_batch_width];
                let mut scores = vec![0.0_f32; sequence_length];
                let mut weights = vec![0.0_f32; sequence_length];
                let mut grad_weights = vec![0.0_f32; sequence_length];

                for query_head in 0..query_heads {
                    let kv_head = query_head / group_size;
                    let query_head_offset = query_head * sequence_length * head_dim;
                    let kv_head_offset = kv_head * sequence_length * head_dim;
                    for query_position in 0..sequence_length {
                        let query_base = query_head_offset + query_position * head_dim;
                        let query_row = &query_batch[query_base..query_base + head_dim];
                        let grad_output_row =
                            &grad_output_batch[query_base..query_base + head_dim];
                        let valid_scores = query_position + 1;
                        let mut max_score = f32::NEG_INFINITY;
                        for key_position in 0..valid_scores {
                            let key_base = kv_head_offset + key_position * head_dim;
                            let mut dot = 0.0_f32;
                            for feature in 0..head_dim {
                                dot += query_row[feature] * key_batch[key_base + feature];
                            }
                            let score = dot * scale;
                            scores[key_position] = score;
                            max_score = max_score.max(score);
                        }
                        let mut denom = 0.0_f32;
                        for key_position in 0..valid_scores {
                            let weight = (scores[key_position] - max_score).exp();
                            weights[key_position] = weight;
                            denom += weight;
                        }
                        for key_position in 0..valid_scores {
                            weights[key_position] /= denom;
                        }

                        let mut weighted_grad_sum = 0.0_f32;
                        for key_position in 0..valid_scores {
                            let value_base = kv_head_offset + key_position * head_dim;
                            let mut grad_weight = 0.0_f32;
                            for feature in 0..head_dim {
                                grad_weight +=
                                    grad_output_row[feature] * value_batch[value_base + feature];
                            }
                            grad_weights[key_position] = grad_weight;
                            weighted_grad_sum += weights[key_position] * grad_weight;
                        }

                        for key_position in 0..valid_scores {
                            let key_base = kv_head_offset + key_position * head_dim;
                            let value_base = kv_head_offset + key_position * head_dim;
                            let weight = weights[key_position];
                            let grad_score =
                                weight * (grad_weights[key_position] - weighted_grad_sum);
                            let query_scale = grad_score * scale;
                            for feature in 0..head_dim {
                                query_gradient_batch[query_base + feature] +=
                                    query_scale * key_batch[key_base + feature];
                                key_gradient_batch[key_base + feature] +=
                                    query_scale * query_row[feature];
                                value_gradient_batch[value_base + feature] +=
                                    weight * grad_output_row[feature];
                            }
                        }
                    }
                }
                Ok::<(), RuntimeError>(())
            },
        )?;

    Ok(ScaledDotProductAttentionBackwardHostValues {
        query: query_gradient,
        key: key_gradient,
        value: value_gradient,
    })
}

fn binary_inputs<'a>(
    step: &ExecutionStep,
    values: &'a BTreeMap<TensorId, CudaBuffer>,
) -> Result<(&'a CudaBuffer, &'a CudaBuffer), RuntimeError> {
    let Some(left_id) = step.inputs.first().copied() else {
        return Err(RuntimeError::Backend(format!(
            "missing left input for step {}",
            step.output
        )));
    };
    let Some(right_id) = step.inputs.get(1).copied() else {
        return Err(RuntimeError::Backend(format!(
            "missing right input for step {}",
            step.output
        )));
    };
    let left = values
        .get(&left_id)
        .ok_or(RuntimeError::MissingInput(left_id))?;
    let right = values
        .get(&right_id)
        .ok_or(RuntimeError::MissingInput(right_id))?;
    Ok((left, right))
}

fn step_input<'a>(
    step: &ExecutionStep,
    values: &'a BTreeMap<TensorId, CudaBuffer>,
    input_index: usize,
) -> Result<&'a CudaBuffer, RuntimeError> {
    let Some(input_id) = step.inputs.get(input_index).copied() else {
        return Err(RuntimeError::Backend(format!(
            "missing input index {input_index} for step {}",
            step.output
        )));
    };
    values
        .get(&input_id)
        .ok_or(RuntimeError::MissingInput(input_id))
}

fn scaled_dot_product_attention_backward_cache_key(
    step: &ExecutionStep,
    scale: f32,
    causal: bool,
) -> Result<ScaledDotProductAttentionBackwardCacheKey, RuntimeError> {
    if step.inputs.len() != 4 {
        return Err(RuntimeError::Backend(format!(
            "cuda {} requires four inputs to build the backward cache key",
            step.op.label()
        )));
    }
    Ok(ScaledDotProductAttentionBackwardCacheKey {
        query: step.inputs[0],
        key: step.inputs[1],
        value: step.inputs[2],
        grad_output: step.inputs[3],
        scale_bits: scale.to_bits(),
        causal,
    })
}

fn supports_cuda_scaled_dot_product_attention_backward_device_path(
    step: &ExecutionStep,
    values: &BTreeMap<TensorId, CudaBuffer>,
) -> Result<bool, RuntimeError> {
    let query = step_input(step, values, 0)?;
    let key = step_input(step, values, 1)?;
    let value = step_input(step, values, 2)?;
    let grad_output = step_input(step, values, 3)?;
    Ok((query.spec().dtype() == DType::F32
        && key.spec().dtype() == DType::F32
        && value.spec().dtype() == DType::F32
        && grad_output.spec().dtype() == DType::F32)
        || (query.spec().dtype() == DType::BF16
            && key.spec().dtype() == DType::BF16
            && value.spec().dtype() == DType::BF16
            && grad_output.spec().dtype() == DType::BF16))
}

fn supports_cuda_rank2_transpose(input_shape: &[usize], axes: &[usize]) -> bool {
    input_shape.len() == 2 && axes == [1, 0]
}

fn supports_cuda_rank4_swap_middle_axes(input_shape: &[usize], axes: &[usize]) -> bool {
    input_shape.len() == 4 && axes == [0, 2, 1, 3]
}

fn supports_cuda_rank3_expand_f32(input_shape: &[usize], output_shape: &[usize]) -> bool {
    input_shape.len() == 3
        && output_shape.len() == 3
        && input_shape
            .iter()
            .zip(output_shape)
            .all(|(&input_dim, &output_dim)| input_dim == output_dim || input_dim == 1)
}

fn supports_cuda_rank4_expand_f32(input_shape: &[usize], output_shape: &[usize]) -> bool {
    input_shape.len() == 4
        && output_shape.len() == 4
        && input_shape
            .iter()
            .zip(output_shape)
            .all(|(&input_dim, &output_dim)| input_dim == output_dim || input_dim == 1)
}

fn reduce_sum_cuda_strategy(
    input_shape: &[usize],
    axis: Option<usize>,
) -> Option<ReduceSumCudaStrategy> {
    if input_shape.is_empty() {
        return None;
    }
    let total_elements = input_shape.iter().product::<usize>();
    match axis {
        None => Some(ReduceSumCudaStrategy::Rows {
            row_count: 1,
            column_count: total_elements,
        }),
        Some(axis_index) if axis_index == input_shape.len() - 1 => {
            let column_count = *input_shape.last()?;
            Some(ReduceSumCudaStrategy::Rows {
                row_count: total_elements / column_count,
                column_count,
            })
        }
        Some(0) => Some(ReduceSumCudaStrategy::Axis0 {
            axis0_extent: input_shape[0],
            row_width: total_elements / input_shape[0],
        }),
        Some(1) if input_shape.len() == 3 => Some(ReduceSumCudaStrategy::Axis1Rank3 {
            dim0: input_shape[0],
            dim1: input_shape[1],
            dim2: input_shape[2],
        }),
        Some(_) => None,
    }
}

fn discovery_report_internal() -> Result<NvidiaBackendReport, RuntimeError> {
    match query_inventory() {
        Ok(rows) => {
            let devices = rows
                .into_iter()
                .map(NvidiaInventoryRow::into_device_descriptor)
                .collect::<Vec<_>>();
            let health = cuda_health(&devices);
            Ok(NvidiaBackendReport { devices, health })
        }
        Err(error) if error.kind == NvidiaQueryErrorKind::NotInstalled => Ok(NvidiaBackendReport {
            devices: Vec::new(),
            health: RuntimeHealth {
                status: HealthStatus::Offline,
                message: String::from(OFFLINE_NO_DRIVER_MESSAGE),
            },
        }),
        Err(error) => Ok(NvidiaBackendReport {
            devices: Vec::new(),
            health: RuntimeHealth {
                status: HealthStatus::Offline,
                message: error.message,
            },
        }),
    }
}

fn architecture_from_compute_capability(compute_capability: Option<&str>) -> Option<String> {
    let architecture = match compute_capability?.trim() {
        "7.0" | "7.2" => "volta",
        "7.5" => "turing",
        "8.0" | "8.6" | "8.7" => "ampere",
        "8.9" => "ada",
        "9.0" => "hopper",
        _ => return None,
    };
    Some(String::from(architecture))
}

fn risk_profile(
    display_attached: Option<bool>,
    mig_profile: Option<&str>,
    persistence_mode_enabled: Option<bool>,
) -> NvidiaRiskProfile {
    let mut warnings = Vec::new();
    let mig_partitioned = mig_profile.is_some();
    if display_attached == Some(true) {
        warnings.push(String::from(
            "display-attached NVIDIA devices may show variable latency under local desktop load",
        ));
    }
    if mig_partitioned {
        warnings.push(String::from(
            "MIG-enabled NVIDIA devices expose only a partitioned slice of the physical GPU",
        ));
    }
    if persistence_mode_enabled == Some(false) {
        warnings.push(String::from(
            "persistence mode disabled; first request after idle may pay driver-load latency",
        ));
    }

    NvidiaRiskProfile {
        level: if display_attached == Some(true) || mig_partitioned {
            NvidiaRiskLevel::Elevated
        } else {
            NvidiaRiskLevel::Standard
        },
        display_attached,
        mig_partitioned,
        warnings,
    }
}

fn recovery_profile(recovery_action: Option<&str>) -> NvidiaRecoveryProfile {
    let normalized = normalize_value(recovery_action.unwrap_or_default())
        .map(|value| value.to_ascii_lowercase());
    match normalized.as_deref() {
        Some("none") => NvidiaRecoveryProfile {
            supports_gpu_reset: Some(true),
            expected_actions: vec![
                NvidiaRecoveryAction::ProcessRestart,
                NvidiaRecoveryAction::GpuReset,
                NvidiaRecoveryAction::RebootHost,
            ],
        },
        Some(value) if value.contains("reboot") => NvidiaRecoveryProfile {
            supports_gpu_reset: Some(false),
            expected_actions: vec![
                NvidiaRecoveryAction::ProcessRestart,
                NvidiaRecoveryAction::RebootHost,
            ],
        },
        Some(value) if value.contains("reset") => NvidiaRecoveryProfile {
            supports_gpu_reset: Some(true),
            expected_actions: vec![
                NvidiaRecoveryAction::ProcessRestart,
                NvidiaRecoveryAction::GpuReset,
                NvidiaRecoveryAction::RebootHost,
            ],
        },
        _ => NvidiaRecoveryProfile {
            supports_gpu_reset: None,
            expected_actions: vec![
                NvidiaRecoveryAction::ProcessRestart,
                NvidiaRecoveryAction::RebootHost,
            ],
        },
    }
}

fn cuda_health(devices: &[DeviceDescriptor]) -> RuntimeHealth {
    if devices.is_empty() {
        return RuntimeHealth {
            status: HealthStatus::Offline,
            message: String::from("cuda query succeeded but no NVIDIA GPUs were detected"),
        };
    }

    let elevated_devices = devices
        .iter()
        .filter(|device| {
            device
                .nvidia_metadata
                .as_ref()
                .map(|metadata| metadata.risk.level == NvidiaRiskLevel::Elevated)
                .unwrap_or(false)
        })
        .count();
    if elevated_devices > 0 {
        return RuntimeHealth {
            status: HealthStatus::Degraded,
            message: format!(
                "cuda discovered {} NVIDIA device(s); {} device(s) are display-attached or MIG-partitioned",
                devices.len(),
                elevated_devices
            ),
        };
    }

    RuntimeHealth {
        status: HealthStatus::Ready,
        message: format!("cuda ready on {} NVIDIA device(s)", devices.len()),
    }
}

fn query_inventory() -> Result<Vec<NvidiaInventoryRow>, NvidiaQueryError> {
    let stdout = run_nvidia_smi_query(INVENTORY_QUERY, None)?;
    let mut rows = Vec::new();
    for line in stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        rows.push(parse_inventory_row(line)?);
    }
    Ok(rows)
}

fn query_recovery_action(ordinal: u16) -> Result<String, NvidiaQueryError> {
    run_nvidia_smi_query("gpu_recovery_action", Some(ordinal))
        .map(|output| output.lines().next().unwrap_or_default().trim().to_string())
}

fn run_nvidia_smi_query(query: &str, ordinal: Option<u16>) -> Result<String, NvidiaQueryError> {
    let mut command = Command::new(NVIDIA_SMI_BINARY);
    command.arg(format!("--query-gpu={query}"));
    if let Some(ordinal) = ordinal {
        command.arg(format!("--id={ordinal}"));
    }
    command.args(["--format=csv,noheader,nounits"]);

    let output = command.output().map_err(|error| {
        let kind = if error.kind() == ErrorKind::NotFound {
            NvidiaQueryErrorKind::NotInstalled
        } else {
            NvidiaQueryErrorKind::Failed
        };
        NvidiaQueryError {
            kind,
            message: if kind == NvidiaQueryErrorKind::NotInstalled {
                String::from(OFFLINE_NO_DRIVER_MESSAGE)
            } else {
                format!("failed to execute {NVIDIA_SMI_BINARY}: {error}")
            },
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(NvidiaQueryError {
            kind: NvidiaQueryErrorKind::Failed,
            message: if stderr.is_empty() {
                format!("{NVIDIA_SMI_BINARY} exited with {}", output.status)
            } else {
                format!("{NVIDIA_SMI_BINARY} query failed: {stderr}")
            },
        });
    }

    String::from_utf8(output.stdout).map_err(|error| NvidiaQueryError {
        kind: NvidiaQueryErrorKind::InvalidUtf8,
        message: format!("{NVIDIA_SMI_BINARY} returned non-utf8 output: {error}"),
    })
}

fn parse_inventory_row(line: &str) -> Result<NvidiaInventoryRow, NvidiaQueryError> {
    let fields = line.split(',').map(str::trim).collect::<Vec<_>>();
    if fields.len() != 9 {
        return Err(NvidiaQueryError {
            kind: NvidiaQueryErrorKind::Failed,
            message: format!(
                "nvidia-smi returned {} inventory fields, expected 9: {line}",
                fields.len()
            ),
        });
    }

    let ordinal = fields[0].parse::<u16>().map_err(|error| NvidiaQueryError {
        kind: NvidiaQueryErrorKind::Failed,
        message: format!("invalid CUDA device ordinal {:?}: {error}", fields[0]),
    })?;

    Ok(NvidiaInventoryRow {
        ordinal,
        name: fields[1].to_string(),
        pci_bdf: normalize_value(fields[2]),
        memory_bytes: parse_memory_bytes(fields[3]),
        compute_capability: normalize_value(fields[4]),
        display_attached: parse_yes_no(fields[5]),
        mig_profile: parse_mig_profile(fields[6]),
        persistence_mode_enabled: parse_enabled_disabled(fields[7]),
        addressing_mode: normalize_value(fields[8]).map(|value| value.to_ascii_lowercase()),
    })
}

fn normalize_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("[N/A]")
        || trimmed.eq_ignore_ascii_case("N/A")
    {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_mig_profile(value: &str) -> Option<String> {
    let normalized = normalize_value(value)?;
    if normalized.eq_ignore_ascii_case("disabled")
        || normalized.eq_ignore_ascii_case("not supported")
    {
        None
    } else {
        Some(normalized.to_ascii_lowercase())
    }
}

fn parse_yes_no(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "enabled" => Some(true),
        "no" | "disabled" => Some(false),
        _ => None,
    }
}

fn parse_enabled_disabled(value: &str) -> Option<bool> {
    parse_yes_no(value)
}

fn parse_memory_bytes(value: &str) -> Option<u64> {
    normalize_value(value)?
        .parse::<u64>()
        .ok()
        .map(|mebibytes| mebibytes * 1024 * 1024)
}

#[cfg(target_os = "linux")]
mod platform {
    use std::{
        collections::{BTreeMap, BTreeSet},
        ffi::{CStr, c_char, c_int, c_longlong, c_void},
        sync::{Arc, Mutex},
    };

    use libloading::Library;

    use super::{
        CudaAllocatorPool, CudaAllocatorPoolTelemetry, CudaCommandStatus, CudaCommandWait,
        QuantizationMode, allocator_pool_cache_key,
    };
    use psionic_core::TensorSpec;
    use psionic_runtime::{AllocatorPoolPolicy, AllocatorPoolReport, RuntimeError};

    type CudaError = i32;
    type CudaStream = *mut c_void;
    type CudaGraph = *mut c_void;
    type CudaGraphExec = *mut c_void;
    type CudaEvent = *mut c_void;
    type CublasStatus = i32;
    type CublasHandle = *mut c_void;
    type CublasLtHandle = *mut c_void;
    type CublasLtMatmulDesc = *mut c_void;
    type CublasLtMatrixLayout = *mut c_void;
    type CublasLtMatmulPreference = *mut c_void;

    const CUDA_SUCCESS: CudaError = 0;
    const CUBLAS_STATUS_SUCCESS: CublasStatus = 0;
    const CUDA_MEMCPY_HOST_TO_DEVICE: c_int = 1;
    const CUDA_MEMCPY_DEVICE_TO_HOST: c_int = 2;
    const CUDA_MEMCPY_DEVICE_TO_DEVICE: c_int = 3;
    const CUBLAS_OP_N: c_int = 0;
    const CUBLAS_OP_T: c_int = 1;
    const CUDA_R_32F: c_int = 0;
    const CUDA_R_16F: c_int = 2;
    const CUDA_R_16BF: c_int = 14;
    const CUDA_STREAM_CAPTURE_MODE_RELAXED: c_int = 2;
    const CUBLASLT_MATMUL_DESC_TRANSA: u32 = 0;
    const CUBLASLT_MATMUL_DESC_TRANSB: u32 = 1;
    const CUBLASLT_MATMUL_PREF_MAX_WORKSPACE_BYTES: u32 = 1;
    const CUBLAS_COMPUTE_32F: c_int = 68;
    const CUBLAS_COMPUTE_32F_FAST_16F: c_int = 74;
    const CUBLAS_GEMM_DEFAULT_TENSOR_OP: c_int = 99;
    const CUBLASLT_DEFAULT_MAX_WORKSPACE_BYTES: usize = 4 * 1024 * 1024;
    const CUBLASLT_MAX_CANDIDATES: usize = 6;
    const CUBLASLT_WARMUP_ITERS: usize = 1;
    const CUBLASLT_BENCH_ITERS: usize = 2;
    const CUBLASLT_PLAN_CACHE_SCOPE: &str = "per_device_runtime_owner";
    const CUBLASLT_F16_TO_F32_BACKEND_ROUTE: &str = "cublaslt_matmul_f16_to_f32";

    type CudaGetErrorString = unsafe extern "C" fn(CudaError) -> *const c_char;
    type CudaSetDevice = unsafe extern "C" fn(c_int) -> CudaError;
    type CudaMalloc = unsafe extern "C" fn(*mut *mut c_void, usize) -> CudaError;
    type CudaMallocHost = unsafe extern "C" fn(*mut *mut c_void, usize) -> CudaError;
    type CudaMemGetInfo = unsafe extern "C" fn(*mut usize, *mut usize) -> CudaError;
    type CudaFree = unsafe extern "C" fn(*mut c_void) -> CudaError;
    type CudaFreeHost = unsafe extern "C" fn(*mut c_void) -> CudaError;
    type CudaMemcpy = unsafe extern "C" fn(*mut c_void, *const c_void, usize, c_int) -> CudaError;
    type CudaMemcpyAsync =
        unsafe extern "C" fn(*mut c_void, *const c_void, usize, c_int, CudaStream) -> CudaError;
    type CudaMemsetAsync = unsafe extern "C" fn(*mut c_void, c_int, usize, CudaStream) -> CudaError;
    type CudaStreamCreate = unsafe extern "C" fn(*mut CudaStream) -> CudaError;
    type CudaStreamDestroy = unsafe extern "C" fn(CudaStream) -> CudaError;
    type CudaStreamSynchronize = unsafe extern "C" fn(CudaStream) -> CudaError;
    type CudaStreamBeginCapture = unsafe extern "C" fn(CudaStream, c_int) -> CudaError;
    type CudaStreamEndCapture = unsafe extern "C" fn(CudaStream, *mut CudaGraph) -> CudaError;
    type CudaGraphInstantiate =
        unsafe extern "C" fn(*mut CudaGraphExec, CudaGraph, u64) -> CudaError;
    type CudaGraphLaunch = unsafe extern "C" fn(CudaGraphExec, CudaStream) -> CudaError;
    type CudaGraphExecDestroy = unsafe extern "C" fn(CudaGraphExec) -> CudaError;
    type CudaGraphDestroy = unsafe extern "C" fn(CudaGraph) -> CudaError;
    type CudaEventCreate = unsafe extern "C" fn(*mut CudaEvent) -> CudaError;
    type CudaEventDestroy = unsafe extern "C" fn(CudaEvent) -> CudaError;
    type CudaEventRecord = unsafe extern "C" fn(CudaEvent, CudaStream) -> CudaError;
    type CudaEventSynchronize = unsafe extern "C" fn(CudaEvent) -> CudaError;
    type CudaEventElapsedTime = unsafe extern "C" fn(*mut f32, CudaEvent, CudaEvent) -> CudaError;
    type CublasCreate = unsafe extern "C" fn(*mut CublasHandle) -> CublasStatus;
    type CublasDestroy = unsafe extern "C" fn(CublasHandle) -> CublasStatus;
    type CublasSetStream = unsafe extern "C" fn(CublasHandle, CudaStream) -> CublasStatus;
    type CublasSgemm = unsafe extern "C" fn(
        CublasHandle,
        c_int,
        c_int,
        c_int,
        c_int,
        c_int,
        *const f32,
        *const f32,
        c_int,
        *const f32,
        c_int,
        *const f32,
        *mut f32,
        c_int,
    ) -> CublasStatus;
    type CublasSgeam = unsafe extern "C" fn(
        CublasHandle,
        c_int,
        c_int,
        c_int,
        c_int,
        *const f32,
        *const f32,
        c_int,
        *const f32,
        *const f32,
        c_int,
        *mut f32,
        c_int,
    ) -> CublasStatus;
    type CublasGemmEx = unsafe extern "C" fn(
        CublasHandle,
        c_int,
        c_int,
        c_int,
        c_int,
        c_int,
        *const c_void,
        *const c_void,
        c_int,
        c_int,
        *const c_void,
        c_int,
        c_int,
        *const c_void,
        *mut c_void,
        c_int,
        c_int,
        c_int,
        c_int,
    ) -> CublasStatus;
    type CublasGemmStridedBatchedEx = unsafe extern "C" fn(
        CublasHandle,
        c_int,
        c_int,
        c_int,
        c_int,
        c_int,
        *const c_void,
        *const c_void,
        c_int,
        c_int,
        c_longlong,
        *const c_void,
        c_int,
        c_int,
        c_longlong,
        *const c_void,
        *mut c_void,
        c_int,
        c_int,
        c_longlong,
        c_int,
        c_int,
        c_int,
    ) -> CublasStatus;
    type CublasLtCreate = unsafe extern "C" fn(*mut CublasLtHandle) -> CublasStatus;
    type CublasLtDestroy = unsafe extern "C" fn(CublasLtHandle) -> CublasStatus;
    type CublasLtMatmulDescCreate =
        unsafe extern "C" fn(*mut CublasLtMatmulDesc, c_int, c_int) -> CublasStatus;
    type CublasLtMatmulDescDestroy = unsafe extern "C" fn(CublasLtMatmulDesc) -> CublasStatus;
    type CublasLtMatmulDescSetAttribute =
        unsafe extern "C" fn(CublasLtMatmulDesc, u32, *const c_void, usize) -> CublasStatus;
    type CublasLtMatrixLayoutCreate =
        unsafe extern "C" fn(*mut CublasLtMatrixLayout, c_int, u64, u64, i64) -> CublasStatus;
    type CublasLtMatrixLayoutDestroy = unsafe extern "C" fn(CublasLtMatrixLayout) -> CublasStatus;
    type CublasLtMatmulPreferenceCreate =
        unsafe extern "C" fn(*mut CublasLtMatmulPreference) -> CublasStatus;
    type CublasLtMatmulPreferenceDestroy =
        unsafe extern "C" fn(CublasLtMatmulPreference) -> CublasStatus;
    type CublasLtMatmulPreferenceSetAttribute =
        unsafe extern "C" fn(CublasLtMatmulPreference, u32, *const c_void, usize) -> CublasStatus;
    type CublasLtMatmulAlgoGetHeuristic = unsafe extern "C" fn(
        CublasLtHandle,
        CublasLtMatmulDesc,
        CublasLtMatrixLayout,
        CublasLtMatrixLayout,
        CublasLtMatrixLayout,
        CublasLtMatrixLayout,
        CublasLtMatmulPreference,
        c_int,
        *mut CublasLtMatmulHeuristicResult,
        *mut c_int,
    ) -> CublasStatus;
    type CublasLtMatmul = unsafe extern "C" fn(
        CublasLtHandle,
        CublasLtMatmulDesc,
        *const c_void,
        *const c_void,
        CublasLtMatrixLayout,
        *const c_void,
        CublasLtMatrixLayout,
        *const c_void,
        *mut c_void,
        CublasLtMatrixLayout,
        *mut c_void,
        CublasLtMatrixLayout,
        *const CublasLtMatmulAlgo,
        *mut c_void,
        usize,
        CudaStream,
    ) -> CublasStatus;

    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct CublasLtMatmulAlgo {
        data: [u64; 8],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CublasLtMatmulHeuristicResult {
        algo: CublasLtMatmulAlgo,
        workspace_size: usize,
        state: CublasStatus,
        waves_count: f32,
        reserved: [c_int; 4],
    }

    fn cublas_row_major_op(transpose: bool) -> c_int {
        if transpose { CUBLAS_OP_T } else { CUBLAS_OP_N }
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    struct CublasLtPlanKey {
        scope: super::CudaGemmTuningScope,
        rows: usize,
        inner: usize,
        cols: usize,
    }

    struct CublasLtPlan {
        key: CublasLtPlanKey,
        desc: CublasLtMatmulDesc,
        right_layout: CublasLtMatrixLayout,
        left_layout: CublasLtMatrixLayout,
        output_layout: CublasLtMatrixLayout,
        algo: CublasLtMatmulAlgo,
        workspace_size: usize,
        mean_time_us: u64,
    }

    struct CublasLtContext {
        _library: Library,
        handle: CublasLtHandle,
        destroy: CublasLtDestroy,
        matmul_desc_create: CublasLtMatmulDescCreate,
        matmul_desc_destroy: CublasLtMatmulDescDestroy,
        matmul_desc_set_attribute: CublasLtMatmulDescSetAttribute,
        matrix_layout_create: CublasLtMatrixLayoutCreate,
        matrix_layout_destroy: CublasLtMatrixLayoutDestroy,
        matmul_preference_create: CublasLtMatmulPreferenceCreate,
        matmul_preference_destroy: CublasLtMatmulPreferenceDestroy,
        matmul_preference_set_attribute: CublasLtMatmulPreferenceSetAttribute,
        matmul_algo_get_heuristic: CublasLtMatmulAlgoGetHeuristic,
        matmul: CublasLtMatmul,
        workspace_ptr: *mut c_void,
        workspace_size: usize,
        weight_scopes: BTreeMap<usize, super::CudaGemmTuningScope>,
        plans: BTreeMap<CublasLtPlanKey, CublasLtPlan>,
        fallback_shapes: BTreeSet<CublasLtPlanKey>,
        unavailable_reason: Option<String>,
    }

    struct CublasLtMatmulDescGuard {
        raw: CublasLtMatmulDesc,
        destroy: CublasLtMatmulDescDestroy,
    }

    impl CublasLtMatmulDescGuard {
        fn into_raw(self) -> CublasLtMatmulDesc {
            let raw = self.raw;
            std::mem::forget(self);
            raw
        }
    }

    impl Drop for CublasLtMatmulDescGuard {
        fn drop(&mut self) {
            if !self.raw.is_null() {
                let _ = unsafe { (self.destroy)(self.raw) };
                self.raw = std::ptr::null_mut();
            }
        }
    }

    struct CublasLtMatrixLayoutGuard {
        raw: CublasLtMatrixLayout,
        destroy: CublasLtMatrixLayoutDestroy,
    }

    impl CublasLtMatrixLayoutGuard {
        fn into_raw(self) -> CublasLtMatrixLayout {
            let raw = self.raw;
            std::mem::forget(self);
            raw
        }
    }

    impl Drop for CublasLtMatrixLayoutGuard {
        fn drop(&mut self) {
            if !self.raw.is_null() {
                let _ = unsafe { (self.destroy)(self.raw) };
                self.raw = std::ptr::null_mut();
            }
        }
    }

    struct CublasLtMatmulPreferenceGuard {
        raw: CublasLtMatmulPreference,
        destroy: CublasLtMatmulPreferenceDestroy,
    }

    impl Drop for CublasLtMatmulPreferenceGuard {
        fn drop(&mut self) {
            if !self.raw.is_null() {
                let _ = unsafe { (self.destroy)(self.raw) };
                self.raw = std::ptr::null_mut();
            }
        }
    }

    struct CudaEventGuard<'a> {
        runtime: &'a CudaRuntime,
        raw: CudaEvent,
    }

    impl<'a> CudaEventGuard<'a> {
        fn new(runtime: &'a CudaRuntime) -> Result<Self, RuntimeError> {
            let mut raw = std::ptr::null_mut();
            runtime.check(
                unsafe { (runtime.cuda_event_create)(&mut raw) },
                "cudaEventCreate",
            )?;
            Ok(Self { runtime, raw })
        }

        fn record(&self, stream: CudaStream) -> Result<(), RuntimeError> {
            self.runtime.check(
                unsafe { (self.runtime.cuda_event_record)(self.raw, stream) },
                "cudaEventRecord",
            )
        }

        fn synchronize(&self) -> Result<(), RuntimeError> {
            self.runtime.check(
                unsafe { (self.runtime.cuda_event_synchronize)(self.raw) },
                "cudaEventSynchronize",
            )
        }

        fn elapsed_ms(&self, start: &Self) -> Result<f32, RuntimeError> {
            let mut elapsed_ms = 0.0_f32;
            self.runtime.check(
                unsafe {
                    (self.runtime.cuda_event_elapsed_time)(&mut elapsed_ms, start.raw, self.raw)
                },
                "cudaEventElapsedTime",
            )?;
            Ok(elapsed_ms)
        }
    }

    impl Drop for CudaEventGuard<'_> {
        fn drop(&mut self) {
            if !self.raw.is_null() {
                let _ = self.runtime.set_device();
                let _ = self.runtime.check(
                    unsafe { (self.runtime.cuda_event_destroy)(self.raw) },
                    "cudaEventDestroy",
                );
                self.raw = std::ptr::null_mut();
            }
        }
    }

    impl Drop for CublasLtContext {
        fn drop(&mut self) {
            for plan in self.plans.values_mut() {
                if !plan.desc.is_null() {
                    let _ = unsafe { (self.matmul_desc_destroy)(plan.desc) };
                    plan.desc = std::ptr::null_mut();
                }
                if !plan.right_layout.is_null() {
                    let _ = unsafe { (self.matrix_layout_destroy)(plan.right_layout) };
                    plan.right_layout = std::ptr::null_mut();
                }
                if !plan.left_layout.is_null() {
                    let _ = unsafe { (self.matrix_layout_destroy)(plan.left_layout) };
                    plan.left_layout = std::ptr::null_mut();
                }
                if !plan.output_layout.is_null() {
                    let _ = unsafe { (self.matrix_layout_destroy)(plan.output_layout) };
                    plan.output_layout = std::ptr::null_mut();
                }
            }
            if !self.workspace_ptr.is_null() {
                // The owning CUDA runtime frees this pointer before dropping the
                // context library handles.
                self.workspace_ptr = std::ptr::null_mut();
                self.workspace_size = 0;
            }
            if !self.handle.is_null() {
                let _ = unsafe { (self.destroy)(self.handle) };
                self.handle = std::ptr::null_mut();
            }
        }
    }

    type QuantizedMatvecKernel = unsafe extern "C" fn(
        *const c_void,
        c_int,
        c_int,
        c_int,
        *const c_void,
        *mut c_void,
        CudaStream,
    ) -> CudaError;
    type QuantizedMatvecQ81Kernel = unsafe extern "C" fn(
        *const c_void,
        c_int,
        c_int,
        c_int,
        *const c_void,
        *const c_void,
        *mut c_void,
        CudaStream,
    ) -> CudaError;
    type QuantizedMatvecQ81ArgmaxKernel = unsafe extern "C" fn(
        *const c_void,
        c_int,
        c_int,
        c_int,
        *const c_void,
        *const c_void,
        *mut c_void,
        CudaStream,
    ) -> CudaError;
    type QuantizeQ81Kernel =
        unsafe extern "C" fn(*const c_void, c_int, c_int, *mut c_void, CudaStream) -> CudaError;
    type DequantizeRowToF32Kernel = unsafe extern "C" fn(
        *const c_void,
        c_int,
        c_int,
        c_int,
        *const c_void,
        *mut c_void,
        CudaStream,
    ) -> CudaError;
    type GatherF16RowToF32Kernel = unsafe extern "C" fn(
        *const c_void,
        c_int,
        c_int,
        *const c_void,
        *mut c_void,
        CudaStream,
    ) -> CudaError;
    type GatherF32ByIndicesKernel = unsafe extern "C" fn(
        *const c_void,
        c_int,
        *const c_void,
        c_int,
        *mut c_void,
        CudaStream,
    ) -> CudaError;
    type SplitInterleavedQueryGateRmsNormF32Kernel = unsafe extern "C" fn(
        *const c_void,
        c_int,
        c_int,
        *const c_void,
        f32,
        *mut c_void,
        *mut c_void,
        CudaStream,
    ) -> CudaError;
    type PackQwen35KeyValueRmsNormF32Kernel = unsafe extern "C" fn(
        *const c_void,
        c_int,
        c_int,
        c_int,
        c_int,
        *const c_void,
        f32,
        *mut c_void,
        c_int,
        c_int,
        CudaStream,
    ) -> CudaError;
    type PackQwen35HybridQkvRmsNormF32Kernel = unsafe extern "C" fn(
        *const c_void,
        c_int,
        c_int,
        c_int,
        c_int,
        c_int,
        c_int,
        *const c_void,
        *const c_void,
        f32,
        *mut c_void,
        c_int,
        c_int,
        c_int,
        CudaStream,
    ) -> CudaError;

    unsafe extern "C" {
        fn psionic_cuda_quantized_kernels_compiled() -> c_int;
        fn psionic_cuda_q3_k_matvec(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q5_k_matvec(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_iq3_s_matvec(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_iq4_xs_matvec(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q8_0_matvec(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q4_k_matvec(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q6_k_matvec(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_mxfp4_matvec(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_quantize_q8_1(
            input: *const c_void,
            rows: c_int,
            cols: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q3_k_dequantize_row_to_f32(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            decode_params: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q5_k_dequantize_row_to_f32(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            decode_params: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_iq3_s_dequantize_row_to_f32(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            decode_params: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_iq4_xs_dequantize_row_to_f32(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            decode_params: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q8_0_dequantize_row_to_f32(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            decode_params: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_mxfp4_dequantize_row_to_f32(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            decode_params: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_cast_f32_to_f16(
            input: *const c_void,
            element_count: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_cast_f32_to_bf16(
            input: *const c_void,
            element_count: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_cast_bf16_to_f32(
            input: *const c_void,
            element_count: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_gather_f16_row_to_f32(
            input: *const c_void,
            rows: c_int,
            cols: c_int,
            decode_params: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_gather_f32_by_indices(
            input: *const c_void,
            input_len: c_int,
            indices: *const c_void,
            index_count: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q3_k_matvec_q8_1(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q5_k_matvec_q8_1(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_iq3_s_matvec_q8_1(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_iq4_xs_matvec_q8_1(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q8_0_matvec_q8_1(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_mxfp4_matvec_q8_1(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q4_k_matvec_q8_1(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q6_k_matvec_q8_1(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q3_k_matvec_q8_1_argmax(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q5_k_matvec_q8_1_argmax(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_iq3_s_matvec_q8_1_argmax(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_iq4_xs_matvec_q8_1_argmax(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q8_0_matvec_q8_1_argmax(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q4_k_matvec_q8_1_argmax(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q6_k_matvec_q8_1_argmax(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_mxfp4_matvec_q8_1_argmax(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_argmax_f32(
            input: *const c_void,
            rows: c_int,
            cols: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_top_k_f32(
            input: *const c_void,
            rows: c_int,
            cols: c_int,
            top_k: c_int,
            selected_indices: *mut c_void,
            selected_values: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_top_k_f32_one_row_partitioned(
            input: *const c_void,
            cols: c_int,
            top_k: c_int,
            partial_block_count: c_int,
            partial_indices: *mut c_void,
            partial_values: *mut c_void,
            selected_indices: *mut c_void,
            selected_values: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_apply_sampling_penalties_f32_sparse(
            logits: *mut c_void,
            vocab_size: c_int,
            token_ids: *const c_void,
            token_counts: *const c_void,
            active_token_count: c_int,
            repeat_penalty: f32,
            presence_penalty: f32,
            frequency_penalty: f32,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_top_k_f32_one_row_radix_sort_temp_storage_bytes(
            cols: c_int,
            temp_storage_bytes: *mut usize,
        ) -> CudaError;
        fn psionic_cuda_top_k_f32_one_row_radix_sort(
            input: *const c_void,
            cols: c_int,
            top_k: c_int,
            input_indices: *const c_void,
            sorted_values: *mut c_void,
            sorted_indices: *mut c_void,
            temp_storage: *mut c_void,
            temp_storage_bytes: usize,
            selected_indices: *mut c_void,
            selected_values: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_rms_norm(
            input: *const c_void,
            weight: *const c_void,
            element_count: c_int,
            feature_count: c_int,
            epsilon: f32,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_rms_norm_region(
            input: *const c_void,
            input_offset: c_int,
            weight: *const c_void,
            element_count: c_int,
            feature_count: c_int,
            epsilon: f32,
            output: *mut c_void,
            output_offset: c_int,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_parameter_golf_token_embedding_lookup(
            token_ids: *const c_void,
            token_embedding: *const c_void,
            row_count: c_int,
            vocab_size: c_int,
            width: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_parameter_golf_token_embedding_lookup_bf16_to_f32(
            token_ids: *const c_void,
            token_embedding: *const c_void,
            row_count: c_int,
            vocab_size: c_int,
            width: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_parameter_golf_token_embedding_lookup_backward(
            token_ids: *const c_void,
            grad_output: *const c_void,
            row_count: c_int,
            vocab_size: c_int,
            width: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_parameter_golf_projection_loss(
            logits: *const c_void,
            target_ids: *const c_void,
            row_count: c_int,
            vocab_size: c_int,
            logit_softcap: f32,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_parameter_golf_projection_token_losses(
            logits: *const c_void,
            target_ids: *const c_void,
            row_count: c_int,
            vocab_size: c_int,
            logit_softcap: f32,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_parameter_golf_projection_loss_backward(
            logits: *const c_void,
            target_ids: *const c_void,
            grad_output: *const c_void,
            row_count: c_int,
            vocab_size: c_int,
            logit_softcap: f32,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_mul_f32(
            left: *const c_void,
            right: *const c_void,
            element_count: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_silu_mul_f32(
            activation_input: *const c_void,
            activation_offset: c_int,
            rhs: *const c_void,
            rhs_offset: c_int,
            element_count: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_silu_mul_q8_1(
            activation_input: *const c_void,
            activation_offset: c_int,
            rhs: *const c_void,
            rhs_offset: c_int,
            element_count: c_int,
            output_q8_1: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_sigmoid_mul_f32(
            values: *const c_void,
            values_offset: c_int,
            gate: *const c_void,
            gate_offset: c_int,
            element_count: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_sigmoid_mul_q8_1(
            values: *const c_void,
            values_offset: c_int,
            gate: *const c_void,
            gate_offset: c_int,
            element_count: c_int,
            output_q8_1: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_split_interleaved_query_gate_f32(
            input: *const c_void,
            head_count: c_int,
            head_dim: c_int,
            query_output: *mut c_void,
            gate_output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_split_interleaved_query_gate_rms_norm_f32(
            input: *const c_void,
            head_count: c_int,
            head_dim: c_int,
            weight: *const c_void,
            epsilon: f32,
            query_output: *mut c_void,
            gate_output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_pack_qwen35_key_value_rms_norm_f32(
            input: *const c_void,
            key_offset: c_int,
            value_offset: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            weight: *const c_void,
            epsilon: f32,
            output: *mut c_void,
            output_key_offset: c_int,
            output_value_offset: c_int,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_pack_qwen35_hybrid_qkv_rms_norm_f32(
            input: *const c_void,
            q_offset: c_int,
            k_offset: c_int,
            v_offset: c_int,
            group_count: c_int,
            state_size: c_int,
            v_size: c_int,
            q_weight: *const c_void,
            k_weight: *const c_void,
            epsilon: f32,
            output: *mut c_void,
            output_q_offset: c_int,
            output_k_offset: c_int,
            output_v_offset: c_int,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_depthwise_causal_conv1d_step_f32(
            input: *const c_void,
            state: *mut c_void,
            weights: *const c_void,
            channels: c_int,
            kernel_size: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_depthwise_causal_conv1d_step_silu_f32(
            input: *const c_void,
            state: *mut c_void,
            weights: *const c_void,
            channels: c_int,
            kernel_size: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_qwen35_ssm_decay_beta_f32(
            input: *const c_void,
            alpha_offset: c_int,
            beta_offset: c_int,
            ssm_a: *const c_void,
            ssm_dt: *const c_void,
            element_count: c_int,
            decay_output: *mut c_void,
            beta_output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_gated_delta_step_f32(
            qkv: *const c_void,
            query_offset: c_int,
            key_offset: c_int,
            value_offset: c_int,
            decay: *const c_void,
            beta: *const c_void,
            state: *mut c_void,
            key_head_count: c_int,
            value_head_count: c_int,
            key_dim: c_int,
            value_dim: c_int,
            v_head_reordered: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_rms_norm_q8_1(
            input: *const c_void,
            weight: *const c_void,
            element_count: c_int,
            epsilon: f32,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_rms_norm_input_backward(
            input: *const c_void,
            weight: *const c_void,
            grad_output: *const c_void,
            element_count: c_int,
            feature_count: c_int,
            epsilon: f32,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_rms_norm_weight_backward(
            input: *const c_void,
            grad_output: *const c_void,
            element_count: c_int,
            feature_count: c_int,
            epsilon: f32,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_add_residual_rms_norm(
            input: *const c_void,
            residual: *const c_void,
            input_bias: *const c_void,
            weight: *const c_void,
            element_count: c_int,
            epsilon: f32,
            summed_output: *mut c_void,
            normalized_output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_add_residual_rms_norm_q8_1(
            input: *const c_void,
            residual: *const c_void,
            input_bias: *const c_void,
            weight: *const c_void,
            element_count: c_int,
            epsilon: f32,
            summed_output: *mut c_void,
            normalized_output: *mut c_void,
            quantized_output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_add_residual_rms_norm_q8_1_router_topk(
            input: *const c_void,
            residual: *const c_void,
            input_bias: *const c_void,
            weight: *const c_void,
            element_count: c_int,
            epsilon: f32,
            summed_output: *mut c_void,
            normalized_output: *mut c_void,
            quantized_output: *mut c_void,
            router_weights: *const c_void,
            router_bias: *const c_void,
            expert_count: c_int,
            top_k: c_int,
            selected_ids: *mut c_void,
            selected_weights: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_add_f32_offset_in_place(
            destination: *mut c_void,
            element_offset: c_int,
            rhs: *const c_void,
            element_count: c_int,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_rope_neox_in_place(
            values: *mut c_void,
            element_offset: c_int,
            head_count: c_int,
            head_dim: c_int,
            rotary_dim: c_int,
            position: c_int,
            freq_scale: f32,
            ext_factor: f32,
            corr_low: f32,
            corr_high: f32,
            theta_scale: f32,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_rotary_embedding_backward(
            grad_output: *const c_void,
            cos: *const c_void,
            sin: *const c_void,
            batch_size: c_int,
            head_count: c_int,
            sequence_length: c_int,
            head_dim: c_int,
            batched_tables: c_int,
            grad_input: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_permute_rank2_transpose_f32(
            input: *const c_void,
            rows: c_int,
            cols: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_permute_rank4_swap_middle_axes_f32(
            input: *const c_void,
            dim0: c_int,
            dim1: c_int,
            dim2: c_int,
            dim3: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_reduce_sum_rows_f32(
            input: *const c_void,
            row_count: c_int,
            column_count: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_reduce_sum_axis0_f32(
            input: *const c_void,
            axis0_extent: c_int,
            row_width: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_reduce_sum_axis1_rank3_f32(
            input: *const c_void,
            dim0: c_int,
            dim1: c_int,
            dim2: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_expand_rank3_f32(
            input: *const c_void,
            input_dim0: c_int,
            input_dim1: c_int,
            input_dim2: c_int,
            output_dim0: c_int,
            output_dim1: c_int,
            output_dim2: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_expand_rank4_f32(
            input: *const c_void,
            input_dim0: c_int,
            input_dim1: c_int,
            input_dim2: c_int,
            input_dim3: c_int,
            output_dim0: c_int,
            output_dim1: c_int,
            output_dim2: c_int,
            output_dim3: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_decode(
            query: *const c_void,
            query_offset: c_int,
            current_key: *const c_void,
            key_offset: c_int,
            current_value: *const c_void,
            value_offset: c_int,
            cache_keys: *const c_void,
            cache_values: *const c_void,
            cache_width: c_int,
            layer_offset: c_int,
            past_tokens: c_int,
            sliding_window: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            attention_sinks: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_causal_sequence_f32(
            query: *const c_void,
            key: *const c_void,
            value: *const c_void,
            batch_size: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            sequence_length: c_int,
            head_dim: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_causal_sequence_bf16(
            query: *const c_void,
            key: *const c_void,
            value: *const c_void,
            batch_size: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            sequence_length: c_int,
            head_dim: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_causal_row_softmax_in_place_f32(
            logits: *mut c_void,
            row_count: c_int,
            sequence_length: c_int,
            scale: f32,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_causal_row_softmax_backward_in_place_f32(
            probabilities: *const c_void,
            grad_probabilities: *mut c_void,
            row_count: c_int,
            sequence_length: c_int,
            post_scale: f32,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_causal_sequence_backward_f32(
            query: *const c_void,
            key: *const c_void,
            value: *const c_void,
            grad_output: *const c_void,
            batch_size: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            sequence_length: c_int,
            head_dim: c_int,
            query_gradient: *mut c_void,
            key_gradient: *mut c_void,
            value_gradient: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_causal_sequence_backward_bf16_to_f32(
            query: *const c_void,
            key: *const c_void,
            value: *const c_void,
            grad_output: *const c_void,
            batch_size: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            sequence_length: c_int,
            head_dim: c_int,
            query_gradient: *mut c_void,
            key_gradient: *mut c_void,
            value_gradient: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_decode_rope_cache(
            qkv: *const c_void,
            query_offset: c_int,
            key_offset: c_int,
            value_offset: c_int,
            cache_keys: *mut c_void,
            cache_values: *mut c_void,
            cache_width: c_int,
            layer_offset: c_int,
            past_tokens: c_int,
            sliding_window: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            rotary_dim: c_int,
            position: c_int,
            freq_scale: f32,
            ext_factor: f32,
            corr_low: f32,
            corr_high: f32,
            theta_scale: f32,
            attention_sinks: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_decode_rope_cache_f16_kv(
            qkv: *const c_void,
            query_offset: c_int,
            key_offset: c_int,
            value_offset: c_int,
            cache_keys: *mut c_void,
            cache_values: *mut c_void,
            cache_width: c_int,
            layer_offset: c_int,
            past_tokens: c_int,
            sliding_window: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            rotary_dim: c_int,
            position: c_int,
            freq_scale: f32,
            ext_factor: f32,
            corr_low: f32,
            corr_high: f32,
            theta_scale: f32,
            attention_sinks: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_decode_mrope_cache_f16_kv(
            qkv: *const c_void,
            query_offset: c_int,
            key_offset: c_int,
            value_offset: c_int,
            cache_keys: *mut c_void,
            cache_values: *mut c_void,
            cache_width: c_int,
            layer_offset: c_int,
            past_tokens: c_int,
            sliding_window: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            rotary_dim: c_int,
            position_t: c_int,
            position_h: c_int,
            position_w: c_int,
            section_t: c_int,
            section_h: c_int,
            section_w: c_int,
            section_extra: c_int,
            mrope_interleaved: c_int,
            freq_scale: f32,
            ext_factor: f32,
            corr_low: f32,
            corr_high: f32,
            theta_scale: f32,
            attention_sinks: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_decode_rope_cache_turboquant_kv(
            qkv: *const c_void,
            query_offset: c_int,
            key_offset: c_int,
            value_offset: c_int,
            cache_keys: *mut c_void,
            cache_values: *mut c_void,
            cache_width: c_int,
            layer_offset: c_int,
            past_tokens: c_int,
            sliding_window: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            rotary_dim: c_int,
            position: c_int,
            freq_scale: f32,
            ext_factor: f32,
            corr_low: f32,
            corr_high: f32,
            theta_scale: f32,
            attention_sinks: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_decode_rope_cache_f16_kv_q8_1(
            qkv: *const c_void,
            query_offset: c_int,
            key_offset: c_int,
            value_offset: c_int,
            cache_keys: *mut c_void,
            cache_values: *mut c_void,
            cache_width: c_int,
            layer_offset: c_int,
            past_tokens: c_int,
            sliding_window: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            rotary_dim: c_int,
            position: c_int,
            freq_scale: f32,
            ext_factor: f32,
            corr_low: f32,
            corr_high: f32,
            theta_scale: f32,
            attention_sinks: *const c_void,
            output_q8_1: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_decode_rope_cache_f16_kv_graph(
            qkv: *const c_void,
            query_offset: c_int,
            key_offset: c_int,
            value_offset: c_int,
            cache_keys: *mut c_void,
            cache_values: *mut c_void,
            cache_width: c_int,
            layer_offset: c_int,
            decode_params: *const c_void,
            sliding_window: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            rotary_dim: c_int,
            freq_scale: f32,
            ext_factor: f32,
            corr_low: f32,
            corr_high: f32,
            theta_scale: f32,
            attention_sinks: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_decode_rope_cache_f16_kv_graph_fa3(
            qkv: *const c_void,
            query_offset: c_int,
            key_offset: c_int,
            value_offset: c_int,
            cache_keys: *mut c_void,
            cache_values: *mut c_void,
            cache_width: c_int,
            layer_offset: c_int,
            decode_params: *const c_void,
            sliding_window: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            rotary_dim: c_int,
            freq_scale: f32,
            ext_factor: f32,
            corr_low: f32,
            corr_high: f32,
            theta_scale: f32,
            attention_sinks: *const c_void,
            partial_output: *mut c_void,
            partial_max: *mut c_void,
            partial_sum: *mut c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_decode_rope_cache_turboquant_kv_graph(
            qkv: *const c_void,
            query_offset: c_int,
            key_offset: c_int,
            value_offset: c_int,
            cache_keys: *mut c_void,
            cache_values: *mut c_void,
            cache_width: c_int,
            layer_offset: c_int,
            decode_params: *const c_void,
            sliding_window: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            rotary_dim: c_int,
            freq_scale: f32,
            ext_factor: f32,
            corr_low: f32,
            corr_high: f32,
            theta_scale: f32,
            attention_sinks: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_decode_rope_cache_f16_kv_graph_q8_1(
            qkv: *const c_void,
            query_offset: c_int,
            key_offset: c_int,
            value_offset: c_int,
            cache_keys: *mut c_void,
            cache_values: *mut c_void,
            cache_width: c_int,
            layer_offset: c_int,
            decode_params: *const c_void,
            sliding_window: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            rotary_dim: c_int,
            freq_scale: f32,
            ext_factor: f32,
            corr_low: f32,
            corr_high: f32,
            theta_scale: f32,
            attention_sinks: *const c_void,
            output_q8_1: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_router_topk_softmax(
            weights: *const c_void,
            bias: *const c_void,
            input: *const c_void,
            expert_count: c_int,
            input_size: c_int,
            top_k: c_int,
            selected_ids: *mut c_void,
            selected_weights: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_router_topk_delayed_softmax(
            logits: *const c_void,
            expert_count: c_int,
            top_k: c_int,
            selected_ids: *mut c_void,
            selected_weights: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_moe_gate_up_swiglu_q8_1(
            weights: *const c_void,
            mode: c_int,
            row_stride: c_int,
            rows_per_expert: c_int,
            columns: c_int,
            gate_rows: c_int,
            up_rows: c_int,
            selected_ids: *const c_void,
            selected_count: c_int,
            input_q8_1: *const c_void,
            gate_bias: *const c_void,
            up_bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_moe_gate_up_swiglu(
            weights: *const c_void,
            mode: c_int,
            row_stride: c_int,
            rows_per_expert: c_int,
            columns: c_int,
            gate_rows: c_int,
            up_rows: c_int,
            selected_ids: *const c_void,
            selected_count: c_int,
            input: *const c_void,
            gate_bias: *const c_void,
            up_bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_moe_gate_up_swiglu_q8_1_selected4_quantized(
            weights: *const c_void,
            mode: c_int,
            row_stride: c_int,
            rows_per_expert: c_int,
            columns: c_int,
            gate_rows: c_int,
            up_rows: c_int,
            selected_ids: *const c_void,
            selected_count: c_int,
            input_q8_1: *const c_void,
            gate_bias: *const c_void,
            up_bias: *const c_void,
            output_q8_1: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_moe_down_aggregate(
            weights: *const c_void,
            mode: c_int,
            row_stride: c_int,
            rows: c_int,
            columns: c_int,
            selected_ids: *const c_void,
            selected_weights: *const c_void,
            selected_count: c_int,
            activated: *const c_void,
            bias: *const c_void,
            residual: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_moe_down_aggregate_q8_1(
            weights: *const c_void,
            mode: c_int,
            row_stride: c_int,
            rows: c_int,
            columns: c_int,
            selected_ids: *const c_void,
            selected_weights: *const c_void,
            selected_count: c_int,
            activated_q8_1: *const c_void,
            bias: *const c_void,
            residual: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_moe_down_aggregate_q8_1_f32(
            weights: *const c_void,
            mode: c_int,
            row_stride: c_int,
            rows: c_int,
            columns: c_int,
            selected_ids: *const c_void,
            selected_weights: *const c_void,
            selected_count: c_int,
            activated: *const c_void,
            bias: *const c_void,
            residual: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_moe_down_project_q8_1_selected4(
            weights: *const c_void,
            mode: c_int,
            row_stride: c_int,
            rows: c_int,
            columns: c_int,
            selected_ids: *const c_void,
            selected_count: c_int,
            activated_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_accumulate_selected4(
            input: *const c_void,
            selected_weights: *const c_void,
            selected_count: c_int,
            rows: c_int,
            residual: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
    }

    pub(super) struct ConfiguredBackend {
        runtime: Arc<CudaRuntime>,
        stream: CudaStream,
        allocator_pool: Arc<Mutex<CudaAllocatorPool>>,
    }

    #[derive(Clone)]
    pub(super) struct PlatformBuffer {
        inner: Arc<PlatformBufferInner>,
    }

    #[derive(Clone)]
    pub(super) struct PlatformHostBuffer {
        inner: Arc<PlatformHostBufferInner>,
    }

    pub(super) struct PlatformGraphExec {
        runtime: Arc<CudaRuntime>,
        graph: CudaGraph,
        instance: CudaGraphExec,
        stream: CudaStream,
    }

    pub(super) struct PlatformSubmission {
        runtime: Arc<CudaRuntime>,
        stream: CudaStream,
        status: CudaCommandStatus,
        owned_stream: bool,
        capturing: bool,
    }

    struct PoolReturnToken {
        allocator_pool: Arc<Mutex<CudaAllocatorPool>>,
        cache_key: String,
        byte_len: usize,
    }

    struct PlatformBufferInner {
        runtime: Arc<CudaRuntime>,
        device_ptr: *mut c_void,
        allocation_owner: Option<Arc<PlatformBufferInner>>,
        pool_return: Option<PoolReturnToken>,
    }

    struct PlatformHostBufferInner {
        runtime: Arc<CudaRuntime>,
        host_ptr: *mut c_void,
        byte_len: usize,
    }

    struct CudaRuntime {
        ordinal: u16,
        _cudart_library: Library,
        _cublas_library: Library,
        cuda_get_error_string: CudaGetErrorString,
        cuda_set_device: CudaSetDevice,
        cuda_malloc: CudaMalloc,
        cuda_malloc_host: CudaMallocHost,
        cuda_mem_get_info: CudaMemGetInfo,
        cuda_free: CudaFree,
        cuda_free_host: CudaFreeHost,
        cuda_memcpy: CudaMemcpy,
        cuda_memcpy_async: CudaMemcpyAsync,
        cuda_memset_async: CudaMemsetAsync,
        cuda_stream_create: CudaStreamCreate,
        cuda_stream_destroy: CudaStreamDestroy,
        cuda_stream_synchronize: CudaStreamSynchronize,
        cuda_stream_begin_capture: CudaStreamBeginCapture,
        cuda_stream_end_capture: CudaStreamEndCapture,
        cuda_graph_instantiate: CudaGraphInstantiate,
        cuda_graph_launch: CudaGraphLaunch,
        cuda_graph_exec_destroy: CudaGraphExecDestroy,
        cuda_graph_destroy: CudaGraphDestroy,
        cuda_event_create: CudaEventCreate,
        cuda_event_destroy: CudaEventDestroy,
        cuda_event_record: CudaEventRecord,
        cuda_event_synchronize: CudaEventSynchronize,
        cuda_event_elapsed_time: CudaEventElapsedTime,
        cublas_handle: CublasHandle,
        cublas_create: CublasCreate,
        cublas_destroy: CublasDestroy,
        cublas_set_stream: CublasSetStream,
        cublas_sgemm: CublasSgemm,
        cublas_sgeam: CublasSgeam,
        cublas_gemm_ex: CublasGemmEx,
        cublas_gemm_strided_batched_ex: CublasGemmStridedBatchedEx,
        cublaslt: Option<Mutex<CublasLtContext>>,
    }

    impl ConfiguredBackend {
        pub(super) fn allocator_pool_report(&self) -> AllocatorPoolReport {
            self.allocator_pool
                .lock()
                .expect("cuda allocator pool mutex should not be poisoned")
                .report()
        }

        pub(super) fn allocator_pool_telemetry(&self) -> CudaAllocatorPoolTelemetry {
            self.allocator_pool
                .lock()
                .expect("cuda allocator pool mutex should not be poisoned")
                .telemetry()
        }

        pub(super) fn device_memory_info(
            &self,
        ) -> Result<super::CudaDeviceMemoryInfo, RuntimeError> {
            self.runtime.device_memory_info()
        }

        pub(super) fn cuda_gemm_tuning_report(&self) -> super::CudaGemmTuningReport {
            self.runtime.cuda_gemm_tuning_report()
        }

        pub(super) fn register_cublaslt_weight_scope(
            &self,
            allocation_identity: usize,
            scope: super::CudaGemmTuningScope,
        ) {
            self.runtime
                .register_cublaslt_weight_scope(allocation_identity, scope);
        }

        pub(super) fn allocate(
            &self,
            spec: &TensorSpec,
            byte_len: usize,
        ) -> Result<PlatformBuffer, RuntimeError> {
            self.runtime.set_device()?;
            let allocation_len = byte_len.max(1);
            let cache_key = allocator_pool_cache_key(spec);
            let device_ptr = if let Some(cached_ptr) = self
                .allocator_pool
                .lock()
                .expect("cuda allocator pool mutex should not be poisoned")
                .checkout(cache_key.as_str(), allocation_len)
            {
                cached_ptr as *mut c_void
            } else {
                let mut device_ptr = std::ptr::null_mut();
                self.runtime.check(
                    unsafe { (self.runtime.cuda_malloc)(&mut device_ptr, allocation_len) },
                    "cudaMalloc",
                )?;
                self.allocator_pool
                    .lock()
                    .expect("cuda allocator pool mutex should not be poisoned")
                    .record_cold_allocation(allocation_len);
                device_ptr
            };
            Ok(PlatformBuffer {
                inner: Arc::new(PlatformBufferInner {
                    runtime: Arc::clone(&self.runtime),
                    device_ptr,
                    allocation_owner: None,
                    pool_return: Some(PoolReturnToken {
                        allocator_pool: Arc::clone(&self.allocator_pool),
                        cache_key,
                        byte_len: allocation_len,
                    }),
                }),
            })
        }

        pub(super) fn allocate_host(
            &self,
            byte_len: usize,
        ) -> Result<PlatformHostBuffer, RuntimeError> {
            self.runtime.set_device()?;
            let mut host_ptr = std::ptr::null_mut();
            let allocation_len = byte_len.max(1);
            self.runtime.check(
                unsafe { (self.runtime.cuda_malloc_host)(&mut host_ptr, allocation_len) },
                "cudaMallocHost",
            )?;
            Ok(PlatformHostBuffer {
                inner: Arc::new(PlatformHostBufferInner {
                    runtime: Arc::clone(&self.runtime),
                    host_ptr,
                    byte_len: allocation_len,
                }),
            })
        }

        pub(super) fn begin_submission(&self) -> Result<PlatformSubmission, RuntimeError> {
            self.runtime.set_device()?;
            Ok(PlatformSubmission {
                runtime: Arc::clone(&self.runtime),
                stream: self.stream,
                status: CudaCommandStatus::Submitted,
                owned_stream: false,
                capturing: false,
            })
        }

        pub(super) fn begin_capture_submission(&self) -> Result<PlatformSubmission, RuntimeError> {
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (self.runtime.cuda_stream_begin_capture)(
                        self.stream,
                        CUDA_STREAM_CAPTURE_MODE_RELAXED,
                    )
                },
                "cudaStreamBeginCapture",
            )?;
            Ok(PlatformSubmission {
                runtime: Arc::clone(&self.runtime),
                stream: self.stream,
                status: CudaCommandStatus::Submitted,
                owned_stream: false,
                capturing: true,
            })
        }

        pub(super) fn top_k_f32_one_row_radix_sort_temp_storage_bytes(
            &self,
            column_count: usize,
        ) -> Result<usize, RuntimeError> {
            let cols = c_int::try_from(column_count)
                .map_err(|_| RuntimeError::Backend(String::from("cuda top_k cols exceed c_int")))?;
            self.runtime.set_device()?;
            let mut temp_storage_bytes = 0usize;
            self.runtime.check(
                unsafe {
                    psionic_cuda_top_k_f32_one_row_radix_sort_temp_storage_bytes(
                        cols,
                        &mut temp_storage_bytes,
                    )
                },
                "psionic_cuda_top_k_f32_one_row_radix_sort_temp_storage_bytes",
            )?;
            Ok(temp_storage_bytes)
        }
    }

    pub(super) fn quantized_kernels_compiled() -> bool {
        unsafe { psionic_cuda_quantized_kernels_compiled() != 0 }
    }

    impl PlatformBuffer {
        pub(super) fn allocation_identity(&self) -> usize {
            self.inner.device_ptr as usize
        }

        pub(super) fn sub_buffer(&self, byte_offset: usize) -> Result<Self, RuntimeError> {
            self.inner.runtime.set_device()?;
            Ok(Self {
                inner: Arc::new(PlatformBufferInner {
                    runtime: Arc::clone(&self.inner.runtime),
                    device_ptr: unsafe { self.inner.device_ptr.cast::<u8>().add(byte_offset) }
                        .cast(),
                    allocation_owner: Some(Arc::clone(&self.inner)),
                    pool_return: None,
                }),
            })
        }

        pub(super) fn write_bytes(&self, bytes: &[u8]) -> Result<(), RuntimeError> {
            if bytes.is_empty() {
                return Ok(());
            }
            self.inner.runtime.set_device()?;
            self.inner.runtime.check(
                unsafe {
                    (self.inner.runtime.cuda_memcpy)(
                        self.inner.device_ptr,
                        bytes.as_ptr().cast(),
                        bytes.len(),
                        CUDA_MEMCPY_HOST_TO_DEVICE,
                    )
                },
                "cudaMemcpy host_to_device",
            )
        }

        pub(super) fn write_bytes_at_offset(
            &self,
            byte_offset: usize,
            bytes: &[u8],
        ) -> Result<(), RuntimeError> {
            if bytes.is_empty() {
                return Ok(());
            }
            self.inner.runtime.set_device()?;
            self.inner.runtime.check(
                unsafe {
                    (self.inner.runtime.cuda_memcpy)(
                        self.inner.device_ptr.cast::<u8>().add(byte_offset).cast(),
                        bytes.as_ptr().cast(),
                        bytes.len(),
                        CUDA_MEMCPY_HOST_TO_DEVICE,
                    )
                },
                "cudaMemcpy host_to_device_region",
            )
        }

        pub(super) fn read_bytes(&self, byte_len: usize) -> Result<Vec<u8>, RuntimeError> {
            if byte_len == 0 {
                return Ok(Vec::new());
            }
            self.inner.runtime.set_device()?;
            let mut bytes = vec![0u8; byte_len];
            self.inner.runtime.check(
                unsafe {
                    (self.inner.runtime.cuda_memcpy)(
                        bytes.as_mut_ptr().cast(),
                        self.inner.device_ptr,
                        byte_len,
                        CUDA_MEMCPY_DEVICE_TO_HOST,
                    )
                },
                "cudaMemcpy device_to_host",
            )?;
            Ok(bytes)
        }

        pub(super) fn read_bytes_at_offset(
            &self,
            byte_offset: usize,
            byte_len: usize,
        ) -> Result<Vec<u8>, RuntimeError> {
            if byte_len == 0 {
                return Ok(Vec::new());
            }
            self.inner.runtime.set_device()?;
            let mut bytes = vec![0u8; byte_len];
            self.inner.runtime.check(
                unsafe {
                    (self.inner.runtime.cuda_memcpy)(
                        bytes.as_mut_ptr().cast(),
                        self.inner.device_ptr.cast::<u8>().add(byte_offset).cast(),
                        byte_len,
                        CUDA_MEMCPY_DEVICE_TO_HOST,
                    )
                },
                "cudaMemcpy device_to_host_region",
            )?;
            Ok(bytes)
        }
    }

    impl PlatformHostBuffer {
        pub(super) fn write_bytes(&self, bytes: &[u8]) -> Result<(), RuntimeError> {
            if bytes.len() != self.inner.byte_len {
                return Err(RuntimeError::Backend(format!(
                    "cuda host buffer write length mismatch: expected {}, actual {}",
                    self.inner.byte_len,
                    bytes.len()
                )));
            }
            if bytes.is_empty() {
                return Ok(());
            }
            unsafe {
                std::ptr::copy_nonoverlapping(
                    bytes.as_ptr(),
                    self.inner.host_ptr.cast::<u8>(),
                    bytes.len(),
                );
            }
            Ok(())
        }

        pub(super) fn read_bytes(&self, byte_len: usize) -> Result<Vec<u8>, RuntimeError> {
            if byte_len != self.inner.byte_len {
                return Err(RuntimeError::Backend(format!(
                    "cuda host buffer read length mismatch: expected {}, actual {}",
                    self.inner.byte_len, byte_len
                )));
            }
            if byte_len == 0 {
                return Ok(Vec::new());
            }
            let mut bytes = vec![0_u8; byte_len];
            unsafe {
                std::ptr::copy_nonoverlapping(
                    self.inner.host_ptr.cast::<u8>(),
                    bytes.as_mut_ptr(),
                    byte_len,
                );
            }
            Ok(bytes)
        }

        pub(super) fn read_bytes_prefix_into(&self, output: &mut [u8]) -> Result<(), RuntimeError> {
            if output.len() > self.inner.byte_len {
                return Err(RuntimeError::Backend(format!(
                    "cuda host buffer prefix read length exceeds allocation: requested {}, allocation {}",
                    output.len(),
                    self.inner.byte_len
                )));
            }
            if output.is_empty() {
                return Ok(());
            }
            unsafe {
                std::ptr::copy_nonoverlapping(
                    self.inner.host_ptr.cast::<u8>(),
                    output.as_mut_ptr(),
                    output.len(),
                );
            }
            Ok(())
        }
    }

    impl PlatformSubmission {
        pub(super) fn allocate(&self, byte_len: usize) -> Result<PlatformBuffer, RuntimeError> {
            self.runtime.set_device()?;
            let mut device_ptr = std::ptr::null_mut();
            let allocation_len = byte_len.max(1);
            self.runtime.check(
                unsafe { (self.runtime.cuda_malloc)(&mut device_ptr, allocation_len) },
                "cudaMalloc",
            )?;
            Ok(PlatformBuffer {
                inner: Arc::new(PlatformBufferInner {
                    runtime: Arc::clone(&self.runtime),
                    device_ptr,
                    allocation_owner: None,
                    pool_return: None,
                }),
            })
        }

        pub(super) fn fill_buffer(
            &mut self,
            buffer: &PlatformBuffer,
            byte_len: usize,
            value: u8,
        ) -> Result<(), RuntimeError> {
            if byte_len == 0 {
                return Ok(());
            }
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (self.runtime.cuda_memset_async)(
                        buffer.inner.device_ptr,
                        i32::from(value),
                        byte_len,
                        self.stream,
                    )
                },
                "cudaMemsetAsync",
            )
        }

        pub(super) fn copy_buffer(
            &mut self,
            source: &PlatformBuffer,
            destination: &PlatformBuffer,
            byte_len: usize,
        ) -> Result<(), RuntimeError> {
            if byte_len == 0 {
                return Ok(());
            }
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (self.runtime.cuda_memcpy_async)(
                        destination.inner.device_ptr,
                        source.inner.device_ptr,
                        byte_len,
                        CUDA_MEMCPY_DEVICE_TO_DEVICE,
                        self.stream,
                    )
                },
                "cudaMemcpyAsync device_to_device",
            )
        }

        pub(super) fn copy_host_to_device(
            &mut self,
            source: &PlatformHostBuffer,
            destination: &PlatformBuffer,
            byte_len: usize,
        ) -> Result<(), RuntimeError> {
            if byte_len == 0 {
                return Ok(());
            }
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (self.runtime.cuda_memcpy_async)(
                        destination.inner.device_ptr,
                        source.inner.host_ptr,
                        byte_len,
                        CUDA_MEMCPY_HOST_TO_DEVICE,
                        self.stream,
                    )
                },
                "cudaMemcpyAsync host_to_device",
            )
        }

        pub(super) fn copy_device_to_host(
            &mut self,
            source: &PlatformBuffer,
            destination: &PlatformHostBuffer,
            byte_len: usize,
        ) -> Result<(), RuntimeError> {
            if byte_len == 0 {
                return Ok(());
            }
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (self.runtime.cuda_memcpy_async)(
                        destination.inner.host_ptr,
                        source.inner.device_ptr,
                        byte_len,
                        CUDA_MEMCPY_DEVICE_TO_HOST,
                        self.stream,
                    )
                },
                "cudaMemcpyAsync device_to_host",
            )
        }

        pub(super) fn copy_buffer_region(
            &mut self,
            source: &PlatformBuffer,
            source_byte_offset: usize,
            destination: &PlatformBuffer,
            destination_byte_offset: usize,
            byte_len: usize,
        ) -> Result<(), RuntimeError> {
            if byte_len == 0 {
                return Ok(());
            }
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (self.runtime.cuda_memcpy_async)(
                        destination
                            .inner
                            .device_ptr
                            .cast::<u8>()
                            .add(destination_byte_offset)
                            .cast(),
                        source
                            .inner
                            .device_ptr
                            .cast::<u8>()
                            .add(source_byte_offset)
                            .cast(),
                        byte_len,
                        CUDA_MEMCPY_DEVICE_TO_DEVICE,
                        self.stream,
                    )
                },
                "cudaMemcpyAsync device_to_device_region",
            )
        }

        pub(super) fn encode_add(
            &mut self,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            if element_count == 0 {
                return Ok(());
            }
            let m = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda add element count exceeds cublas limits"))
            })?;
            let alpha = 1.0_f32;
            let beta = 1.0_f32;
            self.runtime.bind_stream(self.stream)?;
            self.runtime.check_cublas(
                unsafe {
                    (self.runtime.cublas_sgeam)(
                        self.runtime.cublas_handle,
                        CUBLAS_OP_N,
                        CUBLAS_OP_N,
                        m,
                        1,
                        &alpha,
                        left.inner.device_ptr.cast(),
                        m,
                        &beta,
                        right.inner.device_ptr.cast(),
                        m,
                        output.inner.device_ptr.cast(),
                        m,
                    )
                },
                "cublasSgeam",
            )
        }

        pub(super) fn encode_mul(
            &mut self,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            if element_count == 0 {
                return Ok(());
            }
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda mul element count exceeds c_int"))
            })?;
            self.runtime.bind_stream(self.stream)?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_mul_f32(
                        left.inner.device_ptr.cast(),
                        right.inner.device_ptr.cast(),
                        element_count,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_mul_f32",
            )
        }

        pub(super) fn encode_matmul(
            &mut self,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
            rows: usize,
            inner: usize,
            cols: usize,
        ) -> Result<(), RuntimeError> {
            if rows == 0 || inner == 0 || cols == 0 {
                return Ok(());
            }
            let m = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda matmul column count exceeds cublas limits",
                ))
            })?;
            let n = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda matmul row count exceeds cublas limits"))
            })?;
            let k = c_int::try_from(inner).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda matmul inner dimension exceeds cublas limits",
                ))
            })?;
            let alpha = 1.0_f32;
            let beta = 0.0_f32;
            self.runtime.bind_stream(self.stream)?;
            self.runtime.check_cublas(
                unsafe {
                    (self.runtime.cublas_sgemm)(
                        self.runtime.cublas_handle,
                        CUBLAS_OP_N,
                        CUBLAS_OP_N,
                        m,
                        n,
                        k,
                        &alpha,
                        right.inner.device_ptr.cast(),
                        m,
                        left.inner.device_ptr.cast(),
                        k,
                        &beta,
                        output.inner.device_ptr.cast(),
                        m,
                    )
                },
                "cublasSgemm_v2",
            )
        }

        pub(super) fn encode_matmul_f16_to_f32(
            &mut self,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
            rows: usize,
            inner: usize,
            cols: usize,
        ) -> Result<(), RuntimeError> {
            if rows == 0 || inner == 0 || cols == 0 {
                return Ok(());
            }
            let m = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda mixed matmul column count exceeds cublas limits",
                ))
            })?;
            let n = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda mixed matmul row count exceeds cublas limits",
                ))
            })?;
            let k = c_int::try_from(inner).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda mixed matmul inner dimension exceeds cublas limits",
                ))
            })?;
            let alpha = 1.0_f32;
            let beta = 0.0_f32;
            self.runtime.bind_stream(self.stream)?;
            if self.runtime.maybe_execute_cublaslt_f16_to_f32(
                self.stream,
                self.capturing,
                left,
                right,
                output,
                rows,
                inner,
                cols,
            )? {
                return Ok(());
            }
            self.runtime.check_cublas(
                unsafe {
                    (self.runtime.cublas_gemm_ex)(
                        self.runtime.cublas_handle,
                        CUBLAS_OP_N,
                        CUBLAS_OP_N,
                        m,
                        n,
                        k,
                        (&alpha as *const f32).cast(),
                        right.inner.device_ptr,
                        CUDA_R_16F,
                        m,
                        left.inner.device_ptr,
                        CUDA_R_16F,
                        k,
                        (&beta as *const f32).cast(),
                        output.inner.device_ptr,
                        CUDA_R_32F,
                        m,
                        CUBLAS_COMPUTE_32F_FAST_16F,
                        CUBLAS_GEMM_DEFAULT_TENSOR_OP,
                    )
                },
                "cublasGemmEx",
            )
        }

        pub(super) fn encode_matmul_bf16_to_f32(
            &mut self,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
            rows: usize,
            inner: usize,
            cols: usize,
        ) -> Result<(), RuntimeError> {
            if rows == 0 || inner == 0 || cols == 0 {
                return Ok(());
            }
            let m = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda bf16 matmul column count exceeds cublas limits",
                ))
            })?;
            let n = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda bf16 matmul row count exceeds cublas limits",
                ))
            })?;
            let k = c_int::try_from(inner).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda bf16 matmul inner dimension exceeds cublas limits",
                ))
            })?;
            let alpha = 1.0_f32;
            let beta = 0.0_f32;
            self.runtime.bind_stream(self.stream)?;
            self.runtime.check_cublas(
                unsafe {
                    (self.runtime.cublas_gemm_ex)(
                        self.runtime.cublas_handle,
                        CUBLAS_OP_N,
                        CUBLAS_OP_N,
                        m,
                        n,
                        k,
                        (&alpha as *const f32).cast(),
                        right.inner.device_ptr,
                        CUDA_R_16BF,
                        m,
                        left.inner.device_ptr,
                        CUDA_R_16BF,
                        k,
                        (&beta as *const f32).cast(),
                        output.inner.device_ptr,
                        CUDA_R_32F,
                        m,
                        CUBLAS_COMPUTE_32F,
                        CUBLAS_GEMM_DEFAULT_TENSOR_OP,
                    )
                },
                "cublasGemmEx",
            )
        }

        pub(super) fn encode_matmul_f32_bf16_to_f32(
            &mut self,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
            rows: usize,
            inner: usize,
            cols: usize,
        ) -> Result<(), RuntimeError> {
            if rows == 0 || inner == 0 || cols == 0 {
                return Ok(());
            }
            let m = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda mixed f32xbf16 matmul column count exceeds cublas limits",
                ))
            })?;
            let n = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda mixed f32xbf16 matmul row count exceeds cublas limits",
                ))
            })?;
            let k = c_int::try_from(inner).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda mixed f32xbf16 matmul inner dimension exceeds cublas limits",
                ))
            })?;
            let alpha = 1.0_f32;
            let beta = 0.0_f32;
            self.runtime.bind_stream(self.stream)?;
            self.runtime.check_cublas(
                unsafe {
                    (self.runtime.cublas_gemm_ex)(
                        self.runtime.cublas_handle,
                        CUBLAS_OP_N,
                        CUBLAS_OP_N,
                        m,
                        n,
                        k,
                        (&alpha as *const f32).cast(),
                        right.inner.device_ptr,
                        CUDA_R_16BF,
                        m,
                        left.inner.device_ptr,
                        CUDA_R_32F,
                        k,
                        (&beta as *const f32).cast(),
                        output.inner.device_ptr,
                        CUDA_R_32F,
                        m,
                        CUBLAS_COMPUTE_32F,
                        CUBLAS_GEMM_DEFAULT_TENSOR_OP,
                    )
                },
                "cublasGemmEx",
            )
        }

        pub(super) fn encode_matmul_bf16_strided_batched_to_f32_with_strides(
            &mut self,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
            rows: usize,
            inner: usize,
            cols: usize,
            batch_count: usize,
            left_stride_elements: usize,
            right_stride_elements: usize,
            output_stride_elements: usize,
            transpose_left: bool,
            transpose_right: bool,
        ) -> Result<(), RuntimeError> {
            if rows == 0 || inner == 0 || cols == 0 || batch_count == 0 {
                return Ok(());
            }
            let m = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda bf16 strided batched matmul column count exceeds cublas limits",
                ))
            })?;
            let n = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda bf16 strided batched matmul row count exceeds cublas limits",
                ))
            })?;
            let k = c_int::try_from(inner).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda bf16 strided batched matmul inner dimension exceeds cublas limits",
                ))
            })?;
            let batch_count = c_int::try_from(batch_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda bf16 strided batched matmul batch count exceeds cublas limits",
                ))
            })?;
            let lda = if transpose_right { k } else { m };
            let ldb = if transpose_left { n } else { k };
            let stride_a = c_longlong::try_from(right_stride_elements).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda bf16 strided batched matmul rhs stride exceeds cublas limits",
                ))
            })?;
            let stride_b = c_longlong::try_from(left_stride_elements).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda bf16 strided batched matmul lhs stride exceeds cublas limits",
                ))
            })?;
            let stride_c = c_longlong::try_from(output_stride_elements).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda bf16 strided batched matmul output stride exceeds cublas limits",
                ))
            })?;
            let alpha = 1.0_f32;
            let beta = 0.0_f32;
            self.runtime.bind_stream(self.stream)?;
            self.runtime.check_cublas(
                unsafe {
                    (self.runtime.cublas_gemm_strided_batched_ex)(
                        self.runtime.cublas_handle,
                        cublas_row_major_op(transpose_right),
                        cublas_row_major_op(transpose_left),
                        m,
                        n,
                        k,
                        (&alpha as *const f32).cast(),
                        right.inner.device_ptr,
                        CUDA_R_16BF,
                        lda,
                        stride_a,
                        left.inner.device_ptr,
                        CUDA_R_16BF,
                        ldb,
                        stride_b,
                        (&beta as *const f32).cast(),
                        output.inner.device_ptr,
                        CUDA_R_32F,
                        m,
                        stride_c,
                        batch_count,
                        CUBLAS_COMPUTE_32F,
                        CUBLAS_GEMM_DEFAULT_TENSOR_OP,
                    )
                },
                "cublasGemmStridedBatchedEx",
            )
        }

        pub(super) fn encode_cast_f32_to_f16(
            &mut self,
            input: &PlatformBuffer,
            output: &PlatformBuffer,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda f32->f16 cast length exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_cast_f32_to_f16(
                        input.inner.device_ptr.cast(),
                        element_count,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_cast_f32_to_f16",
            )
        }

        pub(super) fn encode_cast_f32_to_bf16(
            &mut self,
            input: &PlatformBuffer,
            output: &PlatformBuffer,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda f32->bf16 cast length exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_cast_f32_to_bf16(
                        input.inner.device_ptr.cast(),
                        element_count,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_cast_f32_to_bf16",
            )
        }

        pub(super) fn encode_cast_bf16_to_f32(
            &mut self,
            input: &PlatformBuffer,
            output: &PlatformBuffer,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda bf16->f32 cast length exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_cast_bf16_to_f32(
                        input.inner.device_ptr.cast(),
                        element_count,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_cast_bf16_to_f32",
            )
        }

        pub(super) fn encode_quantized_matvec(
            &mut self,
            weights: &PlatformBuffer,
            byte_offset: usize,
            mode: QuantizationMode,
            rows: usize,
            cols: usize,
            input: &PlatformBuffer,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            if !quantized_kernels_compiled() {
                return Err(RuntimeError::Backend(String::from(
                    "cuda quantized text-generation kernels are not available in this build",
                )));
            }
            let Some((elements_per_block, bytes_per_block)) = mode.ggml_block_spec() else {
                return Err(RuntimeError::Backend(format!(
                    "cuda quantized matvec does not support mode {mode:?}"
                )));
            };
            let row_stride = (cols / elements_per_block) * bytes_per_block;
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda quantized matvec rows exceed c_int"))
            })?;
            let cols = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from("cuda quantized matvec cols exceed c_int"))
            })?;
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized matvec row stride exceeds c_int",
                ))
            })?;
            let kernel: QuantizedMatvecKernel = match mode {
                QuantizationMode::GgmlQ8_0 => psionic_cuda_q8_0_matvec,
                QuantizationMode::GgmlQ3K => psionic_cuda_q3_k_matvec,
                QuantizationMode::GgmlQ5K => psionic_cuda_q5_k_matvec,
                QuantizationMode::GgmlQ4K => psionic_cuda_q4_k_matvec,
                QuantizationMode::GgmlQ6K => psionic_cuda_q6_k_matvec,
                QuantizationMode::GgmlIq3S => psionic_cuda_iq3_s_matvec,
                QuantizationMode::GgmlIq4Xs => psionic_cuda_iq4_xs_matvec,
                QuantizationMode::GgmlMxfp4 => psionic_cuda_mxfp4_matvec,
                _ => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda quantized matvec does not support mode {mode:?}"
                    )));
                }
            };
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    kernel(
                        weights
                            .inner
                            .device_ptr
                            .cast::<u8>()
                            .add(byte_offset)
                            .cast(),
                        rows,
                        cols,
                        row_stride,
                        input.inner.device_ptr,
                        output.inner.device_ptr,
                        self.stream,
                    )
                },
                "psionic_cuda_quantized_matvec",
            )
        }

        pub(super) fn encode_quantize_f32_to_q8_1(
            &mut self,
            input: &PlatformBuffer,
            rows: usize,
            cols: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda q8_1 quantize rows exceed c_int"))
            })?;
            let cols = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from("cuda q8_1 quantize cols exceed c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (psionic_cuda_quantize_q8_1 as QuantizeQ81Kernel)(
                        input.inner.device_ptr.cast(),
                        rows,
                        cols,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_quantize_q8_1",
            )
        }

        pub(super) fn encode_dequantize_row_to_f32(
            &mut self,
            weights: &PlatformBuffer,
            mode: QuantizationMode,
            rows: usize,
            row_stride: usize,
            cols: usize,
            decode_params: &PlatformBuffer,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            if !quantized_kernels_compiled() {
                return Err(RuntimeError::Backend(String::from(
                    "cuda quantized text-generation kernels are not available in this build",
                )));
            }
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda dequantize row rows exceed c_int"))
            })?;
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from("cuda dequantize row stride exceeds c_int"))
            })?;
            let cols = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from("cuda dequantize row cols exceed c_int"))
            })?;
            let kernel: DequantizeRowToF32Kernel = match mode {
                QuantizationMode::GgmlQ8_0 => psionic_cuda_q8_0_dequantize_row_to_f32,
                QuantizationMode::GgmlQ3K => psionic_cuda_q3_k_dequantize_row_to_f32,
                QuantizationMode::GgmlQ5K => psionic_cuda_q5_k_dequantize_row_to_f32,
                QuantizationMode::GgmlIq3S => psionic_cuda_iq3_s_dequantize_row_to_f32,
                QuantizationMode::GgmlIq4Xs => psionic_cuda_iq4_xs_dequantize_row_to_f32,
                QuantizationMode::GgmlMxfp4 => psionic_cuda_mxfp4_dequantize_row_to_f32,
                _ => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda dequantize row does not support mode {mode:?}"
                    )));
                }
            };
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    kernel(
                        weights.inner.device_ptr.cast(),
                        rows,
                        cols,
                        row_stride,
                        decode_params.inner.device_ptr.cast(),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_dequantize_row_to_f32",
            )
        }

        pub(super) fn encode_gather_f16_row_to_f32(
            &mut self,
            input: &PlatformBuffer,
            rows: usize,
            cols: usize,
            decode_params: &PlatformBuffer,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda gather f16 row rows exceed c_int"))
            })?;
            let cols = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from("cuda gather f16 row cols exceed c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (psionic_cuda_gather_f16_row_to_f32 as GatherF16RowToF32Kernel)(
                        input.inner.device_ptr.cast(),
                        rows,
                        cols,
                        decode_params.inner.device_ptr.cast(),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_gather_f16_row_to_f32",
            )
        }

        pub(super) fn encode_gather_f32_by_indices(
            &mut self,
            input: &PlatformBuffer,
            input_len: usize,
            indices: &PlatformBuffer,
            index_count: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let input_len = c_int::try_from(input_len).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda sparse gather input length exceeds c_int",
                ))
            })?;
            let index_count = c_int::try_from(index_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda sparse gather index count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (psionic_cuda_gather_f32_by_indices as GatherF32ByIndicesKernel)(
                        input.inner.device_ptr.cast(),
                        input_len,
                        indices.inner.device_ptr.cast(),
                        index_count,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_gather_f32_by_indices",
            )
        }

        pub(super) fn encode_quantized_matvec_q8_1(
            &mut self,
            weights: &PlatformBuffer,
            byte_offset: usize,
            mode: QuantizationMode,
            rows: usize,
            cols: usize,
            input_q8_1: &PlatformBuffer,
            bias: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            if !quantized_kernels_compiled() {
                return Err(RuntimeError::Backend(String::from(
                    "cuda quantized text-generation kernels are not available in this build",
                )));
            }
            let Some((elements_per_block, bytes_per_block)) = mode.ggml_block_spec() else {
                return Err(RuntimeError::Backend(format!(
                    "cuda quantized matvec does not support mode {mode:?}"
                )));
            };
            let row_stride = (cols / elements_per_block) * bytes_per_block;
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda quantized matvec rows exceed c_int"))
            })?;
            let cols = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from("cuda quantized matvec cols exceed c_int"))
            })?;
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized matvec row stride exceeds c_int",
                ))
            })?;
            let kernel: QuantizedMatvecQ81Kernel = match mode {
                QuantizationMode::GgmlQ8_0 => psionic_cuda_q8_0_matvec_q8_1,
                QuantizationMode::GgmlQ3K => psionic_cuda_q3_k_matvec_q8_1,
                QuantizationMode::GgmlQ5K => psionic_cuda_q5_k_matvec_q8_1,
                QuantizationMode::GgmlQ4K => psionic_cuda_q4_k_matvec_q8_1,
                QuantizationMode::GgmlQ6K => psionic_cuda_q6_k_matvec_q8_1,
                QuantizationMode::GgmlIq3S => psionic_cuda_iq3_s_matvec_q8_1,
                QuantizationMode::GgmlIq4Xs => psionic_cuda_iq4_xs_matvec_q8_1,
                QuantizationMode::GgmlMxfp4 => psionic_cuda_mxfp4_matvec_q8_1,
                _ => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda quantized matvec does not support mode {mode:?}"
                    )));
                }
            };
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    kernel(
                        weights
                            .inner
                            .device_ptr
                            .cast::<u8>()
                            .add(byte_offset)
                            .cast(),
                        rows,
                        cols,
                        row_stride,
                        input_q8_1.inner.device_ptr.cast(),
                        bias.map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_quantized_matvec_q8_1",
            )
        }

        pub(super) fn encode_quantized_matvec_q8_1_argmax(
            &mut self,
            weights: &PlatformBuffer,
            byte_offset: usize,
            mode: QuantizationMode,
            rows: usize,
            cols: usize,
            input_q8_1: &PlatformBuffer,
            bias: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            if !quantized_kernels_compiled() {
                return Err(RuntimeError::Backend(String::from(
                    "cuda quantized text-generation kernels are not available in this build",
                )));
            }
            let Some((elements_per_block, bytes_per_block)) = mode.ggml_block_spec() else {
                return Err(RuntimeError::Backend(format!(
                    "cuda quantized matvec argmax does not support mode {mode:?}"
                )));
            };
            let row_stride = (cols / elements_per_block) * bytes_per_block;
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized matvec argmax rows exceed c_int",
                ))
            })?;
            let cols = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized matvec argmax cols exceed c_int",
                ))
            })?;
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized matvec argmax row stride exceeds c_int",
                ))
            })?;
            let kernel: QuantizedMatvecQ81ArgmaxKernel = match mode {
                QuantizationMode::GgmlQ8_0 => psionic_cuda_q8_0_matvec_q8_1_argmax,
                QuantizationMode::GgmlQ3K => psionic_cuda_q3_k_matvec_q8_1_argmax,
                QuantizationMode::GgmlQ5K => psionic_cuda_q5_k_matvec_q8_1_argmax,
                QuantizationMode::GgmlQ4K => psionic_cuda_q4_k_matvec_q8_1_argmax,
                QuantizationMode::GgmlQ6K => psionic_cuda_q6_k_matvec_q8_1_argmax,
                QuantizationMode::GgmlIq3S => psionic_cuda_iq3_s_matvec_q8_1_argmax,
                QuantizationMode::GgmlIq4Xs => psionic_cuda_iq4_xs_matvec_q8_1_argmax,
                QuantizationMode::GgmlMxfp4 => psionic_cuda_mxfp4_matvec_q8_1_argmax,
                _ => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda quantized matvec argmax does not support mode {mode:?}"
                    )));
                }
            };
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    kernel(
                        weights
                            .inner
                            .device_ptr
                            .cast::<u8>()
                            .add(byte_offset)
                            .cast(),
                        rows,
                        cols,
                        row_stride,
                        input_q8_1.inner.device_ptr.cast(),
                        bias.map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_quantized_matvec_q8_1_argmax",
            )
        }

        pub(super) fn encode_argmax_f32(
            &mut self,
            input: &PlatformBuffer,
            rows: usize,
            cols: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda argmax rows exceed c_int"))
            })?;
            let cols = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from("cuda argmax cols exceed c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_argmax_f32(
                        input.inner.device_ptr.cast(),
                        rows,
                        cols,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_argmax_f32",
            )
        }

        pub(super) fn encode_top_k_f32(
            &mut self,
            input: &PlatformBuffer,
            rows: usize,
            cols: usize,
            top_k: usize,
            selected_indices: &PlatformBuffer,
            selected_values: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let rows = c_int::try_from(rows)
                .map_err(|_| RuntimeError::Backend(String::from("cuda top_k rows exceed c_int")))?;
            let cols = c_int::try_from(cols)
                .map_err(|_| RuntimeError::Backend(String::from("cuda top_k cols exceed c_int")))?;
            let top_k = c_int::try_from(top_k).map_err(|_| {
                RuntimeError::Backend(String::from("cuda top_k selection width exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_top_k_f32(
                        input.inner.device_ptr.cast(),
                        rows,
                        cols,
                        top_k,
                        selected_indices.inner.device_ptr.cast(),
                        selected_values.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_top_k_f32",
            )
        }

        pub(super) fn encode_top_k_f32_one_row_partitioned(
            &mut self,
            input: &PlatformBuffer,
            cols: usize,
            top_k: usize,
            partial_block_count: usize,
            partial_indices: &PlatformBuffer,
            partial_values: &PlatformBuffer,
            selected_indices: &PlatformBuffer,
            selected_values: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let cols = c_int::try_from(cols)
                .map_err(|_| RuntimeError::Backend(String::from("cuda top_k cols exceed c_int")))?;
            let top_k = c_int::try_from(top_k).map_err(|_| {
                RuntimeError::Backend(String::from("cuda top_k selection width exceeds c_int"))
            })?;
            let partial_block_count = c_int::try_from(partial_block_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda partitioned top_k block count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_top_k_f32_one_row_partitioned(
                        input.inner.device_ptr.cast(),
                        cols,
                        top_k,
                        partial_block_count,
                        partial_indices.inner.device_ptr.cast(),
                        partial_values.inner.device_ptr.cast(),
                        selected_indices.inner.device_ptr.cast(),
                        selected_values.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_top_k_f32_one_row_partitioned",
            )
        }

        pub(super) fn encode_apply_sampling_penalties_f32_sparse(
            &mut self,
            logits: &PlatformBuffer,
            vocab_size: usize,
            token_ids: &PlatformBuffer,
            token_counts: &PlatformBuffer,
            active_token_count: usize,
            repeat_penalty: f32,
            presence_penalty: f32,
            frequency_penalty: f32,
        ) -> Result<(), RuntimeError> {
            let vocab_size = c_int::try_from(vocab_size).map_err(|_| {
                RuntimeError::Backend(String::from("cuda penalty vocab size exceeds c_int"))
            })?;
            let active_token_count = c_int::try_from(active_token_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda penalty active token count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_apply_sampling_penalties_f32_sparse(
                        logits.inner.device_ptr.cast(),
                        vocab_size,
                        token_ids.inner.device_ptr.cast(),
                        token_counts.inner.device_ptr.cast(),
                        active_token_count,
                        repeat_penalty,
                        presence_penalty,
                        frequency_penalty,
                        self.stream,
                    )
                },
                "psionic_cuda_apply_sampling_penalties_f32_sparse",
            )
        }

        pub(super) fn encode_top_k_f32_one_row_radix_sort(
            &mut self,
            input: &PlatformBuffer,
            cols: usize,
            top_k: usize,
            input_indices: &PlatformBuffer,
            sorted_values: &PlatformBuffer,
            sorted_indices: &PlatformBuffer,
            temp_storage: &PlatformBuffer,
            temp_storage_bytes: usize,
            selected_indices: &PlatformBuffer,
            selected_values: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let cols = c_int::try_from(cols)
                .map_err(|_| RuntimeError::Backend(String::from("cuda top_k cols exceed c_int")))?;
            let top_k = c_int::try_from(top_k).map_err(|_| {
                RuntimeError::Backend(String::from("cuda top_k selection width exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_top_k_f32_one_row_radix_sort(
                        input.inner.device_ptr.cast(),
                        cols,
                        top_k,
                        input_indices.inner.device_ptr.cast(),
                        sorted_values.inner.device_ptr.cast(),
                        sorted_indices.inner.device_ptr.cast(),
                        temp_storage.inner.device_ptr.cast(),
                        temp_storage_bytes,
                        selected_indices.inner.device_ptr.cast(),
                        selected_values.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_top_k_f32_one_row_radix_sort",
            )
        }

        pub(super) fn encode_rms_norm(
            &mut self,
            input: &PlatformBuffer,
            weight: &PlatformBuffer,
            output: &PlatformBuffer,
            element_count: usize,
            feature_count: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda rms_norm element count exceeds c_int"))
            })?;
            let feature_count = c_int::try_from(feature_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda rms_norm feature count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_rms_norm(
                        input.inner.device_ptr.cast(),
                        weight.inner.device_ptr.cast(),
                        element_count,
                        feature_count,
                        epsilon,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_rms_norm",
            )
        }

        pub(super) fn encode_rms_norm_region(
            &mut self,
            input: &PlatformBuffer,
            input_offset: usize,
            weight: &PlatformBuffer,
            output: &PlatformBuffer,
            output_offset: usize,
            element_count: usize,
            feature_count: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let input_offset = c_int::try_from(input_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda rms_norm_region input offset exceeds c_int",
                ))
            })?;
            let output_offset = c_int::try_from(output_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda rms_norm_region output offset exceeds c_int",
                ))
            })?;
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda rms_norm_region element count exceeds c_int",
                ))
            })?;
            let feature_count = c_int::try_from(feature_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda rms_norm_region feature count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_rms_norm_region(
                        input.inner.device_ptr.cast(),
                        input_offset,
                        weight.inner.device_ptr.cast(),
                        element_count,
                        feature_count,
                        epsilon,
                        output.inner.device_ptr.cast(),
                        output_offset,
                        self.stream,
                    )
                },
                "psionic_cuda_rms_norm_region",
            )
        }

        pub(super) fn encode_parameter_golf_projection_loss(
            &mut self,
            logits: &PlatformBuffer,
            target_ids: &PlatformBuffer,
            output: &PlatformBuffer,
            row_count: usize,
            vocab_size: usize,
            logit_softcap: f32,
        ) -> Result<(), RuntimeError> {
            let row_count = c_int::try_from(row_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda parameter_golf_projection_loss row count exceeds c_int",
                ))
            })?;
            let vocab_size = c_int::try_from(vocab_size).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda parameter_golf_projection_loss vocab size exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_parameter_golf_projection_loss(
                        logits.inner.device_ptr.cast(),
                        target_ids.inner.device_ptr.cast(),
                        row_count,
                        vocab_size,
                        logit_softcap,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_parameter_golf_projection_loss",
            )
        }

        pub(super) fn encode_parameter_golf_projection_token_losses(
            &mut self,
            logits: &PlatformBuffer,
            target_ids: &PlatformBuffer,
            output: &PlatformBuffer,
            row_count: usize,
            vocab_size: usize,
            logit_softcap: f32,
        ) -> Result<(), RuntimeError> {
            let row_count = c_int::try_from(row_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda parameter_golf_projection_token_losses row count exceeds c_int",
                ))
            })?;
            let vocab_size = c_int::try_from(vocab_size).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda parameter_golf_projection_token_losses vocab size exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_parameter_golf_projection_token_losses(
                        logits.inner.device_ptr.cast(),
                        target_ids.inner.device_ptr.cast(),
                        row_count,
                        vocab_size,
                        logit_softcap,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_parameter_golf_projection_token_losses",
            )
        }

        pub(super) fn encode_parameter_golf_token_embedding_lookup(
            &mut self,
            token_ids: &PlatformBuffer,
            token_embedding: &PlatformBuffer,
            row_count: usize,
            vocab_size: usize,
            width: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let row_count = c_int::try_from(row_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda parameter_golf_token_embedding_lookup row count exceeds c_int",
                ))
            })?;
            let vocab_size = c_int::try_from(vocab_size).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda parameter_golf_token_embedding_lookup vocab size exceeds c_int",
                ))
            })?;
            let width = c_int::try_from(width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda parameter_golf_token_embedding_lookup width exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_parameter_golf_token_embedding_lookup(
                        token_ids.inner.device_ptr.cast(),
                        token_embedding.inner.device_ptr.cast(),
                        row_count,
                        vocab_size,
                        width,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_parameter_golf_token_embedding_lookup",
            )
        }

        pub(super) fn encode_parameter_golf_token_embedding_lookup_bf16_to_f32(
            &mut self,
            token_ids: &PlatformBuffer,
            token_embedding: &PlatformBuffer,
            row_count: usize,
            vocab_size: usize,
            width: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let row_count = c_int::try_from(row_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda parameter_golf_token_embedding_lookup row count exceeds c_int",
                ))
            })?;
            let vocab_size = c_int::try_from(vocab_size).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda parameter_golf_token_embedding_lookup vocab size exceeds c_int",
                ))
            })?;
            let width = c_int::try_from(width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda parameter_golf_token_embedding_lookup width exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_parameter_golf_token_embedding_lookup_bf16_to_f32(
                        token_ids.inner.device_ptr.cast(),
                        token_embedding.inner.device_ptr.cast(),
                        row_count,
                        vocab_size,
                        width,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_parameter_golf_token_embedding_lookup_bf16_to_f32",
            )
        }

        pub(super) fn encode_parameter_golf_token_embedding_lookup_backward(
            &mut self,
            token_ids: &PlatformBuffer,
            grad_output: &PlatformBuffer,
            row_count: usize,
            vocab_size: usize,
            width: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let row_count = c_int::try_from(row_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda parameter_golf_token_embedding_lookup_backward row count exceeds c_int",
                ))
            })?;
            let vocab_size = c_int::try_from(vocab_size).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda parameter_golf_token_embedding_lookup_backward vocab size exceeds c_int",
                ))
            })?;
            let width = c_int::try_from(width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda parameter_golf_token_embedding_lookup_backward width exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_parameter_golf_token_embedding_lookup_backward(
                        token_ids.inner.device_ptr.cast(),
                        grad_output.inner.device_ptr.cast(),
                        row_count,
                        vocab_size,
                        width,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_parameter_golf_token_embedding_lookup_backward",
            )
        }

        pub(super) fn encode_parameter_golf_projection_loss_backward(
            &mut self,
            logits: &PlatformBuffer,
            target_ids: &PlatformBuffer,
            grad_output: &PlatformBuffer,
            output: &PlatformBuffer,
            row_count: usize,
            vocab_size: usize,
            logit_softcap: f32,
        ) -> Result<(), RuntimeError> {
            let row_count = c_int::try_from(row_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda parameter_golf_projection_loss_backward row count exceeds c_int",
                ))
            })?;
            let vocab_size = c_int::try_from(vocab_size).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda parameter_golf_projection_loss_backward vocab size exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_parameter_golf_projection_loss_backward(
                        logits.inner.device_ptr.cast(),
                        target_ids.inner.device_ptr.cast(),
                        grad_output.inner.device_ptr.cast(),
                        row_count,
                        vocab_size,
                        logit_softcap,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_parameter_golf_projection_loss_backward",
            )
        }

        pub(super) fn encode_rms_norm_q8_1(
            &mut self,
            input: &PlatformBuffer,
            weight: &PlatformBuffer,
            output_q8_1: &PlatformBuffer,
            element_count: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda rms_norm_q8_1 element count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_rms_norm_q8_1(
                        input.inner.device_ptr.cast(),
                        weight.inner.device_ptr.cast(),
                        element_count,
                        epsilon,
                        output_q8_1.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_rms_norm_q8_1",
            )
        }

        pub(super) fn encode_rms_norm_input_backward(
            &mut self,
            input: &PlatformBuffer,
            weight: &PlatformBuffer,
            grad_output: &PlatformBuffer,
            output: &PlatformBuffer,
            element_count: usize,
            feature_count: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda rms_norm_input_backward element count exceeds c_int",
                ))
            })?;
            let feature_count = c_int::try_from(feature_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda rms_norm_input_backward feature count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_rms_norm_input_backward(
                        input.inner.device_ptr.cast(),
                        weight.inner.device_ptr.cast(),
                        grad_output.inner.device_ptr.cast(),
                        element_count,
                        feature_count,
                        epsilon,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_rms_norm_input_backward",
            )
        }

        pub(super) fn encode_rms_norm_weight_backward(
            &mut self,
            input: &PlatformBuffer,
            grad_output: &PlatformBuffer,
            output: &PlatformBuffer,
            element_count: usize,
            feature_count: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda rms_norm_weight_backward element count exceeds c_int",
                ))
            })?;
            let feature_count = c_int::try_from(feature_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda rms_norm_weight_backward feature count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_rms_norm_weight_backward(
                        input.inner.device_ptr.cast(),
                        grad_output.inner.device_ptr.cast(),
                        element_count,
                        feature_count,
                        epsilon,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_rms_norm_weight_backward",
            )
        }

        pub(super) fn encode_add_residual_rms_norm(
            &mut self,
            input: &PlatformBuffer,
            residual: &PlatformBuffer,
            input_bias: Option<&PlatformBuffer>,
            weight: &PlatformBuffer,
            summed_output: &PlatformBuffer,
            normalized_output: &PlatformBuffer,
            element_count: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda add_residual_rms_norm element count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_add_residual_rms_norm(
                        input.inner.device_ptr.cast(),
                        residual.inner.device_ptr.cast(),
                        input_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        weight.inner.device_ptr.cast(),
                        element_count,
                        epsilon,
                        summed_output.inner.device_ptr.cast(),
                        normalized_output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_add_residual_rms_norm",
            )
        }

        pub(super) fn encode_add_residual_rms_norm_q8_1(
            &mut self,
            input: &PlatformBuffer,
            residual: &PlatformBuffer,
            input_bias: Option<&PlatformBuffer>,
            weight: &PlatformBuffer,
            summed_output: &PlatformBuffer,
            normalized_output: &PlatformBuffer,
            quantized_output: &PlatformBuffer,
            element_count: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda add_residual_rms_norm_q8_1 element count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_add_residual_rms_norm_q8_1(
                        input.inner.device_ptr.cast(),
                        residual.inner.device_ptr.cast(),
                        input_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        weight.inner.device_ptr.cast(),
                        element_count,
                        epsilon,
                        summed_output.inner.device_ptr.cast(),
                        normalized_output.inner.device_ptr.cast(),
                        quantized_output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_add_residual_rms_norm_q8_1",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_add_residual_rms_norm_q8_1_router_topk(
            &mut self,
            input: &PlatformBuffer,
            residual: &PlatformBuffer,
            input_bias: Option<&PlatformBuffer>,
            weight: &PlatformBuffer,
            summed_output: &PlatformBuffer,
            normalized_output: &PlatformBuffer,
            quantized_output: &PlatformBuffer,
            router_weights: &PlatformBuffer,
            router_bias: Option<&PlatformBuffer>,
            expert_count: usize,
            top_k: usize,
            selected_ids: &PlatformBuffer,
            selected_weights: &PlatformBuffer,
            element_count: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda add_residual_rms_norm_q8_1_router_topk element count exceeds c_int",
                ))
            })?;
            let expert_count = c_int::try_from(expert_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda add_residual_rms_norm_q8_1_router_topk expert count exceeds c_int",
                ))
            })?;
            let top_k = c_int::try_from(top_k).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda add_residual_rms_norm_q8_1_router_topk top-k exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_add_residual_rms_norm_q8_1_router_topk(
                        input.inner.device_ptr.cast(),
                        residual.inner.device_ptr.cast(),
                        input_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        weight.inner.device_ptr.cast(),
                        element_count,
                        epsilon,
                        summed_output.inner.device_ptr.cast(),
                        normalized_output.inner.device_ptr.cast(),
                        quantized_output.inner.device_ptr.cast(),
                        router_weights.inner.device_ptr.cast(),
                        router_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        expert_count,
                        top_k,
                        selected_ids.inner.device_ptr.cast(),
                        selected_weights.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_add_residual_rms_norm_q8_1_router_topk",
            )
        }

        pub(super) fn encode_add_f32_in_place(
            &mut self,
            destination: &PlatformBuffer,
            element_offset: usize,
            rhs: &PlatformBuffer,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            let element_offset = c_int::try_from(element_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda add_f32_in_place element offset exceeds c_int",
                ))
            })?;
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda add_f32_in_place element count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_add_f32_offset_in_place(
                        destination.inner.device_ptr.cast(),
                        element_offset,
                        rhs.inner.device_ptr.cast(),
                        element_count,
                        self.stream,
                    )
                },
                "psionic_cuda_add_f32_offset_in_place",
            )
        }

        pub(super) fn encode_silu_mul_f32(
            &mut self,
            activation_input: &PlatformBuffer,
            activation_offset: usize,
            rhs: &PlatformBuffer,
            rhs_offset: usize,
            element_count: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let activation_offset = c_int::try_from(activation_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda silu_mul activation offset exceeds c_int",
                ))
            })?;
            let rhs_offset = c_int::try_from(rhs_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda silu_mul rhs offset exceeds c_int"))
            })?;
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda silu_mul element count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_silu_mul_f32(
                        activation_input.inner.device_ptr.cast(),
                        activation_offset,
                        rhs.inner.device_ptr.cast(),
                        rhs_offset,
                        element_count,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_silu_mul_f32",
            )
        }

        pub(super) fn encode_silu_mul_q8_1(
            &mut self,
            activation_input: &PlatformBuffer,
            activation_offset: usize,
            rhs: &PlatformBuffer,
            rhs_offset: usize,
            element_count: usize,
            output_q8_1: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            if !quantized_kernels_compiled() {
                return Err(RuntimeError::Backend(String::from(
                    "cuda quantized text-generation kernels are not available in this build",
                )));
            }
            let activation_offset = c_int::try_from(activation_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda silu_mul_q8_1 activation offset exceeds c_int",
                ))
            })?;
            let rhs_offset = c_int::try_from(rhs_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda silu_mul_q8_1 rhs offset exceeds c_int"))
            })?;
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda silu_mul_q8_1 element count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_silu_mul_q8_1(
                        activation_input.inner.device_ptr.cast(),
                        activation_offset,
                        rhs.inner.device_ptr.cast(),
                        rhs_offset,
                        element_count,
                        output_q8_1.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_silu_mul_q8_1",
            )
        }

        pub(super) fn encode_sigmoid_mul_f32(
            &mut self,
            values: &PlatformBuffer,
            values_offset: usize,
            gate: &PlatformBuffer,
            gate_offset: usize,
            element_count: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let values_offset = c_int::try_from(values_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda sigmoid_mul values offset exceeds c_int"))
            })?;
            let gate_offset = c_int::try_from(gate_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda sigmoid_mul gate offset exceeds c_int"))
            })?;
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda sigmoid_mul element count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_sigmoid_mul_f32(
                        values.inner.device_ptr.cast(),
                        values_offset,
                        gate.inner.device_ptr.cast(),
                        gate_offset,
                        element_count,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_sigmoid_mul_f32",
            )
        }

        pub(super) fn encode_sigmoid_mul_q8_1(
            &mut self,
            values: &PlatformBuffer,
            values_offset: usize,
            gate: &PlatformBuffer,
            gate_offset: usize,
            element_count: usize,
            output_q8_1: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            if !quantized_kernels_compiled() {
                return Err(RuntimeError::Backend(String::from(
                    "cuda quantized text-generation kernels are not available in this build",
                )));
            }
            let values_offset = c_int::try_from(values_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda sigmoid_mul_q8_1 values offset exceeds c_int",
                ))
            })?;
            let gate_offset = c_int::try_from(gate_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda sigmoid_mul_q8_1 gate offset exceeds c_int",
                ))
            })?;
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda sigmoid_mul_q8_1 element count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_sigmoid_mul_q8_1(
                        values.inner.device_ptr.cast(),
                        values_offset,
                        gate.inner.device_ptr.cast(),
                        gate_offset,
                        element_count,
                        output_q8_1.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_sigmoid_mul_q8_1",
            )
        }

        pub(super) fn encode_split_interleaved_query_gate_f32(
            &mut self,
            input: &PlatformBuffer,
            head_count: usize,
            head_dim: usize,
            query_output: &PlatformBuffer,
            gate_output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 query/gate split head count exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 query/gate split head dim exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_split_interleaved_query_gate_f32(
                        input.inner.device_ptr.cast(),
                        head_count,
                        head_dim,
                        query_output.inner.device_ptr.cast(),
                        gate_output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_split_interleaved_query_gate_f32",
            )
        }

        pub(super) fn encode_split_interleaved_query_gate_rms_norm_f32(
            &mut self,
            input: &PlatformBuffer,
            head_count: usize,
            head_dim: usize,
            weight: &PlatformBuffer,
            epsilon: f32,
            query_output: &PlatformBuffer,
            gate_output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused query/gate split rms norm head count exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused query/gate split rms norm head dim exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (psionic_cuda_split_interleaved_query_gate_rms_norm_f32
                        as SplitInterleavedQueryGateRmsNormF32Kernel)(
                        input.inner.device_ptr.cast(),
                        head_count,
                        head_dim,
                        weight.inner.device_ptr.cast(),
                        epsilon,
                        query_output.inner.device_ptr.cast(),
                        gate_output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_split_interleaved_query_gate_rms_norm_f32",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_pack_qwen35_key_value_rms_norm_f32(
            &mut self,
            input: &PlatformBuffer,
            key_offset: usize,
            value_offset: usize,
            kv_head_count: usize,
            head_dim: usize,
            weight: &PlatformBuffer,
            epsilon: f32,
            output: &PlatformBuffer,
            output_key_offset: usize,
            output_value_offset: usize,
        ) -> Result<(), RuntimeError> {
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused key/value pack key offset exceeds c_int",
                ))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused key/value pack value offset exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused key/value pack kv head count exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused key/value pack head dim exceeds c_int",
                ))
            })?;
            let output_key_offset = c_int::try_from(output_key_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused key/value pack output key offset exceeds c_int",
                ))
            })?;
            let output_value_offset = c_int::try_from(output_value_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused key/value pack output value offset exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (psionic_cuda_pack_qwen35_key_value_rms_norm_f32
                        as PackQwen35KeyValueRmsNormF32Kernel)(
                        input.inner.device_ptr.cast(),
                        key_offset,
                        value_offset,
                        kv_head_count,
                        head_dim,
                        weight.inner.device_ptr.cast(),
                        epsilon,
                        output.inner.device_ptr.cast(),
                        output_key_offset,
                        output_value_offset,
                        self.stream,
                    )
                },
                "psionic_cuda_pack_qwen35_key_value_rms_norm_f32",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_pack_qwen35_hybrid_qkv_rms_norm_f32(
            &mut self,
            input: &PlatformBuffer,
            q_offset: usize,
            k_offset: usize,
            v_offset: usize,
            group_count: usize,
            state_size: usize,
            v_size: usize,
            q_weight: &PlatformBuffer,
            k_weight: &PlatformBuffer,
            epsilon: f32,
            output: &PlatformBuffer,
            output_q_offset: usize,
            output_k_offset: usize,
            output_v_offset: usize,
        ) -> Result<(), RuntimeError> {
            let q_offset = c_int::try_from(q_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused hybrid q offset exceeds c_int",
                ))
            })?;
            let k_offset = c_int::try_from(k_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused hybrid k offset exceeds c_int",
                ))
            })?;
            let v_offset = c_int::try_from(v_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused hybrid v offset exceeds c_int",
                ))
            })?;
            let group_count = c_int::try_from(group_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused hybrid group count exceeds c_int",
                ))
            })?;
            let state_size = c_int::try_from(state_size).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused hybrid state size exceeds c_int",
                ))
            })?;
            let v_size = c_int::try_from(v_size).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused hybrid v size exceeds c_int",
                ))
            })?;
            let output_q_offset = c_int::try_from(output_q_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused hybrid output q offset exceeds c_int",
                ))
            })?;
            let output_k_offset = c_int::try_from(output_k_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused hybrid output k offset exceeds c_int",
                ))
            })?;
            let output_v_offset = c_int::try_from(output_v_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 fused hybrid output v offset exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (psionic_cuda_pack_qwen35_hybrid_qkv_rms_norm_f32
                        as PackQwen35HybridQkvRmsNormF32Kernel)(
                        input.inner.device_ptr.cast(),
                        q_offset,
                        k_offset,
                        v_offset,
                        group_count,
                        state_size,
                        v_size,
                        q_weight.inner.device_ptr.cast(),
                        k_weight.inner.device_ptr.cast(),
                        epsilon,
                        output.inner.device_ptr.cast(),
                        output_q_offset,
                        output_k_offset,
                        output_v_offset,
                        self.stream,
                    )
                },
                "psionic_cuda_pack_qwen35_hybrid_qkv_rms_norm_f32",
            )
        }

        pub(super) fn encode_depthwise_causal_conv1d_step_f32(
            &mut self,
            input: &PlatformBuffer,
            state: &PlatformBuffer,
            weights: &PlatformBuffer,
            channels: usize,
            kernel_size: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let channels = c_int::try_from(channels).map_err(|_| {
                RuntimeError::Backend(String::from("cuda depthwise conv channels exceed c_int"))
            })?;
            let kernel_size = c_int::try_from(kernel_size).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda depthwise conv kernel size exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_depthwise_causal_conv1d_step_f32(
                        input.inner.device_ptr.cast(),
                        state.inner.device_ptr.cast(),
                        weights.inner.device_ptr.cast(),
                        channels,
                        kernel_size,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_depthwise_causal_conv1d_step_f32",
            )
        }

        pub(super) fn encode_depthwise_causal_conv1d_step_silu_f32(
            &mut self,
            input: &PlatformBuffer,
            state: &PlatformBuffer,
            weights: &PlatformBuffer,
            channels: usize,
            kernel_size: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let channels = c_int::try_from(channels).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 depthwise conv+silu channel count exceeds c_int",
                ))
            })?;
            let kernel_size = c_int::try_from(kernel_size).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda qwen35 depthwise conv+silu kernel size exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_depthwise_causal_conv1d_step_silu_f32(
                        input.inner.device_ptr.cast(),
                        state.inner.device_ptr.cast(),
                        weights.inner.device_ptr.cast(),
                        channels,
                        kernel_size,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_depthwise_causal_conv1d_step_silu_f32",
            )
        }

        pub(super) fn encode_qwen35_ssm_decay_beta_f32(
            &mut self,
            input: &PlatformBuffer,
            alpha_offset: usize,
            beta_offset: usize,
            ssm_a: &PlatformBuffer,
            ssm_dt: &PlatformBuffer,
            element_count: usize,
            decay_output: &PlatformBuffer,
            beta_output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let alpha_offset = c_int::try_from(alpha_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda qwen35 ssm alpha offset exceeds c_int"))
            })?;
            let beta_offset = c_int::try_from(beta_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda qwen35 ssm beta offset exceeds c_int"))
            })?;
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda qwen35 ssm element count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_qwen35_ssm_decay_beta_f32(
                        input.inner.device_ptr.cast(),
                        alpha_offset,
                        beta_offset,
                        ssm_a.inner.device_ptr.cast(),
                        ssm_dt.inner.device_ptr.cast(),
                        element_count,
                        decay_output.inner.device_ptr.cast(),
                        beta_output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_qwen35_ssm_decay_beta_f32",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_gated_delta_step_f32(
            &mut self,
            qkv: &PlatformBuffer,
            query_offset: usize,
            key_offset: usize,
            value_offset: usize,
            decay: &PlatformBuffer,
            beta: &PlatformBuffer,
            state: &PlatformBuffer,
            key_head_count: usize,
            value_head_count: usize,
            key_dim: usize,
            value_dim: usize,
            v_head_reordered: bool,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let query_offset = c_int::try_from(query_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda gated_delta query offset exceeds c_int"))
            })?;
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda gated_delta key offset exceeds c_int"))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda gated_delta value offset exceeds c_int"))
            })?;
            let key_head_count = c_int::try_from(key_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda gated_delta key head count exceeds c_int",
                ))
            })?;
            let value_head_count = c_int::try_from(value_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda gated_delta value head count exceeds c_int",
                ))
            })?;
            let key_dim = c_int::try_from(key_dim).map_err(|_| {
                RuntimeError::Backend(String::from("cuda gated_delta key dim exceeds c_int"))
            })?;
            let value_dim = c_int::try_from(value_dim).map_err(|_| {
                RuntimeError::Backend(String::from("cuda gated_delta value dim exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_gated_delta_step_f32(
                        qkv.inner.device_ptr.cast(),
                        query_offset,
                        key_offset,
                        value_offset,
                        decay.inner.device_ptr.cast(),
                        beta.inner.device_ptr.cast(),
                        state.inner.device_ptr.cast(),
                        key_head_count,
                        value_head_count,
                        key_dim,
                        value_dim,
                        c_int::from(v_head_reordered),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_gated_delta_step_f32",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_rope_neox_in_place(
            &mut self,
            values: &PlatformBuffer,
            element_offset: usize,
            head_count: usize,
            head_dim: usize,
            rotary_dim: usize,
            position: usize,
            freq_scale: f32,
            ext_factor: f32,
            corr_dims: [f32; 2],
            theta_scale: f32,
        ) -> Result<(), RuntimeError> {
            let element_offset = c_int::try_from(element_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda rope_neox element offset exceeds c_int"))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda rope_neox head count exceeds c_int"))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from("cuda rope_neox head dim exceeds c_int"))
            })?;
            let rotary_dim = c_int::try_from(rotary_dim).map_err(|_| {
                RuntimeError::Backend(String::from("cuda rope_neox rotary dim exceeds c_int"))
            })?;
            let position = c_int::try_from(position).map_err(|_| {
                RuntimeError::Backend(String::from("cuda rope_neox position exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_rope_neox_in_place(
                        values.inner.device_ptr.cast(),
                        element_offset,
                        head_count,
                        head_dim,
                        rotary_dim,
                        position,
                        freq_scale,
                        ext_factor,
                        corr_dims[0],
                        corr_dims[1],
                        theta_scale,
                        self.stream,
                    )
                },
                "psionic_cuda_rope_neox_in_place",
            )
        }

        pub(super) fn encode_rotary_embedding_backward(
            &mut self,
            grad_output: &PlatformBuffer,
            cos: &PlatformBuffer,
            sin: &PlatformBuffer,
            batch_size: usize,
            head_count: usize,
            sequence_length: usize,
            head_dim: usize,
            batched_tables: bool,
            grad_input: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let batch_size = c_int::try_from(batch_size).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda rotary_embedding_backward batch size exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda rotary_embedding_backward head count exceeds c_int",
                ))
            })?;
            let sequence_length = c_int::try_from(sequence_length).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda rotary_embedding_backward sequence length exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda rotary_embedding_backward head dim exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_rotary_embedding_backward(
                        grad_output.inner.device_ptr.cast(),
                        cos.inner.device_ptr.cast(),
                        sin.inner.device_ptr.cast(),
                        batch_size,
                        head_count,
                        sequence_length,
                        head_dim,
                        if batched_tables { 1 } else { 0 },
                        grad_input.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_rotary_embedding_backward",
            )
        }

        pub(super) fn encode_permute_rank2_transpose_f32(
            &mut self,
            input: &PlatformBuffer,
            rows: usize,
            cols: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda permute rank-2 transpose rows exceed c_int",
                ))
            })?;
            let cols = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda permute rank-2 transpose cols exceed c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_permute_rank2_transpose_f32(
                        input.inner.device_ptr.cast(),
                        rows,
                        cols,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_permute_rank2_transpose_f32",
            )
        }

        pub(super) fn encode_permute_rank4_swap_middle_axes_f32(
            &mut self,
            input: &PlatformBuffer,
            dim0: usize,
            dim1: usize,
            dim2: usize,
            dim3: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let dim0 = c_int::try_from(dim0).map_err(|_| {
                RuntimeError::Backend(String::from("cuda permute rank-4 dim0 exceeds c_int"))
            })?;
            let dim1 = c_int::try_from(dim1).map_err(|_| {
                RuntimeError::Backend(String::from("cuda permute rank-4 dim1 exceeds c_int"))
            })?;
            let dim2 = c_int::try_from(dim2).map_err(|_| {
                RuntimeError::Backend(String::from("cuda permute rank-4 dim2 exceeds c_int"))
            })?;
            let dim3 = c_int::try_from(dim3).map_err(|_| {
                RuntimeError::Backend(String::from("cuda permute rank-4 dim3 exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_permute_rank4_swap_middle_axes_f32(
                        input.inner.device_ptr.cast(),
                        dim0,
                        dim1,
                        dim2,
                        dim3,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_permute_rank4_swap_middle_axes_f32",
            )
        }

        pub(super) fn encode_reduce_sum_rows_f32(
            &mut self,
            input: &PlatformBuffer,
            row_count: usize,
            column_count: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let row_count = c_int::try_from(row_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda reduce_sum row count exceeds c_int"))
            })?;
            let column_count = c_int::try_from(column_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda reduce_sum column count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_reduce_sum_rows_f32(
                        input.inner.device_ptr.cast(),
                        row_count,
                        column_count,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_reduce_sum_rows_f32",
            )
        }

        pub(super) fn encode_reduce_sum_axis0_f32(
            &mut self,
            input: &PlatformBuffer,
            axis0_extent: usize,
            row_width: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let axis0_extent = c_int::try_from(axis0_extent).map_err(|_| {
                RuntimeError::Backend(String::from("cuda reduce_sum axis0 extent exceeds c_int"))
            })?;
            let row_width = c_int::try_from(row_width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda reduce_sum axis0 row width exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_reduce_sum_axis0_f32(
                        input.inner.device_ptr.cast(),
                        axis0_extent,
                        row_width,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_reduce_sum_axis0_f32",
            )
        }

        pub(super) fn encode_reduce_sum_axis1_rank3_f32(
            &mut self,
            input: &PlatformBuffer,
            dim0: usize,
            dim1: usize,
            dim2: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let dim0 = c_int::try_from(dim0).map_err(|_| {
                RuntimeError::Backend(String::from("cuda reduce_sum axis1 dim0 exceeds c_int"))
            })?;
            let dim1 = c_int::try_from(dim1).map_err(|_| {
                RuntimeError::Backend(String::from("cuda reduce_sum axis1 dim1 exceeds c_int"))
            })?;
            let dim2 = c_int::try_from(dim2).map_err(|_| {
                RuntimeError::Backend(String::from("cuda reduce_sum axis1 dim2 exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_reduce_sum_axis1_rank3_f32(
                        input.inner.device_ptr.cast(),
                        dim0,
                        dim1,
                        dim2,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_reduce_sum_axis1_rank3_f32",
            )
        }

        pub(super) fn encode_expand_rank3_f32(
            &mut self,
            input: &PlatformBuffer,
            input_dims: [usize; 3],
            output_dims: [usize; 3],
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let input_dim0 = c_int::try_from(input_dims[0]).map_err(|_| {
                RuntimeError::Backend(String::from("cuda expand rank-3 input dim0 exceeds c_int"))
            })?;
            let input_dim1 = c_int::try_from(input_dims[1]).map_err(|_| {
                RuntimeError::Backend(String::from("cuda expand rank-3 input dim1 exceeds c_int"))
            })?;
            let input_dim2 = c_int::try_from(input_dims[2]).map_err(|_| {
                RuntimeError::Backend(String::from("cuda expand rank-3 input dim2 exceeds c_int"))
            })?;
            let output_dim0 = c_int::try_from(output_dims[0]).map_err(|_| {
                RuntimeError::Backend(String::from("cuda expand rank-3 output dim0 exceeds c_int"))
            })?;
            let output_dim1 = c_int::try_from(output_dims[1]).map_err(|_| {
                RuntimeError::Backend(String::from("cuda expand rank-3 output dim1 exceeds c_int"))
            })?;
            let output_dim2 = c_int::try_from(output_dims[2]).map_err(|_| {
                RuntimeError::Backend(String::from("cuda expand rank-3 output dim2 exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_expand_rank3_f32(
                        input.inner.device_ptr.cast(),
                        input_dim0,
                        input_dim1,
                        input_dim2,
                        output_dim0,
                        output_dim1,
                        output_dim2,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_expand_rank3_f32",
            )
        }

        pub(super) fn encode_expand_rank4_f32(
            &mut self,
            input: &PlatformBuffer,
            input_dims: [usize; 4],
            output_dims: [usize; 4],
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let input_dim0 = c_int::try_from(input_dims[0]).map_err(|_| {
                RuntimeError::Backend(String::from("cuda expand rank-4 input dim0 exceeds c_int"))
            })?;
            let input_dim1 = c_int::try_from(input_dims[1]).map_err(|_| {
                RuntimeError::Backend(String::from("cuda expand rank-4 input dim1 exceeds c_int"))
            })?;
            let input_dim2 = c_int::try_from(input_dims[2]).map_err(|_| {
                RuntimeError::Backend(String::from("cuda expand rank-4 input dim2 exceeds c_int"))
            })?;
            let input_dim3 = c_int::try_from(input_dims[3]).map_err(|_| {
                RuntimeError::Backend(String::from("cuda expand rank-4 input dim3 exceeds c_int"))
            })?;
            let output_dim0 = c_int::try_from(output_dims[0]).map_err(|_| {
                RuntimeError::Backend(String::from("cuda expand rank-4 output dim0 exceeds c_int"))
            })?;
            let output_dim1 = c_int::try_from(output_dims[1]).map_err(|_| {
                RuntimeError::Backend(String::from("cuda expand rank-4 output dim1 exceeds c_int"))
            })?;
            let output_dim2 = c_int::try_from(output_dims[2]).map_err(|_| {
                RuntimeError::Backend(String::from("cuda expand rank-4 output dim2 exceeds c_int"))
            })?;
            let output_dim3 = c_int::try_from(output_dims[3]).map_err(|_| {
                RuntimeError::Backend(String::from("cuda expand rank-4 output dim3 exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_expand_rank4_f32(
                        input.inner.device_ptr.cast(),
                        input_dim0,
                        input_dim1,
                        input_dim2,
                        input_dim3,
                        output_dim0,
                        output_dim1,
                        output_dim2,
                        output_dim3,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_expand_rank4_f32",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache(
            &mut self,
            qkv: &PlatformBuffer,
            query_offset: usize,
            key_offset: usize,
            value_offset: usize,
            cache_keys: &PlatformBuffer,
            cache_values: &PlatformBuffer,
            cache_width: usize,
            layer_offset: usize,
            past_tokens: usize,
            sliding_window: usize,
            head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            rotary_dim: usize,
            position: usize,
            freq_scale: f32,
            ext_factor: f32,
            corr_dims: [f32; 2],
            theta_scale: f32,
            attention_sinks: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let query_offset = c_int::try_from(query_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention query offset exceeds c_int",
                ))
            })?;
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention key offset exceeds c_int",
                ))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention value offset exceeds c_int",
                ))
            })?;
            let cache_width = c_int::try_from(cache_width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention cache width exceeds c_int",
                ))
            })?;
            let layer_offset = c_int::try_from(layer_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention layer offset exceeds c_int",
                ))
            })?;
            let past_tokens = c_int::try_from(past_tokens).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention past token count exceeds c_int",
                ))
            })?;
            let sliding_window = c_int::try_from(sliding_window).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention sliding window exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention kv head count exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from("cuda fused attention head dim exceeds c_int"))
            })?;
            let rotary_dim = c_int::try_from(rotary_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention rotary dim exceeds c_int",
                ))
            })?;
            let position = c_int::try_from(position).map_err(|_| {
                RuntimeError::Backend(String::from("cuda fused attention position exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_decode_rope_cache(
                        qkv.inner.device_ptr.cast(),
                        query_offset,
                        key_offset,
                        value_offset,
                        cache_keys.inner.device_ptr.cast(),
                        cache_values.inner.device_ptr.cast(),
                        cache_width,
                        layer_offset,
                        past_tokens,
                        sliding_window,
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        position,
                        freq_scale,
                        ext_factor,
                        corr_dims[0],
                        corr_dims[1],
                        theta_scale,
                        attention_sinks
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_decode_rope_cache",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv(
            &mut self,
            qkv: &PlatformBuffer,
            query_offset: usize,
            key_offset: usize,
            value_offset: usize,
            cache_keys: &PlatformBuffer,
            cache_values: &PlatformBuffer,
            cache_width: usize,
            layer_offset: usize,
            past_tokens: usize,
            sliding_window: usize,
            head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            rotary_dim: usize,
            position: usize,
            freq_scale: f32,
            ext_factor: f32,
            corr_dims: [f32; 2],
            theta_scale: f32,
            attention_sinks: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let query_offset = c_int::try_from(query_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) query offset exceeds c_int",
                ))
            })?;
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) key offset exceeds c_int",
                ))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) value offset exceeds c_int",
                ))
            })?;
            let cache_width = c_int::try_from(cache_width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) cache width exceeds c_int",
                ))
            })?;
            let layer_offset = c_int::try_from(layer_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) layer offset exceeds c_int",
                ))
            })?;
            let past_tokens = c_int::try_from(past_tokens).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) past token count exceeds c_int",
                ))
            })?;
            let sliding_window = c_int::try_from(sliding_window).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) sliding window exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) kv head count exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) head dim exceeds c_int",
                ))
            })?;
            let rotary_dim = c_int::try_from(rotary_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) rotary dim exceeds c_int",
                ))
            })?;
            let position = c_int::try_from(position).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) position exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_decode_rope_cache_f16_kv(
                        qkv.inner.device_ptr.cast(),
                        query_offset,
                        key_offset,
                        value_offset,
                        cache_keys.inner.device_ptr.cast(),
                        cache_values.inner.device_ptr.cast(),
                        cache_width,
                        layer_offset,
                        past_tokens,
                        sliding_window,
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        position,
                        freq_scale,
                        ext_factor,
                        corr_dims[0],
                        corr_dims[1],
                        theta_scale,
                        attention_sinks
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_decode_rope_cache_f16_kv",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_mrope_cache_f16_kv(
            &mut self,
            qkv: &PlatformBuffer,
            query_offset: usize,
            key_offset: usize,
            value_offset: usize,
            cache_keys: &PlatformBuffer,
            cache_values: &PlatformBuffer,
            cache_width: usize,
            layer_offset: usize,
            past_tokens: usize,
            sliding_window: usize,
            head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            rotary_dim: usize,
            positions: [usize; 3],
            sections: [usize; 4],
            interleaved: bool,
            freq_scale: f32,
            ext_factor: f32,
            corr_dims: [f32; 2],
            theta_scale: f32,
            attention_sinks: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let to_c_int = |value: usize, name: &str| {
                c_int::try_from(value).map_err(|_| {
                    RuntimeError::Backend(format!(
                        "cuda fused mrope attention(f16 kv) {name} exceeds c_int"
                    ))
                })
            };
            let query_offset = to_c_int(query_offset, "query offset")?;
            let key_offset = to_c_int(key_offset, "key offset")?;
            let value_offset = to_c_int(value_offset, "value offset")?;
            let cache_width = to_c_int(cache_width, "cache width")?;
            let layer_offset = to_c_int(layer_offset, "layer offset")?;
            let past_tokens = to_c_int(past_tokens, "past token count")?;
            let sliding_window = to_c_int(sliding_window, "sliding window")?;
            let head_count = to_c_int(head_count, "head count")?;
            let kv_head_count = to_c_int(kv_head_count, "kv head count")?;
            let head_dim = to_c_int(head_dim, "head dim")?;
            let rotary_dim = to_c_int(rotary_dim, "rotary dim")?;
            let positions = [
                to_c_int(positions[0], "temporal position")?,
                to_c_int(positions[1], "height position")?,
                to_c_int(positions[2], "width position")?,
            ];
            let sections = [
                to_c_int(sections[0], "temporal section")?,
                to_c_int(sections[1], "height section")?,
                to_c_int(sections[2], "width section")?,
                to_c_int(sections[3], "extra section")?,
            ];
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_decode_mrope_cache_f16_kv(
                        qkv.inner.device_ptr.cast(),
                        query_offset,
                        key_offset,
                        value_offset,
                        cache_keys.inner.device_ptr.cast(),
                        cache_values.inner.device_ptr.cast(),
                        cache_width,
                        layer_offset,
                        past_tokens,
                        sliding_window,
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        positions[0],
                        positions[1],
                        positions[2],
                        sections[0],
                        sections[1],
                        sections[2],
                        sections[3],
                        c_int::from(interleaved),
                        freq_scale,
                        ext_factor,
                        corr_dims[0],
                        corr_dims[1],
                        theta_scale,
                        attention_sinks
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_decode_mrope_cache_f16_kv",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_turboquant_kv(
            &mut self,
            qkv: &PlatformBuffer,
            query_offset: usize,
            key_offset: usize,
            value_offset: usize,
            cache_keys: &PlatformBuffer,
            cache_values: &PlatformBuffer,
            cache_width: usize,
            layer_offset: usize,
            past_tokens: usize,
            sliding_window: usize,
            head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            rotary_dim: usize,
            position: usize,
            freq_scale: f32,
            ext_factor: f32,
            corr_dims: [f32; 2],
            theta_scale: f32,
            attention_sinks: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let query_offset = c_int::try_from(query_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(TurboQuant kv) query offset exceeds c_int",
                ))
            })?;
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(TurboQuant kv) key offset exceeds c_int",
                ))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(TurboQuant kv) value offset exceeds c_int",
                ))
            })?;
            let cache_width = c_int::try_from(cache_width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(TurboQuant kv) cache width exceeds c_int",
                ))
            })?;
            let layer_offset = c_int::try_from(layer_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(TurboQuant kv) layer offset exceeds c_int",
                ))
            })?;
            let past_tokens = c_int::try_from(past_tokens).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(TurboQuant kv) past token count exceeds c_int",
                ))
            })?;
            let sliding_window = c_int::try_from(sliding_window).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(TurboQuant kv) sliding window exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(TurboQuant kv) head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(TurboQuant kv) kv head count exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(TurboQuant kv) head dim exceeds c_int",
                ))
            })?;
            let rotary_dim = c_int::try_from(rotary_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(TurboQuant kv) rotary dim exceeds c_int",
                ))
            })?;
            let position = c_int::try_from(position).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(TurboQuant kv) position exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_decode_rope_cache_turboquant_kv(
                        qkv.inner.device_ptr.cast(),
                        query_offset,
                        key_offset,
                        value_offset,
                        cache_keys.inner.device_ptr.cast(),
                        cache_values.inner.device_ptr.cast(),
                        cache_width,
                        layer_offset,
                        past_tokens,
                        sliding_window,
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        position,
                        freq_scale,
                        ext_factor,
                        corr_dims[0],
                        corr_dims[1],
                        theta_scale,
                        attention_sinks
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_decode_rope_cache_turboquant_kv",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv_q8_1(
            &mut self,
            qkv: &PlatformBuffer,
            query_offset: usize,
            key_offset: usize,
            value_offset: usize,
            cache_keys: &PlatformBuffer,
            cache_values: &PlatformBuffer,
            cache_width: usize,
            layer_offset: usize,
            past_tokens: usize,
            sliding_window: usize,
            head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            rotary_dim: usize,
            position: usize,
            freq_scale: f32,
            ext_factor: f32,
            corr_dims: [f32; 2],
            theta_scale: f32,
            attention_sinks: Option<&PlatformBuffer>,
            output_q8_1: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let query_offset = c_int::try_from(query_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) query offset exceeds c_int",
                ))
            })?;
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) key offset exceeds c_int",
                ))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) value offset exceeds c_int",
                ))
            })?;
            let cache_width = c_int::try_from(cache_width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) cache width exceeds c_int",
                ))
            })?;
            let layer_offset = c_int::try_from(layer_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) layer offset exceeds c_int",
                ))
            })?;
            let past_tokens = c_int::try_from(past_tokens).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) past token count exceeds c_int",
                ))
            })?;
            let sliding_window = c_int::try_from(sliding_window).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) sliding window exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) kv head count exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) head dim exceeds c_int",
                ))
            })?;
            let rotary_dim = c_int::try_from(rotary_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) rotary dim exceeds c_int",
                ))
            })?;
            let position = c_int::try_from(position).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) position exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_decode_rope_cache_f16_kv_q8_1(
                        qkv.inner.device_ptr.cast(),
                        query_offset,
                        key_offset,
                        value_offset,
                        cache_keys.inner.device_ptr.cast(),
                        cache_values.inner.device_ptr.cast(),
                        cache_width,
                        layer_offset,
                        past_tokens,
                        sliding_window,
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        position,
                        freq_scale,
                        ext_factor,
                        corr_dims[0],
                        corr_dims[1],
                        theta_scale,
                        attention_sinks
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output_q8_1.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_decode_rope_cache_f16_kv_q8_1",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv_graph(
            &mut self,
            qkv: &PlatformBuffer,
            query_offset: usize,
            key_offset: usize,
            value_offset: usize,
            cache_keys: &PlatformBuffer,
            cache_values: &PlatformBuffer,
            cache_width: usize,
            layer_offset: usize,
            decode_params: &PlatformBuffer,
            sliding_window: usize,
            head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            rotary_dim: usize,
            freq_scale: f32,
            ext_factor: f32,
            corr_dims: [f32; 2],
            theta_scale: f32,
            attention_sinks: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let query_offset = c_int::try_from(query_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) query offset exceeds c_int",
                ))
            })?;
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) key offset exceeds c_int",
                ))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) value offset exceeds c_int",
                ))
            })?;
            let cache_width = c_int::try_from(cache_width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) cache width exceeds c_int",
                ))
            })?;
            let layer_offset = c_int::try_from(layer_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) layer offset exceeds c_int",
                ))
            })?;
            let sliding_window = c_int::try_from(sliding_window).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) sliding window exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) kv head count exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) head dim exceeds c_int",
                ))
            })?;
            let rotary_dim = c_int::try_from(rotary_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) rotary dim exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_decode_rope_cache_f16_kv_graph(
                        qkv.inner.device_ptr.cast(),
                        query_offset,
                        key_offset,
                        value_offset,
                        cache_keys.inner.device_ptr.cast(),
                        cache_values.inner.device_ptr.cast(),
                        cache_width,
                        layer_offset,
                        decode_params.inner.device_ptr.cast(),
                        sliding_window,
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        freq_scale,
                        ext_factor,
                        corr_dims[0],
                        corr_dims[1],
                        theta_scale,
                        attention_sinks
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_decode_rope_cache_f16_kv_graph",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv_graph_fa3(
            &mut self,
            qkv: &PlatformBuffer,
            query_offset: usize,
            key_offset: usize,
            value_offset: usize,
            cache_keys: &PlatformBuffer,
            cache_values: &PlatformBuffer,
            cache_width: usize,
            layer_offset: usize,
            decode_params: &PlatformBuffer,
            sliding_window: usize,
            head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            rotary_dim: usize,
            freq_scale: f32,
            ext_factor: f32,
            corr_dims: [f32; 2],
            theta_scale: f32,
            attention_sinks: Option<&PlatformBuffer>,
            partial_output: &PlatformBuffer,
            partial_max: &PlatformBuffer,
            partial_sum: &PlatformBuffer,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let query_offset = c_int::try_from(query_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fa3 attention(graph, f16 kv) query offset exceeds c_int",
                ))
            })?;
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fa3 attention(graph, f16 kv) key offset exceeds c_int",
                ))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fa3 attention(graph, f16 kv) value offset exceeds c_int",
                ))
            })?;
            let cache_width = c_int::try_from(cache_width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fa3 attention(graph, f16 kv) cache width exceeds c_int",
                ))
            })?;
            let layer_offset = c_int::try_from(layer_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fa3 attention(graph, f16 kv) layer offset exceeds c_int",
                ))
            })?;
            let sliding_window = c_int::try_from(sliding_window).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fa3 attention(graph, f16 kv) sliding window exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fa3 attention(graph, f16 kv) head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fa3 attention(graph, f16 kv) kv head count exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fa3 attention(graph, f16 kv) head dim exceeds c_int",
                ))
            })?;
            let rotary_dim = c_int::try_from(rotary_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fa3 attention(graph, f16 kv) rotary dim exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_decode_rope_cache_f16_kv_graph_fa3(
                        qkv.inner.device_ptr.cast(),
                        query_offset,
                        key_offset,
                        value_offset,
                        cache_keys.inner.device_ptr.cast(),
                        cache_values.inner.device_ptr.cast(),
                        cache_width,
                        layer_offset,
                        decode_params.inner.device_ptr.cast(),
                        sliding_window,
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        freq_scale,
                        ext_factor,
                        corr_dims[0],
                        corr_dims[1],
                        theta_scale,
                        attention_sinks
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        partial_output.inner.device_ptr.cast(),
                        partial_max.inner.device_ptr.cast(),
                        partial_sum.inner.device_ptr.cast(),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_decode_rope_cache_f16_kv_graph_fa3",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_turboquant_kv_graph(
            &mut self,
            qkv: &PlatformBuffer,
            query_offset: usize,
            key_offset: usize,
            value_offset: usize,
            cache_keys: &PlatformBuffer,
            cache_values: &PlatformBuffer,
            cache_width: usize,
            layer_offset: usize,
            decode_params: &PlatformBuffer,
            sliding_window: usize,
            head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            rotary_dim: usize,
            freq_scale: f32,
            ext_factor: f32,
            corr_dims: [f32; 2],
            theta_scale: f32,
            attention_sinks: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let query_offset = c_int::try_from(query_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, TurboQuant kv) query offset exceeds c_int",
                ))
            })?;
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, TurboQuant kv) key offset exceeds c_int",
                ))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, TurboQuant kv) value offset exceeds c_int",
                ))
            })?;
            let cache_width = c_int::try_from(cache_width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, TurboQuant kv) cache width exceeds c_int",
                ))
            })?;
            let layer_offset = c_int::try_from(layer_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, TurboQuant kv) layer offset exceeds c_int",
                ))
            })?;
            let sliding_window = c_int::try_from(sliding_window).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, TurboQuant kv) sliding window exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, TurboQuant kv) head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, TurboQuant kv) kv head count exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, TurboQuant kv) head dim exceeds c_int",
                ))
            })?;
            let rotary_dim = c_int::try_from(rotary_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, TurboQuant kv) rotary dim exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_decode_rope_cache_turboquant_kv_graph(
                        qkv.inner.device_ptr.cast(),
                        query_offset,
                        key_offset,
                        value_offset,
                        cache_keys.inner.device_ptr.cast(),
                        cache_values.inner.device_ptr.cast(),
                        cache_width,
                        layer_offset,
                        decode_params.inner.device_ptr.cast(),
                        sliding_window,
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        freq_scale,
                        ext_factor,
                        corr_dims[0],
                        corr_dims[1],
                        theta_scale,
                        attention_sinks
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_decode_rope_cache_turboquant_kv_graph",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv_graph_q8_1(
            &mut self,
            qkv: &PlatformBuffer,
            query_offset: usize,
            key_offset: usize,
            value_offset: usize,
            cache_keys: &PlatformBuffer,
            cache_values: &PlatformBuffer,
            cache_width: usize,
            layer_offset: usize,
            decode_params: &PlatformBuffer,
            sliding_window: usize,
            head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            rotary_dim: usize,
            freq_scale: f32,
            ext_factor: f32,
            corr_dims: [f32; 2],
            theta_scale: f32,
            attention_sinks: Option<&PlatformBuffer>,
            output_q8_1: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let query_offset = c_int::try_from(query_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) query offset exceeds c_int",
                ))
            })?;
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) key offset exceeds c_int",
                ))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) value offset exceeds c_int",
                ))
            })?;
            let cache_width = c_int::try_from(cache_width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) cache width exceeds c_int",
                ))
            })?;
            let layer_offset = c_int::try_from(layer_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) layer offset exceeds c_int",
                ))
            })?;
            let sliding_window = c_int::try_from(sliding_window).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) sliding window exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) kv head count exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) head dim exceeds c_int",
                ))
            })?;
            let rotary_dim = c_int::try_from(rotary_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) rotary dim exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_decode_rope_cache_f16_kv_graph_q8_1(
                        qkv.inner.device_ptr.cast(),
                        query_offset,
                        key_offset,
                        value_offset,
                        cache_keys.inner.device_ptr.cast(),
                        cache_values.inner.device_ptr.cast(),
                        cache_width,
                        layer_offset,
                        decode_params.inner.device_ptr.cast(),
                        sliding_window,
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        freq_scale,
                        ext_factor,
                        corr_dims[0],
                        corr_dims[1],
                        theta_scale,
                        attention_sinks
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output_q8_1.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_decode_rope_cache_f16_kv_graph_q8_1",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode(
            &mut self,
            query: &PlatformBuffer,
            query_offset: usize,
            current_key: &PlatformBuffer,
            key_offset: usize,
            current_value: &PlatformBuffer,
            value_offset: usize,
            cache_keys: &PlatformBuffer,
            cache_values: &PlatformBuffer,
            cache_width: usize,
            layer_offset: usize,
            past_tokens: usize,
            sliding_window: usize,
            head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            attention_sinks: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let query_offset = c_int::try_from(query_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention query offset exceeds c_int"))
            })?;
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention key offset exceeds c_int"))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention value offset exceeds c_int"))
            })?;
            let cache_width = c_int::try_from(cache_width).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention cache width exceeds c_int"))
            })?;
            let layer_offset = c_int::try_from(layer_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention layer offset exceeds c_int"))
            })?;
            let past_tokens = c_int::try_from(past_tokens).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda attention past token count exceeds c_int",
                ))
            })?;
            let sliding_window = c_int::try_from(sliding_window).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention sliding window exceeds c_int"))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention head count exceeds c_int"))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention kv head count exceeds c_int"))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention head dim exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_decode(
                        query.inner.device_ptr.cast(),
                        query_offset,
                        current_key.inner.device_ptr.cast(),
                        key_offset,
                        current_value.inner.device_ptr.cast(),
                        value_offset,
                        cache_keys.inner.device_ptr.cast(),
                        cache_values.inner.device_ptr.cast(),
                        cache_width,
                        layer_offset,
                        past_tokens,
                        sliding_window,
                        head_count,
                        kv_head_count,
                        head_dim,
                        attention_sinks
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_decode",
            )
        }

        pub(super) fn encode_attention_causal_sequence_f32(
            &mut self,
            query: &PlatformBuffer,
            key: &PlatformBuffer,
            value: &PlatformBuffer,
            batch_size: usize,
            head_count: usize,
            kv_head_count: usize,
            sequence_length: usize,
            head_dim: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let batch_size = c_int::try_from(batch_size).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention batch size exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention kv head count exceeds c_int",
                ))
            })?;
            let sequence_length = c_int::try_from(sequence_length).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention sequence length exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from("cuda causal attention head dim exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_causal_sequence_f32(
                        query.inner.device_ptr.cast(),
                        key.inner.device_ptr.cast(),
                        value.inner.device_ptr.cast(),
                        batch_size,
                        head_count,
                        kv_head_count,
                        sequence_length,
                        head_dim,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_causal_sequence_f32",
            )
        }

        pub(super) fn encode_attention_causal_sequence_bf16(
            &mut self,
            query: &PlatformBuffer,
            key: &PlatformBuffer,
            value: &PlatformBuffer,
            batch_size: usize,
            head_count: usize,
            kv_head_count: usize,
            sequence_length: usize,
            head_dim: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let batch_size = c_int::try_from(batch_size).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention batch size exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention kv head count exceeds c_int",
                ))
            })?;
            let sequence_length = c_int::try_from(sequence_length).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention sequence length exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from("cuda causal attention head dim exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_causal_sequence_bf16(
                        query.inner.device_ptr.cast(),
                        key.inner.device_ptr.cast(),
                        value.inner.device_ptr.cast(),
                        batch_size,
                        head_count,
                        kv_head_count,
                        sequence_length,
                        head_dim,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_causal_sequence_bf16",
            )
        }

        pub(super) fn encode_attention_causal_row_softmax_in_place_f32(
            &mut self,
            logits: &PlatformBuffer,
            row_count: usize,
            sequence_length: usize,
            scale: f32,
        ) -> Result<(), RuntimeError> {
            let row_count = c_int::try_from(row_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention row-softmax row count exceeds c_int",
                ))
            })?;
            let sequence_length = c_int::try_from(sequence_length).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention row-softmax sequence length exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_causal_row_softmax_in_place_f32(
                        logits.inner.device_ptr.cast(),
                        row_count,
                        sequence_length,
                        scale,
                        self.stream,
                    )
                },
                "psionic_cuda_attention_causal_row_softmax_in_place_f32",
            )
        }

        pub(super) fn encode_attention_causal_row_softmax_backward_in_place_f32(
            &mut self,
            probabilities: &PlatformBuffer,
            grad_probabilities: &PlatformBuffer,
            row_count: usize,
            sequence_length: usize,
            post_scale: f32,
        ) -> Result<(), RuntimeError> {
            let row_count = c_int::try_from(row_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention row-softmax backward row count exceeds c_int",
                ))
            })?;
            let sequence_length = c_int::try_from(sequence_length).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention row-softmax backward sequence length exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_causal_row_softmax_backward_in_place_f32(
                        probabilities.inner.device_ptr.cast(),
                        grad_probabilities.inner.device_ptr.cast(),
                        row_count,
                        sequence_length,
                        post_scale,
                        self.stream,
                    )
                },
                "psionic_cuda_attention_causal_row_softmax_backward_in_place_f32",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_causal_sequence_backward_f32(
            &mut self,
            query: &PlatformBuffer,
            key: &PlatformBuffer,
            value: &PlatformBuffer,
            grad_output: &PlatformBuffer,
            batch_size: usize,
            head_count: usize,
            kv_head_count: usize,
            sequence_length: usize,
            head_dim: usize,
            query_gradient: &PlatformBuffer,
            key_gradient: &PlatformBuffer,
            value_gradient: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let batch_size = c_int::try_from(batch_size).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention backward batch size exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention backward head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention backward kv head count exceeds c_int",
                ))
            })?;
            let sequence_length = c_int::try_from(sequence_length).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention backward sequence length exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention backward head dim exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_causal_sequence_backward_f32(
                        query.inner.device_ptr.cast(),
                        key.inner.device_ptr.cast(),
                        value.inner.device_ptr.cast(),
                        grad_output.inner.device_ptr.cast(),
                        batch_size,
                        head_count,
                        kv_head_count,
                        sequence_length,
                        head_dim,
                        query_gradient.inner.device_ptr.cast(),
                        key_gradient.inner.device_ptr.cast(),
                        value_gradient.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_causal_sequence_backward_f32",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_causal_sequence_backward_bf16_to_f32(
            &mut self,
            query: &PlatformBuffer,
            key: &PlatformBuffer,
            value: &PlatformBuffer,
            grad_output: &PlatformBuffer,
            batch_size: usize,
            head_count: usize,
            kv_head_count: usize,
            sequence_length: usize,
            head_dim: usize,
            query_gradient: &PlatformBuffer,
            key_gradient: &PlatformBuffer,
            value_gradient: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let batch_size = c_int::try_from(batch_size).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention backward batch size exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention backward head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention backward kv head count exceeds c_int",
                ))
            })?;
            let sequence_length = c_int::try_from(sequence_length).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention backward sequence length exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda causal attention backward head dim exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_causal_sequence_backward_bf16_to_f32(
                        query.inner.device_ptr.cast(),
                        key.inner.device_ptr.cast(),
                        value.inner.device_ptr.cast(),
                        grad_output.inner.device_ptr.cast(),
                        batch_size,
                        head_count,
                        kv_head_count,
                        sequence_length,
                        head_dim,
                        query_gradient.inner.device_ptr.cast(),
                        key_gradient.inner.device_ptr.cast(),
                        value_gradient.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_causal_sequence_backward_bf16_to_f32",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_router_topk_softmax(
            &mut self,
            weights: &PlatformBuffer,
            bias: Option<&PlatformBuffer>,
            input: &PlatformBuffer,
            expert_count: usize,
            input_size: usize,
            top_k: usize,
            selected_ids: &PlatformBuffer,
            selected_weights: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let expert_count = c_int::try_from(expert_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda router expert count exceeds c_int"))
            })?;
            let input_size = c_int::try_from(input_size).map_err(|_| {
                RuntimeError::Backend(String::from("cuda router input size exceeds c_int"))
            })?;
            let top_k = c_int::try_from(top_k).map_err(|_| {
                RuntimeError::Backend(String::from("cuda router top-k exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_router_topk_softmax(
                        weights.inner.device_ptr.cast(),
                        bias.map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        input.inner.device_ptr.cast(),
                        expert_count,
                        input_size,
                        top_k,
                        selected_ids.inner.device_ptr.cast(),
                        selected_weights.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_router_topk_softmax",
            )
        }

        pub(super) fn encode_router_topk_delayed_softmax(
            &mut self,
            logits: &PlatformBuffer,
            expert_count: usize,
            top_k: usize,
            selected_ids: &PlatformBuffer,
            selected_weights: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let expert_count = c_int::try_from(expert_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda router expert count exceeds c_int"))
            })?;
            let top_k = c_int::try_from(top_k).map_err(|_| {
                RuntimeError::Backend(String::from("cuda router top-k exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_router_topk_delayed_softmax(
                        logits.inner.device_ptr.cast(),
                        expert_count,
                        top_k,
                        selected_ids.inner.device_ptr.cast(),
                        selected_weights.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_router_topk_delayed_softmax",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_gate_up_swiglu(
            &mut self,
            weights: &PlatformBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows_per_expert: usize,
            columns: usize,
            gate_rows: usize,
            up_rows: usize,
            selected_ids: &PlatformBuffer,
            selected_count: usize,
            input: &PlatformBuffer,
            gate_bias: Option<&PlatformBuffer>,
            up_bias: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let mode = match mode {
                QuantizationMode::GgmlQ8_0 => 0,
                QuantizationMode::GgmlMxfp4 => 1,
                other => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda moe gate/up kernel does not support {other:?}"
                    )));
                }
            };
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe gate/up row stride exceeds c_int"))
            })?;
            let rows_per_expert = c_int::try_from(rows_per_expert).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda moe gate/up rows per expert exceeds c_int",
                ))
            })?;
            let columns = c_int::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe gate/up columns exceeds c_int"))
            })?;
            let gate_rows = c_int::try_from(gate_rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe gate rows exceeds c_int"))
            })?;
            let up_rows = c_int::try_from(up_rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe up rows exceeds c_int"))
            })?;
            let selected_count = c_int::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe selected count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_moe_gate_up_swiglu(
                        weights.inner.device_ptr.cast(),
                        mode,
                        row_stride,
                        rows_per_expert,
                        columns,
                        gate_rows,
                        up_rows,
                        selected_ids.inner.device_ptr.cast(),
                        selected_count,
                        input.inner.device_ptr.cast(),
                        gate_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        up_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_moe_gate_up_swiglu",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_gate_up_swiglu_q8_1(
            &mut self,
            weights: &PlatformBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows_per_expert: usize,
            columns: usize,
            gate_rows: usize,
            up_rows: usize,
            selected_ids: &PlatformBuffer,
            selected_count: usize,
            input_q8_1: &PlatformBuffer,
            gate_bias: Option<&PlatformBuffer>,
            up_bias: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let mode = match mode {
                QuantizationMode::GgmlQ8_0 => 0,
                QuantizationMode::GgmlMxfp4 => 1,
                other => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda moe gate/up kernel does not support {other:?}"
                    )));
                }
            };
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe gate/up row stride exceeds c_int"))
            })?;
            let rows_per_expert = c_int::try_from(rows_per_expert).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda moe gate/up rows per expert exceeds c_int",
                ))
            })?;
            let columns = c_int::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe gate/up columns exceeds c_int"))
            })?;
            let gate_rows = c_int::try_from(gate_rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe gate rows exceeds c_int"))
            })?;
            let up_rows = c_int::try_from(up_rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe up rows exceeds c_int"))
            })?;
            let selected_count = c_int::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe selected count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_moe_gate_up_swiglu_q8_1(
                        weights.inner.device_ptr.cast(),
                        mode,
                        row_stride,
                        rows_per_expert,
                        columns,
                        gate_rows,
                        up_rows,
                        selected_ids.inner.device_ptr.cast(),
                        selected_count,
                        input_q8_1.inner.device_ptr.cast(),
                        gate_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        up_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_moe_gate_up_swiglu_q8_1",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_gate_up_swiglu_q8_1_selected4_quantized(
            &mut self,
            weights: &PlatformBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows_per_expert: usize,
            columns: usize,
            gate_rows: usize,
            up_rows: usize,
            selected_ids: &PlatformBuffer,
            selected_count: usize,
            input_q8_1: &PlatformBuffer,
            gate_bias: Option<&PlatformBuffer>,
            up_bias: Option<&PlatformBuffer>,
            output_q8_1: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let mode = match mode {
                QuantizationMode::GgmlQ8_0 => 0,
                QuantizationMode::GgmlMxfp4 => 1,
                other => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda quantized selected4 moe gate/up kernel does not support {other:?}"
                    )));
                }
            };
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized moe gate/up row stride exceeds c_int",
                ))
            })?;
            let rows_per_expert = c_int::try_from(rows_per_expert).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized moe gate/up rows per expert exceeds c_int",
                ))
            })?;
            let columns = c_int::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized moe gate/up columns exceed c_int",
                ))
            })?;
            let gate_rows = c_int::try_from(gate_rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda quantized moe gate rows exceed c_int"))
            })?;
            let up_rows = c_int::try_from(up_rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda quantized moe up rows exceed c_int"))
            })?;
            let selected_count = c_int::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized moe selected count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_moe_gate_up_swiglu_q8_1_selected4_quantized(
                        weights.inner.device_ptr.cast(),
                        mode,
                        row_stride,
                        rows_per_expert,
                        columns,
                        gate_rows,
                        up_rows,
                        selected_ids.inner.device_ptr.cast(),
                        selected_count,
                        input_q8_1.inner.device_ptr.cast(),
                        gate_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        up_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output_q8_1.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_moe_gate_up_swiglu_q8_1_selected4_quantized",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_down_aggregate(
            &mut self,
            weights: &PlatformBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows: usize,
            columns: usize,
            selected_ids: &PlatformBuffer,
            selected_weights: &PlatformBuffer,
            selected_count: usize,
            activated: &PlatformBuffer,
            bias: Option<&PlatformBuffer>,
            residual: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let mode = match mode {
                QuantizationMode::GgmlQ8_0 => 0,
                QuantizationMode::GgmlMxfp4 => 1,
                other => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda moe down kernel does not support {other:?}"
                    )));
                }
            };
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe down row stride exceeds c_int"))
            })?;
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe down rows exceeds c_int"))
            })?;
            let columns = c_int::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe down columns exceeds c_int"))
            })?;
            let selected_count = c_int::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe selected count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_moe_down_aggregate(
                        weights.inner.device_ptr.cast(),
                        mode,
                        row_stride,
                        rows,
                        columns,
                        selected_ids.inner.device_ptr.cast(),
                        selected_weights.inner.device_ptr.cast(),
                        selected_count,
                        activated.inner.device_ptr.cast(),
                        bias.map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        residual
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_moe_down_aggregate",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_down_aggregate_q8_1(
            &mut self,
            weights: &PlatformBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows: usize,
            columns: usize,
            selected_ids: &PlatformBuffer,
            selected_weights: &PlatformBuffer,
            selected_count: usize,
            activated_q8_1: &PlatformBuffer,
            bias: Option<&PlatformBuffer>,
            residual: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let mode = match mode {
                QuantizationMode::GgmlQ8_0 => 0,
                QuantizationMode::GgmlMxfp4 => 1,
                other => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda moe down kernel does not support {other:?}"
                    )));
                }
            };
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe down row stride exceeds c_int"))
            })?;
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe down rows exceeds c_int"))
            })?;
            let columns = c_int::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe down columns exceeds c_int"))
            })?;
            let selected_count = c_int::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe selected count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_moe_down_aggregate_q8_1(
                        weights.inner.device_ptr.cast(),
                        mode,
                        row_stride,
                        rows,
                        columns,
                        selected_ids.inner.device_ptr.cast(),
                        selected_weights.inner.device_ptr.cast(),
                        selected_count,
                        activated_q8_1.inner.device_ptr.cast(),
                        bias.map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        residual
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_moe_down_aggregate_q8_1",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_down_aggregate_q8_1_f32(
            &mut self,
            weights: &PlatformBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows: usize,
            columns: usize,
            selected_ids: &PlatformBuffer,
            selected_weights: &PlatformBuffer,
            selected_count: usize,
            activated: &PlatformBuffer,
            bias: Option<&PlatformBuffer>,
            residual: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            if selected_count > 4 {
                return Err(RuntimeError::Backend(String::from(
                    "cuda fused moe down f32->q8_1 path requires selected_count <= 4",
                )));
            }
            let mode = match mode {
                QuantizationMode::GgmlQ8_0 => 0,
                QuantizationMode::GgmlMxfp4 => 1,
                other => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda fused moe down kernel does not support {other:?}"
                    )));
                }
            };
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from("cuda fused moe down row stride exceeds c_int"))
            })?;
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda fused moe down rows exceeds c_int"))
            })?;
            let columns = c_int::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from("cuda fused moe down columns exceeds c_int"))
            })?;
            let selected_count = c_int::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda fused moe selected count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_moe_down_aggregate_q8_1_f32(
                        weights.inner.device_ptr.cast(),
                        mode,
                        row_stride,
                        rows,
                        columns,
                        selected_ids.inner.device_ptr.cast(),
                        selected_weights.inner.device_ptr.cast(),
                        selected_count,
                        activated.inner.device_ptr.cast(),
                        bias.map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        residual
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_moe_down_aggregate_q8_1_f32",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_down_project_q8_1_selected4(
            &mut self,
            weights: &PlatformBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows: usize,
            columns: usize,
            selected_ids: &PlatformBuffer,
            selected_count: usize,
            activated_q8_1: &PlatformBuffer,
            bias: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            if selected_count > 4 {
                return Err(RuntimeError::Backend(String::from(
                    "cuda selected4 moe down projection requires selected_count <= 4",
                )));
            }
            let mode = match mode {
                QuantizationMode::GgmlQ8_0 => 0,
                QuantizationMode::GgmlMxfp4 => 1,
                other => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda selected4 moe down projection does not support {other:?}"
                    )));
                }
            };
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda selected4 moe down projection row stride exceeds c_int",
                ))
            })?;
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda selected4 moe down projection rows exceed c_int",
                ))
            })?;
            let columns = c_int::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda selected4 moe down projection columns exceed c_int",
                ))
            })?;
            let selected_count = c_int::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda selected4 moe down projection selected_count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_moe_down_project_q8_1_selected4(
                        weights.inner.device_ptr.cast(),
                        mode,
                        row_stride,
                        rows,
                        columns,
                        selected_ids.inner.device_ptr.cast(),
                        selected_count,
                        activated_q8_1.inner.device_ptr.cast(),
                        bias.map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_moe_down_project_q8_1_selected4",
            )
        }

        pub(super) fn encode_accumulate_selected4(
            &mut self,
            input: &PlatformBuffer,
            selected_weights: &PlatformBuffer,
            selected_count: usize,
            rows: usize,
            residual: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            if selected_count > 4 {
                return Err(RuntimeError::Backend(String::from(
                    "cuda selected4 accumulate requires selected_count <= 4",
                )));
            }
            let selected_count = c_int::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda selected4 accumulate selected_count exceeds c_int",
                ))
            })?;
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda selected4 accumulate rows exceed c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_accumulate_selected4(
                        input.inner.device_ptr.cast(),
                        selected_weights.inner.device_ptr.cast(),
                        selected_count,
                        rows,
                        residual
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_accumulate_selected4",
            )
        }

        pub(super) fn commit(
            mut self,
            wait: CudaCommandWait,
        ) -> Result<CudaCommandStatus, RuntimeError> {
            if self.capturing {
                return Err(RuntimeError::Backend(String::from(
                    "captured platform submissions must use commit_captured",
                )));
            }
            match wait {
                CudaCommandWait::Completed => {
                    self.runtime.set_device()?;
                    self.runtime.check(
                        unsafe { (self.runtime.cuda_stream_synchronize)(self.stream) },
                        "cudaStreamSynchronize",
                    )?;
                    self.status = CudaCommandStatus::Completed;
                }
            }
            Ok(self.status)
        }

        pub(super) fn commit_captured(
            mut self,
            wait: CudaCommandWait,
        ) -> Result<(CudaCommandStatus, PlatformGraphExec), RuntimeError> {
            if !self.capturing {
                return Err(RuntimeError::Backend(String::from(
                    "ordinary platform submissions must use commit",
                )));
            }
            self.runtime.set_device()?;
            let mut graph = std::ptr::null_mut();
            self.runtime.check(
                unsafe { (self.runtime.cuda_stream_end_capture)(self.stream, &mut graph) },
                "cudaStreamEndCapture",
            )?;
            let mut instance = std::ptr::null_mut();
            self.runtime.check(
                unsafe { (self.runtime.cuda_graph_instantiate)(&mut instance, graph, 0) },
                "cudaGraphInstantiate",
            )?;
            self.runtime.check(
                unsafe { (self.runtime.cuda_graph_launch)(instance, self.stream) },
                "cudaGraphLaunch",
            )?;
            match wait {
                CudaCommandWait::Completed => {
                    self.runtime.check(
                        unsafe { (self.runtime.cuda_stream_synchronize)(self.stream) },
                        "cudaStreamSynchronize",
                    )?;
                    self.status = CudaCommandStatus::Completed;
                }
            }
            self.capturing = false;
            Ok((
                self.status,
                PlatformGraphExec {
                    runtime: Arc::clone(&self.runtime),
                    graph,
                    instance,
                    stream: self.stream,
                },
            ))
        }
    }

    impl PlatformGraphExec {
        pub(super) fn launch(
            &self,
            wait: CudaCommandWait,
        ) -> Result<CudaCommandStatus, RuntimeError> {
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe { (self.runtime.cuda_graph_launch)(self.instance, self.stream) },
                "cudaGraphLaunch",
            )?;
            match wait {
                CudaCommandWait::Completed => {
                    self.runtime.check(
                        unsafe { (self.runtime.cuda_stream_synchronize)(self.stream) },
                        "cudaStreamSynchronize",
                    )?;
                    Ok(CudaCommandStatus::Completed)
                }
            }
        }
    }

    impl Drop for PlatformSubmission {
        fn drop(&mut self) {
            if self.owned_stream && !self.stream.is_null() {
                let _ = self.runtime.set_device();
                let _ = self.runtime.check(
                    unsafe { (self.runtime.cuda_stream_destroy)(self.stream) },
                    "cudaStreamDestroy",
                );
                self.stream = std::ptr::null_mut();
            }
        }
    }

    impl Drop for PlatformGraphExec {
        fn drop(&mut self) {
            let _ = self.runtime.set_device();
            if !self.instance.is_null() {
                let _ = self.runtime.check(
                    unsafe { (self.runtime.cuda_graph_exec_destroy)(self.instance) },
                    "cudaGraphExecDestroy",
                );
                self.instance = std::ptr::null_mut();
            }
            if !self.graph.is_null() {
                let _ = self.runtime.check(
                    unsafe { (self.runtime.cuda_graph_destroy)(self.graph) },
                    "cudaGraphDestroy",
                );
                self.graph = std::ptr::null_mut();
            }
        }
    }

    impl Drop for PlatformBufferInner {
        fn drop(&mut self) {
            if self.allocation_owner.is_some() {
                return;
            }
            if !self.device_ptr.is_null() {
                if let Some(pool_return) = &self.pool_return {
                    let cached = pool_return
                        .allocator_pool
                        .lock()
                        .expect("cuda allocator pool mutex should not be poisoned")
                        .checkin(
                            pool_return.cache_key.clone(),
                            pool_return.byte_len,
                            self.device_ptr as usize,
                        );
                    if cached {
                        self.device_ptr = std::ptr::null_mut();
                        return;
                    }
                }
                let _ = self.runtime.set_device();
                let freed = self
                    .runtime
                    .check(
                        unsafe { (self.runtime.cuda_free)(self.device_ptr) },
                        "cudaFree",
                    )
                    .is_ok();
                if freed {
                    if let Some(pool_return) = &self.pool_return {
                        pool_return
                            .allocator_pool
                            .lock()
                            .expect("cuda allocator pool mutex should not be poisoned")
                            .record_device_free(pool_return.byte_len);
                    }
                }
                self.device_ptr = std::ptr::null_mut();
            }
        }
    }

    impl Drop for PlatformHostBufferInner {
        fn drop(&mut self) {
            if !self.host_ptr.is_null() {
                let _ = self.runtime.set_device();
                let _ = self.runtime.check(
                    unsafe { (self.runtime.cuda_free_host)(self.host_ptr) },
                    "cudaFreeHost",
                );
                self.host_ptr = std::ptr::null_mut();
            }
        }
    }

    impl CudaRuntime {
        fn load(ordinal: u16) -> Result<Arc<Self>, RuntimeError> {
            let cudart_library = load_cudart_library()?;
            let cublas_library = load_cublas_library()?;
            let mut runtime = Self {
                ordinal,
                cuda_get_error_string: unsafe {
                    load_symbol(&cudart_library, b"cudaGetErrorString\0")?
                },
                cuda_set_device: unsafe { load_symbol(&cudart_library, b"cudaSetDevice\0")? },
                cuda_malloc: unsafe { load_symbol(&cudart_library, b"cudaMalloc\0")? },
                cuda_malloc_host: unsafe { load_symbol(&cudart_library, b"cudaMallocHost\0")? },
                cuda_mem_get_info: unsafe { load_symbol(&cudart_library, b"cudaMemGetInfo\0")? },
                cuda_free: unsafe { load_symbol(&cudart_library, b"cudaFree\0")? },
                cuda_free_host: unsafe { load_symbol(&cudart_library, b"cudaFreeHost\0")? },
                cuda_memcpy: unsafe { load_symbol(&cudart_library, b"cudaMemcpy\0")? },
                cuda_memcpy_async: unsafe { load_symbol(&cudart_library, b"cudaMemcpyAsync\0")? },
                cuda_memset_async: unsafe { load_symbol(&cudart_library, b"cudaMemsetAsync\0")? },
                cuda_stream_create: unsafe { load_symbol(&cudart_library, b"cudaStreamCreate\0")? },
                cuda_stream_destroy: unsafe {
                    load_symbol(&cudart_library, b"cudaStreamDestroy\0")?
                },
                cuda_stream_synchronize: unsafe {
                    load_symbol(&cudart_library, b"cudaStreamSynchronize\0")?
                },
                cuda_stream_begin_capture: unsafe {
                    load_symbol(&cudart_library, b"cudaStreamBeginCapture\0")?
                },
                cuda_stream_end_capture: unsafe {
                    load_symbol(&cudart_library, b"cudaStreamEndCapture\0")?
                },
                cuda_graph_instantiate: unsafe {
                    load_symbol(&cudart_library, b"cudaGraphInstantiate\0")?
                },
                cuda_graph_launch: unsafe { load_symbol(&cudart_library, b"cudaGraphLaunch\0")? },
                cuda_graph_exec_destroy: unsafe {
                    load_symbol(&cudart_library, b"cudaGraphExecDestroy\0")?
                },
                cuda_graph_destroy: unsafe { load_symbol(&cudart_library, b"cudaGraphDestroy\0")? },
                cuda_event_create: unsafe { load_symbol(&cudart_library, b"cudaEventCreate\0")? },
                cuda_event_destroy: unsafe { load_symbol(&cudart_library, b"cudaEventDestroy\0")? },
                cuda_event_record: unsafe { load_symbol(&cudart_library, b"cudaEventRecord\0")? },
                cuda_event_synchronize: unsafe {
                    load_symbol(&cudart_library, b"cudaEventSynchronize\0")?
                },
                cuda_event_elapsed_time: unsafe {
                    load_symbol(&cudart_library, b"cudaEventElapsedTime\0")?
                },
                cublas_handle: std::ptr::null_mut(),
                cublas_create: unsafe { load_symbol(&cublas_library, b"cublasCreate_v2\0")? },
                cublas_destroy: unsafe { load_symbol(&cublas_library, b"cublasDestroy_v2\0")? },
                cublas_set_stream: unsafe {
                    load_symbol(&cublas_library, b"cublasSetStream_v2\0")?
                },
                cublas_sgemm: unsafe { load_symbol(&cublas_library, b"cublasSgemm_v2\0")? },
                cublas_sgeam: unsafe { load_symbol(&cublas_library, b"cublasSgeam\0")? },
                cublas_gemm_ex: unsafe { load_symbol(&cublas_library, b"cublasGemmEx\0")? },
                cublas_gemm_strided_batched_ex: unsafe {
                    load_symbol(&cublas_library, b"cublasGemmStridedBatchedEx\0")?
                },
                cublaslt: load_cublaslt_context()?,
                _cudart_library: cudart_library,
                _cublas_library: cublas_library,
            };
            runtime.set_device()?;
            let mut handle = std::ptr::null_mut();
            runtime.check_cublas(
                unsafe { (runtime.cublas_create)(&mut handle) },
                "cublasCreate_v2",
            )?;
            runtime.cublas_handle = handle;
            Ok(Arc::new(runtime))
        }

        fn set_device(&self) -> Result<(), RuntimeError> {
            self.check(
                unsafe { (self.cuda_set_device)(i32::from(self.ordinal)) },
                "cudaSetDevice",
            )
        }

        fn device_memory_info(&self) -> Result<super::CudaDeviceMemoryInfo, RuntimeError> {
            self.set_device()?;
            let mut free_bytes = 0usize;
            let mut total_bytes = 0usize;
            self.check(
                unsafe { (self.cuda_mem_get_info)(&mut free_bytes, &mut total_bytes) },
                "cudaMemGetInfo",
            )?;
            Ok(super::CudaDeviceMemoryInfo {
                free_bytes: free_bytes.try_into().unwrap_or(u64::MAX),
                total_bytes: total_bytes.try_into().unwrap_or(u64::MAX),
            })
        }

        fn bind_stream(&self, stream: CudaStream) -> Result<(), RuntimeError> {
            self.set_device()?;
            self.check_cublas(
                unsafe { (self.cublas_set_stream)(self.cublas_handle, stream) },
                "cublasSetStream_v2",
            )
        }

        fn register_cublaslt_weight_scope(
            &self,
            allocation_identity: usize,
            scope: super::CudaGemmTuningScope,
        ) {
            let Some(context) = self.cublaslt.as_ref() else {
                return;
            };
            let mut context = context
                .lock()
                .expect("cuda cublasLt context mutex should not be poisoned");
            context.weight_scopes.insert(allocation_identity, scope);
        }

        fn cuda_gemm_tuning_report(&self) -> super::CudaGemmTuningReport {
            let Some(context) = self.cublaslt.as_ref() else {
                return super::CudaGemmTuningReport {
                    tuning_status: String::from("unavailable"),
                    plan_cache_scope: String::from(CUBLASLT_PLAN_CACHE_SCOPE),
                    selected_plan_count: 0,
                    tuned_shape_count: 0,
                    fallback_shape_count: 0,
                    max_workspace_bytes: 0,
                    selected_plans: Vec::new(),
                };
            };
            let context = context
                .lock()
                .expect("cuda cublasLt context mutex should not be poisoned");
            let tuning_status = if let Some(reason) = context.unavailable_reason.as_ref() {
                format!("unavailable:{reason}")
            } else if !context.plans.is_empty() && context.fallback_shapes.is_empty() {
                String::from("tuning_complete")
            } else if !context.plans.is_empty() {
                String::from("partial_tuning_with_fallbacks")
            } else if !context.fallback_shapes.is_empty() {
                String::from("fallback_only")
            } else {
                String::from("ready_no_plans")
            };
            super::CudaGemmTuningReport {
                tuning_status,
                plan_cache_scope: String::from(CUBLASLT_PLAN_CACHE_SCOPE),
                selected_plan_count: context.plans.len(),
                tuned_shape_count: context.plans.len(),
                fallback_shape_count: context.fallback_shapes.len(),
                max_workspace_bytes: context.workspace_size as u64,
                selected_plans: context
                    .plans
                    .values()
                    .map(|plan| super::CudaGemmTuningPlanReport {
                        model_family: plan.key.scope.model_family.clone(),
                        op_kind: plan.key.scope.op_kind.clone(),
                        rows: plan.key.rows,
                        inner: plan.key.inner,
                        cols: plan.key.cols,
                        input_dtype: String::from("f16"),
                        output_dtype: String::from("f32"),
                        backend_route: String::from(CUBLASLT_F16_TO_F32_BACKEND_ROUTE),
                        workspace_bytes: plan.workspace_size as u64,
                        mean_time_us: plan.mean_time_us,
                        algorithm_fingerprint: cublaslt_algo_fingerprint(plan.algo),
                    })
                    .collect(),
            }
        }

        fn maybe_execute_cublaslt_f16_to_f32(
            &self,
            stream: CudaStream,
            capturing: bool,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
            rows: usize,
            inner: usize,
            cols: usize,
        ) -> Result<bool, RuntimeError> {
            if rows == 0
                || inner == 0
                || cols == 0
                || rows > super::CUDA_CUBLASLT_F16_TO_F32_MAX_ROWS
            {
                return Ok(false);
            }
            let Some(context_mutex) = self.cublaslt.as_ref() else {
                return Ok(false);
            };
            let mut context = context_mutex
                .lock()
                .expect("cuda cublasLt context mutex should not be poisoned");
            if context.unavailable_reason.is_some() {
                return Ok(false);
            }
            let Some(scope) = context
                .weight_scopes
                .get(&right.allocation_identity())
                .cloned()
            else {
                return Ok(false);
            };
            let key = CublasLtPlanKey {
                scope,
                rows,
                inner,
                cols,
            };
            if !context.plans.contains_key(&key) {
                if capturing {
                    return Ok(false);
                }
                match self.tune_cublaslt_f16_to_f32_plan(
                    &mut context,
                    key.clone(),
                    stream,
                    left,
                    right,
                    output,
                ) {
                    Ok(plan) => {
                        context.plans.insert(key.clone(), plan);
                    }
                    Err(_) => {
                        context.fallback_shapes.insert(key);
                        return Ok(false);
                    }
                }
            }
            let execute_result = {
                let Some(plan) = context.plans.get(&key) else {
                    return Ok(false);
                };
                self.execute_cublaslt_f16_to_f32_plan(&context, plan, stream, left, right, output)
            };
            if execute_result.is_err() {
                context.fallback_shapes.insert(key);
                return Ok(false);
            }
            Ok(true)
        }

        fn tune_cublaslt_f16_to_f32_plan(
            &self,
            context: &mut CublasLtContext,
            key: CublasLtPlanKey,
            stream: CudaStream,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
        ) -> Result<CublasLtPlan, RuntimeError> {
            self.set_device()?;

            let mut desc = std::ptr::null_mut();
            self.check_cublas(
                unsafe {
                    (context.matmul_desc_create)(&mut desc, CUBLAS_COMPUTE_32F_FAST_16F, CUDA_R_32F)
                },
                "cublasLtMatmulDescCreate",
            )?;
            let desc = CublasLtMatmulDescGuard {
                raw: desc,
                destroy: context.matmul_desc_destroy,
            };
            let trans_a = CUBLAS_OP_T;
            let trans_b = CUBLAS_OP_N;
            self.check_cublas(
                unsafe {
                    (context.matmul_desc_set_attribute)(
                        desc.raw,
                        CUBLASLT_MATMUL_DESC_TRANSA,
                        (&trans_a as *const c_int).cast(),
                        std::mem::size_of_val(&trans_a),
                    )
                },
                "cublasLtMatmulDescSetAttribute(transa)",
            )?;
            self.check_cublas(
                unsafe {
                    (context.matmul_desc_set_attribute)(
                        desc.raw,
                        CUBLASLT_MATMUL_DESC_TRANSB,
                        (&trans_b as *const c_int).cast(),
                        std::mem::size_of_val(&trans_b),
                    )
                },
                "cublasLtMatmulDescSetAttribute(transb)",
            )?;

            let mut right_layout = std::ptr::null_mut();
            self.check_cublas(
                unsafe {
                    (context.matrix_layout_create)(
                        &mut right_layout,
                        CUDA_R_16F,
                        key.inner as u64,
                        key.cols as u64,
                        key.inner as i64,
                    )
                },
                "cublasLtMatrixLayoutCreate(right)",
            )?;
            let right_layout = CublasLtMatrixLayoutGuard {
                raw: right_layout,
                destroy: context.matrix_layout_destroy,
            };

            let mut left_layout = std::ptr::null_mut();
            self.check_cublas(
                unsafe {
                    (context.matrix_layout_create)(
                        &mut left_layout,
                        CUDA_R_16F,
                        key.inner as u64,
                        key.rows as u64,
                        key.inner as i64,
                    )
                },
                "cublasLtMatrixLayoutCreate(left)",
            )?;
            let left_layout = CublasLtMatrixLayoutGuard {
                raw: left_layout,
                destroy: context.matrix_layout_destroy,
            };

            let mut output_layout = std::ptr::null_mut();
            self.check_cublas(
                unsafe {
                    (context.matrix_layout_create)(
                        &mut output_layout,
                        CUDA_R_32F,
                        key.cols as u64,
                        key.rows as u64,
                        key.cols as i64,
                    )
                },
                "cublasLtMatrixLayoutCreate(output)",
            )?;
            let output_layout = CublasLtMatrixLayoutGuard {
                raw: output_layout,
                destroy: context.matrix_layout_destroy,
            };

            let mut preference = std::ptr::null_mut();
            self.check_cublas(
                unsafe { (context.matmul_preference_create)(&mut preference) },
                "cublasLtMatmulPreferenceCreate",
            )?;
            let preference = CublasLtMatmulPreferenceGuard {
                raw: preference,
                destroy: context.matmul_preference_destroy,
            };
            let workspace_limit = CUBLASLT_DEFAULT_MAX_WORKSPACE_BYTES;
            self.check_cublas(
                unsafe {
                    (context.matmul_preference_set_attribute)(
                        preference.raw,
                        CUBLASLT_MATMUL_PREF_MAX_WORKSPACE_BYTES,
                        (&workspace_limit as *const usize).cast(),
                        std::mem::size_of_val(&workspace_limit),
                    )
                },
                "cublasLtMatmulPreferenceSetAttribute(max_workspace)",
            )?;

            let mut heuristics = vec![
                // Safety: `cublasLtMatmulAlgoGetHeuristic` writes the returned entries before
                // Psionic reads them, and we only inspect the prefix reported through `returned`.
                unsafe { std::mem::zeroed::<CublasLtMatmulHeuristicResult>() };
                CUBLASLT_MAX_CANDIDATES
            ];
            let mut returned = 0_i32;
            self.check_cublas(
                unsafe {
                    (context.matmul_algo_get_heuristic)(
                        context.handle,
                        desc.raw,
                        right_layout.raw,
                        left_layout.raw,
                        output_layout.raw,
                        output_layout.raw,
                        preference.raw,
                        c_int::try_from(CUBLASLT_MAX_CANDIDATES).unwrap_or(c_int::MAX),
                        heuristics.as_mut_ptr(),
                        &mut returned,
                    )
                },
                "cublasLtMatmulAlgoGetHeuristic",
            )?;
            if returned <= 0 {
                return Err(RuntimeError::Backend(String::from(
                    "cublasLt returned no admitted heuristic algorithms",
                )));
            }

            let alpha = 1.0_f32;
            let beta = 0.0_f32;
            let alpha_ptr = (&alpha as *const f32).cast::<c_void>();
            let beta_ptr = (&beta as *const f32).cast::<c_void>();
            let mut best: Option<(CublasLtMatmulAlgo, usize, u64)> = None;
            for heuristic in heuristics
                .into_iter()
                .take(returned.max(0) as usize)
                .filter(|heuristic| heuristic.state == CUBLAS_STATUS_SUCCESS)
                .filter(|heuristic| {
                    heuristic.workspace_size <= CUBLASLT_DEFAULT_MAX_WORKSPACE_BYTES
                })
            {
                self.ensure_cublaslt_workspace(context, heuristic.workspace_size)?;
                let workspace_ptr = if heuristic.workspace_size == 0 {
                    std::ptr::null_mut()
                } else {
                    context.workspace_ptr
                };
                let mut admitted = true;
                for _ in 0..CUBLASLT_WARMUP_ITERS {
                    if unsafe {
                        (context.matmul)(
                            context.handle,
                            desc.raw,
                            alpha_ptr,
                            right.inner.device_ptr,
                            right_layout.raw,
                            left.inner.device_ptr,
                            left_layout.raw,
                            beta_ptr,
                            output.inner.device_ptr,
                            output_layout.raw,
                            output.inner.device_ptr,
                            output_layout.raw,
                            &heuristic.algo,
                            workspace_ptr,
                            heuristic.workspace_size,
                            stream,
                        )
                    } != CUBLAS_STATUS_SUCCESS
                    {
                        admitted = false;
                        break;
                    }
                }
                if !admitted {
                    continue;
                }
                let start = CudaEventGuard::new(self)?;
                let stop = CudaEventGuard::new(self)?;
                start.record(stream)?;
                for _ in 0..CUBLASLT_BENCH_ITERS {
                    self.check_cublas(
                        unsafe {
                            (context.matmul)(
                                context.handle,
                                desc.raw,
                                alpha_ptr,
                                right.inner.device_ptr,
                                right_layout.raw,
                                left.inner.device_ptr,
                                left_layout.raw,
                                beta_ptr,
                                output.inner.device_ptr,
                                output_layout.raw,
                                output.inner.device_ptr,
                                output_layout.raw,
                                &heuristic.algo,
                                workspace_ptr,
                                heuristic.workspace_size,
                                stream,
                            )
                        },
                        "cublasLtMatmul(benchmark)",
                    )?;
                }
                stop.record(stream)?;
                stop.synchronize()?;
                let mean_time_us = ((stop.elapsed_ms(&start)? * 1000.0)
                    / CUBLASLT_BENCH_ITERS as f32)
                    .round() as u64;
                if best
                    .as_ref()
                    .map(|(_, _, current)| mean_time_us < *current)
                    .unwrap_or(true)
                {
                    best = Some((heuristic.algo, heuristic.workspace_size, mean_time_us));
                }
            }

            let Some((algo, workspace_size, mean_time_us)) = best else {
                return Err(RuntimeError::Backend(String::from(
                    "cublasLt admitted no benchmark-stable algorithms for the requested shape",
                )));
            };
            Ok(CublasLtPlan {
                key,
                desc: desc.into_raw(),
                right_layout: right_layout.into_raw(),
                left_layout: left_layout.into_raw(),
                output_layout: output_layout.into_raw(),
                algo,
                workspace_size,
                mean_time_us,
            })
        }

        fn ensure_cublaslt_workspace(
            &self,
            context: &mut CublasLtContext,
            workspace_size: usize,
        ) -> Result<(), RuntimeError> {
            if workspace_size <= context.workspace_size {
                return Ok(());
            }
            if !context.workspace_ptr.is_null() {
                self.check(
                    unsafe { (self.cuda_free)(context.workspace_ptr) },
                    "cudaFree",
                )?;
                context.workspace_ptr = std::ptr::null_mut();
                context.workspace_size = 0;
            }
            let mut workspace_ptr = std::ptr::null_mut();
            self.check(
                unsafe { (self.cuda_malloc)(&mut workspace_ptr, workspace_size.max(1)) },
                "cudaMalloc",
            )?;
            context.workspace_ptr = workspace_ptr;
            context.workspace_size = workspace_size;
            Ok(())
        }

        fn execute_cublaslt_f16_to_f32_plan(
            &self,
            context: &CublasLtContext,
            plan: &CublasLtPlan,
            stream: CudaStream,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let alpha = 1.0_f32;
            let beta = 0.0_f32;
            let alpha_ptr = (&alpha as *const f32).cast::<c_void>();
            let beta_ptr = (&beta as *const f32).cast::<c_void>();
            let workspace_ptr = if plan.workspace_size == 0 {
                std::ptr::null_mut()
            } else {
                context.workspace_ptr
            };
            self.check_cublas(
                unsafe {
                    (context.matmul)(
                        context.handle,
                        plan.desc,
                        alpha_ptr,
                        right.inner.device_ptr,
                        plan.right_layout,
                        left.inner.device_ptr,
                        plan.left_layout,
                        beta_ptr,
                        output.inner.device_ptr,
                        plan.output_layout,
                        output.inner.device_ptr,
                        plan.output_layout,
                        &plan.algo,
                        workspace_ptr,
                        plan.workspace_size,
                        stream,
                    )
                },
                "cublasLtMatmul",
            )
        }

        fn check(&self, code: CudaError, operation: &str) -> Result<(), RuntimeError> {
            if code == CUDA_SUCCESS {
                return Ok(());
            }
            let message = unsafe {
                let error_ptr = (self.cuda_get_error_string)(code);
                if error_ptr.is_null() {
                    None
                } else {
                    Some(CStr::from_ptr(error_ptr).to_string_lossy().into_owned())
                }
            }
            .unwrap_or_else(|| format!("CUDA error code {code}"));
            Err(RuntimeError::Backend(format!(
                "{operation} failed: {message}"
            )))
        }

        fn check_cublas(&self, status: CublasStatus, operation: &str) -> Result<(), RuntimeError> {
            if status == CUBLAS_STATUS_SUCCESS {
                return Ok(());
            }
            Err(RuntimeError::Backend(format!(
                "{operation} failed with cuBLAS status {status}"
            )))
        }
    }

    impl Drop for CudaRuntime {
        fn drop(&mut self) {
            if let Some(context) = self.cublaslt.as_ref() {
                if let Ok(mut context) = context.lock() {
                    if !context.workspace_ptr.is_null() {
                        let _ = self.set_device();
                        let _ = self.check(
                            unsafe { (self.cuda_free)(context.workspace_ptr) },
                            "cudaFree",
                        );
                        context.workspace_ptr = std::ptr::null_mut();
                        context.workspace_size = 0;
                    }
                }
            }
            if !self.cublas_handle.is_null() {
                let _ = self.set_device();
                let _ = self.check_cublas(
                    unsafe { (self.cublas_destroy)(self.cublas_handle) },
                    "cublasDestroy_v2",
                );
                self.cublas_handle = std::ptr::null_mut();
            }
        }
    }

    pub(super) fn configure_backend(
        descriptor: super::DeviceDescriptor,
        allocator_pool_policy: AllocatorPoolPolicy,
    ) -> Result<ConfiguredBackend, RuntimeError> {
        let runtime = CudaRuntime::load(descriptor.device.ordinal())?;
        runtime.set_device()?;
        let mut stream = std::ptr::null_mut();
        runtime.check(
            unsafe { (runtime.cuda_stream_create)(&mut stream) },
            "cudaStreamCreate",
        )?;
        Ok(ConfiguredBackend {
            runtime,
            stream,
            allocator_pool: Arc::new(Mutex::new(CudaAllocatorPool::new(allocator_pool_policy))),
        })
    }

    impl Drop for ConfiguredBackend {
        fn drop(&mut self) {
            if !self.stream.is_null() {
                let _ = self.runtime.set_device();
                let _ = self.runtime.check(
                    unsafe { (self.runtime.cuda_stream_destroy)(self.stream) },
                    "cudaStreamDestroy",
                );
                self.stream = std::ptr::null_mut();
            }
        }
    }

    unsafe fn load_symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, RuntimeError> {
        unsafe { library.get::<T>(name) }
            .map(|symbol| *symbol)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "failed to load CUDA runtime symbol {}: {error}",
                    String::from_utf8_lossy(name).trim_end_matches('\0')
                ))
            })
    }

    fn load_cublaslt_context() -> Result<Option<Mutex<CublasLtContext>>, RuntimeError> {
        let library = match load_library(
            &["libcublasLt.so.13", "libcublasLt.so"],
            "failed to load libcublasLt.so.13 or libcublasLt.so",
        ) {
            Ok(library) => library,
            Err(_) => return Ok(None),
        };
        let create: CublasLtCreate = match unsafe { load_symbol(&library, b"cublasLtCreate\0") } {
            Ok(symbol) => symbol,
            Err(_) => return Ok(None),
        };
        let destroy: CublasLtDestroy = match unsafe { load_symbol(&library, b"cublasLtDestroy\0") }
        {
            Ok(symbol) => symbol,
            Err(_) => return Ok(None),
        };
        let matmul_desc_create: CublasLtMatmulDescCreate =
            match unsafe { load_symbol(&library, b"cublasLtMatmulDescCreate\0") } {
                Ok(symbol) => symbol,
                Err(_) => return Ok(None),
            };
        let matmul_desc_destroy: CublasLtMatmulDescDestroy =
            match unsafe { load_symbol(&library, b"cublasLtMatmulDescDestroy\0") } {
                Ok(symbol) => symbol,
                Err(_) => return Ok(None),
            };
        let matmul_desc_set_attribute: CublasLtMatmulDescSetAttribute =
            match unsafe { load_symbol(&library, b"cublasLtMatmulDescSetAttribute\0") } {
                Ok(symbol) => symbol,
                Err(_) => return Ok(None),
            };
        let matrix_layout_create: CublasLtMatrixLayoutCreate =
            match unsafe { load_symbol(&library, b"cublasLtMatrixLayoutCreate\0") } {
                Ok(symbol) => symbol,
                Err(_) => return Ok(None),
            };
        let matrix_layout_destroy: CublasLtMatrixLayoutDestroy =
            match unsafe { load_symbol(&library, b"cublasLtMatrixLayoutDestroy\0") } {
                Ok(symbol) => symbol,
                Err(_) => return Ok(None),
            };
        let matmul_preference_create: CublasLtMatmulPreferenceCreate =
            match unsafe { load_symbol(&library, b"cublasLtMatmulPreferenceCreate\0") } {
                Ok(symbol) => symbol,
                Err(_) => return Ok(None),
            };
        let matmul_preference_destroy: CublasLtMatmulPreferenceDestroy =
            match unsafe { load_symbol(&library, b"cublasLtMatmulPreferenceDestroy\0") } {
                Ok(symbol) => symbol,
                Err(_) => return Ok(None),
            };
        let matmul_preference_set_attribute: CublasLtMatmulPreferenceSetAttribute =
            match unsafe { load_symbol(&library, b"cublasLtMatmulPreferenceSetAttribute\0") } {
                Ok(symbol) => symbol,
                Err(_) => return Ok(None),
            };
        let matmul_algo_get_heuristic: CublasLtMatmulAlgoGetHeuristic =
            match unsafe { load_symbol(&library, b"cublasLtMatmulAlgoGetHeuristic\0") } {
                Ok(symbol) => symbol,
                Err(_) => return Ok(None),
            };
        let matmul: CublasLtMatmul = match unsafe { load_symbol(&library, b"cublasLtMatmul\0") } {
            Ok(symbol) => symbol,
            Err(_) => return Ok(None),
        };
        let mut handle = std::ptr::null_mut();
        if unsafe { (create)(&mut handle) } != CUBLAS_STATUS_SUCCESS {
            return Ok(None);
        }
        Ok(Some(Mutex::new(CublasLtContext {
            _library: library,
            handle,
            destroy,
            matmul_desc_create,
            matmul_desc_destroy,
            matmul_desc_set_attribute,
            matrix_layout_create,
            matrix_layout_destroy,
            matmul_preference_create,
            matmul_preference_destroy,
            matmul_preference_set_attribute,
            matmul_algo_get_heuristic,
            matmul,
            workspace_ptr: std::ptr::null_mut(),
            workspace_size: 0,
            weight_scopes: BTreeMap::new(),
            plans: BTreeMap::new(),
            fallback_shapes: BTreeSet::new(),
            unavailable_reason: None,
        })))
    }

    fn cublaslt_algo_fingerprint(algo: CublasLtMatmulAlgo) -> String {
        algo.data
            .iter()
            .map(|word| format!("{word:016x}"))
            .collect::<Vec<_>>()
            .join("")
    }

    fn load_cudart_library() -> Result<Library, RuntimeError> {
        load_library(
            &["libcudart.so.13", "libcudart.so"],
            "failed to load libcudart.so.13 or libcudart.so",
        )
    }

    fn load_cublas_library() -> Result<Library, RuntimeError> {
        load_library(
            &["libcublas.so.13", "libcublas.so"],
            "failed to load libcublas.so.13 or libcublas.so",
        )
    }

    fn load_library(candidates: &[&str], failure: &str) -> Result<Library, RuntimeError> {
        for candidate in candidates {
            let library = unsafe { Library::new(candidate) };
            if let Ok(library) = library {
                return Ok(library);
            }
        }
        Err(RuntimeError::Backend(String::from(failure)))
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::{
        CudaAllocatorPoolTelemetry, CudaCommandStatus, CudaCommandWait, CudaDeviceMemoryInfo,
        QuantizationMode,
    };
    use psionic_core::TensorSpec;
    use psionic_runtime::{AllocatorPoolPolicy, AllocatorPoolReport, RuntimeError};

    #[derive(Clone)]
    pub(super) struct PlatformBuffer;

    #[derive(Clone)]
    pub(super) struct PlatformHostBuffer;

    pub(super) struct PlatformGraphExec;

    pub(super) struct PlatformSubmission;

    pub(super) struct ConfiguredBackend;

    impl ConfiguredBackend {
        pub(super) fn allocator_pool_report(&self) -> AllocatorPoolReport {
            AllocatorPoolReport {
                policy: AllocatorPoolPolicy::disabled(),
                state: Default::default(),
            }
        }

        pub(super) fn allocator_pool_telemetry(&self) -> CudaAllocatorPoolTelemetry {
            CudaAllocatorPoolTelemetry::default()
        }

        pub(super) fn device_memory_info(&self) -> Result<CudaDeviceMemoryInfo, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn cuda_gemm_tuning_report(&self) -> super::CudaGemmTuningReport {
            super::CudaGemmTuningReport {
                tuning_status: String::from("unavailable"),
                plan_cache_scope: String::from("per_device_runtime_owner"),
                selected_plan_count: 0,
                tuned_shape_count: 0,
                fallback_shape_count: 0,
                max_workspace_bytes: 0,
                selected_plans: Vec::new(),
            }
        }

        pub(super) fn register_cublaslt_weight_scope(
            &self,
            _allocation_identity: usize,
            _scope: super::CudaGemmTuningScope,
        ) {
        }

        pub(super) fn allocate(
            &self,
            _spec: &TensorSpec,
            _byte_len: usize,
        ) -> Result<PlatformBuffer, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn allocate_host(
            &self,
            _byte_len: usize,
        ) -> Result<PlatformHostBuffer, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn begin_submission(&self) -> Result<PlatformSubmission, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn begin_capture_submission(&self) -> Result<PlatformSubmission, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn top_k_f32_one_row_radix_sort_temp_storage_bytes(
            &self,
            _column_count: usize,
        ) -> Result<usize, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }
    }

    pub(super) fn quantized_kernels_compiled() -> bool {
        false
    }

    impl PlatformBuffer {
        pub(super) fn allocation_identity(&self) -> usize {
            0
        }

        pub(super) fn sub_buffer(&self, _byte_offset: usize) -> Result<Self, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn write_bytes(&self, _bytes: &[u8]) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn write_bytes_at_offset(
            &self,
            _byte_offset: usize,
            _bytes: &[u8],
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn read_bytes(&self, _byte_len: usize) -> Result<Vec<u8>, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn read_bytes_at_offset(
            &self,
            _byte_offset: usize,
            _byte_len: usize,
        ) -> Result<Vec<u8>, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }
    }

    impl PlatformHostBuffer {
        pub(super) fn write_bytes(&self, _bytes: &[u8]) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn read_bytes(&self, _byte_len: usize) -> Result<Vec<u8>, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn read_bytes_prefix_into(
            &self,
            _output: &mut [u8],
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }
    }

    impl PlatformSubmission {
        pub(super) fn allocate(&self, _byte_len: usize) -> Result<PlatformBuffer, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn fill_buffer(
            &mut self,
            _buffer: &PlatformBuffer,
            _byte_len: usize,
            _value: u8,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn copy_buffer(
            &mut self,
            _source: &PlatformBuffer,
            _destination: &PlatformBuffer,
            _byte_len: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn copy_host_to_device(
            &mut self,
            _source: &PlatformHostBuffer,
            _destination: &PlatformBuffer,
            _byte_len: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn copy_device_to_host(
            &mut self,
            _source: &PlatformBuffer,
            _destination: &PlatformHostBuffer,
            _byte_len: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn copy_buffer_region(
            &mut self,
            _source: &PlatformBuffer,
            _source_byte_offset: usize,
            _destination: &PlatformBuffer,
            _destination_byte_offset: usize,
            _byte_len: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn encode_add(
            &mut self,
            _left: &PlatformBuffer,
            _right: &PlatformBuffer,
            _output: &PlatformBuffer,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn encode_mul(
            &mut self,
            _left: &PlatformBuffer,
            _right: &PlatformBuffer,
            _output: &PlatformBuffer,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn encode_matmul(
            &mut self,
            _left: &PlatformBuffer,
            _right: &PlatformBuffer,
            _output: &PlatformBuffer,
            _rows: usize,
            _inner: usize,
            _cols: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn encode_matmul_f16_to_f32(
            &mut self,
            _left: &PlatformBuffer,
            _right: &PlatformBuffer,
            _output: &PlatformBuffer,
            _rows: usize,
            _inner: usize,
            _cols: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn encode_matmul_bf16_to_f32(
            &mut self,
            _left: &PlatformBuffer,
            _right: &PlatformBuffer,
            _output: &PlatformBuffer,
            _rows: usize,
            _inner: usize,
            _cols: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn encode_matmul_f32_bf16_to_f32(
            &mut self,
            _left: &PlatformBuffer,
            _right: &PlatformBuffer,
            _output: &PlatformBuffer,
            _rows: usize,
            _inner: usize,
            _cols: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn encode_matmul_bf16_strided_batched_to_f32(
            &mut self,
            _left: &PlatformBuffer,
            _right: &PlatformBuffer,
            _output: &PlatformBuffer,
            _rows: usize,
            _inner: usize,
            _cols: usize,
            _batch_count: usize,
            _transpose_left: bool,
            _transpose_right: bool,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn encode_matmul_bf16_strided_batched_to_f32_with_strides(
            &mut self,
            _left: &PlatformBuffer,
            _right: &PlatformBuffer,
            _output: &PlatformBuffer,
            _rows: usize,
            _inner: usize,
            _cols: usize,
            _batch_count: usize,
            _left_stride_elements: usize,
            _right_stride_elements: usize,
            _output_stride_elements: usize,
            _transpose_left: bool,
            _transpose_right: bool,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn encode_quantized_matvec(
            &mut self,
            _weights: &PlatformBuffer,
            _byte_offset: usize,
            _mode: QuantizationMode,
            _rows: usize,
            _cols: usize,
            _input: &PlatformBuffer,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_quantize_f32_to_q8_1(
            &mut self,
            _input: &PlatformBuffer,
            _rows: usize,
            _cols: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_dequantize_row_to_f32(
            &mut self,
            _weights: &PlatformBuffer,
            _mode: QuantizationMode,
            _rows: usize,
            _row_stride: usize,
            _cols: usize,
            _decode_params: &PlatformBuffer,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_gather_f16_row_to_f32(
            &mut self,
            _input: &PlatformBuffer,
            _rows: usize,
            _cols: usize,
            _decode_params: &PlatformBuffer,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_gather_f32_by_indices(
            &mut self,
            _input: &PlatformBuffer,
            _input_len: usize,
            _indices: &PlatformBuffer,
            _index_count: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_cast_f32_to_f16(
            &mut self,
            _input: &PlatformBuffer,
            _output: &PlatformBuffer,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_cast_f32_to_bf16(
            &mut self,
            _input: &PlatformBuffer,
            _output: &PlatformBuffer,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_cast_bf16_to_f32(
            &mut self,
            _input: &PlatformBuffer,
            _output: &PlatformBuffer,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_quantized_matvec_q8_1(
            &mut self,
            _weights: &PlatformBuffer,
            _byte_offset: usize,
            _mode: QuantizationMode,
            _rows: usize,
            _cols: usize,
            _input_q8_1: &PlatformBuffer,
            _bias: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_quantized_matvec_q8_1_argmax(
            &mut self,
            _weights: &PlatformBuffer,
            _byte_offset: usize,
            _mode: QuantizationMode,
            _rows: usize,
            _cols: usize,
            _input_q8_1: &PlatformBuffer,
            _bias: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_argmax_f32(
            &mut self,
            _input: &PlatformBuffer,
            _rows: usize,
            _cols: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_top_k_f32(
            &mut self,
            _input: &PlatformBuffer,
            _rows: usize,
            _cols: usize,
            _top_k: usize,
            _selected_indices: &PlatformBuffer,
            _selected_values: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_top_k_f32_one_row_partitioned(
            &mut self,
            _input: &PlatformBuffer,
            _cols: usize,
            _top_k: usize,
            _partial_block_count: usize,
            _partial_indices: &PlatformBuffer,
            _partial_values: &PlatformBuffer,
            _selected_indices: &PlatformBuffer,
            _selected_values: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_apply_sampling_penalties_f32_sparse(
            &mut self,
            _logits: &PlatformBuffer,
            _vocab_size: usize,
            _token_ids: &PlatformBuffer,
            _token_counts: &PlatformBuffer,
            _active_token_count: usize,
            _repeat_penalty: f32,
            _presence_penalty: f32,
            _frequency_penalty: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_top_k_f32_one_row_radix_sort(
            &mut self,
            _input: &PlatformBuffer,
            _cols: usize,
            _top_k: usize,
            _input_indices: &PlatformBuffer,
            _sorted_values: &PlatformBuffer,
            _sorted_indices: &PlatformBuffer,
            _temp_storage: &PlatformBuffer,
            _temp_storage_bytes: usize,
            _selected_indices: &PlatformBuffer,
            _selected_values: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_rms_norm(
            &mut self,
            _input: &PlatformBuffer,
            _weight: &PlatformBuffer,
            _output: &PlatformBuffer,
            _element_count: usize,
            _feature_count: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_rms_norm_region(
            &mut self,
            _input: &PlatformBuffer,
            _input_offset: usize,
            _weight: &PlatformBuffer,
            _output: &PlatformBuffer,
            _output_offset: usize,
            _element_count: usize,
            _feature_count: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_parameter_golf_projection_loss(
            &mut self,
            _logits: &PlatformBuffer,
            _target_ids: &PlatformBuffer,
            _output: &PlatformBuffer,
            _row_count: usize,
            _vocab_size: usize,
            _logit_softcap: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_parameter_golf_projection_token_losses(
            &mut self,
            _logits: &PlatformBuffer,
            _target_ids: &PlatformBuffer,
            _output: &PlatformBuffer,
            _row_count: usize,
            _vocab_size: usize,
            _logit_softcap: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_parameter_golf_token_embedding_lookup(
            &mut self,
            _token_ids: &PlatformBuffer,
            _token_embedding: &PlatformBuffer,
            _row_count: usize,
            _vocab_size: usize,
            _width: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_parameter_golf_token_embedding_lookup_bf16_to_f32(
            &mut self,
            _token_ids: &PlatformBuffer,
            _token_embedding: &PlatformBuffer,
            _row_count: usize,
            _vocab_size: usize,
            _width: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_parameter_golf_token_embedding_lookup_backward(
            &mut self,
            _token_ids: &PlatformBuffer,
            _grad_output: &PlatformBuffer,
            _row_count: usize,
            _vocab_size: usize,
            _width: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_parameter_golf_projection_loss_backward(
            &mut self,
            _logits: &PlatformBuffer,
            _target_ids: &PlatformBuffer,
            _grad_output: &PlatformBuffer,
            _output: &PlatformBuffer,
            _row_count: usize,
            _vocab_size: usize,
            _logit_softcap: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_rms_norm_q8_1(
            &mut self,
            _input: &PlatformBuffer,
            _weight: &PlatformBuffer,
            _output_q8_1: &PlatformBuffer,
            _element_count: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_rms_norm_input_backward(
            &mut self,
            _input: &PlatformBuffer,
            _weight: &PlatformBuffer,
            _grad_output: &PlatformBuffer,
            _output: &PlatformBuffer,
            _element_count: usize,
            _feature_count: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_rms_norm_weight_backward(
            &mut self,
            _input: &PlatformBuffer,
            _grad_output: &PlatformBuffer,
            _output: &PlatformBuffer,
            _element_count: usize,
            _feature_count: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_add_residual_rms_norm(
            &mut self,
            _input: &PlatformBuffer,
            _residual: &PlatformBuffer,
            _input_bias: Option<&PlatformBuffer>,
            _weight: &PlatformBuffer,
            _summed_output: &PlatformBuffer,
            _normalized_output: &PlatformBuffer,
            _element_count: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_add_residual_rms_norm_q8_1(
            &mut self,
            _input: &PlatformBuffer,
            _residual: &PlatformBuffer,
            _input_bias: Option<&PlatformBuffer>,
            _weight: &PlatformBuffer,
            _summed_output: &PlatformBuffer,
            _normalized_output: &PlatformBuffer,
            _quantized_output: &PlatformBuffer,
            _element_count: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_add_residual_rms_norm_q8_1_router_topk(
            &mut self,
            _input: &PlatformBuffer,
            _residual: &PlatformBuffer,
            _input_bias: Option<&PlatformBuffer>,
            _weight: &PlatformBuffer,
            _summed_output: &PlatformBuffer,
            _normalized_output: &PlatformBuffer,
            _quantized_output: &PlatformBuffer,
            _router_weights: &PlatformBuffer,
            _router_bias: Option<&PlatformBuffer>,
            _expert_count: usize,
            _top_k: usize,
            _selected_ids: &PlatformBuffer,
            _selected_weights: &PlatformBuffer,
            _element_count: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_add_f32_in_place(
            &mut self,
            _destination: &PlatformBuffer,
            _element_offset: usize,
            _rhs: &PlatformBuffer,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_silu_mul_f32(
            &mut self,
            _activation_input: &PlatformBuffer,
            _activation_offset: usize,
            _rhs: &PlatformBuffer,
            _rhs_offset: usize,
            _element_count: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_silu_mul_q8_1(
            &mut self,
            _activation_input: &PlatformBuffer,
            _activation_offset: usize,
            _rhs: &PlatformBuffer,
            _rhs_offset: usize,
            _element_count: usize,
            _output_q8_1: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_sigmoid_mul_f32(
            &mut self,
            _values: &PlatformBuffer,
            _values_offset: usize,
            _gate: &PlatformBuffer,
            _gate_offset: usize,
            _element_count: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_sigmoid_mul_q8_1(
            &mut self,
            _values: &PlatformBuffer,
            _values_offset: usize,
            _gate: &PlatformBuffer,
            _gate_offset: usize,
            _element_count: usize,
            _output_q8_1: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_split_interleaved_query_gate_f32(
            &mut self,
            _input: &PlatformBuffer,
            _head_count: usize,
            _head_dim: usize,
            _query_output: &PlatformBuffer,
            _gate_output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_split_interleaved_query_gate_rms_norm_f32(
            &mut self,
            _input: &PlatformBuffer,
            _head_count: usize,
            _head_dim: usize,
            _weight: &PlatformBuffer,
            _epsilon: f32,
            _query_output: &PlatformBuffer,
            _gate_output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_pack_qwen35_key_value_rms_norm_f32(
            &mut self,
            _input: &PlatformBuffer,
            _key_offset: usize,
            _value_offset: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _weight: &PlatformBuffer,
            _epsilon: f32,
            _output: &PlatformBuffer,
            _output_key_offset: usize,
            _output_value_offset: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_pack_qwen35_hybrid_qkv_rms_norm_f32(
            &mut self,
            _input: &PlatformBuffer,
            _q_offset: usize,
            _k_offset: usize,
            _v_offset: usize,
            _group_count: usize,
            _state_size: usize,
            _v_size: usize,
            _q_weight: &PlatformBuffer,
            _k_weight: &PlatformBuffer,
            _epsilon: f32,
            _output: &PlatformBuffer,
            _output_q_offset: usize,
            _output_k_offset: usize,
            _output_v_offset: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_depthwise_causal_conv1d_step_f32(
            &mut self,
            _input: &PlatformBuffer,
            _state: &PlatformBuffer,
            _weights: &PlatformBuffer,
            _channels: usize,
            _kernel_size: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_depthwise_causal_conv1d_step_silu_f32(
            &mut self,
            _input: &PlatformBuffer,
            _state: &PlatformBuffer,
            _weights: &PlatformBuffer,
            _channels: usize,
            _kernel_size: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_qwen35_ssm_decay_beta_f32(
            &mut self,
            _input: &PlatformBuffer,
            _alpha_offset: usize,
            _beta_offset: usize,
            _ssm_a: &PlatformBuffer,
            _ssm_dt: &PlatformBuffer,
            _element_count: usize,
            _decay_output: &PlatformBuffer,
            _beta_output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_gated_delta_step_f32(
            &mut self,
            _qkv: &PlatformBuffer,
            _query_offset: usize,
            _key_offset: usize,
            _value_offset: usize,
            _decay: &PlatformBuffer,
            _beta: &PlatformBuffer,
            _state: &PlatformBuffer,
            _key_head_count: usize,
            _value_head_count: usize,
            _key_dim: usize,
            _value_dim: usize,
            _v_head_reordered: bool,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_rope_neox_in_place(
            &mut self,
            _values: &PlatformBuffer,
            _element_offset: usize,
            _head_count: usize,
            _head_dim: usize,
            _rotary_dim: usize,
            _position: usize,
            _freq_scale: f32,
            _ext_factor: f32,
            _corr_dims: [f32; 2],
            _theta_scale: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_rotary_embedding_backward(
            &mut self,
            _grad_output: &PlatformBuffer,
            _cos: &PlatformBuffer,
            _sin: &PlatformBuffer,
            _batch_size: usize,
            _head_count: usize,
            _sequence_length: usize,
            _head_dim: usize,
            _batched_tables: bool,
            _grad_input: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_permute_rank2_transpose_f32(
            &mut self,
            _input: &PlatformBuffer,
            _rows: usize,
            _cols: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_permute_rank4_swap_middle_axes_f32(
            &mut self,
            _input: &PlatformBuffer,
            _dim0: usize,
            _dim1: usize,
            _dim2: usize,
            _dim3: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_reduce_sum_rows_f32(
            &mut self,
            _input: &PlatformBuffer,
            _row_count: usize,
            _column_count: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_reduce_sum_axis0_f32(
            &mut self,
            _input: &PlatformBuffer,
            _axis0_extent: usize,
            _row_width: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_reduce_sum_axis1_rank3_f32(
            &mut self,
            _input: &PlatformBuffer,
            _dim0: usize,
            _dim1: usize,
            _dim2: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_expand_rank3_f32(
            &mut self,
            _input: &PlatformBuffer,
            _input_dims: [usize; 3],
            _output_dims: [usize; 3],
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_expand_rank4_f32(
            &mut self,
            _input: &PlatformBuffer,
            _input_dims: [usize; 4],
            _output_dims: [usize; 4],
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache(
            &mut self,
            _qkv: &PlatformBuffer,
            _query_offset: usize,
            _key_offset: usize,
            _value_offset: usize,
            _cache_keys: &PlatformBuffer,
            _cache_values: &PlatformBuffer,
            _cache_width: usize,
            _layer_offset: usize,
            _past_tokens: usize,
            _sliding_window: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _rotary_dim: usize,
            _position: usize,
            _freq_scale: f32,
            _ext_factor: f32,
            _corr_dims: [f32; 2],
            _theta_scale: f32,
            _attention_sinks: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv(
            &mut self,
            _qkv: &PlatformBuffer,
            _query_offset: usize,
            _key_offset: usize,
            _value_offset: usize,
            _cache_keys: &PlatformBuffer,
            _cache_values: &PlatformBuffer,
            _cache_width: usize,
            _layer_offset: usize,
            _past_tokens: usize,
            _sliding_window: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _rotary_dim: usize,
            _position: usize,
            _freq_scale: f32,
            _ext_factor: f32,
            _corr_dims: [f32; 2],
            _theta_scale: f32,
            _attention_sinks: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_mrope_cache_f16_kv(
            &mut self,
            _qkv: &PlatformBuffer,
            _query_offset: usize,
            _key_offset: usize,
            _value_offset: usize,
            _cache_keys: &PlatformBuffer,
            _cache_values: &PlatformBuffer,
            _cache_width: usize,
            _layer_offset: usize,
            _past_tokens: usize,
            _sliding_window: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _rotary_dim: usize,
            _positions: [usize; 3],
            _sections: [usize; 4],
            _interleaved: bool,
            _freq_scale: f32,
            _ext_factor: f32,
            _corr_dims: [f32; 2],
            _theta_scale: f32,
            _attention_sinks: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_turboquant_kv(
            &mut self,
            _qkv: &PlatformBuffer,
            _query_offset: usize,
            _key_offset: usize,
            _value_offset: usize,
            _cache_keys: &PlatformBuffer,
            _cache_values: &PlatformBuffer,
            _cache_width: usize,
            _layer_offset: usize,
            _past_tokens: usize,
            _sliding_window: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _rotary_dim: usize,
            _position: usize,
            _freq_scale: f32,
            _ext_factor: f32,
            _corr_dims: [f32; 2],
            _theta_scale: f32,
            _attention_sinks: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv_q8_1(
            &mut self,
            _qkv: &PlatformBuffer,
            _query_offset: usize,
            _key_offset: usize,
            _value_offset: usize,
            _cache_keys: &PlatformBuffer,
            _cache_values: &PlatformBuffer,
            _cache_width: usize,
            _layer_offset: usize,
            _past_tokens: usize,
            _sliding_window: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _rotary_dim: usize,
            _position: usize,
            _freq_scale: f32,
            _ext_factor: f32,
            _corr_dims: [f32; 2],
            _theta_scale: f32,
            _attention_sinks: Option<&PlatformBuffer>,
            _output_q8_1: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv_graph(
            &mut self,
            _qkv: &PlatformBuffer,
            _query_offset: usize,
            _key_offset: usize,
            _value_offset: usize,
            _cache_keys: &PlatformBuffer,
            _cache_values: &PlatformBuffer,
            _cache_width: usize,
            _layer_offset: usize,
            _decode_params: &PlatformBuffer,
            _sliding_window: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _rotary_dim: usize,
            _freq_scale: f32,
            _ext_factor: f32,
            _corr_dims: [f32; 2],
            _theta_scale: f32,
            _attention_sinks: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv_graph_fa3(
            &mut self,
            _qkv: &PlatformBuffer,
            _query_offset: usize,
            _key_offset: usize,
            _value_offset: usize,
            _cache_keys: &PlatformBuffer,
            _cache_values: &PlatformBuffer,
            _cache_width: usize,
            _layer_offset: usize,
            _decode_params: &PlatformBuffer,
            _sliding_window: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _rotary_dim: usize,
            _freq_scale: f32,
            _ext_factor: f32,
            _corr_dims: [f32; 2],
            _theta_scale: f32,
            _attention_sinks: Option<&PlatformBuffer>,
            _partial_output: &PlatformBuffer,
            _partial_max: &PlatformBuffer,
            _partial_sum: &PlatformBuffer,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_turboquant_kv_graph(
            &mut self,
            _qkv: &PlatformBuffer,
            _query_offset: usize,
            _key_offset: usize,
            _value_offset: usize,
            _cache_keys: &PlatformBuffer,
            _cache_values: &PlatformBuffer,
            _cache_width: usize,
            _layer_offset: usize,
            _decode_params: &PlatformBuffer,
            _sliding_window: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _rotary_dim: usize,
            _freq_scale: f32,
            _ext_factor: f32,
            _corr_dims: [f32; 2],
            _theta_scale: f32,
            _attention_sinks: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv_graph_q8_1(
            &mut self,
            _qkv: &PlatformBuffer,
            _query_offset: usize,
            _key_offset: usize,
            _value_offset: usize,
            _cache_keys: &PlatformBuffer,
            _cache_values: &PlatformBuffer,
            _cache_width: usize,
            _layer_offset: usize,
            _decode_params: &PlatformBuffer,
            _sliding_window: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _rotary_dim: usize,
            _freq_scale: f32,
            _ext_factor: f32,
            _corr_dims: [f32; 2],
            _theta_scale: f32,
            _attention_sinks: Option<&PlatformBuffer>,
            _output_q8_1: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode(
            &mut self,
            _query: &PlatformBuffer,
            _query_offset: usize,
            _current_key: &PlatformBuffer,
            _key_offset: usize,
            _current_value: &PlatformBuffer,
            _value_offset: usize,
            _cache_keys: &PlatformBuffer,
            _cache_values: &PlatformBuffer,
            _cache_width: usize,
            _layer_offset: usize,
            _past_tokens: usize,
            _sliding_window: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _attention_sinks: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_attention_causal_sequence_f32(
            &mut self,
            _query: &PlatformBuffer,
            _key: &PlatformBuffer,
            _value: &PlatformBuffer,
            _batch_size: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _sequence_length: usize,
            _head_dim: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_attention_causal_sequence_bf16(
            &mut self,
            _query: &PlatformBuffer,
            _key: &PlatformBuffer,
            _value: &PlatformBuffer,
            _batch_size: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _sequence_length: usize,
            _head_dim: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_attention_causal_row_softmax_in_place_f32(
            &mut self,
            _logits: &PlatformBuffer,
            _row_count: usize,
            _sequence_length: usize,
            _scale: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_attention_causal_row_softmax_backward_in_place_f32(
            &mut self,
            _probabilities: &PlatformBuffer,
            _grad_probabilities: &PlatformBuffer,
            _row_count: usize,
            _sequence_length: usize,
            _post_scale: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_causal_sequence_backward_f32(
            &mut self,
            _query: &PlatformBuffer,
            _key: &PlatformBuffer,
            _value: &PlatformBuffer,
            _grad_output: &PlatformBuffer,
            _batch_size: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _sequence_length: usize,
            _head_dim: usize,
            _query_gradient: &PlatformBuffer,
            _key_gradient: &PlatformBuffer,
            _value_gradient: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_causal_sequence_backward_bf16_to_f32(
            &mut self,
            _query: &PlatformBuffer,
            _key: &PlatformBuffer,
            _value: &PlatformBuffer,
            _grad_output: &PlatformBuffer,
            _batch_size: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _sequence_length: usize,
            _head_dim: usize,
            _query_gradient: &PlatformBuffer,
            _key_gradient: &PlatformBuffer,
            _value_gradient: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_router_topk_softmax(
            &mut self,
            _weights: &PlatformBuffer,
            _bias: Option<&PlatformBuffer>,
            _input: &PlatformBuffer,
            _expert_count: usize,
            _input_size: usize,
            _top_k: usize,
            _selected_ids: &PlatformBuffer,
            _selected_weights: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_router_topk_delayed_softmax(
            &mut self,
            _logits: &PlatformBuffer,
            _expert_count: usize,
            _top_k: usize,
            _selected_ids: &PlatformBuffer,
            _selected_weights: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_gate_up_swiglu(
            &mut self,
            _weights: &PlatformBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows_per_expert: usize,
            _columns: usize,
            _gate_rows: usize,
            _up_rows: usize,
            _selected_ids: &PlatformBuffer,
            _selected_count: usize,
            _input: &PlatformBuffer,
            _gate_bias: Option<&PlatformBuffer>,
            _up_bias: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_gate_up_swiglu_q8_1(
            &mut self,
            _weights: &PlatformBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows_per_expert: usize,
            _columns: usize,
            _gate_rows: usize,
            _up_rows: usize,
            _selected_ids: &PlatformBuffer,
            _selected_count: usize,
            _input_q8_1: &PlatformBuffer,
            _gate_bias: Option<&PlatformBuffer>,
            _up_bias: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_gate_up_swiglu_q8_1_selected4_quantized(
            &mut self,
            _weights: &PlatformBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows_per_expert: usize,
            _columns: usize,
            _gate_rows: usize,
            _up_rows: usize,
            _selected_ids: &PlatformBuffer,
            _selected_count: usize,
            _input_q8_1: &PlatformBuffer,
            _gate_bias: Option<&PlatformBuffer>,
            _up_bias: Option<&PlatformBuffer>,
            _output_q8_1: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_down_aggregate(
            &mut self,
            _weights: &PlatformBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows: usize,
            _columns: usize,
            _selected_ids: &PlatformBuffer,
            _selected_weights: &PlatformBuffer,
            _selected_count: usize,
            _activated: &PlatformBuffer,
            _bias: Option<&PlatformBuffer>,
            _residual: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_down_aggregate_q8_1(
            &mut self,
            _weights: &PlatformBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows: usize,
            _columns: usize,
            _selected_ids: &PlatformBuffer,
            _selected_weights: &PlatformBuffer,
            _selected_count: usize,
            _activated_q8_1: &PlatformBuffer,
            _bias: Option<&PlatformBuffer>,
            _residual: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_down_aggregate_q8_1_f32(
            &mut self,
            _weights: &PlatformBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows: usize,
            _columns: usize,
            _selected_ids: &PlatformBuffer,
            _selected_weights: &PlatformBuffer,
            _selected_count: usize,
            _activated: &PlatformBuffer,
            _bias: Option<&PlatformBuffer>,
            _residual: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_down_project_q8_1_selected4(
            &mut self,
            _weights: &PlatformBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows: usize,
            _columns: usize,
            _selected_ids: &PlatformBuffer,
            _selected_count: usize,
            _activated_q8_1: &PlatformBuffer,
            _bias: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_accumulate_selected4(
            &mut self,
            _input: &PlatformBuffer,
            _selected_weights: &PlatformBuffer,
            _selected_count: usize,
            _rows: usize,
            _residual: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn commit(
            self,
            _wait: CudaCommandWait,
        ) -> Result<CudaCommandStatus, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn commit_captured(
            self,
            _wait: CudaCommandWait,
        ) -> Result<(CudaCommandStatus, PlatformGraphExec), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }
    }

    impl PlatformGraphExec {
        pub(super) fn launch(
            &self,
            _wait: CudaCommandWait,
        ) -> Result<CudaCommandStatus, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }
    }

    pub(super) fn configure_backend(
        _descriptor: super::DeviceDescriptor,
        _allocator_pool_policy: AllocatorPoolPolicy,
    ) -> Result<ConfiguredBackend, RuntimeError> {
        Err(RuntimeError::Backend(String::from(
            "cuda runtime substrate currently requires Linux libcudart",
        )))
    }
}

#[cfg(test)]
mod tests {
    use half::bf16;
    use psionic_backend_cpu::CpuBackend;
    use psionic_backend_cpu::decode_quantized_row_into;
    use psionic_backend_cpu::quantized_row_dot;
    use psionic_backend_tests::{GraphBackendConformanceHarness, run_graph_backend_conformance};
    use psionic_compiler::compile_graph;
    use psionic_core::{
        BackendExtensionKind, DType, Device, DeviceKind, QuantizationMode, Shape, TensorData,
        TensorSpec,
    };
    use psionic_ir::{AutodiffContext, AutodiffGraphBuilder, Graph, GraphBuilder, evaluate_graph};
    use serde::Deserialize;

    use super::CudaMemorySpace;
    use super::{
        CudaAllocatorPool, CudaBackend, CudaBuffer, CudaCommandStatus, CudaCommandWait,
        HealthStatus, SUPPORTED_OPS, allocator_pool_cache_key,
        architecture_from_compute_capability, cuda_health, parse_inventory_row, parse_mig_profile,
        recovery_profile, risk_profile, validate_supported_plan,
    };
    use psionic_runtime::{
        Allocator, AllocatorPoolPolicy, BackendDegradedPolicy, BackendSelectionState, BufferHandle,
        DeviceDiscovery, ExecutionResult, NvidiaRecoveryAction, NvidiaRiskLevel, RuntimeError,
        ServedProductBackendPolicy,
    };

    const QWEN38_QUANT_REFERENCE_VECTORS: &str =
        include_str!("../../../fixtures/qwen38/qwen38_ggml_quant_reference_vectors_v1.json");

    #[derive(Deserialize)]
    struct Qwen38QuantReferenceReport {
        vectors: Vec<Qwen38QuantReferenceVector>,
    }

    #[derive(Deserialize)]
    struct Qwen38QuantReferenceVector {
        block_type: String,
        block_bytes_hex: String,
    }

    #[test]
    fn bounded_scaled_dot_product_attention_support_tracks_selected_device() {
        let backend = CudaBackend::new();
        assert_eq!(
            backend.supports_bounded_scaled_dot_product_attention(),
            backend.selected_device().is_some()
        );
        if backend.supports_bounded_scaled_dot_product_attention() {
            assert_eq!(
                backend.bounded_scaled_dot_product_attention_refusal_reason(),
                None
            );
        } else {
            assert!(
                backend
                    .bounded_scaled_dot_product_attention_refusal_reason()
                    .is_some()
            );
        }
    }

    #[test]
    fn allocator_pool_reuses_exact_tensor_spec() {
        let device = Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0")));
        let spec = TensorSpec::new(Shape::new(vec![2, 8]), DType::F16, device);
        let cache_key = allocator_pool_cache_key(&spec);
        let mut pool = CudaAllocatorPool::new(AllocatorPoolPolicy::exact_tensor_spec(2, 128));
        assert_eq!(pool.checkout(cache_key.as_str(), 32), None);
        pool.record_cold_allocation(32);
        assert!(pool.checkin(cache_key.clone(), 32, 0xfeedusize));
        assert_eq!(pool.report().state.cached_buffers, 1);
        assert_eq!(pool.report().state.cached_bytes, 32);
        assert_eq!(pool.checkout(cache_key.as_str(), 32), Some(0xfeedusize));
        assert_eq!(pool.report().state.cached_buffers, 0);
        assert_eq!(pool.report().state.cached_bytes, 0);
        let telemetry = pool.telemetry();
        assert_eq!(telemetry.cold_allocations, 1);
        assert_eq!(telemetry.reuse_hits, 1);
        assert_eq!(telemetry.returned_buffers, 1);
        assert_eq!(telemetry.evicted_returns, 0);
        assert_eq!(telemetry.resident_device_bytes, 32);
        assert_eq!(telemetry.peak_resident_device_bytes, 32);
    }

    #[test]
    fn allocator_pool_tracks_current_and_peak_device_bytes() {
        let mut pool = CudaAllocatorPool::new(AllocatorPoolPolicy::exact_tensor_spec(2, 128));
        pool.record_cold_allocation(32);
        pool.record_cold_allocation(16);
        pool.record_device_free(32);

        let telemetry = pool.telemetry();
        assert_eq!(telemetry.cold_allocations, 2);
        assert_eq!(telemetry.resident_device_bytes, 16);
        assert_eq!(telemetry.peak_resident_device_bytes, 48);
    }

    #[test]
    fn allocator_pool_enforces_budget_and_records_evictions() {
        let device = Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0")));
        let spec = TensorSpec::new(Shape::new(vec![4, 4]), DType::F32, device);
        let cache_key = allocator_pool_cache_key(&spec);
        let mut pool = CudaAllocatorPool::new(AllocatorPoolPolicy::exact_tensor_spec(1, 64));
        assert!(pool.checkin(cache_key.clone(), 64, 0xbeefusize));
        assert!(!pool.checkin(cache_key, 64, 0xcafeusize));
        assert_eq!(pool.report().state.cached_buffers, 1);
        assert_eq!(pool.report().state.cached_bytes, 64);
        assert_eq!(pool.telemetry().evicted_returns, 1);
    }

    impl GraphBackendConformanceHarness for CudaBackend {
        type Buffer = CudaBuffer;

        fn backend_selection(
            &self,
            supported_ops: &[&str],
        ) -> Result<psionic_runtime::BackendSelection, RuntimeError> {
            CudaBackend::backend_selection(self, supported_ops)
        }

        fn input_buffer(
            &mut self,
            shape: Shape,
            values: Vec<f32>,
        ) -> Result<Self::Buffer, RuntimeError> {
            CudaBackend::input_buffer(self, shape, values)
        }

        fn compile_and_execute(
            &mut self,
            graph: &Graph,
            inputs: &std::collections::BTreeMap<psionic_core::TensorId, Self::Buffer>,
        ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError> {
            CudaBackend::compile_and_execute(self, graph, inputs)
        }

        fn dense_values(&self, buffer: &Self::Buffer) -> Result<Vec<f32>, RuntimeError> {
            buffer.read_f32()
        }

        fn known_unsupported_case(&mut self) -> Result<String, RuntimeError> {
            let selected = self.selected_device().cloned().ok_or_else(|| {
                RuntimeError::Backend(String::from("no cuda device for refusal case"))
            })?;
            let mut builder = GraphBuilder::new(selected.device);
            let input = builder.input("input", Shape::new(vec![1, 4]), DType::F32);
            let casted = builder
                .cast(&input, DType::I8)
                .map_err(|error| RuntimeError::Backend(error.to_string()))?;
            let graph = builder.finish(vec![casted]);
            let inputs = std::collections::BTreeMap::from([(
                input.id(),
                self.input_buffer(Shape::new(vec![1, 4]), vec![1.0_f32, -2.0, 3.5, 0.25])?,
            )]);
            match self.compile_and_execute(&graph, &inputs) {
                Err(RuntimeError::UnsupportedStep(step)) if step.starts_with("cast ") => Ok(
                    format!("cast refusal remained explicit for unsupported target dtype: {step}"),
                ),
                Err(other) => Err(RuntimeError::Backend(format!(
                    "expected unsupported cast refusal, found {other}"
                ))),
                Ok(_) => Err(RuntimeError::Backend(String::from(
                    "unsupported cast case executed successfully instead of refusing",
                ))),
            }
        }
    }

    #[test]
    fn cuda_backend_shared_conformance_harness_has_no_failures() {
        let mut backend = CudaBackend::new();
        let report = run_graph_backend_conformance(&mut backend);
        assert_eq!(report.backend, "cuda");
        assert_eq!(report.surface, "graph_execution");
        assert!(!report.has_failures(), "{report:?}");
    }

    #[test]
    fn inventory_row_parses_into_expected_descriptor_inputs() {
        let row = parse_inventory_row(
            "0, NVIDIA GeForce RTX 4080, 00000000:01:00.0, 16376, 8.9, Enabled, [N/A], Disabled, HMM",
        )
        .expect("inventory row should parse");

        assert_eq!(row.ordinal, 0);
        assert_eq!(row.name, "NVIDIA GeForce RTX 4080");
        assert_eq!(row.compute_capability.as_deref(), Some("8.9"));
        assert_eq!(row.memory_bytes, Some(16376 * 1024 * 1024));
        assert_eq!(row.display_attached, Some(true));
        assert_eq!(row.mig_profile, None);
        assert_eq!(row.persistence_mode_enabled, Some(false));
        assert_eq!(row.addressing_mode.as_deref(), Some("hmm"));
    }

    #[test]
    fn architecture_mapping_handles_known_compute_capabilities() {
        assert_eq!(
            architecture_from_compute_capability(Some("8.9")).as_deref(),
            Some("ada")
        );
        assert_eq!(
            architecture_from_compute_capability(Some("9.0")).as_deref(),
            Some("hopper")
        );
        assert_eq!(architecture_from_compute_capability(Some("6.1")), None);
    }

    #[test]
    fn mig_parser_skips_disabled_and_na_values() {
        assert_eq!(parse_mig_profile("[N/A]"), None);
        assert_eq!(parse_mig_profile("Disabled"), None);
        assert_eq!(parse_mig_profile("1g.10gb"), Some(String::from("1g.10gb")));
    }

    #[test]
    fn risk_profile_marks_display_and_mig_devices_as_elevated() {
        let display_risk = risk_profile(Some(true), None, Some(false));
        assert_eq!(display_risk.level, NvidiaRiskLevel::Elevated);
        assert!(
            display_risk
                .warnings
                .iter()
                .any(|warning| warning.contains("display-attached"))
        );

        let mig_risk = risk_profile(Some(false), Some("1g.10gb"), Some(true));
        assert_eq!(mig_risk.level, NvidiaRiskLevel::Elevated);
        assert!(mig_risk.mig_partitioned);
    }

    #[test]
    fn recovery_profile_preserves_gpu_reset_when_driver_reports_none() {
        let recovery = recovery_profile(Some("None"));
        assert_eq!(recovery.supports_gpu_reset, Some(true));
        assert_eq!(
            recovery.expected_actions,
            vec![
                NvidiaRecoveryAction::ProcessRestart,
                NvidiaRecoveryAction::GpuReset,
                NvidiaRecoveryAction::RebootHost,
            ]
        );
    }

    #[test]
    fn cuda_dense_surface_is_documented() {
        assert_eq!(
            SUPPORTED_OPS,
            &[
                "input",
                "constant",
                "cast",
                "matmul",
                "add",
                "mul",
                "relu_squared",
                "relu_squared_backward",
                "relu_squared_backward_from_output",
                "leaky_relu_squared",
                "leaky_relu_squared_backward",
                "silu",
                "silu_backward",
                "parameter_golf_token_embedding_lookup",
                "parameter_golf_banked_linear",
                "parameter_golf_banked_linear_input_backward",
                "parameter_golf_banked_linear_weight_backward",
                "parameter_golf_token_embedding_lookup_backward",
                "parameter_golf_projection_loss",
                "parameter_golf_projection_token_losses",
                "parameter_golf_projection_loss_backward",
                "rms_norm",
                "rotary_embedding",
                "rotary_embedding_backward",
                "scaled_dot_product_attention",
                "scaled_dot_product_attention_query_backward",
                "scaled_dot_product_attention_key_backward",
                "scaled_dot_product_attention_value_backward",
            ]
        );
    }

    #[test]
    fn cuda_plan_validation_accepts_host_fallback_view_ops()
    -> Result<(), Box<dyn std::error::Error>> {
        let device = Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0")));
        let mut builder = GraphBuilder::new(device);
        let input = builder.input("features", Shape::new(vec![1, 2]), DType::F32);
        let reshaped = builder.reshape(&input, Shape::new(vec![2]))?;
        let graph = builder.finish(vec![reshaped]);
        let plan = compile_graph(&graph)?;
        validate_supported_plan(&plan)?;
        Ok(())
    }

    #[test]
    fn execution_value_retain_counts_track_consumers_and_final_outputs()
    -> Result<(), Box<dyn std::error::Error>> {
        let device = Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0")));
        let mut builder = GraphBuilder::new(device);
        let left = builder.input("left", Shape::new(vec![1, 2]), DType::F32);
        let right = builder.input("right", Shape::new(vec![1, 2]), DType::F32);
        let summed = builder.add(&left, &right)?;
        let scaled = builder.mul(&summed, &right)?;
        let graph = builder.finish(vec![scaled.clone()]);
        let plan = compile_graph(&graph)?;
        let retain_counts = super::execution_value_retain_counts(&plan);

        assert_eq!(retain_counts.get(&left.id()).copied(), Some(1));
        assert_eq!(retain_counts.get(&right.id()).copied(), Some(2));
        assert_eq!(retain_counts.get(&summed.id()).copied(), Some(1));
        assert_eq!(retain_counts.get(&scaled.id()).copied(), Some(1));
        Ok(())
    }

    #[test]
    fn permute_contiguous_values_matches_expected_layout() -> Result<(), Box<dyn std::error::Error>>
    {
        let values = vec![0.0_f32, 1.0, 2.0, 3.0, 4.0, 5.0];
        let permuted = super::permute_contiguous_values(&values, &[2, 3], &[1, 0])?;
        assert_eq!(permuted, vec![0.0, 3.0, 1.0, 4.0, 2.0, 5.0]);
        Ok(())
    }

    #[test]
    fn permute_contiguous_values_matches_attention_layout_swap()
    -> Result<(), Box<dyn std::error::Error>> {
        let values = (0..12).map(|value| value as f32).collect::<Vec<_>>();
        let permuted = super::permute_contiguous_values(&values, &[1, 2, 3, 2], &[0, 2, 1, 3])?;
        assert_eq!(
            permuted,
            vec![0.0, 1.0, 6.0, 7.0, 2.0, 3.0, 8.0, 9.0, 4.0, 5.0, 10.0, 11.0]
        );
        Ok(())
    }

    #[test]
    fn expand_contiguous_values_repeats_broadcast_dimensions()
    -> Result<(), Box<dyn std::error::Error>> {
        let values = vec![10.0_f32, 20.0];
        let expanded = super::expand_contiguous_values(&values, &[1, 2], &[3, 2])?;
        assert_eq!(expanded, vec![10.0, 20.0, 10.0, 20.0, 10.0, 20.0]);
        Ok(())
    }

    #[test]
    fn reduce_sum_contiguous_values_reduces_requested_axis()
    -> Result<(), Box<dyn std::error::Error>> {
        let values = vec![1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0];
        let reduced = super::reduce_sum_contiguous_values(&values, &[2, 3], Some(1))?;
        assert_eq!(reduced, vec![6.0, 15.0]);
        assert_eq!(
            super::reduce_sum_contiguous_values(&values, &[2, 3], None)?,
            vec![21.0]
        );
        Ok(())
    }

    #[test]
    fn reduce_sum_contiguous_values_reduces_first_and_last_axis_fast_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let values = vec![1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0];
        assert_eq!(
            super::reduce_sum_contiguous_values(&values, &[2, 3], Some(0))?,
            vec![5.0, 7.0, 9.0]
        );
        assert_eq!(
            super::reduce_sum_contiguous_values(&values, &[2, 3], Some(1))?,
            vec![6.0, 15.0]
        );
        Ok(())
    }

    #[test]
    fn cuda_health_is_degraded_when_a_discovered_device_is_elevated() {
        let devices = vec![parse_inventory_row(
            "0, NVIDIA GeForce RTX 4080, 00000000:01:00.0, 16376, 8.9, Yes, [N/A], Disabled, HMM",
        )
        .expect("inventory row should parse")
        .into_device_descriptor()];
        let health = cuda_health(&devices);
        assert_eq!(health.status, HealthStatus::Degraded);
        assert!(health.message.contains("display-attached"));
    }

    #[test]
    fn cuda_backend_report_is_self_consistent() -> Result<(), psionic_runtime::RuntimeError> {
        let backend = CudaBackend::new();
        let report = backend.discovery_report()?;
        assert_eq!(backend.backend_name(), "cuda");
        match report.health.status {
            HealthStatus::Ready => assert!(!report.devices.is_empty()),
            HealthStatus::Degraded => assert!(!report.devices.is_empty()),
            HealthStatus::Offline => {}
        }
        Ok(())
    }

    #[test]
    fn cuda_backend_runtime_resources_are_explicit_when_available() {
        let backend = CudaBackend::new();
        if let Some(descriptor) = backend.selected_device() {
            let resources = backend
                .runtime_resources()
                .expect("available cuda backend should surface runtime resources");
            assert_eq!(resources.allocator_pool.policy.max_cached_buffers, 128);
            assert!(!resources.kernel_cache.policy.enabled);
            assert_eq!(
                resources
                    .device_memory_budget
                    .as_ref()
                    .and_then(|budget| budget.total_bytes),
                descriptor.memory_capacity_bytes
            );
        } else {
            assert!(backend.runtime_resources().is_none());
        }
    }

    #[test]
    fn cuda_backend_selection_reports_ready_or_degraded_cuda_when_available()
    -> Result<(), psionic_runtime::RuntimeError> {
        let backend = CudaBackend::new();
        let Some(_) = backend.selected_device() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let selection = backend.backend_selection(SUPPORTED_OPS)?;
        assert_eq!(selection.requested_backend, "cuda");
        assert_eq!(selection.effective_backend, "cuda");
        assert_eq!(
            selection.supported_ops,
            SUPPORTED_OPS
                .iter()
                .map(|label| String::from(*label))
                .collect::<Vec<_>>()
        );
        assert!(selection.selected_device.is_some());
        assert!(selection.runtime_resources.is_some());
        assert_eq!(
            selection.policy,
            ServedProductBackendPolicy::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend
            )
        );

        match backend.health().status {
            HealthStatus::Ready => {
                assert_eq!(selection.selection_state, BackendSelectionState::Direct);
                assert!(selection.degraded_reason.is_none());
            }
            HealthStatus::Degraded => {
                assert_eq!(
                    selection.selection_state,
                    BackendSelectionState::SameBackendDegraded
                );
                assert!(selection.degraded_reason.is_some());
            }
            HealthStatus::Offline => {
                panic!("selected CUDA device should not report offline health");
            }
        }

        Ok(())
    }

    #[test]
    fn cuda_backend_fallback_selection_reports_explicit_cpu_fallback_when_unavailable()
    -> Result<(), psionic_runtime::RuntimeError> {
        let backend = CudaBackend::new();
        if backend.selected_device().is_none() {
            let cpu = CpuBackend::new();
            let selection = backend.fallback_selection(&cpu, SUPPORTED_OPS)?;
            assert_eq!(selection.requested_backend, "cuda");
            assert_eq!(selection.effective_backend, "cpu");
            assert_eq!(
                selection.supported_ops,
                SUPPORTED_OPS
                    .iter()
                    .map(|label| String::from(*label))
                    .collect::<Vec<_>>()
            );
            assert!(selection.selected_device.is_some());
            assert!(selection.runtime_resources.is_some());
            assert!(selection.fallback_reason.is_some());
            assert_eq!(
                selection.policy,
                ServedProductBackendPolicy::fallback_to_compatible_backend(
                    BackendDegradedPolicy::AllowSameBackend
                )
            );
            assert_eq!(
                selection.selection_state,
                BackendSelectionState::CrossBackendFallback
            );
            assert!(selection.degraded_reason.is_none());
        }
        Ok(())
    }

    #[test]
    fn cuda_backend_allocates_and_submits_copy_when_available()
    -> Result<(), psionic_runtime::RuntimeError> {
        let mut backend = CudaBackend::new();
        let Some(device) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let spec = TensorSpec::new(Shape::new(vec![4]), DType::F32, device.device.clone());
        let mut left = backend.allocate(&spec)?;
        let right = backend.allocate(&spec)?;
        assert_eq!(left.memory_space(), CudaMemorySpace::Device);
        assert!(!left.host_visible());
        left.write_f32(&[1.0, 2.0, 3.0, 4.0])?;

        let mut submission = backend.begin_submission()?;
        submission.copy_buffer(&left, &right)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.status, CudaCommandStatus::Completed);
        assert_eq!(report.encoded_operations, 1);
        assert_eq!(right.read_f32()?, vec![1.0, 2.0, 3.0, 4.0]);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_dense_surface_when_available() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut builder = GraphBuilder::new(selected.device.clone());
        let input = builder.input("features", Shape::new(vec![1, 2]), DType::F32);
        let weights = builder.constant_f32(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])?;
        let bias = builder.constant_f32(Shape::new(vec![1, 2]), vec![0.5, 0.5])?;
        let projected = builder.matmul(&input, &weights)?;
        let shifted = builder.add(&projected, &bias)?;
        let graph = builder.finish(vec![shifted.clone()]);

        let mut inputs = std::collections::BTreeMap::new();
        inputs.insert(
            input.id(),
            backend.input_buffer(Shape::new(vec![1, 2]), vec![1.0, 0.0])?,
        );

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output = result
            .outputs
            .get(&shifted.id())
            .ok_or("missing cuda dense output")?;
        assert_eq!(output.read_f32()?, vec![1.5, 2.5]);
        assert_eq!(result.metrics.steps_executed, 5);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_mul_when_available() -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut builder = GraphBuilder::new(selected.device.clone());
        let input = builder.input("features", Shape::new(vec![1, 3]), DType::F32);
        let scale = builder.constant_f32(Shape::new(vec![1, 3]), vec![2.0, -1.0, 0.5])?;
        let scaled = builder.mul(&input, &scale)?;
        let graph = builder.finish(vec![scaled.clone()]);

        let mut inputs = std::collections::BTreeMap::new();
        inputs.insert(
            input.id(),
            backend.input_buffer(Shape::new(vec![1, 3]), vec![1.5, 2.0, -4.0])?,
        );

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output = result
            .outputs
            .get(&scaled.id())
            .ok_or("missing cuda mul output")?;
        assert_eq!(output.read_f32()?, vec![3.0, -2.0, -2.0]);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_relu_squared_backend_extension_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let input_values = vec![-2.0_f32, -0.5, 0.75, 3.0];
        let mut builder = GraphBuilder::new(selected.device.clone());
        let input = builder.input("x", Shape::new(vec![input_values.len()]), DType::F32);
        let activated = builder.relu_squared(&input)?;
        let graph = builder.finish(vec![activated.clone()]);

        let inputs = std::collections::BTreeMap::from([(
            input.id(),
            backend.input_buffer(Shape::new(vec![input_values.len()]), input_values.clone())?,
        )]);
        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output = result
            .outputs
            .get(&activated.id())
            .ok_or("missing cuda relu_squared output")?;
        assert_close(
            &output.read_f32()?,
            &super::relu_squared_forward_values(&input_values),
            1e-6,
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_leaky_relu_squared_backend_extension_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let input_values = vec![-2.0_f32, -0.5, 0.75, 3.0];
        let mut builder = GraphBuilder::new(selected.device.clone());
        let input = builder.input("x", Shape::new(vec![input_values.len()]), DType::F32);
        let activated = builder.leaky_relu_squared(&input, 0.5)?;
        let graph = builder.finish(vec![activated.clone()]);

        let inputs = std::collections::BTreeMap::from([(
            input.id(),
            backend.input_buffer(Shape::new(vec![input_values.len()]), input_values.clone())?,
        )]);
        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output = result
            .outputs
            .get(&activated.id())
            .ok_or("missing cuda leaky_relu_squared output")?;
        assert_close(
            &output.read_f32()?,
            &super::leaky_relu_squared_forward_values(&input_values, 0.5),
            1e-6,
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_silu_backend_extension_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let input_values = vec![-2.0_f32, -0.5, 0.75, 3.0];
        let mut builder = GraphBuilder::new(selected.device.clone());
        let input = builder.input("x", Shape::new(vec![input_values.len()]), DType::F32);
        let activated = builder.silu(&input)?;
        let graph = builder.finish(vec![activated.clone()]);

        let inputs = std::collections::BTreeMap::from([(
            input.id(),
            backend.input_buffer(Shape::new(vec![input_values.len()]), input_values.clone())?,
        )]);
        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output = result
            .outputs
            .get(&activated.id())
            .ok_or("missing cuda silu output")?;
        assert_close(
            &output.read_f32()?,
            &super::silu_forward_values(&input_values),
            1e-6,
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_parameter_golf_projection_loss_backend_extension_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let logit_softcap = 30.0_f32;
        let logits_shape = Shape::new(vec![1, 2, 4]);
        let target_shape = Shape::new(vec![1, 2]);
        let logits_values = vec![0.1_f32, -0.2, 0.3, 0.7, -0.4, 0.5, -0.1, 0.2];
        let target_values = vec![3_i32, 1];

        let mut builder = GraphBuilder::new(selected.device.clone());
        let logits = builder.input("logits", logits_shape.clone(), DType::F32);
        let target_ids = builder.input("target_ids", target_shape.clone(), DType::I32);
        let loss = builder.parameter_golf_projection_loss(&logits, &target_ids, logit_softcap)?;
        let graph = builder.finish(vec![loss.clone()]);

        let inputs = std::collections::BTreeMap::from([
            (
                logits.id(),
                backend.input_buffer(logits_shape.clone(), logits_values.clone())?,
            ),
            (
                target_ids.id(),
                backend.input_i32_buffer(target_shape.clone(), target_values.clone())?,
            ),
        ]);
        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output = result
            .outputs
            .get(&loss.id())
            .ok_or("missing cuda parameter golf projection loss output")?;

        let expected = evaluate_graph(
            &graph,
            &std::collections::BTreeMap::from([
                (logits.id(), TensorData::F32(logits_values)),
                (target_ids.id(), TensorData::I32(target_values)),
            ]),
        )?;
        let expected_loss = expected
            .get(&loss.id())
            .ok_or("missing reference parameter golf projection loss output")?
            .as_f32_slice()
            .ok_or("reference parameter golf projection loss output should be f32")?;
        assert_close(&output.read_f32()?, expected_loss, 1e-5);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_parameter_golf_projection_token_losses_backend_extension_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let logit_softcap = 30.0_f32;
        let logits_shape = Shape::new(vec![1, 2, 4]);
        let target_shape = Shape::new(vec![1, 2]);
        let logits_values = vec![0.1_f32, -0.2, 0.3, 0.7, -0.4, 0.5, -0.1, 0.2];
        let target_values = vec![3_i32, 1];

        let mut builder = GraphBuilder::new(selected.device.clone());
        let logits = builder.input("logits", logits_shape.clone(), DType::F32);
        let target_ids = builder.input("target_ids", target_shape.clone(), DType::I32);
        let token_losses =
            builder.parameter_golf_projection_token_losses(&logits, &target_ids, logit_softcap)?;
        let graph = builder.finish(vec![token_losses.clone()]);

        let inputs = std::collections::BTreeMap::from([
            (
                logits.id(),
                backend.input_buffer(logits_shape.clone(), logits_values.clone())?,
            ),
            (
                target_ids.id(),
                backend.input_i32_buffer(target_shape.clone(), target_values.clone())?,
            ),
        ]);
        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output = result
            .outputs
            .get(&token_losses.id())
            .ok_or("missing cuda parameter golf projection token-loss output")?;

        let expected = evaluate_graph(
            &graph,
            &std::collections::BTreeMap::from([
                (logits.id(), TensorData::F32(logits_values)),
                (target_ids.id(), TensorData::I32(target_values)),
            ]),
        )?;
        let expected_losses = expected
            .get(&token_losses.id())
            .ok_or("missing reference parameter golf projection token-loss output")?
            .as_f32_slice()
            .ok_or("reference parameter golf projection token-loss output should be f32")?;
        assert_close(&output.read_f32()?, expected_losses, 1e-5);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_parameter_golf_banked_linear_backend_extension_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let input_shape = Shape::new(vec![1, 2, 3]);
        let bank_shape = Shape::new(vec![2, 4, 3]);
        let input_values = vec![1.0_f32, -2.0, 0.5, 0.25, 1.5, -0.75];
        let bank_values = vec![
            0.2_f32, -0.4, 0.6, 1.0, 0.5, -0.5, -0.3, 0.8, 0.1, 0.7, -0.2, 0.9, -0.6, 0.4, 0.3,
            0.5, -0.7, 0.2, 0.1, 0.9, -0.8, -0.2, 0.6, 0.4,
        ];
        let quantized_bank_values = bank_values
            .iter()
            .map(|value| bf16::from_f32(*value).to_f32())
            .collect::<Vec<_>>();
        let bank_index = 1;

        let mut builder = GraphBuilder::new(selected.device.clone());
        let input = builder.input("input", input_shape.clone(), DType::F32);
        let matrix_bank = builder.input("matrix_bank", bank_shape.clone(), DType::BF16);
        let output = builder.parameter_golf_banked_linear(&input, &matrix_bank, bank_index)?;
        let graph = builder.finish(vec![output.clone()]);

        let inputs = std::collections::BTreeMap::from([
            (
                input.id(),
                backend.input_buffer(input_shape.clone(), input_values.clone())?,
            ),
            (
                matrix_bank.id(),
                backend.input_bf16_buffer(bank_shape.clone(), bank_values.clone())?,
            ),
        ]);
        let result = backend.compile_and_execute(&graph, &inputs)?;
        let actual = result
            .outputs
            .get(&output.id())
            .ok_or("missing cuda parameter golf banked-linear output")?;

        let expected = evaluate_graph(
            &graph,
            &std::collections::BTreeMap::from([
                (input.id(), TensorData::F32(input_values)),
                (matrix_bank.id(), TensorData::BF16(quantized_bank_values)),
            ]),
        )?;
        let expected_values = expected
            .get(&output.id())
            .ok_or("missing reference parameter golf banked-linear output")?
            .as_f32_slice()
            .ok_or("reference parameter golf banked-linear output should be f32")?;
        assert_close(&actual.read_f32()?, expected_values, 1e-5);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_rms_norm_backend_extension_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut builder = GraphBuilder::new(selected.device.clone());
        let input = builder.input("features", Shape::new(vec![2, 4]), DType::F32);
        let weight = builder.constant_f32(Shape::new(vec![4]), vec![1.0, 2.0, 3.0, 4.0])?;
        let normalized = builder.rms_norm(&input, &weight, 1e-5)?;
        let graph = builder.finish(vec![normalized.clone()]);

        let input_values = vec![1.0_f32, -2.0, 0.5, 4.0, -1.5, 0.25, 3.0, -0.75];
        let mut inputs = std::collections::BTreeMap::new();
        inputs.insert(
            input.id(),
            backend.input_buffer(Shape::new(vec![2, 4]), input_values.clone())?,
        );

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output = result
            .outputs
            .get(&normalized.id())
            .ok_or("missing cuda rms_norm output")?;
        let weights = [1.0_f32, 2.0, 3.0, 4.0];
        let expected = input_values
            .chunks_exact(weights.len())
            .flat_map(|row| {
                let mean_square =
                    row.iter().map(|value| value * value).sum::<f32>() / row.len() as f32;
                let inv_rms = 1.0_f32 / (mean_square + 1e-5).sqrt();
                row.iter()
                    .zip(weights.iter())
                    .map(|(input, weight)| input * weight * inv_rms)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_close(&output.read_f32()?, &expected, 1e-5);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_bounded_permute_graphs_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let rank2_shape = Shape::new(vec![2, 3]);
        let rank4_shape = Shape::new(vec![1, 2, 3, 2]);
        let rank2_values = vec![0.0_f32, 1.0, 2.0, 3.0, 4.0, 5.0];
        let rank4_values = (0..12).map(|value| value as f32).collect::<Vec<_>>();

        let mut builder = GraphBuilder::new(selected.device.clone());
        let rank2 = builder.input("rank2", rank2_shape.clone(), DType::F32);
        let rank4 = builder.input("rank4", rank4_shape.clone(), DType::F32);
        let transposed = builder.permute(&rank2, vec![1, 0])?;
        let swapped = builder.permute(&rank4, vec![0, 2, 1, 3])?;
        let graph = builder.finish(vec![transposed.clone(), swapped.clone()]);

        let inputs = std::collections::BTreeMap::from([
            (
                rank2.id(),
                backend.input_buffer(rank2_shape, rank2_values.clone())?,
            ),
            (
                rank4.id(),
                backend.input_buffer(rank4_shape, rank4_values.clone())?,
            ),
        ]);
        let result = backend.compile_and_execute(&graph, &inputs)?;
        let expected = evaluate_graph(
            &graph,
            &std::collections::BTreeMap::from([
                (rank2.id(), TensorData::F32(rank2_values)),
                (rank4.id(), TensorData::F32(rank4_values)),
            ]),
        )?;

        assert_close(
            &result
                .outputs
                .get(&transposed.id())
                .ok_or("missing transposed output")?
                .read_f32()?,
            expected
                .get(&transposed.id())
                .ok_or("missing expected transposed output")?
                .as_f32_slice()
                .ok_or("expected transposed output is not f32")?,
            1e-5,
        );
        assert_close(
            &result
                .outputs
                .get(&swapped.id())
                .ok_or("missing swapped output")?
                .read_f32()?,
            expected
                .get(&swapped.id())
                .ok_or("missing expected swapped output")?
                .as_f32_slice()
                .ok_or("expected swapped output is not f32")?,
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_bounded_reduce_sum_graphs_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let input_shape = Shape::new(vec![2, 3]);
        let input_values = vec![1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0];
        let rank3_shape = Shape::new(vec![2, 4, 3]);
        let rank3_values = (1..=24).map(|value| value as f32).collect::<Vec<_>>();

        let mut builder = GraphBuilder::new(selected.device.clone());
        let input = builder.input("input", input_shape.clone(), DType::F32);
        let rank3 = builder.input("rank3", rank3_shape.clone(), DType::F32);
        let full = builder.reduce_sum(&input);
        let axis0 = builder.reduce_sum_axis(&input, 0)?;
        let axis1 = builder.reduce_sum_axis(&input, 1)?;
        let rank3_axis1 = builder.reduce_sum_axis(&rank3, 1)?;
        let graph = builder.finish(vec![
            full.clone(),
            axis0.clone(),
            axis1.clone(),
            rank3_axis1.clone(),
        ]);

        let inputs = std::collections::BTreeMap::from([
            (
                input.id(),
                backend.input_buffer(input_shape, input_values.clone())?,
            ),
            (
                rank3.id(),
                backend.input_buffer(rank3_shape, rank3_values.clone())?,
            ),
        ]);
        let result = backend.compile_and_execute(&graph, &inputs)?;
        let expected = evaluate_graph(
            &graph,
            &std::collections::BTreeMap::from([
                (input.id(), TensorData::F32(input_values)),
                (rank3.id(), TensorData::F32(rank3_values)),
            ]),
        )?;

        for tensor in [&full, &axis0, &axis1, &rank3_axis1] {
            assert_close(
                &result
                    .outputs
                    .get(&tensor.id())
                    .ok_or("missing reduce_sum output")?
                    .read_f32()?,
                expected
                    .get(&tensor.id())
                    .ok_or("missing expected reduce_sum output")?
                    .as_f32_slice()
                    .ok_or("expected reduce_sum output is not f32")?,
                1e-5,
            );
        }
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_bounded_expand_graphs_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let rank3_shape = Shape::new(vec![1, 1, 3]);
        let rank4_shape = Shape::new(vec![1, 2, 1, 1]);
        let rank3_values = vec![1.0_f32, 2.0, 3.0];
        let rank4_values = vec![0.5_f32, 1.5];

        let mut builder = GraphBuilder::new(selected.device.clone());
        let rank3 = builder.input("rank3", rank3_shape.clone(), DType::F32);
        let rank4 = builder.input("rank4", rank4_shape.clone(), DType::F32);
        let expanded_rank3 = builder.expand(&rank3, Shape::new(vec![2, 4, 3]))?;
        let expanded_rank4 = builder.expand(&rank4, Shape::new(vec![2, 2, 3, 4]))?;
        let graph = builder.finish(vec![expanded_rank3.clone(), expanded_rank4.clone()]);

        let inputs = std::collections::BTreeMap::from([
            (
                rank3.id(),
                backend.input_buffer(rank3_shape, rank3_values.clone())?,
            ),
            (
                rank4.id(),
                backend.input_buffer(rank4_shape, rank4_values.clone())?,
            ),
        ]);
        let result = backend.compile_and_execute(&graph, &inputs)?;
        let expected = evaluate_graph(
            &graph,
            &std::collections::BTreeMap::from([
                (rank3.id(), TensorData::F32(rank3_values)),
                (rank4.id(), TensorData::F32(rank4_values)),
            ]),
        )?;

        for tensor in [&expanded_rank3, &expanded_rank4] {
            assert_close(
                &result
                    .outputs
                    .get(&tensor.id())
                    .ok_or("missing expand output")?
                    .read_f32()?,
                expected
                    .get(&tensor.id())
                    .ok_or("missing expected expand output")?
                    .as_f32_slice()
                    .ok_or("expected expand output is not f32")?,
                1e-5,
            );
        }
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_add_after_expand_materialization_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let expand_shape = Shape::new(vec![1, 1, 3]);
        let dense_shape = Shape::new(vec![2, 4, 3]);
        let expand_values = vec![1.0_f32, 2.0, 3.0];
        let dense_values = (0..24).map(|value| value as f32).collect::<Vec<_>>();

        let mut builder = GraphBuilder::new(selected.device.clone());
        let expand_input = builder.input("expand_input", expand_shape.clone(), DType::F32);
        let dense_input = builder.input("dense_input", dense_shape.clone(), DType::F32);
        let expanded = builder.expand(&expand_input, dense_shape.clone())?;
        let summed = builder.add(&expanded, &dense_input)?;
        let graph = builder.finish(vec![summed.clone()]);

        let inputs = std::collections::BTreeMap::from([
            (
                expand_input.id(),
                backend.input_buffer(expand_shape, expand_values.clone())?,
            ),
            (
                dense_input.id(),
                backend.input_buffer(dense_shape, dense_values.clone())?,
            ),
        ]);
        let result = backend.compile_and_execute(&graph, &inputs)?;
        let expected = evaluate_graph(
            &graph,
            &std::collections::BTreeMap::from([
                (expand_input.id(), TensorData::F32(expand_values)),
                (dense_input.id(), TensorData::F32(dense_values)),
            ]),
        )?;

        assert_close(
            &result
                .outputs
                .get(&summed.id())
                .ok_or("missing add output")?
                .read_f32()?,
            expected
                .get(&summed.id())
                .ok_or("missing expected add output")?
                .as_f32_slice()
                .ok_or("expected add output is not f32")?,
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_mul_after_expand_materialization_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let expand_shape = Shape::new(vec![1, 1, 3]);
        let dense_shape = Shape::new(vec![2, 4, 3]);
        let expand_values = vec![1.0_f32, 2.0, 3.0];
        let dense_values = (1..=24).map(|value| value as f32).collect::<Vec<_>>();

        let mut builder = GraphBuilder::new(selected.device.clone());
        let expand_input = builder.input("expand_input", expand_shape.clone(), DType::F32);
        let dense_input = builder.input("dense_input", dense_shape.clone(), DType::F32);
        let expanded = builder.expand(&expand_input, dense_shape.clone())?;
        let product = builder.mul(&expanded, &dense_input)?;
        let graph = builder.finish(vec![product.clone()]);

        let inputs = std::collections::BTreeMap::from([
            (
                expand_input.id(),
                backend.input_buffer(expand_shape, expand_values.clone())?,
            ),
            (
                dense_input.id(),
                backend.input_buffer(dense_shape, dense_values.clone())?,
            ),
        ]);
        let result = backend.compile_and_execute(&graph, &inputs)?;
        let expected = evaluate_graph(
            &graph,
            &std::collections::BTreeMap::from([
                (expand_input.id(), TensorData::F32(expand_values)),
                (dense_input.id(), TensorData::F32(dense_values)),
            ]),
        )?;

        assert_close(
            &result
                .outputs
                .get(&product.id())
                .ok_or("missing mul output")?
                .read_f32()?,
            expected
                .get(&product.id())
                .ok_or("missing expected mul output")?
                .as_f32_slice()
                .ok_or("expected mul output is not f32")?,
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_bf16_host_fallback_permute_graphs_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let input_shape = Shape::new(vec![2, 3]);
        let input_values = vec![0.0_f32, 1.0, 2.0, 3.0, 4.0, 5.0];

        let mut builder = GraphBuilder::new(selected.device.clone());
        let input = builder.input("input", input_shape.clone(), DType::BF16);
        let transposed = builder.permute(&input, vec![1, 0])?;
        let graph = builder.finish(vec![transposed.clone()]);

        let inputs = std::collections::BTreeMap::from([(
            input.id(),
            backend.input_bf16_buffer(input_shape, input_values.clone())?,
        )]);
        let result = backend.compile_and_execute(&graph, &inputs)?;
        let expected = evaluate_graph(
            &graph,
            &std::collections::BTreeMap::from([(input.id(), TensorData::BF16(input_values))]),
        )?;

        let actual = result
            .outputs
            .get(&transposed.id())
            .ok_or("missing transposed output")?
            .read_bf16_to_f32()?;
        assert_close(
            &actual,
            expected
                .get(&transposed.id())
                .ok_or("missing expected transposed output")?
                .as_f32_slice()
                .ok_or("expected transposed output is not dense float")?,
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_bf16_host_fallback_reduce_sum_graphs_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let input_shape = Shape::new(vec![2, 3]);
        let input_values = vec![1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0];

        let mut builder = GraphBuilder::new(selected.device.clone());
        let input = builder.input("input", input_shape.clone(), DType::BF16);
        let full = builder.reduce_sum(&input);
        let axis1 = builder.reduce_sum_axis(&input, 1)?;
        let graph = builder.finish(vec![full.clone(), axis1.clone()]);

        let inputs = std::collections::BTreeMap::from([(
            input.id(),
            backend.input_bf16_buffer(input_shape, input_values.clone())?,
        )]);
        let result = backend.compile_and_execute(&graph, &inputs)?;
        let expected = evaluate_graph(
            &graph,
            &std::collections::BTreeMap::from([(input.id(), TensorData::BF16(input_values))]),
        )?;

        for tensor in [&full, &axis1] {
            let output = result
                .outputs
                .get(&tensor.id())
                .ok_or("missing reduce_sum output")?;
            let actual = match output.spec().dtype() {
                DType::F32 => output.read_f32()?,
                DType::BF16 => output.read_bf16_to_f32()?,
                actual => return Err(format!("unexpected reduce_sum dtype {actual:?}").into()),
            };
            assert_close(
                &actual,
                expected
                    .get(&tensor.id())
                    .ok_or("missing expected reduce_sum output")?
                    .as_f32_slice()
                    .ok_or("expected reduce_sum output is not dense float")?,
                1e-5,
            );
        }
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_parameter_golf_token_embedding_lookup_backward_with_bf16_embeddings_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut builder = AutodiffGraphBuilder::with_context(
            selected.device.clone(),
            AutodiffContext::training(),
        );
        let token_ids = builder.input("token_ids", Shape::new(vec![2, 2]), DType::I32, false);
        let embeddings = builder.input("embeddings", Shape::new(vec![4, 3]), DType::BF16, true);
        let looked_up = builder.parameter_golf_token_embedding_lookup(&token_ids, &embeddings)?;
        let graph = builder.finish(vec![looked_up.clone()]);
        let backward_plan = graph.backward_plan(looked_up.id())?;
        let embedding_gradient = backward_plan
            .gradient_for(embeddings.id())
            .ok_or("missing embedding gradient output")?;
        let token_id_binding = backward_plan
            .primal_bindings
            .iter()
            .find(|binding| binding.primal_tensor == token_ids.id())
            .ok_or("missing token id binding")?
            .gradient_graph_input;
        let embedding_binding = backward_plan
            .primal_bindings
            .iter()
            .find(|binding| binding.primal_tensor == embeddings.id())
            .ok_or("missing embedding binding")?
            .gradient_graph_input;

        let inputs = std::collections::BTreeMap::from([
            (
                token_id_binding,
                backend.input_i32_buffer(Shape::new(vec![2, 2]), vec![0_i32, 1, 1, 3])?,
            ),
            (
                embedding_binding,
                backend.input_bf16_buffer(
                    Shape::new(vec![4, 3]),
                    vec![
                        1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
                    ],
                )?,
            ),
            (
                backward_plan.seed_input,
                backend.input_buffer(Shape::new(vec![2, 2, 3]), vec![1.0_f32; 12])?,
            ),
        ]);
        let result = backend.compile_and_execute(&backward_plan.gradient_graph, &inputs)?;
        let gradient = result
            .outputs
            .get(&embedding_gradient)
            .ok_or("missing embedding gradient")?;
        assert_close(
            &gradient.read_f32()?,
            &[
                1.0_f32, 1.0, 1.0, 2.0, 2.0, 2.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0,
            ],
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_reports_rms_norm_extension_support_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let backend = CudaBackend::new();
        if backend.selected_device().is_none() {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        }

        let selection = backend.backend_selection(SUPPORTED_OPS)?;
        assert!(
            selection
                .backend_extensions
                .iter()
                .any(|support| support.kind == BackendExtensionKind::ParameterGolfProjectionLoss)
        );
        assert!(
            selection
                .backend_extensions
                .iter()
                .any(|support| support.kind == BackendExtensionKind::RmsNorm)
        );
        assert!(
            selection
                .backend_extensions
                .iter()
                .any(|support| support.kind == BackendExtensionKind::RotaryEmbedding)
        );
        assert!(
            selection
                .backend_extensions
                .iter()
                .any(|support| support.kind == BackendExtensionKind::ScaledDotProductAttention)
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_parameter_golf_rope_gqa_decoder_block_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut cuda_backend = CudaBackend::new();
        let Some(selected) = cuda_backend.selected_device().cloned() else {
            assert_eq!(cuda_backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let batch_size = 1usize;
        let query_heads = 4usize;
        let kv_heads = 2usize;
        let sequence_length = 4usize;
        let head_dim = 8usize;
        let scale = 1.0_f32 / (head_dim as f32).sqrt();
        let (cos_values, sin_values) = standard_rope_tables(sequence_length, head_dim / 2);
        let query_values = (0..batch_size * query_heads * sequence_length * head_dim)
            .map(|index| (index as f32 - 60.0) * 0.0175)
            .collect::<Vec<_>>();
        let key_values = (0..batch_size * kv_heads * sequence_length * head_dim)
            .map(|index| (index as f32 - 24.0) * -0.021)
            .collect::<Vec<_>>();
        let value_values = (0..batch_size * kv_heads * sequence_length * head_dim)
            .map(|index| (index as f32 + 9.0) * 0.013)
            .collect::<Vec<_>>();
        let residual_values = (0..batch_size * query_heads * sequence_length * head_dim)
            .map(|index| (index as f32 % 9.0 - 4.0) * 0.05)
            .collect::<Vec<_>>();

        let query_shape = Shape::new(vec![batch_size, query_heads, sequence_length, head_dim]);
        let key_shape = Shape::new(vec![batch_size, kv_heads, sequence_length, head_dim]);
        let cos_shape = Shape::new(vec![sequence_length, head_dim / 2]);

        let mut cuda_builder = GraphBuilder::new(selected.device.clone());
        let cuda_query = cuda_builder.input("query", query_shape.clone(), DType::F32);
        let cuda_key = cuda_builder.input("key", key_shape.clone(), DType::F32);
        let cuda_value = cuda_builder.input("value", key_shape.clone(), DType::F32);
        let cuda_residual = cuda_builder.input("residual", query_shape.clone(), DType::F32);
        let cuda_cos = cuda_builder.constant_f32(cos_shape.clone(), cos_values.clone())?;
        let cuda_sin = cuda_builder.constant_f32(cos_shape.clone(), sin_values.clone())?;
        let cuda_output = cuda_builder.attention_rotary_residual_block(
            &cuda_query,
            &cuda_key,
            &cuda_value,
            &cuda_cos,
            &cuda_sin,
            &cuda_residual,
            scale,
            true,
        )?;
        let cuda_graph = cuda_builder.finish(vec![cuda_output.clone()]);

        let mut cpu_builder = GraphBuilder::new(Device::cpu());
        let cpu_query = cpu_builder.input("query", query_shape.clone(), DType::F32);
        let cpu_key = cpu_builder.input("key", key_shape.clone(), DType::F32);
        let cpu_value = cpu_builder.input("value", key_shape.clone(), DType::F32);
        let cpu_residual = cpu_builder.input("residual", query_shape.clone(), DType::F32);
        let cpu_cos = cpu_builder.constant_f32(cos_shape.clone(), cos_values)?;
        let cpu_sin = cpu_builder.constant_f32(cos_shape, sin_values)?;
        let cpu_output = cpu_builder.attention_rotary_residual_block(
            &cpu_query,
            &cpu_key,
            &cpu_value,
            &cpu_cos,
            &cpu_sin,
            &cpu_residual,
            scale,
            true,
        )?;
        let cpu_graph = cpu_builder.finish(vec![cpu_output.clone()]);

        let mut cuda_inputs = std::collections::BTreeMap::new();
        cuda_inputs.insert(
            cuda_query.id(),
            cuda_backend.input_buffer(query_shape.clone(), query_values.clone())?,
        );
        cuda_inputs.insert(
            cuda_key.id(),
            cuda_backend.input_buffer(key_shape.clone(), key_values.clone())?,
        );
        cuda_inputs.insert(
            cuda_value.id(),
            cuda_backend.input_buffer(key_shape.clone(), value_values.clone())?,
        );
        cuda_inputs.insert(
            cuda_residual.id(),
            cuda_backend.input_buffer(query_shape.clone(), residual_values.clone())?,
        );
        let cuda_result = cuda_backend.compile_and_execute(&cuda_graph, &cuda_inputs)?;
        let actual = cuda_result
            .outputs
            .get(&cuda_output.id())
            .ok_or("missing cuda rope/gqa output")?
            .read_f32()?;

        let mut cpu_backend = CpuBackend::new();
        let mut cpu_inputs = std::collections::BTreeMap::new();
        cpu_inputs.insert(
            cpu_query.id(),
            cpu_backend.input_buffer(query_shape.clone(), query_values)?,
        );
        cpu_inputs.insert(
            cpu_key.id(),
            cpu_backend.input_buffer(key_shape.clone(), key_values)?,
        );
        cpu_inputs.insert(
            cpu_value.id(),
            cpu_backend.input_buffer(key_shape.clone(), value_values)?,
        );
        cpu_inputs.insert(
            cpu_residual.id(),
            cpu_backend.input_buffer(query_shape, residual_values)?,
        );
        let cpu_result = cpu_backend.compile_and_execute(&cpu_graph, &cpu_inputs)?;
        let expected = cpu_result
            .outputs
            .get(&cpu_output.id())
            .ok_or("missing cpu rope/gqa output")?
            .logical_values()?;

        assert_close(&actual, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_bf16_scaled_dot_product_attention_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let batch_size = 1usize;
        let query_heads = 4usize;
        let kv_heads = 2usize;
        let sequence_length = 4usize;
        let head_dim = 8usize;
        let scale = 1.0_f32 / (head_dim as f32).sqrt();
        let query_shape = Shape::new(vec![batch_size, query_heads, sequence_length, head_dim]);
        let key_shape = Shape::new(vec![batch_size, kv_heads, sequence_length, head_dim]);

        let mut builder = GraphBuilder::new(selected.device.clone());
        let query = builder.input("query", query_shape.clone(), DType::BF16);
        let key = builder.input("key", key_shape.clone(), DType::BF16);
        let value = builder.input("value", key_shape.clone(), DType::BF16);
        let attended = builder.scaled_dot_product_attention(&query, &key, &value, scale, true)?;
        let graph = builder.finish(vec![attended.clone()]);

        let query_values = (0..batch_size * query_heads * sequence_length * head_dim)
            .map(|index| (index as f32 - 24.0) * 0.03125)
            .collect::<Vec<_>>();
        let key_values = (0..batch_size * kv_heads * sequence_length * head_dim)
            .map(|index| (index as f32 - 11.0) * -0.0275)
            .collect::<Vec<_>>();
        let value_values = (0..batch_size * kv_heads * sequence_length * head_dim)
            .map(|index| (index as f32 + 7.0) * 0.01875)
            .collect::<Vec<_>>();

        let inputs = std::collections::BTreeMap::from([
            (
                query.id(),
                backend.input_bf16_buffer(query_shape.clone(), query_values.clone())?,
            ),
            (
                key.id(),
                backend.input_bf16_buffer(key_shape.clone(), key_values.clone())?,
            ),
            (
                value.id(),
                backend.input_bf16_buffer(key_shape.clone(), value_values.clone())?,
            ),
        ]);
        let result = backend.compile_and_execute(&graph, &inputs)?;
        let expected = evaluate_graph(
            &graph,
            &std::collections::BTreeMap::from([
                (query.id(), TensorData::BF16(query_values)),
                (key.id(), TensorData::BF16(key_values)),
                (value.id(), TensorData::BF16(value_values)),
            ]),
        )?;

        let actual = result
            .outputs
            .get(&attended.id())
            .ok_or("missing bf16 attention output")?
            .read_bf16_to_f32()?;
        assert_close(
            &actual,
            expected
                .get(&attended.id())
                .ok_or("missing expected bf16 attention output")?
                .as_f32_slice()
                .ok_or("expected bf16 attention output is not dense float")?,
            2e-2,
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_bf16_scaled_dot_product_attention_backward_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let batch_size = 1usize;
        let query_heads = 4usize;
        let kv_heads = 2usize;
        let sequence_length = 4usize;
        let head_dim = 8usize;
        let scale = 1.0_f32 / (head_dim as f32).sqrt();
        let query_shape = Shape::new(vec![batch_size, query_heads, sequence_length, head_dim]);
        let key_shape = Shape::new(vec![batch_size, kv_heads, sequence_length, head_dim]);

        let mut builder = AutodiffGraphBuilder::with_context(
            selected.device.clone(),
            AutodiffContext::training(),
        );
        let query = builder.input("query", query_shape.clone(), DType::BF16, true);
        let key = builder.input("key", key_shape.clone(), DType::BF16, true);
        let value = builder.input("value", key_shape.clone(), DType::BF16, true);
        let attended = builder.scaled_dot_product_attention(&query, &key, &value, scale, true)?;
        let graph = builder.finish(vec![attended.clone()]);

        let backward_plan = graph.backward_plan(attended.id())?;
        assert!(
            backward_plan
                .gradient_graph
                .nodes()
                .iter()
                .any(|node| matches!(
                    node.op(),
                    psionic_ir::OpKind::BackendExtension {
                        op: psionic_core::BackendExtensionOp::ScaledDotProductAttentionQueryBackward { .. }
                    }
                ))
        );

        let query_values = (0..batch_size * query_heads * sequence_length * head_dim)
            .map(|index| (index as f32 - 19.0) * 0.03125)
            .collect::<Vec<_>>();
        let key_values = (0..batch_size * kv_heads * sequence_length * head_dim)
            .map(|index| (index as f32 - 7.0) * -0.0275)
            .collect::<Vec<_>>();
        let value_values = (0..batch_size * kv_heads * sequence_length * head_dim)
            .map(|index| (index as f32 + 11.0) * 0.01875)
            .collect::<Vec<_>>();
        let primal_inputs = std::collections::BTreeMap::from([
            (query.id(), TensorData::BF16(query_values.clone())),
            (key.id(), TensorData::BF16(key_values.clone())),
            (value.id(), TensorData::BF16(value_values.clone())),
        ]);
        let upstream_seed = (0..batch_size * query_heads * sequence_length * head_dim)
            .map(|index| ((index * 11 % 23) as f32 - 11.0) * 0.0625)
            .collect::<Vec<_>>();
        let forward_values = evaluate_graph(graph.graph(), &primal_inputs)?;
        let expected = graph.backward_materialized_with_seed(
            attended.id(),
            &primal_inputs,
            Some(TensorData::BF16(upstream_seed.clone())),
        )?;

        let plan = compile_graph(&backward_plan.gradient_graph)?;
        validate_supported_plan(&plan)?;

        let mut backward_inputs = std::collections::BTreeMap::new();
        for binding in &backward_plan.primal_bindings {
            let value = forward_values
                .get(&binding.primal_tensor)
                .ok_or("missing forward value for backward binding")?;
            let spec = backward_plan
                .gradient_graph
                .node(binding.gradient_graph_input)
                .ok_or("missing backward graph input node")?
                .tensor()
                .spec()
                .clone();
            let buffer = match value {
                TensorData::F32(values) => {
                    backend.input_buffer(spec.shape().clone(), values.clone())?
                }
                TensorData::BF16(values) => {
                    backend.input_bf16_buffer(spec.shape().clone(), values.clone())?
                }
                TensorData::I32(values) => {
                    backend.input_i32_buffer(spec.shape().clone(), values.clone())?
                }
                TensorData::QuantizedBlocks(_) => {
                    return Err("unexpected quantized backward binding value".into());
                }
            };
            backward_inputs.insert(binding.gradient_graph_input, buffer);
        }
        let seed_shape = backward_plan
            .gradient_graph
            .node(backward_plan.seed_input)
            .ok_or("missing backward seed node")?
            .tensor()
            .spec()
            .shape()
            .clone();
        backward_inputs.insert(
            backward_plan.seed_input,
            backend.input_bf16_buffer(seed_shape, upstream_seed)?,
        );

        let result =
            backend.compile_and_execute(&backward_plan.gradient_graph, &backward_inputs)?;
        let query_gradient = result
            .outputs
            .get(
                &backward_plan
                    .gradient_for(query.id())
                    .ok_or("missing query gradient id")?,
            )
            .ok_or("missing cuda bf16 attention query gradient output")?;
        let key_gradient = result
            .outputs
            .get(
                &backward_plan
                    .gradient_for(key.id())
                    .ok_or("missing key gradient id")?,
            )
            .ok_or("missing cuda bf16 attention key gradient output")?;
        let value_gradient = result
            .outputs
            .get(
                &backward_plan
                    .gradient_for(value.id())
                    .ok_or("missing value gradient id")?,
            )
            .ok_or("missing cuda bf16 attention value gradient output")?;

        let expected_query = match expected
            .gradient(query.id())
            .ok_or("missing expected bf16 query gradient")?
        {
            TensorData::F32(values) | TensorData::BF16(values) => values.as_slice(),
            TensorData::I32(_) | TensorData::QuantizedBlocks(_) => {
                return Err("unexpected non-float expected bf16 query gradient".into());
            }
        };
        let expected_key = match expected
            .gradient(key.id())
            .ok_or("missing expected bf16 key gradient")?
        {
            TensorData::F32(values) | TensorData::BF16(values) => values.as_slice(),
            TensorData::I32(_) | TensorData::QuantizedBlocks(_) => {
                return Err("unexpected non-float expected bf16 key gradient".into());
            }
        };
        let expected_value = match expected
            .gradient(value.id())
            .ok_or("missing expected bf16 value gradient")?
        {
            TensorData::F32(values) | TensorData::BF16(values) => values.as_slice(),
            TensorData::I32(_) | TensorData::QuantizedBlocks(_) => {
                return Err("unexpected non-float expected bf16 value gradient".into());
            }
        };

        assert_close(&query_gradient.read_bf16_to_f32()?, expected_query, 3e-2);
        assert_close(&key_gradient.read_bf16_to_f32()?, expected_key, 3e-2);
        assert_close(&value_gradient.read_bf16_to_f32()?, expected_value, 3e-2);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_rotary_embedding_backward_graph_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let batch_size = 1usize;
        let head_count = 4usize;
        let sequence_length = 4usize;
        let head_dim = 8usize;
        let input_shape = Shape::new(vec![batch_size, head_count, sequence_length, head_dim]);
        let cos_shape = Shape::new(vec![sequence_length, head_dim / 2]);
        let (cos_values, sin_values) = standard_rope_tables(sequence_length, head_dim / 2);

        let mut builder = AutodiffGraphBuilder::with_context(
            selected.device.clone(),
            AutodiffContext::training(),
        );
        let input = builder.input("input", input_shape.clone(), DType::F32, true);
        let cos = builder.constant_f32(cos_shape.clone(), cos_values)?;
        let sin = builder.constant_f32(cos_shape, sin_values)?;
        let roped = builder.rope(&input, &cos, &sin, false)?;
        let graph = builder.finish(vec![roped.clone()]);

        let backward_plan = graph.backward_plan(roped.id())?;
        assert!(
            backward_plan
                .gradient_graph
                .nodes()
                .iter()
                .any(|node| matches!(
                    node.op(),
                    psionic_ir::OpKind::BackendExtension {
                        op: psionic_core::BackendExtensionOp::RotaryEmbeddingBackward { .. }
                    }
                ))
        );

        let input_values = (0..batch_size * head_count * sequence_length * head_dim)
            .map(|index| (index as f32 - 32.0) * 0.0375)
            .collect::<Vec<_>>();
        let primal_inputs =
            std::collections::BTreeMap::from([(input.id(), TensorData::F32(input_values))]);
        let upstream_seed = (0..batch_size * head_count * sequence_length * head_dim)
            .map(|index| ((index * 13 % 29) as f32 - 14.0) * 0.03125)
            .collect::<Vec<_>>();
        let forward_values = evaluate_graph(graph.graph(), &primal_inputs)?;
        let expected = graph.backward_materialized_with_seed(
            roped.id(),
            &primal_inputs,
            Some(TensorData::F32(upstream_seed.clone())),
        )?;

        let plan = compile_graph(&backward_plan.gradient_graph)?;
        validate_supported_plan(&plan)?;

        let mut backward_inputs = std::collections::BTreeMap::new();
        for binding in &backward_plan.primal_bindings {
            let value = forward_values
                .get(&binding.primal_tensor)
                .ok_or("missing forward value for backward binding")?;
            let spec = backward_plan
                .gradient_graph
                .node(binding.gradient_graph_input)
                .ok_or("missing backward graph input node")?
                .tensor()
                .spec()
                .shape()
                .clone();
            backward_inputs.insert(
                binding.gradient_graph_input,
                backend.input_buffer(
                    spec,
                    value
                        .as_f32_slice()
                        .ok_or("non-f32 forward value")?
                        .to_vec(),
                )?,
            );
        }
        let seed_shape = backward_plan
            .gradient_graph
            .node(backward_plan.seed_input)
            .ok_or("missing backward seed node")?
            .tensor()
            .spec()
            .shape()
            .clone();
        backward_inputs.insert(
            backward_plan.seed_input,
            backend.input_buffer(seed_shape, upstream_seed)?,
        );

        let result =
            backend.compile_and_execute(&backward_plan.gradient_graph, &backward_inputs)?;
        let gradient = result
            .outputs
            .get(
                &backward_plan
                    .gradient_for(input.id())
                    .ok_or("missing input gradient id")?,
            )
            .ok_or("missing cuda rope input gradient output")?;

        assert_close(
            &gradient.read_f32()?,
            expected
                .gradient(input.id())
                .ok_or("missing expected input gradient")?
                .as_f32_slice()
                .ok_or("expected input gradient is not f32")?,
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_parameter_golf_decoder_backward_graph_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let batch_size = 1usize;
        let query_heads = 4usize;
        let kv_heads = 2usize;
        let sequence_length = 4usize;
        let head_dim = 8usize;
        let scale = 1.0_f32 / (head_dim as f32).sqrt();
        let query_shape = Shape::new(vec![batch_size, query_heads, sequence_length, head_dim]);
        let key_shape = Shape::new(vec![batch_size, kv_heads, sequence_length, head_dim]);
        let cos_shape = Shape::new(vec![sequence_length, head_dim / 2]);
        let (cos_values, sin_values) = standard_rope_tables(sequence_length, head_dim / 2);

        let mut builder = AutodiffGraphBuilder::with_context(
            selected.device.clone(),
            AutodiffContext::training(),
        );
        let query = builder.input("query", query_shape.clone(), DType::F32, true);
        let key = builder.input("key", key_shape.clone(), DType::F32, true);
        let value = builder.input("value", key_shape.clone(), DType::F32, true);
        let cos = builder.constant_f32(cos_shape.clone(), cos_values)?;
        let sin = builder.constant_f32(cos_shape, sin_values)?;
        let roped_query = builder.rope(&query, &cos, &sin, false)?;
        let roped_key = builder.rope(&key, &cos, &sin, false)?;
        let attended =
            builder.scaled_dot_product_attention(&roped_query, &roped_key, &value, scale, true)?;
        let graph = builder.finish(vec![attended.clone()]);

        let backward_plan = graph.backward_plan(attended.id())?;
        assert!(
            backward_plan
                .gradient_graph
                .nodes()
                .iter()
                .any(|node| matches!(
                    node.op(),
                    psionic_ir::OpKind::BackendExtension {
                        op: psionic_core::BackendExtensionOp::RotaryEmbeddingBackward { .. }
                    }
                ))
        );
        assert!(
            backward_plan
                .gradient_graph
                .nodes()
                .iter()
                .any(|node| matches!(
                    node.op(),
                    psionic_ir::OpKind::BackendExtension {
                        op: psionic_core::BackendExtensionOp::ScaledDotProductAttentionQueryBackward { .. }
                    }
                ))
        );
        assert!(
            backward_plan
                .gradient_graph
                .nodes()
                .iter()
                .any(|node| matches!(
                    node.op(),
                    psionic_ir::OpKind::BackendExtension {
                        op: psionic_core::BackendExtensionOp::ScaledDotProductAttentionKeyBackward { .. }
                    }
                ))
        );
        assert!(
            backward_plan
                .gradient_graph
                .nodes()
                .iter()
                .any(|node| matches!(
                    node.op(),
                    psionic_ir::OpKind::BackendExtension {
                        op: psionic_core::BackendExtensionOp::ScaledDotProductAttentionValueBackward { .. }
                    }
                ))
        );

        let query_values = (0..batch_size * query_heads * sequence_length * head_dim)
            .map(|index| (index as f32 - 60.0) * 0.0175)
            .collect::<Vec<_>>();
        let key_values = (0..batch_size * kv_heads * sequence_length * head_dim)
            .map(|index| (index as f32 - 24.0) * -0.021)
            .collect::<Vec<_>>();
        let value_values = (0..batch_size * kv_heads * sequence_length * head_dim)
            .map(|index| (index as f32 + 9.0) * 0.013)
            .collect::<Vec<_>>();
        let primal_inputs = std::collections::BTreeMap::from([
            (query.id(), TensorData::F32(query_values.clone())),
            (key.id(), TensorData::F32(key_values.clone())),
            (value.id(), TensorData::F32(value_values.clone())),
        ]);
        let upstream_seed = (0..batch_size * query_heads * sequence_length * head_dim)
            .map(|index| ((index * 17 % 41) as f32 - 20.0) * 0.05)
            .collect::<Vec<_>>();
        let forward_values = evaluate_graph(graph.graph(), &primal_inputs)?;
        let expected = graph.backward_materialized_with_seed(
            attended.id(),
            &primal_inputs,
            Some(TensorData::F32(upstream_seed.clone())),
        )?;

        let plan = compile_graph(&backward_plan.gradient_graph)?;
        validate_supported_plan(&plan)?;

        let mut backward_inputs = std::collections::BTreeMap::new();
        for binding in &backward_plan.primal_bindings {
            let value = forward_values
                .get(&binding.primal_tensor)
                .ok_or("missing forward value for backward binding")?;
            let spec = backward_plan
                .gradient_graph
                .node(binding.gradient_graph_input)
                .ok_or("missing backward graph input node")?
                .tensor()
                .spec()
                .shape()
                .clone();
            backward_inputs.insert(
                binding.gradient_graph_input,
                backend.input_buffer(
                    spec,
                    value
                        .as_f32_slice()
                        .ok_or("non-f32 forward value")?
                        .to_vec(),
                )?,
            );
        }
        let seed_shape = backward_plan
            .gradient_graph
            .node(backward_plan.seed_input)
            .ok_or("missing backward seed node")?
            .tensor()
            .spec()
            .shape()
            .clone();
        backward_inputs.insert(
            backward_plan.seed_input,
            backend.input_buffer(seed_shape, upstream_seed)?,
        );

        let result =
            backend.compile_and_execute(&backward_plan.gradient_graph, &backward_inputs)?;
        let query_gradient = result
            .outputs
            .get(
                &backward_plan
                    .gradient_for(query.id())
                    .ok_or("missing query gradient id")?,
            )
            .ok_or("missing cuda decoder query gradient output")?;
        let key_gradient = result
            .outputs
            .get(
                &backward_plan
                    .gradient_for(key.id())
                    .ok_or("missing key gradient id")?,
            )
            .ok_or("missing cuda decoder key gradient output")?;
        let value_gradient = result
            .outputs
            .get(
                &backward_plan
                    .gradient_for(value.id())
                    .ok_or("missing value gradient id")?,
            )
            .ok_or("missing cuda decoder value gradient output")?;

        assert_close(
            &query_gradient.read_f32()?,
            expected
                .gradient(query.id())
                .ok_or("missing expected query gradient")?
                .as_f32_slice()
                .ok_or("expected query gradient is not f32")?,
            1e-5,
        );
        assert_close(
            &key_gradient.read_f32()?,
            expected
                .gradient(key.id())
                .ok_or("missing expected key gradient")?
                .as_f32_slice()
                .ok_or("expected key gradient is not f32")?,
            1e-5,
        );
        assert_close(
            &value_gradient.read_f32()?,
            expected
                .gradient(value.id())
                .ok_or("missing expected value gradient")?
                .as_f32_slice()
                .ok_or("expected value gradient is not f32")?,
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_relu_squared_backward_graph_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut builder = AutodiffGraphBuilder::with_context(
            selected.device.clone(),
            AutodiffContext::training(),
        );
        let input = builder.input("x", Shape::new(vec![1, 4]), DType::F32, true);
        let activated = builder.relu_squared(&input)?;
        let scale = builder.constant_f32(Shape::new(vec![1, 4]), vec![0.5, -1.0, 0.25, 1.5])?;
        let scaled = builder.mul(&activated, &scale)?;
        let graph = builder.finish(vec![scaled.clone()]);

        let backward_plan = graph.backward_plan(scaled.id())?;
        assert!(backward_plan.gradient_for(input.id()).is_some());
        assert!(
            backward_plan
                .gradient_graph
                .nodes()
                .iter()
                .any(|node| matches!(
                    node.op(),
                    psionic_ir::OpKind::BackendExtension {
                        op: psionic_core::BackendExtensionOp::ReluSquaredBackwardFromOutput
                    }
                ))
        );

        let input_values = vec![-1.25_f32, -0.5, 0.75, 2.0];
        let upstream_seed = vec![1.0_f32, 0.5, -0.25, 2.0];
        let primal_inputs =
            std::collections::BTreeMap::from([(input.id(), TensorData::F32(input_values))]);
        let forward_values = evaluate_graph(graph.graph(), &primal_inputs)?;
        let expected = graph.backward_materialized_with_seed(
            scaled.id(),
            &primal_inputs,
            Some(TensorData::F32(upstream_seed.clone())),
        )?;

        let plan = compile_graph(&backward_plan.gradient_graph)?;
        validate_supported_plan(&plan)?;

        let mut backward_inputs = std::collections::BTreeMap::new();
        for binding in &backward_plan.primal_bindings {
            let value = forward_values
                .get(&binding.primal_tensor)
                .ok_or("missing forward value for backward binding")?;
            let spec = backward_plan
                .gradient_graph
                .node(binding.gradient_graph_input)
                .ok_or("missing backward graph input node")?
                .tensor()
                .spec()
                .shape()
                .clone();
            backward_inputs.insert(
                binding.gradient_graph_input,
                backend.input_buffer(
                    spec,
                    value
                        .as_f32_slice()
                        .ok_or("non-f32 forward value")?
                        .to_vec(),
                )?,
            );
        }
        let seed_shape = backward_plan
            .gradient_graph
            .node(backward_plan.seed_input)
            .ok_or("missing backward seed node")?
            .tensor()
            .spec()
            .shape()
            .clone();
        backward_inputs.insert(
            backward_plan.seed_input,
            backend.input_buffer(seed_shape, upstream_seed)?,
        );

        let result =
            backend.compile_and_execute(&backward_plan.gradient_graph, &backward_inputs)?;
        let input_gradient = result
            .outputs
            .get(
                &backward_plan
                    .gradient_for(input.id())
                    .ok_or("missing input gradient id")?,
            )
            .ok_or("missing cuda relu_squared input gradient output")?;

        assert_close(
            &input_gradient.read_f32()?,
            expected
                .gradient(input.id())
                .ok_or("missing expected input gradient")?
                .as_f32_slice()
                .ok_or("expected input gradient is not f32")?,
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_leaky_relu_squared_backward_graph_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut builder = AutodiffGraphBuilder::with_context(
            selected.device.clone(),
            AutodiffContext::training(),
        );
        let input = builder.input("x", Shape::new(vec![1, 4]), DType::F32, true);
        let activated = builder.leaky_relu_squared(&input, 0.5)?;
        let scale = builder.constant_f32(Shape::new(vec![1, 4]), vec![0.5, -1.0, 0.25, 1.5])?;
        let scaled = builder.mul(&activated, &scale)?;
        let graph = builder.finish(vec![scaled.clone()]);

        let backward_plan = graph.backward_plan(scaled.id())?;
        assert!(backward_plan.gradient_for(input.id()).is_some());
        assert!(
            backward_plan
                .gradient_graph
                .nodes()
                .iter()
                .any(|node| matches!(
                    node.op(),
                    psionic_ir::OpKind::BackendExtension {
                        op: psionic_core::BackendExtensionOp::LeakyReluSquaredBackward { .. }
                    }
                ))
        );

        let input_values = vec![-1.25_f32, -0.5, 0.75, 2.0];
        let upstream_seed = vec![1.0_f32, 0.5, -0.25, 2.0];
        let primal_inputs =
            std::collections::BTreeMap::from([(input.id(), TensorData::F32(input_values))]);
        let forward_values = evaluate_graph(graph.graph(), &primal_inputs)?;
        let expected = graph.backward_materialized_with_seed(
            scaled.id(),
            &primal_inputs,
            Some(TensorData::F32(upstream_seed.clone())),
        )?;

        let plan = compile_graph(&backward_plan.gradient_graph)?;
        validate_supported_plan(&plan)?;

        let mut backward_inputs = std::collections::BTreeMap::new();
        for binding in &backward_plan.primal_bindings {
            let value = forward_values
                .get(&binding.primal_tensor)
                .ok_or("missing forward value for backward binding")?;
            let spec = backward_plan
                .gradient_graph
                .node(binding.gradient_graph_input)
                .ok_or("missing backward graph input node")?
                .tensor()
                .spec()
                .shape()
                .clone();
            backward_inputs.insert(
                binding.gradient_graph_input,
                backend.input_buffer(
                    spec,
                    value
                        .as_f32_slice()
                        .ok_or("non-f32 forward value")?
                        .to_vec(),
                )?,
            );
        }
        let seed_shape = backward_plan
            .gradient_graph
            .node(backward_plan.seed_input)
            .ok_or("missing backward seed node")?
            .tensor()
            .spec()
            .shape()
            .clone();
        backward_inputs.insert(
            backward_plan.seed_input,
            backend.input_buffer(seed_shape, upstream_seed)?,
        );

        let result =
            backend.compile_and_execute(&backward_plan.gradient_graph, &backward_inputs)?;
        let input_gradient = result
            .outputs
            .get(
                &backward_plan
                    .gradient_for(input.id())
                    .ok_or("missing input gradient id")?,
            )
            .ok_or("missing cuda leaky_relu_squared input gradient output")?;

        assert_close(
            &input_gradient.read_f32()?,
            expected
                .gradient(input.id())
                .ok_or("missing expected input gradient")?
                .as_f32_slice()
                .ok_or("expected input gradient is not f32")?,
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_silu_backward_graph_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut builder = AutodiffGraphBuilder::with_context(
            selected.device.clone(),
            AutodiffContext::training(),
        );
        let input = builder.input("x", Shape::new(vec![1, 4]), DType::F32, true);
        let activated = builder.silu(&input)?;
        let scale = builder.constant_f32(Shape::new(vec![1, 4]), vec![0.5, -1.0, 0.25, 1.5])?;
        let scaled = builder.mul(&activated, &scale)?;
        let graph = builder.finish(vec![scaled.clone()]);

        let backward_plan = graph.backward_plan(scaled.id())?;
        assert!(backward_plan.gradient_for(input.id()).is_some());
        assert!(
            backward_plan
                .gradient_graph
                .nodes()
                .iter()
                .any(|node| matches!(
                    node.op(),
                    psionic_ir::OpKind::BackendExtension {
                        op: psionic_core::BackendExtensionOp::SiluBackward
                    }
                ))
        );

        let input_values = vec![-1.25_f32, -0.5, 0.75, 2.0];
        let upstream_seed = vec![1.0_f32, 0.5, -0.25, 2.0];
        let primal_inputs =
            std::collections::BTreeMap::from([(input.id(), TensorData::F32(input_values))]);
        let forward_values = evaluate_graph(graph.graph(), &primal_inputs)?;
        let expected = graph.backward_materialized_with_seed(
            scaled.id(),
            &primal_inputs,
            Some(TensorData::F32(upstream_seed.clone())),
        )?;

        let plan = compile_graph(&backward_plan.gradient_graph)?;
        validate_supported_plan(&plan)?;

        let mut backward_inputs = std::collections::BTreeMap::new();
        for binding in &backward_plan.primal_bindings {
            let value = forward_values
                .get(&binding.primal_tensor)
                .ok_or("missing forward value for backward binding")?;
            let spec = backward_plan
                .gradient_graph
                .node(binding.gradient_graph_input)
                .ok_or("missing backward graph input node")?
                .tensor()
                .spec()
                .shape()
                .clone();
            backward_inputs.insert(
                binding.gradient_graph_input,
                backend.input_buffer(
                    spec,
                    value
                        .as_f32_slice()
                        .ok_or("non-f32 forward value")?
                        .to_vec(),
                )?,
            );
        }
        let seed_shape = backward_plan
            .gradient_graph
            .node(backward_plan.seed_input)
            .ok_or("missing backward seed node")?
            .tensor()
            .spec()
            .shape()
            .clone();
        backward_inputs.insert(
            backward_plan.seed_input,
            backend.input_buffer(seed_shape, upstream_seed)?,
        );

        let result =
            backend.compile_and_execute(&backward_plan.gradient_graph, &backward_inputs)?;
        let input_gradient = result
            .outputs
            .get(
                &backward_plan
                    .gradient_for(input.id())
                    .ok_or("missing input gradient id")?,
            )
            .ok_or("missing cuda silu input gradient output")?;

        assert_close(
            &input_gradient.read_f32()?,
            expected
                .gradient(input.id())
                .ok_or("missing expected input gradient")?
                .as_f32_slice()
                .ok_or("expected input gradient is not f32")?,
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_rms_norm_backward_graph_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let epsilon = 1e-5_f32;
        let mut builder = AutodiffGraphBuilder::with_context(
            selected.device.clone(),
            AutodiffContext::training(),
        );
        let input = builder.input("x", Shape::new(vec![1, 4]), DType::F32, true);
        let weight = builder.input("weight", Shape::new(vec![4]), DType::F32, true);
        let normalized = builder.rms_norm(&input, &weight, epsilon)?;
        let scale = builder.constant_f32(Shape::new(vec![1, 4]), vec![0.5, -1.0, 0.25, 1.5])?;
        let scaled = builder.mul(&normalized, &scale)?;
        let graph = builder.finish(vec![scaled.clone()]);

        let backward_plan = graph.backward_plan(scaled.id())?;
        assert!(
            backward_plan
                .gradient_graph
                .nodes()
                .iter()
                .any(|node| matches!(
                    node.op(),
                    psionic_ir::OpKind::BackendExtension {
                        op: psionic_core::BackendExtensionOp::RmsNormInputBackward { .. }
                    }
                ))
        );
        assert!(
            backward_plan
                .gradient_graph
                .nodes()
                .iter()
                .any(|node| matches!(
                    node.op(),
                    psionic_ir::OpKind::BackendExtension {
                        op: psionic_core::BackendExtensionOp::RmsNormWeightBackward { .. }
                    }
                ))
        );

        let input_values = vec![0.3_f32, -0.8, 1.1, -1.0];
        let weight_values = vec![1.2_f32, -0.7, 0.9, 1.5];
        let upstream_seed = vec![1.0_f32, 0.5, -0.25, 2.0];
        let primal_inputs = std::collections::BTreeMap::from([
            (input.id(), TensorData::F32(input_values.clone())),
            (weight.id(), TensorData::F32(weight_values.clone())),
        ]);
        let forward_values = evaluate_graph(graph.graph(), &primal_inputs)?;
        let expected = graph.backward_materialized_with_seed(
            scaled.id(),
            &primal_inputs,
            Some(TensorData::F32(upstream_seed.clone())),
        )?;

        let plan = compile_graph(&backward_plan.gradient_graph)?;
        validate_supported_plan(&plan)?;

        let mut backward_inputs = std::collections::BTreeMap::new();
        for binding in &backward_plan.primal_bindings {
            let value = forward_values
                .get(&binding.primal_tensor)
                .ok_or("missing forward value for backward binding")?;
            let spec = backward_plan
                .gradient_graph
                .node(binding.gradient_graph_input)
                .ok_or("missing backward graph input node")?
                .tensor()
                .spec()
                .shape()
                .clone();
            backward_inputs.insert(
                binding.gradient_graph_input,
                backend.input_buffer(
                    spec,
                    value
                        .as_f32_slice()
                        .ok_or("non-f32 forward value")?
                        .to_vec(),
                )?,
            );
        }
        let seed_shape = backward_plan
            .gradient_graph
            .node(backward_plan.seed_input)
            .ok_or("missing backward seed node")?
            .tensor()
            .spec()
            .shape()
            .clone();
        backward_inputs.insert(
            backward_plan.seed_input,
            backend.input_buffer(seed_shape, upstream_seed.clone())?,
        );

        let result =
            backend.compile_and_execute(&backward_plan.gradient_graph, &backward_inputs)?;
        let input_gradient = result
            .outputs
            .get(
                &backward_plan
                    .gradient_for(input.id())
                    .ok_or("missing input gradient id")?,
            )
            .ok_or("missing cuda rms_norm input gradient output")?;
        let weight_gradient = result
            .outputs
            .get(
                &backward_plan
                    .gradient_for(weight.id())
                    .ok_or("missing weight gradient id")?,
            )
            .ok_or("missing cuda rms_norm weight gradient output")?;

        assert_close(
            &input_gradient.read_f32()?,
            expected
                .gradient(input.id())
                .ok_or("missing expected input gradient")?
                .as_f32_slice()
                .ok_or("expected input gradient is not f32")?,
            1e-5,
        );
        assert_close(
            &weight_gradient.read_f32()?,
            expected
                .gradient(weight.id())
                .ok_or("missing expected weight gradient")?
                .as_f32_slice()
                .ok_or("expected weight gradient is not f32")?,
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_parameter_golf_projection_loss_backward_graph_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let logit_softcap = 30.0_f32;
        let logits_shape = Shape::new(vec![1, 2, 4]);
        let target_shape = Shape::new(vec![1, 2]);
        let mut builder = AutodiffGraphBuilder::with_context(
            selected.device.clone(),
            AutodiffContext::training(),
        );
        let logits = builder.input("logits", logits_shape.clone(), DType::F32, true);
        let target_ids = builder.input("target_ids", target_shape.clone(), DType::I32, false);
        let loss = builder.parameter_golf_projection_loss(&logits, &target_ids, logit_softcap)?;
        let graph = builder.finish(vec![loss.clone()]);

        let backward_plan = graph.backward_plan(loss.id())?;
        assert!(backward_plan.gradient_for(logits.id()).is_some());
        assert!(backward_plan.gradient_for(target_ids.id()).is_none());
        assert!(backward_plan
            .gradient_graph
            .nodes()
            .iter()
            .any(|node| matches!(
                node.op(),
                psionic_ir::OpKind::BackendExtension {
                    op: psionic_core::BackendExtensionOp::ParameterGolfProjectionLossBackward { .. }
                }
            )));

        let logits_values = vec![0.1_f32, -0.2, 0.3, 0.7, -0.4, 0.5, -0.1, 0.2];
        let target_values = vec![3_i32, 1];
        let primal_inputs = std::collections::BTreeMap::from([
            (logits.id(), TensorData::F32(logits_values.clone())),
            (target_ids.id(), TensorData::I32(target_values.clone())),
        ]);
        let forward_values = evaluate_graph(graph.graph(), &primal_inputs)?;
        let expected = graph.backward_materialized(loss.id(), &primal_inputs)?;

        let plan = compile_graph(&backward_plan.gradient_graph)?;
        validate_supported_plan(&plan)?;

        let mut backward_inputs = std::collections::BTreeMap::new();
        for binding in &backward_plan.primal_bindings {
            let value = forward_values
                .get(&binding.primal_tensor)
                .ok_or("missing forward value for backward binding")?;
            let spec = backward_plan
                .gradient_graph
                .node(binding.gradient_graph_input)
                .ok_or("missing backward graph input node")?
                .tensor()
                .spec()
                .clone();
            let buffer = match value {
                TensorData::F32(values) => {
                    backend.input_buffer(spec.shape().clone(), values.clone())?
                }
                TensorData::BF16(values) => {
                    backend.input_bf16_buffer(spec.shape().clone(), values.clone())?
                }
                TensorData::I32(values) => {
                    backend.input_i32_buffer(spec.shape().clone(), values.clone())?
                }
                TensorData::QuantizedBlocks(_) => {
                    return Err("unexpected quantized backward binding value".into());
                }
            };
            backward_inputs.insert(binding.gradient_graph_input, buffer);
        }
        let seed_shape = backward_plan
            .gradient_graph
            .node(backward_plan.seed_input)
            .ok_or("missing backward seed node")?
            .tensor()
            .spec()
            .shape()
            .clone();
        backward_inputs.insert(
            backward_plan.seed_input,
            backend.input_buffer(seed_shape, vec![1.0_f32])?,
        );

        let result =
            backend.compile_and_execute(&backward_plan.gradient_graph, &backward_inputs)?;
        let logits_gradient = result
            .outputs
            .get(
                &backward_plan
                    .gradient_for(logits.id())
                    .ok_or("missing logits gradient id")?,
            )
            .ok_or("missing cuda parameter golf projection loss logits gradient output")?;

        assert_close(
            &logits_gradient.read_f32()?,
            expected
                .gradient(logits.id())
                .ok_or("missing expected logits gradient")?
                .as_f32_slice()
                .ok_or("expected logits gradient is not f32")?,
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_parameter_golf_residual_mix_backward_graph_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let shape = Shape::new(vec![1, 2, 4]);
        let mut builder = AutodiffGraphBuilder::with_context(
            selected.device.clone(),
            AutodiffContext::training(),
        );
        let current = builder.input("current", shape.clone(), DType::F32, false);
        let source = builder.input("source", shape.clone(), DType::F32, false);
        let mix_current = builder.input("mix_current", shape.clone(), DType::F32, true);
        let mix_source = builder.constant_f32(
            shape.clone(),
            vec![0.1, -0.7, 1.3, 0.8, -0.5, 0.2, 0.75, -1.0],
        )?;
        let attention = builder.constant_f32(
            shape.clone(),
            vec![0.4, -0.6, 0.3, 0.2, -0.7, 1.0, -0.2, 0.5],
        )?;
        let attn_scale = builder.constant_f32(
            shape.clone(),
            vec![1.0, 0.5, -1.1, 0.3, 0.2, -0.4, 0.9, 1.4],
        )?;
        let mlp = builder.constant_f32(
            shape.clone(),
            vec![0.6, 0.7, -0.5, 0.1, -0.3, 0.2, 1.1, -0.8],
        )?;
        let mlp_scale = builder.constant_f32(
            shape.clone(),
            vec![0.25, -0.9, 0.4, 0.6, 1.2, -0.2, 0.5, 0.75],
        )?;
        let skip = builder.constant_f32(
            shape.clone(),
            vec![0.3, -1.0, 0.2, 0.9, -0.4, 0.8, -0.6, 1.3],
        )?;
        let skip_scale = builder.constant_f32(
            shape.clone(),
            vec![1.5, 0.3, -0.7, 0.2, 0.6, -1.4, 0.8, 0.45],
        )?;

        let mixed_current = builder.mul(&current, &mix_current)?;
        let mixed_source = builder.mul(&source, &mix_source)?;
        let mixed = builder.add(&mixed_current, &mixed_source)?;
        let attention_delta = builder.mul(&attention, &attn_scale)?;
        let with_attention = builder.add(&mixed, &attention_delta)?;
        let mlp_delta = builder.mul(&mlp, &mlp_scale)?;
        let with_mlp = builder.add(&with_attention, &mlp_delta)?;
        let skip_delta = builder.mul(&skip, &skip_scale)?;
        let updated = builder.add(&with_mlp, &skip_delta)?;
        let graph = builder.finish(vec![updated.clone()]);

        let backward_plan = graph.backward_plan(updated.id())?;
        let plan = compile_graph(&backward_plan.gradient_graph)?;
        validate_supported_plan(&plan)?;

        let primal_inputs = std::collections::BTreeMap::from([
            (
                current.id(),
                TensorData::F32(vec![0.2, -0.4, 0.8, 1.0, -1.5, 0.3, 0.7, -0.9]),
            ),
            (
                source.id(),
                TensorData::F32(vec![1.2, 0.5, -0.2, 0.4, 0.6, -1.1, 0.9, 0.25]),
            ),
            (
                mix_current.id(),
                TensorData::F32(vec![0.9, 1.1, -0.3, 0.5, 0.4, -0.8, 0.6, 1.2]),
            ),
        ]);
        let upstream_seed = TensorData::F32(vec![1.0, -0.25, 0.5, 1.2, -0.75, 0.6, -1.1, 0.8]);
        let forward_values =
            evaluate_graph(graph.graph(), &primal_inputs).expect("residual-mix forward eval");
        let expected = graph
            .backward_materialized_with_seed(
                updated.id(),
                &primal_inputs,
                Some(upstream_seed.clone()),
            )
            .expect("residual-mix backward materialization");

        let mut backward_inputs = std::collections::BTreeMap::new();
        for node in backward_plan.gradient_graph.nodes() {
            let psionic_ir::OpKind::Input { .. } = node.op() else {
                continue;
            };
            if node.tensor().id() == backward_plan.seed_input {
                continue;
            }
            let value = backward_plan
                .primal_bindings
                .iter()
                .find(|binding| binding.gradient_graph_input == node.tensor().id())
                .and_then(|binding| forward_values.get(&binding.primal_tensor))
                .or_else(|| forward_values.get(&node.tensor().id()))
                .ok_or("missing forward value for residual-mix backward input")?;
            backward_inputs.insert(
                node.tensor().id(),
                backend.input_buffer(
                    node.tensor().spec().shape().clone(),
                    value
                        .as_f32_slice()
                        .ok_or("non-f32 residual-mix forward value")?
                        .to_vec(),
                )?,
            );
        }
        let seed_shape = backward_plan
            .gradient_graph
            .node(backward_plan.seed_input)
            .ok_or("missing residual-mix backward seed node")?
            .tensor()
            .spec()
            .shape()
            .clone();
        backward_inputs.insert(
            backward_plan.seed_input,
            backend.input_buffer(
                seed_shape,
                upstream_seed
                    .as_f32_slice()
                    .ok_or("non-f32 residual-mix seed")?
                    .to_vec(),
            )?,
        );
        let result = backend
            .compile_and_execute(&backward_plan.gradient_graph, &backward_inputs)
            .expect("cuda residual-mix compile and execute");
        let gradient_id = backward_plan
            .gradient_for(mix_current.id())
            .ok_or("missing residual-mix control gradient id")?;
        let actual = result
            .outputs
            .get(&gradient_id)
            .ok_or("missing cuda residual-mix control gradient output")?
            .read_f32()?;
        let expected_gradient = expected
            .gradient(mix_current.id())
            .ok_or("missing expected residual-mix control gradient")?
            .as_f32_slice()
            .ok_or("expected residual-mix control gradient is not f32")?;
        assert_close(&actual, expected_gradient, 1e-5);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_f16_rhs_matmul_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut left = backend.f16_buffer(2)?;
        let mut left_bytes = Vec::new();
        for value in [1.0_f32, 2.0] {
            left_bytes.extend_from_slice(&f32_to_f16_bits(value).to_le_bytes());
        }
        left.write_bytes(left_bytes.as_slice())?;
        let mut right = backend.f16_buffer(4)?;
        let mut right_bytes = Vec::new();
        for value in [1.0_f32, 2.0, 3.0, 4.0] {
            right_bytes.extend_from_slice(&f32_to_f16_bits(value).to_le_bytes());
        }
        right.write_bytes(right_bytes.as_slice())?;
        let output = backend.f32_buffer(2)?;

        let mut submission = backend.begin_submission()?;
        submission.matmul_f16_to_f32(&left, &right, &output, 1, 2, 2)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.status, CudaCommandStatus::Completed);
        assert_eq!(output.read_f32()?, vec![7.0, 10.0]);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_bf16_matmul_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut left = backend.bf16_buffer(2)?;
        left.write_bf16_from_f32(&[1.0, 2.0])?;
        let mut right = backend.bf16_buffer(4)?;
        right.write_bf16_from_f32(&[1.0, 2.0, 3.0, 4.0])?;
        let output = backend.f32_buffer(2)?;

        let mut submission = backend.begin_submission()?;
        submission.matmul_bf16_to_f32(&left, &right, &output, 1, 2, 2)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.status, CudaCommandStatus::Completed);
        assert_close(&left.read_bf16_to_f32()?, &[1.0, 2.0], 1e-5);
        assert_close(&right.read_bf16_to_f32()?, &[1.0, 2.0, 3.0, 4.0], 1e-5);
        assert_close(&output.read_f32()?, &[7.0, 10.0], 1e-5);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_f32_bf16_matmul_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let left = backend.input_buffer(Shape::new(vec![1, 2]), vec![1.0_f32, 2.0])?;
        let right = backend.input_bf16_buffer(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])?;
        let left_bf16 = backend.bf16_buffer(2)?;
        let output = backend.f32_buffer(2)?;

        let mut submission = backend.begin_submission()?;
        submission.cast_f32_to_bf16(&left, &left_bf16, 2)?;
        submission.matmul_bf16_to_f32(&left_bf16, &right, &output, 1, 2, 2)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.status, CudaCommandStatus::Completed);
        assert_close(&left_bf16.read_bf16_to_f32()?, &[1.0, 2.0], 1e-5);
        assert_close(&right.read_bf16_to_f32()?, &[1.0, 2.0, 3.0, 4.0], 1e-5);
        assert_close(&output.read_f32()?, &[7.0, 10.0], 1e-5);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_bf16_strided_batched_matmul_with_rhs_transpose_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut left = backend.bf16_buffer(12)?;
        left.write_bf16_from_f32(&[
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
        ])?;
        let mut right = backend.bf16_buffer(12)?;
        right.write_bf16_from_f32(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0])?;
        let output = backend.f32_buffer(8)?;

        let mut submission = backend.begin_submission()?;
        submission
            .matmul_bf16_strided_batched_to_f32(&left, &right, &output, 2, 3, 2, 2, false, true)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.status, CudaCommandStatus::Completed);
        assert_close(
            &output.read_f32()?,
            &[14.0, 32.0, 32.0, 77.0, 16.0, 8.0, 22.0, 11.0],
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_f32_bf16_strided_batched_matmul_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let left = backend.input_buffer(
            Shape::new(vec![2, 2, 2]),
            vec![1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        )?;
        let mut right = backend.bf16_buffer(8)?;
        right.write_bf16_from_f32(&[1.0, 0.0, 0.0, 1.0, 2.0, 1.0, 1.0, 2.0])?;
        let output = backend.f32_buffer(8)?;

        let mut submission = backend.begin_submission()?;
        submission.matmul_f32_bf16_strided_batched_to_f32(
            &left, &right, &output, 2, 2, 2, 2, false, false,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.status, CudaCommandStatus::Completed);
        assert_close(
            &output.read_f32()?,
            &[1.0, 2.0, 3.0, 4.0, 16.0, 17.0, 22.0, 23.0],
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_bf16_strided_batched_matmul_with_custom_batch_strides_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut left = backend.bf16_buffer(10)?;
        left.write_bf16_from_f32(&[1.0, 2.0, 3.0, 4.0, 99.0, 99.0, 5.0, 6.0, 7.0, 8.0])?;
        let mut right = backend.bf16_buffer(9)?;
        right.write_bf16_from_f32(&[1.0, 0.0, 0.0, 1.0, 88.0, 2.0, 1.0, 1.0, 2.0])?;
        let output = backend.f32_buffer(10)?;

        let mut submission = backend.begin_submission()?;
        submission.fill_buffer(&output, 0)?;
        submission.matmul_bf16_strided_batched_to_f32_with_strides(
            &left, &right, &output, 2, 2, 2, 2, 6, 5, 6, false, false,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.status, CudaCommandStatus::Completed);
        assert_close(
            &output.read_f32()?,
            &[1.0, 2.0, 3.0, 4.0, 0.0, 0.0, 16.0, 17.0, 22.0, 23.0],
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_f32_bf16_strided_batched_matmul_with_custom_batch_strides_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let left = backend.input_buffer(
            Shape::new(vec![10]),
            vec![1.0_f32, 2.0, 3.0, 4.0, 99.0, 99.0, 5.0, 6.0, 7.0, 8.0],
        )?;
        let mut right = backend.bf16_buffer(9)?;
        right.write_bf16_from_f32(&[1.0, 0.0, 0.0, 1.0, 88.0, 2.0, 1.0, 1.0, 2.0])?;
        let output = backend.f32_buffer(10)?;

        let mut submission = backend.begin_submission()?;
        submission.fill_buffer(&output, 0)?;
        submission.matmul_f32_bf16_strided_batched_to_f32_with_strides(
            &left, &right, &output, 2, 2, 2, 2, 6, 5, 6, false, false,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.status, CudaCommandStatus::Completed);
        assert_close(
            &output.read_f32()?,
            &[1.0, 2.0, 3.0, 4.0, 0.0, 0.0, 16.0, 17.0, 22.0, 23.0],
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_buffer_contiguous_subbuffer_shares_underlying_bank_storage_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let bank_shape = Shape::new(vec![2, 2, 3]);
        let mut bank = backend.bf16_buffer(bank_shape.element_count())?;
        bank.write_bf16_from_f32(&[0.0; 12])?;

        let slice_spec =
            TensorSpec::new(Shape::new(vec![2, 3]), DType::BF16, selected.device.clone());
        let slice_byte_len = slice_spec
            .storage_size()
            .checked_mul(std::mem::size_of::<bf16>())
            .ok_or("slice byte length overflow")?;
        let mut second_bank_slice =
            bank.contiguous_subbuffer_with_spec(&slice_spec, slice_byte_len)?;
        second_bank_slice.write_bf16_from_f32(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0])?;

        assert_ne!(
            second_bank_slice.allocation_identity(),
            bank.allocation_identity()
        );
        assert_close(
            &bank.read_bf16_to_f32()?,
            &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            1e-5,
        );
        assert_close(
            &second_bank_slice.read_bf16_to_f32()?,
            &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_attention_causal_row_softmax_in_place_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let logits = backend.input_buffer(
            Shape::new(vec![4, 4]),
            vec![
                10.0_f32, -1.0, -2.0, -3.0, 0.0, 0.0, 5.0, 6.0, -1.0, 0.0, 1.0, 2.0, 1.0, 2.0, 3.0,
                4.0,
            ],
        )?;

        let mut submission = backend.begin_submission()?;
        submission.attention_causal_row_softmax_in_place_f32(&logits, 4, 4, 1.0)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.status, CudaCommandStatus::Completed);
        assert_close(
            &logits.read_f32()?,
            &[
                1.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.0, 0.0, 0.09003057, 0.24472848, 0.66524094, 0.0,
                0.0320586, 0.08714432, 0.23688284, 0.6439143,
            ],
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_attention_causal_row_softmax_backward_in_place_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let probabilities = backend.input_buffer(
            Shape::new(vec![4, 4]),
            vec![
                1.0_f32, 0.0, 0.0, 0.0, 0.25, 0.75, 0.0, 0.0, 0.1, 0.2, 0.7, 0.0, 0.25, 0.25, 0.25,
                0.25,
            ],
        )?;
        let grad_probabilities = backend.input_buffer(
            Shape::new(vec![4, 4]),
            vec![
                2.0_f32, 9.0, 9.0, 9.0, 2.0, -1.0, 9.0, 9.0, 3.0, -1.0, 2.0, 9.0, 4.0, 1.0, -2.0,
                3.0,
            ],
        )?;

        let mut submission = backend.begin_submission()?;
        submission.attention_causal_row_softmax_backward_in_place_f32(
            &probabilities,
            &grad_probabilities,
            4,
            4,
            1.0,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.status, CudaCommandStatus::Completed);
        assert_close(
            &grad_probabilities.read_f32()?,
            &[
                0.0, 0.0, 0.0, 0.0, 0.5625, -0.5625, 0.0, 0.0, 0.15, -0.5, 0.35, 0.0, 0.625,
                -0.125, -0.875, 0.375,
            ],
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_buffer_writes_bf16_bits_when_available() -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut buffer = backend.bf16_buffer(4)?;
        let source = [1.0_f32, -2.0, 3.5, 0.25];
        let bits = source
            .iter()
            .map(|value| half::bf16::from_f32(*value).to_bits())
            .collect::<Vec<_>>();
        buffer.write_bf16_bits(bits.as_slice())?;
        let expected = source
            .iter()
            .map(|value| half::bf16::from_f32(*value).to_f32())
            .collect::<Vec<_>>();
        assert_close(&buffer.read_bf16_to_f32()?, expected.as_slice(), 1e-5);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_f32_to_bf16_cast_graphs_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut builder = GraphBuilder::new(selected.device.clone());
        let input = builder.input("input", Shape::new(vec![1, 4]), DType::F32);
        let casted = builder.cast(&input, DType::BF16)?;
        let casted_id = casted.id();
        let graph = builder.finish(vec![casted]);
        let inputs = std::collections::BTreeMap::from([(
            input.id(),
            backend.input_buffer(Shape::new(vec![1, 4]), vec![1.0_f32, -2.0, 3.5, 0.25])?,
        )]);
        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output = result
            .outputs
            .get(&casted_id)
            .ok_or("missing f32->bf16 cast output")?;
        assert_eq!(output.spec().dtype(), DType::BF16);
        assert_close(&output.read_bf16_to_f32()?, &[1.0, -2.0, 3.5, 0.25], 1e-3);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_bf16_to_f32_cast_graphs_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut builder = GraphBuilder::new(selected.device.clone());
        let input = builder.input("input", Shape::new(vec![1, 4]), DType::BF16);
        let casted = builder.cast(&input, DType::F32)?;
        let casted_id = casted.id();
        let graph = builder.finish(vec![casted]);
        let inputs = std::collections::BTreeMap::from([(
            input.id(),
            backend.input_bf16_buffer(Shape::new(vec![1, 4]), vec![1.0_f32, -2.0, 3.5, 0.25])?,
        )]);
        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output = result
            .outputs
            .get(&casted_id)
            .ok_or("missing bf16->f32 cast output")?;
        assert_eq!(output.spec().dtype(), DType::F32);
        assert_close(&output.read_f32()?, &[1.0, -2.0, 3.5, 0.25], 1e-3);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_multiple_matmuls_and_adds_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut builder = GraphBuilder::new(selected.device.clone());
        let token_input = builder.input("token", Shape::new(vec![1, 2]), DType::F32);
        let position_input = builder.input("position", Shape::new(vec![1, 2]), DType::F32);
        let context_input = builder.input("context", Shape::new(vec![1, 2]), DType::F32);
        let token_embedding =
            builder.constant_f32(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])?;
        let position_embedding =
            builder.constant_f32(Shape::new(vec![2, 2]), vec![0.5, 1.5, 2.5, 3.5])?;
        let context_projection =
            builder.constant_f32(Shape::new(vec![2, 2]), vec![2.0, 0.0, 0.0, 2.0])?;
        let lm_head =
            builder.constant_f32(Shape::new(vec![2, 3]), vec![1.0, 0.0, 2.0, 0.5, 1.0, -1.0])?;
        let lm_bias = builder.constant_f32(Shape::new(vec![1, 3]), vec![0.25, -0.5, 1.0])?;

        let token_hidden = builder.matmul(&token_input, &token_embedding)?;
        let position_hidden = builder.matmul(&position_input, &position_embedding)?;
        let context_hidden = builder.matmul(&context_input, &context_projection)?;
        let hidden = builder.add(&token_hidden, &position_hidden)?;
        let hidden = builder.add(&hidden, &context_hidden)?;
        let logits = builder.matmul(&hidden, &lm_head)?;
        let logits = builder.add(&logits, &lm_bias)?;
        let graph = builder.finish(vec![hidden.clone(), logits.clone()]);

        let mut inputs = std::collections::BTreeMap::new();
        inputs.insert(
            token_input.id(),
            backend.input_buffer(Shape::new(vec![1, 2]), vec![1.0, 0.0])?,
        );
        inputs.insert(
            position_input.id(),
            backend.input_buffer(Shape::new(vec![1, 2]), vec![0.0, 1.0])?,
        );
        inputs.insert(
            context_input.id(),
            backend.input_buffer(Shape::new(vec![1, 2]), vec![0.5, 0.25])?,
        );

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let hidden_output = result
            .outputs
            .get(&hidden.id())
            .ok_or("missing cuda hidden output")?;
        let logits_output = result
            .outputs
            .get(&logits.id())
            .ok_or("missing cuda logits output")?;
        assert_eq!(hidden_output.read_f32()?, vec![4.5, 6.0]);
        assert_eq!(logits_output.read_f32()?, vec![7.75, 5.5, 4.0]);
        assert_eq!(result.metrics.steps_executed, 15);
        Ok(())
    }

    #[test]
    fn cuda_backend_refuses_allocation_when_unavailable() {
        let mut backend = CudaBackend::new();
        if backend.selected_device().is_some() {
            return;
        }
        let spec = TensorSpec::new(Shape::new(vec![1]), DType::F32, Device::cpu());
        assert!(backend.allocate(&spec).is_err());
    }

    #[test]
    fn cuda_backend_executes_q8_0_quantized_matvec_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = vec![1.0_f32; 32];
        let row_a = sample_q8_0_row(0.25, 1);
        let row_b = sample_q8_0_row(0.5, -1);
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_a)?,
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_b)?,
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let actual =
            backend.quantized_matvec(&weights, QuantizationMode::GgmlQ8_0, 2, 32, &input)?;
        assert_close(&actual, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_backend_dequantizes_q8_0_row_when_available() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let row_a = sample_q8_0_row(0.25, 1);
        let row_b = sample_q8_0_row(0.5, -1);
        let mut expected = Vec::new();
        decode_quantized_row_into(QuantizationMode::GgmlQ8_0, &row_b, &mut expected)?;
        let mut bytes = row_a;
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let decode_params = backend.byte_buffer(&i32_slice_to_bytes(&[0_i32, 0_i32, 1_i32]))?;
        let output = backend.f32_buffer(32)?;
        let mut submission = backend.begin_submission()?;
        submission.dequantize_row_to_f32(
            &weights,
            QuantizationMode::GgmlQ8_0,
            2,
            34,
            32,
            &decode_params,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);
        assert_close(&output.read_f32()?, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_mxfp4_quantized_matvec_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_reference_vector();
        let row_a = sample_mxfp4_row(4);
        let row_b = sample_mxfp4_row(6);
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlMxfp4, &row_a)?,
            quantized_row_dot(&input, QuantizationMode::GgmlMxfp4, &row_b)?,
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let actual =
            backend.quantized_matvec(&weights, QuantizationMode::GgmlMxfp4, 2, 32, &input)?;
        assert_close(&actual, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_q4_k_quantized_matvec_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_reference_vector_256();
        let row_a = sample_q4_k_row(0.5, 0.125, 3);
        let row_b = sample_q4_k_row(0.375, 0.25, 11);
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlQ4K, &row_a)?,
            quantized_row_dot(&input, QuantizationMode::GgmlQ4K, &row_b)?,
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let actual =
            backend.quantized_matvec(&weights, QuantizationMode::GgmlQ4K, 2, 256, &input)?;
        assert_close(&actual, &expected, 1e-3);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_q6_k_quantized_matvec_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_reference_vector_256();
        let row_a = sample_q6_k_row(0.25, -3);
        let row_b = sample_q6_k_row(0.5, 2);
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlQ6K, &row_a)?,
            quantized_row_dot(&input, QuantizationMode::GgmlQ6K, &row_b)?,
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let actual =
            backend.quantized_matvec(&weights, QuantizationMode::GgmlQ6K, 2, 256, &input)?;
        assert_close(&actual, &expected, 1e-3);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_qwen38_quantized_rows_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_reference_vector_256();
        for (mode, row) in qwen38_quantized_rows()? {
            let expected_dot = quantized_row_dot(&input, mode, &row)?;
            let weights = backend.byte_buffer(&row)?;
            let actual = backend.quantized_matvec(&weights, mode, 1, 256, &input)?;
            assert_close(&actual, &[expected_dot], 2e-3);

            let mut expected_row = Vec::new();
            decode_quantized_row_into(mode, &row, &mut expected_row)?;
            let decode_params = backend.byte_buffer(&i32_slice_to_bytes(&[0_i32, 0_i32, 0_i32]))?;
            let output = backend.f32_buffer(256)?;
            let mut submission = backend.begin_submission()?;
            submission.dequantize_row_to_f32(
                &weights,
                mode,
                1,
                row.len(),
                256,
                &decode_params,
                &output,
            )?;
            submission.commit(CudaCommandWait::Completed)?;
            assert_close(&output.read_f32()?, &expected_row, 1e-6);
        }
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_qwen38_q8_1_and_argmax_paths_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector_256();
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input.clone())?;
        let q8_1_buffer =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 256)?])?;
        for (mode, row) in qwen38_quantized_rows()? {
            let expected = quantized_row_dot(&input, mode, &row)?;
            let mut two_rows = row.clone();
            two_rows.extend_from_slice(&row);
            let weights = backend.byte_buffer(&two_rows)?;
            let bias = backend.input_buffer(Shape::new(vec![2]), vec![0.0, 1.0])?;
            let output = backend.f32_buffer(2)?;
            let mut argmax_output = backend.byte_buffer(&vec![0_u8; std::mem::size_of::<u64>()])?;
            argmax_output.write_bytes(
                &(((u64::from(!f32::NEG_INFINITY.to_bits())) << 32)
                    | u64::from(u32::MAX - i32::MAX as u32))
                .to_ne_bytes(),
            )?;

            let mut submission = backend.begin_submission()?;
            submission.quantize_f32_to_q8_1(&input_buffer, 1, 256, &q8_1_buffer)?;
            submission.quantized_matvec_q8_1(
                &weights,
                0,
                mode,
                2,
                256,
                &q8_1_buffer,
                Some(&bias),
                &output,
            )?;
            submission.quantized_matvec_q8_1_argmax(
                &weights,
                0,
                mode,
                2,
                256,
                &q8_1_buffer,
                Some(&bias),
                &argmax_output,
            )?;
            submission.commit(CudaCommandWait::Completed)?;
            assert_close(&output.read_f32()?, &[expected, expected + 1.0], 2e-3);

            let packed_bytes = argmax_output.read_bytes()?;
            let packed = u64::from_ne_bytes(
                packed_bytes[..std::mem::size_of::<u64>()]
                    .try_into()
                    .expect("packed argmax buffer should be eight bytes"),
            );
            assert_eq!((u32::MAX - packed as u32) as usize, 1, "mode {mode:?}");
        }
        Ok(())
    }

    #[test]
    fn cuda_backend_dequantizes_mxfp4_row_when_available() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let row_a = sample_mxfp4_row(4);
        let row_b = sample_mxfp4_row(6);
        let mut expected = Vec::new();
        decode_quantized_row_into(QuantizationMode::GgmlMxfp4, &row_b, &mut expected)?;
        let mut bytes = row_a;
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let decode_params = backend.byte_buffer(&i32_slice_to_bytes(&[0_i32, 0_i32, 1_i32]))?;
        let output = backend.f32_buffer(32)?;
        let mut submission = backend.begin_submission()?;
        submission.dequantize_row_to_f32(
            &weights,
            QuantizationMode::GgmlMxfp4,
            2,
            17,
            32,
            &decode_params,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);
        assert_close(&output.read_f32()?, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_backend_gathers_f16_row_into_f32_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        let row_a = sample_reference_vector();
        let row_b = sample_q8_1_exact_vector();
        let mut bytes =
            Vec::with_capacity((row_a.len() + row_b.len()) * std::mem::size_of::<u16>());
        for value in row_a.iter().chain(row_b.iter()) {
            bytes.extend_from_slice(&f32_to_f16_bits(*value).to_le_bytes());
        }
        let input = backend.byte_buffer(&bytes)?;
        let decode_params = backend.byte_buffer(&i32_slice_to_bytes(&[0_i32, 0_i32, 1_i32]))?;
        let output = backend.f32_buffer(row_b.len())?;
        let mut submission = backend.begin_submission()?;
        submission.gather_f16_row_to_f32(&input, 2, row_b.len(), &decode_params, &output)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);
        assert_close(&output.read_f32()?, &row_b, 1e-3);
        Ok(())
    }

    #[test]
    fn cuda_backend_gathers_f32_values_by_index_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        let input = backend.input_buffer(
            Shape::new(vec![6]),
            vec![1.0_f32, -2.0, 4.25, 3.0, 9.5, 0.0],
        )?;
        let indices = backend.input_i32_buffer(Shape::new(vec![4]), vec![4_i32, 2, 5, 1])?;
        let output = backend.f32_buffer(4)?;
        let mut submission = backend.begin_submission()?;
        submission.gather_f32_by_indices(&input, 6, &indices, 4, &output)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);
        assert_close(&output.read_f32()?, &[9.5_f32, 4.25, 0.0, -2.0], 1e-6);
        Ok(())
    }

    #[test]
    fn cuda_backend_i32_buffer_preserves_i32_dtype_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        let mut buffer = backend.i32_buffer(3)?;
        buffer.write_i32_at_offset(0, &[4_i32, 2, 5])?;
        assert_eq!(buffer.read_i32()?, vec![4_i32, 2, 5]);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_argmax_when_available() -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = backend.input_buffer(
            Shape::new(vec![6]),
            vec![1.0_f32, -2.0, 4.25, 3.0, 9.5, 0.0],
        )?;
        let output = backend.byte_buffer(&vec![0_u8; std::mem::size_of::<i32>()])?;
        let mut submission = backend.begin_submission()?;
        submission.argmax_f32(&input, 1, 6, &output)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);

        let bytes = output.read_bytes()?;
        let argmax = i32::from_ne_bytes(bytes[..4].try_into().expect("argmax bytes"));
        assert_eq!(argmax, 4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_argmax_for_wide_rows_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let mut values = vec![-1.0_f32; 2049];
        values[1733] = 42.0;
        let input = backend.input_buffer(Shape::new(vec![values.len()]), values)?;
        let output = backend.byte_buffer(&vec![0_u8; std::mem::size_of::<i32>()])?;
        let mut submission = backend.begin_submission()?;
        submission.argmax_f32(&input, 1, 2049, &output)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);

        let bytes = output.read_bytes()?;
        let argmax = i32::from_ne_bytes(bytes[..4].try_into().expect("argmax bytes"));
        assert_eq!(argmax, 1733);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_top_k_for_one_row_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = backend.input_buffer(
            Shape::new(vec![6]),
            vec![1.0_f32, -2.0, 4.25, 3.0, 9.5, 4.25],
        )?;
        let selected_indices = backend.i32_buffer(3)?;
        let selected_values = backend.f32_buffer(3)?;
        let mut submission = backend.begin_submission()?;
        submission.top_k_f32(&input, 1, 6, 3, &selected_indices, &selected_values)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);
        let selected_indices = selected_indices
            .read_bytes()?
            .chunks_exact(std::mem::size_of::<i32>())
            .map(|chunk| i32::from_ne_bytes(chunk.try_into().expect("top_k i32 bytes")))
            .collect::<Vec<_>>();
        assert_eq!(selected_indices, vec![4, 2, 5]);
        assert_close(&selected_values.read_f32()?, &[9.5, 4.25, 4.25], 1e-6);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_top_k_for_multiple_rows_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = backend.input_buffer(
            Shape::new(vec![2, 5]),
            vec![0.5_f32, 7.0, -1.0, 7.0, 3.0, 4.0, 2.0, 9.0, 8.0, 8.0],
        )?;
        let selected_indices = backend.i32_buffer(4)?;
        let selected_values = backend.f32_buffer(4)?;
        let mut submission = backend.begin_submission()?;
        submission.top_k_f32(&input, 2, 5, 2, &selected_indices, &selected_values)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);
        let selected_indices = selected_indices
            .read_bytes()?
            .chunks_exact(std::mem::size_of::<i32>())
            .map(|chunk| i32::from_ne_bytes(chunk.try_into().expect("top_k i32 bytes")))
            .collect::<Vec<_>>();
        assert_eq!(selected_indices, vec![1, 3, 2, 3]);
        assert_close(&selected_values.read_f32()?, &[7.0, 7.0, 9.0, 8.0], 1e-6);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_top_k_for_wide_one_row_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let values = (0..257)
            .map(|index| {
                let bucket = ((index * 73) % 257) as f32;
                bucket - ((index % 11) as f32 * 0.01)
            })
            .collect::<Vec<_>>();
        let input = backend.input_buffer(Shape::new(vec![values.len()]), values.clone())?;
        let top_k = 100usize;
        let selected_indices = backend.i32_buffer(top_k)?;
        let selected_values = backend.f32_buffer(top_k)?;
        let mut submission = backend.begin_submission()?;
        submission.top_k_f32(
            &input,
            1,
            values.len(),
            top_k,
            &selected_indices,
            &selected_values,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);

        let mut expected = values.iter().copied().enumerate().collect::<Vec<_>>();
        expected.sort_by(|(left_index, left_value), (right_index, right_value)| {
            right_value
                .total_cmp(left_value)
                .then_with(|| left_index.cmp(right_index))
        });
        expected.truncate(top_k);

        let selected_indices = selected_indices
            .read_bytes()?
            .chunks_exact(std::mem::size_of::<i32>())
            .map(|chunk| i32::from_ne_bytes(chunk.try_into().expect("top_k i32 bytes")))
            .collect::<Vec<_>>();
        let expected_indices = expected
            .iter()
            .map(|(index, _)| i32::try_from(*index).expect("expected index fits in i32"))
            .collect::<Vec<_>>();
        assert_eq!(selected_indices, expected_indices);
        let expected_values = expected.iter().map(|(_, value)| *value).collect::<Vec<_>>();
        assert_close(&selected_values.read_f32()?, &expected_values, 1e-6);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_partitioned_top_k_for_wide_one_row_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let values = (0..4097)
            .map(|index| {
                let bucket = ((index * 73) % 4097) as f32;
                bucket - ((index % 11) as f32 * 0.01)
            })
            .collect::<Vec<_>>();
        let input = backend.input_buffer(Shape::new(vec![values.len()]), values.clone())?;
        let top_k = 100usize;
        let partial_block_count = 8usize;
        let partial_len = top_k * partial_block_count;
        let partial_indices = backend.i32_buffer(partial_len)?;
        let partial_values = backend.f32_buffer(partial_len)?;
        let selected_indices = backend.i32_buffer(top_k)?;
        let selected_values = backend.f32_buffer(top_k)?;
        let mut submission = backend.begin_submission()?;
        submission.top_k_f32_one_row_partitioned(
            &input,
            values.len(),
            top_k,
            partial_block_count,
            &partial_indices,
            &partial_values,
            &selected_indices,
            &selected_values,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 3);

        let mut expected = values.iter().copied().enumerate().collect::<Vec<_>>();
        expected.sort_by(|(left_index, left_value), (right_index, right_value)| {
            right_value
                .total_cmp(left_value)
                .then_with(|| left_index.cmp(right_index))
        });
        expected.truncate(top_k);

        let selected_indices = selected_indices
            .read_bytes()?
            .chunks_exact(std::mem::size_of::<i32>())
            .map(|chunk| i32::from_ne_bytes(chunk.try_into().expect("top_k i32 bytes")))
            .collect::<Vec<_>>();
        let expected_indices = expected
            .iter()
            .map(|(index, _)| i32::try_from(*index).expect("expected index fits in i32"))
            .collect::<Vec<_>>();
        assert_eq!(selected_indices, expected_indices);
        let expected_values = expected.iter().map(|(_, value)| *value).collect::<Vec<_>>();
        assert_close(&selected_values.read_f32()?, &expected_values, 1e-6);
        Ok(())
    }

    #[test]
    fn cuda_submission_applies_sampling_penalties_to_sparse_logits_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let logits = backend.input_buffer(Shape::new(vec![4]), vec![1.0_f32, 3.0, 2.5, -0.5])?;
        let token_ids = backend.input_i32_buffer(Shape::new(vec![2]), vec![1_i32, 3])?;
        let token_counts = backend.input_i32_buffer(Shape::new(vec![2]), vec![2_i32, 1])?;
        let mut submission = backend.begin_submission()?;
        submission.apply_sampling_penalties_f32_sparse(
            &logits,
            4,
            &token_ids,
            &token_counts,
            2,
            2.0,
            0.5,
            0.5,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);

        let mut expected = vec![1.0_f32, 3.0, 2.5, -0.5];
        psionic_runtime::apply_sampling_penalties(
            expected.as_mut_slice(),
            &[1_u32, 1, 3],
            &psionic_runtime::SamplingPolicy {
                strategy: psionic_runtime::SamplingStrategy::Greedy,
                temperature: None,
                top_k: None,
                top_p: None,
                min_p: None,
                typical_p: None,
                mirostat: None,
                mirostat_tau: None,
                mirostat_eta: None,
                repeat_penalty: Some(2.0),
                repeat_last_n: Some(3),
                presence_penalty: Some(0.5),
                frequency_penalty: Some(0.5),
                seed: None,
            },
        );
        assert_close(&logits.read_f32()?, &expected, 1e-6);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_one_row_radix_sort_top_k_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = backend.input_buffer(
            Shape::new(vec![6]),
            vec![1.0_f32, -2.0, 4.25, 3.0, 9.5, 4.25],
        )?;
        let input_indices = backend.byte_buffer(&i32_slice_to_bytes(&[0, 1, 2, 3, 4, 5]))?;
        let sorted_values = backend.f32_buffer(6)?;
        let sorted_indices = backend.i32_buffer(6)?;
        let temp_storage_bytes = backend.top_k_f32_one_row_radix_sort_temp_storage_bytes(6)?;
        let temp_storage = backend.byte_buffer(&vec![0_u8; temp_storage_bytes])?;
        let selected_indices = backend.i32_buffer(3)?;
        let selected_values = backend.f32_buffer(3)?;
        let mut submission = backend.begin_submission()?;
        submission.top_k_f32_one_row_radix_sort(
            &input,
            6,
            3,
            &input_indices,
            &sorted_values,
            &sorted_indices,
            &temp_storage,
            temp_storage_bytes,
            &selected_indices,
            &selected_values,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);
        let selected_indices = selected_indices
            .read_bytes()?
            .chunks_exact(std::mem::size_of::<i32>())
            .map(|chunk| i32::from_ne_bytes(chunk.try_into().expect("top_k i32 bytes")))
            .collect::<Vec<_>>();
        assert_eq!(selected_indices, vec![4, 2, 5]);
        assert_close(&selected_values.read_f32()?, &[9.5, 4.25, 4.25], 1e-6);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_add_residual_rms_norm_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = vec![1.0_f32, -2.0, 3.0, 0.5];
        let residual = vec![0.25_f32, 0.5, -1.0, 2.0];
        let weight = vec![1.0_f32, 0.5, 2.0, 1.5];
        let combined = input
            .iter()
            .zip(residual.iter())
            .map(|(left, right)| left + right)
            .collect::<Vec<_>>();
        let mean_square =
            combined.iter().map(|value| value * value).sum::<f32>() / combined.len() as f32;
        let inv_rms = (mean_square + 1e-5_f32).sqrt().recip();
        let expected = combined
            .iter()
            .zip(weight.iter())
            .map(|(value, scale)| value * scale * inv_rms)
            .collect::<Vec<_>>();

        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let residual_buffer = backend.input_buffer(Shape::new(vec![residual.len()]), residual)?;
        let weight_buffer = backend.input_buffer(Shape::new(vec![weight.len()]), weight)?;
        let summed_output = backend.f32_buffer(expected.len())?;
        let normalized_output = backend.f32_buffer(expected.len())?;

        let mut submission = backend.begin_submission()?;
        submission.add_residual_rms_norm(
            &input_buffer,
            &residual_buffer,
            None,
            &weight_buffer,
            &summed_output,
            &normalized_output,
            expected.len(),
            1e-5,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);
        assert_close(&summed_output.read_f32()?, &combined, 1e-6);
        assert_close(&normalized_output.read_f32()?, &expected, 1e-5);
        Ok(())
    }

    #[test]
    fn cuda_submission_fused_attention_matches_separate_rope_attention_and_cache_path_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let head_count = 2usize;
        let kv_head_count = 1usize;
        let head_dim = 4usize;
        let rotary_dim = 4usize;
        let q_rows = head_count * head_dim;
        let k_rows = kv_head_count * head_dim;
        let v_rows = kv_head_count * head_dim;
        let qkv = vec![
            0.5_f32, -1.0, 0.75, 0.25, 1.25, 0.5, -0.5, 2.0, -0.25, 1.5, 0.75, -1.25, 0.5, 1.0,
            -0.5, 2.0,
        ];
        let freq_scale = 1.0_f32;
        let ext_factor = 0.0_f32;
        let corr_dims = [0.0_f32, rotary_dim as f32 - 1.0];
        let theta_scale = 10000.0_f32.powf(-2.0 / rotary_dim as f32);

        let qkv_separate = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv.clone())?;
        let qkv_fused = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv)?;
        let cache_keys_separate = backend.f32_buffer(k_rows)?;
        let cache_values_separate = backend.f32_buffer(v_rows)?;
        let cache_keys_fused = backend.f32_buffer(k_rows)?;
        let cache_values_fused = backend.f32_buffer(v_rows)?;
        let output_separate = backend.f32_buffer(q_rows)?;
        let output_fused = backend.f32_buffer(q_rows)?;

        let mut separate = backend.begin_submission()?;
        separate.rope_neox_in_place(
            &qkv_separate,
            0,
            head_count,
            head_dim,
            rotary_dim,
            3,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
        )?;
        separate.rope_neox_in_place(
            &qkv_separate,
            q_rows,
            kv_head_count,
            head_dim,
            rotary_dim,
            3,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
        )?;
        separate.attention_decode(
            &qkv_separate,
            0,
            &qkv_separate,
            q_rows,
            &qkv_separate,
            q_rows + k_rows,
            &cache_keys_separate,
            &cache_values_separate,
            k_rows,
            0,
            0,
            0,
            head_count,
            kv_head_count,
            head_dim,
            None,
            &output_separate,
        )?;
        separate.copy_buffer_region(
            &qkv_separate,
            q_rows * std::mem::size_of::<f32>(),
            &cache_keys_separate,
            0,
            k_rows * std::mem::size_of::<f32>(),
        )?;
        separate.copy_buffer_region(
            &qkv_separate,
            (q_rows + k_rows) * std::mem::size_of::<f32>(),
            &cache_values_separate,
            0,
            v_rows * std::mem::size_of::<f32>(),
        )?;
        let separate_report = separate.commit(CudaCommandWait::Completed)?;
        assert_eq!(separate_report.encoded_operations, 5);

        let mut fused = backend.begin_submission()?;
        fused.attention_decode_rope_cache(
            &qkv_fused,
            0,
            q_rows,
            q_rows + k_rows,
            &cache_keys_fused,
            &cache_values_fused,
            k_rows,
            0,
            0,
            0,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            3,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            None,
            &output_fused,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 1);

        assert_close(
            &output_fused.read_f32()?,
            &output_separate.read_f32()?,
            1e-5,
        );
        assert_close(
            &cache_keys_fused.read_f32()?,
            &cache_keys_separate.read_f32()?,
            1e-5,
        );
        assert_close(
            &cache_values_fused.read_f32()?,
            &cache_values_separate.read_f32()?,
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_f16_kv_mrope_uses_each_interleaved_axis_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let head_dim = 6usize;
        let query = [0.5_f32, -0.25, 0.75, 1.0, -0.5, 0.125];
        let key = [1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0];
        let value = [0.25_f32, 0.5, 0.75, 1.0, 1.25, 1.5];
        let qkv = backend.input_buffer(
            Shape::new(vec![head_dim * 3]),
            query
                .into_iter()
                .chain(key)
                .chain(value)
                .collect::<Vec<_>>(),
        )?;
        let cache_keys = backend.f16_buffer(head_dim)?;
        let cache_values = backend.f16_buffer(head_dim)?;
        let output = backend.f32_buffer(head_dim)?;
        let theta_scale = 0.5_f32;

        let mut submission = backend.begin_submission()?;
        submission.attention_decode_mrope_cache_f16_kv(
            &qkv,
            0,
            head_dim,
            head_dim * 2,
            &cache_keys,
            &cache_values,
            head_dim,
            0,
            0,
            0,
            1,
            1,
            head_dim,
            head_dim,
            [2, 3, 4],
            [1, 1, 1, 0],
            true,
            1.0,
            0.0,
            [0.0, 0.0],
            theta_scale,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);

        let expected_key = [
            (key[0] * 2.0_f32.cos()) - (key[3] * 2.0_f32.sin()),
            (key[1] * 1.5_f32.cos()) - (key[4] * 1.5_f32.sin()),
            (key[2] * 1.0_f32.cos()) - (key[5] * 1.0_f32.sin()),
            (key[0] * 2.0_f32.sin()) + (key[3] * 2.0_f32.cos()),
            (key[1] * 1.5_f32.sin()) + (key[4] * 1.5_f32.cos()),
            (key[2] * 1.0_f32.sin()) + (key[5] * 1.0_f32.cos()),
        ];
        let actual_key = cache_keys
            .read_bytes()?
            .chunks_exact(std::mem::size_of::<u16>())
            .map(|bytes| {
                half::f16::from_bits(u16::from_ne_bytes(
                    bytes.try_into().expect("f16 cache key bytes"),
                ))
                .to_f32()
            })
            .collect::<Vec<_>>();
        assert_close(actual_key.as_slice(), &expected_key, 5e-3);
        assert_close(&output.read_f32()?, &value, 1e-6);
        Ok(())
    }

    #[test]
    fn cuda_submission_f16_kv_mrope_initializes_logits_beyond_one_block_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let head_dim = 6usize;
        let past_tokens = 257usize;
        let cache_token_capacity = past_tokens + 1;
        let mut qkv = vec![0.0_f32; head_dim * 3];
        qkv[head_dim * 2..].fill(3.0);
        let cache_keys = backend.byte_buffer(&vec![
            0_u8;
            cache_token_capacity
                * head_dim
                * std::mem::size_of::<u16>()
        ])?;
        let cache_values = backend.byte_buffer(&f32_slice_to_f16_le_bytes(&vec![
            1.0_f32;
            cache_token_capacity
                * head_dim
        ]))?;
        let qkv = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv)?;
        let output = backend.f32_buffer(head_dim)?;

        let mut submission = backend.begin_submission()?;
        submission.attention_decode_mrope_cache_f16_kv(
            &qkv,
            0,
            head_dim,
            head_dim * 2,
            &cache_keys,
            &cache_values,
            head_dim,
            0,
            past_tokens,
            0,
            1,
            1,
            head_dim,
            head_dim,
            [past_tokens; 3],
            [1, 1, 1, 0],
            true,
            1.0,
            0.0,
            [0.0, 0.0],
            0.5,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);

        let expected = (past_tokens as f32 + 3.0) / (past_tokens + 1) as f32;
        let actual = output.read_f32()?;
        assert!(actual.iter().all(|value| value.is_finite()));
        assert_close(actual.as_slice(), vec![expected; head_dim].as_slice(), 1e-5);
        Ok(())
    }

    #[test]
    fn cuda_submission_fused_attention_f16_kv_q8_1_matches_separate_attention_and_quantize_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let head_count = 2usize;
        let kv_head_count = 1usize;
        let head_dim = 32usize;
        let rotary_dim = 32usize;
        let q_rows = head_count * head_dim;
        let k_rows = kv_head_count * head_dim;
        let v_rows = kv_head_count * head_dim;
        let qkv = (0..(q_rows + k_rows + v_rows))
            .map(|index| ((index as f32 % 19.0) - 9.0) * 0.125)
            .collect::<Vec<_>>();
        let previous_keys = (0..k_rows)
            .map(|index| ((index as f32 % 13.0) - 6.0) * 0.2)
            .collect::<Vec<_>>();
        let previous_values = (0..v_rows)
            .map(|index| ((index as f32 % 11.0) - 5.0) * 0.15)
            .collect::<Vec<_>>();
        let cache_token_capacity = 2usize;
        let mut cache_keys_bytes =
            vec![0_u8; cache_token_capacity * k_rows * std::mem::size_of::<u16>()];
        let mut cache_values_bytes =
            vec![0_u8; cache_token_capacity * v_rows * std::mem::size_of::<u16>()];
        let previous_key_bytes = f32_slice_to_f16_le_bytes(&previous_keys);
        let previous_value_bytes = f32_slice_to_f16_le_bytes(&previous_values);
        cache_keys_bytes[..previous_key_bytes.len()].copy_from_slice(&previous_key_bytes);
        cache_values_bytes[..previous_value_bytes.len()].copy_from_slice(&previous_value_bytes);

        let qkv_separate = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv.clone())?;
        let qkv_fused = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv)?;
        let cache_keys_separate = backend.byte_buffer(&cache_keys_bytes)?;
        let cache_values_separate = backend.byte_buffer(&cache_values_bytes)?;
        let cache_keys_fused = backend.byte_buffer(&cache_keys_bytes)?;
        let cache_values_fused = backend.byte_buffer(&cache_values_bytes)?;
        let output_separate = backend.f32_buffer(q_rows)?;
        let output_q8_1_separate =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, q_rows)?])?;
        let output_q8_1_fused =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, q_rows)?])?;
        let freq_scale = 1.0_f32;
        let ext_factor = 0.0_f32;
        let corr_dims = [0.0_f32, 0.0_f32];
        let theta_scale = 0.5_f32;

        let mut separate = backend.begin_submission()?;
        separate.attention_decode_rope_cache_f16_kv(
            &qkv_separate,
            0,
            q_rows,
            q_rows + k_rows,
            &cache_keys_separate,
            &cache_values_separate,
            k_rows,
            0,
            1,
            0,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            5,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            None,
            &output_separate,
        )?;
        separate.quantize_f32_to_q8_1(&output_separate, 1, q_rows, &output_q8_1_separate)?;
        let separate_report = separate.commit(CudaCommandWait::Completed)?;
        assert_eq!(separate_report.encoded_operations, 2);

        let mut fused = backend.begin_submission()?;
        fused.attention_decode_rope_cache_f16_kv_q8_1(
            &qkv_fused,
            0,
            q_rows,
            q_rows + k_rows,
            &cache_keys_fused,
            &cache_values_fused,
            k_rows,
            0,
            1,
            0,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            5,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            None,
            &output_q8_1_fused,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 1);

        assert_eq!(
            output_q8_1_fused.read_bytes()?,
            output_q8_1_separate.read_bytes()?
        );
        assert_eq!(
            cache_keys_fused.read_bytes()?,
            cache_keys_separate.read_bytes()?
        );
        assert_eq!(
            cache_values_fused.read_bytes()?,
            cache_values_separate.read_bytes()?
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_fused_attention_graph_f16_kv_q8_1_matches_separate_attention_and_quantize_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let head_count = 2usize;
        let kv_head_count = 1usize;
        let head_dim = 32usize;
        let rotary_dim = 32usize;
        let q_rows = head_count * head_dim;
        let k_rows = kv_head_count * head_dim;
        let v_rows = kv_head_count * head_dim;
        let qkv = (0..(q_rows + k_rows + v_rows))
            .map(|index| ((index as f32 % 23.0) - 11.0) * 0.11)
            .collect::<Vec<_>>();
        let previous_keys = (0..k_rows)
            .map(|index| ((index as f32 % 17.0) - 8.0) * 0.14)
            .collect::<Vec<_>>();
        let previous_values = (0..v_rows)
            .map(|index| ((index as f32 % 7.0) - 3.0) * 0.21)
            .collect::<Vec<_>>();
        let cache_token_capacity = 2usize;
        let mut cache_keys_bytes =
            vec![0_u8; cache_token_capacity * k_rows * std::mem::size_of::<u16>()];
        let mut cache_values_bytes =
            vec![0_u8; cache_token_capacity * v_rows * std::mem::size_of::<u16>()];
        let previous_key_bytes = f32_slice_to_f16_le_bytes(&previous_keys);
        let previous_value_bytes = f32_slice_to_f16_le_bytes(&previous_values);
        cache_keys_bytes[..previous_key_bytes.len()].copy_from_slice(&previous_key_bytes);
        cache_values_bytes[..previous_value_bytes.len()].copy_from_slice(&previous_value_bytes);

        let qkv_separate = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv.clone())?;
        let qkv_fused = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv)?;
        let cache_keys_separate = backend.byte_buffer(&cache_keys_bytes)?;
        let cache_values_separate = backend.byte_buffer(&cache_values_bytes)?;
        let cache_keys_fused = backend.byte_buffer(&cache_keys_bytes)?;
        let cache_values_fused = backend.byte_buffer(&cache_values_bytes)?;
        let decode_params = backend.byte_buffer(&i32_slice_to_bytes(&[1_i32, 6_i32]))?;
        let output_separate = backend.f32_buffer(q_rows)?;
        let output_q8_1_separate =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, q_rows)?])?;
        let output_q8_1_fused =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, q_rows)?])?;
        let freq_scale = 1.0_f32;
        let ext_factor = 0.0_f32;
        let corr_dims = [0.0_f32, 0.0_f32];
        let theta_scale = 0.5_f32;

        let mut separate = backend.begin_submission()?;
        separate.attention_decode_rope_cache_f16_kv_graph(
            &qkv_separate,
            0,
            q_rows,
            q_rows + k_rows,
            &cache_keys_separate,
            &cache_values_separate,
            k_rows,
            0,
            &decode_params,
            0,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            None,
            &output_separate,
        )?;
        separate.quantize_f32_to_q8_1(&output_separate, 1, q_rows, &output_q8_1_separate)?;
        let separate_report = separate.commit(CudaCommandWait::Completed)?;
        assert_eq!(separate_report.encoded_operations, 2);

        let mut fused = backend.begin_submission()?;
        fused.attention_decode_rope_cache_f16_kv_graph_q8_1(
            &qkv_fused,
            0,
            q_rows,
            q_rows + k_rows,
            &cache_keys_fused,
            &cache_values_fused,
            k_rows,
            0,
            &decode_params,
            0,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            None,
            &output_q8_1_fused,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 1);

        assert_eq!(
            output_q8_1_fused.read_bytes()?,
            output_q8_1_separate.read_bytes()?
        );
        assert_eq!(
            cache_keys_fused.read_bytes()?,
            cache_keys_separate.read_bytes()?
        );
        assert_eq!(
            cache_values_fused.read_bytes()?,
            cache_values_separate.read_bytes()?
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_fused_attention_graph_fa3_f16_kv_matches_legacy_reference_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }
        let supports_fa3 = selected
            .nvidia_metadata
            .as_ref()
            .and_then(|metadata| metadata.topology.compute_capability.as_deref())
            .and_then(|compute_capability| compute_capability.split('.').next())
            .and_then(|major| major.parse::<u32>().ok())
            .map_or_else(
                || {
                    matches!(
                        selected
                            .nvidia_metadata
                            .as_ref()
                            .and_then(|metadata| metadata.topology.architecture.as_deref()),
                        Some("Ada Lovelace" | "Hopper" | "Blackwell")
                    )
                },
                |major| major >= 8,
            );
        if !supports_fa3 {
            return Ok(());
        }

        let head_count = 4usize;
        let kv_head_count = 2usize;
        let head_dim = 128usize;
        let rotary_dim = 64usize;
        let q_rows = head_count * head_dim;
        let k_rows = kv_head_count * head_dim;
        let v_rows = kv_head_count * head_dim;
        let past_tokens = 640usize;
        let cache_token_capacity = past_tokens + 1;
        let qkv = (0..(q_rows + k_rows + v_rows))
            .map(|index| (((index % 67) as f32) - 33.0) * 0.03125)
            .collect::<Vec<_>>();
        let previous_keys = (0..(past_tokens * k_rows))
            .map(|index| {
                ((((index / k_rows) % 29) as f32) - 14.0) * 0.015625
                    + (((index % 17) as f32) - 8.0) * 0.0078125
            })
            .collect::<Vec<_>>();
        let previous_values = (0..(past_tokens * v_rows))
            .map(|index| {
                ((((index / v_rows) % 19) as f32) - 9.0) * 0.0234375
                    - (((index % 13) as f32) - 6.0) * 0.00390625
            })
            .collect::<Vec<_>>();
        let mut cache_keys_bytes =
            vec![0_u8; cache_token_capacity * k_rows * std::mem::size_of::<u16>()];
        let mut cache_values_bytes =
            vec![0_u8; cache_token_capacity * v_rows * std::mem::size_of::<u16>()];
        let previous_key_bytes = f32_slice_to_f16_le_bytes(&previous_keys);
        let previous_value_bytes = f32_slice_to_f16_le_bytes(&previous_values);
        cache_keys_bytes[..previous_key_bytes.len()].copy_from_slice(&previous_key_bytes);
        cache_values_bytes[..previous_value_bytes.len()].copy_from_slice(&previous_value_bytes);

        let qkv_buffer = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv)?;
        let cache_keys_dense = backend.byte_buffer(&cache_keys_bytes)?;
        let cache_values_dense = backend.byte_buffer(&cache_values_bytes)?;
        let cache_keys_fa3 = backend.byte_buffer(&cache_keys_bytes)?;
        let cache_values_fa3 = backend.byte_buffer(&cache_values_bytes)?;
        let decode_params = backend.byte_buffer(&i32_slice_to_bytes(&[
            i32::try_from(past_tokens)?,
            i32::try_from(past_tokens + 3)?,
        ]))?;
        let output_dense = backend.f32_buffer(q_rows)?;
        let output_fa3 = backend.f32_buffer(q_rows)?;
        let partial_output = backend.f32_buffer(crate::ATTENTION_FA3_MAX_SPLITS * q_rows)?;
        let partial_max = backend.f32_buffer(crate::ATTENTION_FA3_MAX_SPLITS * head_count)?;
        let partial_sum = backend.f32_buffer(crate::ATTENTION_FA3_MAX_SPLITS * head_count)?;
        let freq_scale = 1.0_f32;
        let ext_factor = 0.0_f32;
        let corr_dims = [0.0_f32, 0.0_f32];
        let theta_scale = 0.5_f32;

        let mut dense = backend.begin_submission()?;
        dense.attention_decode_rope_cache_f16_kv_graph(
            &qkv_buffer,
            0,
            q_rows,
            q_rows + k_rows,
            &cache_keys_dense,
            &cache_values_dense,
            k_rows,
            0,
            &decode_params,
            0,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            None,
            &output_dense,
        )?;
        let dense_report = dense.commit(CudaCommandWait::Completed)?;
        assert_eq!(dense_report.encoded_operations, 1);

        let mut fa3 = backend.begin_submission()?;
        fa3.attention_decode_rope_cache_f16_kv_graph_fa3(
            &qkv_buffer,
            0,
            q_rows,
            q_rows + k_rows,
            &cache_keys_fa3,
            &cache_values_fa3,
            k_rows,
            0,
            &decode_params,
            0,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            None,
            &partial_output,
            &partial_max,
            &partial_sum,
            &output_fa3,
        )?;
        let fa3_report = fa3.commit(CudaCommandWait::Completed)?;
        assert_eq!(fa3_report.encoded_operations, 1);

        assert_close(&output_fa3.read_f32()?, &output_dense.read_f32()?, 1e-3);
        assert_eq!(cache_keys_fa3.read_bytes()?, cache_keys_dense.read_bytes()?);
        assert_eq!(
            cache_values_fa3.read_bytes()?,
            cache_values_dense.read_bytes()?
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_fused_attention_turboquant_kv_matches_f16_kv_reference_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let head_count = 2usize;
        let kv_head_count = 1usize;
        let head_dim = 32usize;
        let rotary_dim = 32usize;
        let q_rows = head_count * head_dim;
        let k_rows = kv_head_count * head_dim;
        let v_rows = kv_head_count * head_dim;
        let exact_block = sample_q8_1_exact_vector()
            .into_iter()
            .map(|value| value * 0.125)
            .collect::<Vec<_>>();
        let reversed_block = exact_block.iter().copied().rev().collect::<Vec<_>>();
        let query = exact_block
            .iter()
            .copied()
            .chain(reversed_block.iter().copied())
            .collect::<Vec<_>>();
        let qkv = query
            .iter()
            .copied()
            .chain(reversed_block.iter().copied())
            .chain(exact_block.iter().copied())
            .collect::<Vec<_>>();
        let previous_keys = exact_block.clone();
        let previous_values = reversed_block.clone();
        let cache_token_capacity = 2usize;
        let mut cache_keys_bytes =
            vec![0_u8; cache_token_capacity * k_rows * std::mem::size_of::<u16>()];
        let mut cache_values_bytes =
            vec![0_u8; cache_token_capacity * v_rows * std::mem::size_of::<u16>()];
        let previous_key_bytes = f32_slice_to_f16_le_bytes(&previous_keys);
        let previous_value_bytes = f32_slice_to_f16_le_bytes(&previous_values);
        cache_keys_bytes[..previous_key_bytes.len()].copy_from_slice(&previous_key_bytes);
        cache_values_bytes[..previous_value_bytes.len()].copy_from_slice(&previous_value_bytes);

        let qkv_buffer = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv)?;
        let cache_keys_dense = backend.byte_buffer(&cache_keys_bytes)?;
        let cache_values_dense = backend.byte_buffer(&cache_values_bytes)?;
        let output_dense = backend.f32_buffer(q_rows)?;

        let q8_1_row_bytes = crate::ggml_q8_1_storage_bytes(1, k_rows)?;
        let cache_keys_turbo =
            backend.byte_buffer(&vec![0_u8; cache_token_capacity * q8_1_row_bytes])?;
        let cache_values_turbo =
            backend.byte_buffer(&vec![0_u8; cache_token_capacity * q8_1_row_bytes])?;
        let previous_keys_buffer =
            backend.input_buffer(Shape::new(vec![previous_keys.len()]), previous_keys)?;
        let previous_values_buffer =
            backend.input_buffer(Shape::new(vec![previous_values.len()]), previous_values)?;
        let mut init = backend.begin_submission()?;
        init.quantize_f32_to_q8_1(&previous_keys_buffer, 1, k_rows, &cache_keys_turbo)?;
        init.quantize_f32_to_q8_1(&previous_values_buffer, 1, v_rows, &cache_values_turbo)?;
        let init_report = init.commit(CudaCommandWait::Completed)?;
        assert_eq!(init_report.encoded_operations, 2);

        let output_turbo = backend.f32_buffer(q_rows)?;
        let freq_scale = 1.0_f32;
        let ext_factor = 0.0_f32;
        let corr_dims = [0.0_f32, 0.0_f32];
        let theta_scale = 0.5_f32;

        let mut dense = backend.begin_submission()?;
        dense.attention_decode_rope_cache_f16_kv(
            &qkv_buffer,
            0,
            q_rows,
            q_rows + k_rows,
            &cache_keys_dense,
            &cache_values_dense,
            k_rows,
            0,
            1,
            0,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            5,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            None,
            &output_dense,
        )?;
        let dense_report = dense.commit(CudaCommandWait::Completed)?;
        assert_eq!(dense_report.encoded_operations, 1);

        let mut turbo = backend.begin_submission()?;
        turbo.attention_decode_rope_cache_turboquant_kv(
            &qkv_buffer,
            0,
            q_rows,
            q_rows + k_rows,
            &cache_keys_turbo,
            &cache_values_turbo,
            k_rows,
            0,
            1,
            0,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            5,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            None,
            &output_turbo,
        )?;
        let turbo_report = turbo.commit(CudaCommandWait::Completed)?;
        assert_eq!(turbo_report.encoded_operations, 1);

        assert_close(&output_turbo.read_f32()?, &output_dense.read_f32()?, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_fused_attention_graph_turboquant_kv_matches_f16_kv_reference_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let head_count = 2usize;
        let kv_head_count = 1usize;
        let head_dim = 32usize;
        let rotary_dim = 32usize;
        let q_rows = head_count * head_dim;
        let k_rows = kv_head_count * head_dim;
        let v_rows = kv_head_count * head_dim;
        let exact_block = sample_q8_1_exact_vector()
            .into_iter()
            .map(|value| value * 0.125)
            .collect::<Vec<_>>();
        let reversed_block = exact_block.iter().copied().rev().collect::<Vec<_>>();
        let query = exact_block
            .iter()
            .copied()
            .chain(reversed_block.iter().copied())
            .collect::<Vec<_>>();
        let qkv = query
            .iter()
            .copied()
            .chain(exact_block.iter().copied())
            .chain(reversed_block.iter().copied())
            .collect::<Vec<_>>();
        let previous_keys = reversed_block.clone();
        let previous_values = exact_block.clone();
        let cache_token_capacity = 2usize;
        let mut cache_keys_bytes =
            vec![0_u8; cache_token_capacity * k_rows * std::mem::size_of::<u16>()];
        let mut cache_values_bytes =
            vec![0_u8; cache_token_capacity * v_rows * std::mem::size_of::<u16>()];
        let previous_key_bytes = f32_slice_to_f16_le_bytes(&previous_keys);
        let previous_value_bytes = f32_slice_to_f16_le_bytes(&previous_values);
        cache_keys_bytes[..previous_key_bytes.len()].copy_from_slice(&previous_key_bytes);
        cache_values_bytes[..previous_value_bytes.len()].copy_from_slice(&previous_value_bytes);

        let qkv_buffer = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv)?;
        let cache_keys_dense = backend.byte_buffer(&cache_keys_bytes)?;
        let cache_values_dense = backend.byte_buffer(&cache_values_bytes)?;
        let output_dense = backend.f32_buffer(q_rows)?;

        let q8_1_row_bytes = crate::ggml_q8_1_storage_bytes(1, k_rows)?;
        let cache_keys_turbo =
            backend.byte_buffer(&vec![0_u8; cache_token_capacity * q8_1_row_bytes])?;
        let cache_values_turbo =
            backend.byte_buffer(&vec![0_u8; cache_token_capacity * q8_1_row_bytes])?;
        let previous_keys_buffer =
            backend.input_buffer(Shape::new(vec![previous_keys.len()]), previous_keys)?;
        let previous_values_buffer =
            backend.input_buffer(Shape::new(vec![previous_values.len()]), previous_values)?;
        let mut init = backend.begin_submission()?;
        init.quantize_f32_to_q8_1(&previous_keys_buffer, 1, k_rows, &cache_keys_turbo)?;
        init.quantize_f32_to_q8_1(&previous_values_buffer, 1, v_rows, &cache_values_turbo)?;
        let init_report = init.commit(CudaCommandWait::Completed)?;
        assert_eq!(init_report.encoded_operations, 2);

        let output_turbo = backend.f32_buffer(q_rows)?;
        let decode_params = backend.byte_buffer(&i32_slice_to_bytes(&[1_i32, 6_i32]))?;
        let freq_scale = 1.0_f32;
        let ext_factor = 0.0_f32;
        let corr_dims = [0.0_f32, 0.0_f32];
        let theta_scale = 0.5_f32;

        let mut dense = backend.begin_submission()?;
        dense.attention_decode_rope_cache_f16_kv_graph(
            &qkv_buffer,
            0,
            q_rows,
            q_rows + k_rows,
            &cache_keys_dense,
            &cache_values_dense,
            k_rows,
            0,
            &decode_params,
            0,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            None,
            &output_dense,
        )?;
        let dense_report = dense.commit(CudaCommandWait::Completed)?;
        assert_eq!(dense_report.encoded_operations, 1);

        let mut turbo = backend.begin_submission()?;
        turbo.attention_decode_rope_cache_turboquant_kv_graph(
            &qkv_buffer,
            0,
            q_rows,
            q_rows + k_rows,
            &cache_keys_turbo,
            &cache_values_turbo,
            k_rows,
            0,
            &decode_params,
            0,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            None,
            &output_turbo,
        )?;
        let turbo_report = turbo.commit(CudaCommandWait::Completed)?;
        assert_eq!(turbo_report.encoded_operations, 1);

        assert_close(&output_turbo.read_f32()?, &output_dense.read_f32()?, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_fused_add_residual_rms_norm_q8_1_router_topk_matches_separate_kernels_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let element_count = 32usize;
        let expert_count = 4usize;
        let top_k = 2usize;
        let input = (0..element_count)
            .map(|index| ((index as f32 % 9.0) - 4.0) * 0.35)
            .collect::<Vec<_>>();
        let residual = (0..element_count)
            .map(|index| ((index as f32 % 11.0) - 5.0) * 0.2)
            .collect::<Vec<_>>();
        let input_bias = (0..element_count)
            .map(|index| ((index as f32 % 7.0) - 3.0) * 0.05)
            .collect::<Vec<_>>();
        let weight = (0..element_count)
            .map(|index| 0.8 + index as f32 * 0.01)
            .collect::<Vec<_>>();
        let router_weights = (0..expert_count)
            .flat_map(|expert| {
                (0..element_count).map(move |index| {
                    ((expert as f32 + 1.0) * 0.09) + ((index as f32 % 13.0) - 6.0) * 0.015
                })
            })
            .collect::<Vec<_>>();
        let router_bias = vec![0.25_f32, -0.5_f32, 0.1_f32, -0.15_f32];

        let input_buffer = backend.input_buffer(Shape::new(vec![element_count]), input)?;
        let residual_buffer = backend.input_buffer(Shape::new(vec![element_count]), residual)?;
        let input_bias_buffer =
            backend.input_buffer(Shape::new(vec![element_count]), input_bias)?;
        let weight_buffer = backend.input_buffer(Shape::new(vec![element_count]), weight)?;
        let router_weights_buffer = backend.input_buffer(
            Shape::new(vec![expert_count, element_count]),
            router_weights,
        )?;
        let router_bias_buffer =
            backend.input_buffer(Shape::new(vec![expert_count]), router_bias)?;
        let q8_1_bytes = crate::ggml_q8_1_storage_bytes(1, element_count)?;

        let summed_separate = backend.f32_buffer(element_count)?;
        let normalized_separate = backend.f32_buffer(element_count)?;
        let quantized_separate = backend.byte_buffer(&vec![0_u8; q8_1_bytes])?;
        let selected_ids_separate = backend.byte_buffer(&vec![0_u8; top_k * size_of::<i32>()])?;
        let selected_weights_separate = backend.f32_buffer(top_k)?;

        let summed_fused = backend.f32_buffer(element_count)?;
        let normalized_fused = backend.f32_buffer(element_count)?;
        let quantized_fused = backend.byte_buffer(&vec![0_u8; q8_1_bytes])?;
        let selected_ids_fused = backend.byte_buffer(&vec![0_u8; top_k * size_of::<i32>()])?;
        let selected_weights_fused = backend.f32_buffer(top_k)?;

        let mut separate = backend.begin_submission()?;
        separate.add_residual_rms_norm_q8_1(
            &input_buffer,
            &residual_buffer,
            Some(&input_bias_buffer),
            &weight_buffer,
            &summed_separate,
            &normalized_separate,
            &quantized_separate,
            element_count,
            1.0e-5,
        )?;
        separate.router_topk_softmax(
            &router_weights_buffer,
            Some(&router_bias_buffer),
            &normalized_separate,
            expert_count,
            element_count,
            top_k,
            &selected_ids_separate,
            &selected_weights_separate,
        )?;
        let separate_report = separate.commit(CudaCommandWait::Completed)?;
        assert_eq!(separate_report.encoded_operations, 2);

        let mut fused = backend.begin_submission()?;
        fused.add_residual_rms_norm_q8_1_router_topk(
            &input_buffer,
            &residual_buffer,
            Some(&input_bias_buffer),
            &weight_buffer,
            &summed_fused,
            &normalized_fused,
            &quantized_fused,
            &router_weights_buffer,
            Some(&router_bias_buffer),
            expert_count,
            top_k,
            &selected_ids_fused,
            &selected_weights_fused,
            element_count,
            1.0e-5,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 1);

        assert_close(
            &summed_fused.read_f32()?,
            &summed_separate.read_f32()?,
            1e-5,
        );
        assert_close(
            &normalized_fused.read_f32()?,
            &normalized_separate.read_f32()?,
            1e-5,
        );
        assert_eq!(
            quantized_fused.read_bytes()?,
            quantized_separate.read_bytes()?
        );
        assert_eq!(
            selected_ids_fused.read_bytes()?,
            selected_ids_separate.read_bytes()?
        );
        assert_close(
            &selected_weights_fused.read_f32()?,
            &selected_weights_separate.read_f32()?,
            1e-6,
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_qwen35_ssm_decay_beta_matches_host_formula()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let input = vec![
            0.25_f32, -0.5, 1.0, 0.75, -1.25, 0.5, -0.2, 0.35, -0.8, 0.9, 0.6, -0.4,
        ];
        let alpha_offset = 2usize;
        let beta_offset = 7usize;
        let element_count = 4usize;
        let ssm_a = vec![-0.8_f32, -0.55, -0.35, -0.15];
        let ssm_dt = vec![0.1_f32, -0.2, 0.05, 0.3];

        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input.clone())?;
        let ssm_a_buffer = backend.input_buffer(Shape::new(vec![ssm_a.len()]), ssm_a.clone())?;
        let ssm_dt_buffer = backend.input_buffer(Shape::new(vec![ssm_dt.len()]), ssm_dt.clone())?;
        let decay_output = backend.f32_buffer(element_count)?;
        let beta_output = backend.f32_buffer(element_count)?;

        let mut submission = backend.begin_submission()?;
        submission.qwen35_ssm_decay_beta_f32(
            &input_buffer,
            alpha_offset,
            beta_offset,
            &ssm_a_buffer,
            &ssm_dt_buffer,
            element_count,
            &decay_output,
            &beta_output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);

        let expected_decay = (0..element_count)
            .map(|index| {
                let shifted = input[alpha_offset + index] + ssm_dt[index];
                let softplus = if shifted > 20.0 {
                    shifted
                } else {
                    (1.0 + shifted.exp()).ln()
                };
                (softplus * ssm_a[index]).exp()
            })
            .collect::<Vec<_>>();
        let expected_beta = (0..element_count)
            .map(|index| {
                let value = input[beta_offset + index];
                1.0 / (1.0 + (-value).exp())
            })
            .collect::<Vec<_>>();

        assert_close(&decay_output.read_f32()?, &expected_decay, 1e-6);
        assert_close(&beta_output.read_f32()?, &expected_beta, 1e-6);
        Ok(())
    }

    #[test]
    fn cuda_submission_gated_delta_matches_host_for_grouped_and_reordered_v_heads()
    -> Result<(), Box<dyn std::error::Error>> {
        fn host_gated_delta_step(
            qkv: &[f32],
            query_offset: usize,
            key_offset: usize,
            value_offset: usize,
            decay: &[f32],
            beta: &[f32],
            state: &mut [f32],
            key_head_count: usize,
            value_head_count: usize,
            key_dim: usize,
            value_dim: usize,
            v_head_reordered: bool,
            output: &mut [f32],
        ) {
            let repeat_factor = value_head_count / key_head_count.max(1);
            for value_head_index in 0..value_head_count {
                let key_head_index = if v_head_reordered {
                    value_head_index % key_head_count.max(1)
                } else if repeat_factor > 0 {
                    value_head_index / repeat_factor
                } else {
                    0
                };
                let query = &qkv[query_offset + key_head_index * key_dim
                    ..query_offset + (key_head_index + 1) * key_dim];
                let key = &qkv[key_offset + key_head_index * key_dim
                    ..key_offset + (key_head_index + 1) * key_dim];
                let value = &qkv[value_offset + value_head_index * value_dim
                    ..value_offset + (value_head_index + 1) * value_dim];
                for value_dim_index in 0..value_dim {
                    let row_offset =
                        (value_head_index * value_dim + value_dim_index).saturating_mul(key_dim);
                    let state_row = &mut state[row_offset..row_offset + key_dim];
                    for entry in state_row.iter_mut() {
                        *entry *= decay[value_head_index];
                    }
                    let kv_mem = state_row
                        .iter()
                        .zip(key.iter())
                        .map(|(left, right)| left * right)
                        .sum::<f32>();
                    let delta = (value[value_dim_index] - kv_mem) * beta[value_head_index];
                    for (entry, key_value) in state_row.iter_mut().zip(key.iter().copied()) {
                        *entry += key_value * delta;
                    }
                    output[value_head_index * value_dim + value_dim_index] = state_row
                        .iter()
                        .zip(query.iter())
                        .map(|(left, right)| left * right)
                        .sum::<f32>();
                }
            }
        }

        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let query_offset = 0usize;
        let key_offset = 4usize;
        let value_offset = 8usize;
        let key_head_count = 2usize;
        let value_head_count = 4usize;
        let key_dim = 2usize;
        let value_dim = 2usize;
        let qkv = vec![
            0.5_f32, -0.25, 0.75, 0.125, // q
            -0.2, 0.6, 0.4, -0.3, // k
            0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, // v
        ];
        let decay = vec![0.9_f32, 0.8, 0.7, 0.6];
        let beta = vec![0.2_f32, 0.3, 0.4, 0.5];
        let initial_state = vec![
            0.05_f32, -0.01, 0.02, 0.03, -0.02, 0.04, 0.01, -0.05, 0.06, -0.02, -0.03, 0.07, 0.01,
            0.08, -0.04, 0.02,
        ];

        for v_head_reordered in [false, true] {
            let mut expected_state = initial_state.clone();
            let mut expected_output = vec![0.0_f32; value_head_count * value_dim];
            host_gated_delta_step(
                &qkv,
                query_offset,
                key_offset,
                value_offset,
                decay.as_slice(),
                beta.as_slice(),
                expected_state.as_mut_slice(),
                key_head_count,
                value_head_count,
                key_dim,
                value_dim,
                v_head_reordered,
                expected_output.as_mut_slice(),
            );

            let qkv_buffer = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv.clone())?;
            let decay_buffer =
                backend.input_buffer(Shape::new(vec![decay.len()]), decay.clone())?;
            let beta_buffer = backend.input_buffer(Shape::new(vec![beta.len()]), beta.clone())?;
            let state_buffer = backend
                .input_buffer(Shape::new(vec![initial_state.len()]), initial_state.clone())?;
            let output_buffer = backend.f32_buffer(value_head_count * value_dim)?;

            let mut submission = backend.begin_submission()?;
            submission.gated_delta_step_f32(
                &qkv_buffer,
                query_offset,
                key_offset,
                value_offset,
                &decay_buffer,
                &beta_buffer,
                &state_buffer,
                key_head_count,
                value_head_count,
                key_dim,
                value_dim,
                v_head_reordered,
                &output_buffer,
            )?;
            let report = submission.commit(CudaCommandWait::Completed)?;
            assert_eq!(report.encoded_operations, 1);

            assert_close(&state_buffer.read_f32()?, &expected_state, 1e-6);
            assert_close(&output_buffer.read_f32()?, &expected_output, 1e-6);
        }

        Ok(())
    }

    #[test]
    fn cuda_submission_rms_norm_region_matches_host_formula()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let input = vec![
            -0.4_f32, 0.2, 1.0, -2.0, 0.5, 1.5, -0.25, 0.75, -1.25, 0.6, 0.8, -0.9,
        ];
        let weight = vec![0.9_f32, 1.1, 1.3, 0.7];
        let input_offset = 2usize;
        let output_offset = 3usize;
        let element_count = 8usize;
        let epsilon = 1.0e-5_f32;
        let initial_output = vec![
            9.0_f32, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0,
        ];

        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input.clone())?;
        let weight_buffer = backend.input_buffer(Shape::new(vec![weight.len()]), weight.clone())?;
        let mut output_buffer = backend.f32_buffer(initial_output.len())?;
        output_buffer.write_f32(initial_output.as_slice())?;

        let mut submission = backend.begin_submission()?;
        submission.rms_norm_region(
            &input_buffer,
            input_offset,
            &weight_buffer,
            &output_buffer,
            output_offset,
            element_count,
            epsilon,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);

        let mut expected = initial_output;
        for (row_index, row) in input[input_offset..input_offset + element_count]
            .chunks_exact(weight.len())
            .enumerate()
        {
            let mean_square = row.iter().map(|value| value * value).sum::<f32>() / row.len() as f32;
            let inv_rms = (mean_square + epsilon).sqrt().recip();
            let output_row_offset = output_offset + row_index * weight.len();
            for (feature_index, value) in row.iter().copied().enumerate() {
                expected[output_row_offset + feature_index] =
                    value * weight[feature_index] * inv_rms;
            }
        }

        assert_close(&output_buffer.read_f32()?, &expected, 1e-6);
        Ok(())
    }

    #[test]
    fn cuda_submission_split_interleaved_query_gate_rms_norm_matches_separate_kernels()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let head_count = 3usize;
        let head_dim = 8usize;
        let element_count = head_count * head_dim;
        let input = (0..(element_count * 2))
            .map(|index| ((index as f32 % 17.0) - 8.0) * 0.125)
            .collect::<Vec<_>>();
        let weight = (0..head_dim)
            .map(|index| 0.5_f32 + (index as f32 % 11.0) * 0.0625)
            .collect::<Vec<_>>();
        let epsilon = 1.0e-5_f32;

        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let weight_buffer = backend.input_buffer(Shape::new(vec![weight.len()]), weight)?;
        let split_query = backend.f32_buffer(element_count)?;
        let split_gate = backend.f32_buffer(element_count)?;
        let normalized_query = backend.f32_buffer(element_count)?;
        let fused_query = backend.f32_buffer(element_count)?;
        let fused_gate = backend.f32_buffer(element_count)?;

        let mut separate = backend.begin_submission()?;
        separate.split_interleaved_query_gate_f32(
            &input_buffer,
            head_count,
            head_dim,
            &split_query,
            &split_gate,
        )?;
        separate.rms_norm_region(
            &split_query,
            0,
            &weight_buffer,
            &normalized_query,
            0,
            element_count,
            epsilon,
        )?;
        let separate_report = separate.commit(CudaCommandWait::Completed)?;
        assert_eq!(separate_report.encoded_operations, 2);

        let mut fused = backend.begin_submission()?;
        fused.split_interleaved_query_gate_rms_norm_f32(
            &input_buffer,
            head_count,
            head_dim,
            &weight_buffer,
            epsilon,
            &fused_query,
            &fused_gate,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 1);

        assert_close(
            &fused_query.read_f32()?,
            &normalized_query.read_f32()?,
            1e-6,
        );
        assert_close(&fused_gate.read_f32()?, &split_gate.read_f32()?, 1e-6);
        Ok(())
    }

    #[test]
    fn cuda_submission_pack_qwen35_key_value_rms_norm_matches_separate_kernels()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let kv_head_count = 2usize;
        let head_dim = 8usize;
        let kv_width = kv_head_count * head_dim;
        let key_offset = 5usize;
        let value_offset = key_offset + kv_width;
        let output_key_offset = 7usize;
        let output_value_offset = output_key_offset + kv_width;
        let input = (0..(value_offset + kv_width))
            .map(|index| ((index as f32 % 19.0) - 9.0) * 0.09375)
            .collect::<Vec<_>>();
        let weight = (0..head_dim)
            .map(|index| 0.75_f32 + (index as f32 % 5.0) * 0.125)
            .collect::<Vec<_>>();
        let epsilon = 1.0e-5_f32;

        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let weight_buffer = backend.input_buffer(Shape::new(vec![weight.len()]), weight)?;
        let output_len = output_value_offset + kv_width;
        let mut separate_output = backend.f32_buffer(output_len)?;
        separate_output.write_f32(&vec![17.0_f32; output_len])?;
        let mut fused_output = backend.f32_buffer(output_len)?;
        fused_output.write_f32(&vec![17.0_f32; output_len])?;

        let mut separate = backend.begin_submission()?;
        separate.rms_norm_region(
            &input_buffer,
            key_offset,
            &weight_buffer,
            &separate_output,
            output_key_offset,
            kv_width,
            epsilon,
        )?;
        separate.copy_buffer_region(
            &input_buffer,
            value_offset.saturating_mul(std::mem::size_of::<f32>()),
            &separate_output,
            output_value_offset.saturating_mul(std::mem::size_of::<f32>()),
            kv_width.saturating_mul(std::mem::size_of::<f32>()),
        )?;
        let separate_report = separate.commit(CudaCommandWait::Completed)?;
        assert_eq!(separate_report.encoded_operations, 2);

        let mut fused = backend.begin_submission()?;
        fused.pack_qwen35_key_value_rms_norm_f32(
            &input_buffer,
            key_offset,
            value_offset,
            kv_head_count,
            head_dim,
            &weight_buffer,
            epsilon,
            &fused_output,
            output_key_offset,
            output_value_offset,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 1);

        assert_close(
            &fused_output.read_f32()?,
            &separate_output.read_f32()?,
            1e-6,
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_pack_qwen35_hybrid_qkv_rms_norm_matches_separate_kernels()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let group_count = 3usize;
        let state_size = 8usize;
        let q_size = group_count * state_size;
        let k_size = q_size;
        let v_size = 20usize;
        let q_offset = 4usize;
        let k_offset = q_offset + q_size;
        let v_offset = k_offset + k_size;
        let output_q_offset = 6usize;
        let output_k_offset = output_q_offset + q_size;
        let output_v_offset = output_k_offset + k_size;
        let input = (0..(v_offset + v_size))
            .map(|index| ((index as f32 % 23.0) - 11.0) * 0.0625)
            .collect::<Vec<_>>();
        let q_weight = (0..state_size)
            .map(|index| 0.5_f32 + (index as f32 % 7.0) * 0.0625)
            .collect::<Vec<_>>();
        let k_weight = (0..state_size)
            .map(|index| 0.75_f32 + (index as f32 % 5.0) * 0.09375)
            .collect::<Vec<_>>();
        let epsilon = 1.0e-6_f32;

        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let q_weight_buffer = backend.input_buffer(Shape::new(vec![q_weight.len()]), q_weight)?;
        let k_weight_buffer = backend.input_buffer(Shape::new(vec![k_weight.len()]), k_weight)?;
        let output_len = output_v_offset + v_size;
        let mut separate_output = backend.f32_buffer(output_len)?;
        separate_output.write_f32(&vec![13.0_f32; output_len])?;
        let mut fused_output = backend.f32_buffer(output_len)?;
        fused_output.write_f32(&vec![13.0_f32; output_len])?;

        let mut separate = backend.begin_submission()?;
        separate.rms_norm_region(
            &input_buffer,
            q_offset,
            &q_weight_buffer,
            &separate_output,
            output_q_offset,
            q_size,
            epsilon,
        )?;
        separate.rms_norm_region(
            &input_buffer,
            k_offset,
            &k_weight_buffer,
            &separate_output,
            output_k_offset,
            k_size,
            epsilon,
        )?;
        separate.copy_buffer_region(
            &input_buffer,
            v_offset.saturating_mul(std::mem::size_of::<f32>()),
            &separate_output,
            output_v_offset.saturating_mul(std::mem::size_of::<f32>()),
            v_size.saturating_mul(std::mem::size_of::<f32>()),
        )?;
        let separate_report = separate.commit(CudaCommandWait::Completed)?;
        assert_eq!(separate_report.encoded_operations, 3);

        let mut fused = backend.begin_submission()?;
        fused.pack_qwen35_hybrid_qkv_rms_norm_f32(
            &input_buffer,
            q_offset,
            k_offset,
            v_offset,
            group_count,
            state_size,
            v_size,
            &q_weight_buffer,
            &k_weight_buffer,
            epsilon,
            &fused_output,
            output_q_offset,
            output_k_offset,
            output_v_offset,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 1);

        assert_close(
            &fused_output.read_f32()?,
            &separate_output.read_f32()?,
            1e-6,
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_depthwise_causal_conv1d_step_silu_matches_separate_kernels()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let channels = 64usize;
        let kernel_size = 4usize;
        let state_tokens = kernel_size - 1;
        let input = (0..channels)
            .map(|index| ((index as f32 % 13.0) - 6.0) * 0.125)
            .collect::<Vec<_>>();
        let state = (0..channels * state_tokens)
            .map(|index| ((index as f32 % 17.0) - 8.0) * 0.0625)
            .collect::<Vec<_>>();
        let weights = (0..channels * kernel_size)
            .map(|index| (((index as f32 % 11.0) - 5.0) * 0.03125) + 0.25)
            .collect::<Vec<_>>();
        let ones = vec![1.0_f32; channels];

        let input_buffer = backend.input_buffer(Shape::new(vec![channels]), input)?;
        let weights_buffer =
            backend.input_buffer(Shape::new(vec![channels * kernel_size]), weights)?;
        let ones_buffer = backend.input_buffer(Shape::new(vec![channels]), ones)?;

        let separate_state = backend.input_buffer(Shape::new(vec![state.len()]), state.clone())?;
        let separate_conv = backend.f32_buffer(channels)?;
        let separate_output = backend.f32_buffer(channels)?;

        let fused_state = backend.input_buffer(Shape::new(vec![state.len()]), state)?;
        let fused_output = backend.f32_buffer(channels)?;

        let mut separate = backend.begin_submission()?;
        separate.depthwise_causal_conv1d_step_f32(
            &input_buffer,
            &separate_state,
            &weights_buffer,
            channels,
            kernel_size,
            &separate_conv,
        )?;
        separate.silu_mul_f32(
            &separate_conv,
            0,
            &ones_buffer,
            0,
            channels,
            &separate_output,
        )?;
        let separate_report = separate.commit(CudaCommandWait::Completed)?;
        assert_eq!(separate_report.encoded_operations, 2);

        let mut fused = backend.begin_submission()?;
        fused.depthwise_causal_conv1d_step_silu_f32(
            &input_buffer,
            &fused_state,
            &weights_buffer,
            channels,
            kernel_size,
            &fused_output,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 1);

        assert_close(
            &fused_output.read_f32()?,
            &separate_output.read_f32()?,
            1e-6,
        );
        assert_close(&fused_state.read_f32()?, &separate_state.read_f32()?, 1e-6);
        Ok(())
    }

    #[test]
    fn cuda_submission_router_topk_delayed_softmax_matches_fused_router_kernel_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let element_count = 2880usize;
        let expert_count = 32usize;
        let top_k = 4usize;
        let input = (0..element_count)
            .map(|index| ((index as f32 % 17.0) - 8.0) * 0.03125)
            .collect::<Vec<_>>();
        let router_weights = (0..expert_count)
            .flat_map(|expert| {
                (0..element_count).map(move |index| {
                    ((expert as f32 + 1.0) * 0.0015) + (((index as f32 % 29.0) - 14.0) * 0.00075)
                })
            })
            .collect::<Vec<_>>();
        let mut router_weights_transposed = vec![0.0_f32; router_weights.len()];
        for expert in 0..expert_count {
            let row_start = expert * element_count;
            let row = &router_weights[row_start..row_start + element_count];
            for (column, value) in row.iter().copied().enumerate() {
                router_weights_transposed[column * expert_count + expert] = value;
            }
        }
        let router_bias = (0..expert_count)
            .map(|expert| ((expert as f32 % 5.0) - 2.0) * 0.0625)
            .collect::<Vec<_>>();

        let input_buffer = backend.input_buffer(Shape::new(vec![element_count]), input)?;
        let router_weights_buffer = backend.input_buffer(
            Shape::new(vec![expert_count, element_count]),
            router_weights,
        )?;
        let router_weights_transposed_buffer = backend.input_buffer(
            Shape::new(vec![element_count, expert_count]),
            router_weights_transposed,
        )?;
        let router_bias_buffer =
            backend.input_buffer(Shape::new(vec![expert_count]), router_bias)?;
        let selected_ids_fused = backend.byte_buffer(&vec![0_u8; top_k * size_of::<i32>()])?;
        let selected_weights_fused = backend.f32_buffer(top_k)?;
        let router_logits = backend.f32_buffer(expert_count)?;
        let selected_ids_split = backend.byte_buffer(&vec![0_u8; top_k * size_of::<i32>()])?;
        let selected_weights_split = backend.f32_buffer(top_k)?;

        let mut fused = backend.begin_submission()?;
        fused.router_topk_softmax(
            &router_weights_buffer,
            Some(&router_bias_buffer),
            &input_buffer,
            expert_count,
            element_count,
            top_k,
            &selected_ids_fused,
            &selected_weights_fused,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 1);

        let mut split = backend.begin_submission()?;
        split.matmul(
            &input_buffer,
            &router_weights_transposed_buffer,
            &router_logits,
            1,
            element_count,
            expert_count,
        )?;
        split.add_f32_in_place(&router_logits, 0, &router_bias_buffer, expert_count)?;
        split.router_topk_delayed_softmax(
            &router_logits,
            expert_count,
            top_k,
            &selected_ids_split,
            &selected_weights_split,
        )?;
        let split_report = split.commit(CudaCommandWait::Completed)?;
        assert_eq!(split_report.encoded_operations, 3);

        assert_eq!(
            selected_ids_split.read_bytes()?,
            selected_ids_fused.read_bytes()?
        );
        assert_close(
            &selected_weights_split.read_f32()?,
            &selected_weights_fused.read_f32()?,
            1e-6,
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_q8_0_quantized_matvec_q8_1_fast_path_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector();
        let row_a = sample_q8_0_row(0.25, 1);
        let row_b = sample_q8_0_row(0.5, -1);
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_a)?,
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_b)?,
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let q8_1_buffer =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 32)?])?;
        let output = backend.f32_buffer(2)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 32, &q8_1_buffer)?;
        submission.quantized_matvec_q8_1(
            &weights,
            0,
            QuantizationMode::GgmlQ8_0,
            2,
            32,
            &q8_1_buffer,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);
        assert_close(&output.read_f32()?, &expected, 5e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_q8_0_quantized_matvec_q8_1_fast_path_with_bias_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector();
        let row_a = sample_q8_0_row(0.25, 1);
        let row_b = sample_q8_0_row(0.5, -1);
        let bias = vec![0.75_f32, -1.25_f32];
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_a)? + bias[0],
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_b)? + bias[1],
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let bias_buffer = backend.input_buffer(Shape::new(vec![bias.len()]), bias)?;
        let q8_1_buffer =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 32)?])?;
        let output = backend.f32_buffer(2)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 32, &q8_1_buffer)?;
        submission.quantized_matvec_q8_1(
            &weights,
            0,
            QuantizationMode::GgmlQ8_0,
            2,
            32,
            &q8_1_buffer,
            Some(&bias_buffer),
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);
        assert_close(&output.read_f32()?, &expected, 5e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_q8_0_quantized_matvec_q8_1_argmax_fast_path_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector();
        let row_a = sample_q8_0_row(0.25, 1);
        let row_b = sample_q8_0_row(0.5, -1);
        let bias = vec![0.75_f32, -1.25_f32];
        let expected = [
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_a)? + bias[0],
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_b)? + bias[1],
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let bias_buffer = backend.input_buffer(Shape::new(vec![bias.len()]), bias)?;
        let q8_1_buffer =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 32)?])?;
        let mut argmax_output = backend.byte_buffer(&vec![0_u8; std::mem::size_of::<u64>()])?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 32, &q8_1_buffer)?;
        argmax_output.write_bytes(
            &(((u64::from(!f32::NEG_INFINITY.to_bits())) << 32)
                | u64::from(u32::MAX - i32::MAX as u32))
            .to_ne_bytes(),
        )?;
        submission.quantized_matvec_q8_1_argmax(
            &weights,
            0,
            QuantizationMode::GgmlQ8_0,
            2,
            32,
            &q8_1_buffer,
            Some(&bias_buffer),
            &argmax_output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);

        let packed_bytes = argmax_output.read_bytes()?;
        let packed = u64::from_ne_bytes(
            packed_bytes[..std::mem::size_of::<u64>()]
                .try_into()
                .expect("packed argmax buffer should be eight bytes"),
        );
        let actual_index = (u32::MAX - packed as u32) as usize;
        let expected_index = if expected[0] >= expected[1] { 0 } else { 1 };
        assert_eq!(actual_index, expected_index);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_q4_k_quantized_matvec_q8_1_fast_path_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector_256();
        let row_a = sample_q4_k_row(0.5, 0.125, 3);
        let row_b = sample_q4_k_row(0.375, 0.25, 11);
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlQ4K, &row_a)?,
            quantized_row_dot(&input, QuantizationMode::GgmlQ4K, &row_b)?,
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let q8_1_buffer =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 256)?])?;
        let output = backend.f32_buffer(2)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 256, &q8_1_buffer)?;
        submission.quantized_matvec_q8_1(
            &weights,
            0,
            QuantizationMode::GgmlQ4K,
            2,
            256,
            &q8_1_buffer,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);
        assert_close(&output.read_f32()?, &expected, 1e-3);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_q4_k_quantized_matvec_q8_1_argmax_fast_path_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector_256();
        let row_a = sample_q4_k_row(0.5, 0.125, 3);
        let row_b = sample_q4_k_row(0.375, 0.25, 11);
        let bias = vec![0.75_f32, -1.25_f32];
        let expected = [
            quantized_row_dot(&input, QuantizationMode::GgmlQ4K, &row_a)? + bias[0],
            quantized_row_dot(&input, QuantizationMode::GgmlQ4K, &row_b)? + bias[1],
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let bias_buffer = backend.input_buffer(Shape::new(vec![bias.len()]), bias)?;
        let q8_1_buffer =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 256)?])?;
        let mut argmax_output = backend.byte_buffer(&vec![0_u8; std::mem::size_of::<u64>()])?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 256, &q8_1_buffer)?;
        argmax_output.write_bytes(
            &(((u64::from(!f32::NEG_INFINITY.to_bits())) << 32)
                | u64::from(u32::MAX - i32::MAX as u32))
            .to_ne_bytes(),
        )?;
        submission.quantized_matvec_q8_1_argmax(
            &weights,
            0,
            QuantizationMode::GgmlQ4K,
            2,
            256,
            &q8_1_buffer,
            Some(&bias_buffer),
            &argmax_output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);

        let packed_bytes = argmax_output.read_bytes()?;
        let packed = u64::from_ne_bytes(
            packed_bytes[..std::mem::size_of::<u64>()]
                .try_into()
                .expect("packed argmax buffer should be eight bytes"),
        );
        let actual_index = (u32::MAX - packed as u32) as usize;
        let expected_index = if expected[0] >= expected[1] { 0 } else { 1 };
        assert_eq!(actual_index, expected_index);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_q6_k_quantized_matvec_q8_1_fast_path_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector_256();
        let row_a = sample_q6_k_row(0.25, -3);
        let row_b = sample_q6_k_row(0.5, 2);
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlQ6K, &row_a)?,
            quantized_row_dot(&input, QuantizationMode::GgmlQ6K, &row_b)?,
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let q8_1_buffer =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 256)?])?;
        let output = backend.f32_buffer(2)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 256, &q8_1_buffer)?;
        submission.quantized_matvec_q8_1(
            &weights,
            0,
            QuantizationMode::GgmlQ6K,
            2,
            256,
            &q8_1_buffer,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);
        assert_close(&output.read_f32()?, &expected, 1e-3);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_mxfp4_quantized_matvec_q8_1_fast_path_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector();
        let row_a = sample_mxfp4_row(4);
        let row_b = sample_mxfp4_row(6);
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlMxfp4, &row_a)?,
            quantized_row_dot(&input, QuantizationMode::GgmlMxfp4, &row_b)?,
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let q8_1_buffer =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 32)?])?;
        let output = backend.f32_buffer(2)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 32, &q8_1_buffer)?;
        submission.quantized_matvec_q8_1(
            &weights,
            0,
            QuantizationMode::GgmlMxfp4,
            2,
            32,
            &q8_1_buffer,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);
        assert_close(&output.read_f32()?, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_q8_0_quantized_matvec_q8_1_fast_path_for_multi_block_rows_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector_64();
        let row_a = sample_q8_0_row(0.25, 1)
            .into_iter()
            .chain(sample_q8_0_row(0.5, -1))
            .collect::<Vec<_>>();
        let row_b = sample_q8_0_row(0.125, -1)
            .into_iter()
            .chain(sample_q8_0_row(0.375, 1))
            .collect::<Vec<_>>();
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_a)?,
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_b)?,
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let q8_1_buffer =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 64)?])?;
        let output = backend.f32_buffer(2)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 64, &q8_1_buffer)?;
        submission.quantized_matvec_q8_1(
            &weights,
            0,
            QuantizationMode::GgmlQ8_0,
            2,
            64,
            &q8_1_buffer,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);
        assert_close(&output.read_f32()?, &expected, 5e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_mxfp4_quantized_matvec_q8_1_fast_path_for_multi_block_rows_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector_64();
        let row_a = sample_mxfp4_row(4)
            .into_iter()
            .chain(sample_mxfp4_row(5))
            .collect::<Vec<_>>();
        let row_b = sample_mxfp4_row(6)
            .into_iter()
            .chain(sample_mxfp4_row(7))
            .collect::<Vec<_>>();
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlMxfp4, &row_a)?,
            quantized_row_dot(&input, QuantizationMode::GgmlMxfp4, &row_b)?,
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let q8_1_buffer =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 64)?])?;
        let output = backend.f32_buffer(2)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 64, &q8_1_buffer)?;
        submission.quantized_matvec_q8_1(
            &weights,
            0,
            QuantizationMode::GgmlMxfp4,
            2,
            64,
            &q8_1_buffer,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);
        assert_close(&output.read_f32()?, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_silu_mul_q8_1_matches_separate_kernels()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let activation = sample_q8_1_exact_vector_64();
        let rhs = activation
            .iter()
            .enumerate()
            .map(|(index, value)| {
                if index & 1 == 0 {
                    *value * 0.25
                } else {
                    -*value * 0.125
                }
            })
            .collect::<Vec<_>>();
        let activation_buffer =
            backend.input_buffer(Shape::new(vec![activation.len()]), activation.clone())?;
        let rhs_buffer = backend.input_buffer(Shape::new(vec![rhs.len()]), rhs.clone())?;
        let separate_output = backend.f32_buffer(activation.len())?;
        let separate_q8_1 =
            backend.byte_buffer(&vec![
                0_u8;
                crate::ggml_q8_1_storage_bytes(1, activation.len())?
            ])?;
        let fused_q8_1 =
            backend.byte_buffer(&vec![
                0_u8;
                crate::ggml_q8_1_storage_bytes(1, activation.len())?
            ])?;

        let mut separate = backend.begin_submission()?;
        separate.silu_mul_f32(
            &activation_buffer,
            0,
            &rhs_buffer,
            0,
            activation.len(),
            &separate_output,
        )?;
        separate.quantize_f32_to_q8_1(&separate_output, 1, activation.len(), &separate_q8_1)?;
        let separate_report = separate.commit(CudaCommandWait::Completed)?;
        assert_eq!(separate_report.encoded_operations, 2);

        let mut fused = backend.begin_submission()?;
        fused.silu_mul_q8_1(
            &activation_buffer,
            0,
            &rhs_buffer,
            0,
            activation.len(),
            &fused_q8_1,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 1);

        assert_eq!(fused_q8_1.read_bytes()?, separate_q8_1.read_bytes()?);
        Ok(())
    }

    #[test]
    fn cuda_submission_sigmoid_mul_q8_1_matches_separate_kernels()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let values = sample_q8_1_exact_vector_64();
        let gate = values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                if index % 3 == 0 {
                    *value * 0.5
                } else {
                    -*value * 0.25
                }
            })
            .collect::<Vec<_>>();
        let values_buffer = backend.input_buffer(Shape::new(vec![values.len()]), values.clone())?;
        let gate_buffer = backend.input_buffer(Shape::new(vec![gate.len()]), gate.clone())?;
        let separate_output = backend.f32_buffer(values.len())?;
        let separate_q8_1 =
            backend.byte_buffer(&vec![
                0_u8;
                crate::ggml_q8_1_storage_bytes(1, values.len())?
            ])?;
        let fused_q8_1 =
            backend.byte_buffer(&vec![
                0_u8;
                crate::ggml_q8_1_storage_bytes(1, values.len())?
            ])?;

        let mut separate = backend.begin_submission()?;
        separate.sigmoid_mul_f32(
            &values_buffer,
            0,
            &gate_buffer,
            0,
            values.len(),
            &separate_output,
        )?;
        separate.quantize_f32_to_q8_1(&separate_output, 1, values.len(), &separate_q8_1)?;
        let separate_report = separate.commit(CudaCommandWait::Completed)?;
        assert_eq!(separate_report.encoded_operations, 2);

        let mut fused = backend.begin_submission()?;
        fused.sigmoid_mul_q8_1(
            &values_buffer,
            0,
            &gate_buffer,
            0,
            values.len(),
            &fused_q8_1,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 1);

        assert_eq!(fused_q8_1.read_bytes()?, separate_q8_1.read_bytes()?);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_mxfp4_moe_gate_up_swiglu_q8_1_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector();
        let gate_rows = 32usize;
        let up_rows = 32usize;
        let rows_per_expert = gate_rows + up_rows;
        let selected_ids = [1_i32, 0_i32];
        let selected_count = selected_ids.len();
        let weights_bytes = sample_mxfp4_expert_gate_up_weights(2, gate_rows, up_rows);
        let weights = backend.byte_buffer(&weights_bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input.clone())?;
        let input_q8_1 =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 32)?])?;
        let selected_ids_buffer = backend.byte_buffer(&i32_slice_to_bytes(&selected_ids))?;
        let output = backend.f32_buffer(selected_count * gate_rows)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 32, &input_q8_1)?;
        submission.moe_gate_up_swiglu_q8_1(
            &weights,
            QuantizationMode::GgmlMxfp4,
            17,
            rows_per_expert,
            32,
            gate_rows,
            up_rows,
            &selected_ids_buffer,
            selected_count,
            &input_q8_1,
            None,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);

        let actual = output.read_f32()?;
        let expected = expected_moe_gate_up_outputs(
            &input,
            &weights_bytes,
            selected_ids.as_slice(),
            gate_rows,
            up_rows,
        )?;
        assert_close(&actual, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_mxfp4_expert_gate_up_swiglu_q8_1_ids_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector();
        let gate_rows = 32usize;
        let up_rows = 32usize;
        let rows_per_expert = gate_rows + up_rows;
        let selected_ids = [3_i32, 1_i32, 0_i32, 2_i32];
        let selected_count = selected_ids.len();
        let weights_bytes = sample_mxfp4_expert_gate_up_weights(4, gate_rows, up_rows);
        let weights = backend.byte_buffer(&weights_bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input.clone())?;
        let input_q8_1 =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 32)?])?;
        let selected_ids_buffer = backend.byte_buffer(&i32_slice_to_bytes(&selected_ids))?;
        let output_f32 = backend.f32_buffer(selected_count * gate_rows)?;
        let output_q8_1 = backend.byte_buffer(&vec![
            0_u8;
            crate::ggml_q8_1_storage_bytes(
                selected_count,
                gate_rows
            )?
        ])?;
        let separate_q8_1 = backend.byte_buffer(&vec![
            0_u8;
            crate::ggml_q8_1_storage_bytes(
                selected_count,
                gate_rows
            )?
        ])?;

        let mut separate = backend.begin_submission()?;
        separate.quantize_f32_to_q8_1(&input_buffer, 1, 32, &input_q8_1)?;
        separate.moe_gate_up_swiglu_q8_1(
            &weights,
            QuantizationMode::GgmlMxfp4,
            17,
            rows_per_expert,
            32,
            gate_rows,
            up_rows,
            &selected_ids_buffer,
            selected_count,
            &input_q8_1,
            None,
            None,
            &output_f32,
        )?;
        separate.quantize_f32_to_q8_1(&output_f32, selected_count, gate_rows, &separate_q8_1)?;
        let separate_report = separate.commit(CudaCommandWait::Completed)?;
        assert_eq!(separate_report.encoded_operations, 3);

        let mut fused = backend.begin_submission()?;
        fused.quantize_f32_to_q8_1(&input_buffer, 1, 32, &input_q8_1)?;
        fused.expert_gate_up_swiglu_q8_1_ids(
            &weights,
            QuantizationMode::GgmlMxfp4,
            17,
            rows_per_expert,
            32,
            gate_rows,
            up_rows,
            &selected_ids_buffer,
            selected_count,
            &input_q8_1,
            None,
            None,
            &output_q8_1,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 2);

        assert_eq!(output_q8_1.read_bytes()?, separate_q8_1.read_bytes()?);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_mxfp4_moe_down_aggregate_q8_1_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let selected_ids = [1_i32, 0_i32];
        let selected_weights = [0.75_f32, 0.25_f32];
        let activated = sample_selected_activated_vectors();
        let weights_bytes = sample_mxfp4_expert_down_weights(2, 32, 32);
        let weights = backend.byte_buffer(&weights_bytes)?;
        let activated_buffer =
            backend.input_buffer(Shape::new(vec![activated.len()]), activated.clone())?;
        let activated_q8_1 =
            backend.byte_buffer(&vec![
                0_u8;
                crate::ggml_q8_1_storage_bytes(selected_ids.len(), 32)?
            ])?;
        let selected_ids_buffer = backend.byte_buffer(&i32_slice_to_bytes(&selected_ids))?;
        let selected_weights_buffer = backend.input_buffer(
            Shape::new(vec![selected_weights.len()]),
            selected_weights.to_vec(),
        )?;
        let output = backend.f32_buffer(32)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(
            &activated_buffer,
            selected_ids.len(),
            32,
            &activated_q8_1,
        )?;
        submission.moe_down_aggregate_q8_1(
            &weights,
            QuantizationMode::GgmlMxfp4,
            17,
            32,
            32,
            &selected_ids_buffer,
            &selected_weights_buffer,
            selected_ids.len(),
            &activated_q8_1,
            None,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);

        let actual = output.read_f32()?;
        let expected = expected_moe_down_outputs(
            &activated,
            &weights_bytes,
            selected_ids.as_slice(),
            selected_weights.as_slice(),
            32,
        )?;
        assert_close(&actual, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_mxfp4_moe_down_aggregate_q8_1_f32_selected4_path_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let selected_ids = [3_i32, 1_i32, 0_i32, 2_i32];
        let selected_weights = [0.40_f32, 0.27_f32, 0.19_f32, 0.14_f32];
        let activated = sample_selected_activated_vectors_for_count(selected_ids.len());
        let weights_bytes = sample_mxfp4_expert_down_weights(selected_ids.len(), 32, 32);
        let weights = backend.byte_buffer(&weights_bytes)?;
        let activated_buffer =
            backend.input_buffer(Shape::new(vec![activated.len()]), activated.clone())?;
        let selected_ids_buffer = backend.byte_buffer(&i32_slice_to_bytes(&selected_ids))?;
        let selected_weights_buffer = backend.input_buffer(
            Shape::new(vec![selected_weights.len()]),
            selected_weights.to_vec(),
        )?;
        let output = backend.f32_buffer(32)?;
        let mut submission = backend.begin_submission()?;
        submission.moe_down_aggregate_q8_1_f32(
            &weights,
            QuantizationMode::GgmlMxfp4,
            17,
            32,
            32,
            &selected_ids_buffer,
            &selected_weights_buffer,
            selected_ids.len(),
            &activated_buffer,
            None,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);

        let actual = output.read_f32()?;
        let expected = expected_moe_down_outputs(
            &activated,
            &weights_bytes,
            selected_ids.as_slice(),
            selected_weights.as_slice(),
            32,
        )?;
        assert_close(&actual, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_mxfp4_moe_down_aggregate_q8_1_grouped_selected_path_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let selected_ids = [4_i32, 1_i32, 3_i32, 0_i32, 2_i32];
        let selected_weights = [0.30_f32, 0.24_f32, 0.18_f32, 0.16_f32, 0.12_f32];
        let activated = sample_selected_activated_vectors_for_count(selected_ids.len());
        let weights_bytes = sample_mxfp4_expert_down_weights(selected_ids.len(), 32, 32);
        let weights = backend.byte_buffer(&weights_bytes)?;
        let activated_buffer =
            backend.input_buffer(Shape::new(vec![activated.len()]), activated.clone())?;
        let activated_q8_1 =
            backend.byte_buffer(&vec![
                0_u8;
                crate::ggml_q8_1_storage_bytes(selected_ids.len(), 32)?
            ])?;
        let selected_ids_buffer = backend.byte_buffer(&i32_slice_to_bytes(&selected_ids))?;
        let selected_weights_buffer = backend.input_buffer(
            Shape::new(vec![selected_weights.len()]),
            selected_weights.to_vec(),
        )?;
        let output = backend.f32_buffer(32)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(
            &activated_buffer,
            selected_ids.len(),
            32,
            &activated_q8_1,
        )?;
        submission.moe_down_aggregate_q8_1(
            &weights,
            QuantizationMode::GgmlMxfp4,
            17,
            32,
            32,
            &selected_ids_buffer,
            &selected_weights_buffer,
            selected_ids.len(),
            &activated_q8_1,
            None,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);

        let actual = output.read_f32()?;
        let expected = expected_moe_down_outputs(
            &activated,
            &weights_bytes,
            selected_ids.as_slice(),
            selected_weights.as_slice(),
            32,
        )?;
        assert_close(&actual, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_mxfp4_grouped_expert_matvec_and_accumulate_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let selected_ids = [3_i32, 1_i32, 0_i32, 2_i32];
        let selected_weights = [0.40_f32, 0.27_f32, 0.19_f32, 0.14_f32];
        let activated = sample_selected_activated_vectors_for_count(selected_ids.len());
        let weights_bytes = sample_mxfp4_expert_down_weights(selected_ids.len(), 32, 32);
        let weights = backend.byte_buffer(&weights_bytes)?;
        let activated_buffer =
            backend.input_buffer(Shape::new(vec![activated.len()]), activated.clone())?;
        let activated_q8_1 =
            backend.byte_buffer(&vec![
                0_u8;
                crate::ggml_q8_1_storage_bytes(selected_ids.len(), 32)?
            ])?;
        let selected_ids_buffer = backend.byte_buffer(&i32_slice_to_bytes(&selected_ids))?;
        let selected_weights_buffer = backend.input_buffer(
            Shape::new(vec![selected_weights.len()]),
            selected_weights.to_vec(),
        )?;
        let projected = backend.f32_buffer(selected_ids.len() * 32)?;
        let output = backend.f32_buffer(32)?;

        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(
            &activated_buffer,
            selected_ids.len(),
            32,
            &activated_q8_1,
        )?;
        submission.expert_matvec_q8_1_ids(
            &weights,
            QuantizationMode::GgmlMxfp4,
            17,
            32,
            32,
            &selected_ids_buffer,
            selected_ids.len(),
            &activated_q8_1,
            None,
            &projected,
        )?;
        submission.accumulate_expert_outputs(
            &projected,
            &selected_weights_buffer,
            selected_ids.len(),
            32,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 3);

        let actual_projected = projected.read_f32()?;
        let expected_projected = expected_moe_down_project_outputs(
            &activated,
            &weights_bytes,
            selected_ids.as_slice(),
            32,
        )?;
        assert_close(&actual_projected, &expected_projected, 1e-4);

        let actual = output.read_f32()?;
        let expected = expected_moe_down_outputs(
            &activated,
            &weights_bytes,
            selected_ids.as_slice(),
            selected_weights.as_slice(),
            32,
        )?;
        assert_close(&actual, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_backend_rejects_non_cuda_tensor_specs_when_available() {
        let mut backend = CudaBackend::new();
        let Some(_) = backend.selected_device() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return;
        };
        let spec = TensorSpec::new(Shape::new(vec![1]), DType::F32, Device::cpu());
        assert!(backend.allocate(&spec).is_err());
    }

    fn assert_close(actual: &[f32], expected: &[f32], tolerance: f32) {
        assert_eq!(actual.len(), expected.len());
        for (index, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!(
                (actual - expected).abs() <= tolerance,
                "index {index}: actual={actual} expected={expected}"
            );
        }
    }

    fn standard_rope_tables(sequence_length: usize, half_dim: usize) -> (Vec<f32>, Vec<f32>) {
        let head_dim = half_dim * 2;
        let mut cos = vec![0.0_f32; sequence_length * half_dim];
        let mut sin = vec![0.0_f32; sequence_length * half_dim];
        for position in 0..sequence_length {
            for feature in 0..half_dim {
                let exponent = (2 * feature) as f32 / head_dim as f32;
                let inv_freq = 1.0_f32 / crate::CUDA_PUBLIC_STANDARD_ROPE_BASE.powf(exponent);
                let angle = position as f32 * inv_freq;
                let index = position * half_dim + feature;
                cos[index] = angle.cos();
                sin[index] = angle.sin();
            }
        }
        (cos, sin)
    }

    fn sample_q8_0_row(scale: f32, multiplier: i8) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(34);
        bytes.extend_from_slice(&f32_to_f16_bits(scale).to_le_bytes());
        for index in 0_i8..32_i8 {
            bytes.push(index.saturating_mul(multiplier).to_le_bytes()[0]);
        }
        bytes
    }

    fn sample_reference_vector() -> Vec<f32> {
        (0..32).map(|index| (index as f32 + 1.0) * 0.25).collect()
    }

    fn sample_reference_vector_256() -> Vec<f32> {
        (0..256)
            .map(|index| ((index as i32 * 13) % 29 - 14) as f32 / 3.0)
            .collect()
    }

    fn qwen38_quantized_rows()
    -> Result<Vec<(QuantizationMode, Vec<u8>)>, Box<dyn std::error::Error>> {
        let report =
            serde_json::from_str::<Qwen38QuantReferenceReport>(QWEN38_QUANT_REFERENCE_VECTORS)?;
        let mut rows = report
            .vectors
            .into_iter()
            .map(|vector| {
                let mode = match vector.block_type.as_str() {
                    "Q3_K" => QuantizationMode::GgmlQ3K,
                    "IQ3_S" => QuantizationMode::GgmlIq3S,
                    "IQ4_XS" => QuantizationMode::GgmlIq4Xs,
                    other => return Err(format!("unexpected reference block type {other}")),
                };
                let bytes =
                    hex::decode(vector.block_bytes_hex).map_err(|error| error.to_string())?;
                Ok((mode, bytes))
            })
            .collect::<Result<Vec<_>, String>>()?;
        rows.push((QuantizationMode::GgmlQ5K, sample_q5_k_row(0.5, 0.125, 3)));
        Ok(rows)
    }

    fn sample_q8_1_exact_vector() -> Vec<f32> {
        let mut values = Vec::with_capacity(32);
        for index in 0..31_i32 {
            values.push(((index * 11) % 29 - 14) as f32);
        }
        values.push(127.0);
        values
    }

    fn sample_q8_1_exact_vector_64() -> Vec<f32> {
        sample_q8_1_exact_vector()
            .into_iter()
            .chain((0..31_i32).map(|index| ((index * 17) % 41 - 20) as f32))
            .chain(std::iter::once(127.0))
            .collect()
    }

    fn sample_q8_1_exact_vector_256() -> Vec<f32> {
        std::iter::repeat_with(sample_q8_1_exact_vector)
            .take(8)
            .flatten()
            .collect()
    }

    fn sample_mxfp4_row(scale_exponent: u8) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(17);
        bytes.push(scale_exponent);
        for pair in 0..16_u8 {
            let low = pair & 0x07;
            let high = 0x0f_u8.saturating_sub(pair & 0x07);
            bytes.push(low | (high << 4));
        }
        bytes
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
            bytes.push(offset.wrapping_add(index.wrapping_mul(11)));
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

    fn sample_mxfp4_expert_gate_up_weights(
        expert_count: usize,
        gate_rows: usize,
        up_rows: usize,
    ) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(expert_count * (gate_rows + up_rows) * 17);
        for expert_index in 0..expert_count {
            for row in 0..gate_rows {
                bytes.extend_from_slice(&sample_mxfp4_row(4 + ((expert_index + row) % 4) as u8));
            }
            for row in 0..up_rows {
                bytes.extend_from_slice(&sample_mxfp4_row(5 + ((expert_index + row) % 4) as u8));
            }
        }
        bytes
    }

    fn sample_mxfp4_expert_down_weights(
        expert_count: usize,
        rows: usize,
        columns: usize,
    ) -> Vec<u8> {
        assert_eq!(columns, 32);
        let mut bytes = Vec::with_capacity(expert_count * rows * 17);
        for expert_index in 0..expert_count {
            for row in 0..rows {
                bytes
                    .extend_from_slice(&sample_mxfp4_row(3 + ((expert_index * 3 + row) % 5) as u8));
            }
        }
        bytes
    }

    fn sample_selected_activated_vectors() -> Vec<f32> {
        sample_selected_activated_vectors_for_count(2)
    }

    fn sample_selected_activated_vectors_for_count(selected_count: usize) -> Vec<f32> {
        let mut values = Vec::with_capacity(selected_count.saturating_mul(32));
        for selected_index in 0..selected_count {
            if selected_index == 0 {
                values.extend(sample_q8_1_exact_vector());
                continue;
            }
            values.extend((0..31_i32).map(|index| {
                let step = (selected_index as i32 + 3) * 5;
                ((index * step) % 37 - 18) as f32
            }));
            values.push(127.0 - selected_index as f32);
        }
        values
    }

    fn expected_moe_gate_up_outputs(
        input: &[f32],
        weights: &[u8],
        selected_ids: &[i32],
        gate_rows: usize,
        up_rows: usize,
    ) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let rows_per_expert = gate_rows + up_rows;
        let row_byte_len = 17usize;
        let mut outputs = Vec::with_capacity(selected_ids.len() * gate_rows);
        for &selected_id in selected_ids {
            let expert_index = usize::try_from(selected_id)?;
            for row in 0..gate_rows {
                let gate_start = (expert_index * rows_per_expert + row) * row_byte_len;
                let up_start = (expert_index * rows_per_expert + gate_rows + row) * row_byte_len;
                let gate = quantized_row_dot(
                    input,
                    QuantizationMode::GgmlMxfp4,
                    &weights[gate_start..gate_start + row_byte_len],
                )?;
                let up = quantized_row_dot(
                    input,
                    QuantizationMode::GgmlMxfp4,
                    &weights[up_start..up_start + row_byte_len],
                )?;
                outputs.push(reference_oai_swiglu(gate, up));
            }
        }
        Ok(outputs)
    }

    fn expected_moe_down_outputs(
        activated: &[f32],
        weights: &[u8],
        selected_ids: &[i32],
        selected_weights: &[f32],
        rows: usize,
    ) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let projected = expected_moe_down_project_outputs(activated, weights, selected_ids, rows)?;
        let mut outputs = vec![0.0_f32; rows];
        for (selected_slot, &route) in selected_weights.iter().enumerate() {
            for row_index in 0..rows {
                outputs[row_index] += projected[selected_slot * rows + row_index] * route;
            }
        }
        Ok(outputs)
    }

    fn expected_moe_down_project_outputs(
        activated: &[f32],
        weights: &[u8],
        selected_ids: &[i32],
        rows: usize,
    ) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let row_byte_len = 17usize;
        let columns = 32usize;
        let mut outputs = vec![0.0_f32; selected_ids.len() * rows];
        for (selected_slot, &selected_id) in selected_ids.iter().enumerate() {
            let expert_index = usize::try_from(selected_id)?;
            let activated_row = &activated[selected_slot * columns..(selected_slot + 1) * columns];
            for row_index in 0..rows {
                let row_start = (expert_index * rows + row_index) * row_byte_len;
                outputs[selected_slot * rows + row_index] = quantized_row_dot(
                    activated_row,
                    QuantizationMode::GgmlMxfp4,
                    &weights[row_start..row_start + row_byte_len],
                )?;
            }
        }
        Ok(outputs)
    }

    fn i32_slice_to_bytes(values: &[i32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(values.len() * size_of::<i32>());
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    fn f32_slice_to_f16_le_bytes(values: &[f32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(values.len() * size_of::<u16>());
        for &value in values {
            bytes.extend_from_slice(&f32_to_f16_bits(value).to_le_bytes());
        }
        bytes
    }

    fn reference_oai_swiglu(gate: f32, up: f32) -> f32 {
        const LIMIT: f32 = 7.0;
        const ALPHA: f32 = 1.702;
        let x = gate.min(LIMIT);
        let y = up.clamp(-LIMIT, LIMIT);
        let out_glu = x / (1.0 + (ALPHA * -x).exp());
        out_glu * (y + 1.0)
    }

    fn f32_to_f16_bits(value: f32) -> u16 {
        let bits = value.to_bits();
        let sign = ((bits >> 16) & 0x8000) as u16;
        let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
        let mantissa = bits & 0x007f_ffff;
        if exponent <= 0 {
            return sign;
        }
        if exponent >= 0x1f {
            return sign | 0x7c00;
        }
        sign | ((exponent as u16) << 10) | ((mantissa >> 13) as u16)
    }
}
