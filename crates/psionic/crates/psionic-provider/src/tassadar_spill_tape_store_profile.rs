use serde::{Deserialize, Serialize};

use psionic_eval::TassadarSpillTapeStoreReport;

/// Provider-facing receipt for the bounded spill/tape continuation profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSpillTapeStoreReceipt {
    pub report_id: String,
    pub runtime_bundle_ref: String,
    pub profile_id: String,
    pub spill_segment_family_id: String,
    pub external_tape_store_family_id: String,
    pub portability_envelope_ids: Vec<String>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub process_ids: Vec<String>,
    pub spill_manifest_digests: Vec<String>,
    pub detail: String,
}

impl TassadarSpillTapeStoreReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarSpillTapeStoreReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            runtime_bundle_ref: report.runtime_bundle_ref.clone(),
            profile_id: report.profile_id.clone(),
            spill_segment_family_id: report.spill_segment_family_id.clone(),
            external_tape_store_family_id: report.external_tape_store_family_id.clone(),
            portability_envelope_ids: report.portability_envelope_ids.clone(),
            exact_case_count: report.exact_case_count,
            refusal_case_count: report.refusal_case_count,
            process_ids: report
                .case_reports
                .iter()
                .map(|case| case.process_id.clone())
                .collect(),
            spill_manifest_digests: report
                .case_reports
                .iter()
                .flat_map(|case| case.spill_segment_artifacts.iter())
                .map(|artifact| artifact.manifest_ref.manifest_digest.clone())
                .collect(),
            detail: format!(
                "spill/tape receipt `{}` carries {} exact rows, {} refusal rows, and {} spill manifests under profile `{}`",
                report.report_id,
                report.exact_case_count,
                report.refusal_case_count,
                report
                    .case_reports
                    .iter()
                    .map(|case| case.spill_segment_artifacts.len())
                    .sum::<usize>(),
                report.profile_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarSpillTapeStoreReceipt;
    use psionic_eval::build_tassadar_spill_tape_store_report;

    #[test]
    fn spill_tape_store_receipt_projects_report() {
        let report = build_tassadar_spill_tape_store_report().expect("report");
        let receipt = TassadarSpillTapeStoreReceipt::from_report(&report);

        assert_eq!(receipt.profile_id, "tassadar.internal_compute.spill_tape_store.v1");
        assert_eq!(receipt.portability_envelope_ids, vec![String::from("cpu_reference_current_host")]);
        assert_eq!(receipt.exact_case_count, 2);
        assert_eq!(receipt.refusal_case_count, 3);
        assert_eq!(receipt.process_ids.len(), 3);
        assert_eq!(receipt.spill_manifest_digests.len(), 5);
    }
}
