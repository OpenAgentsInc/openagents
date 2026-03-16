//! Canonical graph and plan representation for Psionic.

mod autodiff;

use std::collections::{BTreeMap, BTreeSet};

use psionic_core::{
    BackendExtensionOp, DType, Device, LazyOp, PsionicRefusal, PsionicRefusalCode,
    PsionicRefusalScope, QuantizationMode, QuantizedTensorData, Shape, Tensor, TensorData,
    TensorId, TensorSpec,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use autodiff::*;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "canonical graph, autodiff, and execution plan types";

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
    /// An operator was invoked with the wrong number of inputs.
    #[error("invalid operator arity for `{op}`: expected {expected}, actual {actual}")]
    InvalidOperatorArity {
        /// Stable operator label.
        op: String,
        /// Human-readable expected arity description.
        expected: String,
        /// Actual input count.
        actual: usize,
    },
    /// An operator did not receive valid inputs for meta execution or schema
    /// validation.
    #[error("invalid operator inputs for `{op}`: {message}")]
    InvalidOperatorInputs {
        /// Stable operator label.
        op: String,
        /// Human-readable validation failure.
        message: String,
    },
    /// A target capability profile does not support one operator that appeared
    /// during fake or meta execution.
    #[error("unsupported operator capability for `{op}`: {message}")]
    UnsupportedOperatorCapability {
        /// Stable operator label.
        op: String,
        /// Human-readable capability failure.
        message: String,
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
    /// A declared step output spec diverged from operator meta execution.
    #[error("meta execution mismatch for `{op}`: declared={expected} actual={actual}")]
    MetaExecutionMismatch {
        /// Stable operator label.
        op: String,
        /// Declared step spec.
        expected: String,
        /// Meta-derived step spec.
        actual: String,
    },
}

impl GraphError {
    /// Returns the canonical refusal when this graph error belongs to one
    /// explicit unsupported or incompatibility family.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        match self {
            Self::ConstantLengthMismatch { .. } => Some(PsionicRefusal::new(
                PsionicRefusalCode::SerializationIncompatibility,
                PsionicRefusalScope::Graph,
                self.to_string(),
            )),
            Self::InvalidQuantizedConstant { mode, .. } => Some(
                PsionicRefusal::new(
                    PsionicRefusalCode::SerializationIncompatibility,
                    PsionicRefusalScope::Graph,
                    self.to_string(),
                )
                .with_subject(format!("{mode:?}")),
            ),
            Self::InvalidOperatorArity { op, .. }
            | Self::InvalidOperatorInputs { op, .. }
            | Self::InvalidBackendExtension { op, .. } => Some(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedOp,
                    PsionicRefusalScope::Graph,
                    self.to_string(),
                )
                .with_subject(op.clone()),
            ),
            Self::UnsupportedOperatorCapability { op, .. } => Some(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedBackendCapability,
                    PsionicRefusalScope::Graph,
                    self.to_string(),
                )
                .with_subject(op.clone()),
            ),
            Self::BinaryShapeMismatch { .. }
            | Self::InvalidMatmulShapes { .. }
            | Self::InvalidReshape { .. }
            | Self::InvalidPermute { .. }
            | Self::InvalidSlice { .. }
            | Self::InvalidSelect { .. }
            | Self::InvalidConcat { .. }
            | Self::InvalidExpand { .. }
            | Self::InvalidReduceAxis { .. } => Some(PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedLayout,
                PsionicRefusalScope::Graph,
                self.to_string(),
            )),
            Self::BinaryDTypeMismatch { .. } | Self::MetaExecutionMismatch { .. } => None,
        }
    }
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
    /// Gradient-stopping identity.
    Detach,
    /// Binary add with broadcast and dtype-promotion semantics.
    Add,
    /// Binary multiply with broadcast and dtype-promotion semantics.
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
            Self::Detach => "detach",
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

    /// Returns the built-in operator schema for this graph op.
    #[must_use]
    pub fn schema(&self) -> &'static OperatorSchema {
        OperatorRegistry::builtin().schema_for_op_kind(self)
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
        digest_lines(self.stable_signature_lines())
    }

    /// Returns a stable debug string for snapshots and diagnostics.
    #[must_use]
    pub fn stable_debug(&self) -> String {
        self.stable_signature_lines().join("\n")
    }

    /// Returns the canonical line-oriented signature used for replay fixtures.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
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
    /// Gradient-stopping identity.
    Detach,
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
            Self::Detach => "detach",
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
            OpKind::Detach => Self::Detach,
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

    /// Returns the built-in operator schema for this execution op.
    #[must_use]
    pub fn schema(&self) -> &'static OperatorSchema {
        OperatorRegistry::builtin().schema_for_execution_op(self)
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
        digest_lines(self.stable_signature_lines())
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

    /// Returns the canonical line-oriented signature used for replay fixtures.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
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
        lines
    }
}

/// Stable schema version for the built-in Psionic operator registry.
pub const BUILTIN_OPERATOR_SCHEMA_VERSION: u16 = 1;

/// Stable implementation family for one operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperatorImplementationKind {
    /// Source op whose runtime shape is declared directly by the surrounding
    /// graph or plan.
    SchemaOnly,
    /// Framework-owned behavior realized without a backend kernel family.
    Composite,
    /// Backend-owned kernel or loop implementation.
    BackendKernel,
}

/// Declared input count contract for one operator schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperatorArity {
    /// Fixed number of inputs.
    Fixed(u8),
    /// Variadic inputs with a minimum count.
    Variadic { min_inputs: u8 },
}

impl OperatorArity {
    fn accepts(self, actual: usize) -> bool {
        match self {
            Self::Fixed(expected) => actual == usize::from(expected),
            Self::Variadic { min_inputs } => actual >= usize::from(min_inputs),
        }
    }

    fn describe(self) -> String {
        match self {
            Self::Fixed(expected) => expected.to_string(),
            Self::Variadic { min_inputs } => format!("{min_inputs}+"),
        }
    }
}

/// Meta execution posture for one operator schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperatorMetaExecutionKind {
    /// Output spec is carried explicitly by the surrounding graph or plan.
    DeclaredOutput,
    /// Output spec is computed from operator attributes and input specs.
    BuiltinInference,
}

