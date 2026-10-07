use psionic_eval::{BenchmarkAggregateSummary, EvalRunState};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    AdapterContributionArtifactReceipt, AdapterContributionProvenanceBundle,
    AdapterContributionSecurityDisposition, AdapterContributionSecurityReceipt,
    AdapterContributionSubmissionReceipt, AdapterContributionValidatorDisposition,
    AdapterTrainingWindowStateMachine, AdapterWindowContractError, PolicyRevision,
};

/// Error returned by adapter validator and window-scoring flows.
#[derive(Debug, Error)]
pub enum AdapterValidationError {
    /// The target contribution was not present in the bundle set.
    #[error("missing validation bundle for contribution `{contribution_id}`")]
    MissingBundle {
        /// Stable contribution identifier.
        contribution_id: String,
    },
    /// The validation bundle referenced a different contribution id than the window record.
    #[error(
        "validation bundle contribution mismatch: expected `{expected_contribution_id}`, found `{actual_contribution_id}`"
    )]
    ContributionMismatch {
        /// Expected stable contribution identifier.
        expected_contribution_id: String,
        /// Actual stable contribution identifier.
        actual_contribution_id: String,
    },
    /// The target eval run must be finalized before scoring.
    #[error("eval run `{eval_run_id}` must be finalized before window scoring")]
    EvalRunNotFinalized {
        /// Stable eval run identifier.
        eval_run_id: String,
    },
    /// The underlying adapter-window contract rejected the requested transition.
    #[error(transparent)]
    WindowContract(#[from] AdapterWindowContractError),
}

/// Validator policy for decentralized adapter contributions and candidate windows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionValidatorPolicy {
    /// Stable validator policy identifier.
    pub validator_policy_id: String,
    /// Deterministic replay sample rate for contribution-level validator replays.
    pub replay_sample_bps: u16,
    /// Minimum held-out average score required for promotion readiness.
    pub minimum_held_out_score_bps: u32,
    /// Minimum benchmark pass rate required for promotion readiness.
    pub minimum_benchmark_pass_rate_bps: u32,
    /// Adapter formats that require runtime-smoke verification before promotion.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_smoke_required_adapter_formats: Vec<String>,
}

impl Default for AdapterContributionValidatorPolicy {
    fn default() -> Self {
        Self {
            validator_policy_id: String::from("adapter-validator-default"),
            replay_sample_bps: 2_500,
            minimum_held_out_score_bps: 8_000,
            minimum_benchmark_pass_rate_bps: 9_000,
            runtime_smoke_required_adapter_formats: vec![String::from("apple.fmadapter")],
        }
    }
}

/// One validator replay receipt for a sampled adapter contribution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionReplayReceipt {
    /// Stable replay identifier.
    pub replay_id: String,
    /// Stable contribution identifier.
    pub contribution_id: String,
    /// Stable expected adapter-delta digest from the worker submission.
    pub expected_adapter_delta_digest: String,
    /// Stable digest observed by validator replay.
    pub observed_adapter_delta_digest: String,
    /// Whether the validator replay matched the uploaded contribution intent.
    pub matched_uploaded_delta: bool,
    /// Replay completion time.
    pub replayed_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

impl AdapterContributionReplayReceipt {
    /// Creates one replay receipt.
    #[must_use]
    pub fn new(
        contribution_id: impl Into<String>,
        expected_adapter_delta_digest: impl Into<String>,
        observed_adapter_delta_digest: impl Into<String>,
        replayed_at_ms: u64,
    ) -> Self {
        let contribution_id = contribution_id.into();
        let expected_adapter_delta_digest = expected_adapter_delta_digest.into();
        let observed_adapter_delta_digest = observed_adapter_delta_digest.into();
        let matched_uploaded_delta = expected_adapter_delta_digest == observed_adapter_delta_digest;
        Self {
            replay_id: format!("adapter-replay:{contribution_id}"),
            contribution_id: contribution_id.clone(),
            receipt_digest: stable_replay_receipt_digest(
                contribution_id.as_str(),
                expected_adapter_delta_digest.as_str(),
                observed_adapter_delta_digest.as_str(),
                matched_uploaded_delta,
                replayed_at_ms,
            ),
            expected_adapter_delta_digest,
            observed_adapter_delta_digest,
            matched_uploaded_delta,
            replayed_at_ms,
        }
    }
}

/// One validator-ready contribution bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionValidationBundle {
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Worker submission receipt.
    pub submission: AdapterContributionSubmissionReceipt,
    /// Staged artifact receipt.
    pub artifact: AdapterContributionArtifactReceipt,
    /// Accepted or quarantined provenance bundle retained from security verification.
    pub provenance: AdapterContributionProvenanceBundle,
    /// Security receipt bound to the contribution.
    pub security: AdapterContributionSecurityReceipt,
    /// Optional validator replay receipt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replay: Option<AdapterContributionReplayReceipt>,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

impl AdapterContributionValidationBundle {
    /// Creates a validation bundle from existing contribution receipts.
    #[must_use]
    pub fn new(
        submission: AdapterContributionSubmissionReceipt,
        artifact: AdapterContributionArtifactReceipt,
        provenance: AdapterContributionProvenanceBundle,
        security: AdapterContributionSecurityReceipt,
        replay: Option<AdapterContributionReplayReceipt>,
    ) -> Self {
        let bundle_id = format!("adapter-validation:{}", artifact.contribution_id);
        let bundle_digest = stable_validation_bundle_digest(
            bundle_id.as_str(),
            submission.receipt_digest.as_str(),
            artifact.receipt_digest.as_str(),
            provenance.bundle_digest.as_str(),
            security.receipt_digest.as_str(),
            replay
                .as_ref()
                .map(|receipt| receipt.receipt_digest.as_str()),
        );
        Self {
            bundle_id,
            submission,
            artifact,
            provenance,
            security,
            replay,
            bundle_digest,
        }
    }
}

