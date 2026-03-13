//! Canonical graph and plan representation for Psionic.

use psionic_core::{
    BackendExtensionOp, DType, Device, LazyOp, QuantizationMode, QuantizedTensorData, Shape,
    Tensor, TensorData, TensorId, TensorSpec,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "canonical IR and execution plan types";

/// Error type raised during graph construction.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GraphError {
    /// A constant payload length does not match the declared tensor shape.
    #[error("constant payload length {actual} does not match expected element count {expected}")]
    ConstantLengthMismatch {
        /// Expected element count from the shape.
        expected: usize,
        /// Actual element count in the payload.
        actual: usize,
    },
    /// A quantized constant payload did not match its declared GGML block layout.
    #[error("invalid quantized constant mode {mode:?} for shape {shape}: {message}")]
    InvalidQuantizedConstant {
        /// Quantization family for the payload.
        mode: QuantizationMode,
        /// Declared logical tensor shape.
        shape: Shape,
        /// Human-readable validation failure.
        message: String,
    },
    /// Two tensors with incompatible shapes were used in a binary op.
    #[error("binary op shape mismatch: left={left} right={right}")]
    BinaryShapeMismatch {
        /// Left-hand shape.
        left: Shape,
        /// Right-hand shape.
        right: Shape,
    },
    /// Two tensors with incompatible dtypes were used in a binary op.
    #[error("binary op dtype mismatch: left={left:?} right={right:?}")]
    BinaryDTypeMismatch {
        /// Left-hand dtype.
        left: DType,
        /// Right-hand dtype.
        right: DType,
    },
    /// A matmul used tensors with unsupported ranks or dimensions.
    #[error("invalid matmul shapes: left={left} right={right}")]
    InvalidMatmulShapes {
        /// Left-hand shape.
        left: Shape,
        /// Right-hand shape.
        right: Shape,
    },
    /// A reshape changed the number of elements.
    #[error("reshape would change element count: from={from} to={to}")]
    InvalidReshape {
        /// Original shape.
        from: Shape,
        /// Requested target shape.
        to: Shape,
    },
    /// A permute requested an invalid axis order.
    #[error("invalid permute axes {axes:?} for shape {shape}")]
    InvalidPermute {
        /// Input shape.
        shape: Shape,
        /// Requested axes.
        axes: Vec<usize>,
    },
    /// A slice requested invalid bounds.
    #[error("invalid slice axis={axis} start={start} end={end} for shape {shape}")]
    InvalidSlice {
        /// Input shape.
        shape: Shape,
        /// Requested axis.
        axis: usize,
        /// Slice start.
        start: usize,
        /// Slice end.
        end: usize,
    },
    /// A select requested an invalid index.
    #[error("invalid select axis={axis} index={index} for shape {shape}")]
    InvalidSelect {
        /// Input shape.
        shape: Shape,
        /// Requested axis.
        axis: usize,
        /// Selected index.
        index: usize,
    },
    /// A concat could not be formed from the provided tensors.
    #[error("invalid concat axis={axis} for shapes {shapes:?}")]
    InvalidConcat {
        /// Requested axis.
        axis: usize,
        /// Input shapes.
        shapes: Vec<Shape>,
    },
    /// An expand requested an incompatible target shape.
    #[error("invalid expand from {from} to {to}")]
    InvalidExpand {
        /// Input shape.
        from: Shape,
        /// Requested target shape.
        to: Shape,
    },
    /// A reduction requested an invalid axis.
    #[error("invalid reduce_sum axis={axis} for shape {shape}")]
    InvalidReduceAxis {
        /// Input shape.
        shape: Shape,
        /// Requested axis.
        axis: usize,
    },
    /// A typed backend-extension op was constructed with invalid inputs.
    #[error("invalid backend extension `{op}`: {message}")]
    InvalidBackendExtension {
        /// Stable backend-extension label.
        op: String,
        /// Human-readable validation failure.
        message: String,
    },
}

/// Operation kind recorded in the canonical graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OpKind {
    /// Graph input.
    Input {
        /// Input name for diagnostics.
        name: String,
    },
    /// Constant tensor payload.
    Constant {
        /// Constant data.
        data: TensorData,
    },
    /// Binary add.
    Add,
    /// Binary multiply.
    Mul,
    /// Matrix multiplication.
    Matmul,
    /// Tensor reshape.
    Reshape,
    /// Tensor permute.
    Permute {
        /// Axis order.
        axes: Vec<usize>,
    },
    /// Tensor slice.
    Slice {
        /// Slice axis.
        axis: usize,
        /// Inclusive start.
        start: usize,
        /// Exclusive end.
        end: usize,
    },
    /// Tensor select.
    Select {
        /// Selection axis.
        axis: usize,
        /// Selected index.
        index: usize,
    },
    /// Tensor concat.
    Concat {
        /// Concat axis.
        axis: usize,
    },
    /// Tensor expand/broadcast.
    Expand {
        /// Requested target shape.
        shape: Shape,
    },
    /// Full or axis-specific reduction.
    ReduceSum {
        /// Reduction axis. `None` means reduce all elements.
        axis: Option<usize>,
    },
    /// Typed backend-extension op.
    BackendExtension {
        /// Backend-extension payload.
        op: BackendExtensionOp,
    },
}

