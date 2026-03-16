//! Public lazy array facade above `psionic-core` and `psionic-ir`.
//!
//! This first surface is intentionally narrow. It establishes a user-facing
//! lazy array handle, public device and stream contracts, graph-backed
//! arithmetic, and explicit evaluation semantics above the lower graph builder
//! substrate without claiming full runtime scheduling or broader MLX-class
//! array closure yet.

use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    fs,
    path::PathBuf,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use psionic_compiler::{
    CompileTransformConfig, CompileTransformDebugMode, CompileTransformError,
    CompileTransformResult, CompileTransformTraceMode, compile_transform,
};
use psionic_core::{
    DType, Device, DeviceKind, LazyOp, PsionicRefusal, Shape, Tensor, TensorData, TensorId,
    TensorSpec,
};
use psionic_ir::{
    BackendPluginExtensionContract, CustomKernelExtensionContract, CustomOpExtensionContract,
    ExtensibleOperatorRegistry, ExtensionContractKind, ExtensionContractSemanticsReport, Graph,
    GraphBuilder, GraphError, KernelRegistration, MetaTensor, OpKind, OperatorDispatchContract,
    QuantizerPluginExtensionContract, RegisteredOperatorSchema, RegistryExtensionError,
    builtin_extension_contract_semantics_report,
};
use psionic_runtime::{
    AllocatorPoolPolicy, AllocatorPoolReport, AllocatorPoolState, BackendHealthTracker,
    BackendProbeState, BackendRuntimeResources, BackendToolchainIdentity, CacheInvalidationPolicy,
    DeterminismContractError, DeviceDescriptor, DeviceInventoryQualifiers, DeviceMemoryBudget,
    DeviceMemoryClass, ExecutionCapabilityProfile, ExecutionPlanCachePolicy,
    ExecutionPlanCacheReport, ExecutionPlanCacheState, GeneratorState, HealthStatus,
    IsolationResetScope, KernelCachePolicy, KernelCacheReport, KernelCacheState,
    LocalRuntimeObservability, LocalServingIsolationPolicy, MemoryResidencySnapshot,
    RuntimeDeterminismContract, RuntimeHealth, default_cache_invalidation_policy,
};
use rand::{Rng, SeedableRng, rngs::StdRng};
use serde::Serialize;
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "public lazy array facade above psionic-core and psionic-ir";

const DEFAULT_EXECUTION_PLAN_CACHE_ENTRIES: usize = 16;
const DEFAULT_EXECUTION_PLAN_CACHE_BYTES: u64 = 512 * 1024;
const DEFAULT_ALLOCATOR_POOL_BUFFERS: usize = 16;
const DEFAULT_ALLOCATOR_POOL_BYTES: u64 = 512 * 1024;
const DEFAULT_KERNEL_CACHE_ENTRIES: usize = 16;
const DEFAULT_KERNEL_CACHE_BYTES: u64 = 256 * 1024;
const DEFAULT_DEBUG_LOG_HISTORY_LIMIT: usize = 32;
const DEFAULT_DEBUG_CAPTURE_HISTORY_LIMIT: usize = 8;

#[derive(Debug)]
struct ArrayContextInner {
    device: ArrayDevice,
    backend_identity: BackendToolchainIdentity,
    builder: RefCell<GraphBuilder>,
    determinism: RefCell<RuntimeDeterminismContract>,
    extension_state: RefCell<ArrayExtensionState>,
    next_stream_id: RefCell<u32>,
    runtime_state: RefCell<ArrayRuntimeState>,
    debug_state: RefCell<ArrayDebugState>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ArrayRuntimeState {
    total_bytes: u64,
    peak_bytes: u64,
    tensor_bytes: BTreeMap<TensorId, u64>,
    runtime_resources: BackendRuntimeResources,
    cache_invalidation_policy: CacheInvalidationPolicy,
    execution_plan_cache: VecDeque<ArrayCacheEntry>,
    allocator_pool: VecDeque<ArrayAllocatorEntry>,
    kernel_cache: VecDeque<ArrayCacheEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ArrayCacheEntry {
    key: String,
    bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ArrayAllocatorEntry {
    signature: String,
    bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ArrayExtensionState {
    registry: ExtensibleOperatorRegistry,
    quantizer_plugins: BTreeMap<String, QuantizerPluginExtensionContract>,
}

#[derive(Debug)]
struct ArrayDebugState {
    backend_health: BackendHealthTracker,
    recent_logs: VecDeque<ArrayBackendDebugLogEvent>,
    recent_captures: VecDeque<ArrayBackendCaptureSummary>,
    next_capture_sequence: u64,
}

/// Public backend lane for the bounded array debug surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArrayBackendDebugLane {
    /// CPU or host-only reference lane.
    Cpu,
    /// Metal-labeled accelerator lane.
    Metal,
    /// CUDA-labeled accelerator lane.
    Cuda,
    /// One other backend family outside the seeded matrix.
    Other,
}

/// Public debug-capture artifact format for one backend lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArrayBackendCaptureFormat {
    /// Stable Psionic JSON bundle for the current reference lane.
    PsionicDebugJson,
    /// Stable Metal-labeled Psionic JSON bundle.
    MetalDebugJson,
    /// Stable CUDA-labeled Psionic JSON bundle.
    CudaDebugJson,
}

/// Public support matrix for the bounded backend debug surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ArrayBackendDebugSupport {
    /// Backend lane represented by the current context.
    pub lane: ArrayBackendDebugLane,
    /// Trace modes admitted by the public compiler-backed debug hook.
    pub supported_trace_modes: Vec<CompileTransformTraceMode>,
    /// Debug modes admitted by the public compiler-backed debug hook.
    pub supported_debug_modes: Vec<CompileTransformDebugMode>,
    /// Artifact formats the current backend lane can emit.
    pub capture_formats: Vec<ArrayBackendCaptureFormat>,
    /// Whether runtime log events are retained on this surface.
    pub supports_runtime_logging: bool,
    /// Whether runtime observability snapshots are retained on this surface.
    pub supports_runtime_observability: bool,
    /// Whether the current bounded surface exposes vendor-native profiler capture.
    pub supports_vendor_native_capture: bool,
}

/// One public backend debug log event retained by the array context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ArrayBackendDebugLogEvent {
    /// High-level event family.
    pub kind: ArrayBackendLogKind,
    /// Stable backend label that emitted the event.
    pub backend: String,
    /// Plain-language event detail.
    pub message: String,
    /// Stable graph digest involved in the event, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_digest: Option<String>,
    /// Stable capture identifier when the event belongs to one capture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_id: Option<String>,
    /// Timestamp when the event was observed.
    pub observed_at_millis: u64,
}

/// Stable log-event family for one public backend debug event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArrayBackendLogKind {
    /// One explicit materialization boundary completed.
    Materialize,
    /// One backend debug capture completed.
    Capture,
    /// Runtime cache policies changed.
    CachePolicyUpdated,
    /// Runtime cache families were reset explicitly.
    CachesReset,
}

/// Stable summary retained for one recent backend debug capture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ArrayBackendCaptureSummary {
    /// Stable capture identifier.
    pub capture_id: String,
    /// Stable backend label that produced the capture.
    pub backend: String,
    /// Graph digest captured by the request.
    pub graph_digest: String,
    /// Compiler trace mode used for the capture.
    pub trace_mode: CompileTransformTraceMode,
    /// Compiler debug mode used for the capture.
    pub debug_mode: CompileTransformDebugMode,
    /// Stable emitted artifact when the capture wrote one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<ArrayBackendCaptureArtifact>,
    /// Timestamp when the capture completed.
    pub observed_at_millis: u64,
}

/// Snapshot of backend debug state for the current array context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ArrayBackendDebugSnapshot {
    /// Stable backend/toolchain identity for the context.
    pub backend: BackendToolchainIdentity,
    /// Support matrix for the current backend lane.
    pub support: ArrayBackendDebugSupport,
    /// Current bounded runtime observability snapshot.
    pub runtime_observability: LocalRuntimeObservability,
    /// Current bounded runtime resource snapshot.
    pub runtime_resources: ArrayRuntimeResourceReport,
    /// Recent retained backend log events in chronological order.
    pub recent_logs: Vec<ArrayBackendDebugLogEvent>,
    /// Recent retained captures in chronological order.
    pub recent_captures: Vec<ArrayBackendCaptureSummary>,
}

/// Explicit request to write one backend debug artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrayBackendCaptureArtifactRequest {
    /// Destination path for the emitted capture bundle.
    pub path: PathBuf,
    /// Optional explicit artifact format; defaults to the backend lane default.
    pub format: Option<ArrayBackendCaptureFormat>,
}

/// Stable emitted artifact for one backend debug capture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ArrayBackendCaptureArtifact {
    /// Artifact path written by the capture.
    pub path: PathBuf,
    /// Artifact format emitted at that path.
    pub format: ArrayBackendCaptureFormat,
    /// Artifact size in bytes.
    pub bytes: u64,
}

/// Public configuration for one explicit backend debug capture.
#[derive(Clone, Debug, PartialEq)]
pub struct ArrayBackendCaptureConfig {
    /// Compiler-backed trace/debug posture for the captured graph.
    pub compile: CompileTransformConfig,
    /// Whether to snapshot runtime observability and resources before/after capture.
    pub include_runtime_snapshot: bool,
    /// Optional on-disk capture artifact request.
    pub artifact: Option<ArrayBackendCaptureArtifactRequest>,
    /// Optional plain-language label carried into the capture receipt.
    pub label: Option<String>,
}

impl Default for ArrayBackendCaptureConfig {
    fn default() -> Self {
        Self {
            compile: CompileTransformConfig::default(),
            include_runtime_snapshot: true,
            artifact: None,
            label: None,
        }
    }
}

/// Public receipt for one explicit backend debug capture.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ArrayBackendCaptureReceipt {
    /// Stable capture identifier.
    pub capture_id: String,
    /// Stable graph digest captured by the request.
    pub graph_digest: String,
    /// Stable backend/toolchain identity for the captured lane.
    pub backend: BackendToolchainIdentity,
    /// Support matrix for the captured lane.
    pub support: ArrayBackendDebugSupport,
    /// Optional caller-supplied capture label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Compiler trace/debug result captured for the graph.
    pub compile: CompileTransformResult,
    /// Runtime observability snapshot before capture, when requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_observability_before: Option<LocalRuntimeObservability>,
    /// Runtime observability snapshot after capture, when requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_observability_after: Option<LocalRuntimeObservability>,
    /// Runtime resource snapshot before capture, when requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_resources_before: Option<ArrayRuntimeResourceReport>,
    /// Runtime resource snapshot after capture, when requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_resources_after: Option<ArrayRuntimeResourceReport>,
    /// Eval receipts emitted by the capture materialization.
    pub eval_receipts: Vec<EvalReceipt>,
    /// Recent log events retained for the context after capture.
    pub recent_logs: Vec<ArrayBackendDebugLogEvent>,
    /// Emitted capture artifact when the caller requested one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<ArrayBackendCaptureArtifact>,
}

/// Snapshot of the bounded public extension-authoring surface for one array
/// context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ArrayExtensionRegistrySnapshot {
    /// Stable backend label that owns the current authoring surface.
    pub backend: String,
    /// Registered built-in plus custom schemas in stable name order.
    pub schemas: Vec<RegisteredOperatorSchema>,
    /// Registered kernel contracts in stable operator/backend order.
    pub kernel_registrations: Vec<KernelRegistration>,
    /// Registered quantizer-plugin contracts in stable plugin-id order.
    pub quantizer_plugins: Vec<QuantizerPluginExtensionContract>,
    /// Canonical bounded extension-contract semantics report for the current scope.
    pub semantics: ExtensionContractSemanticsReport,
}

/// Receipt for one successful public extension-contract registration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ArrayExtensionRegistrationReceipt {
    /// Contract family that was registered.
    pub kind: ExtensionContractKind,
    /// Stable subject identifier such as an op or plugin id.
    pub subject: String,
    /// Backend label used when resolving dispatch receipts.
    pub backend: String,
    /// Resolved dispatch contracts emitted by the registration when applicable.
    pub dispatch_contracts: Vec<OperatorDispatchContract>,
}

/// Public active, peak, and cached-memory counters for the bounded array surface.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ArrayMemoryCounters {
    /// Bytes currently owned by the graph-backed array context.
    pub active_bytes: u64,
    /// Peak active bytes observed by the context so far.
    pub peak_bytes: u64,
    /// Bytes currently retained by runtime-owned caches.
    pub cached_bytes: u64,
}

/// Public cache-limit control for the bounded array runtime surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrayCacheLimitControl {
    /// Execution-plan cache policy for graph-eval reuse.
    pub execution_plan_cache: ExecutionPlanCachePolicy,
    /// Allocator-pool policy for reusable reference buffers.
    pub allocator_pool: AllocatorPoolPolicy,
    /// Kernel-cache policy for bounded op-family reuse.
    pub kernel_cache: KernelCachePolicy,
}

impl ArrayCacheLimitControl {
    /// Returns the default bounded cache-control posture for the current array surface.
    #[must_use]
    pub const fn bounded_reference_defaults() -> Self {
        Self {
            execution_plan_cache: ExecutionPlanCachePolicy::bounded(
                DEFAULT_EXECUTION_PLAN_CACHE_ENTRIES,
                Some(DEFAULT_EXECUTION_PLAN_CACHE_BYTES),
            ),
            allocator_pool: AllocatorPoolPolicy::exact_tensor_spec(
                DEFAULT_ALLOCATOR_POOL_BUFFERS,
                DEFAULT_ALLOCATOR_POOL_BYTES,
            ),
            kernel_cache: KernelCachePolicy::bounded(
                DEFAULT_KERNEL_CACHE_ENTRIES,
                Some(DEFAULT_KERNEL_CACHE_BYTES),
            ),
        }
    }
}

impl Default for ArrayCacheLimitControl {
    fn default() -> Self {
        Self::bounded_reference_defaults()
    }
}

/// Public runtime-resource report for the bounded array surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ArrayRuntimeResourceReport {
    /// Active, peak, and cached-memory counters.
    pub memory: ArrayMemoryCounters,
    /// Explicit runtime-owned cache policy and occupancy state.
    pub backend_resources: BackendRuntimeResources,
    /// Explicit cache invalidation policy inherited from the runtime substrate.
    pub cache_invalidation_policy: CacheInvalidationPolicy,
}

/// Runtime cache family that can be reset on the bounded array surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrayCacheResetScope {
    /// Reset only the execution-plan cache.
    ExecutionPlanCache,
    /// Reset only the reusable allocator pool.
    AllocatorPool,
    /// Reset only the kernel cache.
    KernelCache,
    /// Reset all backend runtime resources together.
    BackendRuntimeResources,
}

impl ArrayCacheResetScope {
    /// Returns the matching isolation-reset scope exposed by the runtime substrate.
    #[must_use]
    pub const fn isolation_reset_scope(self) -> IsolationResetScope {
        match self {
            Self::ExecutionPlanCache
            | Self::AllocatorPool
            | Self::KernelCache
            | Self::BackendRuntimeResources => IsolationResetScope::BackendRuntimeResources,
        }
    }
}

/// Stable receipt for one explicit runtime-cache reset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrayCacheResetReceipt {
    /// Cache scopes that were explicitly reset.
    pub reset_scopes: Vec<ArrayCacheResetScope>,
    /// Cache bytes reclaimed by the reset.
    pub reclaimed_cache_bytes: u64,
    /// Resource report before the reset.
    pub before: ArrayRuntimeResourceReport,
    /// Resource report after the reset.
    pub after: ArrayRuntimeResourceReport,
}

