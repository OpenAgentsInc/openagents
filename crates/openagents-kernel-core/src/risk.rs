use crate::receipts::{Money, VerificationTier};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CoverageOfferStatus {
    #[default]
    Open,
    Bound,
    Cancelled,
    Expired,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CoverageBindingStatus {
    #[default]
    Active,
    Triggered,
    Settled,
    Expired,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PredictionSide {
    #[default]
    Pass,
    Fail,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PredictionPositionStatus {
    #[default]
    Open,
    Resolved,
    Cancelled,
    Expired,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RiskClaimStatus {
    #[default]
    Open,
    Approved,
    Denied,
    Paid,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RiskSignalStatus {
    #[default]
    Active,
    Superseded,
    Expired,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CoverageOffer {
    pub offer_id: String,
    pub outcome_ref: String,
    #[serde(default)]
    pub contract_id: Option<String>,
    pub underwriter_id: String,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
    pub coverage_cap: Money,
    pub premium: Money,
    #[serde(default)]
    pub deductible: Option<Money>,
    #[serde(default)]
    pub status: CoverageOfferStatus,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CoverageBinding {
    pub binding_id: String,
    pub outcome_ref: String,
    #[serde(default)]
    pub contract_id: Option<String>,
    #[serde(default)]
    pub offer_ids: Vec<String>,
    pub created_at_ms: i64,
    #[serde(default)]
    pub warranty_window_end_ms: Option<i64>,
    pub total_coverage: Money,
    pub premium_total: Money,
    #[serde(default)]
    pub status: CoverageBindingStatus,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PredictionPosition {
    pub position_id: String,
    pub outcome_ref: String,
    pub participant_id: String,
    pub side: PredictionSide,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
    pub collateral: Money,
    pub max_payout: Money,
    #[serde(default)]
    pub status: PredictionPositionStatus,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RiskClaim {
    pub claim_id: String,
    pub binding_id: String,
    pub outcome_ref: String,
    pub claimant_id: String,
    pub created_at_ms: i64,
    pub requested_payout: Money,
    #[serde(default)]
    pub approved_payout: Option<Money>,
    #[serde(default)]
    pub resolution_ref: Option<String>,
    pub reason_code: String,
    #[serde(default)]
    pub status: RiskClaimStatus,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RiskSignal {
    pub signal_id: String,
    pub outcome_ref: String,
    pub created_at_ms: i64,
    pub implied_fail_probability_bps: u32,
    pub calibration_score: f64,
    pub coverage_concentration_hhi: f64,
    #[serde(default)]
    pub verification_tier_floor: Option<VerificationTier>,
    pub collateral_multiplier_bps: u32,
    pub autonomy_mode: String,
    #[serde(default)]
    pub status: RiskSignalStatus,
    #[serde(default)]
    pub metadata: Value,
}
