//! Runtime traits and execution surfaces for Mox.

mod parity;

use std::collections::BTreeMap;

use mox_core::{DType, Device, QuantizationMode, QuantizedBlockLayout, TensorId, TensorSpec};
use mox_ir::ExecutionPlan;
pub use parity::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "runtime traits for devices and execution";

/// Stable runtime backend name.
pub type BackendName = &'static str;

/// Runtime failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeError {
    /// The requested tensor input was not supplied.
    #[error("missing input tensor {0}")]
    MissingInput(TensorId),
    /// A buffer shape or dtype was not what execution expected.
    #[error("invalid buffer for tensor {tensor}: expected {expected:?}, actual {actual:?}")]
    InvalidBuffer {
        /// Tensor ID that failed validation.
        tensor: TensorId,
        /// Expected tensor specification.
        expected: TensorSpec,
        /// Actual tensor specification.
        actual: TensorSpec,
    },
    /// The execution plan referenced a node that the backend cannot execute.
    #[error("unsupported execution step `{0}`")]
    UnsupportedStep(String),
    /// Generic backend failure.
    #[error("{0}")]
    Backend(String),
}

/// Runtime-visible device description.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceDescriptor {
    /// Backend family name.
    pub backend: String,
    /// Logical device.
    pub device: Device,
    /// Human-readable device name when the backend can supply one.
    pub device_name: Option<String>,
    /// Supported dtypes for the device.
    pub supported_dtypes: Vec<DType>,
    /// Supported quantization modes for model-backed execution.
    pub supported_quantization: Vec<QuantizationSupport>,
    /// Optional memory capacity in bytes.
    pub memory_capacity_bytes: Option<u64>,
    /// Whether the device shares memory with the host, when known.
    pub unified_memory: Option<bool>,
    /// Stable feature flags relevant to runtime/backend selection.
    pub feature_flags: Vec<String>,
    /// AMD-specific topology/risk metadata when the device belongs to an AMD backend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amd_metadata: Option<AmdDeviceMetadata>,
}

/// Distinct AMD runtime mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmdRuntimeMode {
    /// Kernel-mediated AMD KFD posture using the standard `amdgpu` driver stack.
    Kfd,
    /// Explicitly opted-in userspace/AM-driver posture.
    Userspace,
}

/// Whether an AMD mode requires or has satisfied explicit opt-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmdOptInStatus {
    /// The backend does not require an explicit opt-in gate.
    NotRequired,
    /// The backend is present but currently disabled until the operator opts in.
    Disabled,
    /// The operator has explicitly enabled the backend.
    Enabled,
}

/// Risk posture for an AMD backend mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmdRiskLevel {
    /// Lower-risk operational posture.
    Standard,
    /// Higher-risk posture that needs stronger operator intent.
    Elevated,
}

/// Driver ownership/binding state relevant to AMD recovery posture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmdDriverBinding {
    /// The kernel `amdgpu` driver still owns the device.
    KernelAmdgpu,
    /// A userspace stack has taken ownership of the device.
    UserspaceClaimed,
    /// Mox could not determine the binding state.
    Unknown,
}

/// Expected operator-level recovery step for an AMD backend mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmdRecoveryAction {
    /// Restart the affected process/runtime first.
    ProcessRestart,
    /// Attempt a kernel-driver reset or recovery path.
    KernelDriverReset,
    /// Rebind or restore the kernel driver after userspace mode.
    RebindKernelDriver,
    /// Reboot the host when the runtime cannot recover in-place.
    RebootHost,
}

/// Stable AMD topology fields relevant to backend discovery and later capability reporting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdTopologyInfo {
    /// Stable architecture label such as `gfx1100`, when known.
    pub architecture: Option<String>,
    /// PCI bus/device/function address, when known.
    pub pci_bdf: Option<String>,
    /// Number of XCC partitions, when known.
    pub xcc_count: Option<u16>,
    /// Number of shader engines, when known.
    pub shader_engine_count: Option<u16>,
    /// Number of compute units, when known.
    pub compute_unit_count: Option<u16>,
    /// Total VRAM bytes, when known.
    pub vram_bytes: Option<u64>,
    /// Host-visible VRAM bytes, when known.
    pub visible_vram_bytes: Option<u64>,
}