/// Error type raised by the public lazy-array facade.
#[derive(Debug, Error, PartialEq)]
pub enum ArrayError {
    /// Two arrays came from different public graph-construction contexts.
    #[error("array operations require arrays from the same ArrayContext")]
    MixedContexts,
    /// One operation expected at least one array but received none.
    #[error("array operation requires at least one input array")]
    EmptyArrayList,
    /// One deterministic-random contract is invalid for the requested array context.
    #[error(transparent)]
    DeterminismContract(#[from] DeterminismContractError),
    /// One random-uniform request used invalid bounds.
    #[error("random uniform requires high > low; found low={low} high={high}")]
    InvalidRandomUniformBounds {
        /// Lower bound.
        low: f32,
        /// Upper bound.
        high: f32,
    },
    /// One random-normal request used an invalid standard deviation.
    #[error("random normal requires stddev > 0; found stddev={stddev}")]
    InvalidRandomNormalStddev {
        /// Requested standard deviation.
        stddev: f32,
    },
    /// One arange request used a zero or non-progressing step.
    #[error(
        "arange requires a non-zero step that progresses from start={start} toward stop={stop}; found step={step}"
    )]
    InvalidArangeStep {
        /// Start value.
        start: f32,
        /// Stop value.
        stop: f32,
        /// Step value.
        step: f32,
    },
    /// One linspace request used a zero sample count.
    #[error("linspace requires count > 0")]
    InvalidLinspaceCount,
    /// One host-interop request could not convert the bounded reference payload.
    #[error("cannot export tensor {tensor} with dtype {dtype:?} to host data: {detail}")]
    HostInteropRefusal {
        /// Tensor identifier that could not be exported.
        tensor: TensorId,
        /// Logical dtype requested by the public array surface.
        dtype: DType,
        /// Plain-language refusal detail.
        detail: String,
    },
    /// One scalar-access request targeted an array with more than one element.
    #[error("item access requires exactly one logical element; tensor {tensor} has shape {shape}")]
    NonSingletonItem {
        /// Tensor identifier that was requested as a scalar.
        tensor: TensorId,
        /// Logical shape that refused scalar extraction.
        shape: Shape,
    },
    /// One stream belongs to a different device than the owning context.
    #[error("stream {stream_id} belongs to device `{stream_device}` instead of `{context_device}`")]
    StreamDeviceMismatch {
        /// Stream identifier.
        stream_id: u32,
        /// Stream device label.
        stream_device: String,
        /// Context device label.
        context_device: String,
    },
    /// One graph input was never bound to a materialized value.
    #[error("cannot evaluate unresolved input `{name}` for tensor {tensor}")]
    UnboundInput {
        /// Input tensor identifier.
        tensor: TensorId,
        /// Input binding name.
        name: String,
    },
    /// One graph node could not be materialized on the bounded current surface.
    #[error("cannot materialize tensor {tensor} for op `{op}`: {detail}")]
    MaterializationRefusal {
        /// Output tensor identifier.
        tensor: TensorId,
        /// Stable operator label.
        op: String,
        /// Plain-language refusal detail.
        detail: String,
    },
    /// One graph node referenced a missing evaluated dependency.
    #[error("graph dependency for tensor {tensor} referenced missing input {input}")]
    MissingDependency {
        /// Current output tensor identifier.
        tensor: TensorId,
        /// Missing input tensor identifier.
        input: TensorId,
    },
    /// The lower graph builder rejected the requested operation.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// The lower extensible-operator registry rejected one authoring request.
    #[error(transparent)]
    RegistryExtension(#[from] RegistryExtensionError),
    /// The public compile-transform debug surface refused or failed.
    #[error(transparent)]
    CompileTransform(#[from] CompileTransformError),
    /// One public extension contract was refused under the bounded current scope.
    #[error("extension contract refused: {refusal:?}")]
    ExtensionContractRefusal {
        /// Canonical refusal returned by the lower extension contract.
        refusal: PsionicRefusal,
    },
    /// One quantizer-plugin id was already registered on the context.
    #[error("quantizer plugin `{plugin_id}` is already registered on this context")]
    DuplicateQuantizerPlugin {
        /// Duplicate plugin id.
        plugin_id: String,
    },
    /// One requested capture artifact format does not belong to the active backend lane.
    #[error("backend debug format {format:?} is unsupported for backend `{backend}`")]
    UnsupportedBackendDebugFormat {
        /// Active backend label.
        backend: String,
        /// Unsupported requested artifact format.
        format: ArrayBackendCaptureFormat,
    },
    /// One on-disk debug capture bundle could not be written.
    #[error("cannot write backend debug capture artifact at `{path}`: {detail}")]
    DebugCaptureArtifactWrite {
        /// Artifact path that failed.
        path: PathBuf,
        /// Plain-language write failure detail.
        detail: String,
    },
}

impl ArrayBackendDebugSupport {
    fn for_backend(backend: &str) -> Self {
        let lane = backend_debug_lane(backend);
        let capture_formats = match lane {
            ArrayBackendDebugLane::Metal => vec![
                ArrayBackendCaptureFormat::MetalDebugJson,
                ArrayBackendCaptureFormat::PsionicDebugJson,
            ],
            ArrayBackendDebugLane::Cuda => vec![
                ArrayBackendCaptureFormat::CudaDebugJson,
                ArrayBackendCaptureFormat::PsionicDebugJson,
            ],
            ArrayBackendDebugLane::Cpu | ArrayBackendDebugLane::Other => {
                vec![ArrayBackendCaptureFormat::PsionicDebugJson]
            }
        };
        Self {
            lane,
            supported_trace_modes: vec![
                CompileTransformTraceMode::Disabled,
                CompileTransformTraceMode::CacheIdentity,
                CompileTransformTraceMode::TraceFamilyIdentity,
                CompileTransformTraceMode::FullArtifacts,
            ],
            supported_debug_modes: vec![
                CompileTransformDebugMode::Disabled,
                CompileTransformDebugMode::PlanDebug,
                CompileTransformDebugMode::DisableCompile,
            ],
            capture_formats,
            supports_runtime_logging: true,
            supports_runtime_observability: true,
            supports_vendor_native_capture: false,
        }
    }

    fn default_capture_format(&self) -> ArrayBackendCaptureFormat {
        self.capture_formats
            .first()
            .copied()
            .unwrap_or(ArrayBackendCaptureFormat::PsionicDebugJson)
    }

    fn supports_format(&self, format: ArrayBackendCaptureFormat) -> bool {
        self.capture_formats.contains(&format)
    }
}

impl ArrayDebugState {
    fn new(backend: &str, health: RuntimeHealth) -> Self {
        let observed_at_millis = current_time_millis();
        let mut backend_health =
            BackendHealthTracker::with_history_limit(DEFAULT_DEBUG_LOG_HISTORY_LIMIT);
        backend_health.observe(backend.to_string(), health, observed_at_millis);
        Self {
            backend_health,
            recent_logs: VecDeque::new(),
            recent_captures: VecDeque::new(),
            next_capture_sequence: 1,
        }
    }

    fn next_capture_id(&mut self, backend: &str) -> String {
        let capture_id = format!("{backend}-capture-{:04}", self.next_capture_sequence);
        self.next_capture_sequence = self.next_capture_sequence.saturating_add(1);
        capture_id
    }

    fn record_log(&mut self, event: ArrayBackendDebugLogEvent) {
        if self.recent_logs.len() == DEFAULT_DEBUG_LOG_HISTORY_LIMIT {
            self.recent_logs.pop_front();
        }
        self.recent_logs.push_back(event);
    }

    fn record_capture(&mut self, summary: ArrayBackendCaptureSummary) {
        if self.recent_captures.len() == DEFAULT_DEBUG_CAPTURE_HISTORY_LIMIT {
            self.recent_captures.pop_front();
        }
        self.recent_captures.push_back(summary);
    }
}

impl ArrayExtensionState {
    fn new() -> Self {
        Self {
            registry: ExtensibleOperatorRegistry::with_builtin(),
            quantizer_plugins: BTreeMap::new(),
        }
    }

    fn snapshot(&self, backend: &str) -> ArrayExtensionRegistrySnapshot {
        ArrayExtensionRegistrySnapshot {
            backend: backend.to_string(),
            schemas: self.registry.schemas(),
            kernel_registrations: self.registry.kernel_registrations(),
            quantizer_plugins: self.quantizer_plugins.values().cloned().collect(),
            semantics: builtin_extension_contract_semantics_report(),
        }
    }

    fn ensure_custom_schema(
        &mut self,
        schema: RegisteredOperatorSchema,
    ) -> Result<(), RegistryExtensionError> {
        match self.registry.schema(schema.name.as_str()) {
            Some(existing) if existing == &schema => Ok(()),
            Some(_) => Err(RegistryExtensionError::DuplicateSchema { name: schema.name }),
            None => self.registry.register_custom_schema(schema),
        }
    }
}

impl ArrayRuntimeState {
    fn new(device: &ArrayDevice) -> Self {
        let limits = ArrayCacheLimitControl::default();
        let execution_plan_cache = limits.execution_plan_cache;
        let allocator_pool = limits.allocator_pool;
        let allocator_pool_budget_bytes = allocator_pool.max_cached_bytes;
        let kernel_cache = limits.kernel_cache;
        let kernel_cache_budget_bytes = kernel_cache.max_cached_bytes.unwrap_or(0);
        let runtime_resources = BackendRuntimeResources {
            execution_plan_cache: ExecutionPlanCacheReport {
                policy: execution_plan_cache,
                state: ExecutionPlanCacheState::default(),
            },
            allocator_pool: AllocatorPoolReport {
                policy: allocator_pool,
                state: AllocatorPoolState::default(),
            },
            kernel_cache: KernelCacheReport {
                policy: kernel_cache,
                state: KernelCacheState::default(),
            },
            device_memory_budget: Some(DeviceMemoryBudget::new(
                device.descriptor().memory_capacity_bytes,
                allocator_pool_budget_bytes,
                kernel_cache_budget_bytes,
            )),
        };
        Self {
            total_bytes: 0,
            peak_bytes: 0,
            tensor_bytes: BTreeMap::new(),
            runtime_resources,
            cache_invalidation_policy: default_cache_invalidation_policy(),
            execution_plan_cache: VecDeque::new(),
            allocator_pool: VecDeque::new(),
            kernel_cache: VecDeque::new(),
        }
    }

    fn report(&self) -> ArrayRuntimeResourceReport {
        ArrayRuntimeResourceReport {
            memory: ArrayMemoryCounters {
                active_bytes: self.total_bytes,
                peak_bytes: self.peak_bytes,
                cached_bytes: self.cached_bytes(),
            },
            backend_resources: self.runtime_resources.clone(),
            cache_invalidation_policy: self.cache_invalidation_policy.clone(),
        }
    }

    fn cached_bytes(&self) -> u64 {
        self.runtime_resources
            .execution_plan_cache
            .state
            .cached_bytes
            .saturating_add(self.runtime_resources.allocator_pool.state.cached_bytes)
            .saturating_add(self.runtime_resources.kernel_cache.state.cached_bytes)
    }

    fn record_tensor(&mut self, tensor: &Tensor) {
        self.tensor_bytes.entry(tensor.id()).or_insert_with(|| {
            let bytes = tensor_bytes(tensor.spec());
            self.total_bytes = self.total_bytes.saturating_add(bytes);
            self.peak_bytes = self.peak_bytes.max(self.total_bytes);
            bytes
        });
    }

    fn apply_cache_limits(
        &mut self,
        limits: ArrayCacheLimitControl,
        memory_capacity_bytes: Option<u64>,
    ) {
        let execution_plan_cache = limits.execution_plan_cache;
        let allocator_pool = limits.allocator_pool;
        let allocator_pool_budget_bytes = allocator_pool.max_cached_bytes;
        let kernel_cache = limits.kernel_cache;
        let kernel_cache_budget_bytes = kernel_cache.max_cached_bytes.unwrap_or(0);
        self.runtime_resources.execution_plan_cache.policy = execution_plan_cache;
        self.runtime_resources.allocator_pool.policy = allocator_pool;
        self.runtime_resources.kernel_cache.policy = kernel_cache;
        self.runtime_resources.device_memory_budget = Some(DeviceMemoryBudget::new(
            memory_capacity_bytes,
            allocator_pool_budget_bytes,
            kernel_cache_budget_bytes,
        ));
        self.enforce_execution_plan_cache_limits();
        self.enforce_allocator_pool_limits();
        self.enforce_kernel_cache_limits();
        self.refresh_cache_state();
    }

    fn record_eval(&mut self, graph: &Graph, outputs: &[Array]) {
        self.record_execution_plan(graph);
        self.record_kernel_families(graph);
        self.record_allocator_buffers(outputs);
        self.refresh_cache_state();
    }

    fn record_execution_plan(&mut self, graph: &Graph) {
        let policy = &self.runtime_resources.execution_plan_cache.policy;
        if !policy.enabled || policy.max_cached_entries == 0 {
            self.execution_plan_cache.clear();
            return;
        }
        let digest = graph.stable_digest();
        if let Some(index) = self
            .execution_plan_cache
            .iter()
            .position(|entry| entry.key == digest)
        {
            if let Some(entry) = self.execution_plan_cache.remove(index) {
                self.execution_plan_cache.push_back(entry);
            }
            return;
        }
        self.execution_plan_cache.push_back(ArrayCacheEntry {
            key: digest,
            bytes: estimate_execution_plan_cache_bytes(graph),
        });
        self.enforce_execution_plan_cache_limits();
    }

    fn record_kernel_families(&mut self, graph: &Graph) {
        let policy = &self.runtime_resources.kernel_cache.policy;
        if !policy.enabled || policy.max_cached_entries == 0 {
            self.kernel_cache.clear();
            return;
        }
        for label in graph
            .nodes()
            .iter()
            .filter_map(|node| kernel_cache_label(node.op()))
        {
            if let Some(index) = self
                .kernel_cache
                .iter()
                .position(|entry| entry.key == label)
            {
                if let Some(entry) = self.kernel_cache.remove(index) {
                    self.kernel_cache.push_back(entry);
                }
                continue;
            }
            self.kernel_cache.push_back(ArrayCacheEntry {
                bytes: estimate_kernel_cache_bytes(label.as_str()),
                key: label,
            });
        }
        self.enforce_kernel_cache_limits();
    }

    fn record_allocator_buffers(&mut self, outputs: &[Array]) {
        let policy = &self.runtime_resources.allocator_pool.policy;
        if policy.mode == psionic_runtime::AllocatorPoolMode::Disabled
            || policy.max_cached_buffers == 0
        {
            self.allocator_pool.clear();
            return;
        }
        for output in outputs {
            let signature = tensor_spec_signature(output.spec());
            if let Some(index) = self
                .allocator_pool
                .iter()
                .position(|entry| entry.signature == signature)
            {
                if let Some(entry) = self.allocator_pool.remove(index) {
                    self.allocator_pool.push_back(entry);
                }
                continue;
            }
            self.allocator_pool.push_back(ArrayAllocatorEntry {
                bytes: tensor_bytes(output.spec()),
                signature,
            });
        }
        self.enforce_allocator_pool_limits();
    }

    fn reset_caches(&mut self, scopes: &[ArrayCacheResetScope]) -> ArrayCacheResetReceipt {
        let before = self.report();
        for scope in scopes {
            match scope {
                ArrayCacheResetScope::ExecutionPlanCache => self.execution_plan_cache.clear(),
                ArrayCacheResetScope::AllocatorPool => self.allocator_pool.clear(),
                ArrayCacheResetScope::KernelCache => self.kernel_cache.clear(),
                ArrayCacheResetScope::BackendRuntimeResources => {
                    self.execution_plan_cache.clear();
                    self.allocator_pool.clear();
                    self.kernel_cache.clear();
                }
            }
        }
        self.refresh_cache_state();
        let after = self.report();
        ArrayCacheResetReceipt {
            reset_scopes: scopes.to_vec(),
            reclaimed_cache_bytes: before
                .memory
                .cached_bytes
                .saturating_sub(after.memory.cached_bytes),
            before,
            after,
        }
    }

    fn refresh_cache_state(&mut self) {
        self.runtime_resources.execution_plan_cache.state = ExecutionPlanCacheState {
            cached_entries: self.execution_plan_cache.len(),
            cached_bytes: self
                .execution_plan_cache
                .iter()
                .map(|entry| entry.bytes)
                .sum(),
        };
        self.runtime_resources.allocator_pool.state = AllocatorPoolState {
            cached_buffers: self.allocator_pool.len(),
            cached_bytes: self.allocator_pool.iter().map(|entry| entry.bytes).sum(),
        };
        self.runtime_resources.kernel_cache.state = KernelCacheState {
            cached_entries: self.kernel_cache.len(),
            cached_bytes: self.kernel_cache.iter().map(|entry| entry.bytes).sum(),
        };
    }

    fn enforce_execution_plan_cache_limits(&mut self) {
        let policy = &self.runtime_resources.execution_plan_cache.policy;
        if !policy.enabled || policy.max_cached_entries == 0 {
            self.execution_plan_cache.clear();
            return;
        }
        while self.execution_plan_cache.len() > policy.max_cached_entries {
            self.execution_plan_cache.pop_front();
        }
        if let Some(max_cached_bytes) = policy.max_cached_bytes {
            while cache_bytes(self.execution_plan_cache.iter().map(|entry| entry.bytes))
                > max_cached_bytes
            {
                if self.execution_plan_cache.pop_front().is_none() {
                    break;
                }
            }
        }
    }

    fn enforce_allocator_pool_limits(&mut self) {
        let policy = &self.runtime_resources.allocator_pool.policy;
        if policy.mode == psionic_runtime::AllocatorPoolMode::Disabled
            || policy.max_cached_buffers == 0
        {
            self.allocator_pool.clear();
            return;
        }
        while self.allocator_pool.len() > policy.max_cached_buffers {
            self.allocator_pool.pop_front();
        }
        while cache_bytes(self.allocator_pool.iter().map(|entry| entry.bytes))
            > policy.max_cached_bytes
        {
            if self.allocator_pool.pop_front().is_none() {
                break;
            }
        }
    }

    fn enforce_kernel_cache_limits(&mut self) {
        let policy = &self.runtime_resources.kernel_cache.policy;
        if !policy.enabled || policy.max_cached_entries == 0 {
            self.kernel_cache.clear();
            return;
        }
        while self.kernel_cache.len() > policy.max_cached_entries {
            self.kernel_cache.pop_front();
        }
        if let Some(max_cached_bytes) = policy.max_cached_bytes {
            while cache_bytes(self.kernel_cache.iter().map(|entry| entry.bytes)) > max_cached_bytes
            {
                if self.kernel_cache.pop_front().is_none() {
                    break;
                }
            }
        }
    }
}

impl ArrayContextInner {
    fn backend_debug_support(&self) -> ArrayBackendDebugSupport {
        ArrayBackendDebugSupport::for_backend(self.device.backend())
    }

    fn runtime_observability_snapshot(&self) -> LocalRuntimeObservability {
        let runtime_state = self.runtime_state.borrow();
        let resident_host_bytes = runtime_state
            .total_bytes
            .saturating_add(runtime_state.cached_bytes());
        let debug_state = self.debug_state.borrow();
        LocalRuntimeObservability {
            isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
            cache_invalidation_policy: runtime_state.cache_invalidation_policy.clone(),
            execution_profile: ExecutionCapabilityProfile::single_request_latency_optimized(),
            queue_depth: 0,
            queue_capacity: Some(1),
            active_sessions: 0,
            active_requests: 0,
            memory_footprint: MemoryResidencySnapshot {
                loaded_models: 0,
                resident_host_bytes,
                resident_device_bytes: 0,
            },
            backend_health: debug_state.backend_health.snapshot(),
            recent_transitions: debug_state.backend_health.recent_changes(),
        }
    }

    fn debug_snapshot(&self) -> ArrayBackendDebugSnapshot {
        let debug_state = self.debug_state.borrow();
        ArrayBackendDebugSnapshot {
            backend: self.backend_identity.clone(),
            support: self.backend_debug_support(),
            runtime_observability: self.runtime_observability_snapshot(),
            runtime_resources: self.runtime_state.borrow().report(),
            recent_logs: debug_state.recent_logs.iter().cloned().collect(),
            recent_captures: debug_state.recent_captures.iter().cloned().collect(),
        }
    }

    fn record_backend_log(
        &self,
        kind: ArrayBackendLogKind,
        message: impl Into<String>,
        graph_digest: Option<String>,
        capture_id: Option<String>,
    ) {
        self.debug_state
            .borrow_mut()
            .record_log(ArrayBackendDebugLogEvent {
                kind,
                backend: self.device.backend().to_string(),
                message: message.into(),
                graph_digest,
                capture_id,
                observed_at_millis: current_time_millis(),
            });
    }
}

fn backend_debug_lane(backend: &str) -> ArrayBackendDebugLane {
    match backend {
        "cpu" => ArrayBackendDebugLane::Cpu,
        "metal" => ArrayBackendDebugLane::Metal,
        "cuda" => ArrayBackendDebugLane::Cuda,
        _ => ArrayBackendDebugLane::Other,
    }
}

fn backend_identity_for_device(device: &ArrayDevice) -> BackendToolchainIdentity {
    let features = device.descriptor().feature_flags.clone();
    let identity = BackendToolchainIdentity::new(
        device.backend(),
        format!("psionic-array-reference@{}", env!("CARGO_PKG_VERSION")),
        features.clone(),
    );
    if device.descriptor().device_name.is_some()
        || device.descriptor().memory_capacity_bytes.is_some()
        || !features.is_empty()
    {
        identity.with_probe(BackendProbeState::CompiledAndProbed, features)
    } else {
        identity
    }
}

fn initial_backend_health(device: &ArrayDevice) -> RuntimeHealth {
    match backend_debug_lane(device.backend()) {
        ArrayBackendDebugLane::Cpu => RuntimeHealth {
            status: HealthStatus::Ready,
            message: String::from("cpu reference runtime and debug hooks ready"),
        },
        ArrayBackendDebugLane::Metal => RuntimeHealth {
            status: HealthStatus::Degraded,
            message: String::from(
                "metal debug hooks route through the reference runtime; vendor-native capture remains unavailable",
            ),
        },
        ArrayBackendDebugLane::Cuda => RuntimeHealth {
            status: HealthStatus::Degraded,
            message: String::from(
                "cuda debug hooks route through the reference runtime; vendor-native capture remains unavailable",
            ),
        },
        ArrayBackendDebugLane::Other => RuntimeHealth {
            status: HealthStatus::Degraded,
            message: format!(
                "backend `{}` debug hooks route through the reference runtime only",
                device.backend()
            ),
        },
    }
}

fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn write_backend_capture_artifact(
    request: &ArrayBackendCaptureArtifactRequest,
    format: ArrayBackendCaptureFormat,
    receipt: &ArrayBackendCaptureReceipt,
) -> Result<ArrayBackendCaptureArtifact, ArrayError> {
    let payload = serde_json::to_vec_pretty(receipt).map_err(|error| {
        ArrayError::DebugCaptureArtifactWrite {
            path: request.path.clone(),
            detail: format!("serialize capture receipt: {error}"),
        }
    })?;
    fs::write(&request.path, &payload).map_err(|error| ArrayError::DebugCaptureArtifactWrite {
        path: request.path.clone(),
        detail: error.to_string(),
    })?;
    Ok(ArrayBackendCaptureArtifact {
        path: request.path.clone(),
        format,
        bytes: payload.len() as u64,
    })
}

fn tensor_bytes(spec: &TensorSpec) -> u64 {
    (spec.shape().element_count() as u64).saturating_mul(spec.dtype().element_size_bytes() as u64)
}

fn tensor_spec_signature(spec: &TensorSpec) -> String {
    format!(
        "{}:{:?}:{:?}",
        spec.device(),
        spec.dtype(),
        spec.shape().dims()
    )
}

fn cache_bytes<I>(bytes: I) -> u64
where
    I: IntoIterator<Item = u64>,
{
    bytes.into_iter().sum()
}

fn estimate_execution_plan_cache_bytes(graph: &Graph) -> u64 {
    (graph.nodes().len() as u64)
        .saturating_mul(64)
        .saturating_add((graph.outputs().len() as u64).saturating_mul(16))
}

fn kernel_cache_label(op: &OpKind) -> Option<String> {
    match op {
        OpKind::Input { .. } | OpKind::Constant { .. } => None,
        other => Some(other.label().to_string()),
    }
}

fn estimate_kernel_cache_bytes(label: &str) -> u64 {
    32_u64.saturating_add(label.len() as u64)
}

/// Honest current unified-memory posture for one public array device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnifiedMemoryCapability {
    /// Pure host execution with no separate accelerator memory.
    HostOnly,
    /// Shared host/device memory is explicitly supported.
    SharedHostDevice,
    /// Dedicated accelerator memory is explicitly advertised.
    DedicatedDevice,
    /// The runtime has not yet reported unified-memory posture.
    Unknown,
}

