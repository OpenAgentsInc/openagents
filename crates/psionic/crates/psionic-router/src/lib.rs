//! Multi-model routing and worker-policy contracts for Psionic.
//!
//! This crate owns reusable fleet-routing truth for served Psionic workers.
//! It also owns pluggable response-state retention backends for served agent
//! loops. It does not own request execution, app UX, procurement, settlement,
//! or app-facing product storage. Those concerns stay in `psionic-serve`, app
//! code, and kernel or Nexus services.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

mod response_state;
mod tassadar_async_lifecycle_route_policy;
mod tassadar_broad_general_compute_validator_route_policy;
mod tassadar_broad_internal_compute_route_policy;
mod tassadar_composite_routing;
mod tassadar_counterfactual_route_quality;
mod tassadar_cross_profile_link_compatibility;
mod tassadar_delegation_benchmark;
mod tassadar_effect_route_policy;
mod tassadar_evidence_routing;
mod tassadar_float_profile_route_policy;
mod tassadar_general_internal_compute_red_team_route_exercises;
mod tassadar_hybrid_process_controller;
mod tassadar_internal_compute_package_manager;
mod tassadar_latency_evidence_tradeoff;
mod tassadar_linked_program_bundle_route;
mod tassadar_module_catalog;
mod tassadar_module_installation;
mod tassadar_module_overlap_resolution;
mod tassadar_negative_invocation;
mod tassadar_planner_policy;
mod tassadar_post_article_starter_plugin_tool_loop;
mod tassadar_proposal_profile_route_policy;
mod tassadar_route;
mod tassadar_self_installation_gate;
mod tassadar_semantic_window_route_policy;
mod tassadar_session_process_route_policy;
mod tassadar_world_mount_compatibility;
mod tool_loop;

pub use response_state::{
    ResponseConversationRef, ResponseStateBackend, ResponseStateCapability, ResponseStateContext,
    ResponseStateError, ResponseStateRecord, ResponseStateRetentionPolicy, ResponseStateStore,
    SparseRouteBinding,
};
pub use tassadar_async_lifecycle_route_policy::*;
pub use tassadar_broad_general_compute_validator_route_policy::*;
pub use tassadar_broad_internal_compute_route_policy::*;
pub use tassadar_composite_routing::*;
pub use tassadar_counterfactual_route_quality::*;
pub use tassadar_cross_profile_link_compatibility::*;
pub use tassadar_delegation_benchmark::*;
pub use tassadar_effect_route_policy::*;
pub use tassadar_evidence_routing::*;
pub use tassadar_float_profile_route_policy::*;
pub use tassadar_general_internal_compute_red_team_route_exercises::*;
pub use tassadar_hybrid_process_controller::*;
pub use tassadar_internal_compute_package_manager::*;
pub use tassadar_latency_evidence_tradeoff::*;
pub use tassadar_linked_program_bundle_route::*;
pub use tassadar_module_catalog::*;
pub use tassadar_module_installation::*;
pub use tassadar_module_overlap_resolution::*;
pub use tassadar_negative_invocation::*;
pub use tassadar_planner_policy::*;
pub use tassadar_post_article_starter_plugin_tool_loop::*;
pub use tassadar_proposal_profile_route_policy::*;
pub use tassadar_route::*;
pub use tassadar_self_installation_gate::*;
pub use tassadar_semantic_window_route_policy::*;
pub use tassadar_session_process_route_policy::*;
pub use tassadar_world_mount_compatibility::*;
pub use tool_loop::{
    ToolExecutionRequest, ToolGateway, ToolHistoryVisibility, ToolLoopController, ToolLoopError,
    ToolLoopModelRunner, ToolLoopModelTurn, ToolLoopOutcome, ToolLoopPolicy, ToolLoopRequest,
    ToolLoopStepReceipt, ToolLoopTerminationReason, ToolLoopToolCall, ToolLoopToolExecutor,
    ToolLoopToolResult, ToolLoopTurnRequest, ToolProviderDescriptor, ToolProviderInterface,
    ToolResultVisibility,
};

use psionic_runtime::{
    ClusterExecutionCapabilityProfile, ExecutionCapabilityProfile, ExecutionTopologyKind,
    GenerationSchedulerPolicy, HealthStatus, KvCacheEncodingFamily, KvCacheEncodingPolicy,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "multi-model routing and control-plane policy contracts";

/// Routed API surface in front of one or more Psionic workers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingEndpoint {
    /// OpenAI-compatible chat completions.
    ChatCompletions,
    /// OpenAI-compatible responses API.
    Responses,
    /// OpenAI-compatible embeddings API.
    Embeddings,
}

impl RoutingEndpoint {
    /// Returns the stable API path for this routed endpoint.
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::ChatCompletions => "/v1/chat/completions",
            Self::Responses => "/v1/responses",
            Self::Embeddings => "/v1/embeddings",
        }
    }
}

/// Target used when resolving a route.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingTarget {
    /// Resolve the router's configured default model.
    Default,
    /// Resolve one requested model alias or canonical name.
    RequestedModel(String),
    /// Resolve a previously pinned stable model key.
    ModelKey(String),
}

/// Capability filters required by one routed request.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingCapabilityFilters {
    /// Require structured-output support.
    pub structured_outputs: bool,
    /// Require tool-calling support.
    pub tool_calling: bool,
    /// Require response-state support.
    pub response_state: bool,
}

impl RoutingCapabilityFilters {
    /// Returns whether no capability constraints were requested.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        !self.structured_outputs && !self.tool_calling && !self.response_state
    }
}

/// Request-side KV-cache encoding constraints for routed served generation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingKvCacheEncodingPreferences {
    /// Require one explicit KV-cache encoding capability family.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require: Option<KvCacheEncodingFamily>,
    /// Prefer one explicit KV-cache encoding capability family when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefer: Option<KvCacheEncodingFamily>,
    /// Exclude any route advertising these KV-cache encoding families.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<KvCacheEncodingFamily>,
}

impl RoutingKvCacheEncodingPreferences {
    /// Returns whether no KV-cache encoding constraints were requested.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.require.is_none() && self.prefer.is_none() && self.exclude.is_empty()
    }
}

/// Request-side placement hints used by cache-aware and warm-aware policies.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingPolicyHints {
    /// Stable cache-affinity key when a higher layer knows one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_key: Option<String>,
    /// Tenant or security-domain boundary for cache reuse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_scope: Option<String>,
    /// Optional topology or route-pinning scope required for safe reuse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_scope: Option<String>,
    /// Stable request key used to seed bounded-choice sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_key: Option<String>,
}

/// Warmth posture for one worker-local model route.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutedWarmState {
    /// No loaded or warm state is available.
    #[default]
    Cold,
    /// The model is loading or otherwise not yet warm.
    Warming,
    /// The model is warm and eligible for warm-route preference.
    Warm,
}

impl RoutedWarmState {
    #[must_use]
    const fn is_warm(self) -> bool {
        matches!(self, Self::Warm)
    }
}

/// Locality for one routed worker entry and the selections derived from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutedExecutionLocality {
    /// Request execution stays on the local Psionic server.
    Local,
    /// Request execution is proxied to a remote mesh peer.
    RemoteProxy,
}

/// Execution provenance published for one routed worker entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutedExecutionProvenance {
    /// The route is executed directly by the current Psionic server.
    LocalExecution,
    /// The route is executed through the Psionic bootstrap proxy path.
    BootstrapProxy,
}

/// One cache entry that the router may safely bias toward.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutedCacheEntry {
    /// Stable cache-affinity key.
    pub cache_key: String,
    /// Tenant or security-domain scope required for reuse.
    pub tenant_scope: String,
    /// Optional topology or route-pinning scope required for reuse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_scope: Option<String>,
    /// Explicit cache-encoding policy required for safe prefix reuse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_cache_encoding_policy: Option<KvCacheEncodingPolicy>,
    /// Approximate reusable token count represented by the cache entry.
    pub reusable_tokens: usize,
}

impl RoutedCacheEntry {
    /// Creates one cache entry.
    #[must_use]
    pub fn new(
        cache_key: impl Into<String>,
        tenant_scope: impl Into<String>,
        reusable_tokens: usize,
    ) -> Self {
        Self {
            cache_key: cache_key.into(),
            tenant_scope: tenant_scope.into(),
            topology_scope: None,
            kv_cache_encoding_policy: None,
            reusable_tokens,
        }
    }

    /// Pins the cache entry to one topology or route scope.
    #[must_use]
    pub fn with_topology_scope(mut self, topology_scope: impl Into<String>) -> Self {
        self.topology_scope = Some(topology_scope.into());
        self
    }

    /// Pins the cache entry to one explicit KV-cache encoding policy.
    #[must_use]
    pub fn with_kv_cache_encoding_policy(
        mut self,
        kv_cache_encoding_policy: KvCacheEncodingPolicy,
    ) -> Self {
        self.kv_cache_encoding_policy = Some(kv_cache_encoding_policy);
        self
    }
}

/// Live runtime state that routing policy can safely consume.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutedModelRuntimeState {
    /// Warmth posture for the routed model.
    pub warm_state: RoutedWarmState,
    /// Current active-request count for load-aware tie-breaking.
    pub active_requests: usize,
    /// Cache entries that are safe to reuse under explicit scope checks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cache_entries: Vec<RoutedCacheEntry>,
}

/// One model-route request evaluated against router inventory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingRequest {
    /// API endpoint the caller needs.
    pub endpoint: RoutingEndpoint,
    /// Requested target model posture.
    pub target: RoutingTarget,
    /// Optional product surface publishing the demand signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_id: Option<String>,
    /// Required capabilities.
    pub capability_filters: RoutingCapabilityFilters,
    /// Explicit KV-cache encoding placement constraints.
    pub kv_cache_encoding_preferences: RoutingKvCacheEncodingPreferences,
    /// Policy hints for warm/cache-aware routing.
    pub policy_hints: RoutingPolicyHints,
    /// Ordered preferred worker identifiers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preferred_worker_ids: Vec<String>,
    /// Optional preferred model family.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_family: Option<String>,
}

impl RoutingRequest {
    /// Creates one routing request for an endpoint.
    #[must_use]
    pub fn new(endpoint: RoutingEndpoint) -> Self {
        Self {
            endpoint,
            target: RoutingTarget::Default,
            product_id: None,
            capability_filters: RoutingCapabilityFilters::default(),
            kv_cache_encoding_preferences: RoutingKvCacheEncodingPreferences::default(),
            policy_hints: RoutingPolicyHints::default(),
            preferred_worker_ids: Vec::new(),
            preferred_family: None,
        }
    }

    /// Pins resolution to one requested model alias or canonical name.
    #[must_use]
    pub fn with_requested_model(mut self, requested_model: impl Into<String>) -> Self {
        self.target = RoutingTarget::RequestedModel(requested_model.into());
        self
    }

    /// Pins resolution to one stable model key.
    #[must_use]
    pub fn with_model_key(mut self, model_key: impl Into<String>) -> Self {
        self.target = RoutingTarget::ModelKey(model_key.into());
        self
    }

    /// Tags the request with the product surface publishing this demand signal.
    #[must_use]
    pub fn with_product_id(mut self, product_id: impl Into<String>) -> Self {
        self.product_id = Some(product_id.into());
        self
    }

    /// Requires structured-output support.
    #[must_use]
    pub fn require_structured_outputs(mut self) -> Self {
        self.capability_filters.structured_outputs = true;
        self
    }

    /// Requires tool-calling support.
    #[must_use]
    pub fn require_tool_calling(mut self) -> Self {
        self.capability_filters.tool_calling = true;
        self
    }

    /// Requires response-state support.
    #[must_use]
    pub fn require_response_state(mut self) -> Self {
        self.capability_filters.response_state = true;
        self
    }

    /// Requires one explicit KV-cache encoding capability family.
    #[must_use]
    pub fn require_kv_cache_encoding(mut self, family: KvCacheEncodingFamily) -> Self {
        self.kv_cache_encoding_preferences.require = Some(family);
        self
    }

    /// Prefers one explicit KV-cache encoding capability family when available.
    #[must_use]
    pub fn prefer_kv_cache_encoding(mut self, family: KvCacheEncodingFamily) -> Self {
        self.kv_cache_encoding_preferences.prefer = Some(family);
        self
    }

    /// Excludes any route that advertises one explicit KV-cache encoding capability family.
    #[must_use]
    pub fn exclude_kv_cache_encoding(mut self, family: KvCacheEncodingFamily) -> Self {
        if !self.kv_cache_encoding_preferences.exclude.contains(&family) {
            self.kv_cache_encoding_preferences.exclude.push(family);
        }
        self
    }

    /// Adds a cache-affinity key and required tenant scope.
    #[must_use]
    pub fn with_cache_affinity(
        mut self,
        cache_key: impl Into<String>,
        tenant_scope: impl Into<String>,
    ) -> Self {
        self.policy_hints.cache_key = Some(cache_key.into());
        self.policy_hints.tenant_scope = Some(tenant_scope.into());
        self
    }

