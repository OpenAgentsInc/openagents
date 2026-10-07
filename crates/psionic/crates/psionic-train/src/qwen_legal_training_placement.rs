use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_core::QuantizationMode;

/// Schema version emitted by the Qwen legal placement planner.
pub const QWEN_LEGAL_TRAINING_PLACEMENT_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_training_placement.v1";
/// Dense Qwen3.6-27B model id.
pub const QWEN36_27B_PLACEMENT_MODEL_ID: &str = "Qwen/Qwen3.6-27B";
/// MoE Qwen3.6-35B-A3B model id.
pub const QWEN36_35B_A3B_PLACEMENT_MODEL_ID: &str = "Qwen/Qwen3.6-35B-A3B";

const DENSE_27B_WEIGHT_BYTES_BF16: u64 = 55_562_855_904;
const MOE_35B_ACTIVE_WEIGHT_BYTES_BF16: u64 = 8_600_000_000;
const MOE_35B_CACHED_EXPERT_WEIGHT_BYTES_BF16: u64 = 76_000_000_000;
const DENSE_27B_HIDDEN_SIZE: u64 = 5_120;
const DENSE_27B_LAYER_COUNT: u32 = 64;
const MOE_35B_LAYER_COUNT: u32 = 48;
const QWEN36_LORA_TARGET_PARAM_COUNT: u64 = 4_124_016_640;
const ADAPTER_F32_BYTES_PER_PARAM: u64 = 4;

/// Adapter mode requested for a Qwen legal training job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalTrainingAdapterMode {
    /// LoRA over a BF16 or int8 frozen base.
    Lora,
    /// QLoRA over an admitted quantized frozen base.
    Qlora,
}

/// High-level execution topology requested by the operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalPlacementTopology {
    /// One node owns the full trainable step.
    SingleNode,
    /// Multiple Pylons split layers and aggregate adapter updates.
    MultiPylon,
}

/// Placement status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalPlacementStatus {
    /// The request fits under the declared planner assumptions.
    Admitted,
    /// The request cannot run under the declared planner assumptions.
    Refused,
}

/// Refusal code emitted by the placement planner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalPlacementRefusalCode {
    /// Model id is outside the current Qwen legal lane.
    UnsupportedModel,
    /// Quantization does not match the adapter mode.
    UnsupportedQuantization,
    /// Declared memory cannot hold the plan.
    InsufficientMemory,
    /// Multi-node topology is missing enough trusted nodes.
    TopologyMismatch,
    /// Router or gate training was requested on the MoE model.
    RouterTrainingRefused,
}

/// Model profile used by the placement planner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalPlacementModelProfile {
    /// Public model id.
    pub model_id: String,
    /// Dense or MoE profile label.
    pub model_class: String,
    /// Transformer layer count used for layer placement.
    pub layer_count: u32,
    /// BF16 frozen-base bytes before quantization or offload.
    pub bf16_weight_bytes: u64,
    /// Whether router/gate parameters exist.
    pub has_router_or_gate: bool,
    /// Whether router/gate parameters are frozen in this plan.
    pub router_or_gate_frozen: bool,
}

/// One Pylon or local node capability report accepted by the planner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalPylonNodeCapability {
    /// Stable node id.
    pub node_id: String,
    /// Backend label, for example `metal`, `cuda`, `cpu`, or `loopback`.
    pub backend_label: String,
    /// Total host memory available to the trainer.
    pub host_memory_bytes: u64,
    /// Accelerator memory available to the trainer.
    pub accelerator_memory_bytes: u64,
    /// Whether the node already has the model cached.
    pub model_cached: bool,
    /// Job kinds this node accepts.
    pub allowed_job_types: Vec<String>,
    /// Trust state reported by the scheduler.
    pub trust_state: String,
    /// Payout target reference. This is a reference, not wallet secret material.
    pub payment_target_ref: String,
}

impl QwenLegalPylonNodeCapability {
    /// Returns the planner-visible memory pool for a train placement.
    #[must_use]
    pub const fn effective_memory_bytes(&self) -> u64 {
        if self.accelerator_memory_bytes > 0 {
            self.accelerator_memory_bytes
        } else {
            self.host_memory_bytes
        }
    }