/// Public device handle for the MLX-class array layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrayDevice {
    descriptor: DeviceDescriptor,
    inventory: DeviceInventoryQualifiers,
    unified_memory: UnifiedMemoryCapability,
}

impl ArrayDevice {
    /// Builds a public array-device handle from runtime-owned device truth.
    #[must_use]
    pub fn from_descriptor(descriptor: DeviceDescriptor) -> Self {
        let inventory = descriptor.inventory_qualifiers();
        let unified_memory = match inventory.memory_class {
            DeviceMemoryClass::HostOnly => UnifiedMemoryCapability::HostOnly,
            DeviceMemoryClass::SharedHostDevice => UnifiedMemoryCapability::SharedHostDevice,
            DeviceMemoryClass::DedicatedDevice => UnifiedMemoryCapability::DedicatedDevice,
        };
        Self {
            descriptor,
            inventory,
            unified_memory,
        }
    }

    /// Builds a logical-only handle when runtime discovery metadata is absent.
    #[must_use]
    pub fn logical(device: Device) -> Self {
        let backend = device.kind().to_string();
        let unified_memory = match device.kind() {
            DeviceKind::Cpu => Some(true),
            _ => None,
        };
        Self::from_descriptor(DeviceDescriptor {
            backend,
            device,
            device_name: None,
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: None,
            unified_memory,
            feature_flags: Vec::new(),
            amd_metadata: None,
            nvidia_metadata: None,
        })
    }

    /// Returns the lower runtime device descriptor.
    #[must_use]
    pub fn descriptor(&self) -> &DeviceDescriptor {
        &self.descriptor
    }

    /// Returns reusable inventory qualifiers derived from runtime truth.
    #[must_use]
    pub fn inventory(&self) -> &DeviceInventoryQualifiers {
        &self.inventory
    }

    /// Returns the logical device identifier.
    #[must_use]
    pub fn device(&self) -> &Device {
        &self.descriptor.device
    }

    /// Returns the backend family name.
    #[must_use]
    pub fn backend(&self) -> &str {
        &self.descriptor.backend
    }

    /// Returns the friendly device name when known.
    #[must_use]
    pub fn device_name(&self) -> Option<&str> {
        self.descriptor.device_name.as_deref()
    }

    /// Returns the current unified-memory posture.
    #[must_use]
    pub const fn unified_memory_capability(&self) -> UnifiedMemoryCapability {
        self.unified_memory
    }

    /// Returns whether the runtime explicitly reports shared host/device memory.
    #[must_use]
    pub const fn supports_unified_memory(&self) -> bool {
        matches!(
            self.unified_memory,
            UnifiedMemoryCapability::HostOnly | UnifiedMemoryCapability::SharedHostDevice
        )
    }

    /// Returns the stable inventory device identifier.
    #[must_use]
    pub fn stable_id(&self) -> &str {
        &self.inventory.stable_device_id
    }
}

/// Public stream kind for the bounded array-runtime layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamKind {
    /// The stream is the default stream for the device.
    Default,
    /// The stream was created explicitly by the caller.
    Explicit,
}

/// Honest current dependency posture between public streams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamDependencyPolicy {
    /// Work on the same stream is already ordered.
    InOrderSameStream,
    /// Different streams on the same device need an explicit fence or sync edge.
    ExplicitFenceRequired,
    /// Different devices require an explicit transfer or broader runtime coordination.
    CrossDeviceTransferRequired,
}

/// Public stream handle for the bounded array-runtime layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrayStream {
    stream_id: u32,
    device: ArrayDevice,
    kind: StreamKind,
}

impl ArrayStream {
    fn default_for(device: ArrayDevice) -> Self {
        Self {
            stream_id: 0,
            device,
            kind: StreamKind::Default,
        }
    }

    fn explicit(stream_id: u32, device: ArrayDevice) -> Self {
        Self {
            stream_id,
            device,
            kind: StreamKind::Explicit,
        }
    }

    /// Returns the stream identifier.
    #[must_use]
    pub const fn stream_id(&self) -> u32 {
        self.stream_id
    }

    /// Returns the stream kind.
    #[must_use]
    pub const fn kind(&self) -> StreamKind {
        self.kind
    }

    /// Returns the owning device for the stream.
    #[must_use]
    pub fn device(&self) -> &ArrayDevice {
        &self.device
    }

    /// Returns the dependency policy for scheduling work after `upstream`.
    #[must_use]
    pub fn dependency_policy_after(&self, upstream: &Self) -> StreamDependencyPolicy {
        if self.device.device() != upstream.device.device() {
            StreamDependencyPolicy::CrossDeviceTransferRequired
        } else if self.stream_id == upstream.stream_id {
            StreamDependencyPolicy::InOrderSameStream
        } else {
            StreamDependencyPolicy::ExplicitFenceRequired
        }
    }
}

/// Explicit materialization trigger for the current lazy-array surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum MaterializationTrigger {
    /// Synchronous explicit materialization through `eval`.
    Eval,
    /// Deferred explicit materialization through `async_eval(...).wait()`.
    AsyncEvalWait,
    /// Explicit backend debug capture through `capture_backend_debug`.
    DebugCapture,
    /// Explicit host-data export through `to_host_data`.
    ToHostData,
    /// Explicit singleton scalar extraction through `item`.
    Item,
}

impl MaterializationTrigger {
    fn label(self) -> &'static str {
        match self {
            Self::Eval => "eval",
            Self::AsyncEvalWait => "async_eval_wait",
            Self::DebugCapture => "debug_capture",
            Self::ToHostData => "to_host_data",
            Self::Item => "item",
        }
    }
}

/// Current policy for implicit host or display materialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ImplicitMaterializationPolicy {
    /// Only explicit eval entrypoints may materialize values today.
    ExplicitOnly,
}

/// Replay boundary for one public materialization contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ReplayBoundary {
    /// Evaluation replays against the captured graph snapshot digest.
    GraphSnapshot,
}

/// Public materialization boundary report for one lazy array.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MaterializationBoundary {
    /// Explicit materialization triggers available on the current surface.
    pub explicit_triggers: Vec<MaterializationTrigger>,
    /// Current implicit-materialization posture.
    pub implicit_policy: ImplicitMaterializationPolicy,
    /// Replay boundary for current evaluation receipts.
    pub replay_boundary: ReplayBoundary,
}

impl MaterializationBoundary {
    fn explicit_only() -> Self {
        Self {
            explicit_triggers: vec![
                MaterializationTrigger::Eval,
                MaterializationTrigger::AsyncEvalWait,
                MaterializationTrigger::DebugCapture,
                MaterializationTrigger::ToHostData,
                MaterializationTrigger::Item,
            ],
            implicit_policy: ImplicitMaterializationPolicy::ExplicitOnly,
            replay_boundary: ReplayBoundary::GraphSnapshot,
        }
    }
}

/// Receipt emitted for one explicit lazy-array evaluation boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EvalReceipt {
    /// Stable digest of the graph snapshot that was evaluated.
    pub graph_digest: String,
    /// Ordered output tensor IDs requested by the caller.
    pub outputs: Vec<TensorId>,
    /// Explicit trigger that caused materialization.
    pub trigger: MaterializationTrigger,
    /// Replay boundary for the evaluated snapshot.
    pub replay_boundary: ReplayBoundary,
    /// Stable device identifier that owned the evaluation boundary.
    pub device_id: String,
    /// Stream identifier used for the current bounded evaluation path.
    pub stream_id: u32,
}

/// Materialized output from one explicit lazy-array evaluation call.
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluatedArray {
    tensor: Tensor,
    data: TensorData,
    receipt: EvalReceipt,
    boundary: MaterializationBoundary,
}

impl EvaluatedArray {
    /// Returns the materialized tensor identifier.
    #[must_use]
    pub const fn tensor_id(&self) -> TensorId {
        self.tensor.id()
    }

    /// Returns the materialized tensor specification.
    #[must_use]
    pub fn spec(&self) -> &TensorSpec {
        self.tensor.spec()
    }

    /// Returns the logical materialized shape.
    #[must_use]
    pub fn shape(&self) -> &Shape {
        self.tensor.spec().shape()
    }

    /// Returns the materialized dtype.
    #[must_use]
    pub fn dtype(&self) -> DType {
        self.tensor.spec().dtype()
    }

    /// Returns the pinned device for the materialized result.
    #[must_use]
    pub fn device(&self) -> &Device {
        self.tensor.spec().device()
    }

    /// Returns the evaluation receipt.
    #[must_use]
    pub fn receipt(&self) -> &EvalReceipt {
        &self.receipt
    }

    /// Returns the explicit materialization boundary used for this result.
    #[must_use]
    pub fn boundary(&self) -> &MaterializationBoundary {
        &self.boundary
    }

    /// Exports the evaluated payload into an explicit host-owned typed buffer.
    pub fn to_host_data(&self) -> Result<HostArrayData, ArrayError> {
        host_array_data_from_evaluated(self.tensor_id(), self.dtype(), &self.data)
    }

    /// Extracts one explicit singleton scalar from the evaluated payload.
    pub fn item(&self) -> Result<ArrayScalar, ArrayError> {
        scalar_from_evaluated(self.tensor_id(), self.shape(), self.dtype(), &self.data)
    }
}

/// Bounded host-visible element storage family for one evaluated array.
#[derive(Clone, Debug, PartialEq)]
pub enum HostArrayStorage {
    /// Host-visible `f32` values used for logical `f32`, `f16`, and `bf16`.
    F32(Vec<f32>),
    /// Host-visible `i8` values for logical `i8`.
    I8(Vec<i8>),
}

/// Explicit host-owned data export for one evaluated array.
#[derive(Clone, Debug, PartialEq)]
pub struct HostArrayData {
    dtype: DType,
    values: HostArrayStorage,
}

impl HostArrayData {
    fn new(dtype: DType, values: HostArrayStorage) -> Self {
        Self { dtype, values }
    }

    /// Returns the logical dtype preserved by the host export.
    #[must_use]
    pub const fn dtype(&self) -> DType {
        self.dtype
    }

    /// Returns the number of exported elements.
    #[must_use]
    pub fn len(&self) -> usize {
        match &self.values {
            HostArrayStorage::F32(values) => values.len(),
            HostArrayStorage::I8(values) => values.len(),
        }
    }

    /// Returns whether the exported payload is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the host payload as `f32` values when that family applies.
    #[must_use]
    pub fn as_f32_slice(&self) -> Option<&[f32]> {
        match &self.values {
            HostArrayStorage::F32(values) => Some(values.as_slice()),
            HostArrayStorage::I8(_) => None,
        }
    }

    /// Returns the host payload as `i8` values when that family applies.
    #[must_use]
    pub fn as_i8_slice(&self) -> Option<&[i8]> {
        match &self.values {
            HostArrayStorage::F32(_) => None,
            HostArrayStorage::I8(values) => Some(values.as_slice()),
        }
    }
}

/// Host-visible scalar value exported from a singleton array.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HostScalarValue {
    /// Floating-point scalar used for logical `f32`, `f16`, and `bf16`.
    F32(f32),
    /// Integer scalar used for logical `i8`.
    I8(i8),
}

/// Explicit singleton scalar exported from a public array.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArrayScalar {
    dtype: DType,
    value: HostScalarValue,
}

impl ArrayScalar {
    fn new(dtype: DType, value: HostScalarValue) -> Self {
        Self { dtype, value }
    }

    /// Returns the logical dtype preserved by the scalar export.
    #[must_use]
    pub const fn dtype(&self) -> DType {
        self.dtype
    }

    /// Returns the scalar as `f32` when the logical dtype is floating-point.
    #[must_use]
    pub fn as_f32(&self) -> Option<f32> {
        match self.value {
            HostScalarValue::F32(value) => Some(value),
            HostScalarValue::I8(_) => None,
        }
    }

    /// Returns the scalar as `i8` when the logical dtype is integer.
    #[must_use]
    pub fn as_i8(&self) -> Option<i8> {
        match self.value {
            HostScalarValue::F32(_) => None,
            HostScalarValue::I8(value) => Some(value),
        }
    }
}

/// Generic tree container used to preserve array structure explicitly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tree<T> {
    /// One leaf value.
    Leaf(T),
    /// One list-like sequence with stable positional order.
    List(Vec<Tree<T>>),
    /// One tuple-like sequence with stable positional order.
    Tuple(Vec<Tree<T>>),
    /// One mapping with stable key order.
    Dict(BTreeMap<String, Tree<T>>),
}

/// Shape-only tree description used for deterministic flatten or unflatten.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeSpec {
    /// One leaf slot.
    Leaf,
    /// One list-like sequence.
    List(Vec<TreeSpec>),
    /// One tuple-like sequence.
    Tuple(Vec<TreeSpec>),
    /// One mapping with stable key order.
    Dict(BTreeMap<String, TreeSpec>),
}

/// Flattened leaves plus deterministic structure for one tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlattenedTree<T> {
    /// Flattened leaves in deterministic traversal order.
    pub leaves: Vec<T>,
    /// Structure needed to rebuild the original tree.
    pub spec: TreeSpec,
}

/// Error raised while rebuilding a tree from flattened leaves.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TreeError {
    /// The provided leaves did not match the requested tree structure.
    #[error("tree spec expected {expected} leaves but received {actual}")]
    LeafCountMismatch {
        /// Number of leaves required by the structure.
        expected: usize,
        /// Number of leaves actually provided.
        actual: usize,
    },
}

impl<T> Tree<T> {
    /// Returns the number of leaves in the tree.
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        match self {
            Self::Leaf(_) => 1,
            Self::List(values) | Self::Tuple(values) => values.iter().map(Self::leaf_count).sum(),
            Self::Dict(values) => values.values().map(Self::leaf_count).sum(),
        }
    }

    /// Returns the shape-only tree specification.
    #[must_use]
    pub fn spec(&self) -> TreeSpec {
        match self {
            Self::Leaf(_) => TreeSpec::Leaf,
            Self::List(values) => TreeSpec::List(values.iter().map(Self::spec).collect()),
            Self::Tuple(values) => TreeSpec::Tuple(values.iter().map(Self::spec).collect()),
            Self::Dict(values) => TreeSpec::Dict(
                values
                    .iter()
                    .map(|(key, value)| (key.clone(), value.spec()))
                    .collect(),
            ),
        }
    }

    /// Applies one fallible closure to every leaf while preserving structure.
    pub fn map_leaves<U, E, F>(&self, f: &mut F) -> Result<Tree<U>, E>
    where
        F: FnMut(&T) -> Result<U, E>,
    {
        match self {
            Self::Leaf(value) => f(value).map(Tree::Leaf),
            Self::List(values) => values
                .iter()
                .map(|value| value.map_leaves(f))
                .collect::<Result<Vec<_>, _>>()
                .map(Tree::List),
            Self::Tuple(values) => values
                .iter()
                .map(|value| value.map_leaves(f))
                .collect::<Result<Vec<_>, _>>()
                .map(Tree::Tuple),
            Self::Dict(values) => values
                .iter()
                .map(|(key, value)| value.map_leaves(f).map(|mapped| (key.clone(), mapped)))
                .collect::<Result<BTreeMap<_, _>, _>>()
                .map(Tree::Dict),
        }
    }

    /// Consumes the tree into deterministic flattened leaves plus structure.
    #[must_use]
    pub fn flatten(self) -> FlattenedTree<T> {
        let spec = self.spec();
        let mut leaves = Vec::with_capacity(spec.leaf_count());
        self.into_flattened_leaves(&mut leaves);
        FlattenedTree { leaves, spec }
    }

    fn into_flattened_leaves(self, leaves: &mut Vec<T>) {
        match self {
            Self::Leaf(value) => leaves.push(value),
            Self::List(values) | Self::Tuple(values) => {
                for value in values {
                    value.into_flattened_leaves(leaves);
                }
            }
            Self::Dict(values) => {
                for (_, value) in values {
                    value.into_flattened_leaves(leaves);
                }
            }
        }
    }
}

impl<T: Clone> Tree<T> {
    /// Returns cloned leaves in deterministic traversal order.
    #[must_use]
    pub fn leaves(&self) -> Vec<T> {
        let mut leaves = Vec::with_capacity(self.leaf_count());
        self.collect_leaves(&mut leaves);
        leaves
    }

    fn collect_leaves(&self, leaves: &mut Vec<T>) {
        match self {
            Self::Leaf(value) => leaves.push(value.clone()),
            Self::List(values) | Self::Tuple(values) => {
                for value in values {
                    value.collect_leaves(leaves);
                }
            }
            Self::Dict(values) => {
                for value in values.values() {
                    value.collect_leaves(leaves);
                }
            }
        }
    }
}

