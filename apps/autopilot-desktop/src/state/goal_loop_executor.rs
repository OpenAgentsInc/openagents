//! Autonomous goal loop runtime state for iterative turn execution.

use serde::{Deserialize, Serialize};

use crate::state::autopilot_goals::{GoalLifecycleStatus, GoalRecord, GoalRetryPolicy};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GoalLoopPhase {
    WaitingForThread,
    DispatchingTurn,
    WaitingForTurnResult,
    Backoff,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GoalLoopStopReason {
    GoalComplete,
    ConditionStop {
        reasons: Vec<String>,
    },
    RetryLimitExceeded {
        retries_used: u32,
        max_retries: u32,
        last_error: String,
    },
    DispatchFailed {
        error: String,
    },
    GoalMissing,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GoalLoopAttemptRecord {
    pub attempt_index: u32,
    pub submitted_at_epoch_seconds: u64,
    pub finished_at_epoch_seconds: Option<u64>,
    pub turn_status: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActiveGoalLoopRun {
    pub run_id: String,
    pub goal_id: String,
    pub started_at_epoch_seconds: u64,
    pub initial_wallet_sats: u64,
    pub recovered_from_restart: bool,
    pub phase: GoalLoopPhase,
    pub retries_used: u32,
    pub backoff_until_epoch_seconds: Option<u64>,
    pub attempts: Vec<GoalLoopAttemptRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GoalLoopRunReceipt {
    pub run_id: String,
    pub goal_id: String,
    pub started_at_epoch_seconds: u64,
    pub finished_at_epoch_seconds: u64,
    pub recovered_from_restart: bool,
    pub lifecycle_status: GoalLifecycleStatus,
    pub stop_reason: GoalLoopStopReason,
    pub attempts: Vec<GoalLoopAttemptRecord>,
    pub retries_used: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GoalLoopExecutorState {
    pub active_run: Option<ActiveGoalLoopRun>,
    pub run_receipts: Vec<GoalLoopRunReceipt>,
}

impl Default for GoalLoopExecutorState {
    fn default() -> Self {
        Self {
            active_run: None,
            run_receipts: Vec::new(),
        }
    }
}

impl GoalLoopExecutorState {
    pub fn begin_run(
        &mut self,
        goal_id: &str,
        now_epoch_seconds: u64,
        initial_wallet_sats: u64,
        recovered_from_restart: bool,
    ) -> bool {
        if self.active_run.is_some() {
            return false;
        }
        self.active_run = Some(ActiveGoalLoopRun {
            run_id: format!("goal-run-{goal_id}-{now_epoch_seconds}"),
            goal_id: goal_id.to_string(),
            started_at_epoch_seconds: now_epoch_seconds,
            initial_wallet_sats,
            recovered_from_restart,
            phase: GoalLoopPhase::DispatchingTurn,
            retries_used: 0,
            backoff_until_epoch_seconds: None,
            attempts: Vec::new(),
        });
        true
    }

    pub fn mark_attempt_submitted(&mut self, now_epoch_seconds: u64) {
        let Some(run) = self.active_run.as_mut() else {
            return;
        };
        let attempt_index = run.attempts.len() as u32 + 1;
        run.phase = GoalLoopPhase::WaitingForTurnResult;
        run.backoff_until_epoch_seconds = None;
        run.attempts.push(GoalLoopAttemptRecord {
            attempt_index,
            submitted_at_epoch_seconds: now_epoch_seconds,
            finished_at_epoch_seconds: None,
            turn_status: None,
            error: None,
        });
    }

    pub fn mark_attempt_finished(
        &mut self,
        now_epoch_seconds: u64,
        turn_status: &str,
        error: Option<String>,
    ) {
        let Some(run) = self.active_run.as_mut() else {
            return;
        };
        if let Some(last) = run.attempts.last_mut()
            && last.finished_at_epoch_seconds.is_none()
        {
            last.finished_at_epoch_seconds = Some(now_epoch_seconds);
            last.turn_status = Some(turn_status.to_string());
            last.error = error;
        }
    }

    pub fn mark_backoff(&mut self, until_epoch_seconds: u64) {
        let Some(run) = self.active_run.as_mut() else {
            return;
        };
        run.phase = GoalLoopPhase::Backoff;
        run.backoff_until_epoch_seconds = Some(until_epoch_seconds);
    }

    pub fn mark_dispatching(&mut self) {
        let Some(run) = self.active_run.as_mut() else {
            return;
        };
        run.phase = GoalLoopPhase::DispatchingTurn;
        run.backoff_until_epoch_seconds = None;
    }

    pub fn increment_retries(&mut self) -> u32 {
        let Some(run) = self.active_run.as_mut() else {
            return 0;
        };
        run.retries_used = run.retries_used.saturating_add(1);
        run.retries_used
    }

    pub fn complete_run(
        &mut self,
        now_epoch_seconds: u64,
        lifecycle_status: GoalLifecycleStatus,
        stop_reason: GoalLoopStopReason,
    ) {
        let Some(run) = self.active_run.take() else {
            return;
        };
        self.run_receipts.push(GoalLoopRunReceipt {
            run_id: run.run_id,
            goal_id: run.goal_id,
            started_at_epoch_seconds: run.started_at_epoch_seconds,
            finished_at_epoch_seconds: now_epoch_seconds,
            recovered_from_restart: run.recovered_from_restart,
            lifecycle_status,
            stop_reason,
            attempts: run.attempts,
            retries_used: run.retries_used,
        });
        if self.run_receipts.len() > 1_024 {
            let overflow = self.run_receipts.len().saturating_sub(1_024);
            self.run_receipts.drain(0..overflow);
        }
    }
}

pub fn select_runnable_goal(goals: &[GoalRecord]) -> Option<&GoalRecord> {
    goals
        .iter()
        .filter_map(|goal| runnable_goal_rank(goal.lifecycle_status).map(|rank| (rank, goal)))
        .min_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.goal_id.cmp(&right.1.goal_id))
        })
        .map(|entry| entry.1)
}

fn runnable_goal_rank(status: GoalLifecycleStatus) -> Option<u8> {
    match status {
        GoalLifecycleStatus::Running => Some(0),
        GoalLifecycleStatus::Queued => Some(1),
        _ => None,
    }
}

pub fn retry_backoff_seconds(policy: &GoalRetryPolicy, retries_used: u32) -> u64 {
    if retries_used == 0 {
        return policy.backoff_seconds.max(1);
    }
    if !policy.exponential_backoff {
        return policy.backoff_seconds.max(1);
    }
    let exp = retries_used.saturating_sub(1).min(10);
    policy.backoff_seconds.max(1).saturating_mul(1u64 << exp)
}

#[cfg(test)]
mod tests {
    use super::{GoalLoopExecutorState, retry_backoff_seconds, select_runnable_goal};
    use crate::state::autopilot_goals::{
        GoalConstraints, GoalLifecycleStatus, GoalObjective, GoalRecord, GoalRetryPolicy,
        GoalScheduleConfig, GoalStopCondition,
    };

    fn sample_goal(goal_id: &str, lifecycle_status: GoalLifecycleStatus) -> GoalRecord {
        GoalRecord {
            goal_id: goal_id.to_string(),
            title: "Goal".to_string(),
            objective: GoalObjective::Custom {
                instruction: "do work".to_string(),
            },
            constraints: GoalConstraints::default(),
            stop_conditions: vec![GoalStopCondition::SuccessCountAtLeast { count: 1 }],
            retry_policy: GoalRetryPolicy::default(),
            schedule: GoalScheduleConfig::default(),
            lifecycle_status,
            created_at_epoch_seconds: 1_700_000_000,
            updated_at_epoch_seconds: 1_700_000_000,
            attempt_count: 0,
            last_failure_reason: None,
            terminal_reason: None,
            last_receipt_id: None,
            recovery_replay_pending: false,
        }
    }

    #[test]
    fn select_runnable_goal_prefers_running_then_deterministic_goal_id() {
        let goals = vec![
            sample_goal("goal-c", GoalLifecycleStatus::Queued),
            sample_goal("goal-b", GoalLifecycleStatus::Running),
            sample_goal("goal-a", GoalLifecycleStatus::Running),
        ];
        let selected = select_runnable_goal(&goals).expect("must pick a runnable goal");
        assert_eq!(selected.goal_id, "goal-a");
    }

    #[test]
    fn select_runnable_goal_returns_none_when_no_runnable_status_present() {
        let goals = vec![
            sample_goal("goal-a", GoalLifecycleStatus::Draft),
            sample_goal("goal-b", GoalLifecycleStatus::Succeeded),
        ];
        assert!(select_runnable_goal(&goals).is_none());
    }

    #[test]
    fn retry_backoff_honors_exponential_policy() {
        let policy = GoalRetryPolicy {
            max_retries: 5,
            backoff_seconds: 3,
            exponential_backoff: true,
        };
        assert_eq!(retry_backoff_seconds(&policy, 1), 3);
        assert_eq!(retry_backoff_seconds(&policy, 2), 6);
        assert_eq!(retry_backoff_seconds(&policy, 3), 12);
    }

    #[test]
    fn retry_backoff_uses_fixed_policy_when_disabled() {
        let policy = GoalRetryPolicy {
            max_retries: 5,
            backoff_seconds: 7,
            exponential_backoff: false,
        };
        assert_eq!(retry_backoff_seconds(&policy, 1), 7);
        assert_eq!(retry_backoff_seconds(&policy, 4), 7);
    }

    #[test]
    fn begin_run_rejects_duplicate_concurrent_goal_run() {
        let mut executor = GoalLoopExecutorState::default();
        assert!(executor.begin_run("goal-a", 1_700_000_000, 1_000, false));
        assert!(!executor.begin_run("goal-a", 1_700_000_001, 1_100, true));
    }
}
