use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use psionic_core::{
    BackendExtensionOp, DType, Device, PsionicRefusal, PsionicRefusalCode, PsionicRefusalScope,
    Shape, Tensor, TensorData, TensorId,
};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use thiserror::Error;

use crate::{Graph, GraphBuilder, GraphError, OpKind};

/// Execution-mode posture for autodiff tracking.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutodiffExecutionMode {
    /// Training mode allows gradient tracking when it is enabled.
    Training,
    /// Evaluation mode keeps graph execution explicit but disables gradients.
    Evaluation,
}

/// Gradient-tracking context for graph construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutodiffContext {
    /// High-level execution mode.
    pub execution_mode: AutodiffExecutionMode,
    /// Whether gradient tracking is enabled under the current mode.
    pub gradients_enabled: bool,
}

impl AutodiffContext {
    /// Returns the default training posture with gradients enabled.
    #[must_use]
    pub const fn training() -> Self {
        Self {
            execution_mode: AutodiffExecutionMode::Training,
            gradients_enabled: true,
        }
    }

    /// Returns an evaluation posture with gradients disabled.
    #[must_use]
    pub const fn evaluation() -> Self {
        Self {
            execution_mode: AutodiffExecutionMode::Evaluation,
            gradients_enabled: false,
        }
    }

    /// Returns a copy with an explicit gradient-tracking posture.
    #[must_use]
    pub const fn with_gradients_enabled(mut self, gradients_enabled: bool) -> Self {
        self.gradients_enabled = gradients_enabled;
        self
    }

    /// Returns whether gradients are active for new graph nodes.
    #[must_use]
    pub const fn gradients_active(self) -> bool {
        matches!(self.execution_mode, AutodiffExecutionMode::Training) && self.gradients_enabled
    }
}

/// Autodiff-aware tensor handle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutodiffTensor {
    tensor: Tensor,
    requires_grad: bool,
}

impl AutodiffTensor {
    fn new(tensor: Tensor, requires_grad: bool) -> Self {
        Self {
            tensor,
            requires_grad,
        }
    }

    /// Returns the underlying canonical tensor handle.
    #[must_use]
    pub fn tensor(&self) -> &Tensor {
        &self.tensor
    }

    /// Returns the tensor identifier.
    #[must_use]
    pub const fn id(&self) -> TensorId {
        self.tensor.id()
    }

    /// Returns the tensor specification.
    #[must_use]
    pub fn spec(&self) -> &psionic_core::TensorSpec {
        self.tensor.spec()
    }

    /// Returns whether this tensor is gradient-bearing under the current context.
    #[must_use]
    pub const fn requires_grad(&self) -> bool {
        self.requires_grad
    }
}

/// Typed support classification for reverse-mode autodiff over one graph op.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "support", rename_all = "snake_case")]
pub enum AutodiffGradientSupport {
    /// Reverse-mode semantics are implemented for this op family.
    Implemented,
    /// Reverse-mode semantics are intentionally unsupported for now.
    Unsupported {
        /// Stable reason code for the refusal family.
        reason: AutodiffUnsupportedGradientReason,
    },
}

/// Stable refusal family for unsupported reverse-mode gradients.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutodiffUnsupportedGradientReason {
    /// Backend-extension op families still require dedicated reverse-mode
    /// contracts.
    BackendExtensionFamily,
}

/// Returns the reverse-mode support posture for one graph op.
#[must_use]
pub const fn gradient_support_for_op(op: &OpKind) -> AutodiffGradientSupport {
    match op {
        OpKind::Input { .. }
        | OpKind::Constant { .. }
        | OpKind::Detach
        | OpKind::Add
        | OpKind::Mul
        | OpKind::Matmul
        | OpKind::Reshape
        | OpKind::Cast { .. }
        | OpKind::Permute { .. }
        | OpKind::Slice { .. }
        | OpKind::Select { .. }
        | OpKind::Concat { .. }
        | OpKind::Expand { .. }
        | OpKind::ReduceSum { .. } => AutodiffGradientSupport::Implemented,
        OpKind::BackendExtension { op } => match op {
            BackendExtensionOp::ParameterGolfBankedLinear { .. }
            | BackendExtensionOp::ParameterGolfTokenEmbeddingLookup
            | BackendExtensionOp::ReluSquared
            | BackendExtensionOp::LeakyReluSquared { .. }
            | BackendExtensionOp::Silu
            | BackendExtensionOp::ParameterGolfProjectionLoss { .. }
            | BackendExtensionOp::RmsNorm { .. }
            | BackendExtensionOp::RotaryEmbedding { .. }
            | BackendExtensionOp::ScaledDotProductAttention { .. } => {
                AutodiffGradientSupport::Implemented
            }
            BackendExtensionOp::ParameterGolfProjectionTokenLosses { .. } => {
                AutodiffGradientSupport::Unsupported {
                    reason: AutodiffUnsupportedGradientReason::BackendExtensionFamily,
                }
            }
            _ => AutodiffGradientSupport::Unsupported {
                reason: AutodiffUnsupportedGradientReason::BackendExtensionFamily,
            },
        },
    }
}

/// Typed support classification for public `vmap` over one graph op.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "support", rename_all = "snake_case")]
pub enum VmapSupport {
    /// Public `vmap` semantics are implemented for this op family.
    Implemented,
    /// Public `vmap` semantics are intentionally unsupported for now.
    Unsupported {
        /// Stable reason code for the refusal family.
        reason: VmapUnsupportedReason,
    },
}

/// Stable refusal family for unsupported public `vmap` coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VmapUnsupportedReason {
    /// Dtype-cast ops are intentionally excluded from the first bounded
    /// public `vmap` surface.
    CastFamily,
    /// Backend-extension op families still require dedicated vectorization
    /// contracts.
    BackendExtensionFamily,
}

/// Returns the public `vmap` support posture for one graph op.
#[must_use]
pub const fn vmap_support_for_op(op: &OpKind) -> VmapSupport {
    match op {
        OpKind::Input { .. }
        | OpKind::Constant { .. }
        | OpKind::Detach
        | OpKind::Add
        | OpKind::Mul
        | OpKind::Matmul
        | OpKind::Reshape
        | OpKind::Permute { .. }
        | OpKind::Slice { .. }
        | OpKind::Select { .. }
        | OpKind::Concat { .. }
        | OpKind::Expand { .. }
        | OpKind::ReduceSum { .. } => VmapSupport::Implemented,
        OpKind::Cast { .. } => VmapSupport::Unsupported {
            reason: VmapUnsupportedReason::CastFamily,
        },
        OpKind::BackendExtension { .. } => VmapSupport::Unsupported {
            reason: VmapUnsupportedReason::BackendExtensionFamily,
        },
    }
}

/// Typed error returned by the reference graph evaluator.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ReferenceEvaluationError {
    /// The caller omitted a required graph input.
    #[error("graph input tensor `{tensor_id}` is missing")]
    MissingInput {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One graph reference pointed at a missing tensor.
    #[error("graph tensor `{tensor_id}` is unknown")]
    UnknownTensor {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// The reference path only supports dense `f32` tensors.
    #[error("graph tensor `{tensor_id}` must be dense `f32` while evaluating `{op}`")]
    DenseF32Required {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Operation currently being evaluated.
        op: String,
    },
    /// The reference path only supports `f32` tensor specs.
    #[error(
        "graph tensor `{tensor_id}` uses unsupported dtype `{dtype:?}` while evaluating `{op}`"
    )]
    UnsupportedDType {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Operation currently being evaluated.
        op: String,
        /// Observed dtype.
        dtype: DType,
    },
    /// One tensor payload length mismatched its logical shape.
    #[error(
        "graph tensor `{tensor_id}` payload length mismatch: expected {expected_len}, found {actual_len}"
    )]
    PayloadLengthMismatch {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Expected logical element count.
        expected_len: usize,
        /// Actual payload length.
        actual_len: usize,
    },
    /// The current evaluator intentionally refuses a non-primitive op.
    #[error("graph tensor `{tensor_id}` used unsupported op `{op}` in reference evaluation")]
    UnsupportedOp {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Stable op label.
        op: String,
    },
}

impl ReferenceEvaluationError {
    /// Returns the canonical refusal when the reference path intentionally
    /// refuses one graph family.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        match self {
            Self::UnsupportedDType { tensor_id, .. } => Some(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedBackendCapability,
                    PsionicRefusalScope::Graph,
                    self.to_string(),
                )
                .with_subject(format!("{tensor_id:?}")),
            ),
            Self::UnsupportedOp { tensor_id, op } => Some(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedOp,
                    PsionicRefusalScope::Graph,
                    self.to_string(),
                )
                .with_subject(format!("{tensor_id:?}:{op}")),
            ),
            Self::MissingInput { .. }
            | Self::UnknownTensor { .. }
            | Self::DenseF32Required { .. }
            | Self::PayloadLengthMismatch { .. } => None,
        }
    }
}

/// Typed error returned by the autodiff layer.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AutodiffError {
    /// The requested tensor is not present in the graph.
    #[error("autodiff graph does not contain tensor `{tensor_id}`")]
    UnknownTensor {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// The requested output is not gradient-bearing under the current context.
    #[error("tensor `{tensor_id}` is not gradient-tracked under the current autodiff context")]
    OutputNotTracked {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// The current autodiff reference layer only supports dense `f32` or
    /// dense `bf16` gradients.
    #[error("tensor `{tensor_id}` uses unsupported gradient dtype `{dtype:?}`")]
    UnsupportedGradientDType {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Observed dtype.
        dtype: DType,
    },
    /// One op does not yet expose reverse-mode semantics.
    #[error("tensor `{tensor_id}` used unsupported gradient op `{op}`")]
    UnsupportedGradientOp {
        /// Stable output tensor identifier.
        tensor_id: TensorId,
        /// Stable op label.
        op: String,
    },
    /// The caller requested backward over a non-scalar output without a seed.
    #[error("tensor `{tensor_id}` with shape {shape} requires an explicit upstream seed gradient")]
    NonScalarOutputRequiresSeed {
        /// Stable output tensor identifier.
        tensor_id: TensorId,
        /// Output shape.
        shape: Shape,
    },
    /// The provided upstream seed used an unsupported storage family.
    #[error("seed gradient for tensor `{tensor_id}` must be dense `f32`")]
    SeedDenseF32Required {
        /// Stable output tensor identifier.
        tensor_id: TensorId,
    },
    /// The provided upstream seed length mismatched the output tensor.
    #[error(
        "seed gradient for tensor `{tensor_id}` length mismatch: expected {expected_len}, found {actual_len}"
    )]
    SeedLengthMismatch {
        /// Stable output tensor identifier.
        tensor_id: TensorId,
        /// Expected logical element count.
        expected_len: usize,
        /// Actual payload length.
        actual_len: usize,
    },
    /// One symbolic backward rewrite produced an invalid graph op.
    #[error("autodiff backward graph construction failed: {message}")]
    BackwardGraphConstruction {
        /// Human-readable invariant failure.
        message: String,
    },
    /// One lower-layer reference-evaluation operation failed.
    #[error(transparent)]
    ReferenceEvaluation(#[from] ReferenceEvaluationError),
}

impl AutodiffError {
    /// Returns the canonical refusal when the autodiff layer intentionally
    /// refuses one unsupported gradient family.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        match self {
            Self::UnsupportedGradientDType { tensor_id, .. }
            | Self::UnsupportedGradientOp { tensor_id, .. }
            | Self::SeedDenseF32Required { tensor_id } => Some(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedGradient,
                    PsionicRefusalScope::Autodiff,
                    self.to_string(),
                )
                .with_subject(format!("{tensor_id:?}")),
            ),
            Self::ReferenceEvaluation(error) => error.refusal(),
            Self::UnknownTensor { .. }
            | Self::OutputNotTracked { .. }
            | Self::NonScalarOutputRequiresSeed { .. }
            | Self::SeedLengthMismatch { .. }
            | Self::BackwardGraphConstruction { .. } => None,
        }
    }
}

/// One binding from a primal forward tensor to a gradient-graph input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutodiffPrimalBinding {
    /// Forward-graph tensor whose value must be bound into the backward graph.
    pub primal_tensor: TensorId,
    /// Gradient-graph input tensor that expects that value.
    pub gradient_graph_input: TensorId,
}

/// One binding from a primal tensor to its symbolic gradient output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutodiffGradientTarget {
    /// Forward-graph tensor whose gradient is being exposed.
    pub primal_tensor: TensorId,
    /// Gradient-graph output tensor that materializes that gradient.
    pub gradient_tensor: TensorId,
}

/// Symbolic reverse-mode plan over the canonical IR.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutodiffBackwardPlan {
    /// Symbolic backward graph.
    pub gradient_graph: Graph,
    /// Required primal-value bindings for the backward graph.
    pub primal_bindings: Vec<AutodiffPrimalBinding>,
    /// Input tensor carrying the upstream seed gradient.
    pub seed_input: TensorId,
    /// Gradient outputs exposed by the backward graph.
    pub gradient_targets: Vec<AutodiffGradientTarget>,
}

impl AutodiffBackwardPlan {
    /// Returns the backward-graph output tensor for one primal tensor when present.
    #[must_use]
    pub fn gradient_for(&self, primal_tensor: TensorId) -> Option<TensorId> {
        self.gradient_targets
            .iter()
            .find(|target| target.primal_tensor == primal_tensor)
            .map(|target| target.gradient_tensor)
    }
}

/// Materialized backward result over dense `f32` buffers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutodiffBackwardResult {
    /// Forward values materialized for the primal graph.
    pub forward_values: BTreeMap<TensorId, TensorData>,
    /// Symbolic backward plan used for materialization.
    pub plan: AutodiffBackwardPlan,
    /// Materialized gradients keyed by primal tensor ID.
    pub gradients: BTreeMap<TensorId, TensorData>,
}

impl AutodiffBackwardResult {
    /// Returns the materialized gradient for one primal tensor when present.
    #[must_use]
    pub fn gradient(&self, primal_tensor: TensorId) -> Option<&TensorData> {
        self.gradients.get(&primal_tensor)
    }
}

/// Stable target signature for one reverse-mode transform.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReverseModeTransformSignature {
    /// Output tensor differentiated by the transform.
    pub output: TensorId,
    /// Primal tensors whose cotangents are exposed by the transform.
    pub primal_targets: Vec<TensorId>,
}

/// Typed error returned by the public reverse-mode transform layer.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ReverseModeTransformError {
    /// One requested transform tensor is not present in the graph.
    #[error("reverse-mode transform graph does not contain tensor `{tensor_id}`")]
    UnknownTensor {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One requested primal target is not gradient-bearing.
    #[error("reverse-mode transform target `{tensor_id}` is not gradient-tracked")]
    UntrackedTarget {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One requested target uses a dtype outside the bounded reverse-mode surface.
    #[error(
        "reverse-mode transform target `{tensor_id}` uses unsupported gradient dtype `{dtype:?}`"
    )]
    UnsupportedTargetDType {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Observed dtype.
        dtype: DType,
    },
    /// `grad` and `value_and_grad` currently require a singleton output.
    #[error(
        "reverse-mode transform `{transform}` requires a singleton output; tensor `{tensor_id}` has shape {shape}"
    )]
    NonSingletonOutput {
        /// Stable transform label.
        transform: String,
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Observed output shape.
        shape: Shape,
    },
    /// One forward output value was missing after evaluation.
    #[error("reverse-mode transform output `{tensor_id}` did not materialize a forward value")]
    MissingForwardValue {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One lower autodiff operation refused the requested transform.
    #[error(transparent)]
    Autodiff(#[from] AutodiffError),
}

impl ReverseModeTransformError {
    /// Returns the canonical refusal when the transform layer intentionally
    /// refuses one unsupported family.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        match self {
            Self::UnsupportedTargetDType { tensor_id, .. } => Some(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedGradient,
                    PsionicRefusalScope::Autodiff,
                    self.to_string(),
                )
                .with_subject(format!("{tensor_id:?}")),
            ),
            Self::Autodiff(error) => error.refusal(),
            Self::UnknownTensor { .. }
            | Self::UntrackedTarget { .. }
            | Self::NonSingletonOutput { .. }
            | Self::MissingForwardValue { .. } => None,
        }
    }
}

/// Materialized result from one public `grad` transform call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradTransformResult {
    /// Stable signature used to build the transform.
    pub signature: ReverseModeTransformSignature,
    /// Materialized gradients for each requested primal target.
    pub gradients: BTreeMap<TensorId, TensorData>,
    /// Underlying autodiff result retained for debugging or plan inspection.
    pub backward_result: AutodiffBackwardResult,
}

/// Materialized result from one public `value_and_grad` transform call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValueAndGradTransformResult {
    /// Stable signature used to build the transform.
    pub signature: ReverseModeTransformSignature,
    /// Forward value for the differentiated output tensor.
    pub value: TensorData,
    /// Materialized gradients for each requested primal target.
    pub gradients: BTreeMap<TensorId, TensorData>,
    /// Underlying autodiff result retained for debugging or plan inspection.
    pub backward_result: AutodiffBackwardResult,
}

/// Materialized result from one public `vjp` transform call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VjpTransformResult {
    /// Stable signature used to build the transform.
    pub signature: ReverseModeTransformSignature,
    /// Forward value for the differentiated output tensor.
    pub value: TensorData,
    /// Materialized cotangents for each requested primal target.
    pub cotangents: BTreeMap<TensorId, TensorData>,
    /// Underlying autodiff result retained for debugging or plan inspection.
    pub backward_result: AutodiffBackwardResult,
}

/// Stable target signature for one public `jvp` transform.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JvpTransformSignature {
    /// Output tensor whose primal value and tangent are exposed.
    pub output: TensorId,
    /// Primal tensors that must receive explicit tangent inputs.
    pub primal_targets: Vec<TensorId>,
}

/// Typed error returned by the public forward-mode transform layer.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ForwardModeTransformError {
    /// One requested tensor is not present in the graph.
    #[error("forward-mode transform graph does not contain tensor `{tensor_id}`")]
    UnknownTensor {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One requested target is not gradient-bearing under the bounded current graph contract.
    #[error("forward-mode transform target `{tensor_id}` is not gradient-tracked")]
    UntrackedTarget {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One requested target uses an unsupported dtype.
    #[error(
        "forward-mode transform target `{tensor_id}` uses unsupported tangent dtype `{dtype:?}`"
    )]
    UnsupportedTargetDType {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Observed dtype.
        dtype: DType,
    },
    /// One required tangent input was not provided.
    #[error("forward-mode transform target `{tensor_id}` requires an explicit tangent input")]
    MissingTangentTarget {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One tangent input was provided for a non-target tensor.
    #[error("forward-mode transform received unexpected tangent input for tensor `{tensor_id}`")]
    UnexpectedTangentTarget {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One tangent input used an unsupported storage family.
    #[error("tangent input for tensor `{tensor_id}` must be dense `f32`")]
    TangentDenseF32Required {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One tangent input length mismatched its target tensor.
    #[error(
        "tangent input for tensor `{tensor_id}` length mismatch: expected {expected_len}, found {actual_len}"
    )]
    TangentLengthMismatch {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Expected logical element count.
        expected_len: usize,
        /// Observed tangent length.
        actual_len: usize,
    },
    /// One forward-mode reference step hit an unsupported op family.
    #[error("forward-mode transform used unsupported op `{op}` at tensor `{tensor_id}`")]
    UnsupportedOp {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Stable op label.
        op: String,
    },
    /// One forward output value was missing after evaluation.
    #[error("forward-mode transform output `{tensor_id}` did not materialize a forward value")]
    MissingForwardValue {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One forward output tangent was missing after evaluation.
    #[error("forward-mode transform output `{tensor_id}` did not materialize a tangent value")]
    MissingOutputTangent {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One lower-layer reference-evaluation operation failed.
    #[error(transparent)]
    ReferenceEvaluation(#[from] ReferenceEvaluationError),
}

impl ForwardModeTransformError {
    /// Returns the canonical refusal when the forward-mode layer intentionally
    /// refuses one unsupported family.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        match self {
            Self::UnsupportedTargetDType { tensor_id, .. }
            | Self::TangentDenseF32Required { tensor_id }
            | Self::UnsupportedOp { tensor_id, .. } => Some(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedGradient,
                    PsionicRefusalScope::Autodiff,
                    self.to_string(),
                )
                .with_subject(format!("{tensor_id:?}")),
            ),
            Self::ReferenceEvaluation(error) => error.refusal(),
            Self::UnknownTensor { .. }
            | Self::UntrackedTarget { .. }
            | Self::MissingTangentTarget { .. }
            | Self::UnexpectedTangentTarget { .. }
            | Self::TangentLengthMismatch { .. }
            | Self::MissingForwardValue { .. }
            | Self::MissingOutputTangent { .. } => None,
        }
    }
}

/// Materialized result from one public `jvp` transform call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JvpTransformResult {
    /// Stable signature used to build the transform.
    pub signature: JvpTransformSignature,
    /// Forward value for the requested output tensor.
    pub value: TensorData,
    /// Tangent value for the requested output tensor.
    pub tangent: TensorData,
    /// Forward values for all materialized graph tensors.
    pub forward_values: BTreeMap<TensorId, TensorData>,
    /// Tangent values for all materialized graph tensors.
    pub tangent_values: BTreeMap<TensorId, TensorData>,
}

/// One public mapped-input binding for `vmap`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmapInputBinding {
    /// Graph input tensor that receives a batched runtime value.
    pub input: TensorId,
    /// Runtime batch axis inserted into that input.
    pub axis: usize,
}

/// Stable target signature for one public `vmap` transform.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmapTransformSignature {
    /// Output tensor whose lane values are stacked into the batched result.
    pub output: TensorId,
    /// Graph inputs that receive batched runtime values.
    pub mapped_inputs: Vec<VmapInputBinding>,
    /// Axis where the output batch dimension is inserted.
    pub output_axis: usize,
}

/// Typed error returned by the public `vmap` transform layer.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum VmapTransformError {
    /// The requested tensor is not present in the graph.
    #[error("vmap transform graph does not contain tensor `{tensor_id}`")]
    UnknownTensor {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// `vmap` currently requires at least one mapped graph input.
    #[error("vmap transform requires at least one mapped graph input")]
    MissingMappedInputs,
    /// One mapped target does not refer to a graph input tensor.
    #[error("vmap transform target `{tensor_id}` is not a graph input")]
    NonInputTarget {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One mapped graph input was declared more than once.
    #[error("vmap transform target `{tensor_id}` was declared more than once")]
    DuplicateMappedInput {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One requested mapped-input axis is outside the bounded supported range.
    #[error("vmap transform target `{tensor_id}` uses invalid batch axis {axis} for rank {rank}")]
    InvalidInputAxis {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Requested axis.
        axis: usize,
        /// Unbatched tensor rank.
        rank: usize,
    },
    /// The requested output axis is outside the bounded supported range.
    #[error("vmap transform output `{tensor_id}` uses invalid output axis {axis} for rank {rank}")]
    InvalidOutputAxis {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Requested axis.
        axis: usize,
        /// Unbatched output rank.
        rank: usize,
    },
    /// One requested target uses a dtype outside the bounded `vmap` surface.
    #[error("vmap transform target `{tensor_id}` uses unsupported dtype `{dtype:?}`")]
    UnsupportedTargetDType {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Observed dtype.
        dtype: DType,
    },
    /// One graph input required by the `vmap` signature was not provided.
    #[error("vmap transform target `{tensor_id}` requires a batched runtime input")]
    MissingMappedInput {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One mapped runtime input used an unsupported storage family.
    #[error("vmap transform target `{tensor_id}` must use dense `f32` runtime data")]
    MappedInputDenseF32Required {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One mapped runtime input length does not match whole unbatched lanes.
    #[error(
        "vmap transform target `{tensor_id}` length mismatch: expected a whole-number multiple of lane size {lane_len}, found {actual_len}"
    )]
    MappedInputLengthMismatch {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Logical element count for one unbatched lane.
        lane_len: usize,
        /// Observed runtime payload length.
        actual_len: usize,
    },
    /// Mapped runtime inputs must agree on one batch size.
    #[error(
        "vmap transform target `{tensor_id}` batch size mismatch: expected {expected_batch_size}, found {actual_batch_size}"
    )]
    BatchSizeMismatch {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Established batch size from an earlier mapped input.
        expected_batch_size: usize,
        /// Observed batch size for this input.
        actual_batch_size: usize,
    },
    /// One graph op is outside the bounded current `vmap` support matrix.
    #[error("vmap transform used unsupported op `{op}` at tensor `{tensor_id}`")]
    UnsupportedOp {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Stable op label.
        op: String,
    },
    /// One forward output value was missing after lane evaluation.
    #[error("vmap transform output `{tensor_id}` did not materialize a forward value")]
    MissingOutputValue {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One lower-layer reference-evaluation operation failed.
    #[error(transparent)]
    ReferenceEvaluation(#[from] ReferenceEvaluationError),
}

impl VmapTransformError {
    /// Returns the canonical refusal when the public `vmap` layer intentionally
    /// refuses one unsupported family.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        match self {
            Self::UnsupportedTargetDType { tensor_id, .. }
            | Self::MappedInputDenseF32Required { tensor_id } => Some(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedBackendCapability,
                    PsionicRefusalScope::Graph,
                    self.to_string(),
                )
                .with_subject(format!("{tensor_id:?}")),
            ),
            Self::UnsupportedOp { tensor_id, op } => Some(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedOp,
                    PsionicRefusalScope::Graph,
                    self.to_string(),
                )
                .with_subject(format!("{tensor_id:?}:{op}")),
            ),
            Self::ReferenceEvaluation(error) => error.refusal(),
            Self::UnknownTensor { .. }
            | Self::MissingMappedInputs
            | Self::NonInputTarget { .. }
            | Self::DuplicateMappedInput { .. }
            | Self::InvalidInputAxis { .. }
            | Self::InvalidOutputAxis { .. }
            | Self::MissingMappedInput { .. }
            | Self::MappedInputLengthMismatch { .. }
            | Self::BatchSizeMismatch { .. }
            | Self::MissingOutputValue { .. } => None,
        }
    }
}

/// Materialized result from one public `vmap` transform call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VmapTransformResult {
    /// Stable signature used to build the transform.
    pub signature: VmapTransformSignature,
    /// Batched output value produced by stacking the lane outputs.
    pub value: TensorData,
    /// Per-lane output values before stacking.
    pub lane_outputs: Vec<TensorData>,
}

/// Summary from one bounded retained-value reference evaluation pass.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetainedEvaluationSummary {
    /// Tensor ids retained after the pass completes.
    pub retained_tensors: Vec<TensorId>,
    /// Number of tensors dropped after their final consumer.
    pub dropped_tensor_count: usize,
    /// Peak simultaneously-live tensor count during the pass.
    pub peak_live_tensors: usize,
}

/// Materialized rematerialization report from one public `checkpoint` transform.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRematerializationReport {
    /// Output tensor kept from the initial forward pass.
    pub output_tensor: TensorId,
    /// Retention summary from the initial forward pass.
    pub initial_forward: RetainedEvaluationSummary,
    /// Retention summary from the replay forward pass.
    pub replay_forward: RetainedEvaluationSummary,
    /// Forward tensors intentionally replayed to bind the backward graph.
    pub replayed_binding_tensors: Vec<TensorId>,
}

/// Typed error returned by the public `checkpoint` transform layer.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum CheckpointTransformError {
    /// One graph op is outside the bounded current checkpoint support matrix.
    #[error("checkpoint transform used unsupported op `{op}` at tensor `{tensor_id}`")]
    UnsupportedOp {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Stable op label.
        op: String,
    },
    /// One retained forward output value was unexpectedly absent.
    #[error("checkpoint transform output `{tensor_id}` did not materialize a forward value")]
    MissingForwardValue {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One replay pass failed to retain a required backward binding.
    #[error("checkpoint transform replay did not retain required primal tensor `{tensor_id}`")]
    MissingReplayBinding {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One lower reverse-mode transform validation failed.
    #[error(transparent)]
    ReverseMode(#[from] ReverseModeTransformError),
    /// One lower autodiff operation refused the requested transform.
    #[error(transparent)]
    Autodiff(#[from] AutodiffError),
    /// One lower-layer reference-evaluation operation failed.
    #[error(transparent)]
    ReferenceEvaluation(#[from] ReferenceEvaluationError),
}

impl CheckpointTransformError {
    /// Returns the canonical refusal when the checkpoint layer intentionally
    /// refuses one unsupported family.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        match self {
            Self::UnsupportedOp { tensor_id, op } => Some(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedGradient,
                    PsionicRefusalScope::Autodiff,
                    self.to_string(),
                )
                .with_subject(format!("{tensor_id:?}:{op}")),
            ),
            Self::ReverseMode(error) => error.refusal(),
            Self::Autodiff(error) => error.refusal(),
            Self::ReferenceEvaluation(error) => error.refusal(),
            Self::MissingForwardValue { .. } | Self::MissingReplayBinding { .. } => None,
        }
    }
}

/// Materialized result from one public `checkpoint` transform call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckpointTransformResult {
    /// Stable signature used to build the transform.
    pub signature: ReverseModeTransformSignature,
    /// Forward value for the differentiated output tensor.
    pub value: TensorData,
    /// Materialized gradients for each requested primal target.
    pub gradients: BTreeMap<TensorId, TensorData>,
    /// Symbolic backward plan replayed by the transform.
    pub plan: AutodiffBackwardPlan,
    /// Replayed primal bindings retained for backward execution.
    pub replayed_primal_values: BTreeMap<TensorId, TensorData>,
    /// Explicit rematerialization report for the bounded reference path.
    pub rematerialization: CheckpointRematerializationReport,
}

/// First-class public `checkpoint` transform over one autodiff graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckpointTransform {
    graph: AutodiffGraph,
    signature: ReverseModeTransformSignature,
}

impl CheckpointTransform {
    /// Returns the stable signature for the transform.
    #[must_use]
    pub fn signature(&self) -> &ReverseModeTransformSignature {
        &self.signature
    }

    /// Materializes the forward value and requested gradients using explicit
    /// forward replay for the backward-plan primal bindings.
    pub fn apply(
        &self,
        inputs: &BTreeMap<TensorId, TensorData>,
    ) -> Result<CheckpointTransformResult, CheckpointTransformError> {
        self.apply_with_seed(inputs, None)
    }

    /// Materializes the forward value and gradients with one explicit upstream
    /// seed when the output is non-scalar.
    pub fn apply_with_seed(
        &self,
        inputs: &BTreeMap<TensorId, TensorData>,
        seed: Option<TensorData>,
    ) -> Result<CheckpointTransformResult, CheckpointTransformError> {
        let plan = self.graph.backward_plan(self.signature.output)?;
        let seed = resolve_backward_seed(&self.graph, self.signature.output, seed)?;
        let initial_forward = evaluate_graph_retaining(
            self.graph.graph(),
            inputs,
            BTreeSet::from([self.signature.output]),
        )?;
        let value = initial_forward
            .values
            .get(&self.signature.output)
            .cloned()
            .ok_or(CheckpointTransformError::MissingForwardValue {
                tensor_id: self.signature.output,
            })?;

        let replay_targets = plan
            .primal_bindings
            .iter()
            .map(|binding| binding.primal_tensor)
            .collect::<BTreeSet<_>>();
        let replay_forward = evaluate_graph_retaining(self.graph.graph(), inputs, replay_targets)?;
        let mut backward_inputs = BTreeMap::new();
        for binding in &plan.primal_bindings {
            let replayed = replay_forward
                .values
                .get(&binding.primal_tensor)
                .cloned()
                .ok_or(CheckpointTransformError::MissingReplayBinding {
                    tensor_id: binding.primal_tensor,
                })?;
            backward_inputs.insert(binding.gradient_graph_input, replayed);
        }
        backward_inputs.insert(plan.seed_input, seed);

        let backward_values = evaluate_graph(&plan.gradient_graph, &backward_inputs)?;
        let gradients = collect_requested_gradients_from_plan(
            &self.graph,
            &plan,
            &backward_values,
            self.signature.primal_targets.as_slice(),
        )?;
        let replayed_binding_tensors = replay_forward.summary.retained_tensors.clone();

        Ok(CheckpointTransformResult {
            signature: self.signature.clone(),
            value,
            gradients,
            plan,
            replayed_primal_values: replay_forward.values,
            rematerialization: CheckpointRematerializationReport {
                output_tensor: self.signature.output,
                initial_forward: initial_forward.summary,
                replay_forward: replay_forward.summary,
                replayed_binding_tensors,
            },
        })
    }
}

