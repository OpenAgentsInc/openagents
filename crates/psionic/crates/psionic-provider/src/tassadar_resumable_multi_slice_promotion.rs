use serde::{Deserialize, Serialize};

use psionic_eval::TassadarResumableMultiSlicePromotionReport;

/// Provider-facing receipt for the resumable multi-slice promotion lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarResumableMultiSlicePromotionReceipt {
    /// Stable report identifier.
    pub report_id: String,
    /// Stable resumable profile identifier.
    pub profile_id: String,
    /// Total exact fresh-vs-resumed parity rows.
    pub exact_resume_parity_count: u32,
    /// Total typed refusal rows.
    pub refusal_case_count: u32,
    /// Total checkpoint locator rows.
    pub checkpoint_locator_count: u32,
    /// Stable checkpoint-family identifiers carried by the report.
    pub checkpoint_family_ids: Vec<String>,
    /// Stable latest manifest digests from the call-frame resume lane.
    pub call_frame_manifest_digests: Vec<String>,
    /// Plain-language receipt detail.
    pub detail: String,
}

impl TassadarResumableMultiSlicePromotionReceipt {
    /// Builds a provider-facing receipt from the shared eval report.
    #[must_use]
    pub fn from_report(report: &TassadarResumableMultiSlicePromotionReport) -> Self {
        let call_frame_manifest_digests = report
            .call_frame_case_reports
            .iter()
            .map(|case| case.checkpoint_artifact.manifest_ref.manifest_digest.clone())
            .collect::<Vec<_>>();
        Self {
            report_id: report.report_id.clone(),
            profile_id: report.profile_id.clone(),
            exact_resume_parity_count: report.exact_resume_parity_count,
            refusal_case_count: report.refusal_case_count,
            checkpoint_locator_count: report.checkpoint_locator_count,
            checkpoint_family_ids: report.checkpoint_family_ids.clone(),
            call_frame_manifest_digests,
            detail: format!(
                "resumable multi-slice promotion receipt `{}` carries profile `{}` with exact_resume_parity_count={}, refusal_case_count={}, and checkpoint_locator_count={}",
                report.report_id,
                report.profile_id,
                report.exact_resume_parity_count,
                report.refusal_case_count,
                report.checkpoint_locator_count,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarResumableMultiSlicePromotionReceipt;
    use psionic_eval::build_tassadar_resumable_multi_slice_promotion_report;

    #[test]
    fn resumable_multi_slice_promotion_receipt_projects_report() {
        let report = build_tassadar_resumable_multi_slice_promotion_report().expect("report");
        let receipt = TassadarResumableMultiSlicePromotionReceipt::from_report(&report);

        assert_eq!(
            receipt.profile_id,
            "tassadar.internal_compute.resumable_multi_slice.v1"
        );
        assert_eq!(receipt.call_frame_manifest_digests.len(), 2);
        assert!(receipt.exact_resume_parity_count >= 6);
    }
}
