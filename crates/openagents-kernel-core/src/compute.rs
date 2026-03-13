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
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeFamily {
    Inference,
    Embeddings,
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
    pub promised_capability_envelope: Option<ComputeCapabilityEnvelope>,
    #[serde(default)]
    pub observed_capability_envelope: Option<ComputeCapabilityEnvelope>,
    #[serde(default)]
    pub metadata: Value,
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
    match product_id {
        "ollama.text_generation" | "gpt_oss.text_generation" => Some(LaunchComputeProductSpec {
            product_id: "gpt_oss.text_generation",
            backend_family: ComputeBackendFamily::GptOss,
            execution_kind: ComputeExecutionKind::LocalInference,
            compute_family: ComputeFamily::Inference,
        }),
        "ollama.embeddings" | "gpt_oss.embeddings" => Some(LaunchComputeProductSpec {
            product_id: "gpt_oss.embeddings",
            backend_family: ComputeBackendFamily::GptOss,
            execution_kind: ComputeExecutionKind::LocalInference,
            compute_family: ComputeFamily::Embeddings,
        }),
        "apple_foundation_models.text_generation" => Some(LaunchComputeProductSpec {
            product_id: "apple_foundation_models.text_generation",
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
        ApplePlatformCapability, COMPUTE_LAUNCH_TAXONOMY_VERSION, ComputeBackendFamily,
        ComputeCapabilityEnvelope, ComputeExecutionKind, ComputeFamily, ComputeHostCapability,
        ComputeProduct, ComputeProductStatus, ComputeSettlementMode, GptOssRuntimeCapability,
        validate_launch_compute_product,
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

    #[test]
    fn validates_launch_gpt_oss_product() {
        let product = launch_product("gpt_oss.text_generation");
        let spec =
            validate_launch_compute_product(&product).expect("launch product should validate");
        assert_eq!(spec.product_id, "gpt_oss.text_generation");
        assert_eq!(spec.backend_family, ComputeBackendFamily::GptOss);
        assert_eq!(spec.compute_family, ComputeFamily::Inference);
    }

    #[test]
    fn rejects_apple_embeddings_product() {
        let mut product = launch_product("apple_foundation_models.text_generation");
        product.product_id = "apple_foundation_models.embeddings".to_string();
        product.capability_envelope = Some(ComputeCapabilityEnvelope {
            backend_family: Some(ComputeBackendFamily::AppleFoundationModels),
            execution_kind: Some(ComputeExecutionKind::LocalInference),
            compute_family: Some(ComputeFamily::Embeddings),
            model_policy: Some("embeddings".to_string()),
            model_family: Some("apple.foundation".to_string()),
            host_capability: Some(ComputeHostCapability {
                accelerator_vendor: Some("apple".to_string()),
                accelerator_family: Some("m4_max".to_string()),
                memory_gb: Some(64),
            }),
            apple_platform: Some(ApplePlatformCapability {
                apple_silicon_required: true,
                apple_intelligence_required: true,
                apple_intelligence_available: Some(true),
                minimum_macos_version: Some("15.1".to_string()),
            }),
            gpt_oss_runtime: None,
            latency_ms_p50: Some(150),
            throughput_per_minute: Some(600),
            concurrency_limit: Some(1),
        });
        let err = validate_launch_compute_product(&product)
            .expect_err("apple embeddings should be rejected");
        assert_eq!(err, "compute_product_launch_product_id_unsupported");
    }
}