/// Stable AMD risk posture derived from the backend/runtime mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdRiskProfile {
    /// High-level risk classification.
    pub level: AmdRiskLevel,
    /// Whether the mode requires explicit operator intent before activation.
    pub requires_explicit_opt_in: bool,
    /// Whether the mode may unbind or otherwise displace the kernel driver.
    pub may_unbind_kernel_driver: bool,
    /// Plain-text warnings the operator should see or preserve in logs.
    pub warnings: Vec<String>,
}

/// Stable AMD recovery posture derived from the backend/runtime mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdRecoveryProfile {
    /// Current or expected driver binding state.
    pub driver_binding: AmdDriverBinding,
    /// Ordered recovery actions Mox expects the operator/runtime to consider.
    pub expected_actions: Vec<AmdRecoveryAction>,
}

/// AMD-specific device metadata carried through runtime and provider truth surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdDeviceMetadata {
    /// Runtime mode that discovered the device.
    pub mode: AmdRuntimeMode,
    /// Stable topology snapshot.
    pub topology: AmdTopologyInfo,
    /// Risk posture for the selected AMD mode.
    pub risk: AmdRiskProfile,
    /// Recovery posture for the selected AMD mode.
    pub recovery: AmdRecoveryProfile,
}

/// Backend-local AMD discovery report that preserves mode and opt-in truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdBackendReport {
    /// AMD backend mode represented by the report.
    pub mode: AmdRuntimeMode,
    /// Opt-in state for the backend mode.
    pub opt_in: AmdOptInStatus,
    /// Discovered devices for the mode.
    pub devices: Vec<DeviceDescriptor>,
    /// Honest readiness/health for the mode.
    pub health: RuntimeHealth,
}

/// How a backend handles a quantization mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationExecution {
    /// Execute the quantized representation directly.
    Native,
    /// Dequantize weights to `f32` before execution.
    DequantizeToF32,
}

/// Explicit load/storage posture for a quantized mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationLoadPath {
    /// Weights arrive as ordinary dense `f32` tensors.
    DenseF32,
    /// The runtime loads quantized weights and immediately dequantizes them to `f32`.
    DequantizedF32,
    /// The runtime preserves quantized blocks in backend-owned storage.
    BackendQuantized,
}

/// Runtime support declaration for a quantization mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizationSupport {
    /// Supported quantization mode.
    pub mode: QuantizationMode,
    /// Explicit load/storage path for the quantized weights.
    pub load_path: QuantizationLoadPath,
    /// How the runtime executes that mode.
    pub execution: QuantizationExecution,
}

/// Runtime health state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Device/runtime is ready for work.
    Ready,
    /// Device/runtime can execute but with caveats.
    Degraded,
    /// Device/runtime cannot execute.
    Offline,
}

/// Health report for a runtime or backend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeHealth {
    /// Current health status.
    pub status: HealthStatus,
    /// Plain-text explanation.
    pub message: String,
}

/// Explicit runtime backend selection and fallback truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendSelection {
    /// Backend the caller or higher-level runtime requested.
    pub requested_backend: String,
    /// Backend that will actually execute the work.
    pub effective_backend: String,
    /// Selected device for the effective backend, when one exists.
    pub selected_device: Option<DeviceDescriptor>,
    /// Supported op labels for the advertised product path.
    pub supported_ops: Vec<String>,
    /// Explicit fallback reason when the effective backend differs from the requested backend.
    pub fallback_reason: Option<String>,
}

impl BackendSelection {
    /// Creates a direct backend selection with no fallback.
    #[must_use]
    pub fn direct(
        backend: impl Into<String>,
        selected_device: Option<DeviceDescriptor>,
        supported_ops: Vec<String>,
    ) -> Self {
        let backend = backend.into();
        Self {
            requested_backend: backend.clone(),
            effective_backend: backend,
            selected_device,
            supported_ops,
            fallback_reason: None,
        }
    }

    /// Creates an explicit fallback selection.
    #[must_use]
    pub fn fallback(
        requested_backend: impl Into<String>,
        effective_backend: impl Into<String>,
        selected_device: Option<DeviceDescriptor>,
        supported_ops: Vec<String>,
        fallback_reason: impl Into<String>,
    ) -> Self {
        Self {
            requested_backend: requested_backend.into(),
            effective_backend: effective_backend.into(),
            selected_device,
            supported_ops,
            fallback_reason: Some(fallback_reason.into()),
        }
    }

