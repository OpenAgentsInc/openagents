use serde::{Deserialize, Serialize};

use psionic_eval::TassadarExceptionProfileReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExceptionProfileReceipt {
    pub report_id: String,
    pub green_profile_ids: Vec<String>,
    pub public_profile_allowed_profile_ids: Vec<String>,
    pub default_served_profile_allowed_profile_ids: Vec<String>,
    pub portability_envelope_ids: Vec<String>,
    pub exact_trap_stack_parity_case_count: u32,
    pub overall_green: bool,
    pub detail: String,
}

impl TassadarExceptionProfileReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarExceptionProfileReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            green_profile_ids: report.green_profile_ids.clone(),
            public_profile_allowed_profile_ids: report.public_profile_allowed_profile_ids.clone(),
            default_served_profile_allowed_profile_ids: report
                .default_served_profile_allowed_profile_ids
                .clone(),
            portability_envelope_ids: report.portability_envelope_ids.clone(),
            exact_trap_stack_parity_case_count: report.exact_trap_stack_parity_case_count,
            overall_green: report.overall_green,
            detail: format!(
                "exception profile report `{}` keeps green_profiles={}, public_profile_allowed_profiles={}, default_served_profile_allowed_profiles={}, portability_envelopes={}, trap_stack_parity_cases={}, overall_green={}",
                report.report_id,
                report.green_profile_ids.len(),
                report.public_profile_allowed_profile_ids.len(),
                report.default_served_profile_allowed_profile_ids.len(),
                report.portability_envelope_ids.len(),
                report.exact_trap_stack_parity_case_count,
                report.overall_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarExceptionProfileReceipt;
    use psionic_eval::build_tassadar_exception_profile_report;

    #[test]
    fn exception_profile_receipt_projects_report() {
        let report = build_tassadar_exception_profile_report();
        let receipt = TassadarExceptionProfileReceipt::from_report(&report);

        assert!(receipt.overall_green);
        assert_eq!(receipt.exact_trap_stack_parity_case_count, 2);
        assert!(
            receipt
                .public_profile_allowed_profile_ids
                .contains(&String::from(
                    "tassadar.proposal_profile.exceptions_try_catch_rethrow.v1"
                ))
        );
        assert!(
            receipt
                .default_served_profile_allowed_profile_ids
                .is_empty()
        );
    }
}
