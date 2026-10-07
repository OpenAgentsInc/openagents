use serde::{Deserialize, Serialize};

use psionic_ir::TassadarModuleTrustPosture;
use psionic_runtime::TassadarModuleTrustIsolationReport;

/// Provider-facing receipt for the runtime-owned module trust-isolation report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleTrustIsolationReceipt {
    pub report_id: String,
    pub bundle_count: u32,
    pub research_bundle_count: u32,
    pub benchmark_internal_bundle_count: u32,
    pub challenge_gated_bundle_count: u32,
    pub allowed_case_count: u32,
    pub refused_case_count: u32,
    pub cross_tier_refusal_count: u32,
    pub privilege_escalation_refusal_count: u32,
    pub mount_policy_refusal_count: u32,
    pub detail: String,
}

impl TassadarModuleTrustIsolationReceipt {
    /// Builds a provider-facing receipt from the runtime trust-isolation report.
    #[must_use]
    pub fn from_report(report: &TassadarModuleTrustIsolationReport) -> Self {
        let research_bundle_count = report
            .bundles
            .iter()
            .filter(|bundle| bundle.trust_posture == TassadarModuleTrustPosture::ResearchOnly)
            .count() as u32;
        let benchmark_internal_bundle_count = report
            .bundles
            .iter()
            .filter(|bundle| {
                bundle.trust_posture == TassadarModuleTrustPosture::BenchmarkGatedInternal
            })
            .count() as u32;
        let challenge_gated_bundle_count = report
            .bundles
            .iter()
            .filter(|bundle| {
                bundle.trust_posture == TassadarModuleTrustPosture::ChallengeGatedInstall
            })
            .count() as u32;
        Self {
            report_id: report.report_id.clone(),
            bundle_count: report.bundles.len() as u32,
            research_bundle_count,
            benchmark_internal_bundle_count,
            challenge_gated_bundle_count,
            allowed_case_count: report.allowed_case_count,
            refused_case_count: report.refused_case_count,
            cross_tier_refusal_count: report.cross_tier_refusal_count,
            privilege_escalation_refusal_count: report.privilege_escalation_refusal_count,
            mount_policy_refusal_count: report.mount_policy_refusal_count,
            detail: format!(
                "module trust-isolation report `{}` exposes {} bundles with research={}, benchmark_internal={}, challenge_gated={}, allowed_cases={}, refused_cases={}, cross_tier_refusals={}, privilege_escalation_refusals={}, and mount_policy_refusals={}",
                report.report_id,
                report.bundles.len(),
                research_bundle_count,
                benchmark_internal_bundle_count,
                challenge_gated_bundle_count,
                report.allowed_case_count,
                report.refused_case_count,
                report.cross_tier_refusal_count,
                report.privilege_escalation_refusal_count,
                report.mount_policy_refusal_count,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarModuleTrustIsolationReceipt;
    use psionic_runtime::build_tassadar_module_trust_isolation_report;

    #[test]
    fn module_trust_isolation_receipt_projects_runtime_report() {
        let report = build_tassadar_module_trust_isolation_report();
        let receipt = TassadarModuleTrustIsolationReceipt::from_report(&report);

        assert_eq!(receipt.bundle_count, 4);
        assert_eq!(receipt.research_bundle_count, 1);
        assert_eq!(receipt.challenge_gated_bundle_count, 1);
        assert_eq!(receipt.allowed_case_count, 2);
        assert_eq!(receipt.refused_case_count, 3);
    }
}
