use serde::{Deserialize, Serialize};

use psionic_serve::TassadarInternalModuleLibraryPublication;

/// Provider-facing receipt for the benchmark-gated internal module library surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalModuleLibraryReceipt {
    /// Stable library identifier.
    pub library_id: String,
    /// Served product identifier.
    pub product_id: String,
    /// Number of active module families published provider-side.
    pub active_module_count: u32,
    /// Number of linked consumer families published provider-side.
    pub linked_consumer_family_count: u32,
    /// Number of rollback-ready module families.
    pub rollback_ready_module_count: u32,
    /// Count of benchmark refs backing the receipt.
    pub benchmark_ref_count: u32,
    /// Plain-language receipt detail.
    pub detail: String,
}

impl TassadarInternalModuleLibraryReceipt {
    /// Builds a provider-facing receipt from the served library publication.
    #[must_use]
    pub fn from_publication(publication: &TassadarInternalModuleLibraryPublication) -> Self {
        Self {
            library_id: publication.library_id.clone(),
            product_id: publication.product_id.clone(),
            active_module_count: publication.active_module_versions.len() as u32,
            linked_consumer_family_count: publication.linked_consumer_families.len() as u32,
            rollback_ready_module_count: publication.rollback_ready_module_count,
            benchmark_ref_count: publication.benchmark_refs.len() as u32,
            detail: format!(
                "internal module library `{}` currently exports {} active modules across {} linked consumer families, with {} rollback-ready families kept explicit",
                publication.library_id,
                publication.active_module_versions.len(),
                publication.linked_consumer_families.len(),
                publication.rollback_ready_module_count,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarInternalModuleLibraryReceipt;
    use psionic_serve::build_tassadar_internal_module_library_publication;

    #[test]
    fn internal_module_library_receipt_projects_served_publication() {
        let publication =
            build_tassadar_internal_module_library_publication().expect("publication");
        let receipt = TassadarInternalModuleLibraryReceipt::from_publication(&publication);

        assert_eq!(receipt.active_module_count, 3);
        assert_eq!(receipt.rollback_ready_module_count, 1);
        assert!(receipt.linked_consumer_family_count >= 3);
        assert!(receipt.benchmark_ref_count >= 2);
    }
}
