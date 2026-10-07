use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Stable routed product identifier for bounded module installation.
pub const TASSADAR_MODULE_INSTALL_ROUTE_PRODUCT_ID: &str = "psionic.executor_module_install";

/// Install scope used by route negotiation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleInstallScope {
    SessionMount,
    WorkerMount,
}

/// Typed refusal reason surfaced during module-install route negotiation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleInstallRouteRefusalReason {
    ProviderNotReady,
    ScopeUnsupported,
    UnsafeModuleClass,
    ChallengeTicketMissing,
    BenchmarkEvidenceMissing,
}

/// Route descriptor exported for module-install negotiation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleInstallRouteDescriptor {
    /// Stable route identifier.
    pub route_id: String,
    /// Routed product identifier.
    pub product_id: String,
    /// Supported install scopes.
    pub supported_scopes: Vec<TassadarModuleInstallScope>,
    /// Trusted module classes published by the route.
    pub trusted_module_classes: Vec<String>,
    /// Stable benchmark refs gating the route.
    pub benchmark_refs: Vec<String>,
    /// Plain-language route note.
    pub note: String,
    /// Stable digest over the route descriptor.
    pub descriptor_digest: String,
}

impl TassadarModuleInstallRouteDescriptor {
    /// Creates one route descriptor.
    #[must_use]
    pub fn new(
        route_id: impl Into<String>,
        mut supported_scopes: Vec<TassadarModuleInstallScope>,
        mut trusted_module_classes: Vec<String>,
        mut benchmark_refs: Vec<String>,
        note: impl Into<String>,
    ) -> Self {
        supported_scopes.sort();
        supported_scopes.dedup();
        trusted_module_classes.sort();
        trusted_module_classes.dedup();
        benchmark_refs.sort();
        benchmark_refs.dedup();
        let mut descriptor = Self {
            route_id: route_id.into(),
            product_id: String::from(TASSADAR_MODULE_INSTALL_ROUTE_PRODUCT_ID),
            supported_scopes,
            trusted_module_classes,
            benchmark_refs,
            note: note.into(),
            descriptor_digest: String::new(),
        };
        descriptor.descriptor_digest =
            stable_digest(b"psionic_tassadar_module_install_route_descriptor|", &descriptor);
        descriptor
    }
}

/// One provider-side route candidate for module installation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleInstallRouteCandidate {
    /// Stable provider identifier.
    pub provider_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Whether the provider is currently ready for install routing.
    pub ready: bool,
    /// Exported route descriptor.
    pub route_descriptor: TassadarModuleInstallRouteDescriptor,
}

impl TassadarModuleInstallRouteCandidate {
    /// Creates one route candidate.
    #[must_use]
    pub fn new(
        provider_id: impl Into<String>,
        worker_id: impl Into<String>,
        ready: bool,
        route_descriptor: TassadarModuleInstallRouteDescriptor,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            worker_id: worker_id.into(),
            ready,
            route_descriptor,
        }
    }
}

/// Request evaluated against module-install route candidates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleInstallRoutingRequest {
    /// Stable install identifier.
    pub install_id: String,
    /// Stable module identifier.
    pub module_id: String,
    /// Stable module class.
    pub module_class: String,
    /// Requested install scope.
    pub scope: TassadarModuleInstallScope,
    /// Whether challenge acknowledgement is attached.
    pub challenge_ticket_acknowledged: bool,
    /// Whether benchmark evidence is attached to the request.
    pub benchmark_evidence_present: bool,
}

/// Selected route for one bounded module install.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleInstallRouteSelection {
    /// Selected provider identifier.
    pub provider_id: String,
    /// Selected worker identifier.
    pub worker_id: String,
    /// Route descriptor used for the install.
    pub route_descriptor: TassadarModuleInstallRouteDescriptor,
}

/// Negotiates one bounded module-install route.
pub fn negotiate_tassadar_module_install_route(
    request: &TassadarModuleInstallRoutingRequest,
    candidates: &[TassadarModuleInstallRouteCandidate],
) -> Result<TassadarModuleInstallRouteSelection, TassadarModuleInstallRouteRefusalReason> {
    if !request.benchmark_evidence_present {
        return Err(TassadarModuleInstallRouteRefusalReason::BenchmarkEvidenceMissing);
    }
    if request.scope == TassadarModuleInstallScope::WorkerMount
        && !request.challenge_ticket_acknowledged
    {
        return Err(TassadarModuleInstallRouteRefusalReason::ChallengeTicketMissing);
    }
    let candidate = candidates
        .iter()
        .find(|candidate| candidate.ready)
        .ok_or(TassadarModuleInstallRouteRefusalReason::ProviderNotReady)?;
    if !candidate
        .route_descriptor
        .supported_scopes
        .contains(&request.scope)
    {
        return Err(TassadarModuleInstallRouteRefusalReason::ScopeUnsupported);
    }
    if !candidate
        .route_descriptor
        .trusted_module_classes
        .contains(&request.module_class)
    {
        return Err(TassadarModuleInstallRouteRefusalReason::UnsafeModuleClass);
    }
    Ok(TassadarModuleInstallRouteSelection {
        provider_id: candidate.provider_id.clone(),
        worker_id: candidate.worker_id.clone(),
        route_descriptor: candidate.route_descriptor.clone(),
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        negotiate_tassadar_module_install_route, TassadarModuleInstallRouteCandidate,
        TassadarModuleInstallRouteDescriptor, TassadarModuleInstallRouteRefusalReason,
        TassadarModuleInstallRoutingRequest, TassadarModuleInstallScope,
    };

    #[test]
    fn module_install_route_negotiation_keeps_challenge_and_policy_refusals_explicit() {
        let descriptor = TassadarModuleInstallRouteDescriptor::new(
            "install-route-a",
            vec![
                TassadarModuleInstallScope::SessionMount,
                TassadarModuleInstallScope::WorkerMount,
            ],
            vec![
                String::from("frontier_relax_core"),
                String::from("candidate_select_core"),
            ],
            vec![String::from(
                "fixtures/tassadar/reports/tassadar_internal_module_library_report.json",
            )],
            "bounded install route",
        );
        let candidates = vec![TassadarModuleInstallRouteCandidate::new(
            "provider-a",
            "worker-a",
            true,
            descriptor,
        )];
        let refused = TassadarModuleInstallRoutingRequest {
            install_id: String::from("install-1"),
            module_id: String::from("candidate_select_core"),
            module_class: String::from("candidate_select_core"),
            scope: TassadarModuleInstallScope::WorkerMount,
            challenge_ticket_acknowledged: false,
            benchmark_evidence_present: true,
        };
        assert_eq!(
            negotiate_tassadar_module_install_route(&refused, &candidates),
            Err(TassadarModuleInstallRouteRefusalReason::ChallengeTicketMissing)
        );
        let selected = TassadarModuleInstallRoutingRequest {
            install_id: String::from("install-2"),
            module_id: String::from("frontier_relax_core"),
            module_class: String::from("frontier_relax_core"),
            scope: TassadarModuleInstallScope::SessionMount,
            challenge_ticket_acknowledged: false,
            benchmark_evidence_present: true,
        };
        let selection =
            negotiate_tassadar_module_install_route(&selected, &candidates).expect("selection");
        assert_eq!(selection.provider_id, "provider-a");
    }
}