    /// Adds one topology scope for safe cache or warm reuse.
    #[must_use]
    pub fn with_topology_scope(mut self, topology_scope: impl Into<String>) -> Self {
        self.policy_hints.topology_scope = Some(topology_scope.into());
        self
    }

    /// Supplies a stable request key used for bounded-choice sampling.
    #[must_use]
    pub fn with_request_key(mut self, request_key: impl Into<String>) -> Self {
        self.policy_hints.request_key = Some(request_key.into());
        self
    }

    /// Marks one worker as preferred during deterministic tie-breaking.
    #[must_use]
    pub fn prefer_worker(mut self, worker_id: impl Into<String>) -> Self {
        self.preferred_worker_ids.push(worker_id.into());
        self
    }

    /// Restricts resolution to one preferred model family.
    #[must_use]
    pub fn prefer_family(mut self, family: impl Into<String>) -> Self {
        self.preferred_family = Some(family.into());
        self
    }
}

/// Stable demand key for one routed product and model lane.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RoutingDemandKey {
    /// Product surface that emitted the demand.
    pub product_id: String,
    /// Stable model identifier selected by the router.
    pub model_id: String,
    /// Requested external route alias, when demand came through one alias.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_alias: Option<String>,
}

impl RoutingDemandKey {
    /// Creates one stable demand key.
    #[must_use]
    pub fn new(
        product_id: impl Into<String>,
        model_id: impl Into<String>,
        route_alias: Option<String>,
    ) -> Self {
        Self {
            product_id: product_id.into(),
            model_id: model_id.into(),
            route_alias,
        }
    }
}

/// Freshness policy for routed demand snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingDemandPolicy {
    /// Demand becomes stale after this many milliseconds without new observations.
    pub freshness_window_ms: u64,
}

impl Default for RoutingDemandPolicy {
    fn default() -> Self {
        Self {
            freshness_window_ms: 300_000,
        }
    }
}

/// Aggregated demand snapshot for one routed product/model lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingDemandSnapshot {
    /// Stable product/model demand key.
    pub key: RoutingDemandKey,
    /// Canonical model name behind the selected route.
    pub canonical_name: String,
    /// Requests observed in the current freshness window.
    pub request_count: usize,
    /// Highest active-request count seen on the selected route.
    pub peak_selected_active_requests: usize,
    /// First observation currently represented by this window.
    pub first_observed_at_ms: u64,
    /// Most recent observation in this window.
    pub last_observed_at_ms: u64,
    /// Time after which this demand window is stale.
    pub expires_at_ms: u64,
}

impl RoutingDemandSnapshot {
    /// Creates one fresh demand window from a selected route.
    #[must_use]
    pub fn new(
        key: RoutingDemandKey,
        selection: &RouteSelection,
        observed_at_ms: u64,
        policy: RoutingDemandPolicy,
    ) -> Self {
        Self {
            key,
            canonical_name: selection.canonical_name.clone(),
            request_count: 1,
            peak_selected_active_requests: selection.metrics.selected_active_requests,
            first_observed_at_ms: observed_at_ms,
            last_observed_at_ms: observed_at_ms,
            expires_at_ms: observed_at_ms.saturating_add(policy.freshness_window_ms),
        }
    }

    /// Returns whether the demand window is stale at the observed time.
    #[must_use]
    pub const fn is_expired_at(&self, observed_at_ms: u64) -> bool {
        observed_at_ms > self.expires_at_ms
    }
}

/// Mutable demand ledger keyed by product, model, and alias where needed.
#[derive(Clone, Debug, Default)]
pub struct RoutingDemandLedger {
    policy: RoutingDemandPolicy,
    snapshots_by_key: BTreeMap<RoutingDemandKey, RoutingDemandSnapshot>,
}

impl RoutingDemandLedger {
    /// Creates one routed demand ledger with explicit freshness policy.
    #[must_use]
    pub fn new(policy: RoutingDemandPolicy) -> Self {
        Self {
            policy,
            snapshots_by_key: BTreeMap::new(),
        }
    }

    /// Returns the current freshness policy.
    #[must_use]
    pub const fn policy(&self) -> RoutingDemandPolicy {
        self.policy
    }

    /// Records one routed request against the selected worker and model.
    pub fn record(
        &mut self,
        request: &RoutingRequest,
        selection: &RouteSelection,
        observed_at_ms: u64,
    ) {
        let key = RoutingDemandKey::new(
            request
                .product_id
                .clone()
                .unwrap_or_else(|| String::from("psionic.unscoped")),
            selection.model_key.clone(),
            route_alias_for_demand(request),
        );
        match self.snapshots_by_key.get_mut(&key) {
            Some(snapshot) if !snapshot.is_expired_at(observed_at_ms) => {
                snapshot.canonical_name = selection.canonical_name.clone();
                snapshot.request_count = snapshot.request_count.saturating_add(1);
                snapshot.peak_selected_active_requests = snapshot
                    .peak_selected_active_requests
                    .max(selection.metrics.selected_active_requests);
                snapshot.last_observed_at_ms = observed_at_ms;
                snapshot.expires_at_ms =
                    observed_at_ms.saturating_add(self.policy.freshness_window_ms);
            }
            Some(snapshot) => {
                *snapshot =
                    RoutingDemandSnapshot::new(key.clone(), selection, observed_at_ms, self.policy);
            }
            None => {
                let snapshot =
                    RoutingDemandSnapshot::new(key.clone(), selection, observed_at_ms, self.policy);
                self.snapshots_by_key.insert(key, snapshot);
            }
        }
    }

    /// Returns the current demand windows in deterministic order.
    #[must_use]
    pub fn snapshot_at(&self, observed_at_ms: u64) -> Vec<RoutingDemandSnapshot> {
        let mut snapshots = self.snapshots_by_key.values().cloned().collect::<Vec<_>>();
        snapshots.sort_by(|left, right| {
            left.is_expired_at(observed_at_ms)
                .cmp(&right.is_expired_at(observed_at_ms))
                .then_with(|| left.key.cmp(&right.key))
                .then_with(|| left.last_observed_at_ms.cmp(&right.last_observed_at_ms))
        });
        snapshots
    }
}

/// Router-visible model inventory for one worker.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutedModelInventory {
    /// Stable model key used by workers.
    pub model_key: String,
    /// Canonical user-facing model name.
    pub canonical_name: String,
    /// All accepted aliases for this model.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// High-level model family label.
    pub family: String,
    /// Supported routed endpoints.
    pub supported_endpoints: Vec<RoutingEndpoint>,
    /// Machine-checkable execution profile for the model.
    pub execution_profile: ExecutionCapabilityProfile,
    /// Optional scheduler policy surfaced by the worker.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_policy: Option<GenerationSchedulerPolicy>,
    /// Active KV-cache encoding policy for the routed lane, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_cache_encoding_policy: Option<KvCacheEncodingPolicy>,
    /// Declared KV-cache encoding capabilities for this routed lane.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_kv_cache_encoding_policies: Vec<KvCacheEncodingPolicy>,
    /// Whether structured outputs are supported.
    pub structured_outputs: bool,
    /// Whether tool calling is supported.
    pub tool_calling: bool,
    /// Whether response-state flows are supported.
    pub response_state: bool,
    /// Explicit execution refusal reason when the route is published but cannot
    /// execute locally yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_refusal_reason: Option<String>,
    /// Generic clustered execution modes admitted or published for this route.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cluster_execution_modes: Vec<RoutedClusterExecutionMode>,
    /// Cluster topology kinds admitted or published for this route.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cluster_execution_topologies: Vec<ExecutionTopologyKind>,
    /// Runtime-owned cluster capability profile when this route publishes one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_execution_capability_profile: Option<ClusterExecutionCapabilityProfile>,
    /// Explicit sparse expert-topology truth for family-specific distributed
    /// lanes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sparse_expert_topology: Option<RoutedSparseExpertTopology>,
    /// Explicit sparse shard artifact and placement truth for family-specific
    /// distributed lanes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sparse_shard_state: Option<RoutedSparseShardState>,
    /// Live runtime facts that cache-aware and warm-aware policy can consume.
    pub runtime_state: RoutedModelRuntimeState,
}

/// Generic clustered execution mode surfaced to downstream consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutedClusterExecutionMode {
    /// Forward the whole request to one remote executor.
    RemoteWholeRequest,
    /// Route requests across multiple warm full-model replicas.
    Replicated,
    /// Split one dense model across multiple machines.
    DenseSplit,
    /// Execute one sparse or MoE family across distributed expert hosts.
    SparseExpert,
}

/// Runtime contract required for one sparse expert lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutedSparseExpertRuntimeContract {
    /// The runtime has native expert-family execution.
    NativeMoe,
    /// The runtime needs explicit family-specific placement truth.
    FamilySpecificPlacement,
}

/// Explicit sparse expert-topology truth carried by one routed model lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutedSparseExpertTopology {
    /// Model family label.
    pub family: String,
    /// Raw architecture label.
    pub architecture: String,
    /// Primary artifact digest that the sparse lane expects on expert hosts.
    pub served_artifact_digest: String,
    /// Stable sharded-model manifest digest when one manifest seeded the lane.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sharded_model_manifest_digest: Option<String>,
    /// Runtime contract required for honest execution.
    pub runtime_contract: RoutedSparseExpertRuntimeContract,
    /// Total expert count declared by the model.
    pub expert_count: usize,
    /// Routed active-expert count when the model declares one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_expert_count: Option<usize>,
    /// Expert feed-forward width when the model declares one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expert_feed_forward_length: Option<usize>,
}

impl RoutedSparseExpertTopology {
    /// Creates one routed sparse expert-topology record.
    #[must_use]
    pub fn new(
        family: impl Into<String>,
        architecture: impl Into<String>,
        served_artifact_digest: impl Into<String>,
        runtime_contract: RoutedSparseExpertRuntimeContract,
        expert_count: usize,
    ) -> Self {
        Self {
            family: family.into(),
            architecture: architecture.into(),
            served_artifact_digest: served_artifact_digest.into(),
            sharded_model_manifest_digest: None,
            runtime_contract,
            expert_count,
            active_expert_count: None,
            expert_feed_forward_length: None,
        }
    }

    /// Attaches a sharded-model manifest digest.
    #[must_use]
    pub fn with_sharded_model_manifest_digest(mut self, digest: impl Into<String>) -> Self {
        self.sharded_model_manifest_digest = Some(digest.into());
        self
    }

    /// Attaches the active expert count.
    #[must_use]
    pub const fn with_active_expert_count(mut self, active_expert_count: usize) -> Self {
        self.active_expert_count = Some(active_expert_count);
        self
    }

