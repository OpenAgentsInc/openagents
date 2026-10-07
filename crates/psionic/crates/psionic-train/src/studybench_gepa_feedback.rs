use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PSIONIC_STUDYBENCH_GEPA_FEEDBACK_SCHEMA_VERSION: &str =
    "psionic.studybench_gepa_feedback_refs.v1";
pub const OPENAGENTS_STUDYBENCH_PUBLIC_RETAINED_TARGET_SUITE_REF: &str =
    "target_suite.openagents_studybench.public_retained.v0";
pub const OPENAGENTS_STUDYBENCH_PRIVATE_VALIDATION_TARGET_SUITE_REF: &str =
    "target_suite.openagents_studybench.private_validation.v0";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicStudybenchGepaFeedbackRefs {
    pub schema_version: String,
    pub feedback_ref: String,
    pub task_id: String,
    pub candidate_hash: String,
    pub target_suite_refs: Vec<String>,
    pub failed_claim_refs: Vec<String>,
    pub missed_evidence_span_refs: Vec<String>,
    pub forbidden_claim_refs: Vec<String>,
    pub skipped_test_refs: Vec<String>,
    pub wrong_file_refs: Vec<String>,
    pub budget_failure_refs: Vec<String>,
    pub optimizer_acceptance_boundary_ref: String,
    pub raw_gold_answer_included: bool,
    pub raw_judge_rationale_included: bool,
    pub runtime_promotion_allowed: bool,
    pub public_claim_authority_allowed: bool,
    pub payout_authority_allowed: bool,
}

#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum PsionicStudybenchGepaFeedbackError {
    #[error("StudyBench GEPA feedback `{feedback_ref}` has invalid schema `{schema_version}`")]
    InvalidSchema {
        feedback_ref: String,
        schema_version: String,
    },
    #[error("StudyBench GEPA feedback `{feedback_ref}` is missing `{field}`")]
    MissingRequiredRef { feedback_ref: String, field: String },
    #[error(
        "StudyBench GEPA feedback `{feedback_ref}` contains raw or prose material in `{field}`"
    )]
    RawMaterial { feedback_ref: String, field: String },
    #[error(
        "StudyBench GEPA feedback `{feedback_ref}` is missing target suite `{target_suite_ref}`"
    )]
    MissingTargetSuite {
        feedback_ref: String,
        target_suite_ref: String,
    },
    #[error("StudyBench GEPA feedback `{feedback_ref}` overclaims authority in `{field}`")]
    AuthorityOverclaim { feedback_ref: String, field: String },
}