    fn is_trusted_for_training(&self) -> bool {
        self.trust_state == "trusted"
            && self
                .allowed_job_types
                .iter()
                .any(|job| job == "sft_train_shard" || job == "qwen_legal_train")
    }
}

/// Placement request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalTrainingPlacementRequest {
    /// Stable planner request id.
    pub request_id: String,
    /// Model id.
    pub model_id: String,
    /// Adapter mode.
    pub adapter_mode: QwenLegalTrainingAdapterMode,
    /// Base quantization.
    pub base_quantization: QuantizationMode,
    /// Sequence length.
    pub sequence_len: u32,
    /// Per-step micro-batch size.
    pub micro_batch_size: u32,
    /// Gradient accumulation steps.
    pub gradient_accumulation_steps: u32,
    /// Requested topology.
    pub topology: QwenLegalPlacementTopology,
    /// Whether router/gate parameters may be trained.
    pub train_router_or_gate: bool,
    /// Requested LoRA target modules.
    pub target_modules: Vec<String>,
    /// Candidate nodes.
    pub nodes: Vec<QwenLegalPylonNodeCapability>,
}

/// Memory estimate retained in the placement plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalPlacementMemoryEstimate {
    /// Frozen base bytes after quantization.
    pub base_resident_bytes: u64,
    /// Adapter parameter bytes.
    pub adapter_bytes: u64,
    /// Optimizer state bytes.
    pub optimizer_state_bytes: u64,
    /// Activation bytes retained under checkpointing assumptions.
    pub activation_checkpoint_bytes: u64,
    /// Total bytes the plan needs across all assigned nodes.
    pub total_required_bytes: u64,
    /// Bytes needed on the largest single assigned node.
    pub max_required_bytes_per_node: u64,
    /// Whether activation checkpointing is required.
    pub activation_checkpointing: bool,
    /// Whether frozen-base offload is required.
    pub frozen_base_offload: bool,
}

/// One node assignment inside a placement plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalPlacementNodeAssignment {
    /// Assigned node id.
    pub node_id: String,
    /// Backend label.
    pub backend_label: String,
    /// Inclusive first layer index.
    pub first_layer: u32,
    /// Inclusive last layer index.
    pub last_layer: u32,
    /// Required bytes on this node.
    pub required_bytes: u64,
    /// Available bytes on this node.
    pub available_bytes: u64,
    /// Whether the model was already cached on this node.
    pub model_cached: bool,
    /// Payout target reference for later settlement evidence.
    pub payment_target_ref: String,
}

/// Scheduler-facing fact extracted from an admitted placement plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalSchedulerPlacementFact {
    /// Request id.
    pub request_id: String,
    /// Plan digest the scheduler must preserve in the job envelope.
    pub plan_digest: String,
    /// Assigned node id.
    pub node_id: String,
    /// Job kind understood by Qwen legal Pylon workers.
    pub job_type: String,
    /// Model id.
    pub model_id: String,
    /// Adapter mode.
    pub adapter_mode: QwenLegalTrainingAdapterMode,
    /// Base quantization.
    pub base_quantization: QuantizationMode,
    /// Inclusive first layer index.
    pub first_layer: u32,
    /// Inclusive last layer index.
    pub last_layer: u32,
    /// Required bytes on this node.
    pub required_bytes: u64,
    /// Available bytes on this node.
    pub available_bytes: u64,
    /// Backend label.
    pub backend_label: String,
    /// Payout target reference.
    pub payment_target_ref: String,
    /// Whether frozen base weights stay frozen.
    pub frozen_base_weights: bool,
    /// Whether router/gate parameters stay frozen.
    pub router_or_gate_frozen: bool,
}

/// Refusal detail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalPlacementRefusal {
    /// Machine-readable refusal code.
    pub code: QwenLegalPlacementRefusalCode,
    /// Plain reason.
    pub reason: String,
}