impl<T> FlattenedTree<T> {
    /// Rebuilds one tree from the stored leaves and structure.
    pub fn unflatten(self) -> Result<Tree<T>, TreeError> {
        self.spec.unflatten(self.leaves)
    }
}

impl TreeSpec {
    /// Returns the number of leaves required by the structure.
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        match self {
            Self::Leaf => 1,
            Self::List(values) | Self::Tuple(values) => values.iter().map(Self::leaf_count).sum(),
            Self::Dict(values) => values.values().map(Self::leaf_count).sum(),
        }
    }

    /// Rebuilds one tree from deterministic flattened leaves.
    pub fn unflatten<T>(&self, leaves: Vec<T>) -> Result<Tree<T>, TreeError> {
        let expected = self.leaf_count();
        let actual = leaves.len();
        if actual != expected {
            return Err(TreeError::LeafCountMismatch { expected, actual });
        }
        let mut leaves = leaves.into_iter();
        Ok(self.unflatten_from_iter(&mut leaves))
    }

    fn unflatten_from_iter<T, I>(&self, leaves: &mut I) -> Tree<T>
    where
        I: Iterator<Item = T>,
    {
        match self {
            Self::Leaf => {
                let Some(leaf) = leaves.next() else {
                    unreachable!("leaf count should have been validated before unflatten")
                };
                Tree::Leaf(leaf)
            }
            Self::List(values) => Tree::List(
                values
                    .iter()
                    .map(|value| value.unflatten_from_iter(leaves))
                    .collect(),
            ),
            Self::Tuple(values) => Tree::Tuple(
                values
                    .iter()
                    .map(|value| value.unflatten_from_iter(leaves))
                    .collect(),
            ),
            Self::Dict(values) => Tree::Dict(
                values
                    .iter()
                    .map(|(key, value)| (key.clone(), value.unflatten_from_iter(leaves)))
                    .collect(),
            ),
        }
    }
}

/// Current asynchronous evaluation status for the bounded reference surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsyncEvalStatus {
    /// The ticket has been created but not yet synchronized.
    Pending,
}

/// Deferred public evaluation ticket returned by `async_eval`.
#[derive(Clone, Debug)]
pub struct PendingAsyncEval {
    context: Rc<ArrayContextInner>,
    graph: Graph,
    outputs: Vec<Array>,
    stream: ArrayStream,
}

impl PendingAsyncEval {
    /// Returns the current ticket status.
    #[must_use]
    pub const fn status(&self) -> AsyncEvalStatus {
        AsyncEvalStatus::Pending
    }

    /// Synchronizes the deferred ticket and materializes the requested outputs.
    pub fn wait(self) -> Result<Vec<EvaluatedArray>, ArrayError> {
        let outputs = evaluate_graph_snapshot(
            &self.graph,
            self.outputs.as_slice(),
            MaterializationTrigger::AsyncEvalWait,
            &self.stream,
        )?;
        self.context
            .runtime_state
            .borrow_mut()
            .record_eval(&self.graph, self.outputs.as_slice());
        self.context.record_backend_log(
            ArrayBackendLogKind::Materialize,
            format!(
                "materialized graph {} via {} on stream {}",
                self.graph.stable_digest(),
                MaterializationTrigger::AsyncEvalWait.label(),
                self.stream.stream_id()
            ),
            Some(self.graph.stable_digest()),
            None,
        );
        Ok(outputs)
    }
}

#[derive(Clone, Debug)]
struct DenseValue {
    tensor: Tensor,
    values: Vec<f32>,
}

/// Public graph-construction context for lazy arrays.
///
/// The context owns one deterministic `GraphBuilder` so arrays can share graph
/// identity without bloating lower substrate crates. The current explicit eval
/// and async-eval entrypoints use replay-safe graph snapshots and a bounded
/// CPU-reference materializer instead of a backend scheduler.
#[derive(Clone, Debug)]
pub struct ArrayContext {
    inner: Rc<ArrayContextInner>,
    stream: ArrayStream,
}

impl ArrayContext {
    /// Creates a context pinned to one device.
    #[must_use]
    pub fn new(device: Device) -> Self {
        let device = ArrayDevice::logical(device);
        Self::with_device(device)
    }

    /// Creates a context pinned to one runtime-described device.
    #[must_use]
    pub fn from_device_descriptor(descriptor: DeviceDescriptor) -> Self {
        Self::with_device(ArrayDevice::from_descriptor(descriptor))
    }

    /// Creates a context pinned to one public array-device handle.
    #[must_use]
    pub fn with_device(device: ArrayDevice) -> Self {
        match Self::with_device_and_determinism(device, RuntimeDeterminismContract::best_effort()) {
            Ok(context) => context,
            Err(error) => {
                unreachable!("best-effort array determinism contract should validate: {error}")
            }
        }
    }

    /// Creates a context pinned to one device plus one explicit runtime determinism contract.
    pub fn with_runtime_determinism(
        device: Device,
        determinism: RuntimeDeterminismContract,
    ) -> Result<Self, ArrayError> {
        Self::with_device_and_determinism(ArrayDevice::logical(device), determinism)
    }

    /// Creates a context pinned to one runtime-described device plus one explicit determinism contract.
    pub fn from_device_descriptor_with_determinism(
        descriptor: DeviceDescriptor,
        determinism: RuntimeDeterminismContract,
    ) -> Result<Self, ArrayError> {
        Self::with_device_and_determinism(ArrayDevice::from_descriptor(descriptor), determinism)
    }

    /// Creates a CPU-backed context with one seeded replay contract.
    pub fn cpu_seeded(seed: u64) -> Result<Self, ArrayError> {
        Self::with_runtime_determinism(Device::cpu(), RuntimeDeterminismContract::seeded(seed))
    }

    fn with_device_and_determinism(
        device: ArrayDevice,
        determinism: RuntimeDeterminismContract,
    ) -> Result<Self, ArrayError> {
        determinism.validate()?;
        let determinism = if determinism.generator.is_some() {
            RuntimeDeterminismContract {
                generator: Some(
                    determinism.derive_local_device_generator(device.stable_id().to_string())?,
                ),
                ..determinism
            }
        } else {
            determinism
        };
        let stream = ArrayStream::default_for(device.clone());
        let runtime_state = ArrayRuntimeState::new(&device);
        let backend_identity = backend_identity_for_device(&device);
        let debug_state = ArrayDebugState::new(device.backend(), initial_backend_health(&device));
        Ok(Self {
            inner: Rc::new(ArrayContextInner {
                builder: RefCell::new(GraphBuilder::new(device.device().clone())),
                backend_identity,
                determinism: RefCell::new(determinism),
                debug_state: RefCell::new(debug_state),
                device,
                extension_state: RefCell::new(ArrayExtensionState::new()),
                next_stream_id: RefCell::new(1),
                runtime_state: RefCell::new(runtime_state),
            }),
            stream,
        })
    }

    /// Creates a CPU-backed context for tests and reference graph building.
    #[must_use]
    pub fn cpu() -> Self {
        Self::new(Device::cpu())
    }

    /// Returns the current runtime determinism contract for the context.
    #[must_use]
    pub fn determinism(&self) -> RuntimeDeterminismContract {
        self.inner.determinism.borrow().clone()
    }

    /// Returns the current replayable generator state when the context is seeded.
    #[must_use]
    pub fn random_generator_state(&self) -> Option<GeneratorState> {
        self.inner.determinism.borrow().generator.clone()
    }

    /// Returns the logical device assigned to the context.
    #[must_use]
    pub fn device(&self) -> &Device {
        self.inner.device.device()
    }

    /// Returns the public device handle assigned to the context.
    #[must_use]
    pub fn device_handle(&self) -> &ArrayDevice {
        &self.inner.device
    }

    /// Returns the currently selected stream for the context.
    #[must_use]
    pub fn stream(&self) -> &ArrayStream {
        &self.stream
    }

    /// Returns the current bounded runtime-memory and cache report for the context.
    #[must_use]
    pub fn runtime_resource_report(&self) -> ArrayRuntimeResourceReport {
        self.inner.runtime_state.borrow().report()
    }

    /// Returns the stable support matrix for backend debug hooks on this context.
    #[must_use]
    pub fn backend_debug_support(&self) -> ArrayBackendDebugSupport {
        self.inner.backend_debug_support()
    }

    /// Returns the current backend debug snapshot for this context.
    #[must_use]
    pub fn backend_debug_snapshot(&self) -> ArrayBackendDebugSnapshot {
        self.inner.debug_snapshot()
    }

    /// Returns the current public extension-authoring snapshot for this context.
    #[must_use]
    pub fn extension_registry_snapshot(&self) -> ArrayExtensionRegistrySnapshot {
        self.inner
            .extension_state
            .borrow()
            .snapshot(self.inner.device.backend())
    }

    /// Registers one custom-op extension contract on the current context.
    pub fn register_custom_op_extension(
        &self,
        contract: CustomOpExtensionContract,
    ) -> Result<ArrayExtensionRegistrationReceipt, ArrayError> {
        contract
            .validate()
            .map_err(|refusal| ArrayError::ExtensionContractRefusal { refusal })?;
        let subject = contract.schema.name.clone();
        self.inner
            .extension_state
            .borrow_mut()
            .ensure_custom_schema(contract.schema)?;
        Ok(ArrayExtensionRegistrationReceipt {
            kind: ExtensionContractKind::CustomOp,
            subject,
            backend: self.inner.device.backend().to_string(),
            dispatch_contracts: Vec::new(),
        })
    }

    /// Registers one custom-kernel extension contract on the current context
    /// and resolves its dispatch for the current backend lane.
    pub fn register_custom_kernel_extension(
        &self,
        contract: CustomKernelExtensionContract,
    ) -> Result<ArrayExtensionRegistrationReceipt, ArrayError> {
        contract
            .validate()
            .map_err(|refusal| ArrayError::ExtensionContractRefusal { refusal })?;
        let backend = self.inner.device.backend().to_string();
        let mut extension_state = self.inner.extension_state.borrow_mut();
        extension_state.ensure_custom_schema(contract.schema.clone())?;
        extension_state
            .registry
            .register_kernel(contract.registration.clone())?;
        let dispatch = extension_state
            .registry
            .resolve_dispatch(contract.schema.name.as_str(), backend.as_str())?;
        Ok(ArrayExtensionRegistrationReceipt {
            kind: ExtensionContractKind::CustomKernel,
            subject: contract.schema.name,
            backend,
            dispatch_contracts: vec![dispatch],
        })
    }

