use psionic_compiler::build_tassadar_module_catalog_report;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Dedicated served product identifier for the bounded module catalog surface.
pub const EXECUTOR_MODULE_CATALOG_PRODUCT_ID: &str = "psionic.executor_module_catalog";

/// Benchmark-gated served publication for the bounded module catalog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleCatalogPublication {
    /// Served product identifier.
    pub product_id: String,
    /// Compiler report ref backing the publication.
    pub report_ref: String,
    /// Compiler report digest backing the publication.
    pub report_digest: String,
    /// Number of served catalog entries.
    pub entry_count: u32,
    /// Maximum reuse rate across the served entries.
    pub max_reuse_rate_bps: u16,
    /// Maximum held-out-program lift across the served entries.
    pub max_held_out_program_lift_bps: i16,
    /// Stable benchmark refs gating the publication.
    pub benchmark_refs: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
}

/// Publication failure for the bounded module catalog.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarModuleCatalogPublicationError {
    /// One catalog entry was missing benchmark refs.
    #[error("catalog entry `{entry_id}` was missing benchmark refs")]
    MissingBenchmarkRefs { entry_id: String },
}

/// Builds the benchmark-gated served publication for the bounded module catalog.
pub fn build_tassadar_module_catalog_publication()
-> Result<TassadarModuleCatalogPublication, TassadarModuleCatalogPublicationError> {
    let report = build_tassadar_module_catalog_report();
    if let Some(entry) = report
        .entries
        .iter()
        .find(|entry| entry.benchmark_refs.is_empty())
    {
        return Err(
            TassadarModuleCatalogPublicationError::MissingBenchmarkRefs {
                entry_id: entry.entry_id.clone(),
            },
        );
    }
    let mut benchmark_refs = report
        .entries
        .iter()
        .flat_map(|entry| entry.benchmark_refs.iter().cloned())
        .collect::<Vec<_>>();
    benchmark_refs.sort();
    benchmark_refs.dedup();
    Ok(TassadarModuleCatalogPublication {
        product_id: String::from(EXECUTOR_MODULE_CATALOG_PRODUCT_ID),
        report_ref: String::from("fixtures/tassadar/reports/tassadar_module_catalog_report.json"),
        report_digest: report.report_digest,
        entry_count: report.entries.len() as u32,
        max_reuse_rate_bps: report
            .entries
            .iter()
            .map(|entry| entry.reuse_rate_bps)
            .max()
            .unwrap_or(0),
        max_held_out_program_lift_bps: report
            .entries
            .iter()
            .map(|entry| entry.held_out_program_lift_bps)
            .max()
            .unwrap_or(0),
        benchmark_refs,
        claim_boundary: String::from(
            "this served publication is benchmark-gated by the bounded compiler-owned module catalog report and keeps reuse-rate plus held-out-program lift explicit. It does not widen served capability by implication from catalog membership alone",
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::{EXECUTOR_MODULE_CATALOG_PRODUCT_ID, build_tassadar_module_catalog_publication};

    #[test]
    fn module_catalog_publication_is_benchmark_gated() {
        let publication = build_tassadar_module_catalog_publication().expect("publication");

        assert_eq!(publication.product_id, EXECUTOR_MODULE_CATALOG_PRODUCT_ID);
        assert_eq!(publication.entry_count, 3);
        assert_eq!(publication.max_reuse_rate_bps, 10000);
        assert_eq!(publication.max_held_out_program_lift_bps, 2400);
        assert!(!publication.benchmark_refs.is_empty());
    }
}
