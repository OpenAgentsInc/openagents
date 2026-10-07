use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub packet_abi_version: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub invocation_receipt_profile_id: String,
    pub conformance_harness_id: String,
    pub benchmark_harness_id: String,
    pub contract_status: String,
    pub conformance_row_count: u32,
    pub workflow_row_count: u32,
    pub isolation_negative_row_count: u32,
    pub benchmark_row_count: u32,
    pub validation_row_count: u32,
    pub deferred_issue_ids: Vec<String>,
    pub conformance_sandbox_green: bool,
    pub operator_internal_only_posture: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessSummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            packet_abi_version: summary.packet_abi_version.clone(),
            host_owned_runtime_api_id: summary.host_owned_runtime_api_id.clone(),
            engine_abstraction_id: summary.engine_abstraction_id.clone(),
            invocation_receipt_profile_id: summary.invocation_receipt_profile_id.clone(),
            conformance_harness_id: summary.conformance_harness_id.clone(),
            benchmark_harness_id: summary.benchmark_harness_id.clone(),
            contract_status: format!("{:?}", summary.contract_status).to_lowercase(),
            conformance_row_count: summary.conformance_row_count,
            workflow_row_count: summary.workflow_row_count,
            isolation_negative_row_count: summary.isolation_negative_row_count,
            benchmark_row_count: summary.benchmark_row_count,
            validation_row_count: summary.validation_row_count,
            deferred_issue_ids: summary.deferred_issue_ids.clone(),
            conformance_sandbox_green: summary.conformance_sandbox_green,
            operator_internal_only_posture: summary.operator_internal_only_posture,
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            weighted_plugin_control_allowed: summary.weighted_plugin_control_allowed,
            plugin_publication_allowed: summary.plugin_publication_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article plugin conformance summary `{}` keeps contract_status={:?}, conformance_harness_id=`{}`, conformance_rows={}, benchmark_rows={}, and deferred_issue_ids={}.",
                summary.report_id,
                summary.contract_status,
                summary.conformance_harness_id,
                summary.conformance_row_count,
                summary.benchmark_row_count,
                summary.deferred_issue_ids.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReceipt;
    use psionic_research::build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_summary;

    #[test]
    fn post_article_plugin_conformance_harness_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_summary()
                .expect("summary");
        let receipt =
            TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReceipt::from_summary(
                &summary,
            );

        assert_eq!(receipt.contract_status, "green");
        assert_eq!(receipt.packet_abi_version, "packet.v1");
        assert_eq!(
            receipt.host_owned_runtime_api_id,
            "tassadar.plugin_runtime.host_owned_api.v1"
        );
        assert_eq!(
            receipt.engine_abstraction_id,
            "tassadar.plugin_runtime.engine_abstraction.v1"
        );
        assert_eq!(
            receipt.invocation_receipt_profile_id,
            "tassadar.plugin_runtime.invocation_receipts.v1"
        );
        assert_eq!(
            receipt.conformance_harness_id,
            "tassadar.plugin_runtime.conformance_harness.v1"
        );
        assert_eq!(
            receipt.benchmark_harness_id,
            "tassadar.plugin_runtime.benchmark_harness.v1"
        );
        assert!(receipt.deferred_issue_ids.is_empty());
        assert!(receipt.conformance_sandbox_green);
        assert!(receipt.operator_internal_only_posture);
        assert!(receipt.rebase_claim_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.weighted_plugin_control_allowed);
        assert!(!receipt.plugin_publication_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