/// Stable kind identifier for one custom transform hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomTransformHookKind {
    /// User-defined reverse-mode vector-Jacobian product override.
    CustomVjp,
}

impl CustomTransformHookKind {
    fn label(self) -> &'static str {
        match self {
            Self::CustomVjp => "custom_vjp",
        }
    }
}

/// Serializable metadata for one registered transform hook.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformHookRegistration {
    /// Hook family under registration.
    pub kind: CustomTransformHookKind,
    /// Stable digest for the bound graph.
    pub graph_digest: String,
    /// Stable reverse-mode target signature for the hook.
    pub signature: ReverseModeTransformSignature,
    /// Stable digest for the reverse-mode signature.
    pub signature_digest: String,
    /// Human-readable custom rule label.
    pub rule_label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TransformHookKey {
    kind: CustomTransformHookKind,
    graph_digest: String,
    signature_digest: String,
}

#[derive(Clone)]
struct RegisteredCustomVjpRule {
    registration: TransformHookRegistration,
    rule: Arc<dyn CustomVjpRule>,
}

/// Typed error returned while registering one custom transform hook.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TransformHookRegistrationError {
    /// Custom rules must carry a stable human-readable label.
    #[error("transform hook registration requires a non-empty rule label")]
    EmptyRuleLabel,
    /// One graph/signature pair was already registered.
    #[error(
        "transform hook `{kind}` is already registered for graph `{graph_digest}` and signature `{signature_digest}`"
    )]
    DuplicateRegistration {
        /// Hook family under registration.
        kind: String,
        /// Stable digest for the bound graph.
        graph_digest: String,
        /// Stable digest for the reverse-mode signature.
        signature_digest: String,
    },
    /// One lower reverse-mode transform validation failed.
    #[error(transparent)]
    ReverseMode(#[from] ReverseModeTransformError),
}

impl TransformHookRegistrationError {
    /// Returns the canonical refusal when the registration layer intentionally
    /// refuses one unsupported family.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        match self {
            Self::ReverseMode(error) => error.refusal(),
            Self::EmptyRuleLabel | Self::DuplicateRegistration { .. } => None,
        }
    }
}

/// Typed error returned while looking up one registered custom transform hook.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TransformHookLookupError {
    /// No hook is currently registered for the requested graph/signature pair.
    #[error(
        "transform hook `{kind}` is not registered for graph `{graph_digest}` and signature `{signature_digest}`"
    )]
    MissingRegistration {
        /// Hook family under lookup.
        kind: String,
        /// Stable digest for the bound graph.
        graph_digest: String,
        /// Stable digest for the reverse-mode signature.
        signature_digest: String,
    },
    /// One lower reverse-mode transform validation failed.
    #[error(transparent)]
    ReverseMode(#[from] ReverseModeTransformError),
}

impl TransformHookLookupError {
    /// Returns the canonical refusal when the lookup layer intentionally refuses
    /// one unsupported family.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        match self {
            Self::ReverseMode(error) => error.refusal(),
            Self::MissingRegistration { .. } => None,
        }
    }
}

/// Runtime invocation context for one custom-vjp rule.
pub struct CustomVjpInvocation<'a> {
    /// Graph whose reverse-mode contract is being overridden.
    pub graph: &'a AutodiffGraph,
    /// Stable digest for that graph.
    pub graph_digest: &'a str,
    /// Reverse-mode signature under execution.
    pub signature: &'a ReverseModeTransformSignature,
    /// Runtime primal inputs for the graph.
    pub inputs: &'a BTreeMap<TensorId, TensorData>,
    /// Materialized forward value for the requested output.
    pub value: &'a TensorData,
    /// Upstream cotangent seed for the requested output.
    pub seed: &'a TensorData,
}

/// User-defined reverse-mode cotangent override.
pub trait CustomVjpRule: Send + Sync {
    /// Stable rule label surfaced in diagnostics and receipts.
    fn label(&self) -> &str;

    /// Materializes custom cotangents for the requested reverse-mode targets.
    fn apply(
        &self,
        invocation: &CustomVjpInvocation<'_>,
    ) -> Result<BTreeMap<TensorId, TensorData>, PsionicRefusal>;
}

/// Graph-scoped registry for reusable custom transform hooks.
#[derive(Clone, Default)]
pub struct TransformHookRegistry {
    custom_vjp_rules: BTreeMap<TransformHookKey, RegisteredCustomVjpRule>,
}

impl std::fmt::Debug for TransformHookRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransformHookRegistry")
            .field("registrations", &self.registrations())
            .finish()
    }
}

impl TransformHookRegistry {
    /// Returns all registered hooks in deterministic order.
    #[must_use]
    pub fn registrations(&self) -> Vec<TransformHookRegistration> {
        self.custom_vjp_rules
            .values()
            .map(|entry| entry.registration.clone())
            .collect()
    }

    /// Registers one custom-vjp rule for a graph/signature pair.
    pub fn register_custom_vjp(
        &mut self,
        graph: &AutodiffGraph,
        output: TensorId,
        primal_targets: &[TensorId],
        rule: Arc<dyn CustomVjpRule>,
    ) -> Result<TransformHookRegistration, TransformHookRegistrationError> {
        let signature =
            validate_reverse_mode_transform_signature(graph, output, primal_targets, None)?;
        let rule_label = rule.label().trim().to_string();
        if rule_label.is_empty() {
            return Err(TransformHookRegistrationError::EmptyRuleLabel);
        }
        let graph_digest = graph.graph().stable_digest();
        let signature_digest = stable_reverse_mode_transform_signature_digest(&signature);
        let key = TransformHookKey {
            kind: CustomTransformHookKind::CustomVjp,
            graph_digest: graph_digest.clone(),
            signature_digest: signature_digest.clone(),
        };
        if self.custom_vjp_rules.contains_key(&key) {
            return Err(TransformHookRegistrationError::DuplicateRegistration {
                kind: String::from(CustomTransformHookKind::CustomVjp.label()),
                graph_digest,
                signature_digest,
            });
        }
        let registration = TransformHookRegistration {
            kind: CustomTransformHookKind::CustomVjp,
            graph_digest: key.graph_digest.clone(),
            signature,
            signature_digest: key.signature_digest.clone(),
            rule_label,
        };
        self.custom_vjp_rules.insert(
            key,
            RegisteredCustomVjpRule {
                registration: registration.clone(),
                rule,
            },
        );
        Ok(registration)
    }

    /// Looks up one registered custom-vjp transform for the provided graph and
    /// reverse-mode targets.
    pub fn custom_vjp(
        &self,
        graph: &AutodiffGraph,
        output: TensorId,
        primal_targets: &[TensorId],
    ) -> Result<CustomVjpTransform, TransformHookLookupError> {
        let signature =
            validate_reverse_mode_transform_signature(graph, output, primal_targets, None)?;
        let graph_digest = graph.graph().stable_digest();
        let signature_digest = stable_reverse_mode_transform_signature_digest(&signature);
        let key = TransformHookKey {
            kind: CustomTransformHookKind::CustomVjp,
            graph_digest: graph_digest.clone(),
            signature_digest: signature_digest.clone(),
        };
        let registered = self.custom_vjp_rules.get(&key).ok_or(
            TransformHookLookupError::MissingRegistration {
                kind: String::from(CustomTransformHookKind::CustomVjp.label()),
                graph_digest,
                signature_digest,
            },
        )?;
        Ok(CustomVjpTransform {
            graph: graph.clone(),
            signature,
            registration: registered.registration.clone(),
            rule: Arc::clone(&registered.rule),
        })
    }
}

/// Typed error returned by the public `custom_vjp` transform layer.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum CustomVjpTransformError {
    /// One retained forward output value was unexpectedly absent.
    #[error("custom_vjp transform output `{tensor_id}` did not materialize a forward value")]
    MissingForwardValue {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// The custom rule returned a cotangent for an unknown target.
    #[error("custom_vjp transform returned unexpected cotangent target `{tensor_id}`")]
    UnexpectedCotangentTarget {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// The custom rule omitted one requested cotangent.
    #[error("custom_vjp transform omitted cotangent target `{tensor_id}`")]
    MissingCotangentTarget {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One returned cotangent used an unsupported storage family.
    #[error("custom_vjp transform cotangent for tensor `{tensor_id}` must be dense `f32`")]
    CotangentDenseF32Required {
        /// Stable tensor identifier.
        tensor_id: TensorId,
    },
    /// One returned cotangent length mismatched its target tensor.
    #[error(
        "custom_vjp transform cotangent for tensor `{tensor_id}` length mismatch: expected {expected_len}, found {actual_len}"
    )]
    CotangentLengthMismatch {
        /// Stable tensor identifier.
        tensor_id: TensorId,
        /// Expected logical element count.
        expected_len: usize,
        /// Observed cotangent length.
        actual_len: usize,
    },
    /// The user-defined rule refused the requested invocation.
    #[error("custom_vjp rule `{rule_label}` refused: {detail}")]
    RuleRefusal {
        /// Human-readable rule label.
        rule_label: String,
        /// Plain-language refusal detail.
        detail: String,
        /// Canonical refusal emitted by the rule.
        refusal: PsionicRefusal,
    },
    /// One lower reverse-mode transform validation failed.
    #[error(transparent)]
    ReverseMode(#[from] ReverseModeTransformError),
    /// One lower autodiff operation refused the requested transform.
    #[error(transparent)]
    Autodiff(#[from] AutodiffError),
    /// One lower-layer reference-evaluation operation failed.
    #[error(transparent)]
    ReferenceEvaluation(#[from] ReferenceEvaluationError),
}

impl CustomVjpTransformError {
    /// Returns the canonical refusal when the custom-vjp layer intentionally
    /// refuses one unsupported family.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        match self {
            Self::CotangentDenseF32Required { tensor_id } => Some(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedGradient,
                    PsionicRefusalScope::Autodiff,
                    self.to_string(),
                )
                .with_subject(format!("{tensor_id:?}")),
            ),
            Self::RuleRefusal { refusal, .. } => Some(refusal.clone()),
            Self::ReverseMode(error) => error.refusal(),
            Self::Autodiff(error) => error.refusal(),
            Self::ReferenceEvaluation(error) => error.refusal(),
            Self::MissingForwardValue { .. }
            | Self::UnexpectedCotangentTarget { .. }
            | Self::MissingCotangentTarget { .. }
            | Self::CotangentLengthMismatch { .. } => None,
        }
    }
}

/// Materialized result from one public `custom_vjp` transform call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomVjpTransformResult {
    /// Stable signature used to build the transform.
    pub signature: ReverseModeTransformSignature,
    /// Forward value for the differentiated output tensor.
    pub value: TensorData,
    /// Materialized cotangents from the custom rule.
    pub cotangents: BTreeMap<TensorId, TensorData>,
    /// Registration metadata for the applied custom rule.
    pub registration: TransformHookRegistration,
}

/// First-class public `custom_vjp` transform over one autodiff graph.
#[derive(Clone)]
pub struct CustomVjpTransform {
    graph: AutodiffGraph,
    signature: ReverseModeTransformSignature,
    registration: TransformHookRegistration,
    rule: Arc<dyn CustomVjpRule>,
}

impl std::fmt::Debug for CustomVjpTransform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomVjpTransform")
            .field("signature", &self.signature)
            .field("registration", &self.registration)
            .finish()
    }
}

impl CustomVjpTransform {
    /// Returns the stable signature for the transform.
    #[must_use]
    pub fn signature(&self) -> &ReverseModeTransformSignature {
        &self.signature
    }

    /// Returns the registration metadata for the transform.
    #[must_use]
    pub fn registration(&self) -> &TransformHookRegistration {
        &self.registration
    }

    /// Materializes the forward value and requested cotangents with the
    /// default scalar seed when possible.
    pub fn apply(
        &self,
        inputs: &BTreeMap<TensorId, TensorData>,
    ) -> Result<CustomVjpTransformResult, CustomVjpTransformError> {
        self.apply_with_seed(inputs, None)
    }

    /// Materializes the forward value and requested cotangents using one
    /// explicit upstream seed when the output is non-scalar.
    pub fn apply_with_seed(
        &self,
        inputs: &BTreeMap<TensorId, TensorData>,
        seed: Option<TensorData>,
    ) -> Result<CustomVjpTransformResult, CustomVjpTransformError> {
        let forward_values = evaluate_graph(self.graph.graph(), inputs)?;
        let value = forward_values.get(&self.signature.output).cloned().ok_or(
            CustomVjpTransformError::MissingForwardValue {
                tensor_id: self.signature.output,
            },
        )?;
        let seed = resolve_backward_seed(&self.graph, self.signature.output, seed)?;
        let graph_digest = self.graph.graph().stable_digest();
        let invocation = CustomVjpInvocation {
            graph: &self.graph,
            graph_digest: graph_digest.as_str(),
            signature: &self.signature,
            inputs,
            value: &value,
            seed: &seed,
        };
        let cotangents = self.rule.apply(&invocation).map_err(|refusal| {
            CustomVjpTransformError::RuleRefusal {
                rule_label: self.registration.rule_label.clone(),
                detail: refusal.detail.clone(),
                refusal,
            }
        })?;
        let cotangents = validate_custom_vjp_cotangents(&self.graph, &self.signature, cotangents)?;
        Ok(CustomVjpTransformResult {
            signature: self.signature.clone(),
            value,
            cotangents,
            registration: self.registration.clone(),
        })
    }
}

/// First-class public `grad` transform over one autodiff graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradTransform {
    graph: AutodiffGraph,
    signature: ReverseModeTransformSignature,
}

impl GradTransform {
    /// Returns the stable signature for the transform.
    #[must_use]
    pub fn signature(&self) -> &ReverseModeTransformSignature {
        &self.signature
    }

    /// Materializes the requested gradients for one set of primal inputs.
    pub fn apply(
        &self,
        inputs: &BTreeMap<TensorId, TensorData>,
    ) -> Result<GradTransformResult, ReverseModeTransformError> {
        let backward_result = self
            .graph
            .backward_materialized(self.signature.output, inputs)?;
        let gradients = collect_requested_gradients(
            &self.graph,
            &backward_result,
            self.signature.primal_targets.as_slice(),
        )?;
        Ok(GradTransformResult {
            signature: self.signature.clone(),
            gradients,
            backward_result,
        })
    }
}

/// First-class public `value_and_grad` transform over one autodiff graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValueAndGradTransform {
    graph: AutodiffGraph,
    signature: ReverseModeTransformSignature,
}

impl ValueAndGradTransform {
    /// Returns the stable signature for the transform.
    #[must_use]
    pub fn signature(&self) -> &ReverseModeTransformSignature {
        &self.signature
    }

    /// Materializes the forward value and requested gradients for one input set.
    pub fn apply(
        &self,
        inputs: &BTreeMap<TensorId, TensorData>,
    ) -> Result<ValueAndGradTransformResult, ReverseModeTransformError> {
        let backward_result = self
            .graph
            .backward_materialized(self.signature.output, inputs)?;
        let value = backward_result
            .forward_values
            .get(&self.signature.output)
            .cloned()
            .ok_or(ReverseModeTransformError::MissingForwardValue {
                tensor_id: self.signature.output,
            })?;
        let gradients = collect_requested_gradients(
            &self.graph,
            &backward_result,
            self.signature.primal_targets.as_slice(),
        )?;
        Ok(ValueAndGradTransformResult {
            signature: self.signature.clone(),
            value,
            gradients,
            backward_result,
        })
    }
}

/// First-class public `vjp` transform over one autodiff graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VjpTransform {
    graph: AutodiffGraph,
    signature: ReverseModeTransformSignature,
}

impl VjpTransform {
    /// Returns the stable signature for the transform.
    #[must_use]
    pub fn signature(&self) -> &ReverseModeTransformSignature {
        &self.signature
    }

    /// Materializes the forward value and requested cotangents using one explicit upstream seed.
    pub fn apply(
        &self,
        inputs: &BTreeMap<TensorId, TensorData>,
        seed: TensorData,
    ) -> Result<VjpTransformResult, ReverseModeTransformError> {
        let backward_result = self.graph.backward_materialized_with_seed(
            self.signature.output,
            inputs,
            Some(seed),
        )?;
        let value = backward_result
            .forward_values
            .get(&self.signature.output)
            .cloned()
            .ok_or(ReverseModeTransformError::MissingForwardValue {
                tensor_id: self.signature.output,
            })?;
        let cotangents = collect_requested_gradients(
            &self.graph,
            &backward_result,
            self.signature.primal_targets.as_slice(),
        )?;
        Ok(VjpTransformResult {
            signature: self.signature.clone(),
            value,
            cotangents,
            backward_result,
        })
    }
}

/// First-class public `jvp` transform over one autodiff graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JvpTransform {
    graph: AutodiffGraph,
    signature: JvpTransformSignature,
}

impl JvpTransform {
    /// Returns the stable signature for the transform.
    #[must_use]
    pub fn signature(&self) -> &JvpTransformSignature {
        &self.signature
    }

    /// Materializes the forward value and output tangent using explicit primal tangents.
    pub fn apply(
        &self,
        inputs: &BTreeMap<TensorId, TensorData>,
        tangents: &BTreeMap<TensorId, TensorData>,
    ) -> Result<JvpTransformResult, ForwardModeTransformError> {
        let tangent_inputs =
            validate_and_seed_tangent_inputs(&self.graph, &self.signature, tangents)?;
        let (forward_values, tangent_values) =
            evaluate_graph_forward_mode(self.graph.graph(), inputs, &tangent_inputs)?;
        let value = forward_values.get(&self.signature.output).cloned().ok_or(
            ForwardModeTransformError::MissingForwardValue {
                tensor_id: self.signature.output,
            },
        )?;
        let tangent = tangent_values.get(&self.signature.output).cloned().ok_or(
            ForwardModeTransformError::MissingOutputTangent {
                tensor_id: self.signature.output,
            },
        )?;
        Ok(JvpTransformResult {
            signature: self.signature.clone(),
            value,
            tangent,
            forward_values,
            tangent_values,
        })
    }
}

/// First-class public `vmap` transform over one single-lane graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VmapTransform {
    graph: AutodiffGraph,
    signature: VmapTransformSignature,
}

impl VmapTransform {
    /// Returns the stable signature for the transform.
    #[must_use]
    pub fn signature(&self) -> &VmapTransformSignature {
        &self.signature
    }

    /// Materializes the batched output by evaluating the single-lane graph once
    /// per runtime batch element.
    pub fn apply(
        &self,
        inputs: &BTreeMap<TensorId, TensorData>,
    ) -> Result<VmapTransformResult, VmapTransformError> {
        let (batch_size, mapped_inputs) =
            validate_and_seed_vmap_inputs(&self.graph, &self.signature, inputs)?;
        let output_node = self.graph.graph().node(self.signature.output).ok_or(
            VmapTransformError::UnknownTensor {
                tensor_id: self.signature.output,
            },
        )?;
        let lane_shape = output_node.tensor().spec().shape().clone();
        let mut lane_outputs = Vec::with_capacity(batch_size);

        for lane in 0..batch_size {
            let mut lane_inputs = inputs.clone();
            for binding in &self.signature.mapped_inputs {
                let mapped_input = mapped_inputs.get(&binding.input).ok_or(
                    VmapTransformError::MissingMappedInput {
                        tensor_id: binding.input,
                    },
                )?;
                lane_inputs.insert(
                    binding.input,
                    TensorData::F32(select_values(
                        mapped_input.values.as_slice(),
                        &mapped_input.batched_shape,
                        binding.axis,
                        lane,
                    )),
                );
            }
            let lane_values = evaluate_graph(self.graph.graph(), &lane_inputs)?;
            let lane_output = lane_values.get(&self.signature.output).cloned().ok_or(
                VmapTransformError::MissingOutputValue {
                    tensor_id: self.signature.output,
                },
            )?;
            lane_outputs.push(lane_output);
        }

        let value = stack_vmap_lane_outputs(
            lane_outputs.as_slice(),
            &lane_shape,
            self.signature.output_axis,
            self.signature.output,
        )?;
        Ok(VmapTransformResult {
            signature: self.signature.clone(),
            value,
            lane_outputs,
        })
    }
}

/// Creates one public `grad` transform over the provided output and primal targets.
pub fn grad(
    graph: &AutodiffGraph,
    output: TensorId,
    primal_targets: &[TensorId],
) -> Result<GradTransform, ReverseModeTransformError> {
    let signature =
        validate_reverse_mode_transform_signature(graph, output, primal_targets, Some("grad"))?;
    Ok(GradTransform {
        graph: graph.clone(),
        signature,
    })
}

/// Creates one public `value_and_grad` transform over the provided output and primal targets.
pub fn value_and_grad(
    graph: &AutodiffGraph,
    output: TensorId,
    primal_targets: &[TensorId],
) -> Result<ValueAndGradTransform, ReverseModeTransformError> {
    let signature = validate_reverse_mode_transform_signature(
        graph,
        output,
        primal_targets,
        Some("value_and_grad"),
    )?;
    Ok(ValueAndGradTransform {
        graph: graph.clone(),
        signature,
    })
}

/// Creates one public `vjp` transform over the provided output and primal targets.
pub fn vjp(
    graph: &AutodiffGraph,
    output: TensorId,
    primal_targets: &[TensorId],
) -> Result<VjpTransform, ReverseModeTransformError> {
    let signature = validate_reverse_mode_transform_signature(graph, output, primal_targets, None)?;
    Ok(VjpTransform {
        graph: graph.clone(),
        signature,
    })
}

/// Creates one public `jvp` transform over the provided output and primal targets.
pub fn jvp(
    graph: &AutodiffGraph,
    output: TensorId,
    primal_targets: &[TensorId],
) -> Result<JvpTransform, ForwardModeTransformError> {
    let signature = validate_jvp_transform_signature(graph, output, primal_targets)?;
    Ok(JvpTransform {
        graph: graph.clone(),
        signature,
    })
}

/// Creates one public `vmap` transform over the provided output and mapped
/// graph inputs.
pub fn vmap(
    graph: &AutodiffGraph,
    output: TensorId,
    mapped_inputs: &[VmapInputBinding],
    output_axis: usize,
) -> Result<VmapTransform, VmapTransformError> {
    let signature = validate_vmap_transform_signature(graph, output, mapped_inputs, output_axis)?;
    Ok(VmapTransform {
        graph: graph.clone(),
        signature,
    })
}

/// Creates one public `checkpoint` transform over the provided output and
/// primal targets.
pub fn checkpoint(
    graph: &AutodiffGraph,
    output: TensorId,
    primal_targets: &[TensorId],
) -> Result<CheckpointTransform, CheckpointTransformError> {
    for node in graph.graph().nodes() {
        if let AutodiffGradientSupport::Unsupported { .. } = gradient_support_for_op(node.op()) {
            return Err(CheckpointTransformError::UnsupportedOp {
                tensor_id: node.tensor().id(),
                op: String::from(node.op().label()),
            });
        }
    }
    let signature = validate_reverse_mode_transform_signature(graph, output, primal_targets, None)?;
    Ok(CheckpointTransform {
        graph: graph.clone(),
        signature,
    })
}

/// Looks up one registered public `custom_vjp` transform over the provided
/// output and primal targets.
pub fn custom_vjp(
    registry: &TransformHookRegistry,
    graph: &AutodiffGraph,
    output: TensorId,
    primal_targets: &[TensorId],
) -> Result<CustomVjpTransform, TransformHookLookupError> {
    registry.custom_vjp(graph, output, primal_targets)
}

/// Autodiff-aware graph bundle with per-tensor tracking posture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutodiffGraph {
    graph: Graph,
    context: AutodiffContext,
    gradient_tracking: BTreeMap<TensorId, bool>,
}

impl AutodiffGraph {
    /// Returns the underlying canonical graph.
    #[must_use]
    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Returns the graph construction context.
    #[must_use]
    pub const fn context(&self) -> AutodiffContext {
        self.context
    }

    /// Returns whether the given tensor is gradient-bearing.
    #[must_use]
    pub fn requires_grad(&self, tensor_id: TensorId) -> bool {
        self.gradient_tracking
            .get(&tensor_id)
            .copied()
            .unwrap_or(false)
    }

    /// Returns the tracked tensor IDs in deterministic order.
    #[must_use]
    pub fn tracked_tensor_ids(&self) -> Vec<TensorId> {
        self.gradient_tracking
            .iter()
            .filter_map(|(tensor_id, requires_grad)| requires_grad.then_some(*tensor_id))
            .collect()
    }

