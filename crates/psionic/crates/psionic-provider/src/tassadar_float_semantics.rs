use serde::{Deserialize, Serialize};

use psionic_eval::TassadarFloatSemanticsComparisonMatrixReport;

/// Provider-facing receipt for the bounded float-semantics matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFloatSemanticsReceipt {
    pub report_id: String,
    pub family_id: String,
    pub profile_id: String,
    pub nan_policy_id: String,
    pub comparison_policy_id: String,
    pub exact_case_ids: Vec<String>,
    pub refusal_case_ids: Vec<String>,
    pub detail: String,
}

impl TassadarFloatSemanticsReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarFloatSemanticsComparisonMatrixReport) -> Self {
        let exact_case_ids = report
            .cases
            .iter()
            .filter(|case| case.status == psionic_eval::TassadarFloatSemanticsCaseStatus::Exact)
            .map(|case| case.case_id.clone())
            .collect::<Vec<_>>();
        let refusal_case_ids = report
            .cases
            .iter()
            .filter(|case| case.status == psionic_eval::TassadarFloatSemanticsCaseStatus::Refused)
            .map(|case| case.case_id.clone())
            .collect::<Vec<_>>();
        Self {
            report_id: report.report_id.clone(),
            family_id: report.policy.family_id.clone(),
            profile_id: report.policy.profile_id.clone(),
            nan_policy_id: report.policy.nan_policy_id.clone(),
            comparison_policy_id: report.policy.comparison_policy_id.clone(),
            exact_case_ids,
            refusal_case_ids,
            detail: format!(
                "float semantics `{}` keeps bounded scalar-f32 policy explicit with exact_cases={} and refusal_cases={}",
                report.report_id,
                report.exact_case_count,
                report.refusal_case_count,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarFloatSemanticsReceipt;
    use psionic_eval::build_tassadar_float_semantics_comparison_matrix_report;

    #[test]
    fn float_semantics_receipt_projects_report() {
        let report = build_tassadar_float_semantics_comparison_matrix_report().expect("report");
        let receipt = TassadarFloatSemanticsReceipt::from_report(&report);

        assert_eq!(receipt.family_id, "tassadar.float_semantics.matrix.v1");
        assert_eq!(receipt.profile_id, "tassadar.float_semantics.scalar_f32.v1");
        assert!(receipt
            .refusal_case_ids
            .contains(&String::from("f64_scalar_refusal")));
    }
}