/// Placement plan emitted for Qwen legal training jobs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalTrainingPlacementPlan {
    /// Schema version.
    pub schema_version: String,
    /// Request id.
    pub request_id: String,
    /// Placement status.
    pub status: QwenLegalPlacementStatus,
    /// Optional refusal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<QwenLegalPlacementRefusal>,
    /// Model profile.
    pub model_profile: QwenLegalPlacementModelProfile,
    /// Adapter mode.
    pub adapter_mode: QwenLegalTrainingAdapterMode,
    /// Base quantization.
    pub base_quantization: QuantizationMode,
    /// Requested sequence length.
    pub sequence_len: u32,
    /// Per-step micro-batch size.
    pub micro_batch_size: u32,
    /// Gradient accumulation steps.
    pub gradient_accumulation_steps: u32,
    /// Requested topology.
    pub topology: QwenLegalPlacementTopology,
    /// Requested LoRA target modules.
    pub target_modules: Vec<String>,
    /// Precision policy label.
    pub precision_policy: String,
    /// Memory estimate.
    pub memory: QwenLegalPlacementMemoryEstimate,
    /// Assigned nodes.
    pub node_assignments: Vec<QwenLegalPlacementNodeAssignment>,
    /// Frozen base guarantee.
    pub frozen_base_weights: bool,
    /// Router/gate guarantee.
    pub router_or_gate_frozen: bool,
    /// Optimizer label.
    pub optimizer: String,
    /// Stable digest over the plan.
    pub plan_digest: String,
}

impl QwenLegalTrainingPlacementPlan {
    /// Facts a scheduler can copy into Pylon training jobs after admission.
    #[must_use]
    pub fn scheduler_facts(&self) -> Vec<QwenLegalSchedulerPlacementFact> {
        if self.status != QwenLegalPlacementStatus::Admitted {
            return Vec::new();
        }
        self.node_assignments
            .iter()
            .map(|assignment| QwenLegalSchedulerPlacementFact {
                request_id: self.request_id.clone(),
                plan_digest: self.plan_digest.clone(),
                node_id: assignment.node_id.clone(),
                job_type: String::from("qwen_legal_train"),
                model_id: self.model_profile.model_id.clone(),
                adapter_mode: self.adapter_mode,
                base_quantization: self.base_quantization,
                first_layer: assignment.first_layer,
                last_layer: assignment.last_layer,
                required_bytes: assignment.required_bytes,
                available_bytes: assignment.available_bytes,
                backend_label: assignment.backend_label.clone(),
                payment_target_ref: assignment.payment_target_ref.clone(),
                frozen_base_weights: self.frozen_base_weights,
                router_or_gate_frozen: self.router_or_gate_frozen,
            })
            .collect()
    }

    fn refused(
        request: &QwenLegalTrainingPlacementRequest,
        profile: QwenLegalPlacementModelProfile,
        memory: QwenLegalPlacementMemoryEstimate,
        code: QwenLegalPlacementRefusalCode,
        reason: impl Into<String>,
    ) -> Self {
        let mut plan = Self {
            schema_version: String::from(QWEN_LEGAL_TRAINING_PLACEMENT_SCHEMA_VERSION),
            request_id: request.request_id.clone(),
            status: QwenLegalPlacementStatus::Refused,
            refusal: Some(QwenLegalPlacementRefusal {
                code,
                reason: reason.into(),
            }),
            model_profile: profile,
            adapter_mode: request.adapter_mode,
            base_quantization: request.base_quantization,
            sequence_len: request.sequence_len,
            micro_batch_size: request.micro_batch_size,
            gradient_accumulation_steps: request.gradient_accumulation_steps,
            topology: request.topology,
            target_modules: request.target_modules.clone(),
            precision_policy: precision_policy(request.adapter_mode, request.base_quantization),
            memory,
            node_assignments: Vec::new(),
            frozen_base_weights: true,
            router_or_gate_frozen: !request.train_router_or_gate,
            optimizer: String::from("adamw"),
            plan_digest: String::new(),
        };
        plan.plan_digest = stable_json_digest(b"psionic_qwen_legal_training_placement|", &plan);
        plan
    }

