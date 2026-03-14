use std::collections::BTreeMap;

use psionic_environments::{EnvironmentPackageContract, EnvironmentPackageKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{RolloutArtifact, TrainingSftTraceArtifact, TrainingSftTraceKind};

/// Sampling channel for one training candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingSampleChannel {
    /// Offline dataset or trace replay.
    Offline,
    /// Online or live rollout collection.
    Online,
}

/// Difficulty rule used by the curriculum controller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingDifficultyTierRule {
    /// Difficulty tier admitted by the policy.
    pub difficulty_tier: String,
    /// Selection weight applied when the candidate is accepted.
    pub selection_weight_bps: u32,
}

/// Channel-specific sampling filter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TrainingSamplingFilter {
    /// Optional accepted trace kinds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_trace_kinds: Vec<TrainingSftTraceKind>,
    /// Required difficulty tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_tags: Vec<String>,
    /// Optional cap per source ref.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_samples_per_source: Option<u32>,
    /// Minimum long-context window when a trace declares one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_context_window_tokens: Option<u32>,
}

/// Digest-bound curriculum and filtering policy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingCurriculumPolicy {
    /// Difficulty tiers admitted by the policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tier_rules: Vec<TrainingDifficultyTierRule>,
    /// Whether rollout candidates must carry non-zero advantage.
    pub require_non_zero_advantage: bool,
    /// Minimum absolute aggregate advantage when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_abs_advantage_sum: Option<f32>,
    /// Suppress rollout samples below this absolute reward.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trivial_reward_abs_threshold: Option<f32>,
    /// Offline sampling filter.
    pub offline_filter: TrainingSamplingFilter,
    /// Online sampling filter.
    pub online_filter: TrainingSamplingFilter,
    /// Stable digest over the policy contents.
    pub policy_digest: String,
}

impl Default for TrainingCurriculumPolicy {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl TrainingCurriculumPolicy {
    /// Creates one curriculum policy.
    #[must_use]
    pub fn new(tier_rules: Vec<TrainingDifficultyTierRule>) -> Self {
        let mut policy = Self {
            tier_rules,
            require_non_zero_advantage: false,
            min_abs_advantage_sum: None,
            trivial_reward_abs_threshold: None,
            offline_filter: TrainingSamplingFilter::default(),
            online_filter: TrainingSamplingFilter::default(),
            policy_digest: String::new(),
        };
        policy.refresh_digest();
        policy
    }

    /// Enables the non-zero-advantage gate.
    #[must_use]
    pub fn with_non_zero_advantage(mut self, require_non_zero_advantage: bool) -> Self {
        self.require_non_zero_advantage = require_non_zero_advantage;
        self.refresh_digest();
        self
    }

    /// Attaches a minimum absolute advantage threshold.
    #[must_use]
    pub fn with_min_abs_advantage_sum(mut self, threshold: f32) -> Self {
        self.min_abs_advantage_sum = Some(threshold);
        self.refresh_digest();
        self
    }

    /// Attaches a trivial reward suppression threshold.
    #[must_use]
    pub fn with_trivial_reward_abs_threshold(mut self, threshold: f32) -> Self {
        self.trivial_reward_abs_threshold = Some(threshold);
        self.refresh_digest();
        self
    }

    /// Attaches the offline filter.
    #[must_use]
    pub fn with_offline_filter(mut self, filter: TrainingSamplingFilter) -> Self {
        self.offline_filter = filter;
        self.refresh_digest();
        self
    }

    /// Attaches the online filter.
    #[must_use]
    pub fn with_online_filter(mut self, filter: TrainingSamplingFilter) -> Self {
        self.online_filter = filter;
        self.refresh_digest();
        self
    }

    fn refresh_digest(&mut self) {
        self.policy_digest = stable_curriculum_policy_digest(self);
    }
}

/// One reproducible training candidate admitted or rejected by the curriculum controller.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingSampleCandidate {
    /// Stable sample identifier.
    pub sample_id: String,
    /// Stable source reference or digest.
    pub source_ref: String,
    /// Environment package identity.
    pub environment: EnvironmentPackageKey,
    /// Online versus offline sampling channel.
    pub channel: TrainingSampleChannel,
    /// Difficulty tier copied from environment metadata when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub difficulty_tier: Option<String>,
    /// Difficulty tags copied from environment metadata.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub difficulty_tags: Vec<String>,
    /// Trace kind when the candidate came from SFT traces.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_kind: Option<TrainingSftTraceKind>,
    /// Long-context window tokens when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<u32>,
    /// Aggregate reward when this is a rollout-derived candidate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reward_sum: Option<f32>,
    /// Aggregate advantage when this is a rollout-derived candidate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advantage_sum: Option<f32>,
    /// Stable lineage digest from the source candidate.
    pub lineage_digest: String,
}

