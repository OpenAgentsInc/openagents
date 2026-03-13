//! Metal backend discovery, allocation, submission, and minimal execution
//! surfaces for Psionic.

#![allow(
    clippy::borrow_as_ptr,
    clippy::manual_is_multiple_of,
    clippy::ref_as_ptr,
    clippy::result_large_err,
    clippy::too_many_arguments,
    clippy::vec_init_then_push
)]
#![cfg_attr(
    test,
    allow(
        clippy::bool_assert_comparison,
        clippy::expect_used,
        clippy::panic_in_result_fn
    )
)]

use std::{
    any::Any,
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt,
    sync::Arc,
    thread,
};

use psionic_compiler::compile_graph;
use psionic_core::{
    BackendExtensionKind, BackendExtensionOp, DType, DeviceKind, Shape, TensorData, TensorId,
    TensorSpec,
};
use psionic_ir::{ExecutionOp, ExecutionPlan, ExecutionStep, Graph};
use psionic_runtime::{
    Allocator, AllocatorPoolMode, AllocatorPoolPolicy, AllocatorPoolReport, AllocatorPoolState,
    BackendDegradedPolicy, BackendExtensionSupport, BackendName, BackendRuntimeResources,
    BackendSelection, BufferHandle, BufferResidency, BufferStorageKind, CacheAction, CacheKind,
    CacheObservation, CompilePathEvidence, CompilePathTemperature, DeviceDescriptor,
    DeviceDiscovery, DeviceMemoryBudget, ExecutionBackend, ExecutionMetrics,
    ExecutionPlanCachePolicy, ExecutionPlanCacheReport, ExecutionPlanCacheState, ExecutionResult,
    HealthStatus, KernelCachePolicy, KernelCacheReport, KernelCacheState, KvCacheAccounting,
    KvCachePageLayout, KvCacheState, PrefixCacheIdentity, PrefixCacheState, RuntimeError,
    RuntimeHealth, ServedProductBackendPolicy,
};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "Metal backend discovery, allocation, and submission";

#[cfg(target_os = "macos")]
const MODERN_FAMILY_FLAG: &str = "family_modern";
#[cfg(target_os = "macos")]
const LEGACY_FAMILY_FLAG: &str = "family_legacy";
const FLASH_ATTENTION_FEATURE_FLAG: &str = "flash_attention";

const METAL_POOL_MAX_CACHED_BUFFERS: usize = 128;
const METAL_POOL_MAX_CACHED_BYTES: u64 = 64 * 1024 * 1024;
const METAL_EXECUTION_PLAN_CACHE_MAX_ENTRIES: usize = 64;
const METAL_EXECUTION_PLAN_CACHE_MAX_CACHED_BYTES: u64 = 1024 * 1024;
#[cfg(target_os = "macos")]
const METAL_KERNEL_CACHE_MAX_ENTRIES: usize = 1;
#[cfg(target_os = "macos")]
const METAL_KERNEL_CACHE_MAX_CACHED_BYTES: u64 = 1024 * 1024;
#[cfg(target_os = "macos")]
const METAL_DENSE_PIPELINE_ESTIMATED_BYTES: u64 = 1024 * 1024;
const METAL_TEXT_GENERATION_POOL_MAX_CACHED_BUFFERS: usize = 512;
const METAL_TEXT_GENERATION_POOL_MAX_CACHED_BYTES: u64 = 512 * 1024 * 1024;
const METAL_TEXT_GENERATION_KERNEL_CACHE_MAX_ENTRIES: usize = 8;
const METAL_TEXT_GENERATION_KERNEL_CACHE_MAX_CACHED_BYTES: u64 = 64 * 1024 * 1024;
const METAL_TEXT_GENERATION_MIN_AVAILABLE_BYTES: u64 = 128 * 1024 * 1024;

/// Exact plan surface currently supported for the first accelerated
/// `psionic.embeddings` milestone.
pub const EMBEDDINGS_SUPPORTED_OPS: &[&str] = &["input", "constant", "matmul", "add"];

/// Dense plan surface currently covered for the first Metal-backed
/// `psionic.text_generation` milestone.
pub const TEXT_GENERATION_SUPPORTED_OPS: &[&str] = &[
    "input",
    "constant",
    "matmul",
    "add",
    "backend_extension:rms_norm",
    "backend_extension:rotary_embedding",
    "backend_extension:scaled_dot_product_attention",
    "argmax_f32",
    "top_k_f32",
    "mul_mv_id_q8_0",
    "mul_mv_id_mxfp4",
    "expert_matvec_f32_ids_q8_0",
    "expert_matvec_f32_ids_mxfp4",
];

/// Metal buffer storage mode visible to Psionic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetalStorageMode {
    /// Host-visible storage shared with the GPU.
    Shared,
    /// Host-visible managed storage that requires explicit GPU-to-host sync.
    Managed,
    /// GPU-private storage that is not host visible.
    Private,
}

/// How long Psionic should wait after a Metal submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetalCommandWait {
    /// Commit and return immediately.
    None,
    /// Wait until the command buffer is scheduled.
    Scheduled,
    /// Wait until the command buffer is completed.
    Completed,
}

/// Stable command-buffer lifecycle state exposed by Psionic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetalCommandStatus {
    /// The command buffer has not been enqueued yet.
    NotEnqueued,
    /// The command buffer is enqueued.
    Enqueued,
    /// The command buffer was committed.
    Committed,
    /// The command buffer is scheduled on the device.
    Scheduled,
    /// The command buffer completed successfully.
    Completed,
    /// The command buffer failed.
    Error,
}

/// Submission metadata returned after a command buffer is committed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetalSubmissionReport {
    /// Final command-buffer status observed by Psionic.
    pub status: MetalCommandStatus,
    /// Number of explicit encoded operations recorded in the submission.
    pub encoded_operations: usize,
    /// Number of explicit GPU-to-host synchronizations encoded.
    pub synchronized_buffers: usize,
}

/// Flattened top-k selection result returned by the Metal backend.
#[derive(Clone, Debug, PartialEq)]
pub struct MetalTopKResult {
    /// Number of rows processed from the source logits buffer.
    pub row_count: usize,
    /// Number of selected elements per row.
    pub top_k: usize,
    /// Row-major selected indices.
    pub indices: Vec<u32>,
    /// Row-major selected values aligned with `indices`.
    pub values: Vec<f32>,
}

/// Output mode for logits selection on the Metal backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetalLogitsOutputMode {
    /// Return only the greedy token ids.
    GreedyToken,
    /// Return only the bounded top-k candidates and logits.
    TopKCandidates(usize),
    /// Materialize the full raw logits vector.
    RawLogits,
}

/// Observable token-selection metrics for one Metal logits output path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetalLogitsSelectionMetrics {
    /// Output mode used for the selection path.
    pub output_mode: MetalLogitsOutputMode,
    /// Number of bytes returned to the caller on the host path.
    pub readback_bytes: u64,
    /// Whether full raw logits were materialized on the host.
    pub raw_logits_materialized: bool,
}

/// Result of one backend-owned logits selection request.
#[derive(Clone, Debug, PartialEq)]
pub struct MetalLogitsSelectionResult {
    /// Selected token ids, one per row.
    pub selected_tokens: Vec<u32>,
    /// Bounded top-k candidates when requested.
    pub candidates: Option<MetalTopKResult>,
    /// Full raw logits when requested.
    pub logits: Option<Vec<f32>>,
    /// Observable output-mode metrics.
    pub metrics: MetalLogitsSelectionMetrics,
}

/// Explicit grouped-expert execution evidence returned by Metal `mul_mv_id`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetalGroupedExpertStats {
    /// Whether the grouped ids-enabled path executed.
    pub grouped_path: bool,
    /// Number of packed experts available in the weights buffer.
    pub expert_count: usize,
    /// Number of selected experts evaluated for this dispatch.
    pub selected_count: usize,
    /// Number of output rows produced per selected expert.
    pub rows_per_expert: usize,
    /// Packed byte stride for one expert row.
    pub row_stride: usize,
}

/// Flattened output from one grouped selected-expert matvec request.
#[derive(Clone, Debug, PartialEq)]
pub struct MetalGroupedExpertMatvecResult {
    /// Row-major outputs with shape `[selected_count, rows_per_expert]`.
    pub values: Vec<f32>,
    /// Explicit grouped-path evidence.
    pub stats: MetalGroupedExpertStats,
}

/// Output from one quantized row-wise matrix-vector request on Metal-owned storage.
#[derive(Clone, Debug, PartialEq)]
pub struct MetalQuantizedMatvecResult {
    /// Row-major output values with logical shape `[rows]`.
    pub values: Vec<f32>,
}

/// Explicit decode-attention execution evidence returned by the Metal backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetalDecodeAttentionStats {
    /// Whether the flash-style online-softmax path was used.
    pub flash_attention_path: bool,
    /// Whether RoPE was applied inside the backend decode path.
    pub rotary_applied: bool,
    /// Whether device-resident KV state participated in the decode path.
    pub used_device_kv: bool,
    /// Zero-based write index used for the current KV append.
    pub cache_write_index: usize,
    /// Current cached token count after the append.
    pub cached_tokens: usize,
    /// Number of query heads.
    pub query_head_count: usize,
    /// Number of KV heads.
    pub kv_head_count: usize,
}

/// Output of one backend-owned decode-attention step.
#[derive(Clone)]
pub struct MetalDecodeAttentionResult {
    /// Attention output buffer with logical shape `[1, query_heads, 1, head_dim]`.
    pub output: MetalBuffer,
    /// Observable KV state after the current decode step.
    pub cache_state: KvCacheState,
    /// Explicit decode-attention execution evidence.
    pub stats: MetalDecodeAttentionStats,
    /// Reserved graph reuse evidence when the step used a steady-state runtime.
    pub graph_metrics: Option<MetalGraphReuseMetrics>,
}

/// Reserved graph family for steady-state Metal GPT-OSS execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetalGraphReserveKind {
    /// Prompt/prefill graph shape.
    Prompt,
    /// Decode-step graph shape.
    Decode,
}

impl MetalGraphReserveKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Prompt => "prompt",
            Self::Decode => "decode",
        }
    }
}

/// Explicit shape reservation for one Metal attention graph family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetalAttentionGraphReserve {
    /// Reserved graph family.
    pub kind: MetalGraphReserveKind,
    /// Reserved batch size.
    pub batch_size: usize,
    /// Reserved sequence length.
    pub sequence_len: usize,
    /// Reserved query head count.
    pub query_head_count: usize,
    /// Reserved KV head count.
    pub kv_head_count: usize,
    /// Reserved head dimension.
    pub head_dim: usize,
    /// Reserved max context tokens.
    pub max_context_tokens: usize,
    /// Whether the reserved shape is causal.
    pub causal: bool,
    /// Whether RoPE pairs are interleaved.
    pub interleaved: bool,
    /// Whether the reserved shape can use the flash-attention path.
    pub flash_attention: bool,
}

/// Stable identity for one reserved Metal attention graph shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetalGraphIdentity {
    /// Reserved graph family.
    pub kind: MetalGraphReserveKind,
    /// Reserved batch size.
    pub batch_size: usize,
    /// Reserved sequence length.
    pub sequence_len: usize,
    /// Reserved query head count.
    pub query_head_count: usize,
    /// Reserved KV head count.
    pub kv_head_count: usize,
    /// Reserved head dimension.
    pub head_dim: usize,
    /// Reserved max context tokens.
    pub max_context_tokens: usize,
    /// Whether the reserved shape is causal.
    pub causal: bool,
    /// Whether RoPE pairs are interleaved.
    pub interleaved: bool,
    /// Whether the reserved shape can use the flash-attention path.
    pub flash_attention: bool,
    /// Stable string identity for reuse comparison and reporting.
    pub stable_digest: String,
}

/// Observable reserve/reuse evidence for one reserved Metal graph shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetalGraphReuseMetrics {
    /// Stable identity for the reserved graph shape.
    pub identity: MetalGraphIdentity,
    /// Explicit rebuild-versus-reuse evidence for this prepare step.
    pub compile_path: CompilePathEvidence,
    /// Stable command label used for the reserved runtime.
    pub command_label: String,
    /// Whether the reserved command/runtime state was reused.
    pub command_state_reused: bool,
    /// Bytes reserved for the output buffer of the shape.
    pub reserved_output_bytes: u64,
    /// Number of times the runtime reused the same shape.
    pub reuse_count: usize,
    /// Number of times the runtime was rebuilt for a new shape.
    pub rebuild_count: usize,
}

/// Reserved prompt or decode graph runtime for steady-state Metal execution.
#[derive(Clone)]
pub struct MetalAttentionGraphRuntime {
    identity: MetalGraphIdentity,
    output_buffer: MetalBuffer,
    command_label: String,
    reuse_count: usize,
    rebuild_count: usize,
    last_metrics: MetalGraphReuseMetrics,
}

/// Explicit allocator and kernel-cache policy for Metal token generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetalTextGenerationRuntimePolicy {
    /// Allocator-pool policy for token-generation workloads.
    pub allocator_pool: AllocatorPoolPolicy,
    /// Kernel-cache policy for token-generation workloads.
    pub kernel_cache: KernelCachePolicy,
    /// Minimum execution bytes required after reserved budgets, when known.
    pub minimum_available_bytes: Option<u64>,
}

impl MetalTextGenerationRuntimePolicy {
    /// Returns the default GPT-OSS-oriented Metal runtime policy.
    #[must_use]
    pub fn gpt_oss_default() -> Self {
        Self {
            allocator_pool: AllocatorPoolPolicy::exact_tensor_spec(
                METAL_TEXT_GENERATION_POOL_MAX_CACHED_BUFFERS,
                METAL_TEXT_GENERATION_POOL_MAX_CACHED_BYTES,
            ),
            kernel_cache: KernelCachePolicy::bounded(
                METAL_TEXT_GENERATION_KERNEL_CACHE_MAX_ENTRIES,
                Some(METAL_TEXT_GENERATION_KERNEL_CACHE_MAX_CACHED_BYTES),
            ),
            minimum_available_bytes: Some(METAL_TEXT_GENERATION_MIN_AVAILABLE_BYTES),
        }
    }
}

/// Observable admission decision for Metal token-generation runtime configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetalTextGenerationAdmission {
    /// Whether the current runtime budgets admit token generation.
    pub admitted: bool,
    /// Memory-related refusal reason when admission failed.
    pub refusal_reason: Option<String>,
}

/// Observable Metal token-generation runtime state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetalTextGenerationRuntimeResources {
    /// Applied token-generation runtime policy.
    pub policy: MetalTextGenerationRuntimePolicy,
    /// Current allocator-pool report.
    pub allocator_pool: AllocatorPoolReport,
    /// Current kernel-cache report.
    pub kernel_cache: KernelCacheReport,
    /// Device-visible memory budget after applying runtime policies.
    pub device_memory_budget: DeviceMemoryBudget,
    /// Admission decision for the configured runtime.
    pub admission: MetalTextGenerationAdmission,
}

/// Device-resident GPT-OSS KV cache mirror for the Metal backend.
#[derive(Clone, Debug)]
pub struct MetalKvCacheMirror {
    key_buffer: MetalBuffer,
    value_buffer: MetalBuffer,
    width: usize,
    len: usize,
    capacity_tokens: usize,
    max_context_tokens: usize,
}

/// Compatibility tuple required for safe shared-prefix reuse on Metal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetalSharedPrefixCompatibility {
    /// Stable served-artifact digest used to validate ownership.
    pub served_artifact_digest: String,
    /// Stable model identifier.
    pub model_id: String,
    /// Stable model revision.
    pub model_revision: String,
    /// Stable weight-bundle digest.
    pub weight_bundle_digest: String,
    /// Stable tokenizer family label.
    pub tokenizer_family: String,
    /// Stable backend compatibility label.
    pub backend_compatibility: String,
    /// KV width required for reuse.
    pub kv_width: usize,
    /// Logical KV page layout required for reuse.
    pub page_layout: KvCachePageLayout,
}

#[derive(Clone, Debug)]
struct MetalSharedPrefixEntry {
    compatibility: MetalSharedPrefixCompatibility,
    prompt_tokens: Vec<u32>,
    cache: MetalKvCacheMirror,
}

/// Result of one shared-prefix lookup against the Metal device cache store.
#[derive(Clone, Debug)]
pub struct MetalSharedPrefixLookup {
    /// Observable prefix-cache state for the request.
    pub state: PrefixCacheState,
    /// Number of prompt tokens reused from the device-resident prefix.
    pub reused_tokens: usize,
    /// Stable identity for the reused prefix when one existed.
    pub identity: Option<PrefixCacheIdentity>,
    /// Device-resident truncated cache when reuse succeeded.
    pub cache: Option<MetalKvCacheMirror>,
}

/// Shared prompt-prefix reuse store backed by Metal device-resident KV mirrors.
#[derive(Clone, Debug, Default)]
pub struct MetalSharedPrefixStore {
    entries: Vec<MetalSharedPrefixEntry>,
}

/// Runtime-visible prompt residency metrics for one Metal request path.
#[derive(Clone, Debug, PartialEq)]
pub struct MetalPromptResidencyMetrics {
    /// Current KV-cache accounting.
    pub kv_accounting: KvCacheAccounting,
    /// Shared-prefix reuse state for the request.
    pub prefix_state: PrefixCacheState,
    /// Stable identity for the reused prefix when one existed.
    pub prefix_identity: Option<PrefixCacheIdentity>,
    /// Explicit cache observations explaining the outcome.
    pub observations: Vec<CacheObservation>,
}

/// Metal-backed tensor buffer.
#[derive(Clone)]
pub struct MetalBuffer {
    spec: TensorSpec,
    byte_len: usize,
    storage_kind: BufferStorageKind,
    storage_mode: MetalStorageMode,
    host_visible: bool,
    host_writable: bool,
    _keepalive: Option<Arc<dyn Any>>,
    platform: platform::PlatformBuffer,
}

impl fmt::Debug for MetalBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MetalBuffer")
            .field("spec", &self.spec)
            .field("byte_len", &self.byte_len)
            .field("storage_kind", &self.storage_kind)
            .field("storage_mode", &self.storage_mode)
            .field("host_visible", &self.host_visible)
            .field("host_writable", &self.host_writable)
            .field("platform", &"<metal platform buffer>")
            .finish_non_exhaustive()
    }
}

impl MetalBuffer {
    /// Returns the backing allocation size in bytes.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// Returns the Metal storage mode backing the buffer.
    #[must_use]
    pub const fn storage_mode(&self) -> MetalStorageMode {
        self.storage_mode
    }

    /// Returns whether the CPU can map the backing storage directly.
    #[must_use]
    pub const fn host_visible(&self) -> bool {
        self.host_visible
    }

    /// Writes raw bytes into the host-visible buffer contents.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        if bytes.len() != self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "metal buffer write length mismatch: expected {}, actual {}",
                self.byte_len,
                bytes.len()
            )));
        }
        if !self.host_writable {
            return Err(RuntimeError::Backend(String::from(
                "metal buffer is not host writable",
            )));
        }
        self.platform.write_bytes(bytes, self.storage_mode)
    }

    /// Writes raw bytes into a byte range inside the host-visible buffer contents.
    pub fn write_bytes_at_offset(
        &mut self,
        byte_offset: usize,
        bytes: &[u8],
    ) -> Result<(), RuntimeError> {
        if byte_offset.saturating_add(bytes.len()) > self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "metal buffer ranged write exceeds allocation: offset={} len={} allocation={}",
                byte_offset,
                bytes.len(),
                self.byte_len
            )));
        }
        if !self.host_writable {
            return Err(RuntimeError::Backend(String::from(
                "metal buffer is not host writable",
            )));
        }
        self.platform
            .write_bytes_at_offset(byte_offset, bytes, self.storage_mode)
    }

    /// Reads raw bytes from the host-visible buffer contents.
    pub fn read_bytes(&self) -> Result<Vec<u8>, RuntimeError> {
        self.platform.read_bytes(self.byte_len)
    }

    /// Reads raw bytes from a byte range inside the buffer contents.
    pub fn read_bytes_at_offset(
        &self,
        byte_offset: usize,
        byte_len: usize,
    ) -> Result<Vec<u8>, RuntimeError> {
        if byte_offset.saturating_add(byte_len) > self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "metal buffer ranged read exceeds allocation: offset={} len={} allocation={}",
                byte_offset, byte_len, self.byte_len
            )));
        }
        self.platform.read_bytes_at_offset(byte_offset, byte_len)
    }

    /// Borrows a host-visible byte range without allocating a copy.
    pub fn with_bytes_at_offset<T>(
        &self,
        byte_offset: usize,
        byte_len: usize,
        map: impl FnOnce(&[u8]) -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        if byte_offset.saturating_add(byte_len) > self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "metal buffer ranged read exceeds allocation: offset={} len={} allocation={}",
                byte_offset, byte_len, self.byte_len
            )));
        }
        self.platform
            .with_bytes_at_offset(byte_offset, byte_len, map)
    }

    /// Borrows the full host-visible byte contents without allocating a copy.
    pub fn with_bytes<T>(
        &self,
        map: impl FnOnce(&[u8]) -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        self.with_bytes_at_offset(0, self.byte_len, map)
    }

    /// Writes contiguous `f32` values into an `f32` buffer.
    pub fn write_f32(&mut self, values: &[f32]) -> Result<(), RuntimeError> {
        if self.spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "write_f32 requires F32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        if self.storage_kind != BufferStorageKind::DenseF32 {
            return Err(RuntimeError::Backend(format!(
                "write_f32 requires dense f32 storage, actual {:?}",
                self.storage_kind
            )));
        }
        if values.len() != self.spec.storage_size() {
            return Err(RuntimeError::Backend(format!(
                "metal buffer write length mismatch: expected {} values, actual {}",
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

    /// Writes a prefix of contiguous `f32` values into an `f32` buffer.
    pub fn write_f32_prefix(&mut self, values: &[f32]) -> Result<(), RuntimeError> {
        if self.spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "write_f32_prefix requires F32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        if self.storage_kind != BufferStorageKind::DenseF32 {
            return Err(RuntimeError::Backend(format!(
                "write_f32_prefix requires dense f32 storage, actual {:?}",
                self.storage_kind
            )));
        }
        if values.len() > self.spec.storage_size() {
            return Err(RuntimeError::Backend(format!(
                "metal buffer prefix write exceeds allocation: values {} allocation {}",
                values.len(),
                self.spec.storage_size()
            )));
        }
        let mut bytes = Vec::with_capacity(
            values
                .len()
                .saturating_mul(size_of_dtype(self.spec.dtype())),
        );
        for value in values {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        self.write_bytes_at_offset(0, bytes.as_slice())
    }

    /// Reads contiguous `f32` values from an `f32` buffer.
    pub fn read_f32(&self) -> Result<Vec<f32>, RuntimeError> {
        if self.spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "read_f32 requires F32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        if self.storage_kind != BufferStorageKind::DenseF32 {
            return Err(RuntimeError::Backend(format!(
                "read_f32 requires dense f32 storage, actual {:?}",
                self.storage_kind
            )));
        }
        let bytes = self.read_bytes()?;
        let mut values = Vec::with_capacity(bytes.len() / size_of_dtype(self.spec.dtype()));
        for chunk in bytes.chunks_exact(size_of_dtype(self.spec.dtype())) {
            values.push(f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        Ok(values)
    }

    /// Reads a prefix of contiguous `f32` values from an `f32` buffer into a reusable vector.
    pub fn read_f32_prefix_into(
        &self,
        element_count: usize,
        output: &mut Vec<f32>,
    ) -> Result<(), RuntimeError> {
        if self.spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "read_f32_prefix_into requires F32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        if self.storage_kind != BufferStorageKind::DenseF32 {
            return Err(RuntimeError::Backend(format!(
                "read_f32_prefix_into requires dense f32 storage, actual {:?}",
                self.storage_kind
            )));
        }
        if element_count > self.spec.storage_size() {
            return Err(RuntimeError::Backend(format!(
                "metal buffer prefix read exceeds allocation: values {} allocation {}",
                element_count,
                self.spec.storage_size()
            )));
        }
        let byte_len = element_count.saturating_mul(size_of_dtype(self.spec.dtype()));
        output.clear();
        output.reserve(element_count.saturating_sub(output.capacity()));
        self.with_bytes_at_offset(0, byte_len, |bytes| {
            output.extend(
                bytes
                    .chunks_exact(size_of_dtype(self.spec.dtype()))
                    .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])),
            );
            Ok(())
        })
    }
}

impl BufferHandle for MetalBuffer {
    fn spec(&self) -> &TensorSpec {
        &self.spec
    }

    fn storage_kind(&self) -> BufferStorageKind {
        self.storage_kind.clone()
    }
}

/// Metal command submission that keeps synchronization explicit.
pub struct MetalSubmission {
    encoded_operations: usize,
    synchronized_buffers: usize,
    platform: platform::PlatformSubmission,
}

impl MetalSubmission {
    /// Fills a buffer with a constant byte value using a blit command.
    pub fn fill_buffer(&mut self, buffer: &MetalBuffer, value: u8) -> Result<(), RuntimeError> {
        self.platform
            .fill_buffer(&buffer.platform, buffer.byte_len, value)?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Copies one Metal buffer into another with explicit size checking.
    pub fn copy_buffer(
        &mut self,
        source: &MetalBuffer,
        destination: &MetalBuffer,
    ) -> Result<(), RuntimeError> {
        if source.byte_len != destination.byte_len {
            return Err(RuntimeError::Backend(format!(
                "metal buffer copy length mismatch: source {}, destination {}",
                source.byte_len, destination.byte_len
            )));
        }
        self.platform
            .copy_buffer(&source.platform, &destination.platform, source.byte_len)?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Encodes an explicit GPU-to-host synchronization for managed storage.
    pub fn synchronize_buffer(&mut self, buffer: &MetalBuffer) -> Result<(), RuntimeError> {
        if self
            .platform
            .synchronize_buffer(&buffer.platform, buffer.storage_mode)?
        {
            self.synchronized_buffers += 1;
        }
        Ok(())
    }

    /// Commits the submission and optionally waits for scheduling/completion.
    pub fn commit(self, wait: MetalCommandWait) -> Result<MetalSubmissionReport, RuntimeError> {
        let status = self.platform.commit(wait)?;
        Ok(MetalSubmissionReport {
            status,
            encoded_operations: self.encoded_operations,
            synchronized_buffers: self.synchronized_buffers,
        })
    }
}

/// Discovery report containing device descriptors and backend health.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetalDiscoveryReport {
    /// Discovered Metal devices.
    pub devices: Vec<DeviceDescriptor>,
    /// Backend health derived from discovery.
    pub health: RuntimeHealth,
}

#[cfg(any(test, target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeviceSupportTier {
    Modern,
    Legacy,
}

#[cfg(any(test, target_os = "macos"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct FamilySupport {
    common2: bool,
    common3: bool,
    mac1: bool,
    mac2: bool,
    metal3: bool,
    metal4: bool,
    apple: bool,
}

#[cfg(any(test, target_os = "macos"))]
fn classify_support(family: FamilySupport) -> DeviceSupportTier {
    if family.apple || family.common3 || family.metal3 || family.metal4 {
        DeviceSupportTier::Modern
    } else {
        DeviceSupportTier::Legacy
    }
}

enum MetalBackendState {
    Available(Box<AvailableMetalBackend>),
    Unavailable(RuntimeHealth),
}

struct AvailableMetalBackend {
    descriptor: DeviceDescriptor,
    platform: platform::ConfiguredBackend,
    pool: MetalAllocatorPool,
    execution_plan_cache: MetalExecutionPlanCache,
}

/// Metal backend discovery, allocation, and submission implementation.
pub struct MetalBackend {
    state: MetalBackendState,
}

#[derive(Clone, Debug)]
struct MetalAllocatorPool {
    policy: AllocatorPoolPolicy,
    cached: HashMap<TensorSpec, Vec<MetalBuffer>>,
    state: AllocatorPoolState,
}

impl MetalAllocatorPool {
    fn new(policy: AllocatorPoolPolicy) -> Self {
        Self {
            policy,
            cached: HashMap::new(),
            state: AllocatorPoolState::default(),
        }
    }

    fn take(&mut self, spec: &TensorSpec) -> Option<MetalBuffer> {
        if self.policy.mode != AllocatorPoolMode::ExactTensorSpec {
            return None;
        }
        let mut should_remove = false;
        let buffer = self.cached.get_mut(spec).and_then(|entries| {
            let buffer = entries.pop();
            should_remove = entries.is_empty();
            buffer
        });
        if should_remove {
            self.cached.remove(spec);
        }
        if let Some(buffer) = buffer {
            self.state.cached_buffers = self.state.cached_buffers.saturating_sub(1);
            self.state.cached_bytes = self
                .state
                .cached_bytes
                .saturating_sub(buffer_bytes(buffer.byte_len()));
            Some(buffer)
        } else {
            None
        }
    }

    fn recycle(&mut self, buffer: MetalBuffer) {
        if buffer.storage_kind != BufferStorageKind::DenseF32 {
            return;
        }
        if self.policy.mode != AllocatorPoolMode::ExactTensorSpec {
            return;
        }
        let bytes = buffer_bytes(buffer.byte_len());
        if self.state.cached_buffers >= self.policy.max_cached_buffers
            || self.state.cached_bytes.saturating_add(bytes) > self.policy.max_cached_bytes
        {
            return;
        }
        self.cached
            .entry(buffer.spec.clone())
            .or_default()
            .push(buffer);
        self.state.cached_buffers += 1;
        self.state.cached_bytes = self.state.cached_bytes.saturating_add(bytes);
    }

    fn report(&self) -> AllocatorPoolReport {
        AllocatorPoolReport {
            policy: self.policy.clone(),
            state: self.state.clone(),
        }
    }

    fn set_policy(&mut self, policy: AllocatorPoolPolicy) {
        self.policy = policy;
        self.trim_to_policy();
    }