    fn admitted(
        request: &QwenLegalTrainingPlacementRequest,
        profile: QwenLegalPlacementModelProfile,
        memory: QwenLegalPlacementMemoryEstimate,
        node_assignments: Vec<QwenLegalPlacementNodeAssignment>,
    ) -> Self {
        let mut plan = Self {
            schema_version: String::from(QWEN_LEGAL_TRAINING_PLACEMENT_SCHEMA_VERSION),
            request_id: request.request_id.clone(),
            status: QwenLegalPlacementStatus::Admitted,
            refusal: None,
            model_profile: profile,
            adapter_mode: request.adapter_mode,
            base_quantization: request.base_quantization,
            sequence_len: request.sequence_len,
            micro_batch_size: request.micro_batch_size,
            gradient_accumulation_steps: request.gradient_accumulation_steps,
            topology: request.topology,
            target_modules: request.target_modules.clone(),
            precision_policy: precision_policy(request.adapter_mode, request.base_quantization),
            memory,
            node_assignments,
            frozen_base_weights: true,
            router_or_gate_frozen: !request.train_router_or_gate,
            optimizer: String::from("adamw"),
            plan_digest: String::new(),
        };
        plan.plan_digest = stable_json_digest(b"psionic_qwen_legal_training_placement|", &plan);
        plan
    }
}

/// Error returned by malformed placement requests.
#[derive(Debug, Error)]
pub enum QwenLegalTrainingPlacementError {
    /// Request fields are invalid before planning.
    #[error("invalid Qwen legal placement request: {detail}")]
    InvalidRequest { detail: String },
}

/// Builds a deterministic placement plan for one Qwen legal training request.
pub fn plan_qwen_legal_training_placement(
    request: &QwenLegalTrainingPlacementRequest,
) -> Result<QwenLegalTrainingPlacementPlan, QwenLegalTrainingPlacementError> {
    validate_request(request)?;
    let profile = match model_profile(request.model_id.as_str(), request.train_router_or_gate) {
        Some(profile) => profile,
        None => {
            return Ok(QwenLegalTrainingPlacementPlan::refused(
                request,
                fallback_profile(request.model_id.as_str()),
                zero_memory_estimate(),
                QwenLegalPlacementRefusalCode::UnsupportedModel,
                format!("unsupported Qwen legal model `{}`", request.model_id),
            ));
        }
    };
    let memory = memory_estimate(request, &profile);
    if !quantization_allowed(request.adapter_mode, request.base_quantization) {
        return Ok(QwenLegalTrainingPlacementPlan::refused(
            request,
            profile,
            memory,
            QwenLegalPlacementRefusalCode::UnsupportedQuantization,
            format!(
                "adapter mode `{:?}` does not support base quantization `{}`",
                request.adapter_mode,
                request.base_quantization.label()
            ),
        ));
    }
    if profile.has_router_or_gate && request.train_router_or_gate {
        return Ok(QwenLegalTrainingPlacementPlan::refused(
            request,
            profile,
            memory,
            QwenLegalPlacementRefusalCode::RouterTrainingRefused,
            "Qwen3.6-35B-A3B router/gate training is frozen in this lane",
        ));
    }

    let trusted_nodes = request
        .nodes
        .iter()
        .filter(|node| node.is_trusted_for_training())
        .cloned()
        .collect::<Vec<_>>();
    if trusted_nodes.is_empty() {
        return Ok(QwenLegalTrainingPlacementPlan::refused(
            request,
            profile,
            memory,
            QwenLegalPlacementRefusalCode::TopologyMismatch,
            "no trusted training-capable Pylon nodes were supplied",
        ));
    }

    let assignments = match request.topology {
        QwenLegalPlacementTopology::SingleNode => {
            single_node_assignment(&trusted_nodes, &memory, &profile)
        }
        QwenLegalPlacementTopology::MultiPylon => {
            multi_pylon_assignments(&trusted_nodes, &memory, &profile)
        }
    };
    let Some(assignments) = assignments else {
        return Ok(QwenLegalTrainingPlacementPlan::refused(
            request,
            profile,
            memory,
            QwenLegalPlacementRefusalCode::InsufficientMemory,
            "declared memory cannot hold the requested Qwen legal training placement",
        ));
    };

    Ok(QwenLegalTrainingPlacementPlan::admitted(
        request,
        profile,
        memory,
        assignments,
    ))
}

