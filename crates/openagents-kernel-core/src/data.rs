use crate::receipts::Money;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DataAssetStatus {
    #[default]
    Active,
    Disabled,
    Retired,
}

impl DataAssetStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
            Self::Retired => "retired",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AccessGrantStatus {
    #[default]
    Offered,
    Accepted,
    Delivered,
    Revoked,
    Refunded,
    Expired,
}

impl AccessGrantStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Offered => "offered",
            Self::Accepted => "accepted",
            Self::Delivered => "delivered",
            Self::Revoked => "revoked",
            Self::Refunded => "refunded",
            Self::Expired => "expired",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryBundleStatus {
    #[default]
    Issued,
    Accessed,
    Revoked,
    Expired,
}

impl DeliveryBundleStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Issued => "issued",
            Self::Accessed => "accessed",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RevocationStatus {
    #[default]
    Revoked,
    Refunded,
}

impl RevocationStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Revoked => "revoked",
            Self::Refunded => "refunded",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PermissionPolicy {
    pub policy_id: String,
    #[serde(default)]
    pub allowed_scopes: Vec<String>,
    #[serde(default)]
    pub allowed_tool_tags: Vec<String>,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    #[serde(default)]
    pub export_allowed: bool,
    #[serde(default)]
    pub derived_outputs_allowed: bool,
    #[serde(default)]
    pub retention_seconds: Option<u64>,
    #[serde(default)]
    pub max_bundle_size_bytes: Option<u64>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DataAsset {
    pub asset_id: String,
    pub provider_id: String,
    pub asset_kind: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub content_digest: Option<String>,
    #[serde(default)]
    pub provenance_ref: Option<String>,
    #[serde(default)]
    pub default_policy: Option<PermissionPolicy>,
    #[serde(default)]
    pub price_hint: Option<Money>,
    pub created_at_ms: i64,
    #[serde(default)]
    pub status: DataAssetStatus,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct AccessGrant {
    pub grant_id: String,
    pub asset_id: String,
    pub provider_id: String,
    #[serde(default)]
    pub consumer_id: Option<String>,
    pub permission_policy: PermissionPolicy,
    #[serde(default)]
    pub offer_price: Option<Money>,
    #[serde(default)]
    pub warranty_window_ms: Option<u64>,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
    #[serde(default)]
    pub accepted_at_ms: Option<i64>,
    #[serde(default)]
    pub status: AccessGrantStatus,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DeliveryBundle {
    pub delivery_bundle_id: String,
    pub asset_id: String,
    pub grant_id: String,
    pub provider_id: String,
    pub consumer_id: String,
    pub created_at_ms: i64,
    pub delivery_ref: String,
    #[serde(default)]
    pub delivery_digest: Option<String>,
    #[serde(default)]
    pub bundle_size_bytes: Option<u64>,
    #[serde(default)]
    pub manifest_refs: Vec<String>,
    #[serde(default)]
    pub expires_at_ms: Option<i64>,
    #[serde(default)]
    pub status: DeliveryBundleStatus,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RevocationReceipt {
    pub revocation_id: String,
    pub asset_id: String,
    pub grant_id: String,
    pub provider_id: String,
    #[serde(default)]
    pub consumer_id: Option<String>,
    pub created_at_ms: i64,
    pub reason_code: String,
    #[serde(default)]
    pub refund_amount: Option<Money>,
    #[serde(default)]
    pub revoked_delivery_bundle_ids: Vec<String>,
    #[serde(default)]
    pub replacement_delivery_bundle_id: Option<String>,
    #[serde(default)]
    pub status: RevocationStatus,
    #[serde(default)]
    pub metadata: Value,
}
