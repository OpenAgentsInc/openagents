use crate::receipts::Money;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryProofStatus {
    #[default]
    Recorded,
    Accepted,
    Rejected,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeIndexStatus {
    #[default]
    Published,
    Superseded,
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
    pub variance_reason: Option<String>,
    #[serde(default)]
    pub attestation_digest: Option<String>,
    #[serde(default)]
    pub cost_attestation_ref: Option<String>,
    #[serde(default)]
    pub status: DeliveryProofStatus,
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
    pub metadata: Value,
}