/// One declared operator schema in the built-in registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperatorSchema {
    /// Stable operator label.
    pub name: &'static str,
    /// Stable schema version.
    pub schema_version: u16,
    /// Input-count contract.
    pub arity: OperatorArity,
    /// Runtime implementation family.
    pub implementation: OperatorImplementationKind,
    /// Meta execution posture.
    pub meta_execution: OperatorMetaExecutionKind,
}

/// Shape-only tensor record emitted by fake or meta execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaTensor {
    /// Logical tensor spec proven by meta execution.
    pub spec: TensorSpec,
}

/// One step traced during fake or meta execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaExecutionStep {
    /// Output tensor ID.
    pub output: TensorId,
    /// Stable operator label.
    pub op: String,
    /// Runtime implementation family.
    pub implementation: OperatorImplementationKind,
    /// Meta-derived tensor spec.
    pub spec: TensorSpec,
}

/// Report emitted by fake or meta execution over a graph or plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaExecutionReport {
    /// Step-by-step meta execution trace.
    pub steps: Vec<MetaExecutionStep>,
    /// Final output tensors that were requested by the graph or plan.
    pub outputs: BTreeMap<TensorId, MetaTensor>,
}

impl MetaExecutionReport {
    /// Returns one output tensor by ID.
    #[must_use]
    pub fn output(&self, id: TensorId) -> Option<&MetaTensor> {
        self.outputs.get(&id)
    }
}

/// Capability profile used to answer whether a fake or meta execution target
/// claims support for one backend-kernel surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaCapabilityProfile {
    /// Stable backend-kernel labels declared as supported.
    pub supported_backend_kernels: BTreeSet<String>,
}

impl MetaCapabilityProfile {
    /// Creates an empty capability profile.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            supported_backend_kernels: BTreeSet::new(),
        }
    }

    /// Creates a profile that supports all built-in backend kernels.
    #[must_use]
    pub fn all_builtin() -> Self {
        let supported_backend_kernels = OperatorRegistry::builtin()
            .all()
            .iter()
            .filter(|schema| schema.implementation == OperatorImplementationKind::BackendKernel)
            .map(|schema| schema.name.to_string())
            .collect();
        Self {
            supported_backend_kernels,
        }
    }

    /// Replaces the declared backend-kernel capability set.
    #[must_use]
    pub fn with_supported_backend_kernels(
        mut self,
        labels: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.supported_backend_kernels = labels.into_iter().map(Into::into).collect();
        self
    }

    fn supports(&self, schema: &OperatorSchema) -> bool {
        match schema.implementation {
            OperatorImplementationKind::SchemaOnly | OperatorImplementationKind::Composite => true,
            OperatorImplementationKind::BackendKernel => {
                self.supported_backend_kernels.contains(schema.name)
            }
        }
    }
}

impl OperatorSchema {
    const fn new(
        name: &'static str,
        arity: OperatorArity,
        implementation: OperatorImplementationKind,
        meta_execution: OperatorMetaExecutionKind,
    ) -> Self {
        Self {
            name,
            schema_version: BUILTIN_OPERATOR_SCHEMA_VERSION,
            arity,
            implementation,
            meta_execution,
        }
    }
}

const BUILTIN_OPERATOR_SCHEMAS: &[OperatorSchema] = &[
    OperatorSchema::new(
        "input",
        OperatorArity::Fixed(0),
        OperatorImplementationKind::SchemaOnly,
        OperatorMetaExecutionKind::DeclaredOutput,
    ),
    OperatorSchema::new(
        "constant",
        OperatorArity::Fixed(0),
        OperatorImplementationKind::SchemaOnly,
        OperatorMetaExecutionKind::DeclaredOutput,
    ),
    OperatorSchema::new(
        "detach",
        OperatorArity::Fixed(1),
        OperatorImplementationKind::Composite,
        OperatorMetaExecutionKind::BuiltinInference,
    ),
    OperatorSchema::new(
        "add",
        OperatorArity::Fixed(2),
        OperatorImplementationKind::BackendKernel,
        OperatorMetaExecutionKind::BuiltinInference,
    ),
    OperatorSchema::new(
        "mul",
        OperatorArity::Fixed(2),
        OperatorImplementationKind::BackendKernel,
        OperatorMetaExecutionKind::BuiltinInference,
    ),
    OperatorSchema::new(
        "matmul",
        OperatorArity::Fixed(2),
        OperatorImplementationKind::BackendKernel,
        OperatorMetaExecutionKind::BuiltinInference,
    ),
    OperatorSchema::new(
        "reshape",
        OperatorArity::Fixed(1),
        OperatorImplementationKind::Composite,
        OperatorMetaExecutionKind::BuiltinInference,
    ),
    OperatorSchema::new(
        "permute",
        OperatorArity::Fixed(1),
        OperatorImplementationKind::Composite,
        OperatorMetaExecutionKind::BuiltinInference,
    ),
    OperatorSchema::new(
        "slice",
        OperatorArity::Fixed(1),
        OperatorImplementationKind::Composite,
        OperatorMetaExecutionKind::BuiltinInference,
    ),
    OperatorSchema::new(
        "select",
        OperatorArity::Fixed(1),
        OperatorImplementationKind::Composite,
        OperatorMetaExecutionKind::BuiltinInference,
    ),
    OperatorSchema::new(
        "concat",
        OperatorArity::Variadic { min_inputs: 1 },
        OperatorImplementationKind::BackendKernel,
        OperatorMetaExecutionKind::BuiltinInference,
    ),
    OperatorSchema::new(
        "expand",
        OperatorArity::Fixed(1),
        OperatorImplementationKind::Composite,
        OperatorMetaExecutionKind::BuiltinInference,
    ),
    OperatorSchema::new(
        "reduce_sum",
        OperatorArity::Fixed(1),
        OperatorImplementationKind::BackendKernel,
        OperatorMetaExecutionKind::BuiltinInference,
    ),
    OperatorSchema::new(
        "rms_norm",
        OperatorArity::Fixed(2),
        OperatorImplementationKind::BackendKernel,
        OperatorMetaExecutionKind::BuiltinInference,
    ),
    OperatorSchema::new(
        "layer_norm",
        OperatorArity::Fixed(3),
        OperatorImplementationKind::BackendKernel,
        OperatorMetaExecutionKind::BuiltinInference,
    ),
    OperatorSchema::new(
        "rotary_embedding",
        OperatorArity::Fixed(3),
        OperatorImplementationKind::BackendKernel,
        OperatorMetaExecutionKind::BuiltinInference,
    ),
    OperatorSchema::new(
        "scaled_dot_product_attention",
        OperatorArity::Fixed(3),
        OperatorImplementationKind::BackendKernel,
        OperatorMetaExecutionKind::BuiltinInference,
    ),
    OperatorSchema::new(
        "quantized_matmul",
        OperatorArity::Fixed(2),
        OperatorImplementationKind::BackendKernel,
        OperatorMetaExecutionKind::BuiltinInference,
    ),
];