    /// Creates a direct selection from a discovered backend.
    pub fn from_backend<B>(backend: &B, supported_ops: &[&str]) -> Result<Self, RuntimeError>
    where
        B: DeviceDiscovery + ?Sized,
    {
        Ok(Self::direct(
            backend.backend_name(),
            backend.discover_devices()?.into_iter().next(),
            supported_ops
                .iter()
                .map(|label| String::from(*label))
                .collect(),
        ))
    }

    /// Creates a fallback selection to an effective backend discovered at runtime.
    pub fn fallback_to_backend<B>(
        requested_backend: impl Into<String>,
        effective_backend: &B,
        supported_ops: &[&str],
        fallback_reason: impl Into<String>,
    ) -> Result<Self, RuntimeError>
    where
        B: DeviceDiscovery + ?Sized,
    {
        Ok(Self::fallback(
            requested_backend,
            effective_backend.backend_name(),
            effective_backend.discover_devices()?.into_iter().next(),
            supported_ops
                .iter()
                .map(|label| String::from(*label))
                .collect(),
            fallback_reason,
        ))
    }
}

/// Minimal execution metrics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    /// Number of plan steps executed.
    pub steps_executed: usize,
}

/// Trait for backend-owned buffers.
pub trait BufferHandle {
    /// Returns the buffer tensor spec.
    fn spec(&self) -> &TensorSpec;

    /// Returns the storage posture for the buffer.
    fn storage_kind(&self) -> BufferStorageKind {
        BufferStorageKind::DenseF32
    }
}

/// Physical residency of a backend-owned buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BufferResidency {
    /// Storage lives in host-managed memory.
    Host,
    /// Storage lives in backend-owned device memory.
    Backend,
}

/// Explicit buffer storage kind surfaced by runtime backends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BufferStorageKind {
    /// Ordinary dense `f32` tensor storage.
    DenseF32,
    /// Dense `f32` storage that came from a quantized source tensor.
    DequantizedF32 {
        /// Source quantization mode that was dequantized.
        source_quantization: QuantizationMode,
    },
    /// Quantized GGML/GGUF block storage that remains quantized.
    QuantizedBlocks {
        /// Quantized storage family.
        mode: QuantizationMode,
        /// Stable GGML block layout.
        layout: QuantizedBlockLayout,
        /// Whether the storage is host- or backend-resident.
        residency: BufferResidency,
    },
}

/// How a runtime load plan sources model artifact bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelArtifactStorageKind {
    /// The artifact was copied into an in-memory buffer before planning.
    InMemoryCopy,
    /// The artifact stays backed by a paged local blob.
    PagedLocalBlob,
}

/// Blob family used by a paged local model artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelArtifactBlobKind {
    /// Standalone GGUF file discovered on disk.
    GgufFile,
    /// Ollama-managed blob resolved by digest.
    OllamaBlob,
}

/// Actual local read path used for a paged model artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactReadPath {
    /// The artifact bytes are exposed through a memory map.
    MemoryMapped,
    /// The artifact bytes are exposed from a buffered host copy.
    Buffered,
}

/// Runtime-visible storage truth for a model artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelArtifactStorage {
    /// Stable artifact name.
    pub artifact_name: String,
    /// Stable SHA-256 digest of the artifact bytes.
    pub artifact_sha256: String,
    /// High-level storage posture used by the runtime.
    pub storage_kind: ModelArtifactStorageKind,
    /// Blob family when the runtime kept paged local blob storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob_kind: Option<ModelArtifactBlobKind>,
    /// Actual local read path when the runtime kept paged local blob storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_path: Option<ArtifactReadPath>,
    /// Logical page size when the runtime kept paged local blob storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<usize>,
    /// Explicit fallback reason when mmap was preferred but not used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
}

