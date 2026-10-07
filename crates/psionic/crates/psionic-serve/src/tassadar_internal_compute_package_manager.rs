use serde::{Deserialize, Serialize};
use thiserror::Error;

use psionic_eval::build_tassadar_internal_compute_package_manager_eval_report;

pub const INTERNAL_COMPUTE_PACKAGE_MANAGER_PRODUCT_ID: &str =
    "psionic.internal_compute_package_manager";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComputePackageManagerPublication {
    pub product_id: String,
    pub eval_report_ref: String,
    pub eval_report_digest: String,
    pub public_package_ids: Vec<String>,
    pub default_served_package_ids: Vec<String>,
    pub routeable_package_ids: Vec<String>,
    pub refused_package_case_ids: Vec<String>,
    pub benchmark_ref_count: u32,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarInternalComputePackageManagerPublicationError {
    #[error("internal compute package manager eval report was not green")]
    EvalReportNotGreen,
    #[error("internal compute package manager had no public packages")]
    MissingPublicPackages,
    #[error("internal compute package manager widened the default served lane")]
    DefaultServedPackagesMustStayEmpty,
}

pub fn build_tassadar_internal_compute_package_manager_publication(
) -> Result<TassadarInternalComputePackageManagerPublication, TassadarInternalComputePackageManagerPublicationError>
{
    let report = build_tassadar_internal_compute_package_manager_eval_report()
        .map_err(|_| TassadarInternalComputePackageManagerPublicationError::EvalReportNotGreen)?;
    if !report.overall_green || !report.served_publication_allowed {
        return Err(TassadarInternalComputePackageManagerPublicationError::EvalReportNotGreen);
    }
    if report.public_package_ids.is_empty() {
        return Err(TassadarInternalComputePackageManagerPublicationError::MissingPublicPackages);
    }
    if !report.default_served_package_ids.is_empty() {
        return Err(
            TassadarInternalComputePackageManagerPublicationError::DefaultServedPackagesMustStayEmpty,
        );
    }
    Ok(TassadarInternalComputePackageManagerPublication {
        product_id: String::from(INTERNAL_COMPUTE_PACKAGE_MANAGER_PRODUCT_ID),
        eval_report_ref: String::from(
            "fixtures/tassadar/reports/tassadar_internal_compute_package_manager_eval_report.json",
        ),
        eval_report_digest: report.report_digest,
        public_package_ids: report.public_package_ids,
        default_served_package_ids: report.default_served_package_ids,
        routeable_package_ids: report.routeable_package_ids,
        refused_package_case_ids: report.refused_package_case_ids,
        benchmark_ref_count: report.benchmark_ref_count,
        claim_boundary: String::from(
            "this served publication keeps internal-compute packages named-public and routeable in bounded form only, with zero default-served packages. It does not imply arbitrary package discovery, arbitrary dependency solving, or broad internal-compute publication",
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        INTERNAL_COMPUTE_PACKAGE_MANAGER_PRODUCT_ID,
        build_tassadar_internal_compute_package_manager_publication,
    };

    #[test]
    fn internal_compute_package_manager_publication_is_benchmark_gated() {
        let publication =
            build_tassadar_internal_compute_package_manager_publication().expect("publication");

        assert_eq!(
            publication.product_id,
            INTERNAL_COMPUTE_PACKAGE_MANAGER_PRODUCT_ID
        );
        assert_eq!(publication.public_package_ids.len(), 3);
        assert!(publication.default_served_package_ids.is_empty());
        assert_eq!(publication.routeable_package_ids.len(), 3);
        assert_eq!(publication.refused_package_case_ids.len(), 3);
    }
}