    /// Builds a symbolic reverse-mode plan for one graph output.
    pub fn backward_plan(&self, output: TensorId) -> Result<AutodiffBackwardPlan, AutodiffError> {
        let output_node = self
            .graph
            .node(output)
            .ok_or(AutodiffError::UnknownTensor { tensor_id: output })?;
        if !self.requires_grad(output) {
            return Err(AutodiffError::OutputNotTracked { tensor_id: output });
        }
        ensure_supported_gradient_dtype(output_node.tensor())?;

        let mut backward_builder = GraphBuilder::new(output_node.tensor().spec().device().clone());
        let seed = backward_builder.input(
            format!("grad.seed.{}", output),
            output_node.tensor().spec().shape().clone(),
            output_node.tensor().spec().dtype(),
        );
        let mut gradients = BTreeMap::<TensorId, Tensor>::new();
        gradients.insert(output, seed.clone());
        let mut primal_bindings = BTreeMap::<TensorId, Tensor>::new();

        for node in self.graph.nodes().iter().rev() {
            let output_id = node.tensor().id();
            let Some(current_gradient) = gradients.get(&output_id).cloned() else {
                continue;
            };
            ensure_supported_gradient_dtype(node.tensor())?;
            if let AutodiffGradientSupport::Unsupported { .. } = gradient_support_for_op(node.op())
            {
                return Err(AutodiffError::UnsupportedGradientOp {
                    tensor_id: output_id,
                    op: String::from(node.op().label()),
                });
            }

            match node.op() {
                OpKind::Input { .. } | OpKind::Constant { .. } | OpKind::Detach => {}
                OpKind::Cast { .. } => {
                    let input_id = node.inputs()[0];
                    if self.requires_grad(input_id) {
                        let input_dtype = self
                            .graph
                            .node(input_id)
                            .ok_or(AutodiffError::UnknownTensor {
                                tensor_id: input_id,
                            })?
                            .tensor()
                            .spec()
                            .dtype();
                        let contribution = if current_gradient.spec().dtype() == input_dtype {
                            current_gradient.clone()
                        } else {
                            backward_builder
                                .cast(&current_gradient, input_dtype)
                                .map_err(map_graph_error)?
                        };
                        accumulate_gradient(
                            &mut backward_builder,
                            &mut gradients,
                            input_id,
                            contribution,
                        )?;
                    }
                }
                OpKind::Add => {
                    for input_id in node.inputs() {
                        if self.requires_grad(*input_id) {
                            accumulate_gradient(
                                &mut backward_builder,
                                &mut gradients,
                                *input_id,
                                current_gradient.clone(),
                            )?;
                        }
                    }
                }
                OpKind::Mul => {
                    let left_id = node.inputs()[0];
                    let right_id = node.inputs()[1];
                    if self.requires_grad(left_id) {
                        let right = primal_placeholder(
                            &mut backward_builder,
                            &mut primal_bindings,
                            &self.graph,
                            right_id,
                        )?;
                        let contribution = backward_builder
                            .mul(&current_gradient, &right)
                            .map_err(map_graph_error)?;
                        accumulate_gradient(
                            &mut backward_builder,
                            &mut gradients,
                            left_id,
                            contribution,
                        )?;
                    }
                    if self.requires_grad(right_id) {
                        let left = primal_placeholder(
                            &mut backward_builder,
                            &mut primal_bindings,
                            &self.graph,
                            left_id,
                        )?;
                        let contribution = backward_builder
                            .mul(&current_gradient, &left)
                            .map_err(map_graph_error)?;
                        accumulate_gradient(
                            &mut backward_builder,
                            &mut gradients,
                            right_id,
                            contribution,
                        )?;
                    }
                }
                OpKind::Matmul => {
                    let left_id = node.inputs()[0];
                    let right_id = node.inputs()[1];
                    if self.requires_grad(left_id) {
                        let right = primal_placeholder(
                            &mut backward_builder,
                            &mut primal_bindings,
                            &self.graph,
                            right_id,
                        )?;
                        let right_transposed = backward_builder
                            .permute(&right, vec![1, 0])
                            .map_err(map_graph_error)?;
                        let contribution = backward_builder
                            .matmul(&current_gradient, &right_transposed)
                            .map_err(map_graph_error)?;
                        accumulate_gradient(
                            &mut backward_builder,
                            &mut gradients,
                            left_id,
                            contribution,
                        )?;
                    }
                    if self.requires_grad(right_id) {
                        let left = primal_placeholder(
                            &mut backward_builder,
                            &mut primal_bindings,
                            &self.graph,
                            left_id,
                        )?;
                        let left_transposed = backward_builder
                            .permute(&left, vec![1, 0])
                            .map_err(map_graph_error)?;
                        let contribution = backward_builder
                            .matmul(&left_transposed, &current_gradient)
                            .map_err(map_graph_error)?;
                        accumulate_gradient(
                            &mut backward_builder,
                            &mut gradients,
                            right_id,
                            contribution,
                        )?;
                    }
                }
                OpKind::Reshape => {
                    let input_id = node.inputs()[0];
                    if self.requires_grad(input_id) {
                        let input_shape = self
                            .graph
                            .node(input_id)
                            .ok_or(AutodiffError::UnknownTensor {
                                tensor_id: input_id,
                            })?
                            .tensor()
                            .spec()
                            .shape()
                            .clone();
                        let contribution = backward_builder
                            .reshape(&current_gradient, input_shape)
                            .map_err(map_graph_error)?;
                        accumulate_gradient(
                            &mut backward_builder,
                            &mut gradients,
                            input_id,
                            contribution,
                        )?;
                    }
                }
                OpKind::Permute { axes } => {
                    let input_id = node.inputs()[0];
                    if self.requires_grad(input_id) {
                        let inverse = invert_axes(axes);
                        let contribution = backward_builder
                            .permute(&current_gradient, inverse)
                            .map_err(map_graph_error)?;
                        accumulate_gradient(
                            &mut backward_builder,
                            &mut gradients,
                            input_id,
                            contribution,
                        )?;
                    }
                }
                OpKind::Slice { axis, start, end } => {
                    let input_id = node.inputs()[0];
                    if self.requires_grad(input_id) {
                        let input_shape = self
                            .graph
                            .node(input_id)
                            .ok_or(AutodiffError::UnknownTensor {
                                tensor_id: input_id,
                            })?
                            .tensor()
                            .spec()
                            .shape()
                            .clone();
                        let contribution = pad_axis_with_zeros(
                            &mut backward_builder,
                            &current_gradient,
                            &input_shape,
                            *axis,
                            *start,
                            *end,
                        )?;
                        accumulate_gradient(
                            &mut backward_builder,
                            &mut gradients,
                            input_id,
                            contribution,
                        )?;
                    }
                }
                OpKind::Select { axis, index } => {
                    let input_id = node.inputs()[0];
                    if self.requires_grad(input_id) {
                        let input_shape = self
                            .graph
                            .node(input_id)
                            .ok_or(AutodiffError::UnknownTensor {
                                tensor_id: input_id,
                            })?
                            .tensor()
                            .spec()
                            .shape()
                            .clone();
                        let select_shape = Shape::new(insert_axis(
                            current_gradient.spec().shape().dims(),
                            *axis,
                            1,
                        ));
                        let reshaped = backward_builder
                            .reshape(&current_gradient, select_shape)
                            .map_err(map_graph_error)?;
                        let contribution = pad_axis_with_zeros(
                            &mut backward_builder,
                            &reshaped,
                            &input_shape,
                            *axis,
                            *index,
                            index.saturating_add(1),
                        )?;
                        accumulate_gradient(
                            &mut backward_builder,
                            &mut gradients,
                            input_id,
                            contribution,
                        )?;
                    }
                }
                OpKind::Concat { axis } => {
                    let mut offset = 0usize;
                    for input_id in node.inputs() {
                        if !self.requires_grad(*input_id) {
                            offset = offset.saturating_add(
                                self.graph
                                    .node(*input_id)
                                    .ok_or(AutodiffError::UnknownTensor {
                                        tensor_id: *input_id,
                                    })?
                                    .tensor()
                                    .spec()
                                    .shape()
                                    .dims()[*axis],
                            );
                            continue;
                        }
                        let input_shape = self
                            .graph
                            .node(*input_id)
                            .ok_or(AutodiffError::UnknownTensor {
                                tensor_id: *input_id,
                            })?
                            .tensor()
                            .spec()
                            .shape()
                            .clone();
                        let next_offset = offset.saturating_add(input_shape.dims()[*axis]);
                        let contribution = backward_builder
                            .slice(&current_gradient, *axis, offset, next_offset)
                            .map_err(map_graph_error)?;
                        accumulate_gradient(
                            &mut backward_builder,
                            &mut gradients,
                            *input_id,
                            contribution,
                        )?;
                        offset = next_offset;
                    }
                }
                OpKind::Expand { .. } => {
                    let input_id = node.inputs()[0];
                    if self.requires_grad(input_id) {
                        let input_shape = self
                            .graph
                            .node(input_id)
                            .ok_or(AutodiffError::UnknownTensor {
                                tensor_id: input_id,
                            })?
                            .tensor()
                            .spec()
                            .shape()
                            .clone();
                        let contribution = reduce_gradient_to_shape(
                            &mut backward_builder,
                            &current_gradient,
                            &input_shape,
                        )?;
                        accumulate_gradient(
                            &mut backward_builder,
                            &mut gradients,
                            input_id,
                            contribution,
                        )?;
                    }
                }
                OpKind::ReduceSum { axis } => {
                    let input_id = node.inputs()[0];
                    if self.requires_grad(input_id) {
                        let input_shape = self
                            .graph
                            .node(input_id)
                            .ok_or(AutodiffError::UnknownTensor {
                                tensor_id: input_id,
                            })?
                            .tensor()
                            .spec()
                            .shape()
                            .clone();
                        let expanded = if let Some(axis) = axis {
                            let reduced_shape = Shape::new(insert_axis(
                                current_gradient.spec().shape().dims(),
                                *axis,
                                1,
                            ));
                            let reshaped = backward_builder
                                .reshape(&current_gradient, reduced_shape)
                                .map_err(map_graph_error)?;
                            backward_builder
                                .expand(&reshaped, input_shape)
                                .map_err(map_graph_error)?
                        } else {
                            backward_builder
                                .expand(&current_gradient, input_shape)
                                .map_err(map_graph_error)?
                        };
                        accumulate_gradient(
                            &mut backward_builder,
                            &mut gradients,
                            input_id,
                            expanded,
                        )?;
                    }
                }
                OpKind::BackendExtension { op } => match op {
                    BackendExtensionOp::ReluSquared => {
                        let input_id = node.inputs()[0];
                        if self.requires_grad(input_id) {
                            let output = primal_placeholder(
                                &mut backward_builder,
                                &mut primal_bindings,
                                &self.graph,
                                output_id,
                            )?;
                            let contribution = backward_builder
                                .relu_squared_backward_from_output(&output, &current_gradient)
                                .map_err(map_graph_error)?;
                            accumulate_gradient(
                                &mut backward_builder,
                                &mut gradients,
                                input_id,
                                contribution,
                            )?;
                        }
                    }
                    BackendExtensionOp::LeakyReluSquared { negative_slope } => {
                        let input_id = node.inputs()[0];
                        if self.requires_grad(input_id) {
                            let input = primal_placeholder(
                                &mut backward_builder,
                                &mut primal_bindings,
                                &self.graph,
                                input_id,
                            )?;
                            let contribution = backward_builder
                                .leaky_relu_squared_backward(
                                    &input,
                                    &current_gradient,
                                    negative_slope.to_f32(),
                                )
                                .map_err(map_graph_error)?;
                            accumulate_gradient(
                                &mut backward_builder,
                                &mut gradients,
                                input_id,
                                contribution,
                            )?;
                        }
                    }
                    BackendExtensionOp::Silu => {
                        let input_id = node.inputs()[0];
                        if self.requires_grad(input_id) {
                            let input = primal_placeholder(
                                &mut backward_builder,
                                &mut primal_bindings,
                                &self.graph,
                                input_id,
                            )?;
                            let contribution = backward_builder
                                .silu_backward(&input, &current_gradient)
                                .map_err(map_graph_error)?;
                            accumulate_gradient(
                                &mut backward_builder,
                                &mut gradients,
                                input_id,
                                contribution,
                            )?;
                        }
                    }
                    BackendExtensionOp::ParameterGolfTokenEmbeddingLookup => {
                        let token_ids_id = node.inputs()[0];
                        let token_embedding_id = node.inputs()[1];
                        if self.requires_grad(token_ids_id) {
                            return Err(AutodiffError::UnsupportedGradientOp {
                                tensor_id: node.tensor().id(),
                                op: String::from(
                                    "parameter_golf_token_embedding_lookup_token_ids",
                                ),
                            });
                        }
                        if self.requires_grad(token_embedding_id) {
                            let token_ids = primal_placeholder(
                                &mut backward_builder,
                                &mut primal_bindings,
                                &self.graph,
                                token_ids_id,
                            )?;
                            let token_embedding = primal_placeholder(
                                &mut backward_builder,
                                &mut primal_bindings,
                                &self.graph,
                                token_embedding_id,
                            )?;
                            let contribution = backward_builder
                                .parameter_golf_token_embedding_lookup_backward(
                                    &token_ids,
                                    &token_embedding,
                                    &current_gradient,
                                )
                                .map_err(map_graph_error)?;
                            accumulate_gradient(
                                &mut backward_builder,
                                &mut gradients,
                                token_embedding_id,
                                contribution,
                            )?;
                        }
                    }
                    BackendExtensionOp::ParameterGolfBankedLinear { .. } => {
                        let input_id = node.inputs()[0];
                        let matrix_bank_id = node.inputs()[1];
                        if self.requires_grad(input_id) {
                            let matrix_bank = primal_placeholder(
                                &mut backward_builder,
                                &mut primal_bindings,
                                &self.graph,
                                matrix_bank_id,
                            )?;
                            let contribution = backward_builder
                                .parameter_golf_banked_linear_input_backward(
                                    &current_gradient,
                                    &matrix_bank,
                                    match node.op() {
                                        OpKind::BackendExtension {
                                            op: BackendExtensionOp::ParameterGolfBankedLinear {
                                                bank_index,
                                            },
                                        } => *bank_index,
                                        _ => unreachable!(),
                                    },
                                )
                                .map_err(map_graph_error)?;
                            accumulate_gradient(
                                &mut backward_builder,
                                &mut gradients,
                                input_id,
                                contribution,
                            )?;
                        }
                        if self.requires_grad(matrix_bank_id) {
                            let input = primal_placeholder(
                                &mut backward_builder,
                                &mut primal_bindings,
                                &self.graph,
                                input_id,
                            )?;
                            let matrix_bank = primal_placeholder(
                                &mut backward_builder,
                                &mut primal_bindings,
                                &self.graph,
                                matrix_bank_id,
                            )?;
                            let contribution = backward_builder
                                .parameter_golf_banked_linear_weight_backward(
                                    &input,
                                    &matrix_bank,
                                    &current_gradient,
                                    match node.op() {
                                        OpKind::BackendExtension {
                                            op: BackendExtensionOp::ParameterGolfBankedLinear {
                                                bank_index,
                                            },
                                        } => *bank_index,
                                        _ => unreachable!(),
                                    },
                                )
                                .map_err(map_graph_error)?;
                            accumulate_gradient(
                                &mut backward_builder,
                                &mut gradients,
                                matrix_bank_id,
                                contribution,
                            )?;
                        }
                    }
                    BackendExtensionOp::ParameterGolfProjectionLoss { logit_softcap } => {
                        let logits_id = node.inputs()[0];
                        let target_ids_id = node.inputs()[1];
                        if self.requires_grad(target_ids_id) {
                            return Err(AutodiffError::UnsupportedGradientOp {
                                tensor_id: node.tensor().id(),
                                op: String::from("parameter_golf_projection_loss_target_ids"),
                            });
                        }
                        if self.requires_grad(logits_id) {
                            let logits = primal_placeholder(
                                &mut backward_builder,
                                &mut primal_bindings,
                                &self.graph,
                                logits_id,
                            )?;
                            let target_ids = primal_placeholder(
                                &mut backward_builder,
                                &mut primal_bindings,
                                &self.graph,
                                target_ids_id,
                            )?;
                            let contribution = backward_builder
                                .parameter_golf_projection_loss_backward(
                                    &logits,
                                    &target_ids,
                                    &current_gradient,
                                    logit_softcap.to_f32(),
                                )
                                .map_err(map_graph_error)?;
                            accumulate_gradient(
                                &mut backward_builder,
                                &mut gradients,
                                logits_id,
                                contribution,
                            )?;
                        }
                    }
                    BackendExtensionOp::RmsNorm { epsilon } => {
                        let input_id = node.inputs()[0];
                        let weight_id = node.inputs()[1];
                        let input = primal_placeholder(
                            &mut backward_builder,
                            &mut primal_bindings,
                            &self.graph,
                            input_id,
                        )?;
                        if self.requires_grad(input_id) {
                            let weight = primal_placeholder(
                                &mut backward_builder,
                                &mut primal_bindings,
                                &self.graph,
                                weight_id,
                            )?;
                            let contribution = backward_builder
                                .rms_norm_input_backward(
                                    &input,
                                    &weight,
                                    &current_gradient,
                                    epsilon.to_f32(),
                                )
                                .map_err(map_graph_error)?;
                            accumulate_gradient(
                                &mut backward_builder,
                                &mut gradients,
                                input_id,
                                contribution,
                            )?;
                        }
                        if self.requires_grad(weight_id) {
                            let contribution = backward_builder
                                .rms_norm_weight_backward(
                                    &input,
                                    &current_gradient,
                                    epsilon.to_f32(),
                                )
                                .map_err(map_graph_error)?;
                            accumulate_gradient(
                                &mut backward_builder,
                                &mut gradients,
                                weight_id,
                                contribution,
                            )?;
                        }
                    }
                    BackendExtensionOp::RotaryEmbedding { interleaved } => {
                        let input_id = node.inputs()[0];
                        let cos_id = node.inputs()[1];
                        let sin_id = node.inputs()[2];
                        if self.requires_grad(cos_id) || self.requires_grad(sin_id) {
                            return Err(AutodiffError::UnsupportedGradientOp {
                                tensor_id: node.tensor().id(),
                                op: String::from("rotary_embedding_table_gradients"),
                            });
                        }
                        if self.requires_grad(input_id) {
                            let cos = primal_placeholder(
                                &mut backward_builder,
                                &mut primal_bindings,
                                &self.graph,
                                cos_id,
                            )?;
                            let sin = primal_placeholder(
                                &mut backward_builder,
                                &mut primal_bindings,
                                &self.graph,
                                sin_id,
                            )?;
                            let contribution = backward_builder
                                .rotary_embedding_backward(
                                    &current_gradient,
                                    &cos,
                                    &sin,
                                    *interleaved,
                                )
                                .map_err(map_graph_error)?;
                            accumulate_gradient(
                                &mut backward_builder,
                                &mut gradients,
                                input_id,
                                contribution,
                            )?;
                        }
                    }
                    BackendExtensionOp::ScaledDotProductAttention { scale, causal } => {
                        let current_gradient = backward_builder
                            .reshape(&current_gradient, node.tensor().spec().shape().clone())
                            .map_err(map_graph_error)?;
                        let query_id = node.inputs()[0];
                        let key_id = node.inputs()[1];
                        let value_id = node.inputs()[2];
                        let query = primal_placeholder(
                            &mut backward_builder,
                            &mut primal_bindings,
                            &self.graph,
                            query_id,
                        )?;
                        let key = primal_placeholder(
                            &mut backward_builder,
                            &mut primal_bindings,
                            &self.graph,
                            key_id,
                        )?;
                        let value = primal_placeholder(
                            &mut backward_builder,
                            &mut primal_bindings,
                            &self.graph,
                            value_id,
                        )?;
                        if self.requires_grad(query_id) {
                            let contribution = backward_builder
                                .scaled_dot_product_attention_query_backward(
                                    &query,
                                    &key,
                                    &value,
                                    &current_gradient,
                                    scale.to_f32(),
                                    *causal,
                                )
                                .map_err(map_graph_error)?;
                            accumulate_gradient(
                                &mut backward_builder,
                                &mut gradients,
                                query_id,
                                contribution,
                            )?;
                        }
                        if self.requires_grad(key_id) {
                            let contribution = backward_builder
                                .scaled_dot_product_attention_key_backward(
                                    &query,
                                    &key,
                                    &value,
                                    &current_gradient,
                                    scale.to_f32(),
                                    *causal,
                                )
                                .map_err(map_graph_error)?;
                            accumulate_gradient(
                                &mut backward_builder,
                                &mut gradients,
                                key_id,
                                contribution,
                            )?;
                        }
                        if self.requires_grad(value_id) {
                            let contribution = backward_builder
                                .scaled_dot_product_attention_value_backward(
                                    &query,
                                    &key,
                                    &value,
                                    &current_gradient,
                                    scale.to_f32(),
                                    *causal,
                                )
                                .map_err(map_graph_error)?;
                            accumulate_gradient(
                                &mut backward_builder,
                                &mut gradients,
                                value_id,
                                contribution,
                            )?;
                        }
                    }
                    _ => unreachable!(
                        "unsupported backend extensions should have been rejected by the autodiff support matrix"
                    ),
                },
            }
        }

        let gradient_targets = self
            .graph
            .nodes()
            .iter()
            .filter_map(|node| {
                let tensor_id = node.tensor().id();
                self.requires_grad(tensor_id)
                    .then_some((tensor_id, gradients.get(&tensor_id).cloned()))
            })
            .filter_map(|(tensor_id, gradient_tensor)| {
                gradient_tensor.map(|gradient_tensor| AutodiffGradientTarget {
                    primal_tensor: tensor_id,
                    gradient_tensor: gradient_tensor.id(),
                })
            })
            .collect::<Vec<_>>();
        let gradient_outputs = gradient_targets
            .iter()
            .filter_map(|target| gradients.get(&target.primal_tensor).cloned())
            .scan(BTreeSet::new(), |seen, tensor| {
                seen.insert(tensor.id()).then_some(tensor)
            })
            .collect::<Vec<_>>();
        let primal_bindings = primal_bindings
            .into_iter()
            .map(
                |(primal_tensor, gradient_graph_input)| AutodiffPrimalBinding {
                    primal_tensor,
                    gradient_graph_input: gradient_graph_input.id(),
                },
            )
            .collect::<Vec<_>>();

        Ok(AutodiffBackwardPlan {
            gradient_graph: backward_builder.finish(gradient_outputs),
            primal_bindings,
            seed_input: seed.id(),
            gradient_targets,
        })
    }

    /// Materializes gradients for one graph output with the default scalar seed.
    pub fn backward_materialized(
        &self,
        output: TensorId,
        inputs: &BTreeMap<TensorId, TensorData>,
    ) -> Result<AutodiffBackwardResult, AutodiffError> {
        self.backward_materialized_with_seed(output, inputs, None)
    }

    /// Materializes gradients for one graph output using an explicit upstream seed when needed.
    pub fn backward_materialized_with_seed(
        &self,
        output: TensorId,
        inputs: &BTreeMap<TensorId, TensorData>,
        seed: Option<TensorData>,
    ) -> Result<AutodiffBackwardResult, AutodiffError> {
        let plan = self.backward_plan(output)?;
        let seed = resolve_backward_seed(self, output, seed)?;

        let forward_values = evaluate_graph(&self.graph, inputs)?;
        let mut backward_inputs = BTreeMap::new();
        for binding in &plan.primal_bindings {
            let value =
                forward_values
                    .get(&binding.primal_tensor)
                    .ok_or(AutodiffError::UnknownTensor {
                        tensor_id: binding.primal_tensor,
                    })?;
            backward_inputs.insert(binding.gradient_graph_input, value.clone());
        }
        backward_inputs.insert(plan.seed_input, seed);

        let backward_values = evaluate_graph(&plan.gradient_graph, &backward_inputs)?;
        let gradients = plan
            .gradient_targets
            .iter()
            .filter_map(|target| {
                backward_values
                    .get(&target.gradient_tensor)
                    .cloned()
                    .map(|gradient| (target.primal_tensor, gradient))
            })
            .collect::<BTreeMap<_, _>>();

        Ok(AutodiffBackwardResult {
            forward_values,
            plan,
            gradients,
        })
    }
}

fn validate_reverse_mode_transform_signature(
    graph: &AutodiffGraph,
    output: TensorId,
    primal_targets: &[TensorId],
    singleton_transform: Option<&str>,
) -> Result<ReverseModeTransformSignature, ReverseModeTransformError> {
    let output_node = graph
        .graph()
        .node(output)
        .ok_or(ReverseModeTransformError::UnknownTensor { tensor_id: output })?;
    ensure_supported_transform_target(output_node.tensor())?;
    if let Some(transform) = singleton_transform {
        if output_node.tensor().spec().shape().element_count() != 1 {
            return Err(ReverseModeTransformError::NonSingletonOutput {
                transform: String::from(transform),
                tensor_id: output,
                shape: output_node.tensor().spec().shape().clone(),
            });
        }
    }

    for target in primal_targets {
        let node = graph
            .graph()
            .node(*target)
            .ok_or(ReverseModeTransformError::UnknownTensor { tensor_id: *target })?;
        if !graph.requires_grad(*target) {
            return Err(ReverseModeTransformError::UntrackedTarget { tensor_id: *target });
        }
        ensure_supported_transform_target(node.tensor())?;
    }

    Ok(ReverseModeTransformSignature {
        output,
        primal_targets: primal_targets.to_vec(),
    })
}

fn stable_reverse_mode_transform_signature_digest(
    signature: &ReverseModeTransformSignature,
) -> String {
    let mut lines = vec![format!("output={}", signature.output)];
    for target in &signature.primal_targets {
        lines.push(format!("target={target}"));
    }
    let mut digest = sha2::Sha256::new();
    for line in lines {
        digest.update(line.as_bytes());
        digest.update(b"\n");
    }
    format!("{:x}", digest.finalize())
}

fn ensure_supported_transform_target(tensor: &Tensor) -> Result<(), ReverseModeTransformError> {
    if tensor.spec().dtype() == DType::F32 {
        Ok(())
    } else {
        Err(ReverseModeTransformError::UnsupportedTargetDType {
            tensor_id: tensor.id(),
            dtype: tensor.spec().dtype(),
        })
    }
}

fn collect_requested_gradients(
    graph: &AutodiffGraph,
    backward_result: &AutodiffBackwardResult,
    primal_targets: &[TensorId],
) -> Result<BTreeMap<TensorId, TensorData>, ReverseModeTransformError> {
    primal_targets
        .iter()
        .map(|target| {
            let gradient = if let Some(gradient) = backward_result.gradient(*target) {
                gradient.clone()
            } else {
                zero_gradient_for_target(graph, *target)?
            };
            Ok((*target, gradient))
        })
        .collect()
}

fn collect_requested_gradients_from_plan(
    graph: &AutodiffGraph,
    plan: &AutodiffBackwardPlan,
    backward_values: &BTreeMap<TensorId, TensorData>,
    primal_targets: &[TensorId],
) -> Result<BTreeMap<TensorId, TensorData>, ReverseModeTransformError> {
    primal_targets
        .iter()
        .map(|target| {
            let gradient = if let Some(gradient_tensor) = plan.gradient_for(*target) {
                backward_values
                    .get(&gradient_tensor)
                    .cloned()
                    .unwrap_or(zero_gradient_for_target(graph, *target)?)
            } else {
                zero_gradient_for_target(graph, *target)?
            };
            Ok((*target, gradient))
        })
        .collect()
}

fn zero_gradient_for_target(
    graph: &AutodiffGraph,
    tensor_id: TensorId,
) -> Result<TensorData, ReverseModeTransformError> {
    let node = graph
        .graph()
        .node(tensor_id)
        .ok_or(ReverseModeTransformError::UnknownTensor { tensor_id })?;
    Ok(TensorData::F32(vec![
        0.0;
        node.tensor()
            .spec()
            .shape()
            .element_count()
    ]))
}

fn validate_jvp_transform_signature(
    graph: &AutodiffGraph,
    output: TensorId,
    primal_targets: &[TensorId],
) -> Result<JvpTransformSignature, ForwardModeTransformError> {
    let output_node = graph
        .graph()
        .node(output)
        .ok_or(ForwardModeTransformError::UnknownTensor { tensor_id: output })?;
    ensure_supported_forward_mode_target(output_node.tensor())?;

    for target in primal_targets {
        let node = graph
            .graph()
            .node(*target)
            .ok_or(ForwardModeTransformError::UnknownTensor { tensor_id: *target })?;
        if !graph.requires_grad(*target) {
            return Err(ForwardModeTransformError::UntrackedTarget { tensor_id: *target });
        }
        ensure_supported_forward_mode_target(node.tensor())?;
    }

    Ok(JvpTransformSignature {
        output,
        primal_targets: primal_targets.to_vec(),
    })
}

fn ensure_supported_forward_mode_target(tensor: &Tensor) -> Result<(), ForwardModeTransformError> {
    if tensor.spec().dtype() == DType::F32 {
        Ok(())
    } else {
        Err(ForwardModeTransformError::UnsupportedTargetDType {
            tensor_id: tensor.id(),
            dtype: tensor.spec().dtype(),
        })
    }
}

fn validate_vmap_transform_signature(
    graph: &AutodiffGraph,
    output: TensorId,
    mapped_inputs: &[VmapInputBinding],
    output_axis: usize,
) -> Result<VmapTransformSignature, VmapTransformError> {
    for node in graph.graph().nodes() {
        if let VmapSupport::Unsupported { .. } = vmap_support_for_op(node.op()) {
            return Err(VmapTransformError::UnsupportedOp {
                tensor_id: node.tensor().id(),
                op: String::from(node.op().label()),
            });
        }
    }

    let output_node = graph
        .graph()
        .node(output)
        .ok_or(VmapTransformError::UnknownTensor { tensor_id: output })?;
    ensure_supported_vmap_target(output_node.tensor())?;
    if output_axis > output_node.tensor().spec().shape().rank() {
        return Err(VmapTransformError::InvalidOutputAxis {
            tensor_id: output,
            axis: output_axis,
            rank: output_node.tensor().spec().shape().rank(),
        });
    }
    if mapped_inputs.is_empty() {
        return Err(VmapTransformError::MissingMappedInputs);
    }

    let mut seen = BTreeSet::new();
    for binding in mapped_inputs {
        let node = graph
            .graph()
            .node(binding.input)
            .ok_or(VmapTransformError::UnknownTensor {
                tensor_id: binding.input,
            })?;
        if !matches!(node.op(), OpKind::Input { .. }) {
            return Err(VmapTransformError::NonInputTarget {
                tensor_id: binding.input,
            });
        }
        if !seen.insert(binding.input) {
            return Err(VmapTransformError::DuplicateMappedInput {
                tensor_id: binding.input,
            });
        }
        ensure_supported_vmap_target(node.tensor())?;
        if binding.axis > node.tensor().spec().shape().rank() {
            return Err(VmapTransformError::InvalidInputAxis {
                tensor_id: binding.input,
                axis: binding.axis,
                rank: node.tensor().spec().shape().rank(),
            });
        }
    }

    Ok(VmapTransformSignature {
        output,
        mapped_inputs: mapped_inputs.to_vec(),
        output_axis,
    })
}

fn ensure_supported_vmap_target(tensor: &Tensor) -> Result<(), VmapTransformError> {
    if tensor.spec().dtype() == DType::F32 {
        Ok(())
    } else {
        Err(VmapTransformError::UnsupportedTargetDType {
            tensor_id: tensor.id(),
            dtype: tensor.spec().dtype(),
        })
    }
}

fn validate_custom_vjp_cotangents(
    graph: &AutodiffGraph,
    signature: &ReverseModeTransformSignature,
    cotangents: BTreeMap<TensorId, TensorData>,
) -> Result<BTreeMap<TensorId, TensorData>, CustomVjpTransformError> {
    for tensor_id in cotangents.keys() {
        if !signature.primal_targets.contains(tensor_id) {
            return Err(CustomVjpTransformError::UnexpectedCotangentTarget {
                tensor_id: *tensor_id,
            });
        }
    }

    let mut validated = BTreeMap::new();
    for tensor_id in &signature.primal_targets {
        let cotangent =
            cotangents
                .get(tensor_id)
                .ok_or(CustomVjpTransformError::MissingCotangentTarget {
                    tensor_id: *tensor_id,
                })?;
        let Some(values) = cotangent.as_f32_slice() else {
            return Err(CustomVjpTransformError::CotangentDenseF32Required {
                tensor_id: *tensor_id,
            });
        };
        let node =
            graph
                .graph()
                .node(*tensor_id)
                .ok_or(ReverseModeTransformError::UnknownTensor {
                    tensor_id: *tensor_id,
                })?;
        let expected_len = node.tensor().spec().shape().element_count();
        if values.len() != expected_len {
            return Err(CustomVjpTransformError::CotangentLengthMismatch {
                tensor_id: *tensor_id,
                expected_len,
                actual_len: values.len(),
            });
        }
        validated.insert(*tensor_id, TensorData::F32(values.to_vec()));
    }
    Ok(validated)
}

#[derive(Clone, Debug, PartialEq)]
struct ValidatedMappedVmapInput {
    values: Vec<f32>,
    batched_shape: Shape,
}

fn validate_and_seed_vmap_inputs(
    graph: &AutodiffGraph,
    signature: &VmapTransformSignature,
    inputs: &BTreeMap<TensorId, TensorData>,
) -> Result<(usize, BTreeMap<TensorId, ValidatedMappedVmapInput>), VmapTransformError> {
    let mut batch_size = None;
    let mut validated = BTreeMap::new();
    for binding in &signature.mapped_inputs {
        let input = inputs
            .get(&binding.input)
            .ok_or(VmapTransformError::MissingMappedInput {
                tensor_id: binding.input,
            })?;
        let values =
            input
                .as_f32_slice()
                .ok_or(VmapTransformError::MappedInputDenseF32Required {
                    tensor_id: binding.input,
                })?;
        let node = graph
            .graph()
            .node(binding.input)
            .ok_or(VmapTransformError::UnknownTensor {
                tensor_id: binding.input,
            })?;
        let lane_shape = node.tensor().spec().shape();
        let lane_len = lane_shape.element_count();
        let current_batch_size = if lane_len == 0 {
            if !values.is_empty() {
                return Err(VmapTransformError::MappedInputLengthMismatch {
                    tensor_id: binding.input,
                    lane_len,
                    actual_len: values.len(),
                });
            }
            0
        } else if values.len() % lane_len != 0 {
            return Err(VmapTransformError::MappedInputLengthMismatch {
                tensor_id: binding.input,
                lane_len,
                actual_len: values.len(),
            });
        } else {
            values.len() / lane_len
        };
        if let Some(expected_batch_size) = batch_size {
            if expected_batch_size != current_batch_size {
                return Err(VmapTransformError::BatchSizeMismatch {
                    tensor_id: binding.input,
                    expected_batch_size,
                    actual_batch_size: current_batch_size,
                });
            }
        } else {
            batch_size = Some(current_batch_size);
        }
        validated.insert(
            binding.input,
            ValidatedMappedVmapInput {
                values: values.to_vec(),
                batched_shape: Shape::new(insert_axis(
                    lane_shape.dims(),
                    binding.axis,
                    current_batch_size,
                )),
            },
        );
    }
    Ok((batch_size.unwrap_or(0), validated))
}