impl ModelArtifactStorage {
    /// Creates storage truth for an eager in-memory artifact copy.
    #[must_use]
    pub fn in_memory_copy(
        artifact_name: impl Into<String>,
        artifact_sha256: impl Into<String>,
    ) -> Self {
        Self {
            artifact_name: artifact_name.into(),
            artifact_sha256: artifact_sha256.into(),
            storage_kind: ModelArtifactStorageKind::InMemoryCopy,
            blob_kind: None,
            read_path: None,
            page_size: None,
            fallback_reason: None,
        }
    }

    /// Creates storage truth for a paged local blob artifact.
    #[must_use]
    pub fn paged_local_blob(
        artifact_name: impl Into<String>,
        artifact_sha256: impl Into<String>,
        blob_kind: ModelArtifactBlobKind,
        read_path: ArtifactReadPath,
        page_size: usize,
        fallback_reason: Option<String>,
    ) -> Self {
        Self {
            artifact_name: artifact_name.into(),
            artifact_sha256: artifact_sha256.into(),
            storage_kind: ModelArtifactStorageKind::PagedLocalBlob,
            blob_kind: Some(blob_kind),
            read_path: Some(read_path),
            page_size: Some(page_size),
            fallback_reason,
        }
    }
}

/// Runtime-visible paged tensor byte plan derived from a blob-backed artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PagedTensorStoragePlan {
    /// Stable tensor name.
    pub tensor_name: String,
    /// Backing artifact name.
    pub artifact_name: String,
    /// Byte offset inside the artifact.
    pub byte_offset: u64,
    /// Tensor byte length inside the artifact.
    pub byte_length: u64,
    /// Logical page size for reads over the tensor bytes.
    pub page_size: usize,
    /// Total page count for the tensor byte range.
    pub page_count: usize,
}

/// Trait for device discovery.
pub trait DeviceDiscovery {
    /// Returns the backend name.
    fn backend_name(&self) -> BackendName;

    /// Returns discovered devices.
    fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError>;

    /// Returns current runtime health.
    fn health(&self) -> RuntimeHealth;
}

/// Trait for backend allocators.
pub trait Allocator {
    /// Concrete buffer type.
    type Buffer: BufferHandle;

    /// Allocates a buffer for a tensor spec.
    fn allocate(&mut self, spec: &TensorSpec) -> Result<Self::Buffer, RuntimeError>;
}

/// Trait for graph execution.
pub trait ExecutionBackend {
    /// Concrete buffer type.
    type Buffer: BufferHandle;

    /// Executes a compiled plan with host-supplied inputs.
    fn execute(
        &mut self,
        plan: &ExecutionPlan,
        inputs: &BTreeMap<TensorId, Self::Buffer>,
    ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError>;
}

/// Execution result containing output buffers and basic metrics.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionResult<B> {
    /// Materialized outputs by tensor ID.
    pub outputs: BTreeMap<TensorId, B>,
    /// Runtime metrics for the execution.
    pub metrics: ExecutionMetrics,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mox_core::{DType, Device, Shape, TensorSpec};
    use mox_ir::{ExecutionOp, ExecutionPlan, ExecutionStep};
    use serde_json::json;

    use super::{
        Allocator, AmdBackendReport, AmdDeviceMetadata, AmdDriverBinding, AmdOptInStatus,
        AmdRecoveryAction, AmdRecoveryProfile, AmdRiskLevel, AmdRiskProfile, AmdRuntimeMode,
        AmdTopologyInfo, ArtifactReadPath, BackendSelection, BufferHandle, BufferResidency,
        BufferStorageKind, DeviceDescriptor, DeviceDiscovery, ExecutionBackend, ExecutionMetrics,
        ExecutionResult, HealthStatus, ModelArtifactBlobKind, ModelArtifactStorage,
        ModelArtifactStorageKind, PagedTensorStoragePlan, QuantizationExecution,
        QuantizationLoadPath, QuantizationSupport, RuntimeError, RuntimeHealth,
    };

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct MockBuffer {
        spec: TensorSpec,
    }

    impl BufferHandle for MockBuffer {
        fn spec(&self) -> &TensorSpec {
            &self.spec
        }
    }

    struct MockRuntime;

    impl DeviceDiscovery for MockRuntime {
        fn backend_name(&self) -> super::BackendName {
            "mock"
        }

        fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError> {
            Ok(vec![DeviceDescriptor {
                backend: String::from("mock"),
                device: Device::cpu(),
                device_name: Some(String::from("mock cpu")),
                supported_dtypes: vec![DType::F32],
                supported_quantization: vec![QuantizationSupport {
                    mode: mox_core::QuantizationMode::None,
                    load_path: QuantizationLoadPath::DenseF32,
                    execution: QuantizationExecution::Native,
                }],
                memory_capacity_bytes: None,
                unified_memory: Some(true),
                feature_flags: vec![String::from("mock_execution")],
                amd_metadata: None,
            }])
        }

        fn health(&self) -> RuntimeHealth {
            RuntimeHealth {
                status: HealthStatus::Ready,
                message: String::from("ready"),
            }
        }
    }

    impl Allocator for MockRuntime {
        type Buffer = MockBuffer;

        fn allocate(&mut self, spec: &TensorSpec) -> Result<Self::Buffer, RuntimeError> {
            Ok(MockBuffer { spec: spec.clone() })
        }
    }

    impl ExecutionBackend for MockRuntime {
        type Buffer = MockBuffer;

        fn execute(
            &mut self,
            plan: &ExecutionPlan,
            _inputs: &BTreeMap<mox_core::TensorId, Self::Buffer>,
        ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError> {
            Ok(ExecutionResult {
                outputs: BTreeMap::new(),
                metrics: ExecutionMetrics {
                    steps_executed: plan.steps.len(),
                },
            })
        }
    }

    #[test]
    fn mock_runtime_reports_device_and_executes_plan() -> Result<(), RuntimeError> {
        let mut runtime = MockRuntime;
        let devices = runtime.discover_devices()?;
        if devices.len() != 1 {
            return Err(RuntimeError::Backend(format!(
                "expected 1 discovered device, found {}",
                devices.len()
            )));
        }
        if runtime.health().status != HealthStatus::Ready {
            return Err(RuntimeError::Backend(String::from(
                "expected mock runtime health to be ready",
            )));
        }

        let spec = TensorSpec::new(Shape::new(vec![1, 2]), DType::F32, Device::cpu());
        let buffer = runtime.allocate(&spec)?;
        let mut inputs = BTreeMap::new();
        inputs.insert(mox_core::TensorId(0), buffer);

        let plan = ExecutionPlan {
            graph_digest: String::from("digest"),
            steps: vec![ExecutionStep {
                output: mox_core::TensorId(1),
                op: ExecutionOp::Add,
                spec: TensorSpec::new(Shape::new(vec![1, 2]), DType::F32, Device::cpu()),
                inputs: vec![mox_core::TensorId(0)],
            }],
            outputs: vec![mox_core::TensorId(1)],
        };

        let result = runtime.execute(&plan, &inputs)?;
        if result.metrics.steps_executed != 1 {
            return Err(RuntimeError::Backend(format!(
                "expected 1 executed step, found {}",
                result.metrics.steps_executed
            )));
        }
        Ok(())
    }

    #[test]
    fn backend_selection_helpers_capture_direct_and_fallback_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let direct = BackendSelection::from_backend(&MockRuntime, &["input", "matmul"])?;
        assert_eq!(direct.requested_backend, "mock");
        assert_eq!(direct.effective_backend, "mock");
        assert_eq!(
            direct.supported_ops,
            vec![String::from("input"), String::from("matmul")]
        );
        assert!(direct.fallback_reason.is_none());
        assert_eq!(
            serde_json::to_value(&direct)?,
            json!({
                "requested_backend": "mock",
                "effective_backend": "mock",
                "selected_device": {
                    "backend": "mock",
                    "device": {
                        "kind": "Cpu",
                        "ordinal": 0,
                        "label": "cpu:0"
                    },
                    "device_name": "mock cpu",
                    "supported_dtypes": ["F32"],
                    "supported_quantization": [{
                        "mode": "none",
                        "load_path": "dense_f32",
                        "execution": "native"
                    }],
                    "memory_capacity_bytes": null,
                    "unified_memory": true,
                    "feature_flags": ["mock_execution"]
                },
                "supported_ops": ["input", "matmul"],
                "fallback_reason": null
            })
        );

