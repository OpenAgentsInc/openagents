use serde::{Deserialize, Serialize};

use psionic_eval::TassadarFloatProfileAcceptanceGateReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFloatProfileAcceptanceGateReceipt {
    pub report_id: String,
    pub green_profile_ids: Vec<String>,
    pub public_profile_allowed_profile_ids: Vec<String>,
    pub default_served_profile_allowed_profile_ids: Vec<String>,
    pub suppressed_profile_ids: Vec<String>,
    pub failed_profile_ids: Vec<String>,
    pub overall_green: bool,
    pub detail: String,
}

impl TassadarFloatProfileAcceptanceGateReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarFloatProfileAcceptanceGateReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            green_profile_ids: report.green_profile_ids.clone(),
            public_profile_allowed_profile_ids: report
                .public_profile_allowed_profile_ids
                .clone(),
            default_served_profile_allowed_profile_ids: report
                .default_served_profile_allowed_profile_ids
                .clone(),
            suppressed_profile_ids: report.suppressed_profile_ids.clone(),
            failed_profile_ids: report.failed_profile_ids.clone(),
            overall_green: report.overall_green,
            detail: format!(
                "float profile acceptance gate `{}` keeps green_profiles={}, public_profile_allowed_profiles={}, default_served_profile_allowed_profiles={}, suppressed_profiles={}, failed_profiles={}, overall_green={}",
                report.report_id,
                report.green_profile_ids.len(),
                report.public_profile_allowed_profile_ids.len(),
                report.default_served_profile_allowed_profile_ids.len(),
                report.suppressed_profile_ids.len(),
                report.failed_profile_ids.len(),
                report.overall_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarFloatProfileAcceptanceGateReceipt;
    use psionic_eval::build_tassadar_float_profile_acceptance_gate_report;

    #[test]
    fn float_profile_acceptance_gate_receipt_projects_report() {
        let report = build_tassadar_float_profile_acceptance_gate_report().expect("report");
        let receipt = TassadarFloatProfileAcceptanceGateReceipt::from_report(&report);

        assert!(receipt.overall_green);
        assert!(receipt
            .public_profile_allowed_profile_ids
            .contains(&String::from("tassadar.numeric_profile.f32_only.v1")));
        assert!(receipt
            .public_profile_allowed_profile_ids
            .contains(&String::from("tassadar.numeric_profile.mixed_i32_f32.v1")));
        assert!(receipt.default_served_profile_allowed_profile_ids.is_empty());
        assert!(receipt
            .suppressed_profile_ids
            .contains(&String::from(
                "tassadar.numeric_profile.bounded_f64_conversion.v1"
            )));
    }
}
