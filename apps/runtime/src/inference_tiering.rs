use protocol::jobs;
use protocol::verification::{AdjudicationStrategy, Verification, VerificationMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferenceRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferenceCostHint {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceTierDecision {
    pub verification: Verification,
    pub reason_codes: Vec<&'static str>,
}

/// Choose a verification strategy for subjective inference based on (risk, cost).
///
/// This is a policy engine, not a protocol contract. It intentionally does not mutate authority
/// state; it just produces deterministic routing decisions to be consumed by higher-level
/// orchestration.
pub fn choose_subjective_verification(
    job_type: &str,
    risk: InferenceRisk,
    cost: InferenceCostHint,
) -> InferenceTierDecision {
    let mut reason_codes = Vec::new();
    let base = base_verification_for_job(job_type);

    if base.mode == VerificationMode::Objective {
        reason_codes.push("objective_passthrough");
        return InferenceTierDecision {
            verification: base,
            reason_codes,
        };
    }

    let base_redundancy = base.redundancy.max(1);
    let min_redundancy = match risk {
        InferenceRisk::Low => base_redundancy,
        InferenceRisk::Medium => base_redundancy.max(2),
        InferenceRisk::High => base_redundancy.max(3),
    };

    let cap_redundancy = match cost {
        InferenceCostHint::Low => 5,
        InferenceCostHint::Medium => 3,
        InferenceCostHint::High => 2,
    };

    let mut redundancy = min_redundancy;
    if redundancy <= cap_redundancy {
        redundancy = redundancy.min(cap_redundancy);
    } else {
        reason_codes.push("cost_cap_ignored_due_to_risk");
    }
    if redundancy > 5 {
        redundancy = 5;
        reason_codes.push("redundancy_capped_at_5");
    }

    let mut adjudication = base.adjudication;
    match risk {
        InferenceRisk::High => {
            if adjudication != AdjudicationStrategy::JudgeModel {
                reason_codes.push("risk_high_forces_judge");
            }
            adjudication = AdjudicationStrategy::JudgeModel;
        }
        InferenceRisk::Low | InferenceRisk::Medium => {
            if cost == InferenceCostHint::High && redundancy > 1 {
                if adjudication != AdjudicationStrategy::MajorityVote {
                    reason_codes.push("cost_high_prefers_majority");
                }
                adjudication = AdjudicationStrategy::MajorityVote;
            }
        }
    }

    if redundancy <= 1 {
        adjudication = AdjudicationStrategy::None;
    } else if adjudication == AdjudicationStrategy::None {
        adjudication = AdjudicationStrategy::MajorityVote;
        reason_codes.push("adjudication_defaulted_to_majority");
    }

    InferenceTierDecision {
        verification: Verification {
            mode: VerificationMode::Subjective,
            redundancy,
            adjudication,
            judge_model: base.judge_model.clone(),
        },
        reason_codes,
    }
}

fn base_verification_for_job(job_type: &str) -> Verification {
    jobs::registered_job_types()
        .into_iter()
        .find(|info| info.job_type == job_type)
        .map(|info| info.default_verification)
        .unwrap_or_else(|| Verification::subjective_with_majority(2))
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::JobRequest;

    #[test]
    fn objective_jobs_are_passthrough() {
        let decision = choose_subjective_verification(
            protocol::SandboxRunRequest::JOB_TYPE,
            InferenceRisk::High,
            InferenceCostHint::Low,
        );
        assert_eq!(decision.verification.mode, VerificationMode::Objective);
        assert_eq!(decision.reason_codes, vec!["objective_passthrough"]);
    }

    #[test]
    fn chunk_analysis_high_risk_forces_redundancy_and_judge() {
        let decision = choose_subjective_verification(
            protocol::ChunkAnalysisRequest::JOB_TYPE,
            InferenceRisk::High,
            InferenceCostHint::Medium,
        );
        assert_eq!(decision.verification.mode, VerificationMode::Subjective);
        assert_eq!(decision.verification.redundancy, 3);
        assert_eq!(
            decision.verification.adjudication,
            AdjudicationStrategy::JudgeModel
        );
    }

    #[test]
    fn embeddings_medium_risk_lifts_single_provider_to_redundant() {
        let decision = choose_subjective_verification(
            protocol::jobs::embeddings::EmbeddingsRequest::JOB_TYPE,
            InferenceRisk::Medium,
            InferenceCostHint::Low,
        );
        assert_eq!(decision.verification.mode, VerificationMode::Subjective);
        assert_eq!(decision.verification.redundancy, 2);
        assert_eq!(
            decision.verification.adjudication,
            AdjudicationStrategy::MajorityVote
        );
    }

    #[test]
    fn cost_high_prefers_majority_when_not_high_risk() {
        let decision = choose_subjective_verification(
            protocol::ChunkAnalysisRequest::JOB_TYPE,
            InferenceRisk::Medium,
            InferenceCostHint::High,
        );
        assert_eq!(decision.verification.mode, VerificationMode::Subjective);
        assert_eq!(decision.verification.redundancy, 2);
        assert_eq!(
            decision.verification.adjudication,
            AdjudicationStrategy::MajorityVote
        );
        assert!(
            decision
                .reason_codes
                .contains(&"cost_high_prefers_majority")
        );
    }
}
