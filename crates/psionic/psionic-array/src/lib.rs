//! Public lazy array facade above `psionic-core` and `psionic-ir`.
//!
//! This first surface is intentionally narrow. It establishes a user-facing
//! lazy array handle, graph-backed arithmetic, and explicit evaluation
//! semantics above the lower graph builder substrate without claiming device-
//! stream execution or broader MLX-class array closure yet.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use psionic_core::{DType, Device, LazyOp, Shape, Tensor, TensorData, TensorId, TensorSpec};
use psionic_ir::{Graph, GraphBuilder, GraphError, OpKind};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "public lazy array facade above psionic-core and psionic-ir";

#[derive(Debug)]
struct ArrayContextInner {
    device: Device,
    builder: RefCell<GraphBuilder>,
}

/// Error type raised by the public lazy-array facade.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArrayError {
    /// Two arrays came from different public graph-construction contexts.
    #[error("array operations require arrays from the same ArrayContext")]
    MixedContexts,
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
}

impl ArrayContext {
    /// Creates a context pinned to one device.
    #[must_use]
    pub fn new(device: Device) -> Self {
        Self {
            inner: Rc::new(ArrayContextInner {
                builder: RefCell::new(GraphBuilder::new(device.clone())),
                device,
            }),
        }
    }

    /// Creates a CPU-backed context for tests and reference graph building.
    #[must_use]
    pub fn cpu() -> Self {
        Self::new(Device::cpu())
    }

    /// Returns the device assigned to the context.
    #[must_use]
    pub fn device(&self) -> &Device {
        &self.inner.device
    }

    /// Adds a named input array to the current lazy graph.
    #[must_use]
    pub fn input(&self, name: impl Into<String>, shape: Shape, dtype: DType) -> Array {
        let tensor = self.inner.builder.borrow_mut().input(name, shape, dtype);
        Array::from_tensor(self.inner.clone(), tensor)
    }

    /// Adds an `f32` constant array to the current lazy graph.
    pub fn constant_f32(
        &self,
        shape: Shape,
        values: impl Into<Vec<f32>>,
    ) -> Result<Array, ArrayError> {
        let tensor = self.inner.builder.borrow_mut().constant_f32(shape, values)?;
        Ok(Array::from_tensor(self.inner.clone(), tensor))
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
        evaluate_graph_snapshot(&graph, outputs, MaterializationTrigger::Eval)
    }

    /// Captures a replay-stable deferred evaluation ticket for the requested
    /// outputs.
    pub fn async_eval(&self, outputs: &[Array]) -> Result<PendingAsyncEval, ArrayError> {
        let graph = self.graph_for(outputs)?;
        Ok(PendingAsyncEval {
            graph,
            outputs: outputs.to_vec(),
        })
    }
}

/// Public lazy array handle backed by the canonical Psionic graph builder.
#[derive(Clone, Debug)]
pub struct Array {
    context: Rc<ArrayContextInner>,
    tensor: Tensor,
    graph: Graph,
}

impl Array {
    fn from_tensor(context: Rc<ArrayContextInner>, tensor: Tensor) -> Self {
        let graph = context.builder.borrow().clone().finish(vec![tensor.clone()]);
        Self {
            context,
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
        Ok(Self::from_tensor(self.context.clone(), tensor))
    }

    /// Returns the owning public graph-construction context.
    #[must_use]
    pub fn context(&self) -> ArrayContext {
        ArrayContext {
            inner: self.context.clone(),
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
        Self::from_tensor(self.context.clone(), tensor)
    }

    /// Reduces the array to a scalar sum.
    #[must_use]
    pub fn sum(&self) -> Self {
        let tensor = self.context.builder.borrow_mut().reduce_sum(&self.tensor);
        Self::from_tensor(self.context.clone(), tensor)
    }
}

fn evaluate_graph_snapshot(
    graph: &Graph,
    requested_outputs: &[Array],
    trigger: MaterializationTrigger,
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
                        "bounded explicit eval currently materializes only constant, detach, add, mul, matmul, expand, and reduce_sum graphs",
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
        ArrayContext, ArrayError, AsyncEvalStatus, ImplicitMaterializationPolicy,
        MaterializationTrigger, ReplayBoundary,
    };
    use psionic_core::{DType, Shape};

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
        assert_eq!(evaluated.data.as_f32_slice(), Some(&[20.0][..]));

        let pending = output.async_eval()?;
        assert_eq!(pending.status(), AsyncEvalStatus::Pending);
        let mut resolved = pending.wait()?;
        let evaluated_async = resolved.remove(0);
        assert_eq!(
            evaluated_async.receipt().trigger,
            MaterializationTrigger::AsyncEvalWait
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
}
