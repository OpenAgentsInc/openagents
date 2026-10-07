use serde::{Deserialize, Serialize};

use psionic_eval::TassadarSimulatorEffectProfileReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSimulatorEffectProfileReceipt {
    pub report_id: String,
    pub profile_id: String,
    pub runtime_bundle_ref: String,
    pub sandbox_boundary_report_ref: String,
    pub allowed_simulator_profile_ids: Vec<String>,
    pub refused_effect_ids: Vec<String>,
    pub portability_envelope_ids: Vec<String>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub served_publication_allowed: bool,
    pub detail: String,
}

impl TassadarSimulatorEffectProfileReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarSimulatorEffectProfileReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            profile_id: report.profile_id.clone(),
            runtime_bundle_ref: report.runtime_bundle_ref.clone(),
            sandbox_boundary_report_ref: report.sandbox_boundary_report_ref.clone(),
            allowed_simulator_profile_ids: report.allowed_simulator_profile_ids.clone(),
            refused_effect_ids: report.refused_effect_ids.clone(),
            portability_envelope_ids: report.portability_envelope_ids.clone(),
            exact_case_count: report.exact_case_count,
            refusal_case_count: report.refusal_case_count,
            served_publication_allowed: report.served_publication_allowed,
            detail: format!(
                "simulator-effect profile receipt `{}` carries simulator_profiles={}, refused_effects={}, exact_cases={}, refusal_rows={}, served_publication_allowed={}",
                report.report_id,
                report.allowed_simulator_profile_ids.len(),
                report.refused_effect_ids.len(),
                report.exact_case_count,
                report.refusal_case_count,
                report.served_publication_allowed,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarSimulatorEffectProfileReceipt;
    use psionic_eval::build_tassadar_simulator_effect_profile_report;

    #[test]
    fn simulator_effect_profile_receipt_projects_report() {
        let report = build_tassadar_simulator_effect_profile_report().expect("report");
        let receipt = TassadarSimulatorEffectProfileReceipt::from_report(&report);

        assert_eq!(receipt.profile_id, "tassadar.effect_profile.simulator_backed_io.v1");
        assert_eq!(
            receipt.portability_envelope_ids,
            vec![String::from("cpu_reference_current_host")]
        );
        assert_eq!(receipt.allowed_simulator_profile_ids.len(), 3);
        assert_eq!(receipt.refused_effect_ids.len(), 3);
        assert_eq!(receipt.exact_case_count, 3);
        assert_eq!(receipt.refusal_case_count, 3);
        assert!(!receipt.served_publication_allowed);
    }
}
