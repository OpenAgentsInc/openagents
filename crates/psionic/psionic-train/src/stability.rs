use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{RolloutIngestionTelemetry, TrainingStepReceipt};

/// Final operator posture emitted by train safety policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingOperationalAction {
    Continue,
    Quarantine,
    Halt,
}

/// One risky runtime optimization that must be explicitly policy-gated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingRiskyOptimization {
    AsyncCheckpointOverlap,
    AggressiveOffPolicyReuse,
    UnboundedSandboxReuse,
    MixedPrecisionLossScaleBypass,
}

/// One instability signal tracked by the safety controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingInstabilitySignalKind {
    MaxGradientNormL2,
    MeanClippingRatio,
    EntropyDriftBps,
    StaleRolloutDropRateBps,
    CheckpointCatchupLatencyMs,
    TopologyChurnEvents,
    EnvironmentFailureRateBps,
    SandboxFailureRateBps,
}

/// Aggregated instability telemetry over the current training slice.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TrainingInstabilityTelemetry {
    /// Maximum observed group gradient norm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_gradient_norm_l2: Option<f32>,
    /// Mean clipping ratio across groups that clipped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_clipping_ratio: Option<f32>,
    /// Entropy drift in basis points when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entropy_drift_bps: Option<u32>,
    /// Discarded rollout rate in basis points.
    pub stale_rollout_drop_rate_bps: u32,
    /// Checkpoint catch-up latency.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_catchup_latency_ms: Option<u64>,
    /// Observed topology churn events over the evaluation window.
    pub topology_churn_events: u32,
    /// Environment failures in basis points.
    pub environment_failure_rate_bps: u32,
    /// Sandbox failures in basis points.
    pub sandbox_failure_rate_bps: u32,
}

impl TrainingInstabilityTelemetry {
    /// Derives telemetry from existing step receipts and rollout ingestion telemetry.
    #[must_use]
    pub fn from_step_receipts_and_rollouts(
        receipts: &[TrainingStepReceipt],
        rollout_telemetry: &RolloutIngestionTelemetry,
    ) -> Self {
        let mut max_gradient_norm_l2 = None;
        let mut clipping_ratios = Vec::new();
        for receipt in receipts {
            for group in &receipt.group_telemetry {
                max_gradient_norm_l2 = Some(
                    max_gradient_norm_l2
                        .unwrap_or(group.gradient_norm_l2)
                        .max(group.gradient_norm_l2),
                );
                if let Some(clipping_ratio) = group.clipping_ratio {
                    clipping_ratios.push(clipping_ratio);
                }
            }
        }
        let mean_clipping_ratio = if clipping_ratios.is_empty() {
            None
        } else {
            Some(clipping_ratios.iter().sum::<f32>() / clipping_ratios.len() as f32)
        };
        let dropped = rollout_telemetry.discarded_rollout_count as u32;
        let total = rollout_telemetry.accepted_exact_rollout_count as u32
            + rollout_telemetry.accepted_off_policy_rollout_count as u32
            + rollout_telemetry.quarantined_rollout_count as u32
            + rollout_telemetry.discarded_rollout_count as u32;
        let stale_rollout_drop_rate_bps = if total == 0 {
            0
        } else {
            (dropped.saturating_mul(10_000)) / total
        };
        Self {
            max_gradient_norm_l2,
            mean_clipping_ratio,
            entropy_drift_bps: None,
            stale_rollout_drop_rate_bps,
            checkpoint_catchup_latency_ms: None,
            topology_churn_events: 0,
            environment_failure_rate_bps: 0,
            sandbox_failure_rate_bps: 0,
        }
    }

    /// Attaches entropy drift telemetry.
    #[must_use]
    pub const fn with_entropy_drift_bps(mut self, entropy_drift_bps: u32) -> Self {
        self.entropy_drift_bps = Some(entropy_drift_bps);
        self
    }

    /// Attaches checkpoint catch-up latency telemetry.
    #[must_use]
    pub const fn with_checkpoint_catchup_latency_ms(
        mut self,
        checkpoint_catchup_latency_ms: u64,
    ) -> Self {
        self.checkpoint_catchup_latency_ms = Some(checkpoint_catchup_latency_ms);
        self
    }

    /// Attaches topology churn telemetry.
    #[must_use]
    pub const fn with_topology_churn_events(mut self, topology_churn_events: u32) -> Self {
        self.topology_churn_events = topology_churn_events;
        self
    }

