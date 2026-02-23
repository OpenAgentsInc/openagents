use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::{RuntimeWorker, WorkerOwner, WorkerStatus};
use crate::workers::WorkerSnapshot;

pub const PROVIDER_CATALOG_SCHEMA_V1: &str = "openagents.marketplace.provider_catalog.v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SupplyClass {
    SingleNode,
    LocalCluster,
    BundleRack,
    InstanceMarket,
    ReservePool,
}

impl Default for SupplyClass {
    fn default() -> Self {
        Self::SingleNode
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderCatalogEntry {
    pub schema: String,
    pub provider_id: String,
    pub worker_id: String,
    pub adapter: String,
    pub owner: WorkerOwner,
    pub status: WorkerStatus,
    pub heartbeat_state: String,
    pub heartbeat_age_ms: Option<i64>,
    #[serde(default)]
    pub supply_class: SupplyClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cluster_members: Vec<String>,
    #[serde(default)]
    pub reserve_pool: bool,
    #[serde(default)]
    pub quarantined: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quarantine_reason: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    pub base_url: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub min_price_msats: Option<u64>,
    pub updated_at: DateTime<Utc>,
}

impl ProviderCatalogEntry {
    fn from_snapshot(snapshot: &WorkerSnapshot) -> Option<Self> {
        let worker = &snapshot.worker;
        let meta = &worker.metadata;
        let roles = metadata_string_array(meta, "roles");
        if !roles.iter().any(|role| role == "provider") {
            return None;
        }

        let provider_id = metadata_string(meta, "provider_id").unwrap_or_else(|| worker.worker_id.clone());
        let base_url = metadata_string(meta, "provider_base_url");
        let capabilities = metadata_string_array(meta, "capabilities");
        let min_price_msats = metadata_u64(meta, "min_price_msats");
        let reserve_pool_flag = metadata_bool(meta, "reserve_pool").unwrap_or(false)
            || roles.iter().any(|role| role == "reserve_pool");
        let supply_class = supply_class_from_metadata(meta, reserve_pool_flag);
        let reserve_pool = reserve_pool_flag || supply_class == SupplyClass::ReservePool;
        let cluster_id = metadata_string(meta, "cluster_id");
        let cluster_members = metadata_string_array(meta, "cluster_members");
        let quarantined = metadata_bool(meta, "quarantined").unwrap_or(false);
        let quarantine_reason = metadata_string(meta, "quarantine_reason");

        Some(Self {
            schema: PROVIDER_CATALOG_SCHEMA_V1.to_string(),
            provider_id,
            worker_id: worker.worker_id.clone(),
            adapter: worker.adapter.clone(),
            owner: worker.owner.clone(),
            status: worker.status.clone(),
            heartbeat_state: snapshot.liveness.heartbeat_state.clone(),
            heartbeat_age_ms: snapshot.liveness.heartbeat_age_ms,
            supply_class,
            cluster_id,
            cluster_members,
            reserve_pool,
            quarantined,
            quarantine_reason,
            roles,
            base_url,
            capabilities,
            min_price_msats,
            updated_at: worker.updated_at,
        })
    }
}

pub fn build_provider_catalog(workers: &[WorkerSnapshot]) -> Vec<ProviderCatalogEntry> {
    let mut out = workers
        .iter()
        .filter_map(ProviderCatalogEntry::from_snapshot)
        .collect::<Vec<_>>();
    out.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));
    out
}

fn metadata_string(metadata: &Value, key: &str) -> Option<String> {
    metadata.get(key)?.as_str().map(|v| v.to_string())
}

fn metadata_u64(metadata: &Value, key: &str) -> Option<u64> {
    metadata.get(key)?.as_u64()
}

fn metadata_bool(metadata: &Value, key: &str) -> Option<bool> {
    metadata.get(key)?.as_bool()
}