    /// Attaches the expert feed-forward width.
    #[must_use]
    pub const fn with_expert_feed_forward_length(
        mut self,
        expert_feed_forward_length: usize,
    ) -> Self {
        self.expert_feed_forward_length = Some(expert_feed_forward_length);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutedSparseShardArtifactStatus {
    Materialized,
    Reused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutedSparseShardHealth {
    Healthy,
    RebuildRequired,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutedSparseShardReplica {
    pub node_id: String,
    pub first_expert_index: usize,
    pub last_expert_index_exclusive: usize,
    pub build_cache_key: String,
    pub shard_artifact_digest: String,
    pub artifact_status: RoutedSparseShardArtifactStatus,
}

impl RoutedSparseShardReplica {
    #[must_use]
    pub fn new(
        node_id: impl Into<String>,
        first_expert_index: usize,
        last_expert_index_exclusive: usize,
        build_cache_key: impl Into<String>,
        shard_artifact_digest: impl Into<String>,
        artifact_status: RoutedSparseShardArtifactStatus,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            first_expert_index,
            last_expert_index_exclusive,
            build_cache_key: build_cache_key.into(),
            shard_artifact_digest: shard_artifact_digest.into(),
            artifact_status,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutedSparseShardState {
    pub placement_digest: String,
    pub inventory_digest: String,
    pub shard_version_digest: String,
    pub health: RoutedSparseShardHealth,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replicas: Vec<RoutedSparseShardReplica>,
}

impl RoutedSparseShardState {
    #[must_use]
    pub fn new(
        placement_digest: impl Into<String>,
        inventory_digest: impl Into<String>,
        shard_version_digest: impl Into<String>,
        health: RoutedSparseShardHealth,
    ) -> Self {
        Self {
            placement_digest: placement_digest.into(),
            inventory_digest: inventory_digest.into(),
            shard_version_digest: shard_version_digest.into(),
            health,
            replicas: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_replica(mut self, replica: RoutedSparseShardReplica) -> Self {
        self.replicas.push(replica);
        self
    }
}

impl RoutedModelInventory {
    /// Creates a model inventory entry and seeds stable aliases.
    #[must_use]
    pub fn new(
        model_key: impl Into<String>,
        canonical_name: impl Into<String>,
        family: impl Into<String>,
        execution_profile: ExecutionCapabilityProfile,
    ) -> Self {
        let model_key = model_key.into();
        let canonical_name = canonical_name.into();
        let mut aliases = vec![model_key.clone()];
        if canonical_name != model_key {
            aliases.push(canonical_name.clone());
        }
        Self {
            model_key,
            canonical_name,
            aliases,
            family: family.into(),
            supported_endpoints: Vec::new(),
            execution_profile,
            scheduler_policy: None,
            kv_cache_encoding_policy: None,
            supported_kv_cache_encoding_policies: Vec::new(),
            structured_outputs: false,
            tool_calling: false,
            response_state: false,
            execution_refusal_reason: None,
            cluster_execution_modes: Vec::new(),
            cluster_execution_topologies: Vec::new(),
            cluster_execution_capability_profile: None,
            sparse_expert_topology: None,
            sparse_shard_state: None,
            runtime_state: RoutedModelRuntimeState::default(),
        }
    }

    /// Appends one alias when it is not already present.
    #[must_use]
    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        let alias = alias.into();
        if !self.aliases.iter().any(|existing| existing == &alias) {
            self.aliases.push(alias);
        }
        self
    }

    /// Appends one supported endpoint when it is not already present.
    #[must_use]
    pub fn with_supported_endpoint(mut self, endpoint: RoutingEndpoint) -> Self {
        if !self.supported_endpoints.contains(&endpoint) {
            self.supported_endpoints.push(endpoint);
            self.supported_endpoints.sort();
        }
        self
    }

    /// Attaches a scheduler policy.
    #[must_use]
    pub fn with_scheduler_policy(mut self, policy: GenerationSchedulerPolicy) -> Self {
        self.scheduler_policy = Some(policy);
        self
    }

    /// Publishes the active KV-cache encoding policy for the routed lane.
    #[must_use]
    pub fn with_kv_cache_encoding_policy(
        mut self,
        kv_cache_encoding_policy: KvCacheEncodingPolicy,
    ) -> Self {
        self.kv_cache_encoding_policy = Some(kv_cache_encoding_policy);
        self
    }

    /// Appends one declared KV-cache encoding capability when it is not already present.
    #[must_use]
    pub fn with_supported_kv_cache_encoding_policy(
        mut self,
        kv_cache_encoding_policy: KvCacheEncodingPolicy,
    ) -> Self {
        if !self
            .supported_kv_cache_encoding_policies
            .iter()
            .any(|existing| existing == &kv_cache_encoding_policy)
        {
            self.supported_kv_cache_encoding_policies
                .push(kv_cache_encoding_policy);
        }
        self
    }

    /// Marks structured outputs as supported.
    #[must_use]
    pub const fn with_structured_outputs(mut self) -> Self {
        self.structured_outputs = true;
        self
    }

    /// Marks tool calling as supported.
    #[must_use]
    pub const fn with_tool_calling(mut self) -> Self {
        self.tool_calling = true;
        self
    }

    /// Marks response-state flows as supported.
    #[must_use]
    pub const fn with_response_state(mut self) -> Self {
        self.response_state = true;
        self
    }

    /// Publishes one explicit execution refusal reason for the lane.
    #[must_use]
    pub fn with_execution_refusal_reason(
        mut self,
        execution_refusal_reason: impl Into<String>,
    ) -> Self {
        self.execution_refusal_reason = Some(execution_refusal_reason.into());
        self
    }

    /// Publishes one generic clustered execution mode for the lane.
    #[must_use]
    pub fn with_cluster_execution_mode(
        mut self,
        cluster_execution_mode: RoutedClusterExecutionMode,
    ) -> Self {
        if !self
            .cluster_execution_modes
            .iter()
            .any(|existing| existing == &cluster_execution_mode)
        {
            self.cluster_execution_modes.push(cluster_execution_mode);
            self.cluster_execution_modes.sort_unstable();
        }
        self
    }

    /// Publishes one cluster topology kind for the lane.
    #[must_use]
    pub fn with_cluster_execution_topology(
        mut self,
        cluster_execution_topology: ExecutionTopologyKind,
    ) -> Self {
        if !self
            .cluster_execution_topologies
            .iter()
            .any(|existing| existing == &cluster_execution_topology)
        {
            self.cluster_execution_topologies
                .push(cluster_execution_topology);
            self.cluster_execution_topologies.sort_unstable();
        }
        self
    }

    /// Publishes one runtime-owned cluster capability profile for the lane.
    #[must_use]
    pub fn with_cluster_execution_capability_profile(
        mut self,
        cluster_execution_capability_profile: ClusterExecutionCapabilityProfile,
    ) -> Self {
        self.cluster_execution_capability_profile = Some(cluster_execution_capability_profile);
        self
    }

    /// Publishes explicit sparse expert-topology truth for the lane.
    #[must_use]
    pub fn with_sparse_expert_topology(
        mut self,
        sparse_expert_topology: RoutedSparseExpertTopology,
    ) -> Self {
        self.sparse_expert_topology = Some(sparse_expert_topology);
        self
    }

    /// Publishes explicit sparse shard materialization and placement truth for
    /// the lane.
    #[must_use]
    pub fn with_sparse_shard_state(mut self, sparse_shard_state: RoutedSparseShardState) -> Self {
        self.sparse_shard_state = Some(sparse_shard_state);
        self
    }

    /// Marks the model route warm.
    #[must_use]
    pub fn with_warm_state(mut self, warm_state: RoutedWarmState) -> Self {
        self.runtime_state.warm_state = warm_state;
        self
    }

    /// Sets the current active-request count.
    #[must_use]
    pub fn with_active_requests(mut self, active_requests: usize) -> Self {
        self.runtime_state.active_requests = active_requests;
        self
    }

    /// Appends one reusable cache entry.
    #[must_use]
    pub fn with_cache_entry(mut self, cache_entry: RoutedCacheEntry) -> Self {
        self.runtime_state.cache_entries.push(cache_entry);
        self
    }
}

/// One worker and the model inventory it exposes to the router.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutedWorkerInventory {
    /// Stable worker identifier.
    pub worker_id: String,
    /// Upstream mesh peer identifier when this routed worker is a proxy-backed remote peer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_worker_id: Option<String>,
    /// Worker backend label.
    pub backend_label: String,
    /// Worker execution-mode label.
    pub execution_mode_label: String,
    /// Worker execution-engine label.
    pub execution_engine_label: String,
    /// Whether execution happens locally or through a remote proxy hop.
    pub execution_locality: RoutedExecutionLocality,
    /// Typed provenance for the routed worker path.
    pub execution_provenance: RoutedExecutionProvenance,
    /// Models exposed by the worker.
    pub models: Vec<RoutedModelInventory>,
}

impl RoutedWorkerInventory {
    /// Creates one worker inventory entry.
    #[must_use]
    pub fn new(
        worker_id: impl Into<String>,
        backend_label: impl Into<String>,
        execution_mode_label: impl Into<String>,
        execution_engine_label: impl Into<String>,
    ) -> Self {
        Self {
            worker_id: worker_id.into(),
            peer_worker_id: None,
            backend_label: backend_label.into(),
            execution_mode_label: execution_mode_label.into(),
            execution_engine_label: execution_engine_label.into(),
            execution_locality: RoutedExecutionLocality::Local,
            execution_provenance: RoutedExecutionProvenance::LocalExecution,
            models: Vec::new(),
        }
    }

    /// Marks the worker as one remote bootstrap-proxy peer.
    #[must_use]
    pub const fn as_remote_bootstrap_proxy(mut self) -> Self {
        self.execution_locality = RoutedExecutionLocality::RemoteProxy;
        self.execution_provenance = RoutedExecutionProvenance::BootstrapProxy;
        self
    }

    /// Publishes the upstream mesh peer identifier for one proxied remote worker.
    #[must_use]
    pub fn with_peer_worker_id(mut self, peer_worker_id: impl Into<String>) -> Self {
        self.peer_worker_id = Some(peer_worker_id.into());
        self
    }

    /// Appends one model entry.
    #[must_use]
    pub fn with_model(mut self, model: RoutedModelInventory) -> Self {
        self.models.push(model);
        self
    }

    /// Appends multiple model entries.
    #[must_use]
    pub fn with_model_entries<I>(mut self, models: I) -> Self
    where
        I: IntoIterator<Item = RoutedModelInventory>,
    {
        self.models.extend(models);
        self
    }
}

/// Machine-checkable route chosen by the router.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteSelectionStrategy {
    /// No warm/cache state was available, so deterministic first-ready routing won.
    FirstReady,
    /// Cache-compatible candidates existed and one was selected directly.
    CacheAware,
    /// Warm candidates existed and one was selected directly.
    WarmAware,
    /// A bounded power-of-two choice among an already eligible pool picked the least-loaded route.
    PowerOfTwoLeastLoaded,
}

/// Inspectable metrics and trace output for one placement decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteSelectionMetrics {
    /// Number of candidates that passed compatibility checks.
    pub eligible_workers: usize,
    /// Number of candidates that were already warm.
    pub warm_workers: usize,
    /// Number of candidates with a safe cache-affinity match.
    pub cache_matches: usize,
    /// Number of candidates sampled by the bounded-choice picker.
    pub sampled_workers: usize,
    /// Active requests on the selected route at selection time.
    pub selected_active_requests: usize,
    /// Policy that selected the final route.
    pub strategy: RouteSelectionStrategy,
    /// Explicit reason when routing had to fall back to a simpler policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
}

/// Machine-checkable route chosen by the router.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RouteSelection {
    /// Worker chosen for the request.
    pub worker_id: String,
    /// Upstream mesh peer identifier when the selected worker is proxy-backed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_worker_id: Option<String>,
    /// Stable model key routed to.
    pub model_key: String,
    /// Canonical model name exposed to callers.
    pub canonical_name: String,
    /// API endpoint that was routed.
    pub endpoint: RoutingEndpoint,
    /// Model family label.
    pub family: String,
    /// Worker backend label.
    pub backend_label: String,
    /// Worker execution mode.
    pub execution_mode_label: String,
    /// Worker execution engine.
    pub execution_engine_label: String,
    /// Whether execution stays local or crosses the remote-proxy boundary.
    pub execution_locality: RoutedExecutionLocality,
    /// Typed provenance for the selected execution path.
    pub execution_provenance: RoutedExecutionProvenance,
    /// Routed execution profile.
    pub execution_profile: ExecutionCapabilityProfile,
    /// Routed scheduler policy when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_policy: Option<GenerationSchedulerPolicy>,
    /// Active KV-cache encoding policy for the selected route, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_cache_encoding_policy: Option<KvCacheEncodingPolicy>,
    /// Declared KV-cache encoding capabilities for the selected route.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_kv_cache_encoding_policies: Vec<KvCacheEncodingPolicy>,
    /// Inspectable metrics for the selection.
    pub metrics: RouteSelectionMetrics,
    /// Plain-language route notes explaining tie-breaks and filters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routing_notes: Vec<String>,
}

#[derive(Clone, Debug)]
struct RouteBinding {
    worker_id: String,
    model_key: String,
}

#[derive(Clone, Debug)]
struct EligibleRoute {
    preference_rank: usize,
    active_requests: usize,
    warm: bool,
    cache_match_tokens: usize,
    preferred_kv_cache_encoding_match: bool,
    selection: RouteSelection,
}

/// Errors produced while constructing or using the router.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RoutingError {
    #[error("router requires at least one worker inventory")]
    EmptyWorkerInventory,
    #[error("worker `{worker_id}` was declared more than once")]
    DuplicateWorkerId { worker_id: String },
    #[error("default model `{default_model}` is not present in router inventory")]
    UnknownDefaultModel { default_model: String },
    #[error("requested model `{requested}` is not loaded")]
    UnknownRequestedModel { requested: String },
    #[error("requested model key `{model_key}` is not loaded")]
    UnknownModelKey { model_key: String },
    #[error(
        "router inventory is inconsistent: worker `{worker_id}` is missing model `{model_key}`"
    )]
    InconsistentInventory {
        worker_id: String,
        model_key: String,
    },
    #[error("no eligible route for target `{target}` on `{endpoint}`: {reason}")]
    NoEligibleRoute {
        target: String,
        endpoint: String,
        reason: String,
    },
}

/// Deterministic router for multi-model Psionic worker fleets.
#[derive(Clone, Debug)]
pub struct FleetRouter {
    default_model: String,
    workers_by_id: BTreeMap<String, RoutedWorkerInventory>,
    aliases: BTreeMap<String, Vec<RouteBinding>>,
    model_keys: BTreeMap<String, Vec<RouteBinding>>,
}