impl OpKind {
    /// Returns a stable operation label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Input { .. } => "input",
            Self::Constant { .. } => "constant",
            Self::Add => "add",
            Self::Mul => "mul",
            Self::Matmul => "matmul",
            Self::Reshape => "reshape",
            Self::Permute { .. } => "permute",
            Self::Slice { .. } => "slice",
            Self::Select { .. } => "select",
            Self::Concat { .. } => "concat",
            Self::Expand { .. } => "expand",
            Self::ReduceSum { .. } => "reduce_sum",
            Self::BackendExtension { op } => op.label(),
        }
    }
}

/// Canonical graph node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    tensor: Tensor,
    inputs: Vec<TensorId>,
    op: OpKind,
}

impl Node {
    /// Returns the output tensor handle.
    #[must_use]
    pub fn tensor(&self) -> &Tensor {
        &self.tensor
    }

    /// Returns the input tensor IDs.
    #[must_use]
    pub fn inputs(&self) -> &[TensorId] {
        &self.inputs
    }

    /// Returns the node operation.
    #[must_use]
    pub fn op(&self) -> &OpKind {
        &self.op
    }
}

/// Canonical computation graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    nodes: Vec<Node>,
    outputs: Vec<TensorId>,
}

impl Graph {
    /// Returns graph nodes in deterministic insertion order.
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Returns output tensor IDs.
    #[must_use]
    pub fn outputs(&self) -> &[TensorId] {
        &self.outputs
    }

    /// Returns a node by output tensor ID.
    #[must_use]
    pub fn node(&self, id: TensorId) -> Option<&Node> {
        self.nodes.iter().find(|node| node.tensor.id() == id)
    }

    /// Returns a stable digest of the graph structure and payloads.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        digest_lines(self.stable_lines())
    }

    /// Returns a stable debug string for snapshots and diagnostics.
    #[must_use]
    pub fn stable_debug(&self) -> String {
        self.stable_lines().join("\n")
    }

    fn stable_lines(&self) -> Vec<String> {
        let mut lines = Vec::with_capacity(self.nodes.len() + 1);
        for node in &self.nodes {
            let input_labels = node
                .inputs
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            lines.push(format!(
                "{}|{}|{}|{}|{}|{}",
                node.tensor.id(),
                node.op.label(),
                format_spec(node.tensor.spec()),
                input_labels,
                format_lazy_op(node.tensor.op()),
                format_constant_payload(&node.op),
            ));
        }
        let outputs = self
            .outputs
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        lines.push(format!("outputs|{outputs}"));
        lines
    }
}

/// Executable operation payload emitted by the compiler layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ExecutionOp {
    /// Graph input.
    Input {
        /// Input name.
        name: String,
    },
    /// Constant tensor payload.
    Constant {
        /// Constant data.
        data: TensorData,
    },
    /// Binary add.
    Add,
    /// Binary multiply.
    Mul,
    /// Matrix multiplication.
    Matmul,
    /// Tensor reshape.
    Reshape,
    /// Tensor permute.
    Permute {
        /// Axis order.
        axes: Vec<usize>,
    },
    /// Tensor slice.
    Slice {
        /// Slice axis.
        axis: usize,
        /// Inclusive start.
        start: usize,
        /// Exclusive end.
        end: usize,
    },
    /// Tensor select.
    Select {
        /// Selection axis.
        axis: usize,
        /// Selected index.
        index: usize,
    },
    /// Tensor concat.
    Concat {
        /// Concat axis.
        axis: usize,
    },
    /// Tensor expand/broadcast.
    Expand {
        /// Requested target shape.
        shape: Shape,
    },
    /// Full or axis-specific reduction.
    ReduceSum {
        /// Reduction axis. `None` means reduce all elements.
        axis: Option<usize>,
    },
    /// Typed backend-extension op.
    BackendExtension {
        /// Backend-extension payload.
        op: BackendExtensionOp,
    },
}

impl ExecutionOp {
    /// Returns a stable operation label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Input { .. } => "input",
            Self::Constant { .. } => "constant",
            Self::Add => "add",
            Self::Mul => "mul",
            Self::Matmul => "matmul",
            Self::Reshape => "reshape",
            Self::Permute { .. } => "permute",
            Self::Slice { .. } => "slice",
            Self::Select { .. } => "select",
            Self::Concat { .. } => "concat",
            Self::Expand { .. } => "expand",
            Self::ReduceSum { .. } => "reduce_sum",
            Self::BackendExtension { op } => op.label(),
        }
    }

    /// Converts a graph op into an executable op payload.
    #[must_use]
    pub fn from_op_kind(op: &OpKind) -> Self {
        match op {
            OpKind::Input { name } => Self::Input { name: name.clone() },
            OpKind::Constant { data } => Self::Constant { data: data.clone() },
            OpKind::Add => Self::Add,
            OpKind::Mul => Self::Mul,
            OpKind::Matmul => Self::Matmul,
            OpKind::Reshape => Self::Reshape,
            OpKind::Permute { axes } => Self::Permute { axes: axes.clone() },
            OpKind::Slice { axis, start, end } => Self::Slice {
                axis: *axis,
                start: *start,
                end: *end,
            },
            OpKind::Select { axis, index } => Self::Select {
                axis: *axis,
                index: *index,
            },
            OpKind::Concat { axis } => Self::Concat { axis: *axis },
            OpKind::Expand { shape } => Self::Expand {
                shape: shape.clone(),
            },
            OpKind::ReduceSum { axis } => Self::ReduceSum { axis: *axis },
            OpKind::BackendExtension { op } => Self::BackendExtension { op: op.clone() },
        }
    }
}

