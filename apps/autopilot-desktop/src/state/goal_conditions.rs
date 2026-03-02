//! Goal condition evaluation for autonomous goal runs.

use std::collections::BTreeMap;

use crate::state::autopilot_goals::{GoalRecord, GoalStopCondition};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoalProgressSnapshot {
    pub started_at_epoch_seconds: u64,
    pub now_epoch_seconds: u64,
    pub attempt_count: u32,
    pub wallet_delta_sats: i64,
    pub jobs_completed: u32,
    pub successes: u32,
    pub errors: u32,
    pub total_spend_sats: u64,
    pub total_swap_cents: u64,
    pub external_signals: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionEvaluation {
    pub goal_complete: bool,
    pub should_continue: bool,
    pub completion_reasons: Vec<String>,
    pub stop_reasons: Vec<String>,
}

impl ConditionEvaluation {
    fn new() -> Self {
        Self {
            goal_complete: false,
            should_continue: true,
            completion_reasons: Vec::new(),
            stop_reasons: Vec::new(),
        }
    }
}

pub fn evaluate_conditions(
    goal: &GoalRecord,
    progress: &GoalProgressSnapshot,
) -> ConditionEvaluation {
    let mut evaluation = ConditionEvaluation::new();
    let elapsed_seconds = progress
        .now_epoch_seconds
        .saturating_sub(progress.started_at_epoch_seconds);

    let mut completion_condition_total = 0u32;
    let mut completion_condition_met = 0u32;

    for condition in &goal.stop_conditions {
        match condition {
            GoalStopCondition::WalletDeltaSatsAtLeast { sats } => {
                completion_condition_total = completion_condition_total.saturating_add(1);
                if progress.wallet_delta_sats >= *sats as i64 {
                    completion_condition_met = completion_condition_met.saturating_add(1);
                    evaluation
                        .completion_reasons
                        .push(format!("wallet delta reached {} sats", sats));
                }
            }
            GoalStopCondition::JobCountAtLeast { count } => {
                completion_condition_total = completion_condition_total.saturating_add(1);
                if progress.jobs_completed >= *count {
                    completion_condition_met = completion_condition_met.saturating_add(1);
                    evaluation
                        .completion_reasons
                        .push(format!("job count reached {}", count));
                }
            }
            GoalStopCondition::SuccessCountAtLeast { count } => {
                completion_condition_total = completion_condition_total.saturating_add(1);
                if progress.successes >= *count {
                    completion_condition_met = completion_condition_met.saturating_add(1);
                    evaluation
                        .completion_reasons
                        .push(format!("success count reached {}", count));
                }
            }
            GoalStopCondition::ExternalSignal { key, expected } => {
                completion_condition_total = completion_condition_total.saturating_add(1);
                let actual = progress.external_signals.get(key);
                if actual.is_some_and(|value| value == expected) {
                    completion_condition_met = completion_condition_met.saturating_add(1);
                    evaluation
                        .completion_reasons
                        .push(format!("external signal {key} matched expected value"));
                }
            }
            GoalStopCondition::DeadlineEpochSeconds { epoch_seconds } => {
                if progress.now_epoch_seconds >= *epoch_seconds {
                    evaluation
                        .stop_reasons
                        .push(format!("deadline reached at {epoch_seconds}"));
                }
            }
            GoalStopCondition::ErrorBudgetExceeded { max_errors } => {
                if progress.errors > *max_errors {
                    evaluation.stop_reasons.push(format!(
                        "error budget exceeded ({} > {})",
                        progress.errors, max_errors
                    ));
                }
            }
        }
    }

    if completion_condition_total > 0 && completion_condition_met == completion_condition_total {
        evaluation.goal_complete = true;
    }

    if elapsed_seconds >= goal.constraints.max_runtime_seconds {
        evaluation.stop_reasons.push(format!(
            "max runtime reached ({}s >= {}s)",
            elapsed_seconds, goal.constraints.max_runtime_seconds
        ));
    }
    if progress.attempt_count >= goal.constraints.max_attempts {
        evaluation.stop_reasons.push(format!(
            "max attempts reached ({} >= {})",
            progress.attempt_count, goal.constraints.max_attempts
        ));
    }
    if let Some(max_spend_sats) = goal.constraints.max_total_spend_sats
        && progress.total_spend_sats > max_spend_sats
    {
        evaluation.stop_reasons.push(format!(
            "spend limit exceeded ({} sats > {} sats)",
            progress.total_spend_sats, max_spend_sats
        ));
    }
    if let Some(max_swap_cents) = goal.constraints.max_total_swap_cents
        && progress.total_swap_cents > max_swap_cents
    {
        evaluation.stop_reasons.push(format!(
            "swap limit exceeded ({} cents > {} cents)",
            progress.total_swap_cents, max_swap_cents
        ));
    }

    evaluation.should_continue = !evaluation.goal_complete && evaluation.stop_reasons.is_empty();
    evaluation
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::state::autopilot_goals::{
        GoalConstraints, GoalLifecycleStatus, GoalObjective, GoalRecord, GoalRetryPolicy,
        GoalScheduleConfig, GoalStopCondition,
    };

    use super::{GoalProgressSnapshot, evaluate_conditions};

    fn base_goal() -> GoalRecord {
        GoalRecord {
            goal_id: "goal-eval".to_string(),
            title: "Earn +1000 sats and 2 jobs".to_string(),
            objective: GoalObjective::EarnBitcoin {
                min_wallet_delta_sats: 1_000,
                note: None,
            },
            constraints: GoalConstraints {
                max_runtime_seconds: 300,
                max_attempts: 5,
                max_total_spend_sats: Some(10_000),
                max_total_swap_cents: Some(20_000),
                swap_policy: crate::state::swap_contract::SwapPolicy::default(),
            },
            stop_conditions: vec![
                GoalStopCondition::WalletDeltaSatsAtLeast { sats: 1_000 },
                GoalStopCondition::JobCountAtLeast { count: 2 },
            ],
            retry_policy: GoalRetryPolicy::default(),
            schedule: GoalScheduleConfig::default(),
            lifecycle_status: GoalLifecycleStatus::Queued,
            created_at_epoch_seconds: 100,
            updated_at_epoch_seconds: 100,
            attempt_count: 0,
            last_failure_reason: None,
            terminal_reason: None,
            last_receipt_id: None,
        }
    }

    fn base_progress() -> GoalProgressSnapshot {
        GoalProgressSnapshot {
            started_at_epoch_seconds: 100,
            now_epoch_seconds: 120,
            attempt_count: 1,
            wallet_delta_sats: 0,
            jobs_completed: 0,
            successes: 0,
            errors: 0,
            total_spend_sats: 0,
            total_swap_cents: 0,
            external_signals: BTreeMap::new(),
        }
    }

    #[test]
    fn evaluate_requires_all_completion_conditions() {
        let goal = base_goal();
        let mut progress = base_progress();
        progress.wallet_delta_sats = 1_000;
        progress.jobs_completed = 1;

        let evaluation = evaluate_conditions(&goal, &progress);
        assert!(!evaluation.goal_complete);
        assert!(evaluation.should_continue);
    }

    #[test]
    fn evaluate_marks_goal_complete_when_all_targets_met() {
        let goal = base_goal();
        let mut progress = base_progress();
        progress.wallet_delta_sats = 1_200;
        progress.jobs_completed = 2;

        let evaluation = evaluate_conditions(&goal, &progress);
        assert!(evaluation.goal_complete);
        assert!(!evaluation.should_continue);
    }

    #[test]
    fn evaluate_stops_on_runtime_and_error_budget() {
        let mut goal = base_goal();
        goal.stop_conditions
            .push(GoalStopCondition::ErrorBudgetExceeded { max_errors: 1 });
        let mut progress = base_progress();
        progress.now_epoch_seconds = 500;
        progress.errors = 3;

        let evaluation = evaluate_conditions(&goal, &progress);
        assert!(!evaluation.goal_complete);
        assert!(!evaluation.should_continue);
        assert!(
            evaluation
                .stop_reasons
                .iter()
                .any(|reason| reason.contains("max runtime reached"))
        );
        assert!(
            evaluation
                .stop_reasons
                .iter()
                .any(|reason| reason.contains("error budget exceeded"))
        );
    }
}
