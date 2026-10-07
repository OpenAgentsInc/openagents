use serde::{Deserialize, Serialize};

use psionic_runtime::TassadarTcmV1RuntimeContractReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalSubstrateReceipt {
    pub report_id: String,
    pub model_id: String,
    pub runtime_envelope: String,
    pub satisfied_runtime_semantic_ids: Vec<String>,
    pub refused_out_of_model_semantic_ids: Vec<String>,
    pub detail: String,
}

impl TassadarUniversalSubstrateReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarTcmV1RuntimeContractReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            model_id: report.substrate_model.model_id.clone(),
            runtime_envelope: report.runtime_envelope.clone(),
            satisfied_runtime_semantic_ids: report.satisfied_runtime_semantic_ids.clone(),
            refused_out_of_model_semantic_ids: report.refused_out_of_model_semantic_ids.clone(),
            detail: format!(
                "universal substrate receipt `{}` binds model `{}` to runtime_rows={}, refusal_rows={}, overall_green={}",
                report.report_id,
                report.substrate_model.model_id,
                report.runtime_rows.len(),
                report.refusal_rows.len(),
                report.overall_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarUniversalSubstrateReceipt;
    use psionic_runtime::build_tassadar_tcm_v1_runtime_contract_report;

    #[test]
    fn universal_substrate_receipt_projects_runtime_contract() {
        let report = build_tassadar_tcm_v1_runtime_contract_report().expect("report");
        let receipt = TassadarUniversalSubstrateReceipt::from_report(&report);

        assert_eq!(receipt.model_id, "tcm.v1");
        assert_eq!(receipt.satisfied_runtime_semantic_ids.len(), 4);
        assert_eq!(receipt.refused_out_of_model_semantic_ids.len(), 2);
    }
}
