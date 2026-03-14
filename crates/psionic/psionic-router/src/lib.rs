//! Multi-model routing and worker-policy contracts for Psionic.
//!
//! This crate owns reusable fleet-routing truth for served Psionic workers.
//! It does not own request execution, app UX, procurement, settlement, or
//! long-term storage. Those concerns stay in `psionic-serve`, app code, and
//! kernel or Nexus services.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

use psionic_runtime::{ExecutionCapabilityProfile, GenerationSchedulerPolicy};
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
            reusable_tokens,
        }
    }

    /// Pins the cache entry to one topology or route scope.
    #[must_use]
    pub fn with_topology_scope(mut self, topology_scope: impl Into<String>) -> Self {
        self.topology_scope = Some(topology_scope.into());
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
    /// Required capabilities.
    pub capability_filters: RoutingCapabilityFilters,
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
            capability_filters: RoutingCapabilityFilters::default(),
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
    /// Whether structured outputs are supported.
    pub structured_outputs: bool,
    /// Whether tool calling is supported.
    pub tool_calling: bool,
    /// Whether response-state flows are supported.
    pub response_state: bool,
    /// Live runtime facts that cache-aware and warm-aware policy can consume.
    pub runtime_state: RoutedModelRuntimeState,
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
            structured_outputs: false,
            tool_calling: false,
            response_state: false,
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
    /// Worker backend label.
    pub backend_label: String,
    /// Worker execution-mode label.
    pub execution_mode_label: String,
    /// Worker execution-engine label.
    pub execution_engine_label: String,
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
            backend_label: backend_label.into(),
            execution_mode_label: execution_mode_label.into(),
            execution_engine_label: execution_engine_label.into(),
            models: Vec::new(),
        }
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
    /// Routed execution profile.
    pub execution_profile: ExecutionCapabilityProfile,
    /// Routed scheduler policy when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_policy: Option<GenerationSchedulerPolicy>,
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

        let mut fallback_reason = None;
        let mut pool_reason = "eligible";
        let cache_pool = eligible
            .iter()
            .filter(|candidate| candidate.cache_match_tokens > 0)
            .cloned()
            .collect::<Vec<_>>();
        let warm_pool = eligible
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
                eligible.clone()
            }
        } else if request.policy_hints.cache_key.is_some() {
            fallback_reason = Some(String::from(
                "no safe cache-compatible worker route was available, so cache-aware placement fell back",
            ));
            if !warm_pool.is_empty() {
                pool_reason = "warm";
                warm_pool
            } else {
                eligible.clone()
            }
        } else if !warm_pool.is_empty() {
            pool_reason = "warm";
            warm_pool
        } else {
            fallback_reason = Some(String::from(
                "no warm or cache-compatible worker route was available, so placement fell back to first-ready",
            ));
            eligible.clone()
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
            model_key: model.model_key.clone(),
            canonical_name: model.canonical_name.clone(),
            endpoint,
            family: model.family.clone(),
            backend_label: worker.backend_label.clone(),
            execution_mode_label: worker.execution_mode_label.clone(),
            execution_engine_label: worker.execution_engine_label.clone(),
            execution_profile: model.execution_profile.clone(),
            scheduler_policy: model.scheduler_policy.clone(),
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
    request.policy_hints.cache_key.hash(&mut hasher);
    request.policy_hints.tenant_scope.hash(&mut hasher);
    request.policy_hints.topology_scope.hash(&mut hasher);
    request.policy_hints.request_key.hash(&mut hasher);
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

#[cfg(test)]
mod tests {
    use super::{
        FleetRouter, RouteSelectionStrategy, RoutedCacheEntry, RoutedModelInventory,
        RoutedWarmState, RoutedWorkerInventory, RoutingEndpoint, RoutingError, RoutingRequest,
    };
    use psionic_runtime::{ExecutionCapabilityProfile, PrefillDecodeCapability};

    fn sample_profile() -> ExecutionCapabilityProfile {
        ExecutionCapabilityProfile::single_request_latency_optimized()
            .with_prefill_decode_capability(PrefillDecodeCapability::colocated_split())
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
}
