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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderTier {
    Provisional,
    Qualified,
    Preferred,
}

impl Default for ProviderTier {
    fn default() -> Self {
        Self::Qualified
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PricingStage {
    Fixed,
    Banded,
    Bidding,
}

impl Default for PricingStage {
    fn default() -> Self {
        Self::Fixed
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PricingBand {
    pub capability: String,
    pub min_price_msats: u64,
    pub max_price_msats: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_msats: Option<u64>,
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
    #[serde(default)]
    pub pricing_stage: PricingStage,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pricing_bands: Vec<PricingBand>,
    #[serde(default)]
    pub tier: ProviderTier,
    #[serde(default)]
    pub failure_strikes: u64,
    #[serde(default)]
    pub success_count: u64,
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

        let provider_id =
            metadata_string(meta, "provider_id").unwrap_or_else(|| worker.worker_id.clone());
        let base_url = metadata_string(meta, "provider_base_url");
        let capabilities = metadata_string_array(meta, "capabilities");
        let pricing_stage = pricing_stage_from_metadata(meta);
        let pricing_bands = pricing_bands_from_metadata(meta);
        let min_price_msats = metadata_u64(meta, "min_price_msats")
            .or_else(|| derive_min_price_msats(&pricing_bands));
        let reserve_pool_flag = metadata_bool(meta, "reserve_pool").unwrap_or(false)
            || roles.iter().any(|role| role == "reserve_pool");
        let supply_class =
            supply_class_from_metadata(worker.adapter.as_str(), meta, reserve_pool_flag);
        let reserve_pool = reserve_pool_flag || supply_class == SupplyClass::ReservePool;
        let cluster_id = metadata_string(meta, "cluster_id");
        let cluster_members = metadata_string_array(meta, "cluster_members");
        let quarantined = metadata_bool(meta, "quarantined").unwrap_or(false);
        let quarantine_reason = metadata_string(meta, "quarantine_reason");
        let failure_strikes = metadata_u64(meta, "failure_strikes").unwrap_or(0);
        let success_count = metadata_u64(meta, "success_count").unwrap_or(0);
        let tier = provider_tier_from_metadata(meta, success_count, failure_strikes);

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
            pricing_stage,
            pricing_bands,
            tier,
            failure_strikes,
            success_count,
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

fn provider_tier_from_metadata(
    metadata: &Value,
    success_count: u64,
    failure_strikes: u64,
) -> ProviderTier {
    if let Some(value) = metadata_string(metadata, "tier")
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        match value {
            "provisional" => return ProviderTier::Provisional,
            "qualified" => return ProviderTier::Qualified,
            "preferred" => return ProviderTier::Preferred,
            _ => {}
        }
    }

    if failure_strikes >= 2 {
        return ProviderTier::Provisional;
    }
    if success_count >= 5 && failure_strikes == 0 {
        return ProviderTier::Preferred;
    }

    ProviderTier::Qualified
}

fn pricing_stage_from_metadata(metadata: &Value) -> PricingStage {
    let Some(value) = metadata.get("pricing_stage").and_then(Value::as_str) else {
        return PricingStage::Fixed;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "fixed" => PricingStage::Fixed,
        "banded" => PricingStage::Banded,
        "bidding" => PricingStage::Bidding,
        _ => PricingStage::Fixed,
    }
}

fn pricing_bands_from_metadata(metadata: &Value) -> Vec<PricingBand> {
    let Some(value) = metadata.get("pricing_bands") else {
        return Vec::new();
    };
    match serde_json::from_value::<Vec<PricingBand>>(value.clone()) {
        Ok(mut bands) => {
            bands.retain(|band| {
                !band.capability.trim().is_empty()
                    && band.min_price_msats > 0
                    && band.max_price_msats >= band.min_price_msats
            });
            bands
        }
        Err(_) => Vec::new(),
    }
}

fn derive_min_price_msats(bands: &[PricingBand]) -> Option<u64> {
    bands
        .iter()
        .map(|band| band.min_price_msats)
        .min()
        .filter(|value| *value > 0)
}

fn supply_class_from_metadata(adapter: &str, metadata: &Value, reserve_pool: bool) -> SupplyClass {
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

    let adapter = adapter.trim();
    if !adapter.is_empty() {
        let normalized = adapter.to_ascii_lowercase();
        if normalized.contains("bundle_rack") {
            return SupplyClass::BundleRack;
        }
        if normalized.contains("instance_market") {
            return SupplyClass::InstanceMarket;
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
    select_provider_for_capability_excluding(workers, owner_filter, capability, None)
}

pub fn select_provider_for_capability_excluding(
    workers: &[WorkerSnapshot],
    owner_filter: Option<&WorkerOwner>,
    capability: &str,
    exclude_worker_id: Option<&str>,
) -> Option<ProviderSelection> {
    let capability = capability.trim();
    if capability.is_empty() {
        return None;
    }

    let exclude_worker_id = exclude_worker_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let mut providers = build_provider_catalog(workers);
    providers.retain(|provider| {
        if let Some(exclude) = exclude_worker_id {
            if provider.worker_id == exclude {
                return false;
            }
        }
        is_provider_available(provider, capability)
    });

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
    reserve_pool
        .into_iter()
        .next()
        .map(|provider| ProviderSelection {
            provider,
            tier: ProviderSelectionTier::ReservePool,
        })
}

const PROVIDER_STRIKE_PRICE_PENALTY_MSATS: u64 = 500;
const PROVIDER_PROVISIONAL_PENALTY_MSATS: u64 = 1_000;

fn provider_rank_key(provider: &ProviderCatalogEntry) -> (u64, u8, u64, String) {
    let base_price = provider.min_price_msats.unwrap_or(u64::MAX);
    let strike_penalty = provider
        .failure_strikes
        .saturating_mul(PROVIDER_STRIKE_PRICE_PENALTY_MSATS);
    let tier_penalty = match provider.tier {
        ProviderTier::Provisional => PROVIDER_PROVISIONAL_PENALTY_MSATS,
        ProviderTier::Qualified | ProviderTier::Preferred => 0,
    };
    let effective_price = base_price
        .saturating_add(strike_penalty)
        .saturating_add(tier_penalty);

    (
        effective_price,
        provider_tier_rank(&provider.tier),
        provider.failure_strikes,
        provider.provider_id.clone(),
    )
}

fn provider_tier_rank(tier: &ProviderTier) -> u8 {
    match tier {
        ProviderTier::Preferred => 0,
        ProviderTier::Qualified => 1,
        ProviderTier::Provisional => 2,
    }
}

fn owners_match(left: &WorkerOwner, right: &WorkerOwner) -> bool {
    match (left.user_id, right.user_id) {
        (Some(left_id), Some(right_id)) => left_id == right_id,
        (None, None) => {
            left.guest_scope.as_deref().map(str::trim)
                == right.guest_scope.as_deref().map(str::trim)
        }
        _ => false,
    }
}

fn is_provider_available(provider: &ProviderCatalogEntry, capability: &str) -> bool {
    if provider
        .base_url
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
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

    provider.capabilities.iter().any(|cap| cap == capability)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{RuntimeWorker, WorkerLiveness, WorkerStatus};
    use chrono::Utc;

    fn snapshot_provider(
        worker_id: &str,
        owner_user_id: u64,
        provider_id: &str,
        min_price_msats: u64,
        failure_strikes: u64,
        success_count: u64,
        adapter: &str,
        reserve_pool: bool,
    ) -> WorkerSnapshot {
        let now = Utc::now();
        let owner = WorkerOwner {
            user_id: Some(owner_user_id),
            guest_scope: None,
        };

        WorkerSnapshot {
            worker: RuntimeWorker {
                worker_id: worker_id.to_string(),
                owner,
                workspace_ref: None,
                codex_home_ref: None,
                adapter: adapter.to_string(),
                status: WorkerStatus::Running,
                latest_seq: 1,
                metadata: serde_json::json!({
                    "roles": ["provider"],
                    "provider_id": provider_id,
                    "provider_base_url": "http://127.0.0.1:1",
                    "capabilities": ["oa.sandbox_run.v1"],
                    "min_price_msats": min_price_msats,
                    "reserve_pool": reserve_pool,
                    "quarantined": false,
                    "failure_strikes": failure_strikes,
                    "success_count": success_count,
                }),
                started_at: now,
                stopped_at: None,
                last_heartbeat_at: Some(now),
                updated_at: now,
            },
            liveness: WorkerLiveness {
                heartbeat_age_ms: Some(0),
                heartbeat_stale_after_ms: 120_000,
                heartbeat_state: "fresh".to_string(),
            },
        }
    }

    #[test]
    fn routing_prefers_lower_effective_price_after_strikes() {
        let owner = WorkerOwner {
            user_id: Some(11),
            guest_scope: None,
        };
        let workers = vec![
            snapshot_provider("worker:a", 11, "provider-a", 1000, 2, 0, "test", false),
            snapshot_provider("worker:b", 11, "provider-b", 1200, 0, 0, "test", false),
        ];
        let selection = select_provider_for_capability(&workers, Some(&owner), "oa.sandbox_run.v1")
            .expect("expected selection");
        assert_eq!(selection.provider.provider_id, "provider-b");
    }

    #[test]
    fn routing_prefers_preferred_tier_on_equal_price() {
        let owner = WorkerOwner {
            user_id: Some(11),
            guest_scope: None,
        };
        let workers = vec![
            snapshot_provider(
                "worker:preferred",
                11,
                "provider-preferred",
                1000,
                0,
                5,
                "test",
                false,
            ),
            snapshot_provider(
                "worker:qualified",
                11,
                "provider-qualified",
                1000,
                0,
                0,
                "test",
                false,
            ),
        ];
        let selection = select_provider_for_capability(&workers, Some(&owner), "oa.sandbox_run.v1")
            .expect("expected selection");
        assert_eq!(selection.provider.provider_id, "provider-preferred");
        assert_eq!(selection.provider.tier, ProviderTier::Preferred);
    }

    #[test]
    fn excluding_worker_id_skips_provider() {
        let owner = WorkerOwner {
            user_id: Some(11),
            guest_scope: None,
        };
        let workers = vec![
            snapshot_provider("worker:a", 11, "provider-a", 1000, 0, 0, "test", false),
            snapshot_provider("worker:b", 11, "provider-b", 1100, 0, 0, "test", false),
        ];
        let selection = select_provider_for_capability_excluding(
            &workers,
            Some(&owner),
            "oa.sandbox_run.v1",
            Some("worker:a"),
        )
        .expect("expected selection");
        assert_eq!(selection.provider.provider_id, "provider-b");
    }

    #[test]
    fn adapter_infers_bundle_rack_supply_class() {
        let workers = vec![snapshot_provider(
            "worker:br",
            11,
            "provider-br",
            1000,
            0,
            0,
            "bundle_rack_adapter",
            false,
        )];
        let catalog = build_provider_catalog(&workers);
        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog[0].supply_class, SupplyClass::BundleRack);
    }

    #[test]
    fn adapter_infers_instance_market_supply_class() {
        let workers = vec![snapshot_provider(
            "worker:im",
            11,
            "provider-im",
            1000,
            0,
            0,
            "instance_market_adapter",
            false,
        )];
        let catalog = build_provider_catalog(&workers);
        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog[0].supply_class, SupplyClass::InstanceMarket);
    }
}