    /// Attaches environment failure-rate telemetry.
    #[must_use]
    pub const fn with_environment_failure_rate_bps(
        mut self,
        environment_failure_rate_bps: u32,
    ) -> Self {
        self.environment_failure_rate_bps = environment_failure_rate_bps;
        self
    }

    /// Attaches sandbox failure-rate telemetry.
    #[must_use]
    pub const fn with_sandbox_failure_rate_bps(mut self, sandbox_failure_rate_bps: u32) -> Self {
        self.sandbox_failure_rate_bps = sandbox_failure_rate_bps;
        self
    }
}

/// Rule mapping a telemetry signal to a threshold and action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingInstabilityRule {
    /// Signal this rule monitors.
    pub signal: TrainingInstabilitySignalKind,
    /// Maximum admitted value before the rule fires.
    pub max_value: f64,
    /// Action taken when the signal exceeds the threshold.
    pub action: TrainingOperationalAction,
}

/// Policy for one risky runtime optimization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingRiskyOptimizationRule {
    /// Optimization being controlled.
    pub optimization: TrainingRiskyOptimization,
    /// Action applied when the optimization is requested.
    pub action: TrainingOperationalAction,
}

/// Digest-bound safety and risky-optimization policy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct TrainingInstabilityPolicy {
    /// Threshold rules over telemetry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telemetry_rules: Vec<TrainingInstabilityRule>,
    /// Explicit risky-optimization policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risky_optimization_rules: Vec<TrainingRiskyOptimizationRule>,
    /// Stable policy digest.
    pub policy_digest: String,
}

impl TrainingInstabilityPolicy {
    /// Creates one safety policy.
    #[must_use]
    pub fn new(
        telemetry_rules: Vec<TrainingInstabilityRule>,
        risky_optimization_rules: Vec<TrainingRiskyOptimizationRule>,
    ) -> Self {
        let mut policy = Self {
            telemetry_rules,
            risky_optimization_rules,
            policy_digest: String::new(),
        };
        policy.policy_digest = stable_instability_policy_digest(&policy);
        policy
    }
}

/// One fired telemetry signal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingInstabilitySignalReceipt {
    /// Signal that breached policy.
    pub signal: TrainingInstabilitySignalKind,
    /// Observed signal value.
    pub observed_value: f64,
    /// Threshold value.
    pub threshold_value: f64,
    /// Action selected by the rule.
    pub action: TrainingOperationalAction,
}

/// One risky-optimization decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingRiskyOptimizationReceipt {
    /// Optimization being evaluated.
    pub optimization: TrainingRiskyOptimization,
    /// Action applied by the policy.
    pub action: TrainingOperationalAction,
    /// Whether the optimization is allowed to proceed.
    pub allowed: bool,
}

/// Final verdict over telemetry plus requested risky optimizations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingStabilityVerdict {
    /// Stable policy digest.
    pub policy_digest: String,
    /// Final posture.
    pub action: TrainingOperationalAction,
    /// Fired signal receipts.
    pub signal_receipts: Vec<TrainingInstabilitySignalReceipt>,
    /// Optimization decisions.
    pub optimization_receipts: Vec<TrainingRiskyOptimizationReceipt>,
    /// Stable digest over the verdict contents.
    pub verdict_digest: String,
}

/// Safety controller that evaluates telemetry and risky optimizations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingStabilityController {
    /// Digest-bound safety policy.
    pub policy: TrainingInstabilityPolicy,
}

impl TrainingStabilityController {
    /// Creates one controller.
    #[must_use]
    pub fn new(policy: TrainingInstabilityPolicy) -> Self {
        Self { policy }
    }

