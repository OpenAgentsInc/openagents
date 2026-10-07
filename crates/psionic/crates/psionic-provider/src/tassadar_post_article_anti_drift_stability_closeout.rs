use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleAntiDriftStabilityCloseoutSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleAntiDriftStabilityCloseoutReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub closeout_status: String,
    pub lock_row_count: u32,
    pub invalidation_row_count: u32,
    pub validation_row_count: u32,
    pub all_required_surface_locks_green: bool,
    pub machine_identity_lock_complete: bool,
    pub control_and_replay_posture_locked: bool,
    pub semantics_and_continuation_locked: bool,
    pub equivalent_choice_and_served_boundary_locked: bool,
    pub portability_and_minimality_locked: bool,
    pub plugin_capability_boundary_locked: bool,
    pub stronger_terminal_claims_require_closure_bundle: bool,
    pub stronger_plugin_platform_claims_require_closure_bundle: bool,
    pub closure_bundle_issue_id: String,
    pub detail: String,
}

impl TassadarPostArticleAntiDriftStabilityCloseoutReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarPostArticleAntiDriftStabilityCloseoutSummary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_model_id: summary.canonical_model_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            closeout_status: match summary.closeout_status {
                psionic_eval::TassadarPostArticleAntiDriftCloseoutStatus::Green => {
                    String::from("green")
                }
                psionic_eval::TassadarPostArticleAntiDriftCloseoutStatus::Blocked => {
                    String::from("blocked")
                }
            },
            lock_row_count: summary.lock_row_count,
            invalidation_row_count: summary.invalidation_row_count,
            validation_row_count: summary.validation_row_count,
            all_required_surface_locks_green: summary.all_required_surface_locks_green,
            machine_identity_lock_complete: summary.machine_identity_lock_complete,
            control_and_replay_posture_locked: summary.control_and_replay_posture_locked,
            semantics_and_continuation_locked: summary.semantics_and_continuation_locked,
            equivalent_choice_and_served_boundary_locked: summary
                .equivalent_choice_and_served_boundary_locked,
            portability_and_minimality_locked: summary.portability_and_minimality_locked,
            plugin_capability_boundary_locked: summary.plugin_capability_boundary_locked,
            stronger_terminal_claims_require_closure_bundle: summary
                .stronger_terminal_claims_require_closure_bundle,
            stronger_plugin_platform_claims_require_closure_bundle: summary
                .stronger_plugin_platform_claims_require_closure_bundle,
            closure_bundle_issue_id: summary.closure_bundle_issue_id.clone(),
            detail: format!(
                "post-article anti-drift closeout summary `{}` keeps closeout_status={:?}, machine_identity_id=`{}`, canonical_route_id=`{}`, and closure_bundle_issue_id=`{}`.",
                summary.report_id,
                summary.closeout_status,
                summary.machine_identity_id,
                summary.canonical_route_id,
                summary.closure_bundle_issue_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleAntiDriftStabilityCloseoutReceipt;
    use psionic_research::build_tassadar_post_article_anti_drift_stability_closeout_summary;

    #[test]
    fn post_article_anti_drift_closeout_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_anti_drift_stability_closeout_summary().expect("summary");
        let receipt =
            TassadarPostArticleAntiDriftStabilityCloseoutReceipt::from_summary(&summary);

        assert_eq!(receipt.closeout_status, "green");
        assert!(receipt.all_required_surface_locks_green);
        assert!(receipt.machine_identity_lock_complete);
        assert!(receipt.control_and_replay_posture_locked);
        assert!(receipt.semantics_and_continuation_locked);
        assert!(receipt.equivalent_choice_and_served_boundary_locked);
        assert!(receipt.portability_and_minimality_locked);
        assert!(receipt.plugin_capability_boundary_locked);
        assert!(receipt.stronger_terminal_claims_require_closure_bundle);
        assert!(receipt.stronger_plugin_platform_claims_require_closure_bundle);
        assert_eq!(receipt.closure_bundle_issue_id, "TAS-215");
    }
}
