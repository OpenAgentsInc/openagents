use serde::{Deserialize, Serialize};

use psionic_eval::TassadarThreadsResearchProfileEvalReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarThreadsResearchProfileReceipt {
    pub report_id: String,
    pub profile_id: String,
    pub green_scheduler_ids: Vec<String>,
    pub refused_scheduler_ids: Vec<String>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub sandbox_negative_only_case_count: u32,
    pub served_publication_allowed: bool,
    pub overall_green: bool,
    pub detail: String,
}

impl TassadarThreadsResearchProfileReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarThreadsResearchProfileEvalReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            profile_id: report.profile_id.clone(),
            green_scheduler_ids: report.green_scheduler_ids.clone(),
            refused_scheduler_ids: report.refused_scheduler_ids.clone(),
            exact_case_count: report.exact_case_count,
            refusal_case_count: report.refusal_case_count,
            sandbox_negative_only_case_count: report.sandbox_negative_only_case_count,
            served_publication_allowed: report.served_publication_allowed,
            overall_green: report.overall_green,
            detail: format!(
                "threads research profile receipt `{}` keeps green_schedulers={}, refused_schedulers={}, exact_cases={}, refusal_cases={}, sandbox_negative_only_cases={}, served_publication_allowed={}, overall_green={}",
                report.report_id,
                report.green_scheduler_ids.len(),
                report.refused_scheduler_ids.len(),
                report.exact_case_count,
                report.refusal_case_count,
                report.sandbox_negative_only_case_count,
                report.served_publication_allowed,
                report.overall_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarThreadsResearchProfileReceipt;
    use psionic_eval::build_tassadar_threads_research_profile_report;

    #[test]
    fn threads_research_profile_receipt_projects_report() {
        let report = build_tassadar_threads_research_profile_report();
        let receipt = TassadarThreadsResearchProfileReceipt::from_report(&report);

        assert!(receipt.overall_green);
        assert_eq!(receipt.exact_case_count, 2);
        assert_eq!(receipt.refusal_case_count, 1);
        assert_eq!(receipt.sandbox_negative_only_case_count, 1);
        assert!(!receipt.served_publication_allowed);
        assert!(
            receipt
                .refused_scheduler_ids
                .contains(&String::from("host_nondeterministic_runtime"))
        );
    }
}
