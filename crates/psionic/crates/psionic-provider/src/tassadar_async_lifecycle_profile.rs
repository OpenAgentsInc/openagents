use serde::{Deserialize, Serialize};

use psionic_eval::TassadarAsyncLifecycleProfileReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAsyncLifecycleProfileReceipt {
    pub report_id: String,
    pub profile_id: String,
    pub public_profile_allowed_profile_ids: Vec<String>,
    pub default_served_profile_allowed_profile_ids: Vec<String>,
    pub routeable_lifecycle_surface_ids: Vec<String>,
    pub refused_lifecycle_surface_ids: Vec<String>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub overall_green: bool,
    pub detail: String,
}

impl TassadarAsyncLifecycleProfileReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarAsyncLifecycleProfileReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            profile_id: report.profile_id.clone(),
            public_profile_allowed_profile_ids: report.public_profile_allowed_profile_ids.clone(),
            default_served_profile_allowed_profile_ids: report
                .default_served_profile_allowed_profile_ids
                .clone(),
            routeable_lifecycle_surface_ids: report.routeable_lifecycle_surface_ids.clone(),
            refused_lifecycle_surface_ids: report.refused_lifecycle_surface_ids.clone(),
            exact_case_count: report.exact_case_count,
            refusal_case_count: report.refusal_case_count,
            overall_green: report.overall_green,
            detail: format!(
                "async-lifecycle profile report `{}` carries public_profiles={}, default_served_profiles={}, routeable_surfaces={}, refused_surfaces={}, exact_cases={}, refusal_cases={}, overall_green={}",
                report.report_id,
                report.public_profile_allowed_profile_ids.len(),
                report.default_served_profile_allowed_profile_ids.len(),
                report.routeable_lifecycle_surface_ids.len(),
                report.refused_lifecycle_surface_ids.len(),
                report.exact_case_count,
                report.refusal_case_count,
                report.overall_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarAsyncLifecycleProfileReceipt;
    use psionic_eval::build_tassadar_async_lifecycle_profile_report;

    #[test]
    fn async_lifecycle_profile_receipt_projects_report() {
        let report = build_tassadar_async_lifecycle_profile_report().expect("report");
        let receipt = TassadarAsyncLifecycleProfileReceipt::from_report(&report);

        assert!(receipt.overall_green);
        assert_eq!(
            receipt.profile_id,
            "tassadar.internal_compute.async_lifecycle.v1"
        );
        assert_eq!(
            receipt.public_profile_allowed_profile_ids,
            vec![String::from("tassadar.internal_compute.async_lifecycle.v1")]
        );
        assert!(receipt.default_served_profile_allowed_profile_ids.is_empty());
        assert_eq!(receipt.routeable_lifecycle_surface_ids.len(), 3);
        assert_eq!(receipt.refused_lifecycle_surface_ids.len(), 3);
        assert_eq!(receipt.exact_case_count, 3);
        assert_eq!(receipt.refusal_case_count, 3);
    }
}
