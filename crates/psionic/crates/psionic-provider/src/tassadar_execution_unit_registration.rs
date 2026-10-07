use serde::{Deserialize, Serialize};

use psionic_serve::TassadarExecutionUnitRegistrationReport;

/// Provider-facing receipt for the serve-owned executor-family registration report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionUnitRegistrationReceipt {
    pub report_id: String,
    pub unit_id: String,
    pub publishable_workload_class_count: u32,
    pub refusal_taxonomy_count: u32,
    pub runtime_backend: String,
    pub pricing_posture: psionic_serve::TassadarExecutionUnitPricingPosture,
    pub settlement_eligible: bool,
    pub detail: String,
}

impl TassadarExecutionUnitRegistrationReceipt {
    /// Builds a provider-facing receipt from the serve report.
    #[must_use]
    pub fn from_report(report: &TassadarExecutionUnitRegistrationReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            unit_id: report.descriptor.unit_id.clone(),
            publishable_workload_class_count: report.publishable_workload_class_count,
            refusal_taxonomy_count: report.refusal_taxonomy_count,
            runtime_backend: report.descriptor.runtime_backend.clone(),
            pricing_posture: report.descriptor.pricing.pricing_posture,
            settlement_eligible: report.descriptor.pricing.settlement_eligible,
            detail: format!(
                "execution-unit registration `{}` exposes unit_id=`{}`, publishable_workload_classes={}, refusal_taxonomy_count={}, runtime_backend=`{}`, pricing_posture=`{:?}`, settlement_eligible={}",
                report.report_id,
                report.descriptor.unit_id,
                report.publishable_workload_class_count,
                report.refusal_taxonomy_count,
                report.descriptor.runtime_backend,
                report.descriptor.pricing.pricing_posture,
                report.descriptor.pricing.settlement_eligible,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarExecutionUnitRegistrationReceipt;
    use psionic_serve::build_tassadar_execution_unit_registration_report;

    #[test]
    fn execution_unit_registration_receipt_projects_serve_report() {
        let report = build_tassadar_execution_unit_registration_report().expect("report");
        let receipt = TassadarExecutionUnitRegistrationReceipt::from_report(&report);

        assert_eq!(receipt.runtime_backend, "cpu");
        assert!(receipt.publishable_workload_class_count >= 1);
        assert!(receipt.refusal_taxonomy_count >= 1);
        assert_eq!(receipt.settlement_eligible, false);
    }
}
