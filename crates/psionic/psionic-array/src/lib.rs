//! Public lazy array facade above `psionic-core` and `psionic-ir`.
//!
//! This first surface is intentionally narrow. It establishes a user-facing
//! lazy array handle, public device and stream contracts, graph-backed
//! arithmetic, and explicit evaluation semantics above the lower graph builder
//! substrate without claiming full runtime scheduling or broader MLX-class
//! array closure yet.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use psionic_core::{DType, Device, DeviceKind, LazyOp, Shape, Tensor, TensorData, TensorId, TensorSpec};
use psionic_ir::{Graph, GraphBuilder, GraphError, OpKind};
use psionic_runtime::{DeviceDescriptor, DeviceInventoryQualifiers, DeviceMemoryClass};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "public lazy array facade above psionic-core and psionic-ir";

#[derive(Debug)]
struct ArrayContextInner {
    device: ArrayDevice,
    builder: RefCell<GraphBuilder>,
    next_stream_id: RefCell<u32>,
}

/// Error type raised by the public lazy-array facade.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArrayError {
    /// Two arrays came from different public graph-construction contexts.
    #[error("array operations require arrays from the same ArrayContext")]
    MixedContexts,
    /// One operation expected at least one array but received none.
    #[error("array operation requires at least one input array")]
    EmptyArrayList,
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
        matches!(self.unified_memory, UnifiedMemoryCapability::HostOnly | UnifiedMemoryCapability::SharedHostDevice)
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterializationTrigger {
    /// Synchronous explicit materialization through `eval`.
    Eval,
    /// Deferred explicit materialization through `async_eval(...).wait()`.
    AsyncEvalWait,
}

/// Current policy for implicit host or display materialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImplicitMaterializationPolicy {
    /// Only explicit eval entrypoints may materialize values today.
    ExplicitOnly,
}

/// Replay boundary for one public materialization contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayBoundary {
    /// Evaluation replays against the captured graph snapshot digest.
    GraphSnapshot,
}

/// Public materialization boundary report for one lazy array.
#[derive(Clone, Debug, PartialEq, Eq)]
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
            ],
            implicit_policy: ImplicitMaterializationPolicy::ExplicitOnly,
            replay_boundary: ReplayBoundary::GraphSnapshot,
        }
    }
}