fn validate_request(
    request: &QwenLegalTrainingPlacementRequest,
) -> Result<(), QwenLegalTrainingPlacementError> {
    if request.request_id.trim().is_empty() {
        return invalid_request("request_id must be present");
    }
    if request.model_id.trim().is_empty() {
        return invalid_request("model_id must be present");
    }
    if request.sequence_len == 0
        || request.micro_batch_size == 0
        || request.gradient_accumulation_steps == 0
    {
        return invalid_request(
            "sequence_len, micro_batch_size, and gradient_accumulation_steps must be non-zero",
        );
    }
    if request.target_modules.is_empty() {
        return invalid_request("target_modules must not be empty");
    }
    Ok(())
}

fn invalid_request<T>(detail: impl Into<String>) -> Result<T, QwenLegalTrainingPlacementError> {
    Err(QwenLegalTrainingPlacementError::InvalidRequest {
        detail: detail.into(),
    })
}

fn model_profile(
    model_id: &str,
    train_router_or_gate: bool,
) -> Option<QwenLegalPlacementModelProfile> {
    match model_id {
        QWEN36_27B_PLACEMENT_MODEL_ID | "Qwen3.6-27B" => Some(QwenLegalPlacementModelProfile {
            model_id: String::from(QWEN36_27B_PLACEMENT_MODEL_ID),
            model_class: String::from("dense_27b"),
            layer_count: DENSE_27B_LAYER_COUNT,
            bf16_weight_bytes: DENSE_27B_WEIGHT_BYTES_BF16,
            has_router_or_gate: false,
            router_or_gate_frozen: true,
        }),
        QWEN36_35B_A3B_PLACEMENT_MODEL_ID | "Qwen3.6-35B-A3B" => {
            Some(QwenLegalPlacementModelProfile {
                model_id: String::from(QWEN36_35B_A3B_PLACEMENT_MODEL_ID),
                model_class: String::from("moe_35b_a3b"),
                layer_count: MOE_35B_LAYER_COUNT,
                bf16_weight_bytes: MOE_35B_ACTIVE_WEIGHT_BYTES_BF16
                    + MOE_35B_CACHED_EXPERT_WEIGHT_BYTES_BF16,
                has_router_or_gate: true,
                router_or_gate_frozen: !train_router_or_gate,
            })
        }
        _ => None,
    }
}

fn fallback_profile(model_id: &str) -> QwenLegalPlacementModelProfile {
    QwenLegalPlacementModelProfile {
        model_id: String::from(model_id),
        model_class: String::from("unsupported"),
        layer_count: 0,
        bf16_weight_bytes: 0,
        has_router_or_gate: false,
        router_or_gate_frozen: true,
    }
}

fn memory_estimate(
    request: &QwenLegalTrainingPlacementRequest,
    profile: &QwenLegalPlacementModelProfile,
) -> QwenLegalPlacementMemoryEstimate {
    let base_resident_bytes =
        quantized_base_bytes(profile.bf16_weight_bytes, request.base_quantization);
    let adapter_bytes = QWEN36_LORA_TARGET_PARAM_COUNT
        .saturating_mul(ADAPTER_F32_BYTES_PER_PARAM)
        .saturating_div(8)
        .max(1);
    let optimizer_state_bytes = adapter_bytes.saturating_mul(2);
    let activation_checkpoint_bytes = DENSE_27B_HIDDEN_SIZE
        .saturating_mul(u64::from(request.sequence_len))
        .saturating_mul(u64::from(request.micro_batch_size))
        .saturating_mul(2)
        .saturating_mul(4);
    let total_required_bytes = base_resident_bytes
        .saturating_add(adapter_bytes)
        .saturating_add(optimizer_state_bytes)
        .saturating_add(activation_checkpoint_bytes);
    let max_required_bytes_per_node = match request.topology {
        QwenLegalPlacementTopology::SingleNode => total_required_bytes,
        QwenLegalPlacementTopology::MultiPylon => {
            let nodes = u64::try_from(request.nodes.len().max(1)).unwrap_or(1);
            base_resident_bytes
                .saturating_div(nodes)
                .saturating_add(adapter_bytes)
                .saturating_add(optimizer_state_bytes)
                .saturating_add(activation_checkpoint_bytes)
        }
    };
    QwenLegalPlacementMemoryEstimate {
        base_resident_bytes,
        adapter_bytes,
        optimizer_state_bytes,
        activation_checkpoint_bytes,
        total_required_bytes,
        max_required_bytes_per_node,
        activation_checkpointing: true,
        frozen_base_offload: request.topology == QwenLegalPlacementTopology::MultiPylon,
    }
}