/// Execution step placeholder emitted by the compiler layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExecutionStep {
    /// Output tensor ID produced by the step.
    pub output: TensorId,
    /// Operation payload.
    pub op: ExecutionOp,
    /// Static output tensor spec.
    pub spec: TensorSpec,
    /// Input tensor IDs.
    pub inputs: Vec<TensorId>,
}

/// Placeholder execution plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// Stable digest of the source graph.
    pub graph_digest: String,
    /// Ordered execution steps.
    pub steps: Vec<ExecutionStep>,
    /// Output tensor IDs to materialize.
    pub outputs: Vec<TensorId>,
}

impl ExecutionPlan {
    /// Returns a stable digest of the plan shape.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut lines = vec![format!("graph|{}", self.graph_digest)];
        for step in &self.steps {
            let inputs = step
                .inputs
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            lines.push(format!(
                "step|{}|{}|{}|{}|{}",
                step.output,
                step.op.label(),
                format_spec(&step.spec),
                inputs,
                format_execution_payload(&step.op),
            ));
        }
        lines.push(format!(
            "outputs|{}",
            self.outputs
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ));
        digest_lines(lines)
    }

    /// Returns a stable debug view of the plan.
    #[must_use]
    pub fn stable_debug(&self) -> String {
        let mut lines = vec![format!("graph|{}", self.graph_digest)];
        for step in &self.steps {
            let payload = format_execution_payload(&step.op);
            let op = if payload.is_empty() {
                step.op.label().to_string()
            } else {
                format!("{}[{payload}]", step.op.label())
            };
            lines.push(format!(
                "{} <- {}({})",
                step.output,
                op,
                step.inputs
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        lines.push(format!(
            "outputs: {}",
            self.outputs
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
        lines.join("\n")
    }
}

/// Deterministic graph builder.
#[derive(Clone, Debug, Default)]
pub struct GraphBuilder {
    device: Option<Device>,
    next_id: u32,
    nodes: Vec<Node>,
}

impl GraphBuilder {
    /// Creates a builder pinned to a device.
    #[must_use]
    pub fn new(device: Device) -> Self {
        Self {
            device: Some(device),
            next_id: 0,
            nodes: Vec::new(),
        }
    }

    /// Adds a named input tensor.
    pub fn input(&mut self, name: impl Into<String>, shape: Shape, dtype: DType) -> Tensor {
        let name = name.into();
        let spec = TensorSpec::new(shape, dtype, self.device());
        self.register(
            LazyOp::Input { name: name.clone() },
            OpKind::Input { name },
            Vec::new(),
            spec,
        )
    }

    /// Adds an `f32` constant.
    pub fn constant_f32(
        &mut self,
        shape: Shape,
        values: impl Into<Vec<f32>>,
    ) -> Result<Tensor, GraphError> {
        let values = values.into();
        let expected = shape.element_count();
        let actual = values.len();
        if expected != actual {
            return Err(GraphError::ConstantLengthMismatch { expected, actual });
        }
        let spec = TensorSpec::new(shape, DType::F32, self.device());
        Ok(self.register(
            LazyOp::Constant,
            OpKind::Constant {
                data: TensorData::F32(values),
            },
            Vec::new(),
            spec,
        ))
    }

    /// Adds a quantized GGML/GGUF block constant whose logical dtype is `f32`.
    pub fn constant_quantized_blocks(
        &mut self,
        shape: Shape,
        mode: QuantizationMode,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Tensor, GraphError> {
        let Some(layout) = mode.ggml_block_layout(&shape) else {
            return Err(GraphError::InvalidQuantizedConstant {
                mode,
                shape,
                message: String::from(
                    "shape must be non-scalar with a block-aligned last dimension",
                ),
            });
        };
        let bytes = bytes.into();
        if bytes.len() != layout.byte_len() {
            return Err(GraphError::InvalidQuantizedConstant {
                mode,
                shape,
                message: format!(
                    "expected {} bytes from the block layout, got {}",
                    layout.byte_len(),
                    bytes.len()
                ),
            });
        }
        let spec = TensorSpec::new(shape, DType::F32, self.device());
        Ok(self.register(
            LazyOp::Constant,
            OpKind::Constant {
                data: TensorData::QuantizedBlocks(QuantizedTensorData::new(mode, layout, bytes)),
            },
            Vec::new(),
            spec,
        ))
    }

    /// Adds two tensors.
    pub fn add(&mut self, left: &Tensor, right: &Tensor) -> Result<Tensor, GraphError> {
        self.binary_tensor_op(left, right, LazyOp::Add, OpKind::Add)
    }

    /// Multiplies two tensors elementwise.
    pub fn mul(&mut self, left: &Tensor, right: &Tensor) -> Result<Tensor, GraphError> {
        self.binary_tensor_op(left, right, LazyOp::Mul, OpKind::Mul)
    }

    /// Matrix multiply for rank-2 tensors.
    pub fn matmul(&mut self, left: &Tensor, right: &Tensor) -> Result<Tensor, GraphError> {
        let left_shape = left.spec().shape();
        let right_shape = right.spec().shape();
        let valid = left_shape.rank() == 2
            && right_shape.rank() == 2
            && left_shape.dims()[1] == right_shape.dims()[0];
        if !valid {
            return Err(GraphError::InvalidMatmulShapes {
                left: left_shape.clone(),
                right: right_shape.clone(),
            });
        }
        let output_shape = Shape::new(vec![left_shape.dims()[0], right_shape.dims()[1]]);
        let spec = TensorSpec::new(
            output_shape,
            left.spec().dtype(),
            left.spec().device().clone(),
        );
        Ok(self.register(
            LazyOp::Matmul,
            OpKind::Matmul,
            vec![left.id(), right.id()],
            spec,
        ))
    }

    /// Reshapes a tensor without changing the element count.
    pub fn reshape(&mut self, input: &Tensor, new_shape: Shape) -> Result<Tensor, GraphError> {
        if input.spec().shape().element_count() != new_shape.element_count() {
            return Err(GraphError::InvalidReshape {
                from: input.spec().shape().clone(),
                to: new_shape,
            });
        }
        let spec = input.spec().with_shape(new_shape);
        Ok(self.register(LazyOp::Reshape, OpKind::Reshape, vec![input.id()], spec))
    }

    /// Reorders axes using a logical view.
    pub fn permute(&mut self, input: &Tensor, axes: Vec<usize>) -> Result<Tensor, GraphError> {
        let Some(layout) = input.spec().layout().permuted(&axes) else {
            return Err(GraphError::InvalidPermute {
                shape: input.spec().shape().clone(),
                axes,
            });
        };
        Ok(self.register(
            LazyOp::Permute { axes: axes.clone() },
            OpKind::Permute { axes },
            vec![input.id()],
            input.spec().with_layout(layout),
        ))
    }

    /// Returns a narrowed tensor view.
    pub fn slice(
        &mut self,
        input: &Tensor,
        axis: usize,
        start: usize,
        end: usize,
    ) -> Result<Tensor, GraphError> {
        let Some(layout) = input.spec().layout().sliced(axis, start, end) else {
            return Err(GraphError::InvalidSlice {
                shape: input.spec().shape().clone(),
                axis,
                start,
                end,
            });
        };
        Ok(self.register(
            LazyOp::Slice { axis, start, end },
            OpKind::Slice { axis, start, end },
            vec![input.id()],
            input.spec().with_layout(layout),
        ))
    }

    /// Returns a view that removes one axis by selecting a single index.
    pub fn select(
        &mut self,
        input: &Tensor,
        axis: usize,
        index: usize,
    ) -> Result<Tensor, GraphError> {
        let Some(layout) = input.spec().layout().selected(axis, index) else {
            return Err(GraphError::InvalidSelect {
                shape: input.spec().shape().clone(),
                axis,
                index,
            });
        };
        Ok(self.register(
            LazyOp::Select { axis, index },
            OpKind::Select { axis, index },
            vec![input.id()],
            input.spec().with_layout(layout),
        ))
    }

    /// Concatenates tensors along a single axis.
    pub fn concat(&mut self, inputs: &[Tensor], axis: usize) -> Result<Tensor, GraphError> {
        let Some(first) = inputs.first() else {
            return Err(GraphError::InvalidConcat {
                axis,
                shapes: Vec::new(),
            });
        };
        let rank = first.spec().shape().rank();
        if axis >= rank {
            return Err(GraphError::InvalidConcat {
                axis,
                shapes: inputs
                    .iter()
                    .map(|tensor| tensor.spec().shape().clone())
                    .collect(),
            });
        }

        let dtype = first.spec().dtype();
        let device = first.spec().device().clone();
        let mut dims = first.spec().shape().dims().to_vec();
        let mut shapes = Vec::with_capacity(inputs.len());
        for tensor in inputs {
            let shape = tensor.spec().shape();
            shapes.push(shape.clone());
            if tensor.spec().dtype() != dtype
                || shape.rank() != rank
                || shape
                    .dims()
                    .iter()
                    .enumerate()
                    .any(|(index, dim)| index != axis && *dim != dims[index])
            {
                return Err(GraphError::InvalidConcat { axis, shapes });
            }
        }

        dims[axis] = inputs
            .iter()
            .map(|tensor| tensor.spec().shape().dims()[axis])
            .sum();
        let spec = TensorSpec::new(Shape::new(dims), dtype, device);
        Ok(self.register(
            LazyOp::Concat { axis },
            OpKind::Concat { axis },
            inputs.iter().map(Tensor::id).collect(),
            spec,
        ))
    }

    /// Expands a tensor view through broadcast semantics.
    pub fn expand(&mut self, input: &Tensor, shape: Shape) -> Result<Tensor, GraphError> {
        let Some(layout) = input.spec().layout().expanded(&shape) else {
            return Err(GraphError::InvalidExpand {
                from: input.spec().shape().clone(),
                to: shape,
            });
        };
        Ok(self.register(
            LazyOp::Expand {
                shape: shape.clone(),
            },
            OpKind::Expand { shape },
            vec![input.id()],
            input.spec().with_layout(layout),
        ))
    }

    /// Reduces a tensor to a scalar sum.
    pub fn reduce_sum(&mut self, input: &Tensor) -> Tensor {
        let spec = TensorSpec::new(
            Shape::scalar(),
            input.spec().dtype(),
            input.spec().device().clone(),
        );
        self.register(
            LazyOp::ReduceSum { axis: None },
            OpKind::ReduceSum { axis: None },
            vec![input.id()],
            spec,
        )
    }

    /// Reduces a tensor along a single axis.
    pub fn reduce_sum_axis(&mut self, input: &Tensor, axis: usize) -> Result<Tensor, GraphError> {
        let Some(output_shape) = input.spec().shape().without_axis(axis) else {
            return Err(GraphError::InvalidReduceAxis {
                shape: input.spec().shape().clone(),
                axis,
            });
        };
        let spec = TensorSpec::new(
            output_shape,
            input.spec().dtype(),
            input.spec().device().clone(),
        );
        Ok(self.register(
            LazyOp::ReduceSum { axis: Some(axis) },
            OpKind::ReduceSum { axis: Some(axis) },
            vec![input.id()],
            spec,
        ))
    }

    /// Applies RMS normalization over the last dimension.
    pub fn rms_norm(
        &mut self,
        input: &Tensor,
        weight: &Tensor,
        epsilon: f32,
    ) -> Result<Tensor, GraphError> {
        ensure_matching_context("rms_norm", &[input, weight])?;
        let Some(&last_dim) = input.spec().shape().dims().last() else {
            return Err(extension_error(
                "rms_norm",
                "input must have at least one dimension",
            ));
        };
        if weight.spec().shape().dims() != [last_dim] {
            return Err(extension_error(
                "rms_norm",
                format!(
                    "weight shape {} must match input last dimension {last_dim}",
                    weight.spec().shape()
                ),
            ));
        }
        let spec = TensorSpec::new(
            input.spec().shape().clone(),
            input.spec().dtype(),
            input.spec().device().clone(),
        );
        Ok(self.register_backend_extension(
            BackendExtensionOp::RmsNorm {
                epsilon: psionic_core::StableF32::from_f32(epsilon),
            },
            vec![input.id(), weight.id()],
            spec,
        ))
    }

    /// Applies layer normalization over the last dimension.
    pub fn layer_norm(
        &mut self,
        input: &Tensor,
        weight: &Tensor,
        bias: &Tensor,
        epsilon: f32,
    ) -> Result<Tensor, GraphError> {
        ensure_matching_context("layer_norm", &[input, weight, bias])?;
        let Some(&last_dim) = input.spec().shape().dims().last() else {
            return Err(extension_error(
                "layer_norm",
                "input must have at least one dimension",
            ));
        };
        if weight.spec().shape().dims() != [last_dim] {
            return Err(extension_error(
                "layer_norm",
                format!(
                    "weight shape {} must match input last dimension {last_dim}",
                    weight.spec().shape()
                ),
            ));
        }
        if bias.spec().shape().dims() != [last_dim] {
            return Err(extension_error(
                "layer_norm",
                format!(
                    "bias shape {} must match input last dimension {last_dim}",
                    bias.spec().shape()
                ),
            ));
        }
        let spec = TensorSpec::new(
            input.spec().shape().clone(),
            input.spec().dtype(),
            input.spec().device().clone(),
        );
        Ok(self.register_backend_extension(
            BackendExtensionOp::LayerNorm {
                epsilon: psionic_core::StableF32::from_f32(epsilon),
            },
            vec![input.id(), weight.id(), bias.id()],
            spec,
        ))
    }

    /// Applies RoPE over a rank-4 `[batch, heads, seq, dim]` tensor.
    pub fn rope(
        &mut self,
        input: &Tensor,
        cos: &Tensor,
        sin: &Tensor,
        interleaved: bool,
    ) -> Result<Tensor, GraphError> {
        ensure_matching_context("rotary_embedding", &[input, cos, sin])?;
        let input_dims = input.spec().shape().dims();
        if input_dims.len() != 4 || input_dims[3] == 0 || !input_dims[3].is_multiple_of(2) {
            return Err(extension_error(
                "rotary_embedding",
                format!(
                    "input shape {} must be rank-4 with an even last dimension",
                    input.spec().shape()
                ),
            ));
        }
        let seq_len = input_dims[2];
        let half_dim = input_dims[3] / 2;
        let cos_dims = cos.spec().shape().dims();
        let sin_dims = sin.spec().shape().dims();
        if cos_dims != sin_dims {
            return Err(extension_error(
                "rotary_embedding",
                format!(
                    "cos shape {} must match sin shape {}",
                    cos.spec().shape(),
                    sin.spec().shape()
                ),
            ));
        }
        let valid = matches!(cos_dims, [s, d] if *s == seq_len && *d == half_dim)
            || matches!(cos_dims, [b, s, d] if *b == input_dims[0] && *s == seq_len && *d == half_dim);
        if !valid {
            return Err(extension_error(
                "rotary_embedding",
                format!(
                    "cos/sin shape {} must be [{seq_len}, {half_dim}] or [{}, {seq_len}, {half_dim}]",
                    cos.spec().shape(),
                    input_dims[0]
                ),
            ));
        }
        let spec = TensorSpec::new(
            input.spec().shape().clone(),
            input.spec().dtype(),
            input.spec().device().clone(),
        );
        Ok(self.register_backend_extension(
            BackendExtensionOp::RotaryEmbedding { interleaved },
            vec![input.id(), cos.id(), sin.id()],
            spec,
        ))
    }

    /// Applies scaled dot-product attention over rank-4 `[batch, heads, seq, dim]` tensors.
    pub fn scaled_dot_product_attention(
        &mut self,
        query: &Tensor,
        key: &Tensor,
        value: &Tensor,
        scale: f32,
        causal: bool,
    ) -> Result<Tensor, GraphError> {
        ensure_matching_context("scaled_dot_product_attention", &[query, key, value])?;
        let query_dims = query.spec().shape().dims();
        let key_dims = key.spec().shape().dims();
        let value_dims = value.spec().shape().dims();
        let valid = query_dims.len() == 4
            && key_dims.len() == 4
            && value_dims.len() == 4
            && query_dims[0] == key_dims[0]
            && query_dims[0] == value_dims[0]
            && query_dims[1] == key_dims[1]
            && query_dims[1] == value_dims[1]
            && key_dims[2] == value_dims[2]
            && query_dims[3] == key_dims[3];
        if !valid {
            return Err(extension_error(
                "scaled_dot_product_attention",
                format!(
                    "query/key/value shapes {} / {} / {} are incompatible",
                    query.spec().shape(),
                    key.spec().shape(),
                    value.spec().shape()
                ),
            ));
        }
        let spec = TensorSpec::new(
            Shape::new(vec![
                query_dims[0],
                query_dims[1],
                query_dims[2],
                value_dims[3],
            ]),
            query.spec().dtype(),
            query.spec().device().clone(),
        );
        Ok(self.register_backend_extension(
            BackendExtensionOp::ScaledDotProductAttention {
                scale: psionic_core::StableF32::from_f32(scale),
                causal,
            },
            vec![query.id(), key.id(), value.id()],
            spec,
        ))
    }

    /// Registers a matmul that is eligible for a quantized-GEMM specialization.
    pub fn quantized_matmul(
        &mut self,
        left: &Tensor,
        right: &Tensor,
        rhs_mode: QuantizationMode,
    ) -> Result<Tensor, GraphError> {
        if rhs_mode == QuantizationMode::None {
            return Err(extension_error(
                "quantized_matmul",
                "rhs quantization mode must be non-dense",
            ));
        }
        ensure_matching_context("quantized_matmul", &[left, right])?;
        let left_shape = left.spec().shape();
        let right_shape = right.spec().shape();
        let valid = left_shape.rank() == 2
            && right_shape.rank() == 2
            && left_shape.dims()[1] == right_shape.dims()[1];
        if !valid {
            return Err(extension_error(
                "quantized_matmul",
                format!("invalid matmul shapes: left={left_shape} right={right_shape}"),
            ));
        }
        let spec = TensorSpec::new(
            Shape::new(vec![left_shape.dims()[0], right_shape.dims()[0]]),
            left.spec().dtype(),
            left.spec().device().clone(),
        );
        Ok(self.register_backend_extension(
            BackendExtensionOp::QuantizedMatmul { rhs_mode },
            vec![left.id(), right.id()],
            spec,
        ))
    }

    /// Finishes the graph with the provided outputs.
    #[must_use]
    pub fn finish(self, outputs: Vec<Tensor>) -> Graph {
        Graph {
            nodes: self.nodes,
            outputs: outputs.into_iter().map(|tensor| tensor.id()).collect(),
        }
    }

    fn binary_tensor_op(
        &mut self,
        left: &Tensor,
        right: &Tensor,
        lazy_op: LazyOp,
        op: OpKind,
    ) -> Result<Tensor, GraphError> {
        if left.spec().shape() != right.spec().shape() {
            return Err(GraphError::BinaryShapeMismatch {
                left: left.spec().shape().clone(),
                right: right.spec().shape().clone(),
            });
        }
        if left.spec().dtype() != right.spec().dtype() {
            return Err(GraphError::BinaryDTypeMismatch {
                left: left.spec().dtype(),
                right: right.spec().dtype(),
            });
        }

        let spec = TensorSpec::new(
            left.spec().shape().clone(),
            left.spec().dtype(),
            left.spec().device().clone(),
        );
        Ok(self.register(lazy_op, op, vec![left.id(), right.id()], spec))
    }

    fn register_backend_extension(
        &mut self,
        op: BackendExtensionOp,
        inputs: Vec<TensorId>,
        spec: TensorSpec,
    ) -> Tensor {
        self.register(
            LazyOp::BackendExtension { op: op.clone() },
            OpKind::BackendExtension { op },
            inputs,
            spec,
        )
    }

    fn register(
        &mut self,
        lazy_op: LazyOp,
        op: OpKind,
        inputs: Vec<TensorId>,
        spec: TensorSpec,
    ) -> Tensor {
        let id = TensorId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        let tensor = Tensor::new(id, spec, lazy_op);
        self.nodes.push(Node {
            tensor: tensor.clone(),
            inputs,
            op,
        });
        tensor
    }

    fn device(&self) -> Device {
        self.device.clone().unwrap_or_else(Device::cpu)
    }
}

fn ensure_matching_context(op: &str, tensors: &[&Tensor]) -> Result<(), GraphError> {
    let Some(first) = tensors.first() else {
        return Ok(());
    };
    let first_dtype = first.spec().dtype();
    let first_device = first.spec().device();
    if let Some(tensor) = tensors.iter().skip(1).find(|tensor| {
        tensor.spec().dtype() != first_dtype || tensor.spec().device() != first_device
    }) {
        return Err(extension_error(
            op,
            format!(
                "all inputs must share dtype/device; expected {:?} on {}, actual {:?} on {}",
                first_dtype,
                first_device,
                tensor.spec().dtype(),
                tensor.spec().device()
            ),
        ));
    }
    Ok(())
}

fn extension_error(op: &str, message: impl Into<String>) -> GraphError {
    GraphError::InvalidBackendExtension {
        op: String::from(op),
        message: message.into(),
    }
}

fn format_spec(spec: &TensorSpec) -> String {
    format!(
        "shape={} strides={:?} offset={} dtype={:?} device={}",
        spec.shape(),
        spec.layout().strides(),
        spec.layout().offset(),
        spec.dtype(),
        spec.device()
    )
}

fn format_lazy_op(op: &LazyOp) -> String {
    match op {
        LazyOp::Input { name } => format!("input:{name}"),
        LazyOp::Constant => String::from("constant"),
        LazyOp::Add => String::from("add"),
        LazyOp::Mul => String::from("mul"),
        LazyOp::Matmul => String::from("matmul"),
        LazyOp::Reshape => String::from("reshape"),
        LazyOp::Permute { axes } => format!("permute:axes={}", format_axes(axes)),
        LazyOp::Slice { axis, start, end } => {
            format!("slice:axis={axis},start={start},end={end}")
        }
        LazyOp::Select { axis, index } => format!("select:axis={axis},index={index}"),
        LazyOp::Concat { axis } => format!("concat:axis={axis}"),
        LazyOp::Expand { shape } => format!("expand:shape={shape}"),
        LazyOp::ReduceSum { axis } => format_reduce_axis(*axis),
        LazyOp::BackendExtension { op } => format_backend_extension_payload(op),
    }
}

fn format_constant_payload(op: &OpKind) -> String {
    match op {
        OpKind::Constant { data } => format_tensor_data(data),
        _ => String::new(),
    }
}

fn format_execution_payload(op: &ExecutionOp) -> String {
    match op {
        ExecutionOp::Constant { data } => format_tensor_data(data),
        ExecutionOp::Input { name } => format!("input:{name}"),
        ExecutionOp::Permute { axes } => format!("axes={}", format_axes(axes)),
        ExecutionOp::Slice { axis, start, end } => {
            format!("axis={axis},start={start},end={end}")
        }
        ExecutionOp::Select { axis, index } => format!("axis={axis},index={index}"),
        ExecutionOp::Concat { axis } => format!("axis={axis}"),
        ExecutionOp::Expand { shape } => format!("shape={shape}"),
        ExecutionOp::ReduceSum { axis } => format_reduce_axis(*axis),
        ExecutionOp::BackendExtension { op } => format_backend_extension_payload(op),
        _ => String::new(),
    }
}

fn format_backend_extension_payload(op: &BackendExtensionOp) -> String {
    match op {
        BackendExtensionOp::RmsNorm { epsilon } => {
            format!("epsilon_bits={:08x}", epsilon.0)
        }
        BackendExtensionOp::LayerNorm { epsilon } => {
            format!("epsilon_bits={:08x}", epsilon.0)
        }
        BackendExtensionOp::RotaryEmbedding { interleaved } => {
            format!("interleaved={interleaved}")
        }
        BackendExtensionOp::ScaledDotProductAttention { scale, causal } => {
            format!("scale_bits={:08x},causal={causal}", scale.0)
        }
        BackendExtensionOp::QuantizedMatmul { rhs_mode } => {
            format!("rhs_mode={rhs_mode:?}")
        }
    }
}

fn format_tensor_data(data: &TensorData) -> String {
    match data {
        TensorData::F32(values) => {
            let bits = values
                .iter()
                .map(|value| format!("{:08x}", value.to_bits()))
                .collect::<Vec<_>>()
                .join(",");
            format!("f32:{bits}")
        }
        TensorData::QuantizedBlocks(data) => format!(
            "quantized:{:?}:blocks={}:bytes_per_block={}:bytes={}",
            data.mode,
            data.layout.block_count,
            data.layout.bytes_per_block,
            hex::encode(&data.bytes)
        ),
    }
}

fn format_axes(axes: &[usize]) -> String {
    axes.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn format_reduce_axis(axis: Option<usize>) -> String {
    match axis {
        Some(axis) => format!("axis={axis}"),
        None => String::from("axis=all"),
    }
}

fn digest_lines(lines: Vec<String>) -> String {
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use psionic_core::{Device, QuantizationMode};

    use super::{DType, GraphBuilder, Shape};

    #[test]
    fn graph_digest_is_stable_for_identical_layout_graphs() {
        let digest_a = build_sample_graph();
        let digest_b = build_sample_graph();
        assert!(digest_a.is_ok());
        assert!(digest_b.is_ok());
        let Ok(digest_a) = digest_a.map(|graph| graph.stable_digest()) else {
            return;
        };
        let Ok(digest_b) = digest_b.map(|graph| graph.stable_digest()) else {
            return;
        };
        assert_eq!(digest_a, digest_b);
    }

    #[test]
    fn graph_debug_lists_layout_ops_and_parameters() {
        let graph = build_sample_graph();
        assert!(graph.is_ok());
        let Ok(graph) = graph else {
            return;
        };
        let debug = graph.stable_debug();
        assert!(debug.contains("permute:axes=1,0"));
        assert!(debug.contains("concat:axis=0"));
        assert!(debug.contains("reduce_sum"));
        assert!(debug.contains("axis=0"));
    }

    #[test]
    fn builder_tracks_expected_view_shapes() {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![2, 3]), DType::F32);
        let permuted = builder.permute(&input, vec![1, 0]);
        assert!(permuted.is_ok());
        let Ok(permuted) = permuted else {
            return;
        };
        let sliced = builder.slice(&permuted, 0, 1, 3);
        assert!(sliced.is_ok());
        let Ok(sliced) = sliced else {
            return;
        };
        let selected = builder.select(&sliced, 1, 0);
        assert!(selected.is_ok());
        let Ok(selected) = selected else {
            return;
        };
        let expanded = builder.expand(&selected, Shape::new(vec![2, 2]));
        assert!(expanded.is_ok());
        let Ok(expanded) = expanded else {
            return;
        };

        assert_eq!(permuted.spec().shape().dims(), &[3, 2]);
        assert_eq!(sliced.spec().shape().dims(), &[2, 2]);
        assert_eq!(selected.spec().shape().dims(), &[2]);
        assert_eq!(expanded.spec().shape().dims(), &[2, 2]);
    }

    #[test]
    fn builder_tracks_backend_extension_ops_and_payloads() {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![1, 2, 2, 32]), DType::F32);
        let input_norm_weight = builder.constant_f32(Shape::new(vec![32]), vec![1.0; 32]);
        assert!(input_norm_weight.is_ok());
        let Ok(input_norm_weight) = input_norm_weight else {
            return;
        };
        let output_norm_weight = builder.constant_f32(Shape::new(vec![4]), vec![1.0; 4]);
        assert!(output_norm_weight.is_ok());
        let Ok(output_norm_weight) = output_norm_weight else {
            return;
        };
        let output_norm_bias = builder.constant_f32(Shape::new(vec![4]), vec![0.0; 4]);
        assert!(output_norm_bias.is_ok());
        let Ok(output_norm_bias) = output_norm_bias else {
            return;
        };
        let cos = builder.constant_f32(Shape::new(vec![2, 16]), vec![1.0; 32]);
        assert!(cos.is_ok());
        let Ok(cos) = cos else {
            return;
        };
        let sin = builder.constant_f32(Shape::new(vec![2, 16]), vec![0.0; 32]);
        assert!(sin.is_ok());
        let Ok(sin) = sin else {
            return;
        };
        let qk_weights = builder.constant_quantized_blocks(
            Shape::new(vec![4, 32]),
            QuantizationMode::GgmlQ4_0,
            vec![0x88_u8; 72],
        );
        assert!(qk_weights.is_ok());
        let Ok(qk_weights) = qk_weights else {
            return;
        };
        let normed = builder.rms_norm(&input, &input_norm_weight, 1e-5);
        assert!(normed.is_ok());
        let Ok(normed) = normed else {
            return;
        };
        let roped = builder.rope(&normed, &cos, &sin, true);
        assert!(roped.is_ok());
        let Ok(roped) = roped else {
            return;
        };
        let attended = builder.scaled_dot_product_attention(&roped, &roped, &roped, 0.5, true);
        assert!(attended.is_ok());
        let Ok(attended) = attended else {
            return;
        };
        let flattened = builder.reshape(&attended, Shape::new(vec![4, 32]));
        assert!(flattened.is_ok());
        let Ok(flattened) = flattened else {
            return;
        };
        let projected =
            builder.quantized_matmul(&flattened, &qk_weights, QuantizationMode::GgmlQ4_0);
        assert!(projected.is_ok());
        let Ok(projected) = projected else {
            return;
        };
        let shifted = builder.layer_norm(&projected, &output_norm_weight, &output_norm_bias, 1e-5);
        assert!(shifted.is_ok());
        let Ok(shifted) = shifted else {
            return;
        };
        let graph = builder.finish(vec![shifted]);

        let debug = graph.stable_debug();
        assert!(debug.contains("rms_norm"));
        assert!(debug.contains("rotary_embedding"));
        assert!(debug.contains("scaled_dot_product_attention"));
        assert!(debug.contains("quantized_matmul"));
        assert!(debug.contains("quantized:GgmlQ4_0"));
        assert!(debug.contains("layer_norm"));
    }

    #[test]
    fn quantized_matmul_uses_rowwise_rhs_orientation() {
        let mut builder = GraphBuilder::new(Device::cpu());
        let left = builder.input("left", Shape::new(vec![2, 32]), DType::F32);
        let rhs = builder.constant_quantized_blocks(
            Shape::new(vec![3, 32]),
            QuantizationMode::GgmlQ8_0,
            vec![0_u8; 102],
        );
        assert!(rhs.is_ok());
        let Ok(rhs) = rhs else {
            return;
        };

        let output = builder.quantized_matmul(&left, &rhs, QuantizationMode::GgmlQ8_0);
        assert!(output.is_ok());
        let Ok(output) = output else {
            return;
        };
        assert_eq!(output.spec().shape().dims(), &[2, 3]);
    }

    #[test]
    fn rope_rejects_invalid_cos_shape() {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![1, 2, 2, 4]), DType::F32);
        let cos = builder.constant_f32(Shape::new(vec![3, 2]), vec![1.0f32; 6]);
        assert!(cos.is_ok());
        let Ok(cos) = cos else {
            return;
        };
        let sin = builder.constant_f32(Shape::new(vec![3, 2]), vec![0.0f32; 6]);
        assert!(sin.is_ok());
        let Ok(sin) = sin else {
            return;
        };

        let error = builder.rope(&input, &cos, &sin, false);
        assert!(matches!(
            error,
            Err(super::GraphError::InvalidBackendExtension { .. })
        ));
    }

    fn build_sample_graph() -> Result<super::Graph, super::GraphError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![2, 2]), DType::F32);
        let permuted = builder.permute(&input, vec![1, 0])?;
        let first_row = builder.slice(&permuted, 0, 0, 1)?;
        let expanded = builder.expand(&first_row, Shape::new(vec![2, 2]))?;
        let concatenated = builder.concat(&[input.clone(), expanded], 0)?;
        let reduced = builder.reduce_sum_axis(&concatenated, 0)?;
        Ok(builder.finish(vec![reduced]))
    }
}