impl FleetRouter {
    /// Builds one router over worker inventories.
    pub fn new(
        default_model: impl Into<String>,
        workers: Vec<RoutedWorkerInventory>,
    ) -> Result<Self, RoutingError> {
        if workers.is_empty() {
            return Err(RoutingError::EmptyWorkerInventory);
        }
        let default_model = default_model.into();
        let mut workers_by_id = BTreeMap::new();
        let mut aliases = BTreeMap::new();
        let mut model_keys = BTreeMap::new();
        for worker in workers {
            if workers_by_id.contains_key(worker.worker_id.as_str()) {
                return Err(RoutingError::DuplicateWorkerId {
                    worker_id: worker.worker_id,
                });
            }
            for model in &worker.models {
                let binding = RouteBinding {
                    worker_id: worker.worker_id.clone(),
                    model_key: model.model_key.clone(),
                };
                model_keys
                    .entry(model.model_key.clone())
                    .or_insert_with(Vec::new)
                    .push(binding.clone());
                for alias in &model.aliases {
                    aliases
                        .entry(alias.clone())
                        .or_insert_with(Vec::new)
                        .push(binding.clone());
                }
            }
            workers_by_id.insert(worker.worker_id.clone(), worker);
        }
        let has_default = aliases.contains_key(default_model.as_str())
            || model_keys.contains_key(default_model.as_str());
        if !has_default {
            return Err(RoutingError::UnknownDefaultModel { default_model });
        }
        Ok(Self {
            default_model,
            workers_by_id,
            aliases,
            model_keys,
        })
    }

    /// Returns the configured default model target.
    #[must_use]
    pub fn default_model(&self) -> &str {
        self.default_model.as_str()
    }

    /// Returns cloned worker inventory for diagnostic surfaces.
    #[must_use]
    pub fn inventory(&self) -> Vec<RoutedWorkerInventory> {
        self.workers_by_id.values().cloned().collect()
    }

    /// Returns one worker inventory entry by stable worker identifier.
    #[must_use]
    pub fn worker(&self, worker_id: &str) -> Option<&RoutedWorkerInventory> {
        self.workers_by_id.get(worker_id)
    }

    /// Returns one routed model entry by stable worker identifier and model key.
    #[must_use]
    pub fn routed_model(&self, worker_id: &str, model_key: &str) -> Option<&RoutedModelInventory> {
        self.worker(worker_id).and_then(|worker| {
            worker
                .models
                .iter()
                .find(|model| model.model_key == model_key)
        })
    }

    /// Resolves one route request into a concrete worker and model path.
    pub fn resolve(&self, request: &RoutingRequest) -> Result<RouteSelection, RoutingError> {
        let target_label = target_label(&request.target, self.default_model.as_str());
        let candidates = self.candidates_for_target(&request.target)?;
        let mut refusal_notes = Vec::new();
        let mut eligible = Vec::new();

        for binding in candidates {
            let worker = self
                .workers_by_id
                .get(binding.worker_id.as_str())
                .ok_or_else(|| RoutingError::InconsistentInventory {
                    worker_id: binding.worker_id.clone(),
                    model_key: binding.model_key.clone(),
                })?;
            let model = worker
                .models
                .iter()
                .find(|candidate| candidate.model_key == binding.model_key)
                .ok_or_else(|| RoutingError::InconsistentInventory {
                    worker_id: worker.worker_id.clone(),
                    model_key: binding.model_key.clone(),
                })?;

            if !model.supported_endpoints.contains(&request.endpoint) {
                let supported = model
                    .supported_endpoints
                    .iter()
                    .map(|endpoint| endpoint.path())
                    .collect::<Vec<_>>()
                    .join(", ");
                refusal_notes.push(format!(
                    "worker `{}` model `{}` does not support `{}`; supported endpoints: {}",
                    worker.worker_id,
                    model.canonical_name,
                    request.endpoint.path(),
                    supported
                ));
                continue;
            }
            if let Some(preferred_family) = request.preferred_family.as_deref()
                && model.family != preferred_family
            {
                refusal_notes.push(format!(
                    "worker `{}` model `{}` is family `{}` not requested family `{preferred_family}`",
                    worker.worker_id, model.canonical_name, model.family
                ));
                continue;
            }
            if request.capability_filters.structured_outputs && !model.structured_outputs {
                refusal_notes.push(format!(
                    "worker `{}` model `{}` lacks structured-output support",
                    worker.worker_id, model.canonical_name
                ));
                continue;
            }
            if request.capability_filters.tool_calling && !model.tool_calling {
                refusal_notes.push(format!(
                    "worker `{}` model `{}` lacks tool-calling support",
                    worker.worker_id, model.canonical_name
                ));
                continue;
            }
            if request.capability_filters.response_state && !model.response_state {
                refusal_notes.push(format!(
                    "worker `{}` model `{}` lacks response-state support",
                    worker.worker_id, model.canonical_name
                ));
                continue;
            }
            if let Some(required_family) = request.kv_cache_encoding_preferences.require
                && !model_supports_kv_cache_encoding(model, required_family)
            {
                refusal_notes.push(format!(
                    "worker `{}` model `{}` lacks required kv-cache encoding support `{}`",
                    worker.worker_id,
                    model.canonical_name,
                    required_family.as_str()
                ));
                continue;
            }
            if let Some(excluded_family) = request
                .kv_cache_encoding_preferences
                .exclude
                .iter()
                .copied()
                .find(|family| model_supports_kv_cache_encoding(model, *family))
            {
                refusal_notes.push(format!(
                    "worker `{}` model `{}` advertises excluded kv-cache encoding support `{}`",
                    worker.worker_id,
                    model.canonical_name,
                    excluded_family.as_str()
                ));
                continue;
            }

            let preference_rank = request
                .preferred_worker_ids
                .iter()
                .position(|preferred| preferred == &worker.worker_id)
                .unwrap_or(usize::MAX);
            eligible.push(EligibleRoute {
                preference_rank,
                active_requests: model.runtime_state.active_requests,
                warm: model.runtime_state.warm_state.is_warm(),
                cache_match_tokens: cache_match_tokens(model, request),
                preferred_kv_cache_encoding_match: request
                    .kv_cache_encoding_preferences
                    .prefer
                    .is_some_and(|family| model_supports_kv_cache_encoding(model, family)),
                selection: self.selection_for(worker, model, request.endpoint, &request.target),
            });
        }

        let eligible_workers = eligible.len();
        let warm_workers = eligible.iter().filter(|candidate| candidate.warm).count();
        let cache_matches = eligible
            .iter()
            .filter(|candidate| candidate.cache_match_tokens > 0)
            .count();
        let Some(mut selection) = self.select_from_policy_pool(
            eligible,
            request,
            eligible_workers,
            warm_workers,
            cache_matches,
        ) else {
            let reason = if refusal_notes.is_empty() {
                String::from("no candidates matched the requested target")
            } else {
                refusal_notes.join("; ")
            };
            return Err(RoutingError::NoEligibleRoute {
                target: target_label,
                endpoint: request.endpoint.path().to_string(),
                reason,
            });
        };
        if let Some(preferred_family) = request.preferred_family.as_deref() {
            selection.routing_notes.push(format!(
                "family filter `{preferred_family}` matched routed model"
            ));
        }
        if !request.capability_filters.is_empty() {
            selection.routing_notes.push(String::from(
                "capability filters were satisfied by the selected worker route",
            ));
        }
        if let Some(required_family) = request.kv_cache_encoding_preferences.require {
            selection.routing_notes.push(format!(
                "required kv-cache encoding support `{}` matched routed capability publication",
                required_family.as_str()
            ));
        }
        if let Some(preferred_family) = request.kv_cache_encoding_preferences.prefer {
            if route_supports_kv_cache_encoding(&selection, preferred_family) {
                selection.routing_notes.push(format!(
                    "preferred kv-cache encoding support `{}` matched routed capability publication",
                    preferred_family.as_str()
                ));
            } else {
                selection.routing_notes.push(format!(
                    "preferred kv-cache encoding support `{}` was unavailable, so placement continued on the remaining eligible pool",
                    preferred_family.as_str()
                ));
            }
        }
        if !request.kv_cache_encoding_preferences.exclude.is_empty() {
            let excluded = request
                .kv_cache_encoding_preferences
                .exclude
                .iter()
                .map(|family| family.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            selection.routing_notes.push(format!(
                "excluded kv-cache encoding support filters were applied: {excluded}"
            ));
        }
        Ok(selection)
    }

    fn select_from_policy_pool(
        &self,
        mut eligible: Vec<EligibleRoute>,
        request: &RoutingRequest,
        eligible_workers: usize,
        warm_workers: usize,
        cache_matches: usize,
    ) -> Option<RouteSelection> {
        if eligible.is_empty() {
            return None;
        }
        sort_candidates(eligible.as_mut_slice());

        let preferred_eligible = if request.kv_cache_encoding_preferences.prefer.is_some() {
            let preferred = eligible
                .iter()
                .filter(|candidate| candidate.preferred_kv_cache_encoding_match)
                .cloned()
                .collect::<Vec<_>>();
            if preferred.is_empty() {
                eligible.clone()
            } else {
                preferred
            }
        } else {
            eligible.clone()
        };

        let mut fallback_reason = None;
        let mut pool_reason = "eligible";
        let cache_pool = preferred_eligible
            .iter()
            .filter(|candidate| candidate.cache_match_tokens > 0)
            .cloned()
            .collect::<Vec<_>>();
        let warm_pool = preferred_eligible
            .iter()
            .filter(|candidate| candidate.warm)
            .cloned()
            .collect::<Vec<_>>();

        let selected_pool = if !cache_pool.is_empty() {
            pool_reason = "cache-matched";
            cache_pool
        } else if request.policy_hints.cache_key.is_some()
            && request.policy_hints.tenant_scope.is_none()
        {
            fallback_reason = Some(String::from(
                "cache-affinity hint omitted tenant scope, so cache-aware placement was skipped",
            ));
            if !warm_pool.is_empty() {
                pool_reason = "warm";
                warm_pool
            } else {
                preferred_eligible.clone()
            }
        } else if request.policy_hints.cache_key.is_some() {
            fallback_reason = Some(String::from(
                "no safe cache-compatible worker route was available, so cache-aware placement fell back",
            ));
            if !warm_pool.is_empty() {
                pool_reason = "warm";
                warm_pool
            } else {
                preferred_eligible.clone()
            }
        } else if !warm_pool.is_empty() {
            pool_reason = "warm";
            warm_pool
        } else {
            fallback_reason = Some(String::from(
                "no warm or cache-compatible worker route was available, so placement fell back to first-ready",
            ));
            preferred_eligible.clone()
        };

        let mut pool = selected_pool;
        sort_candidates(pool.as_mut_slice());
        let mut strategy = match pool_reason {
            "cache-matched" => RouteSelectionStrategy::CacheAware,
            "warm" => RouteSelectionStrategy::WarmAware,
            _ => RouteSelectionStrategy::FirstReady,
        };
        let mut sampled_workers = 1usize;
        let chosen = if pool.len() > 1 && !matches!(strategy, RouteSelectionStrategy::FirstReady) {
            let sampled =
                power_of_two_sample(pool.as_slice(), request, self.default_model.as_str());
            sampled_workers = sampled.len();
            strategy = RouteSelectionStrategy::PowerOfTwoLeastLoaded;
            sampled
                .into_iter()
                .min_by(|left, right| compare_candidates(left, right))
                .cloned()
                .unwrap_or_else(|| pool[0].clone())
        } else {
            pool.into_iter().next().expect("non-empty pool guaranteed")
        };

        let mut selection = chosen.selection;
        selection.metrics = RouteSelectionMetrics {
            eligible_workers,
            warm_workers,
            cache_matches,
            sampled_workers,
            selected_active_requests: chosen.active_requests,
            strategy,
            fallback_reason: fallback_reason.clone(),
        };
        if chosen.preference_rank != usize::MAX {
            selection.routing_notes.push(format!(
                "selected preferred worker `{}` as the final route tiebreak",
                selection.worker_id
            ));
        }
        selection.routing_notes.push(format!(
            "placement policy selected the route from the `{pool_reason}` candidate pool"
        ));
        if let Some(fallback_reason) = fallback_reason {
            selection.routing_notes.push(fallback_reason);
        }
        Some(selection)
    }

    fn candidates_for_target(
        &self,
        target: &RoutingTarget,
    ) -> Result<&[RouteBinding], RoutingError> {
        match target {
            RoutingTarget::Default => self
                .aliases
                .get(self.default_model.as_str())
                .map(Vec::as_slice)
                .or_else(|| {
                    self.model_keys
                        .get(self.default_model.as_str())
                        .map(Vec::as_slice)
                })
                .ok_or_else(|| RoutingError::UnknownDefaultModel {
                    default_model: self.default_model.clone(),
                }),
            RoutingTarget::RequestedModel(requested) => self
                .aliases
                .get(requested.as_str())
                .map(Vec::as_slice)
                .ok_or_else(|| RoutingError::UnknownRequestedModel {
                    requested: requested.clone(),
                }),
            RoutingTarget::ModelKey(model_key) => self
                .model_keys
                .get(model_key.as_str())
                .map(Vec::as_slice)
                .ok_or_else(|| RoutingError::UnknownModelKey {
                    model_key: model_key.clone(),
                }),
        }
    }

    fn selection_for(
        &self,
        worker: &RoutedWorkerInventory,
        model: &RoutedModelInventory,
        endpoint: RoutingEndpoint,
        target: &RoutingTarget,
    ) -> RouteSelection {
        let mut routing_notes = vec![format!(
            "resolved target `{}` to model `{}` on worker `{}`",
            target_label(target, self.default_model.as_str()),
            model.canonical_name,
            worker.worker_id
        )];
        if !model.aliases.iter().any(|alias| alias == &model.model_key) {
            routing_notes.push(String::from(
                "selected model key is not exposed as an external alias",
            ));
        }
        RouteSelection {
            worker_id: worker.worker_id.clone(),
            peer_worker_id: worker.peer_worker_id.clone(),
            model_key: model.model_key.clone(),
            canonical_name: model.canonical_name.clone(),
            endpoint,
            family: model.family.clone(),
            backend_label: worker.backend_label.clone(),
            execution_mode_label: worker.execution_mode_label.clone(),
            execution_engine_label: worker.execution_engine_label.clone(),
            execution_locality: worker.execution_locality,
            execution_provenance: worker.execution_provenance,
            execution_profile: model.execution_profile.clone(),
            scheduler_policy: model.scheduler_policy.clone(),
            kv_cache_encoding_policy: model.kv_cache_encoding_policy.clone(),
            supported_kv_cache_encoding_policies: model
                .supported_kv_cache_encoding_policies
                .clone(),
            metrics: RouteSelectionMetrics {
                eligible_workers: 0,
                warm_workers: 0,
                cache_matches: 0,
                sampled_workers: 0,
                selected_active_requests: model.runtime_state.active_requests,
                strategy: RouteSelectionStrategy::FirstReady,
                fallback_reason: None,
            },
            routing_notes,
        }
    }
}

/// Reliability policy enforced by the router control plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteReliabilityPolicy {
    /// Maximum retry attempts before the router refuses the request.
    pub max_retry_attempts: u8,
    /// Maximum queued requests allowed per worker.
    pub max_queue_depth: usize,
    /// Maximum admitted requests per worker before rate limiting queues or refuses new work.
    pub max_requests_per_window: usize,
    /// Consecutive failures that open the worker circuit breaker.
    pub circuit_breaker_failure_threshold: usize,
}