        let fallback = BackendSelection::fallback_to_backend(
            "metal",
            &MockRuntime,
            &["input", "matmul"],
            "metal backend unavailable: offline",
        )?;
        assert_eq!(fallback.requested_backend, "metal");
        assert_eq!(fallback.effective_backend, "mock");
        assert_eq!(
            fallback.fallback_reason.as_deref(),
            Some("metal backend unavailable: offline")
        );
        Ok(())
    }

    #[test]
    fn quantization_support_surfaces_storage_path_and_pending_execution_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let support = QuantizationSupport {
            mode: mox_core::QuantizationMode::GgmlQ4_0,
            load_path: QuantizationLoadPath::BackendQuantized,
            execution: QuantizationExecution::DequantizeToF32,
        };

        assert_eq!(
            serde_json::to_value(&support)?,
            json!({
                "mode": "ggml_q4_0",
                "load_path": "backend_quantized",
                "execution": "dequantize_to_f32"
            })
        );
        Ok(())
    }

    #[test]
    fn buffer_handles_can_distinguish_quantized_storage_from_dequantized_fallback() {
        #[derive(Clone, Debug, PartialEq, Eq)]
        struct QuantizedMockBuffer {
            spec: TensorSpec,
        }

        impl BufferHandle for QuantizedMockBuffer {
            fn spec(&self) -> &TensorSpec {
                &self.spec
            }

            fn storage_kind(&self) -> BufferStorageKind {
                BufferStorageKind::QuantizedBlocks {
                    mode: mox_core::QuantizationMode::GgmlQ8_0,
                    layout: mox_core::QuantizedBlockLayout::new(32, 34, 2),
                    residency: BufferResidency::Backend,
                }
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        struct DequantizedMockBuffer {
            spec: TensorSpec,
        }

        impl BufferHandle for DequantizedMockBuffer {
            fn spec(&self) -> &TensorSpec {
                &self.spec
            }

            fn storage_kind(&self) -> BufferStorageKind {
                BufferStorageKind::DequantizedF32 {
                    source_quantization: mox_core::QuantizationMode::GgmlQ8_0,
                }
            }
        }

        let spec = TensorSpec::new(Shape::new(vec![64]), DType::F32, Device::cpu());
        let quantized = QuantizedMockBuffer { spec: spec.clone() };
        let dequantized = DequantizedMockBuffer { spec };

        assert_eq!(
            quantized.storage_kind(),
            BufferStorageKind::QuantizedBlocks {
                mode: mox_core::QuantizationMode::GgmlQ8_0,
                layout: mox_core::QuantizedBlockLayout::new(32, 34, 2),
                residency: BufferResidency::Backend,
            }
        );
        assert_eq!(
            dequantized.storage_kind(),
            BufferStorageKind::DequantizedF32 {
                source_quantization: mox_core::QuantizationMode::GgmlQ8_0,
            }
        );
    }

    #[test]
    fn runtime_model_storage_truth_distinguishes_paged_blobs_from_copies()
    -> Result<(), Box<dyn std::error::Error>> {
        let copy = ModelArtifactStorage::in_memory_copy("weights.gguf", "abcd");
        let paged = ModelArtifactStorage::paged_local_blob(
            "weights.gguf",
            "abcd",
            ModelArtifactBlobKind::OllamaBlob,
            ArtifactReadPath::MemoryMapped,
            4096,
            Some(String::from("mmap preferred and available")),
        );

        assert_eq!(copy.storage_kind, ModelArtifactStorageKind::InMemoryCopy);
        assert_eq!(
            serde_json::to_value(&copy)?,
            json!({
                "artifact_name": "weights.gguf",
                "artifact_sha256": "abcd",
                "storage_kind": "in_memory_copy"
            })
        );
        assert_eq!(paged.storage_kind, ModelArtifactStorageKind::PagedLocalBlob);
        assert_eq!(
            serde_json::to_value(&paged)?,
            json!({
                "artifact_name": "weights.gguf",
                "artifact_sha256": "abcd",
                "storage_kind": "paged_local_blob",
                "blob_kind": "ollama_blob",
                "read_path": "memory_mapped",
                "page_size": 4096,
                "fallback_reason": "mmap preferred and available"
            })
        );
        Ok(())
    }

    #[test]
    fn paged_tensor_storage_plan_serializes_byte_window_and_page_counts()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = PagedTensorStoragePlan {
            tensor_name: String::from("blk.0.attn_q.weight"),
            artifact_name: String::from("weights.gguf"),
            byte_offset: 8192,
            byte_length: 16384,
            page_size: 4096,
            page_count: 4,
        };

        assert_eq!(
            serde_json::to_value(&plan)?,
            json!({
                "tensor_name": "blk.0.attn_q.weight",
                "artifact_name": "weights.gguf",
                "byte_offset": 8192,
                "byte_length": 16384,
                "page_size": 4096,
                "page_count": 4
            })
        );
        Ok(())
    }

    #[test]
    fn amd_backend_model_serializes_mode_topology_risk_and_recovery()
    -> Result<(), Box<dyn std::error::Error>> {
        let device = DeviceDescriptor {
            backend: String::from("amd_userspace"),
            device: Device::new(
                mox_core::DeviceKind::AmdUserspace,
                0,
                Some(String::from("amd_userspace:0")),
            ),
            device_name: Some(String::from("AMD Radeon Test")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(24 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("userspace_opt_in")],
            amd_metadata: Some(AmdDeviceMetadata {
                mode: AmdRuntimeMode::Userspace,
                topology: AmdTopologyInfo {
                    architecture: Some(String::from("gfx1100")),
                    pci_bdf: Some(String::from("0000:03:00.0")),
                    xcc_count: Some(1),
                    shader_engine_count: Some(4),
                    compute_unit_count: Some(60),
                    vram_bytes: Some(24 * 1024 * 1024 * 1024),
                    visible_vram_bytes: Some(16 * 1024 * 1024 * 1024),
                },
                risk: AmdRiskProfile {
                    level: AmdRiskLevel::Elevated,
                    requires_explicit_opt_in: true,
                    may_unbind_kernel_driver: true,
                    warnings: vec![String::from(
                        "userspace mode may require unloading or rebinding amdgpu",
                    )],
                },
                recovery: AmdRecoveryProfile {
                    driver_binding: AmdDriverBinding::UserspaceClaimed,
                    expected_actions: vec![
                        AmdRecoveryAction::ProcessRestart,
                        AmdRecoveryAction::RebindKernelDriver,
                    ],
                },
            }),
        };
        let report = AmdBackendReport {
            mode: AmdRuntimeMode::Userspace,
            opt_in: AmdOptInStatus::Enabled,
            devices: vec![device],
            health: RuntimeHealth {
                status: HealthStatus::Degraded,
                message: String::from("amdgpu is still loaded; userspace mode not yet ready"),
            },
        };

        assert_eq!(
            serde_json::to_value(&report)?,
            json!({
                "mode": "userspace",
                "opt_in": "enabled",
                "devices": [{
                    "backend": "amd_userspace",
                    "device": {
                        "kind": "AmdUserspace",
                        "ordinal": 0,
                        "label": "amd_userspace:0"
                    },
                    "device_name": "AMD Radeon Test",
                    "supported_dtypes": ["F32"],
                    "supported_quantization": [],
                    "memory_capacity_bytes": 25769803776u64,
                    "unified_memory": false,
                    "feature_flags": ["userspace_opt_in"],
                    "amd_metadata": {
                        "mode": "userspace",
                        "topology": {
                            "architecture": "gfx1100",
                            "pci_bdf": "0000:03:00.0",
                            "xcc_count": 1,
                            "shader_engine_count": 4,
                            "compute_unit_count": 60,
                            "vram_bytes": 25769803776u64,
                            "visible_vram_bytes": 17179869184u64
                        },
                        "risk": {
                            "level": "elevated",
                            "requires_explicit_opt_in": true,
                            "may_unbind_kernel_driver": true,
                            "warnings": [
                                "userspace mode may require unloading or rebinding amdgpu"
                            ]
                        },
                        "recovery": {
                            "driver_binding": "userspace_claimed",
                            "expected_actions": ["process_restart", "rebind_kernel_driver"]
                        }
                    }
                }],
                "health": {
                    "status": "Degraded",
                    "message": "amdgpu is still loaded; userspace mode not yet ready"
                }
            })
        );
        Ok(())
    }
}
