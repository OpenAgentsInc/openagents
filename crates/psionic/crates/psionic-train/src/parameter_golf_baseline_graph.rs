use std::collections::BTreeMap;

use psionic_core::{DType, Device, DeviceKind, Shape, TensorData, TensorId};
use psionic_ir::{
    AutodiffContext, AutodiffError, AutodiffGraph, AutodiffGraphBuilder, AutodiffTensor, Graph,
    GraphError, ReferenceEvaluationError,
};
use psionic_models::{
    ParameterGolfBankedWeights, ParameterGolfConfig, ParameterGolfConfigError,
    ParameterGolfExecutionError, ParameterGolfMlpActivation, ParameterGolfModelDescriptor,
    ParameterGolfModelError, ParameterGolfReferenceModel, ParameterGolfTensor3,
    ParameterGolfTensorError, PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON, PARAMETER_GOLF_KV_BANK_NAME,
    PARAMETER_GOLF_MATRIX_BANK_NAMES, PARAMETER_GOLF_MLP_DOWN_BANK_NAME,
    PARAMETER_GOLF_MLP_UP_BANK_NAME, PARAMETER_GOLF_QO_BANK_NAME,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::parameter_golf_graph_parameter_dtype;

/// How one Parameter Golf graph parameter receives gradients on the current
/// Rust-owned baseline lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfBaselineGradientSource {
    /// The parameter is differentiated directly by the lowered graph.
    GraphOnly,
}

/// Stable parameter binding for the lowered Parameter Golf baseline graph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfBaselineGraphParameterBinding {
    /// Stable Parameter Golf parameter identifier.
    pub parameter_id: String,
    /// Logical tensor shape.
    pub shape: Shape,
    /// Graph input dtype carrying the parameter value.
    pub graph_input_dtype: DType,
    /// Graph input tensor carrying the parameter value.
    pub graph_input_tensor_id: TensorId,
    /// Honest gradient-source posture for the parameter.
    pub gradient_source: ParameterGolfBaselineGradientSource,
}

/// Explicit PGOLF matrix-execution posture for the banked runtime surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfMatrixExecutionMode {
    /// Lower rank-3 banks through one direct bank-aware backend op.
    DirectBanked,
    /// Slice one logical matrix back out of the rank-3 bank and run the older
    /// split-matrix hot path.
    SplitSliced,
}

impl ParameterGolfMatrixExecutionMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectBanked => "direct_banked",
            Self::SplitSliced => "split_sliced",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "direct_banked" => Ok(Self::DirectBanked),
            "split_sliced" => Ok(Self::SplitSliced),
            other => Err(format!(
                "unsupported parameter golf matrix execution mode `{other}`; expected `direct_banked` or `split_sliced`"
            )),
        }
    }

    #[must_use]
    pub const fn uses_direct_banked_execution(self) -> bool {
        matches!(self, Self::DirectBanked)
    }
}

fn default_parameter_golf_matrix_execution_mode() -> ParameterGolfMatrixExecutionMode {
    ParameterGolfMatrixExecutionMode::DirectBanked
}

/// Lowered Parameter Golf baseline graph plus machine-readable bindings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfBaselineGraph {
    /// The lowered autodiff graph.
    pub graph: AutodiffGraph,
    /// Non-trainable integer token-id tensor consumed by the graph.
    pub input_token_ids_tensor_id: TensorId,
    /// Optional non-trainable hashed-bigram token ids consumed by the graph.
    pub bigram_token_ids_tensor_id: Option<TensorId>,
    /// Final pre-softcap logits tensor emitted by the graph.
    pub pre_softcap_logits_tensor_id: TensorId,
    /// Parameter bindings in deterministic order.
    pub parameter_bindings: Vec<ParameterGolfBaselineGraphParameterBinding>,
}

impl ParameterGolfBaselineGraph {
    /// Looks up one parameter binding by stable parameter id.
    #[must_use]
    pub fn parameter_binding(
        &self,
        parameter_id: &str,
    ) -> Option<&ParameterGolfBaselineGraphParameterBinding> {
        self.parameter_bindings
            .iter()
            .find(|binding| binding.parameter_id == parameter_id)
    }
}

/// Lowered Parameter Golf baseline training graph that applies the bounded
/// projection loss directly on-device.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfBaselineTrainingGraph {
    /// The lowered autodiff graph.
    pub graph: AutodiffGraph,
    /// Non-trainable integer token-id tensor consumed by the graph.
    pub input_token_ids_tensor_id: TensorId,
    /// Optional non-trainable hashed-bigram token ids consumed by the graph.
    pub bigram_token_ids_tensor_id: Option<TensorId>,
    /// Integer target ids consumed by the projection-loss op.
    pub target_ids_tensor_id: TensorId,
    /// Final scalar mean-loss tensor emitted by the graph.
    pub loss_tensor_id: TensorId,
    /// Parameter bindings in deterministic order.
    pub parameter_bindings: Vec<ParameterGolfBaselineGraphParameterBinding>,
}

impl ParameterGolfBaselineTrainingGraph {
    /// Looks up one parameter binding by stable parameter id.
    #[must_use]
    pub fn parameter_binding(
        &self,
        parameter_id: &str,
    ) -> Option<&ParameterGolfBaselineGraphParameterBinding> {
        self.parameter_bindings
            .iter()
            .find(|binding| binding.parameter_id == parameter_id)
    }
}

/// Lowered Parameter Golf baseline eval graph that applies the bounded
/// projection loss on the eval surface without retaining the training-graph
/// machinery.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfBaselineEvalGraph {
    /// The lowered eval graph.
    pub graph: Graph,
    /// Non-trainable integer token-id tensor consumed by the graph.
    pub input_token_ids_tensor_id: TensorId,
    /// Optional non-trainable hashed-bigram token ids consumed by the graph.
    pub bigram_token_ids_tensor_id: Option<TensorId>,
    /// Integer target ids consumed by the projection-loss op.
    pub target_ids_tensor_id: TensorId,
    /// Per-token loss tensor emitted by the eval graph.
    pub token_losses_tensor_id: TensorId,
    /// Final scalar mean-loss tensor emitted by the graph.
    pub loss_tensor_id: TensorId,
    /// Parameter bindings in deterministic order.
    pub parameter_bindings: Vec<ParameterGolfBaselineGraphParameterBinding>,
}

impl ParameterGolfBaselineEvalGraph {
    /// Looks up one parameter binding by stable parameter id.
    #[must_use]
    pub fn parameter_binding(
        &self,
        parameter_id: &str,
    ) -> Option<&ParameterGolfBaselineGraphParameterBinding> {
        self.parameter_bindings
            .iter()
            .find(|binding| binding.parameter_id == parameter_id)
    }
}

/// Host-owned logits post-processing and seed materialized from one pre-softcap
/// graph output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfProjectionSeed {
    /// Softcapped logits matching the public reference model.
    pub softcapped_logits: ParameterGolfTensor3,
    /// Mean cross-entropy over the supplied targets.
    pub mean_loss: f32,
    /// Seed gradient with respect to the graph's pre-softcap logits.
    pub pre_softcap_gradient: ParameterGolfTensor3,
}

/// Parameter-gradient materialization for one bounded Parameter Golf baseline
/// graph replay.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfBaselineGradientBundle {
    /// Stable parameter gradients keyed by Parameter Golf tensor id.
    pub parameter_gradients: BTreeMap<String, Vec<f32>>,
}