fn stack_vmap_lane_outputs(
    lane_outputs: &[TensorData],
    lane_shape: &Shape,
    output_axis: usize,
    output: TensorId,
) -> Result<TensorData, VmapTransformError> {
    let stacked = lane_outputs
        .iter()
        .map(|lane_output| {
            lane_output.as_f32_slice().map(<[f32]>::to_vec).ok_or(
                VmapTransformError::UnsupportedTargetDType {
                    tensor_id: output,
                    dtype: DType::F32,
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TensorData::F32(stack_values(
        stacked.as_slice(),
        lane_shape,
        output_axis,
    )))
}

fn validate_and_seed_tangent_inputs(
    graph: &AutodiffGraph,
    signature: &JvpTransformSignature,
    tangents: &BTreeMap<TensorId, TensorData>,
) -> Result<BTreeMap<TensorId, TensorData>, ForwardModeTransformError> {
    for tensor_id in tangents.keys() {
        if !signature.primal_targets.contains(tensor_id) {
            return Err(ForwardModeTransformError::UnexpectedTangentTarget {
                tensor_id: *tensor_id,
            });
        }
    }

    let mut validated = BTreeMap::new();
    for tensor_id in &signature.primal_targets {
        let tangent =
            tangents
                .get(tensor_id)
                .ok_or(ForwardModeTransformError::MissingTangentTarget {
                    tensor_id: *tensor_id,
                })?;
        let Some(values) = tangent.as_f32_slice() else {
            return Err(ForwardModeTransformError::TangentDenseF32Required {
                tensor_id: *tensor_id,
            });
        };
        let node =
            graph
                .graph()
                .node(*tensor_id)
                .ok_or(ForwardModeTransformError::UnknownTensor {
                    tensor_id: *tensor_id,
                })?;
        let expected_len = node.tensor().spec().shape().element_count();
        if values.len() != expected_len {
            return Err(ForwardModeTransformError::TangentLengthMismatch {
                tensor_id: *tensor_id,
                expected_len,
                actual_len: values.len(),
            });
        }
        validated.insert(*tensor_id, TensorData::F32(values.to_vec()));
    }
    Ok(validated)
}

fn evaluate_graph_forward_mode(
    graph: &Graph,
    inputs: &BTreeMap<TensorId, TensorData>,
    tangents: &BTreeMap<TensorId, TensorData>,
) -> Result<
    (
        BTreeMap<TensorId, TensorData>,
        BTreeMap<TensorId, TensorData>,
    ),
    ForwardModeTransformError,
> {
    let mut forward_values = BTreeMap::new();
    let mut tangent_values = BTreeMap::new();

    for node in graph.nodes() {
        let forward = match node.op() {
            OpKind::Input { .. } => inputs.get(&node.tensor().id()).cloned().ok_or(
                ReferenceEvaluationError::MissingInput {
                    tensor_id: node.tensor().id(),
                },
            )?,
            OpKind::Constant { data } => data.clone(),
            OpKind::Detach => forward_values.get(&node.inputs()[0]).cloned().ok_or(
                ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                },
            )?,
            OpKind::Add => {
                let left = resolve_dense_input(
                    graph,
                    &forward_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                let right = resolve_dense_input(
                    graph,
                    &forward_values,
                    node.inputs()[1],
                    node.op().label(),
                )?;
                TensorData::F32(
                    left.iter()
                        .zip(right.iter())
                        .map(|(left, right)| left + right)
                        .collect(),
                )
            }
            OpKind::Mul => {
                let left = resolve_dense_input(
                    graph,
                    &forward_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                let right = resolve_dense_input(
                    graph,
                    &forward_values,
                    node.inputs()[1],
                    node.op().label(),
                )?;
                TensorData::F32(
                    left.iter()
                        .zip(right.iter())
                        .map(|(left, right)| left * right)
                        .collect(),
                )
            }
            OpKind::Matmul => {
                let left_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let right_node = graph.node(node.inputs()[1]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[1],
                    },
                )?;
                let left = resolve_dense_input(
                    graph,
                    &forward_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                let right = resolve_dense_input(
                    graph,
                    &forward_values,
                    node.inputs()[1],
                    node.op().label(),
                )?;
                TensorData::F32(matmul_values(
                    left,
                    left_node.tensor().spec().shape(),
                    right,
                    right_node.tensor().spec().shape(),
                ))
            }
            OpKind::Reshape => forward_values.get(&node.inputs()[0]).cloned().ok_or(
                ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                },
            )?,
            OpKind::Permute { axes } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input = resolve_dense_input(
                    graph,
                    &forward_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                TensorData::F32(permute_values(
                    input,
                    input_node.tensor().spec().shape(),
                    axes,
                ))
            }
            OpKind::Slice { axis, start, end } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input = resolve_dense_input(
                    graph,
                    &forward_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                TensorData::F32(slice_values(
                    input,
                    input_node.tensor().spec().shape(),
                    *axis,
                    *start,
                    *end,
                ))
            }
            OpKind::Select { axis, index } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input = resolve_dense_input(
                    graph,
                    &forward_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                TensorData::F32(select_values(
                    input,
                    input_node.tensor().spec().shape(),
                    *axis,
                    *index,
                ))
            }
            OpKind::Concat { axis } => {
                let mut parts = Vec::with_capacity(node.inputs().len());
                for input_id in node.inputs() {
                    let input_node =
                        graph
                            .node(*input_id)
                            .ok_or(ReferenceEvaluationError::UnknownTensor {
                                tensor_id: *input_id,
                            })?;
                    let input =
                        resolve_dense_input(graph, &forward_values, *input_id, node.op().label())?;
                    parts.push((input_node.tensor().spec().shape().clone(), input.to_vec()));
                }
                TensorData::F32(concat_values(parts.as_slice(), *axis))
            }
            OpKind::Expand { shape } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input = resolve_dense_input(
                    graph,
                    &forward_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                TensorData::F32(expand_values(
                    input,
                    input_node.tensor().spec().shape(),
                    shape,
                ))
            }
            OpKind::Cast { .. } | OpKind::BackendExtension { .. } => {
                return Err(ForwardModeTransformError::UnsupportedOp {
                    tensor_id: node.tensor().id(),
                    op: String::from(node.op().label()),
                });
            }
            OpKind::ReduceSum { axis } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input = resolve_dense_input(
                    graph,
                    &forward_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                TensorData::F32(reduce_sum_values(
                    input,
                    input_node.tensor().spec().shape(),
                    *axis,
                ))
            }
        };
        validate_output_length(node.tensor(), &forward)?;
        forward_values.insert(node.tensor().id(), forward);

        let tangent = match node.op() {
            OpKind::Input { .. } => tangents
                .get(&node.tensor().id())
                .cloned()
                .unwrap_or_else(|| zero_tensor_like(node.tensor())),
            OpKind::Constant { .. } | OpKind::Detach => zero_tensor_like(node.tensor()),
            OpKind::Add => {
                let left = resolve_dense_input(
                    graph,
                    &tangent_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                let right = resolve_dense_input(
                    graph,
                    &tangent_values,
                    node.inputs()[1],
                    node.op().label(),
                )?;
                TensorData::F32(
                    left.iter()
                        .zip(right.iter())
                        .map(|(left, right)| left + right)
                        .collect(),
                )
            }
            OpKind::Mul => {
                let left_primal = resolve_dense_input(
                    graph,
                    &forward_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                let right_primal = resolve_dense_input(
                    graph,
                    &forward_values,
                    node.inputs()[1],
                    node.op().label(),
                )?;
                let left_tangent = resolve_dense_input(
                    graph,
                    &tangent_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                let right_tangent = resolve_dense_input(
                    graph,
                    &tangent_values,
                    node.inputs()[1],
                    node.op().label(),
                )?;
                TensorData::F32(
                    left_tangent
                        .iter()
                        .zip(right_primal.iter())
                        .zip(left_primal.iter().zip(right_tangent.iter()))
                        .map(
                            |((left_tangent, right_primal), (left_primal, right_tangent))| {
                                (left_tangent * right_primal) + (left_primal * right_tangent)
                            },
                        )
                        .collect(),
                )
            }
            OpKind::Matmul => {
                let left_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let right_node = graph.node(node.inputs()[1]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[1],
                    },
                )?;
                let left_primal = resolve_dense_input(
                    graph,
                    &forward_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                let right_primal = resolve_dense_input(
                    graph,
                    &forward_values,
                    node.inputs()[1],
                    node.op().label(),
                )?;
                let left_tangent = resolve_dense_input(
                    graph,
                    &tangent_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                let right_tangent = resolve_dense_input(
                    graph,
                    &tangent_values,
                    node.inputs()[1],
                    node.op().label(),
                )?;
                let left_term = matmul_values(
                    left_tangent,
                    left_node.tensor().spec().shape(),
                    right_primal,
                    right_node.tensor().spec().shape(),
                );
                let right_term = matmul_values(
                    left_primal,
                    left_node.tensor().spec().shape(),
                    right_tangent,
                    right_node.tensor().spec().shape(),
                );
                TensorData::F32(
                    left_term
                        .iter()
                        .zip(right_term.iter())
                        .map(|(left, right)| left + right)
                        .collect(),
                )
            }
            OpKind::Reshape => tangent_values.get(&node.inputs()[0]).cloned().ok_or(
                ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                },
            )?,
            OpKind::Permute { axes } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input = resolve_dense_input(
                    graph,
                    &tangent_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                TensorData::F32(permute_values(
                    input,
                    input_node.tensor().spec().shape(),
                    axes,
                ))
            }
            OpKind::Slice { axis, start, end } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input = resolve_dense_input(
                    graph,
                    &tangent_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                TensorData::F32(slice_values(
                    input,
                    input_node.tensor().spec().shape(),
                    *axis,
                    *start,
                    *end,
                ))
            }
            OpKind::Select { axis, index } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input = resolve_dense_input(
                    graph,
                    &tangent_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                TensorData::F32(select_values(
                    input,
                    input_node.tensor().spec().shape(),
                    *axis,
                    *index,
                ))
            }
            OpKind::Concat { axis } => {
                let mut parts = Vec::with_capacity(node.inputs().len());
                for input_id in node.inputs() {
                    let input_node =
                        graph
                            .node(*input_id)
                            .ok_or(ReferenceEvaluationError::UnknownTensor {
                                tensor_id: *input_id,
                            })?;
                    let input =
                        resolve_dense_input(graph, &tangent_values, *input_id, node.op().label())?;
                    parts.push((input_node.tensor().spec().shape().clone(), input.to_vec()));
                }
                TensorData::F32(concat_values(parts.as_slice(), *axis))
            }
            OpKind::Expand { shape } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input = resolve_dense_input(
                    graph,
                    &tangent_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                TensorData::F32(expand_values(
                    input,
                    input_node.tensor().spec().shape(),
                    shape,
                ))
            }
            OpKind::Cast { .. } | OpKind::BackendExtension { .. } => {
                return Err(ForwardModeTransformError::UnsupportedOp {
                    tensor_id: node.tensor().id(),
                    op: String::from(node.op().label()),
                });
            }
            OpKind::ReduceSum { axis } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input = resolve_dense_input(
                    graph,
                    &tangent_values,
                    node.inputs()[0],
                    node.op().label(),
                )?;
                TensorData::F32(reduce_sum_values(
                    input,
                    input_node.tensor().spec().shape(),
                    *axis,
                ))
            }
        };
        validate_output_length(node.tensor(), &tangent)?;
        tangent_values.insert(node.tensor().id(), tangent);
    }

    Ok((forward_values, tangent_values))
}

fn zero_tensor_like(tensor: &Tensor) -> TensorData {
    TensorData::F32(vec![0.0; tensor.spec().shape().element_count()])
}

#[derive(Clone, Debug, PartialEq)]
struct RetainedEvaluationResult {
    values: BTreeMap<TensorId, TensorData>,
    summary: RetainedEvaluationSummary,
}

fn resolve_backward_seed(
    graph: &AutodiffGraph,
    output: TensorId,
    seed: Option<TensorData>,
) -> Result<TensorData, AutodiffError> {
    let output_node = graph
        .graph()
        .node(output)
        .ok_or(AutodiffError::UnknownTensor { tensor_id: output })?;
    let output_len = output_node.tensor().spec().shape().element_count();
    let seed = match seed {
        Some(seed) => seed,
        None if output_len == 1 => match output_node.tensor().spec().dtype() {
            DType::BF16 => TensorData::BF16(vec![1.0]),
            _ => TensorData::F32(vec![1.0]),
        },
        None => {
            return Err(AutodiffError::NonScalarOutputRequiresSeed {
                tensor_id: output,
                shape: output_node.tensor().spec().shape().clone(),
            });
        }
    };
    let Some(seed_values) = seed.as_f32_slice() else {
        return Err(AutodiffError::SeedDenseF32Required { tensor_id: output });
    };
    if seed_values.len() != output_len {
        return Err(AutodiffError::SeedLengthMismatch {
            tensor_id: output,
            expected_len: output_len,
            actual_len: seed_values.len(),
        });
    }
    Ok(seed)
}

/// Autodiff-aware wrapper over the canonical graph builder.
#[derive(Clone, Debug)]
pub struct AutodiffGraphBuilder {
    builder: GraphBuilder,
    context: AutodiffContext,
    gradient_tracking: BTreeMap<TensorId, bool>,
}

impl AutodiffGraphBuilder {
    /// Creates a builder in default training mode.
    #[must_use]
    pub fn new(device: Device) -> Self {
        Self::with_context(device, AutodiffContext::training())
    }

    /// Creates a builder with an explicit autodiff context.
    #[must_use]
    pub fn with_context(device: Device, context: AutodiffContext) -> Self {
        Self {
            builder: GraphBuilder::new(device),
            context,
            gradient_tracking: BTreeMap::new(),
        }
    }

    /// Returns the current autodiff context.
    #[must_use]
    pub const fn context(&self) -> AutodiffContext {
        self.context
    }

    /// Replaces the current autodiff context for subsequently created tensors.
    pub fn set_context(&mut self, context: AutodiffContext) {
        self.context = context;
    }

    /// Adds a named input tensor.
    pub fn input(
        &mut self,
        name: impl Into<String>,
        shape: Shape,
        dtype: DType,
        requires_grad: bool,
    ) -> AutodiffTensor {
        let tensor = self.builder.input(name, shape, dtype);
        self.wrap(tensor, self.context.gradients_active() && requires_grad)
    }

    /// Adds a dense `f32` constant.
    pub fn constant_f32(
        &mut self,
        shape: Shape,
        values: impl Into<Vec<f32>>,
    ) -> Result<AutodiffTensor, GraphError> {
        let tensor = self.builder.constant_f32(shape, values)?;
        Ok(self.wrap(tensor, false))
    }

    /// Adds a quantized GGML/GGUF block constant.
    pub fn constant_quantized_blocks(
        &mut self,
        shape: Shape,
        mode: psionic_core::QuantizationMode,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<AutodiffTensor, GraphError> {
        let tensor = self.builder.constant_quantized_blocks(shape, mode, bytes)?;
        Ok(self.wrap(tensor, false))
    }

    /// Adds two tensors.
    pub fn add(
        &mut self,
        left: &AutodiffTensor,
        right: &AutodiffTensor,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[left, right]);
        let tensor = self.builder.add(left.tensor(), right.tensor())?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Adds a gradient-stopping identity node.
    #[must_use]
    pub fn detach(&mut self, input: &AutodiffTensor) -> AutodiffTensor {
        let tensor = self.builder.detach(input.tensor());
        self.wrap(tensor, false)
    }

    /// Multiplies two tensors elementwise.
    pub fn mul(
        &mut self,
        left: &AutodiffTensor,
        right: &AutodiffTensor,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[left, right]);
        let tensor = self.builder.mul(left.tensor(), right.tensor())?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Matrix multiply for rank-2 tensors.
    pub fn matmul(
        &mut self,
        left: &AutodiffTensor,
        right: &AutodiffTensor,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[left, right]);
        let tensor = self.builder.matmul(left.tensor(), right.tensor())?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Reshapes a tensor.
    pub fn reshape(
        &mut self,
        input: &AutodiffTensor,
        new_shape: Shape,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[input]);
        let tensor = self.builder.reshape(input.tensor(), new_shape)?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Reorders axes using a logical view.
    pub fn permute(
        &mut self,
        input: &AutodiffTensor,
        axes: Vec<usize>,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[input]);
        let tensor = self.builder.permute(input.tensor(), axes)?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Returns a narrowed tensor view.
    pub fn slice(
        &mut self,
        input: &AutodiffTensor,
        axis: usize,
        start: usize,
        end: usize,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[input]);
        let tensor = self.builder.slice(input.tensor(), axis, start, end)?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Returns a view that removes one axis by selecting a single index.
    pub fn select(
        &mut self,
        input: &AutodiffTensor,
        axis: usize,
        index: usize,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[input]);
        let tensor = self.builder.select(input.tensor(), axis, index)?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Concatenates tensors along one axis.
    pub fn concat(
        &mut self,
        inputs: &[AutodiffTensor],
        axis: usize,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad =
            self.context.gradients_active() && inputs.iter().any(AutodiffTensor::requires_grad);
        let tensors = inputs
            .iter()
            .map(|input| input.tensor.clone())
            .collect::<Vec<_>>();
        let tensor = self.builder.concat(tensors.as_slice(), axis)?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Expands a tensor through broadcast semantics.
    pub fn expand(
        &mut self,
        input: &AutodiffTensor,
        shape: Shape,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[input]);
        let tensor = self.builder.expand(input.tensor(), shape)?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Casts a tensor into a different logical dtype.
    pub fn cast(
        &mut self,
        input: &AutodiffTensor,
        dtype: DType,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[input]);
        let tensor = self.builder.cast(input.tensor(), dtype)?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Reduces a tensor to a scalar sum.
    #[must_use]
    pub fn reduce_sum(&mut self, input: &AutodiffTensor) -> AutodiffTensor {
        let requires_grad = self.any_requires_grad(&[input]);
        let tensor = self.builder.reduce_sum(input.tensor());
        self.wrap(tensor, requires_grad)
    }

    /// Reduces a tensor along one axis.
    pub fn reduce_sum_axis(
        &mut self,
        input: &AutodiffTensor,
        axis: usize,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[input]);
        let tensor = self.builder.reduce_sum_axis(input.tensor(), axis)?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Applies ReLU-squared pointwise activation.
    pub fn relu_squared(&mut self, input: &AutodiffTensor) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[input]);
        let tensor = self.builder.relu_squared(input.tensor())?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Applies leaky-ReLU-squared pointwise activation.
    pub fn leaky_relu_squared(
        &mut self,
        input: &AutodiffTensor,
        negative_slope: f32,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[input]);
        let tensor = self
            .builder
            .leaky_relu_squared(input.tensor(), negative_slope)?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Applies SiLU pointwise activation.
    pub fn silu(&mut self, input: &AutodiffTensor) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[input]);
        let tensor = self.builder.silu(input.tensor())?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Applies RMS normalization.
    pub fn rms_norm(
        &mut self,
        input: &AutodiffTensor,
        weight: &AutodiffTensor,
        epsilon: f32,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[input, weight]);
        let tensor = self
            .builder
            .rms_norm(input.tensor(), weight.tensor(), epsilon)?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Applies layer normalization.
    pub fn layer_norm(
        &mut self,
        input: &AutodiffTensor,
        weight: &AutodiffTensor,
        bias: &AutodiffTensor,
        epsilon: f32,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[input, weight, bias]);
        let tensor =
            self.builder
                .layer_norm(input.tensor(), weight.tensor(), bias.tensor(), epsilon)?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Applies RoPE.
    pub fn rope(
        &mut self,
        input: &AutodiffTensor,
        cos: &AutodiffTensor,
        sin: &AutodiffTensor,
        interleaved: bool,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[input, cos, sin]);
        let tensor = self
            .builder
            .rope(input.tensor(), cos.tensor(), sin.tensor(), interleaved)?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Applies scaled dot-product attention.
    pub fn scaled_dot_product_attention(
        &mut self,
        query: &AutodiffTensor,
        key: &AutodiffTensor,
        value: &AutodiffTensor,
        scale: f32,
        causal: bool,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[query, key, value]);
        let tensor = self.builder.scaled_dot_product_attention(
            query.tensor(),
            key.tensor(),
            value.tensor(),
            scale,
            causal,
        )?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Looks up Parameter Golf token embeddings from integer token ids.
    pub fn parameter_golf_token_embedding_lookup(
        &mut self,
        token_ids: &AutodiffTensor,
        token_embedding: &AutodiffTensor,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[token_ids, token_embedding]);
        let tensor = self
            .builder
            .parameter_golf_token_embedding_lookup(token_ids.tensor(), token_embedding.tensor())?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Applies one Parameter Golf linear against a rank-3 matrix bank without
    /// first lowering that bank through a graph-level slice node.
    pub fn parameter_golf_banked_linear(
        &mut self,
        input: &AutodiffTensor,
        matrix_bank: &AutodiffTensor,
        bank_index: usize,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[input, matrix_bank]);
        let tensor = self.builder.parameter_golf_banked_linear(
            input.tensor(),
            matrix_bank.tensor(),
            bank_index,
        )?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Applies the bounded Parameter Golf tanh-softcap next-token mean loss.
    pub fn parameter_golf_projection_loss(
        &mut self,
        pre_softcap_logits: &AutodiffTensor,
        target_ids: &AutodiffTensor,
        logit_softcap: f32,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[pre_softcap_logits, target_ids]);
        let tensor = self.builder.parameter_golf_projection_loss(
            pre_softcap_logits.tensor(),
            target_ids.tensor(),
            logit_softcap,
        )?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Applies the bounded Parameter Golf tanh-softcap next-token loss per
    /// token as `[batch, seq]`.
    pub fn parameter_golf_projection_token_losses(
        &mut self,
        pre_softcap_logits: &AutodiffTensor,
        target_ids: &AutodiffTensor,
        logit_softcap: f32,
    ) -> Result<AutodiffTensor, GraphError> {
        let tensor = self.builder.parameter_golf_projection_token_losses(
            pre_softcap_logits.tensor(),
            target_ids.tensor(),
            logit_softcap,
        )?;
        Ok(self.wrap(tensor, false))
    }

    /// Registers a quantized matmul.
    pub fn quantized_matmul(
        &mut self,
        left: &AutodiffTensor,
        right: &AutodiffTensor,
        rhs_mode: psionic_core::QuantizationMode,
    ) -> Result<AutodiffTensor, GraphError> {
        let requires_grad = self.any_requires_grad(&[left, right]);
        let tensor = self
            .builder
            .quantized_matmul(left.tensor(), right.tensor(), rhs_mode)?;
        Ok(self.wrap(tensor, requires_grad))
    }

    /// Finishes the graph with the provided outputs.
    #[must_use]
    pub fn finish(self, outputs: Vec<AutodiffTensor>) -> AutodiffGraph {
        let graph_outputs = outputs
            .iter()
            .map(|output| output.tensor.clone())
            .collect::<Vec<_>>();
        AutodiffGraph {
            graph: self.builder.finish(graph_outputs),
            context: self.context,
            gradient_tracking: self.gradient_tracking,
        }
    }

    fn any_requires_grad(&self, inputs: &[&AutodiffTensor]) -> bool {
        self.context.gradients_active() && inputs.iter().any(|input| input.requires_grad())
    }

    fn wrap(&mut self, tensor: Tensor, requires_grad: bool) -> AutodiffTensor {
        self.gradient_tracking.insert(tensor.id(), requires_grad);
        AutodiffTensor::new(tensor, requires_grad)
    }
}

fn evaluate_graph_retaining(
    graph: &Graph,
    inputs: &BTreeMap<TensorId, TensorData>,
    retain_tensors: BTreeSet<TensorId>,
) -> Result<RetainedEvaluationResult, ReferenceEvaluationError> {
    let mut values = BTreeMap::new();
    let mut remaining_uses = graph_input_use_counts(graph);
    let mut dropped_tensor_count = 0usize;
    let mut peak_live_tensors = 0usize;

    for node in graph.nodes() {
        let value = match node.op() {
            OpKind::Input { .. } => inputs.get(&node.tensor().id()).cloned().ok_or(
                ReferenceEvaluationError::MissingInput {
                    tensor_id: node.tensor().id(),
                },
            )?,
            OpKind::Constant { data } => data.clone(),
            OpKind::Detach => values.get(&node.inputs()[0]).cloned().ok_or(
                ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                },
            )?,
            OpKind::Add => {
                let left =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                let right =
                    resolve_dense_input(graph, &values, node.inputs()[1], node.op().label())?;
                TensorData::F32(
                    left.iter()
                        .zip(right.iter())
                        .map(|(left, right)| left + right)
                        .collect(),
                )
            }
            OpKind::Mul => {
                let left =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                let right =
                    resolve_dense_input(graph, &values, node.inputs()[1], node.op().label())?;
                TensorData::F32(
                    left.iter()
                        .zip(right.iter())
                        .map(|(left, right)| left * right)
                        .collect(),
                )
            }
            OpKind::Matmul => {
                let left_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let right_node = graph.node(node.inputs()[1]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[1],
                    },
                )?;
                let left =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                let right =
                    resolve_dense_input(graph, &values, node.inputs()[1], node.op().label())?;
                TensorData::F32(matmul_values(
                    left,
                    left_node.tensor().spec().shape(),
                    right,
                    right_node.tensor().spec().shape(),
                ))
            }
            OpKind::Reshape => values.get(&node.inputs()[0]).cloned().ok_or(
                ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                },
            )?,
            OpKind::Permute { axes } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                TensorData::F32(permute_values(
                    input,
                    input_node.tensor().spec().shape(),
                    axes,
                ))
            }
            OpKind::Slice { axis, start, end } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                TensorData::F32(slice_values(
                    input,
                    input_node.tensor().spec().shape(),
                    *axis,
                    *start,
                    *end,
                ))
            }
            OpKind::Select { axis, index } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                TensorData::F32(select_values(
                    input,
                    input_node.tensor().spec().shape(),
                    *axis,
                    *index,
                ))
            }
            OpKind::Concat { axis } => {
                let mut parts = Vec::with_capacity(node.inputs().len());
                for input_id in node.inputs() {
                    let input_node =
                        graph
                            .node(*input_id)
                            .ok_or(ReferenceEvaluationError::UnknownTensor {
                                tensor_id: *input_id,
                            })?;
                    let input = resolve_dense_input(graph, &values, *input_id, node.op().label())?;
                    parts.push((input_node.tensor().spec().shape().clone(), input.to_vec()));
                }
                TensorData::F32(concat_values(parts.as_slice(), *axis))
            }
            OpKind::Expand { shape } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                TensorData::F32(expand_values(
                    input,
                    input_node.tensor().spec().shape(),
                    shape,
                ))
            }
            OpKind::Cast { dtype } => {
                let input =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                match dtype {
                    DType::F32 | DType::F16 | DType::BF16 => TensorData::F32(input.to_vec()),
                    DType::I32 => TensorData::I32(
                        input
                            .iter()
                            .map(|current| {
                                current.round().clamp(i32::MIN as f32, i32::MAX as f32) as i32
                            })
                            .collect(),
                    ),
                    DType::I8 => TensorData::F32(
                        input
                            .iter()
                            .map(|current| current.round().clamp(i8::MIN as f32, i8::MAX as f32))
                            .collect(),
                    ),
                }
            }
            OpKind::ReduceSum { axis } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                TensorData::F32(reduce_sum_values(
                    input,
                    input_node.tensor().spec().shape(),
                    *axis,
                ))
            }
            OpKind::BackendExtension { op } => {
                evaluate_backend_extension_reference(graph, node, &values, op)?
            }
        };
        validate_output_length(node.tensor(), &value)?;
        values.insert(node.tensor().id(), value);
        peak_live_tensors = peak_live_tensors.max(values.len());

        release_consumed_tensors(
            node.inputs(),
            &mut remaining_uses,
            &mut values,
            &retain_tensors,
            &mut dropped_tensor_count,
        );
        release_tensor_if_dead(
            node.tensor().id(),
            &remaining_uses,
            &mut values,
            &retain_tensors,
            &mut dropped_tensor_count,
        );
    }

    let retained_ids = retain_tensors
        .iter()
        .copied()
        .filter(|tensor_id| values.contains_key(tensor_id))
        .collect::<Vec<_>>();
    values.retain(|tensor_id, _| retain_tensors.contains(tensor_id));
    Ok(RetainedEvaluationResult {
        values,
        summary: RetainedEvaluationSummary {
            retained_tensors: retained_ids,
            dropped_tensor_count,
            peak_live_tensors,
        },
    })
}

/// Evaluates a canonical graph through the dense `f32` reference path.
pub fn evaluate_graph(
    graph: &Graph,
    inputs: &BTreeMap<TensorId, TensorData>,
) -> Result<BTreeMap<TensorId, TensorData>, ReferenceEvaluationError> {
    let mut values = BTreeMap::new();
    for node in graph.nodes() {
        let value = match node.op() {
            OpKind::Input { .. } => inputs.get(&node.tensor().id()).cloned().ok_or(
                ReferenceEvaluationError::MissingInput {
                    tensor_id: node.tensor().id(),
                },
            )?,
            OpKind::Constant { data } => data.clone(),
            OpKind::Detach => values.get(&node.inputs()[0]).cloned().ok_or(
                ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                },
            )?,
            OpKind::Add => {
                let left =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                let right =
                    resolve_dense_input(graph, &values, node.inputs()[1], node.op().label())?;
                TensorData::F32(
                    left.iter()
                        .zip(right.iter())
                        .map(|(left, right)| left + right)
                        .collect(),
                )
            }
            OpKind::Mul => {
                let left =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                let right =
                    resolve_dense_input(graph, &values, node.inputs()[1], node.op().label())?;
                TensorData::F32(
                    left.iter()
                        .zip(right.iter())
                        .map(|(left, right)| left * right)
                        .collect(),
                )
            }
            OpKind::Matmul => {
                let left_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let right_node = graph.node(node.inputs()[1]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[1],
                    },
                )?;
                let left =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                let right =
                    resolve_dense_input(graph, &values, node.inputs()[1], node.op().label())?;
                TensorData::F32(matmul_values(
                    left,
                    left_node.tensor().spec().shape(),
                    right,
                    right_node.tensor().spec().shape(),
                ))
            }
            OpKind::Reshape => values.get(&node.inputs()[0]).cloned().ok_or(
                ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                },
            )?,
            OpKind::Permute { axes } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                TensorData::F32(permute_values(
                    input,
                    input_node.tensor().spec().shape(),
                    axes,
                ))
            }
            OpKind::Slice { axis, start, end } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                TensorData::F32(slice_values(
                    input,
                    input_node.tensor().spec().shape(),
                    *axis,
                    *start,
                    *end,
                ))
            }
            OpKind::Select { axis, index } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                TensorData::F32(select_values(
                    input,
                    input_node.tensor().spec().shape(),
                    *axis,
                    *index,
                ))
            }
            OpKind::Concat { axis } => {
                let mut parts = Vec::with_capacity(node.inputs().len());
                for input_id in node.inputs() {
                    let input_node =
                        graph
                            .node(*input_id)
                            .ok_or(ReferenceEvaluationError::UnknownTensor {
                                tensor_id: *input_id,
                            })?;
                    let input = resolve_dense_input(graph, &values, *input_id, node.op().label())?;
                    parts.push((input_node.tensor().spec().shape().clone(), input.to_vec()));
                }
                TensorData::F32(concat_values(parts.as_slice(), *axis))
            }
            OpKind::Expand { shape } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                TensorData::F32(expand_values(
                    input,
                    input_node.tensor().spec().shape(),
                    shape,
                ))
            }
            OpKind::Cast { dtype } => {
                let input =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                match dtype {
                    DType::F32 | DType::F16 | DType::BF16 => TensorData::F32(input.to_vec()),
                    DType::I32 => TensorData::I32(
                        input
                            .iter()
                            .map(|current| {
                                current.round().clamp(i32::MIN as f32, i32::MAX as f32) as i32
                            })
                            .collect(),
                    ),
                    DType::I8 => TensorData::F32(
                        input
                            .iter()
                            .map(|current| current.round().clamp(i8::MIN as f32, i8::MAX as f32))
                            .collect(),
                    ),
                }
            }
            OpKind::ReduceSum { axis } => {
                let input_node = graph.node(node.inputs()[0]).ok_or(
                    ReferenceEvaluationError::UnknownTensor {
                        tensor_id: node.inputs()[0],
                    },
                )?;
                let input =
                    resolve_dense_input(graph, &values, node.inputs()[0], node.op().label())?;
                TensorData::F32(reduce_sum_values(
                    input,
                    input_node.tensor().spec().shape(),
                    *axis,
                ))
            }
            OpKind::BackendExtension { op } => {
                evaluate_backend_extension_reference(graph, node, &values, op)?
            }
        };
        validate_output_length(node.tensor(), &value)?;
        values.insert(node.tensor().id(), value);
    }
    Ok(values)
}