/// One contribution-level validator reason code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterContributionValidationReasonCode {
    /// Security verification rejected the contribution.
    SecurityRejected,
    /// Security verification quarantined the contribution.
    SecurityQuarantined,
    /// Validator sampling requires a replay that is not yet present.
    ReplayRequired,
    /// Validator replay did not match the worker-reported adapter delta.
    ReplayMismatch,
}

/// Final validator verdict for one adapter contribution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionValidationVerdict {
    /// Stable verdict identifier.
    pub verdict_id: String,
    /// Stable validator policy identifier.
    pub validator_policy_id: String,
    /// Stable contribution identifier.
    pub contribution_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Final disposition applied to the contribution.
    pub disposition: AdapterContributionValidatorDisposition,
    /// Whether the bundle was selected for replay.
    pub replay_checked: bool,
    /// Machine-readable reason codes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason_codes: Vec<AdapterContributionValidationReasonCode>,
    /// Stable verdict digest.
    pub verdict_digest: String,
}

/// Candidate evaluation over one sealed or sealing adapter window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AdapterWindowCandidateEvaluation {
    /// Candidate output policy revision under review.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_policy_revision: Option<PolicyRevision>,
    /// Held-out eval run for the candidate state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub held_out_eval: Option<EvalRunState>,
    /// Benchmark aggregate summary for the candidate state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark_summary: Option<BenchmarkAggregateSummary>,
    /// Runtime-smoke eval run for the candidate state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_smoke_eval: Option<EvalRunState>,
}

/// Window-level gate reason for promotion readiness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterWindowScoreReasonCode {
    /// A candidate policy revision was present without a held-out eval run.
    HeldOutEvalMissing,
    /// Held-out eval did not meet the minimum score threshold.
    HeldOutEvalBelowThreshold,
    /// A candidate policy revision was present without a benchmark summary.
    BenchmarkMissing,
    /// Benchmark pass rate did not meet the minimum threshold.
    BenchmarkBelowThreshold,
    /// The adapter format requires runtime smoke before promotion.
    RuntimeSmokeRequired,
    /// Runtime smoke was present but failed.
    RuntimeSmokeFailed,
}

/// Scored summary for one validated adapter window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterWindowScoreSummary {
    /// Stable validator policy identifier.
    pub validator_policy_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Total contributions reviewed for the window.
    pub total_contributions: u32,
    /// Contributions admitted past hard rejection.
    pub admitted_contributions: u32,
    /// Accepted contributions.
    pub accepted_contributions: u32,
    /// Quarantined contributions.
    pub quarantined_contributions: u32,
    /// Rejected contributions.
    pub rejected_contributions: u32,
    /// Replay-required contributions.
    pub replay_required_contributions: u32,
    /// Contributions that actually ran validator replay.
    pub replay_checked_contributions: u32,
    /// Optional held-out average score for the candidate state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub held_out_average_score_bps: Option<u32>,
    /// Optional benchmark aggregate pass rate for the candidate state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark_pass_rate_bps: Option<u32>,
    /// Whether runtime smoke passed when required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_smoke_passed: Option<bool>,
    /// Whether the candidate state is promotion-ready under the current validator view.
    pub promotion_ready: bool,
    /// Window-level gate reasons that prevented promotion readiness.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gate_reason_codes: Vec<AdapterWindowScoreReasonCode>,
    /// Scoring time.
    pub scored_at_ms: u64,
    /// Stable summary digest.
    pub summary_digest: String,
}

/// Stateful adapter contribution validator and window scorer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionValidatorState {
    /// Active validator policy.
    pub policy: AdapterContributionValidatorPolicy,
    /// Contribution-level verdict history.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verdicts: Vec<AdapterContributionValidationVerdict>,
    /// Window-level scoring summaries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub window_summaries: Vec<AdapterWindowScoreSummary>,
}

impl AdapterContributionValidatorState {
    /// Creates a validator state from policy.
    #[must_use]
    pub fn new(policy: AdapterContributionValidatorPolicy) -> Self {
        Self {
            policy,
            verdicts: Vec::new(),
            window_summaries: Vec::new(),
        }
    }

