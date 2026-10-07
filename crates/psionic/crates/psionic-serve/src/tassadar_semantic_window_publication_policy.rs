use serde::{Deserialize, Serialize};
use thiserror::Error;

use psionic_eval::build_tassadar_semantic_window_migration_planner_report;

pub const TASSADAR_SEMANTIC_WINDOW_PUBLICATION_POLICY_ID: &str =
    "psionic.semantic_window_publication";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSemanticWindowPublicationPolicy {
    pub publication_id: String,
    pub migration_planner_report_ref: String,
    pub active_window_id: String,
    pub routeable_requested_window_ids: Vec<String>,
    pub downgrade_only_requested_window_ids: Vec<String>,
    pub refused_requested_window_ids: Vec<String>,
    pub served_publication_allowed_window_ids: Vec<String>,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarSemanticWindowPublicationPolicyError {
    #[error("semantic-window migration planner was not green")]
    InvalidPlanner,
    #[error("semantic-window publication widened served semantic windows")]
    ServedPublicationMustStayEmpty,
}

pub fn build_tassadar_semantic_window_publication_policy()
-> Result<TassadarSemanticWindowPublicationPolicy, TassadarSemanticWindowPublicationPolicyError> {
    let report = build_tassadar_semantic_window_migration_planner_report()
        .map_err(|_| TassadarSemanticWindowPublicationPolicyError::InvalidPlanner)?;
    if !report.overall_green {
        return Err(TassadarSemanticWindowPublicationPolicyError::InvalidPlanner);
    }
    if !report.served_publication_allowed_window_ids.is_empty() {
        return Err(TassadarSemanticWindowPublicationPolicyError::ServedPublicationMustStayEmpty);
    }
    Ok(TassadarSemanticWindowPublicationPolicy {
        publication_id: String::from(TASSADAR_SEMANTIC_WINDOW_PUBLICATION_POLICY_ID),
        migration_planner_report_ref: String::from(
            psionic_eval::TASSADAR_SEMANTIC_WINDOW_MIGRATION_PLANNER_REPORT_REF,
        ),
        active_window_id: report.active_window_id,
        routeable_requested_window_ids: report.selected_requested_window_ids,
        downgrade_only_requested_window_ids: report.downgraded_requested_window_ids,
        refused_requested_window_ids: report.refused_requested_window_ids,
        served_publication_allowed_window_ids: report.served_publication_allowed_window_ids,
        claim_boundary: String::from(
            "this served publication policy exposes only the active semantic window and the bounded downgrade-or-refuse map for declared requests. It keeps served semantic-window publication empty until a later explicit promotion gate is green",
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_SEMANTIC_WINDOW_PUBLICATION_POLICY_ID,
        build_tassadar_semantic_window_publication_policy,
    };

    #[test]
    fn semantic_window_publication_policy_keeps_served_window_widening_empty() {
        let policy = build_tassadar_semantic_window_publication_policy().expect("policy");

        assert_eq!(
            policy.publication_id,
            TASSADAR_SEMANTIC_WINDOW_PUBLICATION_POLICY_ID
        );
        assert_eq!(
            policy.active_window_id,
            "tassadar.frozen_core_wasm.window.v1"
        );
        assert_eq!(policy.routeable_requested_window_ids.len(), 2);
        assert_eq!(policy.downgrade_only_requested_window_ids.len(), 1);
        assert_eq!(policy.refused_requested_window_ids.len(), 2);
        assert!(policy.served_publication_allowed_window_ids.is_empty());
    }
}
