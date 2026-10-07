use serde::{Deserialize, Serialize};

use psionic_eval::TassadarPreemptiveJobProfileReport;

/// Provider-facing receipt for the bounded preemptive-job profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPreemptiveJobReceipt {
    pub report_id: String,
    pub profile_id: String,
    pub runtime_bundle_ref: String,
    pub fairness_report_ref: String,
    pub green_scheduler_ids: Vec<String>,
    pub refused_scheduler_ids: Vec<String>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub resumable_process_ids: Vec<String>,
    pub served_publication_allowed: bool,
    pub detail: String,
}

impl TassadarPreemptiveJobReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarPreemptiveJobProfileReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            profile_id: report.profile_id.clone(),
            runtime_bundle_ref: report.runtime_bundle_ref.clone(),
            fairness_report_ref: report.fairness_report_ref.clone(),
            green_scheduler_ids: report.green_scheduler_ids.clone(),
            refused_scheduler_ids: report.refused_scheduler_ids.clone(),
            exact_case_count: report.exact_case_count,
            refusal_case_count: report.refusal_case_count,
            resumable_process_ids: report.resumable_process_ids.clone(),
            served_publication_allowed: report.served_publication_allowed,
            detail: format!(
                "preemptive-job profile report `{}` carries {} exact rows, {} refusal rows, {} green schedulers, and {} resumable process ids",
                report.report_id,
                report.exact_case_count,
                report.refusal_case_count,
                report.green_scheduler_ids.len(),
                report.resumable_process_ids.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPreemptiveJobReceipt;
    use psionic_eval::build_tassadar_preemptive_job_profile_report;

    #[test]
    fn preemptive_job_receipt_projects_report() {
        let report = build_tassadar_preemptive_job_profile_report().expect("report");
        let receipt = TassadarPreemptiveJobReceipt::from_report(&report);

        assert_eq!(
            receipt.profile_id,
            "tassadar.internal_compute.preemptive_jobs.v1"
        );
        assert_eq!(receipt.exact_case_count, 2);
        assert_eq!(receipt.refusal_case_count, 2);
        assert_eq!(receipt.green_scheduler_ids.len(), 2);
        assert_eq!(
            receipt.refused_scheduler_ids,
            vec![String::from("host_nondeterministic_scheduler")]
        );
        assert!(!receipt.served_publication_allowed);
    }
}
