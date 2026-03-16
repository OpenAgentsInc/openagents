use std::collections::BTreeMap;

use psionic_core::{
    DType, Device, PsionicRefusal, PsionicRefusalCode, PsionicRefusalScope, Shape, Tensor,
    TensorData, TensorId,
};
use serde::{Deserialize, Serialize};
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
    /// Dtype-cast ops are not yet part of the bounded reverse-mode surface.
    CastFamily,
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
        | OpKind::Permute { .. }
        | OpKind::Slice { .. }
        | OpKind::Select { .. }
        | OpKind::Concat { .. }
        | OpKind::Expand { .. }
        | OpKind::ReduceSum { .. } => AutodiffGradientSupport::Implemented,
        OpKind::Cast { .. } => AutodiffGradientSupport::Unsupported {
            reason: AutodiffUnsupportedGradientReason::CastFamily,
        },
        OpKind::BackendExtension { .. } => AutodiffGradientSupport::Unsupported {
            reason: AutodiffUnsupportedGradientReason::BackendExtensionFamily,
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
    /// The current autodiff reference layer only supports dense `f32` gradients.
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
                OpKind::Input { .. }
                | OpKind::Constant { .. }
                | OpKind::Detach
                | OpKind::Cast { .. } => {}
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
                OpKind::BackendExtension { .. } => unreachable!(
                    "backend extensions should have been rejected by the autodiff support matrix"
                ),
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
        let output_node = self
            .graph
            .node(output)
            .ok_or(AutodiffError::UnknownTensor { tensor_id: output })?;
        let output_len = output_node.tensor().spec().shape().element_count();
        let seed = match seed {
            Some(seed) => seed,
            None if output_len == 1 => TensorData::F32(vec![1.0]),
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
                TensorData::F32(
                    input
                        .iter()
                        .map(|current| match dtype {
                            DType::F32 | DType::F16 | DType::BF16 => *current,
                            DType::I8 => current.round().clamp(i8::MIN as f32, i8::MAX as f32),
                        })
                        .collect(),
                )
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
            OpKind::BackendExtension { .. } => {
                return Err(ReferenceEvaluationError::UnsupportedOp {
                    tensor_id: node.tensor().id(),
                    op: String::from(node.op().label()),
                });
            }
        };
        validate_output_length(node.tensor(), &value)?;
        values.insert(node.tensor().id(), value);
    }
    Ok(values)
}

fn ensure_supported_gradient_dtype(tensor: &Tensor) -> Result<(), AutodiffError> {
    if tensor.spec().dtype() == DType::F32 {
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
    if tensor.spec().dtype() != DType::F32 {
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

    use std::{collections::BTreeMap, error::Error};

    use psionic_core::{DType, Device, PsionicRefusalCode, PsionicRefusalScope, Shape, TensorData};

    use crate::{
        gradient_support_for_op, AutodiffContext, AutodiffError, AutodiffGradientSupport,
        AutodiffGraphBuilder, AutodiffUnsupportedGradientReason, TensorId,
    };

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
    fn unsupported_gradient_ops_refuse_through_typed_error() -> Result<(), Box<dyn Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![1, 2]), DType::F32, true);
        let weight = builder.input("weight", Shape::new(vec![2]), DType::F32, true);
        let normalized = builder.rms_norm(&input, &weight, 1e-5)?;
        let loss = builder.reduce_sum(&normalized);
        let graph = builder.finish(vec![loss.clone()]);

        assert_eq!(
            graph.backward_plan(loss.id()),
            Err(AutodiffError::UnsupportedGradientOp {
                tensor_id: normalized.id(),
                op: String::from("rms_norm"),
            })
        );
        Ok(())
    }

    #[test]
    fn autodiff_refusal_taxonomy_maps_unsupported_gradient_family() {
        let refusal = AutodiffError::UnsupportedGradientOp {
            tensor_id: TensorId(7),
            op: String::from("rms_norm"),
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
    fn autodiff_support_matrix_marks_primitives_and_backend_extensions_explicitly() {
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
        let reduced = builder.reduce_sum_axis(&permuted, 0).expect("axis reduce");
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
            AutodiffGradientSupport::Unsupported {
                reason: AutodiffUnsupportedGradientReason::BackendExtensionFamily
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
        let cos = rope_builder.constant_f32(Shape::new(vec![2, 2]), vec![1.0; 4])?;
        let sin = rope_builder.constant_f32(Shape::new(vec![2, 2]), vec![0.0; 4])?;
        let roped = rope_builder.rope(&rope_input, &cos, &sin, false)?;
        let rope_loss = rope_builder.reduce_sum(&roped);
        let rope_graph = rope_builder.finish(vec![rope_loss.clone()]);
        assert_eq!(
            rope_graph.backward_plan(rope_loss.id()),
            Err(AutodiffError::UnsupportedGradientOp {
                tensor_id: roped.id(),
                op: String::from("rotary_embedding"),
            })
        );

        let mut attention_builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let query = attention_builder.input("q", Shape::new(vec![1, 1, 2, 4]), DType::F32, true);
        let key = attention_builder.input("k", Shape::new(vec![1, 1, 2, 4]), DType::F32, true);
        let value = attention_builder.input("v", Shape::new(vec![1, 1, 2, 4]), DType::F32, true);
        let attended =
            attention_builder.scaled_dot_product_attention(&query, &key, &value, 0.5, true)?;
        let attention_loss = attention_builder.reduce_sum(&attended);
        let attention_graph = attention_builder.finish(vec![attention_loss.clone()]);
        assert_eq!(
            attention_graph.backward_plan(attention_loss.id()),
            Err(AutodiffError::UnsupportedGradientOp {
                tensor_id: attended.id(),
                op: String::from("scaled_dot_product_attention"),
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

    fn dense_gradient(result: &super::AutodiffBackwardResult, tensor_id: TensorId) -> Vec<f32> {
        let gradient = result
            .gradient(tensor_id)
            .expect("gradient should be present");
        let TensorData::F32(values) = gradient else {
            panic!("expected dense f32 gradient");
        };
        values.clone()
    }
}