fn metadata_string_array(metadata: &Value, key: &str) -> Vec<String> {
    match metadata.get(key).and_then(|value| value.as_array()) {
        Some(values) => values
            .iter()
            .filter_map(|value| value.as_str())
            .map(|value| value.to_string())
            .collect(),
        None => Vec::new(),
    }
}

pub fn is_provider_worker(worker: &RuntimeWorker) -> bool {
    metadata_string_array(&worker.metadata, "roles")
        .iter()
        .any(|role| role == "provider")
}

fn supply_class_from_metadata(metadata: &Value, reserve_pool: bool) -> SupplyClass {
    if reserve_pool {
        return SupplyClass::ReservePool;
    }

    if let Some(value) = metadata_string(metadata, "supply_class")
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        match value {
            "single_node" => return SupplyClass::SingleNode,
            "local_cluster" => return SupplyClass::LocalCluster,
            "bundle_rack" => return SupplyClass::BundleRack,
            "instance_market" => return SupplyClass::InstanceMarket,
            "reserve_pool" => return SupplyClass::ReservePool,
            _ => {}
        }
    }

    if metadata_bool(metadata, "local_cluster").unwrap_or(false) {
        return SupplyClass::LocalCluster;
    }
    if metadata_bool(metadata, "bundle_rack").unwrap_or(false) {
        return SupplyClass::BundleRack;
    }
    if metadata_bool(metadata, "instance_market").unwrap_or(false) {
        return SupplyClass::InstanceMarket;
    }

    SupplyClass::SingleNode
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSelectionTier {
    Owned,
    ReservePool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderSelection {
    pub provider: ProviderCatalogEntry,
    pub tier: ProviderSelectionTier,
}

pub fn select_provider_for_capability(
    workers: &[WorkerSnapshot],
    owner_filter: Option<&WorkerOwner>,
    capability: &str,
) -> Option<ProviderSelection> {
    let capability = capability.trim();
    if capability.is_empty() {
        return None;
    }

    let mut providers = build_provider_catalog(workers);
    providers.retain(|provider| is_provider_available(provider, capability));

    if let Some(owner) = owner_filter {
        let mut owned = providers
            .iter()
            .filter(|provider| owners_match(&provider.owner, owner))
            .cloned()
            .collect::<Vec<_>>();
        owned.sort_by(|a, b| provider_rank_key(a).cmp(&provider_rank_key(b)));
        if let Some(provider) = owned.into_iter().next() {
            return Some(ProviderSelection {
                provider,
                tier: ProviderSelectionTier::Owned,
            });
        }
    }

    let mut reserve_pool = providers
        .into_iter()
        .filter(|provider| provider.supply_class == SupplyClass::ReservePool)
        .collect::<Vec<_>>();
    reserve_pool.sort_by(|a, b| provider_rank_key(a).cmp(&provider_rank_key(b)));
    reserve_pool.into_iter().next().map(|provider| ProviderSelection {
        provider,
        tier: ProviderSelectionTier::ReservePool,
    })
}

fn provider_rank_key(provider: &ProviderCatalogEntry) -> (u64, String) {
    (
        provider.min_price_msats.unwrap_or(u64::MAX),
        provider.provider_id.clone(),
    )
}

fn owners_match(left: &WorkerOwner, right: &WorkerOwner) -> bool {
    match (left.user_id, right.user_id) {
        (Some(left_id), Some(right_id)) => left_id == right_id,
        (None, None) => left.guest_scope.as_deref().map(str::trim)
            == right.guest_scope.as_deref().map(str::trim),
        _ => false,
    }
}

fn is_provider_available(provider: &ProviderCatalogEntry, capability: &str) -> bool {
    if provider.base_url.as_deref().map(str::trim).unwrap_or("").is_empty() {
        return false;
    }

    if provider.quarantined {
        return false;
    }

    if provider.status != WorkerStatus::Running {
        return false;
    }

    if provider.heartbeat_state != "fresh" {
        return false;
    }

    provider
        .capabilities
        .iter()
        .any(|cap| cap == capability)
}
