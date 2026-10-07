use serde::{Deserialize, Serialize};

use psionic_eval::TassadarSimdProfileReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSimdProfileReceipt {
    pub report_id: String,
    pub public_profile_allowed_profile_ids: Vec<String>,
    pub default_served_profile_allowed_profile_ids: Vec<String>,
    pub exact_backend_ids: Vec<String>,
    pub fallback_backend_ids: Vec<String>,
    pub refused_backend_ids: Vec<String>,
    pub overall_green: bool,
    pub detail: String,
}

impl TassadarSimdProfileReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarSimdProfileReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            public_profile_allowed_profile_ids: report.public_profile_allowed_profile_ids.clone(),
            default_served_profile_allowed_profile_ids: report
                .default_served_profile_allowed_profile_ids
                .clone(),
            exact_backend_ids: report.exact_backend_ids.clone(),
            fallback_backend_ids: report.fallback_backend_ids.clone(),
            refused_backend_ids: report.refused_backend_ids.clone(),
            overall_green: report.overall_green,
            detail: format!(
                "simd profile report `{}` keeps public_profiles={}, default_served_profiles={}, exact_backends={}, fallback_backends={}, refused_backends={}, overall_green={}",
                report.report_id,
                report.public_profile_allowed_profile_ids.len(),
                report.default_served_profile_allowed_profile_ids.len(),
                report.exact_backend_ids.len(),
                report.fallback_backend_ids.len(),
                report.refused_backend_ids.len(),
                report.overall_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarSimdProfileReceipt;
    use psionic_eval::build_tassadar_simd_profile_report;

    #[test]
    fn simd_profile_receipt_projects_report() {
        let report = build_tassadar_simd_profile_report().expect("report");
        let receipt = TassadarSimdProfileReceipt::from_report(&report);

        assert!(receipt.overall_green);
        assert_eq!(
            receipt.public_profile_allowed_profile_ids,
            vec![String::from("tassadar.proposal_profile.simd_deterministic.v1")]
        );
        assert!(receipt
            .exact_backend_ids
            .contains(&String::from("cpu_reference_current_host")));
        assert!(receipt
            .fallback_backend_ids
            .contains(&String::from("metal_served")));
        assert!(receipt
            .refused_backend_ids
            .contains(&String::from("accelerator_specific_unbounded")));
    }
}