/// Error returned while lowering or replaying the Parameter Golf baseline graph.
#[derive(Debug, Error)]
pub enum ParameterGolfBaselineGraphError {
    #[error(transparent)]
    Graph(#[from] GraphError),
    #[error(transparent)]
    Autodiff(#[from] AutodiffError),
    #[error(transparent)]
    ReferenceEvaluation(#[from] ReferenceEvaluationError),
    #[error(transparent)]
    Config(#[from] ParameterGolfConfigError),
    #[error(transparent)]
    Train(#[from] crate::ParameterGolfTrainError),
    #[error(transparent)]
    Execution(#[from] ParameterGolfExecutionError),
    #[error(transparent)]
    Model(#[from] ParameterGolfModelError),
    #[error(transparent)]
    Tensor(#[from] ParameterGolfTensorError),
    #[error(
        "parameter golf baseline graph expected tied embeddings or an untied lm_head; descriptor `{model_id}` had tie_embeddings={tie_embeddings} and lm_head_present={lm_head_present}"
    )]
    InvalidHeadPosture {
        model_id: String,
        tie_embeddings: bool,
        lm_head_present: bool,
    },
    #[error("parameter golf baseline graph is missing parameter binding for `{parameter_id}`")]
    MissingParameterBinding { parameter_id: String },
    #[error("parameter golf baseline graph is missing weight vector for `{parameter_id}`")]
    MissingWeightVector { parameter_id: String },
    #[error("parameter golf baseline graph is missing gradient for `{parameter_id}`")]
    MissingGradient { parameter_id: String },
    #[error("parameter golf baseline graph is missing forward logits for tensor `{tensor_id}`")]
    MissingForwardLogits { tensor_id: TensorId },
    #[error(
        "parameter golf baseline graph expected dense floating-point tensor data for {context}"
    )]
    NonDenseTensorData { context: String },
}

struct ParameterGolfBaselineGraphBuildState {
    input_token_ids: AutodiffTensor,
    bigram_token_ids: Option<AutodiffTensor>,
    parameter_bindings: Vec<ParameterGolfBaselineGraphParameterBinding>,
    pre_softcap_logits: AutodiffTensor,
}

/// Builds one trainer-owned Parameter Golf baseline graph that emits pre-softcap
/// logits from integer token ids plus the explicit baseline parameter surface.
pub fn build_parameter_golf_baseline_graph(
    device: Device,
    descriptor: &ParameterGolfModelDescriptor,
    batch_size: usize,
    sequence_length: usize,
) -> Result<ParameterGolfBaselineGraph, ParameterGolfBaselineGraphError> {
    build_parameter_golf_baseline_graph_with_matrix_execution_mode(
        device,
        descriptor,
        batch_size,
        sequence_length,
        default_parameter_golf_matrix_execution_mode(),
    )
}

pub fn build_parameter_golf_baseline_graph_with_matrix_execution_mode(
    device: Device,
    descriptor: &ParameterGolfModelDescriptor,
    batch_size: usize,
    sequence_length: usize,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
) -> Result<ParameterGolfBaselineGraph, ParameterGolfBaselineGraphError> {
    let mut builder = AutodiffGraphBuilder::with_context(device, AutodiffContext::training());
    let state = build_baseline_graph_state(
        &mut builder,
        descriptor,
        batch_size,
        sequence_length,
        true,
        false,
        matrix_execution_mode,
    )?;
    let graph = builder.finish(vec![state.pre_softcap_logits.clone()]);

    Ok(ParameterGolfBaselineGraph {
        input_token_ids_tensor_id: state.input_token_ids.id(),
        bigram_token_ids_tensor_id: state.bigram_token_ids.as_ref().map(AutodiffTensor::id),
        pre_softcap_logits_tensor_id: state.pre_softcap_logits.id(),
        graph,
        parameter_bindings: state.parameter_bindings,
    })
}

/// Builds one trainer-owned Parameter Golf baseline graph that emits the
/// bounded on-device projection loss directly from integer token ids, target
/// ids, and the explicit baseline parameter surface.
pub fn build_parameter_golf_baseline_training_graph(
    device: Device,
    descriptor: &ParameterGolfModelDescriptor,
    batch_size: usize,
    sequence_length: usize,
) -> Result<ParameterGolfBaselineTrainingGraph, ParameterGolfBaselineGraphError> {
    build_parameter_golf_baseline_training_graph_with_matrix_execution_mode(
        device,
        descriptor,
        batch_size,
        sequence_length,
        default_parameter_golf_matrix_execution_mode(),
    )
}

pub fn build_parameter_golf_baseline_training_graph_with_matrix_execution_mode(
    device: Device,
    descriptor: &ParameterGolfModelDescriptor,
    batch_size: usize,
    sequence_length: usize,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
) -> Result<ParameterGolfBaselineTrainingGraph, ParameterGolfBaselineGraphError> {
    let use_bf16_fast_path = device.kind() == DeviceKind::Cuda;
    let mut builder = AutodiffGraphBuilder::with_context(device, AutodiffContext::training());
    let state = build_baseline_graph_state(
        &mut builder,
        descriptor,
        batch_size,
        sequence_length,
        true,
        use_bf16_fast_path,
        matrix_execution_mode,
    )?;
    let target_ids = builder.input(
        "target_ids",
        Shape::new(vec![batch_size, sequence_length]),
        DType::I32,
        false,
    );
    let loss = builder.parameter_golf_projection_loss(
        &state.pre_softcap_logits,
        &target_ids,
        descriptor.config.logit_softcap,
    )?;
    let graph = builder.finish(vec![loss.clone()]);

    Ok(ParameterGolfBaselineTrainingGraph {
        input_token_ids_tensor_id: state.input_token_ids.id(),
        bigram_token_ids_tensor_id: state.bigram_token_ids.as_ref().map(AutodiffTensor::id),
        target_ids_tensor_id: target_ids.id(),
        loss_tensor_id: loss.id(),
        graph,
        parameter_bindings: state.parameter_bindings,
    })
}

/// Builds one eval-specific Parameter Golf baseline graph that emits the
/// bounded on-device projection loss from integer token ids, target ids, and
/// the explicit baseline parameter surface without routing through the
/// training-graph surface.
pub fn build_parameter_golf_baseline_eval_graph(
    device: Device,
    descriptor: &ParameterGolfModelDescriptor,
    batch_size: usize,
    sequence_length: usize,
) -> Result<ParameterGolfBaselineEvalGraph, ParameterGolfBaselineGraphError> {
    build_parameter_golf_baseline_eval_graph_with_matrix_execution_mode(
        device,
        descriptor,
        batch_size,
        sequence_length,
        default_parameter_golf_matrix_execution_mode(),
    )
}

pub fn build_parameter_golf_baseline_eval_graph_with_matrix_execution_mode(
    device: Device,
    descriptor: &ParameterGolfModelDescriptor,
    batch_size: usize,
    sequence_length: usize,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
) -> Result<ParameterGolfBaselineEvalGraph, ParameterGolfBaselineGraphError> {
    let use_bf16_fast_path = device.kind() == DeviceKind::Cuda;
    let mut builder = AutodiffGraphBuilder::with_context(device, AutodiffContext::evaluation());
    let state = build_baseline_graph_state(
        &mut builder,
        descriptor,
        batch_size,
        sequence_length,
        false,
        use_bf16_fast_path,
        matrix_execution_mode,
    )?;
    let target_ids = builder.input(
        "target_ids",
        Shape::new(vec![batch_size, sequence_length]),
        DType::I32,
        false,
    );
    let token_losses = builder.parameter_golf_projection_token_losses(
        &state.pre_softcap_logits,
        &target_ids,
        descriptor.config.logit_softcap,
    )?;
    let token_loss_sum = builder.reduce_sum(&token_losses);
    let loss_scale = builder.constant_f32(
        Shape::scalar(),
        vec![1.0_f32 / (batch_size.saturating_mul(sequence_length).max(1) as f32)],
    )?;
    let loss = builder.mul(&token_loss_sum, &loss_scale)?;
    let graph = builder
        .finish(vec![token_losses.clone(), loss.clone()])
        .graph()
        .clone();

    Ok(ParameterGolfBaselineEvalGraph {
        graph,
        input_token_ids_tensor_id: state.input_token_ids.id(),
        bigram_token_ids_tensor_id: state.bigram_token_ids.as_ref().map(AutodiffTensor::id),
        target_ids_tensor_id: target_ids.id(),
        token_losses_tensor_id: token_losses.id(),
        loss_tensor_id: loss.id(),
        parameter_bindings: state.parameter_bindings,
    })
}

/// Builds one token-id plus parameter input map for the lowered Parameter Golf
/// baseline graph.
pub fn bind_parameter_golf_baseline_graph_inputs(
    graph: &ParameterGolfBaselineGraph,
    model: &ParameterGolfReferenceModel,
    input_ids: &[Vec<u32>],
) -> Result<BTreeMap<TensorId, TensorData>, ParameterGolfBaselineGraphError> {
    bind_parameter_golf_baseline_graph_inputs_with_banked_weights(graph, model, None, input_ids)
}

/// Builds one token-id plus parameter input map for the lowered Parameter Golf
/// baseline graph while reusing one explicit banked runtime surface when the
/// caller already owns it.
pub fn bind_parameter_golf_baseline_graph_inputs_with_banked_weights(
    graph: &ParameterGolfBaselineGraph,
    model: &ParameterGolfReferenceModel,
    explicit_banked_weights: Option<&ParameterGolfBankedWeights>,
    input_ids: &[Vec<u32>],
) -> Result<BTreeMap<TensorId, TensorData>, ParameterGolfBaselineGraphError> {
    bind_parameter_golf_graph_inputs(
        graph.input_token_ids_tensor_id,
        graph.bigram_token_ids_tensor_id,
        graph.parameter_bindings.as_slice(),
        model,
        explicit_banked_weights,
        input_ids,
    )
}

/// Builds one token-id plus parameter input map for the lowered Parameter Golf
/// baseline training graph.
pub fn bind_parameter_golf_baseline_training_graph_inputs(
    graph: &ParameterGolfBaselineTrainingGraph,
    model: &ParameterGolfReferenceModel,
    input_ids: &[Vec<u32>],
    target_ids: &[Vec<u32>],
) -> Result<BTreeMap<TensorId, TensorData>, ParameterGolfBaselineGraphError> {
    bind_parameter_golf_baseline_training_graph_inputs_with_banked_weights(
        graph, model, None, input_ids, target_ids,
    )
}

/// Builds one token-id plus parameter input map for the lowered Parameter Golf
/// baseline training graph while reusing one explicit banked runtime surface
/// when the caller already owns it.
pub fn bind_parameter_golf_baseline_training_graph_inputs_with_banked_weights(
    graph: &ParameterGolfBaselineTrainingGraph,
    model: &ParameterGolfReferenceModel,
    explicit_banked_weights: Option<&ParameterGolfBankedWeights>,
    input_ids: &[Vec<u32>],
    target_ids: &[Vec<u32>],
) -> Result<BTreeMap<TensorId, TensorData>, ParameterGolfBaselineGraphError> {
    bind_parameter_golf_loss_graph_inputs(
        graph.input_token_ids_tensor_id,
        graph.bigram_token_ids_tensor_id,
        graph.target_ids_tensor_id,
        graph.parameter_bindings.as_slice(),
        model,
        explicit_banked_weights,
        input_ids,
        target_ids,
    )
}

/// Builds one token-id plus parameter input map for the lowered Parameter Golf
/// baseline eval graph.
pub fn bind_parameter_golf_baseline_eval_graph_inputs(
    graph: &ParameterGolfBaselineEvalGraph,
    model: &ParameterGolfReferenceModel,
    input_ids: &[Vec<u32>],
    target_ids: &[Vec<u32>],
) -> Result<BTreeMap<TensorId, TensorData>, ParameterGolfBaselineGraphError> {
    bind_parameter_golf_baseline_eval_graph_inputs_with_banked_weights(
        graph, model, None, input_ids, target_ids,
    )
}

/// Builds one token-id plus parameter input map for the lowered Parameter Golf
/// baseline eval graph while reusing one explicit banked runtime surface when
/// the caller already owns it.
pub fn bind_parameter_golf_baseline_eval_graph_inputs_with_banked_weights(
    graph: &ParameterGolfBaselineEvalGraph,
    model: &ParameterGolfReferenceModel,
    explicit_banked_weights: Option<&ParameterGolfBankedWeights>,
    input_ids: &[Vec<u32>],
    target_ids: &[Vec<u32>],
) -> Result<BTreeMap<TensorId, TensorData>, ParameterGolfBaselineGraphError> {
    bind_parameter_golf_loss_graph_inputs(
        graph.input_token_ids_tensor_id,
        graph.bigram_token_ids_tensor_id,
        graph.target_ids_tensor_id,
        graph.parameter_bindings.as_slice(),
        model,
        explicit_banked_weights,
        input_ids,
        target_ids,
    )
}

fn bind_parameter_golf_graph_inputs(
    input_token_ids_tensor_id: TensorId,
    bigram_token_ids_tensor_id: Option<TensorId>,
    parameter_bindings: &[ParameterGolfBaselineGraphParameterBinding],
    model: &ParameterGolfReferenceModel,
    explicit_banked_weights: Option<&ParameterGolfBankedWeights>,
    input_ids: &[Vec<u32>],
) -> Result<BTreeMap<TensorId, TensorData>, ParameterGolfBaselineGraphError> {
    let config = &model.descriptor().config;
    validate_token_batch(input_ids, config.vocab_size)?;

    let parameter_vectors = parameter_golf_parameter_values_for_bindings(
        parameter_bindings,
        model,
        explicit_banked_weights,
    )?;
    let mut inputs = BTreeMap::new();
    inputs.insert(
        input_token_ids_tensor_id,
        TensorData::I32(
            input_ids
                .iter()
                .flat_map(|row| row.iter().map(|token_id| *token_id as i32))
                .collect(),
        ),
    );
    if let Some(bigram_token_ids_tensor_id) = bigram_token_ids_tensor_id {
        let bigram_ids = config
            .bigram_hash_batch(input_ids)?
            .expect("bigram graph input must not exist when the feature is disabled");
        inputs.insert(
            bigram_token_ids_tensor_id,
            TensorData::I32(
                bigram_ids
                    .iter()
                    .flat_map(|row| row.iter().map(|token_id| *token_id as i32))
                    .collect(),
            ),
        );
    }
    for binding in parameter_bindings {
        let parameter = parameter_vectors
            .get(&binding.parameter_id)
            .ok_or_else(|| ParameterGolfBaselineGraphError::MissingWeightVector {
                parameter_id: binding.parameter_id.clone(),
            })?;
        let parameter_data = match binding.graph_input_dtype {
            DType::F32 => TensorData::F32(parameter.clone()),
            DType::BF16 => TensorData::BF16(parameter.clone()),
            actual => {
                return Err(ParameterGolfBaselineGraphError::Graph(
                    GraphError::InvalidOperatorInputs {
                        op: String::from("parameter_golf_baseline_graph_input_binding"),
                        message: format!(
                            "parameter golf baseline graph does not support graph input dtype {actual:?} for `{}`",
                            binding.parameter_id
                        ),
                    },
                ))
            }
        };
        inputs.insert(binding.graph_input_tensor_id, parameter_data);
    }
    Ok(inputs)
}

fn bind_parameter_golf_loss_graph_inputs(
    input_token_ids_tensor_id: TensorId,
    bigram_token_ids_tensor_id: Option<TensorId>,
    target_ids_tensor_id: TensorId,
    parameter_bindings: &[ParameterGolfBaselineGraphParameterBinding],
    model: &ParameterGolfReferenceModel,
    explicit_banked_weights: Option<&ParameterGolfBankedWeights>,
    input_ids: &[Vec<u32>],
    target_ids: &[Vec<u32>],
) -> Result<BTreeMap<TensorId, TensorData>, ParameterGolfBaselineGraphError> {
    let mut inputs = bind_parameter_golf_graph_inputs(
        input_token_ids_tensor_id,
        bigram_token_ids_tensor_id,
        parameter_bindings,
        model,
        explicit_banked_weights,
        input_ids,
    )?;
    let (batch_size, sequence_length) =
        validate_token_batch(input_ids, model.descriptor().config.vocab_size)?;
    validate_target_shape(target_ids, batch_size, sequence_length)?;
    validate_token_batch(target_ids, model.descriptor().config.vocab_size)?;
    inputs.insert(
        target_ids_tensor_id,
        TensorData::I32(
            target_ids
                .iter()
                .flat_map(|row| row.iter().map(|token_id| *token_id as i32))
                .collect(),
        ),
    );
    Ok(inputs)
}

pub(crate) fn parameter_golf_parameter_values_for_bindings(
    parameter_bindings: &[ParameterGolfBaselineGraphParameterBinding],
    model: &ParameterGolfReferenceModel,
    explicit_banked_weights: Option<&ParameterGolfBankedWeights>,
) -> Result<BTreeMap<String, Vec<f32>>, ParameterGolfBaselineGraphError> {
    let config = &model.descriptor().config;
    let split_parameter_vectors = model
        .weights()
        .parameter_vectors(config)
        .into_iter()
        .map(|parameter| (parameter.parameter_id, parameter.values))
        .collect::<BTreeMap<_, _>>();
    let needs_banked_runtime_surface = parameter_bindings
        .iter()
        .any(|binding| PARAMETER_GOLF_MATRIX_BANK_NAMES.contains(&binding.parameter_id.as_str()));
    let owned_banked_weights = if explicit_banked_weights.is_none() && needs_banked_runtime_surface
    {
        Some(model.banked_weights()?)
    } else {
        None
    };
    let banked_weights = explicit_banked_weights.or(owned_banked_weights.as_ref());
    let banked_parameter_vectors = if let Some(banked_weights) = banked_weights {
        banked_weights
            .parameter_vectors(config)
            .into_iter()
            .map(|parameter| (parameter.parameter_id, parameter.values))
            .collect::<BTreeMap<_, _>>()
    } else {
        BTreeMap::new()
    };
    let mut parameter_vectors = BTreeMap::new();
    for binding in parameter_bindings {
        let values = if PARAMETER_GOLF_MATRIX_BANK_NAMES.contains(&binding.parameter_id.as_str()) {
            banked_parameter_vectors
                .get(&binding.parameter_id)
                .ok_or_else(|| ParameterGolfBaselineGraphError::MissingWeightVector {
                    parameter_id: binding.parameter_id.clone(),
                })?
        } else {
            split_parameter_vectors
                .get(&binding.parameter_id)
                .ok_or_else(|| ParameterGolfBaselineGraphError::MissingWeightVector {
                    parameter_id: binding.parameter_id.clone(),
                })?
        };
        parameter_vectors.insert(binding.parameter_id.clone(), values.clone());
    }
    Ok(parameter_vectors)
}

/// Applies the host-owned logit softcap and cross-entropy gradient seed for one
/// pre-softcap logits tensor.
pub fn parameter_golf_projection_seed(
    pre_softcap_logits: &ParameterGolfTensor3,
    target_ids: &[Vec<u32>],
    logit_softcap: f32,
) -> Result<ParameterGolfProjectionSeed, ParameterGolfBaselineGraphError> {
    if !logit_softcap.is_finite() || logit_softcap <= 0.0 {
        return Err(ParameterGolfExecutionError::InvalidAttentionWindowSize {
            attention_window_size: 0,
        }
        .into());
    }
    validate_target_shape(
        target_ids,
        pre_softcap_logits.batch_size(),
        pre_softcap_logits.sequence_length(),
    )?;
    validate_token_batch(target_ids, pre_softcap_logits.width())?;

    let mut softcapped = vec![0.0_f32; pre_softcap_logits.values().len()];
    let mut seed = vec![0.0_f32; pre_softcap_logits.values().len()];
    let mut total_loss = 0.0_f32;
    let position_count =
        (pre_softcap_logits.batch_size() * pre_softcap_logits.sequence_length()) as f32;

    for batch in 0..pre_softcap_logits.batch_size() {
        for position in 0..pre_softcap_logits.sequence_length() {
            let offset = (batch * pre_softcap_logits.sequence_length() + position)
                * pre_softcap_logits.width();
            let logits_row =
                &pre_softcap_logits.values()[offset..offset + pre_softcap_logits.width()];
            let softcapped_row = &mut softcapped[offset..offset + pre_softcap_logits.width()];
            let seed_row = &mut seed[offset..offset + pre_softcap_logits.width()];
            for (destination, source) in softcapped_row.iter_mut().zip(logits_row.iter()) {
                *destination = logit_softcap * (*source / logit_softcap).tanh();
            }
            let max_logit = softcapped_row
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            let mut probabilities = vec![0.0_f32; softcapped_row.len()];
            let mut exp_sum = 0.0_f32;
            for (index, value) in softcapped_row.iter().enumerate() {
                let exp = (*value - max_logit).exp();
                probabilities[index] = exp;
                exp_sum += exp;
            }
            let exp_sum = exp_sum.max(f32::EPSILON);
            let target = target_ids[batch][position] as usize;
            total_loss += max_logit + exp_sum.ln() - softcapped_row[target];
            for index in 0..probabilities.len() {
                probabilities[index] /= exp_sum;
                let delta = if index == target { 1.0 } else { 0.0 };
                let softcap_derivative = 1.0 - (softcapped_row[index] / logit_softcap).powi(2);
                seed_row[index] =
                    ((probabilities[index] - delta) / position_count) * softcap_derivative;
            }
        }
    }

    Ok(ParameterGolfProjectionSeed {
        softcapped_logits: ParameterGolfTensor3::new(pre_softcap_logits.shape(), softcapped)?,
        mean_loss: total_loss / position_count,
        pre_softcap_gradient: ParameterGolfTensor3::new(pre_softcap_logits.shape(), seed)?,
    })
}

/// Reconstructs stable Parameter Golf parameter gradients from one seeded
/// backward replay.
pub fn materialize_parameter_golf_baseline_gradients(
    graph: &ParameterGolfBaselineGraph,
    backward: &psionic_ir::AutodiffBackwardResult,
    config: &ParameterGolfConfig,
    input_ids: &[Vec<u32>],
) -> Result<ParameterGolfBaselineGradientBundle, ParameterGolfBaselineGraphError> {
    materialize_parameter_golf_gradients_from_bindings(
        graph.parameter_bindings.as_slice(),
        backward,
        config,
        input_ids,
    )
}

/// Reconstructs stable Parameter Golf parameter gradients from one seeded
/// backward replay over the lowered training graph.
pub fn materialize_parameter_golf_baseline_training_gradients(
    graph: &ParameterGolfBaselineTrainingGraph,
    backward: &psionic_ir::AutodiffBackwardResult,
    config: &ParameterGolfConfig,
    input_ids: &[Vec<u32>],
) -> Result<ParameterGolfBaselineGradientBundle, ParameterGolfBaselineGraphError> {
    materialize_parameter_golf_gradients_from_bindings(
        graph.parameter_bindings.as_slice(),
        backward,
        config,
        input_ids,
    )
}

fn materialize_parameter_golf_gradients_from_bindings(
    parameter_bindings: &[ParameterGolfBaselineGraphParameterBinding],
    backward: &psionic_ir::AutodiffBackwardResult,
    _config: &ParameterGolfConfig,
    _input_ids: &[Vec<u32>],
) -> Result<ParameterGolfBaselineGradientBundle, ParameterGolfBaselineGraphError> {
    let mut gradients = BTreeMap::new();
    for binding in parameter_bindings {
        let graph_gradient = backward.gradient(binding.graph_input_tensor_id);
        let values = match binding.gradient_source {
            ParameterGolfBaselineGradientSource::GraphOnly => dense_gradient_values(
                graph_gradient.ok_or_else(|| ParameterGolfBaselineGraphError::MissingGradient {
                    parameter_id: binding.parameter_id.clone(),
                })?,
                binding.parameter_id.clone(),
            )?,
        };
        gradients.insert(binding.parameter_id.clone(), values);
    }
    Ok(ParameterGolfBaselineGradientBundle {
        parameter_gradients: gradients,
    })
}

fn build_baseline_graph_state(
    builder: &mut AutodiffGraphBuilder,
    descriptor: &ParameterGolfModelDescriptor,
    batch_size: usize,
    sequence_length: usize,
    parameter_requires_grad: bool,
    use_bf16_fast_path: bool,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
) -> Result<ParameterGolfBaselineGraphBuildState, ParameterGolfBaselineGraphError> {
    let config = &descriptor.config;
    if batch_size == 0 {
        return Err(ParameterGolfExecutionError::EmptyBatch.into());
    }
    if sequence_length == 0 {
        return Err(ParameterGolfExecutionError::EmptySequence.into());
    }
    if !config.tie_embeddings
        && !descriptor
            .weights
            .tensors
            .iter()
            .any(|tensor| tensor.name == "lm_head.weight")
    {
        return Err(ParameterGolfBaselineGraphError::InvalidHeadPosture {
            model_id: descriptor.model.model_id.clone(),
            tie_embeddings: config.tie_embeddings,
            lm_head_present: false,
        });
    }

    let input_token_ids = builder.input(
        "input_token_ids",
        Shape::new(vec![batch_size, sequence_length]),
        DType::I32,
        false,
    );
    let bigram_token_ids = if config.bigram_vocab_size > 0 {
        Some(builder.input(
            "bigram_token_ids",
            Shape::new(vec![batch_size, sequence_length]),
            DType::I32,
            false,
        ))
    } else {
        None
    };

    let mut parameter_inputs = BTreeMap::new();
    let mut parameter_bindings = Vec::new();
    for tensor in &descriptor.weights.tensors {
        let graph_input_dtype = parameter_golf_graph_parameter_dtype(&tensor.name, &tensor.shape)?;
        let tensor_input = builder.input(
            tensor.name.clone(),
            tensor.shape.clone(),
            graph_input_dtype,
            parameter_requires_grad,
        );
        parameter_bindings.push(ParameterGolfBaselineGraphParameterBinding {
            parameter_id: tensor.name.clone(),
            shape: tensor.shape.clone(),
            graph_input_dtype,
            graph_input_tensor_id: tensor_input.id(),
            gradient_source: ParameterGolfBaselineGradientSource::GraphOnly,
        });
        parameter_inputs.insert(tensor.name.clone(), tensor_input);
    }
    let tok_emb = parameter_inputs
        .get("tok_emb.weight")
        .cloned()
        .ok_or_else(
            || ParameterGolfBaselineGraphError::MissingParameterBinding {
                parameter_id: String::from("tok_emb.weight"),
            },
        )?;
    let mut embedded_input =
        builder.parameter_golf_token_embedding_lookup(&input_token_ids, &tok_emb)?;
    if let Some(bigram_token_ids) = &bigram_token_ids {
        let bigram_emb = parameter_inputs
            .get("bigram.embed.weight")
            .cloned()
            .ok_or_else(
                || ParameterGolfBaselineGraphError::MissingParameterBinding {
                    parameter_id: String::from("bigram.embed.weight"),
                },
            )?;
        let bigram_embedded =
            builder.parameter_golf_token_embedding_lookup(bigram_token_ids, &bigram_emb)?;
        let bigram_features = if config.bigram_dim == config.model_dim {
            bigram_embedded
        } else {
            let bigram_proj = parameter_inputs
                .get("bigram.proj.weight")
                .cloned()
                .ok_or_else(
                    || ParameterGolfBaselineGraphError::MissingParameterBinding {
                        parameter_id: String::from("bigram.proj.weight"),
                    },
                )?;
            linear_3d(
                builder,
                &bigram_embedded,
                &bigram_proj,
                batch_size,
                sequence_length,
                config.bigram_dim,
                config.model_dim,
                use_bf16_fast_path,
            )?
        };
        let bigram_scale = parameter_inputs
            .get("bigram.scale")
            .cloned()
            .ok_or_else(
                || ParameterGolfBaselineGraphError::MissingParameterBinding {
                    parameter_id: String::from("bigram.scale"),
                },
            )?;
        let bigram_scale = builder.reshape(&bigram_scale, Shape::new(vec![1, 1, 1]))?;
        let bigram_scale = builder.expand(
            &bigram_scale,
            Shape::new(vec![batch_size, sequence_length, config.model_dim]),
        )?;
        let bigram_features = builder.mul(&bigram_features, &bigram_scale)?;
        embedded_input = builder.add(&embedded_input, &bigram_features)?;
    }
    let value_embedding_base = if config.ve_layer_indices.is_empty() {
        None
    } else {
        let value_embedding = parameter_inputs
            .get("ve_shared.embed.weight")
            .cloned()
            .ok_or_else(
                || ParameterGolfBaselineGraphError::MissingParameterBinding {
                    parameter_id: String::from("ve_shared.embed.weight"),
                },
            )?;
        let value_embedding =
            builder.parameter_golf_token_embedding_lookup(&input_token_ids, &value_embedding)?;
        let value_embedding = if config.ve_dim == config.kv_dim()? {
            value_embedding
        } else {
            let value_embedding_proj = parameter_inputs
                .get("ve_shared.proj.weight")
                .cloned()
                .ok_or_else(
                    || ParameterGolfBaselineGraphError::MissingParameterBinding {
                        parameter_id: String::from("ve_shared.proj.weight"),
                    },
                )?;
            linear_3d(
                builder,
                &value_embedding,
                &value_embedding_proj,
                batch_size,
                sequence_length,
                config.ve_dim,
                config.kv_dim()?,
                use_bf16_fast_path,
            )?
        };
        let value_embedding_scale = parameter_inputs
            .get("ve_shared.scale")
            .cloned()
            .ok_or_else(
                || ParameterGolfBaselineGraphError::MissingParameterBinding {
                    parameter_id: String::from("ve_shared.scale"),
                },
            )?;
        let value_embedding_scale =
            builder.reshape(&value_embedding_scale, Shape::new(vec![1, 1, 1]))?;
        let value_embedding_scale = builder.expand(
            &value_embedding_scale,
            Shape::new(vec![batch_size, sequence_length, config.kv_dim()?]),
        )?;
        Some(builder.mul(&value_embedding, &value_embedding_scale)?)
    };

    let ones_model = builder.constant_f32(
        Shape::new(vec![config.model_dim]),
        vec![1.0; config.model_dim],
    )?;
    let head_dim = config.head_dim()?;
    let ones_head = builder.constant_f32(Shape::new(vec![head_dim]), vec![1.0; head_dim])?;
    let (rope_cos, rope_sin) = rope_table_constants(
        builder,
        sequence_length,
        config.effective_rope_rotary_dim()?,
        config.rope_base,
    )?;
    let use_banked_matrix_execution = matrix_execution_mode.uses_direct_banked_execution();

    let mut x = builder.rms_norm(
        &embedded_input,
        &ones_model,
        PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON,
    )?;
    let x0 = x.clone();
    let mut skips = Vec::new();
    for layer_index in 0..config.num_encoder_layers() {
        x = block_forward_graph(
            builder,
            &parameter_inputs,
            &x,
            &x0,
            config,
            batch_size,
            sequence_length,
            layer_index,
            value_embedding_base.as_ref(),
            &ones_model,
            &ones_head,
            &rope_cos,
            &rope_sin,
            use_bf16_fast_path,
            use_banked_matrix_execution,
        )?;
        skips.push(x.clone());
    }
    let skip_weights = parameter_inputs
        .get("skip_weights")
        .cloned()
        .ok_or_else(
            || ParameterGolfBaselineGraphError::MissingParameterBinding {
                parameter_id: String::from("skip_weights"),
            },
        )?;
    for decoder_index in 0..config.num_decoder_layers() {
        if let Some(skip) = skips.pop() {
            let skip_scale = select_parameter_row_graph(
                builder,
                &skip_weights,
                decoder_index,
                config.model_dim,
            )?;
            x = add_scaled_graph(
                builder,
                &x,
                &skip,
                &skip_scale,
                Shape::new(vec![batch_size, sequence_length, config.model_dim]),
            )?;
        }
        x = block_forward_graph(
            builder,
            &parameter_inputs,
            &x,
            &x0,
            config,
            batch_size,
            sequence_length,
            config.num_encoder_layers() + decoder_index,
            value_embedding_base.as_ref(),
            &ones_model,
            &ones_head,
            &rope_cos,
            &rope_sin,
            use_bf16_fast_path,
            use_banked_matrix_execution,
        )?;
    }
    let hidden = builder.rms_norm(&x, &ones_model, PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON)?;
    let pre_softcap_logits = hidden_to_pre_softcap_logits_graph(
        builder,
        &parameter_inputs,
        &hidden,
        config,
        batch_size,
        sequence_length,
        use_bf16_fast_path,
    )?;

    Ok(ParameterGolfBaselineGraphBuildState {
        input_token_ids,
        bigram_token_ids,
        parameter_bindings,
        pre_softcap_logits,
    })
}

fn block_forward_graph(
    builder: &mut AutodiffGraphBuilder,
    parameters: &BTreeMap<String, AutodiffTensor>,
    x: &AutodiffTensor,
    x0: &AutodiffTensor,
    config: &ParameterGolfConfig,
    batch_size: usize,
    sequence_length: usize,
    layer_index: usize,
    value_embedding_base: Option<&AutodiffTensor>,
    ones_model: &AutodiffTensor,
    ones_head: &AutodiffTensor,
    rope_cos: &AutodiffTensor,
    rope_sin: &AutodiffTensor,
    use_bf16_fast_path: bool,
    use_banked_matrix_execution: bool,
) -> Result<AutodiffTensor, ParameterGolfBaselineGraphError> {
    let resid_mix = parameters
        .get(format!("blocks.{layer_index}.resid_mix").as_str())
        .cloned()
        .ok_or_else(
            || ParameterGolfBaselineGraphError::MissingParameterBinding {
                parameter_id: format!("blocks.{layer_index}.resid_mix"),
            },
        )?;
    let mixed = blend_with_source_graph(
        builder,
        x,
        x0,
        &resid_mix,
        batch_size,
        sequence_length,
        config.model_dim,
    )?;
    let normed_for_attention =
        builder.rms_norm(&mixed, ones_model, PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON)?;
    let normed_for_attention = apply_layer_norm_scale_graph(
        builder,
        &normed_for_attention,
        batch_size,
        sequence_length,
        config.model_dim,
        config.layer_norm_scale_factor(layer_index),
    )?;
    let attention = attention_forward_graph(
        builder,
        parameters,
        &normed_for_attention,
        config,
        batch_size,
        sequence_length,
        layer_index,
        value_embedding_base,
        ones_head,
        rope_cos,
        rope_sin,
        use_bf16_fast_path,
        use_banked_matrix_execution,
    )?;
    let attn_scale = parameters
        .get(format!("blocks.{layer_index}.attn_scale").as_str())
        .cloned()
        .ok_or_else(
            || ParameterGolfBaselineGraphError::MissingParameterBinding {
                parameter_id: format!("blocks.{layer_index}.attn_scale"),
            },
        )?;
    let x = add_scaled_graph(
        builder,
        &mixed,
        &attention,
        &attn_scale,
        Shape::new(vec![batch_size, sequence_length, config.model_dim]),
    )?;
    let normed_for_mlp =
        builder.rms_norm(&x, ones_model, PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON)?;
    let normed_for_mlp = apply_layer_norm_scale_graph(
        builder,
        &normed_for_mlp,
        batch_size,
        sequence_length,
        config.model_dim,
        config.layer_norm_scale_factor(layer_index),
    )?;
    let mlp = mlp_forward_graph(
        builder,
        parameters,
        &normed_for_mlp,
        config,
        batch_size,
        sequence_length,
        layer_index,
        use_bf16_fast_path,
        use_banked_matrix_execution,
    )?;
    let mlp_scale = parameters
        .get(format!("blocks.{layer_index}.mlp_scale").as_str())
        .cloned()
        .ok_or_else(
            || ParameterGolfBaselineGraphError::MissingParameterBinding {
                parameter_id: format!("blocks.{layer_index}.mlp_scale"),
            },
        )?;
    add_scaled_graph(
        builder,
        &x,
        &mlp,
        &mlp_scale,
        Shape::new(vec![batch_size, sequence_length, config.model_dim]),
    )
}

fn attention_forward_graph(
    builder: &mut AutodiffGraphBuilder,
    parameters: &BTreeMap<String, AutodiffTensor>,
    input: &AutodiffTensor,
    config: &ParameterGolfConfig,
    batch_size: usize,
    sequence_length: usize,
    layer_index: usize,
    value_embedding_base: Option<&AutodiffTensor>,
    ones_head: &AutodiffTensor,
    rope_cos: &AutodiffTensor,
    rope_sin: &AutodiffTensor,
    use_bf16_fast_path: bool,
    use_banked_matrix_execution: bool,
) -> Result<AutodiffTensor, ParameterGolfBaselineGraphError> {
    let head_dim = config.head_dim()?;
    let kv_dim = config.kv_dim()?;
    let rope_rotary_dim = config.effective_rope_rotary_dim()?;
    let q_proj = parameter_or_banked_linear_3d_graph(
        builder,
        parameters,
        input,
        &format!("blocks.{layer_index}.attn.c_q.weight"),
        PARAMETER_GOLF_QO_BANK_NAME,
        layer_index,
        batch_size,
        sequence_length,
        config.model_dim,
        config.model_dim,
        use_bf16_fast_path,
        use_banked_matrix_execution,
    )?;
    let k_proj = parameter_or_banked_linear_3d_graph(
        builder,
        parameters,
        input,
        &format!("blocks.{layer_index}.attn.c_k.weight"),
        PARAMETER_GOLF_KV_BANK_NAME,
        layer_index,
        batch_size,
        sequence_length,
        config.model_dim,
        kv_dim,
        use_bf16_fast_path,
        use_banked_matrix_execution,
    )?;
    let mut v_proj = parameter_or_banked_linear_3d_graph(
        builder,
        parameters,
        input,
        &format!("blocks.{layer_index}.attn.c_v.weight"),
        PARAMETER_GOLF_KV_BANK_NAME,
        config.num_layers + layer_index,
        batch_size,
        sequence_length,
        config.model_dim,
        kv_dim,
        use_bf16_fast_path,
        use_banked_matrix_execution,
    )?;
    if let Some(value_embedding) = value_embedding_for_layer_graph(
        builder,
        parameters,
        value_embedding_base,
        config,
        batch_size,
        sequence_length,
        layer_index,
        kv_dim,
    )? {
        v_proj = builder.add(&v_proj, &value_embedding)?;
    }

    let q = reshape_to_attention_heads(
        builder,
        &q_proj,
        batch_size,
        sequence_length,
        config.num_heads,
        head_dim,
    )?;
    let k = reshape_to_attention_heads(
        builder,
        &k_proj,
        batch_size,
        sequence_length,
        config.num_kv_heads,
        head_dim,
    )?;
    let v = reshape_to_attention_heads(
        builder,
        &v_proj,
        batch_size,
        sequence_length,
        config.num_kv_heads,
        head_dim,
    )?;

    let q = builder.rms_norm(&q, ones_head, PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON)?;
    let k = builder.rms_norm(&k, ones_head, PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON)?;
    let q = apply_rope_graph(builder, &q, rope_cos, rope_sin, head_dim, rope_rotary_dim)?;
    let k = apply_rope_graph(builder, &k, rope_cos, rope_sin, head_dim, rope_rotary_dim)?;
    let q_gain = parameters
        .get(format!("blocks.{layer_index}.attn.q_gain").as_str())
        .cloned()
        .ok_or_else(
            || ParameterGolfBaselineGraphError::MissingParameterBinding {
                parameter_id: format!("blocks.{layer_index}.attn.q_gain"),
            },
        )?;
    let q_gain = builder.reshape(&q_gain, Shape::new(vec![1, config.num_heads, 1, 1]))?;
    let q_gain = builder.expand(
        &q_gain,
        Shape::new(vec![
            batch_size,
            config.num_heads,
            sequence_length,
            head_dim,
        ]),
    )?;
    let q = builder.mul(&q, &q_gain)?;
    let use_bf16_attention_fast_path = use_bf16_fast_path;
    let q = if use_bf16_attention_fast_path {
        builder.cast(&q, DType::BF16)?
    } else {
        q
    };
    let k = if use_bf16_attention_fast_path {
        builder.cast(&k, DType::BF16)?
    } else {
        k
    };
    let v = if use_bf16_attention_fast_path {
        builder.cast(&v, DType::BF16)?
    } else {
        v
    };
    let attended = builder.scaled_dot_product_attention(
        &q,
        &k,
        &v,
        1.0_f32 / (head_dim as f32).sqrt(),
        true,
    )?;
    let attended = if config.xsa_applies_to_layer(layer_index) {
        let attended = if use_bf16_attention_fast_path {
            builder.cast(&attended, DType::F32)?
        } else {
            attended
        };
        let value_for_xsa = if use_bf16_attention_fast_path {
            builder.cast(&v, DType::F32)?
        } else {
            v.clone()
        };
        apply_xsa_graph(
            builder,
            &attended,
            &value_for_xsa,
            ones_head,
            batch_size,
            sequence_length,
            config.num_heads,
            config.num_kv_heads,
            head_dim,
        )?
    } else if use_bf16_attention_fast_path {
        builder.cast(&attended, DType::F32)?
    } else {
        attended
    };
    let merged = builder.permute(&attended, vec![0, 2, 1, 3])?;
    let merged = builder.reshape(
        &merged,
        Shape::new(vec![batch_size, sequence_length, config.model_dim]),
    )?;
    parameter_or_banked_linear_3d_graph(
        builder,
        parameters,
        &merged,
        &format!("blocks.{layer_index}.attn.proj.weight"),
        PARAMETER_GOLF_QO_BANK_NAME,
        config.num_layers + layer_index,
        batch_size,
        sequence_length,
        config.model_dim,
        config.model_dim,
        use_bf16_fast_path,
        use_banked_matrix_execution,
    )
}

fn value_embedding_for_layer_graph(
    builder: &mut AutodiffGraphBuilder,
    parameters: &BTreeMap<String, AutodiffTensor>,
    value_embedding_base: Option<&AutodiffTensor>,
    config: &ParameterGolfConfig,
    batch_size: usize,
    sequence_length: usize,
    layer_index: usize,
    kv_dim: usize,
) -> Result<Option<AutodiffTensor>, ParameterGolfBaselineGraphError> {
    let Some(value_embedding_base) = value_embedding_base else {
        return Ok(None);
    };
    let Some(slot) = config.ve_layer_slot(layer_index) else {
        return Ok(None);
    };
    let layer_scale = parameters
        .get(format!("ve_layer_scales.{slot}").as_str())
        .cloned()
        .ok_or_else(
            || ParameterGolfBaselineGraphError::MissingParameterBinding {
                parameter_id: format!("ve_layer_scales.{slot}"),
            },
        )?;
    let layer_scale = builder.reshape(&layer_scale, Shape::new(vec![1, 1, 1]))?;
    let layer_scale = builder.expand(
        &layer_scale,
        Shape::new(vec![batch_size, sequence_length, kv_dim]),
    )?;
    Ok(Some(builder.mul(value_embedding_base, &layer_scale)?))
}

fn mlp_forward_graph(
    builder: &mut AutodiffGraphBuilder,
    parameters: &BTreeMap<String, AutodiffTensor>,
    input: &AutodiffTensor,
    config: &ParameterGolfConfig,
    batch_size: usize,
    sequence_length: usize,
    layer_index: usize,
    use_bf16_fast_path: bool,
    use_banked_matrix_execution: bool,
) -> Result<AutodiffTensor, ParameterGolfBaselineGraphError> {
    let hidden_dim = config.mlp_hidden_dim()?;
    let hidden = parameter_or_banked_linear_3d_graph(
        builder,
        parameters,
        input,
        &format!("blocks.{layer_index}.mlp.fc.weight"),
        PARAMETER_GOLF_MLP_UP_BANK_NAME,
        layer_index,
        batch_size,
        sequence_length,
        config.model_dim,
        hidden_dim,
        use_bf16_fast_path,
        use_banked_matrix_execution,
    )?;
    let activated = match config.mlp_activation {
        ParameterGolfMlpActivation::ReluSquared => builder.relu_squared(&hidden)?,
        ParameterGolfMlpActivation::LeakyReluSquared { negative_slope } => {
            builder.leaky_relu_squared(&hidden, negative_slope.to_f32())?
        }
    };
    parameter_or_banked_linear_3d_graph(
        builder,
        parameters,
        &activated,
        &format!("blocks.{layer_index}.mlp.proj.weight"),
        PARAMETER_GOLF_MLP_DOWN_BANK_NAME,
        layer_index,
        batch_size,
        sequence_length,
        hidden_dim,
        config.model_dim,
        use_bf16_fast_path,
        use_banked_matrix_execution,
    )
}

fn hidden_to_pre_softcap_logits_graph(
    builder: &mut AutodiffGraphBuilder,
    parameters: &BTreeMap<String, AutodiffTensor>,
    hidden: &AutodiffTensor,
    config: &ParameterGolfConfig,
    batch_size: usize,
    sequence_length: usize,
    use_bf16_fast_path: bool,
) -> Result<AutodiffTensor, ParameterGolfBaselineGraphError> {
    let head_weight = if config.tie_embeddings {
        parameters.get("tok_emb.weight").cloned().ok_or_else(|| {
            ParameterGolfBaselineGraphError::MissingParameterBinding {
                parameter_id: String::from("tok_emb.weight"),
            }
        })?
    } else {
        parameters.get("lm_head.weight").cloned().ok_or_else(|| {
            ParameterGolfBaselineGraphError::MissingParameterBinding {
                parameter_id: String::from("lm_head.weight"),
            }
        })?
    };
    linear_3d(
        builder,
        hidden,
        &head_weight,
        batch_size,
        sequence_length,
        config.model_dim,
        config.vocab_size,
        use_bf16_fast_path,
    )
}

fn linear_3d(
    builder: &mut AutodiffGraphBuilder,
    input: &AutodiffTensor,
    weight: &AutodiffTensor,
    batch_size: usize,
    sequence_length: usize,
    in_features: usize,
    out_features: usize,
    use_bf16_fast_path: bool,
) -> Result<AutodiffTensor, ParameterGolfBaselineGraphError> {
    let flattened = builder.reshape(
        input,
        Shape::new(vec![batch_size * sequence_length, in_features]),
    )?;
    let transposed_weight = builder.permute(weight, vec![1, 0])?;
    let output = if use_bf16_fast_path && transposed_weight.spec().dtype() == DType::BF16 {
        let lowered_input = if flattened.spec().dtype() == DType::F32 {
            flattened.clone()
        } else {
            builder.cast(&flattened, DType::F32)?
        };
        builder.matmul(&lowered_input, &transposed_weight)?
    } else {
        builder.matmul(&flattened, &transposed_weight)?
    };
    Ok(builder.reshape(
        &output,
        Shape::new(vec![batch_size, sequence_length, out_features]),
    )?)
}

fn parameter_or_banked_linear_3d_graph(
    builder: &mut AutodiffGraphBuilder,
    parameters: &BTreeMap<String, AutodiffTensor>,
    input: &AutodiffTensor,
    parameter_id: &str,
    bank_parameter_id: &str,
    bank_index: usize,
    batch_size: usize,
    sequence_length: usize,
    in_features: usize,
    out_features: usize,
    use_bf16_fast_path: bool,
    use_banked_matrix_execution: bool,
) -> Result<AutodiffTensor, ParameterGolfBaselineGraphError> {
    if let Some(parameter) = parameters.get(parameter_id) {
        return linear_3d(
            builder,
            input,
            parameter,
            batch_size,
            sequence_length,
            in_features,
            out_features,
            use_bf16_fast_path,
        );
    }
    let bank = parameters.get(bank_parameter_id).ok_or_else(|| {
        ParameterGolfBaselineGraphError::MissingParameterBinding {
            parameter_id: String::from(parameter_id),
        }
    })?;
    if use_banked_matrix_execution {
        let lowered_input = if use_bf16_fast_path
            && bank.spec().dtype() == DType::BF16
            && input.spec().dtype() == DType::F32
        {
            builder.cast(input, DType::BF16)?
        } else {
            input.clone()
        };
        return builder
            .parameter_golf_banked_linear(&lowered_input, bank, bank_index)
            .map_err(Into::into);
    }
    let weight =
        select_parameter_matrix_slice_graph(builder, bank, bank_index, out_features, in_features)?;
    linear_3d(
        builder,
        input,
        &weight,
        batch_size,
        sequence_length,
        in_features,
        out_features,
        use_bf16_fast_path,
    )
}

fn apply_layer_norm_scale_graph(
    builder: &mut AutodiffGraphBuilder,
    input: &AutodiffTensor,
    batch_size: usize,
    sequence_length: usize,
    feature_count: usize,
    scale: f32,
) -> Result<AutodiffTensor, ParameterGolfBaselineGraphError> {
    if scale == 1.0 {
        return Ok(input.clone());
    }
    let scale = builder.constant_f32(Shape::new(vec![1, 1, 1]), vec![scale])?;
    let scale = builder.expand(
        &scale,
        Shape::new(vec![batch_size, sequence_length, feature_count]),
    )?;
    Ok(builder.mul(input, &scale)?)
}

fn apply_xsa_graph(
    builder: &mut AutodiffGraphBuilder,
    attended: &AutodiffTensor,
    value_heads: &AutodiffTensor,
    ones_head: &AutodiffTensor,
    batch_size: usize,
    sequence_length: usize,
    num_heads: usize,
    num_kv_heads: usize,
    head_dim: usize,
) -> Result<AutodiffTensor, ParameterGolfBaselineGraphError> {
    let group_size = num_heads / num_kv_heads;
    let grouped = builder.reshape(
        attended,
        Shape::new(vec![
            batch_size,
            num_kv_heads,
            group_size,
            sequence_length,
            head_dim,
        ]),
    )?;
    let normalized_value = builder.rms_norm(
        value_heads,
        ones_head,
        PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON,
    )?;
    let normalized_value = scale_tensor4_graph(
        builder,
        &normalized_value,
        batch_size,
        num_kv_heads,
        sequence_length,
        head_dim,
        1.0_f32 / (head_dim as f32).sqrt(),
    )?;
    let normalized_value = builder.reshape(
        &normalized_value,
        Shape::new(vec![batch_size, num_kv_heads, 1, sequence_length, head_dim]),
    )?;
    let normalized_value = builder.expand(
        &normalized_value,
        Shape::new(vec![
            batch_size,
            num_kv_heads,
            group_size,
            sequence_length,
            head_dim,
        ]),
    )?;
    let aligned = builder.mul(&grouped, &normalized_value)?;
    let aligned = builder.reduce_sum_axis(&aligned, 4)?;
    let aligned = builder.reshape(
        &aligned,
        Shape::new(vec![
            batch_size,
            num_kv_heads,
            group_size,
            sequence_length,
            1,
        ]),
    )?;
    let aligned = builder.expand(
        &aligned,
        Shape::new(vec![
            batch_size,
            num_kv_heads,
            group_size,
            sequence_length,
            head_dim,
        ]),
    )?;
    let projection = builder.mul(&aligned, &normalized_value)?;
    let negative_one = builder.constant_f32(Shape::new(vec![1, 1, 1, 1, 1]), vec![-1.0])?;
    let negative_one = builder.expand(
        &negative_one,
        Shape::new(vec![
            batch_size,
            num_kv_heads,
            group_size,
            sequence_length,
            head_dim,
        ]),
    )?;
    let negative_projection = builder.mul(&projection, &negative_one)?;
    let orthogonalized = builder.add(&grouped, &negative_projection)?;
    Ok(builder.reshape(
        &orthogonalized,
        Shape::new(vec![batch_size, num_heads, sequence_length, head_dim]),
    )?)
}

fn scale_tensor4_graph(
    builder: &mut AutodiffGraphBuilder,
    input: &AutodiffTensor,
    dim0: usize,
    dim1: usize,
    dim2: usize,
    dim3: usize,
    scale: f32,
) -> Result<AutodiffTensor, ParameterGolfBaselineGraphError> {
    if scale == 1.0 {
        return Ok(input.clone());
    }
    let scale = builder.constant_f32(Shape::new(vec![1, 1, 1, 1]), vec![scale])?;
    let scale = builder.expand(&scale, Shape::new(vec![dim0, dim1, dim2, dim3]))?;
    Ok(builder.mul(input, &scale)?)
}

fn apply_rope_graph(
    builder: &mut AutodiffGraphBuilder,
    input: &AutodiffTensor,
    rope_cos: &AutodiffTensor,
    rope_sin: &AutodiffTensor,
    head_dim: usize,
    rope_rotary_dim: usize,
) -> Result<AutodiffTensor, ParameterGolfBaselineGraphError> {
    if rope_rotary_dim == head_dim {
        return Ok(builder.rope(input, rope_cos, rope_sin, false)?);
    }
    let rotated_prefix = builder.slice(input, 3, 0, rope_rotary_dim)?;
    let rotated_prefix = builder.rope(&rotated_prefix, rope_cos, rope_sin, false)?;
    let passthrough_suffix = builder.slice(input, 3, rope_rotary_dim, head_dim)?;
    Ok(builder.concat(&[rotated_prefix, passthrough_suffix], 3)?)
}

fn reshape_to_attention_heads(
    builder: &mut AutodiffGraphBuilder,
    tensor: &AutodiffTensor,
    batch_size: usize,
    sequence_length: usize,
    head_count: usize,
    head_dim: usize,
) -> Result<AutodiffTensor, ParameterGolfBaselineGraphError> {
    let reshaped = builder.reshape(
        tensor,
        Shape::new(vec![batch_size, sequence_length, head_count, head_dim]),
    )?;
    Ok(builder.permute(&reshaped, vec![0, 2, 1, 3])?)
}

fn blend_with_source_graph(
    builder: &mut AutodiffGraphBuilder,
    current: &AutodiffTensor,
    source: &AutodiffTensor,
    mix: &AutodiffTensor,
    batch_size: usize,
    sequence_length: usize,
    model_dim: usize,
) -> Result<AutodiffTensor, ParameterGolfBaselineGraphError> {
    let current_mix = select_parameter_row_graph(builder, mix, 0, model_dim)?;
    let source_mix = select_parameter_row_graph(builder, mix, 1, model_dim)?;
    let current_mix = builder.reshape(&current_mix, Shape::new(vec![1, 1, model_dim]))?;
    let source_mix = builder.reshape(&source_mix, Shape::new(vec![1, 1, model_dim]))?;
    let current_mix = builder.expand(
        &current_mix,
        Shape::new(vec![batch_size, sequence_length, model_dim]),
    )?;
    let source_mix = builder.expand(
        &source_mix,
        Shape::new(vec![batch_size, sequence_length, model_dim]),
    )?;
    let current = builder.mul(current, &current_mix)?;
    let source = builder.mul(source, &source_mix)?;
    Ok(builder.add(&current, &source)?)
}

fn add_scaled_graph(
    builder: &mut AutodiffGraphBuilder,
    base: &AutodiffTensor,
    delta: &AutodiffTensor,
    scale: &AutodiffTensor,
    target_shape: Shape,
) -> Result<AutodiffTensor, ParameterGolfBaselineGraphError> {
    let feature_count = *target_shape
        .dims()
        .last()
        .expect("parameter golf add_scaled target shape should have width");
    let scale = builder.reshape(scale, Shape::new(vec![1, 1, feature_count]))?;
    let scale = builder.expand(&scale, target_shape)?;
    let scaled_delta = builder.mul(delta, &scale)?;
    Ok(builder.add(base, &scaled_delta)?)
}

fn select_parameter_row_graph(
    builder: &mut AutodiffGraphBuilder,
    parameter: &AutodiffTensor,
    row_index: usize,
    row_width: usize,
) -> Result<AutodiffTensor, ParameterGolfBaselineGraphError> {
    let row_count = *parameter.spec().shape().dims().first().ok_or_else(|| {
        ParameterGolfBaselineGraphError::Graph(GraphError::InvalidOperatorInputs {
            op: String::from("select_parameter_row_graph"),
            message: format!(
                "expected rank-2 parameter [rows, width], found shape {:?}",
                parameter.spec().shape().dims()
            ),
        })
    })?;
    let actual_row_width = *parameter.spec().shape().dims().get(1).ok_or_else(|| {
        ParameterGolfBaselineGraphError::Graph(GraphError::InvalidOperatorInputs {
            op: String::from("select_parameter_row_graph"),
            message: format!(
                "expected rank-2 parameter [rows, width], found shape {:?}",
                parameter.spec().shape().dims()
            ),
        })
    })?;
    if actual_row_width != row_width {
        return Err(ParameterGolfBaselineGraphError::Graph(
            GraphError::InvalidOperatorInputs {
                op: String::from("select_parameter_row_graph"),
                message: format!("expected row width {row_width}, found {actual_row_width}"),
            },
        ));
    }
    if row_index >= row_count {
        return Err(ParameterGolfBaselineGraphError::Graph(
            GraphError::InvalidOperatorInputs {
                op: String::from("select_parameter_row_graph"),
                message: format!("row index {row_index} exceeds row count {row_count}"),
            },
        ));
    }
    let mut selector = vec![0.0_f32; row_count];
    selector[row_index] = 1.0;
    let selector = builder.constant_f32(Shape::new(vec![1, row_count]), selector)?;
    let selected = builder.matmul(&selector, parameter)?;
    builder
        .reshape(&selected, Shape::new(vec![row_width]))
        .map_err(Into::into)
}

fn select_parameter_matrix_slice_graph(
    builder: &mut AutodiffGraphBuilder,
    parameter: &AutodiffTensor,
    bank_index: usize,
    out_features: usize,
    in_features: usize,
) -> Result<AutodiffTensor, ParameterGolfBaselineGraphError> {
    let dims = parameter.spec().shape().dims();
    if dims.len() != 3 {
        return Err(ParameterGolfBaselineGraphError::Graph(
            GraphError::InvalidOperatorInputs {
                op: String::from("select_parameter_matrix_slice_graph"),
                message: format!(
                    "expected rank-3 bank parameter [bank_len, out_features, in_features], found shape {:?}",
                    dims
                ),
            },
        ));
    }
    if dims[1] != out_features || dims[2] != in_features {
        return Err(ParameterGolfBaselineGraphError::Graph(
            GraphError::InvalidOperatorInputs {
                op: String::from("select_parameter_matrix_slice_graph"),
                message: format!(
                    "expected bank slice shape [{out_features}, {in_features}], found [{}, {}]",
                    dims[1], dims[2]
                ),
            },
        ));
    }
    if bank_index >= dims[0] {
        return Err(ParameterGolfBaselineGraphError::Graph(
            GraphError::InvalidOperatorInputs {
                op: String::from("select_parameter_matrix_slice_graph"),
                message: format!("bank index {bank_index} exceeds bank length {}", dims[0]),
            },
        ));
    }
    let slice = builder.slice(parameter, 0, bank_index, bank_index + 1)?;
    builder
        .reshape(&slice, Shape::new(vec![out_features, in_features]))
        .map_err(Into::into)
}

fn rope_table_constants(
    builder: &mut AutodiffGraphBuilder,
    sequence_length: usize,
    rope_rotary_dim: usize,
    rope_base: f32,
) -> Result<(AutodiffTensor, AutodiffTensor), ParameterGolfBaselineGraphError> {
    let half_dim = rope_rotary_dim / 2;
    let mut cos = vec![0.0_f32; sequence_length * half_dim];
    let mut sin = vec![0.0_f32; sequence_length * half_dim];
    for position in 0..sequence_length {
        for feature in 0..half_dim {
            let exponent = (2 * feature) as f32 / rope_rotary_dim as f32;
            let inv_freq = 1.0_f32 / rope_base.powf(exponent);
            let angle = position as f32 * inv_freq;
            cos[position * half_dim + feature] = angle.cos();
            sin[position * half_dim + feature] = angle.sin();
        }
    }
    Ok((
        builder.constant_f32(Shape::new(vec![sequence_length, half_dim]), cos)?,
        builder.constant_f32(Shape::new(vec![sequence_length, half_dim]), sin)?,
    ))
}

fn validate_token_batch(
    input_ids: &[Vec<u32>],
    vocab_size: usize,
) -> Result<(usize, usize), ParameterGolfExecutionError> {
    if input_ids.is_empty() {
        return Err(ParameterGolfExecutionError::EmptyBatch);
    }
    let sequence_length = input_ids.first().map(Vec::len).unwrap_or(0);
    if sequence_length == 0 {
        return Err(ParameterGolfExecutionError::EmptySequence);
    }
    for (batch_index, row) in input_ids.iter().enumerate() {
        if row.len() != sequence_length {
            return Err(ParameterGolfExecutionError::RaggedBatch {
                batch_index,
                expected: sequence_length,
                actual: row.len(),
            });
        }
        for &token_id in row {
            if token_id as usize >= vocab_size {
                return Err(ParameterGolfExecutionError::TokenOutOfRange {
                    token_id,
                    vocab_size,
                });
            }
        }
    }
    Ok((input_ids.len(), sequence_length))
}

fn validate_target_shape(
    target_ids: &[Vec<u32>],
    expected_batch: usize,
    expected_sequence: usize,
) -> Result<(), ParameterGolfExecutionError> {
    if target_ids.len() != expected_batch
        || target_ids.first().map(Vec::len).unwrap_or(0) != expected_sequence
    {
        return Err(ParameterGolfExecutionError::TargetShapeMismatch {
            expected_batch,
            expected_sequence,
            actual_batch: target_ids.len(),
            actual_sequence: target_ids.first().map(Vec::len).unwrap_or(0),
        });
    }
    Ok(())
}

fn dense_gradient_values(
    data: &TensorData,
    context: String,
) -> Result<Vec<f32>, ParameterGolfBaselineGraphError> {
    match data {
        TensorData::F32(values) | TensorData::BF16(values) => Ok(values.clone()),
        TensorData::I32(_) | TensorData::QuantizedBlocks(_) => {
            Err(ParameterGolfBaselineGraphError::NonDenseTensorData { context })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use psionic_compiler::compile_graph;
    use psionic_core::{BackendExtensionOp, TensorData};
    use psionic_ir::{evaluate_graph, OpKind};
    use serde::Deserialize;

    use super::*;
    use psionic_models::{
        ModelDescriptor, ParameterGolfDeterministicInitializer, ParameterGolfLayerNormScale,
        ParameterGolfWeights,
    };

    #[derive(Deserialize)]
    struct BaselineFixture {
        initializer: ParameterGolfDeterministicInitializer,
        input_ids: Vec<Vec<u32>>,
        target_ids: Vec<Vec<u32>>,
    }

    fn load_baseline_fixture() -> BaselineFixture {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../fixtures/parameter_golf/models/parameter_golf_baseline_model_fixture.json",
        );
        serde_json::from_slice(&fs::read(path).expect("fixture should exist"))
            .expect("fixture should deserialize")
    }

    fn baseline_model() -> Result<ParameterGolfReferenceModel, Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        Ok(ParameterGolfReferenceModel::baseline_fixture(
            fixture.initializer,
        )?)
    }

    fn loss_with_parameter_override(
        model: &ParameterGolfReferenceModel,
        parameter_id: &str,
        values: Vec<f32>,
        input_ids: &[Vec<u32>],
        target_ids: &[Vec<u32>],
    ) -> Result<f32, Box<dyn std::error::Error>> {
        let mut overrides = BTreeMap::new();
        overrides.insert(String::from(parameter_id), values);
        let updated_weights = model
            .weights()
            .with_parameter_overrides(&model.descriptor().config, &overrides)?;
        let updated_model = ParameterGolfReferenceModel::new(
            ModelDescriptor::new(
                model.descriptor().model.model_id.clone(),
                model.descriptor().model.family.clone(),
                model.descriptor().model.revision.clone(),
            ),
            model.descriptor().config.clone(),
            updated_weights,
        )?;
        Ok(updated_model.loss(input_ids, target_ids)?)
    }

    fn output_tensor3(
        data: &TensorData,
        shape: [usize; 3],
        context: &str,
    ) -> Result<ParameterGolfTensor3, ParameterGolfBaselineGraphError> {
        Ok(ParameterGolfTensor3::new(
            shape,
            dense_gradient_values(data, String::from(context))?,
        )?)
    }

    #[test]
    fn parameter_golf_baseline_graph_matches_reference_logits_on_fixture_batch(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let model = baseline_model()?;
        let graph = build_parameter_golf_baseline_graph(
            Device::cpu(),
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;
        compile_graph(graph.graph.graph())?;

        let inputs = bind_parameter_golf_baseline_graph_inputs(
            &graph,
            &model,
            fixture.input_ids.as_slice(),
        )?;
        let values = evaluate_graph(graph.graph.graph(), &inputs)?;
        let pre_softcap_logits = output_tensor3(
            values
                .get(&graph.pre_softcap_logits_tensor_id)
                .ok_or("missing pre-softcap logits")?,
            [1, 4, model.descriptor().config.vocab_size],
            "pre_softcap_logits",
        )?;
        let seeded = parameter_golf_projection_seed(
            &pre_softcap_logits,
            fixture.target_ids.as_slice(),
            model.descriptor().config.logit_softcap,
        )?;
        let reference = model.forward_logits(fixture.input_ids.as_slice())?;
        let max_abs_diff = seeded.softcapped_logits.max_abs_diff(&reference)?;
        assert!(
            max_abs_diff < 5e-5,
            "max logit drift {max_abs_diff} exceeded tolerance"
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_baseline_graph_matches_reference_logits_with_leaky_relu_squared(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let config = ParameterGolfConfig {
            mlp_activation: ParameterGolfMlpActivation::leaky_relu_squared_point_five(),
            ..ParameterGolfConfig::baseline_sp1024_9x512()
        };
        let model = ParameterGolfReferenceModel::new(
            ModelDescriptor::new("parameter-golf-test-leaky", "parameter_golf_decoder", "v1"),
            config.clone(),
            ParameterGolfWeights::from_initializer(&config, fixture.initializer)?,
        )?;
        let graph = build_parameter_golf_baseline_graph(
            Device::cpu(),
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;
        compile_graph(graph.graph.graph())?;

        let inputs = bind_parameter_golf_baseline_graph_inputs(
            &graph,
            &model,
            fixture.input_ids.as_slice(),
        )?;
        let values = evaluate_graph(graph.graph.graph(), &inputs)?;
        let pre_softcap_logits = output_tensor3(
            values
                .get(&graph.pre_softcap_logits_tensor_id)
                .ok_or("missing pre-softcap logits")?,
            [1, 4, model.descriptor().config.vocab_size],
            "pre_softcap_logits",
        )?;
        let seeded = parameter_golf_projection_seed(
            &pre_softcap_logits,
            fixture.target_ids.as_slice(),
            model.descriptor().config.logit_softcap,
        )?;
        let reference = model.forward_logits(fixture.input_ids.as_slice())?;
        let max_abs_diff = seeded.softcapped_logits.max_abs_diff(&reference)?;
        assert!(
            max_abs_diff < 5e-5,
            "max logit drift {max_abs_diff} exceeded tolerance"
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_baseline_graph_matches_reference_logits_with_bigram_hash(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let config = ParameterGolfConfig {
            bigram_vocab_size: 1536,
            bigram_dim: 128,
            ..ParameterGolfConfig::baseline_sp1024_9x512()
        };
        let mut weights = ParameterGolfWeights::from_initializer(&config, fixture.initializer)?;
        let hashed = config
            .bigram_hash_batch(fixture.input_ids.as_slice())?
            .expect("bigram should be enabled");
        let bigram = weights
            .bigram
            .as_mut()
            .expect("bigram weights should exist");
        for row in &hashed {
            for token_id in row {
                let row_offset = *token_id as usize * config.bigram_dim;
                for feature in 0..config.bigram_dim {
                    bigram.embedding[row_offset + feature] = 0.01 * (feature as f32 + 1.0);
                }
            }
        }
        if let Some(proj) = &mut bigram.proj {
            for value in &mut proj.weight {
                *value = 0.001;
            }
        }
        bigram.scale[0] = 0.2;
        let model = ParameterGolfReferenceModel::new(
            ModelDescriptor::new("parameter-golf-test-bigram", "parameter_golf_decoder", "v1"),
            config.clone(),
            weights,
        )?;
        let graph = build_parameter_golf_baseline_graph(
            Device::cpu(),
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;
        compile_graph(graph.graph.graph())?;

        let inputs = bind_parameter_golf_baseline_graph_inputs(
            &graph,
            &model,
            fixture.input_ids.as_slice(),
        )?;
        let values = evaluate_graph(graph.graph.graph(), &inputs)?;
        let pre_softcap_logits = output_tensor3(
            values
                .get(&graph.pre_softcap_logits_tensor_id)
                .ok_or("missing pre-softcap logits")?,
            [1, 4, model.descriptor().config.vocab_size],
            "pre_softcap_logits",
        )?;
        let seeded = parameter_golf_projection_seed(
            &pre_softcap_logits,
            fixture.target_ids.as_slice(),
            model.descriptor().config.logit_softcap,
        )?;
        let reference = model.forward_logits(fixture.input_ids.as_slice())?;
        let max_abs_diff = seeded.softcapped_logits.max_abs_diff(&reference)?;
        assert!(
            max_abs_diff < 5e-5,
            "max logit drift {max_abs_diff} exceeded tolerance"
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_baseline_graph_matches_reference_logits_with_value_embeddings(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let config = ParameterGolfConfig {
            ve_dim: 128,
            ve_layer_indices: vec![6, 7, 8],
            ..ParameterGolfConfig::baseline_sp1024_9x512()
        };
        let mut weights = ParameterGolfWeights::from_initializer(&config, fixture.initializer)?;
        let value_embedding = weights
            .value_embedding
            .as_mut()
            .expect("value-embedding weights should exist");
        for row in &fixture.input_ids {
            for token_id in row {
                let row_offset = *token_id as usize * config.ve_dim;
                for feature in 0..config.ve_dim {
                    value_embedding.embedding[row_offset + feature] = 0.01 * (feature as f32 + 1.0);
                }
            }
        }
        if let Some(proj) = &mut value_embedding.proj {
            for value in &mut proj.weight {
                *value = 0.001;
            }
        }
        value_embedding.scale[0] = 0.2;
        value_embedding.layer_scales = vec![0.5, 0.75, 1.0];
        let model = ParameterGolfReferenceModel::new(
            ModelDescriptor::new(
                "parameter-golf-test-value-embedding",
                "parameter_golf_decoder",
                "v1",
            ),
            config.clone(),
            weights,
        )?;
        let graph = build_parameter_golf_baseline_graph(
            Device::cpu(),
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;
        compile_graph(graph.graph.graph())?;

        let inputs = bind_parameter_golf_baseline_graph_inputs(
            &graph,
            &model,
            fixture.input_ids.as_slice(),
        )?;
        let values = evaluate_graph(graph.graph.graph(), &inputs)?;
        let pre_softcap_logits = output_tensor3(
            values
                .get(&graph.pre_softcap_logits_tensor_id)
                .ok_or("missing pre-softcap logits")?,
            [1, 4, model.descriptor().config.vocab_size],
            "pre_softcap_logits",
        )?;
        let seeded = parameter_golf_projection_seed(
            &pre_softcap_logits,
            fixture.target_ids.as_slice(),
            model.descriptor().config.logit_softcap,
        )?;
        let reference = model.forward_logits(fixture.input_ids.as_slice())?;
        let max_abs_diff = seeded.softcapped_logits.max_abs_diff(&reference)?;
        assert!(
            max_abs_diff < 5e-5,
            "max logit drift {max_abs_diff} exceeded tolerance"
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_baseline_graph_matches_reference_logits_with_xsa(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let config = ParameterGolfConfig {
            xsa_last_n: 2,
            ..ParameterGolfConfig::baseline_sp1024_9x512()
        };
        let model = ParameterGolfReferenceModel::new(
            ModelDescriptor::new("parameter-golf-test-xsa", "parameter_golf_decoder", "v1"),
            config.clone(),
            ParameterGolfWeights::from_initializer(&config, fixture.initializer)?,
        )?;
        let graph = build_parameter_golf_baseline_graph(
            Device::cpu(),
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;
        compile_graph(graph.graph.graph())?;

        let inputs = bind_parameter_golf_baseline_graph_inputs(
            &graph,
            &model,
            fixture.input_ids.as_slice(),
        )?;
        let values = evaluate_graph(graph.graph.graph(), &inputs)?;
        let pre_softcap_logits = output_tensor3(
            values
                .get(&graph.pre_softcap_logits_tensor_id)
                .ok_or("missing pre-softcap logits")?,
            [1, 4, model.descriptor().config.vocab_size],
            "pre_softcap_logits",
        )?;
        let seeded = parameter_golf_projection_seed(
            &pre_softcap_logits,
            fixture.target_ids.as_slice(),
            model.descriptor().config.logit_softcap,
        )?;
        let reference = model.forward_logits(fixture.input_ids.as_slice())?;
        let max_abs_diff = seeded.softcapped_logits.max_abs_diff(&reference)?;
        assert!(
            max_abs_diff < 5e-5,
            "max logit drift {max_abs_diff} exceeded tolerance"
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_baseline_graph_matches_reference_logits_with_partial_rope_and_ln_scale(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let config = ParameterGolfConfig {
            rope_rotary_dim: Some(16),
            layer_norm_scale: ParameterGolfLayerNormScale::InverseSqrtLayerIndexPlusOne,
            ..ParameterGolfConfig::baseline_sp1024_9x512()
        };
        let model = ParameterGolfReferenceModel::new(
            ModelDescriptor::new(
                "parameter-golf-test-scorepath-arch",
                "parameter_golf_decoder",
                "v1",
            ),
            config.clone(),
            ParameterGolfWeights::from_initializer(&config, fixture.initializer)?,
        )?;
        let graph = build_parameter_golf_baseline_graph(
            Device::cpu(),
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;
        compile_graph(graph.graph.graph())?;

        let inputs = bind_parameter_golf_baseline_graph_inputs(
            &graph,
            &model,
            fixture.input_ids.as_slice(),
        )?;
        let values = evaluate_graph(graph.graph.graph(), &inputs)?;
        let pre_softcap_logits = output_tensor3(
            values
                .get(&graph.pre_softcap_logits_tensor_id)
                .ok_or("missing pre-softcap logits")?,
            [1, 4, model.descriptor().config.vocab_size],
            "pre_softcap_logits",
        )?;
        let seeded = parameter_golf_projection_seed(
            &pre_softcap_logits,
            fixture.target_ids.as_slice(),
            model.descriptor().config.logit_softcap,
        )?;
        let reference = model.forward_logits(fixture.input_ids.as_slice())?;
        let max_abs_diff = seeded.softcapped_logits.max_abs_diff(&reference)?;
        assert!(
            max_abs_diff < 5e-5,
            "max logit drift {max_abs_diff} exceeded tolerance"
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_baseline_graph_materializes_seeded_gradients_for_parameter_families(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let model = baseline_model()?;
        let graph = build_parameter_golf_baseline_graph(
            Device::cpu(),
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;

        let inputs = bind_parameter_golf_baseline_graph_inputs(
            &graph,
            &model,
            fixture.input_ids.as_slice(),
        )?;
        let forward = evaluate_graph(graph.graph.graph(), &inputs)?;
        let pre_softcap_logits = output_tensor3(
            forward
                .get(&graph.pre_softcap_logits_tensor_id)
                .ok_or("missing pre-softcap logits")?,
            [1, 4, model.descriptor().config.vocab_size],
            "pre_softcap_logits",
        )?;
        let seeded = parameter_golf_projection_seed(
            &pre_softcap_logits,
            fixture.target_ids.as_slice(),
            model.descriptor().config.logit_softcap,
        )?;
        let backward = graph.graph.backward_materialized_with_seed(
            graph.pre_softcap_logits_tensor_id,
            &inputs,
            Some(TensorData::F32(
                seeded.pre_softcap_gradient.values().to_vec(),
            )),
        )?;
        let gradients = materialize_parameter_golf_baseline_gradients(
            &graph,
            &backward,
            &model.descriptor().config,
            fixture.input_ids.as_slice(),
        )?;

        let checked_coordinates = [
            ("tok_emb.weight", 17 * model.descriptor().config.model_dim),
            ("skip_weights", 0),
            ("blocks.0.attn.c_q.weight", 0),
            ("blocks.0.attn.q_gain", 0),
            ("blocks.0.attn_scale", 0),
            ("blocks.0.mlp.fc.weight", 0),
            ("blocks.0.mlp_scale", 0),
            ("blocks.0.resid_mix", 0),
        ];
        let delta = 1e-3_f32;
        for (parameter_id, flat_index) in checked_coordinates {
            let baseline = model
                .weights()
                .parameter_vector(&model.descriptor().config, parameter_id)
                .ok_or("missing baseline parameter")?;
            let mut plus = baseline.values.clone();
            plus[flat_index] += delta;
            let mut minus = baseline.values.clone();
            minus[flat_index] -= delta;
            let finite = (loss_with_parameter_override(
                &model,
                parameter_id,
                plus,
                fixture.input_ids.as_slice(),
                fixture.target_ids.as_slice(),
            )? - loss_with_parameter_override(
                &model,
                parameter_id,
                minus,
                fixture.input_ids.as_slice(),
                fixture.target_ids.as_slice(),
            )?) / (2.0 * delta);
            let actual = gradients
                .parameter_gradients
                .get(parameter_id)
                .ok_or("missing parameter gradient")?[flat_index];
            assert!(
                (actual - finite).abs() < 5e-2,
                "gradient drift for {parameter_id}[{flat_index}] exceeded tolerance: actual={actual} finite={finite}"
            );
        }
        Ok(())
    }

    #[test]
    fn parameter_golf_baseline_training_graph_matches_reference_loss_and_gradients(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let model = baseline_model()?;
        let graph = build_parameter_golf_baseline_training_graph(
            Device::cpu(),
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;

        let inputs = bind_parameter_golf_baseline_training_graph_inputs(
            &graph,
            &model,
            fixture.input_ids.as_slice(),
            fixture.target_ids.as_slice(),
        )?;
        let forward = evaluate_graph(graph.graph.graph(), &inputs)?;
        let loss = forward
            .get(&graph.loss_tensor_id)
            .ok_or("missing training loss tensor")?
            .as_f32_slice()
            .ok_or("loss tensor should be dense f32")?[0];
        let reference_loss =
            model.loss(fixture.input_ids.as_slice(), fixture.target_ids.as_slice())?;
        assert!(
            (loss - reference_loss).abs() < 5e-5,
            "training-graph loss drift exceeded tolerance: actual={loss} expected={reference_loss}"
        );

        let backward = graph
            .graph
            .backward_materialized(graph.loss_tensor_id, &inputs)?;
        let gradients = materialize_parameter_golf_baseline_training_gradients(
            &graph,
            &backward,
            &model.descriptor().config,
            fixture.input_ids.as_slice(),
        )?;
        let delta = 1e-3_f32;
        for (parameter_id, flat_index) in [
            ("tok_emb.weight", 17 * model.descriptor().config.model_dim),
            ("blocks.0.attn.c_q.weight", 0),
        ] {
            let baseline = model
                .weights()
                .parameter_vector(&model.descriptor().config, parameter_id)
                .ok_or("missing baseline parameter")?;
            let mut plus = baseline.values.clone();
            plus[flat_index] += delta;
            let mut minus = baseline.values.clone();
            minus[flat_index] -= delta;
            let finite = (loss_with_parameter_override(
                &model,
                parameter_id,
                plus,
                fixture.input_ids.as_slice(),
                fixture.target_ids.as_slice(),
            )? - loss_with_parameter_override(
                &model,
                parameter_id,
                minus,
                fixture.input_ids.as_slice(),
                fixture.target_ids.as_slice(),
            )?) / (2.0 * delta);
            let actual = gradients
                .parameter_gradients
                .get(parameter_id)
                .ok_or("missing parameter gradient")?[flat_index];
            assert!(
                (actual - finite).abs() < 5e-2,
                "training-graph gradient drift for {parameter_id}[{flat_index}] exceeded tolerance: actual={actual} finite={finite}"
            );
        }
        Ok(())
    }

    #[test]
    fn parameter_golf_baseline_training_graph_binds_token_ids_as_i32_tensors(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let model = baseline_model()?;
        let graph = build_parameter_golf_baseline_training_graph(
            Device::cpu(),
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;

        let inputs = bind_parameter_golf_baseline_training_graph_inputs(
            &graph,
            &model,
            fixture.input_ids.as_slice(),
            fixture.target_ids.as_slice(),
        )?;

        assert!(matches!(
            inputs.get(&graph.input_token_ids_tensor_id),
            Some(TensorData::I32(values))
                if values
                    == &fixture
                        .input_ids
                        .iter()
                        .flat_map(|row| row.iter().map(|token_id| *token_id as i32))
                        .collect::<Vec<_>>()
        ));
        assert!(matches!(
            inputs.get(&graph.target_ids_tensor_id),
            Some(TensorData::I32(values))
                if values
                    == &fixture
                        .target_ids
                        .iter()
                        .flat_map(|row| row.iter().map(|token_id| *token_id as i32))
                        .collect::<Vec<_>>()
        ));
        Ok(())
    }

    #[test]
    fn parameter_golf_baseline_training_graph_marks_train_visible_weight_inputs_as_bf16(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let model = baseline_model()?;
        let graph = build_parameter_golf_baseline_training_graph(
            Device::cpu(),
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;

        assert_eq!(
            graph
                .parameter_binding("tok_emb.weight")
                .ok_or("missing tok_emb binding")?
                .graph_input_dtype,
            DType::BF16
        );
        assert_eq!(
            graph
                .parameter_binding("blocks.0.attn.c_q.weight")
                .ok_or("missing c_q binding")?
                .graph_input_dtype,
            DType::BF16
        );
        assert_eq!(
            graph
                .parameter_binding("blocks.0.attn_scale")
                .ok_or("missing attn_scale binding")?
                .graph_input_dtype,
            DType::F32
        );

        let inputs = bind_parameter_golf_baseline_training_graph_inputs(
            &graph,
            &model,
            fixture.input_ids.as_slice(),
            fixture.target_ids.as_slice(),
        )?;
        assert!(matches!(
            inputs.get(
                &graph
                    .parameter_binding("tok_emb.weight")
                    .ok_or("missing tok_emb binding")?
                    .graph_input_tensor_id
            ),
            Some(TensorData::BF16(_))
        ));
        assert!(matches!(
            inputs.get(
                &graph
                    .parameter_binding("blocks.0.attn_scale")
                    .ok_or("missing attn_scale binding")?
                    .graph_input_tensor_id
            ),
            Some(TensorData::F32(_))
        ));
        Ok(())
    }

    #[test]
    fn parameter_golf_banked_training_graph_uses_direct_banked_linear_ops_and_matches_split_surface_loss(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let model = baseline_model()?;
        let banked_descriptor = model.banked_descriptor()?;
        let split_graph = build_parameter_golf_baseline_training_graph(
            Device::cpu(),
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;
        let banked_graph = build_parameter_golf_baseline_training_graph(
            Device::cpu(),
            &banked_descriptor,
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;

        assert!(banked_graph.parameter_binding("qo_bank").is_some());
        assert!(banked_graph.parameter_binding("kv_bank").is_some());
        assert!(banked_graph
            .parameter_binding("blocks.0.attn.c_q.weight")
            .is_none());
        let bank_tensor_ids = [
            PARAMETER_GOLF_QO_BANK_NAME,
            PARAMETER_GOLF_KV_BANK_NAME,
            PARAMETER_GOLF_MLP_UP_BANK_NAME,
            PARAMETER_GOLF_MLP_DOWN_BANK_NAME,
        ]
        .into_iter()
        .map(|parameter_id| {
            banked_graph
                .parameter_binding(parameter_id)
                .map(|binding| binding.graph_input_tensor_id)
                .ok_or("missing bank binding")
        })
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        let banked_linear_count = banked_graph
            .graph
            .graph()
            .nodes()
            .iter()
            .filter(|node| {
                matches!(
                    node.op(),
                    OpKind::BackendExtension {
                        op: BackendExtensionOp::ParameterGolfBankedLinear { .. }
                    }
                )
            })
            .count();
        let bank_slice_count = banked_graph
            .graph
            .graph()
            .nodes()
            .iter()
            .filter(|node| {
                matches!(node.op(), OpKind::Slice { .. } | OpKind::Select { .. })
                    && node
                        .inputs()
                        .iter()
                        .any(|tensor_id| bank_tensor_ids.contains(tensor_id))
            })
            .count();
        assert!(banked_linear_count > 0);
        assert_eq!(bank_slice_count, 0);

        let split_inputs = bind_parameter_golf_baseline_training_graph_inputs(
            &split_graph,
            &model,
            fixture.input_ids.as_slice(),
            fixture.target_ids.as_slice(),
        )?;
        let banked_inputs = bind_parameter_golf_baseline_training_graph_inputs(
            &banked_graph,
            &model,
            fixture.input_ids.as_slice(),
            fixture.target_ids.as_slice(),
        )?;
        let split_forward = evaluate_graph(split_graph.graph.graph(), &split_inputs)?;
        let banked_forward = evaluate_graph(banked_graph.graph.graph(), &banked_inputs)?;
        let split_loss = split_forward
            .get(&split_graph.loss_tensor_id)
            .ok_or("missing split loss tensor")?
            .as_f32_slice()
            .ok_or("split loss tensor should be dense f32")?[0];
        let banked_loss = banked_forward
            .get(&banked_graph.loss_tensor_id)
            .ok_or("missing banked loss tensor")?
            .as_f32_slice()
            .ok_or("banked loss tensor should be dense f32")?[0];
        assert!(
            (split_loss - banked_loss).abs() < 5e-5,
            "banked graph loss drift exceeded tolerance: split={split_loss} banked={banked_loss}"
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_banked_training_graph_can_force_split_sliced_matrix_execution(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let model = baseline_model()?;
        let banked_descriptor = model.banked_descriptor()?;
        let direct_graph = build_parameter_golf_baseline_training_graph_with_matrix_execution_mode(
            Device::cpu(),
            &banked_descriptor,
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
            ParameterGolfMatrixExecutionMode::DirectBanked,
        )?;
        let split_sliced_graph =
            build_parameter_golf_baseline_training_graph_with_matrix_execution_mode(
                Device::cpu(),
                &banked_descriptor,
                fixture.input_ids.len(),
                fixture.input_ids[0].len(),
                ParameterGolfMatrixExecutionMode::SplitSliced,
            )?;

        let bank_tensor_ids = [
            PARAMETER_GOLF_QO_BANK_NAME,
            PARAMETER_GOLF_KV_BANK_NAME,
            PARAMETER_GOLF_MLP_UP_BANK_NAME,
            PARAMETER_GOLF_MLP_DOWN_BANK_NAME,
        ]
        .into_iter()
        .map(|parameter_id| {
            split_sliced_graph
                .parameter_binding(parameter_id)
                .map(|binding| binding.graph_input_tensor_id)
                .ok_or("missing bank binding")
        })
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;

        let direct_banked_linear_count = direct_graph
            .graph
            .graph()
            .nodes()
            .iter()
            .filter(|node| {
                matches!(
                    node.op(),
                    OpKind::BackendExtension {
                        op: BackendExtensionOp::ParameterGolfBankedLinear { .. }
                    }
                )
            })
            .count();
        let split_sliced_banked_linear_count = split_sliced_graph
            .graph
            .graph()
            .nodes()
            .iter()
            .filter(|node| {
                matches!(
                    node.op(),
                    OpKind::BackendExtension {
                        op: BackendExtensionOp::ParameterGolfBankedLinear { .. }
                    }
                )
            })
            .count();
        let split_sliced_bank_slice_count = split_sliced_graph
            .graph
            .graph()
            .nodes()
            .iter()
            .filter(|node| {
                matches!(node.op(), OpKind::Slice { .. } | OpKind::Select { .. })
                    && node
                        .inputs()
                        .iter()
                        .any(|tensor_id| bank_tensor_ids.contains(tensor_id))
            })
            .count();

        assert!(direct_banked_linear_count > 0);
        assert_eq!(split_sliced_banked_linear_count, 0);
        assert!(split_sliced_bank_slice_count > 0);

        let direct_inputs = bind_parameter_golf_baseline_training_graph_inputs(
            &direct_graph,
            &model,
            fixture.input_ids.as_slice(),
            fixture.target_ids.as_slice(),
        )?;
        let split_sliced_inputs = bind_parameter_golf_baseline_training_graph_inputs(
            &split_sliced_graph,
            &model,
            fixture.input_ids.as_slice(),
            fixture.target_ids.as_slice(),
        )?;
        let direct_forward = evaluate_graph(direct_graph.graph.graph(), &direct_inputs)?;
        let split_sliced_forward =
            evaluate_graph(split_sliced_graph.graph.graph(), &split_sliced_inputs)?;
        let direct_loss = direct_forward
            .get(&direct_graph.loss_tensor_id)
            .ok_or("missing direct loss tensor")?
            .as_f32_slice()
            .ok_or("direct loss tensor should be dense f32")?[0];
        let split_sliced_loss = split_sliced_forward
            .get(&split_sliced_graph.loss_tensor_id)
            .ok_or("missing split-sliced loss tensor")?
            .as_f32_slice()
            .ok_or("split-sliced loss tensor should be dense f32")?[0];
        assert!(
            (direct_loss - split_sliced_loss).abs() < 5e-5,
            "matrix execution mode loss drift exceeded tolerance: direct={direct_loss} split_sliced={split_sliced_loss}"
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_banked_eval_graph_uses_direct_banked_linear_ops(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let model = baseline_model()?;
        let banked_descriptor = model.banked_descriptor()?;
        let split_graph = build_parameter_golf_baseline_eval_graph(
            Device::cpu(),
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;
        let banked_graph = build_parameter_golf_baseline_eval_graph(
            Device::cpu(),
            &banked_descriptor,
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;
        let bank_tensor_ids = [
            PARAMETER_GOLF_QO_BANK_NAME,
            PARAMETER_GOLF_KV_BANK_NAME,
            PARAMETER_GOLF_MLP_UP_BANK_NAME,
            PARAMETER_GOLF_MLP_DOWN_BANK_NAME,
        ]
        .into_iter()
        .map(|parameter_id| {
            banked_graph
                .parameter_binding(parameter_id)
                .map(|binding| binding.graph_input_tensor_id)
                .ok_or("missing bank binding")
        })
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        let banked_linear_count = banked_graph
            .graph
            .nodes()
            .iter()
            .filter(|node| {
                matches!(
                    node.op(),
                    OpKind::BackendExtension {
                        op: BackendExtensionOp::ParameterGolfBankedLinear { .. }
                    }
                )
            })
            .count();
        let bank_slice_count = banked_graph
            .graph
            .nodes()
            .iter()
            .filter(|node| {
                matches!(node.op(), OpKind::Slice { .. } | OpKind::Select { .. })
                    && node
                        .inputs()
                        .iter()
                        .any(|tensor_id| bank_tensor_ids.contains(tensor_id))
            })
            .count();

        assert!(banked_linear_count > 0);
        assert_eq!(bank_slice_count, 0);

        let split_inputs = bind_parameter_golf_baseline_eval_graph_inputs(
            &split_graph,
            &model,
            fixture.input_ids.as_slice(),
            fixture.target_ids.as_slice(),
        )?;
        let banked_inputs = bind_parameter_golf_baseline_eval_graph_inputs(
            &banked_graph,
            &model,
            fixture.input_ids.as_slice(),
            fixture.target_ids.as_slice(),
        )?;
        let split_forward = evaluate_graph(&split_graph.graph, &split_inputs)?;
        let banked_forward = evaluate_graph(&banked_graph.graph, &banked_inputs)?;
        let split_loss = split_forward
            .get(&split_graph.loss_tensor_id)
            .ok_or("missing split eval loss tensor")?
            .as_f32_slice()
            .ok_or("split eval loss tensor should be dense f32")?[0];
        let banked_loss = banked_forward
            .get(&banked_graph.loss_tensor_id)
            .ok_or("missing banked eval loss tensor")?
            .as_f32_slice()
            .ok_or("banked eval loss tensor should be dense f32")?[0];
        assert!(
            (split_loss - banked_loss).abs() < 5e-5,
            "banked eval graph loss drift exceeded tolerance: split={split_loss} banked={banked_loss}"
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_cuda_banked_graph_feeds_direct_banked_linears_from_bf16_inputs(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let model = baseline_model()?;
        let banked_descriptor = model.banked_descriptor()?;
        let cuda = Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0")));
        let training_graph = build_parameter_golf_baseline_training_graph(
            cuda.clone(),
            &banked_descriptor,
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;
        let eval_graph = build_parameter_golf_baseline_eval_graph(
            cuda,
            &banked_descriptor,
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;

        let training_banked_linear_input_dtypes = training_graph
            .graph
            .graph()
            .nodes()
            .iter()
            .filter_map(|node| match node.op() {
                OpKind::BackendExtension {
                    op: BackendExtensionOp::ParameterGolfBankedLinear { .. },
                } => Some(
                    node.inputs()
                        .first()
                        .and_then(|tensor_id| training_graph.graph.graph().node(*tensor_id))
                        .map(|input| input.tensor().spec().dtype()),
                ),
                _ => None,
            })
            .collect::<Vec<_>>();
        let eval_banked_linear_input_dtypes = eval_graph
            .graph
            .nodes()
            .iter()
            .filter_map(|node| match node.op() {
                OpKind::BackendExtension {
                    op: BackendExtensionOp::ParameterGolfBankedLinear { .. },
                } => Some(
                    node.inputs()
                        .first()
                        .and_then(|tensor_id| eval_graph.graph.node(*tensor_id))
                        .map(|input| input.tensor().spec().dtype()),
                ),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(!training_banked_linear_input_dtypes.is_empty());
        assert!(!eval_banked_linear_input_dtypes.is_empty());
        assert!(training_banked_linear_input_dtypes
            .iter()
            .all(|dtype| *dtype == Some(DType::BF16)));
        assert!(eval_banked_linear_input_dtypes
            .iter()
            .all(|dtype| *dtype == Some(DType::BF16)));
        Ok(())
    }

    #[test]
    fn parameter_golf_baseline_eval_graph_matches_training_graph_loss(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let model = baseline_model()?;
        let training_graph = build_parameter_golf_baseline_training_graph(
            Device::cpu(),
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;
        let eval_graph = build_parameter_golf_baseline_eval_graph(
            Device::cpu(),
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;

        let training_inputs = bind_parameter_golf_baseline_training_graph_inputs(
            &training_graph,
            &model,
            fixture.input_ids.as_slice(),
            fixture.target_ids.as_slice(),
        )?;
        let eval_inputs = bind_parameter_golf_baseline_eval_graph_inputs(
            &eval_graph,
            &model,
            fixture.input_ids.as_slice(),
            fixture.target_ids.as_slice(),
        )?;
        let training_forward = evaluate_graph(training_graph.graph.graph(), &training_inputs)?;
        let eval_forward = evaluate_graph(&eval_graph.graph, &eval_inputs)?;
        let training_loss = training_forward
            .get(&training_graph.loss_tensor_id)
            .ok_or("missing training loss tensor")?
            .as_f32_slice()
            .ok_or("training loss tensor should be dense f32")?[0];
        let eval_loss = eval_forward
            .get(&eval_graph.loss_tensor_id)
            .ok_or("missing eval loss tensor")?
            .as_f32_slice()
            .ok_or("eval loss tensor should be dense f32")?[0];
        assert!(
            (training_loss - eval_loss).abs() < 5e-5,
            "eval-graph loss drift exceeded tolerance: training={training_loss} eval={eval_loss}"
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_baseline_eval_graph_binds_token_ids_as_i32_tensors(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let model = baseline_model()?;
        let graph = build_parameter_golf_baseline_eval_graph(
            Device::cpu(),
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;

        let inputs = bind_parameter_golf_baseline_eval_graph_inputs(
            &graph,
            &model,
            fixture.input_ids.as_slice(),
            fixture.target_ids.as_slice(),
        )?;

        assert!(matches!(
            inputs.get(&graph.input_token_ids_tensor_id),
            Some(TensorData::I32(values))
                if values
                    == &fixture
                        .input_ids
                        .iter()
                        .flat_map(|row| row.iter().map(|token_id| *token_id as i32))
                        .collect::<Vec<_>>()
        ));
        assert!(matches!(
            inputs.get(&graph.target_ids_tensor_id),
            Some(TensorData::I32(values))
                if values
                    == &fixture
                        .target_ids
                        .iter()
                        .flat_map(|row| row.iter().map(|token_id| *token_id as i32))
                        .collect::<Vec<_>>()
        ));
        Ok(())
    }

    #[test]
    fn parameter_golf_cuda_training_and_eval_graphs_lower_linear_and_attention_compute_to_bf16(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = load_baseline_fixture();
        let model = baseline_model()?;
        let cuda = Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0")));
        let training_graph = build_parameter_golf_baseline_training_graph(
            cuda.clone(),
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;
        let eval_graph = build_parameter_golf_baseline_eval_graph(
            cuda,
            model.descriptor(),
            fixture.input_ids.len(),
            fixture.input_ids[0].len(),
        )?;

        let training_bf16_weight_matmul_count = training_graph
            .graph
            .graph()
            .nodes()
            .iter()
            .filter(|node| {
                matches!(node.op(), OpKind::Matmul)
                    && node.tensor().spec().dtype() == DType::F32
                    && node
                        .inputs()
                        .get(1)
                        .and_then(|tensor_id| training_graph.graph.graph().node(*tensor_id))
                        .is_some_and(|input| input.tensor().spec().dtype() == DType::BF16)
            })
            .count();
        let training_bf16_cast_count = training_graph
            .graph
            .graph()
            .nodes()
            .iter()
            .filter(|node| matches!(node.op(), OpKind::Cast { dtype: DType::BF16 }))
            .count();
        let eval_bf16_weight_matmul_count = eval_graph
            .graph
            .nodes()
            .iter()
            .filter(|node| {
                matches!(node.op(), OpKind::Matmul)
                    && node.tensor().spec().dtype() == DType::F32
                    && node
                        .inputs()
                        .get(1)
                        .and_then(|tensor_id| eval_graph.graph.node(*tensor_id))
                        .is_some_and(|input| input.tensor().spec().dtype() == DType::BF16)
            })
            .count();
        let eval_bf16_cast_count = eval_graph
            .graph
            .nodes()
            .iter()
            .filter(|node| matches!(node.op(), OpKind::Cast { dtype: DType::BF16 }))
            .count();

        assert!(training_bf16_weight_matmul_count > 0);
        assert!(training_bf16_cast_count > 0);
        assert!(eval_bf16_weight_matmul_count > 0);
        assert!(eval_bf16_cast_count > 0);
        Ok(())
    }
}