impl Default for RouteReliabilityPolicy {
    fn default() -> Self {
        Self {
            max_retry_attempts: 2,
            max_queue_depth: 8,
            max_requests_per_window: 64,
            circuit_breaker_failure_threshold: 3,
        }
    }
}

/// Circuit-breaker posture for one worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerCircuitState {
    /// Worker is healthy enough to receive traffic.
    Closed,
    /// Worker is quarantined from new traffic.
    Open,
    /// Worker recovered and is waiting for a successful probe.
    HalfOpen,
}

/// High-level router-owned admission action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReliabilityAction {
    /// Execute the request on the selected worker immediately.
    Execute,
    /// Queue the request instead of executing immediately.
    Queue,
    /// Retry the request against a future route selection.
    Retry,
    /// Refuse the request explicitly.
    Refuse,
}

/// Explicit reason for one reliability action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReliabilityReason {
    /// Worker health is offline.
    WorkerOffline,
    /// Worker circuit breaker is open.
    CircuitOpen,
    /// Worker rate limit window is exhausted.
    RateLimited,
    /// Worker queue is already full.
    QueueSaturated,
}

/// Machine-checkable trace for one reliability decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteReliabilityTrace {
    /// Worker the decision applied to.
    pub worker_id: String,
    /// Action the router chose.
    pub action: ReliabilityAction,
    /// Explicit reason for queue, retry, or refusal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<ReliabilityReason>,
    /// Attempt ordinal supplied by the caller.
    pub attempt: u8,
    /// Current queue depth after the decision.
    pub queue_depth: usize,
    /// Current circuit-breaker posture.
    pub circuit_state: WorkerCircuitState,
    /// Current observed worker health.
    pub health_status: HealthStatus,
    /// Remaining retries before the router will refuse.
    pub remaining_retries: u8,
}

/// Inspectable reliability metrics for one worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerReliabilityMetrics {
    /// Worker identifier.
    pub worker_id: String,
    /// Current health status.
    pub health_status: HealthStatus,
    /// Current circuit-breaker posture.
    pub circuit_state: WorkerCircuitState,
    /// Current queued request count.
    pub queue_depth: usize,
    /// Requests admitted in the current synthetic rate-limit window.
    pub requests_in_window: usize,
    /// Consecutive recorded failures.
    pub consecutive_failures: usize,
    /// Number of rate-limited actions observed.
    pub rate_limited_actions: usize,
    /// Number of retry actions observed.
    pub retry_actions: usize,
    /// Number of queue actions observed.
    pub queued_actions: usize,
}

#[derive(Clone, Debug)]
struct WorkerReliabilityState {
    health_status: HealthStatus,
    circuit_state: WorkerCircuitState,
    queue_depth: usize,
    requests_in_window: usize,
    consecutive_failures: usize,
    rate_limited_actions: usize,
    retry_actions: usize,
    queued_actions: usize,
}

impl Default for WorkerReliabilityState {
    fn default() -> Self {
        Self {
            health_status: HealthStatus::Ready,
            circuit_state: WorkerCircuitState::Closed,
            queue_depth: 0,
            requests_in_window: 0,
            consecutive_failures: 0,
            rate_limited_actions: 0,
            retry_actions: 0,
            queued_actions: 0,
        }
    }
}

/// Mutable reliability controller layered on top of routed worker selections.
#[derive(Clone, Debug)]
pub struct RouteReliabilityController {
    policy: RouteReliabilityPolicy,
    workers: BTreeMap<String, WorkerReliabilityState>,
}

impl RouteReliabilityController {
    /// Builds a reliability controller over a router inventory snapshot.
    #[must_use]
    pub fn new(router: &FleetRouter, policy: RouteReliabilityPolicy) -> Self {
        let workers = router
            .inventory()
            .into_iter()
            .map(|worker| (worker.worker_id, WorkerReliabilityState::default()))
            .collect();
        Self { policy, workers }
    }

    /// Records the latest health posture for one worker.
    pub fn observe_worker_health(&mut self, worker_id: &str, status: HealthStatus) {
        let state = self.workers.entry(worker_id.to_string()).or_default();
        state.health_status = status;
        state.circuit_state = match status {
            HealthStatus::Ready => {
                if matches!(state.circuit_state, WorkerCircuitState::Open) {
                    WorkerCircuitState::HalfOpen
                } else {
                    state.circuit_state
                }
            }
            HealthStatus::Degraded => state.circuit_state,
            HealthStatus::Offline => WorkerCircuitState::Open,
        };
    }

    /// Admits, queues, retries, or refuses one selected route.
    pub fn admit(&mut self, selection: &RouteSelection, attempt: u8) -> RouteReliabilityTrace {
        let state = self.workers.entry(selection.worker_id.clone()).or_default();
        if state.health_status == HealthStatus::Offline {
            return reliability_retry_or_refuse(
                state,
                selection.worker_id.clone(),
                attempt,
                self.policy.max_retry_attempts,
                ReliabilityReason::WorkerOffline,
            );
        }
        if matches!(state.circuit_state, WorkerCircuitState::Open) {
            return reliability_retry_or_refuse(
                state,
                selection.worker_id.clone(),
                attempt,
                self.policy.max_retry_attempts,
                ReliabilityReason::CircuitOpen,
            );
        }
        if state.requests_in_window >= self.policy.max_requests_per_window {
            state.rate_limited_actions = state.rate_limited_actions.saturating_add(1);
            if state.queue_depth < self.policy.max_queue_depth {
                state.queue_depth = state.queue_depth.saturating_add(1);
                state.queued_actions = state.queued_actions.saturating_add(1);
                return RouteReliabilityTrace {
                    worker_id: selection.worker_id.clone(),
                    action: ReliabilityAction::Queue,
                    reason: Some(ReliabilityReason::RateLimited),
                    attempt,
                    queue_depth: state.queue_depth,
                    circuit_state: state.circuit_state,
                    health_status: state.health_status,
                    remaining_retries: self.policy.max_retry_attempts.saturating_sub(attempt),
                };
            }
            return reliability_retry_or_refuse(
                state,
                selection.worker_id.clone(),
                attempt,
                self.policy.max_retry_attempts,
                ReliabilityReason::QueueSaturated,
            );
        }
        if state.queue_depth >= self.policy.max_queue_depth {
            return reliability_retry_or_refuse(
                state,
                selection.worker_id.clone(),
                attempt,
                self.policy.max_retry_attempts,
                ReliabilityReason::QueueSaturated,
            );
        }

        state.queue_depth = state.queue_depth.saturating_add(1);
        state.requests_in_window = state.requests_in_window.saturating_add(1);
        RouteReliabilityTrace {
            worker_id: selection.worker_id.clone(),
            action: ReliabilityAction::Execute,
            reason: None,
            attempt,
            queue_depth: state.queue_depth,
            circuit_state: state.circuit_state,
            health_status: state.health_status,
            remaining_retries: self.policy.max_retry_attempts.saturating_sub(attempt),
        }
    }

    /// Records a successful execution completion and clears failure state.
    pub fn record_success(&mut self, worker_id: &str) {
        let state = self.workers.entry(worker_id.to_string()).or_default();
        state.queue_depth = state.queue_depth.saturating_sub(1);
        state.consecutive_failures = 0;
        state.circuit_state = WorkerCircuitState::Closed;
        if state.health_status == HealthStatus::Offline {
            state.health_status = HealthStatus::Degraded;
        }
    }

    /// Records a failed execution completion and opens the circuit when needed.
    pub fn record_failure(&mut self, worker_id: &str) {
        let state = self.workers.entry(worker_id.to_string()).or_default();
        state.queue_depth = state.queue_depth.saturating_sub(1);
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);
        if state.consecutive_failures >= self.policy.circuit_breaker_failure_threshold {
            state.circuit_state = WorkerCircuitState::Open;
        }
    }

    /// Resets the synthetic rate-limit window counters.
    pub fn reset_rate_window(&mut self, worker_id: &str) {
        if let Some(state) = self.workers.get_mut(worker_id) {
            state.requests_in_window = 0;
        }
    }

    /// Returns a metrics snapshot for all tracked workers.
    #[must_use]
    pub fn metrics(&self) -> Vec<WorkerReliabilityMetrics> {
        self.workers
            .iter()
            .map(|(worker_id, state)| WorkerReliabilityMetrics {
                worker_id: worker_id.clone(),
                health_status: state.health_status,
                circuit_state: state.circuit_state,
                queue_depth: state.queue_depth,
                requests_in_window: state.requests_in_window,
                consecutive_failures: state.consecutive_failures,
                rate_limited_actions: state.rate_limited_actions,
                retry_actions: state.retry_actions,
                queued_actions: state.queued_actions,
            })
            .collect()
    }
}

fn reliability_retry_or_refuse(
    state: &mut WorkerReliabilityState,
    worker_id: String,
    attempt: u8,
    max_retry_attempts: u8,
    reason: ReliabilityReason,
) -> RouteReliabilityTrace {
    let remaining_retries = max_retry_attempts.saturating_sub(attempt);
    let action = if attempt < max_retry_attempts {
        state.retry_actions = state.retry_actions.saturating_add(1);
        ReliabilityAction::Retry
    } else {
        ReliabilityAction::Refuse
    };
    RouteReliabilityTrace {
        worker_id,
        action,
        reason: Some(reason),
        attempt,
        queue_depth: state.queue_depth,
        circuit_state: state.circuit_state,
        health_status: state.health_status,
        remaining_retries,
    }
}

fn cache_match_tokens(model: &RoutedModelInventory, request: &RoutingRequest) -> usize {
    let Some(cache_key) = request.policy_hints.cache_key.as_deref() else {
        return 0;
    };
    let Some(tenant_scope) = request.policy_hints.tenant_scope.as_deref() else {
        return 0;
    };
    model
        .runtime_state
        .cache_entries
        .iter()
        .fold(0, |best, entry| {
            if entry.cache_key != cache_key || entry.tenant_scope != tenant_scope {
                return best;
            }
            if let Some(topology_scope) = request.policy_hints.topology_scope.as_deref()
                && entry.topology_scope.as_deref() != Some(topology_scope)
            {
                return best;
            }
            if !cache_entry_matches_kv_cache_encoding(entry, model, request) {
                return best;
            }
            best.max(entry.reusable_tokens)
        })
}

fn sort_candidates(candidates: &mut [EligibleRoute]) {
    candidates.sort_by(compare_candidates);
}

fn compare_candidates(left: &EligibleRoute, right: &EligibleRoute) -> std::cmp::Ordering {
    left.preference_rank
        .cmp(&right.preference_rank)
        .then_with(|| right.cache_match_tokens.cmp(&left.cache_match_tokens))
        .then_with(|| right.warm.cmp(&left.warm))
        .then_with(|| left.active_requests.cmp(&right.active_requests))
        .then_with(|| left.selection.worker_id.cmp(&right.selection.worker_id))
        .then_with(|| left.selection.model_key.cmp(&right.selection.model_key))
}