/// Receipt emitted for one explicit lazy-array evaluation boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
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
        evaluate_graph_snapshot(
            &self.graph,
            self.outputs.as_slice(),
            MaterializationTrigger::AsyncEvalWait,
            &self.stream,
        )
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
        let stream = ArrayStream::default_for(device.clone());
        Self {
            inner: Rc::new(ArrayContextInner {
                builder: RefCell::new(GraphBuilder::new(device.device().clone())),
                device,
                next_stream_id: RefCell::new(1),
            }),
            stream,
        }
    }

    /// Creates a CPU-backed context for tests and reference graph building.
    #[must_use]
    pub fn cpu() -> Self {
        Self::new(Device::cpu())
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
        let tensor = self.inner.builder.borrow_mut().constant_f32(shape, values)?;
        Ok(Array::from_tensor(self.inner.clone(), self.stream.clone(), tensor))
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
        evaluate_graph_snapshot(&graph, outputs, MaterializationTrigger::Eval, &self.stream)
    }

    /// Captures a replay-stable deferred evaluation ticket for the requested
    /// outputs.
    pub fn async_eval(&self, outputs: &[Array]) -> Result<PendingAsyncEval, ArrayError> {
        let graph = self.graph_for(outputs)?;
        Ok(PendingAsyncEval {
            graph,
            outputs: outputs.to_vec(),
            stream: self.stream.clone(),
        })
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

impl Array {
    fn from_tensor(context: Rc<ArrayContextInner>, stream: ArrayStream, tensor: Tensor) -> Self {
        let graph = context.builder.borrow().clone().finish(vec![tensor.clone()]);
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
        Ok(Self::from_tensor(self.context.clone(), self.stream.clone(), tensor))
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

    /// Explicitly materializes this array through the bounded CPU-reference
    /// path.
    pub fn eval(&self) -> Result<EvaluatedArray, ArrayError> {
        let mut outputs = self.context().eval(&[self.clone()])?;
        Ok(outputs.remove(0))
    }

    /// Captures a replay-stable deferred evaluation ticket for this array.
    pub fn async_eval(&self) -> Result<PendingAsyncEval, ArrayError> {
        self.context().async_eval(&[self.clone()])
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
        Ok(Self::from_tensor(self.context.clone(), self.stream.clone(), tensor))
    }

    /// Reorders axes using a logical view.
    pub fn permute(&self, axes: Vec<usize>) -> Result<Self, ArrayError> {
        let tensor = self
            .context
            .builder
            .borrow_mut()
            .permute(&self.tensor, axes)?;
        Ok(Self::from_tensor(self.context.clone(), self.stream.clone(), tensor))
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
        Ok(Self::from_tensor(self.context.clone(), self.stream.clone(), tensor))
    }

    /// Selects one index along one axis and removes that axis.
    pub fn select(&self, axis: usize, index: usize) -> Result<Self, ArrayError> {
        let tensor = self
            .context
            .builder
            .borrow_mut()
            .select(&self.tensor, axis, index)?;
        Ok(Self::from_tensor(self.context.clone(), self.stream.clone(), tensor))
    }

    /// Broadcasts the array to the provided target shape.
    pub fn broadcast_to(&self, shape: Shape) -> Result<Self, ArrayError> {
        let tensor = self
            .context
            .builder
            .borrow_mut()
            .expand(&self.tensor, shape)?;
        Ok(Self::from_tensor(self.context.clone(), self.stream.clone(), tensor))
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
            OpKind::Expand { shape } => expand_dense_value(
                node.tensor(),
                node.inputs(),
                &values,
                shape,
            )?,
            OpKind::ReduceSum { axis } => reduce_sum_dense_value(
                node.tensor(),
                node.inputs(),
                &values,
                *axis,
            )?,
            other => {
                return Err(ArrayError::MaterializationRefusal {
                    tensor: node.tensor().id(),
                    op: other.label().to_string(),
                    detail: String::from(
                        "bounded explicit eval currently materializes only constant, detach, add, mul, matmul, reshape, permute, slice, select, concat, expand, and reduce_sum graphs",
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
    let input = inputs.first().copied().ok_or(ArrayError::MaterializationRefusal {
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
    let left = values
        .get(left_id)
        .ok_or(ArrayError::MissingDependency {
            tensor: tensor.id(),
            input: *left_id,
        })?;
    let right = values
        .get(right_id)
        .ok_or(ArrayError::MissingDependency {
            tensor: tensor.id(),
            input: *right_id,
        })?;
    if left.tensor.spec().shape() != right.tensor.spec().shape() {
        return Err(ArrayError::MaterializationRefusal {
            tensor: tensor.id(),
            op: String::from("binary"),
            detail: String::from("bounded explicit eval expects binary inputs to be shape-aligned after graph expansion"),
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
    let left = values
        .get(left_id)
        .ok_or(ArrayError::MissingDependency {
            tensor: tensor.id(),
            input: *left_id,
        })?;
    let right = values
        .get(right_id)
        .ok_or(ArrayError::MissingDependency {
            tensor: tensor.id(),
            input: *right_id,
        })?;
    let left_shape = left.tensor.spec().shape();
    let right_shape = right.tensor.spec().shape();
    let (m, k_left, k_right, n) = match (
        left_shape.dims(),
        right_shape.dims(),
    ) {
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
    let input = inputs.first().copied().ok_or(ArrayError::MaterializationRefusal {
        tensor: tensor.id(),
        op: String::from("reshape"),
        detail: String::from("reshape requires one input"),
    })?;
    let value = values
        .get(&input)
        .ok_or(ArrayError::MissingDependency {
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
    let input = inputs.first().copied().ok_or(ArrayError::MaterializationRefusal {
        tensor: tensor.id(),
        op: String::from("permute"),
        detail: String::from("permute requires one input"),
    })?;
    let value = values
        .get(&input)
        .ok_or(ArrayError::MissingDependency {
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
    let input = inputs.first().copied().ok_or(ArrayError::MaterializationRefusal {
        tensor: tensor.id(),
        op: String::from("slice"),
        detail: String::from("slice requires one input"),
    })?;
    let value = values
        .get(&input)
        .ok_or(ArrayError::MissingDependency {
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
    let input = inputs.first().copied().ok_or(ArrayError::MaterializationRefusal {
        tensor: tensor.id(),
        op: String::from("select"),
        detail: String::from("select requires one input"),
    })?;
    let value = values
        .get(&input)
        .ok_or(ArrayError::MissingDependency {
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
            values.get(input).cloned().ok_or(ArrayError::MissingDependency {
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
    let input = inputs.first().copied().ok_or(ArrayError::MaterializationRefusal {
        tensor: tensor.id(),
        op: String::from("expand"),
        detail: String::from("expand requires one input"),
    })?;
    let value = values
        .get(&input)
        .ok_or(ArrayError::MissingDependency {
            tensor: tensor.id(),
            input,
        })?;
    let expanded = expand_values(value.tensor.spec().shape(), &value.values, shape);
    Ok(DenseValue {
        tensor: tensor.clone(),
        values: expanded,
    })
}

fn reduce_sum_dense_value(
    tensor: &Tensor,
    inputs: &[TensorId],
    values: &BTreeMap<TensorId, DenseValue>,
    axis: Option<usize>,
) -> Result<DenseValue, ArrayError> {
    let input = inputs.first().copied().ok_or(ArrayError::MaterializationRefusal {
        tensor: tensor.id(),
        op: String::from("reduce_sum"),
        detail: String::from("reduce_sum requires one input"),
    })?;
    let value = values
        .get(&input)
        .ok_or(ArrayError::MissingDependency {
            tensor: tensor.id(),
            input,
        })?;
    let (shape, output) = match axis {
        None => (Shape::scalar(), vec![value.values.iter().sum()]),
        Some(axis) => reduce_sum_axis(tensor.id(), value.tensor.spec().shape(), &value.values, axis)?,
    };
    Ok(DenseValue {
        tensor: Tensor::new(tensor.id(), TensorSpec::new(shape, value.tensor.spec().dtype(), value.tensor.spec().device().clone()), LazyOp::Constant),
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
        Array, ArrayContext, ArrayError, AsyncEvalStatus, ImplicitMaterializationPolicy,
        MaterializationTrigger, ReplayBoundary, StreamDependencyPolicy, StreamKind,
        UnifiedMemoryCapability,
    };
    use psionic_core::{DType, Device, DeviceKind, Shape};
    use psionic_runtime::DeviceDescriptor;

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
            graph.nodes().iter().map(|node| node.op().label()).collect::<Vec<_>>(),
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
            vec![MaterializationTrigger::Eval, MaterializationTrigger::AsyncEvalWait]
        );
        assert_eq!(
            boundary.implicit_policy,
            ImplicitMaterializationPolicy::ExplicitOnly
        );
        assert_eq!(boundary.replay_boundary, ReplayBoundary::GraphSnapshot);

        let evaluated = output.eval()?;
        assert_eq!(evaluated.receipt().trigger, MaterializationTrigger::Eval);
        assert_eq!(evaluated.receipt().stream_id, context.stream().stream_id());
        assert_eq!(evaluated.receipt().device_id, context.device_handle().stable_id());
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
}
