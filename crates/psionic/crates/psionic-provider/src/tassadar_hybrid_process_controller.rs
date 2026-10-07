use serde::{Deserialize, Serialize};

use psionic_eval::TassadarHybridProcessControllerReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHybridProcessControllerReceipt {
    pub report_id: String,
    pub compiled_exact_case_count: u32,
    pub hybrid_verifier_case_count: u32,
    pub refused_case_count: u32,
    pub verifier_positive_delta_case_count: u32,
    pub challenge_green_case_count: u32,
    pub served_publication_allowed: bool,
    pub detail: String,
}

impl TassadarHybridProcessControllerReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarHybridProcessControllerReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            compiled_exact_case_count: report.compiled_exact_case_count,
            hybrid_verifier_case_count: report.hybrid_verifier_case_count,
            refused_case_count: report.refused_case_count,
            verifier_positive_delta_case_count: report.verifier_positive_delta_case_ids.len() as u32,
            challenge_green_case_count: report.challenge_green_case_ids.len() as u32,
            served_publication_allowed: report.served_publication_allowed,
            detail: format!(
                "hybrid process controller report `{}` keeps compiled_exact_cases={}, hybrid_cases={}, refused_cases={}, verifier_positive_delta_cases={}, challenge_green_cases={}, served_publication_allowed={}",
                report.report_id,
                report.compiled_exact_case_count,
                report.hybrid_verifier_case_count,
                report.refused_case_count,
                report.verifier_positive_delta_case_ids.len(),
                report.challenge_green_case_ids.len(),
                report.served_publication_allowed,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarHybridProcessControllerReceipt;
    use psionic_eval::build_tassadar_hybrid_process_controller_report;

    #[test]
    fn hybrid_process_controller_receipt_projects_report() {
        let report = build_tassadar_hybrid_process_controller_report();
        let receipt = TassadarHybridProcessControllerReceipt::from_report(&report);

        assert_eq!(receipt.compiled_exact_case_count, 1);
        assert_eq!(receipt.hybrid_verifier_case_count, 2);
        assert_eq!(receipt.refused_case_count, 1);
        assert_eq!(receipt.verifier_positive_delta_case_count, 2);
        assert!(!receipt.served_publication_allowed);
    }
}
