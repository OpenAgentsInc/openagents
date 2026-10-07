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
    borrow::Cow,
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt,
    sync::{Arc, OnceLock},
    thread,
};

use psionic_compiler::compile_graph;
use psionic_core::{
    BackendExtensionKind, BackendExtensionOp, DType, DeviceKind, QuantizationMode, Shape,
    TensorData, TensorId, TensorSpec,
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
    KvCacheEncodingFamily, KvCacheEncodingPolicy, KvCachePageLayout, KvCacheState,
    PrefixCacheIdentity, PrefixCacheState, RuntimeError, RuntimeHealth, ServedProductBackendPolicy,
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
const METAL_DECODE_SIMDGROUP_THREADS: u64 = 32;
const METAL_DECODE_MAX_SIMDGROUPS: usize = 31;
const METAL_QUANTIZED_ARGMAX_ROWS_PER_THREADGROUP: usize = 8;
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
const GGML_Q8_1_BLOCK_ELEMENTS: usize = 32;
const GGML_Q8_1_BLOCK_BYTES: usize = 36;
pub const METAL_SINGLE_ROW_TOP_K_MAX: usize = 32;

fn metal_decode_env_simdgroups() -> Option<usize> {
    static OVERRIDE: OnceLock<Option<usize>> = OnceLock::new();
    *OVERRIDE.get_or_init(|| {
        std::env::var("PSIONIC_METAL_DECODE_SIMDGROUPS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| (1..=METAL_DECODE_MAX_SIMDGROUPS).contains(value))
    })
}

fn metal_decode_active_simdgroups(token_count: usize) -> usize {
    metal_decode_env_simdgroups()
        .unwrap_or_else(|| token_count.clamp(1, METAL_DECODE_MAX_SIMDGROUPS))
}

pub fn quantized_argmax_candidate_count(rows: usize) -> usize {
    rows.div_ceil(METAL_QUANTIZED_ARGMAX_ROWS_PER_THREADGROUP)
}

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
    "top_k_single_row_f32",
    "mul_mv_id_q5_0",
    "mul_mv_id_q4_k",
    "mul_mv_id_q8_0",
    "mul_mv_id_mxfp4",
    "expert_matvec_f32_ids_q5_0",
    "expert_matvec_f32_ids_q4_k",
    "expert_matvec_f32_ids_q8_0",
    "expert_matvec_f32_ids_mxfp4",
    "top_k_softmax_single_row_f32",
    "add_selected_expert_bias_f32",
    "aggregate_selected_expert_rows_f32",
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

/// Result of one projected decode-attention block that stayed on Metal until
/// the final output projection was materialized on the host.
#[derive(Clone, Debug, PartialEq)]
pub struct MetalProjectedAttentionResult {
    /// Final projected attention output with logical shape `[output_rows]`.
    pub values: Vec<f32>,
    /// Live KV row written for the current token when the layer owns KV.
    pub key_values: Option<Vec<f32>>,
    /// Live value row written for the current token when the layer owns KV.
    pub value_values: Option<Vec<f32>>,
}

/// One quantized row-wise matrix-vector request that shares a staged input.
#[derive(Clone, Copy)]
pub struct MetalQuantizedMatvecRequest<'a> {
    /// Quantized weight buffer.
    pub weights: &'a MetalBuffer,
    /// Byte offset into the packed weight buffer.
    pub byte_offset: usize,
    /// Quantization mode for the packed rows.
    pub mode: psionic_core::QuantizationMode,
    /// Number of logical output rows.
    pub rows: usize,
    /// Number of logical input columns.
    pub columns: usize,
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

/// Output of one backend-owned decode-attention step returned directly on the host path.
#[derive(Clone, Debug, PartialEq)]
pub struct MetalDecodeAttentionHostResult {
    /// Attention output values with logical shape `[1, query_heads, 1, head_dim]`.
    pub output_values: Vec<f32>,
    /// Observable KV state after the current decode step.
    pub cache_state: KvCacheState,
    /// Explicit decode-attention execution evidence.
    pub stats: MetalDecodeAttentionStats,
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
    row_byte_len: usize,
    len: usize,
    capacity_tokens: usize,
    max_context_tokens: usize,
    kv_cache_encoding_policy: KvCacheEncodingPolicy,
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
    /// Tenant or security-domain binding when present.
    pub tenant_id: Option<String>,
    /// Stable sampler digest when reuse depends on decode settings.
    pub sampler_digest: Option<String>,
    /// Stable backend compatibility label.
    pub backend_compatibility: String,
    /// KV width required for reuse.
    pub kv_width: usize,
    /// Logical KV page layout required for reuse.
    pub page_layout: KvCachePageLayout,
    /// Active KV-cache encoding policy required for reuse.
    pub kv_cache_encoding_policy: KvCacheEncodingPolicy,
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
    byte_offset: usize,
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
            .field("byte_offset", &self.byte_offset)
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
    /// Returns the logical byte offset inside the backing allocation.
    #[must_use]
    pub const fn byte_offset(&self) -> usize {
        self.byte_offset
    }

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
        self.platform
            .write_bytes_at_offset(self.byte_offset, bytes, self.storage_mode)
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
        self.platform.write_bytes_at_offset(
            self.byte_offset.saturating_add(byte_offset),
            bytes,
            self.storage_mode,
        )
    }

    /// Reads raw bytes from the host-visible buffer contents.
    pub fn read_bytes(&self) -> Result<Vec<u8>, RuntimeError> {
        self.platform
            .read_bytes_at_offset(self.byte_offset, self.byte_len)
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
        self.platform
            .read_bytes_at_offset(self.byte_offset.saturating_add(byte_offset), byte_len)
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
        self.platform.with_bytes_at_offset(
            self.byte_offset.saturating_add(byte_offset),
            byte_len,
            map,
        )
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
                values.len(),
            )));
        }
        let byte_len = values
            .len()
            .saturating_mul(size_of_dtype(self.spec.dtype()));
        let bytes = unsafe { std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), byte_len) };
        self.write_bytes(bytes)
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
        let byte_len = values
            .len()
            .saturating_mul(size_of_dtype(self.spec.dtype()));
        let bytes = unsafe { std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), byte_len) };
        self.write_bytes_at_offset(0, bytes)
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
        let mut values = Vec::with_capacity(self.spec.storage_size());
        self.with_bytes(|bytes| {
            extend_f32_values_from_bytes(&mut values, bytes);
            Ok(())
        })?;
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
            extend_f32_values_from_bytes(output, bytes);
            Ok(())
        })
    }

    /// Creates a dense `f32` logical view into a parent dense `f32` buffer.
    pub fn dense_f32_view(
        &self,
        element_offset: usize,
        element_count: usize,
    ) -> Result<Self, RuntimeError> {
        if self.spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "metal dense_f32_view requires F32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        if self.storage_kind != BufferStorageKind::DenseF32 {
            return Err(RuntimeError::Backend(format!(
                "metal dense_f32_view requires dense f32 storage, actual {:?}",
                self.storage_kind
            )));
        }
        let total_elements = self.spec.storage_size();
        if element_offset.saturating_add(element_count) > total_elements {
            return Err(RuntimeError::Backend(format!(
                "metal dense_f32_view exceeds allocation: offset={} count={} allocation={}",
                element_offset, element_count, total_elements
            )));
        }
        Ok(Self {
            spec: TensorSpec::new(
                Shape::new(vec![element_count]),
                DType::F32,
                self.spec.device().clone(),
            ),
            byte_offset: self
                .byte_offset
                .saturating_add(element_offset.saturating_mul(size_of_dtype(DType::F32))),
            byte_len: element_count.saturating_mul(size_of_dtype(DType::F32)),
            storage_kind: self.storage_kind.clone(),
            storage_mode: self.storage_mode,
            host_visible: self.host_visible,
            host_writable: self.host_writable,
            _keepalive: self._keepalive.clone(),
            platform: self.platform.clone(),
        })
    }
}