fn evaluate_backend_extension_reference(
    graph: &Graph,
    node: &crate::Node,
    values: &BTreeMap<TensorId, TensorData>,
    op: &BackendExtensionOp,
) -> Result<TensorData, ReferenceEvaluationError> {
    match op {
        BackendExtensionOp::ReluSquared => {
            let input = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            Ok(TensorData::F32(relu_squared_forward_values(input)))
        }
        BackendExtensionOp::ReluSquaredBackward => {
            let input = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let grad_output =
                resolve_dense_input(graph, values, node.inputs()[1], node.op().label())?;
            Ok(TensorData::F32(relu_squared_backward_values(
                input,
                grad_output,
            )))
        }
        BackendExtensionOp::ReluSquaredBackwardFromOutput => {
            let output = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let grad_output =
                resolve_dense_input(graph, values, node.inputs()[1], node.op().label())?;
            Ok(TensorData::F32(relu_squared_backward_from_output_values(
                output,
                grad_output,
            )))
        }
        BackendExtensionOp::LeakyReluSquared { negative_slope } => {
            let input = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            Ok(TensorData::F32(leaky_relu_squared_forward_values(
                input,
                negative_slope.to_f32(),
            )))
        }
        BackendExtensionOp::LeakyReluSquaredBackward { negative_slope } => {
            let input = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let grad_output =
                resolve_dense_input(graph, values, node.inputs()[1], node.op().label())?;
            Ok(TensorData::F32(leaky_relu_squared_backward_values(
                input,
                grad_output,
                negative_slope.to_f32(),
            )))
        }
        BackendExtensionOp::Silu => {
            let input = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            Ok(TensorData::F32(silu_forward_values(input)))
        }
        BackendExtensionOp::SiluBackward => {
            let input = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let grad_output =
                resolve_dense_input(graph, values, node.inputs()[1], node.op().label())?;
            Ok(TensorData::F32(silu_backward_values(input, grad_output)))
        }
        BackendExtensionOp::ParameterGolfTokenEmbeddingLookup => {
            let token_ids = resolve_i32_input(graph, values, node.inputs()[0], node.op().label())?;
            let token_embedding =
                resolve_dense_input(graph, values, node.inputs()[1], node.op().label())?;
            let token_ids_shape = graph
                .node(node.inputs()[0])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            let token_embedding_shape = graph
                .node(node.inputs()[1])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[1],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            Ok(TensorData::F32(
                parameter_golf_token_embedding_lookup_forward_values(
                    node.tensor().id(),
                    token_ids,
                    &token_ids_shape,
                    token_embedding,
                    &token_embedding_shape,
                )?,
            ))
        }
        BackendExtensionOp::ParameterGolfBankedLinear { bank_index } => {
            let input = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let matrix_bank =
                resolve_dense_input(graph, values, node.inputs()[1], node.op().label())?;
            let input_shape = graph
                .node(node.inputs()[0])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            let matrix_bank_shape = graph
                .node(node.inputs()[1])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[1],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            Ok(TensorData::F32(
                parameter_golf_banked_linear_forward_values(
                    node.tensor().id(),
                    input,
                    &input_shape,
                    matrix_bank,
                    &matrix_bank_shape,
                    *bank_index,
                )?,
            ))
        }
        BackendExtensionOp::ParameterGolfBankedLinearInputBackward { bank_index } => {
            let grad_output =
                resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let matrix_bank =
                resolve_dense_input(graph, values, node.inputs()[1], node.op().label())?;
            let grad_output_shape = graph
                .node(node.inputs()[0])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            let matrix_bank_shape = graph
                .node(node.inputs()[1])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[1],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            Ok(TensorData::F32(
                parameter_golf_banked_linear_input_backward_values(
                    node.tensor().id(),
                    grad_output,
                    &grad_output_shape,
                    matrix_bank,
                    &matrix_bank_shape,
                    *bank_index,
                )?,
            ))
        }
        BackendExtensionOp::ParameterGolfBankedLinearWeightBackward { bank_index } => {
            let input = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let matrix_bank =
                resolve_dense_input(graph, values, node.inputs()[1], node.op().label())?;
            let grad_output =
                resolve_dense_input(graph, values, node.inputs()[2], node.op().label())?;
            let input_shape = graph
                .node(node.inputs()[0])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            let matrix_bank_shape = graph
                .node(node.inputs()[1])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[1],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            let grad_output_shape = graph
                .node(node.inputs()[2])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[2],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            Ok(TensorData::F32(
                parameter_golf_banked_linear_weight_backward_values(
                    node.tensor().id(),
                    input,
                    &input_shape,
                    matrix_bank,
                    &matrix_bank_shape,
                    grad_output,
                    &grad_output_shape,
                    *bank_index,
                )?,
            ))
        }
        BackendExtensionOp::ParameterGolfTokenEmbeddingLookupBackward => {
            let token_ids = resolve_i32_input(graph, values, node.inputs()[0], node.op().label())?;
            let token_embedding =
                resolve_dense_input(graph, values, node.inputs()[1], node.op().label())?;
            let grad_output =
                resolve_dense_input(graph, values, node.inputs()[2], node.op().label())?;
            let token_ids_shape = graph
                .node(node.inputs()[0])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            let token_embedding_shape = graph
                .node(node.inputs()[1])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[1],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            Ok(TensorData::F32(
                parameter_golf_token_embedding_lookup_backward_values(
                    node.tensor().id(),
                    token_ids,
                    &token_ids_shape,
                    token_embedding,
                    &token_embedding_shape,
                    grad_output,
                )?,
            ))
        }
        BackendExtensionOp::ParameterGolfProjectionLoss { logit_softcap } => {
            let logits = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let target_ids = resolve_i32_input(graph, values, node.inputs()[1], node.op().label())?;
            let logits_shape = graph
                .node(node.inputs()[0])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            let target_shape = graph
                .node(node.inputs()[1])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[1],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            Ok(TensorData::F32(vec![
                parameter_golf_projection_loss_forward_value(
                    node.tensor().id(),
                    logits,
                    &logits_shape,
                    target_ids,
                    &target_shape,
                    logit_softcap.to_f32(),
                )?,
            ]))
        }
        BackendExtensionOp::ParameterGolfProjectionTokenLosses { logit_softcap } => {
            let logits = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let target_ids = resolve_i32_input(graph, values, node.inputs()[1], node.op().label())?;
            let logits_shape = graph
                .node(node.inputs()[0])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            let target_shape = graph
                .node(node.inputs()[1])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[1],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            Ok(TensorData::F32(
                parameter_golf_projection_token_losses_forward_values(
                    node.tensor().id(),
                    logits,
                    &logits_shape,
                    target_ids,
                    &target_shape,
                    logit_softcap.to_f32(),
                )?,
            ))
        }
        BackendExtensionOp::ParameterGolfProjectionLossBackward { logit_softcap } => {
            let logits = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let target_ids = resolve_i32_input(graph, values, node.inputs()[1], node.op().label())?;
            let grad_output =
                resolve_dense_input(graph, values, node.inputs()[2], node.op().label())?;
            let logits_shape = graph
                .node(node.inputs()[0])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            let target_shape = graph
                .node(node.inputs()[1])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[1],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            Ok(TensorData::F32(
                parameter_golf_projection_loss_backward_values(
                    node.tensor().id(),
                    logits,
                    &logits_shape,
                    target_ids,
                    &target_shape,
                    grad_output,
                    logit_softcap.to_f32(),
                )?,
            ))
        }
        BackendExtensionOp::RmsNorm { epsilon } => {
            let input = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let weight = resolve_dense_input(graph, values, node.inputs()[1], node.op().label())?;
            Ok(TensorData::F32(rms_norm_forward_values(
                input,
                weight,
                epsilon.to_f32(),
            )))
        }
        BackendExtensionOp::RmsNormInputBackward { epsilon } => {
            let input = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let weight = resolve_dense_input(graph, values, node.inputs()[1], node.op().label())?;
            let grad_output =
                resolve_dense_input(graph, values, node.inputs()[2], node.op().label())?;
            Ok(TensorData::F32(rms_norm_input_backward_values(
                input,
                weight,
                grad_output,
                epsilon.to_f32(),
            )))
        }
        BackendExtensionOp::RmsNormWeightBackward { epsilon } => {
            let input = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let grad_output =
                resolve_dense_input(graph, values, node.inputs()[1], node.op().label())?;
            let Some(&last_dim) = graph
                .node(node.inputs()[0])
                .and_then(|input_node| input_node.tensor().spec().shape().dims().last())
            else {
                return Err(ReferenceEvaluationError::UnsupportedOp {
                    tensor_id: node.tensor().id(),
                    op: String::from(node.op().label()),
                });
            };
            Ok(TensorData::F32(rms_norm_weight_backward_values(
                input,
                grad_output,
                last_dim,
                epsilon.to_f32(),
            )))
        }
        BackendExtensionOp::RotaryEmbedding { interleaved } => {
            let input = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let cos = resolve_dense_input(graph, values, node.inputs()[1], node.op().label())?;
            let sin = resolve_dense_input(graph, values, node.inputs()[2], node.op().label())?;
            let input_shape = graph
                .node(node.inputs()[0])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            let cos_shape = graph
                .node(node.inputs()[1])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[1],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            Ok(TensorData::F32(rotary_embedding_values(
                input,
                &input_shape,
                cos,
                sin,
                &cos_shape,
                *interleaved,
            )))
        }
        BackendExtensionOp::RotaryEmbeddingBackward { interleaved } => {
            let grad_output =
                resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let cos = resolve_dense_input(graph, values, node.inputs()[1], node.op().label())?;
            let sin = resolve_dense_input(graph, values, node.inputs()[2], node.op().label())?;
            let grad_shape = graph
                .node(node.inputs()[0])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            let cos_shape = graph
                .node(node.inputs()[1])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[1],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            Ok(TensorData::F32(rotary_embedding_backward_values(
                grad_output,
                &grad_shape,
                cos,
                sin,
                &cos_shape,
                *interleaved,
            )))
        }
        BackendExtensionOp::ScaledDotProductAttention { scale, causal } => {
            let query = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let key = resolve_dense_input(graph, values, node.inputs()[1], node.op().label())?;
            let value = resolve_dense_input(graph, values, node.inputs()[2], node.op().label())?;
            let query_shape = graph
                .node(node.inputs()[0])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            let key_shape = graph
                .node(node.inputs()[1])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[1],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            let value_shape = graph
                .node(node.inputs()[2])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[2],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            Ok(TensorData::F32(
                scaled_dot_product_attention_forward_values(
                    query,
                    &query_shape,
                    key,
                    &key_shape,
                    value,
                    &value_shape,
                    scale.to_f32(),
                    *causal,
                ),
            ))
        }
        BackendExtensionOp::ScaledDotProductAttentionQueryBackward { scale, causal }
        | BackendExtensionOp::ScaledDotProductAttentionKeyBackward { scale, causal }
        | BackendExtensionOp::ScaledDotProductAttentionValueBackward { scale, causal } => {
            let query = resolve_dense_input(graph, values, node.inputs()[0], node.op().label())?;
            let key = resolve_dense_input(graph, values, node.inputs()[1], node.op().label())?;
            let value = resolve_dense_input(graph, values, node.inputs()[2], node.op().label())?;
            let grad_output =
                resolve_dense_input(graph, values, node.inputs()[3], node.op().label())?;
            let query_shape = graph
                .node(node.inputs()[0])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[0],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            let key_shape = graph
                .node(node.inputs()[1])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[1],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            let value_shape = graph
                .node(node.inputs()[2])
                .ok_or(ReferenceEvaluationError::UnknownTensor {
                    tensor_id: node.inputs()[2],
                })?
                .tensor()
                .spec()
                .shape()
                .clone();
            let gradients = scaled_dot_product_attention_backward_values(
                query,
                &query_shape,
                key,
                &key_shape,
                value,
                &value_shape,
                grad_output,
                scale.to_f32(),
                *causal,
            );
            Ok(TensorData::F32(match op {
                BackendExtensionOp::ScaledDotProductAttentionQueryBackward { .. } => {
                    gradients.query
                }
                BackendExtensionOp::ScaledDotProductAttentionKeyBackward { .. } => gradients.key,
                BackendExtensionOp::ScaledDotProductAttentionValueBackward { .. } => {
                    gradients.value
                }
                _ => unreachable!("backward op match arm should stay aligned"),
            }))
        }
        _ => Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id: node.tensor().id(),
            op: String::from(node.op().label()),
        }),
    }
}

fn graph_input_use_counts(graph: &Graph) -> BTreeMap<TensorId, usize> {
    let mut counts = BTreeMap::new();
    for node in graph.nodes() {
        for input in node.inputs() {
            *counts.entry(*input).or_insert(0) += 1;
        }
    }
    counts
}

fn release_consumed_tensors(
    inputs: &[TensorId],
    remaining_uses: &mut BTreeMap<TensorId, usize>,
    values: &mut BTreeMap<TensorId, TensorData>,
    retain_tensors: &BTreeSet<TensorId>,
    dropped_tensor_count: &mut usize,
) {
    for input in inputs {
        let Some(remaining) = remaining_uses.get_mut(input) else {
            continue;
        };
        if *remaining > 0 {
            *remaining -= 1;
        }
        if *remaining == 0 && !retain_tensors.contains(input) && values.remove(input).is_some() {
            *dropped_tensor_count += 1;
        }
    }
}

fn release_tensor_if_dead(
    tensor_id: TensorId,
    remaining_uses: &BTreeMap<TensorId, usize>,
    values: &mut BTreeMap<TensorId, TensorData>,
    retain_tensors: &BTreeSet<TensorId>,
    dropped_tensor_count: &mut usize,
) {
    if remaining_uses.get(&tensor_id).copied().unwrap_or(0) == 0
        && !retain_tensors.contains(&tensor_id)
        && values.remove(&tensor_id).is_some()
    {
        *dropped_tensor_count += 1;
    }
}

fn ensure_supported_gradient_dtype(tensor: &Tensor) -> Result<(), AutodiffError> {
    if tensor.spec().dtype() == DType::F32 || tensor.spec().dtype() == DType::BF16 {
        Ok(())
    } else {
        Err(AutodiffError::UnsupportedGradientDType {
            tensor_id: tensor.id(),
            dtype: tensor.spec().dtype(),
        })
    }
}

fn map_graph_error(error: GraphError) -> AutodiffError {
    AutodiffError::BackwardGraphConstruction {
        message: error.to_string(),
    }
}

fn primal_placeholder(
    builder: &mut GraphBuilder,
    bindings: &mut BTreeMap<TensorId, Tensor>,
    graph: &Graph,
    tensor_id: TensorId,
) -> Result<Tensor, AutodiffError> {
    if let Some(tensor) = bindings.get(&tensor_id) {
        return Ok(tensor.clone());
    }
    let node = graph
        .node(tensor_id)
        .ok_or(AutodiffError::UnknownTensor { tensor_id })?;
    let tensor = builder.input(
        format!("primal.{}", tensor_id),
        node.tensor().spec().shape().clone(),
        node.tensor().spec().dtype(),
    );
    bindings.insert(tensor_id, tensor.clone());
    Ok(tensor)
}

fn accumulate_gradient(
    builder: &mut GraphBuilder,
    gradients: &mut BTreeMap<TensorId, Tensor>,
    tensor_id: TensorId,
    contribution: Tensor,
) -> Result<(), AutodiffError> {
    if let Some(existing) = gradients.get(&tensor_id).cloned() {
        let accumulated = builder
            .add(&existing, &contribution)
            .map_err(map_graph_error)?;
        gradients.insert(tensor_id, accumulated);
    } else {
        gradients.insert(tensor_id, contribution);
    }
    Ok(())
}

fn reduce_gradient_to_shape(
    builder: &mut GraphBuilder,
    gradient: &Tensor,
    target_shape: &Shape,
) -> Result<Tensor, AutodiffError> {
    if gradient.spec().shape() == target_shape {
        return Ok(gradient.clone());
    }

    let current_shape = gradient.spec().shape().clone();
    if current_shape.rank() < target_shape.rank() {
        return Err(AutodiffError::BackwardGraphConstruction {
            message: format!(
                "cannot reduce shape {} down to wider target {}",
                current_shape, target_shape
            ),
        });
    }

    let mut reduced = gradient.clone();
    let mut aligned_target = vec![1; current_shape.rank() - target_shape.rank()];
    aligned_target.extend_from_slice(target_shape.dims());

    let mut reduction_axes = Vec::new();
    for (axis, (&current_dim, &target_dim)) in current_shape
        .dims()
        .iter()
        .zip(aligned_target.iter())
        .enumerate()
    {
        if current_dim == target_dim {
            continue;
        }
        if target_dim == 1 && current_dim > 1 {
            reduction_axes.push(axis);
            continue;
        }
        return Err(AutodiffError::BackwardGraphConstruction {
            message: format!(
                "cannot reduce broadcasted gradient shape {} to target {}",
                current_shape, target_shape
            ),
        });
    }

    for axis in reduction_axes.into_iter().rev() {
        reduced = builder
            .reduce_sum_axis(&reduced, axis)
            .map_err(map_graph_error)?;
    }

    if reduced.spec().shape() != target_shape {
        reduced = builder
            .reshape(&reduced, target_shape.clone())
            .map_err(map_graph_error)?;
    }
    Ok(reduced)
}

fn pad_axis_with_zeros(
    builder: &mut GraphBuilder,
    core: &Tensor,
    full_shape: &Shape,
    axis: usize,
    start: usize,
    end: usize,
) -> Result<Tensor, AutodiffError> {
    let mut parts = Vec::new();
    if start > 0 {
        parts.push(zero_tensor(
            builder,
            replace_axis_dim(full_shape, axis, start),
        )?);
    }
    parts.push(core.clone());
    let suffix_len = full_shape.dims()[axis].saturating_sub(end);
    if suffix_len > 0 {
        parts.push(zero_tensor(
            builder,
            replace_axis_dim(full_shape, axis, suffix_len),
        )?);
    }
    if parts.len() == 1 {
        return Ok(parts.remove(0));
    }
    builder
        .concat(parts.as_slice(), axis)
        .map_err(map_graph_error)
}

fn zero_tensor(builder: &mut GraphBuilder, shape: Shape) -> Result<Tensor, AutodiffError> {
    builder
        .constant_f32(shape.clone(), vec![0.0; shape.element_count()])
        .map_err(map_graph_error)
}

fn replace_axis_dim(shape: &Shape, axis: usize, dim: usize) -> Shape {
    let mut dims = shape.dims().to_vec();
    dims[axis] = dim;
    Shape::new(dims)
}

fn insert_axis(dims: &[usize], axis: usize, dim: usize) -> Vec<usize> {
    let mut expanded = dims.to_vec();
    expanded.insert(axis, dim);
    expanded
}

fn invert_axes(axes: &[usize]) -> Vec<usize> {
    let mut inverse = vec![0; axes.len()];
    for (index, axis) in axes.iter().copied().enumerate() {
        inverse[axis] = index;
    }
    inverse
}

fn resolve_dense_input<'a>(
    graph: &Graph,
    values: &'a BTreeMap<TensorId, TensorData>,
    tensor_id: TensorId,
    op: &str,
) -> Result<&'a [f32], ReferenceEvaluationError> {
    let tensor = graph
        .node(tensor_id)
        .ok_or(ReferenceEvaluationError::UnknownTensor { tensor_id })?
        .tensor();
    if tensor.spec().dtype() != DType::F32 && tensor.spec().dtype() != DType::BF16 {
        return Err(ReferenceEvaluationError::UnsupportedDType {
            tensor_id,
            op: String::from(op),
            dtype: tensor.spec().dtype(),
        });
    }
    let value = values
        .get(&tensor_id)
        .ok_or(ReferenceEvaluationError::UnknownTensor { tensor_id })?;
    let Some(values) = value.as_f32_slice() else {
        return Err(ReferenceEvaluationError::DenseF32Required {
            tensor_id,
            op: String::from(op),
        });
    };
    let expected_len = tensor.spec().shape().element_count();
    if values.len() != expected_len {
        return Err(ReferenceEvaluationError::PayloadLengthMismatch {
            tensor_id,
            expected_len,
            actual_len: values.len(),
        });
    }
    Ok(values)
}

fn resolve_i32_input<'a>(
    graph: &Graph,
    values: &'a BTreeMap<TensorId, TensorData>,
    tensor_id: TensorId,
    op: &str,
) -> Result<&'a [i32], ReferenceEvaluationError> {
    let tensor = graph
        .node(tensor_id)
        .ok_or(ReferenceEvaluationError::UnknownTensor { tensor_id })?
        .tensor();
    if tensor.spec().dtype() != DType::I32 {
        return Err(ReferenceEvaluationError::UnsupportedDType {
            tensor_id,
            op: String::from(op),
            dtype: tensor.spec().dtype(),
        });
    }
    let value = values
        .get(&tensor_id)
        .ok_or(ReferenceEvaluationError::UnknownTensor { tensor_id })?;
    let Some(values) = value.as_i32_slice() else {
        return Err(ReferenceEvaluationError::UnsupportedDType {
            tensor_id,
            op: String::from(op),
            dtype: tensor.spec().dtype(),
        });
    };
    let expected_len = tensor.spec().shape().element_count();
    if values.len() != expected_len {
        return Err(ReferenceEvaluationError::PayloadLengthMismatch {
            tensor_id,
            expected_len,
            actual_len: values.len(),
        });
    }
    Ok(values)
}

fn validate_output_length(
    tensor: &Tensor,
    value: &TensorData,
) -> Result<(), ReferenceEvaluationError> {
    let expected_len = tensor.spec().shape().element_count();
    let actual_len = value.len();
    if expected_len == actual_len {
        Ok(())
    } else {
        Err(ReferenceEvaluationError::PayloadLengthMismatch {
            tensor_id: tensor.id(),
            expected_len,
            actual_len,
        })
    }
}

fn matmul_values(left: &[f32], left_shape: &Shape, right: &[f32], right_shape: &Shape) -> Vec<f32> {
    let rows = left_shape.dims()[0];
    let inner = left_shape.dims()[1];
    let cols = right_shape.dims()[1];
    let mut output = vec![0.0; rows * cols];
    for row in 0..rows {
        for col in 0..cols {
            let mut sum = 0.0;
            for inner_index in 0..inner {
                sum += left[(row * inner) + inner_index] * right[(inner_index * cols) + col];
            }
            output[(row * cols) + col] = sum;
        }
    }
    output
}

fn parameter_golf_banked_linear_forward_values(
    tensor_id: TensorId,
    input: &[f32],
    input_shape: &Shape,
    matrix_bank: &[f32],
    matrix_bank_shape: &Shape,
    bank_index: usize,
) -> Result<Vec<f32>, ReferenceEvaluationError> {
    let batch_size = input_shape.dims()[0];
    let sequence_length = input_shape.dims()[1];
    let in_features = input_shape.dims()[2];
    let out_features = matrix_bank_shape.dims()[1];
    let bank_in_features = matrix_bank_shape.dims()[2];
    if in_features != bank_in_features {
        return Err(ReferenceEvaluationError::PayloadLengthMismatch {
            tensor_id,
            expected_len: in_features,
            actual_len: bank_in_features,
        });
    }
    let rows = batch_size.saturating_mul(sequence_length);
    let matrix_elements = out_features.saturating_mul(in_features);
    let bank_offset = bank_index.saturating_mul(matrix_elements);
    let bank_end = bank_offset.saturating_add(matrix_elements);
    let right = &matrix_bank[bank_offset..bank_end];
    let flattened = Shape::new(vec![rows, in_features]);
    let mut transposed = vec![0.0_f32; in_features.saturating_mul(out_features)];
    for out_index in 0..out_features {
        for in_index in 0..in_features {
            transposed[(in_index * out_features) + out_index] =
                right[(out_index * in_features) + in_index];
        }
    }
    Ok(matmul_values(
        input,
        &flattened,
        &transposed,
        &Shape::new(vec![in_features, out_features]),
    ))
}

fn parameter_golf_banked_linear_input_backward_values(
    tensor_id: TensorId,
    grad_output: &[f32],
    grad_output_shape: &Shape,
    matrix_bank: &[f32],
    matrix_bank_shape: &Shape,
    bank_index: usize,
) -> Result<Vec<f32>, ReferenceEvaluationError> {
    if grad_output_shape.dims().len() != 3 || matrix_bank_shape.dims().len() != 3 {
        return Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id,
            op: String::from("parameter_golf_banked_linear_input_backward_shape"),
        });
    }
    let batch_size = grad_output_shape.dims()[0];
    let sequence_length = grad_output_shape.dims()[1];
    let out_features = grad_output_shape.dims()[2];
    let bank_out_features = matrix_bank_shape.dims()[1];
    let in_features = matrix_bank_shape.dims()[2];
    if out_features != bank_out_features {
        return Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id,
            op: String::from("parameter_golf_banked_linear_input_backward_feature_mismatch"),
        });
    }
    let rows = batch_size.saturating_mul(sequence_length);
    let matrix_elements = out_features.saturating_mul(in_features);
    let bank_offset = bank_index.saturating_mul(matrix_elements);
    let bank_end = bank_offset.saturating_add(matrix_elements);
    let weight = &matrix_bank[bank_offset..bank_end];
    Ok(matmul_values(
        grad_output,
        &Shape::new(vec![rows, out_features]),
        weight,
        &Shape::new(vec![out_features, in_features]),
    ))
}

fn parameter_golf_banked_linear_weight_backward_values(
    tensor_id: TensorId,
    input: &[f32],
    input_shape: &Shape,
    matrix_bank: &[f32],
    matrix_bank_shape: &Shape,
    grad_output: &[f32],
    grad_output_shape: &Shape,
    bank_index: usize,
) -> Result<Vec<f32>, ReferenceEvaluationError> {
    if input_shape.dims().len() != 3
        || matrix_bank_shape.dims().len() != 3
        || grad_output_shape.dims().len() != 3
    {
        return Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id,
            op: String::from("parameter_golf_banked_linear_weight_backward_shape"),
        });
    }
    let batch_size = input_shape.dims()[0];
    let sequence_length = input_shape.dims()[1];
    let in_features = input_shape.dims()[2];
    let bank_len = matrix_bank_shape.dims()[0];
    let out_features = matrix_bank_shape.dims()[1];
    let bank_in_features = matrix_bank_shape.dims()[2];
    if in_features != bank_in_features
        || grad_output_shape.dims()[0] != batch_size
        || grad_output_shape.dims()[1] != sequence_length
        || grad_output_shape.dims()[2] != out_features
    {
        return Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id,
            op: String::from("parameter_golf_banked_linear_weight_backward_feature_mismatch"),
        });
    }
    let rows = batch_size.saturating_mul(sequence_length);
    if input.len() != rows.saturating_mul(in_features)
        || grad_output.len() != rows.saturating_mul(out_features)
        || matrix_bank.len()
            != bank_len
                .saturating_mul(out_features)
                .saturating_mul(in_features)
    {
        return Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id,
            op: String::from("parameter_golf_banked_linear_weight_backward_payload"),
        });
    }
    let mut output = vec![0.0_f32; matrix_bank.len()];
    let bank_offset = bank_index
        .saturating_mul(out_features)
        .saturating_mul(in_features);
    for out_index in 0..out_features {
        for in_index in 0..in_features {
            let mut sum = 0.0_f32;
            for row in 0..rows {
                sum += grad_output[(row * out_features) + out_index]
                    * input[(row * in_features) + in_index];
            }
            output[bank_offset + (out_index * in_features) + in_index] = sum;
        }
    }
    Ok(output)
}

fn permute_values(values: &[f32], input_shape: &Shape, axes: &[usize]) -> Vec<f32> {
    let output_shape = input_shape
        .permuted(axes)
        .unwrap_or_else(|| Shape::new(Vec::<usize>::new()));
    let mut output = vec![0.0; output_shape.element_count()];
    for (output_index, value) in output.iter_mut().enumerate() {
        let output_coords = unravel_index(output_index, output_shape.dims());
        let mut input_coords = vec![0; input_shape.rank()];
        for (output_axis, input_axis) in axes.iter().copied().enumerate() {
            input_coords[input_axis] = output_coords[output_axis];
        }
        *value = values[ravel_index(&input_coords, input_shape.dims())];
    }
    output
}

fn slice_values(
    values: &[f32],
    input_shape: &Shape,
    axis: usize,
    start: usize,
    end: usize,
) -> Vec<f32> {
    let output_shape = replace_axis_dim(input_shape, axis, end.saturating_sub(start));
    let mut output = vec![0.0; output_shape.element_count()];
    for (output_index, value) in output.iter_mut().enumerate() {
        let mut coords = unravel_index(output_index, output_shape.dims());
        coords[axis] = coords[axis].saturating_add(start);
        *value = values[ravel_index(&coords, input_shape.dims())];
    }
    output
}

fn select_values(values: &[f32], input_shape: &Shape, axis: usize, index: usize) -> Vec<f32> {
    let output_shape = input_shape.without_axis(axis).unwrap_or_else(Shape::scalar);
    let mut output = vec![0.0; output_shape.element_count()];
    for (output_index, value) in output.iter_mut().enumerate() {
        let mut coords = unravel_index(output_index, output_shape.dims());
        coords.insert(axis, index);
        *value = values[ravel_index(&coords, input_shape.dims())];
    }
    output
}

fn concat_values(parts: &[(Shape, Vec<f32>)], axis: usize) -> Vec<f32> {
    let mut output_dims = parts[0].0.dims().to_vec();
    output_dims[axis] = parts.iter().map(|(shape, _)| shape.dims()[axis]).sum();
    let output_shape = Shape::new(output_dims);
    let mut output = vec![0.0; output_shape.element_count()];
    let mut axis_offset = 0usize;
    for (shape, values) in parts {
        for (input_index, input_value) in values.iter().copied().enumerate() {
            let mut coords = unravel_index(input_index, shape.dims());
            coords[axis] = coords[axis].saturating_add(axis_offset);
            output[ravel_index(&coords, output_shape.dims())] = input_value;
        }
        axis_offset = axis_offset.saturating_add(shape.dims()[axis]);
    }
    output
}

fn stack_values(parts: &[Vec<f32>], lane_shape: &Shape, axis: usize) -> Vec<f32> {
    let output_shape = Shape::new(insert_axis(lane_shape.dims(), axis, parts.len()));
    let mut output = vec![0.0; output_shape.element_count()];
    for (lane_index, values) in parts.iter().enumerate() {
        for (input_index, value) in values.iter().copied().enumerate() {
            let mut coords = unravel_index(input_index, lane_shape.dims());
            coords.insert(axis, lane_index);
            output[ravel_index(&coords, output_shape.dims())] = value;
        }
    }
    output
}

fn expand_values(values: &[f32], input_shape: &Shape, target_shape: &Shape) -> Vec<f32> {
    let mut output = vec![0.0; target_shape.element_count()];
    let rank_padding = target_shape.rank().saturating_sub(input_shape.rank());
    for (output_index, value) in output.iter_mut().enumerate() {
        let output_coords = unravel_index(output_index, target_shape.dims());
        let mut input_coords = vec![0; input_shape.rank()];
        for input_axis in 0..input_shape.rank() {
            let target_axis = rank_padding + input_axis;
            input_coords[input_axis] = if input_shape.dims()[input_axis] == 1 {
                0
            } else {
                output_coords[target_axis]
            };
        }
        *value = values[ravel_index(&input_coords, input_shape.dims())];
    }
    output
}

fn reduce_sum_values(values: &[f32], input_shape: &Shape, axis: Option<usize>) -> Vec<f32> {
    if let Some(axis) = axis {
        let output_shape = input_shape.without_axis(axis).unwrap_or_else(Shape::scalar);
        let mut output = vec![0.0; output_shape.element_count()];
        for (output_index, value) in output.iter_mut().enumerate() {
            let output_coords = unravel_index(output_index, output_shape.dims());
            let mut sum = 0.0;
            for reduced in 0..input_shape.dims()[axis] {
                let mut input_coords = output_coords.clone();
                input_coords.insert(axis, reduced);
                sum += values[ravel_index(&input_coords, input_shape.dims())];
            }
            *value = sum;
        }
        output
    } else {
        vec![values.iter().sum()]
    }
}

fn relu_squared_forward_values(input: &[f32]) -> Vec<f32> {
    input
        .iter()
        .map(|value| {
            let positive = value.max(0.0);
            positive * positive
        })
        .collect()
}

fn relu_squared_backward_values(input: &[f32], grad_output: &[f32]) -> Vec<f32> {
    input
        .iter()
        .zip(grad_output.iter())
        .map(|(value, grad)| {
            if *value > 0.0 {
                grad * (2.0 * value)
            } else {
                0.0
            }
        })
        .collect()
}