fn zero_memory_estimate() -> QwenLegalPlacementMemoryEstimate {
    QwenLegalPlacementMemoryEstimate {
        base_resident_bytes: 0,
        adapter_bytes: 0,
        optimizer_state_bytes: 0,
        activation_checkpoint_bytes: 0,
        total_required_bytes: 0,
        max_required_bytes_per_node: 0,
        activation_checkpointing: false,
        frozen_base_offload: false,
    }
}

fn quantized_base_bytes(bytes: u64, mode: QuantizationMode) -> u64 {
    match mode {
        QuantizationMode::None => bytes,
        QuantizationMode::Int8Symmetric | QuantizationMode::GgmlQ8_0 => bytes.saturating_div(2),
        QuantizationMode::GgmlQ4K
        | QuantizationMode::GgmlQ4_0
        | QuantizationMode::GgmlQ4_1
        | QuantizationMode::GgmlMxfp4 => bytes.saturating_div(4),
        QuantizationMode::GgmlQ5_0 | QuantizationMode::GgmlQ5K => bytes.saturating_mul(5) / 16,
        QuantizationMode::GgmlQ6K => bytes.saturating_mul(3) / 8,
        // These modes are refused below; keep their refusal receipts conservatively BF16-sized.
        QuantizationMode::GgmlQ3K
        | QuantizationMode::GgmlIq3S
        | QuantizationMode::GgmlIq4Xs => bytes,
    }
}

fn quantization_allowed(
    mode: QwenLegalTrainingAdapterMode,
    quantization: QuantizationMode,
) -> bool {
    match mode {
        QwenLegalTrainingAdapterMode::Lora => {
            matches!(
                quantization,
                QuantizationMode::None | QuantizationMode::Int8Symmetric
            )
        }
        QwenLegalTrainingAdapterMode::Qlora => matches!(
            quantization,
            QuantizationMode::GgmlQ4K
                | QuantizationMode::GgmlQ8_0
                | QuantizationMode::Int8Symmetric
        ),
    }
}

fn precision_policy(mode: QwenLegalTrainingAdapterMode, quantization: QuantizationMode) -> String {
    match (mode, quantization) {
        (QwenLegalTrainingAdapterMode::Lora, QuantizationMode::None) => {
            String::from("bf16_frozen_base_f32_adapter")
        }
        (QwenLegalTrainingAdapterMode::Lora, QuantizationMode::Int8Symmetric) => {
            String::from("int8_frozen_base_f32_adapter")
        }
        (QwenLegalTrainingAdapterMode::Qlora, QuantizationMode::GgmlQ4K) => {
            String::from("q4k_frozen_base_f32_adapter")
        }
        (QwenLegalTrainingAdapterMode::Qlora, QuantizationMode::GgmlQ8_0) => {
            String::from("q8_frozen_base_f32_adapter")
        }
        (QwenLegalTrainingAdapterMode::Qlora, QuantizationMode::Int8Symmetric) => {
            String::from("int8_frozen_base_f32_adapter")
        }
        _ => String::from("unsupported"),
    }
}

fn single_node_assignment(
    nodes: &[QwenLegalPylonNodeCapability],
    memory: &QwenLegalPlacementMemoryEstimate,
    profile: &QwenLegalPlacementModelProfile,
) -> Option<Vec<QwenLegalPlacementNodeAssignment>> {
    let node = nodes
        .iter()
        .find(|node| node.effective_memory_bytes() >= memory.max_required_bytes_per_node)?;
    Some(vec![QwenLegalPlacementNodeAssignment {
        node_id: node.node_id.clone(),
        backend_label: node.backend_label.clone(),
        first_layer: 0,
        last_layer: profile.layer_count.saturating_sub(1),
        required_bytes: memory.max_required_bytes_per_node,
        available_bytes: node.effective_memory_bytes(),
        model_cached: node.model_cached,
        payment_target_ref: node.payment_target_ref.clone(),
    }])
}

