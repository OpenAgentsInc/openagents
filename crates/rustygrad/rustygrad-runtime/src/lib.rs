//! Runtime traits and execution surfaces for Rustygrad.

use std::collections::BTreeMap;

use rustygrad_core::{DType, Device, QuantizationMode, TensorId, TensorSpec};
use rustygrad_ir::ExecutionPlan;
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

/// Runtime support declaration for a quantization mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizationSupport {
    /// Supported quantization mode.
    pub mode: QuantizationMode,
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

    use rustygrad_core::{DType, Device, Shape, TensorSpec};
    use rustygrad_ir::{ExecutionOp, ExecutionPlan, ExecutionStep};
    use serde_json::json;

    use super::{
        Allocator, BackendSelection, BufferHandle, DeviceDescriptor, DeviceDiscovery,
        ExecutionBackend, ExecutionMetrics, ExecutionResult, HealthStatus, QuantizationExecution,
        QuantizationSupport, RuntimeError, RuntimeHealth,
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
                    mode: rustygrad_core::QuantizationMode::None,
                    execution: QuantizationExecution::Native,
                }],
                memory_capacity_bytes: None,
                unified_memory: Some(true),
                feature_flags: vec![String::from("mock_execution")],
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
            _inputs: &BTreeMap<rustygrad_core::TensorId, Self::Buffer>,
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
        inputs.insert(rustygrad_core::TensorId(0), buffer);

        let plan = ExecutionPlan {
            graph_digest: String::from("digest"),
            steps: vec![ExecutionStep {
                output: rustygrad_core::TensorId(1),
                op: ExecutionOp::Add,
                spec: TensorSpec::new(Shape::new(vec![1, 2]), DType::F32, Device::cpu()),
                inputs: vec![rustygrad_core::TensorId(0)],
            }],
            outputs: vec![rustygrad_core::TensorId(1)],
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
    fn backend_selection_helpers_capture_direct_and_fallback_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
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
}