    /// Evaluates telemetry and requested risky optimizations.
    #[must_use]
    pub fn evaluate(
        &self,
        telemetry: &TrainingInstabilityTelemetry,
        requested_optimizations: &[TrainingRiskyOptimization],
    ) -> TrainingStabilityVerdict {
        let mut signal_receipts = Vec::new();
        let mut action = TrainingOperationalAction::Continue;
        for rule in &self.policy.telemetry_rules {
            let observed_value = signal_value(rule.signal, telemetry);
            if observed_value > rule.max_value {
                signal_receipts.push(TrainingInstabilitySignalReceipt {
                    signal: rule.signal,
                    observed_value,
                    threshold_value: rule.max_value,
                    action: rule.action,
                });
                action = action.max(rule.action);
            }
        }

        let mut optimization_receipts = Vec::new();
        for optimization in requested_optimizations {
            let applied_action = self
                .policy
                .risky_optimization_rules
                .iter()
                .find(|rule| rule.optimization == *optimization)
                .map(|rule| rule.action)
                .unwrap_or(TrainingOperationalAction::Halt);
            let allowed = applied_action == TrainingOperationalAction::Continue;
            optimization_receipts.push(TrainingRiskyOptimizationReceipt {
                optimization: *optimization,
                action: applied_action,
                allowed,
            });
            action = action.max(applied_action);
        }

        let verdict_digest = stable_stability_verdict_digest(
            self.policy.policy_digest.as_str(),
            action,
            signal_receipts.as_slice(),
            optimization_receipts.as_slice(),
        );
        TrainingStabilityVerdict {
            policy_digest: self.policy.policy_digest.clone(),
            action,
            signal_receipts,
            optimization_receipts,
            verdict_digest,
        }
    }
}

fn signal_value(
    signal: TrainingInstabilitySignalKind,
    telemetry: &TrainingInstabilityTelemetry,
) -> f64 {
    match signal {
        TrainingInstabilitySignalKind::MaxGradientNormL2 => {
            telemetry.max_gradient_norm_l2.unwrap_or_default() as f64
        }
        TrainingInstabilitySignalKind::MeanClippingRatio => {
            telemetry.mean_clipping_ratio.unwrap_or_default() as f64
        }
        TrainingInstabilitySignalKind::EntropyDriftBps => {
            telemetry.entropy_drift_bps.unwrap_or_default() as f64
        }
        TrainingInstabilitySignalKind::StaleRolloutDropRateBps => {
            telemetry.stale_rollout_drop_rate_bps as f64
        }
        TrainingInstabilitySignalKind::CheckpointCatchupLatencyMs => {
            telemetry.checkpoint_catchup_latency_ms.unwrap_or_default() as f64
        }
        TrainingInstabilitySignalKind::TopologyChurnEvents => {
            telemetry.topology_churn_events as f64
        }
        TrainingInstabilitySignalKind::EnvironmentFailureRateBps => {
            telemetry.environment_failure_rate_bps as f64
        }
        TrainingInstabilitySignalKind::SandboxFailureRateBps => {
            telemetry.sandbox_failure_rate_bps as f64
        }
    }
}