    /// Validates every contribution in the active window and seals the window with a scored summary.
    pub fn validate_window(
        &mut self,
        window: &mut AdapterTrainingWindowStateMachine,
        bundles: Vec<AdapterContributionValidationBundle>,
        candidate: Option<&AdapterWindowCandidateEvaluation>,
        scored_at_ms: u64,
    ) -> Result<AdapterWindowScoreSummary, AdapterValidationError> {
        let mut accepted_count = 0_u32;
        let mut quarantined_count = 0_u32;
        let mut rejected_count = 0_u32;
        let mut replay_required_count = 0_u32;
        let mut replay_checked_count = 0_u32;

        for contribution in window.contributions.clone() {
            let contribution_id = contribution.assignment.binding.contribution_id;
            let bundle = bundles
                .iter()
                .find(|bundle| bundle.artifact.contribution_id == contribution_id)
                .ok_or_else(|| AdapterValidationError::MissingBundle {
                    contribution_id: contribution_id.clone(),
                })?;
            if bundle.submission.contribution_id != contribution_id {
                return Err(AdapterValidationError::ContributionMismatch {
                    expected_contribution_id: contribution_id,
                    actual_contribution_id: bundle.submission.contribution_id.clone(),
                });
            }

            let verdict = self.verify_bundle(bundle);
            let validator_reason = primary_reason_label(verdict.reason_codes.first().copied());
            window.record_validator_disposition(
                verdict.contribution_id.as_str(),
                verdict.disposition,
                validator_reason,
                scored_at_ms,
            )?;
            if verdict.disposition == AdapterContributionValidatorDisposition::Accepted {
                accepted_count = accepted_count.saturating_add(1);
                window.record_aggregation_eligibility(
                    verdict.contribution_id.as_str(),
                    Some(10_000),
                    scored_at_ms,
                )?;
            } else {
                match verdict.disposition {
                    AdapterContributionValidatorDisposition::Quarantined => {
                        quarantined_count = quarantined_count.saturating_add(1);
                    }
                    AdapterContributionValidatorDisposition::Rejected => {
                        rejected_count = rejected_count.saturating_add(1);
                    }
                    AdapterContributionValidatorDisposition::ReplayRequired => {
                        replay_required_count = replay_required_count.saturating_add(1);
                    }
                    AdapterContributionValidatorDisposition::Accepted => {}
                }
                window.record_aggregation_eligibility(
                    verdict.contribution_id.as_str(),
                    None,
                    scored_at_ms,
                )?;
            }
            if verdict.replay_checked {
                replay_checked_count = replay_checked_count.saturating_add(1);
            }
            self.verdicts.push(verdict);
        }

        window.seal()?;

        let total_contributions = window.contributions.len() as u32;
        let admitted_count = total_contributions
            .saturating_sub(rejected_count)
            .saturating_sub(replay_required_count);
        let (
            held_out_average_score_bps,
            benchmark_pass_rate_bps,
            runtime_smoke_passed,
            gate_reason_codes,
            promotion_ready,
        ) = self.score_candidate(window, candidate)?;

        let summary = AdapterWindowScoreSummary {
            validator_policy_id: self.policy.validator_policy_id.clone(),
            window_id: window.window_id.clone(),
            total_contributions,
            admitted_contributions: admitted_count,
            accepted_contributions: accepted_count,
            quarantined_contributions: quarantined_count,
            rejected_contributions: rejected_count,
            replay_required_contributions: replay_required_count,
            replay_checked_contributions: replay_checked_count,
            held_out_average_score_bps,
            benchmark_pass_rate_bps,
            runtime_smoke_passed,
            promotion_ready: promotion_ready && accepted_count > 0 && replay_required_count == 0,
            gate_reason_codes: gate_reason_codes.clone(),
            scored_at_ms,
            summary_digest: stable_window_score_digest(
                self.policy.validator_policy_id.as_str(),
                window.window_id.as_str(),
                total_contributions,
                admitted_count,
                accepted_count,
                quarantined_count,
                rejected_count,
                replay_required_count,
                replay_checked_count,
                held_out_average_score_bps,
                benchmark_pass_rate_bps,
                runtime_smoke_passed,
                promotion_ready && accepted_count > 0 && replay_required_count == 0,
                gate_reason_codes.as_slice(),
                scored_at_ms,
            ),
        };
        self.window_summaries.push(summary.clone());
        Ok(summary)
    }

    fn verify_bundle(
        &self,
        bundle: &AdapterContributionValidationBundle,
    ) -> AdapterContributionValidationVerdict {
        let replay_checked =
            should_sample(bundle.bundle_digest.as_str(), self.policy.replay_sample_bps);
        let mut reason_codes = Vec::new();

        match bundle.security.disposition {
            AdapterContributionSecurityDisposition::Accepted => {}
            AdapterContributionSecurityDisposition::Quarantined => {
                reason_codes.push(AdapterContributionValidationReasonCode::SecurityQuarantined);
            }
            AdapterContributionSecurityDisposition::Rejected => {
                reason_codes.push(AdapterContributionValidationReasonCode::SecurityRejected);
            }
        }

        if replay_checked {
            match bundle.replay.as_ref() {
                None => reason_codes.push(AdapterContributionValidationReasonCode::ReplayRequired),
                Some(replay) if !replay.matched_uploaded_delta => {
                    reason_codes.push(AdapterContributionValidationReasonCode::ReplayMismatch);
                }
                Some(_) => {}
            }
        }

        let disposition = if reason_codes
            .contains(&AdapterContributionValidationReasonCode::SecurityRejected)
            || reason_codes.contains(&AdapterContributionValidationReasonCode::ReplayMismatch)
        {
            AdapterContributionValidatorDisposition::Rejected
        } else if reason_codes.contains(&AdapterContributionValidationReasonCode::ReplayRequired) {
            AdapterContributionValidatorDisposition::ReplayRequired
        } else if reason_codes
            .contains(&AdapterContributionValidationReasonCode::SecurityQuarantined)
        {
            AdapterContributionValidatorDisposition::Quarantined
        } else {
            AdapterContributionValidatorDisposition::Accepted
        };

        AdapterContributionValidationVerdict {
            verdict_id: format!("adapter-validator:{}", bundle.artifact.contribution_id),
            validator_policy_id: self.policy.validator_policy_id.clone(),
            contribution_id: bundle.artifact.contribution_id.clone(),
            worker_id: bundle.artifact.worker_id.clone(),
            disposition,
            replay_checked,
            verdict_digest: stable_verdict_digest(
                self.policy.validator_policy_id.as_str(),
                bundle.artifact.contribution_id.as_str(),
                bundle.artifact.worker_id.as_str(),
                disposition,
                replay_checked,
                reason_codes.as_slice(),
            ),
            reason_codes,
        }
    }

