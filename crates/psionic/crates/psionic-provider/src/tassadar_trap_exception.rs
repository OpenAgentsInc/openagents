use serde::{Deserialize, Serialize};

use psionic_eval::TassadarTrapExceptionReport;

/// Provider-facing receipt for the trap/exception semantics report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTrapExceptionReceipt {
    pub report_id: String,
    pub case_count: u32,
    pub exact_success_parity_case_count: u32,
    pub exact_trap_parity_case_count: u32,
    pub exact_refusal_parity_case_count: u32,
    pub drift_case_count: u32,
    pub non_success_case_count: u32,
    pub generated_from_ref_count: u32,
    pub detail: String,
}

impl TassadarTrapExceptionReceipt {
    /// Builds a provider-facing receipt from the eval report.
    #[must_use]
    pub fn from_report(report: &TassadarTrapExceptionReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            case_count: report.case_audits.len() as u32,
            exact_success_parity_case_count: report.exact_success_parity_case_count,
            exact_trap_parity_case_count: report.exact_trap_parity_case_count,
            exact_refusal_parity_case_count: report.exact_refusal_parity_case_count,
            drift_case_count: report.drift_case_count,
            non_success_case_count: report.exact_trap_parity_case_count
                + report.exact_refusal_parity_case_count
                + report.drift_case_count,
            generated_from_ref_count: report.generated_from_refs.len() as u32,
            detail: format!(
                "trap/exception report `{}` covers {} cases with success_parity={}, trap_parity={}, refusal_parity={}, drift={}, generated_from_refs={}",
                report.report_id,
                report.case_audits.len(),
                report.exact_success_parity_case_count,
                report.exact_trap_parity_case_count,
                report.exact_refusal_parity_case_count,
                report.drift_case_count,
                report.generated_from_refs.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarTrapExceptionReceipt;
    use psionic_eval::build_tassadar_trap_exception_report;

    #[test]
    fn trap_exception_receipt_projects_eval_report() {
        let report = build_tassadar_trap_exception_report();
        let receipt = TassadarTrapExceptionReceipt::from_report(&report);

        assert_eq!(receipt.case_count, 5);
        assert_eq!(receipt.exact_success_parity_case_count, 1);
        assert_eq!(receipt.exact_trap_parity_case_count, 2);
        assert_eq!(receipt.exact_refusal_parity_case_count, 2);
        assert_eq!(receipt.drift_case_count, 0);
        assert!(receipt.generated_from_ref_count >= 5);
    }
}
