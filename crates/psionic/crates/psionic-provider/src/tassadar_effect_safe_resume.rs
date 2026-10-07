use serde::{Deserialize, Serialize};

use psionic_eval::TassadarEffectSafeResumeReport;

/// Provider-facing receipt for deterministic import-mediated effect-safe resume.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectSafeResumeReceipt {
    /// Stable report identifier.
    pub report_id: String,
    /// Stable target profile identifier.
    pub target_profile_id: String,
    /// Stable runtime bundle ref.
    pub runtime_bundle_ref: String,
    /// Number of admitted effect-safe continuation rows.
    pub admitted_case_count: u32,
    /// Number of refused continuation rows.
    pub refusal_case_count: u32,
    /// Stable admitted effect refs.
    pub continuation_safe_effect_refs: Vec<String>,
    /// Stable refused effect refs.
    pub continuation_refused_effect_refs: Vec<String>,
    /// Plain-language detail.
    pub detail: String,
}

impl TassadarEffectSafeResumeReceipt {
    /// Builds a provider-facing receipt from the shared eval report.
    #[must_use]
    pub fn from_report(report: &TassadarEffectSafeResumeReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            target_profile_id: report.target_profile_id.clone(),
            runtime_bundle_ref: report.runtime_bundle_ref.clone(),
            admitted_case_count: report.admitted_case_count,
            refusal_case_count: report.refusal_case_count,
            continuation_safe_effect_refs: report.continuation_safe_effect_refs.clone(),
            continuation_refused_effect_refs: report.continuation_refused_effect_refs.clone(),
            detail: format!(
                "effect-safe resume receipt `{}` carries target_profile=`{}` admitted_cases={}, refused_cases={}, safe_effect_refs={}, refused_effect_refs={}",
                report.report_id,
                report.target_profile_id,
                report.admitted_case_count,
                report.refusal_case_count,
                report.continuation_safe_effect_refs.len(),
                report.continuation_refused_effect_refs.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarEffectSafeResumeReceipt;
    use psionic_eval::build_tassadar_effect_safe_resume_report;

    #[test]
    fn effect_safe_resume_receipt_projects_report() {
        let report = build_tassadar_effect_safe_resume_report().expect("report");
        let receipt = TassadarEffectSafeResumeReceipt::from_report(&report);

        assert_eq!(
            receipt.target_profile_id,
            "tassadar.internal_compute.deterministic_import_subset.v1"
        );
        assert_eq!(receipt.admitted_case_count, 2);
        assert_eq!(receipt.refusal_case_count, 4);
        assert_eq!(receipt.continuation_safe_effect_refs, vec![String::from("env.clock_stub")]);
    }
}
