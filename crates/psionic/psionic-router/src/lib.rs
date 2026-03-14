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
use std::collections::BTreeMap;
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

/// One model-route request evaluated against router inventory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingRequest {
    /// API endpoint the caller needs.
    pub endpoint: RoutingEndpoint,
    /// Requested target model posture.
    pub target: RoutingTarget,
    /// Required capabilities.
    pub capability_filters: RoutingCapabilityFilters,
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
    /// Plain-language route notes explaining tie-breaks and filters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routing_notes: Vec<String>,
}

#[derive(Clone, Debug)]
struct RouteBinding {
    worker_id: String,
    model_key: String,
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
            eligible.push((
                preference_rank,
                worker.worker_id.clone(),
                model.canonical_name.clone(),
                self.selection_for(worker, model, request.endpoint, &request.target),
            ));
        }

        eligible.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
        });
        let Some((preference_rank, _, _, mut selection)) = eligible.into_iter().next() else {
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

        if preference_rank != usize::MAX {
            selection.routing_notes.push(format!(
                "selected preferred worker `{}` for routed request",
                selection.worker_id
            ));
        }
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
            routing_notes,
        }
    }
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
        FleetRouter, RoutedModelInventory, RoutedWorkerInventory, RoutingEndpoint, RoutingError,
        RoutingRequest,
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
}