/// Built-in operator registry for the current compact Psionic framework-core
/// surface.
#[derive(Clone, Copy, Debug, Default)]
pub struct OperatorRegistry;

impl OperatorRegistry {
    /// Returns the built-in operator registry.
    #[must_use]
    pub const fn builtin() -> Self {
        Self
    }

    /// Returns all built-in operator schemas in stable order.
    #[must_use]
    pub fn all(&self) -> &'static [OperatorSchema] {
        BUILTIN_OPERATOR_SCHEMAS
    }

    /// Returns one operator schema by stable name.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&'static OperatorSchema> {
        self.all().iter().find(|schema| schema.name == name)
    }

    /// Returns the schema for a graph op.
    #[must_use]
    pub fn schema_for_op_kind(&self, op: &OpKind) -> &'static OperatorSchema {
        self.find(op.label())
            .expect("all built-in graph ops must have registered schemas")
    }

    /// Returns the schema for an execution op.
    #[must_use]
    pub fn schema_for_execution_op(&self, op: &ExecutionOp) -> &'static OperatorSchema {
        self.find(op.label())
            .expect("all built-in execution ops must have registered schemas")
    }

    /// Computes or validates the output spec for one execution op.
    pub fn meta_execute(
        &self,
        op: &ExecutionOp,
        inputs: &[TensorSpec],
        declared_output: Option<&TensorSpec>,
    ) -> Result<TensorSpec, GraphError> {
        let schema = self.schema_for_execution_op(op);
        if !schema.arity.accepts(inputs.len()) {
            return Err(GraphError::InvalidOperatorArity {
                op: op.label().to_string(),
                expected: schema.arity.describe(),
                actual: inputs.len(),
            });
        }

        match schema.meta_execution {
            OperatorMetaExecutionKind::DeclaredOutput => {
                declared_output
                    .cloned()
                    .ok_or_else(|| GraphError::InvalidOperatorInputs {
                        op: op.label().to_string(),
                        message: String::from("declared output spec is required for source ops"),
                    })
            }
            OperatorMetaExecutionKind::BuiltinInference => {
                meta_execute_builtin(op, inputs, declared_output)
            }
        }
    }

    /// Validates that one execution plan's declared step specs match built-in
    /// meta execution.
    pub fn validate_execution_plan(&self, plan: &ExecutionPlan) -> Result<(), GraphError> {
        let mut known_specs = BTreeMap::<TensorId, TensorSpec>::new();
        for step in &plan.steps {
            let input_specs = step
                .inputs
                .iter()
                .map(|tensor_id| {
                    known_specs.get(tensor_id).cloned().ok_or_else(|| {
                        GraphError::InvalidOperatorInputs {
                            op: step.op.label().to_string(),
                            message: format!("missing input tensor {tensor_id}"),
                        }
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let actual = self.meta_execute(&step.op, input_specs.as_slice(), Some(&step.spec))?;
            if actual != step.spec {
                return Err(GraphError::MetaExecutionMismatch {
                    op: step.op.label().to_string(),
                    expected: format_spec(&step.spec),
                    actual: format_spec(&actual),
                });
            }
            known_specs.insert(step.output, step.spec.clone());
        }
        Ok(())
    }

    /// Runs fake or meta execution over one graph without material tensor data.
    pub fn meta_execute_graph(
        &self,
        graph: &Graph,
        capabilities: Option<&MetaCapabilityProfile>,
    ) -> Result<MetaExecutionReport, GraphError> {
        let steps = graph
            .nodes()
            .iter()
            .map(|node| MetaExecutionInputStep {
                output: node.tensor().id(),
                op: ExecutionOp::from_op_kind(node.op()),
                declared_output: node.tensor().spec().clone(),
                inputs: node.inputs().to_vec(),
            })
            .collect::<Vec<_>>();
        self.meta_execute_steps(steps.as_slice(), graph.outputs(), capabilities)
    }

    /// Runs fake or meta execution over one execution plan without material
    /// tensor data.
    pub fn meta_execute_plan(
        &self,
        plan: &ExecutionPlan,
        capabilities: Option<&MetaCapabilityProfile>,
    ) -> Result<MetaExecutionReport, GraphError> {
        let steps = plan
            .steps
            .iter()
            .map(|step| MetaExecutionInputStep {
                output: step.output,
                op: step.op.clone(),
                declared_output: step.spec.clone(),
                inputs: step.inputs.clone(),
            })
            .collect::<Vec<_>>();
        self.meta_execute_steps(steps.as_slice(), plan.outputs.as_slice(), capabilities)
    }

    fn meta_execute_steps(
        &self,
        steps: &[MetaExecutionInputStep],
        outputs: &[TensorId],
        capabilities: Option<&MetaCapabilityProfile>,
    ) -> Result<MetaExecutionReport, GraphError> {
        let mut tensors = BTreeMap::<TensorId, MetaTensor>::new();
        let mut trace = Vec::with_capacity(steps.len());

        for step in steps {
            let schema = self.schema_for_execution_op(&step.op);
            if let Some(profile) = capabilities {
                if !profile.supports(schema) {
                    return Err(GraphError::UnsupportedOperatorCapability {
                        op: schema.name.to_string(),
                        message: format!(
                            "meta capability profile does not declare backend kernel `{}`",
                            schema.name
                        ),
                    });
                }
            }

            let input_specs = step
                .inputs
                .iter()
                .map(|tensor_id| {
                    tensors
                        .get(tensor_id)
                        .map(|tensor| tensor.spec.clone())
                        .ok_or_else(|| GraphError::InvalidOperatorInputs {
                            op: step.op.label().to_string(),
                            message: format!("missing input tensor {tensor_id}"),
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let spec = self.meta_execute(
                &step.op,
                input_specs.as_slice(),
                Some(&step.declared_output),
            )?;
            tensors.insert(step.output, MetaTensor { spec: spec.clone() });
            trace.push(MetaExecutionStep {
                output: step.output,
                op: schema.name.to_string(),
                implementation: schema.implementation,
                spec,
            });
        }

        let outputs = outputs
            .iter()
            .map(|tensor_id| {
                tensors
                    .get(tensor_id)
                    .cloned()
                    .map(|tensor| (*tensor_id, tensor))
                    .ok_or_else(|| GraphError::InvalidOperatorInputs {
                        op: String::from("meta_execute"),
                        message: format!("missing output tensor {tensor_id}"),
                    })
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;

        Ok(MetaExecutionReport {
            steps: trace,
            outputs,
        })
    }
}

#[derive(Clone, Debug)]
struct MetaExecutionInputStep {
    output: TensorId,
    op: ExecutionOp,
    declared_output: TensorSpec,
    inputs: Vec<TensorId>,
}

fn meta_execute_builtin(
    op: &ExecutionOp,
    inputs: &[TensorSpec],
    declared_output: Option<&TensorSpec>,
) -> Result<TensorSpec, GraphError> {
    match op {
        ExecutionOp::Input { .. } | ExecutionOp::Constant { .. } => declared_output
            .cloned()
            .ok_or_else(|| GraphError::InvalidOperatorInputs {
                op: op.label().to_string(),
                message: String::from("declared output spec is required for source ops"),
            }),
        ExecutionOp::Detach => Ok(inputs[0].clone()),
        ExecutionOp::Add | ExecutionOp::Mul => meta_execute_binary(&inputs[0], &inputs[1]),
        ExecutionOp::Matmul => meta_execute_matmul(&inputs[0], &inputs[1]),
        ExecutionOp::Reshape => meta_execute_reshape(&inputs[0], declared_output),
        ExecutionOp::Permute { axes } => meta_execute_permute(&inputs[0], axes),
        ExecutionOp::Slice { axis, start, end } => {
            meta_execute_slice(&inputs[0], *axis, *start, *end)
        }
        ExecutionOp::Select { axis, index } => meta_execute_select(&inputs[0], *axis, *index),
        ExecutionOp::Concat { axis } => meta_execute_concat(inputs, *axis),
        ExecutionOp::Expand { shape } => meta_execute_expand(&inputs[0], shape),
        ExecutionOp::ReduceSum { axis } => meta_execute_reduce_sum(&inputs[0], *axis),
        ExecutionOp::BackendExtension { op } => meta_execute_backend_extension(op, inputs),
    }
}

fn meta_execute_binary(left: &TensorSpec, right: &TensorSpec) -> Result<TensorSpec, GraphError> {
    let Some(shape) = left.shape().broadcast_with(right.shape()) else {
        return Err(GraphError::BinaryShapeMismatch {
            left: left.shape().clone(),
            right: right.shape().clone(),
        });
    };
    let Some(dtype) = left.dtype().promote_binary(right.dtype()) else {
        return Err(GraphError::BinaryDTypeMismatch {
            left: left.dtype(),
            right: right.dtype(),
        });
    };
    Ok(TensorSpec::new(shape, dtype, left.device().clone()))
}

fn meta_execute_matmul(left: &TensorSpec, right: &TensorSpec) -> Result<TensorSpec, GraphError> {
    let left_shape = left.shape();
    let right_shape = right.shape();
    let valid = left_shape.rank() == 2
        && right_shape.rank() == 2
        && left_shape.dims()[1] == right_shape.dims()[0];
    if !valid {
        return Err(GraphError::InvalidMatmulShapes {
            left: left_shape.clone(),
            right: right_shape.clone(),
        });
    }
    Ok(TensorSpec::new(
        Shape::new(vec![left_shape.dims()[0], right_shape.dims()[1]]),
        left.dtype(),
        left.device().clone(),
    ))
}

fn meta_execute_reshape(
    input: &TensorSpec,
    declared_output: Option<&TensorSpec>,
) -> Result<TensorSpec, GraphError> {
    let Some(declared_output) = declared_output else {
        return Err(GraphError::InvalidOperatorInputs {
            op: String::from("reshape"),
            message: String::from("declared output spec is required for reshape"),
        });
    };
    if input.shape().element_count() != declared_output.shape().element_count() {
        return Err(GraphError::InvalidReshape {
            from: input.shape().clone(),
            to: declared_output.shape().clone(),
        });
    }
    Ok(TensorSpec::new(
        declared_output.shape().clone(),
        input.dtype(),
        input.device().clone(),
    ))
}

fn meta_execute_permute(input: &TensorSpec, axes: &[usize]) -> Result<TensorSpec, GraphError> {
    let Some(layout) = input.layout().permuted(axes) else {
        return Err(GraphError::InvalidPermute {
            shape: input.shape().clone(),
            axes: axes.to_vec(),
        });
    };
    Ok(input.with_layout(layout))
}

fn meta_execute_slice(
    input: &TensorSpec,
    axis: usize,
    start: usize,
    end: usize,
) -> Result<TensorSpec, GraphError> {
    let Some(layout) = input.layout().sliced(axis, start, end) else {
        return Err(GraphError::InvalidSlice {
            shape: input.shape().clone(),
            axis,
            start,
            end,
        });
    };
    Ok(input.with_layout(layout))
}

fn meta_execute_select(
    input: &TensorSpec,
    axis: usize,
    index: usize,
) -> Result<TensorSpec, GraphError> {
    let Some(layout) = input.layout().selected(axis, index) else {
        return Err(GraphError::InvalidSelect {
            shape: input.shape().clone(),
            axis,
            index,
        });
    };
    Ok(input.with_layout(layout))
}

fn meta_execute_concat(inputs: &[TensorSpec], axis: usize) -> Result<TensorSpec, GraphError> {
    let Some(first) = inputs.first() else {
        return Err(GraphError::InvalidConcat {
            axis,
            shapes: Vec::new(),
        });
    };
    let rank = first.shape().rank();
    if axis >= rank {
        return Err(GraphError::InvalidConcat {
            axis,
            shapes: inputs.iter().map(|spec| spec.shape().clone()).collect(),
        });
    }

    let mut dims = first.shape().dims().to_vec();
    let mut shapes = Vec::with_capacity(inputs.len());
    for spec in inputs {
        let shape = spec.shape();
        shapes.push(shape.clone());
        if spec.dtype() != first.dtype()
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

    dims[axis] = inputs.iter().map(|spec| spec.shape().dims()[axis]).sum();
    Ok(TensorSpec::new(
        Shape::new(dims),
        first.dtype(),
        first.device().clone(),
    ))
}

fn meta_execute_expand(input: &TensorSpec, shape: &Shape) -> Result<TensorSpec, GraphError> {
    let Some(layout) = input.layout().expanded(shape) else {
        return Err(GraphError::InvalidExpand {
            from: input.shape().clone(),
            to: shape.clone(),
        });
    };
    Ok(input.with_layout(layout))
}

fn meta_execute_reduce_sum(
    input: &TensorSpec,
    axis: Option<usize>,
) -> Result<TensorSpec, GraphError> {
    let shape = match axis {
        None => Shape::scalar(),
        Some(axis) => input
            .shape()
            .without_axis(axis)
            .ok_or(GraphError::InvalidReduceAxis {
                shape: input.shape().clone(),
                axis,
            })?,
    };
    Ok(TensorSpec::new(
        shape,
        input.dtype(),
        input.device().clone(),
    ))
}

fn meta_execute_backend_extension(
    op: &BackendExtensionOp,
    inputs: &[TensorSpec],
) -> Result<TensorSpec, GraphError> {
    match op {
        BackendExtensionOp::RmsNorm { .. } => {
            ensure_matching_specs("rms_norm", inputs)?;
            let input = &inputs[0];
            let weight = &inputs[1];
            let Some(&last_dim) = input.shape().dims().last() else {
                return Err(extension_error(
                    "rms_norm",
                    "input must have at least one dimension",
                ));
            };
            if weight.shape().dims() != [last_dim] {
                return Err(extension_error(
                    "rms_norm",
                    format!(
                        "weight shape {} must match input last dimension {last_dim}",
                        weight.shape()
                    ),
                ));
            }
            Ok(TensorSpec::new(
                input.shape().clone(),
                input.dtype(),
                input.device().clone(),
            ))
        }
        BackendExtensionOp::LayerNorm { .. } => {
            ensure_matching_specs("layer_norm", inputs)?;
            let input = &inputs[0];
            let weight = &inputs[1];
            let bias = &inputs[2];
            let Some(&last_dim) = input.shape().dims().last() else {
                return Err(extension_error(
                    "layer_norm",
                    "input must have at least one dimension",
                ));
            };
            if weight.shape().dims() != [last_dim] {
                return Err(extension_error(
                    "layer_norm",
                    format!(
                        "weight shape {} must match input last dimension {last_dim}",
                        weight.shape()
                    ),
                ));
            }
            if bias.shape().dims() != [last_dim] {
                return Err(extension_error(
                    "layer_norm",
                    format!(
                        "bias shape {} must match input last dimension {last_dim}",
                        bias.shape()
                    ),
                ));
            }
            Ok(TensorSpec::new(
                input.shape().clone(),
                input.dtype(),
                input.device().clone(),
            ))
        }
        BackendExtensionOp::RotaryEmbedding { .. } => {
            ensure_matching_specs("rotary_embedding", inputs)?;
            let input = &inputs[0];
            let cos = &inputs[1];
            let sin = &inputs[2];
            let input_dims = input.shape().dims();
            if input_dims.len() != 4 || input_dims[3] == 0 || !input_dims[3].is_multiple_of(2) {
                return Err(extension_error(
                    "rotary_embedding",
                    format!(
                        "input shape {} must be rank-4 with an even last dimension",
                        input.shape()
                    ),
                ));
            }
            if cos.shape().dims() != sin.shape().dims() {
                return Err(extension_error(
                    "rotary_embedding",
                    format!(
                        "cos shape {} must match sin shape {}",
                        cos.shape(),
                        sin.shape()
                    ),
                ));
            }
            let seq_len = input_dims[2];
            let half_dim = input_dims[3] / 2;
            let cos_dims = cos.shape().dims();
            let valid = matches!(cos_dims, [s, d] if *s == seq_len && *d == half_dim)
                || matches!(cos_dims, [b, s, d] if *b == input_dims[0] && *s == seq_len && *d == half_dim);
            if !valid {
                return Err(extension_error(
                    "rotary_embedding",
                    format!(
                        "cos/sin shape {} must be [{seq_len}, {half_dim}] or [{}, {seq_len}, {half_dim}]",
                        cos.shape(),
                        input_dims[0]
                    ),
                ));
            }
            Ok(TensorSpec::new(
                input.shape().clone(),
                input.dtype(),
                input.device().clone(),
            ))
        }
        BackendExtensionOp::ScaledDotProductAttention { .. } => {
            ensure_matching_specs("scaled_dot_product_attention", inputs)?;
            let query = &inputs[0];
            let key = &inputs[1];
            let value = &inputs[2];
            let query_dims = query.shape().dims();
            let key_dims = key.shape().dims();
            let value_dims = value.shape().dims();
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
                        query.shape(),
                        key.shape(),
                        value.shape()
                    ),
                ));
            }
            Ok(TensorSpec::new(
                Shape::new(vec![
                    query_dims[0],
                    query_dims[1],
                    query_dims[2],
                    value_dims[3],
                ]),
                query.dtype(),
                query.device().clone(),
            ))
        }
        BackendExtensionOp::QuantizedMatmul { rhs_mode } => {
            if *rhs_mode == QuantizationMode::None {
                return Err(extension_error(
                    "quantized_matmul",
                    "rhs quantization mode must be non-dense",
                ));
            }
            ensure_matching_specs("quantized_matmul", inputs)?;
            let left = &inputs[0];
            let right = &inputs[1];
            let left_shape = left.shape();
            let right_shape = right.shape();
            let valid = left_shape.rank() == 2
                && right_shape.rank() == 2
                && left_shape.dims()[1] == right_shape.dims()[1];
            if !valid {
                return Err(extension_error(
                    "quantized_matmul",
                    format!("invalid matmul shapes: left={left_shape} right={right_shape}"),
                ));
            }
            Ok(TensorSpec::new(
                Shape::new(vec![left_shape.dims()[0], right_shape.dims()[0]]),
                left.dtype(),
                left.device().clone(),
            ))
        }
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

    /// Adds two tensors using broadcast-compatible shape semantics.
    pub fn add(&mut self, left: &Tensor, right: &Tensor) -> Result<Tensor, GraphError> {
        self.binary_tensor_op(left, right, LazyOp::Add, OpKind::Add)
    }

    /// Adds a gradient-stopping identity node.
    pub fn detach(&mut self, input: &Tensor) -> Tensor {
        self.register(
            LazyOp::Detach,
            OpKind::Detach,
            vec![input.id()],
            input.spec().clone(),
        )
    }

    /// Multiplies two tensors elementwise using broadcast-compatible shape
    /// semantics.
    pub fn mul(&mut self, left: &Tensor, right: &Tensor) -> Result<Tensor, GraphError> {
        self.binary_tensor_op(left, right, LazyOp::Mul, OpKind::Mul)
    }

    /// Matrix multiply for rank-2 tensors.
    pub fn matmul(&mut self, left: &Tensor, right: &Tensor) -> Result<Tensor, GraphError> {
        let spec = self.meta_spec(&ExecutionOp::Matmul, &[left, right], None)?;
        Ok(self.register(
            LazyOp::Matmul,
            OpKind::Matmul,
            vec![left.id(), right.id()],
            spec,
        ))
    }

    /// Reshapes a tensor without changing the element count.
    pub fn reshape(&mut self, input: &Tensor, new_shape: Shape) -> Result<Tensor, GraphError> {
        let declared_output = TensorSpec::new(
            new_shape.clone(),
            input.spec().dtype(),
            input.spec().device().clone(),
        );
        let spec = self.meta_spec(&ExecutionOp::Reshape, &[input], Some(&declared_output))?;
        Ok(self.register(LazyOp::Reshape, OpKind::Reshape, vec![input.id()], spec))
    }

    /// Reorders axes using a logical view.
    pub fn permute(&mut self, input: &Tensor, axes: Vec<usize>) -> Result<Tensor, GraphError> {
        let spec = self.meta_spec(&ExecutionOp::Permute { axes: axes.clone() }, &[input], None)?;
        Ok(self.register(
            LazyOp::Permute { axes: axes.clone() },
            OpKind::Permute { axes },
            vec![input.id()],
            spec,
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
        let spec = self.meta_spec(&ExecutionOp::Slice { axis, start, end }, &[input], None)?;
        Ok(self.register(
            LazyOp::Slice { axis, start, end },
            OpKind::Slice { axis, start, end },
            vec![input.id()],
            spec,
        ))
    }

    /// Returns a view that removes one axis by selecting a single index.
    pub fn select(
        &mut self,
        input: &Tensor,
        axis: usize,
        index: usize,
    ) -> Result<Tensor, GraphError> {
        let spec = self.meta_spec(&ExecutionOp::Select { axis, index }, &[input], None)?;
        Ok(self.register(
            LazyOp::Select { axis, index },
            OpKind::Select { axis, index },
            vec![input.id()],
            spec,
        ))
    }

    /// Concatenates tensors along a single axis.
    pub fn concat(&mut self, inputs: &[Tensor], axis: usize) -> Result<Tensor, GraphError> {
        let refs = inputs.iter().collect::<Vec<_>>();
        let spec = self.meta_spec(&ExecutionOp::Concat { axis }, refs.as_slice(), None)?;
        Ok(self.register(
            LazyOp::Concat { axis },
            OpKind::Concat { axis },
            inputs.iter().map(Tensor::id).collect(),
            spec,
        ))
    }

    /// Expands a tensor view through broadcast semantics.
    pub fn expand(&mut self, input: &Tensor, shape: Shape) -> Result<Tensor, GraphError> {
        let spec = self.meta_spec(
            &ExecutionOp::Expand {
                shape: shape.clone(),
            },
            &[input],
            None,
        )?;
        Ok(self.register(
            LazyOp::Expand {
                shape: shape.clone(),
            },
            OpKind::Expand { shape },
            vec![input.id()],
            spec,
        ))
    }

    /// Reduces a tensor to a scalar sum.
    pub fn reduce_sum(&mut self, input: &Tensor) -> Tensor {
        let spec = self
            .meta_spec(&ExecutionOp::ReduceSum { axis: None }, &[input], None)
            .expect("reduce_sum meta execution should accept one input");
        self.register(
            LazyOp::ReduceSum { axis: None },
            OpKind::ReduceSum { axis: None },
            vec![input.id()],
            spec,
        )
    }

    /// Reduces a tensor along a single axis.
    pub fn reduce_sum_axis(&mut self, input: &Tensor, axis: usize) -> Result<Tensor, GraphError> {
        let spec = self.meta_spec(&ExecutionOp::ReduceSum { axis: Some(axis) }, &[input], None)?;
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
        let op = BackendExtensionOp::RmsNorm {
            epsilon: psionic_core::StableF32::from_f32(epsilon),
        };
        let spec = self.meta_spec(
            &ExecutionOp::BackendExtension { op: op.clone() },
            &[input, weight],
            None,
        )?;
        Ok(self.register_backend_extension(op, vec![input.id(), weight.id()], spec))
    }

    /// Applies layer normalization over the last dimension.
    pub fn layer_norm(
        &mut self,
        input: &Tensor,
        weight: &Tensor,
        bias: &Tensor,
        epsilon: f32,
    ) -> Result<Tensor, GraphError> {
        let op = BackendExtensionOp::LayerNorm {
            epsilon: psionic_core::StableF32::from_f32(epsilon),
        };
        let spec = self.meta_spec(
            &ExecutionOp::BackendExtension { op: op.clone() },
            &[input, weight, bias],
            None,
        )?;
        Ok(self.register_backend_extension(op, vec![input.id(), weight.id(), bias.id()], spec))
    }

    /// Applies RoPE over a rank-4 `[batch, heads, seq, dim]` tensor.
    pub fn rope(
        &mut self,
        input: &Tensor,
        cos: &Tensor,
        sin: &Tensor,
        interleaved: bool,
    ) -> Result<Tensor, GraphError> {
        let op = BackendExtensionOp::RotaryEmbedding { interleaved };
        let spec = self.meta_spec(
            &ExecutionOp::BackendExtension { op: op.clone() },
            &[input, cos, sin],
            None,
        )?;
        Ok(self.register_backend_extension(op, vec![input.id(), cos.id(), sin.id()], spec))
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
        let op = BackendExtensionOp::ScaledDotProductAttention {
            scale: psionic_core::StableF32::from_f32(scale),
            causal,
        };
        let spec = self.meta_spec(
            &ExecutionOp::BackendExtension { op: op.clone() },
            &[query, key, value],
            None,
        )?;
        Ok(self.register_backend_extension(op, vec![query.id(), key.id(), value.id()], spec))
    }

    /// Registers a matmul that is eligible for a quantized-GEMM specialization.
    pub fn quantized_matmul(
        &mut self,
        left: &Tensor,
        right: &Tensor,
        rhs_mode: QuantizationMode,
    ) -> Result<Tensor, GraphError> {
        let op = BackendExtensionOp::QuantizedMatmul { rhs_mode };
        let spec = self.meta_spec(
            &ExecutionOp::BackendExtension { op: op.clone() },
            &[left, right],
            None,
        )?;
        Ok(self.register_backend_extension(op, vec![left.id(), right.id()], spec))
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
        let execution_op = ExecutionOp::from_op_kind(&op);
        let output_spec = self.meta_spec(&execution_op, &[left, right], None)?;
        let output_shape = output_spec.shape().clone();

        let left = if left.spec().shape() != &output_shape {
            self.expand(left, output_shape.clone())?
        } else {
            left.clone()
        };
        let right = if right.spec().shape() != &output_shape {
            self.expand(right, output_shape.clone())?
        } else {
            right.clone()
        };

        Ok(self.register(lazy_op, op, vec![left.id(), right.id()], output_spec))
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

    fn meta_spec(
        &self,
        op: &ExecutionOp,
        inputs: &[&Tensor],
        declared_output: Option<&TensorSpec>,
    ) -> Result<TensorSpec, GraphError> {
        let specs = inputs
            .iter()
            .map(|tensor| tensor.spec().clone())
            .collect::<Vec<_>>();
        OperatorRegistry::builtin().meta_execute(op, specs.as_slice(), declared_output)
    }
}

fn ensure_matching_specs(op: &str, specs: &[TensorSpec]) -> Result<(), GraphError> {
    let Some(first) = specs.first() else {
        return Ok(());
    };
    let first_dtype = first.dtype();
    let first_device = first.device();
    if let Some(spec) = specs
        .iter()
        .skip(1)
        .find(|spec| spec.dtype() != first_dtype || spec.device() != first_device)
    {
        return Err(extension_error(
            op,
            format!(
                "all inputs must share dtype/device; expected {:?} on {}, actual {:?} on {}",
                first_dtype,
                first_device,
                spec.dtype(),
                spec.device()
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
        LazyOp::Detach => String::from("detach"),
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
        ExecutionOp::Detach => String::from("detach"),
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
    use psionic_core::{Device, PsionicRefusalCode, PsionicRefusalScope, QuantizationMode};

    use super::{
        DType, ExecutionOp, ExecutionPlan, ExecutionStep, GraphBuilder, GraphError,
        MetaCapabilityProfile, OperatorImplementationKind, OperatorMetaExecutionKind,
        OperatorRegistry, Shape, TensorSpec,
    };

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
        assert!(expanded.spec().layout().is_broadcast_view());
    }

    #[test]
    fn binary_ops_broadcast_inputs_through_explicit_expand_views() {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![2, 3]), DType::F32);
        let row = builder.select(&input, 0, 0);
        assert!(row.is_ok());
        let Ok(row) = row else {
            return;
        };

        let shifted = builder.add(&input, &row);
        assert!(shifted.is_ok());
        let Ok(shifted) = shifted else {
            return;
        };
        let graph = builder.finish(vec![shifted.clone()]);
        let debug = graph.stable_debug();

        assert_eq!(shifted.spec().shape().dims(), &[2, 3]);
        assert_eq!(shifted.spec().dtype(), DType::F32);
        assert!(debug.contains("select:axis=0,index=0"));
        assert!(debug.contains("expand:shape=[2, 3]"));
        assert!(debug.contains("add"));
    }

    #[test]
    fn binary_ops_promote_mixed_dtypes() {
        let mut builder = GraphBuilder::new(Device::cpu());
        let half = builder.input("half", Shape::new(vec![2, 2]), DType::F16);
        let brain = builder.input("brain", Shape::new(vec![2, 2]), DType::BF16);

        let mixed = builder.mul(&half, &brain);
        assert!(mixed.is_ok());
        let Ok(mixed) = mixed else {
            return;
        };

        assert_eq!(mixed.spec().dtype(), DType::F32);
        assert_eq!(mixed.spec().shape().dims(), &[2, 2]);
    }

    #[test]
    fn binary_ops_refuse_incompatible_broadcast_shapes() {
        let mut builder = GraphBuilder::new(Device::cpu());
        let left = builder.input("left", Shape::new(vec![2, 3]), DType::F32);
        let right = builder.input("right", Shape::new(vec![2, 2]), DType::F32);

        let error = builder.add(&left, &right);
        assert!(matches!(
            error,
            Err(super::GraphError::BinaryShapeMismatch { .. })
        ));
    }

    #[test]
    fn graph_error_refusal_taxonomy_maps_layout_capability_and_serialization_boundaries() {
        let layout = GraphError::InvalidExpand {
            from: Shape::new(vec![2, 2]),
            to: Shape::new(vec![3, 2]),
        }
        .refusal();
        assert!(layout.is_some());
        let Some(layout) = layout else {
            return;
        };
        assert_eq!(layout.code, PsionicRefusalCode::UnsupportedLayout);
        assert_eq!(layout.scope, PsionicRefusalScope::Graph);

        let capability = GraphError::UnsupportedOperatorCapability {
            op: String::from("quantized_matmul"),
            message: String::from("backend profile does not expose grouped-block kernels"),
        }
        .refusal();
        assert!(capability.is_some());
        let Some(capability) = capability else {
            return;
        };
        assert_eq!(
            capability.code,
            PsionicRefusalCode::UnsupportedBackendCapability
        );
        assert_eq!(capability.subject.as_deref(), Some("quantized_matmul"));

        let serialization = GraphError::InvalidQuantizedConstant {
            mode: QuantizationMode::GgmlQ4_0,
            shape: Shape::new(vec![1, 32]),
            message: String::from("block payload length mismatch"),
        }
        .refusal();
        assert!(serialization.is_some());
        let Some(serialization) = serialization else {
            return;
        };
        assert_eq!(
            serialization.code,
            PsionicRefusalCode::SerializationIncompatibility
        );
        assert_eq!(serialization.scope, PsionicRefusalScope::Graph);
    }

    #[test]
    fn builtin_operator_registry_exposes_kernel_composite_and_meta_surfaces() {
        let registry = OperatorRegistry::builtin();

        let add = registry.find("add");
        assert!(add.is_some());
        let Some(add) = add else {
            return;
        };
        assert_eq!(
            add.implementation,
            OperatorImplementationKind::BackendKernel
        );
        assert_eq!(
            add.meta_execution,
            OperatorMetaExecutionKind::BuiltinInference
        );

        let expand = registry.find("expand");
        assert!(expand.is_some());
        let Some(expand) = expand else {
            return;
        };
        assert_eq!(expand.implementation, OperatorImplementationKind::Composite);

        let input = registry.find("input");
        assert!(input.is_some());
        let Some(input) = input else {
            return;
        };
        assert_eq!(input.implementation, OperatorImplementationKind::SchemaOnly);
        assert_eq!(
            input.meta_execution,
            OperatorMetaExecutionKind::DeclaredOutput
        );

        let rope = registry.find("rotary_embedding");
        assert!(rope.is_some());
        let Some(rope) = rope else {
            return;
        };
        assert_eq!(rope.arity, super::OperatorArity::Fixed(3));
    }

    #[test]
    fn operator_registry_refuses_wrong_arity_during_meta_execution() {
        let registry = OperatorRegistry::builtin();
        let spec = TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu());

        let error = registry.meta_execute(&ExecutionOp::Add, &[spec], None);
        assert!(matches!(
            error,
            Err(super::GraphError::InvalidOperatorArity { .. })
        ));
    }

    #[test]
    fn operator_registry_validates_execution_plan_specs() {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![2, 3]), DType::F32);
        let row = builder.select(&input, 0, 0);
        assert!(row.is_ok());
        let Ok(row) = row else {
            return;
        };
        let shifted = builder.add(&input, &row);
        assert!(shifted.is_ok());
        let Ok(shifted) = shifted else {
            return;
        };
        let reduced = builder.reduce_sum_axis(&shifted, 1);
        assert!(reduced.is_ok());
        let Ok(reduced) = reduced else {
            return;
        };
        let graph = builder.finish(vec![reduced]);
        let plan = graph_to_execution_plan(&graph);

        let result = OperatorRegistry::builtin().validate_execution_plan(&plan);
        assert!(
            result.is_ok(),
            "built-in operator registry should validate plan"
        );
    }

    #[test]
    fn meta_executor_runs_graph_without_real_tensor_data() {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![2, 3]), DType::F32);
        let row = builder.select(&input, 0, 0);
        assert!(row.is_ok());
        let Ok(row) = row else {
            return;
        };
        let shifted = builder.add(&input, &row);
        assert!(shifted.is_ok());
        let Ok(shifted) = shifted else {
            return;
        };
        let reduced = builder.reduce_sum_axis(&shifted, 1);
        assert!(reduced.is_ok());
        let Ok(reduced) = reduced else {
            return;
        };
        let graph = builder.finish(vec![reduced.clone()]);

        let report = OperatorRegistry::builtin()
            .meta_execute_graph(&graph, Some(&MetaCapabilityProfile::all_builtin()));
        assert!(report.is_ok());
        let Ok(report) = report else {
            return;
        };
        let output = report.output(reduced.id());
        assert!(output.is_some());
        let Some(output) = output else {
            return;
        };
        assert_eq!(output.spec.shape().dims(), &[2]);
        assert_eq!(output.spec.dtype(), DType::F32);
    }

    #[test]
    fn meta_executor_refuses_missing_backend_kernel_capability() {
        let mut builder = GraphBuilder::new(Device::cpu());
        let left = builder.input("left", Shape::new(vec![2, 2]), DType::F32);
        let right = builder.input("right", Shape::new(vec![2, 2]), DType::F32);
        let product = builder.matmul(&left, &right);
        assert!(product.is_ok());
        let Ok(product) = product else {
            return;
        };
        let graph = builder.finish(vec![product]);
        let capabilities = MetaCapabilityProfile::empty().with_supported_backend_kernels(["add"]);

        let error = OperatorRegistry::builtin().meta_execute_graph(&graph, Some(&capabilities));
        assert!(matches!(
            error,
            Err(super::GraphError::UnsupportedOperatorCapability { .. })
        ));
    }

    #[test]
    fn meta_executor_tracks_same_outputs_for_graph_and_plan() {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![2, 2]), DType::F32);
        let permuted = builder.permute(&input, vec![1, 0]);
        assert!(permuted.is_ok());
        let Ok(permuted) = permuted else {
            return;
        };
        let reduced = builder.reduce_sum_axis(&permuted, 0);
        assert!(reduced.is_ok());
        let Ok(reduced) = reduced else {
            return;
        };
        let graph = builder.finish(vec![reduced.clone()]);
        let plan = graph_to_execution_plan(&graph);
        let registry = OperatorRegistry::builtin();

        let graph_report = registry.meta_execute_graph(&graph, None);
        let plan_report = registry.meta_execute_plan(&plan, None);
        assert!(graph_report.is_ok());
        assert!(plan_report.is_ok());
        let Ok(graph_report) = graph_report else {
            return;
        };
        let Ok(plan_report) = plan_report else {
            return;
        };

        assert_eq!(
            graph_report.output(reduced.id()),
            plan_report.output(reduced.id())
        );
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

    fn graph_to_execution_plan(graph: &super::Graph) -> ExecutionPlan {
        ExecutionPlan {
            graph_digest: graph.stable_digest(),
            steps: graph
                .nodes()
                .iter()
                .map(|node| ExecutionStep {
                    output: node.tensor().id(),
                    op: ExecutionOp::from_op_kind(node.op()),
                    spec: node.tensor().spec().clone(),
                    inputs: node.inputs().to_vec(),
                })
                .collect(),
            outputs: graph.outputs().to_vec(),
        }
    }
}