fn multi_pylon_assignments(
    nodes: &[QwenLegalPylonNodeCapability],
    memory: &QwenLegalPlacementMemoryEstimate,
    profile: &QwenLegalPlacementModelProfile,
) -> Option<Vec<QwenLegalPlacementNodeAssignment>> {
    if nodes.len() < 2 {
        return None;
    }
    let node_count = u32::try_from(nodes.len()).ok()?;
    let layers_per_node = profile.layer_count.div_ceil(node_count).max(1);
    let mut assignments = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        let first_layer = u32::try_from(index).ok()?.saturating_mul(layers_per_node);
        if first_layer >= profile.layer_count {
            break;
        }
        let last_layer = first_layer
            .saturating_add(layers_per_node)
            .saturating_sub(1)
            .min(profile.layer_count.saturating_sub(1));
        if node.effective_memory_bytes() < memory.max_required_bytes_per_node {
            return None;
        }
        assignments.push(QwenLegalPlacementNodeAssignment {
            node_id: node.node_id.clone(),
            backend_label: node.backend_label.clone(),
            first_layer,
            last_layer,
            required_bytes: memory.max_required_bytes_per_node,
            available_bytes: node.effective_memory_bytes(),
            model_cached: node.model_cached,
            payment_target_ref: node.payment_target_ref.clone(),
        });
    }
    (assignments.len() >= 2).then_some(assignments)
}

