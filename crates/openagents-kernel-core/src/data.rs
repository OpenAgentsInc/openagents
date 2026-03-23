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

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NostrPublicationRef {
    #[serde(default)]
    pub coordinate: Option<String>,
    #[serde(default)]
    pub event_id: Option<String>,
    #[serde(default)]
    pub relay_url: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataAssetNostrPublications {
    #[serde(default)]
    pub ds_listing: Option<NostrPublicationRef>,
    #[serde(default)]
    pub ds_draft_listing: Option<NostrPublicationRef>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessGrantNostrPublications {
    #[serde(default)]
    pub ds_offer: Option<NostrPublicationRef>,
    #[serde(default)]
    pub ds_access_request: Option<NostrPublicationRef>,
    #[serde(default)]
    pub ds_access_result: Option<NostrPublicationRef>,
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
    pub nostr_publications: DataAssetNostrPublications,
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
    pub nostr_publications: AccessGrantNostrPublications,
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataMarketSummary {
    pub total_assets: u32,
    pub active_assets: u32,
    pub total_grants: u32,
    pub offered_grants: u32,
    pub accepted_grants: u32,
    pub delivered_grants: u32,
    pub terminal_grants: u32,
    pub total_deliveries: u32,
    pub active_deliveries: u32,
    pub total_revocations: u32,
    #[serde(default)]
    pub latest_activity_at_ms: Option<i64>,
}

impl DataMarketSummary {
    pub fn from_parts(
        assets: &[DataAsset],
        grants: &[AccessGrant],
        deliveries: &[DeliveryBundle],
        revocations: &[RevocationReceipt],
    ) -> Self {
        let latest_asset_ms = assets.iter().map(|asset| asset.created_at_ms).max();
        let latest_grant_ms = grants
            .iter()
            .map(|grant| grant.accepted_at_ms.unwrap_or(grant.created_at_ms))
            .max();
        let latest_delivery_ms = deliveries.iter().map(|bundle| bundle.created_at_ms).max();
        let latest_revocation_ms = revocations
            .iter()
            .map(|receipt| receipt.created_at_ms)
            .max();

        Self {
            total_assets: assets.len().min(u32::MAX as usize) as u32,
            active_assets: assets
                .iter()
                .filter(|asset| matches!(asset.status, DataAssetStatus::Active))
                .count()
                .min(u32::MAX as usize) as u32,
            total_grants: grants.len().min(u32::MAX as usize) as u32,
            offered_grants: grants
                .iter()
                .filter(|grant| matches!(grant.status, AccessGrantStatus::Offered))
                .count()
                .min(u32::MAX as usize) as u32,
            accepted_grants: grants
                .iter()
                .filter(|grant| matches!(grant.status, AccessGrantStatus::Accepted))
                .count()
                .min(u32::MAX as usize) as u32,
            delivered_grants: grants
                .iter()
                .filter(|grant| matches!(grant.status, AccessGrantStatus::Delivered))
                .count()
                .min(u32::MAX as usize) as u32,
            terminal_grants: grants
                .iter()
                .filter(|grant| {
                    matches!(
                        grant.status,
                        AccessGrantStatus::Revoked
                            | AccessGrantStatus::Refunded
                            | AccessGrantStatus::Expired
                    )
                })
                .count()
                .min(u32::MAX as usize) as u32,
            total_deliveries: deliveries.len().min(u32::MAX as usize) as u32,
            active_deliveries: deliveries
                .iter()
                .filter(|bundle| {
                    matches!(
                        bundle.status,
                        DeliveryBundleStatus::Issued | DeliveryBundleStatus::Accessed
                    )
                })
                .count()
                .min(u32::MAX as usize) as u32,
            total_revocations: revocations.len().min(u32::MAX as usize) as u32,
            latest_activity_at_ms: [
                latest_asset_ms,
                latest_grant_ms,
                latest_delivery_ms,
                latest_revocation_ms,
            ]
            .into_iter()
            .flatten()
            .max(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DataMarketSnapshot {
    pub refreshed_at_ms: i64,
    #[serde(default)]
    pub summary: DataMarketSummary,
    #[serde(default)]
    pub assets: Vec<DataAsset>,
    #[serde(default)]
    pub grants: Vec<AccessGrant>,
    #[serde(default)]
    pub deliveries: Vec<DeliveryBundle>,
    #[serde(default)]
    pub revocations: Vec<RevocationReceipt>,
}

impl DataMarketSnapshot {
    pub fn from_parts(
        assets: Vec<DataAsset>,
        grants: Vec<AccessGrant>,
        deliveries: Vec<DeliveryBundle>,
        revocations: Vec<RevocationReceipt>,
        refreshed_at_ms: i64,
    ) -> Self {
        let summary = DataMarketSummary::from_parts(
            assets.as_slice(),
            grants.as_slice(),
            deliveries.as_slice(),
            revocations.as_slice(),
        );
        Self {
            refreshed_at_ms,
            summary,
            assets,
            grants,
            deliveries,
            revocations,
        }
    }
}
