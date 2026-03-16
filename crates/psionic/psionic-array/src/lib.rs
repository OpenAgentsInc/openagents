//! Public lazy array facade above `psionic-core` and `psionic-ir`.
//!
//! This first surface is intentionally narrow. It establishes a user-facing
//! lazy array handle and graph-backed arithmetic above the lower graph builder
//! substrate without claiming `eval`, device-stream execution, or broader
//! MLX-class array closure yet.

use std::{cell::RefCell, rc::Rc};

use psionic_core::{DType, Device, LazyOp, Shape, Tensor, TensorId, TensorSpec};
use psionic_ir::{Graph, GraphBuilder, GraphError};
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
    /// The lower graph builder rejected the requested operation.
    #[error(transparent)]
    Graph(#[from] GraphError),
}

/// Public graph-construction context for lazy arrays.
///
/// The context owns one deterministic `GraphBuilder` so arrays can share graph
/// identity without bloating lower substrate crates. Evaluation and runtime
/// materialization semantics intentionally remain outside this crate for now.
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

#[cfg(test)]
mod tests {
    use super::{ArrayContext, ArrayError};
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
}
