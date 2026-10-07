use serde::{Deserialize, Serialize};

use psionic_eval::TassadarEffectTaxonomyReport;

/// Provider-facing receipt for the widened effect taxonomy report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectTaxonomyReceipt {
    pub report_id: String,
    pub taxonomy_id: String,
    pub route_policy_ref: String,
    pub admitted_case_count: u32,
    pub refusal_case_count: u32,
    pub host_state_case_count: u32,
    pub sandbox_delegation_case_count: u32,
    pub receipt_bound_input_case_count: u32,
    pub refused_side_effect_case_count: u32,
    pub detail: String,
}

impl TassadarEffectTaxonomyReceipt {
    /// Builds a provider-facing receipt from the shared eval report.
    #[must_use]
    pub fn from_report(report: &TassadarEffectTaxonomyReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            taxonomy_id: report.effect_taxonomy.taxonomy_id.clone(),
            route_policy_ref: report.route_policy_ref.clone(),
            admitted_case_count: report.admitted_case_count,
            refusal_case_count: report.refusal_case_count,
            host_state_case_count: report.host_state_case_count,
            sandbox_delegation_case_count: report.sandbox_delegation_case_count,
            receipt_bound_input_case_count: report.receipt_bound_input_case_count,
            refused_side_effect_case_count: report.refused_side_effect_case_count,
            detail: format!(
                "effect-taxonomy receipt `{}` covers admitted={}, refused={}, host_state_cases={}, sandbox_delegation_cases={}, receipt_bound_input_cases={}, refused_side_effect_cases={} under taxonomy `{}`",
                report.report_id,
                report.admitted_case_count,
                report.refusal_case_count,
                report.host_state_case_count,
                report.sandbox_delegation_case_count,
                report.receipt_bound_input_case_count,
                report.refused_side_effect_case_count,
                report.effect_taxonomy.taxonomy_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarEffectTaxonomyReceipt;
    use psionic_eval::build_tassadar_effect_taxonomy_report;

    #[test]
    fn effect_taxonomy_receipt_projects_eval_report() {
        let report = build_tassadar_effect_taxonomy_report().expect("report");
        let receipt = TassadarEffectTaxonomyReceipt::from_report(&report);

        assert_eq!(receipt.admitted_case_count, 4);
        assert_eq!(receipt.refusal_case_count, 4);
        assert_eq!(receipt.host_state_case_count, 2);
        assert_eq!(receipt.sandbox_delegation_case_count, 2);
        assert_eq!(receipt.receipt_bound_input_case_count, 2);
        assert_eq!(receipt.refused_side_effect_case_count, 1);
    }
}
