use serde::{Deserialize, Serialize};

use psionic_serve::TassadarSemanticWindowPublicationPolicy;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSemanticWindowMigrationPlannerReceipt {
    pub publication_id: String,
    pub active_window_id: String,
    pub routeable_requested_window_ids: Vec<String>,
    pub downgrade_only_requested_window_ids: Vec<String>,
    pub refused_requested_window_ids: Vec<String>,
    pub served_publication_allowed_window_ids: Vec<String>,
    pub detail: String,
}

impl TassadarSemanticWindowMigrationPlannerReceipt {
    #[must_use]
    pub fn from_policy(policy: &TassadarSemanticWindowPublicationPolicy) -> Self {
        Self {
            publication_id: policy.publication_id.clone(),
            active_window_id: policy.active_window_id.clone(),
            routeable_requested_window_ids: policy.routeable_requested_window_ids.clone(),
            downgrade_only_requested_window_ids: policy.downgrade_only_requested_window_ids.clone(),
            refused_requested_window_ids: policy.refused_requested_window_ids.clone(),
            served_publication_allowed_window_ids: policy
                .served_publication_allowed_window_ids
                .clone(),
            detail: format!(
                "semantic-window publication `{}` keeps active_window_id={}, routeable_windows={}, downgrade_only_windows={}, refused_windows={}, served_publication_allowed_windows={}",
                policy.publication_id,
                policy.active_window_id,
                policy.routeable_requested_window_ids.len(),
                policy.downgrade_only_requested_window_ids.len(),
                policy.refused_requested_window_ids.len(),
                policy.served_publication_allowed_window_ids.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarSemanticWindowMigrationPlannerReceipt;
    use psionic_serve::build_tassadar_semantic_window_publication_policy;

    #[test]
    fn semantic_window_migration_planner_receipt_projects_policy() {
        let policy = build_tassadar_semantic_window_publication_policy().expect("policy");
        let receipt = TassadarSemanticWindowMigrationPlannerReceipt::from_policy(&policy);

        assert_eq!(
            receipt.active_window_id,
            "tassadar.frozen_core_wasm.window.v1"
        );
        assert_eq!(receipt.routeable_requested_window_ids.len(), 2);
        assert_eq!(receipt.downgrade_only_requested_window_ids.len(), 1);
        assert_eq!(receipt.refused_requested_window_ids.len(), 2);
        assert!(receipt.served_publication_allowed_window_ids.is_empty());
    }
}
