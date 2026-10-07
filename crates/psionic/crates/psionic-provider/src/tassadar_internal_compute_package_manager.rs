use serde::{Deserialize, Serialize};

use psionic_serve::TassadarInternalComputePackageManagerPublication;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComputePackageManagerReceipt {
    pub product_id: String,
    pub public_package_count: u32,
    pub default_served_package_count: u32,
    pub routeable_package_count: u32,
    pub refused_package_case_count: u32,
    pub benchmark_ref_count: u32,
    pub detail: String,
}

impl TassadarInternalComputePackageManagerReceipt {
    #[must_use]
    pub fn from_publication(publication: &TassadarInternalComputePackageManagerPublication) -> Self {
        Self {
            product_id: publication.product_id.clone(),
            public_package_count: publication.public_package_ids.len() as u32,
            default_served_package_count: publication.default_served_package_ids.len() as u32,
            routeable_package_count: publication.routeable_package_ids.len() as u32,
            refused_package_case_count: publication.refused_package_case_ids.len() as u32,
            benchmark_ref_count: publication.benchmark_ref_count,
            detail: format!(
                "internal compute package publication exposes {} public packages, {} default-served packages, {} routeable packages, and {} refused package cases",
                publication.public_package_ids.len(),
                publication.default_served_package_ids.len(),
                publication.routeable_package_ids.len(),
                publication.refused_package_case_ids.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarInternalComputePackageManagerReceipt;
    use psionic_serve::build_tassadar_internal_compute_package_manager_publication;

    #[test]
    fn internal_compute_package_manager_receipt_projects_publication() {
        let publication =
            build_tassadar_internal_compute_package_manager_publication().expect("publication");
        let receipt = TassadarInternalComputePackageManagerReceipt::from_publication(&publication);

        assert_eq!(receipt.public_package_count, 3);
        assert_eq!(receipt.default_served_package_count, 0);
        assert_eq!(receipt.routeable_package_count, 3);
        assert_eq!(receipt.refused_package_case_count, 3);
        assert!(receipt.benchmark_ref_count >= 3);
    }
}