    fn score_candidate(
        &self,
        window: &AdapterTrainingWindowStateMachine,
        candidate: Option<&AdapterWindowCandidateEvaluation>,
    ) -> Result<
        (
            Option<u32>,
            Option<u32>,
            Option<bool>,
            Vec<AdapterWindowScoreReasonCode>,
            bool,
        ),
        AdapterValidationError,
    > {
        let mut gate_reason_codes = Vec::new();
        let Some(candidate) = candidate else {
            return Ok((None, None, None, gate_reason_codes, false));
        };
        let candidate_present = candidate.candidate_policy_revision.is_some();
        if !candidate_present {
            return Ok((None, None, None, gate_reason_codes, false));
        }

        let held_out_average_score_bps = match candidate.held_out_eval.as_ref() {
            None => {
                gate_reason_codes.push(AdapterWindowScoreReasonCode::HeldOutEvalMissing);
                None
            }
            Some(eval_run) => {
                if eval_run.summary.is_none() {
                    return Err(AdapterValidationError::EvalRunNotFinalized {
                        eval_run_id: eval_run.contract.eval_run_id.clone(),
                    });
                }
                let score = eval_run
                    .summary
                    .as_ref()
                    .and_then(|summary| summary.average_score_bps);
                if score.is_none_or(|score| score < self.policy.minimum_held_out_score_bps) {
                    gate_reason_codes.push(AdapterWindowScoreReasonCode::HeldOutEvalBelowThreshold);
                }
                score
            }
        };

        let benchmark_pass_rate_bps = match candidate.benchmark_summary.as_ref() {
            None => {
                gate_reason_codes.push(AdapterWindowScoreReasonCode::BenchmarkMissing);
                None
            }
            Some(summary) => {
                if summary.aggregate_pass_rate_bps < self.policy.minimum_benchmark_pass_rate_bps {
                    gate_reason_codes.push(AdapterWindowScoreReasonCode::BenchmarkBelowThreshold);
                }
                Some(summary.aggregate_pass_rate_bps)
            }
        };

        let runtime_smoke_required = self
            .policy
            .runtime_smoke_required_adapter_formats
            .iter()
            .any(|format| format == &window.adapter_target.adapter_format);
        let runtime_smoke_passed = if runtime_smoke_required {
            match candidate.runtime_smoke_eval.as_ref() {
                None => {
                    gate_reason_codes.push(AdapterWindowScoreReasonCode::RuntimeSmokeRequired);
                    Some(false)
                }
                Some(eval_run) => {
                    if eval_run.summary.is_none() {
                        return Err(AdapterValidationError::EvalRunNotFinalized {
                            eval_run_id: eval_run.contract.eval_run_id.clone(),
                        });
                    }
                    let passed = eval_run
                        .summary
                        .as_ref()
                        .is_some_and(|summary| summary.pass_rate_bps == 10_000);
                    if !passed {
                        gate_reason_codes.push(AdapterWindowScoreReasonCode::RuntimeSmokeFailed);
                    }
                    Some(passed)
                }
            }
        } else {
            None
        };

        Ok((
            held_out_average_score_bps,
            benchmark_pass_rate_bps,
            runtime_smoke_passed,
            gate_reason_codes.clone(),
            gate_reason_codes.is_empty(),
        ))
    }
}

fn should_sample(bundle_digest: &str, sample_bps: u16) -> bool {
    if sample_bps == 0 {
        return false;
    }
    if sample_bps >= 10_000 {
        return true;
    }
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_adapter_validator_sampling|");
    hasher.update(bundle_digest.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 2];
    bytes.copy_from_slice(&digest[..2]);
    (u16::from_le_bytes(bytes) % 10_000) < sample_bps
}

fn primary_reason_label(
    reason_code: Option<AdapterContributionValidationReasonCode>,
) -> &'static str {
    match reason_code {
        Some(AdapterContributionValidationReasonCode::SecurityRejected) => {
            "validator.rejected.security"
        }
        Some(AdapterContributionValidationReasonCode::SecurityQuarantined) => {
            "validator.quarantined.security"
        }
        Some(AdapterContributionValidationReasonCode::ReplayRequired) => {
            "validator.replay_required.sampled_replay_missing"
        }
        Some(AdapterContributionValidationReasonCode::ReplayMismatch) => {
            "validator.rejected.replay_mismatch"
        }
        None => "validator.accepted",
    }
}

fn stable_replay_receipt_digest(
    contribution_id: &str,
    expected_adapter_delta_digest: &str,
    observed_adapter_delta_digest: &str,
    matched_uploaded_delta: bool,
    replayed_at_ms: u64,
) -> String {
    stable_digest([
        "adapter_replay_receipt",
        contribution_id,
        expected_adapter_delta_digest,
        observed_adapter_delta_digest,
        if matched_uploaded_delta {
            "matched"
        } else {
            "mismatched"
        },
        replayed_at_ms.to_string().as_str(),
    ])
}

fn stable_validation_bundle_digest(
    bundle_id: &str,
    submission_receipt_digest: &str,
    artifact_receipt_digest: &str,
    provenance_bundle_digest: &str,
    security_receipt_digest: &str,
    replay_receipt_digest: Option<&str>,
) -> String {
    stable_digest([
        "adapter_validation_bundle",
        bundle_id,
        submission_receipt_digest,
        artifact_receipt_digest,
        provenance_bundle_digest,
        security_receipt_digest,
        replay_receipt_digest.unwrap_or("-"),
    ])
}