    /// Registers one backend-plugin extension contract on the current context.
    pub fn register_backend_plugin_extension(
        &self,
        contract: BackendPluginExtensionContract,
    ) -> Result<ArrayExtensionRegistrationReceipt, ArrayError> {
        contract
            .validate()
            .map_err(|refusal| ArrayError::ExtensionContractRefusal { refusal })?;
        let mut extension_state = self.inner.extension_state.borrow_mut();
        for schema in &contract.custom_schemas {
            extension_state.ensure_custom_schema(schema.clone())?;
        }
        for registration in &contract.kernel_registrations {
            extension_state
                .registry
                .register_kernel(registration.clone())?;
        }
        let dispatch_contracts = contract
            .kernel_registrations
            .iter()
            .map(|registration| {
                extension_state
                    .registry
                    .resolve_dispatch(registration.name.as_str(), contract.backend_label.as_str())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ArrayExtensionRegistrationReceipt {
            kind: ExtensionContractKind::BackendPlugin,
            subject: contract.plugin_id,
            backend: contract.backend_label,
            dispatch_contracts,
        })
    }

    /// Registers one quantizer-plugin extension contract on the current context.
    pub fn register_quantizer_plugin_extension(
        &self,
        contract: QuantizerPluginExtensionContract,
    ) -> Result<ArrayExtensionRegistrationReceipt, ArrayError> {
        contract
            .validate()
            .map_err(|refusal| ArrayError::ExtensionContractRefusal { refusal })?;
        let plugin_id = contract.plugin_id.clone();
        let mut extension_state = self.inner.extension_state.borrow_mut();
        if extension_state
            .quantizer_plugins
            .insert(plugin_id.clone(), contract)
            .is_some()
        {
            return Err(ArrayError::DuplicateQuantizerPlugin { plugin_id });
        }
        Ok(ArrayExtensionRegistrationReceipt {
            kind: ExtensionContractKind::QuantizerPlugin,
            subject: plugin_id,
            backend: self.inner.device.backend().to_string(),
            dispatch_contracts: Vec::new(),
        })
    }

    /// Resolves one custom or built-in operator dispatch contract against the
    /// current backend lane.
    pub fn resolve_extension_dispatch(
        &self,
        name: &str,
    ) -> Result<OperatorDispatchContract, ArrayError> {
        self.inner
            .extension_state
            .borrow()
            .registry
            .resolve_dispatch(name, self.inner.device.backend())
            .map_err(ArrayError::from)
    }

    /// Validates one declared dense custom output against the current extension registry.
    pub fn validate_declared_custom_output(
        &self,
        name: &str,
        input_count: usize,
        declared_output: Option<&TensorSpec>,
    ) -> Result<TensorSpec, ArrayError> {
        self.inner
            .extension_state
            .borrow()
            .registry
            .validate_declared_custom_output(name, input_count, declared_output)
            .map_err(ArrayError::from)
    }

    /// Validates one declared meta custom output against the current extension registry.
    pub fn validate_declared_custom_meta_output(
        &self,
        name: &str,
        input_count: usize,
        declared_output: Option<&MetaTensor>,
    ) -> Result<MetaTensor, ArrayError> {
        self.inner
            .extension_state
            .borrow()
            .registry
            .validate_declared_custom_meta_output(name, input_count, declared_output)
            .map_err(ArrayError::from)
    }

    /// Applies explicit cache limits to the bounded runtime surface.
    pub fn configure_cache_limits(
        &self,
        limits: ArrayCacheLimitControl,
    ) -> ArrayRuntimeResourceReport {
        let memory_capacity_bytes = self.inner.device.descriptor().memory_capacity_bytes;
        let mut runtime_state = self.inner.runtime_state.borrow_mut();
        runtime_state.apply_cache_limits(limits, memory_capacity_bytes);
        let report = runtime_state.report();
        drop(runtime_state);
        self.inner.record_backend_log(
            ArrayBackendLogKind::CachePolicyUpdated,
            format!(
                "updated runtime cache limits for backend `{}`",
                self.inner.device.backend()
            ),
            None,
            None,
        );
        report
    }

    /// Explicitly resets one or more bounded runtime cache families.
    pub fn reset_runtime_caches(&self, scopes: &[ArrayCacheResetScope]) -> ArrayCacheResetReceipt {
        let receipt = self.inner.runtime_state.borrow_mut().reset_caches(scopes);
        self.inner.record_backend_log(
            ArrayBackendLogKind::CachesReset,
            format!(
                "reset runtime caches for backend `{}` across {} scope(s)",
                self.inner.device.backend(),
                scopes.len()
            ),
            None,
            None,
        );
        receipt
    }

    /// Allocates a new explicit stream handle on the current device.
    #[must_use]
    pub fn new_stream(&self) -> ArrayStream {
        let stream_id = {
            let mut next = self.inner.next_stream_id.borrow_mut();
            let stream_id = *next;
            *next += 1;
            stream_id
        };
        ArrayStream::explicit(stream_id, self.inner.device.clone())
    }

    /// Returns a sibling context that uses the provided stream.
    pub fn with_stream(&self, stream: ArrayStream) -> Result<Self, ArrayError> {
        if stream.device().device() != self.device() {
            return Err(ArrayError::StreamDeviceMismatch {
                stream_id: stream.stream_id(),
                stream_device: stream.device().device().to_string(),
                context_device: self.device().to_string(),
            });
        }
        Ok(Self {
            inner: self.inner.clone(),
            stream,
        })
    }

    /// Adds a named input array to the current lazy graph.
    #[must_use]
    pub fn input(&self, name: impl Into<String>, shape: Shape, dtype: DType) -> Array {
        let tensor = self.inner.builder.borrow_mut().input(name, shape, dtype);
        Array::from_tensor(self.inner.clone(), self.stream.clone(), tensor)
    }

    /// Adds an `f32` constant array to the current lazy graph.
    pub fn constant_f32(
        &self,
        shape: Shape,
        values: impl Into<Vec<f32>>,
    ) -> Result<Array, ArrayError> {
        let tensor = self
            .inner
            .builder
            .borrow_mut()
            .constant_f32(shape, values)?;
        Ok(Array::from_tensor(
            self.inner.clone(),
            self.stream.clone(),
            tensor,
        ))
    }

    /// Adds a scalar `f32` constant to the current lazy graph.
    pub fn scalar_f32(&self, value: f32) -> Result<Array, ArrayError> {
        self.constant_f32(Shape::scalar(), vec![value])
    }

    /// Adds a zero-filled `f32` array to the current lazy graph.
    pub fn zeros_f32(&self, shape: Shape) -> Result<Array, ArrayError> {
        self.full_f32(shape, 0.0)
    }

    /// Adds a one-filled `f32` array to the current lazy graph.
    pub fn ones_f32(&self, shape: Shape) -> Result<Array, ArrayError> {
        self.full_f32(shape, 1.0)
    }

    /// Adds an `f32` array filled with one repeated value.
    pub fn full_f32(&self, shape: Shape, value: f32) -> Result<Array, ArrayError> {
        self.constant_f32(shape.clone(), vec![value; shape.element_count()])
    }

    /// Adds a deterministic or best-effort random-uniform `f32` array.
    pub fn random_uniform_f32(
        &self,
        shape: Shape,
        low: f32,
        high: f32,
    ) -> Result<Array, ArrayError> {
        if high <= low {
            return Err(ArrayError::InvalidRandomUniformBounds { low, high });
        }
        let span = high - low;
        let values = self.draw_random_f32_values(shape.element_count(), |rng| {
            (rng.random::<f32>() * span) + low
        })?;
        self.constant_f32(shape, values)
    }

    /// Adds a deterministic or best-effort random-normal `f32` array.
    pub fn random_normal_f32(
        &self,
        shape: Shape,
        mean: f32,
        stddev: f32,
    ) -> Result<Array, ArrayError> {
        if stddev <= 0.0 {
            return Err(ArrayError::InvalidRandomNormalStddev { stddev });
        }
        let values = self.draw_random_normal_f32_values(shape.element_count(), mean, stddev)?;
        self.constant_f32(shape, values)
    }

    /// Adds an `f32` arange vector.
    pub fn arange_f32(&self, start: f32, stop: f32, step: f32) -> Result<Array, ArrayError> {
        if step == 0.0 || (start < stop && step < 0.0) || (start > stop && step > 0.0) {
            return Err(ArrayError::InvalidArangeStep { start, stop, step });
        }
        let mut values = Vec::new();
        let mut current = start;
        if step > 0.0 {
            while current < stop {
                values.push(current);
                current += step;
            }
        } else {
            while current > stop {
                values.push(current);
                current += step;
            }
        }
        self.constant_f32(Shape::new(vec![values.len()]), values)
    }

    /// Adds an `f32` linspace vector with an inclusive stop value.
    pub fn linspace_f32(&self, start: f32, stop: f32, count: usize) -> Result<Array, ArrayError> {
        if count == 0 {
            return Err(ArrayError::InvalidLinspaceCount);
        }
        let values = if count == 1 {
            vec![start]
        } else {
            let step = (stop - start) / (count - 1) as f32;
            (0..count)
                .map(|index| start + (step * index as f32))
                .collect::<Vec<_>>()
        };
        self.constant_f32(Shape::new(vec![count]), values)
    }

    /// Adds an `f32` eye matrix.
    pub fn eye_f32(&self, rows: usize, cols: usize) -> Result<Array, ArrayError> {
        let mut values = vec![0.0; rows.saturating_mul(cols)];
        for diagonal in 0..rows.min(cols) {
            values[diagonal * cols + diagonal] = 1.0;
        }
        self.constant_f32(Shape::new(vec![rows, cols]), values)
    }

    fn draw_random_f32_values<F>(&self, count: usize, mut draw: F) -> Result<Vec<f32>, ArrayError>
    where
        F: FnMut(&mut StdRng) -> f32,
    {
        let mut determinism = self.inner.determinism.borrow_mut();
        let mut rng = determinism
            .generator
            .as_ref()
            .map_or_else(StdRng::from_os_rng, GeneratorState::restored_rng);
        let values = (0..count).map(|_| draw(&mut rng)).collect::<Vec<_>>();
        if let Some(generator) = determinism.generator.as_mut() {
            generator.draws = generator.draws.saturating_add(count as u64);
        }
        Ok(values)
    }

    fn draw_random_normal_f32_values(
        &self,
        count: usize,
        mean: f32,
        stddev: f32,
    ) -> Result<Vec<f32>, ArrayError> {
        let mut determinism = self.inner.determinism.borrow_mut();
        let mut rng = determinism
            .generator
            .as_ref()
            .map_or_else(StdRng::from_os_rng, GeneratorState::restored_rng);
        let mut values = Vec::with_capacity(count);
        let mut draws = 0_u64;
        while values.len() < count {
            let u1 = rng.random::<f32>().max(f32::MIN_POSITIVE);
            let u2 = rng.random::<f32>();
            draws = draws.saturating_add(2);
            let radius = (-2.0 * u1.ln()).sqrt();
            let theta = std::f32::consts::TAU * u2;
            values.push(mean + (stddev * radius * theta.cos()));
            if values.len() < count {
                values.push(mean + (stddev * radius * theta.sin()));
            }
        }
        if let Some(generator) = determinism.generator.as_mut() {
            generator.draws = generator.draws.saturating_add(draws);
        }
        Ok(values)
    }

    /// Snapshots the current context graph with the provided output arrays.
    ///
    /// This returns the current builder snapshot rather than a pruned subgraph.
    /// Later issues will refine execution and materialization behavior above the
    /// public lazy-array layer.
    pub fn graph_for(&self, outputs: &[Array]) -> Result<Graph, ArrayError> {
        for output in outputs {
            if !output.belongs_to_context(&self.inner) {
                return Err(ArrayError::MixedContexts);
            }
        }
        let tensors = outputs.iter().map(Array::tensor_handle).collect::<Vec<_>>();
        Ok(self.inner.builder.borrow().clone().finish(tensors))
    }

    /// Explicitly materializes the requested outputs through the bounded
    /// CPU-reference path.
    pub fn eval(&self, outputs: &[Array]) -> Result<Vec<EvaluatedArray>, ArrayError> {
        let graph = self.graph_for(outputs)?;
        let evaluated =
            evaluate_graph_snapshot(&graph, outputs, MaterializationTrigger::Eval, &self.stream)?;
        self.inner
            .runtime_state
            .borrow_mut()
            .record_eval(&graph, outputs);
        self.inner.record_backend_log(
            ArrayBackendLogKind::Materialize,
            format!(
                "materialized graph {} via {} on stream {}",
                graph.stable_digest(),
                MaterializationTrigger::Eval.label(),
                self.stream.stream_id()
            ),
            Some(graph.stable_digest()),
            None,
        );
        Ok(evaluated)
    }

    /// Captures a replay-stable deferred evaluation ticket for the requested
    /// outputs.
    pub fn async_eval(&self, outputs: &[Array]) -> Result<PendingAsyncEval, ArrayError> {
        let graph = self.graph_for(outputs)?;
        Ok(PendingAsyncEval {
            context: self.inner.clone(),
            graph,
            outputs: outputs.to_vec(),
            stream: self.stream.clone(),
        })
    }

    /// Captures compiler-backed debug artifacts plus bounded runtime snapshots
    /// for the requested outputs.
    pub fn capture_backend_debug(
        &self,
        outputs: &[Array],
        config: ArrayBackendCaptureConfig,
    ) -> Result<ArrayBackendCaptureReceipt, ArrayError> {
        let graph = self.graph_for(outputs)?;
        let graph_digest = graph.stable_digest();
        let runtime_observability_before = config
            .include_runtime_snapshot
            .then(|| self.inner.runtime_observability_snapshot());
        let runtime_resources_before = config
            .include_runtime_snapshot
            .then(|| self.inner.runtime_state.borrow().report());
        let mut transform = compile_transform(&graph, config.compile.clone());
        let compile = transform.apply()?;
        let evaluated = evaluate_graph_snapshot(
            &graph,
            outputs,
            MaterializationTrigger::DebugCapture,
            &self.stream,
        )?;
        self.inner
            .runtime_state
            .borrow_mut()
            .record_eval(&graph, outputs);

        let support = self.backend_debug_support();
        let capture_id = {
            self.inner
                .debug_state
                .borrow_mut()
                .next_capture_id(self.inner.device.backend())
        };
        let runtime_observability_after = config
            .include_runtime_snapshot
            .then(|| self.inner.runtime_observability_snapshot());
        let runtime_resources_after = config
            .include_runtime_snapshot
            .then(|| self.inner.runtime_state.borrow().report());

        let mut receipt = ArrayBackendCaptureReceipt {
            capture_id: capture_id.clone(),
            graph_digest: graph_digest.clone(),
            backend: self.inner.backend_identity.clone(),
            support: support.clone(),
            label: config.label.clone(),
            compile,
            runtime_observability_before,
            runtime_observability_after,
            runtime_resources_before,
            runtime_resources_after,
            eval_receipts: evaluated
                .iter()
                .map(|evaluated| evaluated.receipt().clone())
                .collect(),
            recent_logs: Vec::new(),
            artifact: None,
        };
        if let Some(request) = &config.artifact {
            let format = request
                .format
                .unwrap_or_else(|| support.default_capture_format());
            if !support.supports_format(format) {
                return Err(ArrayError::UnsupportedBackendDebugFormat {
                    backend: self.inner.device.backend().to_string(),
                    format,
                });
            }
            receipt.artifact = Some(write_backend_capture_artifact(request, format, &receipt)?);
        }
        self.inner.record_backend_log(
            ArrayBackendLogKind::Capture,
            format!(
                "captured backend debug bundle for graph {} on stream {}",
                graph_digest,
                self.stream.stream_id()
            ),
            Some(graph_digest.clone()),
            Some(capture_id.clone()),
        );
        self.inner
            .debug_state
            .borrow_mut()
            .record_capture(ArrayBackendCaptureSummary {
                capture_id,
                backend: self.inner.device.backend().to_string(),
                graph_digest,
                trace_mode: config.compile.trace_mode,
                debug_mode: config.compile.debug_mode,
                artifact: receipt.artifact.clone(),
                observed_at_millis: current_time_millis(),
            });
        receipt.recent_logs = self
            .inner
            .debug_state
            .borrow()
            .recent_logs
            .iter()
            .cloned()
            .collect();
        Ok(receipt)
    }
}

/// Public lazy array handle backed by the canonical Psionic graph builder.
#[derive(Clone, Debug)]
pub struct Array {
    context: Rc<ArrayContextInner>,
    stream: ArrayStream,
    tensor: Tensor,
    graph: Graph,
}

/// Convenience alias for trees of lazy arrays.
pub type ArrayTree = Tree<Array>;

/// Convenience alias for trees of evaluated arrays.
pub type EvaluatedArrayTree = Tree<EvaluatedArray>;

/// Convenience alias for trees of host-owned array data.
pub type HostArrayTree = Tree<HostArrayData>;

/// Convenience alias for trees of exported scalar values.
pub type ScalarTree = Tree<ArrayScalar>;

impl Array {
    fn from_tensor(context: Rc<ArrayContextInner>, stream: ArrayStream, tensor: Tensor) -> Self {
        context.runtime_state.borrow_mut().record_tensor(&tensor);
        let graph = context
            .builder
            .borrow()
            .clone()
            .finish(vec![tensor.clone()]);
        Self {
            context,
            stream,
            tensor,
            graph,
        }
    }

    fn tensor_handle(&self) -> Tensor {
        self.tensor.clone()
    }

    fn belongs_to_context(&self, context: &Rc<ArrayContextInner>) -> bool {
        Rc::ptr_eq(&self.context, context)
    }

    fn require_same_context(&self, other: &Self) -> Result<(), ArrayError> {
        if self.belongs_to_context(&other.context) {
            Ok(())
        } else {
            Err(ArrayError::MixedContexts)
        }
    }

    fn binary_op<F>(&self, other: &Self, op: F) -> Result<Self, ArrayError>
    where
        F: FnOnce(&mut GraphBuilder, &Tensor, &Tensor) -> Result<Tensor, GraphError>,
    {
        self.require_same_context(other)?;
        let tensor = {
            let mut builder = self.context.builder.borrow_mut();
            op(&mut builder, &self.tensor, &other.tensor)?
        };
        Ok(Self::from_tensor(
            self.context.clone(),
            self.stream.clone(),
            tensor,
        ))
    }

    /// Returns the owning public graph-construction context.
    #[must_use]
    pub fn context(&self) -> ArrayContext {
        ArrayContext {
            inner: self.context.clone(),
            stream: self.stream.clone(),
        }
    }

    /// Returns the stable tensor identifier for this array node.
    #[must_use]
    pub const fn tensor_id(&self) -> TensorId {
        self.tensor.id()
    }

    /// Returns the current tensor specification.
    #[must_use]
    pub fn spec(&self) -> &TensorSpec {
        self.tensor.spec()
    }

    /// Returns the logical array shape.
    #[must_use]
    pub fn shape(&self) -> &Shape {
        self.tensor.spec().shape()
    }

    /// Returns the array dtype.
    #[must_use]
    pub fn dtype(&self) -> DType {
        self.tensor.spec().dtype()
    }

    /// Returns the device pinned to the owning context.
    #[must_use]
    pub fn device(&self) -> &Device {
        self.tensor.spec().device()
    }

    /// Returns the public device handle for the array context.
    #[must_use]
    pub fn device_handle(&self) -> &ArrayDevice {
        &self.context.device
    }

    /// Returns the public stream handle for the array.
    #[must_use]
    pub fn stream(&self) -> &ArrayStream {
        &self.stream
    }

    /// Returns the lazy operation provenance for this array.
    #[must_use]
    pub fn lazy_op(&self) -> &LazyOp {
        self.tensor.op()
    }

    /// Returns the graph snapshot captured when this array was created.
    #[must_use]
    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Returns the current public materialization boundary for this array.
    #[must_use]
    pub fn materialization_boundary(&self) -> MaterializationBoundary {
        MaterializationBoundary::explicit_only()
    }

    fn materialize_with_trigger(
        &self,
        trigger: MaterializationTrigger,
    ) -> Result<EvaluatedArray, ArrayError> {
        let graph = self.context().graph_for(std::slice::from_ref(self))?;
        let mut outputs =
            evaluate_graph_snapshot(&graph, std::slice::from_ref(self), trigger, self.stream())?;
        self.context
            .runtime_state
            .borrow_mut()
            .record_eval(&graph, std::slice::from_ref(self));
        self.context.record_backend_log(
            ArrayBackendLogKind::Materialize,
            format!(
                "materialized graph {} via {} on stream {}",
                graph.stable_digest(),
                trigger.label(),
                self.stream.stream_id()
            ),
            Some(graph.stable_digest()),
            None,
        );
        Ok(outputs.remove(0))
    }

    /// Explicitly materializes this array through the bounded CPU-reference
    /// path.
    pub fn eval(&self) -> Result<EvaluatedArray, ArrayError> {
        self.materialize_with_trigger(MaterializationTrigger::Eval)
    }

    /// Captures a replay-stable deferred evaluation ticket for this array.
    pub fn async_eval(&self) -> Result<PendingAsyncEval, ArrayError> {
        self.context().async_eval(std::slice::from_ref(self))
    }

    /// Explicitly exports this array into one host-owned typed buffer.
    pub fn to_host_data(&self) -> Result<HostArrayData, ArrayError> {
        self.materialize_with_trigger(MaterializationTrigger::ToHostData)?
            .to_host_data()
    }

    /// Explicitly extracts one singleton scalar from this array.
    pub fn item(&self) -> Result<ArrayScalar, ArrayError> {
        self.materialize_with_trigger(MaterializationTrigger::Item)?
            .item()
    }

    /// Returns the dependency policy for using this array after `upstream`.
    #[must_use]
    pub fn dependency_policy_after(&self, upstream: &Self) -> StreamDependencyPolicy {
        self.stream.dependency_policy_after(upstream.stream())
    }

    /// Reshapes the array without changing the element count.
    pub fn reshape(&self, shape: Shape) -> Result<Self, ArrayError> {
        let tensor = self
            .context
            .builder
            .borrow_mut()
            .reshape(&self.tensor, shape)?;
        Ok(Self::from_tensor(
            self.context.clone(),
            self.stream.clone(),
            tensor,
        ))
    }

    /// Reorders axes using a logical view.
    pub fn permute(&self, axes: Vec<usize>) -> Result<Self, ArrayError> {
        let tensor = self
            .context
            .builder
            .borrow_mut()
            .permute(&self.tensor, axes)?;
        Ok(Self::from_tensor(
            self.context.clone(),
            self.stream.clone(),
            tensor,
        ))
    }

    /// Convenience transpose that reverses the current axis order.
    pub fn transpose(&self) -> Result<Self, ArrayError> {
        let axes = (0..self.shape().rank()).rev().collect::<Vec<_>>();
        self.permute(axes)
    }

    /// Returns a narrowed slice along one axis.
    pub fn slice(&self, axis: usize, start: usize, end: usize) -> Result<Self, ArrayError> {
        let tensor = self
            .context
            .builder
            .borrow_mut()
            .slice(&self.tensor, axis, start, end)?;
        Ok(Self::from_tensor(
            self.context.clone(),
            self.stream.clone(),
            tensor,
        ))
    }

    /// Selects one index along one axis and removes that axis.
    pub fn select(&self, axis: usize, index: usize) -> Result<Self, ArrayError> {
        let tensor = self
            .context
            .builder
            .borrow_mut()
            .select(&self.tensor, axis, index)?;
        Ok(Self::from_tensor(
            self.context.clone(),
            self.stream.clone(),
            tensor,
        ))
    }

    /// Broadcasts the array to the provided target shape.
    pub fn broadcast_to(&self, shape: Shape) -> Result<Self, ArrayError> {
        let tensor = self
            .context
            .builder
            .borrow_mut()
            .expand(&self.tensor, shape)?;
        Ok(Self::from_tensor(
            self.context.clone(),
            self.stream.clone(),
            tensor,
        ))
    }

    /// Casts the array to one logical dtype.
    pub fn cast(&self, dtype: DType) -> Result<Self, ArrayError> {
        let tensor = self
            .context
            .builder
            .borrow_mut()
            .cast(&self.tensor, dtype)?;
        Ok(Self::from_tensor(
            self.context.clone(),
            self.stream.clone(),
            tensor,
        ))
    }

    /// Concatenates multiple arrays along one axis.
    pub fn concat(inputs: &[Self], axis: usize) -> Result<Self, ArrayError> {
        let (first, rest) = inputs.split_first().ok_or(ArrayError::EmptyArrayList)?;
        for input in rest {
            first.require_same_context(input)?;
        }
        let tensors = inputs.iter().map(Array::tensor_handle).collect::<Vec<_>>();
        let tensor = first
            .context
            .builder
            .borrow_mut()
            .concat(tensors.as_slice(), axis)?;
        Ok(Self::from_tensor(
            first.context.clone(),
            first.stream.clone(),
            tensor,
        ))
    }

    /// Adds two arrays using the lower IR broadcast semantics.
    pub fn add(&self, other: &Self) -> Result<Self, ArrayError> {
        self.binary_op(other, GraphBuilder::add)
    }

    /// Multiplies two arrays using the lower IR broadcast semantics.
    pub fn mul(&self, other: &Self) -> Result<Self, ArrayError> {
        self.binary_op(other, GraphBuilder::mul)
    }

    /// Matrix-multiplies two arrays using the lower IR shape rules.
    pub fn matmul(&self, other: &Self) -> Result<Self, ArrayError> {
        self.binary_op(other, GraphBuilder::matmul)
    }

    /// Inserts a gradient-stopping identity node.
    #[must_use]
    pub fn detach(&self) -> Self {
        let tensor = self.context.builder.borrow_mut().detach(&self.tensor);
        Self::from_tensor(self.context.clone(), self.stream.clone(), tensor)
    }

    /// Reduces the array to a scalar sum.
    #[must_use]
    pub fn sum(&self) -> Self {
        let tensor = self.context.builder.borrow_mut().reduce_sum(&self.tensor);
        Self::from_tensor(self.context.clone(), self.stream.clone(), tensor)
    }
}

impl Tree<Array> {
    /// Explicitly evaluates every array leaf while preserving structure.
    pub fn eval(&self) -> Result<EvaluatedArrayTree, ArrayError> {
        self.map_leaves(&mut Array::eval)
    }

    /// Explicitly exports every array leaf into host-owned buffers.
    pub fn to_host_data(&self) -> Result<HostArrayTree, ArrayError> {
        self.map_leaves(&mut Array::to_host_data)
    }

    /// Explicitly extracts one scalar from every singleton array leaf.
    pub fn item(&self) -> Result<ScalarTree, ArrayError> {
        self.map_leaves(&mut Array::item)
    }
}

impl Tree<EvaluatedArray> {
    /// Exports every evaluated leaf into host-owned buffers while preserving structure.
    pub fn to_host_data(&self) -> Result<HostArrayTree, ArrayError> {
        self.map_leaves(&mut EvaluatedArray::to_host_data)
    }

