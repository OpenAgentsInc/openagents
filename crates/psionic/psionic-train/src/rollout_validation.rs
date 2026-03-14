use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    RolloutArtifact, RolloutProofKind, RolloutReceiptOutcome, RolloutWorkerOutcomeKind,
    RolloutWorkerOutcomeReceipt,
};

/// Policy surface for rollout validation and sampled adjudication.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutValidatorPolicy {
    /// Stable validator policy identifier.
    pub policy_id: String,
    /// Whether at least one execution proof is required.
    pub require_execution_proof: bool,
    /// Deterministic sample rate for expensive non-benchmark checks.
    pub sampled_expensive_check_bps: u16,
    /// Deterministic sample rate for benchmark-class checks.
    pub benchmark_check_sample_bps: u16,
    /// Weight applied when duplicates are normalized instead of rejected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalize_duplicate_weight_bps: Option<u16>,
}

impl Default for RolloutValidatorPolicy {
    fn default() -> Self {
        Self {
            policy_id: String::from("validator-policy-default"),
            require_execution_proof: true,
            sampled_expensive_check_bps: 1_000,
            benchmark_check_sample_bps: 10_000,
            normalize_duplicate_weight_bps: Some(5_000),
        }
    }
}

/// Benchmark observation captured alongside a rollout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutBenchmarkObservation {
    /// Observed runtime for timer-integrity checks.
    pub observed_runtime_ms: u64,
    /// Observed token count for token-accounting checks.
    pub observed_token_count: u64,
    /// Observed final-state digest when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_final_state_digest: Option<String>,
    /// Declared execution strategy such as `dp` or `single_node`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_execution_strategy: Option<String>,
}

/// Benchmark expectations imposed by validator policy or packaged environments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutBenchmarkExpectation {
    /// Minimum acceptable runtime.
    pub min_runtime_ms: u64,
    /// Maximum acceptable runtime.
    pub max_runtime_ms: u64,
    /// Expected token count.
    pub expected_token_count: u64,
    /// Expected final-state digest when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_final_state_digest: Option<String>,
    /// Expected execution strategy when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_execution_strategy: Option<String>,
}

/// One validator-ready rollout bundle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RolloutVerificationBundle {
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Rollout artifact under validation.
    pub artifact: RolloutArtifact,
    /// Worker outcome that produced or rejected the rollout.
    pub worker_outcome: RolloutWorkerOutcomeReceipt,
    /// Benchmark observation when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark_observation: Option<RolloutBenchmarkObservation>,
    /// Benchmark expectation when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark_expectation: Option<RolloutBenchmarkExpectation>,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

impl RolloutVerificationBundle {
    /// Creates a rollout-verification bundle.
    #[must_use]
    pub fn new(
        bundle_id: impl Into<String>,
        artifact: RolloutArtifact,
        worker_outcome: RolloutWorkerOutcomeReceipt,
        benchmark_observation: Option<RolloutBenchmarkObservation>,
        benchmark_expectation: Option<RolloutBenchmarkExpectation>,
    ) -> Self {
        let bundle_id = bundle_id.into();
        let bundle_digest = stable_rollout_verification_bundle_digest(
            bundle_id.as_str(),
            &artifact,
            &worker_outcome,
            benchmark_observation.as_ref(),
            benchmark_expectation.as_ref(),
        );
        Self {
            bundle_id,
            artifact,
            worker_outcome,
            benchmark_observation,
            benchmark_expectation,
            bundle_digest,
        }
    }
}

/// Reason-code family emitted by one validator verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidatorReasonCode {
    /// Execution proof bundle was missing.
    ExecutionProofMissing,
    /// Worker or orchestrator outcome already marked the rollout stale.
    StalePolicyRejected,
    /// Exact artifact digest was already validated previously.
    ReplayedArtifactDetected,
    /// Response signature matched a different artifact closely enough to count as duplicate.
    DuplicateDetected,
    /// Contribution was retained only under normalization or deweighting.
    ContributionNormalized,
    /// Runtime fell outside the allowed range.
    TimerIntegrityMismatch,
    /// Observed token count did not match the expected count.
    TokenAccountingMismatch,
    /// Final-state digest did not match the benchmark expectation.
    FinalStateMismatch,
    /// Declared execution strategy did not match benchmark expectation.
    ExecutionStrategyMismatch,
}