fn extend_f32_values_from_bytes(output: &mut Vec<f32>, bytes: &[u8]) {
    let element_size = size_of::<f32>();
    let (prefix, aligned, suffix) = unsafe { bytes.align_to::<f32>() };
    if prefix.is_empty() && suffix.is_empty() {
        output.extend_from_slice(aligned);
        return;
    }

    output.extend(
        bytes
            .chunks_exact(element_size)
            .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])),
    );
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
            .fill_buffer(&buffer.platform, buffer.byte_offset, buffer.byte_len, value)?;
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
        self.platform.copy_buffer(
            &source.platform,
            source.byte_offset,
            &destination.platform,
            destination.byte_offset,
            source.byte_len,
        )?;
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

    /// Creates a zeroed dense `f32` buffer on the selected Metal device.
    pub fn zeros_buffer(&mut self, shape: Shape) -> Result<MetalBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        self.allocate(&TensorSpec::new(shape, DType::F32, device))
    }

    /// Creates a zeroed dense `i32` buffer on the selected Metal device.
    pub fn zeros_i32_buffer(&mut self, shape: Shape) -> Result<MetalBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        self.allocate(&TensorSpec::new(shape, DType::I32, device))
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

    /// Executes multiple quantized row-wise matrix-vector products that share
    /// the same staged dense input.
    pub fn quantized_matvec_batch(
        &mut self,
        requests: &[MetalQuantizedMatvecRequest<'_>],
        input: &[f32],
    ) -> Result<Vec<MetalQuantizedMatvecResult>, RuntimeError> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        for request in requests {
            let Some((elements_per_block, bytes_per_block)) = request.mode.ggml_block_spec() else {
                return Err(RuntimeError::Backend(format!(
                    "metal quantized matvec does not support mode {:?}",
                    request.mode
                )));
            };
            if request.columns == 0 || request.columns % elements_per_block != 0 {
                return Err(RuntimeError::Backend(format!(
                    "metal quantized matvec requires block-aligned width {} for {:?}",
                    request.columns, request.mode
                )));
            }
            if input.len() != request.columns {
                return Err(RuntimeError::Backend(format!(
                    "metal quantized matvec input width mismatch: expected {}, actual {}",
                    request.columns,
                    input.len()
                )));
            }
            let row_stride = (request.columns / elements_per_block)
                .checked_mul(bytes_per_block)
                .ok_or_else(|| {
                    RuntimeError::Backend(String::from(
                        "metal quantized matvec row stride overflow",
                    ))
                })?;
            let required_bytes = request.rows.saturating_mul(row_stride);
            let end_offset = request.byte_offset.saturating_add(required_bytes);
            match request.weights.storage_kind() {
                BufferStorageKind::QuantizedBlocks {
                    mode: stored_mode, ..
                } if stored_mode == request.mode => {}
                BufferStorageKind::QuantizedBlocks {
                    mode: stored_mode, ..
                } => {
                    return Err(RuntimeError::Backend(format!(
                        "metal quantized matvec mode mismatch: requested {:?}, stored {stored_mode:?}",
                        request.mode
                    )));
                }
                storage_kind => {
                    return Err(RuntimeError::Backend(format!(
                        "metal quantized matvec requires quantized block storage, actual {:?}",
                        storage_kind
                    )));
                }
            }
            if request.weights.byte_len() < end_offset {
                return Err(RuntimeError::Backend(format!(
                    "metal quantized matvec byte length mismatch: required {end_offset}, actual {}",
                    request.weights.byte_len(),
                )));
            }
        }
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.run_quantized_matvec_batch(requests, input)
    }

    /// Executes one Gemma-style FFN block on Metal by keeping the
    /// gate-projection, up-projection, GELU-GLU activation, and down-projection
    /// on the device until the final projected vector is ready.
    pub fn quantized_gelu_glu_projected(
        &mut self,
        gate_weights: &MetalBuffer,
        gate_mode: QuantizationMode,
        gate_rows: usize,
        gate_columns: usize,
        up_weights: &MetalBuffer,
        up_mode: QuantizationMode,
        up_rows: usize,
        up_columns: usize,
        down_weights: &MetalBuffer,
        down_mode: QuantizationMode,
        down_rows: usize,
        down_columns: usize,
        input: &[f32],
    ) -> Result<Vec<f32>, RuntimeError> {
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.run_quantized_gelu_glu_projected(
            gate_weights,
            gate_mode,
            gate_rows,
            gate_columns,
            up_weights,
            up_mode,
            up_rows,
            up_columns,
            down_weights,
            down_mode,
            down_rows,
            down_columns,
            input,
        )
    }

    /// Executes one Gemma per-layer input block on Metal by keeping the input
    /// gate projection, GELU-times-layer-slice multiply, and projection on the
    /// device until the final projected vector is ready.
    pub fn quantized_gelu_mul_projected(
        &mut self,
        gate_weights: &MetalBuffer,
        gate_mode: QuantizationMode,
        gate_rows: usize,
        gate_columns: usize,
        multiplier: &[f32],
        projection_weights: &MetalBuffer,
        projection_mode: QuantizationMode,
        projection_rows: usize,
        projection_columns: usize,
        input: &[f32],
    ) -> Result<Vec<f32>, RuntimeError> {
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.run_quantized_gelu_mul_projected(
            gate_weights,
            gate_mode,
            gate_rows,
            gate_columns,
            multiplier,
            projection_weights,
            projection_mode,
            projection_rows,
            projection_columns,
            input,
        )
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
        &mut self,
        input: &MetalBuffer,
        row_count: usize,
        column_count: usize,
    ) -> Result<Vec<u32>, RuntimeError> {
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.run_argmax_f32(input, row_count, column_count)
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
        &mut self,
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
                let candidates = if row_count == 1 && top_k <= METAL_SINGLE_ROW_TOP_K_MAX {
                    let Some(backend) = self.selected_backend_mut() else {
                        return Err(RuntimeError::Backend(String::from(
                            "metal backend unavailable: no selected execution device",
                        )));
                    };
                    backend.run_top_k_single_row_f32(input, column_count, top_k)?
                } else {
                    self.top_k_f32(input, row_count, column_count, top_k)?
                };
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

    /// Returns whether the backend can run the dense decode-attention kernel
    /// directly on Metal for the provided head width.
    #[must_use]
    pub fn supports_dense_decode_attention(&self, head_dim: usize) -> bool {
        matches!(head_dim, 64 | 96 | 128 | 256) && self.selected_device().is_some()
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

    /// Encodes one quantized row-wise matrix-vector product and writes one
    /// packed `(value_key, row)` candidate per row block into an `i32` buffer.
    pub fn encode_quantized_matvec_argmax_submission(
        &mut self,
        submission: &mut MetalSubmission,
        weights: &MetalBuffer,
        byte_offset: usize,
        mode: psionic_core::QuantizationMode,
        rows: usize,
        columns: usize,
        input: &MetalBuffer,
        selected: &MetalBuffer,
    ) -> Result<(), RuntimeError> {
        validate_quantized_matvec_argmax_request(
            weights,
            byte_offset,
            mode,
            rows,
            columns,
            input,
            selected,
        )?;
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_quantized_matvec_argmax(
            &mut submission.platform,
            weights,
            byte_offset,
            mode,
            rows,
            columns,
            input,
            selected,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Adds one dense bias vector into an existing dense buffer in place.
    pub fn encode_add_inplace_submission(
        &mut self,
        submission: &mut MetalSubmission,
        values: &MetalBuffer,
        bias: &MetalBuffer,
        element_count: usize,
    ) -> Result<(), RuntimeError> {
        if values.spec().dtype() != DType::F32 || bias.spec().dtype() != DType::F32 {
            return Err(RuntimeError::Backend(String::from(
                "metal add-inplace requires dense f32 buffers",
            )));
        }
        if values.byte_len() != bias.byte_len() {
            return Err(RuntimeError::Backend(format!(
                "metal add-inplace byte length mismatch: values={} bias={}",
                values.byte_len(),
                bias.byte_len(),
            )));
        }
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_add_inplace(
            &mut submission.platform,
            values,
            bias,
            element_count,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Copies one dense f32 slice into another dense f32 buffer.
    pub fn encode_copy_f32_slice_submission(
        &mut self,
        submission: &mut MetalSubmission,
        source: &MetalBuffer,
        destination: &MetalBuffer,
        element_count: usize,
        source_offset_elements: usize,
        destination_offset_elements: usize,
    ) -> Result<(), RuntimeError> {
        if source.spec().dtype() != DType::F32 || destination.spec().dtype() != DType::F32 {
            return Err(RuntimeError::Backend(String::from(
                "metal copy_f32_slice requires dense f32 buffers",
            )));
        }
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_copy_f32_slice(
            &mut submission.platform,
            source,
            destination,
            element_count,
            source_offset_elements,
            destination_offset_elements,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Multiplies a dense buffer by a scalar in place.
    pub fn encode_scale_inplace_submission(
        &mut self,
        submission: &mut MetalSubmission,
        values: &MetalBuffer,
        scale: f32,
        element_count: usize,
    ) -> Result<(), RuntimeError> {
        if values.spec().dtype() != DType::F32 {
            return Err(RuntimeError::Backend(String::from(
                "metal scale-inplace requires a dense f32 buffer",
            )));
        }
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_scale_inplace(
            &mut submission.platform,
            values,
            scale,
            element_count,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Encodes one dense `m x k` by `k x n` matmul into an existing submission.
    pub fn encode_matmul_submission(
        &mut self,
        submission: &mut MetalSubmission,
        left: &MetalBuffer,
        right: &MetalBuffer,
        output: &MetalBuffer,
        m: usize,
        k: usize,
        n: usize,
    ) -> Result<(), RuntimeError> {
        if left.spec().dtype() != DType::F32
            || right.spec().dtype() != DType::F32
            || output.spec().dtype() != DType::F32
        {
            return Err(RuntimeError::Backend(String::from(
                "metal matmul requires dense f32 buffers",
            )));
        }
        if left.spec().storage_size() < m.saturating_mul(k) {
            return Err(RuntimeError::Backend(format!(
                "metal matmul left buffer is too small: need {}, actual {}",
                m.saturating_mul(k),
                left.spec().storage_size()
            )));
        }
        if right.spec().storage_size() < k.saturating_mul(n) {
            return Err(RuntimeError::Backend(format!(
                "metal matmul right buffer is too small: need {}, actual {}",
                k.saturating_mul(n),
                right.spec().storage_size()
            )));
        }
        if output.spec().storage_size() < m.saturating_mul(n) {
            return Err(RuntimeError::Backend(format!(
                "metal matmul output buffer is too small: need {}, actual {}",
                m.saturating_mul(n),
                output.spec().storage_size()
            )));
        }
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_matmul_with_dims(
            &mut submission.platform,
            left,
            right,
            output,
            m,
            k,
            n,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Encodes one single-row top-k selection that preserves raw logits.
    pub fn encode_top_k_single_row_submission(
        &mut self,
        submission: &mut MetalSubmission,
        input: &MetalBuffer,
        column_count: usize,
        top_k: usize,
        selected_ids: &MetalBuffer,
        selected_values: &MetalBuffer,
    ) -> Result<(), RuntimeError> {
        validate_top_k_single_row_request(
            input,
            column_count,
            top_k,
            selected_ids,
            selected_values,
        )?;
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_top_k_single_row_f32(
            &mut submission.platform,
            input,
            selected_ids,
            selected_values,
            column_count,
            top_k,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Encodes one single-row top-k selection with in-k softmax normalization.
    pub fn encode_top_k_softmax_single_row_submission(
        &mut self,
        submission: &mut MetalSubmission,
        input: &MetalBuffer,
        column_count: usize,
        top_k: usize,
        selected_ids: &MetalBuffer,
        selected_weights: &MetalBuffer,
    ) -> Result<(), RuntimeError> {
        validate_top_k_softmax_single_row_request(
            input,
            column_count,
            top_k,
            selected_ids,
            selected_weights,
        )?;
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_top_k_softmax_single_row_f32(
            &mut submission.platform,
            input,
            selected_ids,
            selected_weights,
            column_count,
            top_k,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Applies GELU(gate) * up into an output buffer.
    pub fn encode_gelu_glu_submission(
        &mut self,
        submission: &mut MetalSubmission,
        gate: &MetalBuffer,
        up: &MetalBuffer,
        output: &MetalBuffer,
        element_count: usize,
    ) -> Result<(), RuntimeError> {
        if gate.spec().dtype() != DType::F32
            || up.spec().dtype() != DType::F32
            || output.spec().dtype() != DType::F32
        {
            return Err(RuntimeError::Backend(String::from(
                "metal gelu_glu requires dense f32 buffers",
            )));
        }
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_gelu_glu_f32(
            &mut submission.platform,
            gate,
            up,
            output,
            element_count,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Adds one selected-expert bias slice into grouped expert rows in place.
    pub fn encode_add_selected_expert_bias_submission(
        &mut self,
        submission: &mut MetalSubmission,
        values: &MetalBuffer,
        bias: &MetalBuffer,
        selected_ids: &MetalBuffer,
        selected_count: usize,
        rows_per_expert: usize,
    ) -> Result<(), RuntimeError> {
        validate_add_selected_expert_bias_request(
            values,
            bias,
            selected_ids,
            selected_count,
            rows_per_expert,
        )?;
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_add_selected_expert_bias_f32(
            &mut submission.platform,
            values,
            bias,
            selected_ids,
            rows_per_expert,
            selected_count,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Aggregates one grouped expert output surface into one output row using
    /// the selected routing weights and per-expert scales.
    pub fn encode_aggregate_selected_expert_rows_submission(
        &mut self,
        submission: &mut MetalSubmission,
        input: &MetalBuffer,
        selected_ids: &MetalBuffer,
        selected_weights: &MetalBuffer,
        expert_scales: &MetalBuffer,
        output: &MetalBuffer,
        selected_count: usize,
        rows_per_expert: usize,
    ) -> Result<(), RuntimeError> {
        validate_aggregate_selected_expert_rows_request(
            input,
            selected_ids,
            selected_weights,
            expert_scales,
            output,
            selected_count,
            rows_per_expert,
        )?;
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_aggregate_selected_expert_rows_f32(
            &mut submission.platform,
            input,
            selected_ids,
            selected_weights,
            expert_scales,
            output,
            rows_per_expert,
            selected_count,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Applies per-head RMS normalization with learned weights in place.
    pub fn encode_per_head_rms_norm_submission(
        &mut self,
        submission: &mut MetalSubmission,
        values: &MetalBuffer,
        weight: &MetalBuffer,
        head_count: usize,
        head_dim: usize,
        epsilon: f32,
    ) -> Result<(), RuntimeError> {
        if values.spec().dtype() != DType::F32 || weight.spec().dtype() != DType::F32 {
            return Err(RuntimeError::Backend(String::from(
                "metal per-head rms-norm requires dense f32 buffers",
            )));
        }
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_per_head_rms_norm(
            &mut submission.platform,
            values,
            weight,
            head_count,
            head_dim,
            epsilon,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Applies per-head RMS normalization from one dense f32 buffer into another.
    pub fn encode_per_head_rms_norm_to_output_submission(
        &mut self,
        submission: &mut MetalSubmission,
        input: &MetalBuffer,
        weight: &MetalBuffer,
        output: &MetalBuffer,
        head_count: usize,
        head_dim: usize,
        epsilon: f32,
    ) -> Result<(), RuntimeError> {
        if input.spec().dtype() != DType::F32
            || weight.spec().dtype() != DType::F32
            || output.spec().dtype() != DType::F32
        {
            return Err(RuntimeError::Backend(String::from(
                "metal per-head rms-norm output path requires dense f32 buffers",
            )));
        }
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_per_head_rms_norm_to_output(
            &mut submission.platform,
            input,
            weight,
            output,
            head_count,
            head_dim,
            epsilon,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Applies per-head RMS normalization with implicit unit weights in place.
    pub fn encode_per_head_rms_norm_unit_submission(
        &mut self,
        submission: &mut MetalSubmission,
        values: &MetalBuffer,
        head_count: usize,
        head_dim: usize,
        epsilon: f32,
    ) -> Result<(), RuntimeError> {
        if values.spec().dtype() != DType::F32 {
            return Err(RuntimeError::Backend(String::from(
                "metal per-head unit rms-norm requires a dense f32 buffer",
            )));
        }
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_per_head_rms_norm_unit(
            &mut submission.platform,
            values,
            head_count,
            head_dim,
            epsilon,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Applies non-interleaved NeoX-style RoPE in place.
    pub fn encode_rope_neox_inplace_submission(
        &mut self,
        submission: &mut MetalSubmission,
        values: &MetalBuffer,
        cos: &MetalBuffer,
        sin: &MetalBuffer,
        head_count: usize,
        head_dim: usize,
    ) -> Result<(), RuntimeError> {
        if values.spec().dtype() != DType::F32
            || cos.spec().dtype() != DType::F32
            || sin.spec().dtype() != DType::F32
        {
            return Err(RuntimeError::Backend(String::from(
                "metal rope-neox requires dense f32 buffers",
            )));
        }
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_rope_neox_inplace(
            &mut submission.platform,
            values,
            cos,
            sin,
            head_count,
            head_dim,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Applies non-interleaved NeoX-style RoPE in place by deriving the
    /// rotation directly on Metal from static per-layer parameters.
    #[allow(clippy::too_many_arguments)]
    pub fn encode_rope_neox_position_submission(
        &mut self,
        submission: &mut MetalSubmission,
        values: &MetalBuffer,
        freq_factors: &MetalBuffer,
        head_count: usize,
        head_dim: usize,
        rotary_half: usize,
        position: usize,
        theta_scale: f32,
        freq_scale: f32,
        corr_dims: [f32; 2],
        ext_factor: f32,
        yarn_mscale: f32,
    ) -> Result<(), RuntimeError> {
        if values.spec().dtype() != DType::F32 || freq_factors.spec().dtype() != DType::F32 {
            return Err(RuntimeError::Backend(String::from(
                "metal rope-neox-position requires dense f32 buffers",
            )));
        }
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_rope_neox_position(
            &mut submission.platform,
            values,
            freq_factors,
            head_count,
            head_dim,
            rotary_half,
            position,
            theta_scale,
            freq_scale,
            corr_dims,
            ext_factor,
            yarn_mscale,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Appends one dense KV row from Metal buffers into a dense Metal KV cache.
    pub fn encode_append_dense_kv_submission(
        &mut self,
        submission: &mut MetalSubmission,
        cache: &mut MetalKvCacheMirror,
        key: &MetalBuffer,
        value: &MetalBuffer,
    ) -> Result<usize, RuntimeError> {
        match cache.kv_cache_encoding_policy().family {
            KvCacheEncodingFamily::DenseF32 | KvCacheEncodingFamily::DenseF16Mirror => {}
            family => {
                return Err(RuntimeError::Backend(format!(
                    "metal dense kv append does not support cache encoding family {family:?}",
                )));
            }
        }
        if key.spec().dtype() != DType::F32 || value.spec().dtype() != DType::F32 {
            return Err(RuntimeError::Backend(String::from(
                "metal dense kv append requires dense f32 key/value buffers",
            )));
        }
        let expected_bytes = cache.width().saturating_mul(size_of::<f32>());
        if key.byte_len() != expected_bytes || value.byte_len() != expected_bytes {
            return Err(RuntimeError::Backend(format!(
                "metal dense kv append width mismatch: expected {} bytes, actual key={} value={}",
                expected_bytes,
                key.byte_len(),
                value.byte_len(),
            )));
        }
        if cache.len() >= cache.max_context_tokens {
            return Err(RuntimeError::Backend(format!(
                "metal kv cache exceeded max context {}",
                cache.max_context_tokens
            )));
        }
        cache.ensure_capacity(self, cache.len().saturating_add(1))?;
        let write_index = cache.len();
        let byte_offset = write_index.saturating_mul(cache.row_byte_len);
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_copy_f32_with_offset(
            &mut submission.platform,
            key,
            &cache.key_buffer,
            cache.width(),
            byte_offset / size_of::<f32>(),
        )?;
        backend.platform.encode_copy_f32_with_offset(
            &mut submission.platform,
            value,
            &cache.value_buffer,
            cache.width(),
            byte_offset / size_of::<f32>(),
        )?;
        cache.len = cache.len.saturating_add(1);
        submission.encoded_operations = submission.encoded_operations.saturating_add(2);
        Ok(write_index)
    }

    /// Encodes one dense decode-attention step against a dense Metal KV cache.
    pub fn encode_decode_attention_dense_submission(
        &mut self,
        submission: &mut MetalSubmission,
        query: &MetalBuffer,
        cache: &MetalKvCacheMirror,
        query_head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        scale: f32,
        output: &MetalBuffer,
    ) -> Result<(), RuntimeError> {
        match cache.kv_cache_encoding_policy().family {
            KvCacheEncodingFamily::DenseF32 | KvCacheEncodingFamily::DenseF16Mirror => {}
            family => {
                return Err(RuntimeError::Backend(format!(
                    "metal dense decode attention does not support cache encoding family {family:?}",
                )));
            }
        }
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        let active_simdgroups = metal_decode_active_simdgroups(cache.len());
        backend.platform.encode_decode_attention_dense(
            &mut submission.platform,
            query,
            cache,
            query_head_count,
            kv_head_count,
            head_dim,
            scale,
            output,
            active_simdgroups,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Encodes one argmax reduction over dense f32 rows.
    pub fn encode_argmax_f32_submission(
        &mut self,
        submission: &mut MetalSubmission,
        input: &MetalBuffer,
        output: &MetalBuffer,
        row_count: usize,
        column_count: usize,
    ) -> Result<(), RuntimeError> {
        if input.spec().dtype() != DType::F32 || output.spec().dtype() != DType::F32 {
            return Err(RuntimeError::Backend(String::from(
                "metal argmax requires dense f32 buffers",
            )));
        }
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_argmax_f32(
            &mut submission.platform,
            input,
            output,
            row_count,
            column_count,
        )?;
        submission.encoded_operations += 1;
        Ok(())
    }

    /// Encodes one argmax reduction over packed `(value_key, row)` `i32` candidates.
    pub fn encode_argmax_candidates_submission(
        &mut self,
        submission: &mut MetalSubmission,
        input: &MetalBuffer,
        output: &MetalBuffer,
        candidate_count: usize,
    ) -> Result<(), RuntimeError> {
        if input.spec().dtype() != DType::I32 || output.spec().dtype() != DType::I32 {
            return Err(RuntimeError::Backend(String::from(
                "metal argmax candidates requires i32 buffers",
            )));
        }
        if input.spec().storage_size() < candidate_count.saturating_mul(2) {
            return Err(RuntimeError::Backend(format!(
                "metal argmax candidates input is too small: need {} i32 values, actual {}",
                candidate_count.saturating_mul(2),
                input.spec().storage_size()
            )));
        }
        if output.spec().storage_size() < 2 {
            return Err(RuntimeError::Backend(format!(
                "metal argmax candidates output is too small: need 2 i32 values, actual {}",
                output.spec().storage_size()
            )));
        }
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_argmax_candidates(
            &mut submission.platform,
            input,
            output,
            candidate_count,
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

    /// Encodes one grouped ids-enabled quantized expert matvec with the
    /// selected expert ids provided in a device buffer.
    pub fn encode_grouped_quantized_matvec_id_buffer_submission(
        &mut self,
        submission: &mut MetalSubmission,
        weights: &MetalBuffer,
        mode: psionic_core::QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        columns: usize,
        selected_ids: &MetalBuffer,
        selected_count: usize,
        input: &MetalBuffer,
        output: &MetalBuffer,
    ) -> Result<(), RuntimeError> {
        validate_grouped_quantized_matvec_buffer_ids_request(
            weights,
            mode,
            row_stride,
            rows_per_expert,
            columns,
            selected_ids,
            selected_count,
            input,
            output,
        )?;
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend
            .platform
            .encode_grouped_quantized_matvec_buffer_ids(
                &mut submission.platform,
                weights,
                mode,
                row_stride,
                rows_per_expert,
                columns,
                selected_ids,
                selected_count,
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

    /// Encodes one ids-driven grouped expert projection with selected ids
    /// provided by a device buffer.
    pub fn encode_expert_matvec_f32_id_buffer_submission(
        &mut self,
        submission: &mut MetalSubmission,
        weights: &MetalBuffer,
        mode: psionic_core::QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        columns: usize,
        selected_ids: &MetalBuffer,
        selected_count: usize,
        input: &MetalBuffer,
        output: &MetalBuffer,
    ) -> Result<(), RuntimeError> {
        validate_expert_matvec_f32_ids_buffer_request(
            weights,
            mode,
            row_stride,
            rows_per_expert,
            columns,
            selected_ids,
            selected_count,
            input,
            output,
        )?;
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        backend.platform.encode_expert_matvec_f32_ids_buffer(
            &mut submission.platform,
            weights,
            mode,
            row_stride,
            rows_per_expert,
            columns,
            selected_ids,
            selected_count,
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
        kv_cache_encoding_policy: KvCacheEncodingPolicy,
    ) -> Result<MetalKvCacheMirror, RuntimeError> {
        MetalKvCacheMirror::from_host_rows(
            self,
            width,
            max_context_tokens,
            tokens,
            key_values,
            value_values,
            reserve_tokens,
            kv_cache_encoding_policy,
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

    /// Executes one decode-attention step from host-owned `f32` vectors while
    /// still appending into the Metal KV cache mirror.
    #[allow(clippy::too_many_arguments)]
    pub fn decode_attention_values_f32(
        &mut self,
        query_values: &[f32],
        query_head_count: usize,
        key_values: &[f32],
        kv_head_count: usize,
        value_values: &[f32],
        cos_values: &[f32],
        sin_values: &[f32],
        cache: &mut MetalKvCacheMirror,
        scale: f32,
        causal: bool,
        interleaved: bool,
        flash_preferred: bool,
    ) -> Result<MetalDecodeAttentionHostResult, RuntimeError> {
        if query_head_count == 0 || kv_head_count == 0 {
            return Err(RuntimeError::Backend(String::from(
                "metal decode_attention_values_f32 requires non-zero head counts",
            )));
        }
        if query_values.len() % query_head_count != 0 {
            return Err(RuntimeError::Backend(format!(
                "metal decode_attention_values_f32 query width mismatch: values={} query_heads={query_head_count}",
                query_values.len(),
            )));
        }
        let head_dim = query_values.len() / query_head_count;
        if head_dim == 0 {
            return Err(RuntimeError::Backend(String::from(
                "metal decode_attention_values_f32 requires non-zero head width",
            )));
        }
        let expected_kv_values = kv_head_count.checked_mul(head_dim).ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "metal decode_attention_values_f32 kv shape overflow",
            ))
        })?;
        if key_values.len() != expected_kv_values || value_values.len() != expected_kv_values {
            return Err(RuntimeError::Backend(format!(
                "metal decode_attention_values_f32 kv width mismatch: expected {expected_kv_values}, actual key={} value={}",
                key_values.len(),
                value_values.len(),
            )));
        }
        let rotary_pairs = head_dim / 2;
        if cos_values.len() != rotary_pairs || sin_values.len() != rotary_pairs {
            return Err(RuntimeError::Backend(format!(
                "metal decode_attention_values_f32 rope width mismatch: expected {rotary_pairs}, actual cos={} sin={}",
                cos_values.len(),
                sin_values.len(),
            )));
        }

        let query_dims = vec![1, query_head_count, 1, head_dim];
        let key_dims = vec![1, kv_head_count, 1, head_dim];
        let value_dims = vec![1, kv_head_count, 1, head_dim];
        validate_decode_attention_shapes(
            query_dims.as_slice(),
            key_dims.as_slice(),
            value_dims.as_slice(),
            cache.width(),
        )?;
        let cos_dims = vec![1, rotary_pairs];

        let roped_query = apply_rotary_embedding_values(
            query_values,
            query_dims.as_slice(),
            cos_values,
            sin_values,
            cos_dims.as_slice(),
            interleaved,
        )?;
        let roped_key = apply_rotary_embedding_values(
            key_values,
            key_dims.as_slice(),
            cos_values,
            sin_values,
            cos_dims.as_slice(),
            interleaved,
        )?;

        let flattened_key = flatten_decode_heads(roped_key.as_slice(), kv_head_count, head_dim)?;
        let flattened_value = flatten_decode_heads(value_values, kv_head_count, head_dim)?;
        let cache_write_index =
            cache.append_entry(self, flattened_key.as_slice(), flattened_value.as_slice())?;

        let flash_attention_path = flash_preferred && causal && self.supports_flash_attention();
        let output_values = match cache.kv_cache_encoding_policy().family {
            KvCacheEncodingFamily::DenseF32 | KvCacheEncodingFamily::DenseF16Mirror => {
                decode_attention_over_dense_kv_cache_rows(
                    roped_query.as_slice(),
                    cache,
                    query_head_count,
                    kv_head_count,
                    head_dim,
                    scale,
                )?
            }
            KvCacheEncodingFamily::TurboQuant => {
                let (expanded_key, expanded_value) = expand_kv_cache_for_attention(
                    cache,
                    query_head_count,
                    kv_head_count,
                    head_dim,
                )?;
                scaled_dot_product_attention_values(
                    roped_query.as_slice(),
                    expanded_key.as_slice(),
                    expanded_value.as_slice(),
                    query_dims.as_slice(),
                    &[1, query_head_count, cache.len(), head_dim],
                    &[1, query_head_count, cache.len(), head_dim],
                    scale,
                    false,
                    flash_attention_path,
                )?
            }
        };

        Ok(MetalDecodeAttentionHostResult {
            output_values,
            cache_state: cache.state(),
            stats: MetalDecodeAttentionStats {
                flash_attention_path,
                rotary_applied: true,
                used_device_kv: true,
                cache_write_index,
                cached_tokens: cache.len(),
                query_head_count,
                kv_head_count,
            },
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

        let flash_attention_path = flash_preferred && causal && self.supports_flash_attention();
        let output_values = match cache.kv_cache_encoding_policy().family {
            KvCacheEncodingFamily::DenseF32 | KvCacheEncodingFamily::DenseF16Mirror => {
                decode_attention_over_dense_kv_cache_rows(
                    roped_query.as_slice(),
                    cache,
                    query_head_count,
                    kv_head_count,
                    head_dim,
                    scale,
                )?
            }
            KvCacheEncodingFamily::TurboQuant => {
                let (expanded_key, expanded_value) = expand_kv_cache_for_attention(
                    cache,
                    query_head_count,
                    kv_head_count,
                    head_dim,
                )?;
                scaled_dot_product_attention_values(
                    roped_query.as_slice(),
                    expanded_key.as_slice(),
                    expanded_value.as_slice(),
                    query_dims.as_slice(),
                    &[1, query_head_count, cache.len(), head_dim],
                    &[1, query_head_count, cache.len(), head_dim],
                    scale,
                    false,
                    flash_attention_path,
                )?
            }
        };
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
        kv_cache_encoding_policy: KvCacheEncodingPolicy,
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
        let row_byte_len = metal_kv_row_byte_len(width, &kv_cache_encoding_policy)?;
        let mut key_buffer =
            allocate_metal_kv_byte_buffer(backend, capacity_tokens.saturating_mul(row_byte_len))?;
        let mut value_buffer =
            allocate_metal_kv_byte_buffer(backend, capacity_tokens.saturating_mul(row_byte_len))?;
        if tokens > 0 {
            let key_bytes = encode_metal_kv_rows(key_values, width, &kv_cache_encoding_policy)?;
            let value_bytes = encode_metal_kv_rows(value_values, width, &kv_cache_encoding_policy)?;
            key_buffer.write_bytes_at_offset(0, key_bytes.as_slice())?;
            value_buffer.write_bytes_at_offset(0, value_bytes.as_slice())?;
        }
        Ok(Self {
            key_buffer,
            value_buffer,
            width,
            row_byte_len,
            len: tokens,
            capacity_tokens,
            max_context_tokens,
            kv_cache_encoding_policy,
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
        let mut new_keys =
            allocate_metal_kv_byte_buffer(backend, new_capacity.saturating_mul(self.row_byte_len))?;
        let mut new_values =
            allocate_metal_kv_byte_buffer(backend, new_capacity.saturating_mul(self.row_byte_len))?;
        if self.len > 0 {
            let byte_len = self.len.saturating_mul(self.row_byte_len);
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
        let byte_offset = write_index.saturating_mul(self.row_byte_len);
        let key_bytes = encode_metal_kv_rows(key, self.width, &self.kv_cache_encoding_policy)?;
        let value_bytes = encode_metal_kv_rows(value, self.width, &self.kv_cache_encoding_policy)?;
        self.key_buffer
            .write_bytes_at_offset(byte_offset, key_bytes.as_slice())?;
        self.value_buffer
            .write_bytes_at_offset(byte_offset, value_bytes.as_slice())?;
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
        let byte_offset = token_index.saturating_mul(self.row_byte_len);
        let byte_len = self.row_byte_len;
        Ok((
            decode_metal_kv_row(
                self.key_buffer
                    .read_bytes_at_offset(byte_offset, byte_len)?
                    .as_slice(),
                self.width,
                &self.kv_cache_encoding_policy,
            )?,
            decode_metal_kv_row(
                self.value_buffer
                    .read_bytes_at_offset(byte_offset, byte_len)?
                    .as_slice(),
                self.width,
                &self.kv_cache_encoding_policy,
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

    /// Returns the active KV-cache encoding policy for this mirror.
    #[must_use]
    pub fn kv_cache_encoding_policy(&self) -> &KvCacheEncodingPolicy {
        &self.kv_cache_encoding_policy
    }

    /// Returns the logical page layout for this cache.
    #[must_use]
    pub fn page_layout(&self) -> KvCachePageLayout {
        KvCachePageLayout::new(
            self.max_context_tokens,
            4,
            self.row_byte_len.saturating_mul(2),
        )
    }

    /// Returns the current observable KV state.
    #[must_use]
    pub fn state(&self) -> KvCacheState {
        KvCacheState::paged(&self.page_layout(), self.len)
    }
}

fn allocate_metal_kv_byte_buffer(
    backend: &mut MetalBackend,
    byte_len: usize,
) -> Result<MetalBuffer, RuntimeError> {
    let Some(device) = backend
        .selected_device()
        .map(|descriptor| descriptor.device.clone())
    else {
        return Err(RuntimeError::Backend(String::from(
            "metal backend unavailable: no selected execution device",
        )));
    };
    backend.allocate(&TensorSpec::new(
        Shape::new(vec![byte_len]),
        DType::I8,
        device,
    ))
}

fn metal_kv_row_byte_len(
    width: usize,
    kv_cache_encoding_policy: &KvCacheEncodingPolicy,
) -> Result<usize, RuntimeError> {
    match kv_cache_encoding_policy.family {
        KvCacheEncodingFamily::TurboQuant => ggml_q8_1_storage_bytes(width),
        KvCacheEncodingFamily::DenseF32 | KvCacheEncodingFamily::DenseF16Mirror => width
            .checked_mul(std::mem::size_of::<f32>())
            .ok_or_else(|| {
                RuntimeError::Backend(String::from("metal kv row byte length overflow"))
            }),
    }
}

fn ggml_q8_1_storage_bytes(width: usize) -> Result<usize, RuntimeError> {
    if width == 0 || width % GGML_Q8_1_BLOCK_ELEMENTS != 0 {
        return Err(RuntimeError::Backend(format!(
            "metal TurboQuant rows require width divisible by {}, actual {}",
            GGML_Q8_1_BLOCK_ELEMENTS, width
        )));
    }
    width
        .checked_div(GGML_Q8_1_BLOCK_ELEMENTS)
        .and_then(|blocks| blocks.checked_mul(GGML_Q8_1_BLOCK_BYTES))
        .ok_or_else(|| {
            RuntimeError::Backend(String::from("metal TurboQuant row byte length overflow"))
        })
}

fn encode_metal_kv_rows(
    values: &[f32],
    width: usize,
    kv_cache_encoding_policy: &KvCacheEncodingPolicy,
) -> Result<Vec<u8>, RuntimeError> {
    match kv_cache_encoding_policy.family {
        KvCacheEncodingFamily::TurboQuant => f32_slice_to_q8_1_bytes(values, width),
        KvCacheEncodingFamily::DenseF32 | KvCacheEncodingFamily::DenseF16Mirror => {
            Ok(f32_slice_to_bytes(values))
        }
    }
}

fn decode_metal_kv_row(
    bytes: &[u8],
    width: usize,
    kv_cache_encoding_policy: &KvCacheEncodingPolicy,
) -> Result<Vec<f32>, RuntimeError> {
    match kv_cache_encoding_policy.family {
        KvCacheEncodingFamily::TurboQuant => q8_1_bytes_to_f32_vec(bytes, width),
        KvCacheEncodingFamily::DenseF32 | KvCacheEncodingFamily::DenseF16Mirror => {
            bytes_to_f32_vec(bytes)
        }
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

        self.allocate_without_clear(spec)
    }

    fn allocate_for_overwrite(&mut self, spec: &TensorSpec) -> Result<MetalBuffer, RuntimeError> {
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

        if let Some(buffer) = self.pool.take(spec) {
            return Ok(buffer);
        }

        self.allocate_without_clear(spec)
    }

    fn allocate_without_clear(&mut self, spec: &TensorSpec) -> Result<MetalBuffer, RuntimeError> {
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

        let byte_len = spec
            .storage_size()
            .checked_mul(size_of_dtype(spec.dtype()))
            .ok_or_else(|| RuntimeError::Backend(String::from("metal buffer size overflow")))?;
        let storage_mode = self.platform.storage_mode();
        Ok(MetalBuffer {
            spec: spec.clone(),
            byte_offset: 0,
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
        submission.fill_buffer(&buffer.platform, buffer.byte_offset, buffer.byte_len(), 0)?;
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
            TensorData::BF16(_) => Err(RuntimeError::Backend(String::from(
                "metal constant storage does not yet support dense bf16 payloads",
            ))),
            TensorData::I32(_) => Err(RuntimeError::Backend(String::from(
                "metal constant storage does not yet support dense i32 payloads",
            ))),
            TensorData::QuantizedBlocks(data) => {
                validate_quantized_storage(spec, data)?;
                let storage_mode = self.platform.storage_mode();
                let mut buffer = MetalBuffer {
                    spec: spec.clone(),
                    byte_offset: 0,
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
                    byte_offset: 0,
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
            byte_offset: 0,
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

    fn borrowed_dense_input_buffer(
        &mut self,
        values: &[f32],
    ) -> Result<Option<MetalBuffer>, RuntimeError> {
        let storage_mode = self.platform.storage_mode();
        if !matches!(storage_mode, MetalStorageMode::Shared) {
            return Ok(None);
        }
        let byte_len = values
            .len()
            .checked_mul(std::mem::size_of::<f32>())
            .ok_or_else(|| RuntimeError::Backend(String::from("metal input byte size overflow")))?;
        let bytes = unsafe { std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), byte_len) };
        Ok(Some(MetalBuffer {
            spec: TensorSpec::new(
                Shape::new(vec![values.len()]),
                DType::F32,
                self.descriptor.device.clone(),
            ),
            byte_offset: 0,
            byte_len,
            storage_kind: BufferStorageKind::DenseF32,
            storage_mode,
            host_visible: true,
            host_writable: false,
            _keepalive: None,
            platform: self
                .platform
                .buffer_from_bytes_no_copy(bytes, storage_mode)?,
        }))
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
        let mut owned_input_buffer = None;
        let input_buffer = if let Some(buffer) = self.borrowed_dense_input_buffer(input)? {
            buffer
        } else {
            let mut buffer = self.allocate_for_overwrite(&TensorSpec::new(
                Shape::new(vec![columns]),
                DType::F32,
                device.clone(),
            ))?;
            buffer.write_f32(input)?;
            owned_input_buffer = Some(buffer.clone());
            buffer
        };
        let output = self.allocate_for_overwrite(&TensorSpec::new(
            Shape::new(vec![rows]),
            DType::F32,
            device.clone(),
        ))?;
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
        let values = output.read_f32()?;
        if let Some(buffer) = owned_input_buffer {
            self.pool.recycle(buffer);
        }
        self.pool.recycle(output);
        Ok(MetalQuantizedMatvecResult { values })
    }

    fn run_quantized_matvec_batch(
        &mut self,
        requests: &[MetalQuantizedMatvecRequest<'_>],
        input: &[f32],
    ) -> Result<Vec<MetalQuantizedMatvecResult>, RuntimeError> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        let device = self.descriptor.device.clone();
        let mut owned_input_buffer = None;
        let input_buffer = if let Some(buffer) = self.borrowed_dense_input_buffer(input)? {
            buffer
        } else {
            let mut buffer = self.allocate_for_overwrite(&TensorSpec::new(
                Shape::new(vec![input.len()]),
                DType::F32,
                device.clone(),
            ))?;
            buffer.write_f32(input)?;
            owned_input_buffer = Some(buffer.clone());
            buffer
        };

        let mut outputs = Vec::with_capacity(requests.len());
        let mut submission = MetalSubmission {
            encoded_operations: 0,
            synchronized_buffers: 0,
            platform: self
                .platform
                .begin_submission(String::from("psionic.quantized_matvec.batch"))?,
        };

        for request in requests {
            let output = self.allocate_for_overwrite(&TensorSpec::new(
                Shape::new(vec![request.rows]),
                DType::F32,
                device.clone(),
            ))?;
            self.platform.encode_quantized_matvec(
                &mut submission.platform,
                request.weights,
                request.byte_offset,
                request.mode,
                request.rows,
                request.columns,
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
            outputs.push(output);
        }

        submission.commit(MetalCommandWait::Completed)?;
        let mut values = Vec::with_capacity(outputs.len());
        for output in outputs {
            values.push(MetalQuantizedMatvecResult {
                values: output.read_f32()?,
            });
            self.pool.recycle(output);
        }
        if let Some(buffer) = owned_input_buffer {
            self.pool.recycle(buffer);
        }
        Ok(values)
    }

    #[allow(clippy::too_many_arguments)]
    fn run_quantized_gelu_glu_projected(
        &mut self,
        gate_weights: &MetalBuffer,
        gate_mode: QuantizationMode,
        gate_rows: usize,
        gate_columns: usize,
        up_weights: &MetalBuffer,
        up_mode: QuantizationMode,
        up_rows: usize,
        up_columns: usize,
        down_weights: &MetalBuffer,
        down_mode: QuantizationMode,
        down_rows: usize,
        down_columns: usize,
        input: &[f32],
    ) -> Result<Vec<f32>, RuntimeError> {
        if gate_rows == 0 || gate_columns == 0 {
            return Err(RuntimeError::Backend(String::from(
                "metal quantized_gelu_glu_projected requires non-empty gate projection",
            )));
        }
        if gate_rows != up_rows || gate_columns != up_columns {
            return Err(RuntimeError::Backend(format!(
                "metal quantized_gelu_glu_projected gate/up mismatch: gate=({gate_rows},{gate_columns}) up=({up_rows},{up_columns})",
            )));
        }
        if gate_columns != input.len() {
            return Err(RuntimeError::Backend(format!(
                "metal quantized_gelu_glu_projected input width mismatch: expected {}, actual {}",
                gate_columns,
                input.len()
            )));
        }
        if down_columns != gate_rows {
            return Err(RuntimeError::Backend(format!(
                "metal quantized_gelu_glu_projected down projection width mismatch: expected {}, actual {}",
                gate_rows, down_columns
            )));
        }

        let device = self.descriptor.device.clone();
        let mut owned_input_buffer = None;
        let input_buffer = if let Some(buffer) = self.borrowed_dense_input_buffer(input)? {
            buffer
        } else {
            let mut buffer = self.allocate_for_overwrite(&TensorSpec::new(
                Shape::new(vec![input.len()]),
                DType::F32,
                device.clone(),
            ))?;
            buffer.write_f32(input)?;
            owned_input_buffer = Some(buffer.clone());
            buffer
        };

        let gate_output = self.allocate_for_overwrite(&TensorSpec::new(
            Shape::new(vec![gate_rows]),
            DType::F32,
            device.clone(),
        ))?;
        let up_output = self.allocate_for_overwrite(&TensorSpec::new(
            Shape::new(vec![up_rows]),
            DType::F32,
            device.clone(),
        ))?;
        let activated_output = self.allocate_for_overwrite(&TensorSpec::new(
            Shape::new(vec![gate_rows]),
            DType::F32,
            device.clone(),
        ))?;
        let projected_output = self.allocate_for_overwrite(&TensorSpec::new(
            Shape::new(vec![down_rows]),
            DType::F32,
            device.clone(),
        ))?;

        let mut submission = MetalSubmission {
            encoded_operations: 0,
            synchronized_buffers: 0,
            platform: self
                .platform
                .begin_submission(String::from("psionic.quantized_gelu_glu.projected"))?,
        };

        self.platform.encode_quantized_matvec(
            &mut submission.platform,
            gate_weights,
            0,
            gate_mode,
            gate_rows,
            gate_columns,
            &input_buffer,
            &gate_output,
        )?;
        submission.encoded_operations += 1;

        self.platform.encode_quantized_matvec(
            &mut submission.platform,
            up_weights,
            0,
            up_mode,
            up_rows,
            up_columns,
            &input_buffer,
            &up_output,
        )?;
        submission.encoded_operations += 1;

        self.platform.encode_gelu_glu_f32(
            &mut submission.platform,
            &gate_output,
            &up_output,
            &activated_output,
            gate_rows,
        )?;
        submission.encoded_operations += 1;

        self.platform.encode_quantized_matvec(
            &mut submission.platform,
            down_weights,
            0,
            down_mode,
            down_rows,
            down_columns,
            &activated_output,
            &projected_output,
        )?;
        submission.encoded_operations += 1;

        if self
            .platform
            .synchronize_output(&mut submission.platform, &projected_output)?
        {
            submission.synchronized_buffers += 1;
        }
        submission.commit(MetalCommandWait::Completed)?;

        let values = projected_output.read_f32()?;
        self.pool.recycle(gate_output);
        self.pool.recycle(up_output);
        self.pool.recycle(activated_output);
        self.pool.recycle(projected_output);
        if let Some(buffer) = owned_input_buffer {
            self.pool.recycle(buffer);
        }
        Ok(values)
    }

    #[allow(clippy::too_many_arguments)]
    fn run_quantized_gelu_mul_projected(
        &mut self,
        gate_weights: &MetalBuffer,
        gate_mode: QuantizationMode,
        gate_rows: usize,
        gate_columns: usize,
        multiplier: &[f32],
        projection_weights: &MetalBuffer,
        projection_mode: QuantizationMode,
        projection_rows: usize,
        projection_columns: usize,
        input: &[f32],
    ) -> Result<Vec<f32>, RuntimeError> {
        if gate_rows == 0 || gate_columns == 0 {
            return Err(RuntimeError::Backend(String::from(
                "metal quantized_gelu_mul_projected requires non-empty gate projection",
            )));
        }
        if gate_columns != input.len() {
            return Err(RuntimeError::Backend(format!(
                "metal quantized_gelu_mul_projected input width mismatch: expected {}, actual {}",
                gate_columns,
                input.len()
            )));
        }
        if multiplier.len() != gate_rows {
            return Err(RuntimeError::Backend(format!(
                "metal quantized_gelu_mul_projected multiplier width mismatch: expected {}, actual {}",
                gate_rows,
                multiplier.len()
            )));
        }
        if projection_columns != gate_rows {
            return Err(RuntimeError::Backend(format!(
                "metal quantized_gelu_mul_projected projection width mismatch: expected {}, actual {}",
                gate_rows, projection_columns
            )));
        }

        let device = self.descriptor.device.clone();
        let mut owned_input_buffer = None;
        let input_buffer = if let Some(buffer) = self.borrowed_dense_input_buffer(input)? {
            buffer
        } else {
            let mut buffer = self.allocate_for_overwrite(&TensorSpec::new(
                Shape::new(vec![input.len()]),
                DType::F32,
                device.clone(),
            ))?;
            buffer.write_f32(input)?;
            owned_input_buffer = Some(buffer.clone());
            buffer
        };

        let mut multiplier_buffer = self.allocate_for_overwrite(&TensorSpec::new(
            Shape::new(vec![multiplier.len()]),
            DType::F32,
            device.clone(),
        ))?;
        multiplier_buffer.write_f32(multiplier)?;

        let gate_output = self.allocate_for_overwrite(&TensorSpec::new(
            Shape::new(vec![gate_rows]),
            DType::F32,
            device.clone(),
        ))?;
        let activated_output = self.allocate_for_overwrite(&TensorSpec::new(
            Shape::new(vec![gate_rows]),
            DType::F32,
            device.clone(),
        ))?;
        let projected_output = self.allocate_for_overwrite(&TensorSpec::new(
            Shape::new(vec![projection_rows]),
            DType::F32,
            device.clone(),
        ))?;

        let mut submission = MetalSubmission {
            encoded_operations: 0,
            synchronized_buffers: 0,
            platform: self
                .platform
                .begin_submission(String::from("psionic.quantized_gelu_mul.projected"))?,
        };

        self.platform.encode_quantized_matvec(
            &mut submission.platform,
            gate_weights,
            0,
            gate_mode,
            gate_rows,
            gate_columns,
            &input_buffer,
            &gate_output,
        )?;
        submission.encoded_operations += 1;

        self.platform.encode_gelu_glu_f32(
            &mut submission.platform,
            &gate_output,
            &multiplier_buffer,
            &activated_output,
            gate_rows,
        )?;
        submission.encoded_operations += 1;

        self.platform.encode_quantized_matvec(
            &mut submission.platform,
            projection_weights,
            0,
            projection_mode,
            projection_rows,
            projection_columns,
            &activated_output,
            &projected_output,
        )?;
        submission.encoded_operations += 1;

        if self
            .platform
            .synchronize_output(&mut submission.platform, &projected_output)?
        {
            submission.synchronized_buffers += 1;
        }
        submission.commit(MetalCommandWait::Completed)?;

        let values = projected_output.read_f32()?;
        self.pool.recycle(multiplier_buffer);
        self.pool.recycle(gate_output);
        self.pool.recycle(activated_output);
        self.pool.recycle(projected_output);
        if let Some(buffer) = owned_input_buffer {
            self.pool.recycle(buffer);
        }
        Ok(values)
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
        let mut owned_input_buffer = None;
        let input_buffer = if let Some(buffer) = self.borrowed_dense_input_buffer(input)? {
            buffer
        } else {
            let mut buffer = self.allocate_for_overwrite(&TensorSpec::new(
                Shape::new(vec![columns]),
                DType::F32,
                device.clone(),
            ))?;
            buffer.write_f32(input)?;
            owned_input_buffer = Some(buffer.clone());
            buffer
        };
        match output_mode {
            MetalLogitsOutputMode::GreedyToken => {
                let output = self.allocate_for_overwrite(&TensorSpec::new(
                    Shape::new(vec![rows]),
                    DType::F32,
                    device.clone(),
                ))?;
                let selected = self.allocate_for_overwrite(&TensorSpec::new(
                    Shape::new(vec![1]),
                    DType::F32,
                    device,
                ))?;
                let mut submission = MetalSubmission {
                    encoded_operations: 0,
                    synchronized_buffers: 0,
                    platform: self
                        .platform
                        .begin_submission(String::from("psionic.quantized_matvec.greedy"))?,
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
                self.platform.encode_argmax_f32(
                    &mut submission.platform,
                    &output,
                    &selected,
                    1,
                    rows,
                )?;
                submission.encoded_operations += 1;
                if self
                    .platform
                    .synchronize_output(&mut submission.platform, &selected)?
                {
                    submission.synchronized_buffers += 1;
                }
                submission.commit(MetalCommandWait::Completed)?;
                let selected_tokens =
                    read_argmax_indices_from_f32_buffer(&selected, 1, "metal quantized argmax")?;
                if let Some(buffer) = owned_input_buffer {
                    self.pool.recycle(buffer);
                }
                self.pool.recycle(output);
                self.pool.recycle(selected);
                Ok(MetalLogitsSelectionResult {
                    selected_tokens,
                    candidates: None,
                    logits: None,
                    metrics: MetalLogitsSelectionMetrics {
                        output_mode,
                        readback_bytes: std::mem::size_of::<f32>().try_into().unwrap_or(u64::MAX),
                        raw_logits_materialized: false,
                    },
                })
            }
            MetalLogitsOutputMode::TopKCandidates(top_k) => {
                let output = self.allocate_for_overwrite(&TensorSpec::new(
                    Shape::new(vec![rows]),
                    DType::F32,
                    device.clone(),
                ))?;
                let use_device_top_k = top_k <= METAL_SINGLE_ROW_TOP_K_MAX;
                let selected_ids = use_device_top_k.then(|| {
                    self.allocate_for_overwrite(&TensorSpec::new(
                        Shape::new(vec![top_k]),
                        DType::I32,
                        device.clone(),
                    ))
                });
                let selected_values = use_device_top_k.then(|| {
                    self.allocate_for_overwrite(&TensorSpec::new(
                        Shape::new(vec![top_k]),
                        DType::F32,
                        device.clone(),
                    ))
                });
                let selected_ids = selected_ids.transpose()?;
                let selected_values = selected_values.transpose()?;
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
                if let (Some(selected_ids), Some(selected_values)) =
                    (selected_ids.as_ref(), selected_values.as_ref())
                {
                    self.platform.encode_top_k_single_row_f32(
                        &mut submission.platform,
                        &output,
                        selected_ids,
                        selected_values,
                        rows,
                        top_k,
                    )?;
                    submission.encoded_operations += 1;
                    if self
                        .platform
                        .synchronize_output(&mut submission.platform, selected_ids)?
                    {
                        submission.synchronized_buffers += 1;
                    }
                    if self
                        .platform
                        .synchronize_output(&mut submission.platform, selected_values)?
                    {
                        submission.synchronized_buffers += 1;
                    }
                } else if self
                    .platform
                    .synchronize_output(&mut submission.platform, &output)?
                {
                    submission.synchronized_buffers += 1;
                }
                submission.commit(MetalCommandWait::Completed)?;
                let candidates = if let (Some(selected_ids), Some(selected_values)) =
                    (selected_ids.as_ref(), selected_values.as_ref())
                {
                    read_top_k_result(selected_ids, selected_values, top_k, "metal top_k")?
                } else {
                    top_k_dense_rows(&output, 1, rows, top_k, "metal top_k")?
                };
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
                if let Some(buffer) = owned_input_buffer {
                    self.pool.recycle(buffer);
                }
                self.pool.recycle(output);
                if let Some(buffer) = selected_ids {
                    self.pool.recycle(buffer);
                }
                if let Some(buffer) = selected_values {
                    self.pool.recycle(buffer);
                }
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
                let output = self.allocate_for_overwrite(&TensorSpec::new(
                    Shape::new(vec![rows]),
                    DType::F32,
                    device.clone(),
                ))?;
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
                let logits = output.read_f32()?;
                let selected_tokens =
                    argmax_values(logits.as_slice(), 1, rows, "metal raw logits")?;
                if let Some(buffer) = owned_input_buffer {
                    self.pool.recycle(buffer);
                }
                self.pool.recycle(output);
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

    fn run_argmax_f32(
        &mut self,
        input: &MetalBuffer,
        row_count: usize,
        column_count: usize,
    ) -> Result<Vec<u32>, RuntimeError> {
        validate_dense_row_selection(input, row_count, column_count, "metal argmax")?;
        let device = self.descriptor.device.clone();
        let output = self.allocate_for_overwrite(&TensorSpec::new(
            Shape::new(vec![row_count]),
            DType::F32,
            device,
        ))?;
        let mut submission = MetalSubmission {
            encoded_operations: 0,
            synchronized_buffers: 0,
            platform: self
                .platform
                .begin_submission(String::from("psionic.argmax_f32"))?,
        };
        self.platform.encode_argmax_f32(
            &mut submission.platform,
            input,
            &output,
            row_count,
            column_count,
        )?;
        submission.encoded_operations += 1;
        if self
            .platform
            .synchronize_output(&mut submission.platform, &output)?
        {
            submission.synchronized_buffers += 1;
        }
        submission.commit(MetalCommandWait::Completed)?;
        let indices = read_argmax_indices_from_f32_buffer(&output, row_count, "metal argmax")?;
        self.pool.recycle(output);
        Ok(indices)
    }

    fn run_top_k_single_row_f32(
        &mut self,
        input: &MetalBuffer,
        column_count: usize,
        top_k: usize,
    ) -> Result<MetalTopKResult, RuntimeError> {
        let selected_ids = self.allocate_for_overwrite(&TensorSpec::new(
            Shape::new(vec![top_k]),
            DType::I32,
            self.descriptor.device.clone(),
        ))?;
        let selected_values = self.allocate_for_overwrite(&TensorSpec::new(
            Shape::new(vec![top_k]),
            DType::F32,
            self.descriptor.device.clone(),
        ))?;
        validate_top_k_single_row_request(
            input,
            column_count,
            top_k,
            &selected_ids,
            &selected_values,
        )?;
        let mut submission = MetalSubmission {
            encoded_operations: 0,
            synchronized_buffers: 0,
            platform: self
                .platform
                .begin_submission(String::from("psionic.top_k_single_row"))?,
        };
        self.platform.encode_top_k_single_row_f32(
            &mut submission.platform,
            input,
            &selected_ids,
            &selected_values,
            column_count,
            top_k,
        )?;
        submission.encoded_operations += 1;
        if self
            .platform
            .synchronize_output(&mut submission.platform, &selected_ids)?
        {
            submission.synchronized_buffers += 1;
        }
        if self
            .platform
            .synchronize_output(&mut submission.platform, &selected_values)?
        {
            submission.synchronized_buffers += 1;
        }
        submission.commit(MetalCommandWait::Completed)?;
        let result = read_top_k_result(&selected_ids, &selected_values, top_k, "metal top_k")?;
        self.pool.recycle(selected_ids);
        self.pool.recycle(selected_values);
        Ok(result)
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
        let output = self.allocate_for_overwrite(&TensorSpec::new(
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
        let values = output.read_f32()?;
        self.pool.recycle(output);
        Ok(values)
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
            TensorData::BF16(_) => {
                return Err(RuntimeError::Backend(format!(
                    "metal constant {} does not support dense bf16 payloads",
                    step.output
                )));
            }
            TensorData::I32(_) => {
                return Err(RuntimeError::Backend(format!(
                    "metal constant {} does not support dense i32 payloads",
                    step.output
                )));
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

fn read_argmax_indices_from_f32_buffer(
    input: &MetalBuffer,
    row_count: usize,
    label: &str,
) -> Result<Vec<u32>, RuntimeError> {
    if input.storage_kind() != BufferStorageKind::DenseF32 || input.spec().dtype() != DType::F32 {
        return Err(RuntimeError::Backend(format!(
            "{label} requires dense f32 storage, actual {:?}",
            input.storage_kind()
        )));
    }
    if input.spec().storage_size() != row_count {
        return Err(RuntimeError::Backend(format!(
            "{label} row-count mismatch: expected {row_count}, actual {}",
            input.spec().storage_size()
        )));
    }
    let values = input.read_f32()?;
    let mut indices = Vec::with_capacity(values.len());
    for value in values {
        if !value.is_finite() || value < 0.0 {
            return Err(RuntimeError::Backend(format!(
                "{label} produced invalid argmax index {value}",
            )));
        }
        indices.push(value as u32);
    }
    Ok(indices)
}

/// Reads one argmax index from a Metal `i32[2]` candidate buffer.
pub fn read_argmax_candidate_index(input: &MetalBuffer, label: &str) -> Result<u32, RuntimeError> {
    if input.spec().dtype() != DType::I32 || input.spec().storage_size() != 2 {
        return Err(RuntimeError::Backend(format!(
            "{label} requires an i32[2] candidate buffer, actual {:?} with {} elements",
            input.spec().dtype(),
            input.spec().storage_size()
        )));
    }
    let bytes = input.read_bytes()?;
    if bytes.len() != std::mem::size_of::<u32>() * 2 {
        return Err(RuntimeError::Backend(format!(
            "{label} candidate buffer size mismatch: expected {}, actual {}",
            std::mem::size_of::<u32>() * 2,
            bytes.len()
        )));
    }
    let first = u32::from_ne_bytes(bytes[0..4].try_into().map_err(|_| {
        RuntimeError::Backend(String::from(
            "metal argmax candidate result was missing the first u32",
        ))
    })?);
    let second = u32::from_ne_bytes(bytes[4..8].try_into().map_err(|_| {
        RuntimeError::Backend(String::from(
            "metal argmax candidate result was missing the second u32",
        ))
    })?);
    let _ = first;
    Ok(second)
}

/// Reads one single-row top-k result from Metal `i32[top_k]` / `f32[top_k]` buffers.
pub fn read_top_k_result(
    selected_ids: &MetalBuffer,
    selected_values: &MetalBuffer,
    top_k: usize,
    label: &str,
) -> Result<MetalTopKResult, RuntimeError> {
    if selected_ids.spec().dtype() != DType::I32 || selected_ids.spec().storage_size() < top_k {
        return Err(RuntimeError::Backend(format!(
            "{label} requires an i32[{top_k}] id buffer, actual {:?} with {} elements",
            selected_ids.spec().dtype(),
            selected_ids.spec().storage_size()
        )));
    }
    if selected_values.spec().dtype() != DType::F32 || selected_values.spec().storage_size() < top_k
    {
        return Err(RuntimeError::Backend(format!(
            "{label} requires an f32[{top_k}] value buffer, actual {:?} with {} elements",
            selected_values.spec().dtype(),
            selected_values.spec().storage_size()
        )));
    }
    let index_bytes = selected_ids.read_bytes()?;
    let values = selected_values.read_f32()?;
    let available_indices = index_bytes.len() / std::mem::size_of::<i32>();
    if available_indices < top_k || values.len() < top_k {
        return Err(RuntimeError::Backend(format!(
            "{label} top-k buffer size mismatch: need {top_k}, actual ids={} values={}",
            available_indices,
            values.len()
        )));
    }
    let mut parsed_indices = Vec::with_capacity(top_k);
    let mut parsed_values = Vec::with_capacity(top_k);
    for slot in 0..top_k {
        let byte_offset = slot.saturating_mul(std::mem::size_of::<i32>());
        let index = i32::from_ne_bytes(
            index_bytes[byte_offset..byte_offset + std::mem::size_of::<i32>()]
                .try_into()
                .map_err(|_| {
                    RuntimeError::Backend(format!(
                        "{label} top-k id buffer was truncated at slot {slot}",
                    ))
                })?,
        );
        let value = values[slot];
        if index < 0 {
            return Err(RuntimeError::Backend(format!(
                "{label} produced invalid top-k index {index}",
            )));
        }
        parsed_indices.push(index as u32);
        parsed_values.push(value);
    }
    Ok(MetalTopKResult {
        row_count: 1,
        top_k,
        indices: parsed_indices,
        values: parsed_values,
    })
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

fn decode_attention_over_dense_kv_cache_rows(
    query: &[f32],
    cache: &MetalKvCacheMirror,
    query_head_count: usize,
    kv_head_count: usize,
    head_dim: usize,
    scale: f32,
) -> Result<Vec<f32>, RuntimeError> {
    let token_count = cache.len();
    let row_width = cache.width();
    let expected_row_width = kv_head_count.saturating_mul(head_dim);
    if row_width != expected_row_width {
        return Err(RuntimeError::Backend(format!(
            "metal dense decode attention cache row width mismatch: expected {}, actual {}",
            expected_row_width, row_width
        )));
    }
    if query.len() != query_head_count.saturating_mul(head_dim) {
        return Err(RuntimeError::Backend(format!(
            "metal dense decode attention query length mismatch: expected {}, actual {}",
            query_head_count.saturating_mul(head_dim),
            query.len()
        )));
    }
    if token_count == 0 {
        return Ok(vec![0.0; query_head_count.saturating_mul(head_dim)]);
    }

    let byte_len = token_count.saturating_mul(cache.row_byte_len);
    cache
        .key_buffer
        .with_bytes_at_offset(0, byte_len, |key_bytes| {
            let key_rows = dense_kv_bytes_as_f32_cow(key_bytes)?;
            cache
                .value_buffer
                .with_bytes_at_offset(0, byte_len, |value_bytes| {
                    let value_rows = dense_kv_bytes_as_f32_cow(value_bytes)?;
                    dense_decode_attention_from_row_major_cache(
                        query,
                        key_rows.as_ref(),
                        value_rows.as_ref(),
                        query_head_count,
                        kv_head_count,
                        head_dim,
                        scale,
                    )
                })
        })
}

fn dense_kv_bytes_as_f32_cow(bytes: &[u8]) -> Result<Cow<'_, [f32]>, RuntimeError> {
    if bytes.len() % size_of::<f32>() != 0 {
        return Err(RuntimeError::Backend(format!(
            "metal dense kv cache bytes require 4-byte alignment, actual {}",
            bytes.len()
        )));
    }
    let (prefix, aligned, suffix) = unsafe { bytes.align_to::<f32>() };
    if prefix.is_empty() && suffix.is_empty() {
        Ok(Cow::Borrowed(aligned))
    } else {
        Ok(Cow::Owned(bytes_to_f32_vec(bytes)?))
    }
}

fn dense_decode_attention_from_row_major_cache(
    query: &[f32],
    key_rows: &[f32],
    value_rows: &[f32],
    query_head_count: usize,
    kv_head_count: usize,
    head_dim: usize,
    scale: f32,
) -> Result<Vec<f32>, RuntimeError> {
    let token_count = key_rows.len() / kv_head_count.saturating_mul(head_dim).max(1);
    let row_width = kv_head_count.saturating_mul(head_dim);
    if key_rows.len() != token_count.saturating_mul(row_width)
        || value_rows.len() != token_count.saturating_mul(row_width)
    {
        return Err(RuntimeError::Backend(String::from(
            "metal dense decode attention row-major cache length mismatch",
        )));
    }
    if kv_head_count == 0 {
        return Err(RuntimeError::Backend(String::from(
            "metal dense decode attention requires at least one kv head",
        )));
    }

    let mut output = vec![0.0; query_head_count.saturating_mul(head_dim)];
    for head_index in 0..query_head_count {
        let query_start = head_index.saturating_mul(head_dim);
        let query_end = query_start + head_dim;
        dense_decode_attention_head_into(
            &mut output[query_start..query_end],
            &query[query_start..query_end],
            key_rows,
            value_rows,
            query_head_count,
            kv_head_count,
            token_count,
            row_width,
            head_dim,
            scale,
            head_index,
        )?;
    }
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn dense_decode_attention_head_into(
    output_slice: &mut [f32],
    query: &[f32],
    key_rows: &[f32],
    value_rows: &[f32],
    query_head_count: usize,
    kv_head_count: usize,
    token_count: usize,
    row_width: usize,
    head_dim: usize,
    scale: f32,
    head_index: usize,
) -> Result<(), RuntimeError> {
    let group_size = query_head_count / kv_head_count.max(1);
    let kv_head_index = if group_size == 0 {
        0
    } else {
        head_index / group_size
    }
    .min(kv_head_count.saturating_sub(1));
    let mut max_logit = f32::NEG_INFINITY;
    let mut denom = 0.0f32;
    for token_index in 0..token_count {
        let row_start =
            token_index.saturating_mul(row_width) + kv_head_index.saturating_mul(head_dim);
        let row_end = row_start + head_dim;
        let key = &key_rows[row_start..row_end];
        let value = &value_rows[row_start..row_end];
        let logit = dense_row_dot(query, key)? * scale;
        accumulate_dense_online_softmax_value(
            output_slice,
            value,
            logit,
            &mut max_logit,
            &mut denom,
        )?;
    }
    let normalizer = 1.0 / denom.max(f32::EPSILON);
    for value in output_slice.iter_mut() {
        *value *= normalizer;
    }
    Ok(())
}

fn accumulate_dense_online_softmax_value(
    output: &mut [f32],
    value: &[f32],
    logit: f32,
    max_logit: &mut f32,
    denom: &mut f32,
) -> Result<(), RuntimeError> {
    if output.len() != value.len() {
        return Err(RuntimeError::Backend(format!(
            "metal dense decode attention value width mismatch: output {} value {}",
            output.len(),
            value.len()
        )));
    }
    let next_max = (*max_logit).max(logit);
    let rescale = if *denom == 0.0 {
        0.0
    } else {
        (*max_logit - next_max).exp()
    };
    let weight = (logit - next_max).exp();
    for (output_value, value_value) in output.iter_mut().zip(value.iter()) {
        *output_value = *output_value * rescale + *value_value * weight;
    }
    *denom = *denom * rescale + weight;
    *max_logit = next_max;
    Ok(())
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
        psionic_core::QuantizationMode::GgmlQ4K
        | psionic_core::QuantizationMode::GgmlQ5_0
        | psionic_core::QuantizationMode::GgmlQ8_0
        | psionic_core::QuantizationMode::GgmlMxfp4 => {
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

fn validate_quantized_matvec_argmax_request(
    weights: &MetalBuffer,
    byte_offset: usize,
    mode: psionic_core::QuantizationMode,
    rows: usize,
    columns: usize,
    input: &MetalBuffer,
    selected: &MetalBuffer,
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
                "metal quantized matvec argmax mode mismatch: requested {mode:?}, stored {stored_mode:?}",
            )));
        }
        storage_kind => {
            return Err(RuntimeError::Backend(format!(
                "metal quantized matvec argmax requires quantized block storage, actual {:?}",
                storage_kind
            )));
        }
    }
    if weights.byte_len() < end_offset {
        return Err(RuntimeError::Backend(format!(
            "metal quantized matvec argmax byte length mismatch: required {end_offset}, actual {}",
            weights.byte_len(),
        )));
    }
    if input.storage_kind() != BufferStorageKind::DenseF32 || input.spec().dtype() != DType::F32 {
        return Err(RuntimeError::Backend(format!(
            "metal quantized matvec argmax input requires dense f32 storage, actual {:?}",
            input.storage_kind()
        )));
    }
    if input.spec().storage_size() < columns {
        return Err(RuntimeError::Backend(format!(
            "metal quantized matvec argmax input width mismatch: required at least {columns}, actual {}",
            input.spec().storage_size()
        )));
    }
    let candidate_count = quantized_argmax_candidate_count(rows);
    let required_elements = candidate_count.saturating_mul(2);
    if selected.spec().dtype() != DType::I32 || selected.spec().storage_size() < required_elements {
        return Err(RuntimeError::Backend(format!(
            "metal quantized matvec argmax selected buffer requires at least {required_elements} i32 elements, actual {:?} with {} elements",
            selected.spec().dtype(),
            selected.spec().storage_size()
        )));
    }
    let required_bytes = required_elements.saturating_mul(std::mem::size_of::<u32>());
    if selected.byte_len() < required_bytes {
        return Err(RuntimeError::Backend(format!(
            "metal quantized matvec argmax selected buffer must be at least {required_bytes} bytes, actual {}",
            selected.byte_len()
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

fn validate_grouped_quantized_matvec_buffer_ids_request(
    weights: &MetalBuffer,
    mode: psionic_core::QuantizationMode,
    row_stride: usize,
    rows_per_expert: usize,
    columns: usize,
    selected_ids: &MetalBuffer,
    selected_count: usize,
    input: &MetalBuffer,
    output: &MetalBuffer,
) -> Result<(), RuntimeError> {
    validate_grouped_expert_layout(weights, mode, row_stride, rows_per_expert, columns)?;
    let total_rows = rows_per_expert.saturating_mul(selected_count);
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
    if selected_ids.spec().dtype() != DType::I32
        || selected_ids.spec().storage_size() < selected_count
    {
        return Err(RuntimeError::Backend(format!(
            "metal grouped matvec selected-id buffer is too small: need {selected_count} i32 values, actual {:?} with {} elements",
            selected_ids.spec().dtype(),
            selected_ids.spec().storage_size()
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

fn validate_expert_matvec_f32_ids_buffer_request(
    weights: &MetalBuffer,
    mode: psionic_core::QuantizationMode,
    row_stride: usize,
    rows_per_expert: usize,
    columns: usize,
    selected_ids: &MetalBuffer,
    selected_count: usize,
    input: &MetalBuffer,
    output: &MetalBuffer,
) -> Result<(), RuntimeError> {
    validate_grouped_expert_layout(weights, mode, row_stride, rows_per_expert, columns)?;
    let total_rows = rows_per_expert.saturating_mul(selected_count);
    let total_inputs = columns.saturating_mul(selected_count);
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
    if selected_ids.spec().dtype() != DType::I32
        || selected_ids.spec().storage_size() < selected_count
    {
        return Err(RuntimeError::Backend(format!(
            "metal expert_matvec_f32_ids selected-id buffer is too small: need {selected_count} i32 values, actual {:?} with {} elements",
            selected_ids.spec().dtype(),
            selected_ids.spec().storage_size()
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
    Ok(())
}

fn validate_top_k_softmax_single_row_request(
    input: &MetalBuffer,
    column_count: usize,
    top_k: usize,
    selected_ids: &MetalBuffer,
    selected_weights: &MetalBuffer,
) -> Result<(), RuntimeError> {
    validate_top_k_single_row_request_impl(
        input,
        column_count,
        top_k,
        selected_ids,
        selected_weights,
        "metal top-k softmax",
    )
}

fn validate_top_k_single_row_request(
    input: &MetalBuffer,
    column_count: usize,
    top_k: usize,
    selected_ids: &MetalBuffer,
    selected_values: &MetalBuffer,
) -> Result<(), RuntimeError> {
    validate_top_k_single_row_request_impl(
        input,
        column_count,
        top_k,
        selected_ids,
        selected_values,
        "metal top-k",
    )
}

fn validate_top_k_single_row_request_impl(
    input: &MetalBuffer,
    column_count: usize,
    top_k: usize,
    selected_ids: &MetalBuffer,
    selected_values: &MetalBuffer,
    label: &str,
) -> Result<(), RuntimeError> {
    if input.storage_kind() != BufferStorageKind::DenseF32 || input.spec().dtype() != DType::F32 {
        return Err(RuntimeError::Backend(format!(
            "{label} input requires dense f32 storage, actual {:?}",
            input.storage_kind()
        )));
    }
    if input.spec().storage_size() < column_count {
        return Err(RuntimeError::Backend(format!(
            "{label} input width mismatch: required at least {column_count}, actual {}",
            input.spec().storage_size()
        )));
    }
    if top_k == 0 || top_k > METAL_SINGLE_ROW_TOP_K_MAX {
        return Err(RuntimeError::Backend(format!(
            "{label} supports 1..={METAL_SINGLE_ROW_TOP_K_MAX} selections, actual {top_k}",
        )));
    }
    if selected_ids.spec().dtype() != DType::I32 || selected_ids.spec().storage_size() < top_k {
        return Err(RuntimeError::Backend(format!(
            "{label} selected-id buffer is too small: need {top_k} i32 values, actual {:?} with {} elements",
            selected_ids.spec().dtype(),
            selected_ids.spec().storage_size()
        )));
    }
    if selected_values.spec().dtype() != DType::F32 || selected_values.spec().storage_size() < top_k
    {
        return Err(RuntimeError::Backend(format!(
            "{label} selected-value buffer is too small: need {top_k} f32 values, actual {:?} with {} elements",
            selected_values.spec().dtype(),
            selected_values.spec().storage_size()
        )));
    }
    Ok(())
}

fn validate_add_selected_expert_bias_request(
    values: &MetalBuffer,
    bias: &MetalBuffer,
    selected_ids: &MetalBuffer,
    selected_count: usize,
    rows_per_expert: usize,
) -> Result<(), RuntimeError> {
    let total_rows = selected_count.saturating_mul(rows_per_expert);
    if values.spec().dtype() != DType::F32 || bias.spec().dtype() != DType::F32 {
        return Err(RuntimeError::Backend(String::from(
            "metal add-selected-expert-bias requires dense f32 buffers",
        )));
    }
    if selected_ids.spec().dtype() != DType::I32
        || selected_ids.spec().storage_size() < selected_count
    {
        return Err(RuntimeError::Backend(format!(
            "metal add-selected-expert-bias selected-id buffer is too small: need {selected_count} i32 values, actual {:?} with {} elements",
            selected_ids.spec().dtype(),
            selected_ids.spec().storage_size()
        )));
    }
    if values.spec().storage_size() < total_rows {
        return Err(RuntimeError::Backend(format!(
            "metal add-selected-expert-bias output is too small: need {total_rows}, actual {}",
            values.spec().storage_size()
        )));
    }
    if bias.spec().storage_size() < rows_per_expert {
        return Err(RuntimeError::Backend(format!(
            "metal add-selected-expert-bias bias buffer is too small for one expert row set: need at least {rows_per_expert}, actual {}",
            bias.spec().storage_size()
        )));
    }
    Ok(())
}

fn validate_aggregate_selected_expert_rows_request(
    input: &MetalBuffer,
    selected_ids: &MetalBuffer,
    selected_weights: &MetalBuffer,
    expert_scales: &MetalBuffer,
    output: &MetalBuffer,
    selected_count: usize,
    rows_per_expert: usize,
) -> Result<(), RuntimeError> {
    let total_rows = selected_count.saturating_mul(rows_per_expert);
    if input.spec().dtype() != DType::F32
        || selected_weights.spec().dtype() != DType::F32
        || expert_scales.spec().dtype() != DType::F32
        || output.spec().dtype() != DType::F32
    {
        return Err(RuntimeError::Backend(String::from(
            "metal aggregate-selected-expert-rows requires dense f32 input, weight, scale, and output buffers",
        )));
    }
    if selected_ids.spec().dtype() != DType::I32
        || selected_ids.spec().storage_size() < selected_count
    {
        return Err(RuntimeError::Backend(format!(
            "metal aggregate-selected-expert-rows selected-id buffer is too small: need {selected_count} i32 values, actual {:?} with {} elements",
            selected_ids.spec().dtype(),
            selected_ids.spec().storage_size()
        )));
    }
    if input.spec().storage_size() < total_rows {
        return Err(RuntimeError::Backend(format!(
            "metal aggregate-selected-expert-rows input is too small: need {total_rows}, actual {}",
            input.spec().storage_size()
        )));
    }
    if selected_weights.spec().storage_size() < selected_count {
        return Err(RuntimeError::Backend(format!(
            "metal aggregate-selected-expert-rows selected-weight buffer is too small: need {selected_count}, actual {}",
            selected_weights.spec().storage_size()
        )));
    }
    if output.spec().storage_size() < rows_per_expert {
        return Err(RuntimeError::Backend(format!(
            "metal aggregate-selected-expert-rows output is too small: need {rows_per_expert}, actual {}",
            output.spec().storage_size()
        )));
    }
    if expert_scales.spec().storage_size() == 0 {
        return Err(RuntimeError::Backend(String::from(
            "metal aggregate-selected-expert-rows requires a non-empty expert-scale buffer",
        )));
    }
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

fn f32_slice_to_q8_1_bytes(values: &[f32], width: usize) -> Result<Vec<u8>, RuntimeError> {
    if width == 0 || values.len() % width != 0 {
        return Err(RuntimeError::Backend(format!(
            "metal q8_1 row encode requires a positive row width, got width={} values={}",
            width,
            values.len()
        )));
    }
    let row_byte_len = ggml_q8_1_storage_bytes(width)?;
    let mut bytes = Vec::with_capacity(values.len() / width * row_byte_len);
    for row in values.chunks_exact(width) {
        for block in row.chunks_exact(GGML_Q8_1_BLOCK_ELEMENTS) {
            let mut max_abs = 0.0_f32;
            let mut sum = 0.0_f32;
            for &value in block {
                max_abs = max_abs.max(value.abs());
                sum += value;
            }
            let scale = if max_abs == 0.0 { 0.0 } else { max_abs / 127.0 };
            bytes.extend_from_slice(&f32_to_f16_bits(scale).to_le_bytes());
            bytes.extend_from_slice(&f32_to_f16_bits(sum).to_le_bytes());
            for &value in block {
                let quantized = if scale == 0.0 {
                    0.0
                } else {
                    (value / scale).round().clamp(-127.0, 127.0)
                };
                bytes.push((quantized as i8) as u8);
            }
        }
    }
    Ok(bytes)
}

fn q8_1_bytes_to_f32_vec(bytes: &[u8], width: usize) -> Result<Vec<f32>, RuntimeError> {
    let expected = ggml_q8_1_storage_bytes(width)?;
    if bytes.len() != expected {
        return Err(RuntimeError::Backend(format!(
            "invalid q8_1 byte length {}, expected {}",
            bytes.len(),
            expected,
        )));
    }
    let mut values = Vec::with_capacity(width);
    for block in bytes.chunks_exact(GGML_Q8_1_BLOCK_BYTES) {
        let scale = f16_bits_to_f32(u16::from_le_bytes([block[0], block[1]]));
        for &quantized in &block[4..4 + GGML_Q8_1_BLOCK_ELEMENTS] {
            values.push((quantized as i8) as f32 * scale);
        }
    }
    Ok(values)
}

fn f32_to_f16_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let mantissa = bits & 0x007f_ffff;

    if exponent == 0xff {
        if mantissa == 0 {
            return sign | 0x7c00;
        }
        return sign | 0x7c00 | ((mantissa >> 13) as u16) | 1;
    }

    let half_exponent = exponent - 127 + 15;
    if half_exponent >= 0x1f {
        return sign | 0x7c00;
    }

    if half_exponent <= 0 {
        if half_exponent < -10 {
            return sign;
        }
        let mantissa = mantissa | 0x0080_0000;
        let shift = (14 - half_exponent) as u32;
        let rounded = mantissa + (1 << (shift - 1));
        return sign | ((rounded >> shift) as u16);
    }

    let rounded_mantissa = mantissa + 0x0000_1000;
    if rounded_mantissa & 0x0080_0000 != 0 {
        let adjusted_exponent = half_exponent + 1;
        if adjusted_exponent >= 0x1f {
            return sign | 0x7c00;
        }
        return sign | ((adjusted_exponent as u16) << 10);
    }

    sign | ((half_exponent as u16) << 10) | ((rounded_mantissa >> 13) as u16)
}

fn f16_bits_to_f32(bits: u16) -> f32 {
    let sign = u32::from(bits & 0x8000) << 16;
    let exponent = (bits >> 10) & 0x1f;
    let mantissa = u32::from(bits & 0x03ff);

    let f32_bits = if exponent == 0 {
        if mantissa == 0 {
            sign
        } else {
            let mut mantissa = mantissa;
            let mut exponent = -14_i32;
            while mantissa & 0x0400 == 0 {
                mantissa <<= 1;
                exponent -= 1;
            }
            mantissa &= 0x03ff;
            sign | (((exponent + 127) as u32) << 23) | (mantissa << 13)
        }
    } else if exponent == 0x1f {
        sign | 0x7f80_0000 | (mantissa << 13)
    } else {
        sign | (((i32::from(exponent) - 15 + 127) as u32) << 23) | (mantissa << 13)
    };

    f32::from_bits(f32_bits)
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
        tenant_id: compatibility.tenant_id.clone(),
        sampler_digest: compatibility.sampler_digest.clone(),
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
    use std::{borrow::ToOwned, ptr};

    use metal::{
        Buffer, CommandBuffer, CommandQueue, CompileOptions, ComputeCommandEncoder,
        ComputePipelineState, Device as MetalDevice, DeviceRef as MetalDeviceRef,
        MTLCommandBufferStatus, MTLDeviceLocation, MTLGPUFamily, MTLResourceOptions, MTLSize,
        NSRange,
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
        MetalDiscoveryReport, MetalKernelCache, MetalKvCacheMirror, MetalStorageMode,
        classify_support, quantized_row_stride,
    };

    #[derive(Clone)]
    pub(super) struct PlatformBuffer {
        raw: Buffer,
    }

    struct DensePipelines {
        add: ComputePipelineState,
        add_inplace: ComputePipelineState,
        copy_f32_slice: ComputePipelineState,
        copy_f32_with_offset: ComputePipelineState,
        gelu_glu_f32: ComputePipelineState,
        scale_inplace: ComputePipelineState,
        matmul: ComputePipelineState,
        per_head_rms_norm_f32: ComputePipelineState,
        per_head_rms_norm_to_output_f32: ComputePipelineState,
        per_head_rms_norm_unit_f32: ComputePipelineState,
        rope_neox_f32: ComputePipelineState,
        rope_neox_position_f32: ComputePipelineState,
        decode_attention_dense_f32: ComputePipelineState,
        argmax_f32: ComputePipelineState,
        argmax_candidates_u32: ComputePipelineState,
        quantized_matvec_argmax_q4_k: ComputePipelineState,
        quantized_matvec_argmax_q5_0: ComputePipelineState,
        quantized_matvec_argmax_q5_k: ComputePipelineState,
        quantized_matvec_argmax_q6_k: ComputePipelineState,
        quantized_matvec_argmax_q8_0: ComputePipelineState,
        quantized_matvec_argmax_mxfp4: ComputePipelineState,
        quantized_matvec_q5_0: ComputePipelineState,
        quantized_matvec_q4_k: ComputePipelineState,
        quantized_matvec_q5_k: ComputePipelineState,
        quantized_matvec_q6_k: ComputePipelineState,
        quantized_matvec_q8_0: ComputePipelineState,
        quantized_matvec_mxfp4: ComputePipelineState,
        grouped_quantized_matvec_q5_0: ComputePipelineState,
        grouped_quantized_matvec_q4_k: ComputePipelineState,
        grouped_quantized_matvec_q8_0: ComputePipelineState,
        grouped_quantized_matvec_mxfp4: ComputePipelineState,
        expert_matvec_f32_ids_q5_0: ComputePipelineState,
        expert_matvec_f32_ids_q4_k: ComputePipelineState,
        expert_matvec_f32_ids_q8_0: ComputePipelineState,
        expert_matvec_f32_ids_mxfp4: ComputePipelineState,
        top_k_single_row_f32: ComputePipelineState,
        top_k_softmax_single_row_f32: ComputePipelineState,
        add_selected_expert_bias_f32: ComputePipelineState,
        aggregate_selected_expert_rows_f32: ComputePipelineState,
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
        compute_encoder: Option<ComputeCommandEncoder>,
    }

    impl PlatformSubmission {
        fn compute_encoder(&mut self) -> &ComputeCommandEncoder {
            if self.compute_encoder.is_none() {
                self.compute_encoder =
                    Some(self.command_buffer.new_compute_command_encoder().to_owned());
            }
            self.compute_encoder
                .as_ref()
                .expect("compute encoder is initialized")
        }

        fn end_compute_encoding(&mut self) {
            if let Some(encoder) = self.compute_encoder.take() {
                encoder.end_encoding();
            }
        }

        pub(super) fn fill_buffer(
            &mut self,
            buffer: &PlatformBuffer,
            byte_offset: usize,
            byte_len: usize,
            value: u8,
        ) -> Result<(), RuntimeError> {
            self.end_compute_encoding();
            let encoder = self.command_buffer.new_blit_command_encoder();
            encoder.fill_buffer(
                &buffer.raw,
                NSRange::new(to_metal_size(byte_offset)?, to_metal_size(byte_len)?),
                value,
            );
            encoder.end_encoding();
            Ok(())
        }

        pub(super) fn copy_buffer(
            &mut self,
            source: &PlatformBuffer,
            source_byte_offset: usize,
            destination: &PlatformBuffer,
            destination_byte_offset: usize,
            byte_len: usize,
        ) -> Result<(), RuntimeError> {
            self.end_compute_encoding();
            let encoder = self.command_buffer.new_blit_command_encoder();
            let size = to_metal_size(byte_len)?;
            encoder.copy_from_buffer(
                &source.raw,
                to_metal_size(source_byte_offset)?,
                &destination.raw,
                to_metal_size(destination_byte_offset)?,
                size,
            );
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
            self.end_compute_encoding();
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
            let encoder = self.compute_encoder();
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
            Ok(())
        }

        pub(super) fn encode_add_inplace(
            &mut self,
            pipeline: &ComputePipelineState,
            values: &PlatformBuffer,
            values_byte_offset: usize,
            bias: &PlatformBuffer,
            bias_byte_offset: usize,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&values.raw), to_metal_size(values_byte_offset)?);
            encoder.set_buffer(1, Some(&bias.raw), to_metal_size(bias_byte_offset)?);

            let element_count = u32::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal add-inplace element count overflow"))
            })?;
            encoder.set_bytes(2, 4, (&element_count as *const u32).cast());

            let threadgroup_size = compute_threadgroup_size(
                pipeline,
                usize::try_from(element_count).map_err(|_| {
                    RuntimeError::Backend(String::from(
                        "metal add-inplace element count conversion overflow",
                    ))
                })?,
            )?;
            encoder.dispatch_threads(
                MTLSize::new(u64::from(element_count), 1, 1),
                threadgroup_size,
            );
            Ok(())
        }

        pub(super) fn encode_scale_inplace(
            &mut self,
            pipeline: &ComputePipelineState,
            values: &PlatformBuffer,
            values_byte_offset: usize,
            scale: f32,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&values.raw), to_metal_size(values_byte_offset)?);

            let element_count = u32::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal scale-inplace element count overflow"))
            })?;
            encoder.set_bytes(1, 4, (&scale as *const f32).cast());
            encoder.set_bytes(2, 4, (&element_count as *const u32).cast());

            let threadgroup_size = compute_threadgroup_size(
                pipeline,
                usize::try_from(element_count).map_err(|_| {
                    RuntimeError::Backend(String::from(
                        "metal scale-inplace element count conversion overflow",
                    ))
                })?,
            )?;
            encoder.dispatch_threads(
                MTLSize::new(u64::from(element_count), 1, 1),
                threadgroup_size,
            );
            Ok(())
        }

        pub(super) fn encode_copy_f32_slice(
            &mut self,
            pipeline: &ComputePipelineState,
            source: &PlatformBuffer,
            destination: &PlatformBuffer,
            element_count: usize,
            source_offset_elements: usize,
            destination_offset_elements: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&source.raw), 0);
            encoder.set_buffer(1, Some(&destination.raw), 0);

            let element_count = u32::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal copy_f32_slice element count overflow"))
            })?;
            let source_offset_elements = u32::try_from(source_offset_elements).map_err(|_| {
                RuntimeError::Backend(String::from("metal copy_f32_slice source offset overflow"))
            })?;
            let destination_offset_elements =
                u32::try_from(destination_offset_elements).map_err(|_| {
                    RuntimeError::Backend(String::from(
                        "metal copy_f32_slice destination offset overflow",
                    ))
                })?;
            encoder.set_bytes(2, 4, (&element_count as *const u32).cast());
            encoder.set_bytes(3, 4, (&source_offset_elements as *const u32).cast());
            encoder.set_bytes(4, 4, (&destination_offset_elements as *const u32).cast());

            let threadgroup_size = compute_threadgroup_size(
                pipeline,
                usize::try_from(element_count).map_err(|_| {
                    RuntimeError::Backend(String::from(
                        "metal copy_f32_slice element count conversion overflow",
                    ))
                })?,
            )?;
            encoder.dispatch_threads(
                MTLSize::new(u64::from(element_count), 1, 1),
                threadgroup_size,
            );
            Ok(())
        }

        pub(super) fn encode_copy_f32_with_offset(
            &mut self,
            pipeline: &ComputePipelineState,
            source: &PlatformBuffer,
            source_byte_offset: usize,
            destination: &PlatformBuffer,
            destination_byte_offset: usize,
            element_count: usize,
            destination_offset_elements: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&source.raw), to_metal_size(source_byte_offset)?);
            encoder.set_buffer(
                1,
                Some(&destination.raw),
                to_metal_size(destination_byte_offset)?,
            );

            let element_count = u32::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal copy_f32 element count overflow"))
            })?;
            let destination_offset_elements =
                u32::try_from(destination_offset_elements).map_err(|_| {
                    RuntimeError::Backend(String::from(
                        "metal copy_f32 destination offset overflow",
                    ))
                })?;
            encoder.set_bytes(2, 4, (&element_count as *const u32).cast());
            encoder.set_bytes(3, 4, (&destination_offset_elements as *const u32).cast());

            let threadgroup_size = compute_threadgroup_size(
                pipeline,
                usize::try_from(element_count).map_err(|_| {
                    RuntimeError::Backend(String::from(
                        "metal copy_f32 element count conversion overflow",
                    ))
                })?,
            )?;
            encoder.dispatch_threads(
                MTLSize::new(u64::from(element_count), 1, 1),
                threadgroup_size,
            );
            Ok(())
        }

        pub(super) fn encode_gelu_glu_f32(
            &mut self,
            pipeline: &ComputePipelineState,
            gate: &PlatformBuffer,
            gate_byte_offset: usize,
            up: &PlatformBuffer,
            up_byte_offset: usize,
            output: &PlatformBuffer,
            output_byte_offset: usize,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&gate.raw), to_metal_size(gate_byte_offset)?);
            encoder.set_buffer(1, Some(&up.raw), to_metal_size(up_byte_offset)?);
            encoder.set_buffer(2, Some(&output.raw), to_metal_size(output_byte_offset)?);

            let element_count = u32::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal gelu_glu element count overflow"))
            })?;
            encoder.set_bytes(3, 4, (&element_count as *const u32).cast());

            let threadgroup_size = compute_threadgroup_size(
                pipeline,
                usize::try_from(element_count).map_err(|_| {
                    RuntimeError::Backend(String::from(
                        "metal gelu_glu element count conversion overflow",
                    ))
                })?,
            )?;
            encoder.dispatch_threads(
                MTLSize::new(u64::from(element_count), 1, 1),
                threadgroup_size,
            );
            Ok(())
        }

        pub(super) fn encode_top_k_single_row_f32(
            &mut self,
            pipeline: &ComputePipelineState,
            input: &PlatformBuffer,
            input_byte_offset: usize,
            selected_ids: &PlatformBuffer,
            selected_ids_byte_offset: usize,
            selected_values: &PlatformBuffer,
            selected_values_byte_offset: usize,
            column_count: usize,
            top_k: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&input.raw), to_metal_size(input_byte_offset)?);
            encoder.set_buffer(
                1,
                Some(&selected_ids.raw),
                to_metal_size(selected_ids_byte_offset)?,
            );
            encoder.set_buffer(
                2,
                Some(&selected_values.raw),
                to_metal_size(selected_values_byte_offset)?,
            );

            let column_count = u32::try_from(column_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal top-k column count overflow"))
            })?;
            let top_k = u32::try_from(top_k)
                .map_err(|_| RuntimeError::Backend(String::from("metal top-k overflow")))?;
            encoder.set_bytes(3, 4, (&column_count as *const u32).cast());
            encoder.set_bytes(4, 4, (&top_k as *const u32).cast());

            let threadgroup_size = compute_threadgroup_size(pipeline, 1)?;
            encoder.dispatch_thread_groups(MTLSize::new(1, 1, 1), threadgroup_size);
            Ok(())
        }

        pub(super) fn encode_top_k_softmax_single_row_f32(
            &mut self,
            pipeline: &ComputePipelineState,
            input: &PlatformBuffer,
            input_byte_offset: usize,
            selected_ids: &PlatformBuffer,
            selected_ids_byte_offset: usize,
            selected_weights: &PlatformBuffer,
            selected_weights_byte_offset: usize,
            column_count: usize,
            top_k: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&input.raw), to_metal_size(input_byte_offset)?);
            encoder.set_buffer(
                1,
                Some(&selected_ids.raw),
                to_metal_size(selected_ids_byte_offset)?,
            );
            encoder.set_buffer(
                2,
                Some(&selected_weights.raw),
                to_metal_size(selected_weights_byte_offset)?,
            );

            let column_count = u32::try_from(column_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal top-k softmax column count overflow"))
            })?;
            let top_k = u32::try_from(top_k).map_err(|_| {
                RuntimeError::Backend(String::from("metal top-k softmax top-k overflow"))
            })?;
            encoder.set_bytes(3, 4, (&column_count as *const u32).cast());
            encoder.set_bytes(4, 4, (&top_k as *const u32).cast());

            let threadgroup_size = compute_threadgroup_size(pipeline, 1)?;
            encoder.dispatch_thread_groups(MTLSize::new(1, 1, 1), threadgroup_size);
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
            let encoder = self.compute_encoder();
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
            Ok(())
        }

        pub(super) fn encode_per_head_rms_norm(
            &mut self,
            pipeline: &ComputePipelineState,
            values: &PlatformBuffer,
            values_byte_offset: usize,
            weight: &PlatformBuffer,
            weight_byte_offset: usize,
            head_count: usize,
            head_dim: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&values.raw), to_metal_size(values_byte_offset)?);
            encoder.set_buffer(1, Some(&weight.raw), to_metal_size(weight_byte_offset)?);

            let head_count = u32::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal per-head rms-norm head count overflow"))
            })?;
            let head_dim = u32::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from("metal per-head rms-norm head dim overflow"))
            })?;
            encoder.set_bytes(2, 4, (&head_count as *const u32).cast());
            encoder.set_bytes(3, 4, (&head_dim as *const u32).cast());
            encoder.set_bytes(4, 4, (&epsilon as *const f32).cast());
            encoder.dispatch_thread_groups(
                MTLSize::new(u64::from(head_count), 1, 1),
                rms_norm_threadgroup_size(pipeline, head_dim as usize)?,
            );
            Ok(())
        }

        pub(super) fn encode_per_head_rms_norm_to_output(
            &mut self,
            pipeline: &ComputePipelineState,
            input: &PlatformBuffer,
            input_byte_offset: usize,
            weight: &PlatformBuffer,
            weight_byte_offset: usize,
            output: &PlatformBuffer,
            output_byte_offset: usize,
            head_count: usize,
            head_dim: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&input.raw), to_metal_size(input_byte_offset)?);
            encoder.set_buffer(1, Some(&weight.raw), to_metal_size(weight_byte_offset)?);
            encoder.set_buffer(2, Some(&output.raw), to_metal_size(output_byte_offset)?);

            let head_count = u32::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal per-head rms-norm head count overflow"))
            })?;
            let head_dim = u32::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from("metal per-head rms-norm head dim overflow"))
            })?;
            encoder.set_bytes(3, 4, (&head_count as *const u32).cast());
            encoder.set_bytes(4, 4, (&head_dim as *const u32).cast());
            encoder.set_bytes(5, 4, (&epsilon as *const f32).cast());
            encoder.dispatch_thread_groups(
                MTLSize::new(u64::from(head_count), 1, 1),
                rms_norm_threadgroup_size(pipeline, head_dim as usize)?,
            );
            Ok(())
        }

        pub(super) fn encode_per_head_rms_norm_unit(
            &mut self,
            pipeline: &ComputePipelineState,
            values: &PlatformBuffer,
            values_byte_offset: usize,
            head_count: usize,
            head_dim: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&values.raw), to_metal_size(values_byte_offset)?);

            let head_count = u32::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "metal per-head unit rms-norm head count overflow",
                ))
            })?;
            let head_dim = u32::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "metal per-head unit rms-norm head dim overflow",
                ))
            })?;
            encoder.set_bytes(1, 4, (&head_count as *const u32).cast());
            encoder.set_bytes(2, 4, (&head_dim as *const u32).cast());
            encoder.set_bytes(3, 4, (&epsilon as *const f32).cast());
            encoder.dispatch_thread_groups(
                MTLSize::new(u64::from(head_count), 1, 1),
                rms_norm_threadgroup_size(pipeline, head_dim as usize)?,
            );
            Ok(())
        }

        pub(super) fn encode_rope_neox_inplace(
            &mut self,
            pipeline: &ComputePipelineState,
            values: &PlatformBuffer,
            cos: &PlatformBuffer,
            sin: &PlatformBuffer,
            head_count: usize,
            head_dim: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&values.raw), 0);
            encoder.set_buffer(1, Some(&cos.raw), 0);
            encoder.set_buffer(2, Some(&sin.raw), 0);

            let head_count = u32::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal rope head count overflow"))
            })?;
            let head_dim = u32::try_from(head_dim)
                .map_err(|_| RuntimeError::Backend(String::from("metal rope head dim overflow")))?;
            let half_dim = head_dim / 2;
            encoder.set_bytes(3, 4, (&head_count as *const u32).cast());
            encoder.set_bytes(4, 4, (&head_dim as *const u32).cast());
            encoder.set_bytes(5, 4, (&half_dim as *const u32).cast());
            let total_pairs = u64::from(head_count).saturating_mul(u64::from(half_dim));
            let threadgroup_size = compute_threadgroup_size(
                pipeline,
                usize::try_from(total_pairs).map_err(|_| {
                    RuntimeError::Backend(String::from("metal rope grid conversion overflow"))
                })?,
            )?;
            encoder.dispatch_threads(MTLSize::new(total_pairs, 1, 1), threadgroup_size);
            Ok(())
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_rope_neox_position(
            &mut self,
            pipeline: &ComputePipelineState,
            values: &PlatformBuffer,
            values_byte_offset: usize,
            freq_factors: &PlatformBuffer,
            freq_factors_byte_offset: usize,
            head_count: usize,
            head_dim: usize,
            rotary_half: usize,
            position: usize,
            theta_scale: f32,
            freq_scale: f32,
            corr_dims: [f32; 2],
            ext_factor: f32,
            yarn_mscale: f32,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&values.raw), to_metal_size(values_byte_offset)?);
            encoder.set_buffer(
                1,
                Some(&freq_factors.raw),
                to_metal_size(freq_factors_byte_offset)?,
            );

            let head_count = u32::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal rope head count overflow"))
            })?;
            let head_dim = u32::try_from(head_dim)
                .map_err(|_| RuntimeError::Backend(String::from("metal rope head dim overflow")))?;
            let half_dim = head_dim / 2;
            let rotary_half = u32::try_from(rotary_half).map_err(|_| {
                RuntimeError::Backend(String::from("metal rope rotary-half overflow"))
            })?;
            let position = u32::try_from(position)
                .map_err(|_| RuntimeError::Backend(String::from("metal rope position overflow")))?;
            encoder.set_bytes(2, 4, (&head_count as *const u32).cast());
            encoder.set_bytes(3, 4, (&head_dim as *const u32).cast());
            encoder.set_bytes(4, 4, (&half_dim as *const u32).cast());
            encoder.set_bytes(5, 4, (&rotary_half as *const u32).cast());
            encoder.set_bytes(6, 4, (&position as *const u32).cast());
            encoder.set_bytes(7, 4, (&theta_scale as *const f32).cast());
            encoder.set_bytes(8, 4, (&freq_scale as *const f32).cast());
            encoder.set_bytes(9, 4, (&corr_dims[0] as *const f32).cast());
            encoder.set_bytes(10, 4, (&corr_dims[1] as *const f32).cast());
            encoder.set_bytes(11, 4, (&ext_factor as *const f32).cast());
            encoder.set_bytes(12, 4, (&yarn_mscale as *const f32).cast());
            let total_pairs = u64::from(head_count).saturating_mul(u64::from(rotary_half));
            let threadgroup_size = compute_threadgroup_size(
                pipeline,
                usize::try_from(total_pairs).map_err(|_| {
                    RuntimeError::Backend(String::from(
                        "metal rope-position grid conversion overflow",
                    ))
                })?,
            )?;
            encoder.dispatch_threads(MTLSize::new(total_pairs, 1, 1), threadgroup_size);
            Ok(())
        }

        pub(super) fn encode_decode_attention_dense(
            &mut self,
            pipeline: &ComputePipelineState,
            query: &PlatformBuffer,
            query_byte_offset: usize,
            key_cache: &PlatformBuffer,
            key_cache_byte_offset: usize,
            value_cache: &PlatformBuffer,
            value_cache_byte_offset: usize,
            output: &PlatformBuffer,
            output_byte_offset: usize,
            query_head_count: usize,
            kv_head_count: usize,
            token_count: usize,
            head_dim: usize,
            scale: f32,
            active_simdgroups: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&query.raw), to_metal_size(query_byte_offset)?);
            encoder.set_buffer(
                1,
                Some(&key_cache.raw),
                to_metal_size(key_cache_byte_offset)?,
            );
            encoder.set_buffer(
                2,
                Some(&value_cache.raw),
                to_metal_size(value_cache_byte_offset)?,
            );
            encoder.set_buffer(3, Some(&output.raw), to_metal_size(output_byte_offset)?);

            let query_head_count = u32::try_from(query_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "metal decode attention query head count overflow",
                ))
            })?;
            let kv_head_count = u32::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "metal decode attention kv head count overflow",
                ))
            })?;
            let token_count = u32::try_from(token_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal decode attention token count overflow"))
            })?;
            let head_dim = u32::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from("metal decode attention head dim overflow"))
            })?;
            let active_simdgroups = u32::try_from(active_simdgroups).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "metal decode attention active simdgroup count overflow",
                ))
            })?;
            encoder.set_bytes(4, 4, (&query_head_count as *const u32).cast());
            encoder.set_bytes(5, 4, (&kv_head_count as *const u32).cast());
            encoder.set_bytes(6, 4, (&token_count as *const u32).cast());
            encoder.set_bytes(7, 4, (&head_dim as *const u32).cast());
            encoder.set_bytes(8, 4, (&scale as *const f32).cast());
            encoder.set_bytes(9, 4, (&active_simdgroups as *const u32).cast());
            encoder.dispatch_thread_groups(
                MTLSize::new(u64::from(query_head_count), 1, 1),
                decode_attention_threadgroup_size(pipeline, active_simdgroups as usize)?,
            );
            Ok(())
        }

        pub(super) fn encode_argmax_f32(
            &mut self,
            pipeline: &ComputePipelineState,
            input: &PlatformBuffer,
            input_byte_offset: usize,
            output: &PlatformBuffer,
            output_byte_offset: usize,
            row_count: usize,
            column_count: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&input.raw), to_metal_size(input_byte_offset)?);
            encoder.set_buffer(1, Some(&output.raw), to_metal_size(output_byte_offset)?);

            let row_count = u32::try_from(row_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal argmax row count overflow"))
            })?;
            let column_count = u32::try_from(column_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal argmax column count overflow"))
            })?;
            encoder.set_bytes(2, 4, (&row_count as *const u32).cast());
            encoder.set_bytes(3, 4, (&column_count as *const u32).cast());

            let threadgroup_size = argmax_threadgroup_size(pipeline)?;
            encoder
                .dispatch_thread_groups(MTLSize::new(u64::from(row_count), 1, 1), threadgroup_size);
            Ok(())
        }

        pub(super) fn encode_argmax_candidates(
            &mut self,
            pipeline: &ComputePipelineState,
            input: &PlatformBuffer,
            input_byte_offset: usize,
            output: &PlatformBuffer,
            output_byte_offset: usize,
            candidate_count: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&input.raw), to_metal_size(input_byte_offset)?);
            encoder.set_buffer(1, Some(&output.raw), to_metal_size(output_byte_offset)?);

            let candidate_count = u32::try_from(candidate_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal argmax candidate count overflow"))
            })?;
            encoder.set_bytes(2, 4, (&candidate_count as *const u32).cast());

            let threadgroup_size = argmax_threadgroup_size(pipeline)?;
            encoder.dispatch_thread_groups(MTLSize::new(1, 1, 1), threadgroup_size);
            Ok(())
        }

        pub(super) fn encode_quantized_matvec(
            &mut self,
            pipeline: &ComputePipelineState,
            weights: &PlatformBuffer,
            byte_offset: usize,
            input: &PlatformBuffer,
            input_byte_offset: usize,
            output: &PlatformBuffer,
            output_byte_offset: usize,
            rows: usize,
            columns: usize,
            row_stride: usize,
            active_threads: u32,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&weights.raw), 0);
            encoder.set_buffer(1, Some(&input.raw), to_metal_size(input_byte_offset)?);
            encoder.set_buffer(2, Some(&output.raw), to_metal_size(output_byte_offset)?);

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
            encoder.set_bytes(7, 4, (&active_threads as *const u32).cast());

            let threadgroup_size = MTLSize::new(u64::from(active_threads), 1, 1);
            encoder.dispatch_thread_groups(MTLSize::new(u64::from(rows), 1, 1), threadgroup_size);
            Ok(())
        }

        pub(super) fn encode_quantized_matvec_argmax(
            &mut self,
            pipeline: &ComputePipelineState,
            weights: &PlatformBuffer,
            byte_offset: usize,
            input: &PlatformBuffer,
            selected: &PlatformBuffer,
            rows: usize,
            columns: usize,
            row_stride: usize,
            active_threads: u32,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&weights.raw), 0);
            encoder.set_buffer(1, Some(&input.raw), 0);
            encoder.set_buffer(2, Some(&selected.raw), 0);

            let rows = u32::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("metal quantized argmax rows overflow"))
            })?;
            let columns = u32::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from("metal quantized argmax columns overflow"))
            })?;
            let row_stride = u32::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from("metal quantized argmax row stride overflow"))
            })?;
            let byte_offset = u64::try_from(byte_offset).map_err(|_| {
                RuntimeError::Backend(String::from("metal quantized argmax byte offset overflow"))
            })?;
            encoder.set_bytes(3, 4, (&rows as *const u32).cast());
            encoder.set_bytes(4, 4, (&columns as *const u32).cast());
            encoder.set_bytes(5, 4, (&row_stride as *const u32).cast());
            encoder.set_bytes(6, 8, (&byte_offset as *const u64).cast());
            encoder.set_bytes(7, 4, (&active_threads as *const u32).cast());

            let candidate_count = crate::quantized_argmax_candidate_count(rows as usize);
            let threadgroup_size = MTLSize::new(u64::from(active_threads), 1, 1);
            encoder.dispatch_thread_groups(
                MTLSize::new(
                    u64::try_from(candidate_count).map_err(|_| {
                        RuntimeError::Backend(String::from(
                            "metal quantized argmax candidate count overflow",
                        ))
                    })?,
                    1,
                    1,
                ),
                threadgroup_size,
            );
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
            let encoder = self.compute_encoder();
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
            Ok(())
        }

        pub(super) fn encode_grouped_quantized_matvec_buffer_ids(
            &mut self,
            pipeline: &ComputePipelineState,
            weights: &PlatformBuffer,
            input: &PlatformBuffer,
            output: &PlatformBuffer,
            rows_per_expert: usize,
            columns: usize,
            row_stride: usize,
            selected_ids: &PlatformBuffer,
            selected_ids_byte_offset: usize,
            selected_count: usize,
        ) -> Result<(), RuntimeError> {
            let total_rows = selected_count.saturating_mul(rows_per_expert);
            let encoder = self.compute_encoder();
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
            let selected_count = u32::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal grouped matvec selected count overflow"))
            })?;
            encoder.set_bytes(3, 4, (&rows_per_expert as *const u32).cast());
            encoder.set_bytes(4, 4, (&columns as *const u32).cast());
            encoder.set_bytes(5, 4, (&row_stride as *const u32).cast());
            encoder.set_bytes(6, 4, (&selected_count as *const u32).cast());
            encoder.set_buffer(
                7,
                Some(&selected_ids.raw),
                to_metal_size(selected_ids_byte_offset)?,
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
            let encoder = self.compute_encoder();
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
            Ok(())
        }

        pub(super) fn encode_expert_matvec_f32_ids_buffer(
            &mut self,
            pipeline: &ComputePipelineState,
            weights: &PlatformBuffer,
            input: &PlatformBuffer,
            output: &PlatformBuffer,
            rows_per_expert: usize,
            columns: usize,
            row_stride: usize,
            selected_ids: &PlatformBuffer,
            selected_ids_byte_offset: usize,
            selected_count: usize,
        ) -> Result<(), RuntimeError> {
            let total_rows = selected_count.saturating_mul(rows_per_expert);
            let encoder = self.compute_encoder();
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
            let selected_count = u32::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "metal expert_matvec_f32_ids selected count overflow",
                ))
            })?;
            encoder.set_bytes(3, 4, (&rows_per_expert as *const u32).cast());
            encoder.set_bytes(4, 4, (&columns as *const u32).cast());
            encoder.set_bytes(5, 4, (&row_stride as *const u32).cast());
            encoder.set_bytes(6, 4, (&selected_count as *const u32).cast());
            encoder.set_buffer(
                7,
                Some(&selected_ids.raw),
                to_metal_size(selected_ids_byte_offset)?,
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
            Ok(())
        }

        pub(super) fn encode_add_selected_expert_bias_f32(
            &mut self,
            pipeline: &ComputePipelineState,
            values: &PlatformBuffer,
            values_byte_offset: usize,
            bias: &PlatformBuffer,
            bias_byte_offset: usize,
            selected_ids: &PlatformBuffer,
            selected_ids_byte_offset: usize,
            rows_per_expert: usize,
            selected_count: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&values.raw), to_metal_size(values_byte_offset)?);
            encoder.set_buffer(1, Some(&bias.raw), to_metal_size(bias_byte_offset)?);
            encoder.set_buffer(
                2,
                Some(&selected_ids.raw),
                to_metal_size(selected_ids_byte_offset)?,
            );

            let rows_per_expert = u32::try_from(rows_per_expert).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "metal add-selected-expert-bias rows per expert overflow",
                ))
            })?;
            let selected_count = u32::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "metal add-selected-expert-bias selected count overflow",
                ))
            })?;
            encoder.set_bytes(3, 4, (&rows_per_expert as *const u32).cast());
            encoder.set_bytes(4, 4, (&selected_count as *const u32).cast());

            let element_count = rows_per_expert.saturating_mul(selected_count);
            let threadgroup_size = compute_threadgroup_size(
                pipeline,
                usize::try_from(element_count).map_err(|_| {
                    RuntimeError::Backend(String::from(
                        "metal add-selected-expert-bias element conversion overflow",
                    ))
                })?,
            )?;
            encoder.dispatch_threads(
                MTLSize::new(u64::from(element_count), 1, 1),
                threadgroup_size,
            );
            Ok(())
        }

        pub(super) fn encode_aggregate_selected_expert_rows_f32(
            &mut self,
            pipeline: &ComputePipelineState,
            input: &PlatformBuffer,
            input_byte_offset: usize,
            selected_ids: &PlatformBuffer,
            selected_ids_byte_offset: usize,
            selected_weights: &PlatformBuffer,
            selected_weights_byte_offset: usize,
            expert_scales: &PlatformBuffer,
            expert_scales_byte_offset: usize,
            output: &PlatformBuffer,
            output_byte_offset: usize,
            rows_per_expert: usize,
            selected_count: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.compute_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&input.raw), to_metal_size(input_byte_offset)?);
            encoder.set_buffer(
                1,
                Some(&selected_ids.raw),
                to_metal_size(selected_ids_byte_offset)?,
            );
            encoder.set_buffer(
                2,
                Some(&selected_weights.raw),
                to_metal_size(selected_weights_byte_offset)?,
            );
            encoder.set_buffer(
                3,
                Some(&expert_scales.raw),
                to_metal_size(expert_scales_byte_offset)?,
            );
            encoder.set_buffer(4, Some(&output.raw), to_metal_size(output_byte_offset)?);

            let rows_per_expert = u32::try_from(rows_per_expert).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "metal aggregate-selected-expert rows per expert overflow",
                ))
            })?;
            let selected_count = u32::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "metal aggregate-selected-expert selected count overflow",
                ))
            })?;
            encoder.set_bytes(5, 4, (&rows_per_expert as *const u32).cast());
            encoder.set_bytes(6, 4, (&selected_count as *const u32).cast());

            let threadgroup_size = compute_threadgroup_size(
                pipeline,
                usize::try_from(rows_per_expert).map_err(|_| {
                    RuntimeError::Backend(String::from(
                        "metal aggregate-selected-expert row conversion overflow",
                    ))
                })?,
            )?;
            encoder.dispatch_threads(
                MTLSize::new(u64::from(rows_per_expert), 1, 1),
                threadgroup_size,
            );
            Ok(())
        }

        pub(super) fn commit(
            mut self,
            wait: MetalCommandWait,
        ) -> Result<MetalCommandStatus, RuntimeError> {
            self.end_compute_encoding();
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
            Ok(PlatformSubmission {
                command_buffer,
                compute_encoder: None,
            })
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

        pub(super) fn encode_add_inplace(
            &mut self,
            submission: &mut PlatformSubmission,
            values: &MetalBuffer,
            bias: &MetalBuffer,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.add_inplace;
            submission.encode_add_inplace(
                pipeline,
                &values.platform,
                values.byte_offset,
                &bias.platform,
                bias.byte_offset,
                element_count,
            )
        }

        pub(super) fn encode_gelu_glu_f32(
            &mut self,
            submission: &mut PlatformSubmission,
            gate: &MetalBuffer,
            up: &MetalBuffer,
            output: &MetalBuffer,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.gelu_glu_f32;
            submission.encode_gelu_glu_f32(
                pipeline,
                &gate.platform,
                gate.byte_offset,
                &up.platform,
                up.byte_offset,
                &output.platform,
                output.byte_offset,
                element_count,
            )
        }

        pub(super) fn encode_scale_inplace(
            &mut self,
            submission: &mut PlatformSubmission,
            values: &MetalBuffer,
            scale: f32,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.scale_inplace;
            submission.encode_scale_inplace(
                pipeline,
                &values.platform,
                values.byte_offset,
                scale,
                element_count,
            )
        }

        pub(super) fn encode_copy_f32_slice(
            &mut self,
            submission: &mut PlatformSubmission,
            source: &MetalBuffer,
            destination: &MetalBuffer,
            element_count: usize,
            source_offset_elements: usize,
            destination_offset_elements: usize,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.copy_f32_slice;
            submission.encode_copy_f32_slice(
                pipeline,
                &source.platform,
                &destination.platform,
                element_count,
                source_offset_elements,
                destination_offset_elements,
            )
        }

        pub(super) fn encode_per_head_rms_norm(
            &mut self,
            submission: &mut PlatformSubmission,
            values: &MetalBuffer,
            weight: &MetalBuffer,
            head_count: usize,
            head_dim: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.per_head_rms_norm_f32;
            submission.encode_per_head_rms_norm(
                pipeline,
                &values.platform,
                values.byte_offset,
                &weight.platform,
                weight.byte_offset,
                head_count,
                head_dim,
                epsilon,
            )
        }

        pub(super) fn encode_per_head_rms_norm_to_output(
            &mut self,
            submission: &mut PlatformSubmission,
            input: &MetalBuffer,
            weight: &MetalBuffer,
            output: &MetalBuffer,
            head_count: usize,
            head_dim: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.per_head_rms_norm_to_output_f32;
            submission.encode_per_head_rms_norm_to_output(
                pipeline,
                &input.platform,
                input.byte_offset,
                &weight.platform,
                weight.byte_offset,
                &output.platform,
                output.byte_offset,
                head_count,
                head_dim,
                epsilon,
            )
        }

        pub(super) fn encode_per_head_rms_norm_unit(
            &mut self,
            submission: &mut PlatformSubmission,
            values: &MetalBuffer,
            head_count: usize,
            head_dim: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.per_head_rms_norm_unit_f32;
            submission.encode_per_head_rms_norm_unit(
                pipeline,
                &values.platform,
                values.byte_offset,
                head_count,
                head_dim,
                epsilon,
            )
        }

        pub(super) fn encode_rope_neox_inplace(
            &mut self,
            submission: &mut PlatformSubmission,
            values: &MetalBuffer,
            cos: &MetalBuffer,
            sin: &MetalBuffer,
            head_count: usize,
            head_dim: usize,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.rope_neox_f32;
            submission.encode_rope_neox_inplace(
                pipeline,
                &values.platform,
                &cos.platform,
                &sin.platform,
                head_count,
                head_dim,
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_rope_neox_position(
            &mut self,
            submission: &mut PlatformSubmission,
            values: &MetalBuffer,
            freq_factors: &MetalBuffer,
            head_count: usize,
            head_dim: usize,
            rotary_half: usize,
            position: usize,
            theta_scale: f32,
            freq_scale: f32,
            corr_dims: [f32; 2],
            ext_factor: f32,
            yarn_mscale: f32,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.rope_neox_position_f32;
            submission.encode_rope_neox_position(
                pipeline,
                &values.platform,
                values.byte_offset,
                &freq_factors.platform,
                freq_factors.byte_offset,
                head_count,
                head_dim,
                rotary_half,
                position,
                theta_scale,
                freq_scale,
                corr_dims,
                ext_factor,
                yarn_mscale,
            )
        }

        pub(super) fn encode_copy_f32_with_offset(
            &mut self,
            submission: &mut PlatformSubmission,
            source: &MetalBuffer,
            destination: &MetalBuffer,
            element_count: usize,
            destination_offset_elements: usize,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.copy_f32_with_offset;
            submission.encode_copy_f32_with_offset(
                pipeline,
                &source.platform,
                source.byte_offset,
                &destination.platform,
                destination.byte_offset,
                element_count,
                destination_offset_elements,
            )
        }

        pub(super) fn encode_decode_attention_dense(
            &mut self,
            submission: &mut PlatformSubmission,
            query: &MetalBuffer,
            cache: &MetalKvCacheMirror,
            query_head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            scale: f32,
            output: &MetalBuffer,
            active_simdgroups: usize,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.decode_attention_dense_f32;
            submission.encode_decode_attention_dense(
                pipeline,
                &query.platform,
                query.byte_offset,
                &cache.key_buffer.platform,
                cache.key_buffer.byte_offset,
                &cache.value_buffer.platform,
                cache.value_buffer.byte_offset,
                &output.platform,
                output.byte_offset,
                query_head_count,
                kv_head_count,
                cache.len(),
                head_dim,
                scale,
                active_simdgroups,
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

        pub(super) fn encode_matmul_with_dims(
            &mut self,
            submission: &mut PlatformSubmission,
            left: &MetalBuffer,
            right: &MetalBuffer,
            output: &MetalBuffer,
            m: usize,
            k: usize,
            n: usize,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.matmul;
            submission.encode_matmul(
                pipeline,
                &left.platform,
                &right.platform,
                &output.platform,
                m,
                k,
                n,
            )
        }

        pub(super) fn encode_argmax_f32(
            &mut self,
            submission: &mut PlatformSubmission,
            input: &MetalBuffer,
            output: &MetalBuffer,
            row_count: usize,
            column_count: usize,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.argmax_f32;
            submission.encode_argmax_f32(
                pipeline,
                &input.platform,
                input.byte_offset,
                &output.platform,
                output.byte_offset,
                row_count,
                column_count,
            )
        }

        pub(super) fn encode_argmax_candidates(
            &mut self,
            submission: &mut PlatformSubmission,
            input: &MetalBuffer,
            output: &MetalBuffer,
            candidate_count: usize,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.argmax_candidates_u32;
            submission.encode_argmax_candidates(
                pipeline,
                &input.platform,
                input.byte_offset,
                &output.platform,
                output.byte_offset,
                candidate_count,
            )
        }

        pub(super) fn encode_top_k_single_row_f32(
            &mut self,
            submission: &mut PlatformSubmission,
            input: &MetalBuffer,
            selected_ids: &MetalBuffer,
            selected_values: &MetalBuffer,
            column_count: usize,
            top_k: usize,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.top_k_single_row_f32;
            submission.encode_top_k_single_row_f32(
                pipeline,
                &input.platform,
                input.byte_offset,
                &selected_ids.platform,
                selected_ids.byte_offset,
                &selected_values.platform,
                selected_values.byte_offset,
                column_count,
                top_k,
            )
        }

        pub(super) fn encode_top_k_softmax_single_row_f32(
            &mut self,
            submission: &mut PlatformSubmission,
            input: &MetalBuffer,
            selected_ids: &MetalBuffer,
            selected_weights: &MetalBuffer,
            column_count: usize,
            top_k: usize,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.top_k_softmax_single_row_f32;
            submission.encode_top_k_softmax_single_row_f32(
                pipeline,
                &input.platform,
                input.byte_offset,
                &selected_ids.platform,
                selected_ids.byte_offset,
                &selected_weights.platform,
                selected_weights.byte_offset,
                column_count,
                top_k,
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
                QuantizationMode::GgmlQ5_0 => &pipelines.quantized_matvec_q5_0,
                QuantizationMode::GgmlQ4K => &pipelines.quantized_matvec_q4_k,
                QuantizationMode::GgmlQ5K => &pipelines.quantized_matvec_q5_k,
                QuantizationMode::GgmlQ6K => &pipelines.quantized_matvec_q6_k,
                QuantizationMode::GgmlQ8_0 => &pipelines.quantized_matvec_q8_0,
                QuantizationMode::GgmlMxfp4 => &pipelines.quantized_matvec_mxfp4,
                _ => {
                    return Err(RuntimeError::Backend(format!(
                        "metal quantized matvec does not support mode {mode:?}",
                    )));
                }
            };
            let active_threads = quantized_row_active_thread_count(pipeline, mode, columns)?;
            submission.encode_quantized_matvec(
                pipeline,
                &weights.platform,
                byte_offset,
                &input.platform,
                input.byte_offset,
                &output.platform,
                output.byte_offset,
                rows,
                columns,
                row_stride,
                active_threads,
            )
        }

        pub(super) fn encode_quantized_matvec_argmax(
            &mut self,
            submission: &mut PlatformSubmission,
            weights: &MetalBuffer,
            byte_offset: usize,
            mode: QuantizationMode,
            rows: usize,
            columns: usize,
            input: &MetalBuffer,
            selected: &MetalBuffer,
        ) -> Result<(), RuntimeError> {
            let row_stride = quantized_row_stride(mode, columns)?;
            let pipelines = self.pipelines()?;
            let pipeline = match mode {
                QuantizationMode::GgmlQ5_0 => &pipelines.quantized_matvec_argmax_q5_0,
                QuantizationMode::GgmlQ4K => &pipelines.quantized_matvec_argmax_q4_k,
                QuantizationMode::GgmlQ5K => &pipelines.quantized_matvec_argmax_q5_k,
                QuantizationMode::GgmlQ6K => &pipelines.quantized_matvec_argmax_q6_k,
                QuantizationMode::GgmlQ8_0 => &pipelines.quantized_matvec_argmax_q8_0,
                QuantizationMode::GgmlMxfp4 => &pipelines.quantized_matvec_argmax_mxfp4,
                _ => {
                    return Err(RuntimeError::Backend(format!(
                        "metal quantized argmax does not support mode {mode:?}",
                    )));
                }
            };
            let active_threads = quantized_row_active_thread_count(pipeline, mode, columns)?;
            submission.encode_quantized_matvec_argmax(
                pipeline,
                &weights.platform,
                byte_offset,
                &input.platform,
                &selected.platform,
                rows,
                columns,
                row_stride,
                active_threads,
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
                QuantizationMode::GgmlQ5_0 => &pipelines.grouped_quantized_matvec_q5_0,
                QuantizationMode::GgmlQ4K => &pipelines.grouped_quantized_matvec_q4_k,
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

        pub(super) fn encode_grouped_quantized_matvec_buffer_ids(
            &mut self,
            submission: &mut PlatformSubmission,
            weights: &MetalBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows_per_expert: usize,
            columns: usize,
            selected_ids: &MetalBuffer,
            selected_count: usize,
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
                QuantizationMode::GgmlQ5_0 => &pipelines.grouped_quantized_matvec_q5_0,
                QuantizationMode::GgmlQ4K => &pipelines.grouped_quantized_matvec_q4_k,
                QuantizationMode::GgmlQ8_0 => &pipelines.grouped_quantized_matvec_q8_0,
                QuantizationMode::GgmlMxfp4 => &pipelines.grouped_quantized_matvec_mxfp4,
                _ => {
                    return Err(RuntimeError::Backend(format!(
                        "metal grouped matvec does not support mode {mode:?}",
                    )));
                }
            };
            submission.encode_grouped_quantized_matvec_buffer_ids(
                pipeline,
                &weights.platform,
                &input.platform,
                &output.platform,
                rows_per_expert,
                columns,
                row_stride,
                &selected_ids.platform,
                selected_ids.byte_offset,
                selected_count,
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
                QuantizationMode::GgmlQ5_0 => &pipelines.expert_matvec_f32_ids_q5_0,
                QuantizationMode::GgmlQ4K => &pipelines.expert_matvec_f32_ids_q4_k,
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

        pub(super) fn encode_expert_matvec_f32_ids_buffer(
            &mut self,
            submission: &mut PlatformSubmission,
            weights: &MetalBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows_per_expert: usize,
            columns: usize,
            selected_ids: &MetalBuffer,
            selected_count: usize,
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
                QuantizationMode::GgmlQ5_0 => &pipelines.expert_matvec_f32_ids_q5_0,
                QuantizationMode::GgmlQ4K => &pipelines.expert_matvec_f32_ids_q4_k,
                QuantizationMode::GgmlQ8_0 => &pipelines.expert_matvec_f32_ids_q8_0,
                QuantizationMode::GgmlMxfp4 => &pipelines.expert_matvec_f32_ids_mxfp4,
                _ => {
                    return Err(RuntimeError::Backend(format!(
                        "metal expert_matvec_f32_ids does not support mode {mode:?}",
                    )));
                }
            };
            submission.encode_expert_matvec_f32_ids_buffer(
                pipeline,
                &weights.platform,
                &input.platform,
                &output.platform,
                rows_per_expert,
                columns,
                row_stride,
                &selected_ids.platform,
                selected_ids.byte_offset,
                selected_count,
            )
        }

        pub(super) fn encode_add_selected_expert_bias_f32(
            &mut self,
            submission: &mut PlatformSubmission,
            values: &MetalBuffer,
            bias: &MetalBuffer,
            selected_ids: &MetalBuffer,
            rows_per_expert: usize,
            selected_count: usize,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.add_selected_expert_bias_f32;
            submission.encode_add_selected_expert_bias_f32(
                pipeline,
                &values.platform,
                values.byte_offset,
                &bias.platform,
                bias.byte_offset,
                &selected_ids.platform,
                selected_ids.byte_offset,
                rows_per_expert,
                selected_count,
            )
        }

        pub(super) fn encode_aggregate_selected_expert_rows_f32(
            &mut self,
            submission: &mut PlatformSubmission,
            input: &MetalBuffer,
            selected_ids: &MetalBuffer,
            selected_weights: &MetalBuffer,
            expert_scales: &MetalBuffer,
            output: &MetalBuffer,
            rows_per_expert: usize,
            selected_count: usize,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.aggregate_selected_expert_rows_f32;
            submission.encode_aggregate_selected_expert_rows_f32(
                pipeline,
                &input.platform,
                input.byte_offset,
                &selected_ids.platform,
                selected_ids.byte_offset,
                &selected_weights.platform,
                selected_weights.byte_offset,
                &expert_scales.platform,
                expert_scales.byte_offset,
                &output.platform,
                output.byte_offset,
                rows_per_expert,
                selected_count,
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
                    mode: QuantizationMode::GgmlQ5_0,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::DequantizeToF32,
                },
                QuantizationSupport {
                    mode: QuantizationMode::GgmlQ4K,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::DequantizeToF32,
                },
                QuantizationSupport {
                    mode: QuantizationMode::GgmlQ5K,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::DequantizeToF32,
                },
                QuantizationSupport {
                    mode: QuantizationMode::GgmlQ6K,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::DequantizeToF32,
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
        let add_inplace = library
            .get_function("psionic_add_inplace", None)
            .map_err(|error| {
                RuntimeError::Backend(format!("missing Metal add_inplace kernel: {error}"))
            })?;
        let copy_f32_slice = library
            .get_function("psionic_copy_f32_slice", None)
            .map_err(|error| {
                RuntimeError::Backend(format!("missing Metal copy_f32_slice kernel: {error}"))
            })?;
        let copy_f32_with_offset = library
            .get_function("psionic_copy_f32_with_offset", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal copy_f32_with_offset kernel: {error}"
                ))
            })?;
        let gelu_glu_f32 = library
            .get_function("psionic_gelu_glu_f32", None)
            .map_err(|error| {
                RuntimeError::Backend(format!("missing Metal gelu_glu kernel: {error}"))
            })?;
        let scale_inplace = library
            .get_function("psionic_scale_inplace", None)
            .map_err(|error| {
                RuntimeError::Backend(format!("missing Metal scale_inplace kernel: {error}"))
            })?;
        let matmul = library
            .get_function("psionic_matmul", None)
            .map_err(|error| {
                RuntimeError::Backend(format!("missing Metal matmul kernel: {error}"))
            })?;
        let per_head_rms_norm_f32 = library
            .get_function("psionic_per_head_rms_norm_f32", None)
            .map_err(|error| {
                RuntimeError::Backend(format!("missing Metal per_head_rms_norm kernel: {error}"))
            })?;
        let per_head_rms_norm_to_output_f32 = library
            .get_function("psionic_per_head_rms_norm_to_output_f32", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal per_head_rms_norm_to_output kernel: {error}"
                ))
            })?;
        let per_head_rms_norm_unit_f32 = library
            .get_function("psionic_per_head_rms_norm_unit_f32", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal per_head_rms_norm_unit kernel: {error}"
                ))
            })?;
        let rope_neox_f32 = library
            .get_function("psionic_rope_neox_f32", None)
            .map_err(|error| {
                RuntimeError::Backend(format!("missing Metal rope_neox kernel: {error}"))
            })?;
        let rope_neox_position_f32 = library
            .get_function("psionic_rope_neox_position_f32", None)
            .map_err(|error| {
                RuntimeError::Backend(format!("missing Metal rope_neox_position kernel: {error}"))
            })?;
        let decode_attention_dense_f32 = library
            .get_function("psionic_decode_attention_dense_f32", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal decode_attention_dense kernel: {error}"
                ))
            })?;
        let argmax_f32 = library
            .get_function("psionic_argmax_f32", None)
            .map_err(|error| {
                RuntimeError::Backend(format!("missing Metal argmax kernel: {error}"))
            })?;
        let argmax_candidates_u32 = library
            .get_function("psionic_argmax_candidates_u32", None)
            .map_err(|error| {
                RuntimeError::Backend(format!("missing Metal argmax candidates kernel: {error}"))
            })?;
        let top_k_single_row_f32 = library
            .get_function("psionic_top_k_single_row_f32", None)
            .map_err(|error| {
                RuntimeError::Backend(format!("missing Metal top-k kernel: {error}"))
            })?;
        let top_k_softmax_single_row_f32 = library
            .get_function("psionic_top_k_softmax_single_row_f32", None)
            .map_err(|error| {
                RuntimeError::Backend(format!("missing Metal top-k softmax kernel: {error}"))
            })?;
        let add_selected_expert_bias_f32 = library
            .get_function("psionic_add_selected_expert_bias_f32", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal add_selected_expert_bias kernel: {error}"
                ))
            })?;
        let aggregate_selected_expert_rows_f32 = library
            .get_function("psionic_aggregate_selected_expert_rows_f32", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal aggregate_selected_expert_rows kernel: {error}"
                ))
            })?;
        let quantized_matvec_argmax_q8_0 = library
            .get_function("psionic_quantized_matvec_argmax_q8_0", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal q8_0 quantized argmax kernel: {error}"
                ))
            })?;
        let quantized_matvec_argmax_q4_k = library
            .get_function("psionic_quantized_matvec_argmax_q4_k", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal q4_k quantized argmax kernel: {error}"
                ))
            })?;
        let quantized_matvec_argmax_q5_0 = library
            .get_function("psionic_quantized_matvec_argmax_q5_0", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal q5_0 quantized argmax kernel: {error}"
                ))
            })?;
        let quantized_matvec_argmax_q5_k = library
            .get_function("psionic_quantized_matvec_argmax_q5_k", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal q5_k quantized argmax kernel: {error}"
                ))
            })?;
        let quantized_matvec_argmax_q6_k = library
            .get_function("psionic_quantized_matvec_argmax_q6_k", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal q6_k quantized argmax kernel: {error}"
                ))
            })?;
        let quantized_matvec_argmax_mxfp4 = library
            .get_function("psionic_quantized_matvec_argmax_mxfp4", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal mxfp4 quantized argmax kernel: {error}"
                ))
            })?;
        let quantized_matvec_q8_0 = library
            .get_function("psionic_quantized_matvec_q8_0", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal q8_0 quantized matvec kernel: {error}"
                ))
            })?;
        let quantized_matvec_q4_k = library
            .get_function("psionic_quantized_matvec_q4_k", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal q4_k quantized matvec kernel: {error}"
                ))
            })?;
        let quantized_matvec_q5_0 = library
            .get_function("psionic_quantized_matvec_q5_0", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal q5_0 quantized matvec kernel: {error}"
                ))
            })?;
        let quantized_matvec_q5_k = library
            .get_function("psionic_quantized_matvec_q5_k", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal q5_k quantized matvec kernel: {error}"
                ))
            })?;
        let quantized_matvec_q6_k = library
            .get_function("psionic_quantized_matvec_q6_k", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal q6_k quantized matvec kernel: {error}"
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
        let grouped_quantized_matvec_q4_k = library
            .get_function("psionic_mul_mv_id_q4_k", None)
            .map_err(|error| {
                RuntimeError::Backend(format!("missing Metal q4_k grouped matvec kernel: {error}"))
            })?;
        let grouped_quantized_matvec_q5_0 = library
            .get_function("psionic_mul_mv_id_q5_0", None)
            .map_err(|error| {
                RuntimeError::Backend(format!("missing Metal q5_0 grouped matvec kernel: {error}"))
            })?;
        let grouped_quantized_matvec_mxfp4 = library
            .get_function("psionic_mul_mv_id_mxfp4", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal mxfp4 grouped matvec kernel: {error}"
                ))
            })?;
        let expert_matvec_f32_ids_q4_k = library
            .get_function("psionic_expert_matvec_f32_ids_q4_k", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal q4_k expert_matvec_f32_ids kernel: {error}"
                ))
            })?;
        let expert_matvec_f32_ids_q5_0 = library
            .get_function("psionic_expert_matvec_f32_ids_q5_0", None)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "missing Metal q5_0 expert_matvec_f32_ids kernel: {error}"
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
            add_inplace: device
                .new_compute_pipeline_state_with_function(&add_inplace)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal add_inplace pipeline build failed: {error}"
                    ))
                })?,
            copy_f32_slice: device
                .new_compute_pipeline_state_with_function(&copy_f32_slice)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal copy_f32_slice pipeline build failed: {error}"
                    ))
                })?,
            copy_f32_with_offset: device
                .new_compute_pipeline_state_with_function(&copy_f32_with_offset)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal copy_f32_with_offset pipeline build failed: {error}"
                    ))
                })?,
            gelu_glu_f32: device
                .new_compute_pipeline_state_with_function(&gelu_glu_f32)
                .map_err(|error| {
                    RuntimeError::Backend(format!("metal gelu_glu pipeline build failed: {error}"))
                })?,
            scale_inplace: device
                .new_compute_pipeline_state_with_function(&scale_inplace)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal scale_inplace pipeline build failed: {error}"
                    ))
                })?,
            matmul: device
                .new_compute_pipeline_state_with_function(&matmul)
                .map_err(|error| {
                    RuntimeError::Backend(format!("metal matmul pipeline build failed: {error}"))
                })?,
            per_head_rms_norm_f32: device
                .new_compute_pipeline_state_with_function(&per_head_rms_norm_f32)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal per_head_rms_norm pipeline build failed: {error}"
                    ))
                })?,
            per_head_rms_norm_to_output_f32: device
                .new_compute_pipeline_state_with_function(&per_head_rms_norm_to_output_f32)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal per_head_rms_norm_to_output pipeline build failed: {error}"
                    ))
                })?,
            per_head_rms_norm_unit_f32: device
                .new_compute_pipeline_state_with_function(&per_head_rms_norm_unit_f32)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal per_head_rms_norm_unit pipeline build failed: {error}"
                    ))
                })?,
            rope_neox_f32: device
                .new_compute_pipeline_state_with_function(&rope_neox_f32)
                .map_err(|error| {
                    RuntimeError::Backend(format!("metal rope_neox pipeline build failed: {error}"))
                })?,
            rope_neox_position_f32: device
                .new_compute_pipeline_state_with_function(&rope_neox_position_f32)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal rope_neox_position pipeline build failed: {error}"
                    ))
                })?,
            decode_attention_dense_f32: device
                .new_compute_pipeline_state_with_function(&decode_attention_dense_f32)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal decode_attention_dense pipeline build failed: {error}"
                    ))
                })?,
            argmax_f32: device
                .new_compute_pipeline_state_with_function(&argmax_f32)
                .map_err(|error| {
                    RuntimeError::Backend(format!("metal argmax pipeline build failed: {error}"))
                })?,
            argmax_candidates_u32: device
                .new_compute_pipeline_state_with_function(&argmax_candidates_u32)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal argmax candidates pipeline build failed: {error}"
                    ))
                })?,
            top_k_single_row_f32: device
                .new_compute_pipeline_state_with_function(&top_k_single_row_f32)
                .map_err(|error| {
                    RuntimeError::Backend(format!("metal top-k pipeline build failed: {error}"))
                })?,
            top_k_softmax_single_row_f32: device
                .new_compute_pipeline_state_with_function(&top_k_softmax_single_row_f32)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal top-k softmax pipeline build failed: {error}"
                    ))
                })?,
            add_selected_expert_bias_f32: device
                .new_compute_pipeline_state_with_function(&add_selected_expert_bias_f32)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal add-selected-expert-bias pipeline build failed: {error}"
                    ))
                })?,
            aggregate_selected_expert_rows_f32: device
                .new_compute_pipeline_state_with_function(&aggregate_selected_expert_rows_f32)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal aggregate-selected-expert-rows pipeline build failed: {error}"
                    ))
                })?,
            quantized_matvec_argmax_q4_k: device
                .new_compute_pipeline_state_with_function(&quantized_matvec_argmax_q4_k)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q4_k quantized argmax pipeline build failed: {error}"
                    ))
                })?,
            quantized_matvec_argmax_q5_0: device
                .new_compute_pipeline_state_with_function(&quantized_matvec_argmax_q5_0)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q5_0 quantized argmax pipeline build failed: {error}"
                    ))
                })?,
            quantized_matvec_argmax_q5_k: device
                .new_compute_pipeline_state_with_function(&quantized_matvec_argmax_q5_k)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q5_k quantized argmax pipeline build failed: {error}"
                    ))
                })?,
            quantized_matvec_argmax_q6_k: device
                .new_compute_pipeline_state_with_function(&quantized_matvec_argmax_q6_k)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q6_k quantized argmax pipeline build failed: {error}"
                    ))
                })?,
            quantized_matvec_argmax_q8_0: device
                .new_compute_pipeline_state_with_function(&quantized_matvec_argmax_q8_0)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q8_0 quantized argmax pipeline build failed: {error}"
                    ))
                })?,
            quantized_matvec_argmax_mxfp4: device
                .new_compute_pipeline_state_with_function(&quantized_matvec_argmax_mxfp4)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal mxfp4 quantized argmax pipeline build failed: {error}"
                    ))
                })?,
            quantized_matvec_q4_k: device
                .new_compute_pipeline_state_with_function(&quantized_matvec_q4_k)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q4_k quantized matvec pipeline build failed: {error}"
                    ))
                })?,
            quantized_matvec_q5_0: device
                .new_compute_pipeline_state_with_function(&quantized_matvec_q5_0)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q5_0 quantized matvec pipeline build failed: {error}"
                    ))
                })?,
            quantized_matvec_q5_k: device
                .new_compute_pipeline_state_with_function(&quantized_matvec_q5_k)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q5_k quantized matvec pipeline build failed: {error}"
                    ))
                })?,
            quantized_matvec_q6_k: device
                .new_compute_pipeline_state_with_function(&quantized_matvec_q6_k)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q6_k quantized matvec pipeline build failed: {error}"
                    ))
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
            grouped_quantized_matvec_q5_0: device
                .new_compute_pipeline_state_with_function(&grouped_quantized_matvec_q5_0)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q5_0 grouped matvec pipeline build failed: {error}"
                    ))
                })?,
            grouped_quantized_matvec_q8_0: device
                .new_compute_pipeline_state_with_function(&grouped_quantized_matvec_q8_0)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q8_0 grouped matvec pipeline build failed: {error}"
                    ))
                })?,
            grouped_quantized_matvec_q4_k: device
                .new_compute_pipeline_state_with_function(&grouped_quantized_matvec_q4_k)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q4_k grouped matvec pipeline build failed: {error}"
                    ))
                })?,
            grouped_quantized_matvec_mxfp4: device
                .new_compute_pipeline_state_with_function(&grouped_quantized_matvec_mxfp4)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal mxfp4 grouped matvec pipeline build failed: {error}"
                    ))
                })?,
            expert_matvec_f32_ids_q5_0: device
                .new_compute_pipeline_state_with_function(&expert_matvec_f32_ids_q5_0)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q5_0 expert_matvec_f32_ids pipeline build failed: {error}"
                    ))
                })?,
            expert_matvec_f32_ids_q8_0: device
                .new_compute_pipeline_state_with_function(&expert_matvec_f32_ids_q8_0)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q8_0 expert_matvec_f32_ids pipeline build failed: {error}"
                    ))
                })?,
            expert_matvec_f32_ids_q4_k: device
                .new_compute_pipeline_state_with_function(&expert_matvec_f32_ids_q4_k)
                .map_err(|error| {
                    RuntimeError::Backend(format!(
                        "metal q4_k expert_matvec_f32_ids pipeline build failed: {error}"
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

    fn quantized_row_active_thread_count(
        pipeline: &ComputePipelineState,
        mode: QuantizationMode,
        columns: usize,
    ) -> Result<u32, RuntimeError> {
        if matches!(
            mode,
            QuantizationMode::GgmlQ4K | QuantizationMode::GgmlQ5K | QuantizationMode::GgmlQ6K
        ) {
            return Ok(32);
        }
        let Some((elements_per_block, _)) = mode.ggml_block_spec() else {
            return Err(RuntimeError::Backend(format!(
                "metal quantized kernel does not support mode {mode:?}",
            )));
        };
        let block_count = columns / elements_per_block;
        let desired = if block_count <= 12 {
            8_u64
        } else if block_count <= 24 {
            16_u64
        } else {
            32_u64
        };
        let width = desired
            .min(pipeline.thread_execution_width())
            .min(u64::from(pipeline.max_total_threads_per_threadgroup()));
        if width == 0 {
            return Err(RuntimeError::Backend(String::from(
                "metal quantized kernel reported zero thread execution width",
            )));
        }
        u32::try_from(width).map_err(|_| {
            RuntimeError::Backend(String::from(
                "metal quantized active thread count conversion overflow",
            ))
        })
    }

    fn quantized_row_threadgroup_size(
        pipeline: &ComputePipelineState,
    ) -> Result<MTLSize, RuntimeError> {
        let width = 8_u64
            .min(pipeline.thread_execution_width())
            .min(u64::from(pipeline.max_total_threads_per_threadgroup()));
        if width == 0 {
            return Err(RuntimeError::Backend(String::from(
                "metal quantized kernel reported zero thread execution width",
            )));
        }
        Ok(MTLSize::new(width, 1, 1))
    }

    fn rms_norm_threadgroup_size(
        pipeline: &ComputePipelineState,
        row_size: usize,
    ) -> Result<MTLSize, RuntimeError> {
        let requested_threads = if row_size <= 512 {
            32_u64
        } else if row_size <= 1024 {
            128_u64
        } else {
            let row_reads = row_size.saturating_add(3) / 4;
            let rounded = ((row_reads.saturating_add(31)) / 32).saturating_mul(32);
            rounded.clamp(256, 1024) as u64
        };
        let max_threads = pipeline.max_total_threads_per_threadgroup();
        if max_threads == 0 {
            return Err(RuntimeError::Backend(String::from(
                "metal rms-norm kernel reported zero max threads per threadgroup",
            )));
        }
        if u64::from(max_threads) < requested_threads {
            return Err(RuntimeError::Backend(format!(
                "metal rms-norm kernel requires at least {requested_threads} threads per threadgroup, actual {max_threads}",
            )));
        }
        Ok(MTLSize::new(requested_threads, 1, 1))
    }

    fn argmax_threadgroup_size(pipeline: &ComputePipelineState) -> Result<MTLSize, RuntimeError> {
        const ARGMAX_THREADS: u64 = 128;
        let max_threads = pipeline.max_total_threads_per_threadgroup();
        if max_threads == 0 {
            return Err(RuntimeError::Backend(String::from(
                "metal argmax kernel reported zero max threads per threadgroup",
            )));
        }
        if u64::from(max_threads) < ARGMAX_THREADS {
            return Err(RuntimeError::Backend(format!(
                "metal argmax kernel requires at least {ARGMAX_THREADS} threads per threadgroup, actual {max_threads}",
            )));
        }
        Ok(MTLSize::new(ARGMAX_THREADS, 1, 1))
    }

    fn decode_attention_threadgroup_size(
        pipeline: &ComputePipelineState,
        active_simdgroups: usize,
    ) -> Result<MTLSize, RuntimeError> {
        let active_simdgroups = active_simdgroups.clamp(1, crate::METAL_DECODE_MAX_SIMDGROUPS);
        let attention_threads = crate::METAL_DECODE_SIMDGROUP_THREADS
            * u64::try_from(active_simdgroups).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "metal decode-attention simdgroup count overflow",
                ))
            })?;
        let max_threads = pipeline.max_total_threads_per_threadgroup();
        if max_threads == 0 {
            return Err(RuntimeError::Backend(String::from(
                "metal decode-attention kernel reported zero max threads per threadgroup",
            )));
        }
        if u64::from(max_threads) < attention_threads {
            return Err(RuntimeError::Backend(format!(
                "metal decode-attention kernel requires at least {attention_threads} threads per threadgroup, actual {max_threads}",
            )));
        }
        Ok(MTLSize::new(attention_threads, 1, 1))
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
constant uint PSIONIC_ARGMAX_THREADS = 128;
constant uint PSIONIC_QUANTIZED_ARGMAX_ROWS_PER_THREADGROUP = 8;
constant uint PSIONIC_RMS_NORM_MAX_THREADS = 1024;
constant uint PSIONIC_RMS_NORM_MAX_SIMDGROUPS = PSIONIC_RMS_NORM_MAX_THREADS / 32;
constant uint PSIONIC_DECODE_THREADS = 32;
constant uint PSIONIC_DECODE_MAX_SIMDGROUPS = 31;
constant uint PSIONIC_DECODE_MAX_HEAD_DIM = 256;
constant uint PSIONIC_DECODE_MAX_HEAD_VALUES_PER_THREAD = PSIONIC_DECODE_MAX_HEAD_DIM / PSIONIC_DECODE_THREADS;
constant uint PSIONIC_TOP_K_MAX = 32;

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

kernel void psionic_add_inplace(
    device float* values [[buffer(0)]],
    const device float* bias [[buffer(1)]],
    constant uint& element_count [[buffer(2)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= element_count) {
        return;
    }
    values[gid] += bias[gid];
}

kernel void psionic_scale_inplace(
    device float* values [[buffer(0)]],
    constant float& scale [[buffer(1)]],
    constant uint& element_count [[buffer(2)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= element_count) {
        return;
    }
    values[gid] *= scale;
}

kernel void psionic_copy_f32_slice(
    const device float* source [[buffer(0)]],
    device float* destination [[buffer(1)]],
    constant uint& element_count [[buffer(2)]],
    constant uint& source_offset_elements [[buffer(3)]],
    constant uint& destination_offset_elements [[buffer(4)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= element_count) {
        return;
    }
    destination[destination_offset_elements + gid] = source[source_offset_elements + gid];
}

kernel void psionic_copy_f32_with_offset(
    const device float* source [[buffer(0)]],
    device float* destination [[buffer(1)]],
    constant uint& element_count [[buffer(2)]],
    constant uint& destination_offset_elements [[buffer(3)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= element_count) {
        return;
    }
    destination[destination_offset_elements + gid] = source[gid];
}

kernel void psionic_gelu_glu_f32(
    const device float* gate [[buffer(0)]],
    const device float* up [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& element_count [[buffer(3)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= element_count) {
        return;
    }

    float gate_value = gate[gid];
    float cubic = gate_value * gate_value * gate_value;
    float inner = 0.7978845608f * (gate_value + 0.044715f * cubic);
    float activated = 0.5f * gate_value * (1.0f + tanh(inner));
    output[gid] = activated * up[gid];
}

kernel void psionic_top_k_single_row_f32(
    const device float* input [[buffer(0)]],
    device int* selected_ids [[buffer(1)]],
    device float* selected_values [[buffer(2)]],
    constant uint& column_count [[buffer(3)]],
    constant uint& top_k [[buffer(4)]],
    uint tid [[thread_index_in_threadgroup]]
) {
    if (tid != 0) {
        return;
    }
    uint effective_k = min(min(top_k, column_count), PSIONIC_TOP_K_MAX);
    float best_values[PSIONIC_TOP_K_MAX];
    int best_indices[PSIONIC_TOP_K_MAX];
    for (uint index = 0; index < effective_k; ++index) {
        best_values[index] = -INFINITY;
        best_indices[index] = -1;
    }
    for (uint column = 0; column < column_count; ++column) {
        float value = input[column];
        uint insert_at = effective_k;
        for (uint slot = 0; slot < effective_k; ++slot) {
            bool better = value > best_values[slot]
                || (value == best_values[slot]
                    && (best_indices[slot] < 0 || int(column) < best_indices[slot]));
            if (better) {
                insert_at = slot;
                break;
            }
        }
        if (insert_at == effective_k) {
            continue;
        }
        for (uint slot = effective_k; slot > insert_at + 1; --slot) {
            best_values[slot - 1] = best_values[slot - 2];
            best_indices[slot - 1] = best_indices[slot - 2];
        }
        best_values[insert_at] = value;
        best_indices[insert_at] = int(column);
    }
    for (uint slot = 0; slot < effective_k; ++slot) {
        selected_ids[slot] = best_indices[slot];
        selected_values[slot] = best_values[slot];
    }
    for (uint slot = effective_k; slot < top_k; ++slot) {
        selected_ids[slot] = -1;
        selected_values[slot] = -INFINITY;
    }
}

kernel void psionic_top_k_softmax_single_row_f32(
    const device float* input [[buffer(0)]],
    device int* selected_ids [[buffer(1)]],
    device float* selected_weights [[buffer(2)]],
    constant uint& column_count [[buffer(3)]],
    constant uint& top_k [[buffer(4)]],
    uint tid [[thread_index_in_threadgroup]]
) {
    if (tid != 0) {
        return;
    }
    uint effective_k = min(min(top_k, column_count), PSIONIC_TOP_K_MAX);
    float best_values[PSIONIC_TOP_K_MAX];
    int best_indices[PSIONIC_TOP_K_MAX];
    for (uint index = 0; index < effective_k; ++index) {
        best_values[index] = -INFINITY;
        best_indices[index] = -1;
    }
    for (uint column = 0; column < column_count; ++column) {
        float value = input[column];
        uint insert_at = effective_k;
        for (uint slot = 0; slot < effective_k; ++slot) {
            bool better = value > best_values[slot]
                || (value == best_values[slot]
                    && (best_indices[slot] < 0 || int(column) < best_indices[slot]));
            if (better) {
                insert_at = slot;
                break;
            }
        }
        if (insert_at == effective_k) {
            continue;
        }
        for (uint slot = effective_k; slot > insert_at + 1; --slot) {
            best_values[slot - 1] = best_values[slot - 2];
            best_indices[slot - 1] = best_indices[slot - 2];
        }
        best_values[insert_at] = value;
        best_indices[insert_at] = int(column);
    }
    float max_value = best_values[0];
    float denom = 0.0f;
    for (uint slot = 0; slot < effective_k; ++slot) {
        float weight = exp(best_values[slot] - max_value);
        selected_ids[slot] = best_indices[slot];
        selected_weights[slot] = weight;
        denom += weight;
    }
    if (denom != 0.0f) {
        for (uint slot = 0; slot < effective_k; ++slot) {
            selected_weights[slot] /= denom;
        }
    }
    for (uint slot = effective_k; slot < top_k; ++slot) {
        selected_ids[slot] = -1;
        selected_weights[slot] = 0.0f;
    }
}

kernel void psionic_add_selected_expert_bias_f32(
    device float* values [[buffer(0)]],
    const device float* bias [[buffer(1)]],
    const device int* selected_ids [[buffer(2)]],
    constant uint& rows_per_expert [[buffer(3)]],
    constant uint& selected_count [[buffer(4)]],
    uint gid [[thread_position_in_grid]]
) {
    uint total_rows = rows_per_expert * selected_count;
    if (gid >= total_rows) {
        return;
    }
    uint selected_index = gid / rows_per_expert;
    uint row_in_expert = gid % rows_per_expert;
    int expert_id = selected_ids[selected_index];
    if (expert_id < 0) {
        return;
    }
    ulong bias_index = ulong(expert_id) * ulong(rows_per_expert) + ulong(row_in_expert);
    values[gid] += bias[bias_index];
}

kernel void psionic_aggregate_selected_expert_rows_f32(
    const device float* input [[buffer(0)]],
    const device int* selected_ids [[buffer(1)]],
    const device float* selected_weights [[buffer(2)]],
    const device float* expert_scales [[buffer(3)]],
    device float* output [[buffer(4)]],
    constant uint& rows_per_expert [[buffer(5)]],
    constant uint& selected_count [[buffer(6)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= rows_per_expert) {
        return;
    }
    float sum = 0.0f;
    for (uint selected_index = 0; selected_index < selected_count; ++selected_index) {
        int expert_id = selected_ids[selected_index];
        if (expert_id < 0) {
            continue;
        }
        float scale = selected_weights[selected_index] * expert_scales[expert_id];
        sum += input[ulong(selected_index) * ulong(rows_per_expert) + ulong(gid)] * scale;
    }
    output[gid] = sum;
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

kernel void psionic_per_head_rms_norm_f32(
    device float* values [[buffer(0)]],
    const device float* weight [[buffer(1)]],
    constant uint& head_count [[buffer(2)]],
    constant uint& head_dim [[buffer(3)]],
    constant float& epsilon [[buffer(4)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]],
    uint3 lsize3 [[threads_per_threadgroup]],
    uint lane [[thread_index_in_simdgroup]],
    uint simd_gid [[simdgroup_index_in_threadgroup]]
) {
    uint head = tgpig.x;
    if (head >= head_count) {
        return;
    }
    uint lsize = lsize3.x;
    threadgroup float partial[PSIONIC_RMS_NORM_MAX_SIMDGROUPS];
    threadgroup float shared_scale[1];
    uint active_simdgroups = (lsize + 31) / 32;
    uint base = head * head_dim;
    float sum = 0.0f;
    for (uint index = tid; index < head_dim; index += lsize) {
        float value = values[base + index];
        sum += value * value;
    }
    sum = simd_sum(sum);
    if (lane == 0) {
        partial[simd_gid] = sum;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (simd_gid == 0) {
        float total = lane < active_simdgroups
            ? partial[lane]
            : 0.0f;
        total = simd_sum(total);
        if (lane == 0) {
            shared_scale[0] = metal::precise::rsqrt((total / float(head_dim)) + epsilon);
        }
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    float scale = shared_scale[0];
    for (uint index = tid; index < head_dim; index += lsize) {
        values[base + index] = values[base + index] * scale * weight[index];
    }
}

kernel void psionic_per_head_rms_norm_to_output_f32(
    const device float* input [[buffer(0)]],
    const device float* weight [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& head_count [[buffer(3)]],
    constant uint& head_dim [[buffer(4)]],
    constant float& epsilon [[buffer(5)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]],
    uint3 lsize3 [[threads_per_threadgroup]],
    uint lane [[thread_index_in_simdgroup]],
    uint simd_gid [[simdgroup_index_in_threadgroup]]
) {
    uint head = tgpig.x;
    if (head >= head_count) {
        return;
    }
    uint lsize = lsize3.x;
    threadgroup float partial[PSIONIC_RMS_NORM_MAX_SIMDGROUPS];
    threadgroup float shared_scale[1];
    uint active_simdgroups = (lsize + 31) / 32;
    uint base = head * head_dim;
    float sum = 0.0f;
    for (uint index = tid; index < head_dim; index += lsize) {
        float value = input[base + index];
        sum += value * value;
    }
    sum = simd_sum(sum);
    if (lane == 0) {
        partial[simd_gid] = sum;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (simd_gid == 0) {
        float total = lane < active_simdgroups
            ? partial[lane]
            : 0.0f;
        total = simd_sum(total);
        if (lane == 0) {
            shared_scale[0] = metal::precise::rsqrt((total / float(head_dim)) + epsilon);
        }
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    float scale = shared_scale[0];
    for (uint index = tid; index < head_dim; index += lsize) {
        output[base + index] = input[base + index] * scale * weight[index];
    }
}

kernel void psionic_per_head_rms_norm_unit_f32(
    device float* values [[buffer(0)]],
    constant uint& head_count [[buffer(1)]],
    constant uint& head_dim [[buffer(2)]],
    constant float& epsilon [[buffer(3)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]],
    uint3 lsize3 [[threads_per_threadgroup]],
    uint lane [[thread_index_in_simdgroup]],
    uint simd_gid [[simdgroup_index_in_threadgroup]]
) {
    uint head = tgpig.x;
    if (head >= head_count) {
        return;
    }
    uint lsize = lsize3.x;
    threadgroup float partial[PSIONIC_RMS_NORM_MAX_SIMDGROUPS];
    threadgroup float shared_scale[1];
    uint active_simdgroups = (lsize + 31) / 32;
    uint base = head * head_dim;
    float sum = 0.0f;
    for (uint index = tid; index < head_dim; index += lsize) {
        float value = values[base + index];
        sum += value * value;
    }
    sum = simd_sum(sum);
    if (lane == 0) {
        partial[simd_gid] = sum;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (simd_gid == 0) {
        float total = lane < active_simdgroups
            ? partial[lane]
            : 0.0f;
        total = simd_sum(total);
        if (lane == 0) {
            shared_scale[0] = metal::precise::rsqrt((total / float(head_dim)) + epsilon);
        }
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    float scale = shared_scale[0];
    for (uint index = tid; index < head_dim; index += lsize) {
        values[base + index] = values[base + index] * scale;
    }
}

kernel void psionic_rope_neox_f32(
    device float* values [[buffer(0)]],
    const device float* cos_values [[buffer(1)]],
    const device float* sin_values [[buffer(2)]],
    constant uint& head_count [[buffer(3)]],
    constant uint& head_dim [[buffer(4)]],
    constant uint& half_dim [[buffer(5)]],
    uint gid [[thread_position_in_grid]]
) {
    uint total = head_count * half_dim;
    if (gid >= total) {
        return;
    }
    uint head = gid / half_dim;
    uint pair = gid % half_dim;
    uint base = head * head_dim;
    float x0 = values[base + pair];
    float x1 = values[base + pair + half_dim];
    float cos_theta = cos_values[pair];
    float sin_theta = sin_values[pair];
    values[base + pair] = x0 * cos_theta - x1 * sin_theta;
    values[base + pair + half_dim] = x0 * sin_theta + x1 * cos_theta;
}

inline float psionic_rope_yarn_ramp(float low, float high, uint i0) {
    float y = ((float(i0 / 2) - low) / max(high - low, 0.001f));
    return 1.0f - clamp(y, 0.0f, 1.0f);
}

kernel void psionic_rope_neox_position_f32(
    device float* values [[buffer(0)]],
    const device float* freq_factors [[buffer(1)]],
    constant uint& head_count [[buffer(2)]],
    constant uint& head_dim [[buffer(3)]],
    constant uint& half_dim [[buffer(4)]],
    constant uint& rotary_half [[buffer(5)]],
    constant uint& position [[buffer(6)]],
    constant float& theta_scale [[buffer(7)]],
    constant float& freq_scale [[buffer(8)]],
    constant float& corr_low [[buffer(9)]],
    constant float& corr_high [[buffer(10)]],
    constant float& ext_factor [[buffer(11)]],
    constant float& yarn_mscale [[buffer(12)]],
    uint gid [[thread_position_in_grid]]
) {
    uint total = head_count * rotary_half;
    if (gid >= total) {
        return;
    }
    uint head = gid / rotary_half;
    uint pair = gid % rotary_half;
    uint base = head * head_dim;
    float x0 = values[base + pair];
    float x1 = values[base + pair + half_dim];
    float freq_factor = freq_factors[pair];
    if (!(freq_factor > 0.0f)) {
        freq_factor = 1.0f;
    }
    float theta_extrap = (float(position) * pow(theta_scale, float(pair))) / freq_factor;
    float theta_interp = freq_scale * theta_extrap;
    float theta = theta_interp;
    float mscale = 1.0f;
    if (ext_factor != 0.0f) {
        float ramp = psionic_rope_yarn_ramp(corr_low, corr_high, pair * 2) * ext_factor;
        theta = theta_interp * (1.0f - ramp) + theta_extrap * ramp;
        mscale = yarn_mscale;
    }
    float cos_theta = cos(theta) * mscale;
    float sin_theta = sin(theta) * mscale;
    values[base + pair] = x0 * cos_theta - x1 * sin_theta;
    values[base + pair + half_dim] = x0 * sin_theta + x1 * cos_theta;
}

kernel void psionic_decode_attention_dense_f32(
    const device float* query [[buffer(0)]],
    const device float* keys [[buffer(1)]],
    const device float* values [[buffer(2)]],
    device float* output [[buffer(3)]],
    constant uint& query_head_count [[buffer(4)]],
    constant uint& kv_head_count [[buffer(5)]],
    constant uint& token_count [[buffer(6)]],
    constant uint& head_dim [[buffer(7)]],
    constant float& scale [[buffer(8)]],
    constant uint& active_simdgroups [[buffer(9)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint simd_gid [[simdgroup_index_in_threadgroup]],
    uint lane [[thread_index_in_simdgroup]]
) {
    uint head = tgpig.x;
    if (head >= query_head_count || head_dim > PSIONIC_DECODE_MAX_HEAD_DIM ||
        active_simdgroups == 0 || active_simdgroups > PSIONIC_DECODE_MAX_SIMDGROUPS ||
        simd_gid >= active_simdgroups) {
        return;
    }
    uint values_per_thread = (head_dim + PSIONIC_DECODE_THREADS - 1) / PSIONIC_DECODE_THREADS;
    if (values_per_thread > PSIONIC_DECODE_MAX_HEAD_VALUES_PER_THREAD) {
        return;
    }

    uint group_size = max(query_head_count / max(kv_head_count, 1u), 1u);
    uint kv_head = min(head / group_size, kv_head_count - 1);
    uint query_base = head * head_dim;
    uint cache_row_width = kv_head_count * head_dim;
    uint cache_head_base = kv_head * head_dim;
    float scale_log2e = scale * M_LOG2E_F;

    float q_fragment[PSIONIC_DECODE_MAX_HEAD_VALUES_PER_THREAD];
    float out_fragment[PSIONIC_DECODE_MAX_HEAD_VALUES_PER_THREAD];
    threadgroup float partial_outputs
        [PSIONIC_DECODE_MAX_SIMDGROUPS * PSIONIC_DECODE_MAX_HEAD_VALUES_PER_THREAD * PSIONIC_DECODE_THREADS];
    threadgroup float partial_max[PSIONIC_DECODE_MAX_SIMDGROUPS];
    threadgroup float partial_denom[PSIONIC_DECODE_MAX_SIMDGROUPS];
    threadgroup float block_factors[PSIONIC_DECODE_MAX_SIMDGROUPS];
    threadgroup float global_denom_value;
    for (uint slot = 0; slot < PSIONIC_DECODE_MAX_HEAD_VALUES_PER_THREAD; ++slot) {
        q_fragment[slot] = 0.0f;
        out_fragment[slot] = 0.0f;
    }
    for (uint slot = 0; slot < values_per_thread; ++slot) {
        uint dim = lane + slot * PSIONIC_DECODE_THREADS;
        if (dim < head_dim) {
            q_fragment[slot] = query[query_base + dim];
        }
    }

    float max_logit = -INFINITY;
    float denom = 0.0f;
    for (uint token = simd_gid; token < token_count; token += active_simdgroups) {
        uint token_base = token * cache_row_width + cache_head_base;
        float dot = 0.0f;
        for (uint slot = 0; slot < values_per_thread; ++slot) {
            uint dim = lane + slot * PSIONIC_DECODE_THREADS;
            if (dim < head_dim) {
                dot += q_fragment[slot] * keys[token_base + dim];
            }
        }
        float logit = simd_sum(dot) * scale_log2e;
        float next_max = max(max_logit, logit);
        float factor = fast::exp2(max_logit - next_max);
        float weight = fast::exp2(logit - next_max);
        max_logit = next_max;
        denom = denom * factor + weight;
        for (uint slot = 0; slot < values_per_thread; ++slot) {
            uint dim = lane + slot * PSIONIC_DECODE_THREADS;
            if (dim < head_dim) {
                out_fragment[slot] =
                    out_fragment[slot] * factor + weight * values[token_base + dim];
            }
        }
    }

    if (lane == 0) {
        partial_max[simd_gid] = max_logit;
        partial_denom[simd_gid] = denom;
    }
    for (uint slot = 0; slot < values_per_thread; ++slot) {
        partial_outputs[(simd_gid * PSIONIC_DECODE_MAX_HEAD_VALUES_PER_THREAD + slot) *
                PSIONIC_DECODE_THREADS +
            lane] = out_fragment[slot];
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    if (simd_gid == 0) {
        float local_max = lane < active_simdgroups ? partial_max[lane] : -INFINITY;
        float global_max = simd_max(local_max);
        float factor =
            lane < active_simdgroups ? fast::exp2(partial_max[lane] - global_max) : 0.0f;
        float global_denom =
            simd_sum((lane < active_simdgroups ? partial_denom[lane] : 0.0f) * factor);
        if (lane < active_simdgroups) {
            block_factors[lane] = factor;
        }
        if (lane == 0) {
            global_denom_value = global_denom;
        }
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    if (simd_gid == 0) {
        float normalizer = global_denom_value > 0.0f ? (1.0f / global_denom_value) : 0.0f;
        for (uint slot = 0; slot < values_per_thread; ++slot) {
            uint dim = lane + slot * PSIONIC_DECODE_THREADS;
            if (dim < head_dim) {
                float accumulated = 0.0f;
                for (uint block = 0; block < active_simdgroups; ++block) {
                    accumulated += partial_outputs
                        [(block * PSIONIC_DECODE_MAX_HEAD_VALUES_PER_THREAD + slot) *
                                PSIONIC_DECODE_THREADS +
                            lane] *
                        block_factors[block];
                }
                output[query_base + dim] = accumulated * normalizer;
            }
        }
    }
}

kernel void psionic_argmax_f32(
    const device float* input [[buffer(0)]],
    device float* output [[buffer(1)]],
    constant uint& row_count [[buffer(2)]],
    constant uint& column_count [[buffer(3)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]
) {
    uint row = tgpig.x;
    if (row >= row_count) {
        return;
    }

    threadgroup float partial_values[PSIONIC_ARGMAX_THREADS];
    threadgroup uint partial_indices[PSIONIC_ARGMAX_THREADS];

    uint row_offset = row * column_count;
    float best_value = -INFINITY;
    uint best_index = 0;
    for (uint index = tid; index < column_count; index += PSIONIC_ARGMAX_THREADS) {
        float value = input[row_offset + index];
        if (value > best_value || (value == best_value && index < best_index)) {
            best_value = value;
            best_index = index;
        }
    }

    partial_values[tid] = best_value;
    partial_indices[tid] = best_index;
    threadgroup_barrier(mem_flags::mem_threadgroup);

    for (uint stride = PSIONIC_ARGMAX_THREADS / 2; stride > 0; stride >>= 1) {
        if (tid < stride) {
            float candidate_value = partial_values[tid + stride];
            uint candidate_index = partial_indices[tid + stride];
            if (candidate_value > partial_values[tid]
                || (candidate_value == partial_values[tid]
                    && candidate_index < partial_indices[tid])) {
                partial_values[tid] = candidate_value;
                partial_indices[tid] = candidate_index;
            }
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    if (tid == 0) {
        output[row] = float(partial_indices[0]);
    }
}

kernel void psionic_argmax_candidates_u32(
    const device uint* input [[buffer(0)]],
    device uint* output [[buffer(1)]],
    constant uint& candidate_count [[buffer(2)]],
    uint tid [[thread_index_in_threadgroup]]
) {
    threadgroup uint partial_keys[PSIONIC_ARGMAX_THREADS];
    threadgroup uint partial_rows[PSIONIC_ARGMAX_THREADS];

    uint best_key = 0u;
    uint best_row = 0xffffffffu;
    for (uint index = tid; index < candidate_count; index += PSIONIC_ARGMAX_THREADS) {
        uint key = input[index * 2];
        uint row = input[index * 2 + 1];
        if (key > best_key || (key == best_key && row < best_row)) {
            best_key = key;
            best_row = row;
        }
    }

    partial_keys[tid] = best_key;
    partial_rows[tid] = best_row;
    threadgroup_barrier(mem_flags::mem_threadgroup);

    for (uint stride = PSIONIC_ARGMAX_THREADS / 2; stride > 0; stride >>= 1) {
        if (tid < stride) {
            uint candidate_key = partial_keys[tid + stride];
            uint candidate_row = partial_rows[tid + stride];
            if (candidate_key > partial_keys[tid]
                || (candidate_key == partial_keys[tid]
                    && candidate_row < partial_rows[tid])) {
                partial_keys[tid] = candidate_key;
                partial_rows[tid] = candidate_row;
            }
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    if (tid == 0) {
        output[0] = partial_keys[0];
        output[1] = partial_rows[0];
    }
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

inline float psionic_q5_0_block_dot(
    const device uchar* block,
    const device float* input
) {
    ushort scale_bits = ushort(block[0]) | (ushort(block[1]) << 8);
    float scale = float(as_type<half>(scale_bits));
    uint qh = uint(block[2]) |
        (uint(block[3]) << 8) |
        (uint(block[4]) << 16) |
        (uint(block[5]) << 24);
    const device uchar* quants = block + 6;
    float sum = 0.0f;
    for (uint pair_index = 0; pair_index < 16; ++pair_index) {
        uchar quant = quants[pair_index];
        uchar high_bit_low = uchar((((qh >> pair_index) << 4) & 0x10u));
        uchar high_bit_high = uchar(((qh >> (pair_index + 12u)) & 0x10u));
        float low = float(int((quant & 0x0fu) | high_bit_low) - 16);
        float high = float(int(((quant >> 4) & 0x0fu) | high_bit_high) - 16);
        sum += input[pair_index] * (low * scale);
        sum += input[pair_index + 16] * (high * scale);
    }
    return sum;
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

inline uchar2 psionic_q4_k_scale_min(
    uint index,
    const device uchar* packed
) {
    if (index < 4) {
        return uchar2(packed[index] & 63, packed[index + 4] & 63);
    }
    return uchar2(
        (packed[index + 4] & 0x0f) | ((packed[index - 4] >> 6) << 4),
        (packed[index + 4] >> 4) | ((packed[index] >> 6) << 4)
    );
}

inline float psionic_q4_k_block_dot(
    const device uchar* block,
    const device float* input
) {
    ushort scale_bits = ushort(block[0]) | (ushort(block[1]) << 8);
    float scale = float(as_type<half>(scale_bits));
    ushort min_bits = ushort(block[2]) | (ushort(block[3]) << 8);
    float minimum = float(as_type<half>(min_bits));
    const device uchar* scales = block + 4;
    const device uchar* quants = block + 16;
    float sum = 0.0f;
    uint input_offset = 0;
    uint scale_index = 0;

    for (uint chunk = 0; chunk < 4; chunk++) {
        const device uchar* quant_chunk = quants + (chunk * 32);

        uchar2 low = psionic_q4_k_scale_min(scale_index, scales);
        float low_scale = scale * float(low.x);
        float low_min = minimum * float(low.y);
        float low_input_sum = 0.0f;
        float low_quant_sum = 0.0f;
        for (uint index = 0; index < 32; index += 4) {
            float4 input_values = float4(
                input[input_offset + index + 0],
                input[input_offset + index + 1],
                input[input_offset + index + 2],
                input[input_offset + index + 3]
            );
            uchar4 quant_values = uchar4(
                quant_chunk[index + 0],
                quant_chunk[index + 1],
                quant_chunk[index + 2],
                quant_chunk[index + 3]
            );
            float4 quant_low = float4(
                float(quant_values[0] & 0x0f),
                float(quant_values[1] & 0x0f),
                float(quant_values[2] & 0x0f),
                float(quant_values[3] & 0x0f)
            );
            low_input_sum += input_values[0] + input_values[1] + input_values[2] + input_values[3];
            low_quant_sum += dot(input_values, quant_low);
        }
        sum += (low_quant_sum * low_scale) - (low_input_sum * low_min);
        input_offset += 32;
        scale_index += 1;

        uchar2 high = psionic_q4_k_scale_min(scale_index, scales);
        float high_scale = scale * float(high.x);
        float high_min = minimum * float(high.y);
        float high_input_sum = 0.0f;
        float high_quant_sum = 0.0f;
        for (uint index = 0; index < 32; index += 4) {
            float4 input_values = float4(
                input[input_offset + index + 0],
                input[input_offset + index + 1],
                input[input_offset + index + 2],
                input[input_offset + index + 3]
            );
            uchar4 quant_values = uchar4(
                quant_chunk[index + 0],
                quant_chunk[index + 1],
                quant_chunk[index + 2],
                quant_chunk[index + 3]
            );
            float4 quant_high = float4(
                float((quant_values[0] >> 4) & 0x0f),
                float((quant_values[1] >> 4) & 0x0f),
                float((quant_values[2] >> 4) & 0x0f),
                float((quant_values[3] >> 4) & 0x0f)
            );
            high_input_sum += input_values[0] + input_values[1] + input_values[2] + input_values[3];
            high_quant_sum += dot(input_values, quant_high);
        }
        sum += (high_quant_sum * high_scale) - (high_input_sum * high_min);
        input_offset += 32;
        scale_index += 1;
    }

    return sum;
}

inline float psionic_q5_k_block_dot(
    const device uchar* block,
    const device float* input
) {
    ushort scale_bits = ushort(block[0]) | (ushort(block[1]) << 8);
    float scale = float(as_type<half>(scale_bits));
    ushort min_bits = ushort(block[2]) | (ushort(block[3]) << 8);
    float minimum = float(as_type<half>(min_bits));
    const device uchar* scales = block + 4;
    const device uchar* high_bits = block + 16;
    const device uchar* quants = block + 48;
    float sum = 0.0f;
    uint input_offset = 0;
    uint scale_index = 0;
    uchar low_mask = 1u;
    uchar high_mask = 2u;

    for (uint chunk = 0; chunk < 4; ++chunk) {
        const device uchar* quant_chunk = quants + (chunk * 32);

        uchar2 low = psionic_q4_k_scale_min(scale_index, scales);
        float low_scale = scale * float(low.x);
        float low_min = minimum * float(low.y);
        float low_input_sum = 0.0f;
        float low_quant_sum = 0.0f;
        for (uint index = 0; index < 32; ++index) {
            float input_value = input[input_offset + index];
            uchar quant = quant_chunk[index];
            uchar lifted = (high_bits[index] & low_mask) != 0u ? 16u : 0u;
            float quant_low = float((quant & 0x0f) + lifted);
            low_input_sum += input_value;
            low_quant_sum += input_value * quant_low;
        }
        sum += (low_quant_sum * low_scale) - (low_input_sum * low_min);
        input_offset += 32;
        scale_index += 1;

        uchar2 high = psionic_q4_k_scale_min(scale_index, scales);
        float high_scale = scale * float(high.x);
        float high_min = minimum * float(high.y);
        float high_input_sum = 0.0f;
        float high_quant_sum = 0.0f;
        for (uint index = 0; index < 32; ++index) {
            float input_value = input[input_offset + index];
            uchar quant = quant_chunk[index];
            uchar lifted = (high_bits[index] & high_mask) != 0u ? 16u : 0u;
            float quant_high = float(((quant >> 4) & 0x0f) + lifted);
            high_input_sum += input_value;
            high_quant_sum += input_value * quant_high;
        }
        sum += (high_quant_sum * high_scale) - (high_input_sum * high_min);
        input_offset += 32;
        scale_index += 1;
        low_mask <<= 2;
        high_mask <<= 2;
    }

    return sum;
}

inline float psionic_q6_k_block_dot(
    const device uchar* block,
    const device float* input
) {
    const device uchar* ql = block;
    const device uchar* qh = block + 128;
    const device uchar* scales = block + 192;
    ushort scale_bits = ushort(block[208]) | (ushort(block[209]) << 8);
    float scale = float(as_type<half>(scale_bits));
    float sum = 0.0f;

    for (uint chunk = 0; chunk < 2; chunk++) {
        const device uchar* ql_chunk = ql + (chunk * 64);
        const device uchar* qh_chunk = qh + (chunk * 32);
        const device uchar* scales_chunk = scales + (chunk * 8);
        const device float* input_chunk = input + (chunk * 128);

        for (uint group = 0; group < 2; group++) {
            uint is = group;
            float scale_1 = scale * float(as_type<char>(scales_chunk[is]));
            float scale_2 = scale * float(as_type<char>(scales_chunk[is + 2]));
            float scale_3 = scale * float(as_type<char>(scales_chunk[is + 4]));
            float scale_4 = scale * float(as_type<char>(scales_chunk[is + 6]));
            uint group_offset = group * 16;

            for (uint lane = 0; lane < 16; lane++) {
                uint l = group_offset + lane;
                int q1 = int(ql_chunk[l] & 0x0f) | (int((qh_chunk[l] >> 0) & 0x03) << 4);
                int q2 = int(ql_chunk[l + 32] & 0x0f) | (int((qh_chunk[l] >> 2) & 0x03) << 4);
                int q3 = int(ql_chunk[l] >> 4) | (int((qh_chunk[l] >> 4) & 0x03) << 4);
                int q4 = int(ql_chunk[l + 32] >> 4) | (int((qh_chunk[l] >> 6) & 0x03) << 4);
                q1 -= 32;
                q2 -= 32;
                q3 -= 32;
                q4 -= 32;

                sum += input_chunk[l] * (scale_1 * float(q1));
                sum += input_chunk[l + 32] * (scale_2 * float(q2));
                sum += input_chunk[l + 64] * (scale_3 * float(q3));
                sum += input_chunk[l + 96] * (scale_4 * float(q4));
            }
        }
    }

    return sum;
}

kernel void psionic_quantized_matvec_q4_k(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& rows [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant ulong& byte_offset [[buffer(6)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    ushort lane [[thread_index_in_simdgroup]]
) {
    uint row = tgpig.x;
    if (row >= rows) {
        return;
    }
    const device uchar* row_weights = weights + byte_offset + (ulong(row) * ulong(row_stride));
    uint block_count = columns / 256;

    const short ix = short(lane / 8);
    const short it = short(lane % 8);
    const short iq = short(it / 4);
    const short ir = short(it % 4);

    float sum = 0.0f;

    for (uint block_index = uint(ix); block_index < block_count; block_index += 4) {
        const device uchar* block = row_weights + (block_index * 144);
        const device float* block_input = input + block_index * 256 + 64 * uint(iq) + 8 * uint(ir);

        float yl[16];
        float yh[16];
        float4 sumy = float4(0.0f);

        for (short i = 0; i < 8; ++i) {
            yl[i + 0] = block_input[i + 0];
            yl[i + 8] = block_input[i + 32];
            yh[i + 0] = block_input[i + 128];
            yh[i + 8] = block_input[i + 160];
            sumy[0] += yl[i + 0];
            sumy[1] += yl[i + 8];
            sumy[2] += yh[i + 0];
            sumy[3] += yh[i + 8];
        }

        const device ushort* scales = (const device ushort*)(block + 4) + iq;
        const device ushort* q1 = (const device ushort*)(block + 16) + 16 * iq + 4 * ir;
        const device half* dh = (const device half*)block;

        constexpr ushort kmask1 = 0x3f3f;
        constexpr ushort kmask2 = 0x0f0f;
        constexpr ushort kmask3 = 0xc0c0;

        ushort sc16[4];
        thread const uchar* sc8 = (thread const uchar*)sc16;

        sc16[0] = scales[0] & kmask1;
        sc16[1] = scales[2] & kmask1;
        sc16[2] = ((scales[4] >> 0) & kmask2) | ((scales[0] & kmask3) >> 2);
        sc16[3] = ((scales[4] >> 4) & kmask2) | ((scales[2] & kmask3) >> 2);

        const device ushort* q2 = q1 + 32;
        float4 acc1 = float4(0.0f);
        float4 acc2 = float4(0.0f);

        for (short i = 0; i < 4; ++i) {
            acc1[0] += yl[2 * i + 0] * float(q1[i] & 0x000F);
            acc1[1] += yl[2 * i + 1] * float(q1[i] & 0x0F00);
            acc1[2] += yl[2 * i + 8] * float(q1[i] & 0x00F0);
            acc1[3] += yl[2 * i + 9] * float(q1[i] & 0xF000);
            acc2[0] += yh[2 * i + 0] * float(q2[i] & 0x000F);
            acc2[1] += yh[2 * i + 1] * float(q2[i] & 0x0F00);
            acc2[2] += yh[2 * i + 8] * float(q2[i] & 0x00F0);
            acc2[3] += yh[2 * i + 9] * float(q2[i] & 0xF000);
        }

        sum += float(dh[0]) * (
                (acc1[0] + (1.0f / 256.0f) * acc1[1]) * float(sc8[0]) +
                (acc1[2] + (1.0f / 256.0f) * acc1[3]) * float(sc8[1]) * (1.0f / 16.0f) +
                (acc2[0] + (1.0f / 256.0f) * acc2[1]) * float(sc8[4]) +
                (acc2[2] + (1.0f / 256.0f) * acc2[3]) * float(sc8[5]) * (1.0f / 16.0f)
            ) -
            float(dh[1]) * (
                sumy[0] * float(sc8[2]) +
                sumy[1] * float(sc8[3]) +
                sumy[2] * float(sc8[6]) +
                sumy[3] * float(sc8[7])
            );
    }

    float total = simd_sum(sum);
    if (lane == 0) {
        output[row] = total;
    }
}

inline uint psionic_ordered_float_key(float value) {
    uint bits = as_type<uint>(isfinite(value) ? value : -INFINITY);
    return (bits & 0x80000000u) != 0u ? ~bits : (bits | 0x80000000u);
}

inline bool psionic_argmax_candidate_better(
    uint candidate_key,
    uint candidate_row,
    uint best_key,
    uint best_row
) {
    return candidate_key > best_key || (candidate_key == best_key && candidate_row < best_row);
}

inline void psionic_write_argmax_candidate(
    device uint* output,
    uint candidate_index,
    float value,
    uint row
) {
    uint base = candidate_index * 2;
    output[base] = psionic_ordered_float_key(value);
    output[base + 1] = row;
}

kernel void psionic_quantized_matvec_q5_0(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& rows [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant ulong& byte_offset [[buffer(6)]],
    constant uint& active_threads [[buffer(7)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]
) {
    uint row = tgpig.x;
    if (row >= rows) {
        return;
    }
    threadgroup float partial[PSIONIC_QUANTIZED_ROW_THREADS];
    const device uchar* row_weights = weights + byte_offset + (ulong(row) * ulong(row_stride));
    uint block_count = columns / 32;
    float sum = 0.0f;
    for (uint block_index = tid; block_index < block_count; block_index += active_threads) {
        const device uchar* block = row_weights + (block_index * 22u);
        sum += psionic_q5_0_block_dot(block, input + block_index * 32);
    }
    partial[tid] = sum;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint stride = active_threads / 2; stride > 0; stride >>= 1) {
        if (tid < stride) {
            partial[tid] += partial[tid + stride];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (tid == 0) {
        output[row] = partial[0];
    }
}

kernel void psionic_quantized_matvec_argmax_q4_k(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device uint* output [[buffer(2)]],
    constant uint& rows [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant ulong& byte_offset [[buffer(6)]],
    constant uint& active_threads [[buffer(7)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]
) {
    uint candidate_index = tgpig.x;
    uint base_row = candidate_index * PSIONIC_QUANTIZED_ARGMAX_ROWS_PER_THREADGROUP;
    if (base_row >= rows) {
        return;
    }
    threadgroup float partial[PSIONIC_QUANTIZED_ROW_THREADS];
    uint best_key = 0u;
    uint best_row = 0xffffffffu;
    uint block_count = columns / 256;
    for (uint local_row = 0; local_row < PSIONIC_QUANTIZED_ARGMAX_ROWS_PER_THREADGROUP; ++local_row) {
        uint row = base_row + local_row;
        if (row >= rows) {
            break;
        }
        const device uchar* row_weights = weights + byte_offset + (ulong(row) * ulong(row_stride));
        float sum = 0.0f;
        for (uint block_index = tid; block_index < block_count; block_index += active_threads) {
            const device uchar* block = row_weights + (block_index * 144);
            sum += psionic_q4_k_block_dot(block, input + block_index * 256);
        }
        partial[tid] = sum;
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (uint stride = active_threads / 2; stride > 0; stride >>= 1) {
            if (tid < stride) {
                partial[tid] += partial[tid + stride];
            }
            threadgroup_barrier(mem_flags::mem_threadgroup);
        }
        if (tid == 0) {
            uint candidate_key = psionic_ordered_float_key(partial[0]);
            if (psionic_argmax_candidate_better(candidate_key, row, best_key, best_row)) {
                best_key = candidate_key;
                best_row = row;
            }
        }
    }
    if (tid == 0) {
        uint base = candidate_index * 2;
        output[base] = best_key;
        output[base + 1] = best_row;
    }
}

kernel void psionic_quantized_matvec_argmax_q5_0(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device uint* output [[buffer(2)]],
    constant uint& rows [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant ulong& byte_offset [[buffer(6)]],
    constant uint& active_threads [[buffer(7)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]
) {
    uint candidate_index = tgpig.x;
    uint base_row = candidate_index * PSIONIC_QUANTIZED_ARGMAX_ROWS_PER_THREADGROUP;
    if (base_row >= rows) {
        return;
    }
    threadgroup float partial[PSIONIC_QUANTIZED_ROW_THREADS];
    uint best_key = 0u;
    uint best_row = 0xffffffffu;
    uint block_count = columns / 32;
    for (uint local_row = 0; local_row < PSIONIC_QUANTIZED_ARGMAX_ROWS_PER_THREADGROUP; ++local_row) {
        uint row = base_row + local_row;
        if (row >= rows) {
            break;
        }
        const device uchar* row_weights = weights + byte_offset + (ulong(row) * ulong(row_stride));
        float sum = 0.0f;
        for (uint block_index = tid; block_index < block_count; block_index += active_threads) {
            const device uchar* block = row_weights + (block_index * 22u);
            sum += psionic_q5_0_block_dot(block, input + block_index * 32);
        }
        partial[tid] = sum;
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (uint stride = active_threads / 2; stride > 0; stride >>= 1) {
            if (tid < stride) {
                partial[tid] += partial[tid + stride];
            }
            threadgroup_barrier(mem_flags::mem_threadgroup);
        }
        if (tid == 0) {
            uint candidate_key = psionic_ordered_float_key(partial[0]);
            if (psionic_argmax_candidate_better(candidate_key, row, best_key, best_row)) {
                best_key = candidate_key;
                best_row = row;
            }
        }
    }
    if (tid == 0) {
        uint base = candidate_index * 2;
        output[base] = best_key;
        output[base + 1] = best_row;
    }
}

kernel void psionic_quantized_matvec_q5_k(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& rows [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant ulong& byte_offset [[buffer(6)]],
    constant uint& active_threads [[buffer(7)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]
) {
    uint row = tgpig.x;
    if (row >= rows) {
        return;
    }
    threadgroup float partial[PSIONIC_QUANTIZED_ROW_THREADS];
    const device uchar* row_weights = weights + byte_offset + (ulong(row) * ulong(row_stride));
    uint block_count = columns / 256;
    float sum = 0.0f;
    for (uint block_index = tid; block_index < block_count; block_index += active_threads) {
        const device uchar* block = row_weights + (block_index * 176);
        sum += psionic_q5_k_block_dot(block, input + block_index * 256);
    }
    partial[tid] = sum;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint stride = active_threads / 2; stride > 0; stride >>= 1) {
        if (tid < stride) {
            partial[tid] += partial[tid + stride];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (tid == 0) {
        output[row] = partial[0];
    }
}

kernel void psionic_quantized_matvec_argmax_q5_k(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device uint* output [[buffer(2)]],
    constant uint& rows [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant ulong& byte_offset [[buffer(6)]],
    constant uint& active_threads [[buffer(7)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]
) {
    uint candidate_index = tgpig.x;
    uint base_row = candidate_index * PSIONIC_QUANTIZED_ARGMAX_ROWS_PER_THREADGROUP;
    if (base_row >= rows) {
        return;
    }
    threadgroup float partial[PSIONIC_QUANTIZED_ROW_THREADS];
    uint best_key = 0u;
    uint best_row = 0xffffffffu;
    uint block_count = columns / 256;
    for (uint local_row = 0; local_row < PSIONIC_QUANTIZED_ARGMAX_ROWS_PER_THREADGROUP; ++local_row) {
        uint row = base_row + local_row;
        if (row >= rows) {
            break;
        }
        const device uchar* row_weights = weights + byte_offset + (ulong(row) * ulong(row_stride));
        float sum = 0.0f;
        for (uint block_index = tid; block_index < block_count; block_index += active_threads) {
            const device uchar* block = row_weights + (block_index * 176);
            sum += psionic_q5_k_block_dot(block, input + block_index * 256);
        }
        partial[tid] = sum;
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (uint stride = active_threads / 2; stride > 0; stride >>= 1) {
            if (tid < stride) {
                partial[tid] += partial[tid + stride];
            }
            threadgroup_barrier(mem_flags::mem_threadgroup);
        }
        if (tid == 0) {
            uint candidate_key = psionic_ordered_float_key(partial[0]);
            if (psionic_argmax_candidate_better(candidate_key, row, best_key, best_row)) {
                best_key = candidate_key;
                best_row = row;
            }
        }
    }
    if (tid == 0) {
        uint base = candidate_index * 2;
        output[base] = best_key;
        output[base + 1] = best_row;
    }
}

kernel void psionic_quantized_matvec_q6_k(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& rows [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant ulong& byte_offset [[buffer(6)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    ushort lane [[thread_index_in_simdgroup]]
) {
    uint row = tgpig.x;
    if (row >= rows) {
        return;
    }
    const device uchar* row_weights = weights + byte_offset + (ulong(row) * ulong(row_stride));
    uint block_count = columns / 256;

    const short tid = short(lane / 2);
    const short ix = short(lane % 2);
    const short ip = short(tid / 8);
    const short il = short(tid % 8);
    const short l0 = short(4 * il);
    const short is = short(8 * ip + l0 / 16);
    const short y_offset = short(128 * ip + l0);
    const short q_offset_l = short(64 * ip + l0);
    const short q_offset_h = short(32 * ip + l0);

    float sum = 0.0f;

    for (uint block_index = uint(ix); block_index < block_count; block_index += 2) {
        const device uchar* block = row_weights + (block_index * 210);
        const device uchar* q1 = block + q_offset_l;
        const device uchar* q2 = q1 + 32;
        const device uchar* qh = block + 128 + q_offset_h;
        const device uchar* scales = block + 192 + is;
        ushort scale_bits = ushort(block[208]) | (ushort(block[209]) << 8);
        float scale = float(as_type<half>(scale_bits));
        const device float* block_input = input + block_index * 256 + y_offset;

        float4 sums = float4(0.0f);
        for (short l = 0; l < 4; ++l) {
            float y0 = block_input[l + 0];
            float y1 = block_input[l + 32];
            float y2 = block_input[l + 64];
            float y3 = block_input[l + 96];

            int qv0 = int(q1[l] & 0x0f) | (int((qh[l] >> 0) & 0x03) << 4);
            int qv1 = int(q2[l] & 0x0f) | (int((qh[l] >> 2) & 0x03) << 4);
            int qv2 = int(q1[l] >> 4) | (int((qh[l] >> 4) & 0x03) << 4);
            int qv3 = int(q2[l] >> 4) | (int((qh[l] >> 6) & 0x03) << 4);

            sums[0] += y0 * float(qv0 - 32);
            sums[1] += y1 * float(qv1 - 32);
            sums[2] += y2 * float(qv2 - 32);
            sums[3] += y3 * float(qv3 - 32);
        }

        sum += scale * (
            sums[0] * float(as_type<char>(scales[0])) +
            sums[1] * float(as_type<char>(scales[2])) +
            sums[2] * float(as_type<char>(scales[4])) +
            sums[3] * float(as_type<char>(scales[6]))
        );
    }

    float total = simd_sum(sum);
    if (lane == 0) {
        output[row] = total;
    }
}

kernel void psionic_quantized_matvec_argmax_q6_k(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device uint* output [[buffer(2)]],
    constant uint& rows [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant ulong& byte_offset [[buffer(6)]],
    constant uint& active_threads [[buffer(7)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]
) {
    uint candidate_index = tgpig.x;
    uint base_row = candidate_index * PSIONIC_QUANTIZED_ARGMAX_ROWS_PER_THREADGROUP;
    if (base_row >= rows) {
        return;
    }
    threadgroup float partial[PSIONIC_QUANTIZED_ROW_THREADS];
    uint best_key = 0u;
    uint best_row = 0xffffffffu;
    uint block_count = columns / 256;
    for (uint local_row = 0; local_row < PSIONIC_QUANTIZED_ARGMAX_ROWS_PER_THREADGROUP; ++local_row) {
        uint row = base_row + local_row;
        if (row >= rows) {
            break;
        }
        const device uchar* row_weights = weights + byte_offset + (ulong(row) * ulong(row_stride));
        float sum = 0.0f;
        for (uint block_index = tid; block_index < block_count; block_index += active_threads) {
            const device uchar* block = row_weights + (block_index * 210);
            sum += psionic_q6_k_block_dot(block, input + block_index * 256);
        }
        partial[tid] = sum;
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (uint stride = active_threads / 2; stride > 0; stride >>= 1) {
            if (tid < stride) {
                partial[tid] += partial[tid + stride];
            }
            threadgroup_barrier(mem_flags::mem_threadgroup);
        }
        if (tid == 0) {
            uint candidate_key = psionic_ordered_float_key(partial[0]);
            if (psionic_argmax_candidate_better(candidate_key, row, best_key, best_row)) {
                best_key = candidate_key;
                best_row = row;
            }
        }
    }
    if (tid == 0) {
        uint base = candidate_index * 2;
        output[base] = best_key;
        output[base + 1] = best_row;
    }
}

kernel void psionic_quantized_matvec_q8_0(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& rows [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant ulong& byte_offset [[buffer(6)]],
    constant uint& active_threads [[buffer(7)]],
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
    for (uint block_index = tid; block_index < block_count; block_index += active_threads) {
        const device uchar* block = weights + row_base + ulong(block_index) * 34ul;
        sum += psionic_q8_0_block_dot(block, input + block_index * 32);
    }
    partial[tid] = sum;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint stride = active_threads / 2; stride > 0; stride >>= 1) {
        if (tid < stride) {
            partial[tid] += partial[tid + stride];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (tid == 0) {
        output[row] = partial[0];
    }
}

kernel void psionic_quantized_matvec_argmax_q8_0(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device uint* output [[buffer(2)]],
    constant uint& rows [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant ulong& byte_offset [[buffer(6)]],
    constant uint& active_threads [[buffer(7)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]
) {
    uint candidate_index = tgpig.x;
    uint base_row = candidate_index * PSIONIC_QUANTIZED_ARGMAX_ROWS_PER_THREADGROUP;
    if (base_row >= rows) {
        return;
    }
    threadgroup float partial[PSIONIC_QUANTIZED_ROW_THREADS];
    uint block_count = columns / 32;
    uint best_key = 0u;
    uint best_row = 0xffffffffu;
    for (uint local_row = 0; local_row < PSIONIC_QUANTIZED_ARGMAX_ROWS_PER_THREADGROUP; ++local_row) {
        uint row = base_row + local_row;
        if (row >= rows) {
            break;
        }
        ulong row_base = byte_offset + ulong(row) * ulong(row_stride);
        float sum = 0.0f;
        for (uint block_index = tid; block_index < block_count; block_index += active_threads) {
            const device uchar* block = weights + row_base + ulong(block_index) * 34ul;
            sum += psionic_q8_0_block_dot(block, input + block_index * 32);
        }
        partial[tid] = sum;
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (uint stride = active_threads / 2; stride > 0; stride >>= 1) {
            if (tid < stride) {
                partial[tid] += partial[tid + stride];
            }
            threadgroup_barrier(mem_flags::mem_threadgroup);
        }
        if (tid == 0) {
            uint candidate_key = psionic_ordered_float_key(partial[0]);
            if (psionic_argmax_candidate_better(candidate_key, row, best_key, best_row)) {
                best_key = candidate_key;
                best_row = row;
            }
        }
    }
    if (tid == 0) {
        uint base = candidate_index * 2;
        output[base] = best_key;
        output[base + 1] = best_row;
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
    constant uint& active_threads [[buffer(7)]],
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
    for (uint block_index = tid; block_index < block_count; block_index += active_threads) {
        const device uchar* block = weights + row_base + ulong(block_index) * 17ul;
        sum += psionic_mxfp4_block_dot(block, input + block_index * 32);
    }
    partial[tid] = sum;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint stride = active_threads / 2; stride > 0; stride >>= 1) {
        if (tid < stride) {
            partial[tid] += partial[tid + stride];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (tid == 0) {
        output[row] = partial[0];
    }
}

kernel void psionic_quantized_matvec_argmax_mxfp4(
    const device uchar* weights [[buffer(0)]],
    const device float* input [[buffer(1)]],
    device uint* output [[buffer(2)]],
    constant uint& rows [[buffer(3)]],
    constant uint& columns [[buffer(4)]],
    constant uint& row_stride [[buffer(5)]],
    constant ulong& byte_offset [[buffer(6)]],
    constant uint& active_threads [[buffer(7)]],
    uint3 tgpig [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]
) {
    uint candidate_index = tgpig.x;
    uint base_row = candidate_index * PSIONIC_QUANTIZED_ARGMAX_ROWS_PER_THREADGROUP;
    if (base_row >= rows) {
        return;
    }
    threadgroup float partial[PSIONIC_QUANTIZED_ROW_THREADS];
    uint block_count = columns / 32;
    uint best_key = 0u;
    uint best_row = 0xffffffffu;
    for (uint local_row = 0; local_row < PSIONIC_QUANTIZED_ARGMAX_ROWS_PER_THREADGROUP; ++local_row) {
        uint row = base_row + local_row;
        if (row >= rows) {
            break;
        }
        ulong row_base = byte_offset + ulong(row) * ulong(row_stride);
        float sum = 0.0f;
        for (uint block_index = tid; block_index < block_count; block_index += active_threads) {
            const device uchar* block = weights + row_base + ulong(block_index) * 17ul;
            sum += psionic_mxfp4_block_dot(block, input + block_index * 32);
        }
        partial[tid] = sum;
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (uint stride = active_threads / 2; stride > 0; stride >>= 1) {
            if (tid < stride) {
                partial[tid] += partial[tid + stride];
            }
            threadgroup_barrier(mem_flags::mem_threadgroup);
        }
        if (tid == 0) {
            uint candidate_key = psionic_ordered_float_key(partial[0]);
            if (psionic_argmax_candidate_better(candidate_key, row, best_key, best_row)) {
                best_key = candidate_key;
                best_row = row;
            }
        }
    }
    if (tid == 0) {
        uint base = candidate_index * 2;
        output[base] = best_key;
        output[base + 1] = best_row;
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

kernel void psionic_mul_mv_id_q4_k(
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
    uint block_count = columns / 256;
    float sum = 0.0f;
    for (uint block_index = tid; block_index < block_count; block_index += PSIONIC_QUANTIZED_ROW_THREADS) {
        const device uchar* block = weights + row_base + ulong(block_index) * 144ul;
        sum += psionic_q4_k_block_dot(block, input + block_index * 256);
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

kernel void psionic_mul_mv_id_q5_0(
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
        const device uchar* block = weights + row_base + ulong(block_index) * 22ul;
        sum += psionic_q5_0_block_dot(block, input + block_index * 32);
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

kernel void psionic_expert_matvec_f32_ids_q4_k(
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
    uint block_count = columns / 256;
    const device float* input_row = input + ulong(selected_index) * ulong(columns);
    float sum = 0.0f;
    for (uint block_index = tid; block_index < block_count; block_index += PSIONIC_QUANTIZED_ROW_THREADS) {
        const device uchar* block = weights + row_base + ulong(block_index) * 144ul;
        sum += psionic_q4_k_block_dot(block, input_row + block_index * 256);
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

kernel void psionic_expert_matvec_f32_ids_q5_0(
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
        const device uchar* block = weights + row_base + ulong(block_index) * 22ul;
        sum += psionic_q5_0_block_dot(block, input_row + block_index * 32);
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
        MetalBuffer, MetalCommandStatus, MetalCommandWait, MetalDiscoveryReport,
        MetalKvCacheMirror, MetalStorageMode, RuntimeError,
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
            _byte_offset: usize,
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
            _source_byte_offset: usize,
            _destination: &PlatformBuffer,
            _destination_byte_offset: usize,
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

        pub(super) fn encode_gelu_glu_f32(
            &mut self,
            _pipeline: &(),
            _gate: &PlatformBuffer,
            _up: &PlatformBuffer,
            _output: &PlatformBuffer,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_add_inplace(
            &mut self,
            _pipeline: &(),
            _values: &PlatformBuffer,
            _bias: &PlatformBuffer,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_scale_inplace(
            &mut self,
            _pipeline: &(),
            _values: &PlatformBuffer,
            _scale: f32,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_copy_f32_with_offset(
            &mut self,
            _pipeline: &(),
            _source: &PlatformBuffer,
            _destination: &PlatformBuffer,
            _element_count: usize,
            _destination_offset_elements: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_per_head_rms_norm(
            &mut self,
            _pipeline: &(),
            _values: &PlatformBuffer,
            _weight: &PlatformBuffer,
            _head_count: usize,
            _head_dim: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_per_head_rms_norm_to_output(
            &mut self,
            _pipeline: &(),
            _input: &PlatformBuffer,
            _weight: &PlatformBuffer,
            _output: &PlatformBuffer,
            _head_count: usize,
            _head_dim: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_per_head_rms_norm_unit(
            &mut self,
            _pipeline: &(),
            _values: &PlatformBuffer,
            _head_count: usize,
            _head_dim: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_rope_neox_inplace(
            &mut self,
            _pipeline: &(),
            _values: &PlatformBuffer,
            _cos: &PlatformBuffer,
            _sin: &PlatformBuffer,
            _head_count: usize,
            _head_dim: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_rope_neox_position(
            &mut self,
            _pipeline: &(),
            _values: &PlatformBuffer,
            _freq_factors: &PlatformBuffer,
            _head_count: usize,
            _head_dim: usize,
            _rotary_half: usize,
            _position: usize,
            _theta_scale: f32,
            _freq_scale: f32,
            _corr_dims: [f32; 2],
            _ext_factor: f32,
            _yarn_mscale: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_decode_attention_dense(
            &mut self,
            _pipeline: &(),
            _query: &PlatformBuffer,
            _key_cache: &PlatformBuffer,
            _value_cache: &PlatformBuffer,
            _output: &PlatformBuffer,
            _query_head_count: usize,
            _kv_head_count: usize,
            _token_count: usize,
            _head_dim: usize,
            _scale: f32,
            _active_simdgroups: usize,
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

        pub(super) fn encode_matmul_with_dims(
            &mut self,
            _submission: &mut PlatformSubmission,
            _left: &MetalBuffer,
            _right: &MetalBuffer,
            _output: &MetalBuffer,
            _m: usize,
            _k: usize,
            _n: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_gelu_glu_f32(
            &mut self,
            _submission: &mut PlatformSubmission,
            _gate: &MetalBuffer,
            _up: &MetalBuffer,
            _output: &MetalBuffer,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_add_inplace(
            &mut self,
            _submission: &mut PlatformSubmission,
            _values: &MetalBuffer,
            _bias: &MetalBuffer,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_scale_inplace(
            &mut self,
            _submission: &mut PlatformSubmission,
            _values: &MetalBuffer,
            _scale: f32,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_per_head_rms_norm(
            &mut self,
            _submission: &mut PlatformSubmission,
            _values: &MetalBuffer,
            _weight: &MetalBuffer,
            _head_count: usize,
            _head_dim: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_per_head_rms_norm_to_output(
            &mut self,
            _submission: &mut PlatformSubmission,
            _input: &MetalBuffer,
            _weight: &MetalBuffer,
            _output: &MetalBuffer,
            _head_count: usize,
            _head_dim: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_per_head_rms_norm_unit(
            &mut self,
            _submission: &mut PlatformSubmission,
            _values: &MetalBuffer,
            _head_count: usize,
            _head_dim: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_rope_neox_inplace(
            &mut self,
            _submission: &mut PlatformSubmission,
            _values: &MetalBuffer,
            _cos: &MetalBuffer,
            _sin: &MetalBuffer,
            _head_count: usize,
            _head_dim: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_rope_neox_position(
            &mut self,
            _submission: &mut PlatformSubmission,
            _values: &MetalBuffer,
            _freq_factors: &MetalBuffer,
            _head_count: usize,
            _head_dim: usize,
            _rotary_half: usize,
            _position: usize,
            _theta_scale: f32,
            _freq_scale: f32,
            _corr_dims: [f32; 2],
            _ext_factor: f32,
            _yarn_mscale: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_copy_f32_with_offset(
            &mut self,
            _submission: &mut PlatformSubmission,
            _source: &MetalBuffer,
            _destination: &MetalBuffer,
            _element_count: usize,
            _destination_offset_elements: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_copy_f32_slice(
            &mut self,
            _submission: &mut PlatformSubmission,
            _source: &MetalBuffer,
            _destination: &MetalBuffer,
            _element_count: usize,
            _source_offset_elements: usize,
            _destination_offset_elements: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_decode_attention_dense(
            &mut self,
            _submission: &mut PlatformSubmission,
            _query: &MetalBuffer,
            _cache: &MetalKvCacheMirror,
            _query_head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _scale: f32,
            _output: &MetalBuffer,
            _active_simdgroups: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_argmax_f32(
            &mut self,
            _submission: &mut PlatformSubmission,
            _input: &MetalBuffer,
            _output: &MetalBuffer,
            _row_count: usize,
            _column_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_argmax_candidates(
            &mut self,
            _submission: &mut PlatformSubmission,
            _input: &MetalBuffer,
            _output: &MetalBuffer,
            _candidate_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_top_k_single_row_f32(
            &mut self,
            _submission: &mut PlatformSubmission,
            _input: &MetalBuffer,
            _selected_ids: &MetalBuffer,
            _selected_values: &MetalBuffer,
            _column_count: usize,
            _top_k: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_top_k_softmax_single_row_f32(
            &mut self,
            _submission: &mut PlatformSubmission,
            _input: &MetalBuffer,
            _selected_ids: &MetalBuffer,
            _selected_weights: &MetalBuffer,
            _column_count: usize,
            _top_k: usize,
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

        pub(super) fn encode_quantized_matvec_argmax(
            &mut self,
            _submission: &mut PlatformSubmission,
            _weights: &MetalBuffer,
            _byte_offset: usize,
            _mode: QuantizationMode,
            _rows: usize,
            _columns: usize,
            _input: &MetalBuffer,
            _selected: &MetalBuffer,
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

        pub(super) fn encode_grouped_quantized_matvec_buffer_ids(
            &mut self,
            _submission: &mut PlatformSubmission,
            _weights: &MetalBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows_per_expert: usize,
            _columns: usize,
            _selected_ids: &MetalBuffer,
            _selected_count: usize,
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

        pub(super) fn encode_expert_matvec_f32_ids_buffer(
            &mut self,
            _submission: &mut PlatformSubmission,
            _weights: &MetalBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows_per_expert: usize,
            _columns: usize,
            _selected_ids: &MetalBuffer,
            _selected_count: usize,
            _input: &MetalBuffer,
            _output: &MetalBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_add_selected_expert_bias_f32(
            &mut self,
            _submission: &mut PlatformSubmission,
            _values: &MetalBuffer,
            _bias: &MetalBuffer,
            _selected_ids: &MetalBuffer,
            _rows_per_expert: usize,
            _selected_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_aggregate_selected_expert_rows_f32(
            &mut self,
            _submission: &mut PlatformSubmission,
            _input: &MetalBuffer,
            _selected_ids: &MetalBuffer,
            _selected_weights: &MetalBuffer,
            _expert_scales: &MetalBuffer,
            _output: &MetalBuffer,
            _rows_per_expert: usize,
            _selected_count: usize,
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
    use psionic_backend_tests::{GraphBackendConformanceHarness, run_graph_backend_conformance};
    use psionic_compiler::compile_graph;
    use psionic_core::{
        BackendExtensionKind, DType, Device, DeviceKind, QuantizationMode, QuantizedTensorData,
        Shape, TensorSpec,
    };
    use psionic_ir::{Graph, GraphBuilder};
    use psionic_runtime::{
        Allocator, BackendDegradedPolicy, BackendParityPolicy, BackendSelectionState, BufferHandle,
        BufferResidency, BufferStorageKind, CacheAction, CacheKind, CompilePathTemperature,
        DeviceDiscovery, ExecutionResult, HealthStatus, KvCacheAccounting, KvCacheEncodingFamily,
        KvCacheEncodingObjective, KvCacheEncodingPolicy, KvCachePageLayout, KvCacheState,
        PrefixCacheState, QuantizationExecution, QuantizationLoadPath, QuantizationSupport,
        RuntimeError, ServedProductBackendPolicy,
    };

    use super::{
        DeviceSupportTier, EMBEDDINGS_SUPPORTED_OPS, FamilySupport, GGML_Q8_1_BLOCK_ELEMENTS,
        MetalAttentionGraphReserve, MetalBackend, MetalBuffer, MetalGraphReserveKind,
        MetalPromptResidencyMetrics, MetalSharedPrefixCompatibility, MetalSharedPrefixStore,
        TEXT_GENERATION_SUPPORTED_OPS, classify_support, ggml_q8_1_storage_bytes,
        validate_quantized_storage, validate_supported_plan,
    };

    impl GraphBackendConformanceHarness for MetalBackend {
        type Buffer = MetalBuffer;

        fn backend_selection(
            &self,
            supported_ops: &[&str],
        ) -> Result<psionic_runtime::BackendSelection, RuntimeError> {
            MetalBackend::backend_selection(self, supported_ops)
        }

        fn input_buffer(
            &mut self,
            shape: Shape,
            values: Vec<f32>,
        ) -> Result<Self::Buffer, RuntimeError> {
            MetalBackend::input_buffer(self, shape, values)
        }

        fn compile_and_execute(
            &mut self,
            graph: &Graph,
            inputs: &std::collections::BTreeMap<psionic_core::TensorId, Self::Buffer>,
        ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError> {
            MetalBackend::compile_and_execute(self, graph, inputs)
        }

        fn dense_values(&self, buffer: &Self::Buffer) -> Result<Vec<f32>, RuntimeError> {
            buffer.read_f32()
        }

        fn known_unsupported_case(&mut self) -> Result<String, RuntimeError> {
            let selected = self.selected_device().cloned().ok_or_else(|| {
                RuntimeError::Backend(String::from("no metal device for refusal case"))
            })?;
            let mut builder = GraphBuilder::new(selected.device);
            let input = builder.input("features", Shape::new(vec![1, 2]), DType::F32);
            let weights = builder
                .constant_f32(Shape::new(vec![1, 2]), vec![1.0, 0.0])
                .map_err(|error| RuntimeError::Backend(error.to_string()))?;
            let unsupported = builder
                .mul(&input, &weights)
                .map_err(|error| RuntimeError::Backend(error.to_string()))?;
            let graph = builder.finish(vec![unsupported]);
            let plan =
                compile_graph(&graph).map_err(|error| RuntimeError::Backend(error.to_string()))?;
            match validate_supported_plan(&plan) {
                Err(RuntimeError::UnsupportedStep(step)) if step == "mul" => Ok(String::from(
                    "mul remained an explicit unsupported-step refusal at Metal plan validation",
                )),
                Err(other) => Err(RuntimeError::Backend(format!(
                    "expected Metal mul refusal, found {other}"
                ))),
                Ok(()) => Err(RuntimeError::Backend(String::from(
                    "Metal mul validation succeeded instead of refusing",
                ))),
            }
        }
    }

    #[test]
    fn metal_backend_shared_conformance_harness_has_no_failures() {
        let mut backend = MetalBackend::new();
        let report = run_graph_backend_conformance(&mut backend);
        assert_eq!(report.backend, "metal");
        assert_eq!(report.surface, "graph_execution");
        assert!(!report.has_failures(), "{report:?}");
    }

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

    fn sample_q5_0_row(scale: f32, offset: u8) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(22);
        bytes.extend_from_slice(&f32_to_f16_bits(scale).to_le_bytes());
        let mut qh = 0_u32;
        for pair_index in 0..16_u8 {
            if pair_index % 3 == 0 {
                qh |= 1_u32 << pair_index;
            }
            if pair_index % 5 == 0 {
                qh |= 1_u32 << (u32::from(pair_index) + 12);
            }
        }
        bytes.extend_from_slice(&qh.to_le_bytes());
        for pair_index in 0..16_u8 {
            let low = offset.wrapping_add(pair_index) & 0x0f;
            let high = offset.wrapping_add(pair_index.wrapping_mul(3)) & 0x0f;
            bytes.push(low | (high << 4));
        }
        bytes
    }

    fn sample_q5_k_row(scale: f32, minimum: f32, offset: u8) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(176);
        bytes.extend_from_slice(&f32_to_f16_bits(scale).to_le_bytes());
        bytes.extend_from_slice(&f32_to_f16_bits(minimum).to_le_bytes());
        for index in 0..12_u8 {
            bytes.push(offset.wrapping_add(index.wrapping_mul(5)));
        }
        for index in 0..32_u8 {
            let b0 = (index & 0x03) << 0;
            let b1 = ((index.wrapping_add(1)) & 0x03) << 2;
            let b2 = ((index.wrapping_add(2)) & 0x03) << 4;
            let b3 = ((index.wrapping_add(3)) & 0x03) << 6;
            bytes.push(b0 | b1 | b2 | b3);
        }
        for index in 0..128_u8 {
            let low = index & 0x0f;
            let high = (index.wrapping_mul(3)) & 0x0f;
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

    fn sample_reference_vector() -> Vec<f32> {
        (0..32).map(|index| (index as f32 + 1.0) * 0.25).collect()
    }

    fn sample_reference_vector_256() -> Vec<f32> {
        (0..256)
            .map(|index| (index as f32 + 1.0) * 0.03125)
            .collect()
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
        kv_cache_encoding_policy: KvCacheEncodingPolicy,
    ) -> MetalSharedPrefixCompatibility {
        let row_byte_len = match kv_cache_encoding_policy.family {
            KvCacheEncodingFamily::TurboQuant => ggml_q8_1_storage_bytes(width)
                .expect("sample TurboQuant policy requires block-aligned width"),
            KvCacheEncodingFamily::DenseF32 | KvCacheEncodingFamily::DenseF16Mirror => {
                width.saturating_mul(std::mem::size_of::<f32>())
            }
        };
        MetalSharedPrefixCompatibility {
            served_artifact_digest: String::from("metal-artifact"),
            model_id: String::from("gpt-oss"),
            model_revision: String::from("20b"),
            weight_bundle_digest: String::from("weights-digest"),
            tokenizer_family: String::from("cl100k"),
            tenant_id: None,
            sampler_digest: None,
            backend_compatibility: String::from("metal-apple"),
            kv_width: width,
            page_layout: KvCachePageLayout::new(
                max_context_tokens,
                4,
                row_byte_len.saturating_mul(2),
            ),
            kv_cache_encoding_policy,
        }
    }

    fn sample_dense_kv_policy(width: usize, max_context_tokens: usize) -> KvCacheEncodingPolicy {
        KvCacheEncodingPolicy::dense_f32(
            width
                .saturating_mul(2)
                .saturating_mul(std::mem::size_of::<f32>())
                .try_into()
                .unwrap_or(u64::MAX),
            "gpt-oss",
            max_context_tokens,
        )
        .with_detail("test dense metal kv policy")
    }

    fn sample_turboquant_kv_policy(
        width: usize,
        max_context_tokens: usize,
    ) -> KvCacheEncodingPolicy {
        KvCacheEncodingPolicy {
            family: KvCacheEncodingFamily::TurboQuant,
            objective: Some(KvCacheEncodingObjective::MeanSquaredError),
            bits_per_channel: Some(8),
            block_shape: Some(GGML_Q8_1_BLOCK_ELEMENTS.to_string()),
            outlier_policy: None,
            projection_id: None,
            codebook_id: Some(String::from("ggml_q8_1")),
            model_family_bound: Some(String::from("gpt-oss")),
            context_length_bound: Some(max_context_tokens),
            host_bytes_per_token: Some(
                width
                    .saturating_mul(2)
                    .saturating_mul(std::mem::size_of::<f32>())
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            device_bytes_per_token: ggml_q8_1_storage_bytes(width)
                .ok()
                .and_then(|row_bytes| row_bytes.checked_mul(2))
                .map(|bytes| bytes as u64),
            detail: Some(String::from("test TurboQuant metal kv policy")),
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
                "top_k_single_row_f32",
                "mul_mv_id_q5_0",
                "mul_mv_id_q4_k",
                "mul_mv_id_q8_0",
                "mul_mv_id_mxfp4",
                "expert_matvec_f32_ids_q5_0",
                "expert_matvec_f32_ids_q4_k",
                "expert_matvec_f32_ids_q8_0",
                "expert_matvec_f32_ids_mxfp4",
                "top_k_softmax_single_row_f32",
                "add_selected_expert_bias_f32",
                "aggregate_selected_expert_rows_f32",
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
                    mode: QuantizationMode::GgmlQ5_0,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::DequantizeToF32,
                },
                QuantizationSupport {
                    mode: QuantizationMode::GgmlQ4K,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::DequantizeToF32,
                },
                QuantizationSupport {
                    mode: QuantizationMode::GgmlQ5K,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::DequantizeToF32,
                },
                QuantizationSupport {
                    mode: QuantizationMode::GgmlQ6K,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::DequantizeToF32,
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
    fn metal_backend_single_row_top_k_selection_returns_raw_logits_on_supported_hardware()
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
            super::MetalLogitsOutputMode::TopKCandidates(2),
        )?;
        assert_eq!(selection.selected_tokens, vec![2]);
        assert_eq!(
            selection
                .candidates
                .as_ref()
                .map(|value| value.indices.clone()),
            Some(vec![2, 3])
        );
        assert_eq!(
            selection
                .candidates
                .as_ref()
                .map(|value| value.values.clone()),
            Some(vec![4.25, 3.0])
        );
        assert_eq!(
            selection.metrics.output_mode,
            super::MetalLogitsOutputMode::TopKCandidates(2)
        );
        assert_eq!(selection.metrics.readback_bytes, 16);
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

        let mut cache = backend.kv_cache_mirror_from_host_rows(
            4,
            8,
            0,
            &[],
            &[],
            4,
            sample_dense_kv_policy(4, 8),
        )?;
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
        let mut cache = backend.kv_cache_mirror_from_host_rows(
            4,
            8,
            0,
            &[],
            &[],
            4,
            sample_dense_kv_policy(4, 8),
        )?;
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
    fn metal_backend_executes_q4_k_quantized_matvec_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let row_a = sample_q4_k_row(0.5, 0.125, 3);
        let row_b = sample_q4_k_row(0.75, 0.25, 9);
        let reference = sample_reference_vector_256();
        let expected_a =
            psionic_backend_cpu::quantized_row_dot(&reference, QuantizationMode::GgmlQ4K, &row_a)?;
        let expected_b =
            psionic_backend_cpu::quantized_row_dot(&reference, QuantizationMode::GgmlQ4K, &row_b)?;

        let weights = backend.quantized_buffer(
            Shape::new(vec![2, 256]),
            QuantizationMode::GgmlQ4K,
            [row_a, row_b].concat(),
        )?;
        let values =
            backend.quantized_matvec(&weights, QuantizationMode::GgmlQ4K, 2, 256, &reference)?;
        assert_close(values.as_slice(), &[expected_a, expected_b], 1.0e-4);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_executes_q5_0_quantized_matvec_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let row_a = sample_q5_0_row(0.375, 3);
        let row_b = sample_q5_0_row(0.625, 7);
        let reference = sample_reference_vector();
        let expected_a =
            psionic_backend_cpu::quantized_row_dot(&reference, QuantizationMode::GgmlQ5_0, &row_a)?;
        let expected_b =
            psionic_backend_cpu::quantized_row_dot(&reference, QuantizationMode::GgmlQ5_0, &row_b)?;

        let weights = backend.quantized_buffer(
            Shape::new(vec![2, 32]),
            QuantizationMode::GgmlQ5_0,
            [row_a, row_b].concat(),
        )?;
        let values =
            backend.quantized_matvec(&weights, QuantizationMode::GgmlQ5_0, 2, 32, &reference)?;
        assert_close(values.as_slice(), &[expected_a, expected_b], 1.0e-4);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_executes_q6_k_quantized_matvec_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let row_a = sample_q6_k_row(0.375, -3);
        let row_b = sample_q6_k_row(0.625, 4);
        let reference = sample_reference_vector_256();
        let expected_a =
            psionic_backend_cpu::quantized_row_dot(&reference, QuantizationMode::GgmlQ6K, &row_a)?;
        let expected_b =
            psionic_backend_cpu::quantized_row_dot(&reference, QuantizationMode::GgmlQ6K, &row_b)?;

        let weights = backend.quantized_buffer(
            Shape::new(vec![2, 256]),
            QuantizationMode::GgmlQ6K,
            [row_a, row_b].concat(),
        )?;
        let values =
            backend.quantized_matvec(&weights, QuantizationMode::GgmlQ6K, 2, 256, &reference)?;
        assert_close(values.as_slice(), &[expected_a, expected_b], 1.0e-4);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_executes_q5_k_quantized_matvec_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let row_a = sample_q5_k_row(0.375, 0.0625, 3);
        let row_b = sample_q5_k_row(0.625, 0.125, 7);
        let reference = sample_reference_vector_256();
        let expected_a =
            psionic_backend_cpu::quantized_row_dot(&reference, QuantizationMode::GgmlQ5K, &row_a)?;
        let expected_b =
            psionic_backend_cpu::quantized_row_dot(&reference, QuantizationMode::GgmlQ5K, &row_b)?;

        let weights = backend.quantized_buffer(
            Shape::new(vec![2, 256]),
            QuantizationMode::GgmlQ5K,
            [row_a, row_b].concat(),
        )?;
        let values =
            backend.quantized_matvec(&weights, QuantizationMode::GgmlQ5K, 2, 256, &reference)?;
        assert_close(values.as_slice(), &[expected_a, expected_b], 1.0e-4);
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
    fn metal_backend_mul_mv_id_matches_grouped_q4_k_reference_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let rows_per_expert = 2;
        let expert_count = 3;
        let columns = 256;
        let row_stride = 144;
        let selected_ids = vec![2_i32, 0_i32];
        let weights = [
            sample_q4_k_row(0.5, 0.125, 3),
            sample_q4_k_row(0.75, 0.25, 9),
            sample_q4_k_row(0.625, 0.1875, 5),
            sample_q4_k_row(0.875, 0.3125, 11),
            sample_q4_k_row(1.0, 0.4375, 7),
            sample_q4_k_row(0.5625, 0.0625, 13),
        ]
        .concat();
        let input = sample_reference_vector_256();

        let weight_buffer = backend.quantized_buffer(
            Shape::new(vec![expert_count * rows_per_expert, columns]),
            QuantizationMode::GgmlQ4K,
            weights.clone(),
        )?;
        let input_buffer = backend.input_buffer(Shape::new(vec![columns]), input.clone())?;
        let result = backend.mul_mv_id(
            &weight_buffer,
            QuantizationMode::GgmlQ4K,
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
            QuantizationMode::GgmlQ4K,
            row_stride,
            rows_per_expert,
            selected_ids.as_slice(),
            input.as_slice(),
            weights.as_slice(),
        )?;
        assert_eq!(result.values.len(), expected.len());
        for (actual, expected) in result.values.iter().zip(expected.iter()) {
            assert!((actual - expected).abs() <= 1.0e-2);
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_mul_mv_id_matches_grouped_q5_0_reference_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let rows_per_expert = 2;
        let expert_count = 3;
        let columns = 32;
        let row_stride = 22;
        let selected_ids = vec![2_i32, 0_i32];
        let weights = [
            sample_q5_0_row(0.25, 1),
            sample_q5_0_row(0.5, 3),
            sample_q5_0_row(0.125, 5),
            sample_q5_0_row(0.375, 7),
            sample_q5_0_row(0.625, 9),
            sample_q5_0_row(0.75, 11),
        ]
        .concat();
        let input = sample_reference_vector();

        let weight_buffer = backend.quantized_buffer(
            Shape::new(vec![expert_count * rows_per_expert, columns]),
            QuantizationMode::GgmlQ5_0,
            weights.clone(),
        )?;
        let input_buffer = backend.input_buffer(Shape::new(vec![columns]), input.clone())?;
        let result = backend.mul_mv_id(
            &weight_buffer,
            QuantizationMode::GgmlQ5_0,
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
            QuantizationMode::GgmlQ5_0,
            row_stride,
            rows_per_expert,
            selected_ids.as_slice(),
            input.as_slice(),
            weights.as_slice(),
        )?;
        assert_eq!(result.values.len(), expected.len());
        for (actual, expected) in result.values.iter().zip(expected.iter()) {
            assert!((actual - expected).abs() <= 1.0e-4);
        }
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
    fn metal_backend_expert_matvec_f32_ids_matches_grouped_q4_k_reference_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let rows_per_expert = 2;
        let expert_count = 3;
        let columns = 256;
        let row_stride = 144;
        let selected_ids = vec![2_i32, 0_i32];
        let weights = [
            sample_q4_k_row(0.5, 0.125, 3),
            sample_q4_k_row(0.75, 0.25, 9),
            sample_q4_k_row(0.625, 0.1875, 5),
            sample_q4_k_row(0.875, 0.3125, 11),
            sample_q4_k_row(1.0, 0.4375, 7),
            sample_q4_k_row(0.5625, 0.0625, 13),
        ]
        .concat();
        let inputs = [sample_reference_vector_256(), vec![0.5; columns]].concat();

        let weight_buffer = backend.quantized_buffer(
            Shape::new(vec![expert_count * rows_per_expert, columns]),
            QuantizationMode::GgmlQ4K,
            weights.clone(),
        )?;
        let input_buffer = backend.input_buffer(
            Shape::new(vec![selected_ids.len(), columns]),
            inputs.clone(),
        )?;
        let result = backend.expert_matvec_f32_ids(
            &weight_buffer,
            QuantizationMode::GgmlQ4K,
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
            QuantizationMode::GgmlQ4K,
            row_stride,
            rows_per_expert,
            selected_ids.as_slice(),
            inputs.as_slice(),
            columns,
            weights.as_slice(),
        )?;
        assert_eq!(result.values.len(), expected.len());
        for (actual, expected) in result.values.iter().zip(expected.iter()) {
            assert!((actual - expected).abs() <= 1.0e-2);
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_expert_matvec_f32_ids_matches_grouped_q5_0_reference_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let rows_per_expert = 2;
        let expert_count = 3;
        let columns = 32;
        let row_stride = 22;
        let selected_ids = vec![2_i32, 0_i32];
        let weights = [
            sample_q5_0_row(0.25, 1),
            sample_q5_0_row(0.5, 3),
            sample_q5_0_row(0.125, 5),
            sample_q5_0_row(0.375, 7),
            sample_q5_0_row(0.625, 9),
            sample_q5_0_row(0.75, 11),
        ]
        .concat();
        let inputs = [sample_reference_vector(), vec![0.5; columns]].concat();

        let weight_buffer = backend.quantized_buffer(
            Shape::new(vec![expert_count * rows_per_expert, columns]),
            QuantizationMode::GgmlQ5_0,
            weights.clone(),
        )?;
        let input_buffer = backend.input_buffer(
            Shape::new(vec![selected_ids.len(), columns]),
            inputs.clone(),
        )?;
        let result = backend.expert_matvec_f32_ids(
            &weight_buffer,
            QuantizationMode::GgmlQ5_0,
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
            QuantizationMode::GgmlQ5_0,
            row_stride,
            rows_per_expert,
            selected_ids.as_slice(),
            inputs.as_slice(),
            columns,
            weights.as_slice(),
        )?;
        assert_eq!(result.values.len(), expected.len());
        for (actual, expected) in result.values.iter().zip(expected.iter()) {
            assert!((actual - expected).abs() <= 1.0e-4);
        }
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
            sample_dense_kv_policy(width, max_context_tokens),
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
    fn metal_kv_cache_mirror_roundtrips_turboquant_entries_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let width = 32;
        let max_context_tokens = 64;
        let policy = sample_turboquant_kv_policy(width, max_context_tokens);
        let keys = (0..(width * 2))
            .map(|index| (index as f32 - 16.0) / 4.0)
            .collect::<Vec<_>>();
        let values = (0..(width * 2))
            .map(|index| (index as f32 - 8.0) / 3.0)
            .collect::<Vec<_>>();
        let mut mirror = backend.kv_cache_mirror_from_host_rows(
            width,
            max_context_tokens,
            2,
            keys.as_slice(),
            values.as_slice(),
            4,
            policy.clone(),
        )?;

        assert_eq!(mirror.kv_cache_encoding_policy(), &policy);
        assert_eq!(
            mirror.page_layout(),
            KvCachePageLayout::new(
                max_context_tokens,
                4,
                ggml_q8_1_storage_bytes(width)?.saturating_mul(2),
            )
        );
        let (second_key, second_value) = mirror.read_entry(1)?;
        assert_close(second_key.as_slice(), &keys[width..], 0.1);
        assert_close(second_value.as_slice(), &values[width..], 0.1);

        let append_key = (0..width)
            .map(|index| (index as f32 - 10.0) / 5.0)
            .collect::<Vec<_>>();
        let append_value = (0..width)
            .map(|index| (index as f32 - 5.0) / 6.0)
            .collect::<Vec<_>>();
        let write_index =
            mirror.append_entry(&mut backend, append_key.as_slice(), append_value.as_slice())?;
        assert_eq!(write_index, 2);
        let (stored_key, stored_value) = mirror.read_entry(write_index)?;
        assert_close(stored_key.as_slice(), append_key.as_slice(), 0.1);
        assert_close(stored_value.as_slice(), append_value.as_slice(), 0.1);
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
        let compatibility = sample_prefix_compatibility(
            width,
            max_context_tokens,
            sample_dense_kv_policy(width, max_context_tokens),
        );
        let cache = backend.kv_cache_mirror_from_host_rows(
            width,
            max_context_tokens,
            3,
            &[1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0, 6.5],
            &[
                10.0, 10.5, 11.0, 11.5, 12.0, 12.5, 13.0, 13.5, 14.0, 14.5, 15.0, 15.5,
            ],
            2,
            sample_dense_kv_policy(width, max_context_tokens),
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
    fn metal_shared_prefix_store_rejects_mismatched_kv_cache_encoding_policy_on_supported_hardware()
    -> Result<(), RuntimeError> {
        let mut backend = MetalBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let width = 32;
        let max_context_tokens = 64;
        let dense_policy = sample_dense_kv_policy(width, max_context_tokens);
        let turboquant_policy = sample_turboquant_kv_policy(width, max_context_tokens);
        let dense_compatibility =
            sample_prefix_compatibility(width, max_context_tokens, dense_policy.clone());
        let turboquant_compatibility =
            sample_prefix_compatibility(width, max_context_tokens, turboquant_policy);
        let keys = (0..(width * 3))
            .map(|index| (index as f32 + 1.0) / 8.0)
            .collect::<Vec<_>>();
        let values = (0..(width * 3))
            .map(|index| (index as f32 + 4.0) / 7.0)
            .collect::<Vec<_>>();
        let cache = backend.kv_cache_mirror_from_host_rows(
            width,
            max_context_tokens,
            3,
            keys.as_slice(),
            values.as_slice(),
            2,
            dense_policy,
        )?;

        let mut store = MetalSharedPrefixStore::default();
        store.record(dense_compatibility, &[1, 2, 3], &cache);

        let lookup = store.lookup(&turboquant_compatibility, &[1, 2, 3, 4]);
        assert_eq!(lookup.state, PrefixCacheState::None);
        assert_eq!(lookup.reused_tokens, 0);
        assert!(lookup.identity.is_none());
        assert!(lookup.cache.is_none());
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
        let compatibility = sample_prefix_compatibility(
            width,
            max_context_tokens,
            sample_dense_kv_policy(width, max_context_tokens),
        );
        let stale_cache = backend.kv_cache_mirror_from_host_rows(
            width,
            max_context_tokens,
            2,
            &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
            &[10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0],
            2,
            sample_dense_kv_policy(width, max_context_tokens),
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
