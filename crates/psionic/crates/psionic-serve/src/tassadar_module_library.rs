use std::{collections::BTreeMap, fs, path::PathBuf};

use psionic_runtime::{
    TASSADAR_INTERNAL_MODULE_LIBRARY_REPORT_REF, TassadarInternalModuleLibraryReport,
    TassadarInternalModuleLinkPosture,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

/// Dedicated served product identifier for the internal module library surface.
pub const EXECUTOR_MODULE_LIBRARY_PRODUCT_ID: &str = "psionic.executor_module_library";

/// Benchmark-gated served publication for the internal module library lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalModuleLibraryPublication {
    /// Served product identifier.
    pub product_id: String,
    /// Stable library identifier.
    pub library_id: String,
    /// Runtime report reference consumed by the publication.
    pub runtime_report_ref: String,
    /// Runtime report digest consumed by the publication.
    pub runtime_report_digest: String,
    /// Active module versions exported by the served surface.
    pub active_module_versions: BTreeMap<String, String>,
    /// Consumer families the served library currently supports.
    pub linked_consumer_families: Vec<String>,
    /// Stable benchmark refs gating the served publication.
    pub benchmark_refs: Vec<String>,
    /// Number of module families carrying explicit rollback readiness.
    pub rollback_ready_module_count: u32,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
}

/// Publication failure for the internal module library surface.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarInternalModuleLibraryPublicationError {
    /// The runtime report was missing benchmark lineage for one active module.
    #[error("active module `{module_id}` was missing benchmark refs")]
    MissingBenchmarkRefs { module_id: String },
}

/// Builds the benchmark-gated served publication for the internal module library lane.
pub fn build_tassadar_internal_module_library_publication()
-> Result<TassadarInternalModuleLibraryPublication, TassadarInternalModuleLibraryPublicationError> {
    let runtime_report: TassadarInternalModuleLibraryReport =
        read_repo_json(TASSADAR_INTERNAL_MODULE_LIBRARY_REPORT_REF).expect("runtime report");
    if let Some(module) = runtime_report
        .active_modules
        .iter()
        .find(|module| module.benchmark_refs.is_empty())
    {
        return Err(
            TassadarInternalModuleLibraryPublicationError::MissingBenchmarkRefs {
                module_id: module.module_id.clone(),
            },
        );
    }
    let linked_consumer_families = runtime_report
        .case_reports
        .iter()
        .filter(|case| case.link_posture != TassadarInternalModuleLinkPosture::Refused)
        .map(|case| case.consumer_program_family.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let benchmark_refs = runtime_report
        .active_modules
        .iter()
        .flat_map(|module| module.benchmark_refs.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Ok(TassadarInternalModuleLibraryPublication {
        product_id: String::from(EXECUTOR_MODULE_LIBRARY_PRODUCT_ID),
        library_id: runtime_report.library_id.clone(),
        runtime_report_ref: String::from(TASSADAR_INTERNAL_MODULE_LIBRARY_REPORT_REF),
        runtime_report_digest: runtime_report.report_digest,
        active_module_versions: runtime_report
            .active_modules
            .iter()
            .map(|module| (module.module_id.clone(), module.active_version.clone()))
            .collect(),
        linked_consumer_families,
        benchmark_refs,
        rollback_ready_module_count: runtime_report
            .active_modules
            .iter()
            .filter(|module| module.rollback_version.is_some())
            .count() as u32,
        claim_boundary: String::from(
            "this served publication is benchmark-gated by the internal module runtime report and keeps active versions plus rollback-ready families explicit. It does not claim arbitrary module installation, unrestricted self-extension, or later install governance closure",
        ),
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .map(std::path::Path::to_path_buf)
        .expect("repo root should resolve from psionic-serve crate dir")
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, Box<dyn std::error::Error>> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::{
        EXECUTOR_MODULE_LIBRARY_PRODUCT_ID, build_tassadar_internal_module_library_publication,
    };

    #[test]
    fn internal_module_library_publication_is_benchmark_gated() {
        let publication =
            build_tassadar_internal_module_library_publication().expect("publication");

        assert_eq!(publication.product_id, EXECUTOR_MODULE_LIBRARY_PRODUCT_ID);
        assert_eq!(publication.rollback_ready_module_count, 1);
        assert!(
            publication
                .active_module_versions
                .contains_key("frontier_relax_core")
        );
        assert!(
            publication
                .linked_consumer_families
                .contains(&String::from("verifier_search"))
        );
        assert!(!publication.benchmark_refs.is_empty());
    }
}