/// Final verdict posture for one rollout bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidatorDisposition {
    /// The rollout passed available checks.
    Accepted,
    /// The rollout is retained but deweighted.
    Normalized,
    /// The rollout is rejected.
    Rejected,
}

/// Normalization or deweighting applied to one contribution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContributionNormalization {
    /// Weight retained after normalization.
    pub normalized_weight_bps: u16,
    /// Artifact digests that triggered normalization.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub referenced_artifact_digests: Vec<String>,
}

/// Final validator verdict over one rollout bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorVerdict {
    /// Stable verdict identifier.
    pub verdict_id: String,
    /// Validator policy that produced this verdict.
    pub validator_policy_id: String,
    /// Artifact digest under validation.
    pub artifact_digest: String,
    /// Worker identifier under validation.
    pub worker_id: String,
    /// Final disposition.
    pub disposition: ValidatorDisposition,
    /// Whether expensive sampled checks were run.
    pub ran_expensive_checks: bool,
    /// Whether benchmark checks were run.
    pub ran_benchmark_checks: bool,
    /// Machine-readable reason codes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason_codes: Vec<ValidatorReasonCode>,
    /// Referenced artifact digests for replay or duplicate review.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub referenced_artifact_digests: Vec<String>,
    /// Optional normalization outcome.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalization: Option<ContributionNormalization>,
    /// Stable verdict digest.
    pub verdict_digest: String,
}

/// One seen response signature used for duplicate detection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutResponseSignatureRecord {
    /// Stable response signature digest.
    pub response_signature_digest: String,
    /// Artifact digest that first produced this signature.
    pub artifact_digest: String,
    /// Worker id that first produced this signature.
    pub worker_id: String,
}

/// Stateful rollout validator over replayed bundles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RolloutValidatorState {
    /// Active validator policy.
    pub policy: RolloutValidatorPolicy,
    /// Previously seen artifact digests.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seen_artifact_digests: Vec<String>,
    /// Previously seen response signatures.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seen_response_signatures: Vec<RolloutResponseSignatureRecord>,
    /// Emitted verdict history.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verdicts: Vec<ValidatorVerdict>,
}

impl RolloutValidatorState {
    /// Creates a rollout validator with the given policy.
    #[must_use]
    pub const fn new(policy: RolloutValidatorPolicy) -> Self {
        Self {
            policy,
            seen_artifact_digests: Vec::new(),
            seen_response_signatures: Vec::new(),
            verdicts: Vec::new(),
        }
    }

