use crate::receipts::Money;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const COMPUTE_LAUNCH_TAXONOMY_VERSION: &str = "compute.launch.v1";

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeSettlementMode {
    #[default]
    Physical,
    Cash,
    BuyerElection,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeProductStatus {
    #[default]
    Active,
    Retired,
}

impl ComputeProductStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Retired => "retired",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeEnvironmentPackageStatus {
    Draft,
    #[default]
    Active,
    Deprecated,
    Retired,
}

impl ComputeEnvironmentPackageStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Deprecated => "deprecated",
            Self::Retired => "retired",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "active" => Some(Self::Active),
            "deprecated" => Some(Self::Deprecated),
            "retired" => Some(Self::Retired),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeEvaluationRunStatus {
    #[default]
    Queued,
    Running,
    Finalized,
    Failed,
    Cancelled,
}

impl ComputeEvaluationRunStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Finalized => "finalized",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "running" => Some(Self::Running),
            "finalized" => Some(Self::Finalized),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeEvaluationSampleStatus {
    #[default]
    Recorded,
    Scored,
    Passed,
    Failed,
    Errored,
}

impl ComputeEvaluationSampleStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Recorded => "recorded",
            Self::Scored => "scored",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Errored => "errored",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "recorded" => Some(Self::Recorded),
            "scored" => Some(Self::Scored),
            "passed" => Some(Self::Passed),
            "failed" => Some(Self::Failed),
            "errored" => Some(Self::Errored),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeSyntheticDataJobStatus {
    #[default]
    Queued,
    Generating,
    Generated,
    Verifying,
    Verified,
    Failed,
}

impl ComputeSyntheticDataJobStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Generating => "generating",
            Self::Generated => "generated",
            Self::Verifying => "verifying",
            Self::Verified => "verified",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "generating" => Some(Self::Generating),
            "generated" => Some(Self::Generated),
            "verifying" => Some(Self::Verifying),
            "verified" => Some(Self::Verified),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeSyntheticDataSampleStatus {
    #[default]
    Generated,
    Verified,
    Rejected,
    Errored,
}

impl ComputeSyntheticDataSampleStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Generated => "generated",
            Self::Verified => "verified",
            Self::Rejected => "rejected",
            Self::Errored => "errored",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "generated" => Some(Self::Generated),
            "verified" => Some(Self::Verified),
            "rejected" => Some(Self::Rejected),
            "errored" => Some(Self::Errored),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CapacityReserveState {
    #[default]
    Available,
    Reserved,
    Exhausted,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CapacityLotStatus {
    #[default]
    Open,
    Reserved,
    Delivering,
    Delivered,
    Cancelled,
    Expired,
}

impl CapacityLotStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Reserved => "reserved",
            Self::Delivering => "delivering",
            Self::Delivered => "delivered",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryProofStatus {
    #[default]
    Recorded,
    Accepted,
    Rejected,
}

impl DeliveryProofStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Recorded => "recorded",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeIndexStatus {
    #[default]
    Published,
    Superseded,
}

impl ComputeIndexStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Published => "published",
            Self::Superseded => "superseded",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CapacityInstrumentKind {
    #[default]
    Spot,
    ForwardPhysical,
    FutureCash,
    Reservation,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CapacityInstrumentStatus {
    #[default]
    Open,
    Active,
    Delivering,
    CashSettling,
    Settled,
    Defaulted,
    Cancelled,
    Expired,
}

impl CapacityInstrumentStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Active => "active",
            Self::Delivering => "delivering",
            Self::CashSettling => "cash_settling",
            Self::Settled => "settled",
            Self::Defaulted => "defaulted",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StructuredCapacityInstrumentKind {
    #[default]
    Reservation,
    Swap,
    Strip,
}

impl StructuredCapacityInstrumentKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Reservation => "reservation",
            Self::Swap => "swap",
            Self::Strip => "strip",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StructuredCapacityInstrumentStatus {
    #[default]
    Open,
    Active,
    PartiallyClosed,
    Settled,
    Defaulted,
    Cancelled,
    Expired,
}

impl StructuredCapacityInstrumentStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Active => "active",
            Self::PartiallyClosed => "partially_closed",
            Self::Settled => "settled",
            Self::Defaulted => "defaulted",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StructuredCapacityLegRole {
    #[default]
    ReservationRight,
    SwapPay,
    SwapReceive,
    StripSegment,
}

impl StructuredCapacityLegRole {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ReservationRight => "reservation_right",
            Self::SwapPay => "swap_pay",
            Self::SwapReceive => "swap_receive",
            Self::StripSegment => "strip_segment",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeBackendFamily {
    #[serde(alias = "ollama")]
    GptOss,
    AppleFoundationModels,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeExecutionKind {
    LocalInference,
    ClusteredInference,
    SandboxExecution,
    EvaluationRun,
    TrainingJob,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeFamily {
    Inference,
    Embeddings,
    SandboxExecution,
    Evaluation,
    Training,
    AdapterHosting,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeTopologyKind {
    #[default]
    SingleNode,
    RemoteWholeRequest,
    Replicated,
    PipelineSharded,
    LayerSharded,
    TensorSharded,
    SandboxIsolated,
    TrainingElastic,
}

impl ComputeTopologyKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::SingleNode => "single_node",
            Self::RemoteWholeRequest => "remote_whole_request",
            Self::Replicated => "replicated",
            Self::PipelineSharded => "pipeline_sharded",
            Self::LayerSharded => "layer_sharded",
            Self::TensorSharded => "tensor_sharded",
            Self::SandboxIsolated => "sandbox_isolated",
            Self::TrainingElastic => "training_elastic",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeProvisioningKind {
    #[default]
    DesktopLocal,
    ClusterAttached,
    RemoteSandbox,
    ReservedClusterWindow,
}

impl ComputeProvisioningKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::DesktopLocal => "desktop_local",
            Self::ClusterAttached => "cluster_attached",
            Self::RemoteSandbox => "remote_sandbox",
            Self::ReservedClusterWindow => "reserved_cluster_window",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeProofPosture {
    #[default]
    DeliveryProofOnly,
    None,
    TopologyAndDelivery,
    ToplocAugmented,
    ChallengeEligible,
}

impl ComputeProofPosture {
    pub const fn label(self) -> &'static str {
        match self {
            Self::DeliveryProofOnly => "delivery_proof_only",
            Self::None => "none",
            Self::TopologyAndDelivery => "topology_and_delivery",
            Self::ToplocAugmented => "toploc_augmented",
            Self::ChallengeEligible => "challenge_eligible",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct ComputeValidatorRequirements {
    #[serde(default)]
    pub validator_pool_ref: Option<String>,
    #[serde(default)]
    pub policy_ref: Option<String>,
    #[serde(default)]
    pub minimum_validator_count: Option<u32>,
    #[serde(default)]
    pub challenge_window_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct ComputeArtifactResidency {
    #[serde(default)]
    pub residency_class: Option<String>,
    #[serde(default)]
    pub staging_policy: Option<String>,
    #[serde(default)]
    pub artifact_set_digest: Option<String>,
    #[serde(default)]
    pub warm: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct ComputeEnvironmentBinding {
    #[serde(default)]
    pub environment_ref: String,
    #[serde(default)]
    pub environment_version: Option<String>,
    #[serde(default)]
    pub dataset_ref: Option<String>,
    #[serde(default)]
    pub rubric_ref: Option<String>,
    #[serde(default)]
    pub evaluator_policy_ref: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct ComputeEnvironmentDatasetBinding {
    pub dataset_ref: String,
    #[serde(default)]
    pub split_ref: Option<String>,
    #[serde(default)]
    pub mount_path: Option<String>,
    #[serde(default)]
    pub integrity_ref: Option<String>,
    #[serde(default)]
    pub access_policy_ref: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct ComputeEnvironmentHarness {
    pub harness_ref: String,
    pub runtime_family: String,
    #[serde(default)]
    pub entrypoint: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub sandbox_profile_ref: Option<String>,
    #[serde(default)]
    pub evaluator_policy_ref: Option<String>,
    #[serde(default)]
    pub time_budget_ms: Option<u64>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct ComputeEnvironmentRubricBinding {
    pub rubric_ref: String,
    #[serde(default)]
    pub score_type: Option<String>,
    #[serde(default)]
    pub pass_threshold_bps: Option<u32>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct ComputeEnvironmentArtifactExpectation {
    pub artifact_kind: String,
    #[serde(default)]
    pub artifact_ref: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub verification_policy_ref: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ComputeEnvironmentPackage {
    pub environment_ref: String,
    pub version: String,
    pub family: String,
    pub display_name: String,
    pub owner_id: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    #[serde(default)]
    pub status: ComputeEnvironmentPackageStatus,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub package_digest: Option<String>,
    #[serde(default)]
    pub dataset_bindings: Vec<ComputeEnvironmentDatasetBinding>,
    #[serde(default)]
    pub harness: Option<ComputeEnvironmentHarness>,
    #[serde(default)]
    pub rubric_bindings: Vec<ComputeEnvironmentRubricBinding>,
    #[serde(default)]
    pub expected_artifacts: Vec<ComputeEnvironmentArtifactExpectation>,
    #[serde(default)]
    pub policy_refs: Vec<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ComputeEvaluationMetric {
    pub metric_id: String,
    pub metric_value: f64,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct ComputeEvaluationArtifact {
    pub artifact_kind: String,
    pub artifact_ref: String,
    #[serde(default)]
    pub digest: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ComputeEvaluationSummary {
    #[serde(default)]
    pub total_samples: u64,
    #[serde(default)]
    pub scored_samples: u64,
    #[serde(default)]
    pub passed_samples: u64,
    #[serde(default)]
    pub failed_samples: u64,
    #[serde(default)]
    pub errored_samples: u64,
    #[serde(default)]
    pub average_score_bps: Option<u32>,
    #[serde(default)]
    pub pass_rate_bps: Option<u32>,
    #[serde(default)]
    pub aggregate_metrics: Vec<ComputeEvaluationMetric>,
    #[serde(default)]
    pub artifacts: Vec<ComputeEvaluationArtifact>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ComputeEvaluationRun {
    pub eval_run_id: String,
    pub environment_binding: ComputeEnvironmentBinding,
    #[serde(default)]
    pub product_id: Option<String>,
    #[serde(default)]
    pub capacity_lot_id: Option<String>,
    #[serde(default)]
    pub instrument_id: Option<String>,
    #[serde(default)]
    pub delivery_proof_id: Option<String>,
    #[serde(default)]
    pub model_ref: Option<String>,
    #[serde(default)]
    pub source_ref: Option<String>,
    pub created_at_ms: i64,
    #[serde(default)]
    pub expected_sample_count: Option<u64>,
    #[serde(default)]
    pub status: ComputeEvaluationRunStatus,
    #[serde(default)]
    pub started_at_ms: Option<i64>,
    #[serde(default)]
    pub finalized_at_ms: Option<i64>,
    #[serde(default)]
    pub summary: Option<ComputeEvaluationSummary>,
    #[serde(default)]
    pub run_artifacts: Vec<ComputeEvaluationArtifact>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ComputeEvaluationSample {
    pub eval_run_id: String,
    pub sample_id: String,
    #[serde(default)]
    pub ordinal: Option<u64>,
    #[serde(default)]
    pub status: ComputeEvaluationSampleStatus,
    #[serde(default)]
    pub input_ref: Option<String>,
    #[serde(default)]
    pub output_ref: Option<String>,
    #[serde(default)]
    pub expected_output_ref: Option<String>,
    #[serde(default)]
    pub score_bps: Option<u32>,
    #[serde(default)]
    pub metrics: Vec<ComputeEvaluationMetric>,
    #[serde(default)]
    pub artifacts: Vec<ComputeEvaluationArtifact>,
    #[serde(default)]
    pub error_reason: Option<String>,
    pub recorded_at_ms: i64,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ComputeSyntheticDataJob {
    pub synthetic_job_id: String,
    pub environment_binding: ComputeEnvironmentBinding,
    pub teacher_model_ref: String,
    #[serde(default)]
    pub generation_product_id: Option<String>,
    #[serde(default)]
    pub generation_delivery_proof_id: Option<String>,
    #[serde(default)]
    pub output_artifact_ref: Option<String>,
    pub created_at_ms: i64,
    #[serde(default)]
    pub generated_at_ms: Option<i64>,
    #[serde(default)]
    pub verification_eval_run_id: Option<String>,
    #[serde(default)]
    pub verified_at_ms: Option<i64>,
    #[serde(default)]
    pub target_sample_count: Option<u64>,
    #[serde(default)]
    pub status: ComputeSyntheticDataJobStatus,
    #[serde(default)]
    pub verification_summary: Option<ComputeEvaluationSummary>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ComputeSyntheticDataSample {
    pub synthetic_job_id: String,
    pub sample_id: String,
    #[serde(default)]
    pub ordinal: Option<u64>,
    pub prompt_ref: String,
    pub output_ref: String,
    #[serde(default)]
    pub generation_config_ref: Option<String>,
    #[serde(default)]
    pub generator_machine_ref: Option<String>,
    #[serde(default)]
    pub verification_eval_sample_id: Option<String>,
    #[serde(default)]
    pub verification_status: Option<ComputeEvaluationSampleStatus>,
    #[serde(default)]
    pub verification_score_bps: Option<u32>,
    #[serde(default)]
    pub status: ComputeSyntheticDataSampleStatus,
    pub recorded_at_ms: i64,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct ComputeCheckpointBinding {
    #[serde(default)]
    pub checkpoint_family: String,
    #[serde(default)]
    pub latest_checkpoint_ref: Option<String>,
    #[serde(default)]
    pub recovery_posture: Option<String>,
}

macro_rules! canonical_reason_code_enum {
    ($name:ident { $($variant:ident => $label:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const fn label(self) -> &'static str {
                match self {
                    $(Self::$variant => $label),+
                }
            }

            pub fn parse(value: &str) -> Option<Self> {
                match value {
                    $($label => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

canonical_reason_code_enum!(CapacityLotCancellationReason {
    ProviderUnavailable => "provider_unavailable",
    PolicyDisabled => "policy_disabled",
    MarketHalt => "market_halt",
    Superseded => "superseded",
    OfferExpired => "offer_expired",
});

canonical_reason_code_enum!(CapacityCurtailmentReason {
    RuntimeHealth => "runtime_health",
    ProviderPreempted => "provider_preempted",
    BuyerAborted => "buyer_aborted",
    PolicyEnforcement => "policy_enforcement",
    CapabilityDrift => "capability_drift",
});

canonical_reason_code_enum!(CapacityInstrumentClosureReason {
    Filled => "filled",
    BuyerCancelled => "buyer_cancelled",
    ProviderCancelled => "provider_cancelled",
    Curtailed => "curtailed",
    Expired => "expired",
    Defaulted => "defaulted",
});

canonical_reason_code_enum!(CapacityNonDeliveryReason {
    ProviderOffline => "provider_offline",
    CapabilityMismatch => "capability_mismatch",
    PolicyBlocked => "policy_blocked",
    MissedWindow => "missed_window",
});

canonical_reason_code_enum!(ComputeSettlementFailureReason {
    PaymentTimeout => "payment_timeout",
    ReceiptRejected => "receipt_rejected",
    NonDelivery => "non_delivery",
    CostAttestationMissing => "cost_attestation_missing",
    AdjudicationRequired => "adjudication_required",
});

canonical_reason_code_enum!(ComputeDeliveryVarianceReason {
    CapabilityEnvelopeMismatch => "capability_envelope_mismatch",
    PartialQuantity => "partial_quantity",
    LatencyBreach => "latency_breach",
    ThroughputShortfall => "throughput_shortfall",
    ModelPolicyDrift => "model_policy_drift",
});

canonical_reason_code_enum!(DeliveryRejectionReason {
    AttestationMissing => "attestation_missing",
    CostProofMissing => "cost_proof_missing",
    RuntimeIdentityMismatch => "runtime_identity_mismatch",
    NonConformingDelivery => "non_conforming_delivery",
});

canonical_reason_code_enum!(ComputeIndexCorrectionReason {
    DataQuality => "data_quality",
    ManipulationFilter => "manipulation_filter",
    MethodologyBug => "methodology_bug",
    LateObservation => "late_observation",
});

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct ComputeHostCapability {
    #[serde(default)]
    pub accelerator_vendor: Option<String>,
    #[serde(default)]
    pub accelerator_family: Option<String>,
    #[serde(default)]
    pub memory_gb: Option<u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct ApplePlatformCapability {
    #[serde(default)]
    pub apple_silicon_required: bool,
    #[serde(default)]
    pub apple_intelligence_required: bool,
    #[serde(default)]
    pub apple_intelligence_available: Option<bool>,
    #[serde(default)]
    pub minimum_macos_version: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct GptOssRuntimeCapability {
    #[serde(default)]
    pub runtime_ready: Option<bool>,
    #[serde(default)]
    pub model_name: Option<String>,
    #[serde(default)]
    pub quantization: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct ComputeCapabilityEnvelope {
    #[serde(default)]
    pub backend_family: Option<ComputeBackendFamily>,
    #[serde(default)]
    pub execution_kind: Option<ComputeExecutionKind>,
    #[serde(default)]
    pub compute_family: Option<ComputeFamily>,
    #[serde(default)]
    pub topology_kind: Option<ComputeTopologyKind>,
    #[serde(default)]
    pub provisioning_kind: Option<ComputeProvisioningKind>,
    #[serde(default)]
    pub proof_posture: Option<ComputeProofPosture>,
    #[serde(default)]
    pub validator_requirements: Option<ComputeValidatorRequirements>,
    #[serde(default)]
    pub artifact_residency: Option<ComputeArtifactResidency>,
    #[serde(default)]
    pub environment_binding: Option<ComputeEnvironmentBinding>,
    #[serde(default)]
    pub checkpoint_binding: Option<ComputeCheckpointBinding>,
    #[serde(default)]
    pub model_policy: Option<String>,
    #[serde(default)]
    pub model_family: Option<String>,
    #[serde(default)]
    pub host_capability: Option<ComputeHostCapability>,
    #[serde(default)]
    pub apple_platform: Option<ApplePlatformCapability>,
    #[serde(default, alias = "ollama_runtime")]
    pub gpt_oss_runtime: Option<GptOssRuntimeCapability>,
    #[serde(default)]
    pub latency_ms_p50: Option<u32>,
    #[serde(default)]
    pub throughput_per_minute: Option<u32>,
    #[serde(default)]
    pub concurrency_limit: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchComputeProductSpec {
    pub product_id: &'static str,
    pub backend_family: ComputeBackendFamily,
    pub execution_kind: ComputeExecutionKind,
    pub compute_family: ComputeFamily,
}

pub const PSIONIC_LOCAL_GPT_OSS_INFERENCE_PRODUCT_ID: &str =
    "psionic.local.inference.gpt_oss.single_node";
pub const PSIONIC_LOCAL_GPT_OSS_EMBEDDINGS_PRODUCT_ID: &str =
    "psionic.local.embeddings.gpt_oss.single_node";
pub const PSIONIC_LOCAL_APPLE_FM_INFERENCE_PRODUCT_ID: &str =
    "psionic.local.inference.apple_foundation_models.single_node";
pub const PSIONIC_REMOTE_SANDBOX_CONTAINER_EXEC_PRODUCT_ID: &str =
    "psionic.remote_sandbox.sandbox_execution.container_exec.sandbox_isolated";
pub const PSIONIC_REMOTE_SANDBOX_PYTHON_EXEC_PRODUCT_ID: &str =
    "psionic.remote_sandbox.sandbox_execution.python_exec.sandbox_isolated";
pub const PSIONIC_REMOTE_SANDBOX_NODE_EXEC_PRODUCT_ID: &str =
    "psionic.remote_sandbox.sandbox_execution.node_exec.sandbox_isolated";
pub const PSIONIC_REMOTE_SANDBOX_POSIX_EXEC_PRODUCT_ID: &str =
    "psionic.remote_sandbox.sandbox_execution.posix_exec.sandbox_isolated";

pub fn canonical_compute_product_id(product_id: &str) -> Option<&'static str> {
    match product_id.trim() {
        PSIONIC_LOCAL_GPT_OSS_INFERENCE_PRODUCT_ID
        | "ollama.text_generation"
        | "gpt_oss.text_generation" => Some(PSIONIC_LOCAL_GPT_OSS_INFERENCE_PRODUCT_ID),
        PSIONIC_LOCAL_GPT_OSS_EMBEDDINGS_PRODUCT_ID
        | "ollama.embeddings"
        | "gpt_oss.embeddings" => Some(PSIONIC_LOCAL_GPT_OSS_EMBEDDINGS_PRODUCT_ID),
        PSIONIC_LOCAL_APPLE_FM_INFERENCE_PRODUCT_ID | "apple_foundation_models.text_generation" => {
            Some(PSIONIC_LOCAL_APPLE_FM_INFERENCE_PRODUCT_ID)
        }
        PSIONIC_REMOTE_SANDBOX_CONTAINER_EXEC_PRODUCT_ID | "sandbox.container.exec" => {
            Some(PSIONIC_REMOTE_SANDBOX_CONTAINER_EXEC_PRODUCT_ID)
        }
        PSIONIC_REMOTE_SANDBOX_PYTHON_EXEC_PRODUCT_ID | "sandbox.python.exec" => {
            Some(PSIONIC_REMOTE_SANDBOX_PYTHON_EXEC_PRODUCT_ID)
        }
        PSIONIC_REMOTE_SANDBOX_NODE_EXEC_PRODUCT_ID | "sandbox.node.exec" => {
            Some(PSIONIC_REMOTE_SANDBOX_NODE_EXEC_PRODUCT_ID)
        }
        PSIONIC_REMOTE_SANDBOX_POSIX_EXEC_PRODUCT_ID | "sandbox.posix.exec" => {
            Some(PSIONIC_REMOTE_SANDBOX_POSIX_EXEC_PRODUCT_ID)
        }
        _ => None,
    }
}

pub fn validate_compute_capability_envelope(
    envelope: &ComputeCapabilityEnvelope,
) -> Result<(), String> {
    if let Some(requirements) = envelope.validator_requirements.as_ref() {
        if requirements.minimum_validator_count == Some(0) {
            return Err("compute_validator_count_invalid".to_string());
        }
        let has_policy_ref = requirements
            .policy_ref
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
        let has_pool_ref = requirements
            .validator_pool_ref
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
        if !has_policy_ref && !has_pool_ref {
            return Err("compute_validator_requirements_missing_reference".to_string());
        }
    }

    if envelope.proof_posture == Some(ComputeProofPosture::ChallengeEligible)
        && envelope.validator_requirements.is_none()
    {
        return Err("compute_challenge_posture_requires_validator_requirements".to_string());
    }

    if let Some(environment_binding) = envelope.environment_binding.as_ref() {
        if environment_binding.environment_ref.trim().is_empty() {
            return Err("compute_environment_binding_ref_missing".to_string());
        }
        if environment_binding
            .environment_version
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err("compute_environment_binding_version_invalid".to_string());
        }
    }

    if let Some(checkpoint_binding) = envelope.checkpoint_binding.as_ref() {
        if checkpoint_binding.checkpoint_family.trim().is_empty() {
            return Err("compute_checkpoint_family_missing".to_string());
        }
    }

    if let Some(artifact_residency) = envelope.artifact_residency.as_ref()
        && artifact_residency
            .artifact_set_digest
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
    {
        return Err("compute_artifact_residency_digest_invalid".to_string());
    }

    Ok(())
}

pub fn validate_compute_environment_package(
    package: &ComputeEnvironmentPackage,
) -> Result<(), String> {
    if package.environment_ref.trim().is_empty() {
        return Err("compute_environment_ref_missing".to_string());
    }
    if package.version.trim().is_empty() {
        return Err("compute_environment_version_missing".to_string());
    }
    if package.family.trim().is_empty() {
        return Err("compute_environment_family_missing".to_string());
    }
    if package.display_name.trim().is_empty() {
        return Err("compute_environment_display_name_missing".to_string());
    }
    if package.owner_id.trim().is_empty() {
        return Err("compute_environment_owner_id_missing".to_string());
    }
    if package.updated_at_ms < package.created_at_ms {
        return Err("compute_environment_timestamps_invalid".to_string());
    }
    if package
        .package_digest
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err("compute_environment_package_digest_invalid".to_string());
    }
    if let Some(harness) = package.harness.as_ref() {
        if harness.harness_ref.trim().is_empty() {
            return Err("compute_environment_harness_ref_missing".to_string());
        }
        if harness.runtime_family.trim().is_empty() {
            return Err("compute_environment_runtime_family_missing".to_string());
        }
        if harness
            .entrypoint
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err("compute_environment_entrypoint_invalid".to_string());
        }
        if harness.time_budget_ms == Some(0) {
            return Err("compute_environment_time_budget_invalid".to_string());
        }
    }
    for dataset in &package.dataset_bindings {
        if dataset.dataset_ref.trim().is_empty() {
            return Err("compute_environment_dataset_ref_missing".to_string());
        }
        if dataset
            .mount_path
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err("compute_environment_dataset_mount_invalid".to_string());
        }
        if dataset
            .integrity_ref
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err("compute_environment_dataset_integrity_invalid".to_string());
        }
    }
    for rubric in &package.rubric_bindings {
        if rubric.rubric_ref.trim().is_empty() {
            return Err("compute_environment_rubric_ref_missing".to_string());
        }
        if rubric
            .pass_threshold_bps
            .is_some_and(|value| value > 10_000)
        {
            return Err("compute_environment_rubric_threshold_invalid".to_string());
        }
    }
    for artifact in &package.expected_artifacts {
        if artifact.artifact_kind.trim().is_empty() {
            return Err("compute_environment_artifact_kind_missing".to_string());
        }
        if artifact
            .artifact_ref
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err("compute_environment_artifact_ref_invalid".to_string());
        }
    }
    if package
        .policy_refs
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err("compute_environment_policy_ref_invalid".to_string());
    }
    Ok(())
}

pub fn validate_compute_evaluation_metric(metric: &ComputeEvaluationMetric) -> Result<(), String> {
    if metric.metric_id.trim().is_empty() {
        return Err("compute_eval_metric_id_missing".to_string());
    }
    if !metric.metric_value.is_finite() {
        return Err("compute_eval_metric_value_invalid".to_string());
    }
    if metric
        .unit
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err("compute_eval_metric_unit_invalid".to_string());
    }
    Ok(())
}

pub fn validate_compute_evaluation_artifact(
    artifact: &ComputeEvaluationArtifact,
) -> Result<(), String> {
    if artifact.artifact_kind.trim().is_empty() {
        return Err("compute_eval_artifact_kind_missing".to_string());
    }
    if artifact.artifact_ref.trim().is_empty() {
        return Err("compute_eval_artifact_ref_missing".to_string());
    }
    if artifact
        .digest
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err("compute_eval_artifact_digest_invalid".to_string());
    }
    Ok(())
}

pub fn validate_compute_evaluation_summary(
    summary: &ComputeEvaluationSummary,
) -> Result<(), String> {
    if summary.scored_samples > summary.total_samples
        || summary.passed_samples > summary.scored_samples
        || summary.failed_samples > summary.scored_samples
        || summary.errored_samples > summary.total_samples
        || summary
            .passed_samples
            .saturating_add(summary.failed_samples)
            > summary.scored_samples
    {
        return Err("compute_eval_summary_counts_invalid".to_string());
    }
    if summary
        .average_score_bps
        .is_some_and(|value| value > 10_000)
    {
        return Err("compute_eval_score_invalid".to_string());
    }
    if summary.pass_rate_bps.is_some_and(|value| value > 10_000) {
        return Err("compute_eval_pass_rate_invalid".to_string());
    }
    for metric in &summary.aggregate_metrics {
        validate_compute_evaluation_metric(metric)?;
    }
    for artifact in &summary.artifacts {
        validate_compute_evaluation_artifact(artifact)?;
    }
    Ok(())
}

pub fn validate_compute_evaluation_run(run: &ComputeEvaluationRun) -> Result<(), String> {
    if run.eval_run_id.trim().is_empty() {
        return Err("compute_eval_run_id_missing".to_string());
    }
    if run.environment_binding.environment_ref.trim().is_empty() {
        return Err("compute_environment_binding_ref_missing".to_string());
    }
    if run
        .environment_binding
        .environment_version
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err("compute_environment_version_missing".to_string());
    }
    if run.expected_sample_count == Some(0) {
        return Err("compute_eval_expected_sample_count_invalid".to_string());
    }
    if matches!(
        run.status,
        ComputeEvaluationRunStatus::Finalized
            | ComputeEvaluationRunStatus::Failed
            | ComputeEvaluationRunStatus::Cancelled
    ) && run.finalized_at_ms.is_none()
    {
        return Err("compute_eval_finalized_at_missing".to_string());
    }
    if matches!(run.status, ComputeEvaluationRunStatus::Finalized) && run.summary.is_none() {
        return Err("compute_eval_summary_missing".to_string());
    }
    if let Some(summary) = run.summary.as_ref() {
        validate_compute_evaluation_summary(summary)?;
    }
    for artifact in &run.run_artifacts {
        validate_compute_evaluation_artifact(artifact)?;
    }
    Ok(())
}

pub fn validate_compute_evaluation_sample(sample: &ComputeEvaluationSample) -> Result<(), String> {
    if sample.eval_run_id.trim().is_empty() {
        return Err("compute_eval_run_id_missing".to_string());
    }
    if sample.sample_id.trim().is_empty() {
        return Err("compute_eval_sample_id_missing".to_string());
    }
    if sample.recorded_at_ms <= 0 {
        return Err("compute_eval_sample_recorded_at_invalid".to_string());
    }
    if sample.score_bps.is_some_and(|value| value > 10_000) {
        return Err("compute_eval_score_invalid".to_string());
    }
    if matches!(sample.status, ComputeEvaluationSampleStatus::Errored)
        && sample
            .error_reason
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        return Err("compute_eval_sample_error_reason_missing".to_string());
    }
    for metric in &sample.metrics {
        validate_compute_evaluation_metric(metric)?;
    }
    for artifact in &sample.artifacts {
        validate_compute_evaluation_artifact(artifact)?;
    }
    Ok(())
}

pub fn validate_compute_synthetic_data_job(job: &ComputeSyntheticDataJob) -> Result<(), String> {
    if job.synthetic_job_id.trim().is_empty() {
        return Err("compute_synthetic_job_id_missing".to_string());
    }
    if job.environment_binding.environment_ref.trim().is_empty() {
        return Err("compute_environment_binding_ref_missing".to_string());
    }
    if job
        .environment_binding
        .environment_version
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err("compute_environment_version_missing".to_string());
    }
    if job.teacher_model_ref.trim().is_empty() {
        return Err("compute_synthetic_teacher_model_ref_missing".to_string());
    }
    if job.target_sample_count == Some(0) {
        return Err("compute_synthetic_target_sample_count_invalid".to_string());
    }
    if matches!(
        job.status,
        ComputeSyntheticDataJobStatus::Generated | ComputeSyntheticDataJobStatus::Verified
    ) && job
        .output_artifact_ref
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err("compute_synthetic_output_artifact_ref_missing".to_string());
    }
    if matches!(
        job.status,
        ComputeSyntheticDataJobStatus::Generated | ComputeSyntheticDataJobStatus::Verified
    ) && job.generated_at_ms.is_none()
    {
        return Err("compute_synthetic_generated_at_missing".to_string());
    }
    if matches!(job.status, ComputeSyntheticDataJobStatus::Verified)
        && job
            .verification_eval_run_id
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        return Err("compute_synthetic_verification_eval_run_id_missing".to_string());
    }
    if matches!(job.status, ComputeSyntheticDataJobStatus::Verified) && job.verified_at_ms.is_none()
    {
        return Err("compute_synthetic_verified_at_missing".to_string());
    }
    if matches!(job.status, ComputeSyntheticDataJobStatus::Verified)
        && job.verification_summary.is_none()
    {
        return Err("compute_synthetic_verification_summary_missing".to_string());
    }
    if let Some(summary) = job.verification_summary.as_ref() {
        validate_compute_evaluation_summary(summary)?;
    }
    Ok(())
}

pub fn validate_compute_synthetic_data_sample(
    sample: &ComputeSyntheticDataSample,
) -> Result<(), String> {
    if sample.synthetic_job_id.trim().is_empty() {
        return Err("compute_synthetic_job_id_missing".to_string());
    }
    if sample.sample_id.trim().is_empty() {
        return Err("compute_synthetic_sample_id_missing".to_string());
    }
    if sample.prompt_ref.trim().is_empty() {
        return Err("compute_synthetic_prompt_ref_missing".to_string());
    }
    if sample.output_ref.trim().is_empty() {
        return Err("compute_synthetic_output_ref_missing".to_string());
    }
    if sample.recorded_at_ms <= 0 {
        return Err("compute_synthetic_sample_recorded_at_invalid".to_string());
    }
    if sample
        .verification_score_bps
        .is_some_and(|value| value > 10_000)
    {
        return Err("compute_synthetic_verification_score_invalid".to_string());
    }
    if sample.verification_status.is_some() && sample.verification_eval_sample_id.is_none() {
        return Err("compute_synthetic_verification_eval_sample_id_missing".to_string());
    }
    if matches!(
        sample.status,
        ComputeSyntheticDataSampleStatus::Verified
            | ComputeSyntheticDataSampleStatus::Rejected
            | ComputeSyntheticDataSampleStatus::Errored
    ) && sample.verification_status.is_none()
    {
        return Err("compute_synthetic_verification_status_missing".to_string());
    }
    Ok(())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ComputeProduct {
    pub product_id: String,
    pub resource_class: String,
    pub capacity_unit: String,
    pub window_spec: String,
    pub region_spec: Vec<String>,
    #[serde(default)]
    pub performance_band: Option<String>,
    #[serde(default)]
    pub sla_terms_ref: Option<String>,
    #[serde(default)]
    pub cost_proof_required: bool,
    #[serde(default)]
    pub attestation_required: bool,
    #[serde(default)]
    pub settlement_mode: ComputeSettlementMode,
    #[serde(default)]
    pub index_eligible: bool,
    #[serde(default)]
    pub status: ComputeProductStatus,
    pub version: String,
    pub created_at_ms: i64,
    #[serde(default)]
    pub taxonomy_version: Option<String>,
    #[serde(default)]
    pub capability_envelope: Option<ComputeCapabilityEnvelope>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CapacityLot {
    pub capacity_lot_id: String,
    pub product_id: String,
    pub provider_id: String,
    pub delivery_start_ms: i64,
    pub delivery_end_ms: i64,
    pub quantity: u64,
    #[serde(default)]
    pub min_unit_price: Option<Money>,
    #[serde(default)]
    pub region_hint: Option<String>,
    #[serde(default)]
    pub attestation_posture: Option<String>,
    #[serde(default)]
    pub reserve_state: CapacityReserveState,
    pub offer_expires_at_ms: i64,
    #[serde(default)]
    pub status: CapacityLotStatus,
    #[serde(default)]
    pub environment_binding: Option<ComputeEnvironmentBinding>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DeliveryProof {
    pub delivery_proof_id: String,
    pub capacity_lot_id: String,
    pub product_id: String,
    #[serde(default)]
    pub instrument_id: Option<String>,
    #[serde(default)]
    pub contract_id: Option<String>,
    pub created_at_ms: i64,
    pub metered_quantity: u64,
    pub accepted_quantity: u64,
    #[serde(default)]
    pub performance_band_observed: Option<String>,
    #[serde(default)]
    pub variance_reason: Option<ComputeDeliveryVarianceReason>,
    #[serde(default)]
    pub variance_reason_detail: Option<String>,
    #[serde(default)]
    pub attestation_digest: Option<String>,
    #[serde(default)]
    pub cost_attestation_ref: Option<String>,
    #[serde(default)]
    pub status: DeliveryProofStatus,
    #[serde(default)]
    pub rejection_reason: Option<DeliveryRejectionReason>,
    #[serde(default)]
    pub topology_evidence: Option<DeliveryTopologyEvidence>,
    #[serde(default)]
    pub sandbox_evidence: Option<DeliverySandboxEvidence>,
    #[serde(default)]
    pub verification_evidence: Option<DeliveryVerificationEvidence>,
    #[serde(default)]
    pub promised_capability_envelope: Option<ComputeCapabilityEnvelope>,
    #[serde(default)]
    pub observed_capability_envelope: Option<ComputeCapabilityEnvelope>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DeliveryTopologyEvidence {
    #[serde(default)]
    pub topology_kind: Option<ComputeTopologyKind>,
    #[serde(default)]
    pub topology_digest: Option<String>,
    #[serde(default)]
    pub scheduler_node_ref: Option<String>,
    #[serde(default)]
    pub transport_class: Option<String>,
    #[serde(default)]
    pub selected_node_refs: Vec<String>,
    #[serde(default)]
    pub replica_node_refs: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DeliverySandboxEvidence {
    #[serde(default)]
    pub sandbox_profile_ref: Option<String>,
    #[serde(default)]
    pub sandbox_execution_ref: Option<String>,
    #[serde(default)]
    pub command_digest: Option<String>,
    #[serde(default)]
    pub environment_digest: Option<String>,
    #[serde(default)]
    pub input_artifact_refs: Vec<String>,
    #[serde(default)]
    pub output_artifact_refs: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DeliveryVerificationEvidence {
    #[serde(default)]
    pub proof_bundle_ref: Option<String>,
    #[serde(default)]
    pub activation_fingerprint_ref: Option<String>,
    #[serde(default)]
    pub validator_pool_ref: Option<String>,
    #[serde(default)]
    pub validator_run_ref: Option<String>,
    #[serde(default)]
    pub challenge_result_refs: Vec<String>,
    #[serde(default)]
    pub environment_ref: Option<String>,
    #[serde(default)]
    pub environment_version: Option<String>,
    #[serde(default)]
    pub eval_run_ref: Option<String>,
}

pub fn validate_delivery_proof(proof: &DeliveryProof) -> Result<(), String> {
    if proof.accepted_quantity > proof.metered_quantity {
        return Err("delivery_proof_quantity_invalid".to_string());
    }
    if proof.status == DeliveryProofStatus::Rejected && proof.rejection_reason.is_none() {
        return Err("delivery_proof_rejection_reason_missing".to_string());
    }
    if proof.status != DeliveryProofStatus::Rejected && proof.rejection_reason.is_some() {
        return Err("delivery_proof_rejection_reason_unexpected".to_string());
    }
    if let Some(topology_evidence) = proof.topology_evidence.as_ref() {
        if topology_evidence
            .topology_digest
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err("delivery_proof_topology_digest_invalid".to_string());
        }
    }
    if let Some(verification_evidence) = proof.verification_evidence.as_ref() {
        if !verification_evidence.challenge_result_refs.is_empty()
            && verification_evidence.proof_bundle_ref.is_none()
        {
            return Err("delivery_proof_challenge_requires_proof_bundle".to_string());
        }
        if verification_evidence.environment_version.is_some()
            && verification_evidence.environment_ref.is_none()
        {
            return Err("delivery_proof_environment_version_requires_environment_ref".to_string());
        }
        if verification_evidence.environment_ref.is_some()
            && verification_evidence.environment_version.is_none()
        {
            return Err("delivery_proof_environment_version_missing".to_string());
        }
        if verification_evidence.eval_run_ref.is_some()
            && verification_evidence.environment_ref.is_none()
        {
            return Err("delivery_proof_eval_requires_environment_ref".to_string());
        }
    }
    let topology_kind = proof
        .topology_evidence
        .as_ref()
        .and_then(|value| value.topology_kind)
        .or_else(|| {
            proof
                .promised_capability_envelope
                .as_ref()
                .and_then(|value| value.topology_kind)
        })
        .or_else(|| {
            proof
                .observed_capability_envelope
                .as_ref()
                .and_then(|value| value.topology_kind)
        });
    if topology_kind.is_some_and(is_cluster_topology) {
        let Some(topology_evidence) = proof.topology_evidence.as_ref() else {
            return Err("delivery_proof_topology_evidence_missing".to_string());
        };
        if topology_evidence
            .topology_digest
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err("delivery_proof_topology_digest_missing".to_string());
        }
        if topology_evidence.selected_node_refs.is_empty() {
            return Err("delivery_proof_selected_node_refs_missing".to_string());
        }
    }
    let execution_kind = proof
        .promised_capability_envelope
        .as_ref()
        .and_then(|value| value.execution_kind)
        .or_else(|| {
            proof
                .observed_capability_envelope
                .as_ref()
                .and_then(|value| value.execution_kind)
        });
    if execution_kind == Some(ComputeExecutionKind::SandboxExecution) {
        let Some(sandbox_evidence) = proof.sandbox_evidence.as_ref() else {
            return Err("delivery_proof_sandbox_evidence_missing".to_string());
        };
        if sandbox_evidence
            .sandbox_profile_ref
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err("delivery_proof_sandbox_profile_ref_missing".to_string());
        }
        if sandbox_evidence
            .sandbox_execution_ref
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err("delivery_proof_sandbox_execution_ref_missing".to_string());
        }
    }
    let proof_posture = proof
        .promised_capability_envelope
        .as_ref()
        .and_then(|value| value.proof_posture)
        .or_else(|| {
            proof
                .observed_capability_envelope
                .as_ref()
                .and_then(|value| value.proof_posture)
        });
    if matches!(
        proof_posture,
        Some(
            ComputeProofPosture::TopologyAndDelivery
                | ComputeProofPosture::ToplocAugmented
                | ComputeProofPosture::ChallengeEligible
        )
    ) && proof
        .verification_evidence
        .as_ref()
        .and_then(|value| value.proof_bundle_ref.as_deref())
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err("delivery_proof_bundle_ref_missing".to_string());
    }
    if proof_posture == Some(ComputeProofPosture::ToplocAugmented)
        && proof
            .verification_evidence
            .as_ref()
            .and_then(|value| value.activation_fingerprint_ref.as_deref())
            .is_none_or(|value| value.trim().is_empty())
    {
        return Err("delivery_proof_activation_fingerprint_ref_missing".to_string());
    }
    if proof_posture == Some(ComputeProofPosture::ChallengeEligible)
        && proof
            .verification_evidence
            .as_ref()
            .and_then(|value| value.validator_pool_ref.as_deref())
            .is_none_or(|value| value.trim().is_empty())
    {
        return Err("delivery_proof_validator_pool_ref_missing".to_string());
    }
    Ok(())
}

fn is_cluster_topology(topology_kind: ComputeTopologyKind) -> bool {
    matches!(
        topology_kind,
        ComputeTopologyKind::RemoteWholeRequest
            | ComputeTopologyKind::Replicated
            | ComputeTopologyKind::PipelineSharded
            | ComputeTopologyKind::LayerSharded
            | ComputeTopologyKind::TensorSharded
            | ComputeTopologyKind::TrainingElastic
    )
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ComputeIndex {
    pub index_id: String,
    pub product_id: String,
    pub observation_window_start_ms: i64,
    pub observation_window_end_ms: i64,
    pub published_at_ms: i64,
    #[serde(default)]
    pub observation_count: u64,
    #[serde(default)]
    pub total_accepted_quantity: u64,
    #[serde(default)]
    pub reference_price: Option<Money>,
    #[serde(default)]
    pub methodology: Option<String>,
    #[serde(default)]
    pub status: ComputeIndexStatus,
    #[serde(default)]
    pub correction_reason: Option<ComputeIndexCorrectionReason>,
    #[serde(default)]
    pub corrected_from_index_id: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CapacityInstrument {
    pub instrument_id: String,
    pub product_id: String,
    #[serde(default)]
    pub capacity_lot_id: Option<String>,
    #[serde(default)]
    pub buyer_id: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
    pub delivery_start_ms: i64,
    pub delivery_end_ms: i64,
    pub quantity: u64,
    #[serde(default)]
    pub fixed_price: Option<Money>,
    #[serde(default)]
    pub reference_index_id: Option<String>,
    #[serde(default)]
    pub kind: CapacityInstrumentKind,
    #[serde(default)]
    pub settlement_mode: ComputeSettlementMode,
    pub created_at_ms: i64,
    #[serde(default)]
    pub status: CapacityInstrumentStatus,
    #[serde(default)]
    pub environment_binding: Option<ComputeEnvironmentBinding>,
    #[serde(default)]
    pub closure_reason: Option<CapacityInstrumentClosureReason>,
    #[serde(default)]
    pub non_delivery_reason: Option<CapacityNonDeliveryReason>,
    #[serde(default)]
    pub settlement_failure_reason: Option<ComputeSettlementFailureReason>,
    #[serde(default)]
    pub lifecycle_reason_detail: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct StructuredCapacityLeg {
    pub instrument_id: String,
    #[serde(default)]
    pub role: StructuredCapacityLegRole,
    #[serde(default)]
    pub leg_order: u32,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct StructuredCapacityInstrument {
    pub structured_instrument_id: String,
    pub product_id: String,
    #[serde(default)]
    pub buyer_id: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub kind: StructuredCapacityInstrumentKind,
    pub created_at_ms: i64,
    #[serde(default)]
    pub status: StructuredCapacityInstrumentStatus,
    #[serde(default)]
    pub lifecycle_reason_detail: Option<String>,
    #[serde(default)]
    pub legs: Vec<StructuredCapacityLeg>,
    #[serde(default)]
    pub metadata: Value,
}

pub fn launch_compute_product_spec(product_id: &str) -> Option<LaunchComputeProductSpec> {
    match canonical_compute_product_id(product_id)? {
        PSIONIC_LOCAL_GPT_OSS_INFERENCE_PRODUCT_ID => Some(LaunchComputeProductSpec {
            product_id: PSIONIC_LOCAL_GPT_OSS_INFERENCE_PRODUCT_ID,
            backend_family: ComputeBackendFamily::GptOss,
            execution_kind: ComputeExecutionKind::LocalInference,
            compute_family: ComputeFamily::Inference,
        }),
        PSIONIC_LOCAL_APPLE_FM_INFERENCE_PRODUCT_ID => Some(LaunchComputeProductSpec {
            product_id: PSIONIC_LOCAL_APPLE_FM_INFERENCE_PRODUCT_ID,
            backend_family: ComputeBackendFamily::AppleFoundationModels,
            execution_kind: ComputeExecutionKind::LocalInference,
            compute_family: ComputeFamily::Inference,
        }),
        _ => None,
    }
}

pub fn validate_launch_compute_product(
    product: &ComputeProduct,
) -> Result<LaunchComputeProductSpec, String> {
    if product.resource_class.trim() != "compute" {
        return Err("compute_product_resource_class_invalid".to_string());
    }
    if product.capacity_unit.trim().is_empty() {
        return Err("compute_product_capacity_unit_missing".to_string());
    }
    if product.window_spec.trim().is_empty() {
        return Err("compute_product_window_spec_missing".to_string());
    }

    let Some(taxonomy_version) = product.taxonomy_version.as_deref() else {
        return Err("compute_product_launch_taxonomy_version_missing".to_string());
    };
    if taxonomy_version != COMPUTE_LAUNCH_TAXONOMY_VERSION {
        return Err("compute_product_launch_taxonomy_version_invalid".to_string());
    }

    let Some(spec) = launch_compute_product_spec(product.product_id.as_str()) else {
        return Err("compute_product_launch_product_id_unsupported".to_string());
    };
    let Some(envelope) = product.capability_envelope.as_ref() else {
        return Err("compute_product_capability_envelope_missing".to_string());
    };

    validate_compute_capability_envelope(envelope)?;

    match envelope.backend_family {
        Some(value) if value == spec.backend_family => {}
        Some(_) => return Err("compute_product_backend_family_mismatch".to_string()),
        None => return Err("compute_product_backend_family_missing".to_string()),
    }
    match envelope.execution_kind {
        Some(value) if value == spec.execution_kind => {}
        Some(_) => return Err("compute_product_execution_kind_invalid".to_string()),
        None => return Err("compute_product_execution_kind_missing".to_string()),
    }
    match envelope.compute_family {
        Some(value) if value == spec.compute_family => {}
        Some(_) => return Err("compute_product_compute_family_mismatch".to_string()),
        None => return Err("compute_product_compute_family_missing".to_string()),
    }

    let has_model_identity = envelope
        .model_family
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        || envelope
            .model_policy
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
    if !has_model_identity {
        return Err("compute_product_model_identity_missing".to_string());
    }

    if let Some(host_capability) = envelope.host_capability.as_ref() {
        if host_capability.accelerator_family.is_some()
            && host_capability
                .accelerator_vendor
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
        {
            return Err("compute_product_host_accelerator_vendor_missing".to_string());
        }
        if host_capability.memory_gb == Some(0) {
            return Err("compute_product_host_memory_gb_invalid".to_string());
        }
    }

    match spec.backend_family {
        ComputeBackendFamily::GptOss => {
            if envelope.gpt_oss_runtime.is_none() {
                return Err("compute_product_gpt_oss_runtime_missing".to_string());
            }
        }
        ComputeBackendFamily::AppleFoundationModels => {
            let Some(apple_platform) = envelope.apple_platform.as_ref() else {
                return Err("compute_product_apple_platform_gates_missing".to_string());
            };
            if !apple_platform.apple_silicon_required {
                return Err("compute_product_apple_silicon_requirement_missing".to_string());
            }
        }
    }

    Ok(spec)
}

#[cfg(test)]
mod tests {
    use super::{
        COMPUTE_LAUNCH_TAXONOMY_VERSION, ComputeBackendFamily, ComputeCapabilityEnvelope,
        ComputeCheckpointBinding, ComputeEnvironmentArtifactExpectation, ComputeEnvironmentBinding,
        ComputeEnvironmentDatasetBinding, ComputeEnvironmentHarness, ComputeEnvironmentPackage,
        ComputeEnvironmentPackageStatus, ComputeEnvironmentRubricBinding,
        ComputeEvaluationArtifact, ComputeEvaluationMetric, ComputeEvaluationSampleStatus,
        ComputeEvaluationSummary, ComputeExecutionKind, ComputeFamily, ComputeHostCapability,
        ComputeProduct, ComputeProductStatus, ComputeProofPosture, ComputeProvisioningKind,
        ComputeSettlementMode, ComputeSyntheticDataJob, ComputeSyntheticDataJobStatus,
        ComputeSyntheticDataSample, ComputeSyntheticDataSampleStatus, ComputeTopologyKind,
        ComputeValidatorRequirements, DeliveryProof, DeliveryProofStatus, DeliverySandboxEvidence,
        DeliveryTopologyEvidence, DeliveryVerificationEvidence, GptOssRuntimeCapability,
        PSIONIC_LOCAL_APPLE_FM_INFERENCE_PRODUCT_ID, PSIONIC_LOCAL_GPT_OSS_EMBEDDINGS_PRODUCT_ID,
        PSIONIC_LOCAL_GPT_OSS_INFERENCE_PRODUCT_ID, canonical_compute_product_id,
        validate_compute_capability_envelope, validate_compute_environment_package,
        validate_compute_synthetic_data_job, validate_compute_synthetic_data_sample,
        validate_delivery_proof, validate_launch_compute_product,
    };
    use serde_json::json;

    fn launch_product(product_id: &str) -> ComputeProduct {
        ComputeProduct {
            product_id: product_id.to_string(),
            resource_class: "compute".to_string(),
            capacity_unit: "request".to_string(),
            window_spec: "1h".to_string(),
            region_spec: vec!["global".to_string()],
            performance_band: Some("balanced".to_string()),
            sla_terms_ref: Some("sla.compute.launch".to_string()),
            cost_proof_required: false,
            attestation_required: false,
            settlement_mode: ComputeSettlementMode::Physical,
            index_eligible: true,
            status: ComputeProductStatus::Active,
            version: "v1".to_string(),
            created_at_ms: 1_700_000_000_000,
            taxonomy_version: Some(COMPUTE_LAUNCH_TAXONOMY_VERSION.to_string()),
            capability_envelope: Some(ComputeCapabilityEnvelope {
                backend_family: Some(ComputeBackendFamily::GptOss),
                execution_kind: Some(ComputeExecutionKind::LocalInference),
                compute_family: Some(ComputeFamily::Inference),
                topology_kind: Some(ComputeTopologyKind::SingleNode),
                provisioning_kind: Some(ComputeProvisioningKind::DesktopLocal),
                proof_posture: Some(ComputeProofPosture::DeliveryProofOnly),
                validator_requirements: None,
                artifact_residency: None,
                environment_binding: None,
                checkpoint_binding: None,
                model_policy: Some("text-generation".to_string()),
                model_family: Some("llama3.3".to_string()),
                host_capability: Some(ComputeHostCapability {
                    accelerator_vendor: Some("nvidia".to_string()),
                    accelerator_family: Some("h100".to_string()),
                    memory_gb: Some(80),
                }),
                apple_platform: None,
                gpt_oss_runtime: Some(GptOssRuntimeCapability {
                    runtime_ready: Some(true),
                    model_name: Some("llama3.3".to_string()),
                    quantization: Some("q4_k_m".to_string()),
                }),
                latency_ms_p50: Some(400),
                throughput_per_minute: Some(1_200),
                concurrency_limit: Some(2),
            }),
            metadata: json!({
                "summary": "Launch compute product"
            }),
        }
    }

    fn environment_package() -> ComputeEnvironmentPackage {
        ComputeEnvironmentPackage {
            environment_ref: "env.openagents.math.basic".to_string(),
            version: "2026.03.13".to_string(),
            family: "evaluation".to_string(),
            display_name: "OpenAgents Math Basic".to_string(),
            owner_id: "openagents".to_string(),
            created_at_ms: 1_762_000_400_000,
            updated_at_ms: 1_762_000_401_000,
            status: ComputeEnvironmentPackageStatus::Active,
            description: Some("Reference math eval harness".to_string()),
            package_digest: Some("sha256:env.math.basic".to_string()),
            dataset_bindings: vec![ComputeEnvironmentDatasetBinding {
                dataset_ref: "dataset://math/basic".to_string(),
                split_ref: Some("validation".to_string()),
                mount_path: Some("/datasets/math/basic".to_string()),
                integrity_ref: Some("sha256:dataset.math.basic".to_string()),
                access_policy_ref: Some("policy://dataset/math/basic".to_string()),
                required: true,
                metadata: json!({"format": "jsonl"}),
            }],
            harness: Some(ComputeEnvironmentHarness {
                harness_ref: "harness://openagents/math/basic".to_string(),
                runtime_family: "rust-native".to_string(),
                entrypoint: Some("oa-eval-harness".to_string()),
                args: vec!["--suite".to_string(), "math-basic".to_string()],
                sandbox_profile_ref: Some("sandbox://strict".to_string()),
                evaluator_policy_ref: Some("policy://eval/math/basic".to_string()),
                time_budget_ms: Some(300_000),
                metadata: json!({"max_concurrency": 4}),
            }),
            rubric_bindings: vec![ComputeEnvironmentRubricBinding {
                rubric_ref: "rubric://math/basic".to_string(),
                score_type: Some("accuracy".to_string()),
                pass_threshold_bps: Some(9_000),
                metadata: json!({"top_k": 1}),
            }],
            expected_artifacts: vec![ComputeEnvironmentArtifactExpectation {
                artifact_kind: "scorecard".to_string(),
                artifact_ref: Some("artifact://math/basic/scorecard".to_string()),
                required: true,
                verification_policy_ref: Some("policy://artifact/scorecard".to_string()),
                metadata: json!({"schema": "v1"}),
            }],
            policy_refs: vec![
                "policy://eval/math/basic".to_string(),
                "policy://artifact/scorecard".to_string(),
            ],
            metadata: json!({"tier": "reference"}),
        }
    }

    fn synthetic_data_job() -> ComputeSyntheticDataJob {
        ComputeSyntheticDataJob {
            synthetic_job_id: "synthetic.math.basic.alpha".to_string(),
            environment_binding: ComputeEnvironmentBinding {
                environment_ref: "env.openagents.math.basic".to_string(),
                environment_version: Some("2026.03.13".to_string()),
                dataset_ref: Some("dataset://math/basic".to_string()),
                rubric_ref: Some("rubric://math/basic".to_string()),
                evaluator_policy_ref: Some("policy://eval/math/basic".to_string()),
            },
            teacher_model_ref: "model://llama3.3-instruct".to_string(),
            generation_product_id: Some(PSIONIC_LOCAL_GPT_OSS_INFERENCE_PRODUCT_ID.to_string()),
            generation_delivery_proof_id: Some("delivery.synthetic.alpha".to_string()),
            output_artifact_ref: Some("artifact://synthetic/math/basic/output".to_string()),
            created_at_ms: 1_762_000_500_000,
            generated_at_ms: Some(1_762_000_501_000),
            verification_eval_run_id: Some("eval.synthetic.alpha".to_string()),
            verified_at_ms: Some(1_762_000_502_000),
            target_sample_count: Some(2),
            status: ComputeSyntheticDataJobStatus::Verified,
            verification_summary: Some(ComputeEvaluationSummary {
                total_samples: 2,
                scored_samples: 2,
                passed_samples: 1,
                failed_samples: 1,
                errored_samples: 0,
                average_score_bps: Some(9_250),
                pass_rate_bps: Some(5_000),
                aggregate_metrics: vec![ComputeEvaluationMetric {
                    metric_id: "accuracy".to_string(),
                    metric_value: 0.925,
                    unit: Some("fraction".to_string()),
                    metadata: json!({"split": "synthetic"}),
                }],
                artifacts: vec![ComputeEvaluationArtifact {
                    artifact_kind: "verification_scorecard".to_string(),
                    artifact_ref: "artifact://synthetic/math/basic/scorecard".to_string(),
                    digest: Some("sha256:synthetic-scorecard".to_string()),
                    metadata: json!({"schema": "v1"}),
                }],
            }),
            metadata: json!({"pipeline": "teacher-verify"}),
        }
    }

    fn synthetic_data_sample() -> ComputeSyntheticDataSample {
        ComputeSyntheticDataSample {
            synthetic_job_id: "synthetic.math.basic.alpha".to_string(),
            sample_id: "sample.alpha".to_string(),
            ordinal: Some(1),
            prompt_ref: "artifact://synthetic/prompts/sample.alpha".to_string(),
            output_ref: "artifact://synthetic/outputs/sample.alpha".to_string(),
            generation_config_ref: Some("config://synthetic/default".to_string()),
            generator_machine_ref: Some("machine://provider.alpha/gpu0".to_string()),
            verification_eval_sample_id: Some("sample.alpha".to_string()),
            verification_status: Some(ComputeEvaluationSampleStatus::Passed),
            verification_score_bps: Some(9_500),
            status: ComputeSyntheticDataSampleStatus::Verified,
            recorded_at_ms: 1_762_000_501_000,
            metadata: json!({"prompt_tokens": 64}),
        }
    }

    #[test]
    fn validates_launch_gpt_oss_product() {
        let product = launch_product(PSIONIC_LOCAL_GPT_OSS_INFERENCE_PRODUCT_ID);
        let spec =
            validate_launch_compute_product(&product).expect("launch product should validate");
        assert_eq!(spec.product_id, PSIONIC_LOCAL_GPT_OSS_INFERENCE_PRODUCT_ID);
        assert_eq!(spec.backend_family, ComputeBackendFamily::GptOss);
        assert_eq!(spec.compute_family, ComputeFamily::Inference);
    }

    #[test]
    fn rejects_gpt_oss_embeddings_product() {
        let mut product = launch_product(PSIONIC_LOCAL_GPT_OSS_INFERENCE_PRODUCT_ID);
        product.product_id = PSIONIC_LOCAL_GPT_OSS_EMBEDDINGS_PRODUCT_ID.to_string();
        product.capability_envelope = Some(ComputeCapabilityEnvelope {
            backend_family: Some(ComputeBackendFamily::GptOss),
            execution_kind: Some(ComputeExecutionKind::LocalInference),
            compute_family: Some(ComputeFamily::Embeddings),
            topology_kind: Some(ComputeTopologyKind::SingleNode),
            provisioning_kind: Some(ComputeProvisioningKind::DesktopLocal),
            proof_posture: Some(ComputeProofPosture::DeliveryProofOnly),
            validator_requirements: None,
            artifact_residency: None,
            environment_binding: None,
            checkpoint_binding: None,
            model_policy: Some("embeddings".to_string()),
            model_family: Some("nomic-embed".to_string()),
            host_capability: Some(ComputeHostCapability {
                accelerator_vendor: Some("nvidia".to_string()),
                accelerator_family: Some("h100".to_string()),
                memory_gb: Some(80),
            }),
            apple_platform: None,
            gpt_oss_runtime: Some(GptOssRuntimeCapability {
                runtime_ready: Some(true),
                model_name: Some("nomic-embed".to_string()),
                quantization: Some("q4_k_m".to_string()),
            }),
            latency_ms_p50: Some(150),
            throughput_per_minute: Some(600),
            concurrency_limit: Some(1),
        });
        let err = validate_launch_compute_product(&product)
            .expect_err("gpt_oss embeddings should be rejected");
        assert_eq!(err, "compute_product_launch_product_id_unsupported");
    }

    #[test]
    fn validates_environment_and_checkpoint_bound_launch_product() {
        let mut product = launch_product(PSIONIC_LOCAL_GPT_OSS_INFERENCE_PRODUCT_ID);
        product.capability_envelope = Some(ComputeCapabilityEnvelope {
            backend_family: Some(ComputeBackendFamily::GptOss),
            execution_kind: Some(ComputeExecutionKind::LocalInference),
            compute_family: Some(ComputeFamily::Inference),
            topology_kind: Some(ComputeTopologyKind::SingleNode),
            provisioning_kind: Some(ComputeProvisioningKind::DesktopLocal),
            proof_posture: Some(ComputeProofPosture::DeliveryProofOnly),
            validator_requirements: None,
            artifact_residency: None,
            environment_binding: Some(ComputeEnvironmentBinding {
                environment_ref: "env://math/basic".to_string(),
                environment_version: Some("v1".to_string()),
                dataset_ref: Some("dataset://math/basic".to_string()),
                rubric_ref: Some("rubric://math/basic".to_string()),
                evaluator_policy_ref: Some("policy://eval/basic".to_string()),
            }),
            checkpoint_binding: Some(ComputeCheckpointBinding {
                checkpoint_family: "decoder".to_string(),
                latest_checkpoint_ref: Some("checkpoint://decoder/latest".to_string()),
                recovery_posture: Some("warm-resume".to_string()),
            }),
            model_policy: Some("text-generation".to_string()),
            model_family: Some("llama3.3".to_string()),
            host_capability: Some(ComputeHostCapability {
                accelerator_vendor: Some("nvidia".to_string()),
                accelerator_family: Some("h100".to_string()),
                memory_gb: Some(80),
            }),
            apple_platform: None,
            gpt_oss_runtime: Some(GptOssRuntimeCapability {
                runtime_ready: Some(true),
                model_name: Some("llama3.3".to_string()),
                quantization: Some("q4_k_m".to_string()),
            }),
            latency_ms_p50: Some(400),
            throughput_per_minute: Some(1_200),
            concurrency_limit: Some(2),
        });
        validate_launch_compute_product(&product)
            .expect("environment and checkpoint bindings should remain launch-valid");
    }

    #[test]
    fn canonical_compute_product_ids_preserve_legacy_aliases() {
        assert_eq!(
            canonical_compute_product_id("gpt_oss.text_generation"),
            Some(PSIONIC_LOCAL_GPT_OSS_INFERENCE_PRODUCT_ID)
        );
        assert_eq!(
            canonical_compute_product_id("ollama.text_generation"),
            Some(PSIONIC_LOCAL_GPT_OSS_INFERENCE_PRODUCT_ID)
        );
        assert_eq!(
            canonical_compute_product_id("apple_foundation_models.text_generation"),
            Some(PSIONIC_LOCAL_APPLE_FM_INFERENCE_PRODUCT_ID)
        );
        assert_eq!(
            canonical_compute_product_id("gpt_oss.embeddings"),
            Some(PSIONIC_LOCAL_GPT_OSS_EMBEDDINGS_PRODUCT_ID)
        );
    }

    #[test]
    fn rejects_challenge_posture_without_validator_requirements() {
        let envelope = ComputeCapabilityEnvelope {
            proof_posture: Some(ComputeProofPosture::ChallengeEligible),
            ..ComputeCapabilityEnvelope::default()
        };
        let err = validate_compute_capability_envelope(&envelope)
            .expect_err("challenge posture should require validator requirements");
        assert_eq!(
            err,
            "compute_challenge_posture_requires_validator_requirements"
        );
    }

    #[test]
    fn rejects_empty_environment_binding_ref() {
        let envelope = ComputeCapabilityEnvelope {
            environment_binding: Some(ComputeEnvironmentBinding {
                environment_ref: "   ".to_string(),
                ..ComputeEnvironmentBinding::default()
            }),
            ..ComputeCapabilityEnvelope::default()
        };
        let err = validate_compute_capability_envelope(&envelope)
            .expect_err("environment binding should require a non-empty ref");
        assert_eq!(err, "compute_environment_binding_ref_missing");
    }

    #[test]
    fn validates_environment_package_contract() {
        let package = environment_package();
        validate_compute_environment_package(&package)
            .expect("environment package should validate");
    }

    #[test]
    fn rejects_environment_package_with_missing_dataset_ref() {
        let mut package = environment_package();
        package.dataset_bindings[0].dataset_ref = "   ".to_string();
        let err = validate_compute_environment_package(&package)
            .expect_err("dataset ref should be required");
        assert_eq!(err, "compute_environment_dataset_ref_missing");
    }

    #[test]
    fn validates_synthetic_data_job_and_sample_contracts() {
        validate_compute_synthetic_data_job(&synthetic_data_job())
            .expect("synthetic job should validate");
        validate_compute_synthetic_data_sample(&synthetic_data_sample())
            .expect("synthetic sample should validate");
    }

    #[test]
    fn rejects_verified_synthetic_job_without_eval_run_reference() {
        let mut job = synthetic_data_job();
        job.verification_eval_run_id = None;
        let err = validate_compute_synthetic_data_job(&job)
            .expect_err("verified jobs should require eval run linkage");
        assert_eq!(err, "compute_synthetic_verification_eval_run_id_missing");
    }

    #[test]
    fn rejects_verified_synthetic_sample_without_verification_status() {
        let mut sample = synthetic_data_sample();
        sample.status = ComputeSyntheticDataSampleStatus::Rejected;
        sample.verification_status = None;
        let err = validate_compute_synthetic_data_sample(&sample)
            .expect_err("verified synthetic samples should require verification status");
        assert_eq!(err, "compute_synthetic_verification_status_missing");
    }

    #[test]
    fn rejects_zero_validator_count() {
        let envelope = ComputeCapabilityEnvelope {
            validator_requirements: Some(ComputeValidatorRequirements {
                validator_pool_ref: Some("validator://pool/alpha".to_string()),
                policy_ref: None,
                minimum_validator_count: Some(0),
                challenge_window_ms: Some(30_000),
            }),
            ..ComputeCapabilityEnvelope::default()
        };
        let err = validate_compute_capability_envelope(&envelope)
            .expect_err("validator count must be positive");
        assert_eq!(err, "compute_validator_count_invalid");
    }

    #[test]
    fn validates_clustered_delivery_proof_with_topology_and_validator_refs() {
        let proof = DeliveryProof {
            delivery_proof_id: "delivery.cluster.alpha".to_string(),
            capacity_lot_id: "lot.cluster.alpha".to_string(),
            product_id: "psionic.cluster.inference".to_string(),
            instrument_id: Some("instrument.cluster.alpha".to_string()),
            contract_id: None,
            created_at_ms: 1_700_000_000_500,
            metered_quantity: 128,
            accepted_quantity: 128,
            performance_band_observed: Some("clustered".to_string()),
            variance_reason: None,
            variance_reason_detail: None,
            attestation_digest: Some("sha256:cluster".to_string()),
            cost_attestation_ref: Some("cost:cluster".to_string()),
            status: DeliveryProofStatus::Accepted,
            rejection_reason: None,
            topology_evidence: Some(DeliveryTopologyEvidence {
                topology_kind: Some(ComputeTopologyKind::Replicated),
                topology_digest: Some("topology:replicated".to_string()),
                scheduler_node_ref: Some("node://scheduler/a".to_string()),
                transport_class: Some("wider_network_stream".to_string()),
                selected_node_refs: vec!["node://worker/a".to_string()],
                replica_node_refs: vec!["node://worker/b".to_string()],
            }),
            sandbox_evidence: None,
            verification_evidence: Some(DeliveryVerificationEvidence {
                proof_bundle_ref: Some("proof_bundle:cluster".to_string()),
                activation_fingerprint_ref: None,
                validator_pool_ref: Some("validators.alpha".to_string()),
                validator_run_ref: Some("validator_run:cluster".to_string()),
                challenge_result_refs: vec!["validator_challenge_result:ok".to_string()],
                environment_ref: None,
                environment_version: None,
                eval_run_ref: None,
            }),
            promised_capability_envelope: Some(ComputeCapabilityEnvelope {
                backend_family: Some(ComputeBackendFamily::GptOss),
                execution_kind: Some(ComputeExecutionKind::ClusteredInference),
                compute_family: Some(ComputeFamily::Inference),
                topology_kind: Some(ComputeTopologyKind::Replicated),
                provisioning_kind: Some(ComputeProvisioningKind::ClusterAttached),
                proof_posture: Some(ComputeProofPosture::ChallengeEligible),
                validator_requirements: Some(ComputeValidatorRequirements {
                    validator_pool_ref: Some("validators.alpha".to_string()),
                    policy_ref: Some("policy.validators.alpha".to_string()),
                    minimum_validator_count: Some(2),
                    challenge_window_ms: Some(30_000),
                }),
                artifact_residency: None,
                environment_binding: None,
                checkpoint_binding: None,
                model_policy: Some("cluster.inference".to_string()),
                model_family: Some("gpt-oss".to_string()),
                host_capability: None,
                apple_platform: None,
                gpt_oss_runtime: Some(GptOssRuntimeCapability {
                    runtime_ready: Some(true),
                    model_name: Some("gpt-oss".to_string()),
                    quantization: Some("q4".to_string()),
                }),
                latency_ms_p50: Some(80),
                throughput_per_minute: Some(4_800),
                concurrency_limit: Some(8),
            }),
            observed_capability_envelope: None,
            metadata: json!({}),
        };
        validate_delivery_proof(&proof).expect("cluster delivery proof should validate");
    }

    #[test]
    fn rejects_toploc_delivery_without_activation_fingerprint_ref() {
        let proof = DeliveryProof {
            delivery_proof_id: "delivery.toploc.alpha".to_string(),
            capacity_lot_id: "lot.toploc.alpha".to_string(),
            product_id: "psionic.embeddings".to_string(),
            instrument_id: None,
            contract_id: None,
            created_at_ms: 1_700_000_000_600,
            metered_quantity: 32,
            accepted_quantity: 32,
            performance_band_observed: None,
            variance_reason: None,
            variance_reason_detail: None,
            attestation_digest: None,
            cost_attestation_ref: None,
            status: DeliveryProofStatus::Accepted,
            rejection_reason: None,
            topology_evidence: None,
            sandbox_evidence: None,
            verification_evidence: Some(DeliveryVerificationEvidence {
                proof_bundle_ref: Some("proof_bundle:embed".to_string()),
                activation_fingerprint_ref: None,
                validator_pool_ref: None,
                validator_run_ref: None,
                challenge_result_refs: Vec::new(),
                environment_ref: None,
                environment_version: None,
                eval_run_ref: None,
            }),
            promised_capability_envelope: Some(ComputeCapabilityEnvelope {
                backend_family: Some(ComputeBackendFamily::GptOss),
                execution_kind: Some(ComputeExecutionKind::LocalInference),
                compute_family: Some(ComputeFamily::Embeddings),
                topology_kind: Some(ComputeTopologyKind::SingleNode),
                provisioning_kind: Some(ComputeProvisioningKind::DesktopLocal),
                proof_posture: Some(ComputeProofPosture::ToplocAugmented),
                validator_requirements: None,
                artifact_residency: None,
                environment_binding: None,
                checkpoint_binding: None,
                model_policy: Some("embeddings".to_string()),
                model_family: Some("nomic-embed".to_string()),
                host_capability: None,
                apple_platform: None,
                gpt_oss_runtime: Some(GptOssRuntimeCapability {
                    runtime_ready: Some(true),
                    model_name: Some("nomic-embed".to_string()),
                    quantization: Some("q4".to_string()),
                }),
                latency_ms_p50: Some(20),
                throughput_per_minute: Some(2_400),
                concurrency_limit: Some(4),
            }),
            observed_capability_envelope: None,
            metadata: json!({}),
        };
        let err = validate_delivery_proof(&proof)
            .expect_err("toploc delivery should require an activation fingerprint ref");
        assert_eq!(err, "delivery_proof_activation_fingerprint_ref_missing");
    }

    #[test]
    fn rejects_sandbox_delivery_without_sandbox_refs() {
        let proof = DeliveryProof {
            delivery_proof_id: "delivery.sandbox.alpha".to_string(),
            capacity_lot_id: "lot.sandbox.alpha".to_string(),
            product_id: "psionic.sandbox_execution".to_string(),
            instrument_id: None,
            contract_id: None,
            created_at_ms: 1_700_000_000_700,
            metered_quantity: 4,
            accepted_quantity: 4,
            performance_band_observed: None,
            variance_reason: None,
            variance_reason_detail: None,
            attestation_digest: None,
            cost_attestation_ref: None,
            status: DeliveryProofStatus::Accepted,
            rejection_reason: None,
            topology_evidence: None,
            sandbox_evidence: Some(DeliverySandboxEvidence {
                sandbox_profile_ref: None,
                sandbox_execution_ref: None,
                command_digest: Some("command:digest".to_string()),
                environment_digest: Some("env:digest".to_string()),
                input_artifact_refs: vec!["artifact://input/a".to_string()],
                output_artifact_refs: vec!["artifact://output/a".to_string()],
            }),
            verification_evidence: None,
            promised_capability_envelope: Some(ComputeCapabilityEnvelope {
                backend_family: Some(ComputeBackendFamily::GptOss),
                execution_kind: Some(ComputeExecutionKind::SandboxExecution),
                compute_family: Some(ComputeFamily::SandboxExecution),
                topology_kind: Some(ComputeTopologyKind::SandboxIsolated),
                provisioning_kind: Some(ComputeProvisioningKind::RemoteSandbox),
                proof_posture: Some(ComputeProofPosture::TopologyAndDelivery),
                validator_requirements: None,
                artifact_residency: None,
                environment_binding: None,
                checkpoint_binding: None,
                model_policy: Some("sandbox.exec".to_string()),
                model_family: None,
                host_capability: None,
                apple_platform: None,
                gpt_oss_runtime: None,
                latency_ms_p50: None,
                throughput_per_minute: None,
                concurrency_limit: Some(1),
            }),
            observed_capability_envelope: None,
            metadata: json!({}),
        };
        let err = validate_delivery_proof(&proof)
            .expect_err("sandbox delivery should require explicit sandbox refs");
        assert_eq!(err, "delivery_proof_sandbox_profile_ref_missing");
    }
}