pub fn validate_psionic_studybench_gepa_feedback_refs(
    feedback: &PsionicStudybenchGepaFeedbackRefs,
) -> Result<(), PsionicStudybenchGepaFeedbackError> {
    if feedback.schema_version != PSIONIC_STUDYBENCH_GEPA_FEEDBACK_SCHEMA_VERSION {
        return Err(PsionicStudybenchGepaFeedbackError::InvalidSchema {
            feedback_ref: feedback.feedback_ref.clone(),
            schema_version: feedback.schema_version.clone(),
        });
    }

    require_ref(feedback, "feedback_ref", feedback.feedback_ref.as_str())?;
    require_ref(feedback, "task_id", feedback.task_id.as_str())?;
    require_ref(feedback, "candidate_hash", feedback.candidate_hash.as_str())?;
    require_ref(
        feedback,
        "optimizer_acceptance_boundary_ref",
        feedback.optimizer_acceptance_boundary_ref.as_str(),
    )?;
    require_ref_vec(feedback, "target_suite_refs", &feedback.target_suite_refs)?;
    require_ref_vec(feedback, "failed_claim_refs", &feedback.failed_claim_refs)?;

    for (field, refs) in [
        (
            "missed_evidence_span_refs",
            &feedback.missed_evidence_span_refs,
        ),
        ("forbidden_claim_refs", &feedback.forbidden_claim_refs),
        ("skipped_test_refs", &feedback.skipped_test_refs),
        ("wrong_file_refs", &feedback.wrong_file_refs),
        ("budget_failure_refs", &feedback.budget_failure_refs),
    ] {
        require_optional_ref_vec(feedback, field, refs)?;
    }

    for required_suite in [
        OPENAGENTS_STUDYBENCH_PUBLIC_RETAINED_TARGET_SUITE_REF,
        OPENAGENTS_STUDYBENCH_PRIVATE_VALIDATION_TARGET_SUITE_REF,
    ] {
        if !feedback
            .target_suite_refs
            .iter()
            .any(|suite| suite == required_suite)
        {
            return Err(PsionicStudybenchGepaFeedbackError::MissingTargetSuite {
                feedback_ref: feedback.feedback_ref.clone(),
                target_suite_ref: required_suite.to_string(),
            });
        }
    }

    if feedback.raw_gold_answer_included {
        return Err(PsionicStudybenchGepaFeedbackError::RawMaterial {
            feedback_ref: feedback.feedback_ref.clone(),
            field: "raw_gold_answer_included".to_string(),
        });
    }
    if feedback.raw_judge_rationale_included {
        return Err(PsionicStudybenchGepaFeedbackError::RawMaterial {
            feedback_ref: feedback.feedback_ref.clone(),
            field: "raw_judge_rationale_included".to_string(),
        });
    }
    for (field, allowed) in [
        (
            "runtime_promotion_allowed",
            feedback.runtime_promotion_allowed,
        ),
        (
            "public_claim_authority_allowed",
            feedback.public_claim_authority_allowed,
        ),
        (
            "payout_authority_allowed",
            feedback.payout_authority_allowed,
        ),
    ] {
        if allowed {
            return Err(PsionicStudybenchGepaFeedbackError::AuthorityOverclaim {
                feedback_ref: feedback.feedback_ref.clone(),
                field: field.to_string(),
            });
        }
    }

    Ok(())
}

pub fn canonical_openagents_studybench_gepa_feedback_refs() -> PsionicStudybenchGepaFeedbackRefs {
    PsionicStudybenchGepaFeedbackRefs {
        schema_version: PSIONIC_STUDYBENCH_GEPA_FEEDBACK_SCHEMA_VERSION.to_string(),
        feedback_ref: "gepa_feedback.openagents_studybench.openagents_launch_0001.candidate"
            .to_string(),
        task_id: "openagents_launch_0001".to_string(),
        candidate_hash: "sha256:candidate-studybench-gepa".to_string(),
        target_suite_refs: vec![
            OPENAGENTS_STUDYBENCH_PUBLIC_RETAINED_TARGET_SUITE_REF.to_string(),
            OPENAGENTS_STUDYBENCH_PRIVATE_VALIDATION_TARGET_SUITE_REF.to_string(),
        ],
        failed_claim_refs: vec![
            "gepa_feedback.openagents_studybench.openagents_launch_0001.claim.c1.core_failed"
                .to_string(),
        ],
        missed_evidence_span_refs: vec![
            "gepa_feedback.openagents_studybench.openagents_launch_0001.span.s1.missed".to_string(),
        ],
        forbidden_claim_refs: vec!["blocked_claim.runtime_promotion_from_gepa".to_string()],
        skipped_test_refs: vec![
            "test_command.probe.studybench.openagents_launch_0001.skipped".to_string(),
        ],
        wrong_file_refs: vec![
            "wrong_file.probe.studybench.openagents_launch_0001.apps_web".to_string(),
        ],
        budget_failure_refs: vec![
            "budget_failure.probe.studybench.openagents_launch_0001.tool_calls".to_string(),
        ],
        optimizer_acceptance_boundary_ref:
            "boundary.psionic.gepa.optimizer_acceptance_not_runtime_promotion.v0".to_string(),
        raw_gold_answer_included: false,
        raw_judge_rationale_included: false,
        runtime_promotion_allowed: false,
        public_claim_authority_allowed: false,
        payout_authority_allowed: false,
    }
}

fn require_ref(
    feedback: &PsionicStudybenchGepaFeedbackRefs,
    field: &str,
    value: &str,
) -> Result<(), PsionicStudybenchGepaFeedbackError> {
    if value.trim().is_empty() {
        return Err(PsionicStudybenchGepaFeedbackError::MissingRequiredRef {
            feedback_ref: feedback.feedback_ref.clone(),
            field: field.to_string(),
        });
    }
    if looks_like_raw_material(value) {
        return Err(PsionicStudybenchGepaFeedbackError::RawMaterial {
            feedback_ref: feedback.feedback_ref.clone(),
            field: field.to_string(),
        });
    }
    Ok(())
}