impl TrainingSampleCandidate {
    /// Creates one training candidate from a typed SFT trace.
    pub fn from_sft_trace(
        trace: &TrainingSftTraceArtifact,
        package: &EnvironmentPackageContract,
        channel: TrainingSampleChannel,
    ) -> Result<Self, TrainingCurriculumError> {
        ensure_environment_matches(trace.environment.storage_key().as_str(), package)?;
        Ok(Self {
            sample_id: trace.trace_id.clone(),
            source_ref: trace
                .source_ref
                .clone()
                .unwrap_or_else(|| trace.trace_id.clone()),
            environment: trace.environment.clone(),
            channel,
            difficulty_tier: package
                .difficulty
                .as_ref()
                .map(|difficulty| difficulty.difficulty_tier.clone()),
            difficulty_tags: package
                .difficulty
                .as_ref()
                .map(|difficulty| difficulty.tags.clone())
                .unwrap_or_default(),
            trace_kind: Some(trace.trace_kind),
            context_window_tokens: trace
                .long_context_lineage
                .as_ref()
                .map(|lineage| lineage.context_window_tokens),
            reward_sum: None,
            advantage_sum: None,
            lineage_digest: trace.lineage_digest.clone(),
        })
    }

    /// Creates one training candidate from a rollout artifact.
    pub fn from_rollout(
        rollout: &RolloutArtifact,
        package: &EnvironmentPackageContract,
        channel: TrainingSampleChannel,
    ) -> Result<Self, TrainingCurriculumError> {
        ensure_environment_matches(rollout.environment.storage_key().as_str(), package)?;
        Ok(Self {
            sample_id: rollout.artifact_id.clone(),
            source_ref: rollout.task_id.clone(),
            environment: rollout.environment.clone(),
            channel,
            difficulty_tier: package
                .difficulty
                .as_ref()
                .map(|difficulty| difficulty.difficulty_tier.clone()),
            difficulty_tags: package
                .difficulty
                .as_ref()
                .map(|difficulty| difficulty.tags.clone())
                .unwrap_or_default(),
            trace_kind: None,
            context_window_tokens: None,
            reward_sum: Some(rollout.reward_sum()),
            advantage_sum: Some(rollout.advantage_sum()),
            lineage_digest: rollout.artifact_digest.clone(),
        })
    }
}

/// Acceptance versus rejection posture for one candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingSampleFilterDisposition {
    /// Candidate was accepted.
    Accepted,
    /// Candidate was rejected.
    Rejected,
}

/// Explicit reason code attached to one curriculum decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingSampleFilterReasonCode {
    Accepted,
    MissingDifficultyMetadata,
    DifficultyTierFiltered,
    RequiredTagMissing,
    TraceKindFiltered,
    LongContextBelowMinimum,
    ZeroAdvantageRejected,
    AdvantageBelowThreshold,
    TrivialRewardSuppressed,
    SourceBudgetExceeded,
}

/// Receipt for one curriculum decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingSampleFilterReceipt {
    /// Stable sample identifier.
    pub sample_id: String,
    /// Sampling channel.
    pub channel: TrainingSampleChannel,
    /// Stable source ref.
    pub source_ref: String,
    /// Acceptance versus rejection.
    pub disposition: TrainingSampleFilterDisposition,
    /// Explicit reason code.
    pub reason_code: TrainingSampleFilterReasonCode,
    /// Selection weight when the candidate was accepted.
    pub selection_weight_bps: u32,
    /// Stable policy digest.
    pub policy_digest: String,
}

/// Summary receipt over one deterministic selection pass.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingCurriculumSelectionReceipt {
    /// Stable policy digest.
    pub policy_digest: String,
    /// Number of evaluated candidates.
    pub evaluated_count: u32,
    /// Accepted candidate receipts.
    pub accepted: Vec<TrainingSampleFilterReceipt>,
    /// Rejected candidate receipts.
    pub rejected: Vec<TrainingSampleFilterReceipt>,
    /// Stable digest over the selection result.
    pub selection_digest: String,
}

