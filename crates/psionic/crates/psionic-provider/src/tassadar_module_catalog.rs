use serde::{Deserialize, Serialize};

use psionic_serve::TassadarModuleCatalogPublication;

/// Provider-facing receipt for the served bounded module catalog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleCatalogReceipt {
    /// Served product identifier.
    pub product_id: String,
    /// Number of published catalog entries.
    pub entry_count: u32,
    /// Maximum reuse rate across published entries.
    pub max_reuse_rate_bps: u16,
    /// Maximum held-out-program lift across published entries.
    pub max_held_out_program_lift_bps: i16,
    /// Count of benchmark refs gating the receipt.
    pub benchmark_ref_count: u32,
    /// Plain-language detail.
    pub detail: String,
}

impl TassadarModuleCatalogReceipt {
    /// Builds a provider-facing receipt from the served catalog publication.
    #[must_use]
    pub fn from_publication(publication: &TassadarModuleCatalogPublication) -> Self {
        Self {
            product_id: publication.product_id.clone(),
            entry_count: publication.entry_count,
            max_reuse_rate_bps: publication.max_reuse_rate_bps,
            max_held_out_program_lift_bps: publication.max_held_out_program_lift_bps,
            benchmark_ref_count: publication.benchmark_refs.len() as u32,
            detail: format!(
                "module catalog publication currently exposes {} entries with max reuse_rate={}bps and max held_out_program_lift={}bps",
                publication.entry_count,
                publication.max_reuse_rate_bps,
                publication.max_held_out_program_lift_bps,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarModuleCatalogReceipt;
    use psionic_serve::build_tassadar_module_catalog_publication;

    #[test]
    fn module_catalog_receipt_projects_served_publication() {
        let publication = build_tassadar_module_catalog_publication().expect("publication");
        let receipt = TassadarModuleCatalogReceipt::from_publication(&publication);

        assert_eq!(receipt.entry_count, 3);
        assert_eq!(receipt.max_reuse_rate_bps, 10000);
        assert_eq!(receipt.max_held_out_program_lift_bps, 2400);
        assert!(receipt.benchmark_ref_count >= 3);
    }
}
