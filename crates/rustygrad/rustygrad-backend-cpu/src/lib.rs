//! CPU backend for Rustygrad.

use std::collections::BTreeMap;

use rustygrad_compiler::compile_graph;
use rustygrad_core::{DType, Device, Shape, TensorData, TensorId, TensorSpec};
use rustygrad_ir::{ExecutionOp, ExecutionPlan, ExecutionStep, Graph};
use rustygrad_runtime::{
    Allocator, BackendName, BufferHandle, DeviceDescriptor, DeviceDiscovery, ExecutionBackend,
    ExecutionMetrics, ExecutionResult, HealthStatus, RuntimeError, RuntimeHealth,
};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "CPU reference backend";

/// Host-resident tensor buffer for CPU execution.
#[derive(Clone, Debug, PartialEq)]
pub struct CpuBuffer {
    spec: TensorSpec,
    data: Vec<f32>,
}

impl CpuBuffer {
    /// Builds a buffer from `f32` values.
    pub fn from_f32(spec: TensorSpec, data: impl Into<Vec<f32>>) -> Result<Self, RuntimeError> {
        let data = data.into();
        let expected = spec.element_count();
        if expected != data.len() {
            return Err(RuntimeError::Backend(format!(
                "buffer length mismatch: expected {expected}, actual {}",
                data.len()
            )));
        }
        Ok(Self { spec, data })
    }

    /// Builds a buffer from tensor data.
    pub fn from_tensor_data(spec: TensorSpec, data: &TensorData) -> Result<Self, RuntimeError> {
        Self::from_f32(spec, data.as_f32_slice().to_vec())
    }

    /// Returns a zeroed buffer for a tensor spec.
    #[must_use]
    pub fn zeros(spec: &TensorSpec) -> Self {
        Self {
            spec: spec.clone(),
            data: vec![0.0; spec.element_count()],
        }
    }

    /// Returns the underlying data.
    #[must_use]
    pub fn as_f32_slice(&self) -> &[f32] {
        self.data.as_slice()
    }
}

impl BufferHandle for CpuBuffer {
    fn spec(&self) -> &TensorSpec {
        &self.spec
    }
}

/// CPU reference backend implementation.
#[derive(Clone, Debug, Default)]
pub struct CpuBackend;

impl CpuBackend {
    /// Creates a CPU backend.
    #[must_use]
    pub const fn new() -> Self {
        Self
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
        let plan = compile_graph(graph).map_err(|error| RuntimeError::Backend(error.to_string()))?;
        self.execute(&plan, inputs)
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
            ExecutionOp::Add => {
                let (left, right) = self.binary_inputs(step, values)?;
                CpuBuffer::from_f32(
                    step.spec.clone(),
                    left.as_f32_slice()
                        .iter()
                        .zip(right.as_f32_slice())
                        .map(|(lhs, rhs)| lhs + rhs)
                        .collect::<Vec<_>>(),
                )?
            }
            ExecutionOp::Mul => {
                let (left, right) = self.binary_inputs(step, values)?;
                CpuBuffer::from_f32(
                    step.spec.clone(),
                    left.as_f32_slice()
                        .iter()
                        .zip(right.as_f32_slice())
                        .map(|(lhs, rhs)| lhs * rhs)
                        .collect::<Vec<_>>(),
                )?
            }
            ExecutionOp::Matmul => self.matmul(step, values)?,
            ExecutionOp::Reshape => {
                let source = self
                    .input(step, values, 0)?
                    .as_f32_slice()
                    .to_vec();
                CpuBuffer::from_f32(step.spec.clone(), source)?
            }
            ExecutionOp::ReduceSum => {
                let source = self.input(step, values, 0)?;
                CpuBuffer::from_f32(
                    step.spec.clone(),
                    vec![source.as_f32_slice().iter().copied().sum()],
                )?
            }
        };

        values.insert(step.output, buffer);
        Ok(())
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
        let left_values = left.as_f32_slice();
        let right_values = right.as_f32_slice();

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
}

impl DeviceDiscovery for CpuBackend {
    fn backend_name(&self) -> BackendName {
        "cpu"
    }

    fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError> {
        Ok(vec![DeviceDescriptor {
            backend: String::from(self.backend_name()),
            device: Device::cpu(),
            supported_dtypes: vec![DType::F32],
            memory_capacity_bytes: None,
        }])
    }

    fn health(&self) -> RuntimeHealth {
        RuntimeHealth {
            status: HealthStatus::Ready,
            message: String::from("cpu backend ready"),
        }
    }
}

impl Allocator for CpuBackend {
    type Buffer = CpuBuffer;

    fn allocate(&mut self, spec: &TensorSpec) -> Result<Self::Buffer, RuntimeError> {
        Ok(CpuBuffer::zeros(spec))
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
        for output in &plan.outputs {
            let Some(buffer) = values.get(output).cloned() else {
                return Err(RuntimeError::MissingInput(*output));
            };
            outputs.insert(*output, buffer);
        }

        Ok(ExecutionResult {
            outputs,
            metrics: ExecutionMetrics {
                steps_executed: plan.steps.len(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rustygrad_core::{DType, Device, Shape, TensorSpec};
    use rustygrad_ir::GraphBuilder;
    use rustygrad_runtime::{Allocator, BufferHandle, DeviceDiscovery, RuntimeError};

    use super::CpuBackend;

    #[test]
    fn cpu_backend_reports_default_device() -> Result<(), rustygrad_runtime::RuntimeError> {
        let backend = CpuBackend::new();
        let devices = backend.discover_devices()?;
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device, Device::cpu());
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
        inputs.insert(input.id(), backend.input_buffer(Shape::new(vec![2, 2]), vec![1.0, 0.0, 0.0, 1.0])?);

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let Some(output) = result.outputs.get(&shifted.id()) else {
            return Err(RuntimeError::Backend(String::from("missing output")));
        };
        assert_eq!(output.as_f32_slice(), &[1.5, 2.5, 3.5, 4.5]);
        assert_eq!(result.metrics.steps_executed, 5);
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
        assert_eq!(output.as_f32_slice(), &[10.0]);
        Ok(())
    }

    #[test]
    fn cpu_allocator_creates_zeroed_buffer() -> Result<(), rustygrad_runtime::RuntimeError> {
        let mut backend = CpuBackend::new();
        let spec = TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu());
        let buffer = backend.allocate(&spec)?;
        assert_eq!(buffer.as_f32_slice(), &[0.0, 0.0, 0.0, 0.0]);
        assert_eq!(buffer.spec(), &spec);
        Ok(())
    }
}