fn relu_squared_backward_from_output_values(output: &[f32], grad_output: &[f32]) -> Vec<f32> {
    output
        .iter()
        .zip(grad_output.iter())
        .map(|(value, grad)| {
            if *value > 0.0 {
                grad * (2.0 * value.sqrt())
            } else {
                0.0
            }
        })
        .collect()
}

fn leaky_relu_squared_forward_values(input: &[f32], negative_slope: f32) -> Vec<f32> {
    input
        .iter()
        .map(|value| {
            let activated = if *value >= 0.0 {
                *value
            } else {
                value * negative_slope
            };
            activated * activated
        })
        .collect()
}

fn leaky_relu_squared_backward_values(
    input: &[f32],
    grad_output: &[f32],
    negative_slope: f32,
) -> Vec<f32> {
    input
        .iter()
        .zip(grad_output.iter())
        .map(|(value, grad)| {
            let derivative = if *value >= 0.0 {
                2.0 * value
            } else {
                2.0 * negative_slope * negative_slope * value
            };
            grad * derivative
        })
        .collect()
}

fn silu_forward_values(input: &[f32]) -> Vec<f32> {
    input
        .iter()
        .map(|value| {
            let sigmoid = 1.0 / (1.0 + (-value).exp());
            value * sigmoid
        })
        .collect()
}

fn silu_backward_values(input: &[f32], grad_output: &[f32]) -> Vec<f32> {
    input
        .iter()
        .zip(grad_output.iter())
        .map(|(value, grad)| {
            let sigmoid = 1.0 / (1.0 + (-value).exp());
            let derivative = sigmoid * (1.0 + (value * (1.0 - sigmoid)));
            grad * derivative
        })
        .collect()
}

fn parameter_golf_token_embedding_lookup_forward_values(
    tensor_id: TensorId,
    token_ids: &[i32],
    token_ids_shape: &Shape,
    token_embedding: &[f32],
    token_embedding_shape: &Shape,
) -> Result<Vec<f32>, ReferenceEvaluationError> {
    if token_ids_shape.dims().len() != 2 || token_embedding_shape.dims().len() != 2 {
        return Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id,
            op: String::from("parameter_golf_token_embedding_lookup_shape"),
        });
    }
    let batch = token_ids_shape.dims()[0];
    let seq = token_ids_shape.dims()[1];
    let vocab = token_embedding_shape.dims()[0];
    let width = token_embedding_shape.dims()[1];
    if token_ids.len() != batch * seq || token_embedding.len() != vocab * width {
        return Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id,
            op: String::from("parameter_golf_token_embedding_lookup_payload"),
        });
    }
    let mut output = vec![0.0_f32; batch * seq * width];
    for (row_index, token_id) in token_ids.iter().copied().enumerate() {
        if token_id < 0 || token_id as usize >= vocab {
            return Err(ReferenceEvaluationError::UnsupportedOp {
                tensor_id,
                op: String::from("parameter_golf_token_embedding_lookup_token_value"),
            });
        }
        let source_offset = token_id as usize * width;
        let destination_offset = row_index * width;
        output[destination_offset..destination_offset + width]
            .copy_from_slice(&token_embedding[source_offset..source_offset + width]);
    }
    Ok(output)
}

fn parameter_golf_token_embedding_lookup_backward_values(
    tensor_id: TensorId,
    token_ids: &[i32],
    token_ids_shape: &Shape,
    token_embedding: &[f32],
    token_embedding_shape: &Shape,
    grad_output: &[f32],
) -> Result<Vec<f32>, ReferenceEvaluationError> {
    if token_ids_shape.dims().len() != 2 || token_embedding_shape.dims().len() != 2 {
        return Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id,
            op: String::from("parameter_golf_token_embedding_lookup_backward_shape"),
        });
    }
    let batch = token_ids_shape.dims()[0];
    let seq = token_ids_shape.dims()[1];
    let vocab = token_embedding_shape.dims()[0];
    let width = token_embedding_shape.dims()[1];
    if token_embedding.len() != vocab * width || grad_output.len() != batch * seq * width {
        return Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id,
            op: String::from("parameter_golf_token_embedding_lookup_backward_payload"),
        });
    }
    let mut output = vec![0.0_f32; vocab * width];
    for (row_index, token_id) in token_ids.iter().copied().enumerate() {
        if token_id < 0 || token_id as usize >= vocab {
            return Err(ReferenceEvaluationError::UnsupportedOp {
                tensor_id,
                op: String::from("parameter_golf_token_embedding_lookup_backward_token_value"),
            });
        }
        let source_offset = row_index * width;
        let destination_offset = token_id as usize * width;
        for (destination, gradient) in output[destination_offset..destination_offset + width]
            .iter_mut()
            .zip(grad_output[source_offset..source_offset + width].iter())
        {
            *destination += *gradient;
        }
    }
    Ok(output)
}

fn parameter_golf_projection_loss_forward_value(
    tensor_id: TensorId,
    logits: &[f32],
    logits_shape: &Shape,
    target_ids: &[i32],
    target_shape: &Shape,
    logit_softcap: f32,
) -> Result<f32, ReferenceEvaluationError> {
    if logit_softcap <= 0.0 || !logit_softcap.is_finite() {
        return Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id,
            op: String::from("parameter_golf_projection_loss_invalid_softcap"),
        });
    }
    if logits_shape.dims().len() != 3 || target_shape.dims().len() != 2 {
        return Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id,
            op: String::from("parameter_golf_projection_loss_shape"),
        });
    }
    let batch = logits_shape.dims()[0];
    let seq = logits_shape.dims()[1];
    let vocab = logits_shape.dims()[2];
    let target_ids = parameter_golf_projection_target_ids(
        tensor_id,
        target_ids,
        target_shape,
        batch,
        seq,
        vocab,
    )?;
    let mut total_loss = 0.0_f32;
    for batch_index in 0..batch {
        for position_index in 0..seq {
            let row_offset = (batch_index * seq + position_index) * vocab;
            let logits_row = &logits[row_offset..row_offset + vocab];
            let target = target_ids[batch_index * seq + position_index] as usize;
            let mut softcapped_row = vec![0.0_f32; vocab];
            for (destination, source) in softcapped_row.iter_mut().zip(logits_row.iter()) {
                *destination = logit_softcap * (*source / logit_softcap).tanh();
            }
            let max_logit = softcapped_row
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            let mut exp_sum = 0.0_f32;
            for value in &softcapped_row {
                exp_sum += (*value - max_logit).exp();
            }
            total_loss += max_logit + exp_sum.max(f32::EPSILON).ln() - softcapped_row[target];
        }
    }
    Ok(total_loss / (batch * seq).max(1) as f32)
}

fn parameter_golf_projection_token_losses_forward_values(
    tensor_id: TensorId,
    logits: &[f32],
    logits_shape: &Shape,
    target_ids: &[i32],
    target_shape: &Shape,
    logit_softcap: f32,
) -> Result<Vec<f32>, ReferenceEvaluationError> {
    if logit_softcap <= 0.0 || !logit_softcap.is_finite() {
        return Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id,
            op: String::from("parameter_golf_projection_token_losses_invalid_softcap"),
        });
    }
    if logits_shape.dims().len() != 3 || target_shape.dims().len() != 2 {
        return Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id,
            op: String::from("parameter_golf_projection_token_losses_shape"),
        });
    }
    let batch = logits_shape.dims()[0];
    let seq = logits_shape.dims()[1];
    let vocab = logits_shape.dims()[2];
    let target_ids = parameter_golf_projection_target_ids(
        tensor_id,
        target_ids,
        target_shape,
        batch,
        seq,
        vocab,
    )?;
    let mut output = vec![0.0_f32; batch * seq];
    for batch_index in 0..batch {
        for position_index in 0..seq {
            let row_index = batch_index * seq + position_index;
            let row_offset = row_index * vocab;
            let logits_row = &logits[row_offset..row_offset + vocab];
            let target = target_ids[row_index] as usize;
            let mut softcapped_row = vec![0.0_f32; vocab];
            for (destination, source) in softcapped_row.iter_mut().zip(logits_row.iter()) {
                *destination = logit_softcap * (*source / logit_softcap).tanh();
            }
            let max_logit = softcapped_row
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            let mut exp_sum = 0.0_f32;
            for value in &softcapped_row {
                exp_sum += (*value - max_logit).exp();
            }
            output[row_index] = max_logit + exp_sum.max(f32::EPSILON).ln() - softcapped_row[target];
        }
    }
    Ok(output)
}

fn parameter_golf_projection_loss_backward_values(
    tensor_id: TensorId,
    logits: &[f32],
    logits_shape: &Shape,
    target_ids: &[i32],
    target_shape: &Shape,
    grad_output: &[f32],
    logit_softcap: f32,
) -> Result<Vec<f32>, ReferenceEvaluationError> {
    if logit_softcap <= 0.0 || !logit_softcap.is_finite() {
        return Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id,
            op: String::from("parameter_golf_projection_loss_invalid_softcap"),
        });
    }
    if logits_shape.dims().len() != 3 || target_shape.dims().len() != 2 || grad_output.len() != 1 {
        return Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id,
            op: String::from("parameter_golf_projection_loss_shape"),
        });
    }
    let batch = logits_shape.dims()[0];
    let seq = logits_shape.dims()[1];
    let vocab = logits_shape.dims()[2];
    let target_ids = parameter_golf_projection_target_ids(
        tensor_id,
        target_ids,
        target_shape,
        batch,
        seq,
        vocab,
    )?;
    let scale = grad_output[0];
    let position_count = (batch * seq).max(1) as f32;
    let mut output = vec![0.0_f32; logits.len()];
    for batch_index in 0..batch {
        for position_index in 0..seq {
            let target = target_ids[batch_index * seq + position_index] as usize;
            let row_offset = (batch_index * seq + position_index) * vocab;
            let logits_row = &logits[row_offset..row_offset + vocab];
            let output_row = &mut output[row_offset..row_offset + vocab];
            let mut softcapped_row = vec![0.0_f32; vocab];
            for (destination, source) in softcapped_row.iter_mut().zip(logits_row.iter()) {
                *destination = logit_softcap * (*source / logit_softcap).tanh();
            }
            let max_logit = softcapped_row
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            let mut probabilities = vec![0.0_f32; vocab];
            let mut exp_sum = 0.0_f32;
            for (index, value) in softcapped_row.iter().enumerate() {
                let exp = (*value - max_logit).exp();
                probabilities[index] = exp;
                exp_sum += exp;
            }
            let exp_sum = exp_sum.max(f32::EPSILON);
            for index in 0..vocab {
                probabilities[index] /= exp_sum;
                let delta = if index == target { 1.0 } else { 0.0 };
                let softcap_derivative = 1.0 - (softcapped_row[index] / logit_softcap).powi(2);
                output_row[index] =
                    scale * ((probabilities[index] - delta) / position_count) * softcap_derivative;
            }
        }
    }
    Ok(output)
}

fn parameter_golf_projection_target_ids(
    tensor_id: TensorId,
    target_ids: &[i32],
    target_shape: &Shape,
    batch: usize,
    seq: usize,
    vocab: usize,
) -> Result<Vec<u32>, ReferenceEvaluationError> {
    let expected_len = batch * seq;
    if target_shape.dims() != [batch, seq] || target_ids.len() != expected_len {
        return Err(ReferenceEvaluationError::UnsupportedOp {
            tensor_id,
            op: String::from("parameter_golf_projection_loss_target_shape"),
        });
    }
    target_ids
        .iter()
        .map(|value| {
            if *value < 0 || *value as usize >= vocab {
                return Err(ReferenceEvaluationError::UnsupportedOp {
                    tensor_id,
                    op: String::from("parameter_golf_projection_loss_target_value"),
                });
            }
            Ok(*value as u32)
        })
        .collect()
}

fn rms_norm_forward_values(input: &[f32], weight: &[f32], epsilon: f32) -> Vec<f32> {
    if weight.is_empty() {
        return Vec::new();
    }
    let last_dim = weight.len();
    let mut output = vec![0.0_f32; input.len()];
    for (src_row, dst_row) in input
        .chunks_exact(last_dim)
        .zip(output.chunks_exact_mut(last_dim))
    {
        let mean_square = src_row.iter().map(|value| value * value).sum::<f32>() / last_dim as f32;
        let inv = (mean_square + epsilon).sqrt().recip();
        for ((dst, value), scale) in dst_row.iter_mut().zip(src_row.iter()).zip(weight.iter()) {
            *dst = *value * inv * *scale;
        }
    }
    output
}

fn rms_norm_input_backward_values(
    input: &[f32],
    weight: &[f32],
    grad_output: &[f32],
    epsilon: f32,
) -> Vec<f32> {
    if weight.is_empty() {
        return vec![0.0_f32; input.len()];
    }
    let last_dim = weight.len();
    let mut output = vec![0.0_f32; input.len()];
    for ((src_row, grad_row), dst_row) in input
        .chunks_exact(last_dim)
        .zip(grad_output.chunks_exact(last_dim))
        .zip(output.chunks_exact_mut(last_dim))
    {
        let mean_square = src_row.iter().map(|value| value * value).sum::<f32>() / last_dim as f32;
        let inv = (mean_square + epsilon).sqrt().recip();
        let inv_cubed = inv * inv * inv;
        let weighted_dot = src_row
            .iter()
            .zip(weight.iter())
            .zip(grad_row.iter())
            .map(|((value, scale), grad)| value * scale * grad)
            .sum::<f32>();
        for (((dst, value), scale), grad) in dst_row
            .iter_mut()
            .zip(src_row.iter())
            .zip(weight.iter())
            .zip(grad_row.iter())
        {
            *dst = (grad * scale * inv) - (value * inv_cubed * weighted_dot / last_dim as f32);
        }
    }
    output
}

fn rms_norm_weight_backward_values(
    input: &[f32],
    grad_output: &[f32],
    last_dim: usize,
    epsilon: f32,
) -> Vec<f32> {
    if last_dim == 0 {
        return Vec::new();
    }
    let mut output = vec![0.0_f32; last_dim];
    for (src_row, grad_row) in input
        .chunks_exact(last_dim)
        .zip(grad_output.chunks_exact(last_dim))
    {
        let mean_square = src_row.iter().map(|value| value * value).sum::<f32>() / last_dim as f32;
        let inv = (mean_square + epsilon).sqrt().recip();
        for ((accumulated, value), grad) in
            output.iter_mut().zip(src_row.iter()).zip(grad_row.iter())
        {
            *accumulated += grad * value * inv;
        }
    }
    output
}

fn rotary_embedding_values(
    input: &[f32],
    input_shape: &Shape,
    cos: &[f32],
    sin: &[f32],
    cos_shape: &Shape,
    interleaved: bool,
) -> Vec<f32> {
    let dims = input_shape.dims();
    let batch = dims[0];
    let heads = dims[1];
    let seq_len = dims[2];
    let head_dim = dims[3];
    let half_dim = head_dim / 2;
    let batched_cos = cos_shape.rank() == 3;
    let mut output = input.to_vec();

    for batch_index in 0..batch {
        for head_index in 0..heads {
            for position in 0..seq_len {
                let base = ((batch_index * heads + head_index) * seq_len + position) * head_dim;
                for pair in 0..half_dim {
                    let cos_index = if batched_cos {
                        (batch_index * seq_len + position) * half_dim + pair
                    } else {
                        position * half_dim + pair
                    };
                    let cosine = cos[cos_index];
                    let sine = sin[cos_index];
                    let (left_index, right_index) = if interleaved {
                        (base + pair * 2, base + pair * 2 + 1)
                    } else {
                        (base + pair, base + half_dim + pair)
                    };
                    let left = input[left_index];
                    let right = input[right_index];
                    output[left_index] = left * cosine + right * sine;
                    output[right_index] = (-left * sine) + right * cosine;
                }
            }
        }
    }

    output
}

fn rotary_embedding_backward_values(
    grad_output: &[f32],
    grad_shape: &Shape,
    cos: &[f32],
    sin: &[f32],
    cos_shape: &Shape,
    interleaved: bool,
) -> Vec<f32> {
    let dims = grad_shape.dims();
    let batch = dims[0];
    let heads = dims[1];
    let seq_len = dims[2];
    let head_dim = dims[3];
    let half_dim = head_dim / 2;
    let batched_cos = cos_shape.rank() == 3;
    let mut grad_input = grad_output.to_vec();

    for batch_index in 0..batch {
        for head_index in 0..heads {
            for position in 0..seq_len {
                let base = ((batch_index * heads + head_index) * seq_len + position) * head_dim;
                for pair in 0..half_dim {
                    let cos_index = if batched_cos {
                        (batch_index * seq_len + position) * half_dim + pair
                    } else {
                        position * half_dim + pair
                    };
                    let cosine = cos[cos_index];
                    let sine = sin[cos_index];
                    let (left_index, right_index) = if interleaved {
                        (base + pair * 2, base + pair * 2 + 1)
                    } else {
                        (base + pair, base + half_dim + pair)
                    };
                    let grad_left = grad_output[left_index];
                    let grad_right = grad_output[right_index];
                    grad_input[left_index] = grad_left * cosine - grad_right * sine;
                    grad_input[right_index] = grad_left * sine + grad_right * cosine;
                }
            }
        }
    }

    grad_input
}

fn scaled_dot_product_attention_forward_values(
    query: &[f32],
    query_shape: &Shape,
    key: &[f32],
    key_shape: &Shape,
    value: &[f32],
    value_shape: &Shape,
    scale: f32,
    causal: bool,
) -> Vec<f32> {
    let query_dims = query_shape.dims();
    let key_dims = key_shape.dims();
    let value_dims = value_shape.dims();
    let batch = query_dims[0];
    let query_heads = query_dims[1];
    let key_heads = key_dims[1];
    let query_seq = query_dims[2];
    let key_seq = key_dims[2];
    let head_dim = query_dims[3];
    let value_dim = value_dims[3];
    let group_size = query_heads / key_heads;
    let mut output = vec![0.0_f32; batch * query_heads * query_seq * value_dim];
    let mut scores = vec![0.0_f32; key_seq];
    let mut weights = vec![0.0_f32; key_seq];

    for batch_index in 0..batch {
        for head_index in 0..query_heads {
            let kv_head = head_index / group_size;
            for query_index in 0..query_seq {
                let mut max_score = f32::NEG_INFINITY;
                let mut valid_scores = 0usize;
                for key_index in 0..key_seq {
                    if causal && key_index > query_index {
                        scores[key_index] = f32::NEG_INFINITY;
                        continue;
                    }
                    let query_base = ((batch_index * query_heads + head_index) * query_seq
                        + query_index)
                        * head_dim;
                    let key_base =
                        ((batch_index * key_heads + kv_head) * key_seq + key_index) * head_dim;
                    let mut dot = 0.0_f32;
                    for dim in 0..head_dim {
                        dot += query[query_base + dim] * key[key_base + dim];
                    }
                    let score = dot * scale;
                    scores[key_index] = score;
                    max_score = max_score.max(score);
                    valid_scores += 1;
                }
                if valid_scores == 0 {
                    continue;
                }
                let mut weight_sum = 0.0_f32;
                for key_index in 0..key_seq {
                    if !scores[key_index].is_finite() {
                        weights[key_index] = 0.0;
                        continue;
                    }
                    let weight = (scores[key_index] - max_score).exp();
                    weights[key_index] = weight;
                    weight_sum += weight;
                }
                if weight_sum <= 0.0 {
                    continue;
                }
                let output_base = ((batch_index * query_heads + head_index) * query_seq
                    + query_index)
                    * value_dim;
                for key_index in 0..key_seq {
                    let normalized = weights[key_index] / weight_sum;
                    if normalized == 0.0 {
                        continue;
                    }
                    let value_base =
                        ((batch_index * key_heads + kv_head) * key_seq + key_index) * value_dim;
                    for dim in 0..value_dim {
                        output[output_base + dim] += normalized * value[value_base + dim];
                    }
                }
            }
        }
    }

    output
}

struct ScaledDotProductAttentionBackwardValues {
    query: Vec<f32>,
    key: Vec<f32>,
    value: Vec<f32>,
}

fn scaled_dot_product_attention_backward_values(
    query: &[f32],
    query_shape: &Shape,
    key: &[f32],
    key_shape: &Shape,
    value: &[f32],
    value_shape: &Shape,
    grad_output: &[f32],
    scale: f32,
    causal: bool,
) -> ScaledDotProductAttentionBackwardValues {
    let query_dims = query_shape.dims();
    let key_dims = key_shape.dims();
    let value_dims = value_shape.dims();
    let batch = query_dims[0];
    let query_heads = query_dims[1];
    let key_heads = key_dims[1];
    let query_seq = query_dims[2];
    let key_seq = key_dims[2];
    let head_dim = query_dims[3];
    let value_dim = value_dims[3];
    let group_size = query_heads / key_heads;
    let mut query_grad = vec![0.0_f32; query.len()];
    let mut key_grad = vec![0.0_f32; key.len()];
    let mut value_grad = vec![0.0_f32; value.len()];
    let mut scores = vec![0.0_f32; key_seq];
    let mut probs = vec![0.0_f32; key_seq];
    let mut grad_probs = vec![0.0_f32; key_seq];
    let mut grad_scores = vec![0.0_f32; key_seq];

    for batch_index in 0..batch {
        for head_index in 0..query_heads {
            let kv_head = head_index / group_size;
            for query_index in 0..query_seq {
                let query_base =
                    ((batch_index * query_heads + head_index) * query_seq + query_index) * head_dim;
                let grad_output_base = ((batch_index * query_heads + head_index) * query_seq
                    + query_index)
                    * value_dim;
                let mut max_score = f32::NEG_INFINITY;
                let mut valid_scores = 0usize;
                for key_index in 0..key_seq {
                    if causal && key_index > query_index {
                        scores[key_index] = f32::NEG_INFINITY;
                        probs[key_index] = 0.0;
                        continue;
                    }
                    let key_base =
                        ((batch_index * key_heads + kv_head) * key_seq + key_index) * head_dim;
                    let mut dot = 0.0_f32;
                    for dim in 0..head_dim {
                        dot += query[query_base + dim] * key[key_base + dim];
                    }
                    let score = dot * scale;
                    scores[key_index] = score;
                    max_score = max_score.max(score);
                    valid_scores += 1;
                }
                if valid_scores == 0 {
                    continue;
                }

                let mut weight_sum = 0.0_f32;
                for key_index in 0..key_seq {
                    if !scores[key_index].is_finite() {
                        probs[key_index] = 0.0;
                        continue;
                    }
                    let weight = (scores[key_index] - max_score).exp();
                    probs[key_index] = weight;
                    weight_sum += weight;
                }
                if weight_sum <= 0.0 {
                    continue;
                }
                for probability in &mut probs {
                    *probability /= weight_sum;
                }

                let mut weighted_grad_prob = 0.0_f32;
                for key_index in 0..key_seq {
                    let probability = probs[key_index];
                    if probability == 0.0 {
                        grad_probs[key_index] = 0.0;
                        continue;
                    }
                    let value_base =
                        ((batch_index * key_heads + kv_head) * key_seq + key_index) * value_dim;
                    let mut grad_prob = 0.0_f32;
                    for dim in 0..value_dim {
                        grad_prob += grad_output[grad_output_base + dim] * value[value_base + dim];
                        value_grad[value_base + dim] +=
                            probability * grad_output[grad_output_base + dim];
                    }
                    grad_probs[key_index] = grad_prob;
                    weighted_grad_prob += probability * grad_prob;
                }

                for key_index in 0..key_seq {
                    let probability = probs[key_index];
                    if probability == 0.0 {
                        grad_scores[key_index] = 0.0;
                        continue;
                    }
                    grad_scores[key_index] =
                        probability * (grad_probs[key_index] - weighted_grad_prob);
                }

                for key_index in 0..key_seq {
                    let grad_score = grad_scores[key_index];
                    if grad_score == 0.0 {
                        continue;
                    }
                    let key_base =
                        ((batch_index * key_heads + kv_head) * key_seq + key_index) * head_dim;
                    for dim in 0..head_dim {
                        query_grad[query_base + dim] += scale * grad_score * key[key_base + dim];
                        key_grad[key_base + dim] += scale * grad_score * query[query_base + dim];
                    }
                }
            }
        }
    }

    ScaledDotProductAttentionBackwardValues {
        query: query_grad,
        key: key_grad,
        value: value_grad,
    }
}

fn unravel_index(mut index: usize, dims: &[usize]) -> Vec<usize> {
    if dims.is_empty() {
        return Vec::new();
    }
    let mut coords = vec![0; dims.len()];
    for axis in (0..dims.len()).rev() {
        let dim = dims[axis];
        coords[axis] = index % dim;
        index /= dim;
    }
    coords
}

