//! Coordinator candidate-artifact emission (Khala M6, roadmap Phase 4).
//!
//! This module lands the **candidate-artifact emission** slice the
//! `docs/KHALA_M6_M7_COORDINATOR_PLAN.md` plan explicitly deferred to a reviewed
//! change because it touches an existing runtime contract: a trained
//! [`CoordinatorHead`] ships as a **digest-pinned Candidate** under the existing
//! [`CompiledAgentPromotedArtifactContract`] (`AdapterArtifactKind::CoordinatorHead`
//! packaging), carrying the **heuristic router as `rollback_artifact_id`**, and
//! the promote / hold / rollback decision is driven by [`ShadowComparison`] (the
//! M6 "verified-work-per-sat" gate).
//!
//! ## What it does / does not do
//!
//! - It **emits** a typed candidate entry plus a [`ShadowRecommendation`]-derived
//!   promotion decision. The candidate is digest-pinned over the head's config
//!   and flat-parameter vector so the same trained head always produces the same
//!   `artifact_id` / `artifact_digest`.
//! - The heuristic router is recorded as `rollback_artifact_id` on the candidate
//!   so a rollback target always exists.
//! - Promotion stays an **approval-gated `runtime_promotion`** (same governance
//!   as Artanis authority): a candidate is *eligible* only when
//!   [`ShadowComparison`] returns [`ShadowRecommendation::PromoteCandidate`]
//!   (strict verified-work-per-sat win in the High band). This module **never
//!   auto-promotes** — it emits the Candidate (lifecycle `Candidate`) and the
//!   decision; a separate governance step does the actual promotion.
//! - It **moves no sats, dispatches no work, starts no training.** It is a pure
//!   function over an already-trained head and an already-computed shadow
//!   comparison.
//!
//! ## Remaining dependency (honest)
//!
//! The *full paid shadow-win* — a real verified-work-per-sat win on the
//! in-distribution task set — is still blocked on M4's real Pylon pool plus a
//! reachable `EvalVerdictSource` (the Tassadar verdict over buy-mode eval jobs).
//! Until then the shadow comparison runs on the offline / simulated lane (zero
//! spend), so an emitted candidate is offline-validated, not paid-promoted. This
//! module does not build that paid lane; it consumes whatever
//! [`ShadowComparison`] it is handed.