    /// Extracts one scalar from every singleton evaluated leaf.
    pub fn item(&self) -> Result<ScalarTree, ArrayError> {
        self.map_leaves(&mut EvaluatedArray::item)
    }
}

fn evaluate_graph_snapshot(
    graph: &Graph,
    requested_outputs: &[Array],
    trigger: MaterializationTrigger,
    stream: &ArrayStream,
) -> Result<Vec<EvaluatedArray>, ArrayError> {
    let mut values = BTreeMap::<TensorId, DenseValue>::new();

    for node in graph.nodes() {
        let dense = match node.op() {
            OpKind::Input { name } => {
                return Err(ArrayError::UnboundInput {
                    tensor: node.tensor().id(),
                    name: name.clone(),
                });
            }
            OpKind::Constant { data } => DenseValue {
                tensor: node.tensor().clone(),
                values: dense_constant_values(node.tensor().id(), data)?,
            },
            OpKind::Detach => clone_input_value(node.tensor(), node.inputs(), &values)?,
            OpKind::Add => binary_dense_value(node.tensor(), node.inputs(), &values, |l, r| l + r)?,
            OpKind::Mul => binary_dense_value(node.tensor(), node.inputs(), &values, |l, r| l * r)?,
            OpKind::Matmul => matmul_dense_value(node.tensor(), node.inputs(), &values)?,
            OpKind::Reshape => reshape_dense_value(node.tensor(), node.inputs(), &values)?,
            OpKind::Permute { axes } => {
                permute_dense_value(node.tensor(), node.inputs(), &values, axes.as_slice())?
            }
            OpKind::Slice { axis, start, end } => {
                slice_dense_value(node.tensor(), node.inputs(), &values, *axis, *start, *end)?
            }
            OpKind::Select { axis, index } => {
                select_dense_value(node.tensor(), node.inputs(), &values, *axis, *index)?
            }
            OpKind::Concat { axis } => {
                concat_dense_value(node.tensor(), node.inputs(), &values, *axis)?
            }
            OpKind::Expand { shape } => {
                expand_dense_value(node.tensor(), node.inputs(), &values, shape)?
            }
            OpKind::Cast { dtype } => {
                cast_dense_value(node.tensor(), node.inputs(), &values, *dtype)?
            }
            OpKind::ReduceSum { axis } => {
                reduce_sum_dense_value(node.tensor(), node.inputs(), &values, *axis)?
            }
            other => {
                return Err(ArrayError::MaterializationRefusal {
                    tensor: node.tensor().id(),
                    op: other.label().to_string(),
                    detail: String::from(
                        "bounded explicit eval currently materializes only constant, detach, add, mul, matmul, reshape, permute, slice, select, concat, expand, cast, and reduce_sum graphs",
                    ),
                });
            }
        };
        values.insert(node.tensor().id(), dense);
    }

    let receipt = EvalReceipt {
        graph_digest: graph.stable_digest(),
        outputs: requested_outputs.iter().map(Array::tensor_id).collect(),
        trigger,
        replay_boundary: ReplayBoundary::GraphSnapshot,
        device_id: stream.device().stable_id().to_string(),
        stream_id: stream.stream_id(),
    };

    requested_outputs
        .iter()
        .map(|array| {
            let value = values
                .get(&array.tensor_id())
                .ok_or(ArrayError::MissingDependency {
                    tensor: array.tensor_id(),
                    input: array.tensor_id(),
                })?;
            Ok(EvaluatedArray {
                tensor: value.tensor.clone(),
                data: TensorData::F32(value.values.clone()),
                receipt: receipt.clone(),
                boundary: array.materialization_boundary(),
            })
        })
        .collect()
}

fn host_array_data_from_evaluated(
    tensor: TensorId,
    dtype: DType,
    data: &TensorData,
) -> Result<HostArrayData, ArrayError> {
    let Some(values) = data.as_f32_slice() else {
        return Err(ArrayError::HostInteropRefusal {
            tensor,
            dtype,
            detail: String::from(
                "bounded explicit host interop currently exports only dense CPU-reference payloads",
            ),
        });
    };
    let host_values = match dtype {
        DType::F32 | DType::F16 | DType::BF16 => HostArrayStorage::F32(values.to_vec()),
        DType::I8 => HostArrayStorage::I8(
            values
                .iter()
                .map(|value| value.round().clamp(i8::MIN as f32, i8::MAX as f32) as i8)
                .collect(),
        ),
    };
    Ok(HostArrayData::new(dtype, host_values))
}

fn scalar_from_evaluated(
    tensor: TensorId,
    shape: &Shape,
    dtype: DType,
    data: &TensorData,
) -> Result<ArrayScalar, ArrayError> {
    if shape.element_count() != 1 {
        return Err(ArrayError::NonSingletonItem {
            tensor,
            shape: shape.clone(),
        });
    }
    let host = host_array_data_from_evaluated(tensor, dtype, data)?;
    let value = match &host.values {
        HostArrayStorage::F32(values) => {
            let Some(value) = values.first() else {
                unreachable!("singleton check should guarantee one f32 value")
            };
            HostScalarValue::F32(*value)
        }
        HostArrayStorage::I8(values) => {
            let Some(value) = values.first() else {
                unreachable!("singleton check should guarantee one i8 value")
            };
            HostScalarValue::I8(*value)
        }
    };
    Ok(ArrayScalar::new(dtype, value))
}

fn dense_constant_values(tensor: TensorId, data: &TensorData) -> Result<Vec<f32>, ArrayError> {
    match data {
        TensorData::F32(values) => Ok(values.clone()),
        TensorData::QuantizedBlocks(_) => Err(ArrayError::MaterializationRefusal {
            tensor,
            op: String::from("constant"),
            detail: String::from(
                "bounded explicit eval does not materialize quantized block payloads yet",
            ),
        }),
    }
}

fn clone_input_value(
    tensor: &Tensor,
    inputs: &[TensorId],
    values: &BTreeMap<TensorId, DenseValue>,
) -> Result<DenseValue, ArrayError> {
    let input = inputs
        .first()
        .copied()
        .ok_or(ArrayError::MaterializationRefusal {
            tensor: tensor.id(),
            op: String::from("detach"),
            detail: String::from("detach requires one input"),
        })?;
    let source = values
        .get(&input)
        .cloned()
        .ok_or(ArrayError::MissingDependency {
            tensor: tensor.id(),
            input,
        })?;
    Ok(DenseValue {
        tensor: tensor.clone(),
        values: source.values,
    })
}

fn binary_dense_value<F>(
    tensor: &Tensor,
    inputs: &[TensorId],
    values: &BTreeMap<TensorId, DenseValue>,
    op: F,
) -> Result<DenseValue, ArrayError>
where
    F: Fn(f32, f32) -> f32,
{
    let [left_id, right_id] = inputs else {
        return Err(ArrayError::MaterializationRefusal {
            tensor: tensor.id(),
            op: String::from("binary"),
            detail: String::from("binary ops require exactly two inputs"),
        });
    };
    let left = values.get(left_id).ok_or(ArrayError::MissingDependency {
        tensor: tensor.id(),
        input: *left_id,
    })?;
    let right = values.get(right_id).ok_or(ArrayError::MissingDependency {
        tensor: tensor.id(),
        input: *right_id,
    })?;
    if left.tensor.spec().shape() != right.tensor.spec().shape() {
        return Err(ArrayError::MaterializationRefusal {
            tensor: tensor.id(),
            op: String::from("binary"),
            detail: String::from(
                "bounded explicit eval expects binary inputs to be shape-aligned after graph expansion",
            ),
        });
    }
    let output = left
        .values
        .iter()
        .zip(right.values.iter())
        .map(|(left, right)| op(*left, *right))
        .collect::<Vec<_>>();
    Ok(DenseValue {
        tensor: tensor.clone(),
        values: output,
    })
}

fn matmul_dense_value(
    tensor: &Tensor,
    inputs: &[TensorId],
    values: &BTreeMap<TensorId, DenseValue>,
) -> Result<DenseValue, ArrayError> {
    let [left_id, right_id] = inputs else {
        return Err(ArrayError::MaterializationRefusal {
            tensor: tensor.id(),
            op: String::from("matmul"),
            detail: String::from("matmul requires exactly two inputs"),
        });
    };
    let left = values.get(left_id).ok_or(ArrayError::MissingDependency {
        tensor: tensor.id(),
        input: *left_id,
    })?;
    let right = values.get(right_id).ok_or(ArrayError::MissingDependency {
        tensor: tensor.id(),
        input: *right_id,
    })?;
    let left_shape = left.tensor.spec().shape();
    let right_shape = right.tensor.spec().shape();
    let (m, k_left, k_right, n) = match (left_shape.dims(), right_shape.dims()) {
        ([m, k_left], [k_right, n]) => (*m, *k_left, *k_right, *n),
        _ => {
            return Err(ArrayError::MaterializationRefusal {
                tensor: tensor.id(),
                op: String::from("matmul"),
                detail: String::from("bounded explicit eval only materializes rank-2 matmul"),
            });
        }
    };
    if k_left != k_right {
        return Err(ArrayError::MaterializationRefusal {
            tensor: tensor.id(),
            op: String::from("matmul"),
            detail: String::from("matmul inner dimensions must agree"),
        });
    }
    let mut output = vec![0.0; m * n];
    for row in 0..m {
        for col in 0..n {
            let mut sum = 0.0;
            for inner in 0..k_left {
                let left_index = row * k_left + inner;
                let right_index = inner * n + col;
                sum += left.values[left_index] * right.values[right_index];
            }
            output[row * n + col] = sum;
        }
    }
    Ok(DenseValue {
        tensor: tensor.clone(),
        values: output,
    })
}

fn reshape_dense_value(
    tensor: &Tensor,
    inputs: &[TensorId],
    values: &BTreeMap<TensorId, DenseValue>,
) -> Result<DenseValue, ArrayError> {
    let input = inputs
        .first()
        .copied()
        .ok_or(ArrayError::MaterializationRefusal {
            tensor: tensor.id(),
            op: String::from("reshape"),
            detail: String::from("reshape requires one input"),
        })?;
    let value = values.get(&input).ok_or(ArrayError::MissingDependency {
        tensor: tensor.id(),
        input,
    })?;
    Ok(DenseValue {
        tensor: tensor.clone(),
        values: value.values.clone(),
    })
}

fn permute_dense_value(
    tensor: &Tensor,
    inputs: &[TensorId],
    values: &BTreeMap<TensorId, DenseValue>,
    axes: &[usize],
) -> Result<DenseValue, ArrayError> {
    let input = inputs
        .first()
        .copied()
        .ok_or(ArrayError::MaterializationRefusal {
            tensor: tensor.id(),
            op: String::from("permute"),
            detail: String::from("permute requires one input"),
        })?;
    let value = values.get(&input).ok_or(ArrayError::MissingDependency {
        tensor: tensor.id(),
        input,
    })?;
    let output_shape = tensor.spec().shape();
    let input_shape = value.tensor.spec().shape();
    let mut output = Vec::with_capacity(output_shape.element_count());
    for output_index in 0..output_shape.element_count() {
        let output_coords = unravel_index(output_index, output_shape.dims());
        let mut input_coords = vec![0; input_shape.rank()];
        for (output_axis, input_axis) in axes.iter().copied().enumerate() {
            input_coords[input_axis] = output_coords[output_axis];
        }
        let input_index = ravel_index(&input_coords, input_shape.dims());
        output.push(value.values[input_index]);
    }
    Ok(DenseValue {
        tensor: tensor.clone(),
        values: output,
    })
}

fn slice_dense_value(
    tensor: &Tensor,
    inputs: &[TensorId],
    values: &BTreeMap<TensorId, DenseValue>,
    axis: usize,
    start: usize,
    end: usize,
) -> Result<DenseValue, ArrayError> {
    let input = inputs
        .first()
        .copied()
        .ok_or(ArrayError::MaterializationRefusal {
            tensor: tensor.id(),
            op: String::from("slice"),
            detail: String::from("slice requires one input"),
        })?;
    let value = values.get(&input).ok_or(ArrayError::MissingDependency {
        tensor: tensor.id(),
        input,
    })?;
    let output_shape = tensor.spec().shape();
    let input_shape = value.tensor.spec().shape();
    let mut output = Vec::with_capacity(output_shape.element_count());
    for output_index in 0..output_shape.element_count() {
        let mut input_coords = unravel_index(output_index, output_shape.dims());
        input_coords[axis] += start;
        debug_assert!(input_coords[axis] < end);
        let input_index = ravel_index(&input_coords, input_shape.dims());
        output.push(value.values[input_index]);
    }
    Ok(DenseValue {
        tensor: tensor.clone(),
        values: output,
    })
}

fn select_dense_value(
    tensor: &Tensor,
    inputs: &[TensorId],
    values: &BTreeMap<TensorId, DenseValue>,
    axis: usize,
    index: usize,
) -> Result<DenseValue, ArrayError> {
    let input = inputs
        .first()
        .copied()
        .ok_or(ArrayError::MaterializationRefusal {
            tensor: tensor.id(),
            op: String::from("select"),
            detail: String::from("select requires one input"),
        })?;
    let value = values.get(&input).ok_or(ArrayError::MissingDependency {
        tensor: tensor.id(),
        input,
    })?;
    let output_shape = tensor.spec().shape();
    let input_shape = value.tensor.spec().shape();
    let mut output = Vec::with_capacity(output_shape.element_count());
    for output_index in 0..output_shape.element_count() {
        let output_coords = unravel_index(output_index, output_shape.dims());
        let mut input_coords = Vec::with_capacity(input_shape.rank());
        let mut output_axis = 0;
        for input_axis in 0..input_shape.rank() {
            if input_axis == axis {
                input_coords.push(index);
            } else {
                input_coords.push(output_coords[output_axis]);
                output_axis += 1;
            }
        }
        let input_index = ravel_index(&input_coords, input_shape.dims());
        output.push(value.values[input_index]);
    }
    Ok(DenseValue {
        tensor: tensor.clone(),
        values: output,
    })
}

fn concat_dense_value(
    tensor: &Tensor,
    inputs: &[TensorId],
    values: &BTreeMap<TensorId, DenseValue>,
    axis: usize,
) -> Result<DenseValue, ArrayError> {
    let tensors = inputs
        .iter()
        .map(|input| {
            values
                .get(input)
                .cloned()
                .ok_or(ArrayError::MissingDependency {
                    tensor: tensor.id(),
                    input: *input,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let output_shape = tensor.spec().shape();
    let mut boundaries = Vec::with_capacity(tensors.len());
    let mut running = 0;
    for value in &tensors {
        running += value.tensor.spec().shape().dims()[axis];
        boundaries.push(running);
    }

    let mut output = Vec::with_capacity(output_shape.element_count());
    for output_index in 0..output_shape.element_count() {
        let output_coords = unravel_index(output_index, output_shape.dims());
        let concat_index = output_coords[axis];
        let source_position = boundaries
            .iter()
            .position(|boundary| concat_index < *boundary)
            .ok_or(ArrayError::MaterializationRefusal {
                tensor: tensor.id(),
                op: String::from("concat"),
                detail: String::from("concat output index fell outside source boundaries"),
            })?;
        let source = &tensors[source_position];
        let axis_offset = if source_position == 0 {
            0
        } else {
            boundaries[source_position - 1]
        };
        let mut input_coords = output_coords;
        input_coords[axis] -= axis_offset;
        let input_index = ravel_index(&input_coords, source.tensor.spec().shape().dims());
        output.push(source.values[input_index]);
    }
    Ok(DenseValue {
        tensor: tensor.clone(),
        values: output,
    })
}

fn expand_dense_value(
    tensor: &Tensor,
    inputs: &[TensorId],
    values: &BTreeMap<TensorId, DenseValue>,
    shape: &Shape,
) -> Result<DenseValue, ArrayError> {
    let input = inputs
        .first()
        .copied()
        .ok_or(ArrayError::MaterializationRefusal {
            tensor: tensor.id(),
            op: String::from("expand"),
            detail: String::from("expand requires one input"),
        })?;
    let value = values.get(&input).ok_or(ArrayError::MissingDependency {
        tensor: tensor.id(),
        input,
    })?;
    let expanded = expand_values(value.tensor.spec().shape(), &value.values, shape);
    Ok(DenseValue {
        tensor: tensor.clone(),
        values: expanded,
    })
}

fn cast_dense_value(
    tensor: &Tensor,
    inputs: &[TensorId],
    values: &BTreeMap<TensorId, DenseValue>,
    dtype: DType,
) -> Result<DenseValue, ArrayError> {
    let input = inputs
        .first()
        .copied()
        .ok_or(ArrayError::MaterializationRefusal {
            tensor: tensor.id(),
            op: String::from("cast"),
            detail: String::from("cast requires one input"),
        })?;
    let value = values.get(&input).ok_or(ArrayError::MissingDependency {
        tensor: tensor.id(),
        input,
    })?;
    let converted = value
        .values
        .iter()
        .map(|current| match dtype {
            DType::F32 | DType::F16 | DType::BF16 => *current,
            DType::I8 => current.round().clamp(i8::MIN as f32, i8::MAX as f32),
        })
        .collect::<Vec<_>>();
    Ok(DenseValue {
        tensor: tensor.clone(),
        values: converted,
    })
}

fn reduce_sum_dense_value(
    tensor: &Tensor,
    inputs: &[TensorId],
    values: &BTreeMap<TensorId, DenseValue>,
    axis: Option<usize>,
) -> Result<DenseValue, ArrayError> {
    let input = inputs
        .first()
        .copied()
        .ok_or(ArrayError::MaterializationRefusal {
            tensor: tensor.id(),
            op: String::from("reduce_sum"),
            detail: String::from("reduce_sum requires one input"),
        })?;
    let value = values.get(&input).ok_or(ArrayError::MissingDependency {
        tensor: tensor.id(),
        input,
    })?;
    let (shape, output) = match axis {
        None => (Shape::scalar(), vec![value.values.iter().sum()]),
        Some(axis) => reduce_sum_axis(
            tensor.id(),
            value.tensor.spec().shape(),
            &value.values,
            axis,
        )?,
    };
    Ok(DenseValue {
        tensor: Tensor::new(
            tensor.id(),
            TensorSpec::new(
                shape,
                value.tensor.spec().dtype(),
                value.tensor.spec().device().clone(),
            ),
            LazyOp::Constant,
        ),
        values: output,
    })
}

fn reduce_sum_axis(
    tensor: TensorId,
    input_shape: &Shape,
    input_values: &[f32],
    axis: usize,
) -> Result<(Shape, Vec<f32>), ArrayError> {
    let Some(output_shape) = input_shape.without_axis(axis) else {
        return Err(ArrayError::MaterializationRefusal {
            tensor,
            op: String::from("reduce_sum"),
            detail: format!("axis {axis} is out of range for shape {input_shape}"),
        });
    };
    let output_count = output_shape.element_count();
    let mut output = vec![0.0; output_count];
    for (index, value) in input_values.iter().enumerate() {
        let mut coordinates = unravel_index(index, input_shape.dims());
        coordinates.remove(axis);
        let output_index = ravel_index(&coordinates, output_shape.dims());
        output[output_index] += *value;
    }
    Ok((output_shape, output))
}

fn expand_values(input_shape: &Shape, input_values: &[f32], target_shape: &Shape) -> Vec<f32> {
    let rank = target_shape.rank();
    let input_rank = input_shape.rank();
    let padding = rank.saturating_sub(input_rank);
    let mut output = Vec::with_capacity(target_shape.element_count());
    for index in 0..target_shape.element_count() {
        let target_indices = unravel_index(index, target_shape.dims());
        let mut input_indices = Vec::with_capacity(input_rank);
        for axis in 0..input_rank {
            let dim = input_shape.dims()[axis];
            let target_index = target_indices[padding + axis];
            input_indices.push(if dim == 1 { 0 } else { target_index });
        }
        let input_index = ravel_index(&input_indices, input_shape.dims());
        output.push(input_values[input_index]);
    }
    output
}

fn unravel_index(mut index: usize, dims: &[usize]) -> Vec<usize> {
    if dims.is_empty() {
        return Vec::new();
    }
    let mut coordinates = vec![0; dims.len()];
    for axis in (0..dims.len()).rev() {
        let dim = dims[axis];
        coordinates[axis] = index % dim;
        index /= dim;
    }
    coordinates
}

fn ravel_index(indices: &[usize], dims: &[usize]) -> usize {
    if dims.is_empty() {
        return 0;
    }
    let mut index = 0;
    let mut stride = 1;
    for axis in (0..dims.len()).rev() {
        index += indices[axis] * stride;
        stride *= dims[axis];
    }
    index
}

#[cfg(test)]
mod tests {
    use super::{
        Array, ArrayBackendCaptureArtifact, ArrayBackendCaptureArtifactRequest,
        ArrayBackendCaptureConfig, ArrayBackendCaptureFormat, ArrayBackendDebugLane,
        ArrayBackendLogKind, ArrayCacheLimitControl, ArrayCacheResetScope, ArrayContext,
        ArrayError, ArrayScalar, AsyncEvalStatus, ImplicitMaterializationPolicy,
        MaterializationTrigger, ReplayBoundary, StreamDependencyPolicy, StreamKind, Tree,
        TreeError, TreeSpec, UnifiedMemoryCapability, current_time_millis,
    };
    use psionic_compiler::{
        CompileTransformConfig, CompileTransformDebugMode, CompileTransformTraceMode,
    };
    use psionic_core::{DType, Device, DeviceKind, QuantizationMode, Shape, TensorSpec};
    use psionic_ir::{
        BackendPluginExtensionContract, CustomKernelExtensionContract, CustomOpExtensionContract,
        ExtensionContractKind, KernelDispatchKind, KernelRegistration, OperatorArity,
        OperatorImplementationKind, OperatorMetaExecutionKind, QuantizerPluginExtensionContract,
        RegisteredOperatorSchema,
    };
    use psionic_runtime::{
        AllocatorPoolPolicy, DeviceDescriptor, ExecutionPlanCachePolicy, GeneratorScope,
        HealthStatus, KernelCachePolicy,
    };
    use std::collections::BTreeMap;

    #[test]
    fn public_lazy_array_surface_builds_graph_backed_arithmetic() -> Result<(), ArrayError> {
        let context = ArrayContext::cpu();
        let left = context.input("left", Shape::new(vec![2, 2]), DType::F32);
        let right = context.constant_f32(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])?;
        let sum = left.add(&right)?;
        let product = sum.mul(&right)?;
        let reduced = product.sum();

        assert_eq!(product.shape(), &Shape::new(vec![2, 2]));
        assert_eq!(reduced.shape(), &Shape::scalar());
        assert_eq!(
            reduced
                .graph()
                .nodes()
                .iter()
                .map(|node| node.op().label())
                .collect::<Vec<_>>(),
            vec!["input", "constant", "add", "mul", "reduce_sum"]
        );
        assert_eq!(reduced.graph().outputs(), &[reduced.tensor_id()]);

        Ok(())
    }

    #[test]
    fn public_lazy_array_surface_supports_matmul_and_detach() -> Result<(), ArrayError> {
        let context = ArrayContext::cpu();
        let left = context.input("left", Shape::new(vec![2, 3]), DType::F32);
        let right = context.input("right", Shape::new(vec![3, 4]), DType::F32);
        let output = left.matmul(&right)?.detach();

        assert_eq!(output.shape(), &Shape::new(vec![2, 4]));
        assert_eq!(output.graph().outputs(), &[output.tensor_id()]);
        assert_eq!(
            output
                .graph()
                .nodes()
                .iter()
                .map(|node| node.op().label())
                .collect::<Vec<_>>(),
            vec!["input", "input", "matmul", "detach"]
        );

        Ok(())
    }

    #[test]
    fn public_lazy_array_surface_refuses_mixed_context_ops() {
        let left_context = ArrayContext::cpu();
        let right_context = ArrayContext::cpu();
        let left = left_context.input("left", Shape::new(vec![2]), DType::F32);
        let right = right_context.input("right", Shape::new(vec![2]), DType::F32);

        let error = left.add(&right).expect_err("mixed contexts should refuse");
        assert_eq!(error, ArrayError::MixedContexts);
    }

    #[test]
    fn public_lazy_array_context_snapshots_multi_output_graphs() -> Result<(), ArrayError> {
        let context = ArrayContext::cpu();
        let input = context.input("input", Shape::new(vec![2, 2]), DType::F32);
        let sum = input.sum();
        let detached = input.detach();
        let graph = context.graph_for(&[detached.clone(), sum.clone()])?;

        assert_eq!(graph.outputs(), &[detached.tensor_id(), sum.tensor_id()]);
        assert_eq!(
            graph
                .nodes()
                .iter()
                .map(|node| node.op().label())
                .collect::<Vec<_>>(),
            vec!["input", "reduce_sum", "detach"]
        );

        Ok(())
    }

    #[test]
    fn public_lazy_array_eval_and_async_eval_stay_explicit() -> Result<(), ArrayError> {
        let context = ArrayContext::cpu();
        let left = context.constant_f32(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])?;
        let right = context.constant_f32(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])?;
        let output = left.add(&right)?.sum();

        let boundary = output.materialization_boundary();
        assert_eq!(
            boundary.explicit_triggers,
            vec![
                MaterializationTrigger::Eval,
                MaterializationTrigger::AsyncEvalWait,
                MaterializationTrigger::DebugCapture,
                MaterializationTrigger::ToHostData,
                MaterializationTrigger::Item,
            ]
        );
        assert_eq!(
            boundary.implicit_policy,
            ImplicitMaterializationPolicy::ExplicitOnly
        );
        assert_eq!(boundary.replay_boundary, ReplayBoundary::GraphSnapshot);

        let evaluated = output.eval()?;
        assert_eq!(evaluated.receipt().trigger, MaterializationTrigger::Eval);
        assert_eq!(evaluated.receipt().stream_id, context.stream().stream_id());
        assert_eq!(
            evaluated.receipt().device_id,
            context.device_handle().stable_id()
        );
        assert_eq!(evaluated.data.as_f32_slice(), Some(&[20.0][..]));

        let pending = output.async_eval()?;
        assert_eq!(pending.status(), AsyncEvalStatus::Pending);
        let mut resolved = pending.wait()?;
        let evaluated_async = resolved.remove(0);
        assert_eq!(
            evaluated_async.receipt().trigger,
            MaterializationTrigger::AsyncEvalWait
        );
        assert_eq!(
            evaluated_async.receipt().stream_id,
            context.stream().stream_id()
        );
        assert_eq!(evaluated_async.data.as_f32_slice(), Some(&[20.0][..]));

        Ok(())
    }

    #[test]
    fn public_lazy_array_eval_refuses_unbound_inputs() {
        let context = ArrayContext::cpu();
        let input = context.input("x", Shape::new(vec![2]), DType::F32);

        let error = input.eval().expect_err("unbound inputs should refuse");
        assert_eq!(
            error,
            ArrayError::UnboundInput {
                tensor: input.tensor_id(),
                name: String::from("x"),
            }
        );
    }

    #[test]
    fn public_lazy_array_runtime_resource_report_tracks_active_peak_and_cache_counters()
    -> Result<(), ArrayError> {
        let context = ArrayContext::cpu();
        let left = context.ones_f32(Shape::new(vec![2, 2]))?;
        let right = context.full_f32(Shape::new(vec![2, 2]), 2.0)?;
        let output = left.add(&right)?;

        let initial = context.runtime_resource_report();
        assert_eq!(initial.memory.active_bytes, 48);
        assert_eq!(initial.memory.peak_bytes, 48);
        assert_eq!(initial.memory.cached_bytes, 0);
        assert_eq!(
            initial
                .backend_resources
                .execution_plan_cache
                .state
                .cached_entries,
            0
        );
        assert_eq!(
            initial
                .backend_resources
                .allocator_pool
                .state
                .cached_buffers,
            0
        );
        assert_eq!(
            initial.backend_resources.kernel_cache.state.cached_entries,
            0
        );

        let _ = output.eval()?;

        let after_eval = context.runtime_resource_report();
        assert_eq!(after_eval.memory.active_bytes, 48);
        assert_eq!(after_eval.memory.peak_bytes, 48);
        assert!(after_eval.memory.cached_bytes > 0);
        assert_eq!(
            after_eval
                .backend_resources
                .execution_plan_cache
                .state
                .cached_entries,
            1
        );
        assert_eq!(
            after_eval
                .backend_resources
                .allocator_pool
                .state
                .cached_buffers,
            1
        );
        assert_eq!(
            after_eval
                .backend_resources
                .kernel_cache
                .state
                .cached_entries,
            1
        );

        let _ = output.eval()?;
        let repeated = context.runtime_resource_report();
        assert_eq!(
            repeated
                .backend_resources
                .execution_plan_cache
                .state
                .cached_entries,
            1
        );
        assert_eq!(
            repeated
                .backend_resources
                .allocator_pool
                .state
                .cached_buffers,
            1
        );
        assert_eq!(
            repeated.backend_resources.kernel_cache.state.cached_entries,
            1
        );

        Ok(())
    }

    #[test]
    fn public_lazy_array_cache_limit_controls_clamp_and_reset_runtime_resources()
    -> Result<(), ArrayError> {
        let context = ArrayContext::cpu();
        let _ = context.configure_cache_limits(ArrayCacheLimitControl {
            execution_plan_cache: ExecutionPlanCachePolicy::bounded(1, Some(512)),
            allocator_pool: AllocatorPoolPolicy::exact_tensor_spec(1, 64),
            kernel_cache: KernelCachePolicy::bounded(1, Some(64)),
        });

        let add_left = context.ones_f32(Shape::new(vec![2, 2]))?;
        let add_right = context.full_f32(Shape::new(vec![2, 2]), 3.0)?;
        let add_graph = add_left.add(&add_right)?;
        let mat_left = context.ones_f32(Shape::new(vec![2, 3]))?;
        let mat_right = context.ones_f32(Shape::new(vec![3, 2]))?;
        let mat_graph = mat_left.matmul(&mat_right)?;

        let _ = add_graph.eval()?;
        let _ = mat_graph.eval()?;

        let clamped = context.runtime_resource_report();
        assert_eq!(
            clamped
                .backend_resources
                .execution_plan_cache
                .policy
                .max_cached_entries,
            1
        );
        assert_eq!(
            clamped
                .backend_resources
                .execution_plan_cache
                .state
                .cached_entries,
            1
        );
        assert_eq!(
            clamped
                .backend_resources
                .allocator_pool
                .state
                .cached_buffers,
            1
        );
        assert_eq!(
            clamped.backend_resources.kernel_cache.state.cached_entries,
            1
        );

        let receipt =
            context.reset_runtime_caches(&[ArrayCacheResetScope::BackendRuntimeResources]);
        assert!(receipt.reclaimed_cache_bytes > 0);
        assert_eq!(
            receipt.before.memory.cached_bytes,
            clamped.memory.cached_bytes
        );
        assert_eq!(receipt.after.memory.cached_bytes, 0);
        assert_eq!(
            receipt
                .reset_scopes
                .iter()
                .map(|scope| scope.isolation_reset_scope())
                .collect::<Vec<_>>(),
            vec![psionic_runtime::IsolationResetScope::BackendRuntimeResources]
        );

        Ok(())
    }

    #[test]
    fn public_lazy_array_device_handles_preserve_unified_memory_truth() {
        let metal = ArrayContext::from_device_descriptor(DeviceDescriptor {
            backend: String::from("metal"),
            device: Device::new(DeviceKind::Metal, 0, Some(String::from("metal:0"))),
            device_name: Some(String::from("Apple GPU")),
            supported_dtypes: vec![DType::F32, DType::F16],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(24 * 1024 * 1024 * 1024),
            unified_memory: Some(true),
            feature_flags: vec![String::from("unified_memory")],
            amd_metadata: None,
            nvidia_metadata: None,
        });
        assert_eq!(
            metal.device_handle().unified_memory_capability(),
            UnifiedMemoryCapability::SharedHostDevice
        );
        assert!(metal.device_handle().supports_unified_memory());
        assert_eq!(metal.stream().kind(), StreamKind::Default);
        assert_eq!(metal.stream().stream_id(), 0);

        let cuda = ArrayContext::from_device_descriptor(DeviceDescriptor {
            backend: String::from("cuda"),
            device: Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0"))),
            device_name: Some(String::from("CUDA GPU")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(16 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("cuda_architecture_surface")],
            amd_metadata: None,
            nvidia_metadata: None,
        });
        assert_eq!(
            cuda.device_handle().unified_memory_capability(),
            UnifiedMemoryCapability::DedicatedDevice
        );
        assert!(!cuda.device_handle().supports_unified_memory());
    }

    #[test]
    fn public_lazy_array_streams_report_dependency_policy_honestly() -> Result<(), ArrayError> {
        let context = ArrayContext::cpu();
        let default_stream = context.stream().clone();
        let explicit_stream = context.new_stream();
        let same_device_context = context.with_stream(explicit_stream.clone())?;
        let array_a = context.constant_f32(Shape::new(vec![1]), vec![1.0])?;
        let array_b = same_device_context.constant_f32(Shape::new(vec![1]), vec![2.0])?;

        assert_eq!(
            array_b.dependency_policy_after(&array_a),
            StreamDependencyPolicy::ExplicitFenceRequired
        );
        assert_eq!(
            array_a.dependency_policy_after(&array_a),
            StreamDependencyPolicy::InOrderSameStream
        );
        assert_eq!(explicit_stream.kind(), StreamKind::Explicit);
        assert_ne!(explicit_stream.stream_id(), default_stream.stream_id());

        let cuda_context = ArrayContext::from_device_descriptor(DeviceDescriptor {
            backend: String::from("cuda"),
            device: Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0"))),
            device_name: Some(String::from("CUDA GPU")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(16 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("cuda_architecture_surface")],
            amd_metadata: None,
            nvidia_metadata: None,
        });
        let array_c = cuda_context.constant_f32(Shape::new(vec![1]), vec![3.0])?;
        assert_eq!(
            array_c.dependency_policy_after(&array_a),
            StreamDependencyPolicy::CrossDeviceTransferRequired
        );

        Ok(())
    }

    #[test]
    fn public_lazy_array_creation_and_view_families_materialize() -> Result<(), ArrayError> {
        let context = ArrayContext::cpu();
        let scalar = context.scalar_f32(2.0)?;
        let scalar_broadcast = scalar.broadcast_to(Shape::new(vec![2, 1]))?;
        let zeros = context.zeros_f32(Shape::new(vec![2, 2]))?;
        let ones = context.ones_f32(Shape::new(vec![2, 2]))?;
        let full = context.full_f32(Shape::new(vec![2, 2]), 3.0)?;
        let base = Array::concat(&[zeros, ones, full], 0)?;
        let sliced = base.slice(0, 1, 5)?;
        let reshaped = sliced.reshape(Shape::new(vec![2, 2, 2]))?;
        let permuted = reshaped.permute(vec![1, 0, 2])?;
        let selected = permuted.select(1, 1)?;
        let transposed = selected.transpose()?;
        let broadcast = transposed.broadcast_to(Shape::new(vec![2, 2, 2]))?;
        let evaluated = broadcast.eval()?;
        let scalar_evaluated = scalar_broadcast.eval()?;

        assert_eq!(base.shape(), &Shape::new(vec![6, 2]));
        assert_eq!(scalar.shape(), &Shape::scalar());
        assert_eq!(scalar_broadcast.shape(), &Shape::new(vec![2, 1]));
        assert_eq!(sliced.shape(), &Shape::new(vec![4, 2]));
        assert_eq!(reshaped.shape(), &Shape::new(vec![2, 2, 2]));
        assert_eq!(selected.shape(), &Shape::new(vec![2, 2]));
        assert_eq!(transposed.shape(), &Shape::new(vec![2, 2]));
        assert_eq!(broadcast.shape(), &Shape::new(vec![2, 2, 2]));
        assert_eq!(scalar_evaluated.data.as_f32_slice(), Some(&[2.0, 2.0][..]));
        assert_eq!(
            evaluated.data.as_f32_slice(),
            Some(&[1.0, 3.0, 1.0, 3.0, 1.0, 3.0, 1.0, 3.0][..])
        );

        Ok(())
    }

    #[test]
    fn public_lazy_array_concat_requires_at_least_one_input() {
        let inputs: &[Array] = &[];
        let error = Array::concat(inputs, 0).expect_err("concat should refuse empty inputs");
        assert_eq!(error, ArrayError::EmptyArrayList);
    }

    #[test]
    fn public_lazy_array_random_cast_and_common_creation_families_stay_seeded()
    -> Result<(), ArrayError> {
        let left = ArrayContext::cpu_seeded(7)?;
        let right = ArrayContext::cpu_seeded(7)?;
        let initial_generator = left
            .random_generator_state()
            .expect("seeded context should expose generator state");
        assert_eq!(
            initial_generator.scope,
            GeneratorScope::LocalDevice {
                stable_device_id: left.device_handle().stable_id().to_string(),
            }
        );
        assert_eq!(initial_generator.draws, 0);

        let uniform_left = left.random_uniform_f32(Shape::new(vec![2, 2]), -1.0, 1.0)?;
        let normal_left = left.random_normal_f32(Shape::new(vec![2, 2]), 0.0, 1.0)?;
        let uniform_right = right.random_uniform_f32(Shape::new(vec![2, 2]), -1.0, 1.0)?;
        let normal_right = right.random_normal_f32(Shape::new(vec![2, 2]), 0.0, 1.0)?;
        let cast_left = uniform_left.cast(DType::I8)?;
        let arange = left.arange_f32(0.0, 5.0, 1.5)?;
        let linspace = left.linspace_f32(-1.0, 1.0, 5)?;
        let eye = left.eye_f32(3, 4)?;

        let uniform_left_eval = uniform_left.eval()?;
        let uniform_right_eval = uniform_right.eval()?;
        let normal_left_eval = normal_left.eval()?;
        let normal_right_eval = normal_right.eval()?;
        let cast_left_eval = cast_left.eval()?;
        let arange_eval = arange.eval()?;
        let linspace_eval = linspace.eval()?;
        let eye_eval = eye.eval()?;

        assert_eq!(
            uniform_left_eval.data.as_f32_slice(),
            uniform_right_eval.data.as_f32_slice()
        );
        assert_eq!(
            normal_left_eval.data.as_f32_slice(),
            normal_right_eval.data.as_f32_slice()
        );
        assert_eq!(cast_left.dtype(), DType::I8);
        assert_eq!(cast_left_eval.dtype(), DType::I8);
        let expected_cast = uniform_left_eval
            .data
            .as_f32_slice()
            .expect("uniform eval should be dense")
            .iter()
            .map(|value| value.round().clamp(i8::MIN as f32, i8::MAX as f32))
            .collect::<Vec<_>>();
        assert_eq!(
            cast_left_eval.data.as_f32_slice(),
            Some(expected_cast.as_slice())
        );
        assert_eq!(arange.shape(), &Shape::new(vec![4]));
        assert_eq!(
            arange_eval.data.as_f32_slice(),
            Some(&[0.0, 1.5, 3.0, 4.5][..])
        );
        assert_eq!(linspace.shape(), &Shape::new(vec![5]));
        assert_eq!(
            linspace_eval.data.as_f32_slice(),
            Some(&[-1.0, -0.5, 0.0, 0.5, 1.0][..])
        );
        assert_eq!(eye.shape(), &Shape::new(vec![3, 4]));
        assert_eq!(
            eye_eval.data.as_f32_slice(),
            Some(
                &[
                    1.0, 0.0, 0.0, 0.0, //
                    0.0, 1.0, 0.0, 0.0, //
                    0.0, 0.0, 1.0, 0.0,
                ][..]
            )
        );
        assert_eq!(
            left.random_generator_state()
                .expect("seeded context should track draws")
                .draws,
            8
        );

        Ok(())
    }

    #[test]
    fn public_lazy_array_random_and_creation_families_refuse_invalid_parameters() {
        let context = ArrayContext::cpu_seeded(11).expect("seeded context");

        let uniform = context.random_uniform_f32(Shape::new(vec![1]), 2.0, 2.0);
        assert_eq!(
            uniform.expect_err("equal bounds should refuse"),
            ArrayError::InvalidRandomUniformBounds {
                low: 2.0,
                high: 2.0,
            }
        );

        let normal = context.random_normal_f32(Shape::new(vec![1]), 0.0, 0.0);
        assert_eq!(
            normal.expect_err("zero stddev should refuse"),
            ArrayError::InvalidRandomNormalStddev { stddev: 0.0 }
        );

        let arange = context.arange_f32(0.0, 1.0, 0.0);
        assert_eq!(
            arange.expect_err("zero step should refuse"),
            ArrayError::InvalidArangeStep {
                start: 0.0,
                stop: 1.0,
                step: 0.0,
            }
        );

        let linspace = context.linspace_f32(0.0, 1.0, 0);
        assert_eq!(
            linspace.expect_err("zero-count linspace should refuse"),
            ArrayError::InvalidLinspaceCount
        );
    }

    #[test]
    fn public_lazy_array_host_interop_and_item_access_stay_explicit() -> Result<(), ArrayError> {
        let context = ArrayContext::cpu();
        let scalar = context.scalar_f32(3.5)?;
        let singleton = context.ones_f32(Shape::new(vec![1]))?;
        let vector = context.arange_f32(0.0, 3.0, 1.0)?;
        let integer = context.scalar_f32(3.6)?.cast(DType::I8)?;

        assert_eq!(
            scalar.item()?,
            ArrayScalar::new(DType::F32, super::HostScalarValue::F32(3.5))
        );
        assert_eq!(
            singleton.item()?,
            ArrayScalar::new(DType::F32, super::HostScalarValue::F32(1.0))
        );
        assert_eq!(
            integer.item()?,
            ArrayScalar::new(DType::I8, super::HostScalarValue::I8(4))
        );

        let vector_host = vector.to_host_data()?;
        assert_eq!(vector_host.dtype(), DType::F32);
        assert_eq!(vector_host.as_f32_slice(), Some(&[0.0, 1.0, 2.0][..]));
        assert_eq!(vector_host.as_i8_slice(), None);

        let integer_host = integer.eval()?.to_host_data()?;
        assert_eq!(integer_host.dtype(), DType::I8);
        assert_eq!(integer_host.as_i8_slice(), Some(&[4][..]));
        assert_eq!(integer_host.as_f32_slice(), None);

        let error = vector
            .item()
            .expect_err("multi-element arrays should refuse item");
        assert_eq!(
            error,
            ArrayError::NonSingletonItem {
                tensor: vector.tensor_id(),
                shape: Shape::new(vec![3]),
            }
        );

        assert_eq!(
            scalar.materialization_boundary().explicit_triggers,
            vec![
                MaterializationTrigger::Eval,
                MaterializationTrigger::AsyncEvalWait,
                MaterializationTrigger::DebugCapture,
                MaterializationTrigger::ToHostData,
                MaterializationTrigger::Item,
            ]
        );

        Ok(())
    }

    #[test]
    fn public_lazy_array_backend_debug_support_and_snapshot_track_seeded_lanes() {
        let cpu = ArrayContext::cpu();
        let cpu_support = cpu.backend_debug_support();
        assert_eq!(cpu_support.lane, ArrayBackendDebugLane::Cpu);
        assert_eq!(
            cpu_support.capture_formats,
            vec![ArrayBackendCaptureFormat::PsionicDebugJson]
        );
        assert!(!cpu_support.supports_vendor_native_capture);
        let cpu_snapshot = cpu.backend_debug_snapshot();
        assert_eq!(cpu_snapshot.backend.effective_backend, "cpu");
        assert_eq!(
            cpu_snapshot.runtime_observability.backend_health[0].status,
            HealthStatus::Ready
        );

        let metal = ArrayContext::from_device_descriptor(DeviceDescriptor {
            backend: String::from("metal"),
            device: Device::new(DeviceKind::Metal, 0, Some(String::from("metal:0"))),
            device_name: Some(String::from("Apple GPU")),
            supported_dtypes: vec![DType::F32, DType::F16],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(24 * 1024 * 1024 * 1024),
            unified_memory: Some(true),
            feature_flags: vec![String::from("unified_memory")],
            amd_metadata: None,
            nvidia_metadata: None,
        });
        let metal_support = metal.backend_debug_support();
        assert_eq!(metal_support.lane, ArrayBackendDebugLane::Metal);
        assert_eq!(
            metal_support.capture_formats,
            vec![
                ArrayBackendCaptureFormat::MetalDebugJson,
                ArrayBackendCaptureFormat::PsionicDebugJson,
            ]
        );
        let metal_snapshot = metal.backend_debug_snapshot();
        assert_eq!(metal_snapshot.backend.effective_backend, "metal");
        assert_eq!(
            metal_snapshot.runtime_observability.backend_health[0].status,
            HealthStatus::Degraded
        );

        let cuda = ArrayContext::from_device_descriptor(DeviceDescriptor {
            backend: String::from("cuda"),
            device: Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0"))),
            device_name: Some(String::from("CUDA GPU")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(16 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("cuda_architecture_surface")],
            amd_metadata: None,
            nvidia_metadata: None,
        });
        let cuda_support = cuda.backend_debug_support();
        assert_eq!(cuda_support.lane, ArrayBackendDebugLane::Cuda);
        assert_eq!(
            cuda_support.capture_formats,
            vec![
                ArrayBackendCaptureFormat::CudaDebugJson,
                ArrayBackendCaptureFormat::PsionicDebugJson,
            ]
        );
        let cuda_snapshot = cuda.backend_debug_snapshot();
        assert_eq!(cuda_snapshot.backend.effective_backend, "cuda");
        assert_eq!(
            cuda_snapshot.runtime_observability.backend_health[0].status,
            HealthStatus::Degraded
        );
    }

    #[test]
    fn public_lazy_array_backend_debug_capture_emits_receipt_logs_and_artifact()
    -> Result<(), ArrayError> {
        let context = ArrayContext::cpu();
        let left = context.constant_f32(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])?;
        let right = context.constant_f32(Shape::new(vec![2, 2]), vec![4.0, 3.0, 2.0, 1.0])?;
        let output = left.add(&right)?.sum();
        let artifact_path = std::env::temp_dir().join(format!(
            "psionic-array-debug-capture-{}-{}.json",
            std::process::id(),
            current_time_millis()
        ));

        let receipt = context.capture_backend_debug(
            std::slice::from_ref(&output),
            ArrayBackendCaptureConfig {
                compile: CompileTransformConfig {
                    trace_mode: CompileTransformTraceMode::FullArtifacts,
                    debug_mode: CompileTransformDebugMode::PlanDebug,
                    ..CompileTransformConfig::default()
                },
                artifact: Some(ArrayBackendCaptureArtifactRequest {
                    path: artifact_path.clone(),
                    format: None,
                }),
                label: Some(String::from("unit_test_capture")),
                ..ArrayBackendCaptureConfig::default()
            },
        )?;

        assert!(receipt.capture_id.starts_with("cpu-capture-"));
        assert_eq!(receipt.graph_digest, output.graph().stable_digest());
        assert_eq!(
            receipt.compile.trace.mode,
            CompileTransformTraceMode::FullArtifacts
        );
        assert!(receipt.compile.plan_debug.is_some());
        assert_eq!(
            receipt.eval_receipts[0].trigger,
            MaterializationTrigger::DebugCapture
        );
        assert_eq!(
            receipt.artifact,
            Some(ArrayBackendCaptureArtifact {
                path: artifact_path.clone(),
                format: ArrayBackendCaptureFormat::PsionicDebugJson,
                bytes: receipt
                    .artifact
                    .as_ref()
                    .map_or(0, |artifact| artifact.bytes),
            })
        );
        let snapshot = context.backend_debug_snapshot();
        assert_eq!(snapshot.recent_captures.len(), 1);
        assert_eq!(snapshot.recent_captures[0].capture_id, receipt.capture_id);
        assert_eq!(
            snapshot.recent_logs.last().map(|event| event.kind),
            Some(ArrayBackendLogKind::Capture)
        );
        let artifact_json = std::fs::read_to_string(&artifact_path)
            .expect("debug capture artifact should be readable");
        assert!(artifact_json.contains(&receipt.capture_id));
        assert!(artifact_json.contains("unit_test_capture"));
        std::fs::remove_file(&artifact_path)
            .expect("debug capture artifact cleanup should succeed");

        let error = context
            .capture_backend_debug(
                std::slice::from_ref(&output),
                ArrayBackendCaptureConfig {
                    artifact: Some(ArrayBackendCaptureArtifactRequest {
                        path: artifact_path,
                        format: Some(ArrayBackendCaptureFormat::CudaDebugJson),
                    }),
                    ..ArrayBackendCaptureConfig::default()
                },
            )
            .expect_err("cpu context should refuse cuda-only capture format");
        assert_eq!(
            error,
            ArrayError::UnsupportedBackendDebugFormat {
                backend: String::from("cpu"),
                format: ArrayBackendCaptureFormat::CudaDebugJson,
            }
        );

        Ok(())
    }

    #[test]
    fn public_lazy_array_extension_authoring_surface_registers_custom_ops_and_kernels()
    -> Result<(), ArrayError> {
        let context = ArrayContext::cpu();
        let schema = RegisteredOperatorSchema::custom(
            "x.example.masked_scale",
            1,
            OperatorArity::Fixed(2),
            OperatorImplementationKind::BackendKernel,
            OperatorMetaExecutionKind::DeclaredOutput,
        );

        let op_receipt = context.register_custom_op_extension(CustomOpExtensionContract {
            schema: schema.clone(),
            declared_output_required: true,
        })?;
        assert_eq!(op_receipt.kind, ExtensionContractKind::CustomOp);
        assert_eq!(op_receipt.subject, "x.example.masked_scale");
        assert!(op_receipt.dispatch_contracts.is_empty());

        let declared_output = TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu());
        let validated = context.validate_declared_custom_output(
            "x.example.masked_scale",
            2,
            Some(&declared_output),
        )?;
        assert_eq!(validated, declared_output);

        let kernel_receipt =
            context.register_custom_kernel_extension(CustomKernelExtensionContract {
                schema: schema.clone(),
                registration: KernelRegistration::backend_specific(
                    "x.example.masked_scale",
                    "cpu",
                    "masked_scale_cpu",
                ),
            })?;
        assert_eq!(kernel_receipt.kind, ExtensionContractKind::CustomKernel);
        assert_eq!(
            kernel_receipt.dispatch_contracts[0].dispatch_kind,
            KernelDispatchKind::BackendSpecific
        );
        assert_eq!(
            kernel_receipt.dispatch_contracts[0]
                .resolved_backend
                .as_deref(),
            Some("cpu")
        );
        assert_eq!(
            kernel_receipt.dispatch_contracts[0]
                .kernel_symbol
                .as_deref(),
            Some("masked_scale_cpu")
        );

        let resolved = context.resolve_extension_dispatch("x.example.masked_scale")?;
        assert_eq!(resolved.dispatch_kind, KernelDispatchKind::BackendSpecific);
        let snapshot = context.extension_registry_snapshot();
        assert!(
            snapshot
                .schemas
                .iter()
                .any(|registered| registered.name == "x.example.masked_scale")
        );
        assert!(snapshot.kernel_registrations.iter().any(|registration| {
            registration.name == "x.example.masked_scale"
                && registration.kernel_symbol == "masked_scale_cpu"
        }));
        assert_eq!(
            snapshot.semantics.current_scope_window,
            "psionic_extension_contracts_v1"
        );

        Ok(())
    }

    #[test]
    fn public_lazy_array_extension_authoring_surface_registers_plugins_and_refuses_duplicates()
    -> Result<(), ArrayError> {
        let context = ArrayContext::from_device_descriptor(DeviceDescriptor {
            backend: String::from("cuda"),
            device: Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0"))),
            device_name: Some(String::from("CUDA GPU")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(16 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("cuda_architecture_surface")],
            amd_metadata: None,
            nvidia_metadata: None,
        });
        let plugin_schema = RegisteredOperatorSchema::custom(
            "x.example.flash_mask",
            1,
            OperatorArity::Fixed(3),
            OperatorImplementationKind::BackendKernel,
            OperatorMetaExecutionKind::DeclaredOutput,
        );
        let plugin_receipt =
            context.register_backend_plugin_extension(BackendPluginExtensionContract {
                plugin_id: String::from("cuda.flash"),
                backend_label: String::from("cuda"),
                custom_schemas: vec![plugin_schema.clone()],
                kernel_registrations: vec![KernelRegistration::backend_specific(
                    "x.example.flash_mask",
                    "cuda",
                    "flash_mask_cuda",
                )],
            })?;
        assert_eq!(plugin_receipt.kind, ExtensionContractKind::BackendPlugin);
        assert_eq!(plugin_receipt.backend, "cuda");
        assert_eq!(
            plugin_receipt.dispatch_contracts[0]
                .resolved_backend
                .as_deref(),
            Some("cuda")
        );

        let quantizer_receipt =
            context.register_quantizer_plugin_extension(QuantizerPluginExtensionContract {
                plugin_id: String::from("cuda.int8"),
                supported_weight_modes: vec![QuantizationMode::Int8Symmetric],
                export_aware: true,
                requires_observer_contracts: false,
            })?;
        assert_eq!(
            quantizer_receipt.kind,
            ExtensionContractKind::QuantizerPlugin
        );
        let snapshot = context.extension_registry_snapshot();
        assert_eq!(snapshot.quantizer_plugins.len(), 1);
        assert_eq!(snapshot.quantizer_plugins[0].plugin_id, "cuda.int8");

        let duplicate = context
            .register_quantizer_plugin_extension(QuantizerPluginExtensionContract {
                plugin_id: String::from("cuda.int8"),
                supported_weight_modes: vec![QuantizationMode::Int8Symmetric],
                export_aware: true,
                requires_observer_contracts: false,
            })
            .expect_err("duplicate quantizer plugins should refuse");
        assert_eq!(
            duplicate,
            ArrayError::DuplicateQuantizerPlugin {
                plugin_id: String::from("cuda.int8"),
            }
        );

        Ok(())
    }

    #[test]
    fn public_lazy_array_tree_utilities_preserve_structure_and_refuse_bad_unflatten()
    -> Result<(), ArrayError> {
        let context = ArrayContext::cpu();
        let left = context.scalar_f32(1.0)?;
        let right = context.scalar_f32(2.0)?.cast(DType::I8)?;
        let tree = Tree::Dict(BTreeMap::from([
            (
                String::from("pair"),
                Tree::Tuple(vec![Tree::Leaf(left.clone()), Tree::Leaf(right.clone())]),
            ),
            (
                String::from("list"),
                Tree::List(vec![Tree::Leaf(context.ones_f32(Shape::new(vec![1]))?)]),
            ),
        ]));

        let flattened = tree.clone().flatten();
        assert_eq!(flattened.spec.leaf_count(), 3);
        assert_eq!(flattened.leaves.len(), 3);
        let rebuilt = flattened
            .clone()
            .unflatten()
            .expect("tree should round-trip");
        assert_eq!(rebuilt.spec(), tree.spec());

        let evaluated = tree.eval()?;
        let host_tree = evaluated.to_host_data()?;
        let scalar_tree = tree.item()?;

        match host_tree {
            Tree::Dict(values) => {
                assert_eq!(values.len(), 2);
                let pair = values.get("pair").expect("pair entry");
                match pair {
                    Tree::Tuple(values) => {
                        assert_eq!(values.len(), 2);
                        let first = match &values[0] {
                            Tree::Leaf(value) => value,
                            _ => panic!("first pair leaf should be a leaf"),
                        };
                        assert_eq!(first.dtype(), DType::F32);
                        assert_eq!(first.as_f32_slice(), Some(&[1.0][..]));
                        let second = match &values[1] {
                            Tree::Leaf(value) => value,
                            _ => panic!("second pair leaf should be a leaf"),
                        };
                        assert_eq!(second.dtype(), DType::I8);
                        assert_eq!(second.as_i8_slice(), Some(&[2][..]));
                    }
                    _ => panic!("pair entry should stay a tuple"),
                }
            }
            _ => panic!("host tree should stay a dict"),
        }

        match scalar_tree {
            Tree::Dict(values) => {
                let pair = values.get("pair").expect("pair entry");
                match pair {
                    Tree::Tuple(values) => {
                        let first = match &values[0] {
                            Tree::Leaf(value) => value,
                            _ => panic!("first scalar leaf should be a leaf"),
                        };
                        assert_eq!(first.as_f32(), Some(1.0));
                        let second = match &values[1] {
                            Tree::Leaf(value) => value,
                            _ => panic!("second scalar leaf should be a leaf"),
                        };
                        assert_eq!(second.as_i8(), Some(2));
                    }
                    _ => panic!("pair entry should stay a tuple"),
                }
            }
            _ => panic!("scalar tree should stay a dict"),
        }

        let tree_error = TreeSpec::Tuple(vec![TreeSpec::Leaf, TreeSpec::Leaf])
            .unflatten(vec![left.clone()])
            .expect_err("short leaf list should refuse");
        assert_eq!(
            tree_error,
            TreeError::LeafCountMismatch {
                expected: 2,
                actual: 1,
            }
        );

        Ok(())
    }
}
