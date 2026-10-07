use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_ir::TassadarModuleTrustPosture;

/// One candidate participating in overlapping-capability resolution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleOverlapCandidate {
    /// Stable module ref.
    pub module_ref: String,
    /// Capability label offered by the candidate.
    pub capability_label: String,
    /// Workload family supported by the candidate.
    pub workload_family: String,
    /// Typed trust posture.
    pub trust_posture: TassadarModuleTrustPosture,
    /// Count of benchmark refs backing the candidate.
    pub benchmark_ref_count: u32,
    /// Cost score in basis points. Lower is better.
    pub cost_score_bps: u16,
    /// Evidence score in basis points. Higher is better.
    pub evidence_score_bps: u16,
    /// Compatibility score in basis points. Higher is better.
    pub compatibility_score_bps: u16,
}

/// Explicit resolver policy for one overlapping-capability lookup.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleResolverPolicy {
    /// Stable policy identifier.
    pub policy_id: String,
    /// Capability label being resolved.
    pub capability_label: String,
    /// Workload family being resolved.
    pub workload_family: String,
    /// Minimum trust posture accepted by the caller.
    pub minimum_trust_posture: TassadarModuleTrustPosture,
    /// Minimum benchmark-ref count accepted by the caller.
    pub minimum_benchmark_ref_count: u32,
    /// Mount allowlist when one exists.
    pub allowed_module_refs: Vec<String>,
    /// Explicit preferred module refs in descending priority.
    pub preferred_module_refs: Vec<String>,
    /// Stable digest over the policy.
    pub policy_digest: String,
}

impl TassadarModuleResolverPolicy {
    /// Creates one deterministic resolver policy.
    #[must_use]
    pub fn new(
        policy_id: impl Into<String>,
        capability_label: impl Into<String>,
        workload_family: impl Into<String>,
        minimum_trust_posture: TassadarModuleTrustPosture,
        minimum_benchmark_ref_count: u32,
        mut allowed_module_refs: Vec<String>,
        mut preferred_module_refs: Vec<String>,
    ) -> Self {
        allowed_module_refs.sort();
        allowed_module_refs.dedup();
        preferred_module_refs.sort();
        preferred_module_refs.dedup();
        let mut policy = Self {
            policy_id: policy_id.into(),
            capability_label: capability_label.into(),
            workload_family: workload_family.into(),
            minimum_trust_posture,
            minimum_benchmark_ref_count,
            allowed_module_refs,
            preferred_module_refs,
            policy_digest: String::new(),
        };
        policy.policy_digest = stable_digest(b"psionic_tassadar_module_resolver_policy|", &policy);
        policy
    }
}

/// Deterministic selection produced from overlapping-capability resolution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleOverlapSelection {
    /// Selected module ref.
    pub module_ref: String,
    /// Stable policy digest that produced the selection.
    pub policy_digest: String,
    /// Plain-language detail.
    pub detail: String,
}

/// Failure returned by overlapping-capability resolution.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarModuleOverlapResolutionError {
    /// No candidate satisfied trust, benchmark, and mount constraints.
    #[error(
        "no overlapping-capability candidate satisfied policy `{policy_id}` for capability `{capability_label}`"
    )]
    NoEligibleCandidate {
        policy_id: String,
        capability_label: String,
    },
    /// More than one candidate remained tied after scoring.
    #[error("policy `{policy_id}` could not disambiguate overlapping candidates {module_refs:?}")]
    AmbiguousEligibleCandidate {
        policy_id: String,
        module_refs: Vec<String>,
    },
}

