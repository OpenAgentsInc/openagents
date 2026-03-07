//! Runtime traits and execution surfaces for Rustygrad.

use std::collections::BTreeMap;

use rustygrad_core::{DType, Device, TensorId, TensorSpec};
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
    /// Supported dtypes for the device.
    pub supported_dtypes: Vec<DType>,
    /// Optional memory capacity in bytes.
    pub memory_capacity_bytes: Option<u64>,
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

    use super::{
        Allocator, BufferHandle, DeviceDescriptor, DeviceDiscovery, ExecutionBackend,
        ExecutionMetrics, ExecutionResult, HealthStatus, RuntimeError, RuntimeHealth,
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
                supported_dtypes: vec![DType::F32],
                memory_capacity_bytes: None,
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
        assert_eq!(devices.len(), 1);
        assert_eq!(runtime.health().status, HealthStatus::Ready);

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
        assert_eq!(result.metrics.steps_executed, 1);
        Ok(())
    }
}