fn stable_instability_policy_digest(policy: &TrainingInstabilityPolicy) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_instability_policy|");
    for rule in &policy.telemetry_rules {
        hasher.update(format!("{:?}", rule.signal).as_bytes());
        hasher.update(b"|");
        hasher.update(rule.max_value.to_bits().to_le_bytes());
        hasher.update(b"|");
        hasher.update(format!("{:?}", rule.action).as_bytes());
    }
    for rule in &policy.risky_optimization_rules {
        hasher.update(b"|optimization|");
        hasher.update(format!("{:?}", rule.optimization).as_bytes());
        hasher.update(b"|");
        hasher.update(format!("{:?}", rule.action).as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_stability_verdict_digest(
    policy_digest: &str,
    action: TrainingOperationalAction,
    signal_receipts: &[TrainingInstabilitySignalReceipt],
    optimization_receipts: &[TrainingRiskyOptimizationReceipt],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_stability_verdict|");
    hasher.update(policy_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", action).as_bytes());
    for receipt in signal_receipts {
        hasher.update(b"|signal|");
        hasher.update(format!("{:?}", receipt.signal).as_bytes());
        hasher.update(b"|");
        hasher.update(receipt.observed_value.to_bits().to_le_bytes());
        hasher.update(b"|");
        hasher.update(format!("{:?}", receipt.action).as_bytes());
    }
    for receipt in optimization_receipts {
        hasher.update(b"|optimization|");
        hasher.update(format!("{:?}", receipt.optimization).as_bytes());
        hasher.update(b"|");
        hasher.update(format!("{:?}", receipt.action).as_bytes());
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TrainingInstabilityPolicy, TrainingInstabilityRule, TrainingInstabilitySignalKind,
        TrainingOperationalAction, TrainingRiskyOptimization, TrainingRiskyOptimizationRule,
        TrainingStabilityController,
    };
    use crate::{
        OptimizerStateResidency, RolloutIngestionTelemetry, TrainingGroupTelemetry,
        TrainingOptimizerKind, TrainingParameterClass, TrainingStepExecutionMode,
        TrainingStepReceipt, TrainingStepSchedule, TrainingStepTiming,
    };

    fn step_receipt(gradient_norm_l2: f32, clipping_ratio: Option<f32>) -> TrainingStepReceipt {
        TrainingStepReceipt {
            receipt_id: String::from("receipt-1"),
            run_id: String::from("run-1"),
            checkpoint_family: String::from("train.weather"),
            execution_mode: TrainingStepExecutionMode::ExplicitGradientBatch,
            schedule: TrainingStepSchedule {
                global_step: 1,
                window_index: 1,
                step_in_window: 1,
                cadence_index: 1,
                window_in_cadence: 1,
            },
            batch_id: String::from("batch-1"),
            loss: 0.5,
            sample_count: 1,
            restore_source: None,
            group_telemetry: vec![TrainingGroupTelemetry {
                group_id: String::from("decoder"),
                class: TrainingParameterClass::Head,
                optimizer: TrainingOptimizerKind::Sgd,
                gradient_norm_l2,
                clipped_gradient_norm_l2: gradient_norm_l2 * clipping_ratio.unwrap_or(1.0),
                clipping_ratio,
                update_norm_l2: 0.1,
                parameter_norm_l2: 2.0,
                residency_before: OptimizerStateResidency::HostResident,
                residency_after: OptimizerStateResidency::HostResident,
            }],
            residency_transitions: Vec::new(),
            timing: TrainingStepTiming {
                started_at_ms: 1_000,
                finished_at_ms: 1_010,
                duration_ms: 10,
            },
            receipt_digest: String::from("digest-1"),
        }
    }

    #[test]
    fn stability_controller_derives_quarantine_from_rollout_drop_rate_and_risky_optimization() {
        let telemetry = super::TrainingInstabilityTelemetry::from_step_receipts_and_rollouts(
            &[step_receipt(1.2, Some(0.8))],
            &RolloutIngestionTelemetry {
                accepted_exact_rollout_count: 2,
                accepted_off_policy_rollout_count: 0,
                quarantined_rollout_count: 0,
                discarded_rollout_count: 1,
                accepted_token_count: 2,
                quarantined_token_count: 0,
                discarded_token_count: 1,
            },
        )
        .with_entropy_drift_bps(250)
        .with_checkpoint_catchup_latency_ms(200)
        .with_topology_churn_events(1);
        let policy = TrainingInstabilityPolicy::new(
            vec![
                TrainingInstabilityRule {
                    signal: TrainingInstabilitySignalKind::StaleRolloutDropRateBps,
                    max_value: 2_000.0,
                    action: TrainingOperationalAction::Quarantine,
                },
                TrainingInstabilityRule {
                    signal: TrainingInstabilitySignalKind::MaxGradientNormL2,
                    max_value: 5.0,
                    action: TrainingOperationalAction::Halt,
                },
            ],
            vec![TrainingRiskyOptimizationRule {
                optimization: TrainingRiskyOptimization::AggressiveOffPolicyReuse,
                action: TrainingOperationalAction::Quarantine,
            }],
        );
        let verdict = TrainingStabilityController::new(policy).evaluate(
            &telemetry,
            &[TrainingRiskyOptimization::AggressiveOffPolicyReuse],
        );
        assert_eq!(verdict.action, TrainingOperationalAction::Quarantine);
        assert_eq!(verdict.signal_receipts.len(), 1);
        assert_eq!(verdict.optimization_receipts.len(), 1);
    }

    #[test]
    fn stability_controller_halts_on_gradient_norm_and_ungated_optimization() {
        let telemetry = super::TrainingInstabilityTelemetry::from_step_receipts_and_rollouts(
            &[step_receipt(8.5, Some(0.6))],
            &RolloutIngestionTelemetry::default(),
        )
        .with_environment_failure_rate_bps(200)
        .with_sandbox_failure_rate_bps(100);
        let policy = TrainingInstabilityPolicy::new(
            vec![TrainingInstabilityRule {
                signal: TrainingInstabilitySignalKind::MaxGradientNormL2,
                max_value: 2.0,
                action: TrainingOperationalAction::Halt,
            }],
            vec![],
        );
        let verdict = TrainingStabilityController::new(policy).evaluate(
            &telemetry,
            &[TrainingRiskyOptimization::MixedPrecisionLossScaleBypass],
        );
        assert_eq!(verdict.action, TrainingOperationalAction::Halt);
        assert_eq!(verdict.signal_receipts.len(), 1);
        assert_eq!(verdict.optimization_receipts[0].allowed, false);
    }
}
