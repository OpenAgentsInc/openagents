use serde::{Deserialize, Serialize};

use psionic_eval::TassadarExecutionCheckpointReport;

/// Provider-facing receipt for the checkpointed multi-slice execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionCheckpointProviderReceipt {
    /// Stable report identifier.
    pub report_id: String,
    /// Stable runtime-bundle reference.
    pub runtime_bundle_ref: String,
    /// Stable checkpoint-family identifier.
    pub checkpoint_family_id: String,
    /// Number of exact fresh-vs-resumed parity rows.
    pub exact_resume_parity_count: u32,
    /// Number of typed refusal rows.
    pub refusal_case_count: u32,
    /// Stable latest-checkpoint identifiers surfaced by the report.
    pub latest_checkpoint_ids: Vec<String>,
    /// Stable datastream manifest digests for the latest checkpoints.
    pub latest_checkpoint_manifest_digests: Vec<String>,
    /// Plain-language receipt detail.
    pub detail: String,
}

impl TassadarExecutionCheckpointProviderReceipt {
    /// Builds a provider-facing receipt from the shared eval report.
    #[must_use]
    pub fn from_report(report: &TassadarExecutionCheckpointReport) -> Self {
        let latest_checkpoint_ids = report
            .case_reports
            .iter()
            .map(|case| case.latest_checkpoint_id.clone())
            .collect::<Vec<_>>();
        let latest_checkpoint_manifest_digests = report
            .case_reports
            .iter()
            .filter_map(|case| case.checkpoint_artifacts.last())
            .map(|artifact| artifact.manifest_ref.manifest_digest.clone())
            .collect::<Vec<_>>();
        Self {
            report_id: report.report_id.clone(),
            runtime_bundle_ref: report.runtime_bundle_ref.clone(),
            checkpoint_family_id: report.checkpoint_family_id.clone(),
            exact_resume_parity_count: report.exact_resume_parity_count,
            refusal_case_count: report.refusal_case_count,
            latest_checkpoint_ids,
            latest_checkpoint_manifest_digests,
            detail: format!(
                "execution-checkpoint receipt `{}` carries {} exact parity rows, {} refusal rows, and {} latest checkpoint locators under family `{}`",
                report.report_id,
                report.exact_resume_parity_count,
                report.refusal_case_count,
                report.latest_checkpoint_locator_count,
                report.checkpoint_family_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarExecutionCheckpointProviderReceipt;
    use psionic_eval::build_tassadar_execution_checkpoint_report;

    #[test]
    fn execution_checkpoint_provider_receipt_projects_report() {
        let report = build_tassadar_execution_checkpoint_report().expect("report");
        let receipt = TassadarExecutionCheckpointProviderReceipt::from_report(&report);

        assert_eq!(receipt.exact_resume_parity_count, 3);
        assert_eq!(receipt.refusal_case_count, 12);
        assert_eq!(receipt.latest_checkpoint_ids.len(), 3);
        assert_eq!(receipt.latest_checkpoint_manifest_digests.len(), 3);
    }
}