    fn trim_to_policy(&mut self) {
        if self.policy.mode == AllocatorPoolMode::Disabled {
            self.cached.clear();
            self.state = AllocatorPoolState::default();
            return;
        }

        let mut ordered_specs = self.cached.keys().cloned().collect::<Vec<_>>();
        ordered_specs.sort_by_key(|spec| spec.storage_size());
        while self.state.cached_buffers > self.policy.max_cached_buffers
            || self.state.cached_bytes > self.policy.max_cached_bytes
        {
            let Some(spec) = ordered_specs.pop() else {
                break;
            };
            let mut should_remove = false;
            if let Some(entries) = self.cached.get_mut(&spec) {
                if let Some(buffer) = entries.pop() {
                    self.state.cached_buffers = self.state.cached_buffers.saturating_sub(1);
                    self.state.cached_bytes = self
                        .state
                        .cached_bytes
                        .saturating_sub(buffer_bytes(buffer.byte_len()));
                }
                should_remove = entries.is_empty();
            }
            if should_remove {
                self.cached.remove(&spec);
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Debug)]
struct MetalKernelCache {
    policy: KernelCachePolicy,
    state: KernelCacheState,
}

#[cfg(target_os = "macos")]
impl MetalKernelCache {
    fn new() -> Self {
        Self {
            policy: KernelCachePolicy::bounded(
                METAL_KERNEL_CACHE_MAX_ENTRIES,
                Some(METAL_KERNEL_CACHE_MAX_CACHED_BYTES),
            ),
            state: KernelCacheState::default(),
        }
    }

    fn record_dense_pipelines(&mut self) {
        if self.state.cached_entries == 0 {
            self.state.cached_entries = 1;
            self.state.cached_bytes = METAL_DENSE_PIPELINE_ESTIMATED_BYTES
                .min(self.policy.max_cached_bytes.unwrap_or(u64::MAX));
        }
    }

    fn report(&self) -> KernelCacheReport {
        KernelCacheReport {
            policy: self.policy.clone(),
            state: self.state.clone(),
        }
    }

    fn set_policy(&mut self, policy: KernelCachePolicy) {
        self.policy = policy;
        if !self.policy.enabled {
            self.state = KernelCacheState::default();
            return;
        }
        self.state.cached_entries = self
            .state
            .cached_entries
            .min(self.policy.max_cached_entries);
        self.state.cached_bytes = self
            .state
            .cached_bytes
            .min(self.policy.max_cached_bytes.unwrap_or(u64::MAX));
    }
}

fn metal_allocator_pool_policy() -> AllocatorPoolPolicy {
    AllocatorPoolPolicy::exact_tensor_spec(
        METAL_POOL_MAX_CACHED_BUFFERS,
        METAL_POOL_MAX_CACHED_BYTES,
    )
}

fn buffer_bytes(byte_len: usize) -> u64 {
    byte_len.try_into().unwrap_or(u64::MAX)
}

impl Default for MetalBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MetalBackend {
    /// Creates a Metal backend and selects the first modern device when one is
    /// available.
    #[must_use]
    pub fn new() -> Self {
        match platform::configure_preferred_backend() {
            Ok(platform_backend) => {
                let descriptor = platform_backend.descriptor().clone();
                Self {
                    state: MetalBackendState::Available(Box::new(AvailableMetalBackend {
                        descriptor,
                        platform: platform_backend,
                        pool: MetalAllocatorPool::new(metal_allocator_pool_policy()),
                        execution_plan_cache: MetalExecutionPlanCache::new(
                            metal_execution_plan_cache_policy(),
                        ),
                    })),
                }
            }
            Err(health) => Self {
                state: MetalBackendState::Unavailable(health),
            },
        }
    }

    /// Returns the device selected for allocation/submission, when available.
    #[must_use]
    pub fn selected_device(&self) -> Option<&DeviceDescriptor> {
        match &self.state {
            MetalBackendState::Available(backend) => Some(&backend.descriptor),
            MetalBackendState::Unavailable(_) => None,
        }
    }

    /// Returns whether the selected Metal device can use the flash-attention path.
    #[must_use]
    pub fn supports_flash_attention(&self) -> bool {
        self.selected_device()
            .is_some_and(device_supports_flash_attention)
    }

    /// Applies an explicit token-generation allocator and kernel-cache policy.
    pub fn configure_text_generation_runtime(
        &mut self,
        policy: MetalTextGenerationRuntimePolicy,
    ) -> Result<MetalTextGenerationRuntimeResources, RuntimeError> {
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.configure_text_generation_runtime(policy)
    }

    fn selected_backend_mut(&mut self) -> Option<&mut AvailableMetalBackend> {
        match &mut self.state {
            MetalBackendState::Available(backend) => Some(backend),
            MetalBackendState::Unavailable(_) => None,
        }
    }

    /// Returns the current discovery report for the local machine.
    pub fn discovery_report(&self) -> Result<MetalDiscoveryReport, RuntimeError> {
        platform::discovery_report()
    }

    /// Creates a host-visible `f32` input buffer on the selected Metal device.
    pub fn input_buffer(
        &mut self,
        shape: Shape,
        values: impl Into<Vec<f32>>,
    ) -> Result<MetalBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        let mut buffer = self.allocate(&TensorSpec::new(shape, DType::F32, device))?;
        buffer.write_f32(values.into().as_slice())?;
        Ok(buffer)
    }

    /// Creates a backend-owned quantized GGML/GGUF buffer on the selected Metal device.
    pub fn quantized_buffer(
        &mut self,
        shape: Shape,
        mode: psionic_core::QuantizationMode,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<MetalBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        let spec = TensorSpec::new(shape.clone(), DType::F32, device);
        let tensor_data = TensorData::QuantizedBlocks(psionic_core::QuantizedTensorData::new(
            mode,
            mode.ggml_block_layout(&shape).ok_or_else(|| {
                RuntimeError::Backend(format!(
                    "shape {shape} is invalid for quantized mode {mode:?}",
                ))
            })?,
            bytes,
        ));
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.buffer_from_tensor_data(&spec, &tensor_data)
    }

    /// Creates a backend-owned quantized GGML/GGUF buffer from a caller-owned byte slice.
    pub fn quantized_buffer_from_slice(
        &mut self,
        shape: Shape,
        mode: psionic_core::QuantizationMode,
        bytes: &[u8],
        keepalive: Option<Arc<dyn Any>>,
    ) -> Result<MetalBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        let spec = TensorSpec::new(shape.clone(), DType::F32, device);
        let layout = mode.ggml_block_layout(&shape).ok_or_else(|| {
            RuntimeError::Backend(format!(
                "shape {shape} is invalid for quantized mode {mode:?}"
            ))
        })?;
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.buffer_from_quantized_slice(&spec, mode, layout, bytes, keepalive)
    }

    /// Executes one quantized row-wise matrix-vector product over Metal-owned weights.
    pub fn quantized_matvec(
        &mut self,
        weights: &MetalBuffer,
        mode: psionic_core::QuantizationMode,
        rows: usize,
        columns: usize,
        input: &[f32],
    ) -> Result<Vec<f32>, RuntimeError> {
        Ok(self
            .quantized_matvec_with_offset(weights, 0, mode, rows, columns, input)?
            .values)
    }

    /// Executes one quantized row-wise matrix-vector product from a byte offset.
    pub fn quantized_matvec_with_offset(
        &mut self,
        weights: &MetalBuffer,
        byte_offset: usize,
        mode: psionic_core::QuantizationMode,
        rows: usize,
        columns: usize,
        input: &[f32],
    ) -> Result<MetalQuantizedMatvecResult, RuntimeError> {
        let Some((elements_per_block, bytes_per_block)) = mode.ggml_block_spec() else {
            return Err(RuntimeError::Backend(format!(
                "metal quantized matvec does not support mode {mode:?}",
            )));
        };
        if columns == 0 || columns % elements_per_block != 0 {
            return Err(RuntimeError::Backend(format!(
                "metal quantized matvec requires block-aligned width {columns} for {mode:?}",
            )));
        }
        if input.len() != columns {
            return Err(RuntimeError::Backend(format!(
                "metal quantized matvec input width mismatch: expected {columns}, actual {}",
                input.len()
            )));
        }
        let row_stride = (columns / elements_per_block)
            .checked_mul(bytes_per_block)
            .ok_or_else(|| {
                RuntimeError::Backend(String::from("metal quantized matvec row stride overflow"))
            })?;
        let required_bytes = rows.saturating_mul(row_stride);
        let end_offset = byte_offset.saturating_add(required_bytes);
        match weights.storage_kind() {
            BufferStorageKind::QuantizedBlocks {
                mode: stored_mode, ..
            } if stored_mode == mode => {}
            BufferStorageKind::QuantizedBlocks {
                mode: stored_mode, ..
            } => {
                return Err(RuntimeError::Backend(format!(
                    "metal quantized matvec mode mismatch: requested {mode:?}, stored {stored_mode:?}",
                )));
            }
            storage_kind => {
                return Err(RuntimeError::Backend(format!(
                    "metal quantized matvec requires quantized block storage, actual {:?}",
                    storage_kind
                )));
            }
        }
        if weights.byte_len() < end_offset {
            return Err(RuntimeError::Backend(format!(
                "metal quantized matvec byte length mismatch: required {end_offset}, actual {}",
                weights.byte_len(),
            )));
        }
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.run_quantized_matvec(weights, byte_offset, mode, rows, columns, input)
    }

    /// Executes one quantized row-wise matrix-vector product and returns only
    /// the requested logits output shape on the host path.
    pub fn quantized_matvec_select_logits_output(
        &mut self,
        weights: &MetalBuffer,
        byte_offset: usize,
        mode: psionic_core::QuantizationMode,
        rows: usize,
        columns: usize,
        input: &[f32],
        output_mode: MetalLogitsOutputMode,
    ) -> Result<MetalLogitsSelectionResult, RuntimeError> {
        let Some((elements_per_block, bytes_per_block)) = mode.ggml_block_spec() else {
            return Err(RuntimeError::Backend(format!(
                "metal quantized matvec does not support mode {mode:?}",
            )));
        };
        if columns == 0 || columns % elements_per_block != 0 {
            return Err(RuntimeError::Backend(format!(
                "metal quantized matvec requires block-aligned width {columns} for {mode:?}",
            )));
        }
        if input.len() != columns {
            return Err(RuntimeError::Backend(format!(
                "metal quantized matvec input width mismatch: expected {columns}, actual {}",
                input.len()
            )));
        }
        let row_stride = (columns / elements_per_block)
            .checked_mul(bytes_per_block)
            .ok_or_else(|| {
                RuntimeError::Backend(String::from("metal quantized matvec row stride overflow"))
            })?;
        let required_bytes = rows.saturating_mul(row_stride);
        let end_offset = byte_offset.saturating_add(required_bytes);
        match weights.storage_kind() {
            BufferStorageKind::QuantizedBlocks {
                mode: stored_mode, ..
            } if stored_mode == mode => {}
            BufferStorageKind::QuantizedBlocks {
                mode: stored_mode, ..
            } => {
                return Err(RuntimeError::Backend(format!(
                    "metal quantized matvec mode mismatch: requested {mode:?}, stored {stored_mode:?}",
                )));
            }
            storage_kind => {
                return Err(RuntimeError::Backend(format!(
                    "metal quantized matvec requires quantized block storage, actual {:?}",
                    storage_kind
                )));
            }
        }
        if weights.byte_len() < end_offset {
            return Err(RuntimeError::Backend(format!(
                "metal quantized matvec byte length mismatch: required {end_offset}, actual {}",
                weights.byte_len(),
            )));
        }
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.run_quantized_matvec_select_logits_output(
            weights,
            byte_offset,
            mode,
            rows,
            columns,
            input,
            output_mode,
        )
    }

    /// Compiles and executes a graph on the supported dense Metal surface.
    pub fn compile_and_execute(
        &mut self,
        graph: &Graph,
        inputs: &BTreeMap<TensorId, MetalBuffer>,
    ) -> Result<ExecutionResult<MetalBuffer>, RuntimeError> {
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
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

    /// Reduces each contiguous `f32` row to its argmax index.
    pub fn argmax_f32(
        &self,
        input: &MetalBuffer,
        row_count: usize,
        column_count: usize,
    ) -> Result<Vec<u32>, RuntimeError> {
        argmax_dense_rows(input, row_count, column_count, "metal argmax")
    }

    /// Selects the top-k values from each contiguous `f32` row.
    pub fn top_k_f32(
        &self,
        input: &MetalBuffer,
        row_count: usize,
        column_count: usize,
        top_k: usize,
    ) -> Result<MetalTopKResult, RuntimeError> {
        top_k_dense_rows(input, row_count, column_count, top_k, "metal top_k")
    }

    /// Selects the bounded output shape required for one logits buffer.
    pub fn select_logits_output_f32(
        &self,
        input: &MetalBuffer,
        row_count: usize,
        column_count: usize,
        output_mode: MetalLogitsOutputMode,
    ) -> Result<MetalLogitsSelectionResult, RuntimeError> {
        match output_mode {
            MetalLogitsOutputMode::GreedyToken => {
                let selected_tokens = self.argmax_f32(input, row_count, column_count)?;
                Ok(MetalLogitsSelectionResult {
                    selected_tokens,
                    candidates: None,
                    logits: None,
                    metrics: MetalLogitsSelectionMetrics {
                        output_mode,
                        readback_bytes: row_count
                            .saturating_mul(std::mem::size_of::<u32>())
                            .try_into()
                            .unwrap_or(u64::MAX),
                        raw_logits_materialized: false,
                    },
                })
            }
            MetalLogitsOutputMode::TopKCandidates(top_k) => {
                let candidates = self.top_k_f32(input, row_count, column_count, top_k)?;
                let selected_tokens = candidates
                    .indices
                    .chunks_exact(candidates.top_k.max(1))
                    .map(|row| row[0])
                    .collect::<Vec<_>>();
                let readback_bytes = candidates
                    .indices
                    .len()
                    .saturating_mul(std::mem::size_of::<u32>())
                    .saturating_add(
                        candidates
                            .values
                            .len()
                            .saturating_mul(std::mem::size_of::<f32>()),
                    )
                    .try_into()
                    .unwrap_or(u64::MAX);
                Ok(MetalLogitsSelectionResult {
                    selected_tokens,
                    candidates: Some(candidates),
                    logits: None,
                    metrics: MetalLogitsSelectionMetrics {
                        output_mode,
                        readback_bytes,
                        raw_logits_materialized: false,
                    },
                })
            }
            MetalLogitsOutputMode::RawLogits => {
                let logits = input.read_f32()?;
                let selected_tokens = argmax_values(
                    logits.as_slice(),
                    row_count,
                    column_count,
                    "metal raw logits",
                )?;
                Ok(MetalLogitsSelectionResult {
                    selected_tokens,
                    candidates: None,
                    logits: Some(logits),
                    metrics: MetalLogitsSelectionMetrics {
                        output_mode,
                        readback_bytes: row_count
                            .saturating_mul(column_count)
                            .saturating_mul(std::mem::size_of::<f32>())
                            .try_into()
                            .unwrap_or(u64::MAX),
                        raw_logits_materialized: true,
                    },
                })
            }
        }
    }

    /// Executes a llama.cpp-style grouped `mul_mv_id` expert dispatch over one
    /// decode vector and the selected expert ids.
    pub fn mul_mv_id(
        &mut self,
        weights: &MetalBuffer,
        mode: psionic_core::QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        columns: usize,
        selected_ids: &[i32],
        input: &MetalBuffer,
    ) -> Result<MetalGroupedExpertMatvecResult, RuntimeError> {
        if rows_per_expert == 0 {
            return Err(RuntimeError::Backend(String::from(
                "metal mul_mv_id requires at least one row per expert",
            )));
        }
        if selected_ids.is_empty() {
            return Ok(MetalGroupedExpertMatvecResult {
                values: Vec::new(),
                stats: MetalGroupedExpertStats {
                    grouped_path: true,
                    expert_count: 0,
                    selected_count: 0,
                    rows_per_expert,
                    row_stride,
                },
            });
        }
        let expert_count =
            validate_grouped_expert_layout(weights, mode, row_stride, rows_per_expert, columns)?;
        let selected_experts = selected_expert_indices(selected_ids, expert_count)?;
        let quantized_weights = match weights.storage_kind() {
            BufferStorageKind::QuantizedBlocks {
                mode: stored_mode, ..
            } => {
                if stored_mode != mode {
                    return Err(RuntimeError::Backend(format!(
                        "metal mul_mv_id mode mismatch: requested {mode:?}, stored {stored_mode:?}",
                    )));
                }
                true
            }
            BufferStorageKind::DenseF32 => {
                if mode != psionic_core::QuantizationMode::None {
                    return Err(RuntimeError::Backend(format!(
                        "metal mul_mv_id requested quantized mode {mode:?} for dense expert weights",
                    )));
                }
                false
            }
            storage_kind => {
                return Err(RuntimeError::Backend(format!(
                    "metal mul_mv_id does not support expert storage {:?}",
                    storage_kind
                )));
            }
        };

        if quantized_weights {
            let Some(backend) = self.selected_backend_mut() else {
                return Err(RuntimeError::Backend(String::from(
                    "metal backend unavailable: no selected execution device",
                )));
            };
            let values = backend.run_grouped_quantized_matvec(
                weights,
                mode,
                row_stride,
                rows_per_expert,
                columns,
                selected_ids,
                input,
            )?;
            return Ok(MetalGroupedExpertMatvecResult {
                values,
                stats: MetalGroupedExpertStats {
                    grouped_path: true,
                    expert_count,
                    selected_count: selected_ids.len(),
                    rows_per_expert,
                    row_stride,
                },
            });
        }

        let dense_weights = if !quantized_weights {
            Some(weights.read_f32()?)
        } else {
            None
        };
        let input_values = dense_row_major_values(input, 1, columns, "metal mul_mv_id input")?;
        let mut output = vec![0.0; selected_ids.len().saturating_mul(rows_per_expert)];
        if let Some(dense_weights) = dense_weights.as_ref() {
            grouped_dense_expert_dot_into(
                rows_per_expert,
                columns,
                selected_experts.as_slice(),
                input_values.as_slice(),
                dense_weights.as_slice(),
                output.as_mut_slice(),
            )?;
        }

        Ok(MetalGroupedExpertMatvecResult {
            values: output,
            stats: MetalGroupedExpertStats {
                grouped_path: true,
                expert_count,
                selected_count: selected_ids.len(),
                rows_per_expert,
                row_stride,
            },
        })
    }

    /// Executes one ids-driven grouped expert projection from per-selected
    /// `f32` activation rows into expert-specific `f32` output rows.
    pub fn expert_matvec_f32_ids(
        &mut self,
        weights: &MetalBuffer,
        mode: psionic_core::QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        columns: usize,
        selected_ids: &[i32],
        input: &MetalBuffer,
    ) -> Result<MetalGroupedExpertMatvecResult, RuntimeError> {
        if rows_per_expert == 0 {
            return Err(RuntimeError::Backend(String::from(
                "metal expert_matvec_f32_ids requires at least one row per expert",
            )));
        }
        if selected_ids.is_empty() {
            return Ok(MetalGroupedExpertMatvecResult {
                values: Vec::new(),
                stats: MetalGroupedExpertStats {
                    grouped_path: true,
                    expert_count: 0,
                    selected_count: 0,
                    rows_per_expert,
                    row_stride,
                },
            });
        }
        let expert_count =
            validate_grouped_expert_layout(weights, mode, row_stride, rows_per_expert, columns)?;
        let selected_experts = selected_expert_indices(selected_ids, expert_count)?;
        let quantized_weights = match weights.storage_kind() {
            BufferStorageKind::QuantizedBlocks {
                mode: stored_mode, ..
            } => {
                if stored_mode != mode {
                    return Err(RuntimeError::Backend(format!(
                        "metal expert_matvec_f32_ids mode mismatch: requested {mode:?}, stored {stored_mode:?}",
                    )));
                }
                true
            }
            BufferStorageKind::DenseF32 => {
                if mode != psionic_core::QuantizationMode::None {
                    return Err(RuntimeError::Backend(format!(
                        "metal expert_matvec_f32_ids requested quantized mode {mode:?} for dense expert weights",
                    )));
                }
                false
            }
            storage_kind => {
                return Err(RuntimeError::Backend(format!(
                    "metal expert_matvec_f32_ids does not support expert storage {:?}",
                    storage_kind
                )));
            }
        };

        if quantized_weights {
            let Some(backend) = self.selected_backend_mut() else {
                return Err(RuntimeError::Backend(String::from(
                    "metal backend unavailable: no selected execution device",
                )));
            };
            let values = backend.run_expert_matvec_f32_ids(
                weights,
                mode,
                row_stride,
                rows_per_expert,
                columns,
                selected_ids,
                input,
            )?;
            return Ok(MetalGroupedExpertMatvecResult {
                values,
                stats: MetalGroupedExpertStats {
                    grouped_path: true,
                    expert_count,
                    selected_count: selected_ids.len(),
                    rows_per_expert,
                    row_stride,
                },
            });
        }

        let dense_weights = if !quantized_weights {
            Some(weights.read_f32()?)
        } else {
            None
        };
        let input_values = dense_row_major_values(
            input,
            selected_ids.len(),
            columns,
            "metal expert_matvec_f32_ids input",
        )?;
        let mut output = vec![0.0; selected_ids.len().saturating_mul(rows_per_expert)];
        if let Some(dense_weights) = dense_weights.as_ref() {
            grouped_dense_expert_dot_rows_into(
                rows_per_expert,
                columns,
                selected_experts.as_slice(),
                input_values.as_slice(),
                dense_weights.as_slice(),
                output.as_mut_slice(),
            )?;
        }

        Ok(MetalGroupedExpertMatvecResult {
            values: output,
            stats: MetalGroupedExpertStats {
                grouped_path: true,
                expert_count,
                selected_count: selected_ids.len(),
                rows_per_expert,
                row_stride,
            },
        })
    }

    /// Begins an explicit command submission on the selected Metal device.
    pub fn begin_submission(
        &self,
        label: impl Into<String>,
    ) -> Result<MetalSubmission, RuntimeError> {
        match &self.state {
            MetalBackendState::Available(backend) => Ok(MetalSubmission {
                encoded_operations: 0,
                synchronized_buffers: 0,
                platform: backend.platform.begin_submission(label.into())?,
            }),
            MetalBackendState::Unavailable(health) => Err(RuntimeError::Backend(format!(
                "metal backend unavailable: {}",
                health.message
            ))),
        }
    }

    /// Encodes one quantized row-wise matrix-vector product into an existing submission.
    pub fn encode_quantized_matvec_submission(
        &mut self,
        submission: &mut MetalSubmission,
        weights: &MetalBuffer,
        byte_offset: usize,
        mode: psionic_core::QuantizationMode,
        rows: usize,
        columns: usize,
        input: &MetalBuffer,
        output: &MetalBuffer,
    ) -> Result<(), RuntimeError> {
        validate_quantized_matvec_request(
            weights,
            byte_offset,
            mode,
            rows,
            columns,
            input,
            output,
        )?;
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_quantized_matvec(
            &mut submission.platform,
            weights,
            byte_offset,
            mode,
            rows,
            columns,
            input,
            output,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Encodes one grouped ids-enabled quantized expert matvec into an existing submission.
    pub fn encode_grouped_quantized_matvec_submission(
        &mut self,
        submission: &mut MetalSubmission,
        weights: &MetalBuffer,
        mode: psionic_core::QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        columns: usize,
        selected_ids: &[i32],
        input: &MetalBuffer,
        output: &MetalBuffer,
    ) -> Result<(), RuntimeError> {
        validate_grouped_quantized_matvec_request(
            weights,
            mode,
            row_stride,
            rows_per_expert,
            columns,
            selected_ids,
            input,
            output,
        )?;
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_grouped_quantized_matvec(
            &mut submission.platform,
            weights,
            mode,
            row_stride,
            rows_per_expert,
            columns,
            selected_ids,
            input,
            output,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Encodes one ids-driven grouped expert projection from per-selected
    /// activation rows into expert-specific output rows.
    pub fn encode_expert_matvec_f32_ids_submission(
        &mut self,
        submission: &mut MetalSubmission,
        weights: &MetalBuffer,
        mode: psionic_core::QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        columns: usize,
        selected_ids: &[i32],
        input: &MetalBuffer,
        output: &MetalBuffer,
    ) -> Result<(), RuntimeError> {
        validate_expert_matvec_f32_ids_request(
            weights,
            mode,
            row_stride,
            rows_per_expert,
            columns,
            selected_ids,
            input,
            output,
        )?;
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_expert_matvec_f32_ids(
            &mut submission.platform,
            weights,
            mode,
            row_stride,
            rows_per_expert,
            columns,
            selected_ids,
            input,
            output,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Creates a device-resident KV mirror from host-owned prompt-cache rows.
    pub fn kv_cache_mirror_from_host_rows(
        &mut self,
        width: usize,
        max_context_tokens: usize,
        tokens: usize,
        key_values: &[f32],
        value_values: &[f32],
        reserve_tokens: usize,
    ) -> Result<MetalKvCacheMirror, RuntimeError> {
        MetalKvCacheMirror::from_host_rows(
            self,
            width,
            max_context_tokens,
            tokens,
            key_values,
            value_values,
            reserve_tokens,
        )
    }

    /// Reserves a prompt or decode graph shape for steady-state Metal execution.
    pub fn reserve_attention_graph(
        &mut self,
        reserve: MetalAttentionGraphReserve,
    ) -> Result<MetalAttentionGraphRuntime, RuntimeError> {
        let _ = self.configure_text_generation_runtime(
            MetalTextGenerationRuntimePolicy::gpt_oss_default(),
        )?;
        MetalAttentionGraphRuntime::new(self, reserve)
    }

    /// Executes one backend-owned decode-attention step using RoPE-applied query/key
    /// vectors and a device-resident KV mirror.
    pub fn decode_attention_f32(
        &mut self,
        query: &MetalBuffer,
        key: &MetalBuffer,
        value: &MetalBuffer,
        cos: &MetalBuffer,
        sin: &MetalBuffer,
        cache: &mut MetalKvCacheMirror,
        scale: f32,
        causal: bool,
        interleaved: bool,
        flash_preferred: bool,
    ) -> Result<MetalDecodeAttentionResult, RuntimeError> {
        let (query_dims, output_values, cache_state, stats) = self.compute_decode_attention_f32(
            query,
            key,
            value,
            cos,
            sin,
            cache,
            scale,
            causal,
            interleaved,
            flash_preferred,
        )?;
        let output = self.input_buffer(Shape::new(query_dims), output_values)?;

        Ok(MetalDecodeAttentionResult {
            output,
            cache_state,
            stats,
            graph_metrics: None,
        })
    }

    /// Executes one decode-attention step through a reserved steady-state runtime.
    pub fn decode_attention_f32_reserved(
        &mut self,
        runtime: &mut MetalAttentionGraphRuntime,
        query: &MetalBuffer,
        key: &MetalBuffer,
        value: &MetalBuffer,
        cos: &MetalBuffer,
        sin: &MetalBuffer,
        cache: &mut MetalKvCacheMirror,
        scale: f32,
        causal: bool,
        interleaved: bool,
        flash_preferred: bool,
    ) -> Result<MetalDecodeAttentionResult, RuntimeError> {
        let reserve = reserve_from_decode_inputs(
            query.spec().shape().dims(),
            key.spec().shape().dims(),
            cache.max_context_tokens,
            causal,
            interleaved,
            flash_preferred && self.supports_flash_attention(),
        )?;
        let graph_metrics = runtime.ensure_reserved(self, reserve)?;
        let (_query_dims, output_values, cache_state, stats) = self.compute_decode_attention_f32(
            query,
            key,
            value,
            cos,
            sin,
            cache,
            scale,
            causal,
            interleaved,
            flash_preferred,
        )?;
        runtime.output_buffer.write_f32(output_values.as_slice())?;
        Ok(MetalDecodeAttentionResult {
            output: runtime.output_buffer.clone(),
            cache_state,
            stats,
            graph_metrics: Some(graph_metrics),
        })
    }

    fn compute_decode_attention_f32(
        &mut self,
        query: &MetalBuffer,
        key: &MetalBuffer,
        value: &MetalBuffer,
        cos: &MetalBuffer,
        sin: &MetalBuffer,
        cache: &mut MetalKvCacheMirror,
        scale: f32,
        causal: bool,
        interleaved: bool,
        flash_preferred: bool,
    ) -> Result<
        (
            Vec<usize>,
            Vec<f32>,
            KvCacheState,
            MetalDecodeAttentionStats,
        ),
        RuntimeError,
    > {
        let query_dims = query.spec().shape().dims().to_vec();
        let key_dims = key.spec().shape().dims().to_vec();
        let value_dims = value.spec().shape().dims().to_vec();
        validate_decode_attention_shapes(
            query_dims.as_slice(),
            key_dims.as_slice(),
            value_dims.as_slice(),
            cache.width(),
        )?;

        let query_head_count = query_dims[1];
        let kv_head_count = key_dims[1];
        let head_dim = query_dims[3];

        let query_values = query.read_f32()?;
        let key_values = key.read_f32()?;
        let value_values = value.read_f32()?;
        let cos_values = cos.read_f32()?;
        let sin_values = sin.read_f32()?;
        let cos_dims = cos.spec().shape().dims().to_vec();

        let roped_query = apply_rotary_embedding_values(
            query_values.as_slice(),
            query_dims.as_slice(),
            cos_values.as_slice(),
            sin_values.as_slice(),
            cos_dims.as_slice(),
            interleaved,
        )?;
        let roped_key = apply_rotary_embedding_values(
            key_values.as_slice(),
            key_dims.as_slice(),
            cos_values.as_slice(),
            sin_values.as_slice(),
            cos_dims.as_slice(),
            interleaved,
        )?;

        let flattened_key = flatten_decode_heads(roped_key.as_slice(), kv_head_count, head_dim)?;
        let flattened_value =
            flatten_decode_heads(value_values.as_slice(), kv_head_count, head_dim)?;
        let cache_write_index =
            cache.append_entry(self, flattened_key.as_slice(), flattened_value.as_slice())?;

        let (expanded_key, expanded_value) =
            expand_kv_cache_for_attention(cache, query_head_count, kv_head_count, head_dim)?;
        let flash_attention_path = flash_preferred && causal && self.supports_flash_attention();
        let output_values = scaled_dot_product_attention_values(
            roped_query.as_slice(),
            expanded_key.as_slice(),
            expanded_value.as_slice(),
            query_dims.as_slice(),
            &[1, query_head_count, cache.len(), head_dim],
            &[1, query_head_count, cache.len(), head_dim],
            scale,
            false,
            flash_attention_path,
        )?;
        Ok((
            query_dims,
            output_values,
            cache.state(),
            MetalDecodeAttentionStats {
                flash_attention_path,
                rotary_applied: true,
                used_device_kv: true,
                cache_write_index,
                cached_tokens: cache.len(),
                query_head_count,
                kv_head_count,
            },
        ))
    }

    /// Returns truthful backend-selection data for a supported Metal product path.
    pub fn backend_selection(
        &self,
        supported_ops: &[&str],
    ) -> Result<BackendSelection, RuntimeError> {
        let policy = ServedProductBackendPolicy::fallback_to_compatible_backend(
            BackendDegradedPolicy::AllowSameBackend,
        );
        match &self.state {
            MetalBackendState::Available(backend) => {
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
                        "metal backend unavailable: {}",
                        health.message
                    ))),
                }
            }
            MetalBackendState::Unavailable(health) => Err(RuntimeError::Backend(format!(
                "metal backend unavailable: {}",
                health.message
            ))),
        }
    }

    /// Returns an explicit fallback selection when Metal cannot execute the
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
            MetalBackendState::Available(_) => self.backend_selection(supported_ops),
            MetalBackendState::Unavailable(health) => Ok(BackendSelection::fallback_with_policy(
                self.backend_name(),
                fallback_backend.backend_name(),
                fallback_backend.discover_devices()?.into_iter().next(),
                supported_ops
                    .iter()
                    .map(|label| String::from(*label))
                    .collect(),
                policy,
                format!("metal backend unavailable: {}", health.message),
            )
            .with_runtime_resources(fallback_backend.runtime_resources())
            .with_backend_extensions(fallback_backend.extension_support())),
        }
    }
}

impl DeviceDiscovery for MetalBackend {
    fn backend_name(&self) -> BackendName {
        "metal"
    }

    fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError> {
        self.discovery_report().map(|report| report.devices)
    }

    fn health(&self) -> RuntimeHealth {
        match self.discovery_report() {
            Ok(report) => report.health,
            Err(error) => RuntimeHealth {
                status: HealthStatus::Degraded,
                message: format!("metal discovery failed: {error}"),
            },
        }
    }

    fn runtime_resources(&self) -> Option<BackendRuntimeResources> {
        match &self.state {
            MetalBackendState::Available(backend) => Some(BackendRuntimeResources {
                execution_plan_cache: backend.execution_plan_cache.report(),
                allocator_pool: backend.pool.report(),
                kernel_cache: backend.platform.kernel_cache_report(),
                device_memory_budget: Some(
                    backend
                        .platform
                        .device_memory_budget(backend.pool.policy.max_cached_bytes),
                ),
            }),
            MetalBackendState::Unavailable(_) => None,
        }
    }

    fn extension_support(&self) -> Vec<BackendExtensionSupport> {
        match &self.state {
            MetalBackendState::Available(_) => vec![
                BackendExtensionSupport::reference(BackendExtensionKind::RmsNorm),
                BackendExtensionSupport::reference(BackendExtensionKind::RotaryEmbedding),
                BackendExtensionSupport::reference(BackendExtensionKind::ScaledDotProductAttention),
            ],
            MetalBackendState::Unavailable(_) => Vec::new(),
        }
    }
}

impl MetalKvCacheMirror {
    /// Returns the target capacity for the current request shape.
    #[must_use]
    pub fn capacity_for_request(
        current_tokens: usize,
        reserve_tokens: usize,
        max_context_tokens: usize,
    ) -> usize {
        let requested = current_tokens
            .saturating_add(reserve_tokens)
            .max(64)
            .min(max_context_tokens.max(1));
        requested
            .checked_next_power_of_two()
            .unwrap_or(max_context_tokens.max(1))
            .min(max_context_tokens.max(1))
    }

    /// Builds a device-resident KV mirror from host-owned key/value rows.
    pub fn from_host_rows(
        backend: &mut MetalBackend,
        width: usize,
        max_context_tokens: usize,
        tokens: usize,
        key_values: &[f32],
        value_values: &[f32],
        reserve_tokens: usize,
    ) -> Result<Self, RuntimeError> {
        if key_values.len() != tokens.saturating_mul(width) {
            return Err(RuntimeError::Backend(format!(
                "metal kv key rows length mismatch: expected {}, actual {}",
                tokens.saturating_mul(width),
                key_values.len()
            )));
        }
        if value_values.len() != tokens.saturating_mul(width) {
            return Err(RuntimeError::Backend(format!(
                "metal kv value rows length mismatch: expected {}, actual {}",
                tokens.saturating_mul(width),
                value_values.len()
            )));
        }
        let capacity_tokens =
            Self::capacity_for_request(tokens, reserve_tokens, max_context_tokens);
        let mut key_buffer = backend.input_buffer(
            Shape::new(vec![capacity_tokens.saturating_mul(width)]),
            vec![0.0; capacity_tokens.saturating_mul(width)],
        )?;
        let mut value_buffer = backend.input_buffer(
            Shape::new(vec![capacity_tokens.saturating_mul(width)]),
            vec![0.0; capacity_tokens.saturating_mul(width)],
        )?;
        if tokens > 0 {
            key_buffer.write_bytes_at_offset(0, f32_slice_to_bytes(key_values).as_slice())?;
            value_buffer.write_bytes_at_offset(0, f32_slice_to_bytes(value_values).as_slice())?;
        }
        Ok(Self {
            key_buffer,
            value_buffer,
            width,
            len: tokens,
            capacity_tokens,
            max_context_tokens,
        })
    }

    /// Ensures the device-resident cache can hold the requested number of tokens.
    pub fn ensure_capacity(
        &mut self,
        backend: &mut MetalBackend,
        required_tokens: usize,
    ) -> Result<(), RuntimeError> {
        if required_tokens <= self.capacity_tokens {
            return Ok(());
        }
        let new_capacity = required_tokens
            .max(self.capacity_tokens.saturating_mul(2))
            .checked_next_power_of_two()
            .unwrap_or(required_tokens)
            .min(self.max_context_tokens.max(1));
        let mut new_keys = backend.input_buffer(
            Shape::new(vec![new_capacity.saturating_mul(self.width)]),
            vec![0.0; new_capacity.saturating_mul(self.width)],
        )?;
        let mut new_values = backend.input_buffer(
            Shape::new(vec![new_capacity.saturating_mul(self.width)]),
            vec![0.0; new_capacity.saturating_mul(self.width)],
        )?;
        if self.len > 0 {
            let byte_len = self
                .len
                .saturating_mul(self.width)
                .saturating_mul(std::mem::size_of::<f32>());
            new_keys.write_bytes_at_offset(
                0,
                self.key_buffer
                    .read_bytes_at_offset(0, byte_len)?
                    .as_slice(),
            )?;
            new_values.write_bytes_at_offset(
                0,
                self.value_buffer
                    .read_bytes_at_offset(0, byte_len)?
                    .as_slice(),
            )?;
        }
        self.key_buffer = new_keys;
        self.value_buffer = new_values;
        self.capacity_tokens = new_capacity;
        Ok(())
    }

    /// Appends one key/value entry and returns the write index.
    pub fn append_entry(
        &mut self,
        backend: &mut MetalBackend,
        key: &[f32],
        value: &[f32],
    ) -> Result<usize, RuntimeError> {
        if key.len() != self.width || value.len() != self.width {
            return Err(RuntimeError::Backend(format!(
                "metal kv entry width mismatch: expected {}, actual key {} value {}",
                self.width,
                key.len(),
                value.len()
            )));
        }
        if self.len >= self.max_context_tokens {
            return Err(RuntimeError::Backend(format!(
                "metal kv cache exceeded max context {}",
                self.max_context_tokens
            )));
        }
        self.ensure_capacity(backend, self.len.saturating_add(1))?;
        let write_index = self.len;
        let byte_offset = write_index
            .saturating_mul(self.width)
            .saturating_mul(std::mem::size_of::<f32>());
        self.key_buffer
            .write_bytes_at_offset(byte_offset, f32_slice_to_bytes(key).as_slice())?;
        self.value_buffer
            .write_bytes_at_offset(byte_offset, f32_slice_to_bytes(value).as_slice())?;
        self.len = self.len.saturating_add(1);
        Ok(write_index)
    }

    /// Reads one key/value entry from the device-resident mirror.
    pub fn read_entry(&self, token_index: usize) -> Result<(Vec<f32>, Vec<f32>), RuntimeError> {
        if token_index >= self.len {
            return Err(RuntimeError::Backend(format!(
                "metal kv cache entry read exceeds logical length: index={} len={}",
                token_index, self.len
            )));
        }
        let byte_offset = token_index
            .saturating_mul(self.width)
            .saturating_mul(std::mem::size_of::<f32>());
        let byte_len = self.width.saturating_mul(std::mem::size_of::<f32>());
        Ok((
            bytes_to_f32_vec(
                self.key_buffer
                    .read_bytes_at_offset(byte_offset, byte_len)?
                    .as_slice(),
            )?,
            bytes_to_f32_vec(
                self.value_buffer
                    .read_bytes_at_offset(byte_offset, byte_len)?
                    .as_slice(),
            )?,
        ))
    }

    /// Returns a logical truncated view of the cache.
    #[must_use]
    pub fn truncated(&self, len: usize) -> Self {
        let mut truncated = self.clone();
        truncated.len = len.min(self.len);
        truncated
    }

    /// Returns the current logical token count.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns whether the cache is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the cache width in scalar elements per token.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Returns the logical page layout for this cache.
    #[must_use]
    pub fn page_layout(&self) -> KvCachePageLayout {
        KvCachePageLayout::new(
            self.max_context_tokens,
            4,
            self.width
                .saturating_mul(std::mem::size_of::<f32>())
                .saturating_mul(2),
        )
    }

    /// Returns the current observable KV state.
    #[must_use]
    pub fn state(&self) -> KvCacheState {
        KvCacheState::paged(&self.page_layout(), self.len)
    }
}

impl MetalSharedPrefixStore {
    /// Looks up the best compatible reusable prefix on the Metal device.
    pub fn lookup(
        &mut self,
        compatibility: &MetalSharedPrefixCompatibility,
        prompt_tokens: &[u32],
    ) -> MetalSharedPrefixLookup {
        let compatible_indices = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| (&entry.compatibility == compatibility).then_some(index))
            .collect::<Vec<_>>();
        if compatible_indices.is_empty() {
            return MetalSharedPrefixLookup {
                state: PrefixCacheState::None,
                reused_tokens: 0,
                identity: None,
                cache: None,
            };
        }

        let mut best: Option<(usize, usize)> = None;
        let mut stale_prefix = false;
        for index in compatible_indices {
            let entry = &self.entries[index];
            let shared = shared_prefix_len(entry.prompt_tokens.as_slice(), prompt_tokens);
            if shared == 0 {
                continue;
            }
            if entry.cache.len() < shared {
                stale_prefix = true;
                continue;
            }
            match best {
                Some((_, best_shared)) if best_shared >= shared => {}
                _ => best = Some((index, shared)),
            }
        }

        if let Some((index, shared)) = best {
            let entry = &self.entries[index];
            return MetalSharedPrefixLookup {
                state: PrefixCacheState::Hit,
                reused_tokens: shared,
                identity: Some(prefix_identity(
                    compatibility,
                    &entry.prompt_tokens[..shared],
                )),
                cache: Some(entry.cache.truncated(shared)),
            };
        }

        if stale_prefix {
            self.entries.retain(|entry| {
                !(&entry.compatibility == compatibility
                    && entry.cache.len() < entry.prompt_tokens.len())
            });
            return MetalSharedPrefixLookup {
                state: PrefixCacheState::Rebuilt,
                reused_tokens: 0,
                identity: None,
                cache: None,
            };
        }

        MetalSharedPrefixLookup {
            state: PrefixCacheState::Miss,
            reused_tokens: 0,
            identity: None,
            cache: None,
        }
    }

    /// Records or replaces one reusable prompt prefix.
    pub fn record(
        &mut self,
        compatibility: MetalSharedPrefixCompatibility,
        prompt_tokens: &[u32],
        cache: &MetalKvCacheMirror,
    ) -> PrefixCacheIdentity {
        let identity = prefix_identity(&compatibility, prompt_tokens);
        if let Some(existing) = self.entries.iter_mut().find(|entry| {
            entry.compatibility == compatibility && entry.prompt_tokens.as_slice() == prompt_tokens
        }) {
            existing.cache = cache.clone();
        } else {
            self.entries.push(MetalSharedPrefixEntry {
                compatibility,
                prompt_tokens: prompt_tokens.to_vec(),
                cache: cache.clone(),
            });
        }
        identity
    }

    /// Discards all shared prefix entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl MetalPromptResidencyMetrics {
    /// Creates prompt residency metrics from explicit before/after state and prefix reuse truth.
    #[must_use]
    pub fn new(
        before: &KvCacheState,
        current: KvCacheState,
        prefix_state: PrefixCacheState,
        prefix_identity: Option<PrefixCacheIdentity>,
        kv_action: CacheAction,
    ) -> Self {
        let mut observations = Vec::with_capacity(2);
        observations.push(prefix_cache_observation(prefix_state));
        observations.push(CacheObservation::new(
            CacheKind::KvState,
            kv_action,
            match kv_action {
                CacheAction::Reuse => "device-resident kv state was reused",
                CacheAction::Rebuild => "device-resident kv state was rebuilt",
                CacheAction::Bypass => "device-resident kv state was bypassed",
                CacheAction::Invalidate => "device-resident kv state was invalidated",
                CacheAction::Restore => "device-resident kv state was restored",
            },
        ));
        Self {
            kv_accounting: KvCacheAccounting::from_states(before, current),
            prefix_state,
            prefix_identity,
            observations,
        }
    }
}

impl MetalAttentionGraphRuntime {
    fn new(
        backend: &mut MetalBackend,
        reserve: MetalAttentionGraphReserve,
    ) -> Result<Self, RuntimeError> {
        let identity = graph_identity(&reserve);
        let output_buffer = backend.input_buffer(
            graph_output_shape(&identity),
            graph_zeroed_output(&identity),
        )?;
        let command_label = graph_command_label(&identity);
        let last_metrics = MetalGraphReuseMetrics {
            identity: identity.clone(),
            compile_path: metal_graph_reserve_evidence(identity.kind, false),
            command_label: command_label.clone(),
            command_state_reused: false,
            reserved_output_bytes: graph_output_bytes(&identity),
            reuse_count: 0,
            rebuild_count: 1,
        };
        Ok(Self {
            identity,
            output_buffer,
            command_label,
            reuse_count: 0,
            rebuild_count: 1,
            last_metrics,
        })
    }

    /// Returns the current reserved graph identity.
    #[must_use]
    pub fn identity(&self) -> &MetalGraphIdentity {
        &self.identity
    }

    /// Returns the latest reserve/reuse evidence for this runtime.
    #[must_use]
    pub fn metrics(&self) -> &MetalGraphReuseMetrics {
        &self.last_metrics
    }

    fn ensure_reserved(
        &mut self,
        backend: &mut MetalBackend,
        reserve: MetalAttentionGraphReserve,
    ) -> Result<MetalGraphReuseMetrics, RuntimeError> {
        let identity = graph_identity(&reserve);
        let reused = self.identity == identity;
        if reused {
            self.reuse_count = self.reuse_count.saturating_add(1);
        } else {
            self.identity = identity.clone();
            self.output_buffer = backend.input_buffer(
                graph_output_shape(&identity),
                graph_zeroed_output(&identity),
            )?;
            self.command_label = graph_command_label(&identity);
            self.rebuild_count = self.rebuild_count.saturating_add(1);
        }
        self.last_metrics = MetalGraphReuseMetrics {
            identity,
            compile_path: metal_graph_reserve_evidence(self.identity.kind, reused),
            command_label: self.command_label.clone(),
            command_state_reused: reused,
            reserved_output_bytes: graph_output_bytes(&self.identity),
            reuse_count: self.reuse_count,
            rebuild_count: self.rebuild_count,
        };
        Ok(self.last_metrics.clone())
    }
}

impl Allocator for MetalBackend {
    type Buffer = MetalBuffer;

    fn allocate(&mut self, spec: &TensorSpec) -> Result<Self::Buffer, RuntimeError> {
        match &mut self.state {
            MetalBackendState::Available(backend) => backend.allocate(spec),
            MetalBackendState::Unavailable(health) => Err(RuntimeError::Backend(format!(
                "metal backend unavailable: {}",
                health.message
            ))),
        }
    }
}

impl ExecutionBackend for MetalBackend {
    type Buffer = MetalBuffer;

    fn execute(
        &mut self,
        plan: &ExecutionPlan,
        inputs: &BTreeMap<TensorId, Self::Buffer>,
    ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError> {
        validate_supported_plan(plan)?;
        match &mut self.state {
            MetalBackendState::Available(backend) => backend.execute(plan, inputs),
            MetalBackendState::Unavailable(health) => Err(RuntimeError::Backend(format!(
                "metal backend unavailable: {}",
                health.message
            ))),
        }
    }
}

impl AvailableMetalBackend {
    fn lookup_or_compile(
        &mut self,
        graph: &Graph,
    ) -> Result<(ExecutionPlan, String, CompilePathEvidence), RuntimeError> {
        let kernel_cache_before = self.platform.kernel_cache_report();
        let (plan, plan_digest, plan_cache_hit) =
            self.execution_plan_cache.lookup_or_compile(graph)?;
        let kernel_cache_after = self.platform.kernel_cache_report();
        Ok((
            plan,
            plan_digest,
            metal_compile_path_evidence(plan_cache_hit, &kernel_cache_before, &kernel_cache_after),
        ))
    }

    fn configure_text_generation_runtime(
        &mut self,
        policy: MetalTextGenerationRuntimePolicy,
    ) -> Result<MetalTextGenerationRuntimeResources, RuntimeError> {
        self.pool.set_policy(policy.allocator_pool.clone());
        self.platform
            .configure_kernel_cache_policy(policy.kernel_cache.clone());
        let allocator_pool = self.pool.report();
        let kernel_cache = self.platform.kernel_cache_report();
        let device_memory_budget = self
            .platform
            .device_memory_budget(allocator_pool.policy.max_cached_bytes);
        let admission = metal_text_generation_admission(
            &policy,
            &device_memory_budget,
            &allocator_pool,
            &kernel_cache,
        );
        Ok(MetalTextGenerationRuntimeResources {
            policy,
            allocator_pool,
            kernel_cache,
            device_memory_budget,
            admission,
        })
    }

    fn allocate(&mut self, spec: &TensorSpec) -> Result<MetalBuffer, RuntimeError> {
        if spec.device().kind() != DeviceKind::Metal {
            return Err(RuntimeError::Backend(format!(
                "metal allocator requires a Metal tensor spec, actual device kind {}",
                spec.device().kind()
            )));
        }
        if spec.device().ordinal() != self.descriptor.device.ordinal() {
            return Err(RuntimeError::Backend(format!(
                "metal allocator requires device ordinal {}, actual {}",
                self.descriptor.device.ordinal(),
                spec.device().ordinal()
            )));
        }

        if let Some(mut buffer) = self.pool.take(spec) {
            self.clear_buffer(&mut buffer)?;
            return Ok(buffer);
        }

        let byte_len = spec
            .storage_size()
            .checked_mul(size_of_dtype(spec.dtype()))
            .ok_or_else(|| RuntimeError::Backend(String::from("metal buffer size overflow")))?;
        let storage_mode = self.platform.storage_mode();
        Ok(MetalBuffer {
            spec: spec.clone(),
            byte_len,
            storage_kind: BufferStorageKind::DenseF32,
            storage_mode,
            host_visible: matches!(
                storage_mode,
                MetalStorageMode::Shared | MetalStorageMode::Managed
            ),
            host_writable: true,
            _keepalive: None,
            platform: self.platform.allocate_buffer(byte_len)?,
        })
    }

    fn clear_buffer(&self, buffer: &mut MetalBuffer) -> Result<(), RuntimeError> {
        if buffer.host_visible() {
            buffer.write_bytes(&vec![0u8; buffer.byte_len()])?;
            return Ok(());
        }
        let mut submission = self
            .platform
            .begin_submission(String::from("psionic.pool.clear"))?;
        submission.fill_buffer(&buffer.platform, buffer.byte_len(), 0)?;
        submission.commit(MetalCommandWait::Completed)?;
        Ok(())
    }

    fn buffer_from_tensor_data(
        &mut self,
        spec: &TensorSpec,
        data: &TensorData,
    ) -> Result<MetalBuffer, RuntimeError> {
        match data {
            TensorData::F32(values) => {
                let mut buffer = self.allocate(spec)?;
                buffer.write_f32(values.as_slice())?;
                Ok(buffer)
            }
            TensorData::QuantizedBlocks(data) => {
                validate_quantized_storage(spec, data)?;
                let storage_mode = self.platform.storage_mode();
                let mut buffer = MetalBuffer {
                    spec: spec.clone(),
                    byte_len: data.bytes.len(),
                    storage_kind: BufferStorageKind::QuantizedBlocks {
                        mode: data.mode,
                        layout: data.layout,
                        residency: BufferResidency::Backend,
                    },
                    storage_mode,
                    host_visible: matches!(
                        storage_mode,
                        MetalStorageMode::Shared | MetalStorageMode::Managed
                    ),
                    host_writable: true,
                    _keepalive: None,
                    platform: self.platform.allocate_buffer(data.bytes.len())?,
                };
                buffer.write_bytes(data.bytes.as_slice())?;
                Ok(buffer)
            }
        }
    }

    fn buffer_from_quantized_slice(
        &mut self,
        spec: &TensorSpec,
        mode: psionic_core::QuantizationMode,
        layout: psionic_core::QuantizedBlockLayout,
        bytes: &[u8],
        keepalive: Option<Arc<dyn Any>>,
    ) -> Result<MetalBuffer, RuntimeError> {
        let storage_mode = self.platform.storage_mode();
        if matches!(storage_mode, MetalStorageMode::Shared) {
            if let Some(keepalive) = keepalive {
                return Ok(MetalBuffer {
                    spec: spec.clone(),
                    byte_len: bytes.len(),
                    storage_kind: BufferStorageKind::QuantizedBlocks {
                        mode,
                        layout,
                        residency: BufferResidency::Backend,
                    },
                    storage_mode,
                    host_visible: true,
                    host_writable: false,
                    _keepalive: Some(keepalive),
                    platform: self
                        .platform
                        .buffer_from_bytes_no_copy(bytes, storage_mode)?,
                });
            }
        }
        let mut buffer = MetalBuffer {
            spec: spec.clone(),
            byte_len: bytes.len(),
            storage_kind: BufferStorageKind::QuantizedBlocks {
                mode,
                layout,
                residency: BufferResidency::Backend,
            },
            storage_mode,
            host_visible: matches!(
                storage_mode,
                MetalStorageMode::Shared | MetalStorageMode::Managed
            ),
            host_writable: true,
            _keepalive: None,
            platform: self.platform.allocate_buffer(bytes.len())?,
        };
        buffer.write_bytes(bytes)?;
        Ok(buffer)
    }

    fn run_quantized_matvec(
        &mut self,
        weights: &MetalBuffer,
        byte_offset: usize,
        mode: psionic_core::QuantizationMode,
        rows: usize,
        columns: usize,
        input: &[f32],
    ) -> Result<MetalQuantizedMatvecResult, RuntimeError> {
        let device = self.descriptor.device.clone();
        let mut input_buffer = self.allocate(&TensorSpec::new(
            Shape::new(vec![columns]),
            DType::F32,
            device.clone(),
        ))?;
        input_buffer.write_f32(input)?;
        let output = self.allocate(&TensorSpec::new(Shape::new(vec![rows]), DType::F32, device))?;
        let mut submission = MetalSubmission {
            encoded_operations: 0,
            synchronized_buffers: 0,
            platform: self
                .platform
                .begin_submission(String::from("psionic.quantized_matvec"))?,
        };
        self.platform.encode_quantized_matvec(
            &mut submission.platform,
            weights,
            byte_offset,
            mode,
            rows,
            columns,
            &input_buffer,
            &output,
        )?;
        submission.encoded_operations += 1;
        if self
            .platform
            .synchronize_output(&mut submission.platform, &output)?
        {
            submission.synchronized_buffers += 1;
        }
        submission.commit(MetalCommandWait::Completed)?;
        Ok(MetalQuantizedMatvecResult {
            values: output.read_f32()?,
        })
    }

    fn run_quantized_matvec_select_logits_output(
        &mut self,
        weights: &MetalBuffer,
        byte_offset: usize,
        mode: psionic_core::QuantizationMode,
        rows: usize,
        columns: usize,
        input: &[f32],
        output_mode: MetalLogitsOutputMode,
    ) -> Result<MetalLogitsSelectionResult, RuntimeError> {
        let device = self.descriptor.device.clone();
        let mut input_buffer = self.allocate(&TensorSpec::new(
            Shape::new(vec![columns]),
            DType::F32,
            device.clone(),
        ))?;
        input_buffer.write_f32(input)?;
        let output = self.allocate(&TensorSpec::new(Shape::new(vec![rows]), DType::F32, device))?;
        let mut submission = MetalSubmission {
            encoded_operations: 0,
            synchronized_buffers: 0,
            platform: self
                .platform
                .begin_submission(String::from("psionic.quantized_matvec"))?,
        };
        self.platform.encode_quantized_matvec(
            &mut submission.platform,
            weights,
            byte_offset,
            mode,
            rows,
            columns,
            &input_buffer,
            &output,
        )?;
        submission.encoded_operations += 1;
        if self
            .platform
            .synchronize_output(&mut submission.platform, &output)?
        {
            submission.synchronized_buffers += 1;
        }
        submission.commit(MetalCommandWait::Completed)?;
        match output_mode {
            MetalLogitsOutputMode::GreedyToken => {
                let selected_tokens = argmax_dense_rows(&output, 1, rows, "metal argmax")?;
                Ok(MetalLogitsSelectionResult {
                    selected_tokens,
                    candidates: None,
                    logits: None,
                    metrics: MetalLogitsSelectionMetrics {
                        output_mode,
                        readback_bytes: std::mem::size_of::<u32>().try_into().unwrap_or(u64::MAX),
                        raw_logits_materialized: false,
                    },
                })
            }
            MetalLogitsOutputMode::TopKCandidates(top_k) => {
                let candidates = top_k_dense_rows(&output, 1, rows, top_k, "metal top_k")?;
                let selected_tokens = candidates
                    .indices
                    .chunks_exact(candidates.top_k.max(1))
                    .map(|row| row[0])
                    .collect::<Vec<_>>();
                let readback_bytes = candidates
                    .indices
                    .len()
                    .saturating_mul(std::mem::size_of::<u32>())
                    .saturating_add(
                        candidates
                            .values
                            .len()
                            .saturating_mul(std::mem::size_of::<f32>()),
                    )
                    .try_into()
                    .unwrap_or(u64::MAX);
                Ok(MetalLogitsSelectionResult {
                    selected_tokens,
                    candidates: Some(candidates),
                    logits: None,
                    metrics: MetalLogitsSelectionMetrics {
                        output_mode,
                        readback_bytes,
                        raw_logits_materialized: false,
                    },
                })
            }
            MetalLogitsOutputMode::RawLogits => {
                let logits = output.read_f32()?;
                let selected_tokens =
                    argmax_values(logits.as_slice(), 1, rows, "metal raw logits")?;
                Ok(MetalLogitsSelectionResult {
                    selected_tokens,
                    candidates: None,
                    logits: Some(logits),
                    metrics: MetalLogitsSelectionMetrics {
                        output_mode,
                        readback_bytes: rows
                            .saturating_mul(std::mem::size_of::<f32>())
                            .try_into()
                            .unwrap_or(u64::MAX),
                        raw_logits_materialized: true,
                    },
                })
            }
        }
    }

    fn run_grouped_quantized_matvec(
        &mut self,
        weights: &MetalBuffer,
        mode: psionic_core::QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        columns: usize,
        selected_ids: &[i32],
        input: &MetalBuffer,
    ) -> Result<Vec<f32>, RuntimeError> {
        let total_rows = selected_ids.len().saturating_mul(rows_per_expert);
        let output = self.allocate(&TensorSpec::new(
            Shape::new(vec![total_rows]),
            DType::F32,
            self.descriptor.device.clone(),
        ))?;
        let mut submission = MetalSubmission {
            encoded_operations: 0,
            synchronized_buffers: 0,
            platform: self
                .platform
                .begin_submission(String::from("psionic.mul_mv_id"))?,
        };
        self.platform.encode_grouped_quantized_matvec(
            &mut submission.platform,
            weights,
            mode,
            row_stride,
            rows_per_expert,
            columns,
            selected_ids,
            input,
            &output,
        )?;
        submission.encoded_operations += 1;
        if self
            .platform
            .synchronize_output(&mut submission.platform, &output)?
        {
            submission.synchronized_buffers += 1;
        }
        submission.commit(MetalCommandWait::Completed)?;
        output.read_f32()
    }

    fn run_expert_matvec_f32_ids(
        &mut self,
        weights: &MetalBuffer,
        mode: psionic_core::QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        columns: usize,
        selected_ids: &[i32],
        input: &MetalBuffer,
    ) -> Result<Vec<f32>, RuntimeError> {
        let total_rows = selected_ids.len().saturating_mul(rows_per_expert);
        let output = self.allocate(&TensorSpec::new(
            Shape::new(vec![total_rows]),
            DType::F32,
            self.descriptor.device.clone(),
        ))?;
        let mut submission = MetalSubmission {
            encoded_operations: 0,
            synchronized_buffers: 0,
            platform: self
                .platform
                .begin_submission(String::from("psionic.expert_matvec_f32_ids"))?,
        };
        self.platform.encode_expert_matvec_f32_ids(
            &mut submission.platform,
            weights,
            mode,
            row_stride,
            rows_per_expert,
            columns,
            selected_ids,
            input,
            &output,
        )?;
        submission.encoded_operations += 1;
        if self
            .platform
            .synchronize_output(&mut submission.platform, &output)?
        {
            submission.synchronized_buffers += 1;
        }
        submission.commit(MetalCommandWait::Completed)?;
        output.read_f32()
    }