fn stable_verdict_digest(
    validator_policy_id: &str,
    contribution_id: &str,
    worker_id: &str,
    disposition: AdapterContributionValidatorDisposition,
    replay_checked: bool,
    reason_codes: &[AdapterContributionValidationReasonCode],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_adapter_validation_verdict|");
    hasher.update(validator_policy_id.as_bytes());
    hasher.update(b"|");
    hasher.update(contribution_id.as_bytes());
    hasher.update(b"|");
    hasher.update(worker_id.as_bytes());
    hasher.update(b"|");
    hasher.update(adapter_validator_disposition_label(disposition).as_bytes());
    hasher.update(b"|");
    hasher.update(if replay_checked {
        b"replay_checked"
    } else {
        b"replay_skipped"
    });
    for reason_code in reason_codes {
        hasher.update(b"|reason|");
        hasher.update(adapter_validation_reason_label(*reason_code).as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_window_score_digest(
    validator_policy_id: &str,
    window_id: &str,
    total_contributions: u32,
    admitted_contributions: u32,
    accepted_contributions: u32,
    quarantined_contributions: u32,
    rejected_contributions: u32,
    replay_required_contributions: u32,
    replay_checked_contributions: u32,
    held_out_average_score_bps: Option<u32>,
    benchmark_pass_rate_bps: Option<u32>,
    runtime_smoke_passed: Option<bool>,
    promotion_ready: bool,
    gate_reason_codes: &[AdapterWindowScoreReasonCode],
    scored_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_adapter_window_score|");
    for part in [
        validator_policy_id,
        window_id,
        total_contributions.to_string().as_str(),
        admitted_contributions.to_string().as_str(),
        accepted_contributions.to_string().as_str(),
        quarantined_contributions.to_string().as_str(),
        rejected_contributions.to_string().as_str(),
        replay_required_contributions.to_string().as_str(),
        replay_checked_contributions.to_string().as_str(),
        held_out_average_score_bps
            .map(|score| score.to_string())
            .unwrap_or_else(|| String::from("-"))
            .as_str(),
        benchmark_pass_rate_bps
            .map(|score| score.to_string())
            .unwrap_or_else(|| String::from("-"))
            .as_str(),
        match runtime_smoke_passed {
            Some(true) => "runtime_smoke_passed",
            Some(false) => "runtime_smoke_failed",
            None => "runtime_smoke_not_required",
        },
        if promotion_ready {
            "promotion_ready"
        } else {
            "promotion_blocked"
        },
        scored_at_ms.to_string().as_str(),
    ] {
        hasher.update(part.as_bytes());
        hasher.update(b"|");
    }
    for reason_code in gate_reason_codes {
        hasher.update(b"|gate|");
        hasher.update(adapter_window_reason_label(*reason_code).as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_digest<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update(b"|");
    }
    hex::encode(hasher.finalize())
}

fn adapter_validator_disposition_label(
    disposition: AdapterContributionValidatorDisposition,
) -> &'static str {
    match disposition {
        AdapterContributionValidatorDisposition::Accepted => "accepted",
        AdapterContributionValidatorDisposition::Quarantined => "quarantined",
        AdapterContributionValidatorDisposition::Rejected => "rejected",
        AdapterContributionValidatorDisposition::ReplayRequired => "replay_required",
    }
}

fn adapter_validation_reason_label(
    reason_code: AdapterContributionValidationReasonCode,
) -> &'static str {
    match reason_code {
        AdapterContributionValidationReasonCode::SecurityRejected => "security_rejected",
        AdapterContributionValidationReasonCode::SecurityQuarantined => "security_quarantined",
        AdapterContributionValidationReasonCode::ReplayRequired => "replay_required",
        AdapterContributionValidationReasonCode::ReplayMismatch => "replay_mismatch",
    }
}

fn adapter_window_reason_label(reason_code: AdapterWindowScoreReasonCode) -> &'static str {
    match reason_code {
        AdapterWindowScoreReasonCode::HeldOutEvalMissing => "held_out_eval_missing",
        AdapterWindowScoreReasonCode::HeldOutEvalBelowThreshold => "held_out_eval_below_threshold",
        AdapterWindowScoreReasonCode::BenchmarkMissing => "benchmark_missing",
        AdapterWindowScoreReasonCode::BenchmarkBelowThreshold => "benchmark_below_threshold",
        AdapterWindowScoreReasonCode::RuntimeSmokeRequired => "runtime_smoke_required",
        AdapterWindowScoreReasonCode::RuntimeSmokeFailed => "runtime_smoke_failed",
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };

    use ed25519_dalek::SigningKey;
    use psionic_cluster::{
        AdmissionToken, ClusterBackendReadinessStatus, ClusterId, ClusterMembershipRecord,
        ClusterMembershipStatus, ClusterNamespace, ClusterNodeIdentity, ClusterNodeTelemetry,
        ClusterSnapshot, ClusterStabilityPosture, NodeEpoch, NodeId, NodeRole,
    };
    use psionic_datastream::{DatastreamEncoding, DatastreamManifest, DatastreamSubjectKind};
    use psionic_eval::{
        BenchmarkAggregationKind, BenchmarkCase, BenchmarkExecutionMode, BenchmarkPackage,
        BenchmarkPackageKey, EvalRunContract, EvalRunMode, EvalRunState, EvalSampleRecord,
        EvalSampleStatus,
    };

    use super::{
        AdapterContributionReplayReceipt, AdapterContributionValidationBundle,
        AdapterContributionValidatorPolicy, AdapterContributionValidatorState,
        AdapterValidationError, AdapterWindowCandidateEvaluation, AdapterWindowScoreReasonCode,
    };
    use crate::{
        AdapterArtifactRetentionPolicy, AdapterArtifactStorageState,
        AdapterContributionExecutionSummary, AdapterContributionProvenanceBundle,
        AdapterContributionSecurityController, AdapterContributionSecurityPolicy,
        AdapterContributionUploadLocator, AdapterContributionValidatorDisposition,
        AdapterContributorCapabilityPolicy, AdapterDatasetSliceIdentity, AdapterTargetIdentity,
        AdapterTrainingClusterCoordinator, AdapterTrainingWindowStateMachine,
        AdapterWorkerIdentity, AdapterWorkerProtocolPolicy, AdapterWorkerProtocolState,
        AdapterWorkerTrustClass, CheckpointPointer, CheckpointScopeBinding, CheckpointScopeKind,
        PolicyRevision,
    };

    const GIB_BYTES: u64 = 1024 * 1024 * 1024;

    fn cluster_state() -> psionic_cluster::ClusterState {
        let cluster_id = ClusterId::new(
            &ClusterNamespace::new("adapter-validation"),
            &AdmissionToken::new("shared-secret"),
        );
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships = BTreeMap::from([(
            NodeId::new("worker-b"),
            ClusterMembershipRecord::new(
                ClusterNodeIdentity {
                    cluster_id: cluster_id.clone(),
                    node_id: NodeId::new("worker-b"),
                    node_epoch: NodeEpoch::initial(),
                    role: NodeRole::ExecutorOnly,
                    auth_public_key: String::from("worker-b-pk"),
                    attestation: None,
                },
                Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 33_400)),
                ClusterMembershipStatus::Ready,
            ),
        )]);
        snapshot.telemetry = BTreeMap::from([(
            NodeId::new("worker-b"),
            ClusterNodeTelemetry::new(NodeId::new("worker-b"))
                .with_memory(Some(24 * GIB_BYTES), Some(24 * GIB_BYTES))
                .with_accelerator_count(1)
                .with_backend_readiness(
                    AdapterContributorCapabilityPolicy::default().backend_label,
                    ClusterBackendReadinessStatus::Ready,
                )
                .with_stability_posture(ClusterStabilityPosture::Stable),
        )]);
        psionic_cluster::ClusterState::from_snapshot(snapshot)
    }

    fn checkpoint_reference(
        checkpoint_ref: &str,
        started_at_ms: u64,
    ) -> psionic_runtime::TrainingCheckpointReference {
        psionic_runtime::TrainingCheckpointReference::new(
            "apple.weather.policy",
            format!("stream://{checkpoint_ref}"),
            format!("manifest://{checkpoint_ref}"),
            format!("object://{checkpoint_ref}"),
            "worker-b",
            7,
            "cluster-digest-weather",
            "topology-digest-weather",
            started_at_ms,
        )
        .with_checkpoint_ref(checkpoint_ref)
        .with_step(70)
    }

    fn contribution_fixture(
        signing_key: &SigningKey,
    ) -> Result<
        (
            AdapterTrainingWindowStateMachine,
            AdapterContributionValidationBundle,
        ),
        Box<dyn std::error::Error>,
    > {
        let state = cluster_state();
        let run = crate::TrainingRunState::new(
            "adapter-run-validation",
            "adapter-sft",
            state.cluster_id().as_str(),
            "apple.weather.policy",
            psionic_environments::EnvironmentPackageKey::new("oa.apple.adapter", "2026.03"),
        )?;
        let mut coordinator = AdapterTrainingClusterCoordinator::new(
            run,
            AdapterTargetIdentity::new(
                "apple.weather.adapter",
                "apple.foundation_models",
                "apple://foundation-model/base",
                "apple.fmadapter",
            )?,
            PolicyRevision::new(
                "apple.weather.policy",
                "policy-r7",
                "policy-digest-r7",
                1_000,
            )
            .with_revision_number(7),
            CheckpointPointer::new(
                CheckpointScopeBinding::new(CheckpointScopeKind::Window, "window-weather-6"),
                "apple.weather.policy",
                checkpoint_reference("checkpoint/weather/r7", 1_000),
                "manifest-digest-r7",
                1_001,
            )?,
            AdapterContributorCapabilityPolicy {
                minimum_free_memory_bytes: 12 * GIB_BYTES,
                ..AdapterContributorCapabilityPolicy::default()
            },
        );
        coordinator.observe_cluster_state(&state, 1_010)?;
        let record = coordinator.plan_next_window(
            vec![AdapterDatasetSliceIdentity::new(
                "dataset.weather",
                "train",
                "slice-b",
                "slice-digest-b",
            )?],
            1,
            1_020,
        )?;
        let mut protocol = AdapterWorkerProtocolState::from_window_record(
            &record,
            AdapterWorkerProtocolPolicy::default(),
        );
        protocol.activate_window()?;
        let public_key_hex = hex::encode(signing_key.verifying_key().to_bytes());
        let identity = AdapterWorkerIdentity::new(
            "worker-b",
            "session-1",
            AdapterWorkerTrustClass::SemiTrustedContributor,
            "auth://worker-b",
        )
        .with_submission_signing_public_key_hex(public_key_hex);
        protocol.record_heartbeat(identity, None, None, 1_030)?;

        let assignment = protocol.assignments[0].clone();
        let claim =
            protocol.claim_assignment("worker-b", assignment.assignment_id.as_str(), 1_031)?;
        protocol.acknowledge_assignment("worker-b", "session-1", claim.claim_id.as_str(), 1_032)?;
        let submission = protocol.submit_contribution(
            claim.claim_id.as_str(),
            "worker-b",
            "session-1",
            "policy-r7",
            record.plan.input_checkpoint_pointer.pointer_digest.as_str(),
            AdapterContributionExecutionSummary::new(
                1_033,
                1_040,
                5,
                20,
                Some(205),
                "delta-digest-validation",
            )?,
            AdapterContributionUploadLocator::new(
                format!(
                    "object://adapter-window/{}/{}",
                    record.plan.window_id, claim.contribution_id
                ),
                "upload-manifest-validation",
                4_096,
            )?,
            1_041,
        )?;

        let payload = b"adapter-delta-payload".repeat(4);
        let manifest = DatastreamManifest::from_bytes(
            format!(
                "adapter-contribution:{}:{}",
                assignment.window_id, assignment.contribution_id
            ),
            DatastreamSubjectKind::AdapterPackage,
            &payload,
            8,
            DatastreamEncoding::RawBinary,
        )
        .with_provenance_digest(assignment.upload_expectation.expectation_digest.clone());
        let upload = AdapterContributionUploadLocator::new(
            assignment
                .upload_expectation
                .upload_reference_prefix
                .clone(),
            manifest.manifest_ref().manifest_digest.clone(),
            manifest.manifest_ref().total_bytes,
        )?;
        let mut storage =
            AdapterArtifactStorageState::new(AdapterArtifactRetentionPolicy::default())?;
        let cursor = storage.start_contribution_upload(
            &assignment,
            upload,
            &payload,
            8,
            "worker-b",
            1_042,
        )?;
        for chunk in payload.chunks(8) {
            storage.commit_next_chunk(cursor.upload_id.as_str(), chunk)?;
        }
        let artifact = storage.complete_contribution_upload(cursor.upload_id.as_str(), 1_043)?;

        let session = protocol.sessions[0].identity.clone();
        let provenance = AdapterContributionProvenanceBundle::new_signed(
            &assignment,
            &claim,
            &session,
            &submission,
            &artifact,
            signing_key,
            1_044,
        );
        let mut security_controller = AdapterContributionSecurityController::new(
            AdapterContributionSecurityPolicy::default(),
        );
        let security = security_controller.assess_submission(
            &protocol,
            &artifact,
            &submission,
            provenance.clone(),
            1_045,
        )?;
        let bundle = AdapterContributionValidationBundle::new(
            submission, artifact, provenance, security, None,
        );
        Ok((protocol.window, bundle))
    }

    fn finalized_eval_run(
        eval_run_id: &str,
        mode: EvalRunMode,
        environment: psionic_environments::EnvironmentPackageKey,
        expected_sample_count: u64,
        score_bps: u32,
    ) -> Result<EvalRunState, Box<dyn std::error::Error>> {
        let mut run = EvalRunState::open(
            EvalRunContract::new(eval_run_id, mode, environment)
                .with_expected_sample_count(expected_sample_count),
        )?;
        run.start(10_000)?;
        run.append_sample(EvalSampleRecord {
            sample_id: String::from("sample-1"),
            ordinal: Some(1),
            environment: run.contract.environment.clone(),
            status: EvalSampleStatus::Passed,
            input_ref: Some(String::from("input://1")),
            output_ref: Some(String::from("output://1")),
            expected_output_ref: Some(String::from("expected://1")),
            score_bps: Some(score_bps),
            metrics: Vec::new(),
            artifacts: Vec::new(),
            error_reason: None,
            verification: None,
            session_digest: None,
            metadata: BTreeMap::new(),
        })?;
        run.finalize(10_010, Vec::new())?;
        Ok(run)
    }

    fn benchmark_summary(
        environment: psionic_environments::EnvironmentPackageKey,
        score_bps: u32,
    ) -> Result<psionic_eval::BenchmarkAggregateSummary, Box<dyn std::error::Error>> {
        let package = BenchmarkPackage::new(
            BenchmarkPackageKey::new("apple.adapter.benchmark", "2026.03"),
            "Apple Adapter Benchmark",
            environment.clone(),
            1,
            BenchmarkAggregationKind::MedianScore,
        )
        .with_cases(vec![BenchmarkCase::new("case-1")]);
        let mut round = EvalRunState::open(
            EvalRunContract::new("benchmark-round-1", EvalRunMode::Benchmark, environment)
                .with_expected_sample_count(1)
                .with_benchmark_package(package.key.clone()),
        )?;
        round.start(10_000)?;
        round.append_sample(EvalSampleRecord {
            sample_id: String::from("sample-1"),
            ordinal: Some(1),
            environment: round.contract.environment.clone(),
            status: EvalSampleStatus::Passed,
            input_ref: Some(String::from("input://1")),
            output_ref: Some(String::from("output://1")),
            expected_output_ref: Some(String::from("expected://1")),
            score_bps: Some(score_bps),
            metrics: Vec::new(),
            artifacts: Vec::new(),
            error_reason: None,
            verification: None,
            session_digest: None,
            metadata: BTreeMap::new(),
        })?;
        round.finalize(10_010, Vec::new())?;
        let mut execution = package.open_execution(BenchmarkExecutionMode::Validator)?;
        execution.record_round(&round)?;
        Ok(execution.finalize()?)
    }

    #[test]
    fn validator_accepts_replayed_contribution_and_scores_window()
    -> Result<(), Box<dyn std::error::Error>> {
        let signing_key = SigningKey::from_bytes(&[37_u8; 32]);
        let (mut window, mut bundle) = contribution_fixture(&signing_key)?;
        bundle.replay = Some(AdapterContributionReplayReceipt::new(
            bundle.artifact.contribution_id.clone(),
            bundle
                .submission
                .execution_summary
                .adapter_delta_digest
                .clone(),
            bundle
                .submission
                .execution_summary
                .adapter_delta_digest
                .clone(),
            1_046,
        ));
        let environment =
            psionic_environments::EnvironmentPackageKey::new("oa.apple.adapter", "2026.03");
        let held_out = finalized_eval_run(
            "heldout-1",
            EvalRunMode::OfflineHeldOut,
            environment.clone(),
            1,
            9_200,
        )?;
        let runtime_smoke = finalized_eval_run(
            "runtime-smoke-1",
            EvalRunMode::OnlineShadow,
            environment.clone(),
            1,
            10_000,
        )?;
        let benchmark = benchmark_summary(environment, 9_500)?;
        let candidate = AdapterWindowCandidateEvaluation {
            candidate_policy_revision: Some(PolicyRevision::new(
                "apple.weather.policy",
                "policy-r8",
                "policy-digest-r8",
                1_050,
            )),
            held_out_eval: Some(held_out),
            benchmark_summary: Some(benchmark),
            runtime_smoke_eval: Some(runtime_smoke),
        };
        let mut validator =
            AdapterContributionValidatorState::new(AdapterContributionValidatorPolicy {
                replay_sample_bps: 10_000,
                ..AdapterContributionValidatorPolicy::default()
            });
        let summary =
            validator.validate_window(&mut window, vec![bundle], Some(&candidate), 1_047)?;
        assert_eq!(summary.accepted_contributions, 1);
        assert_eq!(summary.rejected_contributions, 0);
        assert_eq!(summary.replay_required_contributions, 0);
        assert!(summary.promotion_ready);
        assert_eq!(window.status, crate::TrainingWindowStatus::Sealed);
        assert!(window.contributions[0].validator.is_some());
        Ok(())
    }

    #[test]
    fn validator_marks_missing_sampled_replay_as_replay_required()
    -> Result<(), Box<dyn std::error::Error>> {
        let signing_key = SigningKey::from_bytes(&[41_u8; 32]);
        let (mut window, bundle) = contribution_fixture(&signing_key)?;
        let mut validator =
            AdapterContributionValidatorState::new(AdapterContributionValidatorPolicy {
                replay_sample_bps: 10_000,
                ..AdapterContributionValidatorPolicy::default()
            });
        let summary = validator.validate_window(&mut window, vec![bundle], None, 1_047)?;
        assert_eq!(summary.accepted_contributions, 0);
        assert_eq!(summary.replay_required_contributions, 1);
        assert!(!summary.promotion_ready);
        assert_eq!(
            window.contributions[0]
                .validator
                .as_ref()
                .map(|receipt| receipt.disposition),
            Some(AdapterContributionValidatorDisposition::ReplayRequired)
        );
        Ok(())
    }

    #[test]
    fn apple_window_requires_runtime_smoke_before_promotion()
    -> Result<(), Box<dyn std::error::Error>> {
        let signing_key = SigningKey::from_bytes(&[43_u8; 32]);
        let (mut window, mut bundle) = contribution_fixture(&signing_key)?;
        bundle.replay = Some(AdapterContributionReplayReceipt::new(
            bundle.artifact.contribution_id.clone(),
            bundle
                .submission
                .execution_summary
                .adapter_delta_digest
                .clone(),
            bundle
                .submission
                .execution_summary
                .adapter_delta_digest
                .clone(),
            1_046,
        ));
        let environment =
            psionic_environments::EnvironmentPackageKey::new("oa.apple.adapter", "2026.03");
        let candidate = AdapterWindowCandidateEvaluation {
            candidate_policy_revision: Some(PolicyRevision::new(
                "apple.weather.policy",
                "policy-r8",
                "policy-digest-r8",
                1_050,
            )),
            held_out_eval: Some(finalized_eval_run(
                "heldout-1",
                EvalRunMode::OfflineHeldOut,
                environment.clone(),
                1,
                9_100,
            )?),
            benchmark_summary: Some(benchmark_summary(environment, 9_400)?),
            runtime_smoke_eval: None,
        };
        let mut validator =
            AdapterContributionValidatorState::new(AdapterContributionValidatorPolicy {
                replay_sample_bps: 10_000,
                ..AdapterContributionValidatorPolicy::default()
            });
        let summary =
            validator.validate_window(&mut window, vec![bundle], Some(&candidate), 1_047)?;
        assert_eq!(summary.accepted_contributions, 1);
        assert!(!summary.promotion_ready);
        assert!(
            summary
                .gate_reason_codes
                .contains(&AdapterWindowScoreReasonCode::RuntimeSmokeRequired)
        );
        Ok(())
    }

    #[test]
    fn missing_contribution_bundle_is_an_error() -> Result<(), Box<dyn std::error::Error>> {
        let signing_key = SigningKey::from_bytes(&[47_u8; 32]);
        let (mut window, _bundle) = contribution_fixture(&signing_key)?;
        let mut validator =
            AdapterContributionValidatorState::new(AdapterContributionValidatorPolicy::default());
        let error = validator
            .validate_window(&mut window, Vec::new(), None, 1_047)
            .expect_err("missing bundle should fail");
        assert!(matches!(
            error,
            AdapterValidationError::MissingBundle { .. }
        ));
        Ok(())
    }
}