    /// Verifies one rollout bundle and updates duplicate or replay state.
    pub fn verify_bundle(&mut self, bundle: RolloutVerificationBundle) -> ValidatorVerdict {
        let mut reason_codes = Vec::new();
        let mut referenced_artifact_digests = Vec::new();
        let mut normalization = None;
        let ran_expensive_checks = should_sample(
            bundle.bundle_digest.as_str(),
            self.policy.sampled_expensive_check_bps,
        );
        let ran_benchmark_checks = bundle.benchmark_expectation.is_some()
            && should_sample(
                bundle.bundle_digest.as_str(),
                self.policy.benchmark_check_sample_bps,
            );

        if self.policy.require_execution_proof
            && !bundle
                .artifact
                .proof_references
                .iter()
                .any(|proof| proof.kind == RolloutProofKind::ExecutionProof)
        {
            reason_codes.push(ValidatorReasonCode::ExecutionProofMissing);
        }

        if worker_outcome_is_stale(&bundle.worker_outcome) {
            reason_codes.push(ValidatorReasonCode::StalePolicyRejected);
        }

        if self
            .seen_artifact_digests
            .iter()
            .any(|digest| digest == &bundle.artifact.artifact_digest)
        {
            reason_codes.push(ValidatorReasonCode::ReplayedArtifactDetected);
            referenced_artifact_digests.push(bundle.artifact.artifact_digest.clone());
        }

        let response_signature_digest = stable_response_signature_digest(&bundle.artifact);
        if let Some(record) = self.seen_response_signatures.iter().find(|record| {
            record.response_signature_digest == response_signature_digest
                && record.artifact_digest != bundle.artifact.artifact_digest
        }) {
            reason_codes.push(ValidatorReasonCode::DuplicateDetected);
            referenced_artifact_digests.push(record.artifact_digest.clone());
            if let Some(normalized_weight_bps) = self.policy.normalize_duplicate_weight_bps {
                normalization = Some(ContributionNormalization {
                    normalized_weight_bps,
                    referenced_artifact_digests: vec![record.artifact_digest.clone()],
                });
                reason_codes.push(ValidatorReasonCode::ContributionNormalized);
            }
        }

        if ran_benchmark_checks {
            if let (Some(observation), Some(expectation)) = (
                bundle.benchmark_observation.as_ref(),
                bundle.benchmark_expectation.as_ref(),
            ) {
                if observation.observed_runtime_ms < expectation.min_runtime_ms
                    || observation.observed_runtime_ms > expectation.max_runtime_ms
                {
                    reason_codes.push(ValidatorReasonCode::TimerIntegrityMismatch);
                }
                if observation.observed_token_count != expectation.expected_token_count {
                    reason_codes.push(ValidatorReasonCode::TokenAccountingMismatch);
                }
                if expectation.expected_final_state_digest.is_some()
                    && observation.observed_final_state_digest
                        != expectation.expected_final_state_digest
                {
                    reason_codes.push(ValidatorReasonCode::FinalStateMismatch);
                }
                if expectation.expected_execution_strategy.is_some()
                    && observation.declared_execution_strategy
                        != expectation.expected_execution_strategy
                {
                    reason_codes.push(ValidatorReasonCode::ExecutionStrategyMismatch);
                }
            }
        }

        let disposition = if reason_codes.iter().any(|code| {
            matches!(
                code,
                ValidatorReasonCode::ExecutionProofMissing
                    | ValidatorReasonCode::StalePolicyRejected
                    | ValidatorReasonCode::ReplayedArtifactDetected
                    | ValidatorReasonCode::TimerIntegrityMismatch
                    | ValidatorReasonCode::TokenAccountingMismatch
                    | ValidatorReasonCode::FinalStateMismatch
                    | ValidatorReasonCode::ExecutionStrategyMismatch
            )
        }) {
            normalization = None;
            ValidatorDisposition::Rejected
        } else if normalization.is_some() {
            ValidatorDisposition::Normalized
        } else {
            ValidatorDisposition::Accepted
        };

        if !self
            .seen_artifact_digests
            .iter()
            .any(|digest| digest == &bundle.artifact.artifact_digest)
        {
            self.seen_artifact_digests
                .push(bundle.artifact.artifact_digest.clone());
        }
        if !self.seen_response_signatures.iter().any(|record| {
            record.response_signature_digest == response_signature_digest
                && record.artifact_digest == bundle.artifact.artifact_digest
        }) {
            self.seen_response_signatures
                .push(RolloutResponseSignatureRecord {
                    response_signature_digest,
                    artifact_digest: bundle.artifact.artifact_digest.clone(),
                    worker_id: bundle.artifact.worker_id.clone(),
                });
        }

        let verdict_id = format!("{}-verdict", bundle.bundle_id);
        let verdict_digest = stable_validator_verdict_digest(
            verdict_id.as_str(),
            self.policy.policy_id.as_str(),
            bundle.bundle_digest.as_str(),
            bundle.artifact.artifact_digest.as_str(),
            bundle.artifact.worker_id.as_str(),
            disposition,
            ran_expensive_checks,
            ran_benchmark_checks,
            reason_codes.as_slice(),
            referenced_artifact_digests.as_slice(),
            normalization.as_ref(),
        );
        let verdict = ValidatorVerdict {
            verdict_id,
            validator_policy_id: self.policy.policy_id.clone(),
            artifact_digest: bundle.artifact.artifact_digest,
            worker_id: bundle.artifact.worker_id,
            disposition,
            ran_expensive_checks,
            ran_benchmark_checks,
            reason_codes,
            referenced_artifact_digests,
            normalization,
            verdict_digest,
        };
        self.verdicts.push(verdict.clone());
        verdict
    }
}