    fn execute(
        &mut self,
        plan: &ExecutionPlan,
        inputs: &BTreeMap<TensorId, MetalBuffer>,
    ) -> Result<ExecutionResult<MetalBuffer>, RuntimeError> {
        let mut submission = MetalSubmission {
            encoded_operations: 0,
            synchronized_buffers: 0,
            platform: self
                .platform
                .begin_submission(String::from("psionic.execute"))?,
        };
        let mut values = BTreeMap::new();
        let external_input_aliases = plan
            .steps
            .iter()
            .filter(|step| matches!(step.op, ExecutionOp::Input { .. }))
            .map(|step| step.output)
            .collect::<BTreeSet<_>>();

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
                ExecutionOp::Add => {
                    let (left, right) = binary_inputs(step, &values)?;
                    let output = self.allocate(&step.spec)?;
                    self.platform.encode_add(
                        &mut submission.platform,
                        left,
                        right,
                        &output,
                        step.spec.element_count(),
                    )?;
                    submission.encoded_operations += 1;
                    values.insert(step.output, output);
                }
                ExecutionOp::Matmul => {
                    let (left, right) = binary_inputs(step, &values)?;
                    let output = self.allocate(&step.spec)?;
                    self.platform
                        .encode_matmul(&mut submission.platform, left, right, &output)?;
                    submission.encoded_operations += 1;
                    values.insert(step.output, output);
                }
                ExecutionOp::BackendExtension { op } => {
                    values.insert(step.output, self.backend_extension(step, &values, op)?);
                }
                _ => {
                    return Err(RuntimeError::UnsupportedStep(step.op.label().to_string()));
                }
            }
        }

        for output_id in &plan.outputs {
            let Some(buffer) = values.get(output_id) else {
                return Err(RuntimeError::MissingInput(*output_id));
            };
            if self
                .platform
                .synchronize_output(&mut submission.platform, buffer)?
            {
                submission.synchronized_buffers += 1;
            }
        }

