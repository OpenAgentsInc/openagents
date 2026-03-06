use crate::receipts::{Money, ReceiptRef};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum QuoteStatus {
    #[default]
    Quoted,
    Selected,
    Expired,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RoutePlanStatus {
    #[default]
    Selected,
    Executing,
    Settled,
    Failed,
    Refunded,
    Expired,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeStatus {
    #[default]
    Issued,
    Reserved,
    Consumed,
    Expired,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SettlementIntentStatus {
    #[default]
    Pending,
    Executing,
    Settled,
    Failed,
    Refunded,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReservePartitionStatus {
    #[default]
    Active,
    Adjusted,
    Exhausted,
    Released,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Quote {
    pub quote_id: String,
    pub requester_id: String,
    #[serde(default)]
    pub solver_id: Option<String>,
    pub route_kind: String,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
    pub source_amount: Money,
    #[serde(default)]
    pub expected_output: Option<Money>,
    #[serde(default)]
    pub fee_ceiling: Option<Money>,
    #[serde(default)]
    pub source_payment_pointer: Option<String>,
    #[serde(default)]
    pub destination_payment_pointer: Option<String>,
    #[serde(default)]
    pub status: QuoteStatus,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RoutePlan {
    pub route_plan_id: String,
    pub quote_id: String,
    pub requester_id: String,
    pub solver_id: String,
    pub route_kind: String,
    pub selected_at_ms: i64,
    pub expires_at_ms: i64,
    #[serde(default)]
    pub quoted_input: Option<Money>,
    #[serde(default)]
    pub quoted_output: Option<Money>,
    #[serde(default)]
    pub fee_ceiling: Option<Money>,
    #[serde(default)]
    pub route_hops: Vec<String>,
    #[serde(default)]
    pub quote_receipt: Option<ReceiptRef>,
    #[serde(default)]
    pub status: RoutePlanStatus,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Envelope {
    pub envelope_id: String,
    pub route_plan_id: String,
    pub quote_id: String,
    #[serde(default)]
    pub reserve_partition_id: Option<String>,
    pub owner_id: String,
    pub spend_limit: Money,
    #[serde(default)]
    pub reserved_amount: Option<Money>,
    #[serde(default)]
    pub fee_limit: Option<Money>,
    #[serde(default)]
    pub allowed_destinations: Vec<String>,
    pub issued_at_ms: i64,
    pub expires_at_ms: i64,
    #[serde(default)]
    pub status: EnvelopeStatus,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SettlementIntent {
    pub settlement_intent_id: String,
    pub route_plan_id: String,
    pub quote_id: String,
    pub envelope_id: String,
    #[serde(default)]
    pub reserve_partition_id: Option<String>,
    pub created_at_ms: i64,
    #[serde(default)]
    pub executed_at_ms: Option<i64>,
    pub source_amount: Money,
    #[serde(default)]
    pub settled_amount: Option<Money>,
    #[serde(default)]
    pub fee_paid: Option<Money>,
    #[serde(default)]
    pub settlement_proof_ref: Option<String>,
    #[serde(default)]
    pub reason_code: Option<String>,
    #[serde(default)]
    pub status: SettlementIntentStatus,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ReservePartition {
    pub partition_id: String,
    pub owner_id: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub total_amount: Money,
    pub available_amount: Money,
    pub reserved_amount: Money,
    #[serde(default)]
    pub status: ReservePartitionStatus,
    #[serde(default)]
    pub metadata: Value,
}