/// Persistent controller state for reproducible sampling.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingCurriculumState {
    /// Accepted count per source ref.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub accepted_by_source: BTreeMap<String, u32>,
}

/// Runtime error for curriculum candidate construction.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TrainingCurriculumError {
    /// The provided package key does not match the candidate environment.
    #[error("training curriculum environment mismatch: expected `{expected}`, found `{actual}`")]
    EnvironmentMismatch { expected: String, actual: String },
}

/// Stateful curriculum controller over difficulty, filtering, and advantage gates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingCurriculumController {
    /// Digest-bound policy.
    pub policy: TrainingCurriculumPolicy,
    /// Persistent selection state.
    pub state: TrainingCurriculumState,
}

impl TrainingCurriculumController {
    /// Creates one curriculum controller.
    #[must_use]
    pub fn new(policy: TrainingCurriculumPolicy) -> Self {
        Self {
            policy,
            state: TrainingCurriculumState::default(),
        }
    }

    /// Evaluates one candidate and updates controller state when it is accepted.
    pub fn evaluate_candidate(
        &mut self,
        candidate: &TrainingSampleCandidate,
    ) -> TrainingSampleFilterReceipt {
        let filter = match candidate.channel {
            TrainingSampleChannel::Offline => &self.policy.offline_filter,
            TrainingSampleChannel::Online => &self.policy.online_filter,
        };

        let tier_weight = if self.policy.tier_rules.is_empty() {
            Some(10_000)
        } else {
            let Some(difficulty_tier) = candidate.difficulty_tier.as_deref() else {
                return self.rejected(
                    candidate,
                    TrainingSampleFilterReasonCode::MissingDifficultyMetadata,
                );
            };
            self.policy
                .tier_rules
                .iter()
                .find(|rule| rule.difficulty_tier == difficulty_tier)
                .map(|rule| rule.selection_weight_bps)
        };
        let Some(selection_weight_bps) = tier_weight else {
            return self.rejected(
                candidate,
                TrainingSampleFilterReasonCode::DifficultyTierFiltered,
            );
        };
        if !filter.required_tags.is_empty()
            && !filter
                .required_tags
                .iter()
                .all(|tag| candidate.difficulty_tags.contains(tag))
        {
            return self.rejected(
                candidate,
                TrainingSampleFilterReasonCode::RequiredTagMissing,
            );
        }
        if !filter.allowed_trace_kinds.is_empty()
            && candidate
                .trace_kind
                .is_some_and(|kind| !filter.allowed_trace_kinds.contains(&kind))
        {
            return self.rejected(candidate, TrainingSampleFilterReasonCode::TraceKindFiltered);
        }
        if filter.min_context_window_tokens.is_some_and(|minimum| {
            candidate
                .context_window_tokens
                .unwrap_or_default()
                .lt(&minimum)
        }) {
            return self.rejected(
                candidate,
                TrainingSampleFilterReasonCode::LongContextBelowMinimum,
            );
        }
        if self.policy.require_non_zero_advantage
            && candidate
                .advantage_sum
                .is_some_and(|advantage_sum| advantage_sum.abs() == 0.0)
        {
            return self.rejected(
                candidate,
                TrainingSampleFilterReasonCode::ZeroAdvantageRejected,
            );
        }
        if self.policy.min_abs_advantage_sum.is_some_and(|threshold| {
            candidate
                .advantage_sum
                .is_some_and(|advantage_sum| advantage_sum.abs() < threshold)
        }) {
            return self.rejected(
                candidate,
                TrainingSampleFilterReasonCode::AdvantageBelowThreshold,
            );
        }
        if self
            .policy
            .trivial_reward_abs_threshold
            .is_some_and(|threshold| {
                candidate
                    .reward_sum
                    .is_some_and(|reward_sum| reward_sum.abs() < threshold)
            })
        {
            return self.rejected(
                candidate,
                TrainingSampleFilterReasonCode::TrivialRewardSuppressed,
            );
        }
        if filter.max_samples_per_source.is_some_and(|limit| {
            self.state
                .accepted_by_source
                .get(candidate.source_ref.as_str())
                .copied()
                .unwrap_or_default()
                >= limit
        }) {
            return self.rejected(
                candidate,
                TrainingSampleFilterReasonCode::SourceBudgetExceeded,
            );
        }

        let receipt = TrainingSampleFilterReceipt {
            sample_id: candidate.sample_id.clone(),
            channel: candidate.channel,
            source_ref: candidate.source_ref.clone(),
            disposition: TrainingSampleFilterDisposition::Accepted,
            reason_code: TrainingSampleFilterReasonCode::Accepted,
            selection_weight_bps,
            policy_digest: self.policy.policy_digest.clone(),
        };
        *self
            .state
            .accepted_by_source
            .entry(candidate.source_ref.clone())
            .or_default() += 1;
        receipt
    }