fn require_ref_vec(
    feedback: &PsionicStudybenchGepaFeedbackRefs,
    field: &str,
    refs: &[String],
) -> Result<(), PsionicStudybenchGepaFeedbackError> {
    if refs.is_empty() {
        return Err(PsionicStudybenchGepaFeedbackError::MissingRequiredRef {
            feedback_ref: feedback.feedback_ref.clone(),
            field: field.to_string(),
        });
    }
    require_optional_ref_vec(feedback, field, refs)
}

fn require_optional_ref_vec(
    feedback: &PsionicStudybenchGepaFeedbackRefs,
    field: &str,
    refs: &[String],
) -> Result<(), PsionicStudybenchGepaFeedbackError> {
    for value in refs {
        require_ref(feedback, field, value)?;
    }
    Ok(())
}

fn looks_like_raw_material(value: &str) -> bool {
    value.chars().any(char::is_whitespace)
        || value.contains("PRIVATE")
        || value.contains("because")
        || value.contains("critique")
        || value.contains("raw_judge")
        || value.contains("raw_gold")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn studybench_gepa_feedback_accepts_openagents_refs_only_projection() {
        let feedback = canonical_openagents_studybench_gepa_feedback_refs();

        validate_psionic_studybench_gepa_feedback_refs(&feedback).unwrap();

        assert!(feedback
            .target_suite_refs
            .contains(&OPENAGENTS_STUDYBENCH_PUBLIC_RETAINED_TARGET_SUITE_REF.to_string()));
        assert!(feedback
            .target_suite_refs
            .contains(&OPENAGENTS_STUDYBENCH_PRIVATE_VALIDATION_TARGET_SUITE_REF.to_string()));
        assert!(!feedback.runtime_promotion_allowed);
        assert!(!feedback.public_claim_authority_allowed);
        assert!(!feedback.payout_authority_allowed);
    }

    #[test]
    fn studybench_gepa_feedback_rejects_raw_private_or_judge_material() {
        let mut feedback = canonical_openagents_studybench_gepa_feedback_refs();
        feedback
            .failed_claim_refs
            .push("PRIVATE HOLDOUT GOLD ANSWER".to_string());

        assert!(matches!(
            validate_psionic_studybench_gepa_feedback_refs(&feedback),
            Err(PsionicStudybenchGepaFeedbackError::RawMaterial { field, .. })
                if field == "failed_claim_refs"
        ));

        let mut flagged = canonical_openagents_studybench_gepa_feedback_refs();
        flagged.raw_judge_rationale_included = true;
        assert!(matches!(
            validate_psionic_studybench_gepa_feedback_refs(&flagged),
            Err(PsionicStudybenchGepaFeedbackError::RawMaterial { field, .. })
                if field == "raw_judge_rationale_included"
        ));
    }

    #[test]
    fn studybench_gepa_feedback_rejects_authority_overclaims() {
        let mut feedback = canonical_openagents_studybench_gepa_feedback_refs();
        feedback.runtime_promotion_allowed = true;

        assert!(matches!(
            validate_psionic_studybench_gepa_feedback_refs(&feedback),
            Err(PsionicStudybenchGepaFeedbackError::AuthorityOverclaim { field, .. })
                if field == "runtime_promotion_allowed"
        ));
    }

    #[test]
    fn studybench_gepa_feedback_requires_public_and_private_validation_targets() {
        let mut feedback = canonical_openagents_studybench_gepa_feedback_refs();
        feedback
            .target_suite_refs
            .retain(|suite| suite != OPENAGENTS_STUDYBENCH_PRIVATE_VALIDATION_TARGET_SUITE_REF);

        assert!(matches!(
            validate_psionic_studybench_gepa_feedback_refs(&feedback),
            Err(PsionicStudybenchGepaFeedbackError::MissingTargetSuite { target_suite_ref, .. })
                if target_suite_ref == OPENAGENTS_STUDYBENCH_PRIVATE_VALIDATION_TARGET_SUITE_REF
        ));
    }
}
