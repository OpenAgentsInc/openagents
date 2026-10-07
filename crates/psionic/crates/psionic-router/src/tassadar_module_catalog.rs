use serde::{Deserialize, Serialize};

use psionic_compiler::{
    TassadarModuleCatalogLookupError, TassadarModuleCatalogLookupRequest,
    build_tassadar_module_catalog_report, lookup_tassadar_module_catalog_entry,
};
use psionic_ir::TassadarModuleTrustPosture;

/// Typed refusal reason for one module-catalog route lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleCatalogRouteRefusalReason {
    NoCatalogMatch,
    AmbiguousCatalogMatch,
    InsufficientEvidence,
}

/// Route selection produced from the bounded module catalog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleCatalogRouteSelection {
    /// Selected module ref.
    pub module_ref: String,
    /// Stable capability label satisfied by the selection.
    pub capability_label: String,
    /// Stable workload family satisfied by the selection.
    pub workload_family: String,
    /// Reuse rate attached to the selected entry.
    pub reuse_rate_bps: u16,
    /// Held-out-program lift attached to the selected entry.
    pub held_out_program_lift_bps: i16,
    /// Stable compiler catalog digest.
    pub catalog_digest: String,
}

/// Resolves one bounded router lookup against the compiler-owned module catalog.
pub fn resolve_tassadar_module_catalog_route(
    capability_label: &str,
    workload_family: &str,
    minimum_trust_posture: TassadarModuleTrustPosture,
    minimum_benchmark_ref_count: u32,
) -> Result<TassadarModuleCatalogRouteSelection, TassadarModuleCatalogRouteRefusalReason> {
    let report = build_tassadar_module_catalog_report();
    let request = TassadarModuleCatalogLookupRequest {
        capability_label: String::from(capability_label),
        workload_family: String::from(workload_family),
        minimum_trust_posture,
        minimum_benchmark_ref_count,
    };
    let entry =
        lookup_tassadar_module_catalog_entry(&report, &request).map_err(|error| match error {
            TassadarModuleCatalogLookupError::NoCatalogMatch { .. } => {
                TassadarModuleCatalogRouteRefusalReason::NoCatalogMatch
            }
            TassadarModuleCatalogLookupError::AmbiguousCatalogMatch { .. } => {
                TassadarModuleCatalogRouteRefusalReason::AmbiguousCatalogMatch
            }
            TassadarModuleCatalogLookupError::InsufficientEvidence { .. } => {
                TassadarModuleCatalogRouteRefusalReason::InsufficientEvidence
            }
        })?;
    Ok(TassadarModuleCatalogRouteSelection {
        module_ref: entry.module_ref,
        capability_label: String::from(capability_label),
        workload_family: String::from(workload_family),
        reuse_rate_bps: entry.reuse_rate_bps,
        held_out_program_lift_bps: entry.held_out_program_lift_bps,
        catalog_digest: report.report_digest,
    })
}

#[cfg(test)]
mod tests {
    use super::{TassadarModuleCatalogRouteRefusalReason, resolve_tassadar_module_catalog_route};
    use psionic_ir::TassadarModuleTrustPosture;

    #[test]
    fn module_catalog_route_resolves_frontier_relax_core() {
        let selection = resolve_tassadar_module_catalog_route(
            "frontier_relaxation",
            "clrs_shortest_path",
            TassadarModuleTrustPosture::BenchmarkGatedInternal,
            2,
        )
        .expect("selection");

        assert_eq!(selection.module_ref, "frontier_relax_core@1.0.0");
        assert_eq!(selection.reuse_rate_bps, 10000);
    }

    #[test]
    fn module_catalog_route_refuses_ambiguous_bounded_search() {
        let error = resolve_tassadar_module_catalog_route(
            "bounded_search",
            "verifier_search",
            TassadarModuleTrustPosture::BenchmarkGatedInternal,
            1,
        )
        .expect_err("error");

        assert_eq!(
            error,
            TassadarModuleCatalogRouteRefusalReason::AmbiguousCatalogMatch
        );
    }
}