    /// Evaluates a deterministic list of candidates and returns one summary receipt.
    pub fn apply_candidates(
        &mut self,
        candidates: &[TrainingSampleCandidate],
    ) -> TrainingCurriculumSelectionReceipt {
        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        for candidate in candidates {
            let receipt = self.evaluate_candidate(candidate);
            match receipt.disposition {
                TrainingSampleFilterDisposition::Accepted => accepted.push(receipt),
                TrainingSampleFilterDisposition::Rejected => rejected.push(receipt),
            }
        }
        let selection_digest = stable_selection_digest(
            self.policy.policy_digest.as_str(),
            accepted.as_slice(),
            rejected.as_slice(),
        );
        TrainingCurriculumSelectionReceipt {
            policy_digest: self.policy.policy_digest.clone(),
            evaluated_count: candidates.len() as u32,
            accepted,
            rejected,
            selection_digest,
        }
    }

    fn rejected(
        &self,
        candidate: &TrainingSampleCandidate,
        reason_code: TrainingSampleFilterReasonCode,
    ) -> TrainingSampleFilterReceipt {
        TrainingSampleFilterReceipt {
            sample_id: candidate.sample_id.clone(),
            channel: candidate.channel,
            source_ref: candidate.source_ref.clone(),
            disposition: TrainingSampleFilterDisposition::Rejected,
            reason_code,
            selection_weight_bps: 0,
            policy_digest: self.policy.policy_digest.clone(),
        }
    }
}

fn ensure_environment_matches(
    expected: &str,
    package: &EnvironmentPackageContract,
) -> Result<(), TrainingCurriculumError> {
    let actual = package.storage_key();
    if expected != actual {
        return Err(TrainingCurriculumError::EnvironmentMismatch {
            expected: String::from(expected),
            actual,
        });
    }
    Ok(())
}

fn stable_curriculum_policy_digest(policy: &TrainingCurriculumPolicy) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_curriculum_policy|");
    for tier_rule in &policy.tier_rules {
        hasher.update(tier_rule.difficulty_tier.as_bytes());
        hasher.update(b"|");
        hasher.update(tier_rule.selection_weight_bps.to_string().as_bytes());
    }
    if policy.require_non_zero_advantage {
        hasher.update(b"|non_zero_advantage|true");
    } else {
        hasher.update(b"|non_zero_advantage|false");
    }
    if let Some(threshold) = policy.min_abs_advantage_sum {
        hasher.update(b"|min_advantage|");
        hasher.update(threshold.to_bits().to_le_bytes());
    }
    if let Some(threshold) = policy.trivial_reward_abs_threshold {
        hasher.update(b"|trivial_reward|");
        hasher.update(threshold.to_bits().to_le_bytes());
    }
    update_sampling_filter_digest(&mut hasher, b"offline", &policy.offline_filter);
    update_sampling_filter_digest(&mut hasher, b"online", &policy.online_filter);
    hex::encode(hasher.finalize())
}

fn update_sampling_filter_digest(
    hasher: &mut Sha256,
    label: &[u8],
    filter: &TrainingSamplingFilter,
) {
    hasher.update(b"|");
    hasher.update(label);
    for trace_kind in &filter.allowed_trace_kinds {
        hasher.update(b"|trace_kind|");
        hasher.update(format!("{trace_kind:?}").as_bytes());
    }
    for tag in &filter.required_tags {
        hasher.update(b"|tag|");
        hasher.update(tag.as_bytes());
    }
    if let Some(max_samples_per_source) = filter.max_samples_per_source {
        hasher.update(b"|source_limit|");
        hasher.update(max_samples_per_source.to_string().as_bytes());
    }
    if let Some(min_context_window_tokens) = filter.min_context_window_tokens {
        hasher.update(b"|min_context|");
        hasher.update(min_context_window_tokens.to_string().as_bytes());
    }
}