use psionic_adapters::AdapterArtifactKind;
use psionic_models::{CoordinatorHead, CoordinatorHeadConfig, CoordinatorHeadError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::compiled_agent_artifact_contract::{
    CompiledAgentArtifactContractEntry, CompiledAgentArtifactLifecycleState,
    CompiledAgentArtifactPayload, CompiledAgentArtifactValidatorLineage,
};
use crate::coordinator_shadow_comparison::{
    ShadowComparison, ShadowConfidenceBand, ShadowRecommendation,
};

use psionic_eval::{
    canonical_compiled_agent_default_row_contract, CompiledAgentDefaultLearnedRowContract,
    CompiledAgentEvidenceClass, CompiledAgentModuleKind,
};

/// Compatibility version shared with the compiled-agent first-graph contract.
const COORDINATOR_COMPATIBILITY_VERSION: &str = "openagents.compiled_agent.first_graph.v1";
/// Stable label for the sep-CMA-ES learned coordinator candidate.
pub const COORDINATOR_CANDIDATE_LABEL: &str = "coordinator_sep_cmaes_v1";
/// Adapter family the head is packaged under (`AdapterArtifactKind::CoordinatorHead`).
const COORDINATOR_IMPLEMENTATION_FAMILY: &str = "psionic_coordinator_head";
/// Logical module name for the coordinator surface in the contract.
const COORDINATOR_MODULE_NAME: &str = "coordinator_route";

/// Errors raised while emitting a coordinator candidate artifact.
#[derive(Debug, Error)]
pub enum CoordinatorCandidateEmissionError {
    /// The trained head's flat parameters could not be read.
    #[error("coordinator head error: {0}")]
    Head(#[from] CoordinatorHeadError),
    /// The supplied heuristic-router rollback id was empty.
    #[error("heuristic rollback artifact id must be non-empty")]
    EmptyRollbackId,
}

/// The digest-pinned identity of a trained coordinator head, as packaged for the
/// promoted-artifact contract. The backbone weights are *not* part of this
/// payload — only the head config and the flat parameter digest, matching
/// `AdapterArtifactKind::CoordinatorHead` packaging (the head sits on top of the
/// frozen backbone hidden state).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordinatorHeadCandidateArtifact {
    /// Adapter kind — always [`AdapterArtifactKind::CoordinatorHead`].
    pub adapter_kind: AdapterArtifactKind,
    /// The head's static configuration (`hidden_dim`, `num_workers`,
    /// `num_roles`).
    pub head_config: CoordinatorHeadConfig,
    /// Number of learnable head parameters (`hidden_dim * (num_workers +
    /// num_roles)`), recorded so a consumer can sanity-check the digest target.
    pub parameter_count: usize,
    /// SHA-256 digest over the head config and the flat parameter vector. Two
    /// trained heads with identical config and parameters digest-pin to the same
    /// id; any parameter change changes the digest.
    pub parameter_digest: String,
}

impl CoordinatorHeadCandidateArtifact {
    /// Builds the digest-pinned identity from a trained head.
    pub fn from_trained_head(head: &CoordinatorHead) -> Result<Self, CoordinatorHeadError> {
        let config = head.config();
        let params = head.flatten_parameters()?;
        let parameter_digest = digest_head(&config, &params);
        Ok(Self {
            adapter_kind: AdapterArtifactKind::CoordinatorHead,
            head_config: config,
            parameter_count: params.len(),
            parameter_digest,
        })
    }

    /// The stable artifact id for this candidate. Digest-pinned: derived from the
    /// candidate label and the parameter digest so the same trained head always
    /// resolves to the same id.
    #[must_use]
    pub fn artifact_id(&self) -> String {
        format!(
            "compiled_agent.coordinator.sep_cmaes_v1.{}",
            short_digest(&self.parameter_digest)
        )
    }
}

/// SHA-256 over the head config and flat parameters (stable, order-fixed).
fn digest_head(config: &CoordinatorHeadConfig, params: &[f32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_coordinator_head|");
    hasher.update(config.hidden_dim.to_le_bytes());
    hasher.update(config.num_workers.to_le_bytes());
    hasher.update(config.num_roles.to_le_bytes());
    hasher.update(b"|params|");
    for value in params {
        hasher.update(value.to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

fn short_digest(digest: &str) -> &str {
    &digest[..digest.len().min(16)]
}

/// The promotion decision a candidate emission carries. It mirrors the
/// [`ShadowRecommendation`] but is named for the *governance* action: a
/// candidate is only ever **eligible** for an approval-gated `runtime_promotion`;
/// it is never automatically promoted by this module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinatorPromotionDecision {
    /// Eligible for an approval-gated `runtime_promotion` (strict
    /// verified-work-per-sat win in the High band). NOT auto-promoted.
    PromotionEligible,
    /// Keep shadowing; not yet a clean high-confidence win.
    HoldShadow,
    /// Learned head regressed against the heuristic baseline — roll back to the
    /// `rollback_artifact_id` heuristic router.
    Rollback,
}

impl CoordinatorPromotionDecision {
    fn from_recommendation(recommendation: ShadowRecommendation) -> Self {
        match recommendation {
            ShadowRecommendation::PromoteCandidate => Self::PromotionEligible,
            ShadowRecommendation::HoldShadow => Self::HoldShadow,
            ShadowRecommendation::Rollback => Self::Rollback,
        }
    }

    /// Whether the candidate is eligible for an approval-gated promotion. This is
    /// eligibility only — the actual promotion is a separate governance step.
    #[must_use]
    pub fn is_promotion_eligible(self) -> bool {
        matches!(self, Self::PromotionEligible)
    }

    /// Whether the comparison recommends rolling back to the heuristic router.
    #[must_use]
    pub fn is_rollback(self) -> bool {
        matches!(self, Self::Rollback)
    }
}

/// The emitted coordinator candidate: a digest-pinned candidate contract entry,
/// the heuristic-router rollback id it carries, the [`ShadowComparison`] that
/// gated it, and the resulting (approval-gated, never automatic) promotion
/// decision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoordinatorCandidateEmission {
    /// The digest-pinned candidate entry (lifecycle `Candidate`).
    pub entry: CompiledAgentArtifactContractEntry,
    /// The heuristic-router artifact id recorded as the candidate's rollback
    /// target (`entry.rollback_artifact_id`).
    pub heuristic_rollback_artifact_id: String,
    /// The shadow comparison that gated the decision.
    pub shadow: ShadowComparison,
    /// The approval-gated promotion decision (never auto-promotes).
    pub decision: CoordinatorPromotionDecision,
    /// Human-readable summary for receipts and logs.
    pub summary: String,
}

impl CoordinatorCandidateEmission {
    /// Emits a trained coordinator head as a digest-pinned shadow Candidate.
    ///
    /// - `trained_head` is the optimized [`CoordinatorHead`] (config + trained
    ///   flat parameters).
    /// - `heuristic_rollback_artifact_id` is the promoted heuristic router that
    ///   becomes the candidate's `rollback_artifact_id`.
    /// - `shadow` is the [`ShadowComparison`] over the same in-distribution
    ///   samples; it drives the promote / hold / rollback decision.
    /// - `validator_lineage` ties the candidate to its validator report (the
    ///   replay-validator verdict source), exactly as the existing
    ///   compiled-agent candidates do.
    ///
    /// The candidate is always emitted with lifecycle `Candidate`. This function
    /// never promotes: the [`CoordinatorPromotionDecision`] is *eligibility*
    /// only, consumed by the approval-gated `runtime_promotion` governance step.
    pub fn emit(
        trained_head: &CoordinatorHead,
        heuristic_rollback_artifact_id: impl Into<String>,
        shadow: ShadowComparison,
        validator_lineage: CompiledAgentArtifactValidatorLineage,
        evidence_class: CompiledAgentEvidenceClass,
    ) -> Result<Self, CoordinatorCandidateEmissionError> {
        let heuristic_rollback_artifact_id = heuristic_rollback_artifact_id.into();
        if heuristic_rollback_artifact_id.trim().is_empty() {
            return Err(CoordinatorCandidateEmissionError::EmptyRollbackId);
        }

        let artifact = CoordinatorHeadCandidateArtifact::from_trained_head(trained_head)?;
        let artifact_id = artifact.artifact_id();
        let artifact_digest = artifact.parameter_digest.clone();
        let default_row: CompiledAgentDefaultLearnedRowContract =
            canonical_compiled_agent_default_row_contract();

        let decision = CoordinatorPromotionDecision::from_recommendation(shadow.recommendation);

        // The candidate's confidence floor mirrors the High-band governance
        // floor; the actual band the shadow landed in is reported in `detail`.
        let confidence_floor = crate::coordinator_shadow_comparison::DEFAULT_HIGH_CONFIDENCE_MIN;

        let detail = format!(
            "Learned coordinator head ({} params, {:?}) emitted as digest-pinned shadow candidate `{}` \
             with heuristic router `{}` as rollback. ShadowComparison: {} -> decision {:?}. \
             Promotion stays an approval-gated runtime_promotion (Artanis-grade governance); nothing auto-promotes. \
             Full paid shadow-win remains blocked on M4 real pool + a reachable EvalVerdictSource.",
            artifact.parameter_count,
            artifact.head_config,
            COORDINATOR_CANDIDATE_LABEL,
            heuristic_rollback_artifact_id,
            shadow.summary,
            decision,
        );

        // The module key reuses `Route` because the coordinator *is* the routing
        // surface in the first graph, but the candidate is distinguished by its
        // `coordinator_sep_cmaes_v1` label, the `psionic_coordinator_head`
        // implementation family, and the new `CoordinatorHead` payload — so it
        // never collides with the existing route revision/route-model candidates.
        let entry = CompiledAgentArtifactContractEntry {
            module: CompiledAgentModuleKind::Route,
            module_name: String::from(COORDINATOR_MODULE_NAME),
            signature_name: String::from(COORDINATOR_MODULE_NAME),
            implementation_family: String::from(COORDINATOR_IMPLEMENTATION_FAMILY),
            implementation_label: artifact_id.clone(),
            version: String::from("2026-06-22"),
            lifecycle_state: CompiledAgentArtifactLifecycleState::Candidate,
            candidate_label: Some(String::from(COORDINATOR_CANDIDATE_LABEL)),
            compatibility_version: String::from(COORDINATOR_COMPATIBILITY_VERSION),
            confidence_floor,
            artifact_id,
            artifact_digest,
            row_id: default_row.row_id.clone(),
            default_row,
            evidence_class,
            validator_lineage,
            predecessor_artifact_id: Some(heuristic_rollback_artifact_id.clone()),
            rollback_artifact_id: Some(heuristic_rollback_artifact_id.clone()),
            // A candidate has never been promoted, so it has no promotion time.
            promoted_at_utc: None,
            payload: CompiledAgentArtifactPayload::CoordinatorHead { artifact },
            detail,
        };

        let summary = format!(
            "coordinator candidate `{}` (id {}) emitted as shadow Candidate; rollback -> `{}`; \
             shadow band {:?}, decision {:?} (approval-gated, never automatic)",
            COORDINATOR_CANDIDATE_LABEL,
            entry.artifact_id,
            heuristic_rollback_artifact_id,
            shadow.band,
            decision,
        );

        Ok(Self {
            entry,
            heuristic_rollback_artifact_id,
            shadow,
            decision,
            summary,
        })
    }

    /// Whether the emitted candidate is eligible for an approval-gated promotion.
    /// Eligibility only — promotion is a separate governance step.
    #[must_use]
    pub fn is_promotion_eligible(&self) -> bool {
        self.decision.is_promotion_eligible()
    }

    /// The confidence band the gating shadow comparison landed in.
    #[must_use]
    pub fn band(&self) -> ShadowConfidenceBand {
        self.shadow.band
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinator_evolution::{TrajectoryOutcome, VerificationVerdict};
    use psionic_models::CoordinatorHeadConfig;

    fn head_config() -> CoordinatorHeadConfig {
        CoordinatorHeadConfig {
            hidden_dim: 8,
            num_workers: 3,
            num_roles: 3,
        }
    }

    fn trained_head(scale: f32) -> CoordinatorHead {
        let config = head_config();
        let params: Vec<f32> = (0..config.parameter_count())
            .map(|i| (i as f32) * 0.01 * scale)
            .collect();
        CoordinatorHead::from_flat_weights(config, params).expect("head")
    }

    fn lineage() -> CompiledAgentArtifactValidatorLineage {
        CompiledAgentArtifactValidatorLineage {
            validator_report_ref: "fixtures/coordinator/shadow_report_v1.json".to_string(),
            validator_report_digest: "deadbeef".to_string(),
            xtrain_cycle_receipt_ref: None,
            xtrain_cycle_receipt_digest: None,
        }
    }

    fn verified_at(cost: f32) -> TrajectoryOutcome {
        TrajectoryOutcome {
            verdict: VerificationVerdict::Verified,
            cost,
        }
    }
    fn rejected_at(cost: f32) -> TrajectoryOutcome {
        TrajectoryOutcome {
            verdict: VerificationVerdict::Rejected,
            cost,
        }
    }

    /// A clean high-confidence per-sat win shadow comparison.
    fn promote_shadow() -> ShadowComparison {
        // Learned 9/10 verified @10 sats (100); heuristic 8/10 @20 sats (200).
        let mut learned = vec![verified_at(10.0); 9];
        learned.push(rejected_at(10.0));
        let mut heuristic = vec![verified_at(20.0); 8];
        heuristic.push(rejected_at(20.0));
        heuristic.push(rejected_at(20.0));
        ShadowComparison::compare(&learned, &heuristic)
    }

    fn hold_shadow() -> ShadowComparison {
        // Watch band (0.70 verified rate) win -> hold, not promote.
        let mut learned = vec![verified_at(10.0); 7];
        learned.extend(vec![rejected_at(10.0); 3]);
        let mut heuristic = vec![verified_at(40.0); 6];
        heuristic.extend(vec![rejected_at(40.0); 4]);
        ShadowComparison::compare(&learned, &heuristic)
    }

    fn rollback_shadow() -> ShadowComparison {
        // Learned regressed against heuristic -> rollback.
        let learned = vec![verified_at(50.0), rejected_at(50.0)];
        let heuristic = vec![verified_at(10.0), verified_at(10.0)];
        ShadowComparison::compare(&learned, &heuristic)
    }

    #[test]
    fn trained_head_emits_digest_pinned_candidate_with_heuristic_rollback() {
        let head = trained_head(1.0);
        let emission = CoordinatorCandidateEmission::emit(
            &head,
            "compiled_agent.baseline.rule_v1.coordinator_route",
            promote_shadow(),
            lineage(),
            CompiledAgentEvidenceClass::LearnedLane,
        )
        .expect("emit");

        // It is a Candidate, never a Promoted entry.
        assert_eq!(
            emission.entry.lifecycle_state,
            CompiledAgentArtifactLifecycleState::Candidate
        );
        assert_eq!(
            emission.entry.candidate_label.as_deref(),
            Some(COORDINATOR_CANDIDATE_LABEL)
        );
        assert!(emission.entry.promoted_at_utc.is_none());

        // The heuristic router is the rollback target.
        assert_eq!(
            emission.entry.rollback_artifact_id.as_deref(),
            Some("compiled_agent.baseline.rule_v1.coordinator_route")
        );
        assert_eq!(
            emission.heuristic_rollback_artifact_id,
            "compiled_agent.baseline.rule_v1.coordinator_route"
        );

        // It carries the digest-pinned coordinator-head payload.
        match &emission.entry.payload {
            CompiledAgentArtifactPayload::CoordinatorHead { artifact } => {
                assert_eq!(artifact.adapter_kind, AdapterArtifactKind::CoordinatorHead);
                assert_eq!(artifact.parameter_count, head_config().parameter_count());
                assert_eq!(artifact.parameter_digest, emission.entry.artifact_digest);
            }
            other => panic!("expected coordinator-head payload, got {other:?}"),
        }
    }

    #[test]
    fn digest_pinning_is_deterministic_and_parameter_sensitive() {
        let a = CoordinatorHeadCandidateArtifact::from_trained_head(&trained_head(1.0)).expect("a");
        let a2 =
            CoordinatorHeadCandidateArtifact::from_trained_head(&trained_head(1.0)).expect("a2");
        let b = CoordinatorHeadCandidateArtifact::from_trained_head(&trained_head(2.0)).expect("b");
        // Same trained head -> same digest & id (digest-pinned).
        assert_eq!(a.parameter_digest, a2.parameter_digest);
        assert_eq!(a.artifact_id(), a2.artifact_id());
        // Different parameters -> different digest & id.
        assert_ne!(a.parameter_digest, b.parameter_digest);
        assert_ne!(a.artifact_id(), b.artifact_id());
    }

    #[test]
    fn shadow_promote_gates_promotion_eligible_but_never_auto_promotes() {
        let emission = CoordinatorCandidateEmission::emit(
            &trained_head(1.0),
            "heuristic_router_v1",
            promote_shadow(),
            lineage(),
            CompiledAgentEvidenceClass::LearnedLane,
        )
        .expect("emit");
        assert_eq!(
            emission.decision,
            CoordinatorPromotionDecision::PromotionEligible
        );
        assert!(emission.is_promotion_eligible());
        // Eligible, but still emitted ONLY as a Candidate — nothing auto-promotes.
        assert_eq!(
            emission.entry.lifecycle_state,
            CompiledAgentArtifactLifecycleState::Candidate
        );
        assert!(emission.entry.promoted_at_utc.is_none());
        assert_eq!(emission.band(), ShadowConfidenceBand::High);
    }

    #[test]
    fn shadow_hold_gates_hold_not_eligible() {
        let emission = CoordinatorCandidateEmission::emit(
            &trained_head(1.0),
            "heuristic_router_v1",
            hold_shadow(),
            lineage(),
            CompiledAgentEvidenceClass::LearnedLane,
        )
        .expect("emit");
        assert_eq!(emission.decision, CoordinatorPromotionDecision::HoldShadow);
        assert!(!emission.is_promotion_eligible());
        assert_eq!(emission.band(), ShadowConfidenceBand::Watch);
    }

    #[test]
    fn shadow_regression_gates_rollback() {
        let emission = CoordinatorCandidateEmission::emit(
            &trained_head(1.0),
            "heuristic_router_v1",
            rollback_shadow(),
            lineage(),
            CompiledAgentEvidenceClass::LearnedLane,
        )
        .expect("emit");
        assert_eq!(emission.decision, CoordinatorPromotionDecision::Rollback);
        assert!(emission.decision.is_rollback());
        assert!(!emission.is_promotion_eligible());
        // Even on a rollback recommendation the rollback target is the heuristic.
        assert_eq!(
            emission.entry.rollback_artifact_id.as_deref(),
            Some("heuristic_router_v1")
        );
    }

    #[test]
    fn empty_rollback_id_is_rejected() {
        let err = CoordinatorCandidateEmission::emit(
            &trained_head(1.0),
            "   ",
            promote_shadow(),
            lineage(),
            CompiledAgentEvidenceClass::LearnedLane,
        )
        .expect_err("must reject empty rollback id");
        assert!(matches!(
            err,
            CoordinatorCandidateEmissionError::EmptyRollbackId
        ));
    }
}
