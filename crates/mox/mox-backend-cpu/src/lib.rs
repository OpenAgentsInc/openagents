//! CPU backend for Mox.

use std::collections::BTreeMap;

use mox_compiler::compile_graph;
use mox_core::{DType, Device, Shape, TensorData, TensorId, TensorSpec};
use mox_ir::{ExecutionOp, ExecutionPlan, ExecutionStep, Graph};
use mox_runtime::{
    Allocator, BackendName, BackendSelection, BufferHandle, DeviceDescriptor, DeviceDiscovery,
    ExecutionBackend, ExecutionMetrics, ExecutionResult, HealthStatus, QuantizationExecution,
    QuantizationLoadPath, QuantizationSupport, RuntimeError, RuntimeHealth,
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
        Ok(Self { spec, data })
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
            data: vec![0.0; spec.storage_size()],
        }
    }

    /// Returns the raw backing storage.
    #[must_use]
    pub fn as_f32_slice(&self) -> &[f32] {
        self.data.as_slice()
    }

    /// Returns logical values in row-major order.
    #[must_use]
    pub fn logical_values(&self) -> Vec<f32> {
        let mut output = Vec::with_capacity(self.spec.element_count());
        for_each_index(self.spec.shape(), |index| {
            output.push(self.data[self.storage_index(index)]);
        });
        output
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
        Self::from_storage_f32(spec, source.data.clone())
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
        let plan =
            compile_graph(graph).map_err(|error| RuntimeError::Backend(error.to_string()))?;
        self.execute(&plan, inputs)
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
            ExecutionOp::Add => {
                let (left, right) = self.binary_inputs(step, values)?;
                let output = left
                    .logical_values()
                    .into_iter()
                    .zip(right.logical_values())
                    .map(|(lhs, rhs)| lhs + rhs)
                    .collect::<Vec<_>>();
                CpuBuffer::from_f32(step.spec.clone(), output)?
            }
            ExecutionOp::Mul => {
                let (left, right) = self.binary_inputs(step, values)?;
                let output = left
                    .logical_values()
                    .into_iter()
                    .zip(right.logical_values())
                    .map(|(lhs, rhs)| lhs * rhs)
                    .collect::<Vec<_>>();
                CpuBuffer::from_f32(step.spec.clone(), output)?
            }
            ExecutionOp::Matmul => self.matmul(step, values)?,
            ExecutionOp::Reshape => {
                let source = self.input(step, values, 0)?.logical_values();
                CpuBuffer::from_f32(step.spec.clone(), source)?
            }
            ExecutionOp::Permute { .. }
            | ExecutionOp::Slice { .. }
            | ExecutionOp::Select { .. }
            | ExecutionOp::Expand { .. } => {
                CpuBuffer::view_of(self.input(step, values, 0)?, step.spec.clone())?
            }
            ExecutionOp::Concat { axis } => self.concat(step, values, *axis)?,
            ExecutionOp::ReduceSum { axis } => self.reduce_sum(step, values, *axis)?,
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
        let left_values = left.logical_values();
        let right_values = right.logical_values();

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
            .collect::<Vec<_>>();

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
        let logical = input.logical_values();
        let output = match axis {
            None => vec![logical.iter().copied().sum()],
            Some(axis) => reduce_sum_axis(&logical, input.spec().shape().dims(), axis)?,
        };
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
            device_name: Some(String::from("host cpu")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: vec![
                QuantizationSupport {
                    mode: mox_core::QuantizationMode::None,
                    load_path: QuantizationLoadPath::DenseF32,
                    execution: QuantizationExecution::Native,
                },
                QuantizationSupport {
                    mode: mox_core::QuantizationMode::Int8Symmetric,
                    load_path: QuantizationLoadPath::DequantizedF32,
                    execution: QuantizationExecution::DequantizeToF32,
                },
            ],
            memory_capacity_bytes: None,
            unified_memory: Some(true),
            feature_flags: vec![String::from("host_memory")],
            amd_metadata: None,
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
    use std::collections::BTreeMap;

    use mox_core::{DType, Device, Shape, TensorSpec};
    use mox_ir::GraphBuilder;
    use mox_runtime::{
        Allocator, BackendSelectionState, BufferHandle, DeviceDiscovery, HealthStatus,
        RuntimeError, ServedProductBackendPolicy,
    };

    use super::CpuBackend;

    #[test]
    fn cpu_backend_reports_default_device() -> Result<(), mox_runtime::RuntimeError> {
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
                    mode: mox_core::QuantizationMode::None,
                    load_path: super::QuantizationLoadPath::DenseF32,
                    execution: super::QuantizationExecution::Native,
                },
                super::QuantizationSupport {
                    mode: mox_core::QuantizationMode::Int8Symmetric,
                    load_path: super::QuantizationLoadPath::DequantizedF32,
                    execution: super::QuantizationExecution::DequantizeToF32,
                }
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
        assert_eq!(output.as_f32_slice(), &[8.0, 12.0]);
        Ok(())
    }

    #[test]
    fn cpu_allocator_creates_zeroed_buffer() -> Result<(), mox_runtime::RuntimeError> {
        let mut backend = CpuBackend::new();
        let spec = TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu());
        let buffer = backend.allocate(&spec)?;
        assert_eq!(buffer.as_f32_slice(), &[0.0, 0.0, 0.0, 0.0]);
        assert_eq!(buffer.spec(), &spec);
        Ok(())
    }
}