fn power_of_two_sample<'a>(
    candidates: &'a [EligibleRoute],
    request: &RoutingRequest,
    default_model: &str,
) -> Vec<&'a EligibleRoute> {
    if candidates.len() <= 2 {
        return candidates.iter().collect();
    }
    let mut hasher = DefaultHasher::new();
    request.endpoint.path().hash(&mut hasher);
    target_label(&request.target, default_model).hash(&mut hasher);
    request.product_id.hash(&mut hasher);
    request.policy_hints.cache_key.hash(&mut hasher);
    request
        .kv_cache_encoding_preferences
        .require
        .map(KvCacheEncodingFamily::as_str)
        .hash(&mut hasher);
    request
        .kv_cache_encoding_preferences
        .prefer
        .map(KvCacheEncodingFamily::as_str)
        .hash(&mut hasher);
    for family in &request.kv_cache_encoding_preferences.exclude {
        family.as_str().hash(&mut hasher);
    }
    request.policy_hints.tenant_scope.hash(&mut hasher);
    request.policy_hints.topology_scope.hash(&mut hasher);
    request.policy_hints.request_key.hash(&mut hasher);
    request.preferred_worker_ids.hash(&mut hasher);
    request.preferred_family.hash(&mut hasher);
    let first_index = (hasher.finish() as usize) % candidates.len();
    let mut second_index = (first_index + (candidates.len() / 2).max(1)) % candidates.len();
    if second_index == first_index {
        second_index = (first_index + 1) % candidates.len();
    }
    vec![&candidates[first_index], &candidates[second_index]]
}

fn target_label(target: &RoutingTarget, default_model: &str) -> String {
    match target {
        RoutingTarget::Default => format!("default:{default_model}"),
        RoutingTarget::RequestedModel(requested) => format!("requested:{requested}"),
        RoutingTarget::ModelKey(model_key) => format!("model_key:{model_key}"),
    }
}

fn route_alias_for_demand(request: &RoutingRequest) -> Option<String> {
    match &request.target {
        RoutingTarget::RequestedModel(requested) => Some(requested.clone()),
        RoutingTarget::Default | RoutingTarget::ModelKey(_) => None,
    }
}

fn route_supports_kv_cache_encoding(
    selection: &RouteSelection,
    family: KvCacheEncodingFamily,
) -> bool {
    selection
        .kv_cache_encoding_policy
        .as_ref()
        .is_some_and(|policy| policy.family == family)
        || selection
            .supported_kv_cache_encoding_policies
            .iter()
            .any(|policy| policy.family == family)
}

fn model_supports_kv_cache_encoding(
    model: &RoutedModelInventory,
    family: KvCacheEncodingFamily,
) -> bool {
    model
        .kv_cache_encoding_policy
        .as_ref()
        .is_some_and(|policy| policy.family == family)
        || model
            .supported_kv_cache_encoding_policies
            .iter()
            .any(|policy| policy.family == family)
}

fn model_kv_cache_encoding_policy_for_family<'a>(
    model: &'a RoutedModelInventory,
    family: KvCacheEncodingFamily,
) -> Option<&'a KvCacheEncodingPolicy> {
    model
        .supported_kv_cache_encoding_policies
        .iter()
        .find(|policy| policy.family == family)
        .or_else(|| {
            model
                .kv_cache_encoding_policy
                .as_ref()
                .filter(|policy| policy.family == family)
        })
}

fn requested_route_kv_cache_encoding_policy<'a>(
    model: &'a RoutedModelInventory,
    request: &RoutingRequest,
) -> Option<&'a KvCacheEncodingPolicy> {
    if let Some(required_family) = request.kv_cache_encoding_preferences.require {
        return model_kv_cache_encoding_policy_for_family(model, required_family);
    }
    if let Some(preferred_family) = request.kv_cache_encoding_preferences.prefer
        && let Some(policy) = model_kv_cache_encoding_policy_for_family(model, preferred_family)
    {
        return Some(policy);
    }
    model.kv_cache_encoding_policy.as_ref()
}

fn kv_cache_encoding_policies_compatible(
    left: &KvCacheEncodingPolicy,
    right: &KvCacheEncodingPolicy,
) -> bool {
    left.family == right.family
        && left.objective == right.objective
        && left.bits_per_channel == right.bits_per_channel
        && left.block_shape == right.block_shape
        && left.outlier_policy == right.outlier_policy
        && left.projection_id == right.projection_id
        && left.codebook_id == right.codebook_id
}