        let _report = submission.commit(MetalCommandWait::Completed)?;
        let mut outputs = BTreeMap::new();
        for output_id in &plan.outputs {
            let Some(buffer) = values.remove(output_id) else {
                return Err(RuntimeError::MissingInput(*output_id));
            };
            outputs.insert(*output_id, buffer);
        }
        for (tensor_id, buffer) in values {
            if !external_input_aliases.contains(&tensor_id) {
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

    fn backend_extension(
        &mut self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, MetalBuffer>,
        op: &BackendExtensionOp,
    ) -> Result<MetalBuffer, RuntimeError> {
        match op {
            BackendExtensionOp::RmsNorm { epsilon } => {
                self.rms_norm(step, values, epsilon.to_f32())
            }
            BackendExtensionOp::RotaryEmbedding { interleaved } => {
                self.rotary_embedding(step, values, *interleaved)
            }
            BackendExtensionOp::ScaledDotProductAttention { scale, causal } => {
                self.scaled_dot_product_attention(step, values, scale.to_f32(), *causal)
            }
            _ => Err(RuntimeError::UnsupportedStep(op.label().to_string())),
        }
    }

    fn rms_norm(
        &mut self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, MetalBuffer>,
        epsilon: f32,
    ) -> Result<MetalBuffer, RuntimeError> {
        let input_values = input(step, values, 0)?.read_f32()?;
        let weight_values = input(step, values, 1)?.read_f32()?;
        let last_dim = weight_values.len();
        if last_dim == 0 || input_values.len() % last_dim != 0 {
            return Err(RuntimeError::Backend(String::from(
                "metal rms_norm requires a non-empty last dimension that divides the input length",
            )));
        }

        let mut output = vec![0.0; input_values.len()];
        for (src_row, dst_row) in input_values
            .chunks_exact(last_dim)
            .zip(output.chunks_exact_mut(last_dim))
        {
            let mean_square =
                src_row.iter().map(|value| value * value).sum::<f32>() / last_dim as f32;
            let inv = (mean_square + epsilon).sqrt().recip();
            for ((dst, value), scale) in dst_row
                .iter_mut()
                .zip(src_row.iter())
                .zip(weight_values.iter())
            {
                *dst = *value * inv * *scale;
            }
        }

        let mut buffer = self.allocate(&step.spec)?;
        buffer.write_f32(output.as_slice())?;
        Ok(buffer)
    }

    fn rotary_embedding(
        &mut self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, MetalBuffer>,
        interleaved: bool,
    ) -> Result<MetalBuffer, RuntimeError> {
        let input_values = input(step, values, 0)?.read_f32()?;
        let cos_buffer = input(step, values, 1)?;
        let sin_buffer = input(step, values, 2)?;
        let cos_values = cos_buffer.read_f32()?;
        let sin_values = sin_buffer.read_f32()?;
        let output = apply_rotary_embedding_values(
            input_values.as_slice(),
            step.spec.shape().dims(),
            cos_values.as_slice(),
            sin_values.as_slice(),
            cos_buffer.spec().shape().dims(),
            interleaved,
        )?;

        let mut buffer = self.allocate(&step.spec)?;
        buffer.write_f32(output.as_slice())?;
        Ok(buffer)
    }

    fn scaled_dot_product_attention(
        &mut self,
        step: &ExecutionStep,
        values: &BTreeMap<TensorId, MetalBuffer>,
        scale: f32,
        causal: bool,
    ) -> Result<MetalBuffer, RuntimeError> {
        let query = input(step, values, 0)?;
        let key = input(step, values, 1)?;
        let value = input(step, values, 2)?;
        let output = scaled_dot_product_attention_values(
            query.read_f32()?.as_slice(),
            key.read_f32()?.as_slice(),
            value.read_f32()?.as_slice(),
            query.spec().shape().dims(),
            key.spec().shape().dims(),
            value.spec().shape().dims(),
            scale,
            causal,
            device_supports_flash_attention(&self.descriptor),
        )?;
        let mut buffer = self.allocate(&step.spec)?;
        buffer.write_f32(output.as_slice())?;
        Ok(buffer)
    }
}

fn metal_execution_plan_cache_policy() -> ExecutionPlanCachePolicy {
    ExecutionPlanCachePolicy::bounded(
        METAL_EXECUTION_PLAN_CACHE_MAX_ENTRIES,
        Some(METAL_EXECUTION_PLAN_CACHE_MAX_CACHED_BYTES),
    )
}

#[derive(Clone, Debug)]
struct CachedMetalExecutionPlan {
    plan: ExecutionPlan,
    plan_digest: String,
}

#[derive(Clone, Debug)]
struct MetalExecutionPlanCache {
    policy: ExecutionPlanCachePolicy,
    cached: HashMap<String, CachedMetalExecutionPlan>,
    state: ExecutionPlanCacheState,
}

impl MetalExecutionPlanCache {
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
                CachedMetalExecutionPlan {
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

fn metal_compile_path_evidence(
    plan_cache_hit: bool,
    kernel_cache_before: &psionic_runtime::KernelCacheReport,
    kernel_cache_after: &psionic_runtime::KernelCacheReport,
) -> CompilePathEvidence {
    let execution_plan_cache = if plan_cache_hit {
        CacheObservation::new(
            CacheKind::ExecutionPlan,
            CacheAction::Reuse,
            "reused a cached metal execution plan",
        )
    } else {
        CacheObservation::new(
            CacheKind::ExecutionPlan,
            CacheAction::Rebuild,
            "compiled a new metal execution plan",
        )
    };
    let kernel_cache = if !kernel_cache_after.policy.enabled {
        CacheObservation::new(
            CacheKind::KernelCache,
            CacheAction::Bypass,
            "metal kernel cache is disabled for this backend path",
        )
    } else if kernel_cache_after.state.cached_entries > kernel_cache_before.state.cached_entries
        || kernel_cache_after.state.cached_bytes > kernel_cache_before.state.cached_bytes
    {
        CacheObservation::new(
            CacheKind::KernelCache,
            CacheAction::Rebuild,
            "compiled at least one new metal kernel or pipeline",
        )
    } else {
        CacheObservation::new(
            CacheKind::KernelCache,
            CacheAction::Reuse,
            "reused the existing metal kernel cache",
        )
    };
    CompilePathEvidence {
        temperature: if plan_cache_hit {
            CompilePathTemperature::WarmReuse
        } else {
            CompilePathTemperature::ColdCompile
        },
        execution_plan_cache,
        kernel_cache,
    }
}

fn metal_graph_reserve_evidence(kind: MetalGraphReserveKind, reused: bool) -> CompilePathEvidence {
    let label = kind.label();
    CompilePathEvidence {
        temperature: if reused {
            CompilePathTemperature::WarmReuse
        } else {
            CompilePathTemperature::ColdCompile
        },
        execution_plan_cache: CacheObservation::new(
            CacheKind::ExecutionPlan,
            if reused {
                CacheAction::Reuse
            } else {
                CacheAction::Rebuild
            },
            if reused {
                format!("reused reserved metal {label} graph identity")
            } else {
                format!("rebuilt reserved metal {label} graph identity")
            },
        ),
        kernel_cache: CacheObservation::new(
            CacheKind::KernelCache,
            CacheAction::Reuse,
            format!("reused the configured metal kernel cache for the {label} graph"),
        ),
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

fn size_of_dtype(dtype: DType) -> usize {
    dtype.element_size_bytes()
}

fn graph_identity(reserve: &MetalAttentionGraphReserve) -> MetalGraphIdentity {
    MetalGraphIdentity {
        kind: reserve.kind,
        batch_size: reserve.batch_size,
        sequence_len: reserve.sequence_len,
        query_head_count: reserve.query_head_count,
        kv_head_count: reserve.kv_head_count,
        head_dim: reserve.head_dim,
        max_context_tokens: reserve.max_context_tokens,
        causal: reserve.causal,
        interleaved: reserve.interleaved,
        flash_attention: reserve.flash_attention,
        stable_digest: format!(
            "metal:{}:b{}:s{}:q{}:kv{}:d{}:ctx{}:causal{}:rope{}:flash{}",
            reserve.kind.label(),
            reserve.batch_size,
            reserve.sequence_len,
            reserve.query_head_count,
            reserve.kv_head_count,
            reserve.head_dim,
            reserve.max_context_tokens,
            reserve.causal,
            reserve.interleaved,
            reserve.flash_attention
        ),
    }
}

fn graph_output_shape(identity: &MetalGraphIdentity) -> Shape {
    Shape::new(vec![
        identity.batch_size,
        identity.query_head_count,
        identity.sequence_len,
        identity.head_dim,
    ])
}

fn graph_output_bytes(identity: &MetalGraphIdentity) -> u64 {
    identity
        .batch_size
        .saturating_mul(identity.query_head_count)
        .saturating_mul(identity.sequence_len)
        .saturating_mul(identity.head_dim)
        .saturating_mul(std::mem::size_of::<f32>())
        .try_into()
        .unwrap_or(u64::MAX)
}

fn graph_zeroed_output(identity: &MetalGraphIdentity) -> Vec<f32> {
    vec![
        0.0;
        identity
            .batch_size
            .saturating_mul(identity.query_head_count)
            .saturating_mul(identity.sequence_len)
            .saturating_mul(identity.head_dim)
    ]
}

fn graph_command_label(identity: &MetalGraphIdentity) -> String {
    format!(
        "psionic.metal.{}.{}",
        identity.kind.label(),
        identity.stable_digest
    )
}

fn reserve_from_decode_inputs(
    query_dims: &[usize],
    key_dims: &[usize],
    max_context_tokens: usize,
    causal: bool,
    interleaved: bool,
    flash_attention: bool,
) -> Result<MetalAttentionGraphReserve, RuntimeError> {
    if query_dims.len() != 4 || key_dims.len() != 4 {
        return Err(RuntimeError::Backend(String::from(
            "metal decode graph reserve requires rank-4 query/key tensors",
        )));
    }
    Ok(MetalAttentionGraphReserve {
        kind: MetalGraphReserveKind::Decode,
        batch_size: query_dims[0],
        sequence_len: query_dims[2],
        query_head_count: query_dims[1],
        kv_head_count: key_dims[1],
        head_dim: query_dims[3],
        max_context_tokens,
        causal,
        interleaved,
        flash_attention,
    })
}

fn validate_decode_attention_shapes(
    query_dims: &[usize],
    key_dims: &[usize],
    value_dims: &[usize],
    cache_width: usize,
) -> Result<(), RuntimeError> {
    if query_dims.len() != 4 || key_dims.len() != 4 || value_dims.len() != 4 {
        return Err(RuntimeError::Backend(String::from(
            "metal decode attention requires rank-4 query/key/value tensors",
        )));
    }
    if query_dims[0] != 1 || key_dims[0] != 1 || value_dims[0] != 1 {
        return Err(RuntimeError::Backend(String::from(
            "metal decode attention currently requires batch size 1",
        )));
    }
    if query_dims[2] != 1 || key_dims[2] != 1 || value_dims[2] != 1 {
        return Err(RuntimeError::Backend(String::from(
            "metal decode attention currently requires a single decode token",
        )));
    }
    if key_dims[1] != value_dims[1] || key_dims[3] != value_dims[3] {
        return Err(RuntimeError::Backend(String::from(
            "metal decode attention requires matching key/value head geometry",
        )));
    }
    if query_dims[3] != key_dims[3] {
        return Err(RuntimeError::Backend(String::from(
            "metal decode attention requires matching query/key head dimensions",
        )));
    }

    let query_head_count = query_dims[1];
    let kv_head_count = key_dims[1];
    let head_dim = query_dims[3];
    if query_head_count == 0 || kv_head_count == 0 || head_dim == 0 {
        return Err(RuntimeError::Backend(String::from(
            "metal decode attention requires non-zero head geometry",
        )));
    }
    if query_head_count % kv_head_count != 0 {
        return Err(RuntimeError::Backend(format!(
            "metal decode attention requires query heads {} to be divisible by kv heads {}",
            query_head_count, kv_head_count
        )));
    }

    let required_cache_width = kv_head_count.saturating_mul(head_dim);
    if cache_width != required_cache_width {
        return Err(RuntimeError::Backend(format!(
            "metal decode attention cache width mismatch: expected {}, actual {}",
            required_cache_width, cache_width
        )));
    }
    Ok(())
}

fn metal_text_generation_admission(
    policy: &MetalTextGenerationRuntimePolicy,
    device_memory_budget: &DeviceMemoryBudget,
    allocator_pool: &AllocatorPoolReport,
    kernel_cache: &KernelCacheReport,
) -> MetalTextGenerationAdmission {
    let reserved_bytes = allocator_pool.policy.max_cached_bytes.saturating_add(
        kernel_cache
            .policy
            .max_cached_bytes
            .unwrap_or(kernel_cache.state.cached_bytes),
    );
    let refusal_reason = if let Some(total_bytes) = device_memory_budget.total_bytes {
        if reserved_bytes > total_bytes {
            Some(format!(
                "metal text-generation runtime reserves {} bytes, exceeding total device budget {}",
                reserved_bytes, total_bytes
            ))
        } else {
            policy.minimum_available_bytes.and_then(|minimum_available_bytes| {
                device_memory_budget
                    .available_execution_bytes
                    .filter(|available| *available < minimum_available_bytes)
                    .map(|available| {
                        format!(
                            "metal text-generation runtime reserves {} allocator bytes and {} kernel-cache bytes, leaving {} execution bytes below required {}",
                            allocator_pool.policy.max_cached_bytes,
                            kernel_cache.policy.max_cached_bytes.unwrap_or(kernel_cache.state.cached_bytes),
                            available,
                            minimum_available_bytes
                        )
                    })
            })
        }
    } else {
        None
    };
    MetalTextGenerationAdmission {
        admitted: refusal_reason.is_none(),
        refusal_reason,
    }
}

fn validate_quantized_storage(
    spec: &TensorSpec,
    data: &psionic_core::QuantizedTensorData,
) -> Result<(), RuntimeError> {
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
    let Some(expected_layout) = data.mode.ggml_block_layout(spec.shape()) else {
        return Err(RuntimeError::Backend(format!(
            "shape {} is invalid for quantized mode {:?}",
            spec.shape(),
            data.mode
        )));
    };
    if expected_layout != data.layout {
        return Err(RuntimeError::Backend(format!(
            "quantized layout mismatch: expected {:?}, actual {:?}",
            expected_layout, data.layout
        )));
    }
    if data.bytes.len() != data.layout.byte_len() {
        return Err(RuntimeError::Backend(format!(
            "quantized byte length mismatch: expected {}, actual {}",
            data.layout.byte_len(),
            data.bytes.len()
        )));
    }
    Ok(())
}

fn validate_supported_plan(plan: &ExecutionPlan) -> Result<(), RuntimeError> {
    for step in &plan.steps {
        validate_supported_step(step)?;
    }
    Ok(())
}

fn validate_supported_step(step: &ExecutionStep) -> Result<(), RuntimeError> {
    ensure_supported_spec(&step.spec)?;
    match &step.op {
        ExecutionOp::Input { .. } => {
            if !step.inputs.is_empty() {
                return Err(RuntimeError::Backend(format!(
                    "metal input step {} unexpectedly has inputs",
                    step.output
                )));
            }
        }
        ExecutionOp::Constant { data } => match data {
            TensorData::F32(values) => {
                if values.len() != step.spec.storage_size() {
                    return Err(RuntimeError::Backend(format!(
                        "metal constant {} payload length mismatch",
                        step.output
                    )));
                }
            }
            TensorData::QuantizedBlocks(data) => {
                validate_quantized_storage(&step.spec, data)?;
            }
        },
        ExecutionOp::Add => {
            if step.inputs.len() != 2 {
                return Err(RuntimeError::Backend(format!(
                    "metal add step {} requires two inputs",
                    step.output
                )));
            }
        }
        ExecutionOp::Matmul => {
            if step.inputs.len() != 2 {
                return Err(RuntimeError::Backend(format!(
                    "metal matmul step {} requires two inputs",
                    step.output
                )));
            }
            let dims = step.spec.shape().dims();
            if dims.len() != 2 {
                return Err(RuntimeError::Backend(format!(
                    "metal matmul step {} requires a rank-2 output, actual rank {}",
                    step.output,
                    dims.len()
                )));
            }
        }
        ExecutionOp::BackendExtension { op } => match op {
            BackendExtensionOp::RmsNorm { .. } => {
                if step.inputs.len() != 2 {
                    return Err(RuntimeError::Backend(format!(
                        "metal rms_norm step {} requires two inputs",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::RotaryEmbedding { .. } => {
                if step.inputs.len() != 3 {
                    return Err(RuntimeError::Backend(format!(
                        "metal rotary_embedding step {} requires three inputs",
                        step.output
                    )));
                }
            }
            BackendExtensionOp::ScaledDotProductAttention { .. } => {
                if step.inputs.len() != 3 {
                    return Err(RuntimeError::Backend(format!(
                        "metal scaled_dot_product_attention step {} requires three inputs",
                        step.output
                    )));
                }
            }
            _ => {
                return Err(RuntimeError::UnsupportedStep(op.label().to_string()));
            }
        },
        _ => {
            return Err(RuntimeError::UnsupportedStep(step.op.label().to_string()));
        }
    }
    Ok(())
}

fn ensure_supported_spec(spec: &TensorSpec) -> Result<(), RuntimeError> {
    if spec.dtype() != DType::F32 {
        return Err(RuntimeError::Backend(format!(
            "metal dense surface only supports F32 tensors, actual {:?}",
            spec.dtype()
        )));
    }
    if spec.device().kind() != DeviceKind::Metal {
        return Err(RuntimeError::Backend(format!(
            "metal dense surface requires Metal tensor specs, actual device kind {}",
            spec.device().kind()
        )));
    }
    if !spec.layout().is_contiguous() || spec.layout().offset() != 0 {
        return Err(RuntimeError::Backend(String::from(
            "metal dense surface requires contiguous zero-offset tensors",
        )));
    }
    Ok(())
}

fn binary_inputs<'a>(
    step: &ExecutionStep,
    values: &'a BTreeMap<TensorId, MetalBuffer>,
) -> Result<(&'a MetalBuffer, &'a MetalBuffer), RuntimeError> {
    let left = input(step, values, 0)?;
    let right = input(step, values, 1)?;
    if left.spec() != right.spec() && !matches!(step.op, ExecutionOp::Matmul) {
        return Err(RuntimeError::Backend(format!(
            "metal {} requires matching input specs",
            step.op.label()
        )));
    }
    Ok((left, right))
}

fn input<'a>(
    step: &ExecutionStep,
    values: &'a BTreeMap<TensorId, MetalBuffer>,
    index: usize,
) -> Result<&'a MetalBuffer, RuntimeError> {
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

fn dense_row_major_values(
    input: &MetalBuffer,
    row_count: usize,
    column_count: usize,
    label: &str,
) -> Result<Vec<f32>, RuntimeError> {
    let element_count = row_count
        .checked_mul(column_count)
        .ok_or_else(|| RuntimeError::Backend(format!("{label} shape overflow")))?;
    let values = input.read_f32()?;
    if values.len() != element_count {
        return Err(RuntimeError::Backend(format!(
            "{label} shape mismatch: expected {element_count} values, actual {}",
            values.len()
        )));
    }
    Ok(values)
}

fn argmax_dense_rows(
    input: &MetalBuffer,
    row_count: usize,
    column_count: usize,
    label: &str,
) -> Result<Vec<u32>, RuntimeError> {
    let mut indices = Vec::with_capacity(row_count);
    for row_index in 0..row_count {
        let row = read_dense_f32_row(input, row_index, column_count, label)?;
        let mut best_index = 0usize;
        let mut best_value = row[0];
        for (index, value) in row.iter().copied().enumerate().skip(1) {
            if value > best_value {
                best_value = value;
                best_index = index;
            }
        }
        indices.push(u32::try_from(best_index).map_err(|_| {
            RuntimeError::Backend(String::from("metal argmax index conversion overflow"))
        })?);
    }
    Ok(indices)
}

fn top_k_dense_rows(
    input: &MetalBuffer,
    row_count: usize,
    column_count: usize,
    top_k: usize,
    label: &str,
) -> Result<MetalTopKResult, RuntimeError> {
    validate_dense_row_selection(input, row_count, column_count, label)?;
    let top_k = top_k.min(column_count);
    let mut indices = Vec::with_capacity(row_count.saturating_mul(top_k));
    let mut selected_values = Vec::with_capacity(row_count.saturating_mul(top_k));

    for row_index in 0..row_count {
        let row = read_dense_f32_row(input, row_index, column_count, label)?;
        let mut row_indices = (0..row.len()).collect::<Vec<_>>();
        row_indices.sort_by(|left, right| {
            row[*right]
                .partial_cmp(&row[*left])
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.cmp(right))
        });
        row_indices.truncate(top_k);
        for index in row_indices {
            indices.push(u32::try_from(index).map_err(|_| {
                RuntimeError::Backend(String::from("metal top_k index conversion overflow"))
            })?);
            selected_values.push(row[index]);
        }
    }

    Ok(MetalTopKResult {
        row_count,
        top_k,
        indices,
        values: selected_values,
    })
}

fn argmax_values(
    values: &[f32],
    row_count: usize,
    column_count: usize,
    label: &str,
) -> Result<Vec<u32>, RuntimeError> {
    if column_count == 0 {
        return Err(RuntimeError::Backend(format!(
            "{label} requires at least one column",
        )));
    }
    let expected_len = row_count
        .checked_mul(column_count)
        .ok_or_else(|| RuntimeError::Backend(format!("{label} shape overflow")))?;
    if values.len() != expected_len {
        return Err(RuntimeError::Backend(format!(
            "{label} shape mismatch: expected {expected_len} values, actual {}",
            values.len()
        )));
    }

    let mut indices = Vec::with_capacity(row_count);
    for row in values.chunks_exact(column_count) {
        let mut best_index = 0usize;
        let mut best_value = row[0];
        for (index, value) in row.iter().copied().enumerate().skip(1) {
            if value > best_value {
                best_value = value;
                best_index = index;
            }
        }
        indices.push(u32::try_from(best_index).map_err(|_| {
            RuntimeError::Backend(String::from("metal raw logits index conversion overflow"))
        })?);
    }
    Ok(indices)
}

fn validate_dense_row_selection(
    input: &MetalBuffer,
    row_count: usize,
    column_count: usize,
    label: &str,
) -> Result<usize, RuntimeError> {
    if column_count == 0 {
        return Err(RuntimeError::Backend(format!(
            "{label} requires at least one column",
        )));
    }
    if input.storage_kind() != BufferStorageKind::DenseF32 {
        return Err(RuntimeError::Backend(format!(
            "{label} requires dense f32 storage, actual {:?}",
            input.storage_kind()
        )));
    }
    let expected_len = row_count
        .checked_mul(column_count)
        .ok_or_else(|| RuntimeError::Backend(format!("{label} shape overflow")))?;
    if input.spec().storage_size() != expected_len {
        return Err(RuntimeError::Backend(format!(
            "{label} shape mismatch: expected {expected_len} values, actual {}",
            input.spec().storage_size()
        )));
    }
    column_count
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| RuntimeError::Backend(format!("{label} byte width overflow")))
}

fn read_dense_f32_row(
    input: &MetalBuffer,
    row_index: usize,
    column_count: usize,
    label: &str,
) -> Result<Vec<f32>, RuntimeError> {
    if column_count == 0 {
        return Err(RuntimeError::Backend(format!(
            "{label} requires at least one column",
        )));
    }
    if input.storage_kind() != BufferStorageKind::DenseF32 {
        return Err(RuntimeError::Backend(format!(
            "{label} requires dense f32 storage, actual {:?}",
            input.storage_kind()
        )));
    }
    if input.spec().storage_size() % column_count != 0 {
        return Err(RuntimeError::Backend(format!(
            "{label} storage size {} is not divisible by row width {}",
            input.spec().storage_size(),
            column_count
        )));
    }
    let row_count = input.spec().storage_size() / column_count;
    if row_index >= row_count {
        return Err(RuntimeError::Backend(format!(
            "{label} row index {} exceeds row count {}",
            row_index, row_count
        )));
    }
    let row_byte_len = column_count
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| RuntimeError::Backend(format!("{label} byte width overflow")))?;
    let byte_offset = row_index
        .checked_mul(row_byte_len)
        .ok_or_else(|| RuntimeError::Backend(format!("{label} byte offset overflow")))?;
    bytes_to_f32_vec(
        input
            .read_bytes_at_offset(byte_offset, row_byte_len)?
            .as_slice(),
    )
}

fn apply_rotary_embedding_values(
    input: &[f32],
    dims: &[usize],
    cos: &[f32],
    sin: &[f32],
    cos_dims: &[usize],
    interleaved: bool,
) -> Result<Vec<f32>, RuntimeError> {
    if dims.len() != 4 {
        return Err(RuntimeError::Backend(format!(
            "metal rotary_embedding requires rank-4 tensors, actual rank {}",
            dims.len()
        )));
    }
    let batch = dims[0];
    let heads = dims[1];
    let seq_len = dims[2];
    let head_dim = dims[3];
    if head_dim % 2 != 0 {
        return Err(RuntimeError::Backend(String::from(
            "metal rotary_embedding requires an even head dimension",
        )));
    }

    let expected_input = batch
        .saturating_mul(heads)
        .saturating_mul(seq_len)
        .saturating_mul(head_dim);
    if input.len() != expected_input {
        return Err(RuntimeError::Backend(String::from(
            "metal rotary_embedding input length does not match tensor shape",
        )));
    }

    let half_dim = head_dim / 2;
    let batched_cos = cos_dims.len() == 3;
    let expected_cos = if batched_cos {
        batch.saturating_mul(seq_len).saturating_mul(half_dim)
    } else {
        seq_len.saturating_mul(half_dim)
    };
    if cos.len() != expected_cos || sin.len() != expected_cos {
        return Err(RuntimeError::Backend(format!(
            "metal rotary_embedding cos/sin length mismatch: expected {}, actual {} / {}",
            expected_cos,
            cos.len(),
            sin.len()
        )));
    }

    let mut output = input.to_vec();
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
                    output[left_index] = left * cosine - right * sine;
                    output[right_index] = left * sine + right * cosine;
                }
            }
        }
    }
    Ok(output)
}

fn scaled_dot_product_attention_values(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    query_dims: &[usize],
    key_dims: &[usize],
    value_dims: &[usize],
    scale: f32,
    causal: bool,
    flash_attention: bool,
) -> Result<Vec<f32>, RuntimeError> {
    if query_dims.len() != 4 || key_dims.len() != 4 || value_dims.len() != 4 {
        return Err(RuntimeError::Backend(String::from(
            "metal scaled_dot_product_attention requires rank-4 tensors",
        )));
    }
    let valid = query_dims[0] == key_dims[0]
        && query_dims[0] == value_dims[0]
        && query_dims[1] == key_dims[1]
        && query_dims[1] == value_dims[1]
        && key_dims[2] == value_dims[2]
        && query_dims[3] == key_dims[3];
    if !valid {
        return Err(RuntimeError::Backend(format!(
            "metal scaled_dot_product_attention shape mismatch: query={:?} key={:?} value={:?}",
            query_dims, key_dims, value_dims
        )));
    }

    let batch = query_dims[0];
    let heads = query_dims[1];
    let query_seq = query_dims[2];
    let key_seq = key_dims[2];
    let head_dim = query_dims[3];
    let value_dim = value_dims[3];
    let expected_query = batch
        .saturating_mul(heads)
        .saturating_mul(query_seq)
        .saturating_mul(head_dim);
    let expected_key = batch
        .saturating_mul(heads)
        .saturating_mul(key_seq)
        .saturating_mul(head_dim);
    let expected_value = batch
        .saturating_mul(heads)
        .saturating_mul(key_seq)
        .saturating_mul(value_dim);
    if query.len() != expected_query || key.len() != expected_key || value.len() != expected_value {
        return Err(RuntimeError::Backend(String::from(
            "metal scaled_dot_product_attention buffer length does not match tensor shapes",
        )));
    }

    let mut output = vec![0.0; batch * heads * query_seq * value_dim];
    if flash_attention {
        for batch_index in 0..batch {
            for head_index in 0..heads {
                for query_index in 0..query_seq {
                    let query_base =
                        ((batch_index * heads + head_index) * query_seq + query_index) * head_dim;
                    let output_base =
                        ((batch_index * heads + head_index) * query_seq + query_index) * value_dim;
                    let mut running_max = f32::NEG_INFINITY;
                    let mut running_sum = 0.0;
                    let mut running_output = vec![0.0; value_dim];
                    for key_index in 0..key_seq {
                        if causal && key_index > query_index {
                            continue;
                        }
                        let key_base =
                            ((batch_index * heads + head_index) * key_seq + key_index) * head_dim;
                        let value_base =
                            ((batch_index * heads + head_index) * key_seq + key_index) * value_dim;
                        let mut score = 0.0;
                        for dim in 0..head_dim {
                            score += query[query_base + dim] * key[key_base + dim];
                        }
                        score *= scale;

                        let next_max = running_max.max(score);
                        let rescale = if running_sum == 0.0 {
                            0.0
                        } else {
                            (running_max - next_max).exp()
                        };
                        let weight = (score - next_max).exp();
                        for dim in 0..value_dim {
                            running_output[dim] =
                                running_output[dim] * rescale + value[value_base + dim] * weight;
                        }
                        running_sum = running_sum * rescale + weight;
                        running_max = next_max;
                    }
                    if running_sum > 0.0 {
                        for dim in 0..value_dim {
                            output[output_base + dim] = running_output[dim] / running_sum;
                        }
                    }
                }
            }
        }
        return Ok(output);
    }

    let mut scores = vec![0.0; key_seq];
    let mut weights = vec![0.0; key_seq];
    for batch_index in 0..batch {
        for head_index in 0..heads {
            for query_index in 0..query_seq {
                let query_base =
                    ((batch_index * heads + head_index) * query_seq + query_index) * head_dim;
                let mut max_score = f32::NEG_INFINITY;
                let mut valid_scores = 0usize;
                for key_index in 0..key_seq {
                    if causal && key_index > query_index {
                        scores[key_index] = f32::NEG_INFINITY;
                        continue;
                    }
                    let key_base =
                        ((batch_index * heads + head_index) * key_seq + key_index) * head_dim;
                    let mut dot = 0.0;
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

                let output_base =
                    ((batch_index * heads + head_index) * query_seq + query_index) * value_dim;
                for key_index in 0..key_seq {
                    let normalized = weights[key_index] / weight_sum;
                    if normalized == 0.0 {
                        continue;
                    }
                    let value_base =
                        ((batch_index * heads + head_index) * key_seq + key_index) * value_dim;
                    for dim in 0..value_dim {
                        output[output_base + dim] += normalized * value[value_base + dim];
                    }
                }
            }
        }
    }
    Ok(output)
}

fn flatten_decode_heads(
    values: &[f32],
    head_count: usize,
    head_dim: usize,
) -> Result<Vec<f32>, RuntimeError> {
    let expected_len = head_count.saturating_mul(head_dim);
    if values.len() != expected_len {
        return Err(RuntimeError::Backend(format!(
            "metal decode attention head flatten length mismatch: expected {}, actual {}",
            expected_len,
            values.len()
        )));
    }
    Ok(values.to_vec())
}

fn expand_kv_cache_for_attention(
    cache: &MetalKvCacheMirror,
    query_head_count: usize,
    kv_head_count: usize,
    head_dim: usize,
) -> Result<(Vec<f32>, Vec<f32>), RuntimeError> {
    let token_count = cache.len();
    let mut keys = vec![0.0; query_head_count * token_count * head_dim];
    let mut values = vec![0.0; query_head_count * token_count * head_dim];
    let heads_per_kv = query_head_count / kv_head_count;
    for token_index in 0..token_count {
        let (token_keys, token_values) = cache.read_entry(token_index)?;
        for query_head_index in 0..query_head_count {
            let kv_head_index = query_head_index / heads_per_kv;
            let src_start = kv_head_index * head_dim;
            let src_end = src_start + head_dim;
            let dst_start = (query_head_index * token_count + token_index) * head_dim;
            let dst_end = dst_start + head_dim;
            keys[dst_start..dst_end].copy_from_slice(&token_keys[src_start..src_end]);
            values[dst_start..dst_end].copy_from_slice(&token_values[src_start..src_end]);
        }
    }
    Ok((keys, values))
}

fn device_supports_flash_attention(descriptor: &DeviceDescriptor) -> bool {
    descriptor
        .feature_flags
        .iter()
        .any(|flag| flag == FLASH_ATTENTION_FEATURE_FLAG)
}

fn validate_grouped_expert_layout(
    weights: &MetalBuffer,
    mode: psionic_core::QuantizationMode,
    row_stride: usize,
    rows_per_expert: usize,
    columns: usize,
) -> Result<usize, RuntimeError> {
    if columns == 0 {
        return Err(RuntimeError::Backend(String::from(
            "metal mul_mv_id requires a non-zero column count",
        )));
    }
    let expected_row_stride = match mode {
        psionic_core::QuantizationMode::None => columns
            .checked_mul(size_of_dtype(DType::F32))
            .ok_or_else(|| {
                RuntimeError::Backend(String::from("metal mul_mv_id row stride overflow"))
            })?,
        psionic_core::QuantizationMode::GgmlQ8_0 | psionic_core::QuantizationMode::GgmlMxfp4 => {
            let Some((elements_per_block, bytes_per_block)) = mode.ggml_block_spec() else {
                return Err(RuntimeError::Backend(format!(
                    "metal mul_mv_id does not support grouped mode {mode:?}",
                )));
            };
            if columns % elements_per_block != 0 {
                return Err(RuntimeError::Backend(format!(
                    "metal mul_mv_id columns {columns} are not block-aligned for {mode:?}",
                )));
            }
            (columns / elements_per_block).saturating_mul(bytes_per_block)
        }
        _ => {
            return Err(RuntimeError::Backend(format!(
                "metal mul_mv_id does not support grouped mode {mode:?}",
            )));
        }
    };
    if row_stride != expected_row_stride {
        return Err(RuntimeError::Backend(format!(
            "metal mul_mv_id row stride mismatch: expected {expected_row_stride}, actual {row_stride}",
        )));
    }
    let rows_per_group = rows_per_expert.checked_mul(row_stride).ok_or_else(|| {
        RuntimeError::Backend(String::from("metal mul_mv_id group size overflow"))
    })?;
    if rows_per_group == 0 || weights.byte_len() % rows_per_group != 0 {
        return Err(RuntimeError::Backend(format!(
            "metal mul_mv_id packed expert buffer length {} is not divisible by grouped row size {}",
            weights.byte_len(),
            rows_per_group
        )));
    }
    Ok(weights.byte_len() / rows_per_group)
}

fn validate_quantized_matvec_request(
    weights: &MetalBuffer,
    byte_offset: usize,
    mode: psionic_core::QuantizationMode,
    rows: usize,
    columns: usize,
    input: &MetalBuffer,
    output: &MetalBuffer,
) -> Result<usize, RuntimeError> {
    let row_stride = quantized_row_stride(mode, columns)?;
    let required_bytes = rows.saturating_mul(row_stride);
    let end_offset = byte_offset.saturating_add(required_bytes);
    match weights.storage_kind() {
        BufferStorageKind::QuantizedBlocks {
            mode: stored_mode, ..
        } if stored_mode == mode => {}
        BufferStorageKind::QuantizedBlocks {
            mode: stored_mode, ..
        } => {
            return Err(RuntimeError::Backend(format!(
                "metal quantized matvec mode mismatch: requested {mode:?}, stored {stored_mode:?}",
            )));
        }
        storage_kind => {
            return Err(RuntimeError::Backend(format!(
                "metal quantized matvec requires quantized block storage, actual {:?}",
                storage_kind
            )));
        }
    }
    if weights.byte_len() < end_offset {
        return Err(RuntimeError::Backend(format!(
            "metal quantized matvec byte length mismatch: required {end_offset}, actual {}",
            weights.byte_len(),
        )));
    }
    if input.storage_kind() != BufferStorageKind::DenseF32 || input.spec().dtype() != DType::F32 {
        return Err(RuntimeError::Backend(format!(
            "metal quantized matvec input requires dense f32 storage, actual {:?}",
            input.storage_kind()
        )));
    }
    if output.storage_kind() != BufferStorageKind::DenseF32 || output.spec().dtype() != DType::F32 {
        return Err(RuntimeError::Backend(format!(
            "metal quantized matvec output requires dense f32 storage, actual {:?}",
            output.storage_kind()
        )));
    }
    if input.spec().storage_size() < columns {
        return Err(RuntimeError::Backend(format!(
            "metal quantized matvec input width mismatch: required at least {columns}, actual {}",
            input.spec().storage_size()
        )));
    }
    if output.spec().storage_size() < rows {
        return Err(RuntimeError::Backend(format!(
            "metal quantized matvec output rows mismatch: required at least {rows}, actual {}",
            output.spec().storage_size()
        )));
    }
    Ok(row_stride)
}

fn validate_grouped_quantized_matvec_request(
    weights: &MetalBuffer,
    mode: psionic_core::QuantizationMode,
    row_stride: usize,
    rows_per_expert: usize,
    columns: usize,
    selected_ids: &[i32],
    input: &MetalBuffer,
    output: &MetalBuffer,
) -> Result<(), RuntimeError> {
    validate_grouped_expert_layout(weights, mode, row_stride, rows_per_expert, columns)?;
    let total_rows = rows_per_expert.saturating_mul(selected_ids.len());
    if input.storage_kind() != BufferStorageKind::DenseF32 || input.spec().dtype() != DType::F32 {
        return Err(RuntimeError::Backend(format!(
            "metal grouped matvec input requires dense f32 storage, actual {:?}",
            input.storage_kind()
        )));
    }
    if output.storage_kind() != BufferStorageKind::DenseF32 || output.spec().dtype() != DType::F32 {
        return Err(RuntimeError::Backend(format!(
            "metal grouped matvec output requires dense f32 storage, actual {:?}",
            output.storage_kind()
        )));
    }
    if input.spec().storage_size() < columns {
        return Err(RuntimeError::Backend(format!(
            "metal grouped matvec input width mismatch: required at least {columns}, actual {}",
            input.spec().storage_size()
        )));
    }
    if output.spec().storage_size() < total_rows {
        return Err(RuntimeError::Backend(format!(
            "metal grouped matvec output rows mismatch: required at least {total_rows}, actual {}",
            output.spec().storage_size()
        )));
    }
    let _ = selected_expert_indices(
        selected_ids,
        validate_grouped_expert_layout(weights, mode, row_stride, rows_per_expert, columns)?,
    )?;
    Ok(())
}

fn validate_expert_matvec_f32_ids_request(
    weights: &MetalBuffer,
    mode: psionic_core::QuantizationMode,
    row_stride: usize,
    rows_per_expert: usize,
    columns: usize,
    selected_ids: &[i32],
    input: &MetalBuffer,
    output: &MetalBuffer,
) -> Result<(), RuntimeError> {
    validate_grouped_expert_layout(weights, mode, row_stride, rows_per_expert, columns)?;
    let total_rows = rows_per_expert.saturating_mul(selected_ids.len());
    let total_inputs = columns.saturating_mul(selected_ids.len());
    if input.storage_kind() != BufferStorageKind::DenseF32 || input.spec().dtype() != DType::F32 {
        return Err(RuntimeError::Backend(format!(
            "metal expert_matvec_f32_ids input requires dense f32 storage, actual {:?}",
            input.storage_kind()
        )));
    }
    if output.storage_kind() != BufferStorageKind::DenseF32 || output.spec().dtype() != DType::F32 {
        return Err(RuntimeError::Backend(format!(
            "metal expert_matvec_f32_ids output requires dense f32 storage, actual {:?}",
            output.storage_kind()
        )));
    }
    if input.spec().storage_size() < total_inputs {
        return Err(RuntimeError::Backend(format!(
            "metal expert_matvec_f32_ids input size mismatch: required at least {total_inputs}, actual {}",
            input.spec().storage_size()
        )));
    }
    if output.spec().storage_size() < total_rows {
        return Err(RuntimeError::Backend(format!(
            "metal expert_matvec_f32_ids output size mismatch: required at least {total_rows}, actual {}",
            output.spec().storage_size()
        )));
    }
    let _ = selected_expert_indices(
        selected_ids,
        validate_grouped_expert_layout(weights, mode, row_stride, rows_per_expert, columns)?,
    )?;
    Ok(())
}

fn quantized_row_stride(
    mode: psionic_core::QuantizationMode,
    columns: usize,
) -> Result<usize, RuntimeError> {
    let Some((elements_per_block, bytes_per_block)) = mode.ggml_block_spec() else {
        return Err(RuntimeError::Backend(format!(
            "metal quantized row stride does not support mode {mode:?}",
        )));
    };
    if columns == 0 || columns % elements_per_block != 0 {
        return Err(RuntimeError::Backend(format!(
            "metal quantized row stride requires block-aligned width {columns} for {mode:?}",
        )));
    }
    (columns / elements_per_block)
        .checked_mul(bytes_per_block)
        .ok_or_else(|| RuntimeError::Backend(String::from("metal quantized row stride overflow")))
}

fn dense_row_dot(lhs: &[f32], rhs: &[f32]) -> Result<f32, RuntimeError> {
    if lhs.len() != rhs.len() {
        return Err(RuntimeError::Backend(format!(
            "metal dense row dot length mismatch: lhs {}, rhs {}",
            lhs.len(),
            rhs.len()
        )));
    }
    Ok(lhs
        .iter()
        .zip(rhs.iter())
        .map(|(left, right)| left * right)
        .sum())
}

fn host_parallelism(work_items: usize) -> usize {
    if work_items < 8 {
        return 1;
    }
    thread::available_parallelism()
        .map(|parallelism| parallelism.get())
        .unwrap_or(1)
        .min(work_items)
        .max(1)
}

fn join_worker<T>(
    worker: thread::ScopedJoinHandle<'_, Result<T, RuntimeError>>,
) -> Result<T, RuntimeError> {
    worker
        .join()
        .map_err(|_| RuntimeError::Backend(String::from("metal worker thread panicked")))?
}

fn selected_expert_indices(
    selected_ids: &[i32],
    expert_count: usize,
) -> Result<Vec<usize>, RuntimeError> {
    selected_ids
        .iter()
        .copied()
        .map(|selected_id| {
            let expert_index = usize::try_from(selected_id).map_err(|_| {
                RuntimeError::Backend(format!(
                    "metal mul_mv_id does not accept negative expert id {selected_id}",
                ))
            })?;
            if expert_index >= expert_count {
                return Err(RuntimeError::Backend(format!(
                    "metal mul_mv_id expert id {expert_index} exceeds packed expert count {expert_count}",
                )));
            }
            Ok(expert_index)
        })
        .collect()
}

fn grouped_dense_expert_dot_into(
    rows_per_expert: usize,
    columns: usize,
    selected_experts: &[usize],
    input: &[f32],
    dense_weights: &[f32],
    output: &mut [f32],
) -> Result<(), RuntimeError> {
    if output.len() != selected_experts.len().saturating_mul(rows_per_expert) {
        return Err(RuntimeError::Backend(format!(
            "metal dense mul_mv_id output length mismatch: expected {}, actual {}",
            selected_experts.len().saturating_mul(rows_per_expert),
            output.len()
        )));
    }
    let thread_count = host_parallelism(selected_experts.len());
    if thread_count == 1 {
        for (output_chunk, expert_index) in output
            .chunks_mut(rows_per_expert)
            .zip(selected_experts.iter().copied())
        {
            for (row, row_value) in output_chunk.iter_mut().enumerate() {
                let row_index = expert_index
                    .saturating_mul(rows_per_expert)
                    .saturating_add(row);
                let row_start = row_index.saturating_mul(columns);
                let row_end = row_start.saturating_add(columns);
                *row_value = dense_row_dot(input, &dense_weights[row_start..row_end])?;
            }
        }
        return Ok(());
    }

    let experts_per_thread = selected_experts.len().div_ceil(thread_count);
    thread::scope(|scope| {
        let mut workers = Vec::new();
        for (output_chunk, expert_chunk) in output
            .chunks_mut(experts_per_thread.saturating_mul(rows_per_expert))
            .zip(selected_experts.chunks(experts_per_thread))
        {
            workers.push(scope.spawn(move || {
                for (selected_output, expert_index) in output_chunk
                    .chunks_mut(rows_per_expert)
                    .zip(expert_chunk.iter().copied())
                {
                    for (row, row_value) in selected_output.iter_mut().enumerate() {
                        let row_index = expert_index
                            .saturating_mul(rows_per_expert)
                            .saturating_add(row);
                        let row_start = row_index.saturating_mul(columns);
                        let row_end = row_start.saturating_add(columns);
                        *row_value = dense_row_dot(input, &dense_weights[row_start..row_end])?;
                    }
                }
                Ok(())
            }));
        }
        for worker in workers {
            join_worker(worker)?;
        }
        Ok(())
    })
}

fn grouped_dense_expert_dot_rows_into(
    rows_per_expert: usize,
    columns: usize,
    selected_experts: &[usize],
    input_rows: &[f32],
    dense_weights: &[f32],
    output: &mut [f32],
) -> Result<(), RuntimeError> {
    if input_rows.len() != selected_experts.len().saturating_mul(columns) {
        return Err(RuntimeError::Backend(format!(
            "metal dense expert_matvec_f32_ids input length mismatch: expected {}, actual {}",
            selected_experts.len().saturating_mul(columns),
            input_rows.len()
        )));
    }
    if output.len() != selected_experts.len().saturating_mul(rows_per_expert) {
        return Err(RuntimeError::Backend(format!(
            "metal dense expert_matvec_f32_ids output length mismatch: expected {}, actual {}",
            selected_experts.len().saturating_mul(rows_per_expert),
            output.len()
        )));
    }
    let thread_count = host_parallelism(selected_experts.len());
    if thread_count == 1 {
        for ((output_chunk, expert_index), input_chunk) in output
            .chunks_mut(rows_per_expert)
            .zip(selected_experts.iter().copied())
            .zip(input_rows.chunks_exact(columns))
        {
            for (row, row_value) in output_chunk.iter_mut().enumerate() {
                let row_index = expert_index
                    .saturating_mul(rows_per_expert)
                    .saturating_add(row);
                let row_start = row_index.saturating_mul(columns);
                let row_end = row_start.saturating_add(columns);
                *row_value = dense_row_dot(input_chunk, &dense_weights[row_start..row_end])?;
            }
        }
        return Ok(());
    }

    let experts_per_thread = selected_experts.len().div_ceil(thread_count);
    thread::scope(|scope| {
        let mut workers = Vec::new();
        for ((output_chunk, expert_chunk), input_chunk) in output
            .chunks_mut(experts_per_thread.saturating_mul(rows_per_expert))
            .zip(selected_experts.chunks(experts_per_thread))
            .zip(input_rows.chunks(experts_per_thread.saturating_mul(columns)))
        {
            workers.push(scope.spawn(move || {
                for ((selected_output, expert_index), selected_input) in output_chunk
                    .chunks_mut(rows_per_expert)
                    .zip(expert_chunk.iter().copied())
                    .zip(input_chunk.chunks_exact(columns))
                {
                    for (row, row_value) in selected_output.iter_mut().enumerate() {
                        let row_index = expert_index
                            .saturating_mul(rows_per_expert)
                            .saturating_add(row);
                        let row_start = row_index.saturating_mul(columns);
                        let row_end = row_start.saturating_add(columns);
                        *row_value =
                            dense_row_dot(selected_input, &dense_weights[row_start..row_end])?;
                    }
                }
                Ok(())
            }));
        }
        for worker in workers {
            join_worker(worker)?;
        }
        Ok(())
    })
}

fn f32_slice_to_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len().saturating_mul(std::mem::size_of::<f32>()));
    for value in values {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn bytes_to_f32_vec(bytes: &[u8]) -> Result<Vec<f32>, RuntimeError> {
    if bytes.len() % std::mem::size_of::<f32>() != 0 {
        return Err(RuntimeError::Backend(format!(
            "metal f32 byte decode requires 4-byte alignment, actual {}",
            bytes.len()
        )));
    }
    let mut values = Vec::with_capacity(bytes.len() / std::mem::size_of::<f32>());
    for chunk in bytes.chunks_exact(std::mem::size_of::<f32>()) {
        values.push(f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(values)
}

fn shared_prefix_len(left: &[u32], right: &[u32]) -> usize {
    left.iter()
        .zip(right.iter())
        .take_while(|(left, right)| left == right)
        .count()
}

fn prefix_identity(
    compatibility: &MetalSharedPrefixCompatibility,
    prompt_tokens: &[u32],
) -> PrefixCacheIdentity {
    PrefixCacheIdentity {
        served_artifact_digest: compatibility.served_artifact_digest.clone(),
        model_id: compatibility.model_id.clone(),
        model_revision: compatibility.model_revision.clone(),
        weight_bundle_digest: compatibility.weight_bundle_digest.clone(),
        tokenizer_family: compatibility.tokenizer_family.clone(),
        tokenizer_digest: None,
        chat_template_digest: None,
        generation_defaults_digest: None,
        backend_compatibility: compatibility.backend_compatibility.clone(),
        prefix_digest: format!(
            "metal-prefix:{}:{}",
            prompt_tokens.len(),
            prompt_tokens
                .iter()
                .map(|token| token.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ),
        prefix_tokens: prompt_tokens.len(),
    }
}

fn prefix_cache_observation(prefix_state: PrefixCacheState) -> CacheObservation {
    match prefix_state {
        PrefixCacheState::None => CacheObservation::new(
            CacheKind::PrefixCache,
            CacheAction::Bypass,
            "no compatible shared prefix entry existed for this prompt",
        ),
        PrefixCacheState::Hit => CacheObservation::new(
            CacheKind::PrefixCache,
            CacheAction::Reuse,
            "compatible shared prefix state was reused on the Metal device",
        ),
        PrefixCacheState::Miss => CacheObservation::new(
            CacheKind::PrefixCache,
            CacheAction::Rebuild,
            "shared prefix reuse missed and a fresh Metal prefix entry must be recorded",
        ),
        PrefixCacheState::Bypassed => CacheObservation::new(
            CacheKind::PrefixCache,
            CacheAction::Bypass,
            "shared prefix reuse was skipped under the current policy",
        ),
        PrefixCacheState::Rebuilt => CacheObservation::new(
            CacheKind::PrefixCache,
            CacheAction::Invalidate,
            "stale Metal shared prefix state was discarded and rebuilt",
        ),
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ptr;

    use metal::{
        Buffer, CommandBuffer, CommandQueue, CompileOptions, ComputePipelineState,
        Device as MetalDevice, DeviceRef as MetalDeviceRef, MTLCommandBufferStatus,
        MTLDeviceLocation, MTLGPUFamily, MTLResourceOptions, MTLSize, NSRange,
    };
    use psionic_core::{DType, Device, DeviceKind, QuantizationMode};
    use psionic_runtime::{
        BufferHandle, DeviceDescriptor, DeviceMemoryBudget, HealthStatus, KernelCachePolicy,
        KernelCacheReport, QuantizationExecution, QuantizationLoadPath, QuantizationSupport,
        RuntimeError, RuntimeHealth,
    };

    use super::{
        DeviceSupportTier, FLASH_ATTENTION_FEATURE_FLAG, FamilySupport, LEGACY_FAMILY_FLAG,
        MODERN_FAMILY_FLAG, MetalBuffer, MetalCommandStatus, MetalCommandWait,
        MetalDiscoveryReport, MetalKernelCache, MetalStorageMode, classify_support,
        quantized_row_stride,
    };

    #[derive(Clone)]
    pub(super) struct PlatformBuffer {
        raw: Buffer,
    }

    struct DensePipelines {
        add: ComputePipelineState,
        matmul: ComputePipelineState,
        quantized_matvec_q8_0: ComputePipelineState,
        quantized_matvec_mxfp4: ComputePipelineState,
        grouped_quantized_matvec_q8_0: ComputePipelineState,
        grouped_quantized_matvec_mxfp4: ComputePipelineState,
        expert_matvec_f32_ids_q8_0: ComputePipelineState,
        expert_matvec_f32_ids_mxfp4: ComputePipelineState,
    }

    impl PlatformBuffer {
        pub(super) fn write_bytes(
            &self,
            bytes: &[u8],
            storage_mode: MetalStorageMode,
        ) -> Result<(), RuntimeError> {
            let contents = self.raw.contents().cast::<u8>();
            if contents.is_null() {
                return Err(RuntimeError::Backend(String::from(
                    "metal buffer is not host visible",
                )));
            }
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), contents, bytes.len());
            }
            if matches!(storage_mode, MetalStorageMode::Managed) {
                self.raw.did_modify_range(byte_range(bytes.len())?);
            }
            Ok(())
        }

        pub(super) fn write_bytes_at_offset(
            &self,
            byte_offset: usize,
            bytes: &[u8],
            storage_mode: MetalStorageMode,
        ) -> Result<(), RuntimeError> {
            let contents = self.raw.contents().cast::<u8>();
            if contents.is_null() {
                return Err(RuntimeError::Backend(String::from(
                    "metal buffer is not host visible",
                )));
            }
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), contents.add(byte_offset), bytes.len());
            }
            if matches!(storage_mode, MetalStorageMode::Managed) {
                self.raw.did_modify_range(NSRange::new(
                    u64::try_from(byte_offset).map_err(|_| {
                        RuntimeError::Backend(String::from("metal ranged write offset overflow"))
                    })?,
                    u64::try_from(bytes.len()).map_err(|_| {
                        RuntimeError::Backend(String::from("metal ranged write length overflow"))
                    })?,
                ));
            }
            Ok(())
        }

        pub(super) fn read_bytes(&self, byte_len: usize) -> Result<Vec<u8>, RuntimeError> {
            let contents = self.raw.contents().cast::<u8>();
            if contents.is_null() {
                return Err(RuntimeError::Backend(String::from(
                    "metal buffer is not host visible",
                )));
            }
            let mut bytes = vec![0u8; byte_len];
            unsafe {
                ptr::copy_nonoverlapping(contents, bytes.as_mut_ptr(), byte_len);
            }
            Ok(bytes)
        }

        pub(super) fn read_bytes_at_offset(
            &self,
            byte_offset: usize,
            byte_len: usize,
        ) -> Result<Vec<u8>, RuntimeError> {
            let contents = self.raw.contents().cast::<u8>();
            if contents.is_null() {
                return Err(RuntimeError::Backend(String::from(
                    "metal buffer is not host visible",
                )));
            }
            let mut bytes = vec![0u8; byte_len];
            unsafe {
                ptr::copy_nonoverlapping(contents.add(byte_offset), bytes.as_mut_ptr(), byte_len);
            }
            Ok(bytes)
        }

        pub(super) fn with_bytes_at_offset<T>(
            &self,
            byte_offset: usize,
            byte_len: usize,
            map: impl FnOnce(&[u8]) -> Result<T, RuntimeError>,
        ) -> Result<T, RuntimeError> {
            let contents = self.raw.contents().cast::<u8>();
            if contents.is_null() {
                return Err(RuntimeError::Backend(String::from(
                    "metal buffer is not host visible",
                )));
            }
            let bytes = unsafe { std::slice::from_raw_parts(contents.add(byte_offset), byte_len) };
            map(bytes)
        }
    }

    pub(super) struct PlatformSubmission {
        command_buffer: CommandBuffer,
    }

    impl PlatformSubmission {
        pub(super) fn fill_buffer(
            &mut self,
            buffer: &PlatformBuffer,
            byte_len: usize,
            value: u8,
        ) -> Result<(), RuntimeError> {
            let encoder = self.command_buffer.new_blit_command_encoder();
            encoder.fill_buffer(&buffer.raw, byte_range(byte_len)?, value);
            encoder.end_encoding();
            Ok(())
        }

        pub(super) fn copy_buffer(
            &mut self,
            source: &PlatformBuffer,
            destination: &PlatformBuffer,
            byte_len: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.command_buffer.new_blit_command_encoder();
            let size = to_metal_size(byte_len)?;
            encoder.copy_from_buffer(&source.raw, 0, &destination.raw, 0, size);
            encoder.end_encoding();
            Ok(())
        }

        pub(super) fn synchronize_buffer(
            &mut self,
            buffer: &PlatformBuffer,
            storage_mode: MetalStorageMode,
        ) -> Result<bool, RuntimeError> {
            if !matches!(storage_mode, MetalStorageMode::Managed) {
                return Ok(false);
            }
            let encoder = self.command_buffer.new_blit_command_encoder();
            encoder.synchronize_resource(&buffer.raw);
            encoder.end_encoding();
            Ok(true)
        }

        pub(super) fn encode_add(
            &mut self,
            pipeline: &ComputePipelineState,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.command_buffer.new_compute_command_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&left.raw), 0);
            encoder.set_buffer(1, Some(&right.raw), 0);
            encoder.set_buffer(2, Some(&output.raw), 0);

            let element_count = u32::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal add element count overflow"))
            })?;
            encoder.set_bytes(3, 4, (&element_count as *const u32).cast());

            let threadgroup_size = compute_threadgroup_size(
                pipeline,
                usize::try_from(element_count).map_err(|_| {
                    RuntimeError::Backend(String::from(
                        "metal add element count conversion overflow",
                    ))
                })?,
            )?;
            encoder.dispatch_threads(
                MTLSize::new(u64::from(element_count), 1, 1),
                threadgroup_size,
            );
            encoder.end_encoding();
            Ok(())
        }

        pub(super) fn encode_matmul(
            &mut self,
            pipeline: &ComputePipelineState,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
            m: usize,
            k: usize,
            n: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.command_buffer.new_compute_command_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&left.raw), 0);
            encoder.set_buffer(1, Some(&right.raw), 0);
            encoder.set_buffer(2, Some(&output.raw), 0);

            let m = u32::try_from(m)
                .map_err(|_| RuntimeError::Backend(String::from("metal matmul m overflow")))?;
            let k = u32::try_from(k)
                .map_err(|_| RuntimeError::Backend(String::from("metal matmul k overflow")))?;
            let n = u32::try_from(n)
                .map_err(|_| RuntimeError::Backend(String::from("metal matmul n overflow")))?;
            encoder.set_bytes(3, 4, (&m as *const u32).cast());
            encoder.set_bytes(4, 4, (&k as *const u32).cast());
            encoder.set_bytes(5, 4, (&n as *const u32).cast());

            let grid_width = u64::from(m)
                .checked_mul(u64::from(n))
                .ok_or_else(|| RuntimeError::Backend(String::from("metal matmul grid overflow")))?;
            let threadgroup_size = compute_threadgroup_size(
                pipeline,
                usize::try_from(grid_width).map_err(|_| {
                    RuntimeError::Backend(String::from("metal matmul grid conversion overflow"))
                })?,
            )?;
            encoder.dispatch_threads(MTLSize::new(grid_width, 1, 1), threadgroup_size);
            encoder.end_encoding();
            Ok(())
        }

        pub(super) fn encode_quantized_matvec(
            &mut self,
            pipeline: &ComputePipelineState,
            weights: &PlatformBuffer,
            byte_offset: usize,
            input: &PlatformBuffer,
            output: &PlatformBuffer,
            rows: usize,
            columns: usize,
            row_stride: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.command_buffer.new_compute_command_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&weights.raw), 0);
            encoder.set_buffer(1, Some(&input.raw), 0);
            encoder.set_buffer(2, Some(&output.raw), 0);

            let rows = u32::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("metal quantized matvec rows overflow"))
            })?;
            let columns = u32::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from("metal quantized matvec columns overflow"))
            })?;
            let row_stride = u32::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from("metal quantized matvec row stride overflow"))
            })?;
            let byte_offset = u64::try_from(byte_offset).map_err(|_| {
                RuntimeError::Backend(String::from("metal quantized matvec byte offset overflow"))
            })?;
            encoder.set_bytes(3, 4, (&rows as *const u32).cast());
            encoder.set_bytes(4, 4, (&columns as *const u32).cast());
            encoder.set_bytes(5, 4, (&row_stride as *const u32).cast());
            encoder.set_bytes(6, 8, (&byte_offset as *const u64).cast());

            let threadgroup_size = quantized_row_threadgroup_size(pipeline)?;
            encoder.dispatch_thread_groups(MTLSize::new(u64::from(rows), 1, 1), threadgroup_size);
            encoder.end_encoding();
            Ok(())
        }

        pub(super) fn encode_grouped_quantized_matvec(
            &mut self,
            pipeline: &ComputePipelineState,
            weights: &PlatformBuffer,
            input: &PlatformBuffer,
            output: &PlatformBuffer,
            rows_per_expert: usize,
            columns: usize,
            row_stride: usize,
            selected_ids: &[i32],
        ) -> Result<(), RuntimeError> {
            let total_rows = selected_ids.len().saturating_mul(rows_per_expert);
            let encoder = self.command_buffer.new_compute_command_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&weights.raw), 0);
            encoder.set_buffer(1, Some(&input.raw), 0);
            encoder.set_buffer(2, Some(&output.raw), 0);

            let rows_per_expert = u32::try_from(rows_per_expert).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "metal grouped matvec rows per expert overflow",
                ))
            })?;
            let columns = u32::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from("metal grouped matvec columns overflow"))
            })?;
            let row_stride = u32::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from("metal grouped matvec row stride overflow"))
            })?;
            let selected_count = u32::try_from(selected_ids.len()).map_err(|_| {
                RuntimeError::Backend(String::from("metal grouped matvec selected count overflow"))
            })?;
            encoder.set_bytes(3, 4, (&rows_per_expert as *const u32).cast());
            encoder.set_bytes(4, 4, (&columns as *const u32).cast());
            encoder.set_bytes(5, 4, (&row_stride as *const u32).cast());
            encoder.set_bytes(6, 4, (&selected_count as *const u32).cast());
            encoder.set_bytes(
                7,
                selected_ids
                    .len()
                    .saturating_mul(std::mem::size_of::<i32>()) as u64,
                selected_ids.as_ptr().cast(),
            );

            let threadgroup_size = quantized_row_threadgroup_size(pipeline)?;
            encoder.dispatch_thread_groups(
                MTLSize::new(
                    u64::try_from(total_rows).map_err(|_| {
                        RuntimeError::Backend(String::from(
                            "metal grouped matvec row count conversion overflow",
                        ))
                    })?,
                    1,
                    1,
                ),
                threadgroup_size,
            );
            encoder.end_encoding();
            Ok(())
        }

        pub(super) fn encode_expert_matvec_f32_ids(
            &mut self,
            pipeline: &ComputePipelineState,
            weights: &PlatformBuffer,
            input: &PlatformBuffer,
            output: &PlatformBuffer,
            rows_per_expert: usize,
            columns: usize,
            row_stride: usize,
            selected_ids: &[i32],
        ) -> Result<(), RuntimeError> {
            let total_rows = selected_ids.len().saturating_mul(rows_per_expert);
            let encoder = self.command_buffer.new_compute_command_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&weights.raw), 0);
            encoder.set_buffer(1, Some(&input.raw), 0);
            encoder.set_buffer(2, Some(&output.raw), 0);

            let rows_per_expert = u32::try_from(rows_per_expert).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "metal expert_matvec_f32_ids rows per expert overflow",
                ))
            })?;
            let columns = u32::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from("metal expert_matvec_f32_ids columns overflow"))
            })?;
            let row_stride = u32::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "metal expert_matvec_f32_ids row stride overflow",
                ))
            })?;
            let selected_count = u32::try_from(selected_ids.len()).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "metal expert_matvec_f32_ids selected count overflow",
                ))
            })?;
            encoder.set_bytes(3, 4, (&rows_per_expert as *const u32).cast());
            encoder.set_bytes(4, 4, (&columns as *const u32).cast());
            encoder.set_bytes(5, 4, (&row_stride as *const u32).cast());
            encoder.set_bytes(6, 4, (&selected_count as *const u32).cast());
            encoder.set_bytes(
                7,
                selected_ids
                    .len()
                    .saturating_mul(std::mem::size_of::<i32>()) as u64,
                selected_ids.as_ptr().cast(),
            );

            let threadgroup_size = quantized_row_threadgroup_size(pipeline)?;
            encoder.dispatch_thread_groups(
                MTLSize::new(
                    u64::try_from(total_rows).map_err(|_| {
                        RuntimeError::Backend(String::from(
                            "metal expert_matvec_f32_ids row count conversion overflow",
                        ))
                    })?,
                    1,
                    1,
                ),
                threadgroup_size,
            );
            encoder.end_encoding();
            Ok(())
        }

        pub(super) fn commit(
            self,
            wait: MetalCommandWait,
        ) -> Result<MetalCommandStatus, RuntimeError> {
            self.command_buffer.commit();
            match wait {
                MetalCommandWait::None => {}
                MetalCommandWait::Scheduled => self.command_buffer.wait_until_scheduled(),
                MetalCommandWait::Completed => self.command_buffer.wait_until_completed(),
            }

            let status = map_command_status(self.command_buffer.status());
            if status == MetalCommandStatus::Error {
                return Err(RuntimeError::Backend(String::from(
                    "metal command buffer reported an error",
                )));
            }
            match wait {
                MetalCommandWait::Completed if status != MetalCommandStatus::Completed => {
                    Err(RuntimeError::Backend(format!(
                        "metal command buffer did not complete cleanly: {status:?}"
                    )))
                }
                MetalCommandWait::Scheduled
                    if !matches!(
                        status,
                        MetalCommandStatus::Scheduled | MetalCommandStatus::Completed
                    ) =>
                {
                    Err(RuntimeError::Backend(format!(
                        "metal command buffer did not schedule cleanly: {status:?}"
                    )))
                }
                _ => Ok(status),
            }
        }
    }

    pub(super) struct ConfiguredBackend {
        descriptor: DeviceDescriptor,
        device: MetalDevice,
        command_queue: CommandQueue,
        storage_mode: MetalStorageMode,
        pipelines: Option<DensePipelines>,
        kernel_cache: MetalKernelCache,
    }

    impl ConfiguredBackend {
        pub(super) fn descriptor(&self) -> &DeviceDescriptor {
            &self.descriptor
        }

        pub(super) fn storage_mode(&self) -> MetalStorageMode {
            self.storage_mode
        }

        pub(super) fn allocate_buffer(
            &self,
            byte_len: usize,
        ) -> Result<PlatformBuffer, RuntimeError> {
            let raw = self.device.new_buffer(
                to_metal_size(byte_len)?,
                resource_options(self.storage_mode),
            );
            Ok(PlatformBuffer { raw })
        }

        pub(super) fn buffer_from_bytes_no_copy(
            &self,
            bytes: &[u8],
            storage_mode: MetalStorageMode,
        ) -> Result<PlatformBuffer, RuntimeError> {
            let raw = self.device.new_buffer_with_bytes_no_copy(
                bytes.as_ptr().cast(),
                to_metal_size(bytes.len())?,
                resource_options(storage_mode),
                None,
            );
            Ok(PlatformBuffer { raw })
        }

        pub(super) fn begin_submission(
            &self,
            label: String,
        ) -> Result<PlatformSubmission, RuntimeError> {
            let command_buffer = self.command_queue.new_command_buffer().to_owned();
            if !label.is_empty() {
                command_buffer.set_label(&label);
            }
            Ok(PlatformSubmission { command_buffer })
        }

        fn pipelines(&mut self) -> Result<&DensePipelines, RuntimeError> {
            if self.pipelines.is_none() {
                self.pipelines = Some(compile_dense_pipelines(&self.device)?);
                self.kernel_cache.record_dense_pipelines();
            }
            let Some(pipelines) = self.pipelines.as_ref() else {
                return Err(RuntimeError::Backend(String::from(
                    "metal dense pipelines were not initialized",
                )));
            };
            Ok(pipelines)
        }

        pub(super) fn kernel_cache_report(&self) -> KernelCacheReport {
            self.kernel_cache.report()
        }

        pub(super) fn configure_kernel_cache_policy(&mut self, policy: KernelCachePolicy) {
            if !policy.enabled {
                self.pipelines = None;
            }
            self.kernel_cache.set_policy(policy);
        }

        pub(super) fn device_memory_budget(
            &self,
            allocator_pool_budget_bytes: u64,
        ) -> DeviceMemoryBudget {
            let kernel_cache_budget_bytes = self
                .kernel_cache
                .policy
                .max_cached_bytes
                .unwrap_or(self.kernel_cache.state.cached_bytes);
            DeviceMemoryBudget::new(
                self.descriptor.memory_capacity_bytes,
                allocator_pool_budget_bytes,
                kernel_cache_budget_bytes,
            )
        }

        pub(super) fn encode_add(
            &mut self,
            submission: &mut PlatformSubmission,
            left: &MetalBuffer,
            right: &MetalBuffer,
            output: &MetalBuffer,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.add;
            submission.encode_add(
                pipeline,
                &left.platform,
                &right.platform,
                &output.platform,
                element_count,
            )
        }

        pub(super) fn encode_matmul(
            &mut self,
            submission: &mut PlatformSubmission,
            left: &MetalBuffer,
            right: &MetalBuffer,
            output: &MetalBuffer,
        ) -> Result<(), RuntimeError> {
            let left_dims = left.spec().shape().dims();
            let right_dims = right.spec().shape().dims();
            if left_dims.len() != 2 || right_dims.len() != 2 || left_dims[1] != right_dims[0] {
                return Err(RuntimeError::Backend(String::from(
                    "metal matmul requires rank-2 tensors with matching inner dimensions",
                )));
            }
            let pipeline = &self.pipelines()?.matmul;
            submission.encode_matmul(
                pipeline,
                &left.platform,
                &right.platform,
                &output.platform,
                left_dims[0],
                left_dims[1],
                right_dims[1],
            )
        }

        pub(super) fn encode_quantized_matvec(
            &mut self,
            submission: &mut PlatformSubmission,
            weights: &MetalBuffer,
            byte_offset: usize,
            mode: QuantizationMode,
            rows: usize,
            columns: usize,
            input: &MetalBuffer,
            output: &MetalBuffer,
        ) -> Result<(), RuntimeError> {
            let row_stride = quantized_row_stride(mode, columns)?;
            let pipelines = self.pipelines()?;
            let pipeline = match mode {
                QuantizationMode::GgmlQ8_0 => &pipelines.quantized_matvec_q8_0,
                QuantizationMode::GgmlMxfp4 => &pipelines.quantized_matvec_mxfp4,
                _ => {
                    return Err(RuntimeError::Backend(format!(
                        "metal quantized matvec does not support mode {mode:?}",
                    )));
                }
            };
            submission.encode_quantized_matvec(
                pipeline,
                &weights.platform,
                byte_offset,
                &input.platform,
                &output.platform,
                rows,
                columns,
                row_stride,
            )
        }

        pub(super) fn encode_grouped_quantized_matvec(
            &mut self,
            submission: &mut PlatformSubmission,
            weights: &MetalBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows_per_expert: usize,
            columns: usize,
            selected_ids: &[i32],
            input: &MetalBuffer,
            output: &MetalBuffer,
        ) -> Result<(), RuntimeError> {
            let expected_row_stride = quantized_row_stride(mode, columns)?;
            if row_stride != expected_row_stride {
                return Err(RuntimeError::Backend(format!(
                    "metal grouped matvec row stride mismatch: expected {expected_row_stride}, actual {row_stride}",
                )));
            }
            let pipelines = self.pipelines()?;
            let pipeline = match mode {
                QuantizationMode::GgmlQ8_0 => &pipelines.grouped_quantized_matvec_q8_0,
                QuantizationMode::GgmlMxfp4 => &pipelines.grouped_quantized_matvec_mxfp4,
                _ => {
                    return Err(RuntimeError::Backend(format!(
                        "metal grouped matvec does not support mode {mode:?}",
                    )));
                }
            };
            submission.encode_grouped_quantized_matvec(
                pipeline,
                &weights.platform,
                &input.platform,
                &output.platform,
                rows_per_expert,
                columns,
                row_stride,
                selected_ids,
            )
        }

        pub(super) fn encode_expert_matvec_f32_ids(
            &mut self,
            submission: &mut PlatformSubmission,
            weights: &MetalBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows_per_expert: usize,
            columns: usize,
            selected_ids: &[i32],
            input: &MetalBuffer,
            output: &MetalBuffer,
        ) -> Result<(), RuntimeError> {
            let expected_row_stride = quantized_row_stride(mode, columns)?;
            if row_stride != expected_row_stride {
                return Err(RuntimeError::Backend(format!(
                    "metal expert_matvec_f32_ids row stride mismatch: expected {expected_row_stride}, actual {row_stride}",
                )));
            }
            let pipelines = self.pipelines()?;
            let pipeline = match mode {
                QuantizationMode::GgmlQ8_0 => &pipelines.expert_matvec_f32_ids_q8_0,
                QuantizationMode::GgmlMxfp4 => &pipelines.expert_matvec_f32_ids_mxfp4,
                _ => {
                    return Err(RuntimeError::Backend(format!(
                        "metal expert_matvec_f32_ids does not support mode {mode:?}",
                    )));
                }
            };
            submission.encode_expert_matvec_f32_ids(
                pipeline,
                &weights.platform,
                &input.platform,
                &output.platform,
                rows_per_expert,
                columns,
                row_stride,
                selected_ids,
            )
        }

        pub(super) fn synchronize_output(
            &self,
            submission: &mut PlatformSubmission,
            output: &MetalBuffer,
        ) -> Result<bool, RuntimeError> {
            submission.synchronize_buffer(&output.platform, output.storage_mode())
        }
    }

    pub(super) fn configure_preferred_backend() -> Result<ConfiguredBackend, RuntimeHealth> {
        let records = collect_device_records().map_err(|error| RuntimeHealth {
            status: HealthStatus::Degraded,
            message: format!("metal backend discovery failed during configuration: {error}"),
        })?;
        let Some(record) = records
            .into_iter()
            .find(|record| record.support_tier == DeviceSupportTier::Modern)
        else {
            return Err(discovery_report()
                .map(|report| report.health)
                .unwrap_or(RuntimeHealth {
                    status: HealthStatus::Offline,
                    message: String::from("metal runtime reported no devices"),
                }));
        };

        let command_queue = record.device.new_command_queue();
        command_queue.set_label(&format!("psionic.metal.queue.{}", record.descriptor.device));
        let storage_mode = if record.descriptor.unified_memory == Some(true) {
            MetalStorageMode::Shared
        } else {
            MetalStorageMode::Managed
        };

        Ok(ConfiguredBackend {
            descriptor: record.descriptor,
            device: record.device,
            command_queue,
            storage_mode,
            pipelines: None,
            kernel_cache: MetalKernelCache::new(),
        })
    }

    pub(super) fn discovery_report() -> Result<MetalDiscoveryReport, RuntimeError> {
        let records = collect_device_records()?;
        let mut devices = Vec::with_capacity(records.len());
        let mut modern_count = 0usize;
        let mut legacy_count = 0usize;

        for record in records {
            match record.support_tier {
                DeviceSupportTier::Modern => modern_count += 1,
                DeviceSupportTier::Legacy => legacy_count += 1,
            }
            devices.push(record.descriptor);
        }

        let health = if modern_count > 0 {
            let message = if legacy_count > 0 {
                format!(
                    "metal discovery ready on {modern_count} modern device(s); {legacy_count} legacy-only device(s) remain degraded"
                )
            } else {
                format!("metal discovery ready on {modern_count} modern device(s)")
            };
            RuntimeHealth {
                status: HealthStatus::Ready,
                message,
            }
        } else if legacy_count > 0 {
            RuntimeHealth {
                status: HealthStatus::Degraded,
                message: format!(
                    "metal discovered {legacy_count} legacy-only device(s); Psionic currently targets Apple-family or Common3-class GPUs first"
                ),
            }
        } else {
            RuntimeHealth {
                status: HealthStatus::Offline,
                message: String::from("metal runtime reported no devices"),
            }
        };

        Ok(MetalDiscoveryReport { devices, health })
    }

    struct DeviceRecord {
        device: MetalDevice,
        descriptor: DeviceDescriptor,
        support_tier: DeviceSupportTier,
    }

    fn collect_device_records() -> Result<Vec<DeviceRecord>, RuntimeError> {
        let mut records = Vec::new();
        for (ordinal, device) in MetalDevice::all().into_iter().enumerate() {
            let family = collect_family_support(&device);
            let tier = classify_support(family);
            let descriptor = build_descriptor(ordinal, &device, tier, family)?;
            records.push(DeviceRecord {
                device,
                descriptor,
                support_tier: tier,
            });
        }
        Ok(records)
    }

    fn build_descriptor(
        ordinal: usize,
        device: &MetalDeviceRef,
        tier: DeviceSupportTier,
        family: FamilySupport,
    ) -> Result<DeviceDescriptor, RuntimeError> {
        let ordinal = u16::try_from(ordinal)
            .map_err(|_| RuntimeError::Backend(String::from("metal device ordinal overflow")))?;
        let mut feature_flags = Vec::new();
        feature_flags.push(match tier {
            DeviceSupportTier::Modern => String::from(MODERN_FAMILY_FLAG),
            DeviceSupportTier::Legacy => String::from(LEGACY_FAMILY_FLAG),
        });
        feature_flags.push(location_flag(device.location()).to_owned());
        feature_flags.push(if device.has_unified_memory() {
            String::from("unified_memory")
        } else {
            String::from("discrete_memory")
        });
        push_flag(&mut feature_flags, device.is_low_power(), "low_power");
        push_flag(&mut feature_flags, device.is_headless(), "headless");
        push_flag(&mut feature_flags, device.is_removable(), "removable");
        push_flag(&mut feature_flags, family.apple, "gpu_family_apple");
        push_flag(&mut feature_flags, family.common2, "gpu_family_common2");
        push_flag(&mut feature_flags, family.common3, "gpu_family_common3");
        push_flag(&mut feature_flags, family.mac1, "gpu_family_mac1");
        push_flag(&mut feature_flags, family.mac2, "gpu_family_mac2");
        push_flag(&mut feature_flags, family.metal3, "gpu_family_metal3");
        push_flag(&mut feature_flags, family.metal4, "gpu_family_metal4");
        push_flag(
            &mut feature_flags,
            matches!(tier, DeviceSupportTier::Modern),
            "submit_ready",
        );
        push_flag(
            &mut feature_flags,
            matches!(tier, DeviceSupportTier::Modern),
            FLASH_ATTENTION_FEATURE_FLAG,
        );
        push_flag(
            &mut feature_flags,
            matches!(tier, DeviceSupportTier::Legacy),
            "submit_degraded",
        );

        let memory_capacity_bytes = match device.recommended_max_working_set_size() {
            0 => None,
            size => Some(size),
        };

        Ok(DeviceDescriptor {
            backend: String::from("metal"),
            device: Device::new(DeviceKind::Metal, ordinal, Some(format!("metal:{ordinal}"))),
            device_name: Some(device.name().to_owned()),
            supported_dtypes: vec![DType::F32],
            supported_quantization: vec![
                QuantizationSupport {
                    mode: QuantizationMode::None,
                    load_path: QuantizationLoadPath::DenseF32,
                    execution: QuantizationExecution::Native,
                },
                QuantizationSupport {
                    mode: QuantizationMode::GgmlQ8_0,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::DequantizeToF32,
                },
                QuantizationSupport {
                    mode: QuantizationMode::GgmlMxfp4,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::DequantizeToF32,
                },
            ],
            memory_capacity_bytes,
            unified_memory: Some(device.has_unified_memory()),
            feature_flags,
            amd_metadata: None,
            nvidia_metadata: None,
        })
    }

    fn push_flag(feature_flags: &mut Vec<String>, enabled: bool, flag: &str) {
        if enabled {
            feature_flags.push(flag.to_owned());
        }
    }

    fn location_flag(location: MTLDeviceLocation) -> &'static str {
        match location {
            MTLDeviceLocation::BuiltIn => "location_built_in",
            MTLDeviceLocation::Slot => "location_slot",
            MTLDeviceLocation::External => "location_external",
            MTLDeviceLocation::Unspecified => "location_unspecified",
        }
    }

    fn collect_family_support(device: &MetalDeviceRef) -> FamilySupport {
        FamilySupport {
            common2: device.supports_family(MTLGPUFamily::Common2),
            common3: device.supports_family(MTLGPUFamily::Common3),
            mac1: device.supports_family(MTLGPUFamily::Mac1),
            mac2: device.supports_family(MTLGPUFamily::Mac2),
            metal3: device.supports_family(MTLGPUFamily::Metal3),
            metal4: device.supports_family(MTLGPUFamily::Metal4),
            apple: supports_any_apple_family(device),
        }
    }

    fn supports_any_apple_family(device: &MetalDeviceRef) -> bool {
        [
            MTLGPUFamily::Apple1,
            MTLGPUFamily::Apple2,
            MTLGPUFamily::Apple3,
            MTLGPUFamily::Apple4,
            MTLGPUFamily::Apple5,
            MTLGPUFamily::Apple6,
            MTLGPUFamily::Apple7,
            MTLGPUFamily::Apple8,
            MTLGPUFamily::Apple9,
        ]
        .into_iter()
        .any(|family| device.supports_family(family))
    }

    fn resource_options(storage_mode: MetalStorageMode) -> MTLResourceOptions {
        match storage_mode {
            MetalStorageMode::Shared => {
                MTLResourceOptions::CPUCacheModeDefaultCache | MTLResourceOptions::StorageModeShared
            }
            MetalStorageMode::Managed => {
                MTLResourceOptions::CPUCacheModeDefaultCache
                    | MTLResourceOptions::StorageModeManaged
            }
            MetalStorageMode::Private => MTLResourceOptions::StorageModePrivate,
        }
    }

    fn compile_dense_pipelines(device: &MetalDeviceRef) -> Result<DensePipelines, RuntimeError> {
        let options = CompileOptions::new();
        options.set_fast_math_enabled(false);
        let library = device
            .new_library_with_source(EMBEDDINGS_METAL_SOURCE, &options)
            .map_err(|error| {
                RuntimeError::Backend(format!("metal shader compile failed: {error}"))
            })?;
        let add = library
            .get_function("psionic_add", None)
            .map_err(|error| RuntimeError::Backend(format!("missing Metal add kernel: {error}")))?;
        let matmul = library
            .get_function("psionic_matmul", None)
            .map_err(|error| {
                RuntimeError::Backend(format!("missing Metal matmul kernel: {error}"))
            })?;
        let quantized_matvec_q8_0 = library
            .get_function("psionic_quantized_matvec_q8_0", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal q8_0 quantized matvec kernel: {error}"
                ))
            })?;
        let quantized_matvec_mxfp4 = library
            .get_function("psionic_quantized_matvec_mxfp4", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal mxfp4 quantized matvec kernel: {error}"
                ))
            })?;
        let grouped_quantized_matvec_q8_0 = library
            .get_function("psionic_mul_mv_id_q8_0", None)
            .map_err(|error| {
                RuntimeError::Backend(format!("missing Metal q8_0 grouped matvec kernel: {error}"))
            })?;
        let grouped_quantized_matvec_mxfp4 = library
            .get_function("psionic_mul_mv_id_mxfp4", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal mxfp4 grouped matvec kernel: {error}"
                ))
            })?;
        let expert_matvec_f32_ids_q8_0 = library
            .get_function("psionic_expert_matvec_f32_ids_q8_0", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal q8_0 expert_matvec_f32_ids kernel: {error}"
                ))
            })?;
        let expert_matvec_f32_ids_mxfp4 = library
            .get_function("psionic_expert_matvec_f32_ids_mxfp4", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal mxfp4 expert_matvec_f32_ids kernel: {error}"
                ))
            })?;

        Ok(DensePipelines {
            add: device
                .new_compute_pipeline_state_with_function(&add)
                .map_err(|error| {
                    RuntimeError::Backend(format!("metal add pipeline build failed: {error}"))
                })?,
            matmul: device
                .new_compute_pipeline_state_with_function(&matmul)
                .map_err(|error| {
                    RuntimeError::Backend(format!("metal matmul pipeline build failed: {error}"))
                })?,
            quantized_matvec_q8_0: device
                .new_compute_pipeline_state_with_function(&quantized_matvec_q8_0)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q8_0 quantized matvec pipeline build failed: {error}"
                    ))
                })?,
            quantized_matvec_mxfp4: device
                .new_compute_pipeline_state_with_function(&quantized_matvec_mxfp4)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal mxfp4 quantized matvec pipeline build failed: {error}"
                    ))
                })?,
            grouped_quantized_matvec_q8_0: device
                .new_compute_pipeline_state_with_function(&grouped_quantized_matvec_q8_0)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q8_0 grouped matvec pipeline build failed: {error}"
                    ))
                })?,
            grouped_quantized_matvec_mxfp4: device
                .new_compute_pipeline_state_with_function(&grouped_quantized_matvec_mxfp4)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal mxfp4 grouped matvec pipeline build failed: {error}"
                    ))
                })?,
            expert_matvec_f32_ids_q8_0: device
                .new_compute_pipeline_state_with_function(&expert_matvec_f32_ids_q8_0)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q8_0 expert_matvec_f32_ids pipeline build failed: {error}"
                    ))
                })?,
            expert_matvec_f32_ids_mxfp4: device
                .new_compute_pipeline_state_with_function(&expert_matvec_f32_ids_mxfp4)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal mxfp4 expert_matvec_f32_ids pipeline build failed: {error}"
                    ))
                })?,
        })
    }

    fn compute_threadgroup_size(
        pipeline: &ComputePipelineState,
        grid_width: usize,
    ) -> Result<MTLSize, RuntimeError> {
        let width = pipeline.thread_execution_width();
        let max_threads = pipeline.max_total_threads_per_threadgroup();
        let grid_width = to_metal_size(grid_width)?;
        let width = width.min(max_threads).min(grid_width.max(1));
        Ok(MTLSize::new(width.max(1), 1, 1))
    }

    fn quantized_row_threadgroup_size(
        pipeline: &ComputePipelineState,
    ) -> Result<MTLSize, RuntimeError> {
        let width = pipeline.thread_execution_width().min(32);
        if width == 0 {
            return Err(RuntimeError::Backend(String::from(
                "metal quantized kernel reported zero thread execution width",
            )));
        }
        Ok(MTLSize::new(width, 1, 1))
    }

    fn map_command_status(status: MTLCommandBufferStatus) -> MetalCommandStatus {
        match status {
            MTLCommandBufferStatus::NotEnqueued => MetalCommandStatus::NotEnqueued,
            MTLCommandBufferStatus::Enqueued => MetalCommandStatus::Enqueued,
            MTLCommandBufferStatus::Committed => MetalCommandStatus::Committed,
            MTLCommandBufferStatus::Scheduled => MetalCommandStatus::Scheduled,
            MTLCommandBufferStatus::Completed => MetalCommandStatus::Completed,
            MTLCommandBufferStatus::Error => MetalCommandStatus::Error,
        }
    }

    fn to_metal_size(size: usize) -> Result<u64, RuntimeError> {
        u64::try_from(size)
            .map_err(|_| RuntimeError::Backend(String::from("metal size conversion overflow")))
    }

    fn byte_range(byte_len: usize) -> Result<NSRange, RuntimeError> {
        Ok(NSRange::new(0, to_metal_size(byte_len)?))
    }

    const EMBEDDINGS_METAL_SOURCE: &str = r"