/// Resolves one deterministic overlapping-capability choice.
pub fn resolve_tassadar_module_overlap(
    candidates: &[TassadarModuleOverlapCandidate],
    policy: &TassadarModuleResolverPolicy,
) -> Result<TassadarModuleOverlapSelection, TassadarModuleOverlapResolutionError> {
    let eligible = candidates
        .iter()
        .filter(|candidate| {
            candidate.capability_label == policy.capability_label
                && candidate.workload_family == policy.workload_family
                && candidate.trust_posture >= policy.minimum_trust_posture
                && candidate.benchmark_ref_count >= policy.minimum_benchmark_ref_count
                && (policy.allowed_module_refs.is_empty()
                    || policy
                        .allowed_module_refs
                        .iter()
                        .any(|module_ref| module_ref == &candidate.module_ref))
        })
        .cloned()
        .collect::<Vec<_>>();
    if eligible.is_empty() {
        return Err(TassadarModuleOverlapResolutionError::NoEligibleCandidate {
            policy_id: policy.policy_id.clone(),
            capability_label: policy.capability_label.clone(),
        });
    }
    let mut scored = eligible
        .into_iter()
        .map(|candidate| {
            let preferred_rank = policy
                .preferred_module_refs
                .iter()
                .position(|module_ref| module_ref == &candidate.module_ref)
                .map(|index| u16::MAX - index as u16)
                .unwrap_or(0);
            let score = (
                preferred_rank,
                candidate.compatibility_score_bps,
                candidate.evidence_score_bps,
                candidate.trust_posture,
                u16::MAX - candidate.cost_score_bps,
            );
            (candidate, score)
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| right.1.cmp(&left.1));
    let best = scored.first().expect("non-empty");
    let tied = scored
        .iter()
        .filter(|candidate| candidate.1 == best.1)
        .map(|candidate| candidate.0.module_ref.clone())
        .collect::<Vec<_>>();
    if tied.len() > 1 {
        return Err(
            TassadarModuleOverlapResolutionError::AmbiguousEligibleCandidate {
                policy_id: policy.policy_id.clone(),
                module_refs: tied,
            },
        );
    }
    Ok(TassadarModuleOverlapSelection {
        module_ref: best.0.module_ref.clone(),
        policy_digest: policy.policy_digest.clone(),
        detail: format!(
            "policy `{}` selected `{}` using explicit preference, compatibility, evidence, trust, and cost ordering",
            policy.policy_id, best.0.module_ref,
        ),
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarModuleOverlapCandidate, TassadarModuleOverlapResolutionError,
        TassadarModuleResolverPolicy, resolve_tassadar_module_overlap,
    };
    use psionic_ir::TassadarModuleTrustPosture;

    #[test]
    fn module_overlap_resolution_prefers_higher_evidence_when_cost_is_close() {
        let candidates = vec![
            TassadarModuleOverlapCandidate {
                module_ref: String::from("candidate_select_core@1.1.0"),
                capability_label: String::from("bounded_search"),
                workload_family: String::from("verifier_search"),
                trust_posture: TassadarModuleTrustPosture::ChallengeGatedInstall,
                benchmark_ref_count: 2,
                cost_score_bps: 3600,
                evidence_score_bps: 9200,
                compatibility_score_bps: 9000,
            },
            TassadarModuleOverlapCandidate {
                module_ref: String::from("checkpoint_backtrack_core@1.0.0"),
                capability_label: String::from("bounded_search"),
                workload_family: String::from("verifier_search"),
                trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
                benchmark_ref_count: 2,
                cost_score_bps: 3400,
                evidence_score_bps: 8500,
                compatibility_score_bps: 8800,
            },
        ];
        let policy = TassadarModuleResolverPolicy::new(
            "policy.default",
            "bounded_search",
            "verifier_search",
            TassadarModuleTrustPosture::BenchmarkGatedInternal,
            2,
            vec![],
            vec![],
        );

        let selection = resolve_tassadar_module_overlap(&candidates, &policy).expect("selection");

        assert_eq!(selection.module_ref, "candidate_select_core@1.1.0");
    }

    #[test]
    fn module_overlap_resolution_refuses_score_ties() {
        let candidates = vec![
            TassadarModuleOverlapCandidate {
                module_ref: String::from("candidate_select_core@1.1.0"),
                capability_label: String::from("bounded_search"),
                workload_family: String::from("verifier_search"),
                trust_posture: TassadarModuleTrustPosture::ChallengeGatedInstall,
                benchmark_ref_count: 2,
                cost_score_bps: 3500,
                evidence_score_bps: 9000,
                compatibility_score_bps: 9000,
            },
            TassadarModuleOverlapCandidate {
                module_ref: String::from("checkpoint_backtrack_core@1.0.0"),
                capability_label: String::from("bounded_search"),
                workload_family: String::from("verifier_search"),
                trust_posture: TassadarModuleTrustPosture::ChallengeGatedInstall,
                benchmark_ref_count: 2,
                cost_score_bps: 3500,
                evidence_score_bps: 9000,
                compatibility_score_bps: 9000,
            },
        ];
        let policy = TassadarModuleResolverPolicy::new(
            "policy.tie",
            "bounded_search",
            "verifier_search",
            TassadarModuleTrustPosture::BenchmarkGatedInternal,
            2,
            vec![],
            vec![],
        );

        let error = resolve_tassadar_module_overlap(&candidates, &policy).expect_err("error");

        assert_eq!(
            error,
            TassadarModuleOverlapResolutionError::AmbiguousEligibleCandidate {
                policy_id: String::from("policy.tie"),
                module_refs: vec![
                    String::from("candidate_select_core@1.1.0"),
                    String::from("checkpoint_backtrack_core@1.0.0"),
                ],
            }
        );
    }
}