fn cache_entry_matches_kv_cache_encoding(
    entry: &RoutedCacheEntry,
    model: &RoutedModelInventory,
    request: &RoutingRequest,
) -> bool {
    let route_policy = requested_route_kv_cache_encoding_policy(model, request);
    match (entry.kv_cache_encoding_policy.as_ref(), route_policy) {
        (Some(entry_policy), Some(route_policy)) => {
            kv_cache_encoding_policies_compatible(entry_policy, route_policy)
        }
        (Some(_), None) => false,
        (None, Some(route_policy)) => route_policy.family != KvCacheEncodingFamily::TurboQuant,
        (None, None) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FleetRouter, ReliabilityAction, ReliabilityReason, RouteReliabilityController,
        RouteReliabilityPolicy, RouteSelectionStrategy, RoutedCacheEntry,
        RoutedClusterExecutionMode, RoutedExecutionLocality, RoutedExecutionProvenance,
        RoutedModelInventory, RoutedSparseExpertRuntimeContract, RoutedSparseExpertTopology,
        RoutedSparseShardArtifactStatus, RoutedSparseShardHealth, RoutedSparseShardReplica,
        RoutedSparseShardState, RoutedWarmState, RoutedWorkerInventory, RoutingDemandKey,
        RoutingDemandLedger, RoutingDemandPolicy, RoutingEndpoint, RoutingError, RoutingRequest,
        WorkerCircuitState,
    };
    use psionic_runtime::{
        ClusterExecutionCapabilityProfile, ClusterExecutionLane, ClusterServingSemantics,
        ClusterWarmRoutePosture, ExecutionCapabilityProfile, ExecutionTopologyKind, HealthStatus,
        KvCacheEncodingFamily, KvCacheEncodingObjective, KvCacheEncodingPolicy,
        PrefillDecodeCapability,
    };

    fn sample_profile() -> ExecutionCapabilityProfile {
        ExecutionCapabilityProfile::single_request_latency_optimized()
            .with_prefill_decode_capability(PrefillDecodeCapability::colocated_split())
    }

    fn dense_f16_kv_policy() -> KvCacheEncodingPolicy {
        KvCacheEncodingPolicy::dense_f16_mirror(80, 40, "gpt-oss", 131_072)
    }

    fn turboquant_kv_policy() -> KvCacheEncodingPolicy {
        KvCacheEncodingPolicy {
            family: KvCacheEncodingFamily::TurboQuant,
            objective: Some(KvCacheEncodingObjective::MeanSquaredError),
            bits_per_channel: Some(8),
            block_shape: Some(String::from("32")),
            outlier_policy: None,
            projection_id: None,
            codebook_id: Some(String::from("ggml_q8_1")),
            model_family_bound: Some(String::from("gpt-oss")),
            context_length_bound: Some(131_072),
            host_bytes_per_token: Some(80),
            device_bytes_per_token: Some(18),
            detail: Some(String::from("test turboquant capability")),
        }
    }

    #[test]
    fn router_resolves_default_model_on_single_worker() {
        let router = FleetRouter::new(
            "tiny-llama",
            vec![
                RoutedWorkerInventory::new("worker-a", "cpu", "native", "psionic").with_model(
                    RoutedModelInventory::new(
                        "tiny-llama",
                        "tiny-llama",
                        "llama",
                        sample_profile(),
                    )
                    .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                    .with_structured_outputs()
                    .with_tool_calling()
                    .with_response_state(),
                ),
            ],
        )
        .expect("router should build");

        let selection = router
            .resolve(&RoutingRequest::new(RoutingEndpoint::ChatCompletions))
            .expect("default route should resolve");
        assert_eq!(selection.worker_id, "worker-a");
        assert_eq!(selection.model_key, "tiny-llama");
        assert_eq!(selection.execution_locality, RoutedExecutionLocality::Local);
        assert_eq!(
            selection.execution_provenance,
            RoutedExecutionProvenance::LocalExecution
        );
    }

    #[test]
    fn router_publishes_remote_bootstrap_worker_truth() {
        let router = FleetRouter::new(
            "tiny-llama",
            vec![
                RoutedWorkerInventory::new("mesh-peer-a", "cuda", "proxy", "psionic")
                    .as_remote_bootstrap_proxy()
                    .with_model(
                        RoutedModelInventory::new(
                            "tiny-llama",
                            "tiny-llama",
                            "llama",
                            sample_profile(),
                        )
                        .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                        .with_tool_calling(),
                    ),
            ],
        )
        .expect("router should build");

        let worker = router
            .worker("mesh-peer-a")
            .expect("remote worker should stay addressable by worker id");
        assert_eq!(
            worker.execution_locality,
            RoutedExecutionLocality::RemoteProxy
        );
        assert_eq!(
            worker.execution_provenance,
            RoutedExecutionProvenance::BootstrapProxy
        );
        assert!(router.routed_model("mesh-peer-a", "tiny-llama").is_some());

        let selection = router
            .resolve(
                &RoutingRequest::new(RoutingEndpoint::ChatCompletions)
                    .with_requested_model("tiny-llama")
                    .require_tool_calling(),
            )
            .expect("remote routed model should still satisfy typed capability filters");
        assert_eq!(selection.worker_id, "mesh-peer-a");
        assert_eq!(
            selection.execution_locality,
            RoutedExecutionLocality::RemoteProxy
        );
        assert_eq!(
            selection.execution_provenance,
            RoutedExecutionProvenance::BootstrapProxy
        );
    }

    #[test]
    fn router_routes_remote_cuda_gemma_mesh_lane_honestly() {
        let router = FleetRouter::new(
            "tiny-local-llama",
            vec![
                RoutedWorkerInventory::new("worker-local", "cpu", "native", "psionic").with_model(
                    RoutedModelInventory::new(
                        "tiny-local-llama",
                        "tiny-local-llama",
                        "llama",
                        sample_profile(),
                    )
                    .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                    .with_supported_endpoint(RoutingEndpoint::Responses)
                    .with_response_state(),
                ),
                RoutedWorkerInventory::new(
                    "bootstrap:worker-gemma4-e4b",
                    "cuda",
                    "native",
                    "psionic",
                )
                .as_remote_bootstrap_proxy()
                .with_peer_worker_id("worker-gemma4-e4b")
                .with_model(
                    RoutedModelInventory::new(
                        "gemma4:e4b",
                        "gemma4:e4b",
                        "gemma4",
                        sample_profile(),
                    )
                    .with_supported_endpoint(RoutingEndpoint::ChatCompletions),
                ),
            ],
        )
        .expect("router should build");

        let selection = router
            .resolve(
                &RoutingRequest::new(RoutingEndpoint::ChatCompletions)
                    .with_requested_model("gemma4:e4b"),
            )
            .expect("remote gemma lane should resolve through the mesh");
        assert_eq!(selection.worker_id, "bootstrap:worker-gemma4-e4b");
        assert_eq!(
            selection.peer_worker_id.as_deref(),
            Some("worker-gemma4-e4b")
        );
        assert_eq!(selection.model_key, "gemma4:e4b");
        assert_eq!(selection.backend_label, "cuda");
        assert_eq!(
            selection.execution_locality,
            RoutedExecutionLocality::RemoteProxy
        );
        assert_eq!(
            selection.execution_provenance,
            RoutedExecutionProvenance::BootstrapProxy
        );

        let error = router
            .resolve(
                &RoutingRequest::new(RoutingEndpoint::Responses)
                    .with_requested_model("gemma4:e4b")
                    .require_response_state(),
            )
            .expect_err("mesh should refuse unsupported gemma responses endpoint");
        assert!(matches!(error, RoutingError::NoEligibleRoute { .. }));
        assert!(
            error.to_string().contains("/v1/responses"),
            "refusal should keep the unsupported endpoint explicit"
        );
    }

    #[test]
    fn router_keeps_sparse_gemma26b_topology_truth_in_inventory() {
        let topology = RoutedSparseExpertTopology::new(
            "gemma4",
            "gemma4",
            "gemma4-26b-artifact",
            RoutedSparseExpertRuntimeContract::FamilySpecificPlacement,
            64,
        )
        .with_active_expert_count(4)
        .with_expert_feed_forward_length(4096)
        .with_sharded_model_manifest_digest("gemma4-26b-manifest");
        let router = FleetRouter::new(
            "gemma4:26b",
            vec![
                RoutedWorkerInventory::new("worker-gemma4-26b", "cuda", "native", "psionic")
                    .with_model(
                        RoutedModelInventory::new(
                            "gemma4:26b",
                            "gemma4:26b",
                            "gemma4",
                            sample_profile(),
                        )
                        .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                        .with_supported_endpoint(RoutingEndpoint::Responses)
                        .with_execution_refusal_reason(
                            "model `gemma4:26b` requires distributed sparse placement",
                        )
                        .with_sparse_expert_topology(topology.clone()),
                    ),
            ],
        )
        .expect("router should build");

        let routed_model = router
            .routed_model("worker-gemma4-26b", "gemma4:26b")
            .expect("router inventory should keep gemma4 26b truth");
        assert_eq!(
            routed_model.execution_refusal_reason.as_deref(),
            Some("model `gemma4:26b` requires distributed sparse placement")
        );
        assert_eq!(
            routed_model.sparse_expert_topology.as_ref(),
            Some(&topology)
        );
    }

    #[test]
    fn router_keeps_sparse_shard_truth_in_inventory() {
        let shard_state = RoutedSparseShardState::new(
            "placement-a",
            "inventory-a",
            "version-a",
            RoutedSparseShardHealth::Healthy,
        )
        .with_replica(RoutedSparseShardReplica::new(
            "worker-a",
            0,
            32,
            "cache-a",
            "artifact-a",
            RoutedSparseShardArtifactStatus::Materialized,
        ))
        .with_replica(RoutedSparseShardReplica::new(
            "worker-b",
            32,
            64,
            "cache-b",
            "artifact-b",
            RoutedSparseShardArtifactStatus::Materialized,
        ));
        let router = FleetRouter::new(
            "gemma4:26b",
            vec![
                RoutedWorkerInventory::new("worker-gemma4-26b", "cuda", "native", "psionic")
                    .with_model(
                        RoutedModelInventory::new(
                            "gemma4:26b",
                            "gemma4:26b",
                            "gemma4",
                            sample_profile(),
                        )
                        .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                        .with_supported_endpoint(RoutingEndpoint::Responses)
                        .with_sparse_shard_state(shard_state.clone()),
                    ),
            ],
        )
        .expect("router should build");

        let routed_model = router
            .routed_model("worker-gemma4-26b", "gemma4:26b")
            .expect("router inventory should keep gemma4 26b shard truth");
        assert_eq!(routed_model.sparse_shard_state.as_ref(), Some(&shard_state));
    }

    #[test]
    fn router_keeps_generic_cluster_execution_truth_in_inventory() {
        let capability_profile = ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![ClusterExecutionLane::PipelineSharded])
            .with_serving_semantics_capability(ClusterServingSemantics::new(
                ClusterExecutionLane::PipelineSharded,
                sample_profile(),
                ClusterWarmRoutePosture::TopologyPinned,
            ));
        let router = FleetRouter::new(
            "gemma4:e4b",
            vec![
                RoutedWorkerInventory::new("worker-gemma4-e4b", "cuda", "native", "psionic")
                    .with_model(
                        RoutedModelInventory::new(
                            "gemma4:e4b",
                            "gemma4:e4b",
                            "gemma4",
                            sample_profile(),
                        )
                        .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                        .with_supported_endpoint(RoutingEndpoint::Responses)
                        .with_cluster_execution_mode(RoutedClusterExecutionMode::DenseSplit)
                        .with_cluster_execution_topology(ExecutionTopologyKind::PipelineSharded)
                        .with_cluster_execution_capability_profile(capability_profile.clone()),
                    ),
            ],
        )
        .expect("router should build");

        let routed_model = router
            .routed_model("worker-gemma4-e4b", "gemma4:e4b")
            .expect("router inventory should keep cluster execution truth");
        assert_eq!(
            routed_model.cluster_execution_modes,
            vec![RoutedClusterExecutionMode::DenseSplit]
        );
        assert_eq!(
            routed_model.cluster_execution_topologies,
            vec![ExecutionTopologyKind::PipelineSharded]
        );
        assert_eq!(
            routed_model.cluster_execution_capability_profile.as_ref(),
            Some(&capability_profile)
        );
    }

    #[test]
    fn router_prefers_requested_worker_for_shared_model() {
        let worker_model =
            RoutedModelInventory::new("tiny-llama", "tiny-llama", "llama", sample_profile())
                .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                .with_response_state();
        let router = FleetRouter::new(
            "tiny-llama",
            vec![
                RoutedWorkerInventory::new("worker-a", "cpu", "native", "psionic")
                    .with_model(worker_model.clone()),
                RoutedWorkerInventory::new("worker-b", "cpu", "native", "psionic")
                    .with_model(worker_model),
            ],
        )
        .expect("router should build");

        let selection = router
            .resolve(
                &RoutingRequest::new(RoutingEndpoint::ChatCompletions).prefer_worker("worker-b"),
            )
            .expect("preferred worker should win the tiebreak");
        assert_eq!(selection.worker_id, "worker-b");
        assert!(
            selection
                .routing_notes
                .iter()
                .any(|note| note.contains("preferred worker")),
            "route notes should explain the preferred-worker tiebreak"
        );
    }

    #[test]
    fn router_filters_by_endpoint_and_capability_truth() {
        let router = FleetRouter::new(
            "tiny-embed",
            vec![
                RoutedWorkerInventory::new("worker-a", "cpu", "native", "psionic")
                    .with_model(
                        RoutedModelInventory::new(
                            "tiny-embed",
                            "tiny-embed",
                            "bert",
                            sample_profile(),
                        )
                        .with_supported_endpoint(RoutingEndpoint::Embeddings),
                    )
                    .with_model(
                        RoutedModelInventory::new(
                            "tiny-llama",
                            "tiny-llama",
                            "llama",
                            sample_profile(),
                        )
                        .with_supported_endpoint(RoutingEndpoint::Responses)
                        .with_response_state(),
                    ),
            ],
        )
        .expect("router should build");

        let selection = router
            .resolve(
                &RoutingRequest::new(RoutingEndpoint::Responses)
                    .with_requested_model("tiny-llama")
                    .require_response_state(),
            )
            .expect("response-state model should resolve");
        assert_eq!(selection.model_key, "tiny-llama");
    }

    #[test]
    fn router_refuses_missing_capability() {
        let router = FleetRouter::new(
            "tiny-embed",
            vec![
                RoutedWorkerInventory::new("worker-a", "cpu", "native", "psionic").with_model(
                    RoutedModelInventory::new("tiny-embed", "tiny-embed", "bert", sample_profile())
                        .with_supported_endpoint(RoutingEndpoint::Embeddings),
                ),
            ],
        )
        .expect("router should build");

        let error = router
            .resolve(
                &RoutingRequest::new(RoutingEndpoint::Responses)
                    .with_requested_model("tiny-embed")
                    .require_response_state(),
            )
            .expect_err("missing endpoint and response-state support should be refused");
        assert!(matches!(error, RoutingError::NoEligibleRoute { .. }));
        assert!(
            error.to_string().contains("/v1/responses"),
            "refusal should name the unsupported endpoint"
        );
    }

    #[test]
    fn router_prefers_safe_cache_match_over_cold_route() {
        let cached =
            RoutedModelInventory::new("tiny-llama", "tiny-llama", "llama", sample_profile())
                .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                .with_warm_state(RoutedWarmState::Warm)
                .with_active_requests(3)
                .with_cache_entry(RoutedCacheEntry::new("prefix-hello", "tenant-a", 96));
        let cold = RoutedModelInventory::new("tiny-llama", "tiny-llama", "llama", sample_profile())
            .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
            .with_active_requests(0);
        let router = FleetRouter::new(
            "tiny-llama",
            vec![
                RoutedWorkerInventory::new("worker-a", "cpu", "native", "psionic")
                    .with_model(cached),
                RoutedWorkerInventory::new("worker-b", "cpu", "native", "psionic").with_model(cold),
            ],
        )
        .expect("router should build");

        let selection = router
            .resolve(
                &RoutingRequest::new(RoutingEndpoint::ChatCompletions)
                    .with_cache_affinity("prefix-hello", "tenant-a"),
            )
            .expect("safe cache-matched route should resolve");
        assert_eq!(selection.worker_id, "worker-a");
        assert_eq!(selection.metrics.cache_matches, 1);
        assert!(matches!(
            selection.metrics.strategy,
            RouteSelectionStrategy::CacheAware
        ));
    }

    #[test]
    fn router_never_uses_unsafe_cache_match_across_tenants() {
        let cached =
            RoutedModelInventory::new("tiny-llama", "tiny-llama", "llama", sample_profile())
                .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                .with_warm_state(RoutedWarmState::Warm)
                .with_cache_entry(RoutedCacheEntry::new("prefix-hello", "tenant-a", 96));
        let warm_other =
            RoutedModelInventory::new("tiny-llama", "tiny-llama", "llama", sample_profile())
                .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                .with_warm_state(RoutedWarmState::Warm)
                .with_active_requests(1);
        let router = FleetRouter::new(
            "tiny-llama",
            vec![
                RoutedWorkerInventory::new("worker-a", "cpu", "native", "psionic")
                    .with_model(cached),
                RoutedWorkerInventory::new("worker-b", "cpu", "native", "psionic")
                    .with_model(warm_other),
            ],
        )
        .expect("router should build");

        let selection = router
            .resolve(
                &RoutingRequest::new(RoutingEndpoint::ChatCompletions)
                    .with_cache_affinity("prefix-hello", "tenant-b"),
            )
            .expect("unsafe tenant-mismatched cache hint should fall back safely");
        assert_eq!(selection.metrics.cache_matches, 0);
        assert!(
            selection
                .metrics
                .fallback_reason
                .as_deref()
                .unwrap_or_default()
                .contains("no safe cache-compatible worker route"),
            "fallback reason should explain why cache-aware routing was skipped"
        );
        assert!(
            selection
                .routing_notes
                .iter()
                .any(|note| note.contains("cache-aware placement fell back")),
            "routing notes should preserve the explicit fallback trace"
        );
    }

    #[test]
    fn router_can_require_specific_kv_cache_encoding_support() {
        let dense_only =
            RoutedModelInventory::new("gpt-oss", "gpt-oss", "gpt-oss", sample_profile())
                .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                .with_kv_cache_encoding_policy(dense_f16_kv_policy())
                .with_supported_kv_cache_encoding_policy(dense_f16_kv_policy());
        let turboquant_capable =
            RoutedModelInventory::new("gpt-oss", "gpt-oss", "gpt-oss", sample_profile())
                .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                .with_kv_cache_encoding_policy(dense_f16_kv_policy())
                .with_supported_kv_cache_encoding_policy(dense_f16_kv_policy())
                .with_supported_kv_cache_encoding_policy(turboquant_kv_policy());
        let router = FleetRouter::new(
            "gpt-oss",
            vec![
                RoutedWorkerInventory::new("worker-a", "cuda", "native", "psionic")
                    .with_model(dense_only),
                RoutedWorkerInventory::new("worker-b", "cuda", "native", "psionic")
                    .with_model(turboquant_capable),
            ],
        )
        .expect("router should build");

        let selection = router
            .resolve(
                &RoutingRequest::new(RoutingEndpoint::ChatCompletions)
                    .require_kv_cache_encoding(KvCacheEncodingFamily::TurboQuant),
            )
            .expect("turboquant-capable route should resolve");
        assert_eq!(selection.worker_id, "worker-b");
        assert!(
            selection
                .supported_kv_cache_encoding_policies
                .iter()
                .any(|policy| policy.family == KvCacheEncodingFamily::TurboQuant)
        );
        assert!(selection.routing_notes.iter().any(|note| {
            note.contains("required kv-cache encoding support `turboquant` matched")
        }));
    }

    #[test]
    fn router_prefers_specific_kv_cache_encoding_support_when_available() {
        let dense_only =
            RoutedModelInventory::new("gpt-oss", "gpt-oss", "gpt-oss", sample_profile())
                .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                .with_warm_state(RoutedWarmState::Warm)
                .with_active_requests(0)
                .with_kv_cache_encoding_policy(dense_f16_kv_policy())
                .with_supported_kv_cache_encoding_policy(dense_f16_kv_policy());
        let turboquant_capable =
            RoutedModelInventory::new("gpt-oss", "gpt-oss", "gpt-oss", sample_profile())
                .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                .with_active_requests(4)
                .with_kv_cache_encoding_policy(dense_f16_kv_policy())
                .with_supported_kv_cache_encoding_policy(dense_f16_kv_policy())
                .with_supported_kv_cache_encoding_policy(turboquant_kv_policy());
        let router = FleetRouter::new(
            "gpt-oss",
            vec![
                RoutedWorkerInventory::new("worker-a", "cuda", "native", "psionic")
                    .with_model(dense_only),
                RoutedWorkerInventory::new("worker-b", "cuda", "native", "psionic")
                    .with_model(turboquant_capable),
            ],
        )
        .expect("router should build");

        let selection = router
            .resolve(
                &RoutingRequest::new(RoutingEndpoint::ChatCompletions)
                    .prefer_kv_cache_encoding(KvCacheEncodingFamily::TurboQuant),
            )
            .expect("preferred turboquant route should resolve");
        assert_eq!(selection.worker_id, "worker-b");
        assert!(selection.routing_notes.iter().any(|note| {
            note.contains("preferred kv-cache encoding support `turboquant` matched")
        }));
    }

    #[test]
    fn router_can_exclude_specific_kv_cache_encoding_support() {
        let turboquant_capable =
            RoutedModelInventory::new("gpt-oss", "gpt-oss", "gpt-oss", sample_profile())
                .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                .with_warm_state(RoutedWarmState::Warm)
                .with_kv_cache_encoding_policy(dense_f16_kv_policy())
                .with_supported_kv_cache_encoding_policy(dense_f16_kv_policy())
                .with_supported_kv_cache_encoding_policy(turboquant_kv_policy());
        let dense_only =
            RoutedModelInventory::new("gpt-oss", "gpt-oss", "gpt-oss", sample_profile())
                .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                .with_kv_cache_encoding_policy(dense_f16_kv_policy())
                .with_supported_kv_cache_encoding_policy(dense_f16_kv_policy());
        let router = FleetRouter::new(
            "gpt-oss",
            vec![
                RoutedWorkerInventory::new("worker-a", "cuda", "native", "psionic")
                    .with_model(turboquant_capable),
                RoutedWorkerInventory::new("worker-b", "cuda", "native", "psionic")
                    .with_model(dense_only),
            ],
        )
        .expect("router should build");

        let selection = router
            .resolve(
                &RoutingRequest::new(RoutingEndpoint::ChatCompletions)
                    .exclude_kv_cache_encoding(KvCacheEncodingFamily::TurboQuant),
            )
            .expect("dense-only route should resolve");
        assert_eq!(selection.worker_id, "worker-b");
        assert!(
            selection.routing_notes.iter().any(
                |note| note.contains("excluded kv-cache encoding support filters were applied")
            )
        );
    }

    #[test]
    fn router_only_reuses_shared_prefixes_across_matching_kv_cache_codecs() {
        let turboquant_policy = turboquant_kv_policy();
        let dense_entry = RoutedCacheEntry::new("prefix-hello", "tenant-a", 192)
            .with_kv_cache_encoding_policy(dense_f16_kv_policy());
        let turboquant_entry = RoutedCacheEntry::new("prefix-hello", "tenant-a", 192)
            .with_kv_cache_encoding_policy(turboquant_policy.clone());
        let mismatched_cache =
            RoutedModelInventory::new("gpt-oss", "gpt-oss", "gpt-oss", sample_profile())
                .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                .with_warm_state(RoutedWarmState::Warm)
                .with_kv_cache_encoding_policy(dense_f16_kv_policy())
                .with_supported_kv_cache_encoding_policy(dense_f16_kv_policy())
                .with_supported_kv_cache_encoding_policy(turboquant_policy.clone())
                .with_cache_entry(dense_entry);
        let matching_cache =
            RoutedModelInventory::new("gpt-oss", "gpt-oss", "gpt-oss", sample_profile())
                .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                .with_warm_state(RoutedWarmState::Warm)
                .with_kv_cache_encoding_policy(dense_f16_kv_policy())
                .with_supported_kv_cache_encoding_policy(dense_f16_kv_policy())
                .with_supported_kv_cache_encoding_policy(turboquant_policy.clone())
                .with_cache_entry(turboquant_entry);
        let router = FleetRouter::new(
            "gpt-oss",
            vec![
                RoutedWorkerInventory::new("worker-a", "cuda", "native", "psionic")
                    .with_model(mismatched_cache),
                RoutedWorkerInventory::new("worker-b", "cuda", "native", "psionic")
                    .with_model(matching_cache),
            ],
        )
        .expect("router should build");

        let selection = router
            .resolve(
                &RoutingRequest::new(RoutingEndpoint::ChatCompletions)
                    .require_kv_cache_encoding(KvCacheEncodingFamily::TurboQuant)
                    .with_cache_affinity("prefix-hello", "tenant-a"),
            )
            .expect("matching turboquant cache should resolve");
        assert_eq!(selection.worker_id, "worker-b");
        assert_eq!(selection.metrics.cache_matches, 1);
        assert!(matches!(
            selection.metrics.strategy,
            RouteSelectionStrategy::CacheAware
        ));
    }

    #[test]
    fn router_uses_power_of_two_to_pick_less_loaded_warm_route() {
        let warm_a =
            RoutedModelInventory::new("tiny-llama", "tiny-llama", "llama", sample_profile())
                .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                .with_warm_state(RoutedWarmState::Warm)
                .with_active_requests(7);
        let warm_b =
            RoutedModelInventory::new("tiny-llama", "tiny-llama", "llama", sample_profile())
                .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                .with_warm_state(RoutedWarmState::Warm)
                .with_active_requests(2);
        let router = FleetRouter::new(
            "tiny-llama",
            vec![
                RoutedWorkerInventory::new("worker-a", "cpu", "native", "psionic")
                    .with_model(warm_a),
                RoutedWorkerInventory::new("worker-b", "cpu", "native", "psionic")
                    .with_model(warm_b),
            ],
        )
        .expect("router should build");

        let selection = router
            .resolve(
                &RoutingRequest::new(RoutingEndpoint::ChatCompletions).with_request_key("req-1"),
            )
            .expect("warm routes should resolve");
        assert_eq!(selection.worker_id, "worker-b");
        assert_eq!(selection.metrics.sampled_workers, 2);
        assert!(matches!(
            selection.metrics.strategy,
            RouteSelectionStrategy::PowerOfTwoLeastLoaded
        ));
    }

    #[test]
    fn demand_ledger_keys_snapshots_by_product_model_and_requested_alias() {
        let router = FleetRouter::new(
            "tiny-llama",
            vec![
                RoutedWorkerInventory::new("worker-a", "cpu", "native", "psionic").with_model(
                    RoutedModelInventory::new(
                        "tiny-llama",
                        "tiny-llama",
                        "llama",
                        sample_profile(),
                    )
                    .with_alias("chat-default")
                    .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                    .with_warm_state(RoutedWarmState::Warm)
                    .with_active_requests(3),
                ),
            ],
        )
        .expect("router should build");
        let request = RoutingRequest::new(RoutingEndpoint::ChatCompletions)
            .with_product_id("psionic.openai_compat")
            .with_requested_model("chat-default");
        let selection = router.resolve(&request).expect("route should resolve");
        let mut ledger = RoutingDemandLedger::new(RoutingDemandPolicy {
            freshness_window_ms: 120_000,
        });

        ledger.record(&request, &selection, 10_000);

        let snapshots = ledger.snapshot_at(10_000);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(
            snapshots[0].key,
            RoutingDemandKey::new(
                "psionic.openai_compat",
                "tiny-llama",
                Some(String::from("chat-default")),
            )
        );
        assert_eq!(snapshots[0].request_count, 1);
        assert_eq!(snapshots[0].peak_selected_active_requests, 3);
        assert_eq!(snapshots[0].canonical_name, "tiny-llama");
    }

    #[test]
    fn demand_ledger_marks_stale_windows_and_resets_after_expiry() {
        let router = FleetRouter::new(
            "tiny-llama",
            vec![
                RoutedWorkerInventory::new("worker-a", "cpu", "native", "psionic").with_model(
                    RoutedModelInventory::new(
                        "tiny-llama",
                        "tiny-llama",
                        "llama",
                        sample_profile(),
                    )
                    .with_supported_endpoint(RoutingEndpoint::ChatCompletions)
                    .with_warm_state(RoutedWarmState::Warm)
                    .with_active_requests(1),
                ),
            ],
        )
        .expect("router should build");
        let request =
            RoutingRequest::new(RoutingEndpoint::ChatCompletions).with_product_id("psionic.test");
        let selection = router.resolve(&request).expect("route should resolve");
        let mut ledger = RoutingDemandLedger::new(RoutingDemandPolicy {
            freshness_window_ms: 1_000,
        });

        ledger.record(&request, &selection, 100);
        let stale_snapshot = ledger.snapshot_at(1_101);
        assert_eq!(stale_snapshot.len(), 1);
        assert!(stale_snapshot[0].is_expired_at(1_101));

        ledger.record(&request, &selection, 1_200);
        let refreshed_snapshot = ledger.snapshot_at(1_200);
        assert_eq!(refreshed_snapshot.len(), 1);
        assert_eq!(refreshed_snapshot[0].request_count, 1);
        assert_eq!(refreshed_snapshot[0].first_observed_at_ms, 1_200);
        assert_eq!(refreshed_snapshot[0].last_observed_at_ms, 1_200);
        assert!(!refreshed_snapshot[0].is_expired_at(1_200));
    }

    #[test]
    fn reliability_controller_retries_offline_worker_before_refusing() {
        let router = FleetRouter::new(
            "tiny-llama",
            vec![
                RoutedWorkerInventory::new("worker-a", "cpu", "native", "psionic").with_model(
                    RoutedModelInventory::new(
                        "tiny-llama",
                        "tiny-llama",
                        "llama",
                        sample_profile(),
                    )
                    .with_supported_endpoint(RoutingEndpoint::ChatCompletions),
                ),
            ],
        )
        .expect("router should build");
        let selection = router
            .resolve(&RoutingRequest::new(RoutingEndpoint::ChatCompletions))
            .expect("route should resolve");
        let mut controller = RouteReliabilityController::new(
            &router,
            RouteReliabilityPolicy {
                max_retry_attempts: 1,
                ..Default::default()
            },
        );
        controller.observe_worker_health("worker-a", HealthStatus::Offline);

        let retry = controller.admit(&selection, 0);
        assert_eq!(retry.action, ReliabilityAction::Retry);
        assert_eq!(retry.reason, Some(ReliabilityReason::WorkerOffline));

        let refuse = controller.admit(&selection, 1);
        assert_eq!(refuse.action, ReliabilityAction::Refuse);
        assert_eq!(refuse.reason, Some(ReliabilityReason::WorkerOffline));
    }

    #[test]
    fn reliability_controller_opens_circuit_after_failures() {
        let router = FleetRouter::new(
            "tiny-llama",
            vec![
                RoutedWorkerInventory::new("worker-a", "cpu", "native", "psionic").with_model(
                    RoutedModelInventory::new(
                        "tiny-llama",
                        "tiny-llama",
                        "llama",
                        sample_profile(),
                    )
                    .with_supported_endpoint(RoutingEndpoint::ChatCompletions),
                ),
            ],
        )
        .expect("router should build");
        let selection = router
            .resolve(&RoutingRequest::new(RoutingEndpoint::ChatCompletions))
            .expect("route should resolve");
        let mut controller = RouteReliabilityController::new(
            &router,
            RouteReliabilityPolicy {
                circuit_breaker_failure_threshold: 2,
                ..Default::default()
            },
        );

        assert_eq!(
            controller.admit(&selection, 0).action,
            ReliabilityAction::Execute
        );
        controller.record_failure("worker-a");
        assert_eq!(
            controller.admit(&selection, 0).action,
            ReliabilityAction::Execute
        );
        controller.record_failure("worker-a");

        let retry = controller.admit(&selection, 0);
        assert_eq!(retry.action, ReliabilityAction::Retry);
        assert_eq!(retry.reason, Some(ReliabilityReason::CircuitOpen));
        assert_eq!(
            controller.metrics()[0].circuit_state,
            WorkerCircuitState::Open
        );
    }

    #[test]
    fn reliability_controller_rate_limits_and_queues_explicitly() {
        let router = FleetRouter::new(
            "tiny-llama",
            vec![
                RoutedWorkerInventory::new("worker-a", "cpu", "native", "psionic").with_model(
                    RoutedModelInventory::new(
                        "tiny-llama",
                        "tiny-llama",
                        "llama",
                        sample_profile(),
                    )
                    .with_supported_endpoint(RoutingEndpoint::ChatCompletions),
                ),
            ],
        )
        .expect("router should build");
        let selection = router
            .resolve(&RoutingRequest::new(RoutingEndpoint::ChatCompletions))
            .expect("route should resolve");
        let mut controller = RouteReliabilityController::new(
            &router,
            RouteReliabilityPolicy {
                max_retry_attempts: 0,
                max_queue_depth: 1,
                max_requests_per_window: 1,
                ..Default::default()
            },
        );

        assert_eq!(
            controller.admit(&selection, 0).action,
            ReliabilityAction::Execute
        );
        controller.record_success("worker-a");
        let queued = controller.admit(&selection, 0);
        assert_eq!(queued.action, ReliabilityAction::Queue);
        assert_eq!(queued.reason, Some(ReliabilityReason::RateLimited));

        let refused = controller.admit(&selection, 0);
        assert_eq!(refused.action, ReliabilityAction::Refuse);
        assert_eq!(refused.reason, Some(ReliabilityReason::QueueSaturated));

        let metrics = controller.metrics();
        assert_eq!(metrics[0].queued_actions, 1);
        assert_eq!(metrics[0].rate_limited_actions, 2);
    }
}