#include <metal_stdlib>
using namespace metal;

constant uint PSIONIC_QUANTIZED_ROW_THREADS = 32;

kernel void psionic_add(
    const device float* left [[buffer(0)]],
    const device float* right [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& element_count [[buffer(3)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= element_count) {
        return;
    }
    output[gid] = left[gid] + right[gid];
}

kernel void psionic_matmul(
    const device float* left [[buffer(0)]],
    const device float* right [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& m [[buffer(3)]],
    constant uint& k [[buffer(4)]],
    constant uint& n [[buffer(5)]],
    uint gid [[thread_position_in_grid]]
) {
    uint row = gid / n;
    uint col = gid % n;
    if (row >= m || col >= n) {
        return;
    }

    float sum = 0.0f;
    for (uint inner = 0; inner < k; inner++) {
        sum += left[(row * k) + inner] * right[(inner * n) + col];
    }
    output[(row * n) + col] = sum;
}

constant short PSIONIC_MXFP4_VALUES[16] = {
    0, 1, 2, 3, 4, 6, 8, 12,
    0, -1, -2, -3, -4, -6, -8, -12
};

inline float psionic_mxfp4_scale(uchar exponent_bits) {
    uint bits = exponent_bits == 0 ? 0x00400000u : (uint(exponent_bits) << 23);
    return as_type<float>(bits) * 0.5f;
}

inline float psionic_q8_0_block_dot(
    const device uchar* block,
    const device float* input
) {
    ushort scale_bits = ushort(block[0]) | (ushort(block[1]) << 8);
    float scale = float(as_type<half>(scale_bits));
    float sum = 0.0f;
    for (uint index = 0; index < 32; index++) {
        sum += input[index] * float(as_type<char>(block[2 + index]));
    }
    return sum * scale;
}

inline float psionic_mxfp4_block_dot(
    const device uchar* block,
    const device float* input
) {
    float scale = psionic_mxfp4_scale(block[0]);
    float sum = 0.0f;
    for (uint pair_index = 0; pair_index < 16; pair_index++) {
        uchar packed = block[1 + pair_index];
        sum += input[pair_index] * float(PSIONIC_MXFP4_VALUES[packed & 0x0f]) * scale;
        sum += input[pair_index + 16] * float(PSIONIC_MXFP4_VALUES[(packed >> 4) & 0x0f]) * scale;
    }
    return sum;
}

kernel void psionic_quantized_matvec_q8_0(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& rows [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant ulong& byte_offset [[buffer(6)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]
) {
    uint row = tgpig.x;
    if (row >= rows) {
        return;
    }
    threadgroup float partial[PSIONIC_QUANTIZED_ROW_THREADS];
    ulong row_base = byte_offset + ulong(row) * ulong(row_stride);
    uint block_count = columns / 32;
    float sum = 0.0f;
    for (uint block_index = tid; block_index < block_count; block_index += PSIONIC_QUANTIZED_ROW_THREADS) {
        const device uchar* block = weights + row_base + ulong(block_index) * 34ul;
        sum += psionic_q8_0_block_dot(block, input + block_index * 32);
    }
    partial[tid] = sum;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint stride = PSIONIC_QUANTIZED_ROW_THREADS / 2; stride > 0; stride >>= 1) {
        if (tid < stride) {
            partial[tid] += partial[tid + stride];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (tid == 0) {
        output[row] = partial[0];
    }
}

kernel void psionic_quantized_matvec_mxfp4(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& rows [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant ulong& byte_offset [[buffer(6)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]
) {
    uint row = tgpig.x;
    if (row >= rows) {
        return;
    }
    threadgroup float partial[PSIONIC_QUANTIZED_ROW_THREADS];
    ulong row_base = byte_offset + ulong(row) * ulong(row_stride);
    uint block_count = columns / 32;
    float sum = 0.0f;
    for (uint block_index = tid; block_index < block_count; block_index += PSIONIC_QUANTIZED_ROW_THREADS) {
        const device uchar* block = weights + row_base + ulong(block_index) * 17ul;
        sum += psionic_mxfp4_block_dot(block, input + block_index * 32);
    }
    partial[tid] = sum;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint stride = PSIONIC_QUANTIZED_ROW_THREADS / 2; stride > 0; stride >>= 1) {
        if (tid < stride) {
            partial[tid] += partial[tid + stride];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (tid == 0) {
        output[row] = partial[0];
    }
}

kernel void psionic_mul_mv_id_q8_0(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& rows_per_expert [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant uint& selected_count [[buffer(6)]],
    constant int* selected_ids [[buffer(7)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]
) {
    uint total_rows = rows_per_expert * selected_count;
    uint row = tgpig.x;
    if (row >= total_rows) {
        return;
    }
    threadgroup float partial[PSIONIC_QUANTIZED_ROW_THREADS];
    uint selected_index = row / rows_per_expert;
    uint row_in_expert = row % rows_per_expert;
    int expert_id = selected_ids[selected_index];
    if (expert_id < 0) {
        if (tid == 0) {
            output[row] = 0.0f;
        }
        return;
    }
    ulong row_base = (ulong(expert_id) * ulong(rows_per_expert) + ulong(row_in_expert)) * ulong(row_stride);
    uint block_count = columns / 32;
    float sum = 0.0f;
    for (uint block_index = tid; block_index < block_count; block_index += PSIONIC_QUANTIZED_ROW_THREADS) {
        const device uchar* block = weights + row_base + ulong(block_index) * 34ul;
        sum += psionic_q8_0_block_dot(block, input + block_index * 32);
    }
    partial[tid] = sum;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint stride = PSIONIC_QUANTIZED_ROW_THREADS / 2; stride > 0; stride >>= 1) {
        if (tid < stride) {
            partial[tid] += partial[tid + stride];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (tid == 0) {
        output[row] = partial[0];
    }
}

kernel void psionic_mul_mv_id_mxfp4(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& rows_per_expert [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant uint& selected_count [[buffer(6)]],
    constant int* selected_ids [[buffer(7)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]
) {
    uint total_rows = rows_per_expert * selected_count;
    uint row = tgpig.x;
    if (row >= total_rows) {
        return;
    }
    threadgroup float partial[PSIONIC_QUANTIZED_ROW_THREADS];
    uint selected_index = row / rows_per_expert;
    uint row_in_expert = row % rows_per_expert;
    int expert_id = selected_ids[selected_index];
    if (expert_id < 0) {
        if (tid == 0) {
            output[row] = 0.0f;
        }
        return;
    }
    ulong row_base = (ulong(expert_id) * ulong(rows_per_expert) + ulong(row_in_expert)) * ulong(row_stride);
    uint block_count = columns / 32;
    float sum = 0.0f;
    for (uint block_index = tid; block_index < block_count; block_index += PSIONIC_QUANTIZED_ROW_THREADS) {
        const device uchar* block = weights + row_base + ulong(block_index) * 17ul;
        sum += psionic_mxfp4_block_dot(block, input + block_index * 32);
    }
    partial[tid] = sum;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint stride = PSIONIC_QUANTIZED_ROW_THREADS / 2; stride > 0; stride >>= 1) {
        if (tid < stride) {
            partial[tid] += partial[tid + stride];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (tid == 0) {
        output[row] = partial[0];
    }
}

kernel void psionic_expert_matvec_f32_ids_q8_0(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& rows_per_expert [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant uint& selected_count [[buffer(6)]],
    constant int* selected_ids [[buffer(7)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]
) {
    uint total_rows = rows_per_expert * selected_count;
    uint row = tgpig.x;
    if (row >= total_rows) {
        return;
    }
    threadgroup float partial[PSIONIC_QUANTIZED_ROW_THREADS];
    uint selected_index = row / rows_per_expert;
    uint row_in_expert = row % rows_per_expert;
    int expert_id = selected_ids[selected_index];
    if (expert_id < 0) {
        if (tid == 0) {
            output[row] = 0.0f;
        }
        return;
    }
    ulong row_base = (ulong(expert_id) * ulong(rows_per_expert) + ulong(row_in_expert)) * ulong(row_stride);
    uint block_count = columns / 32;
    const device float* input_row = input + ulong(selected_index) * ulong(columns);
    float sum = 0.0f;
    for (uint block_index = tid; block_index < block_count; block_index += PSIONIC_QUANTIZED_ROW_THREADS) {
        const device uchar* block = weights + row_base + ulong(block_index) * 34ul;
        sum += psionic_q8_0_block_dot(block, input_row + block_index * 32);
    }
    partial[tid] = sum;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint stride = PSIONIC_QUANTIZED_ROW_THREADS / 2; stride > 0; stride >>= 1) {
        if (tid < stride) {
            partial[tid] += partial[tid + stride];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (tid == 0) {
        output[row] = partial[0];
    }
}

kernel void psionic_expert_matvec_f32_ids_mxfp4(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& rows_per_expert [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant uint& selected_count [[buffer(6)]],
    constant int* selected_ids [[buffer(7)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]
) {
    uint total_rows = rows_per_expert * selected_count;
    uint row = tgpig.x;
    if (row >= total_rows) {
        return;
    }
    threadgroup float partial[PSIONIC_QUANTIZED_ROW_THREADS];
    uint selected_index = row / rows_per_expert;
    uint row_in_expert = row % rows_per_expert;
    int expert_id = selected_ids[selected_index];
    if (expert_id < 0) {
        if (tid == 0) {
            output[row] = 0.0f;
        }
        return;
    }
    ulong row_base = (ulong(expert_id) * ulong(rows_per_expert) + ulong(row_in_expert)) * ulong(row_stride);
    uint block_count = columns / 32;
    const device float* input_row = input + ulong(selected_index) * ulong(columns);
    float sum = 0.0f;
    for (uint block_index = tid; block_index < block_count; block_index += PSIONIC_QUANTIZED_ROW_THREADS) {
        const device uchar* block = weights + row_base + ulong(block_index) * 17ul;
        sum += psionic_mxfp4_block_dot(block, input_row + block_index * 32);
    }
    partial[tid] = sum;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint stride = PSIONIC_QUANTIZED_ROW_THREADS / 2; stride > 0; stride >>= 1) {
        if (tid < stride) {
            partial[tid] += partial[tid + stride];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (tid == 0) {
        output[row] = partial[0];
    }
}
";
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use psionic_runtime::{
        DeviceMemoryBudget, HealthStatus, KernelCachePolicy, KernelCacheReport, KernelCacheState,
        RuntimeHealth,
    };

    use super::{
        MetalBuffer, MetalCommandStatus, MetalCommandWait, MetalDiscoveryReport, MetalStorageMode,
        RuntimeError,
    };
    use psionic_core::QuantizationMode;

    #[derive(Clone)]
    pub(super) struct PlatformBuffer;

    impl PlatformBuffer {
        pub(super) fn write_bytes(
            &self,
            _bytes: &[u8],
            _storage_mode: MetalStorageMode,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn write_bytes_at_offset(
            &self,
            _byte_offset: usize,
            _bytes: &[u8],
            _storage_mode: MetalStorageMode,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn read_bytes(&self, _byte_len: usize) -> Result<Vec<u8>, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn read_bytes_at_offset(
            &self,
            _byte_offset: usize,
            _byte_len: usize,
        ) -> Result<Vec<u8>, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn with_bytes_at_offset<T>(
            &self,
            _byte_offset: usize,
            _byte_len: usize,
            _map: impl FnOnce(&[u8]) -> Result<T, RuntimeError>,
        ) -> Result<T, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }
    }

    pub(super) struct PlatformSubmission;

    impl PlatformSubmission {
        pub(super) fn fill_buffer(
            &mut self,
            _buffer: &PlatformBuffer,
            _byte_len: usize,
            _value: u8,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn copy_buffer(
            &mut self,
            _source: &PlatformBuffer,
            _destination: &PlatformBuffer,
            _byte_len: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn synchronize_buffer(
            &mut self,
            _buffer: &PlatformBuffer,
            _storage_mode: MetalStorageMode,
        ) -> Result<bool, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_quantized_matvec(
            &mut self,
            _pipeline: &(),
            _weights: &PlatformBuffer,
            _byte_offset: usize,
            _input: &PlatformBuffer,
            _output: &PlatformBuffer,
            _rows: usize,
            _columns: usize,
            _row_stride: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_grouped_quantized_matvec(
            &mut self,
            _pipeline: &(),
            _weights: &PlatformBuffer,
            _input: &PlatformBuffer,
            _output: &PlatformBuffer,
            _rows_per_expert: usize,
            _columns: usize,
            _row_stride: usize,
            _selected_ids: &[i32],
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_expert_matvec_f32_ids(
            &mut self,
            _pipeline: &(),
            _weights: &PlatformBuffer,
            _input: &PlatformBuffer,
            _output: &PlatformBuffer,
            _rows_per_expert: usize,
            _columns: usize,
            _row_stride: usize,
            _selected_ids: &[i32],
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn commit(
            self,
            _wait: MetalCommandWait,
        ) -> Result<MetalCommandStatus, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }
    }

    pub(super) struct ConfiguredBackend {
        descriptor: psionic_runtime::DeviceDescriptor,
    }

    impl ConfiguredBackend {
        pub(super) fn descriptor(&self) -> &psionic_runtime::DeviceDescriptor {
            &self.descriptor
        }

        pub(super) const fn storage_mode(&self) -> MetalStorageMode {
            MetalStorageMode::Shared
        }

        pub(super) fn allocate_buffer(
            &self,
            _byte_len: usize,
        ) -> Result<PlatformBuffer, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn buffer_from_bytes_no_copy(
            &self,
            _bytes: &[u8],
            _storage_mode: MetalStorageMode,
        ) -> Result<PlatformBuffer, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn begin_submission(
            &self,
            _label: String,
        ) -> Result<PlatformSubmission, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_add(
            &mut self,
            _submission: &mut PlatformSubmission,
            _left: &MetalBuffer,
            _right: &MetalBuffer,
            _output: &MetalBuffer,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_matmul(
            &mut self,
            _submission: &mut PlatformSubmission,
            _left: &MetalBuffer,
            _right: &MetalBuffer,
            _output: &MetalBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_quantized_matvec(
            &mut self,
            _submission: &mut PlatformSubmission,
            _weights: &MetalBuffer,
            _byte_offset: usize,
            _mode: QuantizationMode,
            _rows: usize,
            _columns: usize,
            _input: &MetalBuffer,
            _output: &MetalBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_grouped_quantized_matvec(
            &mut self,
            _submission: &mut PlatformSubmission,
            _weights: &MetalBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows_per_expert: usize,
            _columns: usize,
            _selected_ids: &[i32],
            _input: &MetalBuffer,
            _output: &MetalBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_expert_matvec_f32_ids(
            &mut self,
            _submission: &mut PlatformSubmission,
            _weights: &MetalBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows_per_expert: usize,
            _columns: usize,
            _selected_ids: &[i32],
            _input: &MetalBuffer,
            _output: &MetalBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn synchronize_output(
            &self,
            _submission: &mut PlatformSubmission,
            _output: &MetalBuffer,
        ) -> Result<bool, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn kernel_cache_report(&self) -> KernelCacheReport {
            KernelCacheReport {
                policy: KernelCachePolicy::bounded(0, Some(0)),
                state: KernelCacheState::default(),
            }
        }

        pub(super) fn configure_kernel_cache_policy(&mut self, _policy: KernelCachePolicy) {}

        pub(super) fn device_memory_budget(
            &self,
            allocator_pool_budget_bytes: u64,
        ) -> DeviceMemoryBudget {
            DeviceMemoryBudget::new(
                self.descriptor.memory_capacity_bytes,
                allocator_pool_budget_bytes,
                0,
            )
        }
    }

    pub(super) fn configure_preferred_backend() -> Result<ConfiguredBackend, RuntimeHealth> {
        Err(RuntimeHealth {
            status: HealthStatus::Offline,
            message: String::from("metal backend is only available on macOS"),
        })
    }

    pub(super) fn discovery_report() -> Result<MetalDiscoveryReport, psionic_runtime::RuntimeError>
    {
        Ok(MetalDiscoveryReport {
            devices: Vec::new(),
            health: RuntimeHealth {
                status: HealthStatus::Offline,
                message: String::from("metal backend is only available on macOS"),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use psionic_backend_cpu::CpuBackend;
    use psionic_compiler::compile_graph;
    use psionic_core::{
        BackendExtensionKind, DType, Device, DeviceKind, QuantizationMode, QuantizedTensorData,
        Shape, TensorSpec,
    };
    use psionic_ir::GraphBuilder;
    use psionic_runtime::{
        Allocator, BackendDegradedPolicy, BackendParityPolicy, BackendSelectionState, BufferHandle,
        BufferResidency, BufferStorageKind, CacheAction, CacheKind, CompilePathTemperature,
        DeviceDiscovery, HealthStatus, KvCacheAccounting, KvCachePageLayout, KvCacheState,
        PrefixCacheState, QuantizationExecution, QuantizationLoadPath, QuantizationSupport,
        RuntimeError, ServedProductBackendPolicy,
    };

    use super::{
        DeviceSupportTier, EMBEDDINGS_SUPPORTED_OPS, FamilySupport, MetalAttentionGraphReserve,
        MetalBackend, MetalGraphReserveKind, MetalPromptResidencyMetrics,
        MetalSharedPrefixCompatibility, MetalSharedPrefixStore, TEXT_GENERATION_SUPPORTED_OPS,
        classify_support, validate_quantized_storage, validate_supported_plan,
    };

    fn sample_repeated_mxfp4_rows(rows: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(rows * 17);
        for _ in 0..rows {
            bytes.push(128_u8);
            bytes.extend([0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]);
            bytes.extend([0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]);
        }
        bytes
    }

    fn sample_repeated_q8_0_rows(rows: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(rows * 34);
        for _ in 0..rows {
            bytes.extend([0x00, 0x3c]);
            bytes.extend([0_u8; 32]);
        }
        bytes
    }

    fn sample_q8_0_row(scale: f32, multiplier: i8) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(34);
        bytes.extend_from_slice(&f32_to_f16_bits(scale).to_le_bytes());
        for index in 0_i8..32_i8 {
            bytes.push(index.saturating_mul(multiplier).to_le_bytes()[0]);
        }
        bytes
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

    fn sample_reference_vector() -> Vec<f32> {
        (0..32).map(|index| (index as f32 + 1.0) * 0.25).collect()
    }

    fn expected_grouped_expert_outputs(
        mode: QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        selected_ids: &[i32],
        input: &[f32],
        bytes: &[u8],
    ) -> Result<Vec<f32>, RuntimeError> {
        let mut output = Vec::with_capacity(selected_ids.len().saturating_mul(rows_per_expert));
        for &selected_id in selected_ids {
            let expert_index = usize::try_from(selected_id).map_err(|_| {
                RuntimeError::Backend(format!("negative selected expert id {selected_id}"))
            })?;
            for row in 0..rows_per_expert {
                let row_index = expert_index
                    .saturating_mul(rows_per_expert)
                    .saturating_add(row);
                let start = row_index.saturating_mul(row_stride);
                let end = start.saturating_add(row_stride);
                let mut decoded = Vec::with_capacity(input.len());
                psionic_backend_cpu::decode_quantized_row_into(
                    mode,
                    &bytes[start..end],
                    &mut decoded,
                )?;
                output.push(
                    input
                        .iter()
                        .zip(decoded.iter())
                        .map(|(left, right)| left * right)
                        .sum(),
                );
            }
        }
        Ok(output)
    }

    fn expected_grouped_expert_row_outputs(
        mode: QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        selected_ids: &[i32],
        inputs: &[f32],
        columns: usize,
        bytes: &[u8],
    ) -> Result<Vec<f32>, RuntimeError> {
        if inputs.len() != selected_ids.len().saturating_mul(columns) {
            return Err(RuntimeError::Backend(format!(
                "expected grouped expert row inputs length mismatch: expected {}, actual {}",
                selected_ids.len().saturating_mul(columns),
                inputs.len()
            )));
        }
        let mut output = Vec::with_capacity(selected_ids.len().saturating_mul(rows_per_expert));
        for (&selected_id, input_row) in selected_ids.iter().zip(inputs.chunks_exact(columns)) {
            let expert_index = usize::try_from(selected_id).map_err(|_| {
                RuntimeError::Backend(format!("negative selected expert id {selected_id}"))
            })?;
            for row in 0..rows_per_expert {
                let row_index = expert_index
                    .saturating_mul(rows_per_expert)
                    .saturating_add(row);
                let start = row_index.saturating_mul(row_stride);
                let end = start.saturating_add(row_stride);
                let mut decoded = Vec::with_capacity(input_row.len());
                psionic_backend_cpu::decode_quantized_row_into(
                    mode,
                    &bytes[start..end],
                    &mut decoded,
                )?;
                output.push(
                    input_row
                        .iter()
                        .zip(decoded.iter())
                        .map(|(left, right)| left * right)
                        .sum(),
                );
            }
        }
        Ok(output)
    }

    fn f32_to_f16_bits(value: f32) -> u16 {
        let bits = value.to_bits();
        let sign = ((bits >> 16) & 0x8000) as u16;
        let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
        let mantissa = bits & 0x7f_ffff;
        if exponent <= 0 {
            return sign;
        }
        if exponent >= 0x1f {
            return sign | 0x7c00;
        }
        sign | ((exponent as u16) << 10) | ((mantissa >> 13) as u16)
    }

    fn sample_prefix_compatibility(
        width: usize,
        max_context_tokens: usize,
    ) -> MetalSharedPrefixCompatibility {
        MetalSharedPrefixCompatibility {
            served_artifact_digest: String::from("metal-artifact"),
            model_id: String::from("gpt-oss"),
            model_revision: String::from("20b"),
            weight_bundle_digest: String::from("weights-digest"),
            tokenizer_family: String::from("cl100k"),
            backend_compatibility: String::from("metal-apple"),
            kv_width: width,
            page_layout: KvCachePageLayout::new(
                max_context_tokens,
                4,
                width
                    .saturating_mul(std::mem::size_of::<f32>())
                    .saturating_mul(2),
            ),
        }
    }

    fn assert_close(actual: &[f32], expected: &[f32], tolerance: f32) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected.iter()) {
            assert!(
                (actual - expected).abs() <= tolerance,
                "expected {expected}, actual {actual}, tolerance {tolerance}",
            );
        }
    }

    #[test]
    fn apple_family_devices_classify_as_modern() {
        let family = FamilySupport {
            apple: true,
            ..FamilySupport::default()
        };
        assert_eq!(classify_support(family), DeviceSupportTier::Modern);
    }

    #[test]
    fn common_three_devices_classify_as_modern() {
        let family = FamilySupport {
            common3: true,
            ..FamilySupport::default()
        };
        assert_eq!(classify_support(family), DeviceSupportTier::Modern);
    }

    #[test]
    fn legacy_devices_without_modern_families_degrade() {
        let family = FamilySupport {
            common2: true,
            mac1: true,
            ..FamilySupport::default()
        };
        assert_eq!(classify_support(family), DeviceSupportTier::Legacy);
    }

    #[test]
    fn metal_dense_surfaces_and_parity_policy_are_documented() {
        assert_eq!(
            EMBEDDINGS_SUPPORTED_OPS,
            &["input", "constant", "matmul", "add"]
        );
        assert_eq!(
            TEXT_GENERATION_SUPPORTED_OPS,
            &[
                "input",
                "constant",
                "matmul",
                "add",
                "backend_extension:rms_norm",
                "backend_extension:rotary_embedding",
                "backend_extension:scaled_dot_product_attention",
                "argmax_f32",
                "top_k_f32",
                "mul_mv_id_q8_0",
                "mul_mv_id_mxfp4",
                "expert_matvec_f32_ids_q8_0",
                "expert_matvec_f32_ids_mxfp4",
            ]
        );
        let budget = BackendParityPolicy::default().embedding_budget(QuantizationMode::None);
        assert_eq!(budget.numeric.max_abs_delta, 1.0e-5);
        assert_eq!(budget.numeric.max_rel_delta, 1.0e-5);
    }

    #[test]
    fn metal_plan_validation_rejects_unsupported_ops() -> Result<(), Box<dyn std::error::Error>> {
        let device = Device::new(DeviceKind::Metal, 0, Some(String::from("metal:0")));
        let mut builder = GraphBuilder::new(device);
        let input = builder.input("features", Shape::new(vec![1, 2]), DType::F32);
        let weights = builder.constant_f32(Shape::new(vec![1, 2]), vec![1.0, 0.0])?;
        let unsupported = builder.mul(&input, &weights)?;
        let graph = builder.finish(vec![unsupported]);
        let plan = compile_graph(&graph)?;
        let error = validate_supported_plan(&plan).expect_err("mul should be rejected");
        assert_eq!(
            error,
            psionic_runtime::RuntimeError::UnsupportedStep(String::from("mul"))
        );
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn metal_backend_reports_offline_on_unsupported_platform()
    -> Result<(), psionic_runtime::RuntimeError> {
        let backend = MetalBackend::new();
        let report = backend.discovery_report()?;
        assert!(report.devices.is_empty());
        assert_eq!(report.health.status, HealthStatus::Offline);
        assert_eq!(
            report.health.message,
            String::from("metal backend is only available on macOS")
        );
        assert!(backend.selected_device().is_none());
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn metal_backend_fallback_selection_reports_explicit_cpu_fallback()
    -> Result<(), psionic_runtime::RuntimeError> {
        let backend = MetalBackend::new();
        let cpu = CpuBackend::new();
        let selection = backend.fallback_selection(&cpu, EMBEDDINGS_SUPPORTED_OPS)?;
        assert_eq!(selection.requested_backend, "metal");
        assert_eq!(selection.effective_backend, "cpu");
        assert_eq!(
            selection.fallback_reason.as_deref(),
            Some("metal backend unavailable: metal backend is only available on macOS")
        );
        assert_eq!(
            selection.supported_ops,
            EMBEDDINGS_SUPPORTED_OPS
                .iter()
                .map(|label| String::from(*label))
                .collect::<Vec<_>>()
        );
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
        assert!(selection.runtime_resources.is_some());
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn metal_text_generation_fallback_selection_reports_explicit_cpu_fallback()
    -> Result<(), psionic_runtime::RuntimeError> {
        let backend = MetalBackend::new();
        let cpu = CpuBackend::new();
        let selection = backend.fallback_selection(&cpu, TEXT_GENERATION_SUPPORTED_OPS)?;
        assert_eq!(selection.requested_backend, "metal");
        assert_eq!(selection.effective_backend, "cpu");
        assert_eq!(
            selection.fallback_reason.as_deref(),
            Some("metal backend unavailable: metal backend is only available on macOS")
        );
        assert_eq!(
            selection.supported_ops,
            TEXT_GENERATION_SUPPORTED_OPS
                .iter()
                .map(|label| String::from(*label))
                .collect::<Vec<_>>()
        );
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
        assert!(selection.runtime_resources.is_some());
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn metal_backend_rejects_allocation_and_submission_when_unavailable() {
        let mut backend = MetalBackend::new();
        let spec = TensorSpec::new(
            Shape::new(vec![1, 4]),
            DType::F32,
            Device::new(DeviceKind::Metal, 0, Some(String::from("metal:0"))),
        );
        let allocation = backend.allocate(&spec);
        assert!(allocation.is_err());

        let submission = backend.begin_submission("noop");
        assert!(submission.is_err());
    }

    #[test]
    fn metal_quantized_storage_validation_rejects_mismatched_bytes() {
        let spec = TensorSpec::new(
            Shape::new(vec![1, 32]),
            DType::F32,
            Device::new(DeviceKind::Metal, 0, Some(String::from("metal:0"))),
        );
        let data = QuantizedTensorData::new(
            QuantizationMode::GgmlQ8_0,
            QuantizationMode::GgmlQ8_0
                .ggml_block_layout(spec.shape())
                .expect("q8_0 layout"),
            vec![0_u8; 33],
        );

        let error = validate_quantized_storage(&spec, &data).expect_err("mismatch should fail");
        assert_eq!(
            error,
            RuntimeError::Backend(String::from(
                "quantized byte length mismatch: expected 34, actual 33",
            ))
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_health_matches_discovery() -> Result<(), psionic_runtime::RuntimeError> {
        use super::{LEGACY_FAMILY_FLAG, MODERN_FAMILY_FLAG};

        let backend = MetalBackend::new();
        let report = backend.discovery_report()?;
        let health = backend.health();
        assert_eq!(report.health, health);
        match health.status {
            HealthStatus::Ready => assert!(report.devices.iter().any(|descriptor| {
                descriptor
                    .feature_flags
                    .contains(&String::from(MODERN_FAMILY_FLAG))
            })),
            HealthStatus::Degraded => assert!(report.devices.iter().all(|descriptor| {
                descriptor
                    .feature_flags
                    .contains(&String::from(LEGACY_FAMILY_FLAG))
            })),
            HealthStatus::Offline => assert!(report.devices.is_empty()),
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_selection_reports_ready_metal_or_explicit_cpu_fallback()
    -> Result<(), psionic_runtime::RuntimeError> {
        let backend = MetalBackend::new();
        let cpu = CpuBackend::new();
        match backend.backend_selection(EMBEDDINGS_SUPPORTED_OPS) {
            Ok(selection) => {
                assert_eq!(selection.requested_backend, "metal");
                assert_eq!(selection.effective_backend, "metal");
                assert!(selection.selected_device.is_some());
                assert!(selection.fallback_reason.is_none());
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
                        assert_ne!(backend.health().status, HealthStatus::Offline);
                        return Ok(());
                    }
                }
            }
            Err(error) => {
                assert!(error.to_string().starts_with("metal backend unavailable: "));
                let fallback = backend.fallback_selection(&cpu, EMBEDDINGS_SUPPORTED_OPS)?;
                assert_eq!(fallback.requested_backend, "metal");
                assert_eq!(fallback.effective_backend, "cpu");
                assert!(fallback.selected_device.is_some());
                assert!(fallback.fallback_reason.is_some());
                assert!(fallback.runtime_resources.is_some());
                assert_eq!(
                    fallback.policy,
                    ServedProductBackendPolicy::fallback_to_compatible_backend(
                        BackendDegradedPolicy::AllowSameBackend
                    )
                );
                assert_eq!(
                    fallback.selection_state,
                    BackendSelectionState::CrossBackendFallback
                );
                assert!(fallback.degraded_reason.is_none());
            }
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_allocates_and_submits_copy_on_supported_hardware()
    -> Result<(), psionic_runtime::RuntimeError> {
        use super::{MetalCommandStatus, MetalCommandWait};

        let mut backend = MetalBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let spec = TensorSpec::new(Shape::new(vec![1, 4]), DType::F32, selected.device.clone());
        let mut source = backend.allocate(&spec)?;
        source.write_f32(&[1.0, 2.0, 3.0, 4.0])?;
        let destination = backend.allocate(&spec)?;

        let mut submission = backend.begin_submission("buffer_copy")?;
        submission.copy_buffer(&source, &destination)?;
        submission.synchronize_buffer(&destination)?;
        let report = submission.commit(MetalCommandWait::Completed)?;
        assert_eq!(report.status, MetalCommandStatus::Completed);
        assert_eq!(report.encoded_operations, 1);
        assert_eq!(destination.read_f32()?, vec![1.0, 2.0, 3.0, 4.0]);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_selection_supports_text_generation_surface()
    -> Result<(), psionic_runtime::RuntimeError> {
        let backend = MetalBackend::new();
        let cpu = CpuBackend::new();
        match backend.backend_selection(TEXT_GENERATION_SUPPORTED_OPS) {
            Ok(selection) => {
                assert_eq!(selection.requested_backend, "metal");
                assert_eq!(selection.effective_backend, "metal");
                assert_eq!(
                    selection.supported_ops,
                    TEXT_GENERATION_SUPPORTED_OPS
                        .iter()
                        .map(|label| String::from(*label))
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    selection.backend_extensions,
                    vec![
                        psionic_runtime::BackendExtensionSupport::reference(
                            BackendExtensionKind::RmsNorm
                        ),
                        psionic_runtime::BackendExtensionSupport::reference(
                            BackendExtensionKind::RotaryEmbedding
                        ),
                        psionic_runtime::BackendExtensionSupport::reference(
                            BackendExtensionKind::ScaledDotProductAttention
                        ),
                    ]
                );
                assert!(selection.selected_device.is_some());
                assert!(selection.fallback_reason.is_none());
                assert!(selection.runtime_resources.is_some());
            }
            Err(error) => {
                assert!(error.to_string().starts_with("metal backend unavailable: "));
                let fallback = backend.fallback_selection(&cpu, TEXT_GENERATION_SUPPORTED_OPS)?;
                assert_eq!(fallback.requested_backend, "metal");
                assert_eq!(fallback.effective_backend, "cpu");
                assert_eq!(
                    fallback.supported_ops,
                    TEXT_GENERATION_SUPPORTED_OPS
                        .iter()
                        .map(|label| String::from(*label))
                        .collect::<Vec<_>>()
                );
                assert!(fallback.selected_device.is_some());
                assert!(fallback.fallback_reason.is_some());
                assert!(fallback.runtime_resources.is_some());
            }
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_configures_text_generation_runtime_policy_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let policy = super::MetalTextGenerationRuntimePolicy::gpt_oss_default();
        let resources = backend.configure_text_generation_runtime(policy.clone())?;
        assert_eq!(resources.policy, policy);
        assert_eq!(resources.allocator_pool.policy, policy.allocator_pool);
        assert_eq!(resources.kernel_cache.policy, policy.kernel_cache);
        assert_eq!(
            backend
                .runtime_resources()
                .expect("runtime resources")
                .allocator_pool
                .policy,
            policy.allocator_pool
        );
        assert_eq!(
            backend
                .runtime_resources()
                .expect("runtime resources")
                .kernel_cache
                .policy,
            policy.kernel_cache
        );
        if let (Some(available), Some(required)) = (
            resources.device_memory_budget.available_execution_bytes,
            policy.minimum_available_bytes,
        ) {
            assert_eq!(resources.admission.admitted, available >= required);
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_reports_memory_refusal_when_text_generation_policy_exceeds_budget_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let policy = super::MetalTextGenerationRuntimePolicy {
            allocator_pool: psionic_runtime::AllocatorPoolPolicy::exact_tensor_spec(1, u64::MAX),
            kernel_cache: psionic_runtime::KernelCachePolicy::bounded(1, Some(u64::MAX)),
            minimum_available_bytes: Some(u64::MAX),
        };
        let resources = backend.configure_text_generation_runtime(policy)?;
        if resources.device_memory_budget.total_bytes.is_some() {
            assert_eq!(resources.admission.admitted, false);
            assert!(resources.admission.refusal_reason.is_some());
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_reports_quantized_weight_upload_support()
    -> Result<(), psionic_runtime::RuntimeError> {
        let backend = MetalBackend::new();
        let Some(selected) = backend.selected_device() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        assert_eq!(
            selected.supported_quantization,
            vec![
                QuantizationSupport {
                    mode: QuantizationMode::None,
                    load_path: QuantizationLoadPath::DenseF32,
                    execution: QuantizationExecution::Native,
                },
                QuantizationSupport {
                    mode: QuantizationMode::GgmlQ8_0,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::DequantizeToF32,
                },
                QuantizationSupport {
                    mode: QuantizationMode::GgmlMxfp4,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::DequantizeToF32,
                },
            ]
        );
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_executes_embedding_surface_on_supported_hardware()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = MetalBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
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
            .ok_or("missing metal embedding output")?;
        assert_eq!(output.read_f32()?, vec![1.5, 2.5]);
        assert_eq!(result.metrics.steps_executed, 5);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_executes_rms_norm_extension_on_supported_hardware()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = MetalBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let mut builder = GraphBuilder::new(selected.device.clone());
        let input = builder.input("hidden", Shape::new(vec![1, 4]), DType::F32);
        let weight = builder.constant_f32(Shape::new(vec![4]), vec![1.0; 4])?;
        let output = builder.rms_norm(&input, &weight, 1.0e-5)?;
        let graph = builder.finish(vec![output.clone()]);

        let mut inputs = std::collections::BTreeMap::new();
        inputs.insert(
            input.id(),
            backend.input_buffer(Shape::new(vec![1, 4]), vec![1.0, 2.0, 3.0, 4.0])?,
        );

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output = result
            .outputs
            .get(&output.id())
            .ok_or("missing metal rms_norm output")?;
        let values = output.read_f32()?;
        let expected = [0.36514813_f32, 0.73029625_f32, 1.0954444_f32, 1.4605925_f32];
        for (actual, expected) in values.iter().zip(expected.iter()) {
            assert!((actual - expected).abs() <= 1.0e-5);
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_executes_rotary_embedding_extension_on_supported_hardware()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = MetalBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let mut builder = GraphBuilder::new(selected.device.clone());
        let input = builder.input("q", Shape::new(vec![1, 1, 1, 4]), DType::F32);
        let cos = builder.constant_f32(Shape::new(vec![1, 2]), vec![0.0, 1.0])?;
        let sin = builder.constant_f32(Shape::new(vec![1, 2]), vec![1.0, 0.0])?;
        let output = builder.rope(&input, &cos, &sin, false)?;
        let graph = builder.finish(vec![output.clone()]);

        let mut inputs = std::collections::BTreeMap::new();
        inputs.insert(
            input.id(),
            backend.input_buffer(Shape::new(vec![1, 1, 1, 4]), vec![1.0, 2.0, 3.0, 4.0])?,
        );

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output = result
            .outputs
            .get(&output.id())
            .ok_or("missing metal rope output")?;
        assert_eq!(output.read_f32()?, vec![-3.0, 2.0, 1.0, 4.0]);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_executes_text_generation_dense_surface_on_supported_hardware()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = MetalBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
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
            .ok_or("missing metal hidden output")?;
        let logits_output = result
            .outputs
            .get(&logits.id())
            .ok_or("missing metal logits output")?;
        assert_eq!(hidden_output.read_f32()?, vec![4.5, 6.0]);
        assert_eq!(logits_output.read_f32()?, vec![7.75, 5.5, 4.0]);
        assert_eq!(result.metrics.steps_executed, 15);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_argmax_reads_dense_logits_on_supported_hardware() -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let logits = backend.input_buffer(
            Shape::new(vec![2, 4]),
            vec![1.0, -2.0, 4.25, 3.0, 9.5, 0.0, 9.5, -1.0],
        )?;
        assert_eq!(backend.argmax_f32(&logits, 2, 4)?, vec![2, 0]);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_top_k_returns_sorted_logits_on_supported_hardware() -> Result<(), RuntimeError>
    {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let logits = backend.input_buffer(
            Shape::new(vec![2, 4]),
            vec![1.0, -2.0, 4.25, 3.0, 9.5, 0.0, 9.5, -1.0],
        )?;
        let result = backend.top_k_f32(&logits, 2, 4, 2)?;
        assert_eq!(result.row_count, 2);
        assert_eq!(result.top_k, 2);
        assert_eq!(result.indices, vec![2, 3, 0, 2]);
        assert_eq!(result.values, vec![4.25, 3.0, 9.5, 9.5]);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_selects_greedy_token_with_bounded_output_mode_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let logits = backend.input_buffer(Shape::new(vec![1, 4]), vec![1.0, -2.0, 4.25, 3.0])?;
        let selection = backend.select_logits_output_f32(
            &logits,
            1,
            4,
            super::MetalLogitsOutputMode::GreedyToken,
        )?;
        assert_eq!(selection.selected_tokens, vec![2]);
        assert!(selection.candidates.is_none());
        assert!(selection.logits.is_none());
        assert_eq!(
            selection.metrics.output_mode,
            super::MetalLogitsOutputMode::GreedyToken
        );
        assert_eq!(selection.metrics.readback_bytes, 4);
        assert_eq!(selection.metrics.raw_logits_materialized, false);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_bounds_top_k_candidate_output_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let logits = backend.input_buffer(
            Shape::new(vec![2, 4]),
            vec![1.0, -2.0, 4.25, 3.0, 9.5, 0.0, 9.5, -1.0],
        )?;
        let selection = backend.select_logits_output_f32(
            &logits,
            2,
            4,
            super::MetalLogitsOutputMode::TopKCandidates(2),
        )?;
        assert_eq!(selection.selected_tokens, vec![2, 0]);
        assert_eq!(
            selection.candidates.as_ref().map(|value| value.top_k),
            Some(2)
        );
        assert!(selection.logits.is_none());
        assert_eq!(
            selection.metrics.output_mode,
            super::MetalLogitsOutputMode::TopKCandidates(2)
        );
        assert_eq!(selection.metrics.readback_bytes, 32);
        assert_eq!(selection.metrics.raw_logits_materialized, false);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_materializes_raw_logits_only_when_requested_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let logits = backend.input_buffer(Shape::new(vec![1, 4]), vec![1.0, -2.0, 4.25, 3.0])?;
        let selection = backend.select_logits_output_f32(
            &logits,
            1,
            4,
            super::MetalLogitsOutputMode::RawLogits,
        )?;
        assert_eq!(selection.selected_tokens, vec![2]);
        assert!(selection.candidates.is_none());
        assert_eq!(selection.logits, Some(vec![1.0, -2.0, 4.25, 3.0]));
        assert_eq!(
            selection.metrics.output_mode,
            super::MetalLogitsOutputMode::RawLogits
        );
        assert_eq!(selection.metrics.readback_bytes, 16);
        assert_eq!(selection.metrics.raw_logits_materialized, true);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_executes_scaled_dot_product_attention_extension_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let mut builder = GraphBuilder::new(selected.device.clone());
        let query = builder.input("query", Shape::new(vec![1, 1, 2, 2]), DType::F32);
        let key = builder.input("key", Shape::new(vec![1, 1, 2, 2]), DType::F32);
        let value = builder.input("value", Shape::new(vec![1, 1, 2, 2]), DType::F32);
        let attended = builder
            .scaled_dot_product_attention(&query, &key, &value, 1.0, true)
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let graph = builder.finish(vec![attended.clone()]);

        let mut inputs = std::collections::BTreeMap::new();
        inputs.insert(
            query.id(),
            backend.input_buffer(Shape::new(vec![1, 1, 2, 2]), vec![1.0, 0.0, 0.0, 1.0])?,
        );
        inputs.insert(
            key.id(),
            backend.input_buffer(Shape::new(vec![1, 1, 2, 2]), vec![1.0, 0.0, 0.0, 1.0])?,
        );
        inputs.insert(
            value.id(),
            backend.input_buffer(Shape::new(vec![1, 1, 2, 2]), vec![2.0, 1.0, 4.0, 3.0])?,
        );

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output = result
            .outputs
            .get(&attended.id())
            .ok_or_else(|| RuntimeError::Backend(String::from("missing attention output")))?;
        assert_close(
            output.read_f32()?.as_slice(),
            &[2.0, 1.0, 3.4621172, 2.4621172],
            1.0e-5,
        );
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_decode_attention_uses_device_kv_and_flash_path_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let mut cache = backend.kv_cache_mirror_from_host_rows(4, 8, 0, &[], &[], 4)?;
        let cos = backend.input_buffer(Shape::new(vec![1, 2]), vec![1.0, 1.0])?;
        let sin = backend.input_buffer(Shape::new(vec![1, 2]), vec![0.0, 0.0])?;
        let query_shape = Shape::new(vec![1, 2, 1, 4]);
        let kv_shape = Shape::new(vec![1, 1, 1, 4]);
        let first_query = backend.input_buffer(
            query_shape.clone(),
            vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        )?;
        let first_key = backend.input_buffer(kv_shape.clone(), vec![1.0, 0.0, 0.0, 0.0])?;
        let first_value = backend.input_buffer(kv_shape.clone(), vec![2.0, 4.0, 6.0, 8.0])?;

        let first = backend.decode_attention_f32(
            &first_query,
            &first_key,
            &first_value,
            &cos,
            &sin,
            &mut cache,
            1.0,
            true,
            false,
            true,
        )?;
        assert_eq!(first.stats.used_device_kv, true);
        assert_eq!(first.stats.rotary_applied, true);
        assert_eq!(first.stats.cache_write_index, 0);
        assert_eq!(first.cache_state.tokens, 1);
        assert_eq!(cache.len(), 1);
        assert_eq!(
            first.stats.flash_attention_path,
            backend.supports_flash_attention()
        );
        assert_close(
            first.output.read_f32()?.as_slice(),
            &[2.0, 4.0, 6.0, 8.0, 2.0, 4.0, 6.0, 8.0],
            1.0e-5,
        );

        let second_query =
            backend.input_buffer(query_shape, vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0])?;
        let second_key = backend.input_buffer(kv_shape.clone(), vec![0.0, 1.0, 0.0, 0.0])?;
        let second_value = backend.input_buffer(kv_shape, vec![1.0, 3.0, 5.0, 7.0])?;
        let second = backend.decode_attention_f32(
            &second_query,
            &second_key,
            &second_value,
            &cos,
            &sin,
            &mut cache,
            1.0,
            true,
            false,
            true,
        )?;
        assert_eq!(second.stats.cache_write_index, 1);
        assert_eq!(second.stats.cached_tokens, 2);
        assert_eq!(second.cache_state.tokens, 2);
        assert_eq!(
            second.stats.flash_attention_path,
            backend.supports_flash_attention()
        );
        assert_close(
            second.output.read_f32()?.as_slice(),
            &[
                1.7310586, 3.7310586, 5.7310586, 7.7310586, 1.2689414, 3.2689414, 5.2689414,
                7.2689414,
            ],
            1.0e-5,
        );
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_attention_graph_runtime_reports_reserve_reuse_and_rebuild_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let decode_reserve = MetalAttentionGraphReserve {
            kind: MetalGraphReserveKind::Decode,
            batch_size: 1,
            sequence_len: 1,
            query_head_count: 2,
            kv_head_count: 1,
            head_dim: 4,
            max_context_tokens: 8,
            causal: true,
            interleaved: false,
            flash_attention: backend.supports_flash_attention(),
        };
        let mut runtime = backend.reserve_attention_graph(decode_reserve.clone())?;
        assert_eq!(
            runtime.metrics().compile_path.temperature,
            CompilePathTemperature::ColdCompile
        );
        assert_eq!(runtime.metrics().command_state_reused, false);
        assert_eq!(
            runtime.metrics().identity.kind,
            MetalGraphReserveKind::Decode
        );

        let reused = runtime.ensure_reserved(&mut backend, decode_reserve)?;
        assert_eq!(
            reused.compile_path.temperature,
            CompilePathTemperature::WarmReuse
        );
        assert_eq!(reused.command_state_reused, true);
        assert_eq!(reused.reuse_count, 1);

        let prompt_reserve = MetalAttentionGraphReserve {
            kind: MetalGraphReserveKind::Prompt,
            batch_size: 1,
            sequence_len: 16,
            query_head_count: 2,
            kv_head_count: 1,
            head_dim: 4,
            max_context_tokens: 8,
            causal: true,
            interleaved: false,
            flash_attention: backend.supports_flash_attention(),
        };
        let rebuilt = runtime.ensure_reserved(&mut backend, prompt_reserve)?;
        assert_eq!(
            rebuilt.compile_path.temperature,
            CompilePathTemperature::ColdCompile
        );
        assert_eq!(rebuilt.command_state_reused, false);
        assert_eq!(rebuilt.identity.kind, MetalGraphReserveKind::Prompt);
        assert!(rebuilt.rebuild_count >= 2);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_decode_attention_reuses_reserved_runtime_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let reserve = MetalAttentionGraphReserve {
            kind: MetalGraphReserveKind::Decode,
            batch_size: 1,
            sequence_len: 1,
            query_head_count: 2,
            kv_head_count: 1,
            head_dim: 4,
            max_context_tokens: 8,
            causal: true,
            interleaved: false,
            flash_attention: backend.supports_flash_attention(),
        };
        let mut runtime = backend.reserve_attention_graph(reserve)?;
        let mut cache = backend.kv_cache_mirror_from_host_rows(4, 8, 0, &[], &[], 4)?;
        let cos = backend.input_buffer(Shape::new(vec![1, 2]), vec![1.0, 1.0])?;
        let sin = backend.input_buffer(Shape::new(vec![1, 2]), vec![0.0, 0.0])?;
        let query = backend.input_buffer(
            Shape::new(vec![1, 2, 1, 4]),
            vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        )?;
        let key = backend.input_buffer(Shape::new(vec![1, 1, 1, 4]), vec![1.0, 0.0, 0.0, 0.0])?;
        let value = backend.input_buffer(Shape::new(vec![1, 1, 1, 4]), vec![2.0, 4.0, 6.0, 8.0])?;

        let result = backend.decode_attention_f32_reserved(
            &mut runtime,
            &query,
            &key,
            &value,
            &cos,
            &sin,
            &mut cache,
            1.0,
            true,
            false,
            true,
        )?;
        assert_eq!(
            result
                .graph_metrics
                .as_ref()
                .map(|value| value.command_state_reused),
            Some(true)
        );
        assert_eq!(
            result
                .graph_metrics
                .as_ref()
                .map(|value| value.compile_path.temperature),
            Some(CompilePathTemperature::WarmReuse)
        );
        assert_eq!(result.cache_state.tokens, 1);
        assert_close(
            result.output.read_f32()?.as_slice(),
            &[2.0, 4.0, 6.0, 8.0, 2.0, 4.0, 6.0, 8.0],
            1.0e-5,
        );
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_executes_q8_0_quantized_matvec_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let weights = backend.quantized_buffer(
            Shape::new(vec![2, 32]),
            QuantizationMode::GgmlQ8_0,
            sample_repeated_q8_0_rows(2),
        )?;
        let values =
            backend.quantized_matvec(&weights, QuantizationMode::GgmlQ8_0, 2, 32, &[1.0; 32])?;
        assert_eq!(values, vec![0.0, 0.0]);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_mul_mv_id_matches_grouped_q8_0_reference_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let rows_per_expert = 2;
        let expert_count = 3;
        let columns = 32;
        let row_stride = 34;
        let selected_ids = vec![2_i32, 0_i32];
        let weights = [
            sample_q8_0_row(0.25, 1),
            sample_q8_0_row(0.5, -1),
            sample_q8_0_row(0.125, -1),
            sample_q8_0_row(0.375, 1),
            sample_q8_0_row(0.625, 1),
            sample_q8_0_row(0.75, -1),
        ]
        .concat();
        let input = sample_reference_vector();

        let weight_buffer = backend.quantized_buffer(
            Shape::new(vec![expert_count * rows_per_expert, columns]),
            QuantizationMode::GgmlQ8_0,
            weights.clone(),
        )?;
        let input_buffer = backend.input_buffer(Shape::new(vec![columns]), input.clone())?;
        let result = backend.mul_mv_id(
            &weight_buffer,
            QuantizationMode::GgmlQ8_0,
            row_stride,
            rows_per_expert,
            columns,
            selected_ids.as_slice(),
            &input_buffer,
        )?;

        assert_eq!(
            result.stats,
            super::MetalGroupedExpertStats {
                grouped_path: true,
                expert_count,
                selected_count: selected_ids.len(),
                rows_per_expert,
                row_stride,
            }
        );
        let expected = expected_grouped_expert_outputs(
            QuantizationMode::GgmlQ8_0,
            row_stride,
            rows_per_expert,
            selected_ids.as_slice(),
            input.as_slice(),
            weights.as_slice(),
        )?;
        assert_eq!(result.values, expected);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_mul_mv_id_matches_grouped_mxfp4_reference_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let rows_per_expert = 2;
        let expert_count = 3;
        let columns = 32;
        let row_stride = 17;
        let selected_ids = vec![1_i32, 2_i32];
        let weights = [
            sample_mxfp4_row(4),
            sample_mxfp4_row(5),
            sample_mxfp4_row(6),
            sample_mxfp4_row(7),
            sample_mxfp4_row(5),
            sample_mxfp4_row(4),
        ]
        .concat();
        let input = sample_reference_vector();

        let weight_buffer = backend.quantized_buffer(
            Shape::new(vec![expert_count * rows_per_expert, columns]),
            QuantizationMode::GgmlMxfp4,
            weights.clone(),
        )?;
        let input_buffer = backend.input_buffer(Shape::new(vec![columns]), input.clone())?;
        let result = backend.mul_mv_id(
            &weight_buffer,
            QuantizationMode::GgmlMxfp4,
            row_stride,
            rows_per_expert,
            columns,
            selected_ids.as_slice(),
            &input_buffer,
        )?;

        assert_eq!(result.stats.grouped_path, true);
        assert_eq!(result.stats.expert_count, expert_count);
        assert_eq!(result.stats.selected_count, selected_ids.len());
        let expected = expected_grouped_expert_outputs(
            QuantizationMode::GgmlMxfp4,
            row_stride,
            rows_per_expert,
            selected_ids.as_slice(),
            input.as_slice(),
            weights.as_slice(),
        )?;
        assert_eq!(result.values, expected);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_expert_matvec_f32_ids_matches_grouped_q8_0_reference_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let rows_per_expert = 2;
        let expert_count = 3;
        let columns = 32;
        let row_stride = 34;
        let selected_ids = vec![2_i32, 0_i32];
        let weights = [
            sample_q8_0_row(0.25, 1),
            sample_q8_0_row(0.5, -1),
            sample_q8_0_row(0.125, -1),
            sample_q8_0_row(0.375, 1),
            sample_q8_0_row(0.625, 1),
            sample_q8_0_row(0.75, -1),
        ]
        .concat();
        let inputs = [sample_reference_vector(), vec![0.5; columns]].concat();

        let weight_buffer = backend.quantized_buffer(
            Shape::new(vec![expert_count * rows_per_expert, columns]),
            QuantizationMode::GgmlQ8_0,
            weights.clone(),
        )?;
        let input_buffer = backend.input_buffer(
            Shape::new(vec![selected_ids.len(), columns]),
            inputs.clone(),
        )?;
        let result = backend.expert_matvec_f32_ids(
            &weight_buffer,
            QuantizationMode::GgmlQ8_0,
            row_stride,
            rows_per_expert,
            columns,
            selected_ids.as_slice(),
            &input_buffer,
        )?;

        assert_eq!(
            result.stats,
            super::MetalGroupedExpertStats {
                grouped_path: true,
                expert_count,
                selected_count: selected_ids.len(),
                rows_per_expert,
                row_stride,
            }
        );
        let expected = expected_grouped_expert_row_outputs(
            QuantizationMode::GgmlQ8_0,
            row_stride,
            rows_per_expert,
            selected_ids.as_slice(),
            inputs.as_slice(),
            columns,
            weights.as_slice(),
        )?;
        assert_eq!(result.values, expected);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_expert_matvec_f32_ids_matches_grouped_mxfp4_reference_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let rows_per_expert = 2;
        let expert_count = 3;
        let columns = 32;
        let row_stride = 17;
        let selected_ids = vec![1_i32, 2_i32];
        let weights = [
            sample_mxfp4_row(4),
            sample_mxfp4_row(5),
            sample_mxfp4_row(6),
            sample_mxfp4_row(7),
            sample_mxfp4_row(5),
            sample_mxfp4_row(4),
        ]
        .concat();
        let inputs = [sample_reference_vector(), vec![0.25; columns]].concat();

        let weight_buffer = backend.quantized_buffer(
            Shape::new(vec![expert_count * rows_per_expert, columns]),
            QuantizationMode::GgmlMxfp4,
            weights.clone(),
        )?;
        let input_buffer = backend.input_buffer(
            Shape::new(vec![selected_ids.len(), columns]),
            inputs.clone(),
        )?;
        let result = backend.expert_matvec_f32_ids(
            &weight_buffer,
            QuantizationMode::GgmlMxfp4,
            row_stride,
            rows_per_expert,
            columns,
            selected_ids.as_slice(),
            &input_buffer,
        )?;

        assert_eq!(result.stats.grouped_path, true);
        assert_eq!(result.stats.expert_count, expert_count);
        assert_eq!(result.stats.selected_count, selected_ids.len());
        let expected = expected_grouped_expert_row_outputs(
            QuantizationMode::GgmlMxfp4,
            row_stride,
            rows_per_expert,
            selected_ids.as_slice(),
            inputs.as_slice(),
            columns,
            weights.as_slice(),
        )?;
        assert_eq!(result.values, expected);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_kv_cache_mirror_appends_and_reads_entries_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let width = 4;
        let max_context_tokens = 8;
        let before = KvCacheState::default();
        let mut mirror = backend.kv_cache_mirror_from_host_rows(
            width,
            max_context_tokens,
            2,
            &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
            &[10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0],
            2,
        )?;

        assert_eq!(mirror.len(), 2);
        assert_eq!(
            mirror.read_entry(1)?,
            (vec![5.0, 6.0, 7.0, 8.0], vec![50.0, 60.0, 70.0, 80.0])
        );
        assert_eq!(
            mirror.page_layout(),
            KvCachePageLayout::new(max_context_tokens, 4, width * 4 * 2)
        );

        let write_index = mirror.append_entry(
            &mut backend,
            &[9.0, 10.0, 11.0, 12.0],
            &[90.0, 100.0, 110.0, 120.0],
        )?;
        assert_eq!(write_index, 2);
        assert_eq!(
            mirror.read_entry(2)?,
            (vec![9.0, 10.0, 11.0, 12.0], vec![90.0, 100.0, 110.0, 120.0])
        );

        let accounting = KvCacheAccounting::from_states(&before, mirror.state());
        assert_eq!(accounting.current.tokens, 3);
        assert_eq!(accounting.current.pages, 1);
        assert_eq!(accounting.growth.tokens, 3);
        assert_eq!(accounting.growth.pages, 1);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_shared_prefix_store_reuses_device_resident_prefix_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let width = 4;
        let max_context_tokens = 8;
        let compatibility = sample_prefix_compatibility(width, max_context_tokens);
        let cache = backend.kv_cache_mirror_from_host_rows(
            width,
            max_context_tokens,
            3,
            &[1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0, 6.5],
            &[
                10.0, 10.5, 11.0, 11.5, 12.0, 12.5, 13.0, 13.5, 14.0, 14.5, 15.0, 15.5,
            ],
            2,
        )?;

        let mut store = MetalSharedPrefixStore::default();
        let recorded_identity = store.record(compatibility.clone(), &[1, 2, 3], &cache);
        let lookup = store.lookup(&compatibility, &[1, 2, 3, 4]);

        assert_eq!(lookup.state, PrefixCacheState::Hit);
        assert_eq!(lookup.reused_tokens, 3);
        assert_eq!(lookup.identity, Some(recorded_identity.clone()));
        assert_eq!(lookup.cache.as_ref().map(|value| value.len()), Some(3));
        assert_eq!(
            lookup.cache.as_ref().expect("reused cache").read_entry(2)?,
            (vec![5.0, 5.5, 6.0, 6.5], vec![14.0, 14.5, 15.0, 15.5])
        );

        let metrics = MetalPromptResidencyMetrics::new(
            &KvCacheState::default(),
            lookup.cache.as_ref().expect("reused cache").state(),
            lookup.state,
            lookup.identity.clone(),
            CacheAction::Reuse,
        );
        assert_eq!(metrics.prefix_state, PrefixCacheState::Hit);
        assert_eq!(metrics.prefix_identity, Some(recorded_identity));
        assert_eq!(metrics.kv_accounting.current.tokens, 3);
        assert_eq!(metrics.kv_accounting.growth.tokens, 3);
        assert_eq!(metrics.observations.len(), 2);
        assert_eq!(metrics.observations[0].kind, CacheKind::PrefixCache);
        assert_eq!(metrics.observations[0].action, CacheAction::Reuse);
        assert_eq!(metrics.observations[1].kind, CacheKind::KvState);
        assert_eq!(metrics.observations[1].action, CacheAction::Reuse);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_shared_prefix_store_rebuilds_stale_entries_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let width = 4;
        let max_context_tokens = 8;
        let compatibility = sample_prefix_compatibility(width, max_context_tokens);
        let stale_cache = backend.kv_cache_mirror_from_host_rows(
            width,
            max_context_tokens,
            2,
            &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
            &[10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0],
            2,
        )?;

        let mut store = MetalSharedPrefixStore::default();
        store.record(compatibility.clone(), &[1, 2, 3], &stale_cache);

        let lookup = store.lookup(&compatibility, &[1, 2, 3, 4]);
        assert_eq!(lookup.state, PrefixCacheState::Rebuilt);
        assert_eq!(lookup.reused_tokens, 0);
        assert!(lookup.identity.is_none());
        assert!(lookup.cache.is_none());
        assert!(store.entries.is_empty());

        let metrics = MetalPromptResidencyMetrics::new(
            &stale_cache.state(),
            KvCacheState::default(),
            lookup.state,
            None,
            CacheAction::Invalidate,
        );
        assert_eq!(metrics.prefix_state, PrefixCacheState::Rebuilt);
        assert_eq!(metrics.kv_accounting.current, KvCacheState::default());
        assert_eq!(metrics.observations[0].kind, CacheKind::PrefixCache);
        assert_eq!(metrics.observations[0].action, CacheAction::Invalidate);
        assert_eq!(metrics.observations[1].kind, CacheKind::KvState);
        assert_eq!(metrics.observations[1].action, CacheAction::Invalidate);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_outputs_quantized_constant_storage_truth() -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let quantized_shape = Shape::new(vec![2, 32]);
        let quantized_bytes = sample_repeated_q8_0_rows(2);
        let mut builder = GraphBuilder::new(selected.device.clone());
        let rhs = builder
            .constant_quantized_blocks(
                quantized_shape.clone(),
                QuantizationMode::GgmlQ8_0,
                quantized_bytes.clone(),
            )
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let graph = builder.finish(vec![rhs.clone()]);

        let result = backend.compile_and_execute(&graph, &std::collections::BTreeMap::new())?;
        let output = result
            .outputs
            .get(&rhs.id())
            .ok_or_else(|| RuntimeError::Backend(String::from("quantized constant output")))?;
        assert_eq!(
            output.storage_kind(),
            BufferStorageKind::QuantizedBlocks {
                mode: QuantizationMode::GgmlQ8_0,
                layout: QuantizationMode::GgmlQ8_0
                    .ggml_block_layout(&quantized_shape)
                    .ok_or_else(|| RuntimeError::Backend(String::from("q8_0 layout")))?,
                residency: BufferResidency::Backend,
            }
        );
        assert_eq!(output.read_bytes()?, quantized_bytes);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_uploads_mxfp4_constant_bytes_without_dense_rewrite() -> Result<(), RuntimeError>
    {
        let mut backend = MetalBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let quantized_shape = Shape::new(vec![3, 32]);
        let quantized_bytes = sample_repeated_mxfp4_rows(3);
        let mut builder = GraphBuilder::new(selected.device.clone());
        let rhs = builder
            .constant_quantized_blocks(
                quantized_shape.clone(),
                QuantizationMode::GgmlMxfp4,
                quantized_bytes.clone(),
            )
            .map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let graph = builder.finish(vec![rhs.clone()]);

        let result = backend.compile_and_execute(&graph, &std::collections::BTreeMap::new())?;
        let output = result
            .outputs
            .get(&rhs.id())
            .ok_or_else(|| RuntimeError::Backend(String::from("mxfp4 constant output")))?;
        assert_eq!(
            output.storage_kind(),
            BufferStorageKind::QuantizedBlocks {
                mode: QuantizationMode::GgmlMxfp4,
                layout: QuantizationMode::GgmlMxfp4
                    .ggml_block_layout(&quantized_shape)
                    .ok_or_else(|| RuntimeError::Backend(String::from("mxfp4 layout")))?,
                residency: BufferResidency::Backend,
            }
        );
        assert_eq!(output.read_bytes()?, quantized_bytes);
        Ok(())
    }
}