fn stable_json_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    if let Ok(bytes) = serde_json::to_vec(value) {
        hasher.update(bytes);
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planner_admits_concrete_local_dense_27b_lora() {
        let request = dense_request(
            QwenLegalPlacementTopology::SingleNode,
            vec![node("pylon.local.metal.01", 96 * gib(), 96 * gib())],
        );
        let plan = plan_qwen_legal_training_placement(&request).expect("plan");

        assert_eq!(plan.status, QwenLegalPlacementStatus::Admitted);
        assert_eq!(plan.model_profile.model_class, "dense_27b");
        assert_eq!(plan.node_assignments.len(), 1);
        assert!(plan.memory.activation_checkpointing);
        assert!(plan.frozen_base_weights);
        assert_eq!(plan.gradient_accumulation_steps, 8);
        assert_eq!(plan.scheduler_facts().len(), 1);
        assert_eq!(plan.scheduler_facts()[0].job_type, "qwen_legal_train");
        assert_eq!(
            plan.plan_digest,
            stable_json_digest(
                b"psionic_qwen_legal_training_placement|",
                &without_digest(&plan)
            )
        );
    }

    #[test]
    fn planner_refuses_under_memory_dense_27b() {
        let request = dense_request(
            QwenLegalPlacementTopology::SingleNode,
            vec![node("pylon.local.too-small", 16 * gib(), 16 * gib())],
        );
        let plan = plan_qwen_legal_training_placement(&request).expect("plan");

        assert_eq!(plan.status, QwenLegalPlacementStatus::Refused);
        assert!(plan.scheduler_facts().is_empty());
        assert_eq!(
            plan.refusal.as_ref().map(|value| value.code),
            Some(QwenLegalPlacementRefusalCode::InsufficientMemory)
        );
    }

    #[test]
    fn planner_admits_multi_pylon_dense_27b_qlora() {
        let mut request = dense_request(
            QwenLegalPlacementTopology::MultiPylon,
            vec![
                node("pylon.tailnet.cuda.01", 48 * gib(), 48 * gib()),
                node("pylon.tailnet.metal.01", 48 * gib(), 48 * gib()),
            ],
        );
        request.adapter_mode = QwenLegalTrainingAdapterMode::Qlora;
        request.base_quantization = QuantizationMode::GgmlQ4K;
        let plan = plan_qwen_legal_training_placement(&request).expect("plan");

        assert_eq!(plan.status, QwenLegalPlacementStatus::Admitted);
        assert_eq!(plan.node_assignments.len(), 2);
        assert!(plan.memory.frozen_base_offload);
        assert_eq!(plan.precision_policy, "q4k_frozen_base_f32_adapter");
    }

    #[test]
    fn planner_refuses_moe_router_training() {
        let mut request = dense_request(
            QwenLegalPlacementTopology::SingleNode,
            vec![node("pylon.local.moe", 128 * gib(), 128 * gib())],
        );
        request.model_id = String::from(QWEN36_35B_A3B_PLACEMENT_MODEL_ID);
        request.train_router_or_gate = true;
        request.target_modules.push(String::from("router"));
        let plan = plan_qwen_legal_training_placement(&request).expect("plan");

        assert_eq!(plan.status, QwenLegalPlacementStatus::Refused);
        assert_eq!(
            plan.refusal.as_ref().map(|value| value.code),
            Some(QwenLegalPlacementRefusalCode::RouterTrainingRefused)
        );
    }

    #[test]
    fn planner_refuses_topology_without_trusted_training_nodes() {
        let mut request = dense_request(
            QwenLegalPlacementTopology::MultiPylon,
            vec![node("pylon.untrusted.01", 96 * gib(), 96 * gib())],
        );
        request.nodes[0].trust_state = String::from("untrusted");
        let plan = plan_qwen_legal_training_placement(&request).expect("plan");

        assert_eq!(plan.status, QwenLegalPlacementStatus::Refused);
        assert_eq!(
            plan.refusal.as_ref().map(|value| value.code),
            Some(QwenLegalPlacementRefusalCode::TopologyMismatch)
        );
    }

    #[test]
    fn planner_refuses_unsupported_quantization_for_lora() {
        let mut request = dense_request(
            QwenLegalPlacementTopology::SingleNode,
            vec![node("pylon.local.metal.01", 96 * gib(), 96 * gib())],
        );
        request.base_quantization = QuantizationMode::GgmlQ4K;
        let plan = plan_qwen_legal_training_placement(&request).expect("plan");

        assert_eq!(plan.status, QwenLegalPlacementStatus::Refused);
        assert_eq!(
            plan.refusal.as_ref().map(|value| value.code),
            Some(QwenLegalPlacementRefusalCode::UnsupportedQuantization)
        );
    }

    fn dense_request(
        topology: QwenLegalPlacementTopology,
        nodes: Vec<QwenLegalPylonNodeCapability>,
    ) -> QwenLegalTrainingPlacementRequest {
        QwenLegalTrainingPlacementRequest {
            request_id: String::from("qwen-legal-placement-test"),
            model_id: String::from(QWEN36_27B_PLACEMENT_MODEL_ID),
            adapter_mode: QwenLegalTrainingAdapterMode::Lora,
            base_quantization: QuantizationMode::Int8Symmetric,
            sequence_len: 2_048,
            micro_batch_size: 1,
            gradient_accumulation_steps: 8,
            topology,
            train_router_or_gate: false,
            target_modules: vec![
                String::from("q_proj"),
                String::from("k_proj"),
                String::from("v_proj"),
                String::from("o_proj"),
                String::from("up_proj"),
                String::from("down_proj"),
                String::from("gate_proj"),
            ],
            nodes,
        }
    }

    fn node(
        node_id: &str,
        host_memory_bytes: u64,
        accelerator_memory_bytes: u64,
    ) -> QwenLegalPylonNodeCapability {
        QwenLegalPylonNodeCapability {
            node_id: String::from(node_id),
            backend_label: String::from("metal"),
            host_memory_bytes,
            accelerator_memory_bytes,
            model_cached: true,
            allowed_job_types: vec![String::from("sft_train_shard")],
            trust_state: String::from("trusted"),
            payment_target_ref: format!("bitcoin+lightning://{node_id}"),
        }
    }

    fn gib() -> u64 {
        1_073_741_824
    }

    fn without_digest(plan: &QwenLegalTrainingPlacementPlan) -> QwenLegalTrainingPlacementPlan {
        let mut plan = plan.clone();
        plan.plan_digest.clear();
        plan
    }
}
