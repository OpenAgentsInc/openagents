use serde::{Deserialize, Serialize};

use psionic_eval::TassadarProcessObjectReport;

/// Provider-facing receipt for the durable process-object family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProcessObjectFamilyReceipt {
    /// Stable report identifier.
    pub report_id: String,
    /// Stable runtime-bundle reference.
    pub runtime_bundle_ref: String,
    /// Stable named process profile identifier.
    pub profile_id: String,
    /// Stable process-snapshot family identifier.
    pub snapshot_family_id: String,
    /// Stable process-tape family identifier.
    pub tape_family_id: String,
    /// Stable process work-queue family identifier.
    pub work_queue_family_id: String,
    /// Number of exact durable-process parity rows.
    pub exact_process_parity_count: u32,
    /// Number of typed refusal rows.
    pub refusal_case_count: u32,
    /// Stable process identifiers surfaced by the report.
    pub process_ids: Vec<String>,
    /// Stable manifest digests for the persisted process snapshots.
    pub snapshot_manifest_digests: Vec<String>,
    /// Plain-language receipt detail.
    pub detail: String,
}

impl TassadarProcessObjectFamilyReceipt {
    /// Builds a provider-facing receipt from the shared eval report.
    #[must_use]
    pub fn from_report(report: &TassadarProcessObjectReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            runtime_bundle_ref: report.runtime_bundle_ref.clone(),
            profile_id: report.profile_id.clone(),
            snapshot_family_id: report.snapshot_family_id.clone(),
            tape_family_id: report.tape_family_id.clone(),
            work_queue_family_id: report.work_queue_family_id.clone(),
            exact_process_parity_count: report.exact_process_parity_count,
            refusal_case_count: report.refusal_case_count,
            process_ids: report
                .case_reports
                .iter()
                .map(|case| case.process_id.clone())
                .collect(),
            snapshot_manifest_digests: report
                .case_reports
                .iter()
                .map(|case| case.snapshot_artifact.manifest_ref.manifest_digest.clone())
                .collect(),
            detail: format!(
                "process-object receipt `{}` carries {} exact durable-process rows, {} refusal rows, and {} process snapshots under profile `{}`",
                report.report_id,
                report.exact_process_parity_count,
                report.refusal_case_count,
                report.case_reports.len(),
                report.profile_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarProcessObjectFamilyReceipt;
    use psionic_eval::build_tassadar_process_object_report;

    #[test]
    fn process_object_family_receipt_projects_report() {
        let report = build_tassadar_process_object_report().expect("report");
        let receipt = TassadarProcessObjectFamilyReceipt::from_report(&report);

        assert_eq!(receipt.exact_process_parity_count, 3);
        assert_eq!(receipt.refusal_case_count, 9);
        assert_eq!(receipt.process_ids.len(), 3);
        assert_eq!(receipt.snapshot_manifest_digests.len(), 3);
    }
}