fn ravel_index(coords: &[usize], dims: &[usize]) -> usize {
    if dims.is_empty() {
        return 0;
    }
    let mut index = 0usize;
    let mut stride = 1usize;
    for axis in (0..dims.len()).rev() {
        index = index.saturating_add(coords[axis].saturating_mul(stride));
        stride = stride.saturating_mul(dims[axis]);
    }
    index
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::{
        collections::{BTreeMap, BTreeSet},
        error::Error,
        sync::Arc,
    };

    use psionic_core::{DType, Device, PsionicRefusalCode, PsionicRefusalScope, Shape, TensorData};

    use crate::{
        checkpoint, custom_vjp, evaluate_graph, grad, gradient_support_for_op, jvp, value_and_grad,
        vjp, vmap, vmap_support_for_op, AutodiffContext, AutodiffError, AutodiffGradientSupport,
        AutodiffGraphBuilder, AutodiffUnsupportedGradientReason, CheckpointTransformError,
        CustomVjpInvocation, CustomVjpRule, CustomVjpTransformError, ForwardModeTransformError,
        ReverseModeTransformError, TensorId, TransformHookLookupError,
        TransformHookRegistrationError, TransformHookRegistry, VmapInputBinding, VmapSupport,
        VmapTransformError, VmapUnsupportedReason,
    };

    struct ScalingCustomVjpRule {
        label: &'static str,
        scale: f32,
    }

    impl CustomVjpRule for ScalingCustomVjpRule {
        fn label(&self) -> &str {
            self.label
        }

        fn apply(
            &self,
            invocation: &CustomVjpInvocation<'_>,
        ) -> Result<BTreeMap<TensorId, TensorData>, psionic_core::PsionicRefusal> {
            let seed = invocation
                .seed
                .as_f32_slice()
                .expect("custom_vjp seed should be dense f32 in tests");
            let target = invocation
                .signature
                .primal_targets
                .first()
                .copied()
                .expect("custom_vjp test should have one target");
            Ok(BTreeMap::from([(
                target,
                TensorData::F32(seed.iter().map(|value| value * self.scale).collect()),
            )]))
        }
    }

    struct EmptyCustomVjpRule;

    impl CustomVjpRule for EmptyCustomVjpRule {
        fn label(&self) -> &str {
            "empty_rule"
        }

        fn apply(
            &self,
            _invocation: &CustomVjpInvocation<'_>,
        ) -> Result<BTreeMap<TensorId, TensorData>, psionic_core::PsionicRefusal> {
            Ok(BTreeMap::new())
        }
    }

    #[test]
    fn reverse_mode_autodiff_materializes_matmul_chain_gradients() -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let x = builder.input("x", Shape::new(vec![2, 2]), DType::F32, true);
        let w = builder.input("w", Shape::new(vec![2, 1]), DType::F32, true);
        let logits = builder.matmul(&x, &w)?;
        let loss = builder.reduce_sum(&logits);
        let graph = builder.finish(vec![loss.clone()]);

        let backward_plan = graph.backward_plan(loss.id())?;
        assert!(backward_plan.gradient_for(x.id()).is_some());
        assert!(backward_plan.gradient_for(w.id()).is_some());

        let inputs = BTreeMap::from([
            (x.id(), TensorData::F32(vec![1.0, 2.0, 3.0, 4.0])),
            (w.id(), TensorData::F32(vec![5.0, 6.0])),
        ]);
        let result = graph.backward_materialized(loss.id(), &inputs)?;

        assert_eq!(dense_gradient(&result, x.id()), vec![5.0, 6.0, 5.0, 6.0]);
        assert_eq!(dense_gradient(&result, w.id()), vec![4.0, 6.0]);
        Ok(())
    }

    #[test]
    fn reverse_mode_autodiff_materializes_banked_linear_gradients_against_finite_difference(
    ) -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![1, 2, 3]), DType::F32, true);
        let matrix_bank = builder.input("bank", Shape::new(vec![2, 2, 3]), DType::F32, true);
        let output = builder.parameter_golf_banked_linear(&input, &matrix_bank, 1)?;
        let scale =
            builder.constant_f32(Shape::new(vec![1, 2, 2]), vec![0.25, -0.5, 1.25, -0.75])?;
        let scaled = builder.mul(&output, &scale)?;
        let loss = builder.reduce_sum(&scaled);
        let graph = builder.finish(vec![loss.clone()]);

        let backward_plan = graph.backward_plan(loss.id())?;
        assert!(backward_plan.gradient_for(input.id()).is_some());
        assert!(backward_plan.gradient_for(matrix_bank.id()).is_some());
        assert!(backward_plan
            .gradient_graph
            .nodes()
            .iter()
            .any(|node| matches!(
                node.op(),
                crate::OpKind::BackendExtension {
                    op: psionic_core::BackendExtensionOp::ParameterGolfBankedLinearInputBackward { .. }
                }
            )));
        assert!(backward_plan
            .gradient_graph
            .nodes()
            .iter()
            .any(|node| matches!(
                node.op(),
                crate::OpKind::BackendExtension {
                    op: psionic_core::BackendExtensionOp::ParameterGolfBankedLinearWeightBackward { .. }
                }
            )));

        let input_values = vec![0.1_f32, -0.2, 0.3, -0.4, 0.5, -0.6];
        let bank_values = vec![
            0.7_f32, -0.8, 0.9, -1.0, 1.1, -1.2, 0.2, -0.3, 0.4, -0.5, 0.6, -0.7,
        ];
        let inputs = BTreeMap::from([
            (input.id(), TensorData::F32(input_values.clone())),
            (matrix_bank.id(), TensorData::F32(bank_values.clone())),
        ]);
        let result = graph.backward_materialized(loss.id(), &inputs)?;
        let analytical_input = dense_gradient(&result, input.id());
        let analytical_bank = dense_gradient(&result, matrix_bank.id());

        let delta = 1e-3_f32;
        let mut finite_input = vec![0.0_f32; input_values.len()];
        for index in 0..input_values.len() {
            let mut plus = input_values.clone();
            plus[index] += delta;
            let mut minus = input_values.clone();
            minus[index] -= delta;
            finite_input[index] = (scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (input.id(), TensorData::F32(plus)),
                    (matrix_bank.id(), TensorData::F32(bank_values.clone())),
                ]),
            )? - scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (input.id(), TensorData::F32(minus)),
                    (matrix_bank.id(), TensorData::F32(bank_values.clone())),
                ]),
            )?) / (2.0 * delta);
        }

        let mut finite_bank = vec![0.0_f32; bank_values.len()];
        for index in 0..bank_values.len() {
            let mut plus = bank_values.clone();
            plus[index] += delta;
            let mut minus = bank_values.clone();
            minus[index] -= delta;
            finite_bank[index] = (scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (input.id(), TensorData::F32(input_values.clone())),
                    (matrix_bank.id(), TensorData::F32(plus)),
                ]),
            )? - scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (input.id(), TensorData::F32(input_values.clone())),
                    (matrix_bank.id(), TensorData::F32(minus)),
                ]),
            )?) / (2.0 * delta);
        }

        assert_close_slice(&analytical_input, &finite_input, 2e-3);
        assert_close_slice(&analytical_bank, &finite_bank, 2e-3);
        Ok(())
    }

    #[test]
    fn reverse_mode_autodiff_accumulates_shared_paths_and_honors_detach(
    ) -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let x = builder.input("x", Shape::new(vec![2]), DType::F32, true);
        let live = builder.add(&x, &x)?;
        let stopped = builder.detach(&x);
        let combined = builder.add(&live, &stopped)?;
        let loss = builder.reduce_sum(&combined);
        let graph = builder.finish(vec![loss.clone()]);

        let inputs = BTreeMap::from([(x.id(), TensorData::F32(vec![2.0, -3.0]))]);
        let result = graph.backward_materialized(loss.id(), &inputs)?;

        assert_eq!(dense_gradient(&result, x.id()), vec![2.0, 2.0]);
        assert!(!graph.requires_grad(stopped.id()));
        assert!(result.gradient(stopped.id()).is_none());
        Ok(())
    }

    #[test]
    fn autodiff_context_makes_training_and_no_grad_behavior_explicit() {
        let mut training =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let train_input = training.input("train", Shape::new(vec![2]), DType::F32, true);
        let train_output = training
            .mul(&train_input, &train_input)
            .expect("mul should succeed");
        assert!(train_input.requires_grad());
        assert!(train_output.requires_grad());

        let mut evaluation =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::evaluation());
        let eval_input = evaluation.input("eval", Shape::new(vec![2]), DType::F32, true);
        let eval_output = evaluation
            .mul(&eval_input, &eval_input)
            .expect("mul should succeed");
        assert!(!eval_input.requires_grad());
        assert!(!eval_output.requires_grad());

        let mut no_grad = AutodiffGraphBuilder::with_context(
            Device::cpu(),
            AutodiffContext::training().with_gradients_enabled(false),
        );
        let no_grad_input = no_grad.input("no_grad", Shape::new(vec![2]), DType::F32, true);
        let no_grad_output = no_grad
            .mul(&no_grad_input, &no_grad_input)
            .expect("mul should succeed");
        assert!(!no_grad_input.requires_grad());
        assert!(!no_grad_output.requires_grad());
    }

    #[test]
    fn reference_evaluation_executes_silu_backend_extension() -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![4]), DType::F32, true);
        let activated = builder.silu(&input)?;
        let graph = builder.finish(vec![activated.clone()]);

        let inputs = BTreeMap::from([(input.id(), TensorData::F32(vec![-2.0, -0.5, 0.75, 3.0]))]);
        let values = evaluate_graph(graph.graph(), &inputs)?;
        let output = dense_tensor(values.get(&activated.id()).expect("forward output"));
        let expected = vec![
            -2.0 / (1.0 + 2.0_f32.exp()),
            -0.5 / (1.0 + 0.5_f32.exp()),
            0.75 / (1.0 + (-0.75_f32).exp()),
            3.0 / (1.0 + (-3.0_f32).exp()),
        ];
        assert_close_slice(&output, &expected, 1e-6);
        Ok(())
    }

    #[test]
    fn reverse_mode_autodiff_materializes_silu_gradients_against_finite_difference(
    ) -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![4]), DType::F32, true);
        let activated = builder.silu(&input)?;
        let scale = builder.constant_f32(Shape::new(vec![4]), vec![0.5, -1.0, 0.25, 1.5])?;
        let scaled = builder.mul(&activated, &scale)?;
        let loss = builder.reduce_sum(&scaled);
        let graph = builder.finish(vec![loss.clone()]);

        let backward_plan = graph.backward_plan(loss.id())?;
        assert!(backward_plan.gradient_for(input.id()).is_some());
        assert!(backward_plan
            .gradient_graph
            .nodes()
            .iter()
            .any(|node| matches!(
                node.op(),
                crate::OpKind::BackendExtension {
                    op: psionic_core::BackendExtensionOp::SiluBackward
                }
            )));

        let input_values = vec![-1.25_f32, -0.5, 0.75, 2.0];
        let inputs = BTreeMap::from([(input.id(), TensorData::F32(input_values.clone()))]);
        let result = graph.backward_materialized(loss.id(), &inputs)?;
        let analytical = dense_gradient(&result, input.id());

        let delta = 1e-3_f32;
        let mut finite = vec![0.0_f32; input_values.len()];
        for index in 0..input_values.len() {
            let mut plus = input_values.clone();
            plus[index] += delta;
            let mut minus = input_values.clone();
            minus[index] -= delta;
            finite[index] = (scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([(input.id(), TensorData::F32(plus))]),
            )? - scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([(input.id(), TensorData::F32(minus))]),
            )?) / (2.0 * delta);
        }

        assert_close_slice(&analytical, &finite, 2e-3);
        Ok(())
    }

    #[test]
    fn reference_evaluation_executes_relu_squared_backend_extension() -> Result<(), Box<dyn Error>>
    {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![4]), DType::F32, true);
        let activated = builder.relu_squared(&input)?;
        let graph = builder.finish(vec![activated.clone()]);

        let inputs = BTreeMap::from([(input.id(), TensorData::F32(vec![-2.0, -0.5, 0.75, 3.0]))]);
        let values = evaluate_graph(graph.graph(), &inputs)?;
        let output = dense_tensor(values.get(&activated.id()).expect("forward output"));
        assert_close_slice(&output, &[0.0, 0.0, 0.5625, 9.0], 1e-6);
        Ok(())
    }

    #[test]
    fn reverse_mode_autodiff_materializes_relu_squared_gradients_against_finite_difference(
    ) -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![4]), DType::F32, true);
        let activated = builder.relu_squared(&input)?;
        let scale = builder.constant_f32(Shape::new(vec![4]), vec![0.5, -1.0, 0.25, 1.5])?;
        let scaled = builder.mul(&activated, &scale)?;
        let loss = builder.reduce_sum(&scaled);
        let graph = builder.finish(vec![loss.clone()]);

        let backward_plan = graph.backward_plan(loss.id())?;
        assert!(backward_plan.gradient_for(input.id()).is_some());
        assert!(backward_plan
            .gradient_graph
            .nodes()
            .iter()
            .any(|node| matches!(
                node.op(),
                crate::OpKind::BackendExtension {
                    op: psionic_core::BackendExtensionOp::ReluSquaredBackwardFromOutput
                }
            )));
        assert!(backward_plan
            .primal_bindings
            .iter()
            .any(|binding| binding.primal_tensor == activated.id()));
        assert!(!backward_plan
            .primal_bindings
            .iter()
            .any(|binding| binding.primal_tensor == input.id()));

        let input_values = vec![-1.25_f32, -0.5, 0.75, 2.0];
        let inputs = BTreeMap::from([(input.id(), TensorData::F32(input_values.clone()))]);
        let result = graph.backward_materialized(loss.id(), &inputs)?;
        let analytical = dense_gradient(&result, input.id());

        let delta = 1e-3_f32;
        let mut finite = vec![0.0_f32; input_values.len()];
        for index in 0..input_values.len() {
            let mut plus = input_values.clone();
            plus[index] += delta;
            let mut minus = input_values.clone();
            minus[index] -= delta;
            finite[index] = (scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([(input.id(), TensorData::F32(plus))]),
            )? - scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([(input.id(), TensorData::F32(minus))]),
            )?) / (2.0 * delta);
        }

        assert_close_slice(&analytical, &finite, 2e-3);
        Ok(())
    }

    #[test]
    fn reference_evaluation_executes_leaky_relu_squared_backend_extension(
    ) -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![4]), DType::F32, true);
        let activated = builder.leaky_relu_squared(&input, 0.5)?;
        let graph = builder.finish(vec![activated.clone()]);

        let inputs = BTreeMap::from([(input.id(), TensorData::F32(vec![-2.0, -0.5, 0.75, 3.0]))]);
        let values = evaluate_graph(graph.graph(), &inputs)?;
        let output = dense_tensor(values.get(&activated.id()).expect("forward output"));
        assert_close_slice(&output, &[1.0, 0.0625, 0.5625, 9.0], 1e-6);
        Ok(())
    }

    #[test]
    fn reverse_mode_autodiff_materializes_leaky_relu_squared_gradients_against_finite_difference(
    ) -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![4]), DType::F32, true);
        let activated = builder.leaky_relu_squared(&input, 0.5)?;
        let scale = builder.constant_f32(Shape::new(vec![4]), vec![0.5, -1.0, 0.25, 1.5])?;
        let scaled = builder.mul(&activated, &scale)?;
        let loss = builder.reduce_sum(&scaled);
        let graph = builder.finish(vec![loss.clone()]);

        let backward_plan = graph.backward_plan(loss.id())?;
        assert!(backward_plan.gradient_for(input.id()).is_some());
        assert!(backward_plan
            .gradient_graph
            .nodes()
            .iter()
            .any(|node| matches!(
                node.op(),
                crate::OpKind::BackendExtension {
                    op: psionic_core::BackendExtensionOp::LeakyReluSquaredBackward { .. }
                }
            )));

        let input_values = vec![-1.25_f32, -0.5, 0.75, 2.0];
        let inputs = BTreeMap::from([(input.id(), TensorData::F32(input_values.clone()))]);
        let result = graph.backward_materialized(loss.id(), &inputs)?;
        let analytical = dense_gradient(&result, input.id());

        let delta = 1e-3_f32;
        let mut finite = vec![0.0_f32; input_values.len()];
        for index in 0..input_values.len() {
            let mut plus = input_values.clone();
            plus[index] += delta;
            let mut minus = input_values.clone();
            minus[index] -= delta;
            finite[index] = (scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([(input.id(), TensorData::F32(plus))]),
            )? - scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([(input.id(), TensorData::F32(minus))]),
            )?) / (2.0 * delta);
        }

        assert_close_slice(&analytical, &finite, 2e-3);
        Ok(())
    }

    #[test]
    fn reference_evaluation_executes_rms_norm_backend_extension() -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![1, 2]), DType::F32, true);
        let weight = builder.input("weight", Shape::new(vec![2]), DType::F32, true);
        let normalized = builder.rms_norm(&input, &weight, 1e-5)?;
        let graph = builder.finish(vec![normalized.clone()]);

        let inputs = BTreeMap::from([
            (input.id(), TensorData::F32(vec![3.0, 4.0])),
            (weight.id(), TensorData::F32(vec![1.5, 0.5])),
        ]);
        let values = evaluate_graph(graph.graph(), &inputs)?;
        let output = dense_tensor(values.get(&normalized.id()).expect("forward output"));
        let inv = (12.5_f32 + 1e-5).sqrt().recip();
        assert_close_slice(&output, &[3.0 * 1.5 * inv, 4.0 * 0.5 * inv], 1e-5);
        Ok(())
    }

    #[test]
    fn reference_evaluation_executes_parameter_golf_projection_loss_backend_extension(
    ) -> Result<(), Box<dyn Error>> {
        let logit_softcap = 15.0_f32;
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let logits = builder.input("logits", Shape::new(vec![1, 2, 3]), DType::F32, true);
        let target_ids = builder.input("target_ids", Shape::new(vec![1, 2]), DType::I32, false);
        let loss = builder.parameter_golf_projection_loss(&logits, &target_ids, logit_softcap)?;
        let graph = builder.finish(vec![loss.clone()]);

        let logits_values = vec![1.25_f32, -0.5, 0.75, -0.25, 0.5, 1.5];
        let target_values = vec![2_i32, 1_i32];
        let values = evaluate_graph(
            graph.graph(),
            &BTreeMap::from([
                (logits.id(), TensorData::F32(logits_values.clone())),
                (target_ids.id(), TensorData::I32(target_values)),
            ]),
        )?;
        let actual = dense_tensor(values.get(&loss.id()).expect("forward loss"));

        let expected = {
            let rows = [[1.25_f32, -0.5, 0.75], [-0.25_f32, 0.5, 1.5]];
            let targets = [2_usize, 1_usize];
            let mut total = 0.0_f32;
            for (row, target) in rows.iter().zip(targets) {
                let projected = row
                    .iter()
                    .map(|logit| logit_softcap * (logit / logit_softcap).tanh())
                    .collect::<Vec<_>>();
                let max = projected.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                let denom = projected
                    .iter()
                    .map(|value| (value - max).exp())
                    .sum::<f32>();
                let log_prob = projected[target] - max - denom.ln();
                total += -log_prob;
            }
            total / rows.len() as f32
        };

        assert_close_slice(&actual, &[expected], 1e-6);
        Ok(())
    }

    #[test]
    fn reverse_mode_autodiff_materializes_parameter_golf_projection_loss_gradients_against_finite_difference(
    ) -> Result<(), Box<dyn Error>> {
        let logit_softcap = 30.0_f32;
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let logits = builder.input("logits", Shape::new(vec![1, 2, 4]), DType::F32, true);
        let target_ids = builder.input("target_ids", Shape::new(vec![1, 2]), DType::I32, false);
        let loss = builder.parameter_golf_projection_loss(&logits, &target_ids, logit_softcap)?;
        let graph = builder.finish(vec![loss.clone()]);

        let backward_plan = graph.backward_plan(loss.id())?;
        assert!(backward_plan.gradient_for(logits.id()).is_some());
        assert!(backward_plan.gradient_for(target_ids.id()).is_none());
        assert!(backward_plan
            .gradient_graph
            .nodes()
            .iter()
            .any(|node| matches!(
                node.op(),
                crate::OpKind::BackendExtension {
                    op: psionic_core::BackendExtensionOp::ParameterGolfProjectionLossBackward { .. }
                }
            )));

        let logits_values = vec![0.1_f32, -0.2, 0.3, 0.7, -0.4, 0.5, -0.1, 0.2];
        let target_values = vec![3_i32, 1_i32];
        let inputs = BTreeMap::from([
            (logits.id(), TensorData::F32(logits_values.clone())),
            (target_ids.id(), TensorData::I32(target_values.clone())),
        ]);
        let result = graph.backward_materialized(loss.id(), &inputs)?;
        let analytical = dense_gradient(&result, logits.id());

        let delta = 1e-3_f32;
        let mut finite = vec![0.0_f32; logits_values.len()];
        for index in 0..logits_values.len() {
            let mut plus = logits_values.clone();
            plus[index] += delta;
            let mut minus = logits_values.clone();
            minus[index] -= delta;
            finite[index] = (scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (logits.id(), TensorData::F32(plus)),
                    (target_ids.id(), TensorData::I32(target_values.clone())),
                ]),
            )? - scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (logits.id(), TensorData::F32(minus)),
                    (target_ids.id(), TensorData::I32(target_values.clone())),
                ]),
            )?) / (2.0 * delta);
        }

        assert_close_slice(&analytical, &finite, 2e-3);
        Ok(())
    }

    #[test]
    fn reference_evaluation_executes_parameter_golf_token_embedding_lookup_backend_extension(
    ) -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let token_ids = builder.input("token_ids", Shape::new(vec![2, 2]), DType::I32, false);
        let embeddings = builder.input("tok_emb", Shape::new(vec![5, 3]), DType::F32, true);
        let looked_up = builder.parameter_golf_token_embedding_lookup(&token_ids, &embeddings)?;
        let graph = builder.finish(vec![looked_up.clone()]);

        let values = evaluate_graph(
            graph.graph(),
            &BTreeMap::from([
                (token_ids.id(), TensorData::I32(vec![1, 4, 0, 2])),
                (
                    embeddings.id(),
                    TensorData::F32(vec![
                        0.0, 0.1, 0.2, 1.0, 1.1, 1.2, 2.0, 2.1, 2.2, 3.0, 3.1, 3.2, 4.0, 4.1, 4.2,
                    ]),
                ),
            ]),
        )?;
        let actual = dense_tensor(values.get(&looked_up.id()).expect("embedding output"));
        assert_close_slice(
            &actual,
            &[1.0, 1.1, 1.2, 4.0, 4.1, 4.2, 0.0, 0.1, 0.2, 2.0, 2.1, 2.2],
            1e-6,
        );
        Ok(())
    }

    #[test]
    fn reverse_mode_autodiff_materializes_parameter_golf_token_embedding_lookup_gradients_against_finite_difference(
    ) -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let token_ids = builder.input("token_ids", Shape::new(vec![2, 2]), DType::I32, false);
        let embeddings = builder.input("tok_emb", Shape::new(vec![4, 2]), DType::F32, true);
        let looked_up = builder.parameter_golf_token_embedding_lookup(&token_ids, &embeddings)?;
        let scale = builder.constant_f32(
            Shape::new(vec![2, 2, 2]),
            vec![1.0, 0.5, -1.0, 0.25, 0.75, -0.5, 0.5, 1.25],
        )?;
        let weighted = builder.mul(&looked_up, &scale)?;
        let loss = builder.reduce_sum(&weighted);
        let graph = builder.finish(vec![loss.clone()]);

        let token_values = vec![1_i32, 3_i32, 0_i32, 1_i32];
        let embedding_values = vec![0.5, -0.5, 1.0, 1.5, -1.0, 0.25, 2.0, -1.5];
        let inputs = BTreeMap::from([
            (token_ids.id(), TensorData::I32(token_values.clone())),
            (embeddings.id(), TensorData::F32(embedding_values.clone())),
        ]);
        let backward = graph.backward_materialized(loss.id(), &inputs)?;
        let analytical = dense_gradient(&backward, embeddings.id());

        let delta = 1e-3_f32;
        let mut finite = vec![0.0_f32; embedding_values.len()];
        for index in 0..embedding_values.len() {
            let mut plus = embedding_values.clone();
            plus[index] += delta;
            let mut minus = embedding_values.clone();
            minus[index] -= delta;
            finite[index] = (scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (token_ids.id(), TensorData::I32(token_values.clone())),
                    (embeddings.id(), TensorData::F32(plus)),
                ]),
            )? - scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (token_ids.id(), TensorData::I32(token_values.clone())),
                    (embeddings.id(), TensorData::F32(minus)),
                ]),
            )?) / (2.0 * delta);
        }

        assert_close_slice(&analytical, &finite, 2e-3);
        Ok(())
    }

    #[test]
    fn reverse_mode_autodiff_materializes_rms_norm_gradients_against_finite_difference(
    ) -> Result<(), Box<dyn Error>> {
        let epsilon = 1e-5_f32;
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![2, 3]), DType::F32, true);
        let weight = builder.input("weight", Shape::new(vec![3]), DType::F32, true);
        let normalized = builder.rms_norm(&input, &weight, epsilon)?;
        let scale = builder.constant_f32(
            Shape::new(vec![2, 3]),
            vec![0.5, -1.0, 0.25, 1.5, -0.75, 0.8],
        )?;
        let scaled = builder.mul(&normalized, &scale)?;
        let loss = builder.reduce_sum(&scaled);
        let graph = builder.finish(vec![loss.clone()]);

        let backward_plan = graph.backward_plan(loss.id())?;
        assert!(backward_plan.gradient_for(input.id()).is_some());
        assert!(backward_plan.gradient_for(weight.id()).is_some());
        assert!(backward_plan
            .gradient_graph
            .nodes()
            .iter()
            .any(|node| matches!(
                node.op(),
                crate::OpKind::BackendExtension {
                    op: psionic_core::BackendExtensionOp::RmsNormInputBackward { .. }
                }
            )));
        assert!(backward_plan
            .gradient_graph
            .nodes()
            .iter()
            .any(|node| matches!(
                node.op(),
                crate::OpKind::BackendExtension {
                    op: psionic_core::BackendExtensionOp::RmsNormWeightBackward { .. }
                }
            )));

        let input_values = vec![0.3_f32, -0.8, 1.1, -1.0, 0.5, 0.25];
        let weight_values = vec![1.2_f32, -0.7, 0.9];
        let inputs = BTreeMap::from([
            (input.id(), TensorData::F32(input_values.clone())),
            (weight.id(), TensorData::F32(weight_values.clone())),
        ]);
        let result = graph.backward_materialized(loss.id(), &inputs)?;
        let analytical_input = dense_gradient(&result, input.id());
        let analytical_weight = dense_gradient(&result, weight.id());

        let delta = 1e-3_f32;
        let mut finite_input = vec![0.0_f32; input_values.len()];
        for index in 0..input_values.len() {
            let mut plus = input_values.clone();
            plus[index] += delta;
            let mut minus = input_values.clone();
            minus[index] -= delta;
            finite_input[index] = (scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (input.id(), TensorData::F32(plus)),
                    (weight.id(), TensorData::F32(weight_values.clone())),
                ]),
            )? - scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (input.id(), TensorData::F32(minus)),
                    (weight.id(), TensorData::F32(weight_values.clone())),
                ]),
            )?) / (2.0 * delta);
        }
        let mut finite_weight = vec![0.0_f32; weight_values.len()];
        for index in 0..weight_values.len() {
            let mut plus = weight_values.clone();
            plus[index] += delta;
            let mut minus = weight_values.clone();
            minus[index] -= delta;
            finite_weight[index] = (scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (input.id(), TensorData::F32(input_values.clone())),
                    (weight.id(), TensorData::F32(plus)),
                ]),
            )? - scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (input.id(), TensorData::F32(input_values.clone())),
                    (weight.id(), TensorData::F32(minus)),
                ]),
            )?) / (2.0 * delta);
        }

        assert_close_slice(&analytical_input, &finite_input, 2e-3);
        assert_close_slice(&analytical_weight, &finite_weight, 2e-3);
        Ok(())
    }

    #[test]
    fn reverse_mode_autodiff_materializes_rotary_embedding_input_gradients_against_finite_difference(
    ) -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![1, 2, 2, 4]), DType::F32, true);
        let cos = builder.constant_f32(Shape::new(vec![2, 2]), vec![1.0, 0.75, 0.5, 0.25])?;
        let sin = builder.constant_f32(Shape::new(vec![2, 2]), vec![0.0, 0.2, 0.35, 0.45])?;
        let roped = builder.rope(&input, &cos, &sin, false)?;
        let scale = builder.constant_f32(
            Shape::new(vec![1, 2, 2, 4]),
            vec![
                0.5, -1.0, 0.25, 1.5, -0.75, 0.8, 0.1, -0.4, 0.3, -0.2, 0.6, -0.9, 0.4, 0.7, -0.5,
                0.2,
            ],
        )?;
        let scaled = builder.mul(&roped, &scale)?;
        let loss = builder.reduce_sum(&scaled);
        let graph = builder.finish(vec![loss.clone()]);

        let backward_plan = graph.backward_plan(loss.id())?;
        assert!(backward_plan.gradient_for(input.id()).is_some());
        assert!(backward_plan
            .gradient_graph
            .nodes()
            .iter()
            .any(|node| matches!(
                node.op(),
                crate::OpKind::BackendExtension {
                    op: psionic_core::BackendExtensionOp::RotaryEmbeddingBackward { .. }
                }
            )));

        let input_values = vec![
            0.1_f32, -0.3, 0.5, 0.7, -0.2, 0.4, -0.6, 0.8, 0.9, -1.1, 1.3, -1.5, 0.25, -0.45, 0.65,
            -0.85,
        ];
        let inputs = BTreeMap::from([(input.id(), TensorData::F32(input_values.clone()))]);
        let result = graph.backward_materialized(loss.id(), &inputs)?;
        let analytical = dense_gradient(&result, input.id());

        let delta = 1e-3_f32;
        let mut finite = vec![0.0_f32; input_values.len()];
        for index in 0..input_values.len() {
            let mut plus = input_values.clone();
            plus[index] += delta;
            let mut minus = input_values.clone();
            minus[index] -= delta;
            finite[index] = (scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([(input.id(), TensorData::F32(plus))]),
            )? - scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([(input.id(), TensorData::F32(minus))]),
            )?) / (2.0 * delta);
        }

        assert_close_slice(&analytical, &finite, 2e-3);
        Ok(())
    }

    #[test]
    fn reverse_mode_autodiff_materializes_gqa_attention_gradients_against_finite_difference(
    ) -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let query = builder.input("q", Shape::new(vec![1, 2, 2, 4]), DType::F32, true);
        let key = builder.input("k", Shape::new(vec![1, 1, 2, 4]), DType::F32, true);
        let value = builder.input("v", Shape::new(vec![1, 1, 2, 4]), DType::F32, true);
        let attended = builder.scaled_dot_product_attention(&query, &key, &value, 0.5, true)?;
        let scale = builder.constant_f32(
            Shape::new(vec![1, 2, 2, 4]),
            vec![
                0.25, -0.75, 1.25, -0.5, 0.1, 0.9, -0.2, 0.4, -0.3, 0.7, -0.6, 0.8, 1.1, -0.4, 0.2,
                -0.9,
            ],
        )?;
        let scaled = builder.mul(&attended, &scale)?;
        let loss = builder.reduce_sum(&scaled);
        let graph = builder.finish(vec![loss.clone()]);

        let backward_plan = graph.backward_plan(loss.id())?;
        assert!(backward_plan.gradient_for(query.id()).is_some());
        assert!(backward_plan.gradient_for(key.id()).is_some());
        assert!(backward_plan.gradient_for(value.id()).is_some());
        assert!(backward_plan
            .gradient_graph
            .nodes()
            .iter()
            .any(|node| matches!(
                node.op(),
                crate::OpKind::BackendExtension {
                    op: psionic_core::BackendExtensionOp::ScaledDotProductAttentionQueryBackward { .. }
                }
            )));
        assert!(backward_plan
            .gradient_graph
            .nodes()
            .iter()
            .any(|node| matches!(
                node.op(),
                crate::OpKind::BackendExtension {
                    op: psionic_core::BackendExtensionOp::ScaledDotProductAttentionKeyBackward { .. }
                }
            )));
        assert!(backward_plan
            .gradient_graph
            .nodes()
            .iter()
            .any(|node| matches!(
                node.op(),
                crate::OpKind::BackendExtension {
                    op: psionic_core::BackendExtensionOp::ScaledDotProductAttentionValueBackward { .. }
                }
            )));

        let query_values = vec![
            0.1_f32, -0.2, 0.3, -0.4, 0.5, -0.6, 0.7, -0.8, -0.9, 1.0, -1.1, 1.2, -1.3, 1.4, -1.5,
            1.6,
        ];
        let key_values = vec![0.2_f32, -0.1, 0.4, -0.3, 0.6, -0.5, 0.8, -0.7];
        let value_values = vec![-0.15_f32, 0.25, -0.35, 0.45, -0.55, 0.65, -0.75, 0.85];
        let inputs = BTreeMap::from([
            (query.id(), TensorData::F32(query_values.clone())),
            (key.id(), TensorData::F32(key_values.clone())),
            (value.id(), TensorData::F32(value_values.clone())),
        ]);
        let result = graph.backward_materialized(loss.id(), &inputs)?;
        let analytical_query = dense_gradient(&result, query.id());
        let analytical_key = dense_gradient(&result, key.id());
        let analytical_value = dense_gradient(&result, value.id());

        let delta = 1e-3_f32;
        let mut finite_query = vec![0.0_f32; query_values.len()];
        for index in 0..query_values.len() {
            let mut plus = query_values.clone();
            plus[index] += delta;
            let mut minus = query_values.clone();
            minus[index] -= delta;
            finite_query[index] = (scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (query.id(), TensorData::F32(plus)),
                    (key.id(), TensorData::F32(key_values.clone())),
                    (value.id(), TensorData::F32(value_values.clone())),
                ]),
            )? - scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (query.id(), TensorData::F32(minus)),
                    (key.id(), TensorData::F32(key_values.clone())),
                    (value.id(), TensorData::F32(value_values.clone())),
                ]),
            )?) / (2.0 * delta);
        }
        let mut finite_key = vec![0.0_f32; key_values.len()];
        for index in 0..key_values.len() {
            let mut plus = key_values.clone();
            plus[index] += delta;
            let mut minus = key_values.clone();
            minus[index] -= delta;
            finite_key[index] = (scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (query.id(), TensorData::F32(query_values.clone())),
                    (key.id(), TensorData::F32(plus)),
                    (value.id(), TensorData::F32(value_values.clone())),
                ]),
            )? - scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (query.id(), TensorData::F32(query_values.clone())),
                    (key.id(), TensorData::F32(minus)),
                    (value.id(), TensorData::F32(value_values.clone())),
                ]),
            )?) / (2.0 * delta);
        }
        let mut finite_value = vec![0.0_f32; value_values.len()];
        for index in 0..value_values.len() {
            let mut plus = value_values.clone();
            plus[index] += delta;
            let mut minus = value_values.clone();
            minus[index] -= delta;
            finite_value[index] = (scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (query.id(), TensorData::F32(query_values.clone())),
                    (key.id(), TensorData::F32(key_values.clone())),
                    (value.id(), TensorData::F32(plus)),
                ]),
            )? - scalar_loss(
                graph.graph(),
                loss.id(),
                BTreeMap::from([
                    (query.id(), TensorData::F32(query_values.clone())),
                    (key.id(), TensorData::F32(key_values.clone())),
                    (value.id(), TensorData::F32(minus)),
                ]),
            )?) / (2.0 * delta);
        }

        assert_close_slice(&analytical_query, &finite_query, 3e-3);
        assert_close_slice(&analytical_key, &finite_key, 3e-3);
        assert_close_slice(&analytical_value, &finite_value, 3e-3);
        Ok(())
    }

    #[test]
    fn backward_plan_deduplicates_gradient_graph_outputs_for_residual_mix_graph(
    ) -> Result<(), Box<dyn Error>> {
        let shape = Shape::new(vec![1, 2, 4]);
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let current = builder.input("current", shape.clone(), DType::F32, true);
        let source = builder.input("source", shape.clone(), DType::F32, true);
        let mix_current = builder.input("mix_current", shape.clone(), DType::F32, true);
        let mix_source = builder.input("mix_source", shape.clone(), DType::F32, true);
        let mixed_current = builder.mul(&current, &mix_current)?;
        let mixed_source = builder.mul(&source, &mix_source)?;
        let mixed = builder.add(&mixed_current, &mixed_source)?;
        let graph = builder.finish(vec![mixed.clone()]);

        let backward_plan = graph.backward_plan(mixed.id())?;
        let unique_outputs = backward_plan
            .gradient_graph
            .outputs()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            unique_outputs.len(),
            backward_plan.gradient_graph.outputs().len()
        );
        Ok(())
    }

    #[test]
    fn unsupported_gradient_ops_refuse_through_typed_error() -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![1, 2]), DType::F32, true);
        let weight = builder.input("weight", Shape::new(vec![2]), DType::F32, true);
        let bias = builder.input("bias", Shape::new(vec![2]), DType::F32, true);
        let normalized = builder.layer_norm(&input, &weight, &bias, 1e-5)?;
        let loss = builder.reduce_sum(&normalized);
        let graph = builder.finish(vec![loss.clone()]);

        assert_eq!(
            graph.backward_plan(loss.id()),
            Err(AutodiffError::UnsupportedGradientOp {
                tensor_id: normalized.id(),
                op: String::from("layer_norm"),
            })
        );
        Ok(())
    }

    #[test]
    fn autodiff_refusal_taxonomy_maps_unsupported_gradient_family() {
        let refusal = AutodiffError::UnsupportedGradientOp {
            tensor_id: TensorId(7),
            op: String::from("layer_norm"),
        }
        .refusal();
        assert!(refusal.is_some());
        let Some(refusal) = refusal else {
            return;
        };
        assert_eq!(refusal.code, PsionicRefusalCode::UnsupportedGradient);
        assert_eq!(refusal.scope, PsionicRefusalScope::Autodiff);
        assert_eq!(refusal.subject.as_deref(), Some("TensorId(7)"));
    }

    #[test]
    fn autodiff_support_matrix_marks_primitives_float_casts_rms_norm_and_other_extensions_explicitly(
    ) {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![2, 2]), DType::F32, true);
        let row = builder.select(&input, 0, 0).expect("select");
        let row = builder
            .reshape(&row, Shape::new(vec![1, 2]))
            .expect("reshape");
        let tail = builder.slice(&input, 0, 1, 2).expect("slice");
        let combined = builder.concat(&[row, tail], 0).expect("concat");
        let expanded = builder
            .expand(&combined, Shape::new(vec![2, 2]))
            .expect("expand");
        let permuted = builder.permute(&expanded, vec![1, 0]).expect("permute");
        let casted = builder.cast(&permuted, DType::BF16).expect("float cast");
        let reduced = builder.reduce_sum_axis(&casted, 0).expect("axis reduce");
        let loss = builder.reduce_sum(&reduced);
        let graph = builder.finish(vec![loss]);

        for node in graph.graph().nodes() {
            assert_eq!(
                gradient_support_for_op(node.op()),
                AutodiffGradientSupport::Implemented
            );
        }

        let mut extension_builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let ext_input = extension_builder.input("x", Shape::new(vec![1, 2]), DType::F32, true);
        let ext_weight = extension_builder.input("weight", Shape::new(vec![2]), DType::F32, true);
        let normalized = extension_builder
            .rms_norm(&ext_input, &ext_weight, 1e-5)
            .expect("rms_norm");
        let extension_graph = extension_builder.finish(vec![normalized]);
        let Some(node) = extension_graph
            .graph()
            .nodes()
            .iter()
            .find(|node| matches!(node.op(), crate::OpKind::BackendExtension { .. }))
        else {
            panic!("backend extension node should exist");
        };
        assert_eq!(
            gradient_support_for_op(node.op()),
            AutodiffGradientSupport::Implemented
        );

        let mut rope_builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let rope_input = rope_builder.input("x", Shape::new(vec![1, 2, 2, 4]), DType::F32, true);
        let cos = rope_builder
            .constant_f32(Shape::new(vec![2, 2]), vec![1.0, 0.5, 0.25, 0.75])
            .expect("cos");
        let sin = rope_builder
            .constant_f32(Shape::new(vec![2, 2]), vec![0.0, 0.1, 0.2, 0.3])
            .expect("sin");
        let roped = rope_builder
            .rope(&rope_input, &cos, &sin, false)
            .expect("rope");
        let rope_graph = rope_builder.finish(vec![roped]);
        let Some(node) = rope_graph
            .graph()
            .nodes()
            .iter()
            .find(|node| matches!(node.op(), crate::OpKind::BackendExtension { .. }))
        else {
            panic!("backend extension node should exist");
        };
        assert_eq!(
            gradient_support_for_op(node.op()),
            AutodiffGradientSupport::Implemented
        );

        let mut projection_builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let logits =
            projection_builder.input("logits", Shape::new(vec![1, 2, 4]), DType::F32, true);
        let target_ids =
            projection_builder.input("target_ids", Shape::new(vec![1, 2]), DType::I32, false);
        let projection_loss = projection_builder
            .parameter_golf_projection_loss(&logits, &target_ids, 30.0)
            .expect("parameter_golf_projection_loss");
        let projection_graph = projection_builder.finish(vec![projection_loss]);
        let Some(node) = projection_graph
            .graph()
            .nodes()
            .iter()
            .find(|node| matches!(node.op(), crate::OpKind::BackendExtension { .. }))
        else {
            panic!("backend extension node should exist");
        };
        assert_eq!(
            gradient_support_for_op(node.op()),
            AutodiffGradientSupport::Implemented
        );

        let mut attention_builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let query = attention_builder.input("q", Shape::new(vec![1, 2, 2, 4]), DType::F32, true);
        let key = attention_builder.input("k", Shape::new(vec![1, 1, 2, 4]), DType::F32, true);
        let value = attention_builder.input("v", Shape::new(vec![1, 1, 2, 4]), DType::F32, true);
        let attended = attention_builder
            .scaled_dot_product_attention(&query, &key, &value, 0.5, true)
            .expect("attention");
        let attention_graph = attention_builder.finish(vec![attended]);
        let Some(node) = attention_graph
            .graph()
            .nodes()
            .iter()
            .find(|node| matches!(node.op(), crate::OpKind::BackendExtension { .. }))
        else {
            panic!("backend extension node should exist");
        };
        assert_eq!(
            gradient_support_for_op(node.op()),
            AutodiffGradientSupport::Implemented
        );

        let mut unsupported_builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let unsupported_input =
            unsupported_builder.input("x", Shape::new(vec![1, 2]), DType::F32, true);
        let unsupported_weight =
            unsupported_builder.input("weight", Shape::new(vec![2]), DType::F32, true);
        let unsupported_bias =
            unsupported_builder.input("bias", Shape::new(vec![2]), DType::F32, true);
        let layer_norm = unsupported_builder
            .layer_norm(
                &unsupported_input,
                &unsupported_weight,
                &unsupported_bias,
                1e-5,
            )
            .expect("layer_norm");
        let unsupported_graph = unsupported_builder.finish(vec![layer_norm]);
        let Some(node) = unsupported_graph
            .graph()
            .nodes()
            .iter()
            .find(|node| matches!(node.op(), crate::OpKind::BackendExtension { .. }))
        else {
            panic!("backend extension node should exist");
        };
        assert_eq!(
            gradient_support_for_op(node.op()),
            AutodiffGradientSupport::Unsupported {
                reason: AutodiffUnsupportedGradientReason::BackendExtensionFamily,
            }
        );
    }

    #[test]
    fn vmap_support_matrix_marks_primitives_casts_and_backend_extensions_explicitly() {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![2, 2]), DType::F32, true);
        let row = builder.select(&input, 0, 0).expect("select");
        let row = builder
            .reshape(&row, Shape::new(vec![1, 2]))
            .expect("reshape");
        let tail = builder.slice(&input, 0, 1, 2).expect("slice");
        let combined = builder.concat(&[row, tail], 0).expect("concat");
        let expanded = builder
            .expand(&combined, Shape::new(vec![2, 2]))
            .expect("expand");
        let permuted = builder.permute(&expanded, vec![1, 0]).expect("permute");
        let casted = builder.cast(&permuted, DType::I8).expect("cast");
        let primitive_graph = builder.finish(vec![casted.clone()]);

        for node in primitive_graph.graph().nodes() {
            let expected = if matches!(node.op(), crate::OpKind::Cast { .. }) {
                VmapSupport::Unsupported {
                    reason: VmapUnsupportedReason::CastFamily,
                }
            } else {
                VmapSupport::Implemented
            };
            assert_eq!(vmap_support_for_op(node.op()), expected);
        }

        let mut extension_builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let ext_input = extension_builder.input("x", Shape::new(vec![1, 2]), DType::F32, true);
        let ext_weight = extension_builder.input("weight", Shape::new(vec![2]), DType::F32, true);
        let normalized = extension_builder
            .rms_norm(&ext_input, &ext_weight, 1e-5)
            .expect("rms_norm");
        let extension_graph = extension_builder.finish(vec![normalized]);
        let Some(node) = extension_graph
            .graph()
            .nodes()
            .iter()
            .find(|node| matches!(node.op(), crate::OpKind::BackendExtension { .. }))
        else {
            panic!("backend extension node should exist");
        };
        assert_eq!(
            vmap_support_for_op(node.op()),
            VmapSupport::Unsupported {
                reason: VmapUnsupportedReason::BackendExtensionFamily,
            }
        );
    }

    #[test]
    fn reverse_mode_autodiff_covers_select_concat_and_reshape_primitives(
    ) -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let x = builder.input("x", Shape::new(vec![2, 2]), DType::F32, true);
        let row = builder.select(&x, 0, 0)?;
        let row = builder.reshape(&row, Shape::new(vec![1, 2]))?;
        let tail = builder.slice(&x, 0, 1, 2)?;
        let combined = builder.concat(&[row, tail], 0)?;
        let loss = builder.reduce_sum(&combined);
        let graph = builder.finish(vec![loss.clone()]);

        let inputs = BTreeMap::from([(x.id(), TensorData::F32(vec![1.0, 2.0, 3.0, 4.0]))]);
        let result = graph.backward_materialized(loss.id(), &inputs)?;

        assert_eq!(dense_gradient(&result, x.id()), vec![1.0, 1.0, 1.0, 1.0]);
        Ok(())
    }

    #[test]
    fn reverse_mode_autodiff_accepts_non_scalar_axis_seed() -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let x = builder.input("x", Shape::new(vec![2, 2]), DType::F32, true);
        let axis_sum = builder.reduce_sum_axis(&x, 0)?;
        let graph = builder.finish(vec![axis_sum.clone()]);

        let inputs = BTreeMap::from([(x.id(), TensorData::F32(vec![1.0, 2.0, 3.0, 4.0]))]);
        let seed = Some(TensorData::F32(vec![1.0, 2.0]));
        let result = graph.backward_materialized_with_seed(axis_sum.id(), &inputs, seed)?;

        assert_eq!(dense_gradient(&result, x.id()), vec![1.0, 2.0, 1.0, 2.0]);
        Ok(())
    }

    #[test]
    fn reverse_mode_autodiff_accepts_bf16_training_targets() -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let x = builder.input("x", Shape::new(vec![1]), DType::BF16, true);
        let squared = builder.mul(&x, &x)?;
        let graph = builder.finish(vec![squared.clone()]);

        let result = graph.backward_materialized(
            squared.id(),
            &BTreeMap::from([(x.id(), TensorData::BF16(vec![3.0]))]),
        )?;

        assert_eq!(dense_gradient(&result, x.id()), vec![6.0]);
        Ok(())
    }

    #[test]
    fn reverse_mode_autodiff_propagates_float_cast_gradients() -> Result<(), Box<dyn Error>> {
        let mut f32_to_bf16_builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let x = f32_to_bf16_builder.input("x", Shape::new(vec![2]), DType::F32, true);
        let lowered = f32_to_bf16_builder.cast(&x, DType::BF16)?;
        let lowered_loss = f32_to_bf16_builder.reduce_sum(&lowered);
        let lowered_graph = f32_to_bf16_builder.finish(vec![lowered_loss.clone()]);
        let lowered_result = lowered_graph.backward_materialized(
            lowered_loss.id(),
            &BTreeMap::from([(x.id(), TensorData::F32(vec![1.5, -2.5]))]),
        )?;
        assert_eq!(dense_gradient(&lowered_result, x.id()), vec![1.0, 1.0]);

        let mut bf16_to_f32_builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let y = bf16_to_f32_builder.input("y", Shape::new(vec![2]), DType::BF16, true);
        let widened = bf16_to_f32_builder.cast(&y, DType::F32)?;
        let widened_loss = bf16_to_f32_builder.reduce_sum(&widened);
        let widened_graph = bf16_to_f32_builder.finish(vec![widened_loss.clone()]);
        let widened_result = widened_graph.backward_materialized(
            widened_loss.id(),
            &BTreeMap::from([(y.id(), TensorData::BF16(vec![0.25, -0.75]))]),
        )?;
        assert_eq!(dense_gradient(&widened_result, y.id()), vec![1.0, 1.0]);

        Ok(())
    }

    #[test]
    fn unsupported_gradient_backend_extensions_refuse_per_op_label() -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![1, 2]), DType::F32, true);
        let weight = builder.input("weight", Shape::new(vec![2]), DType::F32, true);
        let bias = builder.input("bias", Shape::new(vec![2]), DType::F32, true);
        let layer_norm = builder.layer_norm(&input, &weight, &bias, 1e-5)?;
        let layer_loss = builder.reduce_sum(&layer_norm);
        let layer_graph = builder.finish(vec![layer_loss.clone()]);
        assert_eq!(
            layer_graph.backward_plan(layer_loss.id()),
            Err(AutodiffError::UnsupportedGradientOp {
                tensor_id: layer_norm.id(),
                op: String::from("layer_norm"),
            })
        );

        let mut rope_builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let rope_input = rope_builder.input("x", Shape::new(vec![1, 1, 2, 4]), DType::F32, true);
        let cos = rope_builder.input("cos", Shape::new(vec![2, 2]), DType::F32, true);
        let sin = rope_builder.input("sin", Shape::new(vec![2, 2]), DType::F32, true);
        let roped = rope_builder.rope(&rope_input, &cos, &sin, false)?;
        let rope_loss = rope_builder.reduce_sum(&roped);
        let rope_graph = rope_builder.finish(vec![rope_loss.clone()]);
        assert_eq!(
            rope_graph.backward_plan(rope_loss.id()),
            Err(AutodiffError::UnsupportedGradientOp {
                tensor_id: roped.id(),
                op: String::from("rotary_embedding_table_gradients"),
            })
        );

        let mut quantized_builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let left = quantized_builder.input("left", Shape::new(vec![2, 32]), DType::F32, true);
        let rhs = quantized_builder.constant_quantized_blocks(
            Shape::new(vec![3, 32]),
            psionic_core::QuantizationMode::GgmlQ4_0,
            vec![0x88_u8; 54],
        )?;
        let output = quantized_builder.quantized_matmul(
            &left,
            &rhs,
            psionic_core::QuantizationMode::GgmlQ4_0,
        )?;
        let quantized_loss = quantized_builder.reduce_sum(&output);
        let quantized_graph = quantized_builder.finish(vec![quantized_loss.clone()]);
        assert_eq!(
            quantized_graph.backward_plan(quantized_loss.id()),
            Err(AutodiffError::UnsupportedGradientOp {
                tensor_id: output.id(),
                op: String::from("quantized_matmul"),
            })
        );

        Ok(())
    }

    #[test]
    fn reverse_mode_autodiff_covers_broadcast_and_view_primitives() -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let x = builder.input("x", Shape::new(vec![1, 2]), DType::F32, true);
        let expanded = builder.expand(&x, Shape::new(vec![3, 2]))?;
        let sliced = builder.slice(&expanded, 0, 1, 3)?;
        let permuted = builder.permute(&sliced, vec![1, 0])?;
        let loss = builder.reduce_sum(&permuted);
        let graph = builder.finish(vec![loss.clone()]);

        let inputs = BTreeMap::from([(x.id(), TensorData::F32(vec![1.5, -2.0]))]);
        let result = graph.backward_materialized(loss.id(), &inputs)?;

        assert_eq!(dense_gradient(&result, x.id()), vec![2.0, 2.0]);
        Ok(())
    }

    #[test]
    fn public_reverse_mode_transforms_expose_grad_value_and_grad_and_vjp(
    ) -> Result<(), Box<dyn Error>> {
        let mut grad_builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let x = grad_builder.input("x", Shape::new(vec![2]), DType::F32, true);
        let doubled = grad_builder.add(&x, &x)?;
        let loss = grad_builder.reduce_sum(&doubled);
        let grad_graph = grad_builder.finish(vec![loss.clone()]);
        let inputs = BTreeMap::from([(x.id(), TensorData::F32(vec![1.0, 2.0]))]);

        let grad_transform = grad(&grad_graph, loss.id(), &[x.id()])?;
        let grad_result = grad_transform.apply(&inputs)?;
        assert_eq!(
            dense_tensor(
                grad_result
                    .gradients
                    .get(&x.id())
                    .expect("grad result should include x")
            ),
            vec![2.0, 2.0]
        );

        let value_and_grad_transform = value_and_grad(&grad_graph, loss.id(), &[x.id()])?;
        let value_and_grad_result = value_and_grad_transform.apply(&inputs)?;
        assert_eq!(dense_tensor(&value_and_grad_result.value), vec![6.0]);
        assert_eq!(
            dense_tensor(
                value_and_grad_result
                    .gradients
                    .get(&x.id())
                    .expect("value_and_grad should include x")
            ),
            vec![2.0, 2.0]
        );

        let mut vjp_builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let vx = vjp_builder.input("vx", Shape::new(vec![2]), DType::F32, true);
        let bias = vjp_builder.constant_f32(Shape::new(vec![2]), vec![1.0, -1.0])?;
        let output = vjp_builder.add(&vx, &bias)?;
        let vjp_graph = vjp_builder.finish(vec![output.clone()]);
        let vjp_inputs = BTreeMap::from([(vx.id(), TensorData::F32(vec![3.0, 1.0]))]);

        let vjp_transform = vjp(&vjp_graph, output.id(), &[vx.id()])?;
        let vjp_result = vjp_transform.apply(&vjp_inputs, TensorData::F32(vec![0.5, 2.0]))?;
        assert_eq!(dense_tensor(&vjp_result.value), vec![4.0, 0.0]);
        assert_eq!(
            dense_tensor(
                vjp_result
                    .cotangents
                    .get(&vx.id())
                    .expect("vjp should include vx")
            ),
            vec![0.5, 2.0]
        );

        Ok(())
    }

    #[test]
    fn public_checkpoint_transform_replays_primal_bindings_and_materializes_gradients(
    ) -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let x = builder.input("x", Shape::new(vec![2]), DType::F32, true);
        let bias = builder.input("bias", Shape::new(vec![2]), DType::F32, true);
        let shifted = builder.add(&x, &bias)?;
        let squared = builder.mul(&shifted, &shifted)?;
        let loss = builder.reduce_sum(&squared);
        let graph = builder.finish(vec![loss.clone()]);
        let inputs = BTreeMap::from([
            (x.id(), TensorData::F32(vec![2.0, 3.0])),
            (bias.id(), TensorData::F32(vec![1.0, -1.0])),
        ]);

        let transform = checkpoint(&graph, loss.id(), &[x.id(), bias.id()])?;
        let result = transform.apply(&inputs)?;

        assert_eq!(dense_tensor(&result.value), vec![13.0]);
        assert_eq!(
            dense_tensor(
                result
                    .gradients
                    .get(&x.id())
                    .expect("checkpoint result should include x")
            ),
            vec![6.0, 4.0]
        );
        assert_eq!(
            dense_tensor(
                result
                    .gradients
                    .get(&bias.id())
                    .expect("checkpoint result should include bias")
            ),
            vec![6.0, 4.0]
        );
        assert_eq!(
            result.rematerialization.initial_forward.retained_tensors,
            vec![loss.id()]
        );
        assert_eq!(
            result.rematerialization.replay_forward.retained_tensors,
            vec![shifted.id()]
        );
        assert_eq!(
            result.rematerialization.replayed_binding_tensors,
            vec![shifted.id()]
        );
        assert!(
            result
                .rematerialization
                .initial_forward
                .dropped_tensor_count
                > 0
        );
        assert!(
            result.replayed_primal_values.contains_key(&shifted.id()),
            "checkpoint replay should retain the shifted intermediate for backward"
        );

        Ok(())
    }

    #[test]
    fn checkpoint_transform_refuses_cast_barriers() -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let x = builder.input("x", Shape::new(vec![2]), DType::F32, true);
        let casted = builder.cast(&x, DType::I8)?;
        let graph = builder.finish(vec![casted.clone()]);

        assert_eq!(
            checkpoint(&graph, casted.id(), &[x.id()]),
            Err(CheckpointTransformError::UnsupportedOp {
                tensor_id: casted.id(),
                op: String::from("cast"),
            })
        );
        Ok(())
    }

    #[test]
    fn public_custom_vjp_transform_uses_registered_rule() -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let x = builder.input("x", Shape::new(vec![2]), DType::F32, true);
        let bias = builder.constant_f32(Shape::new(vec![2]), vec![1.0, -1.0])?;
        let output = builder.add(&x, &bias)?;
        let graph = builder.finish(vec![output.clone()]);
        let inputs = BTreeMap::from([(x.id(), TensorData::F32(vec![3.0, 1.0]))]);

        let mut registry = TransformHookRegistry::default();
        let registration = registry.register_custom_vjp(
            &graph,
            output.id(),
            &[x.id()],
            Arc::new(ScalingCustomVjpRule {
                label: "triple_seed",
                scale: 3.0,
            }),
        )?;
        assert_eq!(registry.registrations(), vec![registration.clone()]);

        let transform = custom_vjp(&registry, &graph, output.id(), &[x.id()])?;
        let result = transform.apply_with_seed(&inputs, Some(TensorData::F32(vec![0.5, 2.0])))?;

        assert_eq!(dense_tensor(&result.value), vec![4.0, 0.0]);
        assert_eq!(result.registration.rule_label, "triple_seed");
        assert_eq!(
            dense_tensor(
                result
                    .cotangents
                    .get(&x.id())
                    .expect("custom_vjp should include x")
            ),
            vec![1.5, 6.0]
        );

        Ok(())
    }

    #[test]
    fn custom_vjp_registry_and_transform_refuse_missing_and_duplicate_rules(
    ) -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let x = builder.input("x", Shape::new(vec![2]), DType::F32, true);
        let output = builder.add(&x, &x)?;
        let graph = builder.finish(vec![output.clone()]);
        let inputs = BTreeMap::from([(x.id(), TensorData::F32(vec![1.0, 2.0]))]);

        let registry = TransformHookRegistry::default();
        let missing = custom_vjp(&registry, &graph, output.id(), &[x.id()]);
        assert!(matches!(
            missing,
            Err(TransformHookLookupError::MissingRegistration { .. })
        ));

        let mut registry = TransformHookRegistry::default();
        let _registration = registry.register_custom_vjp(
            &graph,
            output.id(),
            &[x.id()],
            Arc::new(ScalingCustomVjpRule {
                label: "duplicate_guard",
                scale: 1.0,
            }),
        )?;
        let duplicate = registry.register_custom_vjp(
            &graph,
            output.id(),
            &[x.id()],
            Arc::new(ScalingCustomVjpRule {
                label: "duplicate_guard",
                scale: 1.0,
            }),
        );
        assert!(matches!(
            duplicate,
            Err(TransformHookRegistrationError::DuplicateRegistration { .. })
        ));

        let mut invalid_registry = TransformHookRegistry::default();
        invalid_registry.register_custom_vjp(
            &graph,
            output.id(),
            &[x.id()],
            Arc::new(EmptyCustomVjpRule),
        )?;
        let invalid_transform = custom_vjp(&invalid_registry, &graph, output.id(), &[x.id()])?;
        assert_eq!(
            invalid_transform.apply_with_seed(&inputs, Some(TensorData::F32(vec![1.0, 1.0]))),
            Err(CustomVjpTransformError::MissingCotangentTarget { tensor_id: x.id() })
        );

        Ok(())
    }

    #[test]
    fn public_reverse_mode_transforms_refuse_invalid_targets_and_zero_disconnected_paths(
    ) -> Result<(), Box<dyn Error>> {
        let mut invalid_builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let tracked = invalid_builder.input("tracked", Shape::new(vec![2]), DType::F32, true);
        let untracked = invalid_builder.input("untracked", Shape::new(vec![2]), DType::F32, false);
        let nonscalar = invalid_builder.add(&tracked, &tracked)?;
        let invalid_graph = invalid_builder.finish(vec![nonscalar.clone()]);

        assert_eq!(
            grad(&invalid_graph, nonscalar.id(), &[tracked.id()]),
            Err(ReverseModeTransformError::NonSingletonOutput {
                transform: String::from("grad"),
                tensor_id: nonscalar.id(),
                shape: Shape::new(vec![2]),
            })
        );
        assert_eq!(
            value_and_grad(&invalid_graph, nonscalar.id(), &[tracked.id()]),
            Err(ReverseModeTransformError::NonSingletonOutput {
                transform: String::from("value_and_grad"),
                tensor_id: nonscalar.id(),
                shape: Shape::new(vec![2]),
            })
        );
        assert_eq!(
            vjp(&invalid_graph, nonscalar.id(), &[untracked.id()]),
            Err(ReverseModeTransformError::UntrackedTarget {
                tensor_id: untracked.id(),
            })
        );

        let mut zero_builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let x = zero_builder.input("x", Shape::new(vec![2]), DType::F32, true);
        let y = zero_builder.input("y", Shape::new(vec![2]), DType::F32, true);
        let loss = zero_builder.reduce_sum(&x);
        let zero_graph = zero_builder.finish(vec![loss.clone()]);
        let zero_transform = grad(&zero_graph, loss.id(), &[x.id(), y.id()])?;
        let zero_inputs = BTreeMap::from([
            (x.id(), TensorData::F32(vec![2.0, -1.0])),
            (y.id(), TensorData::F32(vec![9.0, 5.0])),
        ]);
        let zero_result = zero_transform.apply(&zero_inputs)?;

        assert_eq!(
            dense_tensor(
                zero_result
                    .gradients
                    .get(&x.id())
                    .expect("x gradient should be present")
            ),
            vec![1.0, 1.0]
        );
        assert_eq!(
            dense_tensor(
                zero_result
                    .gradients
                    .get(&y.id())
                    .expect("y gradient should be synthesized as zeros")
            ),
            vec![0.0, 0.0]
        );

        Ok(())
    }

    #[test]
    fn public_forward_mode_jvp_exposes_value_and_tangent() -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let x = builder.input("x", Shape::new(vec![2]), DType::F32, true);
        let y = builder.input("y", Shape::new(vec![2]), DType::F32, true);
        let output = builder.mul(&x, &y)?;
        let graph = builder.finish(vec![output.clone()]);
        let inputs = BTreeMap::from([
            (x.id(), TensorData::F32(vec![2.0, 3.0])),
            (y.id(), TensorData::F32(vec![4.0, 5.0])),
        ]);
        let tangents = BTreeMap::from([
            (x.id(), TensorData::F32(vec![1.0, 1.0])),
            (y.id(), TensorData::F32(vec![2.0, 0.0])),
        ]);

        let transform = jvp(&graph, output.id(), &[x.id(), y.id()])?;
        let result = transform.apply(&inputs, &tangents)?;

        assert_eq!(dense_tensor(&result.value), vec![8.0, 15.0]);
        assert_eq!(dense_tensor(&result.tangent), vec![8.0, 5.0]);

        Ok(())
    }

    #[test]
    fn public_forward_mode_jvp_refuses_invalid_tangent_bindings() -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let tracked = builder.input("tracked", Shape::new(vec![1]), DType::F32, true);
        let other = builder.input("other", Shape::new(vec![1]), DType::F32, false);
        let graph = builder.finish(vec![tracked.clone()]);
        let inputs = BTreeMap::from([
            (tracked.id(), TensorData::F32(vec![2.0])),
            (other.id(), TensorData::F32(vec![9.0])),
        ]);

        assert_eq!(
            jvp(&graph, tracked.id(), &[other.id()]),
            Err(ForwardModeTransformError::UntrackedTarget {
                tensor_id: other.id(),
            })
        );

        let transform = jvp(&graph, tracked.id(), &[tracked.id()])?;
        assert_eq!(
            transform.apply(&inputs, &BTreeMap::new()),
            Err(ForwardModeTransformError::MissingTangentTarget {
                tensor_id: tracked.id(),
            })
        );
        assert_eq!(
            transform.apply(
                &inputs,
                &BTreeMap::from([(other.id(), TensorData::F32(vec![1.0]))])
            ),
            Err(ForwardModeTransformError::UnexpectedTangentTarget {
                tensor_id: other.id(),
            })
        );

        Ok(())
    }

    #[test]
    fn public_vmap_transform_batches_reference_graph_outputs() -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let x = builder.input("x", Shape::new(vec![2]), DType::F32, true);
        let bias = builder.input("bias", Shape::new(vec![2]), DType::F32, false);
        let squared = builder.mul(&x, &x)?;
        let output = builder.add(&squared, &bias)?;
        let graph = builder.finish(vec![output.clone()]);
        let inputs = BTreeMap::from([
            (x.id(), TensorData::F32(vec![1.0, 2.0, 3.0, 4.0])),
            (bias.id(), TensorData::F32(vec![10.0, 20.0])),
        ]);

        let transform = vmap(
            &graph,
            output.id(),
            &[VmapInputBinding {
                input: x.id(),
                axis: 0,
            }],
            1,
        )?;
        let result = transform.apply(&inputs)?;

        assert_eq!(dense_tensor(&result.value), vec![11.0, 19.0, 24.0, 36.0]);
        assert_eq!(result.lane_outputs.len(), 2);
        assert_eq!(dense_tensor(&result.lane_outputs[0]), vec![11.0, 24.0]);
        assert_eq!(dense_tensor(&result.lane_outputs[1]), vec![19.0, 36.0]);

        Ok(())
    }

    #[test]
    fn public_vmap_transform_refuses_unsupported_ops_and_bad_batch_inputs(
    ) -> Result<(), Box<dyn Error>> {
        let mut cast_builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let cast_input = cast_builder.input("x", Shape::new(vec![2]), DType::F32, true);
        let cast_output = cast_builder.cast(&cast_input, DType::I8)?;
        let cast_graph = cast_builder.finish(vec![cast_output.clone()]);
        assert_eq!(
            vmap(
                &cast_graph,
                cast_output.id(),
                &[VmapInputBinding {
                    input: cast_input.id(),
                    axis: 0,
                }],
                0,
            ),
            Err(VmapTransformError::UnsupportedOp {
                tensor_id: cast_output.id(),
                op: String::from("cast"),
            })
        );

        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let left = builder.input("left", Shape::new(vec![2]), DType::F32, true);
        let right = builder.input("right", Shape::new(vec![2]), DType::F32, true);
        let output = builder.add(&left, &right)?;
        let graph = builder.finish(vec![output.clone()]);

        assert_eq!(
            vmap(
                &graph,
                output.id(),
                &[
                    VmapInputBinding {
                        input: left.id(),
                        axis: 0,
                    },
                    VmapInputBinding {
                        input: left.id(),
                        axis: 1,
                    },
                ],
                0,
            ),
            Err(VmapTransformError::DuplicateMappedInput {
                tensor_id: left.id(),
            })
        );

        let transform = vmap(
            &graph,
            output.id(),
            &[
                VmapInputBinding {
                    input: left.id(),
                    axis: 0,
                },
                VmapInputBinding {
                    input: right.id(),
                    axis: 0,
                },
            ],
            0,
        )?;
        let bad_inputs = BTreeMap::from([
            (left.id(), TensorData::F32(vec![1.0, 2.0, 3.0, 4.0])),
            (right.id(), TensorData::F32(vec![5.0, 6.0])),
        ]);
        assert_eq!(
            transform.apply(&bad_inputs),
            Err(VmapTransformError::BatchSizeMismatch {
                tensor_id: right.id(),
                expected_batch_size: 2,
                actual_batch_size: 1,
            })
        );

        Ok(())
    }

    fn dense_gradient(result: &super::AutodiffBackwardResult, tensor_id: TensorId) -> Vec<f32> {
        let gradient = result
            .gradient(tensor_id)
            .expect("gradient should be present");
        dense_tensor(gradient)
    }

    fn scalar_loss(
        graph: &crate::Graph,
        output: TensorId,
        inputs: BTreeMap<TensorId, TensorData>,
    ) -> Result<f32, Box<dyn Error>> {
        let values = evaluate_graph(graph, &inputs)?;
        let TensorData::F32(output_values) = values
            .get(&output)
            .cloned()
            .expect("scalar output should be present")
        else {
            panic!("expected dense f32 scalar output");
        };
        Ok(*output_values
            .first()
            .expect("scalar output should have one value"))
    }

    fn dense_tensor(data: &TensorData) -> Vec<f32> {
        let TensorData::F32(values) = data else {
            panic!("expected dense f32 tensor");
        };
        values.clone()
    }

    fn assert_close_slice(actual: &[f32], expected: &[f32], tolerance: f32) {
        assert_eq!(actual.len(), expected.len(), "length mismatch");
        for (index, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
            let delta = (actual - expected).abs();
            assert!(
                delta <= tolerance,
                "value mismatch at index {index}: actual={actual} expected={expected} delta={delta} tolerance={tolerance}"
            );
        }
    }
}
