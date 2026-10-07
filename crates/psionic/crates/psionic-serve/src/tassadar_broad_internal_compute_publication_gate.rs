use serde::{Deserialize, Serialize};
use thiserror::Error;

use psionic_eval::{
    build_tassadar_broad_internal_compute_acceptance_gate_report,
    build_tassadar_subset_profile_promotion_gate_report,
    TassadarBroadInternalComputeAcceptanceStatus,
    TASSADAR_BROAD_INTERNAL_COMPUTE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_SUBSET_PROFILE_PROMOTION_GATE_REPORT_REF,
};
use psionic_runtime::TASSADAR_BROAD_INTERNAL_COMPUTE_PORTABILITY_REPORT_REF;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadInternalComputePublicationDecisionStatus {
    Published,
    Suppressed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadInternalComputePublicationDecision {
    pub profile_id: String,
    pub portability_report_ref: String,
    pub acceptance_gate_report_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subset_profile_promotion_gate_report_ref: Option<String>,
    pub status: TassadarBroadInternalComputePublicationDecisionStatus,
    pub detail: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarBroadInternalComputePublicationDecisionError {
    #[error("unknown broad internal-compute profile `{profile_id}`")]
    UnknownProfile { profile_id: String },
    #[error("profile `{profile_id}` is suppressed: {detail}")]
    Suppressed { profile_id: String, detail: String },
    #[error("profile `{profile_id}` failed the acceptance gate: {detail}")]
    Failed { profile_id: String, detail: String },
}

pub fn tassadar_broad_internal_compute_publication_decision(
    profile_id: &str,
) -> Result<
    TassadarBroadInternalComputePublicationDecision,
    TassadarBroadInternalComputePublicationDecisionError,
> {
    let subset_gate = build_tassadar_subset_profile_promotion_gate_report()
        .expect("subset profile promotion gate should build");
    if let Some(row) = subset_gate
        .profile_rows
        .iter()
        .find(|row| row.profile_id == profile_id)
    {
        let status = if row.served_publication_allowed {
            TassadarBroadInternalComputePublicationDecisionStatus::Published
        } else if row.gate_green {
            TassadarBroadInternalComputePublicationDecisionStatus::Suppressed
        } else {
            TassadarBroadInternalComputePublicationDecisionStatus::Failed
        };
        return Ok(TassadarBroadInternalComputePublicationDecision {
            profile_id: String::from(profile_id),
            portability_report_ref: String::from(
                TASSADAR_BROAD_INTERNAL_COMPUTE_PORTABILITY_REPORT_REF,
            ),
            acceptance_gate_report_ref: String::from(
                TASSADAR_BROAD_INTERNAL_COMPUTE_ACCEPTANCE_GATE_REPORT_REF,
            ),
            subset_profile_promotion_gate_report_ref: Some(String::from(
                TASSADAR_SUBSET_PROFILE_PROMOTION_GATE_REPORT_REF,
            )),
            status,
            detail: row.detail.clone(),
        });
    }
    let report = build_tassadar_broad_internal_compute_acceptance_gate_report()
        .expect("broad internal-compute acceptance gate should build");
    let Some(row) = report
        .profile_rows
        .iter()
        .find(|row| row.profile_id == profile_id)
    else {
        return Err(
            TassadarBroadInternalComputePublicationDecisionError::UnknownProfile {
                profile_id: String::from(profile_id),
            },
        );
    };
    let status = match row.gate_status {
        TassadarBroadInternalComputeAcceptanceStatus::Green => {
            TassadarBroadInternalComputePublicationDecisionStatus::Published
        }
        TassadarBroadInternalComputeAcceptanceStatus::Suppressed => {
            TassadarBroadInternalComputePublicationDecisionStatus::Suppressed
        }
        TassadarBroadInternalComputeAcceptanceStatus::Failed => {
            TassadarBroadInternalComputePublicationDecisionStatus::Failed
        }
    };
    Ok(TassadarBroadInternalComputePublicationDecision {
        profile_id: String::from(profile_id),
        portability_report_ref: String::from(
            TASSADAR_BROAD_INTERNAL_COMPUTE_PORTABILITY_REPORT_REF,
        ),
        acceptance_gate_report_ref: String::from(
            TASSADAR_BROAD_INTERNAL_COMPUTE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        subset_profile_promotion_gate_report_ref: None,
        status,
        detail: row.detail.clone(),
    })
}

pub fn require_tassadar_broad_internal_compute_profile_publication(
    profile_id: &str,
) -> Result<
    TassadarBroadInternalComputePublicationDecision,
    TassadarBroadInternalComputePublicationDecisionError,
> {
    let decision = tassadar_broad_internal_compute_publication_decision(profile_id)?;
    match decision.status {
        TassadarBroadInternalComputePublicationDecisionStatus::Published => Ok(decision),
        TassadarBroadInternalComputePublicationDecisionStatus::Suppressed => Err(
            TassadarBroadInternalComputePublicationDecisionError::Suppressed {
                profile_id: decision.profile_id.clone(),
                detail: decision.detail.clone(),
            },
        ),
        TassadarBroadInternalComputePublicationDecisionStatus::Failed => Err(
            TassadarBroadInternalComputePublicationDecisionError::Failed {
                profile_id: decision.profile_id.clone(),
                detail: decision.detail.clone(),
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        require_tassadar_broad_internal_compute_profile_publication,
        tassadar_broad_internal_compute_publication_decision,
        TassadarBroadInternalComputePublicationDecisionError,
        TassadarBroadInternalComputePublicationDecisionStatus,
    };

    #[test]
    fn broad_internal_compute_publication_decision_publishes_article_profile_only() {
        let article = tassadar_broad_internal_compute_publication_decision(
            "tassadar.internal_compute.article_closeout.v1",
        )
        .expect("article profile");
        assert_eq!(
            article.status,
            TassadarBroadInternalComputePublicationDecisionStatus::Published
        );

        let generalized = tassadar_broad_internal_compute_publication_decision(
            "tassadar.internal_compute.generalized_abi.v1",
        )
        .expect("generalized abi profile");
        assert_eq!(
            generalized.status,
            TassadarBroadInternalComputePublicationDecisionStatus::Suppressed
        );
    }

    #[test]
    fn broad_internal_compute_publication_gate_refuses_failed_profiles() {
        let decision = tassadar_broad_internal_compute_publication_decision(
            "tassadar.internal_compute.runtime_support_subset.v1",
        )
        .expect("runtime support subset decision");
        assert_eq!(
            decision.status,
            TassadarBroadInternalComputePublicationDecisionStatus::Suppressed
        );
        assert_eq!(
            decision.subset_profile_promotion_gate_report_ref,
            Some(String::from(
                "fixtures/tassadar/reports/tassadar_subset_profile_promotion_gate_report.json"
            ))
        );

        let error = require_tassadar_broad_internal_compute_profile_publication(
            "tassadar.internal_compute.runtime_support_subset.v1",
        )
        .expect_err("runtime support subset should still stay suppressed");
        assert!(matches!(
            error,
            TassadarBroadInternalComputePublicationDecisionError::Suppressed { .. }
        ));
    }
}