fn worker_outcome_is_stale(worker_outcome: &RolloutWorkerOutcomeReceipt) -> bool {
    matches!(
        worker_outcome.outcome,
        RolloutWorkerOutcomeKind::UploadedQuarantined | RolloutWorkerOutcomeKind::UploadedDiscarded
    ) || worker_outcome
        .admission_receipt
        .as_ref()
        .is_some_and(|receipt| {
            receipt.outcome == RolloutReceiptOutcome::Quarantined
                || receipt.outcome == RolloutReceiptOutcome::Discarded
        })
}

fn should_sample(bundle_digest: &str, sample_bps: u16) -> bool {
    if sample_bps == 0 {
        return false;
    }
    if sample_bps >= 10_000 {
        return true;
    }
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_rollout_validator_sampling|");
    hasher.update(bundle_digest.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 2];
    bytes.copy_from_slice(&digest[..2]);
    (u16::from_le_bytes(bytes) % 10_000) < sample_bps
}

fn stable_rollout_verification_bundle_digest(
    bundle_id: &str,
    artifact: &RolloutArtifact,
    worker_outcome: &RolloutWorkerOutcomeReceipt,
    benchmark_observation: Option<&RolloutBenchmarkObservation>,
    benchmark_expectation: Option<&RolloutBenchmarkExpectation>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_rollout_verification_bundle|");
    hasher.update(bundle_id.as_bytes());
    hasher.update(b"|");
    hasher.update(artifact.artifact_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(worker_outcome.receipt_digest.as_bytes());
    if let Some(benchmark_observation) = benchmark_observation {
        hasher.update(b"|observation|");
        hasher.update(
            benchmark_observation
                .observed_runtime_ms
                .to_string()
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(
            benchmark_observation
                .observed_token_count
                .to_string()
                .as_bytes(),
        );
        if let Some(final_state_digest) = &benchmark_observation.observed_final_state_digest {
            hasher.update(b"|");
            hasher.update(final_state_digest.as_bytes());
        }
        if let Some(execution_strategy) = &benchmark_observation.declared_execution_strategy {
            hasher.update(b"|");
            hasher.update(execution_strategy.as_bytes());
        }
    }
    if let Some(benchmark_expectation) = benchmark_expectation {
        hasher.update(b"|expectation|");
        hasher.update(benchmark_expectation.min_runtime_ms.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(benchmark_expectation.max_runtime_ms.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(
            benchmark_expectation
                .expected_token_count
                .to_string()
                .as_bytes(),
        );
        if let Some(final_state_digest) = &benchmark_expectation.expected_final_state_digest {
            hasher.update(b"|");
            hasher.update(final_state_digest.as_bytes());
        }
        if let Some(execution_strategy) = &benchmark_expectation.expected_execution_strategy {
            hasher.update(b"|");
            hasher.update(execution_strategy.as_bytes());
        }
    }
    hex::encode(hasher.finalize())
}

fn stable_response_signature_digest(artifact: &RolloutArtifact) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_rollout_response_signature|");
    hasher.update(artifact.task_id.as_bytes());
    hasher.update(b"|");
    hasher.update(artifact.source_policy_revision.policy_family.as_bytes());
    for sample in &artifact.samples {
        hasher.update(b"|sample|");
        hasher.update(sample.token_id.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(sample.logprob.to_bits().to_le_bytes());
        hasher.update(b"|");
        hasher.update(sample.reward.to_bits().to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_validator_verdict_digest(
    verdict_id: &str,
    policy_id: &str,
    bundle_digest: &str,
    artifact_digest: &str,
    worker_id: &str,
    disposition: ValidatorDisposition,
    ran_expensive_checks: bool,
    ran_benchmark_checks: bool,
    reason_codes: &[ValidatorReasonCode],
    referenced_artifact_digests: &[String],
    normalization: Option<&ContributionNormalization>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_rollout_validator_verdict|");
    hasher.update(verdict_id.as_bytes());
    hasher.update(b"|");
    hasher.update(policy_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(artifact_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(worker_id.as_bytes());
    hasher.update(b"|");
    hasher.update(validator_disposition_label(disposition));
    hasher.update(if ran_expensive_checks {
        &b"|expensive|"[..]
    } else {
        &b"|cheap|"[..]
    });
    hasher.update(if ran_benchmark_checks {
        &b"|benchmark|"[..]
    } else {
        &b"|no_benchmark|"[..]
    });
    for reason_code in reason_codes {
        hasher.update(b"|reason|");
        hasher.update(validator_reason_code_label(*reason_code));
    }
    for digest in referenced_artifact_digests {
        hasher.update(b"|ref|");
        hasher.update(digest.as_bytes());
    }
    if let Some(normalization) = normalization {
        hasher.update(b"|normalized|");
        hasher.update(normalization.normalized_weight_bps.to_string().as_bytes());
        for digest in &normalization.referenced_artifact_digests {
            hasher.update(b"|normalized_ref|");
            hasher.update(digest.as_bytes());
        }
    }
    hex::encode(hasher.finalize())
}

fn validator_disposition_label(disposition: ValidatorDisposition) -> &'static [u8] {
    match disposition {
        ValidatorDisposition::Accepted => b"accepted",
        ValidatorDisposition::Normalized => b"normalized",
        ValidatorDisposition::Rejected => b"rejected",
    }
}

fn validator_reason_code_label(reason_code: ValidatorReasonCode) -> &'static [u8] {
    match reason_code {
        ValidatorReasonCode::ExecutionProofMissing => b"execution_proof_missing",
        ValidatorReasonCode::StalePolicyRejected => b"stale_policy_rejected",
        ValidatorReasonCode::ReplayedArtifactDetected => b"replayed_artifact_detected",
        ValidatorReasonCode::DuplicateDetected => b"duplicate_detected",
        ValidatorReasonCode::ContributionNormalized => b"contribution_normalized",
        ValidatorReasonCode::TimerIntegrityMismatch => b"timer_integrity_mismatch",
        ValidatorReasonCode::TokenAccountingMismatch => b"token_accounting_mismatch",
        ValidatorReasonCode::FinalStateMismatch => b"final_state_mismatch",
        ValidatorReasonCode::ExecutionStrategyMismatch => b"execution_strategy_mismatch",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RolloutBenchmarkExpectation, RolloutBenchmarkObservation, RolloutValidatorPolicy,
        RolloutValidatorState, RolloutVerificationBundle, ValidatorDisposition,
        ValidatorReasonCode,
    };
    use crate::{
        PolicyRevision, RolloutAdmissionReceipt, RolloutArtifact, RolloutProofKind,
        RolloutProofReference, RolloutReceiptOutcome, RolloutSample, RolloutTerminationReason,
        RolloutUploadLocator, RolloutUploadTransport, RolloutWorkerOutcomeKind,
        RolloutWorkerOutcomeReceipt, RolloutWorkerPolicyPosture, RolloutWorkerTrustClass,
    };

    fn artifact(
        worker_id: &str,
        artifact_id: &str,
        task_id: &str,
        source_policy_revision: PolicyRevision,
        created_at_ms: u64,
    ) -> Result<RolloutArtifact, Box<dyn std::error::Error>> {
        Ok(RolloutArtifact::new(
            artifact_id,
            worker_id,
            psionic_environments::EnvironmentPackageKey::new("oa.train", "2026.03"),
            task_id,
            source_policy_revision,
            vec![
                RolloutSample::new(1, -0.2, 1.0, 0.8),
                RolloutSample::new(2, -0.1, 0.6, 0.4),
            ],
            RolloutTerminationReason::Completed,
            vec![RolloutProofReference::new(
                RolloutProofKind::ExecutionProof,
                format!("proof-{artifact_id}"),
                format!("exec://{artifact_id}"),
            )],
            created_at_ms,
        )?)
    }

    fn admission_receipt(
        artifact: &RolloutArtifact,
        outcome: RolloutReceiptOutcome,
    ) -> RolloutAdmissionReceipt {
        RolloutAdmissionReceipt {
            receipt_id: format!("receipt-{}", artifact.artifact_id),
            run_id: String::from("run-1"),
            stage_id: String::from("stage-rl"),
            window_id: String::from("run-1-window-1"),
            artifact_id: artifact.artifact_id.clone(),
            artifact_digest: artifact.artifact_digest.clone(),
            worker_id: artifact.worker_id.clone(),
            environment_key: artifact.environment.storage_key(),
            target_policy_revision_id: String::from("policy-rev-7"),
            source_policy_revision_id: artifact.source_policy_revision.revision_id.clone(),
            source_policy_digest: artifact.source_policy_revision.policy_digest.clone(),
            outcome,
            revision_drift: None,
            policy_age_ms: None,
            rollout_age_ms: 5,
            signals: Vec::new(),
            token_count: artifact.token_count(),
            reward_sum: artifact.reward_sum(),
            termination_reason: artifact.termination_reason,
            observed_at_ms: 1_125,
            receipt_digest: format!("receipt-digest-{}", artifact.artifact_id),
        }
    }

    fn worker_outcome(
        artifact: &RolloutArtifact,
        outcome: RolloutWorkerOutcomeKind,
        policy_posture: RolloutWorkerPolicyPosture,
        admission_outcome: RolloutReceiptOutcome,
    ) -> RolloutWorkerOutcomeReceipt {
        RolloutWorkerOutcomeReceipt {
            receipt_id: format!("worker-outcome-{}", artifact.artifact_id),
            claim_id: format!("claim-{}", artifact.artifact_id),
            assignment_id: format!("assignment-{}", artifact.worker_id),
            window_id: String::from("run-1-window-1"),
            worker_id: artifact.worker_id.clone(),
            trust_class: RolloutWorkerTrustClass::UntrustedWorker,
            target_policy_revision_id: String::from("policy-rev-7"),
            source_policy_revision_id: artifact.source_policy_revision.revision_id.clone(),
            outcome,
            policy_posture,
            upload: RolloutUploadLocator::new(
                RolloutUploadTransport::InlineArtifact,
                format!("inline://{}", artifact.artifact_id),
                512,
                artifact.artifact_digest.as_str(),
            ),
            rejection_reason: None,
            admission_receipt: Some(admission_receipt(artifact, admission_outcome)),
            observed_at_ms: 1_125,
            receipt_digest: format!("worker-outcome-digest-{}", artifact.artifact_id),
        }
    }

    #[test]
    fn validator_accepts_fresh_rollouts_and_rejects_stale_policy_outcomes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let exact = artifact(
            "worker-a",
            "artifact-a",
            "task-a",
            PolicyRevision::new("train.decoder", "policy-rev-7", "policy-digest-7", 1_100),
            1_120,
        )?;
        let stale = artifact(
            "worker-b",
            "artifact-stale",
            "task-b",
            PolicyRevision::new("train.decoder", "policy-rev-4", "policy-digest-4", 1_000),
            1_120,
        )?;
        let mut validator = RolloutValidatorState::new(RolloutValidatorPolicy::default());
        let accepted = validator.verify_bundle(RolloutVerificationBundle::new(
            "bundle-accepted",
            exact.clone(),
            worker_outcome(
                &exact,
                RolloutWorkerOutcomeKind::UploadedAcceptedExact,
                RolloutWorkerPolicyPosture::ExactTarget,
                RolloutReceiptOutcome::AcceptedExact,
            ),
            None,
            None,
        ));
        assert_eq!(accepted.disposition, ValidatorDisposition::Accepted);

        let rejected = validator.verify_bundle(RolloutVerificationBundle::new(
            "bundle-stale",
            stale.clone(),
            worker_outcome(
                &stale,
                RolloutWorkerOutcomeKind::UploadedDiscarded,
                RolloutWorkerPolicyPosture::DiscardedOffPolicy,
                RolloutReceiptOutcome::Discarded,
            ),
            None,
            None,
        ));
        assert_eq!(rejected.disposition, ValidatorDisposition::Rejected);
        assert!(rejected
            .reason_codes
            .contains(&ValidatorReasonCode::StalePolicyRejected));
        Ok(())
    }

    #[test]
    fn validator_detects_replay_and_normalizes_duplicates() -> Result<(), Box<dyn std::error::Error>>
    {
        let first = artifact(
            "worker-a",
            "artifact-a",
            "task-shared",
            PolicyRevision::new("train.decoder", "policy-rev-7", "policy-digest-7", 1_100),
            1_120,
        )?;
        let replay = first.clone();
        let duplicate = artifact(
            "worker-b",
            "artifact-b",
            "task-shared",
            PolicyRevision::new("train.decoder", "policy-rev-7", "policy-digest-7", 1_100),
            1_121,
        )?;
        let mut validator = RolloutValidatorState::new(RolloutValidatorPolicy::default());

        let first_verdict = validator.verify_bundle(RolloutVerificationBundle::new(
            "bundle-first",
            first.clone(),
            worker_outcome(
                &first,
                RolloutWorkerOutcomeKind::UploadedAcceptedExact,
                RolloutWorkerPolicyPosture::ExactTarget,
                RolloutReceiptOutcome::AcceptedExact,
            ),
            None,
            None,
        ));
        assert_eq!(first_verdict.disposition, ValidatorDisposition::Accepted);

        let replay_verdict = validator.verify_bundle(RolloutVerificationBundle::new(
            "bundle-replay",
            replay.clone(),
            worker_outcome(
                &replay,
                RolloutWorkerOutcomeKind::UploadedAcceptedExact,
                RolloutWorkerPolicyPosture::ExactTarget,
                RolloutReceiptOutcome::AcceptedExact,
            ),
            None,
            None,
        ));
        assert_eq!(replay_verdict.disposition, ValidatorDisposition::Rejected);
        assert!(replay_verdict
            .reason_codes
            .contains(&ValidatorReasonCode::ReplayedArtifactDetected));

        let duplicate_verdict = validator.verify_bundle(RolloutVerificationBundle::new(
            "bundle-duplicate",
            duplicate.clone(),
            worker_outcome(
                &duplicate,
                RolloutWorkerOutcomeKind::UploadedAcceptedExact,
                RolloutWorkerPolicyPosture::ExactTarget,
                RolloutReceiptOutcome::AcceptedExact,
            ),
            None,
            None,
        ));
        assert_eq!(
            duplicate_verdict.disposition,
            ValidatorDisposition::Normalized
        );
        assert!(duplicate_verdict
            .reason_codes
            .contains(&ValidatorReasonCode::DuplicateDetected));
        assert!(duplicate_verdict
            .reason_codes
            .contains(&ValidatorReasonCode::ContributionNormalized));
        assert_eq!(
            duplicate_verdict
                .normalization
                .as_ref()
                .map(|normalization| normalization.normalized_weight_bps),
            Some(5_000)
        );
        Ok(())
    }

    #[test]
    fn validator_runs_benchmark_checks_with_typed_reason_codes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let artifact = artifact(
            "worker-a",
            "artifact-benchmark",
            "task-benchmark",
            PolicyRevision::new("train.decoder", "policy-rev-7", "policy-digest-7", 1_100),
            1_120,
        )?;
        let mut validator = RolloutValidatorState::new(RolloutValidatorPolicy {
            benchmark_check_sample_bps: 10_000,
            ..RolloutValidatorPolicy::default()
        });
        let verdict = validator.verify_bundle(RolloutVerificationBundle::new(
            "bundle-benchmark",
            artifact.clone(),
            worker_outcome(
                &artifact,
                RolloutWorkerOutcomeKind::UploadedAcceptedExact,
                RolloutWorkerPolicyPosture::ExactTarget,
                RolloutReceiptOutcome::AcceptedExact,
            ),
            Some(RolloutBenchmarkObservation {
                observed_runtime_ms: 25,
                observed_token_count: 1,
                observed_final_state_digest: Some(String::from("wrong-final-state")),
                declared_execution_strategy: Some(String::from("single_node")),
            }),
            Some(RolloutBenchmarkExpectation {
                min_runtime_ms: 40,
                max_runtime_ms: 60,
                expected_token_count: 2,
                expected_final_state_digest: Some(String::from("expected-final-state")),
                expected_execution_strategy: Some(String::from("tensor_parallel")),
            }),
        ));
        assert_eq!(verdict.disposition, ValidatorDisposition::Rejected);
        assert!(verdict.ran_benchmark_checks);
        assert!(verdict
            .reason_codes
            .contains(&ValidatorReasonCode::TimerIntegrityMismatch));
        assert!(verdict
            .reason_codes
            .contains(&ValidatorReasonCode::TokenAccountingMismatch));
        assert!(verdict
            .reason_codes
            .contains(&ValidatorReasonCode::FinalStateMismatch));
        assert!(verdict
            .reason_codes
            .contains(&ValidatorReasonCode::ExecutionStrategyMismatch));
        Ok(())
    }
}
