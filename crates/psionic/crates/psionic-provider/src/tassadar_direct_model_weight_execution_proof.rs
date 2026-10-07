use serde::{Deserialize, Serialize};

use psionic_serve::TassadarDirectModelWeightExecutionProofReport;

/// Provider-facing receipt for the direct model-weight execution proof report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDirectModelWeightExecutionProofProviderReceipt {
    pub report_id: String,
    pub case_count: u32,
    pub direct_case_count: u32,
    pub fallback_free_case_count: u32,
    pub zero_external_call_case_count: u32,
    pub model_id: String,
    pub historical_fixture_model_id: String,
    pub parity_report_ref: String,
    pub lineage_contract_ref: String,
    pub route_descriptor_digest: String,
    pub case_ids: Vec<String>,
    pub detail: String,
}

impl TassadarDirectModelWeightExecutionProofProviderReceipt {
    /// Builds a provider-facing receipt from the served proof report.
    #[must_use]
    pub fn from_report(report: &TassadarDirectModelWeightExecutionProofReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            case_count: report.receipts.len() as u32,
            direct_case_count: report.direct_case_count,
            fallback_free_case_count: report.fallback_free_case_count,
            zero_external_call_case_count: report.zero_external_call_case_count,
            model_id: report.model_id.clone(),
            historical_fixture_model_id: report.historical_fixture_model_id.clone(),
            parity_report_ref: report.parity_report_ref.clone(),
            lineage_contract_ref: report.lineage_contract_ref.clone(),
            route_descriptor_digest: report.route_descriptor_digest.clone(),
            case_ids: report.case_ids.clone(),
            detail: format!(
                "direct model-weight proof `{}` covers {} cases for Transformer model `{}` against historical fixture `{}` on route `{}` with parity_ref=`{}`, lineage_ref=`{}`, direct_cases={}, fallback_free_cases={}, zero_external_call_cases={}",
                report.report_id,
                report.receipts.len(),
                report.model_id,
                report.historical_fixture_model_id,
                report.route_descriptor_digest,
                report.parity_report_ref,
                report.lineage_contract_ref,
                report.direct_case_count,
                report.fallback_free_case_count,
                report.zero_external_call_case_count,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarDirectModelWeightExecutionProofProviderReceipt;
    use psionic_serve::build_tassadar_direct_model_weight_execution_proof_report;

    #[test]
    fn direct_model_weight_execution_proof_provider_receipt_projects_report() {
        let report = build_tassadar_direct_model_weight_execution_proof_report().expect("report");
        let receipt = TassadarDirectModelWeightExecutionProofProviderReceipt::from_report(&report);

        assert_eq!(receipt.case_count, 3);
        assert_eq!(receipt.direct_case_count, 3);
        assert_eq!(receipt.fallback_free_case_count, 3);
        assert_eq!(receipt.zero_external_call_case_count, 3);
        assert_eq!(
            receipt.model_id,
            "tassadar-article-transformer-trace-bound-trained-v0"
        );
        assert!(receipt.case_ids.contains(&String::from("long_loop_kernel")));
    }
}