fn stable_selection_digest(
    policy_digest: &str,
    accepted: &[TrainingSampleFilterReceipt],
    rejected: &[TrainingSampleFilterReceipt],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_curriculum_selection|");
    hasher.update(policy_digest.as_bytes());
    for receipt in accepted {
        hasher.update(b"|accepted|");
        hasher.update(receipt.sample_id.as_bytes());
        hasher.update(b"|");
        hasher.update(receipt.source_ref.as_bytes());
    }
    for receipt in rejected {
        hasher.update(b"|rejected|");
        hasher.update(receipt.sample_id.as_bytes());
        hasher.update(b"|");
        hasher.update(format!("{:?}", receipt.reason_code).as_bytes());
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TrainingCurriculumController, TrainingCurriculumPolicy, TrainingDifficultyTierRule,
        TrainingSampleCandidate, TrainingSampleChannel, TrainingSampleFilterDisposition,
        TrainingSampleFilterReasonCode, TrainingSamplingFilter,
    };
    use crate::{
        PolicyRevision, RolloutArtifact, RolloutProofKind, RolloutProofReference, RolloutSample,
        RolloutTerminationReason, TrainingLongContextTraceLineage, TrainingSftTraceArtifact,
        TrainingSftTraceKind, TrainingToolCallTraceLineage, TrainingToolCallTraceStep,
    };
    use psionic_environments::{
        EnvironmentDifficultyMetadata, EnvironmentExecutionEntrypoint, EnvironmentPackageContract,
        EnvironmentPackageFamily, EnvironmentPackageKey, EnvironmentRuntimeFamily,
        EnvironmentStateMode,
    };

    fn environment_package() -> EnvironmentPackageContract {
        EnvironmentPackageContract::new(
            EnvironmentPackageKey::new("env.weather", "2026.03.14"),
            EnvironmentPackageFamily::Agentic,
            "Weather",
            EnvironmentExecutionEntrypoint {
                runtime_family: EnvironmentRuntimeFamily::MultiTurnDialog,
                entrypoint: String::from("weather::run"),
                args: Vec::new(),
                sandbox_profile_ref: None,
                max_turns: 2,
                state_mode: EnvironmentStateMode::SessionPersistent,
                time_budget_ms: None,
            },
        )
        .with_difficulty(EnvironmentDifficultyMetadata {
            difficulty_tier: String::from("intermediate"),
            min_agent_level: Some(2),
            tags: vec![String::from("tool_use"), String::from("weather")],
        })
    }

    fn policy() -> TrainingCurriculumPolicy {
        TrainingCurriculumPolicy::new(vec![TrainingDifficultyTierRule {
            difficulty_tier: String::from("intermediate"),
            selection_weight_bps: 7_500,
        }])
        .with_non_zero_advantage(true)
        .with_min_abs_advantage_sum(0.2)
        .with_trivial_reward_abs_threshold(0.1)
        .with_offline_filter(TrainingSamplingFilter {
            allowed_trace_kinds: vec![
                TrainingSftTraceKind::ToolCall,
                TrainingSftTraceKind::LongContext,
            ],
            required_tags: vec![String::from("tool_use")],
            max_samples_per_source: Some(1),
            min_context_window_tokens: Some(4_096),
        })
        .with_online_filter(TrainingSamplingFilter {
            allowed_trace_kinds: Vec::new(),
            required_tags: vec![String::from("weather")],
            max_samples_per_source: Some(2),
            min_context_window_tokens: None,
        })
    }

    #[test]
    fn curriculum_accepts_tool_call_trace_and_non_zero_advantage_rollout()
    -> Result<(), Box<dyn std::error::Error>> {
        let package = environment_package();
        let trace = TrainingSftTraceArtifact::new(
            "trace-tool",
            package.key.clone(),
            TrainingSftTraceKind::LongContext,
            "input",
            "output",
        )
        .with_long_context_lineage(TrainingLongContextTraceLineage::new(
            8_192,
            vec![String::from("seg-1"), String::from("seg-2")],
        ));
        let rollout = RolloutArtifact::new(
            "rollout-1",
            "worker-a",
            package.key.clone(),
            "task-1",
            PolicyRevision::new("policy.weather", "rev-1", "digest-1", 1_000),
            vec![RolloutSample::new(1, -0.2, 0.3, 0.4)],
            RolloutTerminationReason::Completed,
            vec![RolloutProofReference::new(
                RolloutProofKind::ExecutionProof,
                "proof-1",
                "artifact://proof-1",
            )],
            1_100,
        )?;

        let trace_candidate = TrainingSampleCandidate::from_sft_trace(
            &trace,
            &package,
            TrainingSampleChannel::Offline,
        )?;
        let rollout_candidate = TrainingSampleCandidate::from_rollout(
            &rollout,
            &package,
            TrainingSampleChannel::Online,
        )?;

        let mut controller = TrainingCurriculumController::new(policy());
        let receipt = controller.apply_candidates(&[trace_candidate, rollout_candidate]);
        assert_eq!(receipt.accepted.len(), 2);
        assert_eq!(receipt.rejected.len(), 0);
        assert_eq!(receipt.accepted[0].selection_weight_bps, 7_500);
        Ok(())
    }

    #[test]
    fn curriculum_rejects_zero_advantage_rollout() -> Result<(), Box<dyn std::error::Error>> {
        let package = environment_package();
        let rollout = RolloutArtifact::new(
            "rollout-1",
            "worker-a",
            package.key.clone(),
            "task-1",
            PolicyRevision::new("policy.weather", "rev-1", "digest-1", 1_000),
            vec![RolloutSample::new(1, -0.2, 0.01, 0.0)],
            RolloutTerminationReason::Completed,
            vec![RolloutProofReference::new(
                RolloutProofKind::ExecutionProof,
                "proof-1",
                "artifact://proof-1",
            )],
            1_100,
        )?;
        let candidate = TrainingSampleCandidate::from_rollout(
            &rollout,
            &package,
            TrainingSampleChannel::Online,
        )?;
        let mut controller = TrainingCurriculumController::new(policy());
        let receipt = controller.evaluate_candidate(&candidate);
        assert_eq!(
            receipt.disposition,
            TrainingSampleFilterDisposition::Rejected
        );
        assert_eq!(
            receipt.reason_code,
            TrainingSampleFilterReasonCode::ZeroAdvantageRejected
        );
        Ok(())
    }

    #[test]
    fn curriculum_enforces_source_budget_for_offline_traces()
    -> Result<(), Box<dyn std::error::Error>> {
        let package = environment_package();
        let first_trace = TrainingSftTraceArtifact::new(
            "trace-1",
            package.key.clone(),
            TrainingSftTraceKind::LongContext,
            "input-a",
            "output-a",
        )
        .with_source_ref("dataset://weather/offline")
        .with_long_context_lineage(TrainingLongContextTraceLineage::new(
            8_192,
            vec![String::from("seg-1")],
        ));
        let second_trace = TrainingSftTraceArtifact::new(
            "trace-2",
            package.key.clone(),
            TrainingSftTraceKind::LongContext,
            "input-b",
            "output-b",
        )
        .with_source_ref("dataset://weather/offline")
        .with_long_context_lineage(TrainingLongContextTraceLineage::new(
            8_192,
            vec![String::from("seg-2")],
        ));
        let first_candidate = TrainingSampleCandidate::from_sft_trace(
            &first_trace,
            &package,
            TrainingSampleChannel::Offline,
        )?;
        let second_candidate = TrainingSampleCandidate::from_sft_trace(
            &second_trace,
            &package,
            TrainingSampleChannel::Offline,
        )?;

        let mut controller = TrainingCurriculumController::new(policy());
        let first = controller.evaluate_candidate(&first_candidate);
        let second = controller.evaluate_candidate(&second_candidate);
        assert_eq!(first.disposition, TrainingSampleFilterDisposition::Accepted);
        assert_eq!(
            second.disposition,
            TrainingSampleFilterDisposition::Rejected
        );
        assert_eq!(
            second.reason_code,
            TrainingSampleFilterReasonCode::SourceBudgetExceeded
        );
        Ok(())
    }

    #[test]
    fn curriculum_refuses_environment_mismatch() {
        let package = environment_package();
        let mismatched = TrainingSftTraceArtifact::new(
            "trace-1",
            EnvironmentPackageKey::new("env.other", "2026.03.14"),
            TrainingSftTraceKind::ToolCall,
            "input",
            "output",
        )
        .with_tool_call_lineage(TrainingToolCallTraceLineage::new(vec![
            TrainingToolCallTraceStep {
                tool_name: String::from("get_weather"),
                arguments_digest: String::from("args"),
                result_digest: String::from("result"),
            },
        ]));
        let error = TrainingSampleCandidate::from_sft_trace(
            &mismatched,
            &package,
            TrainingSampleChannel::Offline,
        )
        .expect_err("mismatched environments should fail");
        assert_eq!(
            error,
            super::TrainingCurriculumError::EnvironmentMismatch {
                expected: String::from("env.other@2026.03.14"),
                actual: package.storage_key(),
            }
        );
    }
}
