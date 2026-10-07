use serde::{Deserialize, Serialize};
use thiserror::Error;

use psionic_eval::build_tassadar_installed_process_lifecycle_report;
use psionic_runtime::TASSADAR_INSTALLED_PROCESS_LIFECYCLE_PROFILE_ID;

pub const INSTALLED_PROCESS_LIFECYCLE_PRODUCT_ID: &str = "psionic.installed_process_lifecycle";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledProcessLifecyclePublication {
    pub product_id: String,
    pub eval_report_ref: String,
    pub eval_report_digest: String,
    pub profile_id: String,
    pub portable_process_ids: Vec<String>,
    pub exact_migration_case_count: u32,
    pub exact_rollback_case_count: u32,
    pub refusal_case_count: u32,
    pub portability_envelope_ids: Vec<String>,
    pub served_publication_allowed: bool,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarInstalledProcessLifecyclePublicationError {
    #[error("installed-process lifecycle report was not green")]
    EvalReportNotGreen,
    #[error("installed-process lifecycle portability surface was empty")]
    MissingPortableProcesses,
}

pub fn build_tassadar_installed_process_lifecycle_publication() -> Result<
    TassadarInstalledProcessLifecyclePublication,
    TassadarInstalledProcessLifecyclePublicationError,
> {
    let report = build_tassadar_installed_process_lifecycle_report()
        .map_err(|_| TassadarInstalledProcessLifecyclePublicationError::EvalReportNotGreen)?;
    if !report.overall_green {
        return Err(TassadarInstalledProcessLifecyclePublicationError::EvalReportNotGreen);
    }
    if report.portable_process_ids.is_empty() {
        return Err(TassadarInstalledProcessLifecyclePublicationError::MissingPortableProcesses);
    }
    Ok(TassadarInstalledProcessLifecyclePublication {
        product_id: String::from(INSTALLED_PROCESS_LIFECYCLE_PRODUCT_ID),
        eval_report_ref: String::from(
            "fixtures/tassadar/reports/tassadar_installed_process_lifecycle_report.json",
        ),
        eval_report_digest: report.report_digest,
        profile_id: String::from(TASSADAR_INSTALLED_PROCESS_LIFECYCLE_PROFILE_ID),
        portable_process_ids: report.portable_process_ids,
        exact_migration_case_count: report.exact_migration_case_count,
        exact_rollback_case_count: report.exact_rollback_case_count,
        refusal_case_count: report.refusal_case_count,
        portability_envelope_ids: report.portability_envelope_ids,
        served_publication_allowed: report.served_publication_allowed,
        claim_boundary: String::from(
            "this operator-facing publication keeps portable installed-process lifecycle truth explicit while retaining served_publication_allowed = false. It does not imply arbitrary cluster migration, arbitrary rollback support, or broad served internal-compute publication",
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        INSTALLED_PROCESS_LIFECYCLE_PRODUCT_ID,
        build_tassadar_installed_process_lifecycle_publication,
    };

    #[test]
    fn installed_process_lifecycle_publication_is_green_but_operator_only() {
        let publication =
            build_tassadar_installed_process_lifecycle_publication().expect("publication");

        assert_eq!(
            publication.product_id,
            INSTALLED_PROCESS_LIFECYCLE_PRODUCT_ID
        );
        assert_eq!(publication.exact_migration_case_count, 1);
        assert_eq!(publication.exact_rollback_case_count, 1);
        assert_eq!(publication.refusal_case_count, 3);
        assert_eq!(publication.portable_process_ids.len(), 2);
        assert!(!publication.served_publication_allowed);
    }
}
